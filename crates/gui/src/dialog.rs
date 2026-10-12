//! The frame every modal dialog shares, and the pieces dialogs are made of.
//!
//! A dialog has a header (a bold title, maybe a subtitle), a body that
//! scrolls when the window is short, and a footer: the primary action on the
//! right, Cancel next to it, the other actions on the left. Escape cancels,
//! unless a menu is open over the dialog; Enter confirms only the dialogs
//! that allow it. A click on the backdrop does nothing, so that a stray
//! click doesn't lose the options picked.

use egui::{
    Align, Align2, Context, Frame, Grid, Id, Key, Label, Layout, Margin, Modal, Modifiers, Rect,
    Response, RichText, ScrollArea, Stroke, TextStyle, Ui, UiBuilder, WidgetInfo, WidgetText,
    WidgetType, vec2,
};

use crate::app::static_icon;
use crate::icons::Icon;
use crate::theme::{self, SPACING};

/// Space between the edge of a dialog and its content.
const PAD: i8 = 20;
/// The least space left between a dialog and the edge of the window.
const SCREEN_MARGIN: f32 = 16.0;
/// Reserved for the footer when the body's height is limited.
const FOOTER_HEIGHT: f32 = 64.0;
/// The body never gets shorter than this, however small the window.
const MIN_BODY_HEIGHT: f32 = 120.0;

/// A modal dialog: see the module documentation.
pub struct Dialog {
    id: Id,
    title: String,
    subtitle: Option<String>,
    icon: Option<(Icon, f32)>,
    width: f32,
    enter_confirms: bool,
}

/// Where a dialog was last shown: its top, its height and the frame.
#[derive(Clone, Copy, Debug)]
struct Placement {
    top: f32,
    height: f32,
    frame_nr: u64,
}

/// What a dialog's frame saw this frame, besides what its body returned.
pub struct Shown<R> {
    pub inner: R,
    /// Escape was pressed: the dialog closes as if cancelled.
    pub cancelled: bool,
    /// Enter was pressed, with no text field focused, in a dialog that
    /// allows it.
    pub confirmed: bool,
}

impl Dialog {
    pub fn new(id: &str, title: impl Into<String>) -> Self {
        Self {
            id: Id::new(id),
            title: title.into(),
            subtitle: None,
            icon: None,
            width: 440.0,
            enter_confirms: false,
        }
    }

