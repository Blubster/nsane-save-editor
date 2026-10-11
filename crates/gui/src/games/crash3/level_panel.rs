//! The details of the selected level or boss: its collectibles, the time
//! trial, what the save records about it, and the reset.
//!
//! Like the rows, the panel only pushes edits. What it keeps between frames
//! is what the user is typing and the reset question.

use egui::{
    Button, CollapsingHeader, CornerRadius, Grid, Key, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, TextEdit, TextStyle, TextWrapMode, Ui, Vec2, WidgetInfo, WidgetText, WidgetType,
};
use nsane::Document;
use nsane::crash3::presets::matching_time;
use nsane::crash3::tables::{
    CUTSCENES, bit, field, relic_counted_key, relic_times, second_gem_hint,
};
use nsane::crash3::{
    ABILITIES, Crash3Save, Edit, Kind, LevelId, MAX_TIME_CS, RESET_LEVEL_VERIFIED, Relic,
    SECRET_ROOM, format_time, parse_time, relic_for_time,
};
use nsane::igb::FieldValue;

use super::level_row::{self, push};
use super::rooms;
use crate::app::{Action, static_icon};
use crate::dialog::{self, Dialog};
use crate::icons::{self, Boss, Icon, IconState};
use crate::theme::{self, SPACING};

/// Icon size of the labelled toggles.
const ICON: f32 = level_row::ICON;
/// Between an icon and its label.
const GAP: f32 = 6.0;
/// Width of a leaderboard field.
const TIME_FIELD_WIDTH: f32 = 84.0;

const TIME_FORMATS: &str = "Type a time as 1:00.23, 1:00:23 or 55.44. Leave it empty for no time.";

/// A leaderboard field being typed in.
#[derive(Debug, Clone, PartialEq)]
struct TimeBuffer {
    text: String,
    /// The leaderboard when typing started. When the save no longer holds
    /// it (undo, redo, another edit), the text is dropped.
    base: Vec<u32>,
    /// Why the text was refused, once the field was left.
    error: Option<String>,
}

/// What the panel keeps between frames.
#[derive(Debug, Default)]
pub struct LevelPanel {
    /// The level the buffers belong to: they are dropped when another one is
    /// selected, so a buffer is keyed by (level, slot).
    level: Option<LevelId>,
    times: [Option<TimeBuffer>; 3],
    /// The level whose reset waits for confirmation.
    confirm_reset: Option<LevelId>,
}

impl LevelPanel {
    /// Whether the reset question is open.
    pub fn has_dialog(&self) -> bool {
        self.confirm_reset.is_some()
    }

