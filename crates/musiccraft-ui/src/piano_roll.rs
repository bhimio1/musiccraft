//! The piano roll: edit the selected channel's notes in the selected pattern.

use crate::hint::Hint;
use std::collections::BTreeSet;

use egui::{
    Align2, Color32, FontId, PointerButton, Pos2, Rect, RichText, Sense, Stroke, Vec2, pos2, vec2,
};
use musiccraft_commands::Invocation;
use musiccraft_project::music::key_name;
use musiccraft_project::params::TICKS_PER_STEP;
use musiccraft_project::{Meter, Note, PlayMode, Project};

use crate::geometry::{Grid, Hit, Snap, hit_test};
use crate::icons::{self, Icon};
use crate::{Action, Frame, UiState, theme};

pub(crate) const KEYS_WIDTH: f32 = 52.0;
const RULER_HEIGHT: f32 = 18.0;
/// Default height of the property lane pinned to the bottom of the view.
const LANE_HEIGHT: f32 = 96.0;
/// The lane's height range when the splitter is dragged.
const LANE_HEIGHT_RANGE: (f32, f32) = (40.0, 320.0);
/// Thickness of the splitter between the grid and the lane.
const SPLITTER: f32 = 5.0;
/// Bars the view runs on past the pattern's end, and the fewest it shows.
const EXTRA_BARS: u32 = 8;
const MIN_BARS: u32 = 16;
/// Key-row height: the default and the vertical zoom range, in points.
const KEY_HEIGHT: f32 = 18.0;
const KEY_HEIGHT_RANGE: (f32, f32) = (6.0, 30.0);
/// Horizontal zoom range, in points per step.
pub(crate) const STEP_WIDTH_RANGE: (f32, f32) = (6.0, 120.0);
const KEYS: u32 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragKind {
    Create,
    Move,
    Resize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct NoteDrag {
    kind: DragKind,
    original: Note,
    /// Ticks between the note start and the grab point.
    grab: f32,
    start: u32,
    key: u8,
    length: u32,
}

/// What the left mouse button does in the note grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Tool {
    /// Click adds a note, drag moves or resizes (Ctrl-drag selects).
    #[default]
    Draw,
    /// Drag lays down notes one after another.
    Paint,
    /// Click or drag over notes deletes them.
    Delete,
    /// Drag selects notes; click empty space clears the selection.
    Select,
    /// Click or drag over notes to mute or unmute them.
    Mute,
    /// Click a note (or clip) to cut it in two there.
    Slice,
    /// Click to hear a note (piano roll) or play from there (playlist).
    Play,
    /// Click to zoom in, Shift+click to zoom out.
    Zoom,
}

impl Tool {
    /// The tool a `ui.tool` argument names.
    pub(crate) fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "draw" => Self::Draw,
            "paint" => Self::Paint,
            "delete" => Self::Delete,
            "select" => Self::Select,
            "mute" => Self::Mute,
            "slice" => Self::Slice,
            "play" => Self::Play,
            "zoom" => Self::Zoom,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RollState {
    /// The active tool.
    pub(crate) tool: Tool,
    /// The note (start, key) whose properties window is open.
    pub(crate) props_note: Option<(u32, u8)>,
    /// The selected pattern's bars and beats, refreshed every frame.
    pub(crate) meter: Meter,
    /// New notes are slide notes.
    pub(crate) slide: bool,
    /// New notes are portamento notes.
    pub(crate) porta: bool,
    /// Colour group of new notes (0 = channel colour).
    pub(crate) color: u8,
    /// Notes painted or marked for deletion in the stroke in progress.
    stroke: Vec<Note>,
    /// Notes swept by a right-button drag, deleted when it ends.
    pub(crate) right_erase: Vec<Note>,
    pub(crate) px_per_step: f32,
    pub(crate) key_height: f32,
    pub(crate) snap: Snap,
    pub(crate) last_length: u32,
    pub(crate) velocity: u8,
    drag: Option<NoteDrag>,
    pub(crate) preview: Option<(usize, u8)>,
    /// The preview is a key held on the keyboard, which releases it itself.
    keys_held: bool,
    /// Channel the view was last centred for.
    centred_for: Option<usize>,
    /// Grid and visible area from the last frame (used by tests and tools).
    pub(crate) last_view: Option<(Grid, Rect)>,
    /// Chord drawn by a click, or `None` for single notes.
    chord: Option<&'static str>,
    /// Highlighted scale: (root pitch class, scale index).
    pub(crate) scale: Option<(u8, usize)>,
    /// Whether drawn and moved notes land on the highlighted scale's keys.
    pub(crate) fit_scale: bool,
    /// Note whose velocity is being dragged in the lane.
    lane_drag: Option<Note>,
    /// What the bottom lane edits.
    pub(crate) lane_mode: LaneMode,
    /// Selected notes of the shown channel and pattern, as (start, key).
    pub(crate) selection: BTreeSet<(u32, u8)>,
    /// (channel, pattern) the selection belongs to.
    selection_for: Option<(usize, usize)>,
    /// Rubber-band selection in progress: (origin, current) in screen space.
    marquee: Option<(Pos2, Pos2)>,
    /// Copied notes, with starts relative to the earliest one.
    clipboard: Vec<Note>,
    /// Snapped tick under the pointer, used as the paste position.
    hover_tick: Option<u32>,
    /// Time selection made on the ruler, as (start, end) ticks.
    pub(crate) time_selection: Option<(u32, u32)>,
    /// Scroll offset of the last frame, in points.
    offset: Vec2,
    /// Scroll offset asked for by the scroll bars, applied next frame.
    jump: Option<Vec2>,
    /// Which keys of the keyboard carry their name.
    pub(crate) key_labels: KeyLabels,
    /// Other channels' notes are drawn behind.
    pub(crate) ghosts: bool,
    /// Notes carry their key name.
    pub(crate) note_labels: bool,
    /// Height of the property lane while shown.
    pub(crate) lane_height: f32,
    /// The property lane is shown.
    pub(crate) lane_shown: bool,
}

impl RollState {
    /// The property lane's height on screen: nothing while hidden.
    fn lane(&self) -> f32 {
        if self.lane_shown {
            self.lane_height
        } else {
            0.0
        }
    }
}

/// Which keys of the piano roll's keyboard are labelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyLabels {
    C,
    White,
    All,
    Scale,
    None,
}

impl KeyLabels {
    const ALL: [(Self, &'static str); 5] = [
        (Self::C, "C keys"),
        (Self::White, "White keys"),
        (Self::All, "All keys"),
        (Self::Scale, "Keys of the scale"),
        (Self::None, "None"),
    ];

    /// Whether `key` is labelled; the C keys always fit, the others need
    /// rows at least `LABEL_ROWS` points tall.
    fn shows(self, key: u8, row_height: f32, scale: Option<(u8, usize)>) -> bool {
        let fits = row_height >= LABEL_ROWS;
        match self {
            Self::C => key % 12 == 0,
            Self::White => fits && !is_black(key),
            Self::All => fits,
            Self::Scale => {
                fits && scale.is_some_and(|(root, s)| {
                    musiccraft_project::music::SCALES
                        .get(s)
                        .is_some_and(|(_, d)| musiccraft_project::music::in_scale(key, root, d))
                })
            }
            Self::None => false,
        }
    }
}

/// The shortest key row that takes a label (other than the C keys).
const LABEL_ROWS: f32 = 9.0;

/// The Keyboard labels choices, as radio items.
fn key_labels_list(ui: &mut egui::Ui, state: &mut UiState) {
    for (labels, text) in KeyLabels::ALL {
        if ui.radio(state.roll.key_labels == labels, text).clicked() {
            state.roll.key_labels = labels;
            ui.close();
        }
    }
}

impl Default for RollState {
    fn default() -> Self {
        Self {
            tool: Tool::Draw,
            slide: false,
            porta: false,
            color: 0,
            stroke: Vec::new(),
            right_erase: Vec::new(),
            px_per_step: 24.0,
            key_height: KEY_HEIGHT,
            snap: Snap::Step,
            meter: Meter::default(),
            last_length: TICKS_PER_STEP,
            velocity: 100,
            drag: None,
            preview: None,
            keys_held: false,
            centred_for: None,
            last_view: None,
            chord: None,
            scale: None,
            fit_scale: false,
            lane_drag: None,
            lane_mode: LaneMode::Velocity,
            props_note: None,
            selection: BTreeSet::new(),
            selection_for: None,
            marquee: None,
            clipboard: Vec::new(),
            hover_tick: None,
            time_selection: None,
            offset: Vec2::ZERO,
            jump: None,
            key_labels: KeyLabels::C,
            ghosts: true,
            note_labels: true,
            lane_height: LANE_HEIGHT,
            lane_shown: true,
        }
    }
}

fn is_black(key: u8) -> bool {
    matches!(key % 12, 1 | 3 | 6 | 8 | 10)
}

pub(crate) fn key_of_row(row: u32) -> u8 {
    u8::try_from(KEYS - 1 - row.min(KEYS - 1)).unwrap_or(0)
}

pub(crate) fn row_of_key(key: u8) -> u32 {
    KEYS - 1 - u32::from(key)
}

fn ticks(value: u32) -> String {
    format!("{value}t")
}

/// Wraps several invocations into one undo step.
pub(crate) fn batch(invocations: Vec<Invocation>) -> Option<Invocation> {
    match invocations.len() {
        0 => None,
        1 => invocations.into_iter().next(),
        _ => Some(Invocation::new(
            musiccraft_commands::BATCH,
            invocations.iter().map(ToString::to_string),
        )),
    }
}

/// Moves notes by `dt` ticks and `dk` semitones as one undo step. The deltas are
/// clamped so nothing moves before the start or outside the keyboard, and the
/// notes are processed in an order where no moved note lands on one that has not
/// moved yet.
fn clamp_delta(notes: &[Note], dt: i64, dk: i32) -> (i64, i32) {
    let min_start = notes.iter().map(|n| i64::from(n.start)).min().unwrap_or(0);
    let (lo, hi) = notes.iter().fold((127_i32, 0_i32), |(lo, hi), n| {
        (lo.min(i32::from(n.key)), hi.max(i32::from(n.key)))
    });
    (dt.max(-min_start), dk.clamp(-lo, 127 - hi))
}

fn move_notes(channel: usize, notes: &[Note], dt: i64, dk: i32) -> Option<Invocation> {
    let (dt, dk) = clamp_delta(notes, dt, dk);
    if notes.is_empty() || (dt == 0 && dk == 0) {
        return None;
    }
    let mut ordered = notes.to_vec();
    ordered.sort_by_key(|n| (n.start, n.key));
    if dt > 0 || (dt == 0 && dk > 0) {
        ordered.reverse();
    }
    let ch = (channel + 1).to_string();
    batch(
        ordered
            .iter()
            .map(|n| {
                let start = u32::try_from(i64::from(n.start) + dt).unwrap_or(0);
                let key = u8::try_from(i32::from(n.key) + dk).unwrap_or(n.key);
                Invocation::new(
                    "note.move",
                    [
                        ch.clone(),
                        ticks(n.start),
                        n.key.to_string(),
                        ticks(start),
                        key.to_string(),
                    ],
                )
            })
            .collect(),
    )
}

/// Adds copies of `notes` shifted by `offset` ticks.
fn add_notes(channel: usize, notes: &[Note], offset: u32) -> Option<Invocation> {
    batch(
        notes
            .iter()
            .map(|n| {
                create(
                    channel,
                    &Note {
                        start: n.start + offset,
                        ..*n
                    },
                )
            })
            .collect(),
    )
}

/// Tests a note against the current colour, for the Select menu.
type NotePick = fn(&Note, u8) -> bool;

/// A window-menu item that sets one property: label, notes, property, value.
type PropItem<'a> = (&'a str, Vec<Note>, &'a str, &'a str);

/// Sets property `prop` to `value` on each of `notes`, as one undo step.
fn set_notes_prop(channel: usize, notes: &[Note], prop: &str, value: &str) -> Option<Invocation> {
    let ch = (channel + 1).to_string();
    batch(
        notes
            .iter()
            .map(|n| {
                Invocation::new(
                    "note.set_prop",
                    [
                        ch.clone(),
                        ticks(n.start),
                        n.key.to_string(),
                        prop.to_owned(),
                        value.to_owned(),
                    ],
                )
            })
            .collect(),
    )
}

fn remove_notes(channel: usize, notes: &[Note]) -> Option<Invocation> {
    let ch = (channel + 1).to_string();
    batch(
        notes
            .iter()
            .map(|n| {
                Invocation::new(
                    "note.remove",
                    [ch.clone(), ticks(n.start), n.key.to_string()],
                )
            })
            .collect(),
    )
}

/// Invocation that finishes a drag gesture.
/// The command that creates `note` on `channel`: `note.add` for plain
/// notes, `note.restore` when it carries other properties.
/// Replaces channel `channel`'s notes `old` with `new`, as one undo step.
pub(crate) fn replace_notes(channel: usize, old: &[Note], new: &[Note]) -> Option<Invocation> {
    let ch = (channel + 1).to_string();
    let removes = old.iter().map(|n| {
        Invocation::new(
            "note.remove",
            [ch.clone(), ticks(n.start), n.key.to_string()],
        )
    });
    batch(
        removes
            .chain(new.iter().map(|n| create(channel, n)))
            .collect(),
    )
}

fn create(channel: usize, note: &Note) -> Invocation {
    let ch = (channel + 1).to_string();
    if note.props == musiccraft_project::NoteProps::default() {
        let mut args = vec![
            ch,
            ticks(note.start),
            ticks(note.length),
            note.key.to_string(),
            note.velocity.to_string(),
        ];
        if note.slide {
            args.extend([note.pan.to_string(), "slide".to_owned()]);
        }
        return Invocation::new("note.add", args);
    }
    let p = note.props;
    let json = format!(
        "{{\"start\":{},\"length\":{},\"key\":{},\"velocity\":{},\"pan\":{},\"slide\":{},\"release\":{},\"fine\":{},\"mod_x\":{},\"mod_y\":{},\"color\":{},\"porta\":{},\"muted\":{}}}",
        note.start,
        note.length,
        note.key,
        note.velocity,
        note.pan,
        note.slide,
        p.release,
        p.fine,
        p.mod_x,
        p.mod_y,
        p.color,
        p.porta,
        p.muted
    );
    Invocation::new("note.restore", [ch, json])
}

/// Properties new notes get from the toolbar.
fn new_props(state: &UiState) -> musiccraft_project::NoteProps {
    musiccraft_project::NoteProps {
        porta: state.roll.porta,
        color: state.roll.color,
        ..musiccraft_project::NoteProps::default()
    }
}

fn finish(
    drag: &NoteDrag,
    channel: usize,
    velocity: u8,
    slide: bool,
    props: musiccraft_project::NoteProps,
) -> Option<Invocation> {
    let ch = (channel + 1).to_string();
    let t = |ticks: u32| format!("{ticks}t");
    let n = drag.original;
    match drag.kind {
        DragKind::Create => Some(create(
            channel,
            &Note {
                channel: musiccraft_project::ChannelId(0),
                start: drag.start,
                length: drag.length,
                key: drag.key,
                velocity,
                pan: 0,
                slide,
                props,
            },
        )),
        DragKind::Move if (drag.start, drag.key) != (n.start, n.key) => Some(Invocation::new(
            "note.move",
            [
                ch,
                t(n.start),
                n.key.to_string(),
                t(drag.start),
                drag.key.to_string(),
            ],
        )),
        DragKind::Resize if drag.length != n.length => Some(Invocation::new(
            "note.resize",
            [ch, t(n.start), n.key.to_string(), t(drag.length)],
        )),
        _ => None,
    }
}

