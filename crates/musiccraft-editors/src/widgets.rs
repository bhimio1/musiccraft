//! Small custom widgets drawn with our own artwork.

use std::f32::consts::PI;

use egui::{Color32, PointerButton, Pos2, Response, Sense, Stroke, Ui, Vec2, pos2, vec2};

use crate::theme;

/// What a knob did this frame.
pub struct KnobOutput {
    /// The new value, if it changed.
    pub changed: Option<f32>,
    /// A drag started (begin a coalesced gesture).
    pub started: bool,
    /// The widget's response.
    pub response: Response,
}

/// Whether the user asked to reset a control to its default: a middle-click
/// or a double-click (see `docs/spec/controls.md`).
#[must_use]
pub fn wants_reset(response: &Response) -> bool {
    response.middle_clicked()
        || response.double_clicked()
        || (response.clicked() && response.ctx.input(|i| i.modifiers.alt))
}

/// How far past the default (as a share of the range) a drag must travel
/// before it leaves the detent.
const DETENT: f32 = 0.1;

fn detent_id(widget: egui::Id) -> egui::Id {
    widget.with("detent")
}

/// Whether `response`'s control is parked at its default by the detent.
fn at_detent(response: &Response) -> bool {
    response
        .ctx
        .data(|d| d.get_temp::<f32>(detent_id(response.id)))
        .is_some()
}

/// Applies the default detent to a drag from `t` to `proposed` (0..1
/// positions): crossing or reaching `default` parks the value there until the
/// drag has gone [`DETENT`] further. Ctrl skips it.
fn detent(ui: &Ui, response: &Response, t: f32, proposed: f32, default: f32) -> f32 {
    let id = detent_id(response.id);
    let parked = ui.data(|d| d.get_temp::<f32>(id));
    let skip = ui.input(|i| i.modifiers.command);
    let (value, parked) = match parked {
        _ if skip => (proposed, None),
        Some(past) => {
            let past = past + (proposed - t);
            if past.abs() > DETENT {
                (default + past - DETENT * past.signum(), None)
            } else {
                (default, Some(past))
            }
        }
        None => {
            let crosses = t != default
                && (proposed == default || (t - default).signum() != (proposed - default).signum());
            if crosses {
                (default, Some(0.0))
            } else {
                (proposed, None)
            }
        }
    };
    ui.data_mut(|d| match parked {
        Some(past) => {
            d.insert_temp(id, past);
        }
        None => {
            d.remove_temp::<f32>(id);
        }
    });
    value.clamp(0.0, 1.0)
}

fn status_id(widget: egui::Id) -> egui::Id {
    widget.with("status-anchor")
}

/// Records where `response`'s control wants its status dot: just outside a
/// knob's track at one o'clock, or on a fader cap's top edge.
fn set_status_anchor(response: &Response, pos: Pos2) {
    response
        .ctx
        .data_mut(|d| d.insert_temp(status_id(response.id), pos));
}

/// Where the status dot of `response`'s control goes: the spot the control
/// recorded, or its top-right corner.
#[must_use]
pub fn status_anchor(response: &Response) -> Pos2 {
    response
        .ctx
        .data(|d| d.get_temp(status_id(response.id)))
        .unwrap_or(response.rect.right_top() + vec2(-3.0, 3.0))
}

/// The point at `angle` (radians, 0 = twelve o'clock, clockwise) on a circle.
fn clock(center: Pos2, radius: f32, angle: f32) -> Pos2 {
    center + vec2(angle.sin(), -angle.cos()) * radius
}

/// One o'clock, in radians from twelve.
const ONE_OCLOCK: f32 = PI / 6.0;

/// The status dot that marks an automated or driven control (style guide,
/// controls M): a small filled dot in the automation tone.
pub fn status_dot(painter: &egui::Painter, pos: Pos2) {
    painter.circle_filled(pos, 2.0, theme::AUTOMATION);
}

/// Pixels of vertical drag for a knob's whole range, and with Shift held.
const KNOB_DRAG: f32 = 150.0;
const KNOB_DRAG_FINE: f32 = 600.0;
/// Wheel points for a knob's whole range.
const KNOB_WHEEL: f32 = 600.0;

fn steps_id(widget: egui::Id) -> egui::Id {
    widget.with("steps")
}

/// `t` (a 0..1 position) on the nearest of `steps` equal steps.
fn snap(t: f32, steps: u32) -> f32 {
    let n = steps as f32;
    (t * n).round() / n
}

