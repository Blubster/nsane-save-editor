//! Game-level golden scenarios: edits applied to a save, the summary
//! recomputed, the file built. The bytes, or the error code, must match
//! `tests/golden/manifest.json`.

use std::path::{Path, PathBuf};

use nsane::Error;
use nsane::crash3::{Crash3Save, Edit, EditError};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_json(rel: &str) -> Value {
    let path = repo().join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The game-level scenarios (a missing `layer` means game level).
fn scenarios() -> Vec<Map<String, Value>> {
    let doc = read_json("tests/golden/scenarios.json");
    doc["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_object().unwrap().clone())
        .filter(|s| s.get("layer").and_then(Value::as_str).unwrap_or("crash3") == "crash3")
        .collect()
}

fn ops(scenario: &Map<String, Value>) -> &[Value] {
    scenario["ops"].as_array().unwrap()
}

fn error_code(e: Error) -> Result<String, String> {
    match e {
        Error::Igb(e) => Ok(EditError::from(e).code().to_owned()),
        e => Err(e.to_string()),
    }
}

/// The output bytes, or the error code of the first failing step.
fn run(scenario: &Map<String, Value>) -> Result<Result<Vec<u8>, String>, String> {
    let input = scenario["input"].as_str().ok_or("input is not a string")?;
    let path = repo().join(input);
    let data =
        std::fs::read(&path).map_err(|e| format!("missing input {}: {e}", path.display()))?;
    let mut save = Crash3Save::parse(&data).map_err(|e| format!("{input}: {e}"))?;
    for op in ops(scenario) {
        let applied = Edit::from_json(op).and_then(|edit| save.apply(&edit));
        if let Err(e) = applied {
            return Ok(Err(e.code().to_owned()));
        }
    }
    match save.to_bytes() {
        Ok(out) => Ok(Ok(out)),
        Err(e) => error_code(e).map(Err),
    }
}

fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| repo().join("target"), PathBuf::from)
}

fn file_name(scenario: &str) -> String {
    if scenario.ends_with(".sav") {
        scenario.to_owned()
    } else {
        format!("{scenario}.sav")
    }
}

/// Writes the output next to the build files and, when `GOLDEN_EXPECTED_DIR`
/// holds the expected outputs, says where the two first differ.
fn report_mismatch(name: &str, out: &[u8]) -> String {
    let rel = file_name(name);
    let actual = target_dir().join("golden-actual").join(&rel);
    if let Some(dir) = actual.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    std::fs::write(&actual, out).unwrap();
    let mut msg = format!("  output written to {}", actual.display());
    if let Some(dir) = std::env::var_os("GOLDEN_EXPECTED_DIR") {
        let expected = PathBuf::from(dir).join(&rel);
        match std::fs::read(&expected) {
            Ok(exp) => {
                let first = out.iter().zip(&exp).position(|(a, b)| a != b);
                msg += &format!(
                    "\n  expected {} ({} bytes, output {} bytes), first difference at {}",
                    expected.display(),
                    exp.len(),
                    out.len(),
                    first.map_or("the end of the shorter file".to_owned(), |i| format!(
                        "{i:#x}"
                    )),
                );
            }
            Err(e) => msg += &format!("\n  {}: {e}", expected.display()),
        }
    }
    msg
}

#[test]
fn game_scenarios_match_the_manifest() {
    let manifest = read_json("tests/golden/manifest.json");
    let expected_all = manifest["scenarios"].as_object().unwrap();

    let mut ran = 0;
    let mut failures = Vec::new();
    for scenario in scenarios() {
        ran += 1;
        let name = scenario["name"].as_str().unwrap();
        let Some(expected) = expected_all.get(name) else {
            failures.push(format!("{name}: not in the manifest"));
            continue;
        };
        match run(&scenario) {
            Err(e) => failures.push(format!("{name}: {e}")),
            Ok(Err(code)) => {
                if expected.get("error").and_then(Value::as_str) != Some(code.as_str()) {
                    failures.push(format!("{name}: error {code}, expected {expected}"));
                }
            }
            Ok(Ok(out)) => {
                let hash = sha256_hex(&out);
                if expected.get("sha256").and_then(Value::as_str) != Some(hash.as_str())
                    || expected.get("size").and_then(Value::as_u64) != Some(out.len() as u64)
                {
                    failures.push(format!(
                        "{name}: {} bytes, sha256 {hash}; expected {expected}\n{}",
                        out.len(),
                        report_mismatch(name, &out)
                    ));
                }
            }
        }
    }
    assert!(ran > 0, "no game scenarios found");
    assert!(
        failures.is_empty(),
        "{} of {ran} game scenarios failed:\n{}\n\n\
         To compare with the expected bytes, write the expected outputs with the \
         write mode of the generator of tests/golden/manifest.json, set \
         GOLDEN_EXPECTED_DIR to that folder and run again; then compare both \
         files with a save dump.",
        failures.len(),
        failures.join("\n"),
    );
    eprintln!("{ran} game scenarios passed");
}

/// The scenarios whose output equals a file tested in game are among those
/// that pass above; check they are all there.
#[test]
fn scenarios_cover_the_files_tested_in_game() {
    let with_file = scenarios()
        .iter()
        .filter(|s| s["name"].as_str().unwrap().starts_with("live/"))
        .filter(|s| s.contains_key("expect_file"))
        .count();
    assert_eq!(with_file, 22);
}

/// Every op of a valid scenario reads back to the same JSON once serialized.
#[test]
fn edits_round_trip_through_json() {
    let mut checked = 0;
    for scenario in scenarios()
        .iter()
        .filter(|s| !s.contains_key("expect_error"))
    {
        let name = scenario["name"].as_str().unwrap();
        for op in ops(scenario) {
            let edit = Edit::from_json(op).unwrap_or_else(|e| panic!("{name}: {op}: {e}"));
            assert_eq!(&serde_json::to_value(&edit).unwrap(), op, "{name}");
            assert_eq!(serde_json::from_value::<Edit>(op.clone()).unwrap(), edit);
            checked += 1;
        }
    }
    assert!(checked > 1000, "{checked} ops checked");
}

/// In an error scenario, every op before the last is valid and the last one
/// fails with the expected code, from reading it or from applying it (or, for
/// a save that no longer fits, from building the file).
#[test]
fn error_scenarios_fail_on_their_last_op() {
    let mut checked = 0;
    for scenario in scenarios() {
        let Some(code) = scenario.get("expect_error").and_then(Value::as_str) else {
            continue;
        };
        let name = scenario["name"].as_str().unwrap();
        let input = repo().join(scenario["input"].as_str().unwrap());
        let mut save = Crash3Save::parse(&std::fs::read(input).unwrap()).unwrap();
        let (last, before) = ops(&scenario).split_last().unwrap();
        for op in before {
            save.apply(&Edit::from_json(op).unwrap()).unwrap();
        }
        let result = Edit::from_json(last).and_then(|edit| save.apply(&edit));
        let actual = match result {
            Err(e) => e.code().to_owned(),
            Ok(()) => error_code(save.to_bytes().unwrap_err()).unwrap(),
        };
        assert_eq!(actual, code, "{name}");
        checked += 1;
    }
    assert!(checked >= 30, "{checked} error scenarios");
}
