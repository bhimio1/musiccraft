use std::sync::Arc;

use musiccraft_dsp::ProcessContext;
use musiccraft_dsp::mastering::BandSplitter;
use musiccraft_meter::Meter;
use musiccraft_params::{EffectKind, PPQ, inst};
use rtrb::{Consumer, Producer, RingBuffer};

use crate::Start;
use crate::graph::{ChannelNode, ChannelParams, Instrument, TrackNode};
use crate::midi_out::{MidiOut, MidiOutReceiver, midi_out_pair};
use crate::{
    EngineMessage, External, ExternalTransport, GARBAGE_CAPACITY, Garbage, Graph, LIVE_CAPACITY,
    LiveEvent, MAX_BLOCK, MAX_EXTERNALS, MAX_VOICES, MESSAGE_CAPACITY, Meters, NoteEvent, Sequence,
};
use musiccraft_instruments::{Voice, VoiceStart};

/// The engine's external processor slots.
pub(crate) type Externals = [Option<Box<dyn External>>];

/// Safety cap on loop wraps handled within one block (for absurdly short loops).
const MAX_WRAPS_PER_BLOCK: usize = 64;
/// Metronome click length in seconds.
const CLICK_SECONDS: f64 = 0.03;
/// Metronome level.
const CLICK_GAIN: f32 = 0.35;

/// A synthesized metronome click: a short decaying sine, higher on the downbeat.
#[derive(Debug, Clone, Copy, Default)]
struct Click {
    /// Frames to wait before the click starts (within the current block).
    delay: Option<u32>,
    accent: bool,
    /// Frames already played; `None` when silent.
    played: Option<u32>,
}

impl Click {
    fn trigger(&mut self, delay: u32, accent: bool) {
        self.delay = Some(delay);
        self.accent = accent;
    }

    /// Adds the click into a block.
    fn render(&mut self, left: &mut [f32], right: &mut [f32], sample_rate: f64) {
        let length = (CLICK_SECONDS * sample_rate) as u32;
        let hz = if self.accent { 1_760.0 } else { 1_320.0 };
        for (i, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
            if self.delay == Some(i as u32) {
                self.delay = None;
                self.played = Some(0);
            }
            let Some(n) = self.played else {
                continue;
            };
            if n >= length {
                self.played = None;
                continue;
            }
            let t = f64::from(n) / sample_rate;
            let env = (-t / (CLICK_SECONDS / 5.0)).exp() as f32;
            let s = (t * hz * std::f64::consts::TAU).sin() as f32 * env * CLICK_GAIN;
            *l += s;
            *r += s;
            self.played = Some(n + 1);
        }
        // A delay past this block's end carries into the next block.
        if let Some(delay) = self.delay {
            self.delay = Some(delay.saturating_sub(left.len() as u32));
        }
    }
}

/// Main-thread side of an engine: sends messages, drops returned values.
pub struct EngineHandle {
    messages: Producer<EngineMessage>,
    garbage: Consumer<Garbage>,
    live: Option<LiveSender>,
    midi_out: Option<MidiOutReceiver>,
    meters: Arc<Meters>,
    sample_rate: u32,
}

impl EngineHandle {
    /// Queues a message for the engine.
    ///
    /// # Errors
    ///
    /// Returns the message back if the queue is full.
    // Messages are fixed-size values stored inline in the queue; handing a
    // rejected one back by value is the point, so its size is fine here.
    #[allow(clippy::result_large_err)]
    pub fn send(&mut self, message: EngineMessage) -> Result<(), EngineMessage> {
        self.messages
            .push(message)
            .map_err(|rtrb::PushError::Full(m)| m)
    }

    /// Drops every value the engine has handed back. Call regularly.
    pub fn collect_garbage(&mut self) -> usize {
        let mut count = 0;
        while let Ok(item) = self.garbage.pop() {
            drop(item);
            count += 1;
        }
        count
    }

    /// Free space in the message queue.
    #[must_use]
    pub fn free_slots(&self) -> usize {
        self.messages.slots()
    }

    /// Shared meters published by the engine.
    #[must_use]
    pub fn meters(&self) -> &Arc<Meters> {
        &self.meters
    }

    /// The engine's sample rate.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Takes the live-note sender (for a MIDI thread). Returns `None` after the
    /// first call.
    pub fn take_live_sender(&mut self) -> Option<LiveSender> {
        self.live.take()
    }

    /// Takes the receiver of MIDI Out channels' messages (for a MIDI output
    /// thread). Returns `None` after the first call.
    pub fn take_midi_out(&mut self) -> Option<MidiOutReceiver> {
        self.midi_out.take()
    }
}

/// Sends live notes straight to the engine from any one thread.
pub struct LiveSender {
    producer: Producer<LiveEvent>,
}

impl LiveSender {
    /// Queues a live event. Returns `false` if the queue is full.
    pub fn send(&mut self, event: LiveEvent) -> bool {
        self.producer.push(event).is_ok()
    }
}

/// The audio-thread side of the engine.
pub struct Engine {
    messages: Consumer<EngineMessage>,
    garbage: Producer<Garbage>,
    live: Consumer<LiveEvent>,
    meters: Arc<Meters>,
    sample_rate: f64,
    tempo_bpm: f64,
    playing: bool,
    position: f64,
    cursor: usize,
    frames_played: u64,
    graph: Option<Box<Graph>>,
    sequence: Option<Box<Sequence>>,
    voices: Vec<Voice>,
    voice_counter: u64,
    metronome: bool,
    /// Whether MIDI clock and start/stop go to the MIDI output.
    midi_clock: bool,
    /// Whether the transport was playing at the last block, for start/stop.
    clock_was_playing: bool,
    click: Click,
    preview: Option<Box<crate::Preview>>,
    externals: Vec<Option<Box<dyn External>>>,
    /// Beats elapsed on the free-running clock (used while stopped).
    free_beats: f64,
    /// Running state of each controller of the sequence.
    controller_states: [crate::controllers::ControllerState; crate::controllers::MAX_CONTROLLERS],
    /// Pitch of each channel's last note, for portamento notes.
    last_keys: Vec<Option<f32>>,
    /// Messages of MIDI Out channels.
    midi_out: MidiOut,
    /// How the next Play begins.
    start: Start,
    /// A count-in or wait in progress.
    pending: Pending,
    /// Stop at the loop end instead of wrapping.
    stop_at_end: bool,
}

/// What the engine does before the playhead moves after Play.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pending {
    None,
    /// Clicking: frames left, frames clicked so far, frames per beat and
    /// beats per bar.
    CountIn {
        left: u64,
        elapsed: u64,
        per_beat: f64,
        bar: u32,
    },
    /// Waiting for the first live note.
    Wait,
}

impl Drop for Engine {
    fn drop(&mut self) {
        // Let external processors end processing on the thread that ran them
        // (or the one tearing the engine down, which is now the only one).
        for external in self.externals.iter_mut().flatten() {
            external.stop();
        }
    }
}

