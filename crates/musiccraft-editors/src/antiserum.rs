//! Antiserum's editor (`docs/spec/antiserum-app.md`): a preset bar, a strip
//! of modulation sources to drag onto knobs, and pages for the oscillators,
//! mixer, filters, modulation, matrix, effects, arpeggiator and sequencer, and
//! global settings. Like the other editors it never changes the sound
//! itself: it reports [`AntiserumEdit`]s, which the app turns into commands.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::path::PathBuf;
use std::sync::Arc;

use musiccraft_paths::Resolver;

use egui::{Color32, Pos2, Response, Sense, Stroke, Ui, pos2, vec2};
use musiccraft_antiserum::fx::{FX_SLOTS, MODULES};
use musiccraft_antiserum::modulation::MAX_POINTS;
use musiccraft_antiserum::notes::SEQ_STEPS;
use musiccraft_antiserum::params::{
    self as ap, ARP_BASE, ENV_BASE, ENV_STRIDE, FILTER_ROUTING, FX_SLOT_BASE, LFO_BASE, LFO_STRIDE,
    LFOS, MACRO1, MACROS, NOISE_BASE, OSCS, PARAM_COUNT, SEQ_BASE, SLOTS, SOURCES, SUB_BASE,
    SYNC_RATES, filter_param, fx_param, lfo, lfo_param, osc, osc_param, oscx, oscx_param, slot,
    slot_param,
};
use musiccraft_antiserum::{LfoShape, choices, params};
use musiccraft_antiserum_presets::{factory, label, step_data, table_frames, text};
use musiccraft_dsp::SampleBuffer;
use musiccraft_dsp::resynth::HarmonicTable;
use musiccraft_params::ParamSpec;
use musiccraft_patch::{CATEGORIES, Patch, PointData, StepData, TableRef};
use musiccraft_wavetables::{FRAME_LEN, Table};

use crate::hint::Hint;
use crate::sounds::{self, Listings, SampleChoice, Sounds};
use crate::wavetable::{WavetableEdit, WavetableEditor};
use crate::{theme, widgets};

/// A change the user made in the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum AntiserumEdit {
    /// A gesture began: what follows is one undo step.
    Begin,
    /// Parameter (index) and its new plain value.
    Set(usize, f32),
    /// Oscillator (0-based) and its new table reference.
    Table(usize, String),
    /// LFO (0-based) and its new drawn shape (empty: the default).
    LfoShape(usize, Vec<PointData>),
    /// Sequencer step (0-based) and its new values.
    Step(usize, StepData),
    /// Load the factory preset with this name.
    Preset(String),
    /// Save the patch to a file.
    Save,
    /// Load a patch file.
    Open,
    /// Browse files (the system's file dialog) for the sample oscillator
    /// (0-based) plays.
    ChooseSample(usize),
    /// Oscillator and the sample it now plays (empty: none), chosen from the
    /// sounds menu or dropped on it.
    Sample(usize, String),
    /// Browse files for a multisample zone to add to an oscillator.
    AddZone(usize),
    /// Oscillator and the sample of a zone to add to it, chosen from the
    /// sounds menu or dropped on its zones.
    Zone(usize, String),
    /// Oscillator, zone (0-based), and its new low, high and root keys.
    SetZone(usize, usize, u8, u8, u8),
    /// Remove a zone: oscillator, zone.
    RemoveZone(usize, usize),
}

/// The editor's pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Oscillators,
    Mixer,
    Filters,
    Modulation,
    Matrix,
    Effects,
    ArpSeq,
    Global,
}

const PAGES: [(Page, &str); 8] = [
    (Page::Oscillators, "Osc"),
    (Page::Mixer, "Mixer"),
    (Page::Filters, "Filters"),
    (Page::Modulation, "Mod"),
    (Page::Matrix, "Matrix"),
    (Page::Effects, "FX"),
    (Page::ArpSeq, "Arp/Seq"),
    (Page::Global, "Global"),
];

/// A modulation source being dragged onto a knob (its `SOURCES` number).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceDrag(usize);

/// An oscillator's table open in the wavetable editor.
struct TableEdit {
    osc: usize,
    table: Table,
    editor: WavetableEditor,
    /// The reference to report when the gesture ends (`None`: unchanged).
    pending: Option<String>,
}

/// The editor's own view state; the caller keeps it between frames.
#[derive(Default)]
pub struct AntiserumEditor {
    /// The open page.
    pub page: Page,
    search: String,
    /// The LFO shown on the modulation page.
    lfo: usize,
    /// The drawn shape while a point is dragged: LFO, points, point dragged.
    shape_drag: Option<(usize, Vec<PointData>, usize)>,
    table_edit: Option<TableEdit>,
    /// Built tables for the wavetable views, by reference.
    previews: HashMap<String, Arc<Table>>,
    /// Samples and analyses for the sample and spectral views, by name.
    samples: HashMap<String, Option<Arc<SampleBuffer>>>,
    analyses: HashMap<String, Arc<HarmonicTable>>,
    /// The project's folder, which relative sample paths start from, and
    /// the translation of paths from other systems (as the engine does).
    folder: Option<PathBuf>,
    paths: Resolver,
    /// The browser's folders, offered in the sample slots' menus.
    sound_folders: Vec<PathBuf>,
    listings: Listings,
    /// A sound being dragged from the browser, which slots accept.
    dragged: Option<String>,
}

/// What every control reads.
struct View<'a> {
    values: &'a [f32; PARAM_COUNT],
    specs: &'static [ParamSpec],
}

impl View<'_> {
    fn value(&self, index: usize) -> f32 {
        self.values.get(index).copied().unwrap_or(0.0)
    }

    /// Matrix routes into parameter `index`: (source, amount).
    fn routes(&self, index: usize) -> Vec<(usize, f32)> {
        (0..SLOTS)
            .filter_map(|s| {
                let source = self.value(slot_param(s, slot::SOURCE)) as usize;
                let dest = self.value(slot_param(s, slot::DEST)) as usize;
                (source != 0 && dest == index + 1)
                    .then(|| (source, self.value(slot_param(s, slot::AMOUNT))))
            })
            .collect()
    }
}

/// Colours for modulation rings, by source.
fn source_color(source: usize) -> Color32 {
    theme::palette(source)
}

/// Widest the editor grows.
const MAX_WIDTH: f32 = 640.0;

