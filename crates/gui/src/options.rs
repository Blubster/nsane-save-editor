//! Command-line options. The web build always uses the defaults.

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use nsane::crash3::LevelId;

/// A state of the window that `--view` sets up, for screenshots.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // set from the command line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// The question asked before an unsaved document is closed.
    Unsaved,
    /// A refused time in the selected level's third leaderboard field.
    InvalidTime,
    /// The dialog that unlocks the first locked warp room, as its padlock
    /// opens it.
    UnlockDialog,
    /// The story scenes dialog.
    Cutscenes,
    /// The dialog of the "Complete everything" preset.
    PresetAll,
    /// The dialog of the "Story up to warp room" preset, at room 3.
    PresetStory3,
    /// The diagnostics panel.
    Diagnostics,
    /// The review shown before saving.
    SaveReview,
    /// The dialog that converts the save to the other platform.
    Convert,
    /// The About dialog.
    About,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // set from the command line
impl View {
    pub const ALL: [View; 10] = [
        View::Unsaved,
        View::InvalidTime,
        View::UnlockDialog,
        View::Cutscenes,
        View::PresetAll,
        View::PresetStory3,
        View::Diagnostics,
        View::SaveReview,
        View::Convert,
        View::About,
    ];

    pub fn name(self) -> &'static str {
        match self {
            View::Unsaved => "unsaved",
            View::InvalidTime => "invalid-time",
            View::UnlockDialog => "unlock-dialog",
            View::Cutscenes => "cutscenes",
            View::PresetAll => "preset-108",
            View::PresetStory3 => "preset-story3",
            View::Diagnostics => "diagnostics",
            View::SaveReview => "save-review",
            View::Convert => "convert",
            View::About => "about",
        }
    }

    pub fn from_name(name: &str) -> Option<View> {
        View::ALL.into_iter().find(|v| v.name() == name)
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    /// Show the icon gallery instead of the editor.
    pub icon_gallery: bool,
    /// Write the app icon files into this folder, then quit.
    #[cfg(not(target_arch = "wasm32"))]
    pub export_icon: Option<PathBuf>,
    /// `Some(true)` forces the dark theme, `Some(false)` the light one;
    /// `None` follows the system.
    pub dark: Option<bool>,
    /// Inner window size in points.
    #[cfg(not(target_arch = "wasm32"))]
    pub size: Option<[f32; 2]>,
    /// Start maximized.
    #[cfg(not(target_arch = "wasm32"))]
    pub maximized: bool,
    /// A save to open at start.
    #[cfg(not(target_arch = "wasm32"))]
    pub open: Option<PathBuf>,
    /// Edits to apply to it at start: a JSON list of edit ops.
    #[cfg(not(target_arch = "wasm32"))]
    pub apply: Option<PathBuf>,
    /// The template of `--view convert`.
    #[cfg(not(target_arch = "wasm32"))]
    pub template: Option<PathBuf>,
    /// The level selected at start.
    pub select: Option<LevelId>,
    pub view: Option<View>,
    /// Capture the window to these PNG files in turn, then quit.
    #[cfg(not(target_arch = "wasm32"))]
    pub screenshots: Vec<Shot>,
}

/// One `--screenshot`: the file, and the theme in force when it was given.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    pub dark: Option<bool>,
    pub path: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
pub const USAGE: &str = "\
usage: nsane-save-editor [options]

  --open FILE.sav        open a save
  --apply EDITS.json     apply edits to it at start, one undo step each: a
                         JSON list of edit ops, as in tests/golden
  --select LEVEL         select a level by its key, e.g. toadvillage
  --view NAME            set up a state at start: unsaved (the question
                         before closing), invalid-time (a refused time
                         typed for the selected level), unlock-dialog
                         (the dialog that unlocks the first locked warp
                         room), cutscenes (the story scenes dialog),
                         preset-108 and preset-story3 (preset dialogs),
                         diagnostics (the diagnostics panel),
                         save-review (the review shown before saving),
                         convert (the dialog that converts the save to
                         the other platform), about (the About dialog)
  --template FILE.sav    the template picked in --view convert
  --icon-gallery         show every icon in every state and size
  --export-icon DIR      write the app icon (PNG files and icon.ico) into
                         DIR, then quit
  --theme light|dark     force a theme instead of following the system
  --size WxH             inner window size in points, e.g. 1280x800
  --maximized            start with the window maximized
  --screenshot FILE.png  capture the window after a few frames, then quit;
                         repeat it to capture several files in one run, each
                         in the --theme given before it";