pub fn show(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let mut open = state.windows.piano_roll;
    let project = frame.project;
    state.roll.meter = project.current_pattern().meter();
    let title = match project.channels.get(state.selected_channel) {
        Some(channel) => format!("Piano roll — {}", channel.name),
        None => "Piano roll".to_owned(),
    };
    let area = state.workspace;
    let id = egui::Id::new("piano-roll");
    let slots = state.floating_area();
    let size = vec2(
        (slots.width() / 2.0).max(480.0),
        (slots.height() / 2.0).max(300.0),
    );
    let default = egui::Rect::from_min_size(
        pos2(
            (slots.right() - size.x).max(slots.left()),
            slots.top() + slots.height() / 4.0,
        ),
        size,
    );
    let window = open
        .then(|| {
            crate::chrome::window(ctx, id, "Piano roll", area, default, true)
                .default_height(default.height())
                .show(ctx, |ui| {
                    let empty = project.channels.is_empty();
                    let number = (state.selected_channel + 1).to_string();
                    let folded = crate::chrome::Header::new(id, &title).show_with_extra(
                        ui,
                        &mut open,
                        &mut (&mut *state, &mut *actions),
                        |ui, (state, actions)| {
                            if !empty {
                                options_menu(ui, state, project, &number, actions);
                            }
                        },
                        |ui, (state, actions)| {
                            if !empty {
                                pickers(ui, state, project, &number, actions);
                                play_button(ui, project, PlayMode::Pattern, actions);
                                ui.add_space(6.0);
                                tools(ui, state);
                            }
                        },
                        |ui, (state, _)| {
                            if !empty {
                                channel_picker(ui, state, project);
                            }
                        },
                    );
                    if folded {
                        return;
                    }
                    if empty {
                        ui.label(
                            RichText::new("Add a channel to write notes.").color(theme::TEXT_DIM),
                        );
                        return;
                    }
                    ui.separator();
                    canvas(ui, state, frame, actions);
                })
        })
        .flatten();
    state.windows.piano_roll = open;
    crate::chrome::remember(ctx, id, window.as_ref().map(|w| &w.response));
    let focused = window.is_some() && crate::chrome::focused(ctx, id);
    if open && focused && !ctx.egui_wants_keyboard_input() {
        keys(ctx, state, project, actions);
    }
}

/// The editing tools, in the style guide's order.
const TOOLS: [(Tool, Icon, &str); 8] = [
    (
        Tool::Draw,
        Icon::Pencil,
        "Draw (P): click adds a note, drag moves or resizes, Ctrl+drag selects",
    ),
    (
        Tool::Paint,
        Icon::Brush,
        "Paint (B): drag to lay down notes one after another",
    ),
    (
        Tool::Delete,
        Icon::Eraser,
        "Delete (D): click or drag over notes to delete them",
    ),
    (
        Tool::Mute,
        Icon::Mute,
        "Mute (T): click or drag over notes to mute or unmute them",
    ),
    (
        Tool::Slice,
        Icon::Slice,
        "Slice (C): click a note to cut it in two there",
    ),
    (
        Tool::Select,
        Icon::Select,
        "Select (E): drag a box around notes; click empty space to clear",
    ),
    (
        Tool::Zoom,
        Icon::Zoom,
        "Zoom (Z): click to zoom in, Shift+click to zoom out",
    ),
    (
        Tool::Play,
        Icon::PlayNote,
        "Audition (Y): click to hear a note or key",
    ),
];

/// A window's play button: a click plays in `mode` (the pattern for the
/// piano roll, the song for the playlist), switching to it first, or pauses;
/// a right-click stops and returns to the start.
pub(crate) fn play_button(
    ui: &mut egui::Ui,
    project: &Project,
    mode: PlayMode,
    actions: &mut Vec<Action>,
) {
    let playing = project.transport().is_playing() && project.play_mode == mode;
    let what = match mode {
        PlayMode::Pattern => "pattern",
        PlayMode::Song => "song",
    };
    let (icon, hint) = if playing {
        (
            Icon::Pause,
            format!("Pause the {what} · right-click to stop"),
        )
    } else {
        (
            Icon::Play,
            format!("Play the {what} · right-click to go back to its start"),
        )
    };
    let response = icons::button(ui, icon, playing, theme::ACCENT, &hint);
    let mut run = |line: &str| {
        if let Ok(invocation) = Invocation::parse(line) {
            actions.push(Action::Run(invocation));
        }
    };
    if response.clicked() {
        if playing {
            run("transport.pause");
        } else {
            if project.play_mode != mode {
                run(&format!("transport.set_mode {what}"));
            }
            run("transport.play");
        }
    }
    if response.secondary_clicked() {
        run(if playing {
            "transport.stop"
        } else {
            "transport.jump_to_start"
        });
    }
}

/// The header's tool group.
fn tools(ui: &mut egui::Ui, state: &mut UiState) {
    ui.spacing_mut().item_spacing.x = 1.0;
    for (tool, icon, hint) in TOOLS {
        if icons::button(ui, icon, state.roll.tool == tool, theme::ACCENT, hint).clicked() {
            state.roll.tool = tool;
        }
    }
}

/// A square header button that opens `list` in a popup.
fn picker(
    ui: &mut egui::Ui,
    (key, icon): (&str, Icon),
    look: (bool, bool),
    hint: &str,
    list: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let popup = egui::Id::new(key);
    let open = egui::Popup::is_id_open(ui.ctx(), popup);
    let response = icons::picker(ui, icon, open, look, hint);
    let toggle = response.clicked().then_some(egui::SetOpenCommand::Toggle);
    egui::Popup::menu(&response)
        .id(popup)
        .open_memory(toggle)
        .show(list);
    response
}

fn scale_name(scale: Option<(u8, usize)>) -> String {
    match scale {
        Some((root, i)) => format!(
            "{} {}",
            key_name(root).trim_end_matches(char::is_numeric),
            musiccraft_project::music::SCALES[i].0
        ),
        None => "No scale".to_owned(),
    }
}

/// The options group: note tools, snap, scale and chord, each a square
/// button whose value is in its hint and ticked in its list.
fn pickers(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    number: &str,
    actions: &mut Vec<Action>,
) {
    ui.spacing_mut().item_spacing.x = 1.0;
    let selected = selected_of(state, project);
    let _ = picker(
        ui,
        ("roll-note-tools", Icon::Wand),
        (false, false),
        "Note tools: strum, arpeggiate, generate and shape the notes",
        |ui| tools_menu(ui, state, number, &selected, actions, true),
    );
    let snap = state.roll.snap;
    let button = picker(
        ui,
        ("roll-snap", Icon::Snap),
        (snap == Snap::Off, false),
        &format!(
            "Snap: {} · click to choose · right-click: new notes this long · wheel steps",
            snap.label()
        ),
        |ui| snap_list(ui, state),
    );
    snap_shortcuts(ui, state, &button);
    let scale = state.roll.scale;
    let _ = picker(
        ui,
        ("roll-scale", Icon::Scale),
        (scale.is_none(), scale.is_some() && state.roll.fit_scale),
        &format!(
            "Scale: {} · click to highlight a scale's keys",
            scale_name(scale)
        ),
        |ui| scale_list(ui, state),
    );
    let chord = state.roll.chord;
    let _ = picker(
        ui,
        ("roll-chord", Icon::Chord),
        (false, chord.is_some()),
        &format!(
            "Draw: {} · click to draw single notes or chords",
            chord.unwrap_or("Note")
        ),
        |ui| chord_list(ui, state),
    );
}

/// Right-click on the snap button makes new notes one snap step long; the
/// wheel over it steps through the snap values.
fn snap_shortcuts(ui: &egui::Ui, state: &mut UiState, button: &egui::Response) {
    let snap = state.roll.snap;
    if button.secondary_clicked() && snap != Snap::Off {
        state.roll.last_length = snap.ticks_at(0, &state.roll.meter);
    }
    let notches = if button.hovered() {
        crate::scrollbars::wheel_notches(ui)
    } else {
        0.0
    };
    if notches != 0.0 {
        let at = Snap::ALL.iter().position(|s| *s == snap).unwrap_or(0);
        let next = if notches > 0.0 {
            at.saturating_sub(1)
        } else {
            (at + 1).min(Snap::ALL.len() - 1)
        };
        state.roll.snap = Snap::ALL[next];
    }
}

/// "No scale", then the root notes side by side with the scale types.
fn scale_list(ui: &mut egui::Ui, state: &mut UiState) {
    if ui
        .selectable_label(state.roll.scale.is_none(), "No scale")
        .clicked()
    {
        state.roll.scale = None;
        ui.close();
    }
    ui.separator();
    let current = state.roll.scale;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            for root in 0..12_u8 {
                let name = key_name(root);
                let name = name.trim_end_matches(char::is_numeric);
                let on = current.is_some_and(|(r, _)| r == root);
                if ui.selectable_label(on, name).clicked() {
                    state.roll.scale = Some((root, current.map_or(0, |(_, i)| i)));
                }
            }
        });
        ui.vertical(|ui| {
            for (i, (scale, _)) in musiccraft_project::music::SCALES.iter().enumerate() {
                let on = current.is_some_and(|(_, s)| s == i);
                if ui.selectable_label(on, *scale).clicked() {
                    state.roll.scale = Some((current.map_or(0, |(r, _)| r), i));
                }
            }
        });
    });
    ui.separator();
    let mut fit = state.roll.fit_scale;
    crate::widgets::enabled_ui(ui, state.roll.scale.is_some(), |ui| {
        if ui
            .checkbox(&mut fit, "Fit edited notes to the scale")
            .changed()
        {
            state.roll.fit_scale = fit;
        }
    });
}

/// The chevron after the title: the list of channels, with a tick on each
/// channel that has notes in this pattern.
fn channel_picker(ui: &mut egui::Ui, state: &mut UiState, project: &Project) {
    let popup = egui::Id::new("roll-channel");
    let open = egui::Popup::is_id_open(ui.ctx(), popup);
    let icon = if open {
        Icon::ChevronDown
    } else {
        Icon::ChevronRight
    };
    let (rect, response) = ui.allocate_exact_size(vec2(14.0, 22.0), Sense::click());
    let color = if response.hovered() || open {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    icons::paint(ui.painter(), rect, icon, color);
    let response = response.hint("Choose the channel to edit");
    let toggle = response.clicked().then_some(egui::SetOpenCommand::Toggle);
    let pattern = project.current_pattern();
    egui::Popup::menu(&response)
        .id(popup)
        .open_memory(toggle)
        .show(|ui| {
            for (i, c) in project.channels.iter().enumerate() {
                let used = pattern.notes_of(c.id).next().is_some();
                let text = if used {
                    format!("{}  ✔", c.name)
                } else {
                    c.name.clone()
                };
                if ui
                    .selectable_label(state.selected_channel == i, text)
                    .clicked()
                {
                    state.selected_channel = i;
                    ui.close();
                }
            }
        });
}

/// The selected notes of the shown channel.
fn selected_of(state: &UiState, project: &Project) -> Vec<Note> {
    project
        .channels
        .get(state.selected_channel)
        .map(|c| {
            project
                .current_pattern()
                .notes_of(c.id)
                .filter(|n| state.roll.selection.contains(&(n.start, n.key)))
                .copied()
                .collect()
        })
        .unwrap_or_default()
}

/// The piano roll's own window-menu items: new-note velocity, quantize, note
/// tools, transposing by octaves and clearing the channel's notes.
fn options_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    number: &str,
    actions: &mut Vec<Action>,
) {
    let run = |actions: &mut Vec<Action>, name: &str, args: &[&str]| {
        let mut all = vec![number.to_owned()];
        all.extend(args.iter().map(|a| (*a).to_owned()));
        actions.push(Action::Run(Invocation::new(name, all)));
    };
    ui.menu_button("File", |ui| {
        if ui.button("Export the song as a MIDI file…").clicked() {
            actions.push(Action::Run(Invocation::new(
                "ui.export_midi",
                Vec::<String>::new(),
            )));
            ui.close();
        }
    });
    ui.menu_button("Edit", |ui| {
        let any = !state.roll.selection.is_empty();
        let item = |ui: &mut egui::Ui, enabled: bool, label: &str, keys: &str| {
            crate::widgets::add_enabled(ui, enabled, egui::Button::new(label).shortcut_text(keys))
                .clicked()
        };
        let ops = [
            (any, "Cut", "Ctrl+X", EditOp::Cut),
            (any, "Copy", "Ctrl+C", EditOp::Copy),
            (
                !state.roll.clipboard.is_empty(),
                "Paste",
                "Ctrl+V",
                EditOp::Paste,
            ),
            (any, "Duplicate", "Ctrl+B", EditOp::Duplicate),
            (any, "Delete", "Del", EditOp::Delete),
        ];
        for (enabled, label, keys, op) in ops {
            if item(ui, enabled, label, keys) {
                if matches!(op, EditOp::Copy | EditOp::Cut) {
                    crate::copied(ui.ctx(), "notes");
                }
                edit_op(state, project, op, actions);
                ui.close();
            }
        }
        ui.separator();
        let moves = [
            ("Move left one snap step", "Shift+←", EditOp::Move(-1, 0)),
            ("Move right one snap step", "Shift+→", EditOp::Move(1, 0)),
            ("Transpose up a semitone", "Shift+↑", EditOp::Move(0, 1)),
            ("Transpose down a semitone", "Shift+↓", EditOp::Move(0, -1)),
        ];
        for (label, keys, op) in moves {
            if item(ui, any, label, keys) {
                edit_op(state, project, op, actions);
                ui.close();
            }
        }
        if ui.button("Transpose an octave up").clicked() {
            run(actions, "note.transpose", &["12"]);
            ui.close();
        }
        if ui.button("Transpose an octave down").clicked() {
            run(actions, "note.transpose", &["-12"]);
            ui.close();
        }
        ui.separator();
        let selected = selected_of(state, project);
        let channel = state.selected_channel;
        let color = (state.roll.color + 1).to_string();
        let props: [PropItem; 3] = [
            (
                "Mute",
                selected
                    .iter()
                    .filter(|n| !n.props.muted)
                    .copied()
                    .collect(),
                "muted",
                "on",
            ),
            (
                "Unmute",
                selected.iter().filter(|n| n.props.muted).copied().collect(),
                "muted",
                "off",
            ),
            (
                "Apply the current colour",
                selected
                    .iter()
                    .filter(|n| n.props.color != state.roll.color)
                    .copied()
                    .collect(),
                "color",
                &color,
            ),
        ];
        for (label, notes, prop, value) in props {
            let invocation = set_notes_prop(channel, &notes, prop, value);
            if crate::widgets::add_enabled(ui, invocation.is_some(), egui::Button::new(label))
                .clicked()
            {
                actions.extend(invocation.map(Action::Run));
                ui.close();
            }
        }
        ui.separator();
        if ui
            .button(RichText::new("Clear the channel's notes").color(theme::DANGER))
            .clicked()
        {
            run(actions, "note.clear", &[]);
            ui.close();
        }
    });
    ui.menu_button("Tools", |ui| {
        if ui
            .button("Quantize")
            .hint("Snap note starts to the grid")
            .clicked()
        {
            let grid = format!("{}t", state.roll.snap.ticks_at(0, &state.roll.meter));
            run(actions, "note.quantize", &[&grid]);
            ui.close();
        }
        let selected = selected_of(state, project);
        tools_menu(ui, state, number, &selected, actions, false);
    });
    ui.menu_button("Chord", |ui| chord_list(ui, state));
    ui.menu_button("Snap", |ui| snap_list(ui, state));
    ui.menu_button("Select", |ui| {
        let ops = [
            ("All", "Ctrl+A", EditOp::SelectAll),
            ("None", "Esc", EditOp::SelectNone),
            ("Invert", "", EditOp::InvertSelection),
        ];
        for (label, keys, op) in ops {
            if ui
                .add(egui::Button::new(label).shortcut_text(keys))
                .clicked()
            {
                edit_op(state, project, op, actions);
                ui.close();
            }
        }
        ui.separator();
        let notes: Vec<Note> = project
            .channels
            .get(state.selected_channel)
            .map(|c| project.current_pattern().notes_of(c.id).copied().collect())
            .unwrap_or_default();
        let color = state.roll.color;
        let picks: [(&str, NotePick); 2] = [
            ("Muted notes", |n, _| n.props.muted),
            ("Notes of the current colour", |n, c| n.props.color == c),
        ];
        for (label, pick) in picks {
            if ui.button(label).clicked() {
                state.roll.selection = notes
                    .iter()
                    .filter(|n| pick(n, color))
                    .map(|n| (n.start, n.key))
                    .collect();
                ui.close();
            }
        }
    });
    ui.menu_button("View", |ui| {
        ui.checkbox(&mut state.roll.ghosts, "Ghost notes");
        ui.menu_button("Keyboard labels", |ui| key_labels_list(ui, state));
        ui.checkbox(&mut state.roll.note_labels, "Note labels");
        ui.add(egui::Checkbox::new(
            &mut state.roll.lane_shown,
            "Property lane",
        ))
        .hint("Shift+Enter shows or hides the property lane");
    });
    ui.menu_button("Zoom", |ui| {
        let items = [
            ("Zoom in", "Page Up", Zoom::By(1.5)),
            ("Zoom out", "Page Down", Zoom::By(1.0 / 1.5)),
            ("Fit the pattern", "Shift+4", Zoom::Pattern),
            ("Zoom to selection", "Shift+5", Zoom::Selection),
        ];
        for (label, keys, op) in items {
            if ui
                .add(egui::Button::new(label).shortcut_text(keys))
                .clicked()
            {
                zoom(state, project, op);
                ui.close();
            }
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("New-note velocity");
        ui.add(egui::DragValue::new(&mut state.roll.velocity).range(1..=127));
    });
}

/// Horizontal zoom from the window menu and the window-local keys.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Zoom {
    /// Multiply the step width.
    By(f32),
    /// A fixed step width (Shift+1–3).
    Level(f32),
    /// The whole pattern in the window (Shift+4).
    Pattern,
    /// The time selection, or the selected notes (Shift+5).
    Selection,
}

