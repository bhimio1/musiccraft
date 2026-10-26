//! The settings window (Options → Settings, F10).

use crate::hint::Hint;
use std::path::PathBuf;

use egui::RichText;
use musiccraft_commands::Invocation;
use serde::{Deserialize, Serialize};

use crate::dialogs::{FilePick, Pick};
use crate::{Action, Frame, UiState, theme};

/// Which MIDI input to use.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MidiInput {
    /// The first real input port.
    #[default]
    Auto,
    /// No MIDI input.
    Off,
    /// A port by name.
    Port(String),
}

/// User settings, stored outside projects. Unknown or missing fields take
/// their defaults, so older files keep working.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Output device name; `None` for the system default.
    pub audio_device: Option<String>,
    /// Input device for audio recording; `None` for the system default.
    pub audio_input: Option<String>,
    /// Sample rate; `None` for the device default.
    pub sample_rate: Option<u32>,
    /// Frames per audio callback; `None` for the device default.
    pub buffer_frames: Option<u32>,
    /// MIDI input port.
    pub midi_input: MidiInput,
    /// MIDI output port for MIDI Out channels; `None` sends nothing.
    pub midi_output: Option<String>,
    /// Send MIDI clock and start/stop on the MIDI output.
    pub midi_clock: bool,
    /// Interface scale, 0.75–2.
    pub ui_scale: f32,
    /// Show the hint bar.
    pub show_hints: bool,
    /// Save a recovery copy every few minutes.
    pub autosave: bool,
    /// Start with an empty project instead of the starter kit.
    pub start_empty: bool,
    /// Folders shown in the browser.
    pub browser_folders: Vec<PathBuf>,
    /// Extra folders searched for CLAP plugins.
    pub plugin_folders: Vec<PathBuf>,
    /// Scan the plugin folders when the app starts; when off, the plugin
    /// list stays empty until a rescan.
    pub scan_plugins_on_start: bool,
    /// FL Studio's factory data folder, for `%FLStudioFactoryData%` paths.
    pub fl_factory: Option<PathBuf>,
    /// FL Studio's user data folder, for `%FLStudioUserData%` paths.
    pub fl_user: Option<PathBuf>,
    /// Windows path prefixes (`C:`, `D:/Samples`) and the folders they stand
    /// for here.
    pub drive_map: Vec<(String, PathBuf)>,
    /// A click on another channel's name opens a window of its own instead
    /// of switching the open channel window.
    pub keep_instrument_windows: bool,
    /// The window arrangement when the app last closed, restored on start.
    pub window_layout: Option<WindowLayout>,
}

/// Which windows were open, where, and which were folded to their header.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowLayout {
    /// Open windows, by name (`channel_rack`, `mixer`, `browser`, …).
    pub open: Vec<String>,
    /// Each window's last place: name, then left, top, width, height.
    pub rects: Vec<(String, [f32; 4])>,
    /// Windows folded to their header, by name.
    pub folded: Vec<String>,
    /// Maximized windows, by name; `None` in layouts saved before this was
    /// kept, which start with the playlist maximized.
    pub maximized: Option<Vec<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio_device: None,
            audio_input: None,
            sample_rate: None,
            buffer_frames: None,
            midi_input: MidiInput::Auto,
            midi_output: None,
            midi_clock: false,
            ui_scale: 1.0,
            show_hints: true,
            autosave: true,
            start_empty: false,
            browser_folders: crate::browser::default_folders(),
            plugin_folders: Vec::new(),
            scan_plugins_on_start: true,
            fl_factory: None,
            fl_user: None,
            drive_map: Vec::new(),
            keep_instrument_windows: false,
            window_layout: None,
        }
    }
}

impl Settings {
    /// The scale, kept in its allowed range.
    #[must_use]
    pub fn scale(&self) -> f32 {
        if self.ui_scale.is_finite() {
            self.ui_scale.clamp(0.75, 2.0)
        } else {
            1.0
        }
    }

