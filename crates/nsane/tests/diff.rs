//! The semantic diff, against what the game wrote between two saves of the
//! playthrough (`docs/FORMAT.md` §11).

use nsane::crash3::tables::{cutscene_key, field, relic_counted_key};
use nsane::crash3::{Ability, Crash3Save, Edit, LevelId, Relic};
use nsane::diff::{Change, ChangeGroup, FieldText, Subject, diff, summarize};
use nsane::igb::{FieldValue, SummaryValue};

#[path = "support/corpus.rs"]
mod corpus;

fn load(rel: &str) -> Crash3Save {
    Crash3Save::parse(&corpus::read(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn id(key: &str) -> LevelId {
    LevelId::from_key(key).unwrap()
}

fn summary(key: &str, before: SummaryValue, after: SummaryValue) -> Change {
    Change::Summary {
        key: key.to_owned(),
        before: Some(before),
        after: Some(after),
    }
}

fn zone(level: &str, field: usize, before: FieldText, after: FieldText) -> Change {
    let level = id(level);
    Change::Zone {
        zone: level.level().zone.to_owned(),
        level: Some(level),
        field,
        before,
        after,
    }
}

fn text(s: &str) -> FieldText {
    FieldText::Text(Some(s.to_owned()))
}

#[test]
fn a_gold_relic_run_is_eight_changes() {
    // 0011: Toad Village's time trial beaten in 0:55.44, sapphire → gold.
    use FieldText::Int;
    use SummaryValue::Int as SInt;
    let (a, b) = (
        load("tests/saves/game/0010.sav"),
        load("tests/saves/game/0011.sav"),
    );
    let changes = diff(&a, &b);
    assert_eq!(
        changes,
        [
            summary("sapphire_count", SInt(1), SInt(0)),
            summary("gold_count", SInt(0), SInt(1)),
            zone("toadvillage", 3, Int(4), Int(5)),
            zone("toadvillage", 5, Int(6023), Int(5544)),
            zone("toadvillage", 6, Int(0), Int(6023)),
            zone("toadvillage", 9, text("---"), text("")),
            zone("toadvillage", 11, Int(1), Int(2)),
            zone("toadvillage", 13, Int(0x2202), Int(0x4202)),
        ]
    );

    let rows = summarize(&changes, &b);
    let rows: Vec<_> = rows
        .iter()
        .map(|r| (r.group, r.subject.clone(), r.to_string()))
        .collect();
    assert_eq!(
        rows,
        [
            (
                ChangeGroup::Levels,
                Subject::Level(id("toadvillage")),
                "Toad Village: relic sapphire → gold; times 1:00.23 → 0:55.44, 1:00.23; \
                 completions 4 → 5"
                    .to_owned()
            ),
            (
                ChangeGroup::Summary,
                Subject::Summary("sapphire_count".to_owned()),
                "Sapphire relics: 1 → 0".to_owned()
            ),
            (
                ChangeGroup::Summary,
                Subject::Summary("gold_count".to_owned()),
                "Gold relics: 0 → 1".to_owned()
            ),
        ]
    );
}

#[test]
fn beating_tiny_tiger() {
    // 0009: Tiny Tiger beaten for the first time, from the hub after Makin' Waves.
    use FieldText::Int;
    use SummaryValue::Int as SInt;
    let (a, b) = (
        load("tests/saves/game/0008.sav"),
        load("tests/saves/game/0009.sav"),
    );
    let changes = diff(&a, &b);
    let scene = |name: &str| Change::BoolFlag {
        key: cutscene_key(name),
        before: None,
        after: Some(1),
    };
    let variable = |name: &str| Change::Variable {
        name: name.to_owned(),
        before: Some("0".to_owned()),
        after: Some("1".to_owned()),
    };
    assert_eq!(
        changes,
        [
            summary("farthest_progression_index", SInt(6), SInt(7)),
            summary("previous_level_progression_index", SInt(5), SInt(6)),
            summary(
                "previous_level_name",
                SummaryValue::Str(b"crash3/l305_makinwaves/l305_makinwaves".to_vec()),
                SummaryValue::Str(b"crash3/bosses/b301_tinytiger/b301_tinytiger".to_vec()),
            ),
            summary(
                "percent_complete",
                SummaryValue::Float(9.0),
                SummaryValue::Float(10.0)
            ),
            summary("life_count", SInt(18), SInt(11)),
            zone("tinytiger", field::STATE, Int(-1), Int(1)),
            zone("tinytiger", field::COMPLETIONS, Int(0), Int(1)),
            zone("tinytiger", field::COLLECTIBLES, Int(0), Int(0x0800)),
            variable("C2_CrystalsFailCount"),
            variable("Upgrade_SuperChargedBodySlam"),
            scene("C3_Tiny_PostBossFight"),
            scene("C3_Tiny_PreBossFight"),
            Change::IntFlag {
                key: "the world entity (archetype b301_tinytiger.world_entity_entityData)"
                    .to_owned(),
                before: None,
                after: Some(7),
            },
        ]
    );

    let rows: Vec<_> = summarize(&changes, &b)
        .iter()
        .map(|r| (r.group, r.to_string()))
        .collect();
    assert_eq!(
        rows,
        [
            (
                ChangeGroup::Bosses,
                "Tiny Tiger: defeated; entered, completions 0 → 1".to_owned()
            ),
            (
                ChangeGroup::Abilities,
                "Super Charged Body Slam: off → on".to_owned()
            ),
            (
                ChangeGroup::Cutscenes,
                "Scene C3_Tiny_PostBossFight: seen".to_owned()
            ),
            (
                ChangeGroup::Cutscenes,
                "Scene C3_Tiny_PreBossFight: seen".to_owned()
            ),
            (
                ChangeGroup::Summary,
                "Furthest progression: 6 → 7".to_owned()
            ),
            (
                ChangeGroup::Summary,
                "Last level entered (progression index): 5 → 6".to_owned()
            ),
            (
                ChangeGroup::Summary,
                "Last level entered: Makin' Waves → Tiny Tiger".to_owned()
            ),
            (ChangeGroup::Summary, "Percentage: 9% → 10%".to_owned()),
            (ChangeGroup::Summary, "Lives: 18 → 11".to_owned()),
            (
                ChangeGroup::Other,
                "Variable C2_CrystalsFailCount: 0 → 1".to_owned()
            ),
            (
                ChangeGroup::Other,
                "In-level state the world entity (archetype \
                 b301_tinytiger.world_entity_entityData): none → 7"
                    .to_owned()
            ),
        ]
    );
}

#[test]
fn a_save_has_no_changes_against_itself() {
    let saves = corpus::corpus();
    // 51 files under tests/saves/ and 24 saves rebuilt from their recipe
    assert_eq!(saves.len(), 75);
    for save in saves {
        let (a, b) = (
            Crash3Save::parse(&save.bytes).unwrap(),
            Crash3Save::parse(&save.bytes).unwrap(),
        );
        assert_eq!(diff(&a, &b), [], "{}", save.source);
        assert_eq!(a.zones().count(), 116, "{}", save.source);
    }
}

#[test]
fn bool_padding_and_hash_table_slots_are_not_compared() {
    let save = load("tests/saves/game/0019.sav");
    let session = save.session(id("toadvillage")).unwrap().id;
    let mut igb = save.clone().into_igb();

    // uninitialised bytes of the last bool field
    let f = igb.object_mut(session).unwrap().field_mut(14).unwrap();
    assert!(matches!(f, FieldValue::Bool { .. }));
    *f = FieldValue::Bool {
        value: 0,
        padding: 0xffff_ff00,
    };

    // the same flags at twice the capacity: other slots, same keys
    let table = igb.objects_of("igStringBoolHashTable").next().unwrap().id;
    let items = igb.hash_table(table).unwrap().items().unwrap();
    let capacity = igb.hash_table(table).unwrap().capacity();
    igb.hash_table_mut(table)
        .unwrap()
        .rebuild(capacity * 2, &items)
        .unwrap();

    let edited = Crash3Save::from_igb(igb).unwrap();
    assert_ne!(
        edited.igb().build().unwrap(),
        save.igb().build().unwrap(),
        "the bytes differ"
    );
    assert_eq!(diff(&save, &edited), []);
}

#[test]
fn zones_outside_crash_3_are_other_changes() {
    let save = load("tests/saves/game/0019.sav");
    let (path, session) = save
        .zones()
        .find(|(p, _)| p.starts_with("crash1/"))
        .map(|(p, s)| (p.to_owned(), s.id))
        .unwrap();
    let mut igb = save.clone().into_igb();
    *igb.object_mut(session).unwrap().field_mut(3).unwrap() = FieldValue::Int(5);
    let edited = Crash3Save::from_igb(igb).unwrap();

    let changes = diff(&save, &edited);
    assert_eq!(
        changes,
        [Change::Zone {
            zone: path.clone(),
            level: None,
            field: 3,
            before: FieldText::Int(0),
            after: FieldText::Int(5),
        }]
    );
    assert_eq!(changes[0].group(), ChangeGroup::Other);
    assert_eq!(changes[0].describe(), format!("Zone {path}: field 3 0 → 5"));
    let rows = summarize(&changes, &edited);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].subject, Subject::Zone(path));
}

/// The changes of `edits` on `tests/saves/game/0019.sav`, with the edited save.
fn changes_of(edits: &[Edit]) -> (Vec<Change>, Crash3Save) {
    let save = load("tests/saves/game/0019.sav");
    let mut edited = save.clone();
    edited.apply_all(edits).unwrap();
    (diff(&save, &edited), edited)
}

#[test]
fn descriptions_read_as_game_terms() {
    let deep = id("deeptrouble");
    let (changes, edited) = changes_of(&[
        Edit::SetCrystal {
            level: deep,
            on: true,
        },
        Edit::SetColoredGem {
            level: deep,
            on: true,
        },
        Edit::SetRelic {
            level: deep,
            relic: Some(Relic::Gold),
        },
        Edit::SetAbility {
            ability: Ability::SpeedShoes,
            on: true,
        },
        Edit::SetCutscene {
            cutscene: "C3_NGin_Intro".to_owned(),
            seen: true,
        },
        Edit::SetBonusGem { on: true },
    ]);
    let described: Vec<_> = changes.iter().map(|c| (c.group(), c.describe())).collect();
    let has = |group, text: &str| {
        assert!(
            described.contains(&(group, text.to_owned())),
            "{text} in {described:#?}"
        )
    };
    has(
        ChangeGroup::Levels,
        "Deep Trouble: + crystal, + red gem, + gold relic",
    );
    has(ChangeGroup::Levels, "Deep Trouble: relic none → gold");
    has(ChangeGroup::Levels, "Deep Trouble: crystal field 0 → 1");
    has(ChangeGroup::Levels, "Deep Trouble: relic counted flag set");
    has(ChangeGroup::ColoredGems, "Red gem: variable off → on");
    has(ChangeGroup::Abilities, "Speed Shoes: off → on");
    has(ChangeGroup::Cutscenes, "Scene C3_NGin_Intro: seen");
    has(ChangeGroup::Other, "Bonus gem: taken");
    assert!(changes.iter().any(|c| matches!(
        c,
        Change::BoolFlag { key, .. } if *key == relic_counted_key(deep.level())
    )));

    // In the review, the crystal field and the relic counted flag are folded
    // into the crystal and the relic.
    let rows: Vec<_> = summarize(&changes, &edited)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        rows,
        [
            "Deep Trouble: + crystal, + red gem; relic none → gold; entered, completions 0 → 1",
            "Speed Shoes: off → on",
            "Red gem: variable off → on",
            "Scene C3_NGin_Intro: seen",
            "Bonus gem: taken",
        ]
    );
}

