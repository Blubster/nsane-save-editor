//! Icons drawn with egui's painter: no image files and no game art. Each icon
//! is a few shapes in coordinates relative to its square (0 to 1 on both axes,
//! y pointing down), so it stays sharp from 16 to 48 points. Colours come from
//! `theme.rs`.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use egui::epaint::PathShape;
use egui::{
    Color32, CornerRadius, CursorIcon, Mesh, Painter, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, WidgetInfo, WidgetType, vec2,
};
use nsane::crash3::{ABILITIES, Ability, BOSSES, GemColor, LevelId, Relic};

use crate::theme::{self, Palette, Swatch};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Crystal,
    ClearGem,
    /// The gem the hub gives for relics. A level's second clear gem uses
    /// `ClearGem`; its tooltip says which gem it is.
    BonusGem,
    ColoredGem(GemColor),
    Relic(Relic),
    Boss(Boss),
    Ability(Ability),
    Lock,
    Unlock,
    Warning,
    Info,
    Error,
    Cutscene,
    Lives,
    Timer,
}

impl Icon {
    /// Every icon, in the gallery's order.
    pub fn all() -> Vec<Icon> {
        let mut all = vec![Icon::Crystal, Icon::ClearGem, Icon::BonusGem];
        all.extend(GemColor::ALL.map(Icon::ColoredGem));
        all.extend(Relic::ALL.map(Icon::Relic));
        all.extend(Boss::ALL.map(Icon::Boss));
        all.extend(ABILITIES.map(Icon::Ability));
        all.extend([
            Icon::Lock,
            Icon::Unlock,
            Icon::Warning,
            Icon::Info,
            Icon::Error,
            Icon::Cutscene,
            Icon::Lives,
            Icon::Timer,
        ]);
        all
    }

    /// A short English name, for tooltips and screen readers. Colored gems and
    /// relic tiers are always named, so they never rely on hue alone.
    pub fn name(self) -> &'static str {
        match self {
            Icon::Crystal => "Crystal",
            Icon::ClearGem => "Clear gem",
            Icon::BonusGem => "Bonus gem",
            Icon::ColoredGem(GemColor::Red) => "Red gem",
            Icon::ColoredGem(GemColor::Yellow) => "Yellow gem",
            Icon::ColoredGem(GemColor::Purple) => "Purple gem",
            Icon::ColoredGem(GemColor::Blue) => "Blue gem",
            Icon::ColoredGem(GemColor::Green) => "Green gem",
            Icon::Relic(Relic::Sapphire) => "Sapphire relic",
            Icon::Relic(Relic::Gold) => "Gold relic",
            Icon::Relic(Relic::Platinum) => "Platinum relic",
            Icon::Boss(b) => b.level().name(),
            Icon::Ability(a) => a.name(),
            Icon::Lock => "Locked",
            Icon::Unlock => "Unlocked",
            Icon::Warning => "Warning",
            Icon::Info => "Information",
            Icon::Error => "Error",
            Icon::Cutscene => "Cutscene",
            Icon::Lives => "Lives",
            Icon::Timer => "Time",
        }
    }
}

/// The five bosses, one symbol each: an object from the fight, never the
/// character.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Boss {
    TinyTiger,
    Dingodile,
    NTropy,
    NGin,
    Cortex,
}

impl Boss {
    /// In warp room order, like `BOSSES`.
    pub const ALL: [Boss; 5] = [
        Boss::TinyTiger,
        Boss::Dingodile,
        Boss::NTropy,
        Boss::NGin,
        Boss::Cortex,
    ];

    pub fn level(self) -> LevelId {
        BOSSES[self as usize]
    }

    /// The boss whose fight `level` is, if it is one.
    pub fn of(level: LevelId) -> Option<Boss> {
        let i = BOSSES.iter().position(|&b| b == level)?;
        Some(Boss::ALL[i])
    }
}

/// Whether the save has the thing the icon stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Presence {
    /// Full colour.
    Owned,
    /// Outline only, in the theme's muted colour.
    Missing,
    /// A very faint outline: the item doesn't exist here (no crystal in a
    /// secret level, for example). Not clickable.
    NotApplicable,
}

/// How to draw an icon. Hover combines with any presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IconState {
    pub presence: Presence,
    /// Brighter, with the outline in the accent colour. Ignored for
    /// `NotApplicable`, which can't be clicked.
    pub hovered: bool,
}

impl IconState {
    pub const OWNED: IconState = IconState::new(Presence::Owned);
    pub const MISSING: IconState = IconState::new(Presence::Missing);
    pub const NOT_APPLICABLE: IconState = IconState::new(Presence::NotApplicable);

    pub const fn new(presence: Presence) -> Self {
        Self {
            presence,
            hovered: false,
        }
    }

    pub const fn hovered(self, hovered: bool) -> Self {
        Self { hovered, ..self }
    }
}

/// Paints `icon` centred in the largest square that fits in `rect`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, state: IconState, dark: bool) {
    let side = rect.width().min(rect.height());
    if side <= 0.0 {
        return;
    }
    let pen = Pen {
        painter,
        rect: Rect::from_center_size(rect.center(), Vec2::splat(side)),
        pal: theme::palette(dark),
        presence: state.presence,
        hovered: state.hovered && state.presence != Presence::NotApplicable,
        pixel: 1.0 / painter.pixels_per_point(),
        side,
    };
    match icon {
        Icon::Crystal => crystal(&pen),
        Icon::ClearGem => gem(&pen, pen.pal.gem_clear),
        Icon::BonusGem => bonus_gem(&pen),
        Icon::ColoredGem(color) => gem(&pen, pen.pal.gem(color)),
        Icon::Relic(relic) => relic_icon(&pen, relic),
        Icon::Boss(b) => boss(&pen, b),
        Icon::Ability(Ability::BodySlam) => body_slam(&pen),
        Icon::Ability(Ability::DoubleJump) => double_jump(&pen),
        Icon::Ability(Ability::TornadoSpin) => tornado_spin(&pen),
        Icon::Ability(Ability::Bazooka) => bazooka(&pen),
        Icon::Ability(Ability::SpeedShoes) => speed_shoes(&pen),
        Icon::Lock => padlock(&pen, false),
        Icon::Unlock => padlock(&pen, true),
        Icon::Warning => warning(&pen),
        Icon::Info => info(&pen),
        Icon::Error => error(&pen),
        Icon::Cutscene => cutscene(&pen),
        Icon::Lives => lives(&pen),
        Icon::Timer => timer(&pen),
    }
}

