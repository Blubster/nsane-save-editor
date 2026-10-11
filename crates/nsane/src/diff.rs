//! What changed between two saves, in game terms: [`diff`] compares the game
//! data field by field, and [`summarize`] turns the result into one readable
//! row per level, boss, ability, colored gem, scene or summary key.
//!
//! Bytes are not compared: two saves with the same keys in different hash
//! table slots, or bools that differ only in their padding bytes, are equal.

use std::collections::BTreeMap;
use std::fmt;

use igb::{FieldValue, IgObject, SummaryValue, latin1};

use crate::crash3::format_time;
use crate::crash3::tables::{
    BONUS_GEM_KEY, SECRET_ENDING_KEY, bit, cutscene_name, field, relic_counted_level,
};
use crate::crash3::{
    ABILITIES, Ability, Crash3Save, GemColor, Kind, LEVELS, LevelId, Relic, is_zone_path,
};

/// A session field's value, as compared by [`diff`].
#[derive(Debug, Clone, PartialEq)]
pub enum FieldText {
    /// An int, enum or unsigned int field.
    Int(i64),
    /// A bool field's value byte. The padding bytes are not part of it.
    Bool(u8),
    /// A string field, by its text. `None` for the null string.
    Text(Option<String>),
    /// Any other field type, compared as stored.
    Raw(FieldValue),
    /// The session has no such field, or the save has no such zone.
    Absent,
}

impl FieldText {
    fn new(save: &Crash3Save, value: Option<&FieldValue>) -> FieldText {
        match value {
            Some(&FieldValue::Int(v) | &FieldValue::Enum(v)) => FieldText::Int(i64::from(v)),
            Some(&FieldValue::UInt(v)) => FieldText::Int(i64::from(v)),
            Some(&FieldValue::Bool { value, .. }) => FieldText::Bool(value),
            Some(&FieldValue::Str(i)) => FieldText::Text(save.igb().string(i).map(latin1::decode)),
            Some(&other) => FieldText::Raw(other),
            None => FieldText::Absent,
        }
    }

    /// The value of a number or bool field.
    pub fn int(&self) -> Option<i64> {
        match *self {
            FieldText::Int(v) => Some(v),
            FieldText::Bool(v) => Some(i64::from(v)),
            _ => None,
        }
    }
}

impl fmt::Display for FieldText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FieldText::Int(v) => write!(f, "{v}"),
            FieldText::Bool(v) => write!(f, "{v}"),
            FieldText::Text(Some(s)) => write!(f, "{s:?}"),
            FieldText::Text(None) => f.write_str("null"),
            FieldText::Raw(v) => write!(f, "{v:?}"),
            FieldText::Absent => f.write_str("none"),
        }
    }
}

/// One difference between two saves. `None` means the key is absent on that
/// side.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    Summary {
        key: String,
        before: Option<SummaryValue>,
        after: Option<SummaryValue>,
    },
    /// A field of a zone session. `level` is set for the zones of
    /// [`LEVELS`].
    Zone {
        zone: String,
        level: Option<LevelId>,
        field: usize,
        before: FieldText,
        after: FieldText,
    },
    /// A game variable, by handle name, with its value as text.
    Variable {
        name: String,
        before: Option<String>,
        after: Option<String>,
    },
    /// A key of the one-shot flag table.
    BoolFlag {
        key: String,
        before: Option<u32>,
        after: Option<u32>,
    },
    /// A key of the in-level checkpoint table.
    IntFlag {
        key: String,
        before: Option<u32>,
        after: Option<u32>,
    },
}

/// How the save review groups changes, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChangeGroup {
    Levels,
    Bosses,
    Abilities,
    ColoredGems,
    Cutscenes,
    Summary,
    Other,
}

