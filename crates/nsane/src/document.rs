//! The open save, as the GUI sees it: every change goes through
//! [`Document::apply`], which keeps a snapshot per step for undo and redo.

use std::collections::{BTreeSet, VecDeque};
use std::fmt;

use igb::IgbError;
use thiserror::Error;

use crate::Error;
use crate::crash3::{Crash3Save, Edit, EditError, Totals, Warning};
use crate::diff::{Change, ReviewRow, diff, summarize};

/// How many steps undo keeps. The oldest are dropped first.
pub const UNDO_DEPTH: usize = 200;

/// The two layouts of a save (`docs/FORMAT.md` §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Zero-padded to a fixed file size.
    Switch,
    /// Exactly the content, no padding.
    Pc,
}

impl Platform {
    pub fn name(self) -> &'static str {
        match self {
            Platform::Switch => "Switch",
            Platform::Pc => "PC",
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// How much of the file the save data takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeReport {
    /// Bytes of content, without the padding.
    pub used: usize,
    /// The fixed file size on the Switch; `None` on PC, where the file grows.
    pub capacity: Option<usize>,
}

impl SizeReport {
    pub fn fits(&self) -> bool {
        self.capacity.is_none_or(|c| self.used <= c)
    }
}

/// Why a file can't be opened.
pub type OpenError = Error;

/// Why the document can't be written.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SaveError {
    /// A Switch save's data outgrew its fixed file size.
    #[error("the save data needs {needed} bytes but the file holds {capacity}")]
    DoesNotFit { needed: usize, capacity: usize },

    #[error(transparent)]
    Save(Error),
}

impl From<Error> for SaveError {
    fn from(e: Error) -> Self {
        match e {
            Error::Igb(IgbError::DoesNotFit { needed, capacity }) => {
                SaveError::DoesNotFit { needed, capacity }
            }
            e => SaveError::Save(e),
        }
    }
}

/// What the save review shows: the changes as rows, and the summary values
/// that writing the file will recompute.
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub rows: Vec<ReviewRow>,
    /// Only [`Change::Summary`] entries, for keys no edit changed:
    /// "Crystals 10 → 11", "Percentage 19% → 20%".
    pub recomputed: Vec<Change>,
}

/// The outcome of edits that are not applied (yet).
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    /// From the document as it is now to the result.
    pub changes: Vec<Change>,
    pub rows: Vec<ReviewRow>,
    /// The summary values the edits change once the file is written.
    pub recomputed: Vec<Change>,
    pub totals: Totals,
    pub warnings: Vec<Warning>,
    pub size: SizeReport,
    /// The resulting save, for checks that need more than the above.
    pub save: Crash3Save,
}

/// One undo or redo step: the state on the other side of it.
#[derive(Debug, Clone)]
struct Step {
    label: String,
    state: Crash3Save,
    generation: u64,
    size: SizeReport,
}

/// An open save with its edit history.
#[derive(Debug, Clone)]
pub struct Document {
    pub file_name: Option<String>,
    /// As loaded, for the diff and the review.
    original: Crash3Save,
    current: Crash3Save,
    undo: VecDeque<Step>,
    redo: Vec<Step>,
    /// Identifies the current state. Each apply gets a new number, and undo
    /// and redo bring back the number of the state they restore, so a state
    /// reached again counts as saved if it was saved.
    generation: u64,
    saved_generation: u64,
    last_generation: u64,
    size: SizeReport,
}

/// A copy of `save` with its summary recomputed, as it will be written.
fn summarized(save: &Crash3Save) -> Result<Crash3Save, Error> {
    let mut save = save.clone();
    save.update_summary()?;
    Ok(save)
}

fn measure(save: &Crash3Save) -> Result<SizeReport, Error> {
    let used = summarized(save)?.igb().build_unpadded()?.len();
    let igb = save.igb();
    Ok(SizeReport {
        used,
        capacity: igb.padded().then(|| igb.file_size()),
    })
}

/// [`measure`] for a state reached by edits.
fn measure_edited(save: &Crash3Save) -> Result<SizeReport, EditError> {
    measure(save).map_err(|e| match e {
        Error::Igb(e) => EditError::from(e),
        Error::NotASave(_) => EditError::Container(IgbError::Malformed("summary")),
    })
}

/// The summary changes from `before` to `after` once both are written,
/// leaving out the keys in `edited`, which the review already lists.
fn recomputed(before: &Crash3Save, after: &Crash3Save, edited: &[Change]) -> Vec<Change> {
    let edited: BTreeSet<&str> = edited
        .iter()
        .filter_map(|c| match c {
            Change::Summary { key, .. } => Some(key.as_str()),
            _ => None,
        })
        .collect();
    // `update_summary` only fails on a summary `from_igb` would have refused.
    let Ok(after) = summarized(after) else {
        return Vec::new();
    };
    diff(before, &after)
        .into_iter()
        .filter(|c| matches!(c, Change::Summary { key, .. } if !edited.contains(key.as_str())))
        .collect()
}

