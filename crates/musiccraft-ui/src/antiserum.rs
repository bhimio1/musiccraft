//! An Antiserum channel's Sound tab: the shared editor
//! (`musiccraft_editors::antiserum`), with its edits turned into commands.

use std::fmt;

use musiccraft_commands::Invocation;
use musiccraft_editors::antiserum::{AntiserumEdit, AntiserumEditor};
use musiccraft_patch::Patch;
use musiccraft_project::ChannelId;

use crate::dialogs::{FilePick, Pick};
use crate::{Action, Frame, UiState};

/// The editor's view state and the channel it shows.
#[derive(Default)]
pub(crate) struct AntiserumUi {
    editor: AntiserumEditor,
    channel: Option<ChannelId>,
}

impl AntiserumUi {
    /// Shows no channel until one is picked again.
    pub(crate) fn forget_channel(&mut self) {
        self.channel = None;
    }
}

impl fmt::Debug for AntiserumUi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AntiserumUi")
            .field("page", &self.editor.page)
            .field("channel", &self.channel)
            .finish_non_exhaustive()
    }
}

impl Clone for AntiserumUi {
    /// Only the page carries over; an open wavetable edit stays behind.
    fn clone(&self) -> Self {
        let mut editor = AntiserumEditor::default();
        editor.page = self.editor.page;
        Self {
            editor,
            channel: self.channel,
        }
    }
}

