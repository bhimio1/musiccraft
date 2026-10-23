# Spec: piano roll, notes and the synth (M4)

## Notes

- A pattern holds notes for any number of channels. A note has a start and length
  in ticks (96 per beat, 24 per step), a key 0–127, and a velocity 1–127.
- At most one note per channel, start and key: adding a note at an occupied
  position replaces it.
- Key arguments accept MIDI numbers or names with key 60 = `C5` (octave = key/12,
  sharps `#`, flats `b`).
- Time arguments are in steps; fractions are allowed and `<n>t` is an exact tick
  count.
- Commands: `note.add`, `note.remove`, `note.move`, `note.resize`,
  `note.set_velocity`, `note.clear`, `note.set_all`, `note.transpose`,
  `note.quantize`. Every note and step command acts on the selected pattern unless
  a trailing `@<pattern>` names another. Their inverses always name the pattern,
  so undo is correct after switching patterns.

## MIDI file import

- `note.import_midi <channel> <file.mid> [@pattern]` adds every note of a
  Standard MIDI File (format 0 or 1; all tracks and MIDI channels) to one channel
  in the pattern. File ticks are scaled to 96 per beat. Notes left sounding at a
  track's end stop there. SMPTE-timed files are rejected. One undo step.

## Chords, scales and recording

- `note.add_chord <channel> <start> <length> <root> <quality> [velocity]` adds
  the notes of a chord as one undo step, built upward from the root. The
  qualities come in six families:
  - Triads: major, minor, dim, aug, sus2, sus4, power (root and fifth),
    power8 (power plus the octave), majb5.
  - Sixths: 6, min6, 6/9, min6/9.
  - Sevenths: maj7, min7, dom7, dim7, min7b5 (half-diminished), minmaj7,
    aug7, augmaj7, 7b5, maj7b5, 7sus2, 7sus4.
  - Ninths: add9, minadd9, dom9, maj9, min9, minmaj9, 9sus4, 7b9, 7#9, 7#5,
    7#5b9.
  - Elevenths: add11, minadd11, dom11, maj11, min11, 7#11, maj7#11, 9#11.
  - Thirteenths: dom13, maj13, min13, 13sus4, 7b13, 13b9, 13#11.
  Extended chords keep every tone up to the named one, except that 13ths drop
  the natural 11th (it clashes with the major third). A chord that would go
  above key 127 is refused. The piano roll's chord menu (header button and
  window menu) shows "Note" and then one column per family; picking a chord
  makes a click draw it.
- The scale menu highlights a scale (major, natural and harmonic minor,
  dorian, mixolydian, major and minor pentatonic) on any root. The usual
  black/white key-row shading is replaced: rows in the scale are a step
  lighter than white-key rows, the root's rows also get a faint accent, and
  rows outside the scale are drawn like black-key rows. The last item of the
  scale menu, "Fit edited notes to the scale" (a tick, off by default, only
  while a scale is highlighted), makes drawn and moved notes land on the
  nearest key of the scale (ties go down, as in `note.to_scale`); the scale
  button is lit while it is on.
- With recording armed (REC button or R) and playback running, notes played on
  a MIDI keyboard or the typing keyboard are written into the selected pattern on
  the selected channel at the playhead position (folded into the pattern length
  in song mode). Each recorded note is one undo step. Notes held across the loop
  point keep their full length.

## Note properties and kinds

- Besides start, length, key, velocity, pan and slide, every note has:
  `release` (release velocity 0–127, default 64: lower is a shorter release,
  higher a longer one, up to ×4), `fine` (−100…100 cents), `mod_x` and `mod_y`
  (0–127, default 64; on the synth they offset the filter cutoff by up to ±3
  octaves and the resonance), `color` (1–16, a colour group; groups slide
  independently, see slide notes; plugins receive notes on one channel),
  `porta` and `muted`.
- `note.set_prop <channel> <start> <key> <prop> <value> [@pattern]` sets one
  (`release`, `fine`, `mod_x`, `mod_y`, `color`, `porta on|off`,
  `muted on|off`); undoable.
- **Portamento notes** start a new voice that glides from the pitch of the
  channel's previous note (or from where a slide ended) over the channel's
  `glide` time (0.1 s when that is 0). Unlike slide notes they retrigger the
  envelope. They glide on every instrument whose pitch can move (see slide
  notes); on the others they play normally.
