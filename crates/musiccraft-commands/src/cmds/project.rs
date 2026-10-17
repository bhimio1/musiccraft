use std::fs;
use std::path::{Path, PathBuf};

use musiccraft_project::{PanLaw, Project};

use crate::args::Args;
use crate::command::cmd;
use crate::{Command, CommandError, Invocation, Outcome};

pub(super) fn commands() -> Vec<Box<dyn Command>> {
    vec![
        cmd(
            "project.set_tempo",
            "project.set_tempo <bpm>",
            "Set the project tempo in beats per minute (undoable).",
            set_tempo,
        ),
        cmd(
            "project.set_swing",
            "project.set_swing <0-100>",
            "Set the project swing in percent; each channel's swing_mix scales it (undoable).",
            set_swing,
        ),
        cmd(
            "project.set_master_pitch",
            "project.set_master_pitch <cents>",
            "Transpose every built-in instrument by -1200..1200 cents (undoable).",
            set_master_pitch,
        ),
        cmd(
            "project.set_a4",
            "project.set_a4 <hz>",
            "Tune A4 to a frequency (392-494 Hz), as a master pitch (undoable).",
            set_a4,
        ),
        cmd(
            "project.set_pan_law",
            "project.set_pan_law <circular|triangular>",
            "Choose how channel and mixer pans split left and right (undoable).",
            set_pan_law,
        ),
        cmd(
            "project.rename",
            "project.rename <name>",
            "Rename the project.",
            rename,
        ),
        cmd(
            "project.new",
            "project.new",
            "Replace the project with an empty one (clears undo history).",
            new,
        ),
        cmd(
            "project.save",
            "project.save [path]",
            "Save the project as JSON (default: the file it came from).",
            save,
        ),
        cmd(
            "project.save_new_version",
            "project.save_new_version",
            "Save a copy as the next numbered version (song.mcproj -> song_2.mcproj) and keep working in it.",
            save_new_version,
        ),
        cmd(
            "project.load",
            "project.load <path>",
            "Load a project file (clears undo history).",
            load,
        ),
        cmd(
            "project.import_flp",
            "project.import_flp <file.flp>",
            "Replace the project with one converted from an FL Studio project (clears undo history).",
            import_flp,
        ),
        cmd(
            "project.recover",
            "project.recover <path>",
            "Load a crash backup as unsaved work: saving asks for a file (clears undo history).",
            recover,
        ),
    ]
}

fn set_tempo(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let raw = args.str(0)?;
    let bpm: f64 = raw
        .parse()
        .map_err(|_| args.invalid(format!("`{raw}` is not a number")))?;
    let previous = project.set_tempo_bpm(bpm)?;
    let message = format!("tempo {bpm} BPM");
    if (previous - bpm).abs() < f64::EPSILON {
        Ok(Outcome::done(message))
    } else {
        Ok(Outcome::undoable(
            message,
            Invocation::new(args.name(), [previous.to_string()]),
        ))
    }
}

fn set_swing(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let percent = musiccraft_project::check_range("swing", args.f32(0, "swing")?, 0.0, 100.0)?;
    let swing = percent / 100.0;
    let previous = std::mem::replace(&mut project.swing, swing);
    let message = format!("swing {percent}%");
    if (previous - swing).abs() < f32::EPSILON {
        Ok(Outcome::done(message))
    } else {
        Ok(Outcome::undoable(
            message,
            Invocation::new(args.name(), [(previous * 100.0).to_string()]),
        ))
    }
}

fn master_pitch_outcome(project: &mut Project, cents: f32) -> Outcome {
    let cents = (cents * 100.0).round() / 100.0;
    let previous = std::mem::replace(&mut project.master_pitch, cents);
    let a4 = 440.0 * 2f64.powf(f64::from(cents) / 1200.0);
    let message = format!("master pitch {cents} cents (A4 = {a4:.2} Hz)");
    if (previous - cents).abs() < f32::EPSILON {
        Outcome::done(message)
    } else {
        Outcome::undoable(
            message,
            Invocation::new("project.set_master_pitch", [previous.to_string()]),
        )
    }
}

fn set_master_pitch(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let cents =
        musiccraft_project::check_range("master pitch", args.f32(0, "cents")?, -1200.0, 1200.0)?;
    Ok(master_pitch_outcome(project, cents))
}

fn set_a4(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let hz = musiccraft_project::check_range("A4", args.f32(0, "hz")?, 392.0, 494.0)?;
    let cents = 1200.0 * (f64::from(hz) / 440.0).log2();
    Ok(master_pitch_outcome(project, cents as f32))
}

fn set_pan_law(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let raw = args.str(0)?;
    let law = PanLaw::from_label(raw)
        .ok_or_else(|| args.invalid(format!("`{raw}` is not circular or triangular")))?;
    let previous = std::mem::replace(&mut project.pan_law, law);
    let message = format!("{} pan law", law.label());
    if previous == law {
        Ok(Outcome::done(message))
    } else {
        Ok(Outcome::undoable(
            message,
            Invocation::new(args.name(), [previous.label().to_owned()]),
        ))
    }
}

