//! The shell, driven like a user would: through the harness's input, or the
//! actions the widgets push. File dialogs are not shown: saves go through
//! `save_to`, and the paths a dialog would return are given directly.

use std::fs;

use egui::accesskit::Role;
use egui::{ViewportEvent, ViewportId};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use nsane::igb::{IgbFile, SummaryValue};

use super::*;
use crate::prefs::tests::MemoryStorage;
use crate::test_util::{corpus, repo_file, temp_dir};

/// The save tested in game with 42 lives: 0018.sav with its lives set to 42,
/// rebuilt from its recipe.
fn lives42() -> Vec<u8> {
    corpus::live("lives42").unwrap()
}

fn app(options: Options) -> App {
    App::with_context(&Context::default(), options)
}

fn opening(path: PathBuf) -> App {
    app(Options {
        open: Some(path),
        ..Options::default()
    })
}

fn opening_options(rel: &str) -> Options {
    Options {
        open: Some(repo_file(rel)),
        ..Options::default()
    }
}

/// A harness of `app` with the app's fonts and theme. The app styled the
/// throwaway context it was made with, so the first frame styles the
/// harness's own context and draws nothing: fonts apply on the next pass.
/// The harness puts a margin around what it draws; the app gets the whole
/// window, as in eframe.
pub(super) fn themed(
    builder: egui_kittest::HarnessBuilder<App>,
    app: App,
) -> Harness<'static, App> {
    let mut styled = false;
    let mut harness = builder.build_ui_state(
        move |ui, app: &mut App| {
            if styled {
                let window = egui::UiBuilder::new().max_rect(ui.ctx().content_rect());
                ui.scope_builder(window, |ui| app.show(ui));
            } else {
                theme::fonts(ui.ctx());
                theme::apply(ui.ctx(), app.dark);
                ui.ctx().request_repaint();
                styled = true;
            }
        },
        app,
    );
    harness.run_steps(1);
    harness
}

fn harness(app: App) -> Harness<'static, App> {
    let mut harness = themed(Harness::builder().with_size(Vec2::new(1280.0, 800.0)), app);
    harness.run_steps(2);
    harness
}

/// A copy of a repository save in a fresh folder, to save over.
fn scratch_copy(rel: &str, test: &str) -> PathBuf {
    let dir = temp_dir(test);
    let path = dir.join(Path::new(rel).file_name().unwrap());
    fs::copy(repo_file(rel), &path).unwrap();
    path
}

fn doc<'a>(harness: &'a Harness<'_, App>) -> &'a Document {
    &harness
        .state()
        .file
        .as_ref()
        .expect("a document is open")
        .doc
}

fn lives(harness: &Harness<'_, App>) -> i32 {
    doc(harness).current().lives()
}

/// Clicks the lives field and types `text` over its value, without leaving.
fn start_typing_lives(harness: &mut Harness<'_, App>, text: &str) {
    harness.get_by_role(Role::SpinButton).click();
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run_steps(1);
    harness.get_by_role(Role::SpinButton).type_text(text);
    harness.run_steps(1);
}

/// Types `text` into the lives field and leaves it with Enter.
fn type_lives(harness: &mut Harness<'_, App>, text: &str) {
    start_typing_lives(harness, text);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
}

fn press(harness: &mut Harness<'_, App>, modifiers: Modifiers, key: Key) {
    harness.key_press_modifiers(modifiers, key);
    harness.run_steps(2);
}

/// Saves from the review shown before saving.
fn confirm_review(harness: &mut Harness<'_, App>) {
    assert!(
        matches!(harness.state().dialog, Some(Dialog::Review { .. })),
        "the review is shown"
    );
    harness.run_steps(1); // its first frame only measures it
    harness.get_by_label("Save file").click();
    harness.run_steps(2);
    assert!(harness.state().dialog.is_none());
}

fn backups(dir: &Path) -> Vec<PathBuf> {
    let mut found: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "bak"))
        .collect();
    found.sort();
    found
}

#[test]
fn opens_a_switch_save_with_its_budget() {
    let harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    assert!(harness.state().dialog.is_none());
    assert_eq!(doc(&harness).platform(), nsane::Platform::Switch);
    harness.get_by_label("Switch");
    harness.get_by_label_contains("/ 150 KB");
    harness.get_by_label("0019.sav");
    assert_eq!(harness.state().title, format!("0019.sav — {APP_TITLE}"));
}

#[test]
fn opens_a_pc_save_without_budget() {
    let harness = harness(opening(repo_file("tests/saves/game/ALCHEMYXSAVE_8.sav")));
    assert_eq!(doc(&harness).platform(), nsane::Platform::Pc);
    harness.get_by_label("PC");
    assert!(harness.query_by_label_contains(" KB").is_none());
}

#[test]
fn a_file_that_is_not_a_save_shows_an_error() {
    let mut harness = harness(opening(repo_file("Cargo.toml")));
    let Some(Dialog::Error { title, message }) = &harness.state().dialog else {
        panic!("an error dialog");
    };
    assert_eq!(title, "Can't open Cargo.toml");
    assert!(
        message.starts_with("Not an N. Sane Trilogy save: "),
        "{message}"
    );
    assert!(harness.state().file.is_none());
    harness.get_by_label("OK").click();
    harness.run_steps(2);
    assert!(harness.state().dialog.is_none(), "the app keeps running");
}

#[test]
fn a_missing_file_shows_an_error() {
    let harness = harness(opening(repo_file("tests/saves/game/no_such_file.sav")));
    assert!(matches!(harness.state().dialog, Some(Dialog::Error { .. })));
}

#[test]
fn a_truncated_save_does_not_panic_and_keeps_the_open_one() {
    let bytes = fs::read(repo_file("tests/saves/game/0019.sav")).unwrap();
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    // within the content: a Switch file's padding is not needed to read it
    for cut in [0, 4, 100, 1000, 20_000, 40_000] {
        harness
            .state_mut()
            .open_bytes(&bytes[..cut], "cut.sav".into(), None);
        harness.run_steps(1);
        assert!(
            matches!(harness.state().dialog, Some(Dialog::Error { .. })),
            "cut at {cut}"
        );
        harness.state_mut().dialog = None;
        assert_eq!(doc(&harness).file_name(), Some("0018.sav"));
    }
}

