//! Sample paths from another system made local: `project.localize_samples`
//! and the translation done when an FL Studio project is imported (see
//! `docs/spec/sample-paths.md`).

use std::path::Path;

use musiccraft_paths::Resolver;
use musiccraft_project::{ChannelId, Instrument, Project};
use serde::{Deserialize, Serialize};

use crate::args::Args;
use crate::command::cmd;
use crate::{Command, CommandError, Invocation, Outcome};

pub(super) fn commands() -> Vec<Box<dyn Command>> {
    vec![
        cmd(
            "project.localize_samples",
            "project.localize_samples",
            "Replace sample paths from another system (Windows, FL Studio) with the files found here.",
            localize_samples,
        ),
        cmd(
            "project.restore_sample_paths",
            "project.restore_sample_paths <json>",
            "Put sample paths back as they were (used by undo).",
            restore_sample_paths,
        ),
    ]
}

/// Every sample path an instrument plays, in a fixed order.
fn paths_mut(instrument: &mut Instrument) -> Vec<&mut String> {
    match instrument {
        Instrument::Sampler { sample }
        | Instrument::Granular { sample }
        | Instrument::Resynth { sample } => vec![sample],
        Instrument::Kit { pads } => pads.iter_mut().collect(),
        Instrument::Zones { zones } => zones.iter_mut().map(|z| &mut z.sample).collect(),
        _ => Vec::new(),
    }
}

/// What localizing changed and what it could not find.
#[derive(Debug, Default)]
pub(crate) struct Localized {
    /// Channels whose paths changed, with their previous paths.
    pub before: Vec<(ChannelId, Vec<String>)>,
    /// Paths replaced by a local file.
    pub found: usize,
    /// Foreign paths with no local file.
    pub missing: Vec<String>,
}

/// Replaces every foreign sample path that resolves from `base`.
pub(crate) fn localize(project: &mut Project, base: Option<&Path>) -> Localized {
    let mut resolver = Resolver::new();
    let mut out = Localized::default();
    for channel in &mut project.channels {
        let id = channel.id;
        let mut old = Vec::new();
        let mut changed = false;
        for path in paths_mut(&mut channel.instrument) {
            old.push(path.clone());
            if !musiccraft_paths::is_foreign(path) || cfg!(windows) {
                continue;
            }
            match resolver.resolve(path, base) {
                Some(found) => {
                    *path = found.to_string_lossy().into_owned();
                    out.found += 1;
                    changed = true;
                }
                None => {
                    if !out.missing.contains(path) {
                        out.missing.push(path.clone());
                    }
                }
            }
        }
        if changed {
            out.before.push((id, old));
        }
    }
    out
}

/// A short report: how many were found, and which were not.
pub(crate) fn summary(localized: &Localized) -> String {
    let mut parts = Vec::new();
    if localized.found > 0 {
        let count = localized.found;
        let noun = if count == 1 { "path" } else { "paths" };
        parts.push(format!(
            "{count} sample {noun} from another system found here"
        ));
    }
    if !localized.missing.is_empty() {
        parts.push(format!(
            "samples not found on this system (set folders in Settings → Folders): {}",
            localized.missing.join(", ")
        ));
    }
    parts.join("; ")
}

#[derive(Serialize, Deserialize)]
struct Restore(Vec<(ChannelId, Vec<String>)>);

fn localize_samples(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(0, 0)?;
    let base = project.path().and_then(Path::parent).map(Path::to_path_buf);
    let localized = localize(project, base.as_deref());
    let mut message = summary(&localized);
    if message.is_empty() {
        message = "no sample paths from another system".to_owned();
    }
    if localized.before.is_empty() {
        return Ok(Outcome::done(message));
    }
    let json = serde_json::to_string(&Restore(localized.before))
        .map_err(|e| args.invalid(e.to_string()))?;
    Ok(Outcome::undoable(
        message,
        Invocation::new("project.restore_sample_paths", [json]),
    ))
}

fn restore_sample_paths(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let Restore(rows) =
        serde_json::from_str(args.str(0)?).map_err(|e| args.invalid(e.to_string()))?;
    let mut inverse = Vec::new();
    for (id, paths) in rows {
        let index = project
            .channel_position(id)
            .ok_or_else(|| args.invalid(format!("no channel with id {}", id.0)))?;
        let slots = paths_mut(&mut project.channels[index].instrument);
        if slots.len() != paths.len() {
            return Err(args.invalid("the channel's samples changed"));
        }
        let mut now = Vec::new();
        for (slot, path) in slots.into_iter().zip(paths) {
            now.push(std::mem::replace(slot, path));
        }
        inverse.push((id, now));
    }
    let json = serde_json::to_string(&Restore(inverse)).map_err(|e| args.invalid(e.to_string()))?;
    Ok(Outcome::undoable(
        "restored sample paths",
        Invocation::new(args.name(), [json]),
    ))
}
