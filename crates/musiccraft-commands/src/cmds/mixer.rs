use musiccraft_project::params::{
    EFFECT_KINDS, EffectKind, MAX_EFFECT_SLOTS, MAX_MIXER_TRACKS, index_of,
};
use musiccraft_project::{
    ChannelId, Dock, EffectSlot, MAX_SENDS, MixerSend, MixerTrack, Project, ProjectError,
    check_pan, check_volume,
};
use serde::{Deserialize, Serialize};

use crate::args::{Args, num, on_off};
use crate::command::cmd;
use crate::{BATCH, Command, CommandError, Invocation, Outcome};

pub(super) fn commands() -> Vec<Box<dyn Command>> {
    vec![
        cmd(
            "mixer.add_track",
            "mixer.add_track [name]",
            "Add an insert track at the end of the mixer.",
            add_track,
        ),
        cmd(
            "mixer.remove_track",
            "mixer.remove_track <track>",
            "Remove an insert track; its channels go to the master.",
            remove_track,
        ),
        cmd(
            "mixer.insert_track",
            "mixer.insert_track <track> <json>",
            "Insert a mixer track from its JSON form (used by undo).",
            insert_track,
        ),
        cmd(
            "mixer.reset_track",
            "mixer.reset_track <track>",
            "Return a mixer track to its defaults: default name and colour, no effects, sends or EQ, unity level, centred, routed to the master.",
            reset_track,
        ),
        cmd(
            "mixer.set_track",
            "mixer.set_track <track> <json>",
            "Replace a mixer track's settings with a saved copy (the undo of mixer.reset_track).",
            set_track,
        ),
        cmd(
            "mixer.move_track",
            "mixer.move_track <track> <left|right>",
            "Swap an insert with its left or right neighbour; routing, sends and automation follow.",
            move_track,
        ),
        cmd(
            "mixer.rename",
            "mixer.rename <track> <name>",
            "Rename a mixer track.",
            rename,
        ),
        cmd(
            "mixer.set_volume",
            "mixer.set_volume <track> <0-1.25>",
            "Set a mixer track's volume (linear gain).",
            set_volume,
        ),
        cmd(
            "mixer.set_eq",
            "mixer.set_eq <track> <low|mid|high> <gain dB> [freq Hz]",
            "Set a band of a mixer track's built-in EQ (gain -18..18 dB; 0 is off).",
            set_eq,
        ),
        cmd(
            "mixer.set_effect_mix",
            "mixer.set_effect_mix <track> <slot> <0-1>",
            "Set how much of an effect slot's output is heard (dry/wet mix).",
            set_effect_mix,
        ),
        cmd(
            "mixer.set_separation",
            "mixer.set_separation <track> <-1..1>",
            "Set a mixer track's stereo separation: -1 mono, 0 as is, 1 wider.",
            set_separation,
        ),
        cmd(
            "mixer.set_phase_invert",
            "mixer.set_phase_invert <track> <on|off>",
            "Flip the polarity of a mixer track.",
            set_phase_invert,
        ),
        cmd(
            "mixer.set_track_color",
            "mixer.set_track_color <track> <1-16|none>",
            "Colour a mixer track (none: its automatic colour).",
            set_track_color,
        ),
        cmd(
            "mixer.set_dock",
            "mixer.set_dock <track> <left|middle|right>",
            "Pin an insert's strip to the mixer's fixed left or right dock, or return it to the scrolling middle.",
            set_dock,
        ),
        cmd(
            "mixer.set_swap_sides",
            "mixer.set_swap_sides <track> <on|off>",
            "Exchange a mixer track's left and right sides.",
            set_swap_sides,
        ),
        cmd(
            "mixer.set_pan",
            "mixer.set_pan <track> <-1..1>",
            "Set a mixer track's pan.",
            set_pan,
        ),
        cmd(
            "mixer.set_mute",
            "mixer.set_mute <track> <on|off>",
            "Mute or unmute a mixer track.",
            set_mute,
        ),
        cmd(
            "mixer.toggle_mute",
            "mixer.toggle_mute <track>",
            "Toggle a mixer track's mute.",
            toggle_mute,
        ),
        cmd(
            "mixer.set_solo",
            "mixer.set_solo <track> <on|off>",
            "Solo or unsolo a mixer track.",
            set_solo,
        ),
        cmd(
            "mixer.toggle_solo",
            "mixer.toggle_solo <track>",
            "Toggle a mixer track's solo.",
            toggle_solo,
        ),
        cmd(
            "mixer.set_send",
            "mixer.set_send <track> <target track> <level 0-1.25>",
            "Send an insert to a higher-numbered insert (level 0 removes the send).",
            set_send,
        ),
        cmd(
            "mixer.route_to_master",
            "mixer.route_to_master <track> <on|off>",
            "Choose whether an insert feeds the master directly.",
            route_to_master,
        ),
        cmd(
            "mixer.sidechain_to",
            "mixer.sidechain_to <source insert> <target track> <on|off>",
            "Route an insert to another track as a sidechain key (not heard there).",
            sidechain_to,
        ),
        cmd(
            "mixer.add_effect",
            "mixer.add_effect <track> <kind>",
            "Append an effect (filter, eq, delay, reverb, compressor, distortion).",
            add_effect,
        ),
        cmd(
            "mixer.remove_effect",
            "mixer.remove_effect <track> <slot>",
            "Remove the effect in a slot.",
            remove_effect,
        ),
        cmd(
            "mixer.insert_effect",
            "mixer.insert_effect <track> <slot> <json>",
            "Insert an effect from its JSON form (used by undo).",
            insert_effect,
        ),
        cmd(
            "mixer.move_effect",
            "mixer.move_effect <track> <slot> <new slot>",
            "Reorder an effect within a track.",
            move_effect,
        ),
        cmd(
            "mixer.set_effect_param",
            "mixer.set_effect_param <track> <slot> <param> <value>",
            "Set an effect parameter.",
            set_effect_param,
        ),
        cmd(
            "mixer.set_effects_enabled",
            "mixer.set_effects_enabled <track> <on|off>",
            "Enable or bypass every effect of a track at once.",
            set_effects_enabled,
        ),
        cmd(
            "mixer.set_effect_enabled",
            "mixer.set_effect_enabled <track> <slot> <on|off>",
            "Enable or bypass an effect.",
            set_effect_enabled,
        ),
        cmd(
            "mixer.set_effect_file",
            "mixer.set_effect_file <track> <slot> <file|ir:name|->",
            "Load the convolution reverb's impulse response: an audio file or a generated one (ir:room, ir:chamber, ir:plate, ir:hall, ir:cathedral); - restores the default.",
            set_effect_file,
        ),
        cmd(
            "mixer.toggle_effect",
            "mixer.toggle_effect <track> <slot>",
            "Toggle an effect's bypass.",
            toggle_effect,
        ),
    ]
}