/// Where a stepped knob's gesture really is: the unsnapped position the
/// drag and wheel have reached, kept while the knob is dragged or hovered so
/// that movements smaller than a step add up instead of snapping back. It
/// starts over from `t` when the value changes from elsewhere (automation, a
/// preset) to something the gesture did not reach. The value seen last frame
/// is remembered too, so a host that applies the change a few frames late
/// does not count as such a change.
fn stepped_base(ui: &Ui, response: &Response, t: f32, steps: u32) -> f32 {
    let held = ui.data(|d| d.get_temp::<(f32, f32)>(steps_id(response.id)));
    let near = |a: f32, b: f32| (a - b).abs() < 0.25 / steps as f32;
    match held {
        Some((raw, last)) if near(snap(raw, steps), t) || near(last, t) => raw,
        _ => t,
    }
}

/// The one knob rule for every size, over a 0..1 position: vertical drag
/// (finer with Shift), the wheel, and reset to `default`. A knob with
/// `steps` lands only on those steps; small movements add up until they
/// reach the next one (see [`stepped_base`]).
fn knob_input(
    ui: &Ui,
    response: &Response,
    t: f32,
    default: f32,
    steps: Option<u32>,
) -> Option<f32> {
    let steps = steps.filter(|n| *n > 0);
    let base = steps.map_or(t, |n| stepped_base(ui, response, t, n));
    let mut changed = None;
    if response.dragged_by(PointerButton::Primary) {
        let fine = ui.input(|i| i.modifiers.shift);
        let delta = -response.drag_delta().y / if fine { KNOB_DRAG_FINE } else { KNOB_DRAG };
        if delta != 0.0 {
            changed = Some(detent(
                ui,
                response,
                base,
                (base + delta).clamp(0.0, 1.0),
                default,
            ));
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove_temp::<f32>(detent_id(response.id)));
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            changed = Some((base + scroll / KNOB_WHEEL).clamp(0.0, 1.0));
        }
    }
    if wants_reset(response) {
        changed = Some(default);
    }
    let Some(n) = steps else {
        return changed;
    };
    let id = steps_id(response.id);
    let raw = changed.unwrap_or(base);
    ui.data_mut(|d| {
        if (response.dragged() || response.hovered()) && !response.drag_stopped() {
            d.insert_temp(id, (raw, t));
        } else {
            d.remove_temp::<(f32, f32)>(id);
        }
    });
    let reset = wants_reset(response);
    changed
        .map(|v| snap(v, n))
        .filter(|v| reset || (v - t).abs() >= 0.25 / n as f32)
}

/// Whether a knob gesture began this frame: a drag, a reset or a wheel turn.
/// Each is its own undo step.
fn knob_started(ui: &Ui, response: &Response) -> bool {
    response.drag_started_by(PointerButton::Primary)
        || wants_reset(response)
        || (response.hovered()
            && ui.input(|i| i.smooth_scroll_delta.y != 0.0)
            && !response.dragged())
}

/// The knob body: lighter while hovered or dragged.
fn knob_body(response: &Response) -> Color32 {
    if response.hovered() || response.dragged() {
        theme::RAISED_HOVER
    } else {
        theme::RAISED
    }
}

/// A rotary knob. Drag vertically to change; middle-click or double-click
/// resets to `default`.
pub fn knob(
    ui: &mut Ui,
    value: f32,
    min: f32,
    max: f32,
    default: f32,
    bipolar: bool,
) -> KnobOutput {
    stepped_knob(ui, value, min, max, default, bipolar, None)
}

/// A [`knob`] whose value lands only on `steps` equal steps across its range
/// (for example a whole-number parameter); `None` is continuous.
pub fn stepped_knob(
    ui: &mut Ui,
    value: f32,
    min: f32,
    max: f32,
    default: f32,
    bipolar: bool,
    steps: Option<u32>,
) -> KnobOutput {
    let size = 22.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click_and_drag());
    let range = (max - min).max(1e-6);
    let to_t = |v: f32| ((v - min) / range).clamp(0.0, 1.0);
    let from_t = |t: f32| match steps.filter(|n| *n > 0) {
        // Whole steps, so integer ranges give exact whole numbers.
        Some(n) => min + (t * n as f32).round() * (range / n as f32),
        None => min + t * range,
    };
    let changed = knob_input(ui, &response, to_t(value), to_t(default), steps).map(from_t);
    let shown = changed.unwrap_or(value);
    let painter = ui.painter_at(rect.expand(2.0));
    let center = rect.center();
    let radius = size / 2.0 - 2.0;
    painter.circle_filled(center, radius, knob_body(&response));
    // A thin ring keeps a centred knob visible on dark wells.
    painter.circle_stroke(center, radius, Stroke::new(1.0, theme::GRID_BAR));
    let t = ((shown - min) / range).clamp(0.0, 1.0);
    // A disabled knob keeps its body; only its marks turn faint.
    let enabled = ui.is_enabled();
    let start = PI * 0.75;
    let sweep = PI * 1.5;
    let angle_of = |t: f32| start + sweep * t;
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        (0..=16)
            .map(|i| {
                let a = angle_of(from + (to - from) * i as f32 / 16.0);
                center + vec2(a.cos(), a.sin()) * (radius + 1.0)
            })
            .collect()
    };
    // The unlit track, so a knob at its minimum still reads as a knob.
    painter.add(egui::Shape::line(
        arc(0.0, 1.0),
        Stroke::new(2.0, theme::PANEL),
    ));
    let from = if bipolar { 0.5 } else { 0.0 };
    painter.add(egui::Shape::line(
        arc(from, t),
        Stroke::new(2.0, theme::ACCENT),
    ));
    if at_detent(&response) {
        let a = angle_of(to_t(default));
        let tick = center + vec2(a.cos(), a.sin()) * (radius + 1.0);
        painter.circle_filled(tick, 1.5, theme::HIGHLIGHT);
    }
    let a = angle_of(t);
    painter.line_segment(
        [center, center + vec2(a.cos(), a.sin()) * (radius - 2.0)],
        Stroke::new(
            2.0,
            if enabled {
                theme::TEXT
            } else {
                theme::TEXT_FAINT
            },
        ),
    );
    set_status_anchor(&response, clock(center, radius + 3.0, ONE_OCLOCK));
    KnobOutput {
        changed,
        started: knob_started(ui, &response),
        response,
    }
}

