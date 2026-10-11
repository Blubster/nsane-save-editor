//! The overview panel: progress, totals, lives, the bonus gem, the secret
//! warp room, the story and the diagnostics. Totals are counted from the
//! save as it is now; the stored summary is only rewritten when saving.

use egui::{Align, DragValue, Layout, RichText, Stroke, Ui, Vec2};
use nsane::Document;
use nsane::crash3::tables::{
    MAX_BONUS_GEMS, MAX_BOSSES, MAX_CLEAR_GEMS, MAX_COLORED_GEMS, MAX_CRYSTALS, MAX_GEMS,
    MAX_PERCENT, MAX_RELICS, MAX_SECOND_GEMS,
};
use nsane::crash3::{BOSSES, Edit, LevelId, Relic, SECRET_ROOM, Severity, Totals};

use super::level_row;
use super::rooms;
use crate::app::{Action, static_icon};
use crate::icons::{self, Boss, Icon, IconState};
use crate::theme;

/// The range the editor writes, as the lives edit accepts it.
const LIVES_RANGE: std::ops::RangeInclusive<i32> = 0..=255;

/// The lives field. A drag or a typed value becomes one edit, applied when
/// the drag ends or the field loses focus, so that it is one undo step.
pub fn lives(ui: &mut Ui, doc: &Document, pending: &mut Option<i32>, actions: &mut Vec<Action>) {
    let stored = doc.current().lives();
    let mut value = pending.unwrap_or(stored);
    ui.horizontal(|ui| {
        static_icon(ui, Icon::Lives, theme::ICON_MEDIUM).on_hover_text("Lives");
        ui.label("Lives");
        // A value another tool wrote outside the range is shown as it is,
        // and clamped only when the user changes it.
        let response = ui.add(
            DragValue::new(&mut value)
                .range(LIVES_RANGE)
                .clamp_existing_to_range(false)
                .speed(0.25),
        );
        if response.changed() {
            *pending = Some(value);
        }
        if let Some(v) = *pending
            && !response.dragged()
            && !response.has_focus()
        {
            *pending = None;
            let count = v.clamp(*LIVES_RANGE.start(), *LIVES_RANGE.end()) as u8;
            if i32::from(count) != stored {
                let edit = Edit::SetLives { count };
                actions.push(Action::Apply {
                    label: edit.describe(doc.current()),
                    edits: vec![edit],
                });
            }
        }
        if !LIVES_RANGE.contains(&stored) && pending.is_none() {
            static_icon(ui, Icon::Warning, theme::ICON_SMALL).on_hover_text(format!(
                "This save holds {stored} lives, outside the range 0–255. \
                 Changing the value brings it back into the range."
            ));
        }
    });
}

/// The percentage, large, with a bar to 108.
pub fn progress(ui: &mut Ui, totals: &Totals) {
    let percent = totals.percent();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{percent}%")).size(32.0).strong());
        ui.with_layout(Layout::right_to_left(Align::Max), |ui| {
            ui.weak(format!("of {MAX_PERCENT}%"));
        });
    });
    let pal = theme::palette(ui.visuals().dark_mode);
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), egui::Sense::hover());
    bar(
        ui,
        rect,
        percent as f32 / MAX_PERCENT as f32,
        pal.accent.base,
    );
    response.on_hover_text(
        "One point per crystal, gem, relic (any tier) and boss, counted from the save as \
         it is now",
    );
}

/// An icon, a name and a count on the right.
fn count_row(ui: &mut Ui, icon: Icon, name: &str, count: String, tip: String) {
    ui.horizontal(|ui| {
        static_icon(ui, icon, theme::ICON_MEDIUM);
        ui.label(name);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(count).strong());
        });
    })
    .response
    .on_hover_text(tip);
}

