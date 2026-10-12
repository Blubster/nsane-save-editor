//! Crash Bandicoot 3: Warped: the overview, and in the middle the warp room
//! grid or a level's page.

mod bulk;
mod diagnostics_ui;
mod level_page;
mod level_panel;
pub mod level_row;
mod overview;
mod presets_ui;
mod preview;
mod review;
pub mod rooms;
mod scenes;
pub mod transfer_ui;

pub use diagnostics_ui::save_blocker;
pub use presets_ui::Preset;
pub use review::SaveReview;
pub use transfer_ui::{Convert, Request as ConvertRequest, Written};

use nsane::Document;
use nsane::crash3::LevelId;

use crate::app::{Action, Nav};
use crate::dialog;
use crate::theme::SPACING;

/// What the Crash 3 tab keeps between frames.
#[derive(Debug, Default)]
pub struct Crash3Ui {
    /// Lives being dragged or typed, applied when the user lets go.
    lives: Option<i32>,
    panel: level_panel::LevelPanel,
    bulk: bulk::BulkUi,
    scenes: scenes::Scenes,
    presets: presets_ui::Presets,
    diagnostics: diagnostics_ui::DiagnosticsPanel,
}

impl Crash3Ui {
    /// Drops values being edited and open questions, when the document
    /// changes under them (undo, redo, another file). The diagnostics panel
    /// stays.
    pub fn reset(&mut self) {
        *self = Self {
            diagnostics: std::mem::take(&mut self.diagnostics),
            ..Self::default()
        };
    }

    /// Drops what the level's page keeps (a time being typed, the reset
    /// question) when it closes.
    pub fn close_level(&mut self) {
        self.panel = level_panel::LevelPanel::default();
    }

    /// Whether a question of this tab is open: shortcuts wait meanwhile.
    pub fn has_dialog(&self) -> bool {
        self.panel.has_dialog()
            || self.bulk.has_dialog()
            || self.scenes.has_dialog()
            || self.presets.has_dialog()
    }

    /// The entries of the Presets menu.
    pub fn presets_menu(&mut self, ui: &mut egui::Ui, doc: &Document) {
        self.presets.menu(ui, doc);
    }

    /// The dialog of `preset` (`--view preset-108`, `--view preset-story3`).
    pub fn open_preset(&mut self, preset: Preset) {
        self.presets.open(preset);
    }

    pub fn open_diagnostics(&mut self) {
        self.diagnostics.open = true;
    }

    /// On a phone the diagnostics are a sheet over the page: it closes when
    /// a level is shown.
    pub fn close_diagnostics(&mut self) {
        self.diagnostics.open = false;
    }

    #[cfg(test)]
    pub fn diagnostics_open(&self) -> bool {
        self.diagnostics.open
    }

    /// How many previews the open preset dialog has computed.
    #[cfg(test)]
    pub fn preset_preview_runs(&self) -> Option<usize> {
        self.presets.preview_runs()
    }

    /// How wide the badges of [`Self::diagnostics_badges`] are.
    pub fn diagnostics_badges_width(&self, ui: &egui::Ui, doc: &Document) -> f32 {
        diagnostics_ui::badges_width(ui, doc.diagnostics())
    }

    /// A badge per diagnostic severity, for the toolbar; a click opens the
    /// diagnostics panel.
    pub fn diagnostics_badges(&mut self, ui: &mut egui::Ui, doc: &Document) {
        if diagnostics_ui::badges(ui, doc.diagnostics()) {
            self.diagnostics.open = true;
        }
    }

    /// The windows over the editor: the diagnostics panel, and the preset,
    /// bulk and story scenes dialogs. `shown` is the level whose page is in
    /// view.
    pub fn windows(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        shown: Option<LevelId>,
        nav: &mut Vec<Nav>,
        actions: &mut Vec<Action>,
    ) {
        self.diagnostics.show(ctx, doc, shown, nav, actions);
        self.presets.dialog(ctx, doc, actions);
        self.bulk.show(ctx, doc, actions);
        self.scenes.show(ctx, doc, actions);
    }

    /// The unlock dialog of the first locked warp room, or of room 5, as
    /// its padlock opens it (`--view unlock-dialog`).
    pub fn unlock_dialog(&mut self, doc: &Document) {
        let room = (2..=5)
            .find(|&room| !rooms::room_open(doc, room))
            .unwrap_or(5);
        self.bulk.open(bulk::Bulk::unlock(room));
    }

    /// How many previews the open bulk dialog has computed.
    #[cfg(test)]
    pub fn preview_runs(&self) -> Option<usize> {
        self.bulk.preview_runs()
    }

    /// The story scenes dialog (`--view cutscenes`).
    pub fn open_scenes(&mut self) {
        self.scenes.open = true;
    }

    pub fn overview(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        selected: Option<LevelId>,
        nav: &mut Vec<Nav>,
        actions: &mut Vec<Action>,
    ) {
        let totals = doc.current().totals();
        // On a phone, the overview is a page drawn where the rooms are.
        egui::ScrollArea::vertical()
            .id_salt("overview")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.heading("Overview");
                ui.add_space(SPACING / 2.0);
                overview::progress(ui, &totals);
                ui.add_space(SPACING);
                overview::lives(ui, doc, &mut self.lives, actions);
                dialog::section(ui, "Collected");
                overview::totals(ui, doc, &totals, actions);
                overview::bosses(ui, doc, &totals, selected, nav);
                overview::super_powers(ui, doc, actions);
                overview::story(ui, doc, &mut self.scenes.open);
                overview::diagnostics(ui, doc, &mut self.diagnostics.open);
            });
    }

    /// Shows `text` refused in a leaderboard field of `level`, as if it had
    /// been typed (`--view invalid-time`).
    pub fn refused_time(&mut self, doc: &Document, level: LevelId, slot: usize, text: &str) {
        self.panel.refused_time(doc, level, slot, text);
    }

    /// The warp room grid, or the page of the selected level.
    pub fn central(
        &mut self,
        ui: &mut egui::Ui,
        doc: &Document,
        selected: &mut Option<LevelId>,
        nav: &mut Vec<Nav>,
        actions: &mut Vec<Action>,
    ) {
        let mut open = None;
        if selected.is_some() {
            level_page::show(ui, doc, selected, &mut self.panel, nav, actions, &mut open);
        } else {
            rooms::grid(ui, doc, selected, actions, &mut open);
        }
        if let Some(bulk) = open {
            self.bulk.open(bulk);
        }
    }
}
