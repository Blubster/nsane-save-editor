//! Save slots and the file names that go with them (`docs/FORMAT.md` §1).
//!
//! Both versions of the game keep one file per save slot, named after the
//! slot number: `MACK_<n>.sav` on the Switch, `ALCHEMYXSAVE_<n>.sav` on PC.
//! There are 12 slots, 4 per game: 0–3 for Crash Bandicoot, 4–7 for Crash 2
//! and 8–11 for Crash 3. A save exported from the Switch can be renamed
//! freely; it goes back under its `MACK_` name.

use std::ops::RangeInclusive;

use crate::Platform;

/// Every slot of the game.
pub const SLOTS: RangeInclusive<u32> = 0..=11;
/// The slots Crash 3: Warped lists.
pub const CRASH3_SLOTS: RangeInclusive<u32> = 8..=11;

impl Platform {
    /// What every save file name of this platform starts with.
    pub fn file_prefix(self) -> &'static str {
        match self {
            Platform::Switch => "MACK_",
            Platform::Pc => "ALCHEMYXSAVE_",
        }
    }

    /// The platform a save converts to.
    pub fn other(self) -> Platform {
        match self {
            Platform::Switch => Platform::Pc,
            Platform::Pc => Platform::Switch,
        }
    }
}

/// The game whose menu lists `slot`.
pub fn slot_game(slot: u32) -> Option<&'static str> {
    match slot {
        0..=3 => Some("Crash Bandicoot"),
        4..=7 => Some("Crash 2: Cortex Strikes Back"),
        8..=11 => Some("Crash 3: Warped"),
        _ => None,
    }
}

/// The name the game gives the file of `slot` on `platform`.
pub fn file_name(platform: Platform, slot: u32) -> String {
    format!("{}{slot}.sav", platform.file_prefix())
}

/// The platform and number of a file named like a save of the game, from
/// the last component of `name`: `MACK_8.sav`, `ALCHEMYXSAVE_8.sav`. Case is
/// ignored, as Windows file names ignore it. The number is written as the
/// game writes it (no sign, no leading zero) but may be outside [`SLOTS`].
pub fn parse_file_name(name: &str) -> Option<(Platform, u32)> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let lower = base.to_ascii_lowercase();
    [Platform::Switch, Platform::Pc]
        .into_iter()
        .find_map(|platform| {
            let prefix = platform.file_prefix().to_ascii_lowercase();
            let n = lower.strip_prefix(&prefix)?.strip_suffix(".sav")?;
            let canonical = !n.is_empty()
                && n.bytes().all(|b| b.is_ascii_digit())
                && (n == "0" || !n.starts_with('0'));
            Some((platform, n.parse().ok().filter(|_| canonical)?))
        })
}

/// The Crash 3 slot of a file named like a save of `platform`.
pub fn crash3_slot(name: &str, platform: Platform) -> Option<u32> {
    parse_file_name(name)
        .filter(|&(p, n)| p == platform && CRASH3_SLOTS.contains(&n))
        .map(|(_, n)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(file_name(Platform::Switch, 8), "MACK_8.sav");
        assert_eq!(file_name(Platform::Pc, 11), "ALCHEMYXSAVE_11.sav");
        for (name, expected) in [
            ("MACK_8.sav", Some((Platform::Switch, 8))),
            ("mack_0.SAV", Some((Platform::Switch, 0))),
            ("ALCHEMYXSAVE_8.sav", Some((Platform::Pc, 8))),
            ("ALCHEMYXSAVE_12.sav", Some((Platform::Pc, 12))),
            ("saves/ALCHEMYXSAVE_8.sav", Some((Platform::Pc, 8))),
            (r"C:\saves\MACK_10.sav", Some((Platform::Switch, 10))),
            ("ALCHEMYXSAVE_08.sav", None),
            ("ALCHEMYXSAVE_.sav", None),
            ("ALCHEMYXSAVE_8.sav.bak", None),
            ("ALCHEMYXSAVE_8a.sav", None),
            ("ALCHEMYXSAVE_-1.sav", None),
            ("ALCHEMYXSAVE_99999999999.sav", None),
            ("ALCHEMYXSAVE_8.sav/foo.sav", None),
            ("MACK_8", None),
            ("0019.sav", None),
            ("", None),
        ] {
            assert_eq!(parse_file_name(name), expected, "{name}");
        }
        assert_eq!(crash3_slot("MACK_9.sav", Platform::Switch), Some(9));
        assert_eq!(crash3_slot("MACK_9.sav", Platform::Pc), None);
        assert_eq!(crash3_slot("ALCHEMYXSAVE_2.sav", Platform::Pc), None);
    }

    #[test]
    fn games_of_the_slots() {
        assert_eq!(slot_game(3), Some("Crash Bandicoot"));
        assert_eq!(slot_game(4), Some("Crash 2: Cortex Strikes Back"));
        assert!(
            CRASH3_SLOTS
                .clone()
                .all(|n| slot_game(n) == Some("Crash 3: Warped"))
        );
        assert_eq!(slot_game(12), None);
        assert!(SLOTS.clone().all(|n| slot_game(n).is_some()));
    }
}
