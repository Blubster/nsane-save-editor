//! Reading and writing files, per platform.

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
