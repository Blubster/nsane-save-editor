use igb::IgbError;
use thiserror::Error;

/// Why a file can't be opened or written as a game save.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error(transparent)]
    Igb(#[from] IgbError),

    /// A valid IGB file that lacks something every game save has.
    #[error("not a Crash Bandicoot N. Sane Trilogy save: {0}")]
    NotASave(String),
}
