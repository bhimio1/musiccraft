# Spec: browser

The column at the left of the main window for finding things to add: the
open project's contents, the built-in and installed instruments and effects,
presets, the built-in sound packs and the user's folders.

## Layout

Top to bottom:

1. **Header**: the window menu, Fold all, Refresh, the title "Browser",
   then Collapse and Close at the right end (no maximize). Collapse shrinks
   the column to its header row; Close hides the browser (Alt+F8 and the
   toolbar button show it again).
2. **Tab strip**: All, Project, Plugins, Presets, equal width. Each tab shows
   a glyph; when the column is wide enough the tab names are shown instead.
   The hint names the tab. The tab is remembered for the session.
3. **List**: a well one step darker than the column, scrolling vertically.
4. **Footer**: the search field ("Search…").

The window menu holds Fold all, Refresh folders, Add folder…, Add channels
from files… (both open the system's file dialog), and Rescan plugins (when plugin hosting is available), then
the common window items.

## Tree

The All tab shows these categories in this order; the other tabs show only
their category.

1. **This project**: Patterns (n), Channels (n), Effects (n), Controllers
   (n), Automation (n), Samples (n), History (n).
   - A pattern: click selects it (`pattern.select`); right-click → Split by
     instrument (`pattern.split_by_channel`, see `playlist.md`).
   - A channel: click selects it and opens its settings window.
   - An effect ("track · effect"): click opens or closes its effect window.
   - Controllers: the controller list (`controllers.md`).
   - An automation clip: click opens it in the automation editor.
   - A sample: as a sound row (below).
   - History: the same steps as Edit › History (`commands.md`), the redo
     steps dim above the undo steps, newest first; n counts the undo steps.
     A click undoes or redoes to just after that step.
2. **Instruments & effects**: Effects (the built-in effects), Generators (the
   built-in instruments), Installed (Effects and Generators found by the
   plugin host). An installed row ends in a dim format tag, CLAP or VST3, so
   a plugin installed in both formats shows as two rows that differ only by
   the tag. When plugin hosting is not available, Installed says so.
3. **Presets**: one folder per instrument with presets (today Antiserum),
   holding its factory categories and then User.
4. **Sound packs**: the built-in sounds (Drums).
5. The user's **folders** (Settings → Folders), each at top level under its
   own name. Inside a folder, subfolders come first, then WAV files, each
   sorted by name ignoring case; hidden files are left out. An empty folder
   shows "no sounds".

## Rows

- Every entry is one full-width row: a narrow marker strip, an indent per
  level with faint tree lines from the parent, a type glyph, then the name,
  cut short with an ellipsis when too long; hovering a cut-off name pops it
  out in full in a raised box over the row. Open folders show their name
  stronger and a small down chevron at the right end.
- Hovering a row lights the whole row. The selected row has a band and a
  short accent bar in the marker strip.
- Folders and categories: click opens or closes them. They open closed,
  except This project's parent categories in the Project, Plugins and
  Presets tabs, which open on their own.
- Sounds: click selects and previews, double-click adds a sampler channel,
  right-click offers the "Add as" choices, and they can be dragged onto the
  channel rack or the playlist. Dropped on a channel's row, a sound
  replaces that channel's sound (the row is outlined while it hovers; see
  "Replacing a channel's sound" in `channel-rack.md`); anywhere else in the
  rack it adds a sampler channel.
- Presets: click selects and previews, as a sound does: the preset plays
  middle C for about a second and a half, then its release, on the master
  output (the rendered preview is kept, so a second click plays at once).
- Generators, effects and presets: click selects (generators and effects
  do not preview); double-click adds a
  generator or preset as a new channel, or an effect to the selected mixer
  track. Generators and presets can be dragged onto the channel rack: onto a
  channel's row they replace its sound, anywhere else they add a channel.
  Effects (built-in and installed) can be dragged onto a mixer strip, which
  is outlined while one hovers, or onto the slot panel (the selected
  track); the effect goes into that track's first free slot. Effects
  dropped elsewhere do nothing.

## Search

- The search matches names, ignoring case. While searching, every category
  and folder that holds a match opens on its own, and those without one are
  hidden; user folders are searched to the same depth they can be opened.
  With no match at all the list shows "No matches".
- Antiserum presets also match on their tags.

## Keyboard

While the pointer is over the browser (and no text field has focus), its
keys take precedence over the global ones (`keymap.md`, "Over the
browser"): Up and Down move the selected row; Right opens a closed folder
or steps into an open one; Left closes an open folder or steps out to the
parent; Enter opens or closes a folder and on any other row does what a
double-click does (add the sound, generator, preset or effect); Ctrl+Up
folds all; Ctrl+F puts the cursor in the search field; Esc clears a search.
The list scrolls to keep the selected row in view.

## Sources

- Interface style guide, `docs/spec/style/browser.md` (owner's 2026-10-21
  decision to follow the manual's layout): the header, tab strip, fixed
  category order, full-width rows with type glyphs and tree lines, the
  installed plugins' format tag, the footer search, and search opening the
  folders that hold results.
- Owner's request (2026-10-23): presets preview on click, like samples.
- Own design: the preview note and length, the glyphs, labels, tones and the categories' contents.
