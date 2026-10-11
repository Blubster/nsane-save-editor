//! What a dialog shows before applying many edits at once: the changes
//! grouped, the totals they lead to and the diagnostics they add or resolve.
//!
//! [`Document::preview`] clones the save and applies the edits, so a dialog
//! keeps the result in a [`Previewed`] and computes it again only when its
//! options or the document change, not on every frame.

use egui::Ui;
use nsane::crash3::tables::MAX_PERCENT;
use nsane::crash3::{Crash3Save, Diagnostic, Edit, EditError, Severity, Totals};
use nsane::diff::{ChangeGroup, ReviewRow};
use nsane::{Document, Preview};

use crate::app::static_icon;
use crate::dialog::{self, Group, Total, plural};
use crate::icons::Icon;
use crate::theme;

/// The tallest the change list gets before it scrolls.
const CHANGES_HEIGHT: f32 = 240.0;
/// A preview, and the options and document state it was computed for.
#[derive(Debug)]
pub struct Previewed<K> {
    generation: u64,
    pub key: K,
    pub edits: Vec<Edit>,
    pub result: Result<Preview, String>,
}

impl<K: Clone + PartialEq> Previewed<K> {
    /// Computes the preview of `edits(key)` into `slot` unless it already
    /// holds it for this state of `doc`. Returns whether it ran.
    pub fn refresh(
        slot: &mut Option<Self>,
        doc: &Document,
        key: &K,
        edits: impl FnOnce(&Crash3Save) -> Result<Vec<Edit>, EditError>,
    ) -> bool {
        let generation = doc.generation();
        if slot
            .as_ref()
            .is_some_and(|p| p.generation == generation && p.key == *key)
        {
            return false;
        }
        let (edits, result) = match edits(doc.current()) {
            Ok(edits) => {
                let result = doc.preview(&edits).map_err(|e| e.to_string());
                (edits, result)
            }
            Err(e) => (Vec::new(), Err(e.to_string())),
        };
        *slot = Some(Self {
            generation,
            key: key.clone(),
            edits,
            result,
        });
        true
    }
}

pub fn severity_icon(severity: Severity) -> Icon {
    match severity {
        Severity::Error => Icon::Error,
        Severity::Warning => Icon::Warning,
        Severity::Info => Icon::Info,
    }
}

/// Review rows under a title per [`ChangeGroup`], in group order.
pub fn groups(rows: &[ReviewRow]) -> Vec<Group> {
    let mut groups: Vec<ChangeGroup> = rows.iter().map(|r| r.group).collect();
    groups.sort();
    groups.dedup();
    groups
        .into_iter()
        .map(|group| Group {
            title: group.name().to_owned(),
            rows: rows
                .iter()
                .filter(|r| r.group == group)
                .map(|row| match row.text() {
                    details if details.is_empty() => row.title.clone(),
                    details => format!("{}: {details}", row.title),
                })
                .collect(),
        })
        .collect()
}

/// One line per diagnostic, with its severity icon.
pub fn diagnostic_lines<'a>(ui: &mut Ui, diagnostics: impl IntoIterator<Item = &'a Diagnostic>) {
    for d in diagnostics {
        ui.horizontal(|ui| {
            static_icon(ui, severity_icon(d.severity), theme::ICON_SMALL);
            ui.add(egui::Label::new(d.message.as_str()).wrap());
        });
    }
}

/// Each total that changes, "Crystals: 10 → 11"; the percentage always.
pub fn totals(before: &Totals, after: &Totals) -> Vec<Total> {
    let mut totals = vec![Total {
        name: "Percentage",
        before: format!("{}%", before.percent()),
        after: format!("{}%", after.percent()),
    }];
    let pairs = [
        ("Crystals", before.crystals, after.crystals),
        ("Gems", before.gems, after.gems),
        ("Relics", before.relics(), after.relics()),
        ("Bosses", before.bosses, after.bosses),
    ];
    for (name, b, a) in pairs {
        if b != a {
            totals.push(Total {
                name,
                before: b.to_string(),
                after: a.to_string(),
            });
        }
    }
    totals
}

/// The preview of a dialog: changes, totals, diagnostics. Returns whether
/// there is something to apply.
pub fn show<K>(ui: &mut Ui, doc: &Document, previewed: &Previewed<K>) -> bool {
    dialog::section(ui, "Changes");
    let pal = theme::palette(ui.visuals().dark_mode);
    let preview = match &previewed.result {
        Ok(p) => p,
        Err(e) => {
            ui.horizontal(|ui| {
                static_icon(ui, Icon::Error, theme::ICON_SMALL);
                dialog::wrapped(
                    ui,
                    egui::RichText::new(format!("This can't be applied: {e}"))
                        .color(pal.error_text),
                );
            });
            return false;
        }
    };
    if preview.changes.is_empty() {
        dialog::note(ui, "Nothing to change.");
        return false;
    }
    let before = doc.current().totals();
    dialog::changes(
        ui,
        "preview",
        &groups(&preview.rows),
        &totals(&before, &preview.totals),
        CHANGES_HEIGHT,
    );
    if preview.totals.percent() == MAX_PERCENT {
        dialog::note(ui, "Everything the game counts is collected.");
    }

    let now = doc.diagnostics();
    let new: Vec<_> = preview
        .diagnostics
        .iter()
        .filter(|d| !now.contains(d))
        .collect();
    let resolved = now
        .iter()
        .filter(|d| !preview.diagnostics.contains(d))
        .count();
    if new.is_empty() {
        dialog::note(ui, "No new diagnostics.");
    } else {
        dialog::section(ui, "New diagnostics");
        diagnostic_lines(ui, new);
    }
    if resolved > 0 {
        dialog::note(
            ui,
            format!(
                "Resolves {}.",
                plural(resolved, "diagnostic", "diagnostics")
            ),
        );
    }
    let serious = preview
        .diagnostics
        .iter()
        .filter(|d| d.severity > Severity::Info)
        .count();
    if serious > 0 {
        dialog::wrapped(
            ui,
            egui::RichText::new(format!(
                "Afterwards: {}.",
                plural(serious, "warning or error", "warnings or errors")
            ))
            .color(pal.warning_text),
        );
    }
    true
}
