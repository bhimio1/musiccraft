//! Plain data types that make up a project.

use std::collections::{BTreeMap, BTreeSet};

use musiccraft_meter::TimeSig;
use musiccraft_params::{
    EffectKind, INSTRUMENT_PARAMS, MAX_EFFECT_PARAMS, MAX_INSTRUMENT_PARAMS, ParamSpec,
    RACK_BRANCHES, defaults, inst,
};
use serde::{Deserialize, Serialize};

/// Stable identity of a channel; survives reordering and removal of others.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ChannelId(pub u32);

/// Stable identity of a pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PatternId(pub u32);

/// Lowest channel or mixer volume (linear gain).
pub const MIN_VOLUME: f32 = 0.0;
/// Highest channel or mixer volume (linear gain, about +1.9 dB).
pub const MAX_VOLUME: f32 = 1.25;
/// Default channel volume.
pub const DEFAULT_CHANNEL_VOLUME: f32 = 0.8;
/// Most steps a pattern may have.
pub const MAX_PATTERN_STEPS: u32 = 1024;
/// Number of playlist track rows.
pub const PLAYLIST_TRACKS: u32 = 64;
/// The range of a playlist track's own height, in percent of the global one.
pub const TRACK_HEIGHT_PERCENT: std::ops::RangeInclusive<u16> = 25..=400;
/// Inserts in a new project's mixer (plus master).
pub const DEFAULT_INSERTS: usize = 16;
/// Key that steps toggled in the channel rack play.
pub const STEP_KEY: u8 = 60;
/// Velocity of notes created without an explicit velocity.
pub const DEFAULT_VELOCITY: u8 = 100;

/// Values for a parameter table, stored as `name → value` so files stay readable
/// and survive tables gaining parameters.
fn params_to_map(specs: &[ParamSpec], values: &[f32]) -> BTreeMap<String, f32> {
    specs
        .iter()
        .zip(values)
        .map(|(spec, value)| (spec.name.to_owned(), *value))
        .collect()
}

fn params_from_map<const N: usize>(specs: &[ParamSpec], map: &BTreeMap<String, f32>) -> [f32; N] {
    let mut values: [f32; N] = defaults(specs);
    for (value, spec) in values.iter_mut().zip(specs) {
        if let Some(stored) = map.get(spec.name) {
            if stored.is_finite() {
                *value = spec.sanitize(*stored);
            }
        }
    }
    values
}

/// Instrument parameter values, indexed by `musiccraft_params::inst`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(into = "BTreeMap<String, f32>", from = "BTreeMap<String, f32>")]
pub struct InstrumentParams(pub [f32; MAX_INSTRUMENT_PARAMS]);

impl Default for InstrumentParams {
    fn default() -> Self {
        Self(defaults(INSTRUMENT_PARAMS))
    }
}

impl InstrumentParams {
    /// Defaults suited to a sampler: full sustain so samples play unshaped.
    #[must_use]
    pub fn sampler() -> Self {
        let mut params = Self::default();
        params.0[inst::DECAY] = 0.0;
        params.0[inst::SUSTAIN] = 1.0;
        params.0[inst::RELEASE] = 0.05;
        params
    }

    /// Value at an index from `musiccraft_params::inst`.
    #[must_use]
    pub fn get(&self, index: usize) -> f32 {
        self.0.get(index).copied().unwrap_or(0.0)
    }
}

impl From<InstrumentParams> for BTreeMap<String, f32> {
    fn from(params: InstrumentParams) -> Self {
        params_to_map(INSTRUMENT_PARAMS, &params.0)
    }
}

impl From<BTreeMap<String, f32>> for InstrumentParams {
    fn from(map: BTreeMap<String, f32>) -> Self {
        Self(params_from_map(INSTRUMENT_PARAMS, &map))
    }
}

/// A hosted (CLAP) plugin and everything the project keeps about it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PluginRef {
    /// CLAP plugin id, for example `com.example.synth`.
    pub id: String,
    /// Plugin name when it was added (shown if the plugin is not installed).
    pub name: String,
    /// Instance number, unique within the project. The host keeps one live
    /// plugin instance per number, so moving a slot keeps its instance.
    pub instance: u32,
    /// Parameter values by CLAP parameter id, in the plugin's plain units.
    /// Applied after `state`, so they win over it.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        with = "param_map"
    )]
    pub params: BTreeMap<u32, f64>,
    /// The plugin's saved state, base64-encoded; empty if never saved.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state: String,
}

/// Stores plugin parameter maps with string keys. Integer keys do not survive
/// serde's buffering inside internally tagged enums such as [`Instrument`].
mod param_map {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(map: &BTreeMap<u32, f64>, s: S) -> Result<S::Ok, S::Error> {
        let text: BTreeMap<String, f64> = map.iter().map(|(k, v)| (k.to_string(), *v)).collect();
        text.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<u32, f64>, D::Error> {
        let text = BTreeMap::<String, f64>::deserialize(d)?;
        text.into_iter()
            .filter(|(_, v)| v.is_finite())
            .map(|(k, v)| {
                k.parse()
                    .map(|k| (k, v))
                    .map_err(|_| serde::de::Error::custom(format!("`{k}` is not a parameter id")))
            })
            .collect()
    }
}

impl PluginRef {
    /// A reference with no stored parameters or state.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, instance: u32) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            instance,
            params: BTreeMap::new(),
            state: String::new(),
        }
    }
}

