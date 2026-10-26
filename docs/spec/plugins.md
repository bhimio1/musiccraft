# Spec: plugin hosting (CLAP and VST3)

## Behavior

- MusicCraft hosts **CLAP** plugins as channel instruments and as mixer insert
  effects. A plugin is referenced by its CLAP plugin id (for example
  `com.example.synth`); the file it comes from is found by scanning, so projects
  move between machines.
- **Scanning** looks for `*.clap` files in the standard CLAP locations
  (`$HOME/.clap`, `/usr/lib/clap`, `/usr/local/lib/clap` on Linux;
  `~/Library/Audio/Plug-Ins/CLAP` and `/Library/Audio/Plug-Ins/CLAP` on macOS;
  `%COMMONPROGRAMFILES%\CLAP` and `%LOCALAPPDATA%\Programs\Common\CLAP` on
  Windows) and in every directory listed in `CLAP_PATH`. Each file can hold
  several plugins. A plugin whose features include `instrument` is offered as an
  instrument; one with `audio-effect` as an effect.
- **Instruments**: `channel.add_plugin <plugin-id> [name]` adds a channel whose
  sound comes from the plugin. Notes, step-sequencer hits, live MIDI and
  previews reach the plugin as CLAP note events (velocity 0–1, note pan as a
  pan note expression). Channel volume, pan, mute and mixer routing apply to the
  plugin's stereo output like any other channel. The built-in instrument knobs
  (envelope, filter…) do not apply.
