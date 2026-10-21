# Spec: style guide (layout and general feel)

This guide describes how MusicCraft should look and feel so that someone who
knows the classic pattern-based DAW workflow feels at home within minutes. It
is written under the owner's **"layout plus general feel"** decision of
2026-10-21 (`docs/clean-room.md`, *Look and feel*). Read that section first;
this guide does not widen it.

What this guide **does** describe, from the public online manual and from
normal use of software we lawfully run:

- which windows and panels exist, where they sit, their proportions and
  their order;
- what each control does, where it sits in its row, and how it reacts;
- the density of the interface: a dark, compact UI with small controls;
- the *roles* colours play (background, accent, warning) and how strong the
  contrast between them is.

What it **never** describes, and what implementers must never add:

- colour values sampled from screenshots, or colours chosen to match the
  other product's skin. Our hues are our own (see *Colour* below);
- traced or overlaid screenshots, or pixel-for-pixel measurements;
- their icons, bitmaps, fonts, logos, skins, sample packs or presets;
- their product names, plugin names or branded feature names (see *Names*).

A note on process: the classic two-team clean room (one side writes notes,
the other builds from them) is **not** what protects this project, because
we cannot claim that separation (`docs/clean-room.md`, *Why this document
exists*). What protects it is following the rules above. Notes taken from a
screenshot are fine; matching its colours, artwork or wording is not, no
matter who builds from the notes.

---

## 1. Design principles

1. **The workspace is the hero.** Window chrome is thin; the grid, the steps,
   the notes and the faders take the room. No large title bars, no generous
   padding, no card-style UI with big rounded corners.
2. **Dark and low-glare.** The whole application is dark. Large areas are
   close in brightness to each other; brightness is spent only where the eye
   should go (active steps, notes, clips, the playhead, meters).
3. **Compact and dense.** Controls are small (buttons around 24 × 22 points,
   knobs around 18–24 points). A lot fits on one screen; nothing feels
   cramped because alignment is strict.
4. **Everything is a floating, dockable window inside one main window.** The
   toolbar is fixed at the top, the browser is docked at the left, and the
   editors (channel rack, piano roll, playlist, mixer) float over the
   workspace, snap to each other, can be maximised or detached.
5. **Hover teaches.** Every control writes a one-line explanation into the
   hint bar when hovered, and its value while it is being changed. There are
   few text labels on the controls themselves.
6. **Right-click always does something useful.** Every control and every
   panel has a context menu. Middle-click / double-click resets a value
   (`docs/spec/controls.md`).
7. **Hardware metaphor, lightly.** Displays look like small dark LCD
   windows; switches look like small lamps (LEDs); knobs and faders look like
   physical controls, drawn flat and simple in our own style.

---

## 2. The main window

Top to bottom:

```
+---------------------------------------------------------------------------+
| MENUS  | transport | song pos | tempo | pattern | snap | meters | windows |  toolbar
| hint   |           |          |       |         |      |  CPU   | buttons |  (one band
+--------+------------------------------------------------------------------+   when wide)
|        |                                                                  |
| BROWSER|   WORKSPACE: floating editor windows                             |
| (left, |   - Playlist usually large, filling most of the workspace        |
|  narrow|   - Channel rack floating over it, near the top-left             |
|  tree) |   - Piano roll opened on demand, large                           |
|        |   - Mixer usually docked or floating along the bottom            |
|        |                                                                  |
+--------+------------------------------------------------------------------+
```

- **Toolbar** across the full width at the top (section 3).
- **Browser** docked at the left edge: a narrow column (about 170 points,
  draggable wider), full height under the toolbar (section 4).
- **Workspace** fills the rest. Its background is the darkest large surface
  in the app, slightly textured or flat; editor windows sit on it.
- The default project opens with the **playlist** large and the **channel
  rack** floating on top of it near the top-left. The **mixer** and **piano
  roll** open from the toolbar window buttons or F-keys.

### Window behaviour