#[test]
fn save_without_edits_is_byte_identical_and_backs_up() {
    let path = scratch_copy("tests/saves/game/0019.sav", "noop");
    let dir = path.parent().unwrap().to_owned();
    let original = fs::read(&path).unwrap();
    let mut harness = harness(opening(path.clone()));
    press(&mut harness, Modifiers::COMMAND, Key::S);
    assert_eq!(fs::read(&path).unwrap(), original);
    let found = backups(&dir);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(fs::read(&found[0]).unwrap(), original);
    let toast = &harness.state().toasts.last().unwrap().text;
    assert!(toast.contains(&found[0].display().to_string()), "{toast}");

    // Save As to a new path writes no backup.
    let other = dir.join("copy.sav");
    assert!(harness.state_mut().save_to(other.clone()));
    assert_eq!(fs::read(&other).unwrap(), original);
    assert_eq!(backups(&dir).len(), 1);
    assert_eq!(doc(&harness).file_name(), Some("copy.sav"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn lives_42_gives_the_file_tested_in_game() {
    let path = scratch_copy("tests/saves/game/0018.sav", "lives42");
    let mut harness = harness(opening(path.clone()));
    assert_eq!(lives(&harness), 5);

    type_lives(&mut harness, "42");
    assert_eq!(lives(&harness), 42);
    assert_eq!(doc(&harness).history().collect::<Vec<_>>(), ["Lives: 42"]);
    assert!(harness.state().title.starts_with("• 0018.sav"));
    harness.get_by_label("• 1 change");

    press(&mut harness, Modifiers::COMMAND, Key::S);
    confirm_review(&mut harness);
    assert!(
        fs::read(&path).unwrap() == lives42(),
        "byte-identical to lives42.sav"
    );
    assert!(!doc(&harness).is_dirty());

    // One undo restores 5. The saved state is 42, so 5 is unsaved.
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(lives(&harness), 5);
    assert!(doc(&harness).is_dirty());
    // Back to the saved state: the dirty dot goes.
    press(&mut harness, Modifiers::COMMAND, Key::Y);
    assert_eq!(lives(&harness), 42);
    assert!(!doc(&harness).is_dirty());
    assert!(harness.query_by_label_contains("•").is_none());
    assert!(!harness.state().title.starts_with('•'));
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn undoing_to_the_opened_state_clears_the_dirty_dot() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    type_lives(&mut harness, "42");
    harness.get_by_label("• 1 change");
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(lives(&harness), 5);
    assert!(!doc(&harness).is_dirty());
    assert!(harness.query_by_label_contains("•").is_none());
    // Ctrl+Shift+Z redoes too.
    press(&mut harness, Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
    assert_eq!(lives(&harness), 42);
}

#[test]
fn a_drag_is_one_undo_step() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    let start = harness.get_by_role(Role::SpinButton).rect().center();
    harness.drag_at(start);
    harness.run_steps(1);
    for dx in 1..=40 {
        harness.hover_at(start + Vec2::new(dx as f32 * 2.0, 0.0));
        harness.run_steps(1);
        assert!(
            doc(&harness).history().next().is_none(),
            "nothing applied while dragging"
        );
    }
    harness.drop_at(start + Vec2::new(80.0, 0.0));
    harness.run_steps(2);
    assert!(lives(&harness) > 5, "{}", lives(&harness));
    assert_eq!(doc(&harness).history().count(), 1);
}

#[test]
fn typed_lives_are_clamped() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    type_lives(&mut harness, "999");
    assert_eq!(lives(&harness), 255);
}

#[test]
fn save_shortcut_while_typing_includes_the_typed_value() {
    let path = scratch_copy("tests/saves/game/0018.sav", "typing");
    let mut harness = harness(opening(path.clone()));
    start_typing_lives(&mut harness, "42");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(3);
    confirm_review(&mut harness);
    assert_eq!(fs::read(&path).unwrap(), lives42());
    assert!(!doc(&harness).is_dirty());
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn out_of_range_lives_are_shown_as_they_are() {
    // as another tool could write it
    let mut igb =
        IgbFile::parse(&fs::read(repo_file("tests/saves/game/0018.sav")).unwrap()).unwrap();
    igb.summary_mut()
        .set("life_count", SummaryValue::Int(300))
        .unwrap();
    let bytes = igb.build().unwrap();
    let mut harness = harness(app(Options::default()));
    harness
        .state_mut()
        .open_bytes(&bytes, "many.sav".into(), None);
    harness.run_steps(2);
    assert_eq!(lives(&harness), 300);
    assert_eq!(
        harness.get_by_role(Role::SpinButton).value().as_deref(),
        Some("300")
    );
    assert!(
        doc(&harness).history().next().is_none(),
        "not clamped on view"
    );
}

#[test]
fn the_dirty_guard_asks_before_open() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    type_lives(&mut harness, "42");
    press(&mut harness, Modifiers::COMMAND, Key::O);
    assert_eq!(harness.state().dialog, Some(Dialog::Unsaved(Then::Open)));
    harness.get_by_label_contains("1 change since it was opened");
    harness.get_by_label("Cancel").click();
    harness.run_steps(2);
    assert!(harness.state().dialog.is_none());
    assert_eq!(lives(&harness), 42);

    // A dropped file asks too; Discard opens it.
    let other = repo_file("tests/saves/game/0019.sav");
    harness
        .state_mut()
        .run(&Context::default(), vec![Action::OpenPath(other.clone())]);
    assert_eq!(
        harness.state().dialog,
        Some(Dialog::Unsaved(Then::OpenPath(other)))
    );
    harness.run_steps(1);
    harness.get_by_label("Discard").click();
    harness.run_steps(2);
    assert!(harness.state().dialog.is_none());
    assert_eq!(doc(&harness).file_name(), Some("0019.sav"));
    assert!(!doc(&harness).is_dirty());
}

#[test]
fn the_dirty_guard_asks_before_close() {
    fn request_close(harness: &mut Harness<'_, App>) {
        harness
            .input_mut()
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .events
            .push(ViewportEvent::Close);
        harness.step();
    }
    fn close_cancelled(harness: &Harness<'_, App>) -> bool {
        harness
            .output()
            .viewport_output
            .get(&ViewportId::ROOT)
            .is_some_and(|v| v.commands.contains(&ViewportCommand::CancelClose))
    }

    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    request_close(&mut harness);
    assert!(!close_cancelled(&harness), "clean: closes");
    assert!(harness.state().dialog.is_none());

    type_lives(&mut harness, "42");
    request_close(&mut harness);
    assert!(close_cancelled(&harness));
    assert_eq!(harness.state().dialog, Some(Dialog::Unsaved(Then::Close)));

    harness.run_steps(1); // the dialog's first frame only measures it
    harness.get_by_label("Discard").click();
    harness.run_steps(2);
    assert!(harness.state().closing);
    assert!(harness.state().dialog.is_none());
}

#[test]
fn shortcuts_wait_while_a_dialog_is_open() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    type_lives(&mut harness, "42");
    harness.state_mut().error("Test", "message");
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(lives(&harness), 42);
}

#[test]
fn a_switch_save_that_outgrows_its_file_is_not_written() {
    let path = scratch_copy("tests/saves/game/0018.sav", "too-big");
    let dir = path.parent().unwrap().to_owned();
    let original = fs::read(&path).unwrap();
    let mut harness = harness(opening(path.clone()));
    // flag keys long enough to outgrow the fixed file size
    let edits: Vec<_> = (0..40)
        .map(|i| Edit::RawFlagSet {
            key: format!("{i}_{}", "x".repeat(4000)),
        })
        .collect();
    harness.state_mut().apply("big".into(), edits);
    assert!(!doc(&harness).size_report().fits());
    assert!(!harness.state_mut().save_to(path.clone()));
    let Some(Dialog::Error { title, .. }) = &harness.state().dialog else {
        panic!("an error dialog");
    };
    assert_eq!(title, "The save doesn't fit");
    assert_eq!(fs::read(&path).unwrap(), original, "nothing written");
    assert!(backups(&dir).is_empty());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_unsaved_view_and_a_selected_level() {
    let harness = harness(app(Options {
        open: Some(repo_file("tests/saves/game/0019.sav")),
        view: Some(View::Unsaved),
        select: LevelId::from_key("toadvillage"),
        ..Options::default()
    }));
    harness.get_by_label("Unsaved changes");
    assert_eq!(harness.state().selected, LevelId::from_key("toadvillage"));
    // The page: the row of the room's card, and the title.
    assert_eq!(harness.query_all_by_label("Toad Village").count(), 2);
}

#[test]
fn crash_1_and_2_are_disabled() {
    let harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    for title in ["Crash Bandicoot", "Crash 2: Cortex Strikes Back"] {
        assert!(harness.get_by_label(title).accesskit_node().is_disabled());
    }
    assert!(
        !harness
            .get_by_label("Crash 3: Warped")
            .accesskit_node()
            .is_disabled()
    );
}

#[test]
fn a_dropped_file_asks_before_replacing_unsaved_changes() {
    let bytes = fs::read(repo_file("tests/saves/game/0019.sav")).unwrap();
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    type_lives(&mut harness, "42");
    let drop = Action::OpenBytes {
        name: "0019.sav".into(),
        bytes: bytes.clone(),
    };
    harness
        .state_mut()
        .run(&Context::default(), vec![drop.clone()]);
    assert!(matches!(
        &harness.state().dialog,
        Some(Dialog::Unsaved(Then::OpenBytes { name, .. })) if name == "0019.sav"
    ));
    harness.run_steps(2); // the dialog's first frame only measures it
    harness.get_by_label("Discard").click();
    harness.run_steps(2);
    let file = harness.state().file.as_ref().unwrap();
    assert_eq!(file.doc.file_name(), Some("0019.sav"));
    assert_eq!(file.path, None, "no place on disk: Save asks where");
    assert!(!file.doc.is_dirty());

    // Clean: it opens right away. Not a save: an error, and the save stays.
    harness.state_mut().run(
        &Context::default(),
        vec![Action::OpenBytes {
            name: "notes.txt".into(),
            bytes: b"not a save".to_vec(),
        }],
    );
    assert!(matches!(harness.state().dialog, Some(Dialog::Error { .. })));
    assert_eq!(doc(&harness).file_name(), Some("0019.sav"));
}

/// The app as `App::new` makes it, with the settings in `storage`.
fn app_with_storage(storage: &MemoryStorage, options: Options) -> App {
    App::with_prefs(
        &Context::default(),
        options,
        Some(Prefs::load(Some(storage))),
    )
}

#[test]
fn the_theme_survives_a_restart() {
    let mut storage = MemoryStorage::default();
    let mut first = harness(app_with_storage(
        &storage,
        opening_options("tests/saves/game/0019.sav"),
    ));
    let dark = first.state().dark;
    // A phone: the theme is in the ☰ menu.
    first.set_size(Vec2::new(600.0, 800.0));
    first.run_steps(2);
    click(&mut first, "Menu");
    click(&mut first, if dark { "Light theme" } else { "Dark theme" });
    assert_eq!(first.state().dark, !dark);
    eframe::App::save(first.state_mut(), &mut storage);
    drop(first);

    let second = app_with_storage(&storage, opening_options("tests/saves/game/0019.sav"));
    assert_eq!(second.dark, !dark);

    // A theme given on the command line wins, and is not kept.
    let mut forced = harness(app_with_storage(
        &storage,
        Options {
            dark: Some(dark),
            ..Options::default()
        },
    ));
    assert_eq!(forced.state().dark, dark);
    forced.run_steps(1);
    eframe::App::save(forced.state_mut(), &mut storage);
    assert_eq!(Prefs::load(Some(&storage)).dark, Some(!dark));
}

#[test]
fn settings_are_not_kept_without_storage() {
    // as in screenshot runs and the other tests
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    let dark = harness.state().dark;
    harness.get_by_label(if dark { "☀" } else { "🌙" }).click();
    harness.run_steps(2);
    let mut storage = MemoryStorage::default();
    eframe::App::save(harness.state_mut(), &mut storage);
    assert!(storage.0.is_empty());
}

#[test]
fn the_last_folder_is_remembered() {
    let path = scratch_copy("tests/saves/game/0018.sav", "last-dir");
    let dir = path.parent().unwrap().to_owned();
    let mut harness = harness(opening(path.clone()));
    assert_eq!(
        harness.state().prefs.last_dir.as_deref(),
        Some(dir.as_path())
    );

    let other = temp_dir("last-dir-other");
    assert!(harness.state_mut().save_to(other.join("copy.sav")));
    assert_eq!(harness.state().prefs.last_dir, Some(other.clone()));

    // a file that isn't a save leaves it
    harness.state_mut().open_path(repo_file("Cargo.toml"));
    assert_eq!(harness.state().prefs.last_dir, Some(other.clone()));
    fs::remove_dir_all(dir).unwrap();
    fs::remove_dir_all(other).unwrap();
}

#[test]
fn sizes_in_kb() {
    assert_eq!(kib(153_600), 150);
    assert_eq!(kib(65_000), 63);
    assert_eq!(kib(511), 0);
    assert_eq!(kib(512), 1);
}

// ------------------------------------------------------------ Crash 3 screen

/// A window tall enough to show every warp room card without scrolling.
fn tall_harness(app: App) -> Harness<'static, App> {
    let mut harness = themed(Harness::builder().with_size(Vec2::new(1280.0, 1400.0)), app);
    harness.run_steps(2);
    harness
}

fn level(key: &str) -> LevelId {
    LevelId::from_key(key).unwrap()
}

fn history(harness: &Harness<'_, App>) -> Vec<String> {
    doc(harness).history().map(str::to_owned).collect()
}

fn click(harness: &mut Harness<'_, App>, label: &str) {
    harness.get_by_label(label).click();
    harness.run_steps(2);
}

/// The expected output of a golden scenario, from the manifest.
fn golden_sha256(name: &str) -> String {
    let text = fs::read_to_string(repo_file("tests/golden/manifest.json")).unwrap();
    let manifest: serde_json::Value = serde_json::from_str(&text).unwrap();
    manifest["scenarios"][name]["sha256"]
        .as_str()
        .unwrap_or_else(|| panic!("{name} is not in the manifest"))
        .to_owned()
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn a_crystal_click_is_one_undo_step_and_matches_the_golden_output() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    let deep_trouble = level("deeptrouble");
    harness.get_by_label("10 / 25");
    harness.get_by_label("19%");
    assert!(!doc(&harness).current().crystal(deep_trouble));

    click(&mut harness, "Deep Trouble: crystal");
    assert!(doc(&harness).current().crystal(deep_trouble));
    assert_eq!(history(&harness), ["Deep Trouble: crystal on"]);
    // The totals follow at once, from the save as it is now.
    harness.get_by_label("11 / 25");
    harness.get_by_label("20%");
    harness.get_by_label("1 / 5");
    // The same bytes as the golden scenario of `SetCrystal` on this save.
    let bytes = doc(&harness).to_bytes().unwrap();
    assert_eq!(sha256_hex(&bytes), golden_sha256("ops/set_crystal_on_0019"));

    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert!(!doc(&harness).current().crystal(deep_trouble));
    assert!(history(&harness).is_empty());
    harness.get_by_label("10 / 25");
    harness.get_by_label("19%");
}

#[test]
fn every_collectible_toggle_is_one_edit() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    for (label, expected) in [
        ("Deep Trouble: clear gem", "Deep Trouble: clear gem on"),
        ("Deep Trouble: red gem", "Deep Trouble: red gem on"),
        ("Bone Yard: second gem", "Bone Yard: second gem on"),
        ("Toad Village: crystal", "Toad Village: crystal off"),
        ("Dr. N. Tropy: defeated", "Dr. N. Tropy: defeated"),
        ("Dr. N. Tropy: Death Tornado Spin", "Death Tornado Spin: on"),
        ("Hub: bonus gem", "Bonus gem: on"),
    ] {
        let before = history(&harness).len();
        click(&mut harness, label);
        let after = history(&harness);
        assert_eq!(after.len(), before + 1, "{label}");
        assert_eq!(after.last().unwrap(), expected, "{label}");
    }
    let save = doc(&harness).current();
    assert!(save.gem(level("deeptrouble")));
    assert!(save.colored_gem(level("deeptrouble")));
    assert!(save.second_gem(level("boneyard")));
    assert!(!save.crystal(level("toadvillage")));
    assert!(save.boss_defeated(level("ntropy")));
    assert!(save.ability(nsane::crash3::Ability::TornadoSpin));
    assert!(save.bonus_gem());
    // A level that was never entered is marked entered by the first edit.
    assert!(save.entered(level("deeptrouble")));
}

#[test]
fn the_relic_cycles_on_click_and_is_picked_on_right_click() {
    use nsane::crash3::Relic;
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    let toad = level("toadvillage");
    let relic = |h: &Harness<'_, App>| doc(h).current().relic(toad);
    assert_eq!(relic(&harness), Some(Relic::Gold));
    click(&mut harness, "Toad Village: relic");
    assert_eq!(relic(&harness), Some(Relic::Platinum));
    click(&mut harness, "Toad Village: relic");
    assert_eq!(relic(&harness), None);
    click(&mut harness, "Toad Village: relic");
    assert_eq!(relic(&harness), Some(Relic::Sapphire));
    assert_eq!(history(&harness).len(), 3);

    harness
        .get_by_label("Toad Village: relic")
        .click_secondary();
    harness.run_steps(2);
    harness.get_by_label_contains("Gold (under").click();
    harness.run_steps(2);
    assert_eq!(relic(&harness), Some(Relic::Gold));
    assert_eq!(
        history(&harness).last().unwrap(),
        "Toad Village: gold relic"
    );
    assert_eq!(history(&harness).len(), 4);
}

