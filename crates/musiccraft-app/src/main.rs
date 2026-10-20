//! MusicCraft desktop application: wires the command session, the engine bridge,
//! the audio device, MIDI input and the user interface together.

// Release builds on Windows are GUI-subsystem so launching the app does not open a
// console window. (Usage errors printed to stderr are therefore not visible there.)
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod autosave;
mod control;
mod midi;
mod midi_out;
mod plugins;
mod recorder;
mod screenshot;
mod settings;
mod take;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use eframe::egui;
use musiccraft_audio::{AudioOutput, OutputDevice};
use musiccraft_commands::{Invocation, Session};
use musiccraft_engine::{EngineHandle, EngineMessage, engine_pair};
use musiccraft_project::Project;
use musiccraft_render::Bridge;
use musiccraft_ui::{
    Action, EngineStatus, Frame, History, Keymap, MidiInput, SettingsInfo, UiState,
};

/// How long a click-to-audition note sounds.
const SHORT_PREVIEW: Duration = Duration::from_millis(220);

const USAGE: &str = "\
usage: musiccraft [project.mcproj] [--keymap <file>] [--empty] [--open <windows>]
                 [--dir <folder>] [--control <port> --control-token-file <file>]
  --keymap <file>    use a keymap file instead of the default shortcuts
  --empty            start with an empty project instead of the drum starter kit
  --open <windows>   comma-separated windows to show at start:
                     channel_rack, piano_roll, playlist, mixer, browser,
                     channel_settings, automation, settings[:<tab>],
                     effect:<track>:<slot>
  --dir <folder>     read and write relative paths in this folder
  --control <port>   let an agent drive this window (musiccraft-mcp --bridge)
  --control-token-file <file>  where the control token is written";

/// A sample's key, min/max overview and length in seconds.
type Overview = (String, Vec<(f32, f32)>, f64);

/// Audio running on a device.
struct Audio {
    _output: AudioOutput,
    handle: EngineHandle,
    meters: Arc<musiccraft_engine::Meters>,
    bridge: Bridge,
}

struct App {
    session: Session,
    ui: UiState,
    audio: Option<Audio>,
    audio_label: String,
    midi: Option<midi::MidiIn>,
    midi_label: String,
    /// The engine's outgoing MIDI (MIDI Out channels), shared with the
    /// output thread.
    midi_out_rx: Option<Arc<Mutex<musiccraft_engine::MidiOutReceiver>>>,
    midi_out: Option<midi_out::MidiOut>,
    midi_out_label: String,
    /// The engine's live-note sender, shared with whichever MIDI input runs.
    live: Option<Arc<Mutex<musiccraft_engine::LiveSender>>>,
    played_tx: std::sync::mpsc::Sender<midi::Played>,
    settings: musiccraft_ui::Settings,
    settings_info: SettingsInfo,
    selected_channel: Arc<AtomicUsize>,
    synced_revision: Option<u64>,
    pending_offs: Vec<(Instant, u16, u8)>,
    title: String,
    screenshot: Option<screenshot::Request>,
    autosave: autosave::Autosave,
    recorder: recorder::Recorder,
    midi_notes: Option<Receiver<midi::Played>>,
    /// Waveform overview of the selected channel's sample, keyed by sample.
    overview: Option<Overview>,
    /// Overviews and durations (seconds) of samples played by audio clips.
    clip_audio: Vec<ClipAudio>,
    /// Audio input, opened the first time an audio take starts.
    input: Option<musiccraft_audio::AudioInput>,
    take: Option<take::Take>,
    takes: u32,
    plugins: plugins::PluginGlue,
    /// Live mode: an agent driving this window through `musiccraft-mcp --bridge`.
    control: Option<control::Control>,
    /// The project as last saved, loaded or created; anything else is unsaved.
    saved_json: Option<String>,
    /// An action waiting for the "unsaved changes" question to be answered.
    pending: Option<Pending>,
    /// Set once the window may close without asking again.
    closing: bool,
    /// Pause-marker stops seen from the engine so far.
    marker_pauses: u64,
    /// Loop-end stops (recording without looping) seen so far.
    end_stops: u64,
    /// Replace recording: what the playhead has passed in this take.
    sweep: recorder::Sweep,
    /// Step edit: the chord being entered.
    steps: recorder::StepEntry,
}

/// What happens after the user deals with unsaved changes.
enum Pending {
    /// Close the window.
    Close,
    /// Run a command that replaces the project (new, open, recover).
    Run(Invocation),
}

/// The answer to "save changes first?".
#[derive(Clone, Copy)]
enum UnsavedChoice {
    Save,
    Discard,
    Cancel,
}

/// Commands that replace the open project, losing unsaved changes.
fn replaces_project(name: &str) -> bool {
    matches!(name, "project.new" | "project.load" | "project.recover")
}

/// Commands after which the project matches a file (or a fresh start).
fn marks_saved(name: &str) -> bool {
    matches!(
        name,
        "project.new" | "project.load" | "project.save" | "project.save_new_version"
    )
}

/// Display data for a sample that audio clips play.
#[derive(Clone)]
struct ClipAudio {
    channel: musiccraft_project::ChannelId,
    sample: String,
    peaks: Vec<(f32, f32)>,
    seconds: f64,
}

/// Min/max pairs summarizing a sample for display.
/// What every pitch corrector and tuner in the project currently hears.
fn pitch_readings(
    project: &Project,
    meters: &musiccraft_engine::Meters,
) -> Vec<(usize, usize, f32)> {
    use musiccraft_project::params::EffectKind;
    project
        .mixer
        .iter()
        .enumerate()
        .flat_map(|(t, track)| {
            track
                .effects
                .iter()
                .enumerate()
                .filter(|(_, e)| matches!(e.kind, EffectKind::PitchCorrect | EffectKind::Tuner))
                .filter_map(move |(s, _)| meters.pitch(t, s).map(|hz| (t, s, hz)))
        })
        .collect()
}

