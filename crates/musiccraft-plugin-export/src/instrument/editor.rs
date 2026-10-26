//! An instrument plugin's editor: the shared instrument editor (the app's
//! channel settings pages), the plugin's own swing and bend-range knobs, and
//! the samples, pads or zones as sample slots (built-in sounds, the user's
//! music and sample folders, or the system's file dialog). Edits become host
//! parameter gestures; sample changes load off the audio thread.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{RichText, Ui, vec2};
use musiccraft_editors::hint::Hint;
use musiccraft_editors::instrument::{
    self, InstrumentEdit, InstrumentEditor, Page, key_name, section,
};
use musiccraft_editors::sounds::{self, Listings, SampleChoice, Sounds};
use musiccraft_editors::{theme, widgets};
use musiccraft_file_dialog::Picker;
use musiccraft_instruments::Kind;
use musiccraft_params::BUILTIN_PREFIX;
use musiccraft_patch::ZoneRef;
use musiccraft_plugin_editor::EguiEditor;
use nih_plug::prelude::{GuiContext, ParamSetter};

use super::data::{PADS, SampleData};
use super::params::InstrumentParams;
use crate::editor::{begin, browse_sound, end, set, set_once, sound_folders};
use crate::params::Slot;

/// The editor's logical size.
pub const SIZE: (u32, u32) = (470, 780);

/// What the editor remembers between frames (GUI thread only).
#[derive(Default)]
struct View {
    page: Page,
    /// Slots in a host gesture, by name.
    active: Vec<&'static str>,
    /// The kit pad being edited.
    pad: usize,
    /// The open file dialog, and the slot it fills.
    files: Picker<Target>,
    /// The folders the sample slots' menus offer, and their listings.
    folders: Vec<PathBuf>,
    listings: Listings,
}

/// The slot a chosen sound goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Sample,
    Pad(usize),
    /// A new zone.
    Zone,
}

/// What the plugin draws when the host opens its GUI.
pub fn editor(params: Arc<InstrumentParams>) -> EguiEditor {
    let view = Mutex::new(View {
        folders: sound_folders(),
        ..View::default()
    });
    EguiEditor::new(SIZE, move |ui, gui| {
        if let Ok(mut view) = view.lock() {
            draw(ui, gui, &params, &mut view);
        }
    })
}

fn draw(ui: &mut Ui, gui: &dyn GuiContext, params: &InstrumentParams, view: &mut View) {
    let setter = ParamSetter::new(gui);
    let kind = params.kind;
    let title = crate::ids::INSTRUMENT_TITLES
        .iter()
        .zip(crate::ids::INSTRUMENT_KINDS)
        .find(|(_, k)| *k == kind)
        .map_or("Instrument", |(t, _)| *t);
    let values = params.values();
    let mut edits = Vec::new();
    // The app shows pitch in the channel header; here it sits by the title.
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong().size(15.0));
        ui.add_space(12.0);
        let mut menu = |_: &egui::Response, _: &str| {};
        InstrumentEditor::new(
            kind,
            &values,
            theme::ACCENT,
            egui::Id::new("musiccraft-instrument-header"),
            &mut edits,
            &mut menu,
        )
        .knob(ui, musiccraft_params::inst::PITCH, "Pitch");
    });
    samples(ui, params, view);
    let envelope = instrument::has_envelope(kind);
    if view.page == Page::Envelope && !envelope {
        view.page = Page::Sound;
    }
    ui.horizontal(|ui| {
        for (page, label, shown) in [
            (Page::Sound, "Sound", true),
            (Page::Envelope, "Envelope", envelope),
            (Page::Functions, "Functions", true),
        ] {
            if shown {
                ui.selectable_value(&mut view.page, page, label);
            }
        }
    });
    ui.separator();
    let mut own = Vec::new();
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let mut menu = |_: &egui::Response, _: &str| {};
            let mut editor = InstrumentEditor::new(
                kind,
                &values,
                theme::ACCENT,
                egui::Id::new("musiccraft-instrument"),
                &mut edits,
                &mut menu,
            );
            match view.page {
                Page::Functions => editor.functions_with(ui, &mut |ui, row| {
                    let slot = &params.own[usize::from(row != "swing")];
                    if let Some(value) = own_knob(ui, slot) {
                        own.push((slot.spec.name, value));
                    }
                }),
                page => editor.page(ui, page),
            }
        });
    for (name, change) in own {
        if let Some(slot) = params.named(name) {
            gesture(&setter, slot, &mut view.active, change);
        }
    }
    for edit in edits {
        match edit {
            InstrumentEdit::Begin => {}
            InstrumentEdit::Set(index, value) => {
                if let Some(slot) = params.slot(index) {
                    gesture(&setter, slot, &mut view.active, Some(value));
                }
            }
            InstrumentEdit::Preset(changes) => {
                for (index, value) in changes {
                    if let Some(slot) = params.slot(index) {
                        set_once(&setter, slot, value);
                    }
                }
            }
        }
    }
    // A gesture ends when the mouse lets go.
    if !ui.input(|i| i.pointer.any_down()) {
        for name in view.active.drain(..) {
            if let Some(slot) = params.named(name) {
                end(&setter, slot);
            }
        }
    }
}

