# nsane-save-editor (GUI)

The desktop and web app. It runs natively on Windows and Linux (eframe), and in a browser (WebAssembly, built with Trunk).

## Running

```sh
cargo run -p gui                        # empty window
cargo run -p gui -- --open tests/saves/game/0019.sav
```

Open a save with the **Open** button, by dropping the file onto the window, or with `--open`. Saving over a file first copies it to `<name>.<YYYYMMDD-HHMMSS>.bak` next to it.

### Editing Crash 3

The Crash 3 tab shows the overall progress on the left (percentage, totals, bosses, super powers, lives, the bonus gem, the story and any problems found in the save) and the six warp rooms in the middle. In a room, each level is a row of icons: crystal, clear gem, second gem and colored gem, where the level has them, then the relic and the best time trial time. Click an icon to add or remove it. A click on the relic moves to the next tier (none, sapphire, gold, platinum); a right-click picks one. Every click is one step that Ctrl+Z undoes. Hover over an icon to see what it is and what a click does. A click on a level's name, or on a boss in the overview, selects it.

A boss row has two toggles: the boss defeated, which opens the next warp room, and the super power the boss normally grants; the overview toggles the five super powers too. The padlock of a locked warp room unlocks the rooms up to it, optionally with the bosses' super powers and without their story scenes. The secret warp room's levels open with the total relic count, of any tier (5, 10, 15, 20 and 25 relics for levels 26 to 30): its card shows the count and the next threshold, and the padlock of a locked level gives relics to levels that have none until it opens. Both padlocks show the changes in a dialog before applying them. **Edit…** next to the story scenes count, in the overview, lists the hub story scenes step by step: a scene marked as seen doesn't play.

The **Presets** menu applies a common goal in one step: complete everything (108%), the story up to a warp room, every relic at a tier, every super power on or off, every story scene seen or not. Each opens a dialog with its options and a preview of the changes, the resulting totals and any new diagnostics. **History**, next to undo and redo, lists every step; a click goes back or forward to one.

The badges in the toolbar and the overview count the diagnostics (errors, warnings, notes). A click opens the diagnostics panel: **Fix** applies the suggested change as one step, **Show** selects the level concerned. While a Switch save is too large for its file (an error), Save is disabled.

Saving unsaved changes first shows a review: every change since the file was opened, the totals recomputed when writing, the warnings left, and the backup that will be made. "Don't show this again" turns it off; the settings menu (⚙) turns it back on.

**Convert to PC… / Convert to Switch…** writes a copy of the open save, unsaved changes included, for the other version of the game. The game data is copied into a save of that platform, the template: preferably one of your own saves from it, since it matches your game version, or the new-game save built into the editor. The dialog shows the totals of the converted save as read back from the bytes to write, and refuses to write if any game data would differ. The game keeps one file per save slot, `ALCHEMYXSAVE_<n>.sav` on PC and `MACK_<n>.sav` on the Switch, where Crash 3 uses slots 8 to 11; the suggested name is the file of the template's slot, or of the source's. To replace a slot, write over its file (a backup is made). Converted saves were checked in game in both directions.

### In a browser

```sh
cd crates/gui
trunk serve                 # http://127.0.0.1:8080
trunk build --release       # static files in crates/gui/dist
trunk build --release --public-url /nsane-save-editor/   # served from that sub-path
```

The file never leaves the browser. Open a save with **Open** or by dropping it onto the page. **Save** downloads the edited save under its name, and **Save as** asks for another name first; the opened file is left as it was, so there is no backup to make. The browser may add a number to a downloaded name that already exists (`ALCHEMYXSAVE_8 (1).sav`): the PC game only reads `ALCHEMYXSAVE_<n>.sav`, so rename it. While there are unsaved changes, the browser asks before the tab is closed or reloaded.

The settings menu (⚙) also opens **About**: the version, the license, and what the app does with your files.

### Settings

The app remembers the theme picked with the toolbar toggle, which side panels are shown on a narrow window, whether the review is shown before saving and, on the desktop, the folder of the last file opened or saved. They are kept in:

| Platform | Place |
|---|---|
| Linux | `$XDG_DATA_HOME/nsane-save-editor/app.ron`, by default `~/.local/share/nsane-save-editor/app.ron` |
| Windows | `%APPDATA%\nsane-save-editor\data\app.ron` |
| Browser | the site's local storage |

`--theme` overrides the stored theme without changing it. Runs with `--screenshot` neither read nor write the settings. Window size and panel widths are not kept.

### Keyboard shortcuts