/// Where a plugin sits in a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PluginSlot {
    /// The instrument of the channel at this index (0-based).
    Channel(usize),
    /// An effect: mixer track index (0 = master) and slot index (0-based).
    Effect(usize, usize),
}

/// The sound source of a channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Instrument {
    /// Plays an audio file or a built-in sound (`builtin:<name>`).
    Sampler {
        /// File path (absolute, or relative to the project file) or built-in name.
        sample: String,
    },
    /// Two-oscillator subtractive synthesizer.
    Synth,
    /// Synthesized drums (kick, snare, hat, tom, clap).
    DrumSynth,
    /// Plucked string (Karplus-Strong).
    Pluck,
    /// Four-operator FM synth.
    Fm,
    /// Piano, electric piano or organ.
    Keys,
    /// Additive (harmonic) synth.
    Additive,
    /// A sample played as grains.
    Granular {
        /// File path or built-in name, as for the sampler.
        sample: String,
    },
    /// A sample's harmonics, measured and played back additively.
    Resynth {
        /// File path or built-in name, as for the sampler.
        sample: String,
    },
    /// Samples mapped to key and velocity ranges.
    Zones {
        /// The zones, in the order notes look for a match.
        zones: Vec<SampleZone>,
    },
    /// Sixteen sample pads.
    Kit {
        /// Pad samples, empty for an empty pad.
        pads: Vec<String>,
    },
    /// Plays nothing itself; forwards notes to other channels.
    Layer {
        /// Channels that receive the notes.
        channels: Vec<ChannelId>,
    },
    /// A hosted plugin instrument.
    Plugin {
        /// The plugin.
        plugin: PluginRef,
    },
    /// Makes no sound; sends its notes to external MIDI gear.
    MidiOut {
        /// MIDI channel, 1–16.
        midi_channel: u8,
    },
    /// Antiserum, the wavetable synth (`docs/spec/antiserum-app.md`).
    Antiserum {
        /// Its sound.
        patch: Box<musiccraft_patch::Patch>,
    },
}

impl Instrument {
    /// Short label: `sampler`, `synth` or `plugin`.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sampler { .. } => "sampler",
            Self::Synth => "synth",
            Self::DrumSynth => "drum synth",
            Self::Pluck => "pluck",
            Self::Fm => "fm",
            Self::Keys => "keys",
            Self::Additive => "additive",
            Self::Granular { .. } => "granular",
            Self::Kit { .. } => "kit",
            Self::Zones { .. } => "zones",
            Self::Resynth { .. } => "resynth",
            Self::Layer { .. } => "layer",
            Self::Plugin { .. } => "plugin",
            Self::MidiOut { .. } => "midi out",
            Self::Antiserum { .. } => "antiserum",
        }
    }

    /// The patch, for an Antiserum channel.
    #[must_use]
    pub fn patch(&self) -> Option<&musiccraft_patch::Patch> {
        match self {
            Self::Antiserum { patch } => Some(patch),
            _ => None,
        }
    }

    /// The patch, for an Antiserum channel, to change.
    pub fn patch_mut(&mut self) -> Option<&mut musiccraft_patch::Patch> {
        match self {
            Self::Antiserum { patch } => Some(patch),
            _ => None,
        }
    }

    /// The plugin, for a plugin instrument.
    #[must_use]
    pub fn plugin(&self) -> Option<&PluginRef> {
        match self {
            Self::Plugin { plugin } => Some(plugin),
            _ => None,
        }
    }
}

/// One sample of a zone sampler and the notes that play it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SampleZone {
    /// File path or built-in name, as for the sampler.
    pub sample: String,
    /// Lowest key that plays it.
    pub low: u8,
    /// Highest key that plays it.
    pub high: u8,
    /// Key at which it plays at its recorded pitch.
    pub root: u8,
    /// Lowest velocity (1–127) that plays it.
    pub vel_low: u8,
    /// Highest velocity (1–127) that plays it.
    pub vel_high: u8,
}

impl SampleZone {
    /// A zone over the whole keyboard and every velocity.
    #[must_use]
    pub fn new(sample: impl Into<String>) -> Self {
        Self {
            sample: sample.into(),
            low: 0,
            high: 127,
            root: 60,
            vel_low: 1,
            vel_high: 127,
        }
    }
}

