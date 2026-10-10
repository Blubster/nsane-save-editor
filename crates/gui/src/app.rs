pub const APP_TITLE: &str = "Save editor for Crash Bandicoot N. Sane Trilogy";

#[derive(Default)]
pub struct App;

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                ui.heading(APP_TITLE);
            });
        });
    }
}
