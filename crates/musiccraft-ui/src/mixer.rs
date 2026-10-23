//! The mixer: one strip per track plus the selected track's effect chain.

use crate::hint::Hint;
use egui::{Color32, RichText, vec2};
use musiccraft_commands::Invocation;
use musiccraft_project::params::{EFFECT_KINDS, MAX_EFFECT_SLOTS};
use musiccraft_project::{Dock, MAX_VOLUME, MixerTrack, Project};

use crate::dialogs::Dialog;
use crate::icons::{self, Icon};
use crate::{Action, Frame, UiState, format, theme, widgets};

const STRIP_WIDTH: f32 = 40.0;

fn run(actions: &mut Vec<Action>, name: &str, args: impl IntoIterator<Item = String>) {
    actions.push(Action::Run(Invocation::new(name, args)));
}

/// The mixer window: the strips, then the selected track's slot panel.
pub fn show(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    let mut open = state.windows.mixer;
    let area = state.workspace;
    let title = match project.mixer.get(state.selected_track) {
        Some(track) if state.selected_track > 0 => format!("Mixer — {}", track.name),
        _ => "Mixer — Master".to_owned(),
    };
    let id = egui::Id::new("mixer-window");
    if !open {
        return;
    }
    // The track whose menu was open last frame lifts while it stays open.
    ctx.data_mut(|d| {
        let target = d.remove_temp::<usize>(menu_target_id());
        match target {
            Some(index) => {
                d.insert_temp(menu_target_id().with("last"), index);
            }
            None => d.remove::<usize>(menu_target_id().with("last")),
        }
    });
    // At the top, right of the channel rack, up to the right edge, a little
    // under half the height.
    let slots = state.floating_area();
    let rack_right = ctx
        .memory(|m| m.area_rect(egui::Id::new("channel-rack")))
        .map_or(slots.left(), |r| r.right());
    let left = rack_right.max(slots.left() + slots.width() / 3.0) + 4.0;
    let default = egui::Rect::from_min_max(
        egui::pos2(left.min(slots.right() - 500.0), slots.top() + 4.0),
        egui::pos2(
            slots.right() - 4.0,
            slots.top() + slots.height() * 0.42 - 4.0,
        ),
    );
    // The rack takes a few frames to reach its width, so the mixer follows
    // its default slot for its first frames.
    let settle = egui::Id::new("mixer-settle");
    // A window put back from a saved layout skips this.
    if crate::chrome::placing(ctx, id) {
        ctx.data_mut(|d| d.insert_temp(settle, 4_u8));
    }
    if crate::chrome::focused(ctx, id) {
        keys(ctx, state, project, actions);
    }
    let frames = ctx.data(|d| d.get_temp::<u8>(settle)).unwrap_or(0);
    let mut window = crate::chrome::window(ctx, id, "Mixer", area, default, true);
    if frames < 4 {
        ctx.data_mut(|d| d.insert_temp(settle, frames + 1));
        window = window.current_pos(default.min);
    }
    let window = window
        .default_height(default.height().max(STRIP_MIN_HEIGHT))
        .min_width(320.0)
        .show(ctx, |ui| {
            let folded = crate::chrome::Header::new(id, &title).title_at_end().show(
                ui,
                &mut open,
                &mut (&mut *state, &mut *actions),
                |ui, (state, actions)| window_menu(ui, state, project, actions),
                |ui, (state, actions)| header_tools(ui, state, project, actions),
            );
            if folded {
                return;
            }
            // Room under the strips for their scroll bar and the gap above
            // it; any less and the window grows a little every frame.
            let bar = crate::scrollbars::BAR + ui.spacing().item_spacing.y;
            let height = (ui.available_height() - bar).max(STRIP_MIN_HEIGHT);
            ui.horizontal_top(|ui| {
                // The panel, the separator and the gaps around it, so the
                // row never outgrows the window.
                let panel = if state.mixer_panel && !state.mixer_panel_left {
                    EFFECTS_WIDTH + 2.0 * ui.spacing().item_spacing.x + 8.0
                } else {
                    0.0
                };
                if state.mixer_panel && state.mixer_panel_left {
                    effects(ui, state, frame, actions);
                    ui.separator();
                }
                // The master and the tracks pinned left sit in a fixed left
                // dock, the tracks pinned right in a fixed right dock; darker
                // dividers separate them from the scrolling middle.
                let docked = |dock: Dock| {
                    project.mixer.iter().enumerate().filter(move |(i, t)| {
                        if *i == 0 {
                            dock == Dock::Left
                        } else {
                            t.dock == dock
                        }
                    })
                };
                let right = docked(Dock::Right).count();
                let right_width = if right == 0 {
                    0.0
                } else {
                    right as f32 * (STRIP_WIDTH + 2.0) + DOCK_DIVIDER + ui.spacing().item_spacing.x
                };
                ui.spacing_mut().item_spacing.x = 2.0;
                for (index, track) in docked(Dock::Left) {
                    strip(ui, state, project, index, track, height, actions);
                }
                dock_divider(ui, height);
                // The wheel over a strip's name or empty space steps whole
                // strips; over a knob or fader it is left to the control.
                let strip_bars = egui::Id::new(STRIP_BARS);
                let area = egui::ScrollArea::horizontal()
                    .scroll_source(egui::scroll_area::ScrollSource {
                        mouse_wheel: false,
                        ..egui::scroll_area::ScrollSource::ALL
                    })
                    .id_salt("mixer-strips")
                    .max_width((ui.available_width() - panel - right_width).max(120.0))
                    .auto_shrink([!state.mixer_panel, true]);
                // The strips' bar sits under them, in the room the strip height
                // leaves free.
                ui.vertical(|ui| {
                    let bars = (strip_bars, 42.0);
                    crate::scrollbars::plain_area(ui, area, bars, |ui| {
                        // Behind the strips, so the controls on top keep
                        // their wheel and only empty space steps strips.
                        let behind = ui.interact(
                            ui.clip_rect(),
                            egui::Id::new("mixer-strip-space"),
                            egui::Sense::hover(),
                        );
                        step_strips_on_wheel(ui, &behind);
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            for (index, track) in docked(Dock::Middle) {
                                strip(ui, state, project, index, track, height, actions);
                            }
                        });
                    });
                });
                if right > 0 {
                    dock_divider(ui, height);
                    for (index, track) in docked(Dock::Right) {
                        strip(ui, state, project, index, track, height, actions);
                    }
                }
                if state.mixer_panel && !state.mixer_panel_left {
                    ui.separator();
                    effects(ui, state, frame, actions);
                }
            });
        });
    state.windows.mixer = open;
    crate::chrome::remember(ctx, id, window.as_ref().map(|w| &w.response));
}

/// Width of the divider between the master's dock and the inserts.
const DOCK_DIVIDER: f32 = 10.0;

/// The dark gap between the left dock and the scrolling inserts.
fn dock_divider(ui: &mut egui::Ui, height: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(DOCK_DIVIDER, height), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect.shrink2(vec2(2.0, 0.0)), 1.0, theme::PANEL_DARK);
}

/// The shortest the strips get; the fader gives up travel first.
const STRIP_MIN_HEIGHT: f32 = 300.0;

/// The header's tools: strip details, slot panel, add insert.
fn header_tools(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    actions: &mut Vec<Action>,
) {
    if icons::header_button(
        ui,
        Icon::Details,
        state.mixer_details,
        false,
        "Strip details: show or hide phase, swap, separation and effects",
    )
    .clicked()
    {
        state.mixer_details = !state.mixer_details;
    }
    if icons::header_button(
        ui,
        Icon::SidePanel,
        state.mixer_panel,
        false,
        "Slot panel: show or hide the selected track's effects",
    )
    .clicked()
    {
        state.mixer_panel = !state.mixer_panel;
    }
    let add = icons::header_button(
        ui,
        Icon::Plus,
        false,
        false,
        "Add an insert track (right-click: more)",
    );
    // New inserts go after the last; the first of them becomes selected.
    let mut added = 0;
    if add.clicked() {
        added = 1;
    }
    add.context_menu(|ui| {
        if ui.button("Add one").clicked() {
            added = 1;
        }
        ui.menu_button("Add several", |ui| {
            for count in [2, 4, 8] {
                if ui.button(format!("{count} inserts")).clicked() {
                    added = count;
                    ui.close();
                }
            }
        });
    });
    let room = musiccraft_project::params::MAX_MIXER_TRACKS.saturating_sub(project.mixer.len());
    let added = added.min(room);
    if added > 0 {
        let adds = (0..added)
            .map(|_| Invocation::bare("mixer.add_track"))
            .collect();
        actions.extend(crate::piano_roll::batch(adds).map(Action::Run));
        state.selected_track = project.mixer.len();
        state.mixer_selection.clear();
    }
}

