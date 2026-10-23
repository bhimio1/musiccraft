//! Automation editing: the curve editor window, and the "Create automation
//! clip" menu attached to knobs and faders.

use crate::hint::Hint;
use egui::{Align2, FontId, Pos2, Rect, RichText, Sense, Stroke, pos2, vec2};
use musiccraft_commands::{BATCH, Invocation};
use musiccraft_editors::widgets;
use musiccraft_project::params::TICKS_PER_STEP;
use musiccraft_project::{
    Automation, AutomationPoint, AutomationTarget, Meter, PLAYLIST_TRACKS, PlayMode, Project,
};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{Action, Frame, UiState, curve, theme};

/// The first playlist row with no clips (1-based), or the last row.
fn free_track(project: &Project) -> u32 {
    (0..PLAYLIST_TRACKS)
        .find(|row| !project.playlist.clips.iter().any(|c| c.track == *row))
        .unwrap_or(PLAYLIST_TRACKS - 1)
        + 1
}

/// One undo step that creates an automation for `target` and places a clip of
/// it on a free playlist track at the start of the song.
pub(crate) fn create_invocation(project: &Project, target: &str) -> Invocation {
    let number = project.automations.len() + 1;
    Invocation::new(
        BATCH,
        [
            Invocation::new("automation.create", [target.to_owned()]).to_string(),
            format!(
                "playlist.add_automation_clip {number} {} 0",
                free_track(project)
            ),
        ],
    )
}

/// The command syntax for `target` (`channel:2:volume`, `mixer:3:1:mix`), as
/// the control menus name their targets.
fn target_name(project: &Project, target: &AutomationTarget) -> Option<String> {
    let channel = |id| project.channel_position(id).map(|i| i + 1);
    Some(match target {
        AutomationTarget::ChannelVolume { channel: id } => {
            format!("channel:{}:volume", channel(*id)?)
        }
        AutomationTarget::ChannelPan { channel: id } => format!("channel:{}:pan", channel(*id)?),
        AutomationTarget::ChannelParam { channel: id, param } => {
            format!("channel:{}:{param}", channel(*id)?)
        }
        AutomationTarget::MixerVolume { track } => format!("mixer:{track}:volume"),
        AutomationTarget::MixerPan { track } => format!("mixer:{track}:pan"),
        AutomationTarget::EffectParam { track, slot, param } => {
            format!("mixer:{track}:{}:{param}", slot + 1)
        }
    })
}

/// What drives one control.
#[derive(Clone, Default)]
struct Source {
    /// Text the hint appends, such as " · automated (clip 'Master fade')".
    hint: String,
    /// The automation for it (1-based), if any.
    automation: Option<usize>,
    /// The controllers driving it (1-based).
    controllers: Vec<usize>,
}

/// What every control menu needs to know this frame: the driven controls by
/// target name, and the copied control position.
#[derive(Clone, Default)]
struct Published {
    driven: Arc<HashMap<String, Source>>,
    copied: Option<f32>,
    /// The open menu's target and its default position.
    default: Option<(String, f32)>,
}

fn published_id() -> egui::Id {
    egui::Id::new("control-menus")
}

/// The target whose control menu was open last frame.
fn open_menu_id() -> egui::Id {
    egui::Id::new("control-menu-open")
}

/// Records which controls automation clips and controllers drive, so their
/// menus can mark them (style guide, controls M), and the copied value
/// their Paste item uses. Call once per frame.
pub(crate) fn publish(ctx: &egui::Context, project: &Project, copied: Option<f32>) {
    let mut driven: HashMap<String, Source> = HashMap::new();
    for (i, automation) in project.automations.iter().enumerate() {
        if let Some(name) = target_name(project, &automation.target) {
            let source = driven.entry(name).or_default();
            source.automation = Some(i + 1);
            source.hint = format!(" · automated (clip '{}')", automation.name);
        }
    }
    for (i, controller) in project.controllers.iter().enumerate() {
        if let Some(name) = target_name(project, &controller.target) {
            let source = driven.entry(name).or_default();
            source.controllers.push(i + 1);
            if source.hint.is_empty() {
                source.hint = format!(" · driven by {}", controller.name);
            }
        }
    }
    // Only the open menu's default is looked up, a frame after it opens.
    let default = ctx
        .data_mut(|d| d.remove_temp::<String>(open_menu_id()))
        .and_then(|target| {
            let info = musiccraft_commands::control_info(project, &target)?;
            Some((target, info.default))
        });
    let published = Published {
        driven: Arc::new(driven),
        copied,
        default,
    };
    ctx.data_mut(|d| d.insert_temp(published_id(), published));
}

