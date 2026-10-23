use musiccraft_project::params::{TICKS_PER_STEP, inst};
use musiccraft_project::{Instrument, PlayMode, Project, TimeSig};

use crate::{CommandError, Registry, Session};

/// A 4/4 bar.
const TICKS_PER_BAR: u32 = 384;

fn session() -> Session {
    Session::new(Project::default(), Registry::with_builtin())
}

fn run(s: &mut Session, line: &str) {
    if let Err(err) = s.run_line(line) {
        panic!("`{line}` failed: {err}");
    }
}

fn fails(s: &mut Session, line: &str) -> CommandError {
    let before = s.project().clone();
    let err = s.run_line(line).expect_err(line);
    assert_eq!(
        s.project(),
        &before,
        "`{line}` changed the project on error"
    );
    err
}

/// Every undoable command, in an order where each line is valid.
const SCRIPT: &[&str] = &[
    "project.set_tempo 140",
    "project.set_pan_law triangular",
    "project.set_master_pitch -50",
    "project.rename My Song",
    "channel.add_sampler builtin:kick",
    "channel.add_sampler builtin:snare Snare Drum",
    "channel.add_synth Lead",
    "channel.rename 3 Bass",
    "channel.set_group 3 Low end",
    "channel.set_group 1 Drums",
    "channel.set_group 3",
    "channel.set_color 2 7",
    "channel.set_color 2 none",
    "channel.set_color 1 16",
    "channel.set_volume 1 0.5",
    "channel.set_pan 2 -0.25",
    "channel.set_mute 2 on",
    "channel.toggle_mute 2",
    "channel.solo 1",
    "channel.solo 1",
    "channel.set_solo 2 on",
    "channel.solo 1",
    "channel.set_solo 1 off",
    "channel.route 1 2",
    "channel.set_sample 1 builtin:clap",
    "channel.swap_sound 1 2",
    "channel.swap_sound 2 1",
    "channel.set_param 3 cutoff 1200",
    "channel.set_param 3 wave1 3",
    "step.toggle 1 0",
    "step.toggle 1 4",
    "step.set 2 4 on",
    "step.toggle 1 4",
    "note.add 3 0 2 C4 90",
    "note.add 3 2 2 E4",
    "note.add 3 4 1.5 G4",
    "note.move 3 2 E4 3 F4",
    "note.resize 3 0 C4 4",
    "note.slice 3 0 C4 1",
    "note.limit 3 C4 B5",
    "note.lfo 3 velocity 8 50",
    "note.scale_levels 3 pan 50 10",
    "note.flam 3 6t",
    "note.articulate 3 80 5",
    "note.set_velocity 3 0 C4 127",
    "note.set_pan 3 0 C4 -40",
    "note.set_slide 3 0 C4 on",
    "note.set_prop 3 0 C4 release 100",
    "note.set_prop 3 0 C4 fine -30",
    "note.set_prop 3 0 C4 mod_x 90",
    "note.set_prop 3 0 C4 mod_y 20",
    "note.set_prop 3 0 C4 color 5",
    "note.set_prop 3 0 C4 porta on",
    "note.set_prop 3 3 F4 muted on",
    "note.add 3 10 1 C5 90 0 slide",
    "note.add 3 6 1 B4 80 25",
    "note.transpose 3 12",
    "note.quantize 3 2",
    "note.add_chord 3 8 4 A3 min7 90",
    "channel.clone 1",
    "channel.move 4 1",
    "pattern.add Verse",
    "step.toggle 2 8",
    "pattern.clone 1",
    "pattern.rename 2 Chorus",
    "pattern.set_color 2 5",
    "pattern.set_length 2 32",
    "project.set_swing 33",
    "note.clear 3 @1",
    "mixer.add_track Drums",
    "mixer.set_separation 1 -0.5",
    "mixer.set_phase_invert 1 on",
    "mixer.set_swap_sides 1 on",
    "mixer.set_swap_sides 1 off",
    "mixer.set_track_color 1 3",
    "mixer.set_track_color 1 none",
    "mixer.set_dock 2 right",
    "mixer.set_dock 3 left",
    "mixer.set_dock 2 middle",
    "mixer.rename 1 Kick bus",
    "mixer.set_volume 1 0.7",
    "mixer.set_pan 0 0.1",
    "mixer.set_mute 3 on",
    "mixer.toggle_mute 3",
    "mixer.set_solo 2 on",
    "mixer.toggle_solo 2",
    "mixer.add_effect 2 reverb",
    "mixer.add_effect 2 delay",
    "mixer.add_effect 0 compressor",
    "mixer.set_effect_param 2 1 size 0.9",
    "mixer.set_effect_enabled 2 2 off",
    "mixer.toggle_effect 2 2",
    "mixer.move_effect 2 2 1",
    "mixer.remove_effect 2 1",
    "mixer.add_effect 2 rack",
    "mixer.set_effect_param 2 2 dry 0.5",
    "mixer.rack_add 2 2 1 filter",
    "mixer.rack_add 2 2 1 chorus",
    "mixer.rack_add 2 2 3 reverb",
    "mixer.rack_param 2 2 1 1 cutoff 800",
    "mixer.rack_enable 2 2 1 2 off",
    "mixer.rack_remove 2 2 1 1",
    "mixer.set_send 1 2 0.5",
    "mixer.set_send 1 4 0.25",
    "mixer.set_send 3 4 1",
    "mixer.set_send 1 4 0.3",
    "mixer.route_to_master 1 off",
    "playlist.add_clip 1 1 0",
    "playlist.add_clip 2 1 1",
    "playlist.add_clip 3 2 0 4",
    "playlist.move_clip 2 2 3 4",
    "playlist.resize_clip 1 0 0.5",
    "playlist.set_track_mute 3 on",
    "playlist.toggle_track_mute 3",
    "playlist.set_track_solo 2 on",
    "playlist.toggle_track_solo 4",
    "playlist.toggle_track_arm 5",
    "playlist.rename_track 2 Drums",
    "playlist.rename_track 2 Lead line",
    "playlist.rename_track 2",
    "playlist.rename_track 3 Bass",
    "playlist.set_track_color 3 12",
    "playlist.set_track_color 3 none",
    "playlist.set_track_color 4 2",
    "playlist.set_track_height 3 200",
    "playlist.set_track_height 5 50",
    "playlist.set_track_height 3 reset",
    "playlist.reset_track_heights",
    "playlist.slice_clip 3 6",
    "playlist.toggle_clip_mute 3 6",
    "playlist.make_unique 3 6",
    "playlist.insert_space 2 1",
    "playlist.delete_space 2 1",
    "playlist.remove_clip 1 1.5",
    "automation.create channel:2:cutoff",
    "automation.create mixer:0:volume Fade out",
    "automation.add_point 1 8 0.25",
    "automation.add_point 1 8 0.75",
    "automation.move_point 1 8 12 0.5",
    "automation.remove_point 1 12",
    "automation.rename 1 Filter sweep",
    "playlist.add_automation_clip 1 5 0",
    "playlist.add_automation_clip 2 6 2 1",
    "automation.record channel:2:cutoff 4",
    "automation.record channel:1:volume 8",
    "pattern.record_event channel:2:cutoff 2",
    "pattern.record_event channel:2:cutoff 6",
    "pattern.clear_events 1 channel:2:cutoff",
    "channel.add_sampler builtin:rim",
    "playlist.add_audio_clip 5 7 1 2",
    "playlist.move_clip 7 1 8 3",
    "automation.create mixer:2:1:mix",
    "mixer.add_effect 2 chorus",
    "mixer.move_effect 2 1 2",
    "automation.remove 1",
    "automation.remove 2",
    // One more: the recorded volume automation came before the mix one.
    "automation.remove 2",
    "mixer.remove_track 2",
    "pattern.remove 1",
    "channel.remove 2",
    "transport.set_loop 1 3",
    "transport.set_loop 2 4",
    "transport.clear_loop",
    "transport.set_loop 0 2",
    "playlist.add_marker 1 Verse",
    "playlist.add_marker 3 Drop",
    "playlist.add_marker 1 Intro",
    "playlist.remove_marker 3",
    "playlist.set_marker_action 1 pause",
    "playlist.add_marker 5 End",
    "playlist.set_marker_action 5 song_loop",
    "playlist.add_marker 2 Chorus",
    "playlist.set_marker_action 2 song_loop",
    "playlist.add_marker 1 Intro again",
    "playlist.remove_marker 2",
    "playlist.clear",
    r#"edit.batch "project.set_tempo 90" "channel.set_volume 1 0.3""#,
];

#[test]
fn every_command_undoes_and_redoes_exactly() {
    assert_round_trip(SCRIPT);
}

/// Every undoable plugin command, plus the generic ones that must carry
/// plugin data through undo.
const PLUGIN_SCRIPT: &[&str] = &[
    "channel.add_plugin com.example.keys Keys",
    "channel.add_plugin org.test.bass",
    "channel.set_plugin_param 1 7 0.25",
    "channel.set_plugin_param 1 7 0.5",
    "channel.set_plugin_param 1 9 -3",
    "channel.set_plugin_param 1 9 unset",
    "channel.clone 1",
    "mixer.add_plugin 0 com.example.verb Big Verb",
    "mixer.add_effect 0 delay",
    "mixer.set_plugin_param 0 1 3 0.75",
    "mixer.move_effect 0 1 2",
    "mixer.remove_effect 0 2",
    "note.add 2 0 1 C4",
    "channel.remove 1",
];

#[test]
fn plugin_commands_undo_and_redo_exactly() {
    assert_round_trip(PLUGIN_SCRIPT);
}

/// Every undoable Antiserum command.
const ANTISERUM_SCRIPT: &[&str] = &[
    "channel.add_antiserum",
    "channel.add_antiserum Glass Reese",
    "channel.set_param 1 osc1_unison 7",
    "channel.set_param 1 fx_reverb_decay 4.5",
    "channel.set_param 1 osc1_unison 1",
    // The channel's own pitch (the header knob) on an Antiserum channel.
    "channel.set_param 1 pitch 5",
    "channel.set_param 1 swing_mix 0.5",
    "antiserum.set_table 1 2 factory:formant",
    "antiserum.set_table 1 3 factory:pwm",
    "antiserum.set_lfo_shape 1 3 0:0:0,0.5:1:-0.4",
    "antiserum.set_lfo_shape 1 3 -",
    "antiserum.set_step 1 5 off 7 0.5 0.25",
    "antiserum.load_preset 1 Acid Line",
    "channel.set_param 1 osc2_type 3",
    "antiserum.set_sample 1 2 builtin:crash",
    "antiserum.set_sample 1 2 builtin:snare",
    "antiserum.add_zone 1 3 builtin:kick 0 59 36",
    "antiserum.add_zone 1 3 builtin:hat",
    "antiserum.set_zone 1 3 2 60 127 72",
    "antiserum.remove_zone 1 3 1",
    "antiserum.set_sample 1 2 -",
    "automation.create channel:2:filter1_cutoff",
    "channel.clone 2",
    "channel.remove 1",
];

#[test]
fn antiserum_commands_undo_and_redo_exactly() {
    assert_round_trip(ANTISERUM_SCRIPT);
}

#[test]
fn antiserum_tables_take_frame_data() {
    let mut s = session();
    run(&mut s, "channel.add_antiserum");
    let data = musiccraft_patch::TableRef::Data(vec![0.25; 2048]).to_text();
    run(&mut s, &format!("antiserum.set_table 1 2 {data}"));
    let patch = s.project().channels[0].instrument.patch().unwrap();
    assert_eq!(patch.table(1), data);
    run(&mut s, "edit.undo");
    let patch = s.project().channels[0].instrument.patch().unwrap();
    assert_eq!(patch.table(1), "factory:basic");
}

#[test]
fn antiserum_commands_check_their_arguments() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.add_antiserum");
    fails(&mut s, "channel.add_antiserum No Such Preset");
    fails(&mut s, "antiserum.set_table 1 1 factory:basic");
    fails(&mut s, "antiserum.set_table 2 4 factory:basic");
    fails(&mut s, "antiserum.set_table 2 1 factory:nope");
    fails(&mut s, "antiserum.set_table 2 1 data:AA==");
    fails(&mut s, "antiserum.set_lfo_shape 2 9 -");
    fails(&mut s, "antiserum.set_lfo_shape 2 1 0:x");
    fails(&mut s, "antiserum.set_step 2 65 on 0 1 0.5");
    fails(&mut s, "antiserum.set_step 2 1 on 30 1 0.5");
    fails(&mut s, "antiserum.load_preset 2 Nope");
    fails(&mut s, "antiserum.set_sample 2 1 builtin:nope");
    fails(&mut s, "antiserum.set_sample 2 1 /no/such/file.wav");
    fails(&mut s, "antiserum.set_sample 2 4 builtin:kick");
    fails(&mut s, "antiserum.add_zone 2 1 builtin:kick 70 60");
    fails(&mut s, "antiserum.set_zone 2 1 1 0 127 60");
    fails(&mut s, "antiserum.remove_zone 2 1 1");
    fails(&mut s, "antiserum.set_patch 2 [1]");
    fails(&mut s, "channel.set_param 2 cutoff 100");
    fails(&mut s, "channel.set_param 2 osc1_level 7");
    fails(&mut s, "automation.create channel:2:cutoff");
    // The synth's own names still work on the synth channel.
    run(&mut s, "channel.set_param 1 cutoff 1200");
    run(&mut s, "channel.set_param 2 filter1_cutoff 1200");
    let patch = s.project().channels[1].instrument.patch().unwrap();
    assert_eq!(patch.params.get("filter1_cutoff"), Some(&1200.0));
    // A default value is not stored.
    run(&mut s, "channel.set_param 2 filter1_cutoff 20000");
    let patch = s.project().channels[1].instrument.patch().unwrap();
    assert!(!patch.params.contains_key("filter1_cutoff"));
}

