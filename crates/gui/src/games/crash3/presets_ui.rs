//! The Presets menu: common goals applied in one step. Each entry opens a
//! dialog with the preset's options, pre-filled with its defaults, and a
//! live preview of what it changes. Apply is one undo step, labelled with
//! the preset and its options.

use egui::{RichText, Ui};
use nsane::Document;
use nsane::crash3::presets::{
    self, AllRelics, CompleteAll, DEFAULT_ABILITIES_ON, DEFAULT_ALL_RELICS_TIER,
    DEFAULT_CUTSCENES_SEEN, StoryUpTo, TimePolicy,
};
use nsane::crash3::{ABILITIES, BOSSES, Crash3Save, Edit, EditError, Relic, format_time};

use super::level_panel::parse_slot;
use super::preview::{self, Previewed};
use crate::app::Action;
use crate::dialog::{self, Dialog, option_group, wrapped};
use crate::theme;

/// The width a preset dialog keeps to.
const DIALOG_WIDTH: f32 = 480.0;
/// What the fixed time field starts with.
const DEFAULT_FIXED_TIME: u32 = 6000;

/// A preset with its options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    CompleteAll(CompleteAll),
    StoryUpTo { room: u8, options: StoryUpTo },
    AllRelics { tier: Relic, options: AllRelics },
    AllAbilities { on: bool },
    AllCutscenes { seen: bool },
}

impl Preset {
    /// The menu entries, each with its defaults for `save`.
    pub fn menu(save: &Crash3Save) -> [Preset; 5] {
        [
            Preset::CompleteAll(CompleteAll::default()),
            Preset::StoryUpTo {
                room: presets::default_story_room(save),
                options: StoryUpTo::default(),
            },
            Preset::AllRelics {
                tier: DEFAULT_ALL_RELICS_TIER,
                options: AllRelics::default(),
            },
            Preset::AllAbilities {
                on: DEFAULT_ABILITIES_ON,
            },
            Preset::AllCutscenes {
                seen: DEFAULT_CUTSCENES_SEEN,
            },
        ]
    }

