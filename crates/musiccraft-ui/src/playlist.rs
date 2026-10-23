//! The playlist: arrange patterns as clips on track rows.

use crate::hint::Hint;
use std::collections::BTreeSet;

use egui::{
    Align2, Color32, FontId, PointerButton, Pos2, Rect, RichText, Sense, Stroke, Vec2, pos2, vec2,
};
use musiccraft_commands::Invocation;
use musiccraft_project::params::PPQ;
use musiccraft_project::{
    AutomationId, Clip, ClipSource, MarkerAction, Meter, PLAYLIST_TRACKS, PlayMode, Project,
    TRACK_HEIGHT_PERCENT,
};

use crate::dialogs::Dialog;
use crate::geometry::{Grid, Hit, Snap, hit_test};
use crate::icons::{self, Icon};
use crate::piano_roll::Tool;
use crate::signature;
use crate::{Action, Frame, UiState, theme, widgets};

const HEADER_WIDTH: f32 = 124.0;
const RULER_HEIGHT: f32 = 20.0;
/// The clip list's widths: narrow, normal and wide.
const LIST_WIDTHS: [(&str, f32); 3] = [("Narrow", 100.0), ("Normal", 130.0), ("Wide", 180.0)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DragKind {
    Move,
    Resize,
    /// The left edge: the start moves and the end stays.
    Trim,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ClipDrag {
    kind: DragKind,
    original: Clip,
    grab: f32,
    track: u32,
    start: u32,
    length: u32,
}

/// Ticks the zoom (`px_per_bar`) measures: a 4/4 bar, whatever the song's
/// signature.
const ZOOM_TICKS: u32 = 4 * PPQ;
/// Horizontal zoom range, in points per 4/4 bar.
const BAR_WIDTH_RANGE: (f32, f32) = (16.0, 400.0);
/// Track height: the default and the vertical zoom range, in points.
const TRACK_HEIGHT: f32 = 60.0;
const TRACK_HEIGHT_RANGE: (f32, f32) = (18.0, 160.0);

#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistState {
    pub(crate) px_per_bar: f32,
    row_height: f32,
    /// A track divider being dragged: the track and its height so far, in
    /// percent of `row_height`. Drawn at once, run as one command on release.
    resize: Option<(u32, u16)>,
    /// The selected track (0-based): its selector strip is lit.
    pub(crate) track: u32,
    /// Scroll offset of the last frame, in points.
    pub(crate) offset: Vec2,
    /// Scroll offset asked for by the scroll bars, applied next frame.
    jump: Option<Vec2>,
    snap: Snap,
    drag: Option<ClipDrag>,
    /// Grid and visible area from the last frame (used by tests and tools).
    pub(crate) last_view: Option<(Grid, Rect)>,
    /// Selected clips as (track, start).
    pub(crate) selection: BTreeSet<(u32, u32)>,
    /// Rubber-band selection in progress.
    marquee: Option<(Pos2, Pos2)>,
    /// When set, clicking the playlist places this automation instead of the
    /// selected pattern.
    pub(crate) paint_automation: Option<AutomationId>,
    /// When set, clicking the playlist places an audio clip of this channel.
    pub(crate) paint_audio: Option<musiccraft_project::ChannelId>,
    /// What the picker beside the playlist lists.
    pub(crate) picker: Picker,
    /// Each pattern's miniature in the picker on the last frame, in pattern
    /// order (used by tests).
    pub(crate) picker_items: Vec<Rect>,
    /// Loop range being drawn on the ruler.
    loop_drag: Option<(u32, u32)>,
    /// The active tool.
    pub(crate) tool: Tool,
    /// Clips painted in the stroke in progress: (track, start, length).
    painted: Vec<(u32, u32, u32)>,
    /// The track the paint stroke in progress started on; it stays there.
    paint_row: Option<u32>,
    /// Clips touched by the delete stroke in progress.
    erased: Vec<Clip>,
    /// The song's bars and beats, refreshed every frame.
    pub(crate) meter: Meter,
    /// The playhead in ticks, refreshed every frame (keys zoom around it).
    playhead: f32,
    /// The zoom to return to after Shift+4 zoomed all the way out.
    zoom_back: Option<f32>,
    /// The window menu's View options.
    pub(crate) view: ViewOptions,
    /// Clips cut or copied, starts relative to the first one.
    clipboard: Vec<Clip>,
}

/// What the playlist draws, and whether it follows the playhead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViewOptions {
    /// Every other bar group shaded.
    pub(crate) shading: bool,
    /// A line under each track.
    pub(crate) separators: bool,
    /// During song playback the view pages along with the playhead.
    pub(crate) follow: bool,
    /// The clip list column.
    pub(crate) clip_list: bool,
    /// The clip list sits at the right instead of the left.
    pub(crate) list_right: bool,
    /// Index into `LIST_WIDTHS`.
    pub(crate) list_width: usize,
}

impl ViewOptions {
    fn list_width(self) -> f32 {
        LIST_WIDTHS
            .get(self.list_width)
            .map_or(LIST_WIDTHS[1].1, |w| w.1)
    }
}

impl Default for ViewOptions {
    fn default() -> Self {
        Self {
            shading: true,
            separators: true,
            follow: true,
            clip_list: true,
            list_right: false,
            list_width: 1,
        }
    }
}

impl Default for PlaylistState {
    fn default() -> Self {
        Self {
            px_per_bar: 72.0,
            row_height: TRACK_HEIGHT,
            resize: None,
            track: 0,
            offset: Vec2::ZERO,
            jump: None,
            snap: Snap::Bar,
            drag: None,
            last_view: None,
            selection: BTreeSet::new(),
            marquee: None,
            paint_automation: None,
            paint_audio: None,
            picker: Picker::All,
            picker_items: Vec::new(),
            loop_drag: None,
            tool: Tool::Draw,
            painted: Vec::new(),
            paint_row: None,
            erased: Vec::new(),
            meter: Meter::default(),
            playhead: 0.0,
            zoom_back: None,
            view: ViewOptions::default(),
            clipboard: Vec::new(),
        }
    }
}

impl PlaylistState {
    /// Keeps the tool, zoom, snap and view options and drops everything about
    /// the old project's clips: selections, gestures, the clip clipboard
    /// (its clips name the old project's patterns) and what the picker paints.
    pub(crate) fn forget_project(&mut self) {
        let old = std::mem::take(self);
        *self = Self {
            px_per_bar: old.px_per_bar,
            row_height: old.row_height,
            snap: old.snap,
            picker: old.picker,
            tool: old.tool,
            view: old.view,
            ..Self::default()
        };
    }
}

#[cfg(test)]
fn finish(drag: &ClipDrag) -> Option<Invocation> {
    let o = drag.original;
    let t = |ticks: u32| format!("{ticks}t");
    match drag.kind {
        DragKind::Move if (drag.track, drag.start) != (o.track, o.start) => Some(Invocation::new(
            "playlist.move_clip",
            [
                (o.track + 1).to_string(),
                t(o.start),
                (drag.track + 1).to_string(),
                t(drag.start),
            ],
        )),
        DragKind::Resize if drag.length != o.length => Some(Invocation::new(
            "playlist.resize_clip",
            [(o.track + 1).to_string(), t(o.start), t(drag.length)],
        )),
        DragKind::Trim if drag.start != o.start => Some(Invocation::new(
            "playlist.trim_clip",
            [(o.track + 1).to_string(), t(o.start), t(drag.start)],
        )),
        _ => None,
    }
}

/// The playlist tools, in header order, with their hints.
const TOOLS: [(Tool, Icon, &str); 8] = [
    (
        Tool::Draw,
        Icon::Pencil,
        "Draw (P): click places the selected item, drag moves, edge resizes",
    ),
    (
        Tool::Paint,
        Icon::Brush,
        "Paint (B): drag to lay the selected item end to end",
    ),
    (
        Tool::Delete,
        Icon::Eraser,
        "Delete (D): click or drag over clips to delete them",
    ),
    (
        Tool::Mute,
        Icon::Mute,
        "Mute (T): click a clip to mute or unmute it",
    ),
    (
        Tool::Slice,
        Icon::Slice,
        "Slice (C): click a clip to cut it in two there; Shift+click splits a pattern clip into two new patterns",
    ),
    (
        Tool::Select,
        Icon::Select,
        "Select (E): drag a box around clips; click empty space to clear",
    ),
    (
        Tool::Zoom,
        Icon::Zoom,
        "Zoom (Z): click to zoom in, Shift+click to zoom out",
    ),
    (
        Tool::Play,
        Icon::PlayFrom,
        "Play from here (Y): click to play from there",
    ),
];

/// The playlist window: arrange patterns, audio and automation as clips.
pub fn show(
    ctx: &egui::Context,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let mut open = state.windows.playlist;
    if !open {
        return;
    }
    state.playlist.meter = frame.project.song_meter();
    let area = state.workspace;
    let id = egui::Id::new("playlist-window");
    // Across the whole lower part of the workspace, a little over half its
    // height.
    let top = area.top() + area.height() * 0.42;
    let default = Rect::from_min_max(
        pos2(area.left() + 4.0, top),
        pos2(area.right() - 4.0, area.bottom() - 4.0),
    );
    // A pinned window takes its exact size from the frame; a minimum size
    // set after it would let it keep its old one.
    let pinned = crate::chrome::pinned(ctx, id);
    let maximized = crate::chrome::maximized(ctx, id);
    let mut window = crate::chrome::window(ctx, id, "Playlist", area, default, true)
        .default_height(default.height());
    if maximized {
        // Maximized, the playlist is the backdrop behind the other windows.
        window = window.order(egui::Order::Background);
    }
    if !pinned {
        window = window.min_width(360.0).min_height(140.0);
    }
    let window = window.show(ctx, |ui| {
        let mut data = (&mut *state, &mut *actions);
        let folded = crate::chrome::Header::new(id, "Playlist — Arrangement").show(
            ui,
            &mut open,
            &mut data,
            |ui, (state, actions)| window_menu(ui, state, frame, actions),
            |ui, (state, actions)| {
                tools(ui, state);
                ui.add_space(6.0);
                crate::piano_roll::play_button(ui, frame.project, PlayMode::Song, actions);
            },
        );
        if folded {
            return;
        }
        ui.horizontal_top(|ui| {
            let view = state.playlist.view;
            if !view.clip_list {
                canvas(ui, state, frame, actions);
            } else if view.list_right {
                // The list takes its column at the right edge and the
                // lanes fill what is left of the row.
                let row = ui.available_rect_before_wrap();
                let gap = ui.spacing().item_spacing.x;
                let split = row.right() - view.list_width() - gap;
                let lanes = egui::Rect::from_min_max(row.min, pos2(split, row.bottom()));
                let list = egui::Rect::from_min_max(pos2(split + gap, row.top()), row.max);
                ui.scope_builder(egui::UiBuilder::new().max_rect(lanes), |ui| {
                    canvas(ui, state, frame, actions);
                });
                ui.scope_builder(egui::UiBuilder::new().max_rect(list), |ui| {
                    pattern_list(ui, state, frame.project, actions);
                });
            } else {
                pattern_list(ui, state, frame.project, actions);
                canvas(ui, state, frame, actions);
            }
        });
    });
    crate::chrome::remember(ctx, id, window.as_ref().map(|w| &w.response));
    state.windows.playlist = open;
}

/// The header's tools: the snap button, then the editing tools.
fn tools(ui: &mut egui::Ui, state: &mut UiState) {
    snap_button(ui, state);
    ui.add_space(6.0);
    for (tool, icon, hint) in TOOLS {
        if icons::button(ui, icon, state.playlist.tool == tool, theme::ACCENT, hint).clicked() {
            state.playlist.tool = tool;
        }
    }
}

/// The snap glyph and its current value; a click lists the values.
fn snap_button(ui: &mut egui::Ui, state: &mut UiState) {
    let popup = egui::Id::new("playlist-snap");
    let open = egui::Popup::is_id_open(ui.ctx(), popup);
    let hint = format!(
        "Snap: {} · click to choose, wheel to step, right-click for Bar",
        state.playlist.snap.label()
    );
    let response = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 1.0;
            let glyph = icons::button(ui, Icon::Snap, open, theme::ACCENT, &hint);
            let value = ui.add(
                egui::Label::new(
                    RichText::new(state.playlist.snap.label())
                        .size(theme::FONT_LABEL)
                        .color(theme::TEXT_DIM),
                )
                .sense(Sense::click()),
            );
            glyph.union(value)
        })
        .inner;
    if response.secondary_clicked() {
        state.playlist.snap = Snap::Bar;
    }
    let notches = if response.hovered() {
        crate::scrollbars::wheel_notches(ui)
    } else {
        0.0
    };
    if notches != 0.0 {
        state.playlist.snap = step_snap(state.playlist.snap, notches > 0.0);
    }
    let toggle = response.clicked().then_some(egui::SetOpenCommand::Toggle);
    egui::Popup::menu(&response)
        .id(popup)
        .open_memory(toggle)
        .show(|ui| snap_list(ui, state));
}

/// The next coarser (or finer) snap value, stopping at the ends.
fn step_snap(snap: Snap, coarser: bool) -> Snap {
    let at = Snap::ALL.iter().position(|s| *s == snap).unwrap_or(0);
    let next = if coarser {
        (at + 1).min(Snap::ALL.len() - 1)
    } else {
        at.saturating_sub(1)
    };
    Snap::ALL[next]
}

/// The snap values, the current one ticked.
fn snap_list(ui: &mut egui::Ui, state: &mut UiState) {
    for snap in Snap::ALL {
        if ui
            .selectable_label(state.playlist.snap == snap, snap.label())
            .clicked()
        {
            state.playlist.snap = snap;
            ui.close();
        }
    }
}

/// The playlist's own window-menu items.
fn window_menu(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    let selected = selected_clips(state, project);
    ui.menu_button("Edit", |ui| {
        let any = !selected.is_empty();
        let ops = [
            (any, "Cut (Ctrl+X)", ClipOp::Cut),
            (any, "Copy (Ctrl+C)", ClipOp::Copy),
            (
                !state.playlist.clipboard.is_empty(),
                "Paste at the playhead (Ctrl+V)",
                ClipOp::Paste,
            ),
        ];
        for (enabled, label, op) in ops {
            if crate::widgets::add_enabled(ui, enabled, egui::Button::new(label)).clicked() {
                clipboard_op(ui.ctx(), state, project, op, actions);
                ui.close();
            }
        }
        menu_item(
            ui,
            actions,
            "Duplicate (Ctrl+B)",
            "Copy the selected clips right after their end",
            duplication(project, &selected),
        );
        let removal = removal(&selected);
        if removal.is_some() && ui.button("Delete (Del)").clicked() {
            actions.extend(removal.map(Action::Run));
            state.playlist.selection.clear();
            ui.close();
        }
        let nudges = [
            ("Nudge left (Shift+Left)", Nudge::Left),
            ("Nudge right (Shift+Right)", Nudge::Right),
            ("Move up a track (Shift+Up)", Nudge::Up),
            ("Move down a track (Shift+Down)", Nudge::Down),
        ];
        for (label, nudge) in nudges {
            if crate::widgets::add_enabled(ui, !selected.is_empty(), egui::Button::new(label))
                .clicked()
            {
                nudge_selection(state, project, nudge, actions);
            }
        }
        ui.separator();
        time_items(ui, project, actions);
    });
    ui.menu_button("Clip", |ui| clip_menu(ui, state, frame, actions));
    ui.menu_button("Select", |ui| {
        let sources: Vec<ClipSource> = selected.iter().map(|c| c.source).collect();
        let items = [
            ("All (Ctrl+A)", Pick::All, true),
            ("None (Esc)", Pick::None, true),
            ("Invert", Pick::Invert, true),
            ("Muted clips", Pick::Muted, true),
            (
                "Clips of the same source",
                Pick::SameSource,
                !sources.is_empty(),
            ),
            (
                "Everything inside the loop region",
                Pick::InLoop,
                project.playlist.loop_range.is_some(),
            ),
        ];
        for (label, pick, enabled) in items {
            if crate::widgets::add_enabled(ui, enabled, egui::Button::new(label)).clicked() {
                state.playlist.selection = picked(state, project, pick, &sources);
                ui.close();
            }
        }
    });
    ui.menu_button("Snap", |ui| snap_list(ui, state));
    ui.menu_button("Zoom", |ui| zoom_items(ui, state, project));
    ui.menu_button("Tracks", |ui| {
        let list = &project.playlist;
        menu_item(
            ui,
            actions,
            "Unmute all tracks",
            "Every muted playlist track plays again",
            per_track(&list.muted_tracks, "playlist.toggle_track_mute"),
        );
        menu_item(
            ui,
            actions,
            "Clear solo",
            "No playlist track is soloed",
            per_track(&list.soloed_tracks, "playlist.toggle_track_solo"),
        );
        menu_item(
            ui,
            actions,
            "Reset all track heights",
            "Every playlist track returns to the global height",
            (!list.track_heights.is_empty())
                .then(|| Invocation::bare("playlist.reset_track_heights")),
        );
    });
    ui.menu_button("View", |ui| {
        let view = &mut state.playlist.view;
        ui.add(egui::Checkbox::new(&mut view.clip_list, "Clip list"))
            .hint("Alt+P shows or hides the clip list");
        ui.checkbox(&mut view.list_right, "Clip list on the right side");
        ui.menu_button("Clip list width", |ui| {
            for (i, (label, _)) in LIST_WIDTHS.iter().enumerate() {
                if ui.radio(view.list_width == i, *label).clicked() {
                    view.list_width = i;
                    ui.close();
                }
            }
        });
        ui.checkbox(&mut view.shading, "Bar-group shading");
        ui.checkbox(&mut view.separators, "Track separators");
        ui.checkbox(&mut view.follow, "Follow the playhead");
    });
    ui.menu_button("Markers", |ui| {
        if ui.button("Add marker at the playhead (Alt+T)").clicked() {
            actions.push(Action::Run(Invocation::bare("ui.add_marker")));
            ui.close();
        }
        let playhead = state.playlist.playhead.max(0.0) as u32;
        let beat = playhead - playhead % PPQ;
        menu_item(
            ui,
            actions,
            "Add pause marker at the playhead",
            "Song playback stops here and the playhead stays on the marker",
            pause_marker(project, beat),
        );
        for every in [4, 8] {
            menu_item(
                ui,
                actions,
                &format!("Add a marker every {every} bars"),
                "Through the end of the song; bars that have a marker keep it",
                markers_every(project, &state.playlist.meter, every),
            );
        }
        // A change starts at the playhead's bar; the first bar's signature is
        // the project's.
        let bar = state.playlist.meter.bar(playhead);
        let change = crate::widgets::add_enabled(
            ui,
            bar.index > 0,
            egui::Button::new("Time signature change at this bar…"),
        )
        .on_hover_text("From the playhead's bar on (the first bar's is the project signature)");
        if change.clicked() {
            state.open_dialog(Dialog::time_signature(bar.start, &bar.sig.to_string()));
            ui.close();
        }
        menu_item(
            ui,
            actions,
            "Remove all markers",
            "Time signature changes stay",
            remove_markers(project),
        );
    });
}

