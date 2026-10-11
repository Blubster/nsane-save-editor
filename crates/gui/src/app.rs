use crate::gallery::Gallery;
use crate::options::Options;
use crate::theme;

pub const APP_TITLE: &str = "Save editor for Crash Bandicoot N. Sane Trilogy";

pub struct App {
    dark: bool,
    gallery: Gallery,
    /// The gallery fills the window instead of the editor.
    gallery_only: bool,
    /// The gallery window opened from the debug menu.
    gallery_open: bool,
    #[cfg(not(target_arch = "wasm32"))]
    screenshot: Option<crate::screenshot::Screenshot>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, options: Options) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let first_shot_theme = options.screenshots.first().and_then(|s| s.dark);
        #[cfg(target_arch = "wasm32")]
        let first_shot_theme = None;
        let dark = first_shot_theme
            .or(options.dark)
            .unwrap_or(cc.egui_ctx.theme() == egui::Theme::Dark);
        theme::apply(&cc.egui_ctx, dark);
        Self {
            dark,
            gallery: Gallery::default(),
            gallery_only: options.icon_gallery,
            gallery_open: false,
            #[cfg(not(target_arch = "wasm32"))]
            screenshot: crate::screenshot::Screenshot::new(options.screenshots),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let dark = self.dark;
        if self.gallery_only {
            egui::CentralPanel::default().show(ui, |ui| self.gallery.ui(ui, &mut self.dark));
        } else {
            if cfg!(debug_assertions) {
                egui::Panel::top("debug_menu").show(ui, |ui| {
                    egui::MenuBar::new().ui(ui, |ui| {
                        ui.menu_button("Debug", |ui| {
                            ui.checkbox(&mut self.gallery_open, "Icon gallery");
                        });
                    });
                });
            }
            egui::CentralPanel::default().show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.heading(APP_TITLE);
                });
            });
            egui::Window::new("Icon gallery")
                .open(&mut self.gallery_open)
                .default_size([900.0, 600.0])
                .show(ui.ctx(), |ui| self.gallery.ui(ui, &mut self.dark));
        }
        if self.dark != dark {
            theme::apply(ui.ctx(), self.dark);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(screenshot) = &mut self.screenshot
            && let Some(dark) = screenshot.update(ui.ctx())
        {
            self.dark = dark;
            theme::apply(ui.ctx(), dark);
        }
    }
}
