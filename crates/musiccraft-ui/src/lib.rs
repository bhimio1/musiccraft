//! User interface. Contains no business logic: it renders project state and turns
//! input into [`Action`]s (command invocations, previews and UI-only actions)
//! for the app to run. Keyboard shortcuts come from a data-driven [`Keymap`].

pub mod keymap;

mod automation;
mod browser;
mod channel_rack;
mod channel_settings;
mod controllers;
mod dialogs;
mod effect_window;
mod geometry;
pub mod hint;
mod icons;
mod mixer;
mod piano_roll;
mod playlist;
mod plugins;
mod rack_view;
mod sample_editor;
pub mod settings;
mod signature;
mod spectrum;
mod theme;
mod toolbar;
mod tuner;
mod widgets;
mod zone_editor;

use musiccraft_commands::Invocation;
use musiccraft_project::Project;

pub use geometry::Snap;
pub use keymap::{Chord, DEFAULT_KEYMAP, Keymap};
pub use plugins::{LivePluginView, PluginEntry, PluginParamView, Plugins};
pub use settings::{MidiInput, Settings, SettingsInfo};
pub use theme::apply as apply_theme;

/// Something the user asked for, produced by one frame of UI.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Run a command.
    Run(Invocation),
    /// Run a command as part of a continuous gesture; runs with the same key
    /// merge into one undo step.
    Coalesced(Invocation, u64),
    /// Start or stop an audition note on a channel (0-based index).
    Preview {
        /// Channel index.
        channel: usize,
        /// MIDI key.
        key: u8,
        /// `true` for note on, `false` for note off.
        on: bool,
    },
    /// A live performance note on the selected channel (typing keyboard). The app
    /// records these into the pattern while recording.
    Perform {
        /// Channel index.
        channel: usize,
        /// MIDI key.
        key: u8,
        /// `true` for note on, `false` for note off.
        on: bool,
    },
    /// Play a short audition note that the app stops by itself.
    PreviewShort {
        /// Channel index.
        channel: usize,
        /// MIDI key.
        key: u8,
    },
    /// Move the playhead to a position in ticks.
    Seek(f64),
    /// Audition a sample (file path or `builtin:` name) on the master output.
    PreviewSample(String),
    /// A sample dropped on the playlist: add it as a channel and an audio clip
    /// at (track row, start tick). The app knows the sample's length.
    DropAudio {
        /// Sample reference.
        sample: String,
        /// Playlist row, 0-based.
        track: u32,
        /// Start in ticks.
        start: u32,
    },
    /// Drop decoded samples and reload them from disk.
    ReloadSamples,
    /// Open or close a running plugin's own window.
    PluginEditor {
        /// Plugin instance number.
        instance: u32,
        /// Whether the window should be open.
        open: bool,
    },
    /// Scan the plugin folders again.
    RescanPlugins,
    /// Apply and save new settings.
    Settings(Box<Settings>),
    /// List audio devices and MIDI ports again.
    RefreshDevices,
    /// Close and reopen the audio device.
    RestartAudio,
    /// Close the application.
    Quit,
}

/// Engine and device information shown by the UI. Read-only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EngineStatus {
    /// Playhead position in ticks within the current loop.
    pub position_ticks: f64,
    /// Whether the engine is playing.
    pub playing: bool,
    /// Peak levels (left, right) per mixer track since the last frame.
    pub peaks: Vec<(f32, f32)>,
    /// Sounding voices.
    pub voices: u32,
    /// Recent master output samples for the oscilloscope.
    pub scope: Vec<f32>,
    /// Audio processing load, 0..1 of the real-time budget.
    pub load: f32,
    /// Pitch (Hz) heard by each pitch corrector and tuner, as
    /// (mixer track, effect slot, hertz); missing when there is no clear pitch.
    pub pitches: Vec<(usize, usize, f32)>,
    /// Audio device description or error.
    pub audio: String,
    /// MIDI input description.
    pub midi: String,
    /// MIDI output description.
    pub midi_out: String,
    /// A recording count-in is clicking.
    pub counting_in: bool,
    /// Playback waits for the first live note.
    pub waiting: bool,
}

