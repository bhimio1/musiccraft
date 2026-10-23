//! Playlist clip editing: slice, make unique, merge, reverse, fades, and
//! inserting or deleting time.

use musiccraft_project::params::{TICKS_PER_STEP, inst};
use musiccraft_project::{
    AutomationId, ChannelId, Clip, ClipSource, MAX_PATTERN_STEPS, Note, Pattern, PatternId, Project,
};

use super::playlist::{clip_arg, overlaps, replace_clips, restore_markers};
use crate::args::Args;
use crate::command::cmd;
use crate::{BATCH, Command, CommandError, Invocation, Outcome};

pub(super) fn commands() -> Vec<Box<dyn Command>> {
    vec![
        cmd(
            "playlist.slice_clip",
            "playlist.slice_clip <track> <at>",
            "Cut the clip covering a position into two clips there.",
            slice,
        ),
        cmd(
            "playlist.make_unique",
            "playlist.make_unique <track> <at>",
            "Give a clip its own copy of its pattern, automation or audio channel.",
            make_unique,
        ),
        cmd(
            "playlist.merge_clips",
            "playlist.merge_clips <track> <at> [<track> <at> ...]",
            "Merge pattern clips into one new pattern and one clip.",
            merge,
        ),
        cmd(
            "playlist.reverse_clip",
            "playlist.reverse_clip <track> <at>",
            "Play an audio clip's sample backwards (or forwards again).",
            reverse,
        ),
        cmd(
            "playlist.set_clip_fade",
            "playlist.set_clip_fade <track> <at> <in|out> <seconds>",
            "Set an audio clip's fade-in or fade-out time.",
            set_fade,
        ),
        cmd(
            "playlist.insert_space",
            "playlist.insert_space <start> <length>",
            "Insert empty time: clips and markers after the start move later.",
            insert_space,
        ),
        cmd(
            "playlist.delete_space",
            "playlist.delete_space <start> <length>",
            "Delete a stretch of time: what is inside goes, what follows moves earlier.",
            delete_space,
        ),
    ]
}

/// One undo step that puts the clips back and runs `cleanup` afterwards.
fn restore(
    args: &Args<'_>,
    clips: &[Clip],
    cleanup: Option<Invocation>,
) -> Result<Invocation, CommandError> {
    let json = serde_json::to_string(clips).map_err(|e| args.invalid(e.to_string()))?;
    let set = Invocation::new("playlist.set_clips", [json]);
    Ok(match cleanup {
        None => set,
        Some(cleanup) => Invocation::new(BATCH, [set.to_string(), cleanup.to_string()]),
    })
}

/// The part of `clip` from `at` (song ticks) on, as its own clip.
fn tail(clip: &Clip, at: u32) -> Clip {
    Clip {
        start: at,
        length: clip.end() - at,
        offset: clip.offset + (at - clip.start),
        ..*clip
    }
}

fn slice(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = clip_arg(project, args, 0)?;
    let at = args.bars(1, &project.song_meter())?;
    let clip = project.playlist.clips[index];
    if at <= clip.start || at >= clip.end() {
        return Err(args.invalid("slice inside the clip, not at its edge"));
    }
    let mut clips = project.playlist.clips.clone();
    clips[index].length = at - clip.start;
    clips.push(tail(&clip, at));
    let mut outcome = replace_clips(project, args, clips)?;
    outcome.message = format!("sliced the clip on track {} at {at}t", clip.track + 1);
    Ok(outcome)
}

/// "Name #2", "Name #3", ...: the first that `taken` does not hold.
pub(super) fn copy_name(name: &str, taken: impl Fn(&str) -> bool) -> String {
    let base = name.split(" #").next().unwrap_or(name);
    (2..)
        .map(|n| format!("{base} #{n}"))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| format!("{base} copy"))
}

