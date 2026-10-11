//! Colours, spacing, rounding and font sizes for the light and the dark theme.
//! Every colour the editor paints comes from this file.

use egui::{
    Color32, Context, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Theme, Vec2, Visuals,
    vec2,
};
use nsane::crash3::{GemColor, Relic};

/// Base spacing unit, in points.
pub const SPACING: f32 = 8.0;
pub const ITEM_SPACING: Vec2 = vec2(8.0, 6.0);
pub const BUTTON_PADDING: Vec2 = vec2(8.0, 4.0);

pub const ROUNDING: u8 = 6;
pub const ROUNDING_SMALL: u8 = 3;

/// Icon sizes, in points. Icons are drawn to stay readable at all four.
pub const ICON_SMALL: f32 = 16.0;
pub const ICON_MEDIUM: f32 = 24.0;
pub const ICON_LARGE: f32 = 32.0;
pub const ICON_XLARGE: f32 = 48.0;

pub const FONT_SMALL: f32 = 12.0;
pub const FONT_BODY: f32 = 14.0;
pub const FONT_MONO: f32 = 13.0;
pub const FONT_HEADING: f32 = 20.0;

pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);
pub const BLACK: Color32 = Color32::from_rgb(0, 0, 0);

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

// Collectible colours are the same in both themes: they name the item.
const CRYSTAL: Swatch = Swatch::new(0xB04CE0);
const GEM_CLEAR: Swatch = Swatch::new(0xDCEBFF);
const GEM_RED: Swatch = Swatch::new(0xE5383B);
const GEM_YELLOW: Swatch = Swatch::new(0xF2C230);
const GEM_PURPLE: Swatch = Swatch::new(0x8E44AD);
const GEM_BLUE: Swatch = Swatch::new(0x2D7FF9);
const GEM_GREEN: Swatch = Swatch::new(0x2DBE60);
const RELIC_SAPPHIRE: Swatch = Swatch::new(0x2F6FDB);
const RELIC_GOLD: Swatch = Swatch::new(0xE0A526);
const RELIC_PLATINUM: Swatch = Swatch::new(0xD7DEE6);
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
    /// Window and panel background.
    pub background: Color32,
    /// Raised areas: windows, menus, cards.
    pub surface: Color32,
    /// Text fields and other sunken areas.
    pub field: Color32,
    /// Behind a hovered icon toggle.
    pub hover_bg: Color32,
    pub text: Color32,
    pub text_weak: Color32,
    /// Outline of an icon the save doesn't have.
    pub muted: Color32,
    /// Outline of an icon that doesn't apply.
    pub faint: Color32,
    /// Selection, keyboard focus and the hover outline of icons.
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
    background: rgb(0xF4F5F7),
    surface: rgb(0xFFFFFF),
    field: rgb(0xFFFFFF),
    hover_bg: rgb(0xE3E7EC),
    text: rgb(0x1F2328),
    text_weak: rgb(0x59636E),
    muted: rgb(0x8C959F),
    faint: rgb(0xD5DAE0),
    accent: Swatch::new(0x0969DA),
    glyph_light: WHITE,
    glyph_dark: rgb(0x1F2328),
    neutral: Swatch::new(0x9AA5B1),
    warning: Swatch::new(0xE8A317),
    info: Swatch::new(0x2F80ED),
    error: Swatch::new(0xD73A49),
    crystal: CRYSTAL,
    gem_clear: GEM_CLEAR,
    gem_red: GEM_RED,
    gem_yellow: GEM_YELLOW,
    gem_purple: GEM_PURPLE,
    gem_blue: GEM_BLUE,
    gem_green: GEM_GREEN,
    relic_sapphire: RELIC_SAPPHIRE,
    relic_gold: RELIC_GOLD,
    relic_platinum: RELIC_PLATINUM,
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
    background: rgb(0x1B1D21),
    surface: rgb(0x25282D),
    field: rgb(0x121417),
    hover_bg: rgb(0x343840),
    text: rgb(0xE6E8EB),
    text_weak: rgb(0x9BA3AD),
    muted: rgb(0x7D8590),
    faint: rgb(0x3A3F47),
    accent: Swatch::new(0x4C9AFF),
    glyph_light: WHITE,
    glyph_dark: rgb(0x1F2328),
    neutral: Swatch::new(0xAEB8C2),
    warning: Swatch::new(0xF0B429),
    info: Swatch::new(0x4C9AFF),
    error: Swatch::new(0xF85149),
    ..LIGHT
};

pub fn palette(dark: bool) -> &'static Palette {
    if dark { &DARK } else { &LIGHT }
}

/// Switches egui to the light or dark theme and styles it from the palette.
pub fn apply(ctx: &Context, dark: bool) {
    let theme = if dark { Theme::Dark } else { Theme::Light };
    ctx.set_theme(theme);
    ctx.style_mut_of(theme, |style| {
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
            (
                TextStyle::Heading,
                FontId::new(FONT_HEADING, FontFamily::Proportional),
            ),
        ]
        .into();
        style.spacing.item_spacing = ITEM_SPACING;
        style.spacing.button_padding = BUTTON_PADDING;
        style.visuals = visuals(palette(dark));
    });
}

fn visuals(p: &Palette) -> Visuals {
    let mut v = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    v.panel_fill = p.background;
    v.window_fill = p.surface;
    v.faint_bg_color = p.surface;
    v.extreme_bg_color = p.field;
    v.text_edit_bg_color = Some(p.field);
    v.hyperlink_color = p.accent.base;
    v.warn_fg_color = p.warning.base;
    v.error_fg_color = p.error.base;
    v.selection.bg_fill = mix(p.accent.base, p.background, 65);
    v.selection.stroke = Stroke::new(1.0, p.accent.base);
    v.window_corner_radius = CornerRadius::same(ROUNDING);
    v.menu_corner_radius = CornerRadius::same(ROUNDING);
    v.widgets.noninteractive.fg_stroke.color = p.text;
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.corner_radius = CornerRadius::same(ROUNDING_SMALL);
    }
    v.widgets.hovered.weak_bg_fill = p.hover_bg;
    v
}