#[test]
fn removals_and_inconsistent_fields_are_spelled_out() {
    let toad = id("toadvillage");
    let save = load("tests/saves/game/0019.sav");
    assert!(save.crystal(toad) && save.relic(toad).is_some());

    let (changes, edited) = changes_of(&[
        Edit::SetCrystal {
            level: toad,
            on: false,
        },
        Edit::SetRelic {
            level: toad,
            relic: None,
        },
        Edit::SetTimes {
            level: toad,
            times_cs: vec![],
        },
        Edit::SetBonusGem { on: false },
    ]);
    let rows: Vec<_> = summarize(&changes, &edited)
        .iter()
        .map(ToString::to_string)
        .collect();
    let times: Vec<_> = save
        .times(toad)
        .iter()
        .map(|&t| nsane::crash3::format_time(t))
        .collect();
    assert_eq!(
        rows[0],
        format!(
            "Toad Village: - crystal; relic {} → none; times {} → none",
            save.relic(toad).unwrap().key(),
            times.join(", ")
        )
    );

    // A relic counted flag without its relic, and the crystal field alone.
    let (changes, edited) = changes_of(&[Edit::RawFlagSet {
        key: relic_counted_key(id("deeptrouble").level()),
    }]);
    let rows: Vec<_> = summarize(&changes, &edited)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(rows, ["Deep Trouble: relic counted flag set"]);

    let mut igb = save.clone().into_igb();
    let session = save.session(toad).unwrap().id;
    *igb.object_mut(session)
        .unwrap()
        .field_mut(field::CRYSTAL)
        .unwrap() = FieldValue::Bool {
        value: 0,
        padding: 0,
    };
    let edited = Crash3Save::from_igb(igb).unwrap();
    let rows: Vec<_> = summarize(&diff(&save, &edited), &edited)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(rows, ["Toad Village: crystal field 1 → 0"]);

    // Flags keyed by text the editor doesn't know.
    let (changes, _) = changes_of(&[Edit::RawFlagSet {
        key: "HintDisplay_99".to_owned(),
    }]);
    let described: Vec<_> = changes.iter().map(Change::describe).collect();
    assert_eq!(described, ["Flag HintDisplay_99: set"]);
    assert_eq!(changes[0].group(), ChangeGroup::Other);
}

