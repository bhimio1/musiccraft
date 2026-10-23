use musiccraft_params::{EffectKind, TICKS_PER_STEP, inst};

const TICKS_PER_BAR: u32 = 384;

use super::*;

#[test]
fn new_project_has_defaults() {
    let project = Project::default();
    assert_eq!(project.name(), "Untitled");
    assert!((project.tempo_bpm() - DEFAULT_TEMPO_BPM).abs() < f64::EPSILON);
    assert_eq!(project.transport(), Transport::Stopped);
    assert_eq!(project.patterns.len(), 1);
    assert_eq!(project.mixer.len(), DEFAULT_INSERTS + 1);
    assert_eq!(project.mixer[0].name, "Master");
    assert!(project.channels.is_empty());
    assert_eq!(project.play_mode, PlayMode::Pattern);
}

#[test]
fn set_tempo_returns_previous_value() {
    let mut project = Project::default();
    let previous = project.set_tempo_bpm(140.0).unwrap();
    assert!((previous - DEFAULT_TEMPO_BPM).abs() < f64::EPSILON);
    assert!((project.tempo_bpm() - 140.0).abs() < f64::EPSILON);
}

#[test]
fn tempo_range_edges_are_inclusive() {
    let mut project = Project::default();
    assert!(project.set_tempo_bpm(MIN_TEMPO_BPM).is_ok());
    assert!(project.set_tempo_bpm(MAX_TEMPO_BPM).is_ok());
}

#[test]
fn invalid_tempo_is_rejected_and_project_unchanged() {
    let mut project = Project::default();
    for bad in [0.0, 9.9, 999.1, f64::NAN, f64::INFINITY, -120.0] {
        assert!(matches!(
            project.set_tempo_bpm(bad),
            Err(ProjectError::TempoOutOfRange(_))
        ));
    }
    assert!((project.tempo_bpm() - DEFAULT_TEMPO_BPM).abs() < f64::EPSILON);
}

#[test]
fn set_transport_returns_previous_state() {
    let mut project = Project::default();
    assert_eq!(
        project.set_transport(Transport::Playing),
        Transport::Stopped
    );
    assert_eq!(
        project.set_transport(Transport::Stopped),
        Transport::Playing
    );
}

#[test]
fn numbers_are_one_based_and_master_is_zero() {
    let mut project = Project::default();
    project.channels.push(Channel::new(
        ChannelId(10),
        "Kick",
        Instrument::Sampler {
            sample: "builtin:kick".into(),
        },
    ));
    assert_eq!(project.channel_index(1), Ok(0));
    assert!(project.channel_index(0).is_err());
    assert!(project.channel_index(2).is_err());
    assert_eq!(project.pattern_index(1), Ok(0));
    assert_eq!(project.mixer_index(0), Ok(0));
    assert!(project.mixer_index(DEFAULT_INSERTS + 1).is_err());
}

#[test]
fn pattern_length_grows_to_fit_notes() {
    let mut pattern = Pattern::new(PatternId(1), "P");
    assert_eq!(pattern.length_ticks(), 16 * TICKS_PER_STEP);
    pattern.put_note(Note {
        props: crate::NoteProps::default(),
        slide: false,
        channel: ChannelId(1),
        start: TICKS_PER_BAR + 5,
        length: 10,
        key: 60,
        velocity: 100,
        pan: 0,
    });
    assert_eq!(pattern.length_ticks(), 2 * TICKS_PER_BAR);
}

#[test]
fn put_note_replaces_same_position_and_key() {
    let mut pattern = Pattern::new(PatternId(1), "P");
    let note = Note {
        props: crate::NoteProps::default(),
        slide: false,
        channel: ChannelId(1),
        start: 0,
        length: 24,
        key: 60,
        velocity: 100,
        pan: 0,
    };
    assert_eq!(pattern.put_note(note), None);
    let louder = Note {
        velocity: 127,
        ..note
    };
    assert_eq!(pattern.put_note(louder), Some(note));
    assert_eq!(pattern.notes, [louder]);
}