fn overview(buffer: &musiccraft_dsp::SampleBuffer, points: usize) -> Vec<(f32, f32)> {
    let len = buffer.len();
    if len == 0 {
        return Vec::new();
    }
    let per = len.div_ceil(points).max(1);
    buffer
        .left()
        .chunks(per)
        .zip(buffer.right().chunks(per))
        .map(|(l, r)| {
            l.iter()
                .zip(r)
                .fold((0.0_f32, 0.0_f32), |(lo, hi), (a, b)| {
                    let m = (a + b) * 0.5;
                    (lo.min(m), hi.max(m))
                })
        })
        .collect()
}

/// A new project with a few drum channels, ready to program.
fn starter_project() -> Project {
    let mut session = Session::new(Project::default(), musiccraft_cli::full_registry());
    for line in [
        "channel.add_sampler builtin:kick",
        "channel.add_sampler builtin:clap",
        "channel.add_sampler builtin:hat",
        "channel.add_sampler builtin:snare",
    ] {
        let _ = session.run_line(line);
    }
    session.project().clone()
}

struct Options {
    project: Option<PathBuf>,
    keymap: Option<PathBuf>,
    empty: bool,
    open: Option<Vec<String>>,
    control: Option<u16>,
    token_file: Option<PathBuf>,
}

fn parse_args() -> Result<Options, String> {
    let mut options = Options {
        project: None,
        keymap: None,
        empty: false,
        open: None,
        control: None,
        token_file: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--keymap" => options.keymap = Some(args.next().ok_or(USAGE)?.into()),
            "--empty" => options.empty = true,
            "--dir" => {
                let dir = args.next().ok_or(USAGE)?;
                std::env::set_current_dir(&dir).map_err(|e| format!("--dir {dir}: {e}"))?;
            }
            "--control" => {
                let port = args.next().ok_or(USAGE)?;
                options.control = Some(
                    port.parse()
                        .map_err(|_| format!("bad port {port}\n{USAGE}"))?,
                );
            }
            "--control-token-file" => options.token_file = Some(args.next().ok_or(USAGE)?.into()),
            "--open" => {
                let list = args.next().ok_or(USAGE)?;
                options.open = Some(list.split(',').map(|w| w.trim().to_owned()).collect());
            }
            "-h" | "--help" => return Err(USAGE.to_owned()),
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}\n{USAGE}"));
            }
            path => options.project = Some(path.into()),
        }
    }
    Ok(options)
}

impl App {
    fn new(options: &Options, ctx: &egui::Context) -> Self {
        let mut status = None;
        let keymap = match &options.keymap {
            Some(path) => match std::fs::read_to_string(path)
                .map_err(|e| e.to_string())
                .and_then(|text| Keymap::parse(&text))
            {
                Ok(keymap) => keymap,
                Err(err) => {
                    status = Some(format!("keymap {}: {err}; using defaults", path.display()));
                    Keymap::default_keymap()
                }
            },
            None => Keymap::default_keymap(),
        };
        let settings = settings::load();
        // Sample paths from Windows projects translate with these folders.
        musiccraft_paths::configure(settings.path_config());
        ctx.set_zoom_factor(settings.scale());
        if let Some(host) = musiccraft_cli::plugin_host() {
            host.set_extra_folders(settings.plugin_folders.clone());
        }
        let project = if options.empty || settings.start_empty {
            Project::default()
        } else {
            starter_project()
        };
        let mut session = Session::new(project, musiccraft_cli::full_registry());
        if let Some(path) = &options.project {
            let load = Invocation::new("project.load", [path.display().to_string()]);
            if let Err(err) = session.run(&load) {
                status = Some(err.to_string());
            }
        }

        let selected_channel = Arc::new(AtomicUsize::new(0));
        let (played_tx, played_rx) = channel();
        let revision = session.revision();
        let saved_json = session.project().to_json().ok();
        let mut ui = UiState::new(keymap);
        // An autosave left behind means the last session did not close
        // properly; keep it safe from this session's autosaves.
        ui.autosave_path = autosave::take_crash_backup().map(|p| p.display().to_string());
        if let Some(windows) = &options.open {
            let w = &mut ui.windows;
            w.channel_rack = windows.iter().any(|n| n == "channel_rack");
            w.piano_roll = windows.iter().any(|n| n == "piano_roll");
            w.playlist = windows.iter().any(|n| n == "playlist");
            w.mixer = windows.iter().any(|n| n == "mixer");
            w.browser = windows.iter().any(|n| n == "browser");
            w.channel_settings = windows.iter().any(|n| n == "channel_settings");
            w.spectrum = windows.iter().any(|n| n == "spectrum");
            for spec in windows.iter().filter_map(|n| n.strip_prefix("effect:")) {
                if let Some((track, slot)) = spec.split_once(':') {
                    if let (Ok(track), Ok(slot)) = (track.parse::<usize>(), slot.parse::<usize>()) {
                        ui.effect_windows.push((track, slot.saturating_sub(1)));
                    }
                }
            }
            for name in windows.iter() {
                // `settings` opens Audio; `settings:<tab>` opens that tab.
                if name == "settings" {
                    ui.settings_tab = Some(musiccraft_ui::settings::Tab::Audio);
                } else if let Some(tab) = name.strip_prefix("settings:") {
                    ui.settings_tab = musiccraft_ui::settings::Tab::parse(tab);
                }
            }
            if windows.iter().any(|n| n == "automation") {
                ui.automation_editor = session.project().automations.first().map(|a| a.id);
            }
        }
        let control = match (options.control, &options.token_file) {
            (Some(port), Some(file)) => match control::Control::start(port, file, ctx.clone()) {
                Ok(control) => Some(control),
                Err(err) => {
                    status = Some(err);
                    None
                }
            },
            (Some(_), None) => {
                status = Some(
                    "--control needs --control-token-file; agents can't use this window".to_owned(),
                );
                None
            }
            _ => None,
        };
        if let Some(text) = status {
            ui.set_status(text, true);
        } else if control.is_some() {
            ui.set_status(
                "An agent can work in this window; you can keep working too.",
                false,
            );
        } else if ui.autosave_path.is_some() {
            ui.set_status(
                "MusicCraft did not close properly last time. File → Recover unsaved work restores it.",
                true,
            );
        } else {
            ui.set_status("Ready. Space plays, F5–F9 open windows.", false);
        }
        let mut app = Self {
            session,
            ui,
            audio: None,
            audio_label: String::new(),
            midi: None,
            midi_label: String::new(),
            midi_out_rx: None,
            midi_out: None,
            midi_out_label: String::new(),
            live: None,
            played_tx,
            settings,
            settings_info: SettingsInfo::default(),
            selected_channel,
            synced_revision: None,
            pending_offs: Vec::new(),
            title: String::new(),
            screenshot: screenshot::Request::from_env(),
            autosave: autosave::Autosave::new(revision),
            recorder: recorder::Recorder::default(),
            midi_notes: Some(played_rx),
            overview: None,
            clip_audio: Vec::new(),
            input: None,
            take: None,
            takes: 0,
            plugins: plugins::PluginGlue::new(musiccraft_cli::plugin_host()),
            control,
            saved_json,
            pending: None,
            closing: false,
            marker_pauses: 0,
            end_stops: 0,
            sweep: recorder::Sweep::default(),
            steps: recorder::StepEntry::default(),
        };
        app.refresh_devices();
        app.start_audio(ctx);
        app
    }

