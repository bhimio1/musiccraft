use musiccraft_project::music::key_name;
use musiccraft_project::params::TICKS_PER_STEP;
use musiccraft_project::{DEFAULT_VELOCITY, MAX_PATTERN_STEPS, Note, Project, STEP_KEY};

use super::channel::channel_arg;
use crate::args::{Args, at_pattern, ticks};
use crate::command::cmd;
use crate::{BATCH, Command, CommandError, Invocation, Outcome};

pub(super) fn commands() -> Vec<Box<dyn Command>> {
    vec![
        cmd(
            "step.toggle",
            "step.toggle <channel> <step> [@pattern]",
            "Toggle a step (0-based) in the channel rack.",
            step_toggle,
        ),
        cmd(
            "step.set",
            "step.set <channel> <step> <on|off> [@pattern]",
            "Turn a step on or off.",
            step_set,
        ),
        cmd(
            "note.add",
            "note.add <channel> <start> <length> <key> [velocity] [pan] [slide] [@pattern]",
            "Add a note; times in steps (or <n>t ticks), key as 60 or C5; `slide` makes a slide note.",
            add,
        ),
        cmd(
            "note.add_chord",
            "note.add_chord <channel> <start> <length> <root> <quality> [velocity] [@pattern]",
            "Add a chord (major, minor, dim, aug, sus2, sus4, maj7, min7, dom7, dim7, add9).",
            add_chord,
        ),
        cmd(
            "note.set_pan",
            "note.set_pan <channel> <start> <key> <-100..100> [@pattern]",
            "Change a note's pan.",
            set_pan,
        ),
        cmd(
            "note.set_slide",
            "note.set_slide <channel> <start> <key> <on|off> [@pattern]",
            "Make a note a slide note (glides from the sounding note) or not.",
            set_slide,
        ),
        cmd(
            "note.restore",
            "note.restore <channel> <json> [@pattern]",
            "Put a note back exactly from its JSON form (used by undo).",
            restore,
        ),
        cmd(
            "note.set_prop",
            "note.set_prop <channel> <start> <key> <release|fine|mod_x|mod_y|color|porta|muted> <value> [@pattern]",
            "Set a per-note property.",
            set_prop,
        ),
        cmd(
            "note.remove",
            "note.remove <channel> <start> <key> [@pattern]",
            "Remove the note at a start time and key.",
            remove,
        ),
        cmd(
            "note.move",
            "note.move <channel> <start> <key> <new start> <new key> [@pattern]",
            "Move a note in time and pitch.",
            move_note,
        ),
        cmd(
            "note.slice",
            "note.slice <channel> <start> <key> <at> [@pattern]",
            "Cut a note in two at a time inside it (both keep its properties).",
            slice,
        ),
        cmd(
            "note.resize",
            "note.resize <channel> <start> <key> <length> [@pattern]",
            "Change a note's length.",
            resize,
        ),
        cmd(
            "note.set_velocity",
            "note.set_velocity <channel> <start> <key> <1-127> [@pattern]",
            "Change a note's velocity.",
            set_velocity,
        ),
        cmd(
            "note.clear",
            "note.clear <channel> [@pattern]",
            "Remove every note of a channel in the pattern.",
            clear,
        ),
        cmd(
            "note.set_all",
            "note.set_all <channel> <json> [@pattern]",
            "Replace a channel's notes with [[start,length,key,velocity],...] in ticks.",
            set_all,
        ),
        cmd(
            "note.transpose",
            "note.transpose <channel> <semitones> [@pattern]",
            "Shift every note of a channel up or down.",
            transpose,
        ),
        cmd(
            "note.import_midi",
            "note.import_midi <channel> <file.mid> [@pattern]",
            "Add every note from a Standard MIDI File to a channel.",
            import_midi,
        ),
        cmd(
            "note.quantize",
            "note.quantize <channel> <grid steps> [@pattern]",
            "Snap note starts to a grid.",
            quantize,
        ),
    ]
}

/// The pattern, channel index and channel number a note command applies to.
pub(super) struct Target {
    pub(super) pattern: usize,
    pub(super) channel: usize,
}