/// The fixed zoom levels of Shift+1, Shift+2 and Shift+3, in points per step.
const ZOOM_LEVELS: [f32; 3] = [12.0, 24.0, 48.0];

fn zoom(state: &mut UiState, project: &Project, zoom: Zoom) {
    let clamp = |px: f32| px.clamp(STEP_WIDTH_RANGE.0, STEP_WIDTH_RANGE.1);
    let range = match zoom {
        Zoom::By(factor) => {
            state.roll.px_per_step = clamp(state.roll.px_per_step * factor);
            return;
        }
        Zoom::Level(px) => {
            state.roll.px_per_step = clamp(px);
            return;
        }
        Zoom::Pattern => Some((0, project.current_pattern().length_ticks().max(1))),
        Zoom::Selection => state.roll.time_selection.or_else(|| {
            let id = project.channels.get(state.selected_channel)?.id;
            let picked = project
                .current_pattern()
                .notes_of(id)
                .filter(|n| state.roll.selection.contains(&(n.start, n.key)));
            picked.fold(None, |span, n| {
                let (a, b) = span.unwrap_or((n.start, n.start + n.length));
                Some((a.min(n.start), b.max(n.start + n.length)))
            })
        }),
    };
    let (Some((from, to)), Some((_, clip))) = (range, state.roll.last_view) else {
        return;
    };
    let steps = (to.saturating_sub(from)).max(1) as f32 / TICKS_PER_STEP as f32;
    state.roll.px_per_step = clamp((clip.width() - KEYS_WIDTH) / steps);
    let x = from as f32 / TICKS_PER_STEP as f32 * state.roll.px_per_step;
    state.roll.jump = Some(vec2(x, state.roll.offset.y));
}

/// The snap values, the current one ticked (header picker and window menu).
fn snap_list(ui: &mut egui::Ui, state: &mut UiState) {
    let snap = state.roll.snap;
    for value in Snap::ALL {
        if ui.selectable_label(snap == value, value.label()).clicked() {
            state.roll.snap = value;
            ui.close();
        }
    }
}

/// Single notes or a chord for the Draw tool (header picker and window menu):
/// "Note", then one column per chord family.
fn chord_list(ui: &mut egui::Ui, state: &mut UiState) {
    let chord = state.roll.chord;
    if ui.selectable_label(chord.is_none(), "Note").clicked() {
        state.roll.chord = None;
        ui.close();
    }
    ui.separator();
    ui.horizontal_top(|ui| {
        for (family, chords) in musiccraft_project::music::CHORD_GROUPS {
            ui.vertical(|ui| {
                ui.weak(family);
                for (name, _) in chords {
                    if ui.selectable_label(chord == Some(name), *name).clicked() {
                        state.roll.chord = Some(name);
                        ui.close();
                    }
                }
            });
        }
    });
}

fn canvas(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let project = frame.project;
    let channel_index = state.selected_channel;
    let Some(channel) = project.channels.get(channel_index) else {
        return;
    };
    let pattern = project.current_pattern();
    let context = (channel_index, project.current_pattern);
    if state.roll.selection_for != Some(context) {
        state.roll.selection.clear();
        state.roll.selection_for = Some(context);
    }
    let length = pattern.length_ticks();
    let bar = pattern.time_signature.bar_ticks();
    // Room to write past the pattern's end, so the scroll bar always has
    // somewhere to go (the playlist does the same).
    let visible_ticks = (length.div_ceil(bar.max(1)) + EXTRA_BARS).max(MIN_BARS) * bar;
    let px_per_tick = state.roll.px_per_step / TICKS_PER_STEP as f32;
    let key_h = state.roll.key_height;
    let size = vec2(
        KEYS_WIDTH + visible_ticks as f32 * px_per_tick,
        RULER_HEIGHT + KEYS as f32 * key_h + state.roll.lane(),
    );

    // The scroll/zoom row runs along the top and the vertical bar down the
    // right; the view (ruler, keyboard, grid and lane) fills the rest.
    let body = ui.available_rect_before_wrap();
    ui.advance_cursor_after_rect(body);
    let view_rect = crate::scrollbars::view_rect(body);
    let mut view_ui = ui.new_child(egui::UiBuilder::new().max_rect(view_rect));
    // Alt+wheel edits the note under the pointer (`alt_wheel`), so the view
    // must not scroll with it.
    let over = ui.input(|i| i.pointer.hover_pos().is_some_and(|p| view_rect.contains(p)));
    if over && ui.input(|i| i.modifiers.alt) {
        ui.ctx()
            .input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    }
    let mut scroll = egui::ScrollArea::both()
        .id_salt("roll-scroll")
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(jump) = state.roll.jump.take() {
        scroll = scroll.scroll_offset(jump);
    } else if state.roll.centred_for != Some(channel_index) {
        // Centre vertically on the channel's notes, or on C5 when it has none.
        let keys: Vec<u8> = pattern.notes_of(channel.id).map(|n| n.key).collect();
        let target = match (keys.iter().min(), keys.iter().max()) {
            (Some(lo), Some(hi)) => (u32::from(*lo) + u32::from(*hi)) / 2,
            _ => 60,
        };
        // The middle of the key's row goes to the middle of the grid, between
        // the ruler and the property lane.
        let row = row_of_key(u8::try_from(target).unwrap_or(60)) as f32;
        let grid_h = view_rect.height() - RULER_HEIGHT - state.roll.lane();
        let y = (row + 0.5) * key_h - grid_h / 2.0;
        scroll = scroll.vertical_scroll_offset(y.max(0.0));
        state.roll.centred_for = Some(channel_index);
    }
    let output = scroll.show(&mut view_ui, |ui| {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let clip = ui.clip_rect();
        let grid = Grid {
            origin: pos2(rect.left() + KEYS_WIDTH, rect.top() + RULER_HEIGHT),
            px_per_tick,
            row_height: key_h,
            rows: KEYS,
            scales: None,
        };
        state.roll.last_view = Some((grid, clip));
        // The keyboard keeps middle-drag for zooming.
        let lanes = Rect::from_min_max(pos2(clip.left() + KEYS_WIDTH, clip.top()), clip.max);
        if let Some(offset) = crate::scrollbars::middle_pan(ui, lanes, clip.min - rect.min) {
            state.roll.jump = Some(offset);
        }
        let painter = ui.painter_at(clip);
        draw_background(
            &painter,
            &grid,
            (clip, rect),
            length,
            (&state.roll.meter, state.roll.scale),
        );
        if let Some(range) = state.roll.time_selection {
            let lanes = Rect::from_min_max(clip.min, clip.max - vec2(0.0, state.roll.lane()));
            draw_time_wash(&painter, &grid, lanes, range);
        }

        // Ghost notes: other channels' notes, in one neutral tone.
        let ghosts = pattern
            .notes
            .iter()
            .filter(|n| state.roll.ghosts && n.channel != channel.id);
        for note in ghosts {
            let r = grid.span(row_of_key(note.key), note.start, note.length);
            painter.rect_filled(r.shrink2(vec2(0.5, 1.0)), 2.0, theme::GHOST_NOTE);
        }
        let notes: Vec<&Note> = pattern.notes_of(channel.id).collect();
        let color = crate::channel_color(project, channel_index);
        let labels = state.roll.note_labels;
        // While dragging, the affected notes are drawn at their new place.
        let moving: Vec<Note> = match state.roll.drag {
            Some(d) if d.kind == DragKind::Create => Vec::new(),
            Some(d) => {
                let mut group = selected_notes(state, &notes);
                if group.is_empty() {
                    group.push(d.original);
                }
                group
            }
            None => Vec::new(),
        };
        let erasing = state.roll.tool == Tool::Delete;
        for note in &notes {
            if moving.contains(note)
                || (erasing && state.roll.stroke.contains(note))
                || state.roll.right_erase.contains(note)
            {
                continue;
            }
            let selected = state.roll.selection.contains(&(note.start, note.key));
            let tint = if note.props.color > 0 {
                theme::note_color(note.props.color)
            } else {
                color
            };
            let r = grid.span(row_of_key(note.key), note.start, note.length);
            if note.props.muted {
                // Muted notes are hollow.
                let edge = if selected {
                    theme::NOTE_SELECTED
                } else {
                    tint.gamma_multiply(0.7)
                };
                painter.rect_filled(r.shrink(1.0), 3.0, tint.gamma_multiply(0.12));
                painter.rect_stroke(
                    r.shrink(1.0),
                    3.0,
                    Stroke::new(1.0, edge),
                    egui::StrokeKind::Inside,
                );
            } else if selected {
                // A selected note's whole body turns to the selection tone.
                draw_note(
                    &painter,
                    &grid,
                    note.start,
                    note.length,
                    note.key,
                    127,
                    theme::NOTE_SELECTED,
                    (false, labels),
                );
            } else {
                draw_note(
                    &painter,
                    &grid,
                    note.start,
                    note.length,
                    note.key,
                    note.velocity,
                    tint,
                    (false, labels),
                );
            }
            let mark = if selected && !note.props.muted {
                theme::at_strength(theme::BACKGROUND, 170)
            } else {
                theme::at_strength(theme::TEXT, 160)
            };
            if note.props.porta {
                // A small curve at the right end marks a portamento note.
                let size = (r.height() - 6.0).min(r.width() - 4.0).max(2.0);
                let a = pos2(r.right() - 3.0 - size, r.bottom() - 3.0);
                let b = pos2(r.right() - 3.0, r.top() + 3.0);
                painter.add(egui::Shape::QuadraticBezier(
                    egui::epaint::QuadraticBezierShape::from_points_stroke(
                        [a, pos2(a.x, b.y), b],
                        false,
                        Color32::TRANSPARENT,
                        Stroke::new(1.5, mark),
                    ),
                ));
            }
            if note.slide {
                // A ramp at the right end marks a slide note.
                let r = grid
                    .span(row_of_key(note.key), note.start, note.length)
                    .shrink2(vec2(1.0, 2.0));
                let w = r.width().min(r.height() * 1.5);
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        pos2(r.right() - w, r.bottom()),
                        r.right_top(),
                        r.right_bottom(),
                    ],
                    mark,
                    Stroke::NONE,
                ));
            }
        }
        if state.roll.tool == Tool::Paint {
            for note in &state.roll.stroke {
                draw_note(
                    &painter,
                    &grid,
                    note.start,
                    note.length,
                    note.key,
                    note.velocity,
                    color,
                    (true, labels),
                );
            }
        }
        if let Some(drag) = state.roll.drag {
            if drag.kind == DragKind::Create {
                draw_note(
                    &painter,
                    &grid,
                    drag.start,
                    drag.length,
                    drag.key,
                    state.roll.velocity,
                    color,
                    (true, labels),
                );
            } else {
                let dt = i64::from(drag.start) - i64::from(drag.original.start);
                let dk = i32::from(drag.key) - i32::from(drag.original.key);
                let (dt, dk) = clamp_delta(&moving, dt, dk);
                let dl = i64::from(drag.length) - i64::from(drag.original.length);
                for note in &moving {
                    let (start, key, length) = match drag.kind {
                        DragKind::Resize => (
                            note.start,
                            note.key,
                            u32::try_from((i64::from(note.length) + dl).max(1)).unwrap_or(1),
                        ),
                        _ => {
                            let (start, key) = shifted(note, dt, dk);
                            (start, key, note.length)
                        }
                    };
                    draw_note(
                        &painter,
                        &grid,
                        start,
                        length,
                        key,
                        note.velocity,
                        color,
                        (true, labels),
                    );
                }
            }
        }
        if let Some((origin, current)) = state.roll.marquee {
            let area = Rect::from_two_pos(origin, current);
            painter.rect_filled(area, 0.0, theme::SELECTION_FILL);
            painter.rect_stroke(
                area,
                0.0,
                Stroke::new(1.0, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }

        let playhead = pattern_position(project, frame.engine.position_ticks);
        if let Some(tick) = playhead.filter(|_| frame.engine.playing) {
            let x = grid.x(tick as f32);
            painter.line_segment(
                [pos2(x, clip.top()), pos2(x, clip.bottom())],
                Stroke::new(1.5, theme::PLAYHEAD),
            );
        }

        let boxes: Vec<(usize, Rect)> = notes
            .iter()
            .enumerate()
            .map(|(i, n)| (i, grid.span(row_of_key(n.key), n.start, n.length)))
            .collect();
        edit(
            ui,
            state,
            &grid,
            &response,
            clip,
            &notes,
            &boxes,
            channel_index,
            actions,
        );
        draw_ruler(&painter, &grid, clip, length, &state.roll.meter);
        if let Some((a, b)) = state.roll.time_selection {
            let band = Rect::from_x_y_ranges(
                grid.x(a as f32)..=grid.x(b as f32),
                clip.top()..=clip.top() + RULER_HEIGHT,
            );
            painter.rect_filled(band, 0.0, theme::at_strength(theme::HIGHLIGHT, 60));
        }
        ruler(ui, state, project, &grid, clip, (&notes, actions));
        // The ruler marks the playhead whether or not it is moving.
        if let Some(tick) = playhead {
            let x = grid.x(tick as f32);
            let top = clip.top() + RULER_HEIGHT;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(x - 5.0, top - 8.0),
                    pos2(x + 5.0, top - 8.0),
                    pos2(x, top),
                ],
                theme::PLAYHEAD,
                Stroke::NONE,
            ));
        }
        let ghosts: Vec<&Note> = pattern
            .notes
            .iter()
            .filter(|n| n.channel != channel.id)
            .collect();
        velocity_lane(
            ui,
            state,
            (&grid, clip),
            (&notes, &ghosts),
            (channel_index, crate::channel_color(project, channel_index)),
            actions,
        );
        if response.hovered() && !ui.input(|i| i.modifiers.alt) {
            let zoom = ui.input(|i| i.zoom_delta());
            if zoom != 1.0 {
                state.roll.px_per_step =
                    (state.roll.px_per_step * zoom).clamp(STEP_WIDTH_RANGE.0, STEP_WIDTH_RANGE.1);
            }
        }
        keyboard(ui, state, (&grid, clip), &notes, channel_index, actions);
    });
    state.roll.offset = output.state.offset;
    let notes: Vec<&Note> = pattern.notes_of(channel.id).collect();
    let view = output.inner_rect.size();
    scroll_row(
        ui,
        state,
        body,
        view,
        (&notes, visible_ticks, bar),
        crate::channel_color(project, channel_index),
    );
    corner(
        ui,
        state,
        (project, body),
        crate::channel_color(project, channel_index),
        actions,
    );
}