- Editor windows (channel rack, piano roll, playlist, mixer) have no OS
  title bar. Their first row is a **compact header** (see
  `window-layout.md`): window-menu arrow, title in small text, the window's
  own tools on the same row, then minimise/maximise/close at the right end.
- Windows **snap** to each other's edges and to the workspace edges while
  dragging; holding Alt while dragging turns snapping off.
- Any editor can be **maximised** inside the workspace, or **detached** into
  its own OS window (useful on a second monitor).
- The focused window's header is a little brighter than unfocused ones; that
  is the only focus cue. No thick focus rings.
- The default window shortcuts are F5 playlist, F6 channel rack, F7 piano
  roll, F9 mixer and Alt+F8 browser (as defined in our keymap data; see
  `keymap.md`).

---

## 3. Toolbar

The toolbar is a band of small **panels** placed side by side. Each panel is
a group of related controls, separated from its neighbours by a thin gap or
a subtle divider. Users can hide panels; the default order, left to right:

### 3.1 Menus and hint bar (far left, stacked)

- **Upper row:** text menus in small capitals or small text: FILE, EDIT,
  ADD, PATTERNS, VIEW, OPTIONS, TOOLS, HELP.
- **Lower row:** the **hint bar**, a dark inset strip showing the hint for
  the control under the mouse, or its value while dragging. On its right end
  small **status icons** say what the hovered control supports: can be
  automated, can be linked to MIDI, has a right-click menu. Draw these as our
  own small glyphs.
- Optionally the project name sits beside or above the menus.

### 3.2 Transport

Left to right:

- **PAT / SONG switch**: two small stacked or side-by-side text buttons;
  the active one is lit with the highlight colour. PAT plays the current
  pattern in a loop, SONG plays the playlist.
- **Play** (triangle), **Stop** (square), **Record** (circle). Same size,
  square, flat. Play lights up (accent) while playing; Record lights up in
  the danger colour while armed. Stop is never lit.
- Optional small toggles next to them: **metronome**, **wait for input**,
  **count-in**, **loop record**, **step edit**, **typing keyboard to
  piano**. Each is a tiny icon button that glows when on.

### 3.3 Displays (LCD style)

Each display is a dark inset rectangle, a little darker than the toolbar,
with large bright digits and a tiny caption under or beside the number.

- **Song position**: large `bar.beat.step` (or `min:sec:cs`) digits. Click
  and drag to move the playhead. A small switch toggles bar/time mode.
- **Tempo**: large BPM digits with three decimals in smaller digits (e.g.
  `124.000`). Drag up/down to change, right-click for a menu (tap tempo,
  presets, type in). A **TAP** button may sit beside it.
- **Time signature** (our addition, compact): `4/4` in a small display.

### 3.4 Pattern panel

- A **pattern selector**: the current pattern's name in a dropdown-like
  field with a small `+` button to add a new pattern. Right-click lists all
  patterns.

### 3.5 Snap panel

- A small dropdown showing the global snap (e.g. `Bar`, `Beat`, `1/2 step`,
  `Line`, `Cell`, `(none)`). It is the default snap for the piano roll and
  playlist.

### 3.6 Output and performance meters

- **Oscilloscope / peak display**: a tiny dark window showing the output
  waveform, or a short horizontal peak meter. Peaks that clip show the danger
  colour.
- **CPU** and **polyphony** readouts: small numbers with a tiny caption, and
  optionally a small running graph.
- **Master volume** and **master pitch**: two very small knobs or a small
  slider pair.

### 3.7 Window buttons (right end)

A row of square icon buttons that toggle the main editors. The usual order:
**Playlist, Piano roll, Channel rack, Mixer, Browser** (plus optional
plugin picker / project info / audio editor). A button is lit (accent
background or accent glyph) while its window is open. Each icon is our own
drawing of what the window shows (see section 10).

### 3.8 Layout of the band

- When the window is wide (about 1300 points or more) everything is one
  band; when narrow, it wraps into two bands (`window-layout.md`).
- Panels never use big gaps; their controls align on one baseline.

