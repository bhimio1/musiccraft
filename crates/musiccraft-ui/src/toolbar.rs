//! Menu bar, transport bar and status bar.

use crate::hint::Hint;
use egui::{Rect, RichText, Sense, pos2, vec2};
use musiccraft_commands::Invocation;
use musiccraft_project::{MAX_TEMPO_BPM, MIN_TEMPO_BPM, PlayMode};

use crate::icons::{self, Icon};
use crate::{Action, Frame, UiState, format, geometry::position_label, signature, theme, widgets};

fn run(actions: &mut Vec<Action>, line: &str) {
    if let Ok(invocation) = Invocation::parse(line) {
        actions.push(Action::Run(invocation));
    }
}

fn shortcut(state: &UiState, invocation: &str) -> String {
    state
        .keymap
        .chord_for(invocation)
        .map(ToString::to_string)
        .unwrap_or_default()
}

fn menu_item(
    ui: &mut egui::Ui,
    state: &UiState,
    label: &str,
    invocation: &str,
    actions: &mut Vec<Action>,
) {
    let button = egui::Button::new(label).shortcut_text(shortcut(state, invocation));
    if ui.add(button).clicked() {
        run(actions, invocation);
        ui.close();
    }
}

fn window_toggle(ui: &mut egui::Ui, state: &mut UiState, label: &str, which: &str) {
    let open = match which {
        "playlist" => &mut state.windows.playlist,
        "channel_rack" => &mut state.windows.channel_rack,
        "piano_roll" => &mut state.windows.piano_roll,
        "mixer" => &mut state.windows.mixer,
        "browser" => &mut state.windows.browser,
        "spectrum" => &mut state.windows.spectrum,
        "tap_tempo" => &mut state.windows.tap_tempo,
        _ => &mut state.windows.shortcuts,
    };
    ui.checkbox(open, label);
}

fn settings_item(
    ui: &mut egui::Ui,
    state: &UiState,
    label: &str,
    tab: &str,
    actions: &mut Vec<Action>,
) {
    let invocation = format!("ui.settings {tab}");
    let shortcut = if tab == "audio" {
        shortcut(state, "ui.settings")
    } else {
        String::new()
    };
    if ui
        .add(egui::Button::new(label).shortcut_text(shortcut))
        .clicked()
    {
        run(actions, &invocation);
        ui.close();
    }
}

fn file_menu(ui: &mut egui::Ui, state: &mut UiState, actions: &mut Vec<Action>) {
    menu_item(ui, state, "New", "ui.new", actions);
    menu_item(ui, state, "Open…", "ui.open", actions);
    menu_item(ui, state, "Save", "ui.save", actions);
    menu_item(ui, state, "Save as…", "ui.save_as", actions);
    menu_item(
        ui,
        state,
        "Save new version",
        "ui.save_new_version",
        actions,
    );
    ui.separator();
    menu_item(ui, state, "Export audio (WAV, FLAC)…", "ui.export", actions);
    menu_item(ui, state, "Export stems…", "ui.export_stems", actions);
    menu_item(ui, state, "Export MIDI…", "ui.export_midi", actions);
    if let Some(path) = state.autosave_path.clone() {
        if ui
            .button("Recover unsaved work")
            .hint("Restore the project from the session that did not close properly")
            .clicked()
        {
            actions.push(Action::Run(Invocation::new("project.recover", [path])));
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Exit").clicked() {
        actions.push(Action::Quit);
    }
}

fn add_menu(ui: &mut egui::Ui, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    ui.menu_button("Channel", |ui| {
        for name in musiccraft_project::params::BUILTIN_SAMPLES {
            if ui.button(name.replace('_', " ")).clicked() {
                actions.push(Action::Run(Invocation::new(
                    "channel.add_sampler",
                    [format!(
                        "{}{name}",
                        musiccraft_project::params::BUILTIN_PREFIX
                    )],
                )));
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Synth").clicked() {
            run(actions, "channel.add_synth");
            ui.close();
        }
        ui.menu_button("Drum synth", |ui| {
            for kind in musiccraft_project::params::DRUM_TYPES {
                if ui.button(kind).clicked() {
                    run(actions, &format!("channel.add_drum {kind}"));
                    ui.close();
                }
            }
        });
        if ui.button("Pluck").clicked() {
            run(actions, "channel.add_pluck");
            ui.close();
        }
        ui.menu_button("More instruments", |ui| {
            for (label, line) in [
                ("FM synth", "channel.add_fm"),
                ("Piano", "channel.add_keys piano"),
                ("Electric piano", "channel.add_keys epiano"),
                ("Organ", "channel.add_keys organ"),
                ("Additive", "channel.add_additive"),
                ("Drum kit (16 pads)", "channel.add_kit"),
                ("Zone sampler", "channel.add_zones"),
                ("Channel stack", "channel.add_layer"),
                ("MIDI output", "channel.add_midi_out"),
            ] {
                if ui.button(label).clicked() {
                    if let Ok(invocation) = Invocation::parse(line) {
                        actions.push(Action::Run(invocation));
                    }
                    ui.close();
                }
            }
        });
    });
    ui.menu_button("Plugin instrument", |ui| {
        crate::plugins::add_menu(ui, frame.plugins, true, actions, |p| {
            Invocation::new("channel.add_plugin", [p.id.clone(), p.name.clone()])
        });
    });
    ui.separator();
    let last = crate::automation::last_moved(ui.ctx());
    let label = last
        .as_deref()
        .and_then(|target| musiccraft_commands::control_info(frame.project, target))
        .map_or_else(|| "nothing moved yet".to_owned(), |info| info.label);
    if crate::widgets::add_enabled(
        ui,
        last.is_some(),
        egui::Button::new("Automation clip for the last moved control"),
    )
    .hint(format!(
        "Create an automation clip for the last moved control ({label})"
    ))
    .clicked()
        && let Some(target) = last
    {
        actions.push(Action::Run(Invocation::new("ui.automate", [target])));
        ui.close();
    }
    ui.separator();
    if ui.button("Pattern").clicked() {
        run(actions, "pattern.add");
        ui.close();
    }
    if ui.button("Mixer insert").clicked() {
        run(actions, "mixer.add_track");
        ui.close();
    }
}

fn patterns_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    let number = project.current_pattern + 1;
    menu_item(
        ui,
        state,
        "First empty pattern",
        "pattern.first_empty",
        actions,
    );
    menu_item(
        ui,
        state,
        "Next empty pattern",
        "ui.next_empty_pattern",
        actions,
    );
    menu_item(ui, state, "New pattern", "pattern.add", actions);
    if ui.button("Clone pattern").clicked() {
        run(actions, &format!("pattern.clone {number}"));
        ui.close();
    }
    crate::playlist::split_pattern_item(
        ui,
        project.current_pattern(),
        project.current_pattern,
        actions,
    );
    menu_item(ui, state, "Rename pattern…", "ui.rename_pattern", actions);
    crate::widgets::enabled_ui(ui, project.patterns.len() > 1, |ui| {
        if ui.button("Delete pattern").clicked() {
            run(actions, &format!("pattern.remove {number}"));
            ui.close();
        }
    });
    ui.separator();
    menu_item(ui, state, "Next pattern", "pattern.next", actions);
    menu_item(ui, state, "Previous pattern", "pattern.prev", actions);
    ui.separator();
    for (i, pattern) in project.patterns.iter().enumerate() {
        let text = RichText::new(format!("{}  {}", i + 1, pattern.name))
            .color(crate::pattern_text(pattern));
        if ui
            .selectable_label(i == project.current_pattern, text)
            .clicked()
        {
            run(actions, &format!("pattern.select {}", i + 1));
            ui.close();
        }
    }
}

fn view_menu(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    window_toggle(ui, state, "Playlist (F5)", "playlist");
    window_toggle(ui, state, "Channel rack (F6)", "channel_rack");
    window_toggle(ui, state, "Piano roll (F7)", "piano_roll");
    window_toggle(ui, state, "Browser (Alt+F8)", "browser");
    window_toggle(ui, state, "Mixer (F9)", "mixer");
    window_toggle(ui, state, "Spectrum analyser", "spectrum");
    window_toggle(ui, state, "Tap tempo", "tap_tempo");
    ui.separator();
    let mut hints = frame.settings.show_hints;
    if ui.checkbox(&mut hints, "Show hint bar").changed() {
        actions.push(Action::Settings(Box::new(crate::Settings {
            show_hints: hints,
            ..frame.settings.clone()
        })));
    }
    window_toggle(ui, state, "Keyboard shortcuts", "shortcuts");
}

fn options_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    settings_item(ui, state, "Audio settings", "audio", actions);
    settings_item(ui, state, "MIDI settings", "midi", actions);
    settings_item(ui, state, "General settings", "general", actions);
    settings_item(ui, state, "Folders and plugins", "folders", actions);
    settings_item(ui, state, "Project info", "project", actions);
    ui.separator();
    let mut metronome = frame.project.metronome;
    if ui.checkbox(&mut metronome, "Metronome").changed() {
        run(actions, "transport.toggle_metronome");
    }
    ui.checkbox(
        &mut state.typing_keyboard,
        "Play notes from the computer keyboard",
    );
    ui.checkbox(&mut state.record_audio, "Record audio input")
        .hint("With REC armed in song mode, capture the audio input as a clip");
}

