//! The built-in instruments' editor (`docs/spec/plugin-instruments.md`): the
//! Sound, Envelope and Functions pages of the app's channel settings, shared
//! with the instrument plugins. Like the other editors it never changes the
//! sound itself: it reports [`InstrumentEdit`]s, which the caller applies
//! (the app as commands, a plugin as host parameter changes).
//!
//! What needs the project (sample dialogs, the sample editor, slicing a
//! pattern, previews) stays with the caller, which draws it around these
//! sections.

use egui::{Color32, Pos2, Response, RichText, Sense, Shape, Stroke, Ui, pos2, vec2};
use musiccraft_instruments::{Kind, Params};
use musiccraft_params::presets::{self, Preset};
use musiccraft_params::{
    ARP_RATES, FM_ALGOS, FM_WAVES, INSTRUMENT_PARAMS, MOD_DESTS, MOD_SLOTS, MOD_SOURCES, ParamSpec,
    WAVEFORMS, inst,
};

use crate::hint::Hint;
use crate::{theme, widgets};

/// A change the user made in the editor.
#[derive(Debug, Clone, PartialEq)]
pub enum InstrumentEdit {
    /// A gesture began: what follows is one undo step.
    Begin,
    /// Instrument parameter (index into `INSTRUMENT_PARAMS`) and its new
    /// plain value.
    Set(usize, f32),
    /// A preset: every parameter it changes, as one step.
    Preset(Vec<(usize, f32)>),
}

/// Pages of the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    /// The instrument's own sections.
    #[default]
    Sound,
    /// The amplitude envelope.
    Envelope,
    /// Arpeggiator, swing, timing and echo, voices.
    Functions,
}

/// Whether the instrument shows the amplitude envelope page.
#[must_use]
pub fn has_envelope(kind: Kind) -> bool {
    matches!(
        kind,
        Kind::Sampler
            | Kind::Synth
            | Kind::Fm
            | Kind::Keys
            | Kind::Additive
            | Kind::Granular
            | Kind::Zones
            | Kind::Resynth
    )
}

/// The factory presets of `kind`, if it has any.
#[must_use]
pub fn presets(kind: Kind) -> Option<&'static [Preset]> {
    match kind {
        Kind::Synth => Some(presets::SYNTH),
        Kind::DrumSynth => Some(presets::DRUM),
        Kind::Pluck => Some(presets::PLUCK),
        Kind::Fm => Some(presets::FM),
        Kind::Keys => Some(presets::KEYS),
        Kind::Additive => Some(presets::ADDITIVE),
        _ => None,
    }
}

/// Name of a MIDI key, with key 60 = `C5`.
#[must_use]
pub fn key_name(key: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let name = NAMES.get(usize::from(key % 12)).copied().unwrap_or("?");
    format!("{name}{}", key / 12)
}

/// A titled panel.
pub fn section(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui)) {
    egui::Frame::new()
        .fill(theme::PANEL_DARK)
        .corner_radius(6.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).small().strong().color(theme::TEXT_DIM));
            add(ui);
        });
}

/// One frame of the editor: what it shows and where its edits go.
pub struct InstrumentEditor<'a> {
    kind: Kind,
    values: &'a Params,
    color: Color32,
    /// Keeps widget IDs apart between editors on one screen.
    salt: egui::Id,
    edits: &'a mut Vec<InstrumentEdit>,
    /// Called with every knob's response and its parameter name (the app's
    /// automation menu).
    menu: &'a mut dyn FnMut(&Response, &str),
    /// Whether the Functions page shows the channel arpeggiator (an
    /// instrument with its own, like Antiserum, hides it).
    arpeggiator: bool,
    /// Whether the host draws the presets elsewhere (the app's channel
    /// window header), so the Sound pages leave out their preset row.
    presets_in_header: bool,
}

impl<'a> InstrumentEditor<'a> {
    pub fn new(
        kind: Kind,
        values: &'a Params,
        color: Color32,
        salt: egui::Id,
        edits: &'a mut Vec<InstrumentEdit>,
        menu: &'a mut dyn FnMut(&Response, &str),
    ) -> Self {
        Self {
            kind,
            values,
            color,
            salt,
            edits,
            menu,
            arpeggiator: true,
            presets_in_header: false,
        }
    }

    /// Leaves the preset row off the Sound pages: the host shows
    /// [`Self::preset_picker`] in its own header.
    #[must_use]
    pub fn presets_in_header(mut self) -> Self {
        self.presets_in_header = true;
        self
    }

    /// The previous chevron, preset list and next chevron for this
    /// instrument's presets, in the caller's row (nothing when it has none).
    pub fn preset_picker(&mut self, ui: &mut Ui) {
        if let Some(list) = presets(self.kind) {
            self.picker(ui, list);
        }
    }

    /// Leaves the channel arpeggiator off the Functions page, for an
    /// instrument with its own.
    #[must_use]
    pub fn without_arpeggiator(mut self) -> Self {
        self.arpeggiator = false;
        self
    }

    fn get(&self, index: usize) -> f32 {
        self.values.get(index).copied().unwrap_or(0.0)
    }

    /// One undoable change.
    fn set_once(&mut self, index: usize, value: f32) {
        self.edits.push(InstrumentEdit::Begin);
        self.edits.push(InstrumentEdit::Set(index, value));
    }

