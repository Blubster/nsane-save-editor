//! Crash Bandicoot 3: Warped: the overview, the central area and the level
//! details.

mod overview;

use nsane::Document;
use nsane::crash3::LevelId;

use crate::app::Action;

/// What the Crash 3 tab keeps between frames.
#[derive(Debug, Default)]
pub struct Crash3Ui {
    /// Lives being dragged or typed, applied when the user lets go.
    lives: Option<i32>,
}

impl Crash3Ui {
    /// Drops values being edited, when the document changes under them
    /// (undo, redo, another file).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn overview(&mut self, ui: &mut egui::Ui, doc: &Document, actions: &mut Vec<Action>) {
        ui.heading("Overview");
        ui.add_space(crate::theme::SPACING);
        overview::lives(ui, doc, &mut self.lives, actions);
    }

    pub fn central(&mut self, ui: &mut egui::Ui, _doc: &Document) {
        ui.centered_and_justified(|ui| {
            ui.weak("Warp rooms");
        });
    }

    pub fn details(&mut self, ui: &mut egui::Ui, _doc: &Document, selected: Option<LevelId>) {
        ui.heading("Details");
        ui.add_space(crate::theme::SPACING);
        match selected {
            Some(level) => {
                ui.label(egui::RichText::new(level.name()).strong());
            }
            None => {
                ui.weak("Select a level to see its details.");
            }
        }
    }
}