#[test]
fn a_level_name_selects_it() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    assert_eq!(harness.state().selected, None);
    click(&mut harness, "Deep Trouble");
    assert_eq!(harness.state().selected, Some(level("deeptrouble")));
    assert!(history(&harness).is_empty(), "selecting is not an edit");
}

#[test]
fn the_secret_room_locks_follow_the_relic_count() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    // One relic: every level of the secret room is locked.
    for name in ["26 Ski Crazed", "28 Area 51?", "30 Rings of Power"] {
        harness.get_by_label(&format!("{name}: locked"));
    }
    // Four more relics open level 26, and only it.
    for key in ["underpressure", "orientexpress", "boneyard", "makinwaves"] {
        click(&mut harness, &format!("{}: relic", level(key).name()));
    }
    assert_eq!(doc(&harness).current().totals().relics(), 5);
    harness.get_by_label("26 Ski Crazed: open");
    harness.get_by_label("27 Hang'em High: locked");
    harness.get_by_label("28 Area 51?: locked");

    let harness = tall_harness(opening(repo_file("tests/saves/after_edit/t6_after.sav")));
    for name in [
        "26 Ski Crazed",
        "27 Hang'em High",
        "28 Area 51?",
        "29 Future Frenzy",
        "30 Rings of Power",
    ] {
        harness.get_by_label(&format!("{name}: open"));
    }
}

#[test]
fn the_entrance_rows_select_their_level() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    click(&mut harness, "Hang'em High (alt.)");
    assert_eq!(harness.state().selected, Some(level("hangemhigh")));
    // The page shows room 2's card: the secret room's is back on the grid.
    harness.get_by_label("Hang'em High: crystal");
    click(&mut harness, "All rooms");
    click(&mut harness, "Future Frenzy (alt.)");
    assert_eq!(harness.state().selected, Some(level("futurefrenzy")));
    assert!(history(&harness).is_empty());
}

#[test]
fn the_portal_glitch_shows_on_its_room() {
    let harness = harness(opening(repo_file("tests/saves/edited/t4_crystals.sav")));
    let message = doc(&harness)
        .diagnostics()
        .iter()
        .find(|d| d.code == "portal_glitch")
        .expect("a portal glitch")
        .message
        .clone();
    // Once on Warp Room 4's card, once in the overview's list.
    assert_eq!(harness.query_all_by_label(&message).count(), 2);
}

#[test]
fn the_bonus_gem_badge_shows_when_it_is_due() {
    let harness = harness(opening(repo_file("tests/saves/edited/t7_bonus_gem.sav")));
    assert!(doc(&harness).current().bonus_gem_due());
    harness.get_by_label("due");
    let harness = self::harness(opening(repo_file("tests/saves/game/0019.sav")));
    assert!(harness.query_by_label("due").is_none());
}

#[test]
fn the_overview_toggles_the_colored_gems() {
    use egui::accesskit::Toggled;
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    for color in ["red", "yellow", "purple", "blue", "green"] {
        harness.get_by_label(&format!("Overview: {color} gem"));
    }
    let toggled =
        |h: &Harness<'_, App>, label: &str| h.get_by_label(label).accesskit_node().toggled();
    assert_eq!(
        toggled(&harness, "Tomb Wader: blue gem"),
        Some(Toggled::False)
    );
    click(&mut harness, "Overview: blue gem");
    assert_eq!(history(&harness), ["Tomb Wader: blue gem on"]);
    assert!(doc(&harness).current().colored_gem(level("tombwader")));
    // The grid's toggle is the same gem.
    assert_eq!(
        toggled(&harness, "Tomb Wader: blue gem"),
        Some(Toggled::True)
    );
    assert_eq!(toggled(&harness, "Overview: blue gem"), Some(Toggled::True));
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert!(!doc(&harness).current().colored_gem(level("tombwader")));
}

/// Lives are the first thing the overview edits: above the totals, in view
/// in the smallest reference window.
#[test]
fn lives_come_first_in_the_overview() {
    let mut harness = themed(
        Harness::builder().with_size(Vec2::new(1024.0, 720.0)),
        opening(repo_file("tests/saves/game/0019.sav")),
    );
    harness.run_steps(2);
    let field = harness.get_by_role(Role::SpinButton).rect();
    let collected = harness.get_by_label("Collected").rect();
    assert!(field.bottom() < collected.top(), "{field:?} {collected:?}");
    assert!(field.bottom() < 720.0 / 2.0, "{field:?}");
}

/// In the smallest reference window, the grid keeps two columns beside the
/// overview's.
#[test]
fn a_small_laptop_shows_two_rooms_side_by_side() {
    let mut harness = themed(
        Harness::builder().with_size(Vec2::new(1024.0, 720.0)),
        opening(repo_file("tests/saves/game/0019.sav")),
    );
    harness.run_steps(2);
    let first = harness.get_by_label("Warp Room 1").rect();
    let second = harness.get_by_label("Warp Room 2").rect();
    assert!(second.left() > first.right(), "{first:?} {second:?}");
    assert_eq!(second.top(), first.top());
}

// ------------------------------------------------------------ level panel

fn selecting(rel: &str, key: &str) -> Options {
    Options {
        select: Some(level(key)),
        ..opening_options(rel)
    }
}

/// Leaderboard field `slot` (0–2) of the level panel.
fn time_field<'a>(harness: &'a Harness<'_, App>, slot: usize) -> egui_kittest::Node<'a> {
    harness
        .get_all_by_role(Role::TextInput)
        .nth(slot)
        .expect("a leaderboard field")
}

/// Types `text` over leaderboard field `slot` and leaves it with Enter.
fn type_time(harness: &mut Harness<'_, App>, slot: usize, text: &str) {
    time_field(harness, slot).click();
    harness.run_steps(2);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run_steps(1);
    if text.is_empty() {
        harness.key_press(Key::Backspace);
    } else {
        time_field(harness, slot).type_text(text);
    }
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
}

/// Every session field but the completions counter, strings as text.
fn session_values(save: &nsane::crash3::Crash3Save, level: LevelId) -> Vec<String> {
    use nsane::crash3::tables::field;
    let session = save.session(level).unwrap();
    session
        .fields
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != field::COMPLETIONS)
        .map(|(_, v)| match *v {
            nsane::igb::FieldValue::Str(idx) => {
                String::from_utf8_lossy(save.igb().string(idx).unwrap_or_default()).into_owned()
            }
            nsane::igb::FieldValue::Bool { value, .. } => value.to_string(),
            ref v => format!("{:?}", v.as_u32()),
        })
        .collect()
}

#[test]
fn the_panel_gives_the_gold_relic_the_game_wrote() {
    use nsane::crash3::Crash3Save;
    let mut harness = harness(app(selecting("tests/saves/game/0010.sav", "toadvillage")));
    let toad = level("toadvillage");
    harness.get_by_label("Time trial");
    click(&mut harness, "Gold");
    type_time(&mut harness, 1, "0:55.44");
    assert_eq!(
        history(&harness),
        [
            "Toad Village: gold relic",
            "Toad Village: times 0:55.44, 1:00.23"
        ]
    );
    let expected =
        Crash3Save::parse(&fs::read(repo_file("tests/saves/game/0011.sav")).unwrap()).unwrap();
    let save = doc(&harness).current();
    assert_eq!(session_values(save, toad), session_values(&expected, toad));
    // The game also counted the finish, which no edit writes.
    assert_eq!(save.completions(toad) + 1, expected.completions(toad));
    // The fields show the sorted leaderboard.
    assert_eq!(time_field(&harness, 0).value().as_deref(), Some("0:55.44"));
    assert_eq!(time_field(&harness, 1).value().as_deref(), Some("1:00.23"));
}

#[test]
fn invalid_times_are_refused_and_valid_ones_are_one_step_each() {
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "toadvillage")));
    let toad = level("toadvillage");
    for text in ["1:7x", "99:99:99", "-1", "100:00.00"] {
        type_time(&mut harness, 2, text);
        assert!(history(&harness).is_empty(), "{text:?}");
        assert_eq!(doc(&harness).current().times(toad), [5544, 6023]);
        // The text stays, outlined, so it can be corrected.
        assert_eq!(time_field(&harness, 2).value().as_deref(), Some(text));
    }
    for (i, (text, cs)) in [("0:58.00", 5800), ("0:57:00", 5700), ("56.00", 5600)]
        .into_iter()
        .enumerate()
    {
        type_time(&mut harness, 2, text);
        assert_eq!(history(&harness).len(), i + 1, "{text:?}");
        assert!(
            doc(&harness).current().times(toad).contains(&cs),
            "{text:?}"
        );
    }
    // Each time replaced the slowest one, in the third field.
    assert_eq!(doc(&harness).current().times(toad), [5544, 5600, 5700]);
    // Undo restores the fields from the save.
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(doc(&harness).current().times(toad), [5544, 5700, 5800]);
    assert_eq!(time_field(&harness, 2).value().as_deref(), Some("0:58.00"));
    // Emptying a field removes its time.
    type_time(&mut harness, 0, "");
    assert_eq!(doc(&harness).current().times(toad), [5700, 5800]);
}

#[test]
fn fill_a_time_and_set_the_relic_from_the_best_time() {
    use nsane::crash3::Relic;
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "toadvillage")));
    let toad = level("toadvillage");
    click(&mut harness, "Platinum");
    click(&mut harness, "Fill a time for this relic");
    // Toad Village platinum is under 0:44.03.
    assert_eq!(doc(&harness).current().times(toad), [4402, 5544, 6023]);
    assert_eq!(history(&harness).len(), 2);

    click(&mut harness, "Sapphire");
    click(&mut harness, "Set relic from best time");
    assert_eq!(doc(&harness).current().relic(toad), Some(Relic::Platinum));
    assert_eq!(history(&harness).len(), 4);
    // Both are done: the buttons do nothing more.
    click(&mut harness, "Set relic from best time");
    click(&mut harness, "Fill a time for this relic");
    assert_eq!(history(&harness).len(), 4);
}

#[test]
fn reset_asks_first_and_undo_restores_the_level() {
    // Tall: the reset button is at the end of the details.
    let mut harness = tall_harness(app(selecting(
        "tests/saves/after_edit/t6_after.sav",
        "deeptrouble",
    )));
    let deep = level("deeptrouble");
    let before = doc(&harness).current().totals();
    assert!(doc(&harness).current().entered(deep));
    open_dialog(&mut harness, "Reset level…");
    harness.get_by_label(
        "Reset Deep Trouble to never played? Removes its crystal, gems, relic and times.",
    );
    assert!(harness.query_by_label("Not yet tested in game.").is_none());
    click(&mut harness, "Cancel");
    assert!(history(&harness).is_empty());

    open_dialog(&mut harness, "Reset level…");
    click(&mut harness, "Reset");
    assert_eq!(history(&harness), ["Deep Trouble: reset to never played"]);
    let save = doc(&harness).current();
    assert!(!save.entered(deep));
    let after = save.totals();
    assert_eq!(after.crystals + 1, before.crystals);
    assert_eq!(after.relics() + 1, before.relics());
    assert!(after.percent() < before.percent());

    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert!(doc(&harness).current().entered(deep));
    assert_eq!(doc(&harness).current().totals(), before);
}