    /// Lists output devices, sample rates, MIDI ports and plugin problems.
    fn refresh_devices(&mut self) {
        let info = &mut self.settings_info;
        info.outputs = musiccraft_audio::output_device_names();
        info.inputs = musiccraft_audio::input_device_names();
        info.midi_ports = midi::port_names();
        info.midi_out_ports = midi_out::port_names();
        if let Some(host) = musiccraft_cli::plugin_host() {
            info.plugin_count = host.catalog().len();
            info.plugin_errors = host.scan_errors();
        }
    }

    /// (Re)opens the audio device from the settings, then MIDI. Running
    /// plugins store their state first, because they restart.
    fn start_audio(&mut self, ctx: &egui::Context) {
        if let Some(mut audio) = self.audio.take() {
            self.plugins
                .store_all(Some(&mut audio.bridge), &mut self.session);
            let _ = self.session.run(&Invocation::bare("transport.stop"));
            self.midi = None;
            self.midi_out = None;
            self.midi_out_rx = None;
            self.live = None;
            drop(audio);
        }
        let request = musiccraft_audio::OutputRequest {
            device: self.settings.audio_device.clone(),
            sample_rate: self.settings.sample_rate,
            buffer_frames: self.settings.buffer_frames,
        };
        let opened = OutputDevice::open(&request).and_then(|(device, note)| {
            let rate = device.sample_rate();
            let buffer = device
                .buffer_frames()
                .map_or_else(|| "default buffer".to_owned(), |f| format!("{f} frames"));
            let label = format!("{} · {rate} Hz · {buffer}", device.name());
            let rates = device.supported_rates();
            let (engine, mut handle) = engine_pair(rate);
            let live = handle.take_live_sender();
            let midi_out = handle.take_midi_out();
            let output = device.start(engine)?;
            Ok((output, handle, (live, midi_out), label, rates, rate, note))
        });
        match opened {
            Ok((output, handle, (live, midi_out), label, rates, rate, note)) => {
                self.audio_label = label;
                self.settings_info.sample_rates = rates;
                self.settings_info.running_rate = rate;
                self.settings_info.audio_note = note;
                self.live = live.map(|l| Arc::new(Mutex::new(l)));
                self.midi_out_rx = midi_out.map(|m| Arc::new(Mutex::new(m)));
                self.audio = Some(Audio {
                    _output: output,
                    meters: Arc::clone(handle.meters()),
                    handle,
                    bridge: Bridge::with_plugins(rate, musiccraft_cli::plugin_host()),
                });
                self.synced_revision = None;
                self.connect_midi(ctx);
                self.connect_midi_out();
                self.send(EngineMessage::SetMidiClock(self.settings.midi_clock));
            }
            Err(err) => {
                self.audio_label = format!("no audio: {err}");
                self.settings_info.audio_note = Some(err.to_string());
                self.midi_label = String::new();
            }
        }
    }

    /// Connects MIDI input as the settings say.
    fn connect_midi(&mut self, ctx: &egui::Context) {
        self.midi = None;
        let (Some(live), Some(audio)) = (&self.live, &self.audio) else {
            self.midi_label = "MIDI: unavailable".to_owned();
            return;
        };
        let port = match &self.settings.midi_input {
            MidiInput::Off => {
                self.midi_label = "MIDI: off".to_owned();
                return;
            }
            MidiInput::Auto => None,
            MidiInput::Port(name) => Some(name.as_str()),
        };
        match midi::connect(
            Arc::clone(live),
            Arc::clone(&self.selected_channel),
            Arc::clone(&audio.meters),
            self.played_tx.clone(),
            ctx.clone(),
            port,
        ) {
            Ok(midi) => {
                self.midi_label = format!("MIDI: {}", midi.name);
                self.midi = Some(midi);
            }
            Err(err) => self.midi_label = format!("MIDI: {err}"),
        }
    }

