#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // no console window in release

/// `talos-native` opens the public synthetic tower. `--public-results-demo`
/// opens the authored LAB-REAL01 cached-results fixture instead, and
/// `--version <folder>` (repeatable) opens version folders read-only.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> std::process::ExitCode {
    env_logger::init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let smoke_mode = args == ["--public-echarts-smoke"];
    let project = match if smoke_mode {
        Ok(talos_native::project::Project::public_results_fixture())
    } else {
        project(args.into_iter())
    } {
        Ok(project) => project,
        Err(message) => {
            complain(&message);
            return std::process::ExitCode::from(2);
        }
    };
    #[cfg(not(target_os = "windows"))]
    if smoke_mode {
        complain("ECharts public smoke requires Windows");
        return std::process::ExitCode::from(2);
    }
    #[cfg(target_os = "windows")]
    let (mut smoke, verdict) = if smoke_mode {
        let Some(path) = std::env::var_os("TALOS_PUBLIC_ECHARTS_REPORT") else {
            complain("Public ECharts smoke requires TALOS_PUBLIC_ECHARTS_REPORT");
            return std::process::ExitCode::from(2);
        };
        match talos_native::charts::PublicSmoke::reserve(std::path::Path::new(&path)) {
            Ok((smoke, receiver)) => (Some(smoke), Some(receiver)),
            Err(error) => {
                complain(&error);
                return std::process::ExitCode::FAILURE;
            }
        }
    } else {
        (None, None)
    };
    if let Some(control) = std::env::var_os("TALOS_PUBLIC_ECHARTS_FORCE_FAILURE")
        && (!smoke_mode || control != "1")
    {
        complain("WebView2 failure control requires --public-echarts-smoke and value 1");
        return std::process::ExitCode::from(2);
    }
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
        Box::new(move |cc| {
            let app = talos_native::app::App::new(
                cc,
                talos_native::loader::LoadMode::Background,
                project,
            );
            #[cfg(target_os = "windows")]
            let mut app = app;
            #[cfg(target_os = "windows")]
            if let Some(smoke) = smoke.take() {
                if let Some(error) = app.chart_error() {
                    return Err(std::io::Error::other(error).into());
                }
                app.attach_public_smoke(smoke);
            }
            Ok(Box::new(app))
        }),
    );
    match result {
        Ok(()) => {
            #[cfg(target_os = "windows")]
            if let Some(receiver) = verdict
                && let Err(error) =
                    talos_native::charts::PublicSmoke::require_pass(receiver.try_recv().ok())
            {
                complain(error);
                return std::process::ExitCode::FAILURE;
            }
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            complain(&error.to_string());
            std::process::ExitCode::FAILURE
        }
    }
}

/// A command-line or startup error, on standard error (the window never opens).
#[cfg(not(target_arch = "wasm32"))]
fn complain(message: &str) {
    use std::io::Write as _;
    // Nothing else can report it if standard error is closed.
    writeln!(std::io::stderr(), "talos-native: {message}").ok();
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
