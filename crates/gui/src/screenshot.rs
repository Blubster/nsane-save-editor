//! `--screenshot <file.png>`: let the window settle for a few frames, capture
//! it, write a PNG and quit. Several screenshots are taken in turn in the same
//! window, switching theme between them.

use std::collections::VecDeque;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use egui::{ColorImage, Context, Event, UserData, ViewportCommand};

use crate::options::Shot;

/// Frames rendered before the capture, so fonts and layout have settled.
const WARMUP_FRAMES: u32 = 5;
/// The capture also waits this long after the first frame (or the previous
/// capture). Under WSLg, the compositor has crashed when windows open and
/// close in quick succession, so one window takes every capture.
const WARMUP_SECONDS: f64 = 1.5;

pub struct Screenshot {
    shots: VecDeque<Shot>,
    frames: u32,
    started: Option<f64>,
    requested: bool,
}

impl Screenshot {
    /// `None` when there is nothing to capture.
    pub fn new(shots: Vec<Shot>) -> Option<Self> {
        (!shots.is_empty()).then(|| Self {
            shots: shots.into(),
            frames: 0,
            started: None,
            requested: false,
        })
    }

    /// Call once per frame, after drawing. Returns the theme to switch to
    /// when the next capture asks for one.
    pub fn update(&mut self, ctx: &Context) -> Option<bool> {
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image
            && let Some(shot) = self.shots.pop_front()
        {
            match write_png(&shot.path, &image) {
                Ok(()) => log::info!("screenshot written to {}", shot.path.display()),
                Err(e) => {
                    eprintln!("can't write {}: {e}", shot.path.display());
                    std::process::exit(1);
                }
            }
            (self.frames, self.started, self.requested) = (0, None, false);
            match self.shots.front() {
                Some(next) => {
                    ctx.request_repaint();
                    return next.dark;
                }
                None => {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    return None;
                }
            }
        }
        if self.shots.is_empty() {
            return None;
        }
        self.frames += 1;
        let now = ctx.input(|i| i.time);
        let started = *self.started.get_or_insert(now);
        if self.frames >= WARMUP_FRAMES && now - started >= WARMUP_SECONDS && !self.requested {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(UserData::default()));
            self.requested = true;
        }
        ctx.request_repaint();
        None
    }
}

fn write_png(path: &Path, image: &ColorImage) -> Result<(), Box<dyn std::error::Error>> {
    let [w, h] = image.size;
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = png::Encoder::new(file, w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let rgba: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|c| c.to_srgba_unmultiplied())
        .collect();
    writer.write_image_data(&rgba)?;
    Ok(())
}
