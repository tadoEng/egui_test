//! Headless CPU stress measurement. Run optimized; does not measure GPU/FPS.
#[cfg(not(target_arch = "wasm32"))]
#[expect(
    clippy::print_stdout,
    reason = "The benchmark's intended output is CSV on stdout"
)]
mod native {
    use eframe_template::{SyntheticPreset, TemplateApp};
    use egui_kittest::Harness;
    use std::time::Instant;

    fn measure(app: TemplateApp, label: &str) {
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1200.0, 760.0))
            .with_max_steps(32)
            .build_eframe(|_| app);
        let mut ui = Vec::new();
        let mut tessellation = Vec::new();
        let mut triangles = 0;
        harness.hover_at(egui::pos2(600.0, 300.0));
        harness.drag_at(egui::pos2(600.0, 300.0));
        harness.step();
        for frame in 0..33 {
            harness.hover_at(egui::pos2(600.0 + frame as f32, 300.0 + frame as f32 * 0.5));
            harness.step();
            let started = Instant::now();
            let meshes = harness.ctx.tessellate(
                harness.output().shapes.clone(),
                harness.output().pixels_per_point,
            );
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            if frame >= 3 {
                ui.push(harness.state().statistics().ui_ms);
                tessellation.push(elapsed);
            }
            triangles = meshes
                .iter()
                .map(|m| match &m.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => mesh.indices.len() / 3,
                    egui::epaint::Primitive::Callback(_) => 0,
                })
                .sum::<usize>();
        }
        ui.sort_by(f64::total_cmp);
        tessellation.sort_by(f64::total_cmp);
        let stats = harness.state().statistics();
        println!(
            "{label},{},{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{triangles}",
            stats.members,
            stats.frames,
            stats.shells,
            stats.drawn_segments,
            stats.skipped,
            stats.warnings,
            stats.build_ms,
            ui[15],
            ui[28],
            tessellation[15],
            tessellation[28]
        );
    }

    pub fn run() -> Result<(), String> {
        println!(
            "source,entities,frames,shells,segments,scene_notes,parse_warnings,build_ms,ui_median_ms,ui_p95_ms,tessellation_median_ms,tessellation_p95_ms,triangles"
        );
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.is_empty() {
            for preset in [
                SyntheticPreset::Demo,
                SyntheticPreset::FiftyThousand,
                SyntheticPreset::HundredThousand,
            ] {
                measure(
                    TemplateApp::with_preset(preset),
                    &format!("synthetic_{}", preset.count()),
                );
            }
        } else {
            #[cfg(feature = "e2k")]
            if args.len() == 2 && args[0] == "--e2k" {
                measure(TemplateApp::from_e2k(&args[1])?, "local_e2k");
                return Ok(());
            }
            return Err("Usage: stress [--e2k <local-file>] (E2K requires --features e2k)".into());
        }
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), String> {
    native::run()
}

#[cfg(target_arch = "wasm32")]
fn main() {}