impl Target {
    pub(super) fn resolve(project: &Project, args: &Args<'_>) -> Result<Self, CommandError> {
        Ok(Self {
            pattern: args.pattern_index(project)?,
            channel: channel_arg(project, args, 0)?,
        })
    }

    fn number(&self) -> String {
        (self.channel + 1).to_string()
    }

    /// Puts `note` back exactly as it was, every property included.
    fn add_inverse(&self, note: &Note) -> Invocation {
        let json = serde_json::to_string(&NoteJson::from(note)).unwrap_or_default();
        Invocation::new(
            "note.restore",
            [self.number(), json, at_pattern(self.pattern)],
        )
    }

    fn remove_inverse(&self, start: u32, key: u8) -> Invocation {
        Invocation::new(
            "note.remove",
            [
                self.number(),
                ticks(start),
                key.to_string(),
                at_pattern(self.pattern),
            ],
        )
    }

    pub(super) fn set_all_inverse(&self, notes: &[Note]) -> Invocation {
        let rows: Vec<NoteJson> = notes.iter().map(NoteJson::from).collect();
        let json = serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_owned());
        Invocation::new(
            "note.set_all",
            [self.number(), json, at_pattern(self.pattern)],
        )
    }
}

/// A note without its channel, as stored in undo inverses.
#[derive(serde::Serialize, serde::Deserialize)]
struct NoteJson {
    start: u32,
    length: u32,
    key: u8,
    velocity: u8,
    #[serde(default)]
    pan: i8,
    #[serde(default)]
    slide: bool,
    #[serde(flatten)]
    props: musiccraft_project::NoteProps,
}

impl From<&Note> for NoteJson {
    fn from(n: &Note) -> Self {
        Self {
            start: n.start,
            length: n.length,
            key: n.key,
            velocity: n.velocity,
            pan: n.pan,
            slide: n.slide,
            props: n.props,
        }
    }
}

impl NoteJson {
    /// The note for `channel`, if its values are in range.
    fn note(self, channel: musiccraft_project::ChannelId) -> Option<Note> {
        let p = self.props;
        let valid = self.key <= 127
            && (1..=127).contains(&self.velocity)
            && self.length > 0
            && (-100..=100).contains(&self.pan)
            && p.release <= 127
            && (-100..=100).contains(&p.fine)
            && p.mod_x <= 127
            && p.mod_y <= 127
            && p.color < 16;
        valid.then_some(Note {
            channel,
            start: self.start,
            length: self.length,
            key: self.key,
            velocity: self.velocity,
            pan: self.pan,
            slide: self.slide,
            props: p,
        })
    }
}

fn batch(inverses: Vec<Invocation>) -> Invocation {
    if inverses.len() == 1 {
        inverses
            .into_iter()
            .next()
            .unwrap_or_else(|| Invocation::bare(BATCH))
    } else {
        Invocation::new(BATCH, inverses.iter().map(ToString::to_string))
    }
}

fn step_tick(args: &Args<'_>, i: usize) -> Result<u32, CommandError> {
    let step: u32 = args.parse(i, "step")?;
    if step >= MAX_PATTERN_STEPS {
        return Err(args.invalid(format!("step must be below {MAX_PATTERN_STEPS}")));
    }
    Ok(step * TICKS_PER_STEP)
}

fn set_step(project: &mut Project, target: &Target, tick: u32, on: bool) -> Option<Invocation> {
    let channel = project.channels[target.channel].id;
    let pattern = &mut project.patterns[target.pattern];
    let existing: Vec<Note> = pattern
        .notes_of(channel)
        .filter(|n| n.start == tick)
        .copied()
        .collect();
    match (on, existing.is_empty()) {
        (true, true) => {
            pattern.put_note(Note {
                props: musiccraft_project::NoteProps::default(),
                slide: false,
                channel,
                start: tick,
                length: TICKS_PER_STEP,
                key: STEP_KEY,
                velocity: DEFAULT_VELOCITY,
                pan: 0,
            });
            Some(target.remove_inverse(tick, STEP_KEY))
        }
        (false, false) => {
            pattern
                .notes
                .retain(|n| !(n.channel == channel && n.start == tick));
            Some(batch(
                existing.iter().map(|n| target.add_inverse(n)).collect(),
            ))
        }
        _ => None,
    }
}