fn rich_project() -> Project {
    let mut project = Project::new("Song");
    project.set_tempo_bpm(140.0).unwrap();
    let kick = ChannelId(project.alloc_id());
    let mut channel = Channel::new(
        kick,
        "Kick",
        Instrument::Sampler {
            sample: "builtin:kick".into(),
        },
    );
    channel.params.0[inst::PITCH] = -2.0;
    channel.mixer_track = 1;
    project.channels.push(channel);
    let lead = ChannelId(project.alloc_id());
    project
        .channels
        .push(Channel::new(lead, "Lead", Instrument::Synth));
    project.patterns[0].put_note(Note {
        props: crate::NoteProps::default(),
        slide: false,
        channel: kick,
        start: 0,
        length: 24,
        key: 60,
        velocity: 100,
        pan: 0,
    });
    project.mixer[1]
        .effects
        .push(EffectSlot::new(EffectKind::Reverb));
    project.playlist.clips.push(Clip {
        source: ClipSource::Pattern(PatternId(1)),
        track: 0,
        start: 0,
        length: TICKS_PER_BAR,
        offset: 0,
        muted: false,
    });
    project.play_mode = PlayMode::Song;
    project
}

#[test]
fn json_round_trip_preserves_everything_but_transport() {
    let mut project = rich_project();
    project.set_transport(Transport::Playing);
    let json = project.to_json().unwrap();
    assert!(json.contains("\"format\": \"musiccraft-project\""));
    assert!(
        json.contains("\"pitch\": -2.0"),
        "params are stored by name"
    );
    let loaded = Project::from_json(&json).unwrap();
    project.set_transport(Transport::Stopped);
    assert_eq!(loaded, project);
}

#[test]
fn pan_law_is_saved_and_older_files_load_as_circular() {
    let mut project = Project::default();
    assert_eq!(project.pan_law, PanLaw::Circular);
    project.pan_law = PanLaw::Triangular;
    let json = project.to_json().unwrap();
    assert!(json.contains("\"pan_law\": \"triangular\""));
    assert_eq!(
        Project::from_json(&json).unwrap().pan_law,
        PanLaw::Triangular
    );
    let older = json.replace("\"pan_law\": \"triangular\",", "");
    assert!(!older.contains("pan_law"));
    assert_eq!(
        Project::from_json(&older).unwrap().pan_law,
        PanLaw::Circular
    );
}

#[test]
fn per_pattern_swing_from_older_files_becomes_the_project_swing() {
    let mut project = Project::default();
    project.patterns.push(Pattern::new(PatternId(9), "B"));
    let json = project.to_json().unwrap();
    assert!(
        json.contains("\"swing\": 0.0"),
        "the project swing is saved"
    );
    // An older file: no project swing, swing on two patterns.
    let older = json
        .replace("\"swing\": 0.0,", "")
        .replace(
            "\"length_steps\": 16,",
            "\"length_steps\": 16, \"swing\": 0.25,",
        )
        .replacen("\"swing\": 0.25,", "\"swing\": 0.5,", 1);
    let loaded = Project::from_json(&older).unwrap();
    assert!(
        (loaded.swing - 0.5).abs() < f32::EPSILON,
        "{}",
        loaded.swing
    );
    let saved = loaded.to_json().unwrap();
    assert_eq!(saved.matches("\"swing\"").count(), 1, "only the project's");
}

#[test]
fn loading_rejects_other_formats_and_newer_versions() {
    let json = rich_project().to_json().unwrap();
    let other = json.replace("musiccraft-project", "something-else");
    assert!(matches!(
        Project::from_json(&other),
        Err(ProjectError::Format(_))
    ));
    let newer = json.replace("\"version\": 1", "\"version\": 99");
    assert!(matches!(
        Project::from_json(&newer),
        Err(ProjectError::Format(_))
    ));
    assert!(Project::from_json("{").is_err());
}

#[test]
fn loading_repairs_dangling_references() {
    let mut project = rich_project();
    project.channels.remove(0);
    project.current_pattern = 99;
    project.channels[0].mixer_track = 500;
    let json = project
        .to_json()
        .unwrap()
        .replace("\"tempo_bpm\": 140.0", "\"tempo_bpm\": 141.0");
    let loaded = Project::from_json(&json).unwrap();
    assert!(
        loaded.patterns[0].notes.is_empty(),
        "notes of removed channel dropped"
    );
    assert_eq!(loaded.current_pattern, 0);
    assert_eq!(loaded.channels[0].mixer_track, 0);
    assert!((loaded.tempo_bpm() - 141.0).abs() < f64::EPSILON);
    let fresh = loaded.clone().alloc_id();
    assert!(fresh > loaded.channels[0].id.0);
}

#[test]
fn loading_rejects_bad_tempo() {
    let json = rich_project()
        .to_json()
        .unwrap()
        .replace("\"tempo_bpm\": 140.0", "\"tempo_bpm\": 5000.0");
    assert!(matches!(
        Project::from_json(&json),
        Err(ProjectError::TempoOutOfRange(_))
    ));
}