/// Crystals, gems, relics and bosses.
pub fn totals(ui: &mut Ui, doc: &Document, totals: &Totals) {
    count_row(
        ui,
        Icon::Crystal,
        "Crystals",
        format!("{} / {MAX_CRYSTALS}", totals.crystals),
        "One in each of levels 1–25".to_owned(),
    );
    let g = &totals.gem_breakdown;
    count_row(
        ui,
        Icon::ClearGem,
        "Gems",
        format!("{} / {MAX_GEMS}", totals.gems),
        format!(
            "Clear {} / {MAX_CLEAR_GEMS}, second {} / {MAX_SECOND_GEMS}, \
             colored {} / {MAX_COLORED_GEMS}, bonus {} / {MAX_BONUS_GEMS}",
            g.clear, g.second, g.colored, g.bonus
        ),
    );
    ui.horizontal(|ui| {
        static_icon(ui, Icon::Relic(Relic::Gold), theme::ICON_MEDIUM);
        ui.label("Relics");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(format!("{} / {MAX_RELICS}", totals.relics())).strong());
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(theme::ICON_MEDIUM + ui.spacing().item_spacing.x);
        for (relic, count) in [
            (Relic::Sapphire, totals.sapphire),
            (Relic::Gold, totals.gold),
            (Relic::Platinum, totals.platinum),
        ] {
            static_icon(ui, Icon::Relic(relic), theme::ICON_SMALL)
                .on_hover_text(format!("{count} {} relics", relic.name().to_lowercase()));
            ui.label(count.to_string());
        }
    });
    ui.horizontal(|ui| {
        ui.label("Bosses");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(format!("{} / {MAX_BOSSES}", totals.bosses)).strong());
        });
    });
    ui.horizontal(|ui| {
        let save = doc.current();
        for boss in Boss::ALL {
            let level = boss.level();
            let defeated = save.boss_defeated(level);
            let state = if defeated {
                IconState::OWNED
            } else {
                IconState::MISSING
            };
            let (rect, response) =
                ui.allocate_exact_size(Vec2::splat(theme::ICON_MEDIUM), egui::Sense::hover());
            icons::paint(
                ui.painter(),
                rect,
                Icon::Boss(boss),
                state,
                ui.visuals().dark_mode,
            );
            response.on_hover_text(format!(
                "{}: {}",
                level.name(),
                if defeated { "defeated" } else { "not defeated" }
            ));
        }
    });
}

/// The bonus gem toggle, with a badge when the hub would give it now.
pub fn bonus_gem(ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>) {
    let save = doc.current();
    let on = save.bonus_gem();
    ui.horizontal(|ui| {
        let tip = format!(
            "The 47th gem, given in the hub for every other gem and gold or platinum \
             relics: {}. Click to {}.",
            if on { "taken" } else { "not taken" },
            if on { "remove it" } else { "add it" },
        );
        let response = icons::icon_toggle_named(
            ui,
            Icon::BonusGem,
            "Hub: bonus gem",
            on,
            true,
            theme::ICON_MEDIUM,
        )
        .on_hover_text(tip);
        if response.clicked() {
            level_row::push(actions, doc, Edit::SetBonusGem { on: !on });
        }
        ui.label("Bonus gem");
        if save.bonus_gem_due() {
            badge(ui, "due", theme::palette(ui.visuals().dark_mode).info.base).on_hover_text(
                "The hub would give the bonus gem now: 30 gold or platinum relics and 107%.",
            );
        }
    });
}

/// A small coloured tag.
fn badge(ui: &mut Ui, text: &str, color: egui::Color32) -> egui::Response {
    let pal = theme::palette(ui.visuals().dark_mode);
    egui::Frame::new()
        .fill(theme::mix(color, pal.background, 80))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(theme::ROUNDING_SMALL)
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(pal.text).small());
        })
        .response
}

/// The relic count against the secret room's thresholds, and its levels.
pub fn secret_room(ui: &mut Ui, totals: &Totals, selected: &mut Option<LevelId>) {
    let relics = totals.relics();
    ui.label(RichText::new("Secret warp room").strong());
    relic_bar(ui, relics);
    rooms::secret_room_list(ui, relics, selected);
}

/// A thin bar filled to `fraction`; empty at 0, unlike egui's progress bar.
fn bar(ui: &Ui, rect: egui::Rect, fraction: f32, color: egui::Color32) {
    let pal = theme::palette(ui.visuals().dark_mode);
    let painter = ui.painter();
    painter.rect_filled(rect, theme::ROUNDING_SMALL, pal.field);
    if fraction > 0.0 {
        let right = rect.left() + rect.width() * fraction.min(1.0);
        let filled = egui::Rect::from_min_max(rect.min, egui::pos2(right, rect.bottom()));
        painter.rect_filled(filled, theme::ROUNDING_SMALL, color);
    }
    painter.rect_stroke(
        rect,
        theme::ROUNDING_SMALL,
        Stroke::new(1.0, pal.faint),
        egui::StrokeKind::Inside,
    );
}

