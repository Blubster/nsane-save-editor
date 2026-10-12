//! The overview panel: progress and lives, the totals with the gems that
//! belong to no single row of the grid (the five colored gems as a set, the
//! bonus gem), bosses, super powers, the story and the diagnostics. Totals
//! are counted from the save as it is now; the stored summary is only
//! rewritten when saving.

use egui::{Align, DragValue, Layout, RichText, Stroke, Ui, Vec2, WidgetInfo, WidgetType};
use nsane::Document;
use nsane::crash3::tables::{
    MAX_BONUS_GEMS, MAX_BOSSES, MAX_CLEAR_GEMS, MAX_COLORED_GEMS, MAX_CRYSTALS, MAX_GEMS,
    MAX_PERCENT, MAX_RELICS, MAX_SECOND_GEMS,
};
use nsane::crash3::{
    ABILITIES, BOSSES, Edit, GemColor, LevelId, PICKUP_LIVES_MAX, Relic, Severity, Totals,
};

use super::{level_row, scenes};
use crate::app::{Action, Nav, static_icon};
use crate::dialog;
use crate::icons::{self, Boss, Icon, IconButton};
use crate::theme::{self, SPACING};

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
        let response = ui
            .add(
                DragValue::new(&mut value)
                    .range(LIVES_RANGE)
                    .clamp_existing_to_range(false)
                    .speed(0.25),
            )
            .on_hover_text(format!(
                "0 to 255. In game, picking up a life adds one only below {PICKUP_LIVES_MAX}: \
                 a higher count works, but pickups add nothing until it drops back under it."
            ));
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
        ui.label(theme::bold(format!("{percent}%")).size(32.0));
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
            ui.label(theme::bold(count));
        });
    })
    .response
    .on_hover_text(tip);
}

/// Crystals, gems and relics, with the colored gems and the bonus gem under
/// the gems.
pub fn totals(ui: &mut Ui, doc: &Document, totals: &Totals, actions: &mut Vec<Action>) {
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
    colored_gems(ui, doc, actions);
    bonus_gem(ui, doc, actions);
    ui.horizontal(|ui| {
        static_icon(ui, Icon::Relic(Relic::Gold), theme::ICON_MEDIUM);
        ui.label("Relics");
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(theme::bold(format!("{} / {MAX_RELICS}", totals.relics())));
        });
    });
    ui.horizontal(|ui| {
        indent(ui);
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
}

/// Lines a sub-row up with the name of the row above, after its icon.
fn indent(ui: &mut Ui) {
    ui.add_space(theme::ICON_MEDIUM + ui.spacing().item_spacing.x);
}

/// The five colored gems, each toggled in the level it is found in. They
/// open paths in other levels, so they are thought of as a set.
fn colored_gems(ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>) {
    ui.horizontal(|ui| {
        indent(ui);
        for color in GemColor::ALL {
            let Some(level) = LevelId::all().find(|l| l.level().colored_gem == Some(color)) else {
                continue;
            };
            // Not the grid's name: each label is found once.
            let name = format!("Overview: {} gem", color.key());
            let size = (theme::ICON_MEDIUM, theme::ICON_MEDIUM);
            level_row::colored_gem_toggle(ui, doc, level, &name, size, actions);
        }
    });
}

/// A section title with a count on the right: "Bosses 2 / 5". The count
/// is named "Bosses: 2 / 5" for screen readers and tests.
fn counted_section(ui: &mut Ui, title: &str, count: String, tip: &str) {
    ui.add_space(SPACING);
    ui.horizontal(|ui| {
        ui.label(theme::bold(title).size(theme::FONT_TITLE));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let name = format!("{title}: {count}");
            ui.label(theme::bold(count))
                .widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &name));
        });
    })
    .response
    .on_hover_text(tip);
    ui.add_space(2.0);
}

/// The defeated bosses. A click on one asks for its page.
pub fn bosses(
    ui: &mut Ui,
    doc: &Document,
    totals: &Totals,
    selected: Option<LevelId>,
    nav: &mut Vec<Nav>,
) {
    counted_section(
        ui,
        "Bosses",
        format!("{} / {MAX_BOSSES}", totals.bosses),
        "Each defeated boss counts 1% and opens the next warp room",
    );
    let save = doc.current();
    ui.horizontal(|ui| {
        for boss in Boss::ALL {
            let level = boss.level();
            let defeated = save.boss_defeated(level);
            // Not the grid's or the page's names: each label is found once.
            let name = format!("Overview: {}", level.name());
            let response = ui
                .add(
                    IconButton::new(Icon::Boss(boss), &name)
                        .owned(defeated)
                        .selected(selected == Some(level))
                        .size(theme::ICON_MEDIUM, icons::MIN_HIT),
                )
                .on_hover_text(format!(
                    "{}: {}. Click to show its page.",
                    level.name(),
                    if defeated { "defeated" } else { "not defeated" }
                ));
            if response.clicked() {
                nav.push(Nav::Select(level));
            }
        }
    });
}

