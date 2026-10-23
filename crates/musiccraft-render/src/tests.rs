use musiccraft_commands::{Registry, Session};
use musiccraft_engine::{EngineMessage, engine_pair};
use musiccraft_params::TICKS_PER_STEP;
use musiccraft_project::Project;

use crate::{Bridge, RenderOptions, build_sequence, render};

/// A 4/4 bar.
const TICKS_PER_BAR: u32 = 384;

fn session() -> Session {
    let mut registry = Registry::with_builtin();
    crate::register(&mut registry).unwrap();
    Session::new(Project::default(), registry)
}

fn run(s: &mut Session, lines: &str) {
    for line in lines.lines().map(str::trim).filter(|l| !l.is_empty()) {
        s.run_line(line).unwrap_or_else(|e| panic!("`{line}`: {e}"));
    }
}

fn rms(data: &[f32]) -> f32 {
    (data.iter().map(|s| s * s).sum::<f32>() / data.len().max(1) as f32).sqrt()
}

#[test]
fn pattern_mode_sequence_uses_channel_positions() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick
         channel.add_sampler builtin:hat
         step.toggle 2 4
         step.toggle 1 0",
    );
    let seq = build_sequence(s.project());
    assert_eq!(seq.length(), u64::from(16 * TICKS_PER_STEP));
    let events: Vec<_> = seq.events().iter().map(|e| (e.start, e.channel)).collect();
    assert_eq!(events, [(0, 0), (4 * u64::from(TICKS_PER_STEP), 1)]);
}

#[test]
fn song_mode_repeats_patterns_inside_long_clips_and_skips_muted_tracks() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick
         step.toggle 1 0
         playlist.add_clip 1 1 0 3
         playlist.add_clip 1 2 4
         playlist.set_track_mute 2 on
         transport.set_mode song",
    );
    let seq = build_sequence(s.project());
    let bar = u64::from(TICKS_PER_BAR);
    assert_eq!(seq.length(), 5 * bar);
    let starts: Vec<_> = seq.events().iter().map(|e| e.start).collect();
    assert_eq!(starts, [0, bar, 2 * bar]);
}

#[test]
fn clips_truncate_notes_at_their_end() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         note.add 1 0 16 C5
         playlist.add_clip 1 1 0 0.5
         transport.set_mode song",
    );
    let seq = build_sequence(s.project());
    assert_eq!(seq.events()[0].length, u64::from(TICKS_PER_BAR / 2));
}

#[test]
fn sync_sends_only_what_changed() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    let (mut engine, mut handle) = engine_pair(44_100);
    let mut bridge = Bridge::new(44_100);
    let first = bridge.sync(s.project(), &mut handle);
    assert!(first.rebuilt_graph && first.new_sequence);
    assert!(first.warnings.is_empty());

    let idle = bridge.sync(s.project(), &mut handle);
    assert_eq!(idle.messages, 0);

    run(&mut s, "channel.set_volume 1 0.5");
    let volume = bridge.sync(s.project(), &mut handle);
    assert_eq!(volume.messages, 1);
    assert!(!volume.rebuilt_graph);

    run(&mut s, "step.toggle 1 0");
    let step = bridge.sync(s.project(), &mut handle);
    assert!(step.new_sequence && !step.rebuilt_graph);

    run(&mut s, "mixer.add_effect 0 reverb");
    assert!(bridge.sync(s.project(), &mut handle).rebuilt_graph);

    run(&mut s, "transport.play");
    let play = bridge.sync(s.project(), &mut handle);
    assert_eq!(play.messages, 1);
    let mut out = vec![0.0; 2048];
    engine.process(&mut out, 2);
    assert!(engine.is_playing());
    // Replaced graphs and sequences come back for dropping on this thread.
    let _ = bridge.sync(s.project(), &mut handle);
}

#[test]
fn missing_samples_warn_and_play_silence() {
    let mut s = session();
    run(&mut s, "channel.add_synth");
    let mut project = s.project().clone();
    project.channels[0].instrument = musiccraft_project::Instrument::Sampler {
        sample: "/no/such/file.wav".into(),
    };
    let (_engine, mut handle) = engine_pair(44_100);
    let report = Bridge::new(44_100).sync(&project, &mut handle);
    assert_eq!(report.warnings.len(), 1);
}

#[test]
fn rendered_drum_pattern_has_hits_where_steps_are() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120
         channel.add_sampler builtin:kick
         step.toggle 1 0
         step.toggle 1 8",
    );
    let (left, right, warnings) = render(
        s.project(),
        RenderOptions {
            sample_rate: 48_000,
            loops: 1,
            max_tail_seconds: 0.0,
        },
    );
    assert!(warnings.is_empty());
    // One bar at 120 BPM = 2 s.
    assert_eq!(left.len(), 96_000);
    assert_eq!(left, right, "centred mono sample");
    let before_second_hit = &left[47_000..47_990];
    let second_hit = &left[48_000..49_000];
    assert!(rms(second_hit) > 10.0 * rms(before_second_hit));
    assert!(
        left[..200].iter().any(|s| s.abs() > 0.01),
        "first hit sounds at once"
    );
}

#[test]
fn render_includes_tail_then_stops_at_silence() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120
         channel.add_sampler builtin:crash
         step.toggle 1 15",
    );
    let (left, _, _) = render(s.project(), RenderOptions::default());
    let body = 2 * 44_100;
    assert!(left.len() > body + 22_050, "tail kept");
    assert!(left.len() < body + 10 * 44_100, "stopped at silence");
}

#[test]
fn render_is_deterministic() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         note.add 1 0 4 C5
         note.add 1 4 4 E5
         mixer.add_effect 0 delay
         mixer.add_effect 0 reverb",
    );
    let options = RenderOptions {
        max_tail_seconds: 1.0,
        ..RenderOptions::default()
    };
    assert_eq!(
        render(s.project(), options).0,
        render(s.project(), options).0
    );
}

#[test]
fn mixer_mute_and_solo_silence_tracks() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         note.add 1 0 16 A5
         channel.route 1 1
         mixer.set_solo 2 on",
    );
    let options = RenderOptions {
        max_tail_seconds: 0.0,
        ..RenderOptions::default()
    };
    let (soloed_out, _, _) = render(s.project(), options);
    assert!(
        rms(&soloed_out) < 1e-6,
        "insert 1 is silenced by insert 2's solo"
    );
    run(&mut s, "mixer.set_solo 2 off");
    let (audible, _, _) = render(s.project(), options);
    assert!(rms(&audible) > 0.01);
}

#[test]
fn export_command_writes_a_wav() {
    let dir = std::env::temp_dir().join(format!("musiccraft-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.wav");
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:snare\nstep.toggle 1 4");
    let line = format!("project.export_wav \"{}\" 24 2 48000", path.display());
    let outcome = s.run_line(&line).unwrap();
    assert!(outcome.message.starts_with("exported"));
    assert!(outcome.inverse.is_none());
    let buffer = musiccraft_samples::load_wav(&path).unwrap();
    assert!((buffer.sample_rate() - 48_000.0).abs() < f64::EPSILON);
    assert!(buffer.peak() > 0.1);
    assert!(s.run_line("project.export_wav out.wav 12").is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_pan_law_reaches_channel_pans_but_not_mixer_pans() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         channel.set_param 1 sustain 1
         note.add 1 0 16 A5",
    );
    let options = RenderOptions {
        sample_rate: 22_050,
        loops: 1,
        max_tail_seconds: 0.0,
    };
    let level = |s: &Session| rms(&render(s.project(), options).0);
    let circular = level(&s);
    run(&mut s, "project.set_pan_law triangular");
    let triangular = level(&s);
    // Centre: -3 dB versus -6 dB.
    let ratio = triangular / circular;
    assert!(
        (ratio - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-3,
        "{ratio}"
    );
    // A mixer pan is a balance: centred, it leaves the level alone; panned
    // right, the left side keeps only (1 - pan).
    run(&mut s, "mixer.set_pan 0 0.5");
    assert!((level(&s) / triangular - 0.5).abs() < 1e-3);
}

#[test]
fn pause_keeps_the_playhead_and_stop_rewinds() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    let (mut engine, mut handle) = engine_pair(44_100);
    let mut bridge = Bridge::new(44_100);
    let mut out = vec![0.0; 2048];
    run(&mut s, "transport.play");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    engine.process(&mut out, 2);
    run(&mut s, "transport.play_pause");
    assert!(s.project().transport().is_paused());
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(!engine.is_playing());
    let held = engine.position_ticks();
    assert!(held > 0.0, "pause keeps the playhead");
    // Play again continues from the same place.
    run(&mut s, "transport.play_pause");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(engine.is_playing());
    assert!(engine.position_ticks() > held);
    // Pausing and then stopping rewinds.
    run(&mut s, "transport.pause\ntransport.stop");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(engine.position_ticks().abs() < f64::EPSILON);
}

#[test]
fn jump_to_start_keeps_the_transport_state() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    let (mut engine, mut handle) = engine_pair(44_100);
    let mut bridge = Bridge::new(44_100);
    let mut out = vec![0.0; 2048];
    run(&mut s, "transport.play");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    engine.process(&mut out, 2);
    let played = engine.position_ticks();
    assert!(played > 0.0);
    run(&mut s, "transport.jump_to_start");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(engine.is_playing(), "playback continues");
    assert!(engine.position_ticks() < played, "back at the start");
    // Paused: the playhead moves, the transport stays paused.
    run(&mut s, "transport.pause");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(engine.position_ticks() > 0.0);
    run(&mut s, "transport.jump_to_start");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(!engine.is_playing());
    assert!(engine.position_ticks().abs() < f64::EPSILON);
}

#[test]
fn panic_releases_every_note_and_playback_goes_on() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         channel.set_param 1 sustain 1
         channel.set_param 1 release 0
         note.add 1 0 16 C5",
    );
    let (mut engine, mut handle) = engine_pair(44_100);
    let mut bridge = Bridge::new(44_100);
    let mut out = vec![0.0; 2048];
    run(&mut s, "transport.play");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert_eq!(engine.active_voices(), 1);
    // A held live key is released too.
    let _ = handle.send(musiccraft_engine::EngineMessage::NoteOn {
        channel: 0,
        key: 64,
        velocity: 1.0,
    });
    engine.process(&mut out, 2);
    assert_eq!(engine.active_voices(), 2);
    run(&mut s, "transport.panic");
    let _ = bridge.sync(s.project(), &mut handle);
    for _ in 0..4 {
        engine.process(&mut out, 2);
    }
    assert_eq!(engine.active_voices(), 0, "all notes off");
    assert!(engine.is_playing(), "playback goes on");
}

#[test]
fn requests_from_before_a_bridge_or_a_load_do_nothing() {
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick\ntransport.play");
    run(&mut s, "transport.jump_to_start\ntransport.panic");
    let (mut engine, mut handle) = engine_pair(44_100);
    let mut bridge = Bridge::new(44_100);
    let mut out = vec![0.0; 2048];
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    let position = engine.position_ticks();
    // A new project starts its counters again: no seek, no panic.
    run(
        &mut s,
        "project.new\nchannel.add_sampler builtin:kick\ntransport.play",
    );
    let report = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(engine.position_ticks() > position, "{report:?}");
}