/// A pause marker at `tick`, as one undo step.
fn pause_marker(project: &Project, tick: u32) -> Option<Invocation> {
    let taken = project.playlist.markers.iter().any(|m| m.tick == tick);
    (!taken).then(|| {
        Invocation::new(
            musiccraft_commands::BATCH,
            [
                Invocation::new(
                    "playlist.add_marker",
                    [format!("{tick}t"), "Pause".to_owned()],
                )
                .to_string(),
                Invocation::new(
                    "playlist.set_marker_action",
                    [format!("{tick}t"), "pause".to_owned()],
                )
                .to_string(),
            ],
        )
    })
}

/// A marker named after its bar every `every` bars through the song's end,
/// skipping bars that already have one, as one undo step.
fn markers_every(project: &Project, meter: &Meter, every: u32) -> Option<Invocation> {
    let end = project.playlist.length_ticks();
    let adds = (1..)
        .map(|n| n * every)
        .map(|bar| (bar, meter.bar_start(bar)))
        .take_while(|(_, tick)| *tick < end)
        .filter(|(_, tick)| !project.playlist.markers.iter().any(|m| m.tick == *tick))
        .map(|(bar, tick)| {
            Invocation::new(
                "playlist.add_marker",
                [format!("{tick}t"), format!("Bar {}", bar + 1)],
            )
        })
        .collect();
    batch(adds)
}

/// Removes every marker except time signature changes, as one undo step.
fn remove_markers(project: &Project) -> Option<Invocation> {
    batch(
        project
            .playlist
            .markers
            .iter()
            .filter(|m| !matches!(m.action, MarkerAction::TimeSignature(_)))
            .map(|m| Invocation::new("playlist.remove_marker", [format!("{}t", m.tick)]))
            .collect(),
    )
}

/// `name <track>` for each of `tracks` (0-based), as one undo step.
fn per_track(tracks: &BTreeSet<u32>, name: &str) -> Option<Invocation> {
    batch(
        tracks
            .iter()
            .map(|t| Invocation::new(name, [(t + 1).to_string()]))
            .collect(),
    )
}

/// A Select menu item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    All,
    None,
    Invert,
    Muted,
    /// Every clip that plays one of the selected clips' sources.
    SameSource,
    /// Every clip that overlaps the loop region.
    InLoop,
}

/// The selection a Select menu item makes, as (track, start) pairs.
fn picked(
    state: &UiState,
    project: &Project,
    pick: Pick,
    sources: &[ClipSource],
) -> BTreeSet<(u32, u32)> {
    let selected = &state.playlist.selection;
    let in_loop = |c: &Clip| {
        project
            .playlist
            .loop_range
            .is_some_and(|(from, to)| c.start < to && c.end() > from)
    };
    project
        .playlist
        .clips
        .iter()
        .filter(|c| match pick {
            Pick::All => true,
            Pick::None => false,
            Pick::Invert => !selected.contains(&(c.track, c.start)),
            Pick::Muted => c.muted,
            Pick::SameSource => sources.contains(&c.source),
            Pick::InLoop => in_loop(c),
        })
        .map(|c| (c.track, c.start))
        .collect()
}

/// A change of the playlist's horizontal zoom.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Zoom {
    /// Multiply the bar width, keeping the playhead where it is on screen.
    By(f32),
    /// A fixed bar width (Shift+1–3).
    Level(f32),
    /// All the way out, or back to the zoom before that (Shift+4).
    OutAndBack,
    /// The loop region in the window (Shift+5).
    Loop,
    /// The whole song in the window.
    Song,
}

/// The zoom presets of Shift+1, Shift+2 and Shift+3, in points per 4/4 bar.
const ZOOM_LEVELS: [f32; 3] = [32.0, 96.0, 240.0];

/// The Zoom submenu of the window menu.
fn zoom_items(ui: &mut egui::Ui, state: &mut UiState, project: &Project) {
    let items = [
        ("Zoom in (Page Up)", Zoom::By(1.5), true),
        ("Zoom out (Page Down)", Zoom::By(1.0 / 1.5), true),
        ("Zoom preset 1 (Shift+1)", Zoom::Level(ZOOM_LEVELS[0]), true),
        ("Zoom preset 2 (Shift+2)", Zoom::Level(ZOOM_LEVELS[1]), true),
        ("Zoom preset 3 (Shift+3)", Zoom::Level(ZOOM_LEVELS[2]), true),
        (
            "Zoom all the way out and back (Shift+4)",
            Zoom::OutAndBack,
            true,
        ),
        (
            "Zoom to the loop region (Shift+5)",
            Zoom::Loop,
            project.playlist.loop_range.is_some(),
        ),
        ("Fit the song", Zoom::Song, true),
    ];
    for (label, op, enabled) in items {
        if crate::widgets::add_enabled(ui, enabled, egui::Button::new(label)).clicked() {
            zoom(state, project, op);
        }
    }
}

fn zoom(state: &mut UiState, project: &Project, op: Zoom) {
    let list = &mut state.playlist;
    let clamp = |px: f32| px.clamp(BAR_WIDTH_RANGE.0, BAR_WIDTH_RANGE.1);
    let range = match op {
        Zoom::By(factor) => {
            let old = list.px_per_bar;
            list.px_per_bar = clamp(old * factor);
            // Keep the playhead at the same spot in the view.
            let tick = list.playhead / ZOOM_TICKS as f32;
            let x = (list.offset.x + tick * (list.px_per_bar - old)).max(0.0);
            list.jump = Some(vec2(x, list.offset.y));
            return;
        }
        Zoom::Level(px) => {
            list.px_per_bar = clamp(px);
            return;
        }
        Zoom::OutAndBack => {
            match list.zoom_back.take() {
                Some(px) => list.px_per_bar = px,
                None => {
                    list.zoom_back = Some(list.px_per_bar);
                    list.px_per_bar = BAR_WIDTH_RANGE.0;
                }
            }
            return;
        }
        Zoom::Loop => project.playlist.loop_range,
        Zoom::Song => {
            let end = project.song_meter().bar(project.playlist.length_ticks());
            Some((0, project.song_meter().bar_start(end.index.max(1))))
        }
    };
    let (Some((from, to)), Some((_, view))) = (range, list.last_view) else {
        return;
    };
    let bars = to.saturating_sub(from).max(1) as f32 / ZOOM_TICKS as f32;
    list.px_per_bar = clamp((view.width() - HEADER_WIDTH - 12.0) / bars);
    let x = from as f32 / ZOOM_TICKS as f32 * list.px_per_bar;
    list.jump = Some(vec2(x, list.offset.y));
}

/// Scrolls the playlist to the first clip of `pattern` (Show in playlist).
pub(crate) fn show_pattern(
    state: &mut UiState,
    project: &Project,
    pattern: musiccraft_project::PatternId,
) {
    show_source(state, project, ClipSource::Pattern(pattern));
}

/// Scrolls the playlist to the first clip of `source` (Show in playlist).
fn show_source(state: &mut UiState, project: &Project, source: ClipSource) {
    let list = &mut state.playlist;
    let first = project
        .playlist
        .clips
        .iter()
        .filter(|c| c.source == source)
        .min_by_key(|c| (c.start, c.track));
    if let Some(c) = first {
        let x = c.start as f32 / ZOOM_TICKS as f32 * list.px_per_bar;
        let y = list
            .last_view
            .map_or(c.track as f32 * list.row_height, |(grid, _)| {
                grid.y(c.track) - grid.origin.y
            });
        list.jump = Some(vec2(x, y));
    }
}

/// The clips in the playlist selection.
fn selected_clips(state: &UiState, project: &Project) -> Vec<Clip> {
    project
        .playlist
        .clips
        .iter()
        .filter(|c| state.playlist.selection.contains(&(c.track, c.start)))
        .copied()
        .collect()
}

/// Removes `selected` as one undo step.
fn removal(selected: &[Clip]) -> Option<Invocation> {
    batch(
        selected
            .iter()
            .map(|c| {
                Invocation::new(
                    "playlist.remove_clip",
                    [(c.track + 1).to_string(), format!("{}t", c.start)],
                )
            })
            .collect(),
    )
}

/// How far a duplicate of `selected` moves: from its first start to its end.
fn duplicate_span(selected: &[Clip]) -> u32 {
    let first = selected.iter().map(|c| c.start).min().unwrap_or(0);
    let end = selected.iter().map(Clip::end).max().unwrap_or(first);
    end - first
}

/// Copies `selected` right after its end, as one undo step.
fn duplication(project: &Project, selected: &[Clip]) -> Option<Invocation> {
    let span = duplicate_span(selected);
    batch(
        selected
            .iter()
            .map(|c| add_clip_invocation(project, c, c.track, c.start + span))
            .collect(),
    )
}

fn pattern_list(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    actions: &mut Vec<Action>,
) {
    state.playlist.picker_items.clear();
    let width = state.playlist.view.list_width();
    ui.vertical(|ui| {
        ui.set_width(width);
        // Kind switches: one lit shows only that kind; none lit shows all.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(((width - 3.0 * 28.0 - 4.0) / 2.0).max(0.0));
            for (picker, icon, hint) in [
                (
                    Picker::Patterns,
                    Icon::Pattern,
                    "Patterns: show only patterns · click again for all",
                ),
                (
                    Picker::Audio,
                    Icon::Wave,
                    "Audio: show only audio clips · click again for all",
                ),
                (
                    Picker::Automation,
                    Icon::Automation,
                    "Automation: show only automation · click again for all",
                ),
            ] {
                let lit = state.playlist.picker == picker;
                if icons::toggle(ui, icon, lit, hint).clicked() {
                    state.playlist.picker = if lit { Picker::All } else { picker };
                }
            }
        });
        let picker = state.playlist.picker;
        if matches!(picker, Picker::All | Picker::Patterns) {
            pattern_section(ui, state, project, actions);
        }
        if matches!(picker, Picker::All | Picker::Audio) {
            audio_section(ui, state, project, actions);
        }
        if matches!(picker, Picker::All | Picker::Automation) {
            automation_section(ui, state, project, actions);
        }
    });
}

/// What the picker lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Picker {
    All,
    Patterns,
    Audio,
    Automation,
}

/// Channels that have audio clips; clicking one paints its clips.
fn audio_section(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    actions: &mut Vec<Action>,
) {
    let mut sources: Vec<musiccraft_project::ChannelId> = Vec::new();
    for clip in &project.playlist.clips {
        if let ClipSource::Audio(id) = clip.source {
            if !sources.contains(&id) {
                sources.push(id);
            }
        }
    }
    ui.add_space(6.0);
    if sources.is_empty() && state.playlist.picker == Picker::Audio {
        ui.label(
            RichText::new("Drop a sound on the playlist")
                .small()
                .color(theme::TEXT_DIM),
        );
    }
    for id in sources {
        let Some(index) = project.channel_position(id) else {
            continue;
        };
        let selected = state.playlist.paint_audio == Some(id);
        let name = &project.channels[index].name;
        let response = glyph_row(
            ui,
            Icon::Wave,
            name,
            crate::channel_color(project, index),
            selected,
        )
        .hint("Click to paint this audio clip on the playlist · right-click for options");
        if response.clicked() {
            state.playlist.paint_audio = Some(id);
            state.playlist.paint_automation = None;
        }
        response.context_menu(|ui| {
            if ui.button("Channel settings…").clicked() {
                state.selected_channel = index;
                state.windows.channel_settings = true;
                ui.close();
            }
            if ui
                .button("Split into stems")
                .on_hover_text(
                    "Split this sound into vocals, drums, bass and other, each a new channel with clips under the originals",
                )
                .clicked()
            {
                actions.push(Action::Run(Invocation::new(
                    "sample.split_stems",
                    [(index + 1).to_string()],
                )));
                ui.close();
            }
        });
    }
}

fn pattern_section(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    actions: &mut Vec<Action>,
) {
    {
        // A solid scroll bar takes its own strip: a floating one would lie
        // over the right edge of the miniatures and catch their clicks.
        ui.scope(|ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            egui::ScrollArea::vertical()
                .id_salt("pattern-list")
                .max_height(260.0)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    for (i, pattern) in project.patterns.iter().enumerate() {
                        let selected = i == project.current_pattern;
                        let painting = state.playlist.paint_automation.is_none()
                            && state.playlist.paint_audio.is_none();
                        let response = miniature(ui, project, pattern, selected && painting)
                            .hint(
                                "Click to paint this pattern · drag it onto a track · \
                                 double-click to open it in the piano roll · right-click for options",
                            );
                        if response.drag_started() {
                            egui::DragAndDrop::set_payload(
                                ui.ctx(),
                                crate::browser::Dragged::Pattern {
                                    id: pattern.id,
                                    label: pattern.name.clone(),
                                },
                            );
                        }
                        state.playlist.picker_items.push(response.rect);
                        if response.clicked() {
                            state.playlist.paint_automation = None;
                            state.playlist.paint_audio = None;
                            if !selected {
                                actions.push(Action::Run(Invocation::new(
                                    "pattern.select",
                                    [(i + 1).to_string()],
                                )));
                            }
                        }
                        if response.double_clicked() {
                            open_in_piano_roll(state, i, actions);
                        }
                        response.context_menu(|ui| {
                            if ui.button("Rename…").clicked() {
                                state.open_dialog(Dialog::rename_pattern(i, &pattern.name));
                                ui.close();
                            }
                            if ui.button("Show in playlist (Shift+C)").clicked() {
                                show_source(state, project, ClipSource::Pattern(pattern.id));
                                ui.close();
                            }
                            if ui.button("Clone").clicked() {
                                actions.push(Action::Run(Invocation::new(
                                    "pattern.clone",
                                    [(i + 1).to_string()],
                                )));
                                ui.close();
                            }
                            if crate::widgets::add_enabled(
                                ui,
                                splittable(pattern),
                                egui::Button::new("Split by channel"),
                            )
                            .on_hover_text(
                                "One pattern per channel, with clips stacked where this one's were",
                            )
                            .clicked()
                            {
                                actions.push(Action::Run(Invocation::new(
                                    "pattern.split_by_channel",
                                    [(i + 1).to_string()],
                                )));
                                ui.close();
                            }
                            ui.menu_button("Colour", |ui| {
                                if let Some(choice) = crate::swatches::grid(ui, pattern.color) {
                                    actions.push(Action::Run(Invocation::new(
                                        "pattern.set_color",
                                        [(i + 1).to_string(), crate::swatches::arg(choice)],
                                    )));
                                    ui.close();
                                }
                            });
                            let sig = pattern.time_signature;
                            ui.menu_button(format!("Time signature ({sig})…"), |ui| {
                                if let Some(new) =
                                    signature::picker(ui, Some(sig), &mut state.sig_draft)
                                {
                                    actions.push(Action::Run(Invocation::new(
                                        "pattern.set_time_signature",
                                        [(i + 1).to_string(), new.to_string()],
                                    )));
                                }
                            });
                            if !pattern.automation.is_empty()
                                && ui
                                    .button(format!(
                                        "Clear automation ({} lanes)",
                                        pattern.automation.len()
                                    ))
                                    .on_hover_text(
                                        "Remove the knob moves recorded into this pattern",
                                    )
                                    .clicked()
                            {
                                actions.push(Action::Run(Invocation::new(
                                    "pattern.clear_events",
                                    [(i + 1).to_string()],
                                )));
                                ui.close();
                            }
                            let deletable = project.patterns.len() > 1;
                            crate::widgets::enabled_ui(ui, deletable, |ui| {
                                let label = RichText::new("Delete");
                                let label = if deletable {
                                    label.color(theme::DANGER)
                                } else {
                                    label
                                };
                                if ui.button(label).clicked() {
                                    actions.push(Action::Run(Invocation::new(
                                        "pattern.remove",
                                        [(i + 1).to_string()],
                                    )));
                                    ui.close();
                                }
                            });
                        });
                    }
                });
        });
        let add = ui.vertical_centered(|ui| {
            icons::button(
                ui,
                Icon::Plus,
                false,
                theme::ACCENT,
                "New pattern (F4 picks the next empty one)",
            )
        });
        if add.inner.clicked() {
            state.playlist.paint_automation = None;
            state.playlist.paint_audio = None;
            actions.push(Action::Run(Invocation::bare("pattern.add")));
        }
    }
}

/// Makes pattern `index` (0-based) current and brings the piano roll to
/// the front with it, opening the window if it was closed.
fn open_in_piano_roll(state: &mut UiState, index: usize, actions: &mut Vec<Action>) {
    actions.push(Action::Run(Invocation::new(
        "pattern.select",
        [(index + 1).to_string()],
    )));
    state.windows.piano_roll = true;
    state.arrange = Some(crate::Arrange::Raise("piano-roll"));
}

/// Height of a pattern's miniature in the picker.
const MINIATURE_HEIGHT: f32 = 34.0;

/// A pattern drawn as a small clip: its name over its notes.
fn miniature(
    ui: &mut egui::Ui,
    project: &Project,
    pattern: &musiccraft_project::Pattern,
    selected: bool,
) -> egui::Response {
    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, MINIATURE_HEIGHT), egui::Sense::click_and_drag());
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let color = crate::pattern_color(pattern).unwrap_or(UNCOLOURED_CLIP);
    painter.rect_filled(rect, 4.0, color.gamma_multiply(0.8));
    let title = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + TITLE_HEIGHT));
    painter.rect_filled(
        title,
        egui::CornerRadius {
            nw: 4,
            ne: 4,
            sw: 0,
            se: 0,
        },
        theme::shade(70),
    );
    painter.text(
        pos2(title.left() + 4.0, title.center().y),
        Align2::LEFT_CENTER,
        &pattern.name,
        FontId::proportional(theme::FONT_VALUE),
        theme::TEXT,
    );
    let body = Rect::from_min_max(
        pos2(rect.left() + 3.0, title.bottom() + 2.0),
        pos2(rect.right() - 3.0, rect.bottom() - 2.0),
    );
    if pattern.notes.is_empty() && pattern.automation.is_empty() {
        painter.text(
            body.center(),
            Align2::CENTER_CENTER,
            "empty",
            FontId::proportional(theme::FONT_CAPTION),
            theme::TEXT_DIM,
        );
    } else {
        lane_thumbnail(&painter, pattern, body);
        pattern_thumbnail(&painter, project, pattern, body);
    }
    let edge = if selected {
        Stroke::new(1.5, theme::ACCENT)
    } else if response.hovered() {
        Stroke::new(1.0, theme::TEXT_DIM)
    } else {
        Stroke::new(1.0, theme::shade(140))
    };
    painter.rect_stroke(rect, 4.0, edge, egui::StrokeKind::Inside);
    response
}

