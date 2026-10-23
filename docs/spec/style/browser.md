# Style guide: Browser

Part of the [interface style guide](README.md). Read it with the shared rules in [design language](design-language.md), [controls](controls.md) and the window header in [main window](main-window.md).

The browser is the narrow, full-height column at the side of the main window. It holds everything that can be added to a project: the open project's own contents, the instruments and effects we know about, presets, the built-in sound packs and the user's sample folders. It should feel like a dense file tree: many short one-line rows in a dark inset well, with small tinted type glyphs and faint tree lines. Thin chrome bars frame the well, the header and tabs at the top and the search, tags and preview at the bottom. Anything in it can be previewed with one click and dragged straight onto the channel rack, playlist or mixer. The column is a supply shelf and never draws attention away from the workspace.

There is no browser spec yet. Everything below that goes beyond today's behaviour is **Proposed (needs a new `docs/spec/browser.md` first)** unless it is marked as changing another spec, in which case the tag names that spec. Current behaviour stays the default until the spec lands. The shared window header (menu button, title, drag area, window buttons) is defined once in the main-window section; this section lists only the browser's own tools and items.

## Placement and proportions

- **Where.** Docked to the **left edge** of the main window by default. It runs at full height from just under the toolbar to the bottom edge. The workspace fills everything to its right, and every editor window (channel rack, mixer, playlist, piano roll) lives in that workspace (see the main-window section's default layout). Nothing docks underneath the browser.
- **Width.** A narrow column, roughly a ninth to a tenth of a wide main window. It opens at the width set in `docs/spec/window-layout.md` ("Density"). On a laptop-height screen at Normal zoom the list shows about two dozen rows or more.
- **Vertical stack, top to bottom:**
  1. **Header row**: one row of small tools, the same height as every other window header (main-window section, "The header row").
  2. **Tab strip**: one row, a little taller than the header.
  3. **List well**: takes all remaining height and scrolls vertically.
  4. **Preview strip** (optional): appears only while an audio item is selected and the preview option is on. It spans the full column width and is wide and short, about a quarter as tall as the column is wide.
  5. **Tags panel** (optional): appears only while the Tags toggle is on, as a filter row and a row of tag chips that wraps.
  6. **Footer row**: the search bar, the same height as the header. An option moves it to just under the tab strip.
- **Scroll bar.** Thin, along the column's **outer edge** (the edge away from the workspace), so it never crowds the side items are dragged out of. When the browser is docked right, the bar moves to the right edge so it stays on the outer side.
- **Divider.** A hairline on the workspace side that resizes the column.
  - Drag to resize. The minimum width keeps the tab glyphs and the footer usable; the maximum is about half the window. The workspace and any maximized window follow.
  - **Proposed (needs `window-layout.md` updated first):** right-click the divider to swap between two remembered widths, the normal narrow one and a wider one set by dragging, for a quick "glance wide, go back narrow".
  - Each tab may remember its own width (tab menu, Remember width).
- **Docking.** **Proposed (needs `window-layout.md` updated first)**, as in the main-window section:
  - drag the header's title to the left or right edge of the main window to dock there; the edge drop zone lights up while dragging;
  - drop it anywhere else to float it as its own window;
  - a floating browser can be set to hide while something is dragged out of it, so the drop target is uncovered, and to come back after the drop.
- **Auto-hide (option, proposed).** The column shrinks to a thin strip at its docked edge. When the pointer reaches that edge it slides out over the workspace; when the pointer leaves it slides back. The workspace does not move or resize.
- **Show / hide.** Alt+F8 (`view.toggle browser`, already in `docs/spec/keymap.md`) or the toolbar's browser toggle. Hiding gives its width back to the workspace.
- **Collapse to header** (header window button): when docked, the column shrinks to its header row at the top and the workspace beside it does not jump; when floating, the window rolls up to its header. The same button, or double-clicking the title, expands it again.

## Anatomy

### 1. Header row (left to right)

The shared header of the main-window section, filled with the browser's own tools:

| # | Element | What it shows | Click | Right-click / other | States |
|---|---|---|---|---|---|
| 1 | **Window menu button** | The shared menu glyph at the far-left corner | Opens the browser options (§8) under the button, then the common window items | Same as click | Hover lighter; pressed look while its menu is open |
| 2 | **Fold all** | Glyph button | Closes every open folder in the current tab and scrolls to the top (Ctrl+Up over the browser) | — | Hover |
| 3 | **Refresh** | Glyph button | Rereads every folder from disk (new or deleted files show up) and rebuilds the cached listings. Plugins are not rescanned; that is in the menu | — | Hover; the glyph turns once while reading |
| 4 | **Title** "Browser" | Small header text, like the other windows' titles | Proposed: drag to dock or float (see Placement) | Double-click: collapse to header / expand | Brighter on hover to show it can be dragged |
| 5 | **Drag area** | Empty space, stretches | As the shared header | — | — |
| 6 | **Collapse** | The shared collapse window button | Collapses the column to its header, or expands it | — | Hover; glyph flips while collapsed |
| 7 | **Close** | The shared close cross | Hides the browser (same as Alt+F8) | — | Hover in the `DANGER` tone, as elsewhere |

There is no maximize button. The two tree tools come before the title, as the main-window section's browser column (D) lists them.

### 2. Tab strip

- One row of **equal-width tabs** filling the column width. Default tabs, in order:
  1. **All**: the full tree (§3).
  2. **Project**: only the "This project" branch.
  3. **Plugins**: only the instruments-and-effects branch.
  4. **Presets**: only the Presets branch.
  5. **Favourites**: a flat list of every starred item (§5).
- **Label mode** (options menu → Tab labels):
  - **Auto**: glyph only while the column is narrow, words once it is wide enough for every word.
  - **Glyphs** or **Words**: forces one or the other.
- **Active tab**: a thin rounded outline frame, with its glyph or word in the accent tone. Inactive tabs are dim and brighten on hover. The hint bar names the tab.
- **Click** switches tabs. Each tab keeps its own open and closed folders, scroll position, search text and, optionally, width.
- **Mouse wheel over the strip** steps through the tabs.
- **Drag a tab sideways** to reorder.
- **Right-click a tab** opens the tab menu:
  - **Rename…**
  - **Glyph and tint…**: pick a glyph from our set and a tint from `theme::PALETTE`.
  - Separator.
  - **Hide this tab**
  - **Show hidden ›**: the hidden tabs, to bring back.
  - **Duplicate tab**
  - **Move left / Move right**
  - **Delete tab**: user tabs only. The five defaults can be hidden but not deleted.
  - **Save search as tab**: a new tab showing the current search results.
  - Separator, then **View ›**:
    - **Zoom ›**: Compact, Normal, Large. Changes the row height and text size of this tab's list only. Three levels is our own reduction; more can be added later if they prove useful.
    - **One folder at a time**: opening a folder closes its siblings.
    - **Show folder glyphs**
    - **Show inline waveforms**: a thin waveform drawn behind each audio file's name.
    - **Show file extensions**
    - **Show other file types**: unknown files are listed dimmed and cannot be dragged.
    - **Sort by ›**: Name, Type, Date modified.
    - **Remember width**: this tab restores its own column width.
    - **Lock view**: the tab reopens with the folders and scroll position it had when locked. A tick marks a locked tab in this menu.
  - There is one row layout (the single-line list in §4). A second "detail" layout, with a dim line of information under each name, is an own-design option for later, not part of this guide.
- A user tab can also be made by right-clicking a folder → **Open in new tab**. That tab shows only that folder's contents.

### 3. The tree (list well), All tab

Top-level categories in this fixed order, followed by the user's folders. Names in quotes are proposed labels (see Labels and wording).

1. **"This project"**: the open project's own contents. Its children, in order:
   - **Undo history (n)**: the undo list, oldest at the top and newest at the bottom.
     - The current step has the selection band. Steps after it, which can be redone, are dimmed.
     - Click a step to jump there: one action that runs undo or redo as many times as needed.
   - **Patterns (n)**: each pattern, as a small swatch in its colour plus its name.
     - Click selects the pattern (existing `pattern.select`). Double-click opens it in the channel rack.
     - Right-click: Rename, Clone, Colour, Delete (existing pattern commands).
   - **Effects (n)**: every effect in the mixer, as "‹track› · ‹effect›".
     - Click opens its effect window.
     - Expanding it lists its automatable parameters (later). Right-click a parameter to create an automation clip or link a controller, the same actions as right-clicking that knob.
   - **Channels (n)**: every channel's instrument, with the same click, expand and right-click behaviour as Effects.
   - **Controllers (n)**: the existing controller list (`controllers.rs`, `docs/spec/controllers.md`), moved here from the top level. Each row keeps its on/off box, name, settings and delete button.
   - **Automation (n)**: automation clips. Click opens the clip's editor.
   - **Samples (n)**: every sample file the project uses, as sound rows (§4). Missing files are dimmed, and their hint names the stored path (translated as in `docs/spec/sample-paths.md` where that finds the file).
2. **Recent**: recently opened projects and recently used sound files, newest first, capped at a short list.
3. **"Instruments & effects"**: everything that can become a channel or an effect. Children in alphabetical order:
   - **Effects**: built-in effects grouped into our own sub-categories (for example Delay & reverb, Distortion, Dynamics, Filter & EQ, Modulation, Utility). Installed effects the user has favourited go into the same sub-categories.
   - **Generators**: built-in generators grouped into our own sub-categories (for example Synths, Drums, Keys, Samplers, Utility), with favourited installed instruments added the same way.
   - **Installed**: everything the plugin host found, split into **Effects** and **Generators**. Each row shows the name, with the format (CLAP or VST3) as a small dim tag at the right end. Vendor and id go in the hint. A plugin found in both formats appears as two rows that differ only by that tag.
   - When plugin hosting is unavailable, Installed shows one dim row that says so.
4. **Presets**: one folder per instrument that has presets (today: Antiserum). Each holds its factory categories, then a **User** folder. **Proposed (needs `antiserum-app.md` updated first):** today the spec places these under the Antiserum generator entry.
5. **"Sound packs"**: the built-in sound sets (today: Drums).
6. **Autosaves**: the autosave and any recovered projects (optional, own design).
7. **User folders**: each folder added in Settings → Folders (`docs/spec/settings.md`), at top level, sorted by name.
   - They use the ordinary folder glyph with a different tint (see Colour roles), so user folders stand out from the built-in categories.
   - Dragging a folder from the system file manager onto the browser adds it.

Inside any folder, subfolders come first, then files, each sorted by name ignoring case. Hidden (dot) files are not listed. Only files MusicCraft can load are listed: WAV sounds, `.mcproj` projects and our preset files. Other audio formats are added only once we can decode them.

### 4. Item rows

Every entry is a **single line**, the full width of the well. Left to right:

1. **Focus marker column**: a narrow strip at the far left. A short accent bar appears there on the selected or keyboard-focused row.
2. **Indent**: one step per depth. A child's glyph starts under its parent's name, so the names form a staircase.
3. **Tree lines**: faint, thin connectors. A vertical line runs down from each open parent's glyph, with a short horizontal stub into each child. Hidden at Compact zoom if they clutter.
4. **Type glyph**: a small square glyph, tinted by type (see Icons, Colour roles).
5. **Name**: the list text, one size larger than the header title. Long names are cut short with an ellipsis; hovering a cut-off name pops the full name out over the row, past the column edge if needed. Names of open folders are drawn slightly stronger.
6. **Right-end items**, left to right:
   - the dim format tag (installed plugins only);
   - a small down chevron on open folders;
   - the **favourite star**: an outline that appears on hover, a filled star when starred, always visible while starred.

Behaviour by type:

- **Folder and category rows**:
  - Click toggles open/closed.
  - Double-click opens it and all its subfolders.
  - Right-click opens the folder menu:
    - Open all below / Close all below.
    - Open in new tab, and Add to tab › (the user tabs).
    - Tint › (from `theme::PALETTE`).
    - Show in file manager.
    - Reread this folder.
    - **Remove from browser** (top-level user folders only).
  - Hovering shows the full path in the hint bar.
- **Sound (audio file) rows**:
  - **Click** selects it and **previews** it on the master output (existing). A new click replaces the running preview.
  - **Alt+click** previews the whole file. **Shift+click** in the preview strip previews from the point clicked.
  - **Double-click** adds a new sampler channel with it (existing).
  - **Middle-click** or **Enter** sends it to the selected channel, replacing that channel's sample (`channel.set_sample`).
  - **Drag**: see §7.
  - **Right-click** opens the sound menu (the three "Add as" items exist today):
    - Preview whole file.
    - Send to selected channel.
    - Add as sampler.
    - Add as granular instrument.
    - Add as resynth (play its harmonics).
    - Open in sample editor.
    - Separator.
    - Add to favourites / Remove from favourites.
    - Tags › (tick any existing tag).
    - Add tag… (comma-separated names).
    - Edit tags…
    - Separator.
    - Show in file manager.
    - Copy path.
    - Delete file… (asks first; an open question whether to offer it at all).
- **Plugin (generator or effect) rows**:
  - **Click** selects only (existing rule in `docs/spec/channel-rack.md`).
  - **Double-click** adds it: a generator as a new channel, an effect to the selected mixer track (existing).
  - **Drag**: see §7.
  - **Right-click**:
    - Add as channel, or Add to selected mixer track.
    - Add to library › (category), for installed plugins: files it under a sub-category of Effects or Generators.
    - Remove from library: removes that entry only, never the plugin.
    - Add to favourites.
    - Plugin info (name, vendor, id, format, file).
- **Preset rows**:
  - Click selects.
  - **Double-click** adds a new channel playing the preset. **Proposed (needs `antiserum-app.md` updated first):** that spec says a single click adds it, while the code already needs a double-click; the spec should follow the code and the plugin rule.
  - **Enter** or **middle-click** loads it into the selected channel when that channel is the same instrument.
  - Drag onto the rack.
  - Right-click: Load into new channel, Load into selected channel, Add to favourites, Show in file manager, **Delete preset** (user presets only, `antiserum.delete_preset`).
- **Project file rows**:
  - Click selects.
  - **Double-click** or **middle-click** opens the project, offering to save unsaved changes first. Middle-click keeps its meaning of "load this here".
  - Right-click: Open, Show in file manager, Add to favourites.
- **Empty folder**: one dim, indented line, "no sounds".

### 5. Favourites tab

- A flat list of every starred item (sounds, plugins, presets, projects, folders), sorted by name.
- Each row keeps its type glyph and its filled star.
- Click the star to unstar; the row leaves the list.
- Tab view option **Show parent folders** groups the favourites under their folders instead of listing them flat.
- Several rows selected with Ctrl+click or Shift+click can be starred at once from the right-click menu.

### 6. Bottom area (top to bottom)

**a. Preview strip** (shown while an audio item is selected; options menu → View → Preview strip turns it off)

- A dark, display-like box the full width of the column, in the style of our toolbar displays (`icons::lcd`, `icons::scope`).
- **Waveform**: fills the box, one lane drawn mirrored around a centre line. The part already played is in a stronger tone than the rest, and a thin playhead line moves across while the preview plays.
- **Overlay on hover**: a row along the bottom edge of the box.
  - Left: a **play/stop** button (plays the whole file; becomes stop while playing), a short **preview volume** slider and a **loop** toggle.
  - Right: small info text, for example "44.1 kHz · 24-bit · stereo · 2.35 s", plus "· 120 BPM" when the file carries a tempo.
- **Click** the waveform to play from the start. **Shift+click** plays from that point.
- **Drag** from the waveform behaves like dragging the row.
- **Wheel over the volume slider** changes the preview volume.
- **Right-click** opens the sound menu.
- **Preview length**: by default a click previews only the first few seconds (own default). View → **Preview whole files** removes the limit. Transport stop also stops the preview (already in `docs/spec/transport.md`).

**b. Tags panel** (shown while Tags is on)

- **Filter row**:
  - Left: a **Type** menu button showing small glyphs of the chosen types. It opens a tick list: Sounds, Presets, Projects, Plugins, MIDI files.
  - Right: an **Any | All** two-way switch (`icons::segmented`): results match any selected tag, or every selected tag.
- **Chip rows**:
  - First a **star chip** (only starred items), then one chip per tag, equal width, wrapping onto more rows as needed.
  - Click toggles a chip; several can be on.
  - Right-click a chip: Rename tag, Delete tag (removes it from every item).
- Tags are the user's own tags plus the tags our Antiserum presets already carry.

**c. Footer row** (search bar)

| # | Element | Behaviour |
|---|---|---|
| 1 | **Tags** toggle, a short word button at the left | Shows or hides the tags panel; marked while on |
| 2 | **Search field**, stretching | Placeholder "Search…". Case-insensitive. Supports `*` wildcards and the words AND, OR, NOT, and a quoted phrase for an exact match. **Enter** runs the search and jumps to the first result. **Esc** clears it and gives focus back. Ctrl+F over the browser focuses it |
| 3 | **Search this folder**, a folder glyph at the right inside the field | Limits the search to the selected folder and its subfolders; shown pressed while active |
| 4 | **Search** button, a magnifier at the far right | Runs the search. While a search is active, F3 / F2 step to the next / previous result |

**Search results**:

- Default: results are shown inside their parent folders, which open on their own, with the matched letters marked.
- Option: a flat list of results without folders.
- Searches cover user folders (from a cached index, rebuilt by Refresh), the instruments-and-effects library, presets and this project.
- Option **Include folders**: folder names can match too.

### 7. Drag and drop out of the browser

- **Drag ghost**: a small label chip beside the pointer with the item's glyph and name (the existing ghost in `lib.rs`, given the vector glyph; its tones follow the main-window section).
- **Drop targets**: each target lights up while the item hovers over it.

| Dragged | Dropped on | Result |
|---|---|---|
| Sound | Empty part of the channel rack | New sampler channel (`channel.add_sampler`, existing) |
| Sound | An existing channel row | Replaces that channel's sound: a sampler's sample (`channel.set_sample`), or any other instrument with a sampler (see `channel-rack.md`) |
| Sound | Playlist track | Sampler channel plus an audio clip at the snapped drop position on that track (existing, `docs/spec/playlist.md`) |
| Sound | Sample editor / zone editor / kit pad | Loads it there |
| Generator or preset | An existing channel row | Replaces that channel's sound (`channel.swap_sound`, see `channel-rack.md`) |
| Generator or preset | Empty part of the channel rack | New channel |
| Generator or preset | A mixer strip | New channel routed to that mixer track (own design) |
| Effect | A mixer strip | Effect added in the strip's first free slot (`mixer.add_effect` / `mixer.add_plugin`) |
| Effect | A specific slot in the mixer's effect list | Added at that slot |
| Project file | Workspace | Opens it, offering to save first |
| Folder from the system file manager | Browser | Adds it as a browser folder |

- **Alt held while dropping** stops the new channel's settings window from opening (if it would open).
- Dropping where nothing accepts the item does nothing; the ghost fades out.

### 8. Options menu (header item 1)

Grouped, with small dim section labels and separators, before the common window items:

- **Search**:
  - Show parent folders in results
  - Open folders that contain results
  - Include folders in results
  - Search bar at top
  - Filter as I type (off by default)
  - Previous result (F2)
  - Next result (F3)
- **Navigation**:
  - Back (Backspace)
  - Forward (Shift+Backspace)
- **Actions**: Send previous / next item to selected channel (Shift+Up / Shift+Down)
- **View**:
  - Hide automatically
  - Hide while dragging out (floating only)
  - Tab labels › (Auto, Glyphs, Words)
  - Preview strip
  - Preview whole files
- **Placement** (as in the main-window section): Dock left, Dock right, Float, Auto-hide.
- **Folders**:
  - Fold all (Ctrl+Up)
  - Refresh folders (no key; see §9)
  - Browser folders… (opens Settings → Folders, F10)
  - Rescan plugins (existing button, moved here)

### 9. Keyboard

**Proposed (needs `keymap.md` and `default.keymap` updated first).** All of these are window-local: they apply only while the pointer is over the browser or it has focus, and window-local keys take precedence over global ones, as in the channel rack. They are keymap entries (`browser.*` invocations handled by the UI), never hard-coded.

| Key | Action |
|---|---|
| Up / Down | Move the focus row |
| Right / Left | Open a folder or move into it / close it or move to the parent |
| Enter | Folder: toggle. Sound or preset: send to the selected channel. Plugin: add it. Project: open it. (Window-local, so it does not conflict with the main-window proposal for Enter) |
| Shift+Up / Shift+Down | Move to the previous / next item in the folder **and** send it to the selected channel, to audition sounds in place |
| Space | Stays global play/stop (`transport.toggle`); it does not preview |
| Backspace / Shift+Backspace | Back / forward through the browser's history of opened folders |
| Ctrl+F | Focus the search field |
| Shift+F | Search the selected folder (own choice of key; unbound today) |
| Alt+F | Search for the exact phrase typed (own choice of key; unbound today) |
| F3 / F2 | Next / previous result |
| Esc | Clear the search; stop the preview |
| Ctrl+Up | Fold all |
| Shift+wheel over the list | Zoom the list: steps through Compact, Normal, Large |
| Alt+F8 | Show / hide the browser (global, existing) |
| F8 | Open the plugin chooser (§10; global, unbound today) |

Refresh has **no key**: Ctrl+R is already `ui.export` in `default.keymap`, so refresh stays on the header button and in the menu.

### 10. Related: plugin chooser overlay (F8, or right-click the toolbar's browser toggle)

**Proposed (needs a spec first).**

- A **full-window overlay** over the workspace showing the favourites of the instruments-and-effects library as a grid of tiles: **generators on the left half, effects on the right half**.
- A **category strip** along the bottom edge lists the categories of both halves, with "All" in the middle.
- Typing filters by name at once.
- Arrow keys: Up/Down move between tiles, Left/Right between categories. The mouse wheel changes category.
- **Double-click** a tile: a generator becomes a new channel, an effect goes onto the selected mixer track.
- **Press and drag** a tile: the overlay closes at once and the plugin travels with the pointer to any drop target in §7.
- Esc, or a click on empty space, closes it.
- Each tile is our own drawing (the plugin's type glyph and its name), never a picture of a third-party editor.

## Icons

All glyphs are vector drawings in the style of `crates/musiccraft-ui/src/icons.rs`: a unit square, thin strokes, simple fills. Header glyphs shared with other windows are defined in the main-window section and repeated here only for reference.

| Where | What it does | Our glyph |
|---|---|---|
| Header, far left | Window menu / browser options | The shared menu glyph: three short horizontal bars stacked evenly |
| Header | Fold all | As the main-window section: an upward chevron over two short bars |
| Header | Refresh | An open circle of about three quarters, with a small arrowhead on one end |
| Header, right | Collapse to header / expand | The shared collapse glyph: an upward chevron over a short bar; a downward chevron while collapsed |
| Header, right end | Close | The shared header cross |
| Tab: All | Whole tree | The existing `Icon::Browser` folder outline |
| Tab: Project | This project only | The existing `Icon::Playlist` (three staggered bars) |
| Tab: Plugins | Instruments and effects only | A 2 × 2 grid of small rounded squares, one of them outlined only (the same glyph as the toolbar's proposed `Icon::Plugins`) |
| Tab: Presets | Presets only | A knob: a circle outline with a pointer line from the centre to the rim toward the upper right |
| Tab: Favourites | Starred items | A five-pointed star outline; filled while the tab is active |
| Tree: folder (closed) | Folder | `Icon::Browser` folder outline |
| Tree: folder (open) | Open folder | The same outline with its front drawn as a slanted parallelogram, lifted open |
| Tree: user folder | User-added folder | The folder glyph, tinted as a user folder (see Colour roles) |
| Category: This project | — | `Icon::Playlist` |
| Category: Recent | — | A clock: a circle outline with two hands |
| Category: Instruments & effects | — | The 2 × 2 grid glyph |
| Category: Presets | — | The knob glyph |
| Category: Sound packs | — | A box: a rectangle with a lid line across its upper quarter |
| Category: Autosaves / Undo history | — | A circle outline with an arrowhead turning counter-clockwise and one clock hand inside |
| Row: sound | Audio file | Five thin vertical bars of different heights centred on a line (a small waveform) |
| Row: MIDI file | MIDI file | A circle outline with three small dots along the inside of its upper arc (a round connector) |
| Row: pattern | Pattern | A small rounded square filled with the pattern's colour |
| Row: automation clip | Automation | A three-segment zigzag line with a small circle at each node |
| Row: controller | Controller | One period of a sine wave |
| Row: generator | Instrument | The existing `Icon::Keyboard`, small |
| Row: effect | Effect | A square outline with an S-shaped curve running corner to corner inside (a transfer curve) |
| Row: preset | Preset | The knob glyph, small |
| Row: project file | Project | A page outline with one folded corner and three short staggered bars inside |
| Row: undo step | Undo step | A short curved arrow pointing left |
| Row, right end | Favourite | Star outline (hover) / filled star (starred) |
| Row, right end (open folder) | Open marker | A small downward chevron |
| Footer | Search this folder | Folder outline, small |
| Footer, far right | Search | The existing `Icon::Zoom` magnifier |
| Tags panel | Type filter | A funnel (a downward triangle on a short stem), followed by the word "Type" |
| Preview strip | Play / stop | The existing `Icon::Play` triangle / `Icon::Stop` square |
| Preview strip | Preview volume | The speaker body of `Icon::Mute` without the slash, with one small arc in front |
| Preview strip | Loop | Two curved arrows chasing each other round a circle |
| Toolbar | Show / hide browser | The existing `Icon::Browser` in the window-toggle row |

## Colour roles

| Element | Token(s) | Relative tone and state changes |
|---|---|---|
| Header row, tab strip, footer, tags panel (the column's chrome) | `theme::PANEL` (header fill as the main-window section's `HEADER` / `HEADER_ACTIVE`) | The column's frame tone, matching the main-window section's browser column fill and other window bodies |
| List well (behind the rows) | `theme::BACKGROUND` | One step darker than the chrome, so the list reads as an inset well between the lighter bars |
| Preview strip box | `theme::LCD_BG`, edge `theme::LCD_EDGE` | Darkest tone in the column, like the toolbar displays |
| Divider | `theme::PANEL_DARK` hairline | `theme::ACCENT_DIM` on hover and while dragging, as in the main-window section |
| Header title, inactive tab words and glyphs | `theme::TEXT_DIM` | `theme::TEXT` on hover |
| Active tab | `theme::ACCENT` (outline frame and glyph or word) | A thin rounded outline; no fill, or at most `theme::RAISED` |
| Header tool glyphs | `theme::TEXT_DIM` | `theme::TEXT` on a `theme::RAISED_HOVER` square on hover; the close cross turns `theme::DANGER` on hover |
| Row name (normal) | `theme::TEXT` | Open folders' names slightly stronger (bold), same token |
| Row hover band | `theme::RAISED` | One step lighter than the well, full row width |
| Selected / focused row band | `theme::ACCENT_DIM` | Full width, with a short `theme::ACCENT` bar in the focus marker column |
| Tree lines | proposed new `theme::TREE_LINE` | A step lighter than the well and fainter than `theme::TEXT_DIM`; it may alias `theme::GRID_BEAT` |
| Folder glyph (ordinary) | `theme::TEXT_DIM` | `theme::TEXT` while the row is hovered |
| User-added folder glyph | `theme::HIGHLIGHT` | The only thing marking it, so names stay readable |
| Category glyphs (This project, Recent, Instruments & effects, Presets, Sound packs) | `theme::ACCENT` | Fixed |
| Sound glyph | `theme::ACCENT` | `theme::PLAYHEAD` while that sound previews |
| Preset glyph, star (starred) | `theme::HIGHLIGHT` | The hover-only outline star is `theme::TEXT_DIM` |
| Generator / effect glyphs | `theme::TEXT` | — |
| Pattern swatch | the pattern's own colour (`pattern_text` / `theme::PALETTE`) | — |
| Automation and controller glyphs and names | `theme::AUTOMATION` | As they are coloured elsewhere |
| Format tag (CLAP / VST3), counts "(n)", empty-folder text | `theme::TEXT_DIM` | Small text |
| Missing file / unusable item | `theme::TEXT_DIM` name | No glyph tint; the hint explains |
| Search-match marking in names | `theme::HIGHLIGHT` | Only the matched letters |
| Search field | fill `theme::PANEL_DARK`, text `theme::TEXT`, placeholder `theme::TEXT_DIM` | `theme::ACCENT` outline while focused |
| Tags toggle | `theme::TEXT_DIM` word | `theme::ACCENT` word plus underline while on (as `icons::toggle`) |
| Tag chips | off: fill `theme::PANEL_DARK`, word `theme::TEXT_DIM` | on: fill `theme::HIGHLIGHT`, word `theme::BACKGROUND` (as `icons::segmented`) |
| Any / All switch | `icons::segmented` | Unchanged |
| Preview waveform | played part `theme::ACCENT`, unplayed `theme::ACCENT_DIM` | Playhead line `theme::PLAYHEAD`; centre line `theme::LCD_EDGE` |
| Preview overlay controls | `theme::TEXT_DIM` | `theme::TEXT` on hover; loop on in `theme::ACCENT` |
| Scroll bar | handle `theme::RAISED` | `theme::RAISED_HOVER` on hover, `theme::ACCENT_DIM` while dragged |
| Drag ghost | `theme::RAISED` fill, `theme::TEXT` label, `theme::ACCENT` hairline outline | As the main-window section proposes |
| Drop-target outline | `theme::ACCENT` | Existing for the rack; the same on mixer strips, slots and playlist rows |
| Dock drop zones while dragging the title | `theme::ACCENT_DIM` translucent band with a `theme::ACCENT` outline | Only over a valid edge, as in the main-window section |

## Labels and wording

- **Header title**: "Browser", in sentence case like "Channel rack" and "Mixer". This replaces today's small-caps "BROWSER" (also listed in the main-window section).
- **Tabs**: "All", "Project", "Plugins", "Presets", "Favourites". Shown as words only when the column is wide (Auto mode), and always in the tab's hint.
- **Categories**:
  - "This project": proposed rename of today's "Current project", so the tree does not repeat the original's folder names.
  - "Recent"
  - "Instruments & effects": proposed rename of today's plugin branch heading (`browser.rs`, `plugin_database`), which repeats the original's coined name for that branch.
  - "Presets"
  - "Sound packs": proposed rename of today's "Packs".
  - "Autosaves"
  - User folders use their folder's own name. The "Folders" grouping header goes away.
- **This project children**: "Undo history (n)", "Patterns (n)", "Effects (n)", "Channels (n)", "Controllers (n)", "Automation (n)", "Samples (n)". The count is in parentheses after a space (as today's "Patterns (n)").
- **Library children**: "Effects", "Generators", "Installed". Sub-categories are our own plain words, for example "Delay & reverb", "Distortion", "Dynamics", "Filter & EQ", "Modulation", "Utility", "Synths", "Drums", "Keys", "Samplers".
- **Plugin rows**: the plugin's own name, with no "fx" prefix or text glyph. Format tag: "CLAP" or "VST3". Menu items "Add to library ›" and "Remove from library".
- **Built-in entries keep today's labels**: "Synth", "Antiserum", "Drum synth: ‹kind›", "Pluck", "FM synth", "Keys: piano", "Keys: electric piano", "Keys: organ", "Additive", "Drum kit", "Zone sampler", and the built-in effect titles. Two proposed renames, because today's labels repeat the names of the original's bundled channel plugins:
  - "Layer" → "Channel stack" (hint stays "Plays several channels at once");
  - "MIDI Out" → "MIDI output" (hint "Sends notes to external MIDI gear").
- **Empty states**:
  - "no sounds" (an empty folder): proposed change from today's "no WAV files".
  - "Right-click a knob and add a controller" (existing, `controllers.rs`).
  - "Plugin hosting is not available" (existing, `plugins.rs`).
  - "Nothing starred yet: hover a row and click its star".
  - "No matches" (search), "reading…" (a folder being read).
- **Footer**: "Tags" (a short word button), placeholder "Search…". Tags panel: "Type", "Any | All".
- **Hint bar** (`crates/musiccraft-editors/src/hint.rs`): short verb-first fragments joined with " · ", for example:
  - Sound: "Click to preview · double-click to add · drag onto the channel rack · right-click for more" (existing). Once the playlist drop is listed, it can read "drag onto the channel rack or playlist".
  - Folder: "‹full path› · click to open".
  - Generator: "‹vendor› · ‹format› · double-click to add a channel".
  - Effect: "‹vendor› · ‹format› · double-click to add it to the selected mixer track".
  - Tab: "‹tab name› · right-click for tab options".
  - Divider: "Drag to resize · right-click to swap widths".
- **Preview strip info**:
  - sample rate in kHz with at most one decimal ("44.1 kHz", "48 kHz");
  - bit depth as "16-bit", "24-bit", "32-bit float";
  - channels as "mono" or "stereo";
  - length in seconds with two decimals under a minute ("2.35 s"), otherwise "m:ss.s" ("1:02.5");
  - tempo as "120 BPM", only when known;
  - fields joined with " · ".
- **Menus** use our own sentence-case wording as listed in Anatomy. Items that open a dialog end with "…".

## States and feedback

- **Hover**:
  - Row: a full-width band one step lighter than the well. The outline star appears at the right end, and a cut-off name pops out in full.
  - Header and footer buttons: the button fill appears and the glyph brightens.
  - Tabs: the word or glyph brightens.
  - Divider: accent-dim line and a resize cursor.
  - Preview strip: its control row appears.
- **Pressed**: buttons show the accent-dim fill while held. A row pressed for a drag keeps its selected band.
- **Selected**: a full-width accent-dim band plus the short accent bar at the far left. One item at a time, unless Ctrl+click or Shift+click multi-select is used (for starring, tagging and multi-drag).
- **Keyboard focus**: drawn the same as selected. The list scrolls to keep the focus row visible. Moving onto a sound with Shift+Up/Down sends it.
- **Open folder**: stronger name, open-folder glyph, small chevron at the right end, and tree lines down to its children.
- **Previewing**: the row's sound glyph turns the playhead tone. The preview strip shows the moving playhead and the played part in the stronger tone. When the preview stops, both return to normal.
- **Favourite**: a filled star in the highlight tone, always visible.
- **Active / on toggles**:
  - the active tab has an accent outline;
  - the Tags toggle is underlined;
  - tag chips that are on are filled with the highlight tone;
  - "search this folder" is shown pressed;
  - the preview loop toggle is in the accent.
- **Disabled / unavailable**: dim name, no glyph tint, no drag, and the hint says why (missing file, plugin hosting unavailable, unknown file type).
- **Searching**:
  - matched letters are marked;
  - folders holding results open on their own (or the list goes flat);
  - the search field shows its accent focus outline;
  - with no results, one dim "No matches" line appears.
- **Reading folders**: the refresh glyph turns once. Folders still being read show one dim "reading…" line.
- **Dragging out**: the label chip follows the pointer, and each valid target shows its accent outline while hovered. A floating browser set to hide while dragging fades out until the drop.
- **Zoom**: Shift+wheel steps the list's zoom; rows grow or shrink in place and the focus row stays in view.
- **Muted, recording**: not applicable. Nothing in the browser changes with mute or record state.
- **Collapsed / auto-hidden**: only the header row (collapsed) or a thin edge strip (auto-hide) remains. The strip brightens on hover just before the column slides out.

## MusicCraft today: gaps

Ordered by visual impact. Each item names the file(s) to change; behaviour changes need `docs/spec/browser.md` (new) or the named spec first.

1. **Missing categories.** The tab strip and the fixed category order are in (`docs/spec/browser.md`), but there is no Favourites tab and no Recent or Autosaves category. `crates/musiccraft-ui/src/browser.rs` (`show`, `Tab`).
2. **Search has no tags, Type filter or Any/All.** Search sits in the footer now, but the Tags toggle, "search this folder", the magnifier and the tags panel (with tag storage) are missing. `crates/musiccraft-ui/src/browser.rs` (`show`, `BrowserState`).
3. **No preview strip or preview controls.** A click always previews the whole file at a fixed level. There is no waveform, playhead, volume, loop or info line, and no short-preview default. `crates/musiccraft-ui/src/browser.rs`; `Action::PreviewSample` in `crates/musiccraft-app/src/main.rs` (fixed gain); `Preview` in `crates/musiccraft-engine/src/graph.rs`.
4. **No favourites.** No star on rows and no Favourites tab. `crates/musiccraft-ui/src/browser.rs`; favourites need a per-user store (`crates/musiccraft-ui/src/settings.rs`, `docs/spec/settings.md`).
5. **Drag and drop is partial.**
    - Effects cannot be dragged, and nothing can be dropped on the mixer.
    - Generators dropped on a mixer strip are not routed.
    - Files: `crates/musiccraft-ui/src/browser.rs`, `crates/musiccraft-ui/src/mixer.rs` (no drop handling), `crates/musiccraft-ui/src/channel_rack.rs` (`drop_target`, which replaces samplers' samples only); specs `channel-rack.md`, `mixer.md`.
6. **Only WAV files are listed.** Project files and presets on disk are never shown, and there are no "other file types" or "file extensions" options. `crates/musiccraft-ui/src/browser.rs` (`BrowserState::listing`).
7. **Keyboard navigation is partial.** Up/Down/Left/Right/Enter, Ctrl+Up, Ctrl+F and Esc work (`keymap.md`, "Over the browser"); Enter adds the row rather than sending a sound to the selected channel. Shift+Up/Down send a sound to the selected channel, and Backspace and Shift+Backspace go back and forward through the folders opened. Still missing: Shift+F, Alt+F, F2/F3, Shift+wheel zoom and F8. Refresh gets no key because Ctrl+R is export.
8. **Long names.** The simplified bar now sits on the column's outer (left) edge (`scrollbars::plain_area_left`); it moves to the right edge once docking right exists. Long names are cut with an ellipsis and pop out in full on hover. `crates/musiccraft-ui/src/browser.rs`.
9. **Resize, dock and hide options are missing.** The panel resizes and collapses to its header, but there is no second remembered width, no dock-right, no floating mode and no auto-hide. `docs/spec/window-layout.md` first; then `crates/musiccraft-ui/src/browser.rs` (`egui::Panel::left`), `crates/musiccraft-ui/src/lib.rs` (`WindowState`).
10. **No plugin chooser overlay** (F8). A new spec and a new module in `crates/musiccraft-ui/src/`.

## Sources

- **FL Studio online manual** (public), compared for layout, proportions, behaviour and general feel only, under the owner's decision of 2026-10-21 ("layout plus general feel", `docs/clean-room.md`). Nothing was measured, colour-sampled or traced, and no wording was copied.
  - `browser.htm`:
    - the controls at the top of the panel (menu, collapse, refresh, minimize, close, tabs and their icon/text modes, the divider and its two positions, docking, detaching, auto-hide);
    - mouse and keyboard use of browser content (click preview, Shift/Alt+click, middle-click to load into a channel or open a project, Enter, Shift+Up/Down send, drag targets, Alt while dropping, back/forward);
    - the keyboard shortcut list (search, exact phrase, search in folder, find previous/next, cancel, collapse, reread, quick zoom with Shift+wheel);
    - adding folders;
    - the "Current project" folder (history, patterns, effects, generators, remote and initialized controls, samples);
    - the "Plugin database" (favourite categories versus installed plugins, adding and removing entries);
    - the browser options menu;
    - search (location, parent-folder display, wildcards and boolean words, find previous/next, search in folder, indexing);
    - the preview player and its show/hide option;
    - tags (type filter, any/all, adding and editing tags);
    - the starred tab (hover star);
    - the plugin picker (generators left, effects right, category strip, type to filter, drag closes it);
    - the tab right-click menu (zoom levels, one-folder view, folder icons, waveforms, extensions, unknown types, sort, view layouts, rename/colour/icon, freezing).
  - `toolbar_panels.htm`: the shortcut icons for the browser and the plugin picker (left-click opens the browser, right-click opens the picker; Alt+F8 and F8).
  - `basics_interface.htm`: the overview of where the browser sits beside the workspace.
  - Screenshots looked at, without measuring, sampling colours or tracing:
    - `img_shot/browser.png`, `basics_interface.png`;
    - `browser_project.png`, `browser_plugindatabase.png`;
    - `browser_search.png`, `browser_preview_player.png`;
    - `browser_favoriting.png`, `browser_tab_starred.png`, `browser_tab_library.png`;
    - `plugin_picker.png`, `browser_sounds.png`, `sounds_previewplayer.png`.
  - Used for: column placement and width relative to the window; header order (tools before the title); the equal-width tab strip with an outlined active tab; tree rows with connector lines and a staircase indent; the open-folder marker and star at the row's right end; the scroll bar on the outer edge; the footer with tags toggle, search field, folder button and magnifier; the tags panel above the footer; the preview strip above the footer with controls revealed on hover.
- **Repository**:
  - `crates/musiccraft-ui/src/browser.rs`, `controllers.rs`, `plugins.rs`, `chrome.rs`, `icons.rs`, `toolbar.rs`, `settings.rs`;
  - `crates/musiccraft-ui/src/lib.rs` (drag ghost, panel order, key handling), `channel_rack.rs`, `playlist.rs` and `mixer.rs` (drop handling), `crates/musiccraft-app/src/main.rs` (preview), `crates/musiccraft-engine/src/graph.rs` (preview voice);
  - `crates/musiccraft-editors/src/theme.rs` (tokens), `crates/musiccraft-editors/src/hint.rs` (hint bar);
  - `crates/musiccraft-ui/keymap/default.keymap` (Ctrl+R is export; free keys);
  - `docs/spec/window-layout.md`, `sample-paths.md`, `keymap.md`, `channel-rack.md`, `controllers.md`, `playlist.md`, `antiserum-app.md`, `settings.md`, `transport.md`;
  - `docs/screenshots/playlist-mixer.png`, `docs/screenshots/channel-rack.png`;
  - the main-window section of this guide (shared header, glyphs, divider, docking, drag ghost tones).
- **Own design**:
  - every glyph and its tint, including the 2 × 2 grid glyph for the instruments-and-effects library;
  - all colour-role assignments, and the proposed `theme::TREE_LINE`;
  - all labels and menu wording: tab names, "Favourites", "This project", "Instruments & effects", "Sound packs", "Undo history", "Channels", "Autosaves", the "Installed" sub-grouping, effect and generator sub-category names, "Channel stack", "MIDI output";
  - the preview strip's controls and info format, the short-preview default, and the volume, loop and info line;
  - three zoom levels (a reduction) and the single row layout;
  - Lock view and Remember width wording;
  - drops of generators onto mixer strips and of sounds onto existing channel rows, mapped to our commands;
  - the key choices beyond the manual's list, making the browser keys window-local, and leaving refresh without a key;
  - double-click to open project files, alongside middle-click;
  - the format tag for CLAP/VST3 duplicates;
  - the scroll bar following the outer edge when docked right;
  - the plugin chooser's tile drawing.