    pub fn show(
        &mut self,
        ui: &mut Ui,
        doc: &Document,
        selected: Option<LevelId>,
        actions: &mut Vec<Action>,
    ) {
        if self.level != selected {
            *self = Self {
                level: selected,
                ..Self::default()
            };
        }
        let Some(level) = selected else {
            ui.weak("Select a level to see its details.");
            return;
        };
        let current = doc.current().times(level);
        for buffer in &mut self.times {
            if buffer.as_ref().is_some_and(|b| b.base != current) {
                *buffer = None;
            }
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                header(ui, level);
                ui.separator();
                if level.level().kind == Kind::Boss {
                    boss(ui, doc, level, actions);
                } else {
                    collectibles(ui, doc, level, actions);
                    ui.separator();
                    self.time_trial(ui, doc, level, actions);
                }
                ui.separator();
                stats(ui, doc.current(), level);
                ui.separator();
                if reset_button(ui, level) {
                    self.confirm_reset = Some(level);
                }
            });
        self.reset_dialog(ui, doc, actions);
    }

    /// Puts `text` in field `slot` of `level` as a refused entry. Does
    /// nothing if the text would be accepted.
    pub fn refused_time(&mut self, doc: &Document, level: LevelId, slot: usize, text: &str) {
        let times = doc.current().times(level);
        if let Err(e) = commit_time(level, &times, slot, text) {
            *self = Self {
                level: Some(level),
                ..Self::default()
            };
            self.times[slot] = Some(TimeBuffer {
                text: text.to_owned(),
                base: times,
                error: Some(e),
            });
        }
    }

    fn time_trial(
        &mut self,
        ui: &mut Ui,
        doc: &Document,
        level: LevelId,
        actions: &mut Vec<Action>,
    ) {
        let Some(thresholds) = relic_times(level) else {
            return;
        };
        let save = doc.current();
        let relic = save.relic(level);
        let times = save.times(level);

        dialog::section(ui, "Time trial");
        ui.add_space(SPACING / 2.0);
        relic_selector(ui, doc, level, actions);
        ui.add_space(SPACING);

        Grid::new(("times", level))
            .num_columns(3)
            .spacing(Vec2::new(SPACING, 4.0))
            .show(ui, |ui| {
                for slot in 0..3 {
                    self.time_field(ui, doc, level, slot, &times, actions);
                    ui.end_row();
                }
            });
        let pal = theme::palette(ui.visuals().dark_mode);
        for (slot, buffer) in self.times.iter().enumerate() {
            if let Some(e) = buffer.as_ref().and_then(|b| b.error.as_deref()) {
                ui.colored_label(pal.error_text, format!("Time {}: {e}", slot + 1));
            }
        }
        ui.weak("Times are kept sorted, best first.")
            .on_hover_text(format!(
                "The game shows the leaderboard best first, so the editor sorts it. {}",
                "It does not check the times against the relic."
            ));
        ui.add_space(SPACING / 2.0);

        ui.horizontal_wrapped(|ui| {
            let fill = fill_times(level, &times, relic);
            let tip = match (relic, &fill) {
                (None, _) => "Pick a relic first".to_owned(),
                (Some(r), None) => {
                    format!("The best time already earns {}", r.name().to_lowercase())
                }
                (Some(r), Some(_)) => format!(
                    "Adds {}, the slowest time that earns {}, as the best time",
                    format_time(thresholds.threshold(r) - 1),
                    r.name().to_lowercase()
                ),
            };
            let button = ui
                .add_enabled(fill.is_some(), Button::new("Fill a time for this relic"))
                .on_hover_text(&tip)
                .on_disabled_hover_text(&tip);
            if button.clicked()
                && let Some(times_cs) = fill
            {
                push(actions, doc, Edit::SetTimes { level, times_cs });
            }

            let from_best = relic_from_best(level, &times, relic);
            let tip = match (times.first(), from_best) {
                (None, _) => "There is no time".to_owned(),
                (Some(_), None) => "The relic already matches the best time".to_owned(),
                (Some(&best), Some(r)) => format!(
                    "{} earns {}",
                    format_time(best),
                    match r {
                        Some(r) => format!("a {} relic", r.name().to_lowercase()),
                        None => "no relic".to_owned(),
                    }
                ),
            };
            let button = ui
                .add_enabled(from_best.is_some(), Button::new("Set relic from best time"))
                .on_hover_text(&tip)
                .on_disabled_hover_text(&tip);
            if button.clicked()
                && let Some(relic) = from_best
            {
                push(actions, doc, Edit::SetRelic { level, relic });
            }
        });
    }

    /// One leaderboard field. The text is applied when the field is left
    /// (Enter, Tab or a click elsewhere), as one edit of the whole
    /// leaderboard; Escape drops it.
    fn time_field(
        &mut self,
        ui: &mut Ui,
        doc: &Document,
        level: LevelId,
        slot: usize,
        times: &[u32],
        actions: &mut Vec<Action>,
    ) {
        let label = ui.label(format!("Time {}", slot + 1));
        let buffer = &mut self.times[slot];
        let mut text = match buffer {
            Some(b) => b.text.clone(),
            None => times.get(slot).map(|&t| format_time(t)).unwrap_or_default(),
        };
        let size = Vec2::new(TIME_FIELD_WIDTH, ui.spacing().interact_size.y);
        let response = ui
            .add_sized(
                size,
                TextEdit::singleline(&mut text)
                    .id_salt(("time", level, slot))
                    .desired_width(TIME_FIELD_WIDTH)
                    .hint_text("—"),
            )
            .labelled_by(label.id);
        if response.changed() {
            *buffer = Some(TimeBuffer {
                text,
                base: times.to_vec(),
                error: None,
            });
        }
        if response.lost_focus()
            && let Some(b) = buffer
        {
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                *buffer = None;
            } else {
                match commit_time(level, times, slot, &b.text) {
                    Ok(edit) => {
                        *buffer = None;
                        if let Some(edit) = edit {
                            push(actions, doc, edit);
                        }
                    }
                    Err(e) => b.error = Some(e),
                }
            }
        }

        // Read after the commit, so a refused text is outlined at once.
        match self.times[slot].as_ref().and_then(|b| b.error.as_deref()) {
            Some(e) => {
                let pal = theme::palette(ui.visuals().dark_mode);
                ui.painter().rect_stroke(
                    response.rect,
                    CornerRadius::same(theme::ROUNDING_SMALL),
                    Stroke::new(1.5, pal.error.base),
                    StrokeKind::Outside,
                );
                response.on_hover_text(e);
            }
            None => {
                response.on_hover_text(TIME_FORMATS);
            }
        }

        // The relic the time in the field earns, typed or stored.
        let shown = match &self.times[slot] {
            Some(b) => parse_slot(&b.text).ok().flatten(),
            None => times.get(slot).copied(),
        };
        ui.horizontal(|ui| match shown {
            Some(cs) => match relic_for_time(level, cs) {
                Some(r) => {
                    static_icon(ui, Icon::Relic(r), theme::ICON_SMALL).on_hover_text(format!(
                        "{} earns a {} relic",
                        format_time(cs),
                        r.name().to_lowercase()
                    ));
                }
                None => {
                    ui.weak("no relic").on_hover_text(format!(
                        "{} is slower than the sapphire time",
                        format_time(cs)
                    ));
                }
            },
            None => {
                ui.weak("");
            }
        });
    }

    fn reset_dialog(&mut self, ui: &mut Ui, doc: &Document, actions: &mut Vec<Action>) {
        let Some(level) = self.confirm_reset else {
            return;
        };
        let mut close = false;
        let shown = Dialog::new("reset-level", "Reset level").width(420.0).show(
            ui.ctx(),
            |ui| {
                dialog::wrapped(ui, reset_question(level));
                if level.level().kind == Kind::Boss {
                    dialog::wrapped(ui, "The ability and the story scenes are kept.");
                }
                if !RESET_LEVEL_VERIFIED {
                    ui.add_space(SPACING / 2.0);
                    ui.horizontal(|ui| {
                        static_icon(ui, Icon::Warning, theme::ICON_SMALL);
                        ui.label("Not yet tested in game.");
                    });
                }
            },
            |ui, _| {
                if ui.add(theme::danger_button(ui, "Reset")).clicked() {
                    push(actions, doc, Edit::ResetLevel { level });
                    close = true;
                }
                if ui.button("Cancel").clicked() {
                    close = true;
                }
            },
        );
        if close || shown.cancelled {
            self.confirm_reset = None;
        }
    }
}

