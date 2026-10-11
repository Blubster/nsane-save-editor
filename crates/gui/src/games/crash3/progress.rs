//! The Progress tab: the story-level edits. Warp rooms opened early, the
//! secret warp room's relic thresholds, the super powers and the hub story
//! scenes.
//!
//! The two actions that change many things at once, opening warp rooms and
//! reaching a relic count, open a dialog that previews their changes before
//! they are applied. Every action is one undo step.

use egui::{
    Align, Button, Grid, Id, Key, Layout, Modal, RichText, ScrollArea, Ui, WidgetInfo, WidgetType,
};
use nsane::Document;
use nsane::crash3::presets::{ReachRelicCount, TimePolicy, all_abilities, all_cutscenes};
use nsane::crash3::tables::SecretRoomLevel;
use nsane::crash3::{
    ABILITIES, BOSSES, CUTSCENES, Crash3Save, Edit, LevelId, Relic, SECRET_ROOM, presets,
};

use super::level_row::{self, push};
use super::preview::{self, Previewed, plural};
use super::rooms;
use crate::app::{Action, static_icon};
use crate::icons::{self, Boss, Icon};
use crate::theme::{self, SPACING};

/// The width a modal dialog keeps to.
const DIALOG_WIDTH: f32 = 460.0;
/// The scene name column: fits "Cortex Post NTropy Fight".
const SCENE_NAME_WIDTH: f32 = 170.0;

/// An action on many things, waiting in its dialog for confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bulk {
    /// Open warp rooms up to `room` (2–5).
    Unlock {
        room: u8,
        with_abilities: bool,
        skip_cutscenes: bool,
    },
    /// Open the secret room up to its level `number`, which needs `count`
    /// relics.
    Relics {
        number: u8,
        count: u32,
        options: ReachRelicCount,
    },
}

impl Bulk {
    /// Unlocking up to `room`, with the abilities and without the scenes of
    /// the bosses it defeats.
    pub fn unlock(room: u8) -> Self {
        Bulk::Unlock {
            room,
            with_abilities: true,
            skip_cutscenes: true,
        }
    }

    /// Reaching the relic count of a secret room level, with the preset's
    /// defaults.
    pub fn relics(entry: SecretRoomLevel) -> Self {
        Bulk::Relics {
            number: entry.number,
            count: u32::from(entry.relics),
            options: ReachRelicCount::default(),
        }
    }

    pub fn edits(&self, save: &Crash3Save) -> Vec<Edit> {
        match *self {
            Bulk::Unlock {
                room,
                with_abilities,
                skip_cutscenes,
            } => vec![Edit::UnlockWarpRoom {
                room,
                with_abilities,
                skip_cutscenes,
            }],
            Bulk::Relics { count, options, .. } => {
                presets::reach_relic_count(save, count, &options)
            }
        }
    }

    /// The undo label of `edits`, the edits of this action.
    pub fn label(&self, save: &Crash3Save, edits: &[Edit]) -> String {
        match self {
            Bulk::Unlock { .. } => edits
                .first()
                .map_or_else(String::new, |edit| edit.describe(save)),
            Bulk::Relics { count, options, .. } => {
                let levels = edits
                    .iter()
                    .filter(|e| matches!(e, Edit::SetRelic { .. }))
                    .count();
                format!(
                    "Reach {count} relics: {} on {}",
                    options.tier.name().to_lowercase(),
                    plural(levels, "level", "levels")
                )
            }
        }
    }
}

#[derive(Debug)]
struct BulkDialog {
    bulk: Bulk,
    computed: Option<Previewed<Bulk>>,
    /// How many previews were computed, for the tests.
    #[cfg(test)]
    runs: usize,
}

/// What the Progress tab keeps between frames.
#[derive(Debug, Default)]
pub struct Progress {
    dialog: Option<BulkDialog>,
    /// Bring the story scenes into view on the next frame.
    scroll_to_scenes: bool,
}

impl Progress {
    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn open_dialog(&mut self, bulk: Bulk) {
        self.dialog = Some(BulkDialog {
            bulk,
            computed: None,
            #[cfg(test)]
            runs: 0,
        });
    }

    pub fn scroll_to_scenes(&mut self) {
        self.scroll_to_scenes = true;
    }

