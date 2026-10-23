//! The browser column (`docs/spec/browser.md`): this project, instruments
//! and effects, presets, sound packs and the user's folders, as one tree of
//! full-width rows under a tab strip, with the search in the footer. Clicking
//! a sound previews it and clicking a plugin selects it; double-clicking adds
//! either, and sounds, generators and presets can be dragged onto the channel
//! rack (sounds also onto the playlist).

use crate::hint::Hint;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use egui::{Align2, Color32, FontId, Rect, RichText, Sense, Stroke, pos2, vec2};
use musiccraft_commands::Invocation;
use musiccraft_editors::sounds::{Entry, Listings, MAX_DEPTH};
use musiccraft_project::Instrument;
use musiccraft_project::params::{BUILTIN_PREFIX, BUILTIN_SAMPLES, EFFECT_KINDS};

use crate::icons::{self, Icon};
use crate::{Action, Frame, UiState, dialogs::FilePick, theme};

/// Something being dragged out of the browser: a sample, or a generator
/// that becomes a new channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dragged {
    /// A sample reference (a file path or a built-in sound).
    Sample(String),
    /// A generator from the plugin database.
    Generator {
        /// Its name in the browser.
        label: String,
        /// The command that adds it as a channel.
        add: Invocation,
    },
    /// An effect (built-in or plugin) that goes into a mixer track.
    Effect {
        /// Its name in the browser.
        label: String,
        /// The command that adds it, without the track argument.
        command: String,
        /// The command's arguments after the track.
        args: Vec<String>,
    },
    /// A pattern from the playlist's clip list, placed as a clip where it
    /// is dropped on a lane.
    Pattern {
        /// The pattern.
        id: musiccraft_project::PatternId,
        /// Its name.
        label: String,
    },
}

impl Dragged {
    /// The sample, for things that can also become audio clips.
    #[must_use]
    pub fn sample(&self) -> Option<&str> {
        match self {
            Self::Sample(sample) => Some(sample),
            Self::Generator { .. } | Self::Effect { .. } | Self::Pattern { .. } => None,
        }
    }

    /// The command that adds this item as a channel (effects become none).
    #[must_use]
    pub fn add_channel(&self) -> Option<Invocation> {
        match self {
            Self::Sample(sample) => Some(Invocation::new("channel.add_sampler", [sample.clone()])),
            Self::Generator { add, .. } => Some(add.clone()),
            Self::Effect { .. } | Self::Pattern { .. } => None,
        }
    }

    /// The command that adds this effect to mixer track `track`.
    #[must_use]
    pub fn add_effect(&self, track: usize) -> Option<Invocation> {
        let Self::Effect { command, args, .. } = self else {
            return None;
        };
        let args = std::iter::once(track.to_string()).chain(args.iter().cloned());
        Some(Invocation::new(command.as_str(), args))
    }

    /// Short label for the drag ghost.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Sample(sample) => sample
                .strip_prefix(BUILTIN_PREFIX)
                .map(str::to_owned)
                .or_else(|| {
                    Path::new(sample)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                })
                .unwrap_or_else(|| sample.clone()),
            Self::Generator { label, .. }
            | Self::Effect { label, .. }
            | Self::Pattern { label, .. } => label.clone(),
        }
    }
}

/// The browser's tabs: the whole tree, or one of its categories.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    All,
    Project,
    Plugins,
    Presets,
}

impl Tab {
    const ALL: [Self; 4] = [Self::All, Self::Project, Self::Plugins, Self::Presets];

    fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Project => "Project",
            Self::Plugins => "Plugins",
            Self::Presets => "Presets",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::All => Icon::Browser,
            Self::Project => Icon::Playlist,
            Self::Plugins => Icon::Plugins,
            Self::Presets => Icon::Knob,
        }
    }

    /// Whether this tab shows the category `of`.
    fn shows(self, of: Self) -> bool {
        self == Self::All || self == of
    }
}

/// Browser state: the tab, search text, which branches are open and cached
/// folder listings. The folders themselves are a setting.
#[derive(Debug, Clone, Default)]
pub struct BrowserState {
    search: String,
    pub(crate) tab: Tab,
    /// Branches opened or closed by hand, by key.
    pub(crate) open: BTreeMap<String, bool>,
    /// After "Fold all", branches that open by default start closed too.
    folded: bool,
    /// The column is rolled up to its header.
    collapsed: bool,
    /// Folders opened by hand, oldest first, for Backspace (back).
    back: Vec<String>,
    /// Folders Backspace closed, latest last, for Shift+Backspace.
    forward: Vec<String>,
    /// The browser's folders (a setting), copied here each frame for the
    /// sample slots' menus.
    pub(crate) folders: Vec<PathBuf>,
    /// Folder listings, shared with the sample slots' menus.
    pub(crate) listings: Listings,
}

/// The user's music and sample folders, when they exist.
#[must_use]
pub fn default_folders() -> Vec<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    home.map(|h| {
        ["Music", "Samples", "Music/Samples"]
            .iter()
            .map(|d| h.join(d))
            .filter(|p| p.is_dir())
            .collect()
    })
    .unwrap_or_default()
}

impl BrowserState {
    /// Subfolders and WAV files of `dir`, cached until refreshed.
    fn listing(&mut self, dir: &Path) -> &[Entry] {
        self.listings.get(dir)
    }

    /// Whether `dir` (to `depth` levels down) holds a file matching `search`.
    fn folder_matches(&mut self, dir: &Path, search: &str, depth: usize) -> bool {
        let entries = self.listing(dir).to_vec();
        entries.into_iter().any(|(name, path, is_dir)| {
            if is_dir {
                depth < MAX_DEPTH && self.folder_matches(&path, search, depth + 1)
            } else {
                matches(search, &name)
            }
        })
    }

    /// Whether the branch `key` is open. While searching, every branch that
    /// is drawn is open.
    fn is_open(&self, key: &str, default_open: bool) -> bool {
        !self.search.is_empty()
            || self
                .open
                .get(key)
                .copied()
                .unwrap_or(default_open && !self.folded)
    }

    /// Opens or closes the branch `key` by hand; opening one starts a new
    /// line of the back history.
    fn set_open(&mut self, key: &str, open: bool) {
        self.open.insert(key.to_owned(), open);
        if open {
            self.back.retain(|k| k != key);
            self.back.push(key.to_owned());
            self.forward.clear();
        }
    }

    /// Backspace: closes the folder opened last and returns it, to select.
    fn go_back(&mut self) -> Option<String> {
        let key = self.back.pop()?;
        self.open.insert(key.clone(), false);
        self.forward.push(key.clone());
        Some(key)
    }

    /// Shift+Backspace: reopens the folder Backspace closed last.
    fn go_forward(&mut self) -> Option<String> {
        let key = self.forward.pop()?;
        self.open.insert(key.clone(), true);
        self.back.push(key.clone());
        Some(key)
    }

    fn fold_all(&mut self) {
        self.open.clear();
        self.folded = true;
    }
}

fn matches(search: &str, name: &str) -> bool {
    search.is_empty() || name.to_lowercase().contains(&search.to_lowercase())
}

