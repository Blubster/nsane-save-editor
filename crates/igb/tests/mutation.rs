use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use igb::{ElementType, FieldValue, IgbError, IgbFile};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(rel: &str) -> IgbFile {
    IgbFile::parse(&std::fs::read(repo().join(rel)).unwrap()).unwrap()
}

const SAVES: [&str; 3] = [
    "saves/initial_game.sav",
    "saves/0019.sav",
    "saves/ALCHEMYXSAVE_8.sav",
];

/// What a string reference points to: the text, or the raw value when it is 0
/// or not a valid index.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    Text(Vec<u8>),
    Raw(u32),
}

fn target(file: &IgbFile, idx: u32) -> Target {
    match file.string(idx) {
        Some(s) if idx != 0 => Target::Text(s.to_vec()),
        _ => Target::Raw(idx),
    }
}

/// Every string reference, keyed by where it is: object fields, string blocks
/// and the string half of name blocks (with the hash alongside).
fn references(file: &IgbFile) -> BTreeMap<(&'static str, u32, usize), (Target, u32)> {
    let mut out = BTreeMap::new();
    for object in file.objects() {
        for (i, field) in object.fields.iter().enumerate() {
            if let FieldValue::Str(idx) = *field {
                out.insert(("field", object.id, i), (target(file, idx), 0));
            }
        }
    }
    for block in file.memory_blocks() {
        for i in 0..block.count() {
            match block.element_type {
                ElementType::Str => {
                    let idx = block.u32_at(i).unwrap();
                    out.insert(("str", block.id, i), (target(file, idx), 0));
                }
                ElementType::Name => {
                    let (idx, hash) = block.name_at(i).unwrap();
                    out.insert(("name", block.id, i), (target(file, idx), hash));
                }
                _ => {}
            }
        }
    }
    out
}

fn assert_sorted(file: &IgbFile) {
    let strings = file.strings();
    assert_eq!(strings[0], None);
    let rest: Vec<&[u8]> = strings[1..].iter().map(|s| s.as_deref().unwrap()).collect();
    assert!(rest.is_sorted(), "string table is not byte-sorted");
}

fn check_insert(rel: &str, text: &[u8]) -> u32 {
    let mut file = load(rel);
    let before = references(&file);
    let len = file.strings().len();
    let expected = 1 + file.strings()[1..]
        .iter()
        .filter(|s| s.as_deref().unwrap() < text)
        .count();

    let idx = file.string_index(text).unwrap();
    assert_eq!(idx as usize, expected, "{rel}: index of {text:?}");
    assert_eq!(file.strings().len(), len + 1);
    assert_eq!(file.string(idx), Some(text));
    assert_sorted(&file);
    assert_eq!(
        references(&file),
        before,
        "{rel}: a reference changed target"
    );
    // a second lookup finds it without inserting
    assert_eq!(file.string_index(text).unwrap(), idx);
    assert_eq!(file.strings().len(), len + 1);
    // and the file still reads back the same
    let reparsed = IgbFile::parse(&file.build().unwrap()).unwrap();
    assert_eq!(references(&reparsed), before);
    idx
}

#[test]
fn the_saves_hold_every_kind_of_string_reference() {
    let file = load("saves/0019.sav");
    let refs = references(&file);
    for kind in ["field", "str", "name"] {
        let texts = refs
            .iter()
            .filter(|((k, ..), (t, _))| *k == kind && matches!(t, Target::Text(_)))
            .count();
        assert!(texts > 0, "no {kind} reference to renumber");
    }
}

#[test]
fn inserting_a_string_that_sorts_first() {
    // "" and "---" (initials) sort before letters, so "!" lands after "" only
    assert_eq!(check_insert("saves/initial_game.sav", b"!first"), 1);
    assert_eq!(check_insert("saves/0019.sav", b"!first"), 2);
    assert_eq!(check_insert("saves/ALCHEMYXSAVE_8.sav", b"!first"), 1);
}

#[test]
fn inserting_a_string_that_sorts_last() {
    for rel in SAVES {
        let len = load(rel).strings().len() as u32;
        assert_eq!(check_insert(rel, b"~last"), len);
    }
}

#[test]
fn inserting_a_string_in_the_middle() {
    for rel in SAVES {
        let idx = check_insert(rel, b"HintDisplay_5x");
        assert!(idx > 2 && (idx as usize) < load(rel).strings().len());
    }
    // latin-1 above 0x7F sorts after ASCII
    check_insert("saves/0019.sav", b"Caf\xe9_\xff");
}

