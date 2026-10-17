use std::path::Path;

use musiccraft_commands::{Command, CommandError, Outcome, Registry};
use musiccraft_engine::{EngineMessage, MAX_BLOCK, Sequence, engine_pair};
use musiccraft_params::PPQ;
use musiccraft_project::{Project, Transport};
use musiccraft_samples::{WavFormat, write_wav};

use crate::{Bridge, PluginHost, build_full_sequence};

/// Settings for an offline render.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    /// Output sample rate in hertz.
    pub sample_rate: u32,
    /// How many times to play the loop (pattern or song).
    pub loops: u32,
    /// Longest tail to keep after the last loop, in seconds. Rendering stops
    /// earlier once the output has been silent for half a second.
    pub max_tail_seconds: f64,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            sample_rate: 44_100,
            loops: 1,
            max_tail_seconds: 10.0,
        }
    }
}

/// Level below which the tail counts as silent (about -90 dBFS).
const SILENCE: f32 = 3.2e-5;

/// Renders the project's loop (current pattern or song, by play mode) to stereo
/// audio, followed by the decaying tail of notes and effects.
///
/// Returns the left and right channels and any warnings (for example samples
/// that failed to load).
#[must_use]
pub fn render(project: &Project, options: RenderOptions) -> (Vec<f32>, Vec<f32>, Vec<String>) {
    render_with(project, options, None)
}

/// [`render`] with plugins from `host`; plugins get fresh instances restored
/// from the project.
#[must_use]
pub fn render_with(
    project: &Project,
    options: RenderOptions,
    host: Option<&PluginHost>,
) -> (Vec<f32>, Vec<f32>, Vec<String>) {
    let sample_rate = options.sample_rate.max(8_000);
    let (mut engine, mut handle) = engine_pair(sample_rate);
    let mut bridge = Bridge::with_plugins(sample_rate, host.cloned());
    let mut stopped = project.clone();
    stopped.set_transport(Transport::Stopped);
    // The click is a monitoring aid; it is never part of an export.
    stopped.metronome = false;
    let report = bridge.sync(&stopped, &mut handle);

    // Repeat the loop `loops` times, then leave room for the tail without
    // wrapping back to the start.
    let base = build_full_sequence(project);
    let loop_ticks = base.length().max(1);
    let loops = u64::from(options.loops.max(1));
    let mut events = Vec::new();
    for i in 0..loops {
        events.extend(base.events().iter().map(|e| musiccraft_engine::NoteEvent {
            start: e.start + i * loop_ticks,
            ..*e
        }));
    }
    let ticks_per_second = project.tempo_bpm() / 60.0 * f64::from(PPQ);
    let tail_ticks = (options.max_tail_seconds.max(0.0) * ticks_per_second).ceil() as u64 + 1;
    let total_ticks = loop_ticks * loops;
    let mut lanes = Vec::new();
    for i in 0..loops {
        lanes.extend(base.automation().iter().map(|lane| {
            musiccraft_engine::AutomationLane {
                target: lane.target,
                points: lane
                    .points
                    .iter()
                    .map(|(t, v)| (t + i * loop_ticks, *v))
                    .collect(),
            }
        }));
    }
    let sequence = Sequence::new(events, total_ticks + tail_ticks)
        .with_automation(lanes)
        .with_controllers(base.controllers().to_vec())
        .with_meter(base.meter().clone());
    let _ = handle.send(EngineMessage::SetSequence(Box::new(sequence)));
    // The sync above left the playhead at the project's own home, which is
    // a loop region's start when one is set; the export starts at 0.
    let _ = handle.send(EngineMessage::Seek(0.0));
    let _ = handle.send(EngineMessage::Play);

    let body_frames =
        (total_ticks as f64 / ticks_per_second * f64::from(sample_rate)).ceil() as usize;
    let max_tail_frames = (options.max_tail_seconds.max(0.0) * f64::from(sample_rate)) as usize;
    let silence_frames = sample_rate as usize / 2;

    let mut left = Vec::with_capacity(body_frames);
    let mut right = Vec::with_capacity(body_frames);
    let mut block = vec![0.0_f32; MAX_BLOCK * 2];
    let mut quiet = 0;
    while left.len() < body_frames + max_tail_frames {
        let remaining = body_frames + max_tail_frames - left.len();
        let frames = remaining.min(MAX_BLOCK);
        let out = &mut block[..frames * 2];
        engine.process(out, 2);
        handle.collect_garbage();
        for frame in out.chunks_exact(2) {
            left.push(frame[0]);
            right.push(frame[1]);
        }
        if left.len() > body_frames {
            let loud = out.iter().any(|s| s.abs() > SILENCE);
            quiet = if loud { 0 } else { quiet + frames };
            if quiet >= silence_frames {
                break;
            }
        }
    }
    (left, right, report.warnings)
}

