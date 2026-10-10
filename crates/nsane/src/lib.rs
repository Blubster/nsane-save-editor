//! Game layer for Crash Bandicoot N. Sane Trilogy saves, on top of the
//! [`igb`] container. One module per game; Crash 3: Warped comes first.

pub mod crash3;
mod error;

pub use error::Error;
pub use igb;