/// A clip-list row: the kind glyph and the name in the item's colour, with
/// a raised fill while selected or hovered.
fn glyph_row(
    ui: &mut egui::Ui,
    icon: Icon,
    name: &str,
    color: Color32,
    selected: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click());
    let painter = ui.painter_at(rect);
    if selected || response.hovered() {
        let fill = if selected {
            theme::RAISED
        } else {
            theme::RAISED_HOVER
        };
        painter.rect_filled(rect, 2.0, fill);
    }
    let glyph = Rect::from_min_size(rect.left_top() + vec2(2.0, 2.0), vec2(14.0, 14.0));
    icons::paint(&painter, glyph, icon, color);
    let mut job = egui::text::LayoutJob::simple_singleline(
        name.to_owned(),
        FontId::proportional(theme::FONT_LABEL),
        color,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 22.0);
    let galley = painter.layout_job(job);
    painter.galley(
        pos2(rect.left() + 20.0, rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    response
}

fn automation_section(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    actions: &mut Vec<Action>,
) {
    {
        if !project.automations.is_empty() {
            ui.add_space(6.0);
            for (i, automation) in project.automations.iter().enumerate() {
                let selected = state.playlist.paint_automation == Some(automation.id);
                let response = glyph_row(
                    ui,
                    Icon::Automation,
                    &automation.name,
                    theme::AUTOMATION,
                    selected,
                )
                .hint("Click to paint it on the playlist · double-click to edit");
                if response.clicked() {
                    state.playlist.paint_automation = Some(automation.id);
                    state.playlist.paint_audio = None;
                }
                if response.double_clicked() {
                    state.automation_editor = Some(automation.id);
                }
                response.context_menu(|ui| {
                    if ui.button("Edit…").clicked() {
                        state.automation_editor = Some(automation.id);
                        ui.close();
                    }
                    if ui.button("Rename…").clicked() {
                        state.open_dialog(Dialog::rename_automation(i, &automation.name));
                        ui.close();
                    }
                    if ui
                        .button(RichText::new("Delete").color(theme::DANGER))
                        .clicked()
                    {
                        actions.push(Action::Run(Invocation::new(
                            "automation.remove",
                            [(i + 1).to_string()],
                        )));
                        ui.close();
                    }
                });
            }
        }
    }
}

/// The tracks' own heights as grid row scales, or `None` when every track
/// has the global height.
fn row_scales(
    playlist: &musiccraft_project::Playlist,
    resize: Option<(u32, u16)>,
) -> Option<crate::geometry::RowScales> {
    if playlist.track_heights.is_empty() && resize.is_none() {
        return None;
    }
    let mut scales = [100; crate::geometry::SCALED_ROWS];
    for (&track, &percent) in playlist
        .track_heights
        .iter()
        .chain(resize.as_ref().map(|(t, p)| (t, p)))
    {
        if let Some(scale) = scales.get_mut(track as usize) {
            *scale = percent;
        }
    }
    Some(scales)
}

fn canvas(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let project = frame.project;
    state.playlist.playhead = frame.engine.position_ticks.max(0.0) as f32;
    let px_per_tick = state.playlist.px_per_bar / ZOOM_TICKS as f32;
    let row_h = state.playlist.row_height;
    let scales = row_scales(&project.playlist, state.playlist.resize);
    let meter = state.playlist.meter.clone();
    let bars = (meter.bar(project.playlist.length_ticks()).index + 16).max(32);
    let tracks_height = Grid {
        origin: Pos2::ZERO,
        px_per_tick,
        row_height: row_h,
        rows: PLAYLIST_TRACKS,
        scales,
    }
    .y(PLAYLIST_TRACKS);
    let size = vec2(
        HEADER_WIDTH + meter.bar_start(bars) as f32 * px_per_tick,
        RULER_HEIGHT + tracks_height,
    );
    // The scroll/zoom row runs along the top and the vertical bar down the
    // right; the view (ruler, track headers and lanes) fills the rest.
    let body = ui.available_rect_before_wrap();
    ui.advance_cursor_after_rect(body);
    let mut view_ui =
        ui.new_child(egui::UiBuilder::new().max_rect(crate::scrollbars::view_rect(body)));
    let mut scroll = egui::ScrollArea::both()
        .id_salt("playlist-scroll")
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
    if let Some(jump) = state.playlist.jump.take() {
        scroll = scroll.scroll_offset(jump);
    }
    let output = scroll.show(&mut view_ui, |ui| {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let clip = ui.clip_rect();
        let grid = Grid {
            origin: pos2(rect.left() + HEADER_WIDTH, rect.top() + RULER_HEIGHT),
            px_per_tick,
            row_height: row_h,
            rows: PLAYLIST_TRACKS,
            scales,
        };
        state.playlist.last_view = Some((grid, clip));
        if let Some(offset) = crate::scrollbars::middle_pan(ui, clip, clip.min - rect.min) {
            state.playlist.jump = Some(offset);
        }
        let painter = ui.painter_at(clip);
        draw_grid(&painter, &grid, (clip, rect), (&meter, state.playlist.view));

        let clips = &project.playlist.clips;
        let moving = moving_clips(state, clips);
        let (dt, dtrack, dl) = drag_delta(state, &moving);
        for clip_item in clips {
            let is_moving = moving.contains(clip_item);
            let erasing = state.playlist.erased.contains(clip_item);
            if !is_moving && !erasing {
                let selected = state
                    .playlist
                    .selection
                    .contains(&(clip_item.track, clip_item.start));
                draw_clip(
                    &painter,
                    &grid,
                    frame,
                    clip_item,
                    (clip_item.track, clip_item.start, clip_item.length),
                    if selected {
                        Look::Selected
                    } else {
                        Look::Normal
                    },
                );
            }
        }
        // Clips being painted, drawn as the current pattern.
        let pattern_id = project.current_pattern().id;
        for (track, start, length) in &state.playlist.painted {
            let preview = Clip {
                source: ClipSource::Pattern(pattern_id),
                track: *track,
                start: *start,
                length: *length,
                offset: 0,
                muted: false,
            };
            draw_clip(
                &painter,
                &grid,
                frame,
                &preview,
                (*track, *start, *length),
                Look::Ghost,
            );
        }
        let trimming = state
            .playlist
            .drag
            .is_some_and(|d| d.kind == DragKind::Trim);
        for c in &moving {
            let (track, start) = shifted(c, dt, dtrack);
            let length = u32::try_from((i64::from(c.length) + dl).max(1)).unwrap_or(1);
            let mut shown = *c;
            if trimming {
                shown.offset = u32::try_from(i64::from(c.offset) + dt).unwrap_or(0);
            }
            draw_clip(
                &painter,
                &grid,
                frame,
                &shown,
                (track, start, length),
                Look::Ghost,
            );
        }
        if let Some((origin, current)) = state.playlist.marquee {
            let area = Rect::from_two_pos(origin, current);
            painter.rect_filled(area, 0.0, theme::SELECTION_FILL);
            painter.rect_stroke(
                area,
                0.0,
                Stroke::new(1.0, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        if project.play_mode == PlayMode::Song {
            // The playhead line shows in song mode, stopped or playing.
            let x = grid.x(frame.engine.position_ticks as f32);
            if x >= clip.left() + HEADER_WIDTH {
                painter.line_segment(
                    [pos2(x, clip.top()), pos2(x, clip.bottom())],
                    Stroke::new(1.5, theme::PLAYHEAD),
                );
            }
            // While playing, follow the playhead when it leaves the view.
            let visible = clip.left() + HEADER_WIDTH..clip.right() - 40.0;
            if frame.engine.playing
                && state.playlist.view.follow
                && !visible.contains(&x)
                && state.playlist.drag.is_none()
            {
                let target = Rect::from_min_size(
                    pos2(x - HEADER_WIDTH - 20.0, clip.top()),
                    vec2(clip.width() - 60.0, 1.0),
                );
                ui.scroll_to_rect(target, Some(egui::Align::Min));
            }
        }
        let boxes: Vec<(usize, Rect)> = clips
            .iter()
            .enumerate()
            .map(|(i, c)| (i, grid.span(c.track, c.start, c.length)))
            .collect();
        // A click on a clip's kind glyph selects the clip and opens its menu;
        // drags from the glyph still trim or move the clip.
        let glyph_click = response
            .clicked_by(PointerButton::Primary)
            .then(|| response.interact_pointer_pos())
            .flatten()
            .and_then(|p| glyph_hit(clips, &grid, clip, p).map(|c| (c, p)));
        let menu = egui::Id::new("playlist-clip-glyph-menu");
        if let Some((c, pos)) = glyph_click {
            state.playlist.selection = BTreeSet::from([(c.track, c.start)]);
            ui.data_mut(|d| d.insert_temp(menu, pos));
        } else if !clip_curve(ui, state, frame, &grid, &response, clip, actions) {
            edit(ui, state, project, &grid, &response, clip, &boxes, actions);
        }
        let at = ui.data(|d| d.get_temp::<Pos2>(menu)).unwrap_or(clip.min);
        egui::Popup::menu(&response)
            .id(menu)
            .open_memory(glyph_click.map(|_| egui::SetOpenCommand::Bool(true)))
            .anchor(egui::PopupAnchor::Position(at))
            .show(|ui| clip_menu(ui, state, frame, actions));
        draw_ruler(
            &painter,
            &grid,
            clip,
            project,
            state.playlist.loop_drag,
            &meter,
        );
        if project.play_mode == PlayMode::Song {
            // The ruler marks the playhead with a small triangle.
            let x = grid.x(frame.engine.position_ticks as f32);
            if x >= clip.left() + HEADER_WIDTH {
                let bottom = clip.top() + RULER_HEIGHT;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        pos2(x - 5.0, bottom - 8.0),
                        pos2(x + 5.0, bottom - 8.0),
                        pos2(x, bottom),
                    ],
                    theme::PLAYHEAD,
                    Stroke::NONE,
                ));
            }
        }
        ruler(ui, state, project, &grid, clip, actions);
        headers(ui, state, project, &grid, clip, actions);
        if response.hovered() {
            let zoom = ui.input(|i| i.zoom_delta());
            if zoom != 1.0 {
                state.playlist.px_per_bar =
                    (state.playlist.px_per_bar * zoom).clamp(BAR_WIDTH_RANGE.0, BAR_WIDTH_RANGE.1);
            }
        }
    });
    state.playlist.offset = output.state.offset;
    let total = meter.bar_start(bars) as f32;
    bars_around(ui, state, project, body, (output.inner_rect.size(), total));
}

/// The shared scroll/zoom row and vertical bar around the playlist's view.
fn bars_around(
    ui: &egui::Ui,
    state: &mut UiState,
    project: &Project,
    body: Rect,
    (view, total): (Vec2, f32),
) {
    use crate::scrollbars::{self, Setup};
    let list = &mut state.playlist;
    let zoom = ZOOM_TICKS as f32;
    let (lo, hi) = BAR_WIDTH_RANGE;
    let setup = Setup {
        view,
        offset: list.offset,
        lead: HEADER_WIDTH,
        top: RULER_HEIGHT,
        tail: 0.0,
        x: (
            list.px_per_bar / zoom,
            total,
            list.meter.bar_start(1) as f32,
            (lo / zoom, hi / zoom),
        ),
        y: (
            list.row_height,
            PLAYLIST_TRACKS as f32,
            TRACK_HEIGHT_RANGE,
            TRACK_HEIGHT,
        ),
        y_label: "Track height",
    };
    let out = scrollbars::around(
        ui,
        body,
        egui::Id::new("playlist-bars"),
        setup,
        |p, rail, span| {
            overview(p, rail, project, span);
        },
    );
    if let Some(scale) = out.scale {
        list.px_per_bar = scale * zoom;
    }
    if let Some(row) = out.row {
        list.row_height = row;
    }
    if out.jump.is_some() {
        list.jump = out.jump;
    }
    // Above the track headers, beside the scroll row.
    let corner = Rect::from_min_size(body.min, vec2(HEADER_WIDTH, scrollbars::BAR));
    ui.painter().rect_filled(corner, 0.0, theme::PANEL_DARK);
}

/// A miniature of the song along the scroll rail: one dash per clip in its
/// colour, brighter inside the visible range.
fn overview(painter: &egui::Painter, rail: Rect, project: &Project, view: crate::scrollbars::Span) {
    let total = view.total.max(1.0);
    // Spread the tracks that hold clips over the rail's height.
    let used = project
        .playlist
        .clips
        .iter()
        .map(|c| c.track + 1)
        .max()
        .unwrap_or(1)
        .max(4) as f32;
    for clip in &project.playlist.clips {
        let (color, _, _) = clip_identity(project, clip);
        let x0 = rail.left() + clip.start as f32 / total * rail.width();
        let w = (clip.length as f32 / total * rail.width()).max(1.5);
        let y = rail.top() + (clip.track as f32 + 0.5) / used * rail.height();
        let inside = (clip.start as f32) < view.first + view.span
            && (clip.start + clip.length) as f32 > view.first;
        let tone = theme::at_strength(color, if inside { 220 } else { 110 });
        painter.line_segment([pos2(x0, y), pos2(x0 + w, y)], Stroke::new(1.5, tone));
    }
}

/// Bars in one bar group: the unit of the shading and the strongest lines.
const BAR_GROUP: u32 = 4;
/// Above this many points per bar, a bar group is a single bar.
const WIDE_BAR: f32 = 240.0;

/// One lane tone with track separators, alternate bar groups washed a
/// little lighter, and step, beat, bar and group lines getting stronger in
/// that order.
fn draw_grid(
    painter: &egui::Painter,
    grid: &Grid,
    (clip, rect): (Rect, Rect),
    (meter, view): (&Meter, ViewOptions),
) {
    let lanes = Rect::from_min_max(grid.origin, rect.max);
    painter.rect_filled(lanes, 0.0, theme::LANE);
    let first = grid.tick_at(clip.left().max(grid.origin.x)).max(0.0) as u32;
    let last = grid.tick_at(clip.right()) as u32 + 1;
    let (top, bottom) = (grid.origin.y, rect.bottom());
    let first_bar = meter.bar(first).index;
    let last_bar = meter.bar(last).index;
    // Zoomed far in, each bar is its own group.
    let bar_px = grid.px_per_tick * meter.bar(first).len as f32;
    let group = if bar_px > WIDE_BAR { 1 } else { BAR_GROUP };
    let shaded =
        (first_bar..=last_bar).filter(|b| view.shading && (b / group) % 2 == 1 && b % group == 0);
    for bar in shaded {
        let x0 = grid.x(meter.bar_start(bar) as f32);
        let x1 = grid.x(meter.bar_start(bar + group) as f32);
        painter.rect_filled(
            Rect::from_min_max(pos2(x0, top), pos2(x1, bottom)),
            0.0,
            theme::BAR_GROUP,
        );
    }
    let step = PPQ / 4;
    if grid.px_per_tick * step as f32 >= 6.0 {
        let mut tick = first - first % step;
        while tick <= last {
            let x = grid.x(tick as f32);
            painter.line_segment(
                [pos2(x, top), pos2(x, bottom)],
                Stroke::new(1.0, theme::GRID_LINE),
            );
            tick += step;
        }
    }
    let beat_px = grid.px_per_tick * PPQ as f32;
    for beat in meter.beats(first, last) {
        let color = if !beat.downbeat {
            if beat_px < 5.0 {
                continue;
            }
            theme::GRID_BEAT
        } else if beat.bar % group == 0 {
            theme::GRID_GROUP
        } else {
            theme::GRID_BAR
        };
        let x = grid.x(beat.tick as f32);
        painter.line_segment([pos2(x, top), pos2(x, bottom)], Stroke::new(1.0, color));
    }
    let first_row = grid.row_at(clip.top().max(top)).unwrap_or(0);
    let last_row = grid
        .row_at(clip.bottom() - 1.0)
        .unwrap_or(PLAYLIST_TRACKS - 1);
    for row in (first_row..=last_row).filter(|_| view.separators) {
        let y = grid.y(row + 1) - 0.5;
        painter.line_segment(
            [pos2(grid.origin.x, y), pos2(rect.right(), y)],
            Stroke::new(1.0, theme::GRID_LINE),
        );
    }
}

/// How a clip is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Look {
    Normal,
    Selected,
    /// Being moved or painted.
    Ghost,
}

/// A clip at `(track, start, length)`: a title strip in its colour with the
/// kind glyph and name, over a dark body tinted toward that colour that
/// holds the content.
fn draw_clip(
    painter: &egui::Painter,
    grid: &Grid,
    frame: &Frame<'_>,
    clip: &Clip,
    (track, start, length): (u32, u32, u32),
    look: Look,
) {
    let project = frame.project;
    let (color, name, glyph) = clip_identity(project, clip);
    let rect = grid.span(track, start, length).shrink2(vec2(0.5, 1.0));
    let muted = clip.muted || !project.playlist.track_audible(track);
    let strength = match look {
        Look::Ghost => 0.55,
        _ if muted => 0.3,
        _ => 1.0,
    };
    let faded = |c: Color32| theme::at_strength(c, (f32::from(c.a()) * strength) as u8);
    let selected = look == Look::Selected;
    let body_fill = body_fill(clip, color);
    let title_h = TITLE_HEIGHT.min(rect.height());
    let title = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + title_h));
    let has_body = rect.height() >= title_h + 6.0;
    if has_body {
        painter.rect_filled(rect, CLIP_RADIUS, faded(body_fill));
    }
    let strip = if selected {
        theme::CLIP_SELECTED
    } else {
        color
    };
    let strip_radius = if has_body {
        egui::CornerRadius {
            nw: CLIP_RADIUS as u8,
            ne: CLIP_RADIUS as u8,
            sw: 0,
            se: 0,
        }
    } else {
        egui::CornerRadius::same(CLIP_RADIUS as u8)
    };
    painter.rect_filled(title, strip_radius, faded(strip));
    let edge = if selected {
        theme::CLIP_SELECTED
    } else {
        theme::mix(theme::mix(theme::LANE, color, 0.22), theme::BACKGROUND, 0.5)
    };
    painter.rect_stroke(
        rect,
        CLIP_RADIUS,
        Stroke::new(1.0, faded(edge)),
        egui::StrokeKind::Inside,
    );
    if has_body {
        let body = body_of(rect, title);
        clip_content(
            painter,
            grid,
            frame,
            clip,
            (start, length),
            body,
            &faded,
            color,
        );
    }
    let text = faded(theme::text_on(strip));
    let strip_painter = painter.with_clip_rect(title.intersect(painter.clip_rect()));
    // The glyph and name stay in view when the clip starts left of the
    // lanes (scrolled past), up to the clip's right end.
    let glyph_rect = glyph_rect(rect, painter.clip_rect());
    if painter
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| glyph_rect.contains(p) && painter.clip_rect().contains(p))
    {
        // The glyph opens the clip's menu, so it lights up under the pointer.
        strip_painter.rect_filled(glyph_rect.shrink(1.0), 2.0, text.gamma_multiply(0.25));
    }
    icons::paint(&strip_painter, glyph_rect, glyph, text);
    strip_painter.text(
        pos2(glyph_rect.right() + 1.0, title.center().y),
        Align2::LEFT_CENTER,
        name,
        FontId::proportional(theme::FONT_CAPTION + 0.5),
        text,
    );
}

