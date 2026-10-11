//! Colours, spacing, rounding and fonts for the light and the dark theme.
//! Every colour the editor paints comes from this file.

use std::sync::Arc;

use egui::style::ScrollStyle;
use egui::{
    Button, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin,
    RichText, Shadow, Stroke, TextStyle, Theme, Vec2, Visuals, vec2,
};
use nsane::crash3::{GemColor, Relic};

/// Base spacing unit, in points.
pub const SPACING: f32 = 8.0;
pub const ITEM_SPACING: Vec2 = vec2(8.0, 6.0);
pub const BUTTON_PADDING: Vec2 = vec2(8.0, 4.0);
/// Inside the side panels and the central area.
pub const PANEL_MARGIN: i8 = 12;
/// Inside menus and popups.
pub const MENU_MARGIN: i8 = 8;

pub const ROUNDING: u8 = 6;
pub const ROUNDING_SMALL: u8 = 4;
/// Dialogs and floating windows.
pub const ROUNDING_LARGE: u8 = 10;

/// Icon sizes, in points. Icons are drawn to stay readable at all four.
pub const ICON_SMALL: f32 = 16.0;
pub const ICON_MEDIUM: f32 = 24.0;
pub const ICON_LARGE: f32 = 32.0;
pub const ICON_XLARGE: f32 = 48.0;

pub const FONT_SMALL: f32 = 12.0;
pub const FONT_BODY: f32 = 14.0;
pub const FONT_MONO: f32 = 13.0;
pub const FONT_TITLE: f32 = 15.0;
pub const FONT_HEADING: f32 = 18.0;

pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);

/// The family of bold text: Inter SemiBold, then the same fallbacks as the
/// body text.
pub const BOLD: &str = "bold";

/// Inter 4.1 (SIL Open Font License 1.1, `assets/fonts/OFL.txt`), cut down
/// to Latin and the punctuation the editor uses.
const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");

/// One colour with the tones an icon needs: `light` for highlights and the
/// facets facing the light, `shade` for the facets turned away, `dark` for
/// outlines and details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Swatch {
    pub base: Color32,
    pub light: Color32,
    pub shade: Color32,
    pub dark: Color32,
}

impl Swatch {
    pub const fn new(hex: u32) -> Self {
        let base = rgb(hex);
        Self {
            base,
            light: mix(base, WHITE, 40),
            shade: mix(base, BLACK, 18),
            dark: mix(base, BLACK, 50),
        }
    }

