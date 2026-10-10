use thiserror::Error;

/// Everything that can go wrong while reading or writing a save.
///
/// A malformed file is an error, never a panic: users open arbitrary files.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IgbError {
    #[error("unexpected end of file: {needed} bytes needed at offset {offset:#x}")]
    Truncated { offset: usize, needed: usize },

    #[error("not an IGB save (bad magic {0:#x})")]
    BadMagic(u32),

    #[error("{section}: size is {actual:#x}, the header says {expected:#x}")]
    SectionSize {
        section: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("{section}: {count} objects found, the header says {expected}")]
    ObjectCount {
        section: &'static str,
        expected: usize,
        count: usize,
    },

    #[error("unexpected data after the memory section at offset {0:#x}")]
    TrailingData(usize),

    #[error("{what} index {index} is out of range")]
    BadIndex { what: &'static str, index: usize },

    #[error("unsupported field type {field_type} in class {class}")]
    UnsupportedFieldType { class: String, field_type: String },

    #[error("unsupported memory element type {0}")]
    UnsupportedElementType(String),

    #[error("no class named {0}")]
    UnknownClass(String),

    #[error("malformed {0}")]
    Malformed(&'static str),

    #[error("summary key {0} not found")]
    MissingKey(String),

    #[error("summary key {0} already exists")]
    DuplicateKey(String),

    #[error("summary key {0} holds a value of another type")]
    TypeMismatch(String),

    #[error("the save data needs {needed} bytes but the file holds {capacity}")]
    DoesNotFit { needed: usize, capacity: usize },
}
