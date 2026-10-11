//! Game layer for Crash Bandicoot N. Sane Trilogy saves, on top of the
//! [`igb`] container. One module per game; Crash 3: Warped comes first.
//!
//! [`Document`] holds an open save with its undo history, and [`diff`]
//! compares two saves in game terms. [`transfer`] converts a save between
//! the Switch and PC layouts, and [`slot`] names the files of the save slots.

mod backup;
pub mod crash3;
pub mod diff;
mod document;
mod error;
pub mod slot;
pub mod transfer;

pub use backup::backup_path;
pub use document::{
    Document, OpenError, Platform, Preview, Review, SaveError, SizeReport, UNDO_DEPTH,
};
pub use error::Error;
pub use igb;