/// A toggle per super power, with its name.
pub fn super_powers(ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>) {
    let save = doc.current();
    let on_count = ABILITIES.iter().filter(|&&a| save.ability(a)).count();
    counted_section(
        ui,
        "Super powers",
        format!("{on_count} / {}", ABILITIES.len()),
        "A super power works even if the boss that grants it isn't beaten (verified in game)",
    );
    for (ability, boss) in ABILITIES.into_iter().zip(BOSSES) {
        let on = save.ability(ability);
        let tip = format!(
            "Normally granted by {}. Click to turn it {}.",
            boss.name(),
            if on { "off" } else { "on" }
        );
        ui.horizontal(|ui| {
            let name = format!("Super power: {}", ability.name());
            let response = icons::icon_toggle_named(
                ui,
                Icon::Ability(ability),
                &name,
                on,
                true,
                theme::ICON_MEDIUM,
            )
            .on_hover_text(&tip);
            if response.clicked() {
                level_row::push(actions, doc, Edit::SetAbility { ability, on: !on });
            }
            ui.label(ability.name()).on_hover_text(&tip);
        });
    }
}

/// The bonus gem toggle, with a badge when the hub would give it now.
fn bonus_gem(ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>) {
    let save = doc.current();
    let on = save.bonus_gem();
    ui.horizontal(|ui| {
        indent(ui);
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
        .fill(theme::mix(color, pal.chrome, 82))
        .stroke(Stroke::new(1.0, color))
        .corner_radius(theme::ROUNDING_SMALL)
        .inner_margin(egui::Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(text).color(pal.text).small());
        })
        .response
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
        Stroke::new(1.0, pal.border),
        egui::StrokeKind::Inside,
    );
}

/// Story completion, the last level played, whether the save was made
/// inside a level, and the story scenes seen, whose Edit… button sets
/// `open_scenes`.
pub fn story(ui: &mut Ui, doc: &Document, open_scenes: &mut bool) {
    dialog::section(ui, "Story");
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
    let zone = save.previous_level_name().unwrap_or_default();
    let label = ui.label(format!(
        "Last level played: {}",
        nsane::diff::last_level_text(&zone)
    ));
    if !zone.is_empty() && !nsane::crash3::is_zone_path(&zone) {
        label.on_hover_text(
            "The game stored a few bytes that aren't a level here. It can do that when it \
             saves before any level was entered. Harmless.",
        );
    }
    if save.active_progression_index().is_some_and(|i| i != 0) {
        ui.label("Saved inside a level");
    }
    let (seen, total) = scenes::seen_count(save);
    // The button goes under the count when the panel is too narrow.
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::Label::new(format!("Scenes: {seen} / {total} seen")).extend())
            .on_hover_text("A scene marked as seen doesn't play");
        if ui
            .button("Edit…")
            .on_hover_text("Mark story scenes as seen or not seen")
            .clicked()
        {
            *open_scenes = true;
        }
    });
}

/// Counts by severity, which open the diagnostics panel, then the
/// messages.
pub fn diagnostics(ui: &mut Ui, doc: &Document, open_panel: &mut bool) {
    let all = doc.diagnostics();
    ui.add_space(SPACING);
    ui.horizontal(|ui| {
        ui.label(theme::bold("Diagnostics").size(theme::FONT_TITLE));
        if super::diagnostics_ui::badges(ui, all) {
            *open_panel = true;
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
                ui.horizontal(|ui| {
                    static_icon(
                        ui,
                        super::preview::severity_icon(d.severity),
                        theme::ICON_SMALL,
                    );
                    ui.add(egui::Label::new(d.message.as_str()).wrap());
                });
            }
        });
    if ui
        .button("Fix or show…")
        .on_hover_text("Open the diagnostics panel, with a fix for each message that has one")
        .clicked()
    {
        *open_panel = true;
    }
}