/// The scroll/zoom row, the vertical zoom box and the vertical scroll bar.
/// `view` is the size of the visible view; `content` holds the notes, the
/// ticks the canvas covers and one bar's length.
fn scroll_row(
    ui: &egui::Ui,
    state: &mut UiState,
    body: Rect,
    view: Vec2,
    (notes, total, bar_length): (&[&Note], u32, u32),
    color: Color32,
) {
    use crate::scrollbars::{self, Setup};
    let roll = &mut state.roll;
    let tps = TICKS_PER_STEP as f32;
    let (lo, hi) = STEP_WIDTH_RANGE;
    let setup = Setup {
        view,
        offset: roll.offset,
        lead: KEYS_WIDTH,
        top: RULER_HEIGHT,
        tail: roll.lane(),
        x: (
            roll.px_per_step / tps,
            total as f32,
            bar_length as f32,
            (lo / tps, hi / tps),
        ),
        y: (roll.key_height, KEYS as f32, KEY_HEIGHT_RANGE, KEY_HEIGHT),
        y_label: "Key height",
    };
    let out = scrollbars::around(
        ui,
        body,
        egui::Id::new("roll-bars"),
        setup,
        |p, rail, span| {
            overview(p, rail, (notes, color), span);
        },
    );
    if let Some(scale) = out.scale {
        roll.px_per_step = scale * tps;
    }
    if let Some(row) = out.row {
        roll.key_height = row;
    }
    if out.jump.is_some() {
        roll.jump = out.jump;
    }
}

/// A miniature of the channel's notes along the scroll rail, brighter inside
/// the visible range.
fn overview(
    painter: &egui::Painter,
    rail: Rect,
    (notes, color): (&[&Note], Color32),
    view: crate::scrollbars::Span,
) {
    let total = view.total.max(1.0);
    for note in notes {
        let x0 = rail.left() + note.start as f32 / total * rail.width();
        let w = (note.length as f32 / total * rail.width()).max(1.5);
        let y = rail.top() + row_of_key(note.key) as f32 / KEYS as f32 * rail.height();
        let inside = (note.start as f32) < view.first + view.span
            && (note.start + note.length) as f32 > view.first;
        let color = theme::at_strength(color, if inside { 200 } else { 90 });
        painter.line_segment([pos2(x0, y), pos2(x0 + w, y)], Stroke::new(1.0, color));
    }
}

/// The corner block above the keyboard: the colour group of new notes, with
/// the Porta and Slide toggles on its right edge. Right-click offers to give
/// the selected notes this colour.
fn corner(
    ui: &egui::Ui,
    state: &mut UiState,
    (project, body): (&Project, Rect),
    own: Color32,
    actions: &mut Vec<Action>,
) {
    let block = Rect::from_min_size(
        body.min,
        vec2(KEYS_WIDTH, crate::scrollbars::BAR + RULER_HEIGHT),
    );
    let group = state.roll.color;
    let fill = group_color(group, own);
    let swatch = ui
        .interact(block, egui::Id::new("roll-corner"), Sense::click())
        .hint(format!(
            "Colour group {} of new notes · click to choose · right-click to apply it to the selected notes",
            group + 1
        ));
    let painter = ui.painter();
    painter.rect_filled(block, 0.0, fill);
    if swatch.hovered() {
        painter.rect_filled(block, 0.0, theme::BAR_GROUP);
    }
    painter.text(
        block.left_bottom() + vec2(3.0, -2.0),
        Align2::LEFT_BOTTOM,
        (group + 1).to_string(),
        FontId::proportional(theme::FONT_LABEL),
        theme::BACKGROUND,
    );
    let side = (block.height() / 2.0).floor();
    for (i, (icon, on, hint)) in [
        (
            Icon::Porta,
            state.roll.porta,
            "Porta (O): new notes glide from the previous note",
        ),
        (
            Icon::SlideRamp,
            state.roll.slide,
            "Slide (S): new notes bend the notes they overlap",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let rect = Rect::from_min_size(
            pos2(block.right() - side, block.top() + i as f32 * side),
            vec2(side, side),
        );
        let response = ui
            .interact(
                rect,
                egui::Id::new(("roll-corner-toggle", i)),
                Sense::click(),
            )
            .hint(hint);
        let tone = if on { 255 } else { 80 };
        icons::paint(
            ui.painter(),
            rect,
            icon,
            theme::at_strength(theme::BACKGROUND, tone),
        );
        if response.clicked() {
            match i {
                0 => state.roll.porta = !state.roll.porta,
                _ => state.roll.slide = !state.roll.slide,
            }
        }
    }
    let toggle = swatch.clicked().then_some(egui::SetOpenCommand::Toggle);
    egui::Popup::menu(&swatch)
        .id(egui::Id::new("roll-colour-groups"))
        .open_memory(toggle)
        .show(|ui| colour_groups(ui, state, own));
    swatch.context_menu(|ui| {
        let any = !state.roll.selection.is_empty();
        let item =
            egui::Button::new("Apply this colour to the selected notes").shortcut_text("Alt+C");
        if crate::widgets::add_enabled(ui, any, item).clicked() {
            edit_op(state, project, EditOp::ApplyColour, actions);
            ui.close();
        }
    });
}

/// Colour group `group` (0 is the channel's own colour, `own`).
pub(crate) fn group_color(group: u8, own: Color32) -> Color32 {
    if group == 0 {
        own
    } else {
        theme::note_color(group)
    }
}

/// The 4 × 4 grid of colour groups for new notes.
fn colour_groups(ui: &mut egui::Ui, state: &mut UiState, own: Color32) {
    if let Some(group) = colour_grid(ui, state.roll.color, own) {
        state.roll.color = group;
        ui.close();
    }
}

/// The 4 × 4 grid of colour groups, numbered down each column, with
/// `current` outlined; returns the group clicked.
pub(crate) fn colour_grid(ui: &mut egui::Ui, current: u8, own: Color32) -> Option<u8> {
    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
    let mut picked = None;
    for row in 0..4_u8 {
        ui.horizontal(|ui| {
            for column in 0..4_u8 {
                let group = column * 4 + row;
                let (rect, response) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                let painter = ui.painter();
                painter.rect_filled(rect, 2.0, group_color(group, own));
                if group == current || response.hovered() {
                    painter.rect_stroke(
                        rect,
                        2.0,
                        Stroke::new(2.0, theme::TEXT),
                        egui::StrokeKind::Inside,
                    );
                }
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    (group + 1).to_string(),
                    FontId::proportional(theme::FONT_LABEL),
                    theme::BACKGROUND,
                );
                let n = group + 1;
                if response
                    .hint(format!("Colour group {n} (MIDI channel {n} for plugins)"))
                    .clicked()
                {
                    picked = Some(group);
                }
            }
        });
    }
    picked
}

#[allow(clippy::too_many_arguments)]
fn draw_note(
    painter: &egui::Painter,
    grid: &Grid,
    start: u32,
    length: u32,
    key: u8,
    velocity: u8,
    color: Color32,
    (ghost, label): (bool, bool),
) {
    let rect = grid
        .span(row_of_key(key), start, length)
        .shrink2(vec2(0.5, 0.5));
    let alpha = if ghost {
        150
    } else {
        110 + u32::from(velocity)
    };
    let fill = theme::at_strength(color, alpha.min(255) as u8);
    painter.rect_filled(rect, 3.0, fill);
    painter.rect_stroke(
        rect,
        3.0,
        Stroke::new(1.0, theme::shade(120)),
        egui::StrokeKind::Inside,
    );
    // The key name, cut short with an ellipsis when the note is narrow.
    let room = rect.width() - 6.0;
    if label && room > 8.0 && rect.height() >= 8.0 {
        let mut job = egui::text::LayoutJob::simple_singleline(
            key_name(key),
            FontId::proportional(theme::FONT_CAPTION),
            theme::BACKGROUND,
        );
        job.wrap = egui::text::TextWrapping {
            max_width: room,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        let galley = painter.layout_job(job);
        let at = pos2(rect.left() + 4.0, rect.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, theme::BACKGROUND);
    }
}

fn draw_background(
    painter: &egui::Painter,
    grid: &Grid,
    (clip, rect): (Rect, Rect),
    length: u32,
    (meter, scale): (&Meter, Option<(u8, usize)>),
) {
    let first = grid.row_at(clip.top().max(grid.origin.y)).unwrap_or(0);
    let last = grid.row_at(clip.bottom() - 1.0).unwrap_or(KEYS - 1);
    for row in first..=last {
        let key = key_of_row(row);
        let y = grid.y(row);
        let fill = row_fill(key, scale);
        painter.rect_filled(
            Rect::from_min_max(
                pos2(grid.origin.x, y),
                pos2(rect.right(), y + grid.row_height),
            ),
            0.0,
            fill,
        );
        if key % 12 == 0 {
            painter.line_segment(
                [
                    pos2(grid.origin.x, y + grid.row_height),
                    pos2(rect.right(), y + grid.row_height),
                ],
                Stroke::new(1.0, theme::GRID_BEAT),
            );
        }
    }
    // Past the pattern end is shaded.
    let end = grid.x(length as f32);
    painter.rect_filled(
        Rect::from_min_max(pos2(end, grid.origin.y), rect.right_bottom()),
        0.0,
        theme::PAST_END,
    );
    let first_tick = grid.tick_at(clip.left()) as u32 / TICKS_PER_STEP * TICKS_PER_STEP;
    let last_tick = grid.tick_at(clip.right()) as u32;
    let sig = meter.first();
    for tick in (first_tick..=last_tick).step_by(TICKS_PER_STEP as usize) {
        let color = if tick % sig.bar_ticks() == 0 {
            theme::GRID_BAR
        } else if tick % sig.beat_ticks() == 0 {
            theme::GRID_BEAT
        } else {
            theme::GRID_LINE
        };
        let x = grid.x(tick as f32);
        painter.line_segment(
            [pos2(x, grid.origin.y), pos2(x, rect.bottom())],
            Stroke::new(1.0, color),
        );
    }
}

/// `key` moved to the nearest key of `scale` (ties go down, as in
/// `note.to_scale`); unchanged without a scale.
fn fit_to_scale(key: u8, scale: Option<(u8, usize)>) -> u8 {
    let Some((root, degrees)) = scale.and_then(|(root, s)| {
        musiccraft_project::music::SCALES
            .get(s)
            .map(|(_, d)| (root, *d))
    }) else {
        return key;
    };
    let fits = |k: i32| {
        u8::try_from(k)
            .ok()
            .filter(|k| *k <= 127 && musiccraft_project::music::in_scale(*k, root, degrees))
    };
    (0..12)
        .find_map(|d| fits(i32::from(key) - d).or_else(|| fits(i32::from(key) + d)))
        .unwrap_or(key)
}

/// A key row's tone. Without a scale, black-key rows are darker. With one,
/// that shading is replaced: in-scale rows a step lighter than white-key
/// rows (the root's with a faint accent), the rest like black-key rows.
fn row_fill(key: u8, scale: Option<(u8, usize)>) -> Color32 {
    let degrees = scale.and_then(|(root, s)| {
        musiccraft_project::music::SCALES
            .get(s)
            .map(|(_, d)| (root, *d))
    });
    match degrees {
        None if is_black(key) => theme::BLACK_KEY_ROW,
        None => theme::WHITE_KEY_ROW,
        Some((root, degrees)) if musiccraft_project::music::in_scale(key, root, degrees) => {
            let lighter = theme::mix(theme::WHITE_KEY_ROW, theme::RAISED, 0.45);
            if key % 12 == root % 12 {
                theme::mix(lighter, theme::ACCENT, 0.12)
            } else {
                lighter
            }
        }
        Some(_) => theme::BLACK_KEY_ROW,
    }
}

fn draw_ruler(painter: &egui::Painter, grid: &Grid, clip: Rect, length: u32, meter: &Meter) {
    let ruler = Rect::from_min_max(
        pos2(clip.left(), clip.top()),
        pos2(clip.right(), clip.top() + RULER_HEIGHT),
    );
    painter.rect_filled(ruler, 0.0, theme::PANEL_DARK);
    let first_bar = meter.bar(grid.tick_at(clip.left()).max(0.0) as u32).index;
    let last_bar = meter.bar(grid.tick_at(clip.right()).max(0.0) as u32).index + 1;
    // Narrow bars show only every 2nd or 4th number.
    let bar_width =
        grid.x(meter.bar_start(first_bar + 1) as f32) - grid.x(meter.bar_start(first_bar) as f32);
    let every = if bar_width >= 28.0 {
        1
    } else if bar_width >= 14.0 {
        2
    } else {
        4
    };
    for bar in first_bar..=last_bar {
        let start = meter.bar_start(bar);
        let x = grid.x(start as f32);
        let past = start >= length;
        // Short beat ticks rise from the bottom edge.
        let info = meter.bar(start);
        let beat = info.sig.beat_ticks().max(1);
        for at in (start + beat..info.end()).step_by(beat as usize) {
            let bx = grid.x(at as f32);
            painter.line_segment(
                [pos2(bx, ruler.bottom() - 3.0), pos2(bx, ruler.bottom())],
                Stroke::new(1.0, theme::GRID_BEAT),
            );
        }
        painter.line_segment(
            [pos2(x, ruler.bottom() - 6.0), pos2(x, ruler.bottom())],
            Stroke::new(1.0, theme::GRID_BAR),
        );
        if bar % every == 0 {
            painter.text(
                pos2(x + 3.0, ruler.top() + 2.0),
                Align2::LEFT_TOP,
                (bar + 1).to_string(),
                FontId::monospace(theme::FONT_LABEL),
                if past { theme::TEXT_DIM } else { theme::TEXT },
            );
        }
    }
}

/// The splitter between the grid and the property lane: drag it to resize
/// the lane, double-click to hide or show the lane, right-click to restore
/// its default height.
fn splitter(ui: &egui::Ui, state: &mut UiState, clip: Rect) {
    let y = clip.bottom() - state.roll.lane();
    let rect = Rect::from_min_max(pos2(clip.left(), y - SPLITTER), pos2(clip.right(), y));
    let response = ui
        .interact(rect, ui.id().with("roll-splitter"), Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::ResizeVertical)
        .hint(
            "Drag to resize the property lane · double-click to hide or show it (Shift+Enter) · right-click for the default height",
        );
    let painter = ui.painter_at(clip);
    painter.rect_filled(rect, 0.0, theme::PANEL_DARK);
    let grip = if response.hovered() || response.dragged() {
        theme::TEXT
    } else {
        theme::TEXT_FAINT
    };
    for dx in [-4.0, 0.0, 4.0] {
        painter.circle_filled(rect.center() + vec2(dx, 0.0), 1.0, grip);
    }
    let roll = &mut state.roll;
    if response.double_clicked() {
        roll.lane_shown = !roll.lane_shown;
    } else if response.secondary_clicked() {
        roll.lane_height = LANE_HEIGHT;
        roll.lane_shown = true;
    } else if response.dragged()
        && let Some(p) = response.interact_pointer_pos()
    {
        roll.lane_shown = true;
        let most = LANE_HEIGHT_RANGE.1.min(clip.height() * 0.7);
        roll.lane_height =
            (clip.bottom() - p.y).clamp(LANE_HEIGHT_RANGE.0, most.max(LANE_HEIGHT_RANGE.0));
    }
}

/// The time selection's faint wash over the grid, with its two edges.
/// `clip` is the visible view above the property lane.
fn draw_time_wash(painter: &egui::Painter, grid: &Grid, clip: Rect, (a, b): (u32, u32)) {
    let (x0, x1) = (grid.x(a as f32), grid.x(b as f32));
    let top = clip.top() + RULER_HEIGHT;
    let bottom = clip.bottom();
    painter.rect_filled(
        Rect::from_x_y_ranges(x0..=x1, top..=bottom),
        0.0,
        theme::at_strength(theme::HIGHLIGHT, 16),
    );
    for x in [x0, x1] {
        painter.line_segment(
            [pos2(x, top), pos2(x, bottom)],
            Stroke::new(1.0, theme::at_strength(theme::HIGHLIGHT, 160)),
        );
    }
}

/// The ruler's gestures: a click seeks, a drag makes a time selection and
/// selects the notes that overlap it.
fn ruler(
    ui: &egui::Ui,
    state: &mut UiState,
    project: &Project,
    grid: &Grid,
    clip: Rect,
    (notes, actions): (&[&Note], &mut Vec<Action>),
) {
    let area = Rect::from_min_max(
        pos2(clip.left() + KEYS_WIDTH, clip.top()),
        pos2(clip.right(), clip.top() + RULER_HEIGHT),
    );
    let response = ui
        .interact(area, ui.id().with("roll-ruler"), Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .hint("Click to seek · drag to select time");
    let snap = state.roll.snap;
    let meter = state.roll.meter.clone();
    let tick_at = |x: f32| snap.floor(grid.tick_at(x).max(0.0), &meter);
    if response.clicked() {
        state.roll.time_selection = None;
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(at) = song_position(project, tick_at(pos.x)) {
                actions.push(Action::Seek(at));
            }
        }
    }
    if response.dragged() {
        if let (Some(origin), Some(pos)) = (
            ui.input(|i| i.pointer.press_origin()),
            response.interact_pointer_pos(),
        ) {
            let (a, b) = (
                tick_at(origin.x),
                snap.round(grid.tick_at(pos.x).max(0.0), &meter),
            );
            let range = (a.min(b), a.max(b));
            state.roll.time_selection = (range.1 > range.0).then_some(range);
        }
    }
    if response.drag_stopped() {
        if let Some((a, b)) = state.roll.time_selection {
            state.roll.selection = notes
                .iter()
                .filter(|n| n.start < b && n.start + n.length > a)
                .map(|n| (n.start, n.key))
                .collect();
        }
    }
}

/// Where the playhead goes for pattern tick `tick`: the tick itself in
/// pattern mode; in song mode, the same place inside the clip of this
/// pattern under the playhead, or else inside its first clip.
fn song_position(project: &Project, tick: u32) -> Option<f64> {
    let pattern = project.current_pattern();
    match project.play_mode {
        PlayMode::Pattern => Some(f64::from(tick)),
        PlayMode::Song => {
            let mut clips = project
                .playlist
                .clips
                .iter()
                .filter(|c| c.pattern() == Some(pattern.id));
            let first = clips.next()?;
            let at = first.start + tick.saturating_sub(first.offset);
            Some(f64::from(at.min(first.end().saturating_sub(1))))
        }
    }
}

fn selected_notes(state: &UiState, notes: &[&Note]) -> Vec<Note> {
    notes
        .iter()
        .filter(|n| state.roll.selection.contains(&(n.start, n.key)))
        .map(|n| **n)
        .collect()
}

fn shifted(note: &Note, dt: i64, dk: i32) -> (u32, u8) {
    (
        u32::try_from(i64::from(note.start) + dt).unwrap_or(0),
        u8::try_from(i32::from(note.key) + dk).unwrap_or(note.key),
    )
}

/// The piano roll's Tools menu: note transformations on the shown channel.
fn tools_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    number: &str,
    selected: &[Note],
    actions: &mut Vec<Action>,
    direct: bool,
) {
    let toggles: Vec<Invocation> = selected
        .iter()
        .map(|n| {
            Invocation::new(
                "note.set_slide",
                [
                    number.to_owned(),
                    ticks(n.start),
                    n.key.to_string(),
                    if n.slide { "off" } else { "on" }.to_owned(),
                ],
            )
        })
        .collect();
    let toggle_slide = batch(toggles);
    let mut toggled = None;
    let grid = format!("{}t", state.roll.snap.ticks_at(0, &state.roll.meter));
    let mut run = |args: Vec<String>, name: &str| {
        let mut all = vec![number.to_owned()];
        all.extend(args);
        actions.push(Action::Run(Invocation::new(name, all)));
    };
    let mut body = |ui: &mut egui::Ui| {
        let mut item =
            |ui: &mut egui::Ui, label: &str, hint: &str, name: &str, args: Vec<String>| {
                if ui.button(label).hint(hint.to_owned()).clicked() {
                    run(args, name);
                    ui.close();
                }
            };
        item(
            ui,
            "Strum up",
            "Stagger chord notes, lowest first",
            "note.strum",
            vec!["6t".into(), "up".into()],
        );
        item(
            ui,
            "Strum down",
            "Stagger chord notes, highest first",
            "note.strum",
            vec!["6t".into(), "down".into()],
        );
        let seed = state.begin_gesture().to_string();
        item(
            ui,
            "Humanize",
            "Nudge timing and velocity a little, randomly",
            "note.humanize",
            vec!["4t".into(), "12".into(), seed],
        );
        item(
            ui,
            "Legato",
            "Make every note last until the next one",
            "note.legato",
            vec![],
        );
        item(
            ui,
            "Split at grid lines",
            "Split notes into grid-length pieces",
            "note.chop",
            vec![grid.clone()],
        );
        item(
            ui,
            "Join touching notes",
            "Join touching notes of the same key",
            "note.glue",
            vec![],
        );
        item(
            ui,
            "Mirror in time",
            "Reverse the notes in time",
            "note.flip",
            vec!["time".into()],
        );
        item(
            ui,
            "Mirror in pitch",
            "Turn the melody upside down",
            "note.flip",
            vec!["pitch".into()],
        );
        match state.roll.scale {
            Some((root, scale)) => {
                let name = musiccraft_project::music::SCALES
                    .get(scale)
                    .map_or("major", |(n, _)| n)
                    .replace(' ', "_");
                item(
                    ui,
                    "Snap to scale",
                    "Move notes to the nearest key of the highlighted scale",
                    "note.to_scale",
                    vec![root.to_string(), name],
                );
            }
            None => {
                crate::widgets::add_enabled(ui, false, egui::Button::new("Snap to scale"))
                    .hint("Highlight a scale first");
            }
        }
        let enabled = toggle_slide.is_some();
        if crate::widgets::add_enabled(ui, enabled, egui::Button::new("Toggle slide"))
            .hint("Make the selected notes slide notes, or normal again")
            .clicked()
        {
            toggled = toggle_slide.clone();
            ui.close();
        }
        let root = state.roll.scale.map_or(0, |(root, _)| root);
        let scale_name = state.roll.scale.map(|(_, i)| {
            musiccraft_project::music::SCALES
                .get(i)
                .map_or("major", |(n, _)| n)
                .replace(' ', "_")
        });
        ui.menu_button("Generate", |ui| {
            let mut random = vec![
                "C4".to_owned(),
                "C6".to_owned(),
                "50".to_owned(),
                state.begin_gesture().to_string(),
            ];
            if let Some(name) = &scale_name {
                random.extend([root.to_string(), name.clone()]);
            }
            item(
                ui,
                "Randomize",
                "Replace the notes with random ones on the step grid (in the highlighted scale)",
                "note.randomize",
                random,
            );
            let minor = scale_name.as_deref().is_some_and(|n| n.contains("minor"));
            for (label, degrees) in [("I–V–vi–IV", "1-5-6-4"), ("i–VI–III–VII", "1-6-3-7"), ("ii–V–I", "2-5-1")] {
                item(
                    ui,
                    &format!("Chords {label}"),
                    "Replace the notes with this progression, a bar per chord (key of the highlighted scale)",
                    "note.chords",
                    vec![
                        (48 + root).to_string(),
                        if minor { "minor" } else { "major" }.into(),
                        degrees.into(),
                    ],
                );
            }
        });
        ui.menu_button("Shape", |ui| {
            item(
                ui,
                "Limit to C4–B5",
                "Fold notes into two octaves",
                "note.limit",
                vec!["C4".into(), "B5".into()],
            );
            item(
                ui,
                "Velocity LFO",
                "Sine on velocity, one bar per cycle",
                "note.lfo",
                vec!["velocity".into(), "16".into(), "50".into()],
            );
            item(
                ui,
                "Pan LFO",
                "Sine on pan, one bar per cycle",
                "note.lfo",
                vec!["pan".into(), "16".into(), "80".into()],
            );
            item(
                ui,
                "Scale velocities 75%",
                "Bring velocities closer to the middle",
                "note.scale_levels",
                vec!["velocity".into(), "75".into()],
            );
            item(
                ui,
                "Flam",
                "Add a quieter grace note just before each note",
                "note.flam",
                vec!["6t".into()],
            );
            item(
                ui,
                "Staccato",
                "Half the gap to the next note; accent the beats",
                "note.articulate",
                vec!["50".into(), "10".into()],
            );
            item(
                ui,
                "Tenuto",
                "Hold each note until the next one",
                "note.articulate",
                vec!["100".into()],
            );
            item(
                ui,
                "Keep every 2nd",
                "Keep every second note or chord",
                "note.keep_every",
                vec!["2".into()],
            );
        });
        ui.menu_button("Arpeggiate", |ui| {
            for (label, mode) in [
                ("Up", "up"),
                ("Down", "down"),
                ("Up and down", "updown"),
                ("Random", "random"),
            ] {
                item(
                    ui,
                    label,
                    "Turn chords into notes one at a time, one grid step each",
                    "note.arpeggiate",
                    vec![grid.clone(), mode.into()],
                );
            }
        });
    };
    if direct {
        body(ui);
    } else {
        ui.menu_button("Tools", body);
    }
    if let Some(invocation) = toggled {
        actions.push(Action::Run(invocation));
    }
}