#[test]
fn stop_message_rewinds_engine() {
    let (mut engine, _handle) = engine_pair(44_100);
    engine.apply(EngineMessage::Play);
    let mut out = vec![0.0; 1024];
    engine.process(&mut out, 2);
    engine.apply(EngineMessage::Stop);
    assert!(engine.position_ticks().abs() < f64::EPSILON);
}

#[test]
fn swing_delays_offbeat_sixteenths_only() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:hat
         step.toggle 1 0
         step.toggle 1 1
         step.toggle 1 2
         step.toggle 1 3
         project.set_swing 50",
    );
    let starts: Vec<u64> = build_sequence(s.project())
        .events()
        .iter()
        .map(|e| e.start)
        .collect();
    assert_eq!(starts, [0, 24 + 6, 48, 72 + 6]);
}

/// (start, length) of channel `channel`'s events (0-based).
fn timing(s: &Session, channel: u16) -> Vec<(u64, u64)> {
    build_sequence(s.project())
        .events()
        .iter()
        .filter(|e| e.channel == channel)
        .map(|e| (e.start, e.length))
        .collect()
}

#[test]
fn swing_mix_scales_the_project_swing_per_channel() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:hat
         channel.add_sampler builtin:kick
         channel.add_sampler builtin:snare
         step.toggle 1 1
         step.toggle 2 1
         step.toggle 3 1
         project.set_swing 100
         channel.set_param 2 swing_mix 0.5
         channel.set_param 3 swing_mix 0",
    );
    assert_eq!(timing(&s, 0), [(24 + 12, 24)], "full mix (the default)");
    assert_eq!(timing(&s, 1), [(24 + 6, 24)], "half");
    assert_eq!(timing(&s, 2), [(24, 24)], "straight");
}

#[test]
fn truncated_swing_keeps_the_note_end() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         note.add 1 1 1 C5
         note.add 1 3 2 C5
         project.set_swing 100
         channel.set_param 1 swing_truncate 1",
    );
    assert_eq!(timing(&s, 0), [(36, 12), (84, 36)]);
    // In song mode too.
    run(&mut s, "playlist.add_clip 1 1 0 1\ntransport.set_mode song");
    assert_eq!(timing(&s, 0), [(36, 12), (84, 36)]);
}

#[test]
fn exports_never_contain_the_metronome() {
    let mut s = session();
    run(&mut s, "transport.toggle_metronome");
    assert!(s.project().metronome);
    let (left, _, _) = render(
        s.project(),
        RenderOptions {
            max_tail_seconds: 0.0,
            ..RenderOptions::default()
        },
    );
    assert!(left.iter().all(|x| x.abs() < 1e-6));
}

#[test]
fn stems_split_by_mixer_track_and_line_up() {
    let dir = std::env::temp_dir().join(format!("musiccraft-stems-{}", std::process::id()));
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick
         channel.add_sampler builtin:crash
         channel.add_synth
         step.toggle 1 0
         step.toggle 2 0
         note.add 3 0 4 C5
         channel.route 1 1
         channel.route 2 1
         channel.set_mute 3 off
         mixer.rename 1 Drums/Perc",
    );
    let line = format!("project.export_stems \"{}\"", dir.display());
    let outcome = s.run_line(&line).unwrap();
    assert!(outcome.message.contains("2 stems"), "{}", outcome.message);
    let drums = musiccraft_samples::load_wav(&dir.join("01 Drums_Perc.wav")).unwrap();
    let master = musiccraft_samples::load_wav(&dir.join("00 Master.wav")).unwrap();
    assert_eq!(
        drums.len(),
        master.len(),
        "stems are padded to the same length"
    );
    assert!(drums.peak() > 0.1 && master.peak() > 0.01);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn soloing_a_source_keeps_its_bus_audible() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         note.add 1 0 16 A5
         channel.route 1 1
         mixer.route_to_master 1 off
         mixer.set_send 1 3 1
         mixer.set_solo 1 on",
    );
    let options = RenderOptions {
        max_tail_seconds: 0.0,
        ..RenderOptions::default()
    };
    let (out, _, _) = render(s.project(), options);
    assert!(
        rms(&out) > 0.01,
        "insert 1 reaches the master only through bus 3"
    );
    run(&mut s, "mixer.set_solo 1 off\nmixer.set_solo 2 on");
    let (out, _, _) = render(s.project(), options);
    assert!(rms(&out) < 1e-6);
}

#[test]
fn automation_clip_fades_the_master_in_song_mode() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120
         channel.add_synth
         note.add 1 0 16 A5
         playlist.add_clip 1 1 0 4
         automation.create mixer:0:volume
         automation.set_points 1 [[0,0.75],[1536,0.0]]
         playlist.add_automation_clip 1 2 0 4
         transport.set_mode song",
    );
    let seq = build_sequence(s.project());
    assert_eq!(seq.automation().len(), 1);
    let options = RenderOptions {
        max_tail_seconds: 0.0,
        ..RenderOptions::default()
    };
    let (out, _, _) = render(s.project(), options);
    let second = 44_100;
    let early = rms(&out[second / 2..second + second / 2]);
    let late = rms(&out[7 * second + second / 2..8 * second - 100]);
    assert!(early > 0.03, "early {early}");
    assert!(late < early / 5.0, "faded: early {early}, late {late}");
}

#[test]
fn audio_clips_play_their_sample_unpitched() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:crash
         channel.set_param 1 one_shot 0
         channel.set_param 1 root_key 48
         playlist.add_audio_clip 1 1 1 1
         transport.set_mode song",
    );
    let seq = build_sequence(s.project());
    let event = seq.events()[0];
    assert_eq!(
        (event.start, event.length, event.key),
        (u64::from(TICKS_PER_BAR), u64::from(TICKS_PER_BAR), 48)
    );
}

#[test]
fn song_loop_region_limits_playback_but_not_export() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick
         step.toggle 1 0
         playlist.add_clip 1 1 0 8
         transport.set_mode song
         transport.set_loop 2 4",
    );
    let seq = build_sequence(s.project());
    let bar = u64::from(TICKS_PER_BAR);
    assert_eq!((seq.loop_start(), seq.length()), (2 * bar, 4 * bar));
    let full = crate::build_full_sequence(s.project());
    assert_eq!(full.length(), 8 * bar);
}

/// A pad on insert `pad` ducked by a kick on insert `kick` (kept off the
/// master), with `extra` commands after: the pad's level before the second
/// kick (at 1 s) and under it.
fn ducking(pad: usize, kick: usize, extra: &str) -> (f32, f32) {
    let mut s = session();
    run(
        &mut s,
        &format!(
            "project.set_tempo 120
             channel.add_sampler builtin:kick
             channel.add_synth Pad
             step.toggle 1 0
             step.toggle 1 8
             note.add 2 0 16 A4
             channel.route 1 {kick}
             channel.route 2 {pad}
             mixer.route_to_master {kick} off
             mixer.add_effect {pad} compressor
             mixer.set_effect_param {pad} 1 threshold -40
             mixer.set_effect_param {pad} 1 ratio 20
             mixer.sidechain_to {kick} {pad} on
             mixer.set_effect_param {pad} 1 sidechain {kick}
             {extra}"
        ),
    );
    let options = RenderOptions {
        max_tail_seconds: 0.0,
        ..RenderOptions::default()
    };
    let out = render(s.project(), options).0;
    (
        rms(&out[44_100 - 8_000..44_100 - 500]),
        rms(&out[44_100 + 500..44_100 + 4_000]),
    )
}

#[test]
fn sidechained_compressor_ducks_on_each_kick() {
    let (before, under) = ducking(2, 1, "");
    assert!(
        under < 0.5 * before,
        "pad ducks under the kick: before {before}, under {under}"
    );
}

#[test]
fn a_higher_numbered_insert_keys_a_lower_one() {
    // The kick on insert 7 is processed before the pad on insert 2.
    let (before, under) = ducking(2, 7, "");
    assert!(
        under < 0.5 * before,
        "pad ducks under the kick: before {before}, under {under}"
    );
    // The same through a bus: kick 7 sends to 9, which keys the pad on 3.
    let (before, under) = ducking(
        3,
        7,
        "mixer.sidechain_to 7 3 off
         mixer.set_send 7 9 1
         mixer.route_to_master 9 off
         mixer.sidechain_to 9 3 on
         mixer.set_effect_param 3 1 sidechain 9",
    );
    assert!(
        under < 0.5 * before,
        "pad ducks under the bus: before {before}, under {under}"
    );
}

#[test]
fn removing_the_sidechain_route_stops_the_ducking() {
    let (before, under) = ducking(2, 5, "mixer.sidechain_to 5 2 off");
    assert!(
        under > 0.8 * before,
        "no route, no ducking: before {before}, under {under}"
    );
}

#[test]
fn send_effect_feeds_its_target_from_mid_chain() {
    let level = |script: &str| {
        let mut s = session();
        run(
            &mut s,
            &format!(
                "channel.add_sampler builtin:kick
                 step.toggle 1 0
                 channel.route 1 1
                 mixer.add_effect 1 send
                 mixer.set_effect_param 1 1 dry 0
                 {script}"
            ),
        );
        let options = RenderOptions {
            max_tail_seconds: 0.0,
            ..RenderOptions::default()
        };
        rms(&render(s.project(), options).0[..20_000])
    };
    let off = level("");
    let sent = level("mixer.set_effect_param 1 1 target 2");
    let muted_target = level("mixer.set_effect_param 1 1 target 2\nmixer.toggle_mute 2");
    assert!(off < 1e-6, "dry 0 and no target: silence, got {off}");
    assert!(sent > 0.01, "the kick reaches the master through insert 2");
    assert!(
        muted_target < 1e-6,
        "only through insert 2, got {muted_target}"
    );
}

#[test]
fn band_split_moves_the_low_band_to_its_target() {
    let level = |script: &str| {
        let mut s = session();
        run(
            &mut s,
            &format!(
                "channel.add_synth Sub
                 note.add 1 0 16 C2
                 channel.route 1 1
                 mixer.add_effect 1 band_split
                 mixer.set_effect_param 1 1 low_target 2
                 {script}"
            ),
        );
        let options = RenderOptions {
            max_tail_seconds: 0.0,
            ..RenderOptions::default()
        };
        rms(&render(s.project(), options).0[4_000..20_000])
    };
    let routed = level("");
    let muted = level("mixer.toggle_mute 2");
    assert!(
        routed > 0.01,
        "the bass reaches the master through insert 2"
    );
    assert!(
        muted < routed * 0.5,
        "most of a C2 is in the low band: {muted} vs {routed}"
    );
    let mut s = session();
    run(&mut s, "mixer.add_effect 1 band_split");
    assert!(
        s.run_line("mixer.set_effect_param 1 1 mid_target 1")
            .is_err()
    );
    run(&mut s, "mixer.set_effect_param 1 1 high_target 3");
    assert!(s.run_line("mixer.remove_track 3").is_err());
}