/// A larger knob over a normalized 0..1 value, with a caption that shows the
/// formatted value while hovered or dragged. Middle-click or double-click
/// resets to `default`.
pub fn labeled_knob(
    ui: &mut Ui,
    label: &str,
    t: f32,
    default: f32,
    bipolar: bool,
    value: &str,
) -> KnobOutput {
    stepped_labeled_knob(ui, label, t, default, bipolar, value, None)
}

/// A [`labeled_knob`] that lands only on `steps` equal steps across its
/// travel (for example a whole-number parameter); `None` is continuous.
pub fn stepped_labeled_knob(
    ui: &mut Ui,
    label: &str,
    t: f32,
    default: f32,
    bipolar: bool,
    value: &str,
    steps: Option<u32>,
) -> KnobOutput {
    let size = vec2(52.0, 60.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let changed = knob_input(ui, &response, t, default, steps);
    let shown = changed.unwrap_or(t);
    let painter = ui.painter_at(rect);
    let center = pos2(rect.center().x, rect.top() + 22.0);
    let radius = 15.0;
    painter.circle_filled(center, radius + 3.0, theme::PANEL_DARK);
    painter.circle_filled(center, radius - 2.0, knob_body(&response));
    let start = PI * 0.75;
    let sweep = PI * 1.5;
    let angle = |k: f32| start + sweep * k;
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        (0..=24)
            .map(|i| {
                let a = angle(from + (to - from) * i as f32 / 24.0);
                center + vec2(a.cos(), a.sin()) * (radius + 1.0)
            })
            .collect()
    };
    painter.add(egui::Shape::line(
        arc(0.0, 1.0),
        Stroke::new(3.0, theme::PANEL),
    ));
    let from = if bipolar { 0.5 } else { 0.0 };
    painter.add(egui::Shape::line(
        arc(from, shown),
        Stroke::new(3.0, theme::ACCENT),
    ));
    if at_detent(&response) {
        let a = angle(default);
        let tick = center + vec2(a.cos(), a.sin()) * (radius + 1.0);
        painter.circle_filled(tick, 2.0, theme::HIGHLIGHT);
    }
    let a = angle(shown);
    painter.line_segment(
        [
            center + vec2(a.cos(), a.sin()) * 4.0,
            center + vec2(a.cos(), a.sin()) * (radius - 3.0),
        ],
        Stroke::new(2.0, theme::TEXT),
    );
    set_status_anchor(&response, clock(center, radius + 5.0, ONE_OCLOCK));
    let active = response.hovered() || response.dragged();
    painter.text(
        pos2(rect.center().x, rect.bottom() - 9.0),
        egui::Align2::CENTER_CENTER,
        if active { value } else { label },
        egui::FontId::proportional(theme::FONT_LABEL),
        if active {
            theme::LCD_TEXT
        } else {
            theme::TEXT_DIM
        },
    );
    KnobOutput {
        changed,
        started: knob_started(ui, &response),
        response,
    }
}

pub use musiccraft_params::{volume_from_unit as fader_to_gain, volume_to_unit as gain_to_fader};