/// A clip's body fill: darker for automation clips, so the curve stands out.
fn body_fill(clip: &Clip, color: Color32) -> Color32 {
    let body = theme::clip_body(color);
    match clip.source {
        ClipSource::Automation(_) => theme::mix(body, theme::BACKGROUND, 0.35),
        _ => body,
    }
}

/// The content area of a clip drawn at `rect` with title strip `title`.
fn body_of(rect: Rect, title: Rect) -> Rect {
    Rect::from_min_max(
        pos2(rect.left() + 2.0, title.bottom() + 1.0),
        pos2(rect.right() - 2.0, rect.bottom() - 2.0),
    )
}

/// The body of `clip` as drawn, when it is tall enough to have one.
pub(crate) fn clip_body_rect(grid: &Grid, clip: &Clip) -> Option<Rect> {
    let rect = grid
        .span(clip.track, clip.start, clip.length)
        .shrink2(vec2(0.5, 1.0));
    let title_h = TITLE_HEIGHT.min(rect.height());
    let title = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + title_h));
    (rect.height() >= title_h + 6.0).then(|| body_of(rect, title))
}

/// Point gestures inside automation clips with the Draw or Select tool
/// (`docs/spec/playlist.md`, "Automation clips in the playlist"): the
/// hovered clip shows its points and handles, and gestures on them edit
/// the automation. Returns whether the curve took the pointer.
fn clip_curve(
    ui: &egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    grid: &Grid,
    response: &egui::Response,
    view: Rect,
    actions: &mut Vec<Action>,
) -> bool {
    if !matches!(state.playlist.tool, Tool::Draw | Tool::Select) || state.playlist.drag.is_some() {
        return false;
    }
    let project = frame.project;
    let lanes = Rect::from_min_max(
        pos2(view.left() + HEADER_WIDTH, view.top() + RULER_HEIGHT),
        view.max,
    );
    let menu_id = response.id.with("point-menu");
    let holding = state.clip_curve.busy()
        || egui::Popup::is_id_open(ui.ctx(), menu_id)
        || state.clip_curve.typing();
    let held = state
        .clip_curve
        .clip
        .filter(|_| holding)
        .and_then(|(track, start)| {
            project
                .playlist
                .clips
                .iter()
                .find(|c| c.track == track && c.start == start)
        });
    // Otherwise the clip under the press, or under the pointer.
    let at = ui
        .input(|i| {
            i.pointer
                .press_origin()
                .filter(|_| i.pointer.any_down() || i.pointer.any_released())
        })
        .or_else(|| response.hover_pos())
        .filter(|p| lanes.contains(*p));
    let found = held.or_else(|| {
        let at = at?;
        project.playlist.clips.iter().rev().find(|c| {
            c.automation().is_some()
                && clip_body_rect(grid, c).is_some_and(|b| b.expand2(vec2(0.0, 2.0)).contains(at))
        })
    });
    let Some(clip) = found.copied() else {
        state.clip_curve.clip = None;
        return false;
    };
    let (Some(id), Some(body)) = (clip.automation(), clip_body_rect(grid, &clip)) else {
        return false;
    };
    let Some(index) = project.automation_position(id) else {
        return false;
    };
    let automation = &project.automations[index];
    let curve_view = crate::curve::View {
        rect: body,
        first: clip.offset as f32 + (body.left() - grid.x(clip.start as f32)) / grid.px_per_tick,
        px_per_tick: grid.px_per_tick,
    };
    let meter = Meter::new(project.time_signature);
    let edit = &mut state.clip_curve;
    edit.follow(id);
    let options = crate::curve::Options {
        snap: true,
        step: false,
        slide: false,
        in_clip: true,
    };
    let took = crate::curve::interact(
        ui,
        response,
        &curve_view,
        (index + 1, automation),
        &meter,
        options,
        edit,
        actions,
    );
    crate::curve::menus(
        ui,
        response,
        &curve_view,
        (index + 1, automation),
        edit,
        actions,
    );
    edit.clip = Some((clip.track, clip.start));
    // The shown curve with its points, over the drawn clip.
    let painter = ui.painter_at(lanes).with_clip_rect(
        body.expand(crate::curve::POINT_RADIUS + 2.0)
            .intersect(lanes),
    );
    let (color, _, _) = clip_identity(project, &clip);
    let shown = crate::curve::shown(automation, edit);
    if edit.busy() {
        painter.rect_filled(body, 0.0, body_fill(&clip, color));
    }
    let look = crate::curve::Look {
        line: theme::clip_content(color),
        fill: theme::at_strength(color, 60),
        points: true,
    };
    crate::curve::draw(&painter, &curve_view, &shown, edit, look);
    if let Some(pos) = response
        .hover_pos()
        .or_else(|| response.interact_pointer_pos())
    {
        let text = crate::curve::describe(&curve_view, &shown, edit, &meter, pos);
        crate::curve::readout(&painter, edit, text.as_deref(), pos);
        if let Some(text) = text {
            let _ = response.clone().hint(text);
        }
    }
    took
}

/// Where a clip's kind glyph sits in its title strip (`rect` is the drawn
/// clip, `view` the visible lanes). It opens the clip's menu.
fn glyph_rect(rect: Rect, view: Rect) -> Rect {
    let title_h = TITLE_HEIGHT.min(rect.height());
    // The glyph and name stay in view when the clip starts left of the
    // lanes (scrolled past), up to the clip's right end.
    let lanes_left = view.left() + HEADER_WIDTH;
    let left = (rect.left() + 1.0)
        .max(lanes_left + 1.0)
        .min((rect.right() - title_h - 24.0).max(rect.left() + 1.0));
    Rect::from_min_size(pos2(left, rect.top()), vec2(title_h, title_h))
}

/// The clip under `pos` whose kind glyph `pos` is on.
fn glyph_hit(clips: &[Clip], grid: &Grid, view: Rect, pos: Pos2) -> Option<Clip> {
    if pos.x <= view.left() + HEADER_WIDTH || pos.y <= view.top() + RULER_HEIGHT {
        return None;
    }
    clips.iter().rev().copied().find(|c| {
        let rect = grid
            .span(c.track, c.start, c.length)
            .shrink2(vec2(0.5, 1.0));
        glyph_rect(rect, view).contains(pos)
    })
}

/// A clip's colour, name and kind glyph.
fn clip_identity<'a>(project: &'a Project, clip: &Clip) -> (Color32, &'a str, Icon) {
    match clip.source {
        ClipSource::Pattern(id) => {
            let pattern = project
                .pattern_position(id)
                .and_then(|i| project.patterns.get(i));
            (
                pattern
                    .and_then(crate::pattern_color)
                    .unwrap_or(UNCOLOURED_CLIP),
                pattern.map_or("", |p| p.name.as_str()),
                Icon::Pattern,
            )
        }
        ClipSource::Automation(id) => (
            theme::AUTOMATION,
            project
                .automation_position(id)
                .map_or("", |i| project.automations[i].name.as_str()),
            Icon::Automation,
        ),
        ClipSource::Audio(id) => {
            let index = project.channel_position(id).unwrap_or(0);
            (
                crate::channel_color(project, index),
                project.channels.get(index).map_or("", |c| c.name.as_str()),
                Icon::Wave,
            )
        }
    }
}

/// The content in a clip's body: notes, the automation curve with a faint
/// fill under it, or the waveform in a light tint of the clip colour.
#[allow(clippy::too_many_arguments)]
fn clip_content(
    painter: &egui::Painter,
    grid: &Grid,
    frame: &Frame<'_>,
    clip: &Clip,
    (start, length): (u32, u32),
    body: Rect,
    faded: &dyn Fn(Color32) -> Color32,
    color: Color32,
) {
    let project = frame.project;
    match clip.source {
        ClipSource::Pattern(id) => {
            if let Some(pattern) = project
                .pattern_position(id)
                .and_then(|i| project.patterns.get(i))
            {
                pattern_preview(painter, grid, project, pattern, body, (start, length));
            }
        }
        ClipSource::Automation(id) => {
            let Some(automation) = project
                .automation_position(id)
                .map(|i| &project.automations[i])
            else {
                return;
            };
            // A switched-off automation shows its curve dimmed by half.
            let dim = |c: Color32| {
                let c = faded(c);
                if automation.enabled {
                    c
                } else {
                    c.gamma_multiply(0.5)
                }
            };
            let view = crate::curve::View {
                rect: body,
                first: clip.offset as f32 + (body.left() - grid.x(start as f32)) / grid.px_per_tick,
                px_per_tick: grid.px_per_tick,
            };
            let look = crate::curve::Look {
                line: dim(theme::clip_content(color)),
                fill: dim(theme::at_strength(color, 60)),
                points: false,
            };
            crate::curve::draw(
                &painter.with_clip_rect(body.intersect(painter.clip_rect())),
                &view,
                automation,
                &crate::curve::Edit::default(),
                look,
            );
        }
        ClipSource::Audio(id) => {
            let Some((_, peaks)) = frame.clip_waveforms.iter().find(|(c, _)| *c == id) else {
                return;
            };
            let n = peaks.len().max(1);
            let half = body.height() / 2.0;
            let peak = peaks
                .iter()
                .fold(1e-6_f32, |p, (lo, hi)| p.max(lo.abs()).max(hi.abs()));
            // The waveform is drawn at the sample's real duration.
            let duration = frame
                .clip_durations
                .iter()
                .find(|(c, _)| *c == id)
                .map_or(length, |(_, d)| *d);
            let stroke = Stroke::new(1.0, faded(theme::clip_content(color)));
            for (i, (lo, hi)) in peaks.iter().enumerate() {
                let tick = i as f32 / n as f32 * duration as f32;
                if tick >= length as f32 {
                    break;
                }
                let x = grid.x(start as f32 + tick);
                painter.line_segment(
                    [
                        pos2(x, body.center().y - hi / peak * half),
                        pos2(x, body.center().y - lo / peak * half),
                    ],
                    stroke,
                );
            }
        }
    }
}

/// Height of a clip's title strip.
const TITLE_HEIGHT: f32 = 11.0;

/// Corner radius of clips: barely rounded.
const CLIP_RADIUS: f32 = 2.0;

/// Fill of a clip whose pattern has no colour of its own.
const UNCOLOURED_CLIP: Color32 = theme::CLIP_NEUTRAL;

/// How a preview places a pattern's notes vertically.
enum NoteRows {
    /// By key, from the lowest (bottom) to the highest.
    Keys { lo: u8, span: f32 },
    /// Every note has one key and several channels play: one row per
    /// channel, these channel positions in rack order from the top.
    Channels(Vec<usize>),
    /// Every note on one line.
    Single,
}

impl NoteRows {
    fn of(project: &Project, pattern: &musiccraft_project::Pattern) -> Self {
        let (lo, hi) = pattern
            .notes
            .iter()
            .fold((127u8, 0u8), |(lo, hi), n| (lo.min(n.key), hi.max(n.key)));
        if lo < hi {
            return Self::Keys {
                lo,
                span: f32::from(hi - lo),
            };
        }
        let mut channels: Vec<usize> = pattern
            .notes
            .iter()
            .filter_map(|n| project.channel_position(n.channel))
            .collect();
        channels.sort_unstable();
        channels.dedup();
        if channels.len() > 1 {
            Self::Channels(channels)
        } else {
            Self::Single
        }
    }

    /// The height of `note`'s line inside `body`.
    fn y(&self, project: &Project, note: &musiccraft_project::Note, body: Rect) -> f32 {
        match self {
            Self::Keys { lo, span } => {
                body.bottom()
                    - (f32::from(note.key.saturating_sub(*lo)) / span) * (body.height() - 2.0)
                    - 1.0
            }
            Self::Channels(rows) => {
                let row = project
                    .channel_position(note.channel)
                    .and_then(|c| rows.iter().position(|r| *r == c))
                    .unwrap_or(0);
                let pitch = body.height() / rows.len() as f32;
                body.top() + pitch * (row as f32 + 0.5)
            }
            Self::Single => body.center().y,
        }
    }
}

/// A note's colour, as in the piano roll: its colour group, or its channel's
/// colour for group "channel".
fn note_tint(project: &Project, note: &musiccraft_project::Note) -> Color32 {
    if note.props.color > 0 {
        theme::note_color(note.props.color)
    } else {
        crate::channel_color(project, project.channel_position(note.channel).unwrap_or(0))
    }
}

/// One note of a preview, from `x0` to `x1` at height `y`.
fn note_mark(painter: &egui::Painter, x0: f32, x1: f32, y: f32, tint: Color32) {
    // A dark edge keeps notes readable on a coloured clip.
    painter.line_segment(
        [pos2(x0, y), pos2(x1, y)],
        Stroke::new(3.0, theme::shade(110)),
    );
    painter.line_segment([pos2(x0, y), pos2(x1, y)], Stroke::new(1.5, tint));
}

/// The notes of `pattern` repeated across a clip at `start` lasting `length`,
/// each in its own colour.
fn pattern_preview(
    painter: &egui::Painter,
    grid: &Grid,
    project: &Project,
    pattern: &musiccraft_project::Pattern,
    body: Rect,
    (start, length): (u32, u32),
) {
    let period = pattern.length_ticks().max(1);
    let rows = NoteRows::of(project, pattern);
    // A short tick at the top and bottom of the body where each repeat of
    // the pattern starts, when repeats are wide enough to tell apart.
    if grid.x(period as f32) - grid.x(0.0) >= 8.0 {
        let tick = Stroke::new(1.0, theme::at_strength(theme::TEXT_DIM, 150));
        for repeat in (period..length).step_by(period as usize) {
            let x = grid.x((start + repeat) as f32);
            painter.line_segment([pos2(x, body.top()), pos2(x, body.top() + 4.0)], tick);
            painter.line_segment([pos2(x, body.bottom() - 4.0), pos2(x, body.bottom())], tick);
        }
    }
    let mut offset = 0;
    while offset < length {
        for note in &pattern.notes {
            let s = offset + note.start;
            if s >= length {
                continue;
            }
            let x0 = grid.x((start + s) as f32);
            let x1 = grid
                .x((start + (s + note.length).min(length)) as f32)
                .max(x0 + 1.5);
            let y = rows.y(project, note, body);
            note_mark(painter, x0, x1, y, note_tint(project, note));
        }
        offset += period;
    }
}

/// The automation recorded into `pattern`, as faint curves under its notes.
fn lane_thumbnail(painter: &egui::Painter, pattern: &musiccraft_project::Pattern, body: Rect) {
    let length = pattern.length_ticks().max(1) as f32;
    let color = theme::AUTOMATION.gamma_multiply(0.6);
    for lane in &pattern.automation {
        let points: Vec<Pos2> = lane
            .points
            .iter()
            .map(|p| {
                pos2(
                    body.left() + (p.tick as f32 / length).min(1.0) * body.width(),
                    body.bottom() - p.value.clamp(0.0, 1.0) * body.height(),
                )
            })
            .collect();
        match points[..] {
            [] => {}
            [only] => {
                painter.line_segment(
                    [pos2(body.left(), only.y), pos2(body.right(), only.y)],
                    Stroke::new(1.0, color),
                );
            }
            _ => {
                painter.add(egui::Shape::line(points, Stroke::new(1.0, color)));
            }
        }
    }
}

/// One pass of `pattern`'s notes fitted into `body`, for the picker.
fn pattern_thumbnail(
    painter: &egui::Painter,
    project: &Project,
    pattern: &musiccraft_project::Pattern,
    body: Rect,
) {
    let length = pattern.length_ticks().max(1) as f32;
    let x = |tick: u32| body.left() + (tick as f32 / length).min(1.0) * body.width();
    let rows = NoteRows::of(project, pattern);
    for note in &pattern.notes {
        let x0 = x(note.start);
        let x1 = x(note.start.saturating_add(note.length)).max(x0 + 1.5);
        let y = rows.y(project, note, body);
        note_mark(painter, x0, x1, y, note_tint(project, note));
    }
}

/// The command that places a clip like `clip` at another position.
fn add_clip_invocation(project: &Project, clip: &Clip, track: u32, start: u32) -> Invocation {
    let (name, number) = match clip.source {
        ClipSource::Pattern(id) => ("playlist.add_clip", project.pattern_position(id)),
        ClipSource::Automation(id) => (
            "playlist.add_automation_clip",
            project.automation_position(id),
        ),
        ClipSource::Audio(id) => ("playlist.add_audio_clip", project.channel_position(id)),
    };
    let mut args = vec![
        number.map_or(1, |i| i + 1).to_string(),
        (track + 1).to_string(),
        format!("{start}t"),
        format!("{}t", clip.length),
    ];
    if clip.offset > 0 {
        args.push(format!("{}t", clip.offset));
    }
    Invocation::new(name, args)
}

