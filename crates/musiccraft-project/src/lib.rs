//! Project data model: the single source of truth for what a MusicCraft project
//! contains. Nothing here knows about audio devices or user interfaces.
//!
//! Users and commands number channels, patterns and playlist tracks from 1. Mixer
//! track 0 is the master and inserts are numbered from 1. Positions in time are
//! 0-based tick offsets (see `musiccraft_params::PPQ`).

pub use musiccraft_patch::base64;
mod controller;
mod error;
mod model;
pub mod music;

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use controller::{Controller, ControllerKind};
pub use error::ProjectError;
pub use model::{
    Automation, AutomationId, AutomationPoint, AutomationTarget, Channel, ChannelId, Clip,
    ClipSource, Curve, DEFAULT_CHANNEL_VOLUME, DEFAULT_INSERTS, DEFAULT_VELOCITY, Dock, EffectSlot,
    Instrument, InstrumentParams, MAX_PATTERN_STEPS, MAX_SENDS, MAX_VOLUME, MIN_VOLUME, Marker,
    MarkerAction, MixerSend, MixerTrack, Note, NoteProps, PLAYLIST_TRACKS, PanLaw, Pattern,
    PatternId, PatternLane, PlayMode, Playlist, PluginRef, PluginSlot, STEP_KEY, SampleZone,
    TRACK_HEIGHT_PERCENT, TrackEq,
};
pub use musiccraft_meter as meter;
pub use musiccraft_meter::{Meter, TimeSig};
pub use musiccraft_params as params;

/// Lowest tempo a project accepts, in beats per minute.
pub const MIN_TEMPO_BPM: f64 = 10.0;
/// Highest tempo a project accepts, in beats per minute.
pub const MAX_TEMPO_BPM: f64 = 999.0;
/// Tempo of a newly created project, in beats per minute.
pub const DEFAULT_TEMPO_BPM: f64 = 130.0;
/// Identifier written at the top of every project file.
pub const FORMAT_NAME: &str = "musiccraft-project";
/// Current project file version.
pub const FORMAT_VERSION: u32 = 1;
/// Conventional project file extension.
pub const FILE_EXTENSION: &str = "mcproj";

/// Whether the project is currently playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Transport {
    /// Playback is halted.
    #[default]
    Stopped,
    /// Playback is running.
    Playing,
    /// Playback is halted with the playhead kept where it was.
    Paused,
}

impl Transport {
    /// Returns `true` while playback is running.
    #[must_use]
    pub fn is_playing(self) -> bool {
        self == Self::Playing
    }

    /// Returns `true` while paused (halted, playhead kept).
    #[must_use]
    pub fn is_paused(self) -> bool {
        self == Self::Paused
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_cents(value: &f32) -> bool {
    *value == 0.0
}

/// How recorded notes treat what the pattern already has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordMode {
    /// Add to the existing notes.
    #[default]
    Blend,
    /// Remove the existing notes the playhead passes while recording.
    Replace,
}

impl RecordMode {
    /// `blend` or `replace`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Blend => "blend",
            Self::Replace => "replace",
        }
    }
}

/// Recording options, saved with the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordOptions {
    /// Click one bar before playback starts while armed.
    pub count_in: bool,
    /// Start playback with the first live note while armed.
    pub wait_for_input: bool,
    /// Blend or replace.
    pub mode: RecordMode,
    /// Keep looping while armed; when off, playback stops at the loop end.
    pub loop_record: bool,
}

impl Default for RecordOptions {
    fn default() -> Self {
        Self {
            count_in: false,
            wait_for_input: false,
            mode: RecordMode::Blend,
            loop_record: true,
        }
    }
}

/// Counts of one-shot transport requests. The bridge acts when a count rises;
/// they are runtime-only and ignored by project equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Requests {
    /// `transport.jump_to_start` runs.
    pub jumps_to_start: u32,
    /// `transport.panic` runs.
    pub panics: u32,
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Stopped => "stopped",
            Self::Playing => "playing",
            Self::Paused => "paused",
        })
    }
}

