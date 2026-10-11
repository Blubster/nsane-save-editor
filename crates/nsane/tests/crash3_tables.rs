//! The hand-written Crash 3 tables, field by field against
//! `tests/golden/crash3_tables.json`, plus their own consistency rules.

use std::path::Path;

use nsane::crash3::tables::{
    MAX_PERCENT, cutscene_key, field, is_cutscene, relic_counted_key, relic_times,
};
use nsane::crash3::{
    ABILITIES, BOSSES, CUTSCENES, Kind, LEVELS, LevelId, PROGRESSION, RELIC_TIMES, Relic,
    SECRET_ROOM, format_time, parse_time, relic_for_time,
};
use serde_json::{Value, json};

fn tables() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/crash3_tables.json");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap()
}

fn keys(ids: &[LevelId]) -> Value {
    json!(ids.iter().map(|id| id.key()).collect::<Vec<_>>())
}

#[test]
fn tables_match_the_golden_json() {
    let t = tables();

    let levels: Vec<Value> = LEVELS
        .iter()
        .map(|lv| {
            json!({
                "number": lv.number,
                "key": lv.key,
                "name": lv.name,
                "zone": lv.zone,
                "warp_room": lv.warp_room,
                "kind": lv.kind.key(),
                "second_gem": lv.second_gem,
                "colored_gem": lv.colored_gem.map(|c| c.key()),
                "has_crystal": lv.has_crystal(),
            })
        })
        .collect();
    assert_eq!(json!(levels), t["levels"]);

    assert_eq!(keys(&BOSSES), t["bosses"]);
    assert_eq!(keys(&PROGRESSION), t["progression"]);

    let secret: Vec<Value> = SECRET_ROOM
        .iter()
        .map(|s| json!({"number": s.number, "relics": s.relics, "level": s.level.key()}))
        .collect();
    assert_eq!(json!(secret), t["secret_room"]);

    let relics: Vec<Value> = RELIC_TIMES
        .iter()
        .map(|r| {
            json!({"level": r.level.key(), "platinum": r.platinum, "gold": r.gold,
                   "sapphire": r.sapphire})
        })
        .collect();
    assert_eq!(json!(relics), t["relic_times"]);

    let abilities: Vec<Value> = ABILITIES
        .iter()
        .map(|a| json!({"key": a.key(), "name": a.name(), "variable": a.variable()}))
        .collect();
    assert_eq!(json!(abilities), t["abilities"]);

    let cutscenes: Vec<Value> = CUTSCENES
        .iter()
        .map(|s| {
            let scenes: Vec<Value> = s
                .scenes
                .iter()
                .map(|&name| json!({"name": name, "key": cutscene_key(name)}))
                .collect();
            json!({"step": s.step, "scenes": scenes})
        })
        .collect();
    assert_eq!(json!(cutscenes), t["cutscenes"]);
}

#[test]
fn relic_thresholds_are_ordered_and_cover_every_level() {
    let with_relic: Vec<LevelId> = LevelId::all().filter(|id| id.level().has_relic()).collect();
    assert_eq!(with_relic.len(), 31);
    assert_eq!(
        RELIC_TIMES.iter().map(|r| r.level).collect::<Vec<_>>(),
        with_relic
    );
    for r in &RELIC_TIMES {
        assert!(
            r.platinum < r.gold && r.gold < r.sapphire,
            "{}",
            r.level.key()
        );
    }
    for boss in BOSSES {
        assert!(relic_times(boss).is_none());
        assert_eq!(relic_for_time(boss, 1), None);
    }
}

#[test]
fn relic_times_match_what_the_game_showed() {
    // Toad Village: "less than 1:03.00" for sapphire, "less than 0:57.50" for
    // gold; 1:00.23 earned the sapphire and 0:55.44 the gold (saves 0010, 0011).
    let toad = LevelId::from_key("toadvillage").unwrap();
    assert_eq!(
        relic_for_time(toad, parse_time("1:00:23").unwrap()),
        Some(Relic::Sapphire)
    );
    assert_eq!(
        relic_for_time(toad, parse_time("0:55:44").unwrap()),
        Some(Relic::Gold)
    );
    assert_eq!(relic_for_time(toad, parse_time("1:03:01").unwrap()), None);
    assert_eq!(relic_for_time(toad, 6300), None);
    assert_eq!(relic_for_time(toad, 6299), Some(Relic::Sapphire));
    assert_eq!(relic_for_time(toad, 4402), Some(Relic::Platinum));
    assert_eq!(format_time(6023), "1:00.23");
}