fn draw_ruler(
    painter: &egui::Painter,
    grid: &Grid,
    clip: Rect,
    project: &Project,
    dragging: Option<(u32, u32)>,
    meter: &Meter,
) {
    let ruler = Rect::from_min_max(
        pos2(clip.left(), clip.top()),
        pos2(clip.right(), clip.top() + RULER_HEIGHT),
    );
    painter.rect_filled(ruler, 0.0, theme::PANEL_DARK);
    let song_end = project.song_length_ticks();
    let end_x = grid.x(song_end as f32);
    if end_x > ruler.left() {
        painter.rect_filled(
            Rect::from_min_max(
                pos2(grid.origin.x.max(ruler.left()), ruler.bottom() - 3.0),
                pos2(end_x, ruler.bottom()),
            ),
            0.0,
            theme::ACCENT_DIM,
        );
    }
    let range = dragging
        .map(|(a, b)| (a.min(b), a.max(b)))
        .or(project.playlist.loop_range);
    if let Some((start, end)) = range {
        let band = Rect::from_min_max(
            pos2(
                grid.x(start as f32).max(ruler.left() + HEADER_WIDTH),
                ruler.top(),
            ),
            pos2(grid.x(end as f32), ruler.bottom()),
        );
        if band.width() > 0.0 {
            painter.rect_filled(band, 0.0, theme::at_strength(theme::HIGHLIGHT, 60));
            let shade = theme::at_strength(theme::HIGHLIGHT, 14);
            painter.rect_filled(
                Rect::from_min_max(
                    pos2(band.left(), ruler.bottom()),
                    pos2(band.right(), clip.bottom()),
                ),
                0.0,
                shade,
            );
            for x in [band.left(), band.right()] {
                painter.line_segment(
                    [pos2(x, ruler.top()), pos2(x, clip.bottom())],
                    Stroke::new(1.0, theme::HIGHLIGHT),
                );
            }
        }
    }
    let first = meter.bar(grid.tick_at(clip.left()) as u32);
    let last_bar = meter.bar(grid.tick_at(clip.right()) as u32).index + 1;
    let every = if grid.px_per_tick * first.len as f32 >= 28.0 {
        1
    } else {
        4
    };
    let lanes_left = ruler.left() + HEADER_WIDTH;
    for bar in first.index..=last_bar {
        let info = meter.bar(meter.bar_start(bar));
        let x = grid.x(info.start as f32);
        // Short beat ticks and a longer bar tick rise from the bottom edge.
        if grid.px_per_tick * info.sig.beat_ticks() as f32 >= 6.0 {
            let beat = info.sig.beat_ticks().max(1);
            for at in (info.start + beat..info.end()).step_by(beat as usize) {
                let bx = grid.x(at as f32);
                if bx > lanes_left {
                    painter.line_segment(
                        [pos2(bx, ruler.bottom() - 3.0), pos2(bx, ruler.bottom())],
                        Stroke::new(1.0, theme::GRID_BEAT),
                    );
                }
            }
        }
        if x > lanes_left {
            painter.line_segment(
                [pos2(x, ruler.bottom() - 6.0), pos2(x, ruler.bottom())],
                Stroke::new(1.0, theme::GRID_BAR),
            );
        }
        if bar % every != 0 {
            continue;
        }
        painter.text(
            pos2(x + 3.0, ruler.center().y),
            Align2::LEFT_CENTER,
            (bar + 1).to_string(),
            FontId::monospace(theme::FONT_LABEL),
            theme::TEXT,
        );
    }
    for marker in &project.playlist.markers {
        let x = grid.x(marker.tick as f32);
        if x < ruler.left() + HEADER_WIDTH || x > ruler.right() {
            continue;
        }
        let top = ruler.top();
        // Each action has its own glyph: a flag for a label, two bars for a
        // pause, a down-facing arrow for the song loop point, a bracket for a
        // time signature change.
        let color = match marker.action {
            MarkerAction::None => theme::AUTOMATION,
            MarkerAction::Pause => theme::DANGER,
            MarkerAction::SongLoop => theme::ACCENT,
            MarkerAction::TimeSignature(_) => theme::HIGHLIGHT,
        };
        match marker.action {
            MarkerAction::None => {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        pos2(x, top + 2.0),
                        pos2(x + 7.0, top + 6.0),
                        pos2(x, top + 10.0),
                    ],
                    color,
                    Stroke::NONE,
                ));
            }
            MarkerAction::Pause => {
                for dx in [0.0, 4.0] {
                    painter.rect_filled(
                        Rect::from_min_size(pos2(x + dx, top + 2.0), egui::vec2(2.5, 8.0)),
                        0.0,
                        color,
                    );
                }
            }
            MarkerAction::SongLoop => {
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        pos2(x - 1.0, top + 2.0),
                        pos2(x + 8.0, top + 2.0),
                        pos2(x + 3.5, top + 10.0),
                    ],
                    color,
                    Stroke::NONE,
                ));
            }
            MarkerAction::TimeSignature(_) => {
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, top + 2.0), egui::vec2(2.5, 9.0)),
                    0.0,
                    color,
                );
                painter.rect_filled(
                    Rect::from_min_size(pos2(x, top + 2.0), egui::vec2(7.0, 2.5)),
                    0.0,
                    color,
                );
            }
        }
        painter.line_segment(
            [pos2(x, top), pos2(x, clip.bottom())],
            Stroke::new(1.0, theme::at_strength(color, 70)),
        );
        painter.text(
            pos2(x + 10.0, top + 1.0),
            Align2::LEFT_TOP,
            &marker.name,
            FontId::proportional(theme::FONT_LABEL),
            color,
        );
    }
}

/// Clicking or dragging on the ruler moves the playhead.
/// Ruler gestures: click or drag to seek, Shift+drag to set the loop,
/// right-click for markers and the loop.
#[allow(clippy::too_many_lines)]
fn ruler(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    grid: &Grid,
    clip: Rect,
    actions: &mut Vec<Action>,
) {
    let snap = state.playlist.snap;
    let meter = state.playlist.meter.clone();
    let area = Rect::from_min_max(
        pos2(clip.left() + HEADER_WIDTH, clip.top()),
        pos2(clip.right(), clip.top() + RULER_HEIGHT),
    );
    let response = ui
        .interact(
            area,
            ui.id().with("playlist-ruler"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .hint(
            "Click to seek · Shift+drag to loop · drag a marker to move it, double-click to rename · right-click for markers",
        );
    let shift = ui.input(|i| i.modifiers.shift);
    let tick_at = |x: f32| grid.tick_at(x);
    let marker_at = |x: f32| {
        project.playlist.markers.iter().find(|m| {
            (grid.x(m.tick as f32) - x).abs() < 8.0
                || (x >= grid.x(m.tick as f32) && x < grid.x(m.tick as f32) + 60.0)
        })
    };
    if response.drag_started() && shift {
        if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
            let start = Snap::Bar.floor(tick_at(pos.x), &meter);
            state.playlist.loop_drag = Some((start, start));
        }
    }
    let moving = ui.id().with("marker-drag");
    if response.drag_started() && !shift {
        let press = ui.input(|i| i.pointer.press_origin());
        if let Some(marker) = press.and_then(|p| marker_at(p.x)).filter(|m| movable(m)) {
            ui.data_mut(|d| d.insert_temp(moving, (marker.tick, marker.tick)));
        }
    }
    let marker_drag = ui.data(|d| d.get_temp::<(u32, u32)>(moving));
    if response.double_clicked() {
        let pos = response.interact_pointer_pos();
        if let Some(marker) = pos.and_then(|p| marker_at(p.x)).filter(|m| movable(m)) {
            let x = grid.x(marker.tick as f32);
            let field = Rect::from_min_size(pos2(x, area.top()), vec2(120.0, RULER_HEIGHT));
            state.rename_in_place(
                crate::dialogs::Dialog::rename_marker(marker.tick, &marker.name),
                field,
            );
        }
    }
    if let Some((from, to)) = marker_drag {
        let to = match response.interact_pointer_pos() {
            Some(pos) => snap.round(tick_at(pos.x).max(0.0), &meter),
            None => to,
        };
        let x = grid.x(to as f32);
        let painter = ui.painter_at(area);
        painter.line_segment(
            [pos2(x, area.top()), pos2(x, area.bottom())],
            egui::Stroke::new(2.0, theme::ACCENT),
        );
        if response.drag_stopped() {
            ui.data_mut(|d| d.remove::<(u32, u32)>(moving));
            if to != from {
                actions.push(Action::Run(Invocation::new(
                    "playlist.move_marker",
                    [format!("{from}t"), format!("{to}t")],
                )));
            }
        } else {
            ui.data_mut(|d| d.insert_temp(moving, (from, to)));
        }
    } else if let Some((start, end)) = &mut state.playlist.loop_drag {
        if let Some(pos) = response.interact_pointer_pos() {
            *end = Snap::Bar.round(tick_at(pos.x), &meter);
        }
        if response.drag_stopped() {
            let (a, b) = (*start.min(end), *start.max(end));
            if b > a {
                actions.push(Action::Run(Invocation::new(
                    "transport.set_loop",
                    [format!("{a}t"), format!("{b}t")],
                )));
            }
            state.playlist.loop_drag = None;
        }
    } else if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            let tick = match marker_at(pos.x).filter(|_| response.clicked()) {
                Some(marker) => marker.tick,
                None => snap
                    .floor(tick_at(pos.x), &meter)
                    .min(Snap::Beat.floor(tick_at(pos.x), &meter)),
            };
            actions.push(Action::Seek(f64::from(tick)));
        }
    }
    let press = response.interact_pointer_pos().or(response.hover_pos());
    response.context_menu(|ui| {
        let x = press.map_or(area.left(), |p| p.x);
        let tick = Snap::Bar.floor(tick_at(x), &meter);
        if ui
            .button("Add marker here…")
            .hint("Alt+T adds one at the playhead")
            .clicked()
        {
            state.open_dialog(crate::dialogs::Dialog::add_marker(tick));
            ui.close();
        }
        if ui
            .button("Add pause marker here")
            .hint("Song playback stops when it reaches this point")
            .clicked()
        {
            actions.push(Action::Run(Invocation::new(
                musiccraft_commands::BATCH,
                [
                    format!("playlist.add_marker {tick}t Pause"),
                    format!("playlist.set_marker_action {tick}t pause"),
                ],
            )));
            ui.close();
        }
        let here = meter.sig_at(tick);
        ui.menu_button("Time signature here…", |ui| {
            if tick == 0 {
                ui.label(
                    RichText::new("The start is the project signature (toolbar)")
                        .color(theme::TEXT_DIM),
                );
            }
            if let Some(sig) = signature::picker(ui, Some(here), &mut state.sig_draft) {
                actions.push(Action::Run(if tick == 0 {
                    Invocation::new("project.set_time_signature", [sig.to_string()])
                } else {
                    Invocation::new(
                        "playlist.set_time_signature",
                        [format!("{tick}t"), sig.to_string()],
                    )
                }));
            }
        })
        .response
        .hint("A new bar starts here in this signature");
        if let Some(marker) = marker_at(x) {
            ui.separator();
            ui.label(format!("Marker \"{}\"", marker.name));
            for action in MarkerAction::ALL {
                let (label, hint) = match action {
                    MarkerAction::None => ("Label only", "Only a label"),
                    MarkerAction::Pause => (
                        "Pause here",
                        "Song playback stops here; press play to continue",
                    ),
                    MarkerAction::SongLoop => (
                        "Loop the song here",
                        "At the end of the song, playback returns here instead of to the start",
                    ),
                    MarkerAction::TimeSignature(_) => continue,
                };
                if ui
                    .radio(marker.action == action, label)
                    .hint(hint)
                    .clicked()
                {
                    if marker.action != action {
                        actions.push(Action::Run(Invocation::new(
                            "playlist.set_marker_action",
                            [format!("{}t", marker.tick), action.name().to_owned()],
                        )));
                    }
                    ui.close();
                }
            }
            if ui
                .button(format!("Remove marker \"{}\"", marker.name))
                .clicked()
            {
                actions.push(Action::Run(Invocation::new(
                    "playlist.remove_marker",
                    [format!("{}t", marker.tick)],
                )));
                ui.close();
            }
        }
        if project.playlist.loop_range.is_some() && ui.button("Clear loop").clicked() {
            actions.push(Action::Run(Invocation::bare("transport.clear_loop")));
            ui.close();
        }
    });
}

fn headers(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    grid: &Grid,
    clip: Rect,
    actions: &mut Vec<Action>,
) {
    let area = Rect::from_min_max(
        pos2(clip.left(), clip.top() + RULER_HEIGHT),
        pos2(clip.left() + HEADER_WIDTH, clip.bottom()),
    );
    let painter = ui.painter_at(area);
    // The column is the well tone; each plate stops a hairline short of the
    // next, so the gap reads as a divider.
    painter.rect_filled(area, 0.0, theme::PANEL_DARK);
    let first = grid.row_at(area.top()).unwrap_or(0);
    let last = grid
        .row_at(area.bottom() - 1.0)
        .unwrap_or(PLAYLIST_TRACKS - 1);
    for row in first..=last {
        let r = Rect::from_min_max(
            pos2(area.left(), grid.y(row)),
            pos2(area.right(), grid.y(row + 1)),
        );
        let playlist = &project.playlist;
        let hovered = ui.rect_contains_pointer(r);
        let selected = state.playlist.track == row;
        // The selector strip sits between the plate and the lane.
        let plate = Rect::from_min_max(r.min, r.max - vec2(SELECTOR_WIDTH + 1.0, 1.0));
        painter.rect_filled(
            plate,
            2.0,
            plate_fill(playlist.track_colors.get(&row), hovered || selected),
        );
        if selected {
            let strip = Rect::from_min_max(
                pos2(r.right() - SELECTOR_WIDTH, r.top()),
                r.max - vec2(0.0, 1.0),
            );
            painter.rect_filled(strip, 1.0, theme::ACCENT);
        }
        let text = if playlist.track_audible(row) {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        };
        let layout = HeaderLayout::of(plate);
        painter.with_clip_rect(layout.name).text(
            layout.name.left_center(),
            Align2::LEFT_CENTER,
            playlist.track_name(row),
            FontId::proportional(theme::FONT_LABEL),
            text,
        );
        if let Some(grip) = layout.grip {
            draw_grip(&painter, grip, hovered);
        }
        let name_area = layout.name;
        let number = (row + 1).to_string();
        // Right-click the header: rename or colour the row.
        let header = ui.interact(r, ui.id().with(("track-header", row)), Sense::click());
        if header.clicked() || header.secondary_clicked() {
            state.playlist.track = row;
        }
        if header.double_clicked() {
            state.rename_in_place(
                crate::dialogs::Dialog::rename_playlist_track(row, &playlist.track_name(row)),
                name_area,
            );
        }
        header.context_menu(|ui| {
            if ui.button("Rename…").clicked() {
                state.open_dialog(crate::dialogs::Dialog::rename_playlist_track(
                    row,
                    &playlist.track_name(row),
                ));
                ui.close();
            }
            ui.menu_button("Colour", |ui| {
                let current = playlist.track_colors.get(&row).copied();
                if let Some(choice) = crate::swatches::grid(ui, current) {
                    actions.push(Action::Run(Invocation::new(
                        "playlist.set_track_color",
                        [number.clone(), crate::swatches::arg(choice)],
                    )));
                    ui.close();
                }
            });
            height_menu(ui, playlist, row, actions);
            ui.separator();
            track_items(ui, state, project, row, actions);
        });
        track_divider(
            ui,
            state,
            grid,
            (r, playlist.track_height(row)),
            row,
            actions,
        );
        if !area.contains_rect(layout.light) {
            continue;
        }
        track_switches(ui, playlist, row, (&layout, hovered), actions);
    }
}

/// A track header plate's fill: raised, tinted well down by the track's
/// colour, one step lighter when selected or under the pointer.
fn plate_fill(color: Option<&u8>, lifted: bool) -> egui::Color32 {
    let base = if lifted {
        theme::RAISED_HOVER
    } else {
        theme::RAISED
    };
    color.map_or(base, |&c| theme::mix(base, theme::track_color(c), 0.3))
}

/// Where things sit on a track header plate. A plate at least two lines
/// tall has the name on the top line and the grip and switches on the
/// bottom line; a shorter one puts everything on one line.
struct HeaderLayout {
    name: Rect,
    grip: Option<Rect>,
    arm: Rect,
    solo: Rect,
    light: Rect,
}

/// Width of the track selector strip at a header's right edge.
const SELECTOR_WIDTH: f32 = 5.0;

/// The shortest plate that gets two lines.
const TWO_LINE_HEADER: f32 = 34.0;

impl HeaderLayout {
    fn of(plate: Rect) -> Self {
        let two_lines = plate.height() >= TWO_LINE_HEADER;
        let controls_y = if two_lines {
            plate.bottom() - 10.0
        } else {
            plate.center().y
        };
        let at = |from_right: f32, width: f32| {
            Rect::from_center_size(
                pos2(plate.right() - from_right, controls_y),
                vec2(width, 14.0),
            )
        };
        let name = if two_lines {
            Rect::from_min_max(
                pos2(plate.left() + 8.0, plate.top() + 2.0),
                pos2(plate.right() - 8.0, plate.top() + 18.0),
            )
        } else {
            // The name stops short of the switches at the right.
            Rect::from_min_max(
                pos2(plate.left() + 8.0, plate.top()),
                pos2(plate.right() - 56.0, plate.bottom()),
            )
        };
        Self {
            name,
            grip: two_lines.then(|| {
                Rect::from_center_size(pos2(plate.left() + 12.0, controls_y), vec2(10.0, 6.0))
            }),
            arm: at(46.0, 14.0),
            solo: at(29.0, 16.0),
            light: at(12.0, 14.0),
        }
    }
}

/// Two short lines: the resize grip at a plate's bottom left.
fn draw_grip(painter: &egui::Painter, grip: Rect, hovered: bool) {
    let color = if hovered {
        theme::TEXT
    } else {
        theme::TEXT_DIM
    };
    for y in [grip.top() + 1.0, grip.bottom() - 1.0] {
        painter.line_segment(
            [pos2(grip.left(), y), pos2(grip.right(), y)],
            Stroke::new(1.0, color),
        );
    }
}

/// The arm dot, the solo button and the mute light of one track header.
fn track_switches(
    ui: &mut egui::Ui,
    playlist: &musiccraft_project::Playlist,
    row: u32,
    (layout, hovered): (&HeaderLayout, bool),
    actions: &mut Vec<Action>,
) {
    let number = (row + 1).to_string();
    let muted = playlist.muted_tracks.contains(&row);
    let soloed = playlist.soloed_tracks.contains(&row);
    let silenced = !muted && !playlist.track_audible(row);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(layout.light));
    let color = if silenced {
        theme::ACCENT_DIM
    } else {
        theme::ACCENT
    };
    let light = widgets::dot(&mut child, !muted, color);
    if soloed {
        ui.painter()
            .circle_stroke(light.rect.center(), 6.5, Stroke::new(1.0, theme::HIGHLIGHT));
    }
    let state = if muted {
        "muted"
    } else if soloed {
        "soloed"
    } else if silenced {
        "silenced by solo"
    } else {
        "on"
    };
    if light
        .hint(format!("Track {state} · click to mute or unmute"))
        .clicked()
    {
        actions.push(Action::Run(Invocation::new(
            "playlist.toggle_track_mute",
            [number.clone()],
        )));
    }
    // The arm dot shows while the track is armed or the pointer is over it.
    let armed = playlist.armed_tracks.contains(&row);
    if armed || hovered {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(layout.arm));
        if widgets::dot(&mut child, armed, theme::DANGER)
            .hint("Arm for audio recording: takes go to the first armed track with room")
            .clicked()
        {
            actions.push(Action::Run(Invocation::new(
                "playlist.toggle_track_arm",
                [number.clone()],
            )));
        }
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(layout.solo));
    let label = RichText::new("S").small().color(if soloed {
        theme::HIGHLIGHT
    } else {
        theme::TEXT_DIM
    });
    if child
        .add(egui::Button::new(label).frame(false))
        .hint("Solo track")
        .clicked()
    {
        actions.push(Action::Run(Invocation::new(
            "playlist.toggle_track_solo",
            [number],
        )));
    }
}