/// The mixer's own window-menu items.
fn window_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    actions: &mut Vec<Action>,
) {
    ui.menu_button("View", |ui| {
        ui.checkbox(&mut state.mixer_details, "Strip details");
        ui.checkbox(&mut state.mixer_panel, "Slot panel");
        ui.checkbox(&mut state.mixer_panel_left, "Slot panel on the left");
        ui.checkbox(&mut state.mixer_hairlines, "Hairlines between strips");
        ui.menu_button("Strip colour", |ui| {
            for (tint, label) in TINT_NAMES.iter().enumerate() {
                let tint = tint as u8;
                if ui.radio(state.mixer_tint == tint, *label).clicked() {
                    state.mixer_tint = tint;
                    ui.close();
                }
            }
        });
    });
    let index = state.selected_track;
    let Some(track) = project.mixer.get(index) else {
        return;
    };
    ui.menu_button("Tracks", |ui| {
        if ui.button("Rename…").clicked() {
            state.open_dialog(Dialog::rename_track(index, &track.name));
            ui.close();
        }
        colour_menu(ui, index, track, actions);
        ui.separator();
        let tracks = selection(state);
        if ui.button("Reset selected tracks").clicked() {
            run_each(actions, "mixer.reset_track", &tracks, "");
            ui.close();
        }
        // Highest first, so the numbers still to remove stay put.
        let inserts: Vec<usize> = tracks.iter().rev().copied().filter(|&t| t > 0).collect();
        let delete = RichText::new("Delete selected tracks").color(theme::DANGER);
        if widgets::add_enabled(ui, !inserts.is_empty(), egui::Button::new(delete)).clicked() {
            run_each(actions, "mixer.remove_track", &inserts, "");
            select(state, inserts.last().map_or(0, |t| t - 1), false);
            ui.close();
        }
        let idle = idle_inserts(project);
        let clear = egui::Button::new("Clear out idle inserts");
        if widgets::add_enabled(ui, !idle.is_empty(), clear)
            .hint("Deletes inserts nothing plays through: no channel, no incoming send, no effects, default settings")
            .clicked()
        {
            run_each(actions, "mixer.remove_track", &idle, "");
            select(state, 0, false);
            ui.close();
        }
    });
}

/// Inserts nothing plays through, highest first: no channel routed to them,
/// no send into them, and every setting but the name at its default.
fn idle_inserts(project: &Project) -> Vec<usize> {
    (1..project.mixer.len())
        .rev()
        .filter(|&t| {
            let track = &project.mixer[t];
            let fresh = MixerTrack {
                name: track.name.clone(),
                ..MixerTrack::new("")
            };
            *track == fresh
                && !project.channels.iter().any(|c| c.mixer_track == t)
                && !project.mixer.iter().any(|other| {
                    other.sends.iter().any(|send| send.target == t)
                        || other.sidechain_to.contains(&t)
                })
        })
        .collect()
}

const EFFECTS_WIDTH: f32 = 270.0;

/// The strip colour choices, and how much of the track colour each mixes
/// into the strip.
const TINT_NAMES: [&str; 4] = ["Off", "Faint", "Medium", "Strong"];
const TINTS: [f32; 4] = [0.0, 0.08, 0.16, 0.28];

/// Where a track menu records its track while open.
fn menu_target_id() -> egui::Id {
    egui::Id::new("mixer-menu-target")
}

/// The track whose menu is open (as of last frame).
pub(crate) fn menu_target(ctx: &egui::Context) -> Option<usize> {
    ctx.data(|d| d.get_temp::<usize>(menu_target_id().with("last")))
}

/// A strip's fill: its base tone with the track colour mixed in.
fn strip_fill(selected: bool, color: egui::Color32, tint: u8) -> egui::Color32 {
    let base = if selected {
        theme::STRIP_SELECTED
    } else {
        theme::PANEL_DARK
    };
    let amount = TINTS.get(usize::from(tint)).copied().unwrap_or(0.0);
    theme::mix(base, color, amount)
}

/// Id of the strips' scroll bars.
const STRIP_BARS: &str = "mixer-strip-bars";

/// Width of the name beside the meter in a strip's upper block.
const NAME_WIDTH: f32 = 22.0;

/// A strip's name, written bottom to top so narrow strips fit long names.
fn vertical_name(ui: &mut egui::Ui, name: &str, selected: bool, height: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(NAME_WIDTH, height), egui::Sense::click());
    let mut job = egui::text::LayoutJob::simple_singleline(
        name.to_owned(),
        egui::FontId::proportional(theme::FONT_LABEL),
        if selected {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        },
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(height - 4.0);
    let galley = ui.painter().layout_job(job);
    let pos = egui::pos2(rect.center().x - galley.size().y / 2.0, rect.bottom() - 2.0);
    ui.painter().add(
        egui::epaint::TextShape::new(pos, galley, theme::TEXT)
            .with_angle(-std::f32::consts::FRAC_PI_2),
    );
    step_strips_on_wheel(ui, &response);
    response
}

/// The wheel over `response` (a strip's name, number, meter or empty space)
/// scrolls the strips by whole strips.
fn step_strips_on_wheel(ui: &egui::Ui, response: &egui::Response) {
    let notches = if response.hovered() && !ui.input(|i| i.modifiers.shift) {
        crate::scrollbars::wheel_notches(ui)
    } else {
        0.0
    };
    if notches != 0.0 {
        let step = vec2(-notches * (STRIP_WIDTH + 2.0), 0.0);
        crate::scrollbars::plain_step(ui.ctx(), egui::Id::new(STRIP_BARS), step);
    }
}

/// A round light of `diameter`: filled in `color` when on, a dim ring when
/// off.
fn light(
    ui: &mut egui::Ui,
    diameter: f32,
    on: bool,
    color: egui::Color32,
    hint: &str,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(diameter + 6.0, diameter + 2.0), egui::Sense::click());
    let center = rect.center();
    let radius = diameter / 2.0;
    if on {
        ui.painter().circle_filled(center, radius, color);
    } else {
        ui.painter()
            .circle_filled(center, radius, theme::PANEL_DARK);
        ui.painter().circle_stroke(
            center,
            radius - 0.5,
            egui::Stroke::new(
                1.0,
                if response.hovered() {
                    theme::TEXT
                } else {
                    theme::TEXT_DIM
                },
            ),
        );
    }
    response.hint(hint)
}

/// A tiny square switch showing `icon`: dim when off, the highlight tone on
/// a raised square when on.
fn small_switch(ui: &mut egui::Ui, icon: Icon, on: bool, hint: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(16.0, 14.0), egui::Sense::click());
    if on || response.hovered() {
        let fill = if response.hovered() {
            theme::RAISED_HOVER
        } else {
            theme::RAISED
        };
        ui.painter().rect_filled(rect, 2.0, fill);
    }
    let color = if on {
        theme::HIGHLIGHT
    } else {
        theme::TEXT_DIM
    };
    icons::paint(ui.painter(), rect, icon, color);
    response.hint(hint)
}

