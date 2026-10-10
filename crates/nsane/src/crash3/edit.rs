//! Edits: every change to a save is an [`Edit`], applied by
//! [`Crash3Save::apply`]. Edits are also written as JSON objects with an `op`
//! tag, which is how the golden test vectors in `tests/golden/` list them.

use igb::IgbError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use super::model::Crash3Save;
use super::tables::{Ability, Kind, LevelId, Relic, is_cutscene};
use super::time::format_time;

/// The longest time trial time an edit can write: 99:59.99.
pub const MAX_TIME_CS: u32 = 599_999;

/// Warp rooms that [`Edit::UnlockWarpRoom`] can open. Room 1 is always open.
pub const UNLOCKABLE_ROOMS: std::ops::RangeInclusive<u8> = 2..=5;

/// One change to a save.
///
/// Levels, bosses and abilities are written as their keys. Every field is
/// required, so serializing an edit gives back the JSON it was read from.
/// [`Edit::from_json`] reads edits with the error codes of [`EditError`]; the
/// derived `Deserialize` only gives serde errors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Edit {
    /// The crystal bit and the `crystal` field. Levels 1–25 only.
    SetCrystal {
        level: LevelId,
        on: bool,
    },
    /// The clear gem, for breaking every box.
    SetGem {
        level: LevelId,
        on: bool,
    },
    /// The second clear gem, in the 10 levels that have one.
    SetSecondGem {
        level: LevelId,
        on: bool,
    },
    /// The colored gem bit and its `Gem_<Color>` variable, in the 5 levels
    /// that have one.
    SetColoredGem {
        level: LevelId,
        on: bool,
    },
    /// The relic tier, its bit and the `<Name>_RelicCounted` flag.
    SetRelic {
        level: LevelId,
        #[serde(deserialize_with = "Option::deserialize")]
        relic: Option<Relic>,
    },
    /// The time trial leaderboard: 0 to 3 times in centiseconds, any order.
    SetTimes {
        level: LevelId,
        times_cs: Vec<u32>,
    },
    /// The visit fields as after a first visit. Also takes a boss.
    MarkEntered {
        level: LevelId,
    },
    /// The boss bit. With `on` and `skip_cutscenes`, also marks the story
    /// scenes of the `start` step and of this boss's step as seen.
    SetBossDefeated {
        boss: LevelId,
        on: bool,
        skip_cutscenes: bool,
    },
    /// Opens warp room `room` (2–5): defeats the bosses before it, optionally
    /// marks their story scenes seen and grants their abilities.
    UnlockWarpRoom {
        room: u8,
        with_abilities: bool,
        skip_cutscenes: bool,
    },
    SetAbility {
        ability: Ability,
        on: bool,
    },
    /// The 47th gem, given in the hub.
    SetBonusGem {
        on: bool,
    },
    /// A hub story scene, by its name in [`CUTSCENES`](super::CUTSCENES). A
    /// scene marked seen doesn't play.
    SetCutscene {
        cutscene: String,
        seen: bool,
    },
    SetLives {
        count: u8,
    },
    /// Sets a key of the flag table to 1.
    RawFlagSet {
        key: String,
    },
    /// Removes a key from the flag table.
    RawFlagRemove {
        key: String,
    },
    /// Returns a level or boss to its state in a new game: no relic, colored
    /// gem, times or collectibles, and never entered. Abilities and story
    /// scenes are left alone. Not yet confirmed in game: see
    /// [`RESET_LEVEL_VERIFIED`](super::RESET_LEVEL_VERIFIED).
    ResetLevel {
        level: LevelId,
    },
}

/// Why an edit can't be read or applied. [`code`](EditError::code) gives the
/// stable identifier used by the test vectors.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EditError {
    /// Not a known op, missing or extra fields, a field of the wrong type, or
    /// a value out of range.
    #[error("invalid edit: {0}")]
    InvalidValue(String),

    #[error("unknown level {0:?}")]
    UnknownLevel(String),

    #[error("unknown boss {0:?}")]
    UnknownBoss(String),

    #[error("unknown ability {0:?}")]
    UnknownAbility(String),

    #[error("unknown story scene {0:?}")]
    UnknownCutscene(String),

    /// The level doesn't have the collectible.
    #[error("{0}")]
    NotApplicable(String),

    /// A Switch save's data outgrew its fixed file size.
    #[error("the save data needs {needed} bytes but the file holds {capacity}")]
    DoesNotFit { needed: usize, capacity: usize },

    /// The container refused a change. Only a malformed file gets here.
    #[error(transparent)]
    Container(IgbError),
}

