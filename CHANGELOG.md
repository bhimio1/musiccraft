# Changelog

Notable changes per release. GitHub's generated release notes list every
commit; this file says what they add up to.

## Unreleased (since 0.3.0)

### Sidechaining as in FL Studio

- Select an insert, right-click another track's routing arrow and choose
  **Sidechain to this track** (`mixer.sidechain_to`). The route carries the
  insert's sound to that track's sidechain inputs only, and its arrow is
  drawn dashed. Then pick the insert in the compressor's, vocoder's or a
  plugin's **Sidechain** drop-down, which lists the tracks routed there.
- Any insert can key any other: the mixer runs inserts in the order their
  routing needs, so a kick on insert 9 can duck a bass on insert 2 without
  latency. Routes that would form a loop are refused.
- Hosted CLAP and VST3 effects now get a sidechain signal on their first
  auxiliary input; before, they had none.
- Clicking a routing arrow or a mixer menu no longer selects the strip
  under it.
- Older projects load with a route added for every sidechain they use.

### A more compact interface

- The toolbar is one band in a wide window: menus over the hint bar, then
  the transport, then the window buttons.
- The channel rack, piano roll and mixer put their tools on the title row.
- Mixer strips are half as wide, with names written vertically, so twice as
  many inserts fit.
- Taller step buttons, shaded more clearly by beat; a narrower browser,
  taller playlist tracks and a taller piano roll event lane.

### Pattern colours

- Patterns have no colour until you set one (right-click a pattern →
  Colour, or `pattern.set_color`). Uncoloured clips are drawn neutral, and
  the notes inside every pattern clip show their own colours, as in the
  piano roll, so one pattern can show several.
- The playlist's picker shows each pattern as a miniature of its notes (and
  any automation recorded into it), so patterns with default names are easy
  to tell apart. In previews, a
  step-sequenced pattern (every note on one key, as drums usually are) gets
  one row per channel.

### System file dialogs and sample slots

- Open, Save as, Export (audio, MIDI, stems), adding browser and plugin
  folders, adding channels from files, Antiserum patches and impulse files
  use the system's file dialog (the desktop portal on Linux, so it works in
  the Flatpak) instead of a typed path. The window keeps drawing and audio
  keeps playing while it is open.
- Sample slots (the sampler's sample, drum pads, zones, Antiserum's
  oscillators) open a menu: Browse files…, the built-in sounds, and your
  browser folders as submenus. A sound dragged from the browser onto a slot
  fills it.
- The exported plugins' editors do the same, with your Music and Samples
  folders in place of the browser.

### Stem splitting

- Right-click a sampler channel or an audio clip's sound → Split into stems
  makes vocals, drums, bass and other channels, with clips under the
  original (muted), in one undo step.
- It runs Demucs in ONNX Runtime, GPU first: DirectML on Windows, CoreML on
  macOS, CUDA or ROCm through a system ONNX Runtime on Linux, else the CPU.
  The model downloads the first time. Without ONNX Runtime a built-in
  splitter does a rough split.

### Fixes

- A VST3 plugin that crashes while it is scanned no longer takes MusicCraft
  down. Each module is scanned in a child process; one that crashes, hangs or
  fails to load is skipped and listed under plugin problems in the settings,
  and the scan carries on. Bundles with no binary for this system (for
  example Windows-only plugins on Linux) say so. A module is loaded into
  MusicCraft only when one of its plugins is used.
- Rendering a project twice in a DAW gives the same audio. A host reset,
  which DAWs do before a render, now makes every plugin as good as new:
  - Antiserum stops its voices and effect tails at once, where it used to
    only release them, and restarts its random phases.
  - The built-in instruments return each voice to new; the drum synth's
    noise used to differ on every render.
  - The chorus, flanger, pitch shifter and hall reverb restart their LFO and
    sweep phases.

### Checked in other DAWs

- `tools/host-smoke/run.sh` drives REAPER (CLAP and VST3) and Ardour (VST3)
  headless. It loads all 112 plugins, plays and renders, sets parameters
  from the host, opens editors, routes a sidechain, and saves and reopens.
  Results are in `docs/smoke-tests.md`: every check passes in REAPER 7.82
  and Ardour 8.4.

## 0.3.0 (2026-10-20)

### Antiserum, the built-in wavetable synth

- Three oscillators, each a wavetable, a sample (repitch, fixed or stretch,
  with loops), key- and velocity-mapped multisample zones, a granular cloud or
  a spectral resynthesis; stereo samples, unison and warps on every type.
- Sub and noise oscillators, two filters with 18 types, four envelopes, eight
  LFOs with drawn shapes, eight macros and a 32-slot modulation matrix.
- An effects rack (13 modules), an arpeggiator and a 64-step sequencer.
- A wavetable editor (draw, harmonics, formula, morph, import and export with
  frame-size chunks) and 11 generated factory tables.
- About 165 factory presets, plus user presets saved to a folder of their own,
  listed in the browser and shared with the plugin.
- The channel's pitch knob transposes it; its parameters can be automated.

### MusicCraft in other DAWs

- **MusicCraft Effects**: the mixer's 44 effects as CLAP and VST3 plugins.
- **MusicCraft Instruments**: the 11 built-in instruments and Antiserum.
- Every plugin opens the same editor as the app. Presets are shared, and
  automation is sample-accurate. Plugins follow the host's tempo and transport.
- The compressor and vocoder take a sidechain input. The pitch corrector
  follows MIDI notes. The convolution reverb loads WAV impulse files.
- Release downloads include the plugins for Linux, Windows and macOS, with
  install notes.

### Quality

- The plugins pass clap-validator and the VST3 SDK's validator, run on every
  CI build. A project rendered with a plugin sounds the same as with the
  built-in instrument or effect.
- Subnormal input no longer slows effects (up to 18× before). The frequency
  shifter's and DC blocker's tails end in silence.
- Parameter text reads back as typed (no `-0.0`; 999.7 Hz shows `1.00 kHz`).
- Hosting: our CLAP and VST3 hosts feed plugins' sidechain inputs.

### Export

- MP3 (128–320 kbit/s, LAME) and Ogg Vorbis (quality 0–10) export, in the
  export dialog and as `project.export_mp3` / `project.export_ogg`, with the
  project's name as the title tag.

### Fixes

- Exports start at the beginning of the song even with a loop region set;
  empty samplers load silently instead of warning.
- Windows and FL Studio sample paths are found on Linux and macOS.

### Known limitations

- Builds and plugins are not code-signed or notarized (see
  `docs/releasing.md`).
- The plugins had not yet been tried in Ardour, Bitwig or REAPER
  (`docs/smoke-tests.md`); REAPER and Ardour have been since.
