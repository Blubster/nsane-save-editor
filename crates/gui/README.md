# nsane-save-editor (GUI)

The desktop and web app. It runs natively on Windows and Linux (eframe), and in a browser (WebAssembly, built with Trunk).

## Running

```sh
cargo run -p gui                        # empty window
cargo run -p gui -- --open saves/0019.sav
```

Open a save with the **Open** button, by dropping the file onto the window, or with `--open`. Saving over a file first copies it to `<name>.<YYYYMMDD-HHMMSS>.bak` next to it.

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

The file dialogs go through the desktop portal (`xdg-desktop-portal`) over D-Bus. Where no portal is running (some minimal or WSL setups), the dialog can't open and the app says so: open files by dropping them onto the window, or with `--open`.

## Command-line options (native)

| Option | Effect |
|---|---|
| `--open FILE.sav` | Open a save at start. |
| `--select LEVEL` | Select a level by its key (`toadvillage`, `deeptrouble`, …). |
| `--view NAME` | Show a dialog at start: `unsaved` (the question asked before unsaved changes are lost). |
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
