//! Builds the saves of a round of in-game tests into `test_saves/round9/`:
//! where the game stops adding lives. At 254 lives, picking up a life adds
//! nothing (`docs/FORMAT.md` §6), so the maximum is lower. Each file is the
//! 19% playthrough save with another life count, around the values a cap is
//! likely to be: two digits, a signed byte, round numbers, one below 254.
//!
//!     cargo run -p nsane --example make_live_round9
//!
//! Every file is read back and checked before it is written. A file that
//! already exists is never replaced: once loaded in game it is test
//! evidence, so a run that would write other bytes stops instead.

use std::path::{Path, PathBuf};

use nsane::Platform;
use nsane::crash3::{Crash3Save, Edit};
use nsane::diff::{Change, diff};

const LIVES: [u8; 7] = [98, 99, 100, 127, 128, 200, 253];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn build(source: &Crash3Save, lives: u8) -> Vec<u8> {
    let mut save = source.clone();
    save.apply(&Edit::SetLives { count: lives }).unwrap();
    let bytes = save.to_bytes().unwrap();
    let back = Crash3Save::parse(&bytes).unwrap();
    assert_eq!(Platform::of(&back), Platform::Switch);
    assert_eq!(back.lives(), i32::from(lives));
    assert_eq!(back.totals(), source.totals());
    // Nothing but the life count differs from the source.
    let mut summarized = source.clone();
    summarized.update_summary().unwrap();
    for change in diff(&summarized, &back) {
        match change {
            Change::Summary { ref key, .. } if key == "life_count" => {}
            other => panic!("{lives} lives: unexpected change {other:?}"),
        }
    }
    bytes
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

fn main() {
    let path = repo().join("saves/0019.sav");
    let source = Crash3Save::parse(&std::fs::read(&path).unwrap()).unwrap();
    // Build and check everything before writing anything.
    let built: Vec<_> = LIVES
        .iter()
        .map(|&lives| (format!("r9_lives_{lives}.sav"), build(&source, lives)))
        .collect();
    let dir = repo().join("test_saves/round9");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, bytes) in &built {
        write(&dir, name, bytes);
    }
}
