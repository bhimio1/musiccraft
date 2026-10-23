//! Windows for mixer effects. The editor itself (preset bar, graph, knobs) is
//! `musiccraft_editors::effect`, shared with the exported plugins; this module
//! places it in a window and turns its edits into commands. Hosted plugins
//! show their parameter panel instead.

use egui::vec2;
use musiccraft_commands::{BATCH, Invocation};
use musiccraft_editors::effect::{self, Edit, EffectView, pretty};
use musiccraft_project::EffectSlot;
use musiccraft_project::params::{EffectKind, ParamSpec};

use crate::hint::Hint;
use crate::{Action, Frame, UiState, theme, widgets};
use egui::RichText;

/// Opens (or closes) the window of effect `slot` on `track`.
pub(crate) fn toggle(state: &mut UiState, track: usize, slot: usize) {
    if let Some(i) = state
        .effect_windows
        .iter()
        .position(|w| *w == (track, slot))
    {
        state.effect_windows.remove(i);
    } else {
        state.effect_windows.push((track, slot));
    }
}

/// Opens the window of every effect on `track` (those already open stay).
pub(crate) fn open_all(state: &mut UiState, project: &musiccraft_project::Project, track: usize) {
    let count = project.mixer.get(track).map_or(0, |t| t.effects.len());
    for slot in 0..count {
        if !state.effect_windows.contains(&(track, slot)) {
            state.effect_windows.push((track, slot));
        }
    }
}

fn set_param(track: usize, slot: usize, name: &str, value: f32) -> Invocation {
    Invocation::new(
        "mixer.set_effect_param",
        [
            track.to_string(),
            (slot + 1).to_string(),
            name.to_owned(),
            format!("{value}"),
        ],
    )
}

/// One undo step setting several parameters.
fn batch(invocations: Vec<Invocation>) -> Option<Invocation> {
    match invocations.len() {
        0 => None,
        1 => invocations.into_iter().next(),
        _ => Some(Invocation::new(
            BATCH,
            invocations.iter().map(ToString::to_string),
        )),
    }
}

/// What the editor shows for a project slot.
fn view(effect: &EffectSlot) -> EffectView {
    EffectView {
        kind: effect.kind,
        enabled: effect.enabled,
        values: effect.values,
        file: effect.file.clone(),
    }
}

pub(crate) fn show_all(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    // Windows whose slot is gone close.
    state.effect_windows.retain(|(t, s)| {
        project
            .mixer
            .get(*t)
            .is_some_and(|track| *s < track.effects.len())
    });
    let open = state.effect_windows.clone();
    for (index, (track, slot)) in open.into_iter().enumerate() {
        let Some(effect) = project.mixer.get(track).and_then(|t| t.effects.get(slot)) else {
            continue;
        };
        let track_name = project.mixer.get(track).map_or("", |t| t.name.as_str());
        let title = format!("{} — {track_name}", effect_title(effect));
        let mut keep = true;
        let id = egui::Id::new(("effect-window", track, slot));
        let area = state.workspace;
        let default = egui::Rect::from_min_size(
            area.left_top() + vec2(60.0 + 24.0 * index as f32, 60.0 + 24.0 * index as f32),
            vec2(0.0, 0.0),
        );
        crate::chrome::window(ctx, id, &title, area, default, false).show(ctx, |ui| {
            let mut header_edits = Vec::new();
            let folded = crate::chrome::Header::new(id, &title).fixed_size().show(
                ui,
                &mut keep,
                &mut (&mut *state, &mut *actions, &mut header_edits),
                |ui, (state, actions, _)| {
                    if let Some(mixer_track) = project.mixer.get(track) {
                        crate::mixer::slot_menu(
                            ui,
                            state,
                            mixer_track,
                            &track.to_string(),
                            slot,
                            false,
                            actions,
                        );
                    }
                },
                |ui, (_, _, edits)| {
                    if effect.plugin.is_none() {
                        let editor = egui::Id::new(("effect-editor", track, slot));
                        effect::preset_picker(ui, editor, &view(effect), edits);
                    }
                },
            );
            apply(
                state,
                KnobTarget::Slot(track, slot),
                effect,
                header_edits,
                actions,
            );
            if !folded {
                strip(ui, state, track, slot, effect, actions);
                ui.add_space(4.0);
                contents(ui, state, frame, track, slot, effect, actions);
            }
        });
        if !keep {
            toggle(state, track, slot);
        }
    }
}