impl ChangeGroup {
    pub fn name(self) -> &'static str {
        match self {
            ChangeGroup::Levels => "Levels",
            ChangeGroup::Bosses => "Bosses",
            ChangeGroup::Abilities => "Abilities",
            ChangeGroup::ColoredGems => "Colored gems",
            ChangeGroup::Cutscenes => "Scenes",
            ChangeGroup::Summary => "Summary",
            ChangeGroup::Other => "Other",
        }
    }
}

/// What a review row is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    /// A level or a boss: its session and its `<Name>_RelicCounted` flag.
    Level(LevelId),
    Ability(Ability),
    /// The `Gem_<Color>` variable. The gem's bit is in its level's row.
    ColoredGem(GemColor),
    /// A hub story scene, by name.
    Cutscene(String),
    /// A summary key.
    Summary(String),
    /// A zone outside the level table, by path.
    Zone(String),
    Variable(String),
    BoolFlag(String),
    IntFlag(String),
}

/// One line of the save review: every change to one subject, merged.
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewRow {
    pub group: ChangeGroup,
    pub subject: Subject,
    /// "Toad Village", "Speed Shoes", "Lives"…
    pub title: String,
    /// "relic sapphire → gold", "times 1:00.23 → 0:55.44, 1:00.23"…
    pub details: Vec<String>,
}

impl ReviewRow {
    /// The details, joined by "; ".
    pub fn text(&self) -> String {
        self.details.join("; ")
    }
}

impl fmt::Display for ReviewRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.title, self.text())
    }
}

// ------------------------------------------------------------------ diff

/// The zone sessions by path. Paths are unique in every save; if one were
/// repeated, the last session would count, as for the level table.
fn zone_map(save: &Crash3Save) -> BTreeMap<&str, &IgObject> {
    save.zones().collect()
}

fn variable_text(value: FieldValue) -> String {
    match value {
        FieldValue::Int(v) | FieldValue::Enum(v) => v.to_string(),
        FieldValue::UInt(v) => v.to_string(),
        FieldValue::Bool { value, .. } => value.to_string(),
        FieldValue::Float(bits) => f32::from_bits(bits).to_string(),
        other => format!("{other:?}"),
    }
}

/// `(key, before, after)` for every key whose value differs, in key order.
fn map_diff<K: Ord + Clone, V: PartialEq + Clone>(
    a: &BTreeMap<K, V>,
    b: &BTreeMap<K, V>,
) -> Vec<(K, Option<V>, Option<V>)> {
    let mut keys: Vec<&K> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|k| {
            let (x, y) = (a.get(k), b.get(k));
            (x != y).then(|| (k.clone(), x.cloned(), y.cloned()))
        })
        .collect()
}

/// The differences in game data from `a` to `b`: the summary in `a`'s key
/// order (then keys only `b` has), the zone sessions in [`LEVELS`] order
/// then the other zones by path, then the variables, the flag table and the
/// checkpoint table, each sorted by name.
pub fn diff(a: &Crash3Save, b: &Crash3Save) -> Vec<Change> {
    let mut changes = Vec::new();

    let (sa, sb) = (a.igb().summary(), b.igb().summary());
    let keys = sa.entries().iter().chain(sb.entries()).map(|e| &e.key);
    let mut seen = Vec::new();
    for key in keys {
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let key = latin1::decode(key);
        let (before, after) = (sa.get(&key), sb.get(&key));
        if before != after {
            changes.push(Change::Summary {
                key,
                before: before.cloned(),
                after: after.cloned(),
            });
        }
    }

    let (za, zb) = (zone_map(a), zone_map(b));
    let mut paths: Vec<&str> = LEVELS.iter().map(|l| l.zone).collect();
    let mut others: Vec<&str> = za.keys().chain(zb.keys()).copied().collect();
    others.sort_unstable();
    others.dedup();
    others.retain(|p| !paths.contains(p));
    paths.extend(others);
    for zone in paths {
        let (x, y) = (za.get(zone), zb.get(zone));
        let len = x
            .map_or(0, |s| s.fields.len())
            .max(y.map_or(0, |s| s.fields.len()));
        for i in 0..len {
            let before = FieldText::new(a, x.and_then(|s| s.field(i)));
            let after = FieldText::new(b, y.and_then(|s| s.field(i)));
            if before != after {
                changes.push(Change::Zone {
                    zone: zone.to_owned(),
                    level: LevelId::from_zone(zone),
                    field: i,
                    before,
                    after,
                });
            }
        }
    }

    let vars = |s: &Crash3Save| -> BTreeMap<String, String> {
        s.variables()
            .map(|(name, v)| (name.to_owned(), variable_text(v)))
            .collect()
    };
    for (name, before, after) in map_diff(&vars(a), &vars(b)) {
        changes.push(Change::Variable {
            name,
            before,
            after,
        });
    }
    for (key, before, after) in map_diff(&a.flag_entries(), &b.flag_entries()) {
        changes.push(Change::BoolFlag { key, before, after });
    }
    for (key, before, after) in map_diff(&a.int_flag_entries(), &b.int_flag_entries()) {
        changes.push(Change::IntFlag { key, before, after });
    }
    changes
}

