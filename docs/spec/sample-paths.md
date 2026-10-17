# Spec: sample paths from other systems

Projects carry the sample paths of the computer that made them. A project
saved on Windows (an FL Studio import, or a MusicCraft project saved there)
names files like `C:\Users\Ana\Music\kick.wav` or
`%FLStudioFactoryData%\Data\Patches\Packs\Drums\Kicks\22in Kick.wav`. On
Linux and macOS MusicCraft **translates** such paths when it loads the sample.

## Behaviour

- Translation happens when a sample is loaded, never in the project: the
  stored path stays as written, so the project still works when it goes back to
  Windows. `project.localize_samples` (below) makes the found paths permanent.
- A path is **foreign** when it has a drive letter (`C:\…`, `d:/…`), a
  backslash, or a `%NAME%` placeholder. Other paths load as before (relative
  ones from the project's folder). On Windows nothing is translated.
- A foreign path becomes candidates, tried in order; the first file that exists
  is loaded:
  1. Backslashes become `/`.
  2. Placeholders expand: `%FLStudioFactoryData%` to the FL Studio factory data
     folder, `%FLStudioUserData%` to the FL Studio user data folder,
     `%USERPROFILE%` to the home folder and the Wine user folders.
  3. A drive letter maps through the **drive mappings** (`C:` or a longer
     prefix such as `C:/Users/Ana/Music` → a folder; the longest matching
     prefix wins), then to each Wine prefix (`C:` → `<prefix>/drive_c`, other
     letters → `<prefix>/dosdevices/<x>:`), then to `/mnt/<x>` (a mounted
     Windows drive).
  4. `C:\Users\<name>\<rest>` is also tried as `<home>/<rest>` (folders copied
     across, such as Music or Documents).
  5. Windows file names ignore case: each part of a candidate path is matched
     ignoring case when the exact name does not exist.
  6. Last, the file name alone is looked for in the project's folder and the
     browser's sample folders (up to four folders deep).
- When nothing matches, the sample plays silence and the load warning names
  the original path and says no translation was found.
- Results are remembered until the mappings change (Settings) or samples are
  reloaded.

## Folders

- **FL Studio factory data**: Settings → Folders, or the environment variable
  `MUSICCRAFT_FL_FACTORY`. When unset, MusicCraft looks for an FL Studio
  installed under Wine (`<prefix>/drive_c/Program Files/Image-Line/FL Studio*`
  and `Program Files (x86)`).
- **FL Studio user data**: Settings → Folders, or `MUSICCRAFT_FL_USER`; when
  unset, `<prefix>/drive_c/users/*/Documents/Image-Line/FL Studio` and
  `~/Documents/Image-Line/FL Studio`.
- **Drive mappings**: Settings → Folders (add a Windows prefix and a folder),
  or `MUSICCRAFT_DRIVE_MAP` as `C:=/mnt/win;D:/Samples=/home/ana/Samples`.
- **Wine prefixes**: `$WINEPREFIX`, `~/.wine`, and Bottles' bottles
  (`~/.local/share/bottles/bottles/*`).
- Settings override the environment variables; both add to what is detected.

## Commands

- `project.localize_samples` (undoable) replaces every foreign sample path
  that translates (samplers, granular and resynth channels, kit pads, zones)
  with the file found, and reports how many were changed and which were not
  found.
- `project.import_flp` reports, on Linux and macOS, how many samples were found
  through translation and lists those that were not.

## Sources

- Own design. The Wine prefix layout (`drive_c`, `dosdevices`) follows Wine's
  public documentation; the FL Studio placeholders are as they appear in project
  files read with PyFLP, and the factory and user data folders as named in the
  FL Studio online manual ("File settings").
