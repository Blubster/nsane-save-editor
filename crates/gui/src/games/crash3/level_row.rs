//! The pieces of a level's row: its name, the collectible toggles, the relic
//! button and the best time. The warp room grid is made of them, and the
//! level panel can reuse each piece on its own.
//!
//! Every toggle pushes one edit; nothing here changes the document.

use egui::{Align, Button, Layout, RichText, Ui, Vec2};
use nsane::Document;
use nsane::crash3::tables::{colored_gem_opens, relic_times, second_gem_hint};
use nsane::crash3::{ABILITIES, Edit, GemColor, Kind, LevelId, Relic, format_time};

use crate::app::Action;
use crate::icons::{self, Boss, Icon};
use crate::theme;

/// Icon size in a row.
pub const ICON: f32 = 20.0;
/// The level number, right-aligned.
pub const NUMBER_WIDTH: f32 = 20.0;
/// The best time, right-aligned.
pub const TIME_WIDTH: f32 = 50.0;
/// Icon slots after the name: crystal, clear gem, the level's other gem and
/// relic. The other gem is the second gem or the colored gem: no level has
/// both, as in the game, which shows two gems per level. A collectible a
/// level doesn't have leaves its slot empty, so the columns line up.
pub const SLOTS: usize = 4;
/// Horizontal space between the pieces of a row.
const GAP: f32 = 3.0;

/// Width taken by the icon slots and the time, after the name.
pub fn trailing_width() -> f32 {
    SLOTS as f32 * (ICON + GAP) + TIME_WIDTH
}

/// Asks for `edit` as one undo step, labelled as the edit describes itself.
pub fn push(actions: &mut Vec<Action>, doc: &Document, edit: Edit) {
    actions.push(Action::Apply {
        label: edit.describe(doc.current()),
        edits: vec![edit],
    });
}

/// A row of fixed slots: the pieces are laid out left to right, `GAP` apart.
pub fn row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        add(ui)
    })
    .inner
}

/// A level, boss or secret level with all its toggles. `number` is shown in
/// front of the name; the secret card puts a lock before it with `lead`.
pub fn level_row(
    ui: &mut Ui,
    doc: &Document,
    level: LevelId,
    selected: &mut Option<LevelId>,
    actions: &mut Vec<Action>,
    lead: impl FnOnce(&mut Ui),
) {
    row(ui, |ui| {
        lead(ui);
        number(ui, level.level().number);
        name(ui, doc, level, selected);
        if level.level().kind == Kind::Boss {
            boss_icons(ui, doc, level, actions);
            empty(ui, SLOTS - 2);
            time_slot(ui, |_| {});
        } else {
            crystal(ui, doc, level, actions);
            gem(ui, doc, level, actions);
            if level.level().colored_gem.is_some() {
                colored_gem(ui, doc, level, actions);
            } else {
                second_gem(ui, doc, level, actions);
            }
            relic(ui, doc, level, actions);
            time_slot(ui, |ui| best_time(ui, doc, level));
        }
    });
}

/// The level number, or nothing, right-aligned in its slot.
pub fn number(ui: &mut Ui, number: Option<u8>) {
    fixed(
        ui,
        NUMBER_WIDTH,
        Layout::right_to_left(Align::Center),
        |ui| {
            if let Some(n) = number {
                ui.weak(n.to_string());
            }
        },
    );
}

/// The name, taking the width the slots after it leave. A click selects the
/// level; a level never entered is dimmed.
pub fn name(ui: &mut Ui, doc: &Document, level: LevelId, selected: &mut Option<LevelId>) {
    let entered = doc.current().entered(level);
    let text = if entered {
        RichText::new(level.name())
    } else {
        RichText::new(level.name()).color(theme::palette(ui.visuals().dark_mode).text_weak)
    };
    let tip = if entered {
        format!("Show the details of {}", level.name())
    } else {
        format!("Never entered. Show the details of {}", level.name())
    };
    if name_button(ui, *selected == Some(level), text, &tip) {
        *selected = Some(level);
    }
}

/// A selectable name that fills the space before the slots. Returns whether
/// it was clicked.
pub fn name_button(ui: &mut Ui, is_selected: bool, text: RichText, tip: &str) -> bool {
    let width = (ui.available_width() - trailing_width() - GAP).max(40.0);
    fixed(ui, width, Layout::left_to_right(Align::Center), |ui| {
        // Narrow padding: the name keeps the room it can.
        ui.spacing_mut().button_padding.x = 5.0;
        ui.add(Button::selectable(is_selected, text).truncate())
            .on_hover_text(tip)
            .clicked()
    })
}

/// Leaves `n` icon slots empty.
pub fn empty(ui: &mut Ui, n: usize) {
    // A widget is followed by the item spacing; `add_space` isn't.
    let slot = ICON + ui.spacing().item_spacing.x;
    for _ in 0..n {
        ui.add_space(slot);
    }
}

/// The time slot at the end of the row.
pub fn time_slot(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    fixed(ui, TIME_WIDTH, Layout::right_to_left(Align::Center), add);
}

