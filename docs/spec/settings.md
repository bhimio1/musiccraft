# Spec: settings

## Behavior

- **Options → Settings** (F10) opens one window with tabs. The Options menu
  also opens it directly at a tab: Audio, MIDI, General, Folders and Project.
- Settings belong to the user, not the project: they are stored in
  `$XDG_CONFIG_HOME/musiccraft/settings.json` (`~/.config/…` when unset;
  `%APPDATA%\MusicCraft` on Windows, `~/Library/Application Support/MusicCraft`
  on macOS) and loaded at start. A missing or unreadable file means defaults.
  Changes apply and save immediately; there is no OK/Cancel.
- **Audio**: input device for audio recording (system default first; the next
  take opens the new choice), output device (default first), sample rate (device default or
  one the device supports) and buffer size (device default or 64–4096
  frames, with the resulting latency shown). Changing any of them restarts
  audio: playback stops, running plugins store their state in the project and
  start again at the new rate. If a request cannot be met the device default
  is used and the reason is shown.
- **MIDI**: input port — automatic (first real device), none, or a named
  port; output port for MIDI Out channels — none or a named port; whether to
  send MIDI clock with start and stop on that port (off by default); a refresh
  button re-lists ports. The typing keyboard toggle lives here
  as well as on the toolbar.
- **General**: interface scale (75–200 %), hint bar on/off, autosave on/off,
  start with an empty project instead of the starter kit.
- **Folders**: browser folders (add, remove) and extra plugin folders (add,
  remove), searched in addition to the standard CLAP locations; a rescan
  button, the number of plugins found and any files that failed to load.
  Below them, **samples from Windows projects**: the FL Studio factory and
  user data folders (empty means detected) and drive mappings (a Windows
  prefix → a folder; add, remove). See `sample-paths.md`; the browser folders
  are also searched by file name.
- **Project**: name, tempo and pan law (through the usual commands, so
  undoable), file path, and counts of channels, patterns, clips and plugins.

## Sources

- Tab layout (audio, MIDI, general, file, project) follows the publicly
  documented settings window of pattern-based DAWs; storage and behavior are
  our own design.
