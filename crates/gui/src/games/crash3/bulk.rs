//! The two actions that change many things at once: opening warp rooms up to
//! one (from a locked room's padlock) and reaching the relic count of a
//! secret room level (from its padlock). Each opens a dialog that previews
//! its changes before they are applied, as one undo step.

use egui::Context;
use nsane::Document;
use nsane::crash3::presets::{ReachRelicCount, TimePolicy};
use nsane::crash3::tables::SecretRoomLevel;
use nsane::crash3::{ABILITIES, BOSSES, Crash3Save, Edit, Relic, presets};

use super::preview::{self, Previewed};
use crate::app::Action;
use crate::dialog::{self, Dialog, and_list, note, option_group, plural};

/// The width a bulk dialog keeps to.
const DIALOG_WIDTH: f32 = 460.0;

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

    fn title(&self) -> String {
        match self {
            Bulk::Unlock { room, .. } => format!("Unlock up to Warp Room {room}"),
            Bulk::Relics { number, .. } => {
                format!("Open the secret warp room up to level {number}")
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

/// The open bulk dialog, if any.
#[derive(Debug, Default)]
pub struct BulkUi {
    dialog: Option<BulkDialog>,
}

impl BulkUi {
    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn open(&mut self, bulk: Bulk) {
        self.dialog = Some(BulkDialog {
            bulk,
            computed: None,
            #[cfg(test)]
            runs: 0,
        });
    }

    /// How many previews the open dialog has computed.
    #[cfg(test)]
    pub fn preview_runs(&self) -> Option<usize> {
        self.dialog.as_ref().map(|d| d.runs)
    }

    pub fn show(&mut self, ctx: &Context, doc: &Document, actions: &mut Vec<Action>) {
        let Some(open) = &mut self.dialog else {
            return;
        };
        let ran = Previewed::refresh(&mut open.computed, doc, &open.bulk, |save| {
            Ok(open.bulk.edits(save))
        });
        #[cfg(test)]
        {
            open.runs += usize::from(ran);
        }
        #[cfg(not(test))]
        let _ = ran;
        let Some(computed) = &open.computed else {
            return;
        };

        let mut bulk = open.bulk;
        let mut close = false;
        let mut apply = false;
        let shown = Dialog::new("bulk", bulk.title()).width(DIALOG_WIDTH).show(
            ctx,
            |ui| {
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
                let can_apply = preview::show(ui, doc, computed);
                if let Bulk::Unlock { .. } = computed.key {
                    note(ui, "Each defeated boss counts 1% towards the percentage.");
                }
                can_apply
            },
            |ui, &can_apply| {
                if dialog::primary(ui, "Apply", can_apply, None).clicked() {
                    apply = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            },
        );
        close |= shown.cancelled;
        // The options changed this frame: the preview no longer matches them.
        if bulk != open.bulk {
            open.bulk = bulk;
            ctx.request_repaint();
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

fn unlock_options(
    ui: &mut egui::Ui,
    room: u8,
    with_abilities: &mut bool,
    skip_cutscenes: &mut bool,
) {
    let n = usize::from(room) - 1;
    let bosses: Vec<&str> = BOSSES[..n].iter().map(|b| b.name()).collect();
    let powers: Vec<&str> = ABILITIES[..n].iter().map(|a| a.name()).collect();
    dialog::wrapped(
        ui,
        format!(
            "Marks {} as defeated, so that warp rooms 2 to {room} open.",
            and_list(&bosses)
        ),
    );
    dialog::section(ui, "Options");
    option_group(ui, |ui| {
        ui.checkbox(
            with_abilities,
            "Also grant the abilities of the defeated bosses",
        )
        .on_hover_text(and_list(&powers));
        ui.checkbox(skip_cutscenes, "Skip their story scenes")
            .on_hover_text(
                "Marks the hub scenes up to each of these bosses as seen. Scenes not skipped \
                 play later, which is harmless.",
            );
    });
}

fn relic_options(
    ui: &mut egui::Ui,
    doc: &Document,
    number: u8,
    count: u32,
    options: &mut ReachRelicCount,
) {
    let relics = doc.current().totals().relics();
    dialog::wrapped(
        ui,
        format!(
            "Level {number} opens with {count} relics, and the save has {relics}. The secret \
             room only checks the relic count, so this gives a relic to levels that have none, \
             in level order, until the save has {count}."
        ),
    );
    dialog::section(ui, "Relic");
    option_group(ui, |ui| {
        ui.horizontal(|ui| {
            for relic in Relic::ALL {
                ui.radio_value(&mut options.tier, relic, relic.name());
            }
        });
    });
    dialog::section(ui, "Times");
    option_group(ui, |ui| {
        ui.radio_value(
            &mut options.times,
            TimePolicy::Matching,
            "Write a time just under the relic's threshold",
        );
        ui.radio_value(&mut options.times, TimePolicy::Keep, "Keep the times");
        ui.radio_value(&mut options.times, TimePolicy::Clear, "Clear the times");
    });
}

#[cfg(test)]
mod tests {
    use nsane::crash3::SECRET_ROOM;

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
    /// document, for every option of both dialogs.
    #[test]
    fn the_preview_matches_the_applied_changes() {
        for rel in [
            "tests/saves/game/0019.sav",
            "tests/saves/game/initial_game.sav",
            "tests/saves/game/ALCHEMYXSAVE_8.sav",
        ] {
            let fresh = open(rel);
            let save = fresh.current();
            let mut cases: Vec<Bulk> = Vec::new();
            for room in 2..=5 {
                for (with_abilities, skip_cutscenes) in
                    [(true, true), (false, false), (true, false)]
                {
                    cases.push(Bulk::Unlock {
                        room,
                        with_abilities,
                        skip_cutscenes,
                    });
                }
            }
            for e in SECRET_ROOM {
                for tier in Relic::ALL {
                    for times in [TimePolicy::Matching, TimePolicy::Keep, TimePolicy::Clear] {
                        cases.push(Bulk::Relics {
                            number: e.number,
                            count: u32::from(e.relics),
                            options: ReachRelicCount { tier, times },
                        });
                    }
                }
            }
            for bulk in cases {
                let name = format!("{bulk:?}");
                let edits = bulk.edits(save);
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
}