    /// How many previews the open dialog has computed.
    #[cfg(test)]
    pub fn preview_runs(&self) -> Option<usize> {
        self.dialog.as_ref().map(|d| d.runs)
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        doc: &Document,
        selected: &mut Option<LevelId>,
        actions: &mut Vec<Action>,
    ) {
        let mut open = None;
        let scroll = &mut self.scroll_to_scenes;
        ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            let first = |ui: &mut Ui, selected: &mut _, actions: &mut _, open: &mut _| {
                rooms::card(ui, |ui| warp_rooms(ui, doc, selected, actions, open));
                ui.add_space(rooms::CARD_GAP);
                rooms::card(ui, |ui| secret_room(ui, doc, open));
            };
            let second = |ui: &mut Ui, actions: &mut _, scroll: &mut _| {
                rooms::card(ui, |ui| abilities(ui, doc, actions));
                ui.add_space(rooms::CARD_GAP);
                rooms::card(ui, |ui| cutscenes(ui, doc, actions, scroll));
            };
            if rooms::columns_for(ui.available_width()) >= 2 {
                ui.columns(2, |cols| {
                    first(&mut cols[0], selected, actions, &mut open);
                    second(&mut cols[1], actions, scroll);
                });
            } else {
                first(ui, selected, actions, &mut open);
                ui.add_space(rooms::CARD_GAP);
                second(ui, actions, scroll);
            }
        });
        if let Some(bulk) = open {
            self.open_dialog(bulk);
        }
        self.dialog(ui, doc, actions);
    }

    fn dialog(&mut self, ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>) {
        let Some(dialog) = &mut self.dialog else {
            return;
        };
        let ran = Previewed::refresh(&mut dialog.computed, doc, &dialog.bulk, |save| {
            Ok(dialog.bulk.edits(save))
        });
        #[cfg(test)]
        {
            dialog.runs += usize::from(ran);
        }
        #[cfg(not(test))]
        let _ = ran;
        let Some(computed) = &dialog.computed else {
            return;
        };

        let mut bulk = dialog.bulk;
        let mut close = false;
        let mut apply = false;
        Modal::new(Id::new("progress-bulk")).show(ui.ctx(), |ui| {
            ui.set_max_width(DIALOG_WIDTH);
            match &mut bulk {
                Bulk::Unlock {
                    room,
                    with_abilities,
                    skip_cutscenes,
                } => unlock_options(ui, *room, with_abilities, skip_cutscenes),
                Bulk::Relics {
                    number,
                    count,
                    options,
                } => relic_options(ui, doc, *number, *count, options),
            }
            ui.separator();
            let can_apply = preview::show(ui, doc, computed);
            if let Bulk::Unlock { .. } = computed.key {
                note(ui, "Each defeated boss counts 1% towards the percentage.");
            }
            ui.add_space(SPACING);
            ui.horizontal(|ui| {
                if ui.add_enabled(can_apply, Button::new("Apply")).clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    close = true;
                }
            });
        });
        // The options changed this frame: the preview no longer matches them.
        if bulk != dialog.bulk {
            dialog.bulk = bulk;
            ui.ctx().request_repaint();
        } else if apply {
            actions.push(Action::Apply {
                label: computed.key.label(doc.current(), &computed.edits),
                edits: computed.edits.clone(),
            });
            close = true;
        }
        if close {
            self.dialog = None;
        }
    }
}

/// "a, b and c".
fn and_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

fn title(ui: &mut Ui, text: &str) -> egui::Response {
    ui.label(RichText::new(text).strong().size(theme::FONT_BODY + 2.0))
}

/// A note in the weak colour, wrapped to the width.
fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).weak()).wrap());
}

// ------------------------------------------------------------- warp rooms