    /// A knob for parameter `index`.
    pub fn knob(&mut self, ui: &mut Ui, index: usize, label: &str) {
        let Some(spec) = INSTRUMENT_PARAMS.get(index).copied() else {
            return;
        };
        let value = self.get(index);
        let out = widgets::stepped_labeled_knob(
            ui,
            label,
            spec.to_unit(value),
            spec.to_unit(spec.default),
            spec.min < 0.0 && spec.max > 0.0,
            &spec.format(value),
            spec.steps(),
        );
        (self.menu)(&out.response, spec.name);
        if out.started {
            self.edits.push(InstrumentEdit::Begin);
        }
        if let Some(t) = out.changed {
            let new = spec.from_unit(t);
            if new != value {
                self.edits.push(InstrumentEdit::Set(index, new));
            }
        }
    }

    /// A row of buttons choosing an integer parameter.
    pub fn choice(&mut self, ui: &mut Ui, index: usize, labels: &[&str]) {
        let current = self.get(index).round() as usize;
        for (i, label) in labels.iter().enumerate() {
            if ui.selectable_label(i == current, *label).clicked() && i != current {
                self.set_once(index, i as f32);
            }
        }
    }

    /// A drop-down choosing an integer parameter.
    pub fn combo(&mut self, ui: &mut Ui, index: usize, labels: &[&str]) {
        let current = self.get(index).round() as usize;
        let mut chosen = None;
        egui::ComboBox::from_id_salt(("instrument-combo", self.salt, index))
            .width(84.0)
            .selected_text(labels.get(current).copied().unwrap_or("?"))
            .show_ui(ui, |ui| {
                for (i, label) in labels.iter().enumerate() {
                    if ui.selectable_label(i == current, *label).clicked() && i != current {
                        chosen = Some(i);
                    }
                }
            });
        if let Some(i) = chosen {
            self.set_once(index, i as f32);
        }
    }

    /// A checkbox for an on/off parameter.
    pub fn toggle(&mut self, ui: &mut Ui, index: usize, text: &str, hint: &str) {
        let mut on = self.get(index) >= 0.5;
        if crate::widgets::led_switch(ui, &mut on, text)
            .hint(hint)
            .changed()
        {
            self.set_once(index, if on { 1.0 } else { 0.0 });
        }
    }

    /// A number field for an integer parameter (dragging is one gesture).
    pub fn drag(
        &mut self,
        ui: &mut Ui,
        index: usize,
        format: Option<fn(u8) -> String>,
    ) -> Response {
        let Some(spec) = INSTRUMENT_PARAMS.get(index).copied() else {
            return ui.label("");
        };
        let current = self.get(index).round() as i64;
        let mut value = current;
        let mut field = egui::DragValue::new(&mut value)
            .range(spec.min as i64..=spec.max as i64)
            .speed(0.2);
        if let Some(format) = format {
            field = field.custom_formatter(move |v, _| format(v.clamp(0.0, 127.0) as u8));
        }
        let response = ui.add(field);
        if response.drag_started() {
            self.edits.push(InstrumentEdit::Begin);
        }
        if value != current {
            if !response.dragged() {
                self.edits.push(InstrumentEdit::Begin);
            }
            self.edits.push(InstrumentEdit::Set(index, value as f32));
        }
        response
    }