    /// The same swatch with its outline in `hex`.
    pub const fn outlined(self, hex: u32) -> Self {
        Self {
            dark: rgb(hex),
            ..self
        }
    }
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// `a` moved `percent` of the way towards `b` (both opaque).
pub const fn mix(a: Color32, b: Color32, percent: u32) -> Color32 {
    const fn channel(a: u8, b: u8, percent: u32) -> u8 {
        ((a as u32 * (100 - percent) + b as u32 * percent + 50) / 100) as u8
    }
    Color32::from_rgb(
        channel(a.r(), b.r(), percent),
        channel(a.g(), b.g(), percent),
        channel(a.b(), b.b(), percent),
    )
}

/// `c` moved `percent` of the way towards white, keeping its alpha.
pub fn lighten(c: Color32, percent: u32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let m = mix(Color32::from_rgb(r, g, b), WHITE, percent);
    Color32::from_rgba_unmultiplied(m.r(), m.g(), m.b(), a)
}

// Collectible colours name the item, so they are the same in both themes,
// except two: the clear gem and the platinum relic are near white, which
// reads well on a dark card but not on a white one. The light theme draws
// them bluer and darker, with a dark outline.
const CRYSTAL: Swatch = Swatch::new(0xB04CE0);
const GEM_CLEAR: Swatch = Swatch::new(0xDCEBFF);
const GEM_CLEAR_ON_LIGHT: Swatch = Swatch::new(0x9CC9F5).outlined(0x2B5A87);
const GEM_RED: Swatch = Swatch::new(0xE5383B);
const GEM_YELLOW: Swatch = Swatch::new(0xF2C230);
const GEM_PURPLE: Swatch = Swatch::new(0x8E44AD);
const GEM_BLUE: Swatch = Swatch::new(0x2D7FF9);
const GEM_GREEN: Swatch = Swatch::new(0x2DBE60);
const RELIC_SAPPHIRE: Swatch = Swatch::new(0x2F6FDB);
const RELIC_GOLD: Swatch = Swatch::new(0xE0A526);
const RELIC_PLATINUM: Swatch = Swatch::new(0xD7DEE6);
const RELIC_PLATINUM_ON_LIGHT: Swatch = Swatch::new(0xB3BDC8).outlined(0x46505B);
const SPARKLE: Swatch = Swatch::new(0xFFD34D);
const ABILITY: Swatch = Swatch::new(0xF28C28);
const LIVES: Swatch = Swatch::new(0xE5383B);
const BOSS_ACCENT: Swatch = Swatch::new(0xD93A3A);
const RELIC_JEWEL: Swatch = Swatch::new(0xD7263D);
const FRUIT: Swatch = Swatch::new(0xE5532D);
// Boss symbols.
const STEEL: Swatch = Swatch::new(0xA9B4BE);
const TIGER: Swatch = Swatch::new(0xE8892B);
const CROC: Swatch = Swatch::new(0x5E8C3A);
const FLAME: Swatch = Swatch::new(0xF26B1D);
const FLAME_CORE: Swatch = Swatch::new(0xFFC93C);
const BRONZE: Swatch = Swatch::new(0xB5813E);
const DIAL: Swatch = Swatch::new(0x5DBB7A);
const MISSILE: Swatch = Swatch::new(0xD5DCE3);
const CORTEX: Swatch = Swatch::new(0xF2C94C);

/// The colours of one theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    /// The toolbar, the game tabs and the side panels.
    pub chrome: Color32,
    /// Behind the warp room cards.
    pub canvas: Color32,
    /// Raised areas: cards, dialogs, menus, resting buttons.
    pub surface: Color32,
    /// Text fields and other sunken areas.
    pub field: Color32,
    /// Card edges, separators, the outline of dialogs and resting buttons.
    pub border: Color32,
    /// Outlines that must stand out: hovered and pressed controls.
    pub border_strong: Color32,
    /// Behind a hovered control or icon toggle.
    pub hover_bg: Color32,
    /// Behind a pressed control.
    pub pressed_bg: Color32,
    /// Behind a selected tab, name or badge, with `selected_text` on it.
    pub selection: Color32,
    pub selected_text: Color32,
    /// The confirm button of a dialog, with `glyph_light` text.
    pub primary: Color32,
    pub primary_hover: Color32,
    /// A button that removes progress, with `glyph_light` text.
    pub danger: Color32,
    pub danger_hover: Color32,
    pub text: Color32,
    /// Secondary text, and the names of levels never entered.
    pub text_weak: Color32,
    /// Messages in the colour of their severity, readable on every
    /// background.
    pub error_text: Color32,
    pub warning_text: Color32,
    /// A good outcome ("No game data lost").
    pub success_text: Color32,
    /// Outline of an icon the save doesn't have.
    pub muted: Color32,
    /// Outline of an icon that doesn't apply.
    pub faint: Color32,
    /// Keyboard focus, the progress bar and the hover outline of icons.
    pub accent: Swatch,
    /// Glyphs on a dark or saturated fill ("i", "×").
    pub glyph_light: Color32,
    /// Glyphs on a light fill ("!").
    pub glyph_dark: Color32,
    /// Lock, cutscene, timer and the boss mask.
    pub neutral: Swatch,
    pub warning: Swatch,
    pub info: Swatch,
    pub error: Swatch,
    pub crystal: Swatch,
    pub gem_clear: Swatch,
    pub gem_red: Swatch,
    pub gem_yellow: Swatch,
    pub gem_purple: Swatch,
    pub gem_blue: Swatch,
    pub gem_green: Swatch,
    pub relic_sapphire: Swatch,
    pub relic_gold: Swatch,
    pub relic_platinum: Swatch,
    pub sparkle: Swatch,
    pub ability: Swatch,
    pub lives: Swatch,
    /// The red parts of boss symbols (the missile's nose and band).
    pub boss_accent: Swatch,
    /// Boss symbols: the trident and the tuning fork's steel, Tiny Tiger's
    /// striped grip, Dingodile's nozzle, the flame, N. Tropy's bronze clock
    /// and green dial, N. Gin's missile and Cortex's yellow badge.
    pub steel: Swatch,
    pub tiger: Swatch,
    pub croc: Swatch,
    pub flame: Swatch,
    pub flame_core: Swatch,
    pub bronze: Swatch,
    pub dial: Swatch,
    pub missile: Swatch,
    pub cortex: Swatch,
    /// The jewel in a relic's loop, the same for every tier.
    pub relic_jewel: Swatch,
    /// The Fruit Bazooka's fruit.
    pub fruit: Swatch,
}