fn fixed<R>(ui: &mut Ui, width: f32, layout: Layout, add: impl FnOnce(&mut Ui) -> R) -> R {
    let height = ui.spacing().interact_size.y.max(ICON);
    ui.allocate_ui_with_layout(Vec2::new(width, height), layout, |ui| {
        ui.set_width(width);
        add(ui)
    })
    .inner
}

/// "Collected. Click to remove." and the like, for a collectible toggle.
fn toggle_tip(what: &str, owned: bool, entered: bool, extra: &str) -> String {
    let mut tip = if owned {
        format!("{what}: collected.")
    } else {
        format!("{what}: not collected.")
    };
    if !extra.is_empty() {
        tip += " ";
        tip += extra;
    }
    tip += if owned {
        " Click to remove."
    } else {
        " Click to add."
    };
    if !owned && !entered {
        tip += " Also marks the level as entered.";
    }
    tip
}

/// A collectible toggle. Returns whether it was clicked.
fn toggle(ui: &mut Ui, icon: Icon, name: &str, on: bool, tip: String) -> bool {
    icons::icon_toggle_named(ui, icon, name, on, true, ICON)
        .on_hover_text(tip)
        .clicked()
}

/// The crystal, for levels 1–25; an empty slot otherwise.
pub fn crystal(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    if !level.level().has_crystal() {
        return empty(ui, 1);
    }
    let save = doc.current();
    let on = save.crystal(level);
    let tip = toggle_tip("Crystal", on, save.entered(level), "");
    let name = format!("{}: crystal", level.name());
    if toggle(ui, Icon::Crystal, &name, on, tip) {
        push(actions, doc, Edit::SetCrystal { level, on: !on });
    }
}

/// The clear gem for breaking every box. Every level but the bosses has one.
pub fn gem(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    if !level.level().has_relic() {
        return empty(ui, 1);
    }
    let save = doc.current();
    let on = save.gem(level);
    let tip = toggle_tip("Clear gem (break every box)", on, save.entered(level), "");
    let name = format!("{}: clear gem", level.name());
    if toggle(ui, Icon::ClearGem, &name, on, tip) {
        push(actions, doc, Edit::SetGem { level, on: !on });
    }
}

/// The second clear gem, in the ten levels that have one.
pub fn second_gem(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    let Some(how) = second_gem_hint(level) else {
        return empty(ui, 1);
    };
    let save = doc.current();
    let on = save.second_gem(level);
    let tip = toggle_tip(&format!("Second gem ({how})"), on, save.entered(level), "");
    let name = format!("{}: second gem", level.name());
    if toggle(ui, Icon::ClearGem, &name, on, tip) {
        push(actions, doc, Edit::SetSecondGem { level, on: !on });
    }
}

/// The colored gem, in the five levels it is found in.
pub fn colored_gem(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    let Some(color) = level.level().colored_gem else {
        return empty(ui, 1);
    };
    let name = format!("{}: {} gem", level.name(), color.key());
    colored_gem_toggle(ui, doc, level, &name, ICON, actions);
}

/// The toggle of the colored gem found in `level`, under `name`; the
/// overview shows the five as well.
pub fn colored_gem_toggle(
    ui: &mut Ui,
    doc: &Document,
    level: LevelId,
    name: &str,
    size: f32,
    actions: &mut Vec<Action>,
) {
    let Some(color) = level.level().colored_gem else {
        return;
    };
    let save = doc.current();
    let on = save.colored_gem(level);
    let icon = Icon::ColoredGem(color);
    let what = format!("{} (found in {})", icon.name(), level.name());
    let extra = format!("Opens paths in {}.", colored_gem_paths(color));
    let tip = toggle_tip(&what, on, save.entered(level), &extra);
    if icons::icon_toggle_named(ui, icon, name, on, true, size)
        .on_hover_text(tip)
        .clicked()
    {
        push(actions, doc, Edit::SetColoredGem { level, on: !on });
    }
}

/// The relic after `relic` in the cycle a click follows: none, sapphire,
/// gold, platinum, none.
pub fn next_relic(relic: Option<Relic>) -> Option<Relic> {
    match relic {
        None => Some(Relic::Sapphire),
        Some(Relic::Sapphire) => Some(Relic::Gold),
        Some(Relic::Gold) => Some(Relic::Platinum),
        Some(Relic::Platinum) => None,
    }
}

fn relic_name(relic: Option<Relic>) -> String {
    match relic {
        Some(r) => format!("{} relic", r.name()),
        None => "no relic".to_owned(),
    }
}

