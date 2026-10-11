# Vendored crates

## winit

`winit/` is winit 0.30.13 as published on crates.io, with one change:
[`winit-dpi.patch`](winit-dpi.patch), a backport of
[rust-windowing/winit#4341](https://github.com/rust-windowing/winit/pull/4341).

Without it, dragging the window on Windows 11 from one screen to another with
a different scale (125 % and 100 %, say) doesn't work. The scale flips back and
forth, the window grows at each flip, and it stays on the first screen
([rust-windowing/winit#3040](https://github.com/rust-windowing/winit/issues/3040)).
The fix is in winit 0.31, but eframe 0.36 uses winit 0.30.

The workspace `Cargo.toml` swaps it in with `[patch.crates-io]`. Remove both
the patch entry and this folder once eframe uses a winit release with the fix.
To check that the copy holds only the patch:

```sh
curl -L https://crates.io/api/v1/crates/winit/0.30.13/download | tar xz
diff -ru winit-0.30.13 vendor/winit
```