/// A bar of the relic count with a tick at each level's threshold.
fn relic_bar(ui: &mut Ui, relics: u32) {
    let pal = theme::palette(ui.visuals().dark_mode);
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 26.0), egui::Sense::hover());
    let bar_rect = egui::Rect::from_min_size(rect.min, Vec2::new(width, 8.0));
    let fraction = |count: u32| count.min(MAX_RELICS) as f32 / MAX_RELICS as f32;
    bar(ui, bar_rect, fraction(relics), pal.relic_sapphire.base);
    let bar = bar_rect;
    let painter = ui.painter();
    let x = |count: u32| bar.left() + bar.width() * fraction(count);
    for entry in SECRET_ROOM {
        let tx = x(u32::from(entry.relics));
        let color = if relics >= u32::from(entry.relics) {
            pal.text
        } else {
            pal.muted
        };
        painter.line_segment(
            [
                egui::pos2(tx, bar.top() - 2.0),
                egui::pos2(tx, bar.bottom() + 2.0),
            ],
            Stroke::new(1.5, color),
        );
        painter.text(
            egui::pos2(tx, bar.bottom() + 3.0),
            egui::Align2::CENTER_TOP,
            entry.relics.to_string(),
            egui::FontId::proportional(theme::FONT_SMALL),
            color,
        );
    }
    response.on_hover_text(format!(
        "{relics} of {MAX_RELICS} relics. Each 5 relics, of any tier, open the next level \
         of the secret warp room."
    ));
}

/// Story completion, the last level played and whether the save was made
/// inside a level.
pub fn story(ui: &mut Ui, doc: &Document) {
    let save = doc.current();
    let cortex = BOSSES[BOSSES.len() - 1];
    // Counted from the boss, like the other totals: the stored value is only
    // rewritten when saving.
    let completed = save.boss_defeated(cortex);
    ui.label(format!(
        "Story completed: {}",
        if completed { "yes" } else { "no" }
    ))
    .on_hover_text(format!("Whether {} is defeated", cortex.name()));
    let last = match save.previous_level_name() {
        Some(zone) if !zone.is_empty() => match LevelId::from_zone(&zone) {
            Some(level) => level.name().to_owned(),
            None => zone,
        },
        _ => "none".to_owned(),
    };
    ui.label(format!("Last level played: {last}"));
    if save.active_progression_index().is_some_and(|i| i != 0) {
        ui.label("Saved inside a level");
    }
}

/// Counts by severity, then the messages.
pub fn diagnostics(ui: &mut Ui, doc: &Document) {
    let all = doc.diagnostics();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Diagnostics").strong());
        for (severity, icon, name) in [
            (Severity::Error, Icon::Error, "errors"),
            (Severity::Warning, Icon::Warning, "warnings"),
            (Severity::Info, Icon::Info, "notes"),
        ] {
            let count = all.iter().filter(|d| d.severity == severity).count();
            if count > 0 {
                static_icon(ui, icon, theme::ICON_SMALL).on_hover_text(format!("{count} {name}"));
                ui.label(count.to_string());
            }
        }
    });
    if all.is_empty() {
        ui.weak("No problems found.");
        return;
    }
    let serious = all.iter().any(|d| d.severity > Severity::Info);
    egui::CollapsingHeader::new(format!("{} messages", all.len()))
        .id_salt("diagnostic messages")
        .default_open(serious)
        .show(ui, |ui| {
            for d in all {
                let icon = match d.severity {
                    Severity::Error => Icon::Error,
                    Severity::Warning => Icon::Warning,
                    Severity::Info => Icon::Info,
                };
                ui.horizontal(|ui| {
                    static_icon(ui, icon, theme::ICON_SMALL);
                    ui.add(egui::Label::new(d.message.as_str()).wrap());
                });
            }
        });
}
