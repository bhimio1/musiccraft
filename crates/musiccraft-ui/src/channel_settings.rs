//! Channel settings: the instrument editor for the selected channel.
//!
//! The instruments' own pages come from the shared editor
//! (`musiccraft_editors::instrument`), which the instrument plugins use too;
//! this module turns its edits into commands and adds what needs the
//! project: the header, sample dialogs, the sample editor, kit pads, zones,
//! layers and MIDI out.

use crate::hint::Hint;
use egui::{RichText, vec2};
use musiccraft_commands::Invocation;
use musiccraft_editors::instrument::{self, InstrumentEdit, InstrumentEditor, key_name, section};
use musiccraft_instruments::Kind;
use musiccraft_project::params::{INSTRUMENT_PARAMS, MIDI_CCS, ParamSpec, inst};
use musiccraft_project::{Channel, DEFAULT_CHANNEL_VOLUME, Instrument, MAX_VOLUME};

use crate::dialogs::{FilePick, Pick};
use crate::icons::{self, Icon};
use crate::sample_slot;
use crate::{Action, Frame, UiState, theme, widgets};

/// Pages of the channel settings window.
pub use musiccraft_editors::instrument::Page as ChannelTab;

fn spec(index: usize) -> ParamSpec {
    INSTRUMENT_PARAMS[index]
}

/// The shared editor's instrument kind for a channel (plugins and Antiserum
/// only use its Functions page).
fn kind_of(instrument: &Instrument) -> Kind {
    match instrument {
        Instrument::Sampler { .. } => Kind::Sampler,
        Instrument::Synth => Kind::Synth,
        Instrument::DrumSynth => Kind::DrumSynth,
        Instrument::Pluck => Kind::Pluck,
        Instrument::Fm => Kind::Fm,
        Instrument::Keys => Kind::Keys,
        Instrument::Additive => Kind::Additive,
        Instrument::Granular { .. } => Kind::Granular,
        Instrument::Resynth { .. } => Kind::Resynth,
        Instrument::Zones { .. } => Kind::Zones,
        Instrument::Kit { .. } => Kind::Kit,
        Instrument::Layer { .. } => Kind::Layer,
        Instrument::MidiOut { .. } => Kind::MidiOut,
        Instrument::Plugin { .. } | Instrument::Antiserum { .. } => Kind::External,
    }
}