/// Draws the status dot on a driven control and adds its source to the hint.
fn mark(response: &egui::Response, source: Option<&Source>) {
    if source.is_some() {
        let painter = response
            .ctx
            .layer_painter(response.layer_id)
            .with_clip_rect(response.interact_rect.expand(6.0));
        widgets::status_dot(&painter, widgets::status_anchor(response));
    }
    let hint = source.map(|s| s.hint.clone());
    musiccraft_editors::hint::set_suffix(&response.ctx, response.id, hint);
}

/// The right-click menu of a control that may be a `ComboBox`. egui gives a
/// context menu and a combo's list the same popup id, and the context menu
/// closes that popup on every left click, so the list could never open; the
/// menu gets an id of its own.
pub(crate) fn context_menu(response: &egui::Response) -> egui::Popup<'_> {
    egui::Popup::context_menu(response).id(response.id.with("control-menu"))
}

/// A dim heading row inside a menu.
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(2.0);
    ui.label(
        RichText::new(text)
            .size(theme::FONT_LABEL)
            .color(theme::TEXT_DIM),
    );
}

fn last_moved_id() -> egui::Id {
    egui::Id::new("last-moved-control")
}

/// Remembers `target` as the last moved control.
fn moved(ctx: &egui::Context, target: &str) {
    ctx.data_mut(|d| d.insert_temp(last_moved_id(), target.to_owned()));
}

/// The last control the user dragged, typed into or reset (`docs/spec/
/// controls.md`), by target name.
pub(crate) fn last_moved(ctx: &egui::Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<String>(last_moved_id()))
}

/// Adds the control menu to `target` (command syntax such as
/// `mixer:0:volume`), and marks the control when something drives it. The UI
/// resolves `ui.*` requests with the current project.
pub(crate) fn menu(response: &egui::Response, target: &str, actions: &mut Vec<Action>) {
    let published = response
        .ctx
        .data(|d| d.get_temp::<Published>(published_id()))
        .unwrap_or_default();
    let source = published.driven.get(target);
    mark(response, source);
    if response.dragged() || response.double_clicked() || response.middle_clicked() {
        moved(&response.ctx, target);
    }
    context_menu(response).show(|ui| {
        ui.data_mut(|d| d.insert_temp(open_menu_id(), target.to_owned()));
        let at = response.rect.left_bottom();
        items(ui, target, &published, Some(at), actions);
    });
}

/// The last moved control's menu items (Tools → Last moved control), or a
/// dim line when no control has moved yet.
pub(crate) fn last_moved_menu(ui: &mut egui::Ui, project: &Project, actions: &mut Vec<Action>) {
    let Some(target) = last_moved(ui.ctx()) else {
        heading(ui, "Move a knob or fader first");
        return;
    };
    if let Some(info) = musiccraft_commands::control_info(project, &target) {
        heading(ui, &info.label);
    }
    let published = ui
        .ctx()
        .data(|d| d.get_temp::<Published>(published_id()))
        .unwrap_or_default();
    items(ui, &target, &published, None, actions);
}