#[test]
fn an_existing_string_is_found_not_inserted() {
    let mut file = load("saves/0019.sav");
    let before = file.clone();
    assert_eq!(file.string_index(b"").unwrap(), 1);
    assert_eq!(file.string_index(b"---").unwrap(), 2);
    assert_eq!(file, before);
}

#[test]
fn a_string_with_a_nul_is_rejected() {
    let mut file = load("saves/0019.sav");
    let before = file.clone();
    assert_eq!(file.string_index(b"a\0b"), Err(IgbError::InvalidString));
    assert_eq!(file, before);
}

fn flag_table(file: &IgbFile) -> u32 {
    file.objects_of("igStringBoolHashTable").next().unwrap().id
}

fn int_table(file: &IgbFile) -> u32 {
    file.objects_of("igStringIntHashTable").next().unwrap().id
}

/// Every key is found where probing looks for it, and the count is right.
fn assert_valid(file: &IgbFile, id: u32) -> BTreeMap<Vec<u8>, u32> {
    let t = file.hash_table(id).unwrap();
    let items = t.items().unwrap();
    assert_eq!(t.len(), items.len());
    for (key, value) in &items {
        assert_eq!(t.get(key), Some(*value), "{key:?} not reachable");
    }
    items.into_iter().collect()
}

#[test]
fn the_tables_of_every_save_are_valid() {
    for rel in SAVES {
        let file = load(rel);
        assert_valid(&file, flag_table(&file));
        assert_valid(&file, int_table(&file));
    }
}

#[test]
fn remove_then_insert_the_same_key() {
    let mut file = load("saves/0019.sav");
    let id = flag_table(&file);
    let before = assert_valid(&file, id);
    assert_eq!(before.get(b"HintDisplay_7".as_slice()), Some(&1));

    let mut t = file.hash_table_mut(id).unwrap();
    assert!(t.remove(b"HintDisplay_7").unwrap());
    assert!(!t.contains(b"HintDisplay_7"));
    assert!(!t.remove(b"HintDisplay_7").unwrap());
    let mut without = before.clone();
    without.remove(b"HintDisplay_7".as_slice());
    assert_eq!(assert_valid(&file, id), without);

    file.hash_table_mut(id)
        .unwrap()
        .set(b"HintDisplay_7", 1)
        .unwrap();
    assert_eq!(assert_valid(&file, id), before);
    let reparsed = IgbFile::parse(&file.build().unwrap()).unwrap();
    assert_eq!(assert_valid(&reparsed, id), before);
}

#[test]
fn setting_an_existing_key_only_changes_its_value() {
    let mut file = load("saves/0019.sav");
    let id = int_table(&file);
    let (key, _) = file.hash_table(id).unwrap().items().unwrap()[0].clone();
    file.hash_table_mut(id).unwrap().set(&key, 1234).unwrap();
    let t = file.hash_table(id).unwrap();
    assert_eq!(t.get(&key), Some(1234));
    assert_eq!(t.len(), 19);
}

#[test]
fn an_int_table_rebuild_fills_empty_values_with_0xfa() {
    let mut file = load("saves/0019.sav");
    let id = int_table(&file);
    let items = file.hash_table(id).unwrap().items().unwrap();
    assert_eq!(items.len(), 19);
    for capacity in [64, 128] {
        file.hash_table_mut(id)
            .unwrap()
            .rebuild(capacity, &items)
            .unwrap();
        let t = file.hash_table(id).unwrap();
        assert_eq!(t.capacity(), capacity);
        let values = match file.object(id).unwrap().fields[0] {
            FieldValue::MemRef(m) => file.memory(m).unwrap(),
            _ => unreachable!(),
        };
        let keys = match file.object(id).unwrap().fields[1] {
            FieldValue::MemRef(m) => file.memory(m).unwrap(),
            _ => unreachable!(),
        };
        let mut empty = 0;
        for slot in 0..capacity {
            if keys.u32_at(slot).unwrap() == 0 {
                assert_eq!(values.u32_at(slot), Some(0xFAFA_FAFA));
                empty += 1;
            }
        }
        assert_eq!(empty, capacity - 19);
        assert_eq!(
            assert_valid(&file, id),
            items.iter().cloned().collect::<BTreeMap<_, _>>()
        );
    }
}