/// Tracks named by effect parameters (sidechain sources and send targets),
/// as (track, slot, parameter index, named track).
fn sidechains(project: &Project) -> Vec<(usize, usize, usize, usize)> {
    let mut out = Vec::new();
    for (t, track) in project.mixer.iter().enumerate() {
        for (s, effect) in track.effects.iter().enumerate() {
            for index in effect.kind.track_ref_indices() {
                let source = effect.values.get(index).copied().unwrap_or(0.0).round() as usize;
                if source > 0 {
                    out.push((t, s, index, source));
                }
            }
        }
    }
    out
}

/// The inserts a track's send effects and band splitters feed.
fn effect_targets(track: &MixerTrack) -> impl Iterator<Item = usize> + '_ {
    track.effects.iter().flat_map(|effect| {
        (0..effect.kind.params().len())
            .filter(|&i| effect.kind.is_send_target(i))
            .map(|i| effect.values.get(i).copied().unwrap_or(0.0).round() as usize)
    })
}

/// Tracks that track `from` feeds directly: its sends, its send effects'
/// targets and its sidechain routes (the master is left out; it runs last).
fn feeds_directly(project: &Project, from: usize) -> Vec<usize> {
    let Some(track) = project.mixer.get(from).filter(|_| from > 0) else {
        return Vec::new();
    };
    track
        .sends
        .iter()
        .map(|send| send.target)
        .chain(effect_targets(track))
        .chain(track.sidechain_to.iter().copied())
        .filter(|&t| t > 0)
        .collect()
}

/// Whether audio or a sidechain key flows from track `from` to track `to`,
/// directly or through other inserts.
fn feeds(project: &Project, from: usize, to: usize) -> bool {
    let mut seen = vec![false; project.mixer.len()];
    let mut stack = vec![from];
    while let Some(track) = stack.pop() {
        if track == to {
            return true;
        }
        if std::mem::replace(seen.get_mut(track).unwrap_or(&mut true), true) {
            continue;
        }
        stack.extend(feeds_directly(project, track));
    }
    false
}

/// The error for a connection from `from` to `to` that would close a loop.
fn loop_error(args: &Args<'_>, from: usize, to: usize) -> CommandError {
    args.invalid(format!(
        "track {to} already feeds track {from} (through sends or sidechains), so this would make a loop"
    ))
}