#[cfg(not(target_arch = "wasm32"))]
impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut options = Options::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));
            match arg.as_str() {
                "--open" => options.open = Some(value("--open")?.into()),
                "--apply" => options.apply = Some(value("--apply")?.into()),
                "--template" => options.template = Some(value("--template")?.into()),
                "--select" => {
                    let key = value("--select")?;
                    options.select =
                        Some(LevelId::from_key(&key).ok_or(format!("unknown level {key:?}"))?);
                }
                "--view" => {
                    let name = value("--view")?;
                    options.view = Some(View::from_name(&name).ok_or_else(|| {
                        let names: Vec<_> = View::ALL.iter().map(|v| v.name()).collect();
                        format!("unknown view {name:?} (one of: {})", names.join(", "))
                    })?);
                }
                "--icon-gallery" => options.icon_gallery = true,
                "--export-icon" => options.export_icon = Some(value("--export-icon")?.into()),
                "--theme" => {
                    options.dark = Some(match value("--theme")?.as_str() {
                        "light" => false,
                        "dark" => true,
                        other => return Err(format!("unknown theme {other:?}")),
                    })
                }
                "--size" => {
                    let v = value("--size")?;
                    options.size = Some(parse_size(&v).ok_or(format!("bad size {v:?}"))?);
                }
                "--maximized" => options.maximized = true,
                "--screenshot" => {
                    let path = value("--screenshot")?.into();
                    options.screenshots.push(Shot {
                        dark: options.dark,
                        path,
                    });
                }
                other => return Err(format!("unknown option {other:?}")),
            }
        }
        Ok(options)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn parse_size(s: &str) -> Option<[f32; 2]> {
    let (w, h) = s.split_once('x')?;
    let (w, h): (f32, f32) = (w.parse().ok()?, h.parse().ok()?);
    (w >= 1.0 && h >= 1.0 && w.is_finite() && h.is_finite()).then_some([w, h])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_the_flags() {
        assert_eq!(parse(&[]).unwrap(), Options::default());
        let o = parse(&[
            "--icon-gallery",
            "--theme",
            "light",
            "--size",
            "1200x900",
            "--maximized",
            "--screenshot",
            "out.png",
        ])
        .unwrap();
        assert!(o.icon_gallery);
        assert_eq!(o.dark, Some(false));
        assert_eq!(o.size, Some([1200.0, 900.0]));
        assert!(o.maximized);
        assert_eq!(
            o.screenshots,
            [Shot {
                dark: Some(false),
                path: "out.png".into()
            }]
        );
    }

    #[test]
    fn parses_the_document_flags() {
        let o = parse(&[
            "--open",
            "tests/saves/game/0019.sav",
            "--apply",
            "edits.json",
            "--template",
            "tests/saves/game/ALCHEMYXSAVE_8.sav",
            "--export-icon",
            "assets",
            "--select",
            "toadvillage",
            "--view",
            "unsaved",
            "--screenshot",
            "out.png",
        ])
        .unwrap();
        assert_eq!(o.open, Some("tests/saves/game/0019.sav".into()));
        assert_eq!(o.apply, Some("edits.json".into()));
        assert_eq!(
            o.template,
            Some("tests/saves/game/ALCHEMYXSAVE_8.sav".into())
        );
        assert_eq!(o.export_icon, Some("assets".into()));
        assert_eq!(o.select, LevelId::from_key("toadvillage"));
        assert_eq!(o.view, Some(View::Unsaved));
        assert_eq!(o.screenshots.len(), 1);
    }

    #[test]
    fn each_screenshot_takes_the_theme_before_it() {
        let o = parse(&[
            "--screenshot",
            "a.png",
            "--theme",
            "light",
            "--screenshot",
            "b.png",
            "--theme",
            "dark",
            "--screenshot",
            "c.png",
        ])
        .unwrap();
        let themes: Vec<_> = o.screenshots.iter().map(|s| s.dark).collect();
        assert_eq!(themes, [None, Some(false), Some(true)]);
        assert_eq!(o.dark, Some(true));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&["--theme"]).is_err());
        assert!(parse(&["--theme", "blue"]).is_err());
        assert!(parse(&["--size", "12"]).is_err());
        assert!(parse(&["--size", "0x10"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
        assert!(parse(&["--open"]).is_err());
        assert!(parse(&["--export-icon"]).is_err());
        assert!(parse(&["--select", "nowhere"]).is_err());
        assert_eq!(
            parse(&["--view", "invalid-time"]).unwrap().view,
            Some(View::InvalidTime)
        );
        assert!(parse(&["--view", "nothing"]).is_err());
        assert!(parse(&["--view", "progress"]).is_err());
    }
}
