//! A workspace that is routed but not built yet: it says what it waits for
//! and offers no controls.
use crate::state::Workspace;
use crate::theme::{self, Tokens};

pub fn show(ui: &mut egui::Ui, workspace: Workspace) {
    let tokens = Tokens::of(ui);
    let page = ui.available_rect_before_wrap();
    ui.painter().rect_filled(page, 0.0, tokens.page);
    let size = egui::vec2(540.0_f32.min(page.width() - 32.0), 200.0);
    let rect = egui::Rect::from_center_size(page.center() - egui::vec2(0.0, 40.0), size);
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
        |ui| {
            egui::Frame::new()
                .fill(tokens.card)
                .stroke(egui::Stroke::new(1.0, tokens.border))
                .corner_radius(tokens.card_radius)
                .inner_margin(egui::Margin::symmetric(26, 20))
                .show(ui, |ui| {
                    ui.set_width(size.x - 52.0);
                    ui.vertical_centered(|ui| {
                        crate::icons::show(ui, crate::tabs::icon(workspace), 24.0, tokens.muted);
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(workspace.label())
                                .family(theme::semibold())
                                .size(theme::NAME)
                                .color(tokens.text),
                        );
                        ui.label(
                            egui::RichText::new("Not available in this prototype yet")
                                .color(tokens.muted),
                        );
                        if let Some(reason) = workspace.unavailable_reason() {
                            ui.add_space(6.0);
                            ui.add(
                                egui::Label::new(egui::RichText::new(reason).color(tokens.faint))
                                    .wrap(),
                            );
                        }
                    });
                });
        },
    );
}