fn tools_menu(ui: &mut egui::Ui, frame: &Frame<'_>, state: &UiState, actions: &mut Vec<Action>) {
    ui.menu_button("Panic", |ui| {
        menu_item(ui, state, "All notes off", "transport.panic", actions);
    });
    ui.separator();
    ui.menu_button("Last moved control", |ui| {
        crate::automation::last_moved_menu(ui, frame.project, actions);
    });
    ui.separator();
    if ui.button("Rescan plugins").clicked() {
        actions.push(Action::RescanPlugins);
        ui.close();
    }
    if ui.button("Reload samples").clicked() {
        actions.push(Action::ReloadSamples);
        ui.close();
    }
    let channel = state.selected_channel + 1;
    crate::widgets::enabled_ui(
        ui,
        state.selected_channel < frame.project.channels.len(),
        |ui| {
            if ui
                .button("Quantize selected channel")
                .hint("Snap the selected channel's notes in this pattern to the step grid")
                .clicked()
            {
                run(actions, &format!("note.quantize {channel} 1"));
                ui.close();
            }
        },
    );
}

fn help_menu(ui: &mut egui::Ui, state: &mut UiState) {
    if ui.button("Keyboard shortcuts").clicked() {
        state.windows.shortcuts = true;
        ui.close();
    }
    if ui.button("About MusicCraft").clicked() {
        state.set_status(
            format!(
                "MusicCraft {} — a clean-room pattern-based DAW written in Rust",
                env!("CARGO_PKG_VERSION")
            ),
            false,
        );
        ui.close();
    }
}

/// The hint field: the project name over what the hovered control does,
/// otherwise the last message.
fn hint_field(ui: &mut egui::Ui, state: &UiState, frame: &Frame<'_>, width: f32) {
    let hint = if frame.settings.show_hints {
        state.hint.as_deref()
    } else {
        None
    };
    let (text, color) = match hint {
        Some(hint) => (hint, theme::TEXT),
        None if state.status.error => (state.status.text.as_str(), theme::DANGER),
        None => (state.status.text.as_str(), theme::TEXT_DIM),
    };
    let (rect, response) =
        ui.allocate_exact_size(vec2(width.max(60.0), icons::ROW), egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, theme::PANEL_DARK);
    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, theme::LCD_EDGE),
        egui::StrokeKind::Inside,
    );
    let font = egui::FontId::proportional(theme::FONT_CAPTION);
    let line = |text: &str, color, y| {
        let mut job =
            egui::text::LayoutJob::simple_singleline(text.to_owned(), font.clone(), color);
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 10.0);
        let galley = painter.layout_job(job);
        painter.galley(egui::pos2(rect.left() + 5.0, y), galley, color);
    };
    let name = frame.project.name();
    line(name, theme::TEXT_DIM, rect.top() + 1.0);
    if frame.history.unsaved {
        // A small dot after the project name while there are unsaved changes.
        let width = painter
            .layout_no_wrap(name.to_owned(), font.clone(), theme::TEXT_DIM)
            .size()
            .x;
        let at = egui::pos2(
            (rect.left() + 5.0 + width + 5.0).min(rect.right() - 5.0),
            rect.top() + 1.0 + theme::FONT_CAPTION * 0.65,
        );
        painter.circle_filled(at, 2.0, theme::HIGHLIGHT);
    }
    line(text, color, rect.center().y);
    let shown = text.to_owned();
    response.context_menu(|ui| {
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(shown.clone());
            ui.close();
        }
    });
}