/// Undo history information for menus.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct History {
    /// The command `edit.undo` would reverse.
    pub undo: Option<String>,
    /// Whether `edit.redo` is possible.
    pub can_redo: bool,
}

/// Which floating windows are open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Windows {
    /// Channel rack.
    pub channel_rack: bool,
    /// Piano roll.
    pub piano_roll: bool,
    /// Playlist.
    pub playlist: bool,
    /// Mixer.
    pub mixer: bool,
    /// Browser side panel.
    pub browser: bool,
    /// Keyboard shortcut reference.
    pub shortcuts: bool,
    /// Instrument editor of the selected channel.
    pub channel_settings: bool,
    /// Spectrum analyzer of the master output.
    pub spectrum: bool,
}

impl Default for Windows {
    fn default() -> Self {
        Self {
            channel_rack: true,
            piano_roll: false,
            playlist: true,
            mixer: false,
            browser: true,
            shortcuts: false,
            channel_settings: false,
            spectrum: false,
        }
    }
}

/// A status line message.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusLine {
    /// Text to show.
    pub text: String,
    /// Whether it describes a failure.
    pub error: bool,
}

/// State that belongs to the interface, not to the project.
#[derive(Debug, Clone)]
pub struct UiState {
    /// Open windows.
    pub windows: Windows,
    /// Selected channel (0-based), shown in the piano roll.
    pub selected_channel: usize,
    /// Selected mixer track (0 = master).
    pub selected_track: usize,
    /// Effect slot shown in the mixer's effect editor (0-based).
    pub mixer_slot: usize,
    /// Keyboard shortcuts.
    pub keymap: Keymap,
    /// Last command result.
    pub status: StatusLine,
    /// Smoothed meter levels per mixer track.
    pub meters: Vec<(f32, f32)>,
    /// Location of the last autosave, offered by File → Recover autosave.
    pub autosave_path: Option<String>,
    /// Automation shown in the automation editor.
    pub automation_editor: Option<musiccraft_project::AutomationId>,
    automation_drag: Option<automation::PointDrag>,
    /// A segment tension being dragged in the automation editor.
    tension_drag: Option<automation::TensionDrag>,
    /// The segment (by its first point's tick) whose shape menu is open.
    automation_menu: Option<u32>,
    /// The automation editor's drawing area and length in ticks, last frame.
    pub(crate) automation_view: Option<(egui::Rect, u32)>,
    /// The area between the side and bottom panels, where windows float.
    pub workspace: egui::Rect,
    /// Letter keys play notes on the selected channel.
    pub typing_keyboard: bool,
    /// Times (seconds) of the latest tempo taps.
    pub taps: Vec<f64>,
    /// The custom time signature being entered in a signature menu.
    pub sig_draft: (u32, u32),
    /// The toolbar's position display shows minutes and seconds instead of
    /// bars and beats.
    pub show_seconds: bool,
    /// Text being typed in Settings → Folders: FL factory and user data
    /// folders, and a drive mapping's prefix and folder.
    pub path_drafts: [String; 4],
    /// The channel settings window's page.
    pub channel_tab: channel_settings::ChannelTab,
    /// The channel rack shows only this display filter group (`None`: all).
    pub rack_group: Option<String>,
    /// The channel rack's step graph editor: open, what it edits, and the
    /// step being dragged.
    pub(crate) graph: Option<piano_roll::LaneMode>,
    pub(crate) graph_drag: Option<musiccraft_project::Note>,
    /// Where the graph strip was last drawn and its step count (for tests).
    pub(crate) graph_view: Option<(egui::Rect, u32)>,
    /// A drag across step buttons in progress.
    pub(crate) step_paint: Option<channel_rack::StepStroke>,
    /// Where each visible channel's first two step buttons were drawn, for
    /// tests.
    pub(crate) step_layout: Vec<(usize, egui::Rect)>,
    /// While recording in song mode, also capture the audio input.
    pub record_audio: bool,
    /// Text that filters the plugin parameter list.
    pub plugin_filter: String,
    /// Hint of the control hovered last frame.
    pub(crate) hint: Option<String>,
    /// Smoothed spectrum analyzer levels and their falling peaks (dB per bin).
    pub(crate) spectrum: (Vec<f32>, Vec<f32>),
    /// Open effect windows: (mixer track, slot index).
    pub effect_windows: Vec<(usize, usize)>,
    /// The sample editor's state.
    pub sample_edit: sample_editor::EditorState,
    pub(crate) effect_drag: Option<effect_window::Drag>,
    /// Where the last effect graph was drawn (tests).
    pub(crate) effect_graph: Option<egui::Rect>,
    /// Open settings tab; `None` when the settings window is closed.
    pub settings_tab: Option<settings::Tab>,
    /// Project name being edited in settings.
    pub(crate) project_name: String,
    /// Interface scale while its slider is dragged, in percent.
    pub(crate) scale_draft: Option<f32>,
    /// Where the plugin panel drew its controls last frame: the window
    /// button and (parameter id, slider) pairs.
    pub(crate) plugin_layout: (Option<egui::Rect>, Vec<(u32, egui::Rect)>),
    gesture: u64,
    browser: browser::BrowserState,
    roll: piano_roll::RollState,
    playlist: playlist::PlaylistState,
    dialog: Option<dialogs::Dialog>,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new(Keymap::default_keymap())
    }
}

