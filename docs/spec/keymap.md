# Spec: default keyboard shortcuts

## Behavior

- Shortcuts are data: `crates/musiccraft-ui/keymap/default.keymap` maps key chords
  to invocations. The UI only looks chords up; it has no hard-coded shortcuts.
- A user keymap file can replace the default (`musiccraft --keymap <file>`).
- Shortcuts are ignored while a text field has keyboard focus.
- `view.toggle <window>` and `ui.*` invocations are handled by the UI (they open
  windows and file dialogs and do not change the project); everything else runs
  through the command registry.

## Typing keyboard

- While enabled (Ctrl+T or the ⌨ button), the letter rows play notes on the
  selected channel instead of triggering shortcuts: `Z S X D C V G B H N J M ,`
  play C4 to C5 with black keys on the home row; `Q 2 W 3 E R 5 T 6 Y 7 U I 9 O P`
  continue from C5. Shift is allowed; other modifiers pass through to shortcuts.
  R is a note while it is on, so record with the REC button instead.

## Defaults

| Chord | Action |
| --- | --- |
| Space | Play / stop |
| Ctrl+Space | Play / pause |
| Home | Jump to the start (playback continues) |
| Ctrl+H | Panic: all notes off |
| L | Toggle pattern / song mode |
| Ctrl+M | Metronome on / off |
| R | Arm / disarm recording (`record.toggle`) |
| Ctrl+E | Step edit on / off |
| Ctrl+P | Recording count-in on / off |
| Ctrl+I | Wait for input before recording on / off |
| Ctrl+T | Typing keyboard plays notes |
| F5 / F6 / F7 / F9 | Playlist / channel rack / piano roll / mixer: a closed window opens and comes to the front, an open one behind others comes to the front, the focused one closes |
| Alt+F8 | Browser |
| Enter | Maximize the playlist, or restore it (opens it if closed) |
| Ctrl+Shift+H | Put the windows back in the default layout |
| Esc | Close the focused window (after a selection, menu, dialog or drag has taken it) |
| F12 / Alt+F12 / Ctrl+F12 | Close every window / only effect windows / every window but the focused one |
| Alt+T | Add a playlist marker at the playhead |
| F10 | Settings |
| F1 | Show or hide the keyboard shortcut list (help) |
| F4 | Next empty pattern (adds one if none), then name it |
| Shift+F4 | First empty pattern (adds one if none) |
| F2 | Rename the selected pattern (over the channel rack: the selected channel) |
| Plus / Minus | Next / previous pattern |
| 1–9 | Select pattern 1–9 (over the channel rack: mute channels instead) |
| P B D E C T Y Z | Tools: draw, paint, delete, select, slice, mute, play, zoom (piano roll and playlist) |
| S / O | New piano-roll notes are slide / portamento notes (toggle) |
| Ctrl+Insert / Ctrl+Delete | Insert / delete the loop region's time in the playlist |
| Ctrl+Z | Undo |
| Ctrl+Shift+Z, Ctrl+Y | Redo |
| Ctrl+N | Save new version (`song_2.mcproj`, …) |
| Ctrl+O | Open project |
| Ctrl+S / Ctrl+Shift+S | Save / save as |
| Ctrl+R | Export WAV |
| Ctrl+Shift+M | Export MIDI |

### Over the channel rack

Window-local keys, active while the rack is the focused window (the topmost
one, which a click anywhere in it makes; and the typing keyboard is off); they take precedence over the global keys above.

| Chord | Action |
| --- | --- |
| 1–9, 0 | Mute / unmute channel 1–10 (`channel.toggle_mute`) |
| Ctrl+1–9, Ctrl+0 | Solo channel 1–10 (`channel.solo`) |
| Alt+C / Alt+Delete | Clone / delete the selected channel |
| Alt+Up / Alt+Down | Move the selected channel up / down |
| Ctrl+L | Route the selected channel to a free mixer track (`channel.route_free`) |
| Shift+Ctrl+Left / Right | Shift the selected channel's steps |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy / cut the selected channel's notes in the current pattern; paste replaces the selected channel's notes with the note clipboard, at the copied times (one undo step). The clipboard is shared with the piano roll, so it works across channels and patterns in both windows |
| Ctrl+K | Show or hide the graph editor |
| Alt+M | Hide or show the mixer-track column |
| F2 | Rename the selected channel |
| Page Down / Page Up | Next / previous group in the group filter (All, each group, No group; wraps) |

### Over the browser

Window-local keys, active while the pointer is over the browser and no text
field has focus; they take precedence over the global keys.

