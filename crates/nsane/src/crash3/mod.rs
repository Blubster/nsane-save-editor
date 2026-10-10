//! Crash Bandicoot 3: Warped. The format is described in `docs/FORMAT.md`.

mod edit;
mod model;
pub mod tables;
pub mod time;

pub use edit::{Edit, EditError, MAX_TIME_CS, UNLOCKABLE_ROOMS};
pub use model::{Crash3Save, GemBreakdown, Totals, Warning};
pub use tables::{
    ABILITIES, Ability, BOSSES, CUTSCENES, GemColor, Kind, LEVELS, Level, LevelId, PROGRESSION,
    RELIC_TIMES, Relic, SECRET_ROOM,
};
pub use time::{ParseTimeError, format_time, parse_time, relic_for_time};
