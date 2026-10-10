//! Diagnostics and their fixes. Mismatches that only another tool could
//! write are made by changing session fields directly.

use std::cmp::Reverse;
use std::path::{Path, PathBuf};

use nsane::crash3::presets::{self, AllRelics, CompleteAll, ReachRelicCount, StoryUpTo};
use nsane::crash3::tables::{bit, field};
use nsane::crash3::{Crash3Save, Diagnostic, Edit, LevelId, Relic, Severity};
use nsane::igb::FieldValue;
use nsane::{Document, Platform};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> Vec<u8> {
    let path = repo().join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Opens a save under its own file name, as the GUI does.
fn open(rel: &str) -> Document {
    let name = Path::new(rel).file_name().unwrap().to_string_lossy();
    Document::open(&read(rel), Some(name.into_owned())).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn id(key: &str) -> LevelId {
    LevelId::from_key(key).unwrap()
}

/// The saves of a directory, recursively, as paths relative to the repository.
fn saves_in(dir: &str) -> Vec<String> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "sav") {
                out.push(path);
            }
        }
    }
    let root = repo();
    let mut paths = Vec::new();
    walk(&root.join(dir), &mut paths);
    paths.sort();
    paths
        .iter()
        .map(|p| {
            p.strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

/// The diagnostics with this code and level. Also checks the order.
fn find<'a>(doc: &'a Document, code: &str, level: Option<LevelId>) -> Vec<&'a Diagnostic> {
    let all = doc.diagnostics();
    assert!(all.is_sorted_by_key(|d| Reverse(d.severity)), "{all:#?}");
    all.iter()
        .filter(|d| d.code == code && d.level == level)
        .collect()
}

fn the<'a>(doc: &'a Document, code: &str, level: Option<LevelId>) -> &'a Diagnostic {
    match find(doc, code, level)[..] {
        [d] => d,
        _ => panic!(
            "expected one {code} for {level:?}: {:#?}",
            doc.diagnostics()
        ),
    }
}

/// Applies the diagnostic's fix as one step and checks that it is gone.
fn apply_fix(doc: &mut Document, code: &str, level: Option<LevelId>) {
    let fix = the(doc, code, level)
        .fix
        .clone()
        .unwrap_or_else(|| panic!("{code} has no fix"));
    doc.apply(fix.label.clone(), fix.edits)
        .unwrap_or_else(|e| panic!("{code}: {e}"));
    assert!(find(doc, code, level).is_empty(), "{code} is still there");
    assert_eq!(doc.history().last(), Some(fix.label.as_str()));
}

fn serious(doc: &Document) -> Vec<&Diagnostic> {
    doc.diagnostics()
        .iter()
        .filter(|d| d.severity >= Severity::Warning)
        .collect()
}

/// `0019.sav` with session fields changed directly, as another tool might.
fn poked(level: LevelId, pokes: &[(usize, FieldValue)]) -> Document {
    let save = Crash3Save::parse(&read("saves/0019.sav")).unwrap();
    let session = save.session(level).unwrap().id;
    let mut igb = save.into_igb();
    for &(i, value) in pokes {
        *igb.object_mut(session).unwrap().field_mut(i).unwrap() = value;
    }
    Document::open(&igb.build().unwrap(), Some("0019.sav".to_owned())).unwrap()
}

/// The `collectibles` field of `level` with more bits set.
fn flags(save: &Crash3Save, level: LevelId, set: u32) -> (usize, FieldValue) {
    let value = save.flags(level) | set;
    (field::COLLECTIBLES, FieldValue::UInt(value))
}

fn bool_field(value: bool) -> (usize, FieldValue) {
    let value = u8::from(value);
    (field::CRYSTAL, FieldValue::Bool { value, padding: 0 })
}

#[test]
fn no_warning_or_error_on_the_saves_the_game_wrote() {
    // Stronger than the requirement: no diagnostic at all. Every save in
    // `saves/` comes from the game, which keeps its own data consistent.
    let saves = saves_in("saves");
    assert_eq!(saves.len(), 21);
    for rel in saves {
        let doc = open(&rel);
        assert_eq!(doc.diagnostics(), [], "{rel}");
    }
}

