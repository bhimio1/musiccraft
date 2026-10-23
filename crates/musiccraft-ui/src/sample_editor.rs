//! The sample editor below a channel's sample: a waveform to select a range
//! on and buttons that apply `sample.edit` operations to it (see
//! `docs/spec/sample-editor.md`).

use egui::{Color32, RichText, Sense, Stroke, pos2, vec2};
use musiccraft_commands::Invocation;

use crate::hint::Hint;
use crate::{Action, Frame, UiState, theme};

/// What the editor remembers between frames.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorState {
    /// Channel and selected range (fractions of the sample, start first).
    pub selection: Option<(usize, f32, f32)>,
    /// Where a drag began.
    anchor: Option<f32>,
    /// Values for the stretch, pitch and tune operations.
    ratio: f32,
    semitones: f32,
    key: u8,
    scale: u8,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            selection: None,
            anchor: None,
            ratio: 1.5,
            semitones: 2.0,
            key: 0,
            scale: 1,
        }
    }
}

impl EditorState {
    /// Drops the selection, which belongs to a channel of the old project.
    pub(crate) fn forget_project(&mut self) {
        self.selection = None;
        self.anchor = None;
    }
}

fn edit(channel: usize, operation: &str, extra: &[String]) -> Action {
    let mut args = vec![(channel + 1).to_string(), operation.to_owned()];
    args.extend(extra.iter().cloned());
    Action::Run(Invocation::new("sample.edit", args))
}

