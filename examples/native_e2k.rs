//! Standalone read-only E2K GPU preview. No project/cache result attachment yet.
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/native_viewport.rs"]
#[expect(
    dead_code,
    reason = "Shared legacy constructor is used by the renderer/dashboard examples; this example supplies source geometry"
)]
mod native;
#[cfg(not(target_arch = "wasm32"))]
mod host {
    use super::native::NativeViewport;
    use std::{
        ffi::OsString,
        fs::OpenOptions,
        io::{Read as _, Seek as _, Write as _},
        path::{Path, PathBuf},
        time::{Duration, Instant},
    };
    const MAX_BYTES: u64 = 64 * 1024 * 1024;
    enum Mode {
        Visible(PathBuf),
        Check(PathBuf),
        PublicSmoke,
    }
    fn mode(args: &[OsString]) -> Result<Mode, &'static str> {
        match args {
            [flag] if flag == "--public-smoke" => Ok(Mode::PublicSmoke),
            [flag, path] if flag == "--check-only" && !path.to_string_lossy().starts_with("--") => {
                Ok(Mode::Check(path.into()))
            }
            [path] if !path.to_string_lossy().starts_with("--") => Ok(Mode::Visible(path.into())),
            _ => Err("Usage: native_e2k <export.e2k> | --check-only <export.e2k> | --public-smoke"),
        }
    }
    fn load(path: &Path) -> Result<lab_e2k_mesh::Mesh, String> {
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("e2k"))
        {
            return Err("An explicit E2K text export is required; EDB is not E2K".into());
        }
        let resolved = path
            .canonicalize()
            .map_err(|_private_error| "Cannot resolve selected E2K")?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            options.share_mode(1);
        }
        let mut file = options
            .open(&resolved)
            .map_err(|_private_error| "Cannot open selected E2K read-only")?;
        let before = file
            .metadata()
            .map_err(|_private_error| "Cannot inspect selected E2K")?;
        if !before.is_file() || before.len() > MAX_BYTES {
            return Err("Selected E2K exceeds the bounded input contract".into());
        }
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut file)
            .take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_private_error| "Cannot read selected E2K")?;
        if bytes.len() as u64 != before.len() {
            return Err("E2K changed or exceeded the input bound".into());
        }
        let mesh = lab_e2k_mesh::build(&bytes, Default::default()).map_err(|e| e.to_string())?;
        let after = file
            .metadata()
            .map_err(|_private_error| "Cannot recheck selected E2K")?;
        if before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
            || path.canonicalize().ok().as_ref() != Some(&resolved)
        {
            return Err("E2K changed during preparation".into());
        }
        file.rewind()
            .map_err(|_private_error| "Cannot recheck selected E2K")?;
        let mut reread = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut reread)
            .map_err(|_private_error| "Cannot recheck selected E2K")?;
        if reread != bytes {
            return Err("E2K changed during preparation".into());
        }
        Ok(mesh)
    }
    fn failure(message: impl Into<String>) -> eframe::Error {
        eframe::Error::AppCreation(Box::new(std::io::Error::other(message.into())))
    }
    pub(super) fn run() -> eframe::Result {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        let mode = mode(&args).map_err(failure)?;
        let public = matches!(mode, Mode::PublicSmoke);
        let mesh = match &mode {
            Mode::Visible(path) | Mode::Check(path) => load(path).map_err(failure)?,
            Mode::PublicSmoke => lab_e2k_mesh::build(
                lab_e2k_mesh::public_fixture::E2K.as_bytes(),
                Default::default(),
            )
            .map_err(|e| failure(e.to_string()))?,
        };
        if matches!(mode, Mode::Check(_)) {
            return Ok(());
        }
        let summary = Summary {
            entities: mesh.entities().len(),
            withheld: mesh
                .entities()
                .iter()
                .filter(|e| e.mode == lab_e2k_mesh::DisplayMode::Withheld)
                .count(),
            projection_skips: mesh.projection_skips().len(),
            stories: mesh.stories().len(),
            cpu_buffer_bytes: mesh.buffer_bytes(),
        };
        let options = eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            depth_buffer: 24,
            persist_window: false,
            viewport: egui::ViewportBuilder::default()
                .with_app_id("talos-read-only-e2k-gpu-preview")
                .with_visible(!public)
                .with_inner_size([1100.0, 760.0])
                .with_min_inner_size([640.0, 480.0]),
            ..Default::default()
        };
        eframe::run_native(
            "TALOS | read-only E2K GPU preview",
            options,
            Box::new(move |cc| {
                Ok(Box::new(App {
                    viewport: NativeViewport::new_with_source(cc, Some(mesh))
                        .map_err(std::io::Error::other)?,
                    summary,
                    smoke: public.then(|| Smoke {
                        started: Instant::now(),
                        done: false,
                    }),
                }))
            }),
        )
    }
    struct Summary {
        entities: usize,
        withheld: usize,
        projection_skips: usize,
        stories: usize,
        cpu_buffer_bytes: usize,
    }
    struct Smoke {
        started: Instant,
        done: bool,
    }
    struct App {
        viewport: NativeViewport,
        summary: Summary,
        smoke: Option<Smoke>,
    }
    impl eframe::App for App {
        fn persist_egui_memory(&self) -> bool {
            false
        }
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            egui::Panel::top("source_e2k_header").show(ui, |ui| {
                ui.heading("Read-only E2K connectivity · display proxies");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Fit").clicked() {
                        self.viewport.fit();
                    }
                    let (v, t) = self.viewport.mesh_counts();
                    ui.label(format!("{} source entities · {} stories · {v} vertices · {t} triangles", self.summary.entities, self.summary.stories));
                });
                ui.label(format!("{} withheld entities · {} source projection notes · {} CPU vertex/index bytes",self.summary.withheld,self.summary.projection_skips,self.summary.cpu_buffer_bytes));
                ui.small("Rectangular frame proxies and zero-thickness supported shells. Assigned sections, local axes, offsets, insertion and opening subtraction are unverified. Unknown/invalid surfaces are withheld.");
                ui.colored_label(egui::Color32::YELLOW,"Geometry snapshot only · analysis/source-model freshness unverified. Cached results and selection are not attached in this slice.");
            });
            egui::CentralPanel::default().show(ui, |ui| self.viewport.show(ui));
            if let Some(smoke) = &mut self.smoke
                && !smoke.done
            {
                let stats = self.viewport.gpu_stats();
                if stats.draw_records >= 3 || smoke.started.elapsed() > Duration::from_secs(20) {
                    smoke.done = true;
                    let pass = stats.draw_records >= 3
                        && stats.uploads == 1
                        && stats.vertex_bytes + stats.index_bytes
                            == self.summary.cpu_buffer_bytes as u64;
                    let report = serde_json::json!({"assessment":if pass{"PASS"}else{"FAIL"},"fixture":"LAB-REAL02-GPU-01-r1","source_entities":self.summary.entities,"vertices":self.viewport.mesh_counts().0,"triangles":self.viewport.mesh_counts().1,"mesh_uploads":stats.uploads,"resident_mesh_buffers":2,"vertex_bytes":stats.vertex_bytes,"index_bytes":stats.index_bytes,"draw_indexed_records":stats.draw_records,"callback_records":self.viewport.callback_paints(),"buffer_creation_cpu_ns":stats.buffer_creation_cpu_ns,"presented_frame_or_fps_verified":false,"physical_input_verified":false,"private_input_used":false,"scope":"Owned hidden public E2K GPU callback smoke only; no presentation, real-model fidelity or lifecycle gate acceptance"});
                    let mut out = std::io::stdout().lock();
                    if serde_json::to_writer(&mut out, &report).is_err()
                        || writeln!(out).and_then(|()| out.flush()).is_err()
                    {
                        drop(writeln!(
                            std::io::stderr().lock(),
                            "Cannot write public GPU smoke receipt"
                        ));
                    }
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                } else {
                    ui.ctx().request_repaint_after(Duration::from_millis(10));
                }
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn public_smoke_cannot_accept_a_user_source() {
            assert!(matches!(
                mode(&["--public-smoke".into()]),
                Ok(Mode::PublicSmoke)
            ));
            assert!(mode(&["--public-smoke".into(), "private.e2k".into()]).is_err());
            assert!(mode(&[]).is_err());
        }
        #[test]
        fn edb_is_refused_before_file_access() {
            assert!(load(Path::new("owned.edb")).is_err());
        }
        #[test]
        fn public_file_is_read_without_changes_and_oversized_source_is_refused() {
            let dir = tempfile::tempdir().expect("owned directory");
            let path = dir.path().join("public.e2k");
            let bytes = lab_e2k_mesh::public_fixture::E2K.as_bytes();
            std::fs::write(&path, bytes).expect("public fixture");
            let before = std::fs::metadata(&path)
                .expect("metadata")
                .modified()
                .expect("mtime");
            let mesh = load(&path).expect("read-only public load");
            assert_eq!(mesh.entities().len(), 3);
            assert_eq!(std::fs::read(&path).expect("reread"), bytes);
            assert_eq!(
                std::fs::metadata(&path)
                    .expect("metadata")
                    .modified()
                    .expect("mtime"),
                before
            );
            let large = dir.path().join("oversized.e2k");
            std::fs::File::create(&large)
                .expect("owned sparse file")
                .set_len(MAX_BYTES + 1)
                .expect("size");
            assert!(load(&large).is_err());
            let cleanup_path = dir.path().to_path_buf();
            dir.close().expect("owned fixtures removed");
            assert!(!cleanup_path.exists());
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    host::run()
}
#[cfg(target_arch = "wasm32")]
fn main() {}
