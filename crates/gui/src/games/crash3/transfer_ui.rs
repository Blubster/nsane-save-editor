//! Converting the open save to the other platform: the game data goes into
//! a save of that platform, the *template* (`docs/FORMAT.md` §1).
//!
//! The dialog converts as soon as it has a template, writes nothing yet, and
//! checks the result the way a user's file would go: the output bytes are
//! read back and compared with the source as it will be written (its summary
//! recomputed), so unsaved edits and an outdated summary don't count as
//! differences.

#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use egui::{Button, Grid, Id, Key, Modal, RichText, ScrollArea, Ui};
use nsane::crash3::{Crash3Save, Totals};
use nsane::diff::{Change, diff};
use nsane::igb::IgbError;
use nsane::slot::{CRASH3_SLOTS, crash3_slot, file_name};
use nsane::transfer::{Role, TransferError, VERIFIED_IN_GAME, bundled_template_save, transfer};
use nsane::{Document, Platform};

use super::preview::plural;
use super::rooms::banner;
use crate::app::{Action, static_icon};
use crate::icons::Icon;
use crate::theme::{self, SPACING};

const DIALOG_WIDTH: f32 = 520.0;
/// The tallest the list of differences gets before it scrolls.
const DIFFERENCES_HEIGHT: f32 = 160.0;

/// The file name to write the converted save under: the file of a Crash 3
/// save slot (`docs/FORMAT.md` §1). The template's own name when it is one,
/// so that writing over the template replaces that slot; else the slot of
/// the source's name; else the first Crash 3 slot.
pub fn suggested_name(target: Platform, template_name: Option<&str>, source_name: &str) -> String {
    let slot = template_name
        .and_then(|name| crash3_slot(name, target))
        .or_else(|| crash3_slot(source_name, target.other()))
        .unwrap_or(*CRASH3_SLOTS.start());
    file_name(target, slot)
}

/// How the game names the Crash 3 files on `platform`.
fn naming(platform: Platform) -> String {
    let files = format!(
        "{} to {}",
        file_name(platform, *CRASH3_SLOTS.start()),
        file_name(platform, *CRASH3_SLOTS.end())
    );
    match platform {
        Platform::Pc => format!("The PC game keeps one file per save slot: {files} for Crash 3."),
        Platform::Switch => {
            format!("On the console, the game keeps one file per save slot: {files} for Crash 3.")
        }
    }
}

/// A converted save, and how it compares with its source.
#[derive(Debug, Clone)]
pub struct Converted {
    /// The file to write.
    pub bytes: Vec<u8>,
    /// The platform the bytes read back as.
    pub platform: Platform,
    pub before: Totals,
    pub before_lives: i32,
    /// Read back from `bytes`.
    pub totals: Totals,
    pub lives: i32,
    /// From the source, as it will be written, to the bytes read back.
    /// Anything here is a bug of the conversion.
    pub changes: Vec<Change>,
}

/// Why there is nothing to write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// The template can't take the source's data: shown with the template.
    Template(String),
    /// The conversion itself failed.
    Convert(String),
}

/// Converts `source` with `template`, writes it to bytes and reads them back.
pub fn convert(source: &Crash3Save, template: &Crash3Save) -> Result<Converted, Failure> {
    let failed = |e: nsane::Error| Failure::Convert(e.to_string());
    let mut expected = source.clone();
    expected.update_summary().map_err(failed)?;
    let mut out = transfer(source, template).map_err(|e| match e {
        TransferError::Version {
            role: Role::Template,
            ..
        }
        | TransferError::MissingZone(_)
        | TransferError::MissingVariable(_) => {
            Failure::Template(format!("This template can't be used: {e}."))
        }
        e => Failure::Convert(format!("The save can't be converted: {e}.")),
    })?;
    let bytes = out.to_bytes().map_err(|e| match e {
        nsane::Error::Igb(IgbError::DoesNotFit { needed, capacity }) => Failure::Convert(format!(
            "The converted save data takes {needed} bytes, but a Switch save file holds \
             {capacity}."
        )),
        e => failed(e),
    })?;
    let back = Crash3Save::parse(&bytes).map_err(failed)?;
    Ok(Converted {
        platform: Platform::of(&back),
        before: expected.totals(),
        before_lives: expected.lives(),
        totals: back.totals(),
        lives: back.lives(),
        changes: diff(&expected, &back),
        bytes,
    })
}

