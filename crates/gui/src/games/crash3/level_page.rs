//! A level's page, shown in place of the warp room grid while a level is
//! selected: the way back to the grid, a tab per warp room, the room's card
//! when there is room for it, and the level's details.
//!
//! The header and the card don't change the selection at once. The details
//! are drawn for the level the frame started with, and the new one is
//! selected once the page is drawn: a leaderboard field commits its text
//! when it loses focus, which it only does while it is still drawn.

use egui::{
    Align, Button, Layout, Rect, Response, ScrollArea, Ui, UiBuilder, WidgetInfo, WidgetType, pos2,
    vec2,
};
use nsane::Document;
use nsane::crash3::LevelId;

use super::bulk::Bulk;
use super::level_panel::LevelPanel;
use super::rooms;
use crate::app::{Action, Nav};
use crate::layout::{ROOM_CARD_WIDTH, ROOM_COLUMN_MIN};
use crate::theme::SPACING;

/// Between the room's card and the details.
const COLUMN_GAP: f32 = 16.0;
/// The details in one column are at most this wide.
const DETAILS_WIDTH: f32 = 560.0;
/// From this width of the details' area, they take two columns.
const TWO_COLUMNS_MIN: f32 = 900.0;
/// Each of the two columns, at most.
const HALF_WIDTH: f32 = 440.0;
/// Between the two columns.
const HALF_GAP: f32 = 24.0;

/// The rows of a room's card, in order: rooms 1–5 hold their five levels
/// and their boss, the secret room (6) its three levels and the three
/// secret levels. Its entrance rows 27 and 29 show levels of rooms 2 and 4.
pub fn stops(room: u8) -> Vec<LevelId> {
    LevelId::all()
        .filter(|l| l.level().warp_room == room)
        .collect()
}

/// The page of the selected level. A click on its card or its header
/// selects another level once the page is drawn; ‹ All rooms asks to go
/// back. A padlock of the card puts the dialog it opens in `open`.
pub fn show(
    ui: &mut Ui,
    doc: &Document,
    selected: &mut Option<LevelId>,
    panel: &mut LevelPanel,
    nav: &mut Vec<Nav>,
    actions: &mut Vec<Action>,
    open: &mut Option<Bulk>,
) {
    let Some(level) = *selected else {
        return;
    };
    let mut next = Some(level);
    panel.sync(doc, level);
    let room_column = ui.available_width() >= ROOM_COLUMN_MIN;
    header(ui, level, room_column, &mut next, nav);
    ui.add_space(SPACING);
    if room_column {
        let rect = ui.available_rect_before_wrap();
        let card = Rect::from_min_size(rect.min, vec2(ROOM_CARD_WIDTH, rect.height()));
        ui.scope_builder(UiBuilder::new().max_rect(card), |ui| {
            ScrollArea::vertical()
                .id_salt("level-room")
                .auto_shrink(false)
                .show(ui, |ui| {
                    rooms::room(ui, doc, level.level().warp_room, &mut next, actions, open);
                });
        });
        let rest = Rect::from_min_max(pos2(card.right() + COLUMN_GAP, rect.top()), rect.max);
        ui.scope_builder(UiBuilder::new().max_rect(rest), |ui| {
            details(ui, doc, level, panel, actions);
        });
    } else {
        details(ui, doc, level, panel, actions);
    }
    panel.reset_dialog(ui.ctx(), doc, actions);
    *selected = next;
}

/// Names a button for screen readers and tests, when its text is a symbol
/// or repeats another label.
fn named(response: &Response, name: &str, selected: Option<bool>) {
    response.widget_info(|| match selected {
        Some(s) => WidgetInfo::selected(WidgetType::Button, response.enabled(), s, name),
        None => WidgetInfo::labeled(WidgetType::Button, response.enabled(), name),
    });
}

/// ‹ All rooms, the room tabs, and ‹ › to step through the room's rows
/// when its card isn't shown.
fn header(
    ui: &mut Ui,
    level: LevelId,
    room_column: bool,
    next: &mut Option<LevelId>,
    nav: &mut Vec<Nav>,
) {
    let room = level.level().warp_room;
    ui.horizontal_wrapped(|ui| {
        let back = ui
            .button("‹ All rooms")
            .on_hover_text("Back to the warp rooms (Escape)");
        named(&back, "All rooms", None);
        if back.clicked() {
            nav.push(Nav::Back);
        }
        ui.add_space(SPACING);
        for r in 1..=6 {
            let (text, name, tip) = if r == 6 {
                (
                    "Secret".to_owned(),
                    "Show the secret warp room".to_owned(),
                    "The secret warp room".to_owned(),
                )
            } else {
                (
                    r.to_string(),
                    format!("Show warp room {r}"),
                    format!("Warp room {r}"),
                )
            };
            let response = ui
                .add(Button::selectable(r == room, text))
                .on_hover_text(tip);
            named(&response, &name, Some(r == room));
            if response.clicked() {
                *next = stops(r).first().copied();
            }
        }
        if room_column {
            return;
        }
        let stops = stops(room);
        let at = stops.iter().position(|&l| l == level);
        let previous = at
            .and_then(|i| i.checked_sub(1))
            .and_then(|i| stops.get(i))
            .copied();
        let following = at.and_then(|i| stops.get(i + 1)).copied();
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for (target, text, name) in [
                (following, "›", "Next level"),
                (previous, "‹", "Previous level"),
            ] {
                let response = ui.add_enabled(target.is_some(), Button::new(text));
                let response = match target {
                    Some(l) => response.on_hover_text(format!("{name}: {}", l.name())),
                    None => response,
                };
                named(&response, name, None);
                if response.clicked() {
                    *next = target;
                }
            }
        });
    });
}

/// The details, at most `DETAILS_WIDTH` wide, or in two columns when the
/// area is wide: what can be edited on the left, the stats on the right.
fn details(
    ui: &mut Ui,
    doc: &Document,
    level: LevelId,
    panel: &mut LevelPanel,
    actions: &mut Vec<Action>,
) {
    ScrollArea::vertical()
        .id_salt("level-details")
        .auto_shrink(false)
        .show(ui, |ui| {
            let width = ui.available_width();
            if width >= TWO_COLUMNS_MIN {
                ui.horizontal_top(|ui| {
                    column(ui, HALF_WIDTH, |ui| {
                        panel.main_part(ui, doc, level, actions)
                    });
                    ui.add_space(HALF_GAP - ui.spacing().item_spacing.x);
                    column(ui, HALF_WIDTH, |ui| panel.side_part(ui, doc, level));
                });
            } else {
                column(ui, width.min(DETAILS_WIDTH), |ui| {
                    panel.main_part(ui, doc, level, actions);
                    ui.separator();
                    panel.side_part(ui, doc, level);
                });
            }
        });
}

fn column(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.set_width(width);
        add(ui);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_room_has_its_rows_in_card_order() {
        let names = |room| -> Vec<&str> { stops(room).iter().map(|l| l.name()).collect() };
        assert_eq!(
            names(3),
            [
                "Dino Might!",
                "Deep Trouble",
                "High Time",
                "Road Crash",
                "Double Header",
                "Dr. N. Tropy"
            ]
        );
        assert_eq!(
            names(6),
            [
                "Ski Crazed",
                "Area 51?",
                "Rings of Power",
                "Hot Coco",
                "Eggipus Rex",
                "Future Tense"
            ]
        );
        let all: usize = (1..=6).map(|r| stops(r).len()).sum();
        assert_eq!(all, LevelId::all().count());
    }
}
