use std::f32::consts::FRAC_1_SQRT_2;
use std::sync::Arc;

use musiccraft_dsp::{Effect, SampleBuffer};
use musiccraft_params::{EffectKind, PPQ, inst};

use super::*;

const SR: u32 = 48_000;
/// Frames per beat at 120 BPM and 48 kHz.
const BEAT: usize = 24_000;

/// Unity-gain channel settings: a centred channel is -3 dB (circular pan
/// law), so the volume makes up for it.
fn impulse_params() -> ChannelParams {
    let mut params = ChannelParams {
        volume: std::f32::consts::SQRT_2,
        ..ChannelParams::default()
    };
    params.instrument[inst::ATTACK] = 0.0;
    params.instrument[inst::DECAY] = 0.0;
    params.instrument[inst::SUSTAIN] = 1.0;
    params
}

/// An engine at 120 BPM with one sampler channel playing a single-frame impulse.
fn impulse_engine() -> (Engine, EngineHandle) {
    let (mut engine, handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let impulse = Arc::new(SampleBuffer::mono(vec![1.0], f64::from(SR)));
    graph.add_channel(Instrument::Sampler(impulse), impulse_params());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    (engine, handle)
}

fn note(start_beats: u64, channel: u16) -> NoteEvent {
    NoteEvent {
        shape: crate::NoteShape::default(),
        slide: false,
        start: start_beats * u64::from(PPQ),
        length: u64::from(PPQ) / 4,
        channel,
        key: 60,
        velocity: 1.0,
        pan: 0.0,
    }
}

fn render_mono(engine: &mut Engine, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames];
    // Use an odd device buffer size to exercise block splitting.
    for chunk in out.chunks_mut(333) {
        engine.process(chunk, 1);
    }
    out
}

fn render_stereo(engine: &mut Engine, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0; frames * 2];
    engine.process(&mut out, 2);
    out
}

fn nonzero_frames(samples: &[f32]) -> Vec<usize> {
    samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.abs() > 1e-6)
        .map(|(i, _)| i)
        .collect()
}

#[test]
fn stopped_engine_is_silent_and_does_not_advance() {
    let (mut engine, _handle) = impulse_engine();
    let mut buffer = vec![1.0_f32; 512];
    engine.process(&mut buffer, 2);
    assert!(buffer.iter().all(|s| *s == 0.0));
    assert_eq!(engine.frames_played(), 0);
    assert!(engine.position_beats().abs() < f64::EPSILON);
}

#[test]
fn one_second_at_120_bpm_is_two_beats() {
    let (mut engine, _handle) = impulse_engine();
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![],
        16 * u64::from(PPQ),
    ))));
    engine.apply(EngineMessage::Play);
    render_stereo(&mut engine, SR as usize);
    assert_eq!(engine.frames_played(), u64::from(SR));
    assert!((engine.position_beats() - 2.0).abs() < 1e-9);
}

#[test]
fn tempo_change_does_not_make_position_jump() {
    let (mut engine, _handle) = impulse_engine();
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![],
        16 * u64::from(PPQ),
    ))));
    engine.apply(EngineMessage::Play);
    render_stereo(&mut engine, SR as usize);
    let before = engine.position_beats();
    engine.apply(EngineMessage::SetTempo(60.0));
    assert!((engine.position_beats() - before).abs() < f64::EPSILON);
    render_stereo(&mut engine, SR as usize);
    assert!((engine.position_beats() - (before + 1.0)).abs() < 1e-9);
}

#[test]
fn stop_rewinds_and_pause_keeps_position() {
    let (mut engine, _handle) = impulse_engine();
    engine.apply(EngineMessage::Play);
    render_stereo(&mut engine, 1_000);
    engine.apply(EngineMessage::Pause);
    let paused = engine.position_ticks();
    assert!(paused > 0.0);
    render_stereo(&mut engine, 1_000);
    assert!((engine.position_ticks() - paused).abs() < f64::EPSILON);
    engine.apply(EngineMessage::Stop);
    assert!(engine.position_ticks().abs() < f64::EPSILON);
}

#[test]
fn invalid_tempo_messages_are_ignored() {
    let (mut engine, _handle) = impulse_engine();
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        engine.apply(EngineMessage::SetTempo(bad));
    }
    assert!((engine.tempo_bpm() - 120.0).abs() < f64::EPSILON);
}

#[test]
fn zero_channels_does_not_panic() {
    let (mut engine, _handle) = impulse_engine();
    engine.apply(EngineMessage::Play);
    let mut buffer = vec![0.0_f32; 64];
    engine.process(&mut buffer, 0);
    assert_eq!(engine.frames_played(), 0);
}

#[test]
fn notes_start_on_the_exact_frame() {
    let (mut engine, _handle) = impulse_engine();
    let seq = Sequence::new(vec![note(1, 0), note(3, 0)], 4 * u64::from(PPQ));
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 4 * BEAT);
    assert_eq!(nonzero_frames(&out), [BEAT, 3 * BEAT]);
    assert!((out[BEAT] - 1.0).abs() < 1e-6);
}

#[test]
fn sequence_loops_at_its_length() {
    let (mut engine, _handle) = impulse_engine();
    let seq = Sequence::new(vec![note(0, 0)], u64::from(PPQ));
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 3 * BEAT + 10);
    assert_eq!(nonzero_frames(&out), [0, BEAT, 2 * BEAT, 3 * BEAT]);
}

#[test]
fn replaced_values_come_back_as_garbage() {
    let (mut engine, mut handle) = impulse_engine();
    handle
        .send(EngineMessage::SetGraph(Box::default()))
        .unwrap();
    handle
        .send(EngineMessage::SetSequence(Box::default()))
        .unwrap();
    handle
        .send(EngineMessage::SetSequence(Box::default()))
        .unwrap();
    render_stereo(&mut engine, 16);
    // Old graph from impulse_engine + first sequence replaced by the second.
    assert_eq!(handle.collect_garbage(), 2);
}

#[test]
fn full_garbage_queue_defers_swaps_instead_of_dropping() {
    let (mut engine, mut handle) = impulse_engine();
    for _ in 0..GARBAGE_CAPACITY + 10 {
        handle
            .send(EngineMessage::SetSequence(Box::default()))
            .unwrap();
    }
    render_stereo(&mut engine, 16);
    // The first swap replaces nothing; then the queue fills up and swaps wait.
    assert_eq!(handle.collect_garbage(), GARBAGE_CAPACITY);
    render_stereo(&mut engine, 16);
    assert_eq!(handle.collect_garbage(), 9);
}

#[test]
fn live_notes_hold_until_released() {
    let (mut engine, mut handle) = engine_pair(SR);
    let mut graph = Graph::new();
    graph.add_channel(Instrument::Synth, ChannelParams::default());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    let mut live = handle.take_live_sender().unwrap();
    assert!(handle.take_live_sender().is_none());
    live.send(LiveEvent::NoteOn {
        channel: 0,
        key: 69,
        velocity: 1.0,
    });
    let out = render_stereo(&mut engine, SR as usize);
    assert_eq!(engine.active_voices(), 1);
    assert!(out[SR as usize..].iter().any(|s| s.abs() > 0.01));
    live.send(LiveEvent::NoteOff {
        channel: 0,
        key: 69,
    });
    render_stereo(&mut engine, SR as usize);
    assert_eq!(engine.active_voices(), 0);
}