impl Palette {
    pub fn gem(&self, color: GemColor) -> Swatch {
        match color {
            GemColor::Red => self.gem_red,
            GemColor::Yellow => self.gem_yellow,
            GemColor::Purple => self.gem_purple,
            GemColor::Blue => self.gem_blue,
            GemColor::Green => self.gem_green,
        }
    }

    pub fn relic(&self, relic: Relic) -> Swatch {
        match relic {
            Relic::Sapphire => self.relic_sapphire,
            Relic::Gold => self.relic_gold,
            Relic::Platinum => self.relic_platinum,
        }
    }
}

pub const LIGHT: Palette = Palette {
    dark: false,
    chrome: rgb(0xF6F8FA),
    canvas: rgb(0xEAEEF2),
    surface: rgb(0xFFFFFF),
    field: rgb(0xFFFFFF),
    border: rgb(0xD0D7DE),
    border_strong: rgb(0x6E7781),
    hover_bg: rgb(0xE2E7EC),
    pressed_bg: rgb(0xD5DBE2),
    selection: rgb(0xDAEAFE),
    selected_text: rgb(0x0550AE),
    primary: rgb(0x0969DA),
    primary_hover: rgb(0x0858BA),
    danger: rgb(0xCF222E),
    danger_hover: rgb(0xA40E26),
    text: rgb(0x1F2328),
    text_weak: rgb(0x59636E),
    error_text: rgb(0xCF222E),
    warning_text: rgb(0x8A5C00),
    success_text: rgb(0x146C2E),
    muted: rgb(0x848D97),
    faint: rgb(0xD5DAE0),
    accent: Swatch::new(0x0969DA),
    glyph_light: WHITE,
    glyph_dark: rgb(0x1F2328),
    neutral: Swatch::new(0x9AA5B1),
    warning: Swatch::new(0xE8A317),
    info: Swatch::new(0x2F80ED),
    error: Swatch::new(0xD73A49),
    crystal: CRYSTAL,
    gem_clear: GEM_CLEAR_ON_LIGHT,
    gem_red: GEM_RED,
    gem_yellow: GEM_YELLOW,
    gem_purple: GEM_PURPLE,
    gem_blue: GEM_BLUE,
    gem_green: GEM_GREEN,
    relic_sapphire: RELIC_SAPPHIRE,
    relic_gold: RELIC_GOLD,
    relic_platinum: RELIC_PLATINUM_ON_LIGHT,
    sparkle: SPARKLE,
    ability: ABILITY,
    lives: LIVES,
    boss_accent: BOSS_ACCENT,
    steel: STEEL,
    tiger: TIGER,
    croc: CROC,
    flame: FLAME,
    flame_core: FLAME_CORE,
    bronze: BRONZE,
    dial: DIAL,
    missile: MISSILE,
    cortex: CORTEX,
    relic_jewel: RELIC_JEWEL,
    fruit: FRUIT,
};