/// Height of a row in the list.
const ROW_HEIGHT: f32 = 19.0;
/// Width of the marker strip at a row's left end.
const MARKER: f32 = 6.0;
/// Indent per level.
const INDENT: f32 = 13.0;
/// Size of a row's type glyph.
const GLYPH: f32 = 13.0;

/// What a row shows before its name.
#[derive(Debug, Clone, Copy)]
enum Glyph {
    /// An icon in a colour.
    Icon(Icon, Color32),
    /// A small filled swatch (patterns).
    Swatch(Color32),
}

/// One full-width row of the tree.
struct Row<'a> {
    /// Identifies the row for selection.
    key: &'a str,
    depth: usize,
    glyph: Glyph,
    label: &'a str,
    /// Text colour of the name.
    color: Color32,
    /// A small dim tag at the right end (plugin format, counts).
    tag: &'a str,
    /// `Some(open)` for branches.
    open: Option<bool>,
}

impl<'a> Row<'a> {
    fn new(key: &'a str, depth: usize, glyph: Glyph, label: &'a str) -> Self {
        Self {
            key,
            depth,
            glyph,
            label,
            color: theme::TEXT,
            tag: "",
            open: None,
        }
    }
}

/// The row selected by a single click, by key.
fn selected_id() -> egui::Id {
    egui::Id::new("browser-selected-row")
}

/// A row as drawn, in order, for the keyboard.
#[derive(Debug, Clone)]
struct Drawn {
    key: String,
    depth: usize,
    /// `Some(open)` for branches.
    open: Option<bool>,
}

/// The rows drawn this frame (the keys read last frame's).
fn drawn_id() -> egui::Id {
    egui::Id::new("browser-drawn-rows")
}

/// The row Enter asked to act on, by key.
fn activate_id() -> egui::Id {
    egui::Id::new("browser-activate-row")
}

/// The row the keyboard moved to, scrolled into view when next drawn.
fn reveal_id() -> egui::Id {
    egui::Id::new("browser-reveal-row")
}

/// Whether Enter asked the row `key` to act (as a double-click does).
fn activated(ui: &egui::Ui, key: &str) -> bool {
    let asked = ui
        .data(|d| d.get_temp::<String>(activate_id()))
        .is_some_and(|k| k == key);
    if asked {
        ui.data_mut(|d| d.remove::<String>(activate_id()));
    }
    asked
}

/// The browser's keys, while the pointer is over it: Up and Down move the
/// selected row, Shift+Up and Shift+Down move within the folder and return a
/// sound moved onto (to send), Right opens a folder or steps into it, Left
/// closes it or steps out, Enter toggles a folder or acts as a double-click,
/// Backspace and Shift+Backspace go back and forward through the folders
/// opened, Ctrl+Up folds all, Ctrl+F goes to the search and Esc clears it.
fn keys(ctx: &egui::Context, state: &mut UiState, rows: &[Drawn]) -> Option<String> {
    use egui::{Key, Modifiers};
    let pressed = |modifiers: Modifiers, key: Key| ctx.input_mut(|i| i.consume_key(modifiers, key));
    if pressed(Modifiers::COMMAND, Key::F) {
        ctx.memory_mut(|m| m.request_focus(search_id()));
    }
    if pressed(Modifiers::COMMAND, Key::ArrowUp) {
        state.browser.fold_all();
    }
    if !state.browser.search.is_empty() && pressed(Modifiers::NONE, Key::Escape) {
        state.browser.search.clear();
    }
    let selected = ctx.data(|d| d.get_temp::<String>(selected_id()));
    let at = selected
        .as_ref()
        .and_then(|key| rows.iter().position(|r| &r.key == key));
    let select = |index: usize| {
        if let Some(r) = rows.get(index) {
            ctx.data_mut(|d| {
                d.insert_temp(selected_id(), r.key.clone());
                d.insert_temp(reveal_id(), r.key.clone());
            });
        }
    };
    // Shift first: the plain keys would also take the shifted ones.
    let moved = if pressed(Modifiers::SHIFT, Key::Backspace) {
        state.browser.go_forward()
    } else if pressed(Modifiers::NONE, Key::Backspace) {
        state.browser.go_back()
    } else {
        None
    };
    if let Some(key) = moved {
        ctx.data_mut(|d| {
            d.insert_temp(selected_id(), key.clone());
            d.insert_temp(reveal_id(), key);
        });
        return None;
    }
    // Before the plain arrows, which would also take these.
    for (key, down) in [(Key::ArrowDown, true), (Key::ArrowUp, false)] {
        if let Some(i) = at
            && pressed(Modifiers::SHIFT, key)
            && let Some(next) = sibling(rows, i, down)
        {
            select(next);
            return rows[next].key.strip_prefix("sound:").map(str::to_owned);
        }
    }
    if pressed(Modifiers::NONE, Key::ArrowDown) {
        select(at.map_or(0, |i| (i + 1).min(rows.len().saturating_sub(1))));
    }
    if pressed(Modifiers::NONE, Key::ArrowUp) {
        select(at.map_or(0, |i| i.saturating_sub(1)));
    }
    let (i, row) = at.and_then(|i| rows.get(i).map(|r| (i, r)))?;
    if pressed(Modifiers::NONE, Key::ArrowRight) {
        match row.open {
            Some(false) => {
                state.browser.set_open(&row.key, true);
            }
            Some(true) => select(i + 1),
            None => {}
        }
    }
    if pressed(Modifiers::NONE, Key::ArrowLeft) {
        if row.open == Some(true) {
            state.browser.set_open(&row.key, false);
        } else if let Some(parent) = rows[..i].iter().rposition(|r| r.depth < row.depth) {
            select(parent);
        }
    }
    if pressed(Modifiers::NONE, Key::Enter) {
        match row.open {
            Some(open) => {
                state.browser.set_open(&row.key, !open);
            }
            None => ctx.data_mut(|d| {
                d.insert_temp(activate_id(), row.key.clone());
            }),
        }
    }
    None
}

/// The next (or previous) row in the same folder as row `i`.
fn sibling(rows: &[Drawn], i: usize, down: bool) -> Option<usize> {
    let depth = rows.get(i)?.depth;
    if down {
        let after = rows.get(i + 1..)?;
        let j = after
            .iter()
            .take_while(|r| r.depth >= depth)
            .position(|r| r.depth == depth)?;
        Some(i + 1 + j)
    } else {
        let j = rows[..i]
            .iter()
            .rev()
            .take_while(|r| r.depth >= depth)
            .position(|r| r.depth == depth)?;
        Some(i - 1 - j)
    }
}

