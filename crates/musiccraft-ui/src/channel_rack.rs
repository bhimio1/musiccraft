//! The channel rack: one row per channel with mute, pan, volume, routing, name
//! and step buttons for the selected pattern.

use crate::hint::Hint;
use egui::{PointerButton, RichText, Sense, vec2};
use musiccraft_commands::Invocation;
use musiccraft_project::params::{BUILTIN_PREFIX, BUILTIN_SAMPLES, TICKS_PER_STEP};
use musiccraft_project::{Channel, DEFAULT_CHANNEL_VOLUME, Instrument, MAX_VOLUME, Project};

use crate::dialogs::{Dialog, FilePick, Pick};
use crate::{Action, Frame, UiState, format, theme, widgets};

const STEP_SIZE: egui::Vec2 = egui::Vec2::new(14.0, 24.0);
const NAME_WIDTH: f32 = 96.0;

fn run(actions: &mut Vec<Action>, name: &str, args: impl IntoIterator<Item = String>) {
    actions.push(Action::Run(Invocation::new(name, args)));
}

pub fn show(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let mut open = state.windows.channel_rack;
    let project = frame.project;
    let area = state.workspace;
    state.step_layout.clear();
    // The step under the playhead, and how far into it, while the current
    // pattern plays.
    let playing = crate::piano_roll::pattern_position(project, frame.engine.position_ticks)
        .filter(|_| frame.engine.playing)
        .map(|tick| {
            let into = (tick % TICKS_PER_STEP) as f32 / TICKS_PER_STEP as f32;
            ((tick / TICKS_PER_STEP) as usize, into)
        });
    let id = egui::Id::new("channel-rack");
    let slots = state.floating_area();
    let default = egui::Rect::from_min_size(
        slots.left_top() + egui::vec2(4.0, 4.0),
        egui::vec2((slots.width() - 200.0).clamp(420.0, 560.0), 0.0),
    );
    let window = open
        .then(|| {
            crate::chrome::window(ctx, id, "Channel rack", area, default, true).show(ctx, |ui| {
                let folded = crate::chrome::Header::new(id, "Channel rack").show(
                    ui,
                    &mut open,
                    &mut (&mut *state, &mut *actions),
                    |ui, (state, _)| window_menu(ui, state),
                    |ui, (state, actions)| header(ui, state, project, actions),
                );
                if folded {
                    return;
                }
                ui.separator();
                let area = egui::ScrollArea::both()
                    .id_salt("rack-scroll")
                    .auto_shrink([false, true])
                    .max_height(420.0);
                let bars = (egui::Id::new("rack-bars"), 27.0);
                crate::scrollbars::plain_area(ui, area, bars, |ui| {
                    state.rack_rows.clear();
                    for (index, channel) in project.channels.iter().enumerate() {
                        let shown = state
                            .rack_group
                            .as_ref()
                            .is_none_or(|group| *group == channel.group);
                        if shown {
                            row(
                                ui,
                                state,
                                project,
                                (index, playing, frame.settings.keep_instrument_windows),
                                channel,
                                actions,
                            );
                        }
                    }
                    if project.channels.is_empty() {
                        ui.label(
                            RichText::new(
                                "No channels yet. Add a sound from the browser or below.",
                            )
                            .color(theme::TEXT_DIM),
                        );
                    }
                });
                if state.graph.is_some() {
                    ui.separator();
                    graph_editor(ui, state, project, actions);
                }
                ui.separator();
                let first_new = actions.len();
                add_menu(ui, frame.plugins, actions);
                join_shown_group(state, project, &mut actions[first_new..]);
            })
        })
        .flatten();
    state.windows.channel_rack = open;
    crate::chrome::remember(ctx, id, window.as_ref().map(|w| &w.response));
    // A stroke whose row was not drawn when the button was let go is dropped.
    if !ctx.input(|i| i.pointer.any_down()) {
        state.step_paint = None;
    }
    // Sounds dragged from the browser and released over the rack become channels.
    if let Some(window) = window {
        let over = ctx
            .pointer_hover_pos()
            .is_some_and(|p| window.response.rect.contains(p));
        let focused = crate::chrome::focused(ctx, id);
        if open && focused && !state.typing_keyboard && !ctx.egui_wants_keyboard_input() {
            keys(ctx, state, project, actions);
        }
        let channel_payload = egui::DragAndDrop::payload::<crate::browser::Dragged>(ctx)
            .is_some_and(|d| d.add_channel().is_some());
        if over && channel_payload {
            drop_target(ctx, state, project, window.response.rect, actions);
        }
    }
}

/// A browser item over a channel's row replaces that channel's sound;
/// anywhere else in the rack it adds a channel. The row or the whole rack is
/// outlined to show which.
fn drop_target(
    ctx: &egui::Context,
    state: &UiState,
    project: &Project,
    window: egui::Rect,
    actions: &mut Vec<Action>,
) {
    let Some(pointer) = ctx.pointer_hover_pos() else {
        return;
    };
    let target = state
        .rack_rows
        .iter()
        .find(|(_, span)| span.contains(pointer.y))
        .filter(|(index, _)| *index < project.channels.len());
    let outline = match target {
        Some((_, span)) => {
            egui::Rect::from_x_y_ranges(window.x_range(), *span).shrink2(egui::vec2(4.0, 0.0))
        }
        None => window,
    };
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("rack-drop"),
    ))
    .rect_stroke(
        outline,
        if target.is_some() { 2.0 } else { 6.0 },
        egui::Stroke::new(2.0, theme::ACCENT),
        egui::StrokeKind::Inside,
    );
    if !ctx.input(|i| i.pointer.any_released()) {
        return;
    }
    let Some(dragged) = egui::DragAndDrop::take_payload::<crate::browser::Dragged>(ctx) else {
        return;
    };
    let invocation = match target {
        Some((index, _)) => replace_sound(project, *index, &dragged),
        None => dragged.add_channel(),
    };
    actions.extend(invocation.map(Action::Run));
}

/// The command that gives channel `index` the sound of `dragged`: a new
/// sample for channels that play one, otherwise the dragged item is added as
/// a channel, its sound swapped in, and the added channel removed again, as
/// one undo step.
fn replace_sound(
    project: &Project,
    index: usize,
    dragged: &crate::browser::Dragged,
) -> Option<Invocation> {
    let channel = project.channels.get(index)?;
    let number = (index + 1).to_string();
    if let Some(sample) = dragged.sample()
        && matches!(
            channel.instrument,
            Instrument::Sampler { .. } | Instrument::Granular { .. } | Instrument::Resynth { .. }
        )
    {
        return Some(Invocation::new(
            "channel.set_sample",
            [number, sample.to_owned()],
        ));
    }
    let add = dragged.add_channel()?;
    let added = (project.channels.len() + 1).to_string();
    Some(Invocation::new(
        musiccraft_commands::BATCH,
        [
            add.to_string(),
            Invocation::new("channel.swap_sound", [number, added.clone()]).to_string(),
            Invocation::new("channel.remove", [added]).to_string(),
        ],
    ))
}