---

## 4. Browser

A narrow docked column at the left edge.

- **Header row**: small caps title `BROWSER`, then tiny buttons on the
  right: a refresh arrow, a view/sort menu, and a search field (or a
  magnifier icon that expands into a field).
- **Tree**: a folder tree drawn with small disclosure triangles. Top-level
  groups in a slightly larger or brighter text: *Current project*,
  *Plugin database* (with *Generators* and *Effects* under it), *Packs*
  (sample folders), *Presets*, *Recorded*, *Rendered*, user folders.
- **Row icons**: each row has a small glyph before its name: a folder, a
  note glyph for a generator/instrument, a waveform glyph for an audio file,
  a "knob" or "slot" glyph for an effect, a document glyph for a project or
  preset. Glyphs are monochrome, in the dim text colour; the selected row
  is highlighted with a subtle bar.
- Clicking an audio file **previews** it; a tiny waveform appears in a
  strip at the top or bottom of the browser while it plays.
- Items are **dragged** onto the channel rack (new channel), the playlist
  (audio clip), or a mixer slot (effect).
- Text is small (smaller than the body text elsewhere), rows are tight
  (about 18–20 points tall), and indentation per level is small.

---

## 5. Channel rack

A floating window, usually just wide enough for a 16-step pattern.

### 5.1 Header

Arrow / window menu, title (`Channel rack`), **display filter** dropdown
(which channel group is shown, e.g. *All*), the pattern's name, **swing**
knob, **steps** (pattern length), a **graph editor** toggle, close.

### 5.2 A channel row, left to right

1. **Mute LED**: a small round or rounded-square lamp. Lit (accent) =
   channel playing; dark = muted. Right-click / Ctrl-click solos.
2. **Pan knob**: tiny.
3. **Volume knob**: tiny.
4. **Mixer track number**: a small boxed number showing which mixer insert
   the channel feeds (`--` or blank for master). Drag up/down to change.
5. **Channel name button**: a flat, slightly raised rectangular button
   (about 96 points wide) with the channel's name, cut with an ellipsis if
   long. Its background or left edge takes the channel's colour (dimmed so
   the text stays readable). Click opens the channel settings; right-click
   opens the channel menu.
6. **Step buttons** *or* a **piano roll preview** (see below).
7. **Selection switch**: a small rounded bar at the far right, apart from
   the steps. Outline when not selected, filled with the accent when
   selected.

Row height is uniform and tight (about 24–26 points); rows have a hairline
separator or simply alternate very slightly in brightness.

### 5.3 Step buttons

- 16 steps by default (4 beats × 4 steps); each is a 16th note.
- Each step is **taller than wide**, with a tiny gap between steps.
- **Grouping by beat**: steps alternate in shade every beat (4 steps a
  lighter grey, 4 steps darker, and so on) so the row reads in groups of
  four. This alternation is the main visual rhythm of the rack.
- An **off** step is a flat, dim, slightly raised pad in the beat shade. An
  **on** step is lit: a bright fill (our highlight colour) with a slightly
  lighter top edge so it reads as a lit key. It must be visible at a glance
  from across the room.
- The step under the playhead shows a faint flash or outline while playing.
- Left-click / drag turns steps on, right-click / drag turns them off.

### 5.4 Piano roll preview

When a channel has notes from the piano roll instead of steps, the step area
is replaced by a **read-only miniature** of its notes: thin bars on a dark
background, with alternating lighter and darker blocks per bar. Clicking it
opens the piano roll for that channel.

### 5.5 Bottom of the rack

A **"+"** button centred under the last row adds a channel (opens a list of
instruments). The window's bottom-right corner can be dragged to show more
steps.

### 5.6 Graph editor

Toggling the graph editor shows, under the steps, a lane of vertical bars
per step for velocity, pan, release, fine pitch etc., chosen by small tabs
at its left.

---

## 6. Piano roll

A large window, usually maximised or nearly so.

### 6.1 Header / toolbar

