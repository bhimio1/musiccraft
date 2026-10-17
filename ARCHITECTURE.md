# MusicCraft architecture

## Goals

- Pure Rust, cross-platform (Linux, macOS, Windows), headless-testable.
- The audio engine is isolated from everything else and obeys real-time rules.
- A single command registry is the only way to change a project. UI, shortcuts,
  CLI, and MCP are thin front ends over it, so Claude Code can drive and verify the
  whole app without a GUI.

## Crate graph

```
musiccraft-app ──► musiccraft-ui ──► musiccraft-commands ──► musiccraft-project ──► musiccraft-params
      │                                     ▲    └──► musiccraft-flp                  ▲
      ├──► musiccraft-render ───────────────┤                                         │
      │        ├──► musiccraft-engine ──► musiccraft-dsp ─────────────────────────────┤
      │        └──► musiccraft-samples ──► musiccraft-dsp                              │
      ├──► musiccraft-audio ──► musiccraft-engine                                      │
      │                                                                                │
musiccraft-cli ──► commands, render, audio                                             │
musiccraft-mcp ──► commands, render                                                    │
xtask ──► commands, render, ui (keymap) ───────────────────────────────────────────────┘
musiccraft-project, musiccraft-engine ──► musiccraft-meter ──► musiccraft-params
musiccraft-commands, musiccraft-render, musiccraft-ui ──► musiccraft-paths
```

| Crate | Responsibility |
| --- | --- |
| `musiccraft-formula` | The formula controllers' expression language: parsing on the main thread, allocation-free evaluation on the audio thread. No dependencies. |
| `musiccraft-params` | Shared tables: timing (PPQ), instrument and effect parameter specs, built-in sound names. No logic. |
| `musiccraft-paths` | Translates sample paths written on another system (Windows drives, FL Studio placeholders, Wine prefixes) into local files. No dependencies. |
| `musiccraft-meter` | Time signatures and the bar and beat grid of a timeline (`TimeSig`, `Meter`). Queries are allocation-free, so the engine uses it too. |
| `musiccraft-project` | Project data model and JSON file format. No audio, no UI. |
| `musiccraft-commands` | Command trait, ~70 built-in commands, invocation parsing, undo/redo `Session`; converts FL Studio projects for `project.import_flp`. |
| `musiccraft-flp` | Reads FL Studio projects and presets: a Rust port of PyFLP (GPL-3.0), tested against PyFLP on PyFLP's own test files. No dependencies; safe code. |
| `musiccraft-dsp` | Oscillators, envelopes, filters, effects (incl. a small radix-2 FFT for convolution and the analyzer), sample buffers. Allocation-free processing. |
| `musiccraft-engine` | Real-time engine: voices, sequencing, mixing, meters. Driven only by messages. |
| `musiccraft-samples` | WAV decoding/encoding, FLAC encoding, sample cache, synthesized built-in sounds. Main thread only. |
| `musiccraft-clap` | CLAP plugin host: scanning, loading, instances, the audio-thread processor, plugin windows. **Allowed to use `unsafe`** (with its test fixture `musiccraft-clap-testplugin`). |
| `musiccraft-vst3` | VST3 plugin host: scanning modules, component and controller setup, parameters, state, the audio-thread processor, embedded plugin windows. **Allowed to use `unsafe`** (with its test fixture `musiccraft-vst3-testplugin`). |
| `musiccraft-plugin-window` | The plain X11 window both plugin hosts embed plugin views in. Safe code. |
| `musiccraft-render` | Bridge: diffs a project against engine state and sends messages; offline render; `project.export_wav`; the safe plugin boundary (`PluginFactory`, `PluginInstance`) and plugin instance management. |
| `musiccraft-audio` | Opens the output device with `cpal` and runs the engine in its callback; captures input through a lock-free ring for recording. |
| `musiccraft-cli` | Headless front end: run commands and scripts, dump JSON, play, export. |
| `musiccraft-mcp` | MCP server (JSON-RPC over stdio) exposing every command as a tool. |
| `musiccraft-ui` | egui views (channel rack, piano roll, playlist, mixer, effect windows, settings) and the keymap. Emits invocations only. Uses `musiccraft-dsp`'s `response`, `fft` and `pitch` modules only to draw effect curves, the spectrum analyzer and the tuner display. |
| `musiccraft-app` | Wires session, bridge, audio device, MIDI input and UI together. |
| `xtask` | `cargo xtask parity`: checks the feature checklist against the registry and keymap. |

## Command registry

- `Command` trait: `name`, `summary`, `usage`, `execute(&mut Project, &[String])`.
- `Invocation` is a command name plus string arguments. It is what a shortcut, a
  menu item, a CLI line, and an MCP tool call all become. Its text form supports
  quoting, so file paths and JSON can be arguments.
- `execute` returns an `Outcome` with a message, an optional **inverse
  invocation**, and a `reset_history` flag (new/load). The `Session` keeps undo and
  redo stacks of invocations.
- `edit.undo`, `edit.redo` and `edit.batch` are handled by `Session` itself.
  Batches give multi-part edits a single, atomic undo step.
- Naming: `area.action` (`transport.play`, `project.set_tempo`).
- Transport, mode and selection commands are deliberately not undoable; edits are.
- Other crates add commands with `Registry::register` (the render crate adds
  `project.export_wav`).
- See `docs/spec/commands.md` for numbering, time units and undo details.

## Threading model

- **Audio thread**: owns the `Engine`. Runs inside the audio callback. No
  allocation, deallocation, locks, I/O, or panics.