fn header(ui: &mut egui::Ui, state: &mut UiState, project: &Project, actions: &mut Vec<Action>) {
    let row = ui.horizontal(|ui| {
        let mut groups: Vec<&str> = project
            .channels
            .iter()
            .map(|c| c.group.as_str())
            .filter(|g| !g.is_empty())
            .collect();
        groups.sort_unstable();
        groups.dedup();
        if state
            .rack_group
            .as_ref()
            .is_some_and(|g| !g.is_empty() && !groups.contains(&g.as_str()))
        {
            state.rack_group = None;
        }
        // Always shown, as a fixed part of the header, even before any
        // channel has a group.
        {
            let response = egui::ComboBox::from_id_salt("rack-group")
                .width(90.0)
                .selected_text(match &state.rack_group {
                    None => "All".to_owned(),
                    Some(g) if g.is_empty() => "No group".to_owned(),
                    Some(g) => g.clone(),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut state.rack_group, None, "All");
                    for group in &groups {
                        ui.selectable_value(
                            &mut state.rack_group,
                            Some((*group).to_owned()),
                            *group,
                        );
                    }
                    ui.selectable_value(&mut state.rack_group, Some(String::new()), "No group");
                })
                .response
                .on_hover_text(
                    "Group filter: show one group of channels · right-click to make, rename or remove a group",
                );
            crate::automation::context_menu(&response)
                .show(|ui| group_menu(ui, state, project, actions));
        }
        let pattern = project.current_pattern();
        ui.label(RichText::new(&pattern.name).color(theme::HIGHLIGHT));
        ui.label(RichText::new("swing").color(theme::TEXT_DIM))
            .on_hover_text("Project swing; each channel's swing mix scales it");
        let mut swing = (project.swing * 100.0).round();
        let response = ui.add(
            egui::DragValue::new(&mut swing)
                .range(0.0..=100.0)
                .speed(0.5)
                .suffix("%"),
        );
        if response.changed() {
            run(actions, "project.set_swing", [format!("{swing}")]);
        }
        ui.label(RichText::new("steps").color(theme::TEXT_DIM));
        // An auto length shows the steps it comes to; dragging fixes it.
        let shown = pattern.step_span();
        let mut steps = shown;
        let mut field = egui::DragValue::new(&mut steps)
            .range(1..=musiccraft_project::MAX_PATTERN_STEPS)
            .speed(0.2);
        if pattern.auto_length {
            field = field.suffix(" auto");
        }
        let response = ui.add(field);
        let pattern_number = (project.current_pattern + 1).to_string();
        if response.changed() && steps != shown {
            run(
                actions,
                "pattern.set_length",
                [pattern_number.clone(), steps.to_string()],
            );
        }
        // Right-click: whole-bar lengths in the pattern's own metre.
        let bar = pattern.time_signature.bar_steps();
        response
            .hint("Pattern length in steps · drag, or right-click for auto and whole bars")
            .context_menu(|ui| {
                let auto = egui::Button::selectable(pattern.auto_length, "Auto (follows the notes)");
                if ui.add(auto).clicked() {
                    run(
                        actions,
                        "pattern.set_length",
                        [pattern_number.clone(), "auto".to_owned()],
                    );
                    ui.close();
                }
                ui.separator();
                for bars in [1, 2, 4, 8, 16, 32] {
                    let length = bar * bars;
                    if length > musiccraft_project::MAX_PATTERN_STEPS {
                        break;
                    }
                    let label = if bars == 1 {
                        "1 bar".to_owned()
                    } else {
                        format!("{bars} bars")
                    };
                    let current = !pattern.auto_length && length == pattern.length_steps;
                    if ui.selectable_label(current, label).clicked() {
                        run(
                            actions,
                            "pattern.set_length",
                            [pattern_number.clone(), length.to_string()],
                        );
                        ui.close();
                    }
                }
            });
        let mut open = state.graph.is_some();
        if ui
            .toggle_value(&mut open, "Graph")
            .on_hover_text(
                "Edit velocity, pan, fine pitch and more per step of the selected channel",
            )
            .changed()
        {
            state.graph = open.then_some(crate::piano_roll::LaneMode::Velocity);
        }
    });
    // Shift+wheel over the header row steps through the patterns.
    let shift = ui.input(|i| i.modifiers.shift);
    let notches = if shift && row.response.contains_pointer() {
        crate::scrollbars::wheel_notches(ui)
    } else {
        0.0
    };
    if notches < 0.0 {
        run(actions, "pattern.next", [] as [String; 0]);
    } else if notches > 0.0 {
        run(actions, "pattern.prev", [] as [String; 0]);
    }
}

/// One channel's row; `at` is the channel's index and the playing step.
/// Opens an extra channel window for `channel` (no-op if one is open).
/// Ctrl+press on a channel's name plays its C5 until the button is let go.
fn name_preview(
    ui: &egui::Ui,
    response: &egui::Response,
    index: usize,
    ctrl: bool,
    actions: &mut Vec<Action>,
) {
    let id = response.id.with("preview");
    let sounding = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let held = ctrl && response.is_pointer_button_down_on();
    let key = musiccraft_project::STEP_KEY;
    if held && !sounding {
        actions.push(Action::Preview {
            channel: index,
            key,
            on: true,
        });
        ui.data_mut(|d| d.insert_temp(id, true));
    } else if !held && sounding {
        actions.push(Action::Preview {
            channel: index,
            key,
            on: false,
        });
        ui.data_mut(|d| d.insert_temp(id, false));
    }
}

/// Shift+wheel over a channel's name moves the channel up or down a row.
fn name_wheel(
    ui: &egui::Ui,
    response: &egui::Response,
    (index, count): (usize, usize),
    actions: &mut Vec<Action>,
) {
    if !response.hovered() || !ui.input(|i| i.modifiers.shift) {
        return;
    }
    let notches = crate::scrollbars::wheel_notches(ui);
    let number = (index + 1).to_string();
    if notches > 0.0 && index > 0 {
        run(actions, "channel.move", [number, index.to_string()]);
    } else if notches < 0.0 && index + 1 < count {
        run(actions, "channel.move", [number, (index + 2).to_string()]);
    }
}

