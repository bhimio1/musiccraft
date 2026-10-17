//! `sample.edit`: offline edits of a channel's sample, written to a new file
//! beside the project (see `docs/spec/sample-editor.md`).

use std::path::Path;

use musiccraft_commands::{Command, CommandError, Invocation, Outcome};
use musiccraft_dsp::ProcessContext;
use musiccraft_dsp::edit::{pitch_shift, stretch};
use musiccraft_dsp::pitch::PitchCorrector;
use musiccraft_project::{Instrument, Project};
use musiccraft_samples::{SampleCache, WavFormat, write_wav};

/// `sample.edit <channel> <operation> [arguments]`.
#[derive(Default)]
pub struct SampleEditCommand;

/// Rate built-in sounds are made at when edited.
const BUILTIN_RATE: u32 = 48_000;

/// A stereo sample being edited.
struct Audio {
    left: Vec<f32>,
    right: Vec<f32>,
    rate: f64,
}

impl Audio {
    fn len(&self) -> usize {
        self.left.len().min(self.right.len())
    }

    /// The sample index of `seconds`, clamped to the sample.
    fn at(&self, seconds: f64) -> usize {
        ((seconds.max(0.0) * self.rate).round() as usize).min(self.len())
    }

    fn sides(&mut self) -> [&mut Vec<f32>; 2] {
        [&mut self.left, &mut self.right]
    }
}

fn db_to_gain(db: f64) -> f32 {
    10.0_f64.powf(db / 20.0) as f32
}

impl SampleEditCommand {
    fn invalid(&self, reason: impl Into<String>) -> CommandError {
        CommandError::InvalidArgument {
            command: self.name().to_owned(),
            reason: reason.into(),
        }
    }

    fn number(
        &self,
        args: &[String],
        i: usize,
        what: &str,
        min: f64,
        max: f64,
    ) -> Result<f64, CommandError> {
        let raw = args.get(i).ok_or_else(|| self.usage_error())?;
        let value: f64 = raw
            .parse()
            .ok()
            .filter(|v: &f64| v.is_finite())
            .ok_or_else(|| self.invalid(format!("`{raw}` is not a valid {what}")))?;
        if value < min || value > max {
            return Err(self.invalid(format!("{what} must be {min} to {max}")));
        }
        Ok(value)
    }

    fn usage_error(&self) -> CommandError {
        CommandError::Usage {
            command: self.name().to_owned(),
            usage: self.usage().to_owned(),
        }
    }

    /// A `start end` range from `args[i..]`, or the whole sample if absent.
    fn range(
        &self,
        audio: &Audio,
        args: &[String],
        i: usize,
        required: bool,
    ) -> Result<(usize, usize), CommandError> {
        if args.len() <= i && !required {
            return Ok((0, audio.len()));
        }
        let start = self.number(args, i, "start", 0.0, f64::MAX)?;
        let end = self.number(args, i + 1, "end", 0.0, f64::MAX)?;
        let (a, b) = (audio.at(start), audio.at(end));
        if a >= b {
            return Err(self.invalid("the range must start before it ends, inside the sample"));
        }
        Ok((a, b))
    }

    /// Applies `operation` to `audio`.
    fn apply(
        &self,
        audio: &mut Audio,
        operation: &str,
        args: &[String],
    ) -> Result<(), CommandError> {
        match operation {
            "trim" => {
                let (a, b) = self.range(audio, args, 2, true)?;
                for side in audio.sides() {
                    *side = side.get(a..b).unwrap_or_default().to_vec();
                }
            }
            "cut" => {
                let (a, b) = self.range(audio, args, 2, true)?;
                for side in audio.sides() {
                    side.drain(a..b.min(side.len()));
                }
            }
            "silence" => {
                let (a, b) = self.range(audio, args, 2, true)?;
                for side in audio.sides() {
                    side.iter_mut().take(b).skip(a).for_each(|s| *s = 0.0);
                }
            }
            "fade_in" | "fade_out" => {
                let (a, b) = self.range(audio, args, 2, true)?;
                let span = (b - a) as f32;
                let fade_in = operation == "fade_in";
                for side in audio.sides() {
                    for (i, s) in side.iter_mut().enumerate() {
                        let t = ((i as f32 - a as f32) / span).clamp(0.0, 1.0);
                        *s *= if fade_in { t } else { 1.0 - t };
                    }
                }
            }
            "gain" => {
                let gain = db_to_gain(self.number(args, 2, "gain", -48.0, 24.0)?);
                let (a, b) = self.range(audio, args, 3, false)?;
                for side in audio.sides() {
                    side.iter_mut().take(b).skip(a).for_each(|s| *s *= gain);
                }
            }
            "normalize" => {
                let target = if args.len() > 2 {
                    self.number(args, 2, "peak", -24.0, 0.0)?
                } else {
                    0.0
                };
                let peak = audio
                    .left
                    .iter()
                    .chain(&audio.right)
                    .fold(0.0_f32, |m, s| m.max(s.abs()));
                if peak <= 1e-9 {
                    return Err(self.invalid("the sample is silent"));
                }
                let gain = db_to_gain(target) / peak;
                for side in audio.sides() {
                    side.iter_mut().for_each(|s| *s *= gain);
                }
            }
            "reverse" => {
                let (a, b) = self.range(audio, args, 2, false)?;
                for side in audio.sides() {
                    if let Some(range) = side.get_mut(a..b) {
                        range.reverse();
                    }
                }
            }
            "stretch" => {
                let ratio = self.number(args, 2, "ratio", 0.25, 4.0)?;
                let (l, r) = stretch(&audio.left, &audio.right, ratio);
                audio.left = l;
                audio.right = r;
            }
            "pitch" => {
                let semitones = self.number(args, 2, "semitones", -24.0, 24.0)?;
                let (l, r) = pitch_shift(&audio.left, &audio.right, semitones);
                audio.left = l;
                audio.right = r;
            }
            "tune" => {
                let key = self.number(args, 2, "key", 0.0, 11.0)?.round();
                let scale = self.number(args, 3, "scale", 0.0, 5.0)?.round();
                let speed = if args.len() > 4 {
                    self.number(args, 4, "speed", 0.0, 400.0)?
                } else {
                    0.0
                };
                let mut corrector = PitchCorrector::new(audio.rate as f32);
                let params = [key as f32, scale as f32, speed as f32, 1.0, 440.0, 1.0];
                let ctx = ProcessContext {
                    sample_rate: audio.rate as f32,
                    tempo_bpm: 120.0,
                    beats: 0.0,
                };
                for (l, r) in audio.left.chunks_mut(256).zip(audio.right.chunks_mut(256)) {
                    corrector.process(l, r, &params, ctx);
                }
            }
            other => {
                return Err(self.invalid(format!(
                    "unknown operation `{other}`; choose trim, cut, silence, fade_in, fade_out, gain, normalize, reverse, stretch, pitch or tune"
                )));
            }
        }
        if audio.len() == 0 {
            return Err(self.invalid("the edit would leave nothing"));
        }
        Ok(())
    }
}