/// The track menu's naming and whole-track clip edits.
fn track_items(
    ui: &mut egui::Ui,
    state: &mut UiState,
    project: &Project,
    row: u32,
    actions: &mut Vec<Action>,
) {
    let on_track: Vec<Clip> = project
        .playlist
        .clips
        .iter()
        .filter(|c| c.track == row)
        .copied()
        .collect();
    menu_item(
        ui,
        actions,
        "Name after first clip",
        "Name and colour this track after its first pattern clip",
        name_after_first_clip(project, row, &on_track),
    );
    menu_item(
        ui,
        actions,
        "Reset name, colour and height",
        "Back to the default name, no colour and the global height",
        reset_track(&project.playlist, row),
    );
    ui.separator();
    let muted =
        |m: bool| -> Vec<Clip> { on_track.iter().filter(|c| c.muted == m).copied().collect() };
    menu_item(
        ui,
        actions,
        "Mute all clips",
        "Mute every clip on this track (the track light stays as it is)",
        per_clip(&muted(false), "playlist.toggle_clip_mute", &[]),
    );
    menu_item(
        ui,
        actions,
        "Unmute all clips",
        "Unmute every clip on this track",
        per_clip(&muted(true), "playlist.toggle_clip_mute", &[]),
    );
    if crate::widgets::add_enabled(
        ui,
        !on_track.is_empty(),
        egui::Button::new("Select all clips"),
    )
    .on_hover_text("Select every clip on this track")
    .clicked()
    {
        state.playlist.selection = on_track.iter().map(|c| (c.track, c.start)).collect();
        ui.close();
    }
    menu_item(
        ui,
        actions,
        "Join pattern clips into one",
        "Merge this track's pattern clips into one new pattern",
        merge(&on_track),
    );
    let renderable: Vec<Clip> = on_track
        .iter()
        .filter(|c| !matches!(c.source, ClipSource::Automation(_)))
        .copied()
        .collect();
    menu_item(
        ui,
        actions,
        "Render to audio",
        "Render this track's pattern and audio clips to WAV files and replace them with audio clips",
        per_clip(&renderable, "playlist.consolidate", &[]),
    );
}

/// One `sample.split_stems` per sampler channel that `clips` play, in order.
fn split_stems(project: &Project, clips: &[Clip]) -> Vec<Invocation> {
    let mut channels: Vec<usize> = Vec::new();
    for clip in clips {
        let index = clip.audio().and_then(|id| project.channel_position(id));
        let sampler = index.filter(|i| {
            matches!(
                project.channels[*i].instrument,
                musiccraft_project::Instrument::Sampler { .. }
            )
        });
        if let Some(i) = sampler.filter(|i| !channels.contains(i)) {
            channels.push(i);
        }
    }
    channels
        .into_iter()
        .map(|i| Invocation::new("sample.split_stems", [(i + 1).to_string()]))
        .collect()
}

/// Names and colours `row` after its first pattern clip's pattern.
fn name_after_first_clip(project: &Project, row: u32, on_track: &[Clip]) -> Option<Invocation> {
    let pattern = on_track.iter().find_map(|c| match c.source {
        ClipSource::Pattern(id) => project
            .pattern_position(id)
            .and_then(|i| project.patterns.get(i)),
        _ => None,
    })?;
    let number = (row + 1).to_string();
    let color = pattern
        .color
        .map_or_else(|| "none".to_owned(), |c| (c + 1).to_string());
    batch(vec![
        Invocation::new(
            "playlist.rename_track",
            [number.clone(), pattern.name.clone()],
        ),
        Invocation::new("playlist.set_track_color", [number, color]),
    ])
}

/// Returns `row` to its default name, no colour and the global height.
fn reset_track(playlist: &musiccraft_project::Playlist, row: u32) -> Option<Invocation> {
    let number = (row + 1).to_string();
    let mut steps = Vec::new();
    if playlist.track_names.contains_key(&row) {
        steps.push(Invocation::new("playlist.rename_track", [number.clone()]));
    }
    if playlist.track_colors.contains_key(&row) {
        steps.push(Invocation::new(
            "playlist.set_track_color",
            [number.clone(), "none".to_owned()],
        ));
    }
    if playlist.track_heights.contains_key(&row) {
        steps.push(Invocation::new(
            "playlist.set_track_height",
            [number, "reset".to_owned()],
        ));
    }
    batch(steps)
}

/// Heights a track's menu offers, in percent of the global track height.
const HEIGHT_CHOICES: [u16; 6] = [50, 75, 100, 150, 200, 300];

/// The track menu's Height submenu: this track's own height.
fn height_menu(
    ui: &mut egui::Ui,
    playlist: &musiccraft_project::Playlist,
    row: u32,
    actions: &mut Vec<Action>,
) {
    let current = playlist.track_height(row);
    ui.menu_button("Height", |ui| {
        for percent in HEIGHT_CHOICES {
            if ui
                .selectable_label(current == percent, format!("{percent}%"))
                .clicked()
            {
                actions.push(Action::Run(Invocation::new(
                    "playlist.set_track_height",
                    [(row + 1).to_string(), percent.to_string()],
                )));
                ui.close();
            }
        }
    });
}

/// The bottom edge of a track header: dragging it resizes that track alone,
/// and Alt+click returns it to the global height.
fn track_divider(
    ui: &mut egui::Ui,
    state: &mut UiState,
    grid: &Grid,
    (header, saved): (Rect, u16),
    row: u32,
    actions: &mut Vec<Action>,
) {
    let edge = Rect::from_min_max(
        pos2(header.left(), header.bottom() - 3.0),
        pos2(header.right(), header.bottom() + 2.0),
    );
    let response = ui
        .interact(
            edge,
            ui.id().with(("track-divider", row)),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeVertical);
    let list = &mut state.playlist;
    let origin = ui.input(|i| i.pointer.press_origin());
    if response.dragged()
        && let (Some(pointer), Some(origin)) = (response.interact_pointer_pos(), origin)
    {
        // The saved height plus the drag so far, in steps of 5%.
        let moved = (pointer.y - origin.y) / grid.row_height.max(1.0) * 100.0;
        let percent = ((f32::from(saved) + moved) / 5.0).round().max(0.0) as u16 * 5;
        let percent = percent.clamp(*TRACK_HEIGHT_PERCENT.start(), *TRACK_HEIGHT_PERCENT.end());
        list.resize = Some((row, percent));
    }
    let shown = match list.resize {
        Some((r, percent)) if r == row => percent,
        _ => (grid.height_of(row) / grid.row_height.max(1.0) * 100.0).round() as u16,
    };
    if response.drag_stopped()
        && let Some((track, percent)) = list.resize.take()
    {
        actions.push(Action::Run(Invocation::new(
            "playlist.set_track_height",
            [(track + 1).to_string(), percent.to_string()],
        )));
    }
    if response.clicked() && ui.input(|i| i.modifiers.alt) {
        actions.push(Action::Run(Invocation::new(
            "playlist.set_track_height",
            [(row + 1).to_string(), "reset".to_owned()],
        )));
    }
    response.hint(format!(
        "Track height {shown}% · drag to resize this track · Alt+click to reset"
    ));
}

fn moving_clips(state: &UiState, clips: &[Clip]) -> Vec<Clip> {
    let Some(drag) = state.playlist.drag else {
        return Vec::new();
    };
    let mut group: Vec<Clip> = clips
        .iter()
        .filter(|c| state.playlist.selection.contains(&(c.track, c.start)))
        .copied()
        .collect();
    if !group.contains(&drag.original) {
        group = vec![drag.original];
    }
    group
}

/// The (start, track, length) change of the drag in progress, clamped so no
/// moved clip leaves the playlist.
fn drag_delta(state: &UiState, group: &[Clip]) -> (i64, i64, i64) {
    let Some(drag) = state.playlist.drag else {
        return (0, 0, 0);
    };
    let o = drag.original;
    match drag.kind {
        DragKind::Move => {
            let (dt, dtrack) = clamp_delta(
                group,
                i64::from(drag.start) - i64::from(o.start),
                i64::from(drag.track) - i64::from(o.track),
            );
            (dt, dtrack, 0)
        }
        DragKind::Resize => (0, 0, i64::from(drag.length) - i64::from(o.length)),
        DragKind::Trim => {
            // No clip may start before its source or lose its whole length.
            let dt = i64::from(drag.start) - i64::from(o.start);
            let earliest = group
                .iter()
                .map(|c| -i64::from(c.offset))
                .max()
                .unwrap_or(0);
            let latest = group
                .iter()
                .map(|c| i64::from(c.length) - 1)
                .min()
                .unwrap_or(0);
            let dt = dt.clamp(earliest, latest.max(earliest));
            (dt, 0, -dt)
        }
    }
}

/// Whether a marker can be dragged or renamed on the ruler (time-signature
/// changes have their own command).
fn movable(marker: &musiccraft_project::Marker) -> bool {
    !matches!(marker.action, MarkerAction::TimeSignature(_))
}

/// Whether `pos` is on the left edge of a clip's `rect` (its trim handle).
fn on_left_edge(rect: Rect, pos: egui::Pos2) -> bool {
    pos.x - rect.left() <= crate::geometry::EDGE_GRAB.min(rect.width() / 3.0)
}

fn clamp_delta(group: &[Clip], dt: i64, dtrack: i64) -> (i64, i64) {
    let min_start = group.iter().map(|c| i64::from(c.start)).min().unwrap_or(0);
    let lo = group.iter().map(|c| i64::from(c.track)).min().unwrap_or(0);
    let hi = group.iter().map(|c| i64::from(c.track)).max().unwrap_or(0);
    (
        dt.max(-min_start),
        dtrack.clamp(-lo, i64::from(PLAYLIST_TRACKS) - 1 - hi),
    )
}

fn shifted(clip: &Clip, dt: i64, dtrack: i64) -> (u32, u32) {
    (
        u32::try_from(i64::from(clip.track) + dtrack).unwrap_or(0),
        u32::try_from(i64::from(clip.start) + dt).unwrap_or(0),
    )
}

/// `name <track> <start>t [extra…]` for each clip, as one undo step.
fn per_clip(clips: &[Clip], name: &str, extra: &[String]) -> Option<Invocation> {
    batch(
        clips
            .iter()
            .map(|c| {
                let mut args = vec![(c.track + 1).to_string(), format!("{}t", c.start)];
                args.extend(extra.iter().cloned());
                Invocation::new(name, args)
            })
            .collect(),
    )
}

/// Slices every clip that crosses `at` there.
pub(crate) fn slice_at(clips: &[Clip], at: u32) -> Option<Invocation> {
    batch(
        clips
            .iter()
            .filter(|c| c.start < at && at < c.end())
            .map(|c| {
                Invocation::new(
                    "playlist.slice_clip",
                    [(c.track + 1).to_string(), format!("{at}t")],
                )
            })
            .collect(),
    )
}

/// Splits every pattern clip that crosses `at` into two new patterns there.
pub(crate) fn split_at(clips: &[Clip], at: u32) -> Option<Invocation> {
    batch(
        clips
            .iter()
            .filter(|c| matches!(c.source, ClipSource::Pattern(_)))
            .filter(|c| c.start < at && at < c.end())
            .map(|c| {
                Invocation::new(
                    "playlist.split_clip",
                    [(c.track + 1).to_string(), format!("{at}t")],
                )
            })
            .collect(),
    )
}

/// Splits the patterns of the selected clips by channel, each pattern once.
fn split_by_channel(project: &Project, clips: &[Clip]) -> Option<Invocation> {
    let mut numbers: Vec<usize> = clips
        .iter()
        .filter_map(Clip::pattern)
        .filter_map(|id| project.pattern_position(id))
        .filter(|&i| splittable(&project.patterns[i]))
        .collect();
    numbers.sort_unstable();
    numbers.dedup();
    // Each split appends its patterns, so the numbers stay valid.
    batch(
        numbers
            .iter()
            .map(|i| Invocation::new("pattern.split_by_channel", [(i + 1).to_string()]))
            .collect(),
    )
}

/// Whether a pattern has notes on two or more channels.
pub(crate) fn splittable(pattern: &musiccraft_project::Pattern) -> bool {
    let mut channels = pattern.notes.iter().map(|n| n.channel);
    channels
        .next()
        .is_some_and(|first| channels.any(|c| c != first))
}

/// Merges the selected pattern clips (two or more) into one.
pub(crate) fn merge(clips: &[Clip]) -> Option<Invocation> {
    let patterns: Vec<&Clip> = clips
        .iter()
        .filter(|c| matches!(c.source, ClipSource::Pattern(_)))
        .collect();
    if patterns.len() < 2 {
        return None;
    }
    Some(Invocation::new(
        "playlist.merge_clips",
        patterns
            .iter()
            .flat_map(|c| [(c.track + 1).to_string(), format!("{}t", c.start)]),
    ))
}

/// A menu entry that runs `invocation`, greyed out without one.
fn menu_item(
    ui: &mut egui::Ui,
    actions: &mut Vec<Action>,
    label: &str,
    hint: &str,
    invocation: Option<Invocation>,
) {
    if crate::widgets::add_enabled(ui, invocation.is_some(), egui::Button::new(label))
        .on_hover_text(hint)
        .clicked()
    {
        actions.extend(invocation.map(Action::Run));
        ui.close();
    }
}

/// The Clip menu: edits for the selected clips.
fn clip_menu(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    let project = frame.project;
    let selected = selected_clips(state, project);
    let audio: Vec<Clip> = selected
        .iter()
        .filter(|c| matches!(c.source, ClipSource::Audio(_)))
        .copied()
        .collect();
    if selected.is_empty() {
        ui.label(
            RichText::new("Select clips first (Select tool or Ctrl+A)").color(theme::TEXT_DIM),
        );
    }
    let playhead = frame.engine.position_ticks.max(0.0) as u32;
    menu_item(
        ui,
        actions,
        "Slice at playhead",
        "Cut the selected clips where the playhead is (or use the slice tool)",
        slice_at(&selected, playhead),
    );
    menu_item(
        ui,
        actions,
        "Split at playhead into new patterns",
        "Cut the selected pattern clips where the playhead is; each piece gets a new pattern holding only what it plays (or Shift+click with the slice tool)",
        split_at(&selected, playhead),
    );
    menu_item(
        ui,
        actions,
        "Split by channel",
        "Make one pattern per channel for the selected clips' patterns, with their clips stacked on the tracks below",
        split_by_channel(project, &selected),
    );
    menu_item(
        ui,
        actions,
        "Make independent copy",
        "Give each selected clip its own copy of its pattern, automation or sample channel",
        per_clip(&selected, "playlist.make_unique", &[]),
    );
    menu_item(
        ui,
        actions,
        "Merge",
        "Merge the selected pattern clips into one new pattern",
        merge(&selected),
    );
    menu_item(
        ui,
        actions,
        "Consolidate (render to audio)",
        "Render each selected clip to a WAV file and replace it with an audio clip",
        per_clip(
            &selected
                .iter()
                .filter(|c| !matches!(c.source, ClipSource::Automation(_)))
                .copied()
                .collect::<Vec<_>>(),
            "playlist.consolidate",
            &[],
        ),
    );
    let splits = split_stems(project, &audio);
    if crate::widgets::add_enabled(ui, !splits.is_empty(), egui::Button::new("Split into stems"))
        .on_hover_text(
            "Split the selected audio clips' sounds into vocals, drums, bass and other, each a new channel with clips under the originals",
        )
        .clicked()
    {
        // One split per sound; the app runs them one after another.
        actions.extend(splits.into_iter().map(Action::Run));
        ui.close();
    }
    menu_item(
        ui,
        actions,
        "Reverse",
        "Play the selected audio clips backwards (again: forwards)",
        per_clip(&audio, "playlist.reverse_clip", &[]),
    );
    if let Some(first) = audio.first() {
        let channel = match first.source {
            ClipSource::Audio(id) => project.channel_position(id),
            _ => None,
        };
        if let Some(channel) = channel.and_then(|i| project.channels.get(i)) {
            for (label, which, param) in [
                ("Fade in", "in", musiccraft_project::params::inst::FADE_IN),
                (
                    "Fade out",
                    "out",
                    musiccraft_project::params::inst::FADE_OUT,
                ),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    let mut seconds = channel.params.get(param);
                    let drag = ui.add(
                        egui::DragValue::new(&mut seconds)
                            .range(0.0..=10.0)
                            .speed(0.01)
                            .suffix(" s"),
                    );
                    if drag.drag_started() || drag.gained_focus() {
                        state.begin_gesture();
                    }
                    if drag.changed() {
                        let fades = per_clip(
                            &audio,
                            "playlist.set_clip_fade",
                            &[which.to_owned(), format!("{seconds:.3}")],
                        );
                        actions.extend(fades.map(|i| Action::Coalesced(i, state.gesture())));
                    }
                });
            }
        }
    }
}

/// Insert time and Remove time, over the loop region.
fn time_items(ui: &mut egui::Ui, project: &Project, actions: &mut Vec<Action>) {
    let space = |name: &str| {
        project.playlist.loop_range.map(|(start, end)| {
            Invocation::new(name, [format!("{start}t"), format!("{}t", end - start)])
        })
    };
    menu_item(
        ui,
        actions,
        "Insert time (Ctrl+Ins)",
        "Insert empty time the size of the loop region at its start (Shift+drag the ruler)",
        space("playlist.insert_space"),
    );
    menu_item(
        ui,
        actions,
        "Remove time (Ctrl+Del)",
        "Remove the time of the loop region; later clips and markers move earlier",
        space("playlist.delete_space"),
    );
}

fn batch(invocations: Vec<Invocation>) -> Option<Invocation> {
    match invocations.len() {
        0 => None,
        1 => invocations.into_iter().next(),
        _ => Some(Invocation::new(
            musiccraft_commands::BATCH,
            invocations.iter().map(ToString::to_string),
        )),
    }
}

/// Moves clips as one undo step, ordered so no clip lands on one not yet moved.
fn move_clips(group: &[Clip], dt: i64, dtrack: i64) -> Option<Invocation> {
    if dt == 0 && dtrack == 0 {
        return None;
    }
    let mut ordered = group.to_vec();
    ordered.sort_by_key(|c| (c.start, c.track));
    if dt > 0 || (dt == 0 && dtrack > 0) {
        ordered.reverse();
    }
    batch(
        ordered
            .iter()
            .map(|c| {
                let (track, start) = shifted(c, dt, dtrack);
                Invocation::new(
                    "playlist.move_clip",
                    [
                        (c.track + 1).to_string(),
                        format!("{}t", c.start),
                        (track + 1).to_string(),
                        format!("{start}t"),
                    ],
                )
            })
            .collect(),
    )
}