/// Width the one-band toolbar needs: the menu column and both racks.
const ONE_BAND_WIDTH: f32 = 1000.0;
/// Width of the menu and hint column.
const MENU_COLUMN: f32 = 300.0;

pub fn show(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    if ui.available_width() >= ONE_BAND_WIDTH {
        one_band(ui, state, frame, actions);
    } else {
        two_bands(ui, state, frame, actions);
    }
}

fn band_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::PANEL)
        .inner_margin(egui::Margin::symmetric(6, 3))
}

/// The menu and hint column at the left, then the upper and lower racks
/// (see `docs/spec/window-layout.md`, "Toolbar").
fn one_band(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    egui::Panel::top("toolbar")
        .frame(band_frame())
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(MENU_COLUMN);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    menu_bar(ui, state, frame, actions);
                    hint_field(ui, state, frame, MENU_COLUMN);
                });
                ui.add_space(8.0);
                racks(ui, state, frame, actions, true);
            });
        });
}

/// The narrow fallback: menus, hint field and window buttons, then the racks.
fn two_bands(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    egui::Panel::top("menu").frame(band_frame()).show(ui, |ui| {
        ui.horizontal(|ui| {
            menu_bar(ui, state, frame, actions);
            ui.add_space(6.0);
            let width = ui.available_width() - 160.0;
            if width > 80.0 {
                hint_field(ui, state, frame, width);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                window_buttons(ui, state, actions);
            });
        });
    });
    egui::Panel::top("transport")
        .frame(band_frame())
        .show(ui, |ui| racks(ui, state, frame, actions, false));
}

/// The menu titles in their recessed field.
fn menu_bar(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    egui::Frame::new()
        .fill(theme::PANEL_DARK)
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(2, 0))
        .show(ui, |ui| {
            ui.set_min_height(icons::ROW - 2.0);
            egui::MenuBar::new().ui(ui, |ui| menus(ui, state, frame, actions));
        });
}

/// The upper rack (option toggles and displays) over the lower rack
/// (transport, tempo, pattern and window buttons).
fn racks(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
    with_windows: bool,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.horizontal(|ui| upper_rack(ui, state, frame, actions));
        ui.horizontal(|ui| {
            lower_rack(ui, state, frame, actions);
            if with_windows {
                divider(ui);
                window_buttons(ui, state, actions);
            }
        });
    });
}

fn menus(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let menu = |label: &str| RichText::new(label).size(theme::FONT_LABEL);
    ui.menu_button(menu("File"), |ui| file_menu(ui, state, actions));
    ui.menu_button(menu("Edit"), |ui| {
        let undo = match &frame.history.undo {
            Some(label) => format!("Undo {label}"),
            None => "Undo".to_owned(),
        };
        crate::widgets::enabled_ui(ui, frame.history.undo.is_some(), |ui| {
            menu_item(ui, state, &undo, "edit.undo", actions);
        });
        crate::widgets::enabled_ui(ui, frame.history.can_redo, |ui| {
            menu_item(ui, state, "Redo", "edit.redo", actions);
        });
        ui.separator();
        for (label, key, which) in [
            ("Cut", "Ctrl+X", crate::ClipboardKey::Cut),
            ("Copy", "Ctrl+C", crate::ClipboardKey::Copy),
            ("Paste", "Ctrl+V", crate::ClipboardKey::Paste),
        ] {
            let button = egui::Button::new(label).shortcut_text(key);
            if ui
                .add(button)
                .hint("Acts on the focused editor window")
                .clicked()
            {
                crate::request_clipboard(ui.ctx(), which);
                ui.close();
            }
        }
        ui.separator();
        let empty = frame.history.steps.is_empty() && frame.history.redo_steps.is_empty();
        crate::widgets::enabled_ui(ui, !empty, |ui| {
            ui.menu_button("History", |ui| history_menu(ui, frame, actions));
        });
    });
    ui.menu_button(menu("Add"), |ui| add_menu(ui, frame, actions));
    ui.menu_button(menu("Patterns"), |ui| {
        patterns_menu(ui, state, frame, actions)
    });
    ui.menu_button(menu("View"), |ui| view_menu(ui, state, frame, actions));
    ui.menu_button(menu("Options"), |ui| {
        options_menu(ui, state, frame, actions)
    });
    ui.menu_button(menu("Tools"), |ui| tools_menu(ui, frame, state, actions));
    ui.menu_button(menu("Help"), |ui| help_menu(ui, state));
}

/// Edit › History: redo steps (dim) above the current state, then undo steps
/// newest first. Picking one undoes or redoes until just after it.
fn history_menu(ui: &mut egui::Ui, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let history = &frame.history;
    let mut pick = None;
    for (i, step) in history.redo_steps.iter().enumerate().rev() {
        if ui
            .button(RichText::new(step).color(theme::TEXT_DIM))
            .clicked()
        {
            pick = Some(HistoryPick::Redo(i));
        }
    }
    ui.label(RichText::new("Now").color(theme::HIGHLIGHT));
    for (i, step) in history.steps.iter().enumerate() {
        if ui.button(step).clicked() {
            pick = Some(HistoryPick::Undo(i));
        }
    }
    if history.steps.len() < 15 && ui.button("Start of history").clicked() {
        pick = Some(HistoryPick::Undo(history.steps.len()));
    }
    if let Some(pick) = pick {
        let (name, times) = pick.runs();
        for _ in 0..times {
            run(actions, name);
        }
        ui.close();
    }
}