/// A row in the channel rack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    /// Stable identity.
    pub id: ChannelId,
    /// Display name.
    pub name: String,
    /// Sound source.
    pub instrument: Instrument,
    /// Instrument parameters.
    #[serde(default)]
    pub params: InstrumentParams,
    /// Linear gain.
    pub volume: f32,
    /// Balance, -1..=1.
    pub pan: f32,
    /// Whether the channel is silenced.
    #[serde(default)]
    pub mute: bool,
    /// Whether the channel is soloed: while any channel is, only soloed
    /// channels play (see [`Project::channel_plays`]).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub solo: bool,
    /// Whether the channel is kept out of solo: another channel's solo never
    /// silences it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub solo_lock: bool,
    /// Mixer track receiving this channel; 0 is master.
    #[serde(default)]
    pub mixer_track: usize,
    /// Display filter group ("" = none): the rack can show one group at a time.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
    /// Colour index 0–15 (shown as 1–16) set by the user; `None` follows the
    /// channel's id (see [`Channel::color_index`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
}

impl Channel {
    /// The channel's palette index: the colour the user picked, or one taken
    /// from its id, which reordering does not change.
    #[must_use]
    pub fn color_index(&self) -> usize {
        self.color
            .map_or(self.id.0.saturating_sub(1) as usize, usize::from)
    }

    /// A channel with default settings.
    #[must_use]
    pub fn new(id: ChannelId, name: impl Into<String>, instrument: Instrument) -> Self {
        let params = match instrument {
            Instrument::Sampler { .. } => InstrumentParams::sampler(),
            Instrument::Kit { .. } | Instrument::Zones { .. } => InstrumentParams::sampler(),
            _ => InstrumentParams::default(),
        };
        Self {
            id,
            name: name.into(),
            instrument,
            params,
            volume: DEFAULT_CHANNEL_VOLUME,
            pan: 0.0,
            mute: false,
            solo: false,
            solo_lock: false,
            mixer_track: 0,
            group: String::new(),
            color: None,
        }
    }
}

/// A note in a pattern. Times are in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// Channel that plays the note.
    pub channel: ChannelId,
    /// Start, in ticks from the pattern start.
    pub start: u32,
    /// Duration in ticks (at least 1).
    pub length: u32,
    /// MIDI key, 0..=127.
    pub key: u8,
    /// Velocity, 1..=127.
    pub velocity: u8,
    /// Pan, -100 (left) to 100 (right).
    #[serde(default)]
    pub pan: i8,
    /// A slide note glides the sounding note of its channel to its key
    /// instead of starting a new one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub slide: bool,
    /// Further per-note properties.
    #[serde(flatten)]
    pub props: NoteProps,
}

fn is_mid(v: &u8) -> bool {
    *v == 64
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

fn is_zero_i8(v: &i8) -> bool {
    *v == 0
}

fn mid() -> u8 {
    64
}

/// Per-note properties beyond time, key, velocity, pan and slide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteProps {
    /// Release velocity, 0–127 (64 neutral).
    #[serde(default = "mid", skip_serializing_if = "is_mid")]
    pub release: u8,
    /// Fine pitch in cents, −100–100.
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub fine: i8,
    /// Modulation X, 0–127 (64 neutral): filter cutoff on the synth.
    #[serde(default = "mid", skip_serializing_if = "is_mid")]
    pub mod_x: u8,
    /// Modulation Y, 0–127 (64 neutral): filter resonance on the synth.
    #[serde(default = "mid", skip_serializing_if = "is_mid")]
    pub mod_y: u8,
    /// Colour group, 0–15 (shown as 1–16).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub color: u8,
    /// Glide from the channel's previous note, retriggering.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub porta: bool,
    /// Kept in the pattern but not played.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,
}

impl Default for NoteProps {
    fn default() -> Self {
        Self {
            release: 64,
            fine: 0,
            mod_x: 64,
            mod_y: 64,
            color: 0,
            porta: false,
            muted: false,
        }
    }
}

/// A pattern: notes for any number of channels.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    /// Stable identity.
    pub id: PatternId,
    /// Display name.
    pub name: String,
    /// Minimum length in steps; notes past the end extend the pattern by bars.
    pub length_steps: u32,
    /// The length follows the notes instead: whole bars up to the last bar
    /// with a note in it, at least one bar. `length_steps` is kept for when
    /// this is turned off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub auto_length: bool,
    /// Notes, kept sorted by (channel, start, key).
    #[serde(default)]
    pub notes: Vec<Note>,
    /// Per-pattern swing from older files (0..=1); loading moves it into the
    /// project's swing. Never saved.
    #[serde(default, rename = "swing", skip_serializing)]
    pub(crate) legacy_swing: f32,
    /// Automation inside the pattern: lanes that play wherever it plays.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub automation: Vec<PatternLane>,
    /// The pattern's time signature.
    #[serde(default, skip_serializing_if = "TimeSig::is_common")]
    pub time_signature: TimeSig,
    /// Colour index 0–15 (shown as 1–16) set by the user; none by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
}

/// One parameter's automation inside a pattern; ticks from the pattern start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PatternLane {
    /// The parameter driven.
    pub target: AutomationTarget,
    /// Points sorted by tick, one per tick.
    pub points: Vec<AutomationPoint>,
}