// ----------------------------------------------------------- describing

/// The save slot names of the summary keys `docs/FORMAT.md` explains.
fn summary_label(key: &str) -> Option<&'static str> {
    Some(match key {
        "crystal_count" => "Crystals",
        "gem_count" => "Gems",
        "sapphire_count" => "Sapphire relics",
        "gold_count" => "Gold relics",
        "platinum_count" => "Platinum relics",
        "percent_complete" => "Percentage",
        "life_count" => "Lives",
        "farthest_progression_index" => "Furthest progression",
        "story_completed" => "Story completed",
        "previous_level_name" => "Last level entered",
        "previous_level_progression_index" => "Last level entered (progression index)",
        _ => return None,
    })
}

fn summary_text(key: &str, value: Option<&SummaryValue>) -> String {
    match value {
        None => "none".to_owned(),
        Some(SummaryValue::Int(v)) => v.to_string(),
        Some(SummaryValue::Float(v)) if key == "percent_complete" => format!("{v}%"),
        Some(SummaryValue::Float(v)) => v.to_string(),
        Some(SummaryValue::Str(s)) if key == "previous_level_name" => {
            last_level_text(&latin1::decode(s))
        }
        Some(SummaryValue::Str(s)) => latin1::decode(s),
    }
}

/// How the last level entered reads: the level's name, another zone's path,
/// "none", or "unknown" for leftover bytes that aren't a path.
pub fn last_level_text(zone: &str) -> String {
    if zone.is_empty() {
        "none".to_owned()
    } else if let Some(level) = LevelId::from_zone(zone) {
        level.name().to_owned()
    } else if is_zone_path(zone) {
        zone.to_owned()
    } else {
        "unknown".to_owned()
    }
}

fn on_off(value: Option<&str>) -> &'static str {
    match value {
        None => "none",
        Some("0") => "off",
        Some(_) => "on",
    }
}

/// "set", "removed", or both values.
fn presence(before: Option<u32>, after: Option<u32>) -> String {
    match (before, after) {
        (None, Some(1)) => "set".to_owned(),
        (Some(_), None) => "removed".to_owned(),
        (b, a) => format!("{} → {}", opt_text(b), opt_text(a)),
    }
}

fn opt_text(v: Option<u32>) -> String {
    v.map_or_else(|| "none".to_owned(), |v| v.to_string())
}

fn relic_text(tier: Option<i64>) -> String {
    match tier {
        Some(0) => "none".to_owned(),
        Some(t) => i32::try_from(t)
            .ok()
            .and_then(Relic::from_tier)
            .map_or_else(|| format!("tier {t}"), |r| r.key().to_owned()),
        None => "none".to_owned(),
    }
}

fn time_text(cs: Option<i64>) -> String {
    match cs.and_then(|t| u32::try_from(t).ok()) {
        Some(t) if t > 0 => format_time(t),
        _ => "none".to_owned(),
    }
}