/// Effects that follow a channel, as (track, slot, parameter index, rack
/// position counted from 1).
pub(super) fn channel_followers(project: &Project) -> Vec<(usize, usize, usize, usize)> {
    let mut out = Vec::new();
    for (t, track) in project.mixer.iter().enumerate() {
        for (s, effect) in track.effects.iter().enumerate() {
            if let Some(index) = effect.kind.channel_ref_index() {
                let position = effect.values.get(index).copied().unwrap_or(0.0).round() as usize;
                if position > 0 {
                    out.push((t, s, index, position));
                }
            }
        }
    }
    out
}

/// Runs `change` (which reorders channels) and moves every followed channel
/// position along with its channel; a followed channel that is gone stops
/// being followed.
pub(super) fn keeping_followers<T>(
    project: &mut Project,
    change: impl FnOnce(&mut Project) -> T,
) -> T {
    let followed: Vec<_> = channel_followers(project)
        .into_iter()
        .map(|(t, s, i, position)| (t, s, i, project.channels.get(position - 1).map(|c| c.id)))
        .collect();
    let out = change(project);
    for (t, s, i, id) in followed {
        let position = id
            .and_then(|id| project.channel_position(id))
            .map_or(0, |p| p + 1);
        if let Some(value) = project
            .mixer
            .get_mut(t)
            .and_then(|track| track.effects.get_mut(s))
            .and_then(|e| e.values.get_mut(i))
        {
            *value = position as f32;
        }
    }
    out
}

/// Shifts sidechain sources and send targets at or after `from` by `delta`.
fn shift_sidechains(project: &mut Project, from: usize, delta: isize) {
    for (t, s, index, source) in sidechains(project) {
        if source >= from {
            project.mixer[t].effects[s].values[index] = source.saturating_add_signed(delta) as f32;
        }
    }
}

fn move_track(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = insert_arg(project, args, 0)?;
    let (other, back) = match args.str(1)? {
        "left" => (index - 1, "right"),
        "right" => (index + 1, "left"),
        _ => return Err(args.invalid("expected left or right")),
    };
    if other == 0 || other >= project.mixer.len() {
        return Err(args.invalid(format!("insert {index} cannot move further that way")));
    }
    let (low, high) = (index.min(other), index.max(other));
    if project.mixer[low]
        .sends
        .iter()
        .any(|send| send.target == high)
        || effect_targets(&project.mixer[low]).any(|t| t == high)
    {
        return Err(args.invalid(format!(
            "insert {low} sends to insert {high}; sends must go to a higher number, so remove it first"
        )));
    }
    let swap = |track: usize| match track {
        t if t == index => other,
        t if t == other => index,
        t => t,
    };
    for channel in &mut project.channels {
        channel.mixer_track = swap(channel.mixer_track);
    }
    for track in &mut project.mixer {
        for send in &mut track.sends {
            send.target = swap(send.target);
        }
        track.sends.sort_by_key(|send| send.target);
        for target in &mut track.sidechain_to {
            *target = swap(*target);
        }
        track.sidechain_to.sort_unstable();
    }
    for (t, s, i, source) in sidechains(project) {
        project.mixer[t].effects[s].values[i] = swap(source) as f32;
    }
    super::automation::map_tracks(project, swap);
    project.mixer.swap(index, other);
    Ok(Outcome::undoable(
        format!("moved mixer track {index} to {other}"),
        Invocation::new(args.name(), [other.to_string(), back.to_owned()]),
    ))
}

pub(super) fn track_arg(
    project: &Project,
    args: &Args<'_>,
    i: usize,
) -> Result<usize, CommandError> {
    Ok(project.mixer_index(args.parse(i, "mixer track")?)?)
}

fn insert_arg(project: &Project, args: &Args<'_>, i: usize) -> Result<usize, CommandError> {
    let track = track_arg(project, args, i)?;
    if track == 0 {
        return Err(args.invalid("this applies to inserts, not the master"));
    }
    Ok(track)
}

pub(super) fn slot_arg(
    project: &Project,
    args: &Args<'_>,
    track: usize,
    i: usize,
) -> Result<usize, CommandError> {
    let slot = args.number(i, "effect slot")?;
    if slot > project.mixer[track].effects.len() {
        return Err(ProjectError::NotFound {
            what: "effect slot",
            number: slot,
        }
        .into());
    }
    Ok(slot - 1)
}

fn add_track(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    if project.mixer.len() >= MAX_MIXER_TRACKS {
        return Err(args.invalid(format!("the mixer is limited to {MAX_MIXER_TRACKS} tracks")));
    }
    let number = project.mixer.len();
    let name = if args.len() > 0 {
        args.rest(0)?
    } else {
        format!("Insert {number}")
    };
    project.mixer.push(MixerTrack::new(name.clone()));
    Ok(Outcome::undoable(
        format!("added mixer track {number} ({name})"),
        Invocation::new("mixer.remove_track", [number.to_string()]),
    ))
}