impl UiState {
    /// How far step edit moves per note from `tick` of `meter`: the piano
    /// roll's snap, or one step when snap is off.
    #[must_use]
    pub fn step_edit_ticks(&self, tick: u32, meter: &musiccraft_project::Meter) -> u32 {
        match self.roll.snap {
            geometry::Snap::Off => musiccraft_project::params::TICKS_PER_STEP,
            snap => snap.ticks_at(tick, meter),
        }
    }

    /// The step edit grid line at or before `tick` of `meter`.
    #[must_use]
    pub fn step_edit_floor(&self, tick: f64, meter: &musiccraft_project::Meter) -> u32 {
        match self.roll.snap {
            geometry::Snap::Off => geometry::Snap::Step.floor(tick as f32, meter),
            snap => snap.floor(tick as f32, meter),
        }
    }

    /// Fresh interface state using `keymap`.
    #[must_use]
    pub fn new(keymap: Keymap) -> Self {
        Self {
            windows: Windows::default(),
            selected_channel: 0,
            selected_track: 0,
            mixer_slot: 0,
            keymap,
            status: StatusLine::default(),
            meters: Vec::new(),
            autosave_path: None,
            automation_editor: None,
            automation_drag: None,
            tension_drag: None,
            automation_menu: None,
            automation_view: None,
            workspace: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0)),
            typing_keyboard: false,
            taps: Vec::new(),
            sig_draft: (4, 4),
            channel_tab: channel_settings::ChannelTab::default(),
            show_seconds: false,
            path_drafts: Default::default(),
            rack_group: None,
            graph: None,
            graph_drag: None,
            graph_view: None,
            step_paint: None,
            step_layout: Vec::new(),
            record_audio: false,
            plugin_filter: String::new(),
            hint: None,
            effect_windows: Vec::new(),
            sample_edit: sample_editor::EditorState::default(),
            spectrum: (Vec::new(), Vec::new()),
            effect_drag: None,
            effect_graph: None,
            settings_tab: None,
            project_name: String::new(),
            scale_draft: None,
            plugin_layout: (None, Vec::new()),
            gesture: 0,
            browser: browser::BrowserState::default(),
            roll: piano_roll::RollState::default(),
            playlist: playlist::PlaylistState::default(),
            dialog: None,
        }
    }

    /// Starts a new continuous gesture and returns its key.
    pub(crate) fn begin_gesture(&mut self) -> u64 {
        self.gesture += 1;
        self.gesture
    }

    /// The key of the gesture in progress.
    pub(crate) fn gesture(&self) -> u64 {
        self.gesture
    }

    /// Opens the Save As dialog for `project`.
    pub fn open_save_as(&mut self, project: &Project) {
        self.dialog = Some(dialogs::Dialog::save_as(project));
    }

    /// Records a command result for the status line.
    pub fn set_status(&mut self, text: impl Into<String>, error: bool) {
        self.status = StatusLine {
            text: text.into(),
            error,
        };
    }

    fn update_meters(&mut self, peaks: &[(f32, f32)], dt: f32) {
        self.meters.resize(peaks.len(), (0.0, 0.0));
        // Peaks jump up and fall at about 24 dB per second.
        let fall = 10.0_f32.powf(-1.2 * dt);
        for (meter, peak) in self.meters.iter_mut().zip(peaks) {
            meter.0 = peak.0.max(meter.0 * fall);
            meter.1 = peak.1.max(meter.1 * fall);
        }
    }
}

