//! Drives the real views headlessly with synthetic input and checks the commands
//! they produce.

use std::collections::BTreeSet;

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use musiccraft_commands::{Registry, Session};
use musiccraft_project::Project;
use musiccraft_project::params::TICKS_PER_STEP;

use crate::piano_roll::row_of_key;
use crate::{
    Action, EngineStatus, Frame, History, LivePluginView, PluginEntry, PluginParamView, Plugins,
    Settings, SettingsInfo, UiState, show,
};

/// A 4/4 bar.
const TICKS_PER_BAR: u32 = 384;

struct Harness {
    ctx: egui::Context,
    state: UiState,
    session: Session,
    time: f64,
    /// Modifier keys held during the next frames.
    modifiers: Modifiers,
    /// Plugin information handed to the views.
    plugins: Plugins,
    /// Actions other than commands from the last frames.
    other: Vec<Action>,
    /// The pointer shape the last frame asked for.
    cursor: egui::CursorIcon,
    /// What the engine reports to the views.
    engine: EngineStatus,
}

impl Harness {
    fn new(setup: &[&str]) -> Self {
        let mut session = Session::new(Project::default(), Registry::with_builtin());
        for line in setup {
            session.run_line(line).unwrap();
        }
        // Tests open the windows they need; the playlist floats unless a
        // test asks for the backdrop.
        let mut state = UiState {
            arrange: None,
            ..UiState::default()
        };
        state.windows.channel_rack = false;
        state.windows.playlist = false;
        state.windows.browser = false;
        state.windows.mixer = false;
        state.windows.piano_roll = false;
        Self {
            ctx: egui::Context::default(),
            state,
            session,
            time: 0.0,
            modifiers: Modifiers::NONE,
            plugins: Plugins::default(),
            other: Vec::new(),
            cursor: egui::CursorIcon::Default,
            engine: EngineStatus::default(),
        }
    }

    /// Runs one frame with `events` and returns the project commands produced
    /// (previews are dropped), applying them to the session.
    fn frame(&mut self, mut events: Vec<Event>) -> Vec<String> {
        events.insert(0, Event::ModifiersChanged(self.modifiers));
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        self.time += 1.0 / 60.0;
        let (undo, redo) = self.session.history();
        let history = History {
            steps: undo.iter().take(15).map(ToString::to_string).collect(),
            redo_steps: redo.iter().take(15).map(ToString::to_string).collect(),
            ..History::default()
        };
        let mut actions = Vec::new();
        {
            let Self {
                ctx,
                state,
                session,
                plugins,
                engine,
                ..
            } = self;
            let frame = Frame {
                project: session.project(),
                engine,
                history: &history,
                waveform: None,
                waveform_seconds: None,
                clip_waveforms: &[],
                clip_durations: &[],
                plugins,
                settings: &Settings::default(),
                settings_info: &SettingsInfo::default(),
            };
            let mut output = ctx.run_ui(input, |ui| actions = show(ui, state, &frame));
            // No renderer here: acknowledge texture uploads.
            output.textures_delta.clear();
            self.cursor = output.platform_output.cursor_icon;
        }
        let mut commands = Vec::new();
        for action in actions {
            match action {
                Action::Run(invocation) => {
                    commands.push(invocation.to_string());
                    self.session.run(&invocation).unwrap();
                }
                Action::Coalesced(invocation, key) => {
                    commands.push(invocation.to_string());
                    self.session.run_coalesced(&invocation, key).unwrap();
                }
                other => self.other.push(other),
            }
        }
        commands
    }

    fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(Vec::new());
        }
    }

    fn press(&mut self, pos: Pos2, button: PointerButton) -> Vec<String> {
        self.frame(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: self.modifiers,
            },
        ])
    }

    fn release(&mut self, pos: Pos2, button: PointerButton) -> Vec<String> {
        let modifiers = self.modifiers;
        self.frame(vec![Event::PointerButton {
            pos,
            button,
            pressed: false,
            modifiers,
        }])
    }

    fn click(&mut self, pos: Pos2, button: PointerButton) -> Vec<String> {
        let mut out = self.press(pos, button);
        out.extend(self.release(pos, button));
        out
    }

    fn drag(&mut self, from: Pos2, to: Pos2) -> Vec<String> {
        self.drag_with(from, to, PointerButton::Primary)
    }

    fn drag_with(&mut self, from: Pos2, to: Pos2, button: PointerButton) -> Vec<String> {
        let mut out = self.press(from, button);
        for i in 1..=6 {
            let t = i as f32 / 6.0;
            out.extend(self.frame(vec![Event::PointerMoved(from + (to - from) * t)]));
        }
        out.extend(self.release(to, button));
        out
    }

    fn roll_point(&self, tick: u32, key: u8) -> Pos2 {
        let (grid, clip) = self.state.roll.last_view.expect("piano roll drawn");
        let p = pos2(
            grid.x(tick as f32),
            grid.y(row_of_key(key)) + grid.row_height / 2.0,
        );
        assert!(clip.contains(p), "{p:?} not visible in {clip:?}");
        p
    }

    fn playlist_point(&self, tick: u32, track: u32) -> Pos2 {
        let (grid, clip) = self.state.playlist.last_view.expect("playlist drawn");
        let p = pos2(
            grid.x(tick as f32),
            grid.y(track) + grid.height_of(track) / 2.0,
        );
        assert!(clip.contains(p), "{p:?} not visible in {clip:?}");
        p
    }
}

#[test]
fn piano_roll_click_drag_resize_delete() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.piano_roll = true;
    h.settle();

    let step = TICKS_PER_STEP;
    let added = h.click(h.roll_point(2 * step + 5, 62), PointerButton::Primary);
    assert_eq!(added, ["note.add 1 48t 24t 62 100"]);
    h.settle();

    let moved = h.drag(
        h.roll_point(2 * step + 6, 62),
        h.roll_point(5 * step + 6, 64),
    );
    assert_eq!(moved, ["note.move 1 48t 62 120t 64"]);
    h.settle();

    // Grab the right edge of the note (it spans 120..144 ticks).
    let edge = h.roll_point(6 * step, 64) - vec2(2.0, 0.0);
    let resized = h.drag(edge, h.roll_point(9 * step, 64));
    assert_eq!(resized, ["note.resize 1 120t 64 96t"]);
    h.settle();

    let removed = h.click(h.roll_point(6 * step, 64), PointerButton::Secondary);
    assert_eq!(removed, ["note.remove 1 120t 64"]);
    assert!(h.session.project().current_pattern().notes.is_empty());

    // New notes use the last drawn length.
    let added = h.click(h.roll_point(2, 60), PointerButton::Primary);
    assert_eq!(added, ["note.add 1 0t 96t 60 100"]);
}

#[test]
fn playlist_place_move_delete() {
    let mut h = Harness::new(&["pattern.add Verse"]);
    h.state.windows.playlist = true;
    h.settle();

    let placed = h.click(
        h.playlist_point(TICKS_PER_BAR + 10, 1),
        PointerButton::Primary,
    );
    assert_eq!(placed, ["playlist.add_clip 2 2 384t"]);
    h.settle();

    let moved = h.drag(
        h.playlist_point(TICKS_PER_BAR + 40, 1),
        h.playlist_point(3 * TICKS_PER_BAR + 40, 3),
    );
    assert_eq!(moved, ["playlist.move_clip 2 384t 4 1152t"]);
    h.settle();

    let removed = h.click(
        h.playlist_point(3 * TICKS_PER_BAR + 40, 3),
        PointerButton::Secondary,
    );
    assert_eq!(removed, ["playlist.remove_clip 4 1152t"]);
    assert!(h.session.project().playlist.clips.is_empty());
}

#[test]
fn single_key_tools_slice_and_mute() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 4 C5"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let step = TICKS_PER_STEP;
    // C picks the slice tool everywhere; a click cuts the note on the grid.
    assert!(h.key(egui::Key::C, Modifiers::NONE).is_empty());
    assert_eq!(h.state.roll.tool, crate::piano_roll::Tool::Slice);
    assert_eq!(h.state.playlist.tool, crate::piano_roll::Tool::Slice);
    let cut = h.click(h.roll_point(2 * step + 3, 60), PointerButton::Primary);
    assert_eq!(cut, ["note.slice 1 0t 60 48t"]);
    // S and O toggle slide and porta for new notes; P is draw again.
    h.key(egui::Key::S, Modifiers::NONE);
    h.key(egui::Key::O, Modifiers::NONE);
    assert!(h.state.roll.slide && h.state.roll.porta);
    h.key(egui::Key::P, Modifiers::NONE);
    assert_eq!(h.state.roll.tool, crate::piano_roll::Tool::Draw);
}

#[test]
fn playlist_slice_and_mute_tools() {
    let mut h = Harness::new(&["pattern.add", "playlist.add_clip 1 1 0 2"]);
    h.state.windows.playlist = true;
    h.settle();
    h.key(egui::Key::C, Modifiers::NONE);
    // On the playlist the slice tool cuts the clip.
    let cut = h.click(
        h.playlist_point(TICKS_PER_BAR + 2, 0),
        PointerButton::Primary,
    );
    assert_eq!(cut, ["playlist.slice_clip 1 384t"]);
    h.settle();
    // Shift+click splits the pattern clip into two new patterns instead (no
    // bar line inside the one-bar clip, so at the unsnapped click).
    h.modifiers = Modifiers::SHIFT;
    let split = h.click(
        h.playlist_point(TICKS_PER_BAR / 2 + 2, 0),
        PointerButton::Primary,
    );
    h.modifiers = Modifiers::NONE;
    assert_eq!(split, ["playlist.split_clip 1 194t"]);
    h.settle();
    // T: the mute tool mutes the clicked clip.
    h.key(egui::Key::T, Modifiers::NONE);
    let muted = h.click(
        h.playlist_point(TICKS_PER_BAR + 40, 0),
        PointerButton::Primary,
    );
    assert_eq!(muted, ["playlist.toggle_clip_mute 1 384t"]);
    assert!(h.session.project().playlist.clips[2].muted);
    // The split pieces play patterns of their own.
    let clips = &h.session.project().playlist.clips;
    assert_ne!(clips[0].pattern(), clips[1].pattern());
}

#[test]
fn number_keys_select_patterns_or_mute_channels_over_the_rack() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "channel.add_synth",
        "pattern.add",
        "pattern.select 1",
    ]);
    h.settle();
    // Anywhere else, 2 selects pattern 2.
    assert_eq!(
        h.key(egui::Key::Num2, Modifiers::NONE),
        ["pattern.select 2"]
    );
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new("channel-rack")))
        .expect("rack drawn");
    h.frame(vec![Event::PointerMoved(rack.center())]);
    assert_eq!(
        h.key(egui::Key::Num2, Modifiers::NONE),
        ["channel.toggle_mute 2"]
    );
    assert_eq!(
        h.key(egui::Key::Num1, Modifiers::COMMAND),
        ["channel.solo 1"]
    );
    h.state.selected_channel = 1;
    assert_eq!(
        h.key(egui::Key::ArrowUp, Modifiers::ALT),
        ["channel.move 2 1"]
    );
    assert_eq!(h.state.selected_channel, 0);
    assert_eq!(
        h.key(egui::Key::L, Modifiers::COMMAND),
        ["channel.route_free 1"]
    );
    assert_eq!(h.session.project().channels[0].mixer_track, 1);
    assert_eq!(
        h.key(egui::Key::ArrowRight, Modifiers::COMMAND | Modifiers::SHIFT),
        ["step.shift 1 1"]
    );
}

#[test]
fn the_step_graph_editor_edits_the_note_under_a_bar() {
    let mut h = Harness::new(&["channel.add_synth", "step.toggle 1 0", "step.toggle 1 4"]);
    h.state.windows.channel_rack = true;
    h.state.graph = Some(crate::piano_roll::LaneMode::Velocity);
    h.settle();
    let (rect, steps) = h.state.graph_view.expect("graph drawn");
    let width = rect.width() / steps as f32;
    // High on the bar of step 4: a loud velocity for the note there.
    let found = h.click(
        pos2(rect.left() + width * 4.5, rect.top() + 8.0),
        PointerButton::Primary,
    );
    assert!(
        found
            .iter()
            .any(|c| c.starts_with("note.set_velocity 1 96t 60")),
        "{found:?}"
    );
}

#[test]
fn shortcuts_run_commands_but_not_while_typing() {
    let mut h = Harness::new(&[]);
    h.settle();
    let space = |pressed| Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(h.frame(vec![space(true)]), ["transport.toggle"]);
    h.frame(vec![space(false)]);
    assert!(h.session.project().transport().is_playing());

    let undo = Event::Key {
        key: egui::Key::Z,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    };
    h.session.run_line("project.set_tempo 99").unwrap();
    assert_eq!(h.frame(vec![undo]), ["edit.undo"]);

    // F9 toggles the mixer window without touching the project.
    let f9 = Event::Key {
        key: egui::Key::F9,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    assert!(h.frame(vec![f9]).is_empty());
    assert!(h.state.windows.mixer);

    // With a text field focused (rename prompt), Space types instead.
    h.state
        .open_dialog(crate::dialogs::Dialog::rename_pattern(0, "P"));
    h.settle();
    assert!(h.frame(vec![space(true)]).is_empty());
}

#[test]
fn typing_keyboard_performs_notes_instead_of_shortcuts() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.settle();
    let key = |key, pressed| Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    // Off: L toggles the play mode as usual.
    assert_eq!(
        h.frame(vec![key(egui::Key::L, true)]),
        ["transport.toggle_mode"]
    );
    h.state.typing_keyboard = true;
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))),
        events: vec![
            key(egui::Key::Z, true),
            key(egui::Key::Q, true),
            key(egui::Key::Z, false),
        ],
        ..Default::default()
    };
    let engine = EngineStatus::default();
    let history = History::default();
    let frame = Frame {
        project: h.session.project(),
        engine: &engine,
        history: &history,
        waveform: None,
        waveform_seconds: None,
        clip_waveforms: &[],
        clip_durations: &[],
        plugins: &Plugins::default(),
        settings: &Settings::default(),
        settings_info: &SettingsInfo::default(),
    };
    let mut actions = Vec::new();
    let mut out = h
        .ctx
        .run_ui(input, |ui| actions = show(ui, &mut h.state, &frame));
    out.textures_delta.clear();
    assert_eq!(
        actions,
        [
            Action::Perform {
                channel: 0,
                key: 48,
                on: true
            },
            Action::Perform {
                channel: 0,
                key: 60,
                on: true
            },
            Action::Perform {
                channel: 0,
                key: 48,
                on: false
            },
        ]
    );
}

#[test]
fn velocity_lane_drag_is_one_undo_step() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 4 C5 100"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let (grid, clip) = h.state.roll.last_view.unwrap();
    let x = grid.x(10.0);
    let from = pos2(x, clip.bottom() - 45.0);
    let to = pos2(x, clip.bottom() - 90.0);
    let commands = h.drag(from, to);
    assert!(!commands.is_empty());
    assert!(
        commands
            .iter()
            .all(|c| c.starts_with("note.set_velocity 1 0t 60 ")),
        "{commands:?}"
    );
    let velocity = h.session.project().current_pattern().notes[0].velocity;
    assert!(velocity > 100, "dragged up to {velocity}");
    assert_eq!(
        h.session.undo_depth(),
        3,
        "two setup commands plus one gesture"
    );
    h.session.run_line("edit.undo").unwrap();
    assert_eq!(h.session.project().current_pattern().notes[0].velocity, 100);
}

#[test]
fn alt_click_on_a_lane_stem_resets_it_to_the_default() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 4 C5 40"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let (grid, clip) = h.state.roll.last_view.unwrap();
    let stem = pos2(grid.x(10.0), clip.bottom() - 20.0);
    h.modifiers = Modifiers::ALT;
    let out = h.click(stem, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(out, ["note.set_velocity 1 0t 60 100"]);
    assert_eq!(h.session.project().current_pattern().notes[0].velocity, 100);
    // Already at the default: nothing to do.
    h.modifiers = Modifiers::ALT;
    let out = h.click(stem, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert!(out.is_empty(), "{out:?}");
}

impl Harness {
    fn key(&mut self, key: egui::Key, modifiers: Modifiers) -> Vec<String> {
        let held = self.modifiers;
        self.modifiers = modifiers;
        let event = |pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers,
        };
        let out = self.frame(vec![event(true), event(false)]);
        self.modifiers = held;
        out
    }

    fn roll_notes(&self) -> Vec<(u32, u8)> {
        self.session
            .project()
            .current_pattern()
            .notes
            .iter()
            .map(|n| (n.start, n.key))
            .collect()
    }

    /// Keeps the pointer over the piano roll so its editing keys are active.
    fn hover_roll(&mut self, tick: u32, key: u8) {
        let p = self.roll_point(tick, key);
        self.frame(vec![Event::PointerMoved(p)]);
    }
}

#[test]
fn piano_roll_selection_editing() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "note.add 1 0 1 60",
        "note.add 1 2 1 62",
        "note.add 1 8 1 64",
    ]);
    h.state.windows.piano_roll = true;
    // Compact rows, so the octave moves below stay in the small window.
    h.state.roll.key_height = 14.0;
    h.settle();
    let step = TICKS_PER_STEP;

    // Ctrl+drag a box around the first two notes.
    h.modifiers = Modifiers::COMMAND;
    let selected = h.drag(
        h.roll_point(step / 2 + 300, 66),
        h.roll_point(step / 2 + 300, 58),
    );
    assert!(selected.is_empty(), "selecting runs no commands");
    h.modifiers = Modifiers::NONE;
    // The box above started right of every note: nothing selected yet.
    assert!(h.state.roll.selection.is_empty());
    h.modifiers = Modifiers::COMMAND;
    h.drag(h.roll_point(3 * step + 6, 63), h.roll_point(2, 59));
    h.modifiers = Modifiers::NONE;
    assert_eq!(h.state.roll.selection.len(), 2);

    // Drag one of them: both move two steps right.
    let moved = h.drag(h.roll_point(6, 60), h.roll_point(2 * step + 6, 60));
    assert_eq!(moved.len(), 1);
    assert!(moved[0].starts_with("edit.batch"), "{moved:?}");
    assert_eq!(h.roll_notes(), [(48, 60), (96, 62), (192, 64)]);
    h.session.run_line("edit.undo").unwrap();
    assert_eq!(h.roll_notes(), [(0, 60), (48, 62), (192, 64)]);
    h.session.run_line("edit.redo").unwrap();
    h.settle();

    // Plain arrows leave notes alone; Shift+Up transposes the selection a
    // semitone, Ctrl+Up/Down an octave, Shift+Left/Right a snap step.
    h.hover_roll(10 * step, 60);
    h.key(egui::Key::ArrowUp, Modifiers::NONE);
    assert_eq!(h.roll_notes(), [(48, 60), (96, 62), (192, 64)]);
    h.key(egui::Key::ArrowUp, Modifiers::COMMAND);
    assert_eq!(h.roll_notes(), [(48, 72), (96, 74), (192, 64)]);
    h.key(egui::Key::ArrowDown, Modifiers::COMMAND);
    h.key(egui::Key::ArrowRight, Modifiers::SHIFT);
    h.key(egui::Key::ArrowLeft, Modifiers::SHIFT);
    assert_eq!(h.roll_notes(), [(48, 60), (96, 62), (192, 64)]);
    h.key(egui::Key::ArrowUp, Modifiers::SHIFT);
    assert_eq!(h.roll_notes(), [(48, 61), (96, 63), (192, 64)]);
    // Ctrl+B duplicates the selection to the right.
    h.key(egui::Key::B, Modifiers::COMMAND);
    assert_eq!(h.roll_notes().len(), 5);
    assert!(
        h.roll_notes().contains(&(120, 61)),
        "copy starts where the selection ends: {:?}",
        h.roll_notes()
    );

    // Ctrl+A then Delete clears the pattern in one undo step.
    h.key(egui::Key::A, Modifiers::COMMAND);
    h.key(egui::Key::Delete, Modifiers::NONE);
    assert!(h.roll_notes().is_empty());
    h.session.run_line("edit.undo").unwrap();
    assert_eq!(h.roll_notes().len(), 5);

    // Copy one note and paste it where the pointer is.
    h.settle();
    h.click(h.roll_point(192 + 6, 64), PointerButton::Primary);
    h.key(egui::Key::C, Modifiers::COMMAND);
    h.hover_roll(12 * step + 2, 66);
    h.key(egui::Key::V, Modifiers::COMMAND);
    assert!(h.roll_notes().contains(&(288, 64)), "{:?}", h.roll_notes());
}

#[test]
fn f4_goes_to_the_next_empty_pattern_and_offers_to_name_it() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 1 C5"]);
    h.settle();
    assert_eq!(
        h.key(egui::Key::F4, Modifiers::NONE),
        ["pattern.next_empty"]
    );
    assert_eq!(h.session.project().patterns.len(), 2);
    assert!(h.state.dialog.is_some(), "a rename prompt for pattern 2");
    h.settle();
    let mut typed = h.frame(vec![
        Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        },
        Event::Text("Drop".to_owned()),
    ]);
    typed.extend(h.frame(vec![Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]));
    assert_eq!(typed, ["pattern.rename 2 Drop"]);
}

