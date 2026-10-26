//! Parameter tables shared by the project model, the engine and the UI.
//!
//! The project stores parameters by name; the engine reads them from fixed-size
//! arrays by index. The tables here are the single contract between the two.

use std::fmt;

pub mod describe;
pub mod presets;

use serde::{Deserialize, Serialize};

/// Ticks per quarter note (one beat). A step (1/16 note) is [`TICKS_PER_STEP`].
pub const PPQ: u32 = 96;
/// Ticks per sixteenth-note step.
pub const TICKS_PER_STEP: u32 = PPQ / 4;

/// Capacity of an effect parameter array.
pub const MAX_EFFECT_PARAMS: usize = 24;
/// Capacity of an instrument parameter array.
pub const MAX_INSTRUMENT_PARAMS: usize = 160;
/// Maximum number of effect slots on one mixer track.
pub const MAX_EFFECT_SLOTS: usize = 10;
/// Parallel branches an effect rack holds.
pub const RACK_BRANCHES: usize = 4;
/// Effects one effect-rack branch holds.
pub const RACK_BRANCH_EFFECTS: usize = 6;
/// Maximum number of mixer tracks, master included.
pub const MAX_MIXER_TRACKS: usize = 128;

/// Sounds synthesized at start-up, addressed as `builtin:<name>` sample names.
pub const BUILTIN_SAMPLES: [&str; 8] = [
    "kick", "snare", "clap", "hat", "open_hat", "tom", "rim", "crash",
];
/// Prefix of built-in sample names.
pub const BUILTIN_PREFIX: &str = "builtin:";

/// Description of one numeric parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    /// Stable identifier used by commands and the project file.
    pub name: &'static str,
    /// Lowest accepted value.
    pub min: f32,
    /// Highest accepted value.
    pub max: f32,
    /// Value for a newly created instrument or effect.
    pub default: f32,
    /// Display unit (may be empty).
    pub unit: &'static str,
    /// Whether only whole numbers are meaningful.
    pub integer: bool,
}

const fn spec(
    name: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
) -> ParamSpec {
    ParamSpec {
        name,
        min,
        max,
        default,
        unit,
        integer: false,
    }
}

const fn int_spec(name: &'static str, min: f32, max: f32, default: f32) -> ParamSpec {
    ParamSpec {
        name,
        min,
        max,
        default,
        unit: "",
        integer: true,
    }
}

impl ParamSpec {
    /// Clamps and, for integer parameters, rounds a value into range.
    #[must_use]
    pub fn sanitize(&self, value: f32) -> f32 {
        let value = value.clamp(self.min, self.max);
        if self.integer { value.round() } else { value }
    }

    /// How many whole-number steps an integer parameter's knob crosses (its
    /// range); `None` for a continuous parameter. Integer parameters map
    /// linearly onto a knob, so each step is the same share of its travel.
    #[must_use]
    pub fn steps(&self) -> Option<u32> {
        let range = (self.max - self.min).round();
        (self.integer && range >= 1.0).then_some(range as u32)
    }

    /// Whether `value` is finite and inside the range.
    #[must_use]
    pub fn accepts(&self, value: f32) -> bool {
        value.is_finite() && value >= self.min && value <= self.max
    }

    /// Maps a value to a 0..1 control position. Frequencies use a log scale and
    /// times a cubic one, so useful values get most of a knob's travel.
    #[must_use]
    pub fn to_unit(&self, value: f32) -> f32 {
        let value = value.clamp(self.min, self.max);
        match self.unit {
            "Hz" if self.min > 0.0 => (value / self.min).ln() / (self.max / self.min).ln(),
            "s" | "ms" => ((value - self.min) / (self.max - self.min)).cbrt(),
            _ => (value - self.min) / (self.max - self.min),
        }
    }