- **Effects**: `mixer.add_plugin <track> <plugin-id> [name]` appends a plugin
  effect slot. It takes the slot's place in the chain, stereo in and out.
  Bypass, removal, moving and undo work as for built-in effects. A plugin
  effect's `sidechain` setting (`mixer.set_effect_param <track> <slot>
  sidechain <n>`, 0 = off) feeds track `n`'s post-fader output, which must be
  routed to the slot's track as a sidechain (`docs/spec/mixer.md`,
  *Sidechain*), to the plugin's first auxiliary input (CLAP aux port, VST3
  aux bus). Without one the aux input is silent.
- **Parameters**: a plugin's parameters are identified by their CLAP parameter
  id and use the plugin's own ("plain") values.
  `channel.set_plugin_param <channel> <param-id> <value>` and
  `mixer.set_plugin_param <track> <slot> <param-id> <value>` set one; both are
  undoable and coalesce while dragging. Values set this way are stored in the
  project and win over the saved state.
- **Plugin editor**: a plugin's own window can be opened from its channel or
  slot. When a parameter is moved in that window, MusicCraft records it through
  the same parameter commands, so it is undoable and saved.
- **State**: the plugin's opaque state (CLAP `state` extension) is stored in the
  project as base64 (`channel.set_plugin_state`, `mixer.set_plugin_state`;
  not undoable — they mirror what the plugin already holds). The app refreshes
  it when the plugin reports a change and before saving or exporting. On load,
  the state is restored first, then the stored parameter values are applied.
- **Missing plugins**: a project that names a plugin that is not installed
  still loads; the channel is silent and the effect passes audio through, with a
  warning in the status bar. The stored state is kept untouched.
- **Export** renders plugins offline with fresh instances restored from the
  project, so exports sound like playback.
- `plugin.list` lists the scanned plugins (id, name, kind, file).
- The app scans the plugin folders at start unless **Scan plugins on startup**
  is off in Settings → Folders (`settings.md`); then the catalog stays empty
  until a rescan.

## Real-time rules

- Plugin instances are created, activated, deactivated and destroyed on the
  main thread. The audio thread only calls `start_processing`, `process` and
  `stop_processing`.
- Instances live in a fixed table of engine slots that survives graph
  rebuilds, so adding a channel does not reset a plugin.
- Notes and parameter changes reach a plugin through preallocated, fixed-size
  event buffers; nothing allocates on the audio thread. Parameter changes the
  plugin makes in its own window come back through a lock-free queue.
- A plugin that is removed is handed back to the main thread before it is
  deactivated and destroyed.

## Safety boundary

- Loading a plugin means running code from a shared library through the C ABI,
  which requires `unsafe`. All of it lives in two crates, `musiccraft-clap` and
  `musiccraft-vst3` (plus their test plugins); every other crate keeps
  `unsafe_code = "forbid"`. The rest of MusicCraft talks to
  plugins only through the safe `External` trait (engine) and the
  `PluginFactory` trait (render).
- A crashing plugin can take the app down; plugins are not hosted out of
  process yet. VST3 scanning runs in child processes (see the VST3 section),
  so a VST3 module that crashes while it is scanned is only skipped and
  reported. CLAP scanning still loads every `.clap` file found in process
  (that runs its code); files that fail to load are skipped and reported.

## Plugin windows

- Plugins that can open a floating window of their own get one. On Linux,
  plugins that only embed are placed in an X11 window MusicCraft creates
  (through XWayland on Wayland desktops), marked as a fixed-size dialog unless
  the plugin can resize. Window resizes go through the plugin's
  `adjust_size` before `set_size`.
- While a window is open, the app polls the plugin's timers and file
  descriptors (`timer-support`, `posix-fd-support`) every frame.
- Closing the window with the title-bar button works like the panel's close
  button.

## Limits

- Notes go to the plugin's first note port, as CLAP note events, or as MIDI
  for plugins that only accept MIDI. Only the main audio port's first two
  channels are used; other ports get silent buffers.
- A plugin asking to be restarted (latency or port changes) keeps running
  with its current setup.

## VST3

- MusicCraft also hosts **VST3** plugins, as instruments and effects, with the
  same commands, project storage and windows as CLAP plugins. The plugin
  database lists both kinds together.
- **Ids**: a VST3 plugin is referenced as `vst3:` followed by the 32 hex digits
  of its audio processor's class id (for example
  `vst3:6E33225254224A00AA69301AF318797D`), so CLAP and VST3 ids never clash.
- **Scanning** looks for `*.vst3` bundles (or single files) in the standard
  VST3 locations (`$HOME/.vst3`, `/usr/lib/vst3`, `/usr/local/lib/vst3` on
  Linux; `%COMMONPROGRAMFILES%\VST3` on Windows;
  `~/Library/Audio/Plug-Ins/VST3` and `/Library/Audio/Plug-Ins/VST3` on macOS)
  and in every directory listed in `VST3_PATH`. Inside a bundle the binary is
  `Contents/<arch>-linux/<name>.so`, `Contents/<arch>-win/<name>.vst3` or
  `Contents/MacOS/<name>`. The module's entry function (`ModuleEntry`,
  `InitDll`) runs once when it loads and its exit function when it unloads.
  If the entry function fails, the exit function still runs before the
  module is closed, as the VST3 SDK's own host does, so a module that counts
  its loads starts from zero next time.
  macOS bundles are listed as scan errors for now: their entry function needs
  a CoreFoundation bundle reference MusicCraft does not create yet.
- **Scanning out of process**: the app, CLI and MCP server read each module's
  class list in a child process (the same program, started with
  `MUSICCRAFT_VST3_PROBE` naming the module), several at a time. A module
  that crashes, hangs for more than two minutes or fails to load in the child
  is skipped and listed as a scan error saying what happened (for example
  "crashed while loading (signal 11, SIGSEGV)"); the scan carries on with the
  next module. A module is loaded into the app itself only when one of its
  plugins is first used. A bundle with no binary for this system (for
  example a Windows-only bundle on Linux) is reported as such. Modules that
  are already loaded are not probed again on a rescan. If the child cannot be
  started at all, the module is loaded in process as before.
- **Kind**: a class of category "Audio Module Class" whose sub-categories
  include `Instrument` is an instrument; any other audio module is an effect.
- **Instances**: the audio component is created and initialized with a host
  context (`IHostApplication`, which can also create messages and attribute
  lists). Its edit controller is the component itself when it implements
  `IEditController`, otherwise the class the component names, created and
  initialized the same way. Separate component and controller are connected
  to each other through `IConnectionPoint`, and the controller receives the
  component's state (`setComponentState`). Stereo main buses are activated
  (and the first event input bus for instruments), the bus arrangement is set
  to stereo, processing is set up for real-time 32-bit audio at the engine's
  rate and largest block, and the component is made active. Teardown runs in
  the reverse order.
- **Notes** reach the first event input bus as note-on and note-off events
  (channel 0, velocity 0–1, no note id). Pan per note is not sent.
- **Parameters** come from the edit controller. Continuous parameters use
  normalized values, 0 to 1; a parameter with `n` steps uses whole numbers 0
  to `n` (sent to the plugin as `value / n`). Values reach the processor
  through the input parameter changes of the next `process` call, and the
  controller is told too (`setParamNormalized`). The controller formats
  values for display (`getParamStringByValue`).
- **Edits in the plugin's window** arrive through the host's component handler
  (`beginEdit`, `performEdit`, `endEdit`): MusicCraft records them as
  parameter commands, like CLAP edits, and forwards them to the processor.
  Changes the processor reports in its output parameter changes are recorded
  the same way. A `restartComponent` asking to reload parameter values or
  titles refreshes the parameter list; other restart requests are ignored
  while the plugin keeps running.
- **State** is saved as one blob holding the component's state and the
  controller's state: the bytes `MCV3`, a little-endian `u32` version (1), then
  for each part a little-endian `u64` length and the bytes. Restoring gives the
  component its part (`setState`), then the controller the component's part
  (`setComponentState`) and its own part (`setState`).
- **Windows**: on Linux, a plugin view that supports `X11EmbedWindowID` is
  embedded in an X11 window MusicCraft creates. The host frame
  (`IPlugFrame`) handles resize requests, and the host run loop
  (`Linux::IRunLoop`) lets the plugin register file descriptors and timers,
  which the app polls every frame while the window is open. On Windows and
  macOS, VST3 windows are not offered yet.
- **Real-time rules** are the CLAP ones: `setProcessing` and `process` are the
  only audio-thread calls; parameter queues, the event list and the process
  context are preallocated, and nothing allocates on the audio thread.

## Sources

- CLAP — open plugin standard, MIT licensed: headers and documentation at
  github.com/free-audio/clap (plugin entry and factory, `process`, events,
  `params`, `state`, `gui`, `timer-support`, `posix-fd-support`, search paths in
  `entry.h`).
- VST 3 — Steinberg's plugin API, MIT licensed since SDK 3.8: interfaces and
  documentation at github.com/steinbergmedia/vst3sdk and the VST 3 developer
  portal (module entry functions, `IPluginFactory`, `IComponent`,
  `IAudioProcessor`, `IEditController`, `IConnectionPoint`, `IPlugView`,
  `Linux::IRunLoop`, bundle layout and search paths). Rust declarations come
  from the `vst3` crate (MIT or Apache-2.0), generated from those headers.
- Placement of plugins as channel instruments and mixer insert effects follows
  the publicly documented workflow of pattern-based DAWs; everything else is our
  own design.