/// The control menu's items for `target`. "Enter exact value…" shows only
/// with a place for its box.
fn items(
    ui: &mut egui::Ui,
    target: &str,
    published: &Published,
    type_in_at: Option<Pos2>,
    actions: &mut Vec<Action>,
) {
    let source = published.driven.get(target);
    let run = |actions: &mut Vec<Action>, name: &str, args: Vec<String>| {
        actions.push(Action::Run(Invocation::new(name, args)));
    };
    let target_arg = || vec![target.to_owned()];
    if ui.button("Create automation clip").clicked() {
        run(actions, "ui.automate", target_arg());
        ui.close();
    }
    let automation = source.and_then(|s| s.automation);
    if crate::widgets::add_enabled(
        ui,
        automation.is_some(),
        egui::Button::new("Open its automation…"),
    )
    .clicked()
        && let Some(number) = automation
    {
        run(actions, "ui.open_automation", vec![number.to_string()]);
        ui.close();
    }
    ui.separator();
    heading(ui, "Control from");
    ui.menu_button("Add controller", |ui| {
        for (label, kind) in [
            ("LFO", "lfo"),
            ("Macro knob", "macro"),
            ("Envelope (first channel's notes)", "envelope"),
            ("Keyboard (first channel's key)", "keyboard"),
            ("Peak (insert 1's level)", "peak"),
            ("Formula", "formula"),
        ] {
            if ui.button(label).clicked() {
                run(
                    actions,
                    "controller.add",
                    vec![kind.to_owned(), target.to_owned()],
                );
                ui.close();
            }
        }
    });
    let controllers = source.map(|s| s.controllers.as_slice()).unwrap_or_default();
    if !controllers.is_empty() && ui.button("Remove controller").clicked() {
        // Highest first, so the numbers stay valid.
        let removals = controllers
            .iter()
            .rev()
            .map(|n| Invocation::new("controller.remove", [n.to_string()]).to_string());
        run(actions, BATCH, removals.collect());
        ui.close();
    }
    ui.separator();
    heading(ui, "Value");
    if ui.button("Copy").clicked() {
        run(actions, "ui.copy_control", target_arg());
        ui.close();
    }
    if crate::widgets::add_enabled(ui, published.copied.is_some(), egui::Button::new("Paste"))
        .clicked()
        && let Some(position) = published.copied
    {
        run(
            actions,
            "control.set",
            vec![target.to_owned(), format!("{position}")],
        );
        moved(ui.ctx(), target);
        ui.close();
    }
    if let Some(at) = type_in_at
        && ui.button("Enter exact value…").clicked()
    {
        run(
            actions,
            "ui.type_in",
            vec![target.to_owned(), format!("{}", at.x), format!("{}", at.y)],
        );
        moved(ui.ctx(), target);
        ui.close();
    }
    let default = published
        .default
        .as_ref()
        .filter(|(t, _)| t == target)
        .map(|(_, position)| format!("{:.0}%", position * 100.0))
        .unwrap_or_default();
    let reset = egui::Button::new("Reset to default")
        .right_text(RichText::new(default).color(theme::TEXT_DIM));
    if ui.add(reset).clicked() {
        run(actions, "control.reset", target_arg());
        moved(ui.ctx(), target);
        ui.close();
    }
}

/// Height of the graph when the window is not maximized.
const GRAPH_HEIGHT: f32 = 200.0;
/// Height of the bar ruler over the graph.
const RULER: f32 = 16.0;
/// Space around the curve inside the graph, so edge points stay whole.
const PAD: f32 = 8.0;

/// The automation editor's view and toggles.
#[derive(Debug, Clone, Default)]
pub(crate) struct EditorState {
    pub(crate) edit: curve::Edit,
    /// First tick shown.
    first: f32,
    /// Ticks shown; `None` shows the whole curve.
    span: Option<f32>,
    pub(crate) options: curve::Options,
    /// A copied curve.
    clipboard: Option<Vec<AutomationPoint>>,
    /// Open the newest automation once there are more than this many.
    pub(crate) pending: Option<usize>,
}

impl EditorState {
    /// Keeps the editor's options and copied curve; the view, the gesture and
    /// a waiting "open the new automation" belong to the old project.
    pub(crate) fn forget_project(&mut self) {
        let old = std::mem::take(self);
        *self = Self {
            options: old.options,
            clipboard: old.clipboard,
            ..Self::default()
        };
    }
}

/// Opens an automation created by "Create automation clip" once it exists.
fn open_pending(state: &mut UiState, project: &Project) {
    if let Some(count) = state.automation_ui.pending
        && project.automations.len() > count
        && let Some(last) = project.automations.last()
    {
        state.automation_editor = Some(last.id);
        state.automation_ui.pending = None;
        state.arrange = Some(crate::Arrange::Raise("automation-editor"));
    }
}