    pub fn menu_title(&self) -> &'static str {
        match self {
            Preset::CompleteAll(_) => "Complete everything (108%)…",
            Preset::StoryUpTo { .. } => "Story up to warp room…",
            Preset::AllRelics { .. } => "All relics at a tier…",
            Preset::AllAbilities { .. } => "All super powers on or off…",
            Preset::AllCutscenes { .. } => "Skip all story scenes…",
        }
    }

    /// The title of its dialog, with the options that change it.
    pub fn title(&self) -> String {
        match self {
            Preset::CompleteAll(_) => "Complete everything (108%)".to_owned(),
            Preset::StoryUpTo { room, .. } => format!("Story up to warp room {room}"),
            Preset::AllRelics { tier, .. } => {
                format!("All relics at {}", tier.name().to_lowercase())
            }
            Preset::AllAbilities { .. } => "All super powers".to_owned(),
            Preset::AllCutscenes { .. } => "All story scenes".to_owned(),
        }
    }

    fn menu_tip(&self) -> &'static str {
        match self {
            Preset::CompleteAll(_) => {
                "Every crystal, gem, relic, boss and super power, and the bonus gem"
            }
            Preset::StoryUpTo { .. } => {
                "The crystals and bosses of the first warp rooms, as if the story was played \
                 that far"
            }
            Preset::AllRelics { .. } => "One relic tier on every level, with or without times",
            Preset::AllAbilities { .. } => "Turn the five super powers on or off",
            Preset::AllCutscenes { .. } => "Mark every hub story scene as seen, or as not seen",
        }
    }

    pub fn edits(&self, save: &Crash3Save) -> Result<Vec<Edit>, EditError> {
        Ok(match self {
            Preset::CompleteAll(options) => presets::complete_all(save, options),
            Preset::StoryUpTo { room, options } => presets::story_up_to(*room, options)?,
            Preset::AllRelics { tier, options } => presets::all_relics(save, *tier, options),
            Preset::AllAbilities { on } => presets::all_abilities(*on),
            Preset::AllCutscenes { seen } => presets::all_cutscenes(*seen),
        })
    }

    /// The undo label: the preset and its options.
    pub fn label(&self) -> String {
        let with = |name: String, extras: Vec<String>| {
            if extras.is_empty() {
                name
            } else {
                format!("{name} ({})", extras.join(", "))
            }
        };
        match self {
            Preset::CompleteAll(o) => {
                let mut extras = vec!["108%".to_owned(), times_text(o.times)];
                if o.skip_cutscenes {
                    extras.push("skip scenes".to_owned());
                }
                with("Complete everything".to_owned(), extras)
            }
            Preset::StoryUpTo { room, options: o } => {
                let extras = [
                    (o.with_gems, "+gems"),
                    (o.with_abilities, "+abilities"),
                    (o.skip_cutscenes, "skip scenes"),
                ]
                .into_iter()
                .filter(|(on, _)| *on)
                .map(|(_, text)| text.to_owned())
                .collect();
                with(format!("Story up to warp room {room}"), extras)
            }
            Preset::AllRelics { tier, options: o } => {
                let mut extras = Vec::new();
                if o.only_upgrade {
                    extras.push("only upgrades".to_owned());
                }
                extras.push(times_text(o.times));
                with(
                    format!("All relics at {}", tier.name().to_lowercase()),
                    extras,
                )
            }
            Preset::AllAbilities { on } => {
                format!("All super powers: {}", if *on { "on" } else { "off" })
            }
            Preset::AllCutscenes { seen } => {
                format!(
                    "All story scenes: {}",
                    if *seen { "seen" } else { "not seen" }
                )
            }
        }
    }

    fn times_mut(&mut self) -> Option<&mut TimePolicy> {
        match self {
            Preset::CompleteAll(o) => Some(&mut o.times),
            Preset::AllRelics { options, .. } => Some(&mut options.times),
            _ => None,
        }
    }
}

fn times_text(policy: TimePolicy) -> String {
    match policy {
        TimePolicy::Keep => "times kept".to_owned(),
        TimePolicy::Matching => "matching times".to_owned(),
        TimePolicy::Fixed(cs) => format!("times {}", format_time(cs)),
        TimePolicy::Clear => "times cleared".to_owned(),
    }
}

#[derive(Debug)]
struct PresetDialog {
    preset: Preset,
    /// The text of the fixed time field, kept while it doesn't parse.
    fixed_text: String,
    computed: Option<Previewed<Preset>>,
    /// How many previews were computed, for the tests.
    #[cfg(test)]
    runs: usize,
}

/// What the Presets menu keeps between frames.
#[derive(Debug, Default)]
pub struct Presets {
    dialog: Option<PresetDialog>,
}

