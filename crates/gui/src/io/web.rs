//! Files in the browser: the open dialog, dropped files, downloads, and the
//! question asked before the page is left with unsaved changes.
//!
//! The browser only hands out a file's name and content, never its place on
//! disk, so nothing is ever written over the original: a save is a download.

use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::wasm_bindgen::closure::Closure;
use eframe::wasm_bindgen::{JsCast as _, JsValue};
use egui::{Context, DroppedFileHandle};

/// Where a file read in the background comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    /// The open dialog, which is only shown once unsaved changes were dealt
    /// with.
    Dialog,
    /// Dropped onto the page: it still has to ask about unsaved changes.
    Drop,
    /// The template picked for a conversion.
    Template,
}

/// A file read in the background.
pub struct Incoming {
    pub name: String,
    pub bytes: Result<Vec<u8>, String>,
    pub via: Via,
}

/// Files read asynchronously, delivered to the frame that polls for them.
pub struct Inbox {
    tx: Sender<Incoming>,
    rx: Receiver<Incoming>,
}

impl Default for Inbox {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { tx, rx }
    }
}

impl Inbox {
    /// Shows the open dialog. Call it while handling the click or key press:
    /// browsers only show a file picker in response to the user.
    pub fn pick_open(&self, ctx: &Context) {
        self.pick(ctx, "Open a save file", Via::Dialog);
    }

    /// Shows the dialog that picks a conversion's template, as `pick_open`.
    pub fn pick_template(&self, ctx: &Context) {
        self.pick(ctx, "Pick a save from the other platform", Via::Template);
    }

    fn pick(&self, ctx: &Context, title: &str, via: Via) {
        // Built now, inside the user's gesture; it shows when first polled.
        let dialog = rfd::AsyncFileDialog::new()
            .set_title(title)
            .add_filter("Save files", &["sav"])
            .pick_file();
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let Some(file) = dialog.await else {
                return; // cancelled
            };
            let incoming = Incoming {
                name: file.file_name(),
                bytes: Ok(file.read().await),
                via,
            };
            if tx.send(incoming).is_ok() {
                ctx.request_repaint();
            }
        });
    }

    /// Reads a file dropped onto the page.
    pub fn read_dropped(&self, ctx: &Context, file: DroppedFileHandle) {
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let name = file.path().to_string_lossy().into_owned();
            let bytes = file.bytes_async().await;
            if tx
                .send(Incoming {
                    name,
                    bytes,
                    via: Via::Drop,
                })
                .is_ok()
            {
                ctx.request_repaint();
            }
        });
    }

    pub fn try_recv(&self) -> Option<Incoming> {
        self.rx.try_recv().ok()
    }
}

fn js_error(value: JsValue) -> String {
    value
        .as_string()
        .or_else(|| {
            value
                .dyn_ref::<js_sys::Error>()
                .map(|e| String::from(e.message()))
        })
        .unwrap_or_else(|| format!("{value:?}"))
}

/// How long the download's address stays valid, in milliseconds. The
/// browser has started reading it long before.
const DOWNLOAD_URL_LIFETIME_MS: i32 = 60_000;

/// Offers `bytes` as a download named `name`.
pub fn download(name: &str, bytes: &[u8]) -> Result<(), String> {
    let window = web_sys::window().ok_or("no browser window")?;
    let document = window.document().ok_or("no document")?;
    let body = document.body().ok_or("no document body")?;

    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/octet-stream");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(js_error)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js_error)?;

    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(js_error)?
        .dyn_into()
        .map_err(|_| "not a link element")?;
    anchor.set_href(&url);
    anchor.set_download(name);
    body.append_child(&anchor).map_err(js_error)?;
    anchor.click();
    anchor.remove();

    let revoke = Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    window
        .set_timeout_with_callback_and_timeout_and_arguments_0(
            revoke.unchecked_ref(),
            DOWNLOAD_URL_LIFETIME_MS,
        )
        .map_err(js_error)?;
    Ok(())
}

/// Makes the browser ask before the page is closed or reloaded, while
/// armed. The browser words the question itself.
#[derive(Default)]
pub struct LeaveGuard {
    listener: Option<Closure<dyn FnMut(web_sys::BeforeUnloadEvent)>>,
}

impl LeaveGuard {
    pub fn set(&mut self, armed: bool) {
        let Some(window) = web_sys::window() else {
            return;
        };
        match (armed, &self.listener) {
            (true, None) => {
                let listener = Closure::<dyn FnMut(_)>::new(|event: web_sys::BeforeUnloadEvent| {
                    event.prevent_default();
                    // older browsers ask only when a return value is set
                    event.set_return_value("");
                });
                match window.add_event_listener_with_callback(
                    "beforeunload",
                    listener.as_ref().unchecked_ref(),
                ) {
                    Ok(()) => self.listener = Some(listener),
                    Err(e) => log::warn!("can't watch for the page closing: {}", js_error(e)),
                }
            }
            (false, Some(listener)) => {
                let _ = window.remove_event_listener_with_callback(
                    "beforeunload",
                    listener.as_ref().unchecked_ref(),
                );
                self.listener = None;
            }
            _ => {}
        }
    }
}
