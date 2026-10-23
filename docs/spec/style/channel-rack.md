# Style guide: Channel rack and step sequencer

Part of the [interface style guide](README.md). Read it with the shared rules in [design language](design-language.md), [controls](controls.md) and the window header in [main window](main-window.md).

The channel rack is where a pattern gets built. Every channel in the project is one slim row: a few small controls, a name button and the step sequencer for the current pattern, all stacked into a tight list. It should feel like a dense hardware sequencer panel. Small round controls sit on the left. A long run of tall step pads on the right is the largest, brightest and most clickable thing in the window. Dark, quiet chrome surrounds both. Almost everything is done by clicking or dragging directly on a row, and the right mouse button is the "other half" of a control: erase, solo, add to the selection or "more options". The window shares the common header defined in the main-window section ("The header row, left to right"); this section lists only the rack's own tools, title and right-hand items.

## Placement and proportions

- **Kind of window.** The rack is an editor window inside the workspace (the area right of the browser and under the toolbar), like the piano roll, the mixer and the playlist. F6 and the toolbar's channel-rack button show and hide it. It has no system title bar. Its first row is the shared compact header (main-window section; our current header is described in `docs/spec/window-layout.md`).
- **Where it opens.** In the default layout, at the top-left of the workspace: flush with the browser column on its left and the toolbar above, with the mixer to its right along the top and the playlist below (main-window section, default layout). Today it opens offset from that corner (see gaps).
  - Width: just enough for the left controls plus 16 steps (`window-layout.md`), roughly a quarter to a third of a typical workspace.
  - Height: follows the number of channels (see "Height mode" below). By default it stops at about half the workspace height, and further rows scroll.
- **Moving, snapping, detaching.** As for every editor window (main-window section):
  - Drag the header's drag area, or any empty chrome, to move the window.
  - Snapping to other windows and workspace edges, with Alt held to move without snapping, follows the main-window proposal.
  - "Detach from main window", one of the common window-menu items, moves the rack into its own system window and back (main-window proposal).
- **Resizing.**
  - Every edge resizes, and a small grip in the bottom-right corner is the obvious handle.
  - Making the window taller shows more rows. Making it wider shows more of the pattern's steps; narrowing it hides steps behind the horizontal scroll bar.
  - **Proposed (needs `channel-rack.md` updated first):** dragging the bottom-right grip to the right past the pattern's last step lengthens the pattern in whole beats, and the length box follows (as in the reference). Dragging left only hides steps; it never shortens the pattern, which is the length box's job (own rule). Until the spec changes, resizing never changes the pattern.
  - **Height mode** (window menu) has three settings: manual; fit all rows, where the window grows and shrinks as channels are added or removed, up to the workspace height; and minimum. **Proposed (needs `window-layout.md` updated first).**
- **Collapsing and maximizing.** The shared header's collapse button rolls the window up to its header row and keeps its size for when it expands; maximize fills the workspace (main-window section).
- **Internal proportions** (16-step pattern, name column at default width):
  - The left control block (mute light, pan, volume, mixer-track box) is about as wide as the name button.
  - The name button is about as wide as five or six step buttons. A width setting widens it in steps, up to about four times the default.
  - The selection switch is a thin sliver about half a step wide, with a small gap on either side.
  - The steps take a little more than half of the row.
  - In short: about one fifth small controls, one fifth name, three fifths steps.
- **Rows.** The step button's height sets the row height; steps are clearly taller than they are wide (see `window-layout.md` for our chosen sizes). Knobs and the mute light are centred vertically and are clearly smaller than the row. The gaps between rows are thin, a small fraction of a row, so eight rows read as one dense block rather than a list of cards.
- **Three bands, top to bottom:**
  1. The header: one row of small tools, a little shorter than a channel row.
  2. The rows well: an inset area one tone darker than the chrome, with a narrow margin on every side.
  3. The footer strip, about as tall as the header: the add button under the name column on the left, the horizontal scroll bar under the step column on the right, and the resize grip in the corner.
- **Graph editor.** When it is open, it takes over the step column below the focused channel's step row. It hangs below the window's lower edge as an attached panel over whatever lies beneath, rather than growing the window frame. The left part of every row stays visible beside it. Its default height is about five or six rows, and it has its own resize corner (section 6).

## Anatomy

Behaviour that our specs do not have yet is marked **Proposed (needs `<spec>` updated first)**; the current behaviour stays the default until that spec changes. Layout, order and proportions are this guide's target; the spec changes they need are listed under gaps. Every new key below is window-local (active only while the pointer is over the rack and no text field has focus, as in `keymap.md` "Over the channel rack"), so none clashes with a global binding.

### 1. Header row (left to right)

Slots 1, 5 and the window buttons are the shared header (main-window section, C). The rack fills the others as follows. Filter, width grip, play button and title sit on the left. Swing, length and the two view toggles are the right-hand items (slot 6), pushed to the right end just before the window buttons. The flexible space between the clusters is the drag area.