/// Runs `draw` with the shared editor for channel `index`, then turns its
/// edits into commands.
fn with_editor(
    ui: &mut egui::Ui,
    state: &mut UiState,
    index: usize,
    channel: &Channel,
    actions: &mut Vec<Action>,
    draw: impl FnOnce(&mut egui::Ui, &mut InstrumentEditor<'_>),
) {
    let number = index + 1;
    let mut edits = Vec::new();
    let mut menu_actions = Vec::new();
    let mut menu = |response: &egui::Response, name: &str| {
        crate::automation::menu(
            response,
            &format!("channel:{number}:{name}"),
            &mut menu_actions,
        );
    };
    let mut editor = InstrumentEditor::new(
        kind_of(&channel.instrument),
        &channel.params.0,
        theme::palette(channel.color_index()),
        egui::Id::new(("channel-settings", number)),
        &mut edits,
        &mut menu,
    );
    // Antiserum has its own arpeggiator (its Arp/Seq page).
    if channel.instrument.patch().is_some() {
        editor = editor.without_arpeggiator();
    }
    // The presets are in the window's header.
    editor = editor.presets_in_header();
    draw(ui, &mut editor);
    actions.extend(menu_actions);
    apply(state, number, &edits, actions);
}

/// The commands for the shared editor's `edits` on channel `number`.
fn apply(state: &mut UiState, number: usize, edits: &[InstrumentEdit], actions: &mut Vec<Action>) {
    let set = |index: usize, value: f32| {
        Invocation::new(
            "channel.set_param",
            [
                number.to_string(),
                spec(index).name.to_owned(),
                format!("{value}"),
            ],
        )
    };
    for edit in edits {
        match edit {
            InstrumentEdit::Begin => {
                state.begin_gesture();
            }
            InstrumentEdit::Set(index, value) => {
                actions.push(Action::Coalesced(set(*index, *value), state.gesture()));
            }
            InstrumentEdit::Preset(changes) => {
                let lines: Vec<String> = changes
                    .iter()
                    .map(|(index, value)| set(*index, *value).to_string())
                    .collect();
                actions.push(Action::Run(Invocation::new(
                    musiccraft_commands::BATCH,
                    lines,
                )));
            }
        }
    }
}

/// A parameter knob bound to `channel.set_param` (the header's pitch and
/// the MIDI Out page).
fn param_knob(
    ui: &mut egui::Ui,
    state: &mut UiState,
    number: usize,
    channel: &Channel,
    index: usize,
    label: &str,
    actions: &mut Vec<Action>,
) {
    let spec = spec(index);
    let value = channel.params.get(index);
    let out = widgets::stepped_labeled_knob(
        ui,
        label,
        spec.to_unit(value),
        spec.to_unit(spec.default),
        spec.min < 0.0 && spec.max > 0.0,
        &spec.format(value),
        spec.steps(),
    );
    crate::automation::menu(
        &out.response,
        &format!("channel:{number}:{}", spec.name),
        actions,
    );
    if out.started {
        state.begin_gesture();
    }
    if let Some(t) = out.changed {
        let new = spec.from_unit(t);
        if new != value {
            actions.push(Action::Coalesced(
                Invocation::new(
                    "channel.set_param",
                    [number.to_string(), spec.name.to_owned(), format!("{new}")],
                ),
                state.gesture(),
            ));
        }
    }
}

pub fn show(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    if state.windows.channel_settings {
        let mut open = true;
        let id = egui::Id::new("channel-settings");
        show_window(
            ctx,
            state,
            frame,
            (state.selected_channel, id, 0),
            &mut open,
            actions,
        );
        state.windows.channel_settings = open;
    }
    // Windows opened with Alt+click stay on their own channel and page.
    let project = frame.project;
    let main_tab = state.channel_tab;
    let mut kept = std::mem::take(&mut state.kept_channels);
    kept.retain_mut(|(channel, tab)| {
        let Some(index) = project.channel_position(*channel) else {
            return false;
        };
        let mut open = true;
        let id = egui::Id::new(("channel-settings", channel.0));
        state.channel_tab = *tab;
        let cascade = index % 8 + 1;
        show_window(ctx, state, frame, (index, id, cascade), &mut open, actions);
        *tab = state.channel_tab;
        open
    });
    state.channel_tab = main_tab;
    state.kept_channels = kept;
}

/// One channel window: channel `index`, window `id`, offset `cascade` steps
/// down and left from the default place.
fn show_window(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    (index, id, cascade): (usize, egui::Id, usize),
    open: &mut bool,
    actions: &mut Vec<Action>,
) {
    let Some(channel) = frame.project.channels.get(index) else {
        return;
    };
    let area = state.workspace;
    let title = format!(
        "{} — {}",
        channel.name,
        frame
            .project
            .mixer
            .get(channel.mixer_track)
            .map_or("Master", |t| t.name.as_str())
    );
    let step = 24.0 * cascade as f32;
    let default = egui::Rect::from_min_size(
        area.right_top() + vec2(-470.0 - step, 30.0 + step),
        vec2(440.0, 0.0),
    );
    // Never narrower than the strip; wide pages (Antiserum) widen it to at
    // most half the workspace.
    let min_width = 3.0 * 40.0 + STRIP_CONTROLS;
    let mut window = crate::chrome::window(ctx, id, "Channel settings", area, default, false)
        .max_height((area.height() - 40.0).max(300.0))
        .min_width(min_width)
        .max_width((area.width() * 0.5).max(min_width));
    if !crate::chrome::collapsed(ctx, id) {
        window = window.min_height((area.height() - 60.0).clamp(300.0, 760.0));
    }
    window.show(ctx, |ui| {
        let folded = crate::chrome::Header::new(id, &title).fixed_size().show(
            ui,
            open,
            &mut (&mut *state, &mut *actions),
            |ui, (state, actions)| {
                crate::channel_rack::context_menu(
                    ui,
                    state,
                    frame.project,
                    index,
                    channel,
                    actions,
                );
            },
            |ui, (state, actions)| {
                with_editor(ui, state, index, channel, actions, |ui, e| {
                    e.preset_picker(ui);
                });
            },
        );
        if !folded {
            contents(ui, state, frame, index, channel, actions);
        }
    });
}

fn contents(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    index: usize,
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    let number = index + 1;
    if matches!(channel.instrument, Instrument::MidiOut { .. }) {
        // Sound settings do not apply to external gear: the strip and the
        // MIDI page only.
        state.channel_tab = ChannelTab::Sound;
        strip(ui, state, frame, number, channel, false, actions);
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt("channel-settings-scroll")
            .show(ui, |ui| {
                midi_out(ui, state, frame, number, channel, actions);
            });
        return;
    }
    let has_envelope = instrument::has_envelope(kind_of(&channel.instrument));
    if state.channel_tab == ChannelTab::Envelope && !has_envelope
        || state.channel_tab != ChannelTab::Sound && is_stack(channel)
    {
        state.channel_tab = ChannelTab::Sound;
    }
    strip(ui, state, frame, number, channel, has_envelope, actions);
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("channel-settings-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| match state.channel_tab {
            ChannelTab::Sound => sound(ui, state, frame, index, channel, actions),
            ChannelTab::Envelope => {
                with_editor(ui, state, index, channel, actions, |ui, e| e.envelope(ui));
                keyboard_strip(ui, index, channel, actions);
            }
            ChannelTab::Functions => {
                with_editor(ui, state, index, channel, actions, |ui, e| e.functions(ui));
                keyboard_strip(ui, index, channel, actions);
            }
        });
}

