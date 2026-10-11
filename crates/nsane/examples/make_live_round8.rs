//! Builds the saves of a round of in-game tests into `test_saves/round8/`:
//! the Switch ↔ PC conversion, resetting a level, two presets, the bonus gem
//! with one relic below gold, the largest life count, and a PC save that
//! shows the relic times of a few levels on their time panels.
//!
//!     cargo run -p nsane --example make_live_round8
//!
//! Each file has its own life count, so the save slot screen tells which one
//! is loaded. Every file is read back and checked before it is written. A
//! file that already exists is never replaced: once loaded in game it is
//! test evidence, so a run that would write other bytes stops instead.

use std::path::{Path, PathBuf};

use nsane::Platform;
use nsane::crash3::presets::{self, AllRelics, CompleteAll, StoryUpTo};
use nsane::crash3::{
    ABILITIES, BOSSES, CUTSCENES, Crash3Save, Edit, Kind, LevelId, Relic, relic_for_time,
};
use nsane::diff::{Change, diff};
use nsane::transfer::transfer;

const SWITCH_SIZE: usize = 153_600;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(rel: &str) -> Crash3Save {
    let path = repo().join(rel);
    let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Crash3Save::parse(&data).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn level(key: &str) -> LevelId {
    LevelId::from_key(key).unwrap_or_else(|| panic!("no level {key}"))
}

fn apply(save: &mut Crash3Save, edits: &[Edit]) {
    save.apply_all(edits).unwrap();
}

fn lives(save: &mut Crash3Save, count: u8) {
    apply(save, &[Edit::SetLives { count }]);
}

/// What writing `save` stores: its summary recomputed.
fn summarized(save: &Crash3Save) -> Crash3Save {
    let mut save = save.clone();
    save.update_summary().unwrap();
    save
}

/// Every level that isn't a boss: the 31 that have a relic.
fn relic_levels() -> impl Iterator<Item = LevelId> {
    LevelId::all().filter(|l| l.level().kind != Kind::Boss)
}

/// The expected progress of a file, as the save slot screen counts it.
struct Expect {
    platform: Platform,
    lives: i32,
    percent: u32,
    crystals: u32,
    gems: u32,
    relics: u32,
    bosses: u32,
}

/// Builds the bytes, reads them back and checks them against `expect`.
fn build(name: &str, mut save: Crash3Save, expect: &Expect) -> (Vec<u8>, Crash3Save) {
    let bytes = save.to_bytes().unwrap();
    let back = Crash3Save::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
    let t = back.totals();
    let got = (
        Platform::of(&back),
        back.lives(),
        back.percent(),
        t.crystals,
        t.gems,
        t.relics(),
        t.bosses,
    );
    let want = (
        expect.platform,
        expect.lives,
        expect.percent,
        expect.crystals,
        expect.gems,
        expect.relics,
        expect.bosses,
    );
    assert_eq!(
        got, want,
        "{name}: (platform, lives, %, crystals, gems, relics, bosses)"
    );
    match expect.platform {
        Platform::Switch => assert_eq!(bytes.len(), SWITCH_SIZE, "{name}"),
        Platform::Pc => assert!(bytes.len() < SWITCH_SIZE, "{name}"),
    }
    assert_eq!(back.warnings(), [], "{name}");
    (bytes, back)
}

/// Writes a new file, or checks that an existing one has the same bytes.
fn write(dir: &Path, name: &str, bytes: &[u8]) {
    let path = dir.join(name);
    match std::fs::read(&path) {
        Ok(existing) if existing == bytes => println!("{name}: unchanged"),
        Ok(_) => panic!(
            "{}: exists with other bytes; it may have been tested in game, so it is \
             not replaced. Move it away to build it again.",
            path.display()
        ),
        Err(_) => {
            std::fs::write(&path, bytes).unwrap();
            println!("{name}: written, {} bytes", bytes.len());
        }
    }
}

/// A conversion carries all the game data: the converted file, read back,
/// differs from the source in nothing (the source with its summary
/// recomputed, as writing it would store).
fn assert_same_game_data(name: &str, source: &Crash3Save, converted: &Crash3Save) {
    let changes = diff(&summarized(source), converted);
    assert_eq!(
        changes,
        [],
        "{name}: converted save differs from the source"
    );
}

/// A Switch playthrough save (19%) on PC, converted into the PC new-game
/// save. Expect the same progress, abilities, scenes and 61 lives.
fn switch_to_pc() -> Vec<u8> {
    let mut source = load("saves/0019.sav");
    lives(&mut source, 61);
    let converted = transfer(&source, &load("saves/ALCHEMYXSAVE_8.sav")).unwrap();
    let expect = Expect {
        platform: Platform::Pc,
        lives: 61,
        percent: 19,
        crystals: 10,
        gems: 6,
        relics: 1,
        bosses: 2,
    };
    let (bytes, back) = build("switch to PC", converted, &expect);
    assert_same_game_data("switch to PC", &source, &back);
    bytes
}

/// A 108% PC save on the Switch: everything completed on the PC new-game
/// save, converted into the Switch new-game save. 62 lives.
fn pc_to_switch() -> Vec<u8> {
    let mut source = load("saves/ALCHEMYXSAVE_8.sav");
    let edits = presets::complete_all(&source, &CompleteAll::default());
    apply(&mut source, &edits);
    lives(&mut source, 62);
    let converted = transfer(&source, &load("saves/initial_game.sav")).unwrap();
    let expect = Expect {
        platform: Platform::Switch,
        lives: 62,
        percent: 108,
        crystals: 25,
        gems: 47,
        relics: 31,
        bosses: 5,
    };
    let (bytes, back) = build("PC to Switch", converted, &expect);
    assert_same_game_data("PC to Switch", &source, &back);
    assert!(ABILITIES.iter().all(|&a| back.ability(a)));
    bytes
}

/// Under Pressure back to never played, on the 19% playthrough save: its
/// crystal and clear gem go (crystals 10 → 9, gems 6 → 5), and nothing else
/// in the game data changes. 63 lives.
fn reset_level() -> Vec<u8> {
    let source = load("saves/0019.sav");
    let up = level("underpressure");
    assert!(source.crystal(up) && source.gem(up) && source.entered(up));
    let mut save = source.clone();
    apply(&mut save, &[Edit::ResetLevel { level: up }]);
    lives(&mut save, 63);
    let expect = Expect {
        platform: Platform::Switch,
        lives: 63,
        percent: 17,
        crystals: 9,
        gems: 5,
        relics: 1,
        bosses: 2,
    };
    let (bytes, back) = build("reset level", save, &expect);
    assert!(!back.entered(up) && back.completions(up) == 0);
    assert!(back.times(up).is_empty() && back.relic(up).is_none());
    assert_eq!(back.flags(up), 0);
    // Only Under Pressure's session and the summary change.
    for change in diff(&summarized(&source), &back) {
        match change {
            Change::Zone { level, .. } if level == Some(up) => {}
            Change::Summary { .. } => {}
            other => panic!("reset level: unexpected change {other:?}"),
        }
    }
    bytes
}

/// The story up to warp room 3 on the Switch new-game save: rooms 1–3
/// completed (15 crystals), Tiny Tiger, Dingodile and N. Tropy defeated with
/// their abilities and without their story scenes. Warp rooms 1–4 open,
/// N. Gin's portal not there yet. 64 lives.
fn story_up_to_3() -> Vec<u8> {
    let mut save = load("saves/initial_game.sav");
    let options = StoryUpTo {
        with_gems: false,
        with_abilities: true,
        skip_cutscenes: true,
    };
    apply(&mut save, &presets::story_up_to(3, &options).unwrap());
    lives(&mut save, 64);
    let expect = Expect {
        platform: Platform::Switch,
        lives: 64,
        percent: 18,
        crystals: 15,
        gems: 0,
        relics: 0,
        bosses: 3,
    };
    let (bytes, back) = build("story up to room 3", save, &expect);
    for (i, &ability) in ABILITIES.iter().enumerate() {
        assert_eq!(back.ability(ability), i < 3, "{ability:?}");
    }
    for (i, &boss) in BOSSES.iter().enumerate() {
        assert_eq!(back.boss_defeated(boss), i < 3, "{}", boss.name());
    }
    for step in CUTSCENES
        .iter()
        .filter(|s| s.boss().is_some_and(|b| back.boss_defeated(b)))
    {
        for scene in step.scenes {
            assert!(back.cutscene_seen(scene), "{scene}");
        }
    }
    bytes
}

/// Gold on every level of the 19% playthrough save, with a time 1
/// centisecond under the gold threshold where the level had no time that
/// earns it. 31 relics open every level of the secret warp room. 65 lives.
fn relics_matching() -> Vec<u8> {
    let mut save = load("saves/0019.sav");
    let edits = presets::all_relics(&save, Relic::Gold, &AllRelics::default());
    apply(&mut save, &edits);
    lives(&mut save, 65);
    let expect = Expect {
        platform: Platform::Switch,
        lives: 65,
        percent: 19 - 1 + 31,
        crystals: 10,
        gems: 6,
        relics: 31,
        bosses: 2,
    };
    let (bytes, back) = build("relics matching", save, &expect);
    for level in relic_levels() {
        assert_eq!(back.relic(level), Some(Relic::Gold), "{}", level.name());
        let best = back.times(level)[0];
        assert!(
            relic_for_time(level, best) >= Some(Relic::Gold),
            "{}",
            level.name()
        );
    }
    bytes
}

/// Every gem but the bonus gem and gold on every level, except Future
/// Tense at sapphire: 30 gold relics and 107%. The game should still give
/// the bonus gem in the hub. 67 lives.
fn bonus_gem_30() -> Vec<u8> {
    let mut save = load("test_saves/t7_bonus_gem.sav");
    let t = save.totals();
    assert_eq!((t.gold, t.sapphire, t.platinum), (31, 0, 0));
    assert!(!save.bonus_gem());
    let futuretense = level("futuretense");
    apply(
        &mut save,
        &[Edit::SetRelic {
            level: futuretense,
            relic: Some(Relic::Sapphire),
        }],
    );
    lives(&mut save, 67);
    let expect = Expect {
        platform: Platform::Switch,
        lives: 67,
        percent: 107,
        crystals: 25,
        gems: 46,
        relics: 31,
        bosses: 5,
    };
    let (bytes, back) = build("bonus gem with 30 gold", save, &expect);
    let t = back.totals();
    assert_eq!((t.gold, t.sapphire, t.platinum), (30, 1, 0));
    assert_eq!(back.relic(futuretense), Some(Relic::Sapphire));
    assert!(!back.bonus_gem() && back.bonus_gem_due());
    bytes
}

/// The PC 108% save with a few levels one relic below the time to read:
/// a level's time panel shows the time of the next relic only. Deep
/// Trouble, Double Header and Sphynxinator get sapphire (their panels show
/// gold), Future Frenzy gets gold (its panel shows platinum), each with a
/// time 1 centisecond under its new relic's threshold. Sphynxinator, whose
/// gold time every source agrees on, is the control. 66 lives.
fn relic_panels_pc() -> Vec<u8> {
    let mut save = load("test_saves/pc/pc2_full108.sav");
    let below = [
        ("deeptrouble", Relic::Sapphire),
        ("doubleheader", Relic::Sapphire),
        ("sphynxinator", Relic::Sapphire),
        ("futurefrenzy", Relic::Gold),
    ];
    for (key, relic) in below {
        let level = level(key);
        let time = presets::matching_time(level, relic).unwrap();
        apply(
            &mut save,
            &[
                Edit::SetRelic {
                    level,
                    relic: Some(relic),
                },
                Edit::SetTimes {
                    level,
                    times_cs: vec![time],
                },
            ],
        );
    }
    lives(&mut save, 66);
    let expect = Expect {
        platform: Platform::Pc,
        lives: 66,
        percent: 108,
        crystals: 25,
        gems: 47,
        relics: 31,
        bosses: 5,
    };
    let (bytes, back) = build("relic panels", save, &expect);
    let t = back.totals();
    assert_eq!((t.platinum, t.gold, t.sapphire), (27, 1, 3));
    for (key, relic) in below {
        let level = level(key);
        assert_eq!(back.relic(level), Some(relic), "{key}");
        assert_eq!(
            relic_for_time(level, back.times(level)[0]),
            Some(relic),
            "{key}"
        );
    }
    bytes
}

/// The 19% playthrough save with 255 lives, the most the counter holds.
fn lives_255() -> Vec<u8> {
    let mut save = load("saves/0019.sav");
    lives(&mut save, 255);
    let expect = Expect {
        platform: Platform::Switch,
        lives: 255,
        percent: 19,
        crystals: 10,
        gems: 6,
        relics: 1,
        bosses: 2,
    };
    build("255 lives", save, &expect).0
}

fn main() {
    // Build and check everything before writing anything.
    let built = [
        ("l1_switch_to_pc.sav", switch_to_pc()),
        ("l2_pc_to_switch.sav", pc_to_switch()),
        ("l3_reset_level.sav", reset_level()),
        ("l4_story_up_to_3.sav", story_up_to_3()),
        ("l5_relics_matching.sav", relics_matching()),
        ("l6_relic_panels_pc.sav", relic_panels_pc()),
        ("l7_bonus_gem_30.sav", bonus_gem_30()),
        ("l8_lives_255.sav", lives_255()),
    ];
    let dir = repo().join("test_saves/round8");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in &built {
        write(&dir, name, bytes);
    }
}