impl Presets {
    pub fn has_dialog(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn open(&mut self, preset: Preset) {
        let mut copy = preset;
        let fixed = match copy.times_mut() {
            Some(&mut TimePolicy::Fixed(cs)) => cs,
            _ => DEFAULT_FIXED_TIME,
        };
        self.dialog = Some(PresetDialog {
            preset,
            fixed_text: format_time(fixed),
            computed: None,
            #[cfg(test)]
            runs: 0,
        });
    }

    #[cfg(test)]
    pub fn preview_runs(&self) -> Option<usize> {
        self.dialog.as_ref().map(|d| d.runs)
    }

    /// The menu's entries; `ui` is the open menu.
    pub fn menu(&mut self, ui: &mut Ui, doc: &Document) {
        for preset in Preset::menu(doc.current()) {
            if ui
                .button(preset.menu_title())
                .on_hover_text(preset.menu_tip())
                .clicked()
            {
                self.open(preset);
                ui.close();
            }
        }
        ui.separator();
        ui.weak("Resetting a level is on its page.");
    }

    pub fn dialog(&mut self, ctx: &egui::Context, doc: &Document, actions: &mut Vec<Action>) {
        let Some(open) = &mut self.dialog else {
            return;
        };
        let preset = open.preset;
        let ran = Previewed::refresh(&mut open.computed, doc, &preset, |save| preset.edits(save));
        #[cfg(test)]
        {
            open.runs += usize::from(ran);
        }
        #[cfg(not(test))]
        let _ = ran;
        let Some(computed) = &open.computed else {
            return;
        };

        let mut preset = open.preset;
        let mut close = false;
        let mut apply = false;
        let fixed_text = &mut open.fixed_text;
        let shown = Dialog::new("preset", preset.title())
            .width(DIALOG_WIDTH)
            .show(
                ctx,
                |ui| {
                    let time_error = options(ui, &mut preset, fixed_text);
                    let can_apply = preview::show(ui, doc, computed) && time_error.is_none();
                    (can_apply, time_error)
                },
                |ui, (can_apply, time_error)| {
                    if dialog::primary(ui, "Apply", *can_apply, time_error.as_deref()).clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                },
            );
        close |= shown.cancelled;
        if preset != open.preset {
            // The preview no longer matches the options: it runs again on
            // the next frame.
            open.preset = preset;
            ctx.request_repaint();
        } else if apply {
            actions.push(Action::Apply {
                label: computed.key.label(),
                edits: computed.edits.clone(),
            });
            close = true;
        }
        if close {
            self.dialog = None;
        }
    }
}

/// The options of `preset`. Returns why the fixed time can't be used, if
/// it is chosen and doesn't parse.
fn options(ui: &mut Ui, preset: &mut Preset, fixed_text: &mut String) -> Option<String> {
    let mut time_error = None;
    match preset {
        Preset::CompleteAll(o) => {
            wrapped(
                ui,
                "Every crystal, clear, second and colored gem, a platinum relic on every \
                 level, every boss defeated, every super power on and the bonus gem.",
            );
            time_error = time_policy(ui, &mut o.times, fixed_text);
            dialog::section(ui, "Story scenes");
            option_group(ui, |ui| {
                ui.checkbox(&mut o.skip_cutscenes, "Skip the bosses' story scenes")
                    .on_hover_text(SKIP_TIP);
            });
        }
        Preset::StoryUpTo { room, options: o } => {
            let n = usize::from(*room);
            let after = match *room {
                5 => "which ends the story".to_owned(),
                r => format!("which opens warp room {}", r + 1),
            };
            wrapped(
                ui,
                format!(
                    "The crystals of warp rooms 1 to {room} and their bosses defeated up to \
                     {}, {after}. Nothing is removed.",
                    BOSSES[n - 1].name()
                ),
            );
            let powers: Vec<&str> = ABILITIES[..n].iter().map(|a| a.name()).collect();
            dialog::section(ui, "Up to warp room");
            option_group(ui, |ui| {
                ui.horizontal(|ui| {
                    for r in 1..=5u8 {
                        ui.radio_value(room, r, r.to_string())
                            .on_hover_text(format!("Defeat {}", BOSSES[usize::from(r) - 1].name()));
                    }
                });
            });
            dialog::section(ui, "Options");
            option_group(ui, |ui| {
                ui.checkbox(
                    &mut o.with_gems,
                    "Also their gems (clear, second and colored)",
                );
                ui.checkbox(
                    &mut o.with_abilities,
                    "Grant the abilities of the defeated bosses",
                )
                .on_hover_text(powers.join(", "));
                ui.checkbox(&mut o.skip_cutscenes, "Skip their story scenes")
                    .on_hover_text(SKIP_TIP);
            });
        }
        Preset::AllRelics { tier, options: o } => {
            dialog::section(ui, "Relic");
            option_group(ui, |ui| {
                ui.horizontal(|ui| {
                    for relic in Relic::ALL {
                        ui.radio_value(tier, relic, relic.name());
                    }
                });
                ui.checkbox(&mut o.only_upgrade, "Never replace a better relic");
            });
            time_error = time_policy(ui, &mut o.times, fixed_text);
        }
        Preset::AllAbilities { on } => {
            wrapped(
                ui,
                "The five super powers work on their own, even before their boss is \
                 defeated.",
            );
            dialog::section(ui, "Super powers");
            option_group(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(on, true, "On");
                    ui.radio_value(on, false, "Off");
                });
            });
        }
        Preset::AllCutscenes { seen } => {
            wrapped(
                ui,
                "A scene marked as seen doesn't play. Scenes not seen play the next time \
                 their moment comes, which is harmless.",
            );
            dialog::section(ui, "Story scenes");
            option_group(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.radio_value(seen, true, "Seen (won't play)");
                    ui.radio_value(seen, false, "Not seen");
                });
            });
        }
    }
    time_error
}