impl Pattern {
    /// An empty one-bar 4/4 pattern.
    #[must_use]
    pub fn new(id: PatternId, name: impl Into<String>) -> Self {
        Self::with_signature(id, name, TimeSig::COMMON)
    }

    /// An empty pattern one bar of `sig` long.
    #[must_use]
    pub fn with_signature(id: PatternId, name: impl Into<String>, sig: TimeSig) -> Self {
        Self {
            id,
            name: name.into(),
            length_steps: sig.bar_steps(),
            auto_length: false,
            notes: Vec::new(),
            legacy_swing: 0.0,
            automation: Vec::new(),
            time_signature: sig,
            color: None,
        }
    }

    /// The pattern's bar and beat grid.
    #[must_use]
    pub fn meter(&self) -> musiccraft_meter::Meter {
        musiccraft_meter::Meter::new(self.time_signature)
    }

    /// Changes the signature. A length of whole bars keeps its number of bars.
    pub fn set_time_signature(&mut self, sig: TimeSig) {
        let old = self.time_signature.bar_steps().max(1);
        if self.length_steps % old == 0 {
            let bars = (self.length_steps / old).max(1);
            self.length_steps = (bars * sig.bar_steps()).min(MAX_PATTERN_STEPS);
        }
        self.time_signature = sig;
    }

    /// Notes of one channel.
    pub fn notes_of(&self, channel: ChannelId) -> impl Iterator<Item = &Note> {
        self.notes.iter().filter(move |n| n.channel == channel)
    }

    /// Index of the note of `channel` at `start` with `key`.
    #[must_use]
    pub fn find_note(&self, channel: ChannelId, start: u32, key: u8) -> Option<usize> {
        self.notes
            .iter()
            .position(|n| n.channel == channel && n.start == start && n.key == key)
    }

    /// Inserts a note, replacing one at the same channel, start and key. Returns
    /// the replaced note.
    pub fn put_note(&mut self, note: Note) -> Option<Note> {
        let replaced = self
            .find_note(note.channel, note.start, note.key)
            .map(|i| self.notes.remove(i));
        self.notes.push(note);
        self.sort_notes();
        replaced
    }

    /// Keeps notes in canonical order.
    pub fn sort_notes(&mut self) {
        self.notes.sort_by_key(|n| (n.channel, n.start, n.key));
    }

    /// Length in ticks: the step length, extended to whole bars to fit every note.
    #[must_use]
    pub fn length_ticks(&self) -> u32 {
        let minimum = if self.auto_length {
            1
        } else {
            self.length_steps.max(1)
        };
        let steps = minimum * musiccraft_params::TICKS_PER_STEP;
        let last = self
            .notes
            .iter()
            .map(|n| n.start.saturating_add(1))
            .max()
            .unwrap_or(0);
        let bar = self.time_signature.bar_ticks().max(1);
        if last > steps || self.auto_length {
            last.div_ceil(bar).max(1) * bar
        } else {
            steps
        }
    }

    /// The length in whole steps that step tools (fill, shift) work over.
    #[must_use]
    pub fn step_span(&self) -> u32 {
        if self.auto_length {
            self.length_ticks() / musiccraft_params::TICKS_PER_STEP
        } else {
            self.length_steps
        }
    }
}

/// One effect slot on a mixer track.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(into = "EffectRepr", from = "EffectRepr")]
pub struct EffectSlot {
    /// Which effect; [`EffectKind::Plugin`] when `plugin` is set.
    pub kind: EffectKind,
    /// Whether the effect processes audio (false = bypassed).
    pub enabled: bool,
    /// Parameter values in the order of `kind.params()`.
    pub values: [f32; MAX_EFFECT_PARAMS],
    /// The hosted plugin of a plugin slot.
    pub plugin: Option<Box<PluginRef>>,
    /// A file the effect loads: the convolution reverb's impulse response
    /// (an audio file or `ir:<name>`). `None` uses the effect's default.
    pub file: Option<String>,
    /// The parallel branches of an effect rack, each a chain of effects;
    /// empty for every other effect.
    pub branches: Vec<Vec<EffectSlot>>,
    /// Dry/wet mix, 0..=1: the output is `dry + (wet - dry) * mix`.
    pub mix: f32,
}

impl EffectSlot {
    /// A new enabled effect with default settings.
    #[must_use]
    pub fn new(kind: EffectKind) -> Self {
        Self {
            kind,
            enabled: true,
            values: kind.defaults(),
            plugin: None,
            file: None,
            mix: 1.0,
            branches: if kind == EffectKind::Rack {
                vec![Vec::new(); RACK_BRANCHES]
            } else {
                Vec::new()
            },
        }
    }

    /// A new enabled plugin effect.
    #[must_use]
    pub fn plugin(plugin: PluginRef) -> Self {
        Self {
            plugin: Some(Box::new(plugin)),
            ..Self::new(EffectKind::Plugin)
        }
    }

