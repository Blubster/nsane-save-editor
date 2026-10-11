//! The Crash 3 read model against the saves the game wrote and the live test
//! saves (`docs/FORMAT.md` §6, §7, §10, §11).

use std::path::{Path, PathBuf};

use nsane::Error;
use nsane::crash3::{Ability, Crash3Save, LevelId, Relic, Totals, Warning};
use nsane::igb::IgbFile;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sav_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sav_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "sav") {
            out.push(path);
        }
    }
}

/// The saves the game wrote during the playthrough, plus a fresh PC game.
fn game_saves() -> Vec<PathBuf> {
    let mut out = Vec::new();
    sav_files(&repo().join("saves"), &mut out);
    out.sort();
    assert_eq!(out.len(), 21);
    out
}

fn all_saves() -> Vec<PathBuf> {
    let mut out = game_saves();
    sav_files(&repo().join("test_saves"), &mut out);
    assert_eq!(out.len(), 69);
    out
}

fn load_path(path: &Path) -> Crash3Save {
    let data = std::fs::read(path).unwrap();
    Crash3Save::parse(&data).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn load(rel: &str) -> Crash3Save {
    load_path(&repo().join(rel))
}

fn name(path: &Path) -> String {
    path.file_name().unwrap().to_string_lossy().into_owned()
}

fn id(key: &str) -> LevelId {
    LevelId::from_key(key).unwrap()
}

#[test]
fn every_save_opens() {
    for path in all_saves() {
        load_path(&path);
    }
}

#[test]
fn recomputed_summary_matches_game() {
    for path in game_saves() {
        let mut save = load_path(&path);
        let before = save.igb().summary().clone();
        save.update_summary().unwrap();
        assert_eq!(save.igb().summary(), &before, "{}", name(&path));
    }
}

#[test]
fn unedited_game_saves_are_written_back_unchanged() {
    for path in game_saves() {
        let data = std::fs::read(&path).unwrap();
        let mut save = Crash3Save::parse(&data).unwrap();
        assert!(save.to_bytes().unwrap() == data, "{}", name(&path));
    }
}

#[test]
fn farthest_progression_rule_matches_game() {
    for path in game_saves() {
        let save = load_path(&path);
        let stored = save.farthest_progression_index();
        if name(&path) == "0001.sav" {
            // Quitting Toad Village at once stored 1 without marking it entered.
            assert_eq!((save.farthest_progression(), stored), (0, 1));
        } else {
            assert_eq!(
                save.farthest_progression() as i32,
                stored,
                "{}",
                name(&path)
            );
        }
    }
}

#[test]
fn field12_mirrors_crystal() {
    for path in all_saves() {
        let save = load_path(&path);
        for level in LevelId::all() {
            assert_eq!(
                save.crystal_field(level),
                save.crystal(level),
                "{} {}",
                name(&path),
                level.key()
            );
        }
    }
}

#[test]
fn finishing_without_crystal_only_marks_entered() {
    let level = id("doubleheader");
    let (before, after) = (load("saves/0017.sav"), load("saves/0018.sav"));
    assert!(!before.entered(level) && after.entered(level));
    assert_eq!(after.flags(level), 0);
    assert!(!after.crystal_field(level));
}

#[test]
fn boss_portal_glitch_warning() {
    for (file, glitch) in [
        ("t4_crystals.sav", true),
        ("t4c_room5.sav", true),
        ("t4h_total22.sav", true),
        ("t4b_room4.sav", false),
        ("t4d_rooms1to4.sav", false),
        ("t4f_mixed20.sav", false),
        ("t4g_room3_short.sav", false),
        ("t6_full108.sav", false),
    ] {
        let warnings = load(&format!("test_saves/{file}")).warnings();
        assert_eq!(!warnings.is_empty(), glitch, "{file}");
    }

    let warnings = load("test_saves/t4_crystals.sav").warnings();
    let expected = Warning::PortalGlitch {
        boss: id("ngin"),
        crystals: 22,
        room: 4,
    };
    assert_eq!(warnings, std::slice::from_ref(&expected));
    assert_eq!(expected.code(), "portal_glitch");
    assert_eq!(
        expected.message(),
        "N. Gin is not defeated but there are 22 crystals (more than 20): his portal will \
         glitch in warp room 4. Defeat him or remove crystals."
    );
}

#[test]
fn game_saves_have_no_warning() {
    for path in game_saves() {
        assert_eq!(load_path(&path).warnings(), [], "{}", name(&path));
    }
}

#[test]
fn bonus_gem_due_matches_the_game() {
    // Every relic gold and 107%, gem not taken: the game spawned it (T7).
    assert!(load("test_saves/t7_bonus_gem.sav").bonus_gem_due());
    assert!(!load("test_saves/t7_after_beating_cortex.sav").bonus_gem_due());
    assert!(!load("test_saves/t6_after.sav").bonus_gem_due());
    assert!(!load("saves/0019.sav").bonus_gem_due());
    assert!(load("test_saves/t7_after_beating_cortex.sav").bonus_gem());
}

/// Levels of `saves/0019.sav` with progress: (key, crystal, gem, second gem,
/// colored gem, relic, times). Every other level has none.
type Row = (
    &'static str,
    bool,
    bool,
    bool,
    bool,
    Option<Relic>,
    &'static [u32],
);

#[rustfmt::skip]
const SAVE_0019: &[Row] = &[
    ("toadvillage", true, true, false, false, Some(Relic::Gold), &[5544, 6023]),
    ("underpressure", true, true, false, false, None, &[]),
    ("orientexpress", true, false, false, false, None, &[]),
    ("boneyard", true, true, false, false, None, &[]),
    ("makinwaves", true, true, false, false, None, &[]),
    ("geewiz", true, true, false, false, None, &[]),
    ("hangemhigh", true, true, false, false, None, &[]),
    ("hogride", true, false, false, false, None, &[]),
    ("tombtime", true, false, false, false, None, &[]),
    ("midnightrun", true, false, false, false, None, &[]),
    ("doubleheader", false, false, false, false, None, &[]),
];

#[test]
fn save_0019_reads_as_the_game_wrote_it() {
    let save = load("saves/0019.sav");

    let entered = SAVE_0019;
    let bosses = ["tinytiger", "dingodile"];
    for level in LevelId::all() {
        let key = level.key();
        let row = entered.iter().find(|r| r.0 == key);
        let (crystal, gem, second, colored, relic, times) = match row {
            Some(&(_, c, g, s, col, r, t)) => (c, g, s, col, r, t),
            None => (false, false, false, false, None, &[][..]),
        };
        assert_eq!(save.crystal(level), crystal, "{key} crystal");
        assert_eq!(save.gem(level), gem, "{key} gem");
        assert_eq!(save.second_gem(level), second, "{key} second gem");
        assert_eq!(save.colored_gem(level), colored, "{key} colored gem");
        assert_eq!(save.relic(level), relic, "{key} relic");
        assert_eq!(save.times(level), times, "{key} times");
        assert_eq!(
            save.boss_defeated(level),
            bosses.contains(&key),
            "{key} boss"
        );
        assert_eq!(
            save.entered(level),
            row.is_some() || bosses.contains(&key),
            "{key} entered"
        );
        assert_eq!(save.flags_unknown(level), 0, "{key}");
    }
    assert_eq!(save.completions(id("toadvillage")), 5);
    assert_eq!(save.completions(id("doubleheader")), 1);
    assert_eq!(save.completions(id("dinomight")), 0);

    let t = save.totals();
    assert_eq!(
        (t.crystals, t.gems, t.sapphire, t.gold, t.platinum, t.bosses),
        (10, 6, 0, 1, 0, 2)
    );
    assert_eq!((t.gem_breakdown.clear, t.gem_breakdown.bonus), (6, 0));
    assert_eq!((t.relics(), t.percent(), save.percent()), (1, 19, 19));
    assert!(!save.bonus_gem());

    assert!(save.ability(Ability::BodySlam) && save.ability(Ability::DoubleJump));
    assert!(!save.ability(Ability::TornadoSpin));
    assert!(!save.ability(Ability::Bazooka) && !save.ability(Ability::SpeedShoes));

    assert_eq!(save.lives(), 41);
    assert_eq!(
        save.previous_level_name().as_deref(),
        Some("crash3/l301_toadvillage/l301_toadvillage")
    );
    assert_eq!(save.previous_level_progression_index(), Some(1));
    assert_eq!(save.active_progression_index(), Some(0));
    assert!(!save.story_completed());
    assert_eq!(save.farthest_progression_index(), 18);
    assert!(save.cutscene_seen("C3_Hub2_Intro"));
    assert!(!save.cutscene_seen("C3_PreFinalBattle"));
}

#[test]
fn full_saves_reach_108_percent() {
    let save = load("test_saves/t7_after_beating_cortex.sav");
    let t: Totals = save.totals();
    assert_eq!(t.percent(), 108);
    assert_eq!(
        (
            t.gem_breakdown.clear,
            t.gem_breakdown.second,
            t.gem_breakdown.colored,
            t.gem_breakdown.bonus
        ),
        (31, 10, 5, 1)
    );
    assert!(save.story_completed() && save.bonus_gem());
    assert!(save.flag("OneHundredCortexBeat"));
    assert!(!load("test_saves/t7_bonus_gem.sav").flag("OneHundredCortexBeat"));
}

#[test]
fn fresh_saves_have_no_progress() {
    for rel in ["saves/initial_game.sav", "saves/ALCHEMYXSAVE_8.sav"] {
        let save = load(rel);
        assert_eq!(save.totals(), Totals::default(), "{rel}");
        assert_eq!(save.previous_level_name(), None, "{rel}");
        assert_eq!(save.previous_level_progression_index(), Some(-1), "{rel}");
        assert_eq!(save.zones().count(), 116, "{rel}");
        assert!(LevelId::all().all(|l| !save.entered(l)), "{rel}");
    }
}

#[test]
fn files_that_are_not_saves_are_errors() {
    assert!(matches!(Crash3Save::parse(&[]), Err(Error::Igb(_))));
    let data = std::fs::read(repo().join("saves/0019.sav")).unwrap();
    assert!(Crash3Save::parse(&data[..1000]).is_err());
    let igb = IgbFile::parse(&data).unwrap();
    assert!(Crash3Save::from_igb(igb).is_ok());

    // Valid containers without the expected objects: same-length renames.
    for (from, to, reason) in [
        ("CZoneInfoSaveList", "CZoneInfoSaveLisX", "no zone list"),
        (
            "Upgrade_DoubleJump",
            "Upgrade_DoubleJumX",
            "no Upgrade_DoubleJump variable",
        ),
        ("life_count", "life_counX", "no life_count in the summary"),
    ] {
        let edited = replace_all(&data, from.as_bytes(), to.as_bytes());
        IgbFile::parse(&edited).unwrap();
        assert_eq!(
            Crash3Save::parse(&edited),
            Err(Error::NotASave(reason.to_owned())),
            "{from}"
        );
    }
}

fn replace_all(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = data.to_vec();
    let mut i = 0;
    while let Some(pos) = out[i..].windows(from.len()).position(|w| w == from) {
        out[i + pos..i + pos + from.len()].copy_from_slice(to);
        i += pos + from.len();
    }
    assert!(out != data, "{} not found", String::from_utf8_lossy(from));
    out
}