/// Sends `sample` to the selected channel when it plays a sample, and
/// previews it. Sends to the same channel within two seconds of each other
/// are one undo step, so auditioning a folder undoes in one go.
fn send(
    ctx: &egui::Context,
    state: &mut UiState,
    project: &musiccraft_project::Project,
    sample: String,
    actions: &mut Vec<Action>,
) {
    let index = state.selected_channel;
    let plays_sample = project.channels.get(index).is_some_and(|c| {
        matches!(
            c.instrument,
            Instrument::Sampler { .. } | Instrument::Granular { .. } | Instrument::Resynth { .. }
        )
    });
    if plays_sample {
        let id = egui::Id::new("browser-last-send");
        let now = ctx.input(|i| i.time);
        let last = ctx.data(|d| d.get_temp::<(f64, usize)>(id));
        if last.is_none_or(|(t, was)| now - t > 2.0 || was != index) {
            state.begin_gesture();
        }
        ctx.data_mut(|d| d.insert_temp(id, (now, index)));
        let invocation = Invocation::new(
            "channel.set_sample",
            [(index + 1).to_string(), sample.clone()],
        );
        actions.push(Action::Coalesced(invocation, state.gesture()));
    }
    actions.push(Action::PreviewSample(sample));
}

/// The search field's id.
fn search_id() -> egui::Id {
    egui::Id::new("browser-search")
}

/// Draws `row` across the whole width and returns its response; a click
/// selects it.
fn row(ui: &mut egui::Ui, row: &Row<'_>, sense: Sense) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), sense);
    if response.clicked() {
        ui.data_mut(|d| d.insert_temp(selected_id(), row.key.to_owned()));
    }
    ui.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<Drawn>>(drawn_id())
            .push(Drawn {
                key: row.key.to_owned(),
                depth: row.depth,
                open: row.open,
            });
    });
    if ui
        .data(|d| d.get_temp::<String>(reveal_id()))
        .is_some_and(|k| k == row.key)
    {
        ui.data_mut(|d| d.remove::<String>(reveal_id()));
        ui.scroll_to_rect(rect, None);
    }
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let selected = ui
        .data(|d| d.get_temp::<String>(selected_id()))
        .is_some_and(|s| s == row.key);
    let painter = ui.painter();
    if selected {
        painter.rect_filled(rect, 0.0, theme::ACCENT_DIM);
        painter.rect_filled(
            Rect::from_min_size(rect.min + vec2(1.0, 3.0), vec2(2.5, rect.height() - 6.0)),
            1.0,
            theme::ACCENT,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, 0.0, theme::RAISED);
    }
    // Tree lines: one down from each ancestor's glyph, and a stub into this row.
    let glyph_x = |depth: usize| rect.left() + MARKER + depth as f32 * INDENT + GLYPH / 2.0;
    let line = Stroke::new(1.0, theme::TREE_LINE);
    for level in 0..row.depth {
        let x = glyph_x(level);
        painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], line);
    }
    if row.depth > 0 {
        let x = glyph_x(row.depth - 1);
        painter.line_segment(
            [
                pos2(x, rect.center().y),
                pos2(glyph_x(row.depth) - GLYPH / 2.0 - 1.0, rect.center().y),
            ],
            line,
        );
    }
    let glyph = Rect::from_center_size(
        pos2(glyph_x(row.depth), rect.center().y),
        vec2(GLYPH, GLYPH),
    );
    match row.glyph {
        Glyph::Icon(icon, color) => icons::paint(painter, glyph.expand(1.5), icon, color),
        Glyph::Swatch(color) => {
            painter.rect_filled(glyph.shrink(2.0), 2.0, color);
        }
    }
    let mut right = rect.right() - 4.0;
    if row.open == Some(true) {
        let chevron = Rect::from_center_size(pos2(right - 5.0, rect.center().y), vec2(10.0, 10.0));
        icons::paint(painter, chevron, Icon::ChevronDown, theme::TEXT_DIM);
        right -= 14.0;
    }
    if !row.tag.is_empty() {
        let tag = painter.text(
            pos2(right, rect.center().y),
            Align2::RIGHT_CENTER,
            row.tag,
            FontId::proportional(theme::FONT_CAPTION + 0.5),
            theme::TEXT_DIM,
        );
        right = tag.left() - 4.0;
    }
    let left = glyph.right() + 5.0;
    let font = FontId::proportional(theme::FONT_TITLE);
    let mut job = egui::text::LayoutJob::simple_singleline(row.label.to_owned(), font, row.color);
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - left).max(8.0));
    let galley = painter.layout_job(job);
    let pos = pos2(left, rect.center().y - galley.size().y / 2.0);
    if galley.elided && response.hovered() {
        pop_out(ui, pos, row);
    }
    // Open branches are drawn a little stronger.
    if row.open == Some(true) {
        painter.galley(pos + vec2(0.4, 0.0), galley.clone(), row.color);
    }
    painter.galley(pos, galley, row.color);
    response
}

/// Shows a cut-off name in full: a raised box over the row, starting where
/// the name starts and running past the panel's edge if it must.
fn pop_out(ui: &egui::Ui, pos: egui::Pos2, row: &Row<'_>) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        egui::Id::new("browser-name-pop-out"),
    ));
    let galley = painter.layout_no_wrap(
        row.label.to_owned(),
        FontId::proportional(theme::FONT_TITLE),
        row.color,
    );
    let rect = Rect::from_min_size(pos, galley.size()).expand2(vec2(4.0, 2.0));
    painter.rect(
        rect,
        2.0,
        theme::RAISED,
        Stroke::new(1.0, theme::WINDOW_EDGE),
        egui::StrokeKind::Outside,
    );
    painter.galley(pos, galley, row.color);
}

/// A branch row: click opens or closes it, and `body` draws its children
/// one level deeper while it is open. Hidden while searching unless
/// `has_match`.
#[allow(clippy::too_many_arguments)]
fn branch(
    ui: &mut egui::Ui,
    state: &mut UiState,
    key: &str,
    depth: usize,
    icon: (Icon, Color32),
    label: &str,
    default_open: bool,
    has_match: bool,
    body: impl FnOnce(&mut egui::Ui, &mut UiState),
) -> Option<egui::Response> {
    if !state.browser.search.is_empty() && !has_match {
        return None;
    }
    let open = state.browser.is_open(key, default_open);
    let (glyph, color) = icon;
    let glyph = if open && glyph == Icon::Browser {
        Icon::FolderOpen
    } else {
        glyph
    };
    let mut shown = Row::new(key, depth, Glyph::Icon(glyph, color), label);
    shown.open = Some(open);
    let response = row(ui, &shown, Sense::click());
    if response.clicked() && state.browser.search.is_empty() {
        state.browser.set_open(key, !open);
    }
    if open {
        body(ui, state);
    }
    Some(response)
}

/// A sound row: click previews, double-click adds a sampler channel,
/// right-click offers the other "Add as" choices, and it can be dragged.
fn item(ui: &mut egui::Ui, depth: usize, label: &str, dragged: Dragged, actions: &mut Vec<Action>) {
    let Dragged::Sample(sample) = &dragged else {
        return;
    };
    let sample = sample.clone();
    let key = format!("sound:{sample}");
    let response = row(
        ui,
        &Row::new(&key, depth, Glyph::Icon(Icon::Wave, theme::ACCENT), label),
        Sense::click_and_drag(),
    )
    .hint("Click to preview · double-click to add · drag onto the channel rack or playlist · right-click for more");
    if response.drag_started() {
        response.dnd_set_drag_payload(dragged.clone());
    }
    response.context_menu(|ui| {
        if ui.button("Add as sampler").clicked() {
            actions.extend(dragged.add_channel().map(Action::Run));
            ui.close();
        }
        if ui.button("Add as granular instrument").clicked() {
            actions.push(Action::Run(Invocation::new(
                "channel.add_granular",
                [sample.clone()],
            )));
            ui.close();
        }
        if ui.button("Add as resynth (play its harmonics)").clicked() {
            actions.push(Action::Run(Invocation::new(
                "channel.add_resynth",
                [sample.clone()],
            )));
            ui.close();
        }
    });
    if response.double_clicked() || activated(ui, &key) {
        actions.extend(dragged.add_channel().map(Action::Run));
    } else if response.clicked() {
        actions.push(Action::PreviewSample(sample));
    }
}