/// Alt+click on a grouped channel's mute light: mutes or unmutes the whole
/// group to match the opposite of this channel's state, or with Ctrl solos
/// exactly that group (again on a group that is the whole solo clears it).
/// One undo step either way.
fn group_light(project: &Project, index: usize, solo: bool) -> Option<Action> {
    let group = &project.channels.get(index)?.group;
    let members = |i: usize| project.channels[i].group == *group;
    let mut lines = Vec::new();
    if solo {
        let only = (0..project.channels.len()).all(|i| project.channels[i].solo == members(i));
        for (i, c) in project.channels.iter().enumerate() {
            let want = !only && members(i);
            if c.solo != want {
                let on = if want { "on" } else { "off" };
                lines.push(Invocation::new(
                    "channel.set_solo",
                    [(i + 1).to_string(), on.into()],
                ));
            }
        }
    } else {
        let mute = !project.channels[index].mute;
        for (i, c) in project.channels.iter().enumerate() {
            if members(i) && c.mute != mute {
                let on = if mute { "on" } else { "off" };
                lines.push(Invocation::new(
                    "channel.set_mute",
                    [(i + 1).to_string(), on.into()],
                ));
            }
        }
    }
    batch(lines).map(Action::Run)
}

/// The mute light of a channel kept out of solo: a small padlock whose body
/// is lit while the channel is not muted.
fn padlock(ui: &mut egui::Ui, plays: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
    let painter = ui.painter();
    let c = rect.center();
    let stroke = egui::Stroke::new(1.2, theme::TEXT_DIM);
    let body = egui::Rect::from_center_size(c + egui::vec2(0.0, 2.0), egui::vec2(9.0, 6.5));
    let fill = if plays {
        theme::ACCENT
    } else {
        theme::PANEL_DARK
    };
    painter.rect(body, 1.5, fill, stroke, egui::StrokeKind::Middle);
    let arch: Vec<egui::Pos2> = (0..=8)
        .map(|i| {
            let a = std::f32::consts::PI * i as f32 / 8.0;
            c + egui::vec2(-2.8 * a.cos(), -1.2 - 3.2 * a.sin())
        })
        .collect();
    painter.line(arch, stroke);
    response
}

fn keep_window(state: &mut UiState, channel: musiccraft_project::ChannelId) {
    if !state.kept_channels.iter().any(|(c, _)| *c == channel) {
        state.kept_channels.push((channel, state.channel_tab));
    }
}

fn row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    at: (usize, Option<(usize, f32)>, bool),
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    let (index, playing, keep_windows) = at;
    let number = (index + 1).to_string();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        // Lit: plays. Soloed: a ring in the highlight tone. Silenced by
        // another channel's solo: lit at reduced strength.
        let silenced = !channel.mute && !project.channel_plays(index);
        let tone = if silenced {
            theme::at_strength(theme::ACCENT, 90)
        } else {
            theme::ACCENT
        };
        let light = if channel.solo_lock {
            padlock(ui, !channel.mute)
                .hint("Mute (kept out of solo) · Shift+click: unlock · Ctrl+click: solo")
        } else {
            widgets::dot(ui, !channel.mute, tone).hint(if silenced {
                "Silenced by another channel's solo · click: mute · Ctrl+click: solo · Shift+click: lock"
            } else {
                "Mute · Ctrl+click: solo · Shift+click: lock out of solo · right-click for options"
            })
        };
        if channel.solo {
            ui.painter().circle_stroke(
                light.rect.center(),
                6.5,
                egui::Stroke::new(1.5, theme::HIGHLIGHT),
            );
        }
        if light.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            if modifiers.shift {
                let lock = if channel.solo_lock { "off" } else { "on" };
                run(actions, "channel.set_solo_lock", [number.clone(), lock.to_owned()]);
            } else if modifiers.alt && !channel.group.is_empty() {
                actions.extend(group_light(project, index, modifiers.command));
            } else {
                let command = if modifiers.command {
                    "channel.solo"
                } else {
                    "channel.toggle_mute"
                };
                run(actions, command, [number.clone()]);
            }
        }
        light.context_menu(|ui| {
            let mute = if channel.mute { "Unmute" } else { "Mute" };
            if ui.button(mute).clicked() {
                run(actions, "channel.toggle_mute", [number.clone()]);
                ui.close();
            }
            let solo = if channel.solo { "Unsolo" } else { "Solo" };
            if ui.button(solo).clicked() {
                let on = if channel.solo { "off" } else { "on" };
                run(actions, "channel.set_solo", [number.clone(), on.to_owned()]);
                ui.close();
            }
            let mut lock = channel.solo_lock;
            if ui.checkbox(&mut lock, "Keep out of solo").clicked() {
                let on = if lock { "on" } else { "off" };
                run(actions, "channel.set_solo_lock", [number.clone(), on.to_owned()]);
                ui.close();
            }
            let muted: Vec<String> = project
                .channels
                .iter()
                .enumerate()
                .filter(|(_, c)| c.mute)
                .map(|(i, _)| format!("channel.set_mute {} off", i + 1))
                .collect();
            if crate::widgets::add_enabled(ui, !muted.is_empty(), egui::Button::new("Unmute all"))
                .clicked()
            {
                actions.push(Action::Run(Invocation::new(
                    musiccraft_commands::BATCH,
                    muted,
                )));
                ui.close();
            }
        });
        let pan = widgets::knob(ui, channel.pan, -1.0, 1.0, 0.0, true);
        crate::automation::menu(&pan.response, &format!("channel:{number}:pan"), actions);
        if pan.started {
            state.begin_gesture();
        }
        if let Some(value) = pan.changed {
            actions.push(Action::Coalesced(
                Invocation::new("channel.set_pan", [number.clone(), format!("{value:.3}")]),
                state.gesture(),
            ));
        }
        pan.response
            .hint(format!("Pan {}", format::pan(channel.pan)));
        let volume = widgets::knob(
            ui,
            channel.volume,
            0.0,
            MAX_VOLUME,
            DEFAULT_CHANNEL_VOLUME,
            false,
        );
        crate::automation::menu(
            &volume.response,
            &format!("channel:{number}:volume"),
            actions,
        );
        if volume.started {
            state.begin_gesture();
        }
        if let Some(value) = volume.changed {
            actions.push(Action::Coalesced(
                Invocation::new(
                    "channel.set_volume",
                    [number.clone(), format!("{value:.3}")],
                ),
                state.gesture(),
            ));
        }
        volume
            .response
            .hint(format!("Volume {}", format::level(channel.volume)));

        let silent = matches!(
            channel.instrument,
            Instrument::Layer { .. } | Instrument::MidiOut { .. }
        );
        let mut track = channel.mixer_track;
        // The mixer-track column can be hidden (Alt+M).
        let column = !state.rack_hide_tracks;
        if column && silent {
            ui.add_sized(
                vec2(28.0, 18.0),
                egui::Label::new(RichText::new("–").color(theme::TEXT_FAINT)),
            )
            .hint("Makes no sound of its own, so it plays through no mixer track");
        } else if column {
            let route = ui
                .add_sized(
                    vec2(28.0, 18.0),
                    egui::DragValue::new(&mut track)
                        .range(0..=project.mixer.len().saturating_sub(1))
                        .speed(0.1)
                        .custom_formatter(|v, _| {
                            if v < 0.5 {
                                "M".to_owned()
                            } else {
                                format!("{v:.0}")
                            }
                        })
                        .custom_parser(|text| {
                            let text = text.trim().trim_start_matches('→').trim();
                            if text.eq_ignore_ascii_case("mst") || text.eq_ignore_ascii_case("m") {
                                Some(0.0)
                            } else {
                                text.parse().ok()
                            }
                        }),
                )
                .hint(format!(
                    "Plays through mixer track: {}. Drag up or down to change",
                    route_name(project, channel.mixer_track)
                ));
            if route.changed() && track != channel.mixer_track {
                run(
                    actions,
                    "channel.route",
                    [number.clone(), track.to_string()],
                );
            }
        }

        let selected = state.selected_channel == index;
        let name = egui::Button::new(RichText::new(&channel.name).color(if selected {
            theme::BACKGROUND
        } else {
            theme::TEXT
        }))
        .fill({
            // The channel's colour: full when selected, muted otherwise.
            let color = crate::channel_color(project, index);
            if selected {
                color
            } else {
                theme::mix(theme::RAISED, color, 0.35)
            }
        })
        .truncate();
        let response =
            ui.add_sized(vec2(NAME_WIDTH, STEP_SIZE.y), name)
                .hint(match &channel.instrument {
                    Instrument::Sampler { sample } => format!("Sampler: {sample}"),
                    Instrument::Synth => "Synth".to_owned(),
                    Instrument::Antiserum { patch } => format!("Antiserum: {}", patch.name),
                    Instrument::DrumSynth => "Drum synth".to_owned(),
                    Instrument::Pluck => "Pluck".to_owned(),
                    Instrument::Fm => "FM synth".to_owned(),
                    Instrument::Keys => "Keys".to_owned(),
                    Instrument::Additive => "Additive synth".to_owned(),
                    Instrument::Granular { sample } => format!("Granular: {sample}"),
                    Instrument::Resynth { sample } => format!("Resynth: {sample}"),
                    Instrument::Kit { .. } => "Drum kit (16 pads)".to_owned(),
                    Instrument::Zones { zones } => format!("Zone sampler ({} zones)", zones.len()),
                    Instrument::Layer { channels } => {
                        format!("Layer of {} channels", channels.len())
                    }
                    Instrument::Plugin { plugin } => {
                        format!("Plugin: {} ({})", plugin.name, plugin.id)
                    }
                    Instrument::MidiOut { midi_channel } => {
                        format!("MIDI Out: external gear on MIDI channel {midi_channel}")
                    }
                });
        // The row's height, where a sound dropped from the browser lands.
        let clip = ui.clip_rect().y_range();
        let span = response.rect.y_range();
        if span.max > clip.min && span.min < clip.max {
            state.rack_rows.push((index, span));
        }
        let ctrl = ui.input(|i| i.modifiers.command);
        name_preview(ui, &response, index, ctrl, actions);
        name_wheel(ui, &response, (index, project.channels.len()), actions);
        if ctrl {
            // Ctrl+press only previews.
        } else if response.double_clicked() {
            // The first click opened the settings; keep them open.
            state.windows.channel_settings = true;
            state.rename_in_place(Dialog::rename_channel(index, &channel.name), response.rect);
        } else if response.clicked() {
            let alt = ui.input(|i| i.modifiers.alt);
            let other = state.selected_channel != index && state.windows.channel_settings;
            if alt || (keep_windows && other) {
                // Another window, beside the one already open.
                keep_window(state, channel.id);
            } else {
                // Clicking the selected channel again closes its settings.
                let reopen = state.selected_channel != index || !state.windows.channel_settings;
                state.selected_channel = index;
                state.windows.channel_settings = reopen;
            }
        }
        response.context_menu(|ui| context_menu(ui, state, project, index, channel, actions));

        steps(ui, state, project, (index, playing), channel, actions);
        // The selection switch at the end of the row, set apart from the steps.
        ui.add_space(4.0);
        let (rect, switch) = ui.allocate_exact_size(vec2(8.0, STEP_SIZE.y), egui::Sense::click());
        let pill = rect.shrink2(vec2(1.5, 1.0));
        if selected {
            ui.painter().rect_filled(pill, 2.0, theme::ACCENT);
        } else {
            ui.painter().rect_stroke(
                pill,
                2.0,
                egui::Stroke::new(1.0, theme::RAISED_HOVER),
                egui::StrokeKind::Inside,
            );
        }
        if switch.hint("Select this channel").clicked() {
            state.selected_channel = index;
        }
    });
}