/// Held by tests that point the user presets at a folder of their own.
static USER_PRESETS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn antiserum_user_presets_save_list_load_and_delete() {
    let _folder = USER_PRESETS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let dir = std::env::temp_dir().join(format!("mc-cmd-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    musiccraft_antiserum_presets::user::set_folder(Some(dir.clone()));
    let mut s = session();
    run(&mut s, "channel.add_antiserum");
    run(&mut s, "channel.set_param 1 osc1_unison 5");
    // A factory name is refused; a new one is saved and names the sound.
    assert!(s.run_line("antiserum.save_preset 1 Glass Reese").is_err());
    run(&mut s, "antiserum.save_preset 1 My Wide Init");
    assert!(dir.join("My Wide Init.json").is_file());
    let patch = s.project().channels[0].instrument.patch().unwrap();
    assert_eq!(patch.name, "My Wide Init");
    run(&mut s, "edit.undo");
    let patch = s.project().channels[0].instrument.patch().unwrap();
    assert_ne!(patch.name, "My Wide Init", "undo restores the old name");
    // It is listed, loads and adds like a factory preset.
    let listed = s.run_line("antiserum.presets").unwrap().message;
    assert!(
        listed
            .lines()
            .any(|l| l.starts_with("My Wide Init\t") && l.ends_with(" user"))
    );
    run(&mut s, "channel.add_antiserum My Wide Init");
    let added = s.project().channels[1].instrument.patch().unwrap();
    assert_eq!(added.params.get("osc1_unison"), Some(&5.0));
    run(&mut s, "antiserum.load_preset 1 my wide init");
    run(&mut s, "antiserum.delete_preset My Wide Init");
    assert!(s.run_line("antiserum.load_preset 1 My Wide Init").is_err());
    musiccraft_antiserum_presets::user::set_folder(None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn antiserum_opens_and_imports_serum_presets() {
    use musiccraft_serum::fixtures::{self, map, module, n};
    let dir = std::env::temp_dir().join(format!("mc-serum-open-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let fxp = dir.join("Zed One.fxp");
    std::fs::write(
        &fxp,
        fixtures::serum1("Zed One", &[("A Vol", 0.5)], "", &[]),
    )
    .unwrap();
    let state = map(vec![(
        "Oscillator0",
        module(vec![("kParamVolume", n(0.5))], vec![]),
    )]);
    let s2 = dir.join("Zed Two.SerumPreset");
    std::fs::write(&s2, fixtures::serum2("Zed Two", &[], &state)).unwrap();
    let other = dir.join("other.fxp");
    let mut data = fixtures::serum1("Other", &[], "", &[]);
    data[16..20].copy_from_slice(b"ABCD");
    std::fs::write(&other, data).unwrap();

    let mut s = session();
    run(&mut s, "channel.add_antiserum Choir Pad");
    // The open prompt takes Serum files: the first bytes decide.
    let opened = s
        .run_line(&format!("antiserum.open_patch 1 \"{}\"", fxp.display()))
        .unwrap();
    assert!(
        opened.message.starts_with("channel 1 plays Zed One"),
        "{}",
        opened.message
    );
    let patch = s.project().channels[0].instrument.patch().unwrap();
    assert_eq!(patch.name, "Zed One");
    assert_eq!(patch.params.get("osc1_level"), Some(&0.25));
    run(&mut s, "edit.undo");
    let patch = s.project().channels[0].instrument.patch().unwrap();
    assert_eq!(patch.name, "Choir Pad");
    run(
        &mut s,
        &format!("antiserum.import_serum 1 \"{}\"", s2.display()),
    );
    let patch = s.project().channels[0].instrument.patch().unwrap();
    assert_eq!(patch.name, "Zed Two");
    // Not Serum: refused, the project unchanged.
    fails(
        &mut s,
        &format!("antiserum.import_serum 1 {}", other.display()),
    );
    let json = dir.join("patch.json");
    std::fs::write(&json, "{}").unwrap();
    fails(
        &mut s,
        &format!("antiserum.import_serum 1 {}", json.display()),
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn antiserum_imports_a_folder_of_serum_presets() {
    use musiccraft_serum::fixtures;
    let _folder = USER_PRESETS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let base = std::env::temp_dir().join(format!("mc-serum-bulk-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let pack = base.join("pack").join("Leads");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(
        pack.join("a.fxp"),
        fixtures::serum1("Zed Lead", &[], "", &[]),
    )
    .unwrap();
    // A factory preset's name is not taken over.
    std::fs::write(
        pack.join("b.fxp"),
        fixtures::serum1("Glass Reese", &[], "", &[]),
    )
    .unwrap();
    let mut other = fixtures::serum1("Other", &[], "", &[]);
    other[16..20].copy_from_slice(b"ABCD");
    std::fs::write(pack.join("c.fxp"), other).unwrap();
    std::fs::write(pack.join("d.fxp"), b"CcnK broken").unwrap();
    std::fs::write(pack.join("notes.txt"), b"not a preset").unwrap();
    let presets = base.join("user");
    musiccraft_antiserum_presets::user::set_folder(Some(presets.clone()));
    let mut s = session();
    let depth = s.undo_depth();
    let pack_dir = base.join("pack");
    let first = s
        .run_line(&format!(
            "antiserum.import_serum_presets {}",
            pack_dir.display()
        ))
        .unwrap()
        .message;
    assert!(
        first.starts_with("converted 2 Serum presets into user presets; skipped 2 files"),
        "{first}"
    );
    assert!(presets.join("Zed Lead.json").is_file());
    assert!(presets.join("Glass Reese (2).json").is_file());
    // Again: nothing is replaced.
    run(
        &mut s,
        &format!("antiserum.import_serum_presets {}", pack_dir.display()),
    );
    assert!(presets.join("Zed Lead (2).json").is_file());
    assert_eq!(s.undo_depth(), depth, "writing files is not an undo step");
    run(&mut s, "channel.add_antiserum Zed Lead (2)");
    fails(
        &mut s,
        &format!(
            "antiserum.import_serum_presets {}",
            base.join("absent").display()
        ),
    );
    musiccraft_antiserum_presets::user::set_folder(None);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn antiserum_presets_list_and_patch_files_round_trip() {
    let mut s = session();
    let listed = s.run_line("antiserum.presets pad").unwrap();
    assert_eq!(listed.message.lines().count(), 29);
    assert!(s.run_line("antiserum.presets nope").is_err());
    run(&mut s, "channel.add_antiserum Choir Pad");
    run(&mut s, "antiserum.set_step 1 2 off 3 0.5 0.5");
    let dir = std::env::temp_dir().join(format!("mc-patch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("choir.json");
    let path = path.to_string_lossy();
    run(&mut s, &format!("antiserum.save_patch 1 {path}"));
    let saved = s.project().channels[0].instrument.patch().cloned().unwrap();
    run(&mut s, "antiserum.load_preset 1 Deep Sub");
    run(&mut s, &format!("antiserum.open_patch 1 {path}"));
    assert_eq!(s.project().channels[0].instrument.patch(), Some(&saved));
    run(&mut s, "edit.undo");
    assert_eq!(
        s.project().channels[0]
            .instrument
            .patch()
            .map(|p| p.name.as_str()),
        Some("Deep Sub")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn plugin_commands_record_references_params_and_state() {
    let mut s = session();
    run(&mut s, "channel.add_plugin com.example.keys");
    run(&mut s, "channel.set_plugin_param 1 7 0.25");
    run(&mut s, "channel.clone 1");
    let p = s.project();
    let a = p.channels[0].instrument.plugin().unwrap();
    let b = p.channels[1].instrument.plugin().unwrap();
    assert_eq!(a.name, "keys");
    assert_eq!(a.params.get(&7), Some(&0.25));
    assert_ne!(a.instance, b.instance, "a clone runs its own instance");
    assert_eq!(b.params, a.params);

    // State is stored canonically and is not an undo step.
    let depth = s.undo_depth();
    run(&mut s, "channel.set_plugin_state 1 aGVsbG8");
    assert_eq!(
        s.project().channels[0].instrument.plugin().unwrap().state,
        "aGVsbG8="
    );
    assert_eq!(s.undo_depth(), depth);
    fails(&mut s, "channel.set_plugin_state 1 !!!");

    run(&mut s, "channel.add_synth");
    fails(&mut s, "channel.set_plugin_param 3 1 0.5");
    fails(&mut s, "channel.set_plugin_param 1 x 0.5");
    fails(&mut s, "channel.set_plugin_param 1 1 nan");
    fails(&mut s, "channel.add_plugin \"two words\"");
    run(&mut s, "mixer.add_effect 0 reverb");
    fails(&mut s, "mixer.set_plugin_param 0 1 1 0.5");
    fails(&mut s, "mixer.add_effect 0 plugin");

    // The project file keeps plugin references.
    let json = s.project().to_json().unwrap();
    let back = Project::from_json(&json).unwrap();
    assert_eq!(&back, s.project());
}

fn assert_round_trip(script: &[&str]) {
    let mut s = session();
    let mut snapshots = vec![s.project().clone()];
    for line in script {
        run(&mut s, line);
        snapshots.push(s.project().clone());
    }
    assert_eq!(s.undo_depth(), script.len(), "every line is one undo step");
    for (i, line) in script.iter().enumerate().rev() {
        run(&mut s, "edit.undo");
        assert_eq!(s.project(), &snapshots[i], "undo of `{line}` was not exact");
    }
    for (i, line) in script.iter().enumerate() {
        run(&mut s, "edit.redo");
        assert_eq!(
            s.project(),
            &snapshots[i + 1],
            "redo of `{line}` was not exact"
        );
    }
    // Inverses recorded on the first pass must still apply after a redo.
    for (i, line) in script.iter().enumerate().rev() {
        run(&mut s, "edit.undo");
        assert_eq!(
            s.project(),
            &snapshots[i],
            "second undo of `{line}` was not exact"
        );
    }
}

#[test]
fn steps_are_sixteenth_notes_at_the_step_key() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:hat");
    run(&mut s, "step.toggle 1 3");
    let notes = &s.project().current_pattern().notes;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].start, 3 * TICKS_PER_STEP);
    assert_eq!(notes[0].length, TICKS_PER_STEP);
    assert_eq!(notes[0].key, 60);
    run(&mut s, "step.toggle 1 3");
    assert!(s.project().current_pattern().notes.is_empty());
}

#[test]
fn note_times_accept_steps_fractions_and_ticks() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 0.5 1 60");
    run(&mut s, "note.add 1 7t 5t 61");
    let notes = &s.project().current_pattern().notes;
    assert_eq!((notes[0].start, notes[0].length), (7, 5));
    assert_eq!((notes[1].start, notes[1].length), (12, TICKS_PER_STEP));
}

#[test]
fn undo_after_switching_patterns_edits_the_original_pattern() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, "step.toggle 1 0");
    run(&mut s, "pattern.add");
    s.run_line("pattern.select 2").unwrap();
    // Undo pattern.add, then the step toggle: pattern 1 must lose its note.
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    assert!(s.project().patterns[0].notes.is_empty());
}

#[test]
fn pan_law_is_undoable() {
    use musiccraft_project::PanLaw;
    let mut s = session();
    assert_eq!(s.project().pan_law, PanLaw::Circular);
    run(&mut s, "project.set_pan_law triangular");
    assert_eq!(s.project().pan_law, PanLaw::Triangular);
    run(&mut s, "project.set_pan_law triangular");
    assert_eq!(s.undo_depth(), 1, "the same law records nothing");
    fails(&mut s, "project.set_pan_law balance");
    fails(&mut s, "project.set_pan_law");
    run(&mut s, "edit.undo");
    assert_eq!(s.project().pan_law, PanLaw::Circular);
}

#[test]
fn transport_mode_commands() {
    let mut s = session();
    run(&mut s, "transport.set_mode song");
    assert_eq!(s.project().play_mode, PlayMode::Song);
    run(&mut s, "transport.toggle_mode");
    assert_eq!(s.project().play_mode, PlayMode::Pattern);
    fails(&mut s, "transport.set_mode album");
    assert_eq!(s.undo_depth(), 0);
}

#[test]
fn pause_holds_and_play_pause_toggles() {
    use musiccraft_project::Transport;
    let mut s = session();
    run(&mut s, "transport.pause");
    assert_eq!(
        s.project().transport(),
        Transport::Stopped,
        "nothing to pause"
    );
    run(&mut s, "transport.play_pause");
    assert_eq!(s.project().transport(), Transport::Playing);
    run(&mut s, "transport.play_pause");
    assert_eq!(s.project().transport(), Transport::Paused);
    run(&mut s, "transport.toggle");
    assert_eq!(s.project().transport(), Transport::Playing, "Space resumes");
    run(&mut s, "transport.pause");
    run(&mut s, "transport.stop");
    assert_eq!(s.project().transport(), Transport::Stopped);
    assert_eq!(s.undo_depth(), 0);
}

#[test]
fn jump_to_start_and_panic_are_requests_outside_history() {
    use musiccraft_project::Transport;
    let mut s = session();
    let before = s.project().clone();
    run(&mut s, "transport.play");
    run(&mut s, "transport.jump_to_start");
    run(&mut s, "transport.panic");
    run(&mut s, "transport.panic");
    let requests = s.project().requests();
    assert_eq!((requests.jumps_to_start, requests.panics), (1, 2));
    assert_eq!(
        s.project().transport(),
        Transport::Playing,
        "playback goes on"
    );
    assert_eq!(s.undo_depth(), 0);
    run(&mut s, "transport.stop");
    assert_eq!(*s.project(), before, "requests are not project content");
    fails(&mut s, "transport.panic now");
}

#[test]
fn next_empty_selects_an_empty_pattern_or_adds_one() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 0 1 C5");
    for line in ["pattern.add", "pattern.add", "note.add 1 0 1 C5 @3"] {
        run(&mut s, line);
    }
    s.run_line("pattern.select 1").unwrap();
    let depth = s.undo_depth();
    // Pattern 2 is empty: select it, no history.
    run(&mut s, "pattern.next_empty");
    assert_eq!(s.project().current_pattern, 1);
    assert_eq!(s.undo_depth(), depth);
    // Nothing empty after pattern 2 (3 has a note): add pattern 4.
    run(&mut s, "pattern.next_empty");
    assert_eq!(s.project().patterns.len(), 4);
    assert_eq!(s.project().current_pattern, 3);
    assert_eq!(s.undo_depth(), depth + 1);
    run(&mut s, "edit.undo");
    assert_eq!(s.project().patterns.len(), 3);
    assert_eq!(s.project().current_pattern, 1);
    fails(&mut s, "pattern.next_empty 2");
}

#[test]
fn first_empty_looks_from_the_first_pattern() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 0 1 C5");
    run(&mut s, "pattern.add");
    run(&mut s, "pattern.add");
    run(&mut s, "note.add 1 0 1 C5 @3");
    // From pattern 3, the empty pattern 2 lies behind: next_empty would add.
    s.run_line("pattern.select 3").unwrap();
    let depth = s.undo_depth();
    run(&mut s, "pattern.first_empty");
    assert_eq!(s.project().current_pattern, 1);
    assert_eq!(s.undo_depth(), depth);
    run(&mut s, "note.add 1 0 1 C5 @2");
    run(&mut s, "pattern.first_empty");
    assert_eq!(s.project().patterns.len(), 4);
    assert_eq!(s.project().current_pattern, 3);
    fails(&mut s, "pattern.first_empty 1");
}

#[test]
fn recording_state_and_options_stay_out_of_history() {
    use musiccraft_project::RecordMode;
    let mut s = session();
    let before = s.project().clone();
    for line in ["record.toggle", "record.toggle_step_edit"] {
        run(&mut s, line);
    }
    assert!(s.project().is_armed() && s.project().step_edit());
    assert_eq!(
        *s.project(),
        before,
        "armed and step edit are session state"
    );
    run(&mut s, "record.set off");
    assert!(!s.project().is_armed());
    for line in [
        "record.toggle_count_in",
        "record.toggle_wait",
        "record.toggle_loop",
        "record.set_mode replace",
    ] {
        run(&mut s, line);
    }
    let options = s.project().record;
    assert!(options.count_in && options.wait_for_input && !options.loop_record);
    assert_eq!(options.mode, RecordMode::Replace);
    assert_eq!(s.undo_depth(), 0);
    fails(&mut s, "record.set_mode overdub");
    fails(&mut s, "record.set maybe");
    // Options are saved with the project.
    let json = s.project().to_json().unwrap();
    let loaded = musiccraft_project::Project::from_json(&json).unwrap();
    assert_eq!(loaded.record, options);
    assert!(!loaded.is_armed());
}

fn clip_spans(s: &Session) -> Vec<(u32, u32, u32, u32)> {
    s.project()
        .playlist
        .clips
        .iter()
        .map(|c| (c.track, c.start, c.length, c.offset))
        .collect()
}

#[test]
fn slicing_a_clip_keeps_playing_where_it_left_off() {
    let mut s = session();
    run(&mut s, "playlist.add_clip 1 1 0 4");
    run(&mut s, "playlist.slice_clip 1 1.5");
    assert_eq!(clip_spans(&s), [(0, 0, 576, 0), (0, 576, 960, 576)]);
    fails(&mut s, "playlist.slice_clip 1 0");
    // Undoing a removal keeps the offset.
    run(&mut s, "playlist.remove_clip 1 2");
    run(&mut s, "edit.undo");
    assert_eq!(clip_spans(&s)[1], (0, 576, 960, 576));
    run(&mut s, "edit.undo");
    assert_eq!(clip_spans(&s), [(0, 0, 1536, 0)]);
}

#[test]
fn make_unique_copies_the_source_for_one_clip() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 0 1 C5");
    run(&mut s, "playlist.add_clip 1 1 0");
    run(&mut s, "playlist.add_clip 1 1 1");
    run(&mut s, "playlist.make_unique 1 1");
    let project = s.project();
    assert_eq!(project.patterns.len(), 2);
    assert_eq!(project.patterns[1].name, "Pattern 1 #2");
    assert_eq!(project.patterns[1].notes, project.patterns[0].notes);
    let sources: Vec<_> = project.playlist.clips.iter().map(|c| c.pattern()).collect();
    assert_eq!(
        sources,
        [Some(project.patterns[0].id), Some(project.patterns[1].id)]
    );
    run(&mut s, "edit.undo");
    assert_eq!(s.project().patterns.len(), 1);
    assert!(
        s.project()
            .playlist
            .clips
            .iter()
            .all(|c| c.pattern() == Some(s.project().patterns[0].id))
    );
}

#[test]
fn merging_pattern_clips_makes_one_pattern() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 0 1 C5");
    run(&mut s, "pattern.add");
    run(&mut s, "note.add 1 2 1 E5 @2");
    run(&mut s, "playlist.add_clip 1 1 0");
    run(&mut s, "playlist.add_clip 2 2 1");
    run(&mut s, "playlist.merge_clips 1 0 2 1");
    let project = s.project();
    assert_eq!(project.patterns.len(), 3);
    let merged = &project.patterns[2];
    let notes: Vec<(u32, u8)> = merged.notes.iter().map(|n| (n.start, n.key)).collect();
    assert_eq!(notes, [(0, 60), (384 + 48, 64)]);
    assert_eq!(clip_spans(&s), [(0, 0, 768, 0)]);
    fails(&mut s, "playlist.merge_clips 1");
    run(&mut s, "edit.undo");
    assert_eq!(s.project().patterns.len(), 2);
    assert_eq!(clip_spans(&s).len(), 2);
}

#[test]
fn audio_clips_reverse_and_fade_through_their_channel() {
    use musiccraft_project::params::inst;
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, "playlist.add_audio_clip 1 1 0 1");
    run(&mut s, "playlist.reverse_clip 1 0");
    run(&mut s, "playlist.set_clip_fade 1 0 out 0.5");
    let params = &s.project().channels[0].params;
    assert!((params.get(inst::REVERSE) - 1.0).abs() < f32::EPSILON);
    assert!((params.get(inst::FADE_OUT) - 0.5).abs() < f32::EPSILON);
    fails(&mut s, "playlist.set_clip_fade 1 0 sideways 1");
    fails(&mut s, "playlist.set_clip_fade 1 0 in 11");
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    assert!(s.project().channels[0].params.get(inst::REVERSE) < 0.5);
    run(&mut s, "playlist.add_clip 1 2 0");
    fails(&mut s, "playlist.reverse_clip 2 0");
}

#[test]
fn inserting_and_deleting_time_moves_clips_and_markers() {
    let mut s = session();
    run(&mut s, "playlist.add_clip 1 1 0 2");
    run(&mut s, "playlist.add_clip 1 2 3 1");
    run(&mut s, "playlist.add_marker 3 Drop");
    let before = s.project().clone();
    run(&mut s, "playlist.insert_space 1 2");
    // The first clip is cut at bar 1; its rest and everything later move 2 bars.
    assert_eq!(
        clip_spans(&s),
        [(0, 0, 384, 0), (0, 1152, 384, 384), (1, 1920, 384, 0)]
    );
    assert_eq!(s.project().playlist.markers[0].tick, 5 * 384);
    run(&mut s, "playlist.delete_space 1 2");
    assert_eq!(
        clip_spans(&s),
        [(0, 0, 384, 0), (0, 384, 384, 384), (1, 1152, 384, 0)]
    );
    assert_eq!(s.project().playlist.markers[0].tick, 3 * 384);
    // Deleting across a marker removes it; a clip spanning the stretch is split.
    run(&mut s, "playlist.delete_space 2.5 1");
    assert!(s.project().playlist.markers.is_empty());
    for _ in 0..3 {
        run(&mut s, "edit.undo");
    }
    assert_eq!(*s.project(), before);
}

#[test]
fn route_free_takes_an_unused_insert_or_adds_one() {
    let mut s = session();
    run(&mut s, "channel.add_synth Bass");
    run(&mut s, "channel.add_synth Lead");
    run(&mut s, "channel.route 2 1");
    run(&mut s, "mixer.add_effect 2 reverb");
    run(&mut s, "channel.route_free 1");
    // Insert 1 feeds Lead, insert 2 has an effect: insert 3 is free.
    assert_eq!(s.project().channels[0].mixer_track, 3);
    assert_eq!(s.project().mixer[3].name, "Bass");
    run(&mut s, "edit.undo");
    assert_eq!(s.project().channels[0].mixer_track, 0);
    assert_eq!(s.project().mixer[3].name, "Insert 3");
    // With every insert in use a new one is added.
    let tracks = s.project().mixer.len();
    for t in 3..tracks {
        run(&mut s, &format!("mixer.add_effect {t} reverb"));
    }
    run(&mut s, "channel.route_free 1");
    assert_eq!(s.project().mixer.len(), tracks + 1);
    assert_eq!(s.project().channels[0].mixer_track, tracks);
    run(&mut s, "edit.undo");
    assert_eq!(s.project().mixer.len(), tracks);
}

/// (start, length, key, velocity) of channel 1's notes in pattern 1.
fn note_rows(s: &Session) -> Vec<(u32, u32, u8, u8)> {
    s.project().patterns[0]
        .notes
        .iter()
        .map(|n| (n.start, n.length, n.key, n.velocity))
        .collect()
}

fn synth_with(notes: &[&str]) -> Session {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    for line in notes {
        run(&mut s, line);
    }
    s
}

#[test]
fn limit_folds_notes_into_the_range_by_octaves() {
    let mut s = synth_with(&[
        "note.add 1 0 1 C3",
        "note.add 1 1 1 G7",
        "note.add 1 2 1 E5",
    ]);
    run(&mut s, "note.limit 1 C5 B5");
    let keys: Vec<u8> = note_rows(&s).iter().map(|n| n.2).collect();
    assert_eq!(keys, [60, 67, 64]);
    fails(&mut s, "note.limit 1 C6 C5");
}

#[test]
fn lfo_and_scale_levels_shape_velocities() {
    let mut s = synth_with(&[
        "note.add 1 0 1 C5 100",
        "note.add 1 4 1 C5 100",
        "note.add 1 8 1 C5 100",
    ]);
    // A square LFO over 8 steps at full depth: high, low, high.
    run(&mut s, "note.lfo 1 velocity 8 100 square");
    let vels: Vec<u8> = note_rows(&s).iter().map(|n| n.3).collect();
    assert_eq!(vels, [127, 1, 127]);
    // Halving around the centre (64).
    run(&mut s, "note.scale_levels 1 velocity 50");
    let vels: Vec<u8> = note_rows(&s).iter().map(|n| n.3).collect();
    assert_eq!(vels, [96, 33, 96]);
    fails(&mut s, "note.lfo 1 volume 8 100");
    run(&mut s, "note.lfo 1 pan 16 50");
    assert_eq!(
        s.project().patterns[0].notes[1].pan,
        50,
        "a quarter cycle in"
    );
}

#[test]
fn flam_adds_quieter_grace_notes_before() {
    let mut s = synth_with(&["note.add 1 4 1 C5 100", "note.add 1 0 1 E5 100"]);
    run(&mut s, "note.flam 1 6t 2 50");
    let rows = note_rows(&s);
    assert!(rows.contains(&(90, 6, 60, 50)) && rows.contains(&(84, 6, 60, 25)));
    assert_eq!(rows.len(), 4, "the note at 0 has no room for grace notes");
}

#[test]
fn randomize_is_repeatable_and_stays_in_range_and_scale() {
    let mut s = synth_with(&[]);
    run(&mut s, "note.randomize 1 C5 C6 50 7 C major");
    let first = note_rows(&s);
    assert!(!first.is_empty() && first.len() < 16);
    for (_, _, key, _) in &first {
        assert!((60..=72).contains(key));
        assert!([0, 2, 4, 5, 7, 9, 11].contains(&(key % 12)), "{key}");
    }
    run(&mut s, "note.randomize 1 C5 C6 50 7 C major");
    assert_eq!(note_rows(&s), first, "same seed, same notes");
    run(&mut s, "note.randomize 1 C5 C6 100 8");
    assert_eq!(note_rows(&s).len(), 16);
}

#[test]
fn chords_lay_down_a_progression() {
    let mut s = synth_with(&[]);
    run(&mut s, "note.chords 1 C4 major 1-5-6-4");
    let rows = note_rows(&s);
    let at = |bar: u32| -> Vec<u8> {
        rows.iter()
            .filter(|n| n.0 == bar * 384)
            .map(|n| n.2)
            .collect()
    };
    assert_eq!(at(0), [48, 52, 55], "C");
    assert_eq!(at(1), [55, 59, 62], "G");
    assert_eq!(at(2), [57, 60, 64], "Am");
    assert_eq!(at(3), [53, 57, 60], "F");
    fails(&mut s, "note.chords 1 C4 major 1-9");
}

#[test]
fn articulate_and_keep_every() {
    let mut s = synth_with(&[
        "note.add 1 0 1 C5 90",
        "note.add 1 2 1 D5 90",
        "note.add 1 3 1 E5 90",
        "note.add 1 4 4 F5 90",
    ]);
    run(&mut s, "note.articulate 1 50 10");
    let rows = note_rows(&s);
    assert_eq!(
        rows[0],
        (0, 24, 60, 100),
        "half the gap; accented on the beat"
    );
    assert_eq!(rows[1], (48, 12, 62, 90));
    run(&mut s, "note.keep_every 1 2");
    let keys: Vec<u8> = note_rows(&s).iter().map(|n| n.2).collect();
    assert_eq!(keys, [60, 64]);
    fails(&mut s, "note.keep_every 1 0");
}

#[test]
fn master_pitch_and_a4_tuning() {
    let mut s = session();
    run(&mut s, "project.set_a4 432");
    assert!(
        (s.project().master_pitch - -31.77).abs() < 0.01,
        "{}",
        s.project().master_pitch
    );
    run(&mut s, "project.set_master_pitch 1200");
    fails(&mut s, "project.set_master_pitch 1300");
    fails(&mut s, "project.set_a4 500");
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    assert!(s.project().master_pitch.abs() < f32::EPSILON);
}

#[test]
fn channel_groups_are_saved_and_undoable() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.set_group 1 Drums");
    assert_eq!(s.project().channels[0].group, "Drums");
    let json = s.project().to_json().unwrap();
    assert!(json.contains("\"group\": \"Drums\""));
    run(&mut s, "channel.set_group 1");
    assert!(s.project().channels[0].group.is_empty());
    run(&mut s, "edit.undo");
    assert_eq!(s.project().channels[0].group, "Drums");
}

#[test]
fn channel_colours_are_saved_undoable_and_follow_the_channel() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.add_synth");
    let auto = (
        s.project().channels[0].color_index(),
        s.project().channels[1].color_index(),
    );
    assert_ne!(auto.0, auto.1);
    // Reordering keeps each channel's automatic colour.
    run(&mut s, "channel.move 1 2");
    assert_eq!(s.project().channels[1].color_index(), auto.0);
    run(&mut s, "channel.set_color 1 9");
    assert_eq!(s.project().channels[0].color_index(), 8);
    let json = s.project().to_json().unwrap();
    assert!(json.contains("\"color\": 8"));
    run(&mut s, "edit.undo");
    assert_eq!(s.project().channels[0].color, None);
    fails(&mut s, "channel.set_color 1 17");
}

#[test]
fn solo_is_its_own_state_and_leaves_mutes_alone() {
    let mut s = session();
    for line in [
        "channel.add_synth",
        "channel.add_synth",
        "channel.add_synth",
        "channel.set_mute 3 on",
    ] {
        run(&mut s, line);
    }
    run(&mut s, "channel.solo 2");
    let p = s.project();
    assert_eq!(
        p.channels
            .iter()
            .map(|c| (c.mute, c.solo))
            .collect::<Vec<_>>(),
        [(false, false), (false, true), (true, false)]
    );
    assert_eq!(
        (0..3).map(|i| p.channel_plays(i)).collect::<Vec<_>>(),
        [false, true, false]
    );
    assert!(p.to_json().unwrap().contains("\"solo\": true"));
    // Soloing the only soloed channel again clears solo; mutes stay.
    run(&mut s, "channel.solo 2");
    let p = s.project();
    assert!(p.channels.iter().all(|c| !c.solo));
    assert_eq!(
        (0..3).map(|i| p.channel_plays(i)).collect::<Vec<_>>(),
        [true, true, false]
    );
    run(&mut s, "edit.undo");
    assert!(s.project().channels[1].solo);
}

#[test]
fn a_solo_locked_channel_keeps_playing_through_another_solo() {
    let mut s = session();
    for line in [
        "channel.add_synth",
        "channel.add_synth",
        "channel.add_synth",
        "channel.set_solo_lock 3 on",
        "channel.solo 1",
    ] {
        run(&mut s, line);
    }
    let plays = |s: &Session| {
        (0..3)
            .map(|i| s.project().channel_plays(i))
            .collect::<Vec<_>>()
    };
    assert_eq!(plays(&s), [true, false, true]);
    assert!(
        s.project()
            .to_json()
            .unwrap()
            .contains("\"solo_lock\": true")
    );
    // Its own mute still silences it.
    run(&mut s, "channel.set_mute 3 on");
    assert_eq!(plays(&s), [true, false, false]);
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    assert!(!s.project().channels[2].solo_lock);
    assert_eq!(plays(&s), [true, true, true]);
    fails(&mut s, "channel.set_solo_lock 3 maybe");
}

#[test]
fn pattern_navigation_clamps() {
    let mut s = session();
    run(&mut s, "pattern.add");
    run(&mut s, "pattern.add");
    run(&mut s, "pattern.next");
    assert_eq!(s.project().current_pattern, 2);
    run(&mut s, "pattern.select 1");
    run(&mut s, "pattern.prev");
    assert_eq!(s.project().current_pattern, 0);
}

#[test]
fn removing_a_pattern_removes_its_clips() {
    let mut s = session();
    run(&mut s, "pattern.add");
    run(&mut s, "playlist.add_clip 2 1 0");
    run(&mut s, "playlist.add_clip 1 2 0");
    run(&mut s, "pattern.remove 2");
    assert_eq!(s.project().playlist.clips.len(), 1);
    fails(&mut s, "pattern.remove 1");
}

#[test]
fn removing_a_mixer_track_reroutes_channels() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.route 1 3");
    run(&mut s, "channel.route 2 5");
    run(&mut s, "mixer.remove_track 3");
    assert_eq!(s.project().channels[0].mixer_track, 0);
    assert_eq!(s.project().channels[1].mixer_track, 4);
    fails(&mut s, "mixer.remove_track 0");
}

