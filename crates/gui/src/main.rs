#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window on Windows in release

mod app;
#[cfg(not(target_arch = "wasm32"))]
mod app_icon;
mod gallery;
mod games;
mod icons;
mod io;
mod options;
mod prefs;
#[cfg(not(target_arch = "wasm32"))]
mod screenshot;
#[cfg(test)]
mod test_util;
mod theme;

use app::App;
use options::Options;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    io::native::init_logger(); // RUST_LOG=debug for more

    let options = match Options::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("{e}\n\n{}", options::USAGE);
            std::process::exit(2);
        }
    };
    if let Some(dir) = &options.export_icon {
        if let Err(e) = app_icon::export(dir) {
            eprintln!("can't write the icon into {}: {e}", dir.display());
            std::process::exit(1);
        }
        return Ok(());
    }
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(app::APP_TITLE)
            // names the settings folder, and the window for the desktop
            .with_app_id(app::APP_ID)
            .with_icon(app_icon::window_icon())
            .with_inner_size(options.size.unwrap_or([1024.0, 720.0]))
            .with_min_inner_size([480.0, 320.0])
            .with_maximized(options.maximized),
        // Only the settings of `Prefs` are kept: a window size or panel
        // width from the last run would override `--size` and screenshots.
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        app::APP_TITLE,
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc, options)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(App::new(cc, Options::default())))),
            )
            .await;

        // Replace the loading text, or show the error.
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(()) => loading_text.remove(),
                Err(err) => {
                    loading_text.set_inner_html(
                        "<p>The app has crashed. See the developer console for details.</p>",
                    );
                    panic!("Failed to start eframe: {err:?}");
                }
            }
        }
    });
}
