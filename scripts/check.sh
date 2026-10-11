#!/usr/bin/env bash
# Formatting, lints, tests, the wasm build of the core crates and the lints
# of the app built for the browser.
# Stops at the first failure.
set -euo pipefail
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p igb -p nsane --target wasm32-unknown-unknown
# the app in the browser: no desktop-only code outside its cfg gates
cargo clippy -p gui --target wasm32-unknown-unknown -- -D warnings