#[test]
fn the_boss_panel_toggles_defeat_ability_and_scenes() {
    use nsane::crash3::Ability;
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "ngin")));
    let ngin = level("ngin");
    click(&mut harness, "Defeated");
    click(&mut harness, "Ability: Fruit Bazooka");
    click(&mut harness, "C3_NGin_Intro");
    let save = doc(&harness).current();
    assert!(save.boss_defeated(ngin));
    assert!(save.ability(Ability::Bazooka));
    assert!(save.cutscene_seen("C3_NGin_Intro"));
    assert_eq!(history(&harness).len(), 3);
    // No time trial for a boss.
    assert!(harness.query_by_label("Time trial").is_none());
}

#[test]
fn the_invalid_time_view_shows_a_refused_entry() {
    let options = Options {
        view: Some(View::InvalidTime),
        ..selecting("tests/saves/game/0019.sav", "toadvillage")
    };
    let mut harness = harness(app(options));
    assert_eq!(time_field(&harness, 2).value().as_deref(), Some("1:7x"));
    assert!(history(&harness).is_empty());
    // A valid time typed over it is applied.
    type_time(&mut harness, 2, "1:10.00");
    assert_eq!(
        doc(&harness).current().times(level("toadvillage")),
        [5544, 6023, 7000]
    );
}

// ------------------------------------------------------------ level page

fn selected(harness: &Harness<'_, App>) -> Option<LevelId> {
    harness.state().selected
}

/// Starts typing `text` in leaderboard field `slot`, without leaving it.
fn start_typing_time(harness: &mut Harness<'_, App>, slot: usize, text: &str) {
    time_field(harness, slot).click();
    harness.run_steps(2);
    time_field(harness, slot).type_text(text);
    harness.run_steps(1);
}

#[test]
fn a_level_opens_its_page_in_place_of_the_grid() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    click(&mut harness, "Deep Trouble");
    assert_eq!(selected(&harness), Some(level("deeptrouble")));
    harness.get_by_label("All rooms");
    let tab = harness.get_by_label("Show warp room 3");
    assert_eq!(
        tab.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    // The room's card, beside the details.
    harness.get_by_label("Deep Trouble: crystal");
    harness.get_by_label("Time trial");
    assert!(harness.query_by_label("Toad Village").is_none(), "no grid");

    click(&mut harness, "All rooms");
    assert_eq!(selected(&harness), None);
    harness.get_by_label("Toad Village");
    assert!(harness.query_by_label("All rooms").is_none());
}

#[test]
fn escape_and_the_mouse_back_button_go_back_to_the_grid() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    click(&mut harness, "Deep Trouble");
    press(&mut harness, Modifiers::NONE, Key::Escape);
    assert_eq!(selected(&harness), None);

    click(&mut harness, "Deep Trouble");
    harness
        .get_by_label("Time trial")
        .click_button(egui::PointerButton::Extra1);
    harness.run_steps(2);
    assert_eq!(selected(&harness), None);
    assert!(history(&harness).is_empty());
}

#[test]
fn escape_in_a_time_field_drops_the_text_and_keeps_the_page() {
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "toadvillage")));
    start_typing_time(&mut harness, 2, "1:10.00");
    press(&mut harness, Modifiers::NONE, Key::Escape);
    assert_eq!(selected(&harness), Some(level("toadvillage")));
    assert!(history(&harness).is_empty());
    assert_eq!(time_field(&harness, 2).value().unwrap_or_default(), "");
    // Nothing has focus now: the next Escape goes back.
    press(&mut harness, Modifiers::NONE, Key::Escape);
    assert_eq!(selected(&harness), None);
}

#[test]
fn escape_closes_the_reset_question_only() {
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "toadvillage")));
    open_dialog(&mut harness, "Reset level…");
    assert!(harness.state().crash3.has_dialog());
    press(&mut harness, Modifiers::NONE, Key::Escape);
    assert!(!harness.state().crash3.has_dialog());
    assert_eq!(selected(&harness), Some(level("toadvillage")));
    // The mouse's back button waits for the question too.
    open_dialog(&mut harness, "Reset level…");
    harness
        .get_by_label("Time trial")
        .click_button(egui::PointerButton::Extra1);
    harness.run_steps(2);
    assert_eq!(selected(&harness), Some(level("toadvillage")));
}

#[test]
fn the_room_tabs_and_the_card_switch_level() {
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "deeptrouble")));
    click(&mut harness, "Show warp room 4");
    assert_eq!(selected(&harness), Some(level("sphynxinator")));
    // A row of the room's card.
    click(&mut harness, "Tell No Tales");
    assert_eq!(selected(&harness), Some(level("tellnotales")));
    click(&mut harness, "Show the secret warp room");
    assert_eq!(selected(&harness), Some(level("skicrazed")));
    // An entrance row shows its level, in its own room.
    click(&mut harness, "Future Frenzy (alt.)");
    assert_eq!(selected(&harness), Some(level("futurefrenzy")));
    let tab = harness.get_by_label("Show warp room 4");
    assert_eq!(
        tab.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    assert!(history(&harness).is_empty(), "selecting is not an edit");
}

#[test]
fn a_time_being_typed_is_applied_when_another_level_is_picked() {
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "toadvillage")));
    start_typing_time(&mut harness, 2, "1:10.00");
    harness.get_by_label("Under Pressure").click();
    harness.run_steps(2);
    assert_eq!(selected(&harness), Some(level("underpressure")));
    assert_eq!(
        doc(&harness).current().times(level("toadvillage")),
        [5544, 6023, 7000]
    );
    assert_eq!(history(&harness).len(), 1);
    // The new level's fields show its own leaderboard.
    assert_eq!(time_field(&harness, 0).value().unwrap_or_default(), "");
}

#[test]
fn a_toggle_on_the_room_card_is_one_step() {
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "deeptrouble")));
    click(&mut harness, "High Time: crystal");
    assert_eq!(history(&harness), ["High Time: crystal on"]);
    assert_eq!(
        selected(&harness),
        Some(level("deeptrouble")),
        "the page stays"
    );
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert!(!doc(&harness).current().crystal(level("hightime")));
}

/// Without the room's card, ‹ and › go through its rows.
#[test]
fn previous_and_next_step_through_the_room() {
    let mut harness = themed(
        Harness::builder().with_size(Vec2::new(800.0, 1000.0)),
        app(selecting("tests/saves/game/0019.sav", "dinomight")),
    );
    harness.run_steps(2);
    assert!(
        harness.query_by_label("Deep Trouble: crystal").is_none(),
        "no card"
    );
    assert!(disabled(&harness, "Previous level"));
    for key in [
        "deeptrouble",
        "hightime",
        "roadcrash",
        "doubleheader",
        "ntropy",
    ] {
        click(&mut harness, "Next level");
        assert_eq!(selected(&harness), Some(level(key)));
    }
    assert!(disabled(&harness, "Next level"));
    click(&mut harness, "Previous level");
    assert_eq!(selected(&harness), Some(level("doubleheader")));
}

/// The grid is back where it was scrolled to, whether the page's details
/// scroll beside the room's card or in the grid's place.
#[test]
fn the_grid_keeps_its_scroll_position() {
    for size in [Vec2::new(1280.0, 800.0), Vec2::new(800.0, 1000.0)] {
        let mut harness = themed(
            Harness::builder().with_size(size),
            opening(repo_file("tests/saves/game/0019.sav")),
        );
        harness.run_steps(2);
        // Measured with the pointer over the overview: a hovered button
        // grows by a point.
        let top = |h: &mut Harness<'_, App>| {
            h.hover_at(egui::pos2(20.0, size.y - 20.0));
            h.run_steps(2);
            h.get_by_label("Warp Room 3").rect().top()
        };
        let start = top(&mut harness);
        harness.hover_at(harness.get_by_label("Warp Room 1").rect().center());
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: Vec2::new(0.0, -200.0),
            phase: egui::TouchPhase::Move,
            modifiers: Modifiers::NONE,
        });
        harness.run_steps(30);
        let scrolled = top(&mut harness);
        assert!(scrolled < start - 100.0, "{size:?}: {start} → {scrolled}");

        click(&mut harness, "Deep Trouble");
        // The details scroll on their own.
        harness.get_by_label("Time trial");
        click(&mut harness, "All rooms");
        assert_eq!(top(&mut harness), scrolled, "{size:?}");
    }
}

#[test]
fn opening_a_file_closes_the_page() {
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "toadvillage")));
    // Not a save: the document and its page stay.
    harness.state_mut().open_path(repo_file("Cargo.toml"));
    harness.run_steps(2);
    assert!(matches!(harness.state().dialog, Some(Dialog::Error { .. })));
    assert_eq!(selected(&harness), Some(level("toadvillage")));
    click(&mut harness, "OK");
    harness
        .state_mut()
        .open_path(repo_file("tests/saves/game/0018.sav"));
    harness.run_steps(2);
    assert_eq!(selected(&harness), None);
    harness.get_by_label("Toad Village");
}

#[test]
fn the_browser_back_closes_the_page_unless_a_modal_is_open() {
    let mut harness = harness(app(selecting("tests/saves/game/0019.sav", "toadvillage")));
    let ctx = harness.ctx.clone();
    harness.state_mut().error("Test", "message");
    harness.run_steps(2);
    assert!(!harness.state_mut().back_requested(&ctx));
    harness.run_steps(2);
    assert_eq!(selected(&harness), Some(level("toadvillage")));
    click(&mut harness, "OK");
    assert!(harness.state_mut().back_requested(&ctx));
    harness.run_steps(2);
    assert_eq!(selected(&harness), None);
    // With no page open, there is nothing to close.
    assert!(!harness.state_mut().back_requested(&ctx));
}

// ------------------------------------------------ padlocks and story scenes

/// Clicks a button that opens a dialog. The dialog appears the frame after
/// the click, and its first frame only measures it: one more step. It keeps
/// its place afterwards, whatever its content does.
fn open_dialog(harness: &mut Harness<'_, App>, label: &str) {
    click(harness, label);
    harness.run_steps(1);
}

#[test]
fn unlocking_room_4_matches_the_golden_output() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    // Rooms 1–3 are open on 0019: only rooms 4 and 5 offer an unlock.
    harness.get_by_label("Warp Room 3: open");
    assert!(harness.query_by_label("Warp Room 3: locked").is_none());
    open_dialog(&mut harness, "Warp Room 4: locked");
    harness.get_by_label("Unlock up to Warp Room 4");
    for option in [
        "Also grant the abilities of the defeated bosses",
        "Skip their story scenes",
    ] {
        let checkbox = harness.get_by_label(option);
        assert_eq!(
            checkbox.accesskit_node().toggled(),
            Some(egui::accesskit::Toggled::True)
        );
    }
    harness.get_by_label("Each defeated boss counts 1% towards the percentage.");
    harness.get_by_label("Percentage: 19% → 20%");
    // Shortcuts wait while the dialog is open.
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert!(history(&harness).is_empty());

    click(&mut harness, "Apply");
    assert!(!harness.state().crash3.has_dialog());
    assert_eq!(
        history(&harness),
        ["Unlock warp room 4 (+abilities, skip scenes)"]
    );
    let bytes = doc(&harness).to_bytes().unwrap();
    assert_eq!(
        sha256_hex(&bytes),
        golden_sha256("ops/unlock_warp_room_4_all_0019")
    );
    // Room 4 is open now; its padlock no longer opens anything.
    harness.get_by_label("Warp Room 4: open");
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert!(history(&harness).is_empty());
    harness.get_by_label("Warp Room 4: locked");
}

