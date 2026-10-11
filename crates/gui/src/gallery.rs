//! A developer view of every icon in every state at every size, plus rows
//! of live toggles. Opened with `--icon-gallery` or from the debug menu.

use std::collections::HashSet;

use egui::{Align2, FontId, Rect, Sense, Ui, Vec2, pos2, vec2};

use crate::icons::{self, Icon, IconState};
use crate::theme;

const STATES: [(&str, IconState); 5] = [
    ("Owned", IconState::OWNED),
    ("Missing", IconState::MISSING),
    ("Not applicable", IconState::NOT_APPLICABLE),
    ("Owned, hovered", IconState::OWNED.hovered(true)),
    ("Missing, hovered", IconState::MISSING.hovered(true)),
];

/// The icons are shown in two side-by-side columns, and the 16 and 24 point
/// sizes share a slot (one above the other), so the whole gallery fits in a
/// 1500×900 window.
const COLUMNS: usize = 2;
const NAME_WIDTH: f32 = 112.0;
const CELL_GAP: f32 = 5.0;
const GROUP_GAP: f32 = 8.0;
const COLUMN_GAP: f32 = 16.0;
const STACK_GAP: f32 = 4.0;
const HEADER_HEIGHT: f32 = 36.0;
const ROW_HEIGHT: f32 = theme::ICON_XLARGE + 4.0;

#[derive(Default)]
pub struct Gallery {
    on: HashSet<Icon>,
}

impl Gallery {
    pub fn ui(&mut self, ui: &mut Ui, dark: &mut bool) {
        ui.horizontal(|ui| {
            ui.heading("Icons");
            ui.add_space(theme::SPACING * 2.0);
            ui.selectable_value(dark, false, "Light");
            ui.selectable_value(dark, true, "Dark");
        });
        ui.add_space(theme::SPACING);
        egui::ScrollArea::both().show(ui, |ui| {
            grid(ui, *dark);
            ui.add_space(theme::SPACING * 2.0);
            ui.label("Live toggles: click, hover, or Tab to focus. The second row is disabled.");
            for enabled in [true, false] {
                ui.horizontal_wrapped(|ui| {
                    for icon in Icon::all() {
                        let on = self.on.contains(&icon);
                        let response =
                            icons::icon_toggle(ui, icon, on, enabled, theme::ICON_MEDIUM)
                                .on_hover_text(icon.name());
                        if response.clicked() && !self.on.remove(&icon) {
                            self.on.insert(icon);
                        }
                    }
                });
            }
        });
    }
}

/// The slots of a state group: their width and the sizes shown in them.
const SLOTS: [(f32, &[f32]); 3] = [
    (theme::ICON_MEDIUM, &[theme::ICON_SMALL, theme::ICON_MEDIUM]),
    (theme::ICON_LARGE, &[theme::ICON_LARGE]),
    (theme::ICON_XLARGE, &[theme::ICON_XLARGE]),
];

fn group_width() -> f32 {
    SLOTS.iter().map(|(w, _)| w).sum::<f32>() + CELL_GAP * (SLOTS.len() - 1) as f32
}

fn column_width() -> f32 {
    NAME_WIDTH + STATES.len() as f32 * (group_width() + GROUP_GAP)
}

fn grid(ui: &mut Ui, dark: bool) {
    let icons = Icon::all();
    let rows = icons.len().div_ceil(COLUMNS);
    let size = vec2(
        COLUMNS as f32 * (column_width() + COLUMN_GAP),
        HEADER_HEIGHT + rows as f32 * ROW_HEIGHT,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    for (column, chunk) in icons.chunks(rows).enumerate() {
        let left = rect.left() + column as f32 * (column_width() + COLUMN_GAP);
        header(ui, pos2(left, rect.top()));
        for (row, &icon) in chunk.iter().enumerate() {
            let top = rect.top() + HEADER_HEIGHT + row as f32 * ROW_HEIGHT;
            icon_row(ui, pos2(left, top), icon, row, dark);
        }
    }
}

fn header(ui: &Ui, origin: egui::Pos2) {
    let pal = theme::palette(ui.visuals().dark_mode);
    let painter = ui.painter();
    let mut x = origin.x + NAME_WIDTH;
    for (label, _) in STATES {
        painter.text(
            pos2(x, origin.y),
            Align2::LEFT_TOP,
            label,
            FontId::proportional(theme::FONT_BODY),
            pal.text,
        );
        let mut cx = x;
        for (width, sizes) in SLOTS {
            let label: Vec<String> = sizes.iter().map(|s| s.to_string()).collect();
            painter.text(
                pos2(cx + width / 2.0, origin.y + HEADER_HEIGHT - 2.0),
                Align2::CENTER_BOTTOM,
                label.join("/"),
                FontId::proportional(theme::FONT_SMALL),
                pal.text_weak,
            );
            cx += width + CELL_GAP;
        }
        x += group_width() + GROUP_GAP;
    }
}

fn icon_row(ui: &Ui, origin: egui::Pos2, icon: Icon, row: usize, dark: bool) {
    let pal = theme::palette(dark);
    let painter = ui.painter();
    let rect = Rect::from_min_size(origin, vec2(column_width(), ROW_HEIGHT));
    if row % 2 == 1 {
        painter.rect_filled(rect, 0.0, pal.surface);
    }
    let name = painter.layout(
        icon.name().to_owned(),
        FontId::proportional(theme::FONT_BODY),
        pal.text,
        NAME_WIDTH - 8.0,
    );
    let name_pos = pos2(rect.left() + 4.0, rect.center().y - name.size().y / 2.0);
    painter.galley(name_pos, name, pal.text);
    let mut x = rect.left() + NAME_WIDTH;
    for (_, state) in STATES {
        for (width, sizes) in SLOTS {
            let height = sizes.iter().sum::<f32>() + STACK_GAP * (sizes.len() - 1) as f32;
            let mut y = rect.center().y - height / 2.0;
            for &size in sizes {
                let min = pos2(x + (width - size) / 2.0, y);
                icons::paint(
                    painter,
                    Rect::from_min_size(min, Vec2::splat(size)),
                    icon,
                    state,
                    dark,
                );
                y += size + STACK_GAP;
            }
            x += width + CELL_GAP;
        }
        x += GROUP_GAP - CELL_GAP;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_every_size() {
        let sizes: Vec<f32> = SLOTS.iter().flat_map(|(_, s)| s.iter().copied()).collect();
        let all = [
            theme::ICON_SMALL,
            theme::ICON_MEDIUM,
            theme::ICON_LARGE,
            theme::ICON_XLARGE,
        ];
        assert_eq!(sizes, all);
        for (width, sizes) in SLOTS {
            assert!(sizes.iter().all(|&s| s <= width));
            let height = sizes.iter().sum::<f32>() + STACK_GAP * (sizes.len() - 1) as f32;
            assert!(height <= ROW_HEIGHT);
        }
    }
}
