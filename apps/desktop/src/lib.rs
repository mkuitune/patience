mod app;
mod cards;
mod help;
mod language;
mod platform;
mod table;
mod toolbar;

pub use app::PatienceApp;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn start(
    canvas: web_sys::HtmlCanvasElement,
    mobile_layout: bool,
) -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |cc| Ok(Box::new(PatienceApp::new(cc, mobile_layout)))),
        )
        .await
}