/// Creates an engine and the handle that controls it. Call on the main thread,
/// then move the [`Engine`] into the audio callback.
#[must_use]
pub fn engine_pair(sample_rate: u32) -> (Engine, EngineHandle) {
    let sample_rate = sample_rate.max(1);
    let (message_tx, message_rx) = RingBuffer::new(MESSAGE_CAPACITY);
    let (garbage_tx, garbage_rx) = RingBuffer::new(GARBAGE_CAPACITY);
    let (live_tx, live_rx) = RingBuffer::new(LIVE_CAPACITY);
    let meters = Arc::new(Meters::new());
    let (midi_out, midi_out_rx) = midi_out_pair();
    let engine = Engine {
        messages: message_rx,
        garbage: garbage_tx,
        live: live_rx,
        meters: Arc::clone(&meters),
        sample_rate: f64::from(sample_rate),
        tempo_bpm: 130.0,
        playing: false,
        position: 0.0,
        cursor: 0,
        frames_played: 0,
        graph: None,
        sequence: None,
        voices: (0..MAX_VOICES)
            .map(|_| Voice::new(sample_rate as f32))
            .collect(),
        voice_counter: 0,
        metronome: false,
        midi_clock: false,
        clock_was_playing: false,
        click: Click::default(),
        preview: None,
        externals: (0..MAX_EXTERNALS).map(|_| None).collect(),
        free_beats: 0.0,
        controller_states: [crate::controllers::ControllerState::default();
            crate::controllers::MAX_CONTROLLERS],
        last_keys: vec![None; PORTA_CHANNELS],
        midi_out,
        start: Start::Now,
        pending: Pending::None,
        stop_at_end: false,
    };
    let handle = EngineHandle {
        messages: message_tx,
        garbage: garbage_rx,
        live: Some(LiveSender { producer: live_tx }),
        midi_out: Some(midi_out_rx),
        meters,
        sample_rate,
    };
    (engine, handle)
}

impl Engine {
    /// Whether the playhead is advancing.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Current tempo in beats per minute.
    #[must_use]
    pub fn tempo_bpm(&self) -> f64 {
        self.tempo_bpm
    }

    /// Playhead position in beats.
    #[must_use]
    pub fn position_beats(&self) -> f64 {
        self.position / f64::from(PPQ)
    }

    /// Playhead position in ticks.
    #[must_use]
    pub fn position_ticks(&self) -> f64 {
        self.position
    }

    /// Total frames rendered while playing.
    #[must_use]
    pub fn frames_played(&self) -> u64 {
        self.frames_played
    }

    /// Number of sounding voices.
    #[must_use]
    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Active voices on `channel`, cut or not.
    #[cfg(test)]
    pub(crate) fn voices_on(&self, channel: u16) -> usize {
        self.voices
            .iter()
            .filter(|v| v.active && v.channel == channel)
            .count()
    }

    /// The keys of the sounding (not released) voices, lowest first.
    #[cfg(test)]
    pub(crate) fn held_keys(&self) -> Vec<u8> {
        let mut keys: Vec<u8> = self
            .voices
            .iter()
            .filter(|v| v.held())
            .map(|v| v.key)
            .collect();
        keys.sort_unstable();
        keys
    }

    fn ticks_per_frame(&self) -> f64 {
        self.tempo_bpm / 60.0 * f64::from(PPQ) / self.sample_rate
    }

    fn loop_length(&self) -> f64 {
        match self.sequence.as_deref() {
            Some(seq) if seq.length() > 0 => seq.length() as f64,
            _ => f64::from(4 * PPQ),
        }
    }

    /// Where Stop returns to, in ticks: the loop start unless the sequence
    /// names another home.
    fn home(&self) -> f64 {
        let home = self.sequence.as_deref().map_or(0, Sequence::home) as f64;
        if home < self.loop_length() { home } else { 0.0 }
    }

    /// Where the loop restarts, in ticks.
    fn loop_start(&self) -> f64 {
        let start = self.sequence.as_deref().map_or(0, Sequence::loop_start) as f64;
        if start < self.loop_length() {
            start
        } else {
            0.0
        }
    }

    /// Folds a position into the loop.
    fn fold(&self, position: f64) -> f64 {
        let (start, end) = (self.loop_start(), self.loop_length());
        if position < end {
            position
        } else {
            start + (position - start) % (end - start).max(1.0)
        }
    }

    fn resync_cursor(&mut self) {
        self.cursor = self
            .sequence
            .as_deref()
            .map_or(0, |seq| seq.first_at_or_after(self.position));
    }

    fn release_all(&mut self, force: bool) {
        for voice in &mut self.voices {
            if voice.active {
                voice.release(force);
            }
        }
        for external in self.externals.iter_mut().flatten() {
            external.all_notes_off();
        }
        self.midi_out.all_off();
    }

    /// Whether external slot `slot` holds a processor.
    #[must_use]
    pub fn has_external(&self, slot: usize) -> bool {
        self.externals.get(slot).is_some_and(Option::is_some)
    }

    fn discard(&mut self, garbage: Garbage) {
        // `drain_messages` guarantees a free slot before applying a message that
        // produces garbage, so this push cannot fail when called from there.
        if let Err(rtrb::PushError::Full(item)) = self.garbage.push(garbage) {
            // Unreachable in practice; never drop on the audio thread.
            std::mem::forget(item);
        }
    }