#[test]
fn every_fix_resolves_its_diagnostic() {
    let mut fixes = 0;
    for rel in saves_in("saves").into_iter().chain(saves_in("test_saves")) {
        let doc = open(&rel);
        for d in doc.diagnostics() {
            let Some(fix) = &d.fix else { continue };
            let mut fixed = doc.clone();
            fixed
                .apply(fix.label.clone(), fix.edits.clone())
                .unwrap_or_else(|e| panic!("{rel} {}: {e}", d.code));
            let left = find(&fixed, d.code, d.level);
            assert!(left.is_empty(), "{rel} {} {:?}", d.code, d.level);
            assert!(
                serious(&fixed).len() <= serious(&doc).len(),
                "{rel} {}",
                d.code
            );
            fixes += 1;
        }
    }
    assert!(fixes > 250, "{fixes}");
}

#[test]
fn portal_glitch_on_t4_defeats_n_gin() {
    let mut doc = open("test_saves/t4_crystals.sav");
    assert_eq!(doc.current().totals().crystals, 22);
    let ngin = id("ngin");
    let d = the(&doc, "portal_glitch", Some(ngin));
    assert_eq!(d.severity, Severity::Warning);
    assert!(d.message.contains("22 crystals"), "{}", d.message);
    let fix = d.fix.as_ref().unwrap();
    assert_eq!(fix.label, "Defeat N. Gin");
    assert_eq!(
        fix.edits,
        [Edit::SetBossDefeated {
            boss: ngin,
            on: true,
            skip_cutscenes: false
        }]
    );

    // The preview already shows the result.
    let preview = doc.preview(&fix.edits).unwrap();
    assert!(
        preview
            .diagnostics
            .iter()
            .all(|d| d.code != "portal_glitch")
    );

    apply_fix(&mut doc, "portal_glitch", Some(ngin));
    assert!(doc.current().boss_defeated(ngin));
    assert_eq!(serious(&doc), Vec::<&Diagnostic>::new());
    doc.undo();
    the(&doc, "portal_glitch", Some(ngin));
    doc.redo();
    assert!(find(&doc, "portal_glitch", Some(ngin)).is_empty());
}

#[test]
fn portal_glitch_with_12_crystals_defeats_the_first_two_bosses() {
    let mut doc = open("saves/initial_game.sav");
    let crystals: Vec<Edit> = LevelId::all()
        .filter(|l| l.level().number.is_some_and(|n| n <= 12))
        .map(|level| Edit::SetCrystal { level, on: true })
        .collect();
    doc.apply("12 crystals", crystals).unwrap();
    assert_eq!(doc.current().totals().crystals, 12);
    assert_eq!(doc.current().totals().bosses, 0);

    let tiny = id("tinytiger");
    let fix = the(&doc, "portal_glitch", Some(tiny)).fix.clone().unwrap();
    assert_eq!(fix.label, "Defeat bosses up to Dingodile");
    let boss = |key| Edit::SetBossDefeated {
        boss: id(key),
        on: true,
        skip_cutscenes: false,
    };
    assert_eq!(fix.edits, [boss("tinytiger"), boss("dingodile")]);

    apply_fix(&mut doc, "portal_glitch", Some(tiny));
    assert_eq!(serious(&doc), Vec::<&Diagnostic>::new());
    // 12 crystals ≤ 15: N. Tropy's portal is closed, not glitched.
    assert!(!doc.current().boss_defeated(id("ntropy")));

    // With exactly 10, Dingodile's portal works: only Tiny Tiger is needed.
    doc.undo();
    let off = |key| Edit::SetCrystal {
        level: id(key),
        on: false,
    };
    doc.apply("10 crystals", vec![off("dinomight"), off("deeptrouble")])
        .unwrap();
    assert_eq!(doc.current().totals().crystals, 10);
    let fix = the(&doc, "portal_glitch", Some(tiny)).fix.clone().unwrap();
    assert_eq!(fix.label, "Defeat Tiny Tiger");
    assert_eq!(fix.edits, [boss("tinytiger")]);
}

#[test]
fn bonus_gem_due_on_t7() {
    let mut doc = open("test_saves/t7_bonus_gem.sav");
    let d = the(&doc, "bonus_gem_due", None);
    assert_eq!(d.severity, Severity::Info);
    assert_eq!(d.fix.as_ref().unwrap().label, "Give the bonus gem now");
    assert!(!doc.current().bonus_gem());
    apply_fix(&mut doc, "bonus_gem_due", None);
    assert!(doc.current().bonus_gem());
    assert_eq!(doc.current().totals().percent(), 108);
}

