# Save editor for Crash Bandicoot N. Sane Trilogy

A save editor for **Crash Bandicoot N. Sane Trilogy**, on Nintendo Switch and PC. It edits **Crash Bandicoot 3: Warped** progress: crystals, gems, relics and time trial times, warp rooms, super powers, story scenes and lives.

It runs on Windows and Linux as a single program with nothing to install, and in a web browser. Everything happens on your computer: no account, no upload, no network connection.

> **Keep a backup of your saves.** The editor writes saves the way the game does and checks them before writing, and on the desktop it copies a file before replacing it. Still, keep your own copy of a save before editing it.

## Supported files

| Platform | File | Notes |
|---|---|---|
| Nintendo Switch | `MACK_<n>.sav` | Export your save with your usual save manager, edit the file, then import it back. Switch saves are always 153,600 bytes. |
| PC | `ALCHEMYXSAVE_<n>.sav` | Edit the file in the folder where the game keeps its saves. |

`<n>` is the save slot: 0–3 for Crash Bandicoot, 4–7 for Crash Bandicoot 2 and **8–11 for Crash Bandicoot 3**. Every file holds the data of all three games. Only Crash 3 can be edited for now; the other games' data is written back unchanged.

## Features

- **Levels.** Turn each level's crystal, clear gem, second gem and colored gem on or off, set its relic (sapphire, gold, platinum) and its three time trial times. See whether a level was entered and its completion counter.
- **Warp rooms.** Unlock warp rooms, even early, by defeating their bosses, optionally with the boss's super power and with its story scenes marked as seen. Reach the relic counts that open the secret warp room's levels.
- **Super powers, story scenes and lives.** Turn each super power on or off, mark story scenes as seen or not, set lives from 0 to 255 (the game keeps any of them, but only adds lives picked up below 99).
- **Presets.** Complete everything (108%), the story up to a warp room, every relic at a tier, every super power, every story scene, or reset a level. Each shows a preview of the changes and the resulting totals first.
- **Diagnostics.** Warnings about states the game handles badly (for example a boss portal that can't be entered because of extra crystals), relics that don't match their times, and a Switch save that would no longer fit its file. Most come with a fix.
- **Safe editing.** Undo and redo, a history of every step, a review of the changes before saving, and on the desktop a timestamped backup of the file being replaced.
- **Switch ↔ PC conversion.** Converts a save to the other platform by copying its game data into a save from that platform: one of your own, or the new-game save built into the editor. Converted saves were checked in game both ways (Switch to PC and PC to Switch).

The percentage, the totals and the other values the game shows on the save slot screen are recomputed on every save.

### Known limitations

- Crash Bandicoot and Crash Bandicoot 2 progress can't be edited yet. Their tabs are shown disabled.
- The save doesn't store play counts, deaths, boxes or play time, so the editor can't show them.

## Download and run

Release builds, when there are any, are on the repository's Releases page:

- **Windows:** `nsane-save-editor.exe`. Run it, there is nothing to install. Windows may warn that the program is not signed.
- **Linux:** `nsane-save-editor`, a single x86-64 binary. It needs an X11 or Wayland desktop session and a Vulkan or OpenGL driver (Mesa's software renderer works too). File dialogs go through the desktop portal (`xdg-desktop-portal`), or `zenity` where no portal runs. Without either, open saves by dropping them onto the window or with `--open FILE.sav`.
- **Web:** the static web build runs in any recent browser. The save is read and edited in the page, and saving downloads it. Nothing is uploaded.

Usage, settings, keyboard shortcuts and command-line options are described in [crates/gui/README.md](crates/gui/README.md).

## Building from source

You need [Rust](https://rustup.rs) (stable).

```sh
cargo build --release -p gui        # target/release/nsane-save-editor
cargo run -p gui -- --open MACK_8.sav
```

**Windows.** Install Rust with `rustup-init.exe` (the default MSVC toolchain), the Visual Studio Build Tools with the "Desktop development with C++" workload (`rustup-init` offers to install them) and git. Then, from a clone:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1
```

The script pulls the latest commits (`-NoPull` skips that), builds the release and prints the path of the `.exe`, which embeds the app icon and the C runtime (`.cargo/config.toml`).

**Web.** Install [Trunk](https://trunkrs.dev) and the WebAssembly target:

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cd crates/gui
trunk serve                                              # http://127.0.0.1:8080
trunk build --release                                    # static files in crates/gui/dist
trunk build --release --public-url /nsane-save-editor/   # to serve them from a sub-path
```

**Checks.** `scripts/check.sh` runs formatting, lints, every test and the WebAssembly build.

## How it works

| Crate | What it does |
|---|---|
| `crates/igb` | Reads and writes the save's container, byte for byte. No game knowledge. |
| `crates/nsane` | The game layer: the Crash 3 model, edits, presets, diagnostics, undo history and the platform conversion. |
| `crates/gui` | The desktop and web app (egui). |

The save format, with the evidence behind each rule, is documented in [docs/FORMAT.md](docs/FORMAT.md). The tests run the editor on the saves in [tests/saves/](tests/saves/README.md): saves the game wrote, saves that were edited and then loaded in game, and the game's saves after those tests. The edited saves that a recipe of edits rebuilds byte for byte are rebuilt by the tests instead of being kept as files.

## License

[MIT](LICENSE).

The interface is set in [Inter](https://rsms.me/inter/), by the Inter Project Authors, under the SIL Open Font License 1.1 ([crates/gui/assets/fonts/OFL.txt](crates/gui/assets/fonts/OFL.txt)).

Fan-made tool, not affiliated with or endorsed by Activision or Vicarious Visions. Crash Bandicoot is a trademark of Activision.