/// A vertical mixer fader. Drag the cap; middle-click or double-click returns
/// to unity.
pub fn fader(ui: &mut Ui, gain: f32, max: f32, size: Vec2) -> KnobOutput {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let travel = rect.shrink2(vec2(0.0, 10.0));
    let mut changed = None;
    let position = gain_to_fader(gain, max);
    let cap_y = travel.bottom() - travel.height() * position;
    // A press on the rail away from the cap glides the cap toward the
    // pointer while held, instead of jumping.
    let on_rail = ui
        .input(|i| i.pointer.press_origin())
        .is_some_and(|p| (p.y - cap_y).abs() > 8.0);
    let mut glide_started = false;
    let primary = ui.input(|i| i.pointer.primary_down());
    if on_rail && primary && response.is_pointer_button_down_on() {
        if let Some(p) = ui.input(|i| i.pointer.latest_pos()) {
            let target = ((travel.bottom() - p.y) / travel.height()).clamp(0.0, 1.0);
            let step = (target - position).clamp(-0.02, 0.02);
            if step.abs() > 1e-4 {
                changed = Some(fader_to_gain(position + step, max));
                ui.ctx().request_repaint();
            }
            glide_started = ui.input(|i| i.pointer.primary_pressed());
        }
    } else if response.dragged_by(PointerButton::Primary) {
        let fine = ui.input(|i| i.modifiers.shift);
        let delta = -response.drag_delta().y / travel.height() * if fine { 0.25 } else { 1.0 };
        if delta != 0.0 {
            let unity = gain_to_fader(1.0, max);
            let proposed = (position + delta).clamp(0.0, 1.0);
            let moved = detent(ui, &response, position, proposed, unity);
            changed = Some(fader_to_gain(moved, max));
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove_temp::<f32>(detent_id(response.id)));
    }
    if response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            changed = Some(fader_to_gain(position + scroll / 800.0, max));
        }
    }
    if wants_reset(&response) {
        changed = Some(1.0);
    }
    let shown = changed.map_or(position, |g| gain_to_fader(g, max));
    let painter = ui.painter_at(rect.expand(2.0));
    let x = rect.center().x;
    let groove =
        egui::Rect::from_center_size(egui::pos2(x, travel.center().y), vec2(4.0, travel.height()));
    painter.rect_filled(groove, 2.0, theme::METER_BG);
    painter.rect_stroke(
        groove,
        2.0,
        Stroke::new(1.0, theme::LCD_EDGE),
        egui::StrokeKind::Outside,
    );
    let unity = travel.bottom() - travel.height() * 0.75;
    painter.line_segment(
        [egui::pos2(x - 9.0, unity), egui::pos2(x + 9.0, unity)],
        Stroke::new(1.0, theme::TEXT_DIM),
    );
    let y = travel.bottom() - travel.height() * shown;
    let cap = egui::Rect::from_center_size(egui::pos2(x, y), vec2(rect.width().min(26.0), 16.0));
    let fill = if response.dragged() || response.hovered() {
        theme::RAISED_HOVER
    } else {
        theme::RAISED
    };
    painter.rect_filled(cap, 3.0, fill);
    painter.rect_stroke(
        cap,
        3.0,
        Stroke::new(1.0, theme::TEXT_DIM),
        egui::StrokeKind::Inside,
    );
    painter.line_segment(
        [
            egui::pos2(cap.left() + 3.0, y),
            egui::pos2(cap.right() - 3.0, y),
        ],
        Stroke::new(2.0, theme::ACCENT),
    );
    set_status_anchor(&response, cap.right_top() + vec2(-4.0, 4.0));
    let started = (response.drag_started_by(PointerButton::Primary) && !on_rail)
        || glide_started
        || wants_reset(&response)
        || (response.hovered()
            && ui.input(|i| i.smooth_scroll_delta.y != 0.0)
            && !response.dragged());
    KnobOutput {
        changed,
        started,
        response,
    }
}

/// A round on/off indicator button.
pub fn dot(ui: &mut Ui, on: bool, color: Color32) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::click());
    let fill = if on { color } else { theme::PANEL_DARK };
    ui.painter().circle_filled(rect.center(), 5.0, fill);
    ui.painter()
        .circle_stroke(rect.center(), 5.0, Stroke::new(1.0, theme::TEXT_DIM));
    response
}

/// Runs `add_contents` with its widgets disabled when `enabled` is false.
/// Unlike egui's own fade, a disabled control keeps its fill: only its text
/// and glyphs turn `TEXT_FAINT`, and it never reacts to hover.
pub fn enabled_ui<R>(ui: &mut Ui, enabled: bool, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    ui.scope(|ui| {
        if !enabled {
            disable(ui);
        }
        add_contents(ui)
    })
    .inner
}

/// Adds `widget`, disabled when `enabled` is false (see [`enabled_ui`]).
pub fn add_enabled(ui: &mut Ui, enabled: bool, widget: impl egui::Widget) -> Response {
    enabled_ui(ui, enabled, |ui| ui.add(widget))
}