    /// Display name: the plugin's name or the effect kind.
    #[must_use]
    pub fn label(&self) -> String {
        match &self.plugin {
            Some(plugin) => plugin.name.clone(),
            None => self.kind.to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct EffectRepr {
    kind: EffectKind,
    enabled: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    params: BTreeMap<String, f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    plugin: Option<PluginRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    branches: Vec<Vec<EffectSlot>>,
    #[serde(default = "full_mix", skip_serializing_if = "is_full_mix")]
    mix: f32,
}

fn full_mix() -> f32 {
    1.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_full_mix(mix: &f32) -> bool {
    *mix >= 1.0
}

impl From<EffectSlot> for EffectRepr {
    fn from(slot: EffectSlot) -> Self {
        Self {
            kind: slot.kind,
            enabled: slot.enabled,
            params: params_to_map(slot.kind.params(), &slot.values),
            plugin: slot.plugin.map(|p| *p),
            file: slot.file,
            branches: slot.branches,
            mix: slot.mix,
        }
    }
}

impl From<EffectRepr> for EffectSlot {
    fn from(repr: EffectRepr) -> Self {
        // A plugin slot without its plugin cannot play; keep it as a bypassed
        // passthrough rather than failing the whole file.
        let enabled = repr.enabled && (repr.kind != EffectKind::Plugin || repr.plugin.is_some());
        Self {
            kind: repr.kind,
            enabled,
            values: params_from_map(repr.kind.params(), &repr.params),
            plugin: repr.plugin.map(Box::new),
            file: repr.file,
            mix: if repr.mix.is_finite() {
                repr.mix.clamp(0.0, 1.0)
            } else {
                1.0
            },
            // A rack always has its four branches; nothing else has any.
            branches: if repr.kind == EffectKind::Rack {
                let mut branches = repr.branches;
                branches.resize(RACK_BRANCHES, Vec::new());
                branches.truncate(RACK_BRANCHES);
                branches
            } else {
                Vec::new()
            },
        }
    }
}

/// A post-fader send to a higher-numbered insert.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MixerSend {
    /// Receiving mixer track (greater than the sending track).
    pub target: usize,
    /// Linear level, 0 to [`MAX_VOLUME`].
    pub level: f32,
}

/// Most sends a mixer track can have.
pub const MAX_SENDS: usize = 8;

fn yes() -> bool {
    true
}

/// A mixer track. Track 0 is the master; the others are inserts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixerTrack {
    /// Display name.
    pub name: String,
    /// Linear gain.
    pub volume: f32,
    /// Balance, -1..=1.
    pub pan: f32,
    /// Whether the track is silenced.
    #[serde(default)]
    pub mute: bool,
    /// Whether the track is soloed.
    #[serde(default)]
    pub solo: bool,
    /// Insert effects, processed in order.
    #[serde(default)]
    pub effects: Vec<EffectSlot>,
    /// Whether this insert feeds the master (always true for the master).
    #[serde(default = "yes")]
    pub to_master: bool,
    /// Sends to higher-numbered inserts, sorted by target.
    #[serde(default)]
    pub sends: Vec<MixerSend>,
    /// Tracks this insert keys as a sidechain, sorted: its post-fader output
    /// reaches their effects' sidechain inputs but is not heard there.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sidechain_to: Vec<usize>,
    /// Stereo separation, -1 (mono) .. 0 (as is) .. 1 (side doubled).
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub separation: f32,
    /// Flip the polarity of both sides.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub phase_invert: bool,
    /// Exchange the left and right sides.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub swap_sides: bool,
    /// The built-in three-band EQ.
    #[serde(default, skip_serializing_if = "TrackEq::is_flat")]
    pub eq: TrackEq,
    /// Colour index 0–15 (shown as 1–16) set by the user; `None` uses the
    /// track's automatic colour.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<u8>,
    /// Where the mixer shows the strip: fixed at the left or right, or in
    /// the scrolling middle (the default).
    #[serde(default, skip_serializing_if = "Dock::is_middle")]
    pub dock: Dock,
}

/// Where a mixer strip sits in the mixer window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dock {
    /// The fixed dock at the left, with the master.
    Left,
    /// The scrolling middle.
    #[default]
    Middle,
    /// The fixed dock at the right.
    Right,
}

impl Dock {
    /// Every dock, left to right.
    pub const ALL: [Self; 3] = [Self::Left, Self::Middle, Self::Right];

    /// The command argument.
    #[must_use]
    pub fn arg(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Middle => "middle",
            Self::Right => "right",
        }
    }

    /// Reads a command argument.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.arg() == text)
    }

    fn is_middle(&self) -> bool {
        *self == Self::Middle
    }
}

impl MixerTrack {
    /// A track at unity gain with no effects.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            volume: 1.0,
            pan: 0.0,
            mute: false,
            solo: false,
            effects: Vec::new(),
            to_master: true,
            sends: Vec::new(),
            sidechain_to: Vec::new(),
            separation: 0.0,
            phase_invert: false,
            swap_sides: false,
            eq: TrackEq::default(),
            color: None,
            dock: Dock::Middle,
        }
    }
}

