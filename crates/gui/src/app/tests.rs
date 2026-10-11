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
use crate::test_util::{repo_file, temp_dir};

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

fn harness(app: App) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 800.0))
        .build_ui_state(|ui, app: &mut App| app.show(ui), app);
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
    let harness = harness(opening(repo_file("saves/0019.sav")));
    assert!(harness.state().dialog.is_none());
    assert_eq!(doc(&harness).platform(), nsane::Platform::Switch);
    harness.get_by_label("Switch");
    harness.get_by_label_contains("/ 150 KB");
    harness.get_by_label("0019.sav");
    assert_eq!(harness.state().title, format!("0019.sav — {APP_TITLE}"));
}

#[test]
fn opens_a_pc_save_without_budget() {
    let harness = harness(opening(repo_file("saves/ALCHEMYXSAVE_8.sav")));
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
    let harness = harness(opening(repo_file("saves/no_such_file.sav")));
    assert!(matches!(harness.state().dialog, Some(Dialog::Error { .. })));
}

#[test]
fn a_truncated_save_does_not_panic_and_keeps_the_open_one() {
    let bytes = fs::read(repo_file("saves/0019.sav")).unwrap();
    let mut harness = harness(opening(repo_file("saves/0018.sav")));
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
    let path = scratch_copy("saves/0019.sav", "noop");
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
    let path = scratch_copy("saves/0018.sav", "lives42");
    let mut harness = harness(opening(path.clone()));
    assert_eq!(lives(&harness), 5);

    type_lives(&mut harness, "42");
    assert_eq!(lives(&harness), 42);
    assert_eq!(doc(&harness).history().collect::<Vec<_>>(), ["Lives: 42"]);
    assert!(harness.state().title.starts_with("• 0018.sav"));
    harness.get_by_label("• 1 change");

    press(&mut harness, Modifiers::COMMAND, Key::S);
    let tested = fs::read(repo_file("test_saves/lives42.sav")).unwrap();
    assert!(
        fs::read(&path).unwrap() == tested,
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
    let mut harness = harness(opening(repo_file("saves/0018.sav")));
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
    let mut harness = harness(opening(repo_file("saves/0018.sav")));
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
    let mut harness = harness(opening(repo_file("saves/0018.sav")));
    type_lives(&mut harness, "999");
    assert_eq!(lives(&harness), 255);
}

#[test]
fn save_shortcut_while_typing_includes_the_typed_value() {
    let path = scratch_copy("saves/0018.sav", "typing");
    let mut harness = harness(opening(path.clone()));
    start_typing_lives(&mut harness, "42");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(3);
    assert_eq!(
        fs::read(&path).unwrap(),
        fs::read(repo_file("test_saves/lives42.sav")).unwrap()
    );
    assert!(!doc(&harness).is_dirty());
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn out_of_range_lives_are_shown_as_they_are() {
    // as another tool could write it
    let mut igb = IgbFile::parse(&fs::read(repo_file("saves/0018.sav")).unwrap()).unwrap();
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
    let mut harness = harness(opening(repo_file("saves/0018.sav")));
    type_lives(&mut harness, "42");
    press(&mut harness, Modifiers::COMMAND, Key::O);
    assert_eq!(harness.state().dialog, Some(Dialog::Unsaved(Then::Open)));
    harness.get_by_label_contains("1 change since it was opened");
    harness.get_by_label("Cancel").click();
    harness.run_steps(2);
    assert!(harness.state().dialog.is_none());
    assert_eq!(lives(&harness), 42);

    // A dropped file asks too; Discard opens it.
    let other = repo_file("saves/0019.sav");
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

    let mut harness = harness(opening(repo_file("saves/0018.sav")));
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
    let mut harness = harness(opening(repo_file("saves/0018.sav")));
    type_lives(&mut harness, "42");
    harness.state_mut().error("Test", "message");
    press(&mut harness, Modifiers::COMMAND, Key::Z);
    assert_eq!(lives(&harness), 42);
}

#[test]
fn a_switch_save_that_outgrows_its_file_is_not_written() {
    let path = scratch_copy("saves/0018.sav", "too-big");
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
        open: Some(repo_file("saves/0019.sav")),
        view: Some(View::Unsaved),
        select: LevelId::from_key("toadvillage"),
        ..Options::default()
    }));
    harness.get_by_label("Unsaved changes");
    assert_eq!(harness.state().selected, LevelId::from_key("toadvillage"));
    // In the warp room grid and in the details.
    assert_eq!(harness.query_all_by_label("Toad Village").count(), 2);
}