pub fn show(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    open_pending(state, project);
    let Some(id) = state.automation_editor else {
        return;
    };
    let Some(index) = project.automation_position(id) else {
        state.automation_editor = None;
        return;
    };
    let automation = &project.automations[index];
    state.automation_ui.edit.follow(id);
    let mut open = true;
    let area = state.workspace;
    let window_id = egui::Id::new("automation-editor");
    let title = format!("Automation — {}", automation.name);
    let default = egui::Rect::from_min_size(
        area.left_bottom() + vec2(60.0, -GRAPH_HEIGHT - 190.0),
        vec2((area.width() - 120.0).max(480.0), 0.0),
    );
    crate::chrome::window(ctx, window_id, "Automation", area, default, true).show(ctx, |ui| {
        let folded = crate::chrome::Header::new(window_id, &title).show(
            ui,
            &mut open,
            &mut (&mut *state, &mut *actions),
            |ui, (state, actions)| window_menu(ui, state, (index, automation), actions),
            |ui, (state, _)| switcher(ui, state, project, index),
        );
        if folded {
            return;
        }
        settings_band(ui, state, (index, automation), actions);
        let maximized = crate::chrome::maximized(ui.ctx(), window_id);
        graph(ui, state, frame, (index, automation), maximized, actions);
        footer(ui, state);
        if crate::chrome::focused(ui.ctx(), window_id) {
            keys(ui, state, (index, automation), actions);
        }
    });
    if !open {
        state.automation_editor = None;
    }
}

/// The window menu: curve clipboard and curve tools, each one undo step.
fn window_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    (index, automation): (usize, &Automation),
    actions: &mut Vec<Action>,
) {
    let number = index + 1;
    let points = &automation.points;
    heading(ui, "Curve");
    if ui.button("Copy curve").clicked() {
        state.automation_ui.clipboard = Some(points.clone());
        ui.close();
    }
    let pasted = state.automation_ui.clipboard.clone();
    if crate::widgets::add_enabled(ui, pasted.is_some(), egui::Button::new("Paste curve")).clicked()
        && let Some(pasted) = pasted
    {
        actions.push(curve::set_points(number, &pasted));
        ui.close();
    }
    ui.separator();
    heading(ui, "Tools");
    if ui.button("Flip vertically").clicked() {
        let flipped: Vec<AutomationPoint> = points
            .iter()
            .map(|p| AutomationPoint {
                value: 1.0 - p.value,
                ..*p
            })
            .collect();
        actions.push(curve::set_points(number, &flipped));
        ui.close();
    }
    let (lo, hi) = points.iter().fold((1.0_f32, 0.0_f32), |(lo, hi), p| {
        (lo.min(p.value), hi.max(p.value))
    });
    if crate::widgets::add_enabled(ui, hi - lo > 1e-4, egui::Button::new("Normalize"))
        .hint("Stretch the curve so its highest point reaches the top and its lowest the bottom")
        .clicked()
    {
        let stretched: Vec<AutomationPoint> = points
            .iter()
            .map(|p| AutomationPoint {
                value: (p.value - lo) / (hi - lo),
                ..*p
            })
            .collect();
        actions.push(curve::set_points(number, &stretched));
        ui.close();
    }
    ui.separator();
    heading(ui, "Points");
    if ui.button("Select all").clicked() {
        curve::select_all(automation, &mut state.automation_ui.edit);
        ui.close();
    }
    let selected = !state.automation_ui.edit.selection.is_empty();
    if crate::widgets::add_enabled(ui, selected, egui::Button::new("Delete selected points"))
        .clicked()
    {
        curve::delete_selection((number, automation), &mut state.automation_ui.edit, actions);
        ui.close();
    }
}

/// The chevron beside the title: lists every automation to switch to; the
/// wheel steps through them.
fn switcher(ui: &mut egui::Ui, state: &mut UiState, project: &Project, index: usize) {
    let popup = egui::Id::new("automation-switch");
    let open = egui::Popup::is_id_open(ui.ctx(), popup);
    let response = crate::icons::button(
        ui,
        crate::icons::Icon::ChevronDown,
        open,
        theme::ACCENT,
        "Switch automation · click to list them, wheel to step",
    );
    let notches = if response.hovered() {
        crate::scrollbars::wheel_notches(ui)
    } else {
        0.0
    };
    let count = project.automations.len();
    if notches != 0.0 && count > 0 {
        let next = if notches < 0.0 {
            (index + 1) % count
        } else {
            (index + count - 1) % count
        };
        state.automation_editor = Some(project.automations[next].id);
    }
    let toggle = response.clicked().then_some(egui::SetOpenCommand::Toggle);
    egui::Popup::menu(&response)
        .id(popup)
        .open_memory(toggle)
        .show(|ui| {
            for (i, automation) in project.automations.iter().enumerate() {
                if ui.radio(i == index, &automation.name).clicked() {
                    state.automation_editor = Some(automation.id);
                    ui.close();
                }
            }
        });
}