#[test]
fn controllers_drive_their_targets() {
    // A held sine at 120 BPM: one beat is 0.5 s.
    let level = |script: &str, from: f32, to: f32| {
        let mut s = sine_session(&format!("note.add 1 0 64 A4\n{script}"));
        s.run_line("project.set_tempo 120").unwrap();
        let out = render_slide(&s);
        let sr = SLIDE_RATE as f32;
        rms(&out[(from * sr) as usize..(to * sr) as usize])
    };
    let open = level("", 0.2, 1.8);
    // A macro at 0 on the channel volume silences it.
    let muted = level(
        "controller.add macro channel:1:volume\ncontroller.set 1 value 0",
        0.2,
        1.8,
    );
    assert!(open > 0.05 && muted < open * 0.01, "{open} vs {muted}");
    // A square LFO over 2 beats: loud for the first beat, silent the next.
    let lfo =
        "controller.add lfo channel:1:volume\ncontroller.set 1 shape 3\ncontroller.set 1 period 2";
    let first = level(lfo, 0.1, 0.45);
    let second = level(lfo, 0.6, 0.95);
    assert!(first > 0.05 && second < first * 0.05, "{first} vs {second}");
    // A disabled controller does nothing.
    let off = level(&format!("{lfo}\ncontroller.set 1 enabled off"), 0.6, 0.95);
    assert!((off - open).abs() < open * 0.1);
    // A formula: 0 for the first beat, full for the next.
    let formula = "controller.add formula channel:1:volume\ncontroller.set 1 expr floor(t)";
    assert!(level(formula, 0.1, 0.45) < 0.002 && level(formula, 0.6, 0.95) > 0.05);
}

#[test]
fn envelope_and_keyboard_controllers_follow_a_channels_notes() {
    // Channel 1 holds a sine; channel 2, on a muted insert, plays from beat 2
    // (1 s) to beat 3 and drives channel 1's volume.
    let level = |kind: &str, extra: &str| {
        let mut s = sine_session(&format!(
            "note.add 1 0 64 A4
             channel.add_synth
             note.add 2 8 4 C6
             channel.route 2 2
             mixer.set_mute 2 on
             controller.add {kind} channel:1:volume
             controller.set 1 channel 2
             {extra}"
        ));
        s.run_line("project.set_tempo 120").unwrap();
        let out = render_slide(&s);
        let sr = SLIDE_RATE as f32;
        let at = |a: f32, b: f32| rms(&out[(a * sr) as usize..(b * sr) as usize]);
        (at(0.2, 0.9), at(1.1, 1.4), at(1.9, 2.4))
    };
    let (before, during, after) = level(
        "envelope",
        "controller.set 1 attack 0\ncontroller.set 1 sustain 1\ncontroller.set 1 release 0.05",
    );
    assert!(
        before < 0.002 && during > 0.05 && after < 0.002,
        "{before} {during} {after}"
    );
    // C6 (72) between 60 and 84 is halfway; the value holds after the note.
    let (before, during, after) = level(
        "keyboard",
        "controller.set 1 low 60\ncontroller.set 1 high 84",
    );
    assert!(before < during, "{before} {during}");
    assert!((during - after).abs() < during * 0.1, "holds its last key");
}

#[test]
fn controller_commands_undo_and_follow_tracks() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         controller.add peak mixer:2:volume
         controller.set 1 track 3
         controller.set 1 gain 4
         controller.set 1 min 1
         controller.set 1 max 0.2",
    );
    let c = &s.project().controllers[0];
    assert_eq!((c.min, c.max), (1.0, 0.2));
    assert!(
        s.run_line("controller.set 1 expr t").is_err(),
        "peak has no expr"
    );
    assert!(s.run_line("controller.add bogus mixer:1:volume").is_err());
    assert!(
        s.run_line("mixer.remove_track 3").is_err(),
        "the peak listens to 3"
    );
    assert!(
        s.run_line("mixer.remove_track 2").is_err(),
        "the target is on 2"
    );
    s.run_line("mixer.remove_track 1").unwrap();
    let c = &s.project().controllers[0];
    assert!(matches!(
        c.kind,
        musiccraft_project::ControllerKind::Peak { track: 2, .. }
    ));
    assert_eq!(
        c.target,
        musiccraft_project::AutomationTarget::MixerVolume { track: 1 }
    );
    s.run_line("edit.undo").unwrap();
    s.run_line("edit.undo").unwrap();
    assert_eq!(
        s.project().controllers[0].max,
        1.0,
        "undo restores the old max"
    );
    run(&mut s, "controller.add formula mixer:0:pan");
    assert!(s.run_line("controller.set 2 expr sin(").is_err());
    run(
        &mut s,
        "controller.set 2 expr 0.5 + 0.5 * tri(bar)\ncontroller.remove 2",
    );
    s.run_line("edit.undo").unwrap();
    assert_eq!(s.project().controllers.len(), 2);
}

#[test]
fn zone_sampler_picks_a_zone_by_key_and_velocity() {
    let play = |note: &str| {
        let mut s = session();
        run(
            &mut s,
            &format!(
                "channel.add_zones
                 channel.add_zone 1 builtin:kick 0 59 48
                 channel.add_zone 1 builtin:snare 60 127 72
                 channel.set_zone 1 2 vel_low 64
                 {note}"
            ),
        );
        let options = RenderOptions {
            max_tail_seconds: 0.5,
            ..RenderOptions::default()
        };
        render(s.project(), options).0
    };
    let kick = play("note.add 1 0 4 C4");
    let snare = play("note.add 1 0 4 C6 110");
    let soft = play("note.add 1 0 4 C6 30");
    assert!(rms(&kick[..8_000]) > 0.01 && rms(&snare[..8_000]) > 0.01);
    assert!(
        kick[..8_000]
            .iter()
            .zip(&snare[..8_000])
            .any(|(a, b)| (a - b).abs() > 0.01)
    );
    assert!(
        rms(&soft) < 1e-6,
        "below the snare zone's velocities: silent"
    );
    // A zone's root plays the sample at its own pitch: the same sample a
    // fifth up moves faster, so it ends sooner.
    let root = play("note.add 1 0 4 C3");
    let up = play("note.add 1 0 4 G3");
    let end = |x: &[f32]| x.iter().rposition(|s| s.abs() > 1e-3).unwrap_or(0);
    assert!(end(&up) < end(&root), "{} vs {}", end(&up), end(&root));
}

#[test]
fn resynth_channels_play_and_change_sample() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_resynth builtin:tom
         note.add 1 0 8 A5
         channel.set_param 1 resyn_speed 0.5
         channel.set_param 1 resyn_blur 0.3",
    );
    let options = RenderOptions {
        max_tail_seconds: 0.5,
        ..RenderOptions::default()
    };
    let out = render(s.project(), options).0;
    assert!(out.iter().all(|x| x.is_finite()));
    assert!(rms(&out[..20_000]) > 0.001, "it sounds");
    run(&mut s, "channel.set_sample 1 builtin:snare");
    s.run_line("edit.undo").unwrap();
    assert!(matches!(
        &s.project().channels[0].instrument,
        musiccraft_project::Instrument::Resynth { sample } if sample == "builtin:tom"
    ));
}

