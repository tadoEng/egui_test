#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window in release

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        // The model view's pipelines are depth-tested (Depth24Plus).
        depth_buffer: 24,
        viewport: egui::ViewportBuilder::default()
            .with_app_id("talos-native-prototype")
            .with_title("TALOS · prototype (read-only, synthetic demo)")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "talos-native",
        options,
        Box::new(|cc| {
            Ok(Box::new(talos_native::app::App::new(
                cc,
                talos_native::loader::LoadMode::Background,
            )))
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
