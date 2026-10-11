//! The overview panel.

use egui::{DragValue, Ui};
use nsane::Document;
use nsane::crash3::Edit;

use crate::app::{Action, static_icon};
use crate::icons::Icon;
use crate::theme;

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
        let response = ui.add(
            DragValue::new(&mut value)
                .range(LIVES_RANGE)
                .clamp_existing_to_range(false)
                .speed(0.25),
        );
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