#[test]
fn sample_edits_write_new_files_and_undo() {
    let dir = std::env::temp_dir().join(format!("musiccraft-edit-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:snare");
    assert!(
        s.run_line("sample.edit 1 reverse").is_err(),
        "an unsaved project has nowhere to put files"
    );
    s.run_line(&format!(
        "project.save \"{}\"",
        dir.join("p.mcproj").display()
    ))
    .unwrap();
    let sample = |s: &Session| match &s.project().channels[0].instrument {
        musiccraft_project::Instrument::Sampler { sample } => sample.clone(),
        _ => panic!("not a sampler"),
    };
    let length = |s: &Session| {
        musiccraft_samples::load_wav(&dir.join(sample(s)))
            .unwrap()
            .len()
    };
    run(&mut s, "sample.edit 1 trim 0 0.1");
    assert!(sample(&s).starts_with("edited/snare-"), "{}", sample(&s));
    assert_eq!(length(&s), 4_800);
    run(&mut s, "sample.edit 1 stretch 2");
    assert_eq!(length(&s), 9_600);
    run(&mut s, "sample.edit 1 cut 0 0.05");
    assert_eq!(length(&s), 7_200);
    for op in [
        "silence 0 0.01",
        "fade_in 0 0.02",
        "fade_out 0.1 0.15",
        "gain -6",
        "gain 3 0 0.05",
        "normalize -1",
        "reverse",
        "pitch 5",
        "tune 0 1",
    ] {
        run(&mut s, &format!("sample.edit 1 {op}"));
        let audio = musiccraft_samples::load_wav(&dir.join(sample(&s))).unwrap();
        assert!(audio.left().iter().all(|x| x.is_finite()), "{op}");
    }
    let peak = musiccraft_samples::load_wav(&dir.join(sample(&s)))
        .unwrap()
        .peak();
    assert!(peak > 0.1, "still audible: {peak}");
    for bad in ["trim 0.2 0.1", "stretch 9", "bogus", "gain 99"] {
        assert!(
            s.run_line(&format!("sample.edit 1 {bad}")).is_err(),
            "{bad}"
        );
    }
    // Undo walks back to the built-in sound.
    while sample(&s) != "builtin:snare" {
        s.run_line("edit.undo").unwrap();
    }
    run(&mut s, "channel.add_synth");
    assert!(
        s.run_line("sample.edit 2 reverse").is_err(),
        "no sample to edit"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stems_split_into_channels_and_clips_and_undo() {
    let dir = std::env::temp_dir().join(format!("musiccraft-split-stems-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick
         playlist.add_audio_clip 1 1 0 1
         playlist.add_audio_clip 1 3 0 1",
    );
    assert!(
        s.run_line("sample.split_stems 1 builtin").is_err(),
        "an unsaved project has nowhere to put files"
    );
    s.run_line(&format!(
        "project.save \"{}\"",
        dir.join("p.mcproj").display()
    ))
    .unwrap();
    run(&mut s, "sample.split_stems 1 builtin");
    let project = s.project();
    assert_eq!(project.channels.len(), 5);
    let name = &project.channels[0].name;
    let source = musiccraft_samples::builtin("kick", 48_000).unwrap();
    let mut sum = vec![0.0; source.len()];
    for (channel, stem) in project.channels[1..]
        .iter()
        .zip(["vocals", "drums", "bass", "other"])
    {
        assert_eq!(channel.name, format!("{name} ({stem})"));
        assert_eq!(channel.params, project.channels[0].params);
        let musiccraft_project::Instrument::Sampler { sample } = &channel.instrument else {
            panic!("not a sampler");
        };
        assert_eq!(sample, &format!("stems/kick-1/{stem}.wav"));
        let audio = musiccraft_samples::load_wav(&dir.join(sample)).unwrap();
        assert_eq!(audio.len(), source.len());
        for (total, x) in sum.iter_mut().zip(audio.left()) {
            *total += x;
        }
    }
    assert!(
        sum.iter()
            .zip(source.left())
            .all(|(a, b)| (a - b).abs() < 1e-4),
        "the stems add up to the sound"
    );
    // Each clip is muted and gets its stems on the first four free tracks
    // below it: track 3 is busy, so the first clip's go on 4 to 7 and the
    // second's on 8 to 11.
    let clips = &project.playlist.clips;
    assert_eq!(clips.len(), 10);
    let tracks = |id| -> Vec<u32> {
        clips
            .iter()
            .filter(|c| c.audio() == Some(id))
            .map(|c| c.track)
            .collect()
    };
    assert_eq!(tracks(project.channels[0].id), [0, 2]);
    assert!(clips.iter().filter(|c| c.muted).count() == 2);
    assert_eq!(tracks(project.channels[1].id), [3, 7]);
    assert_eq!(tracks(project.channels[4].id), [6, 10]);
    s.run_line("edit.undo").unwrap();
    assert_eq!(s.project().channels.len(), 1);
    assert_eq!(s.project().playlist.clips.len(), 2);
    assert!(s.project().playlist.clips.iter().all(|c| !c.muted));
    s.run_line("edit.redo").unwrap();
    assert_eq!(s.project().channels.len(), 5);
    s.run_line("edit.undo").unwrap();

    run(
        &mut s,
        "sample.add_stems 1 stems/kick-1/vocals.wav stems/kick-1/drums.wav stems/kick-1/bass.wav stems/kick-1/other.wav",
    );
    assert_eq!(s.project().channels.len(), 5);
    assert!(
        s.run_line("sample.add_stems 1 a.wav b.wav c.wav d.wav")
            .is_err(),
        "the files must exist"
    );
    run(&mut s, "channel.add_synth");
    assert!(
        s.run_line("sample.split_stems 6 builtin").is_err(),
        "only sampler channels split"
    );
    assert!(
        s.run_line("sample.split_stems 1 magic").is_err(),
        "unknown engines are refused"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn stem_jobs_split_in_the_background() {
    let dir = std::env::temp_dir().join(format!("musiccraft-stem-job-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:snare");
    s.run_line(&format!(
        "project.save \"{}\"",
        dir.join("p.mcproj").display()
    ))
    .unwrap();
    let job = crate::StemJob::start(s.project(), 1, crate::StemEngine::Builtin).unwrap();
    assert_eq!(job.name(), s.project().channels[0].name);
    let invocation = loop {
        if let Some(result) = job.poll(s.project()) {
            break result.unwrap().unwrap().0;
        }
        std::thread::yield_now();
    };
    assert!((job.progress() - 1.0).abs() < f32::EPSILON);
    assert_eq!(invocation.name, "sample.add_stems");
    s.run(&invocation).unwrap();
    assert_eq!(s.project().channels.len(), 5);

    let job = crate::StemJob::start(s.project(), 1, crate::StemEngine::Builtin).unwrap();
    job.cancel();
    let result = loop {
        if let Some(result) = job.poll(s.project()) {
            break result;
        }
        std::thread::yield_now();
    };
    // A short sound may finish before it hears the cancel.
    if result.unwrap().is_none() {
        assert!(
            !dir.join("stems/snare-2").exists(),
            "a cancelled split leaves no folder"
        );
    }
    assert!(crate::StemJob::start(s.project(), 9, crate::StemEngine::Builtin).is_err());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn zone_commands_check_and_undo() {
    let mut s = session();
    run(&mut s, "channel.add_zones\nchannel.add_zone 1 builtin:hat");
    assert!(s.run_line("channel.add_zone 1 builtin:hat 70 60").is_err());
    assert!(s.run_line("channel.set_zone 1 1 vel_low 0").is_err());
    assert!(
        s.run_line("channel.set_zone 1 2 root 40").is_err(),
        "no zone 2"
    );
    run(
        &mut s,
        "channel.set_zone 1 1 high 64\nchannel.remove_zone 1 1",
    );
    s.run_line("edit.undo").unwrap();
    s.run_line("edit.undo").unwrap();
    let musiccraft_project::Instrument::Zones { zones } = &s.project().channels[0].instrument
    else {
        panic!("not a zone sampler");
    };
    assert_eq!((zones.len(), zones[0].high), (1, 127));
    run(&mut s, "channel.add_synth");
    assert!(
        s.run_line("channel.add_zone 2 builtin:hat").is_err(),
        "not a zone sampler"
    );
}

#[test]
fn send_targets_follow_track_insertion_and_block_removal() {
    let mut s = session();
    run(
        &mut s,
        "mixer.add_effect 1 send
         mixer.set_effect_param 1 1 target 3",
    );
    assert!(s.run_line("mixer.set_effect_param 1 1 target 1").is_err());
    assert!(s.run_line("mixer.remove_track 3").is_err());
    s.run_line("mixer.remove_track 2").unwrap();
    assert_eq!(s.project().mixer[1].effects[0].values[0], 2.0);
}

#[test]
fn flac_export_is_valid_and_smaller_than_wav() {
    let dir = std::env::temp_dir().join(format!("musiccraft-flacx-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick\nstep.toggle 1 0\nstep.toggle 1 8",
    );
    s.run_line(&format!(
        "project.export_flac \"{}\"",
        dir.join("a.flac").display()
    ))
    .unwrap();
    s.run_line(&format!(
        "project.export_wav \"{}\"",
        dir.join("a.wav").display()
    ))
    .unwrap();
    let flac = std::fs::metadata(dir.join("a.flac")).unwrap().len();
    let wav = std::fs::metadata(dir.join("a.wav")).unwrap().len();
    assert!(flac < wav / 2, "flac {flac} bytes, wav {wav} bytes");
    if std::path::Path::new("/usr/bin/flac").exists() {
        let ok = std::process::Command::new("/usr/bin/flac")
            .args(["-s", "-t"])
            .arg(dir.join("a.flac"))
            .status()
            .unwrap();
        assert!(ok.success(), "reference decoder verifies the file");
    }
    assert!(s.run_line("project.export_flac x.flac 32").is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mp3_and_ogg_exports_write_their_formats() {
    let dir = std::env::temp_dir().join(format!("musiccraft-lossy-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick\nstep.toggle 1 0\nstep.toggle 1 8",
    );
    let file = |name: &str| dir.join(name).display().to_string();
    let wav = file("a.wav");
    s.run_line(&format!("project.export_wav \"{wav}\""))
        .unwrap();
    let wav = std::fs::read(&wav).unwrap();
    for (line, name) in [
        ("project.export_mp3 \"{}\"", "a.mp3"),
        ("project.export_mp3 \"{}\" 128 2 48000", "b.mp3"),
        ("project.export_ogg \"{}\"", "a.ogg"),
        ("project.export_ogg \"{}\" 3 1 96000", "b.ogg"),
    ] {
        s.run_line(&line.replace("{}", &file(name))).unwrap();
        let bytes = std::fs::read(file(name)).unwrap();
        let magic: &[u8] = if name.ends_with("mp3") {
            b"ID3"
        } else {
            b"OggS"
        };
        assert!(bytes.starts_with(magic), "{name}");
        assert!(bytes.len() < wav.len() / 3, "{name}: {} bytes", bytes.len());
    }
    for bad in [
        "project.export_mp3 x.mp3 100",
        "project.export_mp3 x.mp3 320 1 96000",
        "project.export_ogg x.ogg 11",
        "project.export_ogg x.ogg 5 0",
    ] {
        assert!(s.run_line(bad).is_err(), "{bad}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn every_synth_preset_plays_without_clipping() {
    use musiccraft_params::INSTRUMENT_PARAMS;
    use musiccraft_params::presets::SYNTH;
    for preset in SYNTH {
        let mut s = session();
        run(
            &mut s,
            "channel.add_synth\nnote.add_chord 1 0 8 C4 min7 110",
        );
        for (name, value) in preset.resolve(INSTRUMENT_PARAMS) {
            run(&mut s, &format!("channel.set_param 1 {name} {value}"));
        }
        let options = RenderOptions {
            sample_rate: 22_050,
            loops: 1,
            max_tail_seconds: 2.0,
        };
        let (left, right, _) = render(s.project(), options);
        let peak = left
            .iter()
            .chain(&right)
            .fold(0.0_f32, |p, x| p.max(x.abs()));
        let level = rms(&left[..left.len() / 2]);
        assert!(peak < 1.0, "{} clips: peak {peak}", preset.name);
        assert!(level > 0.01, "{} is too quiet: rms {level}", preset.name);
    }
}

#[test]
fn every_effect_preset_keeps_audio_sane() {
    use musiccraft_params::EFFECT_KINDS;
    use musiccraft_params::presets;
    for kind in EFFECT_KINDS {
        for preset in presets::effect(kind) {
            let mut s = session();
            run(
                &mut s,
                &format!(
                    "channel.add_synth\nnote.add_chord 1 0 8 C4 min7 110\nmixer.add_effect 0 {}",
                    kind.name()
                ),
            );
            for (name, value) in preset.resolve(kind.params()) {
                run(
                    &mut s,
                    &format!("mixer.set_effect_param 0 1 {name} {value}"),
                );
            }
            let options = RenderOptions {
                sample_rate: 22_050,
                loops: 1,
                max_tail_seconds: 1.0,
            };
            let (left, _, _) = render(s.project(), options);
            let peak = left.iter().fold(0.0_f32, |p, x| p.max(x.abs()));
            assert!(left.iter().all(|x| x.is_finite()), "{kind} {}", preset.name);
            // The vocoder is silent without a modulator (tested separately).
            let quiet_ok = kind == musiccraft_params::EffectKind::Vocoder;
            assert!(
                (quiet_ok || peak > 0.005) && peak < 1.5,
                "{kind} {}: peak {peak}",
                preset.name
            );
        }
    }
}

/// Frequency of a pure tone by counting rising zero crossings.
fn frequency(data: &[f32], sample_rate: f32) -> f32 {
    let crossings = data
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    crossings as f32 * sample_rate / data.len() as f32
}

/// A synth with one sine oscillator and full sustain, at 120 BPM (a step is
/// 1/8 s), plus `notes`.
fn sine_session(notes: &str) -> Session {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120
         channel.add_synth
         channel.set_param 1 wave1 0
         channel.set_param 1 osc_mix 0
         channel.set_param 1 sustain 1",
    );
    run(&mut s, notes);
    s
}

const SLIDE_RATE: u32 = 48_000;

fn render_slide(s: &Session) -> Vec<f32> {
    let options = RenderOptions {
        sample_rate: SLIDE_RATE,
        loops: 1,
        max_tail_seconds: 0.5,
    };
    render(s.project(), options).0
}

/// Frequency over `from..to` seconds.
fn hz_between(data: &[f32], from: f32, to: f32) -> f32 {
    let sr = SLIDE_RATE as f32;
    frequency(&data[(from * sr) as usize..(to * sr) as usize], sr)
}

fn key_hz(key: f32) -> f32 {
    440.0 * 2f32.powf((key - 69.0) / 12.0)
}

#[test]
fn slide_notes_glide_over_the_overlap_and_end_with_the_slid_note() {
    // C4 for 1 s; a C5 slide note from 0.5 s to 1.5 s overlaps it for 0.5 s.
    let s = sine_session("note.add 1 0 8 C4\nnote.add 1 4 8 C5 100 0 slide");
    let left = render_slide(&s);
    let before = hz_between(&left, 0.1, 0.45);
    assert!((before - key_hz(48.0)).abs() < 3.0, "before slide {before}");
    let middle = hz_between(&left, 0.7, 0.8);
    assert!(
        (middle - key_hz(54.0)).abs() < 8.0,
        "halfway through the overlap {middle} vs {}",
        key_hz(54.0)
    );
    let late = hz_between(&left, 0.93, 0.99);
    assert!(
        late > key_hz(58.5) && late < key_hz(60.0) + 3.0,
        "nearly there {late}"
    );
    // The slide does not lengthen the note: it ends at 1 s (plus its release).
    assert!(rms(&left[(1.3 * 48_000.0) as usize..(1.45 * 48_000.0) as usize]) < 1e-3);
}

#[test]
fn pitch_corrector_follows_a_channels_notes() {
    // Channel 1 sings A4 (220 Hz; middle C is C5) into insert 1; channel 2
    // plays C5 on a muted insert and is what the corrector follows.
    let mut s = sine_session(
        "note.add 1 0 16 A4
         channel.add_synth
         note.add 2 0 16 C5
         channel.route 1 1
         channel.route 2 2
         mixer.set_mute 2 on
         mixer.add_effect 1 pitch_correct
         mixer.set_effect_param 1 1 speed 0
         mixer.set_effect_param 1 1 follow 2",
    );
    let left = render_slide(&s);
    let hz = hz_between(&left, 0.8, 1.8);
    assert!((hz - key_hz(60.0)).abs() < 6.0, "pulled to C5: {hz}");
    // Moving channels keeps following the same one; removing it is refused.
    s.run_line("channel.move 2 1").unwrap();
    let follow = s.project().mixer[1].effects[0].values[6];
    assert_eq!(follow, 1.0);
    assert!(s.run_line("channel.remove 1").is_err());
}

#[test]
fn slide_notes_without_an_overlap_are_silent() {
    let s = sine_session(
        "note.add 1 0 4 C4 100 0 slide\nnote.add 1 8 4 C4\nnote.add 1 12 4 G4 100 0 slide",
    );
    let left = render_slide(&s);
    assert!(rms(&left[..48_000]) < 1e-4, "nothing before the first note");
    // The second slide note starts as the note ends: no overlap either.
    assert!(rms(&left[(1.8 * 48_000.0) as usize..]) < 1e-3);
}

#[test]
fn slide_notes_starting_with_their_note_bend_it() {
    let s = sine_session("note.add 1 0 8 C4\nnote.add 1 0 4 C5 100 0 slide");
    let left = render_slide(&s);
    let after = hz_between(&left, 0.6, 0.95);
    assert!((after - key_hz(60.0)).abs() < 4.0, "slid to C5: {after}");
}

#[test]
fn note_colour_groups_reach_the_engine() {
    let s = sine_session(
        "note.add 1 0 16 C4
         note.add 1 0 16 G4
         note.add 1 0 16 E3
         note.set_prop 1 0 E3 color 2
         note.add 1 2 2 A4 100 0 slide",
    );
    let seq = build_sequence(s.project());
    let colors: Vec<(u8, u8)> = seq
        .events()
        .iter()
        .map(|e| (e.key, e.shape.color))
        .collect();
    assert!(colors.contains(&(40, 1)) && colors.contains(&(55, 0)));
}

#[test]
fn arpeggiated_channels_play_their_chords_one_note_at_a_time() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         channel.add_sampler builtin:kick
         note.add_chord 1 0 4 C4 major
         step.toggle 2 0
         channel.set_param 1 arp_mode 1
         channel.set_param 1 arp_range 2",
    );
    let seq = build_sequence(s.project());
    let synth: Vec<(u64, u8)> = seq
        .events()
        .iter()
        .filter(|e| e.channel == 0)
        .map(|e| (e.start, e.key))
        .collect();
    let step = u64::from(TICKS_PER_STEP);
    assert_eq!(synth, [(0, 48), (step, 52), (2 * step, 55), (3 * step, 60)]);
    // Other channels are untouched.
    assert_eq!(seq.events().iter().filter(|e| e.channel == 1).count(), 1);
}

#[test]
fn drum_and_pluck_presets_play_without_clipping() {
    use musiccraft_params::INSTRUMENT_PARAMS;
    use musiccraft_params::presets::{DRUM, PLUCK};
    for (add, notes, list) in [
        ("channel.add_drum", "step.toggle 1 0\nstep.toggle 1 4", DRUM),
        (
            "channel.add_pluck",
            "note.add_chord 1 0 8 C4 minor 110",
            PLUCK,
        ),
    ] {
        for preset in list {
            let mut s = session();
            run(&mut s, &format!("{add}\n{notes}"));
            for (name, value) in preset.resolve(INSTRUMENT_PARAMS) {
                run(&mut s, &format!("channel.set_param 1 {name} {value}"));
            }
            let options = RenderOptions {
                sample_rate: 22_050,
                loops: 1,
                max_tail_seconds: 2.0,
            };
            let (left, right, _) = render(s.project(), options);
            let peak = left
                .iter()
                .chain(&right)
                .fold(0.0_f32, |p, x| p.max(x.abs()));
            assert!(peak < 1.0, "{} clips: {peak}", preset.name);
            assert!(peak > 0.05, "{} is too quiet: {peak}", preset.name);
        }
    }
}

#[test]
fn sliced_samples_play_back_slice_by_slice() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120\nchannel.add_sampler builtin:crash\nchannel.slice 1 4",
    );
    let notes = &s.project().current_pattern().notes;
    assert_eq!(notes.len(), 4);
    assert_eq!(notes[1].key, 61);
    assert_eq!(notes[1].start, 96);
    // A single slice plays only its quarter of the sample.
    let mut one = session();
    run(
        &mut one,
        "channel.add_sampler builtin:crash\nchannel.set_param 1 slices 4\nnote.add 1 0 16 C5",
    );
    let options = RenderOptions {
        sample_rate: 22_050,
        loops: 1,
        max_tail_seconds: 4.0,
    };
    let (whole, _, _) = render(
        &{
            let mut w = session();
            run(
                &mut w,
                "channel.add_sampler builtin:crash\nnote.add 1 0 16 C5",
            );
            w.project().clone()
        },
        options,
    );
    let (first, _, _) = render(one.project(), options);
    let sounding = |d: &[f32]| d.iter().rposition(|x| x.abs() > 1e-3).unwrap_or(0);
    assert!(
        sounding(&first) * 2 < sounding(&whole),
        "{} vs {}",
        sounding(&first),
        sounding(&whole)
    );
    // Undo restores the old notes and slice count.
    run(&mut s, "edit.undo");
    assert!(s.project().current_pattern().notes.is_empty());
    assert_eq!(
        s.project().channels[0]
            .params
            .get(musiccraft_params::inst::SLICES),
        1.0
    );
}

#[test]
fn note_properties_shape_the_sound() {
    let sr = 48_000.0;
    let options = RenderOptions {
        sample_rate: 48_000,
        loops: 1,
        max_tail_seconds: 1.0,
    };
    let sine = "project.set_tempo 120
         channel.add_synth
         channel.set_param 1 wave1 0
         channel.set_param 1 osc_mix 0
         channel.set_param 1 sustain 1";
    let render_with = |extra: &str| {
        let mut s = session();
        run(&mut s, &format!("{sine}\n{extra}"));
        render(s.project(), options).0
    };
    let hz = |key: f32| 440.0 * 2f32.powf((key - 69.0) / 12.0);

    // Muted notes do not play.
    let muted = render_with("note.add 1 0 4 A4\nnote.set_prop 1 0 A4 muted on");
    assert!(muted.iter().all(|s| s.abs() < 1e-6));

    // Fine pitch: +50 cents.
    let fine = render_with("note.add 1 0 8 A4\nnote.set_prop 1 0 A4 fine 50");
    let f = frequency(&fine[4_800..43_200], sr);
    // A4 is key 57 here (C4 = 48).
    assert!((f - hz(57.5)).abs() < 2.0, "fine {f}");

    // Portamento: glides from the previous (already ended) note.
    let porta = render_with("note.add 1 0 2 A3\nnote.add 1 8 8 A4\nnote.set_prop 1 8 A4 porta on");
    let start = (1.0 * sr) as usize;
    let early = frequency(&porta[start + 480..start + 2_880], sr);
    let late = frequency(&porta[start + 24_000..start + 43_000], sr);
    assert!(early < hz(54.0), "starts low: {early}");
    assert!((late - hz(57.0)).abs() < 3.0, "arrives: {late}");

    // Release velocity: a high one rings longer after the note ends.
    let tail = |release: u8| {
        let out = render_with(&format!(
            "channel.set_param 1 release 0.2\nnote.add 1 0 2 A4\nnote.set_prop 1 0 A4 release {release}"
        ));
        out.iter().rposition(|s| s.abs() > 1e-3).unwrap_or(0)
    };
    assert!(tail(127) > tail(0) + 4_800, "{} vs {}", tail(127), tail(0));
}

#[test]
fn fm_keys_and_additive_presets_play_without_clipping() {
    use musiccraft_params::INSTRUMENT_PARAMS;
    use musiccraft_params::presets::{ADDITIVE, FM, KEYS};
    for (add, list) in [
        ("channel.add_fm", FM),
        ("channel.add_keys", KEYS),
        ("channel.add_additive", ADDITIVE),
    ] {
        for preset in list {
            let mut s = session();
            run(&mut s, &format!("{add}\nnote.add_chord 1 0 8 C4 minor 110"));
            for (name, value) in preset.resolve(INSTRUMENT_PARAMS) {
                run(&mut s, &format!("channel.set_param 1 {name} {value}"));
            }
            let options = RenderOptions {
                sample_rate: 22_050,
                loops: 1,
                max_tail_seconds: 2.0,
            };
            let (left, right, _) = render(s.project(), options);
            let peak = left
                .iter()
                .chain(&right)
                .fold(0.0_f32, |p, x| p.max(x.abs()));
            assert!(peak < 1.0, "{} clips: {peak}", preset.name);
            assert!(peak > 0.05, "{} is too quiet: {peak}", preset.name);
        }
    }
}

#[test]
fn layers_double_sequenced_notes_and_kits_play_pads() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_kit
         channel.add_synth
         channel.add_layer
         channel.set_layer 3 1,2
         note.add 3 0 4 C5",
    );
    let (left, _, warnings) = render(s.project(), RenderOptions::default());
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(rms(&left[..4_410]) > 0.01);
}

#[test]
fn vocoder_follows_its_sidechain() {
    let level = |sidechain: u32| {
        let mut s = session();
        run(
            &mut s,
            &format!(
                "channel.add_synth\nnote.add 1 0 16 C4 110\nchannel.route 1 1\n\
                 mixer.route_to_master 1 off\nchannel.add_synth\n\
                 note.add 2 0 16 C3 110\nchannel.route 2 2\n\
                 mixer.add_effect 2 vocoder\nmixer.sidechain_to 1 2 on\nmixer.set_effect_param 2 1 sidechain {sidechain}"
            ),
        );
        let options = RenderOptions {
            sample_rate: 22_050,
            loops: 1,
            max_tail_seconds: 0.0,
        };
        let (left, _, _) = render(s.project(), options);
        rms(&left[left.len() / 4..left.len() / 2])
    };
    let keyed = level(1);
    let unkeyed = level(0);
    assert!(keyed > 0.01, "vocoded output: {keyed}");
    assert!(
        unkeyed < keyed * 0.1,
        "silent without a modulator: {unkeyed}"
    );
}

#[test]
fn convolution_reverb_loads_its_impulse() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:rim\nstep.set 1 1 on\nmixer.add_effect 0 convolution\n\
         mixer.set_effect_param 0 1 mix 1\nmixer.set_effect_file 0 1 ir:cathedral",
    );
    let options = RenderOptions {
        sample_rate: 22_050,
        loops: 1,
        max_tail_seconds: 3.0,
    };
    let (left, _, _) = render(s.project(), options);
    // A short click, so anything a second later is the cathedral's tail.
    let late = rms(&left[22_050..33_075]);
    assert!(late > 1e-4, "tail: {late}");
}

#[test]
fn midi_out_channels_reach_the_midi_queue_and_render_silent() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_midi_out 5
         step.set 1 1 on
         transport.play",
    );
    let (mut engine, mut handle) = engine_pair(44_100);
    let mut midi = handle.take_midi_out().unwrap();
    let mut bridge = Bridge::new(44_100);
    let report = bridge.sync(s.project(), &mut handle);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let mut out = vec![0.0; 2 * 22_050];
    engine.process(&mut out, 2);
    assert!(out.iter().all(|x| *x == 0.0), "MIDI Out makes no sound");
    let bytes: Vec<[u8; 3]> = std::iter::from_fn(|| midi.pop()).map(|e| e.bytes).collect();
    assert_eq!(bytes, vec![[0x94, 60, 100], [0x84, 60, 0]]);

    // Changing the MIDI channel rebuilds it; exports stay silent.
    run(&mut s, "channel.set_midi_channel 1 6");
    assert!(bridge.sync(s.project(), &mut handle).rebuilt_graph);
    let (left, right, _) = render(s.project(), RenderOptions::default());
    assert!(left.iter().chain(&right).all(|x| *x == 0.0));
}

#[test]
fn markers_pause_and_loop_song_playback_but_not_exports() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick
         step.toggle 1 0
         playlist.add_clip 1 1 0 4
         transport.set_mode song
         playlist.add_marker 2 Break
         playlist.set_marker_action 2 pause
         playlist.add_marker 1 Groove
         playlist.set_marker_action 1 song_loop",
    );
    let bar = f64::from(TICKS_PER_BAR);
    let seq = build_sequence(s.project());
    assert_eq!(seq.next_pause(0.0, 4.0 * bar), Some(2.0 * bar));
    assert_eq!(seq.loop_start(), u64::from(TICKS_PER_BAR));
    assert_eq!(seq.home(), 0);
    // Exports render the whole song once, ignoring pause markers.
    let full = crate::sequence::build_full_sequence(s.project());
    assert_eq!(full.next_pause(0.0, 4.0 * bar), None);
    // A loop region takes over from the markers while it is set.
    run(&mut s, "transport.set_loop 2 4");
    let seq = build_sequence(s.project());
    assert_eq!(seq.next_pause(0.0, 4.0 * bar), None);
    assert_eq!(seq.loop_start(), 2 * u64::from(TICKS_PER_BAR));
}

#[test]
fn pausing_the_transport_at_a_pause_marker_keeps_the_playhead_there() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 960
         channel.add_sampler builtin:kick
         step.toggle 1 0
         playlist.add_clip 1 1 0 4
         transport.set_mode song
         playlist.add_marker 1 Break
         playlist.set_marker_action 1 pause
         transport.play",
    );
    let marker = f64::from(TICKS_PER_BAR);
    let (mut engine, mut handle) = engine_pair(44_100);
    let mut bridge = Bridge::new(44_100);
    let _ = bridge.sync(s.project(), &mut handle);
    let mut out = vec![0.0; 2048];
    // Bar 1 starts 0.25 s in at 960 BPM: play past it.
    for _ in 0..16 {
        engine.process(&mut out, 2);
    }
    assert!(!engine.is_playing());
    assert_eq!(handle.meters().marker_pause(), (1, marker));
    // What the app does when it sees the pause: pause the transport.
    run(&mut s, "transport.pause");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!((engine.position_ticks() - marker).abs() < f64::EPSILON);
    // Space continues past the marker; Stop rewinds.
    run(&mut s, "transport.toggle");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(engine.is_playing() && engine.position_ticks() > marker);
    run(&mut s, "transport.stop");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(engine.position_ticks().abs() < f64::EPSILON);
}