    /// The preset picker: choosing a preset is one step.
    pub fn preset_bar(&mut self, ui: &mut Ui, list: &'static [Preset]) {
        if self.presets_in_header {
            return;
        }
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("PRESET")
                    .small()
                    .strong()
                    .color(theme::TEXT_DIM),
            );
            self.picker(ui, list);
        });
    }

    fn picker(&mut self, ui: &mut Ui, list: &'static [Preset]) {
        let Some(n) = (!list.is_empty()).then_some(list.len()) else {
            return;
        };
        let current = list
            .iter()
            .position(|p| p.matches(INSTRUMENT_PARAMS, self.values));
        let mut chosen = None;
        let salt = ("instrument-preset", self.salt, list.as_ptr() as usize);
        if crate::widgets::preset_arrow(ui, true, salt).clicked() {
            chosen = Some(current.map_or(n - 1, |c| (c + n - 1) % n));
        }
        egui::ComboBox::from_id_salt(salt)
            .width(130.0)
            .selected_text(
                current
                    .and_then(|i| list.get(i))
                    .map_or("(custom)", |p| p.name),
            )
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
            chosen = Some(current.map_or(0, |c| (c + 1) % n));
        }
        let Some(preset) = chosen.and_then(|i| list.get(i)) else {
            return;
        };
        let changes: Vec<(usize, f32)> = preset
            .resolve(INSTRUMENT_PARAMS)
            .into_iter()
            .filter_map(|(name, value)| {
                let index = musiccraft_params::index_of(INSTRUMENT_PARAMS, name)?;
                ((self.get(index) - value).abs() > 1e-6).then_some((index, value))
            })
            .collect();
        if !changes.is_empty() {
            self.edits.push(InstrumentEdit::Preset(changes));
        }
    }

    /// The page for `page`.
    pub fn page(&mut self, ui: &mut Ui, page: Page) {
        match page {
            Page::Sound => self.sound(ui),
            Page::Envelope => self.envelope(ui),
            Page::Functions => self.functions(ui),
        }
    }

    /// The Sound page's parameter sections. Sample instruments get their
    /// knobs here; the caller draws the sample itself above them.
    pub fn sound(&mut self, ui: &mut Ui) {
        match self.kind {
            Kind::Synth => self.synth(ui),
            Kind::DrumSynth => self.drum(ui),
            Kind::Pluck => self.pluck(ui),
            Kind::Fm => self.fm(ui),
            Kind::Keys => self.keys(ui),
            Kind::Additive => self.additive(ui),
            Kind::Sampler => self.sampler(ui, &mut |_, _| {}),
            Kind::Granular => self.granular(ui),
            Kind::Resynth => self.resynth(ui),
            Kind::Kit => self.kit(ui),
            Kind::Zones => self.zones(ui),
            Kind::Layer | Kind::External | Kind::MidiOut => {}
        }
    }

    /// The kit's playback: the key of pad 1 and the pads' fades.
    pub fn kit(&mut self, ui: &mut Ui) {
        section(ui, "PLAYBACK", |ui| {
            ui.horizontal(|ui| {
                ui.label("Pad 1 key");
                self.drag(ui, inst::ROOT_KEY, Some(key_name))
                    .hint("Pad n plays on this key + n − 1");
                self.knob(ui, inst::FADE_IN, "Fade in");
                self.knob(ui, inst::FADE_OUT, "Fade out");
            });
        });
    }

    /// The zones' playback: fades, glide, one-shot and reverse.
    pub fn zones(&mut self, ui: &mut Ui) {
        section(ui, "PLAYBACK", |ui| {
            ui.horizontal(|ui| {
                self.knob(ui, inst::FADE_IN, "Fade in");
                self.knob(ui, inst::FADE_OUT, "Fade out");
                self.knob(ui, inst::GLIDE, "Glide");
            });
            ui.horizontal(|ui| {
                self.toggle(
                    ui,
                    inst::ONE_SHOT,
                    "One shot",
                    "Play each zone's whole sample regardless of note length",
                );
                self.toggle(ui, inst::REVERSE, "Reverse", "Play the samples backwards");
            });
        });
    }

    /// The amplitude envelope knobs and their graph.
    pub fn envelope(&mut self, ui: &mut Ui) {
        section(ui, "ENVELOPE", |ui| {
            self.envelope_graph(ui);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                self.knob(ui, inst::ATTACK, "Attack");
                self.knob(ui, inst::DECAY, "Decay");
                self.knob(ui, inst::SUSTAIN, "Sustain");
                self.knob(ui, inst::RELEASE, "Release");
            });
        });
    }

    /// The envelope's graph with a round handle at each stage's corner: the
    /// peak sets the attack, the next corner the decay (across) and sustain
    /// (up and down), the end the release. One drag is one undo step.
    fn envelope_graph(&mut self, ui: &mut Ui) {
        let (rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width().max(160.0), 120.0), Sense::hover());
        ui.painter().rect_filled(rect, 4.0, theme::LCD_BG);
        let shape = EnvelopeShape::new(rect.shrink(8.0));
        let values = [inst::ATTACK, inst::DECAY, inst::SUSTAIN, inst::RELEASE].map(|i| self.get(i));
        let corners = shape.corners(values);
        ui.painter_at(rect)
            .add(Shape::line(corners.to_vec(), Stroke::new(2.0, self.color)));
        let handles = [
            (1, "Attack: drag across"),
            (
                2,
                "Decay and sustain: drag across for the decay, up or down for the sustain",
            ),
            (4, "Release: drag across"),
        ];
        for (corner, hint) in handles {
            let at = corners[corner];
            let response = ui
                .interact(
                    egui::Rect::from_center_size(at, vec2(14.0, 14.0)),
                    self.salt.with(("envelope-handle", corner)),
                    Sense::drag(),
                )
                .hint(hint);
            let lit = response.hovered() || response.dragged();
            ui.painter()
                .circle_filled(at, if lit { 5.5 } else { 4.5 }, self.color);
            if response.drag_started() {
                self.edits.push(InstrumentEdit::Begin);
            }
            let Some(pointer) = response
                .interact_pointer_pos()
                .filter(|_| response.dragged())
            else {
                continue;
            };
            for (index, new) in shape.drag(corner, pointer, values) {
                if (new - self.get(index)).abs() > 1e-6 {
                    self.edits.push(InstrumentEdit::Set(index, new));
                }
            }
        }
    }

    /// The Functions page: voices, timing (with swing), choke, arpeggiator
    /// and echo, in the order of the style guide's Functions layout.
    pub fn functions(&mut self, ui: &mut Ui) {
        self.functions_with(ui, &mut |_, _| {});
    }

    /// [`Self::functions`] with `extra` drawn in the timing ("swing") and
    /// voices rows (a plugin's own swing amount and voice controls).
    pub fn functions_with(&mut self, ui: &mut Ui, extra: &mut dyn FnMut(&mut Ui, &str)) {
        ui.columns(2, |row| {
            section(&mut row[0], "VOICES", |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.knob(ui, inst::POLY_MAX, "Max");
                    self.choice(ui, inst::MONO, &["Poly", "Mono"]);
                    extra(ui, "voices");
                });
            });
            section(&mut row[1], "TIMING", |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.knob(ui, inst::TIME_SHIFT, "Delay");
                    self.knob(ui, inst::GATE, "Length");
                    extra(ui, "swing");
                    self.knob(ui, inst::SWING_MIX, "Swing");
                });
                ui.horizontal_wrapped(|ui| {
                    self.choice(ui, inst::SWING_TRUNCATE, &["Delay", "Shorten"]);
                });
            });
        });
        ui.add_space(6.0);
        if self.arpeggiator {
            self.arpeggiator_section(ui);
            ui.add_space(6.0);
        }
        ui.columns(2, |row| {
            section(&mut row[0], "CHOKE", |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.knob(ui, inst::CUT_GROUP, "Group");
                    self.knob(ui, inst::CUT_BY, "Silenced by");
                });
                self.self_choke(ui);
            });
            section(&mut row[1], "ECHO", |ui| {
                ui.horizontal_wrapped(|ui| {
                    self.knob(ui, inst::ECHO_COUNT, "Repeats");
                    self.knob(ui, inst::ECHO_TIME, "Time");
                    self.knob(ui, inst::ECHO_FEED, "Feedback");
                    self.knob(ui, inst::ECHO_PITCH, "Pitch step");
                });
            });
        });
    }

    /// "Self-choke": puts the channel in one group as both its group and the
    /// group that silences it, so each new note cuts the last (one step).
    fn self_choke(&mut self, ui: &mut Ui) {
        let shared = self_choke_group(self.get(inst::CUT_GROUP), self.get(inst::CUT_BY));
        let clicked = ui
            .selectable_label(shared.is_none(), "Self-choke")
            .on_hover_text("Each new note silences this channel's last one")
            .clicked();
        if let Some(shared) = shared.filter(|_| clicked) {
            self.edits.push(InstrumentEdit::Preset(vec![
                (inst::CUT_GROUP, shared),
                (inst::CUT_BY, shared),
            ]));
        }
    }

    fn arpeggiator_section(&mut self, ui: &mut Ui) {
        section(ui, "ARPEGGIATOR", |ui| {
            ui.horizontal(|ui| {
                self.choice(
                    ui,
                    inst::ARP_MODE,
                    &["Off", "Up", "Down", "Up-down", "Random"],
                );
            });
            if self.get(inst::ARP_MODE) >= 0.5 {
                ui.horizontal(|ui| {
                    let labels: Vec<&str> = ARP_RATES.iter().map(|(l, _)| *l).collect();
                    self.choice(ui, inst::ARP_RATE, &labels);
                });
                ui.horizontal(|ui| {
                    self.knob(ui, inst::ARP_RANGE, "Octaves");
                    self.knob(ui, inst::ARP_GATE, "Gate");
                });
            }
        });
    }

    /// The sampler's knobs, switches, root key and slices. `slices_row` adds
    /// the caller's controls to the slices row, given the slice count.
    pub fn sampler(&mut self, ui: &mut Ui, slices_row: &mut dyn FnMut(&mut Ui, u32)) {
        ui.horizontal(|ui| {
            self.knob(ui, inst::FADE_IN, "Fade in");
            self.knob(ui, inst::FADE_OUT, "Fade out");
        });
        ui.horizontal(|ui| {
            self.toggle(
                ui,
                inst::ONE_SHOT,
                "One shot",
                "Play the whole sample regardless of note length",
            );
            self.toggle(ui, inst::REVERSE, "Reverse", "Play the sample backwards");
            ui.add_space(12.0);
            ui.label("Root key");
            self.drag(ui, inst::ROOT_KEY, Some(key_name));
        });
        ui.horizontal(|ui| {
            ui.label("Slices");
            self.drag(ui, inst::SLICES, None)
                .hint("Cut the sample into equal slices; key root + n plays slice n");
            let slices = self.get(inst::SLICES).round().max(1.0) as u32;
            slices_row(ui, slices);
        });
    }

    /// The granular instrument's knobs.
    pub fn granular(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            self.knob(ui, inst::GRAN_POSITION, "Position");
            self.knob(ui, inst::GRAN_SPREAD, "Spread");
            self.knob(ui, inst::GRAN_SIZE, "Size");
            self.knob(ui, inst::GRAN_DENSITY, "Density");
            self.knob(ui, inst::GRAN_PITCH_RAND, "Detune");
            self.knob(ui, inst::GRAN_SCAN, "Scan");
        });
    }

    /// The resynth's knobs.
    pub fn resynth(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new(
                "The sample's harmonics are measured once and played on every note's pitch.",
            )
            .small()
            .color(theme::TEXT_DIM),
        );
        ui.horizontal(|ui| {
            self.knob(ui, inst::RESYN_SPEED, "Speed");
            self.knob(ui, inst::RESYN_POSITION, "Start");
            self.knob(ui, inst::RESYN_BLUR, "Blur");
            self.knob(ui, inst::ADD_INHARM, "Stretch");
            self.knob(ui, inst::ADD_PARTIALS, "Partials");
        });
        ui.horizontal(|ui| {
            self.choice(ui, inst::RESYN_LOOP, &["Hold end", "Loop"]);
        });
    }

    fn drum(&mut self, ui: &mut Ui) {
        self.preset_bar(ui, presets::DRUM);
        ui.add_space(4.0);
        section(ui, "DRUM", |ui| {
            ui.horizontal(|ui| {
                self.choice(
                    ui,
                    inst::DRUM_TYPE,
                    &["Kick", "Snare", "Hat", "Tom", "Clap"],
                );
            });
            ui.horizontal(|ui| {
                self.knob(ui, inst::DRUM_TUNE, "Tune");
                self.knob(ui, inst::DRUM_DECAY, "Decay");
                self.knob(ui, inst::DRUM_SWEEP, "Sweep");
                self.knob(ui, inst::DRUM_TONE, "Tone");
                self.knob(ui, inst::DRUM_CLICK, "Click");
            });
        });
    }

    fn fm(&mut self, ui: &mut Ui) {
        self.preset_bar(ui, presets::FM);
        ui.add_space(4.0);
        let six = self.get(inst::FM_ALGO).round() >= 6.0;
        section(ui, "ALGORITHM", |ui| {
            ui.horizontal(|ui| {
                self.combo(ui, inst::FM_ALGO, &FM_ALGOS);
            });
            ui.horizontal(|ui| {
                let label = if six { "Feedback 6" } else { "Feedback 4" };
                self.knob(ui, inst::FM_FEEDBACK, label);
            });
        });
        for op in 0..if six { 6 } else { 4 } {
            ui.add_space(4.0);
            section(ui, &format!("OPERATOR {}", op + 1), |ui| {
                ui.horizontal(|ui| {
                    self.combo(ui, inst::OP1_WAVE + op, &FM_WAVES);
                });
                ui.horizontal(|ui| {
                    let base = if op < 4 {
                        inst::OP1_RATIO + op * 4
                    } else {
                        inst::OP5_RATIO + (op - 4) * 4
                    };
                    self.knob(ui, base, "Ratio");
                    self.knob(ui, base + 1, "Level");
                    self.knob(ui, base + 2, "Decay");
                    self.knob(ui, base + 3, "Sustain");
                });
            });
        }
        ui.add_space(4.0);
        section(ui, "FILTER", |ui| {
            ui.horizontal(|ui| {
                self.knob(ui, inst::FM_CUTOFF, "Cutoff");
                self.knob(ui, inst::FM_RESONANCE, "Reso");
            });
        });
    }

    fn keys(&mut self, ui: &mut Ui) {
        self.preset_bar(ui, presets::KEYS);
        ui.add_space(4.0);
        section(ui, "KEYS", |ui| {
            ui.horizontal(|ui| {
                self.choice(ui, inst::KEYS_TYPE, &["Piano", "Electric piano", "Organ"]);
            });
            ui.horizontal(|ui| {
                self.knob(ui, inst::KEYS_TONE, "Tone");
                self.knob(ui, inst::KEYS_DECAY, "Ring");
                self.knob(ui, inst::KEYS_TREMOLO, "Tremolo");
                self.knob(ui, inst::KEYS_HAMMER, "Hammer");
            });
        });
    }

    fn additive(&mut self, ui: &mut Ui) {
        self.preset_bar(ui, presets::ADDITIVE);
        ui.add_space(4.0);
        section(ui, "HARMONICS", |ui| {
            ui.horizontal(|ui| {
                self.knob(ui, inst::ADD_PARTIALS, "Harmonics");
                self.knob(ui, inst::ADD_BRIGHT, "Bright");
                self.knob(ui, inst::ADD_EVEN, "Even");
                self.knob(ui, inst::ADD_INHARM, "Stretch");
                self.knob(ui, inst::ADD_DECAY, "Fade");
            });
            harmonics_graph(ui, self.values);
        });
        ui.add_space(6.0);
        section(ui, "MORPH TO SPECTRUM B", |ui| {
            ui.horizontal(|ui| {
                self.knob(ui, inst::ADD_BRIGHT_B, "Bright B");
                self.knob(ui, inst::ADD_EVEN_B, "Even B");
                self.knob(ui, inst::ADD_INHARM_B, "Stretch B");
            });
            ui.horizontal(|ui| {
                self.knob(ui, inst::ADD_MORPH, "Morph");
                self.knob(ui, inst::ADD_MORPH_TIME, "Sweep");
                self.knob(ui, inst::ADD_MORPH_RATE, "LFO rate");
                self.knob(ui, inst::ADD_MORPH_DEPTH, "LFO depth");
            });
        });
    }

    fn pluck(&mut self, ui: &mut Ui) {
        self.preset_bar(ui, presets::PLUCK);
        ui.add_space(4.0);
        section(ui, "STRING", |ui| {
            ui.horizontal(|ui| {
                self.knob(ui, inst::PLUCK_DECAY, "Decay");
                self.knob(ui, inst::PLUCK_DAMPING, "Damping");
                self.knob(ui, inst::PLUCK_TONE, "Tone");
            });
            ui.horizontal(|ui| {
                self.knob(ui, inst::PLUCK_BOW, "Bow");
                self.knob(ui, inst::PLUCK_STRINGS, "Strings");
                self.knob(ui, inst::PLUCK_DETUNE, "Detune");
            });
        });
    }

    fn synth(&mut self, ui: &mut Ui) {
        self.preset_bar(ui, presets::SYNTH);
        ui.add_space(4.0);
        section(ui, "OSCILLATORS", |ui| {
            for (label, param) in [
                ("Osc 1", inst::WAVE1),
                ("Osc 2", inst::WAVE2),
                ("Osc 3", inst::WAVE3),
            ] {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        vec2(44.0, 20.0),
                        egui::Label::new(RichText::new(label).small()),
                    );
                    let current = self.get(param).round() as usize;
                    for (wave, name) in WAVEFORMS.iter().enumerate() {
                        if wave_button(ui, wave, wave == current).hint(*name).clicked()
                            && wave != current
                        {
                            self.set_once(param, wave as f32);
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                self.knob(ui, inst::DETUNE, "Detune");
                self.knob(ui, inst::OSC_MIX, "Mix");
                self.knob(ui, inst::SUB, "Sub");
                self.knob(ui, inst::GLIDE, "Glide");
                self.knob(ui, inst::UNISON, "Unison");
                self.knob(ui, inst::SPREAD, "Spread");
                self.knob(ui, inst::DRIVE, "Drive");
            });
            ui.horizontal(|ui| {
                self.knob(ui, inst::OSC3_LEVEL, "Osc 3");
                self.knob(ui, inst::OSC3_COARSE, "Osc 3 pitch");
            });
        });
        ui.add_space(6.0);
        section(ui, "FILTER", |ui| {
            ui.horizontal(|ui| {
                self.choice(
                    ui,
                    inst::FILTER_TYPE,
                    &["Low-pass", "Band-pass", "High-pass"],
                );
            });
            ui.horizontal(|ui| {
                self.knob(ui, inst::CUTOFF, "Cutoff");
                self.knob(ui, inst::RESONANCE, "Reso");
                self.knob(ui, inst::ENV_AMOUNT, "Env amt");
                filter_graph(ui, self.values, theme::ACCENT);
            });
        });
        ui.add_space(6.0);
        section(ui, "LFO", |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("To").small());
                self.choice(ui, inst::LFO_TARGET, &["Pitch", "Cutoff", "Volume"]);
            });
            ui.horizontal(|ui| {
                self.knob(ui, inst::LFO_RATE, "Rate");
                self.knob(ui, inst::LFO_DEPTH, "Depth");
            });
        });
        ui.add_space(6.0);
        section(ui, "FILTER 2", |ui| {
            ui.horizontal(|ui| {
                self.choice(
                    ui,
                    inst::FILTER2_TYPE,
                    &["Off", "Low-pass", "Band-pass", "High-pass"],
                );
            });
            if self.get(inst::FILTER2_TYPE) >= 0.5 {
                ui.horizontal(|ui| {
                    self.choice(ui, inst::FILTER_ROUTING, &["Series", "Parallel", "Split"]);
                });
                ui.horizontal(|ui| {
                    self.knob(ui, inst::CUTOFF2, "Cutoff");
                    self.knob(ui, inst::RESONANCE2, "Reso");
                });
            }
        });
        ui.add_space(6.0);
        section(ui, "LFO 2 · MOD ENV", |ui| {
            ui.horizontal(|ui| {
                self.choice(
                    ui,
                    inst::LFO2_SHAPE,
                    &["Sine", "Tri", "Saw", "Square", "Random"],
                );
            });
            ui.horizontal(|ui| {
                self.knob(ui, inst::LFO2_RATE, "Rate");
                self.knob(ui, inst::ENV2_ATTACK, "Attack");
                self.knob(ui, inst::ENV2_DECAY, "Decay");
            });
        });
        ui.add_space(6.0);
        section(ui, "MOD MATRIX", |ui| {
            for slot in 0..MOD_SLOTS {
                let base = inst::MOD1_SOURCE + slot * 3;
                ui.horizontal(|ui| {
                    self.combo(ui, base, &MOD_SOURCES);
                    ui.label(RichText::new("to").small());
                    self.combo(ui, base + 1, &MOD_DESTS);
                    self.knob(ui, base + 2, "Amount");
                });
            }
        });
    }
}