impl EditError {
    pub fn code(&self) -> &'static str {
        match self {
            EditError::InvalidValue(_) => "invalid_value",
            EditError::UnknownLevel(_) => "unknown_level",
            EditError::UnknownBoss(_) => "unknown_boss",
            EditError::UnknownAbility(_) => "unknown_ability",
            EditError::UnknownCutscene(_) => "unknown_cutscene",
            EditError::NotApplicable(_) => "not_applicable",
            EditError::DoesNotFit { .. } => "does_not_fit",
            EditError::Container(_) => "container_error",
        }
    }
}

impl From<IgbError> for EditError {
    fn from(e: IgbError) -> Self {
        match e {
            IgbError::DoesNotFit { needed, capacity } => EditError::DoesNotFit { needed, capacity },
            e => EditError::Container(e),
        }
    }
}

/// The JSON type of an op field.
#[derive(Clone, Copy)]
enum Ty {
    Bool,
    Int,
    Str,
    /// `null` or a string.
    Relic,
    /// A list of integers.
    Times,
}

impl Ty {
    fn matches(self, v: &Value) -> bool {
        let int = |v: &Value| v.as_i64().is_some() || v.as_u64().is_some();
        match self {
            Ty::Bool => v.is_boolean(),
            Ty::Int => int(v),
            Ty::Str => v.is_string(),
            Ty::Relic => v.is_null() || v.is_string(),
            Ty::Times => v.as_array().is_some_and(|a| a.iter().all(int)),
        }
    }
}

/// The fields of each op, after `op`.
fn shape(op: &str) -> Option<&'static [(&'static str, Ty)]> {
    use Ty::*;
    Some(match op {
        "set_crystal" | "set_gem" | "set_second_gem" | "set_colored_gem" => {
            &[("level", Str), ("on", Bool)]
        }
        "set_relic" => &[("level", Str), ("relic", Relic)],
        "set_times" => &[("level", Str), ("times_cs", Times)],
        "mark_entered" | "reset_level" => &[("level", Str)],
        "set_boss_defeated" => &[("boss", Str), ("on", Bool), ("skip_cutscenes", Bool)],
        "unlock_warp_room" => &[
            ("room", Int),
            ("with_abilities", Bool),
            ("skip_cutscenes", Bool),
        ],
        "set_ability" => &[("ability", Str), ("on", Bool)],
        "set_bonus_gem" => &[("on", Bool)],
        "set_cutscene" => &[("cutscene", Str), ("seen", Bool)],
        "set_lives" => &[("count", Int)],
        "raw_flag_set" | "raw_flag_remove" => &[("key", Str)],
        _ => return None,
    })
}

fn invalid(what: impl Into<String>) -> EditError {
    EditError::InvalidValue(what.into())
}

/// Typed access to the fields of an op whose shape was checked.
struct Fields<'a>(&'a Map<String, Value>);

impl Fields<'_> {
    fn str(&self, name: &str) -> Result<&str, EditError> {
        self.0[name].as_str().ok_or_else(|| invalid(name))
    }

    fn bool(&self, name: &str) -> Result<bool, EditError> {
        self.0[name].as_bool().ok_or_else(|| invalid(name))
    }

    /// An integer that fits `T`.
    fn int<T: TryFrom<i64>>(&self, name: &str) -> Result<T, EditError> {
        let v = &self.0[name];
        v.as_i64()
            .and_then(|n| T::try_from(n).ok())
            .ok_or_else(|| invalid(format!("{name} {v}")))
    }

    /// A level key; a boss key too when `boss_ok`.
    fn level(&self, boss_ok: bool) -> Result<LevelId, EditError> {
        let key = self.str("level")?;
        LevelId::from_key(key)
            .filter(|id| boss_ok || id.level().kind != Kind::Boss)
            .ok_or_else(|| EditError::UnknownLevel(key.to_owned()))
    }
}

