//! Edits against what the game itself wrote (`docs/FORMAT.md` §8, §10, §11):
//! the edit that reproduces a step of the playthrough must write what the
//! game wrote at that step.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use nsane::crash3::tables::{SECRET_ENDING_KEY, cutscene_key};
use nsane::crash3::{
    Ability, CUTSCENES, Crash3Save, Edit, EditError, Kind, LevelId, Relic, parse_time,
};
use nsane::igb::{FieldValue, IgbFile, SummaryValue, latin1};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(rel: &str) -> Crash3Save {
    let path = repo().join(rel);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Crash3Save::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn id(key: &str) -> LevelId {
    LevelId::from_key(key).unwrap()
}

fn by_number(n: u8) -> LevelId {
    LevelId::all()
        .find(|l| l.level().number == Some(n))
        .unwrap()
}

fn apply(save: &mut Crash3Save, edits: &[Edit]) {
    for edit in edits {
        save.apply(edit).unwrap_or_else(|e| panic!("{edit:?}: {e}"));
    }
}

/// Writes the save as the editor does, and opens the result.
fn reload(save: &mut Crash3Save) -> Crash3Save {
    Crash3Save::parse(&save.to_bytes().unwrap()).unwrap()
}

/// A level's session fields, with strings resolved. A bool's padding bytes
/// are uninitialised memory, not part of its value.
#[derive(Debug, PartialEq)]
enum Value {
    Str(Option<Vec<u8>>),
    Other(FieldValue),
}

fn zone_values(save: &Crash3Save, level: LevelId) -> Vec<Value> {
    save.session(level)
        .unwrap()
        .fields
        .iter()
        .map(|&f| match f {
            FieldValue::Str(i) => Value::Str(save.igb().string(i).map(<[u8]>::to_vec)),
            FieldValue::Bool { value, .. } => Value::Other(FieldValue::Bool { value, padding: 0 }),
            f => Value::Other(f),
        })
        .collect()
}

fn summary(save: &Crash3Save, key: &str) -> SummaryValue {
    save.igb().summary().get(key).unwrap().clone()
}

fn variable_on(save: &Crash3Save, name: &str) -> bool {
    matches!(save.variable(name), Some(FieldValue::Bool { value: 1, .. }))
}

#[test]
fn gold_relic_edit_reproduces_game() {
    let level = id("toadvillage");
    let mut save = load("saves/0010.sav");
    let times_cs = vec![
        parse_time("0:55:44").unwrap(),
        parse_time("1:00:23").unwrap(),
    ];
    apply(
        &mut save,
        &[
            Edit::SetRelic {
                level,
                relic: Some(Relic::Gold),
            },
            Edit::SetTimes { level, times_cs },
        ],
    );
    // The game also counts the finish in the completions counter, which no
    // edit writes.
    let session = save.session(level).unwrap().id;
    let mut igb = save.into_igb();
    let Some(FieldValue::Int(n)) = igb.object_mut(session).unwrap().field_mut(3) else {
        panic!("completions is not an int");
    };
    *n += 1;
    let mut save = Crash3Save::from_igb(igb).unwrap();

    let edited = reload(&mut save);
    let expected = load("saves/0011.sav");
    assert_eq!(zone_values(&edited, level), zone_values(&expected, level));
    assert_eq!(
        edited.igb().summary().entries(),
        expected.igb().summary().entries()
    );
}

#[test]
fn boss_and_ability_edit_reproduces_game() {
    let boss = id("tinytiger");
    let mut save = load("saves/0008.sav");
    apply(
        &mut save,
        &[
            Edit::SetBossDefeated {
                boss,
                on: true,
                skip_cutscenes: false,
            },
            Edit::SetAbility {
                ability: Ability::BodySlam,
                on: true,
            },
        ],
    );
    let edited = reload(&mut save);
    let expected = load("saves/0009.sav");
    assert_eq!(zone_values(&edited, boss), zone_values(&expected, boss));
    assert!(edited.ability(Ability::BodySlam));
    assert_eq!(
        summary(&edited, "percent_complete"),
        summary(&expected, "percent_complete")
    );
}

#[test]
fn crystal_and_gem_edit_reproduces_game() {
    let level = id("underpressure");
    let mut save = load("saves/0004.sav");
    apply(
        &mut save,
        &[
            Edit::SetCrystal { level, on: true },
            Edit::SetGem { level, on: true },
        ],
    );
    let edited = reload(&mut save);
    assert_eq!(
        zone_values(&edited, level),
        zone_values(&load("saves/0005.sav"), level)
    );
}

#[test]
fn full_completion_fits_in_file() {
    let mut save = load("saves/initial_game.sav");
    let mut edits = Vec::new();
    for level in LevelId::all().filter(|l| l.level().kind != Kind::Boss) {
        // Turning off a crystal a level doesn't have changes nothing: skipped.
        if level.level().has_crystal() {
            edits.push(Edit::SetCrystal { level, on: true });
        }
        edits.push(Edit::SetGem { level, on: true });
        edits.push(Edit::SetRelic {
            level,
            relic: Some(Relic::Platinum),
        });
        edits.push(Edit::SetTimes {
            level,
            times_cs: vec![1000, 2000, 3000],
        });
    }
    for boss in LevelId::all().filter(|l| l.level().kind == Kind::Boss) {
        edits.push(Edit::SetBossDefeated {
            boss,
            on: true,
            skip_cutscenes: false,
        });
    }
    apply(&mut save, &edits);
    let edited = reload(&mut save);
    assert_eq!(summary(&edited, "crystal_count"), SummaryValue::Int(25));
    for level in LevelId::all().filter(|l| l.level().kind == Kind::Level) {
        assert_eq!(
            edited.crystal(level),
            level.level().has_crystal(),
            "{level:?}"
        );
    }
}

#[test]
fn removing_crystal_clears_field12() {
    let level = id("toadvillage");
    let mut save = load("saves/0017.sav");
    assert!(save.crystal(level) && save.crystal_field(level));
    apply(&mut save, &[Edit::SetCrystal { level, on: false }]);
    let edited = reload(&mut save);
    assert!(!edited.crystal(level) && !edited.crystal_field(level));
}

/// Live test T4: crystals in levels 16–20 without raising this index made N.
/// Gin's portal glitch.
#[test]
fn completing_a_room_reaches_its_boss() {
    let mut save = load("saves/0019.sav");
    let edits: Vec<_> = (16..=20)
        .map(|n| Edit::SetCrystal {
            level: by_number(n),
            on: true,
        })
        .collect();
    apply(&mut save, &edits);
    assert_eq!(
        summary(&reload(&mut save), "farthest_progression_index"),
        SummaryValue::Int(24) // N. Gin
    );
}

/// `t5_after.sav`: the yellow gem picked up for real through level 27.
#[test]
fn editor_yellow_gem_matches_game() {
    let level = id("hangemhigh");
    let mut save = load("test_saves/t5_secret_room.sav");
    apply(&mut save, &[Edit::SetColoredGem { level, on: true }]);
    let game = load("test_saves/t5_after.sav");
    assert_eq!(save.flags(level), game.flags(level));
    assert!(variable_on(&save, "Gem_Yellow"));
    assert!(variable_on(&game, "Gem_Yellow"));
}

/// T7: the game gave the 47th gem with every relic gold, then Cortex was
/// beaten again.
#[test]
fn game_bonus_gem_and_cortex_rematch() {
    let game = load("test_saves/t7_after_beating_cortex.sav");
    assert!(game.bonus_gem() && game.flag(SECRET_ENDING_KEY));
    assert_eq!(summary(&game, "story_completed"), SummaryValue::Int(1));

    let mut save = load("test_saves/t7_bonus_gem.sav");
    apply(&mut save, &[Edit::SetBonusGem { on: true }]);
    let edited = reload(&mut save);
    assert_eq!(
        summary(&edited, "percent_complete"),
        SummaryValue::Float(108.0)
    );
    assert_eq!(
        summary(&game, "percent_complete"),
        SummaryValue::Float(108.0)
    );
    assert_eq!(summary(&edited, "story_completed"), SummaryValue::Int(1));
}

/// Every scene of the table marked seen gives exactly the 18 scene keys of a
/// 108% save made by another player.
#[test]
fn all_cutscenes_give_the_scene_keys_of_a_finished_game() {
    let mut save = load("saves/0019.sav");
    let edits: Vec<_> = CUTSCENES
        .iter()
        .flat_map(|step| step.scenes)
        .map(|name| Edit::SetCutscene {
            cutscene: (*name).to_owned(),
            seen: true,
        })
        .collect();
    apply(&mut save, &edits);
    let edited = reload(&mut save);

    let expected: BTreeSet<String> = CUTSCENES
        .iter()
        .flat_map(|step| step.scenes)
        .map(|name| cutscene_key(name))
        .collect();
    assert_eq!(expected.len(), 18);
    let prefix = cutscene_key("");
    let prefix = &prefix[..prefix.find(' ').unwrap()];
    let table = edited
        .igb()
        .objects_of("igStringBoolHashTable")
        .next()
        .unwrap()
        .id;
    let scenes: BTreeSet<String> = edited
        .igb()
        .hash_table(table)
        .unwrap()
        .items()
        .unwrap()
        .into_iter()
        .map(|(key, _)| latin1::decode(&key))
        .filter(|key| key.starts_with(prefix))
        .collect();
    assert_eq!(scenes, expected);
}

/// `t4d_after.sav`: N. Gin beaten for real, from a save whose room 4 crystals
/// were added by the editor.
#[test]
fn editor_boss_matches_game_ngin_win() {
    let boss = id("ngin");
    let mut save = load("test_saves/t4d_rooms1to4.sav");
    apply(
        &mut save,
        &[
            Edit::SetBossDefeated {
                boss,
                on: true,
                skip_cutscenes: true,
            },
            Edit::SetAbility {
                ability: Ability::Bazooka,
                on: true,
            },
        ],
    );
    let edited = reload(&mut save);
    let game = load("test_saves/t4d_after.sav");
    assert_eq!(edited.flags(boss), game.flags(boss));
    assert!(edited.ability(Ability::Bazooka) && game.ability(Ability::Bazooka));
    for key in ["percent_complete", "farthest_progression_index"] {
        assert_eq!(summary(&edited, key), summary(&game, key), "{key}");
    }
    for name in ["C3_PreNGinFight", "C3_PostNGinFight"] {
        assert!(
            edited.cutscene_seen(name) && game.cutscene_seen(name),
            "{name}"
        );
    }
}

/// PC saves are not padded to a fixed size: they grow with the data.
#[test]
fn pc_save_is_written_unpadded() {
    let raw = std::fs::read(repo().join("saves/ALCHEMYXSAVE_8.sav")).unwrap();
    assert!(!IgbFile::parse(&raw).unwrap().padded());
    assert!(load("saves/0019.sav").igb().padded());

    let mut save = Crash3Save::parse(&raw).unwrap();
    let edits: Vec<_> = LevelId::all()
        .filter(|l| l.level().kind != Kind::Boss)
        .map(|level| Edit::SetRelic {
            level,
            relic: Some(Relic::Platinum),
        })
        .collect();
    apply(&mut save, &edits);
    let data = save.to_bytes().unwrap();
    assert!(data.len() > raw.len());
    assert_eq!(IgbFile::parse(&data).unwrap().build().unwrap(), data);
    assert_eq!(Crash3Save::parse(&data).unwrap().totals().platinum, 31);
}

/// 30 gold or platinum relics are enough for the bonus gem: one relic may stay
/// sapphire, not two.
#[test]
fn bonus_gem_due_after_relic_edits() {
    let mut save = load("test_saves/t7_bonus_gem.sav");
    assert!(save.bonus_gem_due());
    apply(
        &mut save,
        &[Edit::SetRelic {
            level: id("futuretense"),
            relic: Some(Relic::Sapphire),
        }],
    );
    assert!(save.bonus_gem_due());
    apply(
        &mut save,
        &[Edit::SetRelic {
            level: id("hotcoco"),
            relic: Some(Relic::Sapphire),
        }],
    );
    assert!(!save.bonus_gem_due());
}

// ------------------------------------------------------------ apply checks

#[test]
fn failing_apply_leaves_the_save_unchanged() {
    let original = load("saves/0019.sav");
    let mut save = original.clone();
    let err = save
        .apply(&Edit::SetCrystal {
            level: id("hotcoco"),
            on: true,
        })
        .unwrap_err();
    assert_eq!(err.code(), "not_applicable");
    assert_eq!(save, original);

    // A valid edit before a failing one stays applied; nothing of the failing
    // one is written.
    let edits = [
        Edit::SetGem {
            level: id("toadvillage"),
            on: false,
        },
        Edit::SetTimes {
            level: id("toadvillage"),
            times_cs: vec![1000, 2000, 3000, 4000],
        },
    ];
    let mut expected = original.clone();
    expected.apply(&edits[0]).unwrap();
    assert_eq!(save.apply_all(&edits).unwrap_err().code(), "invalid_value");
    assert_eq!(save, expected);
}

/// Edits built in code get the checks the JSON reader makes on ids.
#[test]
fn apply_checks_the_kind_of_each_id() {
    let mut save = load("saves/0019.sav");
    let before = save.clone();
    let cases = [
        (
            Edit::SetGem {
                level: id("tinytiger"),
                on: true,
            },
            "unknown_level",
        ),
        (
            Edit::SetBossDefeated {
                boss: id("toadvillage"),
                on: true,
                skip_cutscenes: false,
            },
            "unknown_boss",
        ),
        (
            Edit::SetCutscene {
                cutscene: "start".to_owned(),
                seen: true,
            },
            "unknown_cutscene",
        ),
        (
            Edit::UnlockWarpRoom {
                room: 6,
                with_abilities: false,
                skip_cutscenes: false,
            },
            "invalid_value",
        ),
        (
            Edit::RawFlagSet {
                key: "a\0b".to_owned(),
            },
            "invalid_value",
        ),
        (
            Edit::SetTimes {
                level: id("toadvillage"),
                times_cs: vec![0],
            },
            "invalid_value",
        ),
    ];
    for (edit, code) in cases {
        assert_eq!(
            save.apply(&edit).map_err(|e| e.code()),
            Err(code),
            "{edit:?}"
        );
    }
    assert_eq!(save, before);
    // A boss can be marked entered.
    save.apply(&Edit::MarkEntered {
        level: id("tinytiger"),
    })
    .unwrap();
}

#[test]
fn edits_read_from_json_with_the_contract_codes() {
    let cases = [
        (
            r#"{"op": "set_lives", "count": 255}"#,
            Ok(Edit::SetLives { count: 255 }),
        ),
        (
            r#"{"op": "set_relic", "level": "toadvillage", "relic": null}"#,
            Ok(Edit::SetRelic {
                level: id("toadvillage"),
                relic: None,
            }),
        ),
        (
            r#"{"op": "set_relic", "level": "toadvillage"}"#,
            Err("invalid_value"),
        ),
        (
            r#"{"op": "set_times", "level": "toadvillage", "times_cs": [-1]}"#,
            Err("invalid_value"),
        ),
        (
            r#"{"op": "set_times", "level": "toadvillage", "times_cs": [1.5]}"#,
            Err("invalid_value"),
        ),
        (
            r#"{"op": "unlock_warp_room", "room": 300, "with_abilities": true, "skip_cutscenes": true}"#,
            Err("invalid_value"),
        ),
        (
            r#"{"op": "reset_level", "level": "toadvillage"}"#,
            Err("invalid_value"),
        ),
        (
            r#"{"op": "mark_entered", "level": "tinytiger"}"#,
            Ok(Edit::MarkEntered {
                level: id("tinytiger"),
            }),
        ),
        (
            r#"{"op": "set_relic", "level": "tinytiger", "relic": "gold"}"#,
            Err("unknown_level"),
        ),
        (
            r#"{"op": "set_ability", "ability": "Double Jump", "on": true}"#,
            Err("unknown_ability"),
        ),
    ];
    for (json, expected) in cases {
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        let actual = Edit::from_json(&value).map_err(|e| e.code());
        assert_eq!(actual, expected, "{json}");
    }
    // The derived reader also requires every field.
    let missing: serde_json::Value =
        serde_json::from_str(r#"{"op": "set_relic", "level": "toadvillage"}"#).unwrap();
    assert!(serde_json::from_value::<Edit>(missing).is_err());
}

#[test]
fn edit_errors_have_the_contract_codes() {
    let codes: Vec<_> = [
        EditError::InvalidValue(String::new()),
        EditError::UnknownLevel(String::new()),
        EditError::UnknownBoss(String::new()),
        EditError::UnknownAbility(String::new()),
        EditError::UnknownCutscene(String::new()),
        EditError::NotApplicable(String::new()),
        EditError::DoesNotFit {
            needed: 1,
            capacity: 0,
        },
    ]
    .iter()
    .map(EditError::code)
    .collect();
    assert_eq!(
        codes,
        [
            "invalid_value",
            "unknown_level",
            "unknown_boss",
            "unknown_ability",
            "unknown_cutscene",
            "not_applicable",
            "does_not_fit"
        ]
    );
}

#[test]
fn edits_describe_themselves_for_the_history() {
    let save = load("saves/0019.sav");
    let cases = [
        (
            Edit::SetCrystal {
                level: id("deeptrouble"),
                on: true,
            },
            "Deep Trouble: crystal on",
        ),
        (
            Edit::SetColoredGem {
                level: id("deeptrouble"),
                on: false,
            },
            "Deep Trouble: red gem off",
        ),
        (
            Edit::SetRelic {
                level: id("toadvillage"),
                relic: Some(Relic::Gold),
            },
            "Toad Village: gold relic",
        ),
        (
            Edit::SetTimes {
                level: id("toadvillage"),
                times_cs: vec![6023, 5544],
            },
            "Toad Village: times 0:55.44, 1:00.23",
        ),
        (
            Edit::UnlockWarpRoom {
                room: 4,
                with_abilities: true,
                skip_cutscenes: true,
            },
            "Unlock warp room 4 (+abilities, skip scenes)",
        ),
        (
            Edit::SetBossDefeated {
                boss: id("ngin"),
                on: true,
                skip_cutscenes: true,
            },
            "N. Gin: defeated (skip scenes)",
        ),
        (
            Edit::SetAbility {
                ability: Ability::DoubleJump,
                on: true,
            },
            "Double Jump: on",
        ),
        (Edit::SetLives { count: 42 }, "Lives: 42"),
    ];
    for (edit, label) in cases {
        assert_eq!(edit.describe(&save), label);
    }
}
