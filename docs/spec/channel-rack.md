# Spec: channel rack, steps and the sampler (M2)

## Behavior

- A project has an ordered list of **channels**. Each channel has a name, an
  instrument (sampler or synth), instrument parameters, volume (linear gain 0 to
  1.25, default 0.8), pan (-1 to 1, using the project's pan law; see `mixer.md`), mute, and the mixer track it
  feeds (0 = master).
- Channels are numbered from 1 in display order. Internally each has a stable id
  so that reordering or removing channels never reattaches notes to the wrong
  channel.
- `channel.add_sampler <sample> [name]` adds a sampler. A sample is a WAV file
  path (absolute, or relative to the working directory or the project file) or a
  built-in sound `builtin:<name>`. The default name is the file stem or the
  built-in sound's name.
- `channel.add_synth [name]` adds a synth channel (see the piano roll spec).
- `channel.remove`, `channel.clone`, `channel.move`, `channel.rename`,
  `channel.set_volume`, `channel.set_pan`, `channel.set_mute`,
  `channel.toggle_mute`, `channel.solo`, `channel.route`, `channel.set_sample` and
  `channel.set_param` edit channels. All are undoable. Removing a channel removes
  its notes from every pattern; undo restores both.
- `channel.set_sample <n> <sample>` changes the sample of a sampler,
  granular or resynth channel. When the channel's name is still the default
  name of its old sample (the file stem or built-in sound's name), the name
  follows the new sample; a name the user typed is kept. Undo restores both.
- `channel.swap_sound <a> <b>` swaps the sounds of two channels: their names,
  instruments and instrument parameters. Everything else (steps and notes,
  volume, pan, mute and solo, mixer track, group, colour, the place in the
  rack and layers that use the channel) stays with the channel. Swapping
  again undoes it.
- **Replacing a channel's sound** (UI): dropping a sound, generator or preset
  from the browser onto a channel's row replaces that channel's sound, and the
  row is outlined while it hovers. A sound dropped on a sampler, granular or
  resynth channel only changes its sample (`channel.set_sample`), keeping its
  settings. Anything else (a sound on a synth, a generator or preset on any
  row) adds the new channel, gives the target its sound with
  `channel.swap_sound`, and removes the added channel, in one
  `edit.batch`: one undo step. Dropped anywhere else in the rack, the item
  adds a new channel.
- `channel.route_free <n>` routes a channel to the first mixer insert that no
  other channel feeds and that has no effects, adding an insert when none is
  free, and names that insert after the channel (one undo step).
- Keys over the rack (see `keymap.md`): 1–0 mute channels 1–10, Ctrl+1–0 solo
  them; Alt+C, Alt+Delete and Alt+Up/Down clone, delete and move the selected
  channel; Ctrl+L runs `channel.route_free`; Shift+Ctrl+Left/Right shift its
  steps.
- **Step graph editor**: the rack's Graph toggle opens a strip under the rows
  for the selected channel, one bar per step of the pattern, showing the note
  that starts on that step (the highest, when several do): velocity, pan,
  release, fine pitch, Mod A or Mod B. Dragging a bar sets that note's value
  through the usual note commands; one drag is one undo step.
- **Colour**: every channel has a colour from the 16-colour palette. Until
  the user picks one it follows the channel's identity, so reordering
  channels keeps their colours. `channel.set_color <n> <1-16|none>`
  (undoable, saved as `"color"`, 0-based; `none` returns to the automatic
  colour) sets it; UI: right-click a channel → Colour ▸ (the shared colour
  grid). The colour fills the name button in a muted shade (the selected
  channel's button in full), lights its steps and marks its notes and its
  channel window.
- **Group filter**: `channel.set_group <n> [group]` (undoable,
  saved as `"group"`; no name removes it) puts a channel in a named group. The
  rack's group menu (always at the top left of the rack) shows All, one
  group, or No group (channels without one). Right-click a channel → Group…
  sets it. Filtering only hides rows; numbering and keys stay the same. While
  one named group is shown, a channel added from the rack's plus joins it (the
  add and the `channel.set_group` are one undo step). Over the rack, Page
  Down and Page Up step the filter through All, each group and No group.
  Right-clicking the group menu offers "New group with <selected channel>…"
  (the same prompt as Group…), and for the group shown "Rename this group…"
  and "Remove this group" (danger tone):
  `channel.rename_group <group> [new name]` moves every channel of the group
  to the new name, or to no group without one, as one undo step.
- The rack's window menu shows or hides the graph editor (Ctrl+K) and the
  mixer-track column (Alt+M); both are view choices, not saved.
- **Solo** is its own state, separate from mute (saved as `"solo"`). While
  any channel is soloed, only soloed channels that are not muted play; the
  others are silenced by solo, and their mutes are left as they were.
  `channel.solo <n>` makes channel n the only soloed channel; running it on
  the only soloed channel clears every solo. `channel.set_solo <n> <on|off>`
  sets one channel's solo (both undoable). Old projects saved with the
  earlier mute-based solo keep their mutes.
- **Solo lock**: `channel.set_solo_lock <n> <on|off>` (undoable, saved as
  `"solo_lock"`) keeps a channel out of solo: another channel's solo never
  silences it, so only its own mute decides whether it plays. Shift+click on
  the mute light toggles it, and a small padlock replaces the light while it
  is on (the light's right-click menu offers it too).
- Alt+click on a grouped channel's mute light mutes or unmutes every channel
  in its group (to the opposite of the clicked channel's mute);
  Ctrl+Alt+click solos exactly that group, and again clears solo. Each is one
  undo step (a batch of `channel.set_mute` or `channel.set_solo`).
- **Steps** are sixteenth notes. `step.toggle <channel> <step>` (0-based step)
  adds a one-step note at key C5 (60) and velocity 100 if the channel has no note
  starting there, otherwise removes every note of that channel starting there.
  `step.set` forces a state. Steps and piano-roll notes are the same data.
- **Step buttons** (UI): left-click toggles a step; dragging with the left
  button across a channel's row paints every step it passes to the state
  the first click gave (on or off). Right-click turns a step off, and
  dragging with the right button erases every step it passes. A whole drag
  is one undo step. Turning a step on previews the channel's sound.
  A lit step is filled with the channel colour, one step darker in odd beat
  groups so the grouping survives, and dimmer for a lower velocity (with a
  readable minimum); a lit step under the playhead flashes bright as the
  playhead enters it and fades back over the step's duration (own design).
  Melodic rows show a miniature roll whose alternate bars are shaded; a click
  on it opens the piano roll on that channel, or closes it when it already
  shows that channel.
- Holding Ctrl while pressing a channel's name previews it (C5, held until
  the button is released) and opens nothing. Shift+wheel over a name moves
  that channel one slot per notch (`channel.move`, as Alt+Up / Alt+Down).
- Right-clicking a channel's name opens its menu in two columns. First: Open
  in piano roll, Channel settings; Rename…, Change sample… (samplers),
  Colour ▸, Group…; Clone, Delete. Second: Cut steps, Copy steps, Paste
  steps (the same as Ctrl+X / C / V over the rack, for this channel), Clear
  notes in pattern; Turn on every 2nd / 4th / 8th step; Shift left, Shift
  right; Graph editor (selects the channel and opens the graph), Solo, Move
  up, Move down.
- Shift+wheel over the rack's header row steps to the next or previous
  pattern (`pattern.next`, `pattern.prev`).
- The mute light: a click mutes or unmutes, Ctrl+click solos the channel
  (`channel.solo`), and right-click offers Mute or Unmute, Solo or Unsolo,
  and Unmute all. Lit: the channel plays. Dark: muted. A soloed channel's
  light has a ring in the highlight tone; a channel silenced by another's
  solo shows its light at reduced strength.
  The pattern length box takes 1 to 1024 steps; right-click it for Auto, or
  1, 2, 4, 8, 16 or 32 whole bars in the pattern's time signature. With Auto
  (`pattern.set_length <pattern> auto`, saved as `"auto_length"`) the length
  is whole bars up to the last bar with a note starting in it, at least one
  bar, and the box shows the steps that comes to with "auto" after them.
  Dragging the box or picking a bar length turns Auto off.
- **Row layout** (UI), left to right: mute dot, pan knob, volume knob, the
  mixer track box, the name button, the steps (or a miniature roll), and a
  **selection switch** that selects the channel without opening its
  settings (lit for the selected channel).
- **Mixer track box** (UI): each row shows the mixer track the channel plays
  to as a small number box: "M" for the master track, "3" for insert 3. Drag
  it up or down (or type) to change the track; the tooltip names the track.
  Channel stacks and MIDI outputs make no sound of their own, so their box
  is a dim dash.
- **Browser** (UI): in the plugin database, a single click only selects an
  entry (it is highlighted). Double-clicking a generator adds it as a new
  channel; dragging it onto the channel rack does the same. Double-clicking
  an effect adds it to the selected mixer track. Sounds in packs and folders
  preview on a single click and are added on a double click or by dragging,
  as before.

- **Next empty pattern**: `pattern.next_empty` selects the first pattern
  after the selected one that has no notes; when there is none it adds a new
  pattern (as `pattern.add`, undoable) and selects it. Selecting an existing
  pattern is not undoable. F4 runs it and then offers to name the pattern
  (a rename prompt; Escape keeps the name).
- **First empty pattern**: `pattern.first_empty` does the same from the
  first pattern on, so it finds gaps before the selected one too. Shift+F4
  and a right-click on the toolbar's new-pattern button run it.
- **Swing** is one amount for the whole project: `project.set_swing <0-100>`
  (undoable; saved as `"swing"`, 0–1). Notes that start exactly on an off-beat
  sixteenth (steps 1, 3, 5, …) are delayed by up to half a step: 12 ticks ×
  swing × the channel's **swing mix**. Other notes are unaffected. It applies
  to pattern notes in pattern and song mode, not to audio clips.
  - `swing_mix` (channel parameter, 0–1, default 1 = 100%; `channel.set_param
    <n> swing_mix <v>`) scales the global swing per channel: 0 keeps a channel
    straight.
  - `swing_truncate` (channel parameter, 0 or 1, default 0): a swung note
    keeps its original end and so gets shorter by the delay (never below one
    tick) instead of moving whole, so it cannot run into the next step.
  - Both are in channel settings → Functions → TIMING, captioned "Swing"
    (the mix) and "Delay" / "Shorten" (the truncate choice). The channel
    rack's swing box sets the global amount.
  - Older files stored swing per pattern. On load, a project without a global
    swing takes the largest pattern swing.

## Timing and echo

Channel parameters (channel settings → Functions → TIMING and ECHO), applied when the
sequence is built, after swing and the arpeggiator; they change what the
channel plays, not the stored notes:

- `time_shift` (-96…96 ticks; caption "Delay") moves every note of the channel later
  (negative: earlier, never before the loop start).
- `gate` (0…384 ticks, 0 = off; caption "Length") gives every note that fixed length.
- Note echo: `echo_count` (0–8) repeats each note `echo_time` ticks apart
  (default 48), each repeat's velocity multiplied by `echo_feed` (0–1,
  default 0.5) and its key moved by `echo_pitch` semitones (-12…12). Repeats
  past the loop end, below velocity 1 or outside the key range are dropped.
  Echoes are never slide notes. Captions: "Repeats", "Time", "Feedback",
  "Pitch step".

## Voices: cut groups, mono and polyphony

Channel parameters (`channel.set_param <n> <name> <value>`, undoable; channel
settings → Functions → VOICES and CHOKE). They apply to the built-in instruments; plugin and MIDI
Out channels voice their own notes and are never cut.

- **Cut groups** (choke): `cut_group` (0–16, 0 = none) is the group a
  channel's notes cut; `cut_by` (0–16, 0 = none) is the group that cuts the
  channel. When a note starts on a channel whose `cut_group` is G, every
  sounding voice of every channel whose `cut_by` is G is cut, on the new
  note's own frame. A closed hi-hat with `cut_group 1` and an open hi-hat with
  `cut_by 1` makes the closed hat choke the open one; giving both channels
  `cut_group 1` and `cut_by 1` makes each choke the other (and itself).
  The CHOKE box captions them "Group" and "Silenced by".
- **Mono** (`mono` 0 or 1): a new note cuts every other voice of the channel,
  so one note sounds at a time. With the synth's `glide`, the new note still
  glides from the note it replaces.
- **Max polyphony** (`poly_max`, 0–64, 0 = no limit): when a note starts and
  the channel already has that many voices sounding, the oldest are cut until
  the new note fits. Mono is the same as a limit of 1.
- A **cut** fades the voice out over 3 ms instead of its release time (one-shot
  samples included), so it never clicks. A voice fading out after a cut no
  longer counts toward the polyphony limit.
- Slide notes start no voice, so they cut nothing.

## Sampler

- `reverse` (0 or 1) plays the sample backwards from its end; `fade_in` and
  `fade_out` (0–10 s) fade each note in from its start and out over its last
  seconds (before the note or the sample ends). Used by audio clips (see
  `playlist.md`, "Clip editing"); sliced samplers and kit pads ignore
  `reverse`.
- Plays the sample from its start. The pitch follows the note key relative to the
  `root_key` parameter (default 60) plus the `pitch` parameter in semitones.
  Samples of any rate are resampled by linear interpolation.
- With `one_shot` on (the default) the whole sample plays regardless of note
  length; otherwise the note's end starts the envelope release.
- The sampler applies the shared ADSR envelope; new samplers default to instant
  attack, full sustain and a short release so samples play unshaped.
- Missing or undecodable files play silence and produce a warning.

## Slicing

- The sampler's `slices` parameter (1–64, default 1) cuts the sample into
  equal slices. With more than one, the key `root_key + i` plays slice `i`
  (wrapping) at its original pitch, from the slice start to its end, as a
  one-shot; `pitch` still transposes.
- `channel.slice <channel> <slices> [@pattern]` sets `slices` and rewrites the
  channel's notes in the pattern so slice `i` plays at `i` × (pattern length
  ÷ slices): the loop plays back as it was, now editable note by note. One
  undo step.

## Drum synth

- `channel.add_drum [kick|snare|hat|tom|clap] [name]` adds a channel that
  synthesizes a drum on every note (default kick). Parameters: `drum_type`
  (0 kick, 1 snare, 2 hat, 3 tom, 4 clap), `drum_tune` (±24 semitones),
  `drum_decay` (0.02–2 s), `drum_sweep` (pitch drop in semitones, 0–48),
  `drum_tone` (noise against tone for snares; brightness for hats and claps),
  `drum_click` (a short noise attack, 0–1).
- Kicks and toms are a sine whose pitch falls by `drum_sweep` semitones in the
  first ~40 ms; the note key sets the pitch (C5 plays the tuned default, two
  octaves down for kicks and one for toms). Snares mix a tone with noise;
  hats are high-passed noise; claps are three short noise bursts then a tail.
  Every hit decays exponentially over `drum_decay` and ignores note length.
- Presets: a set of kicks, snares, hats, toms and claps.

## Pluck

- `channel.add_pluck [name]` adds a plucked-string channel (Karplus–Strong:
  a burst of noise circulating in a tuned, damped delay line). Parameters:
  `pluck_decay` (time to fade by 60 dB, 0.1–10 s), `pluck_damping` (0
  bright to 1 dark), `pluck_tone` (brightness of the initial burst). Ending
  the note damps the string quickly. Presets: guitar, harp, bass, muted,
  bowed and twelve-string.
- `pluck_bow` (0–1) bows the string: while the note is held, low-passed
  noise (brightness from `pluck_tone`, level following the channel
  envelope) keeps feeding the string, so it sustains; releasing stops the
  bow and the string damps as usual. `pluck_strings` (1–3) sounds extra
  strings `pluck_detune` cents above and below the note (each with its own
  burst), mixed at equal power.

## FM synth

- `channel.add_fm [name]`: four sine operators. `fm_algo` picks how they
  connect (0: 4→3→2→1; 1: 4→3, 3 and 2 → 1; 2: two stacks 4→3 and 2→1;
  3: 4 modulates 1, 2 and 3; 4: 2→1 plus 3 and 4 as tones; 5: all four as
  tones). Each operator has `opN_ratio` (0.25–16 × the note), `opN_level`,
  `opN_decay` (s) and `opN_sustain`; operator 4 has `fm_feedback`. Tones
  follow the channel envelope; modulators shape the timbre over time.
  Presets: e-piano, bell, bass, brass, organ, pluck, metallic, plus six-operator
  ones.
- Six operators: algorithms 6–11 also use operators 5 and 6 (same four
  parameters each): 6→5→4→3→2→1; two stacks 6→5→4 and 3→2→1; three pairs
  6→5, 4→3, 2→1; 2–6 all into 1; 6 into each of 1–5; all six as tones.
  With six operators the feedback is on operator 6. Algorithms 0–5 ignore
  operators 5 and 6, so older sounds are unchanged.
- `opN_wave` (operators 1–6): sine, half sine, soft square or soft saw.
- `fm_cutoff` and `fm_resonance`: a low-pass filter on the output, off at
  20 kHz.

## Keys

- `channel.add_keys [piano|epiano|organ] [name]`: `keys_type`, `keys_tone`
  (brightness), `keys_decay` (how long a held note rings), `keys_tremolo`
  and `keys_hammer` (attack noise; organ: percussion). The piano sums slightly
  stretched partials whose upper ones fade faster; the e-piano is a decaying
  FM tine with a bell overtone; the organ mixes nine drawbar-like harmonics.
  Release follows the channel envelope.

## Additive

- `channel.add_additive [name]`: up to 32 harmonics (`add_partials`) with
  `add_bright` (spectral slope), `add_even` (level of even harmonics),
  `add_inharm` (stretch) and `add_decay` (upper harmonics fade faster).
- A second spectrum (`add_bright_b`, `add_even_b`, `add_inharm_b`) and a
  morph between them: `add_morph` (0 the first spectrum, 1 the second),
  `add_morph_time` (seconds over which each note moves from `add_morph` to
  the second spectrum; 0 off) and a sine LFO on the position
  (`add_morph_rate` Hz, `add_morph_depth`). Each harmonic's level and the
  stretch are interpolated; both spectra are normalized first, so morphing
  keeps the level.

## Third oscillator

- The synth has a third oscillator: `wave3`, `osc3_level` (0 = off) and
  `osc3_coarse` (±24 semitones), mixed after the first two.

## Granular sampler

- `channel.add_granular <sample> [name]` plays a sample as overlapping
  windowed grains: `gran_size` (5–500 ms), `gran_density` (grains per
  second), `gran_position` (0–1 in the sample), `gran_spread` (random
  position range), `gran_pitch_rand` (random detune, semitones) and
  `gran_scan` (how fast the position moves while held, −2…2 × real time).
  Pitch follows the key around `root_key`.

## Resynth

- `channel.add_resynth <sample> [name]` measures the sample's harmonics once
  (when the channel is built): the median pitch it hears is its fundamental
  (220 Hz if it hears none), and Hann-windowed spectra up to 512 times along
  the sample give the level of harmonics 1–32 (the strongest bin within two
  of each harmonic), normalized to the loudest. Notes play those levels as
  sines on the note's own harmonics, so any sample becomes a pitched
  instrument that keeps its tone changing over time.
- `resyn_speed` (0 freezes, 1 as recorded, up to 4), `resyn_position`
  (start, 0–1 of the sample), `resyn_blur` (0–1: levels follow the analysis
  over up to half a second, smearing changes), `resyn_loop` (hold the last
  frame or loop), and from the additive synth `add_partials` and
  `add_inharm` (stretches the harmonics apart). Levels update every 32
  samples. `channel.set_sample` changes the sample. Browser samples offer
  "Add as resynth" in their right-click menu.

## Zone sampler

- `channel.add_zones [name]` adds a sampler that plays several samples, each
  in a **zone**: a key range (`low`–`high`), a velocity range (`vel_low`–
  `vel_high`, 1–127) and the `root` key at which the sample plays at its
  recorded pitch. A note plays the first zone (in list order) whose ranges
  contain its key and velocity; when none does it is silent. One-shot,
  fades, reverse, envelope and pitch work as on the sampler.
- `channel.add_zone <channel> <sample> [low] [high] [root]` appends a zone
  (defaults: the whole keyboard, root C5 = 60, all velocities);
  `channel.remove_zone <channel> <zone>`, `channel.insert_zone <channel>
  <zone> <json>` (undo) and `channel.set_zone <channel> <zone>
  <low|high|root|vel_low|vel_high|sample> <value>` edit them. All undoable.
- The editor lists the zones with their sample and ranges as dragged
  numbers, a delete button per zone and "Add zone…" (asks for a sample).

## Drum kit

- `channel.add_kit [name]` adds a 16-pad sample kit; key `root_key + n`
  plays pad `n + 1` (one-shot). `channel.set_pad <channel> <pad> <sample>`
  loads a pad (`-` clears it). A new kit loads the built-in sounds on pads
  1–8.

## Layer

- `channel.add_layer [name]` adds a channel that plays nothing itself but
  passes every note (sequenced and live) to the channels set with
  `channel.set_layer <channel> <channel,channel,…>` (`-` for none). Layers
  do not nest.

## MIDI Out

- `channel.add_midi_out [1-16] [name]` adds a channel that makes no sound but
  sends its notes to external MIDI gear on a MIDI channel (default 1).
  `channel.set_midi_channel <channel> <1-16>` changes it (undoable).
- Sequenced notes send note on (velocity 1–127 from the note's velocity)
  at their start and note off after their length; live notes (MIDI keyboard,
  typing keyboard, rack audition) send note on and off as they are played.
  Layers can play MIDI Out channels like any other.
- A muted MIDI Out channel sends nothing new; notes already sounding still
  get their note off. Stop, pausing and rebuilding the channels send note off
  for every held note. Volume, pan, mixer routing and instrument parameters
  do not apply.
- The engine queues the messages on the audio thread (fixed-size queue, a
  full queue drops messages) with their offset into the block; the app sends
  them from a separate thread to the **MIDI output port** chosen in
  Settings → MIDI (none by default). Timing is accurate to about one audio
  buffer. Offline renders and exports leave MIDI Out channels silent and
  send nothing.
- Eight control-change knobs (`midi_cc1` … `midi_cc8`, 0–127; −1, the
  default, sends nothing) with their controller numbers (`midi_cc1_number`
  …, defaults 1, 7, 10, 11, 71, 74, 91, 93) and `midi_program` (0 none,
  1–128). The engine sends a control change or program change whenever the
  value differs from the last one it sent (and again after the channels are
  rebuilt), so automation and controllers drive external gear; a muted
  channel sends none.
- **MIDI clock** (Settings → MIDI, off by default): while playing, 24 clock
  messages per quarter note go to the MIDI output, with Start when playback
  begins at the song start, Continue when it resumes elsewhere and Stop when
  it stops or pauses.

## Built-in sounds

`kick`, `snare`, `clap`, `hat`, `open_hat`, `tom`, `rim`, `crash`. Each is
synthesized deterministically at load time (swept sines, filtered noise, metallic
square partials), normalized to a 0.9 peak and faded out at the end.

## Channel settings window

- Clicking a channel's name opens its settings window; the one window
  follows the clicked channel and keeps its page, and clicking the shown
  channel again closes it. Alt+click opens an additional window for that
  channel and leaves the others open; each additional window stays on its
  own channel and page, cascades a little down and left of the first, and
  closes on its own (one per channel). With Settings → General → "Keep
  instrument windows open" on, a plain click on another channel does the
  same as Alt+click while a channel window is open. Its header title is
  the channel's name, an em dash and the mixer track it feeds ("Lead —
  Insert 3", or "Kick — Master"). For instruments with factory presets
  (synth, drum synth, pluck, FM, keys, additive) the header also holds the
  presets: previous, the list ("(custom)" when none matches) and next;
  right-click on either arrow opens the list. Each choice is one undo step. The exported plugins, which have no header, keep a
  preset row at the top of the Sound page.
- Under the header, one fixed **strip** stays at the top:
  - At the left, the **tabs** as square glyph cells: *Sound* (a sine wave:
    the instrument's own sections, such as sample, oscillators, operators or
    plugin parameters), *Envelope* (an envelope shape: the amplitude
    envelope, for instruments that use it) and *Functions* (three climbing
    squares). The active cell takes the page's tone, so it reads as joined
    to the page. Each cell's hint names the page; the Sound hint also names
    the instrument.
  - At the right, flush right: the **On** light (lit while the channel plays;
    click toggles mute), Pan (shared pan text, "25% left" / "Centre"),
    Volume, Pitch, a gap, then the **Track** box (drag or type,
    `channel.route`; "M" is the master). Pitch stays in its place for hosted
    plugins, drawn disabled, so the strip never shifts. Clicking the
    "Track" caption routes to a free mixer track (`channel.route_free`).
    Double-clicking the box opens the mixer with that track selected.
- The **Envelope** page shows the envelope graph over its four knobs
  (Attack, Decay, Sustain, Release). The graph is drawn in the channel colour
  with a round handle at the peak (drag across: attack), the decay corner
  (across: decay; up or down: sustain) and the end (across: release). Each
  timed stage has a quarter of the width on a square-root time scale, so a
  drag maps back to the same time. One drag is one undo step.
- The Envelope and Functions pages end with a **KEYBOARD** box for
  instruments tuned by the root key (not plugins or Antiserum): "Root C5"
  with a Reset button (back to C5), then a piano from C1 to B8 with the root
  key in the accent tone and a label at each C. Left-click a key to hear it
  while held; right-click a key to make it the root key
  (`channel.set_param <n> root_key <key>`, undoable).
- The **page** under the strip is the selected tab's contents: Functions
  tiles its boxes two to a row (VOICES | TIMING with swing, ARPEGGIATOR full
  width, CHOKE | ECHO). Only the page scrolls. The last tab used stays
  selected while the window is open.
- MIDI output channels have no sound settings: their strip shows the Sound
  tab only, the On light and a "MIDI ch" box (1–16,
  `channel.set_midi_channel`), and the page is the MIDI page. A channel
  stack's strip shows the Sound tab and the On light only.
- The window is never narrower than its strip. Wide pages (Antiserum) widen
  it to at most half the workspace.

## Sources

- The channel settings window's fixed header and tabbed pages follow the
  layout described in the FL Studio online manual, "Channel settings"
  (sampler and instrument pages, miscellaneous functions), with our own
  grouping and names.

- Channel rack layout (rows of instruments with step buttons, volume, pan, mute
  and mixer routing) follows the general workflow described in the FL Studio
  online manual, "Channel Rack" section. Command names, value ranges and defaults
  are our own.
- Step buttons that toggle on left-click, erase on right-click and paint or
  erase when dragged follow the step sequencer workflow described in the FL
  Studio online manual, "Channel Rack". The routing box's labels and the
  browser's select-then-double-click behaviour are our own design.
- Replacing a channel's sound by dropping a browser item on its row follows
  the FL Studio online manual, "Channel Rack" and "Browser" (dragging a
  sample or plugin onto a channel button), with the owner's request of
  2026-10-23. What is kept, the rename rule and the commands are our own
  design.
- The solo lock (Shift+click on the mute light) follows the FL Studio online
  manual, "Channel Rack", "12. Mute Control"; the padlock is our own glyph.
- F4 selecting the next empty pattern and offering to name it follows the FL
  Studio online manual, "Keyboard shortcuts" and the pattern selector.
  "Empty" meaning "no notes" and adding a pattern when none is left are our
  own choices.
- Cut groups ("cut" and "cut by"), mono and maximum polyphony are channel
  settings described in the FL Studio online manual, "Channel settings",
  miscellaneous functions. The 3 ms fade, the group range and the counting
  rules are our own.
- Swing: the FL Studio online manual describes one main swing control, a
  per-channel swing mix (default 100%) and a truncate option for swung
  notes. The delay curve, the truncate rule and the migration are our own.
- MIDI Out: note on/off messages per the MIDI 1.0 specification (MIDI
  Association); the channel kind mirrors the "send notes to external gear"
  generator listed in the FL Studio online manual's plugin list. Design ours.
- Built-in sounds: own design; no third-party audio is used.
- WAV decoding: the RIFF/WAVE format via the `hound` crate.
- Time shift, gate and note echo are channel settings described in the FL
  Studio online manual, "Channel settings" (miscellaneous functions: time,
  echo delay). Units, ranges and rules are our own.
- The group filter follows the channel rack filter groups described in the
  FL Studio online manual, "Channel Rack". "No group" and the menu are our own.
- The step graph editor follows the channel rack graph editor described in the
  FL Studio online manual, "Channel Rack" (graph editor). Its properties match
  our piano roll lane; the layout is our own.