#[derive(Serialize, Deserialize)]
struct TrackSnapshot {
    track: MixerTrack,
    routed: Vec<ChannelId>,
    /// Sends from other (lower) tracks into this one: (source track, level).
    #[serde(default)]
    incoming: Vec<(usize, f32)>,
    /// Inserts routed to this one as a sidechain.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keyed_by: Vec<usize>,
}

fn remove_track(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let index = insert_arg(project, args, 0)?;
    if super::automation::targets_track(project, index, None) {
        return Err(args.invalid("an automation or controller uses this track; delete it first"));
    }
    if sidechains(project)
        .iter()
        .any(|(_, _, _, source)| *source == index)
    {
        return Err(args.invalid(
            "an effect uses this track as its sidechain or send target; change it first",
        ));
    }
    shift_sidechains(project, index + 1, -1);
    let track = project.mixer.remove(index);
    super::automation::shift_tracks(project, index + 1, -1);
    let mut incoming = Vec::new();
    for (source, other) in project.mixer.iter_mut().enumerate() {
        other.sends.retain(|send| {
            let keep = send.target != index;
            if !keep {
                incoming.push((source, send.level));
            }
            keep
        });
        for send in &mut other.sends {
            if send.target > index {
                send.target -= 1;
            }
        }
    }
    let mut keyed_by = Vec::new();
    for (source, other) in project.mixer.iter_mut().enumerate() {
        if other.sidechain_to.contains(&index) {
            keyed_by.push(source);
        }
        other.sidechain_to.retain(|&t| t != index);
        for target in &mut other.sidechain_to {
            if *target > index {
                *target -= 1;
            }
        }
    }
    let mut routed = Vec::new();
    for channel in &mut project.channels {
        if channel.mixer_track == index {
            channel.mixer_track = 0;
            routed.push(channel.id);
        } else if channel.mixer_track > index {
            channel.mixer_track -= 1;
        }
    }
    let name = track.name.clone();
    let json = serde_json::to_string(&TrackSnapshot {
        track,
        routed,
        incoming,
        keyed_by,
    })
    .map_err(|e| args.invalid(e.to_string()))?;
    Ok(Outcome::undoable(
        format!("removed mixer track {index} ({name})"),
        Invocation::new("mixer.insert_track", [index.to_string(), json]),
    ))
}

/// The name a track has when made: "Master", or "Insert n".
fn default_track_name(index: usize) -> String {
    if index == 0 {
        "Master".to_owned()
    } else {
        format!("Insert {index}")
    }
}

fn reset_track(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let index = track_arg(project, args, 0)?;
    let slots = project.mixer[index].effects.len();
    if (0..slots).any(|slot| super::automation::targets_track(project, index, Some(slot))) {
        return Err(args.invalid(
            "an automation drives one of this track's effects; delete the automation first",
        ));
    }
    let old = std::mem::replace(
        &mut project.mixer[index],
        MixerTrack::new(default_track_name(index)),
    );
    let json = serde_json::to_string(&old).map_err(|e| args.invalid(e.to_string()))?;
    Ok(Outcome::undoable(
        format!("reset mixer track {index}"),
        Invocation::new("mixer.set_track", [index.to_string(), json]),
    ))
}

fn set_track(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let track: MixerTrack =
        serde_json::from_str(args.str(1)?).map_err(|e| args.invalid(e.to_string()))?;
    let len = project.mixer.len();
    if track
        .sends
        .iter()
        .any(|send| send.target <= index || send.target >= len)
    {
        return Err(args.invalid("sends must go to existing higher-numbered tracks"));
    }
    if index == 0 && !track.sidechain_to.is_empty()
        || track.sidechain_to.iter().any(|&t| t == index || t >= len)
    {
        return Err(args.invalid("sidechain routes must go to other existing tracks"));
    }
    if track.effects.len() > MAX_EFFECT_SLOTS {
        return Err(args.invalid(format!("a track holds at most {MAX_EFFECT_SLOTS} effects")));
    }
    let old = std::mem::replace(&mut project.mixer[index], track);
    let json = serde_json::to_string(&old).map_err(|e| args.invalid(e.to_string()))?;
    Ok(Outcome::undoable(
        format!("set mixer track {index}"),
        Invocation::new("mixer.set_track", [index.to_string(), json]),
    ))
}