pub(crate) fn context_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    index: usize,
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    let number = (index + 1).to_string();
    // Two columns: the channel itself, then its steps and order. Every line
    // is painted once the columns are laid out, to their own size: a
    // separator takes all the room left in the popup, so the menu grew each
    // frame until it covered the screen (issue #20).
    ui.horizontal_top(|ui| {
        let left = Dividers::column(ui, |ui, lines| {
            channel_items(ui, state, project, index, channel, lines, actions);
        });
        let gap = ui.spacing().item_spacing.x;
        let (line, _) = ui.allocate_exact_size(vec2(gap, 0.0), Sense::hover());
        let right = Dividers::column(ui, |ui, lines| {
            step_items(ui, state, project, index, &number, lines, actions);
        });
        let bottom = left.bottom().max(right.bottom());
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        ui.painter()
            .vline(line.center().x, line.top()..=bottom, stroke);
    });
}

/// Where the lines between groups of a menu column go, painted across the
/// column once its width is known.
struct Dividers(Vec<f32>);

impl Dividers {
    /// Lays out a column with `add` and paints its lines. Returns its rect.
    fn column(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut Self)) -> egui::Rect {
        let mut lines = Self(Vec::new());
        let rect = ui.vertical(|ui| add(ui, &mut lines)).response.rect;
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        for y in lines.0 {
            ui.painter().hline(rect.x_range(), y, stroke);
        }
        rect
    }

    /// Leaves room for a line here.
    fn mark(&mut self, ui: &mut egui::Ui) {
        let spacing = ui.spacing().item_spacing.y;
        let (rect, _) = ui.allocate_exact_size(vec2(0.0, spacing), Sense::hover());
        self.0.push(rect.center().y);
    }
}