/// The keys the keyboard strip shows: C1 to B8.
const STRIP_KEYS: std::ops::Range<u8> = 24..108;

/// The KEYBOARD box at the bottom of the Envelope and Functions pages: the
/// root key with a Reset button, then a piano. Left-click a key to hear it,
/// right-click to make it the root key. Instruments that are not tuned by
/// the root key (plugins, Antiserum) have no strip.
fn keyboard_strip(ui: &mut egui::Ui, index: usize, channel: &Channel, actions: &mut Vec<Action>) {
    if kind_of(&channel.instrument) == Kind::External {
        return;
    }
    let number = index + 1;
    let root = channel.params.get(inst::ROOT_KEY).round().clamp(0.0, 127.0) as u8;
    let set_root = |key: u8| {
        Action::Run(Invocation::new(
            "channel.set_param",
            [number.to_string(), "root_key".to_owned(), key.to_string()],
        ))
    };
    ui.add_space(6.0);
    section(ui, "KEYBOARD", |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Root {}", key_name(root))).color(theme::TEXT));
            let default = spec(inst::ROOT_KEY).default as u8;
            if crate::widgets::add_enabled(ui, root != default, egui::Button::new("Reset").small())
                .hint("Set the root key back to C5")
                .clicked()
            {
                actions.push(set_root(default));
            }
        });
        ui.add_space(2.0);
        let (rect, response) = ui.allocate_exact_size(
            vec2(ui.available_width(), 44.0),
            egui::Sense::click_and_drag(),
        );
        let layout = KeyLayout::new(rect);
        layout.paint(ui.painter(), root);
        let held_id = response.id.with("held");
        let held = ui.ctx().data(|d| d.get_temp::<u8>(held_id));
        let under = response.hover_pos().and_then(|p| layout.key_at(p));
        if let Some(key) = under {
            response.clone().hint(format!(
                "{} · click to hear it · right-click to make it the root key",
                key_name(key)
            ));
        }
        if response.secondary_clicked() {
            if let Some(key) = under.filter(|k| *k != root) {
                actions.push(set_root(key));
            }
        }
        let pressed =
            response.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_down());
        let now = if pressed { under } else { None };
        if now != held {
            if let Some(old) = held {
                actions.push(Action::Preview {
                    channel: index,
                    key: old,
                    on: false,
                });
            }
            if let Some(new) = now {
                actions.push(Action::Preview {
                    channel: index,
                    key: new,
                    on: true,
                });
            }
            ui.ctx().data_mut(|d| match now {
                Some(key) => {
                    d.insert_temp(held_id, key);
                }
                None => d.remove::<u8>(held_id),
            });
        }
    });
}

/// Where the keyboard strip's keys are: white keys share the width, black
/// keys sit over their seams in the upper part.
struct KeyLayout {
    rect: egui::Rect,
    white: f32,
}

impl KeyLayout {
    fn new(rect: egui::Rect) -> Self {
        let whites = STRIP_KEYS.filter(|k| !is_black(*k)).count() as f32;
        Self {
            rect,
            white: rect.width() / whites,
        }
    }

    /// The rectangle of `key`.
    fn key_rect(&self, key: u8) -> egui::Rect {
        let whites_before = (STRIP_KEYS.start..key).filter(|k| !is_black(*k)).count() as f32;
        let left = self.rect.left() + whites_before * self.white;
        if is_black(key) {
            let w = self.white * 0.6;
            egui::Rect::from_min_size(
                egui::pos2(left - w / 2.0, self.rect.top()),
                vec2(w, self.rect.height() * 0.6),
            )
        } else {
            egui::Rect::from_min_size(
                egui::pos2(left, self.rect.top()),
                vec2(self.white, self.rect.height()),
            )
        }
    }