#[test]
fn playlist_selection_editing() {
    let mut h = Harness::new(&[
        "pattern.add",
        "playlist.add_clip 1 1 0",
        "playlist.add_clip 2 2 0",
        "playlist.add_clip 1 1 4",
    ]);
    h.state.windows.playlist = true;
    h.settle();
    let clips = |h: &Harness| -> Vec<(u32, u32)> {
        h.session
            .project()
            .playlist
            .clips
            .iter()
            .map(|c| (c.track, c.start))
            .collect()
    };
    let bar = TICKS_PER_BAR;

    // Shift+click selects the first two clips; dragging one moves both.
    h.click(h.playlist_point(40, 0), PointerButton::Primary);
    h.modifiers = Modifiers::SHIFT;
    h.click(h.playlist_point(40, 1), PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(h.state.playlist.selection.len(), 2);
    let moved = h.drag(h.playlist_point(40, 0), h.playlist_point(bar + 40, 0));
    assert!(moved[0].starts_with("edit.batch"), "{moved:?}");
    assert_eq!(clips(&h), [(0, bar), (0, 4 * bar), (1, bar)]);

    // Ctrl+B duplicates the selection right after itself; Delete removes it.
    h.settle();
    h.frame(vec![Event::PointerMoved(h.playlist_point(10 * bar, 5))]);
    h.key(egui::Key::B, Modifiers::COMMAND);
    assert_eq!(
        clips(&h),
        [(0, bar), (0, 2 * bar), (0, 4 * bar), (1, bar), (1, 2 * bar)]
    );
    h.key(egui::Key::Delete, Modifiers::NONE);
    assert_eq!(clips(&h), [(0, bar), (0, 4 * bar), (1, bar)]);

    // Ctrl+drag a box over track 1 selects both of its clips.
    h.settle();
    h.modifiers = Modifiers::COMMAND;
    h.drag(h.playlist_point(6 * bar, 0), h.playlist_point(10, 0));
    h.modifiers = Modifiers::NONE;
    assert_eq!(
        h.state.playlist.selection,
        BTreeSet::from([(0, bar), (0, 4 * bar)])
    );
}

impl Harness {
    /// Opens the first automation in the editor and returns a function
    /// placing a tick and value on its graph.
    fn open_automation(&mut self) -> impl Fn(u32, f32) -> Pos2 + use<> {
        let id = self.session.project().automations[0].id;
        self.state.automation_editor = Some(id);
        self.settle();
        let view = self.state.automation_view.expect("editor drawn");
        move |tick, value| view.at(tick, value)
    }
}

#[test]
fn automation_editor_adds_moves_and_removes_points() {
    let mut h = Harness::new(&["channel.add_synth", "automation.create channel:1:cutoff"]);
    let at = h.open_automation();
    let before = h.session.project().automations[0].points.clone();
    let v = before[0].value;

    // Left-click on the empty grid only clears the selection.
    assert!(
        h.click(at(TICKS_PER_BAR, 0.25), PointerButton::Primary)
            .is_empty()
    );
    let added = h.click(at(TICKS_PER_BAR, 0.25), PointerButton::Secondary);
    assert_eq!(added.len(), 1);
    assert!(
        added[0].starts_with("automation.add_point 1 384t 0.2"),
        "{added:?}"
    );
    h.settle();

    let moved = h.drag(at(TICKS_PER_BAR, 0.25), at(2 * TICKS_PER_BAR, 0.9));
    assert!(
        moved[0].starts_with("automation.move_point 1 384t 768t 0.9"),
        "{moved:?}"
    );
    h.settle();

    h.modifiers = Modifiers::ALT;
    let removed = h.click(at(2 * TICKS_PER_BAR, 0.9), PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(removed, ["automation.remove_point 1 768t"]);
    let points = &h.session.project().automations[0].points;
    assert_eq!(points.len(), 2);
    assert!((points[0].value - v).abs() < 1e-6);
}

#[test]
fn right_clicking_a_point_opens_its_menu_instead_of_adding() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "automation.create channel:1:cutoff",
        "automation.set_points 1 [[0,0.0],[768,1.0]]",
    ]);
    let at = h.open_automation();
    assert!(h.click(at(768, 1.0), PointerButton::Secondary).is_empty());
    h.settle();
    assert_eq!(h.session.project().automations[0].points.len(), 2);
}

#[test]
fn a_box_selects_points_and_delete_removes_them_in_one_step() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "automation.create channel:1:cutoff",
        "automation.set_points 1 [[0,0.0],[384,0.5],[768,0.6],[1536,1.0]]",
    ]);
    let at = h.open_automation();
    h.drag(at(300, 0.9), at(900, 0.1));
    assert_eq!(
        h.state.automation_ui.edit.selection,
        BTreeSet::from([384, 768])
    );
    // The editor window has focus after the click in it.
    let depth = h.session.undo_depth();
    let deleted = h.frame(vec![Event::Key {
        key: egui::Key::Delete,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(deleted.len(), 1, "{deleted:?}");
    assert!(deleted[0].starts_with("automation.set_points 1"));
    let ticks: Vec<u32> = h.session.project().automations[0]
        .points
        .iter()
        .map(|p| p.tick)
        .collect();
    assert_eq!(ticks, [0, 1536]);
    assert_eq!(h.session.undo_depth(), depth + 1);
}

#[test]
fn dragging_a_selected_point_moves_the_whole_selection() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "automation.create channel:1:cutoff",
        "automation.set_points 1 [[0,0.0],[384,0.5],[768,0.5],[1536,1.0]]",
    ]);
    let at = h.open_automation();
    h.click(at(384, 0.5), PointerButton::Primary);
    h.modifiers = Modifiers::SHIFT;
    h.click(at(768, 0.5), PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    let moved = h.drag(at(384, 0.5), at(384 + 96, 0.5));
    assert_eq!(moved.len(), 1, "{moved:?}");
    let ticks: Vec<u32> = h.session.project().automations[0]
        .points
        .iter()
        .map(|p| p.tick)
        .collect();
    assert_eq!(ticks, [0, 480, 864, 1536]);
}

#[test]
fn dragging_a_tension_handle_bends_its_segment() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "automation.create channel:1:cutoff",
        "automation.set_points 1 [[0,0.0],[1536,1.0]]",
    ]);
    let at = h.open_automation();
    // The handle sits mid-segment, on the straight line.
    let bent = h.drag(at(768, 0.5), at(768, 0.5) + vec2(0.0, 60.0));
    assert_eq!(bent.len(), 1, "{bent:?}");
    assert!(
        bent[0].starts_with("automation.set_curve 1 0t single 0."),
        "{bent:?}"
    );
    // Right-clicking the handle resets the bend.
    h.settle();
    let point = h.session.project().automations[0].points[0];
    let mid = h.session.project().automations[0].value_at(768.0).unwrap();
    assert!(point.tension > 0.3 && mid < 0.5);
    let reset = h.click(at(768, mid), PointerButton::Secondary);
    assert_eq!(reset, ["automation.set_curve 1 0t single"]);
}

#[test]
fn step_drawing_lays_points_on_the_snap_lines() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "automation.create channel:1:cutoff",
        "automation.set_points 1 [[0,0.0],[1536,1.0]]",
    ]);
    h.state.automation_ui.options.step = true;
    let at = h.open_automation();
    let drawn = h.drag(at(384, 0.8), at(768, 0.8));
    assert_eq!(drawn.len(), 1, "{drawn:?}");
    let points = &h.session.project().automations[0].points;
    assert!(points.len() > 10, "a point per step: {}", points.len());
    assert!(points.iter().all(|p| p.tick % TICKS_PER_STEP == 0));
    assert_eq!(points.first().unwrap().tick, 0);
    assert_eq!(points.last().unwrap().tick, 1536);
}

#[test]
fn automate_request_creates_and_places_a_clip() {
    let mut h = Harness::new(&["channel.add_synth", "playlist.add_clip 1 1 0"]);
    h.settle();
    let engine = EngineStatus::default();
    let history = History::default();
    let frame = Frame {
        project: h.session.project(),
        engine: &engine,
        history: &history,
        waveform: None,
        waveform_seconds: None,
        clip_waveforms: &[],
        clip_durations: &[],
        plugins: &Plugins::default(),
        settings: &Settings::default(),
        settings_info: &SettingsInfo::default(),
    };
    let out = crate::resolve_ui_actions(
        &mut h.state,
        &frame,
        vec![Action::Run(
            musiccraft_commands::Invocation::parse("ui.automate mixer:0:volume").unwrap(),
        )],
    );
    let Action::Run(batch) = &out[0] else {
        panic!("{out:?}")
    };
    h.session.run(batch).unwrap();
    let project = h.session.project();
    assert_eq!(project.automations.len(), 1);
    let clip = project
        .playlist
        .clips
        .iter()
        .find(|c| c.automation().is_some())
        .unwrap();
    assert_eq!(clip.track, 1, "placed on the first free track");
    h.session.run_line("edit.undo").unwrap();
    assert!(h.session.project().automations.is_empty());
}

#[test]
fn ruler_sets_loop_and_jumps_to_markers() {
    let mut h = Harness::new(&["playlist.add_marker 3 Drop"]);
    h.state.windows.playlist = true;
    h.settle();
    let (grid, clip) = h.state.playlist.last_view.unwrap();
    let on_ruler = |tick: u32| pos2(grid.x(tick as f32), clip.top() + 10.0);
    h.modifiers = Modifiers::SHIFT;
    let set = h.drag(
        on_ruler(TICKS_PER_BAR + 10),
        on_ruler(4 * TICKS_PER_BAR - 10),
    );
    h.modifiers = Modifiers::NONE;
    assert_eq!(set, ["transport.set_loop 384t 1536t"]);
    assert_eq!(h.session.project().playlist.loop_range, Some((384, 1536)));
    // Clicking a marker's label seeks exactly to it.
    let engine = EngineStatus::default();
    let history = History::default();
    let pos = on_ruler(3 * TICKS_PER_BAR) + vec2(20.0, 0.0);
    h.frame(vec![Event::PointerMoved(pos)]);
    let mut actions = Vec::new();
    for pressed in [true, false] {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0))),
            time: Some(h.time),
            events: vec![Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }],
            ..Default::default()
        };
        h.time += 1.0 / 60.0;
        let frame = Frame {
            project: h.session.project(),
            engine: &engine,
            history: &history,
            waveform: None,
            waveform_seconds: None,
            clip_waveforms: &[],
            clip_durations: &[],
            plugins: &Plugins::default(),
            settings: &Settings::default(),
            settings_info: &SettingsInfo::default(),
        };
        let mut out = h
            .ctx
            .run_ui(input, |ui| actions.extend(show(ui, &mut h.state, &frame)));
        out.textures_delta.clear();
    }
    assert!(
        actions.contains(&Action::Seek(f64::from(3 * TICKS_PER_BAR))),
        "{actions:?}"
    );
}

#[test]
fn plugin_channels_show_parameters_and_open_their_window() {
    let mut h = Harness::new(&["channel.add_plugin com.example.keys Keys"]);
    let instance = h.session.project().channels[0]
        .instrument
        .plugin()
        .unwrap()
        .instance;
    h.plugins = Plugins {
        available: true,
        catalog: vec![PluginEntry {
            id: "com.example.keys".into(),
            name: "Keys".into(),
            vendor: "Example".into(),
            instrument: true,
            format: "CLAP".into(),
        }],
        live: vec![LivePluginView {
            instance,
            params: vec![PluginParamView {
                id: 7,
                name: "Cutoff".into(),
                module: String::new(),
                min: 0.0,
                max: 10.0,
                default: 5.0,
                stepped: false,
                read_only: false,
                value: 0.0,
                text: "0.0".into(),
            }],
            has_editor: true,
            editor_open: false,
        }],
        running: vec![instance],
    };
    h.state.windows.channel_settings = true;
    h.settle();
    let (button, sliders) = h.state.plugin_layout.clone();
    let slider = sliders
        .iter()
        .find(|(id, _)| *id == 7)
        .map(|(_, r)| *r)
        .expect("slider drawn");
    let depth = h.session.undo_depth();
    let commands = h.drag(slider.left_center() + vec2(2.0, 0.0), slider.right_center());
    assert!(
        commands
            .iter()
            .all(|c| c.starts_with("channel.set_plugin_param 1 7 ")),
        "{commands:?}"
    );
    let value = h.session.project().channels[0]
        .instrument
        .plugin()
        .unwrap()
        .params[&7];
    assert!(value > 9.0, "dragged to the top, got {value}");
    assert_eq!(
        h.session.undo_depth(),
        depth + 1,
        "one drag is one undo step"
    );

    h.other.clear();
    h.click(
        button.expect("window button").center(),
        PointerButton::Primary,
    );
    assert!(h.other.contains(&Action::PluginEditor {
        instance,
        open: true
    }));
}

#[test]
fn missing_plugins_explain_themselves_instead_of_showing_knobs() {
    let mut h = Harness::new(&["channel.add_plugin com.nowhere.synth"]);
    h.plugins.available = true;
    h.state.windows.channel_settings = true;
    h.settle();
    assert!(h.state.plugin_layout.1.is_empty());
    assert!(h.state.plugin_layout.0.is_none());
}

#[test]
fn hints_show_in_the_bar_and_settings_open_from_f10() {
    let mut h = Harness::new(&[]);
    h.settle();
    // Hover along both toolbar rows until the play button's hint reaches
    // the hint bar (it shows the next frame).
    let rows = [14.0, 38.0, 60.0];
    let points = rows
        .into_iter()
        .flat_map(|y| (0..120).map(move |i| pos2(8.0 + 8.0 * i as f32, y)));
    let found = points.into_iter().any(|p| {
        h.frame(vec![Event::PointerMoved(p)]);
        h.frame(vec![]);
        h.state
            .hint
            .as_deref()
            .is_some_and(|hint| hint.contains("Play"))
    });
    assert!(found, "no play hint along the toolbar");
    h.frame(vec![Event::Key {
        key: egui::Key::F10,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(h.state.settings_tab, Some(crate::settings::Tab::Audio));
}

#[test]
fn dragging_an_eq_point_is_one_undoable_change() {
    let mut h = Harness::new(&["mixer.add_effect 0 eq"]);
    h.state.effect_windows.push((0, 0));
    h.settle();
    let graph = h.state.effect_graph.expect("graph drawn");
    let depth = h.session.undo_depth();
    // The middle point starts at 1 kHz, 0 dB: near the graph's middle.
    let x = graph.left() + graph.width() * (1_000.0_f32 / 20.0).ln() / 1_000.0_f32.ln();
    let start = pos2(x, graph.center().y);
    let commands = h.drag(start, start + vec2(40.0, -30.0));
    assert!(!commands.is_empty());
    assert!(
        commands.iter().all(|c| c.starts_with("edit.batch")),
        "{commands:?}"
    );
    let eq = &h.session.project().mixer[0].effects[0];
    assert!(eq.values[2] > 3.0, "mid gain raised: {}", eq.values[2]);
    assert!(eq.values[3] > 1_200.0, "mid freq raised: {}", eq.values[3]);
    assert_eq!(h.session.undo_depth(), depth + 1);
}

#[test]
fn effect_windows_close_with_their_slot() {
    let mut h = Harness::new(&["mixer.add_effect 0 reverb"]);
    h.state.effect_windows.push((0, 0));
    h.settle();
    h.session.run_line("mixer.remove_effect 0 1").unwrap();
    h.settle();
    assert!(h.state.effect_windows.is_empty());
}

#[test]
fn piano_roll_paint_delete_and_select_tools() {
    use crate::piano_roll::Tool;
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.piano_roll = true;
    // Compact rows, so every key used below fits the small window.
    h.state.roll.key_height = 14.0;
    h.settle();
    let step = TICKS_PER_STEP;

    // Paint: one drag lays down a run of notes in one undo step.
    h.state.roll.tool = Tool::Paint;
    let depth = h.session.undo_depth();
    let painted = h.drag(h.roll_point(step / 2, 62), h.roll_point(5 * step + 5, 62));
    assert_eq!(painted.len(), 1, "{painted:?}");
    assert!(painted[0].starts_with("edit.batch"), "{painted:?}");
    let notes = &h.session.project().current_pattern().notes;
    assert!(notes.len() >= 4, "painted {} notes", notes.len());
    assert!(
        notes
            .windows(2)
            .all(|w| w[1].start >= w[0].start + w[0].length)
    );
    assert_eq!(h.session.undo_depth(), depth + 1);
    h.settle();

    // Select: a box around the first two notes selects them.
    h.state.roll.tool = Tool::Select;
    let from = h.roll_point(0, 63) + vec2(2.0, 0.0);
    let to = h.roll_point(2 * step - 2, 61);
    assert!(h.drag(from, to).is_empty());
    assert_eq!(h.state.roll.selection.len(), 2);
    h.click(h.roll_point(14 * step, 64), PointerButton::Primary);
    assert!(h.state.roll.selection.is_empty());

    // Delete: dragging across notes removes all it touched, in one step.
    h.state.roll.tool = Tool::Delete;
    let before = h.session.project().current_pattern().notes.len();
    let erased = h.drag(h.roll_point(5, 62), h.roll_point(3 * step + 5, 62));
    assert_eq!(erased.len(), 1, "{erased:?}");
    let after = h.session.project().current_pattern().notes.len();
    assert!(after + 3 <= before, "{before} -> {after}");
}

#[test]
fn playlist_paint_and_delete_tools_are_single_steps() {
    use crate::piano_roll::Tool;
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 4 C4"]);
    h.state.windows.playlist = true;
    h.settle();
    let bar = TICKS_PER_BAR;
    h.state.playlist.tool = Tool::Paint;
    let depth = h.session.undo_depth();
    let painted = h.drag(
        h.playlist_point(bar / 4, 1),
        h.playlist_point(4 * bar + bar / 4, 1),
    );
    assert_eq!(painted.len(), 1, "{painted:?}");
    let clips = &h.session.project().playlist.clips;
    assert!(clips.len() >= 4, "painted {} clips", clips.len());
    assert!(clips.iter().all(|c| c.track == 1));
    assert_eq!(h.session.undo_depth(), depth + 1);
    h.settle();
    h.state.playlist.tool = Tool::Delete;
    let erased = h.drag(
        h.playlist_point(bar / 2, 1),
        h.playlist_point(2 * bar + bar / 2, 1),
    );
    assert_eq!(erased.len(), 1, "{erased:?}");
    assert!(h.session.project().playlist.clips.len() + 3 <= clips_len(&painted));
}

fn clips_len(painted: &[String]) -> usize {
    painted[0].matches("playlist.add_clip").count()
}

#[test]
fn slide_toggle_draws_slide_notes() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.piano_roll = true;
    h.state.roll.slide = true;
    h.settle();
    let added = h.click(
        h.roll_point(4 * TICKS_PER_STEP + 5, 62),
        PointerButton::Primary,
    );
    assert_eq!(added, ["note.add 1 96t 24t 62 100 0 slide"]);
    assert!(h.session.project().current_pattern().notes[0].slide);
}

#[test]
fn right_drag_deletes_every_note_it_passes() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "note.add 1 0 1 62",
        "note.add 1 2 1 62",
        "note.add 1 4 1 62",
        "note.add 1 8 1 62",
        "note.add 1 2 1 70",
    ]);
    h.state.windows.piano_roll = true;
    h.settle();
    let step = TICKS_PER_STEP;
    let depth = h.session.undo_depth();
    let removed = h.drag_with(
        h.roll_point(5, 62),
        h.roll_point(4 * step + 5, 62),
        PointerButton::Secondary,
    );
    assert_eq!(removed.len(), 1, "{removed:?}");
    let left: Vec<(u32, u8)> = h
        .session
        .project()
        .current_pattern()
        .notes
        .iter()
        .map(|n| (n.start, n.key))
        .collect();
    assert_eq!(left, [(2 * step, 70), (8 * step, 62)]);
    assert_eq!(h.session.undo_depth(), depth + 1);
    h.settle();
    let single = h.click(h.roll_point(8 * step + 5, 62), PointerButton::Secondary);
    assert_eq!(single.len(), 1);
    assert_eq!(h.session.project().current_pattern().notes.len(), 1);
}

#[test]
fn note_kinds_from_the_piano_roll() {
    use crate::piano_roll::{LaneMode, Tool};
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.piano_roll = true;
    h.state.roll.porta = true;
    h.state.roll.color = 3;
    h.settle();
    let step = TICKS_PER_STEP;
    let added = h.click(h.roll_point(2 * step + 5, 62), PointerButton::Primary);
    assert_eq!(added.len(), 1);
    assert!(added[0].starts_with("note.restore 1 "), "{added:?}");
    let n = h.session.project().current_pattern().notes[0];
    assert!(n.props.porta && n.props.color == 3);
    h.state.roll.porta = false;
    h.state.roll.color = 0;
    h.settle();
    h.click(h.roll_point(6 * step + 5, 62), PointerButton::Primary);
    h.settle();

    // The mute tool toggles every note it sweeps, in one step.
    h.state.roll.tool = Tool::Mute;
    let depth = h.session.undo_depth();
    h.drag(
        h.roll_point(2 * step + 5, 62),
        h.roll_point(6 * step + 5, 62),
    );
    assert!(
        h.session
            .project()
            .current_pattern()
            .notes
            .iter()
            .all(|n| n.props.muted)
    );
    assert_eq!(h.session.undo_depth(), depth + 1);
    h.state.roll.tool = Tool::Draw;

    // The lane edits fine pitch.
    h.state.roll.lane_mode = LaneMode::Fine;
    h.settle();
    let (grid, clip) = h.state.roll.last_view.unwrap();
    let x = grid.x((2 * step) as f32) + 2.0;
    h.drag(pos2(x, clip.bottom() - 40.0), pos2(x, clip.bottom() - 70.0));
    let fine = h.session.project().current_pattern().notes[0].props.fine;
    assert!(fine > 20, "fine raised: {fine}");
}

/// Steps lit on channel `n` (1-based) of the current pattern.
fn lit_steps(h: &Harness, n: usize) -> Vec<u32> {
    let project = h.session.project();
    let id = project.channels[n - 1].id;
    let mut steps: Vec<u32> = project
        .current_pattern()
        .notes_of(id)
        .map(|note| note.start / TICKS_PER_STEP)
        .collect();
    steps.sort_unstable();
    steps
}