fn warp_rooms(
    ui: &mut Ui,
    doc: &Document,
    selected: &mut Option<LevelId>,
    actions: &mut Vec<Action>,
    open: &mut Option<Bulk>,
) {
    let save = doc.current();
    title(ui, "Warp rooms");
    note(
        ui,
        "Warp Room 1 is open from the start. Defeating a room's boss opens the next room.",
    );
    ui.add_space(SPACING / 2.0);
    Grid::new("progress-rooms")
        .num_columns(3)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            for (room, boss) in (1u8..).zip(BOSSES) {
                let is_open = rooms::room_open(doc, room);
                ui.horizontal(|ui| {
                    let lock = if is_open { Icon::Unlock } else { Icon::Lock };
                    let tip = match room {
                        1 => "Open from the start".to_owned(),
                        _ => {
                            let before = BOSSES[usize::from(room) - 2].name();
                            if is_open {
                                format!("Open: {before} is defeated")
                            } else {
                                format!("Locked: opens when {before} is defeated")
                            }
                        }
                    };
                    static_icon(ui, lock, theme::ICON_SMALL).on_hover_text(tip);
                    ui.label(format!("Warp Room {room}"));
                });
                ui.horizontal(|ui| {
                    boss_toggle(ui, doc, boss, actions);
                    let defeated = save.boss_defeated(boss);
                    let text = RichText::new(boss.name());
                    let text = if defeated {
                        text
                    } else {
                        text.color(theme::palette(ui.visuals().dark_mode).muted)
                    };
                    if ui
                        .add(Button::selectable(*selected == Some(boss), text))
                        .on_hover_text(format!("Show the details of {}", boss.name()))
                        .clicked()
                    {
                        *selected = Some(boss);
                    }
                });
                if is_open {
                    ui.weak("open");
                } else {
                    let before: Vec<&str> = BOSSES[..usize::from(room) - 1]
                        .iter()
                        .map(|b| b.name())
                        .collect();
                    let name = format!("Unlock up to room {room}…");
                    let response = ui.button("Unlock…").on_hover_text(format!(
                        "Unlock up to room {room}: mark {} as defeated. Shows the changes \
                         before applying them.",
                        and_list(&before)
                    ));
                    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
                    if response.clicked() {
                        *open = Some(Bulk::unlock(room));
                    }
                }
                ui.end_row();
            }
        });
    ui.add_space(SPACING / 2.0);
    note(
        ui,
        "To lock a room again, mark the boss before it as not defeated, with its toggle here \
         or in the Levels tab. The abilities stay as they are.",
    );
}

/// The defeat of a boss, as in the warp room grid.
fn boss_toggle(ui: &mut Ui, doc: &Document, boss: LevelId, actions: &mut Vec<Action>) {
    let Some(icon) = Boss::of(boss) else {
        return;
    };
    let defeated = doc.current().boss_defeated(boss);
    let opens = match boss.level().warp_room {
        5 => "Ends the story.".to_owned(),
        room => format!("Opens warp room {}.", room + 1),
    };
    let tip = format!(
        "{}: {}. {opens} Click to mark {}.",
        boss.name(),
        if defeated { "defeated" } else { "not defeated" },
        if defeated { "not defeated" } else { "defeated" },
    );
    let name = format!("{}: defeated", boss.name());
    let response =
        icons::icon_toggle_named(ui, Icon::Boss(icon), &name, defeated, true, level_row::ICON)
            .on_hover_text(tip);
    if response.clicked() {
        let edit = Edit::SetBossDefeated {
            boss,
            on: !defeated,
            skip_cutscenes: false,
        };
        push(actions, doc, edit);
    }
}

fn unlock_options(ui: &mut Ui, room: u8, with_abilities: &mut bool, skip_cutscenes: &mut bool) {
    let n = usize::from(room) - 1;
    let bosses: Vec<&str> = BOSSES[..n].iter().map(|b| b.name()).collect();
    let powers: Vec<&str> = ABILITIES[..n].iter().map(|a| a.name()).collect();
    ui.heading(format!("Unlock up to Warp Room {room}"));
    ui.add_space(SPACING);
    ui.add(
        egui::Label::new(format!(
            "Marks {} as defeated, so that warp rooms 2 to {room} open.",
            and_list(&bosses)
        ))
        .wrap(),
    );
    ui.add_space(SPACING / 2.0);
    ui.checkbox(
        with_abilities,
        "Also grant the abilities of the defeated bosses",
    )
    .on_hover_text(and_list(&powers));
    ui.checkbox(skip_cutscenes, "Skip their story scenes")
        .on_hover_text(
            "Marks the hub scenes up to each of these bosses as seen. Scenes not skipped play \
             later, which is harmless.",
        );
}

// ------------------------------------------------------- secret warp room