    /// A line in the weak colour under the title.
    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = Some(text.into());
        self
    }

    /// An icon before the title.
    pub fn icon(mut self, icon: Icon, size: f32) -> Self {
        self.icon = Some((icon, size));
        self
    }

    /// The width the dialog takes, when the window is wide enough. It is
    /// set exactly, so that the dialog doesn't change size, and move, as
    /// its wrapped text is laid out over several frames.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Enter confirms, unless a text field has focus. Only for dialogs
    /// whose confirmation loses nothing.
    pub fn enter_confirms(mut self) -> Self {
        self.enter_confirms = true;
        self
    }

    /// Shows the dialog. `footer` gets a right-to-left row: add the primary
    /// action first, then Cancel, then [`left`] for the others. It also
    /// gets what `body` returned.
    pub fn show<R>(
        self,
        ctx: &Context,
        body: impl FnOnce(&mut Ui) -> R,
        footer: impl FnOnce(&mut Ui, &R),
    ) -> Shown<R> {
        let style = ctx.global_style();
        let pal = theme::palette(style.visuals.dark_mode);
        let screen = ctx.content_rect();
        let width = self
            .width
            .min(screen.width() - 2.0 * SCREEN_MARGIN)
            .max(160.0);
        let frame = Frame::new()
            .fill(pal.surface)
            .stroke(Stroke::new(1.0, pal.border))
            .corner_radius(theme::ROUNDING_LARGE)
            .shadow(style.visuals.window_shadow);
        let id = self.id;
        let place_id = id.with("place");
        let frame_nr = ctx.cumulative_frame_nr();
        // The top of the dialog is kept from the frame it opened in:
        // centred then, it stays put when its content grows or shrinks (a
        // preview, a result), and only moves up to stay on screen. A dialog
        // opened again starts centred on the height it had last time.
        let saved = ctx.data(|d| d.get_temp::<Placement>(place_id));
        let open = saved.is_some_and(|p| p.frame_nr + 1 >= frame_nr);
        let centred =
            |height: f32| screen.top() + ((screen.height() - height) / 2.0).max(SCREEN_MARGIN);
        let top = match saved {
            Some(p) if open => p.top,
            Some(p) => centred(p.height),
            None => screen.top() + SCREEN_MARGIN,
        };
        let area =
            Modal::default_area(id).anchor(Align2::CENTER_TOP, vec2(0.0, top - screen.top()));
        let response = Modal::new(id).area(area).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            // An area, like a scroll area, offers its content the size it
            // had last frame, so a dialog that grows would grow a step per
            // frame, moving each time. Offer it all the height at once:
            // the body's scroll area limits it.
            ui.scope_builder(UiBuilder::new().max_rect(unbounded(ui)), |ui| {
                self.contents(ui, screen.height(), body, footer)
            })
            .inner
        });
        let height = response.response.rect.height();
        let top = if open {
            let lowest = screen.bottom() - SCREEN_MARGIN - height;
            top.min(lowest).max(screen.top() + SCREEN_MARGIN)
        } else {
            centred(height)
        };
        ctx.data_mut(|d| {
            d.insert_temp(
                place_id,
                Placement {
                    top,
                    height,
                    frame_nr,
                },
            )
        });
        let on_top = response.is_top_modal && !response.any_popup_open;
        let cancelled = on_top && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
        let confirmed = self.enter_confirms
            && on_top
            && !ctx.egui_wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter));
        Shown {
            inner: response.inner,
            cancelled,
            confirmed,
        }
    }

    /// The header, the body that scrolls past what the screen leaves it, a
    /// line, and the footer.
    fn contents<R>(
        &self,
        ui: &mut Ui,
        screen_height: f32,
        body: impl FnOnce(&mut Ui) -> R,
        footer: impl FnOnce(&mut Ui, &R),
    ) -> R {
        let pal = theme::palette(ui.visuals().dark_mode);
        let top = ui.cursor().top();
        header(ui, self);
        let header_height = ui.cursor().top() - top;
        let max_body = (screen_height - 2.0 * SCREEN_MARGIN - header_height - FOOTER_HEIGHT)
            .max(MIN_BODY_HEIGHT);
        let inner = ScrollArea::vertical()
            .id_salt(self.id.with("body"))
            .max_height(max_body)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                // The same for the lists in the body, inside this scroll area.
                ui.scope_builder(UiBuilder::new().max_rect(unbounded(ui)), |ui| {
                    Frame::new()
                        .inner_margin(Margin {
                            left: PAD,
                            right: PAD,
                            top: 0,
                            bottom: PAD - 4,
                        })
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            body(ui)
                        })
                        .inner
                })
                .inner
            })
            .inner;
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0, pal.border),
        );
        Frame::new()
            .inner_margin(Margin::symmetric(PAD, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_width(ui.available_width());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        footer(ui, &inner)
                    });
                });
            });
        inner
    }
}

/// The rest of `ui`'s width, with no limit on the height.
fn unbounded(ui: &Ui) -> Rect {
    let max = ui.max_rect();
    Rect::from_min_size(
        ui.cursor().min,
        vec2(max.right() - ui.cursor().left(), f32::INFINITY),
    )
}

fn header(ui: &mut Ui, dialog: &Dialog) {
    Frame::new()
        .inner_margin(Margin {
            left: PAD,
            right: PAD,
            top: PAD - 2,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                if let Some((icon, size)) = dialog.icon {
                    static_icon(ui, icon, size);
                }
                ui.vertical(|ui| {
                    ui.add(
                        Label::new(RichText::new(&dialog.title).text_style(TextStyle::Heading))
                            .wrap(),
                    );
                    if let Some(subtitle) = &dialog.subtitle {
                        ui.add(Label::new(RichText::new(subtitle).weak()).wrap());
                    }
                });
            });
        });
}

/// Lays out the actions of a footer from the left, after the primary
/// action and Cancel.
pub fn left(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.with_layout(Layout::left_to_right(Align::Center), add);
}

/// The primary action of a footer. `why_not` explains why it is disabled.
pub fn primary(ui: &mut Ui, text: &str, enabled: bool, why_not: Option<&str>) -> Response {
    let button = theme::primary_button(ui, text);
    let response = ui.add_enabled(enabled, button);
    match why_not {
        Some(why) => response.on_disabled_hover_text(why),
        None => response,
    }
}