    /// Applies one message immediately.
    pub fn apply(&mut self, message: EngineMessage) {
        match message {
            EngineMessage::Play => {
                if !self.playing && self.pending == Pending::None {
                    match self.start {
                        Start::CountIn(bars) if bars > 0 => {
                            let sig = self.meter().sig_at(self.position.max(0.0) as u32);
                            let per_beat = self.frames_per_beat() * f64::from(sig.beat_ticks())
                                / f64::from(PPQ);
                            let beats = bars.saturating_mul(sig.num());
                            let frames = f64::from(beats) * per_beat;
                            self.pending = Pending::CountIn {
                                left: (frames.round() as u64).max(1),
                                elapsed: 0,
                                per_beat,
                                bar: sig.num(),
                            };
                        }
                        Start::WaitForInput => self.pending = Pending::Wait,
                        _ => self.begin(),
                    }
                }
            }
            EngineMessage::SetStart(start) => self.start = start,
            EngineMessage::SetStopAtEnd(on) => self.stop_at_end = on,
            EngineMessage::Stop => {
                self.pending = Pending::None;
                self.playing = false;
                self.position = self.home();
                self.resync_cursor();
                self.release_all(true);
            }
            EngineMessage::Pause => {
                self.pending = Pending::None;
                self.playing = false;
                self.release_all(true);
            }
            EngineMessage::SetTempo(bpm) => {
                if bpm.is_finite() && bpm > 0.0 {
                    self.tempo_bpm = bpm;
                }
            }
            EngineMessage::Seek(ticks) => {
                if ticks.is_finite() {
                    self.position = self.fold(ticks.max(0.0));
                    self.resync_cursor();
                }
            }
            EngineMessage::Channel { channel, params } => {
                if let Some(node) = self
                    .graph
                    .as_deref_mut()
                    .and_then(|g| g.channels.get_mut(usize::from(channel)))
                {
                    node.params = params;
                }
            }
            EngineMessage::Track { track, params } => {
                if let Some(node) = self
                    .graph
                    .as_deref_mut()
                    .and_then(|g| g.tracks.get_mut(usize::from(track)))
                {
                    node.params = params;
                }
            }
            EngineMessage::Effect {
                track,
                slot,
                params,
            } => {
                if let Some(slot) = self
                    .graph
                    .as_deref_mut()
                    .and_then(|g| g.tracks.get_mut(usize::from(track)))
                    .and_then(|t| t.effects.get_mut(usize::from(slot)))
                {
                    slot.params = params;
                }
            }
            EngineMessage::RackEffect {
                track,
                slot,
                branch,
                index,
                params,
            } => {
                if let Some(musiccraft_dsp::Effect::Rack(rack)) = self
                    .graph
                    .as_deref_mut()
                    .and_then(|g| g.tracks.get_mut(usize::from(track)))
                    .and_then(|t| t.effects.get_mut(usize::from(slot)))
                    .map(|s| &mut s.effect)
                {
                    rack.set(
                        usize::from(branch),
                        usize::from(index),
                        params.enabled,
                        params.values,
                    );
                }
            }
            EngineMessage::NoteOn {
                channel,
                key,
                velocity,
            } => self.live_note_on(channel, key, velocity),
            EngineMessage::NoteOff { channel, key } => self.live_note_off(channel, key),
            EngineMessage::AllNotesOff => self.release_all(true),
            EngineMessage::SetMetronome(on) => self.metronome = on,
            EngineMessage::SetMidiClock(on) => {
                if !on && self.clock_was_playing {
                    self.midi_out.realtime(crate::midi_out::STOP, 0);
                }
                self.midi_clock = on;
                self.clock_was_playing = on && self.playing;
            }
            EngineMessage::PlayPreview(preview) => {
                if let Some(old) = self.preview.replace(preview) {
                    self.discard(Garbage::Preview(old));
                }
            }
            EngineMessage::StopPreview => {
                if let Some(preview) = self.preview.as_deref_mut() {
                    preview.position = f64::INFINITY;
                }
            }
            EngineMessage::SetExternal { slot, external } => {
                if let Some(entry) = self.externals.get_mut(usize::from(slot)) {
                    let old = std::mem::replace(entry, external);
                    if let Some(mut old) = old {
                        old.stop();
                        self.discard(Garbage::External(old));
                    }
                } else if let Some(external) = external {
                    // Out of range: hand it straight back.
                    self.discard(Garbage::External(external));
                }
            }
            EngineMessage::ExternalParam { slot, id, value } => {
                if let Some(Some(external)) = self.externals.get_mut(usize::from(slot)) {
                    external.set_param(id, value);
                }
            }
            EngineMessage::ExternalData { slot, data } => {
                let back = match self.externals.get_mut(usize::from(slot)) {
                    Some(Some(external)) => external.receive(data),
                    _ => data,
                };
                self.discard(Garbage::Data(back));
            }
            EngineMessage::SetGraph(graph) => {
                for voice in &mut self.voices {
                    voice.active = false;
                }
                for external in self.externals.iter_mut().flatten() {
                    external.all_notes_off();
                }
                // Held MIDI notes survive a rebuild only when their channel
                // still sends on the same MIDI channel.
                let channels = &graph.channels;
                self.midi_out.release_unless(|channel, midi| {
                    matches!(
                        channels.get(usize::from(channel)).map(|n| &n.instrument),
                        Some(Instrument::MidiOut(m)) if *m == midi
                    )
                });
                if let Some(old) = self.graph.replace(graph) {
                    self.discard(Garbage::Graph(old));
                }
            }
            EngineMessage::SetSequence(sequence) => {
                let old = self.sequence.replace(sequence);
                self.position = self.fold(self.position);
                self.resync_cursor();
                if let Some(old) = old {
                    self.discard(Garbage::Sequence(old));
                }
            }
        }
    }

    fn drain_messages(&mut self) {
        while let Ok(message) = self.messages.peek() {
            if message.returns_garbage() && self.garbage.is_full() {
                // Wait for the main thread to collect garbage; never drop here.
                break;
            }
            match self.messages.pop() {
                Ok(message) => self.apply(message),
                Err(_) => break,
            }
        }
        while let Ok(event) = self.live.pop() {
            match event {
                LiveEvent::NoteOn {
                    channel,
                    key,
                    velocity,
                } => self.live_note_on(channel, key, velocity),
                LiveEvent::NoteOff { channel, key } => self.live_note_off(channel, key),
            }
        }
    }

    /// Starts moving the playhead.
    fn begin(&mut self) {
        self.pending = Pending::None;
        self.playing = true;
        self.resync_cursor();
    }

    /// Frames per quarter note.
    fn frames_per_beat(&self) -> f64 {
        self.sample_rate * 60.0 / self.tempo_bpm.max(1.0)
    }

    /// The bars and beats of what is playing.
    fn meter(&self) -> &Meter {
        static COMMON: Meter = Meter::COMMON;
        self.sequence.as_deref().map_or(&COMMON, Sequence::meter)
    }

    /// Clicks the count-in beats that fall in the next `frames` frames.
    fn click_count_in(&mut self, frames: usize) {
        let Pending::CountIn {
            elapsed,
            per_beat,
            bar,
            ..
        } = self.pending
        else {
            return;
        };
        let per_beat = per_beat.max(1.0);
        let end = elapsed + frames as u64;
        let mut beat = (elapsed as f64 / per_beat).ceil() as u64;
        loop {
            let at = (beat as f64 * per_beat).round() as u64;
            if at >= end {
                break;
            }
            if at >= elapsed {
                self.click
                    .trigger((at - elapsed) as u32, beat % u64::from(bar.max(1)) == 0);
            }
            beat += 1;
        }
    }

    /// Advances a count-in by `frames`; the playhead starts when it ends.
    fn advance_count_in(&mut self, frames: usize) {
        if let Pending::CountIn {
            left,
            elapsed,
            per_beat,
            bar,
        } = self.pending
        {
            let frames = frames as u64;
            if frames >= left {
                self.begin();
            } else {
                self.pending = Pending::CountIn {
                    left: left - frames,
                    elapsed: elapsed + frames,
                    per_beat,
                    bar,
                };
            }
        }
    }

    /// Frames until a count-in ends, if one is running.
    fn count_in_left(&self) -> Option<usize> {
        match self.pending {
            Pending::CountIn { left, .. } => Some(usize::try_from(left).unwrap_or(usize::MAX)),
            _ => None,
        }
    }

    fn live_note_on(&mut self, channel: u16, key: u8, velocity: f32) {
        // The first live note starts playback that waits for input.
        if self.pending == Pending::Wait {
            self.begin();
        }
        let start = VoiceStart {
            shape: crate::NoteShape::default(),
            slide: false,
            channel,
            key,
            velocity,
            delay: 0,
            gate: None,
            live: true,
            pan: 0.0,
            glide_from: None,
        };
        if let Some(graph) = self.graph.as_deref() {
            trigger(
                &mut self.voices,
                &mut self.voice_counter,
                &graph.channels,
                &mut self.externals,
                &mut self.midi_out,
                &mut self.last_keys,
                start,
            );
        }
    }