/// Checks a linear volume.
///
/// # Errors
///
/// Returns [`ProjectError::OutOfRange`] outside `MIN_VOLUME..=MAX_VOLUME`.
pub fn check_volume(value: f32) -> Result<f32, ProjectError> {
    check_range("volume", value, MIN_VOLUME, MAX_VOLUME)
}

/// Checks a balance pan value.
///
/// # Errors
///
/// Returns [`ProjectError::OutOfRange`] outside `-1..=1`.
pub fn check_pan(value: f32) -> Result<f32, ProjectError> {
    check_range("pan", value, -1.0, 1.0)
}

/// Checks a finite value against an inclusive range.
///
/// # Errors
///
/// Returns [`ProjectError::OutOfRange`] if the value is not finite or out of range.
pub fn check_range(what: &str, value: f32, min: f32, max: f32) -> Result<f32, ProjectError> {
    if value.is_finite() && value >= min && value <= max {
        Ok(value)
    } else {
        Err(ProjectError::out_of_range(
            what,
            f64::from(value),
            f64::from(min),
            f64::from(max),
        ))
    }
}

/// A MusicCraft project.
///
/// Equality compares content: the identifier allocator and the file location are
/// ignored, so a project equals itself after an edit and its undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    name: String,
    tempo_bpm: f64,
    #[serde(skip)]
    transport: Transport,
    /// One-shot transport requests (jump to start, panic); never saved.
    #[serde(skip)]
    requests: Requests,
    /// Whether recording is armed; never saved.
    #[serde(skip)]
    armed: bool,
    /// Whether step edit is on; never saved.
    #[serde(skip)]
    step_edit: bool,
    /// Count-in, wait for input, blend/replace and loop recording.
    #[serde(default)]
    pub record: RecordOptions,
    /// Whether the transport loops the current pattern or plays the song.
    #[serde(default)]
    pub play_mode: PlayMode,
    /// Channel rack rows, in display order.
    #[serde(default)]
    pub channels: Vec<Channel>,
    /// Patterns, in display order. Never empty.
    pub patterns: Vec<Pattern>,
    /// Index into `patterns` of the selected pattern.
    #[serde(default)]
    pub current_pattern: usize,
    /// Mixer tracks; index 0 is the master.
    pub mixer: Vec<MixerTrack>,
    /// The song arrangement.
    #[serde(default)]
    pub playlist: Playlist,
    /// Automation curves, placed on the playlist by automation clips.
    #[serde(default)]
    pub automations: Vec<Automation>,
    /// Parameters moved continuously by a live source.
    #[serde(default)]
    pub controllers: Vec<Controller>,
    /// Whether the metronome clicks during playback (never rendered to files).
    #[serde(default)]
    pub metronome: bool,
    /// How channel and mixer pans split left and right.
    #[serde(default)]
    pub pan_law: PanLaw,
    /// Master pitch in cents (-1200..=1200): every built-in instrument is
    /// transposed by it (A4 = 440 Hz × 2^(cents/1200)).
    #[serde(default, skip_serializing_if = "is_zero_cents")]
    pub master_pitch: f32,
    /// Swing, 0..=1: notes on off-beat sixteenths are delayed by up to half a
    /// step, scaled by each channel's `swing_mix`.
    #[serde(default)]
    pub swing: f32,
    /// The song's time signature from its start; new patterns take it.
    #[serde(default, skip_serializing_if = "TimeSig::is_common")]
    pub time_signature: TimeSig,
    /// Whether sidechains need a route (`MixerTrack::sidechain_to`); false in
    /// files from before routes existed, which get them added on load.
    #[serde(default)]
    sidechain_routes: bool,
    next_id: u32,
    #[serde(skip)]
    path: Option<PathBuf>,
}

#[derive(Serialize)]
struct FileOut<'a> {
    format: &'static str,
    version: u32,
    project: &'a Project,
}

#[derive(Deserialize)]
struct FileIn {
    format: String,
    version: u32,
    project: Project,
}