    /// The sample path translation these settings describe, on top of what
    /// the computer offers (see `docs/spec/sample-paths.md`).
    #[must_use]
    pub fn path_config(&self) -> musiccraft_paths::PathConfig {
        musiccraft_paths::PathConfig::detect().with_overrides(&musiccraft_paths::Overrides {
            fl_factory: self.fl_factory.clone(),
            fl_user: self.fl_user.clone(),
            drives: self.drive_map.clone(),
            search: self.browser_folders.clone(),
        })
    }

    /// Whether `other` differs in anything that needs audio restarted.
    #[must_use]
    pub fn audio_differs(&self, other: &Self) -> bool {
        self.audio_device != other.audio_device
            || self.sample_rate != other.sample_rate
            || self.buffer_frames != other.buffer_frames
    }
}

/// What the app reports for the settings window. Read-only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsInfo {
    /// Output devices, default first.
    pub outputs: Vec<String>,
    /// Input devices, default first.
    pub inputs: Vec<String>,
    /// Sample rates the current device supports.
    pub sample_rates: Vec<u32>,
    /// The rate audio is running at.
    pub running_rate: u32,
    /// MIDI input ports.
    pub midi_ports: Vec<String>,
    /// MIDI output ports.
    pub midi_out_ports: Vec<String>,
    /// A note about the last audio (re)start, such as a fallback.
    pub audio_note: Option<String>,
    /// Installed plugins found.
    pub plugin_count: usize,
    /// The plugin folders were not scanned at start and no rescan ran yet.
    pub plugins_unscanned: bool,
    /// Plugin files that failed to load.
    pub plugin_errors: Vec<String>,
}

/// Tabs of the settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// Output device, rate, buffer.
    Audio,
    /// MIDI input.
    Midi,
    /// Interface and startup.
    General,
    /// Browser and plugin folders.
    Folders,
    /// Name, tempo and statistics.
    Project,
}

impl Tab {
    const ALL: [Tab; 5] = [
        Tab::Audio,
        Tab::Midi,
        Tab::General,
        Tab::Folders,
        Tab::Project,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::Midi => "MIDI",
            Self::General => "General",
            Self::Folders => "Folders",
            Self::Project => "Project",
        }
    }

    /// Parses a tab name from a `ui.settings <tab>` invocation.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.label().eq_ignore_ascii_case(name))
    }
}

/// Buffer sizes offered, in frames.
const BUFFERS: [u32; 7] = [64, 128, 256, 512, 1024, 2048, 4096];

fn changed(actions: &mut Vec<Action>, settings: Settings) {
    actions.push(Action::Settings(Box::new(settings)));
}

pub fn show(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let Some(mut tab) = state.settings_tab else {
        return;
    };
    let mut open = true;
    let center = state.workspace.center();
    egui::Window::new("Settings")
        .frame(theme::dialog_frame())
        .open(&mut open)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(center)
        .default_size([520.0, 420.0])
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                for t in Tab::ALL {
                    ui.selectable_value(&mut tab, t, RichText::new(t.label()).strong());
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| match tab {
                Tab::Audio => audio(ui, frame, actions),
                Tab::Midi => midi(ui, state, frame, actions),
                Tab::General => general(ui, state, frame, actions),
                Tab::Folders => folders(ui, state, frame, actions),
                Tab::Project => project(ui, state, frame, actions),
            });
        });
    state.settings_tab = open.then_some(tab);
}

fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [130.0, 20.0],
            egui::Label::new(RichText::new(label).color(theme::TEXT_DIM)),
        );
        add(ui);
    });
}

fn note(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().color(theme::TEXT_DIM));
}