/// "Reset Toad Village to never played? …"
pub fn reset_question(level: LevelId) -> String {
    let what = if level.level().kind == Kind::Boss {
        "Marks the boss as not defeated."
    } else {
        "Removes its crystal, gems, relic and times."
    };
    format!("Reset {} to never played? {what}", level.name())
}

/// Number, name, warp room and kind, and the other entrances.
fn header(ui: &mut Ui, level: LevelId) {
    let info = level.level();
    ui.heading(info.name);
    let room = match info.warp_room {
        6 => "Secret warp room".to_owned(),
        r => format!("Warp room {r}"),
    };
    let kind = match (info.kind, info.number) {
        (Kind::Boss, _) => "Boss".to_owned(),
        (Kind::Secret, _) => "Secret level".to_owned(),
        (Kind::Level, Some(n)) => format!("Level {n}"),
        (Kind::Level, None) => "Level".to_owned(),
    };
    ui.label(format!("{kind} · {room}"));
    for entry in SECRET_ROOM {
        if entry.level == level && Some(entry.number) != info.number {
            ui.weak(format!(
                "Also entered from secret level {} ({} relics).",
                entry.number, entry.relics
            ));
        }
    }
    if info.kind == Kind::Secret {
        ui.weak(format!("{}.", rooms::secret_level_access(level)));
    }
}