/// The waveform with the selection shaded; dragging selects.
fn strip(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    channel: usize,
    color: Color32,
) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 60.0), Sense::drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme::LCD_BG);
    let points = frame.waveform.unwrap_or(&[]);
    let peak = points
        .iter()
        .fold(0.0_f32, |p, (lo, hi)| p.max(lo.abs()).max(hi.abs()))
        .max(1e-6);
    let step = rect.width() / points.len().max(1) as f32;
    for (i, (lo, hi)) in points.iter().enumerate() {
        let x = rect.left() + (i as f32 + 0.5) * step;
        let half = rect.height() * 0.45 / peak;
        painter.line_segment(
            [
                pos2(x, rect.center().y - hi * half),
                pos2(x, rect.center().y - lo * half - 0.5),
            ],
            Stroke::new(step.max(1.0), color.gamma_multiply(0.8)),
        );
    }
    let at = |x: f32| ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    let editor = &mut state.sample_edit;
    if response.drag_started() {
        if let Some(p) = response.interact_pointer_pos() {
            editor.anchor = Some(at(p.x));
        }
    }
    if response.dragged() {
        if let (Some(a), Some(p)) = (editor.anchor, response.interact_pointer_pos()) {
            let b = at(p.x);
            editor.selection = Some((channel, a.min(b), a.max(b)));
        }
    }
    if response.drag_stopped() {
        editor.anchor = None;
    }
    if let Some((_, a, b)) = editor.selection.filter(|(c, a, b)| *c == channel && b > a) {
        let span = egui::Rect::from_min_max(
            pos2(rect.left() + rect.width() * a, rect.top()),
            pos2(rect.left() + rect.width() * b, rect.bottom()),
        );
        painter.rect_filled(span, 0.0, theme::HIGHLIGHT.gamma_multiply(0.25));
    }
    response.hint("Drag across the waveform to select a range; the buttons below edit it");
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    channel: usize,
    actions: &mut Vec<Action>,
) {
    let seconds = frame.waveform_seconds.unwrap_or(0.0);
    egui::CollapsingHeader::new(RichText::new("Edit sample").small().strong())
        .id_salt(("sample-editor", channel))
        .show(ui, |ui| {
            if frame.project.path().is_none() {
                ui.label(
                    RichText::new("Save the project first: edited audio is written beside it.")
                        .small()
                        .color(theme::TEXT_DIM),
                );
                return;
            }
            strip(
                ui,
                state,
                frame,
                channel,
                crate::channel_color(frame.project, channel),
            );
            let selection = state
                .sample_edit
                .selection
                .filter(|(c, a, b)| *c == channel && b > a)
                .map(|(_, a, b)| (f64::from(a) * seconds, f64::from(b) * seconds));
            let range: Vec<String> = selection
                .map(|(a, b)| vec![format!("{a:.4}"), format!("{b:.4}")])
                .unwrap_or_default();
            // Without a selection the range ops cover the whole sample, when
            // its length is known; reverse and gain then need no range.
            let whole = if seconds > 0.0 {
                vec!["0".to_owned(), format!("{seconds:.4}")]
            } else {
                Vec::new()
            };
            let span = if range.is_empty() { &whole } else { &range };
            ui.label(
                RichText::new(match selection {
                    Some((a, b)) => format!("Selected {a:.3}–{b:.3} s of {seconds:.3} s"),
                    None => format!("Whole sample: {seconds:.3} s (drag to select)"),
                })
                .small()
                .color(theme::TEXT_DIM),
            );
            let mut actions_here = Vec::new();
            ui.horizontal_wrapped(|ui| {
                for (label, op, hint) in [
                    ("Trim", "trim", "Keep only the selection"),
                    ("Cut", "cut", "Remove the selection"),
                    ("Silence", "silence", "Replace the selection with silence"),
                    ("Fade in", "fade_in", "Fade in across the selection"),
                    ("Fade out", "fade_out", "Fade out across the selection"),
                    ("Reverse", "reverse", "Play the selection backwards"),
                ] {
                    let needs_range = !matches!(op, "reverse");
                    let enabled = !needs_range || !span.is_empty();
                    let enabled = enabled && (op != "trim" && op != "cut" || !range.is_empty());
                    if crate::widgets::add_enabled(ui, enabled, egui::Button::new(label))
                        .hint(hint)
                        .clicked()
                    {
                        actions_here.push(edit(channel, op, span));
                    }
                }
                for (label, db) in [("−3 dB", "-3"), ("+3 dB", "3")] {
                    if ui
                        .button(label)
                        .hint("Change the level of the selection")
                        .clicked()
                    {
                        let mut extra = vec![db.to_owned()];
                        extra.extend(span.iter().cloned());
                        actions_here.push(edit(channel, "gain", &extra));
                    }
                }
                if ui
                    .button("Normalize")
                    .hint("Raise the loudest peak to 0 dB")
                    .clicked()
                {
                    actions_here.push(edit(channel, "normalize", &[]));
                }
            });
            let editor = &mut state.sample_edit;
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut editor.ratio)
                        .range(0.25..=4.0)
                        .speed(0.01)
                        .prefix("× "),
                );
                if ui
                    .button("Stretch")
                    .hint("Change the length, keeping the pitch")
                    .clicked()
                {
                    actions_here.push(edit(channel, "stretch", &[editor.ratio.to_string()]));
                }
                ui.add(
                    egui::DragValue::new(&mut editor.semitones)
                        .range(-24.0..=24.0)
                        .speed(0.1)
                        .suffix(" st"),
                );
                if ui
                    .button("Pitch")
                    .hint("Change the pitch, keeping the length")
                    .clicked()
                {
                    actions_here.push(edit(channel, "pitch", &[editor.semitones.to_string()]));
                }
            });
            ui.horizontal(|ui| {
                let names = musiccraft_project::params::NOTE_NAMES;
                egui::ComboBox::from_id_salt(("tune-key", channel))
                    .width(50.0)
                    .selected_text(names.get(usize::from(editor.key)).copied().unwrap_or("?"))
                    .show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(&mut editor.key, i as u8, *n);
                        }
                    });
                let scales = musiccraft_project::params::CORRECTION_SCALES;
                egui::ComboBox::from_id_salt(("tune-scale", channel))
                    .width(120.0)
                    .selected_text(
                        scales
                            .get(usize::from(editor.scale))
                            .map_or("?", |(n, _)| *n),
                    )
                    .show_ui(ui, |ui| {
                        for (i, (n, _)) in scales.iter().enumerate() {
                            ui.selectable_value(&mut editor.scale, i as u8, *n);
                        }
                    });
                if ui
                    .button("Tune")
                    .hint("Snap a single voice or instrument to the key and scale")
                    .clicked()
                {
                    actions_here.push(edit(
                        channel,
                        "tune",
                        &[editor.key.to_string(), editor.scale.to_string()],
                    ));
                }
            });
            if !actions_here.is_empty() {
                // The edited sample has a new length; the old range no longer fits.
                state.sample_edit.selection = None;
            }
            actions.extend(actions_here);
        });
}