/// The channel menu's first column: open, look, sound, clone and delete.
fn channel_items(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    index: usize,
    channel: &Channel,
    lines: &mut Dividers,
    actions: &mut Vec<Action>,
) {
    let number = (index + 1).to_string();
    if ui.button("Open in piano roll").clicked() {
        state.selected_channel = index;
        state.windows.piano_roll = true;
        ui.close();
    }
    if ui.button("Channel settings").clicked() {
        state.selected_channel = index;
        state.windows.channel_settings = true;
        ui.close();
    }
    lines.mark(ui);
    if ui.button("Rename…").clicked() {
        state.open_dialog(Dialog::rename_channel(index, &channel.name));
        ui.close();
    }
    if let Instrument::Sampler { sample } = &channel.instrument {
        ui.menu_button("Change sample", |ui| {
            let choice = crate::sample_slot::menu_items(ui, state, false);
            let pick = || FilePick::sample(Pick::SetSample(index), sample, project);
            let set = |sound| Invocation::new("channel.set_sample", [number.clone(), sound]);
            crate::sample_slot::apply(state, choice, pick, set, actions);
        });
        if ui
            .button("Split into stems")
            .on_hover_text(
                "Split this sound into vocals, drums, bass and other, each a new channel",
            )
            .clicked()
        {
            run(actions, "sample.split_stems", [number.clone()]);
            ui.close();
        }
    }
    ui.menu_button("Colour", |ui| {
        if let Some(choice) = crate::swatches::grid(ui, channel.color) {
            run(
                actions,
                "channel.set_color",
                [number.clone(), crate::swatches::arg(choice)],
            );
            ui.close();
        }
    });
    if ui.button("Group…").clicked() {
        state.open_dialog(crate::dialogs::Dialog::channel_group(index, &channel.group));
        ui.close();
    }
    lines.mark(ui);
    if ui.button("Clone").clicked() {
        run(actions, "channel.clone", [number.clone()]);
        ui.close();
    }
    if ui
        .button(RichText::new("Delete").color(theme::DANGER))
        .clicked()
    {
        run(actions, "channel.remove", [number]);
        ui.close();
    }
}

/// The channel menu's second column: steps, solo and order.
fn step_items(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    index: usize,
    number: &str,
    lines: &mut Dividers,
    actions: &mut Vec<Action>,
) {
    use crate::ClipboardKey;
    let number = number.to_owned();
    let pasting = state.rack_clipboard.is_some();
    for (label, keys, op, enabled) in [
        ("Cut steps", "Ctrl+X", ClipboardKey::Cut, true),
        ("Copy steps", "Ctrl+C", ClipboardKey::Copy, true),
        ("Paste steps", "Ctrl+V", ClipboardKey::Paste, pasting),
    ] {
        let item = egui::Button::new(label).shortcut_text(keys);
        if crate::widgets::add_enabled(ui, enabled, item).clicked() {
            step_clipboard(ui.ctx(), state, project, (index, op), actions);
            ui.close();
        }
    }
    if ui.button("Clear notes in pattern").clicked() {
        run(actions, "note.clear", [number.clone()]);
        ui.close();
    }
    lines.mark(ui);
    for (every, nth) in [(2, "2nd"), (4, "4th"), (8, "8th")] {
        if ui.button(format!("Turn on every {nth} step")).clicked() {
            run(actions, "step.fill", [number.clone(), every.to_string()]);
            ui.close();
        }
    }
    lines.mark(ui);
    if ui.button("Shift left").clicked() {
        run(actions, "step.shift", [number.clone(), "-1".to_owned()]);
        ui.close();
    }
    if ui.button("Shift right").clicked() {
        run(actions, "step.shift", [number.clone(), "1".to_owned()]);
        ui.close();
    }
    lines.mark(ui);
    let graph = egui::Button::new("Graph editor").shortcut_text("Ctrl+K");
    if ui.add(graph).clicked() {
        state.selected_channel = index;
        state.graph = Some(state.graph.unwrap_or(crate::piano_roll::LaneMode::Velocity));
        ui.close();
    }
    if ui.button("Solo").clicked() {
        run(actions, "channel.solo", [number.clone()]);
        ui.close();
    }
    crate::widgets::enabled_ui(ui, index > 0, |ui| {
        if ui.button("Move up").clicked() {
            run(actions, "channel.move", [number.clone(), index.to_string()]);
            ui.close();
        }
    });
    crate::widgets::enabled_ui(ui, index + 1 < project.channels.len(), |ui| {
        if ui.button("Move down").clicked() {
            run(
                actions,
                "channel.move",
                [number.clone(), (index + 2).to_string()],
            );
            ui.close();
        }
    });
}

/// Whether a channel's notes are all plain steps (key C5, one step long, on
/// the step grid), so the step buttons show them fully.
fn is_step_only(notes: &[&musiccraft_project::Note]) -> bool {
    notes.iter().all(|n| {
        n.key == musiccraft_project::STEP_KEY
            && n.length == TICKS_PER_STEP
            && n.start % TICKS_PER_STEP == 0
    })
}

fn steps(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    at: (usize, Option<(usize, f32)>),
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    let (index, playing) = at;
    let pattern = project.current_pattern();
    let count = pattern.length_ticks().div_ceil(TICKS_PER_STEP);
    // Alternate beats of the pattern's signature are shaded.
    let beat_steps = (pattern.time_signature.beat_ticks() / TICKS_PER_STEP).max(1) as usize;
    let notes: Vec<&musiccraft_project::Note> = pattern.notes_of(channel.id).collect();
    ui.add_space(6.0);
    if !is_step_only(&notes) {
        let bar_steps = (pattern.time_signature.bar_ticks() / TICKS_PER_STEP).max(1);
        let color = crate::channel_color(project, index);
        mini_roll(ui, state, (index, bar_steps, color), &notes, count);
        return;
    }
    let mut on = vec![false; count as usize];
    let mut velocity = vec![0_u8; count as usize];
    for note in &notes {
        let step = (note.start / TICKS_PER_STEP) as usize;
        if let (Some(slot), Some(v)) = (on.get_mut(step), velocity.get_mut(step)) {
            *slot = true;
            *v = (*v).max(note.velocity);
        }
    }
    let pointer = ui.input(|i| {
        (
            i.pointer.latest_pos(),
            i.pointer.button_pressed(PointerButton::Primary),
            i.pointer.button_pressed(PointerButton::Secondary),
            i.pointer.any_down(),
        )
    });
    let (pos, left_pressed, right_pressed, down) = pointer;
    // The stroke ends when the button is let go: every step it passed
    // changes in one undo step.
    if !down
        && state
            .step_paint
            .as_ref()
            .is_some_and(|s| s.channel == index)
    {
        if let Some(stroke) = state.step_paint.take() {
            let changes = stroke
                .steps
                .iter()
                .filter(|step| on.get(**step).copied() != Some(stroke.on))
                .map(|step| {
                    Invocation::new(
                        "step.set",
                        [
                            (index + 1).to_string(),
                            step.to_string(),
                            if stroke.on { "on" } else { "off" }.to_owned(),
                        ],
                    )
                })
                .collect();
            actions.extend(batch(changes).map(Action::Run));
        }
    }
    for (step, lit) in on.iter().enumerate() {
        let (rect, response) = ui.allocate_exact_size(STEP_SIZE, Sense::click_and_drag());
        if step < 2 {
            state.step_layout.push((index, rect));
        }
        let under = pos.is_some_and(|p| rect.contains(p));
        // A press starts a stroke: the left button sets the opposite of
        // this step, the right button clears.
        if under && (left_pressed || right_pressed) && response.hovered() {
            let paint = left_pressed && !lit;
            state.step_paint = Some(StepStroke {
                channel: index,
                on: paint,
                steps: Vec::new(),
            });
            if paint {
                actions.push(Action::PreviewShort {
                    channel: index,
                    key: 60,
                });
            }
        }
        let mut shown = *lit;
        if let Some(stroke) = state.step_paint.as_mut().filter(|s| s.channel == index) {
            if under && down && !stroke.steps.contains(&step) {
                stroke.steps.push(step);
            }
            if stroke.steps.contains(&step) {
                shown = stroke.on;
            }
        }
        let odd = (step / beat_steps) % 2 == 1;
        let base = if odd { theme::STEP_B } else { theme::STEP_A };
        // A step lit by the stroke in progress shows at full velocity.
        let level = match velocity.get(step).copied() {
            Some(v) if *lit => v,
            _ => 100,
        };
        // Hover lifts an off step one tone, never darker than its group.
        let fill = if shown {
            let lit = theme::step_lit(crate::channel_color(project, index), odd, level);
            match playing {
                Some((at, into)) if at == step => theme::mix(lit, theme::TEXT, step_flash(into)),
                _ => lit,
            }
        } else if response.hovered() {
            theme::STEP_HOVER
        } else {
            base
        };
        ui.painter().rect_filled(rect.shrink(1.0), 3.0, fill);
        if playing.is_some_and(|(at, _)| at == step) {
            ui.painter().rect_filled(rect, 2.0, theme::STEP_PLAYING);
        }
    }
}