fn insert_track(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index: usize = args.parse(0, "mixer track")?;
    if index == 0 || index > project.mixer.len() {
        return Err(args.invalid(format!("cannot insert a track at {index}")));
    }
    if project.mixer.len() >= MAX_MIXER_TRACKS {
        return Err(args.invalid(format!("the mixer is limited to {MAX_MIXER_TRACKS} tracks")));
    }
    let snapshot: TrackSnapshot =
        serde_json::from_str(args.str(1)?).map_err(|e| args.invalid(e.to_string()))?;
    for channel in &mut project.channels {
        if channel.mixer_track >= index {
            channel.mixer_track += 1;
        }
        if snapshot.routed.contains(&channel.id) {
            channel.mixer_track = index;
        }
    }
    for other in &mut project.mixer {
        for send in &mut other.sends {
            if send.target >= index {
                send.target += 1;
            }
        }
    }
    for other in &mut project.mixer {
        for target in &mut other.sidechain_to {
            if *target >= index {
                *target += 1;
            }
        }
    }
    for source in &snapshot.keyed_by {
        if let Some(other) = project.mixer.get_mut(*source) {
            other.sidechain_to.push(index);
            other.sidechain_to.sort_unstable();
        }
    }
    for (source, level) in &snapshot.incoming {
        if let Some(other) = project.mixer.get_mut(*source) {
            other.sends.push(MixerSend {
                target: index,
                level: *level,
            });
            other.sends.sort_by_key(|send| send.target);
        }
    }
    let name = snapshot.track.name.clone();
    super::automation::shift_tracks(project, index, 1);
    shift_sidechains(project, index, 1);
    project.mixer.insert(index, snapshot.track);
    Ok(Outcome::undoable(
        format!("inserted mixer track {index} ({name})"),
        Invocation::new("mixer.remove_track", [index.to_string()]),
    ))
}

fn rename(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, usize::MAX)?;
    let index = track_arg(project, args, 0)?;
    let name = args.rest(1)?;
    let previous = std::mem::replace(&mut project.mixer[index].name, name.clone());
    Ok(Outcome::undoable(
        format!("mixer track {index} renamed to {name}"),
        Invocation::new(args.name(), [index.to_string(), previous]),
    ))
}

/// Replaces a track field; `raw` formats a value as a command argument.
fn set_track_field<T: PartialEq + Copy>(
    project: &mut Project,
    args: &Args<'_>,
    index: usize,
    value: T,
    field: fn(&mut MixerTrack) -> &mut T,
    raw: fn(T) -> String,
    label: &str,
) -> Outcome {
    let previous = std::mem::replace(field(&mut project.mixer[index]), value);
    let message = format!("mixer track {index} {label} {}", raw(value));
    if previous == value {
        Outcome::done(message)
    } else {
        Outcome::undoable(
            message,
            Invocation::new(args.name(), [index.to_string(), raw(previous)]),
        )
    }
}

fn set_volume(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let value = check_volume(args.f32(1, "volume")?)?;
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.volume,
        num,
        "volume",
    ))
}

fn set_separation(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let value =
        musiccraft_project::check_range("separation", args.f32(1, "separation")?, -1.0, 1.0)?;
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.separation,
        num,
        "separation",
    ))
}

fn set_eq(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 4)?;
    let index = track_arg(project, args, 0)?;
    let raw = args.str(1)?;
    let band = musiccraft_project::TrackEq::BANDS
        .iter()
        .position(|b| *b == raw)
        .ok_or_else(|| args.invalid(format!("`{raw}` is not low, mid or high")))?;
    let max = musiccraft_project::TrackEq::MAX_GAIN;
    let gain = musiccraft_project::check_range("EQ gain", args.f32(2, "gain")?, -max, max)?;
    let old = project.mixer[index].eq.bands[band];
    let freq = match args.opt(3) {
        Some(_) => {
            let (lo, hi) = musiccraft_project::TrackEq::RANGES[band];
            musiccraft_project::check_range("EQ frequency", args.f32(3, "frequency")?, lo, hi)?
        }
        None => old.1,
    };
    let message = format!("{raw} band {gain:+.1} dB at {freq:.0} Hz");
    if old == (gain, freq) {
        return Ok(Outcome::done(message));
    }
    project.mixer[index].eq.bands[band] = (gain, freq);
    Ok(Outcome::undoable(
        message,
        Invocation::new(
            args.name(),
            [index.to_string(), raw.to_owned(), num(old.0), num(old.1)],
        ),
    ))
}

fn set_effect_mix(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let index = track_arg(project, args, 0)?;
    let slot = slot_arg(project, args, index, 1)?;
    let mix = musiccraft_project::check_range("mix", args.f32(2, "mix")?, 0.0, 1.0)?;
    let effect = &mut project.mixer[index].effects[slot];
    let old = std::mem::replace(&mut effect.mix, mix);
    let message = format!("{} mix {:.0}%", effect.kind, mix * 100.0);
    if old == mix {
        return Ok(Outcome::done(message));
    }
    Ok(Outcome::undoable(
        message,
        Invocation::new(
            args.name(),
            [index.to_string(), (slot + 1).to_string(), num(old)],
        ),
    ))
}