/// Renders the project and writes it as a stereo WAV file.
///
/// # Errors
///
/// Returns a message if the file cannot be written.
pub fn export_wav(
    project: &Project,
    path: &Path,
    format: WavFormat,
    options: RenderOptions,
    host: Option<&PluginHost>,
) -> Result<(f64, Vec<String>), String> {
    let (left, right, warnings) = render_with(project, options, host);
    write_wav(path, &left, &right, options.sample_rate, format).map_err(|e| e.to_string())?;
    Ok((left.len() as f64 / f64::from(options.sample_rate), warnings))
}

/// `project.export_wav <path> [16|24|32f] [loops] [sample rate]`.
#[derive(Default)]
pub struct ExportCommand {
    /// Plugins to render with.
    pub host: Option<PluginHost>,
}

impl Command for ExportCommand {
    fn name(&self) -> &'static str {
        "project.export_wav"
    }

    fn summary(&self) -> &'static str {
        "Render the pattern or song (by play mode) to a WAV file."
    }

    fn usage(&self) -> &'static str {
        "project.export_wav <path> [16|24|32f] [loops] [sample rate]"
    }

    fn execute(&self, project: &mut Project, args: &[String]) -> Result<Outcome, CommandError> {
        let invalid = |reason: String| CommandError::InvalidArgument {
            command: self.name().to_owned(),
            reason,
        };
        let [path, rest @ ..] = args else {
            return Err(CommandError::Usage {
                command: self.name().to_owned(),
                usage: self.usage().to_owned(),
            });
        };
        if rest.len() > 3 {
            return Err(CommandError::Usage {
                command: self.name().to_owned(),
                usage: self.usage().to_owned(),
            });
        }
        let format = match rest.first() {
            Some(raw) => WavFormat::parse(raw)
                .ok_or_else(|| invalid(format!("`{raw}` is not 16, 24 or 32f")))?,
            None => WavFormat::Int16,
        };
        let loops = match rest.get(1) {
            Some(raw) => raw
                .parse::<u32>()
                .ok()
                .filter(|n| (1..=64).contains(n))
                .ok_or_else(|| invalid(format!("`{raw}` is not a loop count 1-64")))?,
            None => 1,
        };
        let sample_rate = match rest.get(2) {
            Some(raw) => raw
                .parse::<u32>()
                .ok()
                .filter(|n| (8_000..=192_000).contains(n))
                .ok_or_else(|| invalid(format!("`{raw}` is not a sample rate 8000-192000")))?,
            None => 44_100,
        };
        let options = RenderOptions {
            sample_rate,
            loops,
            ..RenderOptions::default()
        };
        let (seconds, warnings) = export_wav(
            project,
            Path::new(path),
            format,
            options,
            self.host.as_ref(),
        )
        .map_err(CommandError::Io)?;
        let mut message = format!("exported {seconds:.2} s to {path}");
        for warning in warnings {
            message.push_str(&format!(" (warning: {warning})"));
        }
        Ok(Outcome::done(message))
    }
}

/// Groups of channels exported as stems: one per mixer track that receives
/// channels, in track order. Returns (track, file stem, channel indices).
fn stem_groups(project: &Project) -> Vec<(usize, String, Vec<usize>)> {
    let mut groups = Vec::new();
    for (track_index, track) in project.mixer.iter().enumerate() {
        let channels: Vec<usize> = project
            .channels
            .iter()
            .enumerate()
            .filter(|(_, c)| c.mixer_track == track_index && !c.mute)
            .map(|(i, _)| i)
            .collect();
        if channels.is_empty() {
            continue;
        }
        let name: String = track
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == ' ' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        groups.push((
            track_index,
            format!("{track_index:02} {}", name.trim()),
            channels,
        ));
    }
    groups
}

/// Renders one WAV per mixer track that has channels routed to it, all padded
/// to the same length. Returns the written file names.
///
/// # Errors
///
/// Returns a message if the folder or a file cannot be written.
pub fn export_stems(
    project: &Project,
    dir: &Path,
    format: WavFormat,
    options: RenderOptions,
    host: Option<&PluginHost>,
) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let mut stems = Vec::new();
    for (_, stem, channels) in stem_groups(project) {
        let mut solo = project.clone();
        for (i, channel) in solo.channels.iter_mut().enumerate() {
            channel.mute = !channels.contains(&i);
        }
        let (left, right, _) = render_with(&solo, options, host);
        stems.push((stem, left, right));
    }
    let longest = stems.iter().map(|(_, l, _)| l.len()).max().unwrap_or(0);
    let mut written = Vec::new();
    for (stem, mut left, mut right) in stems {
        left.resize(longest, 0.0);
        right.resize(longest, 0.0);
        let file = format!("{stem}.wav");
        write_wav(&dir.join(&file), &left, &right, options.sample_rate, format)
            .map_err(|e| e.to_string())?;
        written.push(file);
    }
    Ok(written)
}

/// `project.export_stems <folder> [16|24|32f]`.
#[derive(Default)]
pub struct ExportStemsCommand {
    /// Plugins to render with.
    pub host: Option<PluginHost>,
}

