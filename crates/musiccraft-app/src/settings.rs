//! Loading and saving user settings (see `docs/spec/settings.md`).

use std::path::PathBuf;

use musiccraft_ui::Settings;

/// The platform's per-user configuration folder for MusicCraft.
fn config_dir() -> Option<PathBuf> {
    let env = |key: &str| {
        std::env::var_os(key)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(target_os = "windows") {
        env("APPDATA").map(|p| p.join("MusicCraft"))
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|p| p.join("Library/Application Support/MusicCraft"))
    } else {
        env("XDG_CONFIG_HOME")
            .or_else(|| env("HOME").map(|h| h.join(".config")))
            .map(|p| p.join("musiccraft"))
    }
}

/// Where settings are stored.
pub fn path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("settings.json"))
}

/// The saved settings, or defaults when there are none (or they are broken).
pub fn load() -> Settings {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes settings atomically.
pub fn save(settings: &Settings) -> Result<(), String> {
    let path = path().ok_or("no configuration folder")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, json)
        .and_then(|()| std::fs::rename(&temp, &path))
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use musiccraft_ui::MidiInput;

    use super::*;

    #[test]
    fn settings_round_trip_and_tolerate_old_files() {
        let settings = Settings {
            sample_rate: Some(48_000),
            midi_input: MidiInput::Port("Keys".into()),
            plugin_folders: vec!["/opt/clap".into()],
            ..Settings::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(serde_json::from_str::<Settings>(&json).unwrap(), settings);
        let old: Settings = serde_json::from_str(r#"{"ui_scale": 1.5}"#).unwrap();
        assert!((old.scale() - 1.5).abs() < 1e-6);
        assert!(old.autosave);
        assert!(old.scan_plugins_on_start, "scanning stays on for old files");
        assert_eq!(old.midi_input, MidiInput::Auto);
    }
}