/// The On switch and the Low and High knobs.
fn settings_band(
    ui: &mut egui::Ui,
    state: &mut UiState,
    (index, automation): (usize, &Automation),
    actions: &mut Vec<Action>,
) {
    let number = (index + 1).to_string();
    egui::Frame::new()
        .fill(theme::PANEL_DARK)
        .corner_radius(3.0)
        .inner_margin(vec2(8.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let mut on = automation.enabled;
                if widgets::led_switch(ui, &mut on, "On")
                    .hint("Switch this automation's clips on or off")
                    .changed()
                {
                    actions.push(Action::Run(Invocation::new(
                        "automation.set_enabled",
                        [number.clone(), if on { "on" } else { "off" }.to_owned()],
                    )));
                }
                ui.add_space(10.0);
                let (low, high) = (automation.low, automation.high);
                for (label, value, default, hint) in [
                    (
                        "Low",
                        low,
                        0.0,
                        "Low: what the curve's bottom plays as · double-click resets",
                    ),
                    (
                        "High",
                        high,
                        1.0,
                        "High: what the curve's top plays as · double-click resets",
                    ),
                ] {
                    let text = format!("{:.0}%", value * 100.0);
                    let out = widgets::labeled_knob(ui, label, value, default, false, &text);
                    let _ = out.response.hint(hint);
                    if out.started {
                        state.begin_gesture();
                    }
                    if let Some(v) = out.changed {
                        let (l, h) = if label == "Low" { (v, high) } else { (low, v) };
                        actions.push(Action::Coalesced(
                            Invocation::new(
                                "automation.set_range",
                                [number.clone(), format!("{l:.4}"), format!("{h:.4}")],
                            ),
                            state.gesture(),
                        ));
                    }
                }
            });
        });
}

/// Ticks the whole curve needs: its points rounded up to a bar, plus a bar
/// to drag into, and at least four bars.
fn whole(meter: &Meter, automation: &Automation) -> u32 {
    meter.round_up_to_bar(automation.length_ticks().max(meter.bar_start(4))) + meter.bar(0).len
}