    /// The key under `pos`: black keys first, as they lie on top.
    fn key_at(&self, pos: egui::Pos2) -> Option<u8> {
        if !self.rect.contains(pos) {
            return None;
        }
        STRIP_KEYS
            .clone()
            .filter(|k| is_black(*k))
            .chain(STRIP_KEYS.clone().filter(|k| !is_black(*k)))
            .find(|k| self.key_rect(*k).contains(pos))
    }

    fn paint(&self, painter: &egui::Painter, root: u8) {
        for key in STRIP_KEYS.clone().filter(|k| !is_black(*k)) {
            let r = self.key_rect(key);
            let fill = if key == root {
                theme::ACCENT
            } else {
                theme::KEY_WHITE
            };
            painter.rect_filled(r.shrink2(vec2(0.5, 0.0)), 1.0, fill);
            if key % 12 == 0 && r.width() >= 6.0 {
                painter.text(
                    egui::pos2(r.left() + 1.0, r.bottom() - 2.0),
                    egui::Align2::LEFT_BOTTOM,
                    key_name(key),
                    egui::FontId::proportional(theme::FONT_CAPTION - 1.5),
                    theme::KEY_BLACK,
                );
            }
        }
        for key in STRIP_KEYS.clone().filter(|k| is_black(*k)) {
            let fill = if key == root {
                theme::ACCENT
            } else {
                theme::KEY_BLACK
            };
            painter.rect_filled(self.key_rect(key), 1.0, fill);
        }
    }
}

fn is_black(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

/// The fixed strip under the header: the page tabs as glyph cells at the
/// left; the On light, pan, volume, pitch and the mixer track flush right.
fn strip(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    number: usize,
    channel: &Channel,
    has_envelope: bool,
    actions: &mut Vec<Action>,
) {
    egui::Frame::new()
        .fill(theme::PANEL_DARK)
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(0, 0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let tabs = [
                    (
                        ChannelTab::Sound,
                        Icon::Sine,
                        format!("Sound — the {}'s own controls", channel.instrument.label()),
                        true,
                    ),
                    (
                        ChannelTab::Envelope,
                        Icon::Envelope,
                        "Envelope — how each note's level rises and falls".to_owned(),
                        has_envelope,
                    ),
                    (
                        ChannelTab::Functions,
                        Icon::Steps,
                        "Functions — voices, timing, choke, arpeggiator and echo".to_owned(),
                        true,
                    ),
                ];
                // MIDI output and channel stacks only have the Sound page.
                let sound_only =
                    is_stack(channel) || matches!(channel.instrument, Instrument::MidiOut { .. });
                for (tab, icon, hint, shown) in tabs {
                    let shown = shown && (tab == ChannelTab::Sound || !sound_only);
                    if shown && tab_cell(ui, icon, state.channel_tab == tab, &hint).clicked() {
                        state.channel_tab = tab;
                    }
                }
                // The mixing controls sit flush right; the gap in the middle
                // takes what is left.
                let right = match &channel.instrument {
                    Instrument::MidiOut { .. } => STRIP_MIDI,
                    _ if is_stack(channel) => STRIP_ON,
                    _ => STRIP_CONTROLS,
                };
                ui.add_space((ui.available_width() - right).max(8.0));
                ui.spacing_mut().item_spacing.x = 4.0;
                on_light(ui, number, channel, actions);
                if let Instrument::MidiOut { midi_channel } = &channel.instrument {
                    midi_channel_box(ui, number, *midi_channel, actions);
                    return;
                }
                if is_stack(channel) {
                    return;
                }
                pan_knob(ui, state, number, channel, actions);
                volume_knob(ui, state, number, channel, actions);
                crate::widgets::enabled_ui(ui, channel.instrument.plugin().is_none(), |ui| {
                    param_knob(ui, state, number, channel, inst::PITCH, "Pitch", actions);
                });
                ui.add_space(24.0);
                track_box(ui, state, frame, number, channel, actions);
            });
        });
}

/// Width of the strip's right group: On, three knobs, the gap and the track
/// box, with their spacing and the right margin.
const STRIP_CONTROLS: f32 = 28.0 + 3.0 * 52.0 + 24.0 + 40.0 + 6.0 * 4.0 + 8.0;
/// The right group of a channel stack: the On light only.
const STRIP_ON: f32 = 28.0 + 12.0;
/// The right group of a MIDI output channel: On and the MIDI channel box.
const STRIP_MIDI: f32 = 28.0 + 4.0 + 52.0 + 12.0;