    /// Opens the MIDI output port the settings name, for MIDI Out channels.
    fn connect_midi_out(&mut self) {
        self.midi_out = None;
        let Some(port) = self.settings.midi_output.clone() else {
            self.midi_out_label = "MIDI out: off".to_owned();
            return;
        };
        let Some(receiver) = &self.midi_out_rx else {
            self.midi_out_label = "MIDI out: unavailable".to_owned();
            return;
        };
        match midi_out::start(Arc::clone(receiver), &port) {
            Ok(out) => {
                self.midi_out_label = format!("MIDI out: {}", out.name);
                self.midi_out = Some(out);
            }
            Err(err) => self.midi_out_label = format!("MIDI out: {err}"),
        }
    }

    /// Applies new settings, saving them and restarting what changed.
    fn apply_settings(&mut self, ctx: &egui::Context, next: musiccraft_ui::Settings) {
        let previous = std::mem::replace(&mut self.settings, next);
        if let Err(err) = settings::save(&self.settings) {
            self.ui
                .set_status(format!("cannot save settings: {err}"), true);
        }
        if (previous.scale() - self.settings.scale()).abs() > 1e-6 {
            ctx.set_zoom_factor(self.settings.scale());
        }
        if previous.plugin_folders != self.settings.plugin_folders {
            if let Some(host) = musiccraft_cli::plugin_host() {
                host.set_extra_folders(self.settings.plugin_folders.clone());
            }
            let bridge = self.audio.as_mut().map(|a| &mut a.bridge);
            let message = self.plugins.rescan(bridge);
            self.refresh_devices();
            self.ui.set_status(message, false);
        }
        if (
            &previous.fl_factory,
            &previous.fl_user,
            &previous.drive_map,
            &previous.browser_folders,
        ) != (
            &self.settings.fl_factory,
            &self.settings.fl_user,
            &self.settings.drive_map,
            &self.settings.browser_folders,
        ) {
            // Bridges take up the new translation on their next sync.
            musiccraft_paths::configure(self.settings.path_config());
        }
        if previous.audio_input != self.settings.audio_input {
            // The next take opens the newly chosen device.
            self.input = None;
        }
        if previous.audio_differs(&self.settings) {
            self.start_audio(ctx);
            self.ui
                .set_status(format!("audio: {}", self.audio_label), false);
        } else {
            if previous.midi_input != self.settings.midi_input {
                self.connect_midi(ctx);
                self.ui.set_status(self.midi_label.clone(), false);
            }
            if previous.midi_output != self.settings.midi_output {
                self.connect_midi_out();
                self.ui.set_status(self.midi_out_label.clone(), false);
            }
            if previous.midi_clock != self.settings.midi_clock {
                self.send(EngineMessage::SetMidiClock(self.settings.midi_clock));
            }
        }
    }

    fn engine_status(&mut self) -> EngineStatus {
        let tracks = self.session.project().mixer.len();
        let mut status = EngineStatus {
            audio: self.audio_label.clone(),
            midi: self.midi_label.clone(),
            midi_out: self.midi_out_label.clone(),
            ..EngineStatus::default()
        };
        if let Some(audio) = &self.audio {
            let meters = audio.handle.meters();
            status.position_ticks = meters.position_ticks();
            status.playing = meters.is_playing();
            status.counting_in = meters.is_counting_in();
            status.waiting = meters.is_waiting();
            status.voices = meters.active_voices();
            status.peaks = (0..tracks).map(|t| meters.take_peak(t)).collect();
            status.load = meters.load();
            status.pitches = pitch_readings(self.session.project(), meters);
            // Enough for the spectrum analyzer; the toolbar scope uses the end.
            let mut scope = vec![0.0; 2048];
            meters.scope(&mut scope);
            status.scope = scope;
        } else {
            status.peaks = vec![(0.0, 0.0); tracks];
        }
        status
    }