    fn live_note_off(&mut self, channel: u16, key: u8) {
        // A layer releases the notes it started on its channels.
        let mut targets = [channel; MAX_LAYER + 1];
        let mut count = 1;
        if let Some(Instrument::Layer(children)) = self
            .graph
            .as_deref()
            .and_then(|g| g.channels.get(usize::from(channel)))
            .map(|n| &n.instrument)
        {
            for child in children.iter().take(MAX_LAYER) {
                if let Some(slot) = targets.get_mut(count) {
                    *slot = *child;
                    count += 1;
                }
            }
        }
        for target in targets.iter().take(count).copied() {
            if let Some(external) = self.channel_external(target) {
                external.note_off(0, key);
            }
            self.midi_out.note_off(target, key);
            for voice in &mut self.voices {
                if voice.active && voice.live && voice.channel == target && voice.key == key {
                    voice.release(true);
                }
            }
        }
    }

    /// The external processor playing `channel`, if it is a plugin channel.
    fn channel_external(&mut self, channel: u16) -> Option<&mut Box<dyn External>> {
        let node = self.graph.as_deref()?.channels.get(usize::from(channel))?;
        let Instrument::External(slot) = node.instrument else {
            return None;
        };
        self.externals.get_mut(usize::from(slot))?.as_mut()
    }

    /// Fills an interleaved output buffer with `channels` channels. The first two
    /// channels receive left and right; any others are silent; mono receives the
    /// average of left and right.
    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        let started = std::time::Instant::now();
        self.drain_messages();
        output.fill(0.0);
        if channels == 0 {
            return;
        }
        for block in output.chunks_mut(channels * MAX_BLOCK) {
            let total = block.len() / channels;
            let mut done = 0;
            while done < total {
                // A count-in ending inside the block splits it there, so the
                // playhead starts on the exact frame.
                let frames = self
                    .count_in_left()
                    .map_or(total - done, |left| left.clamp(1, total - done));
                if let Some(part) = block.get_mut(done * channels..(done + frames) * channels) {
                    self.render_block(frames);
                    self.advance_count_in(frames);
                    self.write_block(part, channels, frames);
                    if let Some(master) = self.graph.as_deref().and_then(|g| g.tracks.first()) {
                        let mono = master.left.iter().zip(&master.right).take(frames);
                        self.meters.push_scope(mono.map(|(l, r)| (l + r) * 0.5));
                    }
                }
                done += frames;
            }
        }
        let budget = (output.len() / channels) as f64 / self.sample_rate;
        if budget > 0.0 {
            self.meters
                .set_load((started.elapsed().as_secs_f64() / budget) as f32);
        }
        let voices = self.voices.iter().filter(|v| v.active).count() as u32;
        self.meters.publish(self.position, self.playing, voices);
        self.meters.publish_start(
            matches!(self.pending, Pending::CountIn { .. }),
            self.pending == Pending::Wait,
        );
    }

    fn write_block(&self, block: &mut [f32], channels: usize, frames: usize) {
        let Some(master) = self.graph.as_deref().and_then(|g| g.tracks.first()) else {
            return;
        };
        let frames_out = block.chunks_mut(channels).take(frames);
        for ((frame, l), r) in frames_out.zip(&master.left).zip(&master.right) {
            let (l, r) = (sanitize(*l), sanitize(*r));
            match frame {
                [mono] => *mono = (l + r) * 0.5,
                [left, right, ..] => {
                    *left = l;
                    *right = r;
                }
                [] => {}
            }
        }
    }

    fn render_block(&mut self, frames: usize) {
        let frames = frames.min(MAX_BLOCK);
        let bar = self.meter().bar(self.position.max(0.0) as u32);
        let transport = ExternalTransport {
            playing: self.playing,
            tempo_bpm: self.tempo_bpm,
            position_beats: self.position_beats(),
            sample_rate: self.sample_rate,
            time_signature: bar.sig,
            bar_start_beats: f64::from(bar.start) / f64::from(PPQ),
            bar_number: bar.index,
        };
        if self.midi_clock && self.playing != self.clock_was_playing {
            let byte = if !self.playing {
                crate::midi_out::STOP
            } else if self.position <= 0.0 {
                crate::midi_out::START
            } else {
                crate::midi_out::CONTINUE
            };
            self.midi_out.realtime(byte, 0);
            self.clock_was_playing = self.playing;
        }
        if self.playing {
            self.schedule(frames);
            self.frames_played += frames as u64;
        }
        self.click_count_in(frames);
        self.midi_out.advance(frames);
        let sample_rate = self.sample_rate as f32;
        // Tempo-synced effects follow the song while playing and a free
        // clock at the same tempo while stopped.
        let beats = if transport.playing {
            transport.position_beats
        } else {
            self.free_beats
        };
        self.free_beats += frames as f64 * self.tempo_bpm / 60.0 / self.sample_rate;
        let ctx = ProcessContext {
            beats,
            sample_rate,
            tempo_bpm: self.tempo_bpm,
        };
        let Some(graph) = self.graph.as_deref_mut() else {
            return;
        };
        if let Some(sequence) = self.sequence.as_deref() {
            if self.playing {
                apply_automation(graph, &mut self.externals, sequence, self.position);
            }
            let cx = crate::controllers::ControlContext {
                beats,
                seconds: frames as f32 / sample_rate.max(1.0),
                voices: &self.voices,
            };
            crate::controllers::apply_controllers(
                graph,
                &mut self.externals,
                sequence,
                &mut self.controller_states,
                &cx,
            );
        }
        send_midi_controls(graph, &mut self.midi_out);
        let Graph {
            channels,
            tracks,
            schedule,
        } = graph;
        for node in channels.iter_mut() {
            clear(&mut node.left, frames);
            clear(&mut node.right, frames);
        }
        for track in tracks.iter_mut() {
            clear(&mut track.left, frames);
            clear(&mut track.right, frames);
        }
        for voice in self.voices.iter_mut().filter(|v| v.active) {
            match channels.get_mut(usize::from(voice.channel)) {
                Some(ChannelNode {
                    instrument,
                    params,
                    left,
                    right,
                    ..
                }) => {
                    let (l, r) = (prefix(left, frames), prefix(right, frames));
                    voice.render(instrument, &params.instrument, l, r, sample_rate);
                }
                None => voice.active = false,
            }
        }
        for node in channels.iter_mut() {
            if let Instrument::External(slot) = node.instrument {
                if let Some(Some(external)) = self.externals.get_mut(usize::from(slot)) {
                    let pitch = node.params.instrument.get(inst::PITCH).copied();
                    external.set_transpose(pitch.unwrap_or(0.0));
                    let (l, r) = (
                        prefix(&mut node.left, frames),
                        prefix(&mut node.right, frames),
                    );
                    external.process(l, r, &transport);
                    // A plugin's bad output must not reach effect tails.
                    scrub(l, r);
                }
            }
        }
        mix_channels(channels, tracks, frames);
        let fx = FxContext {
            frames,
            ctx,
            transport,
            voices: &self.voices,
        };
        schedule.build(tracks);
        mix_tracks(
            tracks,
            &schedule.order,
            fx,
            &mut self.externals,
            &self.meters,
        );
        if let Some(master) = tracks.first_mut() {
            let TrackNode { left, right, .. } = master;
            let (l, r) = (prefix(left, frames), prefix(right, frames));
            self.click.render(l, r, self.sample_rate);
            if let Some(preview) = self.preview.as_deref_mut() {
                preview.render(l, r, self.sample_rate);
            }
        }
    }

    /// Starts every sequence event that falls inside the next `frames` frames and
    /// advances the playhead, wrapping at the loop end.
    fn schedule(&mut self, frames: usize) {
        let tpf = self.ticks_per_frame();
        let length = self.loop_length();
        let total = frames as f64;
        let mut done = 0.0;
        let mut position = self.position;
        let loop_start = self.loop_start();
        let mut ended = false;
        for _ in 0..MAX_WRAPS_PER_BLOCK {
            let segment_end = position + (total - done) * tpf;
            let wraps = segment_end >= length;
            let mut stop = if wraps { length } else { segment_end };
            // A pause marker inside this stretch ends playback there.
            let pause = self
                .sequence
                .as_deref()
                .and_then(|seq| seq.next_pause(position, stop));
            if let Some(at) = pause {
                stop = at;
            }
            if self.midi_clock {
                // 24 clocks per quarter note.
                let step = f64::from(PPQ) / 24.0;
                let mut tick = (position / step).ceil() * step;
                while tick < stop {
                    let offset = done + (tick - position) / tpf;
                    self.midi_out.realtime(
                        crate::midi_out::CLOCK,
                        (offset - 1e-6).ceil().max(0.0) as u32,
                    );
                    tick += step;
                }
            }
            if self.metronome {
                let mut beat = self.meter().next_beat(position);
                while f64::from(beat.tick) < stop {
                    let offset = done + (f64::from(beat.tick) - position) / tpf;
                    self.click
                        .trigger((offset - 1e-6).ceil().max(0.0) as u32, beat.downbeat);
                    let next = self.meter().next_beat(f64::from(beat.tick) + 1.0);
                    if next.tick <= beat.tick {
                        break;
                    }
                    beat = next;
                }
            }
            if let (Some(seq), Some(graph)) = (self.sequence.as_deref(), self.graph.as_deref()) {
                let events = seq.events();
                while let Some(event) = events.get(self.cursor) {
                    let start = event.start as f64;
                    if start >= stop {
                        break;
                    }
                    if start >= position {
                        let offset = done + (start - position) / tpf;
                        let start = event_start(event, offset, tpf);
                        trigger(
                            &mut self.voices,
                            &mut self.voice_counter,
                            &graph.channels,
                            &mut self.externals,
                            &mut self.midi_out,
                            &mut self.last_keys,
                            start,
                        );
                    }
                    self.cursor += 1;
                }
            }
            if let Some(at) = pause {
                position = at;
                self.playing = false;
                self.release_all(true);
                self.meters.record_marker_pause(at);
                break;
            }
            if !wraps {
                position = segment_end;
                break;
            }
            if self.stop_at_end {
                // Recording without looping: stop at the end and rewind.
                self.playing = false;
                self.release_all(true);
                self.meters.record_end_stop();
                position = self.home();
                ended = true;
                break;
            }
            done += (length - position) / tpf;
            position = loop_start;
            self.cursor = self
                .sequence
                .as_deref()
                .map_or(0, |seq| seq.first_at_or_after(loop_start));
            if done >= total {
                break;
            }
        }
        self.position = position;
        if ended {
            self.resync_cursor();
        }
    }
}

