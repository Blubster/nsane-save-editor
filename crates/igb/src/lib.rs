//! Reader and writer for the Alchemy IGB container that holds the
//! Crash Bandicoot N. Sane Trilogy saves (Switch and PC).
//!
//! This crate knows the container only: objects, memory blocks, the string
//! table and the string-keyed hash tables. It has no knowledge of the games.
//!
//! Edits that change the layout are handled here: [`IgbFile::string_index`]
//! inserts a string and renumbers its references, [`IgbFile::resize_memory`]
//! keeps the directory in step, and [`IgbFile::hash_table_mut`] sets and
//! removes keys, growing the table as the game does.
//!
//! [`IgbFile::parse`] followed by [`IgbFile::build`] gives back the input byte
//! for byte. Strings are latin-1 bytes; [`latin1`] converts them at the API
//! boundary.

mod error;
mod file;
mod hashtable;
mod memory;
mod object;
mod reader;
mod strings;
mod summary;

pub use error::IgbError;
pub use file::{ClassDef, DirEntry, IgbFile};
pub use hashtable::{HashTable, HashTableMut};
pub use memory::{ElementType, MemoryBlock};
pub use object::{FieldValue, IgObject};
pub use summary::{Summary, SummaryEntry, SummaryValue};

/// Raw 32-bit FNV-1a.
pub fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811C_9DC5, |h, &b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

/// Hash of a handle name: FNV-1a of the name in ASCII lowercase.
pub fn handle_hash(name: &str) -> u32 {
    fnv1a(name.to_ascii_lowercase().as_bytes())
}

/// Latin-1 conversion: each byte is one `char` (U+0000–U+00FF).
pub mod latin1 {
    pub fn decode(bytes: &[u8]) -> String {
        bytes.iter().map(|&b| char::from(b)).collect()
    }

    /// `None` if a character is above U+00FF.
    pub fn encode(text: &str) -> Option<Vec<u8>> {
        text.chars().map(|c| u8::try_from(c).ok()).collect()
    }
}