/// The number cell: the number (M for the master) on a cell tinted with the
/// track's colour.
fn number_cell(
    ui: &mut egui::Ui,
    index: usize,
    color: egui::Color32,
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(STRIP_WIDTH, 16.0), egui::Sense::click());
    let tint = theme::mix(theme::RAISED, color, if selected { 0.6 } else { 0.35 });
    ui.painter().rect_filled(rect, 2.0, tint);
    let label = if index == 0 {
        "M".to_owned()
    } else {
        index.to_string()
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(theme::FONT_LABEL + 1.0),
        theme::text_on(tint),
    );
    response
}

/// A mixer track's colour: the one picked for it, or the master's highlight
/// or a palette colour by position.
pub(crate) fn track_color(index: usize, track: &MixerTrack) -> egui::Color32 {
    match track.color {
        Some(c) => theme::palette(usize::from(c)),
        None if index == 0 => theme::HIGHLIGHT,
        None => theme::palette(index + 3),
    }
}

/// The colour grid for track `index`, in the track menus.
fn colour_menu(ui: &mut egui::Ui, index: usize, track: &MixerTrack, actions: &mut Vec<Action>) {
    ui.menu_button("Colour", |ui| {
        if let Some(choice) = crate::swatches::grid(ui, track.color) {
            run(
                actions,
                "mixer.set_track_color",
                [index.to_string(), crate::swatches::arg(choice)],
            );
            ui.close();
        }
    });
}

/// The track menu (right-click on the number or name).
/// Pin ▸: which dock the strip sits in.
fn pin_menu(ui: &mut egui::Ui, index: usize, track: &MixerTrack, actions: &mut Vec<Action>) {
    ui.menu_button("Pin", |ui| {
        for (dock, label) in [
            (Dock::Left, "Left side"),
            (Dock::Middle, "Scrolling area"),
            (Dock::Right, "Right side"),
        ] {
            if ui.selectable_label(track.dock == dock, label).clicked() {
                run(
                    actions,
                    "mixer.set_dock",
                    [index.to_string(), dock.arg().to_owned()],
                );
                ui.close();
            }
        }
    });
}

fn track_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    index: usize,
    actions: &mut Vec<Action>,
) {
    let Some(track) = project.mixer.get(index) else {
        return;
    };
    let number = index.to_string();
    if ui.button("Rename…").clicked() {
        state.open_dialog(Dialog::rename_track(index, &track.name));
        ui.close();
    }
    colour_menu(ui, index, track, actions);
    if index > 0 {
        pin_menu(ui, index, track, actions);
        for (label, right) in [("Shift left", false), ("Shift right", true)] {
            let enabled = can_move(project, index, right);
            if widgets::add_enabled(ui, enabled, egui::Button::new(label)).clicked() {
                move_strip(state, index, right, actions);
                ui.close();
            }
        }
    }
    ui.horizontal(|ui| {
        ui.label("Stereo separation");
        let mut separation = track.separation;
        let drag = ui.add(
            egui::DragValue::new(&mut separation)
                .range(-1.0..=1.0)
                .speed(0.01),
        );
        if drag.drag_started() || drag.gained_focus() {
            state.begin_gesture();
        }
        if drag.changed() {
            actions.push(Action::Coalesced(
                Invocation::new(
                    "mixer.set_separation",
                    [number.clone(), format!("{separation:.3}")],
                ),
                state.gesture(),
            ));
        }
    })
    .response
    .on_hover_text("-1 mono, 0 as is, 1 wider");
    for (label, on, name) in [
        ("Invert phase", track.phase_invert, "mixer.set_phase_invert"),
        (
            "Swap left and right",
            track.swap_sides,
            "mixer.set_swap_sides",
        ),
    ] {
        if ui.selectable_label(on, label).clicked() {
            run(
                actions,
                name,
                [number.clone(), if on { "off" } else { "on" }.to_owned()],
            );
            ui.close();
        }
    }
    if index > 0
        && ui
            .button(RichText::new("Delete").color(theme::DANGER))
            .clicked()
    {
        run(actions, "mixer.remove_track", [number.clone()]);
        ui.close();
    }
}

/// Runs a knob gesture as one coalesced undo step of `name <number> <value>`.
fn knob_command(
    state: &mut UiState,
    out: &widgets::KnobOutput,
    name: &str,
    number: &str,
    actions: &mut Vec<Action>,
) {
    if out.started {
        state.begin_gesture();
    }
    if let Some(value) = out.changed {
        actions.push(Action::Coalesced(
            Invocation::new(name, [number.to_owned(), format!("{value:.4}")]),
            state.gesture(),
        ));
    }
}

/// The selected mixer tracks, the primary one included, in order.
fn selection(state: &UiState) -> Vec<usize> {
    let mut all = state.mixer_selection.clone();
    all.insert(state.selected_track);
    all.into_iter().collect()
}

fn is_selected(state: &UiState, index: usize) -> bool {
    state.selected_track == index || state.mixer_selection.contains(&index)
}

/// Selects `index` alone, or with `toggle` adds it to the selection (as the
/// primary track) or takes it out.
fn select(state: &mut UiState, index: usize, toggle: bool) {
    if !toggle {
        state.mixer_selection.clear();
        state.selected_track = index;
    } else if state.selected_track == index {
        if let Some(next) = state.mixer_selection.pop_last() {
            state.selected_track = next;
        }
    } else if !state.mixer_selection.remove(&index) {
        state.mixer_selection.insert(state.selected_track);
        state.selected_track = index;
    }
}

/// Whether insert `index` can swap with its right (or left) neighbour: both
/// are inserts and no send joins them (sends go to higher numbers).
fn can_move(project: &Project, index: usize, right: bool) -> bool {
    let other = if right {
        index + 1
    } else {
        index.wrapping_sub(1)
    };
    let (low, high) = (index.min(other), index.max(other));
    index > 0
        && low > 0
        && high < project.mixer.len()
        && !project.mixer[low]
            .sends
            .iter()
            .any(|send| send.target == high)
}

/// Swaps insert `index` with its neighbour (`mixer.move_track`); the
/// selection follows the moved tracks.
fn move_strip(state: &mut UiState, index: usize, right: bool, actions: &mut Vec<Action>) {
    let other = if right { index + 1 } else { index - 1 };
    let swap = |t: usize| match t {
        t if t == index => other,
        t if t == other => index,
        t => t,
    };
    let way = if right { "right" } else { "left" };
    run(
        actions,
        "mixer.move_track",
        [index.to_string(), way.to_owned()],
    );
    state.selected_track = swap(state.selected_track);
    state.mixer_selection = state.mixer_selection.iter().map(|&t| swap(t)).collect();
}

/// Makes `index`, already selected, the primary track.
fn make_primary(state: &mut UiState, index: usize) {
    if state.mixer_selection.remove(&index) {
        state.mixer_selection.insert(state.selected_track);
        state.selected_track = index;
    }
}

/// Moves the fader (`mixer.set_volume`), pan (`mixer.set_pan`) or separation
/// (`mixer.set_separation`) of every track in `tracks` by as much as track
/// `index` moves to `value`: faders by the same share of their travel, the
/// knobs by the same amount, each clamped.
fn ganged(project: &Project, tracks: &[usize], index: usize, name: &str, value: f32) -> Invocation {
    let volume = name == "mixer.set_volume";
    let position = |t: &MixerTrack| match name {
        "mixer.set_volume" => widgets::gain_to_fader(t.volume, MAX_VOLUME),
        "mixer.set_separation" => t.separation,
        _ => t.pan,
    };
    let to = if volume {
        widgets::gain_to_fader(value, MAX_VOLUME)
    } else {
        value
    };
    let delta = project.mixer.get(index).map_or(0.0, |t| to - position(t));
    let lines = tracks.iter().filter_map(|&t| {
        let track = project.mixer.get(t)?;
        let moved = if t == index {
            value
        } else if volume {
            widgets::fader_to_gain((position(track) + delta).clamp(0.0, 1.0), MAX_VOLUME)
        } else {
            (position(track) + delta).clamp(-1.0, 1.0)
        };
        Some(Invocation::new(name, [t.to_string(), format!("{moved:.4}")]).to_string())
    });
    Invocation::new(musiccraft_commands::BATCH, lines)
}