/// A save the user picked as template, checked for the target platform.
#[derive(Debug, Clone)]
pub struct Template {
    pub name: String,
    #[cfg(not(target_arch = "wasm32"))]
    pub path: Option<PathBuf>,
    pub save: Result<Crash3Save, String>,
}

impl Template {
    /// Reads `bytes` as a template for a save going to `target`.
    pub fn read(name: String, bytes: &[u8], target: Platform) -> Self {
        let save = Crash3Save::parse(bytes)
            .map_err(|e| crate::app::open_error_message(&e))
            .and_then(|save| {
                let platform = Platform::of(&save);
                if platform == target {
                    Ok(save)
                } else {
                    Err(format!(
                        "{name} is a {platform} save, like the one being converted. Pick a \
                         save from the {target} version."
                    ))
                }
            });
        Self {
            name,
            #[cfg(not(target_arch = "wasm32"))]
            path: None,
            save,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// A save of the user's from the target platform: it matches their
    /// game version.
    Own,
    /// The new-game save that comes with the editor.
    Bundled,
}

/// What the dialog has for the current template.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// "My save" is chosen and no file was picked yet.
    NoTemplate,
    Failed(Failure),
    Converted(Converted),
}

impl Outcome {
    /// The bytes to write, when they hold all the game data.
    pub fn ready(&self) -> Option<&Converted> {
        match self {
            Outcome::Converted(c) if c.changes.is_empty() => Some(c),
            _ => None,
        }
    }
}

/// Where the converted save went.
#[derive(Debug, Clone, PartialEq)]
pub enum Written {
    #[cfg(not(target_arch = "wasm32"))]
    File {
        path: PathBuf,
        backup: Option<PathBuf>,
    },
    #[cfg(target_arch = "wasm32")]
    Download { name: String, bytes: Vec<u8> },
}

/// What the dialog asks the window to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    /// Show a file picker for the template.
    PickTemplate,
    /// Write `bytes`: native, through a Save As dialog suggesting `name`;
    /// on the web, as a download named `name`.
    Write {
        bytes: Vec<u8>,
        name: String,
    },
    Close,
}

/// The "Convert to PC / Convert to Switch" dialog.
#[derive(Debug)]
pub struct Convert {
    pub target: Platform,
    pub choice: Choice,
    own: Option<Template>,
    bundled: Option<Result<Crash3Save, String>>,
    /// The outcome, for the document generation and choice it was
    /// computed for.
    computed: Option<(u64, Choice, Outcome)>,
    /// The download name (web), suggested again when the template changes.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    name: Option<String>,
    write_error: Option<String>,
    written: Option<Written>,
    /// How many conversions the dialog ran.
    #[cfg(test)]
    pub runs: usize,
}

impl Convert {
    pub fn new(target: Platform) -> Self {
        Self {
            target,
            choice: Choice::Own,
            own: None,
            bundled: None,
            computed: None,
            name: None,
            write_error: None,
            written: None,
            #[cfg(test)]
            runs: 0,
        }
    }

    /// The picked template, from its bytes. A file that is not a save of
    /// the target platform shows why and keeps Convert disabled.
    pub fn set_template(
        &mut self,
        name: String,
        bytes: &[u8],
        #[cfg(not(target_arch = "wasm32"))] path: Option<PathBuf>,
    ) {
        #[allow(unused_mut)] // the path is native only
        let mut template = Template::read(name, bytes, self.target);
        #[cfg(not(target_arch = "wasm32"))]
        {
            template.path = path;
        }
        self.own = Some(template);
        self.choice = Choice::Own;
        self.changed();
    }

    /// A template that could not be read at all.
    pub fn template_unreadable(&mut self, name: String, error: String) {
        self.own = Some(Template {
            name,
            #[cfg(not(target_arch = "wasm32"))]
            path: None,
            save: Err(error),
        });
        self.choice = Choice::Own;
        self.changed();
    }