/// Inputs to one frame of UI.
pub struct Frame<'a> {
    /// The project to show.
    pub project: &'a Project,
    /// Engine status.
    pub engine: &'a EngineStatus,
    /// Undo information.
    pub history: &'a History,
    /// Min/max overview of the selected channel's sample, if it is a sampler.
    pub waveform: Option<&'a [(f32, f32)]>,
    /// Length of that sample in seconds.
    pub waveform_seconds: Option<f64>,
    /// Overviews of samples played by audio clips.
    pub clip_waveforms: &'a [(musiccraft_project::ChannelId, Vec<(f32, f32)>)],
    /// Durations in ticks of samples played by audio clips.
    pub clip_durations: &'a [(musiccraft_project::ChannelId, u32)],
    /// Installed and running plugins.
    pub plugins: &'a Plugins,
    /// User settings.
    pub settings: &'a Settings,
    /// Devices and status for the settings window.
    pub settings_info: &'a SettingsInfo,
}

/// Draws the whole interface and returns what the user asked for.
pub fn show(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>) -> Vec<Action> {
    let mut actions = Vec::new();
    let dt = ui.ctx().input(|i| i.stable_dt).min(0.1);
    state.update_meters(&frame.engine.peaks, dt);
    let channels = frame.project.channels.len();
    if state.selected_channel >= channels {
        state.selected_channel = channels.saturating_sub(1);
    }
    if state.selected_track >= frame.project.mixer.len() {
        state.selected_track = 0;
    }

    state.hint = hint::begin_frame(ui.ctx(), frame.settings.show_hints);
    toolbar::show(ui, state, frame, &mut actions);
    if state.windows.browser {
        browser::show(ui, state, frame, &mut actions);
    }
    let workspace = egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::BACKGROUND).inner_margin(6.0))
        .show(ui, |ui| {
            if state.windows.playlist {
                playlist::show(ui, state, frame, &mut actions);
            } else {
                theme::watermark(ui);
            }
        })
        .response
        .rect;
    state.workspace = workspace;

    let ctx = ui.ctx().clone();
    if state.windows.mixer {
        mixer::show(&ctx, state, frame, &mut actions);
    }
    channel_rack::show(&ctx, state, frame, &mut actions);
    piano_roll::show(&ctx, state, frame, &mut actions);
    channel_settings::show(&ctx, state, frame, &mut actions);
    automation::show(&ctx, state, frame, &mut actions);
    dialogs::show(&ctx, state, frame, &mut actions);
    dialogs::shortcuts(&ctx, state);
    effect_window::show_all(&ctx, state, frame, &mut actions);
    spectrum::show(&ctx, state, frame);
    settings::show(&ctx, state, frame, &mut actions);

    if let Some(dragged) = egui::DragAndDrop::payload::<browser::Dragged>(&ctx) {
        if let Some(pos) = ctx.pointer_hover_pos() {
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Tooltip,
                egui::Id::new("drag-ghost"),
            ));
            let galley = painter.layout_no_wrap(
                format!("♪ {}", dragged.label()),
                egui::FontId::proportional(13.0),
                theme::BACKGROUND,
            );
            let rect =
                egui::Rect::from_min_size(pos + egui::vec2(12.0, 8.0), galley.size()).expand(5.0);
            painter.rect_filled(rect, 4.0, theme::ACCENT);
            painter.galley(rect.min + egui::vec2(5.0, 5.0), galley, theme::BACKGROUND);
        }
    }
    handle_keys(&ctx, state, frame, &mut actions);
    resolve_ui_actions(state, frame, actions)
}

