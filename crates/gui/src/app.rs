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
use crate::games::crash3::Crash3Ui;
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

#[derive(Debug, Clone, PartialEq)]
enum Dialog {
    /// Save, discard or cancel, before `Then`.
    Unsaved(Then),
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
        theme::apply(ctx, dark);
        let mut app = Self {
            dark,
            file: None,
            game: Game::Crash3,
            selected: options.select,
            crash3: Crash3Ui::default(),
            dialog: None,
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
        };
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = options.open {
            app.open_path(path);
        }
        if let Some(View::Unsaved) = options.view {
            app.dialog = Some(Dialog::Unsaved(Then::Open));
        }
        app
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

    /// Saves to the document's file, or asks where when it has none. In the
    /// browser, downloads it under its name. Returns whether it was written.
    fn save(&mut self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(path) = self.file.as_ref().and_then(|f| f.path.clone()) {
                return self.save_to(path);
            }
            self.save_as()
        }
        #[cfg(target_arch = "wasm32")]
        {
            let Some(file) = &self.file else {
                return false;
            };
            let name = file.name().to_owned();
            self.download(name)
        }
    }

    /// Asks where to save, then saves. In the browser, asks for the name to
    /// download under. Returns whether the file was written.
    fn save_as(&mut self) -> bool {
        let Some(file) = &self.file else {
            return false;
        };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let suggested = match (&file.path, &self.prefs.last_dir) {
                (Some(path), _) => path.clone(),
                (None, Some(dir)) => dir.join(file.name()),
                (None, None) => PathBuf::from(file.name()),
            };
            match crate::io::native::pick_save(Some(&suggested)) {
                Ok(Some(path)) => self.save_to(path),
                Ok(None) => false,
                Err(_) => {
                    self.dialog_unavailable();
                    false
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.dialog = Some(Dialog::DownloadAs(file.name().to_owned()));
            false
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
    #[cfg(not(target_arch = "wasm32"))]
    fn save_to(&mut self, path: PathBuf) -> bool {
        let Some(bytes) = self.bytes_to_write() else {
            return false;
        };
        let now = crate::io::native::local_now();
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
                Action::Save => {
                    if self.file.is_some() {
                        self.save();
                    }
                }
                Action::SaveAs => {
                    if self.file.is_some() {
                        self.save_as();
                    }
                }
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
        if self.dialog.is_some() {
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
        while let Some(incoming) = self.inbox.try_recv() {
            let name = incoming.name;
            match incoming.bytes {
                Ok(bytes) if incoming.picked => {
                    self.open_bytes(&bytes, name);
                }
                Ok(bytes) => actions.push(Action::OpenBytes { name, bytes }),
                Err(e) => self.error(format!("Can't open {name}"), e),
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
                    if ui
                        .add_enabled(has_file, Button::new("Save"))
                        .on_hover_text(tip(SAVE_TIP, &SAVE))
                        .clicked()
                    {
                        actions.push(Action::Save);
                    }
                    if ui
                        .add_enabled(has_file, Button::new("Save as"))
                        .on_hover_text(tip(SAVE_AS_TIP, &SAVE_AS))
                        .clicked()
                    {
                        actions.push(Action::SaveAs);
                    }
                    ui.separator();
                    self.undo_redo(ui, actions, &tip);
                    if let Some(file) = &mut self.file {
                        ui.separator();
                        status(ui, file);
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.theme_toggle(ui);
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
                .show(ui, |ui| self.crash3.overview(ui, doc, actions));
        }
        if !narrow || self.prefs.show_details {
            egui::Panel::right("details")
                .resizable(true)
                .default_size(300.0)
                .min_size(200.0)
                .show(ui, |ui| self.crash3.details(ui, doc, self.selected));
        }
        egui::CentralPanel::default().show(ui, |ui| self.crash3.central(ui, doc));
    }

    fn dialogs(&mut self, ctx: &Context) {
        let Some(dialog) = &self.dialog else {
            return;
        };
        let mut close = false;
        let mut then = None;
        let mut save_first = false;
        #[cfg(target_arch = "wasm32")]
        let mut download_as = None;
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
                        if ui.button("Save").clicked() {
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
        if close {
            self.dialog = None;
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(name) = download_as {
            self.dialog = None;
            self.download(name);
        }
        if let Some(then) = then {
            self.dialog = None;
            if !save_first || self.save() {
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
    let name = ui.label(RichText::new(file.name()).strong());
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
            ui.label(RichText::new(platform.name()).color(pal.text).small());
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
        ui.label(RichText::new(format!("{} / {} KB", kib(size.used), kib(capacity))).color(color))
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
        ui.label(RichText::new(text).color(pal.warning.base))
            .on_hover_text(format!(
                "Not saved yet. {} since the file was opened.",
                plural(count, "change", "changes")
            ));
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

fn open_error_message(e: &OpenError) -> String {
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