/// Writes every automation lane's current value into the graph.
fn apply_automation(
    graph: &mut Graph,
    externals: &mut Externals,
    sequence: &Sequence,
    position: f64,
) {
    for lane in sequence.automation() {
        if let Some(value) = lane.value_at(position) {
            write_target(graph, externals, lane.target, value);
        }
    }
}

/// MIDI Out channels send each control change and their program when its
/// value differs from what they last sent (muted channels send nothing).
fn send_midi_controls(graph: &mut Graph, midi_out: &mut MidiOut) {
    use musiccraft_params::MIDI_CCS;
    for node in &mut graph.channels {
        let Instrument::MidiOut(midi_channel) = node.instrument else {
            continue;
        };
        if node.params.mute {
            continue;
        }
        let value = |i: usize| {
            node.params
                .instrument
                .get(i)
                .copied()
                .unwrap_or(-1.0)
                .round()
        };
        for k in 0..MIDI_CCS {
            let cc = value(inst::MIDI_CC1 + k);
            if !(0.0..=127.0).contains(&cc) {
                continue;
            }
            let cc = cc as u8;
            let number = value(inst::MIDI_CC1_NUMBER + k).clamp(0.0, 127.0) as u8;
            if let Some(sent) = node.midi_sent.get_mut(k) {
                if *sent != cc {
                    *sent = cc;
                    midi_out.control(midi_channel, number, cc);
                }
            }
        }
        let program = value(inst::MIDI_PROGRAM);
        if (1.0..=128.0).contains(&program) {
            let program = (program - 1.0) as u8;
            if let Some(sent) = node.midi_sent.get_mut(MIDI_CCS) {
                if *sent != program {
                    *sent = program;
                    midi_out.program(midi_channel, program);
                }
            }
        }
    }
}

/// Writes `value` (real units) into the parameter `target` names.
pub(crate) fn write_target(
    graph: &mut Graph,
    externals: &mut Externals,
    target: crate::AutomationTarget,
    value: f32,
) {
    use crate::AutomationTarget as T;
    {
        match target {
            T::ExternalParam(c, id) => {
                let slot = match graph.channels.get(usize::from(c)).map(|n| &n.instrument) {
                    Some(Instrument::External(slot)) => usize::from(*slot),
                    _ => return,
                };
                if let Some(Some(external)) = externals.get_mut(slot) {
                    external.set_param(id, f64::from(value));
                }
            }
            T::ChannelVolume(c) => {
                if let Some(node) = graph.channels.get_mut(usize::from(c)) {
                    node.params.volume = value;
                }
            }
            T::ChannelPan(c) => {
                if let Some(node) = graph.channels.get_mut(usize::from(c)) {
                    node.params.pan = value;
                }
            }
            T::ChannelParam(c, p) => {
                if let Some(slot) = graph
                    .channels
                    .get_mut(usize::from(c))
                    .and_then(|n| n.params.instrument.get_mut(usize::from(p)))
                {
                    *slot = value;
                }
            }
            T::TrackVolume(t) => {
                if let Some(track) = graph.tracks.get_mut(usize::from(t)) {
                    track.params.volume = value;
                }
            }
            T::TrackPan(t) => {
                if let Some(track) = graph.tracks.get_mut(usize::from(t)) {
                    track.params.pan = value;
                }
            }
            T::EffectParam(t, s, p) => {
                if let Some(slot) = graph
                    .tracks
                    .get_mut(usize::from(t))
                    .and_then(|track| track.effects.get_mut(usize::from(s)))
                    .and_then(|slot| slot.params.values.get_mut(usize::from(p)))
                {
                    *slot = value;
                }
            }
        }
    }
}