#[test]
fn the_preview_runs_only_when_the_options_change() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    open_dialog(&mut harness, "Warp Room 5: locked");
    harness.run_steps(5);
    assert_eq!(harness.state().crash3.preview_runs(), Some(1));
    click(&mut harness, "Skip their story scenes");
    harness.run_steps(5);
    assert_eq!(harness.state().crash3.preview_runs(), Some(2));
    click(&mut harness, "Apply");
    assert_eq!(history(&harness), ["Unlock warp room 5 (+abilities)"]);
    let save = doc(&harness).current();
    assert!(save.boss_defeated(level("ngin")));
    assert!(!save.cutscene_seen("C3_NGin_Intro"));
    assert!(save.ability(nsane::crash3::Ability::Bazooka));

    // Cancel applies nothing.
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    open_dialog(&mut harness, "Warp Room 4: locked");
    click(&mut harness, "Cancel");
    assert!(!harness.state().crash3.has_dialog());
    assert!(history(&harness).is_empty());
}

/// The story scenes dialog, opened from the overview: its Edit… button is
/// under the fold at 800 points.
fn scenes_harness(rel: &str) -> Harness<'static, App> {
    let mut harness = tall_harness(opening(repo_file(rel)));
    open_dialog(&mut harness, "Edit…");
    assert!(harness.state().crash3.has_dialog());
    harness
}

#[test]
fn skip_all_story_scenes_leaves_the_18_scene_keys() {
    use nsane::crash3::CUTSCENES;
    use nsane::crash3::tables::{cutscene_key, cutscene_name};
    let mut harness = scenes_harness("tests/saves/game/0019.sav");
    click(&mut harness, "Skip all story scenes");
    assert_eq!(history(&harness), ["All story scenes: seen"]);
    let scenes: Vec<String> = doc(&harness)
        .current()
        .flag_entries()
        .into_keys()
        .filter(|k| cutscene_name(k).is_some())
        .collect();
    let mut expected: Vec<String> = CUTSCENES
        .iter()
        .flat_map(|s| s.scenes)
        .map(|s| cutscene_key(s))
        .collect();
    expected.sort();
    assert_eq!(expected.len(), 18);
    assert_eq!(scenes, expected);
    // Nothing left to skip; the dialog stays open, and the overview counts
    // them all.
    assert!(
        harness
            .get_by_label("Skip all story scenes")
            .accesskit_node()
            .is_disabled()
    );
    assert!(harness.state().crash3.has_dialog());
    harness.get_by_label("Scenes: 18 / 18 seen");
    click(&mut harness, "Close");
    assert!(!harness.state().crash3.has_dialog());
}

#[test]
fn opening_the_secret_room_up_to_29() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    harness.get_by_label("1 relic · next level at 5");
    open_dialog(&mut harness, "29 Future Frenzy: locked");
    harness.get_by_label("Open the secret warp room up to level 29");
    harness.get_by_label("Relics: 1 → 20");
    click(&mut harness, "Apply");
    assert_eq!(
        history(&harness),
        ["Reach 20 relics: sapphire on 19 levels"]
    );
    assert!(doc(&harness).current().totals().relics() >= 20);
    // The card shows 26–29 open and 30 locked.
    for name in [
        "26 Ski Crazed",
        "27 Hang'em High",
        "28 Area 51?",
        "29 Future Frenzy",
    ] {
        harness.get_by_label(&format!("{name}: open"));
    }
    harness.get_by_label("30 Rings of Power: locked");
    harness.get_by_label("20 relics · next level at 25");
    // An open level's padlock opens nothing.
    click(&mut harness, "29 Future Frenzy: open");
    assert!(!harness.state().crash3.has_dialog());
}

#[test]
fn super_powers_and_scenes_are_one_step_each() {
    use nsane::crash3::{ABILITIES, Ability};
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    let on = |h: &Harness<'_, App>| {
        ABILITIES
            .iter()
            .filter(|&&a| doc(h).current().ability(a))
            .count()
    };
    let before = on(&harness);
    harness.get_by_label(&format!("Super powers: {before} / 5"));
    click(&mut harness, "Super power: Speed Shoes");
    assert!(doc(&harness).current().ability(Ability::SpeedShoes));
    click(&mut harness, "Super power: Super Charged Body Slam");
    assert!(!doc(&harness).current().ability(Ability::BodySlam));
    assert_eq!(on(&harness), before);
    harness.get_by_label(&format!("Super powers: {before} / 5"));

    open_dialog(&mut harness, "Edit…");
    click(&mut harness, "Dr. N. Tropy: mark all seen");
    click(&mut harness, "NGin Intro: seen");
    let save = doc(&harness).current();
    for scene in [
        "C3_NTropy_Hub3_Intro",
        "C3_NTropy_PreFight",
        "C3_NTropy_Taunt",
        "C3_Cortex_Post_NTropy_Fight",
        "C3_NGin_Intro",
    ] {
        assert!(save.cutscene_seen(scene), "{scene}");
    }
    click(&mut harness, "Dr. N. Tropy: mark all unseen");
    assert!(!doc(&harness).current().cutscene_seen("C3_NTropy_Taunt"));
    assert_eq!(
        history(&harness),
        [
            "Speed Shoes: on",
            "Super Charged Body Slam: off",
            "Dr. N. Tropy scenes: seen",
            "Scene C3_NGin_Intro: seen",
            "Dr. N. Tropy scenes: not seen",
        ]
    );
}

#[test]
fn an_overview_boss_selects_it() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0019.sav")));
    harness.get_by_label("Bosses: 2 / 5");
    click(&mut harness, "Overview: N. Gin");
    assert_eq!(harness.state().selected, Some(level("ngin")));
    let node = harness.get_by_label("Overview: N. Gin");
    assert_eq!(
        node.accesskit_node().toggled(),
        Some(egui::accesskit::Toggled::True)
    );
    // The details show the boss, and selecting is not an edit.
    harness.get_by_label("Ability: Fruit Bazooka");
    assert!(history(&harness).is_empty());
}

/// Every story-level tool has a place: the padlocks of the locked rooms and
/// secret levels, the story scenes dialog, the grid's boss rows and the
/// Presets menu.
#[test]
fn every_story_tool_is_reachable() {
    use nsane::crash3::{ABILITIES, BOSSES, CUTSCENES, SECRET_ROOM};
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/initial_game.sav")));
    // A new game: rooms 2–5 and every secret level are locked.
    for room in 2..=5 {
        open_dialog(&mut harness, &format!("Warp Room {room}: locked"));
        harness.get_by_label(&format!("Unlock up to Warp Room {room}"));
        click(&mut harness, "Cancel");
        assert!(!harness.state().crash3.has_dialog());
    }
    for entry in SECRET_ROOM {
        let name = format!("{} {}: locked", entry.number, entry.level.name());
        open_dialog(&mut harness, &name);
        harness.get_by_label(&format!(
            "Open the secret warp room up to level {}",
            entry.number
        ));
        click(&mut harness, "Cancel");
    }
    for (boss, ability) in BOSSES.into_iter().zip(ABILITIES) {
        harness.get_by_label(&format!("{}: defeated", boss.name()));
        harness.get_by_label(&format!("{}: {}", boss.name(), ability.name()));
    }
    assert!(history(&harness).is_empty());

    open_dialog(&mut harness, "Edit…");
    for step in &CUTSCENES {
        let step_name = step.boss().map_or("Start", |b| b.name());
        harness.get_by_label(&format!("{step_name}: mark all seen"));
        harness.get_by_label(&format!("{step_name}: mark all unseen"));
    }
    // The Start scenes, which no boss's details show.
    click(&mut harness, "Start: mark all seen");
    for scene in ["Cortex Taunt 1", "AkuAku TimeTrialInfo", "AkuAku Warning"] {
        let label = format!("{scene}: seen");
        let node = harness.get_by_label(&label);
        assert_eq!(
            node.accesskit_node().toggled(),
            Some(egui::accesskit::Toggled::True)
        );
    }
    assert_eq!(history(&harness), ["Start scenes: seen"]);
    click(&mut harness, "Close");

    click(&mut harness, "Presets");
    harness.get_by_label("All super powers on or off…");
    harness.get_by_label("Skip all story scenes…");
}

#[test]
fn the_unlock_and_scenes_views() {
    let options = |view| Options {
        view: Some(view),
        ..opening_options("tests/saves/game/0019.sav")
    };
    let harness = harness(app(options(View::UnlockDialog)));
    assert!(harness.state().crash3.has_dialog());
    harness.get_by_label("Unlock up to Warp Room 4");
    let harness = self::harness(app(options(View::Cutscenes)));
    assert!(harness.state().crash3.has_dialog());
    harness.get_by_label("Story scenes");
    harness.get_by_label("Skip all story scenes");
}

// ------------------------------------------------- presets, diagnostics, review

/// Picks an entry of the Presets menu.
fn open_preset(harness: &mut Harness<'_, App>, entry: &str) {
    click(harness, "Presets");
    open_dialog(harness, entry);
}

fn serious_diagnostics(harness: &Harness<'_, App>) -> Vec<&'static str> {
    doc(harness)
        .diagnostics()
        .iter()
        .filter(|d| d.severity > nsane::crash3::Severity::Info)
        .map(|d| d.code)
        .collect()
}

#[test]
fn complete_everything_from_the_presets_menu() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    open_preset(&mut harness, "Complete everything (108%)…");
    assert!(harness.state().crash3.has_dialog());
    harness.get_by_label("Percentage: 19% → 108%");
    harness.get_by_label("Crystals: 10 → 25");
    harness.get_by_label("Everything the game counts is collected.");
    // Shortcuts wait while the dialog is open.
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(harness.state().crash3.preset_preview_runs(), Some(1));
    click(&mut harness, "Apply");
    assert!(!harness.state().crash3.has_dialog());
    assert_eq!(
        history(&harness),
        ["Complete everything (108%, matching times, skip scenes)"]
    );
    assert_eq!(doc(&harness).current().totals().percent(), 108);
    assert!(serious_diagnostics(&harness).is_empty());
    // One step.
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(doc(&harness).current().totals().percent(), 19);
}

#[test]
fn story_up_to_warp_room_3_with_gems() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/initial_game.sav")));
    open_preset(&mut harness, "Story up to warp room…");
    // A new game starts at room 1; pick room 3.
    harness.get_by_label("Story up to warp room 1");
    harness
        .query_all_by_label("3")
        .find(|n| n.accesskit_node().role() == Role::RadioButton)
        .expect("the room 3 option")
        .click();
    harness.run_steps(2);
    harness.get_by_label("Story up to warp room 3");
    click(&mut harness, "Also their gems (clear, second and colored)");
    harness.run_steps(2);
    assert_eq!(harness.state().crash3.preset_preview_runs(), Some(3));
    click(&mut harness, "Apply");
    assert_eq!(
        history(&harness),
        ["Story up to warp room 3 (+gems, +abilities, skip scenes)"]
    );
    let save = doc(&harness).current();
    assert_eq!(save.totals().crystals, 15);
    assert_eq!(save.totals().bosses, 3);
    assert!(save.boss_defeated(level("ntropy")));
    assert!(serious_diagnostics(&harness).is_empty());
}

/// The fixed time field of a preset dialog, by its text.
fn fixed_field<'a>(harness: &'a Harness<'_, App>, text: &'a str) -> egui_kittest::Node<'a> {
    harness
        .query_all_by_value(text)
        .find(|n| n.accesskit_node().role() == Role::TextInput)
        .expect("the fixed time field")
}

/// A click at `pos`, where there may be no widget.
fn click_at(harness: &mut Harness<'_, App>, pos: egui::Pos2) {
    harness.hover_at(pos);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }
    harness.run_steps(2);
}