/// A plugin, generator or preset row: a click selects it, a double-click
/// runs `add`, and `drag` (generators and presets) can be dragged onto the
/// channel rack.
#[allow(clippy::too_many_arguments)]
fn plugin_entry(
    ui: &mut egui::Ui,
    depth: usize,
    icon: (Icon, Color32),
    label: &str,
    tag: &str,
    hint: String,
    add: Invocation,
    drag: Option<Dragged>,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let key = format!("plugin:{add}");
    let mut shown = Row::new(&key, depth, Glyph::Icon(icon.0, icon.1), label);
    shown.tag = tag;
    let response = row(ui, &shown, Sense::click_and_drag()).hint(hint);
    if let Some(dragged) = drag {
        if response.drag_started() {
            response.dnd_set_drag_payload(dragged);
        }
    }
    if response.double_clicked() || activated(ui, &key) {
        actions.push(Action::Run(add));
    }
    response
}

/// A generator row: double-click or drag onto the channel rack to add it.
fn generator(
    ui: &mut egui::Ui,
    depth: usize,
    label: &str,
    hint: &str,
    add: Invocation,
    actions: &mut Vec<Action>,
) {
    generator_with(
        ui,
        depth,
        (Icon::Keyboard, theme::TEXT),
        label,
        "",
        hint,
        add,
        actions,
    );
}

#[allow(clippy::too_many_arguments)]
fn generator_with(
    ui: &mut egui::Ui,
    depth: usize,
    icon: (Icon, Color32),
    label: &str,
    tag: &str,
    hint: &str,
    add: Invocation,
    actions: &mut Vec<Action>,
) {
    let dragged = Dragged::Generator {
        label: label.to_owned(),
        add: add.clone(),
    };
    plugin_entry(
        ui,
        depth,
        icon,
        label,
        tag,
        format!("{hint} · double-click or drag onto the channel rack to add a channel"),
        add,
        Some(dragged),
        actions,
    );
}

