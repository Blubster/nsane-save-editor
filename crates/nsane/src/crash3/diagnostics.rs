//! Diagnostics: states of a save worth telling the user about, each with an
//! optional fix expressed as [`Edit`]s. Most of them only come from editing,
//! by this editor or another tool. The GUI lists them and applies a fix as one
//! undo step labelled with [`Fix::label`].

use std::cmp::Reverse;

use super::edit::Edit;
use super::model::{Crash3Save, Warning};
use super::presets::matching_time;
use super::tables::{BOSSES, Kind, LevelId, Relic, relic_counted_key};
use super::time::{format_time, relic_for_time};
use crate::{Platform, SizeReport};

/// Above this share of a Switch file, the size is worth mentioning.
const SWITCH_SIZE_INFO_PERCENT: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Harmless or cosmetic.
    Info,
    /// The game may behave oddly with the save.
    Warning,
    /// The file can't be written.
    Error,
}

/// Edits that resolve a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    /// The button text, and the undo label once applied.
    pub label: String,
    pub edits: Vec<Edit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// A stable identifier, such as `portal_glitch`.
    pub code: &'static str,
    /// The level or boss concerned, if any.
    pub level: Option<LevelId>,
    pub message: String,
    pub fix: Option<Fix>,
}

impl Diagnostic {
    fn new(
        severity: Severity,
        code: &'static str,
        level: Option<LevelId>,
        message: String,
    ) -> Self {
        Self {
            severity,
            code,
            level,
            message,
            fix: None,
        }
    }

    fn with_fix(mut self, label: String, edits: Vec<Edit>) -> Self {
        self.fix = Some(Fix { label, edits });
        self
    }

    /// A fix of one edit, labelled as the edit history labels it.
    fn with_edit(self, save: &Crash3Save, edit: Edit) -> Self {
        let label = edit.describe(save);
        self.with_fix(label, vec![edit])
    }
}

/// Everything worth reporting about `save`, most severe first. Within a
/// severity: the file size and name, the save as a whole, then each level in
/// [`LEVELS`](super::LEVELS) order. `file_name` may be a path; only its last
/// component is checked.
pub fn diagnostics(
    save: &Crash3Save,
    platform: Platform,
    size: SizeReport,
    file_name: Option<&str>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    out.extend(switch_size(size));
    if platform == Platform::Pc {
        out.extend(file_name.and_then(pc_file_name));
    }
    out.extend(save.warnings().into_iter().map(|w| warning(save, w)));
    if save.bonus_gem_due() {
        out.push(bonus_gem_due(save));
    }
    for level in LevelId::all() {
        level_diagnostics(save, level, &mut out);
    }
    out.sort_by_key(|d| Reverse(d.severity));
    out
}

/// `153600` → `153,600`.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A Switch file has a fixed size (`docs/FORMAT.md` §1). An edit that
/// outgrows it is kept, but the file can't be written until it is undone.
fn switch_size(size: SizeReport) -> Option<Diagnostic> {
    let capacity = size.capacity?;
    let (used, cap) = (thousands(size.used), thousands(capacity));
    if !size.fits() {
        return Some(Diagnostic::new(
            Severity::Error,
            "switch_size",
            None,
            format!(
                "The save data needs {used} bytes but a Switch save holds {cap}. Undo or remove \
                 changes to save the file."
            ),
        ));
    }
    let percent = size.used * 100 / capacity;
    (percent > SWITCH_SIZE_INFO_PERCENT).then(|| {
        Diagnostic::new(
            Severity::Info,
            "switch_size",
            None,
            format!("The save data uses {used} of the {cap} bytes of a Switch save ({percent}%)."),
        )
    })
}

