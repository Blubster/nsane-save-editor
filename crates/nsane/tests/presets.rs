//! Presets and the reset level edit. The 108% preset is checked against the
//! file that was tested in game on PC; the others by their totals, their
//! warnings and the order of their edits.

use nsane::Document;
use nsane::crash3::presets::{
    AllRelics, CompleteAll, DEFAULT_ABILITIES_ON, DEFAULT_ALL_RELICS_TIER, DEFAULT_CUTSCENES_SEEN,
    ReachRelicCount, StoryUpTo, TimePolicy, all_abilities, all_cutscenes, all_relics, complete_all,
    default_relic_count_target, default_story_room, matching_time, reach_relic_count, story_up_to,
};
use nsane::crash3::tables::{field, relic_counted_key};
use nsane::crash3::{
    ABILITIES, BOSSES, CUTSCENES, Crash3Save, Edit, GemColor, Kind, LevelId, Relic, Totals,
    relic_for_time,
};
use nsane::igb::FieldValue;

#[path = "support/corpus.rs"]
mod corpus;

/// A save under `tests/saves/`, or `live/<name>`, a save rebuilt from its
/// recipe.
fn read(rel: &str) -> Vec<u8> {
    corpus::input(rel).unwrap_or_else(|e| panic!("{e}"))
}