#[test]
fn muted_channel_is_silent() {
    let (mut engine, _handle) = impulse_engine();
    let params = ChannelParams {
        mute: true,
        ..impulse_params()
    };
    engine.apply(EngineMessage::Channel { channel: 0, params });
    // Mute fades over one block so it never clicks.
    render_mono(&mut engine, MAX_BLOCK);
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![note(0, 0)],
        384,
    ))));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 1_000);
    assert!(nonzero_frames(&out).is_empty());
}

#[test]
fn volume_changes_ramp_instead_of_jumping() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    // A constant signal: a long, flat sample.
    let flat = Arc::new(SampleBuffer::mono(vec![0.5; 48_000], f64::from(SR)));
    graph.add_channel(Instrument::Sampler(flat), impulse_params());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 60,
        velocity: 1.0,
    });
    let mut block = vec![0.0; MAX_BLOCK];
    engine.process(&mut block, 1);
    let params = ChannelParams {
        volume: 0.0,
        ..impulse_params()
    };
    engine.apply(EngineMessage::Channel { channel: 0, params });
    engine.process(&mut block, 1);
    let biggest_step = block
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    assert!(biggest_step < 0.01, "step {biggest_step}");
    assert!(block[0] > 0.4 && block[MAX_BLOCK - 1].abs() < 1e-6);
}

#[test]
fn channel_routes_through_insert_track_to_master() {
    let (mut engine, handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let impulse = Arc::new(SampleBuffer::mono(vec![1.0], f64::from(SR)));
    let params = ChannelParams {
        track: 1,
        ..impulse_params()
    };
    graph.add_channel(Instrument::Sampler(impulse), params);
    graph.add_track(
        TrackParams {
            volume: 0.5,
            pan: 1.0,
            ..TrackParams::default()
        },
        Vec::new(),
    );
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![note(0, 0)],
        384,
    ))));
    engine.apply(EngineMessage::Play);
    let out = render_stereo(&mut engine, 64);
    assert!(out[0].abs() < 1e-6, "panned hard right, left is silent");
    assert!((out[1] - 0.5).abs() < 1e-6);
    let meters = handle.meters();
    assert!((meters.take_peak(1).1 - 0.5).abs() < 1e-6);
    assert!((meters.take_peak(0).1 - 0.5).abs() < 1e-6);
}

#[test]
fn bypassed_and_enabled_effects() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let dc = Arc::new(SampleBuffer::mono(vec![1.0; 4_800], f64::from(SR)));
    graph.add_channel(Instrument::Sampler(dc), impulse_params());
    let mut values = EffectKind::Distortion.defaults();
    values[2] = -6.0206; // output gain 0.5
    values[1] = 0.0; // dry only
    let fx = EffectParams {
        enabled: true,
        values,
        mix: 1.0,
    };
    graph.set_master(
        TrackParams::default(),
        vec![(Effect::new(EffectKind::Distortion, SR as f32), fx)],
    );
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 60,
        velocity: 1.0,
    });
    let out = render_mono(&mut engine, 100);
    assert!((out[50] - 0.5).abs() < 1e-3, "got {}", out[50]);
    engine.apply(EngineMessage::Effect {
        track: 0,
        slot: 0,
        params: EffectParams {
            enabled: false,
            values,
            mix: 1.0,
        },
    });
    let out = render_mono(&mut engine, 100);
    assert!((out[50] - 1.0).abs() < 1e-3, "got {}", out[50]);
}

#[test]
fn voice_pool_steals_when_full() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    graph.add_channel(Instrument::Synth, ChannelParams::default());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    for key in 0..(MAX_VOICES as u8 + 10) {
        engine.apply(EngineMessage::NoteOn {
            channel: 0,
            key: 30 + key,
            velocity: 0.5,
        });
    }
    assert_eq!(engine.active_voices(), MAX_VOICES);
    let out = render_stereo(&mut engine, 4_800);
    assert!(out.iter().all(|s| s.is_finite()));
    engine.apply(EngineMessage::AllNotesOff);
    render_stereo(&mut engine, SR as usize);
    assert_eq!(engine.active_voices(), 0);
}

#[test]
fn one_shot_samples_ignore_note_length() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let long = Arc::new(SampleBuffer::mono(vec![0.5; BEAT], f64::from(SR)));
    graph.add_channel(Instrument::Sampler(long), impulse_params());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    // A 1/16 note on a one-beat sample: the whole sample still plays.
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![note(0, 0)],
        8 * 96,
    ))));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 2 * BEAT);
    assert!((out[BEAT - 10] - 0.5).abs() < 1e-6);
    assert!(out[BEAT + 10].abs() < 1e-6);
}

#[test]
fn seek_moves_playhead_within_loop() {
    let (mut engine, _handle) = impulse_engine();
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![note(2, 0)],
        384,
    ))));
    engine.apply(EngineMessage::Seek(f64::from(PPQ) * 1.5));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, BEAT);
    assert_eq!(nonzero_frames(&out), [BEAT / 2]);
    engine.apply(EngineMessage::Seek(f64::from(PPQ) * 9.0));
    assert!(
        (engine.position_beats() - 1.0).abs() < 1e-9,
        "wraps into the loop"
    );
}

#[test]
fn metronome_clicks_on_every_beat_and_accents_bars() {
    let (mut engine, _handle) = engine_pair(SR);
    engine.apply(EngineMessage::SetGraph(Box::default()));
    engine.apply(EngineMessage::SetTempo(120.0));
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![],
        8 * 96,
    ))));
    engine.apply(EngineMessage::SetMetronome(true));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 5 * BEAT);
    let onsets: Vec<usize> = (0..5)
        .map(|beat| beat * BEAT)
        .filter(|start| out[start + 1..start + 200].iter().any(|s| s.abs() > 0.05))
        .collect();
    assert_eq!(onsets, [0, BEAT, 2 * BEAT, 3 * BEAT, 4 * BEAT]);
    assert!(
        out[BEAT / 2..BEAT - 10].iter().all(|s| s.abs() < 1e-3),
        "silent between clicks"
    );
    engine.apply(EngineMessage::SetMetronome(false));
    let quiet = render_mono(&mut engine, 2 * BEAT);
    assert!(quiet.iter().all(|s| s.abs() < 1e-3));
}

/// Sign changes in `samples`: a rough pitch measure.
fn crossings(samples: &[f32]) -> usize {
    samples
        .windows(2)
        .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
        .count()
}