/// Like `knob_command`, but a control of a track inside a multi-selection
/// moves the whole selection.
fn strip_control(
    state: &mut UiState,
    project: &Project,
    index: usize,
    out: &widgets::KnobOutput,
    name: &str,
    actions: &mut Vec<Action>,
) {
    let tracks = selection(state);
    if tracks.len() < 2 || !tracks.contains(&index) {
        knob_command(state, out, name, &index.to_string(), actions);
        return;
    }
    if out.started {
        state.begin_gesture();
    }
    if let Some(value) = out.changed {
        let invocation = ganged(project, &tracks, index, name, value);
        actions.push(Action::Coalesced(invocation, state.gesture()));
    }
}

/// The tracks a switch on track `index` acts on: the whole selection when
/// `index` is part of it, otherwise `index` alone.
fn targets(state: &UiState, index: usize) -> Vec<usize> {
    if is_selected(state, index) {
        selection(state)
    } else {
        vec![index]
    }
}

/// Runs `name <track> <value>` (or `name <track>` when `value` is empty) for
/// each of `tracks`, as one undo step.
fn run_each(actions: &mut Vec<Action>, name: &str, tracks: &[usize], value: &str) {
    let mut lines: Vec<String> = tracks
        .iter()
        .map(|t| {
            let args = [t.to_string(), value.to_owned()];
            Invocation::new(name, args.into_iter().filter(|a| !a.is_empty())).to_string()
        })
        .collect();
    match lines.len() {
        0 => {}
        1 => actions.extend(Invocation::parse(&lines.remove(0)).ok().map(Action::Run)),
        _ => run(actions, musiccraft_commands::BATCH, lines),
    }
}

/// Window-local keys: Ctrl+A selects every track, Esc keeps only the primary.
fn keys(ctx: &egui::Context, state: &mut UiState, project: &Project, actions: &mut Vec<Action>) {
    use egui::{Key, Modifiers};
    for (key, right) in [(Key::ArrowLeft, false), (Key::ArrowRight, true)] {
        let index = state.selected_track;
        if can_move(project, index, right) && ctx.input_mut(|i| i.consume_key(Modifiers::ALT, key))
        {
            move_strip(state, index, right, actions);
        }
    }
    if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::A)) {
        state.mixer_selection = (0..project.mixer.len())
            .filter(|&t| t != state.selected_track)
            .collect();
    }
    if !state.mixer_selection.is_empty()
        && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
    {
        state.mixer_selection.clear();
    }
}

#[allow(clippy::too_many_arguments)]
fn strip(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    index: usize,
    track: &MixerTrack,
    height: f32,
    actions: &mut Vec<Action>,
) {
    let number = index.to_string();
    let selected = is_selected(state, index);
    let primary = state.selected_track == index;
    let color = track_color(index, track);
    let tint = if index == 0 { 0 } else { state.mixer_tint };
    let fill = if menu_target(ui.ctx()) == Some(index) {
        theme::RAISED_HOVER
    } else {
        strip_fill(selected, color, tint)
    };
    let frame = egui::Frame::new()
        .fill(fill)
        .corner_radius(2.0)
        .inner_margin(egui::Margin::symmetric(0, 2));
    let mut on_arrow = false;
    let response = frame.show(ui, |ui| {
        ui.set_width(STRIP_WIDTH);
        ui.set_height(height - 4.0);
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 3.0;
            let hint = if index == 0 {
                format!("{} — click to select, right-click for options", track.name)
            } else {
                format!(
                    "{} — click to select, right-click for options, Shift+wheel to move",
                    track.name
                )
            };
            let cell = number_cell(ui, index, color, selected).hint(&hint);
            step_strips_on_wheel(ui, &cell);
            let block = (height * 0.22).clamp(56.0, 120.0);
            let name = ui
                .horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.add_space(3.0);
                    let name = vertical_name(ui, &track.name, selected, block).hint(&hint);
                    let levels = state.meters.get(index).copied().unwrap_or_default();
                    let meter = widgets::meter(ui, levels, vec2(10.0, block));
                    if selected {
                        ui.painter().rect_stroke(
                            meter.rect.expand(1.0),
                            2.0,
                            egui::Stroke::new(1.0, theme::ACCENT),
                            egui::StrokeKind::Outside,
                        );
                    }
                    let meter = meter.interact(egui::Sense::click()).hint(&hint);
                    step_strips_on_wheel(ui, &meter);
                    name.union(meter)
                })
                .inner
                .union(cell);
            if name.clicked() {
                let toggle = ui.input(|i| i.modifiers.command && i.modifiers.shift);
                select(state, index, toggle);
            }
            if name.double_clicked() {
                // The name reads upward; the field is a plain row at its top.
                let field = egui::Rect::from_min_size(name.rect.left_top(), vec2(120.0, 20.0));
                state.rename_in_place(Dialog::rename_track(index, &track.name), field);
            }
            name.context_menu(|ui| {
                ui.data_mut(|d| d.insert_temp(menu_target_id(), index));
                track_menu(ui, state, project, index, actions);
            });
            if name.hovered() && ui.input(|i| i.modifiers.shift) {
                // Shift+wheel moves the strip: down (towards you) to the right.
                let notches = crate::scrollbars::wheel_notches(ui);
                let right = notches < 0.0;
                if notches != 0.0 && can_move(project, index, right) {
                    move_strip(state, index, right, actions);
                }
            }
            let silenced = index > 0
                && project
                    .mixer_audible()
                    .is_some_and(|audible| !audible.get(index).copied().unwrap_or(true));
            let heard = if track.mute {
                "muted"
            } else if track.solo {
                "soloed"
            } else if silenced {
                "silenced by solo"
            } else {
                "on"
            };
            let enable = light(
                ui,
                10.0,
                !track.mute,
                if silenced {
                    theme::ACCENT_DIM
                } else {
                    theme::ACCENT
                },
                &if index > 0 {
                    format!("Enable ({heard}) · click to mute or unmute · Ctrl+click to solo")
                } else {
                    format!("Enable ({heard}) · click to mute or unmute")
                },
            );
            if track.solo {
                ui.painter().circle_stroke(
                    enable.rect.center(),
                    6.5,
                    egui::Stroke::new(1.0, theme::HIGHLIGHT),
                );
            }
            let solo_key = ui.input(|i| i.modifiers.command);
            let tracks = selection(state);
            if enable.clicked() {
                if solo_key && index > 0 {
                    run(actions, "mixer.toggle_solo", [number.clone()]);
                } else if selected && tracks.len() > 1 {
                    let to = if track.mute { "off" } else { "on" };
                    run_each(actions, "mixer.set_mute", &tracks, to);
                } else {
                    run(actions, "mixer.toggle_mute", [number.clone()]);
                }
            }
            enable.context_menu(|ui| {
                let mute = if track.mute { "Unmute" } else { "Mute" };
                if ui.button(mute).clicked() {
                    run(actions, "mixer.toggle_mute", [number.clone()]);
                    ui.close();
                }
                if index > 0 {
                    let solo = if track.solo { "Unsolo" } else { "Solo" };
                    if ui.button(solo).clicked() {
                        run(actions, "mixer.toggle_solo", [number.clone()]);
                        ui.close();
                    }
                }
            });
            if index > 0 {
                if light(ui, 7.0, track.solo, theme::HIGHLIGHT, "Solo").clicked() {
                    run(actions, "mixer.toggle_solo", [number.clone()]);
                }
            } else {
                ui.add_space(9.0);
            }
            let pan = widgets::knob(ui, track.pan, -1.0, 1.0, 0.0, true);
            crate::automation::menu(&pan.response, &format!("mixer:{index}:pan"), actions);
            strip_control(state, project, index, &pan, "mixer.set_pan", actions);
            pan.response.hint(format!("Pan {}", format::pan(track.pan)));
            let below = if state.mixer_details { 84.0 } else { 0.0 } + 20.0;
            let fader_height = (ui.available_height() - below).max(60.0);
            let fader = widgets::fader(
                ui,
                track.volume,
                MAX_VOLUME,
                vec2(STRIP_WIDTH - 12.0, fader_height),
            );
            crate::automation::menu(&fader.response, &format!("mixer:{index}:volume"), actions);
            strip_control(state, project, index, &fader, "mixer.set_volume", actions);
            fader
                .response
                .hint(format!("Level {}", format::level(track.volume)));
            if state.mixer_details {
                details(ui, state, project, index, actions);
            }
            on_arrow = route_arrow(ui, state, project, index, actions);
        });
    });
    // A click anywhere on the strip selects it. This reads the pointer
    // instead of adding a strip-wide click area, which would sit on top of
    // the lights, switches and name and take their clicks and hovers.
    // Clicks on the routing arrow route the selected insert, and clicks in a
    // menu above the strip belong to the menu: neither selects.
    let click = ui.input(|i| {
        i.pointer
            .interact_pos()
            .filter(|_| i.pointer.primary_clicked())
    });
    let clicked_here = !on_arrow
        && click.is_some_and(|p| {
            response.response.rect.contains(p) && ui.ctx().layer_id_at(p) == Some(ui.layer_id())
        });
    let toggling = ui.input(|i| i.modifiers.command && i.modifiers.shift);
    if clicked_here && ui.is_rect_visible(response.response.rect) && !toggling {
        if is_selected(state, index) {
            make_primary(state, index);
        } else {
            select(state, index, false);
        }
    }
    if state.mixer_hairlines {
        let r = response.response.rect;
        let x = r.right() + 1.0;
        ui.painter().line_segment(
            [egui::pos2(x, r.top()), egui::pos2(x, r.bottom())],
            egui::Stroke::new(1.0, theme::WINDOW_EDGE),
        );
    }
    if primary {
        ui.painter().rect_stroke(
            response.response.rect,
            2.0,
            egui::Stroke::new(1.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    effect_drop(ui, index, response.response.rect, actions);
}

/// An effect dragged from the browser over a strip outlines it, and dropping
/// it there adds it to that track's first free slot.
fn effect_drop(ui: &egui::Ui, index: usize, rect: egui::Rect, actions: &mut Vec<Action>) {
    let Some(dragged) = egui::DragAndDrop::payload::<crate::browser::Dragged>(ui.ctx()) else {
        return;
    };
    let Some(add) = dragged.add_effect(index) else {
        return;
    };
    let over = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| rect.contains(p) && ui.clip_rect().contains(p));
    if !over {
        return;
    }
    ui.ctx()
        .layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("mixer-drop"),
        ))
        .rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(2.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    if ui.input(|i| i.pointer.any_released())
        && egui::DragAndDrop::take_payload::<crate::browser::Dragged>(ui.ctx()).is_some()
    {
        actions.push(Action::Run(add));
    }
}

