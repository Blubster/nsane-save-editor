# nsane-save-editor (GUI)

The desktop and web app. It runs natively on Windows and Linux (eframe), and in a browser (WebAssembly, built with Trunk).

## Running

```sh
cargo run -p gui                        # empty window
cargo run -p gui -- --open saves/0019.sav
```

Open a save with the **Open** button, by dropping the file onto the window, or with `--open`. Saving over a file first copies it to `<name>.<YYYYMMDD-HHMMSS>.bak` next to it.

### Editing Crash 3

The Crash 3 tab shows the overall progress on the left (percentage, totals, lives, the bonus gem, the secret warp room's relic thresholds, the story and any problems found in the save) and the six warp rooms in the middle. In a room, each level is a row of icons: crystal, clear gem, second gem and colored gem, where the level has them, then the relic and the best time trial time. Click an icon to add or remove it. A click on the relic moves to the next tier (none, sapphire, gold, platinum); a right-click picks one. Every click is one step that Ctrl+Z undoes. Hover over an icon to see what it is and what a click does. A click on a level's name selects it.

### In a browser

```sh
cd crates/gui
trunk serve                 # http://127.0.0.1:8080
trunk build --release       # static files in crates/gui/dist
```

The file never leaves the browser. Open a save with **Open** or by dropping it onto the page. **Save** downloads the edited save under its name, and **Save as** asks for another name first; the opened file is left as it was, so there is no backup to make. The browser may add a number to a downloaded name that already exists (`ALCHEMYXSAVE_8 (1).sav`): the PC game only reads `ALCHEMYXSAVE_<n>.sav`, so rename it. While there are unsaved changes, the browser asks before the tab is closed or reloaded.

### Settings

The app remembers the theme picked with the toolbar toggle, which side panels are shown on a narrow window and, on the desktop, the folder of the last file opened or saved. They are kept in:

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
| `--select LEVEL` | Select a level by its key (`toadvillage`, `deeptrouble`, …). |
| `--view NAME` | Set up a state at start: `unsaved` (the question asked before unsaved changes are lost), `invalid-time` (a refused time typed for the selected level), `progress` (the Progress tab), `unlock-dialog` (its dialog for the first locked warp room), `cutscenes` (the Progress tab scrolled to the story scenes). |
| `--theme light\|dark` | Force a theme instead of following the system. |
| `--size WxH` | Inner window size in points, for example `1280x800`. |
| `--maximized` | Start maximized. |
| `--icon-gallery` | Show every icon in every state and size instead of the editor. |
| `--screenshot FILE.png` | Render a few frames, capture the window to a PNG, then quit. |

### Screenshots

`--screenshot` makes the app usable from scripts and tests: it applies the other options, waits until the window has settled (a few frames and 1.5 s), writes the PNG and exits. It works in debug and release builds.

```sh
nsane-save-editor --size 1280x800 --theme light --screenshot empty.png
nsane-save-editor --open saves/0019.sav --select toadvillage --screenshot level.png
```

`--screenshot` can be repeated to capture several files from one window. Each capture uses the `--theme` given before it:

```sh
nsane-save-editor --open saves/0019.sav \
    --theme light --screenshot light.png \
    --theme dark --screenshot dark.png
```

The capture is the window's own rendering, at the display's scale, without the window frame.

## Tests

```sh
cargo test -p gui
```

The interface tests drive the app through `egui_kittest` (clicks, typing, shortcuts) without opening a window.
