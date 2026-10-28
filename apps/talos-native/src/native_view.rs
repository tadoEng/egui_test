//! The native wgpu model view (APP-01B), mounted behind [`ModelView`].
//!
//! Drawing, camera and GPU residency come from `lab-viewport`, the same core the
//! `native_e2k` and workbench examples use. This view owns only presentation
//! state: the camera and a cached highlight index list. Selection, story and
//! document identity stay in [`crate::state::State`]; a click becomes a
//! [`ViewIntent::Pick`] carrying the scope it was made in, and the shell's
//! selection comes back through [`ViewFrame`] to be drawn.
//!
//! The renderer is registered with eframe once (`lab_viewport::install`).
//! Switching tabs only stops paint callbacks; it never recreates pipelines or
//! drops the resident mesh. A version switch uploads the new geometry once,
//! keyed by its digest.
use crate::document::Document;
use crate::view::{ModelView, ViewFrame, ViewIntent};
use eframe::egui_wgpu;
use lab_e2k_mesh::pick::{Hit, Scope};
use lab_viewport::{GpuCounters, GpuGeometry as _, GpuStats, MeshKey, OrbitCamera};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// What the cached highlight was computed for.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HighlightKey {
    scope: Scope,
    selected: Vec<usize>,
    story: Option<String>,
}

pub struct NativeModelView {
    /// `None` when eframe runs without wgpu (headless UI tests): input and
    /// picking still work, nothing is drawn.
    gpu: Option<Arc<GpuCounters>>,
    paints: Arc<AtomicU64>,
    camera: Option<(MeshKey, OrbitCamera)>,
    fit_pending: bool,
    highlight: Option<(HighlightKey, Arc<Vec<u32>>)>,
    last_pick_us: Option<u128>,
    /// Frames this view was laid out in; a hidden tab adds none.
    frames: u64,
}

impl NativeModelView {
    pub fn new(render_state: Option<&egui_wgpu::RenderState>) -> Self {
        Self {
            gpu: render_state.map(lab_viewport::install),
            paints: Arc::new(AtomicU64::new(0)),
            camera: None,
            fit_pending: false,
            highlight: None,
            last_pick_us: None,
            frames: 0,
        }
    }
    /// GPU counters (uploads, draw records); `None` without a renderer.
    pub fn gpu_stats(&self) -> Option<GpuStats> {
        self.gpu.as_ref().map(|gpu| gpu.snapshot())
    }
    /// Paint callbacks recorded, not presented frames.
    pub fn callback_paints(&self) -> u64 {
        self.paints.load(Ordering::Relaxed)
    }
    pub fn frames(&self) -> u64 {
        self.frames
    }
    pub fn camera(&self) -> Option<OrbitCamera> {
        self.camera.map(|(_, camera)| camera)
    }

    /// Fits on first use; on new geometry keeps the view and re-centres on the
    /// new bounds; refits on request.
    fn camera_for(&mut self, document: &Document, aspect: f32) -> OrbitCamera {
        let key = document.key();
        let bounds = document.display_bounds();
        let camera = match self.camera {
            Some((old, camera)) if old == key && !self.fit_pending => camera,
            Some((_, mut camera)) if !self.fit_pending => {
                camera.rebase(bounds);
                camera
            }
            _ => OrbitCamera::fit(bounds, aspect),
        };
        self.fit_pending = false;
        self.camera = Some((key, camera));
        camera
    }

    fn highlight(&mut self, frame: &ViewFrame<'_>) -> Arc<Vec<u32>> {
        let key = HighlightKey {
            scope: frame.scope,
            selected: frame.selected.to_vec(),
            story: frame.story.map(str::to_owned),
        };
        if let Some((cached, indices)) = &self.highlight
            && *cached == key
        {
            return Arc::clone(indices);
        }
        let indices = Arc::new(highlight_indices(frame));
        self.highlight = Some((key, Arc::clone(&indices)));
        indices
    }
}

