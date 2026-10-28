#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window in release

/// `talos-native` opens the public synthetic tower. `--public-results-demo`
/// opens the authored LAB-REAL01 cached-results fixture instead, and
/// `--version <folder>` (repeatable) opens version folders read-only.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    env_logger::init();
    let project = match project(std::env::args().skip(1)) {
        Ok(project) => project,
        Err(message) => {
            eprintln!("talos-native: {message}");
            return std::process::ExitCode::from(2);
        }
    };
    let title = format!(
        "TALOS · prototype (read-only, {})",
        project.authority.label()
    );
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        // The model view's pipelines are depth-tested (Depth24Plus).
        depth_buffer: 24,
        viewport: egui::ViewportBuilder::default()
            .with_app_id("talos-native-prototype")
            .with_title(title)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "talos-native",
        options,
        Box::new(|cc| {
            Ok(Box::new(talos_native::app::App::new(
                cc,
                talos_native::loader::LoadMode::Background,
                project,
            )))
        }),
    );
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("talos-native: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn project(
    mut args: impl Iterator<Item = String>,
) -> Result<talos_native::project::Project, String> {
    use talos_native::project::Project;
    let mut folders = Vec::new();
    let mut fixture = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => folders.push(std::path::PathBuf::from(
                args.next().ok_or("--version needs a version folder")?,
            )),
            "--public-results-demo" => fixture = true,
            other => {
                return Err(format!(
                    "unknown argument {other:?}; use --version <folder> (repeatable) or \
                     --public-results-demo"
                ));
            }
        }
    }
    match (fixture, folders.is_empty()) {
        (true, false) => Err("--public-results-demo and --version cannot be combined".into()),
        (true, true) => Ok(Project::public_results_fixture()),
        (false, false) => Project::from_folders(&folders),
        (false, true) => Ok(Project::public_demo()),
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