/// A knob caption short enough for its knob: the parameter's label with
/// long words abbreviated.
fn caption(name: &str) -> String {
    let words = [
        ("amount", "amt"),
        ("random", "rnd"),
        ("release", "rel"),
        ("attack", "att"),
        ("curve", "crv"),
        ("resonance", "res"),
        ("feedback", "fdbk"),
        ("saturation", "sat"),
        ("diffusion", "diff"),
        ("threshold", "thresh"),
        ("predelay", "pre"),
        ("sustain", "sus"),
        ("track", "trk"),
    ];
    let mut text = label(name);
    for (long, short) in words {
        text = text.replace(long, short);
    }
    // Sentence case, like the other instruments' captions.
    let mut chars = text.chars();
    chars.next().map_or(text.clone(), |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// A spec's value as a knob position.
fn to_unit(spec: &ParamSpec, value: f32) -> f32 {
    spec.to_unit(value)
}

impl AntiserumEditor {
    /// Draws the editor. `values` are every parameter's value (for example
    /// from `musiccraft_antiserum_presets::values`). `menu` is called with
    /// each knob's response and parameter name (for an automation menu).
    pub fn show(
        &mut self,
        ui: &mut Ui,
        values: &[f32; PARAM_COUNT],
        patch: &Patch,
        edits: &mut Vec<AntiserumEdit>,
        menu: &mut dyn FnMut(&Response, &str),
    ) {
        let view = View {
            values,
            specs: params(),
        };
        ui.set_max_width(MAX_WIDTH);
        self.preset_bar(ui, patch, edits);
        if self.table_edit.is_some() {
            self.table_editor(ui, edits);
            return;
        }
        sources_strip(ui);
        ui.horizontal_wrapped(|ui| {
            for (page, name) in PAGES {
                ui.selectable_value(&mut self.page, page, name);
            }
        });
        ui.separator();
        let mut cx = Cx {
            view: &view,
            edits,
            menu,
        };
        egui::ScrollArea::vertical()
            .id_salt("antiserum-page")
            .auto_shrink([false, true])
            .show(ui, |ui| match self.page {
                Page::Oscillators => self.oscillators(ui, &mut cx, patch),
                Page::Mixer => mixer(ui, &mut cx),
                Page::Filters => filters(ui, &mut cx),
                Page::Modulation => self.modulation(ui, &mut cx, patch),
                Page::Matrix => matrix(ui, &mut cx),
                Page::Effects => effects(ui, &mut cx),
                Page::ArpSeq => arp_seq(ui, &mut cx, patch),
                Page::Global => global(ui, &mut cx),
            });
    }

    fn preset_bar(&mut self, ui: &mut Ui, patch: &Patch, edits: &mut Vec<AntiserumEdit>) {
        let bank = factory();
        let current = bank.iter().position(|p| p.name == patch.name);
        ui.horizontal(|ui| {
            let step = |delta: isize| {
                let n = bank.len() as isize;
                let from = current.map_or(if delta > 0 { -1 } else { 0 }, |c| c as isize);
                ((from + delta).rem_euclid(n.max(1))) as usize
            };
            if crate::widgets::preset_arrow(ui, true, "antiserum-presets").clicked()
                && let Some(p) = bank.get(step(-1))
            {
                edits.push(AntiserumEdit::Preset(p.name.clone()));
            }
            egui::ComboBox::from_id_salt("antiserum-presets")
                .width(180.0)
                .selected_text(&patch.name)
                .show_ui(ui, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search"));
                    let search = self.search.to_lowercase();
                    let hit = |p: &Patch| {
                        search.is_empty()
                            || p.name.to_lowercase().contains(&search)
                            || p.tags.iter().any(|t| t.contains(&search))
                    };
                    for category in CATEGORIES {
                        let list: Vec<&Patch> = bank
                            .iter()
                            .filter(|p| p.category == category && hit(p))
                            .collect();
                        if list.is_empty() {
                            continue;
                        }
                        ui.label(egui::RichText::new(category).small().color(theme::TEXT_DIM));
                        for p in list {
                            if ui.selectable_label(p.name == patch.name, &p.name).clicked() {
                                edits.push(AntiserumEdit::Preset(p.name.clone()));
                            }
                        }
                    }
                    // The user's own presets (`docs/spec/antiserum-app.md`).
                    let user = musiccraft_antiserum_presets::user::cached();
                    let mine: Vec<&Patch> = user.iter().filter(|p| hit(p)).collect();
                    if !mine.is_empty() {
                        ui.label(egui::RichText::new("user").small().color(theme::TEXT_DIM));
                        for p in mine {
                            if ui.selectable_label(p.name == patch.name, &p.name).clicked() {
                                edits.push(AntiserumEdit::Preset(p.name.clone()));
                            }
                        }
                    }
                });
            if crate::widgets::preset_arrow(ui, false, "antiserum-presets").clicked()
                && let Some(p) = bank.get(step(1))
            {
                edits.push(AntiserumEdit::Preset(p.name.clone()));
            }
            if ui
                .small_button("Save…")
                .hint("Save this sound as a patch file (in the user presets folder, it becomes a preset)")
                .clicked()
            {
                edits.push(AntiserumEdit::Save);
            }
            if ui.small_button("Open…").hint("Load a patch file").clicked() {
                edits.push(AntiserumEdit::Open);
            }
        });
    }

    /// The table a reference names, built once for drawing.
    fn preview(&mut self, reference: &str) -> Option<Arc<Table>> {
        if let Some(table) = self.previews.get(reference) {
            return Some(Arc::clone(table));
        }
        let table = Arc::new(table_frames(reference).ok()?);
        if self.previews.len() > 16 {
            self.previews.clear();
        }
        self.previews
            .insert(reference.to_owned(), Arc::clone(&table));
        Some(table)
    }

    fn oscillators(&mut self, ui: &mut Ui, cx: &mut Cx<'_, '_>, patch: &Patch) {
        for o in 0..OSCS {
            let p = |field| osc_param(o, field);
            let x = |field| oscx_param(o, field);
            let kind = cx.view.value(x(oscx::TYPE)).round() as usize;
            section(ui, &format!("OSCILLATOR {}", o + 1), |ui| {
                ui.horizontal(|ui| {
                    control(ui, cx, p(osc::ON));
                    control(ui, cx, x(oscx::TYPE));
                });
                if kind != 0 {
                    self.source_oscillator(ui, cx, patch, o, kind);
                    return;
                }
                ui.horizontal(|ui| {
                    let reference = patch.table(o).to_owned();
                    let position = cx.view.value(p(osc::POSITION));
                    if let Some(table) = self.preview(&reference) {
                        table_view(ui, &table, position);
                    }
                    ui.vertical(|ui| {
                        let shown = reference
                            .strip_prefix("factory:")
                            .unwrap_or("edited")
                            .to_owned();
                        egui::ComboBox::from_id_salt(("antiserum-table", o))
                            .width(110.0)
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for name in musiccraft_wavetables::factory::names() {
                                    if ui.selectable_label(false, *name).clicked() {
                                        cx.edits.push(AntiserumEdit::Begin);
                                        cx.edits.push(AntiserumEdit::Table(
                                            o,
                                            format!("factory:{name}"),
                                        ));
                                    }
                                }
                            });
                        if ui
                            .button("Edit…")
                            .hint("Open the wavetable editor")
                            .clicked()
                            && let Ok(table) = table_frames(&reference)
                        {
                            self.table_edit = Some(TableEdit {
                                osc: o,
                                table,
                                editor: WavetableEditor::default(),
                                pending: None,
                            });
                        }
                    });
                });
                knobs(
                    ui,
                    cx,
                    [
                        osc::POSITION,
                        osc::OCTAVE,
                        osc::SEMI,
                        osc::FINE,
                        osc::PHASE,
                        osc::RANDOM_PHASE,
                        osc::UNISON,
                        osc::DETUNE,
                        osc::BLEND,
                        osc::WIDTH,
                        osc::STACK,
                    ]
                    .map(p),
                );
                knobs(
                    ui,
                    cx,
                    [
                        osc::WARP1,
                        osc::WARP1_AMOUNT,
                        osc::WARP2,
                        osc::WARP2_AMOUNT,
                        osc::MOD_SOURCE,
                    ]
                    .map(p),
                );
            });
        }
        section(ui, "SUB", |ui| {
            knobs(ui, cx, (SUB_BASE..SUB_BASE + 5).collect::<Vec<_>>())
        });
        section(ui, "NOISE", |ui| {
            knobs(ui, cx, (NOISE_BASE..NOISE_BASE + 7).collect::<Vec<_>>())
        });
    }

    /// A sample, multisample, granular or spectral oscillator (`kind` 1–4):
    /// its source, its picture and its knobs.
    fn source_oscillator(
        &mut self,
        ui: &mut Ui,
        cx: &mut Cx<'_, '_>,
        patch: &Patch,
        o: usize,
        kind: usize,
    ) {
        let p = |field| osc_param(o, field);
        let x = |field| oscx_param(o, field);
        let source = patch.source(o);
        if kind == 2 {
            let (mut sounds, dragged) = self.sounds();
            zones(ui, cx, o, &source.zones, &mut sounds, dragged);
        } else {
            let (mut sounds, dragged) = self.sounds();
            let row = ui.horizontal(|ui| {
                let name = if source.sample.is_empty() {
                    "(no sample)"
                } else {
                    source
                        .sample
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&source.sample)
                };
                ui.label(name).hint(source.sample.clone());
                let choice = sounds::button(
                    ui,
                    "Sample…",
                    "Choose the sample this oscillator plays (or drag one here from the browser)",
                    &mut sounds,
                    !source.sample.is_empty(),
                );
                match choice {
                    Some(SampleChoice::Browse) => cx.edits.push(AntiserumEdit::ChooseSample(o)),
                    Some(SampleChoice::Use(sample)) => {
                        cx.edits.push(AntiserumEdit::Sample(o, sample));
                    }
                    Some(SampleChoice::Clear) => {
                        cx.edits.push(AntiserumEdit::Sample(o, String::new()));
                    }
                    None => {}
                }
            });
            if let Some(sample) = sounds::drop_target(ui, row.response.rect, dragged) {
                cx.edits.push(AntiserumEdit::Sample(o, sample));
            }
            let sample = self.sample_preview(&source.sample);
            if kind == 4 {
                let analysis = self.analysis_preview(&source.sample, sample.as_deref());
                let at = cx.view.value(p(osc::POSITION)) * analysis.duration();
                partials_view(ui, &analysis.at(at, false));
            } else if let Some(buffer) = sample {
                let marks = [
                    cx.view.value(p(osc::POSITION)),
                    cx.view.value(x(oscx::LOOP_START)),
                    cx.view.value(x(oscx::LOOP_END)),
                ];
                waveform_view(
                    ui,
                    &buffer,
                    marks,
                    kind == 1 && cx.view.value(x(oscx::LOOP)) >= 0.5,
                );
            }
        }
        let pitch = [osc::POSITION, osc::OCTAVE, osc::SEMI, osc::FINE].map(p);
        let unison = [osc::UNISON, osc::DETUNE, osc::BLEND, osc::WIDTH].map(p);
        match kind {
            1 => {
                knobs(ui, cx, pitch.into_iter().chain(unison));
                knobs(
                    ui,
                    cx,
                    [
                        oscx::ROOT,
                        oscx::SAMPLE_MODE,
                        oscx::LOOP,
                        oscx::LOOP_START,
                        oscx::LOOP_END,
                        oscx::GRAIN_SIZE,
                    ]
                    .map(x),
                );
            }
            2 => {
                knobs(ui, cx, pitch.into_iter().chain(unison));
                knobs(
                    ui,
                    cx,
                    [oscx::LOOP, oscx::LOOP_START, oscx::LOOP_END].map(x),
                );
            }
            3 => {
                knobs(
                    ui,
                    cx,
                    pitch.into_iter().chain(unison).chain([x(oscx::ROOT)]),
                );
                knobs(
                    ui,
                    cx,
                    [
                        oscx::GRAIN_SIZE,
                        oscx::GRAIN_DENSITY,
                        oscx::GRAIN_SPREAD,
                        oscx::GRAIN_PITCH,
                        oscx::GRAIN_SCAN,
                    ]
                    .map(x),
                );
            }
            _ => {
                knobs(ui, cx, pitch.into_iter().chain(unison));
                knobs(
                    ui,
                    cx,
                    [
                        oscx::SPECTRAL_SHIFT,
                        oscx::SPECTRAL_STRETCH,
                        oscx::SPECTRAL_TILT,
                        oscx::SPECTRAL_SPEED,
                        oscx::LOOP,
                    ]
                    .map(x),
                );
            }
        }
        let warps = [
            osc::WARP1,
            osc::WARP1_AMOUNT,
            osc::WARP2,
            osc::WARP2_AMOUNT,
            osc::MOD_SOURCE,
        ];
        knobs(ui, cx, warps.map(p));
        // On sample types, warps that reshape a cycle have nothing to act on.
        let shaping = |index: usize| {
            matches!(
                cx.view.value(index).round() as i32,
                1 | 2 | 3 | 5 | 6 | 7 | 10
            )
        };
        if kind != 4 && (shaping(p(osc::WARP1)) || shaping(p(osc::WARP2))) {
            ui.label(
                egui::RichText::new(
                    "Sync, bend, asym, mirror and remap reshape a wave's cycle; a sample has none, so they do nothing here.",
                )
                .small()
                .color(theme::TEXT_DIM),
            );
        }
    }

    /// A sample for drawing, loaded once (built-in names and paths as given).
    /// Sets the folder relative sample paths start from (the project's);
    /// previews load again when it changes.
    /// The folders the sample slots' menus offer, and the sound being
    /// dragged from the browser (if any). Call before each `show`.
    pub fn set_sounds(&mut self, folders: &[PathBuf], dragged: Option<String>) {
        if self.sound_folders != folders {
            self.sound_folders = folders.to_vec();
            self.listings.clear();
        }
        self.dragged = dragged;
    }

    fn sounds(&mut self) -> (Sounds<'_>, Option<&str>) {
        let sounds = Sounds {
            folders: &self.sound_folders,
            listings: &mut self.listings,
        };
        (sounds, self.dragged.as_deref())
    }

    pub fn set_folder(&mut self, folder: Option<&std::path::Path>) {
        if self.folder.as_deref() != folder || self.paths.refresh() {
            self.folder = folder.map(std::path::Path::to_path_buf);
            self.samples.clear();
            self.analyses.clear();
        }
    }

    fn sample_preview(&mut self, name: &str) -> Option<Arc<SampleBuffer>> {
        if name.is_empty() {
            return None;
        }
        if let Some(found) = self.samples.get(name) {
            return found.clone();
        }
        if self.samples.len() > 16 {
            self.samples.clear();
            self.analyses.clear();
        }
        let path = musiccraft_paths::sample_path(name, self.folder.as_deref(), &mut self.paths);
        let loaded = musiccraft_antiserum_presets::load_sample(&path, 48_000.0);
        self.samples.insert(name.to_owned(), loaded.clone());
        loaded
    }

    /// A sample's harmonic analysis for drawing, measured once.
    fn analysis_preview(
        &mut self,
        name: &str,
        sample: Option<&SampleBuffer>,
    ) -> Arc<HarmonicTable> {
        if let Some(found) = self.analyses.get(name) {
            return Arc::clone(found);
        }
        let table = Arc::new(sample.map_or_else(HarmonicTable::silent, HarmonicTable::analyze));
        self.analyses.insert(name.to_owned(), Arc::clone(&table));
        table
    }

    /// The wavetable editor on one oscillator's table, in place of the pages.
    fn table_editor(&mut self, ui: &mut Ui, edits: &mut Vec<AntiserumEdit>) {
        let Some(edit) = self.table_edit.as_mut() else {
            return;
        };
        let mut close = false;
        ui.horizontal(|ui| {
            if ui.button("◀ Back").clicked() {
                close = true;
            }
            ui.label(format!("Oscillator {} wavetable", edit.osc + 1));
        });
        let mut wavetable_edits = Vec::new();
        edit.editor.show(ui, &edit.table, &mut wavetable_edits);
        for change in wavetable_edits {
            match change {
                WavetableEdit::Begin => {}
                WavetableEdit::Op(op) => {
                    if op.apply(&mut edit.table).is_ok() {
                        edit.pending =
                            Some(TableRef::Data(edit.table.samples().to_vec()).to_text());
                    }
                }
                WavetableEdit::Factory(name) => {
                    if let Some(table) = musiccraft_wavetables::factory::table(name) {
                        edit.table = table;
                        edit.pending = Some(format!("factory:{name}"));
                    }
                }
            }
        }
        // One command per finished gesture.
        if !ui.input(|i| i.pointer.any_down())
            && let Some(reference) = edit.pending.take()
        {
            edits.push(AntiserumEdit::Begin);
            edits.push(AntiserumEdit::Table(edit.osc, reference));
        }
        if close {
            self.table_edit = None;
        }
    }

    fn modulation(&mut self, ui: &mut Ui, cx: &mut Cx<'_, '_>, patch: &Patch) {
        for e in 0..ap::ENVS {
            let first = ENV_BASE + e * ENV_STRIDE;
            let title = if e == 0 {
                "ENVELOPE 1 (AMP)".to_owned()
            } else {
                format!("ENVELOPE {}", e + 1)
            };
            section(ui, &title, |ui| {
                knobs(ui, cx, (first..first + ENV_STRIDE).collect::<Vec<_>>())
            });
        }
        ui.horizontal_wrapped(|ui| {
            for l in 0..LFOS {
                ui.selectable_value(&mut self.lfo, l, format!("LFO {}", l + 1));
            }
        });
        let l = self.lfo;
        section(ui, &format!("LFO {}", l + 1), |ui| {
            let first = LFO_BASE + l * LFO_STRIDE;
            knobs(ui, cx, (first..first + LFO_STRIDE).collect::<Vec<_>>());
            self.shape_editor(ui, cx, patch, l);
        });
    }

    /// The drawn shape of LFO `l`: click to add a point, drag to move,
    /// right-click to remove. Marked in beats when the LFO is synced.
    fn shape_editor(&mut self, ui: &mut Ui, cx: &mut Cx<'_, '_>, patch: &Patch, l: usize) {
        let stored: Vec<PointData> = patch.lfo_shape(l).map_or_else(
            || {
                LfoShape::default()
                    .points()
                    .iter()
                    .map(|p| [p.x, p.y, p.curve])
                    .collect()
            },
            <[_]>::to_vec,
        );
        let points = match &self.shape_drag {
            Some((lfo, points, _)) if *lfo == l => points.clone(),
            _ => stored.clone(),
        };
        let (response, painter) = ui.allocate_painter(vec2(380.0, 110.0), Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, 3.0, theme::LCD_BG);
        // Beats across one cycle when synced.
        if cx.view.value(lfo_param(l, lfo::SYNC)) >= 0.5 {
            let rate = cx.view.value(lfo_param(l, lfo::SYNC_RATE)) as usize;
            let beats = SYNC_RATES.get(rate).map_or(1.0, |r| r.1);
            if beats >= 1.0 {
                let n = beats.round() as usize;
                for b in 1..n.min(32) {
                    let x = rect.left() + rect.width() * b as f32 / n as f32;
                    painter.vline(x, rect.y_range(), Stroke::new(1.0, theme::GRID_BEAT));
                }
            }
            painter.text(
                rect.right_top() + vec2(-4.0, 4.0),
                egui::Align2::RIGHT_TOP,
                format!("{beats} beats"),
                egui::FontId::proportional(theme::FONT_VALUE),
                theme::TEXT_DIM,
            );
        }
        let shape = LfoShape::new(
            &points
                .iter()
                .map(|p| musiccraft_antiserum::LfoPoint {
                    x: p[0],
                    y: p[1],
                    curve: p[2],
                })
                .collect::<Vec<_>>(),
        );
        let to_screen = |x: f32, y: f32| {
            pos2(
                rect.left() + x * rect.width(),
                rect.bottom() - y * rect.height(),
            )
        };
        let line: Vec<Pos2> = (0..=190)
            .map(|i| {
                let x = i as f32 / 190.0;
                to_screen(x, shape.value(x))
            })
            .collect();
        painter.add(egui::Shape::line(line, Stroke::new(1.5, theme::ACCENT)));
        for p in &points {
            painter.circle_filled(to_screen(p[0], p[1]), 4.0, theme::HIGHLIGHT);
        }
        let pointer = response.interact_pointer_pos();
        let to_point = |pos: Pos2| {
            [
                ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0),
                ((rect.bottom() - pos.y) / rect.height()).clamp(0.0, 1.0),
            ]
        };
        let near = |pos: Pos2| {
            points
                .iter()
                .position(|p| to_screen(p[0], p[1]).distance(pos) < 7.0)
        };
        let commit = |points: Vec<PointData>, edits: &mut Vec<AntiserumEdit>| {
            edits.push(AntiserumEdit::Begin);
            edits.push(AntiserumEdit::LfoShape(l, points));
        };
        if response.drag_started()
            && let Some(pos) = pointer
        {
            let mut points = points.clone();
            let index = near(pos).unwrap_or_else(|| {
                let [x, y] = to_point(pos);
                points.push([x, y, 0.0]);
                points.len() - 1
            });
            self.shape_drag = Some((l, points, index));
        }
        if response.dragged()
            && let (Some(pos), Some((_, points, index))) = (pointer, self.shape_drag.as_mut())
            && let Some(point) = points.get_mut(*index)
        {
            let [x, y] = to_point(pos);
            *point = [x, y, point[2]];
        }
        if response.drag_stopped()
            && let Some((_, points, _)) = self.shape_drag.take()
        {
            commit(points, cx.edits);
        }
        if response.clicked()
            && let Some(pos) = pointer
            && near(pos).is_none()
            && points.len() < MAX_POINTS
        {
            let mut points = stored.clone();
            let [x, y] = to_point(pos);
            points.push([x, y, 0.0]);
            commit(points, cx.edits);
        }
        if response.secondary_clicked()
            && let Some(pos) = response.interact_pointer_pos()
            && let Some(index) = near(pos)
            && stored.len() > 1
        {
            let mut points = stored;
            points.remove(index);
            commit(points, cx.edits);
        }
        response.hint("Click to add a point, drag to move it, right-click to remove it (used when the shape is custom)");
    }
}

