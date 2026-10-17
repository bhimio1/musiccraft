use super::*;

/// A scratch folder with `files` (relative paths) created empty.
fn tree(name: &str, files: &[&str]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("musiccraft-paths-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for file in files {
        let path = root.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }
    root
}

fn resolver(config: PathConfig) -> Resolver {
    Resolver::with_config(config, true)
}

#[test]
fn foreign_paths_are_recognised() {
    assert!(is_foreign(r"C:\Users\Ana\kick.wav"));
    assert!(is_foreign("d:/samples/snare.wav"));
    assert!(is_foreign(r"%FLStudioFactoryData%\Data\kick.wav"));
    assert!(is_foreign(r"Samples\kick.wav"));
    assert!(!is_foreign("/home/ana/kick.wav"));
    assert!(!is_foreign("samples/kick.wav"));
    assert!(!is_foreign("builtin:kick"));
    assert!(!is_foreign("50% wet.wav"));
}

#[test]
fn fl_factory_data_resolves_ignoring_case() {
    let root = tree(
        "factory",
        &["FL Studio 21/Data/Patches/Packs/Drums/Kicks/22in kick.wav"],
    );
    let config = PathConfig {
        fl_factory: vec![root.join("FL Studio 21")],
        ..PathConfig::default()
    };
    let found = resolver(config).resolve(
        r"%FLStudioFactoryData%\Data\Patches\Packs\Drums\Kicks\22in Kick.wav",
        None,
    );
    assert_eq!(
        found,
        Some(root.join("FL Studio 21/Data/Patches/Packs/Drums/Kicks/22in kick.wav"))
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn drive_letters_use_mappings_then_wine_then_home() {
    let root = tree(
        "drives",
        &[
            "music/loops/a.wav",
            "prefix/drive_c/Samples/b.wav",
            "prefix/dosdevices/d:/c.wav",
            "home/Music/d.wav",
        ],
    );
    let config = PathConfig {
        drives: vec![(normalize_prefix(r"C:\Users\Ana\Music"), root.join("music"))],
        wine_prefixes: vec![root.join("prefix")],
        home: Some(root.join("home")),
        ..PathConfig::default()
    };
    let mut r = resolver(config);
    assert_eq!(
        r.resolve(r"C:\Users\Ana\Music\Loops\A.WAV", None),
        Some(root.join("music/loops/a.wav")),
        "the mapping wins and case is ignored"
    );
    assert_eq!(
        r.resolve(r"C:\Samples\b.wav", None),
        Some(root.join("prefix/drive_c/Samples/b.wav"))
    );
    assert_eq!(
        r.resolve("D:/c.wav", None),
        Some(root.join("prefix/dosdevices/d:/c.wav"))
    );
    assert_eq!(
        r.resolve(r"C:\Users\Bob\Music\d.wav", None),
        Some(root.join("home/Music/d.wav")),
        "a Windows profile folder maps to home"
    );
    assert_eq!(r.resolve(r"C:\nowhere\e.wav", None), None);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn the_longest_mapping_wins() {
    let config = PathConfig {
        drives: vec![
            ("c:".to_owned(), PathBuf::from("/a")),
            ("c:/samples".to_owned(), PathBuf::from("/b")),
        ],
        ..PathConfig::default()
    };
    let list = candidates(r"C:\Samples\x.wav", None, &config);
    assert_eq!(list[0], PathBuf::from("/b/x.wav"));
    assert_eq!(list[1], PathBuf::from("/a/Samples/x.wav"));
    // A prefix must end at a folder boundary.
    let list = candidates(r"C:\SamplesOld\x.wav", None, &config);
    assert_eq!(list[0], PathBuf::from("/a/SamplesOld/x.wav"));
}

#[test]
fn file_names_are_found_beside_the_project_and_in_search_folders() {
    let root = tree(
        "search",
        &["project/Audio/snare.wav", "library/Drums/Hats/hat.wav"],
    );
    let config = PathConfig {
        search: vec![root.join("library")],
        ..PathConfig::default()
    };
    let mut r = resolver(config);
    let base = root.join("project");
    assert_eq!(
        r.resolve(r"D:\Old Disk\Snare.wav", Some(&base)),
        Some(root.join("project/Audio/snare.wav"))
    );
    assert_eq!(
        r.resolve(r"%FLStudioUserData%\Packs\hat.wav", None),
        Some(root.join("library/Drums/Hats/hat.wav")),
        "with no FL folder set, a placeholder path is still found by name"
    );
    assert_eq!(
        r.resolve(r"E:\x\hat.wav", None),
        Some(root.join("library/Drums/Hats/hat.wav"))
    );
    // Relative paths with backslashes resolve from the project's folder.
    assert_eq!(
        r.resolve(r"Audio\snare.wav", Some(&base)),
        Some(root.join("project/Audio/snare.wav"))
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_inactive_resolver_translates_nothing() {
    let root = tree("inactive", &["x/a.wav"]);
    let config = PathConfig {
        drives: vec![("c:".to_owned(), root.join("x"))],
        ..PathConfig::default()
    };
    assert_eq!(
        Resolver::with_config(config, false).resolve("C:/a.wav", None),
        None
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn overrides_come_first_and_maps_parse() {
    let base = PathConfig {
        fl_factory: vec![PathBuf::from("/detected")],
        drives: vec![("d:".to_owned(), PathBuf::from("/d"))],
        ..PathConfig::default()
    };
    let config = base.with_overrides(&Overrides {
        fl_factory: Some(PathBuf::from("/mine")),
        drives: vec![(r"C:\".to_owned(), PathBuf::from("/c"))],
        ..Overrides::default()
    });
    assert_eq!(
        config.fl_factory,
        [PathBuf::from("/mine"), PathBuf::from("/detected")]
    );
    assert_eq!(config.drives[0], ("c:".to_owned(), PathBuf::from("/c")));
    assert_eq!(
        parse_drive_map("C:=/mnt/win; D:/Samples = /home/ana/s"),
        [
            ("c:".to_owned(), PathBuf::from("/mnt/win")),
            ("d:/samples".to_owned(), PathBuf::from("/home/ana/s")),
        ]
    );
}