#[test]
fn relic_time_mismatch_on_t1() {
    let mut doc = open("test_saves/t1_relics.sav");
    let waves = id("makinwaves");
    assert_eq!(doc.current().relic(waves), Some(Relic::Platinum));
    assert_eq!(doc.current().times(waves), Vec::<u32>::new());
    let d = the(&doc, "relic_time_mismatch", Some(waves));
    assert_eq!(d.severity, Severity::Info);
    assert_eq!(
        d.message,
        "Makin' Waves has a platinum relic but no time trial time. The game doesn't check this."
    );
    let t = presets::matching_time(waves, Relic::Platinum).unwrap();
    assert_eq!(
        d.fix.as_ref().unwrap().edits,
        [Edit::SetTimes {
            level: waves,
            times_cs: vec![t]
        }]
    );
    apply_fix(&mut doc, "relic_time_mismatch", Some(waves));
    assert_eq!(doc.current().times(waves), [t]);
}

#[test]
fn relic_time_mismatch_cases() {
    // A slow time, a time for a better relic, and a matching time.
    let toad = id("toadvillage");
    let mut doc = open("saves/initial_game.sav");
    let gold = Relic::Gold;
    let times = |level, times_cs| Edit::SetTimes { level, times_cs };
    let at = |relic| presets::matching_time(toad, relic).unwrap();
    doc.apply(
        "gold",
        vec![Edit::SetRelic {
            level: toad,
            relic: Some(gold),
        }],
    )
    .unwrap();
    for (time, earns) in [
        (at(Relic::Sapphire) + 2, "earns no relic"),
        (at(Relic::Sapphire), "earns sapphire"),
        (at(Relic::Platinum), "earns platinum"),
    ] {
        doc.apply("time", vec![times(toad, vec![time])]).unwrap();
        let d = the(&doc, "relic_time_mismatch", Some(toad));
        assert!(d.message.contains(earns), "{}", d.message);
    }
    doc.apply("time", vec![times(toad, vec![at(gold)])])
        .unwrap();
    assert!(find(&doc, "relic_time_mismatch", Some(toad)).is_empty());
    // Times without a relic: a failed time trial, nothing to report.
    doc.apply(
        "no relic",
        vec![Edit::SetRelic {
            level: toad,
            relic: None,
        }],
    )
    .unwrap();
    assert_eq!(doc.diagnostics(), []);
}

#[test]
fn crystal_field_mismatch() {
    let save = Crash3Save::parse(&read("saves/0019.sav")).unwrap();
    let toad = id("toadvillage");
    let road = id("roadcrash");
    assert!(save.crystal(toad) && !save.crystal(road));

    // Field off under the bit: the fix writes both on.
    let mut doc = poked(toad, &[bool_field(false)]);
    let d = the(&doc, "crystal_field_mismatch", Some(toad));
    assert_eq!(d.severity, Severity::Warning);
    apply_fix(&mut doc, "crystal_field_mismatch", Some(toad));
    assert!(doc.current().crystal(toad) && doc.current().crystal_field(toad));
    assert_eq!(doc.diagnostics(), []);

    // Field on without the bit: the fix writes both off.
    let mut doc = poked(road, &[bool_field(true)]);
    apply_fix(&mut doc, "crystal_field_mismatch", Some(road));
    assert!(!doc.current().crystal(road) && !doc.current().crystal_field(road));
    assert_eq!(doc.diagnostics(), []);
}

#[test]
fn relic_field_mismatch() {
    let save = Crash3Save::parse(&read("saves/0019.sav")).unwrap();
    let toad = id("toadvillage");
    assert_eq!(save.relic(toad), Some(Relic::Gold));

    // The tier says platinum, the bit says gold: the tier wins.
    let mut doc = poked(toad, &[(field::RELIC_TIER, FieldValue::Int(3))]);
    let d = the(&doc, "relic_field_mismatch", Some(toad));
    assert_eq!(d.severity, Severity::Warning);
    assert!(d.message.contains("says platinum"), "{}", d.message);
    apply_fix(&mut doc, "relic_field_mismatch", Some(toad));
    let bits = doc.current().flags(toad) & bit::RELICS;
    assert_eq!(bits, bit::RELIC_PLATINUM);
    assert_eq!(serious(&doc), Vec::<&Diagnostic>::new());

    // Two relic bits.
    let mut doc = poked(toad, &[flags(&save, toad, bit::RELIC_SAPPHIRE)]);
    let d = the(&doc, "relic_field_mismatch", Some(toad));
    assert!(d.message.contains("several"), "{}", d.message);
    apply_fix(&mut doc, "relic_field_mismatch", Some(toad));
    assert_eq!(doc.current().flags(toad) & bit::RELICS, bit::RELIC_GOLD);
    assert_eq!(doc.diagnostics(), []);

    // A tier the game never writes reads as no relic.
    let mut doc = poked(toad, &[(field::RELIC_TIER, FieldValue::Int(7))]);
    let d = the(&doc, "relic_field_mismatch", Some(toad));
    assert!(d.message.contains("unknown value (7)"), "{}", d.message);
    apply_fix(&mut doc, "relic_field_mismatch", Some(toad));
    assert_eq!(doc.current().relic_tier(toad), 0);
    assert_eq!(doc.diagnostics(), []);
}