/// What the controls write to.
struct Cx<'v, 'e> {
    view: &'v View<'v>,
    edits: &'e mut Vec<AntiserumEdit>,
    menu: &'e mut dyn FnMut(&Response, &str),
}

fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(theme::PANEL_DARK)
        .corner_radius(6.0)
        .inner_margin(6.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                egui::RichText::new(title)
                    .small()
                    .strong()
                    .color(theme::TEXT_DIM),
            );
            add(ui);
        });
}

/// The draggable modulation sources.
fn sources_strip(ui: &mut Ui) {
    use musiccraft_antiserum::params::source;
    // Fixed rows: drag sources can't be measured before they are placed, so
    // a wrapping row would overflow instead of wrapping.
    let rows = [
        (source::ENV1, source::VELOCITY, "MOD"),
        (source::VELOCITY, source::MACRO1, ""),
        (source::MACRO1, SOURCES.len(), ""),
    ];
    for (first, end, title) in rows {
        ui.horizontal(|ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            ui.add_sized(
                vec2(28.0, 14.0),
                egui::Label::new(egui::RichText::new(title).small().color(theme::TEXT_DIM)),
            )
            .hint("Drag a source onto a knob to modulate it");
            for (s, name) in SOURCES.iter().enumerate().take(end).skip(first) {
                let id = egui::Id::new(("antiserum-source", s));
                ui.dnd_drag_source(id, SourceDrag(s), |ui| {
                    egui::Frame::new()
                        .fill(source_color(s).gamma_multiply(0.35))
                        .corner_radius(4.0)
                        .inner_margin(vec2(4.0, 1.0))
                        .show(ui, |ui| ui.label(egui::RichText::new(*name).small()));
                });
            }
        });
    }
}