    /// Inverse of [`ParamSpec::to_unit`], sanitized into range.
    #[must_use]
    pub fn from_unit(&self, t: f32) -> f32 {
        let t = if t.is_finite() {
            t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let value = match self.unit {
            "Hz" if self.min > 0.0 => self.min * (self.max / self.min).powf(t),
            "s" | "ms" => self.min + t.powi(3) * (self.max - self.min),
            _ => self.min + t * (self.max - self.min),
        };
        self.sanitize(value)
    }

    /// Formats a value with its unit for display.
    #[must_use]
    pub fn format(&self, value: f32) -> String {
        // The unit is chosen after rounding, and a value that rounds to zero
        // shows as zero, so the text parses back to a value that shows the
        // same text (999.7 Hz is "1.00 kHz", not "1000 Hz"; -0.01 is "0.0").
        let shown = |value: f32, decimals: i32| shown(value, decimals);
        match self.unit {
            "Hz" if shown(value, 0) >= 1_000.0 => format!("{:.2} kHz", value / 1_000.0),
            "Hz" => format!("{:.0} Hz", shown(value, 0)),
            "s" if shown(value * 1_000.0, 0) < 1_000.0 => {
                format!("{:.0} ms", shown(value * 1_000.0, 0))
            }
            "s" => format!("{value:.2} s"),
            "" if self.integer => format!("{:.0}", shown(value, 0)),
            "" => format!("{:.0}%", shown(value * 100.0, 0)),
            unit => format!("{:.1} {unit}", shown(value, 1)),
        }
    }
}

/// `value` rounded to `decimals` places as display text shows it, with no
/// negative zero: format the result with the same number of decimals so the
/// text reads back to the value it shows.
#[must_use]
pub fn shown(value: f32, decimals: i32) -> f32 {
    let scale = 10f32.powi(decimals);
    (value * scale).round() / scale + 0.0
}

/// Fader position (0..1) of a linear gain: silence at the bottom, -24 dB at a
/// quarter, 0 dB at three quarters and `max` at the top.
#[must_use]
pub fn volume_to_unit(gain: f32, max: f32) -> f32 {
    let max_db = 20.0 * max.max(1e-6).log10();
    let low = 10.0_f32.powf(-24.0 / 20.0);
    if gain <= 0.0 || !gain.is_finite() {
        0.0
    } else if gain < low {
        0.25 * gain / low
    } else {
        let db = 20.0 * gain.log10();
        if db <= 0.0 {
            0.75 + db / 48.0
        } else {
            (0.75 + 0.25 * db / max_db).min(1.0)
        }
    }
}

/// Inverse of [`volume_to_unit`].
#[must_use]
pub fn volume_from_unit(position: f32, max: f32) -> f32 {
    let p = if position.is_finite() {
        position.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let max_db = 20.0 * max.max(1e-6).log10();
    let low = 10.0_f32.powf(-24.0 / 20.0);
    if p < 0.25 {
        p / 0.25 * low
    } else if p <= 0.75 {
        10.0_f32.powf((p - 0.75) * 48.0 / 20.0)
    } else {
        10.0_f32.powf((p - 0.75) / 0.25 * max_db / 20.0)
    }
}

/// Looks up a parameter's index by name.
#[must_use]
pub fn index_of(specs: &[ParamSpec], name: &str) -> Option<usize> {
    specs.iter().position(|spec| spec.name == name)
}

/// Default values of a table, padded with zeros to the array size `N`.
#[must_use]
pub fn defaults<const N: usize>(specs: &[ParamSpec]) -> [f32; N] {
    let mut values = [0.0; N];
    for (value, spec) in values.iter_mut().zip(specs) {
        *value = spec.default;
    }
    values
}

/// Oscillator wave shapes, in the order used by the `wave1`/`wave2` parameters.
pub const WAVEFORMS: [&str; 5] = ["sine", "saw", "square", "triangle", "noise"];

/// Indices into the instrument parameter array.
pub mod inst {
    /// Envelope attack time, seconds.
    pub const ATTACK: usize = 0;
    /// Envelope decay time, seconds.
    pub const DECAY: usize = 1;
    /// Envelope sustain level, 0..1.
    pub const SUSTAIN: usize = 2;
    /// Envelope release time, seconds.
    pub const RELEASE: usize = 3;
    /// Synth oscillator 1 waveform index.
    pub const WAVE1: usize = 4;
    /// Synth oscillator 2 waveform index.
    pub const WAVE2: usize = 5;
    /// Oscillator 2 detune, cents.
    pub const DETUNE: usize = 6;
    /// Oscillator mix: 0 = only osc 1, 1 = only osc 2.
    pub const OSC_MIX: usize = 7;
    /// Low-pass cutoff, hertz.
    pub const CUTOFF: usize = 8;
    /// Filter resonance, 0..1.
    pub const RESONANCE: usize = 9;
    /// Envelope to cutoff modulation, octaves.
    pub const ENV_AMOUNT: usize = 10;
    /// Sampler: 1 = play the whole sample regardless of note length.
    pub const ONE_SHOT: usize = 11;
    /// Sampler: key at which the sample plays at its recorded pitch.
    pub const ROOT_KEY: usize = 12;
    /// Coarse pitch offset applied to every note, semitones.
    pub const PITCH: usize = 13;
    /// Synth portamento: time to slide from a held note to a new one, seconds.
    pub const GLIDE: usize = 14;
    pub const UNISON: usize = 15;
    pub const SPREAD: usize = 16;
    pub const SUB: usize = 17;
    pub const LFO_RATE: usize = 18;
    pub const LFO_DEPTH: usize = 19;
    pub const LFO_TARGET: usize = 20;
    pub const FILTER_TYPE: usize = 21;
    pub const DRIVE: usize = 22;
    pub const ARP_MODE: usize = 23;
    pub const ARP_RATE: usize = 24;
    pub const ARP_RANGE: usize = 25;
    pub const ARP_GATE: usize = 26;
    pub const DRUM_TYPE: usize = 27;
    pub const DRUM_TUNE: usize = 28;
    pub const DRUM_DECAY: usize = 29;
    pub const DRUM_SWEEP: usize = 30;
    pub const DRUM_TONE: usize = 31;
    pub const DRUM_CLICK: usize = 32;
    pub const PLUCK_DECAY: usize = 33;
    pub const PLUCK_DAMPING: usize = 34;
    pub const PLUCK_TONE: usize = 35;
    pub const SLICES: usize = 36;
    pub const WAVE3: usize = 37;
    pub const OSC3_LEVEL: usize = 38;
    pub const OSC3_COARSE: usize = 39;
    pub const FM_ALGO: usize = 40;
    pub const FM_FEEDBACK: usize = 41;
    pub const OP1_RATIO: usize = 42;
    pub const OP1_LEVEL: usize = 43;
    pub const OP1_DECAY: usize = 44;
    pub const OP1_SUSTAIN: usize = 45;
    pub const OP2_RATIO: usize = 46;
    pub const OP2_LEVEL: usize = 47;
    pub const OP2_DECAY: usize = 48;
    pub const OP2_SUSTAIN: usize = 49;
    pub const OP3_RATIO: usize = 50;
    pub const OP3_LEVEL: usize = 51;
    pub const OP3_DECAY: usize = 52;
    pub const OP3_SUSTAIN: usize = 53;
    pub const OP4_RATIO: usize = 54;
    pub const OP4_LEVEL: usize = 55;
    pub const OP4_DECAY: usize = 56;
    pub const OP4_SUSTAIN: usize = 57;
    pub const KEYS_TYPE: usize = 58;
    pub const KEYS_TONE: usize = 59;
    pub const KEYS_DECAY: usize = 60;
    pub const KEYS_TREMOLO: usize = 61;
    pub const KEYS_HAMMER: usize = 62;
    pub const ADD_PARTIALS: usize = 63;
    pub const ADD_BRIGHT: usize = 64;
    pub const ADD_EVEN: usize = 65;
    pub const ADD_INHARM: usize = 66;
    pub const ADD_DECAY: usize = 67;
    pub const GRAN_SIZE: usize = 68;
    pub const GRAN_DENSITY: usize = 69;
    pub const GRAN_POSITION: usize = 70;
    pub const GRAN_SPREAD: usize = 71;
    pub const GRAN_PITCH_RAND: usize = 72;
    pub const GRAN_SCAN: usize = 73;
    /// Share of the project swing this channel follows, 0..1.
    pub const SWING_MIX: usize = 74;
    /// 1 = swung notes keep their end (they get shorter).
    pub const SWING_TRUNCATE: usize = 75;
    /// Cut group this channel's notes cut (0 = none).
    pub const CUT_GROUP: usize = 76;
    /// Cut group that cuts this channel (0 = none).
    pub const CUT_BY: usize = 77;
    /// 1 = one note at a time.
    pub const MONO: usize = 78;
    /// Most voices sounding at once (0 = no limit).
    pub const POLY_MAX: usize = 79;
    /// Sampler: 1 = play the sample backwards.
    pub const REVERSE: usize = 80;
    /// Sampler: fade-in time, seconds.
    pub const FADE_IN: usize = 81;
    /// Sampler: fade-out time before the note or sample ends, seconds.
    pub const FADE_OUT: usize = 82;
    /// Moves every note of the channel, ticks (negative is earlier).
    pub const TIME_SHIFT: usize = 83;
    /// Fixed note length in ticks (0 = the notes' own lengths).
    pub const GATE: usize = 84;
    /// Note echo: repeats of each note (0 = off).
    pub const ECHO_COUNT: usize = 85;
    /// Note echo: ticks between repeats.
    pub const ECHO_TIME: usize = 86;
    /// Note echo: velocity kept per repeat, 0..1.
    pub const ECHO_FEED: usize = 87;
    /// Note echo: semitones added per repeat.
    pub const ECHO_PITCH: usize = 88;
    /// Second filter: 0 off, 1 low-pass, 2 band-pass, 3 high-pass.
    pub const FILTER2_TYPE: usize = 89;
    /// Second filter cutoff, hertz.
    pub const CUTOFF2: usize = 90;
    /// Second filter resonance, 0..1.
    pub const RESONANCE2: usize = 91;
    /// 0 series, 1 parallel, 2 split (osc 1 and sub to filter 1, osc 2 and 3 to filter 2).
    pub const FILTER_ROUTING: usize = 92;
    /// Second LFO rate, hertz.
    pub const LFO2_RATE: usize = 93;
    /// Second LFO shape: sine, triangle, saw, square, random steps.
    pub const LFO2_SHAPE: usize = 94;
    /// Modulation envelope attack, seconds.
    pub const ENV2_ATTACK: usize = 95;
    /// Modulation envelope decay, seconds.
    pub const ENV2_DECAY: usize = 96;
    /// Modulation slot 1 source (see `MOD_SOURCES`).
    pub const MOD1_SOURCE: usize = 97;
    /// Modulation slot 1 destination (see `MOD_DESTS`).
    pub const MOD1_DEST: usize = 98;
    /// Modulation slot 1 amount, -1..1.
    pub const MOD1_AMOUNT: usize = 99;
    pub const MOD2_SOURCE: usize = 100;
    pub const MOD2_DEST: usize = 101;
    pub const MOD2_AMOUNT: usize = 102;
    pub const MOD3_SOURCE: usize = 103;
    pub const MOD3_DEST: usize = 104;
    pub const MOD3_AMOUNT: usize = 105;
    /// FM operators 5 and 6 (ratio, level, decay, sustain), like 1–4.
    pub const OP5_RATIO: usize = 106;
    pub const OP5_LEVEL: usize = 107;
    pub const OP5_DECAY: usize = 108;
    pub const OP5_SUSTAIN: usize = 109;
    pub const OP6_RATIO: usize = 110;
    pub const OP6_LEVEL: usize = 111;
    pub const OP6_DECAY: usize = 112;
    pub const OP6_SUSTAIN: usize = 113;
    /// FM operator waveforms (see `FM_WAVES`), operators 1–6.
    pub const OP1_WAVE: usize = 114;
    pub const OP2_WAVE: usize = 115;
    pub const OP3_WAVE: usize = 116;
    pub const OP4_WAVE: usize = 117;
    pub const OP5_WAVE: usize = 118;
    pub const OP6_WAVE: usize = 119;
    /// FM output low-pass cutoff, hertz (20 kHz = off), and resonance.
    pub const FM_CUTOFF: usize = 120;
    pub const FM_RESONANCE: usize = 121;
    /// Pluck: bowing (continuous excitation while held), 0..1.
    pub const PLUCK_BOW: usize = 122;
    /// Pluck: strings sounding together, 1..3.
    pub const PLUCK_STRINGS: usize = 123;
    /// Pluck: detune of the extra strings, cents.
    pub const PLUCK_DETUNE: usize = 124;
    /// Additive: the second spectrum's slope, even-harmonic level and stretch.
    pub const ADD_BRIGHT_B: usize = 125;
    pub const ADD_EVEN_B: usize = 126;
    pub const ADD_INHARM_B: usize = 127;
    /// Additive: morph position between the spectra, 0 = first, 1 = second.
    pub const ADD_MORPH: usize = 128;
    /// Additive: seconds to morph from `add_morph` to the second spectrum (0 = off).
    pub const ADD_MORPH_TIME: usize = 129;
    /// Additive: morph LFO rate (Hz) and depth.
    pub const ADD_MORPH_RATE: usize = 130;
    pub const ADD_MORPH_DEPTH: usize = 131;
    /// MIDI Out: eight control-change values (-1 = not sent), then their
    /// controller numbers, then the program (0 = none, else 1..128).
    pub const MIDI_CC1: usize = 132;
    pub const MIDI_CC1_NUMBER: usize = 140;
    pub const MIDI_PROGRAM: usize = 148;
    /// Resynth: playback speed through the analysis (0 freezes, 1 as recorded).
    pub const RESYN_SPEED: usize = 149;
    /// Resynth: start position, 0..1 of the sample.
    pub const RESYN_POSITION: usize = 150;
    /// Resynth: smears the harmonic levels over time, 0..1.
    pub const RESYN_BLUR: usize = 151;
    /// Resynth: 1 = loop the analysis instead of holding its last frame.
    pub const RESYN_LOOP: usize = 152;
}

/// Parameters shared by every instrument (sampler and synth), in array order.
pub const INSTRUMENT_PARAMS: &[ParamSpec] = &[
    spec("attack", 0.0, 5.0, 0.002, "s"),
    spec("decay", 0.0, 5.0, 0.3, "s"),
    spec("sustain", 0.0, 1.0, 0.8, ""),
    spec("release", 0.0, 5.0, 0.12, "s"),
    int_spec("wave1", 0.0, 4.0, 1.0),
    int_spec("wave2", 0.0, 4.0, 2.0),
    spec("detune", -1200.0, 1200.0, 7.0, "ct"),
    spec("osc_mix", 0.0, 1.0, 0.35, ""),
    spec("cutoff", 20.0, 20_000.0, 6_000.0, "Hz"),
    spec("resonance", 0.0, 1.0, 0.2, ""),
    spec("env_amount", -8.0, 8.0, 0.0, "oct"),
    int_spec("one_shot", 0.0, 1.0, 1.0),
    int_spec("root_key", 0.0, 127.0, 60.0),
    spec("pitch", -48.0, 48.0, 0.0, "st"),
    spec("glide", 0.0, 2.0, 0.0, "s"),
    int_spec("unison", 1.0, 7.0, 1.0),
    spec("spread", 0.0, 100.0, 15.0, "ct"),
    spec("sub", 0.0, 1.0, 0.0, ""),
    spec("lfo_rate", 0.05, 20.0, 5.0, "Hz"),
    spec("lfo_depth", 0.0, 1.0, 0.0, ""),
    int_spec("lfo_target", 0.0, 2.0, 0.0),
    int_spec("filter_type", 0.0, 2.0, 0.0),
    spec("drive", 0.0, 1.0, 0.0, ""),
    int_spec("arp_mode", 0.0, 4.0, 0.0),
    int_spec("arp_rate", 0.0, 4.0, 1.0),
    int_spec("arp_range", 1.0, 4.0, 1.0),
    spec("arp_gate", 0.05, 1.0, 0.8, ""),
    int_spec("drum_type", 0.0, 4.0, 0.0),
    spec("drum_tune", -24.0, 24.0, 0.0, "st"),
    spec("drum_decay", 0.02, 2.0, 0.4, "s"),
    spec("drum_sweep", 0.0, 48.0, 24.0, "st"),
    spec("drum_tone", 0.0, 1.0, 0.5, ""),
    spec("drum_click", 0.0, 1.0, 0.3, ""),
    spec("pluck_decay", 0.1, 10.0, 2.0, "s"),
    spec("pluck_damping", 0.0, 1.0, 0.3, ""),
    spec("pluck_tone", 0.0, 1.0, 0.7, ""),
    int_spec("slices", 1.0, 64.0, 1.0),
    int_spec("wave3", 0.0, 4.0, 2.0),
    spec("osc3_level", 0.0, 1.0, 0.0, ""),
    spec("osc3_coarse", -24.0, 24.0, -12.0, "st"),
    int_spec("fm_algo", 0.0, 11.0, 0.0),
    spec("fm_feedback", 0.0, 1.0, 0.0, ""),
    spec("op1_ratio", 0.25, 16.0, 1.0, ""),
    spec("op1_level", 0.0, 1.0, 1.0, ""),
    spec("op1_decay", 0.01, 5.0, 1.0, "s"),
    spec("op1_sustain", 0.0, 1.0, 0.8, ""),
    spec("op2_ratio", 0.25, 16.0, 1.0, ""),
    spec("op2_level", 0.0, 1.0, 0.5, ""),
    spec("op2_decay", 0.01, 5.0, 0.6, "s"),
    spec("op2_sustain", 0.0, 1.0, 0.2, ""),
    spec("op3_ratio", 0.25, 16.0, 2.0, ""),
    spec("op3_level", 0.0, 1.0, 0.0, ""),
    spec("op3_decay", 0.01, 5.0, 0.4, "s"),
    spec("op3_sustain", 0.0, 1.0, 0.0, ""),
    spec("op4_ratio", 0.25, 16.0, 1.0, ""),
    spec("op4_level", 0.0, 1.0, 0.0, ""),
    spec("op4_decay", 0.01, 5.0, 0.3, "s"),
    spec("op4_sustain", 0.0, 1.0, 0.0, ""),
    int_spec("keys_type", 0.0, 2.0, 0.0),
    spec("keys_tone", 0.0, 1.0, 0.5, ""),
    spec("keys_decay", 0.2, 10.0, 3.0, "s"),
    spec("keys_tremolo", 0.0, 1.0, 0.0, ""),
    spec("keys_hammer", 0.0, 1.0, 0.3, ""),
    int_spec("add_partials", 1.0, 32.0, 16.0),
    spec("add_bright", 0.0, 1.0, 0.5, ""),
    spec("add_even", 0.0, 1.0, 1.0, ""),
    spec("add_inharm", 0.0, 1.0, 0.0, ""),
    spec("add_decay", 0.0, 1.0, 0.0, ""),
    spec("gran_size", 5.0, 500.0, 80.0, "ms"),
    spec("gran_density", 1.0, 100.0, 20.0, "Hz"),
    spec("gran_position", 0.0, 1.0, 0.2, ""),
    spec("gran_spread", 0.0, 1.0, 0.1, ""),
    spec("gran_pitch_rand", 0.0, 12.0, 0.0, "st"),
    spec("gran_scan", -2.0, 2.0, 0.0, ""),
    spec("swing_mix", 0.0, 1.0, 1.0, ""),
    int_spec("swing_truncate", 0.0, 1.0, 0.0),
    int_spec("cut_group", 0.0, 16.0, 0.0),
    int_spec("cut_by", 0.0, 16.0, 0.0),
    int_spec("mono", 0.0, 1.0, 0.0),
    int_spec("poly_max", 0.0, 64.0, 0.0),
    int_spec("reverse", 0.0, 1.0, 0.0),
    spec("fade_in", 0.0, 10.0, 0.0, "s"),
    spec("fade_out", 0.0, 10.0, 0.0, "s"),
    int_spec("time_shift", -96.0, 96.0, 0.0),
    int_spec("gate", 0.0, 384.0, 0.0),
    int_spec("echo_count", 0.0, 8.0, 0.0),
    int_spec("echo_time", 1.0, 384.0, 48.0),
    spec("echo_feed", 0.0, 1.0, 0.5, ""),
    int_spec("echo_pitch", -12.0, 12.0, 0.0),
    int_spec("filter2_type", 0.0, 3.0, 0.0),
    spec("cutoff2", 20.0, 20_000.0, 2_000.0, "Hz"),
    spec("resonance2", 0.0, 1.0, 0.2, ""),
    int_spec("filter_routing", 0.0, 2.0, 0.0),
    spec("lfo2_rate", 0.05, 20.0, 2.0, "Hz"),
    int_spec("lfo2_shape", 0.0, 4.0, 0.0),
    spec("env2_attack", 0.0, 10.0, 0.01, "s"),
    spec("env2_decay", 0.01, 10.0, 0.5, "s"),
    int_spec("mod1_source", 0.0, 6.0, 0.0),
    int_spec("mod1_dest", 0.0, 8.0, 0.0),
    spec("mod1_amount", -1.0, 1.0, 0.0, ""),
    int_spec("mod2_source", 0.0, 6.0, 0.0),
    int_spec("mod2_dest", 0.0, 8.0, 0.0),
    spec("mod2_amount", -1.0, 1.0, 0.0, ""),
    int_spec("mod3_source", 0.0, 6.0, 0.0),
    int_spec("mod3_dest", 0.0, 8.0, 0.0),
    spec("mod3_amount", -1.0, 1.0, 0.0, ""),
    spec("op5_ratio", 0.25, 16.0, 3.0, ""),
    spec("op5_level", 0.0, 1.0, 0.0, ""),
    spec("op5_decay", 0.01, 10.0, 1.0, "s"),
    spec("op5_sustain", 0.0, 1.0, 0.5, ""),
    spec("op6_ratio", 0.25, 16.0, 4.0, ""),
    spec("op6_level", 0.0, 1.0, 0.0, ""),
    spec("op6_decay", 0.01, 10.0, 1.0, "s"),
    spec("op6_sustain", 0.0, 1.0, 0.5, ""),
    int_spec("op1_wave", 0.0, 3.0, 0.0),
    int_spec("op2_wave", 0.0, 3.0, 0.0),
    int_spec("op3_wave", 0.0, 3.0, 0.0),
    int_spec("op4_wave", 0.0, 3.0, 0.0),
    int_spec("op5_wave", 0.0, 3.0, 0.0),
    int_spec("op6_wave", 0.0, 3.0, 0.0),
    spec("fm_cutoff", 20.0, 20_000.0, 20_000.0, "Hz"),
    spec("fm_resonance", 0.0, 1.0, 0.2, ""),
    spec("pluck_bow", 0.0, 1.0, 0.0, ""),
    int_spec("pluck_strings", 1.0, 3.0, 1.0),
    spec("pluck_detune", 0.0, 30.0, 6.0, "cents"),
    spec("add_bright_b", 0.0, 1.0, 0.5, ""),
    spec("add_even_b", 0.0, 1.0, 1.0, ""),
    spec("add_inharm_b", 0.0, 1.0, 0.0, ""),
    spec("add_morph", 0.0, 1.0, 0.0, ""),
    spec("add_morph_time", 0.0, 10.0, 0.0, "s"),
    spec("add_morph_rate", 0.0, 10.0, 0.0, "Hz"),
    spec("add_morph_depth", 0.0, 1.0, 0.5, ""),
    int_spec("midi_cc1", -1.0, 127.0, -1.0),
    int_spec("midi_cc2", -1.0, 127.0, -1.0),
    int_spec("midi_cc3", -1.0, 127.0, -1.0),
    int_spec("midi_cc4", -1.0, 127.0, -1.0),
    int_spec("midi_cc5", -1.0, 127.0, -1.0),
    int_spec("midi_cc6", -1.0, 127.0, -1.0),
    int_spec("midi_cc7", -1.0, 127.0, -1.0),
    int_spec("midi_cc8", -1.0, 127.0, -1.0),
    int_spec("midi_cc1_number", 0.0, 127.0, 1.0),
    int_spec("midi_cc2_number", 0.0, 127.0, 7.0),
    int_spec("midi_cc3_number", 0.0, 127.0, 10.0),
    int_spec("midi_cc4_number", 0.0, 127.0, 11.0),
    int_spec("midi_cc5_number", 0.0, 127.0, 71.0),
    int_spec("midi_cc6_number", 0.0, 127.0, 74.0),
    int_spec("midi_cc7_number", 0.0, 127.0, 91.0),
    int_spec("midi_cc8_number", 0.0, 127.0, 93.0),
    int_spec("midi_program", 0.0, 128.0, 0.0),
    spec("resyn_speed", 0.0, 4.0, 1.0, ""),
    spec("resyn_position", 0.0, 1.0, 0.0, ""),
    spec("resyn_blur", 0.0, 1.0, 0.0, ""),
    int_spec("resyn_loop", 0.0, 1.0, 0.0),
];

/// Number of MIDI Out control-change knobs.
pub const MIDI_CCS: usize = 8;

/// Names of the FM operator waveforms, by `opN_wave` value.
pub const FM_WAVES: [&str; 4] = ["sine", "half sine", "soft square", "soft saw"];
/// Names of the FM algorithms, by `fm_algo` value (6–11 use six operators).
pub const FM_ALGOS: [&str; 12] = [
    "4>3>2>1",
    "4>3, 3+2>1",
    "4>3 2>1",
    "4>1,2,3",
    "2>1 +3 +4",
    "1+2+3+4",
    "6>5>4>3>2>1",
    "6>5>4 3>2>1",
    "6>5 4>3 2>1",
    "2..6>1",
    "6>1..5",
    "1..6",
];

/// Names of the synth's modulation sources, by `modN_source` value.
pub const MOD_SOURCES: [&str; 7] = [
    "off", "LFO 1", "LFO 2", "mod env", "velocity", "key", "amp env",
];
/// Names of the synth's modulation destinations, by `modN_dest` value.
pub const MOD_DESTS: [&str; 9] = [
    "pitch",
    "cutoff",
    "cutoff 2",
    "resonance",
    "osc mix",
    "volume",
    "pan",
    "detune",
    "osc 3",
];
/// Number of modulation slots.
pub const MOD_SLOTS: usize = 3;

/// Names of the keys instrument's types, by `keys_type` value.
pub const KEYS_TYPES: [&str; 3] = ["piano", "epiano", "organ"];

/// Names of the drum synth's sounds, by `drum_type` value.
pub const DRUM_TYPES: [&str; 5] = ["kick", "snare", "hat", "tom", "clap"];

/// Names of the arpeggiator modes, by `arp_mode` value.
pub const ARP_MODES: [&str; 5] = ["off", "up", "down", "up-down", "random"];
/// Arpeggiator rates by `arp_rate` value: (label, ticks).
pub const ARP_RATES: [(&str, u32); 5] = [
    ("1/32", TICKS_PER_STEP / 2),
    ("1/16", TICKS_PER_STEP),
    ("1/8", TICKS_PER_STEP * 2),
    ("1/4", TICKS_PER_STEP * 4),
    ("1/2", TICKS_PER_STEP * 8),
];

/// Names of the LFO targets, by `lfo_target` value.
pub const LFO_TARGETS: [&str; 3] = ["pitch", "cutoff", "volume"];
/// Names of the synth filter types, by `filter_type` value.
pub const FILTER_TYPES: [&str; 3] = ["low-pass", "band-pass", "high-pass"];

/// Generated impulse responses for the convolution reverb, used as
/// `ir:<name>`.
pub const IMPULSE_RESPONSES: [&str; 5] = ["room", "chamber", "plate", "hall", "cathedral"];

/// Built-in insert effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    /// State-variable filter (low-pass, band-pass, high-pass).
    Filter,
    /// Three-band equalizer.
    Eq,
    /// Tempo-synced stereo delay.
    Delay,
    /// Algorithmic reverb.
    Reverb,
    /// Feed-forward compressor.
    Compressor,
    /// Soft-clipping distortion.
    Distortion,
    /// Stereo chorus (modulated delay).
    Chorus,
    /// Peak limiter.
    Limiter,
    /// Swept all-pass phaser.
    Phaser,
    /// Short modulated delay with feedback.
    Flanger,
    /// Bit depth and sample-rate reduction.
    Bitcrusher,
    /// Width, balance and phase.
    Stereo,
    /// Tempo-synced 16-step volume gate.
    Gate,
    /// Tempo-synced auto-pan.
    Autopan,
    /// Low shelf with saturation.
    BassBoost,
    /// Asymmetric saturation with a tone filter.
    Overdrive,
    /// Soft clipper with a knee.
    SoftClipper,
    /// Selectable transfer curves.
    Waveshaper,
    /// Removes DC offset.
    DcBlock,
    /// Multiplies the signal by a sine.
    RingMod,
    /// Shifts every frequency by the same number of hertz.
    FreqShift,
    /// Vowel (formant) filter.
    Formant,
    /// Filter swept by a tempo-synced LFO and the input level.
    AutoFilter,
    /// Silences the signal below a threshold.
    NoiseGate,
    /// Boosts or softens attacks and sustain.
    Transient,
    /// Four-tap tempo-synced delay.
    MultiTap,
    /// Seven-band graphic equalizer.
    GraphicEq,
    /// Seven-band parametric equalizer.
    ParametricEq,
    /// Three-band compressor.
    Multiband,
    /// Channel vocoder; the sidechain is the modulator.
    Vocoder,
    /// Delay-line pitch shifter.
    PitchShift,
    /// Tempo-synced slice repeater.
    Stutter,
    /// Convolution reverb with a loaded or generated impulse response.
    Convolution,
    /// Detects the pitch and pulls it to the nearest note of a scale.
    PitchCorrect,
    /// Shows the detected pitch; leaves audio untouched.
    Tuner,
    /// Noise gate, compressor and limiter in series.
    Dynamics,
    /// Left/right mixing matrix with an inter-channel delay.
    StereoShaper,
    /// Three-band limiter with a master ceiling.
    MultibandLimiter,
    /// Sends a copy of the audio at this point in the chain to a higher-numbered insert; routed by the engine.
    Send,
    /// Dense feedback-delay-network reverb with early reflections.
    HallReverb,
    /// Compressor, drive, tone stack, chorus and cabinet in one insert.
    GuitarChain,
    /// One of several effect blocks played with an X/Y pad.
    MultiFx,
    /// Four X/Y effect blocks in series in one insert.
    FxChain,
    /// Drawn time and volume envelopes over a tempo-synced cycle.
    BeatShaper,
    /// Splits the audio into three bands sent to other inserts; routed by the engine.
    BandSplit,
    /// Freezes recent audio and moves the play point through it like a record.
    Scratch,
    /// Up to four parallel chains of built-in effects in one insert.
    Rack,
    /// A hosted plugin; its parameters belong to the plugin, so its table is
    /// empty. Not in [`EFFECT_KINDS`] and not accepted by [`EffectKind::parse`].
    Plugin,
}

/// Every effect kind, in menu order.
pub const EFFECT_KINDS: [EffectKind; 47] = [
    EffectKind::Filter,
    EffectKind::AutoFilter,
    EffectKind::Formant,
    EffectKind::Eq,
    EffectKind::GraphicEq,
    EffectKind::ParametricEq,
    EffectKind::Delay,
    EffectKind::MultiTap,
    EffectKind::Stutter,
    EffectKind::BeatShaper,
    EffectKind::Scratch,
    EffectKind::Rack,
    EffectKind::MultiFx,
    EffectKind::FxChain,
    EffectKind::Reverb,
    EffectKind::HallReverb,
    EffectKind::Convolution,
    EffectKind::Chorus,
    EffectKind::Phaser,
    EffectKind::Flanger,
    EffectKind::RingMod,
    EffectKind::FreqShift,
    EffectKind::PitchShift,
    EffectKind::PitchCorrect,
    EffectKind::Tuner,
    EffectKind::Vocoder,
    EffectKind::Compressor,
    EffectKind::Multiband,
    EffectKind::MultibandLimiter,
    EffectKind::Limiter,
    EffectKind::Dynamics,
    EffectKind::NoiseGate,
    EffectKind::Transient,
    EffectKind::SoftClipper,
    EffectKind::Gate,
    EffectKind::Autopan,
    EffectKind::Stereo,
    EffectKind::Send,
    EffectKind::BandSplit,
    EffectKind::StereoShaper,
    EffectKind::DcBlock,
    EffectKind::Distortion,
    EffectKind::Overdrive,
    EffectKind::Waveshaper,
    EffectKind::GuitarChain,
    EffectKind::BassBoost,
    EffectKind::Bitcrusher,
];

const BASS_BOOST_PARAMS: &[ParamSpec] = &[
    spec("freq", 30.0, 300.0, 80.0, "Hz"),
    spec("boost", 0.0, 18.0, 6.0, "dB"),
    spec("drive", 0.0, 1.0, 0.2, ""),
];

const OVERDRIVE_PARAMS: &[ParamSpec] = &[
    spec("drive", 0.0, 48.0, 18.0, "dB"),
    spec("tone", 300.0, 16_000.0, 3_000.0, "Hz"),
    spec("bias", 0.0, 0.5, 0.1, ""),
    spec("mix", 0.0, 1.0, 1.0, ""),
    spec("output", -24.0, 6.0, -6.0, "dB"),
];

const SOFT_CLIPPER_PARAMS: &[ParamSpec] = &[
    spec("threshold", -24.0, 0.0, -3.0, "dB"),
    spec("knee", 0.0, 1.0, 0.5, ""),
    spec("output", -24.0, 6.0, 0.0, "dB"),
];

const WAVESHAPER_PARAMS: &[ParamSpec] = &[
    int_spec("shape", 0.0, 6.0, 0.0),
    spec("drive", 0.0, 36.0, 12.0, "dB"),
    spec("mix", 0.0, 1.0, 1.0, ""),
    spec("output", -24.0, 6.0, -6.0, "dB"),
    spec("point_1", -1.0, 1.0, -1.0, ""),
    spec("point_2", -1.0, 1.0, -0.75, ""),
    spec("point_3", -1.0, 1.0, -0.5, ""),
    spec("point_4", -1.0, 1.0, -0.25, ""),
    spec("point_5", -1.0, 1.0, 0.0, ""),
    spec("point_6", -1.0, 1.0, 0.25, ""),
    spec("point_7", -1.0, 1.0, 0.5, ""),
    spec("point_8", -1.0, 1.0, 0.75, ""),
    spec("point_9", -1.0, 1.0, 1.0, ""),
];

/// Number of points of the waveshaper's drawn curve.
pub const SHAPER_POINTS: usize = 9;
/// Index of the waveshaper's first drawn-curve point.
pub const SHAPER_FIRST_POINT: usize = 4;

const DC_BLOCK_PARAMS: &[ParamSpec] = &[spec("freq", 2.0, 40.0, 10.0, "Hz")];

const RING_MOD_PARAMS: &[ParamSpec] = &[
    spec("freq", 1.0, 5_000.0, 440.0, "Hz"),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

const FREQ_SHIFT_PARAMS: &[ParamSpec] = &[
    spec("shift", -2_000.0, 2_000.0, 100.0, "Hz"),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

const FORMANT_PARAMS: &[ParamSpec] = &[
    spec("vowel", 0.0, 4.0, 0.0, ""),
    spec("shift", -12.0, 12.0, 0.0, "st"),
    spec("resonance", 0.0, 1.0, 0.6, ""),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

const AUTO_FILTER_PARAMS: &[ParamSpec] = &[
    int_spec("mode", 0.0, 2.0, 0.0),
    spec("cutoff", 20.0, 20_000.0, 1_000.0, "Hz"),
    spec("resonance", 0.0, 1.0, 0.5, ""),
    int_spec("rate", 0.0, 5.0, 3.0),
    spec("depth", 0.0, 4.0, 2.0, "oct"),
    spec("envelope", -4.0, 4.0, 0.0, "oct"),
    spec("mix", 0.0, 1.0, 1.0, ""),
    int_spec("lfo_shape", 0.0, 5.0, 0.0),
    spec("step_1", -1.0, 1.0, 0.0, ""),
    spec("step_2", -1.0, 1.0, 0.7, ""),
    spec("step_3", -1.0, 1.0, 1.0, ""),
    spec("step_4", -1.0, 1.0, 0.7, ""),
    spec("step_5", -1.0, 1.0, 0.0, ""),
    spec("step_6", -1.0, 1.0, -0.7, ""),
    spec("step_7", -1.0, 1.0, -1.0, ""),
    spec("step_8", -1.0, 1.0, -0.7, ""),
    spec("volume", 0.0, 1.0, 0.0, ""),
    spec("pan", -1.0, 1.0, 0.0, ""),
];

/// Number of points of the auto filter's drawn LFO shape.
pub const LFO_STEPS: usize = 8;
/// Index of the auto filter's first drawn LFO point.
pub const LFO_FIRST_STEP: usize = 8;
/// Names of the auto filter's LFO shapes, by `lfo_shape` value.
pub const LFO_SHAPES: [&str; 6] = ["sine", "triangle", "saw up", "saw down", "square", "drawn"];

const NOISE_GATE_PARAMS: &[ParamSpec] = &[
    spec("threshold", -80.0, 0.0, -40.0, "dB"),
    spec("attack", 0.01, 50.0, 1.0, "ms"),
    spec("hold", 0.0, 500.0, 20.0, "ms"),
    spec("release", 1.0, 2_000.0, 100.0, "ms"),
    spec("range", -80.0, 0.0, -80.0, "dB"),
];

const DYNAMICS_PARAMS: &[ParamSpec] = &[
    spec("gate_threshold", -100.0, 0.0, -100.0, "dB"),
    spec("gate_attack", 0.01, 50.0, 1.0, "ms"),
    spec("gate_hold", 0.0, 500.0, 20.0, "ms"),
    spec("gate_release", 1.0, 2_000.0, 100.0, "ms"),
    spec("gate_range", -80.0, 0.0, -80.0, "dB"),
    spec("comp_threshold", -60.0, 0.0, -12.0, "dB"),
    spec("comp_ratio", 1.0, 20.0, 1.0, ":1"),
    spec("comp_attack", 0.1, 200.0, 10.0, "ms"),
    spec("comp_release", 5.0, 2_000.0, 120.0, "ms"),
    spec("comp_makeup", 0.0, 24.0, 0.0, "dB"),
    spec("limit_gain", 0.0, 24.0, 0.0, "dB"),
    spec("limit_ceiling", -12.0, 0.0, -0.3, "dB"),
    spec("limit_release", 10.0, 1_000.0, 80.0, "ms"),
];

const STEREO_SHAPER_PARAMS: &[ParamSpec] = &[
    spec("left_to_left", -1.0, 1.0, 1.0, ""),
    spec("right_to_left", -1.0, 1.0, 0.0, ""),
    spec("left_to_right", -1.0, 1.0, 0.0, ""),
    spec("right_to_right", -1.0, 1.0, 1.0, ""),
    spec("delay", -20.0, 20.0, 0.0, "ms"),
    spec("output", -24.0, 6.0, 0.0, "dB"),
];

const MULTIBAND_LIMITER_PARAMS: &[ParamSpec] = &[
    spec("low_freq", 40.0, 1_000.0, 200.0, "Hz"),
    spec("high_freq", 500.0, 16_000.0, 2_500.0, "Hz"),
    spec("low_drive", 0.0, 24.0, 0.0, "dB"),
    spec("low_ceiling", -24.0, 0.0, 0.0, "dB"),
    spec("low_level", -18.0, 12.0, 0.0, "dB"),
    spec("mid_drive", 0.0, 24.0, 0.0, "dB"),
    spec("mid_ceiling", -24.0, 0.0, 0.0, "dB"),
    spec("mid_level", -18.0, 12.0, 0.0, "dB"),
    spec("high_drive", 0.0, 24.0, 0.0, "dB"),
    spec("high_ceiling", -24.0, 0.0, 0.0, "dB"),
    spec("high_level", -18.0, 12.0, 0.0, "dB"),
    spec("release", 5.0, 1_000.0, 100.0, "ms"),
    spec("amount", 0.0, 1.0, 1.0, ""),
    spec("ceiling", -12.0, 0.0, -0.3, "dB"),
];

const SEND_PARAMS: &[ParamSpec] = &[
    int_spec("target", 0.0, 127.0, 0.0),
    spec("level", 0.0, 1.25, 1.0, ""),
    spec("pan", -1.0, 1.0, 0.0, ""),
    spec("dry", 0.0, 1.0, 1.0, ""),
];

const HALL_REVERB_PARAMS: &[ParamSpec] = &[
    spec("predelay", 0.0, 200.0, 20.0, "ms"),
    spec("decay", 0.2, 20.0, 2.5, "s"),
    spec("size", 0.0, 1.0, 0.6, ""),
    spec("damping", 500.0, 20_000.0, 6_000.0, "Hz"),
    spec("low_cut", 20.0, 1_000.0, 100.0, "Hz"),
    spec("early", 0.0, 1.0, 0.4, ""),
    spec("mod_depth", 0.0, 1.0, 0.3, ""),
    spec("mod_rate", 0.05, 3.0, 0.4, "Hz"),
    spec("width", 0.0, 1.0, 1.0, ""),
    spec("mix", 0.0, 1.0, 0.3, ""),
];

const GUITAR_CHAIN_PARAMS: &[ParamSpec] = &[
    spec("compress", 0.0, 1.0, 0.3, ""),
    spec("drive", 0.0, 48.0, 18.0, "dB"),
    int_spec("drive_type", 0.0, 2.0, 0.0),
    spec("bass", -12.0, 12.0, 0.0, "dB"),
    spec("mid", -12.0, 12.0, 0.0, "dB"),
    spec("treble", -12.0, 12.0, 0.0, "dB"),
    spec("chorus", 0.0, 1.0, 0.0, ""),
    int_spec("cabinet", 0.0, 2.0, 1.0),
    spec("output", -24.0, 6.0, -6.0, "dB"),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

const MULTI_FX_PARAMS: &[ParamSpec] = &[
    int_spec("block", 0.0, 13.0, 1.0),
    spec("x", 0.0, 1.0, 0.5, ""),
    spec("y", 0.0, 1.0, 0.5, ""),
    spec("mix", 0.0, 1.0, 1.0, ""),
    spec("output", -24.0, 6.0, 0.0, "dB"),
];

const FX_CHAIN_PARAMS: &[ParamSpec] = &[
    int_spec("stage1_block", 0.0, 13.0, 3.0),
    spec("stage1_x", 0.0, 1.0, 0.3, ""),
    spec("stage1_y", 0.0, 1.0, 1.0, ""),
    int_spec("stage2_block", 0.0, 13.0, 1.0),
    spec("stage2_x", 0.0, 1.0, 0.6, ""),
    spec("stage2_y", 0.0, 1.0, 0.3, ""),
    int_spec("stage3_block", 0.0, 13.0, 0.0),
    spec("stage3_x", 0.0, 1.0, 0.5, ""),
    spec("stage3_y", 0.0, 1.0, 0.5, ""),
    int_spec("stage4_block", 0.0, 13.0, 0.0),
    spec("stage4_x", 0.0, 1.0, 0.5, ""),
    spec("stage4_y", 0.0, 1.0, 0.5, ""),
    spec("mix", 0.0, 1.0, 1.0, ""),
    spec("output", -24.0, 6.0, 0.0, "dB"),
];

const BEAT_SHAPER_PARAMS: &[ParamSpec] = &[
    int_spec("cycle", 0.0, 3.0, 2.0),
    int_spec("time_mode", 0.0, 1.0, 0.0),
    spec("time_1", 0.0, 1.0, 0.0, ""),
    spec("time_2", 0.0, 1.0, 0.0, ""),
    spec("time_3", 0.0, 1.0, 0.0, ""),
    spec("time_4", 0.0, 1.0, 0.0, ""),
    spec("time_5", 0.0, 1.0, 0.0, ""),
    spec("time_6", 0.0, 1.0, 0.0, ""),
    spec("time_7", 0.0, 1.0, 0.0, ""),
    spec("time_8", 0.0, 1.0, 0.0, ""),
    spec("volume_1", 0.0, 1.0, 1.0, ""),
    spec("volume_2", 0.0, 1.0, 1.0, ""),
    spec("volume_3", 0.0, 1.0, 1.0, ""),
    spec("volume_4", 0.0, 1.0, 1.0, ""),
    spec("volume_5", 0.0, 1.0, 1.0, ""),
    spec("volume_6", 0.0, 1.0, 1.0, ""),
    spec("volume_7", 0.0, 1.0, 1.0, ""),
    spec("volume_8", 0.0, 1.0, 1.0, ""),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

const BAND_SPLIT_PARAMS: &[ParamSpec] = &[
    spec("low_freq", 40.0, 1_000.0, 200.0, "Hz"),
    spec("high_freq", 500.0, 16_000.0, 2_500.0, "Hz"),
    int_spec("low_target", 0.0, 127.0, 0.0),
    int_spec("mid_target", 0.0, 127.0, 0.0),
    int_spec("high_target", 0.0, 127.0, 0.0),
];

const SCRATCH_PARAMS: &[ParamSpec] = &[
    int_spec("hold", 0.0, 1.0, 0.0),
    spec("position", 0.0, 1.0, 0.0, ""),
    spec("span", 0.0625, 8.0, 1.0, "beats"),
    int_spec("pattern", 0.0, 4.0, 0.0),
    int_spec("rate", 0.0, 5.0, 3.0),
    spec("depth", 0.0, 1.0, 1.0, ""),
    spec("inertia", 0.1, 200.0, 5.0, "ms"),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

/// A hosted plugin effect's own settings (its parameters are the plugin's):
/// the insert whose output feeds the plugin's first auxiliary input.
const PLUGIN_PARAMS: &[ParamSpec] = &[int_spec("sidechain", 0.0, 127.0, 0.0)];

const RACK_PARAMS: &[ParamSpec] = &[
    spec("dry", 0.0, 1.0, 1.0, ""),
    spec("branch1_level", 0.0, 1.25, 1.0, ""),
    spec("branch1_pan", -1.0, 1.0, 0.0, ""),
    spec("branch2_level", 0.0, 1.25, 1.0, ""),
    spec("branch2_pan", -1.0, 1.0, 0.0, ""),
    spec("branch3_level", 0.0, 1.25, 1.0, ""),
    spec("branch3_pan", -1.0, 1.0, 0.0, ""),
    spec("branch4_level", 0.0, 1.25, 1.0, ""),
    spec("branch4_pan", -1.0, 1.0, 0.0, ""),
    spec("output", -24.0, 12.0, 0.0, "dB"),
];

const TRANSIENT_PARAMS: &[ParamSpec] = &[
    spec("attack", -1.0, 1.0, 0.0, ""),
    spec("sustain", -1.0, 1.0, 0.0, ""),
    spec("output", -24.0, 6.0, 0.0, "dB"),
];

const MULTI_TAP_PARAMS: &[ParamSpec] = &[
    spec("tap1_time", 0.0625, 4.0, 0.5, "beats"),
    spec("tap1_level", 0.0, 1.0, 0.6, ""),
    spec("tap2_time", 0.0625, 4.0, 0.75, "beats"),
    spec("tap2_level", 0.0, 1.0, 0.45, ""),
    spec("tap3_time", 0.0625, 4.0, 1.0, "beats"),
    spec("tap3_level", 0.0, 1.0, 0.3, ""),
    spec("tap4_time", 0.0625, 4.0, 1.5, "beats"),
    spec("tap4_level", 0.0, 1.0, 0.2, ""),
    spec("feedback", 0.0, 0.9, 0.3, ""),
    spec("mix", 0.0, 1.0, 0.35, ""),
    spec("damping", 0.0, 0.95, 0.3, ""),
    spec("tap1_pan", -1.0, 1.0, -1.0, ""),
    spec("tap2_pan", -1.0, 1.0, 1.0, ""),
    spec("tap3_pan", -1.0, 1.0, -1.0, ""),
    spec("tap4_pan", -1.0, 1.0, 1.0, ""),
    spec("tap1_low_cut", 20.0, 2_000.0, 20.0, "Hz"),
    spec("tap2_low_cut", 20.0, 2_000.0, 20.0, "Hz"),
    spec("tap3_low_cut", 20.0, 2_000.0, 20.0, "Hz"),
    spec("tap4_low_cut", 20.0, 2_000.0, 20.0, "Hz"),
    spec("tap1_high_cut", 500.0, 20_000.0, 20_000.0, "Hz"),
    spec("tap2_high_cut", 500.0, 20_000.0, 20_000.0, "Hz"),
    spec("tap3_high_cut", 500.0, 20_000.0, 20_000.0, "Hz"),
    spec("tap4_high_cut", 500.0, 20_000.0, 20_000.0, "Hz"),
];

const GRAPHIC_EQ_PARAMS: &[ParamSpec] = &[
    spec("band_63", -18.0, 18.0, 0.0, "dB"),
    spec("band_160", -18.0, 18.0, 0.0, "dB"),
    spec("band_400", -18.0, 18.0, 0.0, "dB"),
    spec("band_1k", -18.0, 18.0, 0.0, "dB"),
    spec("band_2k5", -18.0, 18.0, 0.0, "dB"),
    spec("band_6k3", -18.0, 18.0, 0.0, "dB"),
    spec("band_16k", -18.0, 18.0, 0.0, "dB"),
    spec("output", -18.0, 6.0, 0.0, "dB"),
];

macro_rules! band {
    ($n:literal, $freq:expr, $q:expr) => {
        [
            spec(concat!("band", $n, "_freq"), 20.0, 20_000.0, $freq, "Hz"),
            spec(concat!("band", $n, "_gain"), -18.0, 18.0, 0.0, "dB"),
            spec(concat!("band", $n, "_q"), 0.1, 10.0, $q, ""),
        ]
    };
}

const PARAMETRIC_BANDS: [[ParamSpec; 3]; 7] = [
    band!("1", 60.0, 0.7),
    band!("2", 150.0, 1.0),
    band!("3", 400.0, 1.0),
    band!("4", 1_000.0, 1.0),
    band!("5", 2_500.0, 1.0),
    band!("6", 6_000.0, 1.0),
    band!("7", 12_000.0, 0.7),
];

const PARAMETRIC_EQ_PARAMS: &[ParamSpec] = &[
    PARAMETRIC_BANDS[0][0],
    PARAMETRIC_BANDS[0][1],
    PARAMETRIC_BANDS[0][2],
    PARAMETRIC_BANDS[1][0],
    PARAMETRIC_BANDS[1][1],
    PARAMETRIC_BANDS[1][2],
    PARAMETRIC_BANDS[2][0],
    PARAMETRIC_BANDS[2][1],
    PARAMETRIC_BANDS[2][2],
    PARAMETRIC_BANDS[3][0],
    PARAMETRIC_BANDS[3][1],
    PARAMETRIC_BANDS[3][2],
    PARAMETRIC_BANDS[4][0],
    PARAMETRIC_BANDS[4][1],
    PARAMETRIC_BANDS[4][2],
    PARAMETRIC_BANDS[5][0],
    PARAMETRIC_BANDS[5][1],
    PARAMETRIC_BANDS[5][2],
    PARAMETRIC_BANDS[6][0],
    PARAMETRIC_BANDS[6][1],
    PARAMETRIC_BANDS[6][2],
    spec("output", -18.0, 6.0, 0.0, "dB"),
];

const MULTIBAND_PARAMS: &[ParamSpec] = &[
    spec("low_freq", 40.0, 1_000.0, 200.0, "Hz"),
    spec("high_freq", 500.0, 16_000.0, 2_500.0, "Hz"),
    spec("low_thresh", -60.0, 0.0, -18.0, "dB"),
    spec("low_ratio", 1.0, 20.0, 3.0, ":1"),
    spec("low_gain", -18.0, 18.0, 0.0, "dB"),
    spec("mid_thresh", -60.0, 0.0, -18.0, "dB"),
    spec("mid_ratio", 1.0, 20.0, 3.0, ":1"),
    spec("mid_gain", -18.0, 18.0, 0.0, "dB"),
    spec("high_thresh", -60.0, 0.0, -18.0, "dB"),
    spec("high_ratio", 1.0, 20.0, 3.0, ":1"),
    spec("high_gain", -18.0, 18.0, 0.0, "dB"),
    spec("attack", 0.1, 200.0, 10.0, "ms"),
    spec("release", 5.0, 2_000.0, 150.0, "ms"),
    spec("output", -18.0, 12.0, 0.0, "dB"),
];

const VOCODER_PARAMS: &[ParamSpec] = &[
    int_spec("sidechain", 0.0, 127.0, 0.0),
    spec("bandwidth", 0.0, 1.0, 0.5, ""),
    spec("attack", 0.1, 100.0, 5.0, "ms"),
    spec("release", 5.0, 1_000.0, 60.0, "ms"),
    spec("shift", -12.0, 12.0, 0.0, "st"),
    spec("noise", 0.0, 1.0, 0.1, ""),
    spec("mix", 0.0, 1.0, 1.0, ""),
    spec("output", -24.0, 12.0, 0.0, "dB"),
    int_spec("bands", 4.0, 64.0, 16.0),
    spec("low", 40.0, 1_000.0, 120.0, "Hz"),
    spec("high", 2_000.0, 16_000.0, 7_500.0, "Hz"),
];

const PITCH_SHIFT_PARAMS: &[ParamSpec] = &[
    spec("pitch", -24.0, 24.0, 12.0, "st"),
    spec("fine", -100.0, 100.0, 0.0, "ct"),
    spec("window", 10.0, 100.0, 50.0, "ms"),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

const PITCH_CORRECT_PARAMS: &[ParamSpec] = &[
    int_spec("key", 0.0, 11.0, 0.0),
    int_spec("scale", 0.0, 5.0, 0.0),
    spec("speed", 0.0, 400.0, 20.0, "ms"),
    spec("amount", 0.0, 1.0, 1.0, ""),
    spec("reference", 415.0, 466.0, 440.0, "Hz"),
    spec("mix", 0.0, 1.0, 1.0, ""),
    int_spec("follow", 0.0, 999.0, 0.0),
    int_spec("harmony_1", -12.0, 12.0, 0.0),
    int_spec("harmony_2", -12.0, 12.0, 0.0),
    spec("harmony_level", 0.0, 1.0, 0.7, ""),
];

const TUNER_PARAMS: &[ParamSpec] = &[spec("reference", 415.0, 466.0, 440.0, "Hz")];

/// Pitch-class names, starting at C.
pub const NOTE_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

const fn scale_mask(degrees: &[u8]) -> u16 {
    let mut mask = 0_u16;
    let mut i = 0;
    while i < degrees.len() {
        mask |= 1 << (degrees[i] % 12);
        i += 1;
    }
    mask
}

/// Scales of the pitch corrector's `scale` parameter: (name, bit `n` set when
/// the note `n` semitones above the key belongs to the scale).
pub const CORRECTION_SCALES: [(&str, u16); 6] = [
    ("chromatic", 0x0FFF),
    ("major", scale_mask(&[0, 2, 4, 5, 7, 9, 11])),
    ("minor", scale_mask(&[0, 2, 3, 5, 7, 8, 10])),
    ("harmonic minor", scale_mask(&[0, 2, 3, 5, 7, 8, 11])),
    ("major pentatonic", scale_mask(&[0, 2, 4, 7, 9])),
    ("minor pentatonic", scale_mask(&[0, 3, 5, 7, 10])),
];

const STUTTER_PARAMS: &[ParamSpec] = &[
    int_spec("slice", 0.0, 5.0, 1.0),
    int_spec("every", 0.0, 5.0, 5.0),
    spec("portion", 0.0, 1.0, 0.25, ""),
    spec("decay", 0.0, 1.0, 1.0, ""),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

const CONVOLUTION_PARAMS: &[ParamSpec] = &[
    spec("mix", 0.0, 1.0, 0.3, ""),
    spec("predelay", 0.0, 250.0, 0.0, "ms"),
    spec("low_cut", 20.0, 1_000.0, 20.0, "Hz"),
    spec("high_cut", 1_000.0, 20_000.0, 20_000.0, "Hz"),
    spec("output", -24.0, 12.0, 0.0, "dB"),
];

const PHASER_PARAMS: &[ParamSpec] = &[
    spec("rate", 0.05, 10.0, 0.5, "Hz"),
    spec("depth", 0.0, 1.0, 0.7, ""),
    spec("feedback", 0.0, 0.9, 0.5, ""),
    int_spec("stages", 2.0, 12.0, 6.0),
    spec("mix", 0.0, 1.0, 0.5, ""),
    spec("warm", 0.0, 1.0, 0.0, ""),
];

const FLANGER_PARAMS: &[ParamSpec] = &[
    spec("rate", 0.05, 5.0, 0.25, "Hz"),
    spec("depth", 0.1, 5.0, 2.0, "ms"),
    spec("delay", 0.5, 10.0, 2.0, "ms"),
    spec("feedback", -0.95, 0.95, 0.5, ""),
    spec("mix", 0.0, 1.0, 0.5, ""),
];

const BITCRUSHER_PARAMS: &[ParamSpec] = &[
    int_spec("bits", 1.0, 16.0, 8.0),
    int_spec("downsample", 1.0, 32.0, 4.0),
    spec("mix", 0.0, 1.0, 1.0, ""),
    spec("output", -24.0, 6.0, 0.0, "dB"),
    spec("squeeze", 0.0, 1.0, 0.0, ""),
];

const STEREO_PARAMS: &[ParamSpec] = &[
    spec("width", 0.0, 2.0, 1.0, ""),
    spec("balance", -1.0, 1.0, 0.0, ""),
    int_spec("invert_left", 0.0, 1.0, 0.0),
    int_spec("invert_right", 0.0, 1.0, 0.0),
];

const GATE_PARAMS: &[ParamSpec] = &[
    int_spec("rate", 0.0, 5.0, 1.0),
    spec("duty", 0.05, 1.0, 0.5, ""),
    spec("depth", 0.0, 1.0, 1.0, ""),
    spec("smooth", 0.0, 20.0, 3.0, "ms"),
    int_spec("steps", 0.0, 65_535.0, 65_535.0),
];

const AUTOPAN_PARAMS: &[ParamSpec] = &[
    int_spec("rate", 0.0, 5.0, 3.0),
    spec("depth", 0.0, 1.0, 0.7, ""),
    int_spec("shape", 0.0, 2.0, 0.0),
];

const FILTER_PARAMS: &[ParamSpec] = &[
    int_spec("mode", 0.0, 2.0, 0.0),
    spec("cutoff", 20.0, 20_000.0, 2_000.0, "Hz"),
    spec("resonance", 0.0, 1.0, 0.3, ""),
    spec("mix", 0.0, 1.0, 1.0, ""),
];

const EQ_PARAMS: &[ParamSpec] = &[
    spec("low_gain", -18.0, 18.0, 0.0, "dB"),
    spec("low_freq", 20.0, 1_000.0, 120.0, "Hz"),
    spec("mid_gain", -18.0, 18.0, 0.0, "dB"),
    spec("mid_freq", 100.0, 10_000.0, 1_000.0, "Hz"),
    spec("mid_q", 0.1, 10.0, 0.7, ""),
    spec("high_gain", -18.0, 18.0, 0.0, "dB"),
    spec("high_freq", 1_000.0, 20_000.0, 8_000.0, "Hz"),
];

const DELAY_PARAMS: &[ParamSpec] = &[
    spec("time", 0.0625, 4.0, 0.75, "beats"),
    spec("feedback", 0.0, 0.95, 0.4, ""),
    spec("mix", 0.0, 1.0, 0.3, ""),
    int_spec("ping_pong", 0.0, 1.0, 1.0),
    spec("damping", 0.0, 1.0, 0.3, ""),
    spec("offset", -20.0, 20.0, 0.0, "ms"),
    spec("mod_rate", 0.05, 5.0, 0.5, "Hz"),
    spec("mod_depth", 0.0, 10.0, 0.0, "ms"),
    spec("low_cut", 20.0, 2_000.0, 20.0, "Hz"),
    spec("diffusion", 0.0, 0.9, 0.0, ""),
    spec("saturation", 0.0, 1.0, 0.0, ""),
];

const REVERB_PARAMS: &[ParamSpec] = &[
    spec("size", 0.0, 1.0, 0.6, ""),
    spec("damping", 0.0, 1.0, 0.5, ""),
    spec("width", 0.0, 1.0, 1.0, ""),
    spec("mix", 0.0, 1.0, 0.25, ""),
];

const COMPRESSOR_PARAMS: &[ParamSpec] = &[
    spec("threshold", -60.0, 0.0, -18.0, "dB"),
    spec("ratio", 1.0, 20.0, 4.0, ":1"),
    spec("attack", 0.1, 200.0, 10.0, "ms"),
    spec("release", 5.0, 2_000.0, 120.0, "ms"),
    spec("makeup", 0.0, 24.0, 0.0, "dB"),
    int_spec("sidechain", 0.0, 127.0, 0.0),
];

const CHORUS_PARAMS: &[ParamSpec] = &[
    spec("rate", 0.05, 5.0, 0.8, "Hz"),
    spec("depth", 0.0, 10.0, 3.0, "ms"),
    spec("delay", 5.0, 30.0, 12.0, "ms"),
    spec("mix", 0.0, 1.0, 0.4, ""),
    int_spec("voices", 1.0, 3.0, 1.0),
    spec("tone", 500.0, 20_000.0, 20_000.0, "Hz"),
];

const LIMITER_PARAMS: &[ParamSpec] = &[
    spec("input", 0.0, 24.0, 0.0, "dB"),
    spec("ceiling", -12.0, 0.0, -0.3, "dB"),
    spec("release", 10.0, 1_000.0, 80.0, "ms"),
];

const DISTORTION_PARAMS: &[ParamSpec] = &[
    spec("drive", 0.0, 48.0, 12.0, "dB"),
    spec("mix", 0.0, 1.0, 1.0, ""),
    spec("output", -24.0, 6.0, -6.0, "dB"),
    int_spec("mode", 0.0, 1.0, 0.0),
    spec("threshold", -24.0, 0.0, -6.0, "dB"),
];

impl EffectKind {
    /// Stable identifier used in commands and project files.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Filter => "filter",
            Self::Eq => "eq",
            Self::Delay => "delay",
            Self::Reverb => "reverb",
            Self::Compressor => "compressor",
            Self::Distortion => "distortion",
            Self::Chorus => "chorus",
            Self::Limiter => "limiter",
            Self::Phaser => "phaser",
            Self::Flanger => "flanger",
            Self::Bitcrusher => "bitcrusher",
            Self::Stereo => "stereo",
            Self::Gate => "gate",
            Self::Autopan => "autopan",
            Self::BassBoost => "bass_boost",
            Self::Overdrive => "overdrive",
            Self::SoftClipper => "soft_clipper",
            Self::Waveshaper => "waveshaper",
            Self::DcBlock => "dc_block",
            Self::RingMod => "ring_mod",
            Self::FreqShift => "freq_shift",
            Self::Formant => "formant",
            Self::AutoFilter => "auto_filter",
            Self::NoiseGate => "noise_gate",
            Self::Transient => "transient",
            Self::MultiTap => "multi_tap",
            Self::GraphicEq => "graphic_eq",
            Self::ParametricEq => "parametric_eq",
            Self::Multiband => "multiband",
            Self::Vocoder => "vocoder",
            Self::PitchShift => "pitch_shift",
            Self::Stutter => "stutter",
            Self::Convolution => "convolution",
            Self::PitchCorrect => "pitch_correct",
            Self::Tuner => "tuner",
            Self::Dynamics => "dynamics",
            Self::StereoShaper => "stereo_shaper",
            Self::MultibandLimiter => "multiband_limiter",
            Self::Send => "send",
            Self::HallReverb => "hall_reverb",
            Self::GuitarChain => "guitar_chain",
            Self::MultiFx => "multi_fx",
            Self::FxChain => "fx_chain",
            Self::BeatShaper => "beat_shaper",
            Self::BandSplit => "band_split",
            Self::Scratch => "scratch",
            Self::Rack => "rack",
            Self::Plugin => "plugin",
        }
    }

    /// The name people see (menus, window titles, plugin names).
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Filter => "Filter",
            Self::Eq => "Equaliser",
            Self::Delay => "Delay",
            Self::Reverb => "Reverb",
            Self::Compressor => "Compressor",
            Self::Distortion => "Distortion",
            Self::Chorus => "Chorus",
            Self::Limiter => "Limiter",
            Self::Phaser => "Phaser",
            Self::Flanger => "Flanger",
            Self::Bitcrusher => "Bitcrusher",
            Self::Stereo => "Stereo tool",
            Self::Gate => "Trance gate",
            Self::Autopan => "Auto-pan",
            Self::BassBoost => "Bass boost",
            Self::Overdrive => "Overdrive",
            Self::SoftClipper => "Soft clipper",
            Self::Waveshaper => "Waveshaper",
            Self::DcBlock => "DC remover",
            Self::RingMod => "Ring modulator",
            Self::FreqShift => "Frequency shifter",
            Self::Formant => "Vowel filter",
            Self::AutoFilter => "Auto filter",
            Self::NoiseGate => "Noise gate",
            Self::Transient => "Transient shaper",
            Self::MultiTap => "Multi-tap delay",
            Self::GraphicEq => "Graphic EQ",
            Self::ParametricEq => "Parametric EQ",
            Self::Multiband => "Multiband compressor",
            Self::Vocoder => "Vocoder",
            Self::PitchShift => "Pitch shifter",
            Self::Stutter => "Stutter",
            Self::Convolution => "Convolution reverb",
            Self::PitchCorrect => "Pitch corrector",
            Self::Tuner => "Tuner",
            Self::Dynamics => "Dynamics",
            Self::StereoShaper => "Stereo shaper",
            Self::MultibandLimiter => "Multiband limiter",
            Self::Send => "Send",
            Self::HallReverb => "Hall reverb",
            Self::GuitarChain => "Guitar chain",
            Self::MultiFx => "X/Y multi-effect",
            Self::FxChain => "Effect chain",
            Self::BeatShaper => "Beat shaper",
            Self::BandSplit => "Band splitter",
            Self::Scratch => "Scratch",
            Self::Rack => "Effect rack",
            Self::Plugin => "Plugin",
        }
    }

    /// Parses an identifier produced by [`EffectKind::name`].
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        EFFECT_KINDS.into_iter().find(|kind| kind.name() == name)
    }

    /// The parameter table for this effect.
    #[must_use]
    pub fn params(self) -> &'static [ParamSpec] {
        match self {
            Self::Filter => FILTER_PARAMS,
            Self::Eq => EQ_PARAMS,
            Self::Delay => DELAY_PARAMS,
            Self::Reverb => REVERB_PARAMS,
            Self::Compressor => COMPRESSOR_PARAMS,
            Self::Distortion => DISTORTION_PARAMS,
            Self::Chorus => CHORUS_PARAMS,
            Self::Limiter => LIMITER_PARAMS,
            Self::Phaser => PHASER_PARAMS,
            Self::Flanger => FLANGER_PARAMS,
            Self::Bitcrusher => BITCRUSHER_PARAMS,
            Self::Stereo => STEREO_PARAMS,
            Self::Gate => GATE_PARAMS,
            Self::Autopan => AUTOPAN_PARAMS,
            Self::BassBoost => BASS_BOOST_PARAMS,
            Self::Overdrive => OVERDRIVE_PARAMS,
            Self::SoftClipper => SOFT_CLIPPER_PARAMS,
            Self::Waveshaper => WAVESHAPER_PARAMS,
            Self::DcBlock => DC_BLOCK_PARAMS,
            Self::RingMod => RING_MOD_PARAMS,
            Self::FreqShift => FREQ_SHIFT_PARAMS,
            Self::Formant => FORMANT_PARAMS,
            Self::AutoFilter => AUTO_FILTER_PARAMS,
            Self::NoiseGate => NOISE_GATE_PARAMS,
            Self::Transient => TRANSIENT_PARAMS,
            Self::MultiTap => MULTI_TAP_PARAMS,
            Self::GraphicEq => GRAPHIC_EQ_PARAMS,
            Self::ParametricEq => PARAMETRIC_EQ_PARAMS,
            Self::Multiband => MULTIBAND_PARAMS,
            Self::Vocoder => VOCODER_PARAMS,
            Self::PitchShift => PITCH_SHIFT_PARAMS,
            Self::Stutter => STUTTER_PARAMS,
            Self::Convolution => CONVOLUTION_PARAMS,
            Self::PitchCorrect => PITCH_CORRECT_PARAMS,
            Self::Tuner => TUNER_PARAMS,
            Self::Dynamics => DYNAMICS_PARAMS,
            Self::StereoShaper => STEREO_SHAPER_PARAMS,
            Self::MultibandLimiter => MULTIBAND_LIMITER_PARAMS,
            Self::Send => SEND_PARAMS,
            Self::HallReverb => HALL_REVERB_PARAMS,
            Self::GuitarChain => GUITAR_CHAIN_PARAMS,
            Self::MultiFx => MULTI_FX_PARAMS,
            Self::FxChain => FX_CHAIN_PARAMS,
            Self::BeatShaper => BEAT_SHAPER_PARAMS,
            Self::BandSplit => BAND_SPLIT_PARAMS,
            Self::Scratch => SCRATCH_PARAMS,
            Self::Rack => RACK_PARAMS,
            Self::Plugin => PLUGIN_PARAMS,
        }
    }

    /// Index of the `sidechain` parameter (the insert whose output feeds the
    /// effect's detector or modulator), for effects that have one.
    #[must_use]
    pub fn sidechain_index(self) -> Option<usize> {
        self.params().iter().position(|p| p.name == "sidechain")
    }

    /// Whether this effect can run inside an effect rack: every built-in
    /// effect except the ones the engine routes, plugins and racks.
    #[must_use]
    pub fn allowed_in_rack(self) -> bool {
        !matches!(
            self,
            Self::Send | Self::BandSplit | Self::Plugin | Self::Rack
        )
    }

    /// Whether parameter `index` names an insert this effect feeds (the send's
    /// `target`, the band splitter's `low_target` …).
    #[must_use]
    pub fn is_send_target(self, index: usize) -> bool {
        self.params()
            .get(index)
            .is_some_and(|p| p.name == "target" || p.name.ends_with("_target"))
    }

    /// Index of the parameter naming a channel by its rack position (the
    /// pitch corrector's `follow`), for effects that have one.
    #[must_use]
    pub fn channel_ref_index(self) -> Option<usize> {
        self.params().iter().position(|p| p.name == "follow")
    }

    /// Indices of the parameters naming other mixer tracks: sidechain
    /// sources and send targets.
    pub fn track_ref_indices(self) -> impl Iterator<Item = usize> {
        let sidechain = self.sidechain_index();
        (0..self.params().len()).filter(move |&i| Some(i) == sidechain || self.is_send_target(i))
    }

    /// Default parameter values for this effect.
    #[must_use]
    pub fn defaults(self) -> [f32; MAX_EFFECT_PARAMS] {
        defaults(self.params())
    }
}

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_parameters_count_their_knob_steps() {
        assert_eq!(int_spec("on", 0.0, 1.0, 0.0).steps(), Some(1));
        assert_eq!(int_spec("voices", 1.0, 16.0, 1.0).steps(), Some(15));
        assert_eq!(spec("mix", 0.0, 1.0, 1.0, "%").steps(), None);
        assert_eq!(int_spec("fixed", 3.0, 3.0, 3.0).steps(), None);
    }

    #[test]
    fn tables_fit_their_arrays_and_defaults_are_in_range() {
        assert!(INSTRUMENT_PARAMS.len() <= MAX_INSTRUMENT_PARAMS);
        let mut tables = vec![INSTRUMENT_PARAMS];
        tables.extend(EFFECT_KINDS.iter().map(|kind| kind.params()));
        for table in tables {
            assert!(table.len() <= MAX_EFFECT_PARAMS.max(MAX_INSTRUMENT_PARAMS));
            for spec in table {
                assert!(
                    spec.accepts(spec.default),
                    "{} default out of range",
                    spec.name
                );
                assert!(spec.min < spec.max, "{} has an empty range", spec.name);
            }
        }
        for kind in EFFECT_KINDS {
            assert!(kind.params().len() <= MAX_EFFECT_PARAMS);
        }
    }

    #[test]
    fn instrument_indices_match_table() {
        assert_eq!(INSTRUMENT_PARAMS[inst::ATTACK].name, "attack");
        assert_eq!(INSTRUMENT_PARAMS[inst::CUTOFF].name, "cutoff");
        assert_eq!(INSTRUMENT_PARAMS[inst::ROOT_KEY].name, "root_key");
        assert_eq!(INSTRUMENT_PARAMS[inst::PITCH].name, "pitch");
        for (index, name) in [
            (inst::GLIDE, "glide"),
            (inst::DRIVE, "drive"),
            (inst::ARP_GATE, "arp_gate"),
            (inst::PLUCK_TONE, "pluck_tone"),
            (inst::SLICES, "slices"),
            (inst::WAVE3, "wave3"),
            (inst::FM_FEEDBACK, "fm_feedback"),
            (inst::OP1_RATIO, "op1_ratio"),
            (inst::OP4_SUSTAIN, "op4_sustain"),
            (inst::KEYS_HAMMER, "keys_hammer"),
            (inst::ADD_DECAY, "add_decay"),
            (inst::GRAN_SCAN, "gran_scan"),
            (inst::SWING_MIX, "swing_mix"),
            (inst::SWING_TRUNCATE, "swing_truncate"),
            (inst::CUT_GROUP, "cut_group"),
            (inst::POLY_MAX, "poly_max"),
            (inst::REVERSE, "reverse"),
            (inst::FADE_OUT, "fade_out"),
            (inst::TIME_SHIFT, "time_shift"),
            (inst::ECHO_PITCH, "echo_pitch"),
            (inst::FILTER2_TYPE, "filter2_type"),
            (inst::LFO2_SHAPE, "lfo2_shape"),
            (inst::MOD1_SOURCE, "mod1_source"),
            (inst::MOD3_AMOUNT, "mod3_amount"),
            (inst::OP5_RATIO, "op5_ratio"),
            (inst::OP6_SUSTAIN, "op6_sustain"),
            (inst::OP1_WAVE, "op1_wave"),
            (inst::FM_RESONANCE, "fm_resonance"),
            (inst::PLUCK_BOW, "pluck_bow"),
            (inst::PLUCK_DETUNE, "pluck_detune"),
            (inst::ADD_BRIGHT_B, "add_bright_b"),
            (inst::ADD_MORPH_DEPTH, "add_morph_depth"),
            (inst::MIDI_CC1, "midi_cc1"),
            (inst::MIDI_CC1_NUMBER, "midi_cc1_number"),
            (inst::MIDI_PROGRAM, "midi_program"),
            (inst::RESYN_SPEED, "resyn_speed"),
            (inst::RESYN_LOOP, "resyn_loop"),
        ] {
            assert_eq!(INSTRUMENT_PARAMS[index].name, name);
        }
    }

    #[test]
    fn correction_scales_match_their_degrees() {
        assert_eq!(CORRECTION_SCALES[1].1, 0b1010_1011_0101, "major");
        assert_eq!(CORRECTION_SCALES[5].1, 0b0100_1010_1001, "minor pentatonic");
        for (_, mask) in CORRECTION_SCALES {
            assert!(mask & 1 == 1, "every scale holds its key");
        }
    }

    #[test]
    fn effect_names_round_trip() {
        for kind in EFFECT_KINDS {
            assert_eq!(EffectKind::parse(kind.name()), Some(kind));
        }
        assert_eq!(EffectKind::parse("theremin"), None);
        assert_eq!(EffectKind::parse("plugin"), None);
    }

    #[test]
    fn unit_mappings_round_trip() {
        let cutoff = INSTRUMENT_PARAMS[inst::CUTOFF];
        for value in [20.0, 632.5, 1_000.0, 20_000.0] {
            assert!((cutoff.from_unit(cutoff.to_unit(value)) - value).abs() / value < 1e-3);
        }
        assert!((cutoff.to_unit(632.5) - 0.5).abs() < 0.01);
        assert_eq!(cutoff.format(2_500.0), "2.50 kHz");
        for gain in [0.0, 0.01, 0.5, 1.0, 1.25] {
            let back = volume_from_unit(volume_to_unit(gain, 1.25), 1.25);
            assert!((back - gain).abs() < 1e-3, "{gain} -> {back}");
        }
        assert!((volume_to_unit(1.0, 1.25) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn sanitize_clamps_and_rounds() {
        let wave = INSTRUMENT_PARAMS[inst::WAVE1];
        assert!((wave.sanitize(2.6) - 3.0).abs() < f32::EPSILON);
        assert!((wave.sanitize(99.0) - 4.0).abs() < f32::EPSILON);
        assert_eq!(index_of(INSTRUMENT_PARAMS, "release"), Some(inst::RELEASE));
    }
}
