//! Crash Bandicoot 3: Warped. The format is described in `docs/FORMAT.md`.

pub mod diagnostics;
mod edit;
mod model;
pub mod presets;
pub mod tables;
pub mod time;

pub use diagnostics::{Diagnostic, Fix, Severity, diagnostics};
pub use edit::{Edit, EditError, MAX_TIME_CS, UNLOCKABLE_ROOMS};
pub use model::{Crash3Save, GemBreakdown, Totals, Warning};
pub use tables::{
    ABILITIES, Ability, BOSSES, CUTSCENES, GemColor, Kind, LEVELS, Level, LevelId, PROGRESSION,
    RELIC_TIMES, Relic, SECRET_ROOM,
};
pub use time::{ParseTimeError, format_time, parse_time, relic_for_time};

/// Whether [`Edit::ResetLevel`] was confirmed in game: a reset level must
/// show as never played. While false, the GUI says it is untested.
/// Confirmed in round 8 of the live tests (`docs/FORMAT.md` §10).
pub const RESET_LEVEL_VERIFIED: bool = true;
