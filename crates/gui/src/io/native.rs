//! Files on the desktop: dialogs, reading, and writing with a backup.

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::{Datelike as _, Local, Timelike as _};

/// Set when the file dialog library logs an error. It returns "no file" both
/// when the user cancels and when no dialog could be shown (no desktop portal
/// on a Linux session, for example); its log tells the two apart. On Linux it
/// logs the portal's error, then a warning that it falls back to `zenity`,
/// then an error only if that fails too: the warning clears the flag, so a
/// cancelled fallback dialog is a cancel.
static DIALOG_FAILED: AtomicBool = AtomicBool::new(false);

/// The dialog library's log target.
const DIALOG_TARGET: &str = "rfd";

/// Forwards to `env_logger` (`RUST_LOG`), and watches the dialog library's
/// errors and fallbacks whatever the log level.
struct Logger {
    inner: env_logger::Logger,
}

fn is_dialog_record(metadata: &log::Metadata<'_>) -> bool {
    metadata.level() <= log::Level::Warn && metadata.target().starts_with(DIALOG_TARGET)
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.inner.enabled(metadata) || is_dialog_record(metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        if is_dialog_record(record.metadata()) {
            match record.level() {
                log::Level::Error => DIALOG_FAILED.store(true, Ordering::Relaxed),
                _ if record.args().to_string().contains("fallback") => {
                    DIALOG_FAILED.store(false, Ordering::Relaxed)
                }
                _ => {}
            }
        }
        self.inner.log(record);
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// Installs the logger. Call once, first thing.
pub fn init_logger() {
    let inner = env_logger::Builder::from_default_env().build();
    log::set_max_level(inner.filter().max(log::LevelFilter::Warn));
    if log::set_boxed_logger(Box::new(Logger { inner })).is_err() {
        eprintln!("a logger was already installed");
    }
}

/// No file dialog could be shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogUnavailable;

/// Runs a dialog. `Ok(None)` when the user cancelled it.
fn dialog(show: impl FnOnce() -> Option<PathBuf>) -> Result<Option<PathBuf>, DialogUnavailable> {
    DIALOG_FAILED.store(false, Ordering::Relaxed);
    match show() {
        None if DIALOG_FAILED.load(Ordering::Relaxed) => Err(DialogUnavailable),
        picked => Ok(picked),
    }
}

fn file_dialog(start_dir: Option<&Path>) -> rfd::FileDialog {
    let dialog = rfd::FileDialog::new()
        .add_filter("Save files", &["sav"])
        .add_filter("All files", &["*"]);
    match start_dir {
        Some(dir) if dir.is_dir() => dialog.set_directory(dir),
        _ => dialog,
    }
}

/// Asks for a save to open, starting in `start_dir`.
pub fn pick_open(start_dir: Option<&Path>) -> Result<Option<PathBuf>, DialogUnavailable> {
    dialog(|| {
        file_dialog(start_dir)
            .set_title("Open a save file")
            .pick_file()
    })
}

/// Asks where to save, suggesting `current` (folder and name).
pub fn pick_save(current: Option<&Path>) -> Result<Option<PathBuf>, DialogUnavailable> {
    let dir = current.and_then(Path::parent);
    let name = current.and_then(Path::file_name);
    dialog(|| {
        let mut d = file_dialog(dir).set_title("Save as");
        if let Some(name) = name {
            d = d.set_file_name(name.to_string_lossy());
        }
        d.save_file()
    })
}

pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    fs::read(path)
}

/// The local time, as [`nsane::backup_path`] takes it.
pub fn local_now() -> (i32, u8, u8, u8, u8, u8) {
    let now = Local::now();
    // chrono keeps these within u8 range (month 1–12, second 0–60, …).
    let small = |v: u32| v as u8;
    (
        now.year(),
        small(now.month()),
        small(now.day()),
        small(now.hour()),
        small(now.minute()),
        small(now.second()),
    )
}

/// The backup name for `path` at `now`, numbered when a backup of the same
/// second already exists, so that no backup is ever overwritten.
pub fn free_backup_path(path: &Path, now: (i32, u8, u8, u8, u8, u8)) -> PathBuf {
    let first = nsane::backup_path(path, now);
    let stem = first.file_stem().unwrap_or_default().to_string_lossy();
    let mut candidate = first.clone();
    let mut n = 2;
    while candidate.exists() {
        candidate = first.with_file_name(format!("{stem}-{n}.bak"));
        n += 1;
    }
    candidate
}

/// Writes `bytes` to `path`. If a file is there, it is first copied to a
/// timestamped `.bak` next to it, whose path is returned. The new content
/// goes to a temporary file in the same folder, which then replaces the
/// target, so the target is never left half written.
pub fn write_with_backup(
    path: &Path,
    bytes: &[u8],
    now: (i32, u8, u8, u8, u8, u8),
) -> io::Result<Option<PathBuf>> {
    let backup = if path.exists() {
        let backup = free_backup_path(path, now);
        fs::copy(path, &backup)?;
        Some(backup)
    } else {
        None
    };
    write_atomic(path, bytes)?;
    Ok(backup)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "not a file path"))?;
    let tmp = path.with_file_name(format!(
        ".{}.{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = (|| {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::temp_dir;

    const NOW: (i32, u8, u8, u8, u8, u8) = (2026, 10, 11, 9, 8, 7);

    #[test]
    fn writes_without_backup_when_new() {
        let dir = temp_dir("new");
        let path = dir.join("0001.sav");
        assert_eq!(write_with_backup(&path, b"abc", NOW).unwrap(), None);
        assert_eq!(fs::read(&path).unwrap(), b"abc");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "no temporary left");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn backs_up_and_never_overwrites_a_backup() {
        let dir = temp_dir("backup");
        let path = dir.join("ALCHEMYXSAVE_8.sav");
        fs::write(&path, b"one").unwrap();

        let first = write_with_backup(&path, b"two", NOW).unwrap().unwrap();
        assert_eq!(first, dir.join("ALCHEMYXSAVE_8.20261011-090807.bak"));
        assert_eq!(fs::read(&first).unwrap(), b"one");
        assert_eq!(fs::read(&path).unwrap(), b"two");

        // same second again
        let second = write_with_backup(&path, b"three", NOW).unwrap().unwrap();
        assert_eq!(second, dir.join("ALCHEMYXSAVE_8.20261011-090807-2.bak"));
        assert_eq!(fs::read(&first).unwrap(), b"one");
        assert_eq!(fs::read(&second).unwrap(), b"two");
        assert_eq!(fs::read(&path).unwrap(), b"three");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 3);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_failed_write_leaves_the_target() {
        let dir = temp_dir("fail");
        // a folder where the file should go: the rename fails
        let path = dir.join("target.sav");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("inside"), b"x").unwrap();
        assert!(write_atomic(&path, b"abc").is_err());
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1, "temporary removed");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_dialog_that_could_not_show_is_told_from_a_cancel() {
        init_logger();
        assert_eq!(dialog(|| None), Ok(None), "cancelled");
        let failed = dialog(|| {
            log::error!(target: "rfd::backend::xdg_desktop_portal", "OpenFile failed");
            None
        });
        assert_eq!(failed, Err(DialogUnavailable));
        assert_eq!(dialog(|| None), Ok(None), "the next one starts afresh");
        // the portal fails, the fallback dialog shows and is cancelled
        let cancelled = dialog(|| {
            log::error!(target: "rfd::backend::xdg_desktop_portal::portal", "OpenFile failed");
            log::warn!(target: "rfd::backend::xdg_desktop_portal", "Using zenity fallback");
            None
        });
        assert_eq!(cancelled, Ok(None));
        // ... or the fallback fails too
        let failed = dialog(|| {
            log::error!(target: "rfd::backend::xdg_desktop_portal::portal", "OpenFile failed");
            log::warn!(target: "rfd::backend::xdg_desktop_portal", "Using zenity fallback");
            log::error!(target: "rfd::backend::xdg_desktop_portal", "Failed to pick file with zenity");
            None
        });
        assert_eq!(failed, Err(DialogUnavailable));
        // other errors don't count
        let other = dialog(|| {
            log::error!(target: "eframe", "unrelated");
            None
        });
        assert_eq!(other, Ok(None));
        let picked = dialog(|| Some(PathBuf::from("a.sav")));
        assert_eq!(picked, Ok(Some(PathBuf::from("a.sav"))));
    }

    #[test]
    fn local_now_is_plausible() {
        let (year, month, day, hour, minute, second) = local_now();
        assert!(year >= 2024);
        assert!((1..=12).contains(&month) && (1..=31).contains(&day));
        assert!(hour < 24 && minute < 60 && second <= 60);
    }
}