#[test]
fn step_buttons_paint_with_the_left_button_and_erase_with_the_right() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick", "step.toggle 1 2"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rects: Vec<Rect> = h
        .state
        .step_layout
        .iter()
        .filter(|(channel, _)| *channel == 0)
        .map(|(_, rect)| *rect)
        .collect();
    let [first, second] = rects[..] else {
        panic!("steps drawn: {rects:?}");
    };
    let pitch = second.center() - first.center();
    let step = |n: usize| first.center() + pitch * n as f32;

    // A click toggles one step.
    h.click(step(0), PointerButton::Primary);
    assert_eq!(lit_steps(&h, 1), [0, 2]);
    // Right-click turns a lit step off and leaves an unlit one alone.
    h.click(step(0), PointerButton::Secondary);
    h.click(step(1), PointerButton::Secondary);
    assert_eq!(lit_steps(&h, 1), [2]);

    // A left drag from an unlit step paints every step it passes, as one
    // undo step.
    let depth = h.session.undo_depth();
    h.drag(step(4), step(7));
    assert_eq!(lit_steps(&h, 1), [2, 4, 5, 6, 7]);
    assert_eq!(h.session.undo_depth(), depth + 1);

    // A right drag erases.
    h.drag_with(step(3), step(5), PointerButton::Secondary);
    assert_eq!(lit_steps(&h, 1), [2, 6, 7]);
    h.session.run_line("edit.undo").unwrap();
    assert_eq!(lit_steps(&h, 1), [2, 4, 5, 6, 7]);

    // A left drag from a lit step clears the steps it passes.
    h.drag(step(6), step(7));
    assert_eq!(lit_steps(&h, 1), [2, 4, 5]);
}

#[test]
fn the_playlist_snaps_to_bars_of_the_song_meter() {
    // 3/4 bars of 288 ticks, then 7/8 bars of 336 from the third bar.
    let mut h = Harness::new(&[
        "pattern.add Verse",
        "project.set_time_signature 3/4",
        "playlist.set_time_signature 2 7/8",
    ]);
    h.state.windows.playlist = true;
    h.settle();
    let placed = h.click(h.playlist_point(288 + 10, 1), PointerButton::Primary);
    assert_eq!(placed, ["playlist.add_clip 2 2 288t"]);
    h.settle();
    let placed = h.click(h.playlist_point(576 + 300, 2), PointerButton::Primary);
    assert_eq!(placed, ["playlist.add_clip 2 3 576t"]);
    h.settle();
    // Shift+drag on the ruler sets a loop on bar lines of the meter.
    let (grid, clip) = h.state.playlist.last_view.unwrap();
    let on_ruler = |tick: u32| pos2(grid.x(tick as f32), clip.top() + 10.0);
    h.modifiers = Modifiers::SHIFT;
    let set = h.drag(on_ruler(288 + 10), on_ruler(576 + 336 - 10));
    h.modifiers = Modifiers::NONE;
    assert_eq!(set, ["transport.set_loop 288t 912t"]);
}

#[test]
fn the_rack_and_roll_follow_the_pattern_signature() {
    let mut h = Harness::new(&[
        "channel.add_sampler builtin:kick",
        "pattern.set_time_signature 1 7/8",
    ]);
    h.state.windows.piano_roll = true;
    h.settle();
    // One bar of 7/8 is 14 steps; the roll's bar snap is a 7/8 bar.
    assert_eq!(h.session.project().current_pattern().length_steps, 14);
    assert_eq!(h.state.roll.meter.first().to_string(), "7/8");
    assert_eq!(
        h.state.step_edit_ticks(0, &h.state.roll.meter),
        TICKS_PER_STEP
    );
}

#[test]
fn channel_settings_pages_switch_and_keep_the_header() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.windows.channel_settings = true;
    h.settle();
    for tab in [
        crate::channel_settings::ChannelTab::Functions,
        crate::channel_settings::ChannelTab::Envelope,
        crate::channel_settings::ChannelTab::Sound,
    ] {
        h.state.channel_tab = tab;
        h.settle();
        assert_eq!(h.state.channel_tab, tab);
    }
    // A drum synth has no shared envelope: its page falls back to Sound.
    h.session.run_line("channel.add_drum kick").unwrap();
    h.state.selected_channel = 1;
    h.state.channel_tab = crate::channel_settings::ChannelTab::Envelope;
    h.settle();
    assert_eq!(
        h.state.channel_tab,
        crate::channel_settings::ChannelTab::Sound
    );
}

#[test]
fn alt_dragging_a_segment_bends_it_in_one_step() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "automation.create channel:1:cutoff",
        "automation.set_points 1 [[0,0.0],[1536,1.0]]",
    ]);
    let at = h.open_automation();
    let depth = h.session.undo_depth();
    h.modifiers = Modifiers::ALT;
    // Away from the handle, on the segment.
    let bent = h.drag(at(400, 0.26), at(400, 0.26) + vec2(0.0, 60.0));
    h.modifiers = Modifiers::NONE;
    assert_eq!(bent.len(), 1, "{bent:?}");
    assert!(
        bent[0].starts_with("automation.set_curve 1 0t single 0."),
        "{bent:?}"
    );
    let point = h.session.project().automations[0].points[0];
    assert!(point.tension > 0.3, "bent down: {}", point.tension);
    assert_eq!(h.session.undo_depth(), depth + 1);
}

#[test]
fn points_are_edited_inside_automation_clips_in_the_playlist() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "automation.create channel:1:cutoff",
        "automation.set_points 1 [[0,0.0],[1536,1.0]]",
        "playlist.add_automation_clip 1 2 0 4",
    ]);
    h.state.windows.playlist = true;
    h.settle();
    let (grid, _) = h.state.playlist.last_view.expect("playlist drawn");
    let clip = h.session.project().playlist.clips[0];
    let body = crate::playlist::clip_body_rect(&grid, &clip).expect("tall enough");
    // A point in the body: right-click adds one there.
    let body_y = |value: f32| body.bottom() - value * body.height();
    let x = grid.x(768.0);
    let added = h.click(pos2(x, body_y(0.2)), PointerButton::Secondary);
    assert_eq!(added.len(), 1, "{added:?}");
    assert!(
        added[0].starts_with("automation.add_point 1 768t"),
        "{added:?}"
    );
    assert_eq!(h.session.project().playlist.clips.len(), 1, "clip kept");
    h.settle();
    // Dragging that point moves it, not the clip.
    let p = h.session.project().automations[0].points[1];
    let from = pos2(x, body_y(p.value));
    let moved = h.drag(from, from + vec2(grid.px_per_tick * 96.0, 0.0));
    assert_eq!(moved.len(), 1, "{moved:?}");
    assert!(
        moved[0].starts_with("automation.move_point 1 768t 864t"),
        "{moved:?}"
    );
    assert_eq!(h.session.project().playlist.clips[0].start, 0);
}

#[test]
fn create_automation_clip_opens_the_editor_on_it() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.settle();
    let engine = EngineStatus::default();
    let history = History::default();
    let frame = Frame {
        project: h.session.project(),
        engine: &engine,
        history: &history,
        waveform: None,
        waveform_seconds: None,
        clip_waveforms: &[],
        clip_durations: &[],
        plugins: &Plugins::default(),
        settings: &Settings::default(),
        settings_info: &SettingsInfo::default(),
    };
    let out = crate::resolve_ui_actions(
        &mut h.state,
        &frame,
        vec![Action::Run(
            musiccraft_commands::Invocation::parse("ui.automate mixer:0:volume").unwrap(),
        )],
    );
    let Action::Run(batch) = &out[0] else {
        panic!("{out:?}")
    };
    h.session.run(batch).unwrap();
    h.settle();
    let id = h.session.project().automations[0].id;
    assert_eq!(h.state.automation_editor, Some(id));
}

#[test]
fn pattern_miniatures_in_the_picker_select_the_pattern_to_paint() {
    let mut h = Harness::new(&[
        "channel.add_sampler builtin:kick",
        "step.toggle 1 1",
        "pattern.add",
        "pattern.select 1",
    ]);
    h.state.windows.playlist = true;
    h.state.playlist.paint_audio = Some(h.session.project().channels[0].id);
    h.settle();
    let items = h.state.playlist.picker_items.clone();
    assert_eq!(items.len(), 2, "one miniature per pattern: {items:?}");
    assert!(items[0].height() >= 30.0 && items[1].top() > items[0].bottom());
    // Clicking the second pattern's miniature selects it and paints patterns
    // again instead of audio.
    let commands = h.click(items[1].center(), PointerButton::Primary);
    assert_eq!(commands, ["pattern.select 2"]);
    assert_eq!(h.state.playlist.paint_audio, None);
    // Clicking the selected one again (not soon enough to be a
    // double-click) changes nothing in the project.
    h.time += 1.0;
    let items = h.state.playlist.picker_items.clone();
    assert!(
        h.click(items[1].center(), PointerButton::Primary)
            .is_empty()
    );
}

#[test]
fn a_long_pattern_list_scrolls_without_covering_the_miniatures() {
    // Nine patterns are taller than the list, so it shows a scroll bar.
    let mut setup = vec!["channel.add_sampler builtin:kick"];
    setup.extend(["pattern.add"; 8]);
    setup.push("pattern.select 1");
    let mut h = Harness::new(&setup);
    h.state.windows.playlist = true;
    h.settle();
    let items = h.state.playlist.picker_items.clone();
    assert_eq!(items.len(), 9);
    // The right edge of a miniature still selects its pattern.
    let edge = pos2(items[2].right() - 2.0, items[2].center().y);
    assert_eq!(h.click(edge, PointerButton::Primary), ["pattern.select 3"]);
}

#[test]
fn the_picker_paints_audio_clips_at_their_length() {
    let mut h = Harness::new(&[
        "channel.add_sampler builtin:kick",
        "playlist.add_audio_clip 1 1 0 1",
    ]);
    h.state.windows.playlist = true;
    h.state.playlist.picker = crate::playlist::Picker::Audio;
    h.state.playlist.paint_audio = Some(h.session.project().channels[0].id);
    h.settle();
    let placed = h.click(
        h.playlist_point(TICKS_PER_BAR + 10, 1),
        PointerButton::Primary,
    );
    assert_eq!(placed, ["playlist.add_audio_clip 1 2 384t 384t"]);
}

/// The outer rectangle of window `id`.
fn window_rect(h: &Harness, id: &str) -> Rect {
    h.ctx
        .memory(|m| m.area_rect(egui::Id::new(id)))
        .expect("window drawn")
}

/// The centre of header button `n`, counted from the right end (0 = close).
fn header_button(h: &Harness, id: &str, n: f32) -> Pos2 {
    let rect = window_rect(h, id);
    pos2(rect.right() - 7.0 - 9.0 - 20.0 * n, rect.top() + 6.0 + 10.0)
}

#[test]
fn window_buttons_collapse_maximize_restore_and_close() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let before = window_rect(&h, "channel-rack");
    // Maximize fills the workspace; restore returns to the old rectangle.
    h.click(
        header_button(&h, "channel-rack", 1.0),
        PointerButton::Primary,
    );
    h.settle();
    assert!(crate::chrome::maximized(
        &h.ctx,
        egui::Id::new("channel-rack")
    ));
    let full = window_rect(&h, "channel-rack");
    assert!(
        (full.width() - h.state.workspace.width()).abs() < 2.0,
        "{full:?}"
    );
    assert!(
        (full.height() - h.state.workspace.height()).abs() < 2.0,
        "{full:?}"
    );
    h.click(
        header_button(&h, "channel-rack", 1.0),
        PointerButton::Primary,
    );
    h.settle();
    assert!(!crate::chrome::maximized(
        &h.ctx,
        egui::Id::new("channel-rack")
    ));
    let back = window_rect(&h, "channel-rack");
    assert!(
        (back.width() - before.width()).abs() < 2.0,
        "{back:?} vs {before:?}"
    );
    assert!(
        (back.left() - before.left()).abs() < 2.0,
        "{back:?} vs {before:?}"
    );
    // Collapse rolls the window up to its header.
    h.click(
        header_button(&h, "channel-rack", 2.0),
        PointerButton::Primary,
    );
    h.settle();
    assert!(crate::chrome::collapsed(
        &h.ctx,
        egui::Id::new("channel-rack")
    ));
    assert!(window_rect(&h, "channel-rack").height() < 40.0);
    // Close hides it.
    h.click(
        header_button(&h, "channel-rack", 0.0),
        PointerButton::Primary,
    );
    h.settle();
    assert!(!h.state.windows.channel_rack);
}

#[test]
fn a_narrow_header_folds_its_tools_instead_of_wrapping() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.windows.piano_roll = true;
    h.settle();
    // Rolled up, the header is a single row however many tools it has.
    h.click(header_button(&h, "piano-roll", 2.0), PointerButton::Primary);
    h.settle();
    assert!(crate::chrome::collapsed(
        &h.ctx,
        egui::Id::new("piano-roll")
    ));
    let rect = window_rect(&h, "piano-roll");
    assert!(rect.height() < 40.0, "{rect:?}");
}

#[test]
fn the_playlist_is_a_window_in_the_lower_part_of_the_workspace() {
    let mut h = Harness::new(&["pattern.add"]);
    h.state.windows.playlist = true;
    h.settle();
    let rect = window_rect(&h, "playlist-window");
    let area = h.state.workspace;
    assert!(
        rect.top() > area.center().y - area.height() * 0.2,
        "{rect:?} in {area:?}"
    );
    assert!(
        (rect.bottom() - area.bottom()).abs() < 8.0,
        "{rect:?} in {area:?}"
    );
    assert!(rect.width() > area.width() - 16.0, "{rect:?} in {area:?}");
    // Its header closes it like any other window.
    h.click(
        header_button(&h, "playlist-window", 0.0),
        PointerButton::Primary,
    );
    h.settle();
    assert!(!h.state.windows.playlist);
}

#[test]
fn the_mixer_opens_at_the_top_beside_the_channel_rack() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.windows.channel_rack = true;
    h.state.windows.mixer = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let mixer = window_rect(&h, "mixer-window");
    // Along the top, under the backdrop playlist's header and ruler.
    let area = h.state.floating_area();
    assert!(mixer.left() >= rack.right(), "{mixer:?} beside {rack:?}");
    assert!(
        (mixer.right() - area.right()).abs() < 8.0,
        "{mixer:?} in {area:?}"
    );
    assert!(
        (mixer.top() - rack.top()).abs() < 1.0 && mixer.top() - area.top() < 8.0,
        "{mixer:?} in {area:?}"
    );
    assert!(mixer.bottom() < area.center().y, "{mixer:?} in {area:?}");
}

#[test]
fn the_count_in_toggle_sits_in_the_toolbar() {
    let mut h = Harness::new(&[]);
    h.settle();
    let before = h.session.project().record.count_in;
    let spot = [14.0, 38.0]
        .into_iter()
        .flat_map(|y| (0..120).map(move |i| pos2(8.0 + 8.0 * i as f32, y)))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|hint| hint.starts_with("Count-in"))
        })
        .expect("a count-in toggle in the toolbar");
    h.click(spot, PointerButton::Primary);
    h.settle();
    assert_ne!(h.session.project().record.count_in, before);
}

#[test]
fn the_piano_roll_opens_over_the_right_half_with_its_own_bars() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let roll = window_rect(&h, "piano-roll");
    let area = h.state.workspace;
    assert!(roll.left() >= area.center().x - 8.0, "{roll:?} in {area:?}");
    assert!(roll.width() < area.width() * 0.6, "{roll:?} in {area:?}");

    // Dragging the zoom box up makes key rows taller; a double-click resets.
    let (grid, clip) = h.state.roll.last_view.expect("piano roll drawn");
    let bar = crate::scrollbars::BAR;
    let zoom = pos2(clip.right() + bar / 2.0, clip.top() - bar / 2.0);
    h.drag(zoom, zoom - vec2(0.0, 40.0));
    h.settle();
    let taller = h.state.roll.last_view.unwrap().0.row_height;
    assert!(taller > grid.row_height, "{taller} > {}", grid.row_height);
    h.click(zoom, PointerButton::Primary);
    h.click(zoom, PointerButton::Primary);
    h.settle();
    let reset = h.state.roll.last_view.unwrap().0.row_height;
    assert!((reset - grid.row_height).abs() < 0.01, "{reset}");

    // The corner block carries the Porta toggle at its top right.
    assert!(!h.state.roll.porta);
    let porta = pos2(clip.left() + 52.0 - 6.0, clip.top() - bar + 6.0);
    h.click(porta, PointerButton::Primary);
    h.settle();
    assert!(h.state.roll.porta);
}

#[test]
fn the_playlist_zoom_box_sets_the_track_height() {
    let mut h = Harness::new(&["pattern.add"]);
    h.state.windows.playlist = true;
    h.settle();
    let (grid, clip) = h.state.playlist.last_view.expect("playlist drawn");
    let bar = crate::scrollbars::BAR;
    let zoom = pos2(clip.right() + bar / 2.0, clip.top() - bar / 2.0);
    h.drag(zoom, zoom - vec2(0.0, 60.0));
    h.settle();
    let taller = h.state.playlist.last_view.unwrap().0.row_height;
    assert!(taller > grid.row_height, "{taller} > {}", grid.row_height);

    // The right chevron scrolls the lanes one bar to the left.
    let right = pos2(clip.right() - bar / 2.0, clip.top() - bar / 2.0);
    let before = h.state.playlist.last_view.unwrap().0.origin.x;
    h.click(right, PointerButton::Primary);
    h.settle();
    let after = h.state.playlist.last_view.unwrap().0.origin.x;
    assert!(after < before, "{after} < {before}");
}

#[test]
fn a_long_channel_rack_gets_the_plain_scroll_bar() {
    let setup: Vec<&str> = std::iter::repeat_n("channel.add_sampler builtin:kick", 24).collect();
    let mut h = Harness::new(&setup);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let bar = crate::scrollbars::BAR;
    let hint_at = |h: &mut Harness, p: Pos2| {
        h.frame(vec![Event::PointerMoved(p)]);
        h.frame(vec![]);
        h.state.hint.clone().unwrap_or_default()
    };
    let x = (0..40)
        .map(|i| rack.right() - 2.0 - i as f32)
        .find(|x| hint_at(&mut h, pos2(*x, rack.center().y)).starts_with("Click to page"))
        .expect("a vertical rail at the rack's right edge");
    let down = (0..200)
        .map(|i| pos2(x, rack.bottom() - i as f32 * 2.0))
        .find(|p| hint_at(&mut h, *p) == "Scroll down")
        .expect("a down chevron under the rail");
    assert!(rack.right() - x < 4.0 * bar, "{x} near {rack:?}");
    let offset =
        |h: &Harness| crate::scrollbars::plain_offset(&h.ctx, egui::Id::new("rack-bars")).y;
    assert_eq!(offset(&h), 0.0);
    for _ in 0..3 {
        h.click(down, PointerButton::Primary);
    }
    h.settle();
    assert!(offset(&h) > 0.0, "the rows scrolled");
}

#[test]
fn dragging_the_piano_roll_ruler_selects_a_time_range() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "note.add 1 0 1 60",
        "note.add 1 2 1 62",
        "note.add 1 8 1 64",
    ]);
    h.state.windows.piano_roll = true;
    h.settle();
    let step = TICKS_PER_STEP;
    let (grid, clip) = h.state.roll.last_view.expect("piano roll drawn");
    let y = clip.top() + 8.0;
    let at = |tick: u32| pos2(grid.x(tick as f32), y);
    let ran = h.drag(at(step / 2), at(4 * step));
    assert!(ran.is_empty(), "a time selection runs no commands: {ran:?}");
    assert_eq!(h.state.roll.time_selection, Some((0, 4 * step)));
    let selected: Vec<_> = h.state.roll.selection.iter().copied().collect();
    assert_eq!(selected, [(0, 60), (2 * step, 62)]);
    // A click on the ruler clears the range; Escape clears it too.
    h.click(at(step), PointerButton::Primary);
    h.settle();
    assert_eq!(h.state.roll.time_selection, None);
}

#[test]
fn ctrl_clicking_a_key_selects_its_notes() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "note.add 1 0 1 60",
        "note.add 1 4 1 60",
        "note.add 1 8 1 64",
    ]);
    h.state.windows.piano_roll = true;
    h.settle();
    let step = TICKS_PER_STEP;
    let (_, clip) = h.state.roll.last_view.expect("piano roll drawn");
    let key = pos2(clip.left() + 40.0, h.roll_point(0, 60).y);
    h.modifiers = Modifiers::COMMAND;
    let ran = h.click(key, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    h.settle();
    assert!(ran.is_empty(), "{ran:?}");
    let selected: Vec<_> = h.state.roll.selection.iter().copied().collect();
    assert_eq!(selected, [(0, 60), (4 * step, 60)]);
    assert!(h.state.roll.preview.is_none(), "no note was left sounding");
}

#[test]
fn double_clicking_an_audio_clip_opens_its_channel_settings() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "channel.add_sampler builtin:kick",
        "playlist.add_audio_clip 2 1 0 2",
    ]);
    h.state.windows.playlist = true;
    h.state.selected_channel = 0;
    h.settle();
    let at = h.playlist_point(musiccraft_project::params::PPQ * 2, 0);
    h.click(at, PointerButton::Primary);
    h.click(at, PointerButton::Primary);
    h.settle();
    assert!(h.state.windows.channel_settings);
    assert_eq!(h.state.selected_channel, 1);
}

#[test]
fn dragging_a_browser_item_over_a_window_button_opens_that_window() {
    let mut h = Harness::new(&[]);
    h.settle();
    assert!(!h.state.windows.mixer);
    let spot = [14.0, 38.0, 60.0]
        .into_iter()
        .flat_map(|y| (0..160).map(move |i| pos2(8.0 + 8.0 * i as f32, y)))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|hint| hint.starts_with("Mixer (F9)"))
        })
        .expect("a mixer button in the toolbar");
    assert!(!h.state.windows.mixer, "hovering alone opens nothing");
    egui::DragAndDrop::set_payload(
        &h.ctx,
        crate::browser::Dragged::Sample("kick.wav".to_owned()),
    );
    h.frame(vec![Event::PointerMoved(spot + vec2(1.0, 0.0))]);
    h.frame(vec![]);
    assert!(h.state.windows.mixer);
}

