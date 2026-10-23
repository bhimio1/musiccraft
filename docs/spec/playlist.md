# Spec: playlist and song mode (M5)

## Behavior

- The playlist has 64 track rows, numbered from 1. A **clip** places a pattern on
  a row at a start time with a length (both in ticks). If a clip is longer than its
  pattern, the pattern repeats; notes are cut at the clip's end.
- Clips on one row may not overlap.
- Rows can be muted; muted rows do not play.
- **Track rows** have a name, a colour, mute and solo (all saved, all undoable):
  `playlist.rename_track <track> [name]` (no name restores "Track n"),
  `playlist.set_track_color <track> <1-16|none>`, `playlist.set_track_solo
  <track> <on|off>` and `playlist.toggle_track_solo <track>`. While any row is
  soloed, only soloed rows play; a muted row stays silent even when soloed.
  Silenced rows skip pattern, audio and automation clips alike. The row header
  is a raised plate, tinted well down by the row's colour and one step
  lighter under the pointer, with a hairline gap under it. A plate at least
  two text lines tall has the name on its top line and, on its bottom line, a
  two-line resize grip at the left and the arm dot, S (solo) button and mute
  light at the right; a shorter plate puts the name and switches on one line
  and draws no grip. The arm dot shows only while the row is armed or the
  pointer is over the plate. The mute light gains a ring in the highlight tone
  while the row is soloed, and is lit at reduced strength while another row's
  solo silences it. Right-click the plate to rename it or pick a colour, or
  double-click it to rename it in place. Track and pattern menus share one
  Colour submenu: two rows of eight swatches with the current one outlined,
  a row of up to eight smaller swatches for the colours picked most recently
  (newest first, for this session), then "No colour" and a dice
  button that picks a colour that is neither the current one nor a recent
  one. Silenced rows show their name dimmed.
- **Track selection.** One track is selected (view state, not saved): its
  plate is one step lighter and the narrow selector strip between the plate
  and the lane is lit in the accent. Clicking or right-clicking a header
  selects its track; Ctrl+Up and Ctrl+Down move the selection.
- **Track menu** (right-click a header): Rename…, Colour ▸, Height ▸, then
  "Name after first clip" (the first pattern clip's name and colour), "Reset
  name, colour and height", "Mute all clips" / "Unmute all clips" (clip
  mutes, separate from the track light), "Select all clips", "Join pattern
  clips into one" (the Clip menu's Merge for the whole track) and "Render to
  audio" (Consolidate for the track's pattern and audio clips). Each is one
  undo step; items with nothing to do are greyed out.