/// The ruler, the graph with the curve, and the scroll bar under it.
fn graph(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    (index, automation): (usize, &Automation),
    maximized: bool,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    // Curve ticks count from the clip start: bars of the project signature.
    let meter = Meter::new(project.time_signature);
    let number = index + 1;
    let shown = curve::shown(automation, &state.automation_ui.edit);
    let total = whole(&meter, &shown) as f32;
    let ed = &mut state.automation_ui;
    let span = ed
        .span
        .unwrap_or(total)
        .clamp(TICKS_PER_STEP as f32 * 4.0, total);
    ed.first = ed.first.clamp(0.0, (total - span).max(0.0));
    let width = ui.available_width().max(240.0);
    let height = if maximized {
        (ui.available_height() - RULER - 2.0 * crate::scrollbars::BAR - 8.0).max(GRAPH_HEIGHT)
    } else {
        GRAPH_HEIGHT
    };
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, RULER + height), Sense::click_and_drag());
    let ruler = Rect::from_min_size(rect.min, vec2(width, RULER));
    let area = Rect::from_min_max(pos2(rect.left(), ruler.bottom()), rect.max);
    let inner = area.shrink2(vec2(PAD, PAD));
    let view = curve::View {
        rect: inner,
        first: ed.first,
        px_per_tick: inner.width() / span,
    };
    state.automation_view = Some(view);
    let painter = ui.painter_at(rect);
    painter.rect_filled(area, 3.0, theme::LCD_BG);
    painter.rect_filled(ruler, 0.0, theme::PANEL_DARK);
    let visible = (
        view.tick(area.left()).max(0.0) as u32,
        view.tick(area.right()) as u32 + 1,
    );
    for beat in meter.beats(visible.0, visible.1) {
        let x = view.x(beat.tick as f32);
        let color = if beat.downbeat {
            theme::GRID_BAR
        } else {
            theme::GRID_LINE
        };
        painter.line_segment(
            [pos2(x, area.top()), pos2(x, area.bottom())],
            Stroke::new(1.0, color),
        );
        if beat.downbeat {
            painter.text(
                pos2(x + 3.0, ruler.center().y),
                Align2::LEFT_CENTER,
                (beat.bar + 1).to_string(),
                FontId::monospace(theme::FONT_VALUE),
                theme::TEXT_DIM,
            );
        }
    }
    let line = if automation.enabled {
        theme::AUTOMATION
    } else {
        theme::AUTOMATION.gamma_multiply(0.5)
    };
    let look = curve::Look {
        line,
        fill: theme::AUTOMATION_FILL,
        points: true,
    };
    let curve_painter = ui.painter_at(area);
    curve::draw(&curve_painter, &view, &shown, &ed.edit, look);
    if let Some(tick) = playing_at(frame, automation.id) {
        let x = view.x(tick);
        if area.x_range().contains(x) {
            painter.line_segment(
                [pos2(x, ruler.top()), pos2(x, area.bottom())],
                Stroke::new(1.0, theme::PLAYHEAD),
            );
        }
    }
    let hover = response.hover_pos();
    let text = hover.and_then(|p| curve::describe(&view, &shown, &ed.edit, &meter, p));
    let response = response.hint(text.clone().unwrap_or_else(|| {
        "Automation: right-click adds a point · drag points and handles · right-click a point for its menu · Alt+click deletes · Ctrl+wheel zooms".to_owned()
    }));
    if let Some(pos) = hover.or_else(|| response.interact_pointer_pos()) {
        curve::readout(&painter, &ed.edit, text.as_deref(), pos);
    }
    let options = curve::Options {
        in_clip: false,
        ..ed.options
    };
    curve::interact(
        ui,
        &response,
        &view,
        (number, automation),
        &meter,
        options,
        &mut ed.edit,
        actions,
    );
    curve::menus(
        ui,
        &response,
        &view,
        (number, automation),
        &mut ed.edit,
        actions,
    );
    if response.hovered() {
        wheel(ui, ed, &view, total);
    }
    // The scroll bar along the bottom, with zoom grips.
    let (bar, _) = ui.allocate_exact_size(vec2(width, crate::scrollbars::BAR), Sense::hover());
    let request = crate::scrollbars::horizontal(
        ui,
        bar,
        egui::Id::new("automation-hscroll"),
        (
            crate::scrollbars::Span {
                first: ed.first,
                span,
                total,
            },
            meter.bar(0).len as f32,
        ),
        |painter, rail| overview(painter, rail, automation, total),
    );
    match request {
        Some(crate::scrollbars::Request::To(first)) => ed.first = first,
        Some(crate::scrollbars::Request::Show { first, span }) => {
            ed.first = first;
            ed.span = Some(span);
        }
        None => {}
    }
}

/// A miniature of the curve in the scroll bar's rail.
fn overview(painter: &egui::Painter, rail: egui::Rect, automation: &Automation, total: f32) {
    let steps = (rail.width() / 3.0).clamp(2.0, 300.0) as usize;
    let line: Vec<Pos2> = (0..=steps)
        .filter_map(|i| {
            let share = i as f32 / steps as f32;
            let v = automation.value_at(f64::from(share * total))?;
            Some(pos2(
                rail.left() + share * rail.width(),
                rail.bottom() - 2.0 - v * (rail.height() - 4.0),
            ))
        })
        .collect();
    painter.add(egui::Shape::line(
        line,
        Stroke::new(1.0, theme::AUTOMATION.gamma_multiply(0.6)),
    ));
}

/// Wheel scrolls; Ctrl+wheel zooms around the pointer.
fn wheel(ui: &egui::Ui, ed: &mut EditorState, view: &curve::View, total: f32) {
    let (zoom, scroll, pointer) =
        ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
    let span = view.rect.width() / view.px_per_tick;
    if zoom != 1.0 {
        let anchor = pointer.map_or(view.first + span / 2.0, |p| view.tick(p.x));
        let next = (span / zoom).clamp(TICKS_PER_STEP as f32 * 4.0, total);
        let share = (anchor - view.first) / span;
        ed.first = (anchor - share * next).max(0.0);
        ed.span = Some(next);
    } else {
        let delta = if scroll.x == 0.0 { scroll.y } else { scroll.x };
        if delta != 0.0 {
            ed.first = (ed.first - delta / view.px_per_tick).max(0.0);
        }
    }
}

/// The automation's tick under the song playhead, while the song plays
/// inside one of its clips.
fn playing_at(frame: &Frame<'_>, id: musiccraft_project::AutomationId) -> Option<f32> {
    let project = frame.project;
    if !frame.engine.playing || project.play_mode != PlayMode::Song {
        return None;
    }
    let at = frame.engine.position_ticks as f32;
    project.playlist.clips.iter().find_map(|c| {
        let inside = c.automation() == Some(id) && (c.start as f32..c.end() as f32).contains(&at);
        inside.then_some(at - c.start as f32 + c.offset as f32)
    })
}

