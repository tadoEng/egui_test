#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).

    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let invalid =
        |message: String| eframe::Error::AppCreation(std::io::Error::other(message).into());
    let startup = match arguments.as_slice() {
        [] => None,
        [flag, count] if flag == "--members" => {
            let preset = match count.as_str() {
                "48" => eframe_template::SyntheticPreset::Demo,
                "50000" => eframe_template::SyntheticPreset::FiftyThousand,
                "100000" => eframe_template::SyntheticPreset::HundredThousand,
                _ => return Err(invalid("Supported counts: 48, 50000, 100000".into())),
            };
            Some(eframe_template::TemplateApp::with_preset(preset))
        }
        #[cfg(feature = "e2k")]
        [flag, path] if flag == "--e2k" => {
            Some(eframe_template::TemplateApp::from_e2k(path).map_err(invalid)?)
        }
        _ => {
            return Err(invalid(
                "Usage: --members 50000|100000, or --e2k <file> with --features e2k".into(),
            ));
        }
    };

    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_app_id("talos-egui-structural-lab")
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([780.0, 520.0])
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(
                    &include_bytes!("../assets/favicon-512x512.png")[..],
                )
                .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "TALOS | egui Structural Lab",
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(
                startup.unwrap_or_else(|| eframe_template::TemplateApp::new(cc)),
            ))
        }),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
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
                Box::new(|cc| Ok(Box::new(eframe_template::TemplateApp::new(cc)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(()) => {
                    loading_text.remove();
                }
                Err(err) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {err:?}");
                }
            }
        }
    });
}