/// A row picked in Edit › History, counted from the current state.
#[derive(Debug, Clone, Copy)]
pub(crate) enum HistoryPick {
    /// The undo step this many back (0: the latest), keeping it.
    Undo(usize),
    /// The redo step this many ahead (0: the next), re-running it.
    Redo(usize),
}

impl HistoryPick {
    /// The command to repeat and how many times.
    pub(crate) fn runs(self) -> (&'static str, usize) {
        match self {
            Self::Undo(i) => ("edit.undo", i),
            Self::Redo(i) => ("edit.redo", i + 1),
        }
    }
}

/// The window buttons: playlist, channel rack, piano roll, browser, mixer.
fn window_buttons(ui: &mut egui::Ui, state: &mut UiState, actions: &mut Vec<Action>) {
    ui.spacing_mut().item_spacing.x = 1.0;
    let dragging = egui::DragAndDrop::has_payload_of_type::<crate::browser::Dragged>(ui.ctx());
    let w = &mut state.windows;
    for (icon, open, hint, which, window) in [
        (
            Icon::Playlist,
            &mut w.playlist,
            "Playlist (F5)",
            "playlist",
            "playlist-window",
        ),
        (
            Icon::ChannelRack,
            &mut w.channel_rack,
            "Channel rack (F6)",
            "channel_rack",
            "channel-rack",
        ),
        (
            Icon::PianoRoll,
            &mut w.piano_roll,
            "Piano roll (F7)",
            "piano_roll",
            "piano-roll",
        ),
        (
            Icon::Browser,
            &mut w.browser,
            "Browser (Alt+F8)",
            "browser",
            "",
        ),
        (
            Icon::Mixer,
            &mut w.mixer,
            "Mixer (F9)",
            "mixer",
            "mixer-window",
        ),
    ] {
        let response = icons::toggle(ui, icon, *open, hint);
        if response.clicked() {
            run(actions, &format!("view.toggle {which}"));
        }
        // The playlist and piano roll open maximized on right-click.
        if matches!(which, "playlist" | "piano_roll") && response.secondary_clicked() {
            run(actions, &format!("view.open_maximized {which}"));
        }
        // A browser item dragged over the button opens and raises its window,
        // so the drag can go on into it.
        if dragging && !window.is_empty() && response.contains_pointer() {
            *open = true;
            raise(ui.ctx(), egui::Id::new(window));
        }
    }
    layout_button(ui, actions);
}

/// The window layout button: its menu restores the default layout or
/// maximizes the playlist.
fn layout_button(ui: &mut egui::Ui, actions: &mut Vec<Action>) {
    let button = icons::toggle(ui, Icon::Layout, false, "Window layout");
    let toggle =
        (button.clicked() || button.secondary_clicked()).then_some(egui::SetOpenCommand::Toggle);
    egui::Popup::menu(&button).open_memory(toggle).show(|ui| {
        let items = [
            ("Default layout (Ctrl+Shift+H)", "view.default_layout"),
            (
                "Maximize or restore the playlist (Enter)",
                "view.maximize playlist",
            ),
            ("Piano roll maximized", "view.open_maximized piano_roll"),
        ];
        for (label, invocation) in items {
            if ui.button(label).clicked() {
                run(actions, invocation);
            }
        }
    });
}

/// Puts a workspace window on top of the others.
fn raise(ctx: &egui::Context, id: egui::Id) {
    ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, id));
}