/// Typing-keyboard layout: (key, semitones above C4). The bottom letter row is
/// one octave, the top row the next, with the number row as black keys.
const TYPING_KEYS: [(egui::Key, u8); 29] = {
    use egui::Key::*;
    [
        (Z, 0),
        (S, 1),
        (X, 2),
        (D, 3),
        (C, 4),
        (V, 5),
        (G, 6),
        (B, 7),
        (H, 8),
        (N, 9),
        (J, 10),
        (M, 11),
        (Comma, 12),
        (Q, 12),
        (Num2, 13),
        (W, 14),
        (Num3, 15),
        (E, 16),
        (R, 17),
        (Num5, 18),
        (T, 19),
        (Num6, 20),
        (Y, 21),
        (Num7, 22),
        (U, 23),
        (I, 24),
        (Num9, 25),
        (O, 26),
        (P, 28),
    ]
};
/// Key that the first typing-keyboard note (Z) plays: C4.
const TYPING_BASE: u8 = 48;

fn typing_note(key: egui::Key, modifiers: egui::Modifiers) -> Option<u8> {
    if modifiers.any() && !modifiers.shift_only() {
        return None;
    }
    TYPING_KEYS
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, offset)| TYPING_BASE + offset)
}

fn handle_keys(ctx: &egui::Context, state: &UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let events: Vec<(egui::Key, bool, bool, egui::Modifiers)> = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed,
                    repeat,
                    modifiers,
                    ..
                } => Some((*key, *pressed, *repeat, *modifiers)),
                _ => None,
            })
            .collect()
    });
    let channel_exists = state.selected_channel < frame.project.channels.len();
    for (key, pressed, repeat, modifiers) in events {
        if state.typing_keyboard && channel_exists {
            if let Some(note) = typing_note(key, modifiers) {
                if !repeat {
                    actions.push(Action::Perform {
                        channel: state.selected_channel,
                        key: note,
                        on: pressed,
                    });
                }
                continue;
            }
        }
        if !pressed || repeat {
            continue;
        }
        if let Some(invocation) = state.keymap.lookup(&Chord::from_event(key, modifiers)) {
            actions.push(Action::Run(invocation.clone()));
        }
    }
}

