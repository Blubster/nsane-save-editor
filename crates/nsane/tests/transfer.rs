//! Switch ↔ PC conversion: the game data moves into a save of the other
//! platform and comes back unchanged (`docs/FORMAT.md` §1).

use std::path::{Path, PathBuf};

use nsane::Platform;
use nsane::crash3::{ABILITIES, CUTSCENES, Crash3Save, LevelId};
use nsane::diff::diff;
use nsane::igb::{FieldValue, IgObject, SummaryValue};
use nsane::transfer::{
    Role, SAVE_VERSION, TransferError, bundled_template, bundled_template_save, transfer,
};

const SWITCH_SIZE: usize = 153_600;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> Vec<u8> {
    let path = repo().join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn load(rel: &str) -> Crash3Save {
    Crash3Save::parse(&read(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn saves(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            saves(&path, out);
        } else if path.extension().is_some_and(|e| e == "sav") {
            out.push(path);
        }
    }
}

fn all_saves() -> Vec<PathBuf> {
    let mut out = Vec::new();
    saves(&repo().join("saves"), &mut out);
    saves(&repo().join("test_saves"), &mut out);
    out.sort();
    assert_eq!(out.len(), 69);
    out
}

fn template(platform: Platform) -> Crash3Save {
    bundled_template_save(platform).unwrap()
}

/// What writing `save` would store: its summary recomputed.
fn summarized(save: &Crash3Save) -> Crash3Save {
    let mut save = save.clone();
    save.update_summary().unwrap();
    save
}

/// Converts, writes and reads back, as a user's file would go.
fn convert(source: &Crash3Save, platform: Platform) -> (Vec<u8>, Crash3Save) {
    let bytes = transfer(source, &template(platform))
        .unwrap()
        .to_bytes()
        .unwrap();
    let save = Crash3Save::parse(&bytes).unwrap();
    assert_eq!(Platform::of(&save), platform);
    (bytes, save)
}

fn other(platform: Platform) -> Platform {
    match platform {
        Platform::Switch => Platform::Pc,
        Platform::Pc => Platform::Switch,
    }
}

/// A field with a bool's padding bytes cleared.
fn value(field: &FieldValue) -> FieldValue {
    match *field {
        FieldValue::Bool { value, .. } => FieldValue::Bool { value, padding: 0 },
        other => other,
    }
}

fn values(object: &IgObject) -> Vec<FieldValue> {
    object.fields.iter().map(value).collect()
}

fn bool_padding(object: &IgObject) -> Vec<u32> {
    object
        .fields
        .iter()
        .filter_map(|f| match f {
            FieldValue::Bool { padding, .. } => Some(*padding),
            _ => None,
        })
        .collect()
}

/// The game data outside what [`diff`] compares: the save data singletons
/// (without the difficulty vector, which holds a platform value) and the
/// bools of the two hash table objects.
fn other_data(save: &Crash3Save) -> Vec<FieldValue> {
    let igb = save.igb();
    let object = |class| igb.objects_of(class).next().unwrap();
    let mut out = values(object("CGameSaveData"));
    out.extend(values(object("CDynamicDifficultySaveData")).split_off(1));
    for table in ["igStringBoolHashTable", "igStringIntHashTable"] {
        out.push(value(&object(table).fields[3]));
    }
    out
}

#[test]
fn bundled_templates_are_the_new_game_saves() {
    assert_eq!(
        bundled_template(Platform::Switch),
        read("saves/initial_game.sav")
    );
    assert_eq!(
        bundled_template(Platform::Pc),
        read("saves/ALCHEMYXSAVE_8.sav")
    );
    assert_eq!(Platform::of(&template(Platform::Switch)), Platform::Switch);
    assert_eq!(Platform::of(&template(Platform::Pc)), Platform::Pc);
}

#[test]
fn pc_and_switch_hold_the_same_game_data() {
    let (pc, switch) = (
        load("saves/ALCHEMYXSAVE_8.sav"),
        load("saves/initial_game.sav"),
    );
    assert_eq!(pc.igb().strings(), switch.igb().strings());
    assert_eq!(pc.igb().summary(), switch.igb().summary());

    // Every session holds the same values; the bool padding is another
    // matter, and is why the conversion never copies it.
    let (pc_zones, switch_zones): (Vec<_>, Vec<_>) =
        (pc.zones().collect(), switch.zones().collect());
    assert_eq!(pc_zones.len(), 116);
    let mut padding_differs = 0;
    for ((pc_path, a), (switch_path, b)) in pc_zones.iter().zip(&switch_zones) {
        assert_eq!(pc_path, switch_path);
        assert_eq!(values(a), values(b), "{pc_path}");
        padding_differs += usize::from(bool_padding(a) != bool_padding(b));
    }
    assert_eq!(padding_differs, 84);

    let pc_vars: Vec<_> = pc.variables().map(|(n, v)| (n, value(&v))).collect();
    let switch_vars: Vec<_> = switch.variables().map(|(n, v)| (n, value(&v))).collect();
    assert_eq!(pc_vars, switch_vars);
    let bools = pc
        .variables()
        .zip(switch.variables())
        .filter(|((_, a), _)| matches!(a, FieldValue::Bool { .. }));
    let (count, differs) = bools.fold((0, 0), |(n, d), ((_, a), (_, b))| {
        (n + 1, d + usize::from(a != b))
    });
    assert_eq!((count, differs), (30, 21));

    assert!(diff(&pc, &switch).is_empty());
}

#[test]
fn new_game_converts_to_the_other_new_game_byte_for_byte() {
    for (from, to) in [
        ("saves/initial_game.sav", "saves/ALCHEMYXSAVE_8.sav"),
        ("saves/ALCHEMYXSAVE_8.sav", "saves/initial_game.sav"),
    ] {
        let mut out = transfer(&load(from), &load(to)).unwrap();
        assert!(out.to_bytes().unwrap() == read(to), "{from} → {to}");
    }
}

#[test]
fn every_save_survives_a_round_trip() {
    for path in all_saves() {
        let name = path.strip_prefix(repo()).unwrap().display().to_string();
        let source = load(&name);
        let home = Platform::of(&source);
        let expected = summarized(&source);

        let (_, there) = convert(&source, other(home));
        let changes = diff(&expected, &there);
        assert!(changes.is_empty(), "{name} → {}: {changes:?}", other(home));

        assert_eq!(other_data(&there), other_data(&source), "{name}");

        let (bytes, back) = convert(&there, home);
        let changes = diff(&expected, &back);
        assert!(changes.is_empty(), "{name} and back: {changes:?}");
        assert_eq!(other_data(&back), other_data(&source), "{name}");
        if home == Platform::Switch {
            assert_eq!(bytes.len(), SWITCH_SIZE, "{name}");
        }
    }
}

#[test]
fn output_takes_the_target_size_rules() {
    let full = load("test_saves/t6_after.sav");
    let (bytes, _) = convert(&full, Platform::Pc);
    let template = bundled_template(Platform::Pc);
    assert!(bytes.len() > template.len(), "{}", bytes.len());
    // unpadded: the content ends right where the file does
    let reparsed = Crash3Save::parse(&bytes).unwrap();
    assert_eq!(reparsed.igb().build_unpadded().unwrap().len(), bytes.len());

    let (bytes, _) = convert(&load("test_saves/pc/pc2_full108.sav"), Platform::Switch);
    assert_eq!(bytes.len(), SWITCH_SIZE);
}

#[test]
fn converted_save_shows_the_same_progress() {
    for (rel, platform) in [
        ("saves/0019.sav", Platform::Pc),
        ("test_saves/t6_after.sav", Platform::Pc),
    ] {
        let source = load(rel);
        let (_, out) = convert(&source, platform);
        assert_eq!(out.totals(), source.totals(), "{rel}");
        assert_eq!(out.percent(), source.percent(), "{rel}");
        assert_eq!(out.lives(), source.lives(), "{rel}");
        assert_eq!(out.bonus_gem(), source.bonus_gem(), "{rel}");
        for ability in ABILITIES {
            assert_eq!(out.ability(ability), source.ability(ability), "{rel}");
        }
        for &scene in CUTSCENES.iter().flat_map(|s| s.scenes) {
            assert_eq!(
                out.cutscene_seen(scene),
                source.cutscene_seen(scene),
                "{rel} {scene}"
            );
        }
        for level in LevelId::all() {
            let view = |s: &Crash3Save| {
                (
                    s.entered(level),
                    s.completions(level),
                    s.flags(level),
                    s.crystal_field(level),
                    s.relic(level),
                    s.times(level),
                    s.gem_count(level),
                    s.boss_defeated(level),
                )
            };
            assert_eq!(view(&out), view(&source), "{rel} {}", level.name());
        }
        assert_eq!(out.flag_entries(), source.flag_entries(), "{rel}");
        assert_eq!(out.int_flag_entries(), source.int_flag_entries(), "{rel}");
        assert_eq!(
            out.previous_level_name(),
            source.previous_level_name(),
            "{rel}"
        );
    }
}

#[test]
fn a_same_platform_template_changes_nothing() {
    let source = load("saves/0019.sav");
    let out = transfer(&source, &load("saves/0005.sav")).unwrap();
    assert!(diff(&source, &out).is_empty());
}

#[test]
fn a_template_missing_a_zone_is_refused() {
    let template = load("saves/ALCHEMYXSAVE_8.sav");
    let last = template.zones().last().unwrap().0.to_owned();
    // drop the last entry of the zone list
    let mut igb = template.into_igb();
    let list = igb.objects_of("CZoneInfoSaveList").next().unwrap().id;
    let count = igb.object_mut(list).unwrap().field_mut(0).unwrap();
    assert_eq!(*count, FieldValue::Int(116));
    *count = FieldValue::Int(115);
    let template = Crash3Save::from_igb(igb).unwrap();

    let err = transfer(&load("saves/0019.sav"), &template).unwrap_err();
    assert_eq!(err, TransferError::MissingZone(last));
}

fn with_version(save: Crash3Save, version: i32) -> Crash3Save {
    let mut igb = save.into_igb();
    igb.summary_mut()
        .set("save_version", SummaryValue::Int(version))
        .unwrap();
    Crash3Save::from_igb(igb).unwrap()
}

#[test]
fn another_save_version_is_refused() {
    let (source, template) = (load("saves/0019.sav"), template(Platform::Pc));
    assert_eq!(SAVE_VERSION, 75);

    let err = transfer(&with_version(source.clone(), 76), &template).unwrap_err();
    assert_eq!(
        err,
        TransferError::Version {
            role: Role::Source,
            found: Some(76)
        }
    );
    let err = transfer(&source, &with_version(template, 74)).unwrap_err();
    assert_eq!(
        err,
        TransferError::Version {
            role: Role::Template,
            found: Some(74)
        }
    );
}

#[test]
fn a_non_empty_quest_list_is_refused() {
    let mut igb = load("saves/0019.sav").into_igb();
    let list = igb.objects_of("CQuestTrackSaveDataList").next().unwrap().id;
    *igb.object_mut(list).unwrap().field_mut(0).unwrap() = FieldValue::Int(1);
    let source = Crash3Save::from_igb(igb).unwrap();

    let err = transfer(&source, &template(Platform::Pc)).unwrap_err();
    assert!(
        matches!(&err, TransferError::Unsupported(what) if what.contains("CQuestTrackSaveDataList")),
        "{err}"
    );
}
