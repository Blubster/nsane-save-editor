//! The window: toolbar, game tabs, the three panels, dialogs and toasts.
//!
//! Widgets never change the document: they push [`Action`]s, which run once
//! the frame is drawn, and every edit goes through [`Document::apply`].

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use egui::{
    Align, Align2, Button, Color32, Context, Frame, Id, Key, KeyboardShortcut, Layout, Margin,
    Modal, Modifiers, RichText, Sense, Ui, Vec2, ViewportCommand,
};
use nsane::SaveError;
use nsane::crash3::{Edit, LevelId};
use nsane::{Document, OpenError};

use crate::gallery::Gallery;
use crate::games::crash3::{
    Convert, ConvertRequest, Crash3Ui, Preset, SaveReview, Written, other_platform, save_blocker,
};
use crate::icons::{self, Icon, IconState};
use crate::options::{Options, View};
use crate::prefs::Prefs;
use crate::theme;

pub const APP_TITLE: &str = "Save editor for Crash Bandicoot N. Sane Trilogy";
/// Names the settings folder (and the window, for the desktop).
#[cfg_attr(target_arch = "wasm32", allow(dead_code))] // the web keeps settings per site
pub const APP_ID: &str = "nsane-save-editor";

/// Below this window width the side panels collapse into toggles.
const NARROW_WIDTH: f32 = 900.0;
/// The size budget turns amber above this share of a Switch file.
const BUDGET_WARNING_PERCENT: usize = 80;
/// How long a toast stays, in seconds.
const TOAST_SECONDS: f64 = 6.0;
const ERROR_TOAST_SECONDS: f64 = 12.0;

const OPEN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::O);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const SAVE_AS: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::S);
const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
const REDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);
const REDO_SHIFT: KeyboardShortcut =
    KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);

#[cfg(not(target_arch = "wasm32"))]
const SAVE_TIP: &str = "Save, keeping a backup of the file replaced";
#[cfg(not(target_arch = "wasm32"))]
const SAVE_AS_TIP: &str = "Save to another file";
#[cfg(target_arch = "wasm32")]
const SAVE_TIP: &str = "Download the edited save, under the same name";
#[cfg(target_arch = "wasm32")]
const SAVE_AS_TIP: &str = "Download the edited save under another name";

/// A request from the interface, carried out after the frame is drawn.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Open,
    /// A file named on the command line or dropped onto the window.
    #[cfg(not(target_arch = "wasm32"))]
    OpenPath(PathBuf),
    /// A file dropped onto the page, already read.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // the desktop opens by path
    OpenBytes {
        name: String,
        bytes: Vec<u8>,
    },
    Save,
    SaveAs,
    /// Opens the dialog that converts the save to the other platform.
    Convert,
    Undo,
    Redo,
    /// Edits applied together as one undo step.
    Apply {
        label: String,
        edits: Vec<Edit>,
    },
}

/// What to do once unsaved changes are saved or discarded.
#[derive(Debug, Clone, PartialEq)]
enum Then {
    Open,
    #[cfg(not(target_arch = "wasm32"))]
    OpenPath(PathBuf),
    OpenBytes {
        name: String,
        bytes: Vec<u8>,
    },
    Close,
}

/// Where a save goes once its review is confirmed.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    /// A file, and the time its backup is named after.
    #[cfg(not(target_arch = "wasm32"))]
    File {
        path: PathBuf,
        now: (i32, u8, u8, u8, u8, u8),
    },
    /// A browser download under this name.
    #[cfg(target_arch = "wasm32")]
    Download(String),
}