/// Handles `view.*` and `ui.*` invocations here; passes the rest through.
fn resolve_ui_actions(state: &mut UiState, frame: &Frame<'_>, actions: Vec<Action>) -> Vec<Action> {
    let mut out = Vec::with_capacity(actions.len());
    for action in actions {
        let Action::Run(invocation) = &action else {
            out.push(action);
            continue;
        };
        match invocation.name.as_str() {
            "view.toggle" => {
                let windows = &mut state.windows;
                let flag = match invocation.args.first().map(String::as_str) {
                    Some("playlist") => Some(&mut windows.playlist),
                    Some("channel_rack") => Some(&mut windows.channel_rack),
                    Some("piano_roll") => Some(&mut windows.piano_roll),
                    Some("mixer") => Some(&mut windows.mixer),
                    Some("browser") => Some(&mut windows.browser),
                    Some("shortcuts") => Some(&mut windows.shortcuts),
                    Some("spectrum") => Some(&mut windows.spectrum),
                    _ => None,
                };
                if let Some(flag) = flag {
                    *flag = !*flag;
                }
            }
            "ui.new" => out.push(Action::Run(Invocation::bare("project.new"))),
            "ui.open" => state.dialog = Some(dialogs::Dialog::open(frame.project)),
            "ui.save" => match frame.project.path() {
                Some(_) => out.push(Action::Run(Invocation::bare("project.save"))),
                None => state.dialog = Some(dialogs::Dialog::save_as(frame.project)),
            },
            "ui.save_as" => state.dialog = Some(dialogs::Dialog::save_as(frame.project)),
            "ui.save_new_version" => match frame.project.path() {
                Some(_) => out.push(Action::Run(Invocation::bare("project.save_new_version"))),
                None => state.dialog = Some(dialogs::Dialog::save_as(frame.project)),
            },
            "ui.add_marker" => {
                // A marker at the playhead, on the beat.
                let tick = frame.engine.position_ticks.max(0.0) as u32;
                let tick = tick - tick % musiccraft_project::params::PPQ;
                state.dialog = Some(dialogs::Dialog::add_marker(tick));
            }
            "ui.tool" => {
                // One tool choice for the piano roll and the playlist.
                match invocation
                    .args
                    .first()
                    .and_then(|t| piano_roll::Tool::parse(t))
                {
                    Some(tool) => {
                        state.roll.tool = tool;
                        state.playlist.tool = tool;
                    }
                    None => state.set_status("ui.tool: unknown tool", true),
                }
            }
            "ui.toggle_slide" => state.roll.slide = !state.roll.slide,
            "ui.toggle_porta" => state.roll.porta = !state.roll.porta,
            "ui.insert_space" | "ui.delete_space" => {
                let name = invocation.name.replace("ui.", "playlist.");
                match frame.project.playlist.loop_range {
                    Some((start, end)) => out.push(Action::Run(Invocation::new(
                        &name,
                        [format!("{start}t"), format!("{}t", end - start)],
                    ))),
                    None => state.set_status(
                        "select a time range first: Shift+drag the playlist ruler",
                        true,
                    ),
                }
            }
            "ui.next_empty_pattern" => {
                // Mirrors `pattern.next_empty`, then offers to name the pattern.
                let project = frame.project;
                let (index, name) = match project.next_empty_pattern() {
                    Some(i) => (i, project.patterns[i].name.clone()),
                    None => (
                        project.patterns.len(),
                        format!("Pattern {}", project.patterns.len() + 1),
                    ),
                };
                out.push(Action::Run(Invocation::bare("pattern.next_empty")));
                state.dialog = Some(dialogs::Dialog::rename_pattern(index, &name));
            }
            "ui.export" => state.dialog = Some(dialogs::Dialog::export(frame.project)),
            "ui.export_stems" => state.dialog = Some(dialogs::Dialog::export_stems(frame.project)),
            "ui.export_midi" => state.dialog = Some(dialogs::Dialog::export_midi(frame.project)),
            "ui.quit" => out.push(Action::Quit),
            "ui.typing_keyboard" => state.typing_keyboard = !state.typing_keyboard,
            "ui.automate" => {
                if let Some(target) = invocation.args.first() {
                    out.push(Action::Run(automation::create_invocation(
                        frame.project,
                        target,
                    )));
                }
            }
            "ui.add_folder" | "ui.add_plugin_folder" => {
                if let Some(path) = invocation.args.first() {
                    let path = std::path::PathBuf::from(path);
                    let mut next = frame.settings.clone();
                    let list = if invocation.name == "ui.add_folder" {
                        &mut next.browser_folders
                    } else {
                        &mut next.plugin_folders
                    };
                    if !list.contains(&path) {
                        list.push(path);
                        out.push(Action::Settings(Box::new(next)));
                    }
                }
            }
            "ui.settings" => {
                state.settings_tab = Some(
                    invocation
                        .args
                        .first()
                        .and_then(|t| settings::Tab::parse(t))
                        .unwrap_or(settings::Tab::Audio),
                );
            }
            _ => out.push(action),
        }
    }
    out
}