fn secret_room(ui: &mut Ui, doc: &Document, open: &mut Option<Bulk>) {
    let relics = doc.current().totals().relics();
    title(ui, "Secret warp room");
    ui.label(RichText::new(plural(relics as usize, "relic", "relics")).strong())
        .on_hover_text("Relics of any tier, counted from the save as it is now");
    note(
        ui,
        "Its levels open with the total relic count, of any tier. Nothing else is checked.",
    );
    ui.add_space(SPACING / 2.0);
    Grid::new("progress-secret")
        .num_columns(3)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            for entry in SECRET_ROOM {
                let is_open = relics >= u32::from(entry.relics);
                ui.horizontal(|ui| {
                    let lock = if is_open { Icon::Unlock } else { Icon::Lock };
                    let tip = format!(
                        "{}: opens with {} relics",
                        if is_open { "Open" } else { "Locked" },
                        entry.relics
                    );
                    static_icon(ui, lock, theme::ICON_SMALL).on_hover_text(tip);
                    let alt = if entry.level.level().warp_room == 6 {
                        ""
                    } else {
                        " (alt.)"
                    };
                    ui.label(format!("{} {}{alt}", entry.number, entry.level.name()));
                });
                ui.weak(format!("{} relics", entry.relics));
                if is_open {
                    ui.weak("open");
                } else if ui
                    .button(format!("Open up to {}…", entry.number))
                    .on_hover_text(format!(
                        "Give relics to levels that have none until the save has {}. Shows the \
                         changes before applying them.",
                        entry.relics
                    ))
                    .clicked()
                {
                    *open = Some(Bulk::relics(entry));
                }
                ui.end_row();
            }
        });
}

fn relic_options(
    ui: &mut Ui,
    doc: &Document,
    number: u8,
    count: u32,
    options: &mut ReachRelicCount,
) {
    let relics = doc.current().totals().relics();
    ui.heading(format!("Open the secret warp room up to level {number}"));
    ui.add_space(SPACING);
    ui.add(
        egui::Label::new(format!(
            "Level {number} opens with {count} relics, and the save has {relics}. The secret \
             room only checks the relic count, so this gives a relic to levels that have none, \
             in level order, until the save has {count}."
        ))
        .wrap(),
    );
    ui.add_space(SPACING / 2.0);
    ui.horizontal(|ui| {
        ui.label("Relic");
        for relic in Relic::ALL {
            ui.radio_value(&mut options.tier, relic, relic.name());
        }
    });
    ui.label("Times");
    ui.indent("times", |ui| {
        ui.radio_value(
            &mut options.times,
            TimePolicy::Matching,
            "Write a time just under the relic's threshold",
        );
        ui.radio_value(&mut options.times, TimePolicy::Keep, "Keep the times");
        ui.radio_value(&mut options.times, TimePolicy::Clear, "Clear the times");
    });
}

// -------------------------------------------------------------- abilities

fn abilities(ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>) {
    let save = doc.current();
    title(ui, "Super powers");
    ui.add_space(SPACING / 2.0);
    for (ability, boss) in ABILITIES.into_iter().zip(BOSSES) {
        let on = save.ability(ability);
        ui.horizontal(|ui| {
            let tip = format!(
                "{}: {}. Click to turn it {}.",
                ability.name(),
                if on { "on" } else { "off" },
                if on { "off" } else { "on" },
            );
            let name = format!("Super power: {}", ability.name());
            let response = icons::icon_toggle_named(
                ui,
                Icon::Ability(ability),
                &name,
                on,
                true,
                theme::ICON_LARGE,
            )
            .on_hover_text(tip);
            if response.clicked() {
                push(actions, doc, Edit::SetAbility { ability, on: !on });
            }
            ui.vertical(|ui| {
                ui.strong(ability.name());
                ui.weak(format!("Normally granted by {}", boss.name()));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(if on { "on" } else { "off" });
            });
        });
    }
    ui.add_space(SPACING / 2.0);
    ui.horizontal(|ui| {
        for on in [true, false] {
            let change: Vec<&str> = ABILITIES
                .iter()
                .filter(|&&a| save.ability(a) != on)
                .map(|a| a.name())
                .collect();
            let (text, state) = if on {
                ("All on", "on")
            } else {
                ("All off", "off")
            };
            let response = ui
                .add_enabled(!change.is_empty(), Button::new(text))
                .on_disabled_hover_text(format!("Every super power is already {state}"))
                .on_hover_text(format!("Turns {state}: {}", and_list(&change)));
            if response.clicked() {
                actions.push(Action::Apply {
                    label: format!("All super powers: {state}"),
                    edits: all_abilities(on),
                });
            }
        }
    });
    note(
        ui,
        "Abilities work even if the boss isn't beaten (verified in game).",
    );
}