Left to right: window menu arrow, title (`Piano roll — <channel>`), a
**tools menu** button, **snap** dropdown, a **scale/key** helper toggle,
**chord stamp** toggle, then the tool buttons in this order:

| Tool | Default key | Glyph idea (draw our own) |
|---|---|---|
| Draw | P | pencil |
| Paint | B | brush |
| Delete | D | eraser |
| Mute | T | crossed-out speaker or note |
| Slice | C | blade / cut line |
| Select | E | dashed rectangle |
| Zoom | Z | magnifier |
| Play selected / scrub | Y | small triangle with a cursor |

then a **play/pause** button for the pattern, and at the right end a
**target channel** selector and **ghost notes** toggle.

The active tool's button is lit with the accent colour; others are flat.

### 6.2 Body

- **Keyboard** on the left: a vertical piano keyboard, white and black
  keys drawn flat in our own greys; the C keys are labelled (`C4`, `C5`…) in
  tiny text. Clicking a key previews the note. Low notes at the bottom.
- **Ruler** across the top: bar numbers, finer ticks for beats; markers
  sit in a thin strip above it.
- **Grid**: rows alternate shade for white-key and black-key pitches
  (black-key rows darker). Vertical lines: faint for steps, a little
  stronger for beats, clearly stronger for bars. The C row boundary can be
  marked a little more strongly to help orientation.
- **Notes**: flat rounded rectangles with a thin darker border and a
  lighter top edge. Fill colour = the note's colour group (16 groups, our
  own palette). **Velocity** is shown by the note's brightness/saturation
  (quiet notes duller). The note's name (e.g. `C5`) is written inside the
  note at its left end when the note is wide enough. Selected notes are
  drawn brighter or inverted with a strong outline; muted notes are grey.
  Slide notes carry a small triangle marker.
- **Ghost notes**: notes of other channels in the same pattern are drawn
  as faint, low-opacity bars (solid but dim); notes from overlapping clips
  as faint outlines.
- **Playhead**: a thin bright vertical line across grid and ruler, with a
  small triangle head in the ruler.
- **Scrollbars**: thin, with a zoom handle on each end (dragging an end of
  the horizontal bar zooms).

### 6.3 Event editor (bottom)

A lane under the grid (about 96 points tall, resizable by a divider). At
its left a **target** dropdown (Velocity, Release, Pan, Mod X, Mod Y,
Pitch, automation targets). Each note shows as a **vertical line with a
small circle at the top**, its height = the value, in the note's colour.

---

## 7. Playlist

Usually the largest window, the arrangement view.

### 7.1 Header / toolbar

Window menu arrow, title (`Playlist — Arrangement`) with an arrangement
dropdown, then the **snap** dropdown and tool buttons in order:

Draw (pencil), Paint (brush), Delete (eraser), Mute, Slip (two arrows
inside a frame), Slice (blade), Select (dashed box), Zoom (magnifier),
Play-selected, then Play/Pause. Lit tool = accent; same glyphs as the
piano roll so tools read the same everywhere.

### 7.2 Picker panel (left, inside the playlist)

A narrow column listing every **pattern**, **audio clip** and
**automation clip**, with small filter switches at the top (`All`, `Pat`,
`Aud`, `Aut`). Each pattern entry is a small block in the pattern's colour
showing a miniature of its notes. Toggle with Alt+P. Items are dragged or
clicked onto the playlist.

### 7.3 Track headers

Each track row starts with a header about 120–140 points wide:

- the **track name** (`Track 1` by default), left-aligned;
- an optional small **icon** chosen by the user (our own icon set);
- a **mute switch**: a small round LED at the right edge of the header, lit
  (accent) when the track plays; Right/Ctrl-click solos;
- the header background takes a dim tint of the track colour if one is set.

Default track height around 36 points; tracks can be resized by dragging
their dividers. There are many tracks (100+ by default); the empty ones are
simply blank rows.

### 7.4 Ruler and markers