#[test]
fn table_invariants() {
    assert_eq!(LEVELS.len(), 36);
    for (i, id) in LevelId::all().enumerate() {
        assert_eq!(id.index(), i);
        assert_eq!(LevelId::from_key(id.key()), Some(id));
        assert_eq!(LevelId::from_zone(id.level().zone), Some(id));
    }
    assert_eq!(LevelId::new(36), None);
    assert_eq!(LevelId::from_key("27"), None);

    for (i, id) in PROGRESSION.iter().enumerate() {
        assert_eq!(id.progression_index(), Some(i + 1));
        assert!(id.level().has_crystal() || id.level().kind == Kind::Boss);
    }
    for (room, boss) in (1u8..).zip(BOSSES) {
        assert_eq!(boss.level().kind, Kind::Boss);
        assert_eq!(boss.level().warp_room, room);
    }
    assert_eq!(
        LevelId::from_key("skicrazed").unwrap().progression_index(),
        None
    );

    assert_eq!(LEVELS.iter().filter(|l| l.has_crystal()).count(), 25);
    assert_eq!(LEVELS.iter().filter(|l| l.second_gem).count(), 10);
    assert_eq!(LEVELS.iter().filter(|l| l.colored_gem.is_some()).count(), 5);
    assert_eq!(MAX_PERCENT, 108);

    for step in &CUTSCENES[1..] {
        assert!(step.boss().is_some(), "{}", step.step);
    }
    assert_eq!(CUTSCENES[0].boss(), None);
    assert_eq!(CUTSCENES.iter().map(|s| s.scenes.len()).sum::<usize>(), 18);
    assert!(is_cutscene("C3_PreFinalBattle") && !is_cutscene("C3_Unknown"));

    let hang = LevelId::from_key("hangemhigh").unwrap();
    assert_eq!(relic_counted_key(hang.level()), "Hang'em High_RelicCounted");
    assert_eq!(field::TIMES, [5, 6, 7]);
    for r in Relic::ALL {
        assert_eq!(Relic::from_tier(r.tier()), Some(r));
    }
    assert_eq!(Relic::from_tier(0), None);
}

#[test]
fn flag_keys_read_back() {
    use nsane::crash3::tables::{cutscene_name, relic_counted_level};
    for step in CUTSCENES {
        for &name in step.scenes {
            assert_eq!(cutscene_name(&cutscene_key(name)), Some(name));
        }
    }
    assert_eq!(cutscene_name(&cutscene_key("C3_Other")), Some("C3_Other"));
    assert_eq!(cutscene_name("C3_NGin_Intro"), None);
    assert_eq!(cutscene_name("LastGemRewarded"), None);
    for level in LevelId::all() {
        assert_eq!(
            relic_counted_level(&relic_counted_key(level.level())),
            Some(level)
        );
    }
    assert_eq!(relic_counted_level("Nowhere_RelicCounted"), None);
}

#[test]
fn every_second_gem_and_colored_gem_has_its_hint() {
    use nsane::crash3::GemColor;
    use nsane::crash3::tables::{bit, colored_gem_opens, second_gem_hint};
    for level in LevelId::all() {
        assert_eq!(
            second_gem_hint(level).is_some(),
            level.level().second_gem,
            "{}",
            level.key()
        );
    }
    let bug_lite = LevelId::from_key("buglite").unwrap();
    for color in GemColor::ALL {
        let opens = colored_gem_opens(color);
        assert!(!opens.is_empty(), "{color:?}");
        // Bug Lite's multicolored path needs every gem.
        assert!(opens.iter().any(|p| p.level == bug_lite), "{color:?}");
        for path in opens {
            assert!(path.level.level().second_gem, "{}", path.level.key());
            // The gem's own path is the one the second gem hint names.
            if path.level != bug_lite {
                let hint = second_gem_hint(path.level).unwrap();
                assert_eq!(hint, format!("{} gem path", color.key()), "{color:?}");
            }
        }
    }
    // Every "<color> gem path" hint is listed in that gem's paths.
    for level in LevelId::all() {
        if let Some(color) = second_gem_hint(level)
            .and_then(|h| h.strip_suffix(" gem path"))
            .and_then(GemColor::from_key)
        {
            assert!(colored_gem_opens(color).iter().any(|p| p.level == level));
        }
    }
    let yellow = colored_gem_opens(GemColor::Yellow);
    assert_eq!(yellow[0].leads_to, LevelId::from_key("eggipusrex"));

    let mut all = 0;
    for (b, name) in bit::NAMES {
        assert_eq!(b.count_ones(), 1, "{name}");
        all |= b;
    }
    assert_eq!(all, bit::KNOWN);
}