/// A clickable icon followed by its label, read as a checkbox named after
/// the label.
fn labelled_toggle(ui: &mut Ui, icon: Icon, label: &str, on: bool) -> Response {
    let width = (ui.available_width() - ICON - GAP).max(40.0);
    let galley =
        WidgetText::from(label).into_galley(ui, Some(TextWrapMode::Wrap), width, TextStyle::Body);
    let size = Vec2::new(ICON + GAP + galley.size().x, ICON.max(galley.size().y));
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, on, label));
    if ui.is_rect_visible(rect) {
        let dark = ui.visuals().dark_mode;
        let pal = theme::palette(dark);
        let hovered = response.hovered() || response.has_focus();
        let radius = CornerRadius::same(theme::ROUNDING_SMALL);
        if hovered {
            ui.painter()
                .rect_filled(rect.expand(2.0), radius, pal.hover_bg);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.expand(2.0),
                radius,
                Stroke::new(1.0, pal.accent.base),
                StrokeKind::Inside,
            );
        }
        let icon_rect = Rect::from_min_size(
            rect.min + Vec2::new(0.0, (size.y - ICON) / 2.0),
            Vec2::splat(ICON),
        );
        let state = if on {
            IconState::OWNED
        } else {
            IconState::MISSING
        };
        icons::paint(ui.painter(), icon_rect, icon, state.hovered(hovered), dark);
        let text_pos = egui::pos2(
            icon_rect.max.x + GAP,
            rect.center().y - galley.size().y / 2.0,
        );
        let color = if on { pal.text } else { pal.text_weak };
        ui.painter().galley(text_pos, galley, color);
    }
    response
}

/// A collectible of the panel: one edit per click.
fn collectible(
    ui: &mut Ui,
    doc: &Document,
    level: LevelId,
    (icon, label, on): (Icon, String, bool),
    edit: impl FnOnce(bool) -> Edit,
    actions: &mut Vec<Action>,
) {
    let entered = doc.current().entered(level);
    let mut tip = if on {
        "Collected. Click to remove."
    } else {
        "Not collected. Click to add."
    }
    .to_owned();
    if !on && !entered {
        tip += " Also marks the level as entered.";
    }
    if labelled_toggle(ui, icon, &label, on)
        .on_hover_text(tip)
        .clicked()
    {
        push(actions, doc, edit(!on));
    }
}

fn collectibles(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    let save = doc.current();
    let info = level.level();
    dialog::section(ui, "Collectibles");
    ui.add_space(SPACING / 2.0);
    if info.has_crystal() {
        let item = (Icon::Crystal, "Crystal".to_owned(), save.crystal(level));
        collectible(
            ui,
            doc,
            level,
            item,
            |on| Edit::SetCrystal { level, on },
            actions,
        );
    }
    let item = (
        Icon::ClearGem,
        "Clear gem — break every box".to_owned(),
        save.gem(level),
    );
    collectible(
        ui,
        doc,
        level,
        item,
        |on| Edit::SetGem { level, on },
        actions,
    );
    if let Some(hint) = second_gem_hint(level) {
        let item = (
            Icon::ClearGem,
            format!("Second gem — {hint}"),
            save.second_gem(level),
        );
        collectible(
            ui,
            doc,
            level,
            item,
            |on| Edit::SetSecondGem { level, on },
            actions,
        );
    }
    if let Some(color) = info.colored_gem {
        let icon = Icon::ColoredGem(color);
        let item = (
            icon,
            format!(
                "{} — opens paths in {}",
                icon.name(),
                level_row::colored_gem_paths(color)
            ),
            save.colored_gem(level),
        );
        collectible(
            ui,
            doc,
            level,
            item,
            |on| Edit::SetColoredGem { level, on },
            actions,
        );
    }
}