- **Track heights.** Each row can have its own height, a percentage of the
  global track height (25–400%, default 100%; saved, undoable):
  `playlist.set_track_height <track> <percent|reset>` and
  `playlist.reset_track_heights` (window menu → Tracks → "Reset all track
  heights"). The row's menu has Height ▸ 50%, 75%, 100%, 150%, 200%, 300%.
  Dragging a header's bottom edge resizes that row alone in steps of 5%, shown
  at once and saved as one step on release; the hint shows the percentage.
  Alt+click on the edge resets the row. The vertical zoom still scales every
  row together, keeping their proportions.
- Playlist time arguments are in bars from 0 of the song's time signatures
  (`time-signatures.md`); fractions are allowed and `<n>t` gives exact ticks.
- Commands: `playlist.add_clip <pattern> <track> <start> [length]` (default length:
  the pattern's length), `playlist.remove_clip <track> <at>` and
  `playlist.move_clip`/`playlist.resize_clip` (the clip covering a position),
  `playlist.trim_clip <track> <at> <new start>` (moves the clip's start and
  keeps its end, so the clip shows a later or earlier part of its source;
  refused before the source's start, at or past the clip's end, or over
  another clip),
  `playlist.set_track_mute`, `playlist.toggle_track_mute`, `playlist.clear`,
  `playlist.set_clips` (used by undo). All are undoable.
- In song mode the song length is the end of the last clip rounded up to whole
  bars. Removing a pattern removes its clips (undo restores them).

- **Picker** (UI): the list beside the playlist has three kind switches at
  the top (pattern, audio and automation glyphs). A lit switch shows only that
  kind; clicking it again shows all kinds, with no switch lit. The list has no
  group headings. Clicking an entry chooses what a click on the
  playlist places: the selected pattern, an audio clip of that channel (as
  long as its other audio clips), or that automation. Clips show their name in
  a title strip with the preview (notes, waveform or curve) below it. A
  pattern clip longer than its pattern marks where each repeat starts with a
  short tick at the top and bottom of its body; an automation clip tall
  enough to show them marks its points as small dots on the curve. A "+"
  button centred under the patterns adds a new empty pattern
  (`pattern.add`) and paints with it.

## Loop region and markers

- `transport.set_loop <start bar> <end bar>` loops that range in song mode:
  playback wraps from the end to the start, and Stop returns to the start.
  `transport.clear_loop` removes it. Exports always render the whole song.
  UI: Shift+drag on the ruler; right-click the ruler → Clear loop.
- `playlist.add_marker <bar> <name>` / `playlist.remove_marker <bar>` manage named
  markers (one per position; re-adding at a position renames the marker and keeps
  its action). UI: right-click the ruler → Add marker here, or Alt+T for a marker
  at the playhead (on the beat); clicking a marker's label jumps the playhead to it.
  `playlist.move_marker <from> <to>` (undoable) moves a marker and keeps its
  name and action; it is refused onto another marker's position and for
  time-signature changes (those move with `playlist.set_time_signature`).
  UI: drag a marker's label along the ruler (snapped like clips; an accent
  line shows where it lands). Double-clicking a marker's label renames it in
  place (`playlist.add_marker` at its position, which keeps its action).
  The window menu's Markers submenu also adds a pause marker at the playhead,
  adds a marker named after its bar every 4 or 8 bars through the end of the
  song (bars that have one keep it), and removes all markers except time
  signature changes; each is one undo step. "Time signature change at this
  bar…" asks for a signature (filled with the one in force) and runs
  `playlist.set_time_signature` at the start of the playhead's bar; `none`
  removes the change there. It is disabled in the first bar, whose signature
  is the project's.
- Both are saved and undoable.

### Marker actions: making a song stop instead of looping

- Song playback loops by default: at the end of the song it starts again from the
  beginning. Markers change that. `playlist.set_marker_action <bar>
  <none|pause|song_loop>` (undoable) sets what a marker does; UI: right-click a
  marker → Label only / Pause here / Loop the song here, or right-click the ruler → Add pause
  marker here (a marker named "Pause", one undo step).
  - `none` (default): only a label, drawn as a flag.
  - `pause`: when song playback reaches the marker it pauses there (transport
    `paused`); the playhead stays on the marker, Play continues from it (it
    does not pause again on the marker it starts from) and Stop rewinds. Put
    one at the end of the song to stop instead of looping. Drawn as two red
    bars.
  - `song_loop`: when playback reaches the end of the song it returns to this
    marker instead of the beginning; Stop still returns to the beginning. Only one
    marker is the song loop: choosing it moves it there (one undo step). A
    song-loop marker at or past the last clip extends the song to it (so exports
    include the silence up to it) and playback then wraps to the beginning. Drawn as
    a down-facing arrow.
- Marker actions apply in song mode while no loop region is set; a loop region
  takes over until it is cleared. Pattern mode always loops the pattern.
- Exports always render the whole song once and ignore pause markers.
- Project files store `"action"` only for markers that have one.
- `playlist.set_markers <json>` replaces every marker; undo uses it to restore
  markers exactly.

## Clip kinds

- **Pattern clips** (`playlist.add_clip`) play a pattern, repeating it.
- **Pattern colour**: a pattern has no colour until one is set with
  `pattern.set_color <pattern> <1-16|none>` (undoable; also from the pattern
  list's right-click menu). An uncoloured pattern's clips are drawn in a
  neutral fill and its name in the plain text colour. Inside a pattern clip
  the notes are drawn in their own colours, as in the piano roll: a note's
  colour group, or its channel's colour for group "channel". Notes of one
  pattern can therefore show several colours.
- **Note rows in a preview**: notes sit by key, the lowest at the bottom.
  When every note of a pattern has the same key (a step-sequenced pattern,
  such as drums) and more than one channel plays, each channel gets its own
  row instead, in channel rack order from the top, so the preview reads like
  a small step sequencer.
- **Audio clips** (`playlist.add_audio_clip <channel> <track> <start> <length>`)
  play a sampler channel's sample once from the clip start, unpitched (at the
  channel's root key), cut at the clip end. Dropping a sound from the browser on
  the playlist adds a sampler channel with one-shot off and an audio clip as long
  as the sample. While the sound hovers a lane, a ghost clip one bar long
  marks the snapped spot (the sample's length is known only once it loads) and
  the track's header gets an accent outline. Dropped on a track header instead,
  the sound lands at the start of the visible range (the first snap line in
  view) on that track, and a track that still has its default name takes the
  sound's name. Removing the channel removes its audio clips (undo restores).
- **Automation clips** (`playlist.add_automation_clip <automation> <track>
  <start> [length]`) move one parameter during song playback (see below).
- `playlist.remove_clip`, `move_clip`, `resize_clip` and selection editing work
  the same for every kind. Ctrl+B duplicates any kind.

## Clip editing

- A clip has an **offset**: ticks into its source where it begins (0 for a
  new clip). A pattern clip with an offset starts part-way into the pattern
  (notes starting before it are cut); an audio clip starts part-way into the
  sample (converted at the project tempo); an automation clip shows the curve
  from that point. The add-clip commands take it as an optional last argument
  and undo keeps it; project files store it only when it is not 0.
- `playlist.slice_clip <track> <at>` cuts the clip covering `at` into two clips
  there (the second keeps playing where the first stopped).
- `playlist.split_clip <track> <at>` cuts a pattern clip in two at `at` and
  gives each piece a new pattern of its own holding only what that piece
  plays, from the piece's start (offset 0): notes starting before the piece
  are dropped, notes running past its end are shortened, and repeats of a
  pattern shorter than the clip are written out. Pattern automation lanes are
  cut the same way; a lane gets a point at the piece's start holding the value
  of the last point before it. The pieces are named "<name> #2", "#3", … and
  are as long as the piece (in whole steps, at least one); the original
  pattern stays in the list for its other clips. One undo step. Fails for
  audio and automation clips (slice those) and for a piece longer than 1024
  steps.
- `pattern.split_by_channel <pattern>` makes one new pattern per channel the
  pattern plays, in rack order, named "<name> - <channel>", each holding that
  channel's notes and pattern automation lanes; lanes not aimed at one of
  those channels (mixer targets) go with the first. Length, time signature
  and colour are copied. Every playlist clip of the pattern is replaced by
  one clip per new pattern at the same start, length and offset (muted
  clips stay muted): the first on the clip's own track, the others on the
  following tracks, each skipping down past tracks where it would overlap
  another clip. The original pattern stays in the list; the first new
  pattern is selected. One undo step. Fails for a pattern with notes on
  fewer than two channels, or when the clips would not fit on the 64 tracks.
- `playlist.make_unique <track> <at>` gives the clip its own copy of its
  pattern, automation or sample channel, named "<name> #2" (#3, …); other
  clips keep the original.
- `playlist.merge_clips <track> <at> [<track> <at> …]` merges pattern clips
  into one new pattern ("Merged n") holding everything they played, at their
  positions, and one clip of it on the first of their tracks spanning them all.
  Fails if that clip would overlap another or the pattern would exceed 1024
  steps.
- `playlist.consolidate <track> <at>` renders a pattern or audio clip alone
  (effects included, 48 kHz 32-bit float) to `consolidated/<name>-<n>.wav`
  beside the project, adds a sampler channel for it (one-shot off) and makes
  the clip an audio clip of it. The project must be saved first. Undo puts the
  clip back and removes the channel; the file stays.
- `playlist.reverse_clip <track> <at>` and `playlist.set_clip_fade <track> <at>
  <in|out> <seconds>` (0–10 s) set the sampler parameters `reverse`,
  `fade_in` and `fade_out` of an audio clip's channel (also in channel
  settings). Reversed samples play from their end. The fade-in rises from the
  note's start; the fade-out falls over the last seconds before the note or the
  sample ends. Make the clip unique first to treat one clip on its own.
- `playlist.insert_space <start> <length>` moves every clip and marker at or
  after `start` later by `length`; a clip across `start` is sliced there and
  its second part moves. `playlist.delete_space <start> <length>` removes that
  stretch: clips inside it go, clips across its edges are cut back, and later
  clips and markers move earlier. Both are one undo step.
- UI: the playlist's window menu → Clip acts on the selected clips (Slice at
  playhead, Split at playhead into new patterns, Split by channel (the
  selected clips' patterns), Make independent copy, Merge, Consolidate,
  Reverse, fades; the pattern list's right-click menu and the toolbar's
  Patterns menu have Split by channel too), and
  window menu → Edit → Insert time / Remove time inserts or removes the loop
  region's time; Ctrl+Insert and Ctrl+Delete do the same (they need a loop
  region: Shift+drag the ruler).

## Audio recording

- With Options → Record audio input on, REC armed and the transport playing in song
  mode, the input device chosen in Settings → Audio (the system default unless
  set) is captured. When playback stops or REC is
  disarmed, the take is written as a 32-bit float WAV to `recordings/` beside the
  project (the per-user data folder for unsaved projects) and placed as an audio
  clip at the position where recording started, as one undo step: on the first
  **armed** playlist track that has room for it (`playlist.toggle_track_arm
  <track>`, undoable, saved; the red dot in the track header), else on the
  first empty track. Takes under 0.1 s are discarded. Input arrives through a lock-free
  ring buffer holding four seconds; no latency compensation is applied yet.

## Automation

- An automation has a name, a target and points (tick from clip start, value
  0..1 as a control position). Targets: `channel:<n>:volume|pan|<param>`,
  `mixer:<t>:volume|pan`, `mixer:<t>:<slot>:<param>`. Values map through the
  same curves as the controls (fader curve for volume, log for frequencies,
  cubic for times).
- Commands: `automation.create <target> [name]` (two points at the current value,
  spanning four bars), `automation.remove`, `automation.rename`,
  `automation.add_point`, `automation.remove_point`, `automation.move_point`,
  `automation.set_points`. All undoable.
- **Curve types.** Each point also sets the shape of the segment from it to the
  next point, and a **tension** (-1..1, default 0) that bends it:
  - `single` (default): one curve; tension 0 is a straight line, positive
    tension starts slowly and finishes fast, negative the opposite (the
    position is `t^(4^tension)`).
  - `double`: an S-curve, the single curve mirrored about the middle.
  - `hold`: keeps the point's value until the next point, then jumps.
  - `stairs`: equal steps; tension sets their number (2 at -1, 16 at 1).
  `automation.set_curve <automation> <point time> <single|double|hold|stairs>
  [tension]` (undoable). The editor names the four shapes Curve, S-curve,
  Hold and Steps. Saved as `"curve"` and `"tension"` on a point only
  when not the defaults. The last point's shape is unused. Pattern lanes keep
  straight segments.
- Playback samples curved segments every 6 ticks (a quarter step) into the
  engine's straight-line lanes; `hold` segments change exactly at the next
  point.
- **Settings.** Each automation also has an **On** switch (default on) and a
  **range**, `low` and `high` (control positions 0..1, defaults 0 and 1). The
  curve's bottom plays as `low` and its top as `high`, so the target gets
  `low + (high - low) × value`; `low` above `high` inverts the curve. An
  automation that is off plays nothing, and its clips show their curve dimmed
  by half. `automation.set_enabled <automation> <on|off>` and
  `automation.set_range <automation> <low 0-1> <high 0-1>` (both undoable).
  Saved as `"enabled": false`, `"low"` and `"high"` only when not the
  defaults; files without them load unchanged.
- During song playback each automation clip writes its interpolated value once
  per audio block (at most 256 frames) while the playhead is inside the clip;
  afterwards the parameter keeps its last value until playback stops, when the
  project's values are restored. Automation does not play in pattern mode.
  Exports include automation.
- A mixer track or effect slot driven by an automation cannot be removed until
  the automation is deleted; moving or removing other effect slots keeps
  automation pointing at the right effect.

### Creating automation (UI)

- Right-click any knob or fader → "Create automation clip" places a new clip
  on the first empty track and opens it in the automation editor.
- The **last moved control** is the last knob or fader the user dragged,
  typed into or reset. Add → "Automation clip for the last moved control"
  does the same as "Create automation clip" on it, and Tools → "Last moved
  control" offers the control's menu items (Create automation clip, Add
  controller, Copy, Paste, Reset). Both are disabled (and name no control)
  until a control has moved; their hint names the control.
- The playlist sidebar lists automations; clicking one paints it instead of
  the pattern; double-clicking a clip or list entry opens the automation
  editor.

### Automation editor (UI)

One window shows one automation (`docs/spec/style/dialogs.md`, "Automation
clip editor window").

- **Header**: the window menu (Copy curve, Paste curve; Flip vertically,
  Normalize; Select all, Delete selected points; each edit a single
  `automation.set_points`, one undo step), a chevron button that lists every
  automation so the window can switch (the wheel over it steps through
  them), and the title "Automation — <name>".
- **Settings band**: the On switch, then Low and High knobs (0–100 %),
  committed as `automation.set_enabled` / `automation.set_range` (one undo
  step per knob drag; double-click resets to 0 % and 100 %).
- **Graph**: a bar ruler over the grid, the curve with a faint fill under it,
  a filled circle at every point and a small hollow **tension handle** at the
  middle of every segment, on the curve. A thin playhead shows while the song
  plays inside one of the automation's clips.
- **Gestures** (they replace the earlier click-to-add ones):
  - Right-click empty grid: adds a point there; holding the button and
    dragging places it before it is added. Shift+right-click adds a point on
    the curve at that time without changing its shape.
  - Left-click a point selects it (Shift+click adds or removes it); left-click
    empty grid clears the selection. Ctrl+A selects every point.
  - Left-drag a point moves it, and every selected point with it. Shift locks
    the value, Ctrl locks the time, Alt bypasses snap. Points cannot pass
    their unselected neighbours, and the first point stays at time 0 or
    later. Dragging the last point to the right makes the automation longer.
  - Left-drag empty grid (or Ctrl+drag anywhere) draws a selection box;
    Shift adds to the selection.
  - Left-drag a tension handle up or down bends its segment towards the
    pointer; Ctrl gives fine steps. Right-click a handle resets its tension.
    Alt+drag on a segment also bends it.
  - Alt+left-click a point deletes it. Delete removes the selected points,
    never the last one.
  - Right-click a point opens the **point menu**: a dim "Point n of m"
    heading; Delete; "Shape of the segment before this point" with the four
    shapes as radio items and Reset tension (not on the first point); Copy
    value, Paste value and Type value… (a small field over the point, 0–100,
    Enter applies).
  - **Step drawing** (footer toggle): left-drag on the grid draws, putting a
    point on every snap line the pointer passes and replacing the points
    between the drawing's ends; one undo step.
  - **Slide** (footer toggle): dragging a point also moves every later point
    by the same time.
  - **Snap** (footer toggle, on by default) rounds times to steps.
  - Wheel scrolls horizontally, Ctrl+wheel zooms around the pointer, Home or
    the footer's magnifier zooms out to the whole curve; the horizontal bar
    under the graph scrolls and zooms as in the other editors.
  - Hovering a point or handle shows "Point 3 of 7 · 62 % · bar 2.3" or
    "Tension +0.40" in the hint bar; while dragging, the value also floats
    beside the pointer.
- Every edit is one command and one undo step: `automation.add_point`,
  `automation.move_point` (one point), `automation.remove_point`,
  `automation.set_curve`, or `automation.set_points` (several points, step
  drawing, slide, deleting a selection).

### Automation clips in the playlist

- The body is the clip colour darkened further than a pattern clip's, with
  the curve as a bright line and a translucent fill under it.
- While the pointer is over the body and the Draw or Select tool is active,
  its points and tension handles show, and the editor's point gestures work
  in place: left-drag a point or a
  handle, right-click empty body to add a point, right-click a point for the
  point menu, right-click a handle to reset it, Alt+click a point to delete
  it. Drags that start away from a point or handle, the title strip and
  double-click keep their clip meaning (move, trim, open the editor).
  Right-clicking the title strip still deletes the clip.

## Editor (UI)

- **Tools** as in the piano roll: Draw, Paint (drag to lay the selected
  pattern end to end, skipping places taken by other clips; the stroke stays
  on the track it started on however far the pointer drifts up or down; one
  undo step),
  Delete (click or drag over clips; one undo step) and Select (box select),
  plus Mute (click a clip to mute or unmute it: `playlist.toggle_clip_mute
  <track> <at>`, undoable; a muted clip stays, drawn faint, and does not
  play), Slice (click a clip to cut it at the snapped click; Shift+click
  splits a pattern clip into two new patterns, `playlist.split_clip`), Play (click
  moves the playhead there and plays) and Zoom (click zooms in, Shift+click
  out). Keys P, B, D, E, T, C, Y, Z pick them.
- The **song play button** (after the tools) plays the song from the
  playhead, switching to song mode first, or pauses while the song plays;
  right-click stops and returns to the start (while stopped: moves the
  playhead to the start). It is lit while the song plays, like the piano
  roll's pattern play button (`piano-roll.md`).
- The **snap button** (before the tools) shows the snap glyph and value; a
  click lists the values, the wheel over it steps to the next coarser value
  (up) or finer one (down), and right-click returns it to Bar.

- Left-click on an empty cell places the selected pattern at the snapped bar;
  dragging a clip moves it; dragging its right edge resizes it and dragging
  its left edge trims its start (`playlist.trim_clip`; with several clips
  selected, all of them by the same amount); right-click
  deletes. The pattern list next to the playlist selects the pattern to paint.
- **Pattern miniatures**: the picker shows each pattern as a small clip, so
  patterns are told apart by their content even when they keep their default
  names. A miniature is the width of the picker (less the scroll bar, which
  takes its own strip when the list scrolls, so it never covers a miniature)
  and 34 points tall: the name in a title strip, then one pass of the
  pattern's notes laid out as in a clip preview (above), on the pattern's
  colour or the neutral fill. Automation recorded into the pattern shows as
  faint curves under the notes. A pattern with neither notes nor automation
  says "empty". The pattern being painted is outlined in the accent colour. Click selects it for painting; double-click selects it and brings
  the piano roll to the front with it (opening it if it was closed); drag
  it onto a lane to place a clip of it at the snapped drop point (a ghost
  clip of the pattern's length shows where; one undo step); right-click
  opens the pattern menu (rename, show in playlist, clone, colour, time
  signature, clear automation, delete). Rename in place is F2, the menu or
  the toolbar's pattern field. Show in playlist scrolls to the pattern's
  first clip.
- The playlist is an editor window like the channel rack, piano roll and
  mixer. By default it is maximized as the backdrop behind them
  (`window-layout.md`, "Playlist"). Click
  or drag on the ruler to move the playhead; during song playback the view
  follows the playhead.
- Selection works as in the piano roll: click, Shift+click, Ctrl+drag rectangle;
  dragging moves or resizes every selected clip; Delete removes; Ctrl+A selects
  all; Ctrl+X, Ctrl+C and Ctrl+V cut, copy and paste (pasted clips land at
  the playhead, snapped down, on their own tracks, and become the
  selection); Ctrl+B duplicates the selection right after its end; Shift+Left and
  Shift+Right nudge it one snap step, Shift+Up and Shift+Down move it one
  track (nothing moves if a clip would leave the playlist). One undo step
  each. The window-local keys are listed in `keymap.md`, "Over the playlist".
  The window menu's Select submenu also inverts the selection and selects
  the muted clips, every clip of the selected clips' sources, or every clip
  overlapping the loop region.
- Ctrl+scroll zooms horizontally, and so do Page Up and Page Down, which
  keep the playhead where it is in the view. Shift+1 to Shift+3 pick fixed
  zooms (32, 96 and 240 points per bar), Shift+4 zooms all the way out and
  pressed again goes back, Shift+5 fits the loop region. The shared scroll/zoom row runs above the
  ruler (as in the piano roll): chevrons that scroll a bar and repeat when
  held, a rail that pages and shows a miniature of the clips, a thumb that
  scrolls and whose ends zoom, and the vertical zoom box, which sets the
  track height (18 to 96 points; double-click resets 36). A vertical scroll
  bar runs down the right of the ruler and tracks.
- The ruler has short beat ticks and longer bar ticks rising from its bottom
  edge; in song mode a triangle marks the playhead and its line runs down the
  lanes whether or not the song is playing (the view follows it only while
  playing). A clip's glyph and name stay in view when the clip starts left
  of the visible lanes. The glyph lights up under the pointer; a click on it
  (with any tool) selects that clip alone and opens the Clip menu beside it,
  while a drag from it trims the clip's start as a drag from the left edge
  does. Zoomed far in (a bar wider than 240 points), each
  bar is its own group for the shading and the strongest lines.
- The window menu's Tracks submenu unmutes every muted track and clears
  every track solo, each as one undo step.
- Dragging with the middle button anywhere in the view pans it both ways,
  with the grabbing hand as the pointer; it edits nothing.
- The window menu's View submenu hides the clip list (Alt+P), moves it to
  the right side, sets its width (Narrow 100, Normal 130 or Wide 180
  points) and turns off the bar-group shading, the track separators and
  following the playhead (the list on the left at Normal width, and the
  rest on, by default; not saved with the project). With the clip list
  hidden the lanes take its width.
- Double-clicking a pattern clip opens it in the piano roll (brought to the
  front); an automation
  clip opens the automation editor; an audio clip opens its channel's
  settings.

## Sources

- Double-clicking a clip-list pattern to open the piano roll, dragging
  patterns from the clip list onto the lanes, and paint strokes that keep
  to one track: the owner's requests (2026-10-23). Gestures and feedback are
  our own design.
- The scroll/zoom row, song overview and track-height zoom box: the
  interface style guide (`docs/spec/style/main-window.md`, "Scroll and zoom
  bars"). Drawing and sizes are our own.
- The playlist as a window, its header and window menu, one lane tone with
  step, beat, bar and bar-group lines, bar-group shading, and clips drawn as a
  strong title strip over a dark body: the interface style guide,
  `docs/spec/style/playlist.md` (owner's 2026-10-21 decision to follow the
  manual's layout). The labels Make independent copy, Insert time, Remove
  time, Label only, Pause here, Loop the song here, Curve, S-curve, Hold and
  Steps are our own wording.

- Automation curve types (single curve, double curve, hold, stairs) and
  tension follow the automation clip point modes described in the FL Studio
  online manual, "Automation clips"; the formulas, ranges and step counts are
  our own.
- The automation editor's gestures, settings band (On, Low, High), point
  menu, step drawing, slide and in-clip editing: the interface style guide
  (`docs/spec/style/dialogs.md`, "Automation clip editor window" and
  "Automation clip in the playlist"), which compared the FL Studio online
  manual's "Automation Clips" page for placement and behaviour only. The
  menu wording, the handle-bend direction, plain-drag box selection and the
  Draw/Select-tool rule in the playlist are our own design.
- The last moved control in the Add and Tools menus: the interface style
  guide (`docs/spec/style/toolbar.md`); the item wording is our own.

- Patterns without a colour of their own, with notes shown in their own
  colours inside clips: the project owner's account of how the software
  behaves (2026-10-21); the neutral fill is our own.
- Pattern miniatures in the picker: the owner's request (2026-10-21), compared
  with the picker panel in the FL Studio online manual, "Playlist"
  (`playlist.htm`), under the "layout plus general feel" decision in
  `docs/clean-room.md`. Sizes, the empty label, the outline and the
  one-row-per-channel layout for step patterns are our own.
- Pattern clips arranged on a playlist and a song/pattern mode switch follow the
  workflow described in the FL Studio online manual, "Playlist" section. Row
  count, overlap rule and command names are our own.
- Marker actions follow the time-marker actions described in the same manual
  section ("Pause": playback pauses at the marker; "Loop": playback returns to the
  marker at the end of the song, and one placed past the last data extends the
  render; the loop marker drawn with a down-facing arrow), and its "Add time
  marker" shortcut Alt+T ("Keyboard shortcuts"). Glyphs, colours, the one-loop-
  point rule, resuming past a pause and command names are our own.
- Playlist track names, colours, mute and solo follow the FL Studio online
  manual, "Playlist" (track header options). The colour palette and command
  names are our own.
- Clip editing (slice, make unique, merge, consolidate, reverse, fades, insert
  and delete space with Ctrl+Insert / Ctrl+Delete) follows the playlist clip
  menu and shortcuts in the FL Studio online manual, "Playlist" and "Keyboard
  shortcuts". The offset model, naming and file layout are our own.
- Splitting a clip into new patterns and splitting a pattern by channel: the
  owner's request (2026-10-23), compared with the "Split by channel" pattern
  option in the FL Studio online manual ("Channel Rack" and "Playlist" pattern
  menus). Baking the pieces, the track placement, names and Shift+click on the
  slice tool are our own.
- Per-track arming for audio recording and input device selection follow the
  FL Studio online manual, "Recording" (arming and input selection). Arming
  playlist rows (rather than mixer tracks) is our own simplification.