#[test]
fn enter_maximizes_the_playlist_and_ctrl_shift_h_restores_the_default_layout() {
    let mut h = Harness::new(&[]);
    h.settle();
    let key = |key, modifiers| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    let playlist = egui::Id::new("playlist-window");
    h.frame(vec![key(egui::Key::Enter, Modifiers::NONE)]);
    h.settle();
    assert!(h.state.windows.playlist, "Enter opens the playlist");
    assert!(crate::chrome::maximized(&h.ctx, playlist));
    let maximized = window_rect(&h, "playlist-window");
    assert!(maximized.width() > 1300.0, "{maximized:?}");

    let layout = Modifiers::COMMAND | Modifiers::SHIFT;
    h.modifiers = layout;
    h.frame(vec![key(egui::Key::H, layout)]);
    h.modifiers = Modifiers::NONE;
    h.settle();
    let w = &h.state.windows;
    assert!(w.playlist && w.channel_rack && w.mixer && w.piano_roll && w.browser);
    // The playlist is maximized again, behind the others.
    assert!(crate::chrome::maximized(&h.ctx, playlist));
    // Restored, it goes back to its default slot under the rack. (Enter is
    // still down from the first press.)
    h.frame(vec![Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    h.key(egui::Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!crate::chrome::maximized(&h.ctx, playlist));
    let rack = window_rect(&h, "channel-rack");
    let list = window_rect(&h, "playlist-window");
    assert!(rack.bottom() <= list.top() + 1.0, "{rack:?} over {list:?}");
}

#[test]
fn window_keys_follow_the_focused_window_not_the_pointer() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let one = Event::Key {
        key: egui::Key::Num1,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let rack = window_rect(&h, "channel-rack");
    h.click(
        rack.left_top() + vec2(rack.width() / 2.0, 8.0),
        PointerButton::Primary,
    );
    h.settle();
    // The pointer leaves the rack; its keys still go to it.
    h.frame(vec![Event::PointerMoved(pos2(1390.0, 890.0))]);
    assert_eq!(h.frame(vec![one.clone()]), ["channel.toggle_mute 1"]);

    // Another window in front takes the keys.
    h.state.windows.piano_roll = true;
    h.settle();
    let roll = window_rect(&h, "piano-roll");
    h.click(
        roll.left_top() + vec2(roll.width() / 2.0, 8.0),
        PointerButton::Primary,
    );
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    h.frame(vec![Event::PointerMoved(rack.center())]);
    let out = h.frame(vec![one]);
    assert!(
        !out.iter().any(|c| c.starts_with("channel.toggle_mute")),
        "{out:?}"
    );
}

#[test]
fn esc_closes_the_focused_window_and_f12_hides_them_all() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 4 C5"]);
    h.state.windows.channel_rack = true;
    h.state.windows.piano_roll = true;
    h.settle();
    let key = |key, modifiers| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    let roll = window_rect(&h, "piano-roll");
    h.click(
        roll.left_top() + vec2(roll.width() / 2.0, 8.0),
        PointerButton::Primary,
    );
    h.settle();
    // A selection takes Esc first.
    h.state.roll.selection.insert((0, 72));
    h.frame(vec![key(egui::Key::Escape, Modifiers::NONE)]);
    assert!(h.state.roll.selection.is_empty());
    assert!(h.state.windows.piano_roll);
    // Then Esc closes the focused window only.
    h.frame(vec![Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    h.frame(vec![key(egui::Key::Escape, Modifiers::NONE)]);
    assert!(!h.state.windows.piano_roll);
    assert!(h.state.windows.channel_rack);

    h.state.windows.mixer = true;
    h.settle();
    h.frame(vec![key(egui::Key::F12, Modifiers::NONE)]);
    let w = &h.state.windows;
    assert!(!w.channel_rack && !w.mixer && !w.piano_roll && !w.playlist);
}

#[test]
fn right_clicking_the_empty_workspace_opens_the_window_menu() {
    let mut h = Harness::new(&[]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    h.click(rack.center(), PointerButton::Secondary);
    h.settle();
    assert!(
        !egui::Popup::is_any_open(&h.ctx),
        "a window takes its own clicks"
    );
    h.click(pos2(1200.0, 800.0), PointerButton::Secondary);
    h.settle();
    assert!(egui::Popup::is_any_open(&h.ctx));
}

#[test]
fn pressing_a_note_makes_its_length_and_velocity_the_defaults() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 8 C5 64"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let note = *h.session.project().current_pattern().notes.first().unwrap();
    assert_ne!(h.state.roll.last_length, note.length);
    let p = h.roll_point(note.start + 10, note.key);
    h.click(p, PointerButton::Primary);
    h.settle();
    assert_eq!(h.state.roll.last_length, note.length);
    assert_eq!(h.state.roll.velocity, 64);
}

#[test]
fn piano_roll_cursors_follow_the_tool() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 8 C5"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let note = *h.session.project().current_pattern().notes.first().unwrap();
    let body = h.roll_point(note.start + 10, note.key);
    let empty = h.roll_point(note.start + 10, note.key + 2);
    h.frame(vec![Event::PointerMoved(body)]);
    assert_eq!(h.cursor, egui::CursorIcon::Move);
    h.frame(vec![Event::PointerMoved(empty)]);
    assert_eq!(h.cursor, egui::CursorIcon::Crosshair);
    h.state.roll.tool = crate::piano_roll::Tool::Delete;
    h.frame(vec![Event::PointerMoved(body)]);
    assert_eq!(h.cursor, egui::CursorIcon::Default);
}

#[test]
fn double_clicking_the_tempo_display_types_a_tempo() {
    let mut h = Harness::new(&[]);
    h.settle();
    let spot = [14.0, 38.0, 60.0]
        .into_iter()
        .flat_map(|y| (0..160).map(move |i| pos2(8.0 + 8.0 * i as f32, y)))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|hint| hint.starts_with("Tempo "))
        })
        .expect("a tempo display in the toolbar");
    h.click(spot, PointerButton::Primary);
    h.click(spot, PointerButton::Primary);
    h.settle();
    h.frame(vec![Event::Key {
        key: egui::Key::A,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::COMMAND,
    }]);
    h.frame(vec![Event::Text("98.5".to_owned())]);
    let out = h.frame(vec![Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(out, ["project.set_tempo 98.500"]);
    assert!(!h.state.windows.playlist, "Enter stays in the field");
}

#[test]
fn mixer_strips_step_by_whole_strips() {
    let mut h = Harness::new(&[]);
    h.state.windows.mixer = true;
    h.settle();
    let bars = egui::Id::new("mixer-strip-bars");
    let offset = |h: &Harness| crate::scrollbars::plain_offset(&h.ctx, bars).x;
    let before = offset(&h);
    // What one wheel notch over a strip's name asks for.
    crate::scrollbars::plain_step(&h.ctx, bars, vec2(42.0, 0.0));
    h.settle();
    assert!(
        (offset(&h) - before - 42.0).abs() < 0.5,
        "{before} → {}",
        offset(&h)
    );
}

#[test]
fn a_strip_lifts_while_its_track_menu_is_open() {
    let mut h = Harness::new(&["mixer.rename 2 Bass"]);
    h.state.windows.mixer = true;
    h.settle();
    let mixer = window_rect(&h, "mixer-window");
    let name = (0..100)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| mixer.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 4.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Bass — click"))
        })
        .expect("insert 2's name");
    assert_eq!(crate::mixer::menu_target(&h.ctx), None);
    h.click(name, PointerButton::Secondary);
    h.settle();
    assert_eq!(crate::mixer::menu_target(&h.ctx), Some(2));
    h.key(egui::Key::Escape, Modifiers::NONE);
    h.settle();
    assert_eq!(crate::mixer::menu_target(&h.ctx), None);
}

#[test]
fn the_mixer_plus_adds_an_insert_and_selects_it() {
    let mut h = Harness::new(&[]);
    h.state.windows.mixer = true;
    h.settle();
    let mixer = window_rect(&h, "mixer-window");
    let plus = (0..200)
        .map(|i| mixer.left_top() + vec2(4.0 + 3.0 * i as f32, 11.0))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Add an insert track"))
        })
        .expect("the add-insert button");
    let before = h.session.project().mixer.len();
    assert_eq!(h.click(plus, PointerButton::Primary), ["mixer.add_track"]);
    assert_eq!(h.session.project().mixer.len(), before + 1);
    assert_eq!(h.state.selected_track, before);
    // Right-click opens the menu instead of adding.
    let out = h.click(plus, PointerButton::Secondary);
    assert!(out.is_empty(), "{out:?}");
}

#[test]
fn the_wheel_over_empty_strip_space_steps_the_strips() {
    let mut h = Harness::new(&["mixer.rename 2 Bass"]);
    h.state.windows.mixer = true;
    h.settle();
    let mixer = window_rect(&h, "mixer-window");
    let hint_at = |h: &mut Harness, p: Pos2| {
        h.frame(vec![Event::PointerMoved(p)]);
        h.frame(vec![]);
        h.state.hint.clone()
    };
    let name = (0..100)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| mixer.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 4.0 * j as f32))
        .find(|p| hint_at(&mut h, *p).is_some_and(|t| t.starts_with("Bass — click")))
        .expect("insert 2's name");
    // Somewhere in insert 2's strip with no control under the pointer.
    let space = (0..120)
        .flat_map(|j| (-4..8).map(move |i| (i, j)))
        .map(|(i, j)| name + vec2(3.0 * i as f32, 3.0 * j as f32))
        .find(|p| hint_at(&mut h, *p).is_none())
        .expect("empty strip space");
    let bars = egui::Id::new("mixer-strip-bars");
    let before = crate::scrollbars::plain_offset(&h.ctx, bars).x;
    let out = h.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, -1.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    }]);
    h.settle();
    assert!(out.is_empty(), "{out:?}");
    let after = crate::scrollbars::plain_offset(&h.ctx, bars).x;
    assert!(after > before + 20.0, "{before} → {after} at {space:?}");
    // The wheel over a name steps too (a step asked for from inside the
    // strips is not lost).
    h.frame(vec![Event::PointerMoved(name)]);
    h.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, 1.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    }]);
    h.settle();
    let back = crate::scrollbars::plain_offset(&h.ctx, bars).x;
    assert!(back < after - 20.0, "{after} → {back}");
    // Over a knob the wheel turns the knob and the strips stay put.
    let knob = (0..200)
        .flat_map(|j| (-4..8).map(move |i| (i, j)))
        .map(|(i, j)| name + vec2(3.0 * i as f32, 3.0 * j as f32))
        .find(|p| hint_at(&mut h, *p).is_some_and(|t| t.starts_with("Pan ")))
        .expect("a pan knob");
    h.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, -1.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    }]);
    h.settle();
    let still = crate::scrollbars::plain_offset(&h.ctx, bars).x;
    assert!((still - back).abs() < 0.5, "{back} → {still} at {knob:?}");
}

#[test]
fn ctrl_clicking_a_mute_light_solos_the_channel() {
    let mut h = Harness::new(&["channel.add_synth", "channel.add_synth"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let light = (0..40)
        .flat_map(|j| (0..40).map(move |i| (i, j)))
        .map(|(i, j)| rack.left_top() + vec2(4.0 + 3.0 * i as f32, 20.0 + 3.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Mute · Ctrl+click"))
        })
        .expect("a mute light");
    h.modifiers = Modifiers::COMMAND;
    let out = h.click(light, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(out, ["channel.solo 1"]);
}

#[test]
fn shift_clicking_a_mute_light_keeps_the_channel_out_of_solo() {
    let mut h = Harness::new(&["channel.add_synth", "channel.add_synth"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let light = (0..40)
        .flat_map(|j| (0..40).map(move |i| (i, j)))
        .map(|(i, j)| rack.left_top() + vec2(4.0 + 3.0 * i as f32, 20.0 + 3.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Mute · Ctrl+click"))
        })
        .expect("a mute light");
    h.modifiers = Modifiers::SHIFT;
    let out = h.click(light, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(out, ["channel.set_solo_lock 1 on"]);
    assert!(h.session.project().channels[0].solo_lock);
    h.frame(vec![Event::PointerMoved(light)]);
    h.frame(vec![]);
    let hint = h.state.hint.clone().unwrap_or_default();
    assert!(hint.starts_with("Mute (kept out of solo)"), "{hint}");
}

#[test]
fn alt_clicking_a_mute_light_mutes_or_solos_the_whole_group() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "channel.add_synth",
        "channel.add_synth",
        "channel.set_group 1 Drums",
        "channel.set_group 2 Drums",
    ]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let light = (0..40)
        .flat_map(|j| (0..40).map(move |i| (i, j)))
        .map(|(i, j)| rack.left_top() + vec2(4.0 + 3.0 * i as f32, 20.0 + 3.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Mute · Ctrl+click"))
        })
        .expect("a mute light");
    let states = |h: &Harness| {
        h.session
            .project()
            .channels
            .iter()
            .map(|c| (c.mute, c.solo))
            .collect::<Vec<_>>()
    };
    h.modifiers = Modifiers::ALT;
    h.click(light, PointerButton::Primary);
    assert_eq!(
        states(&h)[..3],
        [(true, false), (true, false), (false, false)]
    );
    h.click(light, PointerButton::Primary);
    h.modifiers = Modifiers::ALT | Modifiers::COMMAND;
    h.click(light, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(
        states(&h)[..3],
        [(false, true), (false, true), (false, false)]
    );
}

#[test]
fn shift_wheel_over_the_rack_header_steps_patterns() {
    let mut h = Harness::new(&["pattern.add", "pattern.add"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, -1.0),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::SHIFT,
    };
    h.modifiers = Modifiers::SHIFT;
    let stepped = (0..60)
        .map(|i| rack.left_top() + vec2(30.0 + 6.0 * i as f32, 11.0))
        .any(|p| {
            h.frame(vec![Event::PointerMoved(p)]);
            h.frame(vec![wheel.clone()])
                .iter()
                .any(|c| c == "pattern.next")
        });
    h.modifiers = Modifiers::NONE;
    assert!(stepped, "no header spot stepped the pattern");
}

#[test]
fn automated_controls_name_their_source_in_the_hint() {
    let mut h = Harness::new(&["channel.add_synth", "automation.create channel:1:pan"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let hints: Vec<String> = (0..40)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| rack.left_top() + vec2(4.0 + 3.0 * i as f32, 20.0 + 3.0 * j as f32))
        .filter_map(|p| {
            h.frame(vec![Event::PointerMoved(p)]);
            h.frame(vec![]);
            h.state.hint.clone()
        })
        .collect();
    let pan = hints
        .iter()
        .find(|t| t.starts_with("Pan "))
        .expect("the pan knob's hint");
    assert!(pan.contains(" · automated (clip '"), "{pan}");
    let volume = hints
        .iter()
        .find(|t| t.starts_with("Volume") || t.starts_with("Level"))
        .expect("the volume knob's hint");
    assert!(!volume.contains("automated"), "{volume}");
}

#[test]
fn the_type_in_box_sets_a_control_and_esc_cancels() {
    let key = |key: egui::Key, modifiers: Modifiers| Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    };
    let release = |key: egui::Key| Event::Key {
        key,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    let mut h = Harness::new(&[]);
    let open = |h: &mut Harness| {
        crate::type_in::open(
            &mut h.state,
            h.session.project(),
            "mixer:0:pan",
            egui::pos2(200.0, 200.0),
        );
        h.settle();
    };
    open(&mut h);
    assert!(h.state.type_in.is_some());
    h.frame(vec![key(egui::Key::A, Modifiers::COMMAND)]);
    h.frame(vec![Event::Text("25".to_owned())]);
    let out = h.frame(vec![key(egui::Key::Enter, Modifiers::NONE)]);
    assert_eq!(out, ["control.set mixer:0:pan 0.25"]);
    assert!(h.state.type_in.is_none());
    h.frame(vec![release(egui::Key::Enter)]);
    open(&mut h);
    let out = h.frame(vec![key(egui::Key::Escape, Modifiers::NONE)]);
    assert!(out.is_empty(), "{out:?}");
    assert!(h.state.type_in.is_none());
}

#[test]
fn double_clicking_a_channel_name_renames_it_in_place() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let name = (0..40)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| rack.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 3.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state.hint.as_deref() == Some("Synth")
        })
        .expect("the channel's name");
    h.click(name, PointerButton::Primary);
    h.click(name, PointerButton::Primary);
    h.settle();
    assert!(h.state.renaming.is_some(), "a double-click opens the field");
    assert!(h.state.windows.channel_settings, "the settings stay open");
    h.frame(vec![Event::Text("Lead".to_owned())]);
    let out = h.frame(vec![Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(out, ["channel.rename 1 Lead"]);
    assert!(h.state.renaming.is_none());
}

#[test]
fn dropping_on_a_channel_row_replaces_its_sound() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick", "channel.add_synth"]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let span = |h: &Harness, index: usize| {
        h.state
            .rack_rows
            .iter()
            .find(|(i, _)| *i == index)
            .map(|(_, span)| span.center())
            .expect("the row is drawn")
    };
    let drop_with = |h: &mut Harness, dragged: crate::browser::Dragged, y: f32| {
        egui::DragAndDrop::set_payload(&h.ctx, dragged);
        let at = pos2(rack.center().x, y);
        h.frame(vec![Event::PointerMoved(at)]);
        h.frame(vec![Event::PointerButton {
            pos: at,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }])
    };
    let clap = || crate::browser::Dragged::Sample("builtin:clap".to_owned());
    // A sampler only changes its sample.
    let sampler = span(&h, 0);
    assert_eq!(
        drop_with(&mut h, clap(), sampler),
        ["channel.set_sample 1 builtin:clap"]
    );
    // Any other channel takes the sound of a channel added for the drop.
    let synth = span(&h, 1);
    let out = drop_with(&mut h, clap(), synth);
    assert_eq!(
        out,
        [
            r#"edit.batch "channel.add_sampler builtin:clap" "channel.swap_sound 2 3" "channel.remove 3""#
        ]
    );
    let generator = crate::browser::Dragged::Generator {
        label: "FM".to_owned(),
        add: musiccraft_commands::Invocation::bare("channel.add_fm"),
    };
    assert_eq!(
        drop_with(&mut h, generator.clone(), sampler),
        [r#"edit.batch channel.add_fm "channel.swap_sound 1 3" "channel.remove 3""#]
    );
    // Below the rows it adds a channel.
    let below = span(&h, 1) + 40.0;
    assert_eq!(drop_with(&mut h, generator, below), ["channel.add_fm"]);
    // The rows kept their places and took the new sounds.
    let channels = &h.session.project().channels;
    assert_eq!(channels.len(), 3);
    assert!(matches!(
        channels[0].instrument,
        musiccraft_project::Instrument::Fm
    ));
    assert_eq!(channels[1].name, "Clap");
    assert!(matches!(
        channels[1].instrument,
        musiccraft_project::Instrument::Sampler { .. }
    ));
}

#[test]
fn mixer_strip_lights_solo_on_ctrl_click() {
    let mut h = Harness::new(&["mixer.add_effect 1 delay"]);
    h.state.windows.mixer = true;
    h.settle();
    let mixer = window_rect(&h, "mixer-window");
    let find = |h: &mut Harness, prefix: &str| {
        (0..100)
            .flat_map(|j| (0..60).map(move |i| (i, j)))
            .map(|(i, j)| mixer.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 4.0 * j as f32))
            .find(|p| {
                h.frame(vec![Event::PointerMoved(*p)]);
                h.frame(vec![]);
                h.state
                    .hint
                    .as_deref()
                    .is_some_and(|t| t.starts_with(prefix))
            })
    };
    let light =
        find(&mut h, "Enable (on) · click to mute or unmute · Ctrl").expect("insert 1's light");
    h.modifiers = Modifiers::COMMAND;
    let out = h.click(light, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(out, ["mixer.toggle_solo 1"]);
}

#[test]
fn shift_wheel_over_a_mixer_strip_name_moves_the_track() {
    let mut h = Harness::new(&["mixer.rename 2 Bass"]);
    h.state.windows.mixer = true;
    h.settle();
    let mixer = window_rect(&h, "mixer-window");
    let name = (0..100)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| mixer.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 4.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Bass — click"))
        })
        .expect("insert 2's name");
    h.click(name, PointerButton::Primary);
    assert_eq!(h.state.selected_track, 2);
    h.modifiers = Modifiers::SHIFT;
    let out = h.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, -1.0),
        modifiers: Modifiers::SHIFT,
        phase: egui::TouchPhase::Move,
    }]);
    h.modifiers = Modifiers::NONE;
    assert_eq!(out, ["mixer.move_track 2 right"]);
    assert_eq!(h.session.project().mixer[3].name, "Bass");
    assert_eq!(h.state.selected_track, 3, "the selection follows");
}

#[test]
fn the_piano_roll_play_button_plays_the_pattern() {
    let mut h = Harness::new(&["channel.add_synth", "transport.set_mode song"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let roll = window_rect(&h, "piano-roll");
    let button = (0..120)
        .map(|i| roll.left_top() + vec2(4.0 + 4.0 * i as f32, 11.0))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Play the pattern"))
        })
        .expect("a play button in the piano roll header");
    let out = h.click(button, PointerButton::Primary);
    assert_eq!(out, ["transport.set_mode pattern", "transport.play"]);
}

#[test]
fn the_playlist_play_button_plays_the_song() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.playlist = true;
    h.settle();
    let window = window_rect(&h, "playlist-window");
    let button = (0..200)
        .map(|i| window.left_top() + vec2(4.0 + 3.0 * i as f32, 11.0))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Play the song"))
        })
        .expect("a play button in the playlist header");
    let out = h.click(button, PointerButton::Primary);
    assert_eq!(out, ["transport.set_mode song", "transport.play"]);
}