/// None / Sapphire / Gold / Platinum, with each threshold under it.
fn relic_selector(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    let Some(thresholds) = relic_times(level) else {
        return;
    };
    let current = doc.current().relic(level);
    let choices = [
        None,
        Some(Relic::Sapphire),
        Some(Relic::Gold),
        Some(Relic::Platinum),
    ];
    Grid::new(("relic", level))
        .num_columns(4)
        .spacing(Vec2::new(4.0, 2.0))
        .show(ui, |ui| {
            // Compact, so that the panel keeps its width whatever the
            // thresholds: a wider panel would narrow the warp room grid.
            ui.spacing_mut().button_padding.x = 6.0;
            for choice in choices {
                let name = choice.map_or("None", Relic::name);
                let tip = match choice {
                    Some(r) => format!(
                        "A time trial time under {} earns {}. The times are left as they are.",
                        format_time(thresholds.threshold(r)),
                        r.name().to_lowercase()
                    ),
                    None => "No relic. The times are left as they are.".to_owned(),
                };
                let button = ui
                    .add(Button::selectable(current == choice, name))
                    .on_hover_text(tip);
                if button.clicked() && choice != current {
                    push(
                        actions,
                        doc,
                        Edit::SetRelic {
                            level,
                            relic: choice,
                        },
                    );
                }
            }
            ui.end_row();
            for choice in choices {
                let text = match choice {
                    Some(r) => format!("< {}", format_time(thresholds.threshold(r))),
                    None => String::new(),
                };
                ui.label(RichText::new(text).weak().small());
            }
            ui.end_row();
        });
}

/// The defeat, the ability and the story scenes of a boss.
fn boss(ui: &mut Ui, doc: &Document, level: LevelId, actions: &mut Vec<Action>) {
    let save = doc.current();
    dialog::section(ui, "Boss");
    ui.add_space(SPACING / 2.0);
    if let Some(b) = Boss::of(level) {
        let defeated = save.boss_defeated(level);
        let opens = match level.level().warp_room {
            5 => "Ends the story.".to_owned(),
            room => format!("Opens warp room {}.", room + 1),
        };
        let tip = format!(
            "{opens} The ability and the story scenes are separate. Click to mark {}.",
            if defeated { "not defeated" } else { "defeated" }
        );
        if labelled_toggle(ui, Icon::Boss(b), "Defeated", defeated)
            .on_hover_text(tip)
            .clicked()
        {
            let edit = Edit::SetBossDefeated {
                boss: level,
                on: !defeated,
                skip_cutscenes: false,
            };
            push(actions, doc, edit);
        }

        let ability = ABILITIES[b as usize];
        let on = save.ability(ability);
        let label = format!("Ability: {}", ability.name());
        let tip = "Abilities are stored separately and work even if the boss isn't beaten.";
        if labelled_toggle(ui, Icon::Ability(ability), &label, on)
            .on_hover_text(tip)
            .clicked()
        {
            push(actions, doc, Edit::SetAbility { ability, on: !on });
        }
    }

    let Some(step) = CUTSCENES.iter().find(|s| s.boss() == Some(level)) else {
        return;
    };
    ui.add_space(SPACING);
    dialog::section(ui, "Story scenes").on_hover_text(
        "The hub scenes that play between the previous boss's defeat and this one's. \
         A scene marked as seen doesn't play.",
    );
    for &scene in step.scenes {
        let mut seen = save.cutscene_seen(scene);
        if ui.checkbox(&mut seen, scene).changed() {
            push(
                actions,
                doc,
                Edit::SetCutscene {
                    cutscene: scene.to_owned(),
                    seen,
                },
            );
        }
    }
}