fn step_toggle(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let target = Target::resolve(project, args)?;
    let tick = step_tick(args, 1)?;
    let channel = project.channels[target.channel].id;
    let on = !project.patterns[target.pattern]
        .notes_of(channel)
        .any(|n| n.start == tick);
    let inverse = set_step(project, &target, tick, on);
    let message = format!(
        "channel {} step {} {}",
        target.channel + 1,
        tick / TICKS_PER_STEP,
        if on { "on" } else { "off" }
    );
    Ok(match inverse {
        Some(inverse) => Outcome::undoable(message, inverse),
        None => Outcome::done(message),
    })
}

fn step_set(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let target = Target::resolve(project, args)?;
    let tick = step_tick(args, 1)?;
    let on = args.flag(2)?;
    let inverse = set_step(project, &target, tick, on);
    let message = format!(
        "channel {} step {} {}",
        target.channel + 1,
        tick / TICKS_PER_STEP,
        if on { "on" } else { "off" }
    );
    Ok(match inverse {
        Some(inverse) => Outcome::undoable(message, inverse),
        None => Outcome::done(message),
    })
}

fn add(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(4, 7)?;
    let target = Target::resolve(project, args)?;
    let pan = match args.opt(5) {
        Some(_) => pan_arg(args, 5)?,
        None => 0,
    };
    let slide = match args.opt(6) {
        None => false,
        Some("slide") => true,
        Some(other) => return Err(args.invalid(format!("`{other}` is not `slide`"))),
    };
    let note = Note {
        props: musiccraft_project::NoteProps::default(),
        slide,
        channel: project.channels[target.channel].id,
        start: args.time(1, TICKS_PER_STEP)?,
        length: args.duration(2, TICKS_PER_STEP)?,
        key: args.key(3)?,
        velocity: args.velocity(4, DEFAULT_VELOCITY)?,
        pan,
    };
    let replaced = project.patterns[target.pattern].put_note(note);
    let inverse = match replaced {
        Some(old) => target.add_inverse(&old),
        None => target.remove_inverse(note.start, note.key),
    };
    Ok(Outcome::undoable(
        format!(
            "added {} at {}t to channel {}",
            key_name(note.key),
            note.start,
            target.channel + 1
        ),
        inverse,
    ))
}

/// Finds the note named by arguments `start_arg` and `start_arg + 1`.
fn find(
    project: &Project,
    args: &Args<'_>,
    target: &Target,
    start_arg: usize,
) -> Result<usize, CommandError> {
    let start = args.time(start_arg, TICKS_PER_STEP)?;
    let key = args.key(start_arg + 1)?;
    let channel = project.channels[target.channel].id;
    project.patterns[target.pattern]
        .find_note(channel, start, key)
        .ok_or_else(|| {
            args.invalid(format!(
                "channel {} has no {} note at {start}t",
                target.channel + 1,
                key_name(key)
            ))
        })
}

fn remove(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(3, 3)?;
    let target = Target::resolve(project, args)?;
    let index = find(project, args, &target, 1)?;
    let note = project.patterns[target.pattern].notes.remove(index);
    Ok(Outcome::undoable(
        format!("removed {} at {}t", key_name(note.key), note.start),
        target.add_inverse(&note),
    ))
}

fn move_note(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(5, 5)?;
    let target = Target::resolve(project, args)?;
    let index = find(project, args, &target, 1)?;
    let new_start = args.time(3, TICKS_PER_STEP)?;
    let new_key = args.key(4)?;
    let pattern = &mut project.patterns[target.pattern];
    let old = pattern.notes.remove(index);
    let moved = Note {
        start: new_start,
        key: new_key,
        ..old
    };
    let replaced = pattern.put_note(moved);
    let mut inverses = vec![target.remove_inverse(new_start, new_key)];
    inverses.extend(replaced.map(|n| target.add_inverse(&n)));
    inverses.push(target.add_inverse(&old));
    Ok(Outcome::undoable(
        format!("moved note to {} at {new_start}t", key_name(new_key)),
        batch(inverses),
    ))
}