fn audio(ui: &mut egui::Ui, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let settings = frame.settings;
    let info = frame.settings_info;
    row(ui, "Input device", |ui| {
        let current = settings
            .audio_input
            .clone()
            .unwrap_or_else(|| "System default".to_owned());
        egui::ComboBox::from_id_salt("audio-input")
            .width(300.0)
            .selected_text(current)
            .show_ui(ui, |ui| {
                let choices = std::iter::once(None).chain(info.inputs.iter().cloned().map(Some));
                for choice in choices {
                    let label = choice
                        .clone()
                        .unwrap_or_else(|| "System default".to_owned());
                    if ui
                        .selectable_label(settings.audio_input == choice, label)
                        .clicked()
                    {
                        changed(
                            actions,
                            Settings {
                                audio_input: choice,
                                ..settings.clone()
                            },
                        );
                    }
                }
            })
            .response
            .hint("Recorded by Options → Record audio input");
    });
    row(ui, "Output device", |ui| {
        let current = settings
            .audio_device
            .clone()
            .unwrap_or_else(|| "System default".to_owned());
        egui::ComboBox::from_id_salt("audio-device")
            .width(300.0)
            .selected_text(current)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(settings.audio_device.is_none(), "System default")
                    .clicked()
                {
                    changed(
                        actions,
                        Settings {
                            audio_device: None,
                            ..settings.clone()
                        },
                    );
                }
                for name in &info.outputs {
                    let selected = settings.audio_device.as_ref() == Some(name);
                    if ui.selectable_label(selected, name).clicked() {
                        changed(
                            actions,
                            Settings {
                                audio_device: Some(name.clone()),
                                ..settings.clone()
                            },
                        );
                    }
                }
            });
    });
    row(ui, "Sample rate", |ui| {
        let text = settings
            .sample_rate
            .map_or_else(|| "Device default".to_owned(), |r| format!("{r} Hz"));
        egui::ComboBox::from_id_salt("sample-rate")
            .width(160.0)
            .selected_text(text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(settings.sample_rate.is_none(), "Device default")
                    .clicked()
                {
                    changed(
                        actions,
                        Settings {
                            sample_rate: None,
                            ..settings.clone()
                        },
                    );
                }
                for rate in &info.sample_rates {
                    let label = format!("{rate} Hz");
                    if ui
                        .selectable_label(settings.sample_rate == Some(*rate), label)
                        .clicked()
                    {
                        changed(
                            actions,
                            Settings {
                                sample_rate: Some(*rate),
                                ..settings.clone()
                            },
                        );
                    }
                }
            });
        ui.label(RichText::new(format!("running at {} Hz", info.running_rate)).small());
    });
    row(ui, "Buffer size", |ui| {
        let rate = f64::from(info.running_rate.max(1));
        let latency = |frames: u32| {
            format!(
                "{frames} frames · {:.1} ms",
                f64::from(frames) / rate * 1000.0
            )
        };
        let text = settings
            .buffer_frames
            .map_or_else(|| "Device default".to_owned(), latency);
        egui::ComboBox::from_id_salt("buffer-size")
            .width(200.0)
            .selected_text(text)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(settings.buffer_frames.is_none(), "Device default")
                    .clicked()
                {
                    changed(
                        actions,
                        Settings {
                            buffer_frames: None,
                            ..settings.clone()
                        },
                    );
                }
                for frames in BUFFERS {
                    if ui
                        .selectable_label(settings.buffer_frames == Some(frames), latency(frames))
                        .clicked()
                    {
                        changed(
                            actions,
                            Settings {
                                buffer_frames: Some(frames),
                                ..settings.clone()
                            },
                        );
                    }
                }
            });
    });
    ui.add_space(6.0);
    ui.label(RichText::new(&frame.engine.audio).color(theme::ACCENT));
    // The note repeats nothing the status line already says (a failed start
    // puts the same error in both).
    if let Some(text) = info
        .audio_note
        .as_ref()
        .filter(|t| !frame.engine.audio.contains(t.as_str()))
    {
        ui.label(RichText::new(text).color(theme::HIGHLIGHT));
    }
    note(
        ui,
        "Smaller buffers lower the delay between playing and hearing, but need more \
         CPU headroom. Changing these restarts audio.",
    );
    ui.horizontal(|ui| {
        if ui.button("Refresh devices").clicked() {
            actions.push(Action::RefreshDevices);
        }
        if ui.button("Restart audio").clicked() {
            actions.push(Action::RestartAudio);
        }
    });
}