    fn send(&mut self, message: EngineMessage) {
        if let Some(audio) = &mut self.audio {
            let _ = audio.handle.send(message);
        }
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        let result = match action {
            Action::Run(invocation) => {
                if replaces_project(&invocation.name) && self.has_unsaved_changes() {
                    self.pending = Some(Pending::Run(invocation));
                    return;
                }
                let invocation = self.automated(invocation);
                self.run_invocation(&invocation)
            }
            Action::Coalesced(invocation, key) => {
                let invocation = self.automated(invocation);
                self.session.run_coalesced(&invocation, key)
            }
            Action::Preview { channel, key, on } => {
                let channel = u16::try_from(channel).unwrap_or(0);
                self.send(if on {
                    EngineMessage::NoteOn {
                        channel,
                        key,
                        velocity: 0.8,
                    }
                } else {
                    EngineMessage::NoteOff { channel, key }
                });
                return;
            }
            Action::Perform { channel, key, on } => {
                let channel16 = u16::try_from(channel).unwrap_or(0);
                self.send(if on {
                    EngineMessage::NoteOn {
                        channel: channel16,
                        key,
                        velocity: 0.8,
                    }
                } else {
                    EngineMessage::NoteOff {
                        channel: channel16,
                        key,
                    }
                });
                self.record(key, if on { 100 } else { 0 }, None);
                return;
            }
            Action::PreviewShort { channel, key } => {
                let channel = u16::try_from(channel).unwrap_or(0);
                self.send(EngineMessage::NoteOn {
                    channel,
                    key,
                    velocity: 0.8,
                });
                self.pending_offs
                    .push((Instant::now() + SHORT_PREVIEW, channel, key));
                return;
            }
            Action::PreviewSample(sample) => {
                let loaded = self
                    .audio
                    .as_mut()
                    .map(|audio| audio.bridge.sample(&sample, self.session.project()));
                match loaded {
                    Some(Ok(buffer)) => {
                        self.send(EngineMessage::PlayPreview(Box::new(
                            musiccraft_engine::Preview::new(buffer, 0.8),
                        )));
                    }
                    Some(Err(err)) => self.ui.set_status(format!("cannot preview: {err}"), true),
                    None => {}
                }
                return;
            }
            Action::DropAudio {
                sample,
                track,
                start,
            } => {
                let loaded = self
                    .audio
                    .as_mut()
                    .map(|audio| audio.bridge.sample(&sample, self.session.project()));
                let seconds = match loaded {
                    Some(Ok(buffer)) => buffer.duration_seconds(),
                    Some(Err(err)) => {
                        self.ui
                            .set_status(format!("cannot load {sample}: {err}"), true);
                        return;
                    }
                    None => 4.0 * 60.0 / self.session.project().tempo_bpm(),
                };
                let project = self.session.project();
                let ticks_per_second =
                    project.tempo_bpm() / 60.0 * f64::from(musiccraft_project::params::PPQ);
                let length = (seconds * ticks_per_second).ceil().max(1.0) as u32;
                let number = project.channels.len() + 1;
                let batch = Invocation::new(
                    musiccraft_commands::BATCH,
                    [
                        Invocation::new("channel.add_sampler", [sample.clone()]).to_string(),
                        format!("channel.set_param {number} one_shot 0"),
                        format!(
                            "playlist.add_audio_clip {number} {} {start}t {length}t",
                            track + 1
                        ),
                    ],
                );
                self.session.run(&batch)
            }
            Action::Seek(ticks) => {
                self.send(EngineMessage::Seek(ticks));
                return;
            }
            Action::ReloadSamples => {
                if let Some(audio) = &mut self.audio {
                    audio.bridge.reload_samples();
                    self.synced_revision = None;
                }
                self.ui.set_status("samples reloaded", false);
                return;
            }
            Action::PluginEditor { instance, open } => {
                let bridge = self.audio.as_mut().map(|a| &mut a.bridge);
                if let Err(err) =
                    self.plugins
                        .set_editor(bridge, self.session.project(), instance, open)
                {
                    self.ui.set_status(format!("plugin window: {err}"), true);
                }
                return;
            }
            Action::Settings(next) => {
                self.apply_settings(ctx, *next);
                return;
            }
            Action::RefreshDevices => {
                self.refresh_devices();
                self.ui.set_status("devices refreshed", false);
                return;
            }
            Action::RestartAudio => {
                self.start_audio(ctx);
                self.ui
                    .set_status(format!("audio: {}", self.audio_label), false);
                return;
            }
            Action::RescanPlugins => {
                let bridge = self.audio.as_mut().map(|a| &mut a.bridge);
                let message = self.plugins.rescan(bridge);
                self.synced_revision = None;
                self.ui.set_status(message, false);
                return;
            }
            Action::Quit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                return;
            }
        };
        match result {
            Ok(outcome) => self.ui.set_status(outcome.message, false),
            Err(err) => self.ui.set_status(err.to_string(), true),
        }
    }

    /// While recording, a control change also records its value at the
    /// playhead: as song automation in song mode, into the selected pattern's
    /// automation in pattern mode (one batch, so one gesture is one undo
    /// step).
    fn automated(&self, invocation: Invocation) -> Invocation {
        use musiccraft_commands::automation_record::{with_automation_record, with_event_record};
        let project = self.session.project();
        let Some(audio) = &self.audio else {
            return invocation;
        };
        let meters = audio.handle.meters();
        if !project.is_armed() || !meters.is_playing() {
            return invocation;
        }
        let tick = meters.position_ticks().max(0.0) as u32;
        let recorded = match project.play_mode {
            musiccraft_project::PlayMode::Song => with_automation_record(&invocation, tick),
            musiccraft_project::PlayMode::Pattern => with_event_record(&invocation, tick),
        };
        recorded.unwrap_or(invocation)
    }

    /// Feeds a live note into the recorder (or step edit); velocity 0 means
    /// note off. `position` is the engine position when the note happened,
    /// if known.
    fn record(&mut self, key: u8, velocity: u8, position: Option<f64>) {
        let Some(audio) = &self.audio else {
            return;
        };
        let meters = audio.handle.meters();
        // Waiting for input counts as playing: the first note starts it.
        let playing = meters.is_playing() || meters.is_waiting();
        let counting_in = meters.is_counting_in();
        let position = position.unwrap_or_else(|| meters.position_ticks());
        if self.session.project().step_edit() && !playing && !counting_in {
            self.step_edit(key, velocity, position);
            return;
        }
        self.steps.clear();
        if !self.session.project().is_armed() || !playing {
            self.recorder.clear();
            return;
        }
        if velocity > 0 {
            self.recorder.note_on(key, velocity, position);
            return;
        }
        let project = self.session.project();
        let pattern_ticks = project.current_pattern().length_ticks();
        let loop_ticks = match project.play_mode {
            musiccraft_project::PlayMode::Pattern => pattern_ticks,
            musiccraft_project::PlayMode::Song => project.song_length_ticks(),
        };
        let Some(note) = self
            .recorder
            .note_off(key, position, loop_ticks, pattern_ticks)
        else {
            return;
        };
        let channel = self.ui.selected_channel;
        if channel >= project.channels.len() {
            return;
        }
        let invocation = Invocation::new(
            "note.add",
            [
                (channel + 1).to_string(),
                format!("{}t", note.start),
                format!("{}t", note.length),
                note.key.to_string(),
                note.velocity.to_string(),
            ],
        );
        match self.session.run(&invocation) {
            Ok(outcome) => {
                self.sweep.recorded(note.start, note.key);
                self.ui
                    .set_status(format!("recorded: {}", outcome.message), false);
            }
            Err(err) => self.ui.set_status(err.to_string(), true),
        }
    }

    /// Step edit: enters a played note at the playhead (snapped), and moves
    /// the playhead one snap step on once every key is up.
    fn step_edit(&mut self, key: u8, velocity: u8, position: f64) {
        let meter = self.session.project().playing_meter();
        let snap_at =
            |ui: &UiState, at: f64| f64::from(ui.step_edit_ticks(at as u32, &meter).max(1));
        if velocity == 0 {
            if let Some(cursor) = self.steps.note_off(key) {
                let snap = snap_at(&self.ui, cursor);
                self.send(EngineMessage::Seek(cursor + snap));
            }
            return;
        }
        let here = f64::from(self.ui.step_edit_floor(position.max(0.0), &meter));
        let cursor = self.steps.note_on(key, here);
        let snap = snap_at(&self.ui, cursor);
        let project = self.session.project();
        let channel = self.ui.selected_channel;
        if channel >= project.channels.len() {
            return;
        }
        let pattern_ticks = project.current_pattern().length_ticks().max(1);
        let start = (cursor as u32) % pattern_ticks;
        let invocation = Invocation::new(
            "note.add",
            [
                (channel + 1).to_string(),
                format!("{start}t"),
                format!("{snap}t"),
                key.to_string(),
                velocity.clamp(1, 127).to_string(),
            ],
        );
        match self.session.run(&invocation) {
            Ok(outcome) => self
                .ui
                .set_status(format!("step: {}", outcome.message), false),
            Err(err) => self.ui.set_status(err.to_string(), true),
        }
    }

    /// Replace recording: removes the recording channel's notes the playhead
    /// has just passed in the recording pattern, unless this take recorded
    /// them.
    fn sweep_replace(&mut self) {
        let Some(audio) = &self.audio else {
            return;
        };
        let meters = audio.handle.meters();
        let project = self.session.project();
        let replacing = project.is_armed()
            && meters.is_playing()
            && project.record.mode == musiccraft_project::RecordMode::Replace;
        let channel = self.ui.selected_channel;
        let Some(id) = project
            .channels
            .get(channel)
            .map(|c| c.id)
            .filter(|_| replacing)
        else {
            self.sweep.reset();
            return;
        };
        let pattern = project.current_pattern();
        let ranges = self
            .sweep
            .advance(meters.position_ticks(), pattern.length_ticks());
        let removals: Vec<String> = pattern
            .notes_of(id)
            .filter(|n| self.sweep.removes(&ranges, n.start, n.key))
            .map(|n| {
                format!(
                    "note.remove {} {}t {} @{}",
                    channel + 1,
                    n.start,
                    n.key,
                    project.current_pattern + 1
                )
            })
            .collect();
        if removals.is_empty() {
            return;
        }
        let batch = Invocation::new(musiccraft_commands::BATCH, removals);
        if let Err(err) = self.session.run(&batch) {
            self.ui.set_status(err.to_string(), true);
        }
    }

    /// Recording without looping stopped the engine at the loop end: stop the
    /// transport to match.
    fn follow_end_stop(&mut self) {
        let Some(audio) = &self.audio else {
            return;
        };
        let count = audio.handle.meters().end_stops();
        if count <= self.end_stops {
            self.end_stops = count;
            return;
        }
        self.end_stops = count;
        if self.session.project().transport().is_playing() {
            match self.session.run(&Invocation::bare("transport.stop")) {
                Ok(_) => self.ui.set_status("recording reached the end", false),
                Err(err) => self.ui.set_status(err.to_string(), true),
            }
        }
    }

    /// Keeps the waveform overview in step with the selected channel's sample.
    fn update_overview(&mut self) {
        let project = self.session.project();
        let sample = match project
            .channels
            .get(self.ui.selected_channel)
            .map(|c| &c.instrument)
        {
            Some(
                musiccraft_project::Instrument::Sampler { sample }
                | musiccraft_project::Instrument::Granular { sample }
                | musiccraft_project::Instrument::Resynth { sample },
            ) => sample.clone(),
            _ => {
                self.overview = None;
                return;
            }
        };
        if self
            .overview
            .as_ref()
            .is_some_and(|(key, _, _)| *key == sample)
        {
            return;
        }
        let Some(audio) = &mut self.audio else {
            return;
        };
        let (points, seconds) = match audio.bridge.sample(&sample, self.session.project()) {
            Ok(buffer) => (overview(&buffer, 400), buffer.duration_seconds()),
            Err(_) => (Vec::new(), 0.0),
        };
        self.overview = Some((sample, points, seconds));
    }

    /// Keeps waveform data for every channel that audio clips play.
    fn update_clip_audio(&mut self) {
        let project = self.session.project();
        let mut wanted: Vec<(musiccraft_project::ChannelId, String)> = project
            .playlist
            .clips
            .iter()
            .filter_map(|c| c.audio())
            .filter_map(|id| {
                let channel = project.channels.get(project.channel_position(id)?)?;
                match &channel.instrument {
                    musiccraft_project::Instrument::Sampler { sample } => {
                        Some((id, sample.clone()))
                    }
                    _ => None,
                }
            })
            .collect();
        wanted.sort();
        wanted.dedup();
        let current: Vec<(musiccraft_project::ChannelId, String)> = self
            .clip_audio
            .iter()
            .map(|c| (c.channel, c.sample.clone()))
            .collect();
        if current == wanted {
            return;
        }
        let Some(audio) = &mut self.audio else {
            return;
        };
        let mut fresh = Vec::new();
        for (id, sample) in wanted {
            if let Some(existing) = self
                .clip_audio
                .iter()
                .find(|c| c.channel == id && c.sample == sample)
            {
                fresh.push(existing.clone());
            } else if let Ok(buffer) = audio.bridge.sample(&sample, self.session.project()) {
                fresh.push(ClipAudio {
                    channel: id,
                    peaks: overview(&buffer, 600),
                    seconds: buffer.duration_seconds(),
                    sample,
                });
            }
        }
        self.clip_audio = fresh;
    }

    /// Starts, feeds and finishes audio takes as recording turns on and off.
    fn record_audio(&mut self) {
        let Some(audio) = &self.audio else {
            return;
        };
        let meters = audio.handle.meters();
        let song = self.session.project().play_mode == musiccraft_project::PlayMode::Song;
        let armed = self.session.project().is_armed();
        let wanted = armed && self.ui.record_audio && meters.is_playing() && song;
        if wanted && self.take.is_none() {
            if self.input.is_none() {
                match musiccraft_audio::AudioInput::open(self.settings.audio_input.as_deref()) {
                    Ok(input) => self.input = Some(input),
                    Err(err) => {
                        self.ui.record_audio = false;
                        self.ui.set_status(format!("no audio input: {err}"), true);
                        return;
                    }
                }
            }
            if let Some(input) = &mut self.input {
                input.clear();
                self.take = Some(take::Take {
                    start: meters.position_ticks(),
                    samples: Vec::new(),
                });
                self.ui
                    .set_status(format!("recording from {}", input.name()), false);
            }
        }
        if let (Some(take), Some(input)) = (&mut self.take, &mut self.input) {
            input.drain(&mut take.samples);
        }
        if !wanted {
            if let (Some(take), Some(input)) = (self.take.take(), &self.input) {
                self.takes += 1;
                let name = format!(
                    "take-{}-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs()),
                    self.takes
                );
                let finished = take::finish(
                    take,
                    input.channels(),
                    input.sample_rate(),
                    self.session.project(),
                    &name,
                );
                match finished {
                    Ok(Some(invocation)) => match self.session.run(&invocation) {
                        Ok(_) => self.ui.set_status(format!("recorded {name}"), false),
                        Err(err) => self.ui.set_status(err.to_string(), true),
                    },
                    Ok(None) => {}
                    Err(err) => self.ui.set_status(format!("recording failed: {err}"), true),
                }
            }
        }
    }

    fn drain_midi(&mut self) {
        let notes: Vec<midi::Played> = self
            .midi_notes
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for (key, velocity, position) in notes {
            self.record(key, velocity, Some(position));
        }
    }

    fn sync(&mut self) {
        let revision = self.session.revision();
        if self.synced_revision == Some(revision) {
            if let Some(audio) = &mut self.audio {
                audio.handle.collect_garbage();
            }
            return;
        }
        if let Some(audio) = &mut self.audio {
            let report = audio.bridge.sync(self.session.project(), &mut audio.handle);
            if let Some(warning) = report.warnings.first() {
                self.ui.set_status(warning.clone(), true);
            }
            if report.dropped > 0 {
                // Queue full: try again next frame.
                return;
            }
        }
        self.synced_revision = Some(revision);
    }

    fn release_due_previews(&mut self) {
        let now = Instant::now();
        let (due, waiting): (Vec<_>, Vec<_>) =
            self.pending_offs.drain(..).partition(|(t, _, _)| *t <= now);
        self.pending_offs = waiting;
        for (_, channel, key) in due {
            self.send(EngineMessage::NoteOff { channel, key });
        }
    }

    /// Runs a command from the interface, keeping track of whether the project
    /// still matches what was last saved.
    fn run_invocation(
        &mut self,
        invocation: &Invocation,
    ) -> Result<musiccraft_commands::Outcome, musiccraft_commands::CommandError> {
        let bridge = self.audio.as_mut().map(|a| &mut a.bridge);
        self.plugins.before(invocation, bridge, &mut self.session);
        let result = self.session.run(invocation);
        if result.is_ok() && marks_saved(&invocation.name) {
            self.saved_json = self.session.project().to_json().ok();
        }
        result
    }

    /// Live mode: answers what agents sent since the last frame, against this
    /// window's session.
    fn serve_control(&mut self) {
        let Some(control) = &self.control else {
            return;
        };
        for request in control.pending() {
            let reply =
                musiccraft_mcp::Server::with_host(LiveHost(self)).handle_line(&request.line);
            let _ = request.reply.send(reply);
        }
    }

    /// Whether the project differs from what was last saved, loaded or created.
    fn has_unsaved_changes(&self) -> bool {
        self.session.project().to_json().ok() != self.saved_json
    }

    /// Asks before the window closes on unsaved changes; on a clean close the
    /// autosave is no longer needed.
    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if self.closing || !self.has_unsaved_changes() {
            self.closing = true;
            autosave::discard();
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.pending = Some(Pending::Close);
    }

    /// The "save changes first?" window, while an action is waiting on it.
    fn show_unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending else {
            return;
        };
        let before = match pending {
            Pending::Close => "closing",
            Pending::Run(_) => "continuing",
        };
        let name = self.session.project().name().to_owned();
        let mut choice = None;
        egui::Window::new("Unsaved changes")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(format!(
                    "Save the changes to \u{201c}{name}\u{201d} before {before}?"
                ));
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        choice = Some(UnsavedChoice::Save);
                    }
                    if ui.button("Don't save").clicked() {
                        choice = Some(UnsavedChoice::Discard);
                    }
                    if ui.button("Cancel").clicked() {
                        choice = Some(UnsavedChoice::Cancel);
                    }
                });
            });
        let Some(choice) = choice else {
            return;
        };
        let Some(pending) = self.pending.take() else {
            return;
        };
        match choice {
            UnsavedChoice::Cancel => {}
            UnsavedChoice::Discard => self.proceed(ctx, pending),
            UnsavedChoice::Save => {
                if self.session.project().path().is_none() {
                    // Never saved: ask where; the user repeats the action after.
                    self.ui.open_save_as(self.session.project());
                    return;
                }
                match self.run_invocation(&Invocation::bare("project.save")) {
                    Ok(outcome) => {
                        self.ui.set_status(outcome.message, false);
                        self.proceed(ctx, pending);
                    }
                    Err(err) => self.ui.set_status(err.to_string(), true),
                }
            }
        }
    }

    /// Carries out an action once unsaved changes are saved or discarded.
    fn proceed(&mut self, ctx: &egui::Context, pending: Pending) {
        match pending {
            Pending::Close => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Pending::Run(invocation) => match self.run_invocation(&invocation) {
                Ok(outcome) => self.ui.set_status(outcome.message, false),
                Err(err) => self.ui.set_status(err.to_string(), true),
            },
        }
    }

    /// Song playback stopped at a pause marker: the engine already holds the
    /// playhead there, so pause the transport to match. Play continues from
    /// the marker; Stop rewinds.
    fn follow_marker_pause(&mut self) {
        let Some(audio) = &self.audio else {
            return;
        };
        let (count, _) = audio.handle.meters().marker_pause();
        if count <= self.marker_pauses {
            // Nothing new (or a restarted engine counting from zero again).
            self.marker_pauses = count;
            return;
        }
        self.marker_pauses = count;
        if !self.session.project().transport().is_playing() {
            return;
        }
        match self.session.run(&Invocation::bare("transport.pause")) {
            Ok(_) => self
                .ui
                .set_status("paused at a marker; press Space to continue", false),
            Err(err) => self.ui.set_status(err.to_string(), true),
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let project = self.session.project();
        let title = format!("MusicCraft — {}", project.name());
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_close_request(&ctx);
        self.show_unsaved_dialog(&ctx);
        let engine = self.engine_status();
        let history = History {
            undo: self.session.undo_label().map(ToString::to_string),
            can_redo: self.session.redo_depth() > 0,
        };
        self.update_overview();
        self.update_clip_audio();
        let ticks_per_second =
            self.session.project().tempo_bpm() / 60.0 * f64::from(musiccraft_project::params::PPQ);
        let clip_waveforms: Vec<(musiccraft_project::ChannelId, Vec<(f32, f32)>)> = self
            .clip_audio
            .iter()
            .map(|c| (c.channel, c.peaks.clone()))
            .collect();
        let clip_durations: Vec<(musiccraft_project::ChannelId, u32)> = self
            .clip_audio
            .iter()
            .map(|c| (c.channel, (c.seconds * ticks_per_second).ceil() as u32))
            .collect();
        let plugins = {
            let bridge = self.audio.as_mut().map(|a| &mut a.bridge);
            self.plugins.view(bridge, self.session.project(), &self.ui)
        };
        let actions = {
            let frame = Frame {
                project: self.session.project(),
                engine: &engine,
                history: &history,
                waveform: self
                    .overview
                    .as_ref()
                    .map(|(_, points, _)| points.as_slice()),
                waveform_seconds: self.overview.as_ref().map(|(_, _, seconds)| *seconds),
                clip_waveforms: &clip_waveforms,
                clip_durations: &clip_durations,
                plugins: &plugins,
                settings: &self.settings,
                settings_info: &self.settings_info,
            };
            musiccraft_ui::show(ui, &mut self.ui, &frame)
        };
        for action in actions {
            self.apply(&ctx, action);
        }
        let editor_open = {
            let bridge = self.audio.as_mut().map(|a| &mut a.bridge);
            self.plugins.poll(bridge, &mut self.session)
        };
        self.serve_control();
        self.drain_midi();
        self.record_audio();
        self.selected_channel
            .store(self.ui.selected_channel, Ordering::Relaxed);
        self.follow_marker_pause();
        self.follow_end_stop();
        self.sweep_replace();
        self.sync();
        self.release_due_previews();
        self.update_title(&ctx);
        let autosaved = if self.settings.autosave {
            self.autosave
                .tick(self.session.project(), self.session.revision())
        } else {
            None
        };
        match autosaved {
            Some(Ok(message)) => {
                if !self.ui.status.error {
                    self.ui.set_status(message, false);
                }
            }
            Some(Err(err)) => self.ui.set_status(format!("autosave failed: {err}"), true),
            None => {}
        }
        let busy = engine.playing
            || !self.pending_offs.is_empty()
            || engine.voices > 0
            || self.take.is_some()
            || editor_open;
        let meters_moving = self.ui.meters.iter().any(|(l, r)| *l > 1e-4 || *r > 1e-4);
        if busy || meters_moving {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        let _ = &self.midi;
        if let Some(request) = &mut self.screenshot {
            request.tick(&ctx);
        }
    }
}

/// The window's session as an MCP server's host: commands run through the
/// interface's own path, and never discard unsaved work.
struct LiveHost<'a>(&'a mut App);