/// A row of controls.
fn knobs(ui: &mut Ui, cx: &mut Cx<'_, '_>, indices: impl IntoIterator<Item = usize>) {
    ui.horizontal_wrapped(|ui| {
        for index in indices {
            control(ui, cx, index);
        }
    });
}

/// The control for parameter `index`: a switch, a menu or a knob.
fn control(ui: &mut Ui, cx: &mut Cx<'_, '_>, index: usize) {
    let Some(spec) = cx.view.specs.get(index).copied() else {
        return;
    };
    let value = cx.view.value(index);
    if let Some(list) = choices(spec.name) {
        if list == ["off", "on"] {
            let on = value >= 0.5;
            let response = ui
                .add(egui::Button::new(label(spec.name)).selected(on))
                .hint(spec.name);
            (cx.menu)(&response, spec.name);
            if response.clicked() {
                cx.edits.push(AntiserumEdit::Begin);
                cx.edits
                    .push(AntiserumEdit::Set(index, if on { 0.0 } else { 1.0 }));
            }
            return;
        }
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(label(spec.name))
                    .small()
                    .color(theme::TEXT_DIM),
            );
            let current = (value - spec.min).max(0.0) as usize;
            let response = egui::ComboBox::from_id_salt(("antiserum-choice", index))
                .width(96.0)
                .selected_text(list.get(current).copied().unwrap_or("?"))
                .show_ui(ui, |ui| {
                    for (i, name) in list.iter().enumerate() {
                        if ui.selectable_label(i == current, *name).clicked() && i != current {
                            cx.edits.push(AntiserumEdit::Begin);
                            cx.edits
                                .push(AntiserumEdit::Set(index, spec.min + i as f32));
                        }
                    }
                })
                .response;
            (cx.menu)(&response, spec.name);
        });
        return;
    }
    let out = widgets::stepped_labeled_knob(
        ui,
        &caption(spec.name),
        to_unit(&spec, value),
        to_unit(&spec, spec.default),
        spec.min < 0.0 && spec.max > 0.0,
        &text(index, value),
        spec.steps(),
    );
    (cx.menu)(&out.response, spec.name);
    if out.started {
        cx.edits.push(AntiserumEdit::Begin);
    }
    if let Some(t) = out.changed {
        let mut new = spec.from_unit(t);
        if spec.integer {
            new = new.round();
        }
        if new != value {
            cx.edits.push(AntiserumEdit::Set(index, new));
        }
    }
    if !spec.integer {
        rings(ui, &out.response, cx.view, index);
        drop_target(&out.response, cx, index);
    }
}