/// The slim strip under the header: the On light (bypass) and the Mix knob,
/// flush right; the same controls as the mixer slot's.
fn strip(
    ui: &mut egui::Ui,
    state: &mut UiState,
    track: usize,
    slot: usize,
    effect: &EffectSlot,
    actions: &mut Vec<Action>,
) {
    let (number, slot_no) = (track.to_string(), (slot + 1).to_string());
    egui::Frame::new()
        .fill(theme::PANEL_DARK)
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - STRIP_CONTROLS).max(0.0));
                let word = if effect.enabled { "on" } else { "bypassed" };
                let light = widgets::dot(ui, effect.enabled, theme::ACCENT)
                    .hint(format!("Effect {word} · click to bypass or restore"));
                if light.clicked() {
                    actions.push(Action::Run(Invocation::new(
                        "mixer.toggle_effect",
                        [number.clone(), slot_no.clone()],
                    )));
                }
                ui.label(RichText::new("On").small().color(theme::TEXT_DIM));
                ui.add_space(8.0);
                let mix = widgets::knob(ui, effect.mix, 0.0, 1.0, 1.0, false);
                if mix.started {
                    state.begin_gesture();
                }
                if let Some(value) = mix.changed {
                    actions.push(Action::Coalesced(
                        Invocation::new(
                            "mixer.set_effect_mix",
                            [number.clone(), slot_no.clone(), format!("{value:.3}")],
                        ),
                        state.gesture(),
                    ));
                }
                mix.response.hint(format!(
                    "Mix {:.0}% · how much of the effect is heard",
                    effect.mix * 100.0
                ));
                ui.label(RichText::new("Mix").small().color(theme::TEXT_DIM));
            });
        });
}

/// Width of the effect strip's right group: light, captions and knob.
const STRIP_CONTROLS: f32 = 110.0;

fn effect_title(effect: &EffectSlot) -> String {
    match &effect.plugin {
        Some(plugin) => plugin.name.clone(),
        None => crate::browser::effect_title(effect.kind).to_owned(),
    }
}

fn contents(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    track: usize,
    slot: usize,
    effect: &EffectSlot,
    actions: &mut Vec<Action>,
) {
    if effect.kind.sidechain_index().is_some() {
        sidechain_picker(ui, frame.project, (track, slot), effect, actions);
    }
    if let Some(plugin) = &effect.plugin {
        let target = crate::plugins::Target::Effect(track, slot + 1);
        crate::plugins::panel(ui, state, frame.plugins, plugin, target, actions);
        return;
    }
    let id = egui::Id::new(("effect-editor", track, slot));
    let view = view(effect);
    let target = KnobTarget::Slot(track, slot);
    let mut edits = Vec::new();
    let sample_rate = match frame.settings_info.running_rate {
        0 => 48_000.0,
        rate => rate as f32,
    };
    if effect.kind == EffectKind::Convolution {
        impulse_picker(ui, state, frame.project, (track, slot), effect, actions);
    }
    if matches!(effect.kind, EffectKind::PitchCorrect | EffectKind::Tuner) {
        let detected = frame
            .engine
            .pitches
            .iter()
            .find(|(t, s, _)| *t == track && *s == slot)
            .map(|(_, _, hz)| *hz);
        effect::tuner(ui, &view, detected);
    } else if effect.kind == EffectKind::Rack {
        crate::rack_view::show(ui, state, track, slot, effect, actions);
    } else {
        state.effect_graph = Some(effect::graph(ui, id, &view, sample_rate, &mut edits));
    }
    ui.add_space(4.0);
    apply(state, target, effect, edits, actions);
    knob_row(ui, state, target, effect, actions);
}

/// The effect's sidechain input: off, or one of the tracks routed to this
/// one as a sidechain (FL Studio style: route first, then pick).
fn sidechain_picker(
    ui: &mut egui::Ui,
    project: &musiccraft_project::Project,
    (track, slot): (usize, usize),
    effect: &EffectSlot,
    actions: &mut Vec<Action>,
) {
    let Some(index) = effect.kind.sidechain_index() else {
        return;
    };
    let current = effect.values.get(index).copied().unwrap_or(0.0).round() as usize;
    let name = |t: usize| {
        project
            .mixer
            .get(t)
            .map_or_else(|| format!("{t}"), |m| format!("{t} {}", m.name))
    };
    let routed: Vec<usize> = (1..project.mixer.len())
        .filter(|&t| project.mixer[t].sidechain_to.contains(&track))
        .collect();
    let selected = match current {
        0 => "Off".to_owned(),
        t if routed.contains(&t) => name(t),
        t => format!("{} (not routed)", name(t)),
    };
    let set = |value: usize| {
        Action::Run(Invocation::new(
            "mixer.set_effect_param",
            [
                track.to_string(),
                (slot + 1).to_string(),
                "sidechain".to_owned(),
                value.to_string(),
            ],
        ))
    };
    ui.horizontal(|ui| {
        ui.label("Sidechain");
        egui::ComboBox::from_id_salt(("sidechain", track, slot))
            .selected_text(selected)
            .show_ui(ui, |ui| {
                if ui.selectable_label(current == 0, "Off").clicked() {
                    actions.push(set(0));
                }
                for &t in &routed {
                    if ui.selectable_label(current == t, name(t)).clicked() {
                        actions.push(set(t));
                    }
                }
                if routed.is_empty() {
                    ui.label(
                        RichText::new("No inserts sidechained here yet")
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                }
            })
            .response
            .hint(
                "The insert whose sound drives this effect. To add one, select it in the mixer, \
                 right-click this track's arrow and choose Sidechain to this track",
            );
    });
}

/// The convolution reverb's impulse response: generated rooms or a file.
fn impulse_picker(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &musiccraft_project::Project,
    (track, slot): (usize, usize),
    effect: &EffectSlot,
    actions: &mut Vec<Action>,
) {
    let current = effect.file.clone().unwrap_or_else(|| "ir:hall".to_owned());
    let label = current.strip_prefix("ir:").map_or_else(
        || {
            std::path::Path::new(&current)
                .file_name()
                .map_or_else(|| current.clone(), |n| n.to_string_lossy().into_owned())
        },
        pretty,
    );
    ui.horizontal(|ui| {
        ui.label("Impulse");
        egui::ComboBox::from_id_salt(("impulse", track, slot))
            .selected_text(label)
            .show_ui(ui, |ui| {
                for name in musiccraft_project::params::IMPULSE_RESPONSES {
                    let key = format!("ir:{name}");
                    if ui.selectable_label(current == key, pretty(name)).clicked() {
                        actions.push(Action::Run(Invocation::new(
                            "mixer.set_effect_file",
                            [track.to_string(), (slot + 1).to_string(), key],
                        )));
                    }
                }
            })
            .response
            .hint("Generated rooms, from small to huge");
        if ui
            .button("Load file…")
            .hint("Use an audio file (a recorded impulse response) as the room")
            .clicked()
        {
            let start = effect.file.clone().filter(|f| !f.starts_with("ir:"));
            state.pick_file(crate::dialogs::FilePick::sample(
                crate::dialogs::Pick::SetEffectFile(track, slot),
                &start.unwrap_or_default(),
                project,
            ));
        }
    });
}

/// Which effect a row of knobs edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KnobTarget {
    /// Effect `slot` (0-based) of mixer `track`.
    Slot(usize, usize),
    /// Effect `n` of `branch` (both 0-based) of the rack in `slot` of `track`.
    Rack(usize, usize, usize, usize),
}