/// A click beside a dialog keeps it and its options; Enter doesn't apply
/// a preset; Escape cancels. Enter closes the dialogs that only inform.
#[test]
fn dialogs_close_on_escape_but_not_on_a_click_beside_them() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    open_preset(&mut harness, "All relics at a tier…");
    click(&mut harness, "Platinum");
    click_at(&mut harness, egui::pos2(20.0, 1300.0));
    assert!(
        harness.state().crash3.has_dialog(),
        "the backdrop does nothing"
    );
    harness.get_by_label("All relics at platinum");
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert!(harness.state().crash3.has_dialog());
    assert!(history(&harness).is_empty());
    press(&mut harness, Modifiers::NONE, Key::Escape);
    assert!(!harness.state().crash3.has_dialog());
    assert!(history(&harness).is_empty());

    click(&mut harness, "⚙");
    open_dialog(&mut harness, "About…");
    click_at(&mut harness, egui::pos2(20.0, 1300.0));
    assert_eq!(harness.state().dialog, Some(Dialog::About));
    press(&mut harness, Modifiers::NONE, Key::Enter);
    assert_eq!(harness.state().dialog, None);
}

#[test]
fn a_fixed_time_that_does_not_parse_disables_apply() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    open_preset(&mut harness, "All relics at a tier…");
    click(&mut harness, "Write this time on every level:");
    harness.run_steps(2);
    assert!(!harness.get_by_label("Apply").accesskit_node().is_disabled());
    fixed_field(&harness, "1:00.00").focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    fixed_field(&harness, "1:00.00").type_text("1:7x");
    harness.run_steps(3);
    assert!(harness.get_by_label("Apply").accesskit_node().is_disabled());
    // A valid time enables it again, and the label carries it.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    fixed_field(&harness, "1:7x").type_text("0:59.00");
    harness.run_steps(3);
    click(&mut harness, "Apply");
    assert_eq!(
        history(&harness),
        ["All relics at gold (only upgrades, times 0:59.00)"]
    );
}

#[test]
fn the_portal_glitch_fix_and_undo() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/edited/t4_crystals.sav")));
    let glitch = |harness: &Harness<'_, App>| {
        doc(harness)
            .diagnostics()
            .iter()
            .any(|d| d.code == "portal_glitch")
    };
    assert!(glitch(&harness));
    // The toolbar's badge and the overview's both open the panel.
    assert_eq!(harness.query_all_by_label("1 warning").count(), 2);
    harness
        .query_all_by_label("1 warning")
        .next()
        .unwrap()
        .click();
    harness.run_steps(2);
    assert!(harness.state().crash3.diagnostics_open());
    click(&mut harness, "Fix: Defeat N. Gin");
    assert!(!glitch(&harness));
    assert_eq!(history(&harness), ["Defeat N. Gin"]);
    // The panel stays open, and the undo shortcut works beside it.
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert!(glitch(&harness));
    assert!(harness.state().crash3.diagnostics_open());
}

#[test]
fn show_selects_the_level_of_a_diagnostic() {
    let mut harness = tall_harness(app(Options {
        view: Some(View::Diagnostics),
        ..opening_options("tests/saves/edited/t1_relics.sav")
    }));
    let level = doc(&harness)
        .diagnostics()
        .iter()
        .find_map(|d| d.level)
        .expect("a diagnostic about a level");
    click(&mut harness, &format!("Show {}", level.name()));
    assert_eq!(harness.state().selected, Some(level));
    assert!(
        harness
            .get_by_label(&format!("Show {}", level.name()))
            .accesskit_node()
            .is_disabled(),
        "already shown"
    );
}