| Keys | Action |
|---|---|
| Ctrl+O | Open |
| Ctrl+S | Save |
| Ctrl+Shift+S | Save as |
| Ctrl+Z | Undo |
| Ctrl+Y, Ctrl+Shift+Z | Redo |

On macOS, Cmd replaces Ctrl.

### Linux file dialogs

The file dialogs go through the desktop portal (`xdg-desktop-portal`) over D-Bus. Where no portal is running (some minimal or WSL setups), `zenity` is used instead if it is installed. With neither, the dialog can't open and the app says so: open files by dropping them onto the window, or with `--open`.

## Command-line options (native)

| Option | Effect |
|---|---|
| `--open FILE.sav` | Open a save at start. |
| `--apply EDITS.json` | Apply edits to it at start, one undo step each: a JSON list of edit ops, as in `tests/golden/scenarios.json`. |
| `--select LEVEL` | Select a level by its key (`toadvillage`, `deeptrouble`, …). |
| `--view NAME` | Set up a state at start: `unsaved` (the question asked before unsaved changes are lost), `invalid-time` (a refused time typed for the selected level), `unlock-dialog` (the dialog that unlocks the first locked warp room, as its padlock opens it), `cutscenes` (the story scenes dialog), `preset-108` and `preset-story3` (the dialogs of those presets), `diagnostics` (the diagnostics panel), `save-review` (the review shown before saving; nothing is written), `convert` (the conversion dialog; nothing is written), `about` (the About dialog). |
| `--template FILE.sav` | The template picked in `--view convert`. |
| `--theme light\|dark` | Force a theme instead of following the system. |
| `--size WxH` | Inner window size in points, for example `1280x800`. |
| `--maximized` | Start maximized. |
| `--icon-gallery` | Show every icon in every state and size instead of the editor. |
| `--export-icon DIR` | Write the app icon into `DIR`, then quit: `icon-<size>.png` from 16 to 256 pixels and `icon.ico`. |
| `--screenshot FILE.png` | Render a few frames, capture the window to a PNG, then quit. |

### Screenshots

`--screenshot` makes the app usable from scripts and tests: it applies the other options, waits until the window has settled (a few frames and 1.5 s), writes the PNG and exits. It works in debug and release builds.

```sh
nsane-save-editor --size 1280x800 --theme light --screenshot empty.png
nsane-save-editor --open tests/saves/game/0019.sav --select toadvillage --screenshot level.png
```

`--screenshot` can be repeated to capture several files from one window. Each capture uses the `--theme` given before it:

```sh
nsane-save-editor --open tests/saves/game/0019.sav \
    --theme light --screenshot light.png \
    --theme dark --screenshot dark.png
```

The capture is the window's own rendering, at the display's scale, without the window frame.

Without a window, an ignored test renders the main view (light and dark, at 1280×800 and 1024×720), a selected level and every `--view`, in both themes, to PNG files. It draws with wgpu and needs no display:

```sh
NSANE_RENDER_DIR=/tmp/renders cargo test -p gui renders -- --ignored
NSANE_RENDER_DIR=/tmp/renders NSANE_RENDER_ONLY=preset cargo test -p gui renders -- --ignored
```

`NSANE_RENDER_ONLY` keeps the renders whose name contains it.

## Fonts

The interface is set in Inter 4.1 (SIL Open Font License 1.1, `assets/fonts/OFL.txt`), regular and semibold, cut down to Latin, Latin Extended and the punctuation the app uses. Other scripts, in file names and system messages, fall back to egui's built-in fonts. To cut them again from the release's `extras/ttf` files, with fontTools:

```sh
pyftsubset Inter-Regular.ttf --no-hinting --name-IDs=0,1,2,3,4,5,6,13,14 \
    --unicodes=U+0020-007E,U+00A0-024F,U+02C6-02DD,U+2010-2027,U+2030-203A,U+2044,U+20AC,U+2122,U+2190-2199,U+2212,U+2260,U+2264-2265 \
    --output-file=crates/gui/assets/fonts/Inter-Regular.ttf
```

and the same for `Inter-SemiBold.ttf`.

## App icon

The icon is the crystal of the icon set, rendered by the app itself. `crates/gui/assets/` keeps the PNG files and `icon.ico`: the Windows executable embeds the `.ico` (`build.rs`), the web build uses it as its favicon, and the window shows the 256-pixel PNG. After changing the crystal, write them again:

```sh
cargo run -p gui -- --export-icon crates/gui/assets
```

A test fails while the files differ from the drawing.

## Tests

```sh
cargo test -p gui
```

The interface tests drive the app through `egui_kittest` (clicks, typing, shortcuts) without opening a window.
