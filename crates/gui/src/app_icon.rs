//! The app icon: the crystal icon, rendered without a window. `--export-icon`
//! writes the PNG files and the Windows `.ico` kept in `crates/gui/assets/`;
//! the Windows executable embeds the `.ico` (`build.rs`), the web build uses
//! it as its favicon, and the window shows the largest PNG.

use std::path::Path;
use std::sync::Arc;

use egui::epaint::{Primitive, Vertex};
use egui::{Color32, IconData, Pos2, RawInput, Rect, Vec2};

use crate::icons::{self, Icon, IconState};

/// The sizes rendered, in pixels: the usual sizes of a Windows icon.
pub const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

const LARGEST: &[u8] = include_bytes!("../assets/icon-256.png");

/// The icon of the window, from the 256-pixel PNG.
pub fn window_icon() -> Arc<IconData> {
    let (width, height, rgba) = decode_png(LARGEST).expect("the bundled icon is a valid PNG");
    Arc::new(IconData {
        rgba,
        width,
        height,
    })
}

/// Writes `icon-<size>.png` for every size and `icon.ico` into `dir`.
pub fn export(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let mut pngs = Vec::new();
    for size in SIZES {
        let png = encode_png(size, size, &render(size))?;
        std::fs::write(dir.join(format!("icon-{size}.png")), &png)?;
        pngs.push((size, png));
    }
    std::fs::write(dir.join("icon.ico"), ico(&pngs))?;
    Ok(())
}

/// The crystal in full colour on a transparent square of `size` pixels, as
/// RGBA bytes with unmultiplied alpha. Each size is drawn on its own, so the
/// small ones get the simpler drawing they get in the app.
pub fn render(size: u32) -> Vec<u8> {
    let side = size as f32;
    let square = Rect::from_min_size(Pos2::ZERO, Vec2::splat(side));
    let ctx = egui::Context::default();
    let input = RawInput {
        screen_rect: Some(square),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| {
        icons::paint(ui.painter(), square, Icon::Crystal, IconState::OWNED, false);
    });
    output.textures_delta.clear(); // the font atlas, unused
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);

    // Premultiplied colours in gamma space, blended as egui's renderers do.
    let mut pixels = vec![[0.0f32; 4]; (size * size) as usize];
    for primitive in primitives {
        let Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        for tri in mesh.indices.as_chunks::<3>().0 {
            let v = tri.map(|i| mesh.vertices[i as usize]);
            fill_triangle(&mut pixels, size, v);
        }
    }
    pixels
        .iter()
        .flat_map(|p| {
            let [r, g, b, a] = p.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            Color32::from_rgba_premultiplied(r, g, b, a).to_srgba_unmultiplied()
        })
        .collect()
}

/// Draws one triangle of vertex colours, sampled at pixel centres. The icons
/// use no texture (the font atlas's white texel at most), and the strokes
/// carry their own anti-aliasing (feathering), so this matches the screen.
fn fill_triangle(pixels: &mut [[f32; 4]], size: u32, v: [Vertex; 3]) {
    let [a, b, c] = v.map(|v| v.pos);
    let area = edge(a, b, c);
    if area == 0.0 || !area.is_finite() {
        return;
    }
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as u32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as u32;
    let max_x = (a.x.max(b.x).max(c.x).ceil() as u32).min(size);
    let max_y = (a.y.max(b.y).max(c.y).ceil() as u32).min(size);
    let colors = v.map(|v| v.color.to_array().map(|c| c as f32 / 255.0));
    for y in min_y..max_y {
        for x in min_x..max_x {
            let p = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
            // Weights of a, b and c, positive inside whatever the winding.
            let w = [edge(b, c, p), edge(c, a, p), edge(a, b, p)].map(|e| e / area);
            let edges = [(b, c), (c, a), (a, b)];
            if !w.iter().zip(edges).all(|(&w, e)| covers(w, e, area)) {
                continue;
            }
            let src: [f32; 4] = std::array::from_fn(|i| {
                w[0] * colors[0][i] + w[1] * colors[1][i] + w[2] * colors[2][i]
            });
            let dst = &mut pixels[(y * size + x) as usize];
            let keep = 1.0 - src[3];
            *dst = std::array::from_fn(|i| src[i] + dst[i] * keep);
        }
    }
}

fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// A pixel centre exactly on an edge shared by two triangles belongs to one
/// of them only, so that it isn't blended twice.
fn covers(weight: f32, (from, to): (Pos2, Pos2), area: f32) -> bool {
    if weight != 0.0 {
        return weight > 0.0;
    }
    let (dx, dy) = (
        (to.x - from.x) * area.signum(),
        (to.y - from.y) * area.signum(),
    );
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}

pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, png::EncodingError> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(rgba)?;
    Ok(out)
}

/// Width, height and RGBA bytes of an 8-bit RGBA PNG.
fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), Box<dyn std::error::Error>> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info()?;
    let mut rgba = vec![0; reader.output_buffer_size().ok_or("PNG too large")?];
    let info = reader.next_frame(&mut rgba)?;
    if (info.color_type, info.bit_depth) != (png::ColorType::Rgba, png::BitDepth::Eight) {
        return Err("not an 8-bit RGBA PNG".into());
    }
    rgba.truncate(info.buffer_size());
    Ok((info.width, info.height, rgba))
}

/// A Windows icon file holding PNG images (supported since Windows Vista).
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    const HEADER: usize = 6;
    const ENTRY: usize = 16;
    let mut out = Vec::new();
    out.extend(0u16.to_le_bytes()); // reserved
    out.extend(1u16.to_le_bytes()); // an icon, not a cursor
    out.extend((images.len() as u16).to_le_bytes());
    let mut offset = HEADER + ENTRY * images.len();
    for (size, png) in images {
        let side = if *size >= 256 { 0 } else { *size as u8 }; // 0 means 256
        out.extend([side, side, 0, 0]); // width, height, no palette, reserved
        out.extend(1u16.to_le_bytes()); // colour planes
        out.extend(32u16.to_le_bytes()); // bits per pixel
        out.extend((png.len() as u32).to_le_bytes());
        out.extend((offset as u32).to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        out.extend(png);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");

    fn premultiplied(rgba: &[u8]) -> Vec<[u8; 4]> {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .map(|&[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a).to_array())
            .collect()
    }

    /// The files in `assets/` are the current drawing: rerun
    /// `--export-icon crates/gui/assets` after changing the crystal.
    #[test]
    fn the_bundled_icons_match_the_drawing() {
        for size in SIZES {
            let path = format!("{ASSETS}/icon-{size}.png");
            let (w, h, rgba) = decode_png(&std::fs::read(&path).unwrap()).unwrap();
            assert_eq!((w, h), (size, size), "{path}");
            let (bundled, drawn) = (premultiplied(&rgba), premultiplied(&render(size)));
            // Rounding may differ between platforms, nothing more.
            let worst = bundled
                .iter()
                .zip(&drawn)
                .flat_map(|(a, b)| (0..4).map(move |i| a[i].abs_diff(b[i])))
                .max();
            assert!(worst <= Some(2), "{path}: differs by {worst:?}");
        }
    }

    #[test]
    fn the_ico_holds_every_size() {
        let ico = std::fs::read(format!("{ASSETS}/icon.ico")).unwrap();
        assert_eq!(&ico[..4], &[0, 0, 1, 0]);
        assert_eq!(u16::from_le_bytes([ico[4], ico[5]]) as usize, SIZES.len());
        for (i, size) in SIZES.into_iter().enumerate() {
            let entry = &ico[6 + 16 * i..][..16];
            let len = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let at = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            let (w, h, _) = decode_png(&ico[at..at + len]).unwrap();
            assert_eq!((w, h), (size, size));
            assert_eq!(entry[0], if size == 256 { 0 } else { size as u8 });
        }
    }

    #[test]
    fn the_crystal_stands_on_a_transparent_square() {
        let size = 64;
        let rgba = render(size);
        let alpha = |x: u32, y: u32| rgba[((y * size + x) * 4 + 3) as usize];
        assert_eq!(
            [alpha(0, 0), alpha(63, 0), alpha(0, 63), alpha(63, 63)],
            [0; 4]
        );
        assert_eq!(alpha(32, 32), 255);
        assert_eq!(window_icon().width, 256);
    }
}