/// A clickable icon of `size` points: owned when `on`, missing otherwise.
/// When `enabled` is false it doesn't react, and an icon that is off is drawn
/// as not applicable. The caller adds the tooltip with `on_hover_text` and
/// acts on `clicked()`.
pub fn icon_toggle(ui: &mut Ui, icon: Icon, on: bool, enabled: bool, size: f32) -> Response {
    icon_toggle_named(ui, icon, icon.name(), on, enabled, size)
}

/// [`icon_toggle`] with the name screen readers and tests find it by, such
/// as "Deep Trouble: crystal", where several of the same icon are shown.
pub fn icon_toggle_named(
    ui: &mut Ui,
    icon: Icon,
    name: &str,
    on: bool,
    enabled: bool,
    size: f32,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), sense);
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, on, name));
    if ui.is_rect_visible(rect) {
        let dark = ui.visuals().dark_mode;
        let pal = theme::palette(dark);
        let hovered = enabled && (response.hovered() || response.has_focus());
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
        let presence = match (on, enabled) {
            (true, _) => Presence::Owned,
            (false, true) => Presence::Missing,
            (false, false) => Presence::NotApplicable,
        };
        let mut painter = ui.painter().clone();
        if !enabled {
            painter.multiply_opacity(0.5);
        }
        let state = IconState::new(presence).hovered(hovered);
        paint(&painter, rect, icon, state, dark);
    }
    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

/// The least clickable square of an [`IconButton`].
pub const MIN_HIT: f32 = 24.0;

/// An icon that acts when clicked instead of toggling: a lock that opens a
/// dialog, a boss that selects itself. The clickable square may be larger
/// than the icon, which is centred in it. Add it with `ui.add`; the caller
/// adds the tooltip and acts on `clicked()`.
pub struct IconButton<'a> {
    icon: Icon,
    name: &'a str,
    owned: bool,
    selected: Option<bool>,
    size: f32,
    hit: f32,
}

impl<'a> IconButton<'a> {
    /// `name` is what screen readers and tests find it by.
    pub fn new(icon: Icon, name: &'a str) -> Self {
        Self {
            icon,
            name,
            owned: true,
            selected: None,
            size: theme::ICON_SMALL,
            hit: MIN_HIT,
        }
    }

    /// Drawn as missing when `owned` is false, like an empty slot.
    pub fn owned(mut self, owned: bool) -> Self {
        self.owned = owned;
        self
    }

    /// A button that selects something: highlighted while it is selected.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// The icon's size, and the clickable square around it: at least the
    /// icon.
    pub fn size(mut self, size: f32, hit: f32) -> Self {
        self.size = size;
        self.hit = hit.max(size);
        self
    }
}

impl egui::Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(self.hit), Sense::click());
        response.widget_info(|| match self.selected {
            Some(selected) => WidgetInfo::selected(WidgetType::Button, true, selected, self.name),
            None => WidgetInfo::labeled(WidgetType::Button, true, self.name),
        });
        if ui.is_rect_visible(rect) {
            let dark = ui.visuals().dark_mode;
            let pal = theme::palette(dark);
            let hovered = response.hovered() || response.has_focus();
            let icon = Rect::from_center_size(rect.center(), Vec2::splat(self.size));
            // At least 2 points around the icon, as around a toggle.
            let back = rect.union(icon.expand(2.0));
            let radius = CornerRadius::same(theme::ROUNDING_SMALL);
            if self.selected == Some(true) {
                ui.painter().rect_filled(back, radius, pal.selection);
            } else if hovered {
                ui.painter().rect_filled(back, radius, pal.hover_bg);
            }
            if response.has_focus() {
                ui.painter().rect_stroke(
                    back,
                    radius,
                    Stroke::new(1.0, pal.accent.base),
                    StrokeKind::Inside,
                );
            }
            let presence = if self.owned {
                Presence::Owned
            } else {
                Presence::Missing
            };
            let state = IconState::new(presence).hovered(hovered);
            paint(ui.painter(), icon, self.icon, state, dark);
        }
        response.on_hover_cursor(CursorIcon::PointingHand)
    }
}

type P = [f32; 2];

/// Draws in the icon's unit square, mapping colours by state.
struct Pen<'a> {
    painter: &'a Painter,
    rect: Rect,
    pal: &'static Palette,
    presence: Presence,
    hovered: bool,
    /// One physical pixel, in points: the thinnest line drawn.
    pixel: f32,
    /// The size the icon is drawn at, even inside a `sub` pen, so that line
    /// widths and detail levels follow the whole icon.
    side: f32,
}