/// What the save records about the level, read-only.
fn stats(ui: &mut Ui, save: &Crash3Save, level: LevelId) {
    let yes_no = |b: bool| if b { "yes" } else { "no" };
    dialog::section(ui, "Stats");
    ui.add_space(SPACING / 2.0);
    Grid::new(("stats", level))
        .num_columns(2)
        .spacing(Vec2::new(2.0 * SPACING, 4.0))
        .show(ui, |ui| {
            ui.label("Entered");
            ui.label(yes_no(save.entered(level)));
            ui.end_row();

            ui.label("Completions").on_hover_text(COMPLETIONS_TIP);
            ui.label(save.completions(level).to_string())
                .on_hover_text(COMPLETIONS_TIP);
            ui.end_row();

            if level.level().has_relic() {
                let key = relic_counted_key(level.level());
                let tip = format!(
                    "The {key} flag: present while the level holds a relic. The editor adds and \
                     removes it with the relic."
                );
                ui.label("Relic counted flag").on_hover_text(&tip);
                ui.label(yes_no(save.flag(&key))).on_hover_text(&tip);
                ui.end_row();
            }
        });

    CollapsingHeader::new("Advanced")
        .id_salt(("advanced", level))
        .default_open(false)
        .show(ui, |ui| advanced(ui, save, level));
}

const COMPLETIONS_TIP: &str = "Becomes 1 the first time you leave the level, then +1 each time \
    you finish it. The save doesn't record deaths, boxes broken or play time.";

/// The collectible bits and every session field, by the names of
/// `docs/FORMAT.md`.
fn advanced(ui: &mut Ui, save: &Crash3Save, level: LevelId) {
    let flags = save.flags(level);
    let mut names: Vec<String> = bit::NAMES
        .iter()
        .filter(|(b, _)| flags & b != 0)
        .map(|(_, n)| (*n).to_owned())
        .collect();
    let unknown = save.flags_unknown(level);
    if unknown != 0 {
        names.push(format!("unknown {unknown:#06x}"));
    }
    ui.label(RichText::new(format!("collectibles = {flags:#06x}")).monospace());
    ui.weak(if names.is_empty() {
        "no bit set".to_owned()
    } else {
        names.join(", ")
    });
    ui.add_space(SPACING / 2.0);
    let Some(session) = save.session(level) else {
        return;
    };
    Grid::new(("fields", level))
        .num_columns(3)
        .striped(true)
        .show(ui, |ui| {
            for (i, name) in field::NAMES.iter().enumerate() {
                ui.weak(i.to_string());
                ui.monospace(*name);
                let value = session
                    .field(i)
                    .map_or("—".to_owned(), |v| field_text(save, i, v));
                ui.monospace(value);
                ui.end_row();
            }
        });
}

/// A session field as text: strings quoted, the bit field in hex.
fn field_text(save: &Crash3Save, index: usize, value: &FieldValue) -> String {
    match *value {
        FieldValue::Str(idx) => match save.igb().string(idx) {
            Some(bytes) => format!(
                "{:?}",
                bytes.iter().map(|&b| char::from(b)).collect::<String>()
            ),
            None => "(none)".to_owned(),
        },
        FieldValue::Int(v) | FieldValue::Enum(v) => v.to_string(),
        FieldValue::UInt(v) if index == field::COLLECTIBLES => format!("{v:#06x}"),
        FieldValue::UInt(v) => v.to_string(),
        FieldValue::Bool { value, .. } => value.to_string(),
        FieldValue::Float(bits) => f32::from_bits(bits).to_string(),
        ref other => format!("{other:?}"),
    }
}