impl Project {
    /// Creates an empty project: one empty pattern, master plus
    /// [`DEFAULT_INSERTS`] insert tracks, no channels.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let mut mixer = vec![MixerTrack::new("Master")];
        mixer.extend((1..=DEFAULT_INSERTS).map(|i| MixerTrack::new(format!("Insert {i}"))));
        Self {
            name: name.into(),
            tempo_bpm: DEFAULT_TEMPO_BPM,
            transport: Transport::Stopped,
            requests: Requests::default(),
            armed: false,
            step_edit: false,
            record: RecordOptions::default(),
            play_mode: PlayMode::Pattern,
            channels: Vec::new(),
            patterns: vec![Pattern::new(PatternId(1), "Pattern 1")],
            current_pattern: 0,
            mixer,
            playlist: Playlist::default(),
            automations: Vec::new(),
            controllers: Vec::new(),
            metronome: false,
            pan_law: PanLaw::Circular,
            swing: 0.0,
            master_pitch: 0.0,
            time_signature: TimeSig::COMMON,
            sidechain_routes: true,
            next_id: 2,
            path: None,
        }
    }

    /// The project's display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Renames the project and returns the previous name.
    pub fn set_name(&mut self, name: impl Into<String>) -> String {
        std::mem::replace(&mut self.name, name.into())
    }

    /// Current tempo in beats per minute.
    #[must_use]
    pub fn tempo_bpm(&self) -> f64 {
        self.tempo_bpm
    }

    /// Current transport state.
    #[must_use]
    pub fn transport(&self) -> Transport {
        self.transport
    }

    /// Sets the tempo and returns the previous value.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::TempoOutOfRange`] if `bpm` is not finite or lies
    /// outside [`MIN_TEMPO_BPM`]..=[`MAX_TEMPO_BPM`]. The project is unchanged.
    pub fn set_tempo_bpm(&mut self, bpm: f64) -> Result<f64, ProjectError> {
        if !bpm.is_finite() || !(MIN_TEMPO_BPM..=MAX_TEMPO_BPM).contains(&bpm) {
            return Err(ProjectError::TempoOutOfRange(bpm));
        }
        Ok(std::mem::replace(&mut self.tempo_bpm, bpm))
    }

    /// Whether recording is armed.
    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.armed
    }

    /// Arms or disarms recording and returns the previous state.
    pub fn set_armed(&mut self, armed: bool) -> bool {
        std::mem::replace(&mut self.armed, armed)
    }

    /// Whether step edit is on.
    #[must_use]
    pub fn step_edit(&self) -> bool {
        self.step_edit
    }

    /// Turns step edit on or off and returns the previous state.
    pub fn set_step_edit(&mut self, on: bool) -> bool {
        std::mem::replace(&mut self.step_edit, on)
    }

    /// The first pattern with no notes.
    #[must_use]
    pub fn first_empty_pattern(&self) -> Option<usize> {
        self.patterns.iter().position(|p| p.notes.is_empty())
    }

    /// The first pattern after the selected one with no notes.
    #[must_use]
    pub fn next_empty_pattern(&self) -> Option<usize> {
        self.patterns
            .iter()
            .enumerate()
            .skip(self.current_pattern + 1)
            .find(|(_, p)| p.notes.is_empty())
            .map(|(i, _)| i)
    }

    /// How many times jump-to-start and panic have been requested.
    #[must_use]
    pub fn requests(&self) -> Requests {
        self.requests
    }

    /// Asks the engine to move the playhead to where Stop returns.
    pub fn request_jump_to_start(&mut self) {
        self.requests.jumps_to_start = self.requests.jumps_to_start.wrapping_add(1);
    }

    /// Asks the engine to release every sounding note.
    pub fn request_panic(&mut self) {
        self.requests.panics = self.requests.panics.wrapping_add(1);
    }

    /// Sets the transport state and returns the previous state.
    pub fn set_transport(&mut self, transport: Transport) -> Transport {
        std::mem::replace(&mut self.transport, transport)
    }

    /// File the project was last saved to or loaded from.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Records the project's file location.
    pub fn set_path(&mut self, path: Option<PathBuf>) {
        self.path = path;
    }

    /// Returns a fresh identifier for a channel or pattern.
    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// The next identifier [`Project::alloc_id`] will return.
    #[must_use]
    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    /// Rewinds or advances the identifier allocator. Undo history uses this so
    /// that redoing a command recreates the identifiers it created originally;
    /// callers must ensure no existing item uses an identifier at or above `id`.
    pub fn set_next_id(&mut self, id: u32) {
        self.next_id = id;
    }

    /// Ensures future identifiers are greater than `id` (used when restoring).
    pub fn reserve_id(&mut self, id: u32) {
        self.next_id = self.next_id.max(id.saturating_add(1));
    }

    /// Index of a 1-based channel number.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::NotFound`] if there is no such channel.
    pub fn channel_index(&self, number: usize) -> Result<usize, ProjectError> {
        number
            .checked_sub(1)
            .filter(|i| *i < self.channels.len())
            .ok_or(ProjectError::NotFound {
                what: "channel",
                number,
            })
    }

    /// Index of a 1-based pattern number.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::NotFound`] if there is no such pattern.
    pub fn pattern_index(&self, number: usize) -> Result<usize, ProjectError> {
        number
            .checked_sub(1)
            .filter(|i| *i < self.patterns.len())
            .ok_or(ProjectError::NotFound {
                what: "pattern",
                number,
            })
    }

    /// Checks a mixer track number (0 = master).
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::NotFound`] if there is no such track.
    pub fn mixer_index(&self, number: usize) -> Result<usize, ProjectError> {
        if number < self.mixer.len() {
            Ok(number)
        } else {
            Err(ProjectError::NotFound {
                what: "mixer track",
                number,
            })
        }
    }

    /// Display index of the channel with `id`.
    #[must_use]
    pub fn channel_position(&self, id: ChannelId) -> Option<usize> {
        self.channels.iter().position(|c| c.id == id)
    }

    /// Display index of the automation with `id`.
    #[must_use]
    pub fn automation_position(&self, id: AutomationId) -> Option<usize> {
        self.automations.iter().position(|a| a.id == id)
    }

    /// Index of a 1-based controller number.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::NotFound`] if there is no such controller.
    pub fn controller_index(&self, number: usize) -> Result<usize, ProjectError> {
        number
            .checked_sub(1)
            .filter(|i| *i < self.controllers.len())
            .ok_or(ProjectError::NotFound {
                what: "controller",
                number,
            })
    }

    /// Index of a 1-based automation number.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::NotFound`] if there is no such automation.
    pub fn automation_index(&self, number: usize) -> Result<usize, ProjectError> {
        number
            .checked_sub(1)
            .filter(|i| *i < self.automations.len())
            .ok_or(ProjectError::NotFound {
                what: "automation",
                number,
            })
    }

    /// Display index of the pattern with `id`.
    #[must_use]
    pub fn pattern_position(&self, id: PatternId) -> Option<usize> {
        self.patterns.iter().position(|p| p.id == id)
    }

    /// Whether channel `index` sounds: not muted, and soloed whenever any
    /// channel is.
    #[must_use]
    pub fn channel_plays(&self, index: usize) -> bool {
        let any_solo = self.channels.iter().any(|c| c.solo);
        self.channels
            .get(index)
            .is_some_and(|c| !c.mute && (!any_solo || c.solo || c.solo_lock))
    }

    /// When any insert is soloed: which tracks stay audible. A soloed track keeps
    /// everything that feeds it and everything it feeds through sends.
    #[must_use]
    pub fn mixer_audible(&self) -> Option<Vec<bool>> {
        let mixer = &self.mixer;
        if !mixer.iter().skip(1).any(|t| t.solo) {
            return None;
        }
        let edges: Vec<(usize, usize)> = mixer
            .iter()
            .enumerate()
            .flat_map(|(i, t)| t.sends.iter().map(move |s| (i, s.target)))
            .collect();
        let mut audible: Vec<bool> = mixer.iter().map(|t| t.solo).collect();
        // Downstream: sends point to higher indices, so one forward pass suffices.
        for i in 0..mixer.len() {
            if audible[i] {
                for (from, to) in &edges {
                    if *from == i && *to < audible.len() {
                        audible[*to] = true;
                    }
                }
            }
        }
        // Upstream from the originally soloed tracks: one backward pass.
        let mut feeds: Vec<bool> = mixer.iter().map(|t| t.solo).collect();
        for i in (0..mixer.len()).rev() {
            if feeds[i] {
                for (from, to) in &edges {
                    if *to == i {
                        feeds[*from] = true;
                    }
                }
            }
        }
        Some(audible.iter().zip(&feeds).map(|(a, f)| *a || *f).collect())
    }

    /// The selected pattern.
    #[must_use]
    pub fn current_pattern(&self) -> &Pattern {
        let last = self.patterns.len().saturating_sub(1);
        &self.patterns[self.current_pattern.min(last)]
    }

    /// The selected pattern, mutably.
    pub fn current_pattern_mut(&mut self) -> &mut Pattern {
        let last = self.patterns.len().saturating_sub(1);
        let index = self.current_pattern.min(last);
        &mut self.patterns[index]
    }

    /// Song length in ticks: the end of the last clip, rounded up to a bar.
    #[must_use]
    pub fn song_length_ticks(&self) -> u32 {
        // A song-loop marker past the last clip extends the song to it.
        let end = self
            .playlist
            .length_ticks()
            .max(self.playlist.song_loop_tick().unwrap_or(0));
        self.song_meter().round_up_to_bar(end)
    }

    /// The song's bar and beat grid: the project signature and the signature
    /// markers.
    #[must_use]
    pub fn song_meter(&self) -> Meter {
        Meter::with_changes(self.time_signature, self.playlist.signature_changes())
    }

    /// The grid of what plays: the selected pattern's in pattern mode, the
    /// song's in song mode.
    #[must_use]
    pub fn playing_meter(&self) -> Meter {
        match self.play_mode {
            PlayMode::Pattern => self.current_pattern().meter(),
            PlayMode::Song => self.song_meter(),
        }
    }

    /// Serializes the project to the versioned JSON file format.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::Format`] if serialization fails.
    pub fn to_json(&self) -> Result<String, ProjectError> {
        serde_json::to_string_pretty(&FileOut {
            format: FORMAT_NAME,
            version: FORMAT_VERSION,
            project: self,
        })
        .map_err(|e| ProjectError::Format(e.to_string()))
    }

    /// Reads a project from the JSON file format, validating and repairing
    /// references.
    ///
    /// # Errors
    ///
    /// Returns [`ProjectError::Format`] for malformed or unsupported files and
    /// [`ProjectError::TempoOutOfRange`] for an invalid tempo.
    pub fn from_json(text: &str) -> Result<Self, ProjectError> {
        let file: FileIn =
            serde_json::from_str(text).map_err(|e| ProjectError::Format(e.to_string()))?;
        if file.format != FORMAT_NAME {
            return Err(ProjectError::Format(format!(
                "not a MusicCraft project (format `{}`)",
                file.format
            )));
        }
        if file.version > FORMAT_VERSION {
            return Err(ProjectError::Format(format!(
                "file version {} is newer than supported version {FORMAT_VERSION}",
                file.version
            )));
        }
        let mut project = file.project;
        project.validate()?;
        Ok(project)
    }

    /// Adds a sidechain route for every effect whose sidechain names a track
    /// (files from before routes existed keyed without one).
    fn add_sidechain_routes(&mut self) {
        let tracks = self.mixer.len();
        let mut routes = Vec::new();
        for (target, track) in self.mixer.iter().enumerate() {
            for effect in &track.effects {
                let Some(index) = effect.kind.sidechain_index() else {
                    continue;
                };
                let source = effect.values.get(index).copied().unwrap_or(0.0).round() as usize;
                if source > 0 && source < tracks && source != target {
                    routes.push((source, target));
                }
            }
        }
        for (source, target) in routes {
            let to = &mut self.mixer[source].sidechain_to;
            if !to.contains(&target) && to.len() < MAX_SENDS {
                to.push(target);
                to.sort_unstable();
            }
        }
    }

    /// Repairs dangling references after loading and checks invariants.
    fn validate(&mut self) -> Result<(), ProjectError> {
        let tempo = self.tempo_bpm;
        self.tempo_bpm = DEFAULT_TEMPO_BPM;
        self.set_tempo_bpm(tempo)?;
        if self.mixer.is_empty() {
            self.mixer.push(MixerTrack::new("Master"));
        }
        if self.patterns.is_empty() {
            let id = self.alloc_id();
            self.patterns.push(Pattern::new(PatternId(id), "Pattern 1"));
        }
        let max_id = self
            .channels
            .iter()
            .map(|c| c.id.0)
            .chain(self.patterns.iter().map(|p| p.id.0))
            .max()
            .unwrap_or(0);
        self.reserve_id(max_id);
        self.current_pattern = self.current_pattern.min(self.patterns.len() - 1);
        let tracks = self.mixer.len();
        for channel in &mut self.channels {
            if channel.mixer_track >= tracks {
                channel.mixer_track = 0;
            }
            channel.volume = channel.volume.clamp(MIN_VOLUME, MAX_VOLUME);
            channel.pan = channel.pan.clamp(-1.0, 1.0);
        }
        for (i, track) in self.mixer.iter_mut().enumerate() {
            track
                .sends
                .retain(|send| send.target > i && send.target < tracks && send.level.is_finite());
            track.sends.sort_by_key(|send| send.target);
            track.sends.dedup_by_key(|send| send.target);
            track.sends.truncate(MAX_SENDS);
            if i == 0 {
                track.to_master = true;
                track.sidechain_to.clear();
            }
            track.sidechain_to.retain(|&t| t != i && t < tracks);
            track.sidechain_to.sort_unstable();
            track.sidechain_to.dedup();
            track.sidechain_to.truncate(MAX_SENDS);
        }
        if !self.sidechain_routes {
            self.add_sidechain_routes();
            self.sidechain_routes = true;
        }
        let playlist = &mut self.playlist;
        playlist.muted_tracks.retain(|t| *t < PLAYLIST_TRACKS);
        playlist.soloed_tracks.retain(|t| *t < PLAYLIST_TRACKS);
        playlist.armed_tracks.retain(|t| *t < PLAYLIST_TRACKS);
        playlist.track_names.retain(|t, _| *t < PLAYLIST_TRACKS);
        playlist
            .track_colors
            .retain(|t, c| *t < PLAYLIST_TRACKS && *c < 16);
        playlist
            .track_heights
            .retain(|t, h| *t < PLAYLIST_TRACKS && *h != 100 && TRACK_HEIGHT_PERCENT.contains(h));
        // Older files kept swing per pattern: take the largest.
        if self.swing == 0.0 {
            self.swing = self
                .patterns
                .iter()
                .map(|p| p.legacy_swing)
                .filter(|s| s.is_finite())
                .fold(0.0, f32::max);
        }
        for pattern in &mut self.patterns {
            pattern.legacy_swing = 0.0;
        }
        self.master_pitch = if self.master_pitch.is_finite() {
            self.master_pitch.clamp(-1200.0, 1200.0)
        } else {
            0.0
        };
        self.swing = if self.swing.is_finite() {
            self.swing.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let channel_ids: Vec<ChannelId> = self.channels.iter().map(|c| c.id).collect();
        for pattern in &mut self.patterns {
            pattern
                .notes
                .retain(|n| channel_ids.contains(&n.channel) && n.key <= 127 && n.length > 0);
            pattern.length_steps = pattern.length_steps.clamp(1, MAX_PATTERN_STEPS);
            pattern.sort_notes();
        }
        let pattern_ids: Vec<PatternId> = self.patterns.iter().map(|p| p.id).collect();
        for automation in &mut self.automations {
            for point in &mut automation.points {
                point.value = if point.value.is_finite() {
                    point.value.clamp(0.0, 1.0)
                } else {
                    0.0
                };
            }
            automation.sort_points();
        }
        self.repair_plugin_instances();
        let automation_ids: Vec<AutomationId> = self.automations.iter().map(|a| a.id).collect();
        let max_id = self.automations.iter().map(|a| a.id.0).max().unwrap_or(0);
        self.reserve_id(max_id);
        let sources_exist = |c: &Clip| match c.source {
            ClipSource::Pattern(id) => pattern_ids.contains(&id),
            ClipSource::Automation(id) => automation_ids.contains(&id),
            ClipSource::Audio(id) => channel_ids.contains(&id),
        };
        self.playlist
            .clips
            .retain(|c| sources_exist(c) && c.length > 0 && c.track < PLAYLIST_TRACKS);

        self.playlist.sort_clips();
        Ok(())
    }
}