/// How far a lit step under the playhead brightens toward the text colour,
/// `into` its step: a flash at the step's start that fades by its end.
fn step_flash(into: f32) -> f32 {
    0.6 - 0.45 * into.clamp(0.0, 1.0)
}

/// Wraps several invocations into one undo step.
fn batch(invocations: Vec<Invocation>) -> Option<Invocation> {
    match invocations.len() {
        0 => None,
        1 => invocations.into_iter().next(),
        _ => Some(Invocation::new(
            musiccraft_commands::BATCH,
            invocations.iter().map(ToString::to_string),
        )),
    }
}

/// A drag across a channel's step buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StepStroke {
    /// The channel row.
    pub channel: usize,
    /// Whether the steps passed turn on (left button from an unlit step) or
    /// off.
    pub on: bool,
    /// The steps passed so far.
    pub steps: Vec<usize>,
}

/// A miniature piano roll in place of the step buttons, for melodic channels.
fn mini_roll(
    ui: &mut egui::Ui,
    state: &mut UiState,
    (index, bar_steps, color): (usize, u32, egui::Color32),
    notes: &[&musiccraft_project::Note],
    steps: u32,
) {
    let size = vec2(STEP_SIZE.x * steps as f32, STEP_SIZE.y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect.shrink(1.0), 3.0, theme::PANEL_DARK);
    // Alternate bars are shaded.
    let bar = bar_steps as f32 * STEP_SIZE.x;
    for k in 0..steps.div_ceil(bar_steps) {
        let x = rect.left() + k as f32 * bar;
        if k % 2 == 1 {
            let band = egui::Rect::from_min_max(
                egui::pos2(x, rect.top() + 1.0),
                egui::pos2((x + bar).min(rect.right()), rect.bottom() - 1.0),
            );
            painter.rect_filled(band, 0.0, theme::BLACK_KEY_ROW);
        }
    }
    let (lo, hi) = notes
        .iter()
        .fold((127_u8, 0_u8), |(lo, hi), n| (lo.min(n.key), hi.max(n.key)));
    let span = f32::from(hi.saturating_sub(lo).max(1));
    let total = (steps * TICKS_PER_STEP) as f32;
    for note in notes {
        let x0 = rect.left() + note.start as f32 / total * rect.width();
        let x1 = rect.left() + (note.start + note.length) as f32 / total * rect.width();
        let t = if hi == lo {
            0.5
        } else {
            f32::from(note.key - lo) / span
        };
        let y = rect.bottom() - 4.0 - t * (rect.height() - 8.0);
        painter.line_segment(
            [
                egui::pos2(x0 + 1.0, y),
                egui::pos2(x1.max(x0 + 3.0) - 1.0, y),
            ],
            egui::Stroke::new(2.5, color),
        );
    }
    if response.hovered() {
        painter.rect_stroke(
            rect.shrink(1.0),
            3.0,
            egui::Stroke::new(1.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    if response
        .hint("Melodic pattern: click to open or close the piano roll")
        .clicked()
    {
        let showing = state.windows.piano_roll && state.selected_channel == index;
        state.selected_channel = index;
        state.windows.piano_roll = !showing;
    }
}

/// While the rack shows one group, a channel added from the plus joins it: the
/// add becomes one undo step with `channel.set_group`.
fn join_shown_group(state: &UiState, project: &Project, added: &mut [Action]) {
    let Some(group) = state.rack_group.as_ref().filter(|g| !g.is_empty()) else {
        return;
    };
    let number = (project.channels.len() + 1).to_string();
    for action in added {
        if let Action::Run(add) = action
            && add.name.starts_with("channel.add_")
        {
            let join = Invocation::new("channel.set_group", [number.clone(), group.clone()]);
            *add = Invocation::new(
                musiccraft_commands::BATCH,
                [add.to_string(), join.to_string()],
            );
        }
    }
}

fn add_menu(ui: &mut egui::Ui, plugins: &crate::Plugins, actions: &mut Vec<Action>) {
    ui.vertical_centered(|ui| {
        ui.menu_button(RichText::new("+").size(theme::FONT_DISPLAY), |ui| {
            for name in BUILTIN_SAMPLES {
                if ui.button(name.replace('_', " ")).clicked() {
                    run(
                        actions,
                        "channel.add_sampler",
                        [format!("{BUILTIN_PREFIX}{name}")],
                    );
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Synth").clicked() {
                actions.push(Action::Run(Invocation::bare("channel.add_synth")));
                ui.close();
            }
            ui.menu_button("Drum synth", |ui| {
                for kind in musiccraft_project::params::DRUM_TYPES {
                    if ui.button(kind).clicked() {
                        actions.push(Action::Run(Invocation::new("channel.add_drum", [kind])));
                        ui.close();
                    }
                }
            });
            if ui.button("Pluck").clicked() {
                actions.push(Action::Run(Invocation::bare("channel.add_pluck")));
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
            ui.menu_button("Plugin instrument", |ui| {
                crate::plugins::add_menu(ui, plugins, true, actions, |p| {
                    Invocation::new("channel.add_plugin", [p.id.clone(), p.name.clone()])
                });
            });
        })
        .response
        .hint("Add a channel");
    });
}

/// Keys while the rack is the focused window: 1–0 mute channels 1–10 (Ctrl:
/// solo), and for the selected channel Alt+C clone, Alt+Delete delete,
/// Alt+Up/Down move, Ctrl+L route to a free mixer track and
/// Shift+Ctrl+Left/Right shift its steps.
fn keys(ctx: &egui::Context, state: &mut UiState, project: &Project, actions: &mut Vec<Action>) {
    use egui::{Key, Modifiers};
    let pressed = |modifiers: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(modifiers, key));
    let digits = [
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
        Key::Num0,
    ];
    for (i, key) in digits.into_iter().enumerate() {
        let number = (i + 1).to_string();
        if i >= project.channels.len() {
            continue;
        }
        if pressed(Modifiers::COMMAND, key) {
            run(actions, "channel.solo", [number]);
        } else if pressed(Modifiers::NONE, key) {
            run(actions, "channel.toggle_mute", [number]);
        }
    }
    let selected = state.selected_channel;
    if selected >= project.channels.len() {
        return;
    }
    let number = (selected + 1).to_string();
    if pressed(Modifiers::ALT, Key::C) {
        run(actions, "channel.clone", [number.clone()]);
    }
    if pressed(Modifiers::ALT, Key::Delete) {
        run(actions, "channel.remove", [number.clone()]);
    }
    if pressed(Modifiers::ALT, Key::ArrowUp) && selected > 0 {
        run(
            actions,
            "channel.move",
            [number.clone(), selected.to_string()],
        );
        state.selected_channel -= 1;
    }
    if pressed(Modifiers::ALT, Key::ArrowDown) && selected + 1 < project.channels.len() {
        run(
            actions,
            "channel.move",
            [number.clone(), (selected + 2).to_string()],
        );
        state.selected_channel += 1;
    }
    if pressed(Modifiers::COMMAND, Key::L) {
        run(actions, "channel.route_free", [number.clone()]);
    }
    let shift_ctrl = Modifiers::COMMAND | Modifiers::SHIFT;
    if pressed(shift_ctrl, Key::ArrowLeft) {
        run(actions, "step.shift", [number.clone(), "-1".to_owned()]);
    }
    if pressed(shift_ctrl, Key::ArrowRight) {
        run(actions, "step.shift", [number, "1".to_owned()]);
    }
    if pressed(Modifiers::ALT, Key::M) {
        state.rack_hide_tracks = !state.rack_hide_tracks;
    }
    if pressed(Modifiers::COMMAND, Key::K) {
        state.graph = match state.graph {
            Some(_) => None,
            None => Some(crate::piano_roll::LaneMode::Velocity),
        };
    }
    if pressed(Modifiers::NONE, Key::F2) {
        let name = &project.channels[selected].name;
        state.open_dialog(Dialog::rename_channel(selected, name));
    }
    for (key, forward) in [(Key::PageDown, true), (Key::PageUp, false)] {
        if pressed(Modifiers::NONE, key) {
            state.rack_group = step_group(project, state.rack_group.as_ref(), forward);
        }
    }
    use crate::ClipboardKey;
    for op in [ClipboardKey::Copy, ClipboardKey::Cut, ClipboardKey::Paste] {
        if crate::clipboard_key(ctx, op) {
            step_clipboard(ctx, state, project, (selected, op), actions);
        }
    }
}

/// Copies, cuts or pastes the notes of channel `index` in the current
/// pattern; a paste replaces them with the copied ones (one undo step).
fn step_clipboard(
    ctx: &egui::Context,
    state: &mut UiState,
    project: &Project,
    (index, op): (usize, crate::ClipboardKey),
    actions: &mut Vec<Action>,
) {
    use crate::ClipboardKey;
    let Some(channel) = project.channels.get(index) else {
        return;
    };
    let notes: Vec<musiccraft_project::Note> = project
        .current_pattern()
        .notes_of(channel.id)
        .copied()
        .collect();
    match op {
        ClipboardKey::Copy | ClipboardKey::Cut => {
            if op == ClipboardKey::Cut {
                let cut = crate::piano_roll::replace_notes(index, &notes, &[]);
                actions.extend(cut.map(Action::Run));
            }
            state.rack_clipboard = Some(notes);
            crate::copied(ctx, "steps");
        }
        ClipboardKey::Paste => {
            if let Some(copied) = &state.rack_clipboard {
                let paste = crate::piano_roll::replace_notes(index, &notes, copied);
                actions.extend(paste.map(Action::Run));
            }
        }
    }
}

/// The rack's own window-menu items.
fn window_menu(ui: &mut egui::Ui, state: &mut UiState) {
    let mut graph = state.graph.is_some();
    if ui.checkbox(&mut graph, "Graph editor (Ctrl+K)").changed() {
        state.graph = graph.then_some(crate::piano_roll::LaneMode::Velocity);
    }
    let mut tracks = !state.rack_hide_tracks;
    if ui
        .checkbox(&mut tracks, "Mixer-track column (Alt+M)")
        .changed()
    {
        state.rack_hide_tracks = !tracks;
    }
}

/// The group filter's right-click menu: a new group for the selected
/// channel, and renaming or removing the group shown.
fn group_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    actions: &mut Vec<Action>,
) {
    if let Some(channel) = project.channels.get(state.selected_channel) {
        let label = format!("New group with {}…", channel.name);
        if ui.button(label).clicked() {
            state.open_dialog(Dialog::channel_group(state.selected_channel, ""));
            ui.close();
        }
    }
    let shown = state.rack_group.clone().filter(|g| !g.is_empty());
    crate::widgets::enabled_ui(ui, shown.is_some(), |ui| {
        if ui.button("Rename this group…").clicked() {
            if let Some(group) = &shown {
                state.open_dialog(Dialog::rename_group(group));
            }
            ui.close();
        }
        let remove = RichText::new("Remove this group").color(theme::DANGER);
        if ui.button(remove).clicked() {
            if let Some(group) = &shown {
                run(actions, "channel.rename_group", [group.clone()]);
                state.rack_group = None;
            }
            ui.close();
        }
    });
}

/// The group filter's choices in order: All, each group alphabetically,
/// then No group.
fn group_choices(project: &Project) -> Vec<Option<String>> {
    let mut groups: Vec<&str> = project
        .channels
        .iter()
        .map(|c| c.group.as_str())
        .filter(|g| !g.is_empty())
        .collect();
    groups.sort_unstable();
    groups.dedup();
    std::iter::once(None)
        .chain(groups.into_iter().map(|g| Some(g.to_owned())))
        .chain(std::iter::once(Some(String::new())))
        .collect()
}

/// The group filter after `current`, or before it, wrapping at the ends.
fn step_group(project: &Project, current: Option<&String>, forward: bool) -> Option<String> {
    let choices = group_choices(project);
    let n = choices.len();
    let at = choices
        .iter()
        .position(|c| c.as_ref() == current)
        .unwrap_or(0);
    let next = if forward {
        (at + 1) % n
    } else {
        (at + n - 1) % n
    };
    choices.into_iter().nth(next).flatten()
}

/// The step graph editor: one bar per step of the selected channel, showing
/// the property of the note that starts on that step. Dragging a bar sets it;
/// one drag is one undo step.
fn graph_editor(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    actions: &mut Vec<Action>,
) {
    use crate::piano_roll::LaneMode;
    let Some(mode) = state.graph else {
        return;
    };
    let channel = state.selected_channel;
    let Some(id) = project.channels.get(channel).map(|c| c.id) else {
        ui.label(RichText::new("Select a channel").color(theme::TEXT_DIM));
        return;
    };
    let pattern = project.current_pattern();
    let steps = (pattern.length_ticks() / TICKS_PER_STEP).max(1);
    let beat_steps = (pattern.time_signature.beat_ticks() / TICKS_PER_STEP).max(1);
    ui.horizontal(|ui| {
        ui.label(&project.channels[channel].name);
        for m in LaneMode::ALL {
            if ui.selectable_label(mode == m, m.label()).clicked() {
                state.graph = Some(m);
            }
        }
    });
    let size = egui::vec2(ui.available_width().max(120.0), 70.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    state.graph_view = Some((rect, steps));
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, theme::PANEL_DARK);
    let width = rect.width() / steps as f32;
    let note_at = |step: u32| {
        pattern
            .notes_of(id)
            .filter(|n| n.start == step * TICKS_PER_STEP)
            .max_by_key(|n| n.key)
            .copied()
    };
    let step_at = |x: f32| ((x - rect.left()) / width).floor().max(0.0) as u32;
    let unit = |y: f32| ((rect.bottom() - 3.0 - y) / (rect.height() - 6.0)).clamp(0.0, 1.0);
    if response.drag_started() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            state.graph_drag = note_at(step_at(pos.x).min(steps - 1));
            state.begin_gesture();
        }
    }
    let mut shown = None;
    if let (Some(note), Some(pos)) = (state.graph_drag, response.interact_pointer_pos()) {
        if response.dragged() || response.clicked() {
            let t = unit(pos.y);
            shown = Some((note.start, t));
            if let Some(invocation) = mode.invocation(channel, &note, t) {
                actions.push(Action::Coalesced(invocation, state.gesture()));
            }
        }
    }
    if response.drag_stopped() || (!response.dragged() && !response.clicked()) {
        state.graph_drag = None;
    }
    let color = crate::channel_color(project, channel);
    let base = if mode.centred() {
        rect.center().y
    } else {
        rect.bottom() - 3.0
    };
    for step in 0..steps {
        let x0 = rect.left() + width * step as f32;
        if step % beat_steps == 0 {
            painter.line_segment(
                [egui::pos2(x0, rect.top()), egui::pos2(x0, rect.bottom())],
                egui::Stroke::new(1.0, theme::GRID_BEAT),
            );
        }
        let Some(note) = note_at(step) else {
            continue;
        };
        let t = match shown {
            Some((start, t)) if start == note.start => t,
            _ => mode.value(&note),
        };
        let tip = rect.bottom() - 3.0 - t * (rect.height() - 6.0);
        let bar = egui::Rect::from_two_pos(
            egui::pos2(x0 + 2.0, base),
            egui::pos2(x0 + (width - 2.0).max(3.0), tip),
        );
        painter.rect_filled(bar, 1.0, color);
    }
}

/// The name of the mixer track a channel plays through, for tooltips.
fn route_name(project: &Project, track: usize) -> String {
    match project.mixer.get(track) {
        Some(_) if track == 0 => "Master".to_owned(),
        Some(t) => format!("{track} ({})", t.name),
        None => track.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playing_step_flashes_and_fades_over_its_duration() {
        let start = step_flash(0.0);
        let middle = step_flash(0.5);
        let end = step_flash(1.0);
        assert!(start > middle && middle > end, "{start} {middle} {end}");
        // It still reads brighter than a lit step at its end.
        assert!(end > 0.1);
        assert_eq!(step_flash(2.0), end);
    }

    #[test]
    fn channels_added_while_a_group_is_shown_join_it() {
        let mut session = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        session.run_line("channel.add_synth").unwrap();
        let mut state = UiState {
            rack_group: Some("Drums".to_owned()),
            ..UiState::default()
        };
        let mut added = vec![Action::Run(Invocation::bare("channel.add_synth"))];
        join_shown_group(&state, session.project(), &mut added);
        let Action::Run(batch) = &added[0] else {
            panic!("{added:?}");
        };
        session.run(batch).unwrap();
        assert_eq!(session.project().channels[1].group, "Drums");
        assert_eq!(session.project().channels[0].group, "");
        // With every channel shown, nothing changes.
        state.rack_group = None;
        let mut added = vec![Action::Run(Invocation::bare("channel.add_synth"))];
        join_shown_group(&state, session.project(), &mut added);
        assert_eq!(added, [Action::Run(Invocation::bare("channel.add_synth"))]);
    }

    #[test]
    fn the_menu_copies_one_channels_steps_onto_another() {
        use crate::ClipboardKey;
        let mut session = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        session.run_line("channel.add_synth").unwrap();
        session.run_line("channel.add_synth").unwrap();
        let n = session.project().channels.len();
        for line in [
            format!("step.toggle {} 1", n - 1),
            format!("step.toggle {} 5", n - 1),
            format!("step.toggle {n} 3"),
        ] {
            session.run_line(&line).unwrap();
        }
        let ctx = egui::Context::default();
        let mut state = UiState::default();
        let mut actions = Vec::new();
        let starts = |s: &musiccraft_commands::Session, i: usize| -> Vec<u32> {
            let p = s.project();
            p.current_pattern()
                .notes_of(p.channels[i].id)
                .map(|n| n.start)
                .collect()
        };
        let project = session.project().clone();
        let channel = project.channels.len() - 2;
        step_clipboard(
            &ctx,
            &mut state,
            &project,
            (channel, ClipboardKey::Copy),
            &mut actions,
        );
        assert!(actions.is_empty() && state.rack_clipboard.is_some());
        step_clipboard(
            &ctx,
            &mut state,
            &project,
            (channel + 1, ClipboardKey::Paste),
            &mut actions,
        );
        for action in actions.drain(..) {
            if let Action::Run(i) = action {
                session.run(&i).unwrap();
            }
        }
        assert_eq!(starts(&session, channel).len(), 2);
        assert_eq!(starts(&session, channel + 1), starts(&session, channel));
        let project = session.project().clone();
        step_clipboard(
            &ctx,
            &mut state,
            &project,
            (channel, ClipboardKey::Cut),
            &mut actions,
        );
        for action in actions.drain(..) {
            if let Action::Run(i) = action {
                session.run(&i).unwrap();
            }
        }
        assert!(starts(&session, channel).is_empty());
    }
}