/// Whether the last component of `name` is `ALCHEMYXSAVE_<n>.sav`. Case is
/// ignored, as Windows file names ignore it.
pub fn is_pc_save_name(name: &str) -> bool {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.to_ascii_lowercase()
        .strip_prefix("alchemyxsave_")
        .and_then(|rest| rest.strip_suffix(".sav"))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn pc_file_name(name: &str) -> Option<Diagnostic> {
    (!is_pc_save_name(name)).then(|| {
        Diagnostic::new(
            Severity::Info,
            "pc_file_name",
            None,
            "The PC game only loads files named ALCHEMYXSAVE_<n>.sav. Rename the file before \
             copying it to the game's save folder."
                .to_owned(),
        )
    })
}

fn warning(save: &Crash3Save, w: Warning) -> Diagnostic {
    match w {
        Warning::PortalGlitch { boss, .. } => {
            let d = Diagnostic::new(Severity::Warning, w.code(), Some(boss), w.message());
            match defeat_bosses_fix(save) {
                Some((label, edits)) => d.with_fix(label, edits),
                None => d,
            }
        }
    }
}

/// Defeats the bosses still alive, in order, until the crystals no longer
/// exceed the portal count of the next one (5 × its room, `docs/FORMAT.md`
/// §7) or none is left. Story scenes are left alone.
fn defeat_bosses_fix(save: &Crash3Save) -> Option<(String, Vec<Edit>)> {
    let crystals = save.totals().crystals;
    let mut bosses = Vec::new();
    for (room, &boss) in (1u32..).zip(BOSSES.iter()) {
        if save.boss_defeated(boss) {
            continue;
        }
        if crystals <= 5 * room {
            break;
        }
        bosses.push(boss);
    }
    let label = match bosses[..] {
        [] => return None,
        [boss] => format!("Defeat {}", boss.name()),
        [.., last] => format!("Defeat bosses up to {}", last.name()),
    };
    let edits = bosses
        .into_iter()
        .map(|boss| Edit::SetBossDefeated {
            boss,
            on: true,
            skip_cutscenes: false,
        })
        .collect();
    Some((label, edits))
}

/// The hub gives the bonus gem on the next visit (`docs/FORMAT.md` §7), so
/// this only says what will happen.
fn bonus_gem_due(save: &Crash3Save) -> Diagnostic {
    let t = save.totals();
    Diagnostic::new(
        Severity::Info,
        "bonus_gem_due",
        None,
        format!(
            "The hub will give the bonus gem on the next visit: {} gold or platinum relics and \
             {}%. This is harmless.",
            t.gold + t.platinum,
            t.percent()
        ),
    )
    .with_fix(
        "Give the bonus gem now".to_owned(),
        vec![Edit::SetBonusGem { on: true }],
    )
}

fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// The relic one set of relic bits stands for: `Ok(None)` for none, `Err`
/// for several.
fn relic_of_bits(flags: u32) -> Result<Option<Relic>, ()> {
    let set: Vec<Relic> = Relic::ALL
        .into_iter()
        .filter(|r| flags & r.bit() != 0)
        .collect();
    match set[..] {
        [] => Ok(None),
        [relic] => Ok(Some(relic)),
        _ => Err(()),
    }
}

fn level_diagnostics(save: &Crash3Save, level: LevelId, out: &mut Vec<Diagnostic>) {
    let lv = level.level();
    let name = level.name();
    let at = Some(level);
    let relic = save.relic(level);
    let times = save.times(level);

    // The game doesn't check times against relics (`docs/FORMAT.md` §4, T1),
    // so a mismatch only shows on the leaderboard.
    if let Some(relic) = relic.filter(|_| lv.has_relic()) {
        let best = times.first().copied();
        let earned = best.and_then(|t| relic_for_time(level, t));
        if earned != Some(relic) {
            let tier = relic.key();
            let message = match (best, earned) {
                (None, _) => format!("{name} has a {tier} relic but no time trial time."),
                (Some(t), None) => format!(
                    "{name} has a {tier} relic but its best time, {}, earns no relic.",
                    format_time(t)
                ),
                (Some(t), Some(e)) => format!(
                    "{name} has a {tier} relic but its best time, {}, earns {}.",
                    format_time(t),
                    e.key()
                ),
            };
            let mut d = Diagnostic::new(
                Severity::Info,
                "relic_time_mismatch",
                at,
                format!("{message} The game doesn't check this."),
            );
            if let Some(t) = matching_time(level, relic) {
                d = d.with_fix(
                    format!("{name}: fill a matching time ({})", format_time(t)),
                    vec![Edit::SetTimes {
                        level,
                        times_cs: vec![t],
                    }],
                );
            }
            out.push(d);
        }
    }

    // The game keeps the bit and the field equal (`docs/FORMAT.md` §4).
    let crystal = save.crystal(level);
    if crystal != save.crystal_field(level) {
        let mut d = Diagnostic::new(
            Severity::Warning,
            "crystal_field_mismatch",
            at,
            format!(
                "{name}: the crystal bit is {} but the crystal field is {}. The game keeps them \
                 equal.",
                on_off(crystal),
                on_off(!crystal)
            ),
        );
        if lv.has_crystal() {
            d = d.with_edit(save, Edit::SetCrystal { level, on: crystal });
        }
        out.push(d);
    }

    // One relic bit, matching `relic_tier` (`docs/FORMAT.md` §4).
    let tier = save.relic_tier(level);
    let message = match relic_of_bits(save.flags(level)) {
        Err(()) => Some(format!(
            "{name} has several relic bits set. The game sets one at a time."
        )),
        Ok(bits) if bits.map_or(0, Relic::tier) != tier => Some(format!(
            "{name}: the relic tier says {} but the relic bits say {}. The game keeps them \
             equal.",
            match (relic, tier) {
                (Some(r), _) => r.key().to_owned(),
                (None, 0) => "no relic".to_owned(),
                (None, _) => format!("an unknown value ({tier})"),
            },
            bits.map_or("no relic", Relic::key)
        )),
        Ok(_) => None,
    };
    if let Some(message) = message {
        let mut d = Diagnostic::new(Severity::Warning, "relic_field_mismatch", at, message);
        if lv.has_relic() {
            d = d.with_edit(save, Edit::SetRelic { level, relic });
        }
        out.push(d);
    }

    // The flag is present while the level holds a relic (`docs/FORMAT.md` §5).
    if lv.has_relic() {
        let counted = save.flag(&relic_counted_key(lv));
        if counted != relic.is_some() {
            let message = match relic {
                Some(r) => format!(
                    "{name} has a {} relic but no relic counted flag. The game sets both.",
                    r.key()
                ),
                None => {
                    format!("{name} has a relic counted flag but no relic. The game removes both.")
                }
            };
            out.push(
                Diagnostic::new(Severity::Warning, "relic_counted_mismatch", at, message)
                    .with_edit(save, Edit::SetRelic { level, relic }),
            );
        }
    }

    // The zone bit and the `Gem_<Color>` variable go together (`docs/FORMAT.md` §8).
    if let Some(color) = lv.colored_gem {
        let gem = save.colored_gem(level);
        if gem != save.colored_gem_variable(color) {
            out.push(
                Diagnostic::new(
                    Severity::Warning,
                    "colored_gem_mismatch",
                    at,
                    format!(
                        "{name}: the {} gem is {} but its {} variable is {}. The game sets both.",
                        color.key(),
                        on_off(gem),
                        color.variable(),
                        on_off(!gem)
                    ),
                )
                .with_edit(save, Edit::SetColoredGem { level, on: gem }),
            );
        }
    }

    // The game marks a level entered on the first visit (`docs/FORMAT.md` §4,
    // §8), so it never writes progress on a level that wasn't.
    let progress = save.flags(level) != 0 || tier != 0 || !times.is_empty();
    if progress && !save.entered(level) {
        out.push(
            Diagnostic::new(
                Severity::Warning,
                "progress_not_entered",
                at,
                format!(
                    "{name} holds progress but is marked as never entered, which the game \
                     doesn't write."
                ),
            )
            .with_edit(save, Edit::MarkEntered { level }),
        );
    }

    let unknown = save.flags_unknown(level);
    if unknown != 0 {
        out.push(Diagnostic::new(
            Severity::Info,
            "unknown_flag_bits",
            at,
            format!(
                "{name} has collectible bits the editor doesn't know: {unknown:#06x}. They are \
                 kept as they are."
            ),
        ));
    }

    // Only levels 1–25 have a crystal, and the crystal edit refuses the others,
    // so only another tool writes this. The totals count it.
    if crystal && !lv.has_crystal() {
        let what = match lv.kind {
            Kind::Boss => "a boss",
            _ => "a level without a crystal",
        };
        out.push(Diagnostic::new(
            Severity::Warning,
            "crystal_on_level_without_crystal",
            at,
            format!("{name} is {what} but its crystal bit is set."),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::super::tables::bit;
    use super::*;

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(153_600), "153,600");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }

    #[test]
    fn pc_save_names() {
        for ok in [
            "ALCHEMYXSAVE_8.sav",
            "ALCHEMYXSAVE_12.sav",
            "alchemyxsave_0.SAV",
            "saves/ALCHEMYXSAVE_8.sav",
            r"C:\saves\ALCHEMYXSAVE_8.sav",
        ] {
            assert!(is_pc_save_name(ok), "{ok}");
        }
        for bad in [
            "foo.sav",
            "ALCHEMYXSAVE_.sav",
            "ALCHEMYXSAVE_8.sav.bak",
            "ALCHEMYXSAVE_8a.sav",
            "ALCHEMYXSAVE_-1.sav",
            "ALCHEMYXSAVE_8.sav/foo.sav",
            "",
        ] {
            assert!(!is_pc_save_name(bad), "{bad}");
        }
    }

    #[test]
    fn relic_bits() {
        assert_eq!(relic_of_bits(0), Ok(None));
        assert_eq!(
            relic_of_bits(bit::RELIC_GOLD | bit::CRYSTAL),
            Ok(Some(Relic::Gold))
        );
        assert_eq!(
            relic_of_bits(bit::RELIC_GOLD | bit::RELIC_PLATINUM),
            Err(())
        );
    }
}