#[test]
fn playlist_rejects_overlaps() {
    let mut s = session();
    run(&mut s, "playlist.add_clip 1 1 0 2");
    fails(&mut s, "playlist.add_clip 1 1 1");
    run(&mut s, "playlist.add_clip 1 1 2");
    fails(&mut s, "playlist.move_clip 1 2 1 1");
    fails(&mut s, "playlist.resize_clip 1 0 3");
    assert_eq!(s.project().playlist.length_ticks(), 3 * TICKS_PER_BAR);
}

#[test]
fn trimming_a_clip_moves_its_start_and_keeps_its_end() {
    let mut s = session();
    run(&mut s, "playlist.add_clip 1 1 0 1");
    run(&mut s, "playlist.add_clip 1 1 2 2");
    let bar = TICKS_PER_BAR;
    run(&mut s, "playlist.trim_clip 1 2 2.5");
    let c = s.project().playlist.clips[1];
    assert_eq!(
        (c.start, c.length, c.offset),
        (5 * bar / 2, 3 * bar / 2, bar / 2)
    );
    // Back towards the source's start, but not before it.
    run(&mut s, "playlist.trim_clip 1 2.5 2.25");
    let c = s.project().playlist.clips[1];
    assert_eq!((c.start, c.offset), (9 * bar / 4, bar / 4));
    fails(&mut s, "playlist.trim_clip 1 2.25 1.5");
    // Not past its end, and not over its neighbour.
    fails(&mut s, "playlist.trim_clip 1 2.25 4");
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    let c = s.project().playlist.clips[1];
    assert_eq!((c.start, c.length, c.offset), (2 * bar, 2 * bar, 0));
    fails(&mut s, "playlist.trim_clip 1 2 0.5");
}