/// The reset button. Returns whether it was clicked.
fn reset_button(ui: &mut Ui, level: LevelId) -> bool {
    let mut tip = format!(
        "Return {} to its state in a new game. You will be asked first.",
        level.name()
    );
    if !RESET_LEVEL_VERIFIED {
        tip += " Not yet tested in game.";
    }
    let clicked = ui
        .add(theme::danger_button(ui, "Reset level…"))
        .on_hover_text(tip)
        .clicked();
    if !RESET_LEVEL_VERIFIED {
        ui.weak("Reset is not yet tested in game.");
    }
    clicked
}

// -------------------------------------------------------------- logic

/// A leaderboard field's text: `None` for an empty field.
pub fn parse_slot(text: &str) -> Result<Option<u32>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let cs = parse_time(text)
        .map_err(|_| format!("\"{}\" is not a time. {TIME_FORMATS}", text.trim()))?;
    if cs == 0 {
        return Err("A time must be at least 0:00.01.".to_owned());
    }
    if cs > MAX_TIME_CS {
        return Err(format!("Times go up to {}.", format_time(MAX_TIME_CS)));
    }
    Ok(Some(cs))
}

/// The edit that applies `text` in leaderboard slot `slot`, the other slots
/// keeping the save's times. `Ok(None)` when nothing changes; `Err` explains
/// why the text is refused.
pub fn commit_time(
    level: LevelId,
    times: &[u32],
    slot: usize,
    text: &str,
) -> Result<Option<Edit>, String> {
    let value = parse_slot(text)?;
    let mut times_cs: Vec<u32> = (0..3)
        .filter_map(|i| {
            if i == slot {
                value
            } else {
                times.get(i).copied()
            }
        })
        .collect();
    times_cs.sort_unstable();
    let mut before = times.to_vec();
    before.sort_unstable();
    Ok((times_cs != before).then_some(Edit::SetTimes { level, times_cs }))
}

/// The leaderboard after "Fill a time for this relic": the slowest time that
/// earns `relic` becomes the best, and the two best others stay. `None`
/// when there is no relic or the best time already earns it.
pub fn fill_times(level: LevelId, times: &[u32], relic: Option<Relic>) -> Option<Vec<u32>> {
    let relic = relic?;
    let best = times.iter().min().copied();
    if best.and_then(|t| relic_for_time(level, t)) >= Some(relic) {
        return None;
    }
    let mut filled: Vec<u32> = std::iter::once(matching_time(level, relic)?)
        .chain(times.iter().copied())
        .collect();
    filled.sort_unstable();
    filled.truncate(3);
    Some(filled)
}