/// Right button: click deletes the note under the pointer; dragging deletes
/// every note the pointer passes over, as one undo step when released.
fn right_erase(
    state: &mut UiState,
    response: &egui::Response,
    notes: &[&Note],
    boxes: &[(usize, Rect)],
    channel: usize,
    actions: &mut Vec<Action>,
) {
    let pressed = response.dragged_by(PointerButton::Secondary)
        || response.secondary_clicked()
        || response.drag_started_by(PointerButton::Secondary);
    if response.drag_started_by(PointerButton::Secondary) {
        state.roll.right_erase.clear();
    }
    if let (true, Some(pos)) = (pressed, response.interact_pointer_pos()) {
        if let Hit::Body(i) | Hit::Edge(i) = hit_test(boxes.iter().copied(), pos) {
            if let Some(note) = notes.get(i) {
                if !state.roll.right_erase.contains(note) {
                    state.roll.right_erase.push(**note);
                }
            }
        }
    }
    if response.drag_stopped_by(PointerButton::Secondary) || response.secondary_clicked() {
        let erased = std::mem::take(&mut state.roll.right_erase);
        for n in &erased {
            state.roll.selection.remove(&(n.start, n.key));
        }
        actions.extend(remove_notes(channel, &erased).map(Action::Run));
    }
}

/// The click tools: Slice cuts the clicked note, Play previews the clicked
/// note or key, Zoom zooms. Returns whether the active tool is one of them.
#[allow(clippy::too_many_arguments)]
fn click_tool(
    ui: &egui::Ui,
    state: &mut UiState,
    response: &egui::Response,
    notes: &[&Note],
    boxes: &[(usize, Rect)],
    channel: usize,
    at: &dyn Fn(Pos2) -> (f32, Option<u8>),
    actions: &mut Vec<Action>,
) -> bool {
    let tool = state.roll.tool;
    if !matches!(tool, Tool::Slice | Tool::Play | Tool::Zoom) {
        return false;
    }
    let Some(p) = response
        .interact_pointer_pos()
        .filter(|_| response.clicked_by(PointerButton::Primary))
    else {
        return true;
    };
    let (tick, key) = at(p);
    let note = match hit_test(boxes.iter().copied(), p) {
        Hit::Body(i) | Hit::Edge(i) => notes.get(i).map(|n| **n),
        Hit::Empty => None,
    };
    match tool {
        Tool::Slice => {
            if let Some(n) = note {
                let snapped = state.roll.snap.round(tick, &state.roll.meter);
                let cut = if snapped > n.start && snapped < n.start + n.length {
                    snapped
                } else {
                    tick.max(0.0) as u32
                };
                if cut > n.start && cut < n.start + n.length {
                    actions.push(Action::Run(Invocation::new(
                        "note.slice",
                        [
                            (channel + 1).to_string(),
                            format!("{}t", n.start),
                            n.key.to_string(),
                            format!("{cut}t"),
                        ],
                    )));
                }
            }
        }
        Tool::Play => {
            if let Some(key) = note.map(|n| n.key).or(key) {
                actions.push(Action::PreviewShort { channel, key });
            }
        }
        _ => {
            let factor = if ui.input(|i| i.modifiers.shift) {
                1.0 / 1.5
            } else {
                1.5
            };
            state.roll.px_per_step = (state.roll.px_per_step * factor).clamp(6.0, 96.0);
        }
    }
    true
}

/// Paint and delete strokes. Returns `true` when the tool handled the input.
#[allow(clippy::too_many_arguments)]
fn tool_stroke(
    ui: &egui::Ui,
    state: &mut UiState,
    response: &egui::Response,
    notes: &[&Note],
    boxes: &[(usize, Rect)],
    channel: usize,
    at: &dyn Fn(Pos2) -> (f32, Option<u8>),
    actions: &mut Vec<Action>,
) -> bool {
    let tool = state.roll.tool;
    if !matches!(tool, Tool::Paint | Tool::Delete | Tool::Mute) {
        return false;
    }
    let snap = state.roll.snap;
    let meter = state.roll.meter.clone();
    let pointer = response.interact_pointer_pos();
    let under = |p: Pos2| match hit_test(boxes.iter().copied(), p) {
        Hit::Body(i) | Hit::Edge(i) => notes.get(i).map(|n| **n),
        Hit::Empty => None,
    };
    match tool {
        Tool::Mute => {
            if response.drag_started_by(PointerButton::Primary) {
                state.roll.stroke.clear();
            }
            let pressed = response.dragged_by(PointerButton::Primary)
                || response.clicked_by(PointerButton::Primary);
            if let (true, Some(p)) = (pressed, pointer) {
                if let Some(note) = under(p) {
                    if !state.roll.stroke.contains(&note) {
                        state.roll.stroke.push(note);
                    }
                }
            }
            if response.drag_stopped_by(PointerButton::Primary)
                || response.clicked_by(PointerButton::Primary)
            {
                let ch = (channel + 1).to_string();
                let toggles = std::mem::take(&mut state.roll.stroke)
                    .iter()
                    .map(|n| {
                        Invocation::new(
                            "note.set_prop",
                            [
                                ch.clone(),
                                ticks(n.start),
                                n.key.to_string(),
                                "muted".to_owned(),
                                if n.props.muted { "off" } else { "on" }.to_owned(),
                            ],
                        )
                    })
                    .collect();
                actions.extend(batch(toggles).map(Action::Run));
            }
            true
        }
        Tool::Delete => {
            if response.drag_started_by(PointerButton::Primary) {
                state.roll.stroke.clear();
            }
            let pressed = response.dragged_by(PointerButton::Primary)
                || response.clicked_by(PointerButton::Primary);
            if let (true, Some(p)) = (pressed, pointer) {
                if let Some(note) = under(p) {
                    if !state.roll.stroke.contains(&note) {
                        state.roll.stroke.push(note);
                    }
                }
            }
            if response.drag_stopped_by(PointerButton::Primary)
                || response.clicked_by(PointerButton::Primary)
            {
                let erased = std::mem::take(&mut state.roll.stroke);
                for n in &erased {
                    state.roll.selection.remove(&(n.start, n.key));
                }
                actions.extend(remove_notes(channel, &erased).map(Action::Run));
            }
            true
        }
        Tool::Paint => {
            if state.roll.drag.is_some() || state.roll.marquee.is_some() {
                // A drag that started on a note moves it, as with the draw tool.
                return false;
            }
            if response.drag_started_by(PointerButton::Primary) {
                state.roll.stroke.clear();
                let origin = ui.input(|i| i.pointer.press_origin());
                if origin.and_then(under).is_some() {
                    // Starting on a note: behave like the draw tool.
                    return false;
                }
            }
            let painting = response.dragged_by(PointerButton::Primary)
                || response.drag_stopped_by(PointerButton::Primary);
            // The stroke starts where the button went down.
            let origin = response
                .drag_started_by(PointerButton::Primary)
                .then(|| ui.input(|i| i.pointer.press_origin()))
                .flatten();
            for p in origin.into_iter().chain(pointer.filter(|_| painting)) {
                if let (tick, Some(key)) = at(p) {
                    let start = snap.floor(tick, &meter);
                    let length = state.roll.last_length.max(1);
                    let free = state
                        .roll
                        .stroke
                        .last()
                        .is_none_or(|last| start >= last.start + last.length);
                    if free {
                        state.roll.stroke.push(Note {
                            props: new_props(state),
                            slide: state.roll.slide,
                            channel: musiccraft_project::ChannelId(0),
                            start,
                            length,
                            key,
                            velocity: state.roll.velocity,
                            pan: 0,
                        });
                        actions.push(Action::PreviewShort { channel, key });
                    }
                }
            }
            if response.drag_stopped_by(PointerButton::Primary) {
                let painted = std::mem::take(&mut state.roll.stroke);
                actions.extend(add_notes(channel, &painted, 0).map(Action::Run));
                return true;
            }
            // Painting is a drag; clicks and drags on notes fall through.
            !state.roll.stroke.is_empty()
        }
        _ => false,
    }
}

