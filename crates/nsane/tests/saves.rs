//! The test saves themselves (`tests/saves/README.md`): the files kept under
//! `tests/saves/`, and the saves rebuilt from their recipe, which must have
//! the bytes of the files that were loaded in game.

use std::collections::BTreeSet;

use nsane::igb::IgbFile;

#[path = "support/corpus.rs"]
mod corpus;

#[test]
fn every_save_is_kept_or_rebuilt() {
    let count = |dir| corpus::files_in(dir).len();
    assert_eq!(count("tests/saves/game"), 21);
    assert_eq!(count("tests/saves/edited"), 13);
    assert_eq!(count("tests/saves/after_edit"), 17);
    assert_eq!(count("tests/saves"), 51);
    assert_eq!(corpus::rebuilt_names().len(), 24);
    // A save is kept or rebuilt, never both.
    let saves = corpus::corpus();
    let names: BTreeSet<_> = saves.iter().map(|s| &s.name).collect();
    assert_eq!(names.len(), saves.len());
}

/// What the container tests check on every file, here on the rebuilt saves.
#[test]
fn rebuilt_saves_round_trip_byte_for_byte() {
    for name in corpus::rebuilt_names() {
        let bytes = corpus::live(name).unwrap();
        let file = IgbFile::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(file.build().unwrap() == bytes, "{name}");
        assert_eq!(file.file_size(), bytes.len(), "{name}");
        assert_eq!(file.objects().count(), 288, "{name}");
    }
}

/// The README lists every save deleted from `tests/saves/`, with the SHA-256
/// of the file that was loaded in game. What replaces it has those bytes.
#[test]
fn deleted_saves_are_rebuilt_byte_for_byte() {
    let readme = std::fs::read_to_string(corpus::file("tests/saves/README.md")).unwrap();
    let section = readme
        .split("\n## Rebuilt saves\n")
        .nth(1)
        .expect("a Rebuilt saves section");
    let rows: Vec<Vec<&str>> = section
        .lines()
        .filter(|line| line.starts_with("| `"))
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().trim_matches('`'))
                .collect()
        })
        .collect();
    assert_eq!(rows.len(), 25);
    let kept: Vec<String> = corpus::files_in("tests/saves")
        .into_iter()
        .map(|s| s.name)
        .collect();
    let mut rebuilt = BTreeSet::new();
    for row in &rows {
        let &[name, _, from, sha256] = &row[..] else {
            panic!("{row:?}")
        };
        assert!(!kept.iter().any(|k| k == name), "{name} is still a file");
        let bytes = match from.strip_prefix("live/") {
            Some(recipe) => {
                assert_eq!(format!("{recipe}.sav"), name);
                rebuilt.insert(recipe);
                corpus::live(recipe).unwrap()
            }
            // a duplicate of a kept file
            None => corpus::read(&format!("tests/saves/{from}")),
        };
        assert_eq!(corpus::sha256_hex(&bytes), sha256, "{name}");
    }
    let all: BTreeSet<_> = corpus::rebuilt_names().into_iter().collect();
    assert_eq!(rebuilt, all);
}