impl Edit {
    /// Reads an edit from its JSON form, checking in this order: the shape
    /// (a known `op`, exactly its fields, each of the right JSON type), then
    /// the identifiers, then the values the field types can't hold. What
    /// depends on the level or on a range is checked by
    /// [`validate`](Edit::validate), which [`Crash3Save::apply`] runs.
    pub fn from_json(value: &Value) -> Result<Edit, EditError> {
        let obj = value
            .as_object()
            .ok_or_else(|| invalid("an edit is a JSON object"))?;
        let op = obj
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("no op"))?;
        let fields = shape(op).ok_or_else(|| invalid(format!("unknown op {op:?}")))?;
        if obj.len() != fields.len() + 1
            || !fields
                .iter()
                .all(|&(name, ty)| obj.get(name).is_some_and(|v| ty.matches(v)))
        {
            let names: Vec<_> = fields.iter().map(|(name, _)| *name).collect();
            return Err(invalid(format!(
                "{op} takes exactly these fields, with the right types: {}",
                names.join(", ")
            )));
        }
        let f = Fields(obj);

        Ok(match op {
            "set_crystal" => Edit::SetCrystal {
                level: f.level(false)?,
                on: f.bool("on")?,
            },
            "set_gem" => Edit::SetGem {
                level: f.level(false)?,
                on: f.bool("on")?,
            },
            "set_second_gem" => Edit::SetSecondGem {
                level: f.level(false)?,
                on: f.bool("on")?,
            },
            "set_colored_gem" => Edit::SetColoredGem {
                level: f.level(false)?,
                on: f.bool("on")?,
            },
            "set_relic" => {
                let level = f.level(false)?;
                let relic = match &obj["relic"] {
                    Value::String(key) => Some(
                        Relic::from_key(key).ok_or_else(|| invalid(format!("relic {key:?}")))?,
                    ),
                    _ => None,
                };
                Edit::SetRelic { level, relic }
            }
            "set_times" => {
                let level = f.level(false)?;
                let times_cs = obj["times_cs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|t| {
                        t.as_u64()
                            .and_then(|t| u32::try_from(t).ok())
                            .ok_or_else(|| invalid(format!("time {t}")))
                    })
                    .collect::<Result<_, _>>()?;
                Edit::SetTimes { level, times_cs }
            }
            "mark_entered" => Edit::MarkEntered {
                level: f.level(true)?,
            },
            "reset_level" => Edit::ResetLevel {
                level: f.level(true)?,
            },
            "set_boss_defeated" => {
                let key = f.str("boss")?;
                let boss = LevelId::from_key(key)
                    .filter(|id| id.level().kind == Kind::Boss)
                    .ok_or_else(|| EditError::UnknownBoss(key.to_owned()))?;
                Edit::SetBossDefeated {
                    boss,
                    on: f.bool("on")?,
                    skip_cutscenes: f.bool("skip_cutscenes")?,
                }
            }
            "unlock_warp_room" => Edit::UnlockWarpRoom {
                room: f.int("room")?,
                with_abilities: f.bool("with_abilities")?,
                skip_cutscenes: f.bool("skip_cutscenes")?,
            },
            "set_ability" => {
                let key = f.str("ability")?;
                Edit::SetAbility {
                    ability: Ability::from_key(key)
                        .ok_or_else(|| EditError::UnknownAbility(key.to_owned()))?,
                    on: f.bool("on")?,
                }
            }
            "set_bonus_gem" => Edit::SetBonusGem { on: f.bool("on")? },
            "set_cutscene" => {
                let cutscene = f.str("cutscene")?;
                if !is_cutscene(cutscene) {
                    return Err(EditError::UnknownCutscene(cutscene.to_owned()));
                }
                Edit::SetCutscene {
                    cutscene: cutscene.to_owned(),
                    seen: f.bool("seen")?,
                }
            }
            "set_lives" => Edit::SetLives {
                count: f.int("count")?,
            },
            "raw_flag_set" => Edit::RawFlagSet {
                key: f.str("key")?.to_owned(),
            },
            "raw_flag_remove" => Edit::RawFlagRemove {
                key: f.str("key")?.to_owned(),
            },
            _ => return Err(invalid(format!("unknown op {op:?}"))),
        })
    }

    /// The level of a level edit (not [`MarkEntered`](Edit::MarkEntered) or
    /// [`ResetLevel`](Edit::ResetLevel), which also take a boss).
    fn level_op(&self) -> Option<LevelId> {
        match *self {
            Edit::SetCrystal { level, .. }
            | Edit::SetGem { level, .. }
            | Edit::SetSecondGem { level, .. }
            | Edit::SetColoredGem { level, .. }
            | Edit::SetRelic { level, .. }
            | Edit::SetTimes { level, .. } => Some(level),
            _ => None,
        }
    }

    /// Checks what the field types don't: the kind of each id (a level edit
    /// takes no boss, a boss edit only a boss), the scene name, whether the
    /// level has the collectible, and the value ranges.
    pub fn validate(&self) -> Result<(), EditError> {
        // identifiers
        if let Some(level) = self.level_op()
            && level.level().kind == Kind::Boss
        {
            return Err(EditError::UnknownLevel(level.key().to_owned()));
        }
        match self {
            Edit::SetBossDefeated { boss, .. } if boss.level().kind != Kind::Boss => {
                return Err(EditError::UnknownBoss(boss.key().to_owned()));
            }
            Edit::SetCutscene { cutscene, .. } if !is_cutscene(cutscene) => {
                return Err(EditError::UnknownCutscene(cutscene.clone()));
            }
            _ => {}
        }

        // applicability
        let missing = match *self {
            Edit::SetCrystal { level, .. } if !level.level().has_crystal() => {
                Some((level, "crystal"))
            }
            Edit::SetSecondGem { level, .. } if !level.level().second_gem => {
                Some((level, "second gem"))
            }
            Edit::SetColoredGem { level, .. } if level.level().colored_gem.is_none() => {
                Some((level, "colored gem"))
            }
            _ => None,
        };
        if let Some((level, what)) = missing {
            return Err(EditError::NotApplicable(format!(
                "{} has no {what}",
                level.name()
            )));
        }

        // values
        match self {
            Edit::SetTimes { times_cs, .. }
                if times_cs.len() > 3
                    || times_cs.iter().any(|t| !(1..=MAX_TIME_CS).contains(t)) =>
            {
                Err(invalid(format!(
                    "times_cs {times_cs:?}: at most 3 times of 1 to {MAX_TIME_CS} centiseconds"
                )))
            }
            Edit::UnlockWarpRoom { room, .. } if !UNLOCKABLE_ROOMS.contains(room) => Err(invalid(
                format!("room {room}: warp rooms 2 to 5 can be unlocked"),
            )),
            Edit::RawFlagSet { key } | Edit::RawFlagRemove { key }
                if key.is_empty() || key.chars().any(|c| !('\u{1}'..='\u{ff}').contains(&c)) =>
            {
                // Keys are stored NUL-terminated, one latin-1 byte per character.
                Err(invalid(format!("flag key {key:?}")))
            }
            _ => Ok(()),
        }
    }

    /// A short label for the edit history, such as "Deep Trouble: crystal on".
    /// It describes the edit alone, so it reads the same before and after the
    /// edit is applied; the save is there for labels that need more context.
    pub fn describe(&self, _save: &Crash3Save) -> String {
        let on_off = |on: bool| if on { "on" } else { "off" };
        match self {
            Edit::SetCrystal { level, on } => format!("{}: crystal {}", level.name(), on_off(*on)),
            Edit::SetGem { level, on } => format!("{}: clear gem {}", level.name(), on_off(*on)),
            Edit::SetSecondGem { level, on } => {
                format!("{}: second gem {}", level.name(), on_off(*on))
            }
            Edit::SetColoredGem { level, on } => format!(
                "{}: {} gem {}",
                level.name(),
                level.level().colored_gem.map_or("colored", |c| c.key()),
                on_off(*on)
            ),
            Edit::SetRelic { level, relic } => match relic {
                Some(relic) => format!("{}: {} relic", level.name(), relic.key()),
                None => format!("{}: no relic", level.name()),
            },
            Edit::SetTimes { level, times_cs } if times_cs.is_empty() => {
                format!("{}: times cleared", level.name())
            }
            Edit::SetTimes { level, times_cs } => {
                let mut times = times_cs.clone();
                times.sort_unstable();
                let times: Vec<_> = times.iter().map(|&t| format_time(t)).collect();
                format!("{}: times {}", level.name(), times.join(", "))
            }
            Edit::MarkEntered { level } => format!("{}: mark entered", level.name()),
            Edit::ResetLevel { level } => format!("{}: reset to never played", level.name()),
            Edit::SetBossDefeated {
                boss,
                on,
                skip_cutscenes,
            } => {
                let state = if *on { "defeated" } else { "not defeated" };
                let skip = if *on && *skip_cutscenes {
                    " (skip scenes)"
                } else {
                    ""
                };
                format!("{}: {state}{skip}", boss.name())
            }
            Edit::UnlockWarpRoom {
                room,
                with_abilities,
                skip_cutscenes,
            } => {
                let extras: Vec<_> = [
                    with_abilities.then_some("+abilities"),
                    skip_cutscenes.then_some("skip scenes"),
                ]
                .into_iter()
                .flatten()
                .collect();
                if extras.is_empty() {
                    format!("Unlock warp room {room}")
                } else {
                    format!("Unlock warp room {room} ({})", extras.join(", "))
                }
            }
            Edit::SetAbility { ability, on } => format!("{}: {}", ability.name(), on_off(*on)),
            Edit::SetBonusGem { on } => format!("Bonus gem: {}", on_off(*on)),
            Edit::SetCutscene { cutscene, seen } => format!(
                "Scene {cutscene}: {}",
                if *seen { "seen" } else { "not seen" }
            ),
            Edit::SetLives { count } => format!("Lives: {count}"),
            Edit::RawFlagSet { key } => format!("Flag {key}: set"),
            Edit::RawFlagRemove { key } => format!("Flag {key}: removed"),
        }
    }
}