/// The track switches: invert phase, swap, separation and the effect count.
fn details(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    index: usize,
    actions: &mut Vec<Action>,
) {
    let Some(track) = project.mixer.get(index) else {
        return;
    };
    let tracks = targets(state, index);
    ui.spacing_mut().item_spacing.y = 1.0;
    for (icon, on, name, label) in [
        (
            Icon::Phase,
            track.phase_invert,
            "mixer.set_phase_invert",
            "Invert phase",
        ),
        (
            Icon::Swap,
            track.swap_sides,
            "mixer.set_swap_sides",
            "Swap left and right",
        ),
    ] {
        let state_word = if on { "on" } else { "off" };
        if small_switch(ui, icon, on, &format!("{label}: {state_word}")).clicked() {
            run_each(actions, name, &tracks, if on { "off" } else { "on" });
        }
    }
    let sep = widgets::knob(ui, track.separation, -1.0, 1.0, 0.0, true);
    strip_control(state, project, index, &sep, "mixer.set_separation", actions);
    sep.response.hint(format!(
        "Stereo separation {:+.0} (left: mono, right: wider)",
        track.separation * 100.0
    ));
    let with_effects: Vec<usize> = tracks
        .into_iter()
        .filter(|&t| project.mixer.get(t).is_some_and(|t| !t.effects.is_empty()))
        .collect();
    effects_switch(ui, track, (index, &with_effects), actions);
}

/// The effect count at the foot of a strip, lit while any effect runs.
/// Clicking it bypasses every effect of the track, or brings them all back.
/// `tracks` are the ones with effects that the click changes.
fn effects_switch(
    ui: &mut egui::Ui,
    track: &MixerTrack,
    (index, tracks): (usize, &[usize]),
    actions: &mut Vec<Action>,
) {
    let number = index.to_string();
    let count = track.effects.len();
    let running = track.effects.iter().filter(|e| e.enabled).count();
    let names: Vec<String> = track.effects.iter().map(|e| e.label()).collect();
    let text = if count > 0 {
        format!("{count} fx")
    } else {
        " ".to_owned()
    };
    let color = if running > 0 {
        theme::ACCENT
    } else {
        theme::TEXT_FAINT
    };
    let response = ui
        .add(
            egui::Label::new(
                RichText::new(text)
                    .size(theme::FONT_CAPTION + 0.5)
                    .color(color),
            )
            .sense(egui::Sense::click()),
        )
        .hint(if names.is_empty() {
            "No effects".to_owned()
        } else if running > 0 {
            format!(
                "Effects: {} · click to bypass them all · right-click to open their windows",
                names.join(", ")
            )
        } else {
            format!(
                "Effects (all bypassed): {} · click to enable them · right-click to open their windows",
                names.join(", ")
            )
        });
    if count > 0 {
        response.context_menu(|ui| {
            if ui.button("Open all effect windows").clicked() {
                run(actions, "ui.open_effects", [number.clone()]);
            }
        });
    }
    if count > 0 && response.clicked() {
        let on = if running > 0 { "off" } else { "on" };
        run_each(actions, "mixer.set_effects_enabled", tracks, on);
    }
}

/// The arrow under a strip shows whether the selected insert sends here;
/// clicking it toggles that route. Right-clicking offers "Sidechain to this
/// track", which a dashed outline shows. Returns whether the pointer is on it.
fn route_arrow(
    ui: &mut egui::Ui,
    state: &UiState,
    project: &Project,
    target: usize,
    actions: &mut Vec<Action>,
) -> bool {
    let source = state.selected_track;
    let (rect, response) = ui.allocate_exact_size(vec2(STRIP_WIDTH, 14.0), egui::Sense::click());
    let on_arrow = response.contains_pointer();
    let Some(from) = project.mixer.get(source) else {
        return on_arrow;
    };
    let c = rect.center();
    let down = vec![
        c + vec2(-6.0, -4.0),
        c + vec2(6.0, -4.0),
        c + vec2(0.0, 4.0),
    ];
    if target == source && source > 0 {
        // The selected insert shows a source arrow; its hint says where it goes.
        ui.painter().add(egui::Shape::convex_polygon(
            vec![c + vec2(-6.0, 4.0), c + vec2(6.0, 4.0), c + vec2(0.0, -4.0)],
            theme::HIGHLIGHT,
            egui::Stroke::NONE,
        ));
        response.hint(format!("This insert goes to {}", route_summary(from)));
        return on_arrow;
    }
    if source == 0 {
        return on_arrow;
    }
    let sidechained = from.sidechain_to.contains(&target);
    sidechain_menu(&response, source, target, sidechained, actions);
    let possible = target == 0 || target > source;
    if !possible {
        // Inert for audio: a faint outline (dashed when sidechained).
        let edge = if sidechained {
            theme::HIGHLIGHT
        } else {
            theme::RAISED_HOVER
        };
        arrow_outline(ui, &down, edge, sidechained);
        response.hint(if sidechained {
            "Sidechained here — right-click to remove"
        } else {
            "Right-click: Sidechain to this track"
        });
        return on_arrow;
    }
    let routed = if target == 0 {
        from.to_master
    } else {
        from.sends.iter().any(|s| s.target == target)
    };
    // Routed: filled in the highlight tone. Sidechained: a dashed outline in
    // the highlight tone. Possible: an outline that brightens on hover.
    if routed {
        ui.painter().add(egui::Shape::convex_polygon(
            down,
            theme::HIGHLIGHT,
            egui::Stroke::new(1.0, theme::HIGHLIGHT),
        ));
    } else {
        let edge = if sidechained {
            theme::HIGHLIGHT
        } else if response.hovered() {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        };
        arrow_outline(ui, &down, edge, sidechained);
    }
    let hint = match (routed, sidechained) {
        (true, true) => "Routed and sidechained here — click to remove the route",
        (true, false) => "Routed here — click to remove",
        (false, true) => "Sidechained here — click to route audio too, right-click to remove",
        (false, false) => "Click to route the selected insert here · right-click to sidechain",
    };
    if response.hint(hint).clicked() {
        let invocation = if target == 0 {
            Invocation::new(
                "mixer.route_to_master",
                [
                    source.to_string(),
                    if routed { "off" } else { "on" }.to_owned(),
                ],
            )
        } else {
            Invocation::new(
                "mixer.set_send",
                [
                    source.to_string(),
                    target.to_string(),
                    if routed { "0" } else { "1" }.to_owned(),
                ],
            )
        };
        actions.push(Action::Run(invocation));
    }
    on_arrow
}