fn rename(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    let name = args.rest(0)?;
    let previous = project.set_name(name.clone());
    Ok(Outcome::undoable(
        format!("project renamed to {name}"),
        Invocation::new(args.name(), [previous]),
    ))
}

fn new(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(0, 0)?;
    *project = Project::default();
    Ok(Outcome::reset("new project"))
}

fn save(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(0, 1)?;
    let path = match args.opt(0) {
        Some(path) => PathBuf::from(path),
        None => project
            .path()
            .map(PathBuf::from)
            .ok_or_else(|| args.invalid("the project has never been saved; give a path"))?,
    };
    write_project(project, &path)?;
    Ok(Outcome::done(format!("saved {}", path.display())))
}

fn save_new_version(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(0, 0)?;
    let current = project
        .path()
        .map(PathBuf::from)
        .ok_or_else(|| args.invalid("the project has never been saved; save it first"))?;
    let path = next_version_path(&current, |p| p.exists());
    write_project(project, &path)?;
    Ok(Outcome::done(format!(
        "saved new version {}",
        path.display()
    )))
}

/// Writes atomically (temporary file, then rename) and adopts `path`.
fn write_project(project: &mut Project, path: &Path) -> Result<(), CommandError> {
    let json = project.to_json()?;
    let temp = path.with_extension("mcproj.tmp");
    fs::write(&temp, json)
        .and_then(|()| fs::rename(&temp, path))
        .map_err(|e| CommandError::Io(format!("cannot save {}: {e}", path.display())))?;
    let absolute = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    project.set_path(Some(absolute));
    Ok(())
}

/// The first free numbered sibling of `path`: `song.mcproj` becomes
/// `song_2.mcproj`, `song_2.mcproj` becomes `song_3.mcproj`, skipping names
/// for which `exists` is true.
pub(crate) fn next_version_path(path: &Path, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_owned());
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_else(|| "mcproj".to_owned());
    let (base, mut number) = match stem.rsplit_once('_') {
        Some((base, n)) if !base.is_empty() => match n.parse::<u32>() {
            Ok(n) => (base.to_owned(), n.saturating_add(1)),
            Err(_) => (stem.clone(), 2),
        },
        _ => (stem.clone(), 2),
    };
    loop {
        let candidate = path.with_file_name(format!("{base}_{number}.{extension}"));
        if !exists(&candidate) || number == u32::MAX {
            return candidate;
        }
        number += 1;
    }
}

fn load(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let path = PathBuf::from(args.str(0)?);
    let text = fs::read_to_string(&path)
        .map_err(|e| CommandError::Io(format!("cannot read {}: {e}", path.display())))?;
    let mut loaded = Project::from_json(&text)?;
    loaded.set_path(Some(
        fs::canonicalize(&path).unwrap_or_else(|_| path.clone()),
    ));
    *project = loaded;
    Ok(Outcome::reset(format!("loaded {}", path.display())))
}

fn import_flp(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let path = PathBuf::from(args.str(0)?);
    let bytes = fs::read(&path)
        .map_err(|e| CommandError::Io(format!("cannot read {}: {e}", path.display())))?;
    let stem = path.file_stem().map_or_else(
        || "Imported".to_owned(),
        |s| s.to_string_lossy().into_owned(),
    );
    let (imported, report) = crate::flp_import::convert(&bytes, &stem)
        .map_err(|reason| args.invalid(format!("cannot import {}: {reason}", path.display())))?;
    *project = imported;
    // Samples named for another system resolve now, from the file's folder;
    // the import keeps the local paths it finds.
    let localized = super::sample_paths::localize(project, path.parent());
    let mut message = format!(
        "imported {}: {} channels, {} patterns, {} clips, {} mixer tracks",
        path.display(),
        project.channels.len(),
        project.patterns.len(),
        project.playlist.clips.len(),
        project.mixer.len()
    );
    if !report.replaced.is_empty() {
        message.push_str(&format!("; replaced {}", report.replaced.join(", ")));
    }
    if !report.dropped.is_empty() {
        message.push_str(&format!("; not imported: {}", report.dropped.join("; ")));
    }
    let samples = super::sample_paths::summary(&localized);
    if !samples.is_empty() {
        message.push_str(&format!("; {samples}"));
    }
    message.push_str("; volumes and pans keep FL's fader positions, not its loudness");
    Ok(Outcome::reset(message))
}

fn recover(project: &mut Project, args: &Args<'_>) -> Result<Outcome, CommandError> {
    args.expect(1, 1)?;
    let path = PathBuf::from(args.str(0)?);
    let text = fs::read_to_string(&path)
        .map_err(|e| CommandError::Io(format!("cannot read {}: {e}", path.display())))?;
    let mut recovered = Project::from_json(&text)?;
    // A backup is not where the work belongs, so saving must ask for a file
    // instead of quietly writing into the app's data folder.
    recovered.set_path(None);
    *project = recovered;
    Ok(Outcome::reset(format!(
        "recovered unsaved work from {}; save it to keep it",
        path.display()
    )))
}