/// Lives 41 → 42, Deep Trouble's crystal and Speed Shoes on 0019, then
/// Save: the review lists the three rows and the recomputed totals, and
/// confirming writes the file with a backup.
#[test]
fn the_save_review_of_a_session_on_0019() {
    use nsane::crash3::Ability;
    let path = scratch_copy("tests/saves/game/0019.sav", "review");
    let dir = path.parent().unwrap().to_owned();
    let mut harness = tall_harness(opening(path.clone()));
    type_lives(&mut harness, "42");
    click(&mut harness, "Deep Trouble: crystal");
    let edit = Edit::SetAbility {
        ability: Ability::SpeedShoes,
        on: true,
    };
    harness.state_mut().run(
        &Context::default(),
        vec![Action::Apply {
            label: "Speed Shoes: on".into(),
            edits: vec![edit],
        }],
    );
    harness.run_steps(2);

    press(&mut harness, Modifiers::COMMAND, Key::S);
    let Some(Dialog::Review { review, then, .. }) = &harness.state().dialog else {
        panic!("the review is shown");
    };
    assert_eq!(*review, crate::games::crash3::SaveReview::of(doc(&harness)));
    assert_eq!(*then, None);
    assert_eq!(review.rows.len(), 3);
    harness.get_by_label("Deep Trouble: + crystal; entered, completions 0 → 1");
    harness.get_by_label("Speed Shoes: off → on");
    harness.get_by_label("Lives: 41 → 42");
    harness.get_by_label("Percentage: 19% → 20%");
    harness.get_by_label("Crystals: 10 → 11");
    harness.get_by_label("No warnings.");
    harness.get_by_label_contains("The file it replaces is kept as ");
    assert!(backups(&dir).is_empty(), "nothing written yet");

    // Cancel writes nothing.
    click(&mut harness, "Cancel");
    assert!(harness.state().dialog.is_none());
    assert!(doc(&harness).is_dirty());
    assert!(backups(&dir).is_empty());

    press(&mut harness, Modifiers::COMMAND, Key::S);
    confirm_review(&mut harness);
    assert!(!doc(&harness).is_dirty());
    assert_eq!(fs::read(&path).unwrap(), doc(&harness).to_bytes().unwrap());
    assert_eq!(backups(&dir).len(), 1);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_review_can_be_turned_off_and_on() {
    let path = scratch_copy("tests/saves/game/0018.sav", "no-review");
    let dir = path.parent().unwrap().to_owned();
    let mut harness = harness(opening(path.clone()));
    // Nothing to review: Save writes at once.
    press(&mut harness, Modifiers::COMMAND, Key::S);
    assert!(harness.state().dialog.is_none());
    assert_eq!(backups(&dir).len(), 1);

    type_lives(&mut harness, "42");
    press(&mut harness, Modifiers::COMMAND, Key::S);
    click(&mut harness, "Don't show this again");
    assert!(!harness.state().prefs.skip_save_review, "only once saved");
    confirm_review(&mut harness);
    assert!(harness.state().prefs.skip_save_review);

    type_lives(&mut harness, "43");
    press(&mut harness, Modifiers::COMMAND, Key::S);
    assert!(harness.state().dialog.is_none(), "saved without the review");
    assert!(!doc(&harness).is_dirty());

    // The settings menu turns it back on.
    click(&mut harness, "⚙");
    click(&mut harness, "Review the changes before saving");
    assert!(!harness.state().prefs.skip_save_review);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_unsaved_question_saves_through_the_review_then_opens() {
    let path = scratch_copy("tests/saves/game/0018.sav", "unsaved-review");
    let other = repo_file("tests/saves/game/0019.sav");
    let mut harness = harness(opening(path.clone()));
    type_lives(&mut harness, "42");
    harness
        .state_mut()
        .run(&Context::default(), vec![Action::OpenPath(other.clone())]);
    harness.run_steps(2);
    // The question's Save, after the toolbar's.
    harness.query_all_by_label("Save").last().unwrap().click();
    harness.run_steps(2);
    let Some(Dialog::Review { then, .. }) = &harness.state().dialog else {
        panic!("the review is shown");
    };
    assert_eq!(*then, Some(Then::OpenPath(other)));
    confirm_review(&mut harness);
    assert_eq!(fs::read(&path).unwrap(), lives42());
    assert_eq!(doc(&harness).file_name(), Some("0019.sav"));
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn an_error_diagnostic_disables_save() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    let edits: Vec<_> = (0..40)
        .map(|i| Edit::RawFlagSet {
            key: format!("{i}_{}", "x".repeat(4000)),
        })
        .collect();
    harness.state_mut().apply("big".into(), edits);
    harness.run_steps(2);
    let reason = harness.state().save_blocked().expect("blocked");
    assert!(reason.contains("a Switch save holds 153,600"), "{reason}");
    for label in ["Save", "Save as"] {
        assert!(harness.get_by_label(label).accesskit_node().is_disabled());
    }
    // In the toolbar and in the overview.
    assert_eq!(harness.query_all_by_label("1 error").count(), 2);
    // The shortcut explains instead of writing.
    press(&mut harness, Modifiers::COMMAND, Key::S);
    let Some(Dialog::Error { title, message }) = &harness.state().dialog else {
        panic!("an error dialog");
    };
    assert_eq!(title, "Can't save");
    assert_eq!(*message, reason);
    // Undo makes it fit again.
    click(&mut harness, "OK");
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert!(harness.state().save_blocked().is_none());
    assert!(!harness.get_by_label("Save").accesskit_node().is_disabled());
}

#[test]
fn the_history_goes_back_and_forward() {
    let mut harness = harness(opening(repo_file("tests/saves/game/0018.sav")));
    assert!(
        harness
            .get_by_label("History")
            .accesskit_node()
            .is_disabled()
    );
    type_lives(&mut harness, "42");
    type_lives(&mut harness, "43");
    type_lives(&mut harness, "44");
    click(&mut harness, "History");
    click(&mut harness, "Opened 0018.sav");
    assert_eq!(lives(&harness), 5);
    assert!(history(&harness).is_empty());
    click(&mut harness, "History");
    click(&mut harness, "Lives: 43");
    assert_eq!(lives(&harness), 43);
    assert_eq!(history(&harness), ["Lives: 42", "Lives: 43"]);
    click(&mut harness, "History");
    click(&mut harness, "Lives: 42");
    assert_eq!(lives(&harness), 42);
}

#[test]
fn the_preset_diagnostics_and_review_views() {
    let options = |rel: &str, view| Options {
        view: Some(view),
        ..opening_options(rel)
    };
    let harness = harness(app(options("tests/saves/game/0019.sav", View::PresetAll)));
    harness.get_by_label("Complete everything (108%)");
    let harness = self::harness(app(options(
        "tests/saves/game/0019.sav",
        View::PresetStory3,
    )));
    harness.get_by_label("Story up to warp room 3");
    let harness = self::harness(app(options(
        "tests/saves/edited/t4_crystals.sav",
        View::Diagnostics,
    )));
    assert!(harness.state().crash3.diagnostics_open());
    let harness = self::harness(app(options("tests/saves/game/0019.sav", View::SaveReview)));
    assert!(matches!(
        harness.state().dialog,
        Some(Dialog::Review { .. })
    ));
}

#[test]
fn edits_from_a_json_file_are_applied_at_start() {
    let dir = temp_dir("apply");
    let edits = dir.join("edits.json");
    fs::write(
        &edits,
        r#"[{"op": "set_lives", "count": 42},
            {"op": "set_crystal", "level": "deeptrouble", "on": true}]"#,
    )
    .unwrap();
    let harness = harness(app(Options {
        apply: Some(edits.clone()),
        ..opening_options("tests/saves/game/0019.sav")
    }));
    assert_eq!(history(&harness), ["Lives: 42", "Deep Trouble: crystal on"]);

    fs::write(&edits, r#"[{"op": "set_lives"}]"#).unwrap();
    let harness = self::harness(app(Options {
        apply: Some(edits),
        ..opening_options("tests/saves/game/0019.sav")
    }));
    assert!(matches!(harness.state().dialog, Some(Dialog::Error { .. })));
    assert!(history(&harness).is_empty());
    fs::remove_dir_all(dir).unwrap();
}

// ------------------------------------------------------------ conversion

fn convert<'a>(harness: &'a Harness<'_, App>) -> &'a Convert {
    harness
        .state()
        .convert
        .as_ref()
        .expect("the dialog is open")
}

fn disabled(harness: &Harness<'_, App>, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().is_disabled()
}

/// Clicks a button of the conversion dialog that shows a file dialog,
/// which answers `path`. Returns where that dialog started.
fn click_with_dialog(harness: &mut Harness<'_, App>, label: &str, path: PathBuf) -> PathBuf {
    harness.state_mut().dialog_answers.push_back(Some(path));
    // It may open the dialog that says where the file went.
    open_dialog(harness, label);
    assert!(
        harness.state().dialog_answers.is_empty(),
        "the dialog was shown for {label}"
    );
    harness
        .state_mut()
        .dialog_starts
        .pop()
        .flatten()
        .expect("a start path")
}

#[test]
fn converting_0019_to_pc_and_back() {
    let template = scratch_copy("tests/saves/game/ALCHEMYXSAVE_8.sav", "convert");
    let dir = template.parent().unwrap().to_owned();
    let source = nsane::crash3::Crash3Save::parse(
        &fs::read(repo_file("tests/saves/game/0019.sav")).unwrap(),
    )
    .unwrap();
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    assert!(harness.query_by_label("Convert to Switch…").is_none());
    open_dialog(&mut harness, "Convert to PC…");
    assert_eq!(convert(&harness).target, nsane::Platform::Pc);
    assert!(
        harness
            .query_by_label_contains("Not yet tested in game")
            .is_none()
    );
    harness.get_by_label("0019.sav · Switch · 19% · 41 lives");
    assert!(disabled(&harness, "Convert…"), "no template yet");
    // Shortcuts wait while the dialog is open.
    press(&mut harness, Modifiers::COMMAND, Key::O);
    assert!(harness.state().dialog.is_none());

    click_with_dialog(&mut harness, "Pick file…", template.clone());
    harness.get_by_label("ALCHEMYXSAVE_8.sav");
    harness.get_by_label("No game data lost: the converted save reads back the same.");
    harness.get_by_label("Suggested file name: ALCHEMYXSAVE_8.sav");
    assert!(!disabled(&harness, "Convert…"));

    // Written over the template, as the dialog suggests: it is backed up.
    let start = click_with_dialog(&mut harness, "Convert…", template.clone());
    assert_eq!(start, template);
    assert!(convert(&harness).is_written());
    assert_eq!(backups(&dir).len(), 1);
    assert_eq!(
        fs::read(&backups(&dir)[0]).unwrap(),
        fs::read(repo_file("tests/saves/game/ALCHEMYXSAVE_8.sav")).unwrap()
    );
    let pc_bytes = fs::read(&template).unwrap();
    let pc = nsane::crash3::Crash3Save::parse(&pc_bytes).unwrap();
    assert_eq!(nsane::Platform::of(&pc), nsane::Platform::Pc);
    assert_eq!(pc_bytes, pc.igb().build_unpadded().unwrap(), "unpadded");
    assert_eq!(pc.totals(), source.totals());
    assert_eq!(pc.lives(), source.lives());

    harness.get_by_label("Converted to PC");
    click(&mut harness, "Open the converted save");
    assert!(harness.state().convert.is_none());
    assert_eq!(doc(&harness).platform(), nsane::Platform::Pc);
    assert_eq!(doc(&harness).file_name(), Some("ALCHEMYXSAVE_8.sav"));

    // Back to Switch with the built-in template: the suggested name is the
    // Switch file of the source's slot.
    open_dialog(&mut harness, "Convert to Switch…");
    click(&mut harness, "Built-in new-game template");
    harness.get_by_label("No game data lost: the converted save reads back the same.");
    let back = dir.join("back.sav");
    let start = click_with_dialog(&mut harness, "Convert…", back.clone());
    assert_eq!(start, dir.join("MACK_8.sav"));
    harness.get_by_label("Converted to Switch");
    harness.get_by_label_contains(&format!("Saved {}", back.display()));
    assert_eq!(backups(&dir).len(), 1, "a new file has nothing to back up");
    let switch_bytes = fs::read(&back).unwrap();
    assert_eq!(switch_bytes.len(), 153_600);
    let switch = nsane::crash3::Crash3Save::parse(&switch_bytes).unwrap();
    assert!(nsane::diff::diff(&source, &switch).is_empty());
    click(&mut harness, "Close");
    assert!(harness.state().convert.is_none());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_wrong_template_is_refused_inline() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    open_dialog(&mut harness, "Convert to PC…");
    click_with_dialog(
        &mut harness,
        "Pick file…",
        repo_file("tests/saves/game/0018.sav"),
    );
    harness.get_by_label(
        "0018.sav is a Switch save, like the one being converted. Pick a save from the PC \
         version.",
    );
    assert!(disabled(&harness, "Convert…"));

    click_with_dialog(&mut harness, "Pick file…", repo_file("Cargo.toml"));
    harness.get_by_label_contains("Not an N. Sane Trilogy save");
    assert!(disabled(&harness, "Convert…"));

    // The built-in template works; going back to the picked file doesn't.
    click(&mut harness, "Built-in new-game template");
    assert!(
        harness
            .query_by_label_contains("Not an N. Sane Trilogy save")
            .is_none()
    );
    assert!(!disabled(&harness, "Convert…"));
    click(&mut harness, "My save from the PC version (recommended)");
    harness.get_by_label_contains("Not an N. Sane Trilogy save");
    assert!(disabled(&harness, "Convert…"));

    // A picked file that can't be read.
    click_with_dialog(
        &mut harness,
        "Pick file…",
        repo_file("tests/saves/game/missing.sav"),
    );
    harness.get_by_label_contains("Can't read missing.sav");
    assert!(disabled(&harness, "Convert…"));
    click(&mut harness, "Cancel");
    assert!(harness.state().convert.is_none());
}

#[test]
fn unsaved_edits_are_converted() {
    let mut harness = tall_harness(opening(repo_file("tests/saves/game/0019.sav")));
    type_lives(&mut harness, "42");
    open_dialog(&mut harness, "Convert to PC…");
    click(&mut harness, "Built-in new-game template");
    harness.get_by_label(
        "Includes the unsaved changes (1 change). The open save itself is not written.",
    );
    let doc = doc(&harness).clone();
    let state = harness.state_mut().convert.as_mut().unwrap();
    let crate::games::crash3::transfer_ui::Outcome::Converted(converted) = state.outcome(&doc)
    else {
        panic!("converted");
    };
    assert_eq!((converted.before_lives, converted.lives), (42, 42));
    assert!(converted.changes.is_empty());
    // One conversion, however many frames.
    harness.run_steps(5);
    assert_eq!(convert(&harness).runs, 1);
    // Ctrl+Z waits for the dialog.
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(lives(&harness), 42);
}

#[test]
fn the_convert_view_picks_the_template() {
    let harness = tall_harness(app(Options {
        view: Some(View::Convert),
        template: Some(repo_file("tests/saves/game/ALCHEMYXSAVE_8.sav")),
        ..opening_options("tests/saves/game/0019.sav")
    }));
    assert_eq!(
        convert(&harness).choice,
        crate::games::crash3::transfer_ui::Choice::Own
    );
    harness.get_by_label("No game data lost: the converted save reads back the same.");
}

#[test]
fn the_about_dialog_names_the_version_and_the_license() {
    let mut harness = harness(app(Options::default()));
    click(&mut harness, "⚙");
    click(&mut harness, "About…");
    assert_eq!(harness.state().dialog, Some(Dialog::About));
    harness.get_by_label(&format!("Version {VERSION} · MIT license"));
    harness.get_by_label(DISCLAIMER);
    harness.get_by_label(PRIVACY);
    // Shortcuts wait while it is open.
    press(&mut harness, Modifiers::COMMAND, Key::O);
    assert_eq!(harness.state().dialog, Some(Dialog::About));
    harness.key_press(Key::Escape);
    harness.run_steps(2);
    assert_eq!(harness.state().dialog, None);

    let harness = harness_with_view(View::About);
    assert_eq!(harness.state().dialog, Some(Dialog::About));
}

fn harness_with_view(view: View) -> Harness<'static, App> {
    harness(app(Options {
        view: Some(view),
        ..Options::default()
    }))
}

#[test]
fn leftover_bytes_as_the_last_level_show_as_unknown() {
    let leftover = harness(opening(repo_file("tests/saves/after_edit/l2_after.sav")));
    leftover.get_by_label("Last level played: unknown");
    let playthrough = harness(opening(repo_file("tests/saves/game/0019.sav")));
    playthrough.get_by_label("Last level played: Toad Village");
}

// ------------------------------------------------------------ layouts

/// The toolbar at its widest: a Switch save with unsaved changes, an error
/// (the save outgrew its file), a warning (the portal glitch) and notes
/// (more than 99 lives), and the debug menu of test builds.
fn widest_toolbar(width: f32) -> Harness<'static, App> {
    let mut harness = themed(
        Harness::builder().with_size(Vec2::new(width, 800.0)),
        opening(repo_file("tests/saves/edited/t4_crystals.sav")),
    );
    let edits: Vec<_> = (0..40)
        .map(|i| Edit::RawFlagSet {
            key: format!("{i}_{}", "x".repeat(4000)),
        })
        .collect();
    harness.state_mut().apply("big".into(), edits);
    harness
        .state_mut()
        .apply("lives".into(), vec![Edit::SetLives { count: 150 }]);
    harness.run_steps(3);
    harness
}

/// The toolbar's items: Open, the last of its left part (the notes'
/// badge), and the right part (Debug, then the theme toggle).
fn toolbar_rects(harness: &Harness<'_, App>) -> [egui::Rect; 4] {
    let theme = if harness.state().dark { "☀" } else { "🌙" };
    let badge = harness
        .query_all_by_label_contains(" notes")
        .find(|n| n.accesskit_node().role() == Role::Button)
        .expect("the notes' badge")
        .rect();
    [
        harness.get_by_label("Open").rect(),
        badge,
        harness.get_by_label("Debug").rect(),
        harness.get_by_label(theme).rect(),
    ]
}

#[test]
fn the_full_toolbar_fits_on_one_line_at_its_widest() {
    for dark in [false, true] {
        let mut harness = widest_toolbar(layout::WIDE_MIN);
        harness.state_mut().dark = dark;
        harness.run_steps(3);
        assert_eq!(layout::current(&harness.ctx), Class::Wide);
        for name in ["1 error", "1 warning", "2 notes"] {
            assert_eq!(harness.query_all_by_label(name).count(), 2, "{name}");
        }
        let [open, badge, debug, theme] = toolbar_rects(&harness);
        // Nothing wrapped, and the right part doesn't cover the left one.
        assert_eq!(open.top(), theme.top(), "dark: {dark}");
        assert!(badge.top() < open.bottom(), "dark: {dark}: {badge:?}");
        assert!(
            debug.left() >= badge.right(),
            "dark: {dark}: {badge:?} {debug:?}"
        );
    }
}

fn sized(size: Vec2, app: App) -> Harness<'static, App> {
    let mut harness = themed(Harness::builder().with_size(size), app);
    harness.run_steps(2);
    harness
}

const MEDIUM: Vec2 = Vec2::new(800.0, 1000.0);
const PHONE: Vec2 = Vec2::new(390.0, 844.0);

#[test]
fn a_medium_window_has_the_compact_bar_beside_the_overview() {
    let mut harness = sized(MEDIUM, opening(repo_file("tests/saves/game/0019.sav")));
    assert_eq!(layout::current(&harness.ctx), Class::Medium);
    harness.get_by_label("Menu");
    assert!(harness.query_by_label("Save as").is_none(), "in the menu");
    harness.get_by_label("64 / 150 KB");
    // The overview and the grid, side by side.
    let lives = harness.get_by_role(Role::SpinButton).rect();
    let grid = harness.get_by_label("Toad Village").rect();
    assert!(lives.right() < grid.left(), "{lives:?} {grid:?}");
    click(&mut harness, "Menu");
    harness.get_by_label("Save as");
    harness.get_by_label("Complete everything (108%)…");
}