#[test]
fn errors_leave_the_project_unchanged() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    for line in [
        "channel.add_sampler builtin:cowbell",
        "channel.add_sampler /no/such/file.wav",
        "channel.set_volume 1 2",
        "channel.set_volume 2 0.5",
        "channel.set_volume 0 0.5",
        "channel.set_pan 1 -3",
        "channel.set_param 1 wobble 1",
        "channel.set_param 1 cutoff 5",
        "channel.set_sample 1 builtin:kick",
        "channel.swap_sound 1 9",
        "channel.route 1 99",
        "note.add 1 0 0 60",
        "note.add 1 0 1 C99",
        "note.add 1 0 1 60 0",
        "note.add 1 0 1 60 100 101",
        "note.add 1 -1 1 60",
        "note.add 1 0 1 60 @7",
        "note.remove 1 0 60",
        "note.transpose 1 200",
        "note.add_chord 1 0 1 C5 jazzy",
        "note.add_chord 1 0 1 G10 major",
        "note.set_all 1 [[0,0,60,100]]",
        "step.toggle 1 99999",
        "pattern.set_length 1 0",
        "project.set_swing 101",
        "project.set_swing -1",
        "pattern.select 4",
        "mixer.add_effect 1 theremin",
        "mixer.set_send 2 1 0.5",
        "mixer.set_send 0 1 0.5",
        "mixer.set_send 1 2 3",
        "mixer.route_to_master 0 off",
        "mixer.remove_effect 1 1",
        "mixer.set_effect_param 0 1 size 1",
        "playlist.add_clip 5 1 0",
        "playlist.add_clip 1 65 0",
        "transport.set_loop 4 2",
        "playlist.remove_marker 9",
        "automation.create channel:9:volume",
        "automation.create channel:1:wobble",
        "automation.create mixer:1:5:mix",
        "automation.create banana",
        "automation.add_point 1 0 0.5",
        "playlist.remove_clip 1 0",
        "project.set_tempo fast",
        "project.load /no/such/project.mcproj",
    ] {
        fails(&mut s, line);
    }
}

#[test]
fn transpose_and_quantize() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 1t 1 60");
    run(&mut s, "note.add 1 13t 1 62");
    run(&mut s, "note.transpose 1 -12");
    run(&mut s, "note.quantize 1 1");
    let notes: Vec<_> = s
        .project()
        .current_pattern()
        .notes
        .iter()
        .map(|n| (n.start, n.key))
        .collect();
    assert_eq!(notes, [(0, 48), (24, 50)]);
}

#[test]
fn set_param_rounds_integer_params() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.set_param 1 wave1 2.6");
    assert!((s.project().channels[0].params.get(inst::WAVE1) - 3.0).abs() < f32::EPSILON);
}

#[test]
fn save_and_load_round_trip() {
    let dir = std::env::temp_dir().join(format!("musiccraft-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.mcproj");
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, "step.toggle 1 0");
    run(&mut s, "project.set_tempo 101");
    let line = format!("project.save \"{}\"", path.display());
    run(&mut s, &line);
    let saved = s.project().clone();
    run(&mut s, "project.new");
    assert!(s.project().channels.is_empty());
    run(&mut s, &format!("project.load \"{}\"", path.display()));
    assert_eq!(s.project().channels, saved.channels);
    assert_eq!(s.project().patterns, saved.patterns);
    assert_eq!(s.undo_depth(), 0);
    run(&mut s, "project.rename Again");
    run(&mut s, "project.save");
    assert!(std::fs::read_to_string(&path).unwrap().contains("Again"));
    std::fs::remove_dir_all(&dir).unwrap();
}