impl Project {
    /// Every hosted plugin with where it sits, channels first.
    #[must_use]
    pub fn plugins(&self) -> Vec<(PluginSlot, &PluginRef)> {
        let channels = self
            .channels
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Some((PluginSlot::Channel(i), c.instrument.plugin()?)));
        let effects =
            self.mixer.iter().enumerate().flat_map(|(t, track)| {
                track.effects.iter().enumerate().filter_map(move |(s, e)| {
                    Some((PluginSlot::Effect(t, s), e.plugin.as_deref()?))
                })
            });
        channels.chain(effects).collect()
    }

    /// The plugin at `slot`, if there is one.
    #[must_use]
    pub fn plugin(&self, slot: PluginSlot) -> Option<&PluginRef> {
        match slot {
            PluginSlot::Channel(c) => self.channels.get(c)?.instrument.plugin(),
            PluginSlot::Effect(t, s) => self.mixer.get(t)?.effects.get(s)?.plugin.as_deref(),
        }
    }

    /// The plugin at `slot`, mutably.
    pub fn plugin_mut(&mut self, slot: PluginSlot) -> Option<&mut PluginRef> {
        match slot {
            PluginSlot::Channel(c) => match &mut self.channels.get_mut(c)?.instrument {
                Instrument::Plugin { plugin } => Some(plugin),
                _ => None,
            },
            PluginSlot::Effect(t, s) => self
                .mixer
                .get_mut(t)?
                .effects
                .get_mut(s)?
                .plugin
                .as_deref_mut(),
        }
    }

    /// Where the plugin with instance number `instance` sits.
    #[must_use]
    pub fn plugin_slot(&self, instance: u32) -> Option<PluginSlot> {
        self.plugins()
            .into_iter()
            .find(|(_, p)| p.instance == instance)
            .map(|(slot, _)| slot)
    }

    /// Gives every plugin a distinct instance number and reserves them.
    fn repair_plugin_instances(&mut self) {
        let slots: Vec<PluginSlot> = self.plugins().into_iter().map(|(s, _)| s).collect();
        let max = self
            .plugins()
            .iter()
            .map(|(_, p)| p.instance)
            .max()
            .unwrap_or(0);
        self.reserve_id(max);
        let mut seen = BTreeSet::new();
        for slot in slots {
            let fresh = self.next_id;
            if let Some(plugin) = self.plugin_mut(slot) {
                if !seen.insert(plugin.instance) {
                    plugin.instance = fresh;
                    seen.insert(fresh);
                    self.next_id += 1;
                }
            }
        }
    }
}

impl PartialEq for Project {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.tempo_bpm == other.tempo_bpm
            && self.transport == other.transport
            && self.play_mode == other.play_mode
            && self.channels == other.channels
            && self.patterns == other.patterns
            && self.current_pattern == other.current_pattern
            && self.mixer == other.mixer
            && self.playlist == other.playlist
            && self.metronome == other.metronome
            && self.pan_law == other.pan_law
            && self.record == other.record
            && self.swing == other.swing
            && self.master_pitch == other.master_pitch
            && self.time_signature == other.time_signature
            && self.automations == other.automations
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

#[cfg(test)]
mod tests;