/// The commands for `edits` on channel `number` (1-based).
fn invocations(number: usize, edit: &AntiserumEdit) -> Option<Invocation> {
    let n = number.to_string();
    Some(match edit {
        AntiserumEdit::Set(index, value) => {
            let name = musiccraft_antiserum::params().get(*index)?.name;
            Invocation::new(
                "channel.set_param",
                [n, name.to_owned(), format!("{value}")],
            )
        }
        AntiserumEdit::Table(osc, table) => Invocation::new(
            "antiserum.set_table",
            [n, (osc + 1).to_string(), table.clone()],
        ),
        AntiserumEdit::LfoShape(lfo, points) => {
            let text = if points.is_empty() {
                "-".to_owned()
            } else {
                points
                    .iter()
                    .map(|p| format!("{}:{}:{}", p[0], p[1], p[2]))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            Invocation::new("antiserum.set_lfo_shape", [n, (lfo + 1).to_string(), text])
        }
        AntiserumEdit::Step(step, (on, transpose, velocity, gate)) => Invocation::new(
            "antiserum.set_step",
            [
                n,
                (step + 1).to_string(),
                if *on { "on" } else { "off" }.to_owned(),
                transpose.to_string(),
                format!("{velocity}"),
                format!("{gate}"),
            ],
        ),
        AntiserumEdit::Preset(name) => Invocation::new("antiserum.load_preset", [n, name.clone()]),
        AntiserumEdit::SetZone(osc, zone, low, high, root) => Invocation::new(
            "antiserum.set_zone",
            [
                n,
                (osc + 1).to_string(),
                (zone + 1).to_string(),
                low.to_string(),
                high.to_string(),
                root.to_string(),
            ],
        ),
        AntiserumEdit::RemoveZone(osc, zone) => Invocation::new(
            "antiserum.remove_zone",
            [n, (osc + 1).to_string(), (zone + 1).to_string()],
        ),
        AntiserumEdit::Sample(osc, sample) => {
            // An empty sample clears the oscillator's.
            let sample = if sample.is_empty() { "-" } else { sample };
            Invocation::new(
                "antiserum.set_sample",
                [n, (osc + 1).to_string(), sample.to_owned()],
            )
        }
        AntiserumEdit::Zone(osc, sample) => Invocation::new(
            "antiserum.add_zone",
            [n, (osc + 1).to_string(), sample.clone()],
        ),
        AntiserumEdit::Begin
        | AntiserumEdit::Save
        | AntiserumEdit::Open
        | AntiserumEdit::ChooseSample(_)
        | AntiserumEdit::AddZone(_) => return None,
    })
}

/// Shows the editor for the Antiserum on channel `index` (0-based).
pub(crate) fn show(
    ui: &mut egui::Ui,
    state: &mut UiState,
    frame: &Frame<'_>,
    index: usize,
    patch: &Patch,
    actions: &mut Vec<Action>,
) {
    let number = index + 1;
    let id = frame.project.channels.get(index).map(|c| c.id);
    if state.antiserum.channel != id {
        // Another channel: start afresh (an open wavetable edit belongs to
        // the old one).
        let page = state.antiserum.editor.page;
        state.antiserum = AntiserumUi {
            editor: AntiserumEditor::default(),
            channel: id,
        };
        state.antiserum.editor.page = page;
    }
    // Sample previews find files as the engine does: from the project's
    // folder, with paths from other systems translated.
    state
        .antiserum
        .editor
        .set_folder(frame.project.path().and_then(std::path::Path::parent));
    // The sample slots offer the browser's folders and take sounds dragged
    // from it.
    let dragged = crate::sample_slot::dragged(ui.ctx());
    state
        .antiserum
        .editor
        .set_sounds(&state.browser.folders, dragged);
    let values = musiccraft_antiserum_presets::values(patch);
    let mut edits = Vec::new();
    let mut menu_actions = Vec::new();
    let mut menu = |response: &egui::Response, name: &str| {
        crate::automation::menu(
            response,
            &format!("channel:{number}:{name}"),
            &mut menu_actions,
        );
    };
    state
        .antiserum
        .editor
        .show(ui, &values, patch, &mut edits, &mut menu);
    actions.extend(menu_actions);
    for edit in &edits {
        match edit {
            AntiserumEdit::Begin => {
                state.begin_gesture();
            }
            AntiserumEdit::Save => {
                state.pick_file(FilePick::save_patch(index, &patch.name, frame.project));
            }
            AntiserumEdit::Open => state.pick_file(FilePick::open_patch(index, frame.project)),
            AntiserumEdit::ChooseSample(osc) => {
                let current = patch.source(*osc).sample;
                let pick = Pick::AntiserumSample(index, *osc);
                state.pick_file(FilePick::sample(pick, &current, frame.project));
            }
            AntiserumEdit::AddZone(osc) => {
                let pick = Pick::AntiserumZone(index, *osc);
                state.pick_file(FilePick::sample(pick, "", frame.project));
            }
            AntiserumEdit::Sample(_, _) | AntiserumEdit::Zone(_, _) => {
                // A drop from the browser: the drag is over.
                egui::DragAndDrop::clear_payload(ui.ctx());
                if let Some(invocation) = invocations(number, edit) {
                    actions.push(Action::Run(invocation));
                }
            }
            AntiserumEdit::Preset(_) | AntiserumEdit::RemoveZone(_, _) => {
                if let Some(invocation) = invocations(number, edit) {
                    actions.push(Action::Run(invocation));
                }
            }
            _ => {
                if let Some(invocation) = invocations(number, edit) {
                    actions.push(Action::Coalesced(invocation, state.gesture()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_become_the_matching_commands() {
        let cutoff = musiccraft_antiserum::index_of("filter1_cutoff").unwrap();
        let text = |edit: AntiserumEdit| invocations(3, &edit).map(|i| i.to_string());
        assert_eq!(
            text(AntiserumEdit::Set(cutoff, 900.0)).as_deref(),
            Some("channel.set_param 3 filter1_cutoff 900")
        );
        assert_eq!(
            text(AntiserumEdit::Table(0, "factory:pwm".to_owned())).as_deref(),
            Some("antiserum.set_table 3 1 factory:pwm")
        );
        assert_eq!(
            text(AntiserumEdit::LfoShape(
                1,
                vec![[0.0, 1.0, 0.0], [0.5, 0.0, -0.5]]
            ))
            .as_deref(),
            Some("antiserum.set_lfo_shape 3 2 0:1:0,0.5:0:-0.5")
        );
        assert_eq!(
            text(AntiserumEdit::LfoShape(1, Vec::new())).as_deref(),
            Some("antiserum.set_lfo_shape 3 2 -")
        );
        assert_eq!(
            text(AntiserumEdit::Step(4, (false, -3, 0.5, 0.25))).as_deref(),
            Some("antiserum.set_step 3 5 off -3 0.5 0.25")
        );
        assert_eq!(
            text(AntiserumEdit::SetZone(1, 0, 36, 59, 48)).as_deref(),
            Some("antiserum.set_zone 3 2 1 36 59 48")
        );
        assert_eq!(
            text(AntiserumEdit::RemoveZone(1, 0)).as_deref(),
            Some("antiserum.remove_zone 3 2 1")
        );
        assert_eq!(text(AntiserumEdit::Begin), None);
        assert_eq!(text(AntiserumEdit::ChooseSample(0)), None, "a dialog");
        // Every edit the editor makes is a command the registry accepts.
        let mut session = musiccraft_commands::Session::new(
            musiccraft_project::Project::default(),
            musiccraft_commands::Registry::with_builtin(),
        );
        for line in [
            "channel.add_synth",
            "channel.add_synth",
            "channel.add_antiserum",
        ] {
            session.run_line(line).unwrap();
        }
        session
            .run_line("antiserum.add_zone 3 2 builtin:kick")
            .unwrap();
        for edit in [
            AntiserumEdit::SetZone(1, 0, 36, 59, 48),
            AntiserumEdit::RemoveZone(1, 0),
            AntiserumEdit::Set(cutoff, 900.0),
            AntiserumEdit::Table(0, "factory:pwm".to_owned()),
            AntiserumEdit::LfoShape(1, vec![[0.0, 1.0, 0.0], [0.5, 0.0, -0.5]]),
            AntiserumEdit::Step(4, (false, -3, 0.5, 0.25)),
            AntiserumEdit::Preset("Glass Reese".to_owned()),
        ] {
            let invocation = invocations(3, &edit).unwrap();
            session
                .run_line(&invocation.to_string())
                .unwrap_or_else(|e| panic!("{invocation}: {e}"));
        }
    }
}