/// A thin vertical line between groups.
fn divider(ui: &mut egui::Ui) {
    ui.add_space(3.0);
    let (rect, _) = ui.allocate_exact_size(vec2(1.0, icons::ROW - 4.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 0.0, theme::SEPARATOR);
    ui.add_space(3.0);
}

/// Buttons in a group nearly touch.
fn group(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.spacing_mut().item_spacing.x = 1.0;
    add(ui);
    ui.spacing_mut().item_spacing.x = 4.0;
}

/// A latching option toggle: an accent fill with a dark glyph while on.
fn option_toggle(
    ui: &mut egui::Ui,
    state: &UiState,
    (icon, on, name): (Icon, bool, &str),
    invocation: &str,
) -> bool {
    let key = shortcut(state, invocation);
    let word = if on { "on" } else { "off" };
    let hint = if key.is_empty() {
        format!("{name}: {word}")
    } else {
        format!("{name}: {word} ({key})")
    };
    icons::button(ui, icon, on, theme::ACCENT, &hint).clicked()
}

/// Option toggles, the time display, the output monitor and the performance
/// panel.
fn upper_rack(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    let record = project.record;
    group(ui, |ui| {
        for (icon, on, name, invocation) in [
            (
                Icon::Metronome,
                project.metronome,
                "Metronome",
                "transport.toggle_metronome",
            ),
            (
                Icon::Wait,
                record.wait_for_input,
                "Wait for input",
                "record.toggle_wait",
            ),
            (
                Icon::CountIn,
                record.count_in,
                "Count-in",
                "record.toggle_count_in",
            ),
            (
                Icon::Blend,
                record.mode == musiccraft_project::RecordMode::Blend,
                "Blend recording (off: replace)",
                "",
            ),
            (
                Icon::LoopRecord,
                record.loop_record,
                "Loop recording",
                "record.toggle_loop",
            ),
            (
                Icon::Keyboard,
                state.typing_keyboard,
                "Computer keyboard as piano",
                "ui.typing_keyboard",
            ),
            (
                Icon::StepEdit,
                project.step_edit(),
                "Step edit",
                "record.toggle_step_edit",
            ),
        ] {
            if !option_toggle(ui, state, (icon, on, name), invocation) {
                continue;
            }
            match icon {
                Icon::Keyboard => state.typing_keyboard = !state.typing_keyboard,
                Icon::Blend => {
                    let mode = if on { "replace" } else { "blend" };
                    run(actions, &format!("record.set_mode {mode}"));
                }
                _ => run(actions, invocation),
            }
        }
    });
    divider(ui);
    time_display(ui, state, frame);
    divider(ui);
    let recent = frame.engine.scope.len().saturating_sub(256);
    let scope = frame.engine.scope.get(recent..).unwrap_or_default();
    let levels = state.meters.first().copied().unwrap_or_default();
    if icons::output_monitor(ui, scope, levels, 124.0)
        .hint("Master output · click for the spectrum analyser")
        .clicked()
    {
        state.windows.spectrum = !state.windows.spectrum;
    }
    ui.add_space(2.0);
    icons::performance(ui, frame.engine.load, frame.engine.voices).hint(format!(
        "Audio load {:.0}% · {} voices · {} · {}",
        frame.engine.load * 100.0,
        frame.engine.voices,
        frame.engine.audio,
        frame.engine.midi
    ));
    divider(ui);
    utilities(ui, state, frame, actions);
}

/// The utility shortcuts: undo, sample editor, record audio, help, save and
/// export. None of them latches.
fn utilities(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    group(ui, |ui| {
        let undo = frame.history.undo.as_ref().map_or_else(
            || "Undo (nothing to undo)".to_owned(),
            |l| format!("Undo {l} (Ctrl+Z)"),
        );
        let button = crate::widgets::enabled_ui(ui, frame.history.undo.is_some(), |ui| {
            icons::button(ui, Icon::Undo, false, theme::ACCENT, &undo)
        });
        if button.clicked() {
            run(actions, "edit.undo");
        }
        button.context_menu(|ui| {
            if crate::widgets::add_enabled(
                ui,
                frame.history.can_redo,
                egui::Button::new("Redo (Ctrl+Y)"),
            )
            .clicked()
            {
                run(actions, "edit.redo");
                ui.close();
            }
        });
        if icons::button(
            ui,
            Icon::Wave,
            false,
            theme::ACCENT,
            "Sample editor: the selected channel's window",
        )
        .clicked()
        {
            state.windows.channel_settings = true;
            state.channel_tab = crate::channel_settings::ChannelTab::Sound;
        }
        let recording = frame.project.is_armed();
        if icons::button(
            ui,
            Icon::RecordAudio,
            recording && state.record_audio,
            theme::DANGER,
            "Record audio: capture the audio input into the playlist",
        )
        .clicked()
        {
            state.record_audio = true;
            run(actions, "record.toggle");
        }
        let help = icons::button(
            ui,
            Icon::Help,
            false,
            theme::ACCENT,
            "Help: keyboard shortcuts (F1)",
        );
        if help.clicked() {
            state.windows.shortcuts = !state.windows.shortcuts;
        }
        let save = icons::button(
            ui,
            Icon::Save,
            false,
            theme::ACCENT,
            "Save (Ctrl+S) · right-click: save as, save new version",
        );
        if save.clicked() {
            run(actions, "ui.save");
        }
        save.context_menu(|ui| {
            for (label, invocation) in [
                ("Save as… (Ctrl+Shift+S)", "ui.save_as"),
                ("Save new version (Ctrl+N)", "ui.save_new_version"),
            ] {
                if ui.button(label).clicked() {
                    run(actions, invocation);
                    ui.close();
                }
            }
        });
        let export = icons::button(
            ui,
            Icon::Export,
            false,
            theme::ACCENT,
            "Export audio (Ctrl+R) · right-click: stems, MIDI",
        );
        if export.clicked() {
            run(actions, "ui.export");
        }
        export.context_menu(|ui| {
            for (label, invocation) in [
                ("Export stems…", "ui.export_stems"),
                ("Export MIDI… (Ctrl+Shift+M)", "ui.export_midi"),
            ] {
                if ui.button(label).clicked() {
                    run(actions, invocation);
                    ui.close();
                }
            }
        });
    });
}

/// The song position: bar.beat.step (last field smaller) or minutes and
/// seconds; COUNT-IN or WAITING while recording waits to start.
fn time_display(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>) {
    use crate::TimeMode;
    let project = frame.project;
    let ticks = frame.engine.position_ticks.max(0.0);
    let seconds = ticks / f64::from(musiccraft_project::params::PPQ) * 60.0 / project.tempo_bpm();
    let time = format!("{}:{:05.2}", (seconds / 60.0) as u32, seconds % 60.0);
    let meter = project.playing_meter();
    let bars = position_label(ticks, &meter);
    let mode = state.time_mode;
    let (text, caption, sep) = match mode {
        TimeMode::Bars => (bars.clone(), "BARS", '.'),
        TimeMode::Time => (time.clone(), "TIME", '.'),
        TimeMode::Ticks => (crate::geometry::tick_label(ticks, &meter), "TICKS", '.'),
    };
    // The least-significant field, drawn smaller.
    let (digits, tail, caption) = if frame.engine.counting_in {
        ("COUNT-IN".to_owned(), String::new(), "REC")
    } else if frame.engine.waiting {
        ("WAITING".to_owned(), String::new(), "REC")
    } else {
        let (head, last) = text.rsplit_once(sep).unwrap_or((text.as_str(), ""));
        (head.to_owned(), format!("{sep}{last}"), caption)
    };
    let other = if mode == TimeMode::Time { &bars } else { &time };
    let position = icons::display(ui, 100.0, (&digits, &tail), caption, egui::Sense::click()).hint(
        format!("Song position ({other}) · click to switch bars and time · right-click for modes"),
    );
    if position.clicked() {
        state.time_mode = if mode == TimeMode::Time {
            TimeMode::Bars
        } else {
            TimeMode::Time
        };
    }
    position.context_menu(|ui| {
        for (choice, label) in [
            (TimeMode::Bars, "Bars, beats and steps"),
            (TimeMode::Time, "Minutes and seconds"),
            (TimeMode::Ticks, "Bars, beats and ticks"),
        ] {
            if ui.radio(state.time_mode == choice, label).clicked() {
                state.time_mode = choice;
                ui.close();
            }
        }
    });
}

/// The master knob, the mode switch and transport, tempo, signature and the
/// pattern panel.
fn lower_rack(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    let playing = project.transport().is_playing();
    if let Some(master) = project.mixer.first() {
        let knob = widgets::knob(
            ui,
            master.volume,
            0.0,
            musiccraft_project::MAX_VOLUME,
            1.0,
            false,
        );
        if knob.started {
            state.begin_gesture();
        }
        if let Some(value) = knob.changed {
            actions.push(Action::Coalesced(
                Invocation::new("mixer.set_volume", ["0".to_owned(), format!("{value:.3}")]),
                state.gesture(),
            ));
        }
        knob.response
            .hint(format!("Master volume: {}", format::level(master.volume)));
    }
    divider(ui);
    group(ui, |ui| {
        let song = project.play_mode == PlayMode::Song;
        let (switch, over) = icons::mode_switch(
            ui,
            song,
            "Pattern or song playback (L) · right-click a half to open the channel rack or playlist",
        );
        if switch.clicked() {
            let label = if song { "pattern" } else { "song" };
            run(actions, &format!("transport.set_mode {label}"));
        }
        if switch.secondary_clicked() {
            match over {
                Some(0) => state.windows.channel_rack = true,
                Some(_) => state.windows.playlist = true,
                None => {}
            }
        }
        ui.add_space(3.0);
        let (icon, hint) = if playing {
            (Icon::Pause, "Stop (Space)")
        } else {
            (Icon::Play, "Play (Space)")
        };
        if icons::button(ui, icon, playing, theme::ACCENT, hint).clicked() {
            run(actions, "transport.toggle");
        }
        let stop = icons::button(
            ui,
            Icon::Stop,
            false,
            theme::TEXT,
            "Stop and return to the start · double-click to silence every sound",
        );
        if stop.double_clicked() {
            run(actions, "transport.panic");
        } else if stop.clicked() {
            run(actions, "transport.stop");
        }
        ui.add_space(3.0);
        let record_hint = format!(
            "Record ({}) · right-click for options",
            shortcut(state, "record.toggle")
        );
        let armed = project.is_armed();
        let taking = armed && (playing || frame.engine.counting_in || frame.engine.waiting);
        let rec = icons::button(
            ui,
            Icon::Record,
            armed,
            record_tone(ui, taking),
            &record_hint,
        );
        if rec.clicked() {
            run(actions, "record.toggle");
        }
        rec.context_menu(|ui| record_options(ui, state, project, actions));
    });
    divider(ui);
    tempo(ui, state, project, actions);
    group(ui, |ui| {
        ui.add_space(2.0);
        let tap = ui.small_button("TAP").hint(
            "Tap the beat a few times to set the tempo · right-click for the tap tempo window",
        );
        if tap.clicked() {
            crate::tap_tempo::tap(ui, state, actions);
        }
        if tap.secondary_clicked() {
            state.windows.tap_tempo = true;
        }
        let sig = project.time_signature;
        ui.menu_button(RichText::new(sig.to_string()).monospace(), |ui| {
            if let Some(new) = signature::picker(ui, Some(sig), &mut state.sig_draft) {
                run(actions, &format!("project.set_time_signature {new}"));
            }
        })
        .response
        .hint(
            "Time signature of the song and of new patterns (changes: right-click the playlist ruler)",
        );
    });
    divider(ui);
    pattern_panel(ui, state, frame, actions);
}

/// Record's fill: the danger tone, pulsing while a take runs.
fn record_tone(ui: &egui::Ui, taking: bool) -> egui::Color32 {
    if !taking {
        return theme::DANGER;
    }
    ui.ctx().request_repaint();
    let t = ui.input(|i| i.time);
    let strength = 0.6 + 0.4 * (t * std::f64::consts::PI).sin().abs() as f32;
    theme::mix(theme::PANEL, theme::DANGER, strength)
}

/// Round tempos offered on the tempo display's menu.
const ROUND_TEMPOS: [f64; 5] = [60.0, 90.0, 120.0, 140.0, 174.0];

/// The tempo a drag on the display reaches: `distance` points upwards from
/// where it started at `start` BPM, on the whole digits or the decimals.
fn dragged_tempo(start: f64, on_whole: bool, distance: f32, fine: bool) -> f64 {
    let distance = f64::from(distance);
    let change = match (on_whole, fine) {
        (true, false) => (distance / 6.0).trunc(),
        (true, true) => (distance / 6.0).trunc() * 0.1,
        (false, false) => distance * 0.01,
        (false, true) => distance * 0.001,
    };
    let value = start + change;
    // Whole-digit drags keep the decimals; others land on the display's grid.
    let value = if on_whole {
        value
    } else {
        (value * 1000.0).round() / 1000.0
    };
    value.clamp(MIN_TEMPO_BPM, MAX_TEMPO_BPM)
}

/// The tempo display: whole BPM large, decimals small. Drag either part,
/// scroll, double-click to type, right-click for the menu.
fn tempo(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &musiccraft_project::Project,
    actions: &mut Vec<Action>,
) {
    let tempo = project.tempo_bpm();
    let text = format!("{tempo:.3}");
    let (whole, decimals) = text.split_once('.').unwrap_or((text.as_str(), "000"));
    let id = egui::Id::new("tempo-display");
    let typing = ui.data(|d| d.get_temp::<String>(id.with("typing")));
    if let Some(typed) = typing {
        tempo_entry(ui, state, id, typed, actions);
        return;
    }
    let (lcd, split) = icons::display_split(
        ui,
        74.0,
        (whole, &format!(".{decimals}")),
        "BPM",
        egui::Sense::click_and_drag(),
    );
    let lcd = lcd.hint(format!(
        "Tempo {tempo:.3} BPM · drag the whole digits or the decimals (Shift: fine) · double-click to type"
    ));
    let set = |actions: &mut Vec<Action>, state: &UiState, value: f64| {
        actions.push(Action::Coalesced(
            Invocation::new("project.set_tempo", [format!("{value:.3}")]),
            state.gesture(),
        ));
    };
    let fine = ui.input(|i| i.modifiers.shift);
    if lcd.drag_started() {
        state.begin_gesture();
        let on_whole = ui
            .input(|i| i.pointer.press_origin())
            .is_some_and(|p| p.x < split);
        ui.data_mut(|d| d.insert_temp(id.with("drag"), (tempo, on_whole)));
    }
    if lcd.dragged() {
        let start = ui.data(|d| d.get_temp::<(f64, bool)>(id.with("drag")));
        let origin = ui.input(|i| i.pointer.press_origin());
        if let (Some((start, on_whole)), Some(origin), Some(now)) =
            (start, origin, lcd.interact_pointer_pos())
        {
            let value = dragged_tempo(start, on_whole, origin.y - now.y, fine);
            if (value - tempo).abs() > 1e-9 {
                set(actions, state, value);
            }
        }
    }
    let notch = if lcd.hovered() {
        crate::scrollbars::wheel_notches(ui)
    } else {
        0.0
    };
    if notch != 0.0 {
        state.begin_gesture();
        let step = if fine { 0.01 } else { 1.0 };
        let value = (tempo + f64::from(notch) * step).clamp(MIN_TEMPO_BPM, MAX_TEMPO_BPM);
        set(actions, state, value);
    }
    if lcd.double_clicked() {
        ui.data_mut(|d| d.insert_temp(id.with("typing"), format!("{tempo:.3}")));
    }
    // The chevrons step 1 BPM, or 0.001 when the decimals were dragged last.
    let on_whole = ui
        .data(|d| d.get_temp::<(f64, bool)>(id.with("drag")))
        .is_none_or(|(_, whole)| whole);
    let step = if on_whole { 1.0 } else { 0.001 };
    let (rect, chevrons) = ui.allocate_exact_size(vec2(10.0, 22.0), Sense::click());
    let chevrons = chevrons.hint(format!("Tempo up or down {step} BPM"));
    let (up, down) = step_arrows(ui.painter(), rect);
    let pointer = chevrons.interact_pointer_pos();
    let sign = match pointer {
        Some(p) if chevrons.clicked() && up.contains(p) => 1.0,
        Some(p) if chevrons.clicked() && down.contains(p) => -1.0,
        _ => 0.0,
    };
    if sign != 0.0 {
        state.begin_gesture();
        let value = ((tempo + sign * step) * 1000.0).round() / 1000.0;
        set(actions, state, value.clamp(MIN_TEMPO_BPM, MAX_TEMPO_BPM));
    }
    let copied = ui.data(|d| d.get_temp::<f64>(id.with("copied")));
    lcd.context_menu(|ui| {
        if ui.button("Enter a tempo…").clicked() {
            ui.data_mut(|d| d.insert_temp(id.with("typing"), format!("{tempo:.3}")));
            ui.close();
        }
        if ui.button("Tap tempo").clicked() {
            state.windows.tap_tempo = true;
            ui.close();
        }
        ui.separator();
        for round in ROUND_TEMPOS {
            if ui.button(format!("{round:.0} BPM")).clicked() {
                state.begin_gesture();
                set(actions, state, round);
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(format!("{tempo:.3}"));
            ui.data_mut(|d| d.insert_temp(id.with("copied"), tempo));
            ui.close();
        }
        let paste = egui::Button::new("Paste");
        if crate::widgets::add_enabled(ui, copied.is_some(), paste).clicked() {
            if let Some(value) = copied {
                state.begin_gesture();
                set(actions, state, value);
            }
            ui.close();
        }
        let reset = musiccraft_project::DEFAULT_TEMPO_BPM;
        if ui.button(format!("Reset to {reset:.0}")).clicked() {
            state.begin_gesture();
            set(actions, state, reset);
            ui.close();
        }
    });
}

/// The tempo display's type-in field: Enter applies, Esc cancels.
fn tempo_entry(
    ui: &mut egui::Ui,
    state: &mut UiState,
    id: egui::Id,
    mut typed: String,
    actions: &mut Vec<Action>,
) {
    let field = ui.add(
        egui::TextEdit::singleline(&mut typed)
            .id(id.with("field"))
            .desired_width(74.0 - 8.0)
            .font(egui::FontId::monospace(theme::FONT_VALUE)),
    );
    if !field.has_focus() && !field.lost_focus() {
        field.request_focus();
    }
    // Taken here so neither key also reaches the window shortcuts.
    let (enter, escape) = ui.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if escape || (field.lost_focus() && !enter) {
        ui.data_mut(|d| d.remove::<String>(id.with("typing")));
    } else if enter {
        ui.data_mut(|d| d.remove::<String>(id.with("typing")));
        if let Ok(value) = typed.trim().parse::<f64>() {
            state.begin_gesture();
            actions.push(Action::Coalesced(
                Invocation::new(
                    "project.set_tempo",
                    [format!("{:.3}", value.clamp(MIN_TEMPO_BPM, MAX_TEMPO_BPM))],
                ),
                state.gesture(),
            ));
        }
    } else {
        ui.data_mut(|d| d.insert_temp(id.with("typing"), typed));
    }
}

/// Draws a pair of small up and down chevrons stacked in `rect` and returns
/// the upper and lower halves.
fn step_arrows(painter: &egui::Painter, rect: Rect) -> (Rect, Rect) {
    let (up, down) = (
        Rect::from_min_max(rect.min, pos2(rect.right(), rect.center().y)),
        Rect::from_min_max(pos2(rect.left(), rect.center().y), rect.max),
    );
    for (half, tip) in [(up, -1.0), (down, 1.0)] {
        let c = half.center();
        let stroke = egui::Stroke::new(1.0, theme::TEXT_DIM);
        painter.line_segment(
            [pos2(c.x - 3.0, c.y - tip * 1.5), pos2(c.x, c.y + tip * 1.5)],
            stroke,
        );
        painter.line_segment(
            [pos2(c.x, c.y + tip * 1.5), pos2(c.x + 3.0, c.y - tip * 1.5)],
            stroke,
        );
    }
    (up, down)
}

/// The pattern list and the plus that adds the next empty pattern.
fn pattern_panel(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    let current = project.current_pattern;
    let options = icons::header_button(ui, Icon::ChevronDown, false, false, "Pattern options");
    let toggle =
        (options.clicked() || options.secondary_clicked()).then_some(egui::SetOpenCommand::Toggle);
    egui::Popup::menu(&options)
        .open_memory(toggle)
        .show(|ui| patterns_menu(ui, state, frame, actions));
    let (rect, field) = ui.allocate_exact_size(vec2(118.0, 22.0), Sense::click());
    let field = field.hint(
        "Pattern: click for the list · wheel or arrows step (Plus / Minus) · double-click renames · Ctrl+click shows it in the playlist",
    );
    let arrows = Rect::from_min_max(pos2(rect.right() - 12.0, rect.top()), rect.max);
    let painter = ui.painter();
    let fill = if field.hovered() {
        theme::RAISED_HOVER
    } else {
        theme::RAISED
    };
    painter.rect_filled(rect, 2.0, fill);
    painter.text(
        pos2(rect.left() + 6.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        format!("{}  {}", current + 1, project.current_pattern().name),
        egui::FontId::proportional(theme::FONT_LABEL),
        crate::pattern_text(project.current_pattern()),
    );
    let (up, down) = step_arrows(painter, arrows);
    let pointer = field.interact_pointer_pos();
    let on = |r: Rect| pointer.is_some_and(|p| r.contains(p));
    let ctrl = ui.input(|i| i.modifiers.command);
    let mut list = None;
    if field.clicked() && on(up) {
        run(actions, "pattern.prev");
    } else if field.clicked() && on(down) {
        run(actions, "pattern.next");
    } else if field.double_clicked() {
        state.rename_in_place(
            crate::dialogs::Dialog::rename_pattern(current, &project.current_pattern().name),
            rect,
        );
    } else if field.clicked() && ctrl {
        state.windows.playlist = true;
        crate::playlist::show_pattern(state, project, project.current_pattern().id);
    } else if field.clicked() || field.secondary_clicked() {
        list = Some(egui::SetOpenCommand::Toggle);
    }
    if field.hovered() {
        let wheel = crate::scrollbars::wheel_notches(ui);
        if wheel > 0.0 {
            run(actions, "pattern.prev");
        } else if wheel < 0.0 {
            run(actions, "pattern.next");
        }
    }
    egui::Popup::menu(&field)
        .open_memory(list)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            for (i, pattern) in project.patterns.iter().enumerate() {
                let text = RichText::new(format!("{}  {}", i + 1, pattern.name))
                    .color(crate::pattern_text(pattern));
                if ui.selectable_label(i == current, text).clicked() {
                    run(actions, &format!("pattern.select {}", i + 1));
                }
            }
        });
    let add = icons::button(
        ui,
        Icon::Plus,
        false,
        theme::ACCENT,
        "New pattern · right-click: first empty pattern (Shift+F4) · F4: next empty pattern",
    );
    if add.clicked() {
        run(actions, "pattern.add");
    }
    if add.secondary_clicked() {
        run(actions, "pattern.first_empty");
    }
}

/// The REC button's menu: count-in, wait for input, loop, blend/replace and
/// step edit.
fn record_options(
    ui: &mut egui::Ui,
    state: &UiState,
    project: &musiccraft_project::Project,
    actions: &mut Vec<Action>,
) {
    let options = project.record;
    let mut toggle = |ui: &mut egui::Ui, on: bool, label: &str, name: &str| {
        let text = match shortcut(state, name) {
            s if s.is_empty() => label.to_owned(),
            s => format!("{label} ({s})"),
        };
        if ui.selectable_label(on, text).clicked() {
            run(actions, name);
            ui.close();
        }
    };
    toggle(
        ui,
        options.count_in,
        "Count-in (one bar)",
        "record.toggle_count_in",
    );
    toggle(
        ui,
        options.wait_for_input,
        "Wait for input",
        "record.toggle_wait",
    );
    toggle(
        ui,
        options.loop_record,
        "Loop recording",
        "record.toggle_loop",
    );
    toggle(
        ui,
        project.step_edit(),
        "Step edit",
        "record.toggle_step_edit",
    );
    ui.separator();
    for mode in [
        musiccraft_project::RecordMode::Blend,
        musiccraft_project::RecordMode::Replace,
    ] {
        if ui
            .selectable_label(options.mode == mode, mode.label())
            .clicked()
        {
            actions.push(Action::Run(Invocation::new(
                "record.set_mode",
                [mode.label().to_owned()],
            )));
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HistoryPick, dragged_tempo};

    #[test]
    fn history_picks_undo_or_redo_to_just_after_the_step() {
        assert_eq!(HistoryPick::Undo(0).runs(), ("edit.undo", 0));
        assert_eq!(HistoryPick::Undo(3).runs(), ("edit.undo", 3));
        assert_eq!(HistoryPick::Redo(0).runs(), ("edit.redo", 1));
        assert_eq!(HistoryPick::Redo(2).runs(), ("edit.redo", 3));
    }

    #[test]
    fn tempo_drags_step_whole_digits_or_decimals() {
        // Whole digits: 1 BPM per 6 points, keeping the decimals.
        assert!((dragged_tempo(120.5, true, 13.0, false) - 122.5).abs() < 1e-9);
        assert!((dragged_tempo(120.5, true, -5.0, false) - 120.5).abs() < 1e-9);
        assert!((dragged_tempo(120.0, true, 12.0, true) - 120.2).abs() < 1e-9);
        // Decimals: 0.01 BPM per point, 0.001 with Shift.
        assert!((dragged_tempo(120.0, false, 10.0, false) - 120.1).abs() < 1e-9);
        assert!((dragged_tempo(120.0, false, -3.0, true) - 119.997).abs() < 1e-9);
        // The range holds.
        assert!(
            (dragged_tempo(15.0, true, -600.0, false) - musiccraft_project::MIN_TEMPO_BPM).abs()
                < 1e-9
        );
    }
}