const SKIP_TIP: &str = "Marks the hub scenes up to each defeated boss as seen. Scenes not \
                        skipped play later, which is harmless.";

/// Keep, matching, fixed (with its field) or clear. Returns why the fixed
/// time can't be used, when it is chosen and doesn't parse.
fn time_policy(ui: &mut Ui, policy: &mut TimePolicy, fixed_text: &mut String) -> Option<String> {
    let parsed = match parse_slot(fixed_text) {
        Ok(Some(cs)) => Ok(cs),
        Ok(None) => Err("Type a time, such as 1:00.00.".to_owned()),
        Err(e) => Err(e),
    };
    dialog::section(ui, "Times");
    let pal = theme::palette(ui.visuals().dark_mode);
    let mut error = None;
    option_group(ui, |ui| {
        ui.radio_value(
            policy,
            TimePolicy::Matching,
            "Write a time just under the relic's threshold",
        )
        .on_hover_text("Levels whose best time already earns the relic keep their times.");
        ui.radio_value(policy, TimePolicy::Keep, "Keep the times");
        ui.horizontal(|ui| {
            let fixed = matches!(policy, TimePolicy::Fixed(_));
            if ui.radio(fixed, "Write this time on every level:").clicked() && !fixed {
                *policy = TimePolicy::Fixed(*parsed.as_ref().unwrap_or(&DEFAULT_FIXED_TIME));
            }
            let mut edit = egui::TextEdit::singleline(fixed_text).desired_width(80.0);
            if fixed && parsed.is_err() {
                edit = edit.text_color(pal.error_text);
            }
            let response = ui.add_enabled(fixed, edit);
            if response.changed()
                && let Ok(Some(cs)) = parse_slot(fixed_text)
            {
                *policy = TimePolicy::Fixed(cs);
            }
            if let (true, Err(e)) = (fixed, &parsed) {
                error = Some(e.clone());
            }
        });
        if let Some(e) = &error {
            wrapped(ui, RichText::new(e.as_str()).color(pal.error_text).small());
        }
        ui.radio_value(policy, TimePolicy::Clear, "Clear the times");
    });
    error
}

#[cfg(test)]
mod tests {
    use nsane::crash3::Severity;

    use super::*;
    use crate::test_util::repo_file;

    fn open(rel: &str) -> Document {
        let bytes = std::fs::read(repo_file(rel)).unwrap();
        Document::open(&bytes, None).unwrap()
    }

    /// Each preset of the menu, with the dialog's defaults, leaves no
    /// warning or error; "Complete everything" reaches 108%.
    #[test]
    fn the_defaults_leave_no_warning_or_error() {
        for rel in [
            "tests/saves/game/initial_game.sav",
            "tests/saves/game/0019.sav",
        ] {
            let fresh = open(rel);
            for preset in Preset::menu(fresh.current()) {
                let mut doc = fresh.clone();
                let edits = preset.edits(doc.current()).unwrap();
                doc.apply(preset.label(), edits).unwrap();
                let serious: Vec<_> = doc
                    .diagnostics()
                    .iter()
                    .filter(|d| d.severity > Severity::Info)
                    .collect();
                assert!(serious.is_empty(), "{rel} {preset:?}: {serious:?}");
                if let Preset::CompleteAll(_) = preset {
                    assert_eq!(doc.current().totals().percent(), 108, "{rel}");
                }
            }
        }
    }

