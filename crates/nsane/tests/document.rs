//! The document layer: undo and redo, dirty tracking, platform and size,
//! preview and review.

use nsane::crash3::{Ability, Edit, EditError, Kind, LevelId, Relic};
use nsane::diff::{Change, ChangeGroup, Subject};
use nsane::igb::SummaryValue;
use nsane::{Document, Platform, SaveError, UNDO_DEPTH};

#[path = "support/corpus.rs"]
mod corpus;

fn open(rel: &str) -> Document {
    Document::open(&corpus::read(rel), Some(rel.to_owned()))
        .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn id(key: &str) -> LevelId {
    LevelId::from_key(key).unwrap()
}

fn bytes(doc: &Document) -> Vec<u8> {
    doc.to_bytes().unwrap()
}

/// Five steps that touch sessions, variables, the flag table (with growth)
/// and the summary.
fn mixed_steps() -> Vec<(&'static str, Vec<Edit>)> {
    let deep = id("deeptrouble");
    let toad = id("toadvillage");
    vec![
        (
            "Deep Trouble crystal and red gem",
            vec![
                Edit::SetCrystal {
                    level: deep,
                    on: true,
                },
                Edit::SetColoredGem {
                    level: deep,
                    on: true,
                },
            ],
        ),
        (
            "Toad Village platinum",
            vec![
                Edit::SetRelic {
                    level: toad,
                    relic: Some(Relic::Platinum),
                },
                Edit::SetTimes {
                    level: toad,
                    times_cs: vec![4000, 7000],
                },
            ],
        ),
        (
            "Speed Shoes",
            vec![Edit::SetAbility {
                ability: Ability::SpeedShoes,
                on: true,
            }],
        ),
        (
            "Unlock room 5",
            vec![Edit::UnlockWarpRoom {
                room: 5,
                with_abilities: true,
                skip_cutscenes: true,
            }],
        ),
        (
            "Lives and a flag",
            vec![
                Edit::SetLives { count: 99 },
                Edit::RawFlagSet {
                    key: "HintDisplay_99".to_owned(),
                },
            ],
        ),
    ]
}

#[test]
fn undo_and_redo_give_back_the_same_bytes() {
    let mut doc = open("tests/saves/game/0019.sav");
    let mut states = vec![bytes(&doc)];
    for (label, edits) in mixed_steps() {
        let before = bytes(&doc);
        doc.apply(label, edits).unwrap();
        let after = bytes(&doc);
        assert_ne!(before, after, "{label}");

        // apply → undo → redo, one step at a time
        assert!(doc.undo());
        assert_eq!(bytes(&doc), before, "undo {label}");
        assert!(doc.redo());
        assert_eq!(bytes(&doc), after, "redo {label}");
        states.push(after);
    }

    // the whole sequence back and forth
    for state in states.iter().rev().skip(1) {
        assert!(doc.undo());
        assert_eq!(&bytes(&doc), state);
    }
    assert!(!doc.undo() && !doc.can_undo());
    for state in &states[1..] {
        assert!(doc.redo());
        assert_eq!(&bytes(&doc), state);
    }
    assert!(!doc.redo() && !doc.can_redo());
    let labels: Vec<_> = mixed_steps().into_iter().map(|(l, _)| l).collect();
    assert_eq!(doc.history().collect::<Vec<_>>(), labels);
}

#[test]
fn a_failing_batch_changes_nothing() {
    let no_second_gem = LevelId::all()
        .find(|l| l.level().kind == Kind::Level && !l.level().second_gem)
        .unwrap();
    let failing = vec![
        Edit::SetLives { count: 7 },
        Edit::SetSecondGem {
            level: no_second_gem,
            on: true,
        },
    ];

    let mut doc = open("tests/saves/game/0019.sav");
    let before = bytes(&doc);
    let err = doc.apply("bad", failing.clone()).unwrap_err();
    assert_eq!(err.code(), "not_applicable");
    assert_eq!(bytes(&doc), before);
    assert_eq!(doc.history().count(), 0);
    assert!(!doc.is_dirty() && !doc.can_undo());
    assert!(doc.changes().is_empty());

    // with history on both sides
    doc.apply("lives", vec![Edit::SetLives { count: 1 }])
        .unwrap();
    doc.apply("lives", vec![Edit::SetLives { count: 2 }])
        .unwrap();
    doc.undo();
    let before = bytes(&doc);
    assert!(doc.apply("bad", failing).is_err());
    assert_eq!(bytes(&doc), before);
    assert_eq!(doc.history().count(), 1);
    assert!(doc.can_redo(), "a failed apply keeps the redo step");
    assert!(doc.is_dirty());
}

#[test]
fn dirty_flag_follows_the_saved_state() {
    let mut doc = open("tests/saves/game/0019.sav");
    assert!(!doc.is_dirty());
    doc.apply("lives", vec![Edit::SetLives { count: 42 }])
        .unwrap();
    assert!(doc.is_dirty());
    doc.mark_saved(None);
    assert!(!doc.is_dirty());
    assert_eq!(doc.file_name(), Some("tests/saves/game/0019.sav"));
    doc.undo();
    assert!(doc.is_dirty());
    doc.redo();
    assert!(!doc.is_dirty());

    // Back at the saved state by undo, then a new edit: its state is new,
    // even though it replaces the redo step.
    doc.apply("lives", vec![Edit::SetLives { count: 43 }])
        .unwrap();
    doc.undo();
    assert!(!doc.is_dirty());
    doc.undo();
    assert!(doc.is_dirty());
    doc.apply("lives", vec![Edit::SetLives { count: 42 }])
        .unwrap();
    assert!(doc.is_dirty(), "same bytes as saved, but a different step");
    assert!(!doc.can_redo());

    doc.mark_saved(Some("copy.sav".to_owned()));
    assert!(!doc.is_dirty());
    assert_eq!(doc.file_name(), Some("copy.sav"));
}

#[test]
fn undo_keeps_the_last_steps() {
    let mut doc = open("tests/saves/game/0019.sav");
    for i in 0..UNDO_DEPTH + 5 {
        let count = i as u8;
        doc.apply(format!("lives {count}"), vec![Edit::SetLives { count }])
            .unwrap();
    }
    let history: Vec<_> = doc.history().collect();
    assert_eq!(history.len(), UNDO_DEPTH);
    assert_eq!(history[0], "lives 5");
    assert_eq!(
        *history.last().unwrap(),
        format!("lives {}", UNDO_DEPTH + 4)
    );
    while doc.undo() {}
    assert_eq!(doc.current().lives(), 4);
    assert!(doc.is_dirty());
}

#[test]
fn platform_and_size() {
    let doc = open("tests/saves/game/0019.sav");
    assert_eq!(doc.platform(), Platform::Switch);
    let size = doc.size_report();
    assert_eq!(size.capacity, Some(153_600));
    assert!(size.used < 80_000, "{size:?}");
    assert!(size.fits());
    assert_eq!(bytes(&doc).len(), 153_600);

    let mut doc = open("tests/saves/game/ALCHEMYXSAVE_8.sav");
    assert_eq!(doc.platform(), Platform::Pc);
    assert_eq!(doc.size_report().capacity, None);
    assert_eq!(doc.size_report().used, bytes(&doc).len());

    // The size follows edits, undo and redo.
    let before = doc.size_report();
    doc.apply(
        "flag",
        vec![Edit::RawFlagSet {
            key: "HintDisplay_99".to_owned(),
        }],
    )
    .unwrap();
    let after = doc.size_report();
    assert!(after.used > before.used);
    assert_eq!(after.used, bytes(&doc).len());
    doc.undo();
    assert_eq!(doc.size_report(), before);
    doc.redo();
    assert_eq!(doc.size_report(), after);
}

#[test]
fn a_switch_save_that_outgrows_its_file_is_kept_but_not_written() {
    // The longest flag key that still fits `initial_game.sav` is 91,000
    // characters (test vectors `edge/switch_exact_fit`, `edge/switch_overflow`).
    let mut doc = open("tests/saves/game/initial_game.sav");
    let key = |n| Edit::RawFlagSet { key: "x".repeat(n) };
    doc.apply("fits", vec![key(91_000)]).unwrap();
    assert_eq!(doc.size_report().used, 153_600);
    assert!(doc.size_report().fits());
    assert_eq!(bytes(&doc).len(), 153_600);
    doc.undo();

    doc.apply("too long", vec![key(91_001)]).unwrap();
    let size = doc.size_report();
    assert_eq!(size.used, 153_601);
    assert!(!size.fits());
    assert_eq!(
        doc.to_bytes(),
        Err(SaveError::DoesNotFit {
            needed: 153_601,
            capacity: 153_600
        })
    );
    doc.undo();
    assert!(doc.size_report().fits());
    assert!(doc.to_bytes().is_ok());
}

/// The scripted session of the save review dialog.
fn review_session() -> Document {
    let mut doc = open("tests/saves/game/0019.sav");
    assert_eq!(doc.current().lives(), 41);
    let deep = id("deeptrouble");
    assert!(!doc.current().entered(deep));
    for edit in [
        Edit::SetLives { count: 42 },
        Edit::SetCrystal {
            level: deep,
            on: true,
        },
        Edit::SetAbility {
            ability: Ability::SpeedShoes,
            on: true,
        },
    ] {
        let label = edit.describe(doc.current());
        doc.apply(label, vec![edit]).unwrap();
    }
    doc
}

#[test]
fn save_review() {
    let doc = review_session();
    assert_eq!(
        doc.history().collect::<Vec<_>>(),
        ["Lives: 42", "Deep Trouble: crystal on", "Speed Shoes: on"]
    );
    let review = doc.review();
    let rows: Vec<_> = review
        .rows
        .iter()
        .map(|r| (r.group, r.subject.clone(), r.details.clone()))
        .collect();
    assert_eq!(
        rows,
        [
            (
                ChangeGroup::Levels,
                Subject::Level(id("deeptrouble")),
                vec![
                    "+ crystal".to_owned(),
                    "entered, completions 0 → 1".to_owned()
                ]
            ),
            (
                ChangeGroup::Abilities,
                Subject::Ability(Ability::SpeedShoes),
                vec!["off → on".to_owned()]
            ),
            (
                ChangeGroup::Summary,
                Subject::Summary("life_count".to_owned()),
                vec!["41 → 42".to_owned()]
            ),
        ]
    );
    assert_eq!(
        review.rows[0].to_string(),
        "Deep Trouble: + crystal; entered, completions 0 → 1"
    );
    assert_eq!(
        review.recomputed,
        [
            Change::Summary {
                key: "percent_complete".to_owned(),
                before: Some(SummaryValue::Float(19.0)),
                after: Some(SummaryValue::Float(20.0)),
            },
            Change::Summary {
                key: "crystal_count".to_owned(),
                before: Some(SummaryValue::Int(10)),
                after: Some(SummaryValue::Int(11)),
            },
        ]
    );
    // In the summary's own key order.
    let recomputed: Vec<_> = review.recomputed.iter().map(Change::describe).collect();
    assert_eq!(recomputed, ["Percentage: 19% → 20%", "Crystals: 10 → 11"]);

    // Fields 2, 3, 12 and 13 of Deep Trouble changed.
    let fields: Vec<_> = doc
        .changes()
        .iter()
        .filter_map(|c| match c {
            Change::Zone { field, .. } => Some(*field),
            _ => None,
        })
        .collect();
    assert_eq!(fields, [2, 3, 12, 13]);

    // Writing doesn't change the review or add a step.
    doc.to_bytes().unwrap();
    assert_eq!(doc.review(), review);
    assert_eq!(doc.history().count(), 3);
}

#[test]
fn a_save_without_changes_has_an_empty_review() {
    let doc = open("tests/saves/game/0019.sav");
    assert!(doc.changes().is_empty());
    assert_eq!(doc.review().rows, []);
    assert_eq!(doc.review().recomputed, []);
}

#[test]
fn preview_matches_apply_and_changes_nothing() {
    let edits = mixed_steps()
        .into_iter()
        .flat_map(|(_, e)| e)
        .collect::<Vec<_>>();
    let mut doc = open("tests/saves/game/0019.sav");
    let before = bytes(&doc);
    let preview = doc.preview(&edits).unwrap();
    assert_eq!(bytes(&doc), before);
    assert!(!doc.is_dirty() && !doc.can_undo());

    doc.apply("all", edits).unwrap();
    assert_eq!(preview.changes, doc.changes());
    assert_eq!(preview.rows, doc.review().rows);
    assert_eq!(preview.recomputed, doc.review().recomputed);
    assert_eq!(preview.totals, doc.current().totals());
    assert_eq!(preview.warnings, doc.current().warnings());
    assert_eq!(preview.size, doc.size_report());
    assert_eq!(&preview.save, doc.current());
}

#[test]
fn preview_shows_what_the_edits_add_to_the_current_state() {
    let doc = review_session();
    let save = doc.current();
    let level = LevelId::all()
        .find(|&l| l.level().kind == Kind::Level && save.entered(l) && !save.gem(l))
        .unwrap();
    let preview = doc.preview(&[Edit::SetGem { level, on: true }]).unwrap();
    // The earlier edits are already in the document: only the new gem shows.
    let rows: Vec<_> = preview.rows.iter().map(ToString::to_string).collect();
    assert_eq!(rows, [format!("{}: + clear gem", level.name())]);
    let gems = save.totals().gems;
    let recomputed: Vec<_> = preview.recomputed.iter().map(Change::describe).collect();
    assert_eq!(
        recomputed,
        [
            "Percentage: 20% → 21%".to_owned(),
            format!("Gems: {gems} → {}", gems + 1)
        ]
    );

    let err = doc
        .preview(&[Edit::SetCrystal {
            level: id("tinytiger"),
            on: true,
        }])
        .unwrap_err();
    assert!(matches!(err, EditError::UnknownLevel(_)));
}

#[test]
fn redo_history_and_generation() {
    let mut doc = open("tests/saves/game/0019.sav");
    let opened = doc.generation();
    for count in 1..=3 {
        doc.apply(format!("lives {count}"), vec![Edit::SetLives { count }])
            .unwrap();
    }
    let last = doc.generation();
    assert_ne!(last, opened);
    assert_eq!(doc.redo_history().count(), 0);

    doc.undo();
    doc.undo();
    assert_eq!(
        doc.redo_history().collect::<Vec<_>>(),
        ["lives 2", "lives 3"],
        "next first"
    );
    assert_eq!(doc.history().collect::<Vec<_>>(), ["lives 1"]);
    doc.undo();
    assert_eq!(doc.generation(), opened, "undo brings the number back");
    doc.redo();
    doc.redo();
    doc.redo();
    assert_eq!(doc.generation(), last);

    // a new step after an undo gets a number never used before
    doc.undo();
    doc.apply("lives 9", vec![Edit::SetLives { count: 9 }])
        .unwrap();
    assert_eq!(doc.redo_history().count(), 0);
    assert!(doc.generation() > last);
}