/// The built-in generators: label, hint and the command that adds one.
fn builtin_generators() -> Vec<(String, &'static str, Invocation)> {
    let mut list = vec![
        (
            "Synth".to_owned(),
            "Subtractive synth",
            Invocation::bare("channel.add_synth"),
        ),
        (
            "Antiserum".to_owned(),
            "Wavetable synth",
            Invocation::bare("channel.add_antiserum"),
        ),
    ];
    for kind in musiccraft_project::params::DRUM_TYPES {
        list.push((
            format!("Drum synth: {kind}"),
            "Built-in drum synth",
            Invocation::new("channel.add_drum", [kind]),
        ));
    }
    let more: [(&str, &'static str, Invocation); 10] = [
        (
            "Pluck",
            "Built-in plucked string",
            Invocation::bare("channel.add_pluck"),
        ),
        (
            "FM synth",
            "Four-operator FM synth",
            Invocation::bare("channel.add_fm"),
        ),
        (
            "Keys: piano",
            "Piano",
            Invocation::new("channel.add_keys", ["piano"]),
        ),
        (
            "Keys: electric piano",
            "Electric piano",
            Invocation::new("channel.add_keys", ["epiano"]),
        ),
        (
            "Keys: organ",
            "Organ",
            Invocation::new("channel.add_keys", ["organ"]),
        ),
        (
            "Additive",
            "Additive (harmonic) synth",
            Invocation::bare("channel.add_additive"),
        ),
        (
            "Drum kit",
            "16-pad sample kit",
            Invocation::bare("channel.add_kit"),
        ),
        (
            "Zone sampler",
            "Several samples mapped to key and velocity zones",
            Invocation::bare("channel.add_zones"),
        ),
        (
            "Channel stack",
            "Plays several channels at once",
            Invocation::bare("channel.add_layer"),
        ),
        (
            "MIDI output",
            "Sends notes to external MIDI gear",
            Invocation::bare("channel.add_midi_out"),
        ),
    ];
    list.extend(more.into_iter().map(|(l, h, i)| (l.to_owned(), h, i)));
    list
}

/// Instruments & effects: the built-in effects and generators, then the
/// installed plugins split by kind, each tagged with its format.
fn instruments_and_effects(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let search = state.browser.search.clone();
    let plugins = frame.plugins;
    let track = state.selected_track.to_string();
    let effects: Vec<_> = EFFECT_KINDS
        .iter()
        .filter(|k| matches(&search, effect_title(**k)) || matches(&search, k.name()))
        .collect();
    let generators: Vec<_> = builtin_generators()
        .into_iter()
        .filter(|(label, ..)| matches(&search, label))
        .collect();
    let installed_effects: Vec<_> = plugins
        .of_kind(false)
        .filter(|p| matches(&search, &p.name))
        .collect();
    let installed_generators: Vec<_> = plugins
        .of_kind(true)
        .filter(|p| matches(&search, &p.name))
        .collect();
    let any_installed = !installed_effects.is_empty() || !installed_generators.is_empty();
    let has_match = !effects.is_empty() || !generators.is_empty() || any_installed;
    let accent = (Icon::Plugins, theme::ACCENT);
    let default_open = state.browser.tab == Tab::Plugins;
    branch(
        ui,
        state,
        "plugins",
        0,
        accent,
        "Instruments & effects",
        default_open,
        has_match,
        |ui, state| {
            let folder = (Icon::Browser, theme::TEXT_DIM);
            branch(
                ui,
                state,
                "plugins/effects",
                1,
                folder,
                "Effects",
                false,
                !effects.is_empty(),
                |ui, _| {
                    for kind in &effects {
                        let hint = format!(
                            "Built-in {} · double-click to add it to the selected mixer track",
                            kind.name()
                        );
                        plugin_entry(
                            ui,
                            2,
                            (Icon::Effect, theme::TEXT),
                            effect_title(**kind),
                            "",
                            hint,
                            Invocation::new(
                                "mixer.add_effect",
                                [track.clone(), kind.name().to_owned()],
                            ),
                            Some(Dragged::Effect {
                                label: effect_title(**kind).to_owned(),
                                command: "mixer.add_effect".to_owned(),
                                args: vec![kind.name().to_owned()],
                            }),
                            actions,
                        );
                    }
                },
            );
            branch(
                ui,
                state,
                "plugins/generators",
                1,
                folder,
                "Generators",
                false,
                !generators.is_empty(),
                |ui, _| {
                    for (label, hint, add) in &generators {
                        generator(ui, 2, label, hint, add.clone(), actions);
                    }
                },
            );
            let installed = !plugins.available || any_installed;
            branch(
                ui,
                state,
                "plugins/installed",
                1,
                folder,
                "Installed",
                false,
                installed,
                |ui, state| {
                    if !plugins.available {
                        dim_line(ui, 2, "Plugin hosting is not available");
                        return;
                    }
                    branch(
                        ui,
                        state,
                        "plugins/installed/effects",
                        2,
                        folder,
                        "Effects",
                        false,
                        !installed_effects.is_empty(),
                        |ui, _| {
                            for entry in &installed_effects {
                                plugin_entry(
                                    ui,
                                    3,
                                    (Icon::Effect, theme::TEXT),
                                    &entry.name,
                                    &entry.format,
                                    format!(
                                        "{} · {} · double-click to add it to the selected mixer track",
                                        entry.vendor, entry.id
                                    ),
                                    Invocation::new(
                                        "mixer.add_plugin",
                                        [track.clone(), entry.id.clone(), entry.name.clone()],
                                    ),
                                    Some(Dragged::Effect {
                                        label: entry.name.clone(),
                                        command: "mixer.add_plugin".to_owned(),
                                        args: vec![entry.id.clone(), entry.name.clone()],
                                    }),
                                    actions,
                                );
                            }
                        },
                    );
                    branch(
                        ui,
                        state,
                        "plugins/installed/generators",
                        2,
                        folder,
                        "Generators",
                        false,
                        !installed_generators.is_empty(),
                        |ui, _| {
                            for entry in &installed_generators {
                                generator_with(
                                    ui,
                                    3,
                                    (Icon::Keyboard, theme::TEXT),
                                    &entry.name,
                                    &entry.format,
                                    &format!("{} · {}", entry.vendor, entry.id),
                                    Invocation::new(
                                        "channel.add_plugin",
                                        [entry.id.clone(), entry.name.clone()],
                                    ),
                                    actions,
                                );
                            }
                        },
                    );
                },
            );
        },
    );
}

/// Presets: one folder per instrument with presets (Antiserum), holding the
/// factory categories, then the user's presets. A preset adds an Antiserum
/// channel playing it.
fn presets(ui: &mut egui::Ui, state: &mut UiState, actions: &mut Vec<Action>) {
    let search = state.browser.search.clone();
    let bank = musiccraft_antiserum_presets::factory();
    let user = musiccraft_antiserum_presets::user::cached();
    let hit = |p: &musiccraft_patch::Patch| {
        matches(&search, &p.name) || p.tags.iter().any(|t| matches(&search, t))
    };
    let any = bank.iter().chain(user.iter()).any(hit);
    let knob = (Icon::Knob, theme::ACCENT);
    let folder = (Icon::Browser, theme::TEXT_DIM);
    let default_open = state.browser.tab == Tab::Presets;
    branch(
        ui,
        state,
        "presets",
        0,
        knob,
        "Presets",
        default_open,
        any,
        |ui, state| {
            branch(
                ui,
                state,
                "presets/antiserum",
                1,
                folder,
                "Antiserum",
                default_open,
                any,
                |ui, state| {
                    for category in musiccraft_patch::CATEGORIES {
                        let list: Vec<_> = bank
                            .iter()
                            .filter(|p| p.category == category && hit(p))
                            .collect();
                        if list.is_empty() {
                            continue;
                        }
                        let key = format!("presets/antiserum/{category}");
                        branch(
                            ui,
                            state,
                            &key,
                            2,
                            folder,
                            category,
                            false,
                            true,
                            |ui, _| {
                                for preset in list {
                                    preset_row(
                                        ui,
                                        &preset.name,
                                        &format!("Antiserum preset ({})", preset.tags.join(", ")),
                                        actions,
                                    );
                                }
                            },
                        );
                    }
                    let mine: Vec<_> = user.iter().filter(|p| hit(p)).collect();
                    if !mine.is_empty() {
                        branch(
                            ui,
                            state,
                            "presets/antiserum/user",
                            2,
                            folder,
                            "User",
                            false,
                            true,
                            |ui, _| {
                                for preset in mine {
                                    preset_row(ui, &preset.name, "Your Antiserum preset", actions);
                                }
                            },
                        );
                    }
                },
            );
        },
    );
}

/// A preset row: a generator row whose click also previews the preset.
fn preset_row(ui: &mut egui::Ui, name: &str, hint: &str, actions: &mut Vec<Action>) {
    let add = Invocation::new("channel.add_antiserum", [name.to_owned()]);
    let dragged = Dragged::Generator {
        label: name.to_owned(),
        add: add.clone(),
    };
    let response = plugin_entry(
        ui,
        3,
        (Icon::Knob, theme::HIGHLIGHT),
        name,
        "",
        format!(
            "{hint} · click to preview · double-click or drag onto the channel rack to add a channel"
        ),
        add,
        Some(dragged),
        actions,
    );
    if response.clicked() {
        actions.push(Action::PreviewPreset(name.to_owned()));
    }
}

/// One dim, indented line of text (empty folders, notices).
fn dim_line(ui: &mut egui::Ui, depth: usize, text: &str) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
    ui.painter().text(
        pos2(
            rect.left() + MARKER + depth as f32 * INDENT + 2.0,
            rect.center().y,
        ),
        Align2::LEFT_CENTER,
        text,
        FontId::proportional(theme::FONT_LABEL),
        theme::TEXT_DIM,
    );
}

/// Display name of a built-in effect.
#[must_use]
pub fn effect_title(kind: musiccraft_project::params::EffectKind) -> &'static str {
    kind.title()
}

