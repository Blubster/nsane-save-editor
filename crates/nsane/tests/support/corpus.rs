//! The test saves: the files under `tests/saves/`, and the editor-made saves
//! that are rebuilt from their recipe in `tests/golden/scenarios.json` instead
//! of being kept as files (`tests/saves/README.md`). Shared through `#[path]`
//! by the tests and examples of `nsane` and by the GUI tests.

#![allow(dead_code)] // each user takes what it needs

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use nsane::crash3::{Crash3Save, Edit};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A file of the repository, by its path from the root.
pub fn file(rel: &str) -> PathBuf {
    repo().join(rel)
}

/// The bytes of a file of the repository.
pub fn read(rel: &str) -> Vec<u8> {
    let path = file(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn read_json(rel: &str) -> Value {
    let path = file(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The scenarios of `tests/golden/scenarios.json`, in file order.
pub fn scenarios() -> &'static [Map<String, Value>] {
    static SCENARIOS: OnceLock<Vec<Map<String, Value>>> = OnceLock::new();
    SCENARIOS.get_or_init(|| {
        read_json("tests/golden/scenarios.json")["scenarios"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_object().unwrap().clone())
            .collect()
    })
}

/// The expected outputs of `tests/golden/manifest.json`, by scenario name.
pub fn manifest() -> &'static Map<String, Value> {
    static MANIFEST: OnceLock<Map<String, Value>> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        read_json("tests/golden/manifest.json")["scenarios"]
            .as_object()
            .unwrap()
            .clone()
    })
}

/// The bytes a scenario starts from: a file of the repository, or
/// `live/<name>`, the output of another recipe.
pub fn input(input: &str) -> Result<Vec<u8>, String> {
    match input.strip_prefix("live/") {
        Some(name) => live(name),
        None => std::fs::read(file(input)).map_err(|e| format!("{input}: {e}")),
    }
}

/// The output of the recipe `live/<name>`: its input, its edits applied, the
/// file written. Checked against the SHA-256 of `tests/golden/manifest.json`,
/// so a rebuilt save has the bytes of the file that was loaded in game.
pub fn live(name: &str) -> Result<Vec<u8>, String> {
    static BUILT: OnceLock<Mutex<HashMap<String, Vec<u8>>>> = OnceLock::new();
    let built = BUILT.get_or_init(Mutex::default);
    if let Some(bytes) = built.lock().unwrap().get(name) {
        return Ok(bytes.clone());
    }
    // Not under the lock: the input may be another recipe.
    let bytes = build_live(name)?;
    built.lock().unwrap().insert(name.to_owned(), bytes.clone());
    Ok(bytes)
}

fn build_live(name: &str) -> Result<Vec<u8>, String> {
    let key = format!("live/{name}");
    let scenario = scenarios()
        .iter()
        .find(|s| s["name"] == key.as_str())
        .ok_or_else(|| format!("no scenario {key}"))?;
    let from = scenario["input"].as_str().ok_or("input is not a string")?;
    let mut save = Crash3Save::parse(&input(from)?).map_err(|e| format!("{from}: {e}"))?;
    for op in scenario["ops"].as_array().ok_or("ops is not a list")? {
        let edit = Edit::from_json(op).map_err(|e| format!("{key}: {op}: {e}"))?;
        save.apply(&edit).map_err(|e| format!("{key}: {op}: {e}"))?;
    }
    let bytes = save.to_bytes().map_err(|e| format!("{key}: {e}"))?;
    let expected = manifest()
        .get(&key)
        .and_then(|e| e["sha256"].as_str())
        .ok_or_else(|| format!("{key}: no hash in the manifest"))?;
    let actual = sha256_hex(&bytes);
    if actual != expected {
        return Err(format!(
            "{key}: rebuilt with sha256 {actual}, the manifest has {expected}"
        ));
    }
    Ok(bytes)
}

/// One test save.
#[derive(Clone)]
pub struct Save {
    /// Its file name (`t4_crystals.sav`), also for a rebuilt save: the
    /// file-name checks read it.
    pub name: String,
    /// Where it comes from: its path from the repository root, or
    /// `live/<name>` for a rebuilt save.
    pub source: String,
    pub bytes: Vec<u8>,
}

fn paths_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            paths_under(&path, out);
        } else if path.extension().is_some_and(|e| e == "sav") {
            out.push(path);
        }
    }
}

/// The `.sav` files under a folder of the repository, recursively, sorted by
/// path.
pub fn files_in(dir: &str) -> Vec<Save> {
    let root = repo();
    let mut paths = Vec::new();
    paths_under(&root.join(dir), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|path| Save {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            source: path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/"),
            bytes: std::fs::read(&path).unwrap(),
        })
        .collect()
}

/// The recipes whose output isn't kept as a file: those with no
/// `expect_file`.
pub fn rebuilt_names() -> Vec<&'static str> {
    scenarios()
        .iter()
        .filter(|s| !s.contains_key("expect_file"))
        .filter_map(|s| s["name"].as_str()?.strip_prefix("live/"))
        .collect()
}

/// Every test save: the files under `tests/saves/`, then the rebuilt saves.
pub fn corpus() -> Vec<Save> {
    let mut saves = files_in("tests/saves");
    for name in rebuilt_names() {
        let bytes = live(name).unwrap_or_else(|e| panic!("{e}"));
        saves.push(Save {
            name: format!("{name}.sav"),
            source: format!("live/{name}"),
            bytes,
        });
    }
    saves
}

/// The bytes of a test save, by file name: a file under `tests/saves/`, or a
/// rebuilt save.
pub fn named(name: &str) -> Vec<u8> {
    let mut paths = Vec::new();
    paths_under(&file("tests/saves"), &mut paths);
    if let Some(path) = paths
        .iter()
        .find(|p| p.file_name().is_some_and(|n| n == name))
    {
        return std::fs::read(path).unwrap();
    }
    let recipe = name.strip_suffix(".sav").unwrap_or(name);
    assert!(rebuilt_names().contains(&recipe), "no test save {name}");
    live(recipe).unwrap_or_else(|e| panic!("{e}"))
}

/// The saves the game wrote during the playthrough, plus a new PC game: the
/// files of `tests/saves/game/`.
pub fn game() -> Vec<Save> {
    files_in("tests/saves/game")
}