/// A mixer track's built-in EQ: low shelf, peak (Q 0.7) and high shelf.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrackEq {
    /// Gain in dB (-18..=18) and frequency in Hz of the low, mid and high
    /// bands.
    pub bands: [(f32, f32); 3],
}

impl TrackEq {
    /// Band names, in order.
    pub const BANDS: [&'static str; 3] = ["low", "mid", "high"];
    /// Frequency ranges of the bands, in Hz.
    pub const RANGES: [(f32, f32); 3] = [(20.0, 1_000.0), (100.0, 10_000.0), (1_000.0, 20_000.0)];
    /// Gain limit in dB.
    pub const MAX_GAIN: f32 = 18.0;

    /// Whether every band is at 0 dB with its default frequency.
    #[must_use]
    pub fn is_flat(&self) -> bool {
        *self == Self::default()
    }

    /// Parameters in the order of the engine's three-band EQ:
    /// `[low gain, low freq, mid gain, mid freq, mid q, high gain, high freq]`.
    #[must_use]
    pub fn params(&self) -> [f32; 7] {
        let [(lg, lf), (mg, mf), (hg, hf)] = self.bands;
        [lg, lf, mg, mf, 0.7, hg, hf]
    }
}

impl Default for TrackEq {
    fn default() -> Self {
        Self {
            bands: [(0.0, 120.0), (0.0, 1_000.0), (0.0, 8_000.0)],
        }
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_f32(value: &f32) -> bool {
    *value == 0.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one_f32(value: &f32) -> bool {
    *value == 1.0
}

fn one_f32() -> f32 {
    1.0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_true(value: &bool) -> bool {
    *value
}

/// Stable identity of an automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AutomationId(pub u32);

/// What a playlist clip plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClipSource {
    /// A pattern, repeated for the clip's length.
    Pattern(PatternId),
    /// An automation curve.
    Automation(AutomationId),
    /// A sampler channel's sample, played once from the clip's start.
    Audio(ChannelId),
}

/// Something placed on the playlist. Times are in ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clip {
    /// What the clip plays.
    #[serde(flatten)]
    pub source: ClipSource,
    /// Track row, 0-based.
    pub track: u32,
    /// Start in ticks from the song start.
    pub start: u32,
    /// Duration in ticks; a pattern repeats if the clip is longer.
    pub length: u32,
    /// Ticks into the source where the clip begins (a sliced clip).
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub offset: u32,
    /// A muted clip stays on the playlist but does not play.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,
}

impl Clip {
    /// The pattern a pattern clip plays.
    #[must_use]
    pub fn pattern(&self) -> Option<PatternId> {
        match self.source {
            ClipSource::Pattern(id) => Some(id),
            _ => None,
        }
    }

    /// The automation an automation clip plays.
    #[must_use]
    pub fn automation(&self) -> Option<AutomationId> {
        match self.source {
            ClipSource::Automation(id) => Some(id),
            _ => None,
        }
    }

    /// The channel an audio clip plays.
    #[must_use]
    pub fn audio(&self) -> Option<ChannelId> {
        match self.source {
            ClipSource::Audio(id) => Some(id),
            _ => None,
        }
    }
}

/// A parameter an automation drives.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AutomationTarget {
    /// A channel's volume.
    ChannelVolume {
        /// The channel.
        channel: ChannelId,
    },
    /// A channel's pan.
    ChannelPan {
        /// The channel.
        channel: ChannelId,
    },
    /// An instrument parameter of a channel.
    ChannelParam {
        /// The channel.
        channel: ChannelId,
        /// Parameter name from the instrument table.
        param: String,
    },
    /// A mixer track's volume.
    MixerVolume {
        /// Track index (0 = master).
        track: usize,
    },
    /// A mixer track's pan.
    MixerPan {
        /// Track index (0 = master).
        track: usize,
    },
    /// A parameter of an effect slot.
    EffectParam {
        /// Track index (0 = master).
        track: usize,
        /// Slot index (0-based).
        slot: usize,
        /// Parameter name from the effect's table.
        param: String,
    },
}

/// One point of an automation curve.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AutomationPoint {
    /// Ticks from the start of the clip.
    pub tick: u32,
    /// Control position 0..=1, mapped through the target's curve.
    pub value: f32,
    /// Shape of the segment from this point to the next.
    #[serde(default, skip_serializing_if = "Curve::is_single")]
    pub curve: Curve,
    /// Bend of that segment, -1..=1.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub tension: f32,
}

impl AutomationPoint {
    /// A point starting a straight segment.
    #[must_use]
    pub fn new(tick: u32, value: f32) -> Self {
        Self {
            tick,
            value,
            curve: Curve::Single,
            tension: 0.0,
        }
    }