#[test]
fn metronome_follows_the_meter() {
    let (mut engine, _handle) = engine_pair(SR);
    engine.apply(EngineMessage::SetGraph(Box::default()));
    engine.apply(EngineMessage::SetTempo(120.0));
    let six_eight: TimeSig = "6/8".parse().unwrap();
    let seq = Sequence::new(vec![], 16 * 96).with_meter(Meter::new(six_eight));
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::SetMetronome(true));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 4 * BEAT);
    let eighth = BEAT / 2;
    let onsets: Vec<usize> = (0..8)
        .map(|k| k * eighth)
        .filter(|start| out[start + 1..start + 200].iter().any(|s| s.abs() > 0.05))
        .collect();
    assert_eq!(onsets, (0..8).map(|k| k * eighth).collect::<Vec<_>>());
    // Bars of six eighths: clicks 0 and 6 are accented (higher).
    let pitch = |k: usize| crossings(&out[k * eighth..k * eighth + 2000]);
    assert!(
        pitch(0) > pitch(1) && pitch(6) > pitch(5),
        "{} {}",
        pitch(0),
        pitch(1)
    );
    assert_eq!(pitch(1), pitch(2));
}

#[test]
fn a_count_in_is_a_bar_of_the_signature_where_playback_starts() {
    let (mut engine, _handle) = beat_engine();
    let notes = (0..4).map(|beat| note(beat, 0)).collect();
    let seven_eight: TimeSig = "7/8".parse().unwrap();
    let seq = Sequence::new(notes, 4 * u64::from(PPQ)).with_meter(Meter::new(seven_eight));
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::SetStart(Start::CountIn(1)));
    engine.apply(EngineMessage::Play);
    let eighth = BEAT / 2;
    let out = render_mono(&mut engine, 7 * eighth + 10);
    for k in 0..7 {
        assert!(out[k * eighth + 5].abs() > 1e-3, "click {k}");
    }
    assert!(
        (out[7 * eighth] - 1.0).abs() < 1e-6,
        "the first note after seven eighths"
    );
    assert!(engine.is_playing());
}

#[test]
fn sends_feed_higher_inserts_and_master_routing_can_be_cut() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let impulse = Arc::new(SampleBuffer::mono(vec![1.0], f64::from(SR)));
    let params = ChannelParams {
        track: 1,
        ..impulse_params()
    };
    graph.add_channel(Instrument::Sampler(impulse), params);
    // Insert 1 sends half its signal to insert 2 and does not feed the master.
    let mut source = TrackParams {
        to_master: false,
        send_count: 1,
        ..TrackParams::default()
    };
    source.sends[0] = SendParams {
        target: 2,
        level: 0.5,
    };
    graph.add_track(source, Vec::new());
    graph.add_track(TrackParams::default(), Vec::new());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![note(0, 0)],
        384,
    ))));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 16);
    assert!(
        (out[0] - 0.5).abs() < 1e-6,
        "only the send reaches the master: {}",
        out[0]
    );
    // Re-enable the master route: direct 1.0 plus send 0.5.
    let both = TrackParams {
        to_master: true,
        ..source
    };
    engine.apply(EngineMessage::Track {
        track: 1,
        params: both,
    });
    engine.apply(EngineMessage::Stop);
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 16);
    assert!((out[0] - 1.5).abs() < 1e-6, "got {}", out[0]);
}

#[test]
fn sends_to_lower_tracks_are_ignored() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let impulse = Arc::new(SampleBuffer::mono(vec![1.0], f64::from(SR)));
    let params = ChannelParams {
        track: 2,
        ..impulse_params()
    };
    graph.add_channel(Instrument::Sampler(impulse), params);
    graph.add_track(TrackParams::default(), Vec::new());
    let mut backwards = TrackParams {
        send_count: 1,
        ..TrackParams::default()
    };
    backwards.sends[0] = SendParams {
        target: 1,
        level: 1.0,
    };
    graph.add_track(backwards, Vec::new());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![note(0, 0)],
        384,
    ))));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 16);
    assert!((out[0] - 1.0).abs() < 1e-6);
}

#[test]
fn preview_plays_on_master_and_is_returned_as_garbage() {
    let (mut engine, mut handle) = engine_pair(SR);
    engine.apply(EngineMessage::SetGraph(Box::default()));
    let sample = Arc::new(SampleBuffer::mono(vec![0.5; 100], f64::from(SR)));
    handle
        .send(EngineMessage::PlayPreview(Box::new(Preview::new(
            Arc::clone(&sample),
            1.0,
        ))))
        .unwrap();
    let out = render_mono(&mut engine, 200);
    assert!((out[50] - 0.5).abs() < 1e-6);
    assert!(out[150].abs() < 1e-6, "stops at the end of the sample");
    handle
        .send(EngineMessage::PlayPreview(Box::new(Preview::new(
            sample, 1.0,
        ))))
        .unwrap();
    handle.send(EngineMessage::StopPreview).unwrap();
    let out = render_mono(&mut engine, 50);
    assert!(out.iter().all(|s| s.abs() < 1e-6));
    assert_eq!(handle.collect_garbage(), 1, "the replaced first preview");
}

#[test]
fn automation_moves_a_track_volume_during_playback() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let dc = Arc::new(SampleBuffer::mono(vec![1.0; 4 * BEAT], f64::from(SR)));
    graph.add_channel(Instrument::Sampler(dc), impulse_params());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    let lane = AutomationLane {
        target: AutomationTarget::TrackVolume(0),
        points: vec![(0, 1.0), (u64::from(PPQ) * 2, 0.0)],
    };
    let sequence = Sequence::new(vec![note(0, 0)], 4 * u64::from(PPQ)).with_automation(vec![lane]);
    engine.apply(EngineMessage::SetSequence(Box::new(sequence)));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 3 * BEAT);
    // Halfway through the ramp (one beat in) the master is at about half.
    assert!((out[BEAT] - 0.5).abs() < 0.02, "got {}", out[BEAT]);
    assert!(out[2 * BEAT + 500].abs() < 0.01, "ramp reached zero");
}

#[test]
fn loop_start_is_where_playback_wraps_and_stop_returns() {
    let (mut engine, _handle) = impulse_engine();
    // Events at beats 0, 1, 2, 3; loop beats 2..4.
    let seq = Sequence::new((0..4).map(|b| note(b, 0)).collect(), 4 * u64::from(PPQ))
        .with_loop_start(2 * u64::from(PPQ));
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::Stop);
    assert!(
        (engine.position_beats() - 2.0).abs() < 1e-9,
        "stop returns to the loop start"
    );
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 4 * BEAT + 10);
    assert_eq!(
        nonzero_frames(&out),
        [0, BEAT, 2 * BEAT, 3 * BEAT, 4 * BEAT]
    );
    engine.apply(EngineMessage::Seek(f64::from(PPQ) * 9.0));
    assert!(
        (engine.position_beats() - 3.0).abs() < 1e-9,
        "seeks fold into the loop"
    );
}