/// Moves `slot` inside a gesture that lasts while the mouse is down.
fn gesture(
    setter: &ParamSetter<'_>,
    slot: &Slot,
    active: &mut Vec<&'static str>,
    value: Option<f32>,
) {
    if !active.contains(&slot.spec.name) {
        begin(setter, slot);
        active.push(slot.spec.name);
    }
    if let Some(value) = value {
        set(setter, slot, value);
    }
}

/// A knob for one of the plugin's own parameters; the new value, if any.
fn own_knob(ui: &mut Ui, slot: &Slot) -> Option<Option<f32>> {
    let spec = slot.spec;
    let value = slot.param.value();
    let label = if spec.name == "swing" {
        "Swing"
    } else {
        "Bend"
    };
    let out = widgets::stepped_labeled_knob(
        ui,
        label,
        spec.to_unit(value),
        spec.to_unit(spec.default),
        false,
        &spec.format(value),
        spec.steps(),
    );
    let hint = if spec.name == "swing" {
        "How far off-beat sixteenths move later (the app's project swing)"
    } else {
        "How far pitch bend moves the notes, in semitones"
    };
    let response = out.response.hint(hint);
    let _ = response;
    match (out.started, out.changed) {
        (_, Some(t)) => Some(Some(spec.from_unit(t))),
        (true, None) => Some(None),
        (false, None) => None,
    }
}

/// The sample, pads or zones, for the instruments that have them.
fn samples(ui: &mut Ui, params: &InstrumentParams, view: &mut View) {
    let shared = &params.shared;
    let kind = params.kind;
    if !matches!(
        kind,
        Kind::Sampler | Kind::Granular | Kind::Resynth | Kind::Kit | Kind::Zones
    ) {
        return;
    }
    let mut data = shared.data();
    let mut changed = false;
    if let Some((target, paths)) = view.files.poll()
        && let Some(path) = paths.first()
    {
        changed |= fill(&mut data, target, path.display().to_string());
    }
    section(ui, "SAMPLES", |ui| {
        match kind {
            Kind::Kit => changed |= pads(ui, &mut data, view),
            Kind::Zones => changed |= zones(ui, &mut data, view),
            _ => {
                let sample = ui
                    .horizontal(|ui| {
                        ui.add_sized(
                            vec2(300.0, 18.0),
                            egui::Label::new(RichText::new(sound_name(&data.sample)).small())
                                .truncate(),
                        )
                        .hint(data.sample.clone());
                        slot(ui, view, "Sample…", false, Target::Sample)
                    })
                    .inner;
                if let Some(sample) = sample {
                    changed |= fill(&mut data, Target::Sample, sample);
                }
                let loaded = shared.loaded();
                let slices = if kind == Kind::Sampler {
                    params.values()[musiccraft_params::inst::SLICES]
                        .round()
                        .max(1.0) as u32
                } else {
                    1
                };
                instrument::waveform(ui, Some(&loaded.overview), theme::ACCENT, slices);
            }
        }
        for error in shared.loaded().errors {
            ui.label(RichText::new(error).small().color(theme::HIGHLIGHT));
        }
    });
    if changed {
        shared.set_data(data);
    }
}

