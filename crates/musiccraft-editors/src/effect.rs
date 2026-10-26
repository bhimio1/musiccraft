//! An effect's editor: a preset bar with bypass, a graph you can drag (filter
//! and EQ curves, dynamics transfer curves, delay taps…) and the effect's
//! knobs. The mixer's effect windows and the exported plugins both draw it;
//! it reports what the user changed as [`Edit`]s, which the app turns into
//! commands and a plugin into host parameter gestures.

use egui::{Color32, Pos2, Rect, Sense, Stroke, pos2, vec2};
use musiccraft_dsp::beat::{BEAT_POINTS, FIRST_TIME, FIRST_VOLUME};
use musiccraft_dsp::blocks::{BLOCK_CONTROLS, BLOCK_NAMES, CHAIN_STAGES};
use musiccraft_dsp::character::GRAPHIC_BANDS;
use musiccraft_dsp::modulation::{autopan_shape, gate_step_on};
use musiccraft_dsp::{SvfMode, response};
use musiccraft_params::presets;
use musiccraft_params::{
    EffectKind, LFO_FIRST_STEP, LFO_SHAPES, LFO_STEPS, MAX_EFFECT_PARAMS, ParamSpec,
    SHAPER_FIRST_POINT, SHAPER_POINTS, index_of,
};

use crate::hint::Hint;
use crate::{theme, widgets};

/// Beats shown on the multi-tap delay's graph.
const TAP_SPAN: f32 = 4.0;

/// Top and bottom (dB) of a frequency graph.
fn db_range(kind: EffectKind) -> (f32, f32) {
    match kind {
        EffectKind::GraphicEq | EffectKind::ParametricEq => (18.0, -18.0),
        EffectKind::BassBoost => (24.0, -12.0),
        _ => (24.0, -48.0),
    }
}

/// A multi-tap delay level for a pointer height.
fn tap_level(rect: Rect, y: f32) -> f32 {
    ((rect.bottom() - 6.0 - y) / (rect.height() - 12.0)).clamp(0.0, 1.0)
}

/// Size of the graph area.
pub const GRAPH: egui::Vec2 = vec2(380.0, 150.0);
/// Lowest and highest frequency on frequency graphs.
const F_LOW: f32 = 20.0;
const F_HIGH: f32 = 20_000.0;

/// What the editor shows: an effect's kind, values and bypass state.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectView {
    /// The effect.
    pub kind: EffectKind,
    /// Whether it is on (not bypassed).
    pub enabled: bool,
    /// Parameter values, in the order of `kind.params()`.
    pub values: [f32; MAX_EFFECT_PARAMS],
    /// The convolution reverb's impulse: `ir:<room>` or a file path.
    pub file: Option<String>,
}

impl EffectView {
    /// An enabled effect at its default values.
    #[must_use]
    pub fn new(kind: EffectKind) -> Self {
        Self {
            kind,
            enabled: true,
            values: kind.defaults(),
            file: None,
        }
    }

    /// The value of parameter `name` (0 if the effect has none).
    #[must_use]
    pub fn value(&self, name: &str) -> f32 {
        value(self, name)
    }
}