fn flp_asset(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../musiccraft-flp/tests/assets")
        .join(name);
    format!("\"{}\"", path.display())
}

#[test]
fn import_flp_converts_pyflps_test_project() {
    use musiccraft_project::{ClipSource, Instrument};
    let mut s = session();
    run(&mut s, "project.set_tempo 101");
    let outcome = s
        .run_line(&format!(
            "project.import_flp {}",
            flp_asset("FL 20.8.4.flp")
        ))
        .expect("imports")
        .message;
    assert!(
        outcome.contains("replaced “BooBass” (now a synth)"),
        "{outcome}"
    );
    assert!(outcome.contains("automation clip"), "{outcome}");
    assert_eq!(s.undo_depth(), 0);
    let p = s.project();
    assert_eq!(p.name(), "PyFLP Test FLP");
    // The pattern with signature markers takes its first one.
    let markers = p.patterns.iter().find(|p| p.name == "Timemarkers").unwrap();
    assert_eq!(markers.time_signature.to_string(), "3/2");
    assert!(p.time_signature.is_common());
    assert!((p.tempo_bpm() - 69.42).abs() < 1e-9);
    assert!(p.path().is_none());
    // 19 channels, less the automation clip.
    assert_eq!(p.channels.len(), 18);
    let by_name = |name: &str| p.channels.iter().find(|c| c.name == name).expect(name);
    assert_eq!(by_name("Zero Volume").volume, 0.0);
    assert_eq!(by_name("Full Volume").volume, 1.0);
    assert_eq!(by_name("100% L").pan, -1.0);
    assert_eq!(by_name("100% R").pan, 1.0);
    assert!(by_name("Disabled").mute);
    assert!(matches!(
        &by_name("22in Kick").instrument,
        Instrument::Sampler { sample } if sample.ends_with("22in Kick.wav")
    ));
    assert!(matches!(
        by_name("Layer").instrument,
        Instrument::Layer { .. }
    ));
    // Channel routing and insert numbers agree.
    let routed = by_name("Instrument track").mixer_track;
    assert_eq!(p.mixer[routed].name, "Instrument track");
    // Inserts carry their settings.
    let insert = |name: &str| p.mixer.iter().find(|m| m.name == name).expect(name);
    assert_eq!(p.mixer[0].name, "Master");
    assert_eq!(insert("Zero Volume").volume, 0.0);
    assert_eq!(insert("100% L").pan, -1.0);
    assert_eq!(insert("100% mono").separation, -1.0);
    assert_eq!(insert("100% separated").separation, 1.0);
    assert!(insert("Swap LR").swap_sides);
    assert!(insert("Invert Polarity").phase_invert);
    assert!(insert("Disabled").mute);
    assert!(!insert("No routes").to_master);
    // The MIDI pattern's 44 notes (4 more belong to the automation clip's
    // channel, which is not imported) and the playlist.
    let midi = p.patterns.iter().find(|x| x.name == "MIDI").expect("MIDI");
    assert_eq!(midi.notes.len(), 44);
    // The first track is unnamed (as PyFLP reads it), then "Enabled",
    // then "Disabled".
    assert_eq!(p.playlist.track_name(1), "Enabled");
    assert_eq!(p.playlist.track_name(2), "Disabled");
    assert_eq!(
        p.playlist.muted_tracks.iter().copied().collect::<Vec<_>>(),
        [2]
    );
    assert_eq!(p.playlist.loop_range, Some((3840, 5376)));
    assert!(
        p.playlist
            .clips
            .iter()
            .any(|c| matches!(c.source, ClipSource::Audio(_)))
    );
}

#[test]
fn import_flp_scales_ticks_to_96_ppq() {
    // PyFLP's multi-channel pattern project.
    let mut s = session();
    run(
        &mut s,
        &format!(
            "project.import_flp {}",
            flp_asset("patterns/multi-channel.flp")
        ),
    );
    let p = s.project();
    assert!(!p.patterns.is_empty());
    for note in p.patterns.iter().flat_map(|x| &x.notes) {
        assert!(note.length >= 1);
        assert!(p.channel_position(note.channel).is_some());
    }
}

#[test]
fn import_flp_refuses_files_pyflp_refuses() {
    let mut s = session();
    for file in [
        "corrupted/invalid-header-magic.flp",
        "corrupted/invalid-ppq.flp",
        "corrupted/invalid-format.flp",
    ] {
        let err = fails(&mut s, &format!("project.import_flp {}", flp_asset(file)));
        assert!(err.to_string().contains("cannot import"), "{err}");
    }
    fails(&mut s, "project.import_flp /no/such/file.flp");
}

#[test]
fn recover_loads_a_backup_without_adopting_its_path() {
    let dir = std::env::temp_dir().join(format!("musiccraft-recover-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("recovery.mcproj");
    let mut s = session();
    run(&mut s, "project.rename Rescued");
    run(&mut s, "project.set_tempo 99");
    run(&mut s, &format!("project.save \"{}\"", path.display()));
    run(&mut s, "project.new");
    run(&mut s, &format!("project.recover \"{}\"", path.display()));
    assert_eq!(s.project().name(), "Rescued");
    assert!((s.project().tempo_bpm() - 99.0).abs() < 1e-9);
    assert!(s.project().path().is_none(), "saving must ask where");
    assert_eq!(s.undo_depth(), 0);
    fails(&mut s, "project.save");
    fails(&mut s, "project.recover /no/such/backup.mcproj");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn marker_actions_pause_and_song_loop() {
    use musiccraft_project::MarkerAction;
    let mut s = session();
    run(&mut s, "playlist.add_marker 3 Break");
    run(&mut s, "playlist.add_marker 5 Outro");
    run(&mut s, "playlist.set_marker_action 3 pause");
    // Playlist bars count from 0.
    assert_eq!(s.project().playlist.pause_ticks(), [3 * TICKS_PER_BAR]);
    assert_eq!(s.project().playlist.song_loop_tick(), None);
    // Only one marker is the song loop point.
    run(&mut s, "playlist.set_marker_action 3 song_loop");
    run(&mut s, "playlist.set_marker_action 5 song_loop");
    let actions: Vec<_> = s
        .project()
        .playlist
        .markers
        .iter()
        .map(|m| m.action)
        .collect();
    assert_eq!(actions, [MarkerAction::None, MarkerAction::SongLoop]);
    assert_eq!(
        s.project().playlist.song_loop_tick(),
        Some(5 * TICKS_PER_BAR)
    );
    // Undo restores the previous loop point in one step.
    run(&mut s, "edit.undo");
    assert_eq!(
        s.project().playlist.song_loop_tick(),
        Some(3 * TICKS_PER_BAR)
    );
    run(&mut s, "edit.redo");
    // Renaming keeps the action.
    run(&mut s, "playlist.add_marker 5 Coda");
    assert_eq!(s.project().playlist.markers[1].name, "Coda");
    assert_eq!(
        s.project().playlist.markers[1].action,
        MarkerAction::SongLoop
    );
    fails(&mut s, "playlist.set_marker_action 5 stop");
    fails(&mut s, "playlist.set_marker_action 9 pause");
    fails(
        &mut s,
        r#"playlist.set_markers "[{\"tick\":0,\"name\":\"a\"},{\"tick\":0,\"name\":\"b\"}]""#,
    );
}

#[test]
fn a_song_loop_marker_past_the_last_clip_extends_the_song() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, "step.toggle 1 0");
    run(&mut s, "playlist.add_clip 1 1 0");
    assert_eq!(s.project().song_length_ticks(), TICKS_PER_BAR);
    run(&mut s, "playlist.add_marker 4.5 Tail");
    assert_eq!(
        s.project().song_length_ticks(),
        TICKS_PER_BAR,
        "a plain marker"
    );
    run(&mut s, "playlist.set_marker_action 4.5 song_loop");
    assert_eq!(s.project().song_length_ticks(), 5 * TICKS_PER_BAR);
}

#[test]
fn plain_markers_store_no_action_in_the_project_file() {
    let mut s = session();
    run(&mut s, "playlist.add_marker 2 Verse");
    let json = s.project().to_json().unwrap();
    assert!(!json.contains("\"action\""), "{json}");
    run(&mut s, "playlist.set_marker_action 2 pause");
    let json = s.project().to_json().unwrap();
    assert!(json.contains("\"action\": \"pause\""), "{json}");
    let loaded = Project::from_json(&json).unwrap();
    assert_eq!(loaded.playlist.pause_ticks(), [2 * TICKS_PER_BAR]);
}

#[test]
fn save_without_a_path_needs_one() {
    let mut s = session();
    fails(&mut s, "project.save");
    fails(&mut s, "project.save_new_version");
}

#[test]
fn next_version_path_numbers_siblings() {
    use std::path::{Path, PathBuf};
    let next = |p: &str, taken: &[&str]| {
        super::project::next_version_path(Path::new(p), |c| taken.iter().any(|t| Path::new(t) == c))
    };
    assert_eq!(
        next("/s/song.mcproj", &[]),
        PathBuf::from("/s/song_2.mcproj")
    );
    assert_eq!(
        next("/s/song_2.mcproj", &[]),
        PathBuf::from("/s/song_3.mcproj")
    );
    assert_eq!(
        next("/s/my_song.mcproj", &[]),
        PathBuf::from("/s/my_song_2.mcproj")
    );
    assert_eq!(
        next("/s/song.mcproj", &["/s/song_2.mcproj", "/s/song_3.mcproj"]),
        PathBuf::from("/s/song_4.mcproj")
    );
}

#[test]
fn save_new_version_writes_a_numbered_copy_and_switches_to_it() {
    let dir = std::env::temp_dir().join(format!("musiccraft-version-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.mcproj");
    let mut s = session();
    run(&mut s, &format!("project.save \"{}\"", path.display()));
    run(&mut s, "project.rename Second");
    run(&mut s, "project.save_new_version");
    let second = dir.join("song_2.mcproj");
    assert!(std::fs::read_to_string(&second).unwrap().contains("Second"));
    assert!(!std::fs::read_to_string(&path).unwrap().contains("Second"));
    assert!(s.project().path().unwrap().ends_with("song_2.mcproj"));
    run(&mut s, "project.save_new_version");
    assert!(dir.join("song_3.mcproj").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn import_midi_converts_ticks_and_undoes() {
    use crate::midi_file::tests::{sample_track, smf};
    let path = std::env::temp_dir().join(format!("musiccraft-{}.mid", std::process::id()));
    std::fs::write(&path, smf(192, &sample_track())).unwrap();
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 15 1 C2");
    run(
        &mut s,
        &format!("note.import_midi 1 \"{}\"", path.display()),
    );
    let notes: Vec<_> = s
        .project()
        .current_pattern()
        .notes
        .iter()
        .map(|n| (n.start, n.length, n.key))
        .collect();
    // 192 file ticks per quarter → 96 project ticks per beat.
    assert_eq!(
        notes,
        [(0, 96, 60), (96, 48, 64), (96, 48, 67), (360, 24, 24)]
    );
    run(&mut s, "edit.undo");
    assert_eq!(s.project().current_pattern().notes.len(), 1);
    std::fs::write(&path, b"garbage").unwrap();
    fails(
        &mut s,
        &format!("note.import_midi 1 \"{}\"", path.display()),
    );
    std::fs::remove_file(path).unwrap();
}

#[test]
fn chords_add_stacked_notes() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add_chord 1 0 4 C5 maj7");
    let keys: Vec<u8> = s
        .project()
        .current_pattern()
        .notes
        .iter()
        .map(|n| n.key)
        .collect();
    assert_eq!(keys, [60, 64, 67, 71]);
    run(&mut s, "edit.undo");
    assert!(s.project().current_pattern().notes.is_empty());
}

#[test]
fn extended_chords_stack_their_tones() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    for (quality, expected) in [
        ("power", &[60, 67][..]),
        ("6/9", &[60, 64, 67, 69, 74]),
        ("min7b5", &[60, 63, 66, 70]),
        ("7#9", &[60, 64, 67, 70, 75]),
        ("min11", &[60, 63, 67, 70, 74, 77]),
        ("dom13", &[60, 64, 67, 70, 74, 81]),
    ] {
        run(&mut s, &format!("note.add_chord 1 0 4 C5 {quality}"));
        let mut keys: Vec<u8> = s
            .project()
            .current_pattern()
            .notes
            .iter()
            .map(|n| n.key)
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, expected, "{quality}");
        run(&mut s, "edit.undo");
    }
    fails(&mut s, "note.add_chord 1 0 4 G9 dom13");
}

#[test]
fn removing_a_track_fixes_sends_pointing_at_and_past_it() {
    let mut s = session();
    run(&mut s, "mixer.set_send 1 3 0.5");
    run(&mut s, "mixer.set_send 1 5 0.7");
    run(&mut s, "mixer.remove_track 3");
    let sends = &s.project().mixer[1].sends;
    assert_eq!(sends.len(), 1);
    assert_eq!((sends[0].target, sends[0].level), (4, 0.7));
    run(&mut s, "edit.undo");
    let targets: Vec<usize> = s.project().mixer[1]
        .sends
        .iter()
        .map(|x| x.target)
        .collect();
    assert_eq!(targets, [3, 5]);
}

#[test]
fn moving_a_track_carries_its_routing_sends_and_automation() {
    let mut s = session();
    for line in [
        "channel.add_synth",
        "channel.route 1 2",
        "mixer.rename 2 Bass",
        "mixer.set_send 1 2 0.5",
        "mixer.set_send 3 4 0.7",
        "mixer.add_effect 2 delay",
        "automation.create mixer:2:1:mix",
    ] {
        run(&mut s, line);
    }
    run(&mut s, "mixer.move_track 2 right");
    let p = s.project();
    assert_eq!(p.mixer[3].name, "Bass");
    assert_eq!(p.channels[0].mixer_track, 3);
    assert_eq!(p.mixer[1].sends[0].target, 3);
    assert_eq!(
        (p.mixer[2].sends[0].target, p.mixer[2].sends[0].level),
        (4, 0.7)
    );
    assert!(matches!(
        p.automations[0].target,
        musiccraft_project::AutomationTarget::EffectParam { track: 3, .. }
    ));
    run(&mut s, "edit.undo");
    let p = s.project();
    assert_eq!(p.mixer[2].name, "Bass");
    assert_eq!(p.channels[0].mixer_track, 2);
    assert_eq!(p.mixer[1].sends[0].target, 2);
    fails(&mut s, "mixer.move_track 1 left");
    fails(&mut s, "mixer.move_track 0 right");
    run(&mut s, "mixer.set_send 2 3 0.5");
    fails(&mut s, "mixer.move_track 2 right");
    let last = s.project().mixer.len() - 1;
    fails(&mut s, &format!("mixer.move_track {last} right"));
}

#[test]
fn automation_guards_mixer_edits() {
    let mut s = session();
    run(&mut s, "mixer.add_effect 2 delay");
    run(&mut s, "mixer.add_effect 2 reverb");
    run(&mut s, "automation.create mixer:2:2:mix");
    fails(&mut s, "mixer.remove_effect 2 2");
    fails(&mut s, "mixer.remove_track 2");
    run(&mut s, "mixer.remove_effect 2 1");
    let target = &s.project().automations[0].target;
    assert_eq!(
        *target,
        musiccraft_project::AutomationTarget::EffectParam {
            track: 2,
            slot: 0,
            param: "mix".into()
        },
        "the automation follows its effect to slot 1"
    );
    run(&mut s, "mixer.remove_track 1");
    assert!(matches!(
        s.project().automations[0].target,
        musiccraft_project::AutomationTarget::EffectParam { track: 1, .. }
    ));
}

#[test]
fn removing_a_channel_removes_its_audio_clips() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:crash");
    run(&mut s, "playlist.add_audio_clip 1 1 0 1");
    run(&mut s, "channel.remove 1");
    assert!(s.project().playlist.clips.is_empty());
    run(&mut s, "edit.undo");
    assert_eq!(s.project().playlist.clips.len(), 1);
    fails(&mut s, "playlist.add_audio_clip 1 1 0");
    run(&mut s, "channel.add_synth");
    fails(&mut s, "playlist.add_audio_clip 2 2 0 1");
}