/// A sound's short name: a built-in sound's, or a file's.
fn sound_name(sample: &str) -> String {
    if sample.is_empty() {
        return "—".to_owned();
    }
    sample.strip_prefix(BUILTIN_PREFIX).map_or_else(
        || {
            std::path::Path::new(sample)
                .file_name()
                .map_or_else(|| sample.to_owned(), |n| n.to_string_lossy().into_owned())
        },
        str::to_owned,
    )
}

/// Puts `sample` in `target` (an empty sample clears a pad); whether the
/// data changed.
fn fill(data: &mut SampleData, target: Target, sample: String) -> bool {
    match target {
        Target::Sample if !sample.is_empty() && sample != data.sample => {
            data.sample = sample;
            true
        }
        Target::Pad(pad) => {
            data.pads.resize(PADS, String::new());
            match data.pads.get_mut(pad) {
                Some(slot) if *slot != sample => {
                    *slot = sample;
                    true
                }
                _ => false,
            }
        }
        Target::Zone if !sample.is_empty() => {
            data.zones.push(ZoneRef::new(sample, 0, 127, 60));
            true
        }
        _ => false,
    }
}

/// A sample slot's menu button; the sound chosen in it (empty: cleared).
/// "Browse files…" opens the system's file dialog, whose answer arrives
/// later through `View::files`.
fn slot(ui: &mut Ui, view: &mut View, text: &str, clear: bool, target: Target) -> Option<String> {
    let mut sounds = Sounds {
        folders: &view.folders,
        listings: &mut view.listings,
    };
    let hint = "Choose a built-in sound, one from your music folders, or a file";
    match sounds::button(ui, text, hint, &mut sounds, clear)? {
        SampleChoice::Browse => {
            let title = match target {
                Target::Sample => "Sample",
                Target::Pad(_) => "Pad sample",
                Target::Zone => "Zone sample",
            };
            browse_sound(&mut view.files, title, target, ui.ctx());
            None
        }
        SampleChoice::Use(sound) => Some(sound),
        SampleChoice::Clear => Some(String::new()),
    }
}

/// Sixteen pads; the chosen one's sample can be changed.
fn pads(ui: &mut Ui, data: &mut SampleData, view: &mut View) -> bool {
    data.pads.resize(PADS, String::new());
    egui::Grid::new("pads").spacing([4.0, 4.0]).show(ui, |ui| {
        for row in (0..4).rev() {
            for col in 0..4 {
                let pad = row * 4 + col;
                let name = data
                    .pads
                    .get(pad)
                    .filter(|p| !p.is_empty())
                    .map(|p| {
                        p.strip_prefix(BUILTIN_PREFIX).map_or_else(
                            || {
                                std::path::Path::new(p)
                                    .file_stem()
                                    .map_or_else(|| p.clone(), |s| s.to_string_lossy().into_owned())
                            },
                            str::to_owned,
                        )
                    })
                    .unwrap_or_else(|| "—".to_owned());
                let button =
                    egui::Button::new(RichText::new(format!("{}\n{name}", pad + 1)).small())
                        .min_size(vec2(96.0, 36.0))
                        .selected(view.pad == pad);
                if ui.add(button).clicked() {
                    view.pad = pad;
                }
            }
            ui.end_row();
        }
    });
    let pad = view.pad.min(PADS - 1);
    let current = data.pads.get(pad).cloned().unwrap_or_default();
    let chosen = ui
        .horizontal(|ui| {
            ui.label(
                RichText::new(format!("Pad {}", pad + 1))
                    .small()
                    .color(theme::TEXT_DIM),
            );
            slot(
                ui,
                view,
                "Pad sound…",
                !current.is_empty(),
                Target::Pad(pad),
            )
        })
        .inner;
    chosen.is_some_and(|sample| fill(data, Target::Pad(pad), sample))
}