impl Pen<'_> {
    fn p(&self, [x, y]: P) -> Pos2 {
        self.rect.min + vec2(x, y) * self.rect.width()
    }

    fn ps(&self, points: &[P]) -> Vec<Pos2> {
        points.iter().map(|&p| self.p(p)).collect()
    }

    /// A pen drawing into the square at `[x, y]` of size `scale`.
    fn sub(&self, [x, y]: P, scale: f32) -> Pen<'_> {
        Pen {
            rect: Rect::from_min_size(self.p([x, y]), Vec2::splat(self.rect.width() * scale)),
            ..*self
        }
    }

    /// Small icons skip the finer facet lines, which would blur into the fill.
    fn small(&self) -> bool {
        self.side < 20.0
    }

    /// A width given as a fraction of the icon, at least one physical pixel.
    fn width(&self, fraction: f32) -> f32 {
        (fraction * self.side).max(self.pixel)
    }

    /// The width of contours.
    fn contour_width(&self) -> f32 {
        self.width(0.055)
    }

    /// Fill colours exist only in full colour. Hovering a missing icon shows
    /// a faint preview of what a click would give.
    fn fill_color(&self, c: Color32) -> Color32 {
        match (self.presence, self.hovered) {
            (Presence::Owned, false) => c,
            (Presence::Owned, true) => theme::lighten(c, 22),
            (Presence::Missing, true) => c.gamma_multiply(0.3),
            _ => Color32::TRANSPARENT,
        }
    }

    /// The colour of lines and glyphs.
    fn ink(&self, c: Color32) -> Color32 {
        match self.presence {
            Presence::Owned => c,
            Presence::Missing if self.hovered => self.pal.text,
            Presence::Missing => self.pal.muted,
            Presence::NotApplicable => self.pal.faint,
        }
    }

    fn contour_color(&self, c: Color32) -> Color32 {
        if self.hovered {
            self.pal.accent.base
        } else {
            self.ink(c)
        }
    }

    fn owned(&self) -> bool {
        self.presence == Presence::Owned
    }

    /// Fills a convex polygon. Fills are meshes without feathering, so the
    /// pieces of a concave figure meet without seams; the contour drawn on
    /// top smooths the edges.
    fn fill(&self, points: &[P], c: Color32) {
        let c = self.fill_color(c);
        if c == Color32::TRANSPARENT || points.len() < 3 {
            return;
        }
        let mut mesh = Mesh::default();
        for &p in points {
            mesh.colored_vertex(self.p(p), c);
        }
        for i in 1..points.len() as u32 - 1 {
            mesh.add_triangle(0, i, i + 1);
        }
        self.painter.add(Shape::mesh(mesh));
    }

    /// Fills a polygon in which every point is visible from `center`.
    fn fill_fan(&self, center: P, points: &[P], c: Color32) {
        let c = self.fill_color(c);
        if c == Color32::TRANSPARENT || points.len() < 2 {
            return;
        }
        let mut mesh = Mesh::default();
        mesh.colored_vertex(self.p(center), c);
        for &p in points {
            mesh.colored_vertex(self.p(p), c);
        }
        let n = points.len() as u32;
        for i in 0..n {
            mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
        }
        self.painter.add(Shape::mesh(mesh));
    }

    /// The closed outline of a figure.
    fn contour(&self, points: &[P], c: Color32) {
        let stroke = Stroke::new(self.contour_width(), self.contour_color(c));
        self.painter
            .add(PathShape::closed_line(self.ps(points), stroke));
    }

    /// Open lines for details and glyphs; `width` is a fraction of the icon.
    fn line(&self, points: &[P], c: Color32, width: f32) {
        let stroke = Stroke::new(self.width(width), self.ink(c));
        self.painter.add(PathShape::line(self.ps(points), stroke));
    }

    /// A detail line, thinner than a contour. Kept at small sizes.
    fn detail(&self, points: &[P], c: Color32) {
        self.line(points, c, 0.035);
    }

    /// A facet line, dropped at small sizes.
    fn facet(&self, points: &[P], c: Color32) {
        if !self.small() {
            self.detail(points, c);
        }
    }

    /// A figure made of overlapping convex parts. In full colour, all the
    /// outlines go under all the fills, so only the outer edge shows.
    fn union(&self, parts: &[&[P]], fill: Color32, edge: Color32) {
        if self.owned() || self.hovered {
            let stroke = Stroke::new(2.0 * self.contour_width(), self.contour_color(edge));
            for part in parts {
                self.painter
                    .add(PathShape::closed_line(self.ps(part), stroke));
            }
            for part in parts {
                self.fill(part, fill);
            }
        } else {
            for part in parts {
                self.contour(part, edge);
            }
        }
    }

    /// The outline of a symbol drawn on a filled icon, so it stays visible
    /// when the icon isn't in full colour.
    fn emblem_outline(&self, points: &[P], c: Color32) {
        if !self.owned() {
            let stroke = Stroke::new(self.width(0.035), self.ink(c));
            self.painter
                .add(PathShape::closed_line(self.ps(points), stroke));
        }
    }

    fn disc(&self, center: P, r: f32, c: Color32) {
        let c = self.fill_color(c);
        if c != Color32::TRANSPARENT {
            self.painter
                .circle_filled(self.p(center), r * self.rect.width(), c);
        }
    }

    fn ring(&self, center: P, r: f32, c: Color32) {
        let stroke = Stroke::new(self.contour_width(), self.contour_color(c));
        self.painter
            .circle_stroke(self.p(center), r * self.rect.width(), stroke);
    }

    /// A filled dot in the ink colour: part of a glyph, so it stays visible
    /// in every state.
    fn dot(&self, center: P, r: f32, c: Color32) {
        let r = (r * self.rect.width()).max(self.pixel);
        self.painter.circle_filled(self.p(center), r, self.ink(c));
    }

    /// A thick stroke such as a spiral or a shackle: outlined in full colour,
    /// a thin line otherwise.
    fn band(&self, points: &[P], width: f32, fill: Color32, edge: Color32) {
        let points = self.ps(points);
        if self.owned() || self.hovered {
            let w = self.width(width);
            let outline = Stroke::new(w + 2.0 * self.contour_width(), self.contour_color(edge));
            self.painter.add(PathShape::line(points.clone(), outline));
            let inner = self.fill_color(fill);
            self.painter
                .add(PathShape::line(points, Stroke::new(w, inner)));
        } else {
            let stroke = Stroke::new(self.contour_width(), self.ink(edge));
            self.painter.add(PathShape::line(points, stroke));
        }
    }
}

/// Points on a circle, `n` of them, starting at `start` radians (0 = right,
/// clockwise on screen since y points down).
fn polygon(center: P, r: f32, n: usize, start: f32) -> Vec<P> {
    (0..n)
        .map(|i| {
            let a = start + TAU * i as f32 / n as f32;
            [center[0] + r * a.cos(), center[1] + r * a.sin()]
        })
        .collect()
}

/// A star with `n` tips, alternating radius `outer` and `inner`, tip up.
fn star(center: P, outer: f32, inner: f32, n: usize) -> Vec<P> {
    (0..2 * n)
        .map(|i| {
            let r = if i % 2 == 0 { outer } else { inner };
            let a = -FRAC_PI_2 + PI * i as f32 / n as f32;
            [center[0] + r * a.cos(), center[1] + r * a.sin()]
        })
        .collect()
}