/// This project: patterns, channels, effects, controllers, automation and
/// samples.
#[allow(clippy::too_many_lines)]
fn this_project(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    actions: &mut Vec<Action>,
) {
    let project = frame.project;
    let search = state.browser.search.clone();
    let patterns: Vec<_> = project
        .patterns
        .iter()
        .enumerate()
        .filter(|(_, p)| matches(&search, &p.name))
        .collect();
    let channels: Vec<_> = project
        .channels
        .iter()
        .enumerate()
        .filter(|(_, c)| matches(&search, &c.name))
        .collect();
    let effects: Vec<(usize, usize, String)> = project
        .mixer
        .iter()
        .enumerate()
        .flat_map(|(t, track)| {
            track.effects.iter().enumerate().map(move |(s, effect)| {
                let name = effect
                    .plugin
                    .as_ref()
                    .map_or_else(|| effect_title(effect.kind).to_owned(), |p| p.name.clone());
                (t, s, format!("{} · {name}", track.name))
            })
        })
        .filter(|(.., label)| matches(&search, label))
        .collect();
    let automations: Vec<_> = project
        .automations
        .iter()
        .filter(|a| matches(&search, &a.name))
        .collect();
    let mut used: Vec<&str> = project
        .channels
        .iter()
        .filter_map(|c| match &c.instrument {
            Instrument::Sampler { sample } if !sample.starts_with(BUILTIN_PREFIX) => {
                Some(sample.as_str())
            }
            _ => None,
        })
        .collect();
    used.sort_unstable();
    used.dedup();
    let samples: Vec<(String, &str)> = used
        .into_iter()
        .map(|sample| {
            let name = Path::new(sample)
                .file_name()
                .map_or_else(|| sample.to_owned(), |n| n.to_string_lossy().into_owned());
            (name, sample)
        })
        .filter(|(name, _)| matches(&search, name))
        .collect();
    let controllers = project
        .controllers
        .iter()
        .any(|c| matches(&search, &c.name));
    let history = &frame.history;
    let redo: Vec<(usize, &String)> = history
        .redo_steps
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, s)| matches(&search, s))
        .collect();
    let undo: Vec<(usize, &String)> = history
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| matches(&search, s))
        .collect();
    let has_match = !patterns.is_empty()
        || !redo.is_empty()
        || !undo.is_empty()
        || !channels.is_empty()
        || !effects.is_empty()
        || !automations.is_empty()
        || !samples.is_empty()
        || controllers;
    let folder = (Icon::Browser, theme::TEXT_DIM);
    let default_open = state.browser.tab == Tab::Project;
    branch(
        ui,
        state,
        "project",
        0,
        (Icon::Playlist, theme::ACCENT),
        "This project",
        default_open,
        has_match,
        |ui, state| {
            let label = format!("Patterns ({})", project.patterns.len());
            branch(
                ui,
                state,
                "project/patterns",
                1,
                folder,
                &label,
                false,
                !patterns.is_empty(),
                |ui, _| {
                    for (i, pattern) in &patterns {
                        let key = format!("pattern:{}", pattern.id.0);
                        let swatch = crate::pattern_color(pattern).unwrap_or(theme::CLIP_NEUTRAL);
                        let response = row(
                            ui,
                            &Row::new(&key, 2, Glyph::Swatch(swatch), &pattern.name),
                            Sense::click(),
                        )
                        .hint("Click to select this pattern · right-click for its menu");
                        if response.clicked() {
                            actions.push(Action::Run(Invocation::new(
                                "pattern.select",
                                [(i + 1).to_string()],
                            )));
                        }
                        response.context_menu(|ui| {
                            crate::playlist::split_pattern_item(ui, pattern, *i, actions);
                        });
                    }
                },
            );
            let label = format!("Channels ({})", project.channels.len());
            branch(
                ui,
                state,
                "project/channels",
                1,
                folder,
                &label,
                false,
                !channels.is_empty(),
                |ui, state| {
                    for (i, channel) in &channels {
                        let key = format!("channel:{}", channel.id.0);
                        let response = row(
                            ui,
                            &Row::new(
                                &key,
                                2,
                                Glyph::Icon(Icon::Keyboard, crate::channel_color(project, *i)),
                                &channel.name,
                            ),
                            Sense::click(),
                        )
                        .hint("Click to open its channel settings");
                        if response.clicked() {
                            state.selected_channel = *i;
                            state.windows.channel_settings = true;
                        }
                    }
                },
            );
            let total: usize = project.mixer.iter().map(|t| t.effects.len()).sum();
            let label = format!("Effects ({total})");
            branch(
                ui,
                state,
                "project/effects",
                1,
                folder,
                &label,
                false,
                !effects.is_empty(),
                |ui, state| {
                    for (track, slot, label) in &effects {
                        let key = format!("effect:{track}:{slot}");
                        if row(
                            ui,
                            &Row::new(&key, 2, Glyph::Icon(Icon::Effect, theme::TEXT), label),
                            Sense::click(),
                        )
                        .hint("Click to open or close its window")
                        .clicked()
                        {
                            crate::effect_window::toggle(state, *track, *slot);
                        }
                    }
                },
            );
            let label = format!("Controllers ({})", project.controllers.len());
            let has_controllers = search.is_empty() || controllers;
            branch(
                ui,
                state,
                "project/controllers",
                1,
                (Icon::Sine, theme::AUTOMATION),
                &label,
                !project.controllers.is_empty(),
                has_controllers,
                |ui, state| {
                    ui.indent("browser-controllers", |ui| {
                        crate::controllers::list(ui, state, project, actions);
                    });
                },
            );
            let label = format!("Automation ({})", project.automations.len());
            branch(
                ui,
                state,
                "project/automation",
                1,
                folder,
                &label,
                false,
                !automations.is_empty(),
                |ui, state| {
                    for automation in &automations {
                        let key = format!("automation:{}", automation.id.0);
                        let mut shown = Row::new(
                            &key,
                            2,
                            Glyph::Icon(Icon::Automation, theme::AUTOMATION),
                            &automation.name,
                        );
                        shown.color = theme::AUTOMATION;
                        if row(ui, &shown, Sense::click())
                            .hint("Click to open it in the automation editor")
                            .clicked()
                        {
                            state.automation_editor = Some(automation.id);
                        }
                    }
                },
            );
            let label = format!("Samples ({})", samples.len());
            branch(
                ui,
                state,
                "project/samples",
                1,
                folder,
                &label,
                false,
                !samples.is_empty(),
                |ui, _| {
                    for (name, sample) in &samples {
                        item(ui, 2, name, Dragged::Sample((*sample).to_owned()), actions);
                    }
                },
            );
            let label = format!("History ({})", history.steps.len());
            branch(
                ui,
                state,
                "project/history",
                1,
                folder,
                &label,
                false,
                !redo.is_empty() || !undo.is_empty(),
                |ui, _| history_rows(ui, &redo, &undo, actions),
            );
        },
    );
}

/// This project › History: redo steps (dim) above the undo steps, newest
/// first, as in Edit › History. A click undoes or redoes to just after it.
fn history_rows(
    ui: &mut egui::Ui,
    redo: &[(usize, &String)],
    undo: &[(usize, &String)],
    actions: &mut Vec<Action>,
) {
    use crate::toolbar::HistoryPick;
    let rows = redo
        .iter()
        .map(|(i, step)| (HistoryPick::Redo(*i), *step, true))
        .chain(
            undo.iter()
                .map(|(i, step)| (HistoryPick::Undo(*i), *step, false)),
        );
    for (pick, step, ahead) in rows {
        let key = format!("history:{pick:?}");
        let mut shown = Row::new(&key, 2, Glyph::Icon(Icon::Undo, theme::TEXT_DIM), step);
        if ahead {
            shown.color = theme::TEXT_DIM;
        }
        let hint = if ahead {
            "Click to redo up to this step"
        } else {
            "Click to undo back to just after this step"
        };
        if row(ui, &shown, Sense::click()).hint(hint).clicked() {
            let (name, times) = pick.runs();
            for _ in 0..times {
                actions.push(Action::Run(Invocation::new(name, Vec::<String>::new())));
            }
        }
    }
}

