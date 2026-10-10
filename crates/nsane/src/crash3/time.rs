//! Time trial times: integer centiseconds (6023 = 1:00.23), written `m:ss.cc`.

use thiserror::Error;

use super::tables::{LevelId, Relic, relic_times};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid time {0:?}: expected m:ss.cc, ss.cc or a number of centiseconds followed by cs")]
pub struct ParseTimeError(pub String);

pub fn format_time(cs: u32) -> String {
    format!("{}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

fn digits(text: &str, max_len: usize) -> Option<u32> {
    if text.is_empty() || text.len() > max_len || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Parses `<digits>cs`, `M:SS.CC`, `M:SS:CC` (as the leaderboard writes it),
/// `SS.CC` or `SS:CC`. Seconds are 0–59; one cents digit means tenths
/// (`1:00.2` is 1:00.20). Surrounding whitespace is ignored; signs, inner
/// spaces and any other character are rejected.
pub fn parse_time(text: &str) -> Result<u32, ParseTimeError> {
    let err = || ParseTimeError(text.to_owned());
    let t = text.trim();
    if let Some(cs) = t.strip_suffix("cs") {
        return digits(cs, usize::MAX).ok_or_else(err);
    }
    let (minutes, rest) = match t.split_once(':') {
        Some((m, rest)) if rest.contains([':', '.']) => (digits(m, usize::MAX), rest),
        _ => (Some(0), t),
    };
    let (seconds, cents) = rest.split_once([':', '.']).ok_or_else(err)?;
    let minutes = minutes.ok_or_else(err)?;
    let seconds = digits(seconds, 2).filter(|&s| s < 60).ok_or_else(err)?;
    let cents = digits(cents, 2).ok_or_else(err)? * if cents.len() == 1 { 10 } else { 1 };
    minutes
        .checked_mul(6000)
        .and_then(|m| m.checked_add(seconds * 100 + cents))
        .ok_or_else(err)
}

/// The best relic a time earns: the first threshold it is under. `None` if
/// it is slower than sapphire, or for a boss (no time trial).
pub fn relic_for_time(level: LevelId, cs: u32) -> Option<Relic> {
    let times = relic_times(level)?;
    [Relic::Platinum, Relic::Gold, Relic::Sapphire]
        .into_iter()
        .find(|&r| cs < times.threshold(r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_time_accepts_the_documented_forms() {
        for (text, cs) in [
            ("1:00:23", 6023),
            ("0:55.44", 5544),
            ("55.44", 5544),
            ("55:44", 5544),
            ("1:00", 100),
            ("6023cs", 6023),
            (" 1:00.2 ", 6020),
            ("0:00.00", 0),
            ("12:59.99", 12 * 6000 + 5999),
        ] {
            assert_eq!(parse_time(text), Ok(cs), "{text:?}");
        }
    }

    #[test]
    fn parse_time_rejects_everything_else() {
        for text in [
            "abc",
            "1:2:3:4",
            "",
            "1:-5.00",
            "1:75.00",
            "1:00.234",
            "1_0:00.00",
            "1.00.23",
            "100",
            "cs",
            "-5cs",
            "1 :00.00",
            "1:00. 2",
            "+1:00.00",
            "1:00.",
            "99999999999:00.00",
            "99999999999cs",
        ] {
            assert!(parse_time(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn format_time_pads_seconds_and_cents() {
        assert_eq!(format_time(6023), "1:00.23");
        assert_eq!(format_time(5544), "0:55.44");
        assert_eq!(format_time(5), "0:00.05");
        assert_eq!(format_time(60 * 6000), "60:00.00");
    }

    #[test]
    fn every_formatted_time_parses_back() {
        for cs in (0..20_000).step_by(7) {
            assert_eq!(parse_time(&format_time(cs)), Ok(cs));
        }
    }
}