/// A routing arrow's outline, dashed for a sidechain route.
fn arrow_outline(ui: &egui::Ui, points: &[egui::Pos2], color: Color32, dashed: bool) {
    let stroke = egui::Stroke::new(1.0, color);
    if dashed {
        let mut closed = points.to_vec();
        closed.extend(points.first().copied());
        ui.painter()
            .extend(egui::Shape::dashed_line(&closed, stroke, 2.0, 2.0));
    } else {
        ui.painter().add(egui::Shape::convex_polygon(
            points.to_vec(),
            Color32::TRANSPARENT,
            stroke,
        ));
    }
}

/// The routing arrow's right-click menu: route the selected insert here as a
/// sidechain only (FL Studio's "Sidechain to this track"), or remove it.
fn sidechain_menu(
    response: &egui::Response,
    source: usize,
    target: usize,
    sidechained: bool,
    actions: &mut Vec<Action>,
) {
    response.context_menu(|ui| {
        if ui
            .add(egui::Button::new("Sidechain to this track").selected(sidechained))
            .hint("Its output reaches this track's sidechain inputs, not its audio")
            .clicked()
        {
            run(
                actions,
                "mixer.sidechain_to",
                [
                    source.to_string(),
                    target.to_string(),
                    if sidechained { "off" } else { "on" }.to_owned(),
                ],
            );
            ui.close();
        }
    });
}

/// Where an insert's output goes: `M`, `3`, `M, 3, 5` or `none` (drawn
/// after an [`Icon::Out`] glyph).
fn route_summary(track: &MixerTrack) -> String {
    let mut to: Vec<String> = Vec::new();
    if track.to_master {
        to.push("M".to_owned());
    }
    to.extend(track.sends.iter().map(|s| s.target.to_string()));
    to.extend(track.sidechain_to.iter().map(|t| match t {
        0 => "SC M".to_owned(),
        t => format!("SC {t}"),
    }));
    if to.is_empty() {
        "none".to_owned()
    } else {
        to.join(", ")
    }
}

/// A device picker; `None` is the system default. Returns the choice made.
fn device_picker(
    ui: &mut egui::Ui,
    id: &str,
    current: Option<&String>,
    devices: &[String],
) -> Option<Option<String>> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(id)
        .width(EFFECTS_WIDTH - 50.0)
        .selected_text(current.map_or("System default", String::as_str))
        .show_ui(ui, |ui| {
            let choices = std::iter::once(None).chain(devices.iter().cloned().map(Some));
            for choice in choices {
                let label = choice
                    .clone()
                    .unwrap_or_else(|| "System default".to_owned());
                if ui
                    .selectable_label(current == choice.as_ref(), label)
                    .clicked()
                {
                    picked = Some(choice);
                }
            }
        });
    picked
}

/// The selected track's input: the audio input device recorded from.
fn input_row(ui: &mut egui::Ui, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        icons::glyph(ui, Icon::In, 12.0, theme::TEXT_DIM).hint(
            "Audio input recorded from: the one input device for every track (Settings → Audio)",
        );
        let settings = frame.settings;
        let inputs = &frame.settings_info.inputs;
        if let Some(choice) =
            device_picker(ui, "mixer-input", settings.audio_input.as_ref(), inputs)
        {
            actions.push(Action::Settings(Box::new(crate::Settings {
                audio_input: choice,
                ..settings.clone()
            })));
        }
    });
}

/// The selected track's output: where an insert routes, or the master's
/// audio device.
fn output_row(
    ui: &mut egui::Ui,
    frame: &Frame<'_>,
    index: usize,
    track: &MixerTrack,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        if index > 0 {
            icons::glyph(ui, Icon::Out, 12.0, theme::TEXT_DIM);
            ui.label(RichText::new(route_summary(track)).monospace())
                .hint("Set with the arrows under the strips, or To master / Send below");
            return;
        }
        icons::glyph(ui, Icon::Out, 12.0, theme::TEXT_DIM)
            .hint("Audio output device the master plays through");
        let settings = frame.settings;
        let outputs = &frame.settings_info.outputs;
        if let Some(choice) =
            device_picker(ui, "mixer-output", settings.audio_device.as_ref(), outputs)
        {
            actions.push(Action::Settings(Box::new(crate::Settings {
                audio_device: choice,
                ..settings.clone()
            })));
        }
    });
}

/// The track's built-in EQ: a gain knob per band; right-click for its
/// frequency.
fn eq_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    index: usize,
    track: &MixerTrack,
    actions: &mut Vec<Action>,
) {
    use musiccraft_project::TrackEq;
    let number = index.to_string();
    ui.horizontal(|ui| {
        ui.label(RichText::new("EQ").small().color(theme::TEXT_DIM));
        for (band, name) in TrackEq::BANDS.iter().enumerate() {
            let (gain, freq) = track.eq.bands[band];
            ui.vertical(|ui| {
                let max = TrackEq::MAX_GAIN;
                let knob = widgets::knob(ui, gain, -max, max, 0.0, true);
                if knob.started {
                    state.begin_gesture();
                }
                if let Some(value) = knob.changed {
                    actions.push(Action::Coalesced(
                        Invocation::new(
                            "mixer.set_eq",
                            [number.clone(), (*name).to_owned(), format!("{value:.2}")],
                        ),
                        state.gesture(),
                    ));
                }
                let response = knob.response.hint(format!(
                    "{name}: {gain:+.1} dB at {freq:.0} Hz (right-click: frequency)"
                ));
                response.context_menu(|ui| {
                    let (lo, hi) = TrackEq::RANGES[band];
                    let mut f = freq;
                    ui.horizontal(|ui| {
                        ui.label("Frequency");
                        let drag = ui.add(
                            egui::DragValue::new(&mut f)
                                .range(lo..=hi)
                                .speed(f64::from(freq) * 0.01)
                                .suffix(" Hz"),
                        );
                        if drag.drag_started() || drag.gained_focus() {
                            state.begin_gesture();
                        }
                        if drag.changed() {
                            actions.push(Action::Coalesced(
                                Invocation::new(
                                    "mixer.set_eq",
                                    [
                                        number.clone(),
                                        (*name).to_owned(),
                                        format!("{gain:.2}"),
                                        format!("{f:.1}"),
                                    ],
                                ),
                                state.gesture(),
                            ));
                        }
                    });
                });
                ui.label(RichText::new(*name).small().color(theme::TEXT_DIM));
            });
        }
    });
}