impl musiccraft_mcp::Host for LiveHost<'_> {
    fn session(&self) -> &Session {
        &self.0.session
    }

    fn run(
        &mut self,
        invocation: &Invocation,
    ) -> Result<musiccraft_commands::Outcome, musiccraft_commands::CommandError> {
        let replaces =
            replaces_project(&invocation.name) || invocation.name == "project.import_flp";
        if replaces && self.0.has_unsaved_changes() {
            return Err(musiccraft_commands::CommandError::Io(format!(
                "{} would discard the window's unsaved changes: save them first (project.save) or ask the user",
                invocation.name
            )));
        }
        let result = self.0.run_invocation(invocation);
        match &result {
            Ok(outcome) => self
                .0
                .ui
                .set_status(format!("Agent: {}", outcome.message), false),
            Err(err) => self.0.ui.set_status(format!("Agent: {err}"), true),
        }
        result
    }

    fn live(&self) -> bool {
        true
    }
}

fn main() -> eframe::Result {
    let options = match parse_args() {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("MusicCraft")
            // Reverse-DNS ID so Linux desktops match the window to its desktop entry
            // (the Flatpak and the installed .desktop file use the same ID).
            .with_app_id("io.github.bhimio1.MusicCraft"),
        ..Default::default()
    };
    eframe::run_native(
        "MusicCraft",
        native,
        Box::new(move |creation| {
            musiccraft_ui::apply_theme(&creation.egui_ctx);
            Ok(Box::new(App::new(&options, &creation.egui_ctx)))
        }),
    )
}