#[test]
fn unknown_and_missing_params_fall_back_to_defaults() {
    let json = rich_project()
        .to_json()
        .unwrap()
        .replace("\"pitch\": -2.0", "\"bogus\": 3.0");
    let loaded = Project::from_json(&json).unwrap();
    assert!((loaded.channels[0].params.get(inst::PITCH)).abs() < f32::EPSILON);
}

#[test]
fn song_length_rounds_up_to_bars() {
    let mut project = Project::default();
    assert_eq!(project.song_length_ticks(), TICKS_PER_BAR);
    project.playlist.clips.push(Clip {
        source: ClipSource::Pattern(PatternId(1)),
        track: 3,
        start: TICKS_PER_BAR,
        length: 10,
        offset: 0,
        muted: false,
    });
    assert_eq!(project.song_length_ticks(), 2 * TICKS_PER_BAR);
    assert_eq!(project.playlist.clip_at(3, TICKS_PER_BAR + 9), Some(0));
    assert_eq!(project.playlist.clip_at(3, TICKS_PER_BAR + 10), None);
}

#[test]
fn range_checks() {
    assert!(check_volume(1.0).is_ok());
    assert!(check_volume(1.3).is_err());
    assert!(check_pan(-1.0).is_ok());
    assert!(check_pan(f32::NAN).is_err());
}