/// Paint and delete strokes. Returns `true` when the tool handled the input.
#[allow(clippy::too_many_arguments)]
fn tool_stroke(
    ui: &egui::Ui,
    state: &mut UiState,
    project: &Project,
    grid: &Grid,
    response: &egui::Response,
    boxes: &[(usize, Rect)],
    editable: &dyn Fn(Pos2) -> bool,
    actions: &mut Vec<Action>,
) -> bool {
    let clips = &project.playlist.clips;
    let under = |p: Pos2| match hit_test(boxes.iter().copied(), p) {
        Hit::Body(i) | Hit::Edge(i) => clips.get(i).copied(),
        Hit::Empty => None,
    };
    let pointer = response.interact_pointer_pos().filter(|p| editable(*p));
    match state.playlist.tool {
        Tool::Delete => {
            if response.drag_started_by(PointerButton::Primary) {
                state.playlist.erased.clear();
            }
            let pressed = response.dragged_by(PointerButton::Primary)
                || response.clicked_by(PointerButton::Primary);
            if let Some(c) = pointer.filter(|_| pressed).and_then(under) {
                if !state.playlist.erased.contains(&c) {
                    state.playlist.erased.push(c);
                }
            }
            if response.drag_stopped_by(PointerButton::Primary)
                || response.clicked_by(PointerButton::Primary)
            {
                let erased = std::mem::take(&mut state.playlist.erased);
                for c in &erased {
                    state.playlist.selection.remove(&(c.track, c.start));
                }
                let removals = erased
                    .iter()
                    .map(|c| {
                        Invocation::new(
                            "playlist.remove_clip",
                            [(c.track + 1).to_string(), format!("{}t", c.start)],
                        )
                    })
                    .collect();
                actions.extend(batch(removals).map(Action::Run));
            }
            true
        }
        Tool::Paint => {
            if state.playlist.drag.is_some() {
                return false;
            }
            if response.drag_started_by(PointerButton::Primary) {
                state.playlist.painted.clear();
                let origin = ui.input(|i| i.pointer.press_origin());
                state.playlist.paint_row = None;
                if origin.and_then(under).is_some() {
                    return false;
                }
                state.playlist.paint_row = origin.and_then(|p| grid.row_at(p.y));
            }
            let painting = response.dragged_by(PointerButton::Primary)
                || response.drag_stopped_by(PointerButton::Primary);
            let origin = response
                .drag_started_by(PointerButton::Primary)
                .then(|| ui.input(|i| i.pointer.press_origin()))
                .flatten()
                .filter(|p| editable(*p));
            let length = project.current_pattern().length_ticks().max(1);
            for p in origin.into_iter().chain(pointer.filter(|_| painting)) {
                // A stroke stays on the track it started on, however the
                // pointer wanders up or down.
                let Some(track) = state.playlist.paint_row else {
                    continue;
                };
                let start = state
                    .playlist
                    .snap
                    .floor(grid.tick_at(p.x), &state.playlist.meter);
                let after_last = state
                    .playlist
                    .painted
                    .last()
                    .is_none_or(|(_, s, l)| start >= s + l);
                let end = start + length;
                let free = clips
                    .iter()
                    .all(|c| c.track != track || c.end() <= start || c.start >= end);
                if after_last && free {
                    state.playlist.painted.push((track, start, length));
                }
            }
            if response.drag_stopped_by(PointerButton::Primary) {
                let pattern = (project.current_pattern + 1).to_string();
                let adds = std::mem::take(&mut state.playlist.painted)
                    .into_iter()
                    .map(|(track, start, _)| {
                        Invocation::new(
                            "playlist.add_clip",
                            [
                                pattern.clone(),
                                (track + 1).to_string(),
                                format!("{start}t"),
                            ],
                        )
                    })
                    .collect();
                actions.extend(batch(adds).map(Action::Run));
                return true;
            }
            !state.playlist.painted.is_empty()
        }
        Tool::Mute | Tool::Slice | Tool::Play | Tool::Zoom => {
            let Some(p) = pointer.filter(|_| response.clicked_by(PointerButton::Primary)) else {
                return true;
            };
            let tick = grid.tick_at(p.x);
            let clicked = under(p);
            match (state.playlist.tool, clicked) {
                (Tool::Mute, Some(c)) => actions.push(Action::Run(Invocation::new(
                    "playlist.toggle_clip_mute",
                    [(c.track + 1).to_string(), format!("{}t", c.start)],
                ))),
                (Tool::Slice, Some(c)) => {
                    let at = state.playlist.snap.round(tick, &state.playlist.meter);
                    let at = if at > c.start && at < c.end() {
                        at
                    } else {
                        tick.max(0.0) as u32
                    };
                    let split = ui.input(|i| i.modifiers.shift)
                        && matches!(c.source, ClipSource::Pattern(_));
                    let cut = if split { split_at } else { slice_at };
                    actions.extend(cut(&[c], at).map(Action::Run));
                }
                (Tool::Play, _) => {
                    let at = state.playlist.snap.floor(tick, &state.playlist.meter);
                    actions.push(Action::Seek(f64::from(at)));
                    actions.push(Action::Run(Invocation::bare("transport.play")));
                }
                (Tool::Zoom, _) => {
                    let factor = if ui.input(|i| i.modifiers.shift) {
                        1.0 / 1.5
                    } else {
                        1.5
                    };
                    state.playlist.px_per_bar = (state.playlist.px_per_bar * factor)
                        .clamp(BAR_WIDTH_RANGE.0, BAR_WIDTH_RANGE.1);
                }
                _ => {}
            }
            true
        }
        Tool::Draw | Tool::Select => false,
    }
}

/// Where a sound dropped at `pos` goes: (track, start tick, over a header).
/// Over a lane it lands at the snapped spot; over a track header, at the
/// start of the visible range on that track.
fn drop_target(
    grid: &Grid,
    clip: Rect,
    (snap, meter): (&Snap, &Meter),
    pos: Pos2,
) -> Option<(u32, u32, bool)> {
    if pos.y <= clip.top() + RULER_HEIGHT || !clip.contains(pos) {
        return None;
    }
    let row = grid.row_at(pos.y)?;
    let header = pos.x <= clip.left() + HEADER_WIDTH;
    let x = if header {
        clip.left() + HEADER_WIDTH
    } else {
        pos.x
    };
    let mut start = snap.floor(grid.tick_at(x), meter);
    if header && grid.x(start as f32) < x - 0.5 {
        // The visible range starts part-way into a snap step: take the next.
        start += snap.ticks_at(start, meter);
    }
    Some((row, start, header))
}

/// While a sound from the browser (or a pattern from the clip list) hovers
/// a lane or a track header: a ghost clip where it would land, one bar long
/// for a sound and the pattern's length for a pattern, and an accent
/// outline on that track's header.
fn drop_ghost(
    ui: &egui::Ui,
    project: &Project,
    grid: &Grid,
    clip: Rect,
    (snap, meter): (&Snap, &Meter),
    response: &egui::Response,
) {
    let Some(dragged) = egui::DragAndDrop::payload::<crate::browser::Dragged>(ui.ctx()) else {
        return;
    };
    let lanes = Rect::from_min_max(
        pos2(clip.left() + HEADER_WIDTH, clip.top() + RULER_HEIGHT),
        clip.max,
    );
    let Some((row, start, _)) = response
        .hover_pos()
        .and_then(|pos| drop_target(grid, clip, (snap, meter), pos))
    else {
        return;
    };
    let (name, length) = match &*dragged {
        crate::browser::Dragged::Pattern { id, label } => {
            let Some(pattern) = project.pattern_position(*id).map(|i| &project.patterns[i]) else {
                return;
            };
            (label.as_str(), pattern.length_ticks().max(1))
        }
        other => {
            let Some(name) = other.sample() else {
                return;
            };
            (name, meter.bar(start).len.max(1))
        }
    };
    let rect = grid.span(row, start, length).shrink2(vec2(0.0, 1.0));
    let painter = ui.painter_at(lanes);
    painter.rect(
        rect,
        2.0,
        theme::SELECTION_FILL,
        Stroke::new(1.0, theme::ACCENT),
        egui::StrokeKind::Inside,
    );
    let label = name.rsplit(['/', '\\']).next().unwrap_or(name);
    painter.text(
        rect.left_top() + vec2(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(theme::FONT_CAPTION),
        theme::TEXT_DIM,
    );
    let header = Rect::from_min_size(
        pos2(clip.left(), grid.y(row)),
        vec2(HEADER_WIDTH, grid.height_of(row)),
    );
    ui.painter_at(clip).rect_stroke(
        header.shrink(1.0),
        2.0,
        Stroke::new(1.0, theme::ACCENT),
        egui::StrokeKind::Inside,
    );
}

#[allow(clippy::too_many_arguments)]
fn edit(
    ui: &egui::Ui,
    state: &mut UiState,
    project: &Project,
    grid: &Grid,
    response: &egui::Response,
    clip: Rect,
    boxes: &[(usize, Rect)],
    actions: &mut Vec<Action>,
) {
    let editable = |p: Pos2| p.x > clip.left() + HEADER_WIDTH && p.y > clip.top() + RULER_HEIGHT;
    let snap = state.playlist.snap;
    let meter = state.playlist.meter.clone();
    let clips = &project.playlist.clips;
    let (ctrl, shift) = ui.input(|i| (i.modifiers.command, i.modifiers.shift));
    if tool_stroke(
        ui, state, project, grid, response, boxes, &editable, actions,
    ) {
        return;
    }
    let tool = state.playlist.tool;

    if response.drag_started_by(PointerButton::Primary) {
        if let Some(origin) = ui
            .input(|i| i.pointer.press_origin())
            .filter(|p| editable(*p))
        {
            let tick = grid.tick_at(origin.x);
            let hit = hit_test(boxes.iter().copied(), origin);
            if (ctrl || tool == Tool::Select) && hit == Hit::Empty {
                state.playlist.marquee = Some((origin, origin));
            }
            state.playlist.drag = match hit {
                Hit::Body(i) | Hit::Edge(i) => clips.get(i).map(|c| {
                    if !state.playlist.selection.contains(&(c.track, c.start)) {
                        state.playlist.selection = BTreeSet::from([(c.track, c.start)]);
                    }
                    let left = boxes
                        .iter()
                        .find(|(index, _)| matches!(hit, Hit::Body(i) if i == *index))
                        .is_some_and(|(_, rect)| on_left_edge(*rect, origin));
                    ClipDrag {
                        kind: if matches!(hit, Hit::Edge(_)) {
                            DragKind::Resize
                        } else if left {
                            DragKind::Trim
                        } else {
                            DragKind::Move
                        },
                        original: *c,
                        grab: tick - c.start as f32,
                        track: c.track,
                        start: c.start,
                        length: c.length,
                    }
                }),
                Hit::Empty => None,
            };
        }
    }
    if response.dragged_by(PointerButton::Primary) {
        let pos = response.interact_pointer_pos();
        if let (Some((_, current)), Some(pos)) = (&mut state.playlist.marquee, pos) {
            *current = pos;
        } else if let (Some(drag), Some(pos)) = (&mut state.playlist.drag, pos) {
            let tick = grid.tick_at(pos.x);
            match drag.kind {
                DragKind::Move => {
                    drag.start = snap.round(tick - drag.grab, &meter);
                    if let Some(row) = grid.row_at(pos.y) {
                        drag.track = row;
                    }
                }
                DragKind::Resize => {
                    let end = snap
                        .round(tick, &meter)
                        .max(drag.start + snap.ticks_at(drag.start, &meter));
                    drag.length = end - drag.start;
                }
                DragKind::Trim => {
                    let o = drag.original;
                    let start = snap.round(tick.max(0.0), &meter);
                    drag.start = start.clamp(o.start - o.offset.min(o.start), o.end() - 1);
                    drag.length = o.end() - drag.start;
                }
            }
        }
    }
    if response.drag_stopped_by(PointerButton::Primary) {
        if let Some((origin, current)) = state.playlist.marquee.take() {
            let area = Rect::from_two_pos(origin, current);
            if !shift {
                state.playlist.selection.clear();
            }
            for (i, rect) in boxes {
                if let (true, Some(c)) = (area.intersects(*rect), clips.get(*i)) {
                    state.playlist.selection.insert((c.track, c.start));
                }
            }
        } else {
            let group = moving_clips(state, clips);
            let (dt, dtrack, dl) = drag_delta(state, &group);
            if let Some(drag) = state.playlist.drag.take() {
                let invocation = match drag.kind {
                    DragKind::Move => move_clips(&group, dt, dtrack),
                    DragKind::Trim => batch(
                        group
                            .iter()
                            .filter(|_| dt != 0)
                            .map(|c| {
                                let start = u32::try_from(i64::from(c.start) + dt).unwrap_or(0);
                                Invocation::new(
                                    "playlist.trim_clip",
                                    [
                                        (c.track + 1).to_string(),
                                        format!("{}t", c.start),
                                        format!("{start}t"),
                                    ],
                                )
                            })
                            .collect(),
                    ),
                    DragKind::Resize => batch(
                        group
                            .iter()
                            .filter(|_| dl != 0)
                            .map(|c| {
                                let length =
                                    u32::try_from((i64::from(c.length) + dl).max(1)).unwrap_or(1);
                                Invocation::new(
                                    "playlist.resize_clip",
                                    [
                                        (c.track + 1).to_string(),
                                        format!("{}t", c.start),
                                        format!("{length}t"),
                                    ],
                                )
                            })
                            .collect(),
                    ),
                };
                if let Some(invocation) = invocation {
                    actions.push(Action::Run(invocation));
                    state.playlist.selection =
                        group.iter().map(|c| shifted(c, dt, dtrack)).collect();
                }
            }
        }
    }
    if response.clicked_by(PointerButton::Primary) {
        if let Some(pos) = response.interact_pointer_pos().filter(|p| editable(*p)) {
            match (hit_test(boxes.iter().copied(), pos), grid.row_at(pos.y)) {
                (Hit::Body(i) | Hit::Edge(i), _) => {
                    if let Some(c) = clips.get(i) {
                        let id = (c.track, c.start);
                        if shift {
                            if !state.playlist.selection.remove(&id) {
                                state.playlist.selection.insert(id);
                            }
                        } else {
                            state.playlist.selection = BTreeSet::from([id]);
                        }
                    }
                }
                (Hit::Empty, _) if tool == Tool::Select => state.playlist.selection.clear(),
                (Hit::Empty, Some(row)) => {
                    let start = snap.floor(grid.tick_at(pos.x), &meter);
                    let painted = state
                        .playlist
                        .paint_automation
                        .and_then(|id| project.automation_position(id));
                    let audio = state.playlist.paint_audio.and_then(|id| {
                        // New audio clips are as long as the channel's others.
                        let length = clips
                            .iter()
                            .find(|c| c.source == ClipSource::Audio(id))
                            .map(|c| c.length)?;
                        Some((project.channel_position(id)?, length))
                    });
                    let invocation = match (audio, painted) {
                        (Some((channel, length)), _) => Invocation::new(
                            "playlist.add_audio_clip",
                            [
                                (channel + 1).to_string(),
                                (row + 1).to_string(),
                                format!("{start}t"),
                                format!("{length}t"),
                            ],
                        ),
                        (None, Some(i)) => Invocation::new(
                            "playlist.add_automation_clip",
                            [
                                (i + 1).to_string(),
                                (row + 1).to_string(),
                                format!("{start}t"),
                            ],
                        ),
                        (None, None) => Invocation::new(
                            "playlist.add_clip",
                            [
                                (project.current_pattern + 1).to_string(),
                                (row + 1).to_string(),
                                format!("{start}t"),
                            ],
                        ),
                    };
                    actions.push(Action::Run(invocation));
                    state.playlist.selection.clear();
                }
                _ => {}
            }
        }
    }
    if response.double_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Hit::Body(i) | Hit::Edge(i) = hit_test(boxes.iter().copied(), pos) {
                match clips.get(i).map(|c| c.source) {
                    Some(ClipSource::Automation(id)) => state.automation_editor = Some(id),
                    Some(ClipSource::Pattern(id)) => {
                        if let Some(index) = project.pattern_position(id) {
                            open_in_piano_roll(state, index, actions);
                        }
                    }
                    Some(ClipSource::Audio(id)) => {
                        if let Some(index) = project.channel_position(id) {
                            state.selected_channel = index;
                            state.windows.channel_settings = true;
                        }
                    }
                    None => {}
                }
            }
        }
    }
    drop_ghost(ui, project, grid, clip, (&snap, &meter), response);
    // Sounds dropped from the browser become audio clips where they land,
    // and patterns dragged from the clip list pattern clips.
    if response.contains_pointer() && ui.input(|i| i.pointer.any_released()) {
        let target = ui
            .ctx()
            .pointer_hover_pos()
            .and_then(|pos| drop_target(grid, clip, (&snap, &meter), pos));
        let dragged = target
            .and_then(|_| egui::DragAndDrop::take_payload::<crate::browser::Dragged>(ui.ctx()));
        if let (Some((track, start, _)), Some(crate::browser::Dragged::Pattern { id, .. })) =
            (target, dragged.as_deref())
            && let Some(index) = project.pattern_position(*id)
        {
            actions.push(Action::Run(Invocation::new(
                "playlist.add_clip",
                [
                    (index + 1).to_string(),
                    (track + 1).to_string(),
                    format!("{start}t"),
                ],
            )));
        }
        if let (Some((track, start, header)), Some(sample)) = (
            target,
            dragged.as_ref().and_then(|d| d.sample()).map(str::to_owned),
        ) {
            // A track dropped on by its header takes the sound's name if it
            // still has its default one.
            let named = project.playlist.track_names.contains_key(&track);
            let name = sample.rsplit(['/', '\\', ':']).next().unwrap_or(&sample);
            let name = name
                .rsplit_once('.')
                .map_or(name, |(stem, _)| stem)
                .to_owned();
            actions.push(Action::DropAudio {
                sample,
                track,
                start,
            });
            if header && !named {
                actions.push(Action::Run(Invocation::new(
                    "playlist.rename_track",
                    [(track + 1).to_string(), name],
                )));
            }
        }
    }
    if response.secondary_clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Hit::Body(i) | Hit::Edge(i) = hit_test(boxes.iter().copied(), pos) {
                if let Some(c) = clips.get(i) {
                    state.playlist.selection.remove(&(c.track, c.start));
                    actions.push(Action::Run(Invocation::new(
                        "playlist.remove_clip",
                        [(c.track + 1).to_string(), format!("{}t", c.start)],
                    )));
                }
            }
        }
    }
    if crate::chrome::focused(ui.ctx(), egui::Id::new("playlist-window")) {
        keys(ui.ctx(), state, project, actions);
    }
    if let Some(pos) = response.hover_pos().filter(|p| editable(*p)) {
        let hit = hit_test(boxes.iter().copied(), pos);
        let left = boxes
            .iter()
            .any(|(i, rect)| hit == Hit::Body(*i) && on_left_edge(*rect, pos));
        let cursor = match hit {
            Hit::Edge(_) => egui::CursorIcon::ResizeHorizontal,
            Hit::Body(_) if left => egui::CursorIcon::ResizeHorizontal,
            Hit::Body(_) => egui::CursorIcon::Grab,
            Hit::Empty if ctrl => egui::CursorIcon::Cell,
            Hit::Empty => egui::CursorIcon::Default,
        };
        ui.ctx().set_cursor_icon(cursor);
    }
}