fn edit_note(
    project: &mut Project,
    args: &Args<'_>,
    change: impl FnOnce(&mut Note) -> Result<String, CommandError>,
) -> Result<Outcome, CommandError> {
    let target = Target::resolve(project, args)?;
    let index = find(project, args, &target, 1)?;
    let note = &mut project.patterns[target.pattern].notes[index];
    let old = *note;
    let message = change(note)?;
    Ok(Outcome::undoable(message, target.add_inverse(&old)))
}

fn resize(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(4, 4)?;
    let length = args.duration(3, TICKS_PER_STEP)?;
    edit_note(project, args, |note| {
        note.length = length;
        Ok(format!("note length {length}t"))
    })
}

fn slice(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(4, 4)?;
    let at = args.time(3, TICKS_PER_STEP)?;
    let target = Target::resolve(project, args)?;
    let index = find(project, args, &target, 1)?;
    let pattern = &mut project.patterns[target.pattern];
    let old = pattern.notes[index];
    if at <= old.start || at >= old.start + old.length {
        return Err(args.invalid("cut inside the note, not at its edge"));
    }
    pattern.notes[index].length = at - old.start;
    pattern.put_note(Note {
        start: at,
        length: old.start + old.length - at,
        ..old
    });
    Ok(Outcome::undoable(
        format!("sliced {} at {at}t", key_name(old.key)),
        Invocation::new(
            crate::BATCH,
            [
                target.remove_inverse(at, old.key).to_string(),
                target.add_inverse(&old).to_string(),
            ],
        ),
    ))
}

fn set_velocity(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(4, 4)?;
    let velocity: u8 = args.parse(3, "velocity")?;
    if !(1..=127).contains(&velocity) {
        return Err(args.invalid("velocity must be 1 to 127"));
    }
    edit_note(project, args, |note| {
        note.velocity = velocity;
        Ok(format!("note velocity {velocity}"))
    })
}

/// Replaces a channel's notes in the target pattern with the result of `f`.
pub(super) fn rewrite(
    project: &mut Project,
    target: &Target,
    f: impl FnOnce(Vec<Note>) -> Vec<Note>,
) -> (Vec<Note>, usize) {
    let channel = project.channels[target.channel].id;
    let pattern = &mut project.patterns[target.pattern];
    let (old, kept): (Vec<Note>, Vec<Note>) =
        pattern.notes.drain(..).partition(|n| n.channel == channel);
    pattern.notes = kept;
    let new = f(old.clone());
    let count = new.len();
    for note in new {
        pattern.put_note(Note { channel, ..note });
    }
    (old, count)
}

fn clear(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let target = Target::resolve(project, args)?;
    let (old, _) = rewrite(project, &target, |_| Vec::new());
    Ok(Outcome::undoable(
        format!("cleared {} notes", old.len()),
        target.set_all_inverse(&old),
    ))
}

fn set_all(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let target = Target::resolve(project, args)?;
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(args.str(1)?).map_err(|e| args.invalid(e.to_string()))?;
    let channel = project.channels[target.channel].id;
    let mut notes = Vec::with_capacity(rows.len());
    for row in rows {
        let note = match row {
            serde_json::Value::Array(values) => {
                let n: Vec<i64> = values
                    .iter()
                    .filter_map(serde_json::Value::as_i64)
                    .collect();
                let (start, length, key, velocity, pan, slide) = match n.as_slice() {
                    [s, l, k, v] => (*s, *l, *k, *v, 0, false),
                    [s, l, k, v, p] => (*s, *l, *k, *v, *p, false),
                    [s, l, k, v, p, slide] => (*s, *l, *k, *v, *p, *slide != 0),
                    _ => {
                        return Err(args.invalid(
                            "each note is [start, length, key, velocity, pan?, slide?] or an object",
                        ));
                    }
                };
                let json = NoteJson {
                    start: u32::try_from(start).unwrap_or(u32::MAX),
                    length: u32::try_from(length).unwrap_or(0),
                    key: u8::try_from(key).unwrap_or(255),
                    velocity: u8::try_from(velocity).unwrap_or(0),
                    pan: i8::try_from(pan).unwrap_or(i8::MAX),
                    slide,
                    props: musiccraft_project::NoteProps::default(),
                };
                json.note(channel)
            }
            other => serde_json::from_value::<NoteJson>(other)
                .map_err(|e| args.invalid(e.to_string()))?
                .note(channel),
        };
        notes.push(note.ok_or_else(|| {
            args.invalid("a note value is out of range (length > 0, key 0-127, velocity 1-127, pan -100..100)")
        })?);
    }
    let (old, count) = rewrite(project, &target, |_| notes);
    Ok(Outcome::undoable(
        format!("channel {} now has {count} notes", target.channel + 1),
        target.set_all_inverse(&old),
    ))
}