fn set_phase_invert(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let value = args.flag(1)?;
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.phase_invert,
        |v| on_off(v).to_owned(),
        "phase invert",
    ))
}

fn set_dock(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    if index == 0 {
        return Err(args.invalid("the master always sits in the left dock"));
    }
    let dock =
        Dock::parse(args.str(1)?).ok_or_else(|| args.invalid("docks are left, middle or right"))?;
    Ok(set_track_field(
        project,
        args,
        index,
        dock,
        |t| &mut t.dock,
        |d| d.arg().to_owned(),
        "dock",
    ))
}

fn set_track_color(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let value = match args.str(1)? {
        "none" => None,
        _ => {
            let n = args.number(1, "colour")?;
            if !(1..=16).contains(&n) {
                return Err(args.invalid("colours are 1 to 16"));
            }
            Some(u8::try_from(n - 1).unwrap_or(0))
        }
    };
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.color,
        |c| c.map_or_else(|| "none".to_owned(), |c| (c + 1).to_string()),
        "colour",
    ))
}

fn set_swap_sides(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let value = args.flag(1)?;
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.swap_sides,
        |v| on_off(v).to_owned(),
        "swap sides",
    ))
}

fn set_pan(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let value = check_pan(args.f32(1, "pan")?)?;
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.pan,
        num,
        "pan",
    ))
}

fn flag_text(value: bool) -> String {
    on_off(value).to_owned()
}

fn set_mute(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let value = args.flag(1)?;
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.mute,
        flag_text,
        "mute",
    ))
}

fn toggle_mute(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let index = track_arg(project, args, 0)?;
    let track = &mut project.mixer[index];
    track.mute = !track.mute;
    Ok(Outcome::undoable(
        format!("mixer track {index} mute {}", on_off(track.mute)),
        Invocation::new(args.name(), [index.to_string()]),
    ))
}

fn set_solo(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let value = args.flag(1)?;
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.solo,
        flag_text,
        "solo",
    ))
}

fn toggle_solo(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let index = track_arg(project, args, 0)?;
    let track = &mut project.mixer[index];
    track.solo = !track.solo;
    Ok(Outcome::undoable(
        format!("mixer track {index} solo {}", on_off(track.solo)),
        Invocation::new(args.name(), [index.to_string()]),
    ))
}

fn add_effect(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let raw = args.str(1)?;
    let kind = EffectKind::parse(raw).ok_or_else(|| {
        let names: Vec<_> = EFFECT_KINDS.iter().map(|k| k.name()).collect();
        args.invalid(format!(
            "unknown effect `{raw}`; choose from {}",
            names.join(", ")
        ))
    })?;
    let effects = &mut project.mixer[index].effects;
    if effects.len() >= MAX_EFFECT_SLOTS {
        return Err(args.invalid(format!("a track holds at most {MAX_EFFECT_SLOTS} effects")));
    }
    effects.push(EffectSlot::new(kind));
    let slot = effects.len();
    Ok(Outcome::undoable(
        format!("added {kind} to mixer track {index} slot {slot}"),
        Invocation::new("mixer.remove_effect", [index.to_string(), slot.to_string()]),
    ))
}

fn remove_effect(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let slot = slot_arg(project, args, index, 1)?;
    if super::automation::targets_track(project, index, Some(slot)) {
        return Err(args.invalid("an automation drives this effect; delete the automation first"));
    }
    let effect = project.mixer[index].effects.remove(slot);
    super::automation::remap_slots(project, index, |s| Some(if s > slot { s - 1 } else { s }));
    let json = serde_json::to_string(&effect).map_err(|e| args.invalid(e.to_string()))?;
    Ok(Outcome::undoable(
        format!("removed {} from mixer track {index}", effect.label()),
        Invocation::new(
            "mixer.insert_effect",
            [index.to_string(), (slot + 1).to_string(), json],
        ),
    ))
}

fn insert_effect(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let index = track_arg(project, args, 0)?;
    let slot = args.number(1, "effect slot")?;
    let effects = &project.mixer[index].effects;
    if slot > effects.len() + 1 || effects.len() >= MAX_EFFECT_SLOTS {
        return Err(args.invalid(format!("cannot insert an effect at slot {slot}")));
    }
    let effect: EffectSlot =
        serde_json::from_str(args.str(2)?).map_err(|e| args.invalid(e.to_string()))?;
    if let Some(plugin) = &effect.plugin {
        if project.plugin_slot(plugin.instance).is_some() {
            return Err(args.invalid(format!(
                "plugin instance {} already exists",
                plugin.instance
            )));
        }
        project.reserve_id(plugin.instance);
    }
    super::automation::remap_slots(project, index, |s| {
        Some(if s >= slot - 1 { s + 1 } else { s })
    });
    let label = effect.label();
    project.mixer[index].effects.insert(slot - 1, effect);
    Ok(Outcome::undoable(
        format!("inserted {label} on mixer track {index}"),
        Invocation::new("mixer.remove_effect", [index.to_string(), slot.to_string()]),
    ))
}

