# Spec: mixer and effects (M3)

## Behavior

- Mixer track 0 is the **master**; tracks 1… are **inserts**. New projects have
  sixteen inserts. At most 128 tracks.
- Each track has a name, volume (linear 0–1.25, default 1.0), pan (-1–1, a
  balance), mute,
  solo and up to ten insert effects processed in order. The mixer is a floating
  window (F9) with the strips on the left and the selected track's effect slots
  on the right.
- Channels feed exactly one track. Each insert feeds the master unless
  `mixer.route_to_master <track> off`, and may also have up to eight post-fader
  **sends** to higher-numbered inserts (`mixer.set_send <track> <target>
  <level>`; level 0 removes it). Sends only go "up", and a send that would close
  a loop through a sidechain route is refused, so routing never feeds back.
  Tracks are processed in number order unless a sidechain route asks for a
  higher-numbered insert first (see *Sidechain*). This builds buses (for
  example drums 1–3 into insert 10) and shared effect returns.
- Removing a track removes sends into it and renumbers sends past it; undo
  restores them.
- Solo: when any insert is soloed, every insert that is not soloed, not feeding
  a soloed insert, and not fed by one is silenced. The master is never silenced
  by solo.
- Commands: `mixer.add_track`, `mixer.remove_track` (channels routed to it move to
  the master; undo restores them), `mixer.rename`, `mixer.set_volume`,
  `mixer.set_pan`, `mixer.set_mute`, `mixer.toggle_mute`, `mixer.set_solo`,
  `mixer.toggle_solo`, `mixer.set_send`, `mixer.route_to_master`, `mixer.sidechain_to`, `mixer.add_effect`, `mixer.remove_effect`,
  `mixer.move_effect`, `mixer.set_effect_param`, `mixer.set_effect_enabled`,
  `mixer.toggle_effect`. Effect slots are numbered from 1. All are undoable.
- **Track colour**: `mixer.set_track_color <track> <1-16|none>` (undoable,
  saved as `"color"`, 0-based). Without one, the master uses the highlight
  tone and each insert a palette colour by position. UI: the track menu
  (right-click the number or name) and the window menu's Tracks → Colour ▸
  (the shared colour grid). The colour tints the number cell. While a
  track menu is open, its strip lifts one tone (`RAISED_HOVER`) so it is
  clear which track the menu acts on.
- **Docks**: the strips sit in three docks, left to right: a fixed left dock
  (the master, then inserts pinned left), the scrolling middle (every other
  insert), and a fixed right dock (inserts pinned right; absent until one
  is). Each dock lists its tracks in number order, and darker dividers about
  a quarter strip wide separate the docks. `mixer.set_dock <track>
  <left|middle|right>` (undoable, saved as `"dock"`, inserts only); UI: the
  track menu's Pin ▸ Left side / Scrolling area / Right side. Track numbers
  never change with the dock.
- **Moving a track**: `mixer.move_track <track> <left|right>` (undoable, inserts
  only) swaps an insert with its neighbour, so the two trade numbers. Every
  reference follows the moved tracks: channel routing, sends, sidechain and
  send-effect sources, peak controllers and automation targets. Moving insert 1
  left or the last insert right is refused, and so is swapping two inserts
  joined by a send (sends always go to a higher number). UI: the track menu's Shift left and
  Shift right, Alt+Left / Alt+Right over the mixer (the primary track), and
  Shift+wheel over a strip's number cell or name (down or towards you moves
  it right); the selection follows the moved track.