    fn changed(&mut self) {
        self.computed = None;
        self.name = None;
        self.write_error = None;
    }

    /// The folder of the picked template: writing there replaces the
    /// user's own save of the target platform.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn template_dir(&self) -> Option<&Path> {
        match self.choice {
            Choice::Own => self.own.as_ref()?.path.as_deref()?.parent(),
            Choice::Bundled => None,
        }
    }

    /// The name to suggest for the converted file.
    pub fn suggested_name(&self, doc: &Document) -> String {
        let template = match self.choice {
            Choice::Own => self.own.as_ref().map(|t| t.name.as_str()),
            Choice::Bundled => None,
        };
        suggested_name(
            self.target,
            template,
            doc.file_name().unwrap_or("Untitled.sav"),
        )
    }

    /// Converts again if the template, the choice or the document changed.
    fn refresh(&mut self, doc: &Document) {
        let generation = doc.generation();
        if self
            .computed
            .as_ref()
            .is_some_and(|(g, c, _)| *g == generation && *c == self.choice)
        {
            return;
        }
        let template = match self.choice {
            Choice::Own => self.own.as_ref().map(|t| t.save.as_ref()),
            Choice::Bundled => Some(
                self.bundled
                    .get_or_insert_with(|| {
                        bundled_template_save(self.target).map_err(|e| e.to_string())
                    })
                    .as_ref(),
            ),
        };
        let outcome = match template {
            None => Outcome::NoTemplate,
            Some(Err(e)) => Outcome::Failed(Failure::Template(e.clone())),
            Some(Ok(template)) => {
                #[cfg(test)]
                {
                    self.runs += 1;
                }
                match convert(doc.current(), template) {
                    Ok(c) => Outcome::Converted(c),
                    Err(f) => Outcome::Failed(f),
                }
            }
        };
        self.computed = Some((generation, self.choice, outcome));
    }

    /// The outcome for the document as it is now.
    #[cfg(test)]
    pub fn outcome(&mut self, doc: &Document) -> &Outcome {
        self.refresh(doc);
        &self.computed.as_ref().expect("just computed").2
    }

    pub fn write_failed(&mut self, error: String) {
        self.write_error = Some(error);
    }

    pub fn written(&mut self, written: Written) {
        self.write_error = None;
        self.written = Some(written);
    }

    #[cfg(test)]
    pub fn is_written(&self) -> bool {
        self.written.is_some()
    }

    /// Draws the dialog. `unsaved` is the number of unsaved changes, when
    /// the document has some.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        doc: &Document,
        unsaved: Option<usize>,
        actions: &mut Vec<Action>,
    ) -> Option<Request> {
        if let Some(written) = self.written.clone() {
            return self.show_written(ctx, written, actions);
        }
        self.refresh(doc);
        #[cfg(target_arch = "wasm32")]
        if self.name.is_none() {
            self.name = Some(self.suggested_name(doc));
        }
        let mut request = None;
        let mut choice = self.choice;
        let target = self.target;
        Modal::new(Id::new("convert")).show(ctx, |ui| {
            ui.set_max_width(DIALOG_WIDTH);
            ui.heading(format!("Convert to {target}"));
            if !VERIFIED_IN_GAME {
                ui.add_space(SPACING / 2.0);
                banner(
                    ui,
                    "Experimental. Not yet tested in game. Keep a backup of your saves.",
                );
            }
            ui.add_space(SPACING);
            source(ui, doc, unsaved);
            ui.separator();
            if self.template(ui, &mut choice) {
                request = Some(Request::PickTemplate);
            }
            ui.separator();
            let outcome = &self.computed.as_ref().expect("refreshed").2;
            result(ui, target, outcome);
            ui.separator();
            #[cfg(not(target_arch = "wasm32"))]
            let name = self.suggested_name(doc);
            #[cfg(target_arch = "wasm32")]
            let name = self.name.clone().unwrap_or_default();
            let name = self.output(ui, name);
            if let Some(e) = &self.write_error {
                error_line(ui, e);
            }
            ui.add_space(SPACING);
            let outcome = &self.computed.as_ref().expect("refreshed").2;
            let blocker = match outcome {
                Outcome::NoTemplate => Some("Pick a template first".to_owned()),
                Outcome::Failed(Failure::Template(e) | Failure::Convert(e)) => Some(e.clone()),
                Outcome::Converted(c) if !c.changes.is_empty() => {
                    Some("The converted save would lose game data".to_owned())
                }
                Outcome::Converted(_) if name.trim().is_empty() => {
                    Some("Give the file a name".to_owned())
                }
                Outcome::Converted(_) => None,
            };
            ui.horizontal(|ui| {
                #[cfg(not(target_arch = "wasm32"))]
                let label = "Convert…";
                #[cfg(target_arch = "wasm32")]
                let label = "Convert and download";
                let response = ui.add_enabled(blocker.is_none(), Button::new(label));
                let response = match &blocker {
                    Some(why) => response.on_disabled_hover_text(why.as_str()),
                    None => response.on_hover_text(format!(
                        "Write the {target} save. The open save stays as it is."
                    )),
                };
                if response.clicked()
                    && let Some(converted) = outcome.ready()
                {
                    request = Some(Request::Write {
                        bytes: converted.bytes.clone(),
                        name: name.trim().to_owned(),
                    });
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    request = Some(Request::Close);
                }
            });
            #[cfg(target_arch = "wasm32")]
            {
                self.name = Some(name);
            }
        });
        if choice != self.choice {
            self.choice = choice;
            self.changed();
            ctx.request_repaint();
        }
        request
    }

    /// The template choice. Returns whether "Pick file…" was clicked.
    fn template(&self, ui: &mut Ui, choice: &mut Choice) -> bool {
        let target = self.target;
        ui.strong("Template");
        wrapped_weak(
            ui,
            format!(
                "The game data is copied into a {target} save, which gives the file its \
                 {target} layout. Its own progress is replaced."
            ),
        );
        ui.radio_value(
            choice,
            Choice::Own,
            format!("My save from the {target} version (recommended)"),
        )
        .on_hover_text("It matches your game version");
        let mut pick = false;
        ui.indent("own-template", |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button("Pick file…")
                    .on_hover_text(format!("Pick a {target} save"))
                    .clicked()
                {
                    pick = true;
                }
                match &self.own {
                    Some(t) => {
                        let label = ui.add(egui::Label::new(RichText::new(&t.name).strong()));
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(path) = &t.path {
                            label.on_hover_text(path.display().to_string());
                        }
                        #[cfg(target_arch = "wasm32")]
                        let _ = label;
                    }
                    None => {
                        ui.weak("No file picked.");
                    }
                }
            });
            let own_error = match (&self.own, *choice) {
                (Some(Template { save: Err(e), .. }), Choice::Own) => Some(e.as_str()),
                (Some(_), Choice::Own) => match self.computed.as_ref().map(|c| &c.2) {
                    Some(Outcome::Failed(Failure::Template(e))) => Some(e.as_str()),
                    _ => None,
                },
                _ => None,
            };
            if let Some(e) = own_error {
                error_line(ui, e);
            }
        });
        ui.radio_value(choice, Choice::Bundled, "Built-in new-game template")
            .on_hover_text(format!("A new game saved by the {target} version"));
        if *choice == Choice::Bundled
            && let Some((_, Choice::Bundled, Outcome::Failed(Failure::Template(e)))) =
                &self.computed
        {
            error_line(ui, e);
        }
        if pick {
            *choice = Choice::Own;
        }
        pick
    }

    /// Where the converted save goes. Returns the name to write under.
    fn output(&self, ui: &mut Ui, name: String) -> String {
        let target = self.target;
        ui.strong("Output");
        #[cfg(not(target_arch = "wasm32"))]
        let name = {
            ui.label(format!("Suggested file name: {name}"));
            wrapped_weak(
                ui,
                format!(
                    "{} To replace a slot, write over its file; a backup is made.",
                    naming(target)
                ),
            );
            name
        };
        #[cfg(target_arch = "wasm32")]
        let name = {
            let mut name = name;
            ui.horizontal(|ui| {
                ui.label("Download as");
                ui.text_edit_singleline(&mut name);
            });
            wrapped_weak(
                ui,
                format!(
                    "{} To replace a slot, put the download in place of its file, after \
                     keeping a copy of that file.",
                    naming(target)
                ),
            );
            name
        };
        name
    }

    fn show_written(
        &mut self,
        ctx: &egui::Context,
        written: Written,
        actions: &mut Vec<Action>,
    ) -> Option<Request> {
        let mut request = None;
        let target = self.target;
        Modal::new(Id::new("converted")).show(ctx, |ui| {
            ui.set_max_width(DIALOG_WIDTH);
            ui.heading(format!("Converted to {target}"));
            ui.add_space(SPACING);
            match &written {
                #[cfg(not(target_arch = "wasm32"))]
                Written::File { path, backup } => {
                    wrapped(ui, format!("Saved {}", path.display()));
                    if let Some(backup) = backup {
                        wrapped_weak(
                            ui,
                            format!("Backup of the previous file: {}", backup.display()),
                        );
                    }
                }
                #[cfg(target_arch = "wasm32")]
                Written::Download { name, .. } => {
                    wrapped(ui, format!("Downloaded {name}."));
                    wrapped_weak(
                        ui,
                        format!(
                            "{} If the browser added a number to the name, remove it.",
                            naming(target)
                        ),
                    );
                }
            }
            if !VERIFIED_IN_GAME {
                wrapped_weak(ui, "Check it in game before deleting anything.");
            }
            ui.add_space(SPACING);
            ui.horizontal(|ui| {
                if ui
                    .button("Open the converted save")
                    .on_hover_text("Open it in the editor, in place of this one")
                    .clicked()
                {
                    actions.push(match &written {
                        #[cfg(not(target_arch = "wasm32"))]
                        Written::File { path, .. } => Action::OpenPath(path.clone()),
                        #[cfg(target_arch = "wasm32")]
                        Written::Download { name, bytes } => Action::OpenBytes {
                            name: name.clone(),
                            bytes: bytes.clone(),
                        },
                    });
                    request = Some(Request::Close);
                }
                if ui.button("Close").clicked()
                    || ui.input(|i| i.key_pressed(Key::Escape) || i.key_pressed(Key::Enter))
                {
                    request = Some(Request::Close);
                }
            });
        });
        request
    }
}