/// The relic "Set relic from best time" gives: the one the best time earns,
/// possibly none. `None` when there is no time or the relic already matches.
pub fn relic_from_best(
    level: LevelId,
    times: &[u32],
    relic: Option<Relic>,
) -> Option<Option<Relic>> {
    let best = *times.iter().min()?;
    let earned = relic_for_time(level, best);
    (earned != relic).then_some(earned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(key: &str) -> LevelId {
        LevelId::from_key(key).unwrap()
    }

    #[test]
    fn a_slot_accepts_the_documented_forms_and_empty() {
        for (text, cs) in [
            ("1:00.23", Some(6023)),
            ("1:00:23", Some(6023)),
            ("55.44", Some(5544)),
            (" 0:55.44 ", Some(5544)),
            ("", None),
            ("   ", None),
            ("99:59.99", Some(MAX_TIME_CS)),
        ] {
            assert_eq!(parse_slot(text), Ok(cs), "{text:?}");
        }
        for text in ["1:7x", "99:99:99", "-1", "0:00.00", "100:00.00", "abc"] {
            assert!(parse_slot(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn a_commit_is_one_edit_of_the_whole_leaderboard() {
        let toad = level("toadvillage");
        assert_eq!(
            commit_time(toad, &[6023], 1, "0:55.44"),
            Ok(Some(Edit::SetTimes {
                level: toad,
                times_cs: vec![5544, 6023]
            }))
        );
        // Clearing a slot drops its time.
        assert_eq!(
            commit_time(toad, &[5544, 6023], 0, ""),
            Ok(Some(Edit::SetTimes {
                level: toad,
                times_cs: vec![6023]
            }))
        );
        // The same time again changes nothing.
        assert_eq!(commit_time(toad, &[6023], 0, "1:00:23"), Ok(None));
        assert_eq!(commit_time(toad, &[], 2, ""), Ok(None));
        assert!(commit_time(toad, &[6023], 1, "1:7x").is_err());
    }

    #[test]
    fn fill_writes_the_slowest_time_that_earns_the_relic() {
        let toad = level("toadvillage");
        // Toad Village platinum is under 0:44.03.
        assert_eq!(
            fill_times(toad, &[5544, 6023], Some(Relic::Platinum)),
            Some(vec![4402, 5544, 6023])
        );
        assert_eq!(
            fill_times(toad, &[5544, 6023, 7000], Some(Relic::Platinum)),
            Some(vec![4402, 5544, 6023])
        );
        // 0:55.44 already earns gold.
        assert_eq!(fill_times(toad, &[5544], Some(Relic::Gold)), None);
        assert_eq!(fill_times(toad, &[5544], Some(Relic::Sapphire)), None);
        assert_eq!(fill_times(toad, &[5544], None), None);
    }

    #[test]
    fn set_relic_from_best_picks_what_the_time_earns() {
        let toad = level("toadvillage");
        assert_eq!(
            relic_from_best(toad, &[5544, 6023], None),
            Some(Some(Relic::Gold))
        );
        assert_eq!(
            relic_from_best(toad, &[5544, 6023], Some(Relic::Gold)),
            None
        );
        assert_eq!(
            relic_from_best(toad, &[4402], Some(Relic::Gold)),
            Some(Some(Relic::Platinum))
        );
        // Slower than sapphire: the relic is removed.
        assert_eq!(
            relic_from_best(toad, &[9000], Some(Relic::Gold)),
            Some(None)
        );
        assert_eq!(relic_from_best(toad, &[], Some(Relic::Gold)), None);
    }

    #[test]
    fn the_panel_edits_reproduce_the_game() {
        use crate::test_util::repo_file;
        // Gold and 0:55.44 above 1:00.23 on Toad Village, as the game wrote
        // between 0010 and 0011 (which also counted the finish).
        let toad = level("toadvillage");
        let read = |rel: &str| Crash3Save::parse(&std::fs::read(repo_file(rel)).unwrap()).unwrap();
        let mut save = read("tests/saves/game/0010.sav");
        let times = save.times(toad);
        assert_eq!(times, [6023]);
        let edits = [
            Edit::SetRelic {
                level: toad,
                relic: Some(Relic::Gold),
            },
            commit_time(toad, &times, 1, "0:55.44").unwrap().unwrap(),
        ];
        for edit in edits {
            save.apply(&edit).unwrap();
        }
        let expected = read("tests/saves/game/0011.sav");
        let values = |s: &Crash3Save| -> Vec<String> {
            let session = s.session(toad).unwrap();
            (0..field::NAMES.len())
                .filter(|&i| i != field::COMPLETIONS)
                .map(|i| field_text(s, i, session.field(i).unwrap()))
                .collect()
        };
        assert_eq!(values(&save), values(&expected));
        assert_ne!(save.completions(toad), expected.completions(toad));
    }

    #[test]
    fn the_reset_question_names_the_level() {
        assert_eq!(
            reset_question(level("toadvillage")),
            "Reset Toad Village to never played? Removes its crystal, gems, relic and times."
        );
    }
}