#[test]
fn recording_options_reach_the_engine_only_while_armed() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120
         channel.add_sampler builtin:kick
         step.toggle 1 0
         record.toggle_count_in",
    );
    let (mut engine, mut handle) = engine_pair(48_000);
    let mut bridge = Bridge::new(48_000);
    let mut out = vec![0.0; 2 * 1024];
    // Not armed: Play starts at once.
    run(&mut s, "transport.play");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(engine.is_playing());
    run(&mut s, "transport.stop\nrecord.toggle");
    let _ = bridge.sync(s.project(), &mut handle);
    run(&mut s, "transport.play");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(!engine.is_playing() && handle.meters().is_counting_in());
    // One bar at 120 BPM is 2 s.
    for _ in 0..94 {
        engine.process(&mut out, 2);
    }
    assert!(engine.is_playing(), "the count-in is over");
    // Wait for input takes over from the count-in.
    run(&mut s, "transport.stop\nrecord.toggle_wait");
    let _ = bridge.sync(s.project(), &mut handle);
    run(&mut s, "transport.play");
    let _ = bridge.sync(s.project(), &mut handle);
    engine.process(&mut out, 2);
    assert!(handle.meters().is_waiting());
}

#[test]
fn recording_without_looping_stops_at_the_pattern_end() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 960
         channel.add_sampler builtin:kick
         record.toggle_loop
         transport.play",
    );
    let (mut engine, mut handle) = engine_pair(48_000);
    let mut bridge = Bridge::new(48_000);
    let mut out = vec![0.0; 2 * 1024];
    let _ = bridge.sync(s.project(), &mut handle);
    // A bar at 960 BPM is 0.25 s: 12 000 frames.
    for _ in 0..16 {
        engine.process(&mut out, 2);
    }
    assert!(engine.is_playing(), "not armed: it loops");
    run(&mut s, "record.toggle");
    let _ = bridge.sync(s.project(), &mut handle);
    for _ in 0..16 {
        engine.process(&mut out, 2);
    }
    assert!(!engine.is_playing());
    assert_eq!(handle.meters().end_stops(), 1);
}

