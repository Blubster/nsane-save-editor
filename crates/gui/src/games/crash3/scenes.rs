//! The story scenes dialog, opened from the overview: the hub scenes step by
//! step, from the start to the last boss, each marked as seen or not. A
//! scene marked as seen doesn't play. Every click is one undo step, applied
//! at once; the dialog stays open until it is closed.

use egui::{Align, Button, Context, Grid, Layout, Ui, WidgetInfo, WidgetType};
use nsane::Document;
use nsane::crash3::presets::all_cutscenes;
use nsane::crash3::{CUTSCENES, Crash3Save, Edit};

use super::level_row::push;
use crate::app::Action;
use crate::dialog::{self, Dialog, and_list, note, plural};
use crate::theme::{self, SPACING};

/// The width the dialog keeps to.
const DIALOG_WIDTH: f32 = 480.0;
/// A column of scenes: fits "Cortex Post NTropy Fight" and its checkbox.
const SCENE_WIDTH: f32 = 200.0;

/// What the dialog keeps between frames.
#[derive(Debug, Default)]
pub struct Scenes {
    pub open: bool,
}

/// The scene name without the game's prefix, words apart:
/// `C3_Tiny_PreBossFight` is "Tiny PreBossFight".
pub fn scene_name(scene: &str) -> String {
    scene.strip_prefix("C3_").unwrap_or(scene).replace('_', " ")
}

/// How many story scenes are marked as seen, and how many there are.
pub fn seen_count(save: &Crash3Save) -> (usize, usize) {
    let every = CUTSCENES.iter().flat_map(|s| s.scenes);
    let total = every.clone().count();
    (every.filter(|s| save.cutscene_seen(s)).count(), total)
}

/// The scenes of a step that a "mark all" would change.
fn to_change<'a>(save: &Crash3Save, scenes: &[&'a str], seen: bool) -> Vec<&'a str> {
    scenes
        .iter()
        .copied()
        .filter(|s| save.cutscene_seen(s) != seen)
        .collect()
}

impl Scenes {
    pub fn has_dialog(&self) -> bool {
        self.open
    }

    pub fn show(&mut self, ctx: &Context, doc: &Document, actions: &mut Vec<Action>) {
        if !self.open {
            return;
        }
        let mut close = false;
        let shown = Dialog::new("scenes", "Story scenes")
            .subtitle("A scene marked as seen doesn't play.")
            .width(DIALOG_WIDTH)
            .show(
                ctx,
                |ui| body(ui, doc, actions),
                |ui, _| {
                    if dialog::primary(ui, "Close", true, None).clicked() {
                        close = true;
                    }
                },
            );
        if close || shown.cancelled {
            self.open = false;
        }
    }
}

fn body(ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>) {
    let save = doc.current();
    note(
        ui,
        "Scenes skipped by unlocking warp rooms early otherwise play later, which is harmless.",
    );
    ui.add_space(SPACING / 2.0);
    let every: Vec<&str> = CUTSCENES.iter().flat_map(|s| s.scenes).copied().collect();
    let unseen = to_change(save, &every, true);
    ui.horizontal(|ui| {
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
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.weak(format!(
                "{} / {} seen",
                every.len() - unseen.len(),
                every.len()
            ));
        });
    });

    let columns = if ui.available_width() >= 2.0 * SCENE_WIDTH + SPACING {
        2
    } else {
        1
    };
    for step in &CUTSCENES {
        let step_name = step.boss().map_or("Start", |b| b.name());
        ui.add_space(SPACING);
        ui.horizontal(|ui| {
            let seen = step.scenes.len() - to_change(save, step.scenes, true).len();
            ui.label(theme::bold(step_name));
            ui.weak(format!("{seen} / {} seen", step.scenes.len()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for (seen, text) in [(false, "Mark all unseen"), (true, "Mark all seen")] {
                    mark_all(ui, doc, step_name, step.scenes, seen, text, actions);
                }
            });
        });
        Grid::new(("scenes", step.step))
            .num_columns(columns)
            .spacing([SPACING, 2.0])
            // The same for every step, so that the columns line up.
            .min_col_width(SCENE_WIDTH)
            .show(ui, |ui| {
                for (i, &scene) in step.scenes.iter().enumerate() {
                    scene_checkbox(ui, doc, scene, actions);
                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });
    }
}

/// "Mark all seen" or "Mark all unseen" for the scenes of a step.
fn mark_all(
    ui: &mut Ui,
    doc: &Document,
    step_name: &str,
    scenes: &[&str],
    seen: bool,
    text: &str,
    actions: &mut Vec<Action>,
) {
    let change = to_change(doc.current(), scenes, seen);
    let state = if seen { "seen" } else { "not seen" };
    let names: Vec<String> = change.iter().map(|s| scene_name(s)).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let response = ui
        .add_enabled(!change.is_empty(), Button::new(text).small())
        .on_disabled_hover_text(format!("Every scene is already {state}"))
        .on_hover_text(format!("Marks as {state}: {}", and_list(&names)));
    let label = format!("{step_name}: {}", text.to_lowercase());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, !change.is_empty(), &label));
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

/// A scene, checked when it is marked as seen. The tooltip gives the key
/// the save stores.
fn scene_checkbox(ui: &mut Ui, doc: &Document, scene: &str, actions: &mut Vec<Action>) {
    let name = scene_name(scene);
    let mut seen = doc.current().cutscene_seen(scene);
    let response = ui.checkbox(&mut seen, &name).on_hover_text(scene);
    response.widget_info(|| {
        WidgetInfo::selected(WidgetType::Checkbox, true, seen, format!("{name}: seen"))
    });
    if response.changed() {
        let edit = Edit::SetCutscene {
            cutscene: scene.to_owned(),
            seen,
        };
        push(actions, doc, edit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::repo_file;

    #[test]
    fn scene_names_drop_the_prefix() {
        assert_eq!(scene_name("C3_Tiny_PreBossFight"), "Tiny PreBossFight");
        assert_eq!(scene_name("C3_PreNGinFight"), "PreNGinFight");
        assert_eq!(scene_name("Other"), "Other");
    }

    #[test]
    fn the_seen_count_covers_the_18_scenes() {
        let bytes = std::fs::read(repo_file("tests/saves/game/0019.sav")).unwrap();
        let mut doc = Document::open(&bytes, None).unwrap();
        let (seen, total) = seen_count(doc.current());
        assert_eq!(total, 18);
        assert!(seen < total);
        doc.apply("skip", all_cutscenes(true)).unwrap();
        assert_eq!(seen_count(doc.current()), (18, 18));
    }
}
