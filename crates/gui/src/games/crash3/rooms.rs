//! The warp room grid: one card per room, a row per level and boss. The
//! padlock of a locked room, or of a locked secret room level, opens the
//! dialog that unlocks up to it.

use egui::{
    Align, Color32, Frame, Layout, Margin, RichText, ScrollArea, Sense, Shadow, Stroke, Ui, Vec2,
    WidgetInfo, WidgetType,
};
use nsane::Document;
use nsane::crash3::tables::SecretRoomLevel;
use nsane::crash3::{BOSSES, Kind, LevelId, SECRET_ROOM};

use super::bulk::Bulk;
use super::level_row::{self, ICON};
use crate::app::{Action, static_icon};
use crate::dialog::plural;
use crate::icons::{self, Icon, IconButton, IconState};
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

/// The six cards, laid out in as many columns as fit. A click on a padlock
/// puts the dialog it opens in `open`.
pub fn grid(
    ui: &mut Ui,
    doc: &Document,
    selected: &mut Option<LevelId>,
    actions: &mut Vec<Action>,
    open: &mut Option<Bulk>,
) {
    ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        let columns = columns_for(ui.available_width());
        let rooms: Vec<u8> = (1..=6).collect();
        for chunk in rooms.chunks(columns) {
            // The gutter between columns is the item spacing.
            ui.spacing_mut().item_spacing.x = CARD_GAP;
            ui.columns(columns, |cols| {
                for (ui, &room) in cols.iter_mut().zip(chunk) {
                    ui.spacing_mut().item_spacing = theme::ITEM_SPACING;
                    card(ui, |ui| {
                        if room == 6 {
                            secret_card(ui, doc, selected, actions, open);
                        } else {
                            room_card(ui, doc, room, selected, actions, open);
                        }
                    });
                }
            });
            ui.spacing_mut().item_spacing = theme::ITEM_SPACING;
            ui.add_space(CARD_GAP - theme::ITEM_SPACING.y);
        }
    });
}

/// Inside a card.
pub const CARD_PADDING: i8 = 12;

pub fn card(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let pal = theme::palette(ui.visuals().dark_mode);
    let shadow = if pal.dark {
        Shadow::NONE
    } else {
        Shadow {
            offset: [0, 1],
            blur: 3,
            spread: 0,
            color: Color32::from_black_alpha(18),
        }
    };
    Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(theme::ROUNDING + 2)
        .shadow(shadow)
        .inner_margin(Margin::same(CARD_PADDING))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui);
        });
}

/// "Warp Room N", its lock and the right-hand summary. As tall as a
/// clickable lock, so that the rows of cards side by side line up.
fn header(ui: &mut Ui, title: &str, lock: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(icons::MIN_HIT);
        ui.label(theme::bold(title).size(theme::FONT_TITLE));
        lock(ui);
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
    ui.add_space(2.0);
}

/// A padlock that only shows a state, named for screen readers and tests.
fn static_lock(ui: &mut Ui, open: bool, size: f32, name: &str, tip: &str) {
    let icon = if open { Icon::Unlock } else { Icon::Lock };
    let response = static_icon(ui, icon, size);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Image, true, name));
    response.on_hover_text(tip);
}

/// The lock of warp room `room`: once locked, a click opens the dialog that
/// unlocks up to it.
fn room_lock(ui: &mut Ui, doc: &Document, room: u8, open: &mut Option<Bulk>) {
    let is_open = room_open(doc, room);
    let name = format!(
        "Warp Room {room}: {}",
        if is_open { "open" } else { "locked" }
    );
    if room == 1 {
        return static_lock(ui, true, theme::ICON_SMALL, &name, "Open from the start");
    }
    let boss = BOSSES[usize::from(room) - 2].name();
    if is_open {
        let tip = format!(
            "Open: {boss} is defeated. To lock the room again, mark {boss} as not defeated."
        );
        return static_lock(ui, true, theme::ICON_SMALL, &name, &tip);
    }
    let tip = format!("Locked: opens when {boss} is defeated. Click to unlock up to room {room}…");
    if ui
        .add(IconButton::new(Icon::Lock, &name))
        .on_hover_text(tip)
        .clicked()
    {
        *open = Some(Bulk::unlock(room));
    }
}

fn room_card(
    ui: &mut Ui,
    doc: &Document,
    room: u8,
    selected: &mut Option<LevelId>,
    actions: &mut Vec<Action>,
    open: &mut Option<Bulk>,
) {
    let save = doc.current();
    let levels: Vec<LevelId> = LevelId::all()
        .filter(|l| l.level().warp_room == room)
        .collect();
    let crystals = levels.iter().filter(|&&l| save.crystal(l)).count();
    let lock = |ui: &mut Ui| room_lock(ui, doc, room, open);
    header(ui, &format!("Warp Room {room}"), lock, |ui| {
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
    open: &mut Option<Bulk>,
) {
    let relics = doc.current().totals().relics();
    let first_open = relics >= SECRET_ROOM_OPENS_AT;
    let lock = |ui: &mut Ui| {
        let tip = if first_open {
            format!("Open: {relics} relics")
        } else {
            format!("Locked: its first level opens with {SECRET_ROOM_OPENS_AT} relics")
        };
        let name = format!(
            "Secret Warp Room: {}",
            if first_open { "open" } else { "locked" }
        );
        static_lock(ui, first_open, theme::ICON_SMALL, &name, &tip);
    };
    header(ui, "Secret Warp Room", lock, |ui| {
        let count = plural(relics as usize, "relic", "relics");
        let next = SECRET_ROOM.iter().find(|e| relics < u32::from(e.relics));
        let mut text = match next {
            Some(e) => format!("{count} · next level at {}", e.relics),
            None => count.clone(),
        };
        // On a narrow card, the count alone: the padlocks show the rest.
        let font = egui::TextStyle::Body.resolve(ui.style());
        let width = ui
            .painter()
            .layout_no_wrap(text.clone(), font, Color32::PLACEHOLDER)
            .size()
            .x;
        if width > ui.available_width() - ui.spacing().item_spacing.x {
            text = count;
        }
        ui.label(text)
            .on_hover_text("Relics of any tier open the secret room's levels");
    });
    for entry in SECRET_ROOM {
        let lock = |ui: &mut Ui| secret_lock(ui, entry, relics, open);
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

/// The lock of a secret room level, from the relic count: once locked, a
/// click opens the dialog that reaches its relic count.
fn secret_lock(ui: &mut Ui, entry: SecretRoomLevel, relics: u32, open: &mut Option<Bulk>) {
    let is_open = relics >= u32::from(entry.relics);
    let tip = format!(
        "{}: opens with {} relics (the save has {relics})",
        if is_open { "Open" } else { "Locked" },
        entry.relics
    );
    let name = format!(
        "{} {}: {}",
        entry.number,
        entry.level.name(),
        if is_open { "open" } else { "locked" }
    );
    if is_open {
        return static_lock(ui, true, ICON, &name, &tip);
    }
    if ui
        .add(IconButton::new(Icon::Lock, &name).size(ICON, ICON))
        .on_hover_text(format!("{tip}. Click to open up to here…"))
        .clicked()
    {
        *open = Some(Bulk::relics(entry));
    }
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