fn midi(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let settings = frame.settings;
    let info = frame.settings_info;
    ui.label(RichText::new("Input").strong());
    let mut choice = |ui: &mut egui::Ui, label: &str, value: MidiInput| {
        if ui.radio(settings.midi_input == value, label).clicked() {
            changed(
                actions,
                Settings {
                    midi_input: value,
                    ..settings.clone()
                },
            );
        }
    };
    choice(ui, "Automatic (first keyboard found)", MidiInput::Auto);
    choice(ui, "None", MidiInput::Off);
    for port in &info.midi_ports {
        choice(ui, port, MidiInput::Port(port.clone()));
    }
    if info.midi_ports.is_empty() {
        note(
            ui,
            "No MIDI input ports found. Plug in a keyboard and refresh.",
        );
    }
    ui.add_space(4.0);
    ui.label(RichText::new(&frame.engine.midi).color(theme::ACCENT));
    ui.add_space(8.0);
    ui.label(RichText::new("Output (MIDI Out channels)").strong());
    let mut output = |ui: &mut egui::Ui, label: &str, value: Option<String>| {
        if ui.radio(settings.midi_output == value, label).clicked() {
            changed(
                actions,
                Settings {
                    midi_output: value,
                    ..settings.clone()
                },
            );
        }
    };
    output(ui, "None", None);
    for port in &info.midi_out_ports {
        output(ui, port, Some(port.clone()));
    }
    let mut clock = settings.midi_clock;
    if crate::widgets::led_switch(
        ui,
        &mut clock,
        "Send MIDI clock (24 per beat) with start and stop",
    )
    .changed()
    {
        changed(
            actions,
            Settings {
                midi_clock: clock,
                ..settings.clone()
            },
        );
    }
    if info.midi_out_ports.is_empty() {
        note(
            ui,
            "No MIDI output ports found. Connect your gear and refresh.",
        );
    }
    ui.add_space(4.0);
    ui.label(RichText::new(&frame.engine.midi_out).color(theme::ACCENT));
    if ui.button("Refresh ports").clicked() {
        actions.push(Action::RefreshDevices);
    }
    ui.add_space(8.0);
    crate::widgets::led_switch(
        ui,
        &mut state.typing_keyboard,
        "Typing keyboard plays notes (Z–M, Q–P rows)",
    );
    note(
        ui,
        "Live notes play the selected channel; with REC on they are recorded into the pattern.",
    );
}

fn general(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let settings = frame.settings;
    row(ui, "Interface scale", |ui| {
        // Applied when the drag ends, so the slider does not move under the
        // pointer while scaling.
        let mut percent = state
            .scale_draft
            .unwrap_or_else(|| (settings.scale() * 100.0).round());
        let slider = egui::Slider::new(&mut percent, 75.0..=200.0)
            .step_by(5.0)
            .suffix(" %");
        let response = ui.add(slider);
        state.scale_draft = Some(percent);
        let done = response.drag_stopped() || (response.changed() && !response.dragged());
        if done && (percent / 100.0 - settings.scale()).abs() > 1e-3 {
            changed(
                actions,
                Settings {
                    ui_scale: percent / 100.0,
                    ..settings.clone()
                },
            );
        }
        if !response.dragged() && !response.changed() {
            state.scale_draft = None;
        }
    });
    let mut toggle = |ui: &mut egui::Ui, label: &str, value: bool, set: fn(&mut Settings, bool)| {
        let mut on = value;
        if crate::widgets::led_switch(ui, &mut on, label).changed() {
            let mut next = settings.clone();
            set(&mut next, on);
            changed(actions, next);
        }
    };
    toggle(ui, "Show the hint bar", settings.show_hints, |s, v| {
        s.show_hints = v
    });
    toggle(
        ui,
        "Autosave a recovery copy every minute",
        settings.autosave,
        |s, v| s.autosave = v,
    );
    toggle(
        ui,
        "Start with an empty project",
        settings.start_empty,
        |s, v| s.start_empty = v,
    );
    toggle(
        ui,
        "Keep instrument windows open (a click on another channel opens its own window)",
        settings.keep_instrument_windows,
        |s, v| s.keep_instrument_windows = v,
    );
}

fn folder_list(ui: &mut egui::Ui, folders: &[PathBuf]) -> Option<usize> {
    let mut remove = None;
    for (i, folder) in folders.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button("×").hint("Remove").clicked() {
                remove = Some(i);
            }
            ui.label(folder.display().to_string());
        });
    }
    if folders.is_empty() {
        note(ui, "None.");
    }
    remove
}

