//! Standalone custom-wgpu structural renderer proof.
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/native_viewport.rs"]
mod native;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 24,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([960.0, 680.0])
            .with_min_inner_size([480.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Native wgpu structural core",
        options,
        Box::new(|cc| {
            Ok(Box::new(
                native::NativeViewport::new(cc).map_err(std::io::Error::other)?,
            ))
        }),
    )
}

#[cfg(not(target_arch = "wasm32"))]
impl eframe::App for native::NativeViewport {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("native_renderer_header").show(ui,|ui|{
            ui.horizontal_wrapped(|ui|{
                ui.heading("Native wgpu structural core");
                if ui.button("Fit").clicked(){self.fit();}
                let(vertices,triangles)=self.mesh_counts();
                ui.label(format!("{vertices} vertices · {triangles} triangles"));
                ui.label(format!("{} callback records",self.callback_paints()));
            });
            ui.small("Synthetic fidelity only: frames are rectangular boxes and the shell is a zero-thickness face. Section profiles, insertion offsets, openings and local axes are unsupported.");
        });
        egui::CentralPanel::default().show(ui, |ui| self.show(ui, None, 0));
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {}
