use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};

use musiccraft_params::{MAX_EFFECT_SLOTS, MAX_MIXER_TRACKS};

/// Samples kept for the oscilloscope (master output, mono).
pub const SCOPE_LEN: usize = 2048;

/// Channels whose note starts are counted for the rack's activity lights.
pub const ACTIVITY_CHANNELS: usize = 512;

/// Values the engine publishes for display: playhead, transport, peak levels,
/// a short history of the master output and processing load. Shared between
/// threads through atomics only.
#[derive(Debug)]
pub struct Meters {
    position_bits: AtomicU64,
    playing: AtomicBool,
    active_voices: AtomicU32,
    peaks: Vec<AtomicU32>,
    scope: Vec<AtomicU32>,
    scope_write: AtomicUsize,
    load_bits: AtomicU32,
    pitches: Vec<AtomicU32>,
    /// Notes started per channel since the front end last took them.
    notes: Vec<AtomicU32>,
    /// Where playback last paused at a pause marker, and how many times it has.
    marker_pause_bits: AtomicU64,
    marker_pauses: AtomicU64,
    /// Clicking a count-in, or waiting for the first live note.
    counting_in: AtomicBool,
    waiting: AtomicBool,
    /// Times playback stopped at the loop end (recording without looping).
    end_stops: AtomicU64,
}

impl Default for Meters {
    fn default() -> Self {
        Self::new()
    }
}