#[test]
fn sidechain_sources_are_validated_and_renumbered() {
    let mut s = session();
    run(&mut s, "mixer.add_effect 3 compressor");
    fails(&mut s, "mixer.set_effect_param 3 1 sidechain 3");
    fails(&mut s, "mixer.set_effect_param 3 1 sidechain 5");
    fails(&mut s, "mixer.set_effect_param 3 1 sidechain 99");
    run(&mut s, "mixer.set_effect_param 3 1 sidechain 2");
    fails(&mut s, "mixer.remove_track 2");
    run(&mut s, "mixer.remove_track 1");
    let value = s.project().mixer[2].effects[0].values[5];
    assert!(
        (value - 1.0).abs() < f32::EPSILON,
        "source followed the renumbering: {value}"
    );
    run(&mut s, "edit.undo");
    assert!((s.project().mixer[3].effects[0].values[5] - 2.0).abs() < f32::EPSILON);
}

#[test]
fn vocoder_sidechain_and_impulse_files() {
    let mut s = session();
    run(&mut s, "mixer.add_effect 3 vocoder");
    fails(&mut s, "mixer.set_effect_param 3 1 sidechain 4");
    run(&mut s, "mixer.set_effect_param 3 1 sidechain 1");
    fails(&mut s, "mixer.remove_track 1");
    fails(&mut s, "mixer.set_effect_file 3 1 ir:hall");
    run(&mut s, "mixer.add_effect 2 convolution");
    fails(&mut s, "mixer.set_effect_file 2 1 ir:garage");
    fails(&mut s, "mixer.set_effect_file 2 1 /no/such/impulse.wav");
    run(&mut s, "mixer.set_effect_file 2 1 ir:plate");
    assert_eq!(
        s.project().mixer[2].effects[0].file.as_deref(),
        Some("ir:plate")
    );
    run(&mut s, "edit.undo");
    assert_eq!(s.project().mixer[2].effects[0].file, None);
}

const TOOLS_SCRIPT: &[&str] = &[
    "channel.add_synth",
    "note.add_chord 1 0 8 C4 maj7",
    "note.add 1 8 2 E4",
    "note.strum 1 6t",
    "note.humanize 1 3t 10 42",
    "note.legato 1",
    "note.chop 1 2",
    "note.glue 1",
    "note.flip 1 time",
    "note.flip 1 pitch",
    "note.to_scale 1 D dorian",
    "note.arpeggiate 1 1 updown",
    "channel.add_sampler builtin:hat",
    "step.fill 2 2",
    "step.fill 2 4 1",
    "step.shift 2 -3",
    "channel.add_drum snare",
    "channel.add_drum Boom",
    "channel.add_pluck Harp",
    "channel.set_param 3 drum_decay 0.3",
    "channel.set_param 5 pluck_damping 0.6",
    "channel.slice 2 8",
    "channel.add_fm",
    "channel.add_keys organ",
    "channel.add_additive Pad",
    "channel.add_granular builtin:crash",
    "channel.add_kit",
    "channel.set_pad 10 9 builtin:rim",
    "channel.set_pad 10 1 -",
    "channel.add_layer",
    "channel.set_layer 11 1,6",
    "channel.set_sample 9 builtin:tom",
    "channel.add_midi_out 10 Synth rig",
    "channel.set_midi_channel 12 3",
    "channel.add_midi_out",
];

#[test]
fn midi_out_channels() {
    use musiccraft_project::Instrument;
    let mut s = session();
    run(&mut s, "channel.add_midi_out");
    run(&mut s, "channel.add_midi_out 10");
    run(&mut s, "channel.add_midi_out 16 Rack synth");
    run(&mut s, "channel.add_midi_out Poly box");
    let p = s.project();
    let names: Vec<&str> = p.channels.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        ["MIDI Out 1", "MIDI Out 10", "Rack synth", "Poly box"]
    );
    assert_eq!(
        p.channels[2].instrument,
        Instrument::MidiOut { midi_channel: 16 }
    );
    fails(&mut s, "channel.add_midi_out 17");
    fails(&mut s, "channel.add_midi_out 0");
    fails(&mut s, "channel.set_midi_channel 1 0");
    run(&mut s, "channel.add_synth");
    fails(&mut s, "channel.set_midi_channel 5 2");
    run(&mut s, "channel.set_midi_channel 1 7");
    assert_eq!(
        s.project().channels[0].instrument,
        Instrument::MidiOut { midi_channel: 7 }
    );
    run(&mut s, "edit.undo");
    assert_eq!(
        s.project().channels[0].instrument,
        Instrument::MidiOut { midi_channel: 1 }
    );
    let json = s.project().to_json().unwrap();
    assert_eq!(&Project::from_json(&json).unwrap(), s.project());
}

#[test]
fn drum_and_pluck_channels() {
    let mut s = session();
    run(&mut s, "channel.add_drum hat");
    run(&mut s, "channel.add_pluck");
    let p = s.project();
    assert_eq!(p.channels[0].name, "Hat");
    assert_eq!(p.channels[0].params.get(inst::DRUM_TYPE), 2.0);
    assert_eq!(
        p.channels[1].instrument,
        musiccraft_project::Instrument::Pluck
    );
    let json = p.to_json().unwrap();
    assert_eq!(&Project::from_json(&json).unwrap(), s.project());
}

#[test]
fn note_tools_undo_and_redo_exactly() {
    assert_round_trip(TOOLS_SCRIPT);
}

fn notes_of(s: &Session, channel: usize) -> Vec<(u32, u32, u8, u8)> {
    let p = s.project();
    let id = p.channels[channel].id;
    let mut v: Vec<_> = p
        .current_pattern()
        .notes_of(id)
        .map(|n| (n.start, n.length, n.key, n.velocity))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn note_tools_produce_the_expected_notes() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add_chord 1 0 4 C4 major 100");
    run(&mut s, "note.strum 1 3t");
    assert_eq!(
        notes_of(&s, 0),
        [(0, 96, 48, 100), (3, 93, 52, 100), (6, 90, 55, 100)]
    );

    // Arpeggiate a chord into steps cycling up through its keys.
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add_chord 1 0 4 C4 major 100");
    run(&mut s, "note.arpeggiate 1 1");
    let keys: Vec<u8> = notes_of(&s, 0).iter().map(|n| n.2).collect();
    assert_eq!(keys, [48, 52, 55, 48]);

    // Legato, chop and glue.
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 0 1 C4");
    run(&mut s, "note.add 1 4 1 D4");
    run(&mut s, "note.legato 1");
    assert_eq!(notes_of(&s, 0), [(0, 96, 48, 100), (96, 24, 50, 100)]);
    run(&mut s, "note.chop 1 2");
    assert_eq!(notes_of(&s, 0).len(), 3);
    run(&mut s, "note.glue 1");
    assert_eq!(notes_of(&s, 0), [(0, 96, 48, 100), (96, 24, 50, 100)]);

    // Flip in time within the one-bar pattern, and in pitch.
    run(&mut s, "note.flip 1 time");
    assert_eq!(notes_of(&s, 0), [(264, 24, 50, 100), (288, 96, 48, 100)]);
    run(&mut s, "note.flip 1 pitch");
    let keys: Vec<u8> = notes_of(&s, 0).iter().map(|n| n.2).collect();
    assert_eq!(keys, [48, 50]);

    // Snap to C major: D# goes down to D.
    run(&mut s, "note.add 1 8 1 D#4");
    run(&mut s, "note.to_scale 1 C major");
    assert!(notes_of(&s, 0).iter().all(|n| n.2 != 51));
    fails(&mut s, "note.to_scale 1 C bebop");

    // Humanize is repeatable for a seed and stays in range.
    let humanized = |seed: &str| {
        let mut s = session();
        run(&mut s, "channel.add_synth");
        run(&mut s, "note.add 1 4 1 C4 120");
        run(&mut s, "note.add 1 8 1 C4 10");
        run(&mut s, &format!("note.humanize 1 6t 20 {seed}"));
        notes_of(&s, 0)
    };
    assert_eq!(humanized("5"), humanized("5"));
    assert!(humanized("5").iter().all(|n| (1..=127).contains(&n.3)));

    // Fill every 4 steps and rotate by one.
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, "step.fill 1 4");
    let starts: Vec<u32> = notes_of(&s, 0).iter().map(|n| n.0).collect();
    assert_eq!(starts, [0, 96, 192, 288]);
    run(&mut s, "step.shift 1 -1");
    let starts: Vec<u32> = notes_of(&s, 0).iter().map(|n| n.0).collect();
    assert_eq!(starts, [72, 168, 264, 360]);
    fails(&mut s, "step.fill 1 0");
}

#[test]
fn note_props_survive_edits_and_files() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "note.add 1 0 1 C4");
    run(&mut s, "note.set_prop 1 0 C4 fine 25");
    run(&mut s, "note.set_prop 1 0 C4 color 3");
    run(&mut s, "note.set_prop 1 0 C4 muted on");
    // A velocity change and its undo keep every property.
    run(&mut s, "note.set_velocity 1 0 C4 30");
    run(&mut s, "edit.undo");
    let n = s.project().current_pattern().notes[0];
    assert_eq!(
        (n.props.fine, n.props.color, n.props.muted, n.velocity),
        (25, 2, true, 100)
    );
    // Transposing (set_all inverse) keeps them through undo too.
    run(&mut s, "note.transpose 1 2");
    run(&mut s, "edit.undo");
    assert_eq!(s.project().current_pattern().notes[0], n);
    let json = s.project().to_json().unwrap();
    assert!(json.contains("\"fine\": 25") && !json.contains("mod_x"));
    assert_eq!(&Project::from_json(&json).unwrap(), s.project());
    fails(&mut s, "note.set_prop 1 0 C4 fine 300");
    fails(&mut s, "note.set_prop 1 0 C4 color 0");
    fails(&mut s, "note.set_prop 1 0 C4 wobble 3");
}