fn arc(center: P, r: f32, from: f32, to: f32, n: usize) -> Vec<P> {
    (0..=n)
        .map(|i| {
            let a = from + (to - from) * i as f32 / n as f32;
            [center[0] + r * a.cos(), center[1] + r * a.sin()]
        })
        .collect()
}

/// The rectangle of half-width `half` around the segment `a`–`b`.
fn bar(a: P, b: P, half: f32) -> [P; 4] {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    let (nx, ny) = (-dy / len * half, dx / len * half);
    [
        [a[0] + nx, a[1] + ny],
        [b[0] + nx, b[1] + ny],
        [b[0] - nx, b[1] - ny],
        [a[0] - nx, a[1] - ny],
    ]
}

/// A teardrop: the tip, then the round end of radius `r` around `center`,
/// between the two points where lines from the tip touch it.
fn teardrop(center: P, r: f32, tip: P, n: usize) -> Vec<P> {
    let (dx, dy) = (tip[0] - center[0], tip[1] - center[1]);
    let d = (dx * dx + dy * dy).sqrt();
    let toward = dy.atan2(dx);
    let spread = (r / d).acos();
    let mut points = vec![tip];
    points.extend(arc(center, r, toward - spread, toward + spread - TAU, n));
    points
}

/// A quadratic curve from `a` to `b` pulled towards `control`, `n` segments.
fn quad(a: P, control: P, b: P, n: usize) -> Vec<P> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            [
                u * u * a[0] + 2.0 * u * t * control[0] + t * t * b[0],
                u * u * a[1] + 2.0 * u * t * control[1] + t * t * b[1],
            ]
        })
        .collect()
}

fn crystal(pen: &Pen) {
    let s = pen.pal.crystal;
    let outline = [
        [0.50, 0.03],
        [0.78, 0.30],
        [0.68, 0.82],
        [0.50, 0.97],
        [0.32, 0.82],
        [0.22, 0.30],
    ];
    pen.fill(&outline, s.base);
    // The left half catches the light.
    pen.fill(
        &[[0.50, 0.03], [0.50, 0.97], [0.32, 0.82], [0.22, 0.30]],
        s.light,
    );
    pen.detail(&[[0.22, 0.30], [0.50, 0.40], [0.78, 0.30]], s.dark);
    pen.detail(&[[0.50, 0.40], [0.50, 0.97]], s.dark);
    pen.contour(&outline, s.dark);
}

/// A cut gem: a crown above the girdle, a pavilion below.
fn gem(pen: &Pen, s: Swatch) {
    let outline = [
        [0.28, 0.14],
        [0.72, 0.14],
        [0.95, 0.40],
        [0.50, 0.92],
        [0.05, 0.40],
    ];
    pen.fill(
        &[[0.28, 0.14], [0.72, 0.14], [0.95, 0.40], [0.05, 0.40]],
        s.light,
    );
    pen.fill(&[[0.05, 0.40], [0.50, 0.40], [0.50, 0.92]], s.base);
    pen.fill(&[[0.50, 0.40], [0.95, 0.40], [0.50, 0.92]], s.shade);
    pen.detail(&[[0.05, 0.40], [0.95, 0.40]], s.dark);
    pen.facet(&[[0.28, 0.14], [0.38, 0.40], [0.50, 0.92]], s.dark);
    pen.facet(&[[0.72, 0.14], [0.62, 0.40], [0.50, 0.92]], s.dark);
    pen.contour(&outline, s.dark);
}

/// The clear gem, smaller, with a sparkle in the free corner.
fn bonus_gem(pen: &Pen) {
    gem(&pen.sub([0.0, 0.16], 0.84), pen.pal.gem_clear);
    let s = pen.pal.sparkle;
    let c = [0.82, 0.18];
    let points = star(c, 0.18, 0.06, 4);
    pen.fill_fan(c, &points, s.base);
    pen.contour(&points, s.dark);
}

/// An ankh with a teardrop jewel in its loop, as relics look in the game:
/// a large loop, short arrow-tipped arms, a long flared stem. Upper faces
/// are light, lower faces dark. Tiers differ by metal only, as in the game;
/// tooltips name them.
fn relic_icon(pen: &Pen, relic: Relic) {
    let s = pen.pal.relic(relic);
    let jewel = pen.pal.relic_jewel;
    let ring = teardrop([0.50, 0.215], 0.188, [0.50, 0.52], 24);
    // Where the loop's sides meet the arms.
    let join = |side: f32| {
        let (a, b) = (
            if side < 0.0 {
                ring[ring.len() - 1]
            } else {
                ring[1]
            },
            ring[0],
        );
        let t = (0.40 - a[1]) / (b[1] - a[1]);
        [a[0] + (b[0] - a[0]) * t, 0.40]
    };
    let mirror = |points: &[P]| -> Vec<P> { points.iter().map(|&[x, y]| [1.0 - x, y]).collect() };
    // The left arm, from the loop to the arrow tip and back to the stem.
    let mut arm = quad(join(-1.0), [0.33, 0.415], [0.246, 0.38], 6);
    arm.extend([[0.19, 0.47], [0.25, 0.566]]);
    arm.extend(
        quad([0.25, 0.566], [0.35, 0.535], [0.442, 0.525], 6)
            .into_iter()
            .skip(1),
    );
    let mut lower = vec![[0.19, 0.47], [0.44, 0.47]];
    lower.extend(quad([0.442, 0.525], [0.35, 0.535], [0.25, 0.566], 6));
    let stem = [
        [0.442, 0.52],
        [0.558, 0.52],
        [0.605, 0.904],
        [0.50, 0.97],
        [0.395, 0.904],
    ];
    let block = [[0.40, 0.38], [0.60, 0.38], [0.60, 0.53], [0.40, 0.53]];

    pen.fill_fan([0.50, 0.30], &ring, s.base);
    pen.fill(&block, s.base);
    pen.fill_fan([0.33, 0.47], &arm, s.base);
    pen.fill_fan([0.67, 0.47], &mirror(&arm), s.base);
    pen.fill(&stem, s.base);
    pen.fill_fan([0.32, 0.51], &lower, s.shade);
    pen.fill_fan([0.68, 0.51], &mirror(&lower), s.shade);
    if !pen.small() {
        let strip = [[0.442, 0.525], [0.462, 0.525], [0.425, 0.91], [0.40, 0.905]];
        pen.fill(&strip, s.shade);
        pen.fill(&mirror(&strip), s.shade);
    }
    let bevel = teardrop([0.50, 0.225], 0.11, [0.50, 0.40], 18);
    pen.fill_fan([0.50, 0.25], &bevel, s.shade);

    // The outline of the whole ankh.
    let mut outline: Vec<P> = ring[1..].iter().rev().copied().collect();
    outline.push(join(1.0));
    let right_arm = mirror(&arm);
    outline.extend(right_arm.iter().skip(1).take(right_arm.len() - 2));
    outline.extend([
        [0.558, 0.525],
        [0.605, 0.904],
        [0.50, 0.97],
        [0.395, 0.904],
        [0.442, 0.525],
    ]);
    outline.extend(arm.iter().rev().skip(1).take(arm.len() - 2));
    outline.push(join(-1.0));
    pen.contour(&outline, s.dark);
    if !pen.small() || pen.owned() {
        let mut closed = bevel.clone();
        closed.push(bevel[0]);
        pen.detail(&closed, s.dark);
    }

    let setting = teardrop([0.50, 0.225], 0.075, [0.50, 0.36], 16);
    let stone = teardrop([0.50, 0.225], 0.058, [0.50, 0.335], 16);
    pen.fill_fan([0.50, 0.24], &setting, jewel.dark);
    pen.fill_fan([0.50, 0.24], &stone, jewel.base);
    if pen.owned() && !pen.small() {
        pen.line(
            &arc([0.50, 0.225], 0.04, PI * 1.15, PI * 1.6, 5),
            theme::WHITE,
            0.025,
        );
    }
    if !pen.owned() {
        let mut closed = stone.clone();
        closed.push(stone[0]);
        pen.detail(&closed, jewel.dark);
    }
}