fn event_start(event: &NoteEvent, offset_frames: f64, tpf: f64) -> VoiceStart {
    VoiceStart {
        shape: event.shape,
        channel: event.channel,
        key: event.key,
        velocity: event.velocity,
        // First frame at or after the event time, tolerant of rounding error.
        delay: (offset_frames - 1e-6).ceil().max(0.0) as u32,
        gate: Some((event.length as f64 / tpf).max(1.0) as u64),
        live: false,
        pan: event.pan,
        glide_from: None,
        slide: event.slide,
    }
}

/// Most channels whose last note is remembered for portamento.
const PORTA_CHANNELS: usize = 512;

/// Most channels one layer plays.
const MAX_LAYER: usize = 16;

fn trigger(
    voices: &mut [Voice],
    counter: &mut u64,
    channels: &[ChannelNode],
    externals: &mut Externals,
    midi_out: &mut MidiOut,
    last_keys: &mut [Option<f32>],
    mut start: VoiceStart,
) {
    // A layer passes the note on to its channels (layers do not nest).
    if let Some(ChannelNode {
        instrument: Instrument::Layer(children),
        ..
    }) = channels.get(usize::from(start.channel))
    {
        for child in children.iter().take(MAX_LAYER) {
            let nested = matches!(
                channels.get(usize::from(*child)).map(|n| &n.instrument),
                Some(Instrument::Layer(_))
            );
            if !nested {
                let mut forwarded = start;
                forwarded.channel = *child;
                trigger(
                    voices, counter, channels, externals, midi_out, last_keys, forwarded,
                );
            }
        }
        return;
    }
    let Some(node) = channels.get(usize::from(start.channel)) else {
        return;
    };
    // A slide note on an instrument that can bend starts no sound, so it cuts
    // nothing; every other note cuts the channels its cut group chokes.
    let bends = start.slide && musiccraft_instruments::glides(&node.instrument);
    if !bends {
        cut_group(voices, channels, &node.params, start.delay);
    }
    if let Some(ChannelNode {
        instrument: Instrument::MidiOut(midi_channel),
        params,
        ..
    }) = channels.get(usize::from(start.channel))
    {
        // A muted channel sends nothing new.
        if !params.mute {
            midi_out.note_on(
                start.channel,
                *midi_channel,
                start.key,
                start.velocity,
                start.delay,
                start.gate,
            );
        }
        return;
    }
    if let Some(ChannelNode {
        instrument: Instrument::External(slot),
        ..
    }) = channels.get(usize::from(start.channel))
    {
        // Plugins voice their own notes.
        if let Some(Some(external)) = externals.get_mut(usize::from(*slot)) {
            external.note_on(
                start.delay,
                start.key,
                start.velocity,
                start.pan,
                start.gate,
            );
        }
        return;
    }
    // A slide note bends the sounding notes of its channel and colour group
    // and never starts a voice. Instruments whose pitch is fixed when a note
    // starts (kit pads, drum synth, pluck, sliced samples) play it as a normal
    // note.
    let slides = start.slide
        && channels
            .get(usize::from(start.channel))
            .is_some_and(|node| musiccraft_instruments::glides(&node.instrument));
    if slides {
        slide(voices, last_keys, &start);
        return;
    }
    // A portamento note glides from the channel's previous note, held or not.
    let previous = last_keys.get(usize::from(start.channel)).copied().flatten();
    if let Some(slot) = last_keys.get_mut(usize::from(start.channel)) {
        *slot = Some(f32::from(start.key) + start.shape.fine);
    }
    if start.shape.porta {
        start.glide_from = previous;
        make_room(voices, &node.params, start.channel, start.delay);
        if let Some(voice) = free_voice(voices) {
            *counter += 1;
            voice.start(start, &node.instrument, &node.params.instrument, *counter);
        }
        return;
    }
    // A held note on the same channel is where a glide starts from.
    start.glide_from = voices
        .iter()
        .filter(|v| v.channel == start.channel && v.held())
        .max_by_key(|v| v.age)
        .map(Voice::current_key);
    make_room(voices, &node.params, start.channel, start.delay);
    if let Some(voice) = free_voice(voices) {
        *counter += 1;
        voice.start(start, &node.instrument, &node.params.instrument, *counter);
    }
}

/// Applies a slide note: the top note of its channel and colour group glides
/// to the slide note's key and the others keep their interval to it, each over
/// its overlap with the slide note. With nothing to bend it is silent.
fn slide(voices: &mut [Voice], last_keys: &mut [Option<f32>], start: &VoiceStart) {
    let length = start.gate.unwrap_or(1);
    let bends = |v: &Voice| {
        v.channel == start.channel
            && v.color() == start.shape.color
            && v.pitched()
            && v.sounding_for(start.delay).is_some()
    };
    let Some(top) = voices.iter().filter(|v| bends(v)).map(|v| v.key).max() else {
        return;
    };
    let semitones = i16::from(start.key) - i16::from(top);
    for voice in voices.iter_mut().filter(|v| bends(v)) {
        let overlap = voice.sounding_for(start.delay).unwrap_or(1).min(length);
        voice.slide_by(semitones, start.delay, overlap);
    }
    // Later portamento notes glide from where the slide ends.
    if let Some(slot) = last_keys.get_mut(usize::from(start.channel)) {
        *slot = Some(f32::from(start.key) + start.shape.fine);
    }
}

/// An integer channel parameter (cut groups, polyphony).
fn int_param(params: &ChannelParams, index: usize) -> u32 {
    params
        .instrument
        .get(index)
        .map_or(0, |v| v.round().max(0.0) as u32)
}

/// Cuts, `at` frames into the block, every voice of every channel that the
/// starting note's cut group chokes.
fn cut_group(voices: &mut [Voice], channels: &[ChannelNode], params: &ChannelParams, at: u32) {
    let group = int_param(params, inst::CUT_GROUP);
    if group == 0 {
        return;
    }
    for voice in voices.iter_mut().filter(|v| v.active) {
        let choked = channels
            .get(usize::from(voice.channel))
            .is_some_and(|node| int_param(&node.params, inst::CUT_BY) == group);
        if choked {
            voice.cut(at);
        }
    }
}

/// Cuts the channel's oldest voices until a new one fits its mono or
/// polyphony limit.
fn make_room(voices: &mut [Voice], params: &ChannelParams, channel: u16, at: u32) {
    let limit = if int_param(params, inst::MONO) > 0 {
        1
    } else {
        int_param(params, inst::POLY_MAX) as usize
    };
    if limit == 0 {
        return;
    }
    let sounding = |v: &Voice| v.active && v.channel == channel && !v.is_cut();
    let mut count = voices.iter().filter(|v| sounding(v)).count();
    while count >= limit {
        let Some(oldest) = voices
            .iter_mut()
            .filter(|v| sounding(v))
            .min_by_key(|v| v.age)
        else {
            break;
        };
        oldest.cut(at);
        count -= 1;
    }
}

/// An idle voice, or the oldest one to steal.
fn free_voice(voices: &mut [Voice]) -> Option<&mut Voice> {
    let index = voices.iter().position(|v| !v.active).or_else(|| {
        voices
            .iter()
            .enumerate()
            .min_by_key(|(_, v)| v.age)
            .map(|(i, _)| i)
    })?;
    voices.get_mut(index)
}

