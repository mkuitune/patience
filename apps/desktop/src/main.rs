#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 780.0])
            .with_min_inner_size([440.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Klondike",
        options,
        Box::new(|cc| Ok(Box::new(patience_app::PatienceApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