/// Each boss as an object from its fight in its own colours, never the
/// character: Tiny Tiger's trident, Dingodile's flamethrower, N. Tropy's
/// clock with tuning-fork prongs, N. Gin's missile, Cortex's "N".
fn boss(pen: &Pen, boss: Boss) {
    let p = pen.pal;
    match boss {
        Boss::TinyTiger => {
            let steel = p.steel;
            let prong = |x: f32| -> [P; 4] {
                [
                    [x - 0.04, 0.14],
                    [x + 0.04, 0.14],
                    [x + 0.04, 0.36],
                    [x - 0.04, 0.36],
                ]
            };
            let barb = |x: f32| -> [P; 3] { [[x - 0.065, 0.17], [x, 0.03], [x + 0.065, 0.17]] };
            let (l, c, r) = (prong(0.24), prong(0.50), prong(0.76));
            let (bl, bc, br) = (barb(0.24), barb(0.50), barb(0.76));
            let base = [[0.20, 0.32], [0.80, 0.32], [0.70, 0.46], [0.30, 0.46]];
            let shaft = [[0.46, 0.44], [0.54, 0.44], [0.54, 0.97], [0.46, 0.97]];
            pen.union(
                &[&l, &c, &r, &bl, &bc, &br, &base, &shaft],
                steel.base,
                steel.dark,
            );
            let grip = [[0.445, 0.62], [0.555, 0.62], [0.555, 0.90], [0.445, 0.90]];
            pen.fill(&grip, p.tiger.base);
            pen.contour(&grip, p.tiger.dark);
            if !pen.small() {
                for y in [0.69, 0.76, 0.83] {
                    pen.line(&[[0.445, y], [0.555, y - 0.03]], p.tiger.dark, 0.035);
                }
            }
        }
        Boss::Dingodile => {
            // A flamethrower seen from the side: grip, barrel and muzzle in
            // steel, a fuel tank under the barrel, and a jet of fire that
            // widens away from the nozzle.
            let grip = [[0.10, 0.50], [0.21, 0.50], [0.17, 0.76], [0.06, 0.76]];
            let barrel = [[0.06, 0.38], [0.64, 0.38], [0.64, 0.50], [0.06, 0.50]];
            let muzzle = [[0.62, 0.33], [0.70, 0.33], [0.70, 0.55], [0.62, 0.55]];
            pen.union(&[&grip, &barrel, &muzzle], p.steel.base, p.steel.dark);
            let mut tank = arc([0.33, 0.58], 0.085, FRAC_PI_2, PI * 1.5, 8);
            tank.extend(arc([0.49, 0.58], 0.085, -FRAC_PI_2, FRAC_PI_2, 8));
            pen.union(&[&tank], p.croc.base, p.croc.dark);
            if !pen.small() {
                pen.detail(&[[0.25, 0.44], [0.58, 0.44]], p.steel.dark);
            }
            // The fire: a stream from the muzzle into a billowing ball with
            // tips licking upwards.
            let stream = [[0.70, 0.40], [0.80, 0.36], [0.80, 0.54], [0.70, 0.48]];
            let ball = polygon([0.84, 0.48], 0.13, 14, 0.0);
            let puff = polygon([0.92, 0.40], 0.075, 12, 0.0);
            let tip_left = teardrop([0.78, 0.40], 0.07, [0.74, 0.16], 10);
            let tip_mid = teardrop([0.86, 0.36], 0.08, [0.88, 0.06], 10);
            let tip_right = teardrop([0.94, 0.38], 0.05, [0.99, 0.20], 8);
            pen.union(
                &[&stream, &tip_left, &tip_mid, &tip_right, &puff, &ball],
                p.flame.base,
                p.flame.dark,
            );
            pen.fill(&polygon([0.84, 0.49], 0.065, 12, 0.0), p.flame_core.base);
            pen.fill(
                &teardrop([0.85, 0.44], 0.045, [0.86, 0.28], 8),
                p.flame_core.base,
            );
        }
        Boss::NTropy => {
            // A clock with a tuning fork rising behind it.
            let mut fork = vec![[0.39, 0.04], [0.39, 0.30]];
            fork.extend(arc([0.50, 0.30], 0.11, PI, 0.0, 10));
            fork.extend([[0.61, 0.30], [0.61, 0.04]]);
            pen.band(&fork, 0.07, p.steel.base, p.steel.dark);
            let (c, r) = ([0.50, 0.68], 0.30);
            pen.disc(c, r, p.bronze.base);
            pen.disc(c, r * 0.74, p.dial.base);
            if !pen.small() {
                for i in 0..12 {
                    let a = TAU * i as f32 / 12.0;
                    let (dx, dy) = (a.cos(), a.sin());
                    let inner = if i % 3 == 0 { 0.56 } else { 0.63 };
                    pen.detail(
                        &[
                            [c[0] + dx * r * inner, c[1] + dy * r * inner],
                            [c[0] + dx * r * 0.72, c[1] + dy * r * 0.72],
                        ],
                        p.dial.dark,
                    );
                }
            }
            pen.line(&[c, [0.50, 0.46]], p.dial.dark, 0.06);
            pen.line(&[c, [0.63, 0.57]], p.dial.dark, 0.06);
            pen.dot(c, 0.035, p.dial.dark);
            pen.ring(c, r * 0.74, p.bronze.dark);
            pen.ring(c, r, p.bronze.dark);
        }
        Boss::NGin => {
            let m = p.missile;
            let body = bar([0.26, 0.74], [0.62, 0.38], 0.10);
            let nose = [[0.55, 0.31], [0.88, 0.12], [0.69, 0.45]];
            let fin_l = [[0.19, 0.67], [0.30, 0.62], [0.12, 0.54]];
            let fin_r = [[0.33, 0.81], [0.38, 0.70], [0.46, 0.88]];
            let tail = bar([0.14, 0.86], [0.26, 0.74], 0.065);
            pen.union(&[&fin_l, &fin_r, &tail, &body, &nose], m.base, m.dark);
            let tip = [[0.74, 0.20], [0.88, 0.12], [0.80, 0.26]];
            let band = bar([0.47, 0.53], [0.53, 0.47], 0.10);
            pen.fill(&tip, p.boss_accent.base);
            pen.fill(&band, p.boss_accent.base);
            pen.contour(&band, p.boss_accent.dark);
        }
        Boss::Cortex => {
            let s = p.cortex;
            pen.disc([0.50, 0.50], 0.44, s.base);
            pen.ring([0.50, 0.50], 0.44, s.dark);
            let ink = p.glyph_dark;
            pen.fill(
                &[[0.28, 0.27], [0.39, 0.27], [0.39, 0.73], [0.28, 0.73]],
                ink,
            );
            pen.fill(
                &[[0.61, 0.27], [0.72, 0.27], [0.72, 0.73], [0.61, 0.73]],
                ink,
            );
            pen.fill(
                &[[0.28, 0.27], [0.40, 0.27], [0.72, 0.73], [0.60, 0.73]],
                ink,
            );
            let outline = [
                [0.28, 0.27],
                [0.40, 0.27],
                [0.61, 0.572],
                [0.61, 0.27],
                [0.72, 0.27],
                [0.72, 0.73],
                [0.60, 0.73],
                [0.39, 0.428],
                [0.39, 0.73],
                [0.28, 0.73],
            ];
            pen.emblem_outline(&outline, ink);
        }
    }
}