- **Muted notes** stay in the pattern but do not play; they are drawn hollow.
  The piano roll's Mute tool toggles muting on the notes it clicks or sweeps.
- The bottom lane edits velocity, pan, release, fine pitch, Mod A or Mod B
  (the note properties `mod_x` and `mod_y`).
- Pointer shapes: a crosshair over empty grid (a cell cursor with Ctrl or the
  Select tool), a four-way move cursor over a note body (grabbing while a
  move drags), horizontal resize over a note's right end, and the plain
  pointer for the Delete, Mute, Slice, Play and Zoom tools. The ruler and the
  keyboard show a pointing hand.
- Pressing a note with the Draw or Paint tool makes its length and velocity
  the ones the next new notes take.
- New notes take the toolbar's colour and Portamento setting. Fine pitch,
  release velocity and Mod A/B apply to the built-in instruments; plugins get
  the note's key and velocity.
- Project files store only properties that differ from their defaults.

## Slide notes

- A note can be a **slide note** (`note.set_slide <channel> <start> <key>
  <on|off>`, undoable; stored as `"slide": true`). A slide note never starts
  a voice of its own: it bends the notes it overlaps.
  - It affects the notes of its channel and **colour group** that are sounding
    when it starts (including notes that start on the same tick). Each colour
    group slides independently, so a chord can carry up to 16 separate slides.
  - A slide note that overlaps no note of its group is **silent**.
  - The **top note** (highest key) of the group sets the offset: it slides to
    the slide note's key, and every other note of the group moves by the same
    number of semitones, keeping its interval. Notes keep the new pitch after
    the slide, until they end.
  - The **overlap sets the slide time**: each note glides from the slide
    note's own frame until the slide note or that note ends, whichever comes
    first. A slide never lengthens a note: it stops when the note being slid
    ends.
  - Several slide notes of one group starting together: the highest wins.
  - Live notes held on a keyboard can be slid by sequenced slide notes; they
    glide over the whole slide note.
- Works on every built-in instrument whose pitch can move: synth, sampler, FM,
  keys, additive and granular (new grains follow the glide). Kit pads, the drum
  synth, the pluck and sliced samples fix their pitch when a note starts, so a
  slide note plays there as a normal note; plugins and MIDI Out channels get a
  normal note too.
- In the piano roll, the Slide toggle on the toolbar makes new notes slide
  notes, and Tools → Toggle slide flips the selected notes. Slide notes are
  drawn with a ramp mark at their right end (portamento notes: a small
  curve there).

## Shaping and generating tools

Each rewrites one channel's notes in the pattern as one undo step (Tools →
Shape / Generate in the piano roll use the defaults in brackets).

- `note.limit <channel> <low> <high>`: notes outside the key range move by
  octaves into it (clamped when the range is under an octave) [C4–B5].
- `note.lfo <channel> <velocity|pan|fine|mod_x|mod_y> <period> <depth>
  [sine|triangle|square|saw]`: sets the property from an LFO over each note's
  start time, swinging around the property's centre (velocity and Mod A/B
  64, pan and fine 0) by `depth` percent of the room on either side [one bar,
  50% velocity or 80% pan, sine].
- `note.scale_levels <channel> <prop> <percent> [offset]`: scales the
  property's distance from its centre and adds an offset, clamped [velocity
  75%].
- `note.flam <channel> <spacing> [count] [velocity percent]`: adds up to 8
  grace notes before each note, `spacing` apart, each quieter by the percent
  (default 60%); notes too close to the start get none [6 ticks, 1].
- `note.randomize <channel> <low> <high> <density> [seed] [root scale]`:
  replaces the notes with one-step notes on the step grid: each step gets a
  note with probability `density` %, a random key in the range (moved into the
  scale when given) and velocity 70–127. The same seed gives the same notes
  [C4–C6, 50%, highlighted scale].
