use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use musiccraft_dsp::convolution::Convolver;
use musiccraft_dsp::rack::{Rack, RackItem};
use musiccraft_dsp::{Effect, SampleBuffer};
use musiccraft_engine::{
    ChannelParams, EffectParams, EngineHandle, EngineMessage, Graph, Instrument, MAX_BLOCK,
    MAX_EXTERNALS, MAX_SENDS, PanLaw, SendParams, Sequence, Start, TrackParams,
};
use musiccraft_params::{BUILTIN_PREFIX, EffectKind};
use musiccraft_paths::Resolver;
use musiccraft_project::{Channel, MixerTrack, Project, Requests, base64};
use musiccraft_samples::SampleCache;

use crate::build_sequence;
use crate::plugins::{PluginEvent, PluginHost, PluginInstance, PluginRequest};

/// Where a channel's sound comes from, as far as the graph is concerned.
#[derive(Debug, Clone, PartialEq)]
enum Source {
    /// A resolved sample reference.
    Sample(String),
    Synth,
    DrumSynth,
    Pluck,
    Fm,
    Keys,
    Additive,
    /// A resolved sample reference played as grains.
    Granular(String),
    /// A resolved sample reference to resynthesize.
    Resynth(String),
    /// Resolved pad sample references (empty for an empty pad).
    Kit(Vec<String>),
    /// Zones with resolved sample references.
    Zones(Vec<musiccraft_project::SampleZone>),
    /// Channel positions the layer plays.
    Layer(Vec<u16>),
    /// A plugin in this engine slot (`None`: not running).
    Plugin(Option<u16>),
    /// External MIDI gear on this MIDI channel (0–15).
    MidiOut(u8),
}

/// What a rebuild depends on for one effect slot: its kind, the engine slot
/// of a plugin, the file it loads, and an effect rack's branch effects (kind
/// and file).
type SlotShape = (
    EffectKind,
    Option<u16>,
    Option<String>,
    Vec<Vec<(EffectKind, Option<String>)>>,
);

/// Settings of one effect inside a rack: (track, slot, branch, position).
type RackParams = ((u16, u8, u8, u8), EffectParams);

/// The parts of a project that require a new [`Graph`] when they change.
#[derive(Debug, Clone, PartialEq)]
struct Structure {
    channels: Vec<Source>,
    /// Effect kinds per track, with the engine slot of plugin effects and
    /// the file an effect loads.
    tracks: Vec<Vec<SlotShape>>,
}

/// A plugin instance the bridge keeps alive.
#[derive(Debug)]
struct Live {
    plugin_id: String,
    slot: u16,
    instance: Box<dyn PluginInstance>,
    /// Parameter values the engine was last sent.
    sent: BTreeMap<u32, f64>,
    /// Values right after the state was restored, used when a stored value is
    /// removed (undo of the first change).
    baseline: BTreeMap<u32, f64>,
}

/// Everything the engine was last told.
#[derive(Debug, Clone, PartialEq)]
struct Desc {
    structure: Structure,
    channels: Vec<ChannelParams>,
    tracks: Vec<TrackParams>,
    effects: Vec<Vec<EffectParams>>,
    /// Every rack branch effect, in track, slot, branch and position order.
    racks: Vec<RackParams>,
    sequence: Sequence,
    tempo: f64,
    playing: bool,
    paused: bool,
    metronome: bool,
    start: Start,
    stop_at_end: bool,
}

/// What a [`Bridge::sync`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Number of messages sent.
    pub messages: usize,
    /// Whether the graph was rebuilt.
    pub rebuilt_graph: bool,
    /// Whether a new sequence was sent.
    pub new_sequence: bool,
    /// Problems such as samples that failed to load (they play silence).
    pub warnings: Vec<String>,
    /// Messages that did not fit in the queue; the next sync retries.
    pub dropped: usize,
}