#[test]
fn rack_branches_hold_effects_and_survive_files() {
    let mut s = session();
    run(&mut s, "mixer.add_effect 0 rack");
    run(&mut s, "mixer.rack_add 0 1 2 delay");
    run(&mut s, "mixer.rack_add 0 1 2 compressor");
    run(&mut s, "mixer.rack_param 0 1 2 2 threshold -30");
    let rack = &s.project().mixer[0].effects[0];
    assert_eq!(rack.branches.len(), 4);
    assert_eq!(rack.branches[1].len(), 2);
    assert_eq!(rack.branches[1][1].values[0], -30.0);
    let json = serde_json::to_string(s.project()).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.mixer[0].effects, s.project().mixer[0].effects);

    fails(&mut s, "mixer.rack_add 0 1 5 delay");
    fails(&mut s, "mixer.rack_add 0 1 1 send");
    fails(&mut s, "mixer.rack_add 0 1 1 rack");
    fails(&mut s, "mixer.rack_param 0 1 2 2 sidechain 1");
    fails(&mut s, "mixer.rack_param 0 1 2 3 mix 1");
    fails(&mut s, "mixer.rack_remove 0 1 1 1");
    for _ in 0..4 {
        run(&mut s, "mixer.rack_add 0 1 2 eq");
    }
    fails(&mut s, "mixer.rack_add 0 1 2 eq");
    run(&mut s, "mixer.add_effect 0 eq");
    fails(&mut s, "mixer.rack_add 0 2 1 delay");
}

fn sig(s: &str) -> TimeSig {
    s.parse().unwrap()
}

#[test]
fn the_project_signature_carries_patterns_that_follow_it() {
    let mut s = session();
    run(&mut s, "pattern.add");
    run(&mut s, "pattern.set_time_signature 2 3/4");
    let before = s.project().clone();
    run(&mut s, "project.set_time_signature 7/8");
    let p = s.project();
    assert_eq!(p.time_signature, sig("7/8"));
    assert_eq!(p.patterns[0].time_signature, sig("7/8"));
    assert_eq!(p.patterns[0].length_steps, 14, "one bar stays one bar");
    assert_eq!(
        p.patterns[1].time_signature,
        sig("3/4"),
        "its own signature stays"
    );
    run(&mut s, "pattern.add");
    assert_eq!(s.project().patterns[2].time_signature, sig("7/8"));
    assert_eq!(s.project().patterns[2].length_ticks(), 336);
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    assert_eq!(*s.project(), before);
    fails(&mut s, "project.set_time_signature 7/9");
    fails(&mut s, "pattern.set_time_signature 9 3/4");
}

#[test]
fn bar_arguments_follow_the_song_meter() {
    let mut s = session();
    run(&mut s, "project.set_time_signature 3/4");
    run(&mut s, "playlist.set_time_signature 2 7/8");
    let p = s.project();
    assert_eq!(p.playlist.markers[0].tick, 576);
    assert_eq!(p.playlist.markers[0].name, "7/8");
    // Bar 3 (index 2) is the first 7/8 bar: 576 + 336 ends it.
    run(&mut s, "playlist.add_clip 1 1 2.5 1");
    let clip = s.project().playlist.clips[0];
    assert_eq!((clip.start, clip.length), (576 + 168, 336));
    run(&mut s, "transport.set_loop 1 3");
    assert_eq!(s.project().playlist.loop_range, Some((288, 912)));
    assert_eq!(s.project().song_length_ticks(), 576 + 336 * 2);
    // Inserting space moves the change; undo puts it back.
    run(&mut s, "playlist.insert_space 0 1");
    assert_eq!(s.project().playlist.markers[0].tick, 576 + 288);
    run(&mut s, "edit.undo");
    assert_eq!(s.project().playlist.markers[0].tick, 576);
    run(&mut s, "playlist.set_time_signature 2 none");
    assert!(s.project().playlist.markers.is_empty());
    run(&mut s, "edit.undo");
    assert_eq!(s.project().playlist.markers.len(), 1);
    fails(&mut s, "playlist.set_time_signature 0 5/4");
    fails(&mut s, "playlist.set_time_signature 1 none");
}

#[test]
fn a_signature_change_keeps_a_label_name() {
    let mut s = session();
    run(&mut s, "playlist.add_marker 4 Chorus");
    run(&mut s, "playlist.set_time_signature 4 6/8");
    let marker = &s.project().playlist.markers[0];
    assert_eq!(marker.name, "Chorus");
    assert_eq!(
        marker.action,
        musiccraft_project::MarkerAction::TimeSignature(sig("6/8"))
    );
}

#[test]
fn chords_last_a_bar_of_the_pattern() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "pattern.set_time_signature 1 3/4");
    run(&mut s, "note.chords 1 C major 1-4");
    let notes = &s.project().patterns[0].notes;
    assert_eq!(notes[0].length, 288);
    assert!(notes.iter().any(|n| n.start == 288));
}

#[test]
fn track_eq_and_slot_mix_are_undoable_and_saved() {
    let mut s = session();
    let before = s.project().clone();
    run(&mut s, "mixer.set_eq 1 low 6 200");
    run(&mut s, "mixer.add_effect 1 reverb");
    run(&mut s, "mixer.set_effect_mix 1 1 0.25");
    let track = &s.project().mixer[1];
    assert_eq!(track.eq.bands[0], (6.0, 200.0));
    assert!((track.effects[0].mix - 0.25).abs() < 1e-6);
    let json = s.project().to_json().unwrap();
    let back = Project::from_json(&json).unwrap();
    assert_eq!(back.mixer[1], s.project().mixer[1]);
    fails(&mut s, "mixer.set_eq 1 low 30");
    fails(&mut s, "mixer.set_eq 1 mid 3 5");
    fails(&mut s, "mixer.set_eq 1 bass 3");
    fails(&mut s, "mixer.set_effect_mix 1 1 2");
    for _ in 0..3 {
        run(&mut s, "edit.undo");
    }
    assert_eq!(*s.project(), before);
}

#[test]
fn automation_curves_are_set_moved_and_undone() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "automation.create channel:1:volume");
    run(&mut s, "automation.add_point 1 4 0.2");
    let before = s.project().clone();
    run(&mut s, "automation.set_curve 1 4 stairs 0.5");
    let p = s.project().automations[0].points[1];
    assert_eq!(
        (p.curve, p.tension),
        (musiccraft_project::Curve::Stairs, 0.5)
    );
    run(&mut s, "automation.move_point 1 4 8 0.3");
    assert_eq!(
        s.project().automations[0].points[1].curve,
        musiccraft_project::Curve::Stairs,
        "a moved point keeps its curve"
    );
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    assert_eq!(*s.project(), before);
    fails(&mut s, "automation.set_curve 1 4 wobble");
    fails(&mut s, "automation.set_curve 1 4 single 2");
    fails(&mut s, "automation.set_curve 1 5 hold");
}

#[test]
fn automation_switch_and_range_are_set_and_undone() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "automation.create channel:1:volume");
    let before = s.project().clone();
    run(&mut s, "automation.set_enabled 1 off");
    run(&mut s, "automation.set_range 1 0.75 0.25");
    let a = &s.project().automations[0];
    assert!(!a.enabled);
    assert_eq!((a.low, a.high), (0.75, 0.25));
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    assert_eq!(*s.project(), before);
    fails(&mut s, "automation.set_range 1 -0.1 1");
    fails(&mut s, "automation.set_range 1 0 1.5");
    fails(&mut s, "automation.set_enabled 1 maybe");
    fails(&mut s, "automation.set_enabled 2 on");
}

#[test]
fn windows_sample_paths_localize_and_imports_report_them() {
    let root = std::env::temp_dir().join(format!("musiccraft-localize-{}", std::process::id()));
    std::fs::create_dir_all(root.join("drums")).unwrap();
    std::fs::write(root.join("drums/kick.wav"), b"x").unwrap();
    musiccraft_paths::configure(musiccraft_paths::PathConfig {
        drives: vec![("z:".to_owned(), root.clone())],
        ..musiccraft_paths::PathConfig::default()
    });
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, r#"channel.set_sample 1 "Z:\\Drums\\Kick.WAV""#);
    // A missing sample is kept as written (added directly: commands refuse it).
    let mut project = s.project().clone();
    project.channels.push(musiccraft_project::Channel::new(
        musiccraft_project::ChannelId(500),
        "Snare",
        musiccraft_project::Instrument::Sampler {
            sample: r"Z:\Missing\snare.wav".to_owned(),
        },
    ));
    s = Session::new(project, Registry::with_builtin());
    let before = s.project().clone();
    let message = s.run_line("project.localize_samples").unwrap().message;
    assert!(message.contains("1 sample path from"), "{message}");
    assert!(message.contains(r"Z:\Missing\snare.wav"), "{message}");
    let sample = |s: &Session, i: usize| match &s.project().channels[i].instrument {
        musiccraft_project::Instrument::Sampler { sample } => sample.clone(),
        _ => String::new(),
    };
    assert_eq!(sample(&s, 0), root.join("drums/kick.wav").to_string_lossy());
    assert_eq!(
        sample(&s, 1),
        r"Z:\Missing\snare.wav",
        "a missing one is kept"
    );
    run(&mut s, "edit.undo");
    assert_eq!(*s.project(), before);
    // An FL project's factory sample is reported when it is not found.
    let message = s
        .run_line(&format!(
            "project.import_flp {}",
            flp_asset("FL 20.8.4.flp")
        ))
        .unwrap()
        .message;
    assert!(
        message.contains("samples not found on this system") && message.contains("22in Kick.wav"),
        "{message}"
    );
    musiccraft_paths::configure(musiccraft_paths::PathConfig::default());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn patterns_have_no_colour_until_one_is_set() {
    let mut s = session();
    assert_eq!(s.project().patterns[0].color, None);
    run(&mut s, "pattern.set_color 1 3");
    assert_eq!(s.project().patterns[0].color, Some(2));
    run(&mut s, "pattern.set_color 1 none");
    assert_eq!(s.project().patterns[0].color, None);
    run(&mut s, "edit.undo");
    assert_eq!(s.project().patterns[0].color, Some(2));
    fails(&mut s, "pattern.set_color 1 17");
    fails(&mut s, "pattern.set_color 9 1");
}

#[test]
fn control_set_and_reset_move_a_control_and_undo_restores_it() {
    let mut s = session();
    run(&mut s, "control.set mixer:0:pan 1");
    assert!((s.project().mixer[0].pan - 1.0).abs() < 1e-6);
    run(&mut s, "control.reset mixer:0:pan");
    assert!(s.project().mixer[0].pan.abs() < 1e-6);
    run(&mut s, "edit.undo");
    assert!((s.project().mixer[0].pan - 1.0).abs() < 1e-6);
    run(&mut s, "edit.undo");
    assert!(s.project().mixer[0].pan.abs() < 1e-6);
    fails(&mut s, "control.set mixer:0:volume 1.5");
}

#[test]
fn control_reset_returns_a_channel_volume_to_its_default() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.set_volume 1 0.2");
    run(&mut s, "control.reset channel:1:volume");
    let volume = s.project().channels[0].volume;
    assert!(
        (volume - musiccraft_project::DEFAULT_CHANNEL_VOLUME).abs() < 1e-4,
        "{volume}"
    );
}

#[test]
fn set_effects_enabled_bypasses_a_whole_track_and_undo_restores_each_slot() {
    let mut s = session();
    run(&mut s, "mixer.add_effect 1 delay");
    run(&mut s, "mixer.add_effect 1 reverb");
    run(&mut s, "mixer.set_effect_enabled 1 2 off");
    run(&mut s, "mixer.set_effects_enabled 1 off");
    assert!(s.project().mixer[1].effects.iter().all(|e| !e.enabled));
    run(&mut s, "edit.undo");
    let enabled: Vec<bool> = s.project().mixer[1]
        .effects
        .iter()
        .map(|e| e.enabled)
        .collect();
    assert_eq!(enabled, [true, false]);
    run(&mut s, "mixer.set_effects_enabled 1 on");
    assert!(s.project().mixer[1].effects.iter().all(|e| e.enabled));
}