/// A change the user made in the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// A drag began: the [`Edit::Drag`]s that follow belong together (one
    /// undo step, one host gesture).
    Begin,
    /// Values set while dragging (or scrolling) a control.
    Drag(Vec<(&'static str, f32)>),
    /// Values set by one click (a preset, a gate step).
    Set(Vec<(&'static str, f32)>),
    /// The bypass switch was clicked.
    ToggleBypass,
}

/// Whether parameter `index` of `kind` routes to another part of the mixer
/// (sidechain sources, send targets, a followed channel); plugins have none.
#[must_use]
pub fn is_routing(kind: EffectKind, index: usize) -> bool {
    Some(index) == kind.channel_ref_index() || kind.track_ref_indices().any(|i| i == index)
}

/// The bypass switch and the effect's factory presets.
pub fn preset_bar(ui: &mut egui::Ui, id: egui::Id, effect: &EffectView, edits: &mut Vec<Edit>) {
    ui.horizontal(|ui| {
        let on = effect.enabled;
        if widgets::dot(ui, on, theme::ACCENT)
            .hint(if on {
                "On — click to bypass"
            } else {
                "Bypassed — click to turn on"
            })
            .clicked()
        {
            edits.push(Edit::ToggleBypass);
        }
        preset_picker(ui, id, effect, edits);
    });
}

/// The previous chevron, the preset drop-down ("(custom)" when no preset
/// matches) and the next chevron, laid out in the caller's row (the app puts
/// them in the effect window's header).
pub fn preset_picker(ui: &mut egui::Ui, id: egui::Id, effect: &EffectView, edits: &mut Vec<Edit>) {
    let list = presets::effect(effect.kind);
    let specs = effect.kind.params();
    let current = list.iter().position(|p| p.matches(specs, &effect.values));
    let step = |delta: isize| {
        let n = list.len() as isize;
        let from = current.map_or(if delta > 0 { -1 } else { 0 }, |c| c as isize);
        ((from + delta).rem_euclid(n.max(1))) as usize
    };
    let mut chosen = None;
    let salt = id.with("preset");
    if crate::widgets::preset_arrow(ui, true, salt).clicked() {
        chosen = Some(step(-1));
    }
    let name = current.map_or("(custom)", |i| list[i].name);
    egui::ComboBox::from_id_salt(salt)
        .width(170.0)
        .selected_text(name)
        .show_ui(ui, |ui| {
            for (i, preset) in list.iter().enumerate() {
                if ui
                    .selectable_label(current == Some(i), preset.name)
                    .clicked()
                {
                    chosen = Some(i);
                }
            }
        });
    if crate::widgets::preset_arrow(ui, false, salt).clicked() {
        chosen = Some(step(1));
    }
    if let Some(preset) = chosen.and_then(|i| list.get(i)) {
        let changes: Vec<_> = preset
            .resolve(specs)
            .into_iter()
            .filter(|(name, value)| (effect.value(name) - value).abs() > 1e-6)
            .collect();
        if !changes.is_empty() {
            edits.push(Edit::Set(changes));
        }
    }
}

/// Whether parameter `index` of `kind` has a knob. The rest are edited in the
/// graph, by clicking the steps or dragging the points.
#[must_use]
pub fn has_knob(kind: EffectKind, index: usize) -> bool {
    let Some(spec) = kind.params().get(index) else {
        return false;
    };
    let name = spec.name;
    let drawn_beat = kind == EffectKind::BeatShaper
        && ((name.starts_with("time_") && name != "time_mode") || name.starts_with("volume_"));
    !(name == "steps" || name.starts_with("point_") || name.starts_with("step_") || drawn_beat)
}

/// An effect's knobs (the ones its graph does not edit), except those
/// `hidden` says to leave out. `on_knob` sees each knob's response (the app
/// adds its automation menu there).
pub fn knob_row(
    ui: &mut egui::Ui,
    effect: &EffectView,
    hidden: &dyn Fn(usize) -> bool,
    edits: &mut Vec<Edit>,
    on_knob: &mut dyn FnMut(&egui::Response, &ParamSpec),
) {
    ui.horizontal_wrapped(|ui| {
        for (p, spec) in effect.kind.params().iter().enumerate() {
            let value = effect.values[p];
            if hidden(p) || !has_knob(effect.kind, p) {
                continue;
            }
            let text = musiccraft_dsp::describe::text(effect.kind, spec, value);
            let label = pretty(spec.name);
            let out = widgets::stepped_labeled_knob(
                ui,
                &label,
                spec.to_unit(value),
                spec.to_unit(spec.default),
                spec.min < 0.0 && spec.max > 0.0,
                &text,
                spec.steps(),
            );
            on_knob(&out.response, spec);
            if out.started {
                edits.push(Edit::Begin);
            }
            if let Some(t) = out.changed {
                let new = spec.from_unit(t);
                if new != value {
                    edits.push(Edit::Drag(vec![(spec.name, new)]));
                }
            }
        }
    });
}

/// The tuner display, for the tuner and the pitch corrector.
pub fn tuner(ui: &mut egui::Ui, effect: &EffectView, detected: Option<f32>) {
    crate::tuner::show(ui, effect, detected, GRAPH);
}

/// The whole editor for a standalone effect (a plugin): the preset bar, the
/// graph (or the tuner, with the pitch last `detected`) and the knobs, without
/// routing parameters.
pub fn editor(
    ui: &mut egui::Ui,
    id: egui::Id,
    effect: &EffectView,
    sample_rate: f32,
    detected: Option<f32>,
    edits: &mut Vec<Edit>,
) {
    preset_bar(ui, id, effect, edits);
    ui.add_space(4.0);
    if matches!(effect.kind, EffectKind::PitchCorrect | EffectKind::Tuner) {
        tuner(ui, effect, detected);
    } else {
        graph(ui, id, effect, sample_rate, edits);
    }
    ui.add_space(4.0);
    let kind = effect.kind;
    knob_row(
        ui,
        effect,
        &|p| is_routing(kind, p),
        edits,
        &mut |_: &egui::Response, _: &ParamSpec| {},
    );
}

/// The stereo shaper's matrix: a line from each input to each output whose
/// thickness is the gain (red when inverted), and the delayed side marked.
fn stereo_shaper_graph(painter: &egui::Painter, rect: Rect, color: Color32, effect: &EffectView) {
    let inputs = [
        pos2(rect.left() + rect.width() * 0.25, rect.top() + 18.0),
        pos2(rect.left() + rect.width() * 0.75, rect.top() + 18.0),
    ];
    let outputs = [
        pos2(inputs[0].x, rect.bottom() - 18.0),
        pos2(inputs[1].x, rect.bottom() - 18.0),
    ];
    let routes = [
        (0, 0, "left_to_left"),
        (1, 0, "right_to_left"),
        (0, 1, "left_to_right"),
        (1, 1, "right_to_right"),
    ];
    for (from, to, name) in routes {
        let gain = value(effect, name);
        if gain.abs() < 0.01 {
            continue;
        }
        let tint = if gain < 0.0 { theme::DANGER } else { color };
        painter.line_segment(
            [inputs[from], outputs[to]],
            Stroke::new(1.0 + gain.abs() * 4.0, tint),
        );
    }
    let delay = value(effect, "delay");
    for (i, label) in ["L", "R"].into_iter().enumerate() {
        let font = egui::FontId::proportional(theme::FONT_LABEL);
        painter.text(
            inputs[i],
            egui::Align2::CENTER_BOTTOM,
            label,
            font.clone(),
            theme::TEXT,
        );
        let late = (i == 1 && delay > 0.0) || (i == 0 && delay < 0.0);
        let text = if late {
            format!("{label} +{:.1} ms", delay.abs())
        } else {
            label.to_owned()
        };
        painter.text(
            outputs[i],
            egui::Align2::CENTER_TOP,
            text,
            font,
            theme::TEXT,
        );
    }
}

/// The beat shaper's time strip (top) and volume strip (bottom).
fn beat_strips(rect: Rect) -> (Rect, Rect) {
    let half = rect.height() * 0.5;
    (
        Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + half - 2.0)),
        Rect::from_min_max(pos2(rect.left(), rect.top() + half + 2.0), rect.max),
    )
}

/// A row of boxes joined by arrows, one per stage of a chain; stages that
/// are on are filled.
fn chain_diagram(painter: &egui::Painter, rect: Rect, color: Color32, stages: &[(&str, bool)]) {
    let n = stages.len().max(1) as f32;
    let cell = rect.width() / n;
    let mid = rect.center().y;
    for (i, (name, on)) in stages.iter().enumerate() {
        let left = rect.left() + cell * i as f32;
        let block = Rect::from_center_size(pos2(left + cell * 0.5, mid), vec2(cell * 0.7, 34.0));
        if *on {
            painter.rect_filled(block, 4.0, color.gamma_multiply(0.7));
        }
        painter.rect_stroke(
            block,
            4.0,
            Stroke::new(1.0, color),
            egui::StrokeKind::Inside,
        );
        painter.text(
            block.center(),
            egui::Align2::CENTER_CENTER,
            *name,
            egui::FontId::proportional(theme::FONT_LABEL),
            if *on {
                theme::BACKGROUND
            } else {
                theme::TEXT_DIM
            },
        );
        if i + 1 < stages.len() {
            let from = pos2(block.right(), mid);
            let to = pos2(left + cell * 1.15, mid);
            painter.line_segment([from, to], Stroke::new(1.0, theme::TEXT_DIM));
        }
    }
}

/// The band at the bottom of the auto filter's graph that shows its LFO.
fn lfo_strip_rect(rect: Rect) -> Rect {
    Rect::from_min_max(
        pos2(rect.left(), rect.bottom() - rect.height() * 0.3),
        rect.max,
    )
}

/// The LFO level (−1–1) at height `y` in the LFO strip.
fn lfo_strip_value(rect: Rect, y: f32) -> f32 {
    let strip = lfo_strip_rect(rect).shrink2(vec2(0.0, 4.0));
    (1.0 - 2.0 * (y - strip.top()) / strip.height().max(1.0)).clamp(-1.0, 1.0)
}

/// Draws one cycle of the auto filter's LFO, with the drawn shape's points as
/// handles.
fn lfo_strip(
    painter: &egui::Painter,
    rect: Rect,
    color: Color32,
    effect: &EffectView,
    handles: &mut Vec<(Pos2, Option<&'static str>, Option<&'static str>)>,
) {
    let strip = lfo_strip_rect(rect);
    painter.rect_filled(strip, 0.0, theme::PANEL_DARK.gamma_multiply(0.85));
    let inner = strip.shrink2(vec2(0.0, 4.0));
    let y = |v: f32| inner.center().y - v * inner.height() * 0.5;
    let shape = value(effect, "lfo_shape");
    let steps: Vec<f32> = (0..LFO_STEPS)
        .map(|k| {
            effect
                .values
                .get(LFO_FIRST_STEP + k)
                .copied()
                .unwrap_or(0.0)
        })
        .collect();
    let width = strip.width().max(1.0) as usize;
    let points: Vec<Pos2> = (0..=width)
        .map(|i| {
            let phase = i as f32 / width as f32;
            let v = musiccraft_dsp::character::lfo_shape(shape, &steps, phase);
            pos2(strip.left() + i as f32, y(v))
        })
        .collect();
    for pair in points.windows(2) {
        painter.line_segment(
            [pair[0], pair[1]],
            Stroke::new(1.5, color.gamma_multiply(0.8)),
        );
    }
    if shape.round() as usize == LFO_SHAPES.len() - 1 {
        for (k, v) in steps.iter().enumerate() {
            let name = effect.kind.params()[LFO_FIRST_STEP + k].name;
            let x = strip.left() + strip.width() * k as f32 / LFO_STEPS as f32;
            handles.push((pos2(x, y(*v)), None, Some(name)));
        }
    }
}

/// Whether a waveshaper slot uses its drawn curve.
fn drawn_shape(effect: &EffectView) -> bool {
    effect.kind == EffectKind::Waveshaper
        && value(effect, "shape").round() as i32 == musiccraft_dsp::character::DRAWN_SHAPE
}

/// A parameter name for display: `low_gain` → `Low gain`.
/// Long labels lose their tail under a knob; these words shorten them.
const ABBREVIATIONS: [(&str, &str); 5] = [
    ("threshold", "thr"),
    ("release", "rel"),
    ("attack", "att"),
    ("ceiling", "ceil"),
    ("makeup", "gain"),
];

/// Labels longer than this are abbreviated.
const LABEL_CHARS: usize = 10;

pub fn pretty(name: &str) -> String {
    // `left_to_right` style routing names read best as arrows.
    if let Some((from, to)) = name.split_once("_to_") {
        let side = |s: &str| match s {
            "left" => Some("L"),
            "right" => Some("R"),
            _ => None,
        };
        if let (Some(a), Some(b)) = (side(from), side(to)) {
            return format!("{a} → {b}");
        }
    }
    // `stage2_block` → `S2 block`.
    // `branch3_pan` → `B3 pan`.
    let name = match name
        .strip_prefix("stage")
        .or_else(|| name.strip_prefix("branch"))
    {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => {
            let letter = if name.starts_with('s') { 's' } else { 'b' };
            format!("{letter}{rest}")
        }
        _ => name.to_owned(),
    };
    let mut text = name.replace('_', " ");
    if text.len() > LABEL_CHARS {
        for (word, short) in ABBREVIATIONS {
            text = text.replace(word, short);
        }
    }
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text
}

// --- graphs ---------------------------------------------------------------

fn freq_x(rect: Rect, f: f32) -> f32 {
    rect.left() + rect.width() * (f.max(F_LOW) / F_LOW).ln() / (F_HIGH / F_LOW).ln()
}

fn x_freq(rect: Rect, x: f32) -> f32 {
    let t = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    F_LOW * (F_HIGH / F_LOW).powf(t)
}

fn db_y(rect: Rect, db: f32, top: f32, bottom: f32) -> f32 {
    let t = ((top - db) / (top - bottom)).clamp(0.0, 1.0);
    rect.top() + rect.height() * t
}

fn y_db(rect: Rect, y: f32, top: f32, bottom: f32) -> f32 {
    let t = ((y - rect.top()) / rect.height()).clamp(0.0, 1.0);
    top - t * (top - bottom)
}

fn value(effect: &EffectView, name: &str) -> f32 {
    let specs = effect.kind.params();
    index_of(specs, name)
        .and_then(|i| effect.values.get(i).copied())
        .unwrap_or(0.0)
}

fn spec(effect: &EffectView, name: &str) -> Option<ParamSpec> {
    let specs = effect.kind.params();
    index_of(specs, name).and_then(|i| specs.get(i).copied())
}

fn frequency_grid(painter: &egui::Painter, rect: Rect) {
    for f in [
        50.0, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
    ] {
        let x = freq_x(rect, f);
        painter.line_segment(
            [pos2(x, rect.top()), pos2(x, rect.bottom())],
            Stroke::new(1.0, theme::GRID_LINE),
        );
        let label = if f >= 1_000.0 {
            format!("{}k", f / 1_000.0)
        } else {
            format!("{f}")
        };
        painter.text(
            pos2(x + 2.0, rect.bottom() - 2.0),
            egui::Align2::LEFT_BOTTOM,
            label,
            egui::FontId::proportional(theme::FONT_CAPTION),
            theme::TEXT_DIM,
        );
    }
}

fn curve(painter: &egui::Painter, rect: Rect, color: Color32, points: Vec<Pos2>) {
    for pair in points.windows(2) {
        painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, color));
    }
    // A soft area under the curve.
    for pair in points.windows(2) {
        let quad = vec![
            pair[0],
            pair[1],
            pos2(pair[1].x, rect.bottom()),
            pos2(pair[0].x, rect.bottom()),
        ];
        painter.add(egui::Shape::convex_polygon(
            quad,
            color.gamma_multiply(0.12),
            Stroke::NONE,
        ));
    }
}

