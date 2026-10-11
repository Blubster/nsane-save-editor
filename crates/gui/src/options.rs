//! Command-line options. The web build always uses the defaults.

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[derive(Debug, Default, PartialEq)]
pub struct Options {
    /// Show the icon gallery instead of the editor.
    pub icon_gallery: bool,
    /// `Some(true)` forces the dark theme, `Some(false)` the light one;
    /// `None` follows the system.
    pub dark: Option<bool>,
    /// Inner window size in points.
    #[cfg(not(target_arch = "wasm32"))]
    pub size: Option<[f32; 2]>,
    /// Start maximized.
    #[cfg(not(target_arch = "wasm32"))]
    pub maximized: bool,
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

  --icon-gallery         show every icon in every state and size
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
                "--icon-gallery" => options.icon_gallery = true,
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
    }
}