// -------------------------------------------------------------- cutscenes

/// The scene name without the game's prefix, words apart:
/// `C3_Tiny_PreBossFight` is "Tiny PreBossFight".
pub fn scene_name(scene: &str) -> String {
    scene.strip_prefix("C3_").unwrap_or(scene).replace('_', " ")
}

/// The scenes of a step that a "mark all" would change.
fn to_change<'a>(save: &Crash3Save, scenes: &[&'a str], seen: bool) -> Vec<&'a str> {
    scenes
        .iter()
        .copied()
        .filter(|s| save.cutscene_seen(s) != seen)
        .collect()
}

fn cutscenes(ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>, scroll: &mut bool) {
    let save = doc.current();
    let response = title(ui, "Story scenes");
    if std::mem::take(scroll) {
        // With the card's margin, so that the title isn't cut.
        ui.scroll_to_rect(response.rect.expand(SPACING * 1.5), Some(Align::TOP));
    }
    note(
        ui,
        "A scene marked as seen doesn't play. Story scenes you skip by unlocking rooms early \
         will otherwise play later, which is harmless.",
    );
    let every: Vec<&str> = CUTSCENES.iter().flat_map(|s| s.scenes).copied().collect();
    let unseen = to_change(save, &every, true);
    let response = ui
        .add_enabled(!unseen.is_empty(), Button::new("Skip all story scenes"))
        .on_disabled_hover_text("Every story scene is already marked as seen")
        .on_hover_text(format!(
            "Marks {} as seen",
            plural(unseen.len(), "scene", "scenes")
        ));
    if response.clicked() {
        actions.push(Action::Apply {
            label: "All story scenes: seen".to_owned(),
            edits: all_cutscenes(true),
        });
    }

    for step in &CUTSCENES {
        let step_name = step.boss().map_or("Start", |b| b.name());
        ui.add_space(SPACING);
        ui.horizontal(|ui| {
            let seen = step.scenes.len() - to_change(save, step.scenes, true).len();
            ui.strong(step_name);
            ui.weak(format!("{seen} / {} seen", step.scenes.len()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for (seen, text) in [(false, "Mark all unseen"), (true, "Mark all seen")] {
                    let change = to_change(save, step.scenes, seen);
                    let state = if seen { "seen" } else { "not seen" };
                    let names: Vec<String> = change.iter().map(|s| scene_name(s)).collect();
                    let names: Vec<&str> = names.iter().map(String::as_str).collect();
                    let response = ui
                        .add_enabled(!change.is_empty(), Button::new(text).small())
                        .on_disabled_hover_text(format!("Every scene is already {state}"))
                        .on_hover_text(format!("Marks as {state}: {}", and_list(&names)));
                    let label = format!("{step_name}: {}", text.to_lowercase());
                    response.widget_info(|| {
                        WidgetInfo::labeled(WidgetType::Button, !change.is_empty(), &label)
                    });
                    if response.clicked() {
                        let edits = change
                            .iter()
                            .map(|&s| Edit::SetCutscene {
                                cutscene: s.to_owned(),
                                seen,
                            })
                            .collect();
                        actions.push(Action::Apply {
                            label: format!("{step_name} scenes: {state}"),
                            edits,
                        });
                    }
                }
            });
        });
        Grid::new(("progress-scenes", step.step))
            .num_columns(2)
            .spacing([12.0, 2.0])
            // The same for every step, so that the checkboxes line up.
            .min_col_width(SCENE_NAME_WIDTH)
            .show(ui, |ui| {
                for &scene in step.scenes {
                    let name = scene_name(scene);
                    ui.label(&name).on_hover_text(scene);
                    let mut seen = save.cutscene_seen(scene);
                    let response = ui.checkbox(&mut seen, "Seen (won't play)");
                    response.widget_info(|| {
                        WidgetInfo::selected(
                            WidgetType::Checkbox,
                            true,
                            seen,
                            format!("{name}: seen"),
                        )
                    });
                    if response.changed() {
                        let edit = Edit::SetCutscene {
                            cutscene: scene.to_owned(),
                            seen,
                        };
                        push(actions, doc, edit);
                    }
                    ui.end_row();
                }
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::repo_file;

    fn open(rel: &str) -> Document {
        let bytes = std::fs::read(repo_file(rel)).unwrap();
        Document::open(&bytes, None).unwrap()
    }

    fn entry(number: u8) -> SecretRoomLevel {
        SECRET_ROOM
            .into_iter()
            .find(|e| e.number == number)
            .unwrap()
    }

    #[test]
    fn the_unlock_dialog_emits_one_edit_with_its_options() {
        let save = open("tests/saves/game/0019.sav");
        let edits = Bulk::unlock(4).edits(save.current());
        assert_eq!(
            edits,
            [Edit::UnlockWarpRoom {
                room: 4,
                with_abilities: true,
                skip_cutscenes: true,
            }]
        );
        assert_eq!(
            Bulk::unlock(4).label(save.current(), &edits),
            "Unlock warp room 4 (+abilities, skip scenes)"
        );
    }

    #[test]
    fn the_secret_room_dialog_uses_the_preset_defaults() {
        let doc = open("tests/saves/game/0019.sav");
        let bulk = Bulk::relics(entry(29));
        let Bulk::Relics { count, options, .. } = bulk else {
            unreachable!()
        };
        assert_eq!(count, 20);
        assert_eq!(options, ReachRelicCount::default());
        assert_eq!(options.tier, Relic::Sapphire);
        assert_eq!(options.times, TimePolicy::Matching);
        let edits = bulk.edits(doc.current());
        assert_eq!(
            edits,
            presets::reach_relic_count(doc.current(), 20, &options)
        );
        // 0019 has 1 relic: 19 levels get one.
        assert_eq!(
            bulk.label(doc.current(), &edits),
            "Reach 20 relics: sapphire on 19 levels"
        );
    }

    /// What a dialog previews is what applying it changes, on a fresh
    /// document, for every bulk action of the tab.
    #[test]
    fn the_preview_matches_the_applied_changes() {
        for rel in [
            "tests/saves/game/0019.sav",
            "tests/saves/game/initial_game.sav",
            "tests/saves/game/ALCHEMYXSAVE_8.sav",
        ] {
            let fresh = open(rel);
            let save = fresh.current();
            let mut cases: Vec<(String, Vec<Edit>)> = vec![
                ("abilities".into(), all_abilities(true)),
                ("scenes".into(), all_cutscenes(true)),
            ];
            for room in 2..=5 {
                for (with_abilities, skip_cutscenes) in
                    [(true, true), (false, false), (true, false)]
                {
                    let bulk = Bulk::Unlock {
                        room,
                        with_abilities,
                        skip_cutscenes,
                    };
                    cases.push((format!("{bulk:?}"), bulk.edits(save)));
                }
            }
            for e in SECRET_ROOM {
                for tier in Relic::ALL {
                    for times in [TimePolicy::Matching, TimePolicy::Keep, TimePolicy::Clear] {
                        let bulk = Bulk::Relics {
                            number: e.number,
                            count: u32::from(e.relics),
                            options: ReachRelicCount { tier, times },
                        };
                        cases.push((format!("{bulk:?}"), bulk.edits(save)));
                    }
                }
            }
            for (name, edits) in cases {
                let preview = fresh.preview(&edits).unwrap();
                let mut doc = fresh.clone();
                doc.apply(name.clone(), edits).unwrap();
                assert_eq!(preview.changes, doc.changes(), "{rel} {name}");
                assert_eq!(preview.totals, doc.current().totals(), "{rel} {name}");
            }
        }
    }

    #[test]
    fn opening_up_to_29_gives_20_relics() {
        let mut doc = open("tests/saves/game/0019.sav");
        let edits = Bulk::relics(entry(29)).edits(doc.current());
        doc.apply("reach", edits).unwrap();
        assert_eq!(doc.current().totals().relics(), 20);
    }

    #[test]
    fn scene_names_drop_the_prefix() {
        assert_eq!(scene_name("C3_Tiny_PreBossFight"), "Tiny PreBossFight");
        assert_eq!(scene_name("C3_PreNGinFight"), "PreNGinFight");
        assert_eq!(scene_name("Other"), "Other");
        assert_eq!(and_list(&["a", "b", "c"]), "a, b and c");
        assert_eq!(and_list(&["a"]), "a");
    }
}