Bar numbers across the top; time markers appear as small flags with text
above the ruler; the loop region is a coloured band in the ruler; the
playhead is a thin bright line with a triangle head.

### 7.5 Grid

The grid alternates every bar (or every few bars at low zoom) between two
very close shades of the background, plus faint vertical lines; horizontal
hairlines separate tracks. Grid density follows snap and zoom.

### 7.6 Clips

- All clips are flat rectangles with a **title strip** along the top
  holding the clip's name in small text (and a tiny menu glyph at the far
  left).
- **Pattern clips** show a miniature of their notes (or step hits) inside,
  in the clip colour, over a darker tinted body.
- **Audio clips** show the waveform, centred, filling the height; fade
  handles appear at the top corners on hover.
- **Automation clips** show a line with small points, filled faintly under
  the line, in our automation colour.
- Selected clips get a brighter outline; muted clips are greyed out.
- Clips may overlap on one track and draw with slight transparency.

---

## 8. Mixer

A wide, fairly short window, usually along the bottom of the workspace.

### 8.1 Header

Window menu arrow, title (`Mixer — <selected track>`), layout/view
switches (meter-wave view, extra properties), `+ Insert` (add track),
hint, close.

### 8.2 Strip area (left/centre)

A row of **narrow vertical strips** (about 40 points each), with the
**Master** strip first at the left, wider or set apart by a gap, then
inserts `1`, `2`, `3`… scrolling to the right. Each strip, top to bottom:

1. **Number** (or `M`) at the top in small text, tinted with the track
   colour.
2. **Name** written **vertically** (rotated), dim until set.
3. **Enable (ON) switch** / mute LED: small, lit when active.
4. **Pan knob**: small, centred.
5. **Fader + peak meter**: the fader slot beside a pair of thin vertical
   meters (left/right). The fader cap is a small flat horizontal block with
   a centre line. Meters are dark at rest and fill from the bottom; the
   colour gradient goes from our accent through the highlight to danger only
   near 0 dB. A peak-hold tick sits above the bar.
6. **Stereo separation knob**: small, under the fader.
7. **Level readout** in dB without the unit (`+0.0`).
8. **Solo (S)** / **record arm** small buttons.
9. **Effect count** and the **routing arrow** at the very bottom: a small
   up or down arrow shows whether this strip sends to the selected one.
   When a strip is selected, the strips it sends to show a lit arrow;
   cables can optionally be drawn.

The **selected strip** is outlined (highlight colour) and its background is
a little lighter. Strips sit on a darker trough; each strip is a slightly
raised panel with rounded corners of a few points at most.

### 8.3 Track inspector (right side)

A panel at the right of the mixer for the **selected track**:

- **Input** dropdown at the top (audio device input, e.g. *System
  default*).
- **Ten effect slots** stacked vertically, numbered `1`–`10`. An empty slot
  shows dim *empty* text and a small arrow/menu at its left to choose an
  effect; a filled slot shows the effect name, a small coloured enable LED
  at the left and a tiny **mix knob** at the right. Clicking the name opens
  the effect window.
- A small **3-band EQ** (low / mid / high knobs or a tiny curve) under the
  slots.
- **Output** dropdown at the bottom.

---

## 9. Other windows

- **Channel settings / instrument windows**: standard small window with a
  compact title bar. Top row has tabs (e.g. plugin, misc functions,
  envelope/instrument, sample) as small text tabs; a small preset arrow
  pair and preset menu in the title bar. The instrument's own face below.
  Our built-in instruments use our own panel art, never another product's
  layout or styling for a specific plugin.
- **Effect windows**: same chrome; title is the effect name; a preset
  dropdown and left/right arrows; an enable LED in the title row.
- **Dialogs** (project info, settings, render): tabbed, with labels on the
  left and controls on the right, all in the same dark, dense style.
  Settings has vertical tabs or a top tab row (Audio, MIDI, General, File,
  Project, Info).
- **Menus**: dark, compact, one-line items with right-aligned shortcut
  text; separators are hairlines; submenus with a small right triangle;
  checkmarks for toggles. Hover row = subtle accent tint.