/// Element selection takes precedence over the story highlight; only attributed
/// index ranges are used, so no vertex is re-sent.
pub(crate) fn highlight_indices(frame: &ViewFrame<'_>) -> Vec<u32> {
    let mesh = frame.document.mesh();
    let entities = mesh.entities();
    let chosen: Vec<_> = if frame.selected.is_empty() {
        frame.story.map_or_else(Vec::new, |story| {
            entities.iter().filter(|e| e.story == story).collect()
        })
    } else {
        frame
            .selected
            .iter()
            .filter_map(|&index| entities.get(index))
            .collect()
    };
    let mut indices = Vec::new();
    for entity in chosen {
        if let Some(range) = mesh
            .indices()
            .get(entity.indices.start as usize..entity.indices.end as usize)
        {
            indices.extend_from_slice(range);
        }
    }
    indices
}

/// Nearest drawn entity under normalized device coordinates (x right, y up).
pub fn pick(document: &Document, camera: OrbitCamera, aspect: f32, ndc: [f32; 2]) -> Option<Hit> {
    let ray = camera.ray(aspect, ndc)?;
    let mesh = document.mesh();
    document.pick_index().pick(
        mesh.vertices(),
        mesh.indices(),
        mesh.entities(),
        &ray,
        |_| true,
    )
}

impl ModelView for NativeModelView {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn fit(&mut self) {
        self.fit_pending = true;
    }

    fn show(&mut self, ui: &mut egui::Ui, frame: &ViewFrame<'_>) -> Vec<ViewIntent> {
        let size = ui.available_size();
        if !size.is_finite() || size.x < 1.0 || size.y < 1.0 {
            return Vec::new();
        }
        self.frames += 1;
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::accesskit::Role::Canvas, true, "3D model view")
        });
        let aspect = rect.width() / rect.height().max(1.0);
        let mut camera = self.camera_for(frame.document, aspect);
        if lab_viewport::navigate(&mut camera, ui, &response, rect.height()) {
            ui.ctx().request_repaint();
        }
        if let Some((_, stored)) = &mut self.camera {
            *stored = camera;
        }

        let mut intents = Vec::new();
        if frame.picking_allowed
            && response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let ndc = [
                (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
            ];
            let started = web_time::Instant::now();
            let hit = pick(frame.document, camera, aspect, ndc);
            self.last_pick_us = Some(started.elapsed().as_micros());
            intents.push(ViewIntent::Pick {
                scope: frame.scope,
                entity: hit.map(|hit| hit.entity),
                toggle: ui.input(|input| input.modifiers.command),
                coincident: hit.map_or(0, |hit| hit.coincident),
            });
        }

        ui.painter()
            .rect_filled(rect, 0.0, lab_viewport::BACKGROUND);
        if self.gpu.is_some() {
            ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                rect,
                lab_viewport::ViewportCallback {
                    mesh: Arc::clone(frame.document) as Arc<dyn lab_viewport::GpuGeometry>,
                    selection: self.highlight(frame),
                    view_projection: camera.view_projection(aspect),
                    paints: Arc::clone(&self.paints),
                },
            ));
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No GPU renderer: the model is not drawn",
                egui::FontId::proportional(14.0),
                egui::Color32::GRAY,
            );
        }
        self.controls(ui, rect);
        intents
    }
}

impl NativeModelView {
    /// A small view toolbar over the bottom of the canvas. Only working
    /// controls are shown.
    fn controls(&mut self, ui: &mut egui::Ui, rect: egui::Rect) {
        let bar = egui::Rect::from_center_size(
            egui::pos2(rect.center().x, rect.bottom() - 28.0),
            egui::vec2(220.0, 32.0),
        );
        let builder = egui::UiBuilder::new()
            .max_rect(bar)
            .layout(egui::Layout::left_to_right(egui::Align::Center));
        ui.scope_builder(builder, |ui| {
            egui::Frame::new()
                .fill(ui.visuals().extreme_bg_color.gamma_multiply(0.92))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(6, 3))
                .show(ui, |ui| {
                    if ui
                        .button("Fit")
                        .on_hover_text("Frame the whole model (F)")
                        .clicked()
                    {
                        self.fit_pending = true;
                        ui.ctx().request_repaint();
                    }
                    ui.label(
                        egui::RichText::new("Drag orbit · Shift drag pan · Wheel zoom")
                            .small()
                            .weak(),
                    );
                });
        });
        if let Some(us) = self.last_pick_us {
            ui.painter().text(
                rect.right_bottom() + egui::vec2(-10.0, -10.0),
                egui::Align2::RIGHT_BOTTOM,
                format!("last pick {us} µs"),
                egui::FontId::proportional(11.0),
                egui::Color32::from_gray(120),
            );
        }
    }
}