#[test]
fn double_clicking_stop_silences_every_sound() {
    let mut h = Harness::new(&[]);
    h.settle();
    let stop = [14.0, 38.0, 60.0]
        .into_iter()
        .flat_map(|y| (0..170).map(move |i| pos2(8.0 + 8.0 * i as f32, y)))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|hint| hint.starts_with("Stop and return"))
        })
        .expect("a stop button in the toolbar");
    let mut out = h.click(stop, PointerButton::Primary);
    out.extend(h.click(stop, PointerButton::Primary));
    assert_eq!(out, ["transport.stop", "transport.panic"]);
}

#[test]
fn the_channel_strip_switches_pages_mutes_and_routes_to_a_free_track() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.channel_settings = true;
    h.settle();
    let window = window_rect(&h, "channel-settings");
    let find = |h: &mut Harness, prefix: &str| -> Pos2 {
        (0..14)
            .flat_map(|j| (0..80).map(move |i| (i, j)))
            .map(|(i, j)| window.left_top() + vec2(4.0 + 6.0 * i as f32, 24.0 + 5.0 * j as f32))
            .find(|p| {
                h.frame(vec![Event::PointerMoved(*p)]);
                h.frame(vec![]);
                h.state
                    .hint
                    .as_deref()
                    .is_some_and(|t| t.starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("no strip control hinted {prefix:?}"))
    };
    let functions = find(&mut h, "Functions —");
    h.click(functions, PointerButton::Primary);
    h.settle();
    assert_eq!(
        h.state.channel_tab,
        crate::channel_settings::ChannelTab::Functions
    );
    let on = find(&mut h, "On:");
    h.click(on, PointerButton::Primary);
    h.settle();
    assert!(h.session.project().channels[0].mute);
    let before = h.session.project().channels[0].mixer_track;
    let caption = find(&mut h, "Click to route this channel to a free mixer track");
    h.click(caption, PointerButton::Primary);
    h.settle();
    assert_ne!(h.session.project().channels[0].mixer_track, before);
}

#[test]
fn the_effect_window_strip_bypasses_and_its_header_steps_presets() {
    let mut h = Harness::new(&["mixer.add_effect 1 delay"]);
    h.state.effect_windows.push((1, 0));
    h.settle();
    let window = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("effect-window", 1usize, 0usize))))
        .expect("effect window drawn");
    let find = |h: &mut Harness, prefix: &str| -> Pos2 {
        (0..12)
            .flat_map(|j| (0..80).map(move |i| (i, j)))
            .map(|(i, j)| window.left_top() + vec2(4.0 + 5.0 * i as f32, 8.0 + 5.0 * j as f32))
            .find(|p| {
                h.frame(vec![Event::PointerMoved(*p)]);
                h.frame(vec![]);
                h.state
                    .hint
                    .as_deref()
                    .is_some_and(|t| t.starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("nothing hinted {prefix:?}"))
    };
    let light = find(&mut h, "Effect on");
    h.click(light, PointerButton::Primary);
    h.settle();
    assert!(!h.session.project().mixer[1].effects[0].enabled);
    let before = h.session.project().mixer[1].effects[0].values;
    // The default delay also matches its second preset, so step backwards
    // (to the last one) to see a change.
    let previous = find(&mut h, "Previous preset");
    h.click(previous, PointerButton::Primary);
    h.settle();
    assert_ne!(h.session.project().mixer[1].effects[0].values, before);
    // Right-clicking an arrow opens the preset list instead of stepping.
    let stepped = h.session.project().mixer[1].effects[0].values;
    let next = find(&mut h, "Next preset");
    assert!(!egui::Popup::is_any_open(&h.ctx));
    h.click(next, PointerButton::Secondary);
    h.settle();
    assert!(egui::Popup::is_any_open(&h.ctx), "the preset list opened");
    assert_eq!(h.session.project().mixer[1].effects[0].values, stepped);
}

#[test]
fn instrument_presets_step_from_the_channel_window_header() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.channel_settings = true;
    h.settle();
    let window = window_rect(&h, "channel-settings");
    let previous = (0..80)
        .map(|i| window.left_top() + vec2(4.0 + 4.0 * i as f32, 16.0))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Previous preset"))
        })
        .expect("the header's previous-preset chevron");
    let before = h.session.project().channels[0].params;
    h.click(previous, PointerButton::Primary);
    h.settle();
    assert_ne!(h.session.project().channels[0].params, before);
    h.session.run_line("edit.undo").unwrap();
    assert_eq!(h.session.project().channels[0].params, before);
}

#[test]
fn midi_output_and_stack_windows_show_a_short_strip() {
    let mut h = Harness::new(&["channel.add_midi_out", "channel.add_layer"]);
    h.state.windows.channel_settings = true;
    h.state.channel_tab = crate::channel_settings::ChannelTab::Functions;
    h.settle();
    // MIDI output: only the Sound page, and the strip carries the MIDI channel.
    assert_eq!(
        h.state.channel_tab,
        crate::channel_settings::ChannelTab::Sound
    );
    let window = window_rect(&h, "channel-settings");
    let hints: Vec<String> = (0..12)
        .flat_map(|j| (0..80).map(move |i| (i, j)))
        .map(|(i, j)| window.left_top() + vec2(4.0 + 6.0 * i as f32, 24.0 + 5.0 * j as f32))
        .filter_map(|p| {
            h.frame(vec![Event::PointerMoved(p)]);
            h.frame(vec![]);
            h.state.hint.clone()
        })
        .collect();
    assert!(
        hints
            .iter()
            .any(|t| t.starts_with("The MIDI channel (1–16)"))
    );
    assert!(hints.iter().any(|t| t.starts_with("On:")));
    assert!(!hints.iter().any(|t| t.starts_with("Functions —")));
    assert!(!hints.iter().any(|t| t.starts_with("Mixer track:")));
    // A channel stack keeps the Sound page too.
    h.state.selected_channel = 1;
    h.state.channel_tab = crate::channel_settings::ChannelTab::Functions;
    h.settle();
    assert_eq!(
        h.state.channel_tab,
        crate::channel_settings::ChannelTab::Sound
    );
}

#[test]
fn dragging_the_envelope_peak_sets_the_attack_in_one_step() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.channel_settings = true;
    h.state.channel_tab = crate::channel_settings::ChannelTab::Envelope;
    h.settle();
    let window = window_rect(&h, "channel-settings");
    let handle = (0..40)
        .flat_map(|j| (0..100).map(move |i| (i, j)))
        .map(|(i, j)| window.left_top() + vec2(4.0 + 4.0 * i as f32, 60.0 + 4.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state.hint.as_deref() == Some("Attack: drag across")
        })
        .expect("the attack handle");
    let attack = |h: &Harness| {
        h.session.project().channels[0]
            .params
            .get(musiccraft_project::params::inst::ATTACK)
    };
    let before = attack(&h);
    h.drag(handle, handle + vec2(40.0, 0.0));
    h.settle();
    assert!(attack(&h) > before + 0.01, "{} → {}", before, attack(&h));
    h.session.run_line("edit.undo").unwrap();
    assert!((attack(&h) - before).abs() < 1e-6);
}

#[test]
fn right_clicking_the_keyboard_strip_sets_the_root_key() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.windows.channel_settings = true;
    h.state.channel_tab = crate::channel_settings::ChannelTab::Functions;
    h.settle();
    let window = window_rect(&h, "channel-settings");
    let (key, at) = (0..120)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| window.left_top() + vec2(10.0 + 6.0 * i as f32, 100.0 + 5.0 * j as f32))
        .find_map(|p| {
            h.frame(vec![Event::PointerMoved(p)]);
            h.frame(vec![]);
            let hint = h.state.hint.clone()?;
            let name =
                hint.strip_suffix(" · click to hear it · right-click to make it the root key")?;
            (name != "C5").then(|| (name.to_owned(), p))
        })
        .expect("a key on the keyboard strip");
    h.click(at, PointerButton::Secondary);
    h.settle();
    let root = h.session.project().channels[0]
        .params
        .get(musiccraft_project::params::inst::ROOT_KEY) as u8;
    assert_eq!(musiccraft_editors::instrument::key_name(root), key);
}

#[test]
fn dragging_an_effect_slot_reorders_the_chain() {
    let mut h = Harness::new(&["mixer.add_effect 1 delay", "mixer.add_effect 1 reverb"]);
    h.state.windows.mixer = true;
    h.state.selected_track = 1;
    h.settle();
    let window = window_rect(&h, "mixer-window");
    // Find the slot panel's column first, then walk down it.
    let hint = "Open or close this effect's window · drag to reorder";
    let x = (0..120)
        .map(|i| window.right() - 8.0 - 4.0 * i as f32)
        .find(|x| {
            (0..40).any(|j| {
                let p = pos2(*x, window.top() + 30.0 + 4.0 * j as f32);
                h.frame(vec![Event::PointerMoved(p)]);
                h.frame(vec![]);
                h.state.hint.as_deref() == Some(hint)
            })
        })
        .expect("the slot panel");
    // Each run of hinted points down the column is one slot's name.
    let mut names: Vec<Pos2> = Vec::new();
    let mut inside = false;
    for j in 0..120 {
        let y = window.top() + 30.0 + 2.0 * j as f32;
        // Names differ in length: try a few points from the column leftwards.
        let hit = (0..12).map(|i| pos2(x - 6.0 * i as f32, y)).find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state.hint.as_deref() == Some(hint)
        });
        if let Some(p) = hit.filter(|_| !inside) {
            names.push(p + vec2(0.0, 3.0));
        }
        inside = hit.is_some();
    }
    assert!(names.len() >= 2, "found {names:?}");
    h.drag(names[0], names[1]);
    h.settle();
    let kinds: Vec<_> = h.session.project().mixer[1]
        .effects
        .iter()
        .map(|e| e.kind)
        .collect();
    assert_eq!(
        kinds,
        vec![
            musiccraft_project::params::EffectKind::Reverb,
            musiccraft_project::params::EffectKind::Delay
        ]
    );
}

#[test]
fn an_effect_dropped_on_a_mixer_strip_goes_into_that_track() {
    let mut h = Harness::new(&[]);
    h.state.windows.mixer = true;
    h.settle();
    let window = window_rect(&h, "mixer-window");
    let name = h.session.project().mixer[2].name.clone();
    let strip = (0..40)
        .flat_map(|j| (0..120).map(move |i| (i, j)))
        .map(|(i, j)| window.left_top() + vec2(4.0 + 5.0 * i as f32, 24.0 + 3.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with(&format!("{name} — click")))
        })
        .expect("track 2's number cell");
    // Hover reaches a few points past a widget's edge: step well inside.
    let strip = strip + vec2(8.0, 0.0);
    egui::DragAndDrop::set_payload(
        &h.ctx,
        crate::browser::Dragged::Effect {
            label: "Delay".to_owned(),
            command: "mixer.add_effect".to_owned(),
            args: vec!["delay".to_owned()],
        },
    );
    h.frame(vec![Event::PointerMoved(strip)]);
    let ran = h.frame(vec![Event::PointerButton {
        pos: strip,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(ran, ["mixer.add_effect 2 delay"]);
}

#[test]
fn fitting_to_the_scale_draws_on_its_keys() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.piano_roll = true;
    h.state.roll.scale = Some((0, 0));
    h.settle();
    // Off: the clicked key is used as it is.
    let added = h.click(h.roll_point(5, 61), PointerButton::Primary);
    assert_eq!(added, ["note.add 1 0t 24t 61 100"]);
    h.session.run_line("edit.undo").unwrap();
    h.settle();
    // On: C# in C major lands on C.
    h.state.roll.fit_scale = true;
    h.settle();
    let added = h.click(h.roll_point(5, 61), PointerButton::Primary);
    assert_eq!(added, ["note.add 1 0t 24t 60 100"]);
}

#[test]
fn piano_roll_zoom_keys_step_and_fit() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 1 60"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let before = h.state.roll.px_per_step;
    h.key(egui::Key::PageUp, Modifiers::NONE);
    assert!((h.state.roll.px_per_step - before * 1.5).abs() < 1e-3);
    h.key(egui::Key::Num1, Modifiers::SHIFT);
    assert!((h.state.roll.px_per_step - 12.0).abs() < 1e-3);
    h.key(egui::Key::Num4, Modifiers::SHIFT);
    let (_, clip) = h.state.roll.last_view.expect("roll drawn");
    let steps = h.session.project().current_pattern().length_steps as f32;
    let (low, high) = crate::piano_roll::STEP_WIDTH_RANGE;
    let fit = ((clip.width() - crate::piano_roll::KEYS_WIDTH) / steps).clamp(low, high);
    assert!((h.state.roll.px_per_step - fit).abs() < 1e-3);
}

#[test]
fn playlist_keys_zoom_nudge_and_move_tracks() {
    let mut h = Harness::new(&["pattern.add", "playlist.add_clip 1 1 1"]);
    h.state.windows.playlist = true;
    h.settle();
    let bar = TICKS_PER_BAR;
    let clip = |h: &Harness| {
        let c = h.session.project().playlist.clips[0];
        (c.track, c.start)
    };
    h.click(h.playlist_point(bar + 40, 0), PointerButton::Primary);
    assert_eq!(h.state.playlist.selection.len(), 1);

    // Shift+arrows nudge one snap step (a bar) and move one track, keeping
    // the selection on the clip.
    h.key(egui::Key::ArrowRight, Modifiers::SHIFT);
    assert_eq!(clip(&h), (0, 2 * bar));
    h.key(egui::Key::ArrowDown, Modifiers::SHIFT);
    assert_eq!(clip(&h), (1, 2 * bar));
    h.key(egui::Key::ArrowLeft, Modifiers::SHIFT);
    assert_eq!(clip(&h), (1, bar));
    h.key(egui::Key::ArrowUp, Modifiers::SHIFT);
    assert_eq!(clip(&h), (0, bar));
    h.key(egui::Key::ArrowUp, Modifiers::SHIFT);
    assert_eq!(clip(&h), (0, bar), "the top track stops the move");
    assert_eq!(h.state.playlist.selection, BTreeSet::from([(0, bar)]));

    // Page Up zooms in; Shift+1 picks a preset; Shift+4 goes all the way out
    // and back.
    let before = h.state.playlist.px_per_bar;
    h.key(egui::Key::PageUp, Modifiers::NONE);
    assert!((h.state.playlist.px_per_bar - before * 1.5).abs() < 1e-3);
    h.key(egui::Key::Num1, Modifiers::SHIFT);
    assert!((h.state.playlist.px_per_bar - 32.0).abs() < 1e-3);
    h.key(egui::Key::Num4, Modifiers::SHIFT);
    assert!((h.state.playlist.px_per_bar - 16.0).abs() < 1e-3);
    h.key(egui::Key::Num4, Modifiers::SHIFT);
    assert!((h.state.playlist.px_per_bar - 32.0).abs() < 1e-3);
}