#[test]
fn channel_pan_follows_the_pan_law() {
    for (law, pan, expected) in [
        (PanLaw::Circular, 0.0, (FRAC_1_SQRT_2, FRAC_1_SQRT_2)),
        (PanLaw::Circular, 1.0, (0.0, 1.0)),
        (PanLaw::Triangular, 0.0, (0.5, 0.5)),
        (PanLaw::Triangular, -0.5, (0.75, 0.25)),
    ] {
        let (mut engine, _handle) = impulse_engine();
        let params = ChannelParams {
            volume: 1.0,
            pan,
            pan_law: law,
            ..impulse_params()
        };
        engine.apply(EngineMessage::Channel { channel: 0, params });
        // Let the gain ramp settle before the note.
        render_stereo(&mut engine, MAX_BLOCK);
        engine.apply(EngineMessage::NoteOn {
            channel: 0,
            key: 60,
            velocity: 1.0,
        });
        let out = render_stereo(&mut engine, 8);
        assert!(
            (out[0] - expected.0).abs() < 1e-5 && (out[1] - expected.1).abs() < 1e-5,
            "{law:?} at {pan}: {:?}",
            &out[..2]
        );
    }
}

#[test]
fn note_pan_moves_a_note_left_or_right() {
    let (mut engine, _handle) = impulse_engine();
    let mut left = note(0, 0);
    left.pan = -1.0;
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![left],
        384,
    ))));
    engine.apply(EngineMessage::Play);
    let out = render_stereo(&mut engine, 8);
    assert!(
        (out[0] - 1.0).abs() < 1e-6 && out[1].abs() < 1e-6,
        "{:?}",
        &out[..2]
    );
}

/// Counters a [`Probe`] shares with the test.
#[derive(Debug, Default)]
struct ProbeLog {
    note_ons: std::sync::atomic::AtomicU32,
    note_offs: std::sync::atomic::AtomicU32,
    all_off: std::sync::atomic::AtomicU32,
    stopped: std::sync::atomic::AtomicBool,
    /// Bits of the loudest sidechain sample seen, 0 if never keyed.
    key_peak: std::sync::atomic::AtomicU32,
}

/// A fake plugin: as an instrument it writes a 1.0 impulse on the frame each
/// note starts; as an effect it multiplies by its `gain` parameter (id 7).
#[derive(Debug)]
struct Probe {
    log: Arc<ProbeLog>,
    pending: Option<u32>,
    gain: f32,
}

impl Probe {
    fn new() -> (Self, Arc<ProbeLog>) {
        let log = Arc::new(ProbeLog::default());
        let probe = Self {
            log: Arc::clone(&log),
            pending: None,
            gain: 1.0,
        };
        (probe, log)
    }
}

impl External for Probe {
    fn note_on(&mut self, delay: u32, _key: u8, _velocity: f32, _pan: f32, _gate: Option<u64>) {
        self.log
            .note_ons
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.pending = Some(delay);
    }

    fn note_off(&mut self, _delay: u32, _key: u8) {
        self.log
            .note_offs
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn all_notes_off(&mut self) {
        self.log
            .all_off
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if id == 7 {
            self.gain = value as f32;
        }
    }

    fn sidechain(&mut self, left: &[f32], right: &[f32]) {
        use std::sync::atomic::Ordering;
        let peak = left
            .iter()
            .chain(right)
            .fold(0.0_f32, |m, s| m.max(s.abs()));
        let seen = f32::from_bits(self.log.key_peak.load(Ordering::Relaxed));
        self.log
            .key_peak
            .store(seen.max(peak).to_bits(), Ordering::Relaxed);
    }

    fn process(&mut self, left: &mut [f32], right: &mut [f32], _transport: &ExternalTransport) {
        if let Some(delay) = self.pending.take() {
            if let (Some(l), Some(r)) =
                (left.get_mut(delay as usize), right.get_mut(delay as usize))
            {
                *l += 1.0;
                *r += 1.0;
            }
        }
        for s in left.iter_mut().chain(right.iter_mut()) {
            *s *= self.gain;
        }
    }

    fn stop(&mut self) {
        self.log
            .stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

fn probe_engine() -> (Engine, EngineHandle, Arc<ProbeLog>) {
    let (mut engine, handle) = engine_pair(SR);
    let (probe, log) = Probe::new();
    engine.apply(EngineMessage::SetExternal {
        slot: 3,
        external: Some(Box::new(probe)),
    });
    let mut graph = Graph::new();
    graph.add_channel(Instrument::External(3), impulse_params());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    (engine, handle, log)
}

#[test]
fn plugin_instruments_get_sequence_notes_on_the_exact_frame() {
    use std::sync::atomic::Ordering;
    let (mut engine, _handle, log) = probe_engine();
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![note(1, 0)],
        u64::from(PPQ) * 4,
    ))));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, BEAT + 1_000);
    assert_eq!(nonzero_frames(&out), [BEAT]);
    assert_eq!(log.note_ons.load(Ordering::Relaxed), 1);
    assert_eq!(engine.active_voices(), 0, "plugins voice their own notes");
    engine.apply(EngineMessage::Stop);
    assert!(log.all_off.load(Ordering::Relaxed) >= 1);
}

#[test]
fn live_notes_reach_plugin_instruments() {
    use std::sync::atomic::Ordering;
    let (mut engine, _handle, log) = probe_engine();
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 64,
        velocity: 1.0,
    });
    engine.apply(EngineMessage::NoteOff {
        channel: 0,
        key: 64,
    });
    assert_eq!(log.note_ons.load(Ordering::Relaxed), 1);
    assert_eq!(log.note_offs.load(Ordering::Relaxed), 1);
    let out = render_mono(&mut engine, 100);
    assert_eq!(nonzero_frames(&out), [0]);
}

#[test]
fn plugin_effects_process_their_track_and_take_parameters() {
    let (mut engine, _handle) = impulse_engine();
    let (probe, _log) = Probe::new();
    engine.apply(EngineMessage::SetExternal {
        slot: 0,
        external: Some(Box::new(probe)),
    });
    let mut graph = Graph::new();
    let impulse = Arc::new(SampleBuffer::mono(vec![1.0], f64::from(SR)));
    graph.add_channel(Instrument::Sampler(impulse), impulse_params());
    graph.set_master(
        TrackParams::default(),
        vec![(
            Effect::new(EffectKind::Plugin, SR as f32),
            EffectParams {
                enabled: true,
                values: [0.0; musiccraft_params::MAX_EFFECT_PARAMS],
                mix: 1.0,
            },
        )],
    );
    assert!(graph.attach_external(0, 0, 0));
    assert!(!graph.attach_external(0, 5, 0));
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::ExternalParam {
        slot: 0,
        id: 7,
        value: 0.5,
    });
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 60,
        velocity: 1.0,
    });
    let out = render_mono(&mut engine, 10);
    assert!((out[0] - 0.5).abs() < 1e-6, "got {}", out[0]);
}