/// A folder on disk, its subfolders first and then its WAV files.
fn folder(
    ui: &mut egui::Ui,
    state: &mut UiState,
    dir: &Path,
    label: &str,
    depth: usize,
    actions: &mut Vec<Action>,
) {
    let search = state.browser.search.clone();
    let has_match = search.is_empty() || state.browser.folder_matches(dir, &search, depth);
    let key = format!("folder:{}", dir.display());
    let color = if depth == 0 {
        theme::HIGHLIGHT
    } else {
        theme::TEXT_DIM
    };
    let response = branch(
        ui,
        state,
        &key,
        depth,
        (Icon::Browser, color),
        label,
        false,
        has_match,
        |ui, state| {
            let entries = state.browser.listing(dir).to_vec();
            if entries.is_empty() {
                dim_line(ui, depth + 1, "no sounds");
            }
            for (name, path, is_dir) in entries {
                if is_dir {
                    if depth < MAX_DEPTH {
                        folder(ui, state, &path, &name, depth + 1, actions);
                    }
                } else if matches(&search, &name) {
                    item(
                        ui,
                        depth + 1,
                        &name,
                        Dragged::Sample(path.display().to_string()),
                        actions,
                    );
                }
            }
        },
    );
    if let Some(response) = response {
        response.hint(format!("{} · click to open", dir.display()));
    }
}

/// The built-in sounds.
fn sound_packs(ui: &mut egui::Ui, state: &mut UiState, actions: &mut Vec<Action>) {
    let search = state.browser.search.clone();
    let sounds: Vec<_> = BUILTIN_SAMPLES
        .iter()
        .filter(|n| matches(&search, n))
        .collect();
    let folder = (Icon::Browser, theme::TEXT_DIM);
    branch(
        ui,
        state,
        "packs",
        0,
        (Icon::Pack, theme::ACCENT),
        "Sound packs",
        false,
        !sounds.is_empty(),
        |ui, state| {
            branch(
                ui,
                state,
                "packs/drums",
                1,
                folder,
                "Drums",
                false,
                true,
                |ui, _| {
                    for name in sounds {
                        item(
                            ui,
                            2,
                            &name.replace('_', " "),
                            Dragged::Sample(format!("{BUILTIN_PREFIX}{name}")),
                            actions,
                        );
                    }
                },
            );
        },
    );
}

/// The header: window menu, fold all, refresh, title, then collapse and
/// close.
fn header(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        ui.set_min_height(20.0);
        ui.spacing_mut().item_spacing.x = 2.0;
        let popup = egui::Id::new("browser-menu");
        let open = egui::Popup::is_id_open(ui.ctx(), popup);
        let menu = icons::header_button(ui, Icon::Menu, open, false, "Browser options");
        let toggle =
            (menu.clicked() || menu.secondary_clicked()).then_some(egui::SetOpenCommand::Toggle);
        egui::Popup::menu(&menu)
            .id(popup)
            .open_memory(toggle)
            .show(|ui| {
                if ui.button("Fold all").clicked() {
                    state.browser.fold_all();
                }
                if ui.button("Refresh folders").clicked() {
                    state.browser.listings.clear();
                }
                if ui.button("Add folder…").clicked() {
                    state.pick_file(FilePick::add_folder());
                }
                if ui.button("Add channels from files…").clicked() {
                    state.pick_file(FilePick::add_sample(frame.project));
                }
                if frame.plugins.available && ui.button("Rescan plugins").clicked() {
                    actions.push(Action::RescanPlugins);
                }
                ui.separator();
                let label = if state.browser.collapsed {
                    "Expand"
                } else {
                    "Collapse to header"
                };
                if ui.button(label).clicked() {
                    state.browser.collapsed = !state.browser.collapsed;
                }
            });
        ui.add_space(4.0);
        if icons::header_button(ui, Icon::FoldAll, false, false, "Fold all").clicked() {
            state.browser.fold_all();
        }
        if icons::header_button(
            ui,
            Icon::Refresh,
            false,
            false,
            "Refresh folders: read them from disk again",
        )
        .clicked()
        {
            state.browser.listings.clear();
        }
        ui.add_space(6.0);
        // The title gives way to the two window buttons in a narrow column.
        let title = RichText::new("Browser")
            .size(theme::FONT_TITLE)
            .color(theme::TEXT_DIM);
        let galley = egui::WidgetText::from(title.clone()).into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        );
        let buttons = 2.0 * 18.0 + 3.0 * ui.spacing().item_spacing.x;
        if galley.size().x + buttons <= ui.available_width() {
            ui.label(title);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if icons::header_button(ui, Icon::Close, false, true, "Close (Alt+F8)").clicked() {
                state.windows.browser = false;
            }
            let (icon, hint) = if state.browser.collapsed {
                (Icon::Unfold, "Expand")
            } else {
                (Icon::Fold, "Collapse to header")
            };
            if icons::header_button(ui, icon, false, false, hint).clicked() {
                state.browser.collapsed = !state.browser.collapsed;
            }
        });
    });
}

