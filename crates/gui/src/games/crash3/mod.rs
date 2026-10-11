//! Crash Bandicoot 3: Warped: the overview, the central area and the level
//! details.

mod level_panel;
pub mod level_row;
mod overview;
mod rooms;

use nsane::Document;
use nsane::crash3::LevelId;

use crate::app::Action;
use crate::theme::SPACING;

/// What the Crash 3 tab keeps between frames.
#[derive(Debug, Default)]
pub struct Crash3Ui {
    /// Lives being dragged or typed, applied when the user lets go.
    lives: Option<i32>,
    panel: level_panel::LevelPanel,
}

impl Crash3Ui {
    /// Drops values being edited, when the document changes under them
    /// (undo, redo, another file).
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn overview(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        selected: &mut Option<LevelId>,
        actions: &mut Vec<Action>,
    ) {
        let totals = doc.current().totals();
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.heading("Overview");
                ui.add_space(SPACING);
                overview::progress(ui, &totals);
                ui.add_space(SPACING);
                overview::totals(ui, doc, &totals);
                ui.separator();
                overview::lives(ui, doc, &mut self.lives, actions);
                overview::bonus_gem(ui, doc, actions);
                ui.separator();
                overview::secret_room(ui, &totals, selected);
                ui.separator();
                overview::story(ui, doc);
                ui.separator();
                overview::diagnostics(ui, doc);
            });
    }

    /// Shows `text` refused in a leaderboard field of `level`, as if it had
    /// been typed (`--view invalid-time`).
    pub fn refused_time(&mut self, doc: &Document, level: LevelId, slot: usize, text: &str) {
        self.panel.refused_time(doc, level, slot, text);
    }

    pub fn central(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        selected: &mut Option<LevelId>,
        actions: &mut Vec<Action>,
    ) {
        rooms::grid(ui, doc, selected, actions);
    }

    pub fn details(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        selected: Option<LevelId>,
        actions: &mut Vec<Action>,
    ) {
        ui.heading("Details");
        ui.add_space(SPACING);
        self.panel.show(ui, doc, selected, actions);
    }
}