#[test]
fn leftover_bytes_as_the_last_level_read_as_unknown() {
    use nsane::crash3::is_zone_path;
    use nsane::diff::last_level_text;
    // The game wrote these bytes when it saved a file that had no last level
    // before any level was entered (docs/FORMAT.md §6).
    let after = load("tests/saves/after_edit/l2_after.sav");
    let leftover = after.previous_level_name().unwrap();
    assert_eq!(leftover, "`\u{1d}\u{c0}\u{f4}\u{10}");
    assert!(!is_zone_path(&leftover));
    assert_eq!(last_level_text(&leftover), "unknown");
    assert_eq!(last_level_text(""), "none");
    assert_eq!(
        last_level_text("crash3/l301_toadvillage/l301_toadvillage"),
        "Toad Village"
    );
    assert_eq!(
        last_level_text("crash1/l101_nsanitybeach/l101_nsanitybeach"),
        "crash1/l101_nsanitybeach/l101_nsanitybeach"
    );
    for bad in [
        "toadvillage",
        "crash3//x",
        "/crash3",
        "Crash3/x",
        "crash3/x y",
    ] {
        assert!(!is_zone_path(bad), "{bad}");
    }
    let changes = diff(&load("tests/saves/edited/l2_pc_to_switch.sav"), &after);
    let texts: Vec<String> = changes.iter().map(Change::describe).collect();
    assert_eq!(texts, ["Last level entered: none → unknown"]);
}