fn mix_channels(channels: &mut [ChannelNode], tracks: &mut [TrackNode], frames: usize) {
    let track_count = tracks.len();
    for node in channels.iter_mut() {
        let from = node.gains;
        let to = node.params.gains();
        node.gains = to;
        if from == (0.0, 0.0) && to == (0.0, 0.0) {
            continue;
        }
        let target = usize::from(node.params.track);
        let target = if target < track_count { target } else { 0 };
        let Some(track) = tracks.get_mut(target) else {
            continue;
        };
        add_ramped(&mut track.left, &node.left, from.0, to.0, frames);
        add_ramped(&mut track.right, &node.right, from.1, to.1, frames);
    }
}

/// Adds `src * gain` into `dest`, the gain moving linearly from `from` to `to`
/// across the block so changes never click.
fn add_ramped(dest: &mut [f32], src: &[f32], from: f32, to: f32, frames: usize) {
    if from == to {
        add_scaled(dest, src, to, frames);
        return;
    }
    let step = (to - from) / frames.max(1) as f32;
    let mut gain = from;
    for (d, s) in dest.iter_mut().zip(src).take(frames) {
        gain += step;
        *d += *s * gain;
    }
}

/// Multiplies `buffer` by a gain ramping from `from` to `to`.
fn scale_ramped(buffer: &mut [f32], from: f32, to: f32) {
    let step = (to - from) / buffer.len().max(1) as f32;
    let mut gain = from;
    for s in buffer.iter_mut() {
        gain += step;
        *s *= gain;
    }
}

/// What effect processing needs to know about the current block.
#[derive(Clone, Copy)]
struct FxContext<'a> {
    frames: usize,
    ctx: ProcessContext,
    transport: ExternalTransport,
    /// The voices, for effects that follow a channel's notes.
    voices: &'a [Voice],
}

/// Processes the inserts in `order` (see [`crate::graph::Schedule`]), then
/// the master. Every insert runs after the inserts that send to it or key it,
/// so it has all of its input and its sidechain keys are finished.
fn mix_tracks(
    tracks: &mut [TrackNode],
    order: &[u16],
    fx: FxContext<'_>,
    externals: &mut Externals,
    meters: &Meters,
) {
    let frames = fx.frames;
    for &i in order {
        let i = usize::from(i);
        if i == 0 || i >= tracks.len() {
            continue;
        }
        let (before, rest) = tracks.split_at_mut(i);
        let Some((track, after)) = rest.split_first_mut() else {
            continue;
        };
        process_track(
            track,
            i,
            fx,
            externals,
            (before, 0),
            Some((&mut *after, i + 1)),
        );
        let (pl, pr) = (peak(&track.left, frames), peak(&track.right, frames));
        track.last_peak = pl.max(pr);
        meters.record_peak(i, pl, pr);
        publish_pitches(i, track, meters);
        if track.params.to_master {
            if let Some(master) = before.first_mut() {
                add_scaled(&mut master.left, &track.left, 1.0, frames);
                add_scaled(&mut master.right, &track.right, 1.0, frames);
            }
        }
        for send in track.params.active_sends() {
            let target = usize::from(send.target);
            let Some(dest) = target.checked_sub(i + 1).and_then(|j| after.get_mut(j)) else {
                continue;
            };
            add_scaled(&mut dest.left, &track.left, send.level, frames);
            add_scaled(&mut dest.right, &track.right, send.level, frames);
        }
    }
    if let Some((master, inserts)) = tracks.split_first_mut() {
        process_track(master, 0, fx, externals, (inserts, 1), None);
        let (pl, pr) = (peak(&master.left, frames), peak(&master.right, frames));
        master.last_peak = pl.max(pr);
        meters.record_peak(0, pl, pr);
        publish_pitches(0, master, meters);
    }
}