fn folders(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let settings = frame.settings;
    let info = frame.settings_info;
    ui.label(RichText::new("Browser folders").strong());
    if let Some(i) = folder_list(ui, &settings.browser_folders) {
        let mut next = settings.clone();
        next.browser_folders.remove(i);
        changed(actions, next);
    }
    if ui.small_button("+ Add folder…").clicked() {
        state.pick_file(FilePick::add_folder());
    }
    ui.add_space(10.0);
    ui.label(RichText::new("Extra plugin folders").strong());
    note(
        ui,
        "Searched for .clap files in addition to ~/.clap, /usr/lib/clap and CLAP_PATH.",
    );
    if let Some(i) = folder_list(ui, &settings.plugin_folders) {
        let mut next = settings.clone();
        next.plugin_folders.remove(i);
        changed(actions, next);
    }
    ui.horizontal(|ui| {
        if ui.small_button("+ Add plugin folder…").clicked() {
            state.pick_file(FilePick::add_plugin_folder());
        }
        if ui.small_button("Rescan plugins").clicked() {
            actions.push(Action::RescanPlugins);
        }
    });
    let mut scan = settings.scan_plugins_on_start;
    if crate::widgets::led_switch(ui, &mut scan, "Scan plugins on startup").changed() {
        let mut next = settings.clone();
        next.scan_plugins_on_start = scan;
        changed(actions, next);
    }
    if info.plugins_unscanned {
        ui.label("Plugins not scanned yet; rescan to list them");
    } else {
        ui.label(format!("{} plugins found", info.plugin_count));
    }
    ui.add_space(10.0);
    foreign_paths(ui, state, settings, actions);
    if !info.plugin_errors.is_empty() {
        egui::CollapsingHeader::new(
            RichText::new(format!("{} files failed to load", info.plugin_errors.len()))
                .color(theme::HIGHLIGHT),
        )
        .show(ui, |ui| {
            for error in &info.plugin_errors {
                note(ui, error);
            }
        });
    }
}

/// Folders that sample paths from Windows projects translate to (see
/// `docs/spec/sample-paths.md`).
fn foreign_paths(
    ui: &mut egui::Ui,
    state: &mut UiState,
    settings: &Settings,
    actions: &mut Vec<Action>,
) {
    ui.label(RichText::new("Samples from Windows projects").strong());
    note(
        ui,
        "Paths like C:\\… or %FLStudioFactoryData%\\… are looked up here (Wine prefixes, \
         /mnt drives and your home folder are tried too). Leave empty to detect.",
    );
    for (label, current, pick, clear) in [
        (
            "FL Studio factory data",
            &settings.fl_factory,
            Pick::FlFactory,
            (|s: &mut Settings| s.fl_factory = None) as fn(&mut Settings),
        ),
        (
            "FL Studio user data",
            &settings.fl_user,
            Pick::FlUser,
            |s: &mut Settings| s.fl_user = None,
        ),
    ] {
        row(ui, label, |ui| {
            match current {
                Some(path) => {
                    ui.label(
                        RichText::new(path.display().to_string())
                            .monospace()
                            .small(),
                    );
                }
                None => {
                    ui.label(RichText::new("detected").color(theme::TEXT_DIM));
                }
            }
            if ui
                .small_button("Choose…")
                .hint("Choose the folder with the system's file dialog")
                .clicked()
            {
                state.pick_file(FilePick::folder(pick));
            }
            if current.is_some() && ui.small_button("×").hint("Detect it again").clicked() {
                let mut next = settings.clone();
                clear(&mut next);
                changed(actions, next);
            }
        });
    }
    ui.label("Drive mappings");
    for (i, (prefix, folder)) in settings.drive_map.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button("×").hint("Remove").clicked() {
                let mut next = settings.clone();
                next.drive_map.remove(i);
                changed(actions, next);
            }
            ui.label(format!("{prefix}  →  {}", folder.display()));
        });
    }
    ui.horizontal(|ui| {
        let (from, to) = &mut state.drive_draft;
        ui.add(
            egui::TextEdit::singleline(from)
                .hint_text("C:\\Users\\Ana\\Music")
                .desired_width(170.0),
        );
        ui.label("→");
        let folder = to
            .as_ref()
            .map_or_else(|| "Choose folder…".to_owned(), |p| p.display().to_string());
        let choose = ui
            .button(folder)
            .hint("The local folder those paths point to (the system's file dialog)")
            .clicked();
        let ready = !from.trim().is_empty() && to.is_some();
        if crate::widgets::add_enabled(ui, ready, egui::Button::new("Add")).clicked()
            && let Some(folder) = to.take()
        {
            let mut next = settings.clone();
            next.drive_map.push((from.trim().to_owned(), folder));
            from.clear();
            changed(actions, next);
        }
        if choose {
            state.pick_file(FilePick::folder(Pick::DriveFolder));
        }
    });
}