    /// Whether the segment from here is a plain straight line.
    #[must_use]
    pub fn is_straight(&self) -> bool {
        self.curve == Curve::Single && self.tension == 0.0
    }
}

/// Shape of an automation segment (see `docs/spec/playlist.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Curve {
    /// One curve bent by the tension (straight at 0).
    #[default]
    Single,
    /// An S-curve.
    Double,
    /// The value holds until the next point.
    Hold,
    /// Equal steps; the tension sets how many.
    Stairs,
}

impl Curve {
    /// Every shape, in menu order.
    pub const ALL: [Self; 4] = [Self::Single, Self::Double, Self::Hold, Self::Stairs];

    /// The name used by commands and files.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Double => "double",
            Self::Hold => "hold",
            Self::Stairs => "stairs",
        }
    }

    /// Parses [`Curve::name`].
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    /// Whether this is the default shape.
    #[must_use]
    pub fn is_single(&self) -> bool {
        *self == Self::Single
    }

    /// How far (0..=1) the segment has moved at `t` (0..=1) of its length.
    #[must_use]
    pub fn shape(self, t: f32, tension: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        let tension = if tension.is_finite() {
            tension.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let bend = |t: f32| t.powf(4.0_f32.powf(tension));
        match self {
            Self::Single => bend(t),
            Self::Double => {
                if t < 0.5 {
                    bend(2.0 * t) / 2.0
                } else {
                    1.0 - bend(2.0 - 2.0 * t) / 2.0
                }
            }
            Self::Hold => {
                if t >= 1.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Stairs => {
                let steps = 2.0 + ((tension + 1.0) / 2.0 * 14.0).round();
                if t >= 1.0 {
                    1.0
                } else {
                    (t * steps).floor() / steps
                }
            }
        }
    }
}

/// A curve that moves one parameter over time when its clips play.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Automation {
    /// Stable identity.
    pub id: AutomationId,
    /// Display name.
    pub name: String,
    /// The parameter driven.
    pub target: AutomationTarget,
    /// Points sorted by tick; values are linearly interpolated between them.
    pub points: Vec<AutomationPoint>,
    /// Whether its clips play.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// The control position the curve's bottom plays as.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub low: f32,
    /// The control position the curve's top plays as.
    #[serde(default = "one_f32", skip_serializing_if = "is_one_f32")]
    pub high: f32,
}

impl Automation {
    /// An automation that is on and plays the curve over the whole range.
    #[must_use]
    pub fn new(
        id: AutomationId,
        name: String,
        target: AutomationTarget,
        points: Vec<AutomationPoint>,
    ) -> Self {
        Self {
            id,
            name,
            target,
            points,
            enabled: true,
            low: 0.0,
            high: 1.0,
        }
    }

    /// The control position played for curve value `value` (0..=1), mapped
    /// into the range.
    #[must_use]
    pub fn output(&self, value: f32) -> f32 {
        (self.low + (self.high - self.low) * value).clamp(0.0, 1.0)
    }

    /// The curve value that plays as control position `position`: the
    /// inverse of [`Automation::output`] (the middle when the range is empty).
    #[must_use]
    pub fn curve_value(&self, position: f32) -> f32 {
        let span = self.high - self.low;
        if span.abs() < 1e-6 {
            return 0.5;
        }
        ((position - self.low) / span).clamp(0.0, 1.0)
    }

    /// Length covered by the points (at least one tick); callers round it to
    /// bars of the meter where it is placed.
    #[must_use]
    pub fn length_ticks(&self) -> u32 {
        self.points.iter().map(|p| p.tick).max().unwrap_or(0).max(1)
    }

    /// Value at `tick`, interpolating between points and holding the ends.
    #[must_use]
    pub fn value_at(&self, tick: f64) -> Option<f32> {
        let first = self.points.first()?;
        if tick <= f64::from(first.tick) {
            return Some(first.value);
        }
        for pair in self.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if tick <= f64::from(b.tick) {
                let span = f64::from(b.tick.saturating_sub(a.tick)).max(1e-9);
                let t = ((tick - f64::from(a.tick)) / span) as f32;
                return Some(a.value + (b.value - a.value) * a.curve.shape(t, a.tension));
            }
        }
        self.points.last().map(|p| p.value)
    }

    /// Keeps points sorted with one point per tick.
    pub fn sort_points(&mut self) {
        self.points.sort_by_key(|p| p.tick);
        self.points.dedup_by_key(|p| p.tick);
    }
}

impl Clip {
    /// End tick (exclusive).
    #[must_use]
    pub fn end(&self) -> u32 {
        self.start.saturating_add(self.length)
    }
}

/// What a marker does when song playback reaches it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerAction {
    /// Only a label.
    #[default]
    None,
    /// Playback pauses here; the playhead stays on the marker.
    Pause,
    /// At the end of the song, playback returns here instead of to the start.
    SongLoop,
    /// The song's time signature changes here; a new bar starts.
    TimeSignature(TimeSig),
}

impl MarkerAction {
    /// The actions `playlist.set_marker_action` sets, in menu order
    /// (signature changes have their own command).
    pub const ALL: [Self; 3] = [Self::None, Self::Pause, Self::SongLoop];