pub const DARK: Palette = Palette {
    dark: true,
    chrome: rgb(0x1C1F23),
    canvas: rgb(0x15171A),
    surface: rgb(0x24282E),
    field: rgb(0x121417),
    border: rgb(0x373C44),
    border_strong: rgb(0x6E7681),
    hover_bg: rgb(0x30353D),
    pressed_bg: rgb(0x3A4049),
    selection: rgb(0x2F4869),
    selected_text: rgb(0x8CC4FF),
    primary: rgb(0x1F6FEB),
    primary_hover: rgb(0x1A5FD0),
    danger: rgb(0xDA3633),
    danger_hover: rgb(0xB62324),
    text: rgb(0xE6E8EB),
    text_weak: rgb(0x9BA3AD),
    error_text: rgb(0xFF7B72),
    warning_text: rgb(0xD29922),
    success_text: rgb(0x56D364),
    muted: rgb(0x7D8590),
    faint: rgb(0x3A3F47),
    accent: Swatch::new(0x4C9AFF),
    glyph_light: WHITE,
    glyph_dark: rgb(0x1F2328),
    neutral: Swatch::new(0xAEB8C2),
    warning: Swatch::new(0xF0B429),
    info: Swatch::new(0x4C9AFF),
    error: Swatch::new(0xF85149),
    gem_clear: GEM_CLEAR,
    relic_platinum: RELIC_PLATINUM,
    ..LIGHT
};

pub fn palette(dark: bool) -> &'static Palette {
    if dark { &DARK } else { &LIGHT }
}

/// Text in the bold family, at the size of its text style.
pub fn bold(text: impl Into<String>) -> RichText {
    RichText::new(text).family(FontFamily::Name(BOLD.into()))
}

/// The confirm action of a dialog: accent fill, white text.
pub fn primary_button(ui: &egui::Ui, text: &str) -> Button<'static> {
    filled_button(ui, text, |p| (p.primary, p.primary_hover))
}

/// An action that removes progress: red fill, white text.
pub fn danger_button(ui: &egui::Ui, text: &str) -> Button<'static> {
    filled_button(ui, text, |p| (p.danger, p.danger_hover))
}

fn filled_button(
    ui: &egui::Ui,
    text: &str,
    colors: fn(&Palette) -> (Color32, Color32),
) -> Button<'static> {
    let pal = palette(ui.visuals().dark_mode);
    let (fill, hover) = colors(pal);
    // The hover tone needs the pointer of the last frame: a button has no
    // hover fill of its own.
    let id = ui.next_auto_id();
    let hovered = ui
        .ctx()
        .read_response(id)
        .is_some_and(|r| r.hovered() && r.enabled());
    let fill = if hovered { hover } else { fill };
    Button::new(bold(text).color(pal.glyph_light))
        .fill(fill)
        .stroke(Stroke::new(1.0, fill))
}

/// The proportional text: Inter, then fonts with other scripts (file names,
/// system errors), arrows and emoji. Called once, at start; fonts apply on
/// the next pass.
pub fn fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    for (name, data) in [("Inter", INTER_REGULAR), ("Inter-SemiBold", INTER_SEMIBOLD)] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(data)));
    }
    let family = |first: &str| -> Vec<String> {
        [
            first,
            "Ubuntu-Light",
            "Hack",
            "NotoEmoji-Regular",
            "emoji-icon-font",
        ]
        .map(str::to_owned)
        .to_vec()
    };
    fonts
        .families
        .insert(FontFamily::Proportional, family("Inter"));
    fonts
        .families
        .insert(FontFamily::Name(BOLD.into()), family("Inter-SemiBold"));
    ctx.set_fonts(fonts);
}