#[test]
fn the_toolbar_pattern_field_steps_renames_and_reveals() {
    let mut h = Harness::new(&["pattern.add", "playlist.add_clip 2 1 6"]);
    h.state.windows.playlist = false;
    h.settle();
    let field = [14.0, 38.0, 60.0]
        .into_iter()
        .flat_map(|y| (0..170).map(move |i| pos2(8.0 + 8.0 * i as f32, y)))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|hint| hint.starts_with("Pattern: click"))
        })
        .expect("the pattern field in the toolbar");
    let wheel = |y: f32| Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, y),
        phase: egui::TouchPhase::Move,
        modifiers: Modifiers::NONE,
    };
    assert_eq!(h.session.project().current_pattern, 1);
    assert_eq!(h.frame(vec![wheel(1.0)]), ["pattern.prev"]);
    assert_eq!(h.frame(vec![wheel(-1.0)]), ["pattern.next"]);
    assert_eq!(h.session.project().current_pattern, 1);

    // Double-click renames in place; Esc cancels.
    h.click(field, PointerButton::Primary);
    h.click(field, PointerButton::Primary);
    assert!(h.state.renaming.is_some());
    h.key(egui::Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(h.state.renaming.is_none());

    // Ctrl+click opens the playlist at the pattern's first clip.
    h.modifiers = Modifiers::COMMAND;
    h.click(field, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert!(h.state.windows.playlist);
    h.settle();
    let offset = h.state.playlist.offset;
    assert!(offset.x > 0.0, "scrolled to bar 7: {offset:?}");
}

#[test]
fn right_clicking_the_piano_roll_button_opens_it_maximized() {
    let mut h = Harness::new(&[]);
    h.state.windows.piano_roll = false;
    h.settle();
    let find = |h: &mut Harness, prefix: &str| {
        [14.0, 38.0, 60.0]
            .into_iter()
            .flat_map(|y| (0..170).map(move |i| pos2(8.0 + 8.0 * i as f32, y)))
            .find(|p| {
                h.frame(vec![Event::PointerMoved(*p)]);
                h.frame(vec![]);
                h.state
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("{prefix:?} in the toolbar"))
    };
    let roll = find(&mut h, "Piano roll (F7)");
    h.click(roll, PointerButton::Secondary);
    h.settle();
    assert!(h.state.windows.piano_roll);
    assert!(crate::chrome::maximized(
        &h.ctx,
        egui::Id::new("piano-roll")
    ));

    // The layout button's first item puts every window back.
    let layout = find(&mut h, "Window layout");
    h.click(layout, PointerButton::Primary);
    h.settle();
    let item = (0..40)
        .map(|j| layout + vec2(20.0, 12.0 + 4.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.ctx
                .memory(|m| m.layer_id_at(*p))
                .is_some_and(|l| l.order == egui::Order::Foreground)
        });
    let item = item.expect("the layout menu") + vec2(0.0, 10.0);
    h.click(item, PointerButton::Primary);
    h.settle();
    assert!(!crate::chrome::maximized(
        &h.ctx,
        egui::Id::new("piano-roll")
    ));
}

#[test]
fn the_playlist_follows_the_playhead_unless_turned_off() {
    let mut h = Harness::new(&["transport.toggle_mode"]);
    assert_eq!(
        h.session.project().play_mode,
        musiccraft_project::PlayMode::Song
    );
    h.state.windows.playlist = true;
    h.state.playlist.view.follow = false;
    h.engine.playing = true;
    h.engine.position_ticks = f64::from(40 * TICKS_PER_BAR);
    h.settle();
    assert_eq!(h.state.playlist.offset.x, 0.0, "stays put");
    h.state.playlist.view.follow = true;
    h.settle();
    assert!(h.state.playlist.offset.x > 0.0, "pages to the playhead");
}

#[test]
fn playlist_copy_and_paste_use_the_desktop_clipboard_events() {
    let mut h = Harness::new(&["pattern.add", "playlist.add_clip 1 1 1"]);
    h.state.windows.playlist = true;
    h.settle();
    let bar = TICKS_PER_BAR;
    h.click(h.playlist_point(bar + 40, 0), PointerButton::Primary);
    // Ctrl+C arrives as a copy event, Ctrl+V as a paste of the system text.
    h.frame(vec![Event::Copy]);
    h.engine.position_ticks = f64::from(4 * bar + 100);
    h.frame(vec![]);
    let pasted = h.frame(vec![Event::Paste("MusicCraft: clips".to_owned())]);
    assert_eq!(pasted.len(), 1, "{pasted:?}");
    let starts: Vec<(u32, u32)> = h
        .session
        .project()
        .playlist
        .clips
        .iter()
        .map(|c| (c.track, c.start))
        .collect();
    assert_eq!(starts, [(0, bar), (0, 4 * bar)]);
    assert_eq!(h.state.playlist.selection, BTreeSet::from([(0, 4 * bar)]));
    // Cut removes the pasted copy again.
    h.frame(vec![Event::Cut]);
    assert_eq!(h.session.project().playlist.clips.len(), 1);
}

#[test]
fn rack_keys_copy_steps_toggle_the_graph_and_step_groups() {
    let mut h = Harness::new(&[
        "channel.add_sampler builtin:kick",
        "channel.add_sampler builtin:hat",
        "step.toggle 1 1",
        "step.toggle 1 5",
        "step.toggle 2 3",
        "channel.set_group 2 Drums",
    ]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    h.frame(vec![Event::PointerMoved(rack.center())]);
    let steps = |h: &Harness, channel: usize| -> Vec<u32> {
        let project = h.session.project();
        let id = project.channels[channel].id;
        project
            .current_pattern()
            .notes_of(id)
            .map(|n| n.start / TICKS_PER_STEP)
            .collect()
    };
    // Copy channel 1's steps onto channel 2 (as the desktop sends Ctrl+C/V).
    h.state.selected_channel = 0;
    h.frame(vec![Event::Copy]);
    h.state.selected_channel = 1;
    let pasted = h.frame(vec![Event::Paste("MusicCraft: steps".to_owned())]);
    assert_eq!(pasted.len(), 1, "one undo step: {pasted:?}");
    assert_eq!(steps(&h, 1), steps(&h, 0));
    assert_eq!(steps(&h, 1).len(), 2);
    h.frame(vec![Event::Cut]);
    assert!(steps(&h, 1).is_empty());

    h.key(egui::Key::K, Modifiers::COMMAND);
    assert!(h.state.graph.is_some());
    h.key(egui::Key::K, Modifiers::COMMAND);
    assert!(h.state.graph.is_none());

    // Page Down steps All → Drums → No group → All.
    h.key(egui::Key::PageDown, Modifiers::NONE);
    assert_eq!(h.state.rack_group.as_deref(), Some("Drums"));
    h.key(egui::Key::PageUp, Modifiers::NONE);
    assert_eq!(h.state.rack_group, None);
    h.key(egui::Key::PageUp, Modifiers::NONE);
    assert_eq!(h.state.rack_group.as_deref(), Some(""));

    h.key(egui::Key::M, Modifiers::ALT);
    assert!(h.state.rack_hide_tracks);
    h.key(egui::Key::F2, Modifiers::NONE);
    assert!(
        format!("{:?}", h.state.dialog).contains("RenameChannel"),
        "F2 over the rack renames the selected channel"
    );
}

#[test]
fn f2_elsewhere_renames_the_selected_pattern() {
    let mut h = Harness::new(&["pattern.add Verse"]);
    h.settle();
    h.frame(vec![Event::PointerMoved(pos2(700.0, 600.0))]);
    h.key(egui::Key::F2, Modifiers::NONE);
    let dialog = format!("{:?}", h.state.dialog);
    assert!(dialog.contains("RenamePattern(1)"), "{dialog}");
    assert!(dialog.contains("Verse"), "{dialog}");
}

#[test]
fn shift_down_in_the_browser_sends_the_next_sound_to_the_selected_sampler() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.selected_channel = h.session.project().channels.len() - 1;
    let channel = h.state.selected_channel + 1;
    h.state.windows.browser = true;
    h.settle();
    h.frame(vec![Event::PointerMoved(pos2(60.0, 300.0))]);
    let selected = |h: &Harness| {
        h.ctx
            .data(|d| d.get_temp::<String>(egui::Id::new("browser-selected-row")))
            .unwrap_or_default()
    };
    h.key(egui::Key::ArrowDown, Modifiers::NONE);
    for _ in 0..1500 {
        let key = selected(&h);
        if key.starts_with("sound:") {
            break;
        }
        h.key(egui::Key::ArrowRight, Modifiers::NONE);
        if selected(&h) == key {
            h.key(egui::Key::ArrowDown, Modifiers::NONE);
        }
    }
    let first = selected(&h);
    assert!(first.starts_with("sound:"), "{first}");
    h.other.clear();
    let out = h.key(egui::Key::ArrowDown, Modifiers::SHIFT);
    let next = selected(&h);
    let sample = next.strip_prefix("sound:").expect("the next sound");
    assert_ne!(next, first);
    assert_eq!(out, [format!("channel.set_sample {channel} {sample}")]);
    assert!(
        h.other
            .iter()
            .any(|a| matches!(a, Action::PreviewSample(s) if s == sample))
    );
    // Back up again: the two sends are one undo step.
    h.key(egui::Key::ArrowUp, Modifiers::SHIFT);
    assert_eq!(selected(&h), first);
    h.session.run_line("edit.undo").unwrap();
    let plays = |h: &Harness| match &h.session.project().channels[channel - 1].instrument {
        musiccraft_project::Instrument::Sampler { sample } => sample.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(plays(&h), "builtin:kick");
}

#[test]
fn backspace_in_the_browser_goes_back_and_forward_through_opened_folders() {
    let mut h = Harness::new(&[]);
    h.state.windows.browser = true;
    h.settle();
    h.frame(vec![Event::PointerMoved(pos2(60.0, 300.0))]);
    let selected = |h: &Harness| {
        h.ctx
            .data(|d| d.get_temp::<String>(egui::Id::new("browser-selected-row")))
            .unwrap_or_default()
    };
    // Open two folders with Right.
    let mut opened = Vec::new();
    h.key(egui::Key::ArrowDown, Modifiers::NONE);
    for _ in 0..40 {
        let key = selected(&h);
        h.key(egui::Key::ArrowRight, Modifiers::NONE);
        if h.state.browser.open.get(&key) == Some(&true) && !opened.contains(&key) {
            opened.push(key);
            if opened.len() == 2 {
                break;
            }
        } else {
            h.key(egui::Key::ArrowDown, Modifiers::NONE);
        }
    }
    assert_eq!(opened.len(), 2, "{opened:?}");
    h.key(egui::Key::Backspace, Modifiers::NONE);
    assert_eq!(selected(&h), opened[1]);
    assert_eq!(h.state.browser.open.get(&opened[1]), Some(&false));
    h.key(egui::Key::Backspace, Modifiers::NONE);
    assert_eq!(selected(&h), opened[0]);
    h.key(egui::Key::Backspace, Modifiers::SHIFT);
    assert_eq!(selected(&h), opened[0]);
    assert_eq!(h.state.browser.open.get(&opened[0]), Some(&true));
}

#[test]
fn the_browser_walks_its_tree_with_the_arrow_keys_and_enter_adds_an_effect() {
    let mut h = Harness::new(&[]);
    h.state.windows.browser = true;
    h.settle();
    h.frame(vec![Event::PointerMoved(pos2(60.0, 300.0))]);
    let selected = |h: &Harness| {
        h.ctx
            .data(|d| d.get_temp::<String>(egui::Id::new("browser-selected-row")))
    };
    h.key(egui::Key::ArrowDown, Modifiers::NONE);
    let first = selected(&h).expect("Down selects the first row");
    // Right opens folders and steps in; Down moves on; stop at an effect.
    let mut out = Vec::new();
    for _ in 0..80 {
        let key = selected(&h).unwrap_or_default();
        if key.starts_with("plugin:") {
            out = h.key(egui::Key::Enter, Modifiers::NONE);
            break;
        }
        h.key(egui::Key::ArrowRight, Modifiers::NONE);
        if selected(&h).unwrap_or_default() == key {
            h.key(egui::Key::ArrowDown, Modifiers::NONE);
        }
    }
    assert!(
        out.iter().any(|c| c.starts_with("mixer.add_effect")),
        "Enter on an effect adds it: {out:?}"
    );
    // Left steps out to the parent folder; Up at the top stays.
    h.key(egui::Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(selected(&h).as_deref(), Some("plugins/effects"));
    for _ in 0..80 {
        h.key(egui::Key::ArrowUp, Modifiers::NONE);
    }
    assert_eq!(selected(&h), Some(first));
}

#[test]
fn this_projects_history_rows_undo_and_redo_to_a_step() {
    let mut h = Harness::new(&[]);
    for line in [
        "project.set_tempo 100",
        "project.set_tempo 110",
        "project.set_tempo 120",
    ] {
        h.session.run_line(line).unwrap();
    }
    h.state.windows.browser = true;
    h.state.browser.tab = crate::browser::Tab::Project;
    h.state
        .browser
        .open
        .insert("project/history".to_owned(), true);
    h.settle();
    let row_with = |h: &mut Harness, hint: &str| {
        (0..250)
            .map(|j| pos2(80.0, 40.0 + 3.0 * j as f32))
            .find(|p| {
                h.frame(vec![Event::PointerMoved(*p)]);
                h.frame(vec![]);
                h.state.hint.as_deref().is_some_and(|t| t.starts_with(hint))
            })
    };
    // The undo rows run newest first; the third is the first tempo change.
    let first = row_with(&mut h, "Click to undo").expect("an undo row");
    let rows: Vec<Pos2> = (0..60)
        .map(|j| first + vec2(0.0, 3.0 * j as f32))
        .filter(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Click to undo"))
        })
        .collect();
    let last = *rows.last().unwrap();
    let out = h.click(last, PointerButton::Primary);
    assert_eq!(
        out,
        ["edit.undo", "edit.undo"],
        "back to just after the first"
    );
    assert!((h.session.project().tempo_bpm() - 100.0).abs() < 0.01);
    h.settle();
    let redo = row_with(&mut h, "Click to redo").expect("a redo row");
    let out = h.click(redo, PointerButton::Primary);
    assert!(
        !out.is_empty() && out.iter().all(|c| c == "edit.redo"),
        "{out:?}"
    );
    assert!(h.session.project().tempo_bpm() > 100.0);
}

#[test]
fn dragging_a_track_divider_resizes_that_track_alone() {
    let mut h = Harness::new(&[]);
    h.state.windows.playlist = true;
    h.settle();
    let (grid, clip) = h.state.playlist.last_view.unwrap();
    let row_h = grid.row_height;
    let edge = pos2(clip.left() + 40.0, grid.y(1) - 1.0);
    let ran = h.drag(edge, edge + vec2(0.0, row_h));
    assert_eq!(ran, ["playlist.set_track_height 1 200"]);
    h.settle();
    let (grid, _) = h.state.playlist.last_view.unwrap();
    assert!((grid.height_of(0) - 2.0 * row_h).abs() < 0.5);
    assert!(
        (grid.height_of(1) - row_h).abs() < 0.5,
        "the next track keeps its height"
    );
    // Alt+click on the divider returns the track to the global height.
    h.modifiers = Modifiers::ALT;
    let edge = pos2(clip.left() + 40.0, grid.y(1) - 1.0);
    let ran = h.click(edge, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(ran, ["playlist.set_track_height 1 reset"]);
    assert!(h.session.project().playlist.track_heights.is_empty());
}

#[test]
fn track_headers_select_a_track_and_take_dropped_sounds() {
    let mut h = Harness::new(&[]);
    h.state.windows.playlist = true;
    h.settle();
    let (grid, clip) = h.state.playlist.last_view.unwrap();
    let header = |row: u32| pos2(clip.left() + 30.0, grid.y(row) + 8.0);
    h.click(header(2), PointerButton::Primary);
    assert_eq!(h.state.playlist.track, 2);
    h.key(egui::Key::ArrowDown, Modifiers::COMMAND);
    assert_eq!(h.state.playlist.track, 3);
    h.key(egui::Key::ArrowUp, Modifiers::COMMAND);
    h.key(egui::Key::ArrowUp, Modifiers::COMMAND);
    assert_eq!(h.state.playlist.track, 1);
    // A sound dropped on a header lands at the start of the view on that
    // track, which takes the sound's name.
    egui::DragAndDrop::set_payload(
        &h.ctx,
        crate::browser::Dragged::Sample("builtin:clap".to_owned()),
    );
    let at = header(1);
    h.frame(vec![Event::PointerMoved(at)]);
    let ran = h.frame(vec![Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    assert!(
        h.other.iter().any(|a| matches!(
            a,
            Action::DropAudio { track: 1, start: 0, sample } if sample == "builtin:clap"
        )),
        "the sound goes to track 2 at the view's start"
    );
    assert!(
        ran.contains(&"playlist.rename_track 2 clap".to_owned()),
        "{ran:?}"
    );
}

#[test]
fn a_track_pinned_right_sits_after_the_scrolling_strips() {
    let mut h = Harness::new(&["mixer.set_dock 3 right"]);
    h.state.windows.mixer = true;
    h.settle();
    let window = window_rect(&h, "mixer-window");
    let mut hint_at = |p: Pos2| {
        h.frame(vec![Event::PointerMoved(p)]);
        h.frame(vec![]);
        h.state.hint.clone().unwrap_or_default()
    };
    let row = (0..40)
        .map(|j| window.top() + 24.0 + 3.0 * j as f32)
        .find(|&y| hint_at(pos2(window.left() + 20.0, y)).starts_with("Master — click"))
        .expect("the master's number cell, at the left");
    let mut strips = Vec::new();
    let mut x = window.left() + 4.0;
    while x < window.right() {
        let hint = hint_at(pos2(x, row));
        if let Some((name, _)) = hint.split_once(" — click") {
            if strips.last() != Some(&name.to_owned()) {
                strips.push(name.to_owned());
            }
        }
        x += 6.0;
    }
    let names: Vec<String> = h
        .session
        .project()
        .mixer
        .iter()
        .map(|t| t.name.clone())
        .collect();
    let at = |n: usize| strips.iter().position(|s| *s == names[n]);
    assert_eq!(at(0), Some(0), "{strips:?}");
    assert_eq!(at(1), Some(1), "{strips:?}");
    assert!(at(2) < at(4), "{strips:?}");
    assert_eq!(
        at(3),
        Some(strips.len() - 1),
        "track 3 is pinned right: {strips:?}"
    );
}

#[test]
fn a_saved_window_layout_comes_back_on_the_next_start() {
    let mut h = Harness::new(&[]);
    h.state.windows.mixer = true;
    h.state.windows.piano_roll = true;
    h.settle();
    let mut layout = crate::window_layout(&h.ctx, &h.state);
    assert!(layout.open.contains(&"mixer".to_owned()), "{layout:?}");
    // Next start: the mixer was moved and folded, the piano roll closed.
    layout.open.retain(|n| n != "piano_roll");
    for (name, rect) in &mut layout.rects {
        if name == "mixer" {
            *rect = [300.0, 160.0, 500.0, 320.0];
        }
    }
    layout.folded.push("mixer".to_owned());
    let mut next = Harness::new(&[]);
    crate::restore_window_layout(&next.ctx, &mut next.state, &layout);
    next.settle();
    assert!(!next.state.windows.piano_roll);
    assert!(next.state.windows.mixer);
    let mixer = window_rect(&next, "mixer-window");
    assert!(
        (mixer.left() - 300.0).abs() < 1.0 && (mixer.top() - 160.0).abs() < 1.0,
        "{mixer:?}"
    );
    assert!(crate::chrome::collapsed(
        &next.ctx,
        egui::Id::new("mixer-window")
    ));
    let again = crate::window_layout(&next.ctx, &next.state);
    assert!(again.folded.contains(&"mixer".to_owned()));
    assert!(!again.open.contains(&"piano_roll".to_owned()));
}

#[test]
fn alt_clicking_a_channel_name_opens_another_channel_window() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "channel.add_synth",
        "channel.rename 2 Pad",
    ]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    // Name buttons hint their kind; the second row's is below the first.
    let find = |h: &mut Harness, below: f32| {
        (0..40)
            .flat_map(|j| (0..60).map(move |i| (i, j)))
            .map(|(i, j)| rack.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 3.0 * j as f32))
            .filter(|p| p.y > below)
            .find(|p| {
                h.frame(vec![Event::PointerMoved(*p)]);
                h.frame(vec![]);
                h.state.hint.as_deref() == Some("Synth")
            })
            .expect("a channel's name")
    };
    let synth = find(&mut h, 0.0);
    let pad = find(&mut h, synth.y + 28.0);
    h.click(synth, PointerButton::Primary);
    h.settle();
    assert!(h.state.windows.channel_settings);
    h.modifiers = Modifiers::ALT;
    h.click(pad, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    h.settle();
    assert_eq!(
        h.state.selected_channel, 0,
        "the first window keeps its channel"
    );
    let pad_id = h.session.project().channels[1].id;
    assert_eq!(h.state.kept_channels.len(), 1);
    assert_eq!(h.state.kept_channels[0].0, pad_id);
    let kept = h
        .ctx
        .memory(|m| m.area_rect(egui::Id::new(("channel-settings", pad_id.0))));
    assert!(kept.is_some(), "the second window is drawn");
    // Alt+clicking the same channel again opens no third window.
    h.modifiers = Modifiers::ALT;
    h.click(pad, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(h.state.kept_channels.len(), 1);
}

#[test]
fn the_toolbar_utilities_open_help_and_the_sample_editor() {
    let mut h = Harness::new(&[]);
    h.settle();
    let find = |h: &mut Harness, prefix: &str| {
        [14.0, 38.0, 60.0]
            .into_iter()
            .flat_map(|y| (0..170).map(move |i| pos2(8.0 + 8.0 * i as f32, y)))
            .find(|p| {
                h.frame(vec![Event::PointerMoved(*p)]);
                h.frame(vec![]);
                h.state
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("no toolbar button hinted {prefix:?}"))
    };
    let help = find(&mut h, "Help: keyboard shortcuts");
    h.click(help, PointerButton::Primary);
    assert!(h.state.windows.shortcuts);
    h.frame(vec![Event::PointerMoved(pos2(700.0, 500.0))]);
    let sample = find(&mut h, "Sample editor");
    h.click(sample, PointerButton::Primary);
    assert!(h.state.windows.channel_settings);
}

#[test]
fn right_clicking_the_snap_button_sets_the_new_note_length() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let window = window_rect(&h, "piano-roll");
    let snap = (0..60)
        .map(|i| window.left_top() + vec2(4.0 + 6.0 * i as f32, 12.0))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Snap:"))
        })
        .expect("the snap button");
    h.state.roll.snap = crate::geometry::Snap::Beat;
    h.state.roll.last_length = 1;
    h.click(snap, PointerButton::Secondary);
    assert_eq!(
        h.state.roll.last_length,
        crate::geometry::Snap::Beat.ticks_at(0, &h.state.roll.meter)
    );
}

#[test]
fn ctrl_shift_clicks_select_several_mixer_tracks_and_their_faders_move_together() {
    let mut h = Harness::new(&[]);
    h.state.windows.mixer = true;
    h.settle();
    let window = window_rect(&h, "mixer-window");
    let names: Vec<String> = h
        .session
        .project()
        .mixer
        .iter()
        .map(|t| t.name.clone())
        .collect();
    let hint_at = |h: &mut Harness, p: Pos2| {
        h.frame(vec![Event::PointerMoved(p)]);
        h.frame(vec![]);
        h.state.hint.clone().unwrap_or_default()
    };
    let row = (0..40)
        .map(|j| window.top() + 24.0 + 3.0 * j as f32)
        .find(|&y| hint_at(&mut h, pos2(window.left() + 20.0, y)).starts_with("Master — click"))
        .expect("the master's number cell");
    // The middle of the run of points whose hint names the track.
    let column = |h: &mut Harness, track: usize| {
        let wanted = format!("{} — click", names[track]);
        let xs: Vec<f32> = (0..)
            .map(|j| window.left() + 4.0 + 2.0 * j as f32)
            .take_while(|&x| x < window.right())
            .filter(|&x| hint_at(h, pos2(x, row)).starts_with(&wanted))
            .collect();
        assert!(!xs.is_empty(), "no strip for track {track}");
        (xs[0] + xs[xs.len() - 1]) / 2.0
    };
    let (one, two) = (column(&mut h, 1), column(&mut h, 2));
    // Rest the pointer, so the scan's motion does not fling the strips
    // sideways when the button is released.
    let rest = |h: &mut Harness, p: Pos2| {
        h.frame(vec![Event::PointerMoved(p)]);
        for _ in 0..30 {
            h.frame(vec![]);
        }
    };
    rest(&mut h, pos2(one, row));
    h.click(pos2(one, row), PointerButton::Primary);
    rest(&mut h, pos2(two, row));
    h.modifiers = Modifiers::COMMAND | Modifiers::SHIFT;
    h.click(pos2(two, row), PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    assert_eq!(h.state.selected_track, 2, "the last click is the primary");
    assert_eq!(
        h.state.mixer_selection.iter().copied().collect::<Vec<_>>(),
        [1]
    );

    let fader = (0..200)
        .map(|j| row + 60.0 + 2.0 * j as f32)
        .find(|&y| hint_at(&mut h, pos2(two, y)).starts_with("Level"))
        .expect("track 2's fader")
        + 30.0;
    rest(&mut h, pos2(two, fader));
    let before: Vec<f32> = h.session.project().mixer.iter().map(|t| t.volume).collect();
    let ran = h.drag(pos2(two, fader), pos2(two, fader + 40.0));
    assert!(ran.iter().any(|c| c.starts_with("edit.batch")), "{ran:?}");
    let after: Vec<f32> = h.session.project().mixer.iter().map(|t| t.volume).collect();
    assert!(after[2] != before[2], "{before:?} → {after:?}");
    assert!((after[1] - after[2]).abs() < 1e-3, "{after:?}");
    assert_eq!(after[3], before[3], "unselected tracks stay");
    h.session.undo().unwrap();
    let undone: Vec<f32> = h.session.project().mixer.iter().map(|t| t.volume).collect();
    assert_eq!(undone, before, "the drag is one undo step");

    h.key(egui::Key::A, Modifiers::COMMAND);
    assert_eq!(h.state.mixer_selection.len() + 1, names.len());
    h.key(egui::Key::Escape, Modifiers::NONE);
    assert!(h.state.mixer_selection.is_empty());
    assert_eq!(h.state.selected_track, 2);
    rest(&mut h, pos2(one, row));
    h.click(pos2(one, row), PointerButton::Primary);
    assert_eq!(h.state.selected_track, 1);
}

#[test]
fn alt_p_hides_the_playlist_clip_list_and_the_lanes_take_its_width() {
    let mut h = Harness::new(&["pattern.add"]);
    h.state.windows.playlist = true;
    h.settle();
    let lanes = |h: &Harness| h.state.playlist.last_view.expect("playlist drawn").1;
    let shown = lanes(&h);
    h.click(h.playlist_point(0, 0), PointerButton::Primary);
    h.key(egui::Key::P, Modifiers::ALT);
    h.settle();
    assert!(!h.state.playlist.view.clip_list);
    let hidden = lanes(&h);
    assert!(
        hidden.left() < shown.left() - 100.0,
        "{shown:?} → {hidden:?}"
    );
    h.key(egui::Key::P, Modifiers::ALT);
    h.settle();
    assert!(h.state.playlist.view.clip_list);
    assert_eq!(lanes(&h).left(), shown.left());
}

#[test]
fn the_playlist_clip_list_can_sit_on_the_right_and_change_width() {
    let mut h = Harness::new(&["pattern.add"]);
    h.state.windows.playlist = true;
    h.settle();
    let lanes = |h: &Harness| h.state.playlist.last_view.expect("playlist drawn").1;
    let left = lanes(&h);
    h.state.playlist.view.list_width = 2;
    h.settle();
    assert!(
        (lanes(&h).left() - left.left() - 50.0).abs() < 1.0,
        "wide adds 50 points"
    );
    h.state.playlist.view.list_width = 1;
    h.state.playlist.view.list_right = true;
    h.settle();
    let right = lanes(&h);
    assert!(right.left() < left.left() - 100.0, "{left:?} → {right:?}");
    assert!(right.right() < left.right() - 100.0, "{left:?} → {right:?}");
    // The list still works from its new side: its kind switches are there.
    let window = window_rect(&h, "playlist-window");
    let hint = (0..60)
        .map(|j| pos2(right.right() + 10.0 + 2.0 * j as f32, window.top() + 40.0))
        .filter(|&p| p.x < window.right())
        .find_map(|p| {
            h.frame(vec![Event::PointerMoved(p)]);
            h.frame(vec![]);
            h.state
                .hint
                .clone()
                .filter(|t| t.starts_with("Patterns: show only"))
        });
    assert!(hint.is_some(), "the kind switches sit right of the lanes");
}

#[test]
fn middle_dragging_pans_the_piano_roll_and_the_playlist() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0t 24t 60 100"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let origin = |h: &Harness| h.state.roll.last_view.expect("piano roll drawn").0.origin;
    let before = origin(&h);
    let from = h.roll_point(48, 60);
    h.drag_with(from, from - vec2(0.0, 120.0), PointerButton::Middle);
    h.settle();
    assert!(
        origin(&h).y < before.y - 60.0,
        "{before:?} → {:?}",
        origin(&h)
    );
    assert_eq!(h.roll_notes(), [(0, 60)], "panning leaves the notes alone");

    let mut h = Harness::new(&["pattern.add"]);
    h.state.windows.playlist = true;
    h.settle();
    let origin = |h: &Harness| h.state.playlist.last_view.expect("playlist drawn").0.origin;
    let before = origin(&h);
    let from = h.playlist_point(TICKS_PER_BAR, 3);
    let ran = h.drag_with(from, from - vec2(0.0, 80.0), PointerButton::Middle);
    h.settle();
    assert!(ran.is_empty(), "{ran:?}");
    assert!(
        origin(&h).y < before.y - 40.0,
        "{before:?} → {:?}",
        origin(&h)
    );
}

#[test]
fn ctrl_dragging_keys_selects_a_range_and_middle_dragging_them_zooms() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "note.add 1 0 1 60",
        "note.add 1 4 1 62",
        "note.add 1 8 1 64",
        "note.add 1 12 1 67",
    ]);
    h.state.windows.piano_roll = true;
    h.settle();
    let step = TICKS_PER_STEP;
    let (_, clip) = h.state.roll.last_view.expect("piano roll drawn");
    let key = |h: &Harness, k: u8| pos2(clip.left() + 40.0, h.roll_point(0, k).y);
    h.modifiers = Modifiers::COMMAND;
    let ran = h.drag(key(&h, 60), key(&h, 64));
    h.modifiers = Modifiers::NONE;
    h.settle();
    assert!(ran.is_empty(), "{ran:?}");
    let selected: Vec<_> = h.state.roll.selection.iter().copied().collect();
    assert_eq!(selected, [(0, 60), (4 * step, 62), (8 * step, 64)]);
    assert!(h.state.roll.preview.is_none());

    let row = |h: &Harness| {
        h.state
            .roll
            .last_view
            .expect("piano roll drawn")
            .0
            .row_height
    };
    let before = row(&h);
    let at = key(&h, 62);
    h.drag_with(at, at - vec2(0.0, 40.0), PointerButton::Middle);
    h.settle();
    assert!(row(&h) > before, "{before} → {}", row(&h));
    // The key first pressed stays under the press point.
    let (grid, _) = h.state.roll.last_view.expect("piano roll drawn");
    let held = grid.row_at(at.y).map(crate::piano_roll::key_of_row);
    assert_eq!(held, Some(62));
}