fn effects(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let project = frame.project;
    let plugins = frame.plugins;
    let index = state.selected_track;
    let Some(track) = project.mixer.get(index) else {
        return;
    };
    let number = index.to_string();
    ui.vertical(|ui| {
        ui.set_width(EFFECTS_WIDTH);
        input_row(ui, frame, actions);
        let area = egui::ScrollArea::vertical().id_salt("effect-chain");
        let bars = (egui::Id::new("mixer-slot-bars"), 25.0);
        crate::scrollbars::plain_area(ui, area, bars, |ui| {
            let mut rows = Vec::with_capacity(MAX_EFFECT_SLOTS);
            for slot in 0..MAX_EFFECT_SLOTS {
                rows.push(slot_row(ui, state, track, &number, slot, plugins, actions));
            }
            if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
                // An effect dropped on the slots goes to the selected track.
                effect_drop(ui, index, first.union(*last), actions);
            }
            ui.ctx().data_mut(|d| d.insert_temp(slot_rows_id(), rows));
            ui.separator();
            eq_row(ui, state, index, track, actions);
            output_row(ui, frame, index, track, actions);
            if index > 0 {
                routing(ui, state, project, index, track, actions);
            }
        });
    });
}

fn track_index(number: &str) -> usize {
    number.parse().unwrap_or(0)
}

/// One effect slot: chevron (slot menu), name, mix knob and a light that
/// bypasses or restores the effect. An empty slot shows a dim "Slot n" and
/// opens the add list.
/// Where last frame's slot rows were, for dropping a dragged effect.
fn slot_rows_id() -> egui::Id {
    egui::Id::new("mixer-slot-rows")
}

/// The loaded slot under `pos` in last frame's rows.
fn slot_at(ui: &egui::Ui, pos: egui::Pos2, loaded: usize) -> Option<usize> {
    let rows: Vec<egui::Rect> = ui.ctx().data(|d| d.get_temp(slot_rows_id()))?;
    rows.iter()
        .position(|r| r.y_range().contains(pos.y))
        .map(|slot| slot.min(loaded.saturating_sub(1)))
}

/// One slot row; returns its rectangle.
fn slot_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    track: &MixerTrack,
    number: &str,
    slot: usize,
    plugins: &crate::Plugins,
    actions: &mut Vec<Action>,
) -> egui::Rect {
    let slot_no = (slot + 1).to_string();
    let selected = state.mixer_slot == slot;
    let effect = track.effects.get(slot);
    let loaded = track.effects.len();
    egui::Frame::new()
        .fill(if selected && effect.is_some() {
            theme::RAISED
        } else {
            theme::PANEL_DARK
        })
        .corner_radius(2.0)
        .inner_margin(egui::Margin::symmetric(2, 1))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let popup = egui::Id::new(("mixer-slot-menu", slot));
                let open = egui::Popup::is_id_open(ui.ctx(), popup);
                let (rect, chevron) =
                    ui.allocate_exact_size(vec2(14.0, 20.0), egui::Sense::click());
                let glyph = if open || chevron.hovered() {
                    theme::TEXT
                } else {
                    theme::TEXT_DIM
                };
                icons::paint(ui.painter(), rect, Icon::ChevronRight, glyph);
                let Some(effect) = effect else {
                    let name = ui
                        .add(
                            egui::Label::new(
                                RichText::new(format!("Slot {slot_no}"))
                                    .italics()
                                    .color(theme::TEXT_FAINT),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .hint("Add an effect");
                    let anchor = chevron.union(name);
                    let toggle = anchor.clicked().then_some(egui::SetOpenCommand::Toggle);
                    egui::Popup::menu(&anchor)
                        .id(popup)
                        .open_memory(toggle)
                        .show(|ui| {
                            add_list(ui, state, number, track.effects.len(), plugins, actions)
                        });
                    return;
                };
                let chevron = chevron.hint("Slot menu");
                let toggle = chevron.clicked().then_some(egui::SetOpenCommand::Toggle);
                egui::Popup::menu(&chevron)
                    .id(popup)
                    .open_memory(toggle)
                    .show(|ui| slot_menu(ui, state, track, number, slot, true, actions));
                let name = RichText::new(effect.label()).color(if effect.enabled {
                    theme::TEXT
                } else {
                    theme::TEXT_DIM
                });
                let name = ui
                    .add(
                        egui::Label::new(name)
                            .truncate()
                            .sense(egui::Sense::click_and_drag()),
                    )
                    .hint("Open or close this effect's window · drag to reorder");
                if name.clicked() {
                    state.mixer_slot = slot;
                    crate::effect_window::toggle(state, track_index(number), slot);
                }
                drag_to_reorder(ui, &name, number, slot, loaded, actions);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let word = if effect.enabled { "on" } else { "bypassed" };
                    if light(
                        ui,
                        8.0,
                        effect.enabled,
                        theme::ACCENT,
                        &format!("Effect {word} · click to bypass or restore"),
                    )
                    .clicked()
                    {
                        run(
                            actions,
                            "mixer.toggle_effect",
                            [number.to_owned(), slot_no.clone()],
                        );
                    }
                    let mix = widgets::knob(ui, effect.mix, 0.0, 1.0, 1.0, false);
                    if mix.started {
                        state.begin_gesture();
                    }
                    if let Some(value) = mix.changed {
                        actions.push(Action::Coalesced(
                            Invocation::new(
                                "mixer.set_effect_mix",
                                [number.to_owned(), slot_no.clone(), format!("{value:.3}")],
                            ),
                            state.gesture(),
                        ));
                    }
                    mix.response.hint(format!("Mix {:.0}%", effect.mix * 100.0));
                });
            });
        })
        .response
        .rect
}

/// Dragging a loaded slot's name moves its effect to the slot it is dropped
/// on (`mixer.move_effect`); a line marks where it will land.
fn drag_to_reorder(
    ui: &egui::Ui,
    name: &egui::Response,
    number: &str,
    slot: usize,
    loaded: usize,
    actions: &mut Vec<Action>,
) {
    let Some(pos) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let target = slot_at(ui, pos, loaded).filter(|t| *t != slot);
    if name.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let rows: Option<Vec<egui::Rect>> = ui.ctx().data(|d| d.get_temp(slot_rows_id()));
        if let Some(rect) = target.and_then(|t| rows?.get(t).copied()) {
            let y = if target > Some(slot) {
                rect.bottom()
            } else {
                rect.top()
            };
            ui.ctx()
                .layer_painter(egui::LayerId::new(egui::Order::Tooltip, name.id))
                .hline(rect.x_range(), y, egui::Stroke::new(2.0, theme::ACCENT));
        }
    }
    if let Some(target) = target.filter(|_| name.drag_stopped()) {
        run(
            actions,
            "mixer.move_effect",
            [
                number.to_owned(),
                (slot + 1).to_string(),
                (target + 1).to_string(),
            ],
        );
    }
}

/// The menu of a loaded slot, also the effect window's menu (`with_open`:
/// whether to offer "Open window").
pub(crate) fn slot_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    track: &MixerTrack,
    number: &str,
    slot: usize,
    with_open: bool,
    actions: &mut Vec<Action>,
) {
    let slot_no = (slot + 1).to_string();
    let enabled = track.effects.get(slot).is_some_and(|e| e.enabled);
    if with_open && ui.button("Open window").clicked() {
        state.mixer_slot = slot;
        crate::effect_window::toggle(state, track_index(number), slot);
        ui.close();
    }
    if ui
        .button(if enabled { "Bypass" } else { "Enable" })
        .clicked()
    {
        run(
            actions,
            "mixer.toggle_effect",
            [number.to_owned(), slot_no.clone()],
        );
        ui.close();
    }
    if crate::widgets::add_enabled(ui, slot > 0, egui::Button::new("Move up")).clicked() {
        run(
            actions,
            "mixer.move_effect",
            [number.to_owned(), slot_no.clone(), slot.to_string()],
        );
        ui.close();
    }
    if crate::widgets::add_enabled(
        ui,
        slot + 1 < track.effects.len(),
        egui::Button::new("Move down"),
    )
    .clicked()
    {
        run(
            actions,
            "mixer.move_effect",
            [number.to_owned(), slot_no.clone(), (slot + 2).to_string()],
        );
        ui.close();
    }
    ui.separator();
    if ui
        .button(RichText::new("Remove").color(theme::DANGER))
        .clicked()
    {
        run(actions, "mixer.remove_effect", [number.to_owned(), slot_no]);
        ui.close();
    }
}