- `note.chords <channel> <root> <major|minor> <degrees> [length]`: replaces
  the notes with triads stacked in thirds on scale degrees 1–7 (`1-5-6-4`),
  one per `length` (default a bar) [I–V–vi–IV, i–VI–III–VII, ii–V–I in the
  highlighted scale's key].
- `note.articulate <channel> <length percent> [accent]`: sets each note to a
  share of the gap to the next note start and adds `accent` to the velocity
  of notes on a beat [staccato 50% +10, tenuto 100%].
- `note.keep_every <channel> <every> [offset]`: keeps every Nth start time (a chord
  counts once) from `offset` and drops the rest [every 2nd].

## Arpeggiator

- Every channel has an arpeggiator (channel settings → ARPEGGIATOR):
  `arp_mode` (off, up, down, up-down, random), `arp_rate` (1/32, 1/16, 1/8,
  1/4 or 1/2 note), `arp_range` (1–4 octaves) and `arp_gate` (length of each
  arpeggio note as a share of the rate).
- When on, the channel's sequenced notes become an arpeggio when the
  sequence is built: on every rate step of the song grid, the keys held during
  that step (spread over the octave range) play one at a time in the mode's
  order, at the loudest held velocity. The order restarts when nothing is
  held. Random order is the same every time it plays.
- It applies to pattern and song playback and to export, for every kind of
  instrument. Live MIDI notes are not arpeggiated.
- Presets leave the arpeggiator settings alone.

## Note tools

Each tool rewrites all notes of one channel in a pattern as one undo step
(the inverse is `note.set_all`). Times are steps unless written `<n>t`.

- `note.strum <channel> <spread> [up|down]`: notes that start together are
  staggered by `spread` per note, lowest first (`up`, default) or highest
  first; each keeps its end time (but stays at least 1 tick long).
- `note.humanize <channel> <timing> <velocity> [seed]`: moves each start by up
  to ±`timing` and each velocity by up to ±`velocity` (1–127). The same seed
  (default 1) always gives the same result.
- `note.legato <channel>`: every note lasts until the next later start; the
  last notes keep their length.
- `note.chop <channel> <length>`: splits notes into pieces of `length`; a
  shorter remainder is kept.
- `note.glue <channel>`: joins notes of the same key where one ends exactly
  where the next begins, keeping the first one's velocity and pan.
- `note.flip <channel> time|pitch`: mirrors the notes in time within the
  pattern length, or upside down around the middle of their key range.
- `note.to_scale <channel> <root> <scale>`: moves each note to the nearest
  key of the scale (ties go down). Scales as in the piano roll's highlight.
- `note.arpeggiate <channel> <step> [up|down|updown|random] [seed]`: every
  group of notes starting together becomes one note at a time, `step` long,
  cycling through the group's keys for the group's longest duration.
- Step tools: `step.fill <channel> <every> [offset]` turns on every `every`th
  step from `offset` (0-based) across the pattern, keeping existing notes;
  `step.shift <channel> <steps>` rotates the channel's notes by whole steps,
  wrapping at the pattern length.
- Notes that end up on the same start and key keep only the first.
- In the piano roll these live in the toolbar's options menu (☰) → Tools; in the channel
  rack, in a channel's right-click menu.

## Pattern length

- A pattern has a minimum length in steps (default 16). When notes start past it,
  the pattern extends to whole bars to fit them. An auto-length pattern has no
  minimum beyond one bar: it is whole bars up to its last note (see
  `channel-rack.md`); step fill and shift then work over that length.

## Synth

- Two oscillators (`wave1`, `wave2`: sine, saw, square, triangle, noise; saw and
  square are band-limited with PolyBLEP). Oscillator 2 is detuned by `detune`
  cents; `osc_mix` crossfades between them.
- `unison` (1–7) stacks copies of both oscillators detuned across `spread`
  cents and spread across the stereo field; the stack is normalized so more
  voices are not louder. `sub` adds a sine one octave below.
- `drive` saturates (tanh) before the filter.
- A resonant state-variable filter per side (`filter_type`: low-, band- or
  high-pass; `cutoff`, `resonance`) whose cutoff is modulated by the amplitude
  envelope by `env_amount` octaves.
- A sine LFO per voice (`lfo_rate` Hz, `lfo_depth` 0–1, `lfo_target`): pitch
  (up to ±2 semitones), cutoff (up to ±3 octaves) or volume (tremolo).
  Modulation, glide and filter coefficients update every 32 samples.
- A second filter (`filter2_type`: off, low-, band- or high-pass; `cutoff2`,
  `resonance2`) with `filter_routing`: series (filter 1 then filter 2),
  parallel (both hear the whole mix, outputs averaged) or split (oscillator 1
  and the sub go through filter 1, oscillators 2 and 3 through filter 2, so
  each oscillator gets its own filter). With filter 2 off the routing is
  ignored and the synth sounds as before.
- A second LFO (`lfo2_rate` Hz, `lfo2_shape`: sine, triangle, saw, square,
  random steps), a modulation envelope (`env2_attack`, `env2_decay` seconds;
  rises 0→1 then falls back to 0, independent of the note length) and a
  three-slot modulation matrix: each slot has `modN_source` (off, LFO 1,
  LFO 2, mod envelope, velocity, key — −1 two octaves below C5 to +1 two
  octaves above, amp envelope), `modN_dest` (pitch ±12 semitones, cutoff
  ±4 octaves, cutoff 2 ±4 octaves, resonance ±1, oscillator mix ±1, volume
  (gain × 1 + amount × source, 0–2), pan ±1, oscillator 2 detune ±100
  cents, oscillator 3 level ±1) and `modN_amount` (−1–1). LFO sources are
  their raw −1–1 shape, independent of `lfo_depth`. Slots add up.
- **Presets**: the synth editor offers factory presets (bass, lead, pad, pluck,
  keys, brass, riser…). Applying one sets every synth parameter in one undo
  step (`edit.batch` of `channel.set_param`); the sampler-only parameters are
  left alone. The editor shows the matching preset or "(custom)".
- Linear ADSR envelope (`attack`, `decay`, `sustain`, `release`); a minimum 1 ms
  release prevents clicks. `pitch` transposes in semitones.
- `glide` (seconds, default 0) is portamento: a note that starts while another
  note on the same channel is still held slides linearly (in semitones) from that
  note's current pitch over the glide time.

## Editor (UI)

- Vertical keyboard on the left (drawn as piano keys), time grid snapped to
  steps by default. Notes of other channels in the pattern show faintly in
  one neutral tone (ghost notes, `theme::GHOST_NOTE`).
- **Snap values** (piano roll and playlist, finest first): None (single
  ticks), 1/6 step, 1/4 step, 1/3 step, 1/2 step, Step, 1/3 beat, 1/2 beat,
  Beat and Bar. The thirds and sixths are triplet grids. Beat fractions count
  from the start of each beat, so they follow the time signature.
- **Pattern play button** (header, after the pickers): a click plays the
  pattern from the playhead, switching to pattern mode first if the song was
  selected, or pauses while the pattern plays; a right-click stops and
  returns to the start while playing, or moves the playhead to the start
  while stopped. It shows a pause glyph and is lit while the pattern plays.
  The playlist header has the same button for the song (after its tools).
- **Tools** (header, in this order: Draw, Paint, Delete, Mute, Slice, Select,
  Zoom, Audition): Draw (the default behavior below), Paint (drag to
  lay notes end to end at the last length; one undo step), Delete (click or
  drag over notes; one undo step) and Select (drag a box without Ctrl; click
  empty space to clear). Starting a Paint drag on a note moves it as Draw would.
  Mute (click or drag toggles muting), Slice (click a note to cut it in two at
  the snapped click: `note.slice <channel> <start> <key> <at> [@pattern]`,
  undoable, both parts keep the note's properties), Audition (click hears the
  note or key under the pointer) and Zoom (click zooms in, Shift+click out). Keys:
  P, B, D, E, T, C, Y, Z pick the tools (the same choice applies to the
  playlist); S and O toggle Slide and Porta for new notes.
- Left-click on empty grid adds a note with the last used length; dragging a note
  moves it; dragging its right edge resizes it; right-click deletes.
- Right-click deletes a note; right-drag deletes every note the pointer passes
  over, as one undo step when the button is released (with any tool).
- Each finished gesture is one undoable command.
- Selection: click a note to select it, Shift+click toggles, Ctrl+drag on empty
  grid draws a selection rectangle (Shift adds to the selection). Dragging a
  selected note moves the whole selection; dragging its edge resizes all
  selected notes by the same amount. A selected note's whole body turns to one
  selection tone (`theme::NOTE_SELECTED`), whatever its colour.
- Keys while the piano roll is the focused window (the topmost one): Delete/Backspace remove the
  selection; Ctrl+A selects all; Escape clears; Ctrl+C / Ctrl+X copy / cut;
  Ctrl+V pastes at the pointer; Ctrl+B duplicates the selection right after its
  end (rounded up to the snap); Shift+Up/Down move by one key, Shift+Left/Right
  by one snap step, and Ctrl+Up/Down by an octave (plain arrows do not move
  notes). Every operation is one undo step (`edit.batch`),
  with moves ordered so notes of the selection never overwrite each other.
- Ctrl+scroll zooms horizontally. The scroll/zoom row above the ruler holds a
  left chevron, a rail with a note overview and a thumb, a right chevron and
  the vertical zoom box. The chevrons scroll by one bar (held: repeat); a
  click on the bare rail pages one screen; dragging the thumb scrolls, and
  dragging either end of it zooms horizontally with the other end fixed.
  Dragging the zoom box up or down changes the key-row height (6 to 30
  points); double-clicking it resets 18. Notes fill their row but for half
  a point at the top and bottom. A vertical scroll bar (chevrons,
  rail, thumb) runs down the right of the ruler and grid. The view runs on
  8 bars past the pattern's end, and covers at least 16 bars, so the
  horizontal bar can always scroll to write past the end; the thumb stays
  under the pointer while dragged, even at its shortest. Opening a channel
  centres the grid (between the ruler and the property lane) on its notes, or on C5 when it has none.
- Dragging with the middle button anywhere in the view pans it both ways,
  with the grabbing hand as the pointer; it edits nothing.
- The corner block above the keyboard shows the colour group of new notes
  (the channel colour for group 1) with its number. Clicking it opens a 4 × 4
  grid of the 16 groups. The Porta and Slide toggles sit on its right edge.
  Right-clicking it offers "Apply this colour to the selected notes" (Alt+C),
  which gives the selected notes that group as one undo step.
- Notes have a pan, -100 to 100 (`note.set_pan`; `note.add` takes it as an
  optional sixth argument; `note.set_all` rows may carry it as a fifth value).
  It is applied as a balance on top of the channel pan.
- The bottom lane edits velocity, pan, release, fine pitch, Mod A or Mod B,
  picked from the menu at its left. The target's name is written faintly at
  the lane's top left; the grid's bar, beat and step lines run through the
  lane a step quieter. Each stem has a round head and a thin tail running
  right for the note's length; selected notes' stems take the selection
  tone, and other channels' notes show faint ghost stems behind. In centred modes (pan, fine pitch…)
  stems grow up or down from the centre and snap to centre within ±3.
  Alt+click on a stem resets that note's value to the lane's default
  (velocity 100, the rest neutral), or every selected note's when that note
  is selected, as one undo step.
- Double-clicking a note (Draw or Select tool) opens the **Note properties**
  window, titled with its key ("Note properties — C5"): knobs for velocity,
  release, pan, fine pitch, Mod A and Mod B (in that order; each shows its
  value while hovered or dragged), Slide and Porta switches, the colour group
  swatch (a click opens the 4 × 4 group grid), and the note's start and
  length. When the note is one of several selected notes, every change
  applies to all of them (the readouts are then hidden and the title says how
  many). Each knob drag is one undo step. Reset puts every property back to
  its default (velocity 100, the rest neutral, slide and porta off, group 1)
  as one step; OK closes the window. It closes on its own when the note is
  gone.
- Alt+wheel over a note steps the lane's property of that note by a tenth
  of its range per notch (Ctrl+Alt+wheel: a fiftieth), or of every selected
  note when the note is selected; the view does not scroll meanwhile. A run
  of notches over the same notes, less than a second apart, is one undo step.
- The ruler marks the playhead with a triangle whenever it is inside the
  shown pattern (the loop position in pattern mode; in song mode, inside a
  clip of this pattern), and a line runs down the roll while playing.
- Clicking the ruler seeks the playhead to the snapped position (in song
  mode, the same place inside the first clip of this pattern). Dragging along the ruler makes a time selection: the ruler
  shows it as a highlighted band, the grid gets a faint wash with two edge
  lines, and the notes that overlap it are selected. Escape or a click on
  the ruler clears it.
- The header is one row: the window menu (☰), then three square buttons for the note tools, snap and scale, a chord
  button, the tools, the title and a chevron that opens the channel list
  (channels with notes in the pattern are ticked). Snap, scale and chord name
  their value in the hint and tick it in their list; a dimmed glyph means
  none is set. Right-clicking the snap button makes one snap step the
  length of new notes (unless snap is off), and the wheel over it steps
  through the snap list. In a narrow window the groups fold into the "more" button
  instead of wrapping.
- The window menu has submenus: **File** (export the song as a MIDI file),
  **Edit** (Cut, Copy, Paste, Duplicate, Delete, move the selection one snap
  step left or right, transpose it a semitone, transpose the channel an
  octave, mute, unmute or apply the current colour to the selected notes
  as one undo step, and clear the channel's notes in the danger tone), **Tools**
  (Quantize and the note tools), **Chord** and **Snap** (the same lists as
  the header buttons), **Select** (All, None, Invert, muted notes, notes of
  the current colour), **View** (ghost notes and note labels on or off, and
  Keyboard labels: C keys, white keys, all keys, keys of the scale or none;
  every key but C needs rows at least 9 points tall) and **Zoom** (in, out,
  fit the pattern to the window, zoom to the selection), then the new-note velocity field. The
  Edit and Select items do exactly what their keys do, and show the keys.
- The window opens over the right half of the workspace, about half its
  width and height.
- A velocity lane is pinned to the bottom of the view: one stem per note of the
  selected channel. Dragging vertically sets the velocity of the note under the
  pointer (the latest-starting one when notes overlap); one drag is one undo step.
- A thin splitter with a grip sits between the grid and the lane. Dragging it
  resizes the lane; double-clicking it, Shift+Enter or View › Property lane
  hides or shows the lane (hidden, the grid runs to the bottom); right-click
  restores the default height. Not saved with the project.
- Clicking a key on the keyboard previews the note on the selected channel.
  The hovered key is shaded (white keys darker, black keys lighter) and its
  name shows in the hint. Ctrl+click on a key selects every note of that key,
  and Ctrl+drag across keys selects every note of the keys crossed (Shift
  adds them to the selection). Middle-drag on the keyboard zooms the key
  rows (up grows them, down shrinks them, within the zoom box's range),
  keeping the key first pressed under the pointer. Middle-drag elsewhere in
  the view pans it. Right-clicking the keyboard offers the Keyboard labels
  choices. These view choices are not saved with the project.

## Sources

- The header's icon pickers, the corner block, the scroll/zoom row, the
  vertical zoom box, the tool order and the selection tone: the interface
  style guide, `docs/spec/style/piano-roll.md` (owner's 2026-10-21 decision
  to follow the manual's layout). Glyphs, sizes and wording are our own.
- Piano roll interaction (draw with left click, delete with right click, resize
  from the note edge) follows conventions described in the FL Studio online
  manual, "Piano roll" section, and common to most DAWs. Implementation and
  layout are our own.
- Slide notes (silent without an overlap, the overlap as the slide time,
  colour groups as independent slides, the top note setting a chord's offset)
  follow the FL Studio online manual, "Piano roll" section, note properties
  and slide notes. Wording and implementation are our own.
- Selection keys (Shift+arrows move, Ctrl+Up/Down by an octave) follow the
  FL Studio online manual, "Keyboard shortcuts", piano roll section.
- Synth architecture: textbook subtractive synthesis. PolyBLEP and the
  trapezoidal SVF follow publicly published papers (Välimäki et al.; Simper).
- Limit, LFO, scale levels, flam, randomizer, chord progressions, articulate
  and claw machine mirror the piano roll tools listed in the FL Studio online
  manual, "Piano roll" (tools menu). Every rule above (ranges, centres, the
  random grid, triad stacking) is our own design.
- Chord qualities: the idea of a broad chord menu follows the chord list in
  the FL Studio online manual, "Piano roll" section. The chord names, their
  intervals and the families are standard music theory; the grouping, naming
  scheme and voicing rules are our own.