fn project(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let project = frame.project;
    row(ui, "Name", |ui| {
        let edit = ui.add(egui::TextEdit::singleline(&mut state.project_name).desired_width(260.0));
        if !edit.has_focus() && !edit.lost_focus() {
            project.name().clone_into(&mut state.project_name);
        }
        if edit.lost_focus()
            && state.project_name.trim() != project.name()
            && !state.project_name.trim().is_empty()
        {
            actions.push(Action::Run(Invocation::new(
                "project.rename",
                [state.project_name.trim().to_owned()],
            )));
        }
    });
    row(ui, "Tempo", |ui| {
        let mut tempo = project.tempo_bpm();
        let drag = ui.add(
            egui::DragValue::new(&mut tempo)
                .range(musiccraft_project::MIN_TEMPO_BPM..=musiccraft_project::MAX_TEMPO_BPM)
                .speed(0.1)
                .suffix(" BPM"),
        );
        if drag.drag_started() || drag.gained_focus() {
            state.begin_gesture();
        }
        if drag.changed() {
            actions.push(Action::Coalesced(
                Invocation::new("project.set_tempo", [format!("{tempo:.3}")]),
                state.gesture(),
            ));
        }
    });
    row(ui, "Master pitch", |ui| {
        let mut cents = project.master_pitch;
        let drag = ui.add(
            egui::DragValue::new(&mut cents)
                .range(-1200.0..=1200.0)
                .speed(1.0)
                .suffix(" ct"),
        );
        if drag.drag_started() || drag.gained_focus() {
            state.begin_gesture();
        }
        if drag.changed() {
            actions.push(Action::Coalesced(
                Invocation::new("project.set_master_pitch", [format!("{cents:.0}")]),
                state.gesture(),
            ));
        }
        let a4 = 440.0 * 2f64.powf(f64::from(project.master_pitch) / 1200.0);
        ui.label(format!("A4 = {a4:.2} Hz"));
    });
    row(ui, "Pan law", |ui| {
        for law in musiccraft_project::PanLaw::ALL {
            let hint = match law {
                musiccraft_project::PanLaw::Circular => "Equal power: -3 dB on each side at centre",
                musiccraft_project::PanLaw::Triangular => {
                    "Equal amplitude: -6 dB on each side at centre"
                }
            };
            let chosen = project.pan_law == law;
            if ui
                .selectable_label(chosen, law.label())
                .on_hover_text(hint)
                .clicked()
                && !chosen
            {
                actions.push(Action::Run(Invocation::new(
                    "project.set_pan_law",
                    [law.label().to_owned()],
                )));
            }
        }
    });
    row(ui, "File", |ui| {
        ui.label(
            project
                .path()
                .map_or_else(|| "not saved yet".to_owned(), |p| p.display().to_string()),
        );
    });
    ui.add_space(6.0);
    let clips = project.playlist.clips.len();
    let plugins = project.plugins().len();
    note(
        ui,
        &format!(
            "{} channels · {} patterns · {} mixer inserts · {clips} playlist clips · {} automations · {plugins} plugins",
            project.channels.len(),
            project.patterns.len(),
            project.mixer.len().saturating_sub(1),
            project.automations.len(),
        ),
    );
}
