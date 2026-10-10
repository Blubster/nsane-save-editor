//! Where the copy of a file about to be overwritten goes.

use std::path::{Path, PathBuf};

/// `<dir>/<stem>.<YYYYMMDD-HHMMSS>.bak` for `<dir>/<stem>.<ext>`, with `now`
/// as `(year, month, day, hour, minute, second)` in local time.
///
/// The extension is `.bak` so that the PC game, which loads
/// `ALCHEMYXSAVE_<n>.sav`, ignores backups left in its save folder.
pub fn backup_path(path: &Path, now: (i32, u8, u8, u8, u8, u8)) -> PathBuf {
    let (year, month, day, hour, minute, second) = now;
    let mut name = path.file_stem().unwrap_or("save".as_ref()).to_os_string();
    name.push(format!(
        ".{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}.bak"
    ));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_names() {
        let now = (2026, 10, 10, 20, 15, 0);
        assert_eq!(
            backup_path(Path::new("dir/ALCHEMYXSAVE_8.sav"), now),
            Path::new("dir/ALCHEMYXSAVE_8.20261010-201500.bak")
        );
        assert_eq!(
            backup_path(Path::new("0019.sav"), (2027, 1, 2, 3, 4, 5)),
            Path::new("0019.20270102-030405.bak")
        );
        assert_eq!(
            backup_path(Path::new("/saves/no_extension"), now),
            Path::new("/saves/no_extension.20261010-201500.bak")
        );
    }
}