/// Arcs outside a knob, one per route into it: clockwise for positive
/// amounts, counter-clockwise for negative.
fn rings(ui: &Ui, response: &Response, view: &View<'_>, index: usize) {
    let routes = view.routes(index);
    if routes.is_empty() {
        return;
    }
    let Some(spec) = view.specs.get(index) else {
        return;
    };
    let rect = response.rect;
    let center = pos2(rect.center().x, rect.top() + 22.0);
    let t = to_unit(spec, view.value(index));
    let painter = ui.painter_at(rect.expand(4.0));
    for (n, (source, amount)) in routes.iter().enumerate() {
        let radius = 19.0 + 2.5 * n as f32;
        let to = (t + amount).clamp(0.0, 1.0);
        let angle = |k: f32| PI * 0.75 + PI * 1.5 * k;
        let arc: Vec<Pos2> = (0..=16)
            .map(|i| {
                let k = t + (to - t) * i as f32 / 16.0;
                center + vec2(angle(k).cos(), angle(k).sin()) * radius
            })
            .collect();
        painter.add(egui::Shape::line(
            arc,
            Stroke::new(2.0, source_color(*source)),
        ));
    }
}

/// Dropping a source on a knob routes it there through the first free
/// matrix slot (unless that route exists).
fn drop_target(response: &Response, cx: &mut Cx<'_, '_>, index: usize) {
    if response.dnd_hover_payload::<SourceDrag>().is_some() {
        response.ctx.debug_painter().rect_stroke(
            response.rect,
            4.0,
            Stroke::new(1.5, theme::HIGHLIGHT),
            egui::StrokeKind::Outside,
        );
    }
    let Some(source) = response.dnd_release_payload::<SourceDrag>() else {
        return;
    };
    for edit in route_edits(cx.view, source.0, index) {
        cx.edits.push(edit);
    }
}