    /// The name used by commands and the project file.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pause => "pause",
            Self::SongLoop => "song_loop",
            Self::TimeSignature(_) => "time_signature",
        }
    }

    /// Parses a name from [`MarkerAction::name`].
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == name)
    }

    /// Whether this is a plain label.
    #[must_use]
    pub fn is_none(&self) -> bool {
        *self == Self::None
    }
}

/// A named position on the song timeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    /// Position in ticks.
    pub tick: u32,
    /// Label.
    pub name: String,
    /// What the marker does during song playback.
    #[serde(default, skip_serializing_if = "MarkerAction::is_none")]
    pub action: MarkerAction,
}

/// The song arrangement.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Playlist {
    /// Placed clips, sorted by (track, start).
    #[serde(default)]
    pub clips: Vec<Clip>,
    /// Track rows that are muted (0-based).
    #[serde(default)]
    pub muted_tracks: BTreeSet<u32>,
    /// Rows armed for audio recording (0-based): takes go to the first one
    /// with room.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub armed_tracks: BTreeSet<u32>,
    /// Soloed track rows (0-based): while any is soloed, only they play.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub soloed_tracks: BTreeSet<u32>,
    /// Track names by row (0-based); unnamed rows show "Track n".
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub track_names: BTreeMap<u32, String>,
    /// Track colours by row (0-based): palette index 0..=15.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub track_colors: BTreeMap<u32, u8>,
    /// Track heights by row (0-based), in percent of the global track height
    /// (`TRACK_HEIGHT_PERCENT`); rows not listed are 100%.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub track_heights: BTreeMap<u32, u16>,
    /// Song-mode loop region `(start, end)` in ticks, if set.
    #[serde(default)]
    pub loop_range: Option<(u32, u32)>,
    /// Markers sorted by tick.
    #[serde(default)]
    pub markers: Vec<Marker>,
}

impl Playlist {
    /// `track`'s own height, in percent of the global track height.
    #[must_use]
    pub fn track_height(&self, track: u32) -> u16 {
        self.track_heights.get(&track).copied().unwrap_or(100)
    }

    /// Whether clips on `track` play: not muted, and soloed when any row is.
    #[must_use]
    pub fn track_audible(&self, track: u32) -> bool {
        !self.muted_tracks.contains(&track)
            && (self.soloed_tracks.is_empty() || self.soloed_tracks.contains(&track))
    }

    /// The row's name ("Track n" when it has none).
    #[must_use]
    pub fn track_name(&self, track: u32) -> String {
        self.track_names
            .get(&track)
            .cloned()
            .unwrap_or_else(|| format!("Track {}", track + 1))
    }

    /// Where song playback returns at the end: the first song-loop marker.
    #[must_use]
    pub fn song_loop_tick(&self) -> Option<u32> {
        self.markers
            .iter()
            .find(|m| m.action == MarkerAction::SongLoop)
            .map(|m| m.tick)
    }

    /// Signature changes from markers, as `(tick, sig)` in order.
    pub fn signature_changes(&self) -> impl Iterator<Item = (u32, TimeSig)> + '_ {
        self.markers.iter().filter_map(|m| match m.action {
            MarkerAction::TimeSignature(sig) => Some((m.tick, sig)),
            _ => None,
        })
    }

    /// Positions of pause markers, in order.
    #[must_use]
    pub fn pause_ticks(&self) -> Vec<u32> {
        self.markers
            .iter()
            .filter(|m| m.action == MarkerAction::Pause)
            .map(|m| m.tick)
            .collect()
    }

    /// Keeps clips in canonical order.
    pub fn sort_clips(&mut self) {
        self.clips.sort_by_key(|c| (c.track, c.start));
    }

    /// Index of the clip on `track` that covers tick `at`.
    #[must_use]
    pub fn clip_at(&self, track: u32, at: u32) -> Option<usize> {
        self.clips
            .iter()
            .position(|c| c.track == track && c.start <= at && at < c.end())
    }

    /// End of the last clip in ticks.
    #[must_use]
    pub fn length_ticks(&self) -> u32 {
        self.clips.iter().map(Clip::end).max().unwrap_or(0)
    }
}

/// What the transport plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayMode {
    /// Loop the selected pattern.
    #[default]
    Pattern,
    /// Play the playlist arrangement.
    Song,
}

impl PlayMode {
    /// `pattern` or `song`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Pattern => "pattern",
            Self::Song => "song",
        }
    }
}

/// How channel and mixer pans split a signal between left and right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanLaw {
    /// Equal power: -3 dB on each side at centre.
    #[default]
    Circular,
    /// Equal amplitude: -6 dB on each side at centre.
    Triangular,
}

impl PanLaw {
    /// Every law, in menu order.
    pub const ALL: [Self; 2] = [Self::Circular, Self::Triangular];

    /// `circular` or `triangular`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Circular => "circular",
            Self::Triangular => "triangular",
        }
    }

    /// The law named `label`.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|law| law.label() == label)
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}