- **Main/UI thread**: owns the `Session`, `Project` and `Bridge`. After commands
  run, the bridge diffs the project against what the engine last received and
  pushes messages through a lock-free SPSC queue (`rtrb`).
- **MIDI thread** (optional): sends live note events through a second SPSC queue.
- **MIDI output thread** (optional): drains a third SPSC queue, filled by the
  engine with the note messages of MIDI Out channels (fixed-size `Copy`
  events; a full queue drops them), and sends them to the chosen port. It
  polls every millisecond, so timing is accurate to about one audio buffer.
- The engine never reads `Project`. Data the engine needs (sample buffers, note
  sequences, effect state) is prepared on the main thread.

### Plugins

- Plugins are reached only through traits: the engine's `External` (audio
  thread: notes, parameter changes, `process`, `stop`) and render's
  `PluginFactory` / `PluginInstance` (main thread: create, parameters, state,
  window). The bridge keeps one live instance per project plugin instance
  number and installs its processor in one of the engine's fixed external
  slots (`SetExternal`), which survive graph rebuilds. Replaced processors come
  back as garbage, already stopped, and are destroyed on the main thread.
- `musiccraft_cli::plugin_host()` creates the one plugin host per process
  (shared by the app, CLI sessions and the MCP server): render's
  `CombinedHost` over a `ClapHost` and a `Vst3Host`, so both kinds appear in
  one catalog. `MUSICCRAFT_NO_PLUGINS` turns hosting off. Without a host, plugin channels are silent and plugin
  effects pass audio through.

### The `unsafe` exception

Hosting a plugin means calling a shared library through the C ABI, so
`musiccraft-clap` and `musiccraft-vst3` (and the test plugins they are tested
against) opt out of the workspace's `unsafe_code = "forbid"`. The user
approved CLAP on 2026-10-10 and extended the exception to VST3 on 2026-10-16.
Rules inside them: `unsafe_op_in_unsafe_fn` is denied and clippy's
`undocumented_unsafe_blocks` requires a `SAFETY:` comment on every block;
plugin function pointers and interface pointers are checked before use; VST3
reference counts are managed by `ComPtr`/`ComWrapper`, never by hand;
main-thread plugin functions are called only from the main thread and
audio-thread functions only from the processor; the processor allocates
nothing (and the VST3 processor locks nothing) after construction. A crashing
plugin still takes the app down (no sandboxing).

### Message rule (refines the CLAUDE.md audio-thread rule)

Control messages are plain `Copy` data. Four messages, `SetGraph`,
`SetSequence`, `PlayPreview` and `SetExternal`, carry a `Box` allocated on the main thread. Moving a box is a
fixed-size pointer move, not an allocation. The value it replaces is pushed onto
a return queue and dropped on the main thread. The engine checks that the return
queue has room *before* taking such a message; if it does not, the message waits
for the next callback. The audio thread therefore never allocates or frees.
Shared sample data lives in `Arc`s that are only ever dropped by the main thread
(they travel inside returned graphs).

## Time

- 96 ticks per quarter note (PPQ), 24 ticks per sixteenth-note step. Bars and
  beats come from time signatures (`docs/spec/time-signatures.md`): every bar
  and beat position goes through a `musiccraft_meter::Meter`, never a fixed bar
  length. The engine keeps the playhead as fractional ticks and starts each note
  on the first frame at or after its tick; its sequence carries the meter for
  the metronome, the count-in and the plugin transport.

## Testing strategy

- Unit tests for every command, including error paths. A round-trip test runs a
  script touching every undoable command, then undoes, redoes and undoes again,
  comparing the project to snapshots at every step.
- Engine and DSP tests render offline and assert on exact sample positions.
- Render tests check sequence building, diff-based sync and exported audio.
- CLI integration tests spawn the real binary.
- `cargo xtask parity` reports how many checklist features are implemented.

## Dependencies

| Dependency | Why |
| --- | --- |
| `cpal` | Cross-platform audio output device access. |
| `rtrb` | Wait-free SPSC ring buffer for main → audio messages, returns and live notes. |
| `serde`, `serde_json` | Project file format and JSON payloads in undo inverses and MCP. |
| `hound` | WAV decoding for samples and WAV encoding for export. |
| `eframe` (egui) | Cross-platform immediate-mode UI; kept swappable behind commands. |
| `midir` | MIDI keyboard input and MIDI Out channels' output port in the app. |
| `clap-sys` | Rust declarations of the CLAP plugin ABI (types and constants only). |
| `libloading` | Opens `.clap` and `.vst3` shared libraries and finds their entry points. |
| `vst3` | Rust declarations of the VST3 interfaces, generated from Steinberg's (MIT-licensed) SDK headers, plus `ComPtr`/`ComWrapper` reference counting; MIT or Apache-2.0. |
| `libc` | `poll(2)` for file descriptors plugin windows register (Unix only). |
| `x11rb` | A pure-Rust X11 window for plugins that embed their editor (Linux only; already in the tree through the windowing stack). |

## Deferred decisions

- Custom `wgpu` renderer if egui cannot reach the target look.
- VST3 on macOS (the module entry needs a CoreFoundation bundle reference) and
  VST3 windows on Windows and macOS.
- Out-of-process plugin scanning/hosting, so a crashing plugin cannot take the
  app down.
- Honoring a plugin's restart request (latency or port changes) while running.
- Decoding compressed samples (FLAC, Ogg, MP3), likely via `symphonia`; lossy
  export formats.
- Tempo automation.