impl Target {
    #[cfg(not(target_arch = "wasm32"))]
    fn file(path: PathBuf) -> Self {
        Target::File {
            path,
            now: crate::io::native::local_now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Dialog {
    /// Save, discard or cancel, before `Then`.
    Unsaved(Then),
    /// The changes, before writing them to `target`. `then` runs once
    /// written.
    Review {
        review: SaveReview,
        target: Target,
        then: Option<Then>,
        dont_show: bool,
    },
    Error {
        title: String,
        message: String,
    },
    /// The name to download the save under.
    #[cfg(target_arch = "wasm32")]
    DownloadAs(String),
}

/// The game tabs. A save holds all three games.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Game {
    Crash1,
    Crash2,
    Crash3,
}

impl Game {
    const ALL: [Game; 3] = [Game::Crash1, Game::Crash2, Game::Crash3];

    fn title(self) -> &'static str {
        match self {
            Game::Crash1 => "Crash Bandicoot",
            Game::Crash2 => "Crash 2: Cortex Strikes Back",
            Game::Crash3 => "Crash 3: Warped",
        }
    }

    fn available(self) -> bool {
        self == Game::Crash3
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToastKind {
    Info,
    Error,
}

#[derive(Debug, Clone)]
struct Toast {
    kind: ToastKind,
    text: String,
    /// Set when the toast is first shown.
    until: Option<f64>,
}

/// The open save.
struct OpenFile {
    doc: Document,
    /// Where Save writes; `None` until the document has a file on disk.
    #[cfg(not(target_arch = "wasm32"))]
    path: Option<PathBuf>,
    /// `changes().len()` and the generation it was counted for: the diff
    /// walks the whole save, so it isn't redone every frame.
    change_count: Option<(u64, usize)>,
}

impl OpenFile {
    fn name(&self) -> &str {
        self.doc.file_name().unwrap_or("Untitled")
    }

    fn change_count(&mut self) -> usize {
        let generation = self.doc.generation();
        match self.change_count {
            Some((g, n)) if g == generation => n,
            _ => {
                let n = self.doc.changes().len();
                self.change_count = Some((generation, n));
                n
            }
        }
    }
}

pub struct App {
    dark: bool,
    file: Option<OpenFile>,
    game: Game,
    selected: Option<LevelId>,
    crash3: Crash3Ui,
    dialog: Option<Dialog>,
    /// The conversion to the other platform, while its dialog is open.
    convert: Option<Convert>,
    toasts: Vec<Toast>,
    prefs: Prefs,
    /// The settings as last written to storage; `None` when they are not
    /// kept (screenshots, tests).
    stored_prefs: Option<Prefs>,
    /// Shortcuts that wait one frame so that a field being typed in commits
    /// its value first.
    deferred: Vec<Action>,
    /// The window title last set.
    title: String,
    /// The user confirmed closing, or nothing was unsaved.
    closing: bool,
    gallery: Gallery,
    /// The gallery fills the window instead of the editor.
    gallery_only: bool,
    /// The gallery window opened from the debug menu.
    gallery_open: bool,
    #[cfg(not(target_arch = "wasm32"))]
    screenshot: Option<crate::screenshot::Screenshot>,
    #[cfg(target_arch = "wasm32")]
    inbox: crate::io::web::Inbox,
    #[cfg(target_arch = "wasm32")]
    leave_guard: crate::io::web::LeaveGuard,
    /// What the file dialogs of the conversion return in tests, in turn.
    #[cfg(test)]
    dialog_answers: std::collections::VecDeque<Option<PathBuf>>,
    /// The paths those dialogs suggested or started in.
    #[cfg(test)]
    dialog_starts: Vec<Option<PathBuf>>,
}

impl App {
    /// The app, with the settings kept from the last run. Screenshot runs
    /// neither read nor write them, so that they look the same everywhere.
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let keep = options.screenshots.is_empty();
        #[cfg(target_arch = "wasm32")]
        let keep = true;
        let stored = keep.then(|| Prefs::load(cc.storage));
        Self::with_prefs(&cc.egui_ctx, options, stored)
    }

    /// The app with default settings, not kept.
    #[cfg(test)]
    pub fn with_context(ctx: &Context, options: Options) -> Self {
        Self::with_prefs(ctx, options, None)
    }

    fn with_prefs(ctx: &Context, options: Options, stored: Option<Prefs>) -> Self {
        let prefs = stored.clone().unwrap_or_default();
        #[cfg(not(target_arch = "wasm32"))]
        let first_shot_theme = options.screenshots.first().and_then(|s| s.dark);
        #[cfg(target_arch = "wasm32")]
        let first_shot_theme = None;
        let dark = first_shot_theme
            .or(options.dark)
            .or(prefs.dark)
            .unwrap_or(ctx.theme() == egui::Theme::Dark);
        theme::fonts(ctx);
        theme::apply(ctx, dark);
        let mut app = Self {
            dark,
            file: None,
            game: Game::Crash3,
            selected: options.select,
            crash3: Crash3Ui::default(),
            dialog: None,
            convert: None,
            toasts: Vec::new(),
            prefs,
            stored_prefs: stored,
            deferred: Vec::new(),
            title: APP_TITLE.to_owned(),
            closing: false,
            gallery: Gallery::default(),
            gallery_only: options.icon_gallery,
            gallery_open: false,
            #[cfg(not(target_arch = "wasm32"))]
            screenshot: crate::screenshot::Screenshot::new(options.screenshots),
            #[cfg(target_arch = "wasm32")]
            inbox: Default::default(),
            #[cfg(target_arch = "wasm32")]
            leave_guard: Default::default(),
            #[cfg(test)]
            dialog_answers: Default::default(),
            #[cfg(test)]
            dialog_starts: Vec::new(),
        };
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = options.open {
            app.open_path(path);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &options.apply {
            app.apply_file(path);
        }
        match options.view {
            Some(View::Unsaved) => app.dialog = Some(Dialog::Unsaved(Then::Open)),
            Some(View::InvalidTime) => {
                if let (Some(file), Some(level)) = (&app.file, app.selected) {
                    app.crash3.refused_time(&file.doc, level, 2, "1:7x");
                }
            }
            Some(View::Progress) => app.crash3.show_progress(),
            Some(View::UnlockDialog) => {
                if let Some(file) = &app.file {
                    app.crash3.unlock_dialog(&file.doc);
                }
            }
            Some(View::Cutscenes) => app.crash3.scroll_to_scenes(),
            Some(View::PresetAll | View::PresetStory3) => {
                if let Some(file) = &app.file {
                    let preset = match options.view {
                        Some(View::PresetAll) => Preset::menu(file.doc.current())[0],
                        _ => Preset::StoryUpTo {
                            room: 3,
                            options: Default::default(),
                        },
                    };
                    app.crash3.open_preset(preset);
                }
            }
            Some(View::Diagnostics) => app.crash3.open_diagnostics(),
            Some(View::Convert) => {
                app.open_convert();
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(path) = options.template {
                    app.template_path(path);
                }
            }
            Some(View::SaveReview) => {
                if let Some(file) = &app.file {
                    #[cfg(not(target_arch = "wasm32"))]
                    let target = Target::file(
                        file.path
                            .clone()
                            .unwrap_or_else(|| PathBuf::from(file.name())),
                    );
                    #[cfg(target_arch = "wasm32")]
                    let target = Target::Download(file.name().to_owned());
                    app.dialog = Some(app.review_dialog(target, None));
                }
            }
            None => {}
        }
        app
    }

    /// Applies the edit ops of a JSON file, one undo step each.
    #[cfg(not(target_arch = "wasm32"))]
    fn apply_file(&mut self, path: &Path) {
        let edits = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| {
                serde_json::from_str::<Vec<serde_json::Value>>(&text).map_err(|e| e.to_string())
            })
            .and_then(|ops| {
                ops.iter()
                    .map(|op| Edit::from_json(op).map_err(|e| e.to_string()))
                    .collect::<Result<Vec<_>, _>>()
            });
        match edits {
            Ok(edits) => {
                for edit in edits {
                    if let Some(file) = &self.file {
                        let label = edit.describe(file.doc.current());
                        self.apply(label, vec![edit]);
                    }
                }
            }
            Err(e) => self.error(format!("Can't apply {}", path.display()), e),
        }
    }

    fn is_dirty(&self) -> bool {
        self.file.as_ref().is_some_and(|f| f.doc.is_dirty())
    }

    fn toast(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.toasts.push(Toast {
            kind,
            text: text.into(),
            until: None,
        });
    }

    fn error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.dialog = Some(Dialog::Error {
            title: title.into(),
            message: message.into(),
        });
    }

    /// Replaces the document with `bytes`, or explains why it can't. The open
    /// document stays when the file is not a save. Returns whether it opened.
    fn open_bytes(
        &mut self,
        bytes: &[u8],
        name: String,
        #[cfg(not(target_arch = "wasm32"))] path: Option<PathBuf>,
    ) -> bool {
        match Document::open(bytes, Some(name.clone())) {
            Ok(doc) => {
                self.file = Some(OpenFile {
                    doc,
                    #[cfg(not(target_arch = "wasm32"))]
                    path,
                    change_count: None,
                });
                self.crash3.reset();
                self.convert = None;
                true
            }
            Err(e) => {
                self.error(format!("Can't open {name}"), open_error_message(&e));
                false
            }
        }
    }

    /// Opens a file the user has picked or dropped, without a place on disk.
    fn open_unsaved_bytes(&mut self, bytes: &[u8], name: String) {
        #[cfg(not(target_arch = "wasm32"))]
        self.open_bytes(bytes, name, None);
        #[cfg(target_arch = "wasm32")]
        self.open_bytes(bytes, name);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_path(&mut self, path: PathBuf) {
        let name = display_name(&path);
        match crate::io::native::read(&path) {
            Ok(bytes) => {
                if self.open_bytes(&bytes, name, Some(path.clone())) {
                    self.remember_folder(&path);
                }
            }
            Err(e) => self.error(format!("Can't open {name}"), e.to_string()),
        }
    }

    /// Dialogs start in the folder of the last file opened or saved.
    #[cfg(not(target_arch = "wasm32"))]
    fn remember_folder(&mut self, path: &Path) {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            let dir = std::path::absolute(dir).unwrap_or_else(|_| dir.to_owned());
            self.prefs.last_dir = Some(dir);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn open_dialog(&mut self) {
        let start = self
            .file
            .as_ref()
            .and_then(|f| f.path.as_deref())
            .and_then(Path::parent)
            .or(self.prefs.last_dir.as_deref());
        match crate::io::native::pick_open(start) {
            Ok(Some(path)) => self.open_path(path),
            Ok(None) => {}
            Err(_) => self.dialog_unavailable(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn dialog_unavailable(&mut self) {
        self.toast(
            ToastKind::Error,
            "No file dialog is available on this system. Drop a save file onto \
             the window instead, or start the editor with --open FILE.sav.",
        );
    }

    /// Why the document can't be written now: the first error diagnostic.
    fn save_blocked(&self) -> Option<String> {
        let file = self.file.as_ref()?;
        save_blocker(&file.doc).map(|d| d.message.clone())
    }

    /// Saves to the document's file, or asks where when it has none. In the
    /// browser, downloads it under its name. Unsaved changes are reviewed
    /// first; `then` runs once the file is written.
    fn save(&mut self, ctx: &Context, then: Option<Then>) {
        let Some(file) = &self.file else {
            return;
        };
        if let Some(reason) = self.save_blocked() {
            self.error("Can't save", reason);
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        match file.path.clone() {
            Some(path) => self.review_or_write(ctx, Target::file(path), then),
            None => self.save_as(ctx, then),
        }
        #[cfg(target_arch = "wasm32")]
        {
            let name = file.name().to_owned();
            self.review_or_write(ctx, Target::Download(name), then);
        }
    }

    /// Asks where to save, then saves. In the browser, asks for the name to
    /// download under.
    fn save_as(&mut self, ctx: &Context, then: Option<Then>) {
        let Some(file) = &self.file else {
            return;
        };
        if let Some(reason) = self.save_blocked() {
            self.error("Can't save", reason);
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let suggested = match (&file.path, &self.prefs.last_dir) {
                (Some(path), _) => path.clone(),
                (None, Some(dir)) => dir.join(file.name()),
                (None, None) => PathBuf::from(file.name()),
            };
            match crate::io::native::pick_save(Some(&suggested)) {
                Ok(Some(path)) => self.review_or_write(ctx, Target::file(path), then),
                Ok(None) => {}
                Err(_) => self.dialog_unavailable(),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            // The browser always has a name to save under: see `save`.
            let _ = (ctx, then);
            self.dialog = Some(Dialog::DownloadAs(file.name().to_owned()));
        }
    }

    fn review_dialog(&self, target: Target, then: Option<Then>) -> Dialog {
        let review = self
            .file
            .as_ref()
            .map(|f| SaveReview::of(&f.doc))
            .unwrap_or(SaveReview {
                rows: Vec::new(),
                recomputed: Vec::new(),
                diagnostics: Vec::new(),
            });
        Dialog::Review {
            review,
            target,
            then,
            dont_show: false,
        }
    }

    /// Shows the review of unsaved changes, or writes right away when there
    /// are none or the review is turned off.
    fn review_or_write(&mut self, ctx: &Context, target: Target, then: Option<Then>) {
        if self.is_dirty() && !self.prefs.skip_save_review {
            self.dialog = Some(self.review_dialog(target, then));
        } else {
            self.write(ctx, target, then);
        }
    }

    fn write(&mut self, ctx: &Context, target: Target, then: Option<Then>) {
        let written = match target {
            #[cfg(not(target_arch = "wasm32"))]
            Target::File { path, now } => self.save_to_at(path, now),
            #[cfg(target_arch = "wasm32")]
            Target::Download(name) => self.download(name),
        };
        if written && let Some(then) = then {
            self.proceed(ctx, then);
        }
    }

    /// The bytes to write, or an error dialog saying why there are none.
    ///
    /// Every save, to a file or as a download, goes through here: a review
    /// of the changes, shown before writing, belongs before this call.
    fn bytes_to_write(&mut self) -> Option<Vec<u8>> {
        let file = self.file.as_mut()?;
        match file.doc.to_bytes() {
            Ok(bytes) => Some(bytes),
            Err(SaveError::DoesNotFit { needed, capacity }) => {
                self.error(
                    "The save doesn't fit",
                    format!(
                        "With these changes the save data takes {needed} bytes, but a \
                         Switch save file holds {capacity}. Nothing was written. Undo \
                         some changes, then save again."
                    ),
                );
                None
            }
            Err(SaveError::Save(e)) => {
                self.error("Can't save", e.to_string());
                None
            }
        }
    }

    /// Writes the document to `path`, backing up the file it replaces.
    #[cfg(test)]
    fn save_to(&mut self, path: PathBuf) -> bool {
        self.save_to_at(path, crate::io::native::local_now())
    }

    /// Writes the document to `path`, backing up the file it replaces under
    /// a name made from `now`.
    #[cfg(not(target_arch = "wasm32"))]
    fn save_to_at(&mut self, path: PathBuf, now: (i32, u8, u8, u8, u8, u8)) -> bool {
        let Some(bytes) = self.bytes_to_write() else {
            return false;
        };
        match crate::io::native::write_with_backup(&path, &bytes, now) {
            Ok(backup) => {
                let mut text = format!("Saved {}", path.display());
                if let Some(backup) = backup {
                    text += &format!("\nBackup of the previous file: {}", backup.display());
                }
                self.remember_folder(&path);
                if let Some(file) = &mut self.file {
                    file.doc.mark_saved(Some(display_name(&path)));
                    file.path = Some(path);
                }
                self.toast(ToastKind::Info, text);
                true
            }
            Err(e) => {
                self.error(format!("Can't write {}", path.display()), e.to_string());
                false
            }
        }
    }

    /// Hands the document to the browser as a download named `name`. The
    /// file it was opened from is left as it was.
    #[cfg(target_arch = "wasm32")]
    fn download(&mut self, name: String) -> bool {
        let Some(bytes) = self.bytes_to_write() else {
            return false;
        };
        match crate::io::web::download(&name, &bytes) {
            Ok(()) => {
                let mut text = format!("Downloaded {name}.");
                if let Some(file) = &mut self.file {
                    file.doc.mark_saved(Some(name));
                    if file.doc.platform() == nsane::Platform::Pc {
                        text += " The game only reads ALCHEMYXSAVE_<n>.sav: if the \
                                 browser added a number to the name, remove it.";
                    }
                }
                self.toast(ToastKind::Info, text);
                true
            }
            Err(e) => {
                self.error(format!("Can't download {name}"), e);
                false
            }
        }
    }

    /// Asks about unsaved changes before `then`, or does it right away.
    fn guard(&mut self, ctx: &Context, then: Then) {
        if self.is_dirty() {
            self.dialog = Some(Dialog::Unsaved(then));
        } else {
            self.proceed(ctx, then);
        }
    }

    fn proceed(&mut self, ctx: &Context, then: Then) {
        match then {
            Then::Open => {
                #[cfg(not(target_arch = "wasm32"))]
                self.open_dialog();
                // still in the click or key press that asked for it
                #[cfg(target_arch = "wasm32")]
                self.inbox.pick_open(ctx);
            }
            #[cfg(not(target_arch = "wasm32"))]
            Then::OpenPath(path) => self.open_path(path),
            Then::OpenBytes { name, bytes } => self.open_unsaved_bytes(&bytes, name),
            Then::Close => {
                self.closing = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    fn apply(&mut self, label: String, edits: Vec<Edit>) {
        let Some(file) = &mut self.file else {
            return;
        };
        if let Err(e) = file.doc.apply(label, edits) {
            self.toast(ToastKind::Error, format!("The change was refused: {e}"));
        }
    }

    /// Opens the dialog that converts the open save to the other platform.
    fn open_convert(&mut self) {
        if let Some(file) = &self.file {
            self.convert = Some(Convert::new(other_platform(file.doc.platform())));
        }
    }

    fn convert_dialog(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        let (Some(convert), Some(file)) = (&mut self.convert, &mut self.file) else {
            return;
        };
        let unsaved = file.doc.is_dirty().then(|| file.change_count());
        match convert.show(ctx, &file.doc, unsaved, actions) {
            None => {}
            Some(ConvertRequest::Close) => self.convert = None,
            Some(ConvertRequest::PickTemplate) => self.pick_template(ctx),
            Some(ConvertRequest::Write { bytes, name }) => self.write_converted(bytes, name),
        }
    }

    /// A file dialog of the conversion: Save As when `save`, else Open.
    #[cfg(all(not(target_arch = "wasm32"), not(test)))]
    fn convert_file_dialog(
        &mut self,
        save: bool,
        start: Option<&Path>,
    ) -> Result<Option<PathBuf>, crate::io::native::DialogUnavailable> {
        if save {
            crate::io::native::pick_save(start)
        } else {
            crate::io::native::pick_open(start)
        }
    }

    /// Tests answer the dialogs in advance.
    #[cfg(test)]
    fn convert_file_dialog(
        &mut self,
        _save: bool,
        start: Option<&Path>,
    ) -> Result<Option<PathBuf>, crate::io::native::DialogUnavailable> {
        self.dialog_starts.push(start.map(Path::to_owned));
        Ok(self
            .dialog_answers
            .pop_front()
            .expect("the test gives the dialog's answer"))
    }

    /// The folder of the open save, or the last one used.
    #[cfg(not(target_arch = "wasm32"))]
    fn current_dir(&self) -> Option<PathBuf> {
        self.file
            .as_ref()
            .and_then(|f| f.path.as_deref())
            .and_then(Path::parent)
            .map(Path::to_owned)
            .or_else(|| self.prefs.last_dir.clone())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn pick_template(&mut self, _ctx: &Context) {
        let start = self
            .convert
            .as_ref()
            .and_then(|c| c.template_dir())
            .map(Path::to_owned)
            .or_else(|| self.current_dir());
        match self.convert_file_dialog(false, start.as_deref()) {
            Ok(Some(path)) => self.template_path(path),
            Ok(None) => {}
            Err(_) => self.dialog_unavailable(),
        }
    }

    /// Still in the click that asked for it: see `proceed`.
    #[cfg(target_arch = "wasm32")]
    fn pick_template(&mut self, ctx: &Context) {
        self.inbox.pick_template(ctx);
    }

    /// Reads the template of the conversion from `path`.
    #[cfg(not(target_arch = "wasm32"))]
    fn template_path(&mut self, path: PathBuf) {
        let name = display_name(&path);
        let Some(convert) = &mut self.convert else {
            return;
        };
        match crate::io::native::read(&path) {
            Ok(bytes) => convert.set_template(name, &bytes, Some(path)),
            Err(e) => convert.template_unreadable(name.clone(), format!("Can't read {name}: {e}")),
        }
    }

    /// Where to suggest writing the converted save: next to the template
    /// picked (to replace it), else next to the open save. Never the open
    /// save itself, which is of the other platform.
    #[cfg(not(target_arch = "wasm32"))]
    fn converted_path(&self, name: &str) -> PathBuf {
        let source = self.file.as_ref().and_then(|f| f.path.as_deref());
        let dir = self
            .convert
            .as_ref()
            .and_then(|c| c.template_dir())
            .map(Path::to_owned)
            .or_else(|| self.current_dir());
        let mut path = dir.map_or_else(|| PathBuf::from(name), |d| d.join(name));
        if source == Some(path.as_path())
            && let Some(convert) = &self.convert
        {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let target = convert.target.name().to_lowercase();
            path = path.with_file_name(format!("{stem}_{target}.sav"));
        }
        path
    }

    /// Asks where to write the converted save, then writes it.
    #[cfg(not(target_arch = "wasm32"))]
    fn write_converted(&mut self, bytes: Vec<u8>, name: String) {
        let suggested = self.converted_path(&name);
        match self.convert_file_dialog(true, Some(&suggested)) {
            Ok(Some(path)) => self.write_converted_to(path, &bytes),
            Ok(None) => {}
            Err(_) => self.dialog_unavailable(),
        }
    }

    /// Writes the converted save to `path`, backing up the file it replaces,
    /// as Save does.
    #[cfg(not(target_arch = "wasm32"))]
    fn write_converted_to(&mut self, path: PathBuf, bytes: &[u8]) {
        let now = crate::io::native::local_now();
        let written = crate::io::native::write_with_backup(&path, bytes, now);
        if written.is_ok() {
            self.remember_folder(&path);
        }
        let Some(convert) = &mut self.convert else {
            return;
        };
        match written {
            Ok(backup) => convert.written(Written::File { path, backup }),
            Err(e) => convert.write_failed(format!("Can't write {}: {e}", path.display())),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn write_converted(&mut self, bytes: Vec<u8>, name: String) {
        let Some(convert) = &mut self.convert else {
            return;
        };
        match crate::io::web::download(&name, &bytes) {
            Ok(()) => convert.written(Written::Download { name, bytes }),
            Err(e) => convert.write_failed(format!("Can't download {name}: {e}")),
        }
    }

    /// Runs the frame's actions. Edits go first: a field that lost focus
    /// because a toolbar button was clicked commits its value in the same
    /// frame, and the save that follows must include it.
    fn run(&mut self, ctx: &Context, actions: Vec<Action>) {
        let (edits, others): (Vec<_>, Vec<_>) = actions
            .into_iter()
            .partition(|a| matches!(a, Action::Apply { .. }));
        for action in edits.into_iter().chain(others) {
            match action {
                Action::Open => self.guard(ctx, Then::Open),
                #[cfg(not(target_arch = "wasm32"))]
                Action::OpenPath(path) => self.guard(ctx, Then::OpenPath(path)),
                Action::OpenBytes { name, bytes } => {
                    self.guard(ctx, Then::OpenBytes { name, bytes })
                }
                Action::Save => self.save(ctx, None),
                Action::SaveAs => self.save_as(ctx, None),
                Action::Convert => self.open_convert(),
                Action::Undo => {
                    if let Some(file) = &mut self.file
                        && file.doc.undo()
                    {
                        self.crash3.reset();
                    }
                }
                Action::Redo => {
                    if let Some(file) = &mut self.file
                        && file.doc.redo()
                    {
                        self.crash3.reset();
                    }
                }
                Action::Apply { label, edits } => self.apply(label, edits),
            }
        }
    }

    /// The keyboard shortcuts of this frame.
    fn shortcuts(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        if self.dialog.is_some() || self.crash3.has_dialog() || self.convert.is_some() {
            return;
        }
        // A text field keeps its own undo, and holds a value not applied
        // yet: the other shortcuts make it commit first.
        let typing = ctx.egui_wants_keyboard_input();
        // Shift variants first: a shortcut without Shift also matches with it.
        let shortcuts = [
            (SAVE_AS, Action::SaveAs),
            (SAVE, Action::Save),
            (OPEN, Action::Open),
            (REDO_SHIFT, Action::Redo),
            (UNDO, Action::Undo),
            (REDO, Action::Redo),
        ];
        for (shortcut, action) in shortcuts {
            if typing && matches!(action, Action::Undo | Action::Redo) {
                continue;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&shortcut)) {
                if typing {
                    ctx.memory_mut(|m| m.stop_text_input());
                    self.deferred.push(action);
                    ctx.request_repaint();
                } else {
                    actions.push(action);
                }
            }
        }
    }

    /// A file dropped onto the window.
    fn dropped_files(&mut self, ctx: &Context, actions: &mut Vec<Action>) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        let Some(file) = dropped.first() else {
            return;
        };
        if dropped.len() > 1 {
            self.toast(ToastKind::Info, "Only the first dropped file was opened.");
        }
        #[cfg(not(target_arch = "wasm32"))]
        actions.push(Action::OpenPath(file.path().to_owned()));
        // The browser reads it in the background: see `incoming`.
        #[cfg(target_arch = "wasm32")]
        {
            let _ = actions;
            self.inbox.read_dropped(ctx, file.clone());
        }
    }

    /// Files the browser has finished reading.
    #[cfg(target_arch = "wasm32")]
    fn incoming(&mut self, actions: &mut Vec<Action>) {
        use crate::io::web::Via;
        while let Some(incoming) = self.inbox.try_recv() {
            let name = incoming.name;
            match (incoming.via, incoming.bytes) {
                (Via::Template, bytes) => {
                    if let Some(convert) = &mut self.convert {
                        match bytes {
                            Ok(bytes) => convert.set_template(name, &bytes),
                            Err(e) => convert.template_unreadable(name, e),
                        }
                    }
                }
                (Via::Dialog, Ok(bytes)) => {
                    self.open_bytes(&bytes, name);
                }
                (Via::Drop, Ok(bytes)) => actions.push(Action::OpenBytes { name, bytes }),
                (_, Err(e)) => self.error(format!("Can't open {name}"), e),
            }
        }
    }

    fn close_requested(&mut self, ctx: &Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.closing {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.screenshot.is_some() {
            return;
        }
        if self.is_dirty() {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::Unsaved(Then::Close));
        }
    }

    fn update_title(&mut self, ctx: &Context) {
        let title = match &self.file {
            Some(f) => format!(
                "{}{} — {APP_TITLE}",
                if f.doc.is_dirty() { "• " } else { "" },
                f.name()
            ),
            None => APP_TITLE.to_owned(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let ctx = ui.ctx().clone();
        let tip = |text: &str, shortcut: &KeyboardShortcut| {
            format!("{text} ({})", ctx.format_shortcut(shortcut))
        };
        egui::Panel::top("toolbar")
            .frame(bar_frame(ui))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let has_file = self.file.is_some();
                    if ui
                        .button("Open")
                        .on_hover_text(tip("Open a save file", &OPEN))
                        .clicked()
                    {
                        actions.push(Action::Open);
                    }
                    // An error diagnostic (a Switch save that outgrew its
                    // file) keeps the file from being written.
                    let blocked = self.save_blocked();
                    let why_not = |response: egui::Response| match &blocked {
                        Some(reason) => {
                            response.on_disabled_hover_text(format!("Can't save: {reason}"))
                        }
                        None => response,
                    };
                    let can_save = has_file && blocked.is_none();
                    if why_not(ui.add_enabled(can_save, Button::new("Save")))
                        .on_hover_text(tip(SAVE_TIP, &SAVE))
                        .clicked()
                    {
                        actions.push(Action::Save);
                    }
                    if why_not(ui.add_enabled(can_save, Button::new("Save as")))
                        .on_hover_text(tip(SAVE_AS_TIP, &SAVE_AS))
                        .clicked()
                    {
                        actions.push(Action::SaveAs);
                    }
                    if let Some(file) = &self.file {
                        let target = other_platform(file.doc.platform());
                        if ui
                            .button(format!("Convert to {target}…"))
                            .on_hover_text(format!(
                                "Make a copy of this save for the {target} version of the \
                                 game (experimental)"
                            ))
                            .clicked()
                        {
                            actions.push(Action::Convert);
                        }
                    }
                    ui.separator();
                    self.undo_redo(ui, actions, &tip);
                    self.history_menu(ui, actions);
                    if let Some(file) = &self.file {
                        ui.add_enabled_ui(self.game == Game::Crash3, |ui| {
                            ui.menu_button("Presets", |ui| {
                                self.crash3.presets_menu(ui, &file.doc);
                            })
                            .response
                            .on_hover_text("Common goals, applied in one step after a preview");
                        });
                    }
                    if let Some(file) = &mut self.file {
                        ui.separator();
                        status(ui, file);
                        if self.game == Game::Crash3 {
                            self.crash3.diagnostics_badges(ui, &file.doc);
                        }
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.theme_toggle(ui);
                        self.settings_menu(ui);
                        #[cfg(not(target_arch = "wasm32"))]
                        let debug_menu = cfg!(debug_assertions) && self.screenshot.is_none();
                        #[cfg(target_arch = "wasm32")]
                        let debug_menu = cfg!(debug_assertions);
                        if debug_menu {
                            ui.menu_button("Debug", |ui| {
                                ui.checkbox(&mut self.gallery_open, "Icon gallery");
                            });
                        }
                    });
                });
            });
    }

    fn undo_redo(
        &self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
        tip: &dyn Fn(&str, &KeyboardShortcut) -> String,
    ) {
        let doc = self.file.as_ref().map(|f| &f.doc);
        let undo = doc.and_then(|d| d.history().last());
        let redo = doc.and_then(|d| d.redo_history().next());
        let response = ui
            .add_enabled(undo.is_some(), Button::new("⟲"))
            .on_disabled_hover_text("Nothing to undo");
        let response = match undo {
            Some(label) => response.on_hover_text(tip(&format!("Undo: {label}"), &UNDO)),
            None => response,
        };
        if response.clicked() {
            actions.push(Action::Undo);
        }
        let response = ui
            .add_enabled(redo.is_some(), Button::new("⟳"))
            .on_disabled_hover_text("Nothing to redo");
        let response = match redo {
            Some(label) => response.on_hover_text(tip(&format!("Redo: {label}"), &REDO)),
            None => response,
        };
        if response.clicked() {
            actions.push(Action::Redo);
        }
    }

    /// The steps undo and redo can reach; a click goes back or forward to
    /// one of them.
    fn history_menu(&self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let Some(file) = &self.file else {
            return;
        };
        let doc = &file.doc;
        let done: Vec<&str> = doc.history().collect();
        let undone: Vec<&str> = doc.redo_history().collect();
        let enabled = !done.is_empty() || !undone.is_empty();
        ui.add_enabled_ui(enabled, |ui| {
            ui.menu_button("History", |ui| {
                ui.set_min_width(260.0);
                // The oldest steps are dropped beyond the undo depth.
                let start = if done.len() < nsane::UNDO_DEPTH {
                    format!("Opened {}", file.name())
                } else {
                    "Oldest step kept".to_owned()
                };
                let current = done.len();
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        let entries = std::iter::once(start.as_str())
                            .chain(done.iter().copied())
                            .chain(undone.iter().copied());
                        for (position, label) in entries.enumerate() {
                            let text = if position > current {
                                RichText::new(label).weak().italics()
                            } else {
                                RichText::new(label)
                            };
                            let response = ui
                                .add(Button::selectable(position == current, text))
                                .on_hover_text(match position.cmp(&current) {
                                    std::cmp::Ordering::Less => "Undo back to this step",
                                    std::cmp::Ordering::Equal => "The save as it is now",
                                    std::cmp::Ordering::Greater => "Redo up to this step",
                                });
                            if response.clicked() {
                                let action = if position < current {
                                    Action::Undo
                                } else {
                                    Action::Redo
                                };
                                let steps = position.abs_diff(current);
                                actions.extend(std::iter::repeat_n(action, steps));
                                ui.close();
                            }
                        }
                    });
            })
            .response
            .on_hover_text("Every step undo and redo can reach")
            .on_disabled_hover_text("Nothing to undo or redo");
        });
    }

    fn settings_menu(&mut self, ui: &mut Ui) {
        ui.menu_button("⚙", |ui| {
            let mut review = !self.prefs.skip_save_review;
            if ui
                .checkbox(&mut review, "Review the changes before saving")
                .changed()
            {
                self.prefs.skip_save_review = !review;
            }
        })
        .response
        .on_hover_text("Settings");
    }

    fn theme_toggle(&mut self, ui: &mut Ui) {
        let (text, tip) = if self.dark {
            ("☀", "Switch to the light theme")
        } else {
            ("🌙", "Switch to the dark theme")
        };
        if ui.button(text).on_hover_text(tip).clicked() {
            self.dark = !self.dark;
        }
    }

    fn game_tabs(&mut self, ui: &mut Ui, narrow: bool) {
        egui::Panel::top("games")
            .frame(bar_frame(ui))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for game in Game::ALL {
                        let response = ui
                            .add_enabled(
                                game.available(),
                                Button::selectable(self.game == game, game.title()),
                            )
                            .on_disabled_hover_text("Coming later");
                        if response.clicked() {
                            self.game = game;
                        }
                    }
                    if narrow {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.toggle_value(&mut self.prefs.show_details, "Details");
                            ui.toggle_value(&mut self.prefs.show_overview, "Overview");
                        });
                    }
                });
            });
    }

    fn document_ui(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let narrow = ui.ctx().content_rect().width() < NARROW_WIDTH;
        self.game_tabs(ui, narrow);
        let Some(file) = &self.file else {
            return;
        };
        let doc = &file.doc;
        if !narrow || self.prefs.show_overview {
            egui::Panel::left("overview")
                .resizable(true)
                .default_size(240.0)
                .min_size(180.0)
                .show(ui, |ui| {
                    self.crash3.overview(ui, doc, &mut self.selected, actions)
                });
        }
        if !narrow || self.prefs.show_details {
            egui::Panel::right("details")
                .resizable(true)
                .default_size(300.0)
                .min_size(200.0)
                .show(ui, |ui| {
                    self.crash3.details(ui, doc, self.selected, actions)
                });
        }
        egui::CentralPanel::default().show(ui, |ui| {
            self.crash3.central(ui, doc, &mut self.selected, actions)
        });
        let ctx = ui.ctx().clone();
        self.crash3.windows(&ctx, doc, &mut self.selected, actions);
    }

    fn dialogs(&mut self, ctx: &Context) {
        let Some(dialog) = &self.dialog else {
            return;
        };
        let mut close = false;
        let mut then = None;
        let mut save_first = false;
        let mut confirmed = false;
        let mut dont_show_now = None;
        #[cfg(target_arch = "wasm32")]
        let mut download_as = None;
        let blocked = self.save_blocked();
        match dialog {
            Dialog::Unsaved(next) => {
                let (name, count) = match &mut self.file {
                    Some(f) => (f.name().to_owned(), f.change_count()),
                    None => ("The save".to_owned(), 0),
                };
                Modal::new(Id::new("unsaved")).show(ctx, |ui| {
                    ui.set_max_width(380.0);
                    ui.heading("Unsaved changes");
                    ui.add_space(theme::SPACING);
                    ui.label(format!(
                        "{name} has unsaved changes ({} since it was opened). \
                         Save them first?",
                        plural(count, "change", "changes")
                    ));
                    ui.add_space(theme::SPACING);
                    ui.horizontal(|ui| {
                        let save = ui.add_enabled(blocked.is_none(), Button::new("Save"));
                        let save = match &blocked {
                            Some(reason) => save.on_disabled_hover_text(reason.as_str()),
                            None => save,
                        };
                        if save.clicked() {
                            save_first = true;
                            then = Some(next.clone());
                        }
                        if ui.button("Discard").clicked() {
                            then = Some(next.clone());
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape))
                        {
                            close = true;
                        }
                    });
                });
            }
            Dialog::Review {
                review,
                target,
                dont_show,
                ..
            } => {
                let name = self.file.as_ref().map_or("The save", |f| f.name());
                let mut dont_show = *dont_show;
                Modal::new(Id::new("save-review")).show(ctx, |ui| {
                    ui.set_max_width(480.0);
                    ui.heading("Review the changes");
                    ui.add_space(theme::SPACING);
                    review.show(ui, name);
                    ui.separator();
                    target_text(ui, target);
                    ui.add_space(theme::SPACING / 2.0);
                    ui.checkbox(&mut dont_show, "Don't show this again")
                        .on_hover_text(
                            "Save without this review. Turn it back on in the settings (⚙).",
                        );
                    ui.add_space(theme::SPACING);
                    ui.horizontal(|ui| {
                        #[cfg(not(target_arch = "wasm32"))]
                        let confirm = "Save file";
                        #[cfg(target_arch = "wasm32")]
                        let confirm = "Download";
                        let response = ui.add_enabled(blocked.is_none(), Button::new(confirm));
                        let response = match &blocked {
                            Some(reason) => response.on_disabled_hover_text(reason.as_str()),
                            None => response,
                        };
                        if response.clicked() {
                            confirmed = true;
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape))
                        {
                            close = true;
                        }
                    });
                });
                dont_show_now = Some(dont_show);
            }
            Dialog::Error { title, message } => {
                Modal::new(Id::new("error")).show(ctx, |ui| {
                    ui.set_max_width(420.0);
                    ui.horizontal(|ui| {
                        static_icon(ui, Icon::Error, theme::ICON_MEDIUM);
                        ui.heading(title.as_str());
                    });
                    ui.add_space(theme::SPACING);
                    ui.label(message.as_str());
                    ui.add_space(theme::SPACING);
                    if ui.button("OK").clicked()
                        || ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter))
                    {
                        close = true;
                    }
                });
            }
            #[cfg(target_arch = "wasm32")]
            Dialog::DownloadAs(name) => {
                let mut name = name.clone();
                Modal::new(Id::new("download-as")).show(ctx, |ui| {
                    ui.set_max_width(380.0);
                    ui.heading("Download as");
                    ui.add_space(theme::SPACING);
                    let field = ui.text_edit_singleline(&mut name);
                    if !field.has_focus() && !field.lost_focus() {
                        field.request_focus();
                    }
                    let entered = field.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                    ui.add_space(theme::SPACING);
                    let valid = !name.trim().is_empty();
                    ui.horizontal(|ui| {
                        if (ui.add_enabled(valid, Button::new("Download")).clicked() || entered)
                            && valid
                        {
                            download_as = Some(name.trim().to_owned());
                        }
                        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape))
                        {
                            close = true;
                        }
                    });
                });
                if let Some(Dialog::DownloadAs(kept)) = &mut self.dialog {
                    *kept = name;
                }
            }
        }
        if let (Some(value), Some(Dialog::Review { dont_show, .. })) =
            (dont_show_now, &mut self.dialog)
        {
            *dont_show = value;
        }
        if close {
            self.dialog = None;
        }
        if confirmed
            && let Some(Dialog::Review {
                target,
                then,
                dont_show,
                ..
            }) = self.dialog.take()
        {
            if dont_show {
                self.prefs.skip_save_review = true;
            }
            self.write(ctx, target, then);
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(name) = download_as {
            self.dialog = None;
            self.review_or_write(ctx, Target::Download(name), None);
        }
        if let Some(then) = then {
            self.dialog = None;
            if save_first {
                self.save(ctx, Some(then));
            } else {
                self.proceed(ctx, then);
            }
        }
    }

    fn toasts(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        self.toasts.retain(|t| t.until.is_none_or(|u| u > now));
        if self.toasts.is_empty() {
            return;
        }
        let pal = theme::palette(self.dark);
        let mut dismissed = None;
        egui::Area::new(Id::new("toasts"))
            .anchor(Align2::RIGHT_BOTTOM, Vec2::splat(-2.0 * theme::SPACING))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                for (i, toast) in self.toasts.iter_mut().enumerate() {
                    let seconds = match toast.kind {
                        ToastKind::Info => TOAST_SECONDS,
                        ToastKind::Error => ERROR_TOAST_SECONDS,
                    };
                    let until = *toast.until.get_or_insert(now + seconds);
                    let accent = match toast.kind {
                        ToastKind::Info => pal.accent.base,
                        ToastKind::Error => pal.error.base,
                    };
                    let response = Frame::popup(ui.style())
                        .stroke(egui::Stroke::new(1.0, accent))
                        .show(ui, |ui| ui.label(toast.text.as_str()))
                        .response
                        .interact(Sense::click())
                        .on_hover_text("Click to dismiss");
                    if response.clicked() {
                        dismissed = Some(i);
                    }
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                        (until - now).max(0.0),
                    ));
                }
            });
        if let Some(i) = dismissed {
            self.toasts.remove(i);
        }
    }

    /// A veil over the window while a file is dragged over it.
    fn drop_hint(&self, ctx: &Context) {
        if ctx.input(|i| i.raw.hovered_files.is_empty()) {
            return;
        }
        let pal = theme::palette(self.dark);
        let rect = ctx.content_rect();
        let painter =
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, Id::new("drop")));
        painter.rect_filled(rect, 0, pal.background.gamma_multiply(0.85));
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Drop the save file to open it",
            egui::FontId::proportional(theme::FONT_HEADING),
            pal.accent.base,
        );
    }

    /// Draws one frame and runs what the user asked for.
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let dark = self.dark;
        if self.gallery_only {
            egui::CentralPanel::default().show(ui, |ui| self.gallery.ui(ui, &mut self.dark));
        } else {
            let mut actions = Vec::new();
            let deferred = std::mem::take(&mut self.deferred);
            self.shortcuts(&ctx, &mut actions);
            self.dropped_files(&ctx, &mut actions);
            #[cfg(target_arch = "wasm32")]
            self.incoming(&mut actions);
            self.close_requested(&ctx);
            self.toolbar(ui, &mut actions);
            if self.file.is_some() {
                self.document_ui(ui, &mut actions);
            } else {
                egui::CentralPanel::default().show(ui, |ui| {
                    if drop_zone(ui).clicked() {
                        actions.push(Action::Open);
                    }
                });
            }
            self.convert_dialog(&ctx, &mut actions);
            self.dialogs(&ctx);
            self.toasts(&ctx);
            self.drop_hint(&ctx);
            egui::Window::new("Icon gallery")
                .open(&mut self.gallery_open)
                .default_size([900.0, 600.0])
                .show(&ctx, |ui| self.gallery.ui(ui, &mut self.dark));
            actions.extend(deferred);
            self.run(&ctx, actions);
            self.update_title(&ctx);
            #[cfg(target_arch = "wasm32")]
            self.leave_guard.set(self.is_dirty());
        }
        if self.dark != dark {
            theme::apply(&ctx, self.dark);
            self.prefs.dark = Some(self.dark);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(screenshot) = &mut self.screenshot
            && let Some(dark) = screenshot.update(&ctx)
        {
            self.dark = dark;
            theme::apply(&ctx, dark);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        self.show(ui);
        // Written as soon as they change: the browser's storage keeps them
        // right away, and the desktop's is written out on exit.
        if let Some(stored) = &mut self.stored_prefs
            && *stored != self.prefs
            && let Some(storage) = frame.storage_mut()
        {
            self.prefs.store(storage);
            *stored = self.prefs.clone();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(stored) = &mut self.stored_prefs {
            self.prefs.store(storage);
            *stored = self.prefs.clone();
        }
    }

    /// Panel widths and the like start afresh: they would make screenshots
    /// depend on the last run.
    fn persist_egui_memory(&self) -> bool {
        false
    }
}

fn bar_frame(ui: &Ui) -> Frame {
    Frame::side_top_panel(ui.style()).inner_margin(Margin::symmetric(8, 6))
}

/// An icon that only shows something, of `size` points.
pub fn static_icon(ui: &mut Ui, icon: Icon, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if ui.is_rect_visible(rect) {
        let dark = ui.visuals().dark_mode;
        icons::paint(ui.painter(), rect, icon, IconState::OWNED, dark);
    }
    response
}

/// File name, platform badge, size budget and unsaved changes.
fn status(ui: &mut Ui, file: &mut OpenFile) {
    let pal = theme::palette(ui.visuals().dark_mode);
    let name = ui.add(egui::Label::new(RichText::new(file.name()).strong()).extend());
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = &file.path {
        name.on_hover_text(path.display().to_string());
    }
    #[cfg(target_arch = "wasm32")]
    let _ = name;

    let platform = file.doc.platform();
    Frame::new()
        .fill(theme::mix(pal.accent.base, pal.background, 80))
        .stroke(egui::Stroke::new(1.0, pal.accent.base))
        .corner_radius(theme::ROUNDING_SMALL)
        .inner_margin(Margin::symmetric(6, 1))
        .show(ui, |ui| {
            ui.add(
                egui::Label::new(RichText::new(platform.name()).color(pal.text).small()).extend(),
            );
        })
        .response
        .on_hover_text(match platform {
            nsane::Platform::Switch => "A Nintendo Switch save: written back at its fixed size",
            nsane::Platform::Pc => "A PC save: written back at the size of its content",
        });

    let size = file.doc.size_report();
    if let Some(capacity) = size.capacity {
        let color = if !size.fits() {
            pal.error.base
        } else if size.used * 100 > capacity * BUDGET_WARNING_PERCENT {
            pal.warning.base
        } else {
            pal.text_weak
        };
        let text = RichText::new(format!("{} / {} KB", kib(size.used), kib(capacity))).color(color);
        ui.add(egui::Label::new(text).extend())
            .on_hover_text(format!(
                "The save data takes {} of the {} bytes of a Switch save file.",
                size.used, capacity
            ));
    }

    if file.doc.is_dirty() {
        let count = file.change_count();
        let text = if count == 0 {
            "• unsaved".to_owned()
        } else {
            format!("• {}", plural(count, "change", "changes"))
        };
        ui.add(egui::Label::new(RichText::new(text).color(pal.warning.base)).extend())
            .on_hover_text(format!(
                "Not saved yet. {} since the file was opened.",
                plural(count, "change", "changes")
            ));
    }
}

/// Where the reviewed save goes: the file and its backup, or the download.
fn target_text(ui: &mut Ui, target: &Target) {
    match target {
        #[cfg(not(target_arch = "wasm32"))]
        Target::File { path, now } => {
            ui.add(egui::Label::new(format!("Writes {}", path.display())).wrap());
            if path.exists() {
                let backup = crate::io::native::free_backup_path(path, *now);
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "The file it replaces is kept as {}",
                            backup.display()
                        ))
                        .weak(),
                    )
                    .wrap(),
                );
            } else {
                ui.weak("A new file: there is nothing to back up.");
            }
        }
        #[cfg(target_arch = "wasm32")]
        Target::Download(name) => {
            ui.label(format!("Downloads {name}"));
            ui.weak("The file it was opened from is left as it was.");
        }
    }
}