fn wrapped(ui: &mut Ui, text: impl Into<egui::WidgetText>) {
    ui.add(egui::Label::new(text).wrap());
}

fn wrapped_weak(ui: &mut Ui, text: impl Into<String>) {
    ui.add(egui::Label::new(RichText::new(text).weak()).wrap());
}

fn error_line(ui: &mut Ui, text: &str) {
    let pal = theme::palette(ui.visuals().dark_mode);
    ui.horizontal(|ui| {
        static_icon(ui, Icon::Error, theme::ICON_SMALL);
        ui.add(egui::Label::new(RichText::new(text).color(pal.error.base)).wrap());
    });
}

/// The open save: name, platform, percentage, and whether unsaved changes
/// go with it.
fn source(ui: &mut Ui, doc: &Document, unsaved: Option<usize>) {
    ui.strong("Source");
    let save = doc.current();
    ui.label(format!(
        "{} · {} · {}% · {} lives",
        doc.file_name().unwrap_or("Untitled"),
        doc.platform(),
        save.totals().percent(),
        save.lives()
    ));
    if let Some(count) = unsaved {
        wrapped_weak(
            ui,
            format!(
                "Includes the unsaved changes ({}). The open save itself is not written.",
                plural(count, "change", "changes")
            ),
        );
    }
}