fn transpose(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let target = Target::resolve(project, args)?;
    let semitones: i32 = args.parse(1, "semitone count")?;
    if !(-127..=127).contains(&semitones) {
        return Err(args.invalid("transpose by at most 127 semitones"));
    }
    let channel = project.channels[target.channel].id;
    let out_of_range = project.patterns[target.pattern]
        .notes_of(channel)
        .any(|n| !(0..=127).contains(&(i32::from(n.key) + semitones)));
    if out_of_range {
        return Err(args.invalid("transposing would move notes outside keys 0-127"));
    }
    let (old, count) = rewrite(project, &target, |notes| {
        notes
            .into_iter()
            .map(|n| Note {
                key: u8::try_from(i32::from(n.key) + semitones).unwrap_or(n.key),
                ..n
            })
            .collect()
    });
    Ok(Outcome::undoable(
        format!("transposed {count} notes by {semitones}"),
        target.set_all_inverse(&old),
    ))
}

fn quantize(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let target = Target::resolve(project, args)?;
    let grid = args.duration(1, TICKS_PER_STEP)?;
    let (old, count) = rewrite(project, &target, |notes| {
        notes
            .into_iter()
            .map(|n| Note {
                start: (n.start + grid / 2) / grid * grid,
                ..n
            })
            .collect()
    });
    Ok(Outcome::undoable(
        format!("quantized {count} notes to {grid}t"),
        target.set_all_inverse(&old),
    ))
}

fn import_midi(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let target = Target::resolve(project, args)?;
    let path = args.str(1)?;
    let data =
        std::fs::read(path).map_err(|e| CommandError::Io(format!("cannot read {path}: {e}")))?;
    let file = crate::midi_file::parse(&data).map_err(|e| args.invalid(format!("{path}: {e}")))?;
    let ppq = u64::from(musiccraft_project::params::PPQ);
    let division = u64::from(file.ticks_per_quarter);
    let convert =
        |ticks: u64| u32::try_from((ticks * ppq + division / 2) / division).unwrap_or(u32::MAX / 2);
    let channel = project.channels[target.channel].id;
    let imported: Vec<Note> = file
        .notes
        .iter()
        .map(|n| Note {
            props: musiccraft_project::NoteProps::default(),
            slide: false,
            channel,
            start: convert(n.start),
            length: convert(n.length).max(1),
            key: n.key.min(127),
            velocity: n.velocity.clamp(1, 127),
            pan: 0,
        })
        .collect();
    let count = imported.len();
    // The file's signature becomes the pattern's.
    let sig = file
        .time_signature
        .and_then(|(num, den)| musiccraft_project::TimeSig::new(num, den))
        .filter(|sig| *sig != project.patterns[target.pattern].time_signature);
    let meter_inverse = match sig {
        Some(_) => Some(super::meter::snapshot(project, args)?),
        None => None,
    };
    if let Some(sig) = sig {
        project.patterns[target.pattern].set_time_signature(sig);
    }
    let (old, _) = rewrite(project, &target, |mut notes| {
        notes.extend(imported);
        notes
    });
    let notes_inverse = target.set_all_inverse(&old);
    let inverse = match meter_inverse {
        Some(meter) => {
            Invocation::new(crate::BATCH, [notes_inverse.to_string(), meter.to_string()])
        }
        None => notes_inverse,
    };
    let message = format!("imported {count} notes into channel {}", target.channel + 1);
    Ok(Outcome::undoable(
        match sig {
            Some(sig) => format!("{message}, pattern in {sig}"),
            None => message,
        },
        inverse,
    ))
}