#[test]
fn crash_1_and_2_are_disabled() {
    let harness = harness(opening(repo_file("saves/0019.sav")));
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
fn narrow_windows_collapse_the_side_panels() {
    let mut harness = harness(opening(repo_file("saves/0019.sav")));
    harness.get_by_role(Role::SpinButton);
    harness.set_size(Vec2::new(600.0, 800.0));
    harness.run_steps(2);
    assert!(harness.query_by_role(Role::SpinButton).is_none());
    harness.get_by_label("Overview").click();
    harness.run_steps(2);
    harness.get_by_role(Role::SpinButton);
}

#[test]
fn a_dropped_file_asks_before_replacing_unsaved_changes() {
    let bytes = fs::read(repo_file("saves/0019.sav")).unwrap();
    let mut harness = harness(opening(repo_file("saves/0018.sav")));
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
fn theme_and_panels_survive_a_restart() {
    let mut storage = MemoryStorage::default();
    let mut first = harness(app_with_storage(
        &storage,
        opening_options("saves/0019.sav"),
    ));
    let dark = first.state().dark;
    first.set_size(Vec2::new(600.0, 800.0));
    first.run_steps(2);
    first.get_by_label(if dark { "☀" } else { "🌙" }).click();
    first.get_by_label("Overview").click();
    first.run_steps(2);
    assert_eq!(first.state().dark, !dark);
    eframe::App::save(first.state_mut(), &mut storage);
    drop(first);

    let second = app_with_storage(&storage, opening_options("saves/0019.sav"));
    assert_eq!(second.dark, !dark);
    assert!(second.prefs.show_overview && !second.prefs.show_details);
    let mut second = harness(second);
    second.set_size(Vec2::new(600.0, 800.0));
    second.run_steps(2);
    second.get_by_role(Role::SpinButton); // the overview shows, unasked

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
    let mut harness = harness(opening(repo_file("saves/0019.sav")));
    let dark = harness.state().dark;
    harness.get_by_label(if dark { "☀" } else { "🌙" }).click();
    harness.run_steps(2);
    let mut storage = MemoryStorage::default();
    eframe::App::save(harness.state_mut(), &mut storage);
    assert!(storage.0.is_empty());
}

#[test]
fn the_last_folder_is_remembered() {
    let path = scratch_copy("saves/0018.sav", "last-dir");
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
    let mut harness = Harness::builder()
        .with_size(Vec2::new(1280.0, 1400.0))
        .build_ui_state(|ui, app: &mut App| app.show(ui), app);
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
    let mut harness = harness(opening(repo_file("saves/0019.sav")));
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
    let mut harness = harness(opening(repo_file("saves/0019.sav")));
    for (label, expected) in [
        ("Deep Trouble: clear gem", "Deep Trouble: clear gem on"),
        ("Deep Trouble: red gem", "Deep Trouble: red gem on"),
        ("Bone Yard: second gem", "Bone Yard: second gem on"),
        ("Toad Village: crystal", "Toad Village: crystal off"),
        ("Dr. N. Tropy: defeated", "Dr. N. Tropy: defeated"),
        ("Death Tornado Spin", "Death Tornado Spin: on"),
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
    let mut harness = harness(opening(repo_file("saves/0019.sav")));
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
    let mut harness = harness(opening(repo_file("saves/0019.sav")));
    assert_eq!(harness.state().selected, None);
    click(&mut harness, "Deep Trouble");
    assert_eq!(harness.state().selected, Some(level("deeptrouble")));
    assert!(history(&harness).is_empty(), "selecting is not an edit");
}

#[test]
fn the_secret_room_locks_follow_the_relic_count() {
    let mut harness = tall_harness(opening(repo_file("saves/0019.sav")));
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

    let harness = tall_harness(opening(repo_file("test_saves/t6_after.sav")));
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
    let mut harness = tall_harness(opening(repo_file("saves/0019.sav")));
    click(&mut harness, "Hang'em High (alt.)");
    assert_eq!(harness.state().selected, Some(level("hangemhigh")));
    click(&mut harness, "Future Frenzy (alt.)");
    assert_eq!(harness.state().selected, Some(level("futurefrenzy")));
    assert!(history(&harness).is_empty());
}

#[test]
fn the_portal_glitch_shows_on_its_room() {
    let harness = harness(opening(repo_file("test_saves/t4_crystals.sav")));
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
    let harness = harness(opening(repo_file("test_saves/t7_bonus_gem.sav")));
    assert!(doc(&harness).current().bonus_gem_due());
    harness.get_by_label("due");
    let harness = self::harness(opening(repo_file("saves/0019.sav")));
    assert!(harness.query_by_label("due").is_none());
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
    let mut harness = harness(app(selecting("saves/0010.sav", "toadvillage")));
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
    let expected = Crash3Save::parse(&fs::read(repo_file("saves/0011.sav")).unwrap()).unwrap();
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
    let mut harness = harness(app(selecting("saves/0019.sav", "toadvillage")));
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
    let mut harness = harness(app(selecting("saves/0019.sav", "toadvillage")));
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
    let mut harness = harness(app(selecting("test_saves/t6_after.sav", "deeptrouble")));
    let deep = level("deeptrouble");
    let before = doc(&harness).current().totals();
    assert!(doc(&harness).current().entered(deep));
    click(&mut harness, "Reset level…");
    harness.get_by_label(
        "Reset Deep Trouble to never played? Removes its crystal, gems, relic and times.",
    );
    harness.get_by_label("Not yet tested in game.");
    click(&mut harness, "Cancel");
    assert!(history(&harness).is_empty());

    click(&mut harness, "Reset level…");
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
    let mut harness = harness(app(selecting("saves/0019.sav", "ngin")));
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
        ..selecting("saves/0019.sav", "toadvillage")
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