| Chord | Action |
| --- | --- |
| Up / Down | Move the selected row |
| Shift+Up / Shift+Down | Move to the previous / next row in the same folder; on a sound, load it into the selected channel when that channel plays a sample (a run of these within two seconds is one undo step) and preview it |
| Right / Left | Open a folder or step into it / close it or step out |
| Backspace / Shift+Backspace | Back: close the folder opened last and select it / forward: reopen the folder Back closed last |
| Enter | Open or close a folder; on other rows, as a double-click (add it) |
| Ctrl+Up | Fold all |
| Ctrl+F | Go to the search field |
| Esc | Clear the search |

### Over the piano roll

Window-local keys, active while the piano roll is the focused window (and
the typing keyboard is off); they take precedence over the global keys.

| Chord | Action |
| --- | --- |
| Ctrl+A / Esc | Select all notes / clear the selection |
| Ctrl+X / Ctrl+C / Ctrl+V | Cut / copy / paste (at the pointer) |
| Ctrl+B | Duplicate the selection after itself |
| Delete / Backspace | Delete the selection |
| Shift+Left / Right | Move the selection one snap step |
| Shift+Up / Down, Ctrl+Up / Down | Transpose the selection a semitone / an octave |
| Page Up / Page Down | Zoom in / out horizontally |
| Shift+1, Shift+2, Shift+3 | Fixed zoom levels (12, 24 and 48 points per step) |
| Shift+4 / Shift+5 | Fit the pattern / zoom to the time selection (or the selected notes) |
| Alt+C | Give the selected notes the colour group of new notes |
| Shift+Enter | Show or hide the property lane |

### Over the playlist

Window-local keys, active while the playlist is the focused window; they
take precedence over the global keys.

| Chord | Action |
| --- | --- |
| Ctrl+A / Esc | Select all clips / clear the selection |
| Ctrl+X / Ctrl+C / Ctrl+V | Cut / copy / paste (at the playhead, snapped down) |
| Ctrl+B | Duplicate the selection after itself |
| Delete / Backspace | Delete the selection |
| Shift+Left / Right | Nudge the selection one snap step |
| Shift+Up / Down | Move the selection up / down one track |
| Ctrl+Up / Ctrl+Down | Select the track above / below |
| Shift+C | Show the current pattern in the playlist (its first clip) |
| Page Up / Page Down | Zoom in / out around the playhead |
| Shift+1, Shift+2, Shift+3 | Zoom presets (32, 96 and 240 points per bar) |
| Shift+4 | Zoom all the way out; again to go back |
| Shift+5 | Zoom to the loop region |
| Alt+P | Show or hide the clip list |

### Over the mixer

Window-local keys, active while the mixer is the focused window; they take
precedence over the global keys.

| Chord | Action |
| --- | --- |
| Ctrl+A / Esc | Select every track / keep only the primary track selected |
| Alt+Left / Alt+Right | Move the primary track left / right (`mixer.move_track`) |

## Sources

- Space, L, Ctrl+M, F4, F5, F6, F7, F9, Alt+F8, Alt+T, numeric-keypad plus/minus,
  Ctrl+S, Ctrl+Shift+S, Ctrl+O, Ctrl+N (save new version), Ctrl+R and R
  (record) follow the default shortcuts listed in
  the publicly published FL Studio online manual ("Keyboard shortcuts"). Shortcuts
  are a convenience for users switching tools; the bindings are data and can be
  changed.
- Ctrl+T (typing keyboard): own choice. New project has no default shortcut;
  it is in the File menu.
- Ctrl+E (step edit), Ctrl+P (count-in) and Ctrl+I (wait for input) follow
  the FL Studio online manual, "Keyboard shortcuts" and "Recording".
- The single-key tools (P, B, D, E, C, T, Y, Z, plus S and O for new note
  kinds), 1–9 pattern select, the channel rack keys and Ctrl+Insert /
  Ctrl+Delete follow the FL Studio online manual, "Keyboard shortcuts"
  (global, channel rack, piano roll and playlist sections). Applying one tool
  choice to both editors is our own simplification.
- Ctrl+Z / Ctrl+Shift+Z / Ctrl+Y undo and redo: common desktop convention (own
  choice; the manual describes a different undo toggle).
- Enter (maximize the playlist), Ctrl+Shift+H (default window layout), Esc
  (close the focused window) and F12 / Alt+F12 / Ctrl+F12 (hide windows)
  follow the FL Studio online manual, "Keyboard shortcuts", "Window
  Navigation". What the default layout restores is our own design.
- The piano roll's and the playlist's window-local keys (Page Up / Page
  Down and Shift+1–5 zoom, Shift+arrow moves, Shift+C) follow the interface
  style guide (`docs/spec/style/piano-roll.md` and `playlist.md`, window
  menus), which follows the FL Studio online manual's window menus. The zoom
  preset widths are our own.
