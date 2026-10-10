//! Typed read access to the Crash 3 part of a save, and the summary the save
//! slot screen shows (`docs/FORMAT.md` §4–§7).

use std::collections::BTreeMap;
use std::fmt;

use igb::{FieldValue, IgObject, IgbError, IgbFile, SummaryValue, latin1};

use super::tables::{
    ABILITIES, Ability, BONUS_GEM_KEY, BOSSES, CORTEX, CUTSCENES, EMPTY_INITIALS, GemColor, Kind,
    LEVELS, LevelId, PROGRESSION, Relic, bit, cutscene_key, field, relic_counted_key,
};
use crate::Error;

/// Summary keys the editor reads or recomputes. Every game save has them.
const INT_KEYS: [&str; 8] = [
    "crystal_count",
    "gem_count",
    "sapphire_count",
    "gold_count",
    "platinum_count",
    "farthest_progression_index",
    "story_completed",
    "life_count",
];
const PERCENT_KEY: &str = "percent_complete";

/// Expected type of each session field the editor reads.
const SESSION_FIELDS: [(usize, &str); 11] = [
    (field::STATE, "igEnumMetaField"),
    (field::COMPLETIONS, "igIntMetaField"),
    (field::TIMES[0], "igIntMetaField"),
    (field::TIMES[1], "igIntMetaField"),
    (field::TIMES[2], "igIntMetaField"),
    (field::INITIALS[0], "igStringMetaField"),
    (field::INITIALS[1], "igStringMetaField"),
    (field::INITIALS[2], "igStringMetaField"),
    (field::RELIC_TIER, "igIntMetaField"),
    (field::CRYSTAL, "igBoolMetaField"),
    (field::COLLECTIBLES, "igUnsignedIntMetaField"),
];

/// The Crash 3 progress of a save, over the parsed container.
///
/// Every object the accessors need is found and checked when the file is
/// opened, so a file that opens can be read without further errors.
#[derive(Debug, Clone, PartialEq)]
pub struct Crash3Save {
    igb: IgbFile,
    /// Every zone session, in the order of the zone list: `(zone path, session id)`.
    zones: Vec<(String, u32)>,
    /// Session id of each entry of [`LEVELS`].
    level_sessions: Vec<u32>,
    /// Game variable object ids by handle name.
    variables: BTreeMap<String, u32>,
    /// Object id of the one-shot flag table.
    flag_table: u32,
    /// Object id of the in-level checkpoint table, which the editor only reads.
    int_table: Option<u32>,
}

/// Progress totals, counted from the sessions and flags (not the stored summary).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Totals {
    pub crystals: u32,
    /// Every gem, bonus gem included.
    pub gems: u32,
    pub sapphire: u32,
    pub gold: u32,
    pub platinum: u32,
    pub bosses: u32,
    pub gem_breakdown: GemBreakdown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GemBreakdown {
    /// Out of 31.
    pub clear: u32,
    /// Out of 10.
    pub second: u32,
    /// Out of 5.
    pub colored: u32,
    /// 0 or 1.
    pub bonus: u32,
}

impl Totals {
    /// Relics of any tier.
    pub fn relics(&self) -> u32 {
        self.sapphire + self.gold + self.platinum
    }

    /// One point per crystal, gem, relic and boss (§6).
    pub fn percent(&self) -> u32 {
        self.crystals + self.gems + self.relics() + self.bosses
    }
}

/// A state only edits can produce, which the game handles badly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Warning {
    /// More than 5 × `room` crystals while `boss`, the first boss still alive,
    /// guards warp room `room`: his portal flickers and can't be entered (§7).
    PortalGlitch {
        boss: LevelId,
        crystals: u32,
        room: u8,
    },
}