/// The built-in effects, then the plugin effects; the choice goes into
/// `slot`, the first free one.
fn add_list(
    ui: &mut egui::Ui,
    state: &mut UiState,
    number: &str,
    slot: usize,
    plugins: &crate::Plugins,
    actions: &mut Vec<Action>,
) {
    if slot >= MAX_EFFECT_SLOTS {
        ui.label(RichText::new("Every slot is in use").color(theme::TEXT_DIM));
        return;
    }
    for kind in EFFECT_KINDS {
        if ui.button(kind.name()).clicked() {
            run(
                actions,
                "mixer.add_effect",
                [number.to_owned(), kind.name().to_owned()],
            );
            state.mixer_slot = slot;
            state.effect_windows.push((track_index(number), slot));
            ui.close();
        }
    }
    ui.separator();
    ui.menu_button("Plugin effects", |ui| {
        let chosen = crate::plugins::add_menu(ui, plugins, false, actions, |p| {
            Invocation::new(
                "mixer.add_plugin",
                [number.to_owned(), p.id.clone(), p.name.clone()],
            )
        });
        if chosen {
            state.mixer_slot = slot;
        }
    });
}

fn routing(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    index: usize,
    track: &MixerTrack,
    actions: &mut Vec<Action>,
) {
    let number = index.to_string();
    ui.separator();
    ui.horizontal(|ui| {
        let mut to_master = track.to_master;
        if crate::widgets::led_switch(ui, &mut to_master, "To master").changed() {
            run(
                actions,
                "mixer.route_to_master",
                [
                    number.clone(),
                    if to_master { "on" } else { "off" }.to_owned(),
                ],
            );
        }
        ui.menu_button("+ Send", |ui| {
            for (target, other) in project.mixer.iter().enumerate().skip(index + 1) {
                if track.sends.iter().any(|s| s.target == target) {
                    continue;
                }
                if ui.button(format!("{target} {}", other.name)).clicked() {
                    run(
                        actions,
                        "mixer.set_send",
                        [number.clone(), target.to_string(), "1".to_owned()],
                    );
                    ui.close();
                }
            }
        });
    });
    for send in &track.sends {
        ui.horizontal(|ui| {
            let name = project
                .mixer
                .get(send.target)
                .map_or("?", |t| t.name.as_str());
            icons::glyph(ui, Icon::Out, 12.0, theme::TEXT_DIM);
            ui.add_sized(
                vec2(58.0, 16.0),
                egui::Label::new(
                    RichText::new(format!("{} {name}", send.target))
                        .monospace()
                        .small(),
                ),
            );
            let mut level = send.level;
            let response =
                ui.add(egui::Slider::new(&mut level, 0.0..=MAX_VOLUME).show_value(false));
            if response.drag_started() {
                state.begin_gesture();
            }
            let reset = crate::widgets::wants_reset(&response);
            if reset {
                run(
                    actions,
                    "mixer.set_send",
                    [number.clone(), send.target.to_string(), "1".to_owned()],
                );
            } else if response.changed() && level > 0.0 {
                actions.push(Action::Coalesced(
                    Invocation::new(
                        "mixer.set_send",
                        [
                            number.clone(),
                            send.target.to_string(),
                            format!("{level:.3}"),
                        ],
                    ),
                    state.gesture(),
                ));
            }
            if icons::header_button(ui, Icon::Close, false, true, "Remove send").clicked() {
                run(
                    actions,
                    "mixer.set_send",
                    [number.clone(), send.target.to_string(), "0".to_owned()],
                );
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_summaries_name_every_destination() {
        let mut track = MixerTrack::new("A");
        assert_eq!(route_summary(&track), "M");
        track.sends.push(musiccraft_project::MixerSend {
            target: 5,
            level: 1.0,
        });
        assert_eq!(route_summary(&track), "M, 5");
        track.to_master = false;
        track.sends.clear();
        assert_eq!(route_summary(&track), "none");
    }

    #[test]
    fn toggling_tracks_keeps_the_last_one_clicked_primary() {
        let mut state = UiState::default();
        select(&mut state, 1, false);
        select(&mut state, 3, true);
        select(&mut state, 2, true);
        assert_eq!(selection(&state), [1, 2, 3]);
        assert_eq!(state.selected_track, 2);
        select(&mut state, 2, true);
        assert_eq!(selection(&state), [1, 3]);
        assert!(!state.mixer_selection.contains(&state.selected_track));
        make_primary(&mut state, 1);
        assert_eq!(state.selected_track, 1);
        select(&mut state, 4, false);
        assert_eq!(selection(&state), [4]);
    }

    #[test]
    fn ganged_pans_move_by_the_same_amount_and_clamp() {
        let mut project = Project::default();
        project.mixer[1].pan = 0.0;
        project.mixer[2].pan = 0.9;
        let invocation = ganged(&project, &[1, 2], 1, "mixer.set_pan", 0.25);
        assert_eq!(invocation.name, musiccraft_commands::BATCH);
        assert_eq!(
            invocation.args,
            ["mixer.set_pan 1 0.2500", "mixer.set_pan 2 1.0000"]
        );
    }

    #[test]
    fn switches_act_on_the_selection_only_when_their_track_is_in_it() {
        let mut state = UiState::default();
        select(&mut state, 1, false);
        select(&mut state, 2, true);
        assert_eq!(targets(&state, 1), [1, 2]);
        assert_eq!(targets(&state, 3), [3]);
        let mut actions = Vec::new();
        run_each(
            &mut actions,
            "mixer.set_phase_invert",
            &targets(&state, 2),
            "on",
        );
        run_each(
            &mut actions,
            "mixer.set_phase_invert",
            &targets(&state, 3),
            "on",
        );
        let ran: Vec<String> = actions
            .iter()
            .filter_map(|a| match a {
                Action::Run(i) => Some(i.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(ran.len(), 2);
        assert!(ran[0].starts_with(musiccraft_commands::BATCH), "{ran:?}");
        assert_eq!(ran[1], "mixer.set_phase_invert 3 on");
        let mut session = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        if let Action::Run(batch) = &actions[0] {
            session.run(batch).unwrap();
        }
        let mixer = &session.project().mixer;
        assert!(mixer[1].phase_invert && mixer[2].phase_invert && !mixer[3].phase_invert);
    }

    #[test]
    fn idle_inserts_are_untouched_unfed_and_unrouted() {
        let mut session = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        let last = session.project().mixer.len() - 1;
        let all: Vec<usize> = (1..=last).rev().collect();
        assert_eq!(
            idle_inserts(session.project()),
            all,
            "a new project's inserts are all idle"
        );
        for line in [
            "channel.add_synth",
            "channel.route 1 1",
            "mixer.set_send 2 3 0.5",
            "mixer.set_pan 4 0.5",
            "mixer.rename 5 Named",
        ] {
            session.run_line(line).unwrap();
        }
        let idle = idle_inserts(session.project());
        assert!(!idle.contains(&1), "a channel plays through 1");
        assert!(!idle.contains(&2), "2 sends to 3");
        assert!(!idle.contains(&3), "3 is fed by 2");
        assert!(!idle.contains(&4), "4 is panned");
        assert!(idle.contains(&5), "a name alone does not count");
        let mut actions = Vec::new();
        run_each(&mut actions, "mixer.remove_track", &idle, "");
        if let [Action::Run(batch)] = actions.as_slice() {
            session.run(batch).unwrap();
        }
        assert_eq!(session.project().mixer.len(), 5);
    }

    #[test]
    fn strip_colour_mixes_more_of_the_track_colour_as_it_strengthens() {
        let red = egui::Color32::from_rgb(220, 40, 40);
        assert_eq!(strip_fill(false, red, 0), theme::PANEL_DARK);
        assert_eq!(strip_fill(true, red, 0), theme::STRIP_SELECTED);
        let reds: Vec<u8> = (0..4).map(|t| strip_fill(false, red, t).r()).collect();
        assert!(reds.windows(2).all(|w| w[0] < w[1]), "{reds:?}");
    }
}