/// The relic: a click moves to the next tier, a right-click picks one.
pub fn relic(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    let Some(times) = relic_times(level) else {
        return empty(ui, 1);
    };
    let current = doc.current().relic(level);
    // With no relic, the outline is the tier a click gives, and hovering
    // previews it.
    let icon = Icon::Relic(current.unwrap_or(Relic::Sapphire));
    let next = next_relic(current);
    let mut tip = match current {
        Some(r) => format!("{} relic.", r.name()),
        None => "No relic.".to_owned(),
    };
    tip += &format!(
        " Click for {}; right-click to pick one.\nTime trial: sapphire under {}, gold under {}, \
         platinum under {}. The times are left as they are.",
        relic_name(next),
        format_time(times.sapphire),
        format_time(times.gold),
        format_time(times.platinum),
    );
    let name = format!("{}: relic", level.name());
    let response =
        icons::icon_toggle_named(ui, icon, &name, current.is_some(), true, ICON).on_hover_text(tip);
    if response.clicked() {
        push(actions, doc, Edit::SetRelic { level, relic: next });
    }
    response.context_menu(|ui| {
        for choice in [
            None,
            Some(Relic::Sapphire),
            Some(Relic::Gold),
            Some(Relic::Platinum),
        ] {
            let label = match choice {
                Some(r) => format!("{} (under {})", r.name(), format_time(times.threshold(r))),
                None => "No relic".to_owned(),
            };
            if ui.radio(current == choice, label).clicked() {
                if choice != current {
                    push(
                        actions,
                        doc,
                        Edit::SetRelic {
                            level,
                            relic: choice,
                        },
                    );
                }
                ui.close();
            }
        }
    });
}

/// The best time trial time, dimmed, or a dash.
pub fn best_time(ui: &mut Ui, doc: &Document, level: LevelId) {
    let times = doc.current().times(level);
    match times.iter().min() {
        Some(&best) => {
            let all: Vec<_> = {
                let mut sorted = times.clone();
                sorted.sort_unstable();
                sorted.into_iter().map(format_time).collect()
            };
            ui.weak(format_time(best))
                .on_hover_text(format!("Time trial: {}", all.join(", ")));
        }
        None => {
            ui.weak("—").on_hover_text("No time trial time");
        }
    }
}

/// The boss and the ability his defeat normally grants.
pub fn boss_icons(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    let Some(boss) = Boss::of(level) else {
        return empty(ui, 2);
    };
    let save = doc.current();
    let defeated = save.boss_defeated(level);
    let opens = match level.level().warp_room {
        5 => "Ends the story.".to_owned(),
        room => format!("Opens warp room {}.", room + 1),
    };
    let tip = format!(
        "{}: {}. {opens} Click to mark {}.",
        level.name(),
        if defeated { "defeated" } else { "not defeated" },
        if defeated { "not defeated" } else { "defeated" },
    );
    let name = format!("{}: defeated", level.name());
    let response = icons::icon_toggle_named(ui, Icon::Boss(boss), &name, defeated, true, ICON)
        .on_hover_text(tip);
    if response.clicked() {
        let edit = Edit::SetBossDefeated {
            boss: level,
            on: !defeated,
            skip_cutscenes: false,
        };
        push(actions, doc, edit);
    }

    let ability = ABILITIES[boss as usize];
    let on = save.ability(ability);
    let tip = format!(
        "{}: {}. Abilities are stored separately and work even if the boss isn't beaten. \
         Click to turn it {}.",
        ability.name(),
        if on { "on" } else { "off" },
        if on { "off" } else { "on" },
    );
    let name = format!("{}: {}", level.name(), ability.name());
    let response = icons::icon_toggle_named(ui, Icon::Ability(ability), &name, on, true, ICON)
        .on_hover_text(tip);
    if response.clicked() {
        push(actions, doc, Edit::SetAbility { ability, on: !on });
    }
}

/// The levels a colored gem opens a path in, as a phrase: "Dino Might! (on
/// to Eggipus Rex) and Bug Lite".
pub fn colored_gem_paths(color: GemColor) -> String {
    let names: Vec<String> = colored_gem_opens(color)
        .iter()
        .map(|p| match p.leads_to {
            Some(next) => format!("{} (on to {})", p.level.name(), next.name()),
            None => p.level.name().to_owned(),
        })
        .collect();
    match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_cycles_the_relic_tiers() {
        let mut relic = None;
        let mut seen = Vec::new();
        for _ in 0..4 {
            relic = next_relic(relic);
            seen.push(relic);
        }
        assert_eq!(
            seen,
            [
                Some(Relic::Sapphire),
                Some(Relic::Gold),
                Some(Relic::Platinum),
                None
            ]
        );
    }

    /// A row has one slot for the second gem or the colored gem.
    #[test]
    fn no_level_has_both_a_second_and_a_colored_gem() {
        let levels = &nsane::crash3::LEVELS;
        for l in levels {
            assert!(!(l.second_gem && l.colored_gem.is_some()), "{}", l.key);
        }
        let with_other = levels
            .iter()
            .filter(|l| l.second_gem || l.colored_gem.is_some());
        assert_eq!(with_other.count(), 10 + 5);
    }

    #[test]
    fn colored_gem_paths_read_as_a_phrase() {
        assert_eq!(colored_gem_paths(GemColor::Red), "Bone Yard and Bug Lite");
        assert_eq!(
            colored_gem_paths(GemColor::Yellow),
            "Dino Might! (on to Eggipus Rex) and Bug Lite"
        );
    }
}
