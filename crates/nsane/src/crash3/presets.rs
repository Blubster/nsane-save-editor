//! Presets: common goals ("108%", "story up to warp room 3") computed as a
//! list of [`Edit`]s against the save as it is now. The GUI previews the list
//! and applies it with one [`Document::apply`](crate::Document::apply), so a
//! preset is one undo step.
//!
//! Each preset emits its edits in a fixed, documented order. The order is part
//! of the result: inserting flags in another order can place them in other
//! hash table slots, and so give other bytes (`docs/FORMAT.md` §2).

use super::edit::{Edit, EditError};
use super::model::Crash3Save;
use super::tables::{ABILITIES, BOSSES, CUTSCENES, Kind, LevelId, Relic, SECRET_ROOM, relic_times};
use super::time::relic_for_time;

/// What a preset that sets relics does to the time trial times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimePolicy {
    /// Leave the times as they are.
    Keep,
    /// Write one time just under the relic's threshold
    /// ([`matching_time`]), unless the best time already earns that relic or
    /// a better one.
    #[default]
    Matching,
    /// Write this one time, in centiseconds.
    Fixed(u32),
    /// Remove every time.
    Clear,
}

/// The time 1 centisecond under `relic`'s threshold: the slowest time that
/// earns it. Platinum on Toad Village is 0:44.02. `None` for a boss.
pub fn matching_time(level: LevelId, relic: Relic) -> Option<u32> {
    Some(relic_times(level)?.threshold(relic) - 1)
}

/// The times edit for a level that gets `relic`, if the policy writes one.
fn times_edit(save: &Crash3Save, level: LevelId, relic: Relic, policy: TimePolicy) -> Option<Edit> {
    let times_cs = match policy {
        TimePolicy::Keep => return None,
        TimePolicy::Matching => {
            let best = save.times(level).first().copied();
            if best.and_then(|t| relic_for_time(level, t)) >= Some(relic) {
                return None;
            }
            vec![matching_time(level, relic)?]
        }
        TimePolicy::Fixed(cs) => vec![cs],
        TimePolicy::Clear => Vec::new(),
    };
    Some(Edit::SetTimes { level, times_cs })
}

/// The relic edit, then the times edit.
fn relic_edits(save: &Crash3Save, level: LevelId, relic: Relic, policy: TimePolicy) -> Vec<Edit> {
    let set = Edit::SetRelic {
        level,
        relic: Some(relic),
    };
    std::iter::once(set)
        .chain(times_edit(save, level, relic, policy))
        .collect()
}

/// Every level that isn't a boss, in [`LEVELS`](super::LEVELS) order.
fn levels() -> impl Iterator<Item = LevelId> {
    LevelId::all().filter(|l| l.level().kind != Kind::Boss)
}

/// The collectible edits of one level, in the order crystal, clear gem,
/// second gem, colored gem, skipping those the level doesn't have.
fn collectible_edits(level: LevelId, with_gems: bool) -> Vec<Edit> {
    let lv = level.level();
    let on = true;
    [
        lv.has_crystal().then_some(Edit::SetCrystal { level, on }),
        with_gems.then_some(Edit::SetGem { level, on }),
        (with_gems && lv.second_gem).then_some(Edit::SetSecondGem { level, on }),
        (with_gems && lv.colored_gem.is_some()).then_some(Edit::SetColoredGem { level, on }),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn abilities(n: usize, on: bool) -> impl Iterator<Item = Edit> {
    ABILITIES[..n]
        .iter()
        .map(move |&ability| Edit::SetAbility { ability, on })
}

fn bosses(n: usize, skip_cutscenes: bool) -> impl Iterator<Item = Edit> {
    BOSSES[..n].iter().map(move |&boss| Edit::SetBossDefeated {
        boss,
        on: true,
        skip_cutscenes,
    })
}

// ------------------------------------------------------------ complete all

/// Options of [`complete_all`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompleteAll {
    pub times: TimePolicy,
    pub skip_cutscenes: bool,
}

impl Default for CompleteAll {
    fn default() -> Self {
        Self {
            times: TimePolicy::Matching,
            skip_cutscenes: true,
        }
    }
}

/// 108%. In order:
///
/// 1. each level in [`LEVELS`](super::LEVELS) order: crystal (levels 1–25), clear gem,
///    second gem and colored gem where the level has them, platinum relic,
///    then times per the policy;
/// 2. each boss defeated, with or without its story scenes;
/// 3. each ability on;
/// 4. the bonus gem.
pub fn complete_all(save: &Crash3Save, options: &CompleteAll) -> Vec<Edit> {
    let mut edits = Vec::new();
    for level in levels() {
        edits.extend(collectible_edits(level, true));
        edits.extend(relic_edits(save, level, Relic::Platinum, options.times));
    }
    edits.extend(bosses(BOSSES.len(), options.skip_cutscenes));
    edits.extend(abilities(ABILITIES.len(), true));
    edits.push(Edit::SetBonusGem { on: true });
    edits
}

// -------------------------------------------------------------- story up to

/// Options of [`story_up_to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoryUpTo {
    /// Also the clear, second and colored gems of those rooms.
    pub with_gems: bool,
    /// The abilities the defeated bosses grant.
    pub with_abilities: bool,
    pub skip_cutscenes: bool,
}

impl Default for StoryUpTo {
    fn default() -> Self {
        Self {
            with_gems: false,
            with_abilities: true,
            skip_cutscenes: true,
        }
    }
}

/// The warp room [`story_up_to`] starts at: that of the first boss still
/// alive, or 5 when all are defeated.
pub fn default_story_room(save: &Crash3Save) -> u8 {
    (1u8..)
        .zip(BOSSES)
        .find(|&(_, boss)| !save.boss_defeated(boss))
        .map_or(5, |(room, _)| room)
}