#[test]
fn soloed_playlist_tracks_silence_the_others() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick
         step.toggle 1 0
         pattern.add
         channel.add_sampler builtin:snare
         step.toggle 2 0
         playlist.add_clip 1 1 0
         playlist.add_clip 2 2 0
         transport.set_mode song",
    );
    let channels = |s: &Session| -> Vec<u16> {
        build_sequence(s.project())
            .events()
            .iter()
            .map(|e| e.channel)
            .collect()
    };
    assert_eq!(channels(&s), [0, 1]);
    run(&mut s, "playlist.set_track_solo 2 on");
    assert_eq!(channels(&s), [1], "only the soloed track");
    run(&mut s, "playlist.set_track_mute 2 on");
    assert!(channels(&s).is_empty(), "muted wins over solo");
    run(
        &mut s,
        "playlist.set_track_mute 2 off\nplaylist.toggle_track_solo 2",
    );
    assert_eq!(channels(&s), [0, 1]);
}

#[test]
fn a_sliced_pattern_clip_plays_on_from_its_offset() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_sampler builtin:kick
         step.toggle 1 0
         step.toggle 1 4
         step.toggle 1 8
         step.toggle 1 12
         playlist.add_clip 1 1 0 2
         playlist.slice_clip 1 1.375
         transport.set_mode song",
    );
    let starts: Vec<u64> = build_sequence(s.project())
        .events()
        .iter()
        .map(|e| e.start)
        .collect();
    // Every beat over two bars, unchanged by the cut at bar 1 + 1.5 beats.
    assert_eq!(starts, (0..8).map(|b| b * 96).collect::<Vec<_>>());
}

#[test]
fn sliced_audio_clips_start_part_way_into_the_sample() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120
         channel.add_sampler builtin:kick
         playlist.add_audio_clip 1 1 0 1
         playlist.slice_clip 1 0.5
         transport.set_mode song",
    );
    let seq = build_sequence(s.project());
    let offsets: Vec<(u64, f32)> = seq
        .events()
        .iter()
        .map(|e| (e.start, e.shape.start))
        .collect();
    // Half a bar at 120 BPM is one second.
    assert_eq!(offsets, [(0, 0.0), (192, 1.0)]);
}