---

## 10. Icons

All icons are **drawn by us** (`crates/musiccraft-ui/src/icons.rs`), as
simple vector strokes so they scale with the interface. What the original
manual tells us is only *what each icon means and where it is*; our
drawings decide how it looks.

Style rules:

- One stroke weight across the set, round caps, no gradients, no
  photographic or 3D rendering.
- Monochrome glyphs in the dim text colour when off; accent colour (or
  accent background with dark glyph) when on/active.
- Fit a square cell roughly 16–18 points inside a 24 × 22 button.

Icon meanings we need (glyph idea is ours):

| Purpose | Our glyph idea |
|---|---|
| Play / Stop / Record | triangle / square / filled circle |
| Metronome | small pyramid with a pendulum line |
| Wait for input / count-in / loop record | hourglass / "1-2" ticks / circular arrow with dot |
| Playlist window | stacked horizontal bars of different lengths |
| Piano roll window | keyboard edge with a few note bars |
| Channel rack window | a grid of small squares |
| Mixer window | three vertical lines with fader caps |
| Browser window | folder |
| Draw / Paint / Delete / Mute / Slip / Slice / Select / Zoom | pencil / brush / eraser / crossed speaker / two-way arrows in a frame / blade / dashed box / magnifier |
| Snap | magnet or grid lines |
| Hint icons (automatable, MIDI-linkable, has menu) | small dial-with-dot, small MIDI-plug-like circle, three-line menu |
| Browser rows | folder, note, waveform, plug/slot, document |

Do not reproduce their bitmaps, the shapes of distinctive icons, or the
product's logo anywhere (splash, about box, window icon, toolbar).

---

## 11. Colour

This is where we deliberately **differ**. The general *feel* we aim for is
"dark grey-blue workshop with one or two bright signal colours"; the actual
hues are MusicCraft's own and live in
`crates/musiccraft-editors/src/theme.rs`. Never pick or tune a hue by
comparing it with a screenshot of the other product.

Colour *roles* (the names in `theme.rs`):

| Role | Where | Relative brightness |
|---|---|---|
| `PANEL_DARK`, `BACKGROUND` | workspace, troughs, the grid background | darkest large areas |
| `PANEL` | window bodies, browser, toolbar | one small step lighter |
| `RAISED`, `RAISED_HOVER` | buttons, channel name buttons, strip panels | a further small step lighter; hover a bit more |
| `LCD_BG`, `LCD_EDGE` | inset displays, hint bar, meters' background | darker than the panel they sit in |
| `STEP_A`, `STEP_B` | alternating beat groups of off steps | two close greys, clearly distinguishable |
| `GRID_LINE`, `GRID_BEAT`, `GRID_BAR` | grid lines | three levels, faint → clear |
| `WHITE_KEY_ROW`, `BLACK_KEY_ROW` | piano roll rows | close, black-key rows darker |
| `TEXT`, `TEXT_DIM` | labels, values / captions, hints | light grey, never pure white; dim grey for secondary |
| `ACCENT` (our teal) | active tool, lit LEDs, play, selected switch, fader caps | the main signal colour |
| `HIGHLIGHT` (our amber) | lit steps, PAT/SONG active, selected mixer strip | secondary signal colour |
| `DANGER` | record armed, clipping | used sparingly |
| `AUTOMATION` | automation clips and curves | its own colour so automation always reads as automation |
| `PLAYHEAD` | playhead line | bright, warm |
| `PALETTE` / `track_color` | channel, pattern, track and note colours | 8 bright hues + darker variants, all our own |

Contrast rules:

- Panels differ from each other by small steps; only signals are bright.
- Text on panels is comfortably readable; dim text is still readable.
- A lit step, a lit LED and the active tool must be recognisable without
  colour vision (brightness change, not hue alone).
- Clip and note colours are muted on large fills (bodies) and full strength
  only on small marks (note bars, title strips).

---

## 12. Typography