#[test]
fn growth_changes_the_directory_and_the_header() {
    let mut file = load("saves/initial_game.sav");
    let id = flag_table(&file);
    let entries = file.directory().len();
    let mut t = file.hash_table_mut(id).unwrap();
    for i in 0..32 {
        t.set(format!("Key_{i:03}").as_bytes(), 1).unwrap();
    }
    assert_eq!(t.capacity(), 64, "32 keys fit at load factor 0.5");
    t.set(b"Key_032", 1).unwrap();
    assert_eq!(t.capacity(), 128);
    assert_eq!(assert_valid(&file, id).len(), 33);

    // The two 128-element blocks get entries of their own. The 64-element bool
    // entry is dropped, while the 64-element string entry stays: the int
    // table's keys still use it.
    assert_eq!(file.directory().len(), entries + 1);
    let out = file.build().unwrap();
    let word = |i: usize| u32::from_le_bytes(out[i * 4..i * 4 + 4].try_into().unwrap());
    assert_eq!(word(1) as usize, entries + 1);
    assert_eq!(
        word(7),
        load("saves/initial_game.sav").memory_blocks().count() as u32
    );
    let reparsed = IgbFile::parse(&out).unwrap();
    assert_eq!(reparsed.directory(), file.directory());
}

#[test]
fn resize_memory_reuses_an_identical_entry_and_drops_unused_ones() {
    let mut file = load("saves/initial_game.sav");
    let flags = flag_table(&file);
    let ints = int_table(&file);
    let keys_of = |file: &IgbFile, id| match file.object(id).unwrap().fields[1] {
        FieldValue::MemRef(m) => m,
        _ => unreachable!(),
    };
    let (a, b) = (keys_of(&file, flags), keys_of(&file, ints));
    let entries = file.directory().len();
    assert_eq!(file.index()[a as usize], file.index()[b as usize]);

    file.resize_memory(a, 3, 0x11).unwrap();
    assert_eq!(file.directory().len(), entries + 1);
    assert_eq!(file.memory(a).unwrap().count(), 3);
    assert_eq!(file.memory(a).unwrap().data().len(), 12);

    file.resize_memory(a, 5, 0x11).unwrap();
    assert_eq!(
        file.directory().len(),
        entries + 1,
        "the 3-element entry is dropped"
    );
    assert_eq!(&file.memory(a).unwrap().data()[12..], &[0x11; 8]);

    file.resize_memory(a, 64, 0).unwrap();
    assert_eq!(file.directory().len(), entries);
    assert_eq!(file.index()[a as usize], file.index()[b as usize]);
    IgbFile::parse(&file.build().unwrap()).unwrap();
}

#[test]
fn a_full_table_gives_an_error() {
    let mut file = load("saves/initial_game.sav");
    let id = flag_table(&file);
    // a load factor above 1 lets the table fill up instead of growing
    *file.object_mut(id).unwrap().field_mut(4).unwrap() = FieldValue::Float(2.0f32.to_bits());
    let mut t = file.hash_table_mut(id).unwrap();
    for i in 0..64 {
        t.set(format!("Key_{i:03}").as_bytes(), 1).unwrap();
    }
    assert_eq!(t.set(b"Key_064", 1), Err(IgbError::HashTableFull));
    assert_eq!(t.get(b"Key_064"), None);
    assert_eq!(t.remove(b"Key_064"), Err(IgbError::HashTableFull));
    assert!(t.remove(b"Key_000").unwrap());

    let items: Vec<_> = (0..65).map(|i| (vec![b'k', i + 1], 1)).collect();
    assert_eq!(t.rebuild(64, &items), Err(IgbError::HashTableFull));
}

#[test]
fn bad_inputs_give_errors() {
    let mut file = load("saves/initial_game.sav");
    let id = flag_table(&file);
    let mut t = file.hash_table_mut(id).unwrap();
    assert_eq!(t.set(b"Flag", 256), Err(IgbError::ValueOutOfRange(256)));
    assert_eq!(t.set(b"a\0b", 1), Err(IgbError::InvalidString));
    let dup = [(b"A".to_vec(), 1), (b"A".to_vec(), 1)];
    assert!(t.rebuild(64, &dup).is_err());
    // not a hash table
    let other = file
        .objects()
        .find(|o| !o.class.contains("HashTable"))
        .unwrap()
        .id;
    assert!(file.hash_table(other).is_err());
    assert!(file.hash_table_mut(other).is_err());
}
