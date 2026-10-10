//! Container-level golden scenarios: flag keys set and removed directly on the
//! one-shot flag table, the file built without touching the summary. The bytes
//! must match `tests/golden/manifest.json`.

use std::path::{Path, PathBuf};

use igb::{IgbError, IgbFile, latin1};
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

/// A raw flag op, checked as the edit op contract says: exactly the `op` and
/// `key` fields, and a non-empty key of characters U+0001–U+00FF (one latin-1
/// byte each, NUL-terminated in the file).
enum RawOp {
    Set(Vec<u8>),
    Remove(Vec<u8>),
}

fn parse_op(op: &Value) -> Result<RawOp, String> {
    let invalid = || "invalid_value".to_owned();
    let fields = op.as_object().ok_or_else(invalid)?;
    let name = fields
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    if !matches!(name, "raw_flag_set" | "raw_flag_remove") {
        panic!("container scenarios only use raw flag ops, not {name}");
    }
    if fields.len() != 2 {
        return Err(invalid());
    }
    let key = fields
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(invalid)?;
    let key = latin1::encode(key)
        .filter(|k| !k.is_empty() && !k.contains(&0))
        .ok_or_else(invalid)?;
    Ok(match name {
        "raw_flag_set" => RawOp::Set(key),
        _ => RawOp::Remove(key),
    })
}

/// The output bytes, or the error code of the first failing step.
fn run(scenario: &Map<String, Value>) -> Result<Result<Vec<u8>, String>, String> {
    let input = scenario["input"].as_str().ok_or("input is not a string")?;
    let path = repo().join(input);
    let data =
        std::fs::read(&path).map_err(|e| format!("missing input {}: {e}", path.display()))?;
    let mut file = IgbFile::parse(&data).map_err(|e| format!("{input}: {e}"))?;
    let table = file
        .objects_of("igStringBoolHashTable")
        .next()
        .ok_or("no igStringBoolHashTable")?
        .id;
    for op in scenario["ops"].as_array().ok_or("ops is not a list")? {
        let op = match parse_op(op) {
            Ok(op) => op,
            Err(code) => return Ok(Err(code)),
        };
        let mut t = file.hash_table_mut(table).map_err(|e| e.to_string())?;
        match op {
            RawOp::Set(key) => t.set(&key, 1),
            RawOp::Remove(key) => t.remove(&key).map(|_| ()),
        }
        .map_err(|e| format!("op failed: {e}"))?;
    }
    match file.build() {
        Ok(out) => Ok(Ok(out)),
        Err(IgbError::DoesNotFit { .. }) => Ok(Err("does_not_fit".to_owned())),
        Err(e) => Err(format!("build failed: {e}")),
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
fn container_scenarios_match_the_manifest() {
    let scenarios = read_json("tests/golden/scenarios.json");
    let manifest = read_json("tests/golden/manifest.json");
    let expected_all = manifest["scenarios"].as_object().unwrap();

    let mut ran = 0;
    let mut failures = Vec::new();
    for scenario in scenarios["scenarios"].as_array().unwrap() {
        let scenario = scenario.as_object().unwrap();
        if scenario
            .get("layer")
            .and_then(Value::as_str)
            .unwrap_or("crash3")
            != "igb"
        {
            continue;
        }
        ran += 1;
        let name = scenario["name"].as_str().unwrap();
        let Some(expected) = expected_all.get(name) else {
            failures.push(format!("{name}: not in the manifest"));
            continue;
        };
        match run(scenario) {
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
    assert!(ran > 0, "no container scenarios found");
    assert!(
        failures.is_empty(),
        "{} of {ran} container scenarios failed:\n{}\n\n\
         To compare with the expected bytes, write the expected outputs with the \
         write mode of the generator of tests/golden/manifest.json, set \
         GOLDEN_EXPECTED_DIR to that folder and run again; then compare both \
         files with a save dump.",
        failures.len(),
        failures.join("\n"),
    );
    eprintln!("{ran} container scenarios passed");
}
