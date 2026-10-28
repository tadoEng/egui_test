//! Workspace pages. Each reads [`crate::state::State`] and returns actions; none
//! mutates state directly, and each owns its own layout.
pub mod model;
pub mod placeholder;
pub mod results;

use crate::state::{DocumentState, State};

/// Shown instead of any version-scoped content while the requested version is
/// not loaded, so nothing from the previous version is ever on screen.
pub(crate) fn not_ready(ui: &mut egui::Ui, state: &State) {
    let tokens = crate::theme::Tokens::of(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.38);
        match &state.session.document {
            DocumentState::Loading(ticket) => {
                ui.spinner();
                ui.label(format!("Loading {}…", ticket.version));
            }
            DocumentState::Failed { ticket, .. } => {
                ui.label(
                    egui::RichText::new(format!("{} is not available.", ticket.version))
                        .color(tokens.warning_text),
                );
            }
            DocumentState::Ready { .. } => {}
        }
    });
}