/// The loudest key a plugin effect on the master sees, with an impulse on
/// insert 1 (kept off the master) named as its sidechain and, if `routed`,
/// routed to the master as one.
fn plugin_key_peak(routed: bool) -> f32 {
    use std::sync::atomic::Ordering;
    let (mut engine, _handle) = impulse_engine();
    let (probe, log) = Probe::new();
    engine.apply(EngineMessage::SetExternal {
        slot: 0,
        external: Some(Box::new(probe)),
    });
    let mut graph = Graph::new();
    let impulse = Arc::new(SampleBuffer::mono(vec![1.0], f64::from(SR)));
    graph.add_channel(
        Instrument::Sampler(impulse),
        ChannelParams {
            track: 1,
            ..impulse_params()
        },
    );
    let mut values = [0.0; musiccraft_params::MAX_EFFECT_PARAMS];
    values[EffectKind::Plugin.sidechain_index().unwrap()] = 1.0;
    graph.set_master(
        TrackParams::default(),
        vec![(
            Effect::new(EffectKind::Plugin, SR as f32),
            EffectParams {
                enabled: true,
                values,
                mix: 1.0,
            },
        )],
    );
    let mut insert = TrackParams {
        to_master: false,
        ..TrackParams::default()
    };
    if routed {
        insert.sidechain_to[0] = 0;
        insert.sidechain_count = 1;
    }
    graph.add_track(insert, Vec::new());
    assert!(graph.attach_external(0, 0, 0));
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 60,
        velocity: 1.0,
    });
    let out = render_mono(&mut engine, 64);
    assert!(out.iter().all(|s| *s == 0.0), "the key is not heard");
    f32::from_bits(log.key_peak.load(Ordering::Relaxed))
}

#[test]
fn plugin_effects_get_their_routed_sidechain() {
    assert!(plugin_key_peak(true) > 0.1);
    assert!(plugin_key_peak(false).abs() < f32::EPSILON);
}

#[test]
fn inserts_run_after_the_inserts_that_feed_or_key_them() {
    let mut graph = Graph::new();
    for _ in 0..5 {
        graph.add_track(TrackParams::default(), Vec::new());
    }
    let order = |graph: &mut Graph| {
        let Graph {
            tracks, schedule, ..
        } = graph;
        schedule.build(tracks);
        schedule.order.clone()
    };
    assert_eq!(order(&mut graph), [1, 2, 3, 4, 5]);
    // 4 keys 2 and 1 sends to 4, so 1 runs before 4 and 4 before 2.
    graph.tracks[4].params.sidechain_to[0] = 2;
    graph.tracks[4].params.sidechain_count = 1;
    graph.tracks[1].params.sends[0] = SendParams {
        target: 4,
        level: 1.0,
    };
    graph.tracks[1].params.send_count = 1;
    assert_eq!(order(&mut graph), [1, 3, 4, 2, 5]);
    // A loop (refused by commands) still runs every insert once.
    graph.tracks[2].params.sends[0] = SendParams {
        target: 4,
        level: 1.0,
    };
    graph.tracks[2].params.send_count = 1;
    let mut looped = order(&mut graph);
    looped.sort_unstable();
    assert_eq!(looped, [1, 2, 3, 4, 5]);
}

#[test]
fn replaced_plugins_are_stopped_and_returned_while_graph_swaps_keep_them() {
    use std::sync::atomic::Ordering;
    let (mut engine, mut handle, log) = probe_engine();
    let mut graph = Graph::new();
    graph.add_channel(Instrument::External(3), impulse_params());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    assert!(engine.has_external(3));
    assert!(!log.stopped.load(Ordering::Relaxed));
    handle.collect_garbage();
    engine.apply(EngineMessage::SetExternal {
        slot: 3,
        external: None,
    });
    assert!(log.stopped.load(Ordering::Relaxed));
    assert!(!engine.has_external(3));
    assert_eq!(handle.collect_garbage(), 1);
    // Notes for an empty slot are ignored.
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 60,
        velocity: 1.0,
    });
    assert!(render_mono(&mut engine, 64).iter().all(|s| *s == 0.0));
}

#[test]
fn layers_play_their_channels() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let impulse = Arc::new(SampleBuffer::mono(vec![1.0], f64::from(SR)));
    graph.add_channel(Instrument::Sampler(Arc::clone(&impulse)), impulse_params());
    graph.add_channel(Instrument::Sampler(impulse), impulse_params());
    graph.add_channel(Instrument::Layer(vec![0, 1]), impulse_params());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::NoteOn {
        channel: 2,
        key: 60,
        velocity: 1.0,
    });
    assert_eq!(engine.active_voices(), 2, "one voice per layered channel");
    let out = render_mono(&mut engine, 64);
    assert!((out[0] - 2.0).abs() < 1e-3, "both play: {}", out[0]);
    engine.apply(EngineMessage::NoteOff {
        channel: 2,
        key: 60,
    });
}

#[test]
fn slide_notes_on_fixed_pitch_instruments_play_as_new_notes() {
    for (name, instrument, voices) in [
        ("drum synth", Instrument::DrumSynth, 2),
        ("synth", Instrument::Synth, 1),
    ] {
        let (mut engine, _handle) = engine_pair(SR);
        let mut graph = Graph::new();
        graph.add_channel(instrument, impulse_params());
        engine.apply(EngineMessage::SetGraph(Box::new(graph)));
        engine.apply(EngineMessage::SetTempo(120.0));
        let mut first = note(0, 0);
        first.length = 2 * u64::from(PPQ);
        let mut slide = note(0, 0);
        slide.start = u64::from(PPQ) / 32;
        slide.key = 67;
        slide.slide = true;
        let seq = Sequence::new(vec![first, slide], 4 * u64::from(PPQ));
        engine.apply(EngineMessage::SetSequence(Box::new(seq)));
        engine.apply(EngineMessage::Play);
        // Just past the slide note (1/32 beat is 750 frames at 120 BPM).
        render_mono(&mut engine, 1_000);
        assert_eq!(engine.active_voices(), voices, "{name}");
    }
}

/// A synth engine at 120 BPM playing `events` (start and length in beats).
fn synth_playing(events: &[(u64, u64, u8, u8, bool)]) -> Engine {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    graph.add_channel(Instrument::Synth, impulse_params());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    let ppq = u64::from(PPQ);
    let notes = events
        .iter()
        .map(|&(start, length, key, color, slide)| {
            let mut event = note(0, 0);
            event.start = start * ppq;
            event.length = length * ppq;
            event.key = key;
            event.shape.color = color;
            event.slide = slide;
            event
        })
        .collect();
    let seq = Sequence::new(notes, 16 * ppq);
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::Play);
    engine
}

#[test]
fn a_chord_slides_by_its_top_note_within_its_colour_group() {
    // C4 + G4 in group 0, E3 in group 1; a slide to A4 in group 0 at beat 1.
    let mut engine = synth_playing(&[
        (0, 4, 48, 0, false),
        (0, 4, 55, 0, false),
        (0, 4, 40, 1, false),
        (1, 1, 57, 0, true),
    ]);
    render_mono(&mut engine, 3 * BEAT);
    // G4 → A4 is +2, so C4 → D4; E3 is in another group and stays.
    assert_eq!(engine.held_keys(), [40, 50, 57]);
    assert_eq!(engine.active_voices(), 3, "a slide note starts no voice");
}