/// The totals of the source and of the converted save, and the verdict.
fn result(ui: &mut Ui, target: Platform, outcome: &Outcome) {
    ui.strong("Result");
    let converted = match outcome {
        Outcome::NoTemplate => {
            ui.weak(format!(
                "Pick a save from the {target} version, or use the built-in template."
            ));
            return;
        }
        // Shown with the template.
        Outcome::Failed(Failure::Template(_)) => {
            ui.weak("No result: see the template.");
            return;
        }
        Outcome::Failed(Failure::Convert(e)) => {
            error_line(ui, e);
            return;
        }
        Outcome::Converted(c) => c,
    };
    let pal = theme::palette(ui.visuals().dark_mode);
    let (b, a) = (&converted.before, &converted.totals);
    let rows = [
        (
            "Percentage",
            format!("{}%", b.percent()),
            format!("{}%", a.percent()),
        ),
        ("Crystals", b.crystals.to_string(), a.crystals.to_string()),
        ("Gems", b.gems.to_string(), a.gems.to_string()),
        ("Relics", b.relics().to_string(), a.relics().to_string()),
        ("Bosses", b.bosses.to_string(), a.bosses.to_string()),
        (
            "Lives",
            converted.before_lives.to_string(),
            converted.lives.to_string(),
        ),
    ];
    Grid::new("convert-totals")
        .num_columns(3)
        .spacing([3.0 * SPACING, 2.0])
        .show(ui, |ui| {
            ui.label("");
            ui.weak("Source");
            ui.weak(format!("Converted ({})", converted.platform));
            ui.end_row();
            for (name, before, after) in rows {
                ui.label(name);
                ui.label(&before);
                if before == after {
                    ui.label(after);
                } else {
                    ui.colored_label(pal.error.base, after);
                }
                ui.end_row();
            }
        });
    ui.add_space(SPACING / 2.0);
    if converted.changes.is_empty() {
        ui.label(
            RichText::new("No game data lost: the converted save reads back the same.")
                .color(pal.accent.base),
        );
        return;
    }
    error_line(
        ui,
        &format!(
            "The converted save reads back with {}. This is a bug of the editor: nothing \
             can be written.",
            plural(converted.changes.len(), "difference", "differences")
        ),
    );
    ScrollArea::vertical()
        .max_height(DIFFERENCES_HEIGHT)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for change in &converted.changes {
                wrapped(ui, format!("• {}", change.describe()));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::repo_file;

    fn load(rel: &str) -> Crash3Save {
        Crash3Save::parse(&std::fs::read(repo_file(rel)).unwrap()).unwrap()
    }

    #[test]
    fn suggested_names_are_crash_3_slot_files() {
        let pc = |template, source| suggested_name(Platform::Pc, template, source);
        let switch = |template, source| suggested_name(Platform::Switch, template, source);
        // The template's name, when it is a Crash 3 slot of the target.
        assert_eq!(
            pc(Some("ALCHEMYXSAVE_10.sav"), "MACK_9.sav"),
            "ALCHEMYXSAVE_10.sav"
        );
        assert_eq!(
            switch(Some("MACK_11.sav"), "ALCHEMYXSAVE_9.sav"),
            "MACK_11.sav"
        );
        // Else the source's slot.
        assert_eq!(pc(Some("backup.sav"), "MACK_9.sav"), "ALCHEMYXSAVE_9.sav");
        assert_eq!(
            pc(Some("ALCHEMYXSAVE_2.sav"), "mack_9.sav"),
            "ALCHEMYXSAVE_9.sav"
        );
        assert_eq!(switch(None, "ALCHEMYXSAVE_10.sav"), "MACK_10.sav");
        assert_eq!(switch(Some("0001.sav"), "ALCHEMYXSAVE_8.sav"), "MACK_8.sav");
        // Else the first Crash 3 slot.
        assert_eq!(pc(None, "0019.sav"), "ALCHEMYXSAVE_8.sav");
        assert_eq!(pc(None, "MACK_3.sav"), "ALCHEMYXSAVE_8.sav");
        assert_eq!(switch(Some("MACK_4.sav"), "foo.sav"), "MACK_8.sav");
        // A source named like the target's files doesn't count.
        assert_eq!(switch(None, "MACK_10.sav"), "MACK_8.sav");
    }

    /// `0019.sav` to PC with the user's own PC save, and back to Switch.
    #[test]
    fn switch_to_pc_and_back() {
        let source = load("saves/0019.sav");
        let pc = convert(&source, &load("saves/ALCHEMYXSAVE_8.sav")).unwrap();
        assert!(pc.changes.is_empty(), "{:?}", pc.changes);
        assert_eq!(pc.platform, Platform::Pc);
        let back = Crash3Save::parse(&pc.bytes).unwrap();
        assert_eq!(pc.bytes, back.igb().build_unpadded().unwrap(), "unpadded");
        assert_eq!(pc.totals, source.totals());
        assert_eq!(
            (pc.before, pc.before_lives),
            (source.totals(), source.lives())
        );
        assert_eq!(pc.lives, source.lives());

        for template in [
            bundled_template_save(Platform::Switch).unwrap(),
            load("saves/0001.sav"),
        ] {
            let switch = convert(&back, &template).unwrap();
            assert_eq!(switch.platform, Platform::Switch);
            assert_eq!(switch.bytes.len(), 153_600);
            assert!(switch.changes.is_empty());
            let again = Crash3Save::parse(&switch.bytes).unwrap();
            assert!(diff(&source, &again).is_empty());
        }
    }

    #[test]
    fn templates_of_the_wrong_platform_or_not_saves_are_refused() {
        let bytes = std::fs::read(repo_file("saves/0018.sav")).unwrap();
        let t = Template::read("0018.sav".into(), &bytes, Platform::Pc);
        let e = t.save.unwrap_err();
        assert!(e.contains("0018.sav is a Switch save"), "{e}");
        assert!(e.contains("from the PC version"), "{e}");
        let t = Template::read("Cargo.toml".into(), b"[package]", Platform::Pc);
        assert!(
            t.save
                .unwrap_err()
                .starts_with("Not an N. Sane Trilogy save")
        );
        let t = Template::read("0018.sav".into(), &bytes, Platform::Switch);
        assert!(t.save.is_ok());
    }

    /// A save too large for a Switch file can't be converted to Switch; an
    /// oversized Switch save converts to PC, which has no size limit.
    #[test]
    fn a_save_too_large_for_the_switch_is_refused() {
        let oversized = |rel: &str| {
            let bytes = std::fs::read(repo_file(rel)).unwrap();
            let mut doc = Document::open(&bytes, None).unwrap();
            let edits = (0..40)
                .map(|i| nsane::crash3::Edit::RawFlagSet {
                    key: format!("{i}_{}", "x".repeat(4000)),
                })
                .collect();
            doc.apply("big", edits).unwrap();
            assert!(!doc.size_report().fits() || doc.platform() == Platform::Pc);
            doc.current().clone()
        };
        let switch = bundled_template_save(Platform::Switch).unwrap();
        match convert(&oversized("saves/ALCHEMYXSAVE_8.sav"), &switch) {
            Err(Failure::Convert(e)) => assert!(e.contains("a Switch save file holds"), "{e}"),
            other => panic!("{other:?}"),
        }
        let pc = bundled_template_save(Platform::Pc).unwrap();
        let converted = convert(&oversized("saves/0018.sav"), &pc).unwrap();
        assert!(converted.changes.is_empty());
        assert!(converted.bytes.len() > 153_600);
    }

    /// A template the core refuses is shown with the template; a source it
    /// refuses, with the result.
    #[test]
    fn transfer_errors_go_where_they_belong() {
        let with_version = |save: Crash3Save, version| {
            let mut igb = save.into_igb();
            igb.summary_mut()
                .set("save_version", nsane::igb::SummaryValue::Int(version))
                .unwrap();
            Crash3Save::from_igb(igb).unwrap()
        };
        let source = load("saves/0019.sav");
        let template = load("saves/ALCHEMYXSAVE_8.sav");
        match convert(&source, &with_version(template.clone(), 74)) {
            Err(Failure::Template(e)) => assert!(e.contains("version 74"), "{e}"),
            other => panic!("{other:?}"),
        }
        match convert(&with_version(source, 76), &template) {
            Err(Failure::Convert(e)) => assert!(e.contains("version 76"), "{e}"),
            other => panic!("{other:?}"),
        }
    }
}
