//! The window's layout class, from its size, and the sizes the layout
//! follows.
//!
//! - **Wide**: the full toolbar on one line, the game tabs, the overview
//!   beside the warp room grid or a level's page.
//! - **Medium**: the same with a compact bar and its ☰ menu.
//! - **Phone**: the compact bar, one page at a time (the overview, or the
//!   rooms) chosen in a bar at the bottom, and dialogs as sheets.

use egui::{Context, Id, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Wide,
    Medium,
    Phone,
}

/// The narrowest window where the full toolbar fits on one line, in its
/// widest state: a Switch save with unsaved changes, every diagnostic
/// severity, and the debug menu.
pub const WIDE_MIN: f32 = 1224.0;
/// Narrower windows are phones.
pub const PHONE_MAX: f32 = 720.0;
/// A touch screen this short, and narrower than `LANDSCAPE_PHONE_WIDTH`, is
/// a phone held sideways.
pub const LANDSCAPE_PHONE_HEIGHT: f32 = 500.0;
pub const LANDSCAPE_PHONE_WIDTH: f32 = 1000.0;

/// The narrowest central area that shows a level's page with its room's
/// card beside the details: the card, a gap, and about as much for the
/// details.
pub const ROOM_COLUMN_MIN: f32 = 760.0;
/// The room's card beside a level's details.
pub const ROOM_CARD_WIDTH: f32 = 370.0;

/// The class of a window of `size` points. By width: an on-screen keyboard
/// can shorten the page. Only a touch screen is classed by its height, so
/// that a short desktop window is never a phone.
pub fn classify(size: Vec2, touch_screen: bool) -> Class {
    let landscape_phone =
        touch_screen && size.y < LANDSCAPE_PHONE_HEIGHT && size.x < LANDSCAPE_PHONE_WIDTH;
    if size.x < PHONE_MAX || landscape_phone {
        Class::Phone
    } else if size.x < WIDE_MIN {
        Class::Medium
    } else {
        Class::Wide
    }
}

fn id() -> Id {
    Id::new("layout-class")
}

/// Sets the class of this frame, for the widgets that adapt to it.
pub fn set(ctx: &Context, class: Class) {
    ctx.data_mut(|d| d.insert_temp(id(), class));
}

/// The class of this frame.
pub fn current(ctx: &Context) -> Class {
    ctx.data(|d| d.get_temp(id())).unwrap_or(Class::Wide)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_are_classed_by_width_and_phones_held_sideways_by_height() {
        let class = |w, h, touch| classify(Vec2::new(w, h), touch);
        for (w, h) in [(360.0, 740.0), (390.0, 844.0), (844.0, 390.0)] {
            assert_eq!(class(w, h, true), Class::Phone, "{w}×{h}");
        }
        assert_eq!(class(768.0, 1024.0, true), Class::Medium);
        assert_eq!(class(1280.0, 800.0, true), Class::Wide);
        assert_eq!(class(1920.0, 1080.0, true), Class::Wide);
        // A short desktop window is not a phone.
        assert_eq!(class(900.0, 480.0, false), Class::Medium);
        assert_eq!(class(844.0, 390.0, false), Class::Medium);
        assert_eq!(class(1024.0, 720.0, false), Class::Medium);
        assert_eq!(class(WIDE_MIN, 720.0, false), Class::Wide);
    }
}
