//! Crash Bandicoot 3: Warped: the overview, the central area (the warp room
//! grid, or the story progress) and the level details.

mod diagnostics_ui;
mod level_panel;
pub mod level_row;
mod overview;
mod presets_ui;
mod preview;
mod progress;
mod review;
mod rooms;
pub mod transfer_ui;

pub use diagnostics_ui::save_blocker;
pub use presets_ui::Preset;
pub use review::SaveReview;
pub use transfer_ui::{Convert, Request as ConvertRequest, Written};

use nsane::Document;
use nsane::crash3::LevelId;

use crate::app::Action;
use crate::dialog;
use crate::theme::SPACING;

/// What the central area shows.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The warp room grid.
    #[default]
    Levels,
    /// Warp rooms, the secret room, super powers and story scenes.
    Progress,
}

impl Tab {
    const ALL: [Tab; 2] = [Tab::Levels, Tab::Progress];

    fn title(self) -> &'static str {
        match self {
            Tab::Levels => "Levels",
            Tab::Progress => "Progress",
        }
    }

    fn tip(self) -> &'static str {
        match self {
            Tab::Levels => "Every level and boss, room by room",
            Tab::Progress => "Unlock warp rooms, the secret room, super powers and story scenes",
        }
    }
}

/// What the Crash 3 tab keeps between frames.
#[derive(Debug, Default)]
pub struct Crash3Ui {
    /// Lives being dragged or typed, applied when the user lets go.
    lives: Option<i32>,
    panel: level_panel::LevelPanel,
    tab: Tab,
    progress: progress::Progress,
    presets: presets_ui::Presets,
    diagnostics: diagnostics_ui::DiagnosticsPanel,
}

impl Crash3Ui {
    /// Drops values being edited and open questions, when the document
    /// changes under them (undo, redo, another file). The tab and the
    /// diagnostics panel stay.
    pub fn reset(&mut self) {
        *self = Self {
            tab: self.tab,
            diagnostics: std::mem::take(&mut self.diagnostics),
            ..Self::default()
        };
    }

    /// Whether a question of this tab is open: shortcuts wait meanwhile.
    pub fn has_dialog(&self) -> bool {
        self.panel.has_dialog() || self.progress.has_dialog() || self.presets.has_dialog()
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

    #[cfg(test)]
    pub fn diagnostics_open(&self) -> bool {
        self.diagnostics.open
    }

    /// How many previews the open preset dialog has computed.
    #[cfg(test)]
    pub fn preset_preview_runs(&self) -> Option<usize> {
        self.presets.preview_runs()
    }

    /// A badge per diagnostic severity, for the toolbar; a click opens the
    /// diagnostics panel.
    pub fn diagnostics_badges(&mut self, ui: &mut egui::Ui, doc: &Document) {
        if diagnostics_ui::badges(ui, doc.diagnostics()) {
            self.diagnostics.open = true;
        }
    }

    /// The windows over the editor: the preset dialog and the diagnostics
    /// panel.
    pub fn windows(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        selected: &mut Option<LevelId>,
        actions: &mut Vec<Action>,
    ) {
        self.diagnostics.show(ctx, doc, selected, actions);
        self.presets.dialog(ctx, doc, actions);
    }

    /// Shows the Progress tab (`--view progress`).
    pub fn show_progress(&mut self) {
        self.tab = Tab::Progress;
    }

    /// The Progress tab with the unlock dialog of the first locked warp
    /// room, or of room 5 (`--view unlock-dialog`).
    pub fn unlock_dialog(&mut self, doc: &Document) {
        self.tab = Tab::Progress;
        let room = (2..=5)
            .find(|&room| !rooms::room_open(doc, room))
            .unwrap_or(5);
        self.progress.open_dialog(progress::Bulk::unlock(room));
    }

    #[cfg(test)]
    pub fn tab(&self) -> Tab {
        self.tab
    }

    /// How many previews the open dialog of the Progress tab has computed.
    #[cfg(test)]
    pub fn preview_runs(&self) -> Option<usize> {
        self.progress.preview_runs()
    }

    /// The Progress tab, scrolled to the story scenes (`--view cutscenes`).
    pub fn scroll_to_scenes(&mut self) {
        self.tab = Tab::Progress;
        self.progress.scroll_to_scenes();
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
                ui.add_space(SPACING / 2.0);
                overview::progress(ui, &totals);
                dialog::section(ui, "Collected");
                overview::totals(ui, doc, &totals);
                dialog::section(ui, "Lives and bonus gem");
                overview::lives(ui, doc, &mut self.lives, actions);
                overview::bonus_gem(ui, doc, actions);
                overview::secret_room(ui, &totals, selected);
                overview::story(ui, doc);
                overview::diagnostics(ui, doc, &mut self.diagnostics.open);
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
        ui.horizontal(|ui| {
            for tab in Tab::ALL {
                if ui
                    .add(egui::Button::selectable(self.tab == tab, tab.title()))
                    .on_hover_text(tab.tip())
                    .clicked()
                {
                    self.tab = tab;
                }
            }
        });
        ui.separator();
        match self.tab {
            Tab::Levels => rooms::grid(ui, doc, selected, actions),
            Tab::Progress => self.progress.show(ui, doc, selected, actions),
        }
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