/// A parameter's spec.
fn spec(index: usize) -> Option<ParamSpec> {
    INSTRUMENT_PARAMS.get(index).copied()
}

fn value(values: &Params, index: usize) -> f32 {
    values
        .get(index)
        .copied()
        .unwrap_or_else(|| spec(index).map_or(0.0, |s| s.default))
}

/// Bars for each harmonic's level.
fn harmonics_graph(ui: &mut Ui, values: &Params) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 50.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme::LCD_BG);
    let count = value(values, inst::ADD_PARTIALS).round().max(1.0) as usize;
    let slope = 2.0 - 1.6 * value(values, inst::ADD_BRIGHT);
    let even = value(values, inst::ADD_EVEN);
    let w = rect.width() / 32.0;
    for n in 1..=count {
        let level = (n as f32).powf(-slope) * if n % 2 == 0 { even } else { 1.0 };
        let x = rect.left() + (n - 1) as f32 * w;
        let top = rect.bottom() - level.min(1.0) * (rect.height() - 4.0);
        painter.rect_filled(
            egui::Rect::from_min_max(pos2(x + 1.0, top), pos2(x + w - 1.0, rect.bottom())),
            1.0,
            theme::ACCENT,
        );
    }
}

/// The synth filter's response at the current settings.
fn filter_graph(ui: &mut Ui, values: &Params, color: Color32) {
    use musiccraft_dsp::{SvfMode, response};
    let (rect, _) = ui.allocate_exact_size(vec2(150.0, 54.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme::LCD_BG);
    let mode = SvfMode::from_param(value(values, inst::FILTER_TYPE));
    let cutoff = value(values, inst::CUTOFF);
    let resonance = value(values, inst::RESONANCE);
    let width = rect.width() as usize;
    let points: Vec<Pos2> = (0..=width)
        .map(|i| {
            let t = i as f32 / width as f32;
            let f = 20.0 * 1_000.0_f32.powf(t);
            let db = response::svf_db(mode, cutoff, resonance, f, 48_000.0).clamp(-40.0, 20.0);
            pos2(
                rect.left() + i as f32,
                rect.top() + rect.height() * (20.0 - db) / 60.0,
            )
        })
        .collect();
    painter.add(Shape::line(points, Stroke::new(1.5, color)));
}

/// A button showing one oscillator wave shape.
fn wave_button(ui: &mut Ui, wave: usize, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(vec2(34.0, 22.0), Sense::click());
    let fill = if selected {
        theme::ACCENT_DIM
    } else if response.hovered() {
        theme::RAISED_HOVER
    } else {
        theme::RAISED
    };
    ui.painter().rect_filled(rect, 4.0, fill);
    let r = rect.shrink2(vec2(6.0, 5.0));
    let at = |x: f32, y: f32| {
        pos2(
            r.left() + r.width() * x,
            r.center().y - r.height() / 2.0 * y,
        )
    };
    const NOISE: [f32; 17] = [
        0.2, -0.7, 0.9, -0.3, 0.6, -1.0, 0.1, 0.8, -0.5, 0.3, -0.9, 0.7, -0.2, 0.5, -0.6, 0.9, -0.1,
    ];
    let points: Vec<Pos2> = match wave {
        0 => (0..=24)
            .map(|i| {
                let x = i as f32 / 24.0;
                at(x, (x * std::f32::consts::TAU).sin())
            })
            .collect(),
        1 => vec![at(0.0, -1.0), at(0.5, 1.0), at(0.5, -1.0), at(1.0, 1.0)],
        2 => vec![at(0.0, 1.0), at(0.5, 1.0), at(0.5, -1.0), at(1.0, -1.0)],
        3 => vec![at(0.0, 0.0), at(0.25, 1.0), at(0.75, -1.0), at(1.0, 0.0)],
        _ => NOISE
            .iter()
            .enumerate()
            .map(|(i, y)| at(i as f32 / 16.0, *y))
            .collect(),
    };
    let color = if selected {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    ui.painter()
        .add(Shape::line(points, Stroke::new(1.5, color)));
    response
}

/// Min/max overview of a sample, with numbered slice boundaries.
pub fn waveform(ui: &mut Ui, overview: Option<&[(f32, f32)]>, color: Color32, slices: u32) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 72.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme::LCD_BG);
    painter.line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0, theme::LCD_EDGE),
    );
    let Some(points) = overview.filter(|p| !p.is_empty()) else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "no audio",
            egui::FontId::proportional(theme::FONT_LABEL),
            theme::TEXT_DIM,
        );
        return;
    };
    let peak = points
        .iter()
        .fold(0.0_f32, |p, (lo, hi)| p.max(lo.abs()).max(hi.abs()))
        .max(1e-6);
    let step = rect.width() / points.len() as f32;
    for (i, (lo, hi)) in points.iter().enumerate() {
        let x = rect.left() + (i as f32 + 0.5) * step;
        let half = rect.height() * 0.45 / peak;
        painter.line_segment(
            [
                pos2(x, rect.center().y - hi * half),
                pos2(x, rect.center().y - lo * half - 0.5),
            ],
            Stroke::new(step.max(1.0), color),
        );
    }
    if slices > 1 {
        for i in 0..slices {
            let x = rect.left() + rect.width() * i as f32 / slices as f32;
            if i > 0 {
                painter.line_segment(
                    [pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(1.0, theme::HIGHLIGHT),
                );
            }
            if slices <= 16 {
                painter.text(
                    pos2(x + 3.0, rect.top() + 2.0),
                    egui::Align2::LEFT_TOP,
                    (i + 1).to_string(),
                    egui::FontId::proportional(theme::FONT_CAPTION),
                    theme::HIGHLIGHT,
                );
            }
        }
    }
}