#[test]
fn colour_groups_slide_independently() {
    let mut engine = synth_playing(&[
        (0, 4, 48, 0, false),
        (0, 4, 40, 1, false),
        (1, 1, 52, 0, true),
        (1, 1, 35, 1, true),
    ]);
    render_mono(&mut engine, 3 * BEAT);
    assert_eq!(engine.held_keys(), [35, 52]);
}

#[test]
fn a_slide_note_with_nothing_of_its_group_sounding_is_silent() {
    // The only held note is in another colour group.
    let mut engine = synth_playing(&[(0, 4, 48, 1, false), (1, 1, 60, 0, true)]);
    render_mono(&mut engine, 3 * BEAT / 2);
    assert_eq!(engine.held_keys(), [48]);
    let mut engine = synth_playing(&[(1, 1, 60, 0, true)]);
    let out = render_mono(&mut engine, 3 * BEAT);
    assert!(
        out.iter().all(|s| s.abs() < 1e-6),
        "no voice for a lone slide"
    );
}

#[test]
fn the_highest_of_several_simultaneous_slide_notes_wins() {
    let mut engine = synth_playing(&[
        (0, 4, 48, 0, false),
        (1, 1, 50, 0, true),
        (1, 1, 53, 0, true),
    ]);
    render_mono(&mut engine, 3 * BEAT);
    assert_eq!(engine.held_keys(), [53]);
}

#[test]
fn a_slide_never_lengthens_the_note_it_bends() {
    // The note ends at beat 2; the slide note runs to beat 4.
    let mut engine = synth_playing(&[(0, 2, 48, 0, false), (1, 3, 60, 0, true)]);
    render_mono(&mut engine, 2 * BEAT + BEAT / 2);
    assert!(engine.held_keys().is_empty());
}

/// Two channels at 120 BPM playing `events` (channel, start beat, key), each
/// held for four beats.
fn two_channels(
    instrument: impl Fn() -> Instrument,
    params: [&[(usize, f32)]; 2],
    events: &[(u16, u64, u8)],
) -> Engine {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    for settings in params {
        let mut p = impulse_params();
        for &(index, value) in settings {
            p.instrument[index] = value;
        }
        graph.add_channel(instrument(), p);
    }
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    let notes = events
        .iter()
        .map(|&(channel, start, key)| {
            let mut event = note(start, channel);
            event.length = 4 * u64::from(PPQ);
            event.key = key;
            event
        })
        .collect();
    let seq = Sequence::new(notes, 16 * u64::from(PPQ));
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::Play);
    engine
}

/// Frames a cut takes, with a little slack.
const CUT_FRAMES: usize = (musiccraft_instruments::CUT_SECONDS * SR as f32) as usize + 8;

#[test]
fn a_cut_group_chokes_the_channels_cut_by_it() {
    // Channel 0 is a closed hat (cuts group 1), channel 1 an open hat (cut by 1).
    let hats = || Instrument::DrumSynth;
    let closed: &[(usize, f32)] = &[(inst::DRUM_TYPE, 2.0), (inst::CUT_GROUP, 1.0)];
    let open: &[(usize, f32)] = &[
        (inst::DRUM_TYPE, 2.0),
        (inst::DRUM_DECAY, 2.0),
        (inst::CUT_BY, 1.0),
    ];
    let mut engine = two_channels(hats, [closed, open], &[(1, 0, 60), (0, 1, 60)]);
    render_mono(&mut engine, BEAT - 10);
    assert_eq!(engine.voices_on(1), 1, "the open hat rings");
    render_mono(&mut engine, 10 + CUT_FRAMES);
    assert_eq!(engine.voices_on(1), 0, "choked by the closed hat");
    assert_eq!(engine.voices_on(0), 1);
    // The other way round nothing is choked.
    let mut engine = two_channels(hats, [closed, open], &[(0, 0, 60), (1, 1, 60)]);
    render_mono(&mut engine, BEAT + CUT_FRAMES);
    assert_eq!(engine.voices_on(1), 1);
    assert_eq!(engine.voices_on(0), 1, "the open hat does not cut");
}

#[test]
fn a_cut_starts_on_the_cutting_notes_frame_and_fades_quickly() {
    // A long one-shot sample of constant level on channel 1, cut by channel 0.
    let sample = Arc::new(SampleBuffer::mono(vec![0.5; 4 * BEAT], f64::from(SR)));
    let cutter: &[(usize, f32)] = &[(inst::CUT_GROUP, 3.0), (inst::ONE_SHOT, 1.0)];
    let cut: &[(usize, f32)] = &[(inst::CUT_BY, 3.0), (inst::ONE_SHOT, 1.0)];
    let mut engine = two_channels(
        || Instrument::Sampler(Arc::clone(&sample)),
        [cutter, cut],
        &[(1, 0, 60), (0, 1, 60)],
    );
    let out = render_mono(&mut engine, BEAT + CUT_FRAMES + 100);
    let level = std::f32::consts::SQRT_2 * 0.5 * std::f32::consts::FRAC_1_SQRT_2;
    assert!((out[BEAT - 1] - level).abs() < 1e-4, "{}", out[BEAT - 1]);
    // From the cut frame on: the new note plus a fading old one.
    assert!(out[BEAT + 1] > level && out[BEAT + 1] < 2.0 * level);
    let after = out[BEAT + CUT_FRAMES + 50];
    assert!((after - level).abs() < 1e-4, "only the new note: {after}");
}

#[test]
fn mono_and_polyphony_limits_cut_the_oldest_voices() {
    let chord = [(0, 0, 60), (0, 0, 64), (0, 0, 67), (0, 1, 72)];
    for (settings, voices) in [
        (&[][..], 4),
        (&[(inst::MONO, 1.0)][..], 1),
        (&[(inst::POLY_MAX, 2.0)][..], 2),
        (&[(inst::POLY_MAX, 3.0)][..], 3),
    ] {
        let mut engine = two_channels(|| Instrument::Synth, [settings, &[]], &chord);
        render_mono(&mut engine, BEAT + CUT_FRAMES);
        assert_eq!(engine.voices_on(0), voices, "{settings:?}");
        // The newest note always sounds.
        assert!(engine.held_keys().contains(&72), "{settings:?}");
    }
}

/// The impulse engine with a note on every beat of a four-beat sequence.
fn beat_engine() -> (Engine, EngineHandle) {
    let (mut engine, handle) = impulse_engine();
    let notes = (0..4).map(|beat| note(beat, 0)).collect();
    let seq = Sequence::new(notes, 4 * u64::from(PPQ));
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    (engine, handle)
}