impl Document {
    pub fn open(bytes: &[u8], file_name: Option<String>) -> Result<Self, OpenError> {
        let save = Crash3Save::parse(bytes)?;
        let size = measure(&save)?;
        Ok(Self {
            file_name,
            original: save.clone(),
            current: save,
            undo: VecDeque::new(),
            redo: Vec::new(),
            generation: 0,
            saved_generation: 0,
            last_generation: 0,
            size,
        })
    }

    pub fn current(&self) -> &Crash3Save {
        &self.current
    }

    /// The save as it was opened.
    pub fn original(&self) -> &Crash3Save {
        &self.original
    }

    /// Applies `edits` in order as one undo step, or none of them: on error
    /// the document is left as it was.
    pub fn apply(&mut self, label: impl Into<String>, edits: Vec<Edit>) -> Result<(), EditError> {
        let mut next = self.current.clone();
        next.apply_all(&edits)?;
        // A state that can't be written is refused here rather than at save
        // time. A Switch file that outgrows its size is not refused: the size
        // report shows it, and `to_bytes` fails until edits are undone.
        let size = measure_edited(&next)?;

        self.last_generation += 1;
        let state = std::mem::replace(&mut self.current, next);
        let generation = std::mem::replace(&mut self.generation, self.last_generation);
        let size = std::mem::replace(&mut self.size, size);
        self.undo.push_back(Step {
            label: label.into(),
            state,
            generation,
            size,
        });
        if self.undo.len() > UNDO_DEPTH {
            self.undo.pop_front();
        }
        self.redo.clear();
        Ok(())
    }

    /// The result of `edits` on the document as it is now, without changing
    /// it. Clones the save: call it when a dialog's options change, not on
    /// every frame.
    pub fn preview(&self, edits: &[Edit]) -> Result<Preview, EditError> {
        let mut save = self.current.clone();
        save.apply_all(edits)?;
        let size = measure_edited(&save)?;
        let changes = diff(&self.current, &save);
        let rows = summarize(&changes, &save);
        let before = summarized(&self.current).unwrap_or_else(|_| self.current.clone());
        let recomputed = recomputed(&before, &save, &changes);
        Ok(Preview {
            rows,
            recomputed,
            totals: save.totals(),
            warnings: save.warnings(),
            size,
            changes,
            save,
        })
    }

    /// Swaps the current state with the one stored in `step`.
    fn swap(&mut self, step: &mut Step) {
        std::mem::swap(&mut self.current, &mut step.state);
        std::mem::swap(&mut self.generation, &mut step.generation);
        std::mem::swap(&mut self.size, &mut step.size);
    }

    pub fn undo(&mut self) -> bool {
        let Some(mut step) = self.undo.pop_back() else {
            return false;
        };
        self.swap(&mut step);
        self.redo.push(step);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(mut step) = self.redo.pop() else {
            return false;
        };
        self.swap(&mut step);
        self.undo.push_back(step);
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The labels of the steps undo can take back, oldest first.
    pub fn history(&self) -> impl Iterator<Item = &str> {
        self.undo.iter().map(|s| s.label.as_str())
    }

    /// Whether the current state differs from the last one saved (or opened).
    pub fn is_dirty(&self) -> bool {
        self.generation != self.saved_generation
    }

    pub fn platform(&self) -> Platform {
        if self.current.igb().padded() {
            Platform::Switch
        } else {
            Platform::Pc
        }
    }

    /// The size the file will have, measured after each change.
    pub fn size_report(&self) -> SizeReport {
        self.size
    }

    /// The file to write: the summary is recomputed on a copy, so writing
    /// doesn't add an undo step.
    pub fn to_bytes(&self) -> Result<Vec<u8>, SaveError> {
        Ok(self.current.clone().to_bytes()?)
    }

    /// Records that the current state was written, under a new name if given.
    pub fn mark_saved(&mut self, new_file_name: Option<String>) {
        self.saved_generation = self.generation;
        if new_file_name.is_some() {
            self.file_name = new_file_name;
        }
    }

    /// Every difference from the save as opened, per field. The summary
    /// totals are not recomputed here: see [`review`](Self::review).
    pub fn changes(&self) -> Vec<Change> {
        diff(&self.original, &self.current)
    }

    /// What the save review shows, against the save as opened.
    pub fn review(&self) -> Review {
        let changes = self.changes();
        Review {
            rows: summarize(&changes, &self.current),
            recomputed: recomputed(&self.original, &self.current, &changes),
        }
    }
}