#[cfg(test)]
mod interaction_tests;

#[cfg(test)]
mod glyph_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_for(project: &Project) -> (EngineStatus, History) {
        let _ = project;
        (EngineStatus::default(), History::default())
    }

    #[test]
    fn view_actions_toggle_windows_and_are_consumed() {
        let project = Project::default();
        let (engine, history) = frame_for(&project);
        let frame = Frame {
            project: &project,
            engine: &engine,
            history: &history,
            waveform: None,
            waveform_seconds: None,
            clip_waveforms: &[],
            clip_durations: &[],
            plugins: &Plugins::default(),
            settings: &Settings::default(),
            settings_info: &SettingsInfo::default(),
        };
        let mut state = UiState::default();
        assert!(!state.windows.mixer);
        let out = resolve_ui_actions(
            &mut state,
            &frame,
            vec![
                Action::Run(Invocation::parse("view.toggle mixer").unwrap()),
                Action::Run(Invocation::parse("transport.toggle").unwrap()),
                Action::Run(Invocation::parse("ui.save").unwrap()),
            ],
        );
        assert!(state.windows.mixer);
        assert_eq!(out, [Action::Run(Invocation::bare("transport.toggle"))]);
        assert!(state.dialog.is_some(), "unsaved project asks for a path");
        state.dialog = None;
        let out = resolve_ui_actions(
            &mut state,
            &frame,
            vec![Action::Run(Invocation::bare("ui.save_new_version"))],
        );
        assert!(out.is_empty());
        assert!(state.dialog.is_some(), "a new version needs a first save");
    }

    #[test]
    fn settings_actions_open_tabs_and_add_folders() {
        let project = Project::default();
        let (engine, history) = frame_for(&project);
        let settings = Settings::default();
        let frame = Frame {
            project: &project,
            engine: &engine,
            history: &history,
            waveform: None,
            waveform_seconds: None,
            clip_waveforms: &[],
            clip_durations: &[],
            plugins: &Plugins::default(),
            settings: &settings,
            settings_info: &SettingsInfo::default(),
        };
        let mut state = UiState::default();
        let out = resolve_ui_actions(
            &mut state,
            &frame,
            vec![
                Action::Run(Invocation::parse("ui.settings midi").unwrap()),
                Action::Run(Invocation::parse("ui.add_plugin_folder /opt/clap").unwrap()),
            ],
        );
        assert_eq!(state.settings_tab, Some(settings::Tab::Midi));
        let [Action::Settings(next)] = out.as_slice() else {
            panic!("expected new settings, got {out:?}");
        };
        assert_eq!(next.plugin_folders, [std::path::PathBuf::from("/opt/clap")]);
        resolve_ui_actions(
            &mut state,
            &frame,
            vec![Action::Run(Invocation::parse("ui.settings").unwrap())],
        );
        assert_eq!(state.settings_tab, Some(settings::Tab::Audio));
    }

    #[test]
    fn meters_jump_up_and_decay() {
        let mut state = UiState::default();
        state.update_meters(&[(1.0, 0.5)], 0.0);
        assert_eq!(state.meters, [(1.0, 0.5)]);
        state.update_meters(&[(0.0, 0.0)], 1.0);
        assert!(state.meters[0].0 < 0.07 && state.meters[0].0 > 0.05);
    }
}
