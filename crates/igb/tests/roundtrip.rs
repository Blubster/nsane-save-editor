use std::path::{Path, PathBuf};

use igb::{FieldValue, IgbError, IgbFile, SummaryValue, handle_hash, latin1};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn collect_saves(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_saves(&path, out);
        } else if path.extension().is_some_and(|e| e == "sav") {
            out.push(path);
        }
    }
}

/// Every `.sav` file under `saves/` and `test_saves/`, as (path relative to the
/// repository, bytes).
fn all_saves() -> Vec<(String, Vec<u8>)> {
    let root = repo();
    let mut paths = Vec::new();
    collect_saves(&root.join("saves"), &mut paths);
    collect_saves(&root.join("test_saves"), &mut paths);
    paths
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            (rel, std::fs::read(&p).unwrap())
        })
        .collect()
}

fn load(rel: &str) -> IgbFile {
    IgbFile::parse(&std::fs::read(repo().join(rel)).unwrap()).unwrap()
}

#[test]
fn every_save_round_trips_byte_for_byte() {
    let saves = all_saves();
    assert_eq!(saves.len(), 69, "expected 69 save files");
    let mut failures = Vec::new();
    for (name, data) in &saves {
        match IgbFile::parse(data).and_then(|f| f.build()) {
            Ok(out) if out == *data => {}
            Ok(out) => {
                let first = out.iter().zip(data).position(|(a, b)| a != b);
                failures.push(format!(
                    "{name}: {} bytes written for {}, first difference at {first:?}",
                    out.len(),
                    data.len()
                ));
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn only_pc_saves_are_unpadded() {
    for (name, data) in all_saves() {
        let file = IgbFile::parse(&data).unwrap();
        let pc = name == "saves/ALCHEMYXSAVE_8.sav"
            || name.starts_with("test_saves/pc/")
            || name == "test_saves/round8/l1_switch_to_pc.sav"
            || name == "test_saves/round8/l1_after.sav"
            || name == "test_saves/round8/l6_relic_panels_pc.sav";
        assert_eq!(file.padded(), !pc, "{name}");
        assert_eq!(file.file_size(), data.len(), "{name}");
        if file.padded() {
            assert_eq!(data.len(), 153_600, "{name}");
        }
        let content = file.build_unpadded().unwrap();
        assert_eq!(content, data[..content.len()], "{name}");
    }
}

#[test]
fn every_save_has_the_same_object_layout() {
    for (name, data) in all_saves() {
        let file = IgbFile::parse(&data).unwrap();
        assert_eq!(file.objects().count(), 288, "{name}");
        assert_eq!(file.objects_of("CZoneInfoSave").count(), 116, "{name}");
        assert_eq!(file.version(), 0xB400_000C, "{name}");
        assert_eq!(file.string(0), None, "{name}");
    }
}

#[test]
fn initial_game_summary() {
    let file = load("saves/initial_game.sav");
    let expected = [
        ("save_version", SummaryValue::Int(75)),
        ("farthest_chapter", SummaryValue::Int(0)),
        ("farthest_progression_index", SummaryValue::Int(0)),
        ("previous_level_progression_index", SummaryValue::Int(-1)),
        ("credits_watched", SummaryValue::Int(0)),
        ("story_completed", SummaryValue::Int(0)),
        ("active_progression_index", SummaryValue::Int(0)),
        ("percent_complete", SummaryValue::Float(0.0)),
        ("life_count", SummaryValue::Int(4)),
        ("key_count", SummaryValue::Int(0)),
        ("gem_count", SummaryValue::Int(0)),
        ("sapphire_count", SummaryValue::Int(0)),
        ("gold_count", SummaryValue::Int(0)),
        ("platinum_count", SummaryValue::Int(0)),
        ("crystal_count", SummaryValue::Int(0)),
    ];
    let actual: Vec<_> = file
        .summary()
        .entries()
        .iter()
        .map(|e| (latin1::decode(&e.key), e.value.clone()))
        .collect();
    let expected: Vec<_> = expected
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(file.summary().get("previous_level_name"), None);
}

#[test]
fn handle_names_resolve_by_hash() {
    let file = load("saves/0019.sav");
    let hash = handle_hash("Upgrade_DoubleJump");
    assert!(file.handle_names().iter().any(|(h, _)| *h == hash));
    assert_eq!(
        file.handle_name((hash, 0)),
        Some(b"Upgrade_DoubleJump".as_slice())
    );
    // the hash ignores ASCII case
    assert_eq!(handle_hash("upgrade_doublejump"), hash);
    // every stored name hashes to its own key
    for (h, name) in file.handle_names() {
        assert_eq!(handle_hash(&latin1::decode(name)), *h);
    }
}

#[test]
fn fnv1a_reference_values() {
    assert_eq!(igb::fnv1a(b""), 0x811C_9DC5);
    assert_eq!(igb::fnv1a(b"a"), 0xE40C_292C);
    assert_eq!(igb::fnv1a(b"foobar"), 0xBF9C_F968);
}

#[test]
fn zone_list_dereferences_to_zone_objects() {
    let file = load("saves/initial_game.sav");
    let list = file.objects_of("CZoneInfoSaveList").next().unwrap();
    let ids = file.deref_list(list).unwrap();
    assert_eq!(ids.len(), 116);
    for id in ids {
        assert_eq!(file.object(id).unwrap().class, "CZoneInfoSave");
    }
    // a "no reference" block is an empty list
    let mut empty = list.clone();
    empty.fields[2] = FieldValue::MemRef(0xFFFF_FFFE);
    assert_eq!(file.deref_list(&empty).unwrap(), Vec::<u32>::new());
}

#[test]
fn summary_edits_are_written() {
    let mut file = load("saves/initial_game.sav");
    let s = file.summary_mut();
    s.set("life_count", SummaryValue::Int(42)).unwrap();
    assert_eq!(
        s.set("missing", SummaryValue::Int(1)),
        Err(IgbError::MissingKey("missing".into()))
    );
    assert_eq!(
        s.set("life_count", SummaryValue::Float(1.0)),
        Err(IgbError::TypeMismatch("life_count".into()))
    );
    s.insert_after(
        "previous_level_progression_index",
        "previous_level_name",
        SummaryValue::Str(b"crash3/hub".to_vec()),
    )
    .unwrap();
    assert!(matches!(
        s.insert_after("save_version", "life_count", SummaryValue::Int(0)),
        Err(IgbError::DuplicateKey(_))
    ));
    assert!(matches!(
        s.insert_after("missing", "new_key", SummaryValue::Int(0)),
        Err(IgbError::MissingKey(_))
    ));

    let out = file.build().unwrap();
    assert_eq!(out.len(), 153_600);
    let again = IgbFile::parse(&out).unwrap();
    assert_eq!(again.summary(), file.summary());
    let keys: Vec<_> = again
        .summary()
        .entries()
        .iter()
        .map(|e| latin1::decode(&e.key))
        .collect();
    assert_eq!(
        keys[3..5],
        ["previous_level_progression_index", "previous_level_name"]
    );
    assert_eq!(
        again.summary().get("life_count"),
        Some(&SummaryValue::Int(42))
    );
}

#[test]
fn switch_files_keep_their_size_and_pc_files_grow() {
    let big = SummaryValue::Str(vec![b'x'; 100_000]);

    let mut switch = load("saves/initial_game.sav");
    switch
        .summary_mut()
        .insert_after("crystal_count", "big", big.clone())
        .unwrap();
    let needed = switch.build_unpadded().unwrap().len();
    assert_eq!(
        switch.build(),
        Err(IgbError::DoesNotFit {
            needed,
            capacity: 153_600
        })
    );

    let mut pc = load("saves/ALCHEMYXSAVE_8.sav");
    let before = pc.file_size();
    pc.summary_mut()
        .insert_after("crystal_count", "big", big)
        .unwrap();
    let out = pc.build().unwrap();
    assert!(out.len() > before + 100_000);
    assert!(!IgbFile::parse(&out).unwrap().padded());
}

#[test]
fn memory_block_accessors() {
    let mut file = load("saves/0019.sav");
    let ids: Vec<_> = file.memory_blocks().map(|b| b.id).collect();
    for id in ids {
        let block = file.memory_mut(id).unwrap();
        assert_eq!(
            block.data().len(),
            block.count() * block.element_type.size()
        );
        let last = block.count().checked_sub(1);
        match block.element_type {
            igb::ElementType::Bool => {
                if let Some(i) = last {
                    let v = block.bool_at(i).unwrap();
                    block.set_bool(i, v).unwrap();
                }
                assert_eq!(block.bool_at(block.count()), None);
                assert_eq!(block.u32_at(0), None);
            }
            igb::ElementType::Name => {
                if let Some(i) = last {
                    let v = block.name_at(i).unwrap();
                    block.set_name(i, v).unwrap();
                }
                assert_eq!(block.name_at(block.count()), None);
                assert_eq!(block.set_name(block.count(), (0, 0)), None);
            }
            _ => {
                if let Some(i) = last {
                    let v = block.u32_at(i).unwrap();
                    block.set_u32(i, v).unwrap();
                }
                assert_eq!(block.u32_at(block.count()), None);
                assert_eq!(block.set_u32(block.count(), 0), None);
            }
        }
    }
    assert_eq!(
        file.build().unwrap(),
        std::fs::read(repo().join("saves/0019.sav")).unwrap()
    );
}

#[test]
fn bool_padding_and_float_bits_survive() {
    // the upper bytes of a bool field are whatever the game left there
    let file = load("saves/0019.sav");
    let fields: Vec<_> = file.objects().flat_map(|o| o.fields.iter()).collect();
    assert!(
        fields
            .iter()
            .any(|f| matches!(f, FieldValue::Bool { padding, .. } if *padding != 0))
    );
    assert!(fields.iter().any(|f| matches!(f, FieldValue::Float(_))));
    // signed fields keep their sign
    assert!(fields.iter().any(|f| matches!(f, FieldValue::Enum(-1))));
}

/// Small fixed-seed generator (xorshift64*), so no extra crate is needed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Parse (and build, when parsing succeeds) must return, never panic.
fn parse_and_build(data: &[u8]) {
    if let Ok(file) = IgbFile::parse(data) {
        let _ = file.build();
    }
}

#[test]
fn truncated_and_corrupted_files_never_panic() {
    let mut rng = Rng(0x5EED_1234_ABCD_0001);
    for (name, mut data) in all_saves() {
        let file = IgbFile::parse(&data).unwrap();
        // most offsets inside the content, some in the padding
        let content = file.build_unpadded().unwrap().len();
        for _ in 0..200 {
            let cut = rng.below(content);
            assert!(IgbFile::parse(&data[..cut]).is_err(), "{name} cut at {cut}");
        }
        for _ in 0..200 {
            let at = rng.below(data.len());
            let old = data[at];
            data[at] = old ^ (1 + rng.below(255) as u8);
            parse_and_build(&data);
            data[at] = old;
        }
    }
    // garbage that merely starts like a save
    let mut header = vec![0xFF; 64];
    header[40..44].copy_from_slice(&0xFADAu32.to_le_bytes());
    parse_and_build(&header);
    assert!(matches!(
        IgbFile::parse(&[0; 48]),
        Err(IgbError::BadMagic(0))
    ));
    assert!(IgbFile::parse(&[]).is_err());
}