#[test]
fn a_count_in_clicks_a_bar_then_starts_on_the_exact_frame() {
    let (mut engine, handle) = beat_engine();
    engine.apply(EngineMessage::SetStart(Start::CountIn(1)));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 4 * BEAT + 10);
    // Four clicks, one per beat, louder at the start of each.
    for beat in 0..4 {
        assert!(out[beat * BEAT + 5].abs() > 1e-3, "click {beat}");
        assert!(
            out[beat * BEAT + BEAT / 2].abs() < 1e-6,
            "silence after {beat}"
        );
    }
    assert!(out[4 * BEAT - 1].abs() < 1e-6);
    // The first note (an impulse) lands on the frame the count-in ends.
    assert!((out[4 * BEAT] - 1.0).abs() < 1e-6, "{}", out[4 * BEAT]);
    assert!(engine.is_playing());
    assert!(!handle.meters().is_counting_in());
    assert!((engine.position_ticks() - 10.0 * f64::from(PPQ) / BEAT as f64).abs() < 1e-6);
}

#[test]
fn stop_cancels_a_count_in() {
    let (mut engine, handle) = beat_engine();
    engine.apply(EngineMessage::SetStart(Start::CountIn(1)));
    engine.apply(EngineMessage::Play);
    render_mono(&mut engine, BEAT);
    assert!(handle.meters().is_counting_in() && !engine.is_playing());
    engine.apply(EngineMessage::Stop);
    render_mono(&mut engine, 4 * BEAT);
    assert!(!engine.is_playing() && !handle.meters().is_counting_in());
    // With the start back to now, Play starts at once.
    engine.apply(EngineMessage::SetStart(Start::Now));
    engine.apply(EngineMessage::Play);
    assert!(engine.is_playing());
}

#[test]
fn waiting_for_input_starts_with_the_first_live_note() {
    let (mut engine, handle) = beat_engine();
    engine.apply(EngineMessage::SetStart(Start::WaitForInput));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 2 * BEAT);
    assert!(out.iter().all(|s| s.abs() < 1e-6), "nothing plays");
    assert!(handle.meters().is_waiting() && !engine.is_playing());
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 60,
        velocity: 1.0,
    });
    assert!(engine.is_playing());
    let out = render_mono(&mut engine, BEAT + 10);
    // The live note and the sequence's first note both sound at once.
    assert!((out[0] - 2.0).abs() < 1e-6, "{}", out[0]);
    assert!((out[BEAT] - 1.0).abs() < 1e-6);
    assert!(!handle.meters().is_waiting());
}

#[test]
fn stop_at_end_stops_and_rewinds_instead_of_looping() {
    let (mut engine, handle) = beat_engine();
    engine.apply(EngineMessage::SetStopAtEnd(true));
    engine.apply(EngineMessage::Seek(f64::from(PPQ)));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 4 * BEAT);
    assert_eq!(
        nonzero_frames(&out),
        [0, BEAT, 2 * BEAT],
        "beats 2-4, no wrap"
    );
    assert!(!engine.is_playing());
    assert!(engine.position_ticks().abs() < f64::EPSILON);
    assert_eq!(handle.meters().end_stops(), 1);
}

#[test]
fn a_nan_burst_cannot_silence_a_reverb_for_good() {
    let (mut engine, _handle) = engine_pair(SR);
    let mut graph = Graph::new();
    // A sample that starts with invalid values, then holds a steady level.
    let mut data = vec![0.5; 480_000];
    data[0] = f32::NAN;
    data[1] = f32::INFINITY;
    let buffer = Arc::new(SampleBuffer::mono(data, f64::from(SR)));
    graph.add_channel(Instrument::Sampler(buffer), impulse_params());
    let fx = EffectParams {
        enabled: true,
        values: EffectKind::Reverb.defaults(),
        mix: 1.0,
    };
    graph.set_master(
        TrackParams::default(),
        vec![(Effect::new(EffectKind::Reverb, SR as f32), fx)],
    );
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 60,
        velocity: 1.0,
    });
    let out = render_mono(&mut engine, 4_800);
    assert!(out.iter().all(|s| s.is_finite()));
    // The reverb (and the dry signal through it) keeps sounding afterwards.
    assert!(
        out[2_000..].iter().any(|s| s.abs() > 0.01),
        "the track was silenced by the bad samples"
    );
}

#[test]
fn tuner_publishes_the_pitch_it_hears() {
    let (mut engine, handle) = engine_pair(SR);
    let mut graph = Graph::new();
    let tone: Vec<f32> = (0..SR as usize)
        .map(|i| (i as f32 * 220.0 * std::f32::consts::TAU / SR as f32).sin() * 0.5)
        .collect();
    let params = impulse_params();
    let root = params.instrument[inst::ROOT_KEY].round() as u8;
    graph.add_channel(
        Instrument::Sampler(Arc::new(SampleBuffer::mono(tone, f64::from(SR)))),
        params,
    );
    let slot = |kind: EffectKind, enabled: bool| {
        (
            Effect::new(kind, SR as f32),
            EffectParams {
                enabled,
                values: kind.defaults(),
                mix: 1.0,
            },
        )
    };
    graph.set_master(
        TrackParams::default(),
        vec![
            slot(EffectKind::Eq, true),
            slot(EffectKind::Tuner, true),
            slot(EffectKind::Tuner, false),
        ],
    );
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: root,
        velocity: 1.0,
    });
    render_mono(&mut engine, 9_600);
    let meters = handle.meters();
    let hz = meters.pitch(0, 1).unwrap_or(0.0);
    assert!((hz - 220.0).abs() < 2.0, "tuner heard {hz}");
    assert_eq!(meters.pitch(0, 0), None, "an EQ detects nothing");
    assert_eq!(meters.pitch(0, 2), None, "a bypassed tuner reports nothing");
}

fn midi_bytes(rx: &mut crate::MidiOutReceiver) -> Vec<[u8; 3]> {
    std::iter::from_fn(|| rx.pop()).map(|e| e.bytes).collect()
}

#[test]
fn midi_out_channels_send_sequenced_notes() {
    let (mut engine, mut handle) = engine_pair(SR);
    let mut rx = handle.take_midi_out().unwrap();
    assert!(handle.take_midi_out().is_none());
    let mut graph = Graph::new();
    graph.add_channel(Instrument::MidiOut(9), ChannelParams::default());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    engine.apply(EngineMessage::SetTempo(120.0));
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![note(1, 0)],
        384,
    ))));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, BEAT - 100);
    assert!(out.iter().all(|s| *s == 0.0), "MIDI Out makes no sound");
    assert!(midi_bytes(&mut rx).is_empty(), "nothing before beat 1");
    // Beat 1 starts the note; a sixteenth (6000 frames) later it ends.
    render_mono(&mut engine, 100 + BEAT / 8);
    assert_eq!(midi_bytes(&mut rx), vec![[0x99, 60, 127]]);
    render_mono(&mut engine, BEAT / 4);
    assert_eq!(midi_bytes(&mut rx), vec![[0x89, 60, 0]]);
}

