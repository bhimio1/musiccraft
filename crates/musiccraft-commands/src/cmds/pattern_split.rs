//! Splitting patterns: a clip into two new patterns, or a pattern into one
//! pattern per channel.

use std::collections::BTreeMap;

use musiccraft_project::params::TICKS_PER_STEP;
use musiccraft_project::{
    AutomationPoint, AutomationTarget, ChannelId, Clip, ClipSource, MAX_PATTERN_STEPS,
    PLAYLIST_TRACKS, Pattern, PatternId, PatternLane, Project,
};

use super::clip_edit::{clip_notes, copy_name};
use super::playlist::clip_arg;
use crate::args::Args;
use crate::command::cmd;
use crate::{BATCH, Command, CommandError, Invocation, Outcome};

pub(super) fn commands() -> Vec<Box<dyn Command>> {
    vec![
        cmd(
            "playlist.split_clip",
            "playlist.split_clip <track> <at>",
            "Cut a pattern clip in two, each piece a new pattern of what it plays.",
            split_clip,
        ),
        cmd(
            "pattern.split_by_channel",
            "pattern.split_by_channel <pattern>",
            "Make one pattern per channel and put their clips where the pattern's were.",
            split_by_channel,
        ),
    ]
}

/// One undo step: the old clips back, the new patterns (numbers ascending)
/// gone, the old pattern selected.
fn undo(
    args: &Args<'_>,
    clips: &[Clip],
    added: std::ops::Range<usize>,
    selected: usize,
) -> Result<Invocation, CommandError> {
    let json = serde_json::to_string(clips).map_err(|e| args.invalid(e.to_string()))?;
    let mut steps = vec![Invocation::new("playlist.set_clips", [json]).to_string()];
    steps.extend(added.rev().map(|i| format!("pattern.remove {}", i + 1)));
    steps.push(format!("pattern.select {}", selected + 1));
    Ok(Invocation::new(BATCH, steps))
}

fn pattern_of(project: &Project, args: &Args<'_>, clip: &Clip) -> Result<Pattern, CommandError> {
    let ClipSource::Pattern(id) = clip.source else {
        return Err(args.invalid(
            "only pattern clips split into new patterns; slice audio and automation clips",
        ));
    };
    project
        .pattern_position(id)
        .map(|i| project.patterns[i].clone())
        .ok_or_else(|| args.invalid("the pattern is missing"))
}

/// The lane's points `piece` plays, from the piece's start, with a point at
/// 0 holding the last value before it.
fn cut_lane(lane: &PatternLane, period: u32, piece: &Clip) -> Option<PatternLane> {
    let (from, to) = (piece.offset, piece.offset + piece.length);
    let mut points: Vec<AutomationPoint> = Vec::new();
    let mut held = None;
    let mut repeat = (from / period).saturating_sub(1) * period;
    while repeat < to {
        for point in &lane.points {
            let at = repeat + point.tick;
            if at < from {
                held = Some(*point);
            } else if at < to {
                points.push(AutomationPoint {
                    tick: at - from,
                    ..*point
                });
            }
        }
        repeat += period;
    }
    points.sort_by_key(|p| p.tick);
    points.dedup_by_key(|p| p.tick);
    if let Some(held) = held.filter(|_| points.first().is_none_or(|p| p.tick > 0)) {
        points.insert(0, AutomationPoint { tick: 0, ..held });
    }
    (!points.is_empty()).then(|| PatternLane {
        target: lane.target.clone(),
        points,
    })
}

/// A new pattern holding what `piece` plays of `source`, from offset 0.
fn baked(project: &mut Project, source: &Pattern, piece: &Clip) -> Pattern {
    let period = source.length_ticks().max(1);
    let mut pattern = source.clone();
    pattern.id = PatternId(project.alloc_id());
    pattern.name = copy_name(&source.name, |n| {
        project.patterns.iter().any(|p| p.name == n)
    });
    pattern.auto_length = false;
    pattern.length_steps = piece.length.div_ceil(TICKS_PER_STEP).max(1);
    pattern.notes.clear();
    for note in clip_notes(source, piece) {
        pattern.put_note(note);
    }
    pattern.automation = source
        .automation
        .iter()
        .filter_map(|lane| cut_lane(lane, period, piece))
        .collect();
    pattern
}