fn make_unique(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = clip_arg(project, args, 0)?;
    let old = project.playlist.clips.clone();
    let clip = old[index];
    let (source, cleanup, what) = match clip.source {
        ClipSource::Pattern(id) => {
            let i = project
                .pattern_position(id)
                .ok_or_else(|| args.invalid("the pattern is missing"))?;
            let mut copy = project.patterns[i].clone();
            copy.id = PatternId(project.alloc_id());
            copy.name = copy_name(&copy.name, |n| project.patterns.iter().any(|p| p.name == n));
            let name = copy.name.clone();
            project.patterns.push(copy.clone());
            let number = project.patterns.len();
            (
                ClipSource::Pattern(copy.id),
                Invocation::new("pattern.remove", [number.to_string()]),
                format!("pattern {number} ({name})"),
            )
        }
        ClipSource::Automation(id) => {
            let i = project
                .automation_position(id)
                .ok_or_else(|| args.invalid("the automation is missing"))?;
            let mut copy = project.automations[i].clone();
            copy.id = AutomationId(project.alloc_id());
            copy.name = copy_name(&copy.name, |n| {
                project.automations.iter().any(|a| a.name == n)
            });
            let name = copy.name.clone();
            project.automations.push(copy.clone());
            let number = project.automations.len();
            (
                ClipSource::Automation(copy.id),
                Invocation::new("automation.remove", [number.to_string()]),
                format!("automation {number} ({name})"),
            )
        }
        ClipSource::Audio(id) => {
            let i = project
                .channel_position(id)
                .ok_or_else(|| args.invalid("the channel is missing"))?;
            let mut copy = project.channels[i].clone();
            copy.id = ChannelId(project.alloc_id());
            copy.name = copy_name(&copy.name, |n| project.channels.iter().any(|c| c.name == n));
            let name = copy.name.clone();
            project.channels.push(copy.clone());
            let number = project.channels.len();
            (
                ClipSource::Audio(copy.id),
                Invocation::new("channel.remove", [number.to_string()]),
                format!("channel {number} ({name})"),
            )
        }
    };
    project.playlist.clips[index].source = source;
    Ok(Outcome::undoable(
        format!("the clip now plays {what}"),
        restore(args, &old, Some(cleanup))?,
    ))
}

/// The notes a pattern clip plays, at clip-relative ticks: the pattern
/// repeats for the clip's length, starting `offset` ticks in.
pub(super) fn clip_notes(pattern: &Pattern, clip: &Clip) -> Vec<Note> {
    let period = pattern.length_ticks().max(1);
    let mut notes = Vec::new();
    let mut repeat = clip.offset / period * period;
    while repeat < clip.offset + clip.length {
        for note in &pattern.notes {
            let Some(local) = (repeat + note.start).checked_sub(clip.offset) else {
                continue;
            };
            if local < clip.length {
                notes.push(Note {
                    start: local,
                    length: note.length.min(clip.length - local),
                    ..*note
                });
            }
        }
        repeat += period;
    }
    notes
}

fn merge(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    if args.len() < 2 || args.len() % 2 != 0 {
        return Err(args.invalid("give one or more <track> <at> pairs"));
    }
    let mut picked = Vec::new();
    for pair in 0..args.len() / 2 {
        let index = clip_arg(project, args, pair * 2)?;
        if !picked.contains(&index) {
            picked.push(index);
        }
    }
    let clips: Vec<Clip> = picked.iter().map(|&i| project.playlist.clips[i]).collect();
    let start = clips.iter().map(|c| c.start).min().unwrap_or(0);
    let end = clips.iter().map(Clip::end).max().unwrap_or(0);
    let mut notes = Vec::new();
    for clip in &clips {
        let ClipSource::Pattern(id) = clip.source else {
            return Err(args.invalid("only pattern clips can be merged"));
        };
        let i = project
            .pattern_position(id)
            .ok_or_else(|| args.invalid("the pattern is missing"))?;
        let pattern = &project.patterns[i];
        notes.extend(clip_notes(pattern, clip).into_iter().map(|n| Note {
            start: n.start + clip.start - start,
            ..n
        }));
    }
    let steps = (end - start).div_ceil(TICKS_PER_STEP);
    if steps > MAX_PATTERN_STEPS {
        return Err(args.invalid(format!(
            "the merged pattern would be longer than {MAX_PATTERN_STEPS} steps"
        )));
    }
    let old = project.playlist.clips.clone();
    let id = PatternId(project.alloc_id());
    let name = format!("Merged {}", project.patterns.len() + 1);
    let sig = project.song_meter().sig_at(start);
    let mut pattern = Pattern::with_signature(id, name, sig);
    pattern.length_steps = steps.max(1);
    for note in notes {
        pattern.put_note(note);
    }
    pattern.sort_notes();
    let track = clips.iter().map(|c| c.track).min().unwrap_or(0);
    let merged = Clip {
        source: ClipSource::Pattern(id),
        track,
        start,
        length: end - start,
        offset: 0,
        muted: false,
    };
    let mut kept: Vec<Clip> = old
        .iter()
        .enumerate()
        .filter(|(i, _)| !picked.contains(i))
        .map(|(_, c)| *c)
        .collect();
    kept.push(merged);
    project.patterns.push(pattern);
    let number = project.patterns.len();
    project.playlist.clips = kept;
    let others = project.playlist.clips.len() - 1;
    if overlaps(project, &merged, Some(others)) {
        project.patterns.pop();
        project.playlist.clips = old;
        return Err(args.invalid(format!(
            "the merged clip would overlap another clip on track {}",
            track + 1
        )));
    }
    project.playlist.sort_clips();
    Ok(Outcome::undoable(
        format!("merged {} clips into pattern {number}", clips.len()),
        restore(
            args,
            &old,
            Some(Invocation::new("pattern.remove", [number.to_string()])),
        )?,
    ))
}

