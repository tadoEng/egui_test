//! The boundary between the shell and a model view.
//!
//! APP-01B mounts the existing native wgpu viewport behind [`ModelView`]
//! without changing the shell: the viewport keeps its camera, GPU buffers
//! (keyed by the document's geometry revision) and CPU picking, but selection
//! and document identity stay in [`crate::state::State`]. A pick becomes a
//! [`ViewIntent`] carrying the scope it was made in, and `State::apply`
//! refuses it if the version, generation or geometry has moved on.
use crate::document::Document;
use lab_e2k_mesh::pick::Scope;

/// Everything a view may read for one frame. Borrowed, so a view cannot hold
/// on to a selection or a document beyond the frame it was given them for.
pub struct ViewFrame<'a> {
    pub document: &'a std::sync::Arc<Document>,
    pub scope: Scope,
    pub selected: &'a [usize],
    /// Story to highlight when nothing is selected.
    pub story: Option<&'a str>,
    /// False while source provenance is unverified (e.g. UNLINKED); a view
    /// must then neither hover nor pick.
    pub picking_allowed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewIntent {
    Pick {
        scope: Scope,
        entity: Option<usize>,
        toggle: bool,
        coincident: usize,
    },
}

pub trait ModelView {
    /// Draw into the whole of `ui` and report what the user asked for.
    fn show(&mut self, ui: &mut egui::Ui, frame: &ViewFrame<'_>) -> Vec<ViewIntent>;
    /// Reset the camera to frame the whole model (view state only).
    fn fit(&mut self) {}
    /// The camera's right, up and back axes in world space, for an axis gizmo.
    fn basis(&self) -> Option<[[f32; 3]; 3]> {
        None
    }
    /// For diagnostics and tests that inspect a concrete view.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// APP-01A stand-in: states what will mount here and draws nothing pretending
/// to be geometry.
#[derive(Default)]
pub struct PlaceholderView;
impl ModelView for PlaceholderView {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn show(&mut self, ui: &mut egui::Ui, frame: &ViewFrame<'_>) -> Vec<ViewIntent> {
        let rect = ui.available_rect_before_wrap();
        let tokens = crate::theme::Tokens::of(ui);
        ui.painter()
            .rect_filled(rect, tokens.radius, tokens.viewport_background);
        let layout = egui::Layout::top_down(egui::Align::Center);
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(layout), |ui| {
            ui.add_space(rect.height() * 0.38);
            ui.label(
                egui::RichText::new("3D viewport")
                    .heading()
                    .color(tokens.muted),
            );
            ui.label(
                egui::RichText::new("The native GPU viewport mounts here in APP-01B.")
                    .color(tokens.muted),
            );
            ui.label(
                egui::RichText::new(format!(
                    "{} source entities in {} · select them from the explorer",
                    frame.document.mesh().entities().len(),
                    frame.document.version()
                ))
                .small()
                .color(tokens.muted),
            );
        });
        Vec::new()
    }
}
