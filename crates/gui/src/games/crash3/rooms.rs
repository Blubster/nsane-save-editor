//! The warp room grid: one card per room, a row per level and boss.

use egui::{
    Align, Frame, Layout, Margin, RichText, ScrollArea, Sense, Stroke, Ui, Vec2, WidgetInfo,
    WidgetType,
};
use nsane::Document;
use nsane::crash3::tables::SecretRoomLevel;
use nsane::crash3::{BOSSES, Kind, LevelId, SECRET_ROOM};

use super::level_row::{self, ICON};
use crate::app::{Action, static_icon};
use crate::icons::{self, Icon, IconState};
use crate::theme;

/// The narrowest a card gets before the grid drops a column.
const MIN_CARD_WIDTH: f32 = 330.0;
/// Space between cards.
pub const CARD_GAP: f32 = 12.0;
/// The relic counts that open the secret room's levels start here.
const SECRET_ROOM_OPENS_AT: u32 = SECRET_ROOM[0].relics as u32;

/// Columns of cards for a width: 3 on wide windows, 2 on medium, 1 on narrow.
pub fn columns_for(width: f32) -> usize {
    (1..=3)
        .rev()
        .find(|&n| n as f32 * MIN_CARD_WIDTH + (n - 1) as f32 * CARD_GAP <= width)
        .unwrap_or(1)
}

/// Whether warp room `room` (1–5) is open: room 1 always, room N+1 once
/// boss N is defeated (`docs/FORMAT.md` §7).
pub fn room_open(doc: &Document, room: u8) -> bool {
    room <= 1 || doc.current().boss_defeated(BOSSES[usize::from(room) - 2])
}

/// The six cards, laid out in as many columns as fit.
pub fn grid(
    ui: &mut Ui,
    doc: &Document,
    selected: &mut Option<LevelId>,
    actions: &mut Vec<Action>,
) {
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        let columns = columns_for(ui.available_width());
        let rooms: Vec<u8> = (1..=6).collect();
        for chunk in rooms.chunks(columns) {
            ui.columns(columns, |cols| {
                for (ui, &room) in cols.iter_mut().zip(chunk) {
                    card(ui, |ui| {
                        if room == 6 {
                            secret_card(ui, doc, selected, actions);
                        } else {
                            room_card(ui, doc, room, selected, actions);
                        }
                    });
                }
            });
            ui.add_space(CARD_GAP);
        }
    });
}

pub fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let pal = theme::palette(ui.visuals().dark_mode);
    Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.faint))
        .corner_radius(theme::ROUNDING)
        .inner_margin(Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// "Warp Room N", its lock and the right-hand summary.
fn header(ui: &mut Ui, title: &str, open: bool, lock_tip: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong().size(theme::FONT_BODY + 2.0));
        let lock = if open { Icon::Unlock } else { Icon::Lock };
        static_icon(ui, lock, theme::ICON_SMALL).on_hover_text(lock_tip);
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    ui.add_space(2.0);
}

fn room_card(
    ui: &mut Ui,
    doc: &Document,
    room: u8,
    selected: &mut Option<LevelId>,
    actions: &mut Vec<Action>,
) {
    let save = doc.current();
    let levels: Vec<LevelId> = LevelId::all()
        .filter(|l| l.level().warp_room == room)
        .collect();
    let crystals = levels.iter().filter(|&&l| save.crystal(l)).count();
    let open = room_open(doc, room);
    let lock_tip = match room {
        1 => "Open from the start".to_owned(),
        _ => {
            let boss = BOSSES[usize::from(room) - 2].name();
            if open {
                format!("Open: {boss} is defeated")
            } else {
                format!("Locked: opens when {boss} is defeated")
            }
        }
    };
    header(ui, &format!("Warp Room {room}"), open, &lock_tip, |ui| {
        ui.label(format!("{crystals} / 5"))
            .on_hover_text(format!("{crystals} of the room's 5 crystals"));
        static_icon(ui, Icon::Crystal, theme::ICON_SMALL);
    });
    for d in doc.diagnostics() {
        if d.code == "portal_glitch" && d.level.is_some_and(|l| l.level().warp_room == room) {
            banner(ui, &d.message);
        }
    }
    for level in levels {
        level_row::level_row(ui, doc, level, selected, actions, |_| {});
    }
}

/// An amber banner with a warning.
pub(super) fn banner(ui: &mut Ui, message: &str) {
    let pal = theme::palette(ui.visuals().dark_mode);
    Frame::new()
        .fill(theme::mix(pal.warning.base, pal.surface, 80))
        .stroke(Stroke::new(1.0, pal.warning.base))
        .corner_radius(theme::ROUNDING_SMALL)
        .inner_margin(Margin::same(6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                static_icon(ui, Icon::Warning, theme::ICON_SMALL);
                ui.add(egui::Label::new(RichText::new(message).color(pal.text)).wrap());
            });
        });
    ui.add_space(2.0);
}