/// The edits that route `source` to parameter `index`, or none if the route
/// exists or every slot is used.
fn route_edits(view: &View<'_>, source: usize, index: usize) -> Vec<AntiserumEdit> {
    let exists = (0..SLOTS).any(|s| {
        view.value(slot_param(s, slot::SOURCE)) as usize == source
            && view.value(slot_param(s, slot::DEST)) as usize == index + 1
    });
    let free = (0..SLOTS).find(|s| {
        view.value(slot_param(*s, slot::SOURCE)) as usize == 0
            && view.value(slot_param(*s, slot::DEST)) as usize == 0
    });
    match (exists, free) {
        (false, Some(s)) => vec![
            AntiserumEdit::Begin,
            AntiserumEdit::Set(slot_param(s, slot::SOURCE), source as f32),
            AntiserumEdit::Set(slot_param(s, slot::DEST), (index + 1) as f32),
            AntiserumEdit::Set(slot_param(s, slot::AMOUNT), 0.5),
        ],
        _ => Vec::new(),
    }
}

/// The frame at `position` of `table`, and the frames behind it.
fn table_view(ui: &mut Ui, table: &Table, position: f32) {
    // Frames stand one behind the other, the first at the front and the
    // last up and to the right; the playing frame is lit where it stands.
    const SHOWN: usize = 12;
    let (rect, _) = ui.allocate_exact_size(vec2(150.0, 60.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, theme::LCD_BG);
    let frames = table.frames().max(1);
    let last = frames - 1;
    let current = ((position.clamp(0.0, 1.0) * last as f32).round() as usize).min(last);
    let inner = rect.shrink2(vec2(5.0, 4.0));
    let (dx, dy) = (inner.width() * 0.16, inner.height() * 0.32);
    let wave_width = inner.width() - dx;
    let amplitude = (inner.height() - dy) * 0.46;
    let draw = |frame: usize, stroke: Stroke| {
        let Some(samples) = table.frame(frame) else {
            return;
        };
        let depth = if last == 0 {
            0.0
        } else {
            frame as f32 / last as f32
        };
        let left = inner.left() + depth * dx;
        let middle = inner.bottom() - amplitude - depth * dy;
        let steps = wave_width.max(1.0) as usize;
        let points: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                let v = samples
                    .get(((t * FRAME_LEN as f32) as usize).min(FRAME_LEN - 1))
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(-1.0, 1.0);
                pos2(left + t * wave_width, middle - v * amplitude)
            })
            .collect();
        painter.add(egui::Shape::line(points, stroke));
    };
    // Back to front, so nearer frames cover farther ones.
    let count = SHOWN.min(frames);
    for k in (0..count).rev() {
        let frame = if count > 1 { k * last / (count - 1) } else { 0 };
        let fade = 1.0 - 0.6 * k as f32 / count as f32;
        draw(
            frame,
            Stroke::new(1.0, theme::ACCENT_DIM.gamma_multiply(fade)),
        );
    }
    draw(current, Stroke::new(1.5, theme::HIGHLIGHT));
}

/// A sample's waveform with marks at `[start, loop start, loop end]` (0..1);
/// the loop marks only when `looping`.
fn waveform_view(ui: &mut Ui, buffer: &SampleBuffer, marks: [f32; 3], looping: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(360.0, 56.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, theme::LCD_BG);
    let len = buffer.len().max(1);
    let columns = rect.width() as usize;
    for c in 0..columns {
        let from = c * len / columns;
        let to = ((c + 1) * len / columns).max(from + 1);
        let peak = buffer
            .left()
            .get(from..to.min(len))
            .unwrap_or(&[])
            .iter()
            .fold(0.0_f32, |m, s| m.max(s.abs()))
            .min(1.0);
        let x = rect.left() + c as f32;
        let h = peak * rect.height() * 0.48;
        painter.line_segment(
            [pos2(x, rect.center().y - h), pos2(x, rect.center().y + h)],
            Stroke::new(1.0, theme::ACCENT_DIM),
        );
    }
    let mark = |t: f32, color: Color32| {
        let x = rect.left() + t.clamp(0.0, 1.0) * rect.width();
        painter.vline(x, rect.y_range(), Stroke::new(1.5, color));
    };
    mark(marks[0], theme::HIGHLIGHT);
    if looping {
        mark(marks[1], theme::ACCENT);
        mark(marks[2], theme::ACCENT);
    }
}

/// The partial levels the spectral oscillator starts from.
fn partials_view(ui: &mut Ui, levels: &[f32]) {
    let (rect, _) = ui.allocate_exact_size(vec2(360.0, 56.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 3.0, theme::LCD_BG);
    let peak = levels.iter().fold(1e-6_f32, |m, l| m.max(*l));
    let bar = rect.width() / levels.len().max(1) as f32;
    for (n, level) in levels.iter().enumerate() {
        let h = level / peak * (rect.height() - 4.0);
        let left = rect.left() + n as f32 * bar;
        painter.rect_filled(
            egui::Rect::from_min_max(
                pos2(left + 1.0, rect.bottom() - h),
                pos2(left + bar - 1.0, rect.bottom()),
            ),
            0.0,
            theme::ACCENT,
        );
    }
}

/// The multisample zones: sample, low, high and root keys, and remove.
fn zones(
    ui: &mut Ui,
    cx: &mut Cx<'_, '_>,
    o: usize,
    zones: &[musiccraft_patch::ZoneRef],
    sounds: &mut Sounds<'_>,
    dragged: Option<&str>,
) {
    let grid = egui::Grid::new(("antiserum-zones", o)).show(ui, |ui| {
        for heading in ["sample", "low", "high", "root", ""] {
            ui.label(egui::RichText::new(heading).small().color(theme::TEXT_DIM));
        }
        ui.end_row();
        for (z, zone) in zones.iter().enumerate() {
            let name = zone
                .sample
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&zone.sample);
            ui.label(name).hint(zone.sample.clone());
            let mut keys = [zone.low, zone.high, zone.root];
            let mut changed = false;
            for key in &mut keys {
                let response = ui.add(egui::DragValue::new(key).range(0..=127).speed(0.2));
                changed |= response.drag_stopped() || response.lost_focus();
            }
            if changed && keys != [zone.low, zone.high, zone.root] && keys[0] <= keys[1] {
                cx.edits.push(AntiserumEdit::Begin);
                cx.edits
                    .push(AntiserumEdit::SetZone(o, z, keys[0], keys[1], keys[2]));
            }
            if ui.small_button("×").hint("Remove this zone").clicked() {
                cx.edits.push(AntiserumEdit::RemoveZone(o, z));
            }
            ui.end_row();
        }
    });
    let choice = sounds::button(
        ui,
        "Add zone…",
        "Add a sample as a zone (or drag one onto the zones from the browser)",
        sounds,
        false,
    );
    match choice {
        Some(SampleChoice::Browse) => cx.edits.push(AntiserumEdit::AddZone(o)),
        Some(SampleChoice::Use(sample)) => cx.edits.push(AntiserumEdit::Zone(o, sample)),
        Some(SampleChoice::Clear) | None => {}
    }
    if let Some(sample) = sounds::drop_target(ui, grid.response.rect, dragged) {
        cx.edits.push(AntiserumEdit::Zone(o, sample));
    }
}

