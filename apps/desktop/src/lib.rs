mod app;
mod cards;
mod help;
mod language;
mod platform;
mod table;
mod toolbar;

pub use app::PatienceApp;

#[cfg(all(target_arch = "wasm32", feature = "webgpu", feature = "webgl2"))]
compile_error!("Select one web backend: webgpu or webgl2, using --no-default-features.");

#[cfg(all(
    target_arch = "wasm32",
    not(any(feature = "webgpu", feature = "webgl2"))
))]
compile_error!("Select a web backend: webgpu or webgl2.");

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn start(
    canvas: web_sys::HtmlCanvasElement,
    mobile_layout: bool,
) -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();
    let runner = eframe::WebRunner::new();
    let result = runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(move |cc| Ok(Box::new(PatienceApp::new(cc, mobile_layout)))),
        )
        .await;
    if result.is_err() {
        runner.destroy();
    }
    result
}