/// An arrow coming down onto the ground, with impact lines.
fn body_slam(pen: &Pen) {
    let s = pen.pal.ability;
    let shaft = [[0.41, 0.05], [0.59, 0.05], [0.59, 0.42], [0.41, 0.42]];
    let head = [[0.20, 0.42], [0.80, 0.42], [0.50, 0.74]];
    let outline = [
        [0.41, 0.05],
        [0.59, 0.05],
        [0.59, 0.42],
        [0.80, 0.42],
        [0.50, 0.74],
        [0.20, 0.42],
        [0.41, 0.42],
    ];
    pen.fill(&shaft, s.base);
    pen.fill(&head, s.base);
    pen.contour(&outline, s.dark);
    pen.line(&[[0.06, 0.88], [0.94, 0.88]], s.dark, 0.07);
    pen.line(&[[0.30, 0.80], [0.14, 0.66]], s.base, 0.06);
    pen.line(&[[0.70, 0.80], [0.86, 0.66]], s.base, 0.06);
}

/// Two stacked chevrons pointing up.
fn double_jump(pen: &Pen) {
    let s = pen.pal.ability;
    for y in [0.06, 0.48] {
        let h = 0.42;
        let right = [
            [0.50, y],
            [0.90, y + h * 0.78],
            [0.76, y + h],
            [0.50, y + h * 0.50],
        ];
        let left = [
            [0.50, y],
            [0.50, y + h * 0.50],
            [0.24, y + h],
            [0.10, y + h * 0.78],
        ];
        let outline = [
            [0.50, y],
            [0.90, y + h * 0.78],
            [0.76, y + h],
            [0.50, y + h * 0.50],
            [0.24, y + h],
            [0.10, y + h * 0.78],
        ];
        pen.fill(&right, s.base);
        pen.fill(&left, s.base);
        pen.contour(&outline, s.dark);
    }
}

/// A spiral, two turns out from the centre.
fn tornado_spin(pen: &Pen) {
    let s = pen.pal.ability;
    let turns = 2.0;
    let n = 64;
    let points: Vec<P> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let a = -FRAC_PI_2 + t * turns * TAU;
            let r = 0.05 + 0.36 * t;
            [0.50 + r * a.cos(), 0.50 + r * a.sin()]
        })
        .collect();
    pen.band(&points, 0.09, s.base, s.dark);
}

/// A tube held at an angle, firing a round fruit.
fn bazooka(pen: &Pen) {
    let s = pen.pal.ability;
    let grip = bar([0.26, 0.70], [0.30, 0.90], 0.05);
    pen.fill(&grip, s.shade);
    pen.contour(&grip, s.dark);
    let tube = bar([0.06, 0.66], [0.58, 0.46], 0.14);
    pen.fill(&tube, s.base);
    pen.contour(&tube, s.dark);
    let muzzle = bar([0.54, 0.475], [0.66, 0.43], 0.18);
    pen.fill(&muzzle, s.shade);
    pen.contour(&muzzle, s.dark);
    let fruit = pen.pal.fruit;
    let c = [0.84, 0.25];
    pen.disc(c, 0.14, fruit.base);
    pen.ring(c, 0.14, fruit.dark);
    pen.line(&[[0.84, 0.11], [0.93, 0.04]], fruit.dark, 0.06);
}