fn mixer(ui: &mut Ui, cx: &mut Cx<'_, '_>) {
    for o in 0..OSCS {
        let p = |field| osc_param(o, field);
        section(ui, &format!("OSCILLATOR {}", o + 1), |ui| {
            knobs(
                ui,
                cx,
                [osc::ON, osc::LEVEL, osc::PAN, osc::FILTER1, osc::FILTER2].map(p),
            );
        });
    }
    use musiccraft_antiserum::params::{noise, sub};
    section(ui, "SUB", |ui| {
        knobs(
            ui,
            cx,
            [sub::ON, sub::LEVEL, sub::PAN, sub::FILTER1, sub::FILTER2].map(|f| SUB_BASE + f),
        );
    });
    section(ui, "NOISE", |ui| {
        knobs(
            ui,
            cx,
            [
                noise::ON,
                noise::LEVEL,
                noise::PAN,
                noise::FILTER1,
                noise::FILTER2,
            ]
            .map(|f| NOISE_BASE + f),
        );
    });
    section(ui, "ROUTING", |ui| knobs(ui, cx, [FILTER_ROUTING]));
}

fn filters(ui: &mut Ui, cx: &mut Cx<'_, '_>) {
    for f in 0..ap::FILTERS {
        section(ui, &format!("FILTER {}", f + 1), |ui| {
            knobs(
                ui,
                cx,
                (0..ap::FILTER_STRIDE)
                    .map(|field| filter_param(f, field))
                    .collect::<Vec<_>>(),
            );
        });
    }
    section(ui, "ROUTING", |ui| knobs(ui, cx, [FILTER_ROUTING]));
}

fn matrix(ui: &mut Ui, cx: &mut Cx<'_, '_>) {
    egui::Grid::new("antiserum-matrix")
        .striped(true)
        .num_columns(7)
        .show(ui, |ui| {
            for heading in [
                "",
                "source",
                "destination",
                "amount",
                "curve",
                "bipolar",
                "aux",
            ] {
                ui.label(egui::RichText::new(heading).small().color(theme::TEXT_DIM));
            }
            ui.end_row();
            for s in 0..SLOTS {
                ui.label(format!("{}", s + 1));
                for field in [slot::SOURCE, slot::DEST] {
                    choice_menu(ui, cx, slot_param(s, field), 110.0);
                }
                for field in [slot::AMOUNT, slot::CURVE] {
                    small_knob(ui, cx, slot_param(s, field));
                }
                control(ui, cx, slot_param(s, slot::BIPOLAR));
                choice_menu(ui, cx, slot_param(s, slot::AUX), 90.0);
                ui.end_row();
            }
        });
}

/// A compact menu for a choice parameter (no caption).
fn choice_menu(ui: &mut Ui, cx: &mut Cx<'_, '_>, index: usize, width: f32) {
    let Some(spec) = cx.view.specs.get(index).copied() else {
        return;
    };
    let Some(list) = choices(spec.name) else {
        return;
    };
    let current = (cx.view.value(index) - spec.min).max(0.0) as usize;
    egui::ComboBox::from_id_salt(("antiserum-menu", index))
        .width(width)
        .height(300.0)
        .selected_text(list.get(current).copied().unwrap_or("?"))
        .show_ui(ui, |ui| {
            for (i, name) in list.iter().enumerate() {
                if ui.selectable_label(i == current, *name).clicked() && i != current {
                    cx.edits.push(AntiserumEdit::Begin);
                    cx.edits
                        .push(AntiserumEdit::Set(index, spec.min + i as f32));
                }
            }
        });
}

/// A small knob without a caption (matrix rows).
fn small_knob(ui: &mut Ui, cx: &mut Cx<'_, '_>, index: usize) {
    let Some(spec) = cx.view.specs.get(index).copied() else {
        return;
    };
    let value = cx.view.value(index);
    let out = widgets::stepped_knob(
        ui,
        value,
        spec.min,
        spec.max,
        spec.default,
        spec.min < 0.0,
        spec.steps(),
    );
    let response = out
        .response
        .hint(format!("{}: {}", spec.name, text(index, value)));
    (cx.menu)(&response, spec.name);
    if out.started {
        cx.edits.push(AntiserumEdit::Begin);
    }
    if let Some(new) = out.changed {
        cx.edits.push(AntiserumEdit::Set(index, new));
    }
}

fn effects(ui: &mut Ui, cx: &mut Cx<'_, '_>) {
    section(ui, "ORDER", |ui| {
        egui::Grid::new("antiserum-fx-order").show(ui, |ui| {
            for s in 0..FX_SLOTS {
                ui.label(
                    egui::RichText::new(format!("{}", s + 1))
                        .small()
                        .color(theme::TEXT_DIM),
                );
                choice_menu(ui, cx, FX_SLOT_BASE + s, 72.0);
                if s % 5 == 4 {
                    ui.end_row();
                }
            }
        });
    });
    let mut seen = Vec::new();
    for s in 0..FX_SLOTS {
        let number = cx.view.value(FX_SLOT_BASE + s) as usize;
        let Some(m) = number.checked_sub(1) else {
            continue;
        };
        if seen.contains(&m) {
            continue;
        }
        seen.push(m);
        let Some(module) = MODULES.get(m) else {
            continue;
        };
        section(ui, &module.name.to_uppercase(), |ui| {
            knobs(
                ui,
                cx,
                (0..module.count)
                    .map(|f| fx_param(m, f))
                    .collect::<Vec<_>>(),
            );
        });
    }
    if seen.is_empty() {
        ui.label(egui::RichText::new("Choose an effect in a slot above.").color(theme::TEXT_DIM));
    }
}

fn arp_seq(ui: &mut Ui, cx: &mut Cx<'_, '_>, patch: &Patch) {
    section(ui, "ARPEGGIATOR", |ui| {
        knobs(ui, cx, (ARP_BASE..ARP_BASE + 4).collect::<Vec<_>>())
    });
    section(ui, "SEQUENCER", |ui| {
        knobs(ui, cx, (SEQ_BASE..SEQ_BASE + 3).collect::<Vec<_>>());
        let length = (cx.view.value(SEQ_BASE + ap::seq::LENGTH) as usize).clamp(1, SEQ_STEPS);
        let default = step_data(musiccraft_antiserum::Step::default());
        egui::ScrollArea::horizontal()
            .id_salt("antiserum-steps")
            .show(ui, |ui| {
                egui::Grid::new("antiserum-steps-grid").show(ui, |ui| {
                    ui.label("");
                    for step in 0..length {
                        ui.label(egui::RichText::new(format!("{}", step + 1)).small());
                    }
                    ui.end_row();
                    for (row, name) in ["on", "transpose", "velocity", "gate"].iter().enumerate() {
                        ui.label(egui::RichText::new(*name).small().color(theme::TEXT_DIM));
                        for step in 0..length {
                            let data = patch.step(step).unwrap_or(default);
                            if let Some(new) = step_cell(ui, row, data) {
                                cx.edits.push(AntiserumEdit::Begin);
                                cx.edits.push(AntiserumEdit::Step(step, new));
                            }
                        }
                        ui.end_row();
                    }
                });
            });
    });
}