/// Whether the channel is a channel stack (it makes no sound of its own).
fn is_stack(channel: &Channel) -> bool {
    matches!(channel.instrument, Instrument::Layer { .. })
}

/// The MIDI output strip's channel box: 1–16 over a "MIDI ch" caption.
fn midi_channel_box(ui: &mut egui::Ui, number: usize, midi_channel: u8, actions: &mut Vec<Action>) {
    ui.vertical(|ui| {
        ui.set_width(52.0);
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            let mut value = midi_channel;
            let response = ui
                .add(egui::DragValue::new(&mut value).range(1..=16))
                .hint("The MIDI channel (1–16) this channel's notes are sent on");
            if response.changed() && value != midi_channel {
                actions.push(Action::Run(Invocation::new(
                    "channel.set_midi_channel",
                    [number.to_string(), value.to_string()],
                )));
            }
            ui.add_space(6.0);
            ui.label(RichText::new("MIDI ch").small().color(theme::TEXT_DIM));
        });
    });
}

/// One square tab cell: the active one takes the page's tone.
fn tab_cell(ui: &mut egui::Ui, icon: Icon, active: bool, hint: &str) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(40.0, 40.0), egui::Sense::click());
    let fill = if active {
        theme::PANEL
    } else if response.hovered() {
        theme::RAISED
    } else {
        theme::PANEL_DARK
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    let color = if active {
        theme::ACCENT
    } else {
        theme::TEXT_DIM
    };
    icons::paint(ui.painter(), rect.shrink(9.0), icon, color);
    response.hint(hint)
}

fn on_light(ui: &mut egui::Ui, number: usize, channel: &Channel, actions: &mut Vec<Action>) {
    ui.vertical(|ui| {
        ui.set_width(28.0);
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            let light = widgets::dot(ui, !channel.mute, theme::ACCENT)
                .hint("On: lit while the channel plays · click to mute");
            if light.clicked() {
                actions.push(Action::Run(Invocation::new(
                    "channel.toggle_mute",
                    [number.to_string()],
                )));
            }
            ui.add_space(10.0);
            ui.label(RichText::new("On").small().color(theme::TEXT_DIM));
        });
    });
}

fn volume_knob(
    ui: &mut egui::Ui,
    state: &mut UiState,
    number: usize,
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    let volume = widgets::labeled_knob(
        ui,
        "Volume",
        channel.volume / MAX_VOLUME,
        DEFAULT_CHANNEL_VOLUME / MAX_VOLUME,
        false,
        &format!("{:.0}%", channel.volume * 100.0),
    );
    crate::automation::menu(
        &volume.response,
        &format!("channel:{number}:volume"),
        actions,
    );
    if volume.started {
        state.begin_gesture();
    }
    if let Some(t) = volume.changed {
        actions.push(Action::Coalesced(
            Invocation::new(
                "channel.set_volume",
                [number.to_string(), format!("{:.3}", t * MAX_VOLUME)],
            ),
            state.gesture(),
        ));
    }
}

fn pan_knob(
    ui: &mut egui::Ui,
    state: &mut UiState,
    number: usize,
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    let pan = widgets::labeled_knob(
        ui,
        "Pan",
        (channel.pan + 1.0) / 2.0,
        0.5,
        true,
        &crate::format::pan(channel.pan),
    );
    crate::automation::menu(&pan.response, &format!("channel:{number}:pan"), actions);
    if pan.started {
        state.begin_gesture();
    }
    if let Some(t) = pan.changed {
        actions.push(Action::Coalesced(
            Invocation::new(
                "channel.set_pan",
                [number.to_string(), format!("{:.3}", t * 2.0 - 1.0)],
            ),
            state.gesture(),
        ));
    }
}