fn move_effect(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let index = track_arg(project, args, 0)?;
    let from = slot_arg(project, args, index, 1)?;
    let to = slot_arg(project, args, index, 2)?;
    let effects = &mut project.mixer[index].effects;
    let effect = effects.remove(from);
    effects.insert(to, effect);
    super::automation::remap_slots(project, index, |s| {
        Some(if s == from {
            to
        } else if from < to && s > from && s <= to {
            s - 1
        } else if to < from && s >= to && s < from {
            s + 1
        } else {
            s
        })
    });
    Ok(Outcome::undoable(
        format!("moved effect to slot {}", to + 1),
        Invocation::new(
            args.name(),
            [
                index.to_string(),
                (to + 1).to_string(),
                (from + 1).to_string(),
            ],
        ),
    ))
}

fn set_effect_param(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(4, 4)?;
    let index = track_arg(project, args, 0)?;
    let slot = slot_arg(project, args, index, 1)?;
    let project_tracks = project.mixer.len();
    let kind = project.mixer[index].effects[slot].kind;
    let specs = kind.params();
    let name = args.str(2)?;
    let param = index_of(specs, name).ok_or_else(|| {
        let names: Vec<_> = specs.iter().map(|p| p.name).collect();
        args.invalid(format!(
            "{kind} has no parameter `{name}`; choose from {}",
            names.join(", ")
        ))
    })?;
    let spec = specs[param];
    let value = args.f32(3, name)?;
    if !spec.accepts(value) {
        return Err(ProjectError::out_of_range(
            name,
            f64::from(value),
            f64::from(spec.min),
            f64::from(spec.max),
        )
        .into());
    }
    let value = spec.sanitize(value);
    if kind.sidechain_index() == Some(param) && value >= 0.5 {
        let source = value as usize;
        if source >= project_tracks || source == index {
            return Err(args.invalid("a sidechain comes from another existing insert"));
        }
        if !project.mixer[source].sidechain_to.contains(&index) {
            return Err(args.invalid(format!(
                "insert {source} is not routed to track {index} as a sidechain; \
                 add it first (mixer.sidechain_to {source} {index} on)"
            )));
        }
    }
    if kind.is_send_target(param) && value >= 0.5 {
        let target = value as usize;
        if index == 0 || target <= index || target >= project_tracks {
            return Err(args.invalid("a send effect feeds a higher-numbered insert"));
        }
        if feeds(project, target, index) {
            return Err(loop_error(args, index, target));
        }
    }
    let effect = &mut project.mixer[index].effects[slot];
    let previous = std::mem::replace(&mut effect.values[param], value);
    let message = format!("{} {name} {}", effect.kind, num(value));
    if previous == value {
        return Ok(Outcome::done(message));
    }
    Ok(Outcome::undoable(
        message,
        Invocation::new(
            args.name(),
            [
                index.to_string(),
                (slot + 1).to_string(),
                name.to_owned(),
                num(previous),
            ],
        ),
    ))
}

fn set_effect_enabled(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let index = track_arg(project, args, 0)?;
    let slot = slot_arg(project, args, index, 1)?;
    let enabled = args.flag(2)?;
    let effect = &mut project.mixer[index].effects[slot];
    let previous = std::mem::replace(&mut effect.enabled, enabled);
    let message = format!(
        "{} {}",
        effect.kind,
        if enabled { "enabled" } else { "bypassed" }
    );
    if previous == enabled {
        return Ok(Outcome::done(message));
    }
    Ok(Outcome::undoable(
        message,
        Invocation::new(
            args.name(),
            [
                index.to_string(),
                (slot + 1).to_string(),
                on_off(previous).to_owned(),
            ],
        ),
    ))
}

fn set_effects_enabled(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let enabled = args.flag(1)?;
    let track = &mut project.mixer[index];
    // Each slot that changes gets its own line in the inverse.
    let restore: Vec<String> = track
        .effects
        .iter_mut()
        .enumerate()
        .filter(|(_, effect)| effect.enabled != enabled)
        .map(|(slot, effect)| {
            effect.enabled = enabled;
            Invocation::new(
                "mixer.set_effect_enabled",
                [
                    index.to_string(),
                    (slot + 1).to_string(),
                    on_off(!enabled).to_owned(),
                ],
            )
            .to_string()
        })
        .collect();
    let message = format!(
        "{} effects {}",
        track.name,
        if enabled { "enabled" } else { "bypassed" }
    );
    if restore.is_empty() {
        return Ok(Outcome::done(message));
    }
    Ok(Outcome::undoable(message, Invocation::new(BATCH, restore)))
}