#[test]
fn midi_out_live_notes_mute_and_stop() {
    let (mut engine, mut handle) = engine_pair(SR);
    let mut rx = handle.take_midi_out().unwrap();
    let mut graph = Graph::new();
    graph.add_channel(Instrument::MidiOut(0), ChannelParams::default());
    let muted = ChannelParams {
        mute: true,
        ..ChannelParams::default()
    };
    graph.add_channel(Instrument::MidiOut(1), muted);
    graph.add_channel(Instrument::Layer(vec![0]), ChannelParams::default());
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    for channel in [1, 2] {
        engine.apply(EngineMessage::NoteOn {
            channel,
            key: 64,
            velocity: 0.5,
        });
    }
    render_mono(&mut engine, 512);
    assert_eq!(
        midi_bytes(&mut rx),
        vec![[0x90, 64, 64]],
        "muted sends nothing; the layer plays channel 1"
    );
    engine.apply(EngineMessage::NoteOff {
        channel: 2,
        key: 64,
    });
    assert_eq!(midi_bytes(&mut rx), vec![[0x80, 64, 0]]);
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 67,
        velocity: 1.0,
    });
    engine.apply(EngineMessage::Stop);
    assert_eq!(midi_bytes(&mut rx), vec![[0x90, 67, 127], [0x80, 67, 0]]);
}

#[test]
fn rebuilding_keeps_only_matching_midi_notes() {
    let (mut engine, mut handle) = engine_pair(SR);
    let mut rx = handle.take_midi_out().unwrap();
    let build = |midi: u8| {
        let mut graph = Graph::new();
        graph.add_channel(Instrument::MidiOut(midi), ChannelParams::default());
        Box::new(graph)
    };
    engine.apply(EngineMessage::SetGraph(build(0)));
    engine.apply(EngineMessage::NoteOn {
        channel: 0,
        key: 60,
        velocity: 1.0,
    });
    engine.apply(EngineMessage::SetGraph(build(0)));
    assert_eq!(midi_bytes(&mut rx), vec![[0x90, 60, 127]], "still held");
    engine.apply(EngineMessage::SetGraph(build(3)));
    assert_eq!(midi_bytes(&mut rx), vec![[0x80, 60, 0]], "channel changed");
    handle.collect_garbage();
}

#[test]
fn pause_markers_stop_playback_on_their_tick_and_resume_past_them() {
    let (mut engine, handle) = impulse_engine();
    let beat = u64::from(PPQ);
    let seq = Sequence::new(vec![note(0, 0), note(2, 0), note(3, 0)], 4 * beat)
        .with_pauses(vec![2 * beat]);
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 3 * BEAT);
    // Beat 0 plays; playback stops on beat 2, before the note there.
    assert_eq!(nonzero_frames(&out), [0]);
    assert!(!handle.meters().is_playing());
    assert!((engine.position_ticks() - 2.0 * f64::from(PPQ)).abs() < 1e-9);
    assert_eq!(handle.meters().marker_pause(), (1, 2.0 * f64::from(PPQ)));
    // Play continues from the marker without pausing on it again.
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 2 * BEAT);
    assert_eq!(nonzero_frames(&out), [0, BEAT]);
    assert_eq!(handle.meters().marker_pause().0, 1);
}

#[test]
fn a_song_loop_point_wraps_there_but_stop_returns_home() {
    let (mut engine, _handle) = impulse_engine();
    let beat = u64::from(PPQ);
    let seq = Sequence::new(vec![note(0, 0), note(2, 0)], 4 * beat)
        .with_loop_start(2 * beat)
        .with_home(0);
    engine.apply(EngineMessage::SetSequence(Box::new(seq)));
    engine.apply(EngineMessage::Play);
    let out = render_mono(&mut engine, 6 * BEAT);
    // Beats 0 and 2, then back to beat 2 when the end (beat 4) is reached.
    assert_eq!(nonzero_frames(&out), [0, 2 * BEAT, 4 * BEAT]);
    engine.apply(EngineMessage::Stop);
    assert!(
        engine.position_ticks().abs() < 1e-9,
        "Stop returns to the start"
    );
}

#[test]
fn track_stereo_options_invert_swap_and_separate() {
    let block = |params: TrackParams| {
        let (mut l, mut r) = (vec![1.0_f32, 0.5], vec![0.0_f32, 0.5]);
        params.stereo(&mut l, &mut r);
        (l, r)
    };
    let base = TrackParams::default();
    assert_eq!(block(base), (vec![1.0, 0.5], vec![0.0, 0.5]), "untouched");
    let swapped = block(TrackParams { swap: true, ..base });
    assert_eq!(swapped, (vec![0.0, 0.5], vec![1.0, 0.5]));
    let inverted = block(TrackParams {
        invert: true,
        ..base
    });
    assert_eq!(inverted, (vec![-1.0, -0.5], vec![0.0, -0.5]));
    let mono = block(TrackParams {
        separation: -1.0,
        ..base
    });
    assert_eq!(mono, (vec![0.5, 0.5], vec![0.5, 0.5]));
    let wide = block(TrackParams {
        separation: 1.0,
        ..base
    });
    assert_eq!(wide, (vec![1.5, 0.5], vec![-0.5, 0.5]), "the side doubles");
}

fn midi_messages(rx: &mut crate::MidiOutReceiver) -> Vec<Vec<u8>> {
    std::iter::from_fn(|| rx.pop())
        .map(|e| e.message().to_vec())
        .collect()
}

#[test]
fn midi_clock_starts_ticks_and_stops() {
    let (mut engine, mut handle) = engine_pair(SR);
    let mut rx = handle.take_midi_out().unwrap();
    engine.apply(EngineMessage::SetGraph(Box::default()));
    engine.apply(EngineMessage::SetTempo(120.0));
    engine.apply(EngineMessage::SetSequence(Box::new(Sequence::new(
        vec![],
        384,
    ))));
    engine.apply(EngineMessage::SetMidiClock(true));
    engine.apply(EngineMessage::Play);
    // Just short of a beat, so the next beat's first clock is not included.
    render_mono(&mut engine, BEAT - 10);
    let got = midi_messages(&mut rx);
    assert_eq!(got.first(), Some(&vec![0xfa]), "start first");
    let clocks = got.iter().filter(|m| m.as_slice() == [0xf8]).count();
    assert_eq!(clocks, 24, "24 clocks per beat");
    engine.apply(EngineMessage::Stop);
    render_mono(&mut engine, BEAT);
    assert_eq!(
        midi_messages(&mut rx),
        vec![vec![0xfc]],
        "stop, then no clocks"
    );
}

#[test]
fn midi_out_sends_controls_and_program_when_they_change() {
    let (mut engine, mut handle) = engine_pair(SR);
    let mut rx = handle.take_midi_out().unwrap();
    let mut params = ChannelParams::default();
    params.instrument[inst::MIDI_CC1 + 1] = 100.0; // second knob: CC 7 by default
    params.instrument[inst::MIDI_PROGRAM] = 5.0;
    let mut graph = Graph::new();
    graph.add_channel(Instrument::MidiOut(3), params);
    engine.apply(EngineMessage::SetGraph(Box::new(graph)));
    render_mono(&mut engine, 256);
    assert_eq!(
        midi_messages(&mut rx),
        vec![vec![0xb3, 7, 100], vec![0xc3, 4]]
    );
    render_mono(&mut engine, 256);
    assert!(
        midi_messages(&mut rx).is_empty(),
        "unchanged values are not resent"
    );
}