fn secret_card(
    ui: &mut Ui,
    doc: &Document,
    selected: &mut Option<LevelId>,
    actions: &mut Vec<Action>,
) {
    let relics = doc.current().totals().relics();
    let open = relics >= SECRET_ROOM_OPENS_AT;
    let lock_tip = if open {
        format!("Open: {relics} relics")
    } else {
        format!("Locked: its first level opens with {SECRET_ROOM_OPENS_AT} relics")
    };
    header(ui, "Secret Warp Room", open, &lock_tip, |ui| {
        let unit = if relics == 1 { "relic" } else { "relics" };
        ui.label(format!("{relics} {unit}"))
            .on_hover_text("Relics of any tier open the secret room's levels");
        static_icon(
            ui,
            Icon::Relic(nsane::crash3::Relic::Sapphire),
            theme::ICON_SMALL,
        );
    });
    for entry in SECRET_ROOM {
        let lock = |ui: &mut Ui| secret_lock(ui, entry, relics);
        if entry.level.level().warp_room == 6 {
            level_row::level_row(ui, doc, entry.level, selected, actions, lock);
        } else {
            entrance_row(ui, entry, selected, lock);
        }
    }
    ui.add_space(4.0);
    ui.weak("Secret levels");
    for level in LevelId::all().filter(|l| l.level().kind == Kind::Secret) {
        let tip = secret_level_access(level);
        // Outlined, so that it reads as a note rather than a warning.
        let lead = |ui: &mut Ui| {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(ICON), Sense::hover());
            let dark = ui.visuals().dark_mode;
            icons::paint(ui.painter(), rect, Icon::Info, IconState::MISSING, dark);
            response.on_hover_text(tip);
        };
        level_row::level_row(ui, doc, level, selected, actions, lead);
    }
}

/// The lock of a secret room level, from the relic count.
fn secret_lock(ui: &mut Ui, entry: SecretRoomLevel, relics: u32) {
    let open = relics >= u32::from(entry.relics);
    let icon = if open { Icon::Unlock } else { Icon::Lock };
    let tip = format!(
        "{}: opens with {} relics (the save has {relics})",
        if open { "Open" } else { "Locked" },
        entry.relics
    );
    let name = format!(
        "{} {}: {}",
        entry.number,
        entry.level.name(),
        if open { "open" } else { "locked" }
    );
    let response = static_icon(ui, icon, ICON);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, true, &name));
    response.on_hover_text(tip);
}

/// Levels 27 and 29 are second entrances into levels 7 and 19 and share
/// their progress, so their row selects the level instead of repeating its
/// toggles.
fn entrance_row(
    ui: &mut Ui,
    entry: SecretRoomLevel,
    selected: &mut Option<LevelId>,
    lead: impl FnOnce(&mut Ui),
) {
    let target = entry.level;
    let target_number = target.level().number.unwrap_or(0);
    level_row::row(ui, |ui| {
        lead(ui);
        level_row::number(ui, Some(entry.number));
        let tip = format!(
            "Alternate entrance to level {target_number}, needs {} relics. Click to select {}.",
            entry.relics,
            target.name()
        );
        let text = RichText::new(format!("{} (alt.)", target.name())).italics();
        if level_row::name_button(ui, *selected == Some(target), text, &tip) {
            *selected = Some(target);
        }
        level_row::empty(ui, level_row::SLOTS);
        level_row::time_slot(ui, |ui| {
            ui.weak(format!("level {target_number}"));
        });
    });
}

/// How a secret level is reached (`docs/FORMAT.md` §7).
pub fn secret_level_access(level: LevelId) -> &'static str {
    match level.key() {
        "hotcoco" => "Reached through the alien road sign in Road Crash",
        "eggipusrex" => "Reached through the yellow gem path in Dino Might!",
        "futuretense" => "Reached from the pad above the hub centre, at any point in the game",
        _ => "A secret level",
    }
}

/// The secret room's levels with their lock icons, for the overview.
pub fn secret_room_list(ui: &mut Ui, relics: u32, selected: &mut Option<LevelId>) {
    for entry in SECRET_ROOM {
        let open = relics >= u32::from(entry.relics);
        ui.horizontal(|ui| {
            let icon = if open { Icon::Unlock } else { Icon::Lock };
            static_icon(ui, icon, theme::ICON_SMALL);
            let alt = entry.level.level().warp_room != 6;
            let name = if alt {
                format!("{} {} (alt.)", entry.number, entry.level.name())
            } else {
                format!("{} {}", entry.number, entry.level.name())
            };
            let tip = format!(
                "{}: opens with {} relics. Click to select {}.",
                if open { "Open" } else { "Locked" },
                entry.relics,
                entry.level.name()
            );
            if ui
                .add(egui::Button::selectable(
                    *selected == Some(entry.level),
                    name,
                ))
                .on_hover_text(tip)
                .clicked()
            {
                *selected = Some(entry.level);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_drops_columns_as_the_window_narrows() {
        assert_eq!(columns_for(1400.0), 3);
        assert_eq!(columns_for(700.0), 2);
        assert_eq!(columns_for(390.0), 1);
        assert_eq!(columns_for(100.0), 1);
    }

    #[test]
    fn every_level_is_on_a_card() {
        // The room cards take warp rooms 1–5, the secret card the rest.
        for level in &nsane::crash3::LEVELS {
            assert!((1..=6).contains(&level.warp_room), "{}", level.key);
        }
    }
}
