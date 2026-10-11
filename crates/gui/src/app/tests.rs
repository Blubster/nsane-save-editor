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
    harness.get_by_label("Toad Village");
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
