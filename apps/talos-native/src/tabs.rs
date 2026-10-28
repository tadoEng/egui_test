//! The top-level workspace tab strip.
//!
//! In egui's AccessKit tree the strip is a `TabList` and each workspace a
//! `Tab` whose `selected` state is the active workspace. While a tab has
//! keyboard focus, Left/Right activate and focus the neighbouring tab (the
//! WAI-ARIA "automatic activation" pattern). These semantics are asserted on
//! the AccessKit tree in headless tests only; how Windows Narrator or other
//! screen readers announce them has not been verified.
use crate::state::Workspace;
use egui::accesskit::Role;

const HEIGHT: f32 = 34.0;

fn tab_id(workspace: Workspace) -> egui::Id {
    egui::Id::unique(("talos-workspace-tab", workspace.label()))
}

/// Returns the workspace the user activated this frame, if any.
pub fn show(ui: &mut egui::Ui, active: Workspace) -> Option<Workspace> {
    let builder = egui::UiBuilder::new()
        .id_salt("talos-workspace-tabs")
        .layout(egui::Layout::left_to_right(egui::Align::Center))
        .accessibility_role(Role::TabList)
        .accessibility_label("Workspaces");
    ui.scope_builder(builder, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let mut chosen = None;
        for (index, workspace) in Workspace::ALL.into_iter().enumerate() {
            if let Some(next) = tab(ui, workspace, workspace == active, index + 1) {
                chosen = Some(next);
            }
        }
        chosen
    })
    .inner
}

fn tab(
    ui: &mut egui::Ui,
    workspace: Workspace,
    selected: bool,
    number: usize,
) -> Option<Workspace> {
    let tokens = crate::theme::Tokens::of(ui);
    let label = workspace.label();
    let galley = egui::WidgetText::from(label).into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Button,
    );
    let size = egui::vec2(galley.size().x + 28.0, HEIGHT);
    let (_, rect) = ui.allocate_space(size);
    let id = tab_id(workspace);
    let response = ui.interact(rect, id, egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(Role::Tab, true, label));
    ui.ctx()
        .accesskit_node_builder(id, |node| node.set_selected(selected));
    ui.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            id,
            egui::EventFilter {
                horizontal_arrows: true,
                ..Default::default()
            },
        );
    });

    let mut chosen = response.clicked().then_some(workspace);
    if response.has_focus() {
        // Consumed, so the newly focused tab does not see the same press.
        let step = ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                Some(true)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                Some(false)
            } else {
                None
            }
        });
        if let Some(forward) = step {
            let all = Workspace::ALL;
            let at = number - 1;
            let next = all[if forward {
                (at + 1) % all.len()
            } else {
                (at + all.len() - 1) % all.len()
            }];
            ui.memory_mut(|memory| memory.request_focus(tab_id(next)));
            chosen = Some(next);
        }
    }

    let visuals = ui.visuals();
    if response.hovered() && !selected {
        ui.painter().rect_filled(
            rect.shrink(2.0),
            tokens.radius,
            visuals.widgets.hovered.weak_bg_fill,
        );
    }
    let color = if selected {
        visuals.strong_text_color()
    } else if workspace.unavailable_reason().is_some() {
        tokens.muted.gamma_multiply(0.8)
    } else {
        tokens.muted
    };
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, color);
    if selected {
        let line = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 10.0, rect.bottom() - 3.0),
            egui::pos2(rect.right() - 10.0, rect.bottom() - 1.0),
        );
        ui.painter().rect_filled(line, 1.0, tokens.accent);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            tokens.radius,
            visuals.selection.stroke,
            egui::StrokeKind::Inside,
        );
    }
    let hint = workspace.unavailable_reason().map_or_else(
        || format!("Ctrl+{number}"),
        |_| format!("Ctrl+{number} · not available in this prototype yet"),
    );
    response.on_hover_text(hint);
    chosen
}
