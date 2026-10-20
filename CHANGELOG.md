# Changelog

Notable changes per release. GitHub's generated release notes list every
commit; this file says what they add up to.

## Unreleased (since 0.2.2)

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

### Agents

- `musiccraft-mcp` and `musiccraft-cli` ship in the release downloads: in the
  Linux and Windows portable archives, and as `MusicCraft-CLI-<v>-macos-universal.zip`.
- Live mode: `musiccraft --control <port> --control-token-file <file>` lets an
  agent work in the open window through `musiccraft-mcp --bridge`, with a
  private token; it never replaces a project with unsaved changes.
- `--dir <folder>` (app and MCP server) puts relative paths in that folder.
  CreativeCraft uses all of this to drive MusicCraft.

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
- The plugins have not yet been tried by hand in Ardour, Bitwig or REAPER
  (`docs/smoke-tests.md`).