#[test]
fn clips_keep_the_old_file_shape() {
    let clip = Clip {
        source: ClipSource::Pattern(PatternId(3)),
        track: 1,
        start: 0,
        length: 384,
        offset: 0,
        muted: false,
    };
    let json = serde_json::to_string(&clip).unwrap();
    assert_eq!(json, r#"{"pattern":3,"track":1,"start":0,"length":384}"#);
    let audio: Clip =
        serde_json::from_str(r#"{"audio":7,"track":0,"start":96,"length":10}"#).unwrap();
    assert_eq!(audio.audio(), Some(ChannelId(7)));
}

#[test]
fn automation_range_and_switch_save_only_when_changed() {
    let mut automation = Automation::new(
        AutomationId(1),
        "Fade".into(),
        AutomationTarget::MixerVolume { track: 0 },
        vec![AutomationPoint::new(0, 0.5)],
    );
    let json = serde_json::to_string(&automation).unwrap();
    assert!(!json.contains("enabled") && !json.contains("low") && !json.contains("high"));
    assert!((automation.output(0.5) - 0.5).abs() < 1e-6);
    automation.enabled = false;
    automation.low = 0.8;
    automation.high = 0.2;
    // Low above high inverts.
    assert!((automation.output(0.0) - 0.8).abs() < 1e-6);
    assert!((automation.output(1.0) - 0.2).abs() < 1e-6);
    let json = serde_json::to_string(&automation).unwrap();
    let back: Automation = serde_json::from_str(&json).unwrap();
    assert_eq!(back, automation);
    // Files from before the settings existed load with the defaults.
    let old = r#"{"id":1,"name":"a","target":{"kind":"mixer_volume","track":0},"points":[]}"#;
    let old: Automation = serde_json::from_str(old).unwrap();
    assert!(old.enabled && old.low == 0.0 && old.high == 1.0);
}

#[test]
fn automation_interpolates_and_holds_ends() {
    let automation = Automation::new(
        AutomationId(1),
        "Cutoff".into(),
        AutomationTarget::MixerVolume { track: 1 },
        vec![
            AutomationPoint::new(96, 0.0),
            AutomationPoint::new(192, 1.0),
        ],
    );
    assert_eq!(automation.value_at(0.0), Some(0.0));
    assert_eq!(automation.value_at(144.0), Some(0.5));
    assert_eq!(automation.value_at(500.0), Some(1.0));
    assert_eq!(automation.length_ticks(), 192);
}

#[test]
fn loading_drops_clips_of_missing_sources() {
    let mut project = rich_project();
    project.playlist.clips.push(Clip {
        source: ClipSource::Automation(AutomationId(999)),
        track: 2,
        start: 0,
        length: 10,
        offset: 0,
        muted: false,
    });
    let loaded = Project::from_json(&project.to_json().unwrap()).unwrap();
    assert_eq!(loaded.playlist.clips.len(), 1);
}

#[test]
fn signatures_shape_patterns_and_the_song() {
    let mut project = Project::new("t");
    let sig = |s: &str| s.parse::<TimeSig>().unwrap();
    project.time_signature = sig("3/4");
    assert_eq!(project.song_length_ticks(), 288);
    project.playlist.clips.push(Clip {
        track: 0,
        start: 0,
        length: 300,
        source: ClipSource::Pattern(project.patterns[0].id),
        offset: 0,
        muted: false,
    });
    project.playlist.markers.push(Marker {
        tick: 288,
        name: "7/8".into(),
        action: MarkerAction::TimeSignature(sig("7/8")),
    });
    assert_eq!(project.song_length_ticks(), 288 + 336);
    assert_eq!(project.song_meter().sig_at(300), sig("7/8"));

    let mut pattern = Pattern::with_signature(PatternId(9), "p", sig("7/8"));
    assert_eq!(pattern.length_ticks(), 336);
    pattern.length_steps = 28;
    pattern.set_time_signature(sig("4/4"));
    assert_eq!(pattern.length_steps, 32, "two bars stay two bars");
    pattern.length_steps = 20;
    pattern.set_time_signature(sig("3/4"));
    assert_eq!(pattern.length_steps, 20, "odd lengths are kept");

    let json = project.to_json().unwrap();
    assert!(json.contains("\"time_signature\": \"3/4\""), "{json}");
    assert!(json.contains("\"action\": {\n            \"time_signature\": \"7/8\""));
    assert_eq!(Project::from_json(&json).unwrap(), project);
}

#[test]
fn curve_shapes_bend_hold_and_step() {
    let at = |c: Curve, t: f32, k: f32| c.shape(t, k);
    assert!((at(Curve::Single, 0.5, 0.0) - 0.5).abs() < 1e-6);
    assert!(at(Curve::Single, 0.5, 1.0) < 0.1, "slow start");
    assert!(at(Curve::Single, 0.5, -1.0) > 0.8, "fast start");
    assert!((at(Curve::Double, 0.5, 0.7) - 0.5).abs() < 1e-6);
    assert!(at(Curve::Double, 0.25, 1.0) < 0.25 && at(Curve::Double, 0.75, 1.0) > 0.75);
    assert_eq!(at(Curve::Hold, 0.99, 0.0), 0.0);
    assert_eq!(at(Curve::Hold, 1.0, 0.0), 1.0);
    assert_eq!(at(Curve::Stairs, 0.6, -1.0), 0.5, "two steps");
    assert_eq!(at(Curve::Stairs, 0.99, 1.0), 15.0 / 16.0, "sixteen steps");
    let mut a = AutomationPoint::new(0, 0.0);
    a.curve = Curve::Hold;
    let automation = Automation::new(
        AutomationId(1),
        "a".into(),
        AutomationTarget::MixerVolume { track: 0 },
        vec![a, AutomationPoint::new(96, 1.0)],
    );
    assert_eq!(automation.value_at(95.0), Some(0.0));
    assert_eq!(automation.value_at(96.0), Some(1.0));
    let json = serde_json::to_string(&automation).unwrap();
    assert!(json.contains("\"curve\":\"hold\""), "{json}");
    let back: Automation = serde_json::from_str(&json).unwrap();
    assert_eq!(back, automation);
}

#[test]
fn mixer_solo_keeps_the_tracks_a_soloed_track_feeds_and_is_fed_by() {
    let mut project = Project::new("Song");
    assert_eq!(project.mixer_audible(), None);
    project.mixer[1].sends.push(MixerSend {
        target: 3,
        level: 1.0,
    });
    project.mixer[3].solo = true;
    let audible = project.mixer_audible().unwrap();
    assert!(audible[1], "feeds the soloed track");
    assert!(!audible[2], "silenced by solo");
    assert!(audible[3]);
}

#[test]
fn older_files_get_routes_for_their_sidechains() {
    let mut project = Project::default();
    let mut compressor = EffectSlot::new(EffectKind::Compressor);
    let index = EffectKind::Compressor.sidechain_index().unwrap();
    compressor.values[index] = 5.0;
    project.mixer[2].effects.push(compressor);
    project.mixer[3].sidechain_to = vec![3, 99, 1, 1];
    let json = project.to_json().unwrap();
    // A file from before routes existed: no flag, so routes are added.
    let old = json.replace("\"sidechain_routes\": true,", "");
    assert_ne!(old, json);
    let loaded = Project::from_json(&old).unwrap();
    assert_eq!(loaded.mixer[5].sidechain_to, vec![2]);
    assert_eq!(loaded.mixer[3].sidechain_to, vec![1], "bad routes dropped");
    // Newer files keep a removed route removed.
    let loaded = Project::from_json(&json).unwrap();
    assert!(loaded.mixer[5].sidechain_to.is_empty());
}