fn times_text(times: &[i64]) -> String {
    let times: Vec<String> = times
        .iter()
        .filter(|&&t| t > 0)
        .map(|&t| time_text(Some(t)))
        .collect();
    if times.is_empty() {
        "none".to_owned()
    } else {
        times.join(", ")
    }
}

/// Collectible bits in display order, with their names.
const BITS: [(u32, &str); 12] = [
    (bit::CRYSTAL, "crystal"),
    (bit::GEM_CLEAR, "clear gem"),
    (bit::GEM_SECOND, "second gem"),
    (bit::GEM_RED, "red gem"),
    (bit::GEM_YELLOW, "yellow gem"),
    (bit::GEM_PURPLE, "purple gem"),
    (bit::GEM_BLUE, "blue gem"),
    (bit::GEM_GREEN, "green gem"),
    (bit::BOSS_DEFEATED, "defeated"),
    (bit::RELIC_SAPPHIRE, "sapphire relic"),
    (bit::RELIC_GOLD, "gold relic"),
    (bit::RELIC_PLATINUM, "platinum relic"),
];

/// "+ crystal, - clear gem, defeated" for the bits that differ, leaving out
/// those in `skip`. The boss bit reads "defeated" / "not defeated".
fn bits_text(before: u32, after: u32, skip: u32) -> String {
    let changed = (before ^ after) & !skip;
    let named = BITS
        .iter()
        .filter(|(b, _)| changed & b != 0)
        .map(|&(b, name)| {
            let on = after & b != 0;
            match b {
                bit::BOSS_DEFEATED if on => name.to_owned(),
                bit::BOSS_DEFEATED => format!("not {name}"),
                _ => format!("{} {name}", if on { '+' } else { '-' }),
            }
        });
    let unknown = (0..32)
        .map(|i| 1u32 << i)
        .filter(|b| changed & b & !bit::KNOWN != 0)
        .map(|b| format!("{} bit {b:#06x}", if after & b != 0 { '+' } else { '-' }));
    named.chain(unknown).collect::<Vec<_>>().join(", ")
}

fn visit_text(state: Option<(i64, i64)>, completions: Option<(i64, i64)>) -> String {
    let mut parts = Vec::new();
    if let Some((b, a)) = state {
        parts.push(match (b >= 1, a >= 1) {
            (false, true) => "entered".to_owned(),
            (true, false) => "not entered".to_owned(),
            _ => format!("state {b} → {a}"),
        });
    }
    if let Some((b, a)) = completions {
        parts.push(format!("completions {b} → {a}"));
    }
    parts.join(", ")
}

/// The detail of one session field change of a level.
fn level_field_text(i: usize, before: &FieldText, after: &FieldText) -> String {
    let (b, a) = (before.int(), after.int());
    let both = |name: &str| format!("{name} {before} → {after}");
    match i {
        field::STATE => visit_text(b.zip(a), None),
        field::COMPLETIONS => visit_text(None, b.zip(a)),
        field::RELIC_TIER => format!("relic {} → {}", relic_text(b), relic_text(a)),
        field::COLLECTIBLES => match (b, a) {
            (Some(b), Some(a)) => bits_text(b as u32, a as u32, 0),
            _ => both(field::NAMES[i]),
        },
        field::CRYSTAL => both("crystal field"),
        _ => match field::TIMES.iter().position(|&t| t == i) {
            Some(n) => format!("time {} {} → {}", n + 1, time_text(b), time_text(a)),
            None => both(field::NAMES.get(i).copied().unwrap_or("field")),
        },
    }
}

fn ability_of(variable: &str) -> Option<Ability> {
    ABILITIES.into_iter().find(|a| a.variable() == variable)
}

fn color_of(variable: &str) -> Option<GemColor> {
    GemColor::ALL.into_iter().find(|c| c.variable() == variable)
}

fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| {
        c.to_uppercase().chain(chars).collect::<String>()
    })
}

impl Change {
    pub fn group(&self) -> ChangeGroup {
        match self.subject() {
            Subject::Level(level) if level.level().kind == Kind::Boss => ChangeGroup::Bosses,
            Subject::Level(_) => ChangeGroup::Levels,
            Subject::Ability(_) => ChangeGroup::Abilities,
            Subject::ColoredGem(_) => ChangeGroup::ColoredGems,
            Subject::Cutscene(_) => ChangeGroup::Cutscenes,
            Subject::Summary(_) => ChangeGroup::Summary,
            Subject::Zone(_)
            | Subject::Variable(_)
            | Subject::BoolFlag(_)
            | Subject::IntFlag(_) => ChangeGroup::Other,
        }
    }

    /// The review row this change belongs to.
    pub fn subject(&self) -> Subject {
        match self {
            Change::Summary { key, .. } => Subject::Summary(key.clone()),
            Change::Zone {
                level: Some(level), ..
            } => Subject::Level(*level),
            Change::Zone { zone, .. } => Subject::Zone(zone.clone()),
            Change::Variable { name, .. } => match (ability_of(name), color_of(name)) {
                (Some(a), _) => Subject::Ability(a),
                (_, Some(c)) => Subject::ColoredGem(c),
                _ => Subject::Variable(name.clone()),
            },
            Change::BoolFlag { key, .. } => match (cutscene_name(key), relic_counted_level(key)) {
                (Some(name), _) => Subject::Cutscene(name.to_owned()),
                (_, Some(level)) => Subject::Level(level),
                _ => Subject::BoolFlag(key.clone()),
            },
            Change::IntFlag { key, .. } => Subject::IntFlag(key.clone()),
        }
    }

    /// One line a player can read, such as "Deep Trouble: + crystal, + red
    /// gem" or "Scene C3_NGin_Intro: seen".
    pub fn describe(&self) -> String {
        format!("{}: {}", self.subject().title(), self.detail())
    }

    /// The part of [`describe`](Self::describe) after the subject.
    fn detail(&self) -> String {
        match self {
            Change::Summary { key, before, after } => format!(
                "{} → {}",
                summary_text(key, before.as_ref()),
                summary_text(key, after.as_ref())
            ),
            Change::Zone {
                level: Some(_),
                field,
                before,
                after,
                ..
            } => level_field_text(*field, before, after),
            Change::Zone {
                field,
                before,
                after,
                ..
            } => format!("field {field} {before} → {after}"),
            Change::Variable {
                name,
                before,
                after,
            } => {
                let (b, a) = (before.as_deref(), after.as_deref());
                if ability_of(name).is_some() {
                    format!("{} → {}", on_off(b), on_off(a))
                } else if color_of(name).is_some() {
                    format!("variable {} → {}", on_off(b), on_off(a))
                } else {
                    format!("{} → {}", b.unwrap_or("none"), a.unwrap_or("none"))
                }
            }
            Change::BoolFlag { key, before, after } => {
                let (b, a) = (*before, *after);
                if cutscene_name(key).is_some() {
                    match (b.is_some_and(|v| v != 0), a.is_some_and(|v| v != 0)) {
                        (false, true) => "seen".to_owned(),
                        (true, false) => "not seen".to_owned(),
                        _ => format!("flag {}", presence(b, a)),
                    }
                } else if relic_counted_level(key).is_some() {
                    format!("relic counted flag {}", presence(b, a))
                } else if key == BONUS_GEM_KEY {
                    match (b.is_some_and(|v| v != 0), a.is_some_and(|v| v != 0)) {
                        (false, true) => "taken".to_owned(),
                        (true, false) => "not taken".to_owned(),
                        _ => format!("flag {}", presence(b, a)),
                    }
                } else {
                    presence(b, a)
                }
            }
            Change::IntFlag { before, after, .. } => {
                format!("{} → {}", opt_text(*before), opt_text(*after))
            }
        }
    }
}