#[test]
fn relic_counted_mismatch() {
    let toad = id("toadvillage");
    let road = id("roadcrash");

    // Missing with a relic: the fix sets the relic again, flag included.
    let mut doc = open("saves/0019.sav");
    let key = |name: &str| format!("{name}_RelicCounted");
    doc.apply(
        "poke",
        vec![Edit::RawFlagRemove {
            key: key("Toad Village"),
        }],
    )
    .unwrap();
    let d = the(&doc, "relic_counted_mismatch", Some(toad));
    assert_eq!(d.severity, Severity::Warning);
    apply_fix(&mut doc, "relic_counted_mismatch", Some(toad));
    assert!(doc.current().flag(&key("Toad Village")));
    assert_eq!(doc.diagnostics(), []);

    // Present without a relic: the fix removes it.
    let mut doc = open("saves/0019.sav");
    doc.apply(
        "poke",
        vec![Edit::RawFlagSet {
            key: key("Road Crash"),
        }],
    )
    .unwrap();
    apply_fix(&mut doc, "relic_counted_mismatch", Some(road));
    assert!(!doc.current().flag(&key("Road Crash")));
    assert_eq!(doc.diagnostics(), []);
}

#[test]
fn colored_gem_mismatch() {
    let save = Crash3Save::parse(&read("saves/0019.sav")).unwrap();
    let hang = id("hangemhigh");
    assert!(!save.colored_gem(hang));
    assert!(save.entered(hang));

    // The zone bit without the `Gem_Yellow` variable: the fix sets both.
    let mut doc = poked(hang, &[flags(&save, hang, bit::GEM_YELLOW)]);
    let d = the(&doc, "colored_gem_mismatch", Some(hang));
    assert_eq!(d.severity, Severity::Warning);
    assert!(d.message.contains("Gem_Yellow"), "{}", d.message);
    apply_fix(&mut doc, "colored_gem_mismatch", Some(hang));
    let yellow = hang.level().colored_gem.unwrap();
    assert!(doc.current().colored_gem(hang));
    assert!(doc.current().colored_gem_variable(yellow));
    assert_eq!(doc.diagnostics(), []);
}

#[test]
fn progress_not_entered() {
    let save = Crash3Save::parse(&read("saves/0019.sav")).unwrap();
    let road = id("roadcrash");
    assert!(!save.entered(road));

    // A relic written without its flag also shows as such.
    let gem = [flags(&save, road, bit::GEM_CLEAR)];
    let relic = [
        (field::RELIC_TIER, FieldValue::Int(1)),
        flags(&save, road, bit::RELIC_SAPPHIRE),
    ];
    let times = [(field::TIMES[0], FieldValue::Int(9000))];
    let cases: [(&[_], &[&str]); 3] = [
        (&gem, &[]),
        (&relic, &["relic_counted_mismatch"]),
        (&times, &[]),
    ];
    for (pokes, left) in cases {
        let mut doc = poked(road, pokes);
        let d = the(&doc, "progress_not_entered", Some(road));
        assert_eq!(d.severity, Severity::Warning);
        apply_fix(&mut doc, "progress_not_entered", Some(road));
        assert!(doc.current().entered(road));
        assert_eq!(doc.current().completions(road), 1);
        let codes: Vec<_> = serious(&doc).iter().map(|d| d.code).collect();
        assert_eq!(codes, left);
    }
}

#[test]
fn bits_only_another_tool_writes() {
    let save = Crash3Save::parse(&read("saves/0019.sav")).unwrap();
    let toad = id("toadvillage");
    let doc = poked(toad, &[flags(&save, toad, 0x0001 | 0x8000)]);
    let d = the(&doc, "unknown_flag_bits", Some(toad));
    assert_eq!(d.severity, Severity::Info);
    assert!(d.message.contains("0x8001"), "{}", d.message);
    assert_eq!(d.fix, None);
    assert_eq!(serious(&doc), Vec::<&Diagnostic>::new());

    // A crystal on level 26, and on a boss. With the crystal field left at 0,
    // the field mismatch shows too, without a fix: the crystal edit refuses
    // these levels.
    for key in ["skicrazed", "tinytiger"] {
        let level = id(key);
        let doc = poked(level, &[flags(&save, level, bit::CRYSTAL)]);
        let d = the(&doc, "crystal_on_level_without_crystal", Some(level));
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.fix, None);
        let d = the(&doc, "crystal_field_mismatch", Some(level));
        assert_eq!(d.fix, None);
    }
}

