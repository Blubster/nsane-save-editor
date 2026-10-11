//! Renders of the window for screenshots, drawn with wgpu and no window: the
//! main view in both themes at two sizes, a selected level, and every
//! `--view` in both themes. Ignored by default:
//!
//! ```text
//! NSANE_RENDER_DIR=/tmp/renders cargo test -p gui renders -- --ignored
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use egui_kittest::Harness;

use super::tests::themed;
use super::*;
use crate::test_util::repo_file;

const WIDE: Vec2 = Vec2::new(1280.0, 800.0);
const MEDIUM: Vec2 = Vec2::new(1024.0, 720.0);

/// One render: the app as `options` sets it up, then `setup`.
struct Case {
    name: String,
    size: Vec2,
    options: Options,
    setup: fn(&mut App),
}

fn case(name: &str, size: Vec2, options: Options) -> Case {
    Case {
        name: name.to_owned(),
        size,
        options,
        setup: |_| {},
    }
}

fn opening(rel: &str) -> Options {
    Options {
        open: Some(repo_file(rel)),
        ..Options::default()
    }
}

/// The scripted session of the save review on 0019: lives, Deep Trouble's
/// crystal and Speed Shoes, then the review.
fn review_session(app: &mut App) {
    use nsane::crash3::{Ability, Edit};
    let deep = LevelId::from_key("deeptrouble").unwrap();
    for edit in [
        Edit::SetLives { count: 42 },
        Edit::SetCrystal {
            level: deep,
            on: true,
        },
        Edit::SetAbility {
            ability: Ability::SpeedShoes,
            on: true,
        },
    ] {
        let label = edit.describe(app.file.as_ref().unwrap().doc.current());
        app.apply(label, vec![edit]);
    }
    let path = app.file.as_ref().unwrap().path.clone().unwrap();
    app.dialog = Some(app.review_dialog(Target::file(path), None));
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for (size_name, size) in [("1280", WIDE), ("1024", MEDIUM)] {
        cases.push(case(
            &format!("main-{size_name}"),
            size,
            opening("tests/saves/game/0019.sav"),
        ));
    }
    let toad = LevelId::from_key("toadvillage").unwrap();
    cases.push(case(
        "level-toadvillage",
        WIDE,
        Options {
            select: Some(toad),
            ..opening("tests/saves/game/0019.sav")
        },
    ));
    cases.push(case("empty", WIDE, Options::default()));
    for view in View::ALL {
        let rel = match view {
            View::Diagnostics => "tests/saves/edited/t4_crystals.sav",
            _ => "tests/saves/game/0019.sav",
        };
        let mut options = Options {
            view: Some(view),
            ..opening(rel)
        };
        match view {
            View::InvalidTime => options.select = Some(toad),
            View::Convert => {
                options.template = Some(repo_file("tests/saves/game/ALCHEMYXSAVE_8.sav"))
            }
            View::SaveReview => options.view = None,
            _ => {}
        }
        let mut case = case(view.name(), WIDE, options);
        if view == View::SaveReview {
            case.setup = review_session;
        }
        cases.push(case);
    }
    cases
}

fn render(dir: &Path, case: &Case, dark: bool) {
    let options = Options {
        dark: Some(dark),
        open: case.options.open.clone(),
        template: case.options.template.clone(),
        select: case.options.select,
        view: case.options.view,
        ..Options::default()
    };
    let mut app = App::with_context(&Context::default(), options);
    (case.setup)(&mut app);
    let mut harness = themed(Harness::builder().with_size(case.size).wgpu(), app);
    // Modals and windows take a few frames to reach their size and place.
    harness.run_steps(8);
    let image = harness.render().expect("a render");
    let theme = if dark { "dark" } else { "light" };
    let path = dir.join(format!("{}-{theme}.png", case.name));
    let file = BufWriter::new(File::create(&path).unwrap());
    let mut encoder = png::Encoder::new(file, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(image.as_raw()).unwrap();
    println!("{}", path.display());
}

#[test]
#[ignore = "renders screenshots: set NSANE_RENDER_DIR"]
fn renders() {
    let dir = std::env::var_os("NSANE_RENDER_DIR").expect("NSANE_RENDER_DIR is set");
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir).unwrap();
    let only = std::env::var("NSANE_RENDER_ONLY").ok();
    for case in cases() {
        if only.as_deref().is_some_and(|o| !case.name.contains(o)) {
            continue;
        }
        for dark in [false, true] {
            render(dir, &case, dark);
        }
    }
}