/// A shoe with speed lines.
fn speed_shoes(pen: &Pen) {
    let s = pen.pal.ability;
    let ankle = [[0.36, 0.28], [0.60, 0.28], [0.62, 0.50], [0.36, 0.50]];
    let foot = [
        [0.36, 0.50],
        [0.62, 0.50],
        [0.86, 0.60],
        [0.94, 0.74],
        [0.94, 0.82],
        [0.36, 0.82],
    ];
    let outline = [
        [0.36, 0.28],
        [0.60, 0.28],
        [0.62, 0.50],
        [0.86, 0.60],
        [0.94, 0.74],
        [0.94, 0.82],
        [0.36, 0.82],
    ];
    pen.fill(&ankle, s.base);
    pen.fill(&foot, s.base);
    pen.fill(
        &[[0.36, 0.72], [0.94, 0.72], [0.94, 0.82], [0.36, 0.82]],
        s.dark,
    );
    pen.contour(&outline, s.dark);
    for (y, x0) in [(0.40, 0.08), (0.56, 0.02), (0.72, 0.10)] {
        pen.line(&[[x0, y], [0.28, y]], s.base, 0.06);
    }
}

fn padlock(pen: &Pen, open: bool) {
    let s = pen.pal.neutral;
    let shackle = if open {
        let mut p = vec![[0.30, 0.48]];
        p.extend(arc([0.50, 0.22], 0.20, PI, TAU, 12));
        p.push([0.70, 0.30]);
        p
    } else {
        let mut p = vec![[0.30, 0.48]];
        p.extend(arc([0.50, 0.32], 0.20, PI, TAU, 12));
        p.push([0.70, 0.48]);
        p
    };
    pen.band(&shackle, 0.08, s.shade, s.dark);
    let body = Rect::from_min_max(pen.p([0.16, 0.46]), pen.p([0.84, 0.94]));
    let radius = CornerRadius::same((0.08 * pen.rect.width()).round() as u8);
    let stroke = Stroke::new(pen.contour_width(), pen.contour_color(s.dark));
    pen.painter.rect(
        body,
        radius,
        pen.fill_color(s.base),
        stroke,
        StrokeKind::Middle,
    );
    pen.dot([0.50, 0.65], 0.065, s.dark);
    pen.line(&[[0.50, 0.66], [0.50, 0.80]], s.dark, 0.07);
}

fn warning(pen: &Pen) {
    let s = pen.pal.warning;
    let outline = [[0.50, 0.07], [0.95, 0.88], [0.05, 0.88]];
    pen.fill(&outline, s.base);
    pen.contour(&outline, s.dark);
    pen.line(&[[0.50, 0.36], [0.50, 0.62]], pen.pal.glyph_dark, 0.10);
    pen.dot([0.50, 0.75], 0.055, pen.pal.glyph_dark);
}

fn info(pen: &Pen) {
    let s = pen.pal.info;
    pen.disc([0.50, 0.50], 0.44, s.base);
    pen.ring([0.50, 0.50], 0.44, s.dark);
    pen.dot([0.50, 0.29], 0.065, pen.pal.glyph_light);
    pen.line(&[[0.50, 0.44], [0.50, 0.75]], pen.pal.glyph_light, 0.11);
}

fn error(pen: &Pen) {
    let s = pen.pal.error;
    let outline = polygon([0.50, 0.50], 0.46, 8, PI / 8.0);
    pen.fill(&outline, s.base);
    pen.contour(&outline, s.dark);
    let glyph = pen.pal.glyph_light;
    pen.line(&[[0.34, 0.34], [0.66, 0.66]], glyph, 0.11);
    pen.line(&[[0.66, 0.34], [0.34, 0.66]], glyph, 0.11);
}

/// A film clapper.
fn cutscene(pen: &Pen) {
    let s = pen.pal.neutral;
    let body = [[0.10, 0.44], [0.90, 0.44], [0.90, 0.90], [0.10, 0.90]];
    pen.fill(&body, s.base);
    pen.fill(
        &[[0.10, 0.44], [0.90, 0.44], [0.90, 0.58], [0.10, 0.58]],
        s.light,
    );
    let arm = bar([0.10, 0.36], [0.88, 0.14], 0.065);
    pen.fill(&arm, s.light);
    if !pen.small() {
        for x in [0.24, 0.44, 0.64] {
            pen.detail(&[[x, 0.58], [x + 0.10, 0.44]], s.dark);
        }
        for t in [0.30, 0.55, 0.80] {
            let (x, y) = (0.10 + 0.78 * t, 0.36 - 0.22 * t);
            pen.detail(&[[x - 0.04, y + 0.06], [x + 0.06, y - 0.06]], s.dark);
        }
    }
    pen.detail(&[[0.10, 0.58], [0.90, 0.58]], s.dark);
    pen.contour(&arm, s.dark);
    pen.contour(&body, s.dark);
}

/// A heart followed by "×".
fn lives(pen: &Pen) {
    let s = pen.pal.lives;
    // Two round lobes and a tip blunt enough that the contour's joins stay
    // short.
    let (left, right, r, tip) = ([0.24, 0.36], [0.50, 0.36], 0.17, [0.37, 0.80]);
    let dy = (r * r - 0.13f32 * 0.13).sqrt();
    let notch = (-dy).atan2(0.13);
    let mut points = arc(left, r, PI * 0.75, TAU + notch, 16);
    points.extend(
        arc(right, r, PI - notch, TAU + PI * 0.25, 16)
            .into_iter()
            .skip(1),
    );
    points.push(tip);
    let (cx, cy) = (0.37, 0.41);
    pen.fill_fan([cx, cy + 0.04], &points, s.base);
    if pen.owned() && !pen.small() {
        let glint = arc([0.25, 0.30], 0.08, PI * 1.05, PI * 1.55, 6);
        pen.line(&glint, s.light, 0.045);
    }
    pen.contour(&points, s.dark);
    pen.line(&[[0.72, 0.62], [0.92, 0.82]], pen.pal.text, 0.075);
    pen.line(&[[0.92, 0.62], [0.72, 0.82]], pen.pal.text, 0.075);
}