#[test]
fn reversed_and_faded_samples() {
    let dir = std::env::temp_dir().join(format!("musiccraft-reverse-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ramp = dir.join("ramp.wav");
    // A rising ramp, 0.5 s at 48 kHz.
    let data: Vec<f32> = (0..24_000).map(|i| i as f32 / 24_000.0).collect();
    musiccraft_samples::write_wav(
        &ramp,
        &data,
        &data,
        48_000,
        musiccraft_samples::WavFormat::Float32,
    )
    .unwrap();
    let mut s = session();
    run(
        &mut s,
        &format!(
            "project.set_tempo 120
             channel.add_sampler \"{}\"
             channel.set_param 1 one_shot 1
             channel.set_pan 1 -1
             step.toggle 1 0",
            ramp.display()
        ),
    );
    let options = RenderOptions {
        sample_rate: 48_000,
        loops: 1,
        max_tail_seconds: 0.0,
    };
    let left = |s: &Session| render(s.project(), options).0;
    let forward = left(&s);
    assert!(forward[100] < forward[20_000], "rises");
    run(&mut s, "channel.set_param 1 reverse 1");
    let backward = left(&s);
    assert!(backward[100] > backward[20_000], "falls when reversed");
    run(
        &mut s,
        "channel.set_param 1 reverse 0\nchannel.set_param 1 fade_out 0.25",
    );
    let faded = left(&s);
    // Halfway through the fade-out the ramp is cut to about half.
    let at = 18_000;
    assert!(
        (faded[at] / forward[at] - 0.5).abs() < 0.02,
        "{}",
        faded[at] / forward[at]
    );
    run(
        &mut s,
        "channel.set_param 1 fade_out 0\nchannel.set_param 1 fade_in 0.1",
    );
    let faded_in = left(&s);
    assert!((faded_in[2_400] / forward[2_400] - 0.5).abs() < 0.02);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn consolidating_a_clip_renders_it_to_an_audio_clip() {
    let dir = std::env::temp_dir().join(format!("musiccraft-consolidate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    run(
        &mut s,
        &format!(
            "channel.add_synth
             note.add 1 0 4 C5
             playlist.add_clip 1 1 2 1
             project.save \"{}\"",
            dir.join("song.mcproj").display()
        ),
    );
    run(&mut s, "playlist.consolidate 1 2");
    let project = s.project();
    assert_eq!(project.channels.len(), 2);
    let clip = project.playlist.clips[0];
    assert_eq!(
        clip.source,
        musiccraft_project::ClipSource::Audio(project.channels[1].id)
    );
    assert_eq!((clip.start, clip.length), (768, 384));
    let wav = dir.join("consolidated/Pattern-1-1.wav");
    assert!(wav.exists());
    // The render holds the note: loud at the start, the length of the clip.
    let buffer = musiccraft_samples::load_wav(&wav).unwrap();
    let expected = 384.0 / 96.0 * 60.0 / 130.0;
    assert!((buffer.duration_seconds() - expected).abs() < 1e-3);
    assert!((0..4800).any(|i| buffer.read(i as f64).0.abs() > 0.05));
    run(&mut s, "edit.undo");
    assert_eq!(s.project().channels.len(), 1);
    assert!(s.project().playlist.clips[0].pattern().is_some());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn midi_export_round_trips_through_import() {
    let dir = std::env::temp_dir().join(format!("musiccraft-midi-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("song.mid");
    let mut s = session();
    run(
        &mut s,
        &format!(
            "channel.add_synth
             note.add 1 0 2 D5 100
             note.add 1 2 1 E5 64
             note.add 1 2 3 G5 127
             channel.add_sampler builtin:kick
             step.toggle 2 0
             playlist.add_audio_clip 2 1 0 1
             project.export_midi \"{}\"",
            path.display()
        ),
    );
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..4], b"MThd");
    // A tempo track plus one track per channel with notes.
    assert_eq!(u16::from_be_bytes([bytes[10], bytes[11]]), 3);
    run(
        &mut s,
        &format!("pattern.add\nnote.import_midi 1 \"{}\" @2", path.display()),
    );
    let notes = |p: usize| -> Vec<(u32, u32, u8, u8)> {
        s.project().patterns[p]
            .notes
            .iter()
            .map(|n| (n.start, n.length, n.key, n.velocity))
            .collect()
    };
    // Channel 1's notes come back, plus the kick from the other track (the
    // importer merges every track into one channel).
    let back = notes(1);
    for note in &notes(0) {
        assert!(back.contains(note), "{note:?} in {back:?}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn time_shift_gate_and_echo_change_what_a_channel_plays() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         note.add 1 4 2 C5 100
         channel.add_synth
         note.add 2 4 2 C5 100",
    );
    let rows = |s: &Session| -> Vec<(u16, u64, u64, u8, u8)> {
        build_sequence(s.project())
            .events()
            .iter()
            .map(|e| {
                (
                    e.channel,
                    e.start,
                    e.length,
                    e.key,
                    (e.velocity * 127.0).round() as u8,
                )
            })
            .collect()
    };
    run(
        &mut s,
        "channel.set_param 1 time_shift -12\nchannel.set_param 1 gate 6",
    );
    assert_eq!(rows(&s), [(0, 84, 6, 60, 100), (1, 96, 48, 60, 100)]);
    run(
        &mut s,
        "channel.set_param 1 time_shift 0
         channel.set_param 1 gate 0
         channel.set_param 1 echo_count 2
         channel.set_param 1 echo_time 24
         channel.set_param 1 echo_feed 0.5
         channel.set_param 1 echo_pitch 12",
    );
    let echoes: Vec<_> = rows(&s).into_iter().filter(|r| r.0 == 0).collect();
    assert_eq!(
        echoes,
        [
            (0, 96, 48, 60, 100),
            (0, 120, 48, 72, 50),
            (0, 144, 48, 84, 25)
        ]
    );
}

#[test]
fn master_pitch_transposes_built_in_instruments() {
    // A5 is key 69 here: 440 Hz.
    let mut s = sine_session("note.add 1 0 8 A5");
    let left = render_slide(&s);
    assert!((hz_between(&left, 0.1, 0.9) - 440.0).abs() < 4.0);
    run(&mut s, "project.set_master_pitch -1200");
    let left = render_slide(&s);
    assert!((hz_between(&left, 0.1, 0.9) - 220.0).abs() < 4.0);
}

#[test]
fn pattern_automation_plays_wherever_the_pattern_plays() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         pattern.record_event channel:1:volume 4",
    );
    // One lane of one point at tick 96 in pattern mode.
    let seq = build_sequence(s.project());
    assert_eq!(seq.automation().len(), 1);
    assert_eq!(seq.automation()[0].points.len(), 1);
    assert_eq!(seq.automation()[0].points[0].0, 96);
    // In song mode once per repeat of the pattern inside each clip; a clip
    // sliced past the point skips that repeat's point.
    run(
        &mut s,
        "playlist.add_clip 1 1 2 2
         transport.set_mode song",
    );
    let ticks: Vec<u64> = build_sequence(s.project())
        .automation()
        .iter()
        .flat_map(|l| l.points.iter().map(|p| p.0))
        .collect();
    assert_eq!(ticks, [768 + 96, 1152 + 96]);
    run(
        &mut s,
        "playlist.slice_clip 1 2.5\nplaylist.remove_clip 1 2",
    );
    let ticks: Vec<u64> = build_sequence(s.project())
        .automation()
        .iter()
        .flat_map(|l| l.points.iter().map(|p| p.0))
        .collect();
    assert_eq!(
        ticks,
        [1152 + 96],
        "the sliced remainder plays the second repeat"
    );
}

#[test]
fn rack_branches_process_in_parallel_and_sync_without_rebuilds() {
    let level = |script: &str| {
        let mut s = session();
        run(
            &mut s,
            &format!(
                "channel.add_synth Lead
                 note.add 1 0 16 C5
                 mixer.add_effect 0 rack
                 {script}"
            ),
        );
        let options = RenderOptions {
            max_tail_seconds: 0.0,
            ..RenderOptions::default()
        };
        rms(&render(s.project(), options).0[4_000..20_000])
    };
    let dry = level("");
    assert!(dry > 0.01);
    assert!(
        level("mixer.set_effect_param 0 1 dry 0") < 1e-6,
        "no branches"
    );
    let doubled = level(
        "mixer.rack_add 0 1 1 eq
         mixer.rack_add 0 1 2 eq
         mixer.set_effect_param 0 1 dry 0",
    );
    assert!((doubled / dry - 2.0).abs() < 0.05, "{doubled} vs {dry}");
    let filtered = level(
        "mixer.rack_add 0 1 1 filter
         mixer.rack_param 0 1 1 1 cutoff 100
         mixer.set_effect_param 0 1 dry 0",
    );
    assert!(filtered < dry * 0.3, "the branch's lowpass ran: {filtered}");

    let mut s = session();
    run(
        &mut s,
        "mixer.add_effect 0 rack\nmixer.rack_add 0 1 1 delay",
    );
    let (_engine, mut handle) = engine_pair(44_100);
    let mut bridge = Bridge::new(44_100);
    assert!(bridge.sync(s.project(), &mut handle).rebuilt_graph);
    run(&mut s, "mixer.rack_param 0 1 1 1 feedback 0.2");
    let param = bridge.sync(s.project(), &mut handle);
    assert!(!param.rebuilt_graph);
    assert_eq!(param.messages, 1);
    run(&mut s, "mixer.rack_add 0 1 1 chorus");
    assert!(bridge.sync(s.project(), &mut handle).rebuilt_graph);
}

#[test]
fn signatures_reach_the_sequence_and_midi_files() {
    let dir = std::env::temp_dir().join(format!("musiccraft-sig-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sig.mid");
    let mut s = session();
    run(
        &mut s,
        &format!(
            "channel.add_synth
             pattern.set_time_signature 1 7/8
             note.add 1 0 2 C5 100
             project.export_midi \"{}\"",
            path.display()
        ),
    );
    let seven_eight: musiccraft_project::TimeSig = "7/8".parse().unwrap();
    assert_eq!(build_sequence(s.project()).meter().first(), seven_eight);
    let bytes = std::fs::read(&path).unwrap();
    let event = [0xff, 0x58, 4, 7, 3, 12, 8];
    assert!(bytes.windows(7).any(|w| w == event), "7/8 is written");
    // Importing it into a 4/4 pattern brings the signature along; undo
    // takes it back.
    run(
        &mut s,
        &format!("pattern.add\nnote.import_midi 1 \"{}\" @2", path.display()),
    );
    assert_eq!(s.project().patterns[1].time_signature, seven_eight);
    run(&mut s, "edit.undo");
    assert!(s.project().patterns[1].time_signature.is_common());
    assert!(s.project().patterns[1].notes.is_empty());

    // Song mode: the song meter, with its change written after the start.
    run(
        &mut s,
        "transport.set_mode song
         playlist.add_clip 1 1 0
         playlist.set_time_signature 1 3/4",
    );
    let sequence = build_sequence(s.project());
    assert_eq!(sequence.meter().sig_at(384), "3/4".parse().unwrap());
    let path = dir.join("song.mid");
    run(
        &mut s,
        &format!("project.export_midi \"{}\"", path.display()),
    );
    let bytes = std::fs::read(&path).unwrap();
    // Delta 384 (0x83 0x00), then 3/4.
    let change = [0x83, 0x00, 0xff, 0x58, 4, 3, 2, 24, 8];
    assert!(
        bytes.windows(9).any(|w| w == change),
        "the change is written"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn track_eq_and_slot_mix_change_the_sound() {
    let level = |lines: &str| {
        let mut s = session();
        run(
            &mut s,
            &format!(
                "channel.add_synth
                 note.add 1 0 8 C3 100
                 channel.route 1 1
                 {lines}"
            ),
        );
        let (l, r, _) = render(s.project(), RenderOptions::default());
        rms(&l) + rms(&r)
    };
    let plain = level("");
    let cut = level("mixer.set_eq 1 low -18 1000\nmixer.set_eq 1 mid -18 1000");
    assert!(cut < plain * 0.7, "EQ cut {cut} vs {plain}");
    // A gain effect at a quarter mix moves the level a quarter of the way.
    let full = level("mixer.add_effect 1 distortion");
    let quarter = level("mixer.add_effect 1 distortion\nmixer.set_effect_mix 1 1 0.25");
    let off = level("mixer.add_effect 1 distortion\nmixer.set_effect_mix 1 1 0");
    assert!(
        (off - plain).abs() < plain * 0.01,
        "mix 0 is dry: {off} vs {plain}"
    );
    assert!(
        (quarter - plain).abs() < (full - plain).abs(),
        "{plain} {quarter} {full}"
    );
}

#[test]
fn shaped_automation_segments_reach_the_engine_lane() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         automation.create channel:1:pan
         automation.set_points 1 [[0,0.0],[384,1.0]]
         playlist.add_automation_clip 1 1 0 1
         transport.set_mode song",
    );
    let straight = build_sequence(s.project()).automation()[0].points.len();
    assert_eq!(straight, 2, "a straight segment needs only its ends");
    run(&mut s, "automation.set_curve 1 0 hold");
    let seq = build_sequence(s.project());
    let lane = &seq.automation()[0];
    let before_end = lane.value_at(382.0).unwrap();
    let at_end = lane.value_at(384.0).unwrap();
    assert!(
        (before_end - -1.0).abs() < 1e-3,
        "held at the start value: {before_end}"
    );
    assert!((at_end - 1.0).abs() < 1e-3, "jumps on the point: {at_end}");
    run(&mut s, "automation.set_curve 1 0 single 1");
    let seq = build_sequence(s.project());
    let lane = &seq.automation()[0];
    assert!(lane.points.len() > 10, "a bent segment is sampled");
    assert!(lane.value_at(192.0).unwrap() < -0.8, "slow start");
}

#[test]
fn automation_range_and_switch_shape_the_engine_lane() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         automation.create channel:1:pan
         automation.set_points 1 [[0,0.0],[384,1.0]]
         playlist.add_automation_clip 1 1 0 1
         transport.set_mode song
         automation.set_range 1 0.5 1",
    );
    // Pan plays from the centre (position 0.5) to hard right.
    let seq = build_sequence(s.project());
    let lane = &seq.automation()[0];
    assert!(lane.value_at(0.0).unwrap().abs() < 1e-3);
    assert!((lane.value_at(384.0).unwrap() - 1.0).abs() < 1e-3);
    run(&mut s, "automation.set_enabled 1 off");
    assert!(build_sequence(s.project()).automation().is_empty());
}

#[test]
fn a_loop_region_does_not_change_an_export() {
    let render_song = |lines: &str| {
        let mut s = session();
        run(
            &mut s,
            &format!(
                "channel.add_synth
                 note.add 1 0 16 C5 100
                 playlist.add_clip 1 1 0
                 transport.set_mode song
                 {lines}"
            ),
        );
        render(s.project(), RenderOptions::default()).0
    };
    let plain = render_song("");
    // A loop region past the song's end (as FL projects can carry).
    let looped = render_song("transport.set_loop 10 14");
    assert_eq!(plain.len(), looped.len());
    let diff = plain
        .iter()
        .zip(&looped)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(diff < 1e-4, "exports differ by {diff}");
}

#[test]
fn an_empty_sampler_is_silent_without_a_warning() {
    let dir = std::env::temp_dir().join(format!("musiccraft-empty-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick");
    let mut project = s.project().clone();
    project.channels[0].instrument = musiccraft_project::Instrument::Sampler {
        sample: String::new(),
    };
    // Saved next to a folder, an empty path must not resolve to that folder.
    project.set_path(Some(dir.join("p.mcp.json")));
    let (_, _, warnings) = render(&project, RenderOptions::default());
    assert!(warnings.is_empty(), "{warnings:?}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_windows_sample_path_plays_through_its_translation() {
    let dir = std::env::temp_dir().join(format!("musiccraft-winpath-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("Packs/Drums")).unwrap();
    let tone: Vec<f32> = (0..12_000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
    musiccraft_samples::write_wav(
        &dir.join("Packs/Drums/tone.wav"),
        &tone,
        &tone,
        48_000,
        musiccraft_samples::WavFormat::Float32,
    )
    .unwrap();
    musiccraft_paths::configure(musiccraft_paths::PathConfig {
        fl_factory: vec![dir.clone()],
        ..musiccraft_paths::PathConfig::default()
    });
    let mut s = session();
    run(&mut s, "channel.add_sampler builtin:kick\nstep.toggle 1 0");
    let mut project = s.project().clone();
    project.channels[0].instrument = musiccraft_project::Instrument::Sampler {
        sample: r"%FLStudioFactoryData%\packs\DRUMS\tone.wav".to_owned(),
    };
    let (left, _, warnings) = render(&project, RenderOptions::default());
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(rms(&left[..12_000]) > 0.1, "the translated sample plays");
    // The project keeps the path as written.
    assert!(matches!(
        &project.channels[0].instrument,
        musiccraft_project::Instrument::Sampler { sample } if sample.starts_with('%')
    ));
    musiccraft_paths::configure(musiccraft_paths::PathConfig::default());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_antiserum_channel_renders_its_notes() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120
         channel.add_antiserum Deep Sub
         note.add 1 0 4 C3",
    );
    let (left, right, warnings) = render(
        s.project(),
        RenderOptions {
            sample_rate: 48_000,
            loops: 1,
            max_tail_seconds: 1.0,
        },
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(left.iter().chain(&right).all(|x| x.is_finite()));
    // The note lasts one beat (0.5 s); it sounds then and is quiet later.
    assert!(rms(&left[2_400..24_000]) > 0.01);
    assert!(rms(&left[48_000..60_000]) < 0.1 * rms(&left[2_400..24_000]));
}

#[test]
fn antiserum_edits_reach_the_running_synth() {
    let mut s = session();
    run(&mut s, "channel.add_antiserum Glass Reese");
    let (mut engine, mut handle) = engine_pair(48_000);
    let mut bridge = Bridge::new(48_000);
    let first = bridge.sync(s.project(), &mut handle);
    assert!(first.rebuilt_graph && first.warnings.is_empty());
    let mut out = vec![0.0; 1_024];
    let mut step = |s: &Session, bridge: &mut Bridge| {
        engine.process(&mut out, 2);
        bridge.sync(s.project(), &mut handle)
    };
    assert_eq!(step(&s, &mut bridge).messages, 0, "idle");
    run(&mut s, "channel.set_param 1 filter1_cutoff 900");
    let param = step(&s, &mut bridge);
    assert_eq!(param.messages, 1);
    assert!(!param.rebuilt_graph, "no rebuild, notes keep sounding");
    run(&mut s, "antiserum.set_table 1 1 factory:pwm");
    assert_eq!(step(&s, &mut bridge).messages, 1);
    run(&mut s, "antiserum.set_lfo_shape 1 2 0:0:0,0.25:1:0");
    assert_eq!(step(&s, &mut bridge).messages, 1);
    run(&mut s, "antiserum.set_step 1 3 off 0 1 0.5");
    assert_eq!(step(&s, &mut bridge).messages, 1);
    // A preset changes many values but still no rebuild.
    run(&mut s, "antiserum.load_preset 1 Choir Pad");
    let preset = step(&s, &mut bridge);
    assert!(preset.messages > 10 && !preset.rebuilt_graph);
    // Removing the channel frees its slot (and rebuilds the graph).
    run(&mut s, "channel.remove 1");
    assert!(step(&s, &mut bridge).rebuilt_graph);
}

#[test]
fn antiserum_automation_targets_the_synth() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_antiserum
         automation.create channel:1:filter1_cutoff
         automation.set_points 1 [[0,0.25],[1536,1.0]]
         playlist.add_automation_clip 1 1 0 4
         transport.set_mode song",
    );
    let sequence = build_sequence(s.project());
    let targets: Vec<_> = sequence.automation().iter().map(|l| l.target).collect();
    let index = musiccraft_antiserum::index_of("filter1_cutoff").unwrap() as u32;
    assert!(
        targets.contains(&musiccraft_engine::AutomationTarget::ExternalParam(
            0, index
        )),
        "{targets:?}"
    );
}

#[test]
fn antiserum_sample_oscillators_render_and_swap_samples() {
    let mut s = session();
    run(
        &mut s,
        "project.set_tempo 120
         channel.add_antiserum
         channel.set_param 1 osc1_type 3
         channel.set_param 1 osc1_grain_spread 0
         antiserum.set_sample 1 1 builtin:crash
         note.add 1 0 4 C4",
    );
    let (left, _, warnings) = render(
        s.project(),
        RenderOptions {
            sample_rate: 48_000,
            loops: 1,
            max_tail_seconds: 0.5,
        },
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(
        rms(&left[2_400..24_000]) > 0.01,
        "the granular crash sounds"
    );
    // A new sample is swapped into the running synth: one message.
    let (mut engine, mut handle) = engine_pair(48_000);
    let mut bridge = Bridge::new(48_000);
    let _ = bridge.sync(s.project(), &mut handle);
    let mut out = vec![0.0; 1_024];
    engine.process(&mut out, 2);
    let _ = bridge.sync(s.project(), &mut handle);
    run(&mut s, "antiserum.set_sample 1 1 builtin:snare");
    let swap = bridge.sync(s.project(), &mut handle);
    assert_eq!(swap.messages, 1);
    assert!(!swap.rebuilt_graph);
    // A missing file warns once and plays silence.
    run(&mut s, "antiserum.add_zone 1 2 builtin:kick");
    let mut project = s.project().clone();
    if let Some(patch) = project.channels[0].instrument.patch_mut() {
        let mut source = patch.source(1);
        source.zones[0].sample = "/no/such/zone.wav".to_owned();
        patch.set_source(1, source);
    }
    engine.process(&mut out, 2);
    let report = bridge.sync(&project, &mut handle);
    assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
}

#[test]
fn the_channel_pitch_knob_transposes_antiserum() {
    let pitch_of = |semitones: i32| {
        let mut s = session();
        run(
            &mut s,
            &format!(
                "project.set_tempo 120
                 channel.add_antiserum
                 channel.set_param 1 osc1_unison 1
                 channel.set_param 1 pitch {semitones}
                 note.add 1 0 8 A4"
            ),
        );
        let (left, _, _) = render(
            s.project(),
            RenderOptions {
                sample_rate: 48_000,
                loops: 1,
                max_tail_seconds: 0.1,
            },
        );
        frequency(&left[4_800..20_000], 48_000.0)
    };
    let plain = pitch_of(0);
    let up = pitch_of(12);
    assert!(plain > 100.0, "it sounds ({plain} Hz)");
    assert!(
        (up / plain - 2.0).abs() < 0.05,
        "an octave up: {plain} -> {up}"
    );
}

#[test]
fn a_soloed_channel_silences_the_others_without_muting_them() {
    let mut s = session();
    run(
        &mut s,
        "channel.add_synth
         channel.add_synth
         note.add 1 0 16 A5
         note.add 2 0 16 A5
         channel.solo 2
         channel.set_mute 2 on",
    );
    let options = RenderOptions {
        max_tail_seconds: 0.0,
        ..RenderOptions::default()
    };
    // Channel 2 is soloed but muted, so nothing plays; channel 1 is silenced
    // by the solo, though not muted itself.
    assert!(!s.project().channels[0].mute);
    let (out, _, _) = render(s.project(), options);
    assert!(rms(&out) < 1e-6);
    run(&mut s, "channel.set_mute 2 off");
    let (soloed, _, _) = render(s.project(), options);
    run(&mut s, "channel.solo 2");
    let (both, _, _) = render(s.project(), options);
    assert!(rms(&soloed) > 0.01 && rms(&both) > rms(&soloed) * 1.2);
}

#[test]
fn preset_previews_play_a_held_note_and_are_kept() {
    let mut bridge = Bridge::new(44_100);
    let project = Project::default();
    let first = bridge.preset_preview("Glass Reese", &project).unwrap();
    assert!(first.is_none(), "rendered on its own thread");
    assert!(bridge.rendering_preset_preview());
    let started = std::time::Instant::now();
    let preview = loop {
        if let Some(buffer) = bridge.finished_preset_preview() {
            break buffer;
        }
        assert!(started.elapsed().as_secs() < 30, "the preview finishes");
        std::thread::yield_now();
    };
    assert!(!bridge.rendering_preset_preview());
    let seconds = preview.duration_seconds();
    assert!((1.5..=4.1).contains(&seconds), "{seconds} s");
    assert!(preview.peak() > 0.01, "audible");
    let again = bridge.preset_preview("glass reese", &project).unwrap();
    assert!(
        again.is_some_and(|b| std::sync::Arc::ptr_eq(&preview, &b)),
        "the second click plays at once"
    );
    assert!(bridge.preset_preview("No Such Preset", &project).is_err());
}