/// One cell of the step grid; returns the step's new values when changed
/// (a drag reports when it ends).
fn step_cell(ui: &mut Ui, row: usize, data: StepData) -> Option<StepData> {
    let (on, transpose, velocity, gate) = data;
    match row {
        0 => {
            let response = ui.add(
                egui::Button::new("")
                    .selected(on)
                    .min_size(vec2(18.0, 14.0)),
            );
            response
                .clicked()
                .then_some((!on, transpose, velocity, gate))
        }
        1 => {
            let mut t = i32::from(transpose);
            let response = ui.add(egui::DragValue::new(&mut t).range(-24..=24).speed(0.2));
            changed(&response, t != i32::from(transpose)).then_some((on, t as i8, velocity, gate))
        }
        2 => {
            let mut v = velocity;
            let response = ui.add(
                egui::DragValue::new(&mut v)
                    .range(0.0..=1.0)
                    .speed(0.01)
                    .max_decimals(2),
            );
            changed(&response, v != velocity).then_some((on, transpose, v, gate))
        }
        _ => {
            let mut g = gate;
            let response = ui.add(
                egui::DragValue::new(&mut g)
                    .range(0.05..=1.0)
                    .speed(0.01)
                    .max_decimals(2),
            );
            changed(&response, g != gate).then_some((on, transpose, velocity, g))
        }
    }
}

/// Whether a value control's change should be reported this frame.
fn changed(response: &Response, differs: bool) -> bool {
    differs && (response.dragged() || response.lost_focus() || response.changed())
}

fn global(ui: &mut Ui, cx: &mut Cx<'_, '_>) {
    section(ui, "VOICING", |ui| knobs(ui, cx, 0..MACRO1));
    section(ui, "MACROS", |ui| knobs(ui, cx, MACRO1..MACRO1 + MACROS));
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{RawInput, Rect};
    use musiccraft_antiserum::index_of;

    #[test]
    fn knob_captions_are_sentence_case() {
        let first = musiccraft_antiserum::params()
            .iter()
            .map(|spec| caption(spec.name))
            .find(|c| !c.is_empty())
            .unwrap();
        assert!(first.starts_with(char::is_uppercase), "{first}");
    }

    fn values(patch: &Patch) -> Box<[f32; PARAM_COUNT]> {
        musiccraft_antiserum_presets::values(patch)
    }

    fn frame(editor: &mut AntiserumEditor, patch: &Patch) -> Vec<AntiserumEdit> {
        let ctx = egui::Context::default();
        let values = values(patch);
        let mut edits = Vec::new();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(480.0, 900.0))),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            editor.show(ui, &values, patch, &mut edits, &mut |_, _| {});
        });
        output.textures_delta.clear();
        edits
    }

    #[test]
    fn every_page_draws_for_every_category() {
        let mut editor = AntiserumEditor::default();
        for category in CATEGORIES {
            let patch = factory().iter().find(|p| p.category == category).unwrap();
            for (page, _) in PAGES {
                editor.page = page;
                assert!(frame(&mut editor, patch).is_empty(), "{category} {page:?}");
            }
        }
    }

    #[test]
    fn every_oscillator_type_draws_with_and_without_a_source() {
        let mut editor = AntiserumEditor::default();
        let mut patch = Patch::default();
        for kind in 0..ap::OSC_TYPES.len() {
            for osc in 0..OSCS {
                let name = params()[oscx_param(osc, oscx::TYPE)].name;
                patch.params.insert(name.to_owned(), kind as f32);
            }
            assert!(frame(&mut editor, &patch).is_empty(), "type {kind} empty");
        }
        let mut source = patch.source(0);
        source.sample = "builtin:crash".to_owned();
        source.zones.push(musiccraft_patch::ZoneRef::new(
            "builtin:kick".to_owned(),
            0,
            59,
            36,
        ));
        patch.set_source(0, source);
        for kind in 0..ap::OSC_TYPES.len() {
            let name = params()[oscx_param(0, oscx::TYPE)].name;
            patch.params.insert(name.to_owned(), kind as f32);
            assert!(frame(&mut editor, &patch).is_empty(), "type {kind} sample");
        }
    }

    #[test]
    fn dropping_a_source_routes_it_through_a_free_slot() {
        let patch = factory().iter().find(|p| p.name == "Glass Reese").unwrap();
        let values = values(patch);
        let view = View {
            values: &values,
            specs: params(),
        };
        let cutoff = index_of("filter2_cutoff").unwrap();
        let edits = route_edits(&view, ap::source::MACRO1, cutoff);
        // The preset uses slot 1, so slot 2 is the first free one.
        assert_eq!(
            edits,
            [
                AntiserumEdit::Begin,
                AntiserumEdit::Set(slot_param(1, slot::SOURCE), ap::source::MACRO1 as f32),
                AntiserumEdit::Set(slot_param(1, slot::DEST), (cutoff + 1) as f32),
                AntiserumEdit::Set(slot_param(1, slot::AMOUNT), 0.5),
            ]
        );
        // An existing route is not made twice.
        let lfo_route = index_of("filter1_cutoff").unwrap();
        assert!(route_edits(&view, ap::source::LFO1, lfo_route).is_empty());
        assert_eq!(view.routes(lfo_route), [(ap::source::LFO1, 0.08)]);
    }

    /// A short 16-bit mono WAV file.
    fn wav_bytes() -> Vec<u8> {
        let samples: Vec<i16> = (0..4_800).map(|i| ((i % 100) * 300) as i16).collect();
        let data_len = (samples.len() * 2) as u32;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&48_000_u32.to_le_bytes());
        out.extend_from_slice(&96_000_u32.to_le_bytes());
        out.extend_from_slice(&2_u16.to_le_bytes());
        out.extend_from_slice(&16_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    #[test]
    fn sample_previews_find_files_from_the_project_folder() {
        let dir = std::env::temp_dir().join(format!("mc-preview-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("samples")).unwrap();
        std::fs::write(dir.join("samples/hit.wav"), wav_bytes()).unwrap();
        let mut editor = AntiserumEditor::default();
        assert!(
            editor.sample_preview("samples/hit.wav").is_none(),
            "no folder yet"
        );
        editor.set_folder(Some(&dir));
        let found = editor.sample_preview("samples/hit.wav");
        assert_eq!(found.map(|b| b.len()), Some(4_800));
        assert!(editor.sample_preview("builtin:kick").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn step_cells_report_nothing_without_input() {
        let ctx = egui::Context::default();
        let mut out = None;
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            for row in 0..4 {
                out = out.or(step_cell(ui, row, (true, 5, 0.5, 0.25)));
            }
        });
        output.textures_delta.clear();
        assert_eq!(out, None);
    }
}