/// A free `edited/<stem>-<n>.wav` path in `dir`, and the same relative to it.
fn free_path(dir: &Path, stem: &str) -> (std::path::PathBuf, String) {
    let folder = dir.join("edited");
    (1..)
        .map(|n| format!("{stem}-{n}.wav"))
        .find(|name| !folder.join(name).exists())
        .map(|name| (folder.join(&name), format!("edited/{name}")))
        .unwrap_or_else(|| {
            (
                folder.join(format!("{stem}.wav")),
                format!("edited/{stem}.wav"),
            )
        })
}

impl Command for SampleEditCommand {
    fn name(&self) -> &'static str {
        "sample.edit"
    }

    fn summary(&self) -> &'static str {
        "Edit a channel's sample offline (trim, cut, silence, fade_in, fade_out, gain, normalize, reverse, stretch, pitch, tune) into a new file."
    }

    fn usage(&self) -> &'static str {
        "sample.edit <channel> <operation> [arguments]"
    }

    fn execute(&self, project: &mut Project, args: &[String]) -> Result<Outcome, CommandError> {
        let (Some(channel), Some(operation)) = (args.first(), args.get(1)) else {
            return Err(self.usage_error());
        };
        let number: usize = channel
            .parse()
            .map_err(|_| self.invalid(format!("`{channel}` is not a channel number")))?;
        let index = project.channel_index(number)?;
        let sample = match &project.channels[index].instrument {
            Instrument::Sampler { sample }
            | Instrument::Granular { sample }
            | Instrument::Resynth { sample } => sample.clone(),
            _ => return Err(self.invalid(format!("channel {number} does not play a sample"))),
        };
        let dir = project
            .path()
            .and_then(Path::parent)
            .ok_or_else(|| {
                self.invalid("save the project first: edited audio is written beside it")
            })?
            .to_path_buf();
        // A path from another system reads the file it translates to.
        let source = musiccraft_paths::Resolver::new()
            .resolve(&sample, Some(&dir))
            .map_or_else(|| sample.clone(), |p| p.to_string_lossy().into_owned());
        let buffer = SampleCache::new()
            .get(&source, Some(&dir), BUILTIN_RATE)
            .map_err(|e| self.invalid(format!("cannot read `{sample}`: {e}")))?;
        let mut audio = Audio {
            left: buffer.left().to_vec(),
            right: buffer.right().to_vec(),
            rate: buffer.sample_rate(),
        };
        self.apply(&mut audio, operation, args)?;
        let stem: String =
            Path::new(sample.trim_start_matches(musiccraft_project::params::BUILTIN_PREFIX))
                .file_stem()
                .map_or_else(|| "sample".to_owned(), |s| s.to_string_lossy().into_owned())
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '-'
                    }
                })
                .collect();
        let (path, relative) = free_path(&dir, &stem);
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(|e| CommandError::Io(e.to_string()))?;
        }
        write_wav(
            &path,
            &audio.left,
            &audio.right,
            audio.rate.round() as u32,
            WavFormat::Float32,
        )
        .map_err(|e| CommandError::Io(e.to_string()))?;
        match &mut project.channels[index].instrument {
            Instrument::Sampler { sample: s }
            | Instrument::Granular { sample: s }
            | Instrument::Resynth { sample: s } => *s = relative.clone(),
            _ => {}
        }
        Ok(Outcome::undoable(
            format!("channel {number}: {operation}, now plays {relative}"),
            Invocation::new("channel.set_sample", [number.to_string(), sample]),
        ))
    }
}