/// Zoom out fully on the left; step drawing, snap and slide on the right.
fn footer(ui: &mut egui::Ui, state: &mut UiState) {
    let ed = &mut state.automation_ui;
    ui.horizontal(|ui| {
        if crate::icons::button(
            ui,
            crate::icons::Icon::Zoom,
            false,
            theme::ACCENT,
            "Zoom out fully (Home)",
        )
        .clicked()
        {
            ed.first = 0.0;
            ed.span = None;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let options = &mut ed.options;
            for (on, icon, hint) in [
                (
                    &mut options.slide,
                    crate::icons::Icon::SlideRamp,
                    "Slide: dragging a point also moves every later point",
                ),
                (
                    &mut options.snap,
                    crate::icons::Icon::Snap,
                    "Snap point times to steps (hold Alt to place freely)",
                ),
                (
                    &mut options.step,
                    crate::icons::Icon::Pencil,
                    "Step drawing: drag on the grid to draw points",
                ),
            ] {
                if crate::icons::button(ui, icon, *on, theme::ACCENT, hint).clicked() {
                    *on = !*on;
                }
            }
        });
    });
}

/// Delete, Ctrl+A and Home while the editor is the focused window.
fn keys(
    ui: &egui::Ui,
    state: &mut UiState,
    (index, automation): (usize, &Automation),
    actions: &mut Vec<Action>,
) {
    use egui::{Key, Modifiers};
    if ui.ctx().egui_wants_keyboard_input() {
        return;
    }
    let ed = &mut state.automation_ui;
    let (delete, all, home) = ui.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::Delete)
                || i.consume_key(Modifiers::NONE, Key::Backspace),
            i.consume_key(Modifiers::COMMAND, Key::A),
            i.consume_key(Modifiers::NONE, Key::Home),
        )
    });
    if delete {
        curve::delete_selection((index + 1, automation), &mut ed.edit, actions);
    }
    if all {
        curve::select_all(automation, &mut ed.edit);
    }
    if home {
        ed.first = 0.0;
        ed.span = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_open_control_menu_gets_its_default_a_frame_later() {
        let ctx = egui::Context::default();
        let project = Project::default();
        publish(&ctx, &project, None);
        let published = |ctx: &egui::Context| {
            ctx.data(|d| d.get_temp::<Published>(published_id()))
                .unwrap_or_default()
        };
        assert!(published(&ctx).default.is_none());
        // The menu for the master's pan was drawn open.
        ctx.data_mut(|d| d.insert_temp(open_menu_id(), "mixer:0:pan".to_owned()));
        publish(&ctx, &project, None);
        let (target, position) = published(&ctx).default.expect("the default");
        assert_eq!(target, "mixer:0:pan");
        assert!((position - 0.5).abs() < 1e-6);
        // Closed again: nothing is looked up.
        publish(&ctx, &project, None);
        assert!(published(&ctx).default.is_none());
    }

    #[test]
    fn a_menu_with_the_control_menu_still_opens_on_a_click() {
        // A frame with one combo box that also has the control menu, as
        // Antiserum's choice parameters do.
        let ctx = egui::Context::default();
        let frame = |events: Vec<egui::Event>| {
            let mut drawn = (egui::Rect::NOTHING, egui::Id::NULL);
            let input = egui::RawInput {
                events,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                let response = egui::ComboBox::from_id_salt("choice")
                    .selected_text("Low pass")
                    .show_ui(ui, |ui| {
                        let _ = ui.selectable_label(true, "Low pass");
                        let _ = ui.selectable_label(false, "High pass");
                    })
                    .response;
                menu(&response, "channel:1:filter1_type", &mut Vec::new());
                drawn = (response.rect, response.id);
            });
            // No renderer here: acknowledge texture uploads.
            output.textures_delta.clear();
            drawn
        };
        let (rect, combo_id) = frame(Vec::new());
        let at = rect.center();
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![egui::Event::PointerMoved(at), button(true)]);
        frame(vec![button(false)]);
        frame(Vec::new());
        assert!(
            egui::ComboBox::is_open(&ctx, combo_id),
            "the list opens; the control menu used to close it at once"
        );
    }
}
