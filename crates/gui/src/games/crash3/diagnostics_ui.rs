//! The diagnostics panel: everything worth telling about the save, most
//! severe first, each with a Fix (one undo step) and a Show (the level's
//! page). It is a window beside the editor, not a modal, so that the levels
//! and the undo shortcuts stay at hand while it is open.

use egui::{Button, Context, Response, RichText, Sense, Ui, Vec2, WidgetInfo, WidgetType};
use nsane::Document;
use nsane::crash3::{Diagnostic, LevelId, Severity};

use super::preview::severity_icon;
use crate::app::{Action, Nav};
use crate::dialog::plural;
use crate::icons::{self, IconState};
use crate::theme::{self, SPACING};

const SEVERITIES: [(Severity, &str, &str); 3] = [
    (Severity::Error, "error", "errors"),
    (Severity::Warning, "warning", "warnings"),
    (Severity::Info, "note", "notes"),
];

/// What the panel keeps between frames.
#[derive(Debug, Default)]
pub struct DiagnosticsPanel {
    pub open: bool,
}

/// An icon and a count, clickable; named "2 warnings" for screen readers.
fn count_badge(ui: &mut Ui, severity: Severity, count: usize, name: &str) -> Response {
    let text = count.to_string();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().text_color();
    let galley = ui.painter().layout_no_wrap(text, font, color);
    let icon = theme::ICON_SMALL;
    let size = Vec2::new(
        4.0 + icon + 4.0 + galley.size().x + 6.0,
        icon.max(galley.size().y) + 4.0,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let pal = theme::palette(ui.visuals().dark_mode);
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, theme::ROUNDING_SMALL, pal.hover_bg);
        }
        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 4.0, rect.center().y - icon / 2.0),
            Vec2::splat(icon),
        );
        icons::paint(
            ui.painter(),
            icon_rect,
            severity_icon(severity),
            IconState::OWNED,
            ui.visuals().dark_mode,
        );
        let pos = egui::pos2(
            icon_rect.right() + 4.0,
            rect.center().y - galley.size().y / 2.0,
        );
        ui.painter().galley(pos, galley, color);
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A badge per severity present. Returns whether one was clicked.
pub fn badges(ui: &mut Ui, all: &[Diagnostic]) -> bool {
    let mut clicked = false;
    for (severity, one, many) in SEVERITIES {
        let count = all.iter().filter(|d| d.severity == severity).count();
        if count > 0 {
            let name = plural(count, one, many);
            clicked |= count_badge(ui, severity, count, &name)
                .on_hover_text(format!("{name}. Click to open the diagnostics."))
                .clicked();
        }
    }
    clicked
}

/// "1 warning, 30 notes".
fn counts_text(all: &[Diagnostic]) -> String {
    SEVERITIES
        .iter()
        .filter_map(|&(severity, one, many)| {
            let count = all.iter().filter(|d| d.severity == severity).count();
            (count > 0).then(|| plural(count, one, many))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The first error, which keeps the file from being written.
pub fn save_blocker(doc: &Document) -> Option<&Diagnostic> {
    doc.diagnostics()
        .iter()
        .find(|d| d.severity == Severity::Error)
}

impl DiagnosticsPanel {
    pub fn show(
        &mut self,
        ctx: &Context,
        doc: &Document,
        shown: Option<LevelId>,
        nav: &mut Vec<Nav>,
        actions: &mut Vec<Action>,
    ) {
        let mut open = self.open;
        egui::Window::new("Diagnostics")
            .id(egui::Id::new("diagnostics-panel"))
            .open(&mut open)
            .collapsible(false)
            .frame(crate::dialog::window_frame(ctx))
            .default_width(460.0)
            .default_height(420.0)
            .pivot(egui::Align2::RIGHT_TOP)
            .default_pos(ctx.content_rect().right_top() + Vec2::new(-24.0, 96.0))
            .show(ctx, |ui| list(ui, doc, shown, nav, actions));
        self.open = open;
    }
}

fn list(
    ui: &mut Ui,
    doc: &Document,
    shown: Option<LevelId>,
    nav: &mut Vec<Nav>,
    actions: &mut Vec<Action>,
) {
    let all = doc.diagnostics();
    if all.is_empty() {
        ui.label("No problems found.");
        return;
    }
    ui.label(counts_text(all));
    ui.weak("Each fix is one step: undo takes it back.");
    ui.add_space(SPACING / 2.0);
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (i, d) in all.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                entry(ui, d, shown, nav, actions);
            }
        });
}

fn entry(
    ui: &mut Ui,
    d: &Diagnostic,
    shown: Option<LevelId>,
    nav: &mut Vec<Nav>,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        crate::app::static_icon(ui, severity_icon(d.severity), theme::ICON_MEDIUM);
        ui.add(egui::Label::new(d.message.as_str()).wrap());
    });
    let has_buttons = d.fix.is_some() || d.level.is_some();
    if !has_buttons {
        return;
    }
    ui.horizontal(|ui| {
        ui.add_space(theme::ICON_MEDIUM + ui.spacing().item_spacing.x);
        if let Some(fix) = &d.fix {
            let response = ui
                .add(Button::new("Fix"))
                .on_hover_text(format!("{}. Undo takes it back.", fix.label));
            let name = format!("Fix: {}", fix.label);
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
            if response.clicked() {
                actions.push(Action::Apply {
                    label: fix.label.clone(),
                    edits: fix.edits.clone(),
                });
            }
        }
        if let Some(level) = d.level {
            let is_shown = shown == Some(level);
            let response = ui
                .add_enabled(!is_shown, Button::new("Show"))
                .on_hover_text(format!("Show {}", level.name()))
                .on_disabled_hover_text(format!("{} is shown", level.name()));
            let name = format!("Show {}", level.name());
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, !is_shown, &name));
            if response.clicked() {
                nav.push(Nav::Select(level));
            }
        }
        if d.fix.is_none() {
            ui.label(RichText::new("No automatic fix.").weak().small());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::repo_file;

    #[test]
    fn counts_read_as_text() {
        let bytes = std::fs::read(repo_file("tests/saves/edited/t4_crystals.sav")).unwrap();
        let doc = Document::open(&bytes, None).unwrap();
        let text = counts_text(doc.diagnostics());
        assert!(text.starts_with("1 warning"), "{text}");
        assert!(save_blocker(&doc).is_none());
    }
}