/// The sidechain key for an effect on track `index` whose `sidechain`
/// parameter names track `source`: that track's finished output, if it is
/// routed to `index` as a sidechain. `low` and `high` are the other tracks,
/// `low.0[j]` being track `j + low.1` (likewise for `high`).
fn sidechain_key<'a>(
    source: f32,
    index: usize,
    frames: usize,
    low: (&'a [TrackNode], usize),
    high: Option<(&'a [TrackNode], usize)>,
) -> Option<(&'a [f32], &'a [f32])> {
    if source < 0.5 {
        return None;
    }
    let source = source.round() as usize;
    let pick = |(tracks, offset): (&'a [TrackNode], usize)| {
        source.checked_sub(offset).and_then(|j| tracks.get(j))
    };
    let track = pick(low).or_else(|| high.and_then(pick))?;
    if !track.params.keys(index) {
        return None;
    }
    let n = frames.min(track.left.len()).min(track.right.len());
    Some((
        track.left.get(..n).unwrap_or_default(),
        track.right.get(..n).unwrap_or_default(),
    ))
}

/// Publishes what the track's pitch detecting effects (corrector, tuner)
/// hear; bypassed ones report nothing.
fn publish_pitches(index: usize, track: &TrackNode, meters: &Meters) {
    for (slot, effect) in track.effects.iter().enumerate() {
        if matches!(
            effect.effect.kind(),
            EffectKind::PitchCorrect | EffectKind::Tuner
        ) {
            let hz = effect
                .params
                .enabled
                .then(|| effect.effect.detected_pitch())
                .flatten();
            meters.record_pitch(index, slot, hz);
        }
    }
}

/// Runs the effects and fader of track `index`. `earlier` are the tracks
/// numbered below it, `earlier.0[j]` being mixer track `j + earlier.1`.
/// `later` holds the higher-numbered tracks, which a send effect may feed,
/// `later.0[j]` being mixer track `j + later.1`; the master has none. Either
/// may hold a finished track keying an effect's sidechain.
fn process_track(
    track: &mut TrackNode,
    index: usize,
    fx: FxContext<'_>,
    externals: &mut Externals,
    earlier: (&[TrackNode], usize),
    mut later: Option<(&mut [TrackNode], usize)>,
) {
    let FxContext {
        frames,
        ctx,
        transport,
        voices,
    } = fx;
    let TrackNode {
        params,
        gains,
        effects,
        left,
        right,
        eq,
        dry_left,
        dry_right,
        ..
    } = track;
    let (l, r) = (prefix(left, frames), prefix(right, frames));
    // Invalid samples would otherwise circulate in reverb and delay feedback
    // forever, silencing the track until the graph is rebuilt.
    scrub(l, r);
    params.stereo(l, r);
    if params.eq_active() {
        eq.process(l, r, &params.eq, ctx);
        scrub(l, r);
    }
    for slot in effects.iter_mut().filter(|s| s.params.enabled) {
        // A partial mix blends the slot's output with what went in.
        let mix = slot.params.mix.clamp(0.0, 1.0);
        let blend = mix < 0.999
            && slot.effect.kind() != EffectKind::Send
            && !matches!(slot.effect, musiccraft_dsp::Effect::BandSplit(_));
        let (dl, dr) = (prefix(dry_left, frames), prefix(dry_right, frames));
        if blend {
            // Zipped copies cannot panic on a length mismatch.
            for (d, s) in dl
                .iter_mut()
                .zip(l.iter())
                .chain(dr.iter_mut().zip(r.iter()))
            {
                *d = *s;
            }
        }
        let source = slot
            .effect
            .kind()
            .sidechain_index()
            .and_then(|i| slot.params.values.get(i))
            .copied()
            .unwrap_or(0.0);
        let high = later.as_ref().map(|(tracks, offset)| (&**tracks, *offset));
        let key = sidechain_key(source, index, frames, earlier, high);
        if let Some(external_slot) = slot.external {
            if let Some(Some(external)) = externals.get_mut(usize::from(external_slot)) {
                if let Some((kl, kr)) = key {
                    external.sidechain(kl, kr);
                }
                external.process(l, r, &transport);
                scrub(l, r);
            }
            if blend {
                mix_dry(l, r, dl, dr, mix);
            }
            continue;
        }
        if let musiccraft_dsp::Effect::PitchCorrect(corrector) = &mut slot.effect {
            corrector.set_follow(followed_note(&slot.params.values, voices));
        }
        if let musiccraft_dsp::Effect::BandSplit(splitter) = &mut slot.effect {
            match later.as_mut() {
                Some((tracks, offset)) => {
                    band_split(l, r, &slot.params.values, splitter, tracks, *offset, ctx);
                }
                None => splitter.process(l, r, &slot.params.values, ctx),
            }
            continue;
        }
        if slot.effect.kind() == EffectKind::Send {
            if let Some((tracks, offset)) = later.as_mut() {
                send(l, r, &slot.params.values, tracks, *offset);
            }
            continue;
        }
        slot.effect
            .process_keyed(l, r, &slot.params.values, ctx, key);
        if scrub(l, r) {
            // The effect's own state went bad; start it afresh.
            slot.effect.reset();
        }
        if blend {
            mix_dry(l, r, dl, dr, mix);
        }
    }
    let from = *gains;
    let to = params.gains();
    *gains = to;
    scale_ramped(l, from.0, to.0);
    scale_ramped(r, from.1, to.1);
}

/// Blends a slot's output with its input: `dry + (wet - dry) * mix`.
fn mix_dry(l: &mut [f32], r: &mut [f32], dl: &[f32], dr: &[f32], mix: f32) {
    for (wet, dry) in l.iter_mut().zip(dl).chain(r.iter_mut().zip(dr)) {
        *wet = dry + (*wet - dry) * mix;
    }
}

/// A send effect: adds this point of the chain to its target track, panned
/// and scaled, then scales what continues down the chain by `dry`.
/// Parameters: `[target, level, pan, dry]`.
fn send(l: &mut [f32], r: &mut [f32], p: &[f32], tracks: &mut [TrackNode], offset: usize) {
    let param = |i: usize, default: f32| p.get(i).copied().unwrap_or(default);
    let target = param(0, 0.0).round();
    let dest = (target >= 0.5)
        .then(|| (target as usize).checked_sub(offset))
        .flatten()
        .and_then(|j| tracks.get_mut(j));
    if let Some(dest) = dest {
        let level = param(1, 1.0).clamp(0.0, 1.25);
        let pan = param(2, 0.0).clamp(-1.0, 1.0);
        let (gl, gr) = ((1.0 - pan).min(1.0) * level, (1.0 + pan).min(1.0) * level);
        add_scaled(&mut dest.left, l, gl, l.len());
        add_scaled(&mut dest.right, r, gr, r.len());
    }
    let dry = param(3, 1.0).clamp(0.0, 1.0);
    if dry < 1.0 {
        for sample in l.iter_mut().chain(r.iter_mut()) {
            *sample *= dry;
        }
    }
}

/// The highest held key on the channel a pitch corrector follows (its
/// `follow` parameter, a rack position counted from 1; 0 is off).
fn followed_note(values: &[f32], voices: &[Voice]) -> Option<u8> {
    let follow = EffectKind::PitchCorrect
        .channel_ref_index()
        .and_then(|i| values.get(i))
        .copied()
        .unwrap_or(0.0)
        .round();
    if follow < 0.5 {
        return None;
    }
    let channel = follow as usize - 1;
    voices
        .iter()
        .filter(|v| v.held() && usize::from(v.channel) == channel)
        .map(|v| v.key)
        .max()
}

/// The band splitter: each band with a valid target insert is added to that
/// insert's input; bands without one stay on this track.
/// Parameters: `[low Hz, high Hz, low target, mid target, high target]`.
fn band_split(
    l: &mut [f32],
    r: &mut [f32],
    p: &[f32],
    splitter: &mut BandSplitter,
    tracks: &mut [TrackNode],
    offset: usize,
    ctx: ProcessContext,
) {
    splitter.prepare(p, ctx.sample_rate);
    // Index into `tracks` of each band's target, if it has a valid one.
    let targets: [Option<usize>; 3] = std::array::from_fn(|b| {
        let target = p.get(2 + b).copied().unwrap_or(0.0).round();
        (target >= 0.5)
            .then(|| (target as usize).checked_sub(offset))
            .flatten()
            .filter(|j| *j < tracks.len())
    });
    for (i, (sl, sr)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
        let bands = splitter.frame(*sl, *sr);
        let mut keep = [0.0_f32; 2];
        for (band, target) in bands.iter().zip(targets) {
            let dest = target.and_then(|j| tracks.get_mut(j));
            match dest {
                Some(dest) => {
                    if let Some(d) = dest.left.get_mut(i) {
                        *d += band[0];
                    }
                    if let Some(d) = dest.right.get_mut(i) {
                        *d += band[1];
                    }
                }
                None => {
                    keep[0] += band[0];
                    keep[1] += band[1];
                }
            }
        }
        *sl = keep[0];
        *sr = keep[1];
    }
}

/// Replaces non-finite and subnormal samples with silence. Returns whether
/// any non-finite ones were found.
fn scrub(left: &mut [f32], right: &mut [f32]) -> bool {
    let mut found = false;
    for sample in left.iter_mut().chain(right.iter_mut()) {
        if !sample.is_finite() {
            *sample = 0.0;
            found = true;
        } else if sample.is_subnormal() {
            // Too quiet to hear and slow to compute with (see
            // `musiccraft_dsp::flush_subnormals`); not a fault.
            *sample = 0.0;
        }
    }
    found
}

fn prefix(buffer: &mut [f32], frames: usize) -> &mut [f32] {
    let len = buffer.len().min(frames);
    buffer.get_mut(..len).unwrap_or_default()
}

fn clear(buffer: &mut [f32], frames: usize) {
    prefix(buffer, frames).fill(0.0);
}

fn add_scaled(dest: &mut [f32], src: &[f32], gain: f32, frames: usize) {
    for (d, s) in dest.iter_mut().zip(src).take(frames) {
        *d += *s * gain;
    }
}

fn peak(buffer: &[f32], frames: usize) -> f32 {
    buffer.iter().take(frames).fold(0.0, |p, s| p.max(s.abs()))
}

fn sanitize(sample: f32) -> f32 {
    if sample.is_finite() {
        sample.clamp(-4.0, 4.0)
    } else {
        0.0
    }
}