- **Track stereo options** (undoable, saved; right-click a strip's name):
  `mixer.set_separation <track> <-1..1>` (-1 folds the track to mono, 0
  leaves it, 1 doubles the side signal), `mixer.set_phase_invert <track>
  <on|off>` (flips the polarity of both sides) and `mixer.set_swap_sides
  <track> <on|off>` (exchanges left and right). They apply to the track's
  input, before its effects and fader.
- **Pan law** (project setting, `project.set_pan_law <circular|triangular>`,
  undoable, saved as `"pan_law"`; Settings → Project). It applies to every
  channel pan. Mixer track pan stays a balance (0 dB at centre; a track
  carries stereo, and a law at every stage would stack its centre cut), and
  note pan stays a balance on top of the channel pan. For pan `p` in -1…1:
  - `circular` (default): left = cos θ, right = sin θ with θ = (p + 1)·π/4.
    Centre is -3 dB on each side (equal power), a hard pan is 0 dB on its side
    and silent on the other.
  - `triangular`: left = (1 − p)/2, right = (1 + p)/2. Centre is -6 dB on each
    side (equal amplitude), a hard pan is 0 dB on its side.
  - Project files without `"pan_law"` (saved before it existed, when pan was a
    balance with 0 dB at centre) load as `circular`, so centred sounds play
    3 dB quieter than they did.
- Changing parameters never interrupts audio. Adding, removing or reordering
  effects rebuilds the track's processing and resets effect tails.

## Effects

| Kind | Parameters |
| --- | --- |
| `filter` | mode (0 low-pass, 1 band-pass, 2 high-pass), cutoff Hz, resonance, mix |
| `eq` | low gain/freq (shelf), mid gain/freq/q (peak), high gain/freq (shelf) |
| `delay` | time in beats (tempo-synced), feedback, mix, ping_pong, damping, offset ms (−20–20: the right side's echoes come this much later, or the left's for negative values), mod_rate Hz and mod_depth ms (a sine LFO, a quarter cycle apart per side, moves the read point for a tape-like wobble; depth 0 is off), and a feedback path where each repeat passes a low cut Hz (one-pole high-pass, off at 20 Hz), diffusion (two all-pass diffusers that smear repeats toward a reverb-like tail, 0 off) and saturation (tanh, 0 off; it never raises the level) |
| `reverb` | size, damping, width, mix |
| `hall_reverb` | predelay ms (0–200), decay s (time for the tail to fall 60 dB), size (scales every delay length, 0.5×–1.5×), damping Hz (one-pole low-pass inside the loop; lower is darker and shortens highs), low cut Hz (high-pass on the wet signal), early (level of six early reflections after the predelay, alternating sides), mod_depth and mod_rate Hz (slow sine modulation of the line lengths, which smooths metallic ringing), width (0 mono wet – 1 full stereo), mix. An eight-line feedback delay network with Householder mixing; each line's loop gain is set so the tail falls 60 dB in the decay time |
| `chorus` | rate Hz, depth ms, delay ms, mix, voices (1–3 delayed copies per side, their LFOs evenly spaced through the cycle and averaged), tone Hz (one-pole low-pass on the wet signal for a darker, vintage sound; off at its 20 kHz maximum) |
| `compressor` | threshold dB, ratio, attack ms, release ms, makeup dB, sidechain (0 = off, else the insert whose post-fader output drives detection) |
| `limiter` | input gain dB, ceiling dB, release ms (instant attack; output never exceeds the ceiling) |
| `distortion` | drive dB, mix, output dB, mode (0 soft: tanh; 1 hard: the driven signal is clipped flat at the threshold, then scaled back to full level), threshold dB (hard mode only, −24–0) |
| `phaser` | rate Hz, depth, feedback, stages (2–12 all-pass stages), mix, warm (0–1: tanh saturation in the feedback loop, which rounds off the resonant peaks; 0 is off) |
| `flanger` | rate Hz, depth ms, delay ms, feedback (−0.95–0.95), mix |
| `bitcrusher` | bits (1–16), downsample (hold every 1–32 samples), mix, output dB, squeeze (0–1: before quantizing, each held sample's level is raised to the power 1 − 0.9 × squeeze, which pushes quiet detail up toward full level for a gritty, squashed sound; 0 is off) |
| `stereo` | width (0 mono – 2 extra wide, mid/side), balance, invert left, invert right |
| `stereo_shaper` | left_to_left, right_to_left, left_to_right, right_to_right (−1–1; each output side is a mix of both inputs, so swapping sides, mono from one side and polarity flips are all settings), delay ms (−20–20: positive delays the right side, negative the left, for Haas widening), output dB |
| `send` | target (0 = off, else a higher-numbered insert), level (0–1.25), pan (balance of the sent copy), dry (0–1, share of the audio that continues down this track's chain). Adds the audio as it is at this point of the chain to the target insert's input, so effects after the send are not heard on the copy. A send on the master, or one naming the same or a lower insert, is refused. Inserting or removing tracks renumbers targets; removing a target track is refused |
| `band_split` | low/mid and mid/high crossover Hz (the `multiband` split; the bands sum flat), low_target, mid_target, high_target (0 = the band stays on this track, else a higher-numbered insert that receives it instead). Lets each band get its own effects and fader. Targets follow the `send` rules |
| `gate` | rate (1/32 note – 1 bar per step), duty (open share of a step), depth, smooth ms, steps (16-step on/off pattern as a number, bit 0 = first step) |
| `autopan` | rate (1/32 note – 1 bar per cycle), depth, shape (sine, triangle, square) |
| `graphic_eq` | seven peak bands at 63, 160, 400, 1k, 2.5k, 6.3k, 16k Hz (±18 dB each), output dB |
| `parametric_eq` | seven bands of freq/gain/q (band 1 a low shelf, 7 a high shelf, 2–6 peaks), output dB; the window's points drag frequency and gain |
| `bass_boost` | freq Hz, boost dB (low shelf), drive (gentle saturation after the shelf) |
| `overdrive` | drive dB, tone Hz (low-pass after the clipper), bias (asymmetry, adds even harmonics), mix, output dB |
| `soft_clipper` | threshold dB, knee (how early the curve bends), output dB; never exceeds the threshold |
| `waveshaper` | shape (tanh, hard clip, fold, sine, asymmetric, rectify, drawn), drive dB, mix, output dB, point_1 … point_9 (the drawn curve: output levels −1–1 at inputs −1, −0.75, … 1, joined by straight lines and held flat beyond ±1; the defaults draw a straight line, so the drawn shape starts clean). With the drawn shape the window shows the nine points; drag one up or down to bend the curve |
| `guitar_chain` | a fixed pedal chain in one insert: compress (0–1; threshold −6 to −40 dB, ratio 1–8 and make-up 0–12 dB together; 0 is off), drive dB (0 is off) and drive_type (soft tanh, hard clip, fuzz: asymmetric and sputtering near silence), bass / mid / treble dB (shelf at 120 Hz, peak at 700 Hz, shelf at 3.2 kHz), chorus (0–1 depth of a fixed chorus, 0 off), cabinet (off, small: 110 Hz – 5 kHz with a 1.8 kHz bump, large: 70 Hz – 3.8 kHz with a 120 Hz bump), output dB, mix. The chorus follows the wet/dry mix. The window shows the chain with the stages that are on lit |
| `dc_block` | freq Hz (one-pole high-pass that removes DC offset) |
| `ring_mod` | freq Hz (sine carrier the signal is multiplied by), mix |
| `freq_shift` | shift Hz (−2000–2000; every frequency moves by the same amount, via a Hilbert transformer), mix |
| `formant` | vowel (0 a – 4 u, fractional values morph), shift semitones, resonance, mix |
| `auto_filter` | mode, cutoff Hz, resonance, rate (tempo-synced LFO), depth octaves, envelope octaves (input level opens or closes the filter), mix, lfo_shape (sine, triangle, saw up, saw down, square, drawn), step_1 … step_8 (the drawn shape: eight levels −1–1 evenly spaced through one cycle, joined by straight lines that wrap from the last back to the first), volume (0–1: the same shape also dips the level, by this much at its lowest point), pan (−1–1: the shape also moves the sound, to the right at its highest point for positive values). The window draws the shape under the response; with the drawn shape drag its points up or down |
| `dynamics` | an all-in-one chain of noise gate → compressor → limiter: gate_threshold dB (at its minimum, −100, the gate is off), gate_attack / gate_hold / gate_release ms, gate_range dB; comp_threshold dB, comp_ratio (1 = off), comp_attack / comp_release ms, comp_makeup dB; limit_gain dB (before the limiter), limit_ceiling dB, limit_release ms. Each stage behaves exactly like the separate `noise_gate`, `compressor` and `limiter`; by default only the limiter acts. Its window draws the combined static curve; drag the knee for the compressor threshold and the right-hand point for the ceiling |
| `noise_gate` | threshold dB, attack ms, hold ms, release ms, range dB (attenuation while closed) |
| `transient` | attack (−1 soften – 1 boost), sustain (−1–1), output dB; from the difference of a fast and a slow envelope |
| `multi_tap` | four taps of time (beats) and level, feedback, mix, damping; per tap a pan (−1 left – 1 right, constant power; defaults alternate hard left/right), a low cut Hz (one-pole high-pass, off at its 20 Hz minimum) and a high cut Hz (one-pole low-pass, off at its 20 kHz maximum). Older projects keep their sound because the new parameters default to the old behaviour |
| `multiband` | low/mid and mid/high crossover Hz (Linkwitz–Riley, bands sum flat), threshold dB / ratio / gain dB per band, attack ms, release ms, output dB |
| `multiband_limiter` | low/mid and mid/high crossover Hz (the same split as `multiband`), per band drive dB (gain into the band's limiter), ceiling dB (the band's peak never exceeds it) and level dB (after the limiter), release ms (all limiters), amount (0–1, scales every band's drive, so one knob goes from untouched to fully driven), ceiling dB (a master peak limiter after the bands are summed; the output never exceeds it). With default settings audio below the master ceiling passes unchanged. Its "Enhance" presets give a one-knob loudness enhancer through `amount` |
| `vocoder` | sidechain (the modulator, e.g. a voice; this insert's audio is the carrier), bandwidth, attack ms, release ms, shift semitones (moves the carrier bands), noise (hiss mixed into the carrier for consonants), mix, output dB, bands (4–64, default 16), low and high Hz (the band centres are log-spaced between them, default 120 Hz–7.5 kHz). More bands are made narrower and their make-up gain lower, so the level stays similar. Without a sidechain only the dry share passes |
| `pitch_shift` | pitch semitones (−24–24), fine cents, window ms (longer is smoother, shorter tracks transients better), mix |
| `pitch_correct` | key (C–B), scale (chromatic, major, minor, harmonic minor, major pentatonic, minor pentatonic), speed ms (how quickly the pitch glides to the target; 0 snaps at once for a hard-tuned sound), amount (share of the distance to the target note that is corrected), reference Hz (the frequency of A4, 415–466), mix, follow (0 off, else a channel's rack number: while that channel holds a note, its highest held note is the target instead of the scale; with nothing held the scale applies. Moving channels keeps following the same channel, and removing a followed channel is refused), harmony_1 and harmony_2 (−12–12 semitones, 0 off: extra voices that follow the corrected pitch at that interval), harmony_level. For one voice or instrument at a time; see *Pitch correction and tuner* |
| `tuner` | reference Hz (the frequency of A4). Leaves audio untouched; its window shows the note heard |
| `multi_fx` | block (off, low-pass, high-pass, drive, crush, ring mod, phaser, flanger, chorus, reverb, width, tremolo, freq shift, fold), x and y (0–1, the block's two controls: cutoff/resonance, drive/mix, bits/rate, pitch/mix, speed/feedback, speed/depth, size/mix, width/balance, shift/mix), mix, output dB. The window is an X/Y pad: drag the point to play both controls at once. Picking another block clears the old one's tail |
| `fx_chain` | four stages of block, x and y (the same blocks as `multi_fx`) processed in order, then mix and output dB. The window shows the four stages; empty stages are off |
| `beat_shaper` | cycle (1 beat, 2 beats, 1 bar, 2 bars; follows the song position like the gate), time_mode (steps: each point holds for its eighth of the cycle; ramps: lines join the points, wrapping to the first), time_1 … time_8 (how far behind the input playback runs, as a share of the cycle: 0 is live, a held value repeats earlier audio, a ramp rising as fast as time freezes it, steeper ramps play backwards), volume_1 … volume_8 (levels joined by lines), mix. Jumps in time crossfade over 3 ms; history reaches back at most 8 seconds. The window shows both envelopes; drag their points |
| `scratch` | hold (off: audio passes and is recorded; on: the last span is frozen like a record under a hand), position (0–1 through the held span: automate or drag it to scratch by hand), span beats (1/16 – 8 beats, at most 8 seconds), pattern (manual, baby: forward and back, chirp: the return cut, tear: the forward stroke in three jerks, transform: a baby scratch chopped eight times), rate (pattern cycle, 1/32 note – 1 bar, synced like the gate), depth (how far a pattern moves the record), inertia ms (how quickly the platter follows; longer is heavier), mix. Presets pick a pattern but never switch hold on |
| `stutter` | slice (1/32 note – 1 bar), every (period, 1/32 note – 1 bar), portion (share at the end of each period that repeats), decay (level of each repeat relative to the last), mix |
| `convolution` | mix, predelay ms, low cut Hz, high cut Hz, output dB; the impulse response is chosen with `mixer.set_effect_file` |

The `send` effect's `target` and the band splitter's targets follow the same
renumbering rules as sidechain sources (below).

Effects with a `sidechain` parameter (the compressor and the vocoder) read the
post-fader output of the track it names, which must be routed to their track as
a sidechain (see *Sidechain*). Removing that insert is refused while it is
used; removing another renumbers the source.

`mixer.set_effect_file <track> <slot> <file|ir:name|->` sets the convolution
reverb's impulse response: a WAV/FLAC file (any length, cut at six seconds) or
a generated room (`ir:room`, `ir:chamber`, `ir:plate`, `ir:hall`,
`ir:cathedral`); `-` restores the default hall. The response is normalized to
unit energy, so swapping rooms keeps the level. The convolution adds 512
samples of latency to the wet signal. Undoable.

The **spectrum analyser** (View menu, or click the toolbar oscilloscope) shows
the master output's last 2048 samples by frequency (Hann window, log frequency
axis, 0 to −90 dB) with a slowly falling peak line.

The gate, auto-pan, auto filter and stutter follow the song position while playing (so they stay in
time with the beat) and a free-running clock at the project tempo while
stopped. The gate's window shows the 16 steps; clicking one turns it on or off.

Ranges and defaults are defined in `musiccraft-params`.

## Track EQ and slot mix

- Every track has a built-in **three-band EQ** after its stereo options and
  before its effect slots: a low shelf, a peak and a high shelf, each with a
  gain (-18…18 dB, 0 = off) and a frequency (low 20–1000 Hz, default 120;
  mid 100–10000 Hz, default 1000, Q 0.7; high 1000–20000 Hz, default 8000).
  `mixer.set_eq <track> <low|mid|high> <gain dB> [freq Hz]` (undoable, saved
  as `"eq"` only when a band is changed). A flat EQ costs nothing.
- Every effect slot has a **mix** (0–1, default 1): the slot's output is
  `dry + (wet − dry) × mix`. `mixer.set_effect_mix <track> <slot> <0-1>`
  (undoable, saved as `"mix"` only when below 1). Send and band-split slots
  ignore it.

## Layout

- The mixer is an editor window. The first time it opens it sits at the top
  of the workspace, to the right of the channel rack, reaching the right
  edge: about two thirds of the width and a little under half the height.
  Strips fill the height of the window; the fader takes the spare height.
- Header: window menu, the strip-details switch, the slot-panel switch and
  the add-insert button (a plus: adds an insert after the last and selects
  it; right-click offers Add one and Add several ▸ 2, 4 or 8 inserts, one
  undo step, the first new one selected), then the
  title (`Mixer — <track>`) at the right end before the window buttons. The
  window menu holds View (Strip details, Slot panel, Slot panel on the left,
  Hairlines between strips, and Strip colour ▸ Off, Faint, Medium or Strong:
  how much of each insert's colour tints its strip, Faint by default; the
  master is never tinted; none of these is saved with the project) and
  Tracks (Rename…
  and Colour ▸ for the primary track; Reset selected tracks, which returns
  each to its defaults with `mixer.reset_track <track>`; Delete selected
  tracks, in the danger tone; Clear out idle inserts, which deletes every
  insert with no channel routed to it, no send into it and every setting but
  its name at the default), then the common items. Each item is one undo
  step. `mixer.reset_track` is refused while an automation drives one of the
  track's effects; its undo is `mixer.set_track <track> <json>`.
- The selected strip is lifted to the raised tone with a thin accent edge
  around the whole strip, and its meter well is outlined in the accent.
- Several tracks can be selected. Ctrl+Shift+click on a number cell, name or
  meter adds the track to the selection or takes it out; Ctrl+A over the
  mixer selects every track and Esc goes back to one. A plain click on a
  number, name or meter selects that track alone. The last track clicked is
  the primary one: the slot panel, the title and the routing arrows follow
  it, and only it gets the accent edge; the others are lifted and have their
  meter outlined. Clicking a fader, knob or light of a track outside the
  selection selects that track alone; one inside keeps the selection.
- Faders, pan and separation knobs, enable lights and the track switches
  are ganged across the selection. Dragging a selected fader moves every
  selected fader by the same share of its travel, and a pan or separation
  knob moves every selected one by the same amount, each clamped to its
  range; the whole drag is one undo step. Clicking a selected track's enable
  light mutes or unmutes every selected track to match it
  (`mixer.set_mute`); its phase or swap switch sets every selected track to
  match it; its effect count bypasses or enables the effects of every
  selected track that has any. Each click is one undo step.
- Each strip is narrow and shows, top to bottom: the number cell (the number,
  or M for the master, on a cell tinted with the track's colour), the name
  written vertically beside the peak meter, a round enable light (lit while
  the track is heard; click mutes, Ctrl+click solos, right-click offers Mute
  and Solo; a soloed track's light gains a ring in the highlight tone, and a
  track silenced by another track's solo shows its light in the dim accent),
  the S (solo) switch as a tiny round light
  under it (not on the master), the pan knob, the fader, then the track
  switches: invert phase, swap left and right, the stereo separation knob and
  the effect count, lit while any effect runs; clicking it bypasses all the
  track's effects, or enables them all again
  (`mixer.set_effects_enabled <track> <on|off>`, one undo step). Its
  right-click menu offers "Open all effect windows", which opens the window
  of every effect on the track (windows already open stay open). A click
  anywhere on a strip selects it, without taking the click from the light or
  switch under the pointer. The wheel over the number cell or the name steps
  the strips sideways by whole strips. The strip-details switch hides the track switches. The
  level in dB is in the fader's hint. The route arrow is at the bottom.
- **Routing**: the arrow under every strip the selected insert may feed shows
  whether it does (filled in the highlight tone) or not (an outline) and
  toggles it. Strips it may not feed show a faint, inert outline. The
  selected strip itself shows an upward source arrow whose hint says where
  it goes ("This insert goes to M", "… to 3" or "… to M, 3, 5"). With the
  master selected no arrows apply, as the master feeds nothing. The slot
  panel's Out row and each send row draw the output glyph (a small circle
  with an arrow leaving it) before the destination, never a text arrow.
- The slot panel beside the strips shows, for the selected track (it has no
  heading; the title names the track): its **input** (the audio input device,
  as in Settings → Audio, for recording), the ten effect slots, the track EQ
  (three gain knobs; right-click one to set its frequency), its **output**
  (inserts: where they route; the master: the audio output device) and, for
  inserts, To master and the sends.
- A slot row, left to right: a chevron that opens the slot menu, the name
  (empty slots show a dim "Slot 3"), the mix knob and a round light at the
  right edge (click: bypass or restore). Clicking a loaded name opens or
  closes its window; clicking an empty slot's name or chevron opens the add
  list (built-in effects, then plugin effects), and the effect goes into the
  first free slot. A loaded slot's menu has
  Open window, Bypass (or Enable), Move up, Move down and Remove. Dragging a
  loaded slot's name onto another loaded slot moves the effect there
  (`mixer.move_effect`, one undo step); an accent line shows where it will
  land.
- The input and output rows start with small glyphs (a circle with an arrow
  entering it; one leaving it) instead of the words "In" and "Out".
- The mouse wheel over a strip's number, name or meter moves the strips one
  whole strip per notch, and so does the wheel over the strips' empty space (between the
  controls and after the last strip). Over a knob or fader the wheel adjusts
  that control instead.

## Effect rack

- The `rack` effect holds up to four parallel **branches** inside one slot,
  each a chain of up to six built-in effects with their own settings. The
  slot's input feeds every branch; each branch's output is scaled by
  `branchN_level` (0–1.25) and placed by `branchN_pan` (balance, −1–1), the
  branches are summed with `dry` (0–1) of the input, and `output` dB scales
  the result. An empty branch is silent, so a new rack (with `dry` at its
  default of 1) passes its input unchanged until effects are added.
- Branch effects may be any built-in effect except the ones the engine
  routes (`send`, `band_split`), plugins and racks themselves. Inside a
  rack, effects have no sidechain and follow no channel (those parameters
  are refused and hidden).
- Commands (undoable): `mixer.rack_add <track> <slot> <branch> <kind>`
  appends an effect to a branch; `mixer.rack_remove <track> <slot> <branch>
  <n>` and `mixer.rack_insert … <n> <json>` (undo) remove and restore one;
  `mixer.rack_param <track> <slot> <branch> <n> <param> <value>` sets a
  parameter; `mixer.rack_enable <track> <slot> <branch> <n> <on|off>`
  bypasses one. Changing a branch's effects rebuilds the track (tails
  reset); changing their parameters does not.
- The rack's window shows the four branches as rows of effect boxes; each
  branch has an "Add" menu, and each effect expands into its knobs and a
  bypass and remove button. Branch effect parameters are not automatable.

## Pitch correction and tuner

- Both effects detect the pitch of a single voice or instrument (monophonic,
  60–1000 Hz) from the mid signal, with a new estimate about every 5 ms. Silence
  (below −60 dBFS) and noise have no pitch.
- The **pitch corrector** finds the note of the chosen scale nearest to the
  detected pitch and shifts the audio toward it: the correction (in semitones)
  is `(target − detected) × amount`, approached with a one-pole glide whose time
  constant is `speed`. Between notes, where no pitch is detected, the last
  correction is held so consonants do not wobble. The shifted copy fades in over
  the first 10 cents of correction, so notes already in tune pass through
  untouched; with amount 0 the audio is unchanged.
- The **tuner** passes audio through unchanged.
- The engine publishes each detecting slot's latest pitch (bypassed slots
  publish none) for the UI. Their windows show the note name (A4 = MIDI note
  69 at the reference frequency), the frequency, a ±50-cent meter that turns
  the accent colour within 5 cents, and, for the corrector, the target note
  and a twelve-key strip with the scale's notes lit.

## Effect windows and presets

- Clicking an effect's name in a slot opens its window (again closes it).
  Windows of removed slots close.
- Every effect window's header holds the window menu (the slot's own items:
  Bypass or Enable, Move up, Move down, Remove), and for built-in effects the
  presets: previous, list, next; "(custom)" when no preset matches, and
  right-click on an arrow opens the list. Under the
  header a slim strip shows, flush right, the On light (bypass,
  `mixer.toggle_effect`) and the Mix knob (`mixer.set_effect_mix`, the same
  value as the slot's knob). (The plugin editors keep a bypass dot and the
  presets in their first row, as they have no header.)
- Each built-in effect window then has a graph:
  - filter and EQ: the frequency response; drag the filter's point for cutoff
    (across) and resonance (up), drag an EQ band's point for its frequency and
    gain, scroll on the middle band for its width;
  - compressor and limiter: the static input/output curve; drag the knee for
    the threshold, or the ceiling point;
  - distortion: the waveshaping curve; delay: the echoes over time; reverb:
    the decay; chorus: the delay modulation.
  Drags and presets are each one undo step (`edit.batch` of
  `mixer.set_effect_param`). Presets never change the sidechain source.
- Hosted plugin slots open a window with the plugin's parameter panel and a
  button for its own window.
- Changes to channel and track volume, pan and mute ramp over one engine
  block (at most 256 frames), so moving a fader never clicks.

## Sidechain

Sidechaining follows FL Studio's model: a track is first *routed* to another
as a sidechain, then an effect on the receiving track picks it as its input.

- **Sidechain routes.** `mixer.sidechain_to <source> <target> on|off`
  (undoable) routes an insert's post-fader output to another track as a key
  signal only: nothing is added to what the target plays. The source is an
  insert; the target is any other track, the master included. An insert has at
  most 8 sidechain routes. In the mixer, select the source insert, right-click
  the routing arrow under the target and choose **Sidechain to this track**
  (choose it again to remove the route). A sidechain route draws that arrow as a
  dashed outline in the highlight tone, and the strip's output summary lists it
  as `SC n`.
- **Picking the input.** The compressor's and vocoder's `sidechain` parameter,
  and the `sidechain` setting of a hosted plugin effect (fed to the plugin's
  first auxiliary input; plugins without one ignore it), name the track whose
  key they use, or 0 for none. The track must have a sidechain route to the
  effect's track; naming one without a route is refused with a hint to add it.
  In the effect window the parameter is a drop-down: **Off**, then each track
  routed to this one as a sidechain (number and name). Removing the route
  silences the key but keeps the setting, as in FL Studio.
- **Any direction.** A source may be higher- or lower-numbered than its target.
  The mixer processes inserts in dependency order: an insert always runs after
  every insert that sends to it (sends, send effects, band splits) or keys it
  (sidechain routes), and otherwise in number order; the master runs last. So a
  sidechain adds no latency. A route, send or send-effect target that would
  close a loop (the target already feeds the source) is refused.
- Removing a track that an effect uses as its sidechain is refused. Removing
  or inserting other tracks renumbers routes and sources; routes into a removed
  track come back with it on undo. Moving a track keeps its routes.
- Projects saved before sidechain routes existed load with a route added for
  every effect sidechain that names a track, so they sound the same.

## Metering

- The engine publishes a peak level per track; the UI shows it decaying.

## Sources

- The mixer's placement, header, strip order (number cell, name beside the
  meter, enable light, pan, fader, track switches) and the slot rows
  (chevron, name, mix knob, light): the interface style guide,
  `docs/spec/style/mixer.md` (owner's 2026-10-21 decision to follow the
  manual's layout). Glyphs and wording are our own.

- The track EQ, slot mix knobs and the input/output pickers beside the slots
  follow the mixer layout described in the FL Studio online manual, "Mixer"
  (per-insert parametric EQ, effect slot mix levels, audio input/output
  selection), in our own drawing; band ranges and defaults are our own.

- Sidechain routes ("Sidechain to this track" on the routing arrow's
  right-click menu, a route that carries no audio, then picking that track as
  an effect's sidechain input): the FL Studio online manual, "Mixer" (routing)
  and "Fruity Limiter" (sidechain), as described by the owner on 2026-10-23.
  The processing order, loop checks, drawing and wording are our own.

- Mixer layout (master plus numbered inserts, channel routing, per-insert effect
  slots) follows the workflow described in the FL Studio online manual, "Mixer"
  section. Effects are our own implementations of textbook designs: RBJ "Audio EQ
  Cookbook" biquads, Freeverb-style reverb (public domain algorithm by Jezar),
  feed-forward compression, peak limiting, a modulated-delay chorus, tanh soft
  clipping. The frequency shifter uses Olli Niemitalo's public-domain
  all-pass Hilbert transformer coefficients; vowel formant frequencies are
  textbook averages (Peterson & Barney style tables). The range of effect types
  (graphic and parametric EQ, gate, transient processing, multi-tap delay,
  waveshaping, frequency shifting, an all-in-one dynamics unit, a stereo
  mixing matrix) mirrors the categories in the FL Studio
  manual's effects list; the designs are ours. Convolution is uniformly
  partitioned overlap-save (textbook); the vocoder is a classic channel
  vocoder (band-pass filter banks and envelope followers); the pitch shifter is
  a two-tap crossfaded delay-line design; pitch detection is the published
  YIN method (de Cheveigné and Kawahara, JASA 2002), and the corrector drives
  the same delay-line shifter; the generated impulse responses are
  filtered decaying noise with early reflections, our own.
- Pan law: the FL Studio online manual, "Project general settings", names a
  circular (default, -3 dB at centre) and a triangular panning law. The gain
  formulas are the textbook sine/cosine and linear laws; the triangular
  formula and the handling of older files are our own choices.
- Effect windows (preset bar, graph, knobs) follow the common layout of plugin
  windows in pattern-based DAWs; the graphs and factory presets are our own.
- Track stereo separation, phase invert and swap left/right are mixer track
  properties described in the FL Studio online manual, "Mixer" (track
  properties). The mid/side formula is textbook; the ranges are our own.
- The effect rack takes the idea of building parallel effect graphs inside
  one plugin slot from the FL Studio online manual's description of
  Patcher; the fixed four-branch layout, the parameters and the commands are
  our own design.