/// The story as played up to the boss of warp room `room` (1–5): beating him
/// opens the next room, and beating Cortex (5) finishes the story. In order:
///
/// 1. each level of warp rooms 1 to `room`, in [`LEVELS`](super::LEVELS) order: its crystal,
///    and with `with_gems` its clear, second and colored gems where it has
///    them;
/// 2. bosses 1 to `room` defeated, with or without their story scenes;
/// 3. with `with_abilities`, abilities 1 to `room` on.
///
/// With 5 crystals per room and those bosses defeated, the next boss's
/// portal works (`docs/FORMAT.md` §7). The edits don't depend on the save.
pub fn story_up_to(room: u8, options: &StoryUpTo) -> Result<Vec<Edit>, EditError> {
    if !(1..=5).contains(&room) {
        return Err(EditError::InvalidValue(format!(
            "room {room}: the story goes up to warp room 1 to 5"
        )));
    }
    let n = usize::from(room);
    let mut edits: Vec<Edit> = levels()
        .filter(|l| l.level().warp_room <= room)
        .flat_map(|l| collectible_edits(l, options.with_gems))
        .collect();
    edits.extend(bosses(n, options.skip_cutscenes));
    if options.with_abilities {
        edits.extend(abilities(n, true));
    }
    Ok(edits)
}

// --------------------------------------------------------------- all relics

/// What [`all_relics`] pre-fills.
pub const DEFAULT_ALL_RELICS_TIER: Relic = Relic::Gold;

/// Options of [`all_relics`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AllRelics {
    /// Never replace a better relic with `tier`.
    pub only_upgrade: bool,
    pub times: TimePolicy,
}

impl Default for AllRelics {
    fn default() -> Self {
        Self {
            only_upgrade: true,
            times: TimePolicy::Matching,
        }
    }
}

/// `tier` on every level. For each level in [`LEVELS`](super::LEVELS) order whose relic
/// changes: the relic, then times per the policy. With `only_upgrade`, a
/// level that has `tier` or better is left alone.
pub fn all_relics(save: &Crash3Save, tier: Relic, options: &AllRelics) -> Vec<Edit> {
    levels()
        .filter(|&level| {
            let current = save.relic(level);
            if options.only_upgrade {
                current < Some(tier)
            } else {
                current != Some(tier)
            }
        })
        .flat_map(|level| relic_edits(save, level, tier, options.times))
        .collect()
}

// -------------------------------------------------------- reach relic count

/// Options of [`reach_relic_count`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReachRelicCount {
    /// The relic given to levels that have none.
    pub tier: Relic,
    pub times: TimePolicy,
}

impl Default for ReachRelicCount {
    fn default() -> Self {
        Self {
            tier: Relic::Sapphire,
            times: TimePolicy::Matching,
        }
    }
}

/// The count [`reach_relic_count`] starts at: the first secret warp room
/// threshold (5, 10, 15, 20, 25) above the current relic count, or 25.
pub fn default_relic_count_target(save: &Crash3Save) -> u32 {
    let relics = save.totals().relics();
    let thresholds = SECRET_ROOM.map(|s| u32::from(s.relics));
    let last = thresholds[thresholds.len() - 1];
    thresholds.into_iter().find(|&t| t > relics).unwrap_or(last)
}

/// At least `count` relics, any tier, which opens the secret warp room's
/// levels (`docs/FORMAT.md` §7). Levels without a relic get `options.tier`,
/// in [`LEVELS`](super::LEVELS) order, until the total reaches `count`: the relic, then
/// times per the policy. Empty if the save already has `count`.
pub fn reach_relic_count(save: &Crash3Save, count: u32, options: &ReachRelicCount) -> Vec<Edit> {
    let missing = count.saturating_sub(save.totals().relics()) as usize;
    levels()
        .filter(|&level| save.relic(level).is_none())
        .take(missing)
        .flat_map(|level| relic_edits(save, level, options.tier, options.times))
        .collect()
}

// ---------------------------------------------------- abilities and scenes

/// What [`all_abilities`] pre-fills.
pub const DEFAULT_ABILITIES_ON: bool = true;

/// What [`all_cutscenes`] pre-fills.
pub const DEFAULT_CUTSCENES_SEEN: bool = true;

/// Each ability, in [`ABILITIES`] order.
pub fn all_abilities(on: bool) -> Vec<Edit> {
    abilities(ABILITIES.len(), on).collect()
}

/// Each story scene of [`CUTSCENES`], step by step, in table order.
pub fn all_cutscenes(seen: bool) -> Vec<Edit> {
    CUTSCENES
        .iter()
        .flat_map(|step| step.scenes)
        .map(|&name| Edit::SetCutscene {
            cutscene: name.to_owned(),
            seen,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::tables::LEVELS;
    use super::*;

    #[test]
    fn matching_time_is_one_centisecond_under_the_threshold() {
        let toad = LevelId::from_key("toadvillage").unwrap();
        assert_eq!(matching_time(toad, Relic::Platinum), Some(4402));
        assert_eq!(super::super::format_time(4402), "0:44.02");
        let tiny = LevelId::from_key("tinytiger").unwrap();
        assert_eq!(matching_time(tiny, Relic::Gold), None);
        for level in levels() {
            for relic in Relic::ALL {
                let t = matching_time(level, relic).unwrap();
                assert_eq!(relic_for_time(level, t), Some(relic), "{level:?} {relic:?}");
            }
        }
        assert_eq!(
            LEVELS.iter().filter(|l| l.has_relic()).count(),
            levels().count()
        );
    }
}