fn load(rel: &str) -> Crash3Save {
    Crash3Save::parse(&read(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn id(key: &str) -> LevelId {
    LevelId::from_key(key).unwrap()
}

fn levels() -> impl Iterator<Item = LevelId> {
    LevelId::all().filter(|l| l.level().kind != Kind::Boss)
}

/// Applies the edits, writes the file and opens it again.
fn applied(save: &Crash3Save, edits: &[Edit]) -> Crash3Save {
    let mut save = save.clone();
    save.apply_all(edits).unwrap();
    Crash3Save::parse(&save.to_bytes().unwrap()).unwrap()
}

fn relic(level: LevelId, relic: Relic) -> Edit {
    Edit::SetRelic {
        level,
        relic: Some(relic),
    }
}

fn times(level: LevelId, times_cs: &[u32]) -> Edit {
    Edit::SetTimes {
        level,
        times_cs: times_cs.to_vec(),
    }
}

// ------------------------------------------------------------ complete all

#[test]
fn complete_all_reproduces_the_108_percent_pc_save_tested_in_game() {
    let mut save = load("tests/saves/game/ALCHEMYXSAVE_8.sav");
    let options = CompleteAll {
        times: TimePolicy::Fixed(6000),
        skip_cutscenes: true,
    };
    let mut edits = complete_all(&save, &options);
    edits.extend(all_cutscenes(true));
    edits.push(Edit::SetLives { count: 99 });
    save.apply_all(&edits).unwrap();
    assert!(save.to_bytes().unwrap() == read("live/pc2_full108"));
}

#[test]
fn complete_all_with_defaults_gives_108_percent() {
    for rel in [
        "tests/saves/game/initial_game.sav",
        "tests/saves/game/0019.sav",
        "tests/saves/game/ALCHEMYXSAVE_8.sav",
    ] {
        let save = load(rel);
        let done = applied(&save, &complete_all(&save, &CompleteAll::default()));
        let t = done.totals();
        assert_eq!(
            (t.crystals, t.gems, t.relics(), t.bosses),
            (25, 47, 31, 5),
            "{rel}"
        );
        assert_eq!(t.platinum, 31, "{rel}");
        assert_eq!(t.percent(), 108, "{rel}");
        assert_eq!(done.percent(), 108, "{rel}");
        assert!(done.warnings().is_empty(), "{rel}");
        assert!(done.story_completed(), "{rel}");
        for level in levels() {
            let best = done.times(level)[0];
            assert_eq!(
                relic_for_time(level, best),
                Some(Relic::Platinum),
                "{rel} {level:?}"
            );
        }
        assert!(ABILITIES.iter().all(|&a| done.ability(a)), "{rel}");
        assert!(done.bonus_gem(), "{rel}");
    }
}

#[test]
fn complete_all_order() {
    let save = load("tests/saves/game/0019.sav");
    let edits = complete_all(&save, &CompleteAll::default());
    let (toad, boneyard, hangem) = (id("toadvillage"), id("boneyard"), id("hangemhigh"));
    let on = true;
    // Toad Village's 0:55.44 doesn't earn platinum, so it gets 0:44.02.
    assert_eq!(
        edits[..4],
        [
            Edit::SetCrystal { level: toad, on },
            Edit::SetGem { level: toad, on },
            relic(toad, Relic::Platinum),
            times(toad, &[4402]),
        ]
    );
    let at = |level| {
        edits
            .iter()
            .position(|e| *e == Edit::SetCrystal { level, on })
            .unwrap()
    };
    let p = matching_time(boneyard, Relic::Platinum).unwrap();
    assert_eq!(
        edits[at(boneyard)..at(boneyard) + 5],
        [
            Edit::SetCrystal {
                level: boneyard,
                on
            },
            Edit::SetGem {
                level: boneyard,
                on
            },
            Edit::SetSecondGem {
                level: boneyard,
                on
            },
            relic(boneyard, Relic::Platinum),
            times(boneyard, &[p]),
        ]
    );
    let p = matching_time(hangem, Relic::Platinum).unwrap();
    assert_eq!(
        edits[at(hangem)..at(hangem) + 5],
        [
            Edit::SetCrystal { level: hangem, on },
            Edit::SetGem { level: hangem, on },
            Edit::SetColoredGem { level: hangem, on },
            relic(hangem, Relic::Platinum),
            times(hangem, &[p]),
        ]
    );
    // Secret levels have no crystal.
    let coco = id("hotcoco");
    let p = matching_time(coco, Relic::Platinum).unwrap();
    let i = edits
        .iter()
        .position(|e| *e == Edit::SetGem { level: coco, on })
        .unwrap();
    assert_eq!(
        edits[i - 1..i + 3],
        [
            times(
                id("ringsofpower"),
                &[matching_time(id("ringsofpower"), Relic::Platinum).unwrap()]
            ),
            Edit::SetGem { level: coco, on },
            relic(coco, Relic::Platinum),
            times(coco, &[p]),
        ]
    );
    // 25 crystals + 31 clear + 10 second + 5 colored gems, 31 relics and
    // 31 times, then the bosses, the abilities and the bonus gem.
    assert_eq!(edits.len(), 25 + 31 + 10 + 5 + 31 + 31 + 5 + 5 + 1);
    let tail: Vec<Edit> = BOSSES
        .iter()
        .map(|&boss| Edit::SetBossDefeated {
            boss,
            on,
            skip_cutscenes: true,
        })
        .chain(
            ABILITIES
                .iter()
                .map(|&ability| Edit::SetAbility { ability, on }),
        )
        .chain([Edit::SetBonusGem { on }])
        .collect();
    assert_eq!(edits[edits.len() - 11..], tail[..]);
    // The level edits follow the level table.
    let order: Vec<LevelId> = edits
        .iter()
        .filter_map(|e| match e {
            Edit::SetGem { level, .. } => Some(*level),
            _ => None,
        })
        .collect();
    assert_eq!(order, levels().collect::<Vec<_>>());
}

#[test]
fn complete_all_is_one_undo_step() {
    let mut doc = Document::open(&read("tests/saves/game/0019.sav"), None).unwrap();
    let edits = complete_all(doc.current(), &CompleteAll::default());
    let preview = doc.preview(&edits).unwrap();
    assert_eq!(preview.totals.percent(), 108);
    assert!(preview.warnings.is_empty());
    doc.apply("Complete everything (108%)", edits).unwrap();
    assert_eq!(doc.current().percent(), 108);
    assert_eq!(
        doc.history().collect::<Vec<_>>(),
        ["Complete everything (108%)"]
    );
    assert!(doc.undo());
    assert_eq!(doc.current(), doc.original());
}

// -------------------------------------------------------------- story up to

#[test]
fn story_up_to_each_room_on_a_new_game() {
    let save = load("tests/saves/game/initial_game.sav");
    for room in 1..=5u8 {
        let done = applied(&save, &story_up_to(room, &StoryUpTo::default()).unwrap());
        let n = usize::from(room);
        let t = done.totals();
        assert_eq!(t.crystals, 5 * u32::from(room), "room {room}");
        assert_eq!(t.gems, 0, "room {room}");
        assert_eq!(t.relics(), 0, "room {room}");
        for (i, &boss) in BOSSES.iter().enumerate() {
            assert_eq!(done.boss_defeated(boss), i < n, "room {room} {boss:?}");
        }
        for (i, &ability) in ABILITIES.iter().enumerate() {
            assert_eq!(done.ability(ability), i < n, "room {room} {ability:?}");
        }
        for step in &CUTSCENES {
            let skipped = step
                .boss()
                .is_none_or(|b| b.index() <= BOSSES[n - 1].index());
            for &scene in step.scenes {
                assert_eq!(done.cutscene_seen(scene), skipped, "room {room} {scene}");
            }
        }
        assert!(done.warnings().is_empty(), "room {room}");
        assert_eq!(done.story_completed(), room == 5);
    }
}

#[test]
fn story_up_to_order() {
    let on = true;
    let options = StoryUpTo {
        with_gems: true,
        with_abilities: true,
        skip_cutscenes: false,
    };
    let crystal = |key| Edit::SetCrystal { level: id(key), on };
    let gem = |key| Edit::SetGem { level: id(key), on };
    let mut expected = Vec::new();
    for key in ["toadvillage", "underpressure", "orientexpress"] {
        expected.extend([crystal(key), gem(key)]);
    }
    expected.extend([
        crystal("boneyard"),
        gem("boneyard"),
        Edit::SetSecondGem {
            level: id("boneyard"),
            on,
        },
        crystal("makinwaves"),
        gem("makinwaves"),
        crystal("geewiz"),
        gem("geewiz"),
        crystal("hangemhigh"),
        gem("hangemhigh"),
        Edit::SetColoredGem {
            level: id("hangemhigh"),
            on,
        },
        crystal("hogride"),
        gem("hogride"),
        crystal("tombtime"),
        gem("tombtime"),
        Edit::SetSecondGem {
            level: id("tombtime"),
            on,
        },
        crystal("midnightrun"),
        gem("midnightrun"),
        Edit::SetBossDefeated {
            boss: id("tinytiger"),
            on,
            skip_cutscenes: false,
        },
        Edit::SetBossDefeated {
            boss: id("dingodile"),
            on,
            skip_cutscenes: false,
        },
        Edit::SetAbility {
            ability: ABILITIES[0],
            on,
        },
        Edit::SetAbility {
            ability: ABILITIES[1],
            on,
        },
    ]);
    assert_eq!(story_up_to(2, &options).unwrap(), expected);

    let defaults = story_up_to(1, &StoryUpTo::default()).unwrap();
    assert_eq!(
        defaults,
        [
            crystal("toadvillage"),
            crystal("underpressure"),
            crystal("orientexpress"),
            crystal("boneyard"),
            crystal("makinwaves"),
            Edit::SetBossDefeated {
                boss: id("tinytiger"),
                on,
                skip_cutscenes: true,
            },
            Edit::SetAbility {
                ability: ABILITIES[0],
                on,
            },
        ]
    );

    for room in [0, 6] {
        let err = story_up_to(room, &StoryUpTo::default()).unwrap_err();
        assert_eq!(err.code(), "invalid_value");
    }
}

// --------------------------------------------------------------- all relics

#[test]
fn all_relics_gold_leaves_a_platinum_save_alone() {
    let save = load("tests/saves/after_edit/t6_after.sav");
    let edits = all_relics(&save, DEFAULT_ALL_RELICS_TIER, &AllRelics::default());
    assert!(edits.is_empty(), "{edits:?}");
}

#[test]
fn all_relics_gold_on_a_mid_game_save() {
    let save = load("tests/saves/game/0019.sav");
    let edits = all_relics(&save, Relic::Gold, &AllRelics::default());
    let done = applied(&save, &edits);
    let t = done.totals();
    assert_eq!((t.relics(), t.gold), (31, 31));
    for level in levels() {
        assert!(done.relic(level) >= save.relic(level), "{level:?}");
        let best = done.times(level)[0];
        assert_eq!(relic_for_time(level, best), Some(Relic::Gold), "{level:?}");
    }
    // Toad Village already has gold and a gold time; the 30 others get a
    // relic and a time just under the gold threshold, in table order.
    let mut expected = Vec::new();
    for level in levels().skip(1) {
        expected.push(relic(level, Relic::Gold));
        expected.push(times(level, &[matching_time(level, Relic::Gold).unwrap()]));
    }
    assert_eq!(edits, expected);
    assert!(done.warnings().is_empty());
}

#[test]
fn all_relics_upgrades_or_replaces() {
    let mut save = load("tests/saves/game/initial_game.sav");
    let (toad, under, orient) = (id("toadvillage"), id("underpressure"), id("orientexpress"));
    save.apply_all(&[
        relic(toad, Relic::Platinum),
        relic(under, Relic::Sapphire),
        relic(orient, Relic::Gold),
    ])
    .unwrap();
    let keep = AllRelics {
        only_upgrade: true,
        times: TimePolicy::Keep,
    };
    let edits = all_relics(&save, Relic::Gold, &keep);
    assert_eq!(edits[0], relic(under, Relic::Gold));
    assert!(
        !edits
            .iter()
            .any(|e| matches!(e, Edit::SetRelic { level, .. } if *level == toad))
    );
    assert!(!edits.iter().any(|e| matches!(e, Edit::SetTimes { .. })));
    assert_eq!(edits.len(), 29);

    let replace = AllRelics {
        only_upgrade: false,
        times: TimePolicy::Clear,
    };
    let edits = all_relics(&save, Relic::Gold, &replace);
    assert_eq!(
        edits[..4],
        [
            relic(toad, Relic::Gold),
            times(toad, &[]),
            relic(under, Relic::Gold),
            times(under, &[]),
        ]
    );
    // Orient Express has gold already.
    assert_eq!(edits[4], relic(id("boneyard"), Relic::Gold));
    assert_eq!(edits.len(), 2 * 30);
}

#[test]
fn matching_times_keep_a_time_that_earns_the_relic() {
    let mut save = load("tests/saves/game/initial_game.sav");
    let (toad, under, orient) = (id("toadvillage"), id("underpressure"), id("orientexpress"));
    let platinum = matching_time(toad, Relic::Platinum).unwrap();
    let gold = matching_time(under, Relic::Gold).unwrap();
    let slow = matching_time(orient, Relic::Sapphire).unwrap();
    save.apply_all(&[
        times(toad, &[platinum]),
        times(under, &[gold, gold + 10]),
        times(orient, &[slow]),
    ])
    .unwrap();
    let edits = all_relics(&save, Relic::Gold, &AllRelics::default());
    // A better time and an equal one are kept; a slower one is replaced.
    assert_eq!(
        edits[..4],
        [
            relic(toad, Relic::Gold),
            relic(under, Relic::Gold),
            relic(orient, Relic::Gold),
            times(orient, &[matching_time(orient, Relic::Gold).unwrap()]),
        ]
    );
    let fixed = AllRelics {
        only_upgrade: true,
        times: TimePolicy::Fixed(6000),
    };
    assert_eq!(
        all_relics(&save, Relic::Gold, &fixed)[..2],
        [relic(toad, Relic::Gold), times(toad, &[6000])]
    );
}

// -------------------------------------------------------- reach relic count

#[test]
fn reach_relic_count_on_a_mid_game_save() {
    let save = load("tests/saves/game/0019.sav");
    let edits = reach_relic_count(&save, 20, &ReachRelicCount::default());
    let done = applied(&save, &edits);
    assert_eq!(done.totals().relics(), 20);
    assert_eq!(done.totals().sapphire, 19);
    assert_eq!(done.relic(id("toadvillage")), Some(Relic::Gold));
    // The 19 levels after Toad Village, in table order, with sapphire times.
    let mut expected = Vec::new();
    for level in levels().skip(1).take(19) {
        expected.push(relic(level, Relic::Sapphire));
        expected.push(times(
            level,
            &[matching_time(level, Relic::Sapphire).unwrap()],
        ));
    }
    assert_eq!(edits, expected);
    assert!(done.warnings().is_empty());

    assert!(reach_relic_count(&save, 1, &ReachRelicCount::default()).is_empty());
    assert!(reach_relic_count(&save, 0, &ReachRelicCount::default()).is_empty());
    // More than the game has gives every level a relic.
    let all = applied(
        &save,
        &reach_relic_count(&save, 40, &ReachRelicCount::default()),
    );
    assert_eq!(all.totals().relics(), 31);
}

// ---------------------------------------------------- abilities and scenes

#[test]
fn all_abilities_and_all_cutscenes_order() {
    for on in [true, false] {
        let expected: Vec<Edit> = ABILITIES
            .iter()
            .map(|&ability| Edit::SetAbility { ability, on })
            .collect();
        assert_eq!(all_abilities(on), expected);
    }
    let scenes = all_cutscenes(true);
    assert_eq!(scenes.len(), 18);
    let names: Vec<&str> = scenes
        .iter()
        .map(|e| match e {
            Edit::SetCutscene { cutscene, seen } => {
                assert!(*seen);
                cutscene.as_str()
            }
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        names[..4],
        [
            "C3_Cortex_Taunt_1",
            "C3_AkuAku_TimeTrialInfo",
            "C3_AkuAku_Warning",
            "C3_Tiny_Intro"
        ]
    );
    assert_eq!(names[17], "C3_PreFinalBattle");
    let flat: Vec<&str> = CUTSCENES
        .iter()
        .flat_map(|s| s.scenes.iter().copied())
        .collect();
    assert_eq!(names, flat);

    let save = load("tests/saves/after_edit/t6_after.sav");
    let none = applied(&save, &all_cutscenes(false));
    assert!(flat.iter().all(|s| !none.cutscene_seen(s)));
    let all = applied(&none, &all_cutscenes(true));
    assert!(flat.iter().all(|s| all.cutscene_seen(s)));
}

// ----------------------------------------------------------------- defaults

#[test]
fn defaults() {
    assert_eq!(
        CompleteAll::default(),
        CompleteAll {
            times: TimePolicy::Matching,
            skip_cutscenes: true
        }
    );
    assert_eq!(
        StoryUpTo::default(),
        StoryUpTo {
            with_gems: false,
            with_abilities: true,
            skip_cutscenes: true
        }
    );
    assert_eq!(
        AllRelics::default(),
        AllRelics {
            only_upgrade: true,
            times: TimePolicy::Matching
        }
    );
    assert_eq!(
        ReachRelicCount::default(),
        ReachRelicCount {
            tier: Relic::Sapphire,
            times: TimePolicy::Matching
        }
    );
    assert_eq!(TimePolicy::default(), TimePolicy::Matching);
    assert_eq!(DEFAULT_ALL_RELICS_TIER, Relic::Gold);
    const { assert!(DEFAULT_ABILITIES_ON && DEFAULT_CUTSCENES_SEEN) };

    let new_game = load("tests/saves/game/initial_game.sav");
    let mid = load("tests/saves/game/0019.sav");
    let full = load("tests/saves/after_edit/t6_after.sav");
    assert_eq!(default_story_room(&new_game), 1);
    assert_eq!(default_story_room(&mid), 3);
    assert_eq!(default_story_room(&full), 5);
    assert_eq!(default_relic_count_target(&new_game), 5);
    assert_eq!(default_relic_count_target(&mid), 5);
    assert_eq!(default_relic_count_target(&full), 25);

    // The next threshold is strictly above the count.
    let five = applied(
        &new_game,
        &reach_relic_count(&new_game, 5, &ReachRelicCount::default()),
    );
    assert_eq!(default_relic_count_target(&five), 10);
    let twenty_four = applied(
        &new_game,
        &reach_relic_count(&new_game, 24, &ReachRelicCount::default()),
    );
    assert_eq!(default_relic_count_target(&twenty_four), 25);
}

// -------------------------------------------------------------- reset level

/// A session's fields with strings resolved, ignoring a bool's padding bytes
/// (uninitialised memory).
#[derive(Debug, PartialEq)]
enum Value {
    Str(Option<Vec<u8>>),
    Other(FieldValue),
}

fn session(save: &Crash3Save, level: LevelId) -> Vec<Value> {
    save.session(level)
        .unwrap()
        .fields
        .iter()
        .map(|&f| match f {
            FieldValue::Str(i) => Value::Str(save.igb().string(i).map(<[u8]>::to_vec)),
            FieldValue::Bool { value, .. } => Value::Other(FieldValue::Bool { value, padding: 0 }),
            other => Value::Other(other),
        })
        .collect()
}

fn reset(level: LevelId) -> Edit {
    Edit::ResetLevel { level }
}

/// Crystals, gems, relics and bosses, as `Totals` counts them.
fn counts(t: Totals) -> [u32; 4] {
    [t.crystals, t.gems, t.relics(), t.bosses]
}

/// What a level adds to [`counts`].
fn contribution(save: &Crash3Save, level: LevelId) -> [u32; 4] {
    [
        u32::from(save.crystal(level)),
        save.gem_count(level),
        u32::from(save.relic(level).is_some()),
        u32::from(save.boss_defeated(level)),
    ]
}

#[test]
fn reset_level_gives_back_the_new_game_session() {
    let fresh = load("tests/saves/game/initial_game.sav");
    let full = load("tests/saves/after_edit/t6_after.sav");
    let before = full.totals();
    assert_eq!(counts(before), [25, 47, 31, 5]);
    let flags = full.flag_entries();
    for level in LevelId::all() {
        let done = applied(&full, &[reset(level)]);
        assert_eq!(session(&done, level), session(&fresh, level), "{level:?}");
        assert!(!done.entered(level), "{level:?}");
        assert_eq!(done.completions(level), 0, "{level:?}");

        let gone = contribution(&full, level);
        let expected: Vec<u32> = counts(before)
            .iter()
            .zip(gone)
            .map(|(b, g)| b - g)
            .collect();
        assert_eq!(counts(done.totals())[..], expected[..], "{level:?}");

        // Only its relic flag and its colored gem variable change.
        let lv = level.level();
        let mut expected_flags = flags.clone();
        if lv.has_relic() {
            expected_flags.remove(&relic_counted_key(lv)).unwrap();
        }
        assert_eq!(done.flag_entries(), expected_flags, "{level:?}");
        let changed: Vec<&str> = full
            .variables()
            .zip(done.variables())
            .filter(|(a, b)| a != b)
            .map(|(a, _)| a.0)
            .collect();
        let expected: Vec<&str> = lv.colored_gem.map(GemColor::variable).into_iter().collect();
        assert_eq!(changed, expected, "{level:?}");
        for &ability in &ABILITIES {
            assert!(done.ability(ability), "{level:?}");
        }
    }
}

#[test]
fn reset_every_level_gives_back_a_new_game() {
    let fresh = load("tests/saves/game/initial_game.sav");
    let full = load("tests/saves/after_edit/t6_after.sav");
    let edits: Vec<Edit> = LevelId::all().map(reset).collect();
    let done = applied(&full, &edits);
    for level in LevelId::all() {
        assert_eq!(session(&done, level), session(&fresh, level), "{level:?}");
    }
    let t = done.totals();
    assert_eq!((t.crystals, t.relics(), t.bosses), (0, 0, 0));
    // The bonus gem is a flag of its own, kept like the abilities and scenes.
    assert_eq!(t.gems, 1);
    assert!(done.bonus_gem());
    assert!(
        !done
            .flag_entries()
            .keys()
            .any(|k| k.ends_with("_RelicCounted"))
    );
    for color in GemColor::ALL {
        let value = done.variable(color.variable());
        assert!(
            matches!(value, Some(FieldValue::Bool { value: 0, .. })),
            "{value:?}"
        );
    }
    for &scene in CUTSCENES.iter().flat_map(|s| s.scenes) {
        assert_eq!(
            done.cutscene_seen(scene),
            full.cutscene_seen(scene),
            "{scene}"
        );
    }
    assert!(ABILITIES.iter().all(|&a| done.ability(a)));
    assert!(done.warnings().is_empty());
}

#[test]
fn reset_level_on_a_mid_game_save() {
    let save = load("tests/saves/game/0019.sav");
    let under = id("underpressure");
    let done = applied(&save, &[reset(under)]);
    let t = done.totals();
    assert_eq!((t.crystals, t.gems), (9, 5));
    assert!(!done.entered(under));
    // A level never entered stays as it was, field for field.
    let road = id("roadcrash");
    let same = applied(&save, &[reset(road)]);
    assert_eq!(session(&same, road), session(&save, road));
    // The edit reads and writes as JSON, with a level or a boss key.
    for key in ["underpressure", "tinytiger"] {
        let json = serde_json::json!({"op": "reset_level", "level": key});
        let edit = Edit::from_json(&json).unwrap();
        assert_eq!(edit, reset(id(key)));
        assert_eq!(serde_json::to_value(&edit).unwrap(), json);
    }
    assert_eq!(
        reset(under).describe(&save),
        "Under Pressure: reset to never played"
    );
    let state = done.session(under).unwrap().field(field::STATE).copied();
    assert_eq!(state, Some(FieldValue::Enum(-1)));
}