/// Editing keys while the playlist is the focused window.
fn keys(ctx: &egui::Context, state: &mut UiState, project: &Project, actions: &mut Vec<Action>) {
    use egui::{Key, Modifiers};
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let clips = &project.playlist.clips;
    let selected = selected_clips(state, project);
    let pressed = |modifiers: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(modifiers, key));
    if pressed(Modifiers::COMMAND, Key::A) {
        state.playlist.selection = clips.iter().map(|c| (c.track, c.start)).collect();
    }
    // Esc clears a selection first; with none, it closes the window.
    if !state.playlist.selection.is_empty() && pressed(Modifiers::NONE, Key::Escape) {
        state.playlist.selection.clear();
    }
    if (pressed(Modifiers::NONE, Key::Delete) || pressed(Modifiers::NONE, Key::Backspace))
        && !selected.is_empty()
    {
        actions.extend(removal(&selected).map(Action::Run));
        state.playlist.selection.clear();
    }
    if pressed(Modifiers::ALT, Key::P) {
        state.playlist.view.clip_list = !state.playlist.view.clip_list;
    }
    if pressed(Modifiers::COMMAND, Key::B) && !selected.is_empty() {
        let span = duplicate_span(&selected);
        actions.extend(duplication(project, &selected).map(Action::Run));
        state.playlist.selection = selected.iter().map(|c| (c.track, c.start + span)).collect();
    }
    for (key, op) in [
        (crate::ClipboardKey::Cut, ClipOp::Cut),
        (crate::ClipboardKey::Copy, ClipOp::Copy),
        (crate::ClipboardKey::Paste, ClipOp::Paste),
    ] {
        if crate::clipboard_key(ctx, key) {
            clipboard_op(ctx, state, project, op, actions);
        }
    }
    let nudges = [
        (Key::ArrowLeft, Nudge::Left),
        (Key::ArrowRight, Nudge::Right),
        (Key::ArrowUp, Nudge::Up),
        (Key::ArrowDown, Nudge::Down),
    ];
    for (key, nudge) in nudges {
        if pressed(Modifiers::SHIFT, key) {
            nudge_selection(state, project, nudge, actions);
        }
    }
    for (key, step) in [(Key::ArrowUp, -1), (Key::ArrowDown, 1)] {
        if pressed(Modifiers::COMMAND, key) {
            let track = state.playlist.track.saturating_add_signed(step);
            state.playlist.track = track.min(PLAYLIST_TRACKS - 1);
        }
    }
    if pressed(Modifiers::SHIFT, Key::C) {
        show_source(
            state,
            project,
            ClipSource::Pattern(project.current_pattern().id),
        );
    }
    let zooms = [
        (Modifiers::NONE, Key::PageUp, Zoom::By(1.5)),
        (Modifiers::NONE, Key::PageDown, Zoom::By(1.0 / 1.5)),
        (Modifiers::SHIFT, Key::Num1, Zoom::Level(ZOOM_LEVELS[0])),
        (Modifiers::SHIFT, Key::Num2, Zoom::Level(ZOOM_LEVELS[1])),
        (Modifiers::SHIFT, Key::Num3, Zoom::Level(ZOOM_LEVELS[2])),
        (Modifiers::SHIFT, Key::Num4, Zoom::OutAndBack),
        (Modifiers::SHIFT, Key::Num5, Zoom::Loop),
    ];
    for (modifiers, key, op) in zooms {
        if pressed(modifiers, key) {
            zoom(state, project, op);
        }
    }
}

/// Cut, copy or paste of playlist clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipOp {
    Cut,
    Copy,
    Paste,
}

/// Cut and copy fill the playlist's clipboard with the selected clips;
/// paste adds them at the playhead (snapped down) on their own tracks, as one
/// undo step, and selects them.
fn clipboard_op(
    ctx: &egui::Context,
    state: &mut UiState,
    project: &Project,
    op: ClipOp,
    actions: &mut Vec<Action>,
) {
    let selected = selected_clips(state, project);
    match op {
        ClipOp::Cut | ClipOp::Copy if !selected.is_empty() => {
            let first = selected.iter().map(|c| c.start).min().unwrap_or(0);
            state.playlist.clipboard = selected
                .iter()
                .map(|c| Clip {
                    start: c.start - first,
                    ..*c
                })
                .collect();
            crate::copied(ctx, "clips");
            if op == ClipOp::Cut {
                actions.extend(removal(&selected).map(Action::Run));
                state.playlist.selection.clear();
            }
        }
        ClipOp::Paste if !state.playlist.clipboard.is_empty() => {
            let list = &state.playlist;
            let at = list.snap.floor(list.playhead, &list.meter);
            let adds = list
                .clipboard
                .iter()
                .map(|c| add_clip_invocation(project, c, c.track, c.start + at))
                .collect();
            actions.extend(batch(adds).map(Action::Run));
            state.playlist.selection = state
                .playlist
                .clipboard
                .iter()
                .map(|c| (c.track, c.start + at))
                .collect();
        }
        _ => {}
    }
}

/// A one-step move of the selected clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Nudge {
    /// One snap step earlier.
    Left,
    /// One snap step later.
    Right,
    /// One track up.
    Up,
    /// One track down.
    Down,
}

/// Moves the selected clips one snap step or one track, as one undo step;
/// nothing moves when a clip would leave the playlist.
fn nudge_selection(
    state: &mut UiState,
    project: &Project,
    nudge: Nudge,
    actions: &mut Vec<Action>,
) {
    let selected = selected_clips(state, project);
    let Some(first) = selected.iter().min_by_key(|c| c.start) else {
        return;
    };
    let list = &state.playlist;
    let step = |tick: u32| i64::from(list.snap.ticks_at(tick, &list.meter));
    let (dt, dtrack) = match nudge {
        Nudge::Left => (-step(first.start.saturating_sub(1)), 0),
        Nudge::Right => (step(first.start), 0),
        Nudge::Up => (0, -1),
        Nudge::Down => (0, 1),
    };
    let fits = selected.iter().all(|c| {
        let track = i64::from(c.track) + dtrack;
        i64::from(c.start) + dt >= 0 && (0..i64::from(PLAYLIST_TRACKS)).contains(&track)
    });
    if !fits {
        return;
    }
    if let Some(moved) = move_clips(&selected, dt, dtrack) {
        actions.push(Action::Run(moved));
        state.playlist.selection = selected.iter().map(|c| shifted(c, dt, dtrack)).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_into_stems_runs_once_per_sampler_sound() {
        let mut project = Project::default();
        for sample in ["builtin:kick", "builtin:snare"] {
            let id = musiccraft_project::ChannelId(project.alloc_id());
            project.channels.push(musiccraft_project::Channel::new(
                id,
                sample,
                musiccraft_project::Instrument::Sampler {
                    sample: sample.to_owned(),
                },
            ));
        }
        let (kick, snare) = (project.channels[0].id, project.channels[1].id);
        let clip = |id, start| Clip {
            source: ClipSource::Audio(id),
            track: 0,
            start,
            length: 96,
            offset: 0,
            muted: false,
        };
        let clips = [clip(snare, 0), clip(kick, 96), clip(snare, 192)];
        let runs: Vec<String> = split_stems(&project, &clips)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(runs, ["sample.split_stems 2", "sample.split_stems 1"]);
        assert!(split_stems(&project, &[]).is_empty());
    }

    #[test]
    fn the_wheel_steps_the_snap_and_stops_at_the_ends() {
        assert_eq!(step_snap(Snap::Bar, false), Snap::ALL[Snap::ALL.len() - 2]);
        assert_eq!(step_snap(Snap::Bar, true), Snap::ALL[Snap::ALL.len() - 1]);
        assert_eq!(step_snap(Snap::Off, false), Snap::Off);
        let last = Snap::ALL[Snap::ALL.len() - 1];
        assert_eq!(step_snap(last, true), last);
    }
    use musiccraft_project::PatternId;

    fn pattern_clip(track: u32, start: u32, length: u32) -> Clip {
        Clip {
            source: ClipSource::Pattern(PatternId(1)),
            track,
            start,
            length,
            offset: 0,
            muted: false,
        }
    }

    #[test]
    fn tall_headers_get_two_lines_and_short_ones_one() {
        let tall = HeaderLayout::of(Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 59.0)));
        assert!(tall.grip.is_some());
        assert!(
            tall.name.bottom() < tall.light.top(),
            "name above the switches"
        );
        let short = HeaderLayout::of(Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 20.0)));
        assert!(short.grip.is_none());
        assert!((short.name.center().y - short.light.center().y).abs() < 0.5);
        assert!(
            short.name.right() < short.arm.left(),
            "name stops short of the switches"
        );
        // The default track is tall enough for two lines.
        const { assert!(TRACK_HEIGHT - 1.0 >= TWO_LINE_HEADER) };
    }

    #[test]
    fn marker_items_add_every_n_bars_a_pause_and_remove_all_but_signatures() {
        let mut s = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        for line in [
            "pattern.add",
            "playlist.set_time_signature 2 3/4",
            "playlist.add_clip 1 1 9",
            "playlist.add_marker 4 Drop",
        ] {
            s.run_line(line).unwrap();
        }
        let names = |s: &musiccraft_commands::Session| -> Vec<String> {
            s.project()
                .playlist
                .markers
                .iter()
                .map(|m| m.name.clone())
                .collect()
        };
        let before = names(&s);
        let every = markers_every(s.project(), &s.project().song_meter(), 4).unwrap();
        s.run(&every).unwrap();
        let added: Vec<String> = names(&s)
            .into_iter()
            .filter(|n| !before.contains(n))
            .collect();
        assert_eq!(added, ["Bar 9"], "bar 5 already had Drop");
        let pause = pause_marker(s.project(), 0).unwrap();
        s.run(&pause).unwrap();
        let first = &s.project().playlist.markers[0];
        assert_eq!((first.tick, first.action), (0, MarkerAction::Pause));
        s.run(&remove_markers(s.project()).unwrap()).unwrap();
        assert!(
            s.project()
                .playlist
                .markers
                .iter()
                .all(|m| matches!(m.action, MarkerAction::TimeSignature(_)))
        );
        assert_eq!(s.project().playlist.markers.len(), 1);
    }

    #[test]
    fn unmute_all_and_clear_solo_toggle_only_the_tracks_that_need_it() {
        let mut s = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        for line in [
            "playlist.toggle_track_mute 2",
            "playlist.toggle_track_mute 5",
            "playlist.toggle_track_solo 3",
        ] {
            s.run_line(line).unwrap();
        }
        let list = &s.project().playlist;
        let unmute = per_track(&list.muted_tracks, "playlist.toggle_track_mute").unwrap();
        let unsolo = per_track(&list.soloed_tracks, "playlist.toggle_track_solo").unwrap();
        s.run(&unmute).unwrap();
        s.run(&unsolo).unwrap();
        assert!(s.project().playlist.muted_tracks.is_empty());
        assert!(s.project().playlist.soloed_tracks.is_empty());
        assert!(per_track(&BTreeSet::new(), "playlist.toggle_track_mute").is_none());
    }

    #[test]
    fn track_items_name_a_track_after_its_first_clip_and_reset_it() {
        let mut s = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        for line in [
            "pattern.add",
            "pattern.rename 2 Hook",
            "pattern.set_color 2 5",
            "playlist.add_clip 2 3 4",
            "playlist.add_clip 1 3 6",
            "playlist.set_track_height 3 200",
        ] {
            s.run_line(line).unwrap();
        }
        let on_track: Vec<Clip> = s.project().playlist.clips.clone();
        let name = name_after_first_clip(s.project(), 2, &on_track).unwrap();
        s.run(&name).unwrap();
        let list = &s.project().playlist;
        assert_eq!(list.track_name(2), "Hook");
        assert_eq!(list.track_colors.get(&2), Some(&4));
        let reset = reset_track(list, 2).unwrap();
        s.run(&reset).unwrap();
        let list = &s.project().playlist;
        assert_eq!(list.track_name(2), "Track 3");
        assert!(list.track_colors.is_empty() && list.track_heights.is_empty());
        assert!(reset_track(list, 2).is_none(), "nothing left to reset");
    }

    #[test]
    fn select_items_pick_inverted_muted_same_source_and_looped_clips() {
        let mut project = Project::default();
        let mut muted = pattern_clip(1, 384, 384);
        muted.muted = true;
        let mut other = pattern_clip(2, 1536, 384);
        other.source = ClipSource::Pattern(PatternId(2));
        project.playlist.clips = vec![pattern_clip(0, 0, 384), muted, other];
        project.playlist.loop_range = Some((300, 500));
        let mut state = UiState::default();
        state.playlist.selection = BTreeSet::from([(0, 0)]);
        let pick = |state: &UiState, pick, sources: &[ClipSource]| {
            picked(state, &project, pick, sources)
                .into_iter()
                .collect::<Vec<_>>()
        };
        assert_eq!(pick(&state, Pick::Invert, &[]), [(1, 384), (2, 1536)]);
        assert_eq!(pick(&state, Pick::Muted, &[]), [(1, 384)]);
        let first = [ClipSource::Pattern(PatternId(1))];
        assert_eq!(pick(&state, Pick::SameSource, &first), [(0, 0), (1, 384)]);
        assert_eq!(pick(&state, Pick::InLoop, &[]), [(0, 0), (1, 384)]);
    }

    #[test]
    fn slicing_at_the_playhead_cuts_only_crossing_clips() {
        let clips = [pattern_clip(0, 0, 384), pattern_clip(1, 384, 384)];
        assert_eq!(
            slice_at(&clips, 192).unwrap().to_string(),
            "playlist.slice_clip 1 192t"
        );
        assert!(slice_at(&clips, 384).is_none(), "an edge is not a cut");
        assert!(merge(&clips[..1]).is_none(), "merging needs two clips");
        assert_eq!(
            merge(&clips).unwrap().to_string(),
            "playlist.merge_clips 1 0t 2 384t"
        );
    }

    #[test]
    fn splitting_needs_pattern_clips_and_two_channels() {
        use musiccraft_project::{ChannelId, Note, NoteProps};
        let mut audio = pattern_clip(2, 0, 384);
        audio.source = ClipSource::Audio(ChannelId(9));
        let clips = [pattern_clip(0, 0, 384), audio];
        assert_eq!(
            split_at(&clips, 192).unwrap().to_string(),
            "playlist.split_clip 1 192t"
        );
        let mut project = Project::default();
        let id = project.patterns[0].id;
        let clips = [Clip {
            source: ClipSource::Pattern(id),
            ..clips[0]
        }];
        let note = |channel| Note {
            channel: ChannelId(channel),
            start: 0,
            length: 24,
            key: 60,
            velocity: 100,
            pan: 0,
            slide: false,
            props: NoteProps::default(),
        };
        project.patterns[0].notes = vec![note(1), note(1)];
        assert!(split_by_channel(&project, &clips).is_none(), "one channel");
        project.patterns[0].notes.push(note(2));
        assert_eq!(
            split_by_channel(&project, &clips).unwrap().to_string(),
            "pattern.split_by_channel 1"
        );
    }

    fn drag(kind: DragKind, track: u32, start: u32, length: u32) -> ClipDrag {
        ClipDrag {
            kind,
            original: Clip {
                source: musiccraft_project::ClipSource::Pattern(PatternId(1)),
                track: 0,
                start: 384,
                length: 384,
                offset: 0,
                muted: false,
            },
            grab: 0.0,
            track,
            start,
            length,
        }
    }

    #[test]
    fn clip_gestures_become_commands() {
        assert_eq!(
            finish(&drag(DragKind::Move, 2, 768, 384))
                .unwrap()
                .to_string(),
            "playlist.move_clip 1 384t 3 768t"
        );
        assert_eq!(
            finish(&drag(DragKind::Resize, 0, 384, 1152))
                .unwrap()
                .to_string(),
            "playlist.resize_clip 1 384t 1152t"
        );
        assert!(finish(&drag(DragKind::Move, 0, 384, 384)).is_none());
    }

    #[test]
    fn previews_place_notes_by_key_or_by_channel_row() {
        let mut s = musiccraft_commands::Session::new(
            Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        for line in [
            "channel.add_sampler builtin:kick",
            "channel.add_sampler builtin:hat",
            "channel.add_synth",
            // Pattern 1: steps, hat entered before kick.
            "step.toggle 2 1",
            "step.toggle 1 1",
            "step.toggle 2 5",
            // Pattern 2: a melody on two keys.
            "pattern.add",
            "note.add 3 0 4 C4 100",
            "note.add 3 4 4 C5 100",
            // Pattern 3: steps of one channel.
            "pattern.add",
            "step.toggle 1 1",
            "step.toggle 1 9",
        ] {
            s.run_line(line).unwrap_or_else(|e| panic!("{line}: {e}"));
        }
        let project = s.project();
        let body = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 20.0));
        let ys = |pattern: &musiccraft_project::Pattern| -> Vec<(usize, f32)> {
            let rows = NoteRows::of(project, pattern);
            pattern
                .notes
                .iter()
                .map(|n| {
                    let channel = project.channel_position(n.channel).unwrap_or(99);
                    (channel, rows.y(project, n, body))
                })
                .collect()
        };

        // Drums: one row per channel, in rack order whatever the note order.
        let steps = &project.patterns[0];
        assert!(matches!(NoteRows::of(project, steps), NoteRows::Channels(ref r) if r == &[0, 1]));
        for (channel, y) in ys(steps) {
            let want = if channel == 0 { 5.0 } else { 15.0 };
            assert!((y - want).abs() < 1e-4, "channel {channel} at {y}");
        }
        // A melody: by key, the lowest at the bottom.
        let melody = &project.patterns[1];
        let mut by_key: Vec<(u8, f32)> = melody
            .notes
            .iter()
            .map(|n| (n.key, NoteRows::of(project, melody).y(project, n, body)))
            .collect();
        by_key.sort_by_key(|(key, _)| *key);
        assert_eq!(
            by_key.iter().map(|(_, y)| *y).collect::<Vec<_>>(),
            [19.0, 1.0]
        );
        // One channel on one key: a single centre line.
        let single = &project.patterns[2];
        assert!(matches!(NoteRows::of(project, single), NoteRows::Single));
        assert!(ys(single).iter().all(|(_, y)| (*y - 10.0).abs() < 1e-4));
    }
}
