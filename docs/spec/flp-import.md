# Spec: importing FL Studio projects

`project.import_flp <file.flp>` replaces the project with one converted from an
FL Studio project, like `project.load` (it clears undo history and the project
has no file path: saving asks for one). The message lists what could not be
brought over.

## Reading

- The file is read by `musiccraft-flp`, a Rust port of PyFLP (see `docs/clean-room.md`).
  It reads what PyFLP reads, and nothing else. The tests compare it with
  PyFLP property by property on PyFLP's own test files (`crates/musiccraft-flp/tests`).
- A file PyFLP cannot parse is refused with the same reason (bad header, unknown
  version, a cut-off event). Parts that fail to read (a channel list naming a
  missing group, a clip naming a missing pattern) are skipped and reported.

## Conversion

Times in ticks are rescaled from the file's PPQ to MusicCraft's 96 (rounded).

- **Project**: the title (or the file name) becomes the name. Tempo is clamped
  to 10–999 BPM. Master pitch (cents) and the pan law (circular or triangular)
  carry over.
- **Channels**, in rack order. Volume 0–12800 maps to gain 0–1 (FL's default
  10000 becomes 0.78), pan 0–12800 to −1…1, and a disabled channel is muted.
  The mixer insert carries over (the "current" insert, −1, becomes master), as
  does the display group name.
  - Sampler with a sample: a sampler playing that path. On Linux and macOS
    Windows paths and FL's placeholders (`%FLStudioFactoryData%…`) are
    translated at import (`sample-paths.md`): a file found on this computer
    replaces the path, and the report says how many were found and lists the
    ones that were not. Those stay as written and play silence until found
    (Settings → Folders, then `project.localize_samples`).
  - Sampler without a sample: an empty sampler (silent, with no warning).
  - VST3 plugin: a plugin instrument with the VST3 class id MusicCraft uses
    (`vst3:<32 hex>`). The id's byte order follows the platform that saved the
    file: a Windows path to the plugin means COM order. It is converted to the
    order of the platform MusicCraft runs on. FL's saved plugin state is
    in FL's wrapper format and is not carried over. The plugin starts from its
    defaults.
  - Layer: a layer of the converted children.
  - Anything else (FL's own instruments, VST2 plugins): a two-oscillator synth
    named after the original. It is listed as replaced.
  - Automation clips are not imported: PyFLP does not read what they control.
- **Patterns**, in order. Every note keeps its time, length, key (clamped to
  0–127), velocity (0–128 clamped to 1–127), pan (0–128 → −100…100), release,
  fine pitch (0–240 is −1200…1200 cents per PyFLP, clamped to −100…100), mod X
  and Y (0–255 → 0–127),
  colour (the low 4 bits of the MIDI channel field) and slide. A pattern with a
  set length keeps it in steps (at least one bar). Notes for channels that were
  not imported are dropped.
- **Playlist**: the arrangement FL had selected (else the first). Track rows
  keep their index (rows past MusicCraft's 64 are dropped), name, and
  disabled state (muted). Pattern clips keep position, length and start
  offset; clips of sampler channels become audio clips; other channel clips
  are dropped. Text markers become markers. The playlist selection becomes the
  loop range when both ends are set.
- **Mixer**: inserts are numbered by their position in the file, master
  first, which is how channels name their insert (PyFLP's `iid` is one less).
  Master and inserts up to the last one used (named, routed to by a channel,
  or holding a plugin) are imported, at least MusicCraft's default count and
  at most 128 tracks. Name, volume (0–16000 → gain 0–1.25),
  pan (−6400…6400 → −1…1), stereo separation (64 merged … −64 separated →
  −1…1), polarity, swapped sides, mute (disabled) and solo carry over. Each
  insert's routing flags give its destinations: the master flag sets "to
  master", higher imported inserts become sends (at most 8), at the route's
  level when the file has one, else 1. Routes at level 0 are skipped. FL 20
  files flag routes to inserts 100–103 at level 0 on every insert, and
  neither PyFLP nor the format explains them. VST3 effects become plugin effect slots (from the
  defaults, as for instruments). FL's own effects are listed as dropped.

FL's volume and pan curves are not linear. These mappings keep fader position,
not loudness, so an imported mix will sound different. The message says so.

## Sources

- The FL Studio file format as PyFLP reads it: PyFLP (https://github.com/demberto/PyFLP),
  GPL-3.0-or-later, ported to Rust by the owner's decision of 2026-10-16
  (see `docs/clean-room.md`). The value ranges above are from PyFLP's documentation
  of each property.
- VST3 class id byte order: the VST3 SDK's `INLINE_UID`/`COM_COMPATIBLE`
  definition (open standard, MIT-licensed SDK).
- The conversion choices are our own design.