/// A bold title over a part of a dialog or a panel.
pub fn section(ui: &mut Ui, title: &str) -> Response {
    ui.add_space(SPACING);
    let response = ui.label(theme::bold(title).size(theme::FONT_TITLE));
    ui.add_space(2.0);
    response
}

/// Radios and checkboxes that belong together, in a light frame.
pub fn option_group<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let pal = theme::palette(ui.visuals().dark_mode);
    Frame::new()
        .fill(pal.chrome)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(theme::ROUNDING)
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Rows under a bold title, in a list of changes.
pub struct Group {
    pub title: String,
    pub rows: Vec<String>,
}

/// A total before and after: "Percentage", "19%", "108%".
pub struct Total {
    pub name: &'static str,
    pub before: String,
    pub after: String,
}

impl Total {
    /// "Percentage: 19% → 108%", what screen readers and tests read.
    pub fn text(&self) -> String {
        format!("{}: {} → {}", self.name, self.before, self.after)
    }
}

/// What a dialog changes: the rows under a bold title per group, in a
/// sunken list that scrolls past `max_height`, then the totals in two
/// columns.
pub fn changes(ui: &mut Ui, id_salt: &str, groups: &[Group], totals: &[Total], max_height: f32) {
    let pal = theme::palette(ui.visuals().dark_mode);
    let id = Id::new(id_salt);
    if !groups.is_empty() {
        Frame::new()
            .fill(pal.canvas)
            .stroke(Stroke::new(1.0, pal.border))
            .corner_radius(theme::ROUNDING)
            .inner_margin(Margin::symmetric(0, 6))
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt(id.with("rows"))
                    .max_height(max_height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        Frame::new()
                            .inner_margin(Margin::symmetric(12, 0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.spacing_mut().item_spacing.y = 3.0;
                                for (i, group) in groups.iter().enumerate() {
                                    if i > 0 {
                                        ui.add_space(SPACING / 2.0);
                                    }
                                    ui.label(theme::bold(&group.title));
                                    for row in &group.rows {
                                        ui.add(Label::new(row.as_str()).wrap());
                                    }
                                }
                            });
                    });
            });
    }
    if totals.is_empty() {
        return;
    }
    ui.add_space(SPACING / 2.0);
    Grid::new(id.with("totals"))
        .num_columns(2)
        .spacing([3.0 * SPACING, 3.0])
        .show(ui, |ui| {
            for total in totals {
                ui.label(RichText::new(total.name).weak());
                let response = ui.label(theme::bold(format!("{} → {}", total.before, total.after)));
                let text = total.text();
                response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, &text));
                ui.end_row();
            }
        });
}

/// A label that wraps at the width of the dialog.
pub fn wrapped(ui: &mut Ui, text: impl Into<WidgetText>) -> Response {
    ui.add(Label::new(text).wrap())
}

/// A wrapped note in the weak colour.
pub fn note(ui: &mut Ui, text: impl Into<String>) -> Response {
    wrapped(ui, RichText::new(text).weak())
}

/// "1 change", "3 changes".
pub fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// "a, b and c".
pub fn and_list(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// The frame of a floating window (the diagnostics panel), like a dialog's.
pub fn window_frame(ctx: &Context) -> Frame {
    let style = ctx.global_style();
    let pal = theme::palette(style.visuals.dark_mode);
    Frame::new()
        .fill(pal.surface)
        .stroke(Stroke::new(1.0, pal.border))
        .corner_radius(theme::ROUNDING_LARGE)
        .shadow(style.visuals.window_shadow)
        .inner_margin(Margin::same(PAD - 4))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plurals_lists_and_totals_read_as_text() {
        assert_eq!(plural(1, "change", "changes"), "1 change");
        assert_eq!(plural(0, "change", "changes"), "0 changes");
        assert_eq!(plural(3, "change", "changes"), "3 changes");
        let total = Total {
            name: "Percentage",
            before: "19%".into(),
            after: "108%".into(),
        };
        assert_eq!(total.text(), "Percentage: 19% → 108%");
        assert_eq!(and_list(&["a", "b", "c"]), "a, b and c");
        assert_eq!(and_list(&["a"]), "a");
    }
}
