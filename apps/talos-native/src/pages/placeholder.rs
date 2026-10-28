//! A workspace that is routed but not built yet: it says what it waits for
//! and offers no controls.
use crate::state::Workspace;

pub fn show(ui: &mut egui::Ui, workspace: Workspace) {
    let tokens = crate::theme::Tokens::of(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.32);
        ui.heading(workspace.label());
        ui.label(egui::RichText::new("Not available in this prototype yet").color(tokens.muted));
        ui.add_space(6.0);
        if let Some(reason) = workspace.unavailable_reason() {
            ui.set_max_width(520.0);
            ui.label(egui::RichText::new(reason).small().color(tokens.muted));
        }
    });
}