/// Alt+wheel over a note steps the property lane's value of that note (or
/// of every selected note, when it is one of them) by a tenth of the range
/// per notch, Ctrl+Alt+wheel by a fiftieth. A run of notches over the same
/// notes is one undo step.
fn alt_wheel(
    ui: &egui::Ui,
    state: &mut UiState,
    pos: Pos2,
    (notes, boxes): (&[&Note], &[(usize, Rect)]),
    channel: usize,
    actions: &mut Vec<Action>,
) {
    let modifiers = ui.input(|i| i.modifiers);
    if !modifiers.alt {
        return;
    }
    let notches = crate::scrollbars::wheel_notches(ui);
    let hovered = match hit_test(boxes.iter().copied(), pos) {
        Hit::Body(i) | Hit::Edge(i) => notes.get(i).copied(),
        Hit::Empty => None,
    };
    let Some(hovered) = hovered.filter(|_| notches != 0.0) else {
        return;
    };
    let step = if modifiers.command {
        1.0 / 50.0
    } else {
        1.0 / 10.0
    };
    let targets: Vec<&Note> = if state.roll.selection.contains(&(hovered.start, hovered.key)) {
        notes
            .iter()
            .copied()
            .filter(|n| state.roll.selection.contains(&(n.start, n.key)))
            .collect()
    } else {
        vec![hovered]
    };
    let mode = state.roll.lane_mode;
    let lines: Vec<Invocation> = targets
        .iter()
        .filter_map(|n| {
            let value = (mode.value(n) + notches * step).clamp(0.0, 1.0);
            mode.invocation(channel, n, value)
        })
        .collect();
    if lines.is_empty() {
        return;
    }
    let key = egui::Id::new("roll-alt-wheel");
    let now = ui.input(|i| i.time);
    let ids: Vec<(u32, u8)> = targets.iter().map(|n| (n.start, n.key)).collect();
    let last = ui.data(|d| d.get_temp::<(f64, Vec<(u32, u8)>)>(key));
    if last.is_none_or(|(t, was)| now - t > 1.0 || was != ids) {
        state.begin_gesture();
    }
    ui.data_mut(|d| d.insert_temp(key, (now, ids)));
    let batch = Invocation::new(
        musiccraft_commands::BATCH,
        lines.iter().map(ToString::to_string),
    );
    actions.push(Action::Coalesced(batch, state.gesture()));
}

#[allow(clippy::too_many_arguments)]
fn edit(
    ui: &egui::Ui,
    state: &mut UiState,
    grid: &Grid,
    response: &egui::Response,
    clip: Rect,
    notes: &[&Note],
    boxes: &[(usize, Rect)],
    channel: usize,
    actions: &mut Vec<Action>,
) {
    let lane = state.roll.lane();
    let editable = |p: Pos2| {
        p.x > clip.left() + KEYS_WIDTH
            && p.y > clip.top() + RULER_HEIGHT
            && p.y < clip.bottom() - lane
    };
    let snap = state.roll.snap;
    let meter = state.roll.meter.clone();
    let fit = state.roll.scale.filter(|_| state.roll.fit_scale);
    let at = |p: Pos2| -> (f32, Option<u8>) {
        let key = grid.row_at(p.y).map(key_of_row);
        (grid.tick_at(p.x), key.map(|k| fit_to_scale(k, fit)))
    };
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    state.roll.hover_tick = response
        .hover_pos()
        .filter(|p| editable(*p))
        .map(|p| snap.floor(grid.tick_at(p.x), &meter));

    right_erase(state, response, notes, boxes, channel, actions);
    let tool = state.roll.tool;
    // Pressing a note makes its length and velocity the new-note defaults.
    if matches!(tool, Tool::Draw | Tool::Paint) && response.is_pointer_button_down_on() {
        let pressed = ui.input(|i| {
            i.pointer
                .primary_pressed()
                .then(|| i.pointer.press_origin())
        });
        if let Some(origin) = pressed.flatten().filter(|p| editable(*p)) {
            if let Hit::Body(i) | Hit::Edge(i) = hit_test(boxes.iter().copied(), origin) {
                if let Some(n) = notes.get(i) {
                    state.roll.last_length = n.length;
                    state.roll.velocity = n.velocity;
                }
            }
        }
    }
    if tool_stroke(ui, state, response, notes, boxes, channel, &at, actions) {
        return;
    }
    if click_tool(ui, state, response, notes, boxes, channel, &at, actions) {
        return;
    }
    if response.drag_started_by(PointerButton::Primary) {
        let origin = ui
            .input(|i| i.pointer.press_origin())
            .filter(|p| editable(*p));
        if let Some(origin) = origin {
            let (tick, key) = at(origin);
            let hit = hit_test(boxes.iter().copied(), origin);
            if (ctrl || tool == Tool::Select) && hit == Hit::Empty {
                state.roll.marquee = Some((origin, origin));
            } else {
                state.roll.drag = match hit {
                    Hit::Body(i) | Hit::Edge(i) => notes.get(i).map(|n| {
                        if !state.roll.selection.contains(&(n.start, n.key)) {
                            state.roll.selection = BTreeSet::from([(n.start, n.key)]);
                        }
                        NoteDrag {
                            kind: if matches!(hit, Hit::Edge(_)) {
                                DragKind::Resize
                            } else {
                                DragKind::Move
                            },
                            original: **n,
                            grab: tick - n.start as f32,
                            start: n.start,
                            key: n.key,
                            length: n.length,
                        }
                    }),
                    Hit::Empty => key.map(|key| {
                        state.roll.selection.clear();
                        let start = snap.floor(tick, &meter);
                        NoteDrag {
                            kind: DragKind::Create,
                            original: Note {
                                props: musiccraft_project::NoteProps::default(),
                                slide: false,
                                channel: musiccraft_project::ChannelId(0),
                                start,
                                length: state.roll.last_length,
                                key,
                                velocity: state.roll.velocity,
                                pan: 0,
                            },
                            grab: tick - start as f32,
                            start,
                            key,
                            length: state.roll.last_length,
                        }
                    }),
                };
                if let Some(drag) = state.roll.drag {
                    actions.push(Action::Preview {
                        channel,
                        key: drag.key,
                        on: true,
                    });
                    state.roll.preview = Some((channel, drag.key));
                }
            }
        }
    }
    if response.dragged_by(PointerButton::Primary) {
        let pos = response.interact_pointer_pos();
        if let (Some((_, current)), Some(pos)) = (&mut state.roll.marquee, pos) {
            *current = pos;
        } else if let (Some(drag), Some(pos)) = (&mut state.roll.drag, pos) {
            let (tick, key) = at(pos);
            match drag.kind {
                DragKind::Create | DragKind::Move => {
                    drag.start = snap.floor(
                        tick - drag.grab + snap.ticks_at(0, &meter) as f32 / 2.0,
                        &meter,
                    );
                    if let Some(key) = key {
                        if key != drag.key {
                            drag.key = key;
                            if let Some((ch, old)) = state.roll.preview.replace((channel, key)) {
                                actions.push(Action::Preview {
                                    channel: ch,
                                    key: old,
                                    on: false,
                                });
                            }
                            actions.push(Action::Preview {
                                channel,
                                key,
                                on: true,
                            });
                        }
                    }
                }
                DragKind::Resize => {
                    let end = snap
                        .round(tick, &meter)
                        .max(drag.start + snap.ticks_at(0, &meter).min(TICKS_PER_STEP));
                    drag.length = end.saturating_sub(drag.start).max(1);
                }
            }
        }
    }
    if response.drag_stopped_by(PointerButton::Primary) {
        if let Some((origin, current)) = state.roll.marquee.take() {
            let area = Rect::from_two_pos(origin, current);
            if !shift {
                state.roll.selection.clear();
            }
            for (i, rect) in boxes {
                if let (true, Some(n)) = (area.intersects(*rect), notes.get(*i)) {
                    state.roll.selection.insert((n.start, n.key));
                }
            }
        } else if let Some(drag) = state.roll.drag.take() {
            if drag.kind != DragKind::Move {
                state.roll.last_length = drag.length;
            }
            let selected = selected_notes(state, notes);
            let group = selected.len() > 1;
            match drag.kind {
                DragKind::Move if group => {
                    let dt = i64::from(drag.start) - i64::from(drag.original.start);
                    let dk = i32::from(drag.key) - i32::from(drag.original.key);
                    let (dt, dk) = clamp_delta(&selected, dt, dk);
                    if let Some(invocation) = move_notes(channel, &selected, dt, dk) {
                        actions.push(Action::Run(invocation));
                        state.roll.selection =
                            selected.iter().map(|n| shifted(n, dt, dk)).collect();
                    }
                }
                DragKind::Resize if group => {
                    let dl = i64::from(drag.length) - i64::from(drag.original.length);
                    let ch = (channel + 1).to_string();
                    let resizes = selected
                        .iter()
                        .filter(|_| dl != 0)
                        .map(|n| {
                            let length =
                                u32::try_from((i64::from(n.length) + dl).max(1)).unwrap_or(1);
                            Invocation::new(
                                "note.resize",
                                [ch.clone(), ticks(n.start), n.key.to_string(), ticks(length)],
                            )
                        })
                        .collect();
                    if let Some(invocation) = batch(resizes) {
                        actions.push(Action::Run(invocation));
                    }
                }
                _ => {
                    if let Some(invocation) = finish(
                        &drag,
                        channel,
                        state.roll.velocity,
                        state.roll.slide,
                        new_props(state),
                    ) {
                        actions.push(Action::Run(invocation));
                    }
                    state.roll.selection = BTreeSet::from([(drag.start, drag.key)]);
                }
            }
        }
    }
    if !response.dragged() && !state.roll.keys_held {
        if let Some((ch, key)) = state.roll.preview.take() {
            actions.push(Action::Preview {
                channel: ch,
                key,
                on: false,
            });
        }
    }
    if response.clicked_by(PointerButton::Primary) {
        if let Some(pos) = response.interact_pointer_pos().filter(|p| editable(*p)) {
            match (hit_test(boxes.iter().copied(), pos), at(pos)) {
                (Hit::Body(i) | Hit::Edge(i), _) => {
                    if let Some(n) = notes.get(i) {
                        let id = (n.start, n.key);
                        if shift {
                            if !state.roll.selection.remove(&id) {
                                state.roll.selection.insert(id);
                            }
                        } else {
                            state.roll.selection = BTreeSet::from([id]);
                        }
                        actions.push(Action::PreviewShort {
                            channel,
                            key: n.key,
                        });
                    }
                }
                (Hit::Empty, _) if tool == Tool::Select => state.roll.selection.clear(),
                (Hit::Empty, (tick, Some(key))) => {
                    let start = snap.floor(tick, &meter);
                    let length = ticks(state.roll.last_length);
                    let velocity = state.roll.velocity.to_string();
                    let ch = (channel + 1).to_string();
                    let invocation = match state.roll.chord {
                        Some(quality) => Invocation::new(
                            "note.add_chord",
                            [
                                ch,
                                ticks(start),
                                length,
                                key.to_string(),
                                quality.to_owned(),
                                velocity,
                            ],
                        ),
                        None => {
                            let _ = (ch, length, velocity);
                            create(
                                channel,
                                &Note {
                                    channel: musiccraft_project::ChannelId(0),
                                    start,
                                    length: state.roll.last_length,
                                    key,
                                    velocity: state.roll.velocity,
                                    pan: 0,
                                    slide: state.roll.slide,
                                    props: new_props(state),
                                },
                            )
                        }
                    };
                    actions.push(Action::Run(invocation));
                    actions.push(Action::PreviewShort { channel, key });
                    state.roll.selection.clear();
                }
                _ => {}
            }
        }
    }
    if response.double_clicked() && matches!(state.roll.tool, Tool::Draw | Tool::Select) {
        let pos = response.interact_pointer_pos().filter(|p| editable(*p));
        if let Some(Hit::Body(i) | Hit::Edge(i)) = pos.map(|p| hit_test(boxes.iter().copied(), p))
            && let Some(note) = notes.get(i)
        {
            state.roll.props_note = Some((note.start, note.key));
        }
    }
    if let Some(pos) = response.hover_pos().filter(|p| editable(*p)) {
        alt_wheel(ui, state, pos, (notes, boxes), channel, actions);
        let moving = state.roll.drag.is_some_and(|d| d.kind == DragKind::Move);
        let cursor = match (tool, hit_test(boxes.iter().copied(), pos)) {
            (Tool::Delete | Tool::Mute | Tool::Slice | Tool::Play | Tool::Zoom, _) => {
                egui::CursorIcon::Default
            }
            (_, Hit::Edge(_)) => egui::CursorIcon::ResizeHorizontal,
            _ if moving => egui::CursorIcon::Grabbing,
            (_, Hit::Body(_)) => egui::CursorIcon::Move,
            (Tool::Select, Hit::Empty) => egui::CursorIcon::Cell,
            (_, Hit::Empty) if ctrl => egui::CursorIcon::Cell,
            (_, Hit::Empty) => egui::CursorIcon::Crosshair,
        };
        ui.ctx().set_cursor_icon(cursor);
    }
}

/// Editing keys, active while the pointer is over the piano roll.
/// The piano roll's selection and clipboard edits, run from keys and from
/// the window menu's Edit and Select submenus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditOp {
    SelectAll,
    SelectNone,
    InvertSelection,
    Delete,
    Cut,
    Copy,
    Paste,
    Duplicate,
    /// Move the selection by snap steps and semitones.
    Move(i64, i8),
    /// Give the selected notes the colour group of new notes.
    ApplyColour,
}

fn keys(ctx: &egui::Context, state: &mut UiState, project: &Project, actions: &mut Vec<Action>) {
    use egui::{Key, Modifiers};
    let pressed = |modifiers: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(modifiers, key));
    let mut ops = Vec::new();
    if pressed(Modifiers::COMMAND, Key::A) {
        ops.push(EditOp::SelectAll);
    }
    // Esc clears a selection first; with none, it closes the window.
    let any = !state.roll.selection.is_empty() || state.roll.time_selection.is_some();
    if any && pressed(Modifiers::NONE, Key::Escape) {
        ops.push(EditOp::SelectNone);
    }
    if pressed(Modifiers::NONE, Key::Delete) || pressed(Modifiers::NONE, Key::Backspace) {
        ops.push(EditOp::Delete);
    }
    use crate::ClipboardKey;
    for (key, op) in [
        (ClipboardKey::Copy, EditOp::Copy),
        (ClipboardKey::Cut, EditOp::Cut),
        (ClipboardKey::Paste, EditOp::Paste),
    ] {
        if crate::clipboard_key(ctx, key) {
            ops.push(op);
        }
    }
    if pressed(Modifiers::COMMAND, Key::B) {
        ops.push(EditOp::Duplicate);
    }
    if pressed(Modifiers::ALT, Key::C) {
        ops.push(EditOp::ApplyColour);
    }
    if pressed(Modifiers::SHIFT, Key::Enter) {
        state.roll.lane_shown = !state.roll.lane_shown;
    }
    if ops.contains(&EditOp::Copy) || ops.contains(&EditOp::Cut) {
        crate::copied(ctx, "notes");
    }
    let moves = [
        (Modifiers::SHIFT, Key::ArrowUp, 0, 1),
        (Modifiers::SHIFT, Key::ArrowDown, 0, -1),
        (Modifiers::COMMAND, Key::ArrowUp, 0, 12),
        (Modifiers::COMMAND, Key::ArrowDown, 0, -12),
        (Modifiers::SHIFT, Key::ArrowRight, 1, 0),
        (Modifiers::SHIFT, Key::ArrowLeft, -1, 0),
    ];
    for (modifiers, key, steps, keys) in moves {
        if pressed(modifiers, key) {
            ops.push(EditOp::Move(steps, keys));
        }
    }
    for op in ops {
        edit_op(state, project, op, actions);
    }
    let zooms = [
        (Modifiers::NONE, Key::PageUp, Zoom::By(1.5)),
        (Modifiers::NONE, Key::PageDown, Zoom::By(1.0 / 1.5)),
        (Modifiers::SHIFT, Key::Num1, Zoom::Level(ZOOM_LEVELS[0])),
        (Modifiers::SHIFT, Key::Num2, Zoom::Level(ZOOM_LEVELS[1])),
        (Modifiers::SHIFT, Key::Num3, Zoom::Level(ZOOM_LEVELS[2])),
        (Modifiers::SHIFT, Key::Num4, Zoom::Pattern),
        (Modifiers::SHIFT, Key::Num5, Zoom::Selection),
    ];
    for (modifiers, key, op) in zooms {
        if pressed(modifiers, key) {
            zoom(state, project, op);
        }
    }
}