/// The empty window: a large target to open or drop a save.
fn drop_zone(ui: &mut Ui) -> egui::Response {
    let pal = theme::palette(ui.visuals().dark_mode);
    let rect = ui.available_rect_before_wrap().shrink(3.0 * theme::SPACING);
    let response = ui.allocate_rect(rect, Sense::click());
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, theme::ROUNDING, pal.hover_bg);
    }
    painter.extend(egui::Shape::dashed_line(
        &[
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
            rect.left_top(),
        ],
        egui::Stroke::new(2.0, pal.muted),
        10.0,
        6.0,
    ));
    let lines: [(&str, f32, Color32); 4] = [
        ("Open a save file (.sav)", theme::FONT_HEADING, pal.text),
        (
            "Click here, or drop the file onto the window.",
            theme::FONT_BODY,
            pal.text_weak,
        ),
        (
            "Switch: a save exported from the console.",
            theme::FONT_BODY,
            pal.text_weak,
        ),
        ("PC: ALCHEMYXSAVE_<n>.sav", theme::FONT_BODY, pal.text_weak),
    ];
    let mut y = rect.center().y - 50.0;
    for (i, (text, size, color)) in lines.into_iter().enumerate() {
        painter.text(
            egui::pos2(rect.center().x, y),
            Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(size),
            color,
        );
        y += if i == 0 { 40.0 } else { 24.0 };
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Bytes as whole KB (1024 bytes), rounded.
fn kib(bytes: usize) -> usize {
    (bytes + 512) / 1024
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

pub(crate) fn open_error_message(e: &OpenError) -> String {
    match e {
        nsane::Error::NotASave(why) => format!("Not an N. Sane Trilogy save: {why}."),
        nsane::Error::Igb(e) => format!("Not an N. Sane Trilogy save: {e}."),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests;