/// The channel an audio clip plays (index into the rack).
fn audio_channel(project: &Project, args: &Args<'_>) -> Result<usize, CommandError> {
    let index = clip_arg(project, args, 0)?;
    match project.playlist.clips[index].source {
        ClipSource::Audio(id) => project
            .channel_position(id)
            .ok_or_else(|| args.invalid("the clip's channel is missing")),
        _ => Err(args.invalid("only audio clips can be reversed or faded")),
    }
}

/// Sets one parameter of a channel, undoable through `channel.set_param`.
fn set_channel_param(
    project: &mut Project,
    channel: usize,
    param: usize,
    name: &str,
    value: f32,
    message: String,
) -> Outcome {
    let params = &mut project.channels[channel].params.0;
    let old = params.get(param).copied().unwrap_or(0.0);
    if let Some(slot) = params.get_mut(param) {
        *slot = value;
    }
    Outcome::undoable(
        message,
        Invocation::new(
            "channel.set_param",
            [(channel + 1).to_string(), name.to_owned(), old.to_string()],
        ),
    )
}

fn reverse(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let channel = audio_channel(project, args)?;
    let on = project.channels[channel].params.get(inst::REVERSE) < 0.5;
    let message = format!(
        "audio of channel {} plays {}",
        channel + 1,
        if on { "backwards" } else { "forwards" }
    );
    let value = if on { 1.0 } else { 0.0 };
    Ok(set_channel_param(
        project,
        channel,
        inst::REVERSE,
        "reverse",
        value,
        message,
    ))
}

fn set_fade(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(4, 4)?;
    let channel = audio_channel(project, args)?;
    let (param, name) = match args.str(2)? {
        "in" => (inst::FADE_IN, "fade_in"),
        "out" => (inst::FADE_OUT, "fade_out"),
        other => return Err(args.invalid(format!("`{other}` is not in or out"))),
    };
    let seconds = musiccraft_project::check_range("fade", args.f32(3, "seconds")?, 0.0, 10.0)?;
    Ok(set_channel_param(
        project,
        channel,
        param,
        name,
        seconds,
        format!(
            "fade {} {seconds} s on channel {}",
            args.str(2)?,
            channel + 1
        ),
    ))
}

/// One undo step that restores clips and markers.
fn restore_all(
    project_clips: &[Clip],
    markers: &[musiccraft_project::Marker],
    args: &Args<'_>,
) -> Result<Invocation, CommandError> {
    let clips = restore(args, project_clips, None)?;
    let markers = restore_markers(args, markers)?;
    Ok(Invocation::new(
        BATCH,
        [clips.to_string(), markers.to_string()],
    ))
}

fn insert_space(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let meter = project.song_meter();
    let start = args.bars(0, &meter)?;
    let length = args.bar_length(1, start, &meter)?;
    let playlist = &mut project.playlist;
    let inverse = restore_all(&playlist.clips, &playlist.markers, args)?;
    let mut clips = Vec::new();
    for clip in &playlist.clips {
        if clip.start >= start {
            clips.push(Clip {
                start: clip.start + length,
                ..*clip
            });
        } else if clip.end() > start {
            // A clip across the start is cut there; its rest moves on.
            clips.push(Clip {
                length: start - clip.start,
                ..*clip
            });
            let mut rest = tail(clip, start);
            rest.start += length;
            clips.push(rest);
        } else {
            clips.push(*clip);
        }
    }
    playlist.clips = clips;
    playlist.sort_clips();
    for marker in &mut playlist.markers {
        if marker.tick >= start {
            marker.tick += length;
        }
    }
    Ok(Outcome::undoable(
        format!("inserted {length}t at {start}t"),
        inverse,
    ))
}

fn delete_space(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let meter = project.song_meter();
    let start = args.bars(0, &meter)?;
    let length = args.bar_length(1, start, &meter)?;
    let end = start + length;
    let playlist = &mut project.playlist;
    let inverse = restore_all(&playlist.clips, &playlist.markers, args)?;
    let mut clips = Vec::new();
    for clip in &playlist.clips {
        if clip.end() <= start {
            clips.push(*clip);
            continue;
        }
        if clip.start >= end {
            clips.push(Clip {
                start: clip.start - length,
                ..*clip
            });
            continue;
        }
        // The clip reaches into the deleted stretch: keep what lies outside.
        if clip.start < start {
            clips.push(Clip {
                length: start - clip.start,
                ..*clip
            });
        }
        if clip.end() > end {
            let mut rest = tail(clip, end);
            rest.start = start;
            clips.push(rest);
        }
    }
    playlist.clips = clips;
    playlist.sort_clips();
    playlist.markers.retain(|m| m.tick < start || m.tick >= end);
    for marker in &mut playlist.markers {
        if marker.tick >= end {
            marker.tick -= length;
        }
    }
    Ok(Outcome::undoable(
        format!("deleted {length}t at {start}t"),
        inverse,
    ))
}
