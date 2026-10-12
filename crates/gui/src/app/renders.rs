//! Renders of the window for screenshots, drawn with wgpu and no window: the
//! main view at every reference size, from phones to a large desktop, a
//! level's page, the phone's pages and sheets, and every `--view`, each in
//! both themes. Ignored by default:
//!
//! ```text
//! NSANE_RENDER_DIR=/tmp/renders cargo test -p gui renders -- --ignored
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::tests::themed;
use super::*;
use crate::test_util::repo_file;

const WIDE: Vec2 = Vec2::new(1280.0, 800.0);
const PHONE: Vec2 = Vec2::new(390.0, 844.0);

/// The reference sizes: phones upright and sideways, a tablet, a small and
/// a common laptop, a large desktop.
const SIZES: [Vec2; 7] = [
    Vec2::new(360.0, 740.0),
    PHONE,
    Vec2::new(844.0, 390.0),
    Vec2::new(768.0, 1024.0),
    Vec2::new(1024.0, 720.0),
    WIDE,
    Vec2::new(1920.0, 1080.0),
];

/// One render: the app as `options` sets it up, then `setup`, then a click
/// on the widget named `click`.
struct Case {
    name: String,
    size: Vec2,
    options: Options,
    setup: fn(&mut App),
    click: Option<&'static str>,
}

impl Case {
    /// Renders run natively, without a touch screen: a short window is a
    /// phone held sideways only when it says it has one.
    fn touch_screen(&self) -> bool {
        self.size.y < crate::layout::LANDSCAPE_PHONE_HEIGHT
    }
}

fn case(name: &str, size: Vec2, options: Options) -> Case {
    Case {
        name: name.to_owned(),
        size,
        options,
        setup: |_| {},
        click: None,
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

fn selecting(key: &str) -> Options {
    Options {
        select: LevelId::from_key(key),
        ..opening("tests/saves/game/0019.sav")
    }
}

fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    for size in SIZES {
        cases.push(case("main", size, opening("tests/saves/game/0019.sav")));
    }
    for size in [SIZES[1], SIZES[2], SIZES[3], WIDE, SIZES[6]] {
        cases.push(case("level-toadvillage", size, selecting("toadvillage")));
    }
    cases.push(case("level-ngin", PHONE, selecting("ngin")));
    let mut overview = case("overview", PHONE, opening("tests/saves/game/0019.sav"));
    overview.setup = |app| app.page = Page::Overview;
    cases.push(overview);
    let mut menu = case("menu", PHONE, opening("tests/saves/game/0019.sav"));
    menu.click = Some("Menu");
    cases.push(menu);
    cases.push(case(
        "preset-108",
        PHONE,
        Options {
            view: Some(View::PresetAll),
            ..opening("tests/saves/game/0019.sav")
        },
    ));
    cases.push(case(
        "diagnostics",
        PHONE,
        Options {
            view: Some(View::Diagnostics),
            ..opening("tests/saves/edited/t4_crystals.sav")
        },
    ));
    cases.push(case("empty", SIZES[0], Options::default()));
    cases.push(case("empty", WIDE, Options::default()));
    let toad = LevelId::from_key("toadvillage").unwrap();
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
    app.touch_screen = case.touch_screen();
    (case.setup)(&mut app);
    let mut harness = themed(Harness::builder().with_size(case.size).wgpu(), app);
    if let Some(label) = case.click {
        harness.run_steps(2);
        harness.get_by_label(label).click();
    }
    // Modals and windows take a few frames to reach their size and place.
    harness.run_steps(8);
    let image = harness.render().expect("a render");
    let theme = if dark { "dark" } else { "light" };
    let (w, h) = (case.size.x, case.size.y);
    let path = dir.join(format!("{}-{w}x{h}-{theme}.png", case.name));
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