/// The mixer track box: "M" or the insert number over a "Track" caption that
/// routes to a free track when clicked; double-click opens the mixer there.
fn track_box(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    number: usize,
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    ui.vertical(|ui| {
        ui.set_width(40.0);
        ui.add_space(16.0);
        ui.vertical_centered(|ui| {
            let last = frame.project.mixer.len().saturating_sub(1);
            let mut track = channel.mixer_track;
            let name = frame
                .project
                .mixer
                .get(track)
                .map_or("", |t| t.name.as_str());
            let response = ui.add(
                egui::DragValue::new(&mut track)
                    .range(0..=last)
                    .custom_formatter(|n, _| track_label(n as usize))
                    .custom_parser(|text| parse_track(text).map(|n| n as f64)),
            );
            if response.changed() {
                actions.push(Action::Run(Invocation::new(
                    "channel.route",
                    [number.to_string(), track.to_string()],
                )));
            }
            if response.double_clicked() {
                state.selected_track = channel.mixer_track;
                state.windows.mixer = true;
                state.arrange = Some(crate::Arrange::Raise("mixer-window"));
            }
            response.hint(format!(
                "Mixer track: {name} · drag or type to route · double-click opens the mixer"
            ));
            ui.add_space(6.0);
            let caption = ui
                .add(
                    egui::Label::new(RichText::new("Track").small().color(theme::TEXT_DIM))
                        .sense(egui::Sense::click()),
                )
                .hint("Click to route this channel to a free mixer track");
            if caption.hovered() {
                ui.painter().text(
                    caption.rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Track",
                    egui::FontId::proportional(theme::FONT_CAPTION),
                    theme::TEXT,
                );
            }
            if caption.clicked() {
                actions.push(Action::Run(Invocation::new(
                    "channel.route_free",
                    [number.to_string()],
                )));
            }
        });
    });
}

/// "M" for the master, else the insert number.
fn track_label(track: usize) -> String {
    if track == 0 {
        "M".to_owned()
    } else {
        track.to_string()
    }
}

fn parse_track(text: &str) -> Option<usize> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("m") {
        Some(0)
    } else {
        text.parse().ok()
    }
}

/// The Sound page: the instrument's own sections.
fn sound(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    index: usize,
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    let number = index + 1;
    match &channel.instrument {
        Instrument::Sampler { sample } => {
            sampler(ui, state, frame, index, channel, sample, actions);
        }
        Instrument::Synth
        | Instrument::DrumSynth
        | Instrument::Pluck
        | Instrument::Fm
        | Instrument::Keys
        | Instrument::Additive => {
            with_editor(ui, state, index, channel, actions, |ui, e| e.sound(ui));
        }
        Instrument::Granular { sample } => {
            granular(ui, state, frame, index, channel, sample, actions);
        }
        Instrument::Kit { pads } => {
            kit(ui, state, frame, index, channel, pads, actions);
            ui.add_space(6.0);
            with_editor(ui, state, index, channel, actions, |ui, e| e.kit(ui));
        }
        Instrument::Zones { zones } => {
            crate::zone_editor::show(ui, state, frame, index, zones, actions);
            ui.add_space(6.0);
            with_editor(ui, state, index, channel, actions, |ui, e| e.zones(ui));
        }
        Instrument::Resynth { sample } => {
            resynth(ui, state, frame, index, channel, sample, actions);
        }
        Instrument::Layer { channels } => {
            layer(ui, frame, index, channels, actions);
        }
        // Handled before the sound sections.
        Instrument::MidiOut { .. } => {}
        Instrument::Antiserum { patch } => {
            crate::antiserum::show(ui, state, frame, index, patch, actions);
        }
        Instrument::Plugin { plugin } => {
            // Plugins shape their own sound; the built-in sections do not apply.
            let target = crate::plugins::Target::Channel(number);
            crate::plugins::panel(ui, state, frame.plugins, plugin, target, actions);
        }
    }
}

/// The sample's name and a menu to change it (`sample_slot`); a sound
/// dragged from the browser onto the row replaces it too.
fn sample_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    index: usize,
    sample: &str,
    hint: &str,
    actions: &mut Vec<Action>,
) {
    let set =
        |sound: String| Invocation::new("channel.set_sample", [(index + 1).to_string(), sound]);
    let row = ui.horizontal(|ui| {
        ui.add(egui::Label::new(RichText::new(sample).monospace().small()).truncate());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let hint = format!("{hint} (or drag one here from the browser)");
            let choice = sample_slot::button(ui, state, "Change…", &hint, false);
            let pick = || FilePick::sample(Pick::SetSample(index), sample, frame.project);
            sample_slot::apply(state, choice, pick, set, actions);
        });
    });
    if let Some(sound) = sample_slot::dropped(ui, row.response.rect) {
        actions.push(Action::Run(set(sound)));
    }
}

fn sampler(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    index: usize,
    channel: &Channel,
    sample: &str,
    actions: &mut Vec<Action>,
) {
    let number = index + 1;
    section(ui, "SAMPLE", |ui| {
        sample_row(
            ui,
            state,
            frame,
            index,
            sample,
            "Load another sample",
            actions,
        );
        let slices = channel.params.get(inst::SLICES).round().max(1.0) as u32;
        instrument::waveform(
            ui,
            frame.waveform,
            crate::channel_color(frame.project, index),
            slices,
        );
        crate::sample_editor::show(ui, state, frame, index, actions);
        let mut slice_actions = Vec::new();
        with_editor(ui, state, index, channel, actions, |ui, e| {
            e.sampler(ui, &mut |ui, slices| {
                let target = if slices > 1 { slices } else { 8 };
                if ui
                    .button(format!("Slice to pattern ({target})"))
                    .hint("Replace this channel's notes with one note per slice, in order")
                    .clicked()
                {
                    slice_actions.push(Action::Run(Invocation::new(
                        "channel.slice",
                        [number.to_string(), target.to_string()],
                    )));
                }
            });
        });
        actions.extend(slice_actions);
    });
}

