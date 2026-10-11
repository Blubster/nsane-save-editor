//! What a dialog shows before applying many edits at once: the changes
//! grouped, the totals they lead to and the diagnostics they add or resolve.
//!
//! [`Document::preview`] clones the save and applies the edits, so a dialog
//! keeps the result in a [`Previewed`] and computes it again only when its
//! options or the document change, not on every frame.

use egui::{RichText, ScrollArea, Ui};
use nsane::crash3::tables::MAX_PERCENT;
use nsane::crash3::{Crash3Save, Diagnostic, Edit, EditError, Severity, Totals};
use nsane::diff::{ChangeGroup, ReviewRow};
use nsane::{Document, Preview};

use crate::app::static_icon;
use crate::icons::Icon;
use crate::theme::{self, SPACING};

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

pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Review rows under a heading per [`ChangeGroup`], in group order.
pub fn grouped_rows(ui: &mut Ui, rows: &[ReviewRow]) {
    let mut groups: Vec<ChangeGroup> = rows.iter().map(|r| r.group).collect();
    groups.sort();
    groups.dedup();
    for group in groups {
        ui.label(RichText::new(group.name()).underline());
        for row in rows.iter().filter(|r| r.group == group) {
            let text = match row.text() {
                details if details.is_empty() => row.title.clone(),
                details => format!("{}: {details}", row.title),
            };
            ui.add(egui::Label::new(format!("• {text}")).wrap());
        }
    }
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

/// "Crystals: 10 → 11" for each total that changes; the percentage always.
pub fn total_lines(before: &Totals, after: &Totals) -> Vec<String> {
    let mut lines = vec![format!(
        "Percentage: {}% → {}%",
        before.percent(),
        after.percent()
    )];
    let pairs = [
        ("Crystals", before.crystals, after.crystals),
        ("Gems", before.gems, after.gems),
        ("Relics", before.relics(), after.relics()),
        ("Bosses", before.bosses, after.bosses),
    ];
    for (name, b, a) in pairs {
        if b != a {
            lines.push(format!("{name}: {b} → {a}"));
        }
    }
    lines
}

/// The preview of a dialog: changes, totals, diagnostics. Returns whether
/// there is something to apply.
pub fn show<K>(ui: &mut Ui, doc: &Document, previewed: &Previewed<K>) -> bool {
    ui.strong("Changes");
    let preview = match &previewed.result {
        Ok(p) => p,
        Err(e) => {
            let pal = theme::palette(ui.visuals().dark_mode);
            ui.horizontal(|ui| {
                static_icon(ui, Icon::Error, theme::ICON_SMALL);
                ui.colored_label(pal.error.base, format!("This can't be applied: {e}"));
            });
            return false;
        }
    };
    if preview.changes.is_empty() {
        ui.weak("Nothing to change.");
        return false;
    }
    ScrollArea::vertical()
        .max_height(CHANGES_HEIGHT)
        .auto_shrink([false, true])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
        .show(ui, |ui| grouped_rows(ui, &preview.rows));

    ui.add_space(SPACING / 2.0);
    let before = doc.current().totals();
    for line in total_lines(&before, &preview.totals) {
        ui.label(line);
    }
    if preview.totals.percent() == MAX_PERCENT {
        ui.weak("Everything the game counts is collected.");
    }

    ui.add_space(SPACING / 2.0);
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
        ui.weak("No new diagnostics.");
    } else {
        ui.label("New diagnostics:");
        diagnostic_lines(ui, new);
    }
    if resolved > 0 {
        ui.weak(format!(
            "Resolves {}.",
            plural(resolved, "diagnostic", "diagnostics")
        ));
    }
    let serious = preview
        .diagnostics
        .iter()
        .filter(|d| d.severity > Severity::Info)
        .count();
    if serious > 0 {
        let pal = theme::palette(ui.visuals().dark_mode);
        ui.colored_label(
            pal.warning.base,
            format!(
                "Afterwards: {}.",
                plural(serious, "warning or error", "warnings or errors")
            ),
        );
    }
    true
}