fn split_clip(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = clip_arg(project, args, 0)?;
    let at = args.bars(1, &project.song_meter())?;
    let clip = project.playlist.clips[index];
    let source = pattern_of(project, args, &clip)?;
    if at <= clip.start || at >= clip.end() {
        return Err(args.invalid("split inside the clip, not at its edge"));
    }
    let pieces = [
        Clip {
            length: at - clip.start,
            ..clip
        },
        Clip {
            start: at,
            length: clip.end() - at,
            offset: clip.offset + (at - clip.start),
            ..clip
        },
    ];
    if pieces
        .iter()
        .any(|p| p.length.div_ceil(TICKS_PER_STEP) > MAX_PATTERN_STEPS)
    {
        return Err(args.invalid(format!(
            "a piece would be longer than {MAX_PATTERN_STEPS} steps"
        )));
    }
    let old = project.playlist.clips.clone();
    let previous = project.current_pattern;
    let first = project.patterns.len();
    let mut clips = old.clone();
    for (i, piece) in pieces.iter().enumerate() {
        let pattern = baked(project, &source, piece);
        let new = Clip {
            source: ClipSource::Pattern(pattern.id),
            offset: 0,
            ..*piece
        };
        project.patterns.push(pattern);
        if i == 0 {
            clips[index] = new;
        } else {
            clips.push(new);
        }
    }
    project.playlist.clips = clips;
    project.playlist.sort_clips();
    Ok(Outcome::undoable(
        format!(
            "split the clip on track {} at {at}t into patterns {} and {}",
            clip.track + 1,
            first + 1,
            first + 2
        ),
        undo(args, &old, first..first + 2, previous)?,
    ))
}

/// The channel a lane drives, if it drives one.
fn lane_channel(target: &AutomationTarget) -> Option<ChannelId> {
    match target {
        AutomationTarget::ChannelVolume { channel }
        | AutomationTarget::ChannelPan { channel }
        | AutomationTarget::ChannelParam { channel, .. } => Some(*channel),
        _ => None,
    }
}

/// Where each piece's copies of the pattern's clips go: per source track,
/// the first piece on that track and each next one on the first track below
/// the previous one where its clips fit. `(piece, clip)` pairs.
fn place(
    args: &Args<'_>,
    kept: &[Clip],
    moved: &[Clip],
    pieces: usize,
) -> Result<Vec<(usize, Clip)>, CommandError> {
    let mut groups: BTreeMap<u32, Vec<Clip>> = BTreeMap::new();
    for clip in moved {
        groups.entry(clip.track).or_default().push(*clip);
    }
    let mut taken: Vec<Clip> = kept.to_vec();
    let mut placed = Vec::new();
    for (track, group) in groups {
        let mut next = track;
        for piece in 0..pieces {
            let fits = |t: u32| {
                group.iter().all(|c| {
                    !taken
                        .iter()
                        .any(|o| o.track == t && o.start < c.end() && c.start < o.end())
                })
            };
            let track = (next..PLAYLIST_TRACKS).find(|&t| fits(t)).ok_or_else(|| {
                args.invalid(format!(
                    "the split clips do not fit on the {PLAYLIST_TRACKS} playlist tracks"
                ))
            })?;
            for clip in &group {
                let clip = Clip { track, ..*clip };
                taken.push(clip);
                placed.push((piece, clip));
            }
            next = track + 1;
        }
    }
    Ok(placed)
}

fn split_by_channel(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let index = project.pattern_index(args.number(0, "pattern")?)?;
    let source = project.patterns[index].clone();
    let mut channels: Vec<(usize, ChannelId)> = source
        .notes
        .iter()
        .map(|n| {
            let position = project.channel_position(n.channel).unwrap_or(usize::MAX);
            (position, n.channel)
        })
        .collect();
    channels.sort_unstable();
    channels.dedup();
    if channels.len() < 2 {
        return Err(args.invalid(format!(
            "pattern {} plays fewer than two channels; there is nothing to split",
            index + 1
        )));
    }
    let old = project.playlist.clips.clone();
    let (moved, kept): (Vec<Clip>, Vec<Clip>) =
        old.iter().partition(|c| c.pattern() == Some(source.id));
    let placed = place(args, &kept, &moved, channels.len())?;
    let previous = project.current_pattern;
    let first = project.patterns.len();
    let ours: Vec<ChannelId> = channels.iter().map(|(_, id)| *id).collect();
    let mut ids = Vec::new();
    for (k, &(position, channel)) in channels.iter().enumerate() {
        let mut pattern = source.clone();
        pattern.id = PatternId(project.alloc_id());
        let name = project
            .channels
            .get(position)
            .map_or("Channel", |c| c.name.as_str());
        pattern.name = format!("{} - {name}", source.name);
        pattern.notes.retain(|n| n.channel == channel);
        pattern
            .automation
            .retain(|lane| match lane_channel(&lane.target) {
                Some(target) if ours.contains(&target) => target == channel,
                _ => k == 0,
            });
        ids.push(pattern.id);
        project.patterns.push(pattern);
    }
    let mut clips = kept;
    clips.extend(placed.into_iter().map(|(piece, clip)| Clip {
        source: ClipSource::Pattern(ids[piece]),
        ..clip
    }));
    project.playlist.clips = clips;
    project.playlist.sort_clips();
    project.current_pattern = first;
    let last = project.patterns.len();
    Ok(Outcome::undoable(
        format!(
            "split pattern {} by channel into patterns {}-{last}",
            index + 1,
            first + 1
        ),
        undo(args, &old, first..last, previous)?,
    ))
}