impl Crash3Save {
    /// Validates `edit`, then applies it. A validation error leaves the save
    /// unchanged. A container error, which only a malformed file can cause,
    /// may leave it partly edited, so callers that must keep the save
    /// intact edit a copy.
    ///
    /// The totals in the summary are recomputed by
    /// [`to_bytes`](Crash3Save::to_bytes), not here.
    pub fn apply(&mut self, edit: &Edit) -> Result<(), EditError> {
        edit.validate()?;
        match *edit {
            Edit::SetCrystal { level, on } => self.set_crystal(level, on),
            Edit::SetGem { level, on } => self.set_gem(level, on),
            Edit::SetSecondGem { level, on } => self.set_second_gem(level, on),
            Edit::SetColoredGem { level, on } => self.set_colored_gem(level, on),
            Edit::SetRelic { level, relic } => self.set_relic(level, relic)?,
            Edit::SetTimes {
                level,
                ref times_cs,
            } => self.set_times(level, times_cs)?,
            Edit::MarkEntered { level } => self.mark_entered(level),
            Edit::ResetLevel { level } => self.reset_level(level)?,
            Edit::SetBossDefeated {
                boss,
                on,
                skip_cutscenes,
            } => {
                self.set_boss_defeated(boss, on);
                if on && skip_cutscenes {
                    self.skip_story_cutscenes(boss)?;
                }
            }
            Edit::UnlockWarpRoom {
                room,
                with_abilities,
                skip_cutscenes,
            } => self.unlock_warp_room(room, with_abilities, skip_cutscenes)?,
            Edit::SetAbility { ability, on } => self.set_ability(ability, on),
            Edit::SetBonusGem { on } => self.set_bonus_gem(on)?,
            Edit::SetCutscene { ref cutscene, seen } => self.set_cutscene(cutscene, seen)?,
            Edit::SetLives { count } => self.set_lives(count)?,
            Edit::RawFlagSet { ref key } => self.set_flag(key, true)?,
            Edit::RawFlagRemove { ref key } => self.set_flag(key, false)?,
        }
        Ok(())
    }

    /// Applies edits in order, stopping at the first error.
    pub fn apply_all<'a>(
        &mut self,
        edits: impl IntoIterator<Item = &'a Edit>,
    ) -> Result<(), EditError> {
        edits.into_iter().try_for_each(|edit| self.apply(edit))
    }
}