impl Subject {
    pub fn title(&self) -> String {
        match self {
            Subject::Level(level) => level.name().to_owned(),
            Subject::Ability(ability) => ability.name().to_owned(),
            Subject::ColoredGem(color) => format!("{} gem", capitalized(color.key())),
            Subject::Cutscene(name) => format!("Scene {name}"),
            Subject::Summary(key) => summary_label(key).unwrap_or(key).to_owned(),
            Subject::Zone(zone) => format!("Zone {zone}"),
            Subject::Variable(name) => format!("Variable {name}"),
            Subject::BoolFlag(key) if key == BONUS_GEM_KEY => "Bonus gem".to_owned(),
            Subject::BoolFlag(key) if key == SECRET_ENDING_KEY => "Secret ending flag".to_owned(),
            Subject::BoolFlag(key) => format!("Flag {key}"),
            Subject::IntFlag(key) => format!("In-level state {key}"),
        }
    }
}

// ------------------------------------------------------------ summarize

/// Groups `changes` into one row per subject, merging related fields:
/// `relic_tier` and the relic bit read "relic sapphire → gold", the times
/// list their whole leaderboard (initials aren't shown), `state` and
/// `completions` read "entered, completions 0 → 1", and the `crystal` field
/// is folded into the crystal bit.
///
/// `save` is the save the changes lead to: the leaderboard of a level is
/// read from it for the slots that didn't change. Rows are in
/// [`ChangeGroup`] order, then in the order of `changes`.
pub fn summarize(changes: &[Change], save: &Crash3Save) -> Vec<ReviewRow> {
    let mut rows: Vec<(Subject, Vec<&Change>)> = Vec::new();
    for change in changes {
        let subject = change.subject();
        match rows.iter_mut().find(|(s, _)| *s == subject) {
            Some((_, list)) => list.push(change),
            None => rows.push((subject, vec![change])),
        }
    }
    let mut rows: Vec<ReviewRow> = rows
        .into_iter()
        .map(|(subject, list)| {
            let details = match subject {
                Subject::Level(level) => level_details(level, &list, save),
                _ => list.iter().map(|c| c.detail()).collect(),
            };
            ReviewRow {
                group: list[0].group(),
                title: subject.title(),
                subject,
                details,
            }
        })
        .collect();
    rows.sort_by_key(|r| r.group);
    rows
}