/// The tab strip: equal-width tabs, glyphs in a narrow column and words in
/// a wide one.
fn tabs(ui: &mut egui::Ui, state: &mut UiState) {
    let width = ui.available_width();
    let words = width >= 260.0;
    let cell = width / Tab::ALL.len() as f32;
    let (strip, _) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
    for (i, tab) in Tab::ALL.into_iter().enumerate() {
        let rect = Rect::from_min_size(
            strip.min + vec2(cell * i as f32, 0.0),
            vec2(cell, strip.height()),
        )
        .shrink2(vec2(1.5, 2.0));
        let response = ui
            .interact(rect, egui::Id::new(("browser-tab", i)), Sense::click())
            .hint(tab.label());
        let active = state.browser.tab == tab;
        let color = if active {
            theme::ACCENT
        } else if response.hovered() {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        };
        if active {
            ui.painter().rect_stroke(
                rect,
                3.0,
                Stroke::new(1.0, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        if words {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                tab.label(),
                FontId::proportional(theme::FONT_LABEL + 1.0),
                color,
            );
        } else {
            icons::paint(
                ui.painter(),
                Rect::from_center_size(rect.center(), vec2(18.0, 18.0)),
                tab.icon(),
                color,
            );
        }
        if response.clicked() {
            state.browser.tab = tab;
        }
    }
}

/// The narrowest the column can be dragged, in points: the header buttons, the
/// tab glyphs and the search field still fit. The contents must fit this
/// width, or a drag that clamps the panel here paints it narrower than its
/// contents and leaves an unpainted strip beside it until the drag ends.
pub const MIN_WIDTH: f32 = 140.0;

pub fn show(ui: &mut egui::Ui, state: &mut UiState, frame: &Frame<'_>, actions: &mut Vec<Action>) {
    egui::Panel::left("browser")
        .resizable(true)
        .default_size(170.0)
        .min_size(MIN_WIDTH)
        .frame(
            egui::Frame::new()
                .fill(theme::PANEL)
                .inner_margin(egui::Margin::symmetric(6, 6)),
        )
        .show(ui, |ui| {
            header(ui, state, frame, actions);
            if state.browser.collapsed {
                return;
            }
            // The keys act on the rows drawn last frame; this frame records
            // them afresh.
            let rows = ui
                .data_mut(|d| d.remove_temp::<Vec<Drawn>>(drawn_id()))
                .unwrap_or_default();
            let over = ui.rect_contains_pointer(ui.max_rect());
            if over
                && !ui.ctx().egui_wants_keyboard_input()
                && let Some(sample) = keys(ui.ctx(), state, &rows)
            {
                send(ui.ctx(), state, frame.project, sample, actions);
            }
            tabs(ui, state);
            let footer = 24.0;
            let well = egui::Frame::new()
                .fill(theme::BACKGROUND)
                .corner_radius(2)
                .inner_margin(egui::Margin::symmetric(0, 2));
            well.show(ui, |ui| {
                ui.set_min_height((ui.available_height() - footer - 6.0).max(40.0));
                let area = egui::ScrollArea::vertical()
                    .id_salt("browser-scroll")
                    .auto_shrink([false, false])
                    .max_height((ui.available_height() - footer - 6.0).max(40.0));
                let bars = (egui::Id::new("browser-bars"), 20.0);
                crate::scrollbars::plain_area_left(ui, area, bars, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    let before = ui.next_widget_position();
                    let tab = state.browser.tab;
                    if tab.shows(Tab::Project) {
                        this_project(ui, state, frame, actions);
                    }
                    if tab.shows(Tab::Plugins) {
                        instruments_and_effects(ui, state, frame, actions);
                    }
                    if tab.shows(Tab::Presets) {
                        presets(ui, state, actions);
                    }
                    if tab == Tab::All {
                        sound_packs(ui, state, actions);
                        for dir in frame.settings.browser_folders.clone() {
                            let label = dir.file_name().map_or_else(
                                || dir.display().to_string(),
                                |n| n.to_string_lossy().into_owned(),
                            );
                            folder(ui, state, &dir, &label, 0, actions);
                        }
                    }
                    if ui.next_widget_position() == before {
                        dim_line(
                            ui,
                            0,
                            if state.browser.search.is_empty() {
                                "Nothing here yet"
                            } else {
                                "No matches"
                            },
                        );
                    }
                });
            });
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut state.browser.search)
                    .id(search_id())
                    .hint_text("Search…")
                    .desired_width(f32::INFINITY),
            )
            .hint("Search (Ctrl+F): names, ignoring case; folders with matches open on their own");
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragged_items_add_the_right_channel() {
        let kick = Dragged::Sample("builtin:kick".into());
        assert_eq!(
            kick.add_channel().unwrap().to_string(),
            "channel.add_sampler builtin:kick"
        );
        assert_eq!(kick.label(), "kick");
        let file = Dragged::Sample("/x/My Loop.wav".into());
        assert_eq!(
            file.add_channel().unwrap().to_string(),
            "channel.add_sampler \"/x/My Loop.wav\""
        );
        assert_eq!(file.label(), "My Loop.wav");
        let synth = Dragged::Generator {
            label: "Synth".into(),
            add: Invocation::bare("channel.add_synth"),
        };
        assert_eq!(
            synth.add_channel().unwrap().to_string(),
            "channel.add_synth"
        );
        let delay = Dragged::Effect {
            label: "Delay".into(),
            command: "mixer.add_effect".into(),
            args: vec!["delay".into()],
        };
        assert!(delay.add_channel().is_none());
        assert_eq!(
            delay.add_effect(3).unwrap().to_string(),
            "mixer.add_effect 3 delay"
        );
        assert_eq!(synth.sample(), None);
    }

    /// Clicks a row drawn by `draw` `clicks` times in quick succession and
    /// returns the actions it asked for.
    fn click_row(clicks: usize, draw: impl Fn(&mut egui::Ui, &mut Vec<Action>)) -> Vec<Action> {
        let ctx = egui::Context::default();
        let at = egui::pos2(30.0, 14.0);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        let mut frames = vec![vec![egui::Event::PointerMoved(at)]];
        for _ in 0..clicks {
            frames.push(vec![press(true)]);
            frames.push(vec![press(false)]);
        }
        frames.push(vec![]);
        let mut all = Vec::new();
        for (i, events) in frames.into_iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 200.0),
                )),
                time: Some(i as f64 * 0.05),
                events,
                ..Default::default()
            };
            let mut actions = Vec::new();
            ctx.run_ui(input, |ui| draw(ui, &mut actions))
                .textures_delta
                .clear();
            all.extend(actions);
        }
        all
    }

    /// Clicks a Synth generator entry `clicks` times in quick succession and
    /// returns the commands it asked for.
    fn click_generator(clicks: usize) -> Vec<String> {
        let actions = click_row(clicks, |ui, actions| {
            generator(
                ui,
                0,
                "Synth",
                "Subtractive synth",
                Invocation::bare("channel.add_synth"),
                actions,
            );
        });
        actions
            .into_iter()
            .filter_map(|a| match a {
                Action::Run(invocation) => Some(invocation.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_click_previews_a_preset_and_a_double_click_adds_it() {
        let click = |clicks| {
            click_row(clicks, |ui, actions| {
                preset_row(ui, "Glass Reese", "Antiserum preset", actions);
            })
        };
        let preview = Action::PreviewPreset("Glass Reese".to_owned());
        assert_eq!(click(1), std::slice::from_ref(&preview));
        let twice = click(2);
        assert!(twice.contains(&preview), "{twice:?}");
        assert!(twice.contains(&Action::Run(Invocation::new(
            "channel.add_antiserum",
            ["Glass Reese".to_owned()]
        ))));
    }

    #[test]
    fn a_click_selects_a_generator_and_a_double_click_adds_one_channel() {
        assert!(click_generator(1).is_empty(), "a single click only selects");
        assert_eq!(click_generator(2), ["channel.add_synth"]);
    }

    #[test]
    fn folder_listing_shows_dirs_then_wavs() {
        let dir = std::env::temp_dir().join(format!("musiccraft-browser-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Drums")).unwrap();
        std::fs::write(dir.join("b.WAV"), b"").unwrap();
        std::fs::write(dir.join("a.wav"), b"").unwrap();
        std::fs::write(dir.join("notes.txt"), b"").unwrap();
        let mut browser = BrowserState::default();
        let names: Vec<String> = browser.listing(&dir).iter().map(|e| e.0.clone()).collect();
        assert_eq!(names, ["Drums", "a.wav", "b.WAV"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn search_finds_sounds_inside_closed_folders() {
        let dir = std::env::temp_dir().join(format!("musiccraft-search-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Drums").join("Kicks")).unwrap();
        std::fs::write(dir.join("Drums").join("Kicks").join("Deep Kick.wav"), b"").unwrap();
        std::fs::create_dir_all(dir.join("Pads")).unwrap();
        let mut browser = BrowserState::default();
        assert!(browser.folder_matches(&dir, "kick", 0));
        assert!(browser.folder_matches(&dir.join("Drums"), "KICK", 0));
        assert!(!browser.folder_matches(&dir.join("Pads"), "kick", 0));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn searching_opens_branches_and_fold_all_closes_them() {
        let mut browser = BrowserState::default();
        browser.open.insert("project".into(), true);
        assert!(browser.is_open("project", false));
        browser.fold_all();
        assert!(
            !browser.is_open("project", true),
            "fold all closes default-open branches too"
        );
        browser.search = "kick".into();
        assert!(
            browser.is_open("project", false),
            "every drawn branch is open while searching"
        );
    }
}
