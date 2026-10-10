#!/usr/bin/env bash
# Formatting, lints, tests and the wasm build of the core crates.
# Stops at the first failure.
set -euo pipefail
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
cd "$(dirname "$0")/.."

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p igb -p nsane --target wasm32-unknown-unknown