impl KnobTarget {
    fn set(self, name: &str, value: f32) -> Invocation {
        match self {
            Self::Slot(track, slot) => set_param(track, slot, name, value),
            Self::Rack(track, slot, branch, n) => Invocation::new(
                "mixer.rack_param",
                [
                    track.to_string(),
                    (slot + 1).to_string(),
                    (branch + 1).to_string(),
                    (n + 1).to_string(),
                    name.to_owned(),
                    format!("{value}"),
                ],
            ),
        }
    }

    /// Bypasses the effect when `on`, turns it on otherwise.
    fn toggle(self, on: bool) -> Invocation {
        match self {
            Self::Slot(track, slot) => Invocation::new(
                "mixer.toggle_effect",
                [track.to_string(), (slot + 1).to_string()],
            ),
            Self::Rack(track, slot, branch, n) => Invocation::new(
                "mixer.rack_enable",
                [
                    track.to_string(),
                    (slot + 1).to_string(),
                    (branch + 1).to_string(),
                    (n + 1).to_string(),
                    if on { "off" } else { "on" }.to_owned(),
                ],
            ),
        }
    }
}

/// Turns the editor's edits into commands: drags coalesce into one undo step
/// per gesture, clicks are steps of their own.
fn apply(
    state: &mut UiState,
    target: KnobTarget,
    effect: &EffectSlot,
    edits: Vec<Edit>,
    actions: &mut Vec<Action>,
) {
    let invocations = |changes: Vec<(&str, f32)>| {
        batch(
            changes
                .into_iter()
                .map(|(name, value)| target.set(name, value))
                .collect(),
        )
    };
    for edit in edits {
        match edit {
            Edit::Begin => {
                state.begin_gesture();
            }
            Edit::Drag(changes) => {
                if let Some(invocation) = invocations(changes) {
                    actions.push(Action::Coalesced(invocation, state.gesture()));
                }
            }
            Edit::Set(changes) => {
                if let Some(invocation) = invocations(changes) {
                    actions.push(Action::Run(invocation));
                }
            }
            Edit::ToggleBypass => actions.push(Action::Run(target.toggle(effect.enabled))),
        }
    }
}

/// An effect's knobs (the ones its graph does not edit). Inside a rack the
/// routing knobs are left out.
pub(crate) fn knob_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    target: KnobTarget,
    effect: &EffectSlot,
    actions: &mut Vec<Action>,
) {
    let in_rack = matches!(target, KnobTarget::Rack(..));
    let kind = effect.kind;
    // The sidechain has its own picker above the knobs.
    let hidden =
        |p: usize| in_rack && effect::is_routing(kind, p) || Some(p) == kind.sidechain_index();
    let mut edits = Vec::new();
    effect::knob_row(
        ui,
        &view(effect),
        &hidden,
        &mut edits,
        &mut |response: &egui::Response, spec: &ParamSpec| {
            if let KnobTarget::Slot(track, slot) = target {
                crate::automation::menu(
                    response,
                    &format!("mixer:{track}:{}:{}", slot + 1, spec.name),
                    actions,
                );
            }
        },
    );
    apply(state, target, effect, edits, actions);
}