/// Runs one selection or clipboard edit on the selected channel's notes.
fn edit_op(state: &mut UiState, project: &Project, op: EditOp, actions: &mut Vec<Action>) {
    let channel = state.selected_channel;
    let Some(id) = project.channels.get(channel).map(|c| c.id) else {
        return;
    };
    let notes: Vec<Note> = project.current_pattern().notes_of(id).copied().collect();
    let selected: Vec<Note> = notes
        .iter()
        .filter(|n| state.roll.selection.contains(&(n.start, n.key)))
        .copied()
        .collect();
    let snap = i64::from(state.roll.snap.ticks_at(0, &state.roll.meter));
    match op {
        EditOp::SelectAll => {
            state.roll.selection = notes.iter().map(|n| (n.start, n.key)).collect();
        }
        EditOp::SelectNone => {
            state.roll.selection.clear();
            state.roll.time_selection = None;
        }
        EditOp::InvertSelection => {
            state.roll.selection = notes
                .iter()
                .map(|n| (n.start, n.key))
                .filter(|k| !state.roll.selection.contains(k))
                .collect();
        }
        EditOp::Delete => {
            actions.extend(remove_notes(channel, &selected).map(Action::Run));
            state.roll.selection.clear();
        }
        EditOp::Copy | EditOp::Cut if !selected.is_empty() => {
            let first = selected.iter().map(|n| n.start).min().unwrap_or(0);
            state.roll.clipboard = selected
                .iter()
                .map(|n| Note {
                    start: n.start - first,
                    ..*n
                })
                .collect();
            if op == EditOp::Cut {
                actions.extend(remove_notes(channel, &selected).map(Action::Run));
                state.roll.selection.clear();
            }
        }
        EditOp::Paste if !state.roll.clipboard.is_empty() => {
            let at = state.roll.hover_tick.unwrap_or(0);
            actions.extend(add_notes(channel, &state.roll.clipboard, at).map(Action::Run));
            state.roll.selection = state
                .roll
                .clipboard
                .iter()
                .map(|n| (n.start + at, n.key))
                .collect();
        }
        EditOp::Duplicate if !selected.is_empty() => {
            let first = selected.iter().map(|n| n.start).min().unwrap_or(0);
            let end = selected
                .iter()
                .map(|n| n.start + n.length)
                .max()
                .unwrap_or(first);
            let span = (end - first).div_ceil(snap.max(1) as u32) * snap.max(1) as u32;
            actions.extend(add_notes(channel, &selected, span).map(Action::Run));
            state.roll.selection = selected.iter().map(|n| (n.start + span, n.key)).collect();
        }
        EditOp::Move(steps, keys) if !selected.is_empty() => {
            let (dt, dk) = clamp_delta(&selected, steps * snap, i32::from(keys));
            if let Some(invocation) = move_notes(channel, &selected, dt, dk) {
                actions.push(Action::Run(invocation));
                state.roll.selection = selected.iter().map(|n| shifted(n, dt, dk)).collect();
            }
        }
        EditOp::ApplyColour => {
            let recolour: Vec<Note> = selected
                .into_iter()
                .filter(|n| n.props.color != state.roll.color)
                .collect();
            let group = (state.roll.color + 1).to_string();
            actions.extend(set_notes_prop(channel, &recolour, "color", &group).map(Action::Run));
        }
        _ => {}
    }
}

/// The note whose stem is under `x` (the latest-starting note covering it).
fn note_at_x<'a>(grid: &Grid, notes: &[&'a Note], x: f32) -> Option<&'a Note> {
    let tick = grid.tick_at(x);
    notes
        .iter()
        .filter(|n| (n.start as f32) <= tick && tick < (n.start + n.length) as f32)
        .max_by_key(|n| (n.start, n.key))
        .copied()
}

/// Lane position 0..1 of a pointer height inside the lane.
fn lane_unit(lane: Rect, y: f32) -> f32 {
    ((lane.bottom() - 4.0 - y) / (lane.height() - 8.0)).clamp(0.0, 1.0)
}

/// What the bottom lane edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum LaneMode {
    #[default]
    Velocity,
    Pan,
    Release,
    Fine,
    ModX,
    ModY,
}

impl LaneMode {
    pub(crate) const ALL: [LaneMode; 6] = [
        LaneMode::Velocity,
        LaneMode::Pan,
        LaneMode::Release,
        LaneMode::Fine,
        LaneMode::ModX,
        LaneMode::ModY,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Velocity => "Velocity",
            Self::Pan => "Pan",
            Self::Release => "Release",
            Self::Fine => "Fine pitch",
            Self::ModX => "Mod A",
            Self::ModY => "Mod B",
        }
    }

    /// Whether the lane's neutral value is in the middle.
    pub(crate) fn centred(self) -> bool {
        !matches!(self, Self::Velocity)
    }

    pub(crate) fn value(self, note: &Note) -> f32 {
        let p = note.props;
        match self {
            Self::Velocity => f32::from(note.velocity.max(1) - 1) / 126.0,
            Self::Pan => (f32::from(note.pan) + 100.0) / 200.0,
            Self::Release => f32::from(p.release) / 127.0,
            Self::Fine => (f32::from(p.fine) + 100.0) / 200.0,
            Self::ModX => f32::from(p.mod_x) / 127.0,
            Self::ModY => f32::from(p.mod_y) / 127.0,
        }
    }

    /// The lane's default as a lane position: velocity 100, the rest neutral.
    pub(crate) fn default_unit(self) -> f32 {
        match self {
            Self::Velocity => 99.0 / 126.0,
            Self::Pan | Self::Fine => 0.5,
            Self::Release | Self::ModX | Self::ModY => 64.0 / 127.0,
        }
    }

    /// The note's value as text, for knob captions.
    pub(crate) fn shown(self, note: &Note) -> String {
        let p = note.props;
        match self {
            Self::Velocity => note.velocity.to_string(),
            Self::Pan => musiccraft_editors::format::pan(f32::from(note.pan) / 100.0),
            Self::Release => p.release.to_string(),
            Self::Fine => format!("{:+} cents", p.fine),
            Self::ModX => p.mod_x.to_string(),
            Self::ModY => p.mod_y.to_string(),
        }
    }

    pub(crate) fn invocation(self, channel: usize, note: &Note, t: f32) -> Option<Invocation> {
        // Snap to the neutral value near the middle of centred lanes.
        let snap = |v: i32, mid: i32, near: i32| if (v - mid).abs() <= near { mid } else { v };
        let p = note.props;
        let (name, prop, value, current) = match self {
            Self::Velocity => {
                let v = (1.0 + t * 126.0).round() as i32;
                ("note.set_velocity", None, v, i32::from(note.velocity))
            }
            Self::Pan => {
                let v = snap((t * 200.0 - 100.0).round() as i32, 0, 3);
                ("note.set_pan", None, v, i32::from(note.pan))
            }
            Self::Release => {
                let v = snap((t * 127.0).round() as i32, 64, 2);
                ("note.set_prop", Some("release"), v, i32::from(p.release))
            }
            Self::Fine => {
                let v = snap((t * 200.0 - 100.0).round() as i32, 0, 3);
                ("note.set_prop", Some("fine"), v, i32::from(p.fine))
            }
            Self::ModX => {
                let v = snap((t * 127.0).round() as i32, 64, 2);
                ("note.set_prop", Some("mod_x"), v, i32::from(p.mod_x))
            }
            Self::ModY => {
                let v = snap((t * 127.0).round() as i32, 64, 2);
                ("note.set_prop", Some("mod_y"), v, i32::from(p.mod_y))
            }
        };
        (value != current).then(|| {
            let mut args = vec![
                (channel + 1).to_string(),
                format!("{}t", note.start),
                note.key.to_string(),
            ];
            args.extend(prop.map(str::to_owned));
            args.push(value.to_string());
            Invocation::new(name, args)
        })
    }
}

/// Where the playhead is inside the shown pattern, in pattern ticks: the
/// loop position in pattern mode; in song mode, inside a clip of this
/// pattern under the playhead (none elsewhere).
pub(crate) fn pattern_position(project: &Project, position: f64) -> Option<u32> {
    let pattern = project.current_pattern();
    let period = pattern.length_ticks().max(1);
    let tick = position.max(0.0) as u32;
    match project.play_mode {
        PlayMode::Pattern => Some(tick % period),
        PlayMode::Song => project
            .playlist
            .clips
            .iter()
            .find(|c| c.pattern() == Some(pattern.id) && c.start <= tick && tick < c.end())
            .map(|c| (tick - c.start + c.offset) % period),
    }
}

fn velocity_lane(
    ui: &mut egui::Ui,
    state: &mut UiState,
    (grid, clip): (&Grid, Rect),
    (notes, ghosts): (&[&Note], &[&Note]),
    (channel, color): (usize, Color32),
    actions: &mut Vec<Action>,
) {
    splitter(ui, state, clip);
    if !state.roll.lane_shown {
        return;
    }
    let mode = state.roll.lane_mode;
    let lane = Rect::from_min_max(
        pos2(clip.left() + KEYS_WIDTH, clip.bottom() - state.roll.lane()),
        clip.right_bottom(),
    );
    let response = ui.interact(lane, ui.id().with("velocity-lane"), Sense::click_and_drag());
    let strip = Rect::from_min_max(pos2(clip.left(), lane.top()), lane.right_bottom());
    let painter = ui.painter_at(strip);
    painter.rect_filled(strip, 0.0, theme::PANEL_DARK);
    lane_grid(&painter, grid, lane, &state.roll.meter);
    painter.line_segment(
        [strip.left_top(), strip.right_top()],
        Stroke::new(1.0, theme::GRID_BAR),
    );
    // The target's name, faint, like a watermark.
    painter.text(
        lane.left_top() + vec2(6.0, 4.0),
        Align2::LEFT_TOP,
        mode.label(),
        FontId::proportional(theme::FONT_DISPLAY),
        theme::WATERMARK,
    );
    // The lane's target: a menu of what it edits.
    let label_rect = Rect::from_min_size(
        pos2(strip.left() + 2.0, lane.top() + 4.0),
        vec2(KEYS_WIDTH - 4.0, 18.0),
    );
    ui.put(label_rect, |ui: &mut egui::Ui| {
        egui::ComboBox::from_id_salt("lane-mode")
            .width(KEYS_WIDTH - 4.0)
            .selected_text(RichText::new(mode.label()).small().color(theme::ACCENT))
            .show_ui(ui, |ui| {
                for m in LaneMode::ALL {
                    ui.selectable_value(&mut state.roll.lane_mode, m, m.label());
                }
            })
            .response
            .hint("What the lane edits: velocity, pan, release, fine pitch, Mod A or Mod B")
    });
    if mode.centred() {
        painter.line_segment(
            [
                pos2(lane.left(), lane.center().y),
                pos2(lane.right(), lane.center().y),
            ],
            Stroke::new(1.0, theme::GRID_BAR),
        );
    }
    let alt = ui.input(|i| i.modifiers.alt);
    if alt && response.clicked() {
        // Alt+click resets the stem under the pointer (the selection when
        // that note is selected) to the lane's default, in one step.
        if let Some(note) = response
            .interact_pointer_pos()
            .and_then(|pos| note_at_x(grid, notes, pos.x))
        {
            let selection = &state.roll.selection;
            let picked = selection.contains(&(note.start, note.key));
            let resets = notes
                .iter()
                .filter(|n| {
                    if picked {
                        selection.contains(&(n.start, n.key))
                    } else {
                        n.start == note.start && n.key == note.key
                    }
                })
                .filter_map(|n| mode.invocation(channel, n, mode.default_unit()))
                .collect();
            actions.extend(batch(resets).map(Action::Run));
        }
    } else if response.drag_started() || response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            state.roll.lane_drag = note_at_x(grid, notes, pos.x).copied();
            state.begin_gesture();
        }
    }
    let mut shown: Option<(Note, f32)> = None;
    if let (Some(note), Some(pos)) = (state.roll.lane_drag, response.interact_pointer_pos()) {
        if response.dragged() || response.clicked() {
            let t = lane_unit(lane, pos.y);
            shown = Some((note, t));
            if let Some(invocation) = mode.invocation(channel, &note, t) {
                actions.push(Action::Coalesced(invocation, state.gesture()));
            }
        }
    }
    if response.drag_stopped() || (!response.dragged() && !response.clicked()) {
        state.roll.lane_drag = None;
    }

    let stem = Stem {
        lane,
        centred: mode.centred(),
    };
    for note in ghosts {
        stem.draw(&painter, grid, note, mode.value(note), theme::GHOST_NOTE);
    }
    for note in notes {
        let t = match shown {
            Some((n, t)) if n.start == note.start && n.key == note.key => t,
            _ => mode.value(note),
        };
        let tone = if state.roll.selection.contains(&(note.start, note.key)) {
            theme::NOTE_SELECTED
        } else if note.props.color > 0 {
            theme::note_color(note.props.color)
        } else {
            color
        };
        stem.draw(&painter, grid, note, t, tone);
    }
}

/// How the lane draws one note's value.
struct Stem {
    lane: Rect,
    centred: bool,
}

impl Stem {
    /// A bar from the baseline to the value with a round head, and a thin
    /// tail running right for the note's length.
    fn draw(&self, painter: &egui::Painter, grid: &Grid, note: &Note, t: f32, color: Color32) {
        let lane = self.lane;
        let x = grid.x(note.start as f32) + 2.0;
        let end = grid.x((note.start + note.length) as f32) - 1.0;
        if end < lane.left() || x > lane.right() {
            return;
        }
        let base = if self.centred {
            lane.center().y
        } else {
            lane.bottom()
        };
        let tip = lane.bottom() - 4.0 - t * (lane.height() - 8.0);
        painter.line_segment([pos2(x, base), pos2(x, tip)], Stroke::new(2.0, color));
        if end > x + 3.0 {
            painter.line_segment([pos2(x, tip), pos2(end, tip)], Stroke::new(1.0, color));
        }
        painter.circle_filled(pos2(x, tip), 3.0, color);
    }
}

/// The grid's step, beat and bar lines through the lane, a step quieter.
fn lane_grid(painter: &egui::Painter, grid: &Grid, lane: Rect, meter: &Meter) {
    let first = grid.tick_at(lane.left()).max(0.0) as u32 / TICKS_PER_STEP * TICKS_PER_STEP;
    let last = grid.tick_at(lane.right()).max(0.0) as u32;
    let sig = meter.first();
    for tick in (first..=last).step_by(TICKS_PER_STEP as usize) {
        let color = if tick % sig.bar_ticks() == 0 {
            theme::GRID_BEAT
        } else if tick % sig.beat_ticks() == 0 {
            theme::GRID_LINE
        } else {
            theme::at_strength(theme::GRID_LINE, 110)
        };
        let x = grid.x(tick as f32);
        painter.line_segment(
            [pos2(x, lane.top()), pos2(x, lane.bottom())],
            Stroke::new(1.0, color),
        );
    }
}