- Our bundled UI font (see `theme::fonts`), never the other product's
  font.
- **Small**: body text around 11–12 points; captions and hints around
  9–10; LCD digits large (around 18–22) in a monospace or tabular face.
- Section titles in the browser and mixer use small caps or uppercase,
  dim, with slight letter spacing.
- Numbers always use tabular figures so displays don't jitter.

---

## 13. Controls (look only; behaviour is in `controls.md`)

- **Knobs**: a filled dark circle with a thin ring; the value is a short
  arc of accent colour around the ring from the default position (centre
  for pan, minimum for volume) and a small indicator line. No 3D shading.
- **Faders**: a thin dark slot with a small flat cap; unity (0 dB) has a
  tiny tick beside the slot.
- **Sliders** (horizontal): thin track, small rectangular handle.
- **Buttons**: flat, slightly raised rectangles with a 2–3 point radius;
  pressed / on = accent fill or accent glyph.
- **LED switches**: small circles or rounded squares; on = lit with a soft
  glow, off = dark with a faint rim.
- **Dropdowns**: dark inset field with a small down triangle at the right.
- **LCD displays**: dark inset, bright digits, tiny caption.
- **Scrollbars**: thin, dark, with zoom handles where zoom applies.

---

## 14. Names

Functional, descriptive names that describe what a thing *is* are fine and
are what users search for: **Browser, Channel rack, Step sequencer, Piano
roll, Playlist, Mixer, Insert, Master, Pattern, Clip, Track, Snap, Swing,
Graph editor, Event editor, Picker, Ghost notes, Time marker, Hint bar**.
They are ordinary music-software vocabulary.

Never use the product's name, its company's name, or its branded plugin,
tool or feature names, in the UI, docs (other than citing the manual in
Sources), filenames or code identifiers. That includes branded prefixes on
plugin names and names of its bundled instruments, effects, audio editor,
assistant or patching tool. Our own instruments and effects get our own
plain or invented names (e.g. *Antiserum*, *Drum synth*, *Zone sampler*,
*Layer*), chosen by the owner when in doubt.

Wording of hints, menus and dialogs is written by us in our own words; do
not copy the manual's or the program's phrasing.

---

## 15. Checklist for a UI change

- [ ] The change is about layout, placement, proportions, density or
      behaviour, not about matching colours, artwork, font or wording.
- [ ] Colours come from `theme.rs` roles; no new hue was picked by eye
      against a screenshot.
- [ ] Any new icon is drawn in `icons.rs` in our stroke style.
- [ ] Names are functional terms or our own names.
- [ ] Every new control writes a hint and has a right-click menu where it
      makes sense.
- [ ] The relevant spec names the manual page it was compared with.

## Sources

- FL Studio online manual (public, image-line.com), read for layout,
  control order and control meaning only, under the owner's 2026-10-21
  "layout plus general feel" decision: *FL Studio basics / Interface*
  (`basics_interface.htm`: hint panel, window snapping, window menus),
  *Toolbar panels* (`toolbar_panels.htm`: menus, hint bar and its icons,
  transport, tempo and song position displays, snap, pattern, output and
  CPU panels, shortcut icons), *Channel rack* (`channelrack.htm`: header
  controls, row order mute–pan–volume–mixer track–channel button–selector,
  steps, piano roll preview), *Piano roll* (`pianoroll.htm`: tool order and
  default keys, keyboard, ghost notes, event editor), *Playlist*
  (`playlist.htm`: tools, picker, track headers, ruler and markers, clip
  types), *Mixer* (`mixer.htm`: strip contents, ten effect slots, EQ,
  input/output, routing arrows, master).
- Observable behaviour of the program in normal use (where windows open by
  default, what lights up when).
- Own design: all colours and colour roles (`theme.rs`), icons and their
  stroke style, fonts and type sizes, knob/fader/button/meter drawings,
  exact sizes, and all wording.
- No colours were sampled, nothing was traced or overlaid, and no
  measurements were taken from screenshots.
