//! The review shown before saving: every change since the file was opened,
//! the summary values writing recomputes, and the warnings and errors left.

use egui::{RichText, ScrollArea, Ui};
use nsane::Document;
use nsane::crash3::{Diagnostic, Severity};
use nsane::diff::{Change, ReviewRow};

use super::preview::{diagnostic_lines, grouped_rows};
use crate::theme::SPACING;

/// The tallest the change list gets before it scrolls.
const ROWS_HEIGHT: f32 = 300.0;

/// What the save review lists. Computed once when the review opens: the
/// diff walks the whole save.
#[derive(Debug, Clone, PartialEq)]
pub struct SaveReview {
    /// Against the save as opened, in [`ChangeGroup`](nsane::diff::ChangeGroup)
    /// order.
    pub rows: Vec<ReviewRow>,
    /// Summary values no edit changed, which writing recomputes.
    pub recomputed: Vec<Change>,
    /// Warnings and errors, most severe first.
    pub diagnostics: Vec<Diagnostic>,
}

impl SaveReview {
    pub fn of(doc: &Document) -> Self {
        let review = doc.review();
        Self {
            rows: review.rows,
            recomputed: review.recomputed,
            diagnostics: doc
                .diagnostics()
                .iter()
                .filter(|d| d.severity > Severity::Info)
                .cloned()
                .collect(),
        }
    }

    pub fn show(&self, ui: &mut Ui, file_name: &str) {
        ui.label(format!("Changes since {file_name} was opened:"));
        if self.rows.is_empty() {
            ui.weak("None: the save is as it was opened.");
        } else {
            ScrollArea::vertical()
                .max_height(ROWS_HEIGHT)
                .auto_shrink([false, true])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                .show(ui, |ui| grouped_rows(ui, &self.rows));
        }
        if !self.recomputed.is_empty() {
            ui.add_space(SPACING / 2.0);
            ui.label(RichText::new("Recomputed when saving").underline())
                .on_hover_text("The values the save slot screen shows, counted from the save");
            for change in &self.recomputed {
                ui.label(format!("• {}", change.describe()));
            }
        }
        ui.add_space(SPACING / 2.0);
        if self.diagnostics.is_empty() {
            ui.weak("No warnings.");
        } else {
            ui.label(RichText::new("Warnings").underline());
            diagnostic_lines(ui, &self.diagnostics);
        }
    }
}

#[cfg(test)]
mod tests {
    use nsane::crash3::{Ability, Edit, LevelId};
    use nsane::diff::{ChangeGroup, Subject};
    use nsane::igb::SummaryValue;

    use super::*;
    use crate::test_util::repo_file;

    /// The scripted review session on 0019: lives 41 → 42, Deep
    /// Trouble's crystal, Speed Shoes. Deep Trouble was never entered, so
    /// its row also says it now is.
    #[test]
    fn the_review_of_a_session_on_0019() {
        let bytes = std::fs::read(repo_file("tests/saves/game/0019.sav")).unwrap();
        let mut doc = Document::open(&bytes, Some("0019.sav".into())).unwrap();
        let deep = LevelId::from_key("deeptrouble").unwrap();
        for edit in [
            Edit::SetLives { count: 42 },
            Edit::SetCrystal {
                level: deep,
                on: true,
            },
            Edit::SetAbility {
                ability: Ability::SpeedShoes,
                on: true,
            },
        ] {
            doc.apply(edit.describe(doc.current()), vec![edit]).unwrap();
        }
        let review = SaveReview::of(&doc);

        let rows: Vec<_> = review
            .rows
            .iter()
            .map(|r| (r.group, r.subject.clone(), r.details.clone()))
            .collect();
        assert_eq!(
            rows,
            [
                (
                    ChangeGroup::Levels,
                    Subject::Level(deep),
                    vec![
                        "+ crystal".to_owned(),
                        "entered, completions 0 → 1".to_owned()
                    ]
                ),
                (
                    ChangeGroup::Abilities,
                    Subject::Ability(Ability::SpeedShoes),
                    vec!["off → on".to_owned()]
                ),
                (
                    ChangeGroup::Summary,
                    Subject::Summary("life_count".into()),
                    vec!["41 → 42".to_owned()]
                ),
            ]
        );
        // Fields 2, 3, 12 and 13 of Deep Trouble's session changed.
        let fields: Vec<usize> = doc
            .changes()
            .iter()
            .filter_map(|c| match c {
                Change::Zone {
                    level: Some(l),
                    field,
                    ..
                } if *l == deep => Some(*field),
                _ => None,
            })
            .collect();
        assert_eq!(fields, [2, 3, 12, 13]);

        let recomputed: Vec<_> = review
            .recomputed
            .iter()
            .map(|c| match c {
                Change::Summary { key, before, after } => {
                    (key.as_str(), before.clone(), after.clone())
                }
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            recomputed,
            [
                (
                    "percent_complete",
                    Some(SummaryValue::Float(19.0)),
                    Some(SummaryValue::Float(20.0))
                ),
                (
                    "crystal_count",
                    Some(SummaryValue::Int(10)),
                    Some(SummaryValue::Int(11))
                ),
            ]
        );
        assert!(review.diagnostics.is_empty());
    }
}