#[test]
fn playlist_track_heights_are_saved_undoable_and_bounded() {
    let mut s = session();
    run(&mut s, "playlist.set_track_height 2 150");
    run(&mut s, "playlist.set_track_height 4 300");
    let p = s.project();
    assert_eq!(p.playlist.track_heights.get(&1), Some(&150));
    assert!(p.to_json().unwrap().contains("track_heights"));
    // 100% is the default and is not stored.
    run(&mut s, "playlist.set_track_height 2 100");
    assert_eq!(s.project().playlist.track_heights.len(), 1);
    assert!(s.run_line("playlist.set_track_height 2 1000").is_err());
    run(&mut s, "playlist.reset_track_heights");
    assert!(s.project().playlist.track_heights.is_empty());
    run(&mut s, "edit.undo");
    assert_eq!(s.project().playlist.track_heights.get(&3), Some(&300));
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    assert_eq!(s.project().playlist.track_heights.get(&1), Some(&150));
}

#[test]
fn mixer_docks_are_saved_undoable_and_never_move_the_master() {
    let mut s = session();
    run(&mut s, "mixer.set_dock 4 right");
    assert_eq!(s.project().mixer[4].dock, musiccraft_project::Dock::Right);
    assert!(
        s.project()
            .to_json()
            .unwrap()
            .contains("\"dock\": \"right\"")
    );
    assert!(s.run_line("mixer.set_dock 0 right").is_err());
    assert!(s.run_line("mixer.set_dock 4 top").is_err());
    run(&mut s, "edit.undo");
    assert_eq!(s.project().mixer[4].dock, musiccraft_project::Dock::Middle);
}

#[test]
fn resetting_a_mixer_track_clears_it_and_undo_brings_it_back() {
    let mut s = session();
    run(&mut s, "mixer.rename 3 Vox");
    run(&mut s, "mixer.set_volume 3 0.5");
    run(&mut s, "mixer.add_effect 3 delay");
    run(&mut s, "mixer.set_send 3 5 0.8");
    run(&mut s, "mixer.set_track_color 3 4");
    let before = s.project().mixer[3].clone();
    run(&mut s, "mixer.reset_track 3");
    let reset = &s.project().mixer[3];
    assert_eq!(reset.name, "Insert 3");
    assert!(reset.effects.is_empty() && reset.sends.is_empty());
    assert_eq!((reset.volume, reset.color), (1.0, None));
    assert!(reset.to_master);
    run(&mut s, "edit.undo");
    assert_eq!(s.project().mixer[3], before);
    run(&mut s, "mixer.reset_track 0");
    assert_eq!(s.project().mixer[0].name, "Master");
    assert!(s.run_line("mixer.set_track 4 {\"bad\": 1}").is_err());
}

#[test]
fn renaming_a_channel_group_moves_its_channels_and_undoes_in_one_step() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.add_synth");
    let n = s.project().channels.len();
    run(&mut s, &format!("channel.set_group {} Drums", n - 1));
    run(&mut s, &format!("channel.set_group {n} Drums"));
    run(&mut s, "channel.rename_group Drums \"Kit one\"");
    let groups = |s: &Session| -> Vec<String> {
        s.project().channels[n - 2..]
            .iter()
            .map(|c| c.group.clone())
            .collect()
    };
    assert_eq!(groups(&s), ["Kit one", "Kit one"]);
    run(&mut s, "edit.undo");
    assert_eq!(groups(&s), ["Drums", "Drums"]);
    run(&mut s, "channel.rename_group Drums");
    assert_eq!(groups(&s), ["", ""]);
    assert!(s.run_line("channel.rename_group Nothing Else").is_err());
}

#[test]
fn an_auto_pattern_length_follows_its_notes_in_whole_bars() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    let n = s.project().channels.len();
    run(&mut s, "pattern.set_length 1 32");
    run(&mut s, "pattern.set_length 1 auto");
    let bar = s.project().current_pattern().time_signature.bar_ticks();
    let length = |s: &Session| s.project().current_pattern().length_ticks();
    assert_eq!(length(&s), bar, "an empty auto pattern is one bar");
    run(&mut s, &format!("note.add {n} {}t 24t 60 100", bar + 10));
    assert_eq!(length(&s), 2 * bar, "a note in bar 2 makes two bars");
    assert!(
        s.project()
            .to_json()
            .unwrap()
            .contains("\"auto_length\": true"),
        "saved with the project"
    );
    run(&mut s, "pattern.set_length 1 8");
    assert!(!s.project().current_pattern().auto_length);
    run(&mut s, "edit.undo");
    assert!(s.project().current_pattern().auto_length);
    run(&mut s, "edit.undo");
    run(&mut s, "edit.undo");
    let pattern = s.project().current_pattern();
    assert!(!pattern.auto_length && pattern.length_steps == 32);
    assert!(s.run_line("pattern.set_length 1 automatic").is_err());
}

#[test]
fn moving_a_marker_keeps_its_name_and_action() {
    let mut s = session();
    run(&mut s, "playlist.add_marker 2 Drop");
    run(&mut s, "playlist.set_marker_action 2 pause");
    run(&mut s, "playlist.add_marker 4 End");
    run(&mut s, "playlist.move_marker 2 6");
    let markers = &s.project().playlist.markers;
    let bar = TICKS_PER_BAR;
    assert_eq!(
        markers.iter().map(|m| m.tick).collect::<Vec<_>>(),
        [4 * bar, 6 * bar]
    );
    assert_eq!(markers[1].name, "Drop");
    assert_eq!(markers[1].action, musiccraft_project::MarkerAction::Pause);
    fails(&mut s, "playlist.move_marker 6 4");
    fails(&mut s, "playlist.move_marker 3 5");
    run(&mut s, "edit.undo");
    assert_eq!(s.project().playlist.markers[0].tick, 2 * bar);
    run(&mut s, "playlist.set_time_signature 8 3/4");
    fails(&mut s, "playlist.move_marker 8 9");
}

#[test]
fn set_sample_renames_a_default_name_only() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, "channel.add_sampler builtin:kick Boom");
    run(&mut s, "channel.add_sampler builtin:kick Clap");
    for n in 1..=3 {
        run(&mut s, &format!("channel.set_sample {n} builtin:clap"));
    }
    let names = |s: &Session| -> Vec<String> {
        s.project()
            .channels
            .iter()
            .map(|c| c.name.clone())
            .collect()
    };
    assert_eq!(names(&s), ["Clap", "Boom", "Clap"]);
    for _ in 1..=3 {
        run(&mut s, "edit.undo");
    }
    // A typed name that happens to match the new default is kept on undo.
    assert_eq!(names(&s), ["Kick", "Boom", "Clap"]);
}

#[test]
fn swap_sound_moves_the_sound_and_keeps_the_rest() {
    let mut s = session();
    run(&mut s, "channel.add_synth Lead");
    run(&mut s, "channel.set_volume 1 0.5");
    run(&mut s, "channel.route 1 2");
    run(&mut s, "note.add 1 0 1 C4");
    run(&mut s, "channel.set_param 1 cutoff 1200");
    run(&mut s, "channel.add_sampler builtin:clap");
    let before = s.project().clone();
    // What a drop from the browser runs: add, swap, remove the added one.
    run(
        &mut s,
        r#"edit.batch "channel.add_sampler builtin:kick" "channel.swap_sound 1 3" "channel.remove 3""#,
    );
    let p = s.project();
    assert_eq!(p.channels.len(), 2);
    let lead = &p.channels[0];
    assert_eq!(lead.id, before.channels[0].id);
    assert_eq!(lead.name, "Kick");
    assert!(matches!(&lead.instrument, Instrument::Sampler { sample } if sample == "builtin:kick"));
    assert_eq!(lead.volume, 0.5);
    assert_eq!(lead.mixer_track, before.channels[0].mixer_track);
    assert_ne!(lead.params, before.channels[0].params);
    assert_eq!(p.patterns[0].notes.len(), 1);
    assert_eq!(p.patterns[0].notes[0].channel, lead.id);
    run(&mut s, "edit.undo");
    assert_eq!(s.project().channels, before.channels);
    run(&mut s, "edit.redo");
    assert_eq!(s.project().channels[0].name, "Kick");
}

#[test]
fn splitting_a_clip_bakes_each_piece_into_its_own_pattern() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    // A one-bar pattern: notes on steps 0, 6 (two steps long) and 12.
    run(&mut s, "note.add 1 0 1 C5");
    run(&mut s, "note.add 1 6 2 D5");
    run(&mut s, "note.add 1 12 1 E5");
    run(&mut s, "pattern.record_event channel:1:volume 0");
    run(&mut s, "playlist.add_clip 1 1 0 2");
    // Cut half-way into the D5 (steps are 24 ticks): it is shortened, and
    // the second piece starts after it.
    run(&mut s, "playlist.split_clip 1 156t");
    let project = s.project();
    assert_eq!(project.patterns.len(), 3);
    assert_eq!(project.patterns[1].name, "Pattern 1 #2");
    assert_eq!(project.patterns[2].name, "Pattern 1 #3");
    let notes = |i: usize| -> Vec<(u32, u32, u8)> {
        s.project().patterns[i]
            .notes
            .iter()
            .map(|n| (n.start, n.length, n.key))
            .collect()
    };
    assert_eq!(notes(1), [(0, 24, 60), (144, 12, 62)]);
    // E5 of the first bar, then the repeat: C5, D5, E5.
    assert_eq!(
        notes(2),
        [(132, 24, 64), (228, 24, 60), (372, 48, 62), (516, 24, 64)]
    );
    assert_eq!(s.project().patterns[1].length_steps, 7);
    assert_eq!(s.project().patterns[2].length_steps, 26);
    // The second piece holds the lane's value from its start.
    let lane = &s.project().patterns[2].automation[0];
    assert_eq!(lane.points[0].tick, 0);
    assert_eq!(clip_spans(&s), [(0, 0, 156, 0), (0, 156, 612, 0)]);
    let ids: Vec<_> = s.project().patterns.iter().map(|p| p.id).collect();
    let sources: Vec<_> = s
        .project()
        .playlist
        .clips
        .iter()
        .map(|c| c.pattern())
        .collect();
    assert_eq!(sources, [Some(ids[1]), Some(ids[2])]);
    fails(&mut s, "playlist.split_clip 1 0");
    run(&mut s, "edit.undo");
    assert_eq!(s.project().patterns.len(), 1);
    assert_eq!(clip_spans(&s), [(0, 0, 768, 0)]);
    // Audio clips are sliced, not split.
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, "playlist.add_audio_clip 2 2 0 1");
    fails(&mut s, "playlist.split_clip 2 0.5");
}

#[test]
fn splitting_a_pattern_by_channel_stacks_one_clip_per_channel() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    run(&mut s, "channel.add_sampler builtin:kick");
    run(&mut s, "channel.rename 1 Lead");
    run(&mut s, "note.add 2 0 1 C5");
    run(&mut s, "note.add 1 4 1 E5");
    run(&mut s, "pattern.record_event channel:2:volume 0");
    run(&mut s, "pattern.record_event mixer:0:volume 0");
    fails(&mut s, "pattern.split_by_channel 2");
    // Two clips of the pattern; track 2 is taken under the second one.
    run(&mut s, "playlist.add_clip 1 1 0");
    run(&mut s, "playlist.add_clip 1 1 2");
    run(&mut s, "pattern.add Other");
    run(&mut s, "playlist.add_clip 2 2 2");
    run(&mut s, "pattern.split_by_channel 1");
    let project = s.project();
    assert_eq!(project.patterns.len(), 4);
    assert_eq!(project.current_pattern, 2);
    let (lead, kick) = (&project.patterns[2], &project.patterns[3]);
    assert_eq!(lead.name, "Pattern 1 - Lead");
    assert!(kick.name.starts_with("Pattern 1 - "));
    assert!(
        lead.notes
            .iter()
            .all(|n| n.channel == project.channels[0].id)
    );
    assert!(
        kick.notes
            .iter()
            .all(|n| n.channel == project.channels[1].id)
    );
    // The kick's volume lane goes with the kick, the mixer lane with the first.
    assert_eq!(lead.automation.len(), 1);
    assert_eq!(kick.automation.len(), 1);
    let placed: Vec<_> = project
        .playlist
        .clips
        .iter()
        .map(|c| (c.track, c.start, c.pattern()))
        .collect();
    assert_eq!(
        placed,
        [
            (0, 0, Some(lead.id)),
            (0, 768, Some(lead.id)),
            (1, 768, Some(project.patterns[1].id)),
            (2, 0, Some(kick.id)),
            (2, 768, Some(kick.id)),
        ]
    );
    run(&mut s, "edit.undo");
    assert_eq!(s.project().patterns.len(), 2);
    assert_eq!(s.project().current_pattern, 1);
    assert_eq!(s.project().playlist.clips.len(), 3);
}

#[test]
fn splitting_undoes_and_redoes_exactly() {
    assert_round_trip(&[
        "channel.add_synth",
        "channel.add_synth",
        "note.add 1 0 1 C5",
        "note.add 2 3 1 G5",
        "playlist.add_clip 1 1 0 2",
        "playlist.split_clip 1 1",
        "pattern.split_by_channel 1",
        "pattern.split_by_channel 2",
    ]);
}