#[test]
fn switch_size() {
    // The longest flag key that still fits `initial_game.sav` is 91,000
    // characters (test vectors `edge/switch_exact_fit`, `edge/switch_overflow`).
    let mut doc = open("saves/initial_game.sav");
    let key = |n| Edit::RawFlagSet { key: "x".repeat(n) };

    doc.apply("fits", vec![key(91_000)]).unwrap();
    let d = the(&doc, "switch_size", None);
    assert_eq!(d.severity, Severity::Info);
    assert_eq!(
        d.message,
        "The save data uses 153,600 of the 153,600 bytes of a Switch save (100%)."
    );
    doc.undo();

    doc.apply("too long", vec![key(91_001)]).unwrap();
    let d = the(&doc, "switch_size", None);
    assert_eq!(d.severity, Severity::Error);
    assert_eq!(d.fix, None);
    assert_eq!(doc.diagnostics()[0].code, "switch_size");
    doc.undo();
    assert_eq!(doc.diagnostics(), []);

    // About 80%: 122,880 bytes. The fresh save uses about 64,800.
    let used = doc.size_report().used;
    doc.apply("80%", vec![key(91_000 - (153_600 - 122_880))])
        .unwrap();
    assert_eq!(doc.size_report().used, 122_880);
    assert!(find(&doc, "switch_size", None).is_empty());
    doc.undo();
    doc.apply("81%", vec![key(91_000 - (153_600 - 124_416))])
        .unwrap();
    assert_eq!(the(&doc, "switch_size", None).severity, Severity::Info);
    assert!(used < 122_880);
}

#[test]
fn pc_file_name() {
    let data = read("saves/ALCHEMYXSAVE_8.sav");
    let doc = Document::open(&data, Some("ALCHEMYXSAVE_8.sav".to_owned())).unwrap();
    assert_eq!(doc.platform(), Platform::Pc);
    assert_eq!(doc.diagnostics(), []);
    let doc = Document::open(&data, None).unwrap();
    assert_eq!(doc.diagnostics(), []);

    let mut doc = Document::open(&data, Some("foo.sav".to_owned())).unwrap();
    let d = the(&doc, "pc_file_name", None);
    assert_eq!(d.severity, Severity::Info);
    assert!(d.message.contains("ALCHEMYXSAVE_<n>.sav"));
    assert_eq!(d.fix, None);
    // Saving under a valid name refreshes the diagnostics.
    doc.mark_saved(Some("ALCHEMYXSAVE_3.sav".to_owned()));
    assert_eq!(doc.diagnostics(), []);

    // A Switch save can have any name.
    let doc = Document::open(&read("saves/0019.sav"), Some("foo.sav".to_owned())).unwrap();
    assert_eq!(doc.platform(), Platform::Switch);
    assert_eq!(doc.diagnostics(), []);
}

#[test]
fn presets_with_their_defaults_leave_no_warning() {
    for rel in ["saves/initial_game.sav", "saves/0019.sav"] {
        let doc = open(rel);
        let save = doc.current();
        let presets: Vec<(&str, Vec<Edit>)> = vec![
            (
                "complete all",
                presets::complete_all(save, &CompleteAll::default()),
            ),
            (
                "story",
                presets::story_up_to(presets::default_story_room(save), &StoryUpTo::default())
                    .unwrap(),
            ),
            (
                "all relics",
                presets::all_relics(
                    save,
                    presets::DEFAULT_ALL_RELICS_TIER,
                    &AllRelics::default(),
                ),
            ),
            (
                "relic count",
                presets::reach_relic_count(
                    save,
                    presets::default_relic_count_target(save),
                    &ReachRelicCount::default(),
                ),
            ),
            (
                "abilities",
                presets::all_abilities(presets::DEFAULT_ABILITIES_ON),
            ),
            (
                "scenes",
                presets::all_cutscenes(presets::DEFAULT_CUTSCENES_SEEN),
            ),
        ];
        for (name, edits) in presets {
            let mut doc = doc.clone();
            doc.apply(name, edits).unwrap();
            assert_eq!(serious(&doc), Vec::<&Diagnostic>::new(), "{rel} {name}");
        }
    }
}
