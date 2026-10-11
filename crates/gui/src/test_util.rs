//! Helpers shared by the tests.

use std::fs;
use std::path::{Path, PathBuf};

/// The test saves, including those rebuilt from their recipe.
#[path = "../../nsane/tests/support/corpus.rs"]
pub mod corpus;

/// A fresh, empty folder for one test.
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "nsane-save-editor-test-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A file of the repository, by its path from the root.
pub fn repo_file(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}
