# Spec: project files and audio export (M6)

## Project files

- Projects are saved as UTF-8 JSON, conventionally with the `.mcproj` extension:

  ```json
  { "format": "musiccraft-project", "version": 1, "project": { ... } }
  ```

- Instrument and effect parameters are stored as `name → value` maps, so files
  stay readable and older files load when parameters are added (missing values
  take defaults; unknown names are ignored; values are clamped into range).
- Loading validates the file: other formats and newer versions are rejected; an
  invalid tempo is rejected; dangling references (notes for removed channels,
  clips for removed patterns, routes to missing tracks) are dropped or reset.
- Clips store their source as `"pattern": <id>`, `"automation": <id>` or
  `"audio": <channel id>`; files from before automation existed load unchanged.
  Clips whose source no longer exists are dropped on load.
- Controllers are stored under `"controllers"` with their `"kind"` and its
  settings flattened beside the name, target and range; files without them
  load with none (see `controllers.md`).
- Sample paths are stored as given. Relative paths resolve against the project
  file's directory first, then the working directory.
- `project.save [path]` writes atomically (temporary file, then rename) and
  remembers the path; without an argument it saves to the remembered path.
  `project.load <path>` and `project.new` replace the project and clear undo
  history. The transport state is not saved.
- `project.save_new_version` (Ctrl+N via `ui.save_new_version`) saves a copy
  as the next numbered sibling and continues in it: `song.mcproj` becomes
  `song_2.mcproj`, `song_2.mcproj` becomes `song_3.mcproj`, and names already
  on disk are skipped. The earlier file is left as it was. A project that has
  never been saved fails the command; the UI opens Save as instead.

## Export

- `project.export_wav <path> [16|24|32f] [loops] [sample rate]` renders offline,
  faster than real time, with the same engine used for playback. Defaults: 16-bit
  with TPDF dither, one loop, 44.1 kHz.
- What is rendered follows the play mode: the selected pattern or the song,
  repeated `loops` times, followed by the tail of sounding notes and effects. The
  tail ends after half a second below -90 dBFS, or after ten seconds.
- `project.export_flac <path> [16|24] [loops] [sample rate]` writes lossless
  FLAC with our own encoder (fixed predictors, partitioned Rice coding, per the
  published FLAC specification); tests check it bit-exact against the reference
  decoder when it is installed. The export dialog offers WAV, FLAC, MP3 and
  Ogg and keeps the file extension in step.
- `project.export_mp3 <path> [128|192|256|320] [loops] [sample rate]` writes
  MP3 (MPEG-1/2 Layer III) at a constant bit rate in kbit/s, default 320,
  with the LAME encoder at its best quality setting. MP3 carries at most
  48 kHz, so the sample rate is 8000–48000 (default 44.1 kHz). The file has
  an ID3 tag with the project's name as its title.
- `project.export_ogg <path> [quality 0-10] [loops] [sample rate]` writes Ogg
  Vorbis at a variable bit rate, quality 0 (smallest) to 10 (best), default
  8 (about 256 kbit/s for stereo), with the aoTuV-tuned libvorbis encoder.
  Any sample rate 8000–192000 (default 44.1 kHz). The project's name is the
  `TITLE` comment.
- Both lossy exports render exactly as WAV export does and are deterministic
  (the Ogg stream's serial number is fixed). Both encoders are C libraries
  built from source with the app (LAME, LGPL; libvorbis, BSD), so no system
  library is needed.
- Rendering is deterministic: the same project always produces the same file.
- `project.export_stems <folder> [16|24|32f]` writes one file per mixer track that
  has unmuted channels routed to it (`NN Name.wav`, with track 00 holding
  channels routed straight to the master). Each stem renders only its channels
  through the full mixer; all stems are padded to the same length.
- `project.export_midi <path>` (File → Export MIDI…, Ctrl+Shift+M) writes a
  Standard MIDI File, format 1, 96 ticks per quarter: a first track with the
  project name, tempo and time signatures (the pattern's, or the song's with
  its changes), then one track per channel that plays notes
  (named after the channel, MIDI channel = rack position modulo 16). It holds
  what the transport would play once (the pattern or the song by play mode,
  swing and arpeggios applied); audio clips are left out. Velocities map
  0..1 to 1..127.

## Autosave

- The desktop app writes the open project to `autosave.mcproj` in the per-user
  data folder (`$XDG_DATA_HOME/musiccraft`, `~/Library/Application
  Support/MusicCraft`, or `%APPDATA%\MusicCraft`) at most once a minute, and only
  after a change. A failed write is retried a minute later.
- Closing the app with nothing unsaved deletes `autosave.mcproj`. An autosave
  found at start-up therefore means the last session did not close properly: it is
  moved to `recovery.mcproj` (so this session's autosaves cannot overwrite it),
  the status line says so, and File → Recover unsaved work offers it.
- Recovering runs `project.recover <file>`: the backup loads as unsaved work with
  no file of its own, so Save asks where to save instead of writing into the data
  folder.
- Closing the window, File → New and File → Open ask whether to save unsaved
  changes first (Save, Don't save, Cancel). "Unsaved" means the project differs
  from what was last saved, loaded or created. Save is the default button,
  drawn with an accent edge: Enter saves and Esc cancels. An FL Studio import
  asks the same way, since it replaces the project too.
- Replacing the project (New, Open, Recover, an FL Studio import, from the
  menu or an assistant) starts the interface afresh on it. Channel, pattern,
  automation and plugin numbers start over in every project, so nothing from
  the old project may act on the new one: piano roll, playlist and mixer
  selections, the playlist's clip clipboard and the automation or audio clip
  it paints, extra channel windows, effect windows, the channel rack's group
  filter, the sample editor's selection, the open type-in box or rename field,
  and any gesture in progress are dropped, and the first channel and the
  master track are selected. Every running plugin and Antiserum synth stops
  and the new project's own start with their saved state. View preferences
  stay: open windows, tools, zoom, snap, view options and the note clipboard
  (notes paste into any project).

## Sources

- Own design. WAV/RIFF is an open format, written with the `hound` crate. We do
  not write any other DAW's project format; the one we read, FL Studio's, is
  read through a port of PyFLP (see `flp-import.md`).
- MP3 and Ogg Vorbis export: open formats (ISO/IEC 11172-3 and 13818-3;
  Xiph.Org's Ogg and Vorbis I specifications), encoded by the open-source
  LAME (LGPL) and libvorbis with aoTuV tuning (BSD) libraries through the
  `mp3lame-encoder` and `vorbis_rs` crates. The defaults (320 kbit/s CBR,
  Vorbis quality 8) are our own choice.
- MIDI export: the Standard MIDI File format of the public MIDI 1.0
  specification; Ctrl+Shift+M follows the FL Studio online manual ("Keyboard
  shortcuts"). Track layout is our own.
- Saving a new numbered version on Ctrl+N follows the "save new version"
  shortcut described in the public FL Studio online manual ("Keyboard
  shortcuts"); the naming scheme is our own.
