# Feature parity checklist

What a pattern-based DAW user expects, and whether MusicCraft has it. Checked by
`cargo xtask parity`, which verifies each requirement mechanically:

- `cmd:<name>` — the command is registered.
- `key:<chord>=<invocation>` — the default keymap binds the chord to it.
- `spec:<file>` — `docs/spec/<file>` exists.
- `fx:<kind>` — the mixer offers this effect kind.
- `done` — implemented in the UI or app; verified by tests and review, not by
  the checker.
- `todo` — not implemented yet.

Features are described in our own words at the level of a public manual's table
of contents; see `docs/clean-room.md`.

| Area | Feature | Requires |
| --- | --- | --- |
| Transport | Play / stop | `cmd:transport.toggle` `key:Space=transport.toggle` |
| Transport | Pause and play / pause | `cmd:transport.pause` `cmd:transport.play_pause` `key:Ctrl+Space=transport.play_pause` |
| Transport | Jump to the start | `cmd:transport.jump_to_start` `key:Home=transport.jump_to_start` |
| Transport | Panic (all notes off) | `cmd:transport.panic` `key:Ctrl+H=transport.panic` |
| Transport | Pattern / song mode | `cmd:transport.set_mode` `key:L=transport.toggle_mode` |
| Transport | Tempo | `cmd:project.set_tempo` `spec:transport.md` |
| Transport | Metronome | `cmd:transport.toggle_metronome` `key:Ctrl+M=transport.toggle_metronome` |
| Transport | Time signatures: project, per pattern, and changes along the song | `cmd:project.set_time_signature` `cmd:pattern.set_time_signature` `cmd:playlist.set_time_signature` `spec:time-signatures.md` |
| Transport | Metronome, count-in, grids and snap follow the time signature | `done` |
| Transport | Loop a time range of the song | `cmd:transport.set_loop` `cmd:transport.clear_loop` |
| Transport | Record audio into the playlist | `done` |
| Channel rack | Sampler channels with built-in sounds | `cmd:channel.add_sampler` `spec:channel-rack.md` |
| Channel rack | WAV samples | `cmd:channel.set_sample` |
| Channel rack | Step sequencer | `cmd:step.toggle` |
| Channel rack | Channel volume / pan | `cmd:channel.set_volume` `cmd:channel.set_pan` |
| Channel rack | Mute / solo | `cmd:channel.toggle_mute` `cmd:channel.solo` |
| Channel rack | Route to mixer insert | `cmd:channel.route` |
| Channel rack | Rename / clone / reorder / delete | `cmd:channel.rename` `cmd:channel.clone` `cmd:channel.move` `cmd:channel.remove` |
| Channel rack | Instrument parameters | `cmd:channel.set_param` |
| Channel rack | Cut groups (choke) | `spec:channel-rack.md` `done` |
| Channel rack | Mono and maximum polyphony | `spec:channel-rack.md` `done` |
| Channel rack | Swing: global amount, per-channel mix and truncate | `cmd:project.set_swing` `spec:channel-rack.md` |
| Channel rack | Channel settings window (envelope, filter, sample view) | `done` |
| Channel rack | Melodic patterns previewed in the rack | `done` |
| Piano roll | Velocity editor lane | `cmd:note.set_velocity` `done` |
| Piano roll | Select, copy, paste, duplicate, nudge | `cmd:edit.batch` `done` |
| Piano roll | Per-note pan lane | `cmd:note.set_pan` `done` |
| Piano roll | Per-note pitch slides (overlap sets the time; per colour group; chords follow the top note) | `cmd:note.set_slide` `spec:piano-roll.md` |
| Piano roll | Portamento and muted notes, mute tool | `cmd:note.set_prop` `spec:piano-roll.md` |
| Piano roll | Note release velocity, fine pitch, Mod X/Y and colour, edited in the event lane | `cmd:note.set_prop` `spec:piano-roll.md` |
| Piano roll | Right-drag deletes notes | `spec:piano-roll.md` `done` |
| Piano roll | Draw, move, resize, delete notes | `cmd:note.add` `cmd:note.move` `cmd:note.resize` `cmd:note.remove` `spec:piano-roll.md` |
| Piano roll | Velocity | `cmd:note.set_velocity` |
| Piano roll | Quantize | `cmd:note.quantize` |
| Piano roll | Transpose | `cmd:note.transpose` |
| Piano roll | Snap to grid | `done` |
| Piano roll | Ghost notes of other channels | `done` |
| Piano roll | Audition notes from the keyboard | `done` |
| Piano roll | Portamento (glide) | `done` |
| Piano roll | Chord and scale helpers | `cmd:note.add_chord` `done` |
| Piano roll | Import MIDI files | `cmd:note.import_midi` |
| Patterns | New pattern | `cmd:pattern.add` |
| Patterns | Next empty pattern, then name it | `cmd:pattern.next_empty` `key:F4=ui.next_empty_pattern` |
| Patterns | Next / previous pattern | `key:Plus=pattern.next` `key:Minus=pattern.prev` |
| Patterns | Clone / rename / delete / length | `cmd:pattern.clone` `cmd:pattern.rename` `cmd:pattern.remove` `cmd:pattern.set_length` |
| Playlist | Track names, colours, mute and solo | `cmd:playlist.rename_track` `cmd:playlist.set_track_color` `cmd:playlist.toggle_track_mute` `cmd:playlist.toggle_track_solo` |
| Playlist | Place, move, resize, delete clips | `cmd:playlist.add_clip` `cmd:playlist.move_clip` `cmd:playlist.resize_clip` `cmd:playlist.remove_clip` `spec:playlist.md` |
| Playlist | Slice, make unique, merge clips | `cmd:playlist.slice_clip` `cmd:playlist.make_unique` `cmd:playlist.merge_clips` |
| Playlist | Consolidate a clip to audio | `cmd:playlist.consolidate` |
| Playlist | Reverse and fade audio clips | `cmd:playlist.reverse_clip` `cmd:playlist.set_clip_fade` |
| Playlist | Insert / delete space | `cmd:playlist.insert_space` `cmd:playlist.delete_space` `key:Ctrl+Insert=ui.insert_space` `key:Ctrl+Delete=ui.delete_space` |
| Playlist | Mute single clips | `cmd:playlist.toggle_clip_mute` |
| Playlist | Track mute | `cmd:playlist.toggle_track_mute` |
| Playlist | Select, duplicate, group move | `cmd:edit.batch` `done` |
| Playlist | Click the ruler to move the playhead; follow playback | `done` |
| Playlist | Audio clips (drop samples on the playlist) | `cmd:playlist.add_audio_clip` `done` |
| Playlist | Automation clips | `cmd:playlist.add_automation_clip` `cmd:automation.create` `cmd:automation.add_point` |
| Playlist | Markers | `cmd:playlist.add_marker` `cmd:playlist.remove_marker` `key:Alt+T=ui.add_marker` |
| Playlist | Pause and song-loop markers (stop at the end instead of looping) | `cmd:playlist.set_marker_action` `spec:playlist.md` |
| Mixer | Insert tracks | `cmd:mixer.add_track` `cmd:mixer.remove_track` `spec:mixer.md` |
| Mixer | Pan law: circular (-3 dB centre) or triangular | `cmd:project.set_pan_law` `spec:mixer.md` |
| Mixer | Stereo separation, phase invert, swap left/right | `cmd:mixer.set_separation` `cmd:mixer.set_phase_invert` `cmd:mixer.set_swap_sides` |
| Mixer | Volume / pan / mute / solo | `cmd:mixer.set_volume` `cmd:mixer.set_pan` `cmd:mixer.toggle_mute` `cmd:mixer.toggle_solo` |
| Mixer | Effect slots: add, remove, reorder, bypass | `cmd:mixer.add_effect` `cmd:mixer.remove_effect` `cmd:mixer.move_effect` `cmd:mixer.toggle_effect` |
| Mixer | Effect parameters | `cmd:mixer.set_effect_param` |
| Mixer | Peak meters | `done` |
| Mixer | Sends between inserts and buses | `cmd:mixer.set_send` `cmd:mixer.route_to_master` |
| Mixer | Sidechain compression | `fx:compressor` `done` |
| Effects | Filter, EQ, delay, reverb, compressor, distortion | `fx:filter` `fx:eq` `fx:delay` `fx:reverb` `fx:compressor` `fx:distortion` |
| Effects | Chorus | `fx:chorus` |
| Effects | Limiter | `fx:limiter` |
| Effects | Phaser, flanger | `fx:phaser` `fx:flanger` |
| Effects | Bitcrusher, stereo tool | `fx:bitcrusher` `fx:stereo` |
| Effects | Tempo-synced trance gate (16 steps) and auto-pan | `fx:gate` `fx:autopan` |
| Effects | Graphic EQ (7 bands) and parametric EQ (7 draggable bands) | `fx:graphic_eq` `fx:parametric_eq` `spec:mixer.md` |
| Effects | Bass boost, overdrive, soft clipper, waveshaper | `fx:bass_boost` `fx:overdrive` `fx:soft_clipper` `fx:waveshaper` |
| Effects | DC remover, ring modulator, frequency shifter | `fx:dc_block` `fx:ring_mod` `fx:freq_shift` |
| Effects | Vowel (formant) filter and LFO/envelope auto filter | `fx:formant` `fx:auto_filter` |
| Effects | Noise gate and transient shaper | `fx:noise_gate` `fx:transient` |
| Effects | All-in-one dynamics (gate, compressor, limiter) | `fx:dynamics` |
| Effects | Stereo shaper (mixing matrix, inter-channel delay) | `fx:stereo_shaper` |
| Effects | Multi-tap delay and stutter | `fx:multi_tap` `fx:stutter` |
| Effects | Multiband compressor | `fx:multiband` |
| Effects | Multiband limiter with a one-knob amount | `fx:multiband_limiter` |
| Effects | Hall reverb (feedback delay network, early reflections) | `fx:hall_reverb` |
| Effects | Guitar pedal chain (compressor, drive, tone, chorus, cabinet) | `fx:guitar_chain` |
| Effects | X/Y multi-effect and four-stage effect chain | `fx:multi_fx` `fx:fx_chain` |
| Effects | Beat shaper (drawn time and volume envelopes) | `fx:beat_shaper` |
| Effects | Scratch (held span, automatable position, patterns) | `fx:scratch` |
| Effects | Send effect (mid-chain send to another insert) | `fx:send` |
| Effects | Effect rack (parallel chains of effects in one slot) | `fx:rack` `cmd:mixer.rack_add` |
| Effects | Band splitter (three bands to separate inserts) | `fx:band_split` |
| Effects | Vocoder with a sidechain modulator | `fx:vocoder` `cmd:mixer.set_effect_param` |
| Effects | Pitch shifter | `fx:pitch_shift` |
| Effects | Convolution reverb with impulse files or generated rooms | `fx:convolution` `cmd:mixer.set_effect_file` |
| Effects | Spectrum analyser | `done` |
| Effects | Pitch correction and tuner | `fx:pitch_correct` `fx:tuner` `spec:mixer.md` |
| Generators | Sample editor: trim, cut, fades, gain, normalize, reverse, time-stretch, pitch-shift, tune | `cmd:sample.edit` `spec:sample-editor.md` |
| Generators | Split a sound into vocals, drums, bass and other stems | `cmd:sample.split_stems` `cmd:sample.add_stems` `cmd:sample.stems_info` `spec:stems.md` |
| Generators | Resynthesis of a sample's harmonics | `cmd:channel.add_resynth` `spec:channel-rack.md` |
| Generators | Zone sampler (samples on key and velocity zones) | `cmd:channel.add_zones` `cmd:channel.add_zone` `cmd:channel.set_zone` `spec:channel-rack.md` |
| Generators | MIDI out to external gear | `cmd:channel.add_midi_out` `cmd:channel.set_midi_channel` `spec:channel-rack.md` |
| Generators | MIDI out control changes, program change and MIDI clock | `cmd:channel.set_param` `spec:channel-rack.md` |
| Instruments | Subtractive synth with unison, sub, drive, filter types, LFO and presets | `cmd:channel.add_synth` `spec:piano-roll.md` |
| Instruments | Drum synth (kick, snare, hat, tom, clap) with presets | `cmd:channel.add_drum` `spec:channel-rack.md` |
| Instruments | Plucked string with presets | `cmd:channel.add_pluck` `spec:channel-rack.md` |
| Sampler | Loop slicing to keys and to the pattern | `cmd:channel.slice` `spec:channel-rack.md` |
| Instruments | Four-operator FM synth with presets | `cmd:channel.add_fm` `spec:channel-rack.md` |
| Instruments | Keys: piano, electric piano, organ | `cmd:channel.add_keys` `spec:channel-rack.md` |
| Instruments | Additive synth | `cmd:channel.add_additive` `spec:channel-rack.md` |
| Instruments | Granular sampler | `cmd:channel.add_granular` `spec:channel-rack.md` |
| Instruments | 16-pad drum kit | `cmd:channel.add_kit` `cmd:channel.set_pad` `spec:channel-rack.md` |
| Instruments | Layer channel playing several channels | `cmd:channel.add_layer` `cmd:channel.set_layer` `spec:channel-rack.md` |
| Instruments | Third synth oscillator | `spec:channel-rack.md` `done` |
| Instruments | Wavetable synth (Antiserum): three wavetable oscillators with warps and unison, two multimode filters, envelopes, LFOs and a mod matrix | `cmd:channel.add_antiserum` `spec:antiserum.md` `spec:antiserum-app.md` |
| Instruments | Wavetable synth: factory and imported wavetables, wavetable editor | `cmd:antiserum.set_table` `spec:antiserum-wavetables.md` `done` |
| Instruments | Wavetable synth: sample, multisample, granular and spectral oscillators | `cmd:antiserum.set_sample` `cmd:antiserum.add_zone` `spec:antiserum-oscillators.md` |
| Instruments | Wavetable synth: effects rack, arpeggiator and step sequencer | `cmd:antiserum.set_step` `spec:antiserum-fx-arp.md` |
| Instruments | Wavetable synth: drag modulation sources onto knobs, modulation rings, drawn LFO shapes | `cmd:antiserum.set_lfo_shape` `spec:antiserum-app.md` `done` |
| Instruments | Wavetable synth: factory bank of about 150 presets with tags and search; patch files | `cmd:antiserum.load_preset` `cmd:antiserum.save_patch` `cmd:antiserum.open_patch` `cmd:antiserum.presets` |
| Instruments | Wavetable synth: open Serum and Serum 2 presets and preset packs | `cmd:antiserum.import_serum` `cmd:antiserum.import_serum_presets` `spec:serum-import.md` |
| Effects | Effect windows with response graphs and presets | `spec:mixer.md` `done` |
| Instruments | CLAP plugins (instruments, effects, plugin windows) | `cmd:channel.add_plugin` `cmd:mixer.add_plugin` `cmd:plugin.list` `spec:plugins.md` |
| Instruments | VST3 plugins (instruments, effects, Linux plugin windows) | `cmd:channel.add_plugin` `cmd:mixer.add_plugin` `spec:plugins.md` `done` |
| Windows | Playlist / rack / piano roll / mixer shortcuts | `key:F5=view.toggle playlist` `key:F6=view.toggle channel_rack` `key:F7=view.toggle piano_roll` `key:F9=view.toggle mixer` `spec:keymap.md` |
| Windows | Browser | `key:Alt+F8=view.toggle browser` |
| Options | Settings window: audio device, rate, buffer, MIDI port, scale, folders, project info | `key:F10=ui.settings` `spec:settings.md` |
| Interface | Hint bar showing what the hovered control does | `done` |
| Interface | Compact layout: one-band toolbar, tools on each window's title row, narrow mixer strips | `spec:window-layout.md` `done` |
| Interface | Look and feel per the style guide: the gaps it lists for each window | `spec:style/README.md` `todo` |
| Editing | Draw, paint, delete and select tools in the piano roll and playlist | `spec:piano-roll.md` `spec:playlist.md` |
| Piano roll | Ghost notes of other channels | `spec:piano-roll.md` |
| Piano roll | Strum, humanize, legato, chop, glue, flip, snap to scale, arpeggiate | `cmd:note.strum` `cmd:note.humanize` `cmd:note.legato` `cmd:note.chop` `cmd:note.glue` `cmd:note.flip` `cmd:note.to_scale` `cmd:note.arpeggiate` |
| Channel rack | Fill every N steps, shift steps | `cmd:step.fill` `cmd:step.shift` |
| Channel | Arpeggiator (up, down, up-down, random; rate, octaves, gate) | `spec:piano-roll.md` `done` |
| Browser | Folders, search, click to preview, drag to rack or playlist | `done` |
| Browser | Plugin database: click selects, double-click or drag adds | `spec:channel-rack.md` `done` |
| Channel rack | Drop a sound, generator or preset on a channel to replace its sound | `cmd:channel.swap_sound` `spec:channel-rack.md` `done` |
| Channel rack | Step buttons: right-click erases, left/right drag paints or erases | `spec:channel-rack.md` `done` |
| Controls | Middle-click or double-click resets any knob, fader or slider | `spec:controls.md` `done` |
| Transport | Oscilloscope, CPU meter, master volume | `done` |
| Project | New / open / save / save as | `cmd:project.new` `cmd:project.load` `cmd:project.save` `cmd:project.save_new_version` `key:Ctrl+S=ui.save` `key:Ctrl+N=ui.save_new_version` `key:Ctrl+O=ui.open` `spec:project-file.md` |
| Project | Undo / redo | `key:Ctrl+Z=edit.undo` `key:Ctrl+Y=edit.redo` `spec:commands.md` |
| Project | Export WAV | `cmd:project.export_wav` `key:Ctrl+R=ui.export` |
| Project | Export stems | `cmd:project.export_stems` |
| Project | Export FLAC (lossless) | `cmd:project.export_flac` |
| Project | Export lossy audio (MP3, Ogg) | `cmd:project.export_mp3` `cmd:project.export_ogg` `spec:project-file.md` |
| Project | Autosave and crash recovery | `done` |
| Project | Import FL Studio projects (.flp) | `cmd:project.import_flp` `spec:flp-import.md` |
| Input | MIDI keyboard plays the selected channel | `done` |
| Input | Record live notes into patterns | `cmd:record.toggle` `key:R=record.toggle` `spec:recording.md` |
| Input | Recording count-in | `cmd:record.toggle_count_in` `key:Ctrl+P=record.toggle_count_in` |
| Input | Wait for input before recording | `cmd:record.toggle_wait` `key:Ctrl+I=record.toggle_wait` |
| Input | Blend or replace recording | `cmd:record.set_mode` |
| Input | Loop recording on / off | `cmd:record.toggle_loop` |
| Input | Step edit / step recording | `cmd:record.toggle_step_edit` `key:Ctrl+E=record.toggle_step_edit` |
| Input | Record knob movements as automation | `cmd:automation.record` `spec:recording.md` |
| Input | Computer keyboard as piano | `key:Ctrl+T=ui.typing_keyboard` `done` |
| Automation | Curve shapes per segment (single, double, hold, stairs) and tension | `cmd:automation.set_curve` `spec:playlist.md` |
| Project | Sample paths from Windows projects and FL Studio found on Linux and macOS | `cmd:project.localize_samples` `spec:sample-paths.md` |
| Mixer | Built-in three-band track EQ and per-slot mix | `cmd:mixer.set_eq` `cmd:mixer.set_effect_mix` `spec:mixer.md` |
| Automation | Automate any knob or fader from its context menu | `cmd:automation.create` `done` |
| Automation | Automation clip editor: tension handles, point selection, point menu, step drawing, slide, zoom | `cmd:automation.set_points` `spec:playlist.md` `done` |
| Automation | Edit automation points directly in playlist clips | `spec:playlist.md` `done` |
| Automation | Automation clip on/off and output range | `cmd:automation.set_enabled` `cmd:automation.set_range` |
| Automation | Automation clip for the last moved control | `spec:controls.md` `done` |
| Scripting | Headless CLI and scripts | `done` |
| Scripting | MCP server for AI agents | `spec:mcp.md` |
| Scripting | AI agents working in the open window (live control) | `spec:live-control.md` |
| Input | Single-key tools (P B D E C T Y Z) | `key:P=ui.tool draw` `key:C=ui.tool slice` `key:Z=ui.tool zoom` |
| Input | Slide / porta toggles for new notes | `key:S=ui.toggle_slide` `key:O=ui.toggle_porta` |
| Input | Pattern select with 1–9 | `key:1=pattern.select 1` `key:9=pattern.select 9` |
| Channel rack | Keys: mute, solo, clone, delete, move, shift steps | `spec:keymap.md` `done` |
| Channel rack | Route to a free mixer track (Ctrl+L) | `cmd:channel.route_free` |
| Piano roll | Slice notes | `cmd:note.slice` |
| Export | MIDI file export | `cmd:project.export_midi` `key:Ctrl+Shift+M=ui.export_midi` `spec:project-file.md` |
| Piano roll | Limit, LFO, scale levels, flam | `cmd:note.limit` `cmd:note.lfo` `cmd:note.scale_levels` `cmd:note.flam` |
| Piano roll | Randomizer and chord progressions | `cmd:note.randomize` `cmd:note.chords` |
| Piano roll | Articulate and claw machine | `cmd:note.articulate` `cmd:note.keep_every` |
| Channel rack | Time shift, gate and note echo | `spec:channel-rack.md` `done` |
| Transport | Master pitch / A4 tuning | `cmd:project.set_master_pitch` `cmd:project.set_a4` |
| Transport | Tap tempo | `spec:transport.md` `done` |
| Channel rack | Group filter | `cmd:channel.set_group` |
| Transport | Arm playlist tracks for audio takes; choose the input device | `cmd:playlist.toggle_track_arm` `spec:settings.md` |
| Channel rack | Step graph editor | `spec:channel-rack.md` `done` |
| Automation | Automation inside patterns (recorded in pattern mode) | `cmd:pattern.record_event` `cmd:pattern.clear_events` `spec:recording.md` |
| Automation | Controllers: LFO, envelope, keyboard, peak, formula and macro sources driving a parameter | `cmd:controller.add` `cmd:controller.set` `spec:controllers.md` |
| Plugins | Built-in instruments and Antiserum as CLAP and VST3 plugins, with the app's editors | `spec:plugin-instruments.md` `done` |
| Release | Antiserum sample oscillators play stereo samples | `spec:antiserum-oscillators.md` `done` |
| Release | Warps on sample, multisample, granular and spectral oscillators | `spec:antiserum-oscillators.md` `done` |
| Release | Unison on granular, spectral and stretched-sample oscillators | `spec:antiserum-oscillators.md` `done` |
| Release | Antiserum editor sample previews resolve project-relative paths | `spec:antiserum-oscillators.md` `done` |
| Release | Factory presets using sample, multisample, granular and spectral oscillators | `spec:antiserum-app.md` `done` |
| Release | Antiserum benchmark covers every oscillator type | `spec:antiserum-oscillators.md` `done` |
| Release | Channel header pitch knob transposes Antiserum | `spec:antiserum-app.md` `done` |
| Release | User preset folder listed in the browser, with save into it | `cmd:antiserum.save_preset` `cmd:antiserum.delete_preset` `spec:antiserum-app.md` |
| Release | Convolution reverb plugin loads impulse files | `spec:plugin-export.md` `done` |
| Release | Sidechain input and MIDI follow tested through our CLAP and VST3 hosts | `spec:plugin-export.md` `done` |
| Validation | Offline renders with our plugins match the in-app render (CLAP and VST3) | `spec:validation.md` `done` |
| Validation | clap-validator and the VST3 validator pass on both bundles in CI | `spec:validation.md` `done` |
| Validation | Denormal and CPU-budget checks for every plugin | `spec:validation.md` `done` |
| Validation | Smoke tests in REAPER (CLAP, VST3) and Ardour (VST3), scripted in `tools/host-smoke` | `spec:validation.md` `done` |
| Validation | Smoke test in Bitwig Studio and on Windows and macOS hosts, by hand | `todo` |
| Release | Plugin bundles for Linux, Windows and macOS in the release downloads, with install notes | `spec:plugin-export.md` `done` |