#[test]
fn a_phone_shows_one_page_at_a_time() {
    let mut harness = sized(PHONE, opening(repo_file("tests/saves/game/0019.sav")));
    assert_eq!(layout::current(&harness.ctx), Class::Phone);
    // The rooms first: no overview, no game tabs.
    harness.get_by_label("Toad Village");
    assert!(harness.query_by_role(Role::SpinButton).is_none());
    assert!(harness.query_by_label("Crash 3: Warped").is_none());
    click(&mut harness, "Show the overview");
    harness.get_by_role(Role::SpinButton);
    harness.get_by_label("Game");
    assert!(harness.query_by_label("Toad Village").is_none());
    click(&mut harness, "Show the rooms");
    harness.get_by_label("Toad Village");

    click(&mut harness, "Toad Village");
    assert_eq!(selected(&harness), Some(level("toadvillage")));
    harness.get_by_label("Time trial");
    click(&mut harness, "All rooms");
    assert_eq!(selected(&harness), None);
}

#[test]
fn a_boss_on_the_phone_overview_shows_its_page() {
    let mut harness = sized(PHONE, opening(repo_file("tests/saves/game/0019.sav")));
    for _ in 0..2 {
        click(&mut harness, "Show the overview");
        click(&mut harness, "Overview: N. Gin");
        assert_eq!(harness.state().page, Page::Rooms);
        assert_eq!(selected(&harness), Some(level("ngin")));
        harness.get_by_label("Ability: Fruit Bazooka");
    }
}

#[test]
fn the_phone_menu_holds_the_toolbar() {
    let mut harness = sized(PHONE, opening(repo_file("tests/saves/game/0018.sav")));
    type_lives_on_the_overview(&mut harness, "42");
    // The change count is a dot, named for screen readers.
    harness.get_by_label("1 change");
    click(&mut harness, "Menu");
    for entry in ["Open", "Save", "Save as", "Convert to PC…", "About…"] {
        harness.get_by_label(entry);
    }
    open_dialog(&mut harness, "History…");
    assert_eq!(harness.state().dialog, Some(Dialog::History));
    // Shortcuts wait for it, like for any dialog.
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(lives(&harness), 42);
    click(&mut harness, "Opened 0018.sav");
    assert_eq!(lives(&harness), 5);
    assert_eq!(harness.state().dialog, None);
}

/// Types into the lives field of a phone's overview page, then goes back to
/// the rooms.
fn type_lives_on_the_overview(harness: &mut Harness<'_, App>, text: &str) {
    click(harness, "Show the overview");
    type_lives(harness, text);
    click(harness, "Show the rooms");
}

/// An on-screen keyboard can shorten the page while a field is typed in:
/// the layout waits until the field is left.
#[test]
fn the_class_is_kept_while_a_field_has_focus() {
    let mut harness = sized(PHONE, opening(repo_file("tests/saves/game/0018.sav")));
    click(&mut harness, "Show the overview");
    start_typing_lives(&mut harness, "42");
    harness.set_size(Vec2::new(1280.0, 800.0));
    harness.run_steps(2);
    assert_eq!(layout::current(&harness.ctx), Class::Phone);
    harness.key_press(Key::Enter);
    harness.run_steps(3);
    assert_eq!(layout::current(&harness.ctx), Class::Wide);
    assert_eq!(lives(&harness), 42);
}

#[test]
fn dialogs_are_sheets_on_a_phone() {
    let harness = sized(
        PHONE,
        app(Options {
            view: Some(View::About),
            ..Options::default()
        }),
    );
    let rect = harness
        .ctx
        .memory(|m| m.area_rect(dialog::area_id("about")))
        .expect("the dialog was shown");
    assert!(rect.width() >= PHONE.x - 24.0, "{rect:?}");
    assert!(rect.top() <= 16.0, "{rect:?}");
}

/// On a narrow phone, the review's checkbox wraps beside the buttons
/// instead of running under them. The web's "Download" is wider than
/// "Save file": 320 points here leave less room than 360 do there.
#[test]
fn the_review_sheet_footer_fits_a_narrow_phone() {
    let mut harness = sized(
        Vec2::new(320.0, 640.0),
        opening(repo_file("tests/saves/game/0018.sav")),
    );
    type_lives_on_the_overview(&mut harness, "42");
    click(&mut harness, "Menu");
    click(&mut harness, "Save");
    harness.run_steps(2);
    let check = harness.get_by_label("Don't show this again").rect();
    let cancel = harness.get_by_label("Cancel").rect();
    assert!(check.right() <= cancel.left(), "{check:?} {cancel:?}");
    assert!(check.left() >= 0.0, "{check:?}");
}

#[test]
fn the_phone_diagnostics_sheet_shows_a_level() {
    let mut harness = sized(
        PHONE,
        opening(repo_file("tests/saves/edited/t4_crystals.sav")),
    );
    let badge = harness
        .query_all_by_label("1 warning")
        .next()
        .expect("the bottom bar's badge");
    assert!(badge.rect().top() > PHONE.y - 60.0, "{:?}", badge.rect());
    badge.click();
    harness.run_steps(2);
    assert!(harness.state().crash3.diagnostics_open());
    click(&mut harness, "Show N. Gin");
    assert!(!harness.state().crash3.diagnostics_open());
    assert_eq!(selected(&harness), Some(level("ngin")));
    harness.get_by_label("Ability: Fruit Bazooka");
}

/// How wide a row's name is on screen: its text in the button's font,
/// italic for the secret room's entrance rows, and the button's padding.
fn name_width(ctx: &Context, text: &str, italics: bool) -> f32 {
    use crate::games::crash3::level_row::NAME_PADDING;
    let format = egui::TextFormat {
        font_id: ctx.global_style().text_styles[&egui::TextStyle::Button].clone(),
        italics,
        ..Default::default()
    };
    let job = egui::text::LayoutJob::single_section(text.to_owned(), format);
    let galley = ctx.fonts_mut(|f| f.layout_job(job));
    galley.size().x + 2.0 * NAME_PADDING
}

/// The widest name of the rows with toggles.
fn widest_name(ctx: &Context) -> (f32, String) {
    LevelId::all()
        .map(|l| (name_width(ctx, l.name(), false), l.name().to_owned()))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap()
}

/// The names of the secret room's entrance rows, which have no toggles.
fn entrance_names() -> Vec<String> {
    nsane::crash3::SECRET_ROOM
        .iter()
        .filter(|e| e.level.level().warp_room != 6)
        .map(|e| format!("{} (alt.)", e.level.name()))
        .collect()
}

/// The space a name has in the row of `level`, from where the name starts
/// to where its row's crystal does.
fn name_space(harness: &Harness<'_, App>, level: &str) -> f32 {
    use crate::games::crash3::level_row::Metrics;
    let gap = Metrics::new(layout::current(&harness.ctx), 0.0).gap;
    let name = harness
        .query_all_by_label(level)
        .find(|n| n.accesskit_node().role() == Role::Button)
        .expect("the row's name")
        .rect();
    let crystal = harness.get_by_label(&format!("{level}: crystal")).rect();
    crystal.left() - gap - name.left()
}

/// The widths of the entrance rows' names.
fn entrance_widths(harness: &Harness<'_, App>) -> Vec<f32> {
    entrance_names()
        .iter()
        .map(|name| harness.get_by_label(name).rect().width())
        .collect()
}

/// No level name is cut at the reference sizes. A cut name keeps its whole
/// accessible name, so the names are measured instead.
#[test]
fn no_level_name_is_cut() {
    use crate::games::crash3::level_row::{Metrics, TIME_MIN_ROW};
    use crate::games::crash3::rooms::{CARD_INSET, MAX_CARD_WIDTH, MIN_CARD_WIDTH};
    let mut harness = sized(PHONE, opening(repo_file("tests/saves/game/0019.sav")));
    let (widest, name) = widest_name(&harness.ctx);
    // The entrance rows have room to spare on a large window: their names
    // are whole there.
    harness.set_size(Vec2::new(1920.0, 1080.0));
    harness.run_steps(3);
    let whole = entrance_widths(&harness);
    let sizes = [
        Vec2::new(320.0, 640.0),
        Vec2::new(360.0, 740.0),
        PHONE,
        Vec2::new(844.0, 390.0),
        Vec2::new(layout::WIDE_MIN, 800.0),
        Vec2::new(1280.0, 800.0),
    ];
    let mut space = 0.0;
    for size in sizes {
        harness.state_mut().touch_screen = size.y < layout::LANDSCAPE_PHONE_HEIGHT;
        harness.set_size(size);
        harness.run_steps(3);
        space = name_space(&harness, "Toad Village");
        assert!(
            widest <= space,
            "{size:?}: {name} is {widest} wide, rows give {space}"
        );
        assert_eq!(
            entrance_widths(&harness),
            whole,
            "{size:?}: an entrance row"
        );
    }
    // At 1280 the cards are as wide as they get: the computation agrees.
    let rows = MAX_CARD_WIDTH - CARD_INSET;
    assert_eq!(Metrics::new(Class::Wide, rows).name_width(rows), space);
    // The narrowest card of the grid, before it drops a column.
    let rows = MIN_CARD_WIDTH - CARD_INSET;
    let space = Metrics::new(Class::Wide, rows).name_width(rows);
    assert!(widest <= space, "the narrowest card: {space}");
    // The narrowest phone rows that keep the best time.
    let space = Metrics::new(Class::Phone, TIME_MIN_ROW).name_width(TIME_MIN_ROW);
    assert!(widest <= space, "the narrowest rows with a time: {space}");
    // The room's card beside a level's page.
    harness.state_mut().selected = Some(level("underpressure"));
    harness.run_steps(3);
    let space = name_space(&harness, "Toad Village");
    assert!(widest <= space, "the page's card: {space}");
}

/// A phone's rows are large enough to tap, and keep the best time down to
/// 360 points wide; a narrower one leaves it to the level's page.
#[test]
fn phone_rows_can_be_tapped() {
    let mut harness = sized(PHONE, opening(repo_file("tests/saves/game/0019.sav")));
    for label in [
        "Toad Village: crystal",
        "Toad Village: relic",
        "Tiny Tiger: defeated",
        "Warp Room 4: locked",
    ] {
        let rect = harness.get_by_label(label).rect();
        assert!(
            rect.width() >= 32.0 && rect.height() >= 32.0,
            "{label}: {rect:?}"
        );
    }
    harness.get_by_label("0:55.44");
    harness.set_size(Vec2::new(360.0, 740.0));
    harness.run_steps(3);
    harness.get_by_label("0:55.44");
    harness.set_size(Vec2::new(320.0, 640.0));
    harness.run_steps(3);
    assert!(
        harness.query_by_label("0:55.44").is_none(),
        "no best time at 320"
    );
    harness.state_mut().touch_screen = true;
    harness.set_size(Vec2::new(844.0, 390.0));
    harness.run_steps(3);
    assert_eq!(layout::current(&harness.ctx), Class::Phone);
    harness.get_by_label("0:55.44");
}

/// On a narrow card the secret room's next level goes under the relic
/// count, instead of being dropped.
#[test]
fn the_next_secret_level_wraps_on_a_narrow_card() {
    use nsane::crash3::Relic;
    let mut harness = sized(
        Vec2::new(360.0, 740.0),
        opening(repo_file("tests/saves/game/0019.sav")),
    );
    let edits = [
        "underpressure",
        "orientexpress",
        "boneyard",
        "makinwaves",
        "geewiz",
        "hangemhigh",
        "hogride",
        "tombtime",
        "midnightrun",
        "dinomight",
    ]
    .map(|key| Edit::SetRelic {
        level: level(key),
        relic: Some(Relic::Sapphire),
    });
    harness.state_mut().apply("relics".into(), edits.to_vec());
    harness.run_steps(3);
    harness.get_by_label("11 relics");
    harness.get_by_label("next level at 15");
    harness.set_size(Vec2::new(1280.0, 800.0));
    harness.run_steps(3);
    harness.get_by_label("11 relics · next level at 15");
}