fn granular(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    index: usize,
    channel: &Channel,
    sample: &str,
    actions: &mut Vec<Action>,
) {
    section(ui, "GRAINS", |ui| {
        sample_row(
            ui,
            state,
            frame,
            index,
            sample,
            "Load another sample",
            actions,
        );
        instrument::waveform(
            ui,
            frame.waveform,
            crate::channel_color(frame.project, index),
            1,
        );
        crate::sample_editor::show(ui, state, frame, index, actions);
        with_editor(ui, state, index, channel, actions, |ui, e| e.granular(ui));
    });
}

fn resynth(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    index: usize,
    channel: &Channel,
    sample: &str,
    actions: &mut Vec<Action>,
) {
    section(ui, "RESYNTHESIS", |ui| {
        sample_row(
            ui,
            state,
            frame,
            index,
            sample,
            "Measure another sample's harmonics",
            actions,
        );
        crate::sample_editor::show(ui, state, frame, index, actions);
        with_editor(ui, state, index, channel, actions, |ui, e| e.resynth(ui));
    });
}

fn kit(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    index: usize,
    channel: &Channel,
    pads: &[String],
    actions: &mut Vec<Action>,
) {
    let root = channel.params.get(inst::ROOT_KEY).round() as u8;
    section(ui, "PADS", |ui| {
        ui.label(
            RichText::new(
                "Click a pad to hear it; right-click to load a sample. Pad 1 is the root key.",
            )
            .small()
            .color(theme::TEXT_DIM),
        );
        egui::Grid::new(("kit-pads", index))
            .spacing([4.0, 4.0])
            .show(ui, |ui| {
                // Pads 13–16 on top, like a pad controller.
                for row in (0..4).rev() {
                    for col in 0..4 {
                        let pad = row * 4 + col;
                        let name = pads
                            .get(pad)
                            .filter(|p| !p.is_empty())
                            .map(|p| {
                                p.strip_prefix(musiccraft_project::params::BUILTIN_PREFIX)
                                    .map(str::to_owned)
                                    .unwrap_or_else(|| {
                                        std::path::Path::new(p).file_stem().map_or_else(
                                            || p.clone(),
                                            |s| s.to_string_lossy().into_owned(),
                                        )
                                    })
                            })
                            .unwrap_or_else(|| "—".to_owned());
                        let button = egui::Button::new(
                            RichText::new(format!("{}\n{name}", pad + 1)).small(),
                        )
                        .min_size(vec2(90.0, 40.0))
                        .fill(if name == "—" {
                            theme::PANEL_DARK
                        } else {
                            theme::RAISED
                        });
                        let response = ui.add(button).hint(format!(
                            "Pad {} — key {} · right-click to choose its sound, or drag one here",
                            pad + 1,
                            key_name(root.saturating_add(pad as u8))
                        ));
                        if response.clicked() {
                            actions.push(Action::PreviewShort {
                                channel: index,
                                key: root.saturating_add(pad as u8),
                            });
                        }
                        let set = |sound: String| {
                            Invocation::new(
                                "channel.set_pad",
                                [(index + 1).to_string(), (pad + 1).to_string(), sound],
                            )
                        };
                        let current = pads.get(pad).cloned().unwrap_or_default();
                        response.context_menu(|ui| {
                            let choice = sample_slot::menu_items(ui, state, !current.is_empty());
                            let pick = || {
                                FilePick::sample(Pick::SetPad(index, pad), &current, frame.project)
                            };
                            sample_slot::apply(state, choice, pick, set, actions);
                        });
                        if let Some(sound) = sample_slot::dropped(ui, response.rect) {
                            actions.push(Action::Run(set(sound)));
                        }
                    }
                    ui.end_row();
                }
            });
    });
}