/// The envelope shape: attack, decay, a fixed sustain segment, release.
/// Where the envelope graph draws its stages. Each timed stage has a
/// quarter of the width, with time on a square-root scale up to the
/// parameter's 5 s maximum, so short stages stay visible and a drag maps
/// back to the same time; the sustain hold is a fixed quarter.
struct EnvelopeShape {
    inner: egui::Rect,
}

impl EnvelopeShape {
    const MAX_SECONDS: f32 = 5.0;

    fn new(inner: egui::Rect) -> Self {
        Self { inner }
    }

    fn quarter(&self) -> f32 {
        self.inner.width() / 4.0
    }

    fn width_of(&self, seconds: f32) -> f32 {
        (seconds.max(0.0) / Self::MAX_SECONDS).sqrt().min(1.0) * self.quarter()
    }

    fn seconds_at(&self, width: f32) -> f32 {
        let t = (width / self.quarter()).clamp(0.0, 1.0);
        t * t * Self::MAX_SECONDS
    }

    fn y(&self, level: f32) -> f32 {
        self.inner.bottom() - level.clamp(0.0, 1.0) * self.inner.height()
    }

    /// Start, peak, decay end, hold end and release end, from attack, decay,
    /// sustain and release.
    fn corners(&self, [a, d, s, r]: [f32; 4]) -> [Pos2; 5] {
        let x0 = self.inner.left();
        let x1 = x0 + self.width_of(a);
        let x2 = x1 + self.width_of(d);
        let x3 = x2 + self.quarter();
        let x4 = x3 + self.width_of(r);
        let (top, bottom) = (self.y(1.0), self.y(0.0));
        [
            pos2(x0, bottom),
            pos2(x1, top),
            pos2(x2, self.y(s)),
            pos2(x3, self.y(s)),
            pos2(x4, bottom),
        ]
    }