impl Warning {
    /// A stable identifier.
    pub fn code(&self) -> &'static str {
        match self {
            Warning::PortalGlitch { .. } => "portal_glitch",
        }
    }

    pub fn message(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Warning::PortalGlitch {
                boss,
                crystals,
                room,
            } => write!(
                f,
                "{} is not defeated but there are {crystals} crystals (more than {}): his \
                 portal will glitch in warp room {room}. Defeat him or remove crystals.",
                boss.name(),
                5 * u32::from(room),
            ),
        }
    }
}

fn not_a_save(reason: impl Into<String>) -> Error {
    Error::NotASave(reason.into())
}

/// Replaces a one-word value, keeping its type (and a bool's padding bytes).
fn set_value(field: &mut FieldValue, value: i32) {
    *field = match *field {
        FieldValue::Int(_) => FieldValue::Int(value),
        FieldValue::Enum(_) => FieldValue::Enum(value),
        FieldValue::UInt(_) => FieldValue::UInt(value as u32),
        FieldValue::Bool { padding, .. } => FieldValue::Bool {
            value: value as u8,
            padding,
        },
        FieldValue::Float(_) => FieldValue::Float((value as f32).to_bits()),
        other => other,
    };
}

/// The truth value of a game variable. Only the value byte of a bool counts.
fn truthy(value: FieldValue) -> bool {
    match value {
        FieldValue::Bool { value, .. } => value != 0,
        FieldValue::Float(bits) => f32::from_bits(bits) != 0.0,
        other => other.as_u32().is_some_and(|v| v != 0),
    }
}

/// The keys and values of a hash table. Both tables were read once at load,
/// and edits keep them readable, so this doesn't fail in practice.
fn table_entries(igb: &IgbFile, id: Option<u32>) -> BTreeMap<String, u32> {
    id.and_then(|id| igb.hash_table(id).ok()?.items().ok())
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| (latin1::decode(&key), value))
        .collect()
}