/// Disables the rest of `ui` in our style: no opacity fade, faint text.
fn disable(ui: &mut Ui) {
    let opacity = ui.opacity();
    ui.disable();
    ui.set_opacity(opacity);
    let visuals = &mut ui.style_mut().visuals;
    visuals.override_text_color = Some(theme::TEXT_FAINT);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
    ] {
        widget.fg_stroke.color = theme::TEXT_FAINT;
    }
}
/// An LED switch: a round light with its label to the right. Clicking the
/// light or the label toggles `on`; the response reports `changed()`.
pub fn led_switch(ui: &mut Ui, on: &mut bool, label: impl Into<egui::WidgetText>) -> Response {
    let galley = label.into().into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    let gap = if galley.size().x > 0.0 { 5.0 } else { 0.0 };
    let size = vec2(14.0 + gap + galley.size().x, galley.size().y.max(16.0));
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let centre = pos2(rect.left() + 7.0, rect.center().y);
    let painter = ui.painter();
    let ring = if response.hovered() {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    painter.circle_filled(centre, 5.5, theme::PANEL_DARK);
    if *on {
        painter.circle_filled(centre, 4.0, theme::ACCENT);
    }
    painter.circle_stroke(centre, 5.5, Stroke::new(1.0, ring));
    let text = if ui.is_enabled() {
        theme::TEXT
    } else {
        theme::TEXT_FAINT
    };
    painter.galley(
        pos2(
            rect.left() + 14.0 + gap,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        text,
    );
    response
}

/// Converts linear gain to a 0..1 meter position (-60 dB .. +6 dB).
fn meter_position(gain: f32) -> f32 {
    if gain <= 1e-6 {
        return 0.0;
    }
    let db = 20.0 * gain.log10();
    ((db + 60.0) / 66.0).clamp(0.0, 1.0)
}

/// The peak of one meter channel held for 1.5 s, then falling, and whether
/// it clipped in the last 3 s.
pub fn held_peak(ui: &Ui, id: egui::Id, level: f32) -> (f32, bool) {
    let now = ui.input(|i| i.time);
    let (mut peak, mut until, mut clip_until) = ui
        .data(|d| d.get_temp::<(f32, f64, f64)>(id))
        .unwrap_or((0.0, 0.0, 0.0));
    if level >= peak {
        peak = level;
        until = now + 1.5;
    } else if now > until {
        peak = (peak * 0.9).max(level);
    }
    if level >= 1.0 {
        clip_until = now + 3.0;
    }
    ui.data_mut(|d| d.insert_temp(id, (peak, until, clip_until)));
    (peak, now < clip_until)
}

/// A small raised tile with a chevron pointing left (`left`) or right, for
/// stepping through presets.
pub fn chevron_button(ui: &mut Ui, left: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::click());
    let hovered = response.hovered();
    let fill = if response.is_pointer_button_down_on() {
        theme::ACCENT_DIM
    } else if hovered {
        theme::RAISED_HOVER
    } else {
        theme::RAISED
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, fill);
    let color = if hovered {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    let c = rect.center();
    let (tip, back) = if left { (-2.5, 2.0) } else { (2.5, -2.0) };
    let stroke = Stroke::new(1.5, color);
    painter.line_segment([pos2(c.x + back, c.y - 4.0), pos2(c.x + tip, c.y)], stroke);
    painter.line_segment([pos2(c.x + tip, c.y), pos2(c.x + back, c.y + 4.0)], stroke);
    response
}

/// A previous (`left`) or next preset arrow beside the preset list whose
/// `ComboBox` uses `list_salt` in this `ui`. Right-click opens that list, as
/// a click on the list does. The list opens on the next frame so the
/// right-click does not count as a click outside it; the previous arrow,
/// drawn before the list, opens it.
pub fn preset_arrow(ui: &mut Ui, left: bool, list_salt: impl egui::AsIdSalt) -> Response {
    let popup = ui
        .make_persistent_id(egui::IdSalt::new(&list_salt))
        .with("popup");
    let pending = popup.with("open-from-arrow");
    if left && ui.data_mut(|d| d.remove_temp::<bool>(pending)).is_some() {
        egui::Popup::open_id(ui.ctx(), popup);
    }
    let hint = if left {
        "Previous preset · right-click for the list"
    } else {
        "Next preset · right-click for the list"
    };
    let response = crate::hint::Hint::hint(chevron_button(ui, left), hint);
    if response.secondary_clicked() {
        ui.data_mut(|d| d.insert_temp(pending, true));
        ui.ctx().request_repaint();
    }
    response
}

/// A vertical stereo peak meter.
pub fn meter(ui: &mut Ui, levels: (f32, f32), size: Vec2) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, theme::METER_BG);
    // The edge keeps a silent meter visible on any background.
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, theme::LCD_EDGE),
        egui::StrokeKind::Inside,
    );
    let half = rect.width() / 2.0;
    for (i, level) in [levels.0, levels.1].into_iter().enumerate() {
        let x0 = rect.left() + half * i as f32 + 1.0;
        let h = rect.height() * meter_position(level);
        let bar = egui::Rect::from_min_max(
            pos2(x0, rect.bottom() - h),
            pos2(x0 + half - 2.0, rect.bottom()),
        );
        let color = if level >= 1.0 {
            theme::DANGER
        } else if level > 0.5 {
            theme::HIGHLIGHT
        } else {
            theme::ACCENT
        };
        painter.rect_filled(bar, 1.0, color);
        // The peak holds for a moment as a thin line; a clip leaves a
        // red mark at the top for a few seconds.
        let (peak, clip) = held_peak(ui, response.id.with(i), level);
        if peak > 1e-6 {
            let y = rect.bottom() - rect.height() * meter_position(peak);
            painter.line_segment(
                [pos2(x0, y), pos2(x0 + half - 2.0, y)],
                Stroke::new(1.0, theme::TEXT),
            );
        }
        if clip {
            let mark = egui::Rect::from_min_size(pos2(x0, rect.top() + 1.0), vec2(half - 2.0, 3.0));
            painter.rect_filled(mark, 0.0, theme::DANGER);
        }
    }
    // 0 dB tick.
    let zero = rect.bottom() - rect.height() * meter_position(1.0);
    painter.line_segment(
        [pos2(rect.left(), zero), pos2(rect.right(), zero)],
        Stroke::new(1.0, theme::TEXT_DIM),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_widgets_keep_their_opacity_and_turn_their_text_faint() {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            enabled_ui(ui, true, |ui| assert!(ui.is_enabled()));
            let opacity = ui.opacity();
            enabled_ui(ui, false, |ui| {
                assert!(!ui.is_enabled());
                assert!((ui.opacity() - opacity).abs() < f32::EPSILON);
                let visuals = ui.visuals();
                assert_eq!(visuals.override_text_color, Some(theme::TEXT_FAINT));
                assert_eq!(visuals.widgets.inactive.fg_stroke.color, theme::TEXT_FAINT);
                assert_eq!(
                    visuals.widgets.inactive.weak_bg_fill,
                    ui.ctx()
                        .global_style()
                        .visuals
                        .widgets
                        .inactive
                        .weak_bg_fill,
                    "fills stay"
                );
            });
            assert!(ui.is_enabled(), "only the inner scope is disabled");
        });
        output.textures_delta.clear();
    }

    #[test]
    fn fader_curve_round_trips_and_puts_unity_at_three_quarters() {
        let max = 1.25;
        assert!((gain_to_fader(1.0, max) - 0.75).abs() < 1e-6);
        assert!(gain_to_fader(0.0, max).abs() < f32::EPSILON);
        assert!((gain_to_fader(max, max) - 1.0).abs() < 1e-4);
        for gain in [0.001, 0.05, 0.3, 0.8, 1.0, 1.2] {
            let back = fader_to_gain(gain_to_fader(gain, max), max);
            assert!((back - gain).abs() / gain < 1e-3, "{gain} -> {back}");
        }
    }

    /// Runs `widget` at the top left for a few frames of `events` each and
    /// returns every value it reported changing to, with whether a gesture
    /// started then.
    fn drive(
        frames: Vec<Vec<egui::Event>>,
        mut widget: impl FnMut(&mut Ui) -> KnobOutput,
    ) -> Vec<(f32, bool)> {
        let ctx = egui::Context::default();
        let mut out = Vec::new();
        for (i, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0))),
                time: Some(i as f64 / 60.0),
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                let result = widget(ui);
                if let Some(value) = result.changed {
                    out.push((value, result.started));
                }
            })
            .textures_delta
            .clear();
        }
        out
    }

    fn click(at: Pos2, button: PointerButton) -> Vec<Vec<egui::Event>> {
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        vec![
            vec![],
            vec![egui::Event::PointerMoved(at), press(true)],
            vec![press(false)],
            vec![],
        ]
    }

    fn drag(from: Pos2, to: Pos2, button: PointerButton) -> Vec<Vec<egui::Event>> {
        let press = |pos, pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut frames = vec![
            vec![],
            vec![egui::Event::PointerMoved(from), press(from, true)],
        ];
        for i in 1..=4 {
            frames.push(vec![egui::Event::PointerMoved(
                from + (to - from) * (i as f32 / 4.0),
            )]);
        }
        frames.push(vec![press(to, false)]);
        frames
    }

    #[test]
    fn middle_click_resets_knobs_and_faders_as_their_own_gesture() {
        // Every widget sits at the top left of the panel; aim inside it.
        let at = pos2(20.0, 20.0);
        let knob_run = drive(click(at, PointerButton::Middle), |ui| {
            knob(ui, 0.3, 0.0, 1.0, 0.8, false)
        });
        assert_eq!(knob_run, [(0.8, true)]);
        let labeled = drive(click(at, PointerButton::Middle), |ui| {
            labeled_knob(ui, "Cutoff", 0.2, 0.6, false, "")
        });
        assert_eq!(labeled, [(0.6, true)]);
        let fader_run = drive(click(at, PointerButton::Middle), |ui| {
            fader(ui, 0.2, 1.25, vec2(30.0, 120.0))
        });
        assert_eq!(fader_run, [(1.0, true)]);
    }

    #[test]
    fn tiny_and_medium_knobs_follow_one_drag_rule() {
        // Every widget sits at the top left of the panel; aim inside it.
        let at = pos2(10.0, 12.0);
        let up = drag(at, at - vec2(0.0, 30.0), PointerButton::Primary);
        let tiny = drive(up.clone(), |ui| knob(ui, 0.0, 0.0, 1.0, 0.0, false));
        let medium = drive(up, |ui| labeled_knob(ui, "Cutoff", 0.0, 0.0, false, ""));
        // Each frame reports its own step from the value passed in.
        let total = |run: &[(f32, bool)]| run.iter().map(|(v, _)| v).sum::<f32>();
        assert!((total(&tiny) - 30.0 / KNOB_DRAG).abs() < 1e-3, "{tiny:?}");
        assert!(
            (total(&tiny) - total(&medium)).abs() < 1e-4,
            "{tiny:?} {medium:?}"
        );
    }

    /// Drags a knob that keeps the value it reports, as the app does, and
    /// returns the value after each frame.
    fn drag_knob(start: f32, default: f32, frames: Vec<Vec<egui::Event>>) -> Vec<f32> {
        let ctx = egui::Context::default();
        let mut value = start;
        let mut seen = Vec::new();
        for (i, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0))),
                time: Some(i as f64 / 60.0),
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                if let Some(v) = knob(ui, value, 0.0, 1.0, default, false).changed {
                    value = v;
                }
            })
            .textures_delta
            .clear();
            seen.push(value);
        }
        seen
    }

    /// Frames that press at `at` and then move the pointer `dy` points
    /// vertically per frame, `n` times, without letting go.
    fn slow_drag(at: Pos2, dy: f32, n: usize) -> Vec<Vec<egui::Event>> {
        let mut frames = drag(at, at, PointerButton::Primary);
        frames.truncate(2);
        for i in 1..=n {
            frames.push(vec![egui::Event::PointerMoved(
                at + vec2(0.0, dy * i as f32),
            )]);
        }
        frames
    }

    /// Like [`drag_knob`] for a knob over `min..=max` with whole-number
    /// steps; `outside` may change the value before a frame, as automation
    /// would.
    fn drag_stepped(
        start: f32,
        max: f32,
        frames: Vec<Vec<egui::Event>>,
        mut outside: impl FnMut(usize, &mut f32),
    ) -> Vec<f32> {
        let ctx = egui::Context::default();
        let mut value = start;
        let mut seen = Vec::new();
        for (i, events) in frames.into_iter().enumerate() {
            outside(i, &mut value);
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0))),
                time: Some(i as f64 / 60.0),
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                let steps = Some(max as u32);
                if let Some(v) = stepped_knob(ui, value, 0.0, max, 0.0, false, steps).changed {
                    value = v;
                }
            })
            .textures_delta
            .clear();
            seen.push(value);
        }
        seen
    }

    #[test]
    fn slow_drags_add_up_on_a_stepped_knob() {
        let at = pos2(10.0, 12.0);
        // Two values: half the knob's travel (75 points) reaches the other
        // one, even moving 3 points a frame.
        let seen = drag_stepped(0.0, 1.0, slow_drag(at, -3.0, 30), |_, _| {});
        let flipped = seen.iter().position(|v| *v == 1.0);
        assert!(flipped.is_some_and(|i| (24..=30).contains(&i)), "{seen:?}");
        // And back down the same way.
        let seen = drag_stepped(1.0, 1.0, slow_drag(at, 3.0, 30), |_, _| {});
        assert_eq!(*seen.last().unwrap(), 0.0, "{seen:?}");
        // Five steps: one every 30 points, each landing on a whole number.
        let seen = drag_stepped(0.0, 5.0, slow_drag(at, -2.0, 75), |_, _| {});
        assert_eq!(*seen.last().unwrap(), 5.0, "{seen:?}");
        let mut levels = seen.clone();
        levels.dedup();
        assert_eq!(levels, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0]);
    }

    #[test]
    fn a_stepped_knob_starts_over_when_the_value_changes_elsewhere() {
        let at = pos2(10.0, 12.0);
        // Halfway to the next step, automation moves the value to 3; the
        // drag carries on from there instead of jumping back toward 1.
        let seen = drag_stepped(0.0, 5.0, slow_drag(at, -2.0, 12), |i, value| {
            if i == 8 {
                *value = 3.0;
            }
        });
        assert!(seen[8..].iter().all(|v| *v >= 3.0), "{seen:?}");
    }

    #[test]
    fn wheel_turns_add_up_on_a_stepped_knob() {
        let at = pos2(10.0, 12.0);
        let mut frames = vec![vec![egui::Event::PointerMoved(at)]];
        // 40 points a turn; 300 points cross half of a two-value knob.
        for _ in 0..10 {
            frames.push(vec![egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: vec2(0.0, 40.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }]);
            frames.extend((0..30).map(|_| vec![]));
        }
        let seen = drag_stepped(0.0, 1.0, frames, |_, _| {});
        assert_eq!(*seen.last().unwrap(), 1.0, "{seen:?}");
    }

    #[test]
    fn alt_click_resets_a_knob() {
        let alt = |events: Vec<egui::Event>| {
            let mut with = vec![egui::Event::ModifiersChanged(egui::Modifiers::ALT)];
            with.extend(events.into_iter().map(|e| match e {
                egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    ..
                } => egui::Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: egui::Modifiers::ALT,
                },
                other => other,
            }));
            with
        };
        let frames = click(pos2(10.0, 12.0), PointerButton::Primary)
            .into_iter()
            .map(alt)
            .collect();
        let run = drive(frames, |ui| knob(ui, 0.3, 0.0, 1.0, 0.8, false));
        assert_eq!(run, [(0.8, true)]);
    }

    #[test]
    fn dragging_through_the_default_pauses_there() {
        let at = pos2(10.0, 12.0);
        // 0.06 of the range per frame upward, from 0.3 toward a 0.5 default.
        let frames = |n: usize| {
            let mut frames = drag(at, at, PointerButton::Primary);
            frames.truncate(2);
            for i in 1..=n {
                frames.push(vec![egui::Event::PointerMoved(
                    at - vec2(0.0, 9.0 * i as f32),
                )]);
            }
            frames
        };
        let seen = drag_knob(0.3, 0.5, frames(8));
        let parked = seen.iter().filter(|v| (**v - 0.5).abs() < 1e-6).count();
        assert!(parked >= 2, "held at the default for a while: {seen:?}");
        assert!(*seen.last().unwrap() > 0.5, "then carries on: {seen:?}");
    }

    #[test]
    fn clicking_an_led_switch_label_toggles_it() {
        let ctx = egui::Context::default();
        let mut on = false;
        let mut changes = 0;
        // The label starts 19 points in; click on it, past the light.
        for (i, events) in click(pos2(40.0, 10.0), PointerButton::Primary)
            .into_iter()
            .enumerate()
        {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(400.0, 400.0))),
                time: Some(i as f64 / 60.0),
                events,
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                if led_switch(ui, &mut on, "Reverse playback").changed() {
                    changes += 1;
                }
            })
            .textures_delta
            .clear();
        }
        assert!(on);
        assert_eq!(changes, 1);
    }

    #[test]
    fn holding_the_fader_rail_glides_the_cap_toward_the_pointer() {
        // The cap of a 120-point fader at unity sits near y = 37; hold the
        // button down near the bottom of the rail.
        let at = pos2(15.0, 100.0);
        let mut frames = vec![
            vec![],
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ];
        frames.extend(std::iter::repeat_n(vec![], 5));
        let run = drive(frames, |ui| fader(ui, 1.0, 1.25, vec2(30.0, 120.0)));
        assert!(run.len() >= 3, "{run:?}");
        assert!(run.first().is_some_and(|(_, started)| *started));
        // Each frame moves a small step down, never a jump.
        let first = run.first().map_or(1.0, |(g, _)| *g);
        assert!(first < 1.0 && first > 0.8, "{run:?}");
    }

    #[test]
    fn only_the_left_button_drags_a_knob() {
        let (from, to) = (pos2(18.0, 18.0), pos2(18.0, -40.0));
        for button in [PointerButton::Secondary, PointerButton::Middle] {
            let moved = drive(drag(from, to, button), |ui| {
                knob(ui, 0.3, 0.0, 1.0, 0.8, false)
            });
            assert!(
                moved.iter().all(|(value, _)| (*value - 0.8).abs() < 1e-6),
                "{button:?} drag moved the knob: {moved:?}"
            );
        }
        let moved = drive(drag(from, to, PointerButton::Primary), |ui| {
            knob(ui, 0.3, 0.0, 1.0, 0.8, false)
        });
        assert!(moved.iter().any(|(value, _)| *value > 0.3), "{moved:?}");
    }

    #[test]
    fn meter_scale() {
        assert!(meter_position(0.0).abs() < f32::EPSILON);
        assert!((meter_position(1.0) - 60.0 / 66.0).abs() < 1e-4);
        assert!((meter_position(100.0) - 1.0).abs() < f32::EPSILON);
    }
}