/// The file a sample path loads from: built-in names as they are, paths
/// from another system translated (see `docs/spec/sample-paths.md`), relative
/// paths from the project's folder.
fn sample_key(sample: &str, base: Option<&Path>, paths: &mut Resolver) -> String {
    // An empty sampler stays empty instead of naming the project folder.
    if sample.is_empty() || sample.starts_with(BUILTIN_PREFIX) {
        return sample.to_owned();
    }
    if let Some(found) = paths.resolve(sample, base) {
        return found.to_string_lossy().into_owned();
    }
    match base {
        Some(dir) if Path::new(sample).is_relative() => {
            dir.join(sample).to_string_lossy().into_owned()
        }
        _ => sample.to_owned(),
    }
}

/// Bars of the recording count-in (of the signature where playback starts).
const COUNT_IN_BARS: u32 = 1;

/// How Play begins: count-in and wait for input apply only while armed, and
/// waiting takes over from the count-in.
fn start_mode(project: &Project) -> Start {
    let options = project.record;
    if !project.is_armed() {
        Start::Now
    } else if options.wait_for_input {
        Start::WaitForInput
    } else if options.count_in {
        Start::CountIn(COUNT_IN_BARS)
    } else {
        Start::Now
    }
}

fn channel_params(channel: &Channel, pan_law: PanLaw, master_pitch: f32) -> ChannelParams {
    let mut instrument = channel.params.0;
    // The master pitch transposes every built-in instrument.
    if let Some(pitch) = instrument.get_mut(musiccraft_params::inst::PITCH) {
        *pitch += master_pitch / 100.0;
    }
    ChannelParams {
        volume: channel.volume,
        pan: channel.pan,
        pan_law,
        mute: channel.mute,
        track: u16::try_from(channel.mixer_track).unwrap_or(0),
        instrument,
    }
}

fn track_params(track: &MixerTrack, index: usize, audible: Option<&[bool]>) -> TrackParams {
    // Solo applies to inserts; the master is never silenced by another solo.
    let soloed_out = index > 0 && audible.is_some_and(|a| !a.get(index).copied().unwrap_or(false));
    let mut params = TrackParams {
        volume: track.volume,
        pan: track.pan,
        separation: track.separation,
        invert: track.phase_invert,
        swap: track.swap_sides,
        eq: track.eq.params(),
        mute: track.mute || soloed_out,
        to_master: track.to_master || index == 0,
        ..TrackParams::default()
    };
    let sends = track
        .sends
        .iter()
        .filter(|s| s.target > index)
        .take(MAX_SENDS);
    for (slot, send) in params.sends.iter_mut().zip(sends) {
        *slot = SendParams {
            target: u16::try_from(send.target).unwrap_or(u16::MAX),
            level: send.level,
        };
        params.send_count += 1;
    }
    params
}