impl Command for ExportStemsCommand {
    fn name(&self) -> &'static str {
        "project.export_stems"
    }

    fn summary(&self) -> &'static str {
        "Render one WAV per mixer track that has channels routed to it."
    }

    fn usage(&self) -> &'static str {
        "project.export_stems <folder> [16|24|32f]"
    }

    fn execute(&self, project: &mut Project, args: &[String]) -> Result<Outcome, CommandError> {
        let (dir, format) = match args {
            [dir] => (dir, WavFormat::Int16),
            [dir, bits] => (
                dir,
                WavFormat::parse(bits).ok_or_else(|| CommandError::InvalidArgument {
                    command: self.name().to_owned(),
                    reason: format!("`{bits}` is not 16, 24 or 32f"),
                })?,
            ),
            _ => {
                return Err(CommandError::Usage {
                    command: self.name().to_owned(),
                    usage: self.usage().to_owned(),
                });
            }
        };
        let files = export_stems(
            project,
            Path::new(dir),
            format,
            RenderOptions::default(),
            self.host.as_ref(),
        )
        .map_err(CommandError::Io)?;
        if files.is_empty() {
            return Ok(Outcome::done("no unmuted channels to export"));
        }
        Ok(Outcome::done(format!(
            "exported {} stems to {dir}: {}",
            files.len(),
            files.join(", ")
        )))
    }
}

/// `project.export_flac <path> [16|24] [loops] [sample rate]`.
#[derive(Default)]
pub struct ExportFlacCommand {
    /// Plugins to render with.
    pub host: Option<PluginHost>,
}

impl Command for ExportFlacCommand {
    fn name(&self) -> &'static str {
        "project.export_flac"
    }

    fn summary(&self) -> &'static str {
        "Render the pattern or song (by play mode) to a lossless FLAC file."
    }

    fn usage(&self) -> &'static str {
        "project.export_flac <path> [16|24] [loops] [sample rate]"
    }

    fn execute(&self, project: &mut Project, args: &[String]) -> Result<Outcome, CommandError> {
        let invalid = |reason: String| CommandError::InvalidArgument {
            command: self.name().to_owned(),
            reason,
        };
        let [path, rest @ ..] = args else {
            return Err(CommandError::Usage {
                command: self.name().to_owned(),
                usage: self.usage().to_owned(),
            });
        };
        if rest.len() > 3 {
            return Err(CommandError::Usage {
                command: self.name().to_owned(),
                usage: self.usage().to_owned(),
            });
        }
        let bits = match rest.first().map(String::as_str) {
            None | Some("16") => 16,
            Some("24") => 24,
            Some(other) => return Err(invalid(format!("`{other}` is not 16 or 24"))),
        };
        let loops = match rest.get(1) {
            Some(raw) => raw
                .parse::<u32>()
                .ok()
                .filter(|n| (1..=64).contains(n))
                .ok_or_else(|| invalid(format!("`{raw}` is not a loop count 1-64")))?,
            None => 1,
        };
        let sample_rate = match rest.get(2) {
            Some(raw) => raw
                .parse::<u32>()
                .ok()
                .filter(|n| (8_000..=192_000).contains(n))
                .ok_or_else(|| invalid(format!("`{raw}` is not a sample rate 8000-192000")))?,
            None => 44_100,
        };
        let options = RenderOptions {
            sample_rate,
            loops,
            ..RenderOptions::default()
        };
        let (left, right, warnings) = render_with(project, options, self.host.as_ref());
        musiccraft_samples::write_flac(Path::new(path), &left, &right, sample_rate, bits)
            .map_err(|e| CommandError::Io(e.to_string()))?;
        let mut message = format!(
            "exported {:.2} s to {path}",
            left.len() as f64 / f64::from(sample_rate)
        );
        for warning in warnings {
            message.push_str(&format!(" (warning: {warning})"));
        }
        Ok(Outcome::done(message))
    }
}

/// Adds the render crate's commands to a registry, without plugins.
///
/// # Errors
///
/// Returns [`CommandError::Duplicate`] if they are already registered.
pub fn register(registry: &mut Registry) -> Result<(), CommandError> {
    register_with(registry, None)
}

/// Adds the render crate's commands to a registry; exports run plugins from
/// `host` and `plugin.list` lists its catalog.
///
/// # Errors
///
/// Returns [`CommandError::Duplicate`] if they are already registered.
pub fn register_with(
    registry: &mut Registry,
    host: Option<PluginHost>,
) -> Result<(), CommandError> {
    registry.register(Box::new(ExportCommand { host: host.clone() }))?;
    registry.register(Box::new(ExportStemsCommand { host: host.clone() }))?;
    registry.register(Box::new(ExportFlacCommand { host: host.clone() }))?;
    registry.register(Box::new(crate::ConsolidateCommand { host: host.clone() }))?;
    registry.register(Box::new(crate::ExportMidiCommand))?;
    registry.register(Box::new(crate::SampleEditCommand))?;
    registry.register(Box::new(crate::plugins::PluginListCommand { host }))
}