fn add_chord(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    use musiccraft_project::music::{chord, chords};
    args.expect(5, 6)?;
    let target = Target::resolve(project, args)?;
    let start = args.time(1, TICKS_PER_STEP)?;
    let length = args.duration(2, TICKS_PER_STEP)?;
    let root = args.key(3)?;
    let quality = args.str(4)?;
    let intervals = chord(quality).ok_or_else(|| {
        let names: Vec<_> = chords().map(|(n, _)| n).collect();
        args.invalid(format!(
            "unknown chord `{quality}`; choose from {}",
            names.join(", ")
        ))
    })?;
    let velocity = args.velocity(5, DEFAULT_VELOCITY)?;
    if intervals
        .iter()
        .any(|i| u16::from(root) + u16::from(*i) > 127)
    {
        return Err(args.invalid("the chord goes above key 127"));
    }
    let channel = project.channels[target.channel].id;
    let added: Vec<Note> = intervals
        .iter()
        .map(|i| Note {
            props: musiccraft_project::NoteProps::default(),
            slide: false,
            channel,
            start,
            length,
            key: root + i,
            velocity,
            pan: 0,
        })
        .collect();
    let (old, _) = rewrite(project, &target, |mut notes| {
        notes.extend(added);
        notes
    });
    Ok(Outcome::undoable(
        format!("added {} {quality} chord at {start}t", key_name(root)),
        target.set_all_inverse(&old),
    ))
}

fn pan_arg(args: &Args<'_>, i: usize) -> Result<i8, CommandError> {
    let pan: i8 = args.parse(i, "pan")?;
    if (-100..=100).contains(&pan) {
        Ok(pan)
    } else {
        Err(args.invalid("pan must be -100 to 100"))
    }
}

fn restore(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(2, 2)?;
    let target = Target::resolve(project, args)?;
    let channel = project.channels[target.channel].id;
    let note = serde_json::from_str::<NoteJson>(args.str(1)?)
        .map_err(|e| args.invalid(e.to_string()))?
        .note(channel)
        .ok_or_else(|| args.invalid("a note value is out of range"))?;
    let replaced = project.patterns[target.pattern].put_note(note);
    let inverse = match replaced {
        Some(old) => target.add_inverse(&old),
        None => target.remove_inverse(note.start, note.key),
    };
    Ok(Outcome::undoable(
        format!("restored {} at {}t", key_name(note.key), note.start),
        inverse,
    ))
}

fn set_prop(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(5, 5)?;
    let prop = args.str(3)?;
    let raw = args.str(4)?;
    let number = |min: i32, max: i32| -> Result<i32, CommandError> {
        let v: i32 = args.parse(4, prop)?;
        if (min..=max).contains(&v) {
            Ok(v)
        } else {
            Err(args.invalid(format!("{prop} must be {min} to {max}")))
        }
    };
    type Change = Box<dyn FnOnce(&mut Note)>;
    let change: Change = match prop {
        "release" => {
            let v = number(0, 127)? as u8;
            Box::new(move |n| n.props.release = v)
        }
        "fine" => {
            let v = number(-100, 100)? as i8;
            Box::new(move |n| n.props.fine = v)
        }
        "mod_x" => {
            let v = number(0, 127)? as u8;
            Box::new(move |n| n.props.mod_x = v)
        }
        "mod_y" => {
            let v = number(0, 127)? as u8;
            Box::new(move |n| n.props.mod_y = v)
        }
        "color" => {
            let v = (number(1, 16)? - 1) as u8;
            Box::new(move |n| n.props.color = v)
        }
        "porta" => {
            let v = args.flag(4)?;
            Box::new(move |n| n.props.porta = v)
        }
        "muted" => {
            let v = args.flag(4)?;
            Box::new(move |n| n.props.muted = v)
        }
        other => {
            return Err(args.invalid(format!(
                "unknown property `{other}`; choose from release, fine, mod_x, mod_y, color, porta, muted"
            )));
        }
    };
    edit_note(project, args, |note| {
        change(note);
        Ok(format!("note {prop} {raw}"))
    })
}

fn set_slide(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(4, 4)?;
    let on = args.flag(3)?;
    edit_note(project, args, |note| {
        note.slide = on;
        Ok(format!("slide {}", if on { "on" } else { "off" }))
    })
}

fn set_pan(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(4, 4)?;
    let pan = pan_arg(args, 3)?;
    edit_note(project, args, |note| {
        note.pan = pan;
        Ok(format!("note pan {pan}"))
    })
}