impl Meters {
    /// Allocates meters for [`MAX_MIXER_TRACKS`] stereo tracks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            position_bits: AtomicU64::new(0.0_f64.to_bits()),
            playing: AtomicBool::new(false),
            active_voices: AtomicU32::new(0),
            peaks: (0..MAX_MIXER_TRACKS * 2)
                .map(|_| AtomicU32::new(0))
                .collect(),
            scope: (0..SCOPE_LEN).map(|_| AtomicU32::new(0)).collect(),
            scope_write: AtomicUsize::new(0),
            load_bits: AtomicU32::new(0),
            pitches: (0..MAX_MIXER_TRACKS * MAX_EFFECT_SLOTS)
                .map(|_| AtomicU32::new(0))
                .collect(),
            notes: (0..ACTIVITY_CHANNELS).map(|_| AtomicU32::new(0)).collect(),
            marker_pause_bits: AtomicU64::new(0.0_f64.to_bits()),
            marker_pauses: AtomicU64::new(0),
            counting_in: AtomicBool::new(false),
            waiting: AtomicBool::new(false),
            end_stops: AtomicU64::new(0),
        }
    }

    /// How many times playback has paused at a pause marker, and the position
    /// (ticks) of the latest pause. Front ends watch the count to stop their
    /// transport and keep the playhead on the marker.
    #[must_use]
    pub fn marker_pause(&self) -> (u64, f64) {
        let count = self.marker_pauses.load(Ordering::Acquire);
        let position = f64::from_bits(self.marker_pause_bits.load(Ordering::Relaxed));
        (count, position)
    }

    pub(crate) fn record_marker_pause(&self, position: f64) {
        self.marker_pause_bits
            .store(position.to_bits(), Ordering::Relaxed);
        self.marker_pauses.fetch_add(1, Ordering::Release);
    }

    /// Whether a count-in is clicking before playback starts.
    #[must_use]
    pub fn is_counting_in(&self) -> bool {
        self.counting_in.load(Ordering::Relaxed)
    }

    /// Whether playback waits for the first live note.
    #[must_use]
    pub fn is_waiting(&self) -> bool {
        self.waiting.load(Ordering::Relaxed)
    }

    /// How many times playback has stopped at the loop end. Front ends watch
    /// the count to stop their transport too.
    #[must_use]
    pub fn end_stops(&self) -> u64 {
        self.end_stops.load(Ordering::Acquire)
    }

    pub(crate) fn record_end_stop(&self) {
        self.end_stops.fetch_add(1, Ordering::Release);
    }

    pub(crate) fn publish_start(&self, counting_in: bool, waiting: bool) {
        self.counting_in.store(counting_in, Ordering::Relaxed);
        self.waiting.store(waiting, Ordering::Relaxed);
    }

    /// Fraction of the real-time budget the last callback used (1.0 = all of it).
    #[must_use]
    pub fn load(&self) -> f32 {
        f32::from_bits(self.load_bits.load(Ordering::Relaxed))
    }

    /// Copies the most recent `out.len()` master samples, oldest first.
    pub fn scope(&self, out: &mut [f32]) {
        let n = out.len().min(SCOPE_LEN);
        let end = self.scope_write.load(Ordering::Relaxed);
        for (i, slot) in out.iter_mut().take(n).enumerate() {
            let index = (end + SCOPE_LEN - n + i) % SCOPE_LEN;
            *slot = self
                .scope
                .get(index)
                .map_or(0.0, |s| f32::from_bits(s.load(Ordering::Relaxed)));
        }
    }

    pub(crate) fn push_scope(&self, samples: impl Iterator<Item = f32>) {
        let mut write = self.scope_write.load(Ordering::Relaxed);
        for sample in samples {
            if let Some(slot) = self.scope.get(write % SCOPE_LEN) {
                slot.store(sample.to_bits(), Ordering::Relaxed);
            }
            write = (write + 1) % SCOPE_LEN;
        }
        self.scope_write.store(write, Ordering::Relaxed);
    }

    pub(crate) fn set_load(&self, load: f32) {
        self.load_bits.store(load.to_bits(), Ordering::Relaxed);
    }

    /// Playhead position in ticks.
    #[must_use]
    pub fn position_ticks(&self) -> f64 {
        f64::from_bits(self.position_bits.load(Ordering::Relaxed))
    }

    /// Whether the engine is playing.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    /// Number of sounding voices.
    #[must_use]
    pub fn active_voices(&self) -> u32 {
        self.active_voices.load(Ordering::Relaxed)
    }

    /// Returns the highest peak (left, right) since the last call and resets it.
    #[must_use]
    pub fn take_peak(&self, track: usize) -> (f32, f32) {
        let take = |i: usize| {
            self.peaks
                .get(i)
                .map_or(0.0, |p| f32::from_bits(p.swap(0, Ordering::Relaxed)))
        };
        (take(track * 2), take(track * 2 + 1))
    }

    /// Returns how many notes `channel` started since the last call and
    /// resets the count.
    #[must_use]
    pub fn take_notes(&self, channel: usize) -> u32 {
        self.notes
            .get(channel)
            .map_or(0, |n| n.swap(0, Ordering::Relaxed))
    }

    pub(crate) fn record_note(&self, channel: u16) {
        if let Some(count) = self.notes.get(usize::from(channel)) {
            count.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn publish(&self, position: f64, playing: bool, voices: u32) {
        self.position_bits
            .store(position.to_bits(), Ordering::Relaxed);
        self.playing.store(playing, Ordering::Relaxed);
        self.active_voices.store(voices, Ordering::Relaxed);
    }

    /// The pitch (Hz) the detecting effect in `slot` of mixer `track` last
    /// heard (pitch corrector, tuner); `None` without a clear pitch.
    #[must_use]
    pub fn pitch(&self, track: usize, slot: usize) -> Option<f32> {
        let index = Self::pitch_index(track, slot)?;
        let hz = f32::from_bits(self.pitches.get(index)?.load(Ordering::Relaxed));
        (hz > 0.0 && hz.is_finite()).then_some(hz)
    }

    fn pitch_index(track: usize, slot: usize) -> Option<usize> {
        (track < MAX_MIXER_TRACKS && slot < MAX_EFFECT_SLOTS)
            .then_some(track * MAX_EFFECT_SLOTS + slot)
    }

    pub(crate) fn record_pitch(&self, track: usize, slot: usize, hz: Option<f32>) {
        if let Some(cell) = Self::pitch_index(track, slot).and_then(|i| self.pitches.get(i)) {
            cell.store(hz.unwrap_or(0.0).to_bits(), Ordering::Relaxed);
        }
    }

    pub(crate) fn record_peak(&self, track: usize, left: f32, right: f32) {
        // For non-negative floats the bit patterns order the same as the values.
        for (i, value) in [(track * 2, left), (track * 2 + 1, right)] {
            if let Some(peak) = self.peaks.get(i) {
                let value = if value.is_finite() { value.abs() } else { 0.0 };
                peak.fetch_max(value.to_bits(), Ordering::Relaxed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaks_hold_until_taken() {
        let meters = Meters::new();
        meters.record_peak(3, 0.5, 0.25);
        meters.record_peak(3, 0.2, 0.75);
        assert_eq!(meters.take_peak(3), (0.5, 0.75));
        assert_eq!(meters.take_peak(3), (0.0, 0.0));
        meters.record_peak(MAX_MIXER_TRACKS + 5, 1.0, 1.0);
        assert_eq!(meters.take_peak(MAX_MIXER_TRACKS + 5), (0.0, 0.0));
    }

    #[test]
    fn note_starts_are_counted_until_taken() {
        let meters = Meters::new();
        meters.record_note(2);
        meters.record_note(2);
        assert_eq!(meters.take_notes(2), 2);
        assert_eq!(meters.take_notes(2), 0);
        assert_eq!(meters.take_notes(1), 0);
        meters.record_note(u16::MAX);
        assert_eq!(meters.take_notes(usize::from(u16::MAX)), 0);
    }

    #[test]
    fn pitches_are_kept_per_slot() {
        let meters = Meters::new();
        assert_eq!(meters.pitch(2, 1), None);
        meters.record_pitch(2, 1, Some(220.0));
        assert_eq!(meters.pitch(2, 1), Some(220.0));
        assert_eq!(meters.pitch(2, 0), None);
        meters.record_pitch(2, 1, None);
        assert_eq!(meters.pitch(2, 1), None);
        meters.record_pitch(MAX_MIXER_TRACKS, 0, Some(1.0));
        assert_eq!(meters.pitch(MAX_MIXER_TRACKS, 0), None);
    }

    #[test]
    fn scope_returns_latest_samples_in_order() {
        let meters = Meters::new();
        meters.push_scope((0..SCOPE_LEN + 3).map(|i| i as f32));
        let mut out = [0.0; 3];
        meters.scope(&mut out);
        assert_eq!(
            out,
            [
                SCOPE_LEN as f32,
                SCOPE_LEN as f32 + 1.0,
                SCOPE_LEN as f32 + 2.0
            ]
        );
    }
}