fn handle(painter: &egui::Painter, at: Pos2, active: bool, color: Color32) {
    painter.circle_filled(at, if active { 7.0 } else { 5.5 }, color);
    painter.circle_stroke(at, 7.5, Stroke::new(1.0, theme::BACKGROUND));
}

/// Draws the graph for `effect` and turns drags into parameter changes.
pub fn graph(
    ui: &mut egui::Ui,
    id: egui::Id,
    effect: &EffectView,
    sample_rate: f32,
    edits: &mut Vec<Edit>,
) -> Rect {
    let (rect, response) = ui.allocate_exact_size(GRAPH, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme::LCD_BG);
    let color = if effect.enabled {
        theme::ACCENT
    } else {
        theme::TEXT_DIM
    };
    let width = rect.width() as usize;
    // Handles: (position, parameters it moves along x and y).
    let mut handles: Vec<(Pos2, Option<&'static str>, Option<&'static str>)> = Vec::new();
    let mut hint = "";
    match effect.kind {
        EffectKind::Filter | EffectKind::Eq => {
            frequency_grid(&painter, rect);
            let (top, bottom) = if effect.kind == EffectKind::Eq {
                (18.0, -18.0)
            } else {
                (24.0, -48.0)
            };
            let zero = db_y(rect, 0.0, top, bottom);
            painter.line_segment(
                [pos2(rect.left(), zero), pos2(rect.right(), zero)],
                Stroke::new(1.0, theme::GRID_BEAT),
            );
            let db_at = |f: f32| -> f32 {
                if effect.kind == EffectKind::Eq {
                    response::eq_db(&effect.values, f, sample_rate)
                } else {
                    let mode = SvfMode::from_param(value(effect, "mode"));
                    let mix = value(effect, "mix");
                    let wet = musiccraft_dsp::db_to_gain(response::svf_db(
                        mode,
                        value(effect, "cutoff"),
                        value(effect, "resonance"),
                        f,
                        sample_rate,
                    ));
                    // Dry and wet are mixed in phase for display.
                    musiccraft_dsp::gain_to_db(1.0 - mix + wet * mix)
                }
            };
            let points = (0..=width)
                .map(|i| {
                    let x = rect.left() + i as f32;
                    let f = x_freq(rect, x);
                    pos2(x, db_y(rect, db_at(f), top, bottom))
                })
                .collect();
            curve(&painter, rect, color, points);
            if effect.kind == EffectKind::Eq {
                for (freq, gain) in [
                    ("low_freq", "low_gain"),
                    ("mid_freq", "mid_gain"),
                    ("high_freq", "high_gain"),
                ] {
                    let at = pos2(
                        freq_x(rect, value(effect, freq)),
                        db_y(rect, value(effect, gain), top, bottom),
                    );
                    handles.push((at, Some(freq), Some(gain)));
                }
                hint = "Drag a point to set a band's frequency and gain; scroll on the middle point for its width";
            } else {
                let res = value(effect, "resonance");
                let at = pos2(
                    freq_x(rect, value(effect, "cutoff")),
                    rect.bottom() - rect.height() * (0.15 + 0.7 * res),
                );
                handles.push((at, Some("cutoff"), Some("resonance")));
                hint = "Drag the point: left/right sets the cutoff, up/down the resonance";
            }
        }
        EffectKind::Compressor
        | EffectKind::Limiter
        | EffectKind::NoiseGate
        | EffectKind::Dynamics => {
            let (top, bottom) = (6.0, -60.0);
            let map = |db_in: f32, db_out: f32| {
                pos2(
                    rect.left() + rect.width() * (db_in - bottom) / (top - bottom),
                    db_y(rect, db_out, top, bottom),
                )
            };
            for db in [-48.0, -36.0, -24.0, -12.0, 0.0] {
                let a = map(db, bottom);
                painter.line_segment(
                    [pos2(a.x, rect.top()), pos2(a.x, rect.bottom())],
                    Stroke::new(1.0, theme::GRID_LINE),
                );
                let y = db_y(rect, db, top, bottom);
                painter.line_segment(
                    [pos2(rect.left(), y), pos2(rect.right(), y)],
                    Stroke::new(1.0, theme::GRID_LINE),
                );
            }
            painter.line_segment(
                [map(bottom, bottom), map(top, top)],
                Stroke::new(1.0, theme::GRID_BEAT),
            );
            let transfer = |db: f32| {
                if effect.kind == EffectKind::Compressor {
                    response::compressor_db(&effect.values, db)
                } else if effect.kind == EffectKind::NoiseGate {
                    response::noise_gate_db(&effect.values, db).max(bottom)
                } else if effect.kind == EffectKind::Dynamics {
                    response::dynamics_db(&effect.values, db).max(bottom)
                } else {
                    response::limiter_db(&effect.values, db)
                }
            };
            let points = (0..=width)
                .map(|i| {
                    let db_in = bottom + (top - bottom) * i as f32 / width as f32;
                    map(db_in, transfer(db_in))
                })
                .collect();
            curve(&painter, rect, color, points);
            if effect.kind == EffectKind::NoiseGate {
                let threshold = value(effect, "threshold");
                handles.push((map(threshold, threshold), Some("threshold"), None));
                hint = "Input level across, output level up: below the threshold the gate closes";
            } else if effect.kind == EffectKind::Compressor {
                let threshold = value(effect, "threshold");
                handles.push((map(threshold, transfer(threshold)), Some("threshold"), None));
                hint = "Input level across, output level up. Drag the knee to set the threshold";
            } else if effect.kind == EffectKind::Dynamics {
                let threshold = value(effect, "comp_threshold");
                handles.push((
                    map(threshold, transfer(threshold)),
                    Some("comp_threshold"),
                    None,
                ));
                let ceiling = value(effect, "limit_ceiling");
                handles.push((
                    pos2(rect.right() - 12.0, db_y(rect, ceiling, top, bottom)),
                    None,
                    Some("limit_ceiling"),
                ));
                hint = "Gate, compressor and limiter combined. Drag the knee for the compressor threshold, the right point for the ceiling";
            } else {
                let ceiling = value(effect, "ceiling");
                handles.push((
                    pos2(rect.right() - 12.0, db_y(rect, ceiling, top, bottom)),
                    None,
                    Some("ceiling"),
                ));
                hint = "Input level across, output level up. Drag the point to set the ceiling";
            }
        }
        EffectKind::Distortion
        | EffectKind::Overdrive
        | EffectKind::SoftClipper
        | EffectKind::Waveshaper => {
            let mid = rect.center();
            painter.line_segment(
                [pos2(rect.left(), mid.y), pos2(rect.right(), mid.y)],
                Stroke::new(1.0, theme::GRID_LINE),
            );
            painter.line_segment(
                [pos2(mid.x, rect.top()), pos2(mid.x, rect.bottom())],
                Stroke::new(1.0, theme::GRID_LINE),
            );
            let points: Vec<Pos2> = (0..=width)
                .map(|i| {
                    let x = i as f32 / width as f32 * 2.0 - 1.0;
                    // A drawn curve is shown as drawn, so its points sit on it;
                    // drive moves the input along it.
                    let y = if drawn_shape(effect) {
                        musiccraft_dsp::character::waveshaper_curve(&effect.values, x)
                    } else {
                        response::shaper(effect.kind, &effect.values, x).unwrap_or(x)
                    }
                    .clamp(-1.2, 1.2);
                    pos2(rect.left() + i as f32, mid.y - y * rect.height() * 0.4)
                })
                .collect();
            for pair in points.windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, color));
            }
            if drawn_shape(effect) {
                for k in 0..SHAPER_POINTS {
                    let name = effect.kind.params()[SHAPER_FIRST_POINT + k].name;
                    let x = rect.left() + rect.width() * k as f32 / (SHAPER_POINTS - 1) as f32;
                    let y = mid.y - value(effect, name) * rect.height() * 0.4;
                    handles.push((pos2(x, y), None, Some(name)));
                }
            }
            hint = match effect.kind {
                EffectKind::SoftClipper => {
                    "Input across, output up: straight below the threshold, bending smoothly into it"
                }
                EffectKind::Waveshaper if drawn_shape(effect) => {
                    "Input across, output up: drag a point up or down to draw the curve; drive pushes the input further along it"
                }
                EffectKind::Waveshaper => {
                    "Input across, output up: the shape picks the curve (pick \"drawn\" to draw your own)"
                }
                EffectKind::Overdrive => {
                    "Input across, output up: bias makes the curve lopsided (before the tone filter)"
                }
                _ => "Input across, output up: more drive bends the curve into clipping",
            };
        }
        EffectKind::Delay => {
            let time = value(effect, "time");
            let feedback = value(effect, "feedback");
            let ping_pong = value(effect, "ping_pong") >= 0.5;
            let span = (time * 8.0).max(1.0);
            for beat in 0..=(span.ceil() as i32) {
                let x = rect.left() + rect.width() * beat as f32 / span;
                painter.line_segment(
                    [pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(1.0, theme::GRID_LINE),
                );
            }
            let mid = rect.center().y;
            painter.line_segment(
                [pos2(rect.left(), mid), pos2(rect.right(), mid)],
                Stroke::new(1.0, theme::GRID_BEAT),
            );
            painter.line_segment(
                [
                    pos2(rect.left() + 3.0, mid),
                    pos2(rect.left() + 3.0, rect.top() + 6.0),
                ],
                Stroke::new(3.0, theme::TEXT),
            );
            // Echo heights relative to the first; mix is on its knob.
            let mut level = 1.0;
            for n in 1..=8 {
                let x = rect.left() + rect.width() * (n as f32 * time) / span;
                if x > rect.right() || level < 0.01 {
                    break;
                }
                let up = !ping_pong || n % 2 == 1;
                let h = level * (rect.height() * 0.45);
                let end = if up { mid - h } else { mid + h };
                painter.line_segment([pos2(x, mid), pos2(x, end)], Stroke::new(3.0, color));
                level *= feedback;
            }
            hint = "Echoes over time (grid lines are beats); left and right alternate in ping-pong";
        }
        EffectKind::Gate => {
            let duty = value(effect, "duty");
            let depth = value(effect, "depth");
            let steps = value(effect, "steps");
            let cell = rect.width() / 16.0;
            for i in 0..16 {
                let x = rect.left() + cell * i as f32;
                let r = Rect::from_min_max(
                    pos2(x + 1.0, rect.top() + 6.0),
                    pos2(x + cell - 1.0, rect.bottom() - 6.0),
                );
                let beat = if i % 4 == 0 {
                    theme::GRID_BAR
                } else {
                    theme::GRID_LINE
                };
                painter.rect_stroke(r, 2.0, Stroke::new(1.0, beat), egui::StrokeKind::Inside);
                if gate_step_on(steps, i) {
                    let open =
                        Rect::from_min_max(r.min, pos2(r.left() + r.width() * duty, r.bottom()));
                    painter.rect_filled(open, 2.0, color.gamma_multiply(0.8));
                } else if depth < 1.0 {
                    let floor = r.bottom() - r.height() * (1.0 - depth);
                    painter.rect_filled(
                        Rect::from_min_max(pos2(r.left(), floor), r.max),
                        2.0,
                        color.gamma_multiply(0.2),
                    );
                }
            }
            hint = "Click a step to turn it on or off; duty sets how much of each step is open";
        }
        EffectKind::Autopan => {
            let shape = value(effect, "shape");
            let depth = value(effect, "depth");
            let mid = rect.center().y;
            painter.line_segment(
                [pos2(rect.left(), mid), pos2(rect.right(), mid)],
                Stroke::new(1.0, theme::GRID_BEAT),
            );
            painter.text(
                pos2(rect.left() + 4.0, rect.top() + 4.0),
                egui::Align2::LEFT_TOP,
                "R",
                egui::FontId::proportional(theme::FONT_VALUE),
                theme::TEXT_DIM,
            );
            painter.text(
                pos2(rect.left() + 4.0, rect.bottom() - 4.0),
                egui::Align2::LEFT_BOTTOM,
                "L",
                egui::FontId::proportional(theme::FONT_VALUE),
                theme::TEXT_DIM,
            );
            let points: Vec<Pos2> = (0..=width)
                .map(|i| {
                    let phase = i as f64 / width as f64 * 2.0;
                    let pan = autopan_shape(shape, phase) * depth;
                    pos2(rect.left() + i as f32, mid - pan * rect.height() * 0.42)
                })
                .collect();
            for pair in points.windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, color));
            }
            hint = "Where the sound sits over two cycles: up is right, down is left";
        }
        EffectKind::Bitcrusher => {
            let bits = value(effect, "bits");
            let hold = value(effect, "downsample").max(1.0);
            let levels = 2.0_f32.powf(bits - 1.0);
            let mid = rect.center().y;
            let wave = |x: f32| (x / rect.width() * std::f32::consts::TAU * 2.0).sin();
            let smooth: Vec<Pos2> = (0..=width)
                .map(|i| {
                    pos2(
                        rect.left() + i as f32,
                        mid - wave(i as f32) * rect.height() * 0.4,
                    )
                })
                .collect();
            for pair in smooth.windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(1.0, theme::TEXT_DIM));
            }
            let px_per_sample = 1.5;
            let crushed: Vec<Pos2> = (0..=width)
                .map(|i| {
                    let held = ((i as f32 / px_per_sample / hold).floor() * hold * px_per_sample)
                        .min(rect.width());
                    let y = (wave(held) * levels).round() / levels;
                    pos2(rect.left() + i as f32, mid - y * rect.height() * 0.4)
                })
                .collect();
            for pair in crushed.windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, color));
            }
            hint = "A sine wave before (grey) and after crushing: fewer bits mean bigger steps";
        }
        EffectKind::Stereo => {
            let width_v = value(effect, "width");
            let balance = value(effect, "balance");
            let center = rect.center();
            let h = rect.height() * 0.42;
            for (side, invert) in [(-1.0_f32, "invert_left"), (1.0, "invert_right")] {
                let level = (1.0 + side * balance).min(1.0);
                let spread = (width_v / 2.0).min(1.0) * 0.8 + 0.1;
                let end = pos2(center.x + side * h * spread * 1.6, center.y - h * level);
                let stroke = if value(effect, invert) >= 0.5 {
                    Stroke::new(2.0, theme::DANGER)
                } else {
                    Stroke::new(2.0, color)
                };
                painter.line_segment([pos2(center.x, rect.bottom() - 6.0), end], stroke);
                painter.text(
                    end,
                    egui::Align2::CENTER_BOTTOM,
                    if side < 0.0 { "L" } else { "R" },
                    egui::FontId::proportional(theme::FONT_LABEL),
                    theme::TEXT,
                );
            }
            hint = "Lines show how far apart left and right sit; red means phase inverted";
        }
        EffectKind::StereoShaper => {
            stereo_shaper_graph(&painter, rect, color, effect);
            hint = "Where each output side's sound comes from (red is phase inverted); the delay shifts one side later";
        }
        EffectKind::Reverb | EffectKind::Chorus | EffectKind::Phaser | EffectKind::Flanger => {
            let points: Vec<Pos2> = if effect.kind == EffectKind::Reverb {
                let size = value(effect, "size");
                let damping = value(effect, "damping");
                let mix = value(effect, "mix");
                let decay = 0.4 + size * 4.0;
                (0..=width)
                    .map(|i| {
                        let t = i as f32 / width as f32 * 5.0;
                        let level = mix.max(0.05) * (-t / decay * (1.0 + damping)).exp();
                        pos2(
                            rect.left() + i as f32,
                            rect.bottom() - level * rect.height() * 0.9,
                        )
                    })
                    .collect()
            } else {
                let rate = value(effect, "rate");
                let depth = value(effect, "depth")
                    / spec(effect, "depth").map_or(10.0, |s| s.max.max(1e-3))
                    * 0.9;
                (0..=width)
                    .map(|i| {
                        let t = i as f32 / width as f32 * 2.0;
                        let y = (t * rate * std::f32::consts::TAU).sin() * depth;
                        pos2(
                            rect.left() + i as f32,
                            rect.center().y - y * rect.height() * 0.4,
                        )
                    })
                    .collect()
            };
            curve(&painter, rect, color, points);
            hint = if effect.kind == EffectKind::Reverb {
                "The tail over five seconds: size lengthens it, damping darkens and shortens it"
            } else {
                "The sweep over two seconds: rate is speed, depth is height"
            };
        }
        EffectKind::HallReverb => {
            let predelay = value(effect, "predelay") * 0.001;
            let decay = value(effect, "decay").max(0.1);
            let early = value(effect, "early");
            let span = (decay * 1.5).clamp(1.0, 20.0) + predelay;
            // A square-root time axis leaves room for the early reflections.
            let x = |t: f32| rect.left() + rect.width() * (t / span).max(0.0).sqrt();
            let level_y = |level: f32| rect.bottom() - level * rect.height() * 0.9;
            for (i, ms) in [7.1_f32, 11.3, 17.9, 23.3, 31.7, 39.1].iter().enumerate() {
                let scale = 0.5 + value(effect, "size");
                let t = predelay + ms * scale * 0.001;
                let level = early * (0.84 - i as f32 * 0.1);
                painter.line_segment(
                    [pos2(x(t), rect.bottom()), pos2(x(t), level_y(level))],
                    Stroke::new(2.0, theme::HIGHLIGHT.gamma_multiply(0.8)),
                );
            }
            // A 60 dB fall over `decay` seconds, drawn on a 60 dB scale.
            let points = (0..=width)
                .map(|i| {
                    let t = span * (i as f32 / width as f32).powi(2);
                    let db = if t < predelay {
                        -60.0
                    } else {
                        -60.0 * (t - predelay) / decay
                    };
                    pos2(
                        rect.left() + i as f32,
                        level_y(((db + 60.0) / 60.0).max(0.0)),
                    )
                })
                .collect();
            curve(&painter, rect, color, points);
            hint = "Early reflections (orange) after the predelay, then the tail falling 60 dB over the decay time (time stretched near the start)";
        }
        EffectKind::GraphicEq
        | EffectKind::ParametricEq
        | EffectKind::BassBoost
        | EffectKind::DcBlock
        | EffectKind::Formant
        | EffectKind::AutoFilter => {
            frequency_grid(&painter, rect);
            let (top, bottom) = db_range(effect.kind);
            let zero = db_y(rect, 0.0, top, bottom);
            painter.line_segment(
                [pos2(rect.left(), zero), pos2(rect.right(), zero)],
                Stroke::new(1.0, theme::GRID_BEAT),
            );
            let points = (0..=width)
                .map(|i| {
                    let x = rect.left() + i as f32;
                    let db = response::effect_db(
                        effect.kind,
                        &effect.values,
                        x_freq(rect, x),
                        sample_rate,
                    )
                    .unwrap_or(0.0);
                    pos2(x, db_y(rect, db, top, bottom))
                })
                .collect();
            curve(&painter, rect, color, points);
            let names = effect.kind.params();
            let name = |i: usize| names.get(i).map(|s| s.name);
            match effect.kind {
                EffectKind::GraphicEq => {
                    for (i, f) in GRAPHIC_BANDS.iter().enumerate() {
                        if let Some(n) = name(i) {
                            let at =
                                pos2(freq_x(rect, *f), db_y(rect, value(effect, n), top, bottom));
                            handles.push((at, None, Some(n)));
                        }
                    }
                    hint = "Drag a band's point up or down to boost or cut it";
                }
                EffectKind::ParametricEq => {
                    for band in 0..7 {
                        if let (Some(f), Some(g)) = (name(band * 3), name(band * 3 + 1)) {
                            let at = pos2(
                                freq_x(rect, value(effect, f)),
                                db_y(rect, value(effect, g), top, bottom),
                            );
                            handles.push((at, Some(f), Some(g)));
                        }
                    }
                    hint = "Drag a point to set a band's frequency and gain; band 1 is a low shelf, band 7 a high shelf";
                }
                EffectKind::BassBoost => {
                    let at = pos2(
                        freq_x(rect, value(effect, "freq")),
                        db_y(rect, value(effect, "boost") * 0.5, top, bottom),
                    );
                    handles.push((at, Some("freq"), Some("boost")));
                    hint = "Drag the point: left/right sets the frequency, up/down the boost";
                }
                EffectKind::DcBlock => {
                    let at = pos2(
                        freq_x(rect, value(effect, "freq")),
                        db_y(rect, -3.0, top, bottom),
                    );
                    handles.push((at, Some("freq"), None));
                    hint = "Everything below the point is removed; drag it left or right";
                }
                EffectKind::AutoFilter => {
                    let res = value(effect, "resonance");
                    let at = pos2(
                        freq_x(rect, value(effect, "cutoff")),
                        rect.bottom() - rect.height() * (0.15 + 0.7 * res),
                    );
                    handles.push((at, Some("cutoff"), Some("resonance")));
                    lfo_strip(&painter, rect, color, effect, &mut handles);
                    hint = if value(effect, "lfo_shape").round() as usize == LFO_SHAPES.len() - 1 {
                        "The filter at rest, and below it one cycle of the drawn LFO: drag its points up or down"
                    } else {
                        "The filter at rest; the LFO (shape below) and the input level sweep it from here. Drag to set cutoff and resonance"
                    };
                }
                _ => hint = "The vowel's three formant peaks; the vowel knob morphs a-e-i-o-u",
            }
        }
        EffectKind::MultiTap => {
            for beat in 0..=(TAP_SPAN as i32) {
                let x = rect.left() + rect.width() * beat as f32 / TAP_SPAN;
                painter.line_segment(
                    [pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(1.0, theme::GRID_LINE),
                );
            }
            painter.line_segment(
                [
                    pos2(rect.left() + 3.0, rect.bottom() - 6.0),
                    pos2(rect.left() + 3.0, rect.top() + 6.0),
                ],
                Stroke::new(3.0, theme::TEXT),
            );
            for tap in 1..=4 {
                let (time, level) = (format!("tap{tap}_time"), format!("tap{tap}_level"));
                let x = rect.left() + rect.width() * value(effect, &time) / TAP_SPAN;
                let y = rect.bottom() - 6.0 - (rect.height() - 12.0) * value(effect, &level);
                painter.line_segment(
                    [pos2(x, rect.bottom() - 6.0), pos2(x, y)],
                    Stroke::new(3.0, color),
                );
                painter.text(
                    pos2(x + 4.0, y),
                    egui::Align2::LEFT_TOP,
                    crate::format::pan(value(effect, &format!("tap{tap}_pan"))),
                    egui::FontId::proportional(theme::FONT_VALUE),
                    theme::TEXT_DIM,
                );
                let names = effect.kind.params();
                let t = names.get((tap - 1) * 2).map(|s| s.name);
                let l = names.get((tap - 1) * 2 + 1).map(|s| s.name);
                handles.push((pos2(x, y), t, l));
            }
            hint = "Four echoes over four beats: drag a tap sideways for its time, up or down for its level; each tap's pan and filters are on the knobs";
        }
        EffectKind::RingMod | EffectKind::FreqShift => {
            frequency_grid(&painter, rect);
            let input = 440.0;
            let mix = value(effect, "mix");
            let outputs: Vec<f32> = if effect.kind == EffectKind::RingMod {
                let f = value(effect, "freq");
                vec![input + f, (input - f).abs()]
            } else {
                vec![(input + value(effect, "shift")).abs()]
            };
            let bar = |f: f32, height: f32, c: Color32| {
                let x = freq_x(rect, f.clamp(F_LOW, F_HIGH));
                painter.line_segment(
                    [
                        pos2(x, rect.bottom() - 4.0),
                        pos2(x, rect.bottom() - 4.0 - height),
                    ],
                    Stroke::new(3.0, c),
                );
            };
            let h = rect.height() * 0.8;
            bar(input, h * (1.0 - mix).max(0.08), theme::TEXT_DIM);
            for f in outputs {
                bar(f, h * mix.max(0.08), color);
            }
            hint = if effect.kind == EffectKind::RingMod {
                "A 440 Hz tone (grey) becomes the sum and difference of it and the carrier"
            } else {
                "A 440 Hz tone (grey) moves by the shift; harmonics move by the same hertz, so they stop being harmonic"
            };
        }
        EffectKind::Transient => {
            let (before, after) = response::transient_envelopes(&effect.values, width + 1);
            let peak = 1.6;
            let to_points = |v: &[f32]| -> Vec<Pos2> {
                v.iter()
                    .enumerate()
                    .map(|(i, level)| {
                        pos2(
                            rect.left() + i as f32,
                            rect.bottom() - 4.0 - (level / peak).min(1.0) * (rect.height() - 8.0),
                        )
                    })
                    .collect()
            };
            for pair in to_points(&before).windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(1.0, theme::TEXT_DIM));
            }
            for pair in to_points(&after).windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, color));
            }
            hint = "A drum hit's level before (grey) and after: attack shapes the front, sustain the tail";
        }
        EffectKind::Multiband => {
            frequency_grid(&painter, rect);
            let (f1, f2) = (value(effect, "low_freq"), value(effect, "high_freq"));
            let (top, bottom) = db_range(EffectKind::GraphicEq);
            let edges = [F_LOW, f1, f2, F_HIGH];
            for (band, name) in ["low", "mid", "high"].iter().enumerate() {
                let (a, b) = (freq_x(rect, edges[band]), freq_x(rect, edges[band + 1]));
                let threshold = value(effect, &format!("{name}_thresh"));
                let ratio = value(effect, &format!("{name}_ratio"));
                // Darker bands compress harder (at −6 dB input).
                let squeeze = -musiccraft_dsp::creative::band_gain_db(-6.0, threshold, ratio);
                let fill = color.gamma_multiply((0.1 + squeeze / 40.0).clamp(0.1, 0.6));
                painter.rect_filled(
                    Rect::from_min_max(pos2(a, rect.top()), pos2(b, rect.bottom())),
                    0.0,
                    fill,
                );
                let gain = db_y(rect, value(effect, &format!("{name}_gain")), top, bottom);
                painter.line_segment([pos2(a, gain), pos2(b, gain)], Stroke::new(2.0, color));
                painter.text(
                    pos2((a + b) * 0.5, rect.top() + 4.0),
                    egui::Align2::CENTER_TOP,
                    format!("{name} {threshold:.0} dB {ratio:.1}:1"),
                    egui::FontId::proportional(theme::FONT_VALUE),
                    theme::TEXT,
                );
            }
            let mid = rect.center().y;
            handles.push((pos2(freq_x(rect, f1), mid), Some("low_freq"), None));
            handles.push((pos2(freq_x(rect, f2), mid), Some("high_freq"), None));
            hint = "Three bands with their own compressors; drag the points to move the crossovers. Lines show each band's gain; brighter shading compresses harder";
        }
        EffectKind::BandSplit => {
            frequency_grid(&painter, rect);
            let (f1, f2) = (value(effect, "low_freq"), value(effect, "high_freq"));
            let edges = [F_LOW, f1, f2, F_HIGH];
            for (band, name) in ["low", "mid", "high"].iter().enumerate() {
                let (a, b) = (freq_x(rect, edges[band]), freq_x(rect, edges[band + 1]));
                let target = value(effect, &format!("{name}_target")).round();
                let fill = if target >= 0.5 { 0.45 } else { 0.12 };
                painter.rect_filled(
                    Rect::from_min_max(pos2(a, rect.top()), pos2(b, rect.bottom())),
                    0.0,
                    color.gamma_multiply(fill),
                );
                let text = if target >= 0.5 {
                    format!("{name} → insert {target:.0}")
                } else {
                    format!("{name} stays")
                };
                painter.text(
                    pos2((a + b) * 0.5, rect.top() + 4.0),
                    egui::Align2::CENTER_TOP,
                    text,
                    egui::FontId::proportional(theme::FONT_VALUE),
                    theme::TEXT,
                );
            }
            let mid = rect.center().y;
            handles.push((pos2(freq_x(rect, f1), mid), Some("low_freq"), None));
            handles.push((pos2(freq_x(rect, f2), mid), Some("high_freq"), None));
            hint = "Three bands; a band with a target insert leaves this track for it. Drag the points to move the crossovers";
        }
        EffectKind::MultibandLimiter => {
            frequency_grid(&painter, rect);
            let (f1, f2) = (value(effect, "low_freq"), value(effect, "high_freq"));
            let (top, bottom) = (6.0, -24.0);
            let edges = [F_LOW, f1, f2, F_HIGH];
            let amount = value(effect, "amount");
            for (band, name) in ["low", "mid", "high"].iter().enumerate() {
                let (a, b) = (freq_x(rect, edges[band]), freq_x(rect, edges[band + 1]));
                let drive = value(effect, &format!("{name}_drive")) * amount;
                let fill = color.gamma_multiply((0.1 + drive / 40.0).clamp(0.1, 0.7));
                painter.rect_filled(
                    Rect::from_min_max(pos2(a, rect.top()), pos2(b, rect.bottom())),
                    0.0,
                    fill,
                );
                let ceiling = db_y(rect, value(effect, &format!("{name}_ceiling")), top, bottom);
                painter.line_segment(
                    [pos2(a, ceiling), pos2(b, ceiling)],
                    Stroke::new(2.0, color),
                );
                painter.text(
                    pos2((a + b) * 0.5, rect.top() + 4.0),
                    egui::Align2::CENTER_TOP,
                    format!("{name} +{drive:.0} dB"),
                    egui::FontId::proportional(theme::FONT_VALUE),
                    theme::TEXT,
                );
            }
            let master = db_y(rect, value(effect, "ceiling"), top, bottom);
            painter.line_segment(
                [pos2(rect.left(), master), pos2(rect.right(), master)],
                Stroke::new(1.0, theme::DANGER),
            );
            let mid = rect.center().y;
            handles.push((pos2(freq_x(rect, f1), mid), Some("low_freq"), None));
            handles.push((pos2(freq_x(rect, f2), mid), Some("high_freq"), None));
            hint = "Three bands, each driven into its own ceiling (lines); brighter shading drives harder. The red line is the master ceiling; drag the points to move the crossovers";
        }
        EffectKind::Vocoder => {
            frequency_grid(&painter, rect);
            let shift = 2.0_f32.powf(value(effect, "shift") / 12.0);
            let (bands, low, high) = musiccraft_dsp::creative::vocoder_layout(&effect.values);
            for f in (0..bands).map(|i| musiccraft_dsp::creative::vocoder_freq(i, bands, low, high))
            {
                let x = freq_x(rect, f);
                let xs = freq_x(rect, (f * shift).min(F_HIGH));
                painter.line_segment(
                    [pos2(x, rect.bottom() - 4.0), pos2(x, rect.center().y)],
                    Stroke::new(2.0, theme::TEXT_DIM),
                );
                painter.line_segment(
                    [pos2(xs, rect.center().y), pos2(xs, rect.top() + 16.0)],
                    Stroke::new(2.0, color),
                );
            }
            let source = value(effect, "sidechain");
            painter.text(
                pos2(rect.left() + 4.0, rect.top() + 2.0),
                egui::Align2::LEFT_TOP,
                if source < 0.5 {
                    "No modulator: set Sidechain to the insert with the voice".to_owned()
                } else {
                    format!("Modulator: insert {source:.0}")
                },
                egui::FontId::proportional(theme::FONT_LABEL),
                theme::TEXT,
            );
            hint = "16 bands: the modulator's levels (grey, from the sidechain) shape the carrier on this insert (colour)";
        }
        EffectKind::PitchShift => {
            frequency_grid(&painter, rect);
            let ratio =
                2.0_f32.powf((value(effect, "pitch") + value(effect, "fine") / 100.0) / 12.0);
            let mix = value(effect, "mix");
            let h = rect.height() * 0.8;
            for (k, f) in [220.0_f32, 440.0, 660.0].iter().enumerate() {
                let level = 1.0 / (k as f32 + 1.0);
                for (freq, height, c) in [
                    (*f, h * level * (1.0 - mix).max(0.08), theme::TEXT_DIM),
                    (*f * ratio, h * level * mix.max(0.08), color),
                ] {
                    let x = freq_x(rect, freq.clamp(F_LOW, F_HIGH));
                    painter.line_segment(
                        [
                            pos2(x, rect.bottom() - 4.0),
                            pos2(x, rect.bottom() - 4.0 - height),
                        ],
                        Stroke::new(3.0, c),
                    );
                }
            }
            hint = "A 220 Hz note and its harmonics (grey) and where they end up (colour)";
        }
        EffectKind::Stutter => {
            let names = musiccraft_dsp::modulation::SYNC_RATES;
            let pick = |name: &str| {
                names
                    .get(value(effect, name).round().max(0.0) as usize)
                    .copied()
                    .unwrap_or(1.0) as f32
            };
            let every = pick("every").max(pick("slice"));
            let slice = pick("slice");
            let portion = value(effect, "portion");
            let start = rect.left() + rect.width() * (1.0 - portion);
            let beats = every.ceil() as i32;
            for beat in 0..=beats {
                let x = rect.left() + rect.width() * beat as f32 / every;
                painter.line_segment(
                    [pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(1.0, theme::GRID_LINE),
                );
            }
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(start, rect.top() + 6.0),
                    pos2(rect.right(), rect.bottom() - 6.0),
                ),
                2.0,
                color.gamma_multiply(0.25),
            );
            let width_slice = rect.width() * slice / every;
            let decay = value(effect, "decay");
            let mut x = start;
            let mut level = 1.0;
            while x < rect.right() - 0.5 && width_slice > 1.0 {
                let end = (x + width_slice).min(rect.right());
                let top = rect.bottom() - 6.0 - (rect.height() - 12.0) * level;
                painter.rect_filled(
                    Rect::from_min_max(pos2(x + 1.0, top), pos2(end - 1.0, rect.bottom() - 6.0)),
                    1.0,
                    color.gamma_multiply(0.8),
                );
                x = end;
                level *= decay;
            }
            hint = "One period (grid lines are beats): the shaded end repeats the slice that starts it";
        }
        EffectKind::Convolution => {
            let file = effect.file.as_deref().unwrap_or("ir:hall");
            let rt60 = match file {
                "ir:room" => 0.6,
                "ir:chamber" => 1.1,
                "ir:plate" => 1.6,
                "ir:cathedral" => 4.5,
                _ => 2.4,
            };
            let span = 5.0;
            let predelay = value(effect, "predelay") / 1_000.0;
            let mix = value(effect, "mix");
            for second in 0..=5 {
                let x = rect.left() + rect.width() * second as f32 / span;
                painter.line_segment(
                    [pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(1.0, theme::GRID_LINE),
                );
            }
            let points = (0..=width)
                .map(|i| {
                    let t = i as f32 / width as f32 * span - predelay;
                    let level = if t < 0.0 {
                        0.0
                    } else {
                        mix.max(0.05) * (-6.9 * t / rt60).exp()
                    };
                    pos2(
                        rect.left() + i as f32,
                        rect.bottom() - level * rect.height() * 0.9,
                    )
                })
                .collect();
            curve(&painter, rect, color, points);
            if !file.starts_with("ir:") {
                painter.text(
                    pos2(rect.right() - 4.0, rect.top() + 2.0),
                    egui::Align2::RIGHT_TOP,
                    "from file",
                    egui::FontId::proportional(theme::FONT_VALUE),
                    theme::TEXT_DIM,
                );
            }
            hint = "The tail over five seconds (a sketch for files): pick a room above or load a recorded impulse response";
        }
        // Drawn by `crate::tuner` instead.
        EffectKind::Plugin | EffectKind::PitchCorrect | EffectKind::Tuner => {}
        EffectKind::Rack => {}
        EffectKind::BeatShaper => {
            let (time_strip, volume_strip) = beat_strips(rect);
            let ramps = value(effect, "time_mode") >= 0.5;
            let specs = effect.kind.params();
            // Time grows downward (further back), volume upward, so at their
            // defaults (live, full level) the two rows of points sit apart.
            for (strip, first, ramped, down, label) in [
                (
                    time_strip,
                    FIRST_TIME,
                    ramps,
                    true,
                    "time (down = further back)",
                ),
                (volume_strip, FIRST_VOLUME, true, false, "volume"),
            ] {
                painter.rect_stroke(
                    strip,
                    0.0,
                    Stroke::new(1.0, theme::GRID_LINE),
                    egui::StrokeKind::Inside,
                );
                let points: Vec<f32> = (0..BEAT_POINTS)
                    .map(|k| effect.values.get(first + k).copied().unwrap_or(0.0))
                    .collect();
                let inner = strip.shrink2(vec2(0.0, 4.0));
                let y = |v: f32| {
                    if down {
                        inner.top() + v * inner.height()
                    } else {
                        inner.bottom() - v * inner.height()
                    }
                };
                let line: Vec<Pos2> = (0..=width)
                    .map(|i| {
                        let phase = i as f32 / width as f32;
                        let v = musiccraft_dsp::beat::envelope(&points, phase.min(0.9999), ramped);
                        pos2(strip.left() + i as f32, y(v))
                    })
                    .collect();
                for pair in line.windows(2) {
                    painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, color));
                }
                for (k, v) in points.iter().enumerate() {
                    let x = strip.left() + strip.width() * k as f32 / BEAT_POINTS as f32;
                    handles.push((pos2(x, y(*v)), None, specs.get(first + k).map(|s| s.name)));
                }
                painter.text(
                    pos2(strip.right() - 4.0, strip.bottom() - 2.0),
                    egui::Align2::RIGHT_BOTTOM,
                    label,
                    egui::FontId::proportional(theme::FONT_VALUE),
                    theme::TEXT_DIM,
                );
            }
            hint = "One cycle, left to right. Top: how far behind playback runs (hold to repeat, ramp down to slow or stop); bottom: volume. Drag the points";
        }
        EffectKind::Scratch => {
            let held = value(effect, "hold") >= 0.5;
            let pattern = value(effect, "pattern");
            let depth = value(effect, "depth");
            let position = value(effect, "position");
            let record = Rect::from_min_max(
                pos2(rect.left() + 8.0, rect.top() + 10.0),
                pos2(rect.right() - 8.0, rect.top() + 30.0),
            );
            painter.rect_filled(
                record,
                3.0,
                color.gamma_multiply(if held { 0.5 } else { 0.15 }),
            );
            painter.text(
                record.center(),
                egui::Align2::CENTER_CENTER,
                if held {
                    "held span"
                } else {
                    "recording (turn hold on to freeze)"
                },
                egui::FontId::proportional(theme::FONT_VALUE),
                theme::TEXT,
            );
            // One cycle of the play point's movement, time down the graph.
            let lane = Rect::from_min_max(
                pos2(record.left(), record.bottom() + 6.0),
                pos2(record.right(), rect.bottom() - 4.0),
            );
            let steps = lane.height().max(1.0) as usize;
            let points: Vec<Pos2> = (0..=steps)
                .map(|i| {
                    let phase = i as f32 / steps as f32;
                    let at = if pattern.round() as i32 == 0 {
                        position
                    } else {
                        (position
                            + musiccraft_dsp::scratch::scratch_pattern(pattern, phase).0 * depth)
                            .min(1.0)
                    };
                    pos2(lane.left() + lane.width() * at, lane.top() + i as f32)
                })
                .collect();
            for pair in points.windows(2) {
                painter.line_segment([pair[0], pair[1]], Stroke::new(2.0, color));
            }
            let needle = pos2(record.left() + record.width() * position, record.center().y);
            handles.push((needle, Some("position"), None));
            hint = "The held stretch of audio, start to end; drag the needle (or automate position) to scratch. The line shows a pattern's movement over one cycle";
        }
        EffectKind::MultiFx => {
            let block = value(effect, "block").round().max(0.0) as usize;
            let (x_name, y_name) = BLOCK_CONTROLS.get(block).copied().unwrap_or(("", ""));
            for i in 1..4 {
                let t = i as f32 / 4.0;
                let x = rect.left() + rect.width() * t;
                let y = rect.top() + rect.height() * t;
                painter.line_segment(
                    [pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(1.0, theme::GRID_LINE),
                );
                painter.line_segment(
                    [pos2(rect.left(), y), pos2(rect.right(), y)],
                    Stroke::new(1.0, theme::GRID_LINE),
                );
            }
            let font = egui::FontId::proportional(theme::FONT_VALUE);
            painter.text(
                pos2(rect.right() - 4.0, rect.bottom() - 4.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("X: {x_name}"),
                font.clone(),
                theme::TEXT_DIM,
            );
            painter.text(
                pos2(rect.left() + 4.0, rect.top() + 4.0),
                egui::Align2::LEFT_TOP,
                format!("Y: {y_name}"),
                font,
                theme::TEXT_DIM,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                BLOCK_NAMES.get(block).copied().unwrap_or("?"),
                egui::FontId::proportional(theme::FONT_DISPLAY),
                color.gamma_multiply(0.5),
            );
            let at = pos2(
                rect.left() + rect.width() * value(effect, "x"),
                rect.bottom() - rect.height() * value(effect, "y"),
            );
            handles.push((at, Some("x"), Some("y")));
            hint = "Drag the point: across and up play the block's two controls";
        }
        EffectKind::FxChain => {
            let stages: Vec<(&str, bool)> = (1..=CHAIN_STAGES)
                .map(|s| {
                    let block = value(effect, &format!("stage{s}_block")).round().max(0.0) as usize;
                    (BLOCK_NAMES.get(block).copied().unwrap_or("?"), block > 0)
                })
                .collect();
            chain_diagram(&painter, rect, color, &stages);
            hint =
                "Four blocks in series, left to right; each stage's X and Y knobs play its block";
        }
        EffectKind::GuitarChain => {
            let tone_on = ["bass", "mid", "treble"]
                .iter()
                .any(|n| value(effect, n).abs() > 0.05);
            let stages = [
                ("Comp", value(effect, "compress") > 0.0),
                ("Drive", value(effect, "drive") > 0.0),
                ("Tone", tone_on),
                ("Cab", value(effect, "cabinet") >= 0.5),
                ("Chorus", value(effect, "chorus") > 0.0),
            ];
            chain_diagram(&painter, rect, color, &stages);
            hint = "The pedal chain, left to right; lit stages are on. Set a stage's knob to zero to switch it off";
        }
        EffectKind::Send => {}
    }

    let drag_id = id.with("graph-drag");
    let dragging: Option<usize> = ui.data(|d| d.get_temp(drag_id));
    for (i, (at, _, _)) in handles.iter().enumerate() {
        handle(&painter, *at, dragging == Some(i), theme::HIGHLIGHT);
    }
    let response = response.hint(hint);
    if response.drag_started() {
        if let Some(pointer) = response.interact_pointer_pos() {
            let nearest = handles
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.0.distance(pointer).total_cmp(&b.1.0.distance(pointer)))
                .map(|(i, _)| i);
            if let Some(handle) = nearest {
                edits.push(Edit::Begin);
                ui.data_mut(|d| d.insert_temp(drag_id, handle));
            }
        }
    }
    if response.drag_stopped() {
        ui.data_mut(|d| d.remove::<usize>(drag_id));
    }
    // Clicking a gate step turns it on or off.
    if effect.kind == EffectKind::Gate && response.clicked() {
        if let Some(pointer) = response.interact_pointer_pos() {
            let index = (((pointer.x - rect.left()) / rect.width() * 16.0) as usize).min(15);
            let steps = value(effect, "steps").round() as u32 ^ (1 << index);
            edits.push(Edit::Set(vec![("steps", steps as f32)]));
        }
    }
    if let (Some(drag), Some(pointer)) = (dragging, response.interact_pointer_pos()) {
        if response.dragged() {
            if let Some((_, x_param, y_param)) = handles.get(drag) {
                let mut changes = Vec::new();
                let mut push = |name: &'static str, raw: f32| {
                    if let Some(spec) = spec(effect, name) {
                        let new = spec.sanitize(raw);
                        if (new - value(effect, name)).abs() > 1e-6 {
                            changes.push((name, new));
                        }
                    }
                };
                match effect.kind {
                    EffectKind::Eq => {
                        if let Some(name) = x_param {
                            push(name, x_freq(rect, pointer.x));
                        }
                        if let Some(name) = y_param {
                            push(name, y_db(rect, pointer.y, 18.0, -18.0));
                        }
                    }
                    EffectKind::Filter => {
                        push("cutoff", x_freq(rect, pointer.x));
                        let t = (rect.bottom() - pointer.y) / rect.height();
                        push("resonance", ((t - 0.15) / 0.7).clamp(0.0, 1.0));
                    }
                    EffectKind::Compressor | EffectKind::NoiseGate => {
                        let db = -60.0 + 66.0 * (pointer.x - rect.left()) / rect.width();
                        push("threshold", db);
                    }
                    EffectKind::Limiter => push("ceiling", y_db(rect, pointer.y, 6.0, -60.0)),
                    EffectKind::Scratch => {
                        let left = rect.left() + 8.0;
                        push(
                            "position",
                            (pointer.x - left) / (rect.width() - 16.0).max(1.0),
                        );
                    }
                    EffectKind::MultiFx => {
                        push("x", (pointer.x - rect.left()) / rect.width());
                        push("y", (rect.bottom() - pointer.y) / rect.height());
                    }
                    EffectKind::BeatShaper => {
                        if let Some(name) = y_param {
                            let (time_strip, volume_strip) = beat_strips(rect);
                            let h = |strip: Rect| strip.shrink2(vec2(0.0, 4.0));
                            if name.starts_with("time_") {
                                let inner = h(time_strip);
                                push(name, (pointer.y - inner.top()) / inner.height().max(1.0));
                            } else {
                                let inner = h(volume_strip);
                                push(name, (inner.bottom() - pointer.y) / inner.height().max(1.0));
                            }
                        }
                    }
                    EffectKind::Waveshaper => {
                        if let Some(name) = y_param {
                            let y = (rect.center().y - pointer.y) / (rect.height() * 0.4);
                            push(name, y.clamp(-1.0, 1.0));
                        }
                    }
                    EffectKind::Dynamics => {
                        if let Some(name) = x_param {
                            push(
                                name,
                                -60.0 + 66.0 * (pointer.x - rect.left()) / rect.width(),
                            );
                        }
                        if let Some(name) = y_param {
                            push(name, y_db(rect, pointer.y, 6.0, -60.0));
                        }
                    }
                    EffectKind::MultiTap => {
                        if let Some(name) = x_param {
                            push(name, (pointer.x - rect.left()) / rect.width() * TAP_SPAN);
                        }
                        if let Some(name) = y_param {
                            push(name, tap_level(rect, pointer.y));
                        }
                    }
                    kind => {
                        let (top, bottom) = db_range(kind);
                        if let Some(name) = x_param {
                            push(name, x_freq(rect, pointer.x));
                        }
                        match *y_param {
                            Some("resonance") => {
                                let t = (rect.bottom() - pointer.y) / rect.height();
                                push("resonance", ((t - 0.15) / 0.7).clamp(0.0, 1.0));
                            }
                            Some(name) if name.starts_with("step_") => {
                                push(name, lfo_strip_value(rect, pointer.y));
                            }
                            Some(name) => push(name, y_db(rect, pointer.y, top, bottom)),
                            None => {}
                        }
                    }
                }
                if !changes.is_empty() {
                    edits.push(Edit::Drag(changes));
                }
            }
        }
    }
    // Scrolling over the middle EQ point changes its width.
    if effect.kind == EffectKind::Eq && response.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        let near_mid = handles
            .get(1)
            .zip(response.hover_pos())
            .is_some_and(|((at, _, _), pointer)| at.distance(pointer) < 20.0);
        if scroll != 0.0 && near_mid {
            if let Some(spec) = spec(effect, "mid_q") {
                let q = spec.sanitize(value(effect, "mid_q") * (1.0 + scroll * 0.004));
                edits.push(Edit::Drag(vec![("mid_q", q)]));
            }
        }
    }
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton, RawInput};

    /// Runs one frame of the full editor with `events`; returns the edits and
    /// the graph's rectangle.
    fn frame(ctx: &egui::Context, effect: &EffectView, events: Vec<Event>) -> (Vec<Edit>, Rect) {
        let mut edits = Vec::new();
        let mut graph_rect = Rect::NOTHING;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(480.0, 480.0))),
            events,
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let id = egui::Id::new("test-editor");
            preset_bar(ui, id, effect, &mut edits);
            graph_rect = graph(ui, id, effect, 48_000.0, &mut edits);
            knob_row(ui, effect, &|_| false, &mut edits, &mut |_, _| {});
        });
        // No renderer here: acknowledge texture uploads.
        output.textures_delta.clear();
        (edits, graph_rect)
    }

    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn dragging_the_filter_curve_is_one_gesture() {
        let ctx = egui::Context::default();
        let effect = EffectView::new(EffectKind::Filter);
        let (_, rect) = frame(&ctx, &effect, vec![]);
        let cutoff = freq_x(rect, effect.value("cutoff"));
        let start = pos2(cutoff, rect.center().y);
        let mut edits = Vec::new();
        for events in [
            vec![Event::PointerMoved(start), button(start, true)],
            vec![Event::PointerMoved(start + vec2(30.0, 0.0))],
            vec![Event::PointerMoved(start + vec2(60.0, -10.0))],
            vec![button(start + vec2(60.0, -10.0), false)],
        ] {
            edits.extend(frame(&ctx, &effect, events).0);
        }
        assert_eq!(edits.first(), Some(&Edit::Begin), "{edits:?}");
        assert_eq!(
            edits.iter().filter(|e| **e == Edit::Begin).count(),
            1,
            "{edits:?}"
        );
        let moved = edits.iter().any(|e| match e {
            Edit::Drag(changes) => changes
                .iter()
                .any(|(name, v)| *name == "cutoff" && *v > effect.value("cutoff")),
            _ => false,
        });
        assert!(moved, "cutoff dragged up: {edits:?}");
    }

    #[test]
    fn clicking_a_gate_step_is_a_single_set() {
        let ctx = egui::Context::default();
        let effect = EffectView::new(EffectKind::Gate);
        let (_, rect) = frame(&ctx, &effect, vec![]);
        // The first step's column.
        let at = pos2(rect.left() + rect.width() / 32.0, rect.center().y);
        let mut edits = Vec::new();
        for events in [
            vec![Event::PointerMoved(at), button(at, true)],
            vec![button(at, false)],
        ] {
            edits.extend(frame(&ctx, &effect, events).0);
        }
        let steps = effect.value("steps").round() as u32 ^ 1;
        assert_eq!(edits, [Edit::Set(vec![("steps", steps as f32)])]);
    }

    #[test]
    fn routing_parameters_are_known() {
        let compressor = EffectKind::Compressor;
        let sidechain = compressor.sidechain_index().unwrap();
        assert!(is_routing(compressor, sidechain));
        assert!(!is_routing(compressor, 0));
        let follow = EffectKind::PitchCorrect.channel_ref_index().unwrap();
        assert!(is_routing(EffectKind::PitchCorrect, follow));
    }
}