    #[test]
    fn the_defaults_are_the_core_defaults() {
        let doc = open("tests/saves/game/0019.sav");
        let [all, story, relics, powers, scenes] = Preset::menu(doc.current());
        assert_eq!(all, Preset::CompleteAll(CompleteAll::default()));
        // 0019: Tiny Tiger and Dingodile are defeated, N. Tropy is next.
        assert_eq!(
            story,
            Preset::StoryUpTo {
                room: 3,
                options: StoryUpTo::default()
            }
        );
        assert_eq!(
            relics,
            Preset::AllRelics {
                tier: Relic::Gold,
                options: AllRelics::default()
            }
        );
        assert_eq!(powers, Preset::AllAbilities { on: true });
        assert_eq!(scenes, Preset::AllCutscenes { seen: true });
    }

    #[test]
    fn labels_name_the_preset_and_its_options() {
        let doc = open("tests/saves/game/0019.sav");
        let labels: Vec<String> = Preset::menu(doc.current())
            .iter()
            .map(Preset::label)
            .collect();
        assert_eq!(
            labels,
            [
                "Complete everything (108%, matching times, skip scenes)",
                "Story up to warp room 3 (+abilities, skip scenes)",
                "All relics at gold (only upgrades, matching times)",
                "All super powers: on",
                "All story scenes: seen",
            ]
        );
        let story = Preset::StoryUpTo {
            room: 3,
            options: StoryUpTo {
                with_gems: true,
                with_abilities: false,
                skip_cutscenes: false,
            },
        };
        assert_eq!(story.label(), "Story up to warp room 3 (+gems)");
        let all = Preset::CompleteAll(CompleteAll {
            times: TimePolicy::Fixed(6000),
            skip_cutscenes: false,
        });
        assert_eq!(all.label(), "Complete everything (108%, times 1:00.00)");
    }

    /// What the dialog previews is what applying it changes, for every
    /// preset and option the dialog offers.
    #[test]
    fn the_preview_matches_the_applied_changes() {
        let policies = [
            TimePolicy::Matching,
            TimePolicy::Keep,
            TimePolicy::Fixed(6000),
            TimePolicy::Clear,
        ];
        for rel in [
            "tests/saves/game/0019.sav",
            "tests/saves/game/initial_game.sav",
            "tests/saves/game/ALCHEMYXSAVE_8.sav",
        ] {
            let fresh = open(rel);
            let mut cases = Vec::new();
            for times in policies {
                for skip_cutscenes in [true, false] {
                    cases.push(Preset::CompleteAll(CompleteAll {
                        times,
                        skip_cutscenes,
                    }));
                }
                for tier in Relic::ALL {
                    for only_upgrade in [true, false] {
                        cases.push(Preset::AllRelics {
                            tier,
                            options: AllRelics {
                                only_upgrade,
                                times,
                            },
                        });
                    }
                }
            }
            for room in 1..=5 {
                for bits in 0..8u8 {
                    cases.push(Preset::StoryUpTo {
                        room,
                        options: StoryUpTo {
                            with_gems: bits & 1 != 0,
                            with_abilities: bits & 2 != 0,
                            skip_cutscenes: bits & 4 != 0,
                        },
                    });
                }
            }
            for on in [true, false] {
                cases.push(Preset::AllAbilities { on });
                cases.push(Preset::AllCutscenes { seen: on });
            }
            for preset in cases {
                let edits = preset.edits(fresh.current()).unwrap();
                let preview = fresh.preview(&edits).unwrap();
                let mut doc = fresh.clone();
                doc.apply(preset.label(), edits).unwrap();
                assert_eq!(preview.changes, doc.changes(), "{rel} {preset:?}");
                assert_eq!(preview.totals, doc.current().totals(), "{rel} {preset:?}");
                assert_eq!(preview.diagnostics, doc.diagnostics(), "{rel} {preset:?}");
            }
        }
    }
}