/// MIDI channel and mute of a MIDI Out channel, and where it sends.
fn midi_out(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    number: usize,
    channel: &Channel,
    actions: &mut Vec<Action>,
) {
    section(ui, "MIDI OUTPUT", |ui| {
        let port = frame
            .settings
            .midi_output
            .as_deref()
            .map_or_else(|| "no port chosen".to_owned(), |p| format!("port {p}"));
        ui.label(
            RichText::new(format!(
                "Notes go to external gear ({port}); choose the port in Settings > MIDI."
            ))
            .small()
            .color(theme::TEXT_DIM),
        );
    });
    ui.add_space(6.0);
    section(ui, "CONTROLS AND PROGRAM", |ui| {
        ui.label(
            RichText::new("Each knob sends its controller number when it changes (−1 sends nothing); automate them like any knob.")
                .small()
                .color(theme::TEXT_DIM),
        );
        for row in 0..2 {
            ui.horizontal(|ui| {
                for k in row * 4..row * 4 + 4 {
                    let cc = channel.params.get(inst::MIDI_CC1_NUMBER + k).round();
                    param_knob(
                        ui,
                        state,
                        number,
                        channel,
                        inst::MIDI_CC1 + k,
                        &format!("CC {cc}"),
                        actions,
                    );
                }
            });
        }
        ui.horizontal(|ui| {
            for k in 0..MIDI_CCS {
                param_knob(
                    ui,
                    state,
                    number,
                    channel,
                    inst::MIDI_CC1_NUMBER + k,
                    &format!("# {}", k + 1),
                    actions,
                );
            }
        });
        ui.horizontal(|ui| {
            param_knob(
                ui,
                state,
                number,
                channel,
                inst::MIDI_PROGRAM,
                "Program",
                actions,
            );
        });
    });
}

fn layer(
    ui: &mut egui::Ui,
    frame: &Frame<'_>,
    index: usize,
    children: &[musiccraft_project::ChannelId],
    actions: &mut Vec<Action>,
) {
    section(ui, "LAYER", |ui| {
        ui.label(
            RichText::new("Notes on this channel play every ticked channel.")
                .small()
                .color(theme::TEXT_DIM),
        );
        for (i, other) in frame.project.channels.iter().enumerate() {
            if i == index || matches!(other.instrument, Instrument::Layer { .. }) {
                continue;
            }
            let mut on = children.contains(&other.id);
            if crate::widgets::led_switch(ui, &mut on, &other.name).changed() {
                let mut next: Vec<usize> = frame
                    .project
                    .channels
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| children.contains(&c.id))
                    .map(|(j, _)| j)
                    .collect();
                if on {
                    next.push(i);
                } else {
                    next.retain(|j| *j != i);
                }
                next.sort_unstable();
                let list = if next.is_empty() {
                    "-".to_owned()
                } else {
                    next.iter()
                        .map(|j| (j + 1).to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                };
                actions.push(Action::Run(Invocation::new(
                    "channel.set_layer",
                    [(index + 1).to_string(), list],
                )));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knob_mappings_round_trip() {
        for (index, value) in [
            (inst::CUTOFF, 1_000.0),
            (inst::ATTACK, 0.05),
            (inst::SUSTAIN, 0.4),
            (inst::DETUNE, -300.0),
        ] {
            let spec = spec(index);
            let back = spec.from_unit(spec.to_unit(value));
            assert!(
                (back - value).abs() / value.abs().max(1.0) < 1e-3,
                "{} {value} -> {back}",
                spec.name
            );
        }
        let cutoff = spec(inst::CUTOFF);
        assert!(
            (cutoff.to_unit(632.5) - 0.5).abs() < 0.01,
            "log scale centre"
        );
        assert_eq!(cutoff.format(2_500.0), "2.50 kHz");
        assert_eq!(spec(inst::ATTACK).format(0.25), "250 ms");
        assert_eq!(spec(inst::SUSTAIN).format(0.5), "50%");
    }

    #[test]
    fn edits_become_set_param_commands() {
        let mut state = UiState::default();
        let mut actions = Vec::new();
        let edits = [
            InstrumentEdit::Begin,
            InstrumentEdit::Set(inst::CUTOFF, 900.0),
            InstrumentEdit::Preset(vec![(inst::ATTACK, 0.1), (inst::RELEASE, 0.5)]),
        ];
        apply(&mut state, 2, &edits, &mut actions);
        let text: Vec<String> = actions
            .iter()
            .map(|a| match a {
                Action::Coalesced(i, _) | Action::Run(i) => i.to_string(),
                _ => String::new(),
            })
            .collect();
        assert_eq!(text[0], "channel.set_param 2 cutoff 900");
        assert!(text[1].contains("channel.set_param 2 attack 0.1"));
        assert!(text[1].contains("channel.set_param 2 release 0.5"));
    }
}