fn set_effect_file(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let index = track_arg(project, args, 0)?;
    let slot = slot_arg(project, args, index, 1)?;
    let file = args.str(2)?;
    if project.mixer[index].effects[slot].kind != EffectKind::Convolution {
        return Err(args.invalid("only the convolution reverb loads a file"));
    }
    let new = match file {
        "-" => None,
        _ => {
            if let Some(name) = file.strip_prefix("ir:") {
                if !musiccraft_project::params::IMPULSE_RESPONSES.contains(&name) {
                    return Err(args.invalid(format!(
                        "unknown impulse response `{name}`; choose from {}",
                        musiccraft_project::params::IMPULSE_RESPONSES.join(", ")
                    )));
                }
            } else {
                super::channel::check_sample(project, args, file)?;
            }
            Some(file.to_owned())
        }
    };
    let effect = &mut project.mixer[index].effects[slot];
    let previous = std::mem::replace(&mut effect.file, new);
    Ok(Outcome::undoable(
        format!("{} loads {file}", effect.kind),
        Invocation::new(
            args.name(),
            [
                index.to_string(),
                (slot + 1).to_string(),
                previous.unwrap_or_else(|| "-".to_owned()),
            ],
        ),
    ))
}

fn toggle_effect(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = track_arg(project, args, 0)?;
    let slot = slot_arg(project, args, index, 1)?;
    let effect = &mut project.mixer[index].effects[slot];
    effect.enabled = !effect.enabled;
    Ok(Outcome::undoable(
        format!(
            "{} {}",
            effect.kind,
            if effect.enabled {
                "enabled"
            } else {
                "bypassed"
            }
        ),
        Invocation::new(args.name(), [index.to_string(), (slot + 1).to_string()]),
    ))
}

fn set_send(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let index = insert_arg(project, args, 0)?;
    let target = track_arg(project, args, 1)?;
    if target <= index {
        return Err(args.invalid("sends go to a higher-numbered insert"));
    }
    let level = check_volume(args.f32(2, "level")?)?;
    if level > 0.0 && feeds(project, target, index) {
        return Err(loop_error(args, index, target));
    }
    let sends = &mut project.mixer[index].sends;
    let previous = sends
        .iter()
        .find(|s| s.target == target)
        .map_or(0.0, |s| s.level);
    sends.retain(|s| s.target != target);
    if level > 0.0 {
        if sends.len() >= MAX_SENDS {
            return Err(args.invalid(format!("a track has at most {MAX_SENDS} sends")));
        }
        sends.push(MixerSend { target, level });
        sends.sort_by_key(|s| s.target);
    }
    let message = if level > 0.0 {
        format!("mixer track {index} sends {} to {target}", num(level))
    } else {
        format!("mixer track {index} no longer sends to {target}")
    };
    if (previous - level).abs() < f32::EPSILON {
        return Ok(Outcome::done(message));
    }
    Ok(Outcome::undoable(
        message,
        Invocation::new(
            args.name(),
            [index.to_string(), target.to_string(), num(previous)],
        ),
    ))
}

fn sidechain_to(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let source = insert_arg(project, args, 0)?;
    let target = track_arg(project, args, 1)?;
    let on = args.flag(2)?;
    if target == source {
        return Err(args.invalid("a track cannot key itself"));
    }
    let routed = project.mixer[source].sidechain_to.contains(&target);
    let message = if on {
        format!("insert {source} sidechained to track {target}")
    } else {
        format!("insert {source} no longer sidechained to track {target}")
    };
    if routed == on {
        return Ok(Outcome::done(message));
    }
    if on {
        if project.mixer[source].sidechain_to.len() >= MAX_SENDS {
            return Err(args.invalid(format!(
                "an insert has at most {MAX_SENDS} sidechain routes"
            )));
        }
        if target > 0 && feeds(project, target, source) {
            return Err(loop_error(args, source, target));
        }
    }
    let to = &mut project.mixer[source].sidechain_to;
    to.retain(|&t| t != target);
    if on {
        to.push(target);
        to.sort_unstable();
    }
    Ok(Outcome::undoable(
        message,
        Invocation::new(
            args.name(),
            [source.to_string(), target.to_string(), flag_text(!on)],
        ),
    ))
}

fn route_to_master(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let index = insert_arg(project, args, 0)?;
    let value = args.flag(1)?;
    Ok(set_track_field(
        project,
        args,
        index,
        value,
        |t| &mut t.to_master,
        flag_text,
        "to master",
    ))
}