#[test]
fn the_piano_roll_splitter_resizes_hides_and_restores_the_property_lane() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 1 60"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let (_, clip) = h.state.roll.last_view.expect("piano roll drawn");
    let x = clip.center().x;
    let split = |h: &Harness| clip.bottom() - h.state.roll.lane_height - 2.0;
    h.state.hint = None;
    h.frame(vec![Event::PointerMoved(pos2(x, split(&h)))]);
    h.frame(vec![]);
    assert!(
        h.state
            .hint
            .clone()
            .unwrap_or_default()
            .starts_with("Drag to resize the property lane"),
        "{:?}",
        h.state.hint
    );
    let from = pos2(x, split(&h));
    let ran = h.drag(from, from - vec2(0.0, 60.0));
    assert!(ran.is_empty(), "{ran:?}");
    assert!(
        (h.state.roll.lane_height - 156.0).abs() < 4.0,
        "{}",
        h.state.roll.lane_height
    );

    let at = pos2(x, split(&h));
    h.click(at, PointerButton::Primary);
    h.click(at, PointerButton::Primary);
    assert!(!h.state.roll.lane_shown, "double-click hides the lane");
    h.click(h.roll_point(96, 60), PointerButton::Primary);
    h.key(egui::Key::Enter, Modifiers::SHIFT);
    assert!(h.state.roll.lane_shown, "Shift+Enter shows it again");
    h.settle();
    h.click(pos2(x, split(&h)), PointerButton::Secondary);
    assert_eq!(
        h.state.roll.lane_height, 96.0,
        "right-click restores the default"
    );
}

#[test]
fn the_mixer_slot_panel_can_sit_left_of_the_strips() {
    let mut h = Harness::new(&[]);
    h.state.windows.mixer = true;
    h.settle();
    let window = window_rect(&h, "mixer-window");
    let master_at = |h: &mut Harness| {
        let mut hint_at = |p: Pos2| {
            h.frame(vec![Event::PointerMoved(p)]);
            h.frame(vec![]);
            h.state.hint.clone().unwrap_or_default()
        };
        (0..40)
            .flat_map(|j| (0..120).map(move |i| (i, j)))
            .map(|(i, j)| {
                pos2(
                    window.left() + 4.0 + 6.0 * i as f32,
                    window.top() + 24.0 + 3.0 * j as f32,
                )
            })
            .find(|&p| hint_at(p).starts_with("Master — click"))
            .expect("the master's number cell")
            .x
    };
    let right = master_at(&mut h);
    assert!(right < window.left() + 40.0, "{right} {window:?}");
    h.state.mixer_panel_left = true;
    h.settle();
    let left = master_at(&mut h);
    assert!(
        left > right + 200.0,
        "the panel now comes first: {right} → {left}"
    );
}

#[test]
fn the_tempo_chevrons_step_it_one_bpm_a_click() {
    let mut h = Harness::new(&[]);
    h.settle();
    let hint_at = |h: &mut Harness, p: Pos2| {
        h.frame(vec![Event::PointerMoved(p)]);
        h.frame(vec![]);
        h.state.hint.clone().unwrap_or_default()
    };
    let x = [14.0, 38.0, 60.0]
        .into_iter()
        .flat_map(|y| (0..340).map(move |i| pos2(8.0 + 4.0 * i as f32, y)))
        .find(|&p| hint_at(&mut h, p).starts_with("Tempo up or down"))
        .expect("the tempo chevrons")
        .x;
    let ys: Vec<f32> = (0..40)
        .map(|j| 2.0 * j as f32)
        .filter(|&y| hint_at(&mut h, pos2(x, y)).starts_with("Tempo up or down"))
        .collect();
    let mid = (ys[0] + ys[ys.len() - 1]) / 2.0;
    let xs: Vec<f32> = (-8..8)
        .map(|i| x + i as f32)
        .filter(|&x| hint_at(&mut h, pos2(x, mid)).starts_with("Tempo up or down"))
        .collect();
    let x = (xs[0] + xs[xs.len() - 1]) / 2.0;
    let (top, bottom) = (mid - 7.0, mid + 7.0);
    let tempo = |h: &Harness| h.session.project().tempo_bpm();
    let start = tempo(&h);
    h.click(pos2(x, top), PointerButton::Primary);
    assert!(
        (tempo(&h) - start - 1.0).abs() < 1e-6,
        "{start} → {}",
        tempo(&h)
    );
    h.click(pos2(x, bottom), PointerButton::Primary);
    h.frame(vec![]);
    h.click(pos2(x, bottom), PointerButton::Primary);
    assert!(
        (tempo(&h) - start + 1.0).abs() < 1e-6,
        "{start} → {}",
        tempo(&h)
    );
    h.session.undo().unwrap();
    assert!(
        (tempo(&h) - start).abs() < 1e-6,
        "each click is its own undo step"
    );
}

#[test]
fn ctrl_pressing_a_channel_name_previews_it_and_shift_wheel_moves_it() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "channel.add_synth",
        "channel.rename 2 Pad",
    ]);
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let synth = (0..40)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| rack.left_top() + vec2(8.0 + 4.0 * i as f32, 20.0 + 3.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state.hint.as_deref() == Some("Synth")
        })
        .expect("a channel's name")
        + vec2(6.0, 4.0);
    let at = |h: &Harness, name: &str| {
        h.session
            .project()
            .channels
            .iter()
            .position(|c| c.name == name)
    };
    let first = h.session.project().channels[at(&h, "Pad").unwrap() - 1]
        .name
        .clone();

    h.modifiers = Modifiers::COMMAND;
    h.press(synth, PointerButton::Primary);
    h.release(synth, PointerButton::Primary);
    h.modifiers = Modifiers::NONE;
    h.settle();
    let previews: Vec<bool> = h
        .other
        .iter()
        .filter_map(|a| match a {
            Action::Preview { on, .. } => Some(*on),
            _ => None,
        })
        .collect();
    assert_eq!(previews, [true, false], "{:?}", h.other);
    assert!(
        !h.state.windows.channel_settings,
        "Ctrl+click only previews"
    );

    // Shift+wheel down over the first synth's name moves it below Pad.
    let before = at(&h, &first).unwrap();
    h.modifiers = Modifiers::SHIFT;
    h.frame(vec![
        Event::PointerMoved(synth),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: vec2(0.0, -1.0),
            modifiers: Modifiers::SHIFT,
            phase: egui::TouchPhase::Move,
        },
    ]);
    h.modifiers = Modifiers::NONE;
    assert_eq!(at(&h, &first), Some(before + 1), "moved down a row");
}

#[test]
fn the_tap_tempo_pad_sets_the_tempo_in_one_undo_step() {
    let mut h = Harness::new(&[]);
    h.state.windows.tap_tempo = true;
    h.settle();
    let window = window_rect(&h, "tap-tempo");
    let pad = (0..60)
        .map(|j| window.left_top() + vec2(window.width() / 2.0, 4.0 + 3.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Tap the beat here"))
        })
        .expect("the tap pad");
    let before = h.session.project().tempo_bpm();
    for _ in 0..3 {
        h.press(pad, PointerButton::Primary);
        h.release(pad, PointerButton::Primary);
        h.time += 0.5 - 2.0 / 60.0;
    }
    assert!((h.session.project().tempo_bpm() - 120.0).abs() < 0.5);
    h.session.run_line("edit.undo").unwrap();
    assert_eq!(h.session.project().tempo_bpm(), before);
}

#[test]
fn the_edit_menu_copy_and_paste_reach_the_focused_editor() {
    let mut h = Harness::new(&["pattern.add", "playlist.add_clip 1 1 1"]);
    h.state.windows.playlist = true;
    h.settle();
    let bar = TICKS_PER_BAR;
    h.click(h.playlist_point(bar + 40, 0), PointerButton::Primary);
    crate::request_clipboard(&h.ctx, crate::ClipboardKey::Copy);
    h.frame(vec![]);
    h.engine.position_ticks = f64::from(4 * bar + 100);
    crate::request_clipboard(&h.ctx, crate::ClipboardKey::Paste);
    let pasted = h.frame(vec![]);
    assert_eq!(pasted.len(), 1, "{pasted:?}");
    assert_eq!(h.session.project().playlist.clips.len(), 2);
    // A request no editor takes lapses at the end of the frame.
    h.state.windows.playlist = false;
    h.settle();
    crate::request_clipboard(&h.ctx, crate::ClipboardKey::Paste);
    h.frame(vec![]);
    h.state.windows.playlist = true;
    h.settle();
    assert_eq!(h.session.project().playlist.clips.len(), 2);
}

#[test]
fn dragging_a_clips_left_edge_trims_its_start() {
    let mut h = Harness::new(&["pattern.add", "playlist.add_clip 1 1 1 2"]);
    h.state.windows.playlist = true;
    h.settle();
    let bar = TICKS_PER_BAR;
    let edge = h.playlist_point(bar, 0) + vec2(2.0, 0.0);
    // The default snap is a bar.
    let out = h.drag(edge, h.playlist_point(2 * bar - 20, 0));
    assert_eq!(out, [format!("playlist.trim_clip 1 {bar}t {}t", 2 * bar)]);
    let c = h.session.project().playlist.clips[0];
    assert_eq!((c.start, c.length, c.offset), (2 * bar, bar, bar));
    assert_eq!(h.state.playlist.selection, BTreeSet::from([(0, 2 * bar)]));
    // Back out to the source's start, and no further.
    let edge = h.playlist_point(2 * bar, 0) + vec2(2.0, 0.0);
    h.drag(edge, h.playlist_point(0, 0) + vec2(4.0, 0.0));
    let c = h.session.project().playlist.clips[0];
    assert_eq!((c.start, c.length, c.offset), (bar, 2 * bar, 0));
}

#[test]
fn the_wheel_over_the_snap_button_steps_it_and_right_click_resets_it() {
    let mut h = Harness::new(&[]);
    h.state.windows.playlist = true;
    h.settle();
    let window = window_rect(&h, "playlist-window");
    let button = (0..200)
        .map(|i| window.left_top() + vec2(4.0 + 3.0 * i as f32, 11.0))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Snap: "))
        })
        .expect("the snap button");
    let snap = |h: &mut Harness| {
        h.frame(vec![]);
        h.frame(vec![]);
        h.state.hint.clone().unwrap_or_default()
    };
    assert!(snap(&mut h).starts_with("Snap: Bar "));
    h.frame(vec![Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, -1.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    }]);
    assert!(snap(&mut h).starts_with("Snap: Beat "), "down is finer");
    h.click(button, PointerButton::Secondary);
    assert!(snap(&mut h).starts_with("Snap: Bar "));
}

#[test]
fn the_clip_lists_plus_button_adds_a_pattern() {
    let mut h = Harness::new(&[]);
    h.state.windows.playlist = true;
    h.settle();
    let window = window_rect(&h, "playlist-window");
    let plus = (0..120)
        .flat_map(|j| (0..40).map(move |i| (i, j)))
        .map(|(i, j)| window.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 4.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("New pattern (F4"))
        })
        .expect("the + button");
    let before = h.session.project().patterns.len();
    assert_eq!(h.click(plus, PointerButton::Primary), ["pattern.add"]);
    assert_eq!(h.session.project().patterns.len(), before + 1);
}

#[test]
fn clicking_a_clips_glyph_selects_it_and_opens_the_clip_menu() {
    let mut h = Harness::new(&[
        "pattern.add",
        "playlist.add_clip 1 1 1",
        "playlist.add_clip 1 1 3",
    ]);
    h.state.windows.playlist = true;
    h.settle();
    let bar = TICKS_PER_BAR;
    let menu = egui::Id::new("playlist-clip-glyph-menu");
    let (grid, _) = h.state.playlist.last_view.unwrap();
    let glyph = pos2(grid.x((3 * bar) as f32) + 5.0, grid.y(0) + 5.0);
    let out = h.click(glyph, PointerButton::Primary);
    assert!(out.is_empty(), "{out:?}");
    h.settle();
    assert_eq!(h.state.playlist.selection, BTreeSet::from([(0, 3 * bar)]));
    assert!(egui::Popup::is_id_open(&h.ctx, menu));
    // The first item, Slice at playhead, cuts the clicked clip.
    let at = 3 * bar + bar / 2;
    h.engine.position_ticks = f64::from(at);
    h.frame(vec![]);
    let area = h.ctx.memory(|m| m.area_rect(menu)).expect("the menu");
    let out = h.click(area.left_top() + vec2(30.0, 12.0), PointerButton::Primary);
    assert_eq!(out, [format!("playlist.slice_clip 1 {at}t")]);
    h.settle();
    assert!(!egui::Popup::is_id_open(&h.ctx, menu));
    // The rest of the clip still works as before: a click on its body with
    // the Draw tool does not open the menu.
    h.click(h.playlist_point(bar + bar / 2, 0), PointerButton::Primary);
    h.settle();
    assert!(!egui::Popup::is_id_open(&h.ctx, menu));
}

#[test]
fn dragging_a_marker_label_moves_it_and_double_click_renames_it() {
    let mut h = Harness::new(&["playlist.add_marker 2 Drop"]);
    h.state.windows.playlist = true;
    h.settle();
    let (grid, clip) = h.state.playlist.last_view.unwrap();
    let on_ruler = |tick: u32| pos2(grid.x(tick as f32), clip.top() + 10.0);
    let bar = TICKS_PER_BAR;
    let out = h.drag(
        on_ruler(2 * bar) + vec2(12.0, 0.0),
        on_ruler(4 * bar) + vec2(4.0, 0.0),
    );
    assert_eq!(
        out,
        [format!("playlist.move_marker {}t {}t", 2 * bar, 4 * bar)]
    );
    assert_eq!(h.session.project().playlist.markers[0].tick, 4 * bar);
    h.frame(vec![]);
    let label = on_ruler(4 * bar) + vec2(12.0, 0.0);
    h.click(label, PointerButton::Primary);
    h.click(label, PointerButton::Primary);
    h.settle();
    assert!(h.state.renaming.is_some(), "a double-click opens the field");
    h.frame(vec![Event::Text("Build".to_owned())]);
    let out = h.frame(vec![Event::Key {
        key: egui::Key::Enter,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(out, [format!("playlist.add_marker {}t Build", 4 * bar)]);
}

#[test]
fn alt_wheel_over_a_note_steps_its_velocity_in_one_undo_step() {
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 4 C5 64"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let note = h.roll_point(24, 60);
    let origin = |h: &Harness| {
        let (grid, _) = h.state.roll.last_view.unwrap();
        (grid.x(0.0), grid.y(0))
    };
    let before = origin(&h);
    let wheel = |up: f32| Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: vec2(0.0, up),
        modifiers: Modifiers::ALT,
        phase: egui::TouchPhase::Move,
    };
    h.modifiers = Modifiers::ALT;
    h.frame(vec![Event::PointerMoved(note)]);
    h.frame(vec![wheel(1.0)]);
    h.frame(vec![wheel(1.0)]);
    // egui spreads a wheel notch over a few frames; Alt is still held.
    for _ in 0..40 {
        h.frame(vec![]);
    }
    h.modifiers = Modifiers::NONE;
    h.settle();
    let velocity = h.session.project().current_pattern().notes[0].velocity;
    assert!(velocity > 85, "two notches up from 64 reach {velocity}");
    assert_eq!(origin(&h), before, "the view stays put");
    assert_eq!(h.session.undo_depth(), 3, "two setup commands plus one run");
    h.session.run_line("edit.undo").unwrap();
    assert_eq!(h.session.project().current_pattern().notes[0].velocity, 64);
}

#[test]
fn double_clicking_a_note_opens_its_properties_and_reset_restores_them() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "note.add 1 0 4 C5 40",
        "note.set_pan 1 0 C5 -50",
        "note.set_prop 1 0 C5 porta on",
    ]);
    h.state.windows.piano_roll = true;
    h.settle();
    let note = h.roll_point(24, 60);
    h.click(note, PointerButton::Primary);
    h.click(note, PointerButton::Primary);
    h.settle();
    assert_eq!(h.state.roll.props_note, Some((0, 60)));
    let window = window_rect(&h, "note-properties");
    let reset = (0..60)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| window.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 4.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state
                .hint
                .as_deref()
                .is_some_and(|t| t.starts_with("Every property back"))
        })
        .expect("the Reset button");
    h.click(reset, PointerButton::Primary);
    let n = h.session.project().current_pattern().notes[0];
    assert_eq!((n.velocity, n.pan, n.props.porta), (100, 0, false));
    h.session.run_line("edit.undo").unwrap();
    let n = h.session.project().current_pattern().notes[0];
    assert_eq!(
        (n.velocity, n.pan, n.props.porta),
        (40, -50, true),
        "one undo step"
    );
    // Deleting the note closes the window.
    h.session.run_line("note.remove 1 0 C5").unwrap();
    h.settle();
    assert_eq!(h.state.roll.props_note, None);
}

#[test]
fn dragging_the_browser_edge_past_its_minimum_stops_at_a_width_its_contents_fit() {
    let mut h = Harness::new(&[]);
    h.state.windows.browser = true;
    h.settle();
    let edge = h.state.workspace.min.x;
    let y = 400.0;
    h.press(pos2(edge, y), PointerButton::Primary);
    for x in [120.0, 80.0, 40.0, 5.0] {
        h.frame(vec![Event::PointerMoved(pos2(x, y))]);
    }
    // Held past the minimum: the workspace starts where the browser was
    // clipped, so no unpainted strip opens between them.
    let held = h.state.workspace.min.x;
    assert!(
        (held - crate::browser::MIN_WIDTH).abs() < 1.0,
        "workspace starts at {held}, browser minimum is {}",
        crate::browser::MIN_WIDTH
    );
    h.release(pos2(5.0, y), PointerButton::Primary);
    h.settle();
    assert!((h.state.workspace.min.x - held).abs() < 1.0);

    // It still widens.
    let edge = h.state.workspace.min.x;
    h.drag(pos2(edge, y), pos2(300.0, y));
    h.settle();
    assert!((h.state.workspace.min.x - 300.0).abs() < 1.0);
}

/// The rects of the open popups (menus and submenus).
fn popup_rects(h: &Harness) -> Vec<Rect> {
    h.ctx.memory(|m| {
        m.layer_ids()
            .filter(|l| l.order == egui::Order::Foreground)
            .filter_map(|l| m.area_rect(l.id))
            .collect()
    })
}