fn keyboard(
    ui: &mut egui::Ui,
    state: &mut UiState,
    (grid, clip): (&Grid, Rect),
    notes: &[&Note],
    channel: usize,
    actions: &mut Vec<Action>,
) {
    let keys_rect = Rect::from_min_max(
        pos2(clip.left(), clip.top() + RULER_HEIGHT),
        pos2(clip.left() + KEYS_WIDTH, clip.bottom() - state.roll.lane()),
    );
    let response = ui
        .interact(
            keys_rect,
            ui.id().with("roll-keys"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let painter = ui.painter_at(keys_rect);
    let first = grid.row_at(keys_rect.top()).unwrap_or(0);
    let last = grid.row_at(keys_rect.bottom() - 1.0).unwrap_or(KEYS - 1);
    let held = state.roll.preview.map(|(_, k)| k);
    let hovered = response
        .hover_pos()
        .and_then(|p| grid.row_at(p.y))
        .map(key_of_row);
    let response = match hovered {
        Some(key) => response.hint(format!(
            "{} · click to hear · Ctrl+click selects its notes, Ctrl+drag a range · middle-drag zooms · right-click for labels",
            key_name(key)
        )),
        None => response,
    };
    let ivory = theme::KEY_WHITE;
    let ebony = theme::KEY_BLACK;
    // White keys run the full width behind the black ones, like a piano.
    painter.rect_filled(keys_rect, 0.0, ivory);
    for row in first..=last {
        let key = key_of_row(row);
        let y = grid.y(row);
        let r = Rect::from_min_max(
            pos2(keys_rect.left(), y),
            pos2(keys_rect.right(), y + grid.row_height),
        );
        if is_black(key) {
            let black = Rect::from_min_max(r.min, pos2(r.left() + r.width() * 0.62, r.bottom()));
            let fill = if held == Some(key) {
                theme::ACCENT
            } else if hovered == Some(key) {
                theme::mix(ebony, ivory, 0.18)
            } else {
                ebony
            };
            painter.rect_filled(
                black,
                egui::CornerRadius {
                    nw: 0,
                    sw: 0,
                    ne: 2,
                    se: 2,
                },
                fill,
            );
        } else {
            if held == Some(key) {
                painter.rect_filled(r, 0.0, theme::ACCENT);
            } else if hovered == Some(key) {
                painter.rect_filled(r, 0.0, theme::shade(40));
            }
            // A seam between neighbouring white keys (E/F and B/C).
            if matches!(key % 12, 4 | 11) {
                painter.line_segment(
                    [r.left_top(), r.right_top()],
                    Stroke::new(1.0, theme::KEY_SEAM),
                );
            }
        }
        if state
            .roll
            .key_labels
            .shows(key, grid.row_height, state.roll.scale)
        {
            painter.text(
                r.right_center() - vec2(3.0, 0.0),
                Align2::RIGHT_CENTER,
                key_name(key),
                FontId::proportional(theme::FONT_CAPTION),
                theme::BACKGROUND,
            );
        }
    }
    middle_zoom(ui, state, (grid, clip), keys_rect);
    response.context_menu(|ui| {
        ui.menu_button("Keyboard labels", |ui| key_labels_list(ui, state));
    });
    // Ctrl+click selects every note of the key, Ctrl+drag every note of the
    // keys it crosses; Shift adds to the selection. (A preview already
    // sounding still gets its note-off below.)
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    if ctrl && state.roll.preview.is_none() {
        let from = ui
            .input(|i| i.pointer.press_origin())
            .and_then(|p| grid.row_at(p.y))
            .map(key_of_row);
        let to = response
            .interact_pointer_pos()
            .and_then(|p| grid.row_at(p.y))
            .map(key_of_row);
        if let (true, Some(a), Some(b)) = (response.dragged_by(PointerButton::Primary), from, to) {
            let range = a.min(b)..=a.max(b);
            if !shift {
                state.roll.selection.clear();
            }
            state.roll.selection.extend(
                notes
                    .iter()
                    .filter(|n| range.contains(&n.key))
                    .map(|n| (n.start, n.key)),
            );
            return;
        }
        if let Some(key) = response
            .clicked()
            .then(|| response.interact_pointer_pos())
            .flatten()
            .and_then(|p| grid.row_at(p.y))
            .map(key_of_row)
        {
            if !shift {
                state.roll.selection.clear();
            }
            state.roll.selection.extend(
                notes
                    .iter()
                    .filter(|n| n.key == key)
                    .map(|n| (n.start, n.key)),
            );
        }
        return;
    }
    let pressed_key = response
        .interact_pointer_pos()
        .filter(|_| response.is_pointer_button_down_on())
        .and_then(|p| grid.row_at(p.y))
        .map(key_of_row);
    match (pressed_key, state.roll.preview) {
        (Some(key), None) => {
            actions.push(Action::Preview {
                channel,
                key,
                on: true,
            });
            state.roll.preview = Some((channel, key));
            state.roll.keys_held = true;
        }
        (Some(key), Some((ch, old))) if old != key && state.roll.drag.is_none() => {
            actions.push(Action::Preview {
                channel: ch,
                key: old,
                on: false,
            });
            actions.push(Action::Preview {
                channel,
                key,
                on: true,
            });
            state.roll.preview = Some((channel, key));
        }
        (None, Some((ch, old))) if state.roll.drag.is_none() => {
            actions.push(Action::Preview {
                channel: ch,
                key: old,
                on: false,
            });
            state.roll.preview = None;
            state.roll.keys_held = false;
        }
        _ => {}
    }
}

/// Middle-drag on the keyboard zooms the key rows: up to grow them, down to
/// shrink them, keeping the key first pressed under the pointer.
fn middle_zoom(ui: &egui::Ui, state: &mut UiState, (grid, clip): (&Grid, Rect), keys: Rect) {
    let (down, origin, dy) = ui.input(|i| {
        (
            i.pointer.button_down(PointerButton::Middle),
            i.pointer.press_origin(),
            i.pointer.delta().y,
        )
    });
    let Some(origin) = origin.filter(|o| down && keys.contains(*o)) else {
        return;
    };
    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    let height = state.roll.key_height;
    let zoomed = (height * (-dy / 100.0).exp()).clamp(KEY_HEIGHT_RANGE.0, KEY_HEIGHT_RANGE.1);
    if dy == 0.0 || zoomed == height {
        return;
    }
    // The pressed point's place in the rows stays at the same height on screen.
    let rows = (origin.y - grid.origin.y) / height;
    let top = origin.y - RULER_HEIGHT - rows * zoomed;
    let left = grid.origin.x - KEYS_WIDTH;
    state.roll.key_height = zoomed;
    state.roll.jump = Some(vec2(clip.left() - left, (clip.top() - top).max(0.0)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use musiccraft_project::ChannelId;

    fn drag(kind: DragKind, start: u32, key: u8, length: u32) -> NoteDrag {
        NoteDrag {
            kind,
            original: Note {
                props: musiccraft_project::NoteProps::default(),
                slide: false,
                channel: ChannelId(1),
                start: 24,
                length: 24,
                key: 60,
                velocity: 100,
                pan: 0,
            },
            grab: 0.0,
            start,
            key,
            length,
        }
    }

    #[test]
    fn gestures_become_single_commands() {
        let create = finish(
            &drag(DragKind::Create, 48, 62, 12),
            0,
            90,
            false,
            musiccraft_project::NoteProps::default(),
        )
        .unwrap();
        assert_eq!(create.to_string(), "note.add 1 48t 12t 62 90");
        let moved = finish(
            &drag(DragKind::Move, 96, 64, 24),
            2,
            100,
            false,
            musiccraft_project::NoteProps::default(),
        )
        .unwrap();
        assert_eq!(moved.to_string(), "note.move 3 24t 60 96t 64");
        let resized = finish(
            &drag(DragKind::Resize, 24, 60, 72),
            0,
            100,
            false,
            musiccraft_project::NoteProps::default(),
        )
        .unwrap();
        assert_eq!(resized.to_string(), "note.resize 1 24t 60 72t");
        assert!(
            finish(
                &drag(DragKind::Move, 24, 60, 24),
                0,
                100,
                false,
                musiccraft_project::NoteProps::default()
            )
            .is_none(),
            "no-op move"
        );
        assert!(
            finish(
                &drag(DragKind::Resize, 24, 60, 24),
                0,
                100,
                false,
                musiccraft_project::NoteProps::default()
            )
            .is_none(),
            "no-op resize"
        );
    }

    fn n(start: u32, key: u8) -> Note {
        Note {
            props: musiccraft_project::NoteProps::default(),
            slide: false,
            channel: ChannelId(1),
            start,
            length: 24,
            key,
            velocity: 100,
            pan: 0,
        }
    }

    #[test]
    fn group_moves_are_ordered_and_clamped() {
        // Moving right: the later note moves first so it is never overwritten.
        let right = move_notes(0, &[n(0, 60), n(24, 60)], 24, 0)
            .unwrap()
            .to_string();
        assert_eq!(
            right,
            r#"edit.batch "note.move 1 24t 60 48t 60" "note.move 1 0t 60 24t 60""#
        );
        let left = move_notes(0, &[n(24, 60), n(48, 60)], -24, 0)
            .unwrap()
            .to_string();
        assert!(
            left.starts_with(r#"edit.batch "note.move 1 24t 60 0t 60""#),
            "{left}"
        );
        // Cannot move before the start or past the top key.
        assert_eq!(clamp_delta(&[n(12, 120)], -100, 20), (-12, 7));
        assert!(
            move_notes(0, &[n(0, 60)], -5, 0).is_none(),
            "already at the start"
        );
        assert_eq!(
            move_notes(0, &[n(0, 60)], 0, 2).unwrap().to_string(),
            "note.move 1 0t 60 0t 62"
        );
    }

    #[test]
    fn lane_modes_map_to_commands() {
        let note = n(0, 60);
        assert_eq!(
            LaneMode::Velocity
                .invocation(0, &note, 1.0)
                .unwrap()
                .to_string(),
            "note.set_velocity 1 0t 60 127"
        );
        assert!(
            LaneMode::Velocity
                .invocation(0, &note, LaneMode::Velocity.value(&note))
                .is_none()
        );
        assert_eq!(
            LaneMode::Pan.invocation(0, &note, 0.0).unwrap().to_string(),
            "note.set_pan 1 0t 60 -100"
        );
        assert!(
            LaneMode::Pan.invocation(0, &note, 0.51).is_none(),
            "snaps to centre"
        );
        let lane = Rect::from_min_max(pos2(0.0, 100.0), pos2(500.0, 164.0));
        assert!((lane_unit(lane, 132.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn rows_map_to_keys_top_down() {
        assert_eq!(key_of_row(0), 127);
        assert_eq!(key_of_row(127), 0);
        assert_eq!(row_of_key(60), 67);
        assert!(is_black(61) && !is_black(60));
    }

    #[test]
    fn a_scale_replaces_the_key_shading() {
        // No scale: black keys dark, white keys light.
        assert_eq!(row_fill(61, None), theme::BLACK_KEY_ROW);
        assert_eq!(row_fill(62, None), theme::WHITE_KEY_ROW);
        // D major: C# (a black key) is in the scale, C (white) is not.
        let d_major = Some((2, 0));
        assert_ne!(row_fill(61, d_major), theme::BLACK_KEY_ROW);
        assert_eq!(row_fill(60, d_major), theme::BLACK_KEY_ROW);
        // The root row differs from other in-scale rows.
        assert_ne!(row_fill(62, d_major), row_fill(64, d_major));
    }

    #[test]
    fn fitting_moves_keys_onto_the_scale() {
        let c_major = Some((0, 0));
        assert_eq!(fit_to_scale(61, c_major), 60, "C# goes down to C");
        assert_eq!(fit_to_scale(64, c_major), 64);
        assert_eq!(fit_to_scale(61, None), 61);
    }

    #[test]
    fn select_invert_and_menu_moves_use_the_shared_edits() {
        let mut session = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        for line in [
            "channel.add_synth",
            "note.add 1 0t 24t 60 100",
            "note.add 1 48t 24t 64 100",
        ] {
            session.run_line(line).unwrap();
        }
        let project = session.project().clone();
        let mut state = UiState::default();
        state.roll.selection.insert((0, 60));
        let mut actions = Vec::new();
        edit_op(&mut state, &project, EditOp::InvertSelection, &mut actions);
        assert!(actions.is_empty());
        assert_eq!(
            state.roll.selection.iter().copied().collect::<Vec<_>>(),
            [(48, 64)]
        );
        edit_op(&mut state, &project, EditOp::Move(0, 1), &mut actions);
        let ran: Vec<String> = actions
            .iter()
            .filter_map(|a| match a {
                Action::Run(i) => Some(i.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(ran, ["note.move 1 48t 64 48t 65"]);
        assert_eq!(
            state.roll.selection.iter().copied().collect::<Vec<_>>(),
            [(48, 65)]
        );
    }

    #[test]
    fn note_props_are_set_on_the_given_notes_as_one_undo_step() {
        let mut session = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        for line in [
            "channel.add_synth",
            "note.add 1 0t 24t 60 100",
            "note.add 1 48t 24t 64 100",
        ] {
            session.run_line(line).unwrap();
        }
        let notes_of = |session: &musiccraft_commands::Session| -> Vec<Note> {
            let p = session.project();
            let id = p.channels[0].id;
            p.current_pattern().notes_of(id).copied().collect()
        };
        let notes = notes_of(&session);
        assert!(set_notes_prop(0, &[], "muted", "on").is_none());
        let mute = set_notes_prop(0, &notes, "muted", "on").unwrap();
        session.run(&mute).unwrap();
        assert!(notes_of(&session).iter().all(|n| n.props.muted));
        let recolour = set_notes_prop(0, &notes[..1], "color", "3").unwrap();
        session.run(&recolour).unwrap();
        let colours: Vec<u8> = notes_of(&session).iter().map(|n| n.props.color).collect();
        assert_eq!(colours, [2, 0]);
        session.undo().unwrap();
        session.undo().unwrap();
        assert!(notes_of(&session).iter().all(|n| !n.props.muted));
    }

    #[test]
    fn applying_the_colour_recolours_only_selected_notes_of_another_group() {
        let mut session = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        for line in [
            "channel.add_synth",
            "note.add 1 0t 24t 60 100",
            "note.add 1 48t 24t 64 100",
            "note.add 1 96t 24t 67 100",
            "note.set_prop 1 48t 64 color 3",
        ] {
            session.run_line(line).unwrap();
        }
        let project = session.project().clone();
        let mut state = UiState::default();
        state.roll.color = 2;
        state.roll.selection.extend([(0, 60), (48, 64)]);
        let mut actions = Vec::new();
        edit_op(&mut state, &project, EditOp::ApplyColour, &mut actions);
        let ran: Vec<String> = actions
            .iter()
            .filter_map(|a| match a {
                Action::Run(i) => Some(i.to_string()),
                _ => None,
            })
            .collect();
        assert_eq!(ran, ["note.set_prop 1 0t 60 color 3"]);
        state.roll.selection.clear();
        actions.clear();
        edit_op(&mut state, &project, EditOp::ApplyColour, &mut actions);
        assert!(actions.is_empty());
    }

    #[test]
    fn keyboard_labels_pick_their_keys_and_need_room() {
        let c_major = Some((0, 0));
        let tall = LABEL_ROWS;
        assert!(
            KeyLabels::C.shows(60, 6.0, None),
            "C keys are labelled at any height"
        );
        assert!(!KeyLabels::C.shows(62, tall, None));
        assert!(KeyLabels::White.shows(62, tall, None));
        assert!(!KeyLabels::White.shows(61, tall, None));
        assert!(
            !KeyLabels::White.shows(62, tall - 1.0, None),
            "short rows stay bare"
        );
        assert!(KeyLabels::All.shows(61, tall, None));
        assert!(KeyLabels::Scale.shows(64, tall, c_major));
        assert!(!KeyLabels::Scale.shows(61, tall, c_major));
        assert!(
            !KeyLabels::Scale.shows(64, tall, None),
            "no scale, no labels"
        );
        assert!(!KeyLabels::None.shows(60, tall, None));
    }
}