    /// The parameters a drag of `corner` to `pointer` sets.
    fn drag(&self, corner: usize, pointer: Pos2, values: [f32; 4]) -> Vec<(usize, f32)> {
        let corners = self.corners(values);
        match corner {
            1 => vec![(inst::ATTACK, self.seconds_at(pointer.x - corners[0].x))],
            2 => {
                let level = (self.inner.bottom() - pointer.y) / self.inner.height();
                vec![
                    (inst::DECAY, self.seconds_at(pointer.x - corners[1].x)),
                    (inst::SUSTAIN, level.clamp(0.0, 1.0)),
                ]
            }
            4 => vec![(inst::RELEASE, self.seconds_at(pointer.x - corners[3].x))],
            _ => Vec::new(),
        }
    }
}

/// The group "Self-choke" puts a channel in, given its choke group and the
/// group that silences it: the first one set, else group 1. `None` when it
/// already chokes itself.
fn self_choke_group(group: f32, by: f32) -> Option<f32> {
    if group > 0.0 && group == by {
        return None;
    }
    Some([group, by].into_iter().find(|g| *g > 0.0).unwrap_or(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{RawInput, Rect};
    use musiccraft_instruments::{KINDS, default_params};

    fn frame(kind: Kind, page: Page, values: &Params) -> Vec<InstrumentEdit> {
        let ctx = egui::Context::default();
        let mut edits = Vec::new();
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(480.0, 1400.0))),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            let mut menu = |_: &Response, _: &str| {};
            let mut editor = InstrumentEditor::new(
                kind,
                values,
                theme::ACCENT,
                egui::Id::new("test"),
                &mut edits,
                &mut menu,
            );
            editor.page(ui, page);
        });
        output.textures_delta.clear();
        edits
    }

    #[test]
    fn every_instrument_and_page_draws_without_edits() {
        let values = default_params();
        for kind in KINDS {
            for page in [Page::Sound, Page::Envelope, Page::Functions] {
                assert!(frame(kind, page, &values).is_empty(), "{kind:?} {page:?}");
            }
        }
        assert!(has_envelope(Kind::Synth) && !has_envelope(Kind::Pluck));
        assert!(presets(Kind::Fm).is_some() && presets(Kind::Sampler).is_none());
        assert_eq!(key_name(60), "C5");
    }

    #[test]
    fn a_preset_reports_only_what_it_changes() {
        let list = presets::SYNTH;
        let preset = &list[1];
        let mut values = default_params();
        for (name, v) in preset.resolve(INSTRUMENT_PARAMS) {
            values[musiccraft_params::index_of(INSTRUMENT_PARAMS, name).unwrap()] = v;
        }
        assert!(preset.matches(INSTRUMENT_PARAMS, &values));
    }

    #[test]
    fn self_choke_shares_the_first_group_set() {
        assert_eq!(self_choke_group(0.0, 0.0), Some(1.0));
        assert_eq!(self_choke_group(3.0, 0.0), Some(3.0));
        assert_eq!(self_choke_group(0.0, 5.0), Some(5.0));
        assert_eq!(self_choke_group(2.0, 4.0), Some(2.0));
        assert_eq!(self_choke_group(4.0, 4.0), None);
    }

    #[test]
    fn envelope_handles_map_back_to_their_stage() {
        let shape = EnvelopeShape::new(egui::Rect::from_min_size(Pos2::ZERO, vec2(400.0, 100.0)));
        let values = [0.2, 0.3, 0.8, 1.25];
        let corners = shape.corners(values);
        // Dragging each handle onto itself gives back the same values.
        assert!((shape.drag(1, corners[1], values)[0].1 - 0.2).abs() < 1e-4);
        let decay = shape.drag(2, corners[2], values);
        assert!((decay[0].1 - 0.3).abs() < 1e-4 && (decay[1].1 - 0.8).abs() < 1e-4);
        assert!((shape.drag(4, corners[4], values)[0].1 - 1.25).abs() < 1e-4);
        // Past the end of its quarter a stage stops at the maximum.
        let far = shape.drag(1, pos2(1_000.0, 0.0), values);
        assert!((far[0].1 - EnvelopeShape::MAX_SECONDS).abs() < 1e-4);
    }
}
