//! Built-in commands, grouped by area.

mod antiserum;
mod automation;
mod channel;
mod clip_edit;
pub(crate) mod control;
mod controller;
mod meter;
mod mixer;
mod note_gen;
mod note_tools;
mod notes;
mod pattern;
mod pattern_split;
mod playlist;
mod plugin;
mod project;
mod rack;
mod record;
pub(crate) mod sample_paths;
mod transport;
mod zones;

#[cfg(test)]
mod tests;

use crate::Command;

/// All commands registered by default.
pub(crate) fn all() -> Vec<Box<dyn Command>> {
    let mut commands = Vec::new();
    commands.extend(transport::commands());
    commands.extend(project::commands());
    commands.extend(meter::commands());
    commands.extend(sample_paths::commands());
    commands.extend(channel::commands());
    commands.extend(antiserum::commands());
    commands.extend(notes::commands());
    commands.extend(note_tools::commands());
    commands.extend(note_gen::commands());
    commands.extend(pattern::commands());
    commands.extend(mixer::commands());
    commands.extend(rack::commands());
    commands.extend(playlist::commands());
    commands.extend(clip_edit::commands());
    commands.extend(pattern_split::commands());
    commands.extend(automation::commands());
    commands.extend(control::commands());
    commands.extend(controller::commands());
    commands.extend(plugin::commands());
    commands.extend(record::commands());
    commands.extend(zones::commands());
    commands
}
