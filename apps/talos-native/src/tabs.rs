//! The top-level workspace tab strip, inline in the one-row header.
//!
//! In egui's AccessKit tree the strip is a `TabList` and each visible
//! workspace a `Tab` whose `selected` state is the active workspace. While a
//! tab has keyboard focus, Left/Right activate and focus the neighbouring tab
//! (the WAI-ARIA "automatic activation" pattern). At narrow widths the
//! workspaces after Results move into a **More** menu (the active one stays in
//! the strip); Ctrl+1…5 still reach every workspace. These semantics are
//! asserted on the AccessKit tree in headless tests only; how Windows Narrator
//! announces them has not been verified.
use crate::icons::{self, Icon};
use crate::state::Workspace;
use egui::accesskit::Role;

const HEIGHT: f32 = 48.0;

fn tab_id(workspace: Workspace) -> egui::Id {
    egui::Id::unique(("talos-workspace-tab", workspace.label()))
}

pub fn icon(workspace: Workspace) -> Icon {
    match workspace {
        Workspace::Model => Icon::Model,
        Workspace::Results => Icon::Results,
        Workspace::Compare => Icon::Compare,
        Workspace::History => Icon::History,
        Workspace::Reports => Icon::Reports,
    }
}

/// Returns the workspace the user activated this frame, if any. `compact`
/// moves the workspaces after Results into the More menu.
pub fn show(ui: &mut egui::Ui, active: Workspace, compact: bool) -> Option<Workspace> {
    let builder = egui::UiBuilder::new()
        .id_salt("talos-workspace-tabs")
        .layout(egui::Layout::left_to_right(egui::Align::Center))
        .accessibility_role(Role::TabList)
        .accessibility_label("Workspaces");
    ui.scope_builder(builder, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let overflowed = |w: Workspace| {
            compact && !matches!(w, Workspace::Model | Workspace::Results) && w != active
        };
        let mut chosen = None;
        for (index, workspace) in Workspace::ALL.into_iter().enumerate() {
            if overflowed(workspace) {
                continue;
            }
            if let Some(next) = tab(ui, workspace, workspace == active, index + 1) {
                chosen = Some(next);
            }
        }
        if compact {
            let tokens = crate::theme::Tokens::of(ui);
            let more = ui.menu_button(
                egui::RichText::new("More")
                    .family(crate::theme::medium())
                    .color(tokens.muted),
                |ui| {
                    for (index, workspace) in Workspace::ALL.into_iter().enumerate() {
                        if overflowed(workspace)
                            && ui
                                .button(format!("{}    Ctrl+{}", workspace.label(), index + 1))
                                .clicked()
                        {
                            chosen = Some(workspace);
                            ui.close();
                        }
                    }
                },
            );
            more.response.on_hover_text("More workspaces");
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
    let galley = egui::WidgetText::from(egui::RichText::new(label).family(crate::theme::medium()))
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Button,
        );
    let size = egui::vec2(galley.size().x + 24.0 + 22.0, HEIGHT);
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
            rect.shrink2(egui::vec2(2.0, 9.0)),
            tokens.radius,
            visuals.widgets.hovered.weak_bg_fill,
        );
    }
    let color = if selected {
        tokens.text
    } else if workspace.unavailable_reason().is_some() {
        tokens.faint
    } else {
        tokens.muted
    };
    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 12.0 + 8.0, rect.center().y),
        egui::vec2(16.0, 16.0),
    );
    icons::paint(ui, icon(workspace), icon_rect, color);
    ui.painter().galley(
        egui::pos2(
            icon_rect.right() + 7.0,
            rect.center().y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
    if selected {
        let line = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 10.0, rect.bottom() - 2.0),
            egui::pos2(rect.right() - 10.0, rect.bottom()),
        );
        ui.painter().rect_filled(line, 1.0, tokens.accent);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink2(egui::vec2(1.0, 8.0)),
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