/// The zone list with key ranges, and a row to add a zone.
fn zones(ui: &mut Ui, data: &mut SampleData, view: &mut View) -> bool {
    let mut changed = false;
    let mut remove = None;
    for (i, zone) in data.zones.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let name = zone.sample.strip_prefix(BUILTIN_PREFIX).map_or_else(
                || {
                    std::path::Path::new(&zone.sample)
                        .file_name()
                        .map_or_else(|| zone.sample.clone(), |n| n.to_string_lossy().into_owned())
                },
                str::to_owned,
            );
            ui.add_sized(
                vec2(110.0, 18.0),
                egui::Label::new(RichText::new(name).small()).truncate(),
            );
            for (label, value) in [
                ("Low", &mut zone.low),
                ("High", &mut zone.high),
                ("Root", &mut zone.root),
            ] {
                ui.label(RichText::new(label).small().color(theme::TEXT_DIM));
                let mut key = u32::from(*value);
                if ui
                    .add(
                        egui::DragValue::new(&mut key)
                            .range(0..=127)
                            .speed(0.2)
                            .custom_formatter(|v, _| key_name(v as u8)),
                    )
                    .changed()
                {
                    *value = key as u8;
                    changed = true;
                }
            }
            if zone.low > zone.high {
                std::mem::swap(&mut zone.low, &mut zone.high);
            }
            if ui.small_button("✕").hint("Remove this zone").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        data.zones.remove(i);
        changed = true;
    }
    if let Some(sample) = slot(ui, view, "Add zone…", false, Target::Zone) {
        changed |= fill(data, Target::Zone, sample);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{INSTRUMENT_KINDS, INSTRUMENT_NAMES};
    use crate::instrument::data::Shared;
    use egui::{Pos2, RawInput, Rect};

    struct NoHost;

    impl GuiContext for NoHost {
        fn plugin_api(&self) -> nih_plug::prelude::PluginApi {
            nih_plug::prelude::PluginApi::Clap
        }
        fn request_resize(&self) -> bool {
            false
        }
        unsafe fn raw_begin_set_parameter(&self, _: nih_plug::prelude::ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _: nih_plug::prelude::ParamPtr, _: f32) {}
        unsafe fn raw_end_set_parameter(&self, _: nih_plug::prelude::ParamPtr) {}
        fn get_state(&self) -> nih_plug::prelude::PluginState {
            unimplemented!("not used by the editor")
        }
        fn set_state(&self, _: nih_plug::prelude::PluginState) {}
    }

    #[test]
    fn every_instrument_editor_draws_every_page() {
        for (kind, name) in INSTRUMENT_KINDS.into_iter().zip(INSTRUMENT_NAMES) {
            let params = InstrumentParams::new(kind, name, Arc::new(Shared::new(kind).0));
            for page in [Page::Sound, Page::Envelope, Page::Functions] {
                let ctx = egui::Context::default();
                let mut view = View {
                    page,
                    ..View::default()
                };
                let input = RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(470.0, 780.0))),
                    ..RawInput::default()
                };
                let mut output = ctx.run_ui(input, |ui| draw(ui, &NoHost, &params, &mut view));
                output.textures_delta.clear();
                assert!(view.active.is_empty(), "{name} {page:?}");
            }
        }
    }
}