/// When any insert is soloed: which tracks stay audible. A soloed track keeps
/// everything that feeds it and everything it feeds through sends.
fn solo_audible(mixer: &[MixerTrack]) -> Option<Vec<bool>> {
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

fn describe(project: &Project, slots: &BTreeMap<u32, u16>, paths: &mut Resolver) -> Desc {
    let base = project.path().and_then(Path::parent);
    let audible = solo_audible(&project.mixer);
    let sequence = build_sequence(project);
    let pan_law = match project.pan_law {
        musiccraft_project::PanLaw::Circular => PanLaw::Circular,
        musiccraft_project::PanLaw::Triangular => PanLaw::Triangular,
    };
    Desc {
        structure: Structure {
            channels: project
                .channels
                .iter()
                .map(|c| match &c.instrument {
                    musiccraft_project::Instrument::Sampler { sample } => {
                        Source::Sample(sample_key(sample, base, paths))
                    }
                    musiccraft_project::Instrument::Synth => Source::Synth,
                    musiccraft_project::Instrument::DrumSynth => Source::DrumSynth,
                    musiccraft_project::Instrument::Pluck => Source::Pluck,
                    musiccraft_project::Instrument::Fm => Source::Fm,
                    musiccraft_project::Instrument::Keys => Source::Keys,
                    musiccraft_project::Instrument::Additive => Source::Additive,
                    musiccraft_project::Instrument::Granular { sample } => {
                        Source::Granular(sample_key(sample, base, paths))
                    }
                    musiccraft_project::Instrument::Resynth { sample } => {
                        Source::Resynth(sample_key(sample, base, paths))
                    }
                    musiccraft_project::Instrument::Kit { pads } => Source::Kit(
                        pads.iter()
                            .map(|p| {
                                if p.is_empty() {
                                    String::new()
                                } else {
                                    sample_key(p, base, paths)
                                }
                            })
                            .collect(),
                    ),
                    musiccraft_project::Instrument::Zones { zones } => Source::Zones(
                        zones
                            .iter()
                            .map(|z| musiccraft_project::SampleZone {
                                sample: sample_key(&z.sample, base, paths),
                                ..z.clone()
                            })
                            .collect(),
                    ),
                    musiccraft_project::Instrument::Layer { channels } => Source::Layer(
                        channels
                            .iter()
                            .filter_map(|id| project.channel_position(*id))
                            .filter_map(|i| u16::try_from(i).ok())
                            .collect(),
                    ),
                    musiccraft_project::Instrument::Plugin { plugin } => {
                        Source::Plugin(slots.get(&plugin.instance).copied())
                    }
                    musiccraft_project::Instrument::MidiOut { midi_channel } => {
                        Source::MidiOut((*midi_channel).clamp(1, 16) - 1)
                    }
                })
                .collect(),
            tracks: project
                .mixer
                .iter()
                .map(|t| {
                    t.effects
                        .iter()
                        .map(|e| {
                            let slot = e
                                .plugin
                                .as_ref()
                                .and_then(|p| slots.get(&p.instance).copied());
                            let branches = e
                                .branches
                                .iter()
                                .map(|b| b.iter().map(|c| (c.kind, c.file.clone())).collect())
                                .collect();
                            (e.kind, slot, e.file.clone(), branches)
                        })
                        .collect()
                })
                .collect(),
        },
        channels: project
            .channels
            .iter()
            .map(|c| channel_params(c, pan_law, project.master_pitch))
            .collect(),
        tracks: project
            .mixer
            .iter()
            .enumerate()
            .map(|(i, t)| track_params(t, i, audible.as_deref()))
            .collect(),
        effects: project
            .mixer
            .iter()
            .map(|t| {
                t.effects
                    .iter()
                    .map(|e| EffectParams {
                        enabled: e.enabled,
                        values: e.values,
                        mix: e.mix,
                    })
                    .collect()
            })
            .collect(),
        racks: rack_params(project),
        sequence,
        tempo: project.tempo_bpm(),
        playing: project.transport().is_playing(),
        paused: project.transport().is_paused(),
        metronome: project.metronome,
        start: start_mode(project),
        stop_at_end: project.is_armed() && !project.record.loop_record,
    }
}

fn rack_params(project: &Project) -> Vec<RackParams> {
    let small = |i: usize| u8::try_from(i).unwrap_or(u8::MAX);
    let mut out = Vec::new();
    for (t, track) in project.mixer.iter().enumerate() {
        for (s, slot) in track.effects.iter().enumerate() {
            for (b, branch) in slot.branches.iter().enumerate() {
                for (i, e) in branch.iter().enumerate() {
                    let at = (
                        u16::try_from(t).unwrap_or(u16::MAX),
                        small(s),
                        small(b),
                        small(i),
                    );
                    let params = EffectParams {
                        enabled: e.enabled,
                        values: e.values,
                        mix: e.mix,
                    };
                    out.push((at, params));
                }
            }
        }
    }
    out
}

/// Keeps an engine in step with a project. Lives on the main thread.
pub struct Bridge {
    sample_rate: u32,
    samples: SampleCache,
    /// Translates sample paths written on another system.
    paths: Resolver,
    last: Option<Desc>,
    host: Option<PluginHost>,
    /// Running plugins by instance number.
    live: BTreeMap<u32, Live>,
    /// Instances that failed to start, by instance number, with their plugin
    /// id; not retried until [`Bridge::retry_plugins`].
    failed: BTreeMap<u32, String>,
    /// The project's transport requests already acted on; `None` before the
    /// first sync.
    requests: Option<Requests>,
    /// Resynthesis analyses by sample key, so rebuilding is cheap.
    harmonics: BTreeMap<String, Arc<musiccraft_dsp::resynth::HarmonicTable>>,
}

impl Bridge {
    /// A bridge for an engine running at `sample_rate`.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            samples: SampleCache::new(),
            paths: Resolver::new(),
            last: None,
            host: None,
            live: BTreeMap::new(),
            failed: BTreeMap::new(),
            requests: None,
            harmonics: BTreeMap::new(),
        }
    }

    /// A bridge that can run plugins from `host`.
    #[must_use]
    pub fn with_plugins(sample_rate: u32, host: Option<PluginHost>) -> Self {
        Self {
            host,
            ..Self::new(sample_rate)
        }
    }

    /// The plugin host, if plugins are available.
    #[must_use]
    pub fn plugin_host(&self) -> Option<&PluginHost> {
        self.host.as_ref()
    }

    /// Tries plugins that failed to start again on the next sync (after a
    /// rescan, for example).
    pub fn retry_plugins(&mut self) {
        self.failed.clear();
    }

    /// The live plugin with instance number `instance`.
    pub fn plugin(&mut self, instance: u32) -> Option<&mut dyn PluginInstance> {
        Some(self.live.get_mut(&instance)?.instance.as_mut())
    }

    /// Whether the plugin with instance number `instance` is running.
    #[must_use]
    pub fn plugin_running(&self, instance: u32) -> bool {
        self.live.contains_key(&instance)
    }

    /// Runs plugin housekeeping and returns what each plugin reported, by
    /// instance number. Parameter changes a plugin made itself count as already
    /// sent, so recording them in the project does not echo them back.
    pub fn poll_plugins(&mut self) -> Vec<(u32, PluginEvent)> {
        let mut events = Vec::new();
        for (instance, live) in &mut self.live {
            for event in live.instance.poll() {
                if let PluginEvent::Param { id, value } = event {
                    live.sent.insert(id, value);
                }
                events.push((*instance, event));
            }
        }
        events
    }

    /// Starts and stops plugin instances so they match the project, then sends
    /// parameter changes.
    fn sync_plugins(
        &mut self,
        project: &Project,
        handle: &mut EngineHandle,
        report: &mut SyncReport,
    ) {
        let wanted: BTreeMap<u32, &musiccraft_project::PluginRef> = project
            .plugins()
            .into_iter()
            .map(|(_, p)| (p.instance, p))
            .collect();
        let stale: Vec<u32> = self
            .live
            .iter()
            .filter(|(k, l)| wanted.get(k).is_none_or(|p| p.id != l.plugin_id))
            .map(|(k, _)| *k)
            .collect();
        for instance in stale {
            let Some(slot) = self.live.get(&instance).map(|l| l.slot) else {
                continue;
            };
            let message = EngineMessage::SetExternal {
                slot,
                external: None,
            };
            if handle.send(message).is_ok() {
                report.messages += 1;
                // The engine hands the processor back as garbage; the instance
                // is destroyed once both halves are gone.
                self.live.remove(&instance);
            } else {
                report.dropped += 1;
            }
        }
        self.failed
            .retain(|k, id| wanted.get(k).is_some_and(|p| &p.id == id));
        for (instance, plugin) in &wanted {
            if self.live.contains_key(instance) || self.failed.contains_key(instance) {
                continue;
            }
            match self.start_plugin(plugin, handle) {
                Ok(Some(live)) => {
                    report.messages += 1;
                    self.live.insert(*instance, live);
                }
                Ok(None) => report.dropped += 1,
                Err(err) => {
                    report
                        .warnings
                        .push(format!("plugin `{}` ({}): {err}", plugin.name, plugin.id));
                    self.failed.insert(*instance, plugin.id.clone());
                }
            }
        }
        for (instance, plugin) in &wanted {
            let Some(live) = self.live.get_mut(instance) else {
                continue;
            };
            let mut changes: Vec<(u32, f64)> = plugin
                .params
                .iter()
                .filter(|(id, v)| live.sent.get(id) != Some(v))
                .map(|(id, v)| (*id, *v))
                .collect();
            // A value removed from the project returns to the baseline.
            changes.extend(
                live.sent
                    .keys()
                    .filter(|id| !plugin.params.contains_key(id))
                    .filter_map(|id| Some((*id, *live.baseline.get(id)?))),
            );
            for (id, value) in changes {
                let message = EngineMessage::ExternalParam {
                    slot: live.slot,
                    id,
                    value,
                };
                if handle.send(message).is_ok() {
                    report.messages += 1;
                    if plugin.params.contains_key(&id) {
                        live.sent.insert(id, value);
                    } else {
                        live.sent.remove(&id);
                    }
                } else {
                    report.dropped += 1;
                }
            }
        }
    }

    /// Creates a plugin and installs its processor in a free engine slot.
    /// `Ok(None)` means the message queue was full; the next sync retries.
    fn start_plugin(
        &self,
        plugin: &musiccraft_project::PluginRef,
        handle: &mut EngineHandle,
    ) -> Result<Option<Live>, String> {
        let host = self
            .host
            .as_ref()
            .ok_or("plugin hosting is not available here")?;
        let used: Vec<u16> = self.live.values().map(|l| l.slot).collect();
        let slot = (0..MAX_EXTERNALS)
            .filter_map(|s| u16::try_from(s).ok())
            .find(|s| !used.contains(s))
            .ok_or_else(|| format!("at most {MAX_EXTERNALS} plugins can run at once"))?;
        let state = if plugin.state.is_empty() {
            None
        } else {
            Some(base64::decode(&plugin.state).ok_or("its saved state is not valid base64")?)
        };
        let request = PluginRequest {
            id: &plugin.id,
            sample_rate: self.sample_rate,
            max_frames: u32::try_from(MAX_BLOCK).unwrap_or(u32::MAX),
            state: state.as_deref(),
        };
        let created = host.instantiate(&request)?;
        let baseline = created
            .instance
            .params()
            .iter()
            .filter_map(|p| Some((p.id, created.instance.param_value(p.id)?)))
            .collect();
        let message = EngineMessage::SetExternal {
            slot,
            external: Some(created.processor),
        };
        if handle.send(message).is_err() {
            return Ok(None);
        }
        Ok(Some(Live {
            plugin_id: plugin.id.clone(),
            slot,
            instance: created.instance,
            sent: BTreeMap::new(),
            baseline,
        }))
    }

    /// Forgets what the engine was told, so the next sync sends everything.
    pub fn invalidate(&mut self) {
        self.last = None;
    }

    /// Forgets decoded samples and forces a full resync (after files changed).
    pub fn reload_samples(&mut self) {
        self.samples.clear();
        self.paths.clear();
        self.last = None;
    }

    /// Loads a channel's sample, substituting silence (and a warning) on error.
    fn load(&mut self, key: &str, warnings: &mut Vec<String>) -> Arc<SampleBuffer> {
        if key.is_empty() {
            // A sampler without a sample is silent by design, not an error.
            return Arc::new(SampleBuffer::silent(f64::from(self.sample_rate)));
        }
        match self.samples.get(key, None, self.sample_rate) {
            Ok(buffer) => buffer,
            Err(err) => {
                let hint = if musiccraft_paths::is_foreign(key) {
                    " (a path from another system with no matching file here: \
                     see Settings → Folders)"
                } else {
                    ""
                };
                warnings.push(format!("sample `{key}`: {err}{hint}"));
                Arc::new(SampleBuffer::silent(f64::from(self.sample_rate)))
            }
        }
    }

    /// The harmonic analysis of a sample (measured once, then cached).
    fn harmonics(
        &mut self,
        key: &str,
        warnings: &mut Vec<String>,
    ) -> Arc<musiccraft_dsp::resynth::HarmonicTable> {
        if let Some(table) = self.harmonics.get(key) {
            return Arc::clone(table);
        }
        let buffer = self.load(key, warnings);
        let table = Arc::new(musiccraft_dsp::resynth::HarmonicTable::analyze(&buffer));
        self.harmonics.insert(key.to_owned(), Arc::clone(&table));
        table
    }

    /// A built-in effect, with its file loaded (the convolution reverb's
    /// impulse response).
    fn effect(
        &mut self,
        slot: &musiccraft_project::EffectSlot,
        warnings: &mut Vec<String>,
    ) -> Effect {
        let sr = self.sample_rate as f32;
        if slot.kind == EffectKind::Rack {
            let branches = std::array::from_fn(|b| {
                slot.branches
                    .get(b)
                    .into_iter()
                    .flatten()
                    // Racks do not nest, and engine-routed effects and
                    // plugins cannot run inside one.
                    .filter(|c| c.kind.allowed_in_rack())
                    .map(|c| RackItem {
                        effect: self.effect(c, warnings),
                        enabled: c.enabled,
                        values: c.values,
                    })
                    .collect()
            });
            return Effect::Rack(Box::new(Rack::new(branches)));
        }
        if slot.kind != EffectKind::Convolution {
            return Effect::new(slot.kind, sr);
        }
        let file = slot.file.as_deref().unwrap_or("ir:hall");
        let convolver = match file.strip_prefix("ir:") {
            Some(name) => Convolver::builtin(name, sr),
            None => {
                let buffer = self.load(file, warnings);
                Convolver::new(buffer.left(), buffer.right(), sr)
            }
        };
        Effect::Convolution(convolver)
    }

    fn build_graph(&mut self, project: &Project, desc: &Desc, warnings: &mut Vec<String>) -> Graph {
        let mut graph = Graph::new();
        for (source, params) in desc.structure.channels.iter().zip(&desc.channels) {
            let instrument = match source {
                Source::Sample(key) => Instrument::Sampler(self.load(key, warnings)),
                Source::Synth => Instrument::Synth,
                Source::DrumSynth => Instrument::DrumSynth,
                Source::Pluck => Instrument::Pluck,
                Source::Fm => Instrument::Fm,
                Source::Keys => Instrument::Keys,
                Source::Additive => Instrument::Additive,
                Source::Granular(key) => Instrument::Granular(self.load(key, warnings)),
                Source::Resynth(key) => Instrument::Resynth(self.harmonics(key, warnings)),
                Source::Kit(keys) => Instrument::Kit(
                    keys.iter()
                        .map(|k| (!k.is_empty()).then(|| self.load(k, warnings)))
                        .collect(),
                ),
                Source::Zones(zones) => Instrument::Zones(
                    zones
                        .iter()
                        .map(|z| musiccraft_engine::Zone {
                            buffer: self.load(&z.sample, warnings),
                            low: z.low,
                            high: z.high,
                            root: z.root,
                            vel_low: z.vel_low,
                            vel_high: z.vel_high,
                        })
                        .collect(),
                ),
                Source::Layer(children) => Instrument::Layer(children.clone()),
                // An out-of-range slot plays nothing.
                Source::Plugin(slot) => Instrument::External(slot.unwrap_or(u16::MAX)),
                Source::MidiOut(midi_channel) => Instrument::MidiOut(*midi_channel),
            };
            graph.add_channel(instrument, *params);
        }
        for (i, (track, params)) in project.mixer.iter().zip(&desc.tracks).enumerate() {
            let effects = track
                .effects
                .iter()
                .map(|e| {
                    (
                        self.effect(e, warnings),
                        EffectParams {
                            enabled: e.enabled,
                            values: e.values,
                            mix: e.mix,
                        },
                    )
                })
                .collect();
            if i == 0 {
                graph.set_master(*params, effects);
            } else {
                graph.add_track(*params, effects);
            }
            let slots = desc.structure.tracks.get(i).into_iter().flatten();
            for (s, (_, external, _, _)) in slots.enumerate() {
                if let Some(external) = external {
                    graph.attach_external(i, s, *external);
                }
            }
        }
        graph
    }

    /// Brings the engine up to date with `project`.
    pub fn sync(&mut self, project: &Project, handle: &mut EngineHandle) -> SyncReport {
        handle.collect_garbage();
        let mut report = SyncReport::default();
        self.sync_plugins(project, handle, &mut report);
        let slots: BTreeMap<u32, u16> = self.live.iter().map(|(k, l)| (*k, l.slot)).collect();
        if self.paths.refresh() {
            // New mappings may find samples that were missing.
            self.last = None;
        }
        let desc = describe(project, &slots, &mut self.paths);
        let mut messages = Vec::new();
        let previous = self.last.take();

        let rebuild = previous
            .as_ref()
            .is_none_or(|p| p.structure != desc.structure);
        if rebuild {
            let graph = self.build_graph(project, &desc, &mut report.warnings);
            messages.push(EngineMessage::SetGraph(Box::new(graph)));
            report.rebuilt_graph = true;
        } else if let Some(prev) = &previous {
            // Automation writes parameters during playback; when it stops,
            // restore the project's values.
            let stopped = prev.playing && !desc.playing && !desc.sequence.automation().is_empty();
            for (i, (now, before)) in desc.channels.iter().zip(&prev.channels).enumerate() {
                if now != before || stopped {
                    messages.push(EngineMessage::Channel {
                        channel: u16::try_from(i).unwrap_or(u16::MAX),
                        params: *now,
                    });
                }
            }
            for (i, (now, before)) in desc.tracks.iter().zip(&prev.tracks).enumerate() {
                if now != before || stopped {
                    messages.push(EngineMessage::Track {
                        track: u16::try_from(i).unwrap_or(u16::MAX),
                        params: *now,
                    });
                }
            }
            for (t, (now, before)) in desc.effects.iter().zip(&prev.effects).enumerate() {
                for (s, (now, before)) in now.iter().zip(before).enumerate() {
                    if now != before || stopped {
                        messages.push(EngineMessage::Effect {
                            track: u16::try_from(t).unwrap_or(u16::MAX),
                            slot: u8::try_from(s).unwrap_or(u8::MAX),
                            params: *now,
                        });
                    }
                }
            }
            for ((now_at, now), (_, before)) in desc.racks.iter().zip(&prev.racks) {
                if now != before || stopped {
                    let (track, slot, branch, index) = *now_at;
                    messages.push(EngineMessage::RackEffect {
                        track,
                        slot,
                        branch,
                        index,
                        params: *now,
                    });
                }
            }
        }
        let sequence_changed = previous
            .as_ref()
            .is_none_or(|p| p.sequence != desc.sequence);
        if sequence_changed {
            messages.push(EngineMessage::SetSequence(Box::new(desc.sequence.clone())));
            report.new_sequence = true;
        }
        if previous.as_ref().is_none_or(|p| p.tempo != desc.tempo) {
            messages.push(EngineMessage::SetTempo(desc.tempo));
        }
        if previous
            .as_ref()
            .is_none_or(|p| p.metronome != desc.metronome)
        {
            messages.push(EngineMessage::SetMetronome(desc.metronome));
        }
        if previous.as_ref().is_none_or(|p| p.start != desc.start) {
            messages.push(EngineMessage::SetStart(desc.start));
        }
        if previous
            .as_ref()
            .is_none_or(|p| p.stop_at_end != desc.stop_at_end)
        {
            messages.push(EngineMessage::SetStopAtEnd(desc.stop_at_end));
        }
        if previous
            .as_ref()
            .is_none_or(|p| (p.playing, p.paused) != (desc.playing, desc.paused))
        {
            messages.push(if desc.playing {
                EngineMessage::Play
            } else if desc.paused {
                EngineMessage::Pause
            } else {
                EngineMessage::Stop
            });
        }
        let requests = project.requests();
        if let Some(handled) = self.requests {
            if requests.jumps_to_start > handled.jumps_to_start {
                let home = desc.sequence.home() as f64;
                messages.push(EngineMessage::Seek(home));
            }
            if requests.panics > handled.panics {
                messages.push(EngineMessage::AllNotesOff);
                messages.push(EngineMessage::StopPreview);
            }
        }

        let mut complete = true;
        for message in messages {
            if handle.send(message).is_ok() {
                report.messages += 1;
            } else {
                report.dropped += 1;
                complete = false;
            }
        }
        // On a full queue, forget the state so the next sync resends everything
        // (and retries the requests).
        self.last = complete.then_some(desc);
        if complete || self.requests.is_none() {
            self.requests = Some(requests);
        }
        report
    }

    /// The decoded sample a channel plays, loading it if needed.
    ///
    /// # Errors
    ///
    /// Returns a message if the sample cannot be loaded.
    pub fn sample(&mut self, sample: &str, project: &Project) -> Result<Arc<SampleBuffer>, String> {
        let key = sample_key(
            sample,
            project.path().and_then(Path::parent),
            &mut self.paths,
        );
        self.samples
            .get(&key, None, self.sample_rate)
            .map_err(|e| e.to_string())
    }

    /// Sample rate this bridge prepares data for.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}