/// The merged details of one level's changes, in this order: collectibles,
/// relic, times, visit, then anything else.
fn level_details(level: LevelId, changes: &[&Change], save: &Crash3Save) -> Vec<String> {
    let mut fields: BTreeMap<usize, (&FieldText, &FieldText)> = BTreeMap::new();
    let mut relic_flag = None;
    for change in changes {
        match change {
            Change::Zone {
                field,
                before,
                after,
                ..
            } => {
                fields.insert(*field, (before, after));
            }
            Change::BoolFlag { before, after, .. } => relic_flag = Some((*before, *after)),
            _ => {}
        }
    }
    let int_pair = |i: usize| {
        let (b, a) = fields.get(&i)?;
        b.int().zip(a.int())
    };
    let mut details = Vec::new();
    let mut rest = Vec::new();

    let relic = int_pair(field::RELIC_TIER);
    let crystal_field = int_pair(field::CRYSTAL);
    if let Some((b, a)) = int_pair(field::COLLECTIBLES) {
        let (b, a) = (b as u32, a as u32);
        let skip = if relic.is_some() { bit::RELICS } else { 0 };
        let text = bits_text(b, a, skip);
        if !text.is_empty() {
            details.push(text);
        }
        let crystal_bit = ((b & bit::CRYSTAL != 0), (a & bit::CRYSTAL != 0));
        if let Some((fb, fa)) = crystal_field
            && (crystal_bit.0 == crystal_bit.1 || (fb != 0, fa != 0) != crystal_bit)
        {
            rest.push(level_field_text(
                field::CRYSTAL,
                &FieldText::Int(fb),
                &FieldText::Int(fa),
            ));
        }
    } else if crystal_field.is_some() {
        let (b, a) = fields[&field::CRYSTAL];
        rest.push(level_field_text(field::CRYSTAL, b, a));
    }

    if let Some((b, a)) = relic {
        details.push(format!(
            "relic {} → {}",
            relic_text(Some(b)),
            relic_text(Some(a))
        ));
    }
    if let Some((b, a)) = relic_flag {
        let has_relic = relic.map_or(save.relic(level).is_some(), |(_, a)| a != 0);
        if relic.is_none() || a.is_some() != has_relic {
            rest.push(format!("relic counted flag {}", presence(b, a)));
        }
    }

    let session = save.session(level);
    let now = |i: usize| match session.and_then(|s| s.field(i)) {
        Some(v) => FieldText::new(save, Some(v)).int().unwrap_or(0),
        None => 0,
    };
    if field::TIMES.iter().any(|i| fields.contains_key(i)) {
        let after: Vec<i64> = field::TIMES.iter().map(|&i| now(i)).collect();
        let before: Vec<i64> = field::TIMES
            .iter()
            .zip(&after)
            .map(|(i, &a)| fields.get(i).and_then(|(b, _)| b.int()).unwrap_or(a))
            .collect();
        details.push(format!(
            "times {} → {}",
            times_text(&before),
            times_text(&after)
        ));
    }

    let visit = visit_text(int_pair(field::STATE), int_pair(field::COMPLETIONS));
    if !visit.is_empty() {
        details.push(visit);
    }

    let merged = [
        field::STATE,
        field::COMPLETIONS,
        field::RELIC_TIER,
        field::CRYSTAL,
        field::COLLECTIBLES,
    ];
    for (&i, &(b, a)) in &fields {
        let handled = (merged.contains(&i) && b.int().is_some() && a.int().is_some())
            || field::TIMES.contains(&i)
            || field::INITIALS.contains(&i);
        if !handled {
            rest.push(level_field_text(i, b, a));
        }
    }
    details.extend(rest);
    if details.is_empty() {
        // Only leaderboard initials changed.
        details.push("leaderboard initials".to_owned());
    }
    details
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_names() {
        assert_eq!(
            bits_text(0, bit::CRYSTAL | bit::GEM_RED, 0),
            "+ crystal, + red gem"
        );
        assert_eq!(bits_text(bit::GEM_CLEAR, 0, 0), "- clear gem");
        assert_eq!(bits_text(0, bit::BOSS_DEFEATED, 0), "defeated");
        assert_eq!(bits_text(bit::BOSS_DEFEATED, 0, 0), "not defeated");
        assert_eq!(bits_text(0, 0x0080, 0), "+ bit 0x0080");
        assert_eq!(
            bits_text(bit::RELIC_SAPPHIRE, bit::RELIC_GOLD, bit::RELICS),
            ""
        );
    }

    #[test]
    fn field_texts() {
        let int = FieldText::Int;
        assert_eq!(
            level_field_text(5, &int(6023), &int(5544)),
            "time 1 1:00.23 → 0:55.44"
        );
        assert_eq!(
            level_field_text(6, &int(6023), &int(0)),
            "time 2 1:00.23 → none"
        );
        assert_eq!(
            level_field_text(11, &int(0), &int(3)),
            "relic none → platinum"
        );
        assert_eq!(level_field_text(2, &int(-1), &int(1)), "entered");
        assert_eq!(level_field_text(3, &int(4), &int(5)), "completions 4 → 5");
        assert_eq!(
            level_field_text(
                9,
                &FieldText::Text(Some("---".into())),
                &FieldText::Text(Some(String::new()))
            ),
            "initials_2 \"---\" → \"\""
        );
        assert_eq!(level_field_text(4, &int(0), &int(7)), "unknown_4 0 → 7");
    }
}