1. **Window menu button** (shared slot 1). Opens the rack's menu: the rack's own items (section 9), a separator, then the common window items (Detach from main window, Maximize or Restore, Collapse to header, Reset position). Hover shows "Window menu" in the hint bar.
2. **Group filter** (tool group). A dark drop-down box about as wide as the name button.
   - It shows the current filter: "All", "No group" or a group name ("No group" is a proposed rename of today's label; see Labels).
   - Left-click lists All, then each group alphabetically, then No group (existing list).
   - Filtering only hides rows. Channel numbers and number-key shortcuts do not change (existing).
   - **Proposed (needs `channel-rack.md` updated first):**
     - Right-click offers "New group…", "Rename group…" and "Delete group". Deleting a group leaves its channels in place, without a group.
     - Page Down and Page Up step to the next and previous group (window-local).
     - While a group is shown, a newly added channel joins that group.
3. **Name-column width grip** (tool group). A narrow vertical grip directly after the filter. **Proposed (needs `channel-rack.md` updated first):** drag sideways to widen or narrow every name button at once; double-click returns to the default width. The same widths are offered as presets in the window menu.
4. **Window play button** (shared slot 3). A small play triangle just before the title. **Proposed (needs `transport.md` updated first):** click plays the current pattern from its start in pattern mode, and click again stops. It is lit in the accent while the pattern plays. This behaviour is our own design.
5. **Title** (shared slot 4). "Channel rack", then an em dash and the current pattern's name as the context, in small text. It is not a selector and not interactive; it belongs to the drag area.
6. **Drag area** (shared slot 5). Drag moves the window. Double-click follows the main-window rule (proposed: toggle maximize). **Proposed (needs `channel-rack.md` updated first):** Shift+scroll here steps through patterns: down for the next, up for the previous.
7. **Swing knob** (right-hand item). One knob, a little larger than the row knobs, sweeping from 0 at the left.
   - Left-drag up or down changes it, with Shift for fine steps.
   - Scroll nudges it. Double-click or middle-click resets it to 0.
   - Right-click opens the standard knob menu (automation, controller, reset; see `docs/spec/controls.md`).
   - It sets the project-wide swing, which each channel's swing mix scales (existing behaviour; today it is a percent box, see gaps).
8. **Pattern length box** (right-hand item). A small dark number display.
   - It shows the pattern's length in steps (existing).
   - Drag vertically to change it, or double-click to type a value (existing).
   - Range: 1 step up to `MAX_PATTERN_STEPS`. This limit is our own; the reference stops lower.
   - "Auto", where the length follows the end of the last bar that has notes; right-click offers Auto plus whole-bar presets (1, 2, 4, 8, 16 and 32 bars, in the pattern's time signature).
9. **Graph editor toggle** (right-hand item). An icon toggle that opens and closes the graph editor for the focused channel (existing as a text toggle). It is lit while the graph editor is open. **Proposed (needs `keymap.md` updated first):** Ctrl+K does the same (window-local; Ctrl+K is unbound today).
10. **Piano-roll view toggle** (right-hand item). **Proposed (needs `channel-rack.md` updated first):** an icon toggle. When it is on, every row shows a miniature note preview instead of step buttons, including step-only channels, whose steps appear as short ticks. It is lit while on.
11. **Collapse, maximize/restore, close** (shared slots 7–9). As in the main-window section. Close hides the window; F6 brings it back.

There is no per-channel looping and no online loop-library tool in MusicCraft. Leave those slots out entirely rather than leaving gaps.

### 2. Channel row (left to right)

1. **Mute light.** A small round light at the far left of the row.
   - Lit means the channel plays; dark means it is muted. Left-click toggles it (existing).
   - Number keys 1–0 mute channels 1–10 and Ctrl+1–0 solo them while the pointer is over the rack (existing, `keymap.md`).
   - Live playing from a MIDI keyboard or the typing keyboard still sounds while the channel is muted.
   - Right-click opens a small menu: "Solo" or "Unsolo", "Keep out of solo" and "Unmute all" (existing).
   - Ctrl+click solos the channel: it becomes the only soloed channel, and a second Ctrl+click clears solo (the same rule as `channel.solo`; solo is its own state, so mutes are left alone) (existing).
   - Alt+click applies the mute or solo to the channel's group (existing).
   - Shift+click locks the mute state so that solo actions skip this channel. A tiny padlock then replaces the light (existing).
2. **Pan knob.** A small knob whose arc grows outward from the centre. Behaviour follows `controls.md`; a reset returns it to the centre. The hint bar shows the value while hovering or dragging (existing).
3. **Volume knob.** The same size, with an arc from the left. A reset returns it to the default channel volume. The hint shows the value (existing).
4. **Mixer-track box.** A small dark number box.
   - It shows "M" for the master track, or the insert number (existing).
   - Drag up or down to change the track, or double-click to type (existing).
   - The hint names the track (existing; wording proposal under Labels).
   - **Proposed (needs `channel-rack.md` updated first):**
     - Channels that make no sound of their own (layer, MIDI out) show their kind glyph here in place of a number, and the box is inert.
     - The whole column can be hidden or shown with Alt+M (window-local) or the window menu. It is shown by default.
5. **Name button.** The widest control on the row.
   - **Content:** the channel's name, centred (an option aligns it left), cut short with an ellipsis (existing). **Proposed:** a small kind glyph at its right end (see Icons), a padlock when the channel is locked to a controller, and a warning glyph when its sample or plugin is missing.
   - **Fill:** a muted version of the channel's own colour, so every row is identifiable by colour at a glance even when nothing is selected (needs a stored channel colour; see gaps).
   - **Left-click:** opens the channel settings window and makes this the focused channel (existing). A second click closes the window if it is already showing this channel (existing).
   - **Right-click:** opens the channel menu (existing; section 9).
   - **Ctrl+press** previews the channel's sound without opening anything, and **Shift+scroll** moves the channel up or down one slot per notch, as Alt+Up/Down does for the selected channel (existing).
   - **Proposed (needs `channel-rack.md` updated first):**
     - A vertical press-and-drag past a small threshold starts a reorder. An insertion line shows where the channel will land, and dropping it is one undo step. This drag is our own design.
   - **Missing state:** if the sample file or plugin cannot be found, the whole button takes the warning fill and the hint gives the missing path (proposed; today the spec only plays silence and warns).
6. **Selection switch and activity light.** A thin, tall pill set between the name button and the steps, with a small gap on each side (layout target; today it sits at the row's end).
   - The outline shows selection: unlit when the channel is not selected, lit when it is (existing look).
   - Left-click selects this channel only, without opening its settings (existing).
   - The inside is an activity light. It flashes briefly whenever the channel starts a note, whether during playback, from live input or from a preview (existing).
   - **Proposed (needs `channel-rack.md` updated first):**
     - Right-click adds the channel to the selection, or removes it.
     - Right-drag sweeps across rows to select a run of channels.
     - Double-click selects all channels.
     - Selected channels receive live MIDI and typing-keyboard notes. They are also the targets of every item in the window menu's "Selected channels" group and of step copy and paste.
     - When the channel is locked to a controller, the pill is lit in the warning tone instead.
7. **Step buttons**, or a **miniature note preview** (section 3).
   - There is one button per sixteenth step of the current pattern (existing).
   - Buttons in each beat of the pattern's time signature alternate between two tones, so steps read in groups of four in 4/4 (existing).
   - The gaps between steps are uniform and narrow. Our steps are flat, slightly rounded pads with no gradient or bevel; a step's state shows only through its fill (own design).
   - Left-click toggles a step; right-click turns it off (existing).
   - Left-dragging paints every step it passes to the state of the first click, and right-dragging erases. A whole stroke is one undo step (existing).
   - Turning a step on previews the sound (existing).
   - Shift+Ctrl+Left/Right shifts the selected channel's steps (existing).
   - Each step plays at C5 with its own velocity, pan and other values, which are edited in the graph editor (existing).
   - **Proposed (needs `channel-rack.md` and `keymap.md` updated first):**
     - A settings switch, "Left button only turns steps on", makes left-click only turn steps on, leaving erasing to the right button. Off by default, so today's toggle stays.
     - Ctrl+C, Ctrl+X and Ctrl+V copy, cut and paste the selected channels' steps between channels and patterns (window-local; all three are unbound today).
8. **End of row.** After the steps there is only the well's margin. Nothing else sits at the row end.

### 3. Miniature note preview (instead of steps)

- It appears when a channel's notes are not all plain steps (any other key, length or off-grid start) (existing), or for every row when the piano-roll view toggle is on (proposed).
- It is one dark strip exactly as wide as the step area would be.
- Its background alternates lighter and darker by bar, so bars are visible in long patterns (today it alternates by beat; see gaps).
- Notes are drawn as short horizontal bars in the channel's colour. A note's height in the strip follows its key, scaled to the channel's lowest and highest keys. Steps-only channels show their steps as short ticks along the middle.
- It cannot be edited.
- Left-click opens the piano roll for this channel (existing).
- **Proposed (needs `channel-rack.md` updated first):**
  - A second left-click closes the piano roll if it already shows this channel.
  - Right-click opens the piano roll scrolled to the clicked point in time.
  - An option adds a small speaker button at the strip's left end that mutes the channel.
- Hovering draws a thin outline around it.

### 4. Step button states

| State | Look (relative) |
|---|---|
| Off, even beat group | The lighter of the two step tones |
| Off, odd beat group | The darker step tone |
| Off, hover | The group tone one step lighter |
| On | Filled with the channel colour. In odd beat groups, the channel colour one step darker, so grouping survives on lit steps |
| On, velocity | Brightness follows velocity, from a readable minimum at the lowest velocity to full colour at the maximum (own design) |
| On, hover | The channel colour one step lighter |
| Being painted (drag in progress) | Shows the stroke's target state immediately |
| Erased-but-kept (option "Keep data of erased steps") | An outline in the channel colour with no fill (proposed) |
| Playing step (transport running, this pattern playing) | The current step's whole column lightens faintly across all rows, and lit steps in that column flash brighter for the step's duration (own design) |
| Past the pattern's end (window wider than the pattern) | Not drawn; the well continues empty |

### 5. Folded rows

**Proposed (needs `channel-rack.md` and `keymap.md` updated first).**

- "Fold" (Alt+Z, window-local) turns each selected row into a short, thin bar in its channel colour. Folded bars collect side by side in one line at the bottom of the rows well.
- Hovering a bar puts the channel's name in the hint bar. Right-click unfolds it. "Unfold all" (Alt+U, window-local) restores every row.
- An option, "Fold empty channels when switching patterns", folds rows that have no notes in the current pattern and unfolds the rest automatically.

### 6. Graph editor

- **Open and close:** the header toggle (existing), Ctrl+K or "Edit in graph editor" in the channel menu (proposed). It always edits the focused channel. Clicking another row's selection switch retargets it.
- **Position** (layout target; today it is a strip under all rows): the focused channel's step row is drawn at the top of the step column. The other rows' step buttons are hidden while the graph is open; the graph fills the step column below the focused row, covering their step area but not their left controls. It hangs below the window's lower edge rather than growing the window frame.
- **Graph area:** a dark well with one column per step, aligned exactly with the step buttons above.
  - Thin lines separate the steps, stronger lines mark beats and the strongest lines mark bars.
  - Three faint horizontal guides mark the quarter, half and three-quarter levels.
- **Bars:** each step that has a note shows one bar in the channel colour. Steps without a note show nothing.
  - A bar's brightness follows its value: darker toward low values, full channel colour at the top (own mapping, matching velocity on steps).
  - Bars rise from the bottom for one-sided values (key, velocity, release, offset, repeat).
  - Bars grow up or down from a centre line for two-sided values (pan, fine pitch, Mod A, Mod B).
- **Property tabs:** a row of small text tabs along the bottom edge, in our order: Key, Velocity, Pan, Release, Fine pitch, Mod A, Mod B, Offset, Repeat. The selected tab is lit; the others are dim text. Velocity is selected by default (existing). Velocity, Pan, Release, Fine pitch and the two mod properties exist today; Key, Offset and Repeat are proposed. Mod A and Mod B are proposed renames of today's labels (see Labels).
- **Gestures:**
  - Left-drag sets every column it passes to the pointer height; each drag is one undo step (existing).
  - **Proposed (needs `channel-rack.md` updated first):** Ctrl+drag on one column scales all columns proportionally; right-drag from one point to another draws a straight ramp across the columns in between; Alt+click or Alt+drag resets columns to their default.
  - The hint bar shows the hovered column's step number and value.
- **Resize:** a grip at its bottom-right corner resizes it both horizontally and vertically. Its columns stay aligned with the steps, so a wider graph shows more step columns.

### 7. Footer strip

- **Add button.** A plus sign centred under the name-button column (layout target; today it is centred under the whole window).
  - Left-click opens the add menu: built-in sounds, instruments and plugin instruments (existing).
  - The new channel goes to the bottom of the rack (existing), and into the shown group when a group filter is active (proposed).
  - Hover lightens it, and the hint reads "Add a channel" (existing).
- **Horizontal scroll bar.** Under the step column only. The left controls and name buttons never scroll sideways (layout target; see gaps).
  - It is thin, with small left and right chevrons at its ends.
  - It appears only when the steps do not all fit.
- **Vertical scroll bar.** Along the right edge of the rows well, and only when the rows do not all fit.
  - Scrolling over the rows scrolls them vertically. The header and footer stay fixed.
  - **Proposed (needs `channel-rack.md` updated first):** an option, "Scroll to the selected channel on play and stop", brings the focused row into view.
- **Resize grip.** The bottom-right corner (see Resizing for the proposed pattern-length drag).

### 8. Special channel rows

- **Layer channel:** a normal row whose steps trigger its member channels. Its name button shows the layer glyph, and (proposed) the mixer-track box shows the layer glyph instead of a number.
- **MIDI-out channel:** a normal row. The name button and (proposed) the mixer-track box show the MIDI-out glyph.
- **Plugin instrument:** the name button shows the shared plugins glyph. If the plugin failed to load, the button shows the warning state.
- **Missing sample:** the warning state on the name button, with the missing path in the hint.
- **Audio clips and automation clips** live in the playlist, not in the rack (our own design). If they ever become rack channels, they go into their own groups, show their kind glyph in the mixer-track box, and show a preview of the clip instead of steps.

### 9. Menus

All menus use our wording and our grouping. Separators split them into the groups listed below. A long menu may wrap into two columns side by side instead of scrolling, which keeps it compact. Items marked ✓ exist today; everything else is **Proposed (needs `channel-rack.md` updated first)**, and new keys are window-local.

- **Window menu, rack items** (header slot 1, before the common window items):
  - **Add:** Add channel ▸ (the same list as the plus button ✓), Plugin instrument ▸ ✓.
  - **Select:** Select all, Select channels with no notes.
  - **Selected channels** (each item acts on every selected channel):
    - Clone (Alt+C ✓), Delete (Alt+Delete ✓, in the danger colour), Move up / Move down (Alt+Up/Down ✓). Today the keys act on the single selected channel.
    - Order by ▸ colour / name / mixer track / kind.
    - Put in a group… (Alt+G).
    - Recolour ▸ blend between two colours… / random.
    - Mute, Unmute, Transpose….
    - Swing mix ▸ 0% / 25% / 50% / 75% / 100% / other….
    - Shorten swung notes ▸ on / off.
    - Give each its own mixer track (Ctrl+L ✓ for one channel).
    - Fold (Alt+Z), Unfold all (Alt+U).
  - **View:**
    - Height ▸ manual / fit all rows / minimum.
    - Follow the selected channel's group.
    - Name column width ▸ 100% … 400%.
    - Name alignment ▸ centred / left.
    - Scroll to the selected channel on play and stop.
    - Keep data of erased steps.
    - Show mixer-track column (Alt+M).
    - Preview the whole pattern in note previews.
    - Mute button on note previews.
  - Then the common window items (main-window section): Detach from main window, Maximize or Restore, Collapse to header, Reset position.
- **Channel menu** (right-click a name button):
  - **Column one:**
    - Open in piano roll ✓, Edit in graph editor, Channel settings ✓.
    - Name and look… (F2, window-local; it opens our combined rename, colour and icon dialog), Pick a random colour, Colour…, Icon…. Today: Rename… ✓.
    - Change sample… (samplers only ✓), Choke itself (a new note cuts the channel's own previous note), Group… ✓.
    - Insert channel before ▸, Replace instrument ▸, Clone ✓, Delete ✓ (in the danger colour).
  - **Column two:**
    - Cut steps, Copy steps, Paste steps, Clear notes in pattern ✓.
    - Turn on every 2nd step / every 4th step / every 8th step (today ✓ under "Steps ▸" as "Fill every N steps"), Rhythm fill….
    - Shift left (Shift+Ctrl+Left ✓), Shift right (Shift+Ctrl+Right ✓).
    - Solo ✓, Move up ✓, Move down ✓.
    - Pass MIDI through, Lock to controller ▸, Write generated notes into pattern ▸.
  - In the name-and-look dialog, F2 picks a random colour and F3 reapplies the last colour used. Both keys are local to that dialog, so they do not clash with the browser's F3 / F2 result stepping or with F2 elsewhere.
- **Mute light menu:** Solo / Unsolo, Unmute all.
- **Group filter menu (right-click):** New group…, Rename group…, Delete group.
- **Pattern length menu (right-click):** Auto, 1 bar, 2 bars, 4 bars, 8 bars, 16 bars, 32 bars.
- **Knob menu (swing, pan, volume):** the shared control menu (Create automation clip, Add controller ▸, Reset; `controls.md`).

### 10. Drag and drop

- A sound dragged from the browser and released over the rack becomes a new channel. While it is over the rack, the window shows an accent outline (existing).
- **Proposed (needs `channel-rack.md` updated first):** dropped between two rows, it is inserted there, with an insertion line marking the spot. Dropped elsewhere, it goes at the bottom (as today). Dropped onto a sampler's name button, it replaces that channel's sample.

## Icons

| Where | What it does | Our glyph |
|---|---|---|
| Header, far left (shared slot 1) | Opens the window menu | Three short stacked horizontal bars (shared with every window; main-window section) |
| Header, after the filter | Name-column width grip | Two short vertical lines side by side |
| Header, before the title | Plays or stops the current pattern | `Icon::Play` (a filled triangle pointing right), drawn small |
| Header, right-hand items | Graph editor toggle | A baseline with four thin vertical stems of different heights, each topped by a small filled dot |
| Header, right-hand items | Piano-roll view toggle | Our existing `Icon::PianoRoll` (a key strip at the left with three note bars) |
| Header, far right | Collapse, maximize/restore, close | The shared window buttons (main-window section); close is our existing cross from `chrome.rs` |
| Group filter box, right end | Opens the list | A small downward chevron |
| Mute light (shift-locked) | Locked mute state | A padlock: a rounded rectangle body under an arch |
| Name button, right end: sampler | Kind glyph | A short zig-zag waveform, three peaks |
| Name button: synth and other built-in instruments | Kind glyph | One period of a sine wave |
| Name button: drum synth or kit | Kind glyph | A circle with a short slanted stick crossing its upper right |
| Name button: plugin instrument | Kind glyph | The shared plugins glyph, drawn small: the toolbar's proposed `Icon::Plugins`, a 2 × 2 grid of rounded squares, three filled and one outlined |
| Name button and mixer-track box: layer | Kind glyph | Two rounded rectangles offset diagonally, the front one filled |
| Name button and mixer-track box: MIDI out | Kind glyph | A small open box with an arrow leaving it to the right |
| Name button: locked to a controller | Lock indicator | The padlock |
| Name button: missing sample or plugin | Warning | An exclamation mark inside an outlined triangle |
| Note preview, left end (option) | Mutes the channel | `Icon::Mute` (speaker) without the slash, with the slash when muted |
| Footer, under the name column | Add a channel | A plus: two crossing strokes of equal length |
| Horizontal scroll bar ends | Scroll the steps | A left chevron and a right chevron |
| Bottom-right corners (window and graph editor) | Resize | Three short parallel diagonal strokes, shortest nearest the corner |
| Toolbar | Show or hide the rack | Our existing `Icon::ChannelRack` (a 4×3 grid of small cells, some filled) |

All glyphs are vector shapes drawn in `crates/musiccraft-ui/src/icons.rs` style: stroke-based, about the weight of the existing pencil and eraser icons, one colour per state.

## Colour roles

| Element | Token(s) | Relative tone and state changes |
|---|---|---|
| Window chrome (header, footer strip, margins) | `theme::PANEL` | Base tone of the window |
| Rows well (inset behind the rows) | `theme::BACKGROUND` | One step darker than the chrome, so the rows read as sitting in a recess |
| Window outline | Proposed `theme::WINDOW_EDGE` (main-window section: one step lighter than `RAISED`) | Focused window: `theme::ACCENT_DIM` |
| Title text | `theme::TEXT` | Dims to `theme::TEXT_DIM` when another window has focus (main-window rule) |
| Pattern name in the title | Pattern colour (`pattern_color`), falling back to `theme::HIGHLIGHT` | Secondary to the window name |
| Header icon buttons, idle | Glyph in `theme::TEXT_DIM`, no fill | Hover: fill `theme::RAISED_HOVER`, glyph `theme::TEXT` |
| Header toggles and play button, on | Glyph in `theme::ACCENT` plus an `ACCENT` underline (`icons::toggle` style) | Pressed: fill `theme::ACCENT_DIM` |
| Group filter, length box and mixer-track box | Fill `theme::PANEL_DARK`, value in `theme::TEXT` | Hover: outline `theme::RAISED_HOVER`. Focused for typing: outline `theme::ACCENT`. "auto" and "M" in `theme::TEXT_DIM` |
| Knobs (swing, pan, volume) | Body `theme::RAISED`, arc `theme::ACCENT`, pointer `theme::TEXT` | Add a thin ring in `theme::GRID_BAR` so a centred pan knob stays visible on the well. Hover: body `theme::RAISED_HOVER` |
| Mute light | On: `theme::ACCENT`. Off (muted): `theme::PANEL_DARK` with a `theme::TEXT_DIM` ring | Soloed: lit, with an outer ring in `theme::HIGHLIGHT`. Silenced by another channel's solo: `theme::ACCENT_DIM`. Locked: padlock in `theme::TEXT_DIM` (same roles as the mixer's lights) |
| Channel colour | Proposed per-channel colour, chosen from the `theme::track_color` palette (default `theme::palette(n)` at creation, then stored) | Stays with the channel when rows are reordered |
| Name button, normal | Proposed `theme::CHANNEL_BUTTON`: the channel colour blended about one third into `theme::RAISED`; text `theme::TEXT` | Hover: one step lighter |
| Name button, focused channel | Channel colour at full strength, text `theme::BACKGROUND` | Hover: unchanged |
| Name button, missing sample or plugin | `theme::DANGER` blended toward `theme::RAISED`, text `theme::TEXT` | Overrides the channel colour |
| Kind glyph on the name button | `theme::TEXT_DIM` | `theme::BACKGROUND` on the focused (full-colour) button |
| Selection switch | Unselected: outline `theme::RAISED_HOVER`. Selected: outline and fill `theme::ACCENT` | Activity flash: inner fill jumps to `theme::TEXT` and fades back. Locked to controller: `theme::HIGHLIGHT` |
| Step, off | `theme::STEP_A` / `theme::STEP_B`, alternating by beat | Hover: proposed `theme::STEP_HOVER`, the group's tone one step lighter (never darker) |
| Step, on | Channel colour. Odd beat groups use proposed `theme::step_lit(colour, odd)`, the channel colour one step darker | Velocity scales brightness down toward the off tone, keeping a readable minimum |
| Step, erased but kept | Channel colour as a thin outline over the off tone | — |
| Playing step column | Proposed `theme::STEP_PLAYING`: `theme::PLAYHEAD` at low strength, laid over the column | Lit steps in that column: the channel colour one step lighter |
| Note preview strip | Well `theme::PANEL_DARK`; alternate bars `theme::BLACK_KEY_ROW`; notes in the channel colour | Hover outline `theme::ACCENT` |
| Folded row bar | Channel colour, dimmed | Hover: full channel colour |
| Graph editor well | `theme::PANEL_DARK` | — |
| Graph editor lines | Steps `theme::GRID_LINE`, beats `theme::GRID_BEAT`, bars and centre line `theme::GRID_BAR` | — |
| Graph editor bars | Channel colour, darkened toward low values (the same blend as `step_lit` velocity scaling) | Bar under the pointer one step lighter; bars being dragged at full brightness |
| Graph editor tabs | Idle `theme::TEXT_DIM` on `theme::PANEL` | Selected: `theme::TEXT` on `theme::RAISED` with an `ACCENT` underline |
| Add button | Glyph `theme::TEXT_DIM` | Hover: fill `theme::RAISED_HOVER`, glyph `theme::TEXT` |
| Scroll bars | Track `theme::PANEL_DARK`, thumb `theme::RAISED` | Hover `theme::RAISED_HOVER`; dragging `theme::ACCENT_DIM` |
| Browser drop outline, reorder insertion line | `theme::ACCENT` | — |
| "Delete" menu items | `theme::DANGER` text | — |
| Empty-rack message | `theme::TEXT_DIM` | — |

## Labels and wording

- **Window title:** "Channel rack" (existing), then " — " and the current pattern's name.
- **Group filter values:** "All" (existing), then the group names exactly as the user typed them, then the value for channels without a group. **Proposed rename:** today's "Unsorted" becomes "No group".
- **Graph tabs:** "Key", "Velocity", "Pan", "Release", "Fine pitch", "Mod A", "Mod B", "Offset", "Repeat". **Proposed renames:** today's "Mod X" and "Mod Y" (`LaneMode` in `crates/musiccraft-ui/src/piano_roll.rs`) become "Mod A" and "Mod B", the same names as in the piano-roll section.
- **Channel menu:** **proposed rename** of today's "Steps ▸ Fill every N steps" to "Turn on every 2nd / 4th / 8th step", and of "Rename…" to "Name and look…" once the combined dialog exists. Other existing labels stay: "Group…", "Channel settings", "Open in piano roll", "Change sample…", "Solo", "Shift left", "Shift right", "Clone", "Move up", "Move down", "Clear notes in pattern", "Delete".
- **Header controls:** no text labels; each is a knob, number box or icon (today "swing", "steps" and "Graph" are text; see gaps). Their names appear in the hint bar. Proposed hints:
  - "Swing 35%: delays every second step. Each channel's swing mix scales it" (today: "Project swing; each channel's swing mix scales it")
  - "Pattern length: 16 steps. Drag to change, right-click for bars"
  - "Graph editor (Ctrl+K)"
  - "Show every channel as notes"
  - "Window menu" (shared)
  - "Name width: drag to widen the name buttons"
  - "Play this pattern"
- **Row hints** (proposed wording; today's text in brackets):
  - Mute light: "Mute (click), solo (Ctrl+click)" [Mute]
  - "Pan: centred", "Pan: 40% left", "Pan: 25% right" [Pan +0%]
  - "Volume 80% (−1.9 dB)" [Volume 80%]
  - "Plays through mixer track 3 (Bass). Drag up or down to change". For the master track: "Plays through the master track" [Plays through mixer track: 3 (Bass). Drag up or down to change]
  - Name button: the instrument description as today, for example "Sampler: kick.wav" or "Plugin: Name (id)", then " · click: settings, right-click: menu"
  - Selection switch: "Select this channel (right-click: add to selection)" [Select this channel]
  - Note preview: "Notes: click to open the piano roll" [Melodic pattern: click to open the piano roll]
  - Step: "Step 5 · click to turn on or off, right-click to turn off, drag to paint"
  - Missing sample: "Sample not found: <path>"
- **Hint style:** sentence case. Start with the control's name and its current value, then a short phrase saying how to change it. Separate the parts with a colon, a full stop or a middle dot. No trailing full stop. Hints go to the hint bar, or into a tooltip when the hint bar is hidden (existing `Hint` trait).
- **Number formats:**
  - Swing and channel volume: whole percent ("35%"). Volume hints add dB with one decimal.
  - Pan: percent with "left" or "right", or "centred".
  - Mixer-track box: "M" for the master track, otherwise the insert number with no prefix (existing).
  - Pattern length: whole steps ("16"), or "auto".
  - Graph values:
    - Key: note name ("C5").
    - Velocity and release: whole percent.
    - Fine pitch: signed cents ("+25 ct").
    - Pan: as above.
    - Mod A and Mod B: signed percent.
    - Offset: percent of a step.
    - Repeat: "×2", "×3", ….
- **Empty rack:** "No channels yet. Add a sound from the browser or with + below" (proposed light edit of today's "No channels yet. Add a sound from the browser or below.").

## States and feedback

- **Hover:** every interactive element lightens one step or gains an outline. Off steps go one step lighter than their group tone, never darker. Knobs lighten their body. Name buttons lighten their fill. The note preview gains an accent outline. The hint bar always updates on hover.
- **Pressed:** buttons darken briefly (accent-dim fill for header toggles). Knobs show no extra press state; the moving arc is the feedback.
- **On or active:** steps fill with the channel colour. Header toggles light in the accent with an underline. The mute light is lit when the channel plays.
- **Selected:** the selection pill is filled with the accent. With several channels selected (proposed multi-select), each selected row's pill is lit.
- **Focused channel:** the last channel clicked; it opens in the piano roll and graph editor. Its name button switches to full channel colour with dark text. Exactly one row is focused at a time.
- **Muted:** the light goes dark. The row stays fully drawn. Do not dim the steps, so the pattern stays readable.
- **Soloed:** the soloed channel's light stays lit and gains a highlight ring. Lights of channels silenced by the solo show the accent at reduced strength, so they differ from muted (dark) lights. This needs solo kept apart from mute (see gaps).
- **Locked mute:** a padlock replaces the light.
- **Locked to controller:** a padlock on the name button and the selection pill in the warning tone.
- **Disabled:** controls that do not apply (the mixer-track box of a layer or MIDI-out channel, "Move up" on the first row) are drawn in the dim text tone and ignore clicks.
- **Missing:** the warning fill on the name button.
- **Playing:**
  - The current step's column lightens faintly across all rows, and lit steps in it flash brighter.
  - Each channel's activity light flashes as its notes start.
  - The window play button in the header is lit while the pattern plays.
- **Recording:** while step-edit or recording is armed, the focused row's selection pill pulses slowly between accent and accent-dim (own design).
- **Dragging:**
  - Painting steps shows the target state on each step as it is passed.
  - Knob drags move the arc live.
  - Reordering shows an accent insertion line between rows, and the dragged row's name button is drawn at reduced strength while it travels.
  - Browser drops outline the whole window in the accent (existing).
- **Keyboard focus:** number boxes being typed into show an accent outline. Rack shortcuts (number keys, Alt+C and the others) work only while the pointer is over the rack and no text field has focus (existing).
- **Folded:** rows reduced to thin colour bars in the bottom line of the well.

## MusicCraft today: gaps

Ordered by visual impact. Where a gap changes layout that an existing spec describes, that spec changes first.

   - Files: `crates/musiccraft-ui/src/channel_rack.rs` (`row`, `steps`, `mini_roll`), `crates/musiccraft-project/src/model.rs` (`Channel` has no colour field), `crates/musiccraft-commands` (no `channel.set_color`).
1. **Header layout and controls differ.**
   - Today the header is: collapse arrow, title, filter, pattern name, the word "swing" with a percent drag box, the word "steps" with a number box, a "Graph" text toggle, then close. All tools are packed left after the title.
   - Our headers put the collapse arrow at the left today. The shared header puts the window menu there and collapse, maximize and close at the right; `docs/spec/window-layout.md` ("Window header" and "Channel rack": "Header, left to right") changes first.
   - The guide wants: window menu, filter, width grip, play triangle and title (with the pattern name) on the left; swing knob, length display, graph toggle and piano-roll view toggle right-aligned before the window buttons.
   - The window menu exists (common items only) and the window buttons are at the right, but there is no piano-roll view toggle, and the title still comes after the tools rather than among them.
   - Files: `crates/musiccraft-ui/src/channel_rack.rs` (`header`), `crates/musiccraft-ui/src/chrome.rs` (`header` puts the fold arrow first), `docs/spec/window-layout.md`.
2. **Selection switch position and model.**
   - Today the switch sits at the end of the row after the steps. The guide puts it between the name button and the steps; `docs/spec/window-layout.md` ("Channel rack") and `docs/spec/channel-rack.md` ("Row layout") change first.
   - Selection is a single index (`UiState::selected_channel` in `crates/musiccraft-ui/src/lib.rs`). There is no multi-select (right-click, right-drag, double-click) and no locked state. The activity flash exists.
   - Files: `crates/musiccraft-ui/src/channel_rack.rs` (`row`).
3. **Graph editor placement and features.**
   - Today it is a separate short strip below all rows, with the channel name and tabs above the bars. `docs/spec/channel-rack.md` ("Step graph editor": "a strip under the rows") changes first.
   - The guide puts it in the step column under the focused channel's steps, hiding the other rows' steps, hanging below the window, with tabs along the bottom, value-following bar brightness and its own two-way resize corner.
   - Missing tabs: Key, Offset, Repeat. Missing gestures: Ctrl-scale, right-drag ramp, Alt reset.
   - Files: `crates/musiccraft-ui/src/channel_rack.rs` (`graph_editor`), `crates/musiccraft-ui/src/piano_roll.rs` (`LaneMode::ALL`).
4. **Scrolling, footer and placement.**
   - The rack has the simplified bars (rail, thumb and chevrons, shown only when the rows overflow; `scrollbars::plain_area`), but they scroll whole rows sideways, so mute, knobs and names scroll away with the steps. Only the step column should scroll horizontally.
   - The height is hard-capped (`max_height(420.0)`) instead of following a height mode.
   - The plus button is centred under the whole window rather than the name column (`window-layout.md` "Channel rack" says "centred under the rows" and changes first).
   - There is no corner grip, so there is no way to widen the rack to reveal or extend steps.
   - File: `crates/musiccraft-ui/src/channel_rack.rs` (`show`, `add_menu`).
5. **Name button content.**
   - No kind glyph, no padlock, no missing-file warning state.
   - Drag reorder is missing.
   - File: `crates/musiccraft-ui/src/channel_rack.rs` (`row`).
   - Files: `docs/spec/channel-rack.md` (`channel.solo`), `crates/musiccraft-project/src/model.rs`.
6. **Note preview.**
    - Left-click toggles the piano roll for that channel, but right-click "open at this point" is missing.
    - There is no all-rows piano-roll view.
    - File: `crates/musiccraft-ui/src/channel_rack.rs` (`mini_roll`).
7. **Channel context menu.**
    - It has the two columns in the guide's order, without the duplicate clear item.
    - Colour, cut, copy and paste steps and the graph editor entry are in. Missing: the icon item, insert and replace, choke itself, MIDI items.
    - File: `crates/musiccraft-ui/src/channel_rack.rs` (`context_menu`).
8. **Group filter.**
    - Right-click new, rename and remove are in, and Page Up/Down cycle the groups. The filter is still an egui drop-down rather than our styled field.
    - File: `crates/musiccraft-ui/src/channel_rack.rs` (`header`, `add_menu`).
9. **No window menu items for selected channels:** no folding of rows, ordering, recolouring, transposing, swing mix or one-track-each for a multi-channel selection.
    - File: `crates/musiccraft-ui/src/channel_rack.rs`.
    - File: `crates/musiccraft-ui/src/channel_rack.rs` (`show`).
10. **Keymap.** Ctrl+K, Alt+M, Page Up/Down, Ctrl+C/X/V and F2 are bound over the rack (`keymap.md`, "Over the channel rack"); F2 opens the rename prompt, not yet the combined name-and-look dialog. Alt+M hides the mixer-track column. Alt+G, Alt+Z and Alt+U wait for the features they drive (hiding the mixer-track column, groups dialog, folding).
11. **Naming.** `crates/musiccraft-ui/src/rack_view.rs` is the mixer's effect-rack body, not part of the channel rack. Nothing in it needs to change for this guide.

## Sources

- **Manual, `channelrack.htm`:**
  - Sections compared: "Channel Rack & Step Sequencer" / "Overview" (row order and contents), "1. Channel Options Menu" (menu groups, fold and unfold, height modes, name width and alignment presets, mixer-track column toggle, keeping data of removed steps, detach), "Channel Filter Groups (Display Filter)", "3. Global Swing", "4. Pattern Length" (auto, bar presets, the reference's lower step limit, dragging the corner to change the length), "6. Graph Editor & Step/Piano roll View" with "Graph Editor", "Target Parameters:" and "Step vs Piano roll Preview".
  - Also compared: "7 & 8. Pan & Volume", "9. Channel Button" (including the rename-and-colour keys for a random and the last colour), "10. Channel Selector", "11. Piano roll Preview", "12. Mute Control", "13. Stepsequencer", "14. Mixer Track Routing" (icons instead of a number for generators that make no audio), "15. Add Plugin Channels", "Red Channel buttons", "Lock Controller Notes to Instrument Channels".
  - Checked and deliberately not carried over: "2. Loop Starter" (online service), "5. Looping Controls" (per-channel looping).
- **Manual screenshots** (viewed for layout, order, proportions, relative sizes and density only; nothing sampled, traced or measured): `img_shot/channelrack_main.png` (header and row order, the larger swing knob, the corner drag that changes the length), `img_shot/channelrack_grapheditor.png` (graph position, hidden step rows, the graph hanging below the window, its two-way resize corner, tab placement, bar tone changing with value), `img_shot/channelrack_pianomode.png`, `img_shot/channelrack_loopcontrols.png`, `img_shot/channelrack_lockednotes.png`, `img_shot/channelrack_swing.png`, `img_shot/menu_zipped.gif`, `img_shot/basics_interface.png`.
- **Manual, other pages:**
  - `envsettings_general.htm`: settings for automatic folding of empty channels, using both mouse buttons in the step sequencer, and selecting the first channel with notes.
  - `basics_interface.htm`: window menu at the top left, windows snapping together, detaching, the hint panel.
  - `stepsequencer_advancedfilltool.htm`: the fill tool is opened from the channel menu.
  - `automation_internal.htm` and `playlist_audioclip.htm`: checked; they describe no rack-row layout.
- **Other guide sections followed for consistency:** main-window (shared header, window menu glyph, common window items, `WINDOW_EDGE`, default layout, snapping and detaching proposals), toolbar (`Icon::Plugins`, F2 for pattern rename), mixer (solo and silenced-by-solo light roles), piano roll ("Mod A" / "Mod B"), browser (window-local F3 / F2).
- **Repository:** `crates/musiccraft-ui/src/channel_rack.rs`, `crates/musiccraft-ui/src/rack_view.rs`, `crates/musiccraft-ui/src/chrome.rs`, `crates/musiccraft-ui/src/icons.rs`, `crates/musiccraft-ui/src/piano_roll.rs`, `crates/musiccraft-ui/src/lib.rs`, `crates/musiccraft-ui/keymap/default.keymap`, `crates/musiccraft-editors/src/theme.rs`, `crates/musiccraft-editors/src/widgets.rs`, `crates/musiccraft-editors/src/hint.rs`, `crates/musiccraft-project/src/model.rs`, `docs/spec/channel-rack.md`, `docs/spec/window-layout.md`, `docs/spec/controls.md`, `docs/spec/keymap.md`, `docs/parity.md`, `docs/screenshots/channel-rack.png`.
- **Own design:**
  - All colours (theme tokens and the proposed `CHANNEL_BUTTON`, `STEP_HOVER`, `STEP_PLAYING` and `step_lit`), the value-following graph bar tone mapping, flat step pads, every glyph, and all wording and labels, including "No group", "Mod A" / "Mod B", "Key", "Offset" and "Repeat" for the graph tabs and their order, "Fold" for collapsed rows, every menu item name and the menus' grouping, "Rhythm fill…", "Choke itself", "Name and look…" and "M" for the master track.
  - The kind glyph in the mixer-track box (the reference shows icons there; the glyphs are ours).
  - The play-pattern behaviour of the window play button.
  - Playing-step, velocity, recording and activity-flash feedback.
  - Drag-to-reorder with an insertion line, browser drops between rows, the rule that the corner drag never shortens a pattern, the `MAX_PATTERN_STEPS` limit, and the mixer-track column shown by default.
  - Audio and automation clips staying in the playlist.
  - The exact sizes, which stay those in `docs/spec/window-layout.md`.