/// Switches egui to the light or dark theme and styles it from the palette.
pub fn apply(ctx: &Context, dark: bool) {
    let theme = if dark { Theme::Dark } else { Theme::Light };
    ctx.set_theme(theme);
    ctx.style_mut_of(theme, |style| {
        let bold = FontFamily::Name(BOLD.into());
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(FONT_SMALL, FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                FontId::new(FONT_BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(FONT_BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(FONT_MONO, FontFamily::Monospace),
            ),
            (TextStyle::Heading, FontId::new(FONT_HEADING, bold)),
        ]
        .into();
        let spacing = &mut style.spacing;
        spacing.item_spacing = ITEM_SPACING;
        spacing.button_padding = BUTTON_PADDING;
        spacing.interact_size.y = 24.0;
        spacing.menu_margin = Margin::same(MENU_MARGIN);
        spacing.window_margin = Margin::same(16);
        // A bar of its own beside the content, so that it never covers
        // the numbers on the right of a panel.
        spacing.scroll = ScrollStyle::solid();
        style.visuals = visuals(palette(dark));
    });
}

fn visuals(p: &Palette) -> Visuals {
    let mut v = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = p.chrome;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.faint_bg_color = mix(p.chrome, p.text, 4);
    v.extreme_bg_color = p.field;
    v.text_edit_bg_color = Some(p.field);
    v.code_bg_color = p.canvas;
    v.weak_text_color = Some(p.text_weak);
    v.hyperlink_color = p.selected_text;
    v.warn_fg_color = p.warning_text;
    v.error_fg_color = p.error_text;
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0, p.selected_text);
    // The diagnostics panel's title on the window's own colour, like a
    // dialog's.
    v.window_highlight_topmost = false;
    v.window_corner_radius = CornerRadius::same(ROUNDING_LARGE);
    v.menu_corner_radius = CornerRadius::same(ROUNDING);
    let shadow = |y: i8, blur: u8, alpha: u8| Shadow {
        offset: [0, y],
        blur,
        spread: 0,
        color: Color32::from_black_alpha(alpha),
    };
    v.window_shadow = if p.dark {
        shadow(8, 24, 110)
    } else {
        shadow(8, 24, 40)
    };
    v.popup_shadow = if p.dark {
        shadow(4, 12, 90)
    } else {
        shadow(4, 12, 32)
    };

    let line = |color| Stroke::new(1.0, color);
    let w = &mut v.widgets;
    // Labels and separators.
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = line(p.border);
    w.noninteractive.fg_stroke = line(p.text);
    // Buttons fill with the weak colour. The other fill is the box of
    // checkboxes and radios and the handle of scroll bars: a gray that shows
    // on the track (`field`) and leaves the check mark readable.
    w.inactive.weak_bg_fill = p.surface;
    w.inactive.bg_fill = p.border;
    w.inactive.bg_stroke = line(p.border);
    w.inactive.fg_stroke = line(p.text);
    w.hovered.weak_bg_fill = p.hover_bg;
    w.hovered.bg_fill = mix(p.border, p.border_strong, 30);
    w.hovered.bg_stroke = line(p.border_strong);
    w.hovered.fg_stroke = Stroke::new(1.5, p.text);
    w.active.weak_bg_fill = p.pressed_bg;
    w.active.bg_fill = mix(p.border, p.border_strong, 50);
    w.active.bg_stroke = line(p.border_strong);
    w.active.fg_stroke = Stroke::new(2.0, p.text);
    w.open.weak_bg_fill = p.hover_bg;
    w.open.bg_fill = p.hover_bg;
    w.open.bg_stroke = line(p.border_strong);
    w.open.fg_stroke = line(p.text);
    for w in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        w.corner_radius = CornerRadius::same(ROUNDING_SMALL);
        // Controls keep their size when hovered or pressed.
        w.expansion = 0.0;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The WCAG 2 relative luminance of an opaque colour.
    fn luminance(c: Color32) -> f32 {
        let channel = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }

    /// The WCAG 2 contrast ratio of two opaque colours, from 1 to 21.
    fn ratio(a: Color32, b: Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// How far apart an owned collectible's main fill and an empty slot
    /// (the card behind it) must be, so that owned and missing differ at a
    /// glance. Not a WCAG level: the outline and the tooltip carry the rest.
    const OWNED_VS_MISSING: f32 = 1.5;

    #[test]
    fn ratios_match_the_wcag_examples() {
        assert!((ratio(BLACK, WHITE) - 21.0).abs() < 0.01);
        assert!((ratio(rgb(0x777777), WHITE) - 4.48).abs() < 0.01);
    }

    /// Text reaches 4.5:1 on every background it is drawn on; outlines that
    /// carry meaning reach 3:1; owned collectibles stand apart from empty
    /// slots. In both themes.
    #[test]
    fn contrast() {
        let mut failures = Vec::new();
        let mut check = |what: String, fg: Color32, bg: Color32, min: f32| {
            let r = ratio(fg, bg);
            if r < min {
                failures.push(format!("{what}: {r:.2} < {min}"));
            }
        };
        for p in [&LIGHT, &DARK] {
            let theme = if p.dark { "dark" } else { "light" };
            let backgrounds = [
                ("chrome", p.chrome),
                ("canvas", p.canvas),
                ("surface", p.surface),
                ("field", p.field),
            ];
            for (bg_name, bg) in backgrounds {
                for (name, fg) in [
                    ("text", p.text),
                    ("text_weak", p.text_weak),
                    ("error_text", p.error_text),
                    ("warning_text", p.warning_text),
                    ("success_text", p.success_text),
                    ("selected_text", p.selected_text),
                ] {
                    check(format!("{theme} {name} on {bg_name}"), fg, bg, 4.5);
                }
            }
            for (bg_name, bg) in [
                ("hover_bg", p.hover_bg),
                ("pressed_bg", p.pressed_bg),
                ("selection", p.selection),
            ] {
                check(format!("{theme} text on {bg_name}"), p.text, bg, 4.5);
            }
            check(
                format!("{theme} text_weak on hover_bg"),
                p.text_weak,
                p.hover_bg,
                4.5,
            );
            check(
                format!("{theme} selected_text on selection"),
                p.selected_text,
                p.selection,
                4.5,
            );
            for (name, fill) in [
                ("primary", p.primary),
                ("primary_hover", p.primary_hover),
                ("danger", p.danger),
                ("danger_hover", p.danger_hover),
            ] {
                check(
                    format!("{theme} glyph_light on {name}"),
                    p.glyph_light,
                    fill,
                    4.5,
                );
            }

            // Outlines against what surrounds them: hovered and pressed
            // controls (the tabs of the central area are on the canvas), and
            // the empty slot of a collectible, on the cards and the panels.
            // A hovered empty slot is drawn in the text colour.
            for (bg_name, bg) in [
                ("chrome", p.chrome),
                ("canvas", p.canvas),
                ("surface", p.surface),
            ] {
                check(
                    format!("{theme} border_strong on {bg_name}"),
                    p.border_strong,
                    bg,
                    3.0,
                );
            }
            for (bg_name, bg) in [("chrome", p.chrome), ("surface", p.surface)] {
                check(format!("{theme} muted on {bg_name}"), p.muted, bg, 3.0);
            }
            // Focus rings and selected text fields.
            check(
                format!("{theme} accent on surface"),
                p.accent.base,
                p.surface,
                3.0,
            );

            let collectibles = [
                ("crystal", p.crystal),
                ("gem_clear", p.gem_clear),
                ("gem_red", p.gem_red),
                ("gem_yellow", p.gem_yellow),
                ("gem_purple", p.gem_purple),
                ("gem_blue", p.gem_blue),
                ("gem_green", p.gem_green),
                ("relic_sapphire", p.relic_sapphire),
                ("relic_gold", p.relic_gold),
                ("relic_platinum", p.relic_platinum),
            ];
            for (bg_name, bg) in [("surface", p.surface), ("chrome", p.chrome)] {
                for (name, s) in collectibles {
                    check(
                        format!("{theme} owned {name} fill on {bg_name}"),
                        s.base,
                        bg,
                        OWNED_VS_MISSING,
                    );
                    // On white, the outline draws the shape; on a dark card
                    // the fill does.
                    if !p.dark {
                        check(
                            format!("{theme} owned {name} outline on {bg_name}"),
                            s.dark,
                            bg,
                            3.0,
                        );
                    }
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    /// The cards stand out from the area behind them, and the panels from
    /// both.
    #[test]
    fn surfaces_are_apart() {
        for p in [&LIGHT, &DARK] {
            assert!(ratio(p.surface, p.canvas) >= 1.15, "{}", p.dark);
            assert_ne!(p.chrome, p.canvas);
            assert_ne!(p.hover_bg, p.surface);
        }
    }
}