impl Crash3Save {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        Self::from_igb(IgbFile::parse(data)?)
    }

    pub fn from_igb(igb: IgbFile) -> Result<Self, Error> {
        let zone_list = igb
            .objects_of("CZoneInfoSaveList")
            .next()
            .ok_or_else(|| not_a_save("no zone list"))?;
        let mut zones = Vec::new();
        for zone_id in igb.deref_list(zone_list)? {
            let malformed = || not_a_save(format!("malformed zone record {zone_id}"));
            let zone = igb.object(zone_id).ok_or_else(malformed)?;
            let (Some(&FieldValue::Str(path)), Some(&FieldValue::ObjRef(session))) =
                (zone.field(0), zone.field(1))
            else {
                return Err(malformed());
            };
            let path = igb.string(path).ok_or_else(malformed)?;
            igb.object(session).ok_or_else(malformed)?;
            zones.push((latin1::decode(path), session));
        }

        let mut level_sessions = Vec::with_capacity(LEVELS.len());
        for level in &LEVELS {
            let (_, session) = zones
                .iter()
                .rev()
                .find(|(path, _)| path == level.zone)
                .ok_or_else(|| not_a_save(format!("no session for {}", level.zone)))?;
            let fields = &igb
                .object(*session)
                .ok_or_else(|| not_a_save("session"))?
                .fields;
            for (i, type_name) in SESSION_FIELDS {
                if fields.get(i).map(FieldValue::type_name) != Some(type_name) {
                    return Err(not_a_save(format!(
                        "unexpected session layout in {}",
                        level.zone
                    )));
                }
            }
            level_sessions.push(*session);
        }

        let mut variables = BTreeMap::new();
        for obj in igb.objects() {
            if !obj.class.starts_with("CGameVariableValueSaved") || obj.class.ends_with("List") {
                continue;
            }
            let (Some(value), Some(&FieldValue::Handle(hash, namespace))) =
                (obj.field(0), obj.field(1))
            else {
                return Err(not_a_save(format!("malformed game variable {}", obj.id)));
            };
            if value.as_u32().is_none() {
                return Err(not_a_save(format!("malformed game variable {}", obj.id)));
            }
            let name = igb
                .handle_name((hash, namespace))
                .map_or_else(|| format!("#{hash:08x}"), latin1::decode);
            variables.insert(name, obj.id);
        }
        let required = ABILITIES
            .iter()
            .map(|a| a.variable())
            .chain(GemColor::ALL.iter().map(|c| c.variable()));
        for name in required {
            if !variables.contains_key(name) {
                return Err(not_a_save(format!("no {name} variable")));
            }
        }

        let flag_table = igb
            .objects_of("igStringBoolHashTable")
            .next()
            .ok_or_else(|| not_a_save("no flag table"))?
            .id;
        igb.hash_table(flag_table)?.items()?;
        let int_table = igb.objects_of("igStringIntHashTable").next().map(|t| t.id);
        if let Some(id) = int_table {
            igb.hash_table(id)?.items()?;
        }

        let summary = igb.summary();
        for key in INT_KEYS {
            if !matches!(summary.get(key), Some(SummaryValue::Int(_))) {
                return Err(not_a_save(format!("no {key} in the summary")));
            }
        }
        if !matches!(summary.get(PERCENT_KEY), Some(SummaryValue::Float(_))) {
            return Err(not_a_save(format!("no {PERCENT_KEY} in the summary")));
        }

        Ok(Self {
            igb,
            zones,
            level_sessions,
            variables,
            flag_table,
            int_table,
        })
    }

    pub fn igb(&self) -> &IgbFile {
        &self.igb
    }

    pub fn into_igb(self) -> IgbFile {
        self.igb
    }

    /// Recomputes the summary, then builds the file (padded on the Switch).
    pub fn to_bytes(&mut self) -> Result<Vec<u8>, Error> {
        self.update_summary()?;
        Ok(self.igb.build()?)
    }

    // ------------------------------------------------------------- zones

    /// Every zone session (all three games), in the order of the zone list.
    pub fn zones(&self) -> impl Iterator<Item = (&str, &IgObject)> {
        self.zones
            .iter()
            .filter_map(|(path, id)| Some((path.as_str(), self.igb.object(*id)?)))
    }

    pub fn session(&self, level: LevelId) -> Option<&IgObject> {
        self.igb.object(self.level_sessions[level.index()])
    }

    /// A session field as an integer. The layout was checked at load.
    fn int(&self, level: LevelId, i: usize) -> i32 {
        match self.session(level).and_then(|s| s.field(i)) {
            Some(FieldValue::Bool { value, .. }) => i32::from(*value),
            Some(v) => v.as_u32().map_or(0, |v| v as i32),
            None => 0,
        }
    }

    // ------------------------------------------------------------ levels

    /// The `collectibles` bits.
    pub fn flags(&self, level: LevelId) -> u32 {
        self.int(level, field::COLLECTIBLES) as u32
    }

    /// Bits the editor doesn't know. Never observed; kept as they are.
    pub fn flags_unknown(&self, level: LevelId) -> u32 {
        self.flags(level) & !bit::KNOWN
    }

    fn has(&self, level: LevelId, bits: u32) -> bool {
        self.flags(level) & bits != 0
    }

    pub fn entered(&self, level: LevelId) -> bool {
        self.int(level, field::STATE) >= 1
    }

    /// 1 once the level was first left (finished or not), then +1 per finish.
    /// It is not a play count.
    pub fn completions(&self, level: LevelId) -> i32 {
        self.int(level, field::COMPLETIONS)
    }

    pub fn crystal(&self, level: LevelId) -> bool {
        self.has(level, bit::CRYSTAL)
    }

    /// The `crystal` bool field, which mirrors the `CRYSTAL` bit.
    pub fn crystal_field(&self, level: LevelId) -> bool {
        self.int(level, field::CRYSTAL) != 0
    }

    pub fn gem(&self, level: LevelId) -> bool {
        self.has(level, bit::GEM_CLEAR)
    }

    pub fn second_gem(&self, level: LevelId) -> bool {
        self.has(level, bit::GEM_SECOND)
    }

    /// `false` for a level without a colored gem.
    pub fn colored_gem(&self, level: LevelId) -> bool {
        level
            .level()
            .colored_gem
            .is_some_and(|c| self.has(level, c.bit()))
    }

    /// Clear gem + second gem + colored gem.
    pub fn gem_count(&self, level: LevelId) -> u32 {
        u32::from(self.gem(level))
            + u32::from(self.second_gem(level))
            + u32::from(self.colored_gem(level))
    }

    pub fn boss_defeated(&self, level: LevelId) -> bool {
        self.has(level, bit::BOSS_DEFEATED)
    }

    /// The relic of the `relic_tier` field. A tier the game never writes reads
    /// as no relic.
    pub fn relic(&self, level: LevelId) -> Option<Relic> {
        Relic::from_tier(self.int(level, field::RELIC_TIER))
    }

    /// The time trial leaderboard, best first, without the empty slots.
    pub fn times(&self, level: LevelId) -> Vec<u32> {
        field::TIMES
            .iter()
            .map(|&i| self.int(level, i))
            .filter(|&t| t > 0)
            .map(|t| t as u32)
            .collect()
    }

    // --------------------------------------------------------- variables

    /// The value of a game variable, by handle name.
    pub fn variable(&self, name: &str) -> Option<FieldValue> {
        let id = self.variables.get(name)?;
        self.igb.object(*id)?.field(0).copied()
    }

    /// Every game variable: `(handle name, value)`, sorted by name.
    pub fn variables(&self) -> impl Iterator<Item = (&str, FieldValue)> {
        self.variables
            .keys()
            .filter_map(|name| Some((name.as_str(), self.variable(name)?)))
    }

    pub fn ability(&self, ability: Ability) -> bool {
        self.variable(ability.variable()).is_some_and(truthy)
    }

    // ------------------------------------------------------------- flags

    pub fn flag(&self, key: &str) -> bool {
        let Some(key) = latin1::encode(key) else {
            return false;
        };
        self.igb
            .hash_table(self.flag_table)
            .ok()
            .and_then(|t| t.get(&key))
            .is_some_and(|v| v != 0)
    }

    /// Every key of the flag table with its value, sorted by key.
    pub fn flag_entries(&self) -> BTreeMap<String, u32> {
        table_entries(&self.igb, Some(self.flag_table))
    }

    /// Every key of the in-level checkpoint table with its value, sorted by
    /// key. Empty if the save has no such table.
    pub fn int_flag_entries(&self) -> BTreeMap<String, u32> {
        table_entries(&self.igb, self.int_table)
    }

    pub fn cutscene_seen(&self, name: &str) -> bool {
        self.flag(&cutscene_key(name))
    }

    pub fn bonus_gem(&self) -> bool {
        self.flag(BONUS_GEM_KEY)
    }

    // ----------------------------------------------------------- summary

    fn summary_int(&self, key: &str) -> Option<i32> {
        match self.igb.summary().get(key) {
            Some(SummaryValue::Int(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn lives(&self) -> i32 {
        self.summary_int("life_count").unwrap_or(0)
    }

    /// Zone path of the last zone entered. A fresh save has none.
    pub fn previous_level_name(&self) -> Option<String> {
        match self.igb.summary().get("previous_level_name") {
            Some(SummaryValue::Str(s)) => Some(latin1::decode(s)),
            _ => None,
        }
    }

    /// Progression index of the last zone entered; -1 in a fresh save.
    pub fn previous_level_progression_index(&self) -> Option<i32> {
        self.summary_int("previous_level_progression_index")
    }

    /// Non-zero when the save was made inside a level.
    pub fn active_progression_index(&self) -> Option<i32> {
        self.summary_int("active_progression_index")
    }

    /// The stored value; [`update_summary`](Self::update_summary) recomputes it.
    pub fn story_completed(&self) -> bool {
        self.summary_int("story_completed").is_some_and(|v| v != 0)
    }

    /// The stored value; [`update_summary`](Self::update_summary) raises it.
    pub fn farthest_progression_index(&self) -> i32 {
        self.summary_int("farthest_progression_index").unwrap_or(0)
    }

    pub fn totals(&self) -> Totals {
        let mut t = Totals::default();
        for level in LevelId::all() {
            if level.level().kind == Kind::Boss {
                t.bosses += u32::from(self.boss_defeated(level));
                continue;
            }
            t.crystals += u32::from(self.crystal(level));
            t.gem_breakdown.clear += u32::from(self.gem(level));
            t.gem_breakdown.second += u32::from(self.second_gem(level));
            t.gem_breakdown.colored += u32::from(self.colored_gem(level));
            match self.relic(level) {
                Some(Relic::Sapphire) => t.sapphire += 1,
                Some(Relic::Gold) => t.gold += 1,
                Some(Relic::Platinum) => t.platinum += 1,
                None => {}
            }
        }
        t.gem_breakdown.bonus = u32::from(self.bonus_gem());
        let g = t.gem_breakdown;
        t.gems = g.clear + g.second + g.colored + g.bonus;
        t
    }

    pub fn percent(&self) -> u32 {
        self.totals().percent()
    }

    /// Writes the totals the save slot screen shows (§6), in this order: the
    /// five counts, `percent_complete`, `farthest_progression_index` (only
    /// ever raised) and `story_completed`.
    pub fn update_summary(&mut self) -> Result<(), Error> {
        let t = self.totals();
        let farthest = self
            .farthest_progression_index()
            .max(self.farthest_progression() as i32);
        let story_completed = self.boss_defeated(CORTEX);
        let summary = self.igb.summary_mut();
        for (key, value) in [
            ("crystal_count", t.crystals),
            ("gem_count", t.gems),
            ("sapphire_count", t.sapphire),
            ("gold_count", t.gold),
            ("platinum_count", t.platinum),
        ] {
            summary.set(key, SummaryValue::Int(value as i32))?;
        }
        summary.set(PERCENT_KEY, SummaryValue::Float(t.percent() as f32))?;
        summary.set("farthest_progression_index", SummaryValue::Int(farthest))?;
        summary.set(
            "story_completed",
            SummaryValue::Int(i32::from(story_completed)),
        )?;
        Ok(())
    }

    /// The progression index the game would have stored (§6): that of the
    /// furthest zone entered, plus one once its completions counter is at
    /// least 1, capped at 30.
    pub fn farthest_progression(&self) -> usize {
        let mut farthest = 0;
        for (i, &level) in PROGRESSION.iter().enumerate() {
            if self.entered(level) {
                let reached = i + 1 + usize::from(self.completions(level) >= 1);
                farthest = farthest.max(reached.min(PROGRESSION.len()));
            }
        }
        farthest
    }

    /// Only the first boss still alive matters: the rooms after his stay
    /// locked in play.
    pub fn warnings(&self) -> Vec<Warning> {
        let crystals = self.totals().crystals;
        for (room, &boss) in (1u8..).zip(BOSSES.iter()) {
            if !self.boss_defeated(boss) {
                if crystals > 5 * u32::from(room) {
                    return vec![Warning::PortalGlitch {
                        boss,
                        crystals,
                        room,
                    }];
                }
                break;
            }
        }
        Vec::new()
    }

    /// Whether the hub would give the bonus gem now (§7): gold or platinum
    /// relics ≥ 30, percentage ≥ 107, and the gem not taken yet.
    pub fn bonus_gem_due(&self) -> bool {
        let t = self.totals();
        !self.bonus_gem() && t.gold + t.platinum >= 30 && t.percent() >= 107
    }

    // ------------------------------------------------------------- edits
    //
    // What each edit writes is listed in docs/FORMAT.md §8. The setters don't
    // check that an edit makes sense for the level: `apply` does. Their order
    // of writes matters: inserting a flag or a string can move other keys and
    // strings, and the outputs must match the saves tested in game byte for
    // byte.

    fn set_int(&mut self, level: LevelId, i: usize, value: i32) {
        let id = self.level_sessions[level.index()];
        if let Some(f) = self.igb.object_mut(id).and_then(|s| s.field_mut(i)) {
            set_value(f, value);
        }
    }

    fn set_str(&mut self, level: LevelId, i: usize, string: u32) {
        let id = self.level_sessions[level.index()];
        if let Some(f) = self.igb.object_mut(id).and_then(|s| s.field_mut(i)) {
            *f = FieldValue::Str(string);
        }
    }

    /// What the game writes after a first visit: `state` and `completions`
    /// raised to 1, never lowered.
    pub(crate) fn mark_entered(&mut self, level: LevelId) {
        if self.int(level, field::STATE) < 1 {
            self.set_int(level, field::STATE, 1);
        }
        if self.completions(level) < 1 {
            self.set_int(level, field::COMPLETIONS, 1);
        }
    }

    /// Turning a collectible on also marks the level entered; turning it off
    /// leaves the visit alone.
    fn set_bits(&mut self, level: LevelId, bits: u32, on: bool) {
        let flags = self.flags(level);
        let flags = if on { flags | bits } else { flags & !bits };
        self.set_int(level, field::COLLECTIBLES, flags as i32);
        if on {
            self.mark_entered(level);
        }
    }

    /// The bit and the `crystal` field, which the game keeps equal.
    pub(crate) fn set_crystal(&mut self, level: LevelId, on: bool) {
        self.set_bits(level, bit::CRYSTAL, on);
        self.set_int(level, field::CRYSTAL, i32::from(on));
        if on {
            self.mark_entered(level);
        }
    }

    pub(crate) fn set_gem(&mut self, level: LevelId, on: bool) {
        self.set_bits(level, bit::GEM_CLEAR, on);
    }

    pub(crate) fn set_second_gem(&mut self, level: LevelId, on: bool) {
        self.set_bits(level, bit::GEM_SECOND, on);
    }

    /// The zone bit and the `Gem_<Color>` variable that makes the gem's
    /// platforms appear. Each color is found in one level only, so turning it
    /// off can clear the variable. Does nothing for a level without one.
    pub(crate) fn set_colored_gem(&mut self, level: LevelId, on: bool) {
        let Some(color) = level.level().colored_gem else {
            return;
        };
        self.set_bits(level, color.bit(), on);
        self.set_variable(color.variable(), i32::from(on));
    }

    pub(crate) fn set_boss_defeated(&mut self, level: LevelId, on: bool) {
        self.set_bits(level, bit::BOSS_DEFEATED, on);
    }

    /// The tier, exactly one relic bit, and the `<Name>_RelicCounted` flag.
    /// Removing the relic doesn't touch the visit.
    pub(crate) fn set_relic(
        &mut self,
        level: LevelId,
        relic: Option<Relic>,
    ) -> Result<(), IgbError> {
        let flags = self.flags(level) & !bit::RELICS;
        self.set_int(level, field::COLLECTIBLES, flags as i32);
        self.set_int(level, field::RELIC_TIER, relic.map_or(0, Relic::tier));
        if let Some(relic) = relic {
            self.set_bits(level, relic.bit(), true);
        }
        self.set_flag(&relic_counted_key(level.level()), relic.is_some())
    }

    /// Up to 3 times, best first, with `""` initials per time and `"---"` per
    /// empty slot. Both initials are looked up (and inserted when missing)
    /// even for an empty list.
    pub(crate) fn set_times(&mut self, level: LevelId, times: &[u32]) -> Result<(), IgbError> {
        let mut times: Vec<u32> = times.iter().copied().filter(|&t| t > 0).collect();
        times.sort_unstable();
        times.truncate(field::TIMES.len());
        let player = self.igb.string_index(b"")?;
        let empty = self.igb.string_index(EMPTY_INITIALS.as_bytes())?;
        for (i, (&time_field, &initials_field)) in
            field::TIMES.iter().zip(&field::INITIALS).enumerate()
        {
            let time = times.get(i).copied();
            self.set_int(level, time_field, time.unwrap_or(0) as i32);
            self.set_str(
                level,
                initials_field,
                if time.is_some() { player } else { empty },
            );
        }
        if !times.is_empty() {
            self.mark_entered(level);
        }
        Ok(())
    }

    /// The session as in a new game. The relic setter removes the
    /// `<Name>_RelicCounted` flag and the colored gem setter clears the
    /// `Gem_<Color>` variable; the rest are plain field writes.
    pub(crate) fn reset_level(&mut self, level: LevelId) -> Result<(), IgbError> {
        self.set_relic(level, None)?;
        self.set_colored_gem(level, false);
        self.set_times(level, &[])?;
        self.set_int(level, field::COLLECTIBLES, 0);
        self.set_int(level, field::CRYSTAL, 0);
        self.set_int(level, field::STATE, -1);
        self.set_int(level, field::COMPLETIONS, 0);
        Ok(())
    }

    /// Writes a game variable, keeping its type. Unknown names are ignored:
    /// the ones the editor writes were checked at load.
    fn set_variable(&mut self, name: &str, value: i32) {
        let Some(&id) = self.variables.get(name) else {
            return;
        };
        if let Some(f) = self.igb.object_mut(id).and_then(|o| o.field_mut(0)) {
            set_value(f, value);
        }
    }

    pub(crate) fn set_ability(&mut self, ability: Ability, on: bool) {
        self.set_variable(ability.variable(), i32::from(on));
    }

    /// Sets a key of the flag table to 1, or removes it.
    pub(crate) fn set_flag(&mut self, key: &str, on: bool) -> Result<(), IgbError> {
        let key = latin1::encode(key).ok_or(IgbError::InvalidString)?;
        let mut table = self.igb.hash_table_mut(self.flag_table)?;
        if on {
            table.set(&key, 1)
        } else {
            table.remove(&key).map(drop)
        }
    }

    pub(crate) fn set_cutscene(&mut self, name: &str, seen: bool) -> Result<(), IgbError> {
        self.set_flag(&cutscene_key(name), seen)
    }

    pub(crate) fn set_bonus_gem(&mut self, on: bool) -> Result<(), IgbError> {
        self.set_flag(BONUS_GEM_KEY, on)
    }

    pub(crate) fn set_lives(&mut self, count: u8) -> Result<(), IgbError> {
        self.igb
            .summary_mut()
            .set("life_count", SummaryValue::Int(i32::from(count)))
    }

    /// Marks the scenes of the `start` step and of `boss`'s step as seen, in
    /// table order, skipping the ones already seen.
    pub(crate) fn skip_story_cutscenes(&mut self, boss: LevelId) -> Result<(), IgbError> {
        let steps = CUTSCENES
            .iter()
            .filter(|s| s.step == "start" || s.boss() == Some(boss));
        for &name in steps.flat_map(|s| s.scenes) {
            if !self.cutscene_seen(name) {
                self.set_cutscene(name, true)?;
            }
        }
        Ok(())
    }

    /// Opens warp room `room` (2–5) by defeating the bosses of the rooms
    /// before it. With `skip_cutscenes`, their story scenes are marked seen,
    /// also for bosses already defeated; with `with_abilities`, the abilities
    /// those bosses grant are turned on.
    pub(crate) fn unlock_warp_room(
        &mut self,
        room: u8,
        with_abilities: bool,
        skip_cutscenes: bool,
    ) -> Result<(), IgbError> {
        let n = usize::from(room.saturating_sub(1)).min(BOSSES.len());
        for &boss in &BOSSES[..n] {
            if !self.boss_defeated(boss) {
                self.set_boss_defeated(boss, true);
            }
            if skip_cutscenes {
                self.skip_story_cutscenes(boss)?;
            }
        }
        if with_abilities {
            for &ability in &ABILITIES[..n] {
                self.set_ability(ability, true);
            }
        }
        Ok(())
    }
}