/// A stopwatch.
fn timer(pen: &Pen) {
    let s = pen.pal.neutral;
    let crown = [[0.40, 0.04], [0.60, 0.04], [0.60, 0.13], [0.40, 0.13]];
    let stem = [[0.46, 0.13], [0.54, 0.13], [0.54, 0.24], [0.46, 0.24]];
    let side = bar([0.76, 0.30], [0.84, 0.22], 0.04);
    for part in [&crown, &stem, &side] {
        pen.fill(part, s.base);
        pen.contour(part, s.dark);
    }
    let (c, r) = ([0.50, 0.59], 0.37);
    pen.disc(c, r, s.base);
    pen.disc(c, r * 0.76, s.light);
    if !pen.small() {
        for i in 0..4 {
            let a = FRAC_PI_2 * i as f32;
            let (dx, dy) = (a.cos(), a.sin());
            pen.detail(
                &[
                    [c[0] + dx * r * 0.60, c[1] + dy * r * 0.60],
                    [c[0] + dx * r * 0.74, c[1] + dy * r * 0.74],
                ],
                s.dark,
            );
        }
    }
    pen.line(&[c, [0.50, 0.38]], s.dark, 0.06);
    pen.line(&[c, [0.63, 0.66]], s.dark, 0.06);
    pen.dot(c, 0.04, s.dark);
    pen.ring(c, r, s.dark);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Event, PointerButton, RawInput, pos2};

    const SIZES: [f32; 4] = [
        theme::ICON_SMALL,
        theme::ICON_MEDIUM,
        theme::ICON_LARGE,
        theme::ICON_XLARGE,
    ];

    const STATES: [IconState; 6] = [
        IconState::OWNED,
        IconState::MISSING,
        IconState::NOT_APPLICABLE,
        IconState::OWNED.hovered(true),
        IconState::MISSING.hovered(true),
        IconState::NOT_APPLICABLE.hovered(true),
    ];

    fn input(ppp: f32) -> RawInput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            ..Default::default()
        };
        input
            .viewports
            .entry(input.viewport_id)
            .or_default()
            .native_pixels_per_point = Some(ppp);
        input
    }

    #[test]
    fn every_icon_paints_in_every_state_size_and_theme() {
        let icons = Icon::all();
        assert_eq!(icons.len(), 29);
        for ppp in [1.0, 1.25, 2.0] {
            let ctx = Context::default();
            let output = ctx.run_ui(input(ppp), |ui| {
                for dark in [false, true] {
                    for &icon in &icons {
                        for state in STATES {
                            for size in SIZES {
                                let painter = ui.painter();
                                let before = count(painter);
                                let rect = Rect::from_min_size(pos2(10.0, 10.0), Vec2::splat(size));
                                paint(painter, rect, icon, state, dark);
                                assert!(
                                    count(painter) > before,
                                    "{icon:?} {state:?} {size} painted nothing"
                                );
                            }
                        }
                    }
                }
                // Degenerate rectangles paint nothing and don't panic.
                paint(
                    ui.painter(),
                    Rect::NOTHING,
                    Icon::Crystal,
                    IconState::OWNED,
                    false,
                );
                paint(
                    ui.painter(),
                    Rect::ZERO,
                    Icon::Lives,
                    IconState::OWNED,
                    true,
                );
            });
            let mut output = output;
            let shapes = std::mem::take(&mut output.shapes);
            let primitives = ctx.tessellate(shapes, output.pixels_per_point);
            assert!(!primitives.is_empty());
            output.drop_without_applying_deltas();
        }
    }

    fn count(painter: &Painter) -> usize {
        let mut n = 0;
        painter.for_each_shape(|_| n += 1);
        n
    }

    #[test]
    fn bosses_follow_the_warp_rooms() {
        for (boss, level) in Boss::ALL.into_iter().zip(BOSSES) {
            assert_eq!(boss.level(), level);
        }
        assert_eq!(Icon::Boss(Boss::NTropy).name(), "Dr. N. Tropy");
    }

    #[test]
    fn names_are_unique() {
        let icons = Icon::all();
        let mut names: Vec<_> = icons.iter().map(|i| i.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), icons.len());
    }

    /// Runs frames with a pointer press and release over the toggle at
    /// (20, 20) and returns whether it reported a click.
    fn click_toggle(enabled: bool) -> bool {
        let ctx = Context::default();
        let mut clicked = false;
        let center = pos2(20.0, 20.0);
        let frames = [
            vec![],
            vec![Event::PointerMoved(center)],
            vec![Event::PointerButton {
                pos: center,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            }],
            vec![Event::PointerButton {
                pos: center,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            }],
        ];
        for events in frames {
            let mut raw = input(1.0);
            raw.events = events;
            let output = ctx.run_ui(raw, |ui| {
                let mut child = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(Rect::from_min_size(pos2(4.0, 4.0), vec2(200.0, 200.0))),
                );
                let response = icon_toggle(&mut child, Icon::Crystal, false, enabled, 32.0);
                clicked |= response.clicked();
            });
            output.drop_without_applying_deltas();
        }
        clicked
    }

    #[test]
    fn toggle_clicks_only_when_enabled() {
        assert!(click_toggle(true));
        assert!(!click_toggle(false));
    }

    #[test]
    fn colours_come_from_the_theme() {
        let source = include_str!("icons.rs");
        let code = source.split("#[cfg(test)]").next().unwrap();
        for constructor in [
            "Color32::from",
            "Color32::WHITE",
            "Color32::BLACK",
            "Color32::RED",
        ] {
            assert!(!code.contains(constructor), "icons.rs uses {constructor}");
        }
    }

    #[test]
    fn each_boss_level_has_its_symbol() {
        for boss in Boss::ALL {
            assert_eq!(Boss::of(boss.level()), Some(boss));
        }
        let toad_village = LevelId::from_key("toadvillage").unwrap();
        assert_eq!(Boss::of(toad_village), None);
    }
}