#[test]
fn the_channel_menu_fits_its_contents() {
    // Issue #20: the channel menu grew to the height of the screen and
    // covered the whole app, from the channel settings window menu and from
    // a right-click on the channel in the rack.
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.channel_settings = true;
    h.settle();
    let window = window_rect(&h, "channel-settings");
    let button = pos2(window.left() + 7.0 + 9.0, window.top() + 16.0);
    h.click(button, PointerButton::Primary);
    h.settle();
    let menus = popup_rects(&h);
    assert_eq!(menus.len(), 1, "one menu, no submenu: {menus:?}");
    let menu = menus[0];
    assert!(menu.height() < 600.0, "menu too tall: {menu:?}");
    assert!(
        menu.top() > button.y,
        "menu opens below its button: {menu:?}"
    );
    h.settle();
    assert_eq!(popup_rects(&h), [menu], "the menu keeps its size");
    h.click(button, PointerButton::Primary);
    h.settle();
    assert!(!egui::Popup::is_any_open(&h.ctx));

    h.state.windows.channel_settings = false;
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    let name = (0..40)
        .flat_map(|j| (0..60).map(move |i| (i, j)))
        .map(|(i, j)| rack.left_top() + vec2(4.0 + 4.0 * i as f32, 20.0 + 3.0 * j as f32))
        .find(|p| {
            h.frame(vec![Event::PointerMoved(*p)]);
            h.frame(vec![]);
            h.state.hint.as_deref() == Some("Synth")
        })
        .expect("the channel's name");
    h.click(name, PointerButton::Secondary);
    h.settle();
    let menus = popup_rects(&h);
    assert!(!menus.is_empty(), "the rack opens the channel menu");
    assert!(
        menus.iter().all(|m| m.height() < 600.0),
        "menu too tall: {menus:?}"
    );
}

#[test]
fn the_mixer_keeps_its_height() {
    // The strips left one point too few for their scroll bar, so the mixer
    // grew a point every frame down to the bottom of the screen.
    let mut h = Harness::new(&[]);
    h.state.windows.mixer = true;
    for (panel, left, details) in [
        (false, false, false),
        (true, false, false),
        (true, true, true),
    ] {
        h.state.mixer_panel = panel;
        h.state.mixer_panel_left = left;
        h.state.mixer_details = details;
        for _ in 0..10 {
            h.frame(Vec::new());
        }
        let settled = window_rect(&h, "mixer-window");
        for _ in 0..30 {
            h.frame(Vec::new());
        }
        assert_eq!(
            window_rect(&h, "mixer-window"),
            settled,
            "panel {panel}, left {left}, details {details}"
        );
    }
}

#[test]
fn the_piano_roll_scroll_bar_scrolls_a_short_pattern_in_a_wide_window() {
    // A one-bar pattern in a maximized piano roll used to fit the view, so
    // the bar had nothing to scroll.
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 1 60"]);
    h.state.windows.piano_roll = true;
    h.settle();
    crate::chrome::maximize(&h.ctx, egui::Id::new("piano-roll"));
    h.settle();
    let (grid, clip) = h.state.roll.last_view.expect("piano roll drawn");
    let bar = crate::scrollbars::BAR;
    let y = clip.top() - bar / 2.0;
    let left = |h: &Harness| h.state.roll.last_view.unwrap().0.origin.x;
    let start = left(&h);

    // The right chevron scrolls one bar.
    h.click(pos2(clip.right() - bar / 2.0, y), PointerButton::Primary);
    h.settle();
    let bar_px = grid.px_per_tick * TICKS_PER_BAR as f32;
    assert!(
        (start - left(&h) - bar_px).abs() < 0.5,
        "{} by {bar_px}",
        start - left(&h)
    );

    // Dragging the thumb's body scrolls on, with the thumb under the pointer.
    let hint_at = |h: &mut Harness, x: f32| {
        h.frame(vec![Event::PointerMoved(pos2(x, y))]);
        h.frame(vec![]);
        h.cursor
    };
    let grips: Vec<f32> = (0..600)
        .map(|i| clip.left() + 52.0 + bar + i as f32)
        .filter(|x| hint_at(&mut h, *x) == egui::CursorIcon::ResizeHorizontal)
        .collect();
    let from = (grips[0] + grips[grips.len() - 1]) / 2.0;
    let before = left(&h);
    h.frame(vec![Event::PointerMoved(pos2(from, y))]);
    h.drag(pos2(from, y), pos2(from + 40.0, y));
    h.settle();
    assert!(left(&h) < before - 40.0, "{} < {before}", left(&h));
    let moved: Vec<f32> = (0..600)
        .map(|i| clip.left() + 52.0 + bar + i as f32)
        .filter(|x| hint_at(&mut h, *x) == egui::CursorIcon::ResizeHorizontal)
        .collect();
    assert!(
        (moved[0] - grips[0] - 40.0).abs() < 1.5,
        "{moved:?} {grips:?}"
    );
}

/// The layer drawn topmost at `pos`, by window id.
fn window_at(h: &Harness, pos: Pos2) -> Option<egui::Id> {
    h.ctx.layer_id_at(pos).map(|layer| layer.id)
}

#[test]
fn the_playlist_starts_maximized_behind_the_other_windows() {
    let mut h = Harness::new(&[]);
    h.state.windows = crate::Windows::default();
    h.state.arrange = Some(crate::Arrange::Backdrop);
    h.settle();
    let playlist = egui::Id::new("playlist-window");
    assert!(crate::chrome::maximized(&h.ctx, playlist));
    let area = h.state.workspace;
    let rect = window_rect(&h, "playlist-window");
    assert!(
        (rect.width() - area.width()).abs() < 1.0 && (rect.height() - area.height()).abs() < 1.0,
        "{rect:?} fills {area:?}"
    );
    // The floating windows stay in front of it, even after it is clicked.
    let rack = window_rect(&h, "channel-rack");
    assert_eq!(
        window_at(&h, rack.center()),
        Some(egui::Id::new("channel-rack"))
    );
    let open = pos2(area.left() + 40.0, area.bottom() - 20.0);
    assert_eq!(window_at(&h, open), Some(playlist));
    h.click(open, PointerButton::Primary);
    h.settle();
    assert_eq!(
        window_at(&h, rack.center()),
        Some(egui::Id::new("channel-rack"))
    );
    // Clicking it gives it the keyboard; clicking the rack takes it back.
    assert_eq!(h.state.focused_window, Some(playlist));
    h.click(rack.center(), PointerButton::Primary);
    h.settle();
    assert_eq!(h.state.focused_window, Some(egui::Id::new("channel-rack")));
}

#[test]
fn the_maximize_button_fills_the_workspace_and_restore_puts_the_playlist_back() {
    let mut h = Harness::new(&[]);
    h.state.windows.playlist = true;
    h.settle();
    let before = window_rect(&h, "playlist-window");
    h.click(
        header_button(&h, "playlist-window", 1.0),
        PointerButton::Primary,
    );
    h.settle();
    let area = h.state.workspace;
    let rect = window_rect(&h, "playlist-window");
    assert!(
        (rect.width() - area.width()).abs() < 1.0 && (rect.height() - area.height()).abs() < 1.0,
        "{rect:?} fills {area:?}"
    );
    h.click(
        header_button(&h, "playlist-window", 1.0),
        PointerButton::Primary,
    );
    h.settle();
    let after = window_rect(&h, "playlist-window");
    assert!(
        (after.min - before.min).length() < 1.0 && (after.size() - before.size()).length() < 1.0,
        "{after:?} back at {before:?}"
    );
}

#[test]
fn double_clicking_a_pattern_in_the_clip_list_opens_the_piano_roll() {
    let mut h = Harness::new(&["channel.add_synth", "pattern.add", "pattern.select 1"]);
    h.state.windows.playlist = true;
    h.settle();
    let items = h.state.playlist.picker_items.clone();
    h.click(items[1].center(), PointerButton::Primary);
    let commands = h.click(items[1].center(), PointerButton::Primary);
    h.settle();
    assert_eq!(commands, ["pattern.select 2"]);
    assert!(h.state.windows.piano_roll);
    assert!(h.state.renaming.is_none(), "no rename in place");
    assert_eq!(h.state.focused_window, Some(egui::Id::new("piano-roll")));
}

#[test]
fn double_clicking_a_pattern_clip_brings_the_piano_roll_to_the_front() {
    let mut h = Harness::new(&["channel.add_synth", "playlist.add_clip 1 1 0"]);
    h.state.windows.playlist = true;
    h.state.windows.piano_roll = true;
    h.settle();
    let at = h.playlist_point(musiccraft_project::params::PPQ, 0);
    h.click(at, PointerButton::Primary);
    h.click(at, PointerButton::Primary);
    h.settle();
    assert_eq!(h.state.focused_window, Some(egui::Id::new("piano-roll")));
}

#[test]
fn a_pattern_dragged_from_the_clip_list_lands_as_a_clip() {
    let mut h = Harness::new(&["channel.add_synth", "pattern.add", "pattern.select 1"]);
    h.state.windows.playlist = true;
    h.settle();
    let items = h.state.playlist.picker_items.clone();
    let to = h.playlist_point(TICKS_PER_BAR * 2 + 10, 2);
    let commands = h.drag(items[1].center(), to);
    assert_eq!(
        commands,
        [format!("playlist.add_clip 2 3 {}t", TICKS_PER_BAR * 2)]
    );
    let clip = h.session.project().playlist.clips[0];
    assert_eq!((clip.track, clip.start), (2, TICKS_PER_BAR * 2));
}

#[test]
fn a_paint_stroke_stays_on_the_track_it_started_on() {
    use crate::piano_roll::Tool;
    let mut h = Harness::new(&["channel.add_synth", "note.add 1 0 4 C4"]);
    h.state.windows.playlist = true;
    h.settle();
    let bar = TICKS_PER_BAR;
    h.state.playlist.tool = Tool::Paint;
    // The pointer drifts down two tracks along the way.
    h.drag(
        h.playlist_point(bar / 4, 1),
        h.playlist_point(4 * bar + bar / 4, 3),
    );
    let clips = &h.session.project().playlist.clips;
    assert!(clips.len() >= 4, "painted {} clips", clips.len());
    assert!(clips.iter().all(|c| c.track == 1), "{clips:?}");
}

#[test]
fn the_saved_layout_keeps_the_playlist_maximized() {
    let mut h = Harness::new(&[]);
    h.state.windows = crate::Windows::default();
    h.state.arrange = Some(crate::Arrange::Backdrop);
    h.settle();
    let layout = crate::window_layout(&h.ctx, &h.state);
    assert_eq!(layout.maximized, Some(vec!["playlist".to_owned()]));
    // A layout saved before maximized windows were kept starts with the
    // playlist as the backdrop; one saved with it restored keeps it so.
    let mut old = layout.clone();
    old.maximized = None;
    crate::restore_window_layout(&h.ctx, &mut h.state, &old);
    assert_eq!(h.state.arrange, Some(crate::Arrange::Backdrop));
    let mut floating = layout;
    floating.maximized = Some(Vec::new());
    crate::restore_window_layout(&h.ctx, &mut h.state, &floating);
    assert_eq!(h.state.arrange, None);
}

/// The first point of `points` whose hint starts with `hint`.
fn point_with_hint(
    h: &mut Harness,
    points: impl IntoIterator<Item = Pos2>,
    hint: &str,
) -> Option<Pos2> {
    points.into_iter().find(|p| {
        h.frame(vec![Event::PointerMoved(*p)]);
        h.frame(vec![]);
        h.state.hint.as_deref().is_some_and(|t| t.starts_with(hint))
    })
}

/// The sampler's Change… button in the channel window.
fn change_sample_button(h: &mut Harness) -> Pos2 {
    let window = window_rect(h, "channel-settings");
    let points: Vec<Pos2> = (0..40)
        .flat_map(|j| (0..80).map(move |i| (i, j)))
        .map(|(i, j)| window.left_top() + vec2(6.0 * i as f32, 60.0 + 4.0 * j as f32))
        .filter(|p| window.contains(*p))
        .collect();
    point_with_hint(h, points, "Load another sample").expect("the Change… button")
}

#[test]
fn a_sample_slot_menu_opens_the_system_file_dialog() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.windows.channel_settings = true;
    h.settle();
    let change = change_sample_button(&mut h);
    h.click(change, PointerButton::Primary);
    h.settle();
    assert!(egui::Popup::is_any_open(&h.ctx), "the sample menu opened");
    let below: Vec<Pos2> = (0..20)
        .flat_map(|j| (-15..15).map(move |i| (i, j)))
        .map(|(i, j)| change + vec2(10.0 * i as f32, 4.0 * j as f32))
        .collect();
    let browse = point_with_hint(&mut h, below, "Choose a sound file").expect("Browse files…");
    assert!(h.click(browse, PointerButton::Primary).is_empty());
    let pick = h
        .state
        .take_file_pick()
        .expect("a file dialog is asked for");
    assert_eq!(pick.pick, crate::Pick::SetSample(0));
    assert_eq!(pick.mode(), crate::PickMode::File);
    // The dialog's answer becomes the command.
    let actions = h.state.picked(&pick, &["/s/snare.wav".into()]);
    assert!(
        matches!(&actions[..], [Action::Run(i)] if i.to_string() == "channel.set_sample 1 /s/snare.wav")
    );
}

#[test]
fn a_sound_dropped_on_a_sample_slot_fills_it() {
    let mut h = Harness::new(&["channel.add_sampler builtin:kick"]);
    h.state.windows.channel_settings = true;
    h.settle();
    // Over the row, a little below where the button's hint starts.
    let row = change_sample_button(&mut h) + vec2(0.0, 8.0);
    egui::DragAndDrop::set_payload(
        &h.ctx,
        crate::browser::Dragged::Sample("builtin:clap".to_owned()),
    );
    h.frame(vec![Event::PointerMoved(row)]);
    let out = h.frame(vec![Event::PointerButton {
        pos: row,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::NONE,
    }]);
    assert_eq!(out, ["channel.set_sample 1 builtin:clap"]);
}

#[test]
fn file_menu_items_ask_for_the_system_file_dialog() {
    let mut h = Harness::new(&[]);
    h.settle();
    for (name, pick) in [
        ("ui.open", crate::Pick::Open),
        ("ui.save_as", crate::Pick::SaveAs),
        ("ui.export", crate::Pick::Export),
        ("ui.export_midi", crate::Pick::ExportMidi),
        ("ui.export_stems", crate::Pick::ExportStems),
    ] {
        let frame_actions = vec![Action::Run(musiccraft_commands::Invocation::bare(name))];
        let project = h.session.project().clone();
        let (engine, history) = (EngineStatus::default(), History::default());
        let plugins = Plugins::default();
        let frame = Frame {
            project: &project,
            engine: &engine,
            history: &history,
            waveform: None,
            waveform_seconds: None,
            clip_waveforms: &[],
            clip_durations: &[],
            plugins: &plugins,
            settings: &Settings::default(),
            settings_info: &SettingsInfo::default(),
        };
        assert!(crate::resolve_ui_actions(&mut h.state, &frame, frame_actions).is_empty());
        assert_eq!(
            h.state.take_file_pick().map(|p| p.pick),
            Some(pick),
            "{name}"
        );
        assert!(h.state.dialog.is_none(), "no text prompt for {name}");
    }
}

#[test]
fn holding_a_piano_key_sounds_it_once_until_release() {
    let mut h = Harness::new(&["channel.add_synth"]);
    h.state.windows.piano_roll = true;
    h.settle();
    let (_, clip) = h.state.roll.last_view.expect("piano roll drawn");
    let key = pos2(clip.left() + 40.0, h.roll_point(0, 60).y);
    h.other.clear();
    h.press(key, PointerButton::Primary);
    for _ in 0..5 {
        h.frame(vec![]);
    }
    let held: Vec<(u8, bool)> = h
        .other
        .iter()
        .filter_map(|a| match a {
            Action::Preview { key, on, .. } => Some((*key, *on)),
            _ => None,
        })
        .collect();
    assert_eq!(held, [(60, true)], "held key retriggered: {held:?}");
    assert_eq!(h.state.roll.preview, Some((0, 60)));
    h.release(key, PointerButton::Primary);
    h.settle();
    let all: Vec<(u8, bool)> = h
        .other
        .iter()
        .filter_map(|a| match a {
            Action::Preview { key, on, .. } => Some((*key, *on)),
            _ => None,
        })
        .collect();
    assert_eq!(all, [(60, true), (60, false)], "{all:?}");
    assert!(h.state.roll.preview.is_none());
}

#[test]
fn piano_roll_notes_paste_into_other_patterns_and_channels() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "channel.add_synth",
        "note.add 1 2 1 60",
        "note.add 1 4 1 62",
        "pattern.add",
        "pattern.select 1",
    ]);
    h.state.windows.piano_roll = true;
    h.state.roll.key_height = 14.0;
    h.state.selected_channel = 0;
    h.settle();
    h.hover_roll(0, 60);
    h.key(egui::Key::A, Modifiers::COMMAND);
    h.frame(vec![Event::Copy]);
    let step = TICKS_PER_STEP;
    let notes_of = |h: &Harness, pattern: usize, channel: usize| -> Vec<(u32, u8)> {
        let project = h.session.project();
        let id = project.channels[channel].id;
        project.patterns[pattern]
            .notes_of(id)
            .map(|n| (n.start, n.key))
            .collect()
    };
    // Into pattern 2 at the pointer: the earliest note lands under it.
    h.session.run_line("pattern.select 2").unwrap();
    h.settle();
    h.hover_roll(8 * step, 60);
    let paste = Event::Paste("MusicCraft: notes".to_owned());
    assert_eq!(h.frame(vec![paste.clone()]).len(), 1);
    assert_eq!(notes_of(&h, 1, 0), [(8 * step, 60), (10 * step, 62)]);
    // Into another channel from the Edit menu (pointer off the grid): the
    // notes keep the times they were copied at.
    h.state.selected_channel = 1;
    h.settle();
    h.frame(vec![Event::PointerGone]);
    crate::request_clipboard(&h.ctx, crate::ClipboardKey::Paste);
    assert_eq!(h.frame(vec![]).len(), 1);
    assert_eq!(notes_of(&h, 1, 1), [(2 * step, 60), (4 * step, 62)]);
    assert_eq!(notes_of(&h, 0, 0).len(), 2, "the source is untouched");
}

#[test]
fn the_rack_and_the_piano_roll_share_one_note_clipboard() {
    let mut h = Harness::new(&[
        "channel.add_synth",
        "channel.add_synth",
        "channel.add_synth",
        "step.toggle 1 1",
        "step.toggle 1 5",
        "note.add 3 8 1 67",
    ]);
    let step = TICKS_PER_STEP;
    let notes_of = |h: &Harness, channel: usize| -> Vec<(u32, u8)> {
        let project = h.session.project();
        let id = project.channels[channel].id;
        project
            .current_pattern()
            .notes_of(id)
            .map(|n| (n.start, n.key))
            .collect()
    };
    let copied = notes_of(&h, 0);
    // Steps copied in the rack paste in the piano roll of another channel.
    h.state.windows.channel_rack = true;
    h.settle();
    let rack = window_rect(&h, "channel-rack");
    h.frame(vec![Event::PointerMoved(rack.center())]);
    h.state.selected_channel = 0;
    h.frame(vec![Event::Copy]);
    h.state.windows.channel_rack = false;
    h.state.windows.piano_roll = true;
    h.state.selected_channel = 1;
    h.settle();
    h.frame(vec![Event::PointerGone]);
    let paste = Event::Paste("MusicCraft: steps".to_owned());
    assert_eq!(h.frame(vec![paste.clone()]).len(), 1);
    assert_eq!(notes_of(&h, 1), copied);
    // Notes copied in the piano roll paste over a channel's steps in the rack.
    h.state.selected_channel = 2;
    h.settle();
    h.hover_roll(8 * step, 67);
    h.key(egui::Key::A, Modifiers::COMMAND);
    h.frame(vec![Event::Copy]);
    h.state.windows.piano_roll = false;
    h.state.windows.channel_rack = true;
    h.settle();
    h.frame(vec![Event::PointerMoved(rack.center())]);
    h.state.selected_channel = 0;
    assert_eq!(h.frame(vec![paste]).len(), 1, "one undo step");
    assert_eq!(notes_of(&h, 0), [(8 * step, 67)]);
}

#[test]
fn right_clicking_a_routing_arrow_sidechains_the_selected_insert() {
    let mut h = Harness::new(&["mixer.add_effect 1 compressor"]);
    h.state.windows.mixer = true;
    h.state.selected_track = 3;
    h.settle();
    let mixer = window_rect(&h, "mixer-window");
    let find = |h: &mut Harness, area: Rect, prefix: &str| -> Pos2 {
        let (columns, rows) = (
            (area.width() / 6.0) as usize,
            (area.height() / 4.0) as usize,
        );
        (0..columns)
            .flat_map(|i| (0..rows).map(move |j| (i, j)))
            .map(|(i, j)| area.left_top() + vec2(6.0 * i as f32, 4.0 * j as f32))
            .find(|p| {
                h.frame(vec![Event::PointerMoved(*p)]);
                h.frame(vec![]);
                h.state
                    .hint
                    .as_deref()
                    .is_some_and(|t| t.starts_with(prefix))
            })
            .unwrap_or_else(|| panic!("nothing hinted {prefix:?}"))
    };
    // Insert 1 is below the selected insert 3: no audio route, but a sidechain.
    let arrow = find(&mut h, mixer, "Right-click: Sidechain to this track");
    h.click(arrow, PointerButton::Secondary);
    h.settle();
    let around = Rect::from_min_size(arrow - vec2(10.0, 10.0), vec2(220.0, 80.0));
    let item = find(&mut h, around, "Its output reaches");
    let ran = h.click(item, PointerButton::Primary);
    assert_eq!(ran, ["mixer.sidechain_to 3 1 on"]);
    h.settle();
    assert_eq!(h.session.project().mixer[3].sidechain_to, vec![1]);
    assert_eq!(h.state.selected_track, 3, "the menu click selects no strip");
    // The arrow now says so, and the compressor can pick insert 3.
    h.frame(vec![Event::PointerMoved(arrow)]);
    h.frame(vec![]);
    assert!(
        h.state
            .hint
            .as_deref()
            .is_some_and(|t| t.starts_with("Sidechained here")),
        "{:?}",
        h.state.hint
    );
    h.session
        .run_line("mixer.set_effect_param 1 1 sidechain 3")
        .unwrap();
}
