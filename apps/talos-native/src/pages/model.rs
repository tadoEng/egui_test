//! Model: the native wgpu view fills the canvas; Explorer and Inspector are
//! docked panes or floating cards (`PanelMode`), with the same content.
//!
//! Canvas overlays (view toolbar, axis gizmo, fidelity chip, version strip)
//! are added after the view, so they take clicks before the canvas does;
//! floating cards also block their whole area. Explorer picks and view picks
//! both become the same [`Action::Pick`], and the story highlight is
//! [`Action::SelectStory`], so the shell is the only owner of what is selected.
use crate::document::{self, Document};
use crate::icons::{self, Icon};
use crate::state::{Action, DocumentState, PanelMode, State};
use crate::theme::{self, Tokens};
use crate::view::{ModelView, ViewFrame, ViewIntent};
use lab_e2k_mesh::pick::Scope;
use lab_e2k_mesh::{
    DisplayMode, EntityKey, EntityMesh, GeometryFidelity, ShellPlacement, SourceClass,
};

pub(crate) const MARGIN: f32 = 14.0;
const EXPLORER_WIDTH: f32 = 272.0;
const INSPECTOR_WIDTH: f32 = 336.0;
/// The Inspector's width on a compact window, where it is the only panel
/// (owner, #54 Windows note: ~240–260 pt).
pub(crate) const COMPACT_INSPECTOR_WIDTH: f32 = 256.0;
const STRIP_HEIGHT: f32 = 64.0;
/// Explorer and inspector widths for a page `total` wide: the design widths
/// on a wide window; below the compact width a narrower Explorer and a
/// 256 pt Inspector (one of them at a time).
pub(crate) fn pane_widths(total: f32) -> (f32, f32) {
    let inspector = if total < crate::app::COMPACT_WIDTH {
        COMPACT_INSPECTOR_WIDTH
    } else {
        INSPECTOR_WIDTH.min((total * 0.32).max(260.0))
    };
    (EXPLORER_WIDTH.min((total * 0.25).max(220.0)), inspector)
}

pub fn show(ui: &mut egui::Ui, state: &State, view: &mut dyn ModelView) -> Vec<Action> {
    let mut actions = Vec::new();
    let (Some(document), Some(scope)) = (state.session.ready(), state.session.scope()) else {
        egui::CentralPanel::no_frame().show(ui, |ui| super::not_ready(ui, state));
        return actions;
    };
    let tokens = Tokens::of(ui);
    let model = &state.model;
    let selected = model.selected(scope);
    let docked = model.panels == PanelMode::Docked;
    let total = ui.available_width();
    // Below the compact width only one contextual panel is drawn (owner,
    // #54): the width class is reported to the state, which keeps the user's
    // panel intent apart from what this width can show.
    let compact = total < crate::app::COMPACT_WIDTH;
    if compact != model.compact {
        actions.push(Action::SetCompact(compact));
        ui.ctx().request_repaint();
    }
    let (explorer_width, inspector_width) = pane_widths(total);
    let pane = egui::Frame::new()
        .fill(tokens.chrome)
        .inner_margin(egui::Margin::same(12));
    if docked && model.shows_explorer() {
        egui::Panel::left("model-explorer")
            .frame(pane.stroke(egui::Stroke::new(1.0, tokens.chrome_line)))
            .resizable(true)
            .default_size(explorer_width)
            .size_range(220.0..=(total * 0.25).max(220.0))
            .show(ui, |ui| {
                explorer(ui, document, scope, selected, state, &mut actions);
            });
    }
    if docked && model.shows_inspector() {
        egui::Panel::right("model-inspector")
            .frame(pane.stroke(egui::Stroke::new(1.0, tokens.chrome_line)))
            .resizable(true)
            .default_size(inspector_width)
            .size_range(240.0..=inspector_width.max(240.0))
            .show(ui, |ui| {
                if inspector(
                    ui,
                    document,
                    selected,
                    model.ambiguous,
                    model.compact && model.explorer_open,
                ) {
                    actions.push(Action::ToggleInspector);
                }
            });
    }
    egui::CentralPanel::no_frame().show(ui, |ui| {
        let canvas = ui.available_rect_before_wrap();
        let frame = ViewFrame {
            document,
            scope,
            selected,
            story: model.story.as_deref(),
            picking_allowed: true,
        };
        for ViewIntent::Pick {
            scope,
            entity,
            toggle,
            coincident,
        } in view.show(ui, &frame)
        {
            actions.push(Action::Pick {
                scope,
                entity,
                toggle,
                coincident,
            });
        }
        overlays(
            ui,
            canvas,
            state,
            document,
            scope,
            selected,
            view,
            &mut actions,
        );
    });
    actions
}

/// Where the floating cards and canvas overlays go inside `canvas`.
struct Layout {
    strip: egui::Rect,
    explorer: Option<egui::Rect>,
    inspector: Option<egui::Rect>,
    free: egui::Rect,
}
fn layout(canvas: egui::Rect, state: &State) -> Layout {
    let model = &state.model;
    let floating = model.panels == PanelMode::Floating;
    let strip = egui::Rect::from_min_max(
        egui::pos2(
            canvas.left() + MARGIN,
            canvas.bottom() - MARGIN - STRIP_HEIGHT,
        ),
        egui::pos2(canvas.right() - MARGIN, canvas.bottom() - MARGIN),
    );
    let column_bottom = strip.top() - MARGIN;
    let (explorer_width, inspector_width) = pane_widths(canvas.width());
    let explorer = (floating && model.shows_explorer()).then(|| {
        egui::Rect::from_min_max(
            canvas.left_top() + egui::vec2(MARGIN, MARGIN),
            egui::pos2(canvas.left() + MARGIN + explorer_width, column_bottom),
        )
    });
    let inspector = (floating && model.shows_inspector()).then(|| {
        egui::Rect::from_min_max(
            egui::pos2(
                canvas.right() - MARGIN - inspector_width,
                canvas.top() + MARGIN,
            ),
            egui::pos2(canvas.right() - MARGIN, column_bottom),
        )
    });
    let left = explorer.map_or_else(|| canvas.left(), |r| r.right());
    let right = inspector.map_or_else(|| canvas.right(), |r| r.left());
    let free = egui::Rect::from_min_max(
        egui::pos2(left, canvas.top()),
        egui::pos2(right.max(left + 1.0), column_bottom),
    );
    Layout {
        strip,
        explorer,
        inspector,
        free,
    }
}

/// A child UI confined to `rect`, added after the canvas so it is on top.
/// It senses clicks and drags over what it draws (below its own widgets), so
/// an overlay never lets a click through to pick or orbit the model.
fn place<R>(ui: &mut egui::Ui, rect: egui::Rect, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min))
            .sense(egui::Sense::click_and_drag()),
        add,
    )
    .inner
}

/// A floating card: blocks the canvas under its whole area, then draws
/// `add` inside a rounded, shadowed frame.
fn floating_card(ui: &mut egui::Ui, name: &str, rect: egui::Rect, add: impl FnOnce(&mut egui::Ui)) {
    ui.interact(
        rect,
        egui::Id::unique(("talos-card-block", name)),
        egui::Sense::click_and_drag(),
    );
    let builder = egui::UiBuilder::new()
        .max_rect(rect)
        .layout(egui::Layout::top_down(egui::Align::Min))
        .accessibility_role(egui::accesskit::Role::Pane)
        .accessibility_label(name);
    ui.scope_builder(builder, |ui| {
        theme::card(ui)
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_min_size(rect.size() - egui::vec2(24.0, 24.0));
                ui.set_max_size(rect.size() - egui::vec2(24.0, 24.0));
                add(ui);
            });
    });
}

#[expect(
    clippy::too_many_arguments,
    reason = "one call site; the overlay set is the page"
)]
fn overlays(
    ui: &mut egui::Ui,
    canvas: egui::Rect,
    state: &State,
    document: &Document,
    scope: Scope,
    selected: &[usize],
    view: &mut dyn ModelView,
    actions: &mut Vec<Action>,
) {
    let tokens = Tokens::of(ui);
    let model = &state.model;
    let layout = layout(canvas, state);
    if let Some(rect) = layout.explorer {
        floating_card(ui, "Explorer", rect, |ui| {
            explorer(ui, document, scope, selected, state, actions);
        });
    }
    if let Some(rect) = layout.inspector {
        floating_card(ui, "Inspector", rect, |ui| {
            if inspector(
                ui,
                document,
                selected,
                model.ambiguous,
                model.compact && model.explorer_open,
            ) {
                actions.push(Action::ToggleInspector);
            }
        });
    }
    // Reopen buttons for closed panels, in either mode.
    if !model.shows_explorer() {
        let rect = egui::Rect::from_min_size(
            canvas.left_top() + egui::vec2(MARGIN, MARGIN),
            egui::vec2(40.0, 36.0),
        );
        place(ui, rect, |ui| {
            theme::card(ui)
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    if icons::button(ui, Icon::PanelLeft, "Show explorer (E)", false).clicked() {
                        actions.push(Action::ToggleExplorer);
                    }
                });
        });
    }
    if !model.shows_inspector() {
        let rect = egui::Rect::from_min_size(
            egui::pos2(canvas.right() - MARGIN - 40.0, canvas.top() + MARGIN),
            egui::vec2(40.0, 36.0),
        );
        place(ui, rect, |ui| {
            theme::card(ui)
                .inner_margin(egui::Margin::same(4))
                .show(ui, |ui| {
                    if icons::button(ui, Icon::PanelRight, "Show inspector (I)", false).clicked() {
                        actions.push(Action::ToggleInspector);
                    }
                });
        });
    }
    gizmo(ui, &layout, !model.shows_inspector(), view);
    view_toolbar(ui, &layout, state, view, actions);
    fidelity_chip(ui, &layout, !model.shows_explorer(), document, &tokens);
    version_strip(ui, layout.strip, state, actions);
}

fn view_toolbar(
    ui: &mut egui::Ui,
    layout: &Layout,
    state: &State,
    view: &mut dyn ModelView,
    actions: &mut Vec<Action>,
) {
    let size = egui::vec2(212.0, 40.0);
    let center = egui::pos2(
        layout.free.center().x,
        layout.strip.top() - 10.0 - size.y / 2.0,
    );
    let rect = egui::Rect::from_center_size(center, size);
    let floating = state.model.panels == PanelMode::Floating;
    place(ui, rect, |ui| {
        theme::card(ui)
            .corner_radius(10)
            .inner_margin(egui::Margin::same(5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    if icons::button(ui, Icon::Fit, "Fit the whole model (F)", false).clicked() {
                        view.fit();
                        ui.ctx().request_repaint();
                    }
                    if icons::button(ui, Icon::Layers, "Clear the story highlight", false).clicked()
                        && state.model.story.is_some()
                    {
                        actions.push(Action::SelectStory(None));
                    }
                    ui.separator();
                    if icons::toggle(ui, Icon::PanelLeft, "Docked panels", !floating).clicked() {
                        actions.push(Action::SetPanelMode(PanelMode::Docked));
                    }
                    if icons::toggle(ui, Icon::Floating, "Floating panels", floating).clicked() {
                        actions.push(Action::SetPanelMode(PanelMode::Floating));
                    }
                    ui.separator();
                    icons::show(ui, Icon::Info, 16.0, Tokens::of(ui).muted);
                })
                .response
                .on_hover_text("Drag: orbit · Shift drag or middle drag: pan · Wheel: zoom");
            });
    });
}

/// The camera's axes, from the view's real camera basis. `after_reopen`:
/// the Inspector's reopen button holds the top-right corner, so move left.
fn gizmo(ui: &egui::Ui, layout: &Layout, after_reopen: bool, view: &dyn ModelView) {
    let Some([right, up, _back]) = view.basis() else {
        return;
    };
    let tokens = Tokens::of(ui);
    let inset = if after_reopen { MARGIN + 48.0 } else { MARGIN };
    let center = egui::pos2(
        layout.free.right() - inset - 34.0,
        layout.free.top() + MARGIN + 34.0,
    );
    // Like every overlay, the gizmo is not a pick target for the model under it.
    ui.interact(
        egui::Rect::from_center_size(center, egui::vec2(60.0, 60.0)),
        egui::Id::unique("talos-gizmo"),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter();
    painter.circle(
        center,
        30.0,
        tokens.surface,
        egui::Stroke::new(1.0, tokens.border),
    );
    let axes = [
        ("X", [1.0, 0.0, 0.0], egui::Color32::from_rgb(239, 100, 97)),
        ("Y", [0.0, 1.0, 0.0], egui::Color32::from_rgb(79, 197, 143)),
        ("Z", [0.0, 0.0, 1.0], egui::Color32::from_rgb(106, 165, 255)),
    ];
    for (name, axis, color) in axes {
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let end = center + egui::vec2(dot(axis, right), -dot(axis, up)) * 20.0;
        painter.line_segment([center, end], egui::Stroke::new(2.5, color));
        painter.text(
            end + (end - center).normalized() * 7.0,
            egui::Align2::CENTER_CENTER,
            name,
            egui::FontId::new(10.0, theme::semibold()),
            color,
        );
    }
}

fn fidelity_chip(
    ui: &mut egui::Ui,
    layout: &Layout,
    after_reopen: bool,
    document: &Document,
    tokens: &Tokens,
) {
    let mut counts = [0usize; 5];
    for entity in document.mesh().entities() {
        counts[fidelity_slot(entity)] += 1;
    }
    // Top-left of the free canvas, clear of the gizmo (top-right) and the
    // view toolbar (bottom-centre) in both panel modes.
    let inset = if after_reopen { MARGIN + 48.0 } else { MARGIN };
    let origin = egui::pos2(layout.free.left() + inset, layout.free.top() + MARGIN);
    let rect = egui::Rect::from_min_size(origin, egui::vec2(420.0, 32.0));
    // A narrow canvas (900 pt, floating) keeps the dots and numbers and moves
    // the words into the tooltip and the accessible name, clear of the gizmo.
    let compact = layout.free.width() < 540.0;
    let summary = format!(
        "{} exact, {} resolved, {} approximate, {} symbolic, {} withheld",
        counts[0], counts[1], counts[2], counts[3], counts[4]
    );
    place(ui, rect, |ui| {
        theme::card(ui)
            .corner_radius(255)
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    for (count, label, color) in [
                        (counts[0], "exact", Some(tokens.exact)),
                        (counts[1], "resolved", Some(tokens.resolved)),
                        (counts[2], "approximate", Some(tokens.approx)),
                        (counts[3], "symbolic", Some(tokens.faint)),
                        (counts[4], "withheld", None),
                    ] {
                        if count == 0 && matches!(label, "symbolic") {
                            continue;
                        }
                        let text = if compact && color.is_some() {
                            count.to_string()
                        } else {
                            format!("{count} {label}")
                        };
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            if let Some(color) = color {
                                let (dot, _) = ui.allocate_exact_size(
                                    egui::vec2(7.0, 7.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().circle_filled(dot.center(), 3.5, color);
                            }
                            ui.label(
                                egui::RichText::new(text)
                                    .size(theme::SMALL)
                                    .color(tokens.muted),
                            );
                        });
                    }
                })
                .response
                .on_hover_text(format!(
                    "Backend fidelity, a mirror of the EtabExtension classification: \
                     {summary}. Withheld entities are not drawn."
                ));
            });
    });
}

/// The version strip: every version of the branch as a node; clicking one is
/// the same `SwitchVersion` the header picker sends.
fn version_strip(ui: &mut egui::Ui, rect: egui::Rect, state: &State, actions: &mut Vec<Action>) {
    let tokens = Tokens::of(ui);
    let session = &state.session;
    let requested = session.requested().clone();
    let loading = matches!(session.document, DocumentState::Loading(_));
    let summaries = rect.width() >= 560.0;
    let note = rect.width() >= 1000.0;
    place(ui, rect, |ui| {
        theme::card(ui)
            .inner_margin(egui::Margin::symmetric(16, 0))
            .show(ui, |ui| {
                let row = egui::vec2(rect.width() - 32.0, rect.height());
                ui.allocate_ui_with_layout(
                    row,
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                icons::show(ui, Icon::Commit, 14.0, tokens.muted);
                                ui.label(
                                    egui::RichText::new(&requested.branch)
                                        .family(theme::semibold())
                                        .color(tokens.text),
                                );
                            });
                            if note {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{} synthetic snapshots",
                                        session.project.versions.len()
                                    ))
                                    .size(theme::SMALL)
                                    .color(tokens.muted),
                                );
                            }
                        });
                        ui.add_space(18.0);
                        let mut previous: Option<egui::Rect> = None;
                        for entry in &session.project.versions {
                            let current = entry.id == requested;
                            if previous.is_some() {
                                // A connector between nodes, never under a label.
                                let (gap, _) = ui.allocate_exact_size(
                                    egui::vec2(if note { 56.0 } else { 28.0 }, 16.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().hline(
                                    gap.x_range().shrink(4.0),
                                    gap.center().y,
                                    egui::Stroke::new(2.0, tokens.border),
                                );
                            }
                            let node =
                                version_node(ui, entry, current, loading && current, summaries);
                            if node.clicked() && !current {
                                actions.push(Action::SwitchVersion(entry.id.clone()));
                            }
                            previous = Some(node.rect);
                        }
                        const NOTE: &str =
                            "Synthetic snapshots: no commit history, dates or analysis runs.";
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if note {
                                ui.label(
                                    egui::RichText::new(NOTE)
                                        .size(theme::SMALL)
                                        .color(tokens.faint),
                                );
                            } else {
                                // Too narrow for the sentence: the same words
                                // behind an info icon, named for screen readers.
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(16.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                icons::paint(ui, Icon::Info, rect, tokens.muted);
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::accesskit::Role::Label,
                                        true,
                                        NOTE,
                                    )
                                });
                                response.on_hover_text(NOTE);
                            }
                        });
                    },
                );
            });
    });
}

fn version_node(
    ui: &mut egui::Ui,
    entry: &crate::project::VersionEntry,
    current: bool,
    loading: bool,
    summaries: bool,
) -> egui::Response {
    let tokens = Tokens::of(ui);
    let title = if current {
        format!("{} · viewing", entry.id.version)
    } else {
        entry.id.version.clone()
    };
    let inner = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let (dot, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
        if current {
            ui.painter()
                .circle_filled(dot.center(), 9.0, tokens.accent_weak);
            ui.painter().circle_filled(dot.center(), 6.0, tokens.accent);
        } else {
            ui.painter().circle(
                dot.center(),
                6.0,
                tokens.chrome,
                egui::Stroke::new(2.0, tokens.muted),
            );
        }
        ui.vertical(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(title)
                        .family(theme::semibold())
                        .color(tokens.text),
                )
                .extend(),
            );
            if summaries {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(if loading { "loading…" } else { entry.summary })
                            .size(theme::SMALL)
                            .color(tokens.muted),
                    )
                    .truncate(),
                );
            }
        });
    });
    let node = ui
        .interact(
            inner.response.rect,
            egui::Id::unique(("talos-version-node", &entry.id)),
            egui::Sense::click(),
        )
        .on_hover_text(format!(
            "{}: {} (synthetic snapshot)",
            entry.id, entry.summary
        ));
    node.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::accesskit::Role::Button,
            true,
            current,
            format!("Version {}", entry.id.version),
        )
    });
    node
}

fn fidelity_slot(entity: &EntityMesh) -> usize {
    let tier = entity
        .frame
        .as_ref()
        .map(|f| f.fidelity)
        .or_else(|| entity.shell.as_ref().map(|s| s.fidelity));
    match (entity.mode, tier) {
        (DisplayMode::Withheld, _) => 4,
        (_, Some(GeometryFidelity::Exact)) => 0,
        (_, Some(GeometryFidelity::Resolved)) => 1,
        (_, Some(GeometryFidelity::Approximate)) => 2,
        (_, Some(GeometryFidelity::Symbolic) | None) => 3,
    }
}

/// UI-only Explorer filters (text and class); never part of the selection.
#[derive(Clone, Default)]
struct ExplorerFilter {
    text: String,
    class: Option<SourceClass>,
}

/// "Explorer", the entity count and the hide button; true when hidden.
/// "Explorer", the entity count and its header button; true when clicked.
/// `swap`: a compact window, where the button switches to the Inspector.
fn explorer_title(ui: &mut egui::Ui, count: usize, swap: bool) -> bool {
    let tokens = Tokens::of(ui);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Explorer")
                .family(theme::semibold())
                .size(theme::TITLE)
                .color(tokens.text),
        );
        ui.label(
            egui::RichText::new(count.to_string())
                .size(theme::SMALL)
                .color(tokens.muted),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (icon, label) = if swap {
                (Icon::PanelRight, "Switch to inspector (E)")
            } else {
                (Icon::PanelLeft, "Hide explorer (E)")
            };
            icons::button(ui, icon, label, false).clicked()
        })
        .inner
    })
    .inner
}

fn explorer(
    ui: &mut egui::Ui,
    document: &Document,
    scope: Scope,
    selected: &[usize],
    state: &State,
    actions: &mut Vec<Action>,
) {
    let story = state.model.story.as_deref();
    let entities = document.mesh().entities();
    let filter_id = egui::Id::unique("talos-explorer-filter");
    let mut filter: ExplorerFilter = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
    let swap = state.model.compact && state.model.inspector_open;
    if explorer_title(ui, entities.len(), swap) {
        actions.push(Action::ToggleExplorer);
    }
    ui.add(
        egui::TextEdit::singleline(&mut filter.text)
            .hint_text("Filter by name or section")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    theme::section(ui, "Stories");
    // Whole rows only, so the list never ends on a half-drawn story.
    let step = ROW + ui.spacing().item_spacing.y;
    let fit = (ui.available_height() * 0.32 / step).floor().max(3.0);
    egui::ScrollArea::vertical()
        .id_salt("explorer-stories")
        .max_height(fit * step - ui.spacing().item_spacing.y)
        .show(ui, |ui| {
            for name in document.story_names() {
                let count = entities.iter().filter(|e| e.story == name).count();
                let on = story == Some(name);
                if row(ui, on, None, name, &count.to_string()).clicked() {
                    actions.push(Action::SelectStory((!on).then(|| name.to_owned())));
                }
            }
        });
    ui.add_space(4.0);
    theme::section(ui, "Types");
    type_rows(ui, document, &mut filter);
    ui.add_space(4.0);
    let needle = filter.text.trim().to_lowercase();
    let rows: Vec<usize> = (0..entities.len())
        .filter(|&index| {
            let entity = &entities[index];
            story.is_none_or(|story| entity.story == story)
                && filter.class.is_none_or(|class| entity.class == class)
                && (needle.is_empty() || matches_text(entity, &needle))
        })
        .collect();
    theme::section(
        ui,
        &match story {
            Some(story) => format!("Entities on {story} ({})", rows.len()),
            None => format!("All entities ({})", rows.len()),
        },
    );
    if let Some(story) = story {
        outside_filter(ui, entities, selected, story, actions);
    }
    let footer = 40.0;
    let row_height = ui.spacing().interact_size.y;
    egui::ScrollArea::vertical()
        .id_salt("explorer-entities")
        .auto_shrink(false)
        .max_height((ui.available_height() - footer).max(row_height * 3.0))
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for &index in &rows[range] {
                let entity = &entities[index];
                let mut text = format!(
                    "{}  ·  {}  ·  {}",
                    short_name(&entity.key),
                    class_label(entity.class),
                    entity.story
                );
                if entity.mode == DisplayMode::Withheld {
                    text.push_str("  ·  withheld");
                }
                if ui
                    .selectable_label(selected.contains(&index), text)
                    .clicked()
                {
                    let toggle = ui.input(|input| input.modifiers.command);
                    actions.push(Action::Pick {
                        scope,
                        entity: Some(index),
                        toggle,
                        coincident: 0,
                    });
                }
            }
        });
    ui.separator();
    units(ui, document);
    ui.data_mut(|d| d.insert_temp(filter_id, filter));
}

const ROW: f32 = 26.0;

/// The story filter and the selection are different things: when the
/// selection lies outside the filtered story, say so and offer to move the
/// filter to it (the selection is kept).
fn outside_filter(
    ui: &mut egui::Ui,
    entities: &[EntityMesh],
    selected: &[usize],
    story: &str,
    actions: &mut Vec<Action>,
) {
    let outside: Vec<&EntityMesh> = selected
        .iter()
        .filter_map(|&index| entities.get(index))
        .filter(|entity| entity.story != story)
        .collect();
    let text = match outside.as_slice() {
        [] => return,
        [entity] => format!(
            "Selected {} is on {}, outside the {story} filter.",
            short_name(&entity.key),
            entity.story
        ),
        many => format!(
            "{} selected entities are outside the {story} filter.",
            many.len()
        ),
    };
    let tokens = Tokens::of(ui);
    egui::Frame::new()
        .fill(tokens.accent_weak)
        .corner_radius(tokens.radius)
        .inner_margin(egui::Margin::symmetric(8, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Info, 14.0, tokens.accent);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .size(theme::SMALL)
                            .color(tokens.text),
                    )
                    .wrap(),
                );
            });
            if ui
                .button(egui::RichText::new("Show in Explorer").size(theme::SMALL))
                .on_hover_text("Filter the Explorer to the selection's story; keeps the selection")
                .clicked()
            {
                actions.push(Action::RevealSelection);
            }
        });
}

/// One row per source class present, coloured as drawn; a row is a class
/// filter for the entity list, never a selection.
fn type_rows(ui: &mut egui::Ui, document: &Document, filter: &mut ExplorerFilter) {
    let entities = document.mesh().entities();
    for class in [
        SourceClass::Column,
        SourceClass::Beam,
        SourceClass::Brace,
        SourceClass::OtherFrame,
        SourceClass::FloorShell,
        SourceClass::PanelShell,
        SourceClass::OtherShell,
    ] {
        let members: Vec<_> = entities.iter().filter(|e| e.class == class).collect();
        let Some(first) = members.first() else {
            continue;
        };
        let on = filter.class == Some(class);
        let color = drawn_color(document, first);
        if row(ui, on, color, plural(class), &members.len().to_string()).clicked() {
            filter.class = (!on).then_some(class);
        }
    }
}

/// A list row: optional colour dot, label, right-aligned count.
fn row(
    ui: &mut egui::Ui,
    on: bool,
    dot: Option<egui::Color32>,
    label: &str,
    count: &str,
) -> egui::Response {
    let tokens = Tokens::of(ui);
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, ROW), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::accesskit::Role::Button,
            true,
            on,
            format!("{label}  ·  {count}"),
        )
    });
    if on || response.hovered() {
        let fill = if on {
            tokens.accent_weak
        } else {
            ui.visuals().widgets.hovered.weak_bg_fill
        };
        ui.painter().rect_filled(rect, tokens.radius, fill);
    }
    let mut x = rect.left() + 8.0;
    if let Some(color) = dot {
        ui.painter()
            .circle_filled(egui::pos2(x + 4.0, rect.center().y), 4.0, color);
        x += 16.0;
    }
    ui.painter().text(
        egui::pos2(x, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(theme::BODY),
        tokens.text,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, rect.center().y),
        egui::Align2::RIGHT_CENTER,
        count,
        egui::FontId::proportional(theme::SMALL),
        tokens.faint,
    );
    response
}

/// The colour the renderer actually drew this entity with.
fn drawn_color(document: &Document, entity: &EntityMesh) -> Option<egui::Color32> {
    let mesh = document.mesh();
    let index = *mesh.indices().get(entity.indices.start as usize)?;
    let [r, g, b] = mesh.vertices().get(index as usize)?.color;
    #[expect(clippy::cast_possible_truncation, reason = "clamped to 0..=255 first")]
    let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(egui::Color32::from_rgb(byte(r), byte(g), byte(b)))
}

fn matches_text(entity: &EntityMesh, needle: &str) -> bool {
    let section = match (&entity.frame, &entity.shell) {
        (Some(frame), _) if frame.section.is_some() => frame.section.as_deref(),
        (_, Some(shell)) => shell.section.as_deref(),
        _ => None,
    }
    .unwrap_or_default();
    short_name(&entity.key).to_lowercase().contains(needle)
        || section.to_lowercase().contains(needle)
}

fn units(ui: &mut egui::Ui, document: &Document) {
    let tokens = Tokens::of(ui);
    let source = document.mesh().source_units();
    let [force, length, temperature] = document::unit_names(source);
    let text = if length == "m" {
        format!("Source units {force} · {length} · {temperature}. Lengths shown as declared.")
    } else {
        format!(
            "Source units {force} · {length} · {temperature}. Lengths shown in m, converted at {} m/{length}.",
            source.metres_per_length_unit()
        )
    };
    ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .size(theme::SMALL)
                .color(tokens.muted),
        )
        .wrap(),
    );
}

/// Returns whether the user asked to hide the inspector.
/// Returns true when its header button is clicked: hide, or with `swap`
/// (a compact window) switch to the Explorer.
pub(crate) fn inspector(
    ui: &mut egui::Ui,
    document: &Document,
    selected: &[usize],
    ambiguous: usize,
    swap: bool,
) -> bool {
    let tokens = Tokens::of(ui);
    let entities = document.mesh().entities();
    let chosen: Vec<(usize, &EntityMesh)> = selected
        .iter()
        .filter_map(|&index| entities.get(index).map(|e| (index, e)))
        .collect();
    let close = |ui: &mut egui::Ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            let (icon, label) = if swap {
                (Icon::PanelLeft, "Switch to explorer (I)")
            } else {
                (Icon::Close, "Hide inspector (I)")
            };
            icons::button(ui, icon, label, false)
        })
        .inner
        .clicked()
    };
    let mut closed = false;
    match chosen.as_slice() {
        [] => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Inspector")
                        .family(theme::semibold())
                        .size(theme::TITLE)
                        .color(tokens.text),
                );
                closed = close(ui);
            });
            ui.add_space(6.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "Nothing selected. Click the model or an entity in the explorer.",
                    )
                    .color(tokens.muted),
                )
                .wrap(),
            );
        }
        [(_, entity)] => {
            ui.horizontal(|ui| {
                title(ui, document, entity);
                closed = close(ui);
            });
            egui::ScrollArea::vertical()
                .id_salt("inspector-body")
                .auto_shrink(false)
                .show(ui, |ui| details(ui, document, entity, ambiguous));
        }
        many => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("{} entities selected", many.len()))
                        .family(theme::semibold())
                        .size(theme::TITLE)
                        .color(tokens.text),
                );
                closed = close(ui);
            });
            egui::ScrollArea::vertical()
                .id_salt("inspector-body")
                .auto_shrink(false)
                .show(ui, |ui| {
                    for (index, entity) in many {
                        egui::CollapsingHeader::new(format!(
                            "{}  ·  {}  ·  {}",
                            short_name(&entity.key),
                            class_label(entity.class),
                            entity.story
                        ))
                        .id_salt(("inspector-many", *index))
                        .default_open(false)
                        .show(ui, |ui| details(ui, document, entity, 0));
                    }
                });
        }
    }
    closed
}

fn title(ui: &mut egui::Ui, document: &Document, entity: &EntityMesh) {
    let tokens = Tokens::of(ui);
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(short_name(&entity.key))
                .family(theme::semibold())
                .size(theme::NAME)
                .color(tokens.text),
        );
        ui.label(
            egui::RichText::new(format!(
                "{} · {} · {}",
                class_label(entity.class),
                entity.story,
                document.version()
            ))
            .color(tokens.muted),
        );
    });
}

fn tier(entity: &EntityMesh) -> Option<GeometryFidelity> {
    entity
        .frame
        .as_ref()
        .map(|f| f.fidelity)
        .or_else(|| entity.shell.as_ref().map(|s| s.fidelity))
}

/// A short native-display state for the chip; the full sentence is a row.
fn display_chip(entity: &EntityMesh) -> &'static str {
    if entity.mode == DisplayMode::Withheld {
        return "Withheld · not drawn";
    }
    if let Some(shell) = &entity.shell {
        return match shell.placement {
            ShellPlacement::CentredMiddle => "Body drawn ±t/2",
            ShellPlacement::Withheld => "Withheld · not drawn",
            _ => "Flat ring · body not drawn",
        };
    }
    match &entity.frame {
        Some(frame) if frame.swept() => "Swept D × B",
        Some(_) => "Display proxy",
        None => "Drawn",
    }
}

/// Why what is drawn differs from the source, in words; `None` when it doesn't.
fn callout(entity: &EntityMesh) -> Option<(&'static str, String)> {
    if let Some(limitation) = entity.limitation {
        return Some((
            "Not drawn.",
            format!("This entity is withheld: {limitation:?}."),
        ));
    }
    if let Some(shell) = &entity.shell {
        let insertion = shell.insertion_point.as_deref().unwrap_or("no insertion");
        return match shell.placement {
            ShellPlacement::FlatAtDeclaredFace => Some((
                "Insertion point not represented.",
                format!(
                    "The source declares {insertion}. Which side of the ring the body occupies is \
                     not certified, so TALOS draws the flat face only."
                ),
            )),
            ShellPlacement::FlatInsertionUnproven => Some((
                "Insertion point unproven.",
                "The source declares no recognised insertion point, so the body is not drawn."
                    .to_owned(),
            )),
            ShellPlacement::FlatBodyUnresolved => Some((
                "Thickness not drawn.",
                format!(
                    "The thickness is {:?}; only the ring is drawn.",
                    shell.thickness_source
                ),
            )),
            _ => None,
        };
    }
    match &entity.frame {
        Some(frame) if !frame.swept() => Some((
            "Display proxy.",
            "The section's depth × width did not resolve, so a fixed-size proxy is drawn."
                .to_owned(),
        )),
        _ => None,
    }
}

fn details(ui: &mut egui::Ui, document: &Document, entity: &EntityMesh, ambiguous: usize) {
    let tokens = Tokens::of(ui);
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        let (label, color) = match tier(entity) {
            Some(GeometryFidelity::Exact) => ("Exact", tokens.exact),
            Some(GeometryFidelity::Resolved) => ("Resolved", tokens.resolved),
            Some(GeometryFidelity::Approximate) => ("Approximate", tokens.approx),
            Some(GeometryFidelity::Symbolic) | None => ("Symbolic", tokens.faint),
        };
        theme::chip(ui, Some(color), label).on_hover_text("Backend fidelity tier");
        theme::chip(ui, None, display_chip(entity)).on_hover_text("What TALOS draws");
    });
    let mut notes: Vec<(&str, String)> = callout(entity).into_iter().collect();
    if ambiguous > 0 {
        notes.push((
            "Ambiguous pick.",
            format!(
                "{ambiguous} other entit{} at the same depth: this pick is ambiguous.",
                if ambiguous == 1 { "y" } else { "ies" }
            ),
        ));
    }
    for (head, body) in notes {
        ui.add_space(6.0);
        egui::Frame::new()
            .fill(tokens.approx_weak)
            .stroke(egui::Stroke::new(1.0, tokens.approx.gamma_multiply(0.5)))
            .corner_radius(10)
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let mut job = egui::text::LayoutJob::default();
                job.append(
                    head,
                    0.0,
                    egui::TextFormat::simple(
                        egui::FontId::new(theme::SMALL + 1.0, theme::semibold()),
                        tokens.approx,
                    ),
                );
                job.append(
                    &format!(" {body}"),
                    0.0,
                    egui::TextFormat::simple(
                        egui::FontId::proportional(theme::SMALL + 1.0),
                        tokens.text,
                    ),
                );
                job.wrap.max_width = ui.available_width();
                ui.add(egui::Label::new(job).wrap());
            });
    }
    for (heading, rows) in property_groups(document, entity) {
        ui.add_space(8.0);
        theme::section(ui, heading);
        for (name, value) in rows {
            property(ui, name, &value);
        }
    }
    ui.add_space(4.0);
    if ui
        .add(egui::Button::new(
            egui::RichText::new("Copy source key").size(theme::SMALL),
        ))
        .on_hover_text("Copy the stable source key to the clipboard")
        .clicked()
    {
        ui.ctx().copy_text(entity.key.to_string());
    }
}

/// The inspector's grouped rows: Section, Display and Source, with lengths in
/// metres beside their source-unit value.
fn property_groups(
    document: &Document,
    entity: &EntityMesh,
) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    let units = document.mesh().source_units();
    let length = |value: Option<f64>| {
        value.map_or_else(|| "none".to_owned(), |m| document::length(m, units))
    };
    let text = |value: Option<&String>| value.cloned().unwrap_or_else(|| "none".to_owned());
    let mut groups: Vec<(&str, Vec<(&str, String)>)> = Vec::new();
    if let Some(frame) = &entity.frame {
        groups.push((
            "Section",
            vec![
                ("Section", text(frame.section.as_ref())),
                ("Material", text(frame.material.as_ref())),
                ("Shape", text(frame.shape.as_ref())),
                ("Depth", length(frame.depth_m)),
                ("Width", length(frame.width_m)),
            ],
        ));
        groups.push((
            "Display",
            vec![
                (
                    "Native display",
                    if frame.swept() {
                        "swept from source depth × width"
                    } else {
                        "fixed-size display proxy"
                    }
                    .to_owned(),
                ),
                ("Backend fidelity", fidelity(frame.fidelity, &frame.reasons)),
            ],
        ));
    }
    if let Some(shell) = &entity.shell {
        groups.push((
            "Section",
            vec![
                ("Section", text(shell.section.as_ref())),
                ("Material", text(shell.material.as_ref())),
                ("Thickness", length(shell.thickness_m)),
                ("Insertion", text(shell.insertion_point.as_ref())),
            ],
        ));
        groups.push((
            "Display",
            vec![
                ("Placement", words(&shell.placement)),
                (
                    "Native display",
                    shell.placement.native_display().to_owned(),
                ),
                ("Backend fidelity", fidelity(shell.fidelity, &shell.reasons)),
            ],
        ));
    }
    let mut source = vec![
        ("Source key", entity.key.to_string()),
        ("Version", document.version().to_string()),
    ];
    if let Some(limitation) = entity.limitation {
        source.push(("Withheld because", words(&limitation)));
    }
    groups.push(("Source", source));
    groups
}

/// Below this content width a property stacks its name above its value.
const STACK_BELOW: f32 = 300.0;

/// One inspector property, laid out within the width the panel gives it:
/// a narrow name column and a value that wraps (long identifiers break
/// mid-token rather than widen the panel). Nothing is elided.
fn property(ui: &mut egui::Ui, name: &str, value: &str) {
    let tokens = Tokens::of(ui);
    let width = ui.available_width();
    let name = egui::Label::new(egui::RichText::new(name).color(tokens.muted)).wrap();
    let value = egui::Label::new(egui::RichText::new(value).color(tokens.text)).wrap();
    if width < STACK_BELOW {
        ui.add(name);
        ui.add(value);
        ui.add_space(4.0);
        return;
    }
    let name_width = (width * 0.34).clamp(96.0, 132.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(name_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(name_width);
                ui.add(name);
            },
        );
        ui.add(value);
    });
    // Rows are separated by space, not rules (calm surfaces, few borders).
    ui.add_space(2.0);
}

/// The backend tier and reasons this native viewer mirrors; never upgraded by
/// what the native view draws.
fn fidelity<T: std::fmt::Debug>(tier: GeometryFidelity, reasons: &[T]) -> String {
    let mut text = format!("{tier:?}");
    for (index, reason) in reasons.iter().enumerate() {
        text.push_str(if index == 0 { " · " } else { ", " });
        text.push_str(&words(reason));
    }
    text
}

/// A backend enum name as lower-case words: `LocalAxisRotationUnavailable`
/// reads "local axis rotation unavailable".
fn words(value: &impl std::fmt::Debug) -> String {
    let name = format!("{value:?}");
    let mut out = String::with_capacity(name.len() + 8);
    for (index, c) in name.chars().enumerate() {
        if c.is_uppercase() && index > 0 {
            out.push(' ');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// The source object name for a row; the full stable key is in the inspector.
fn short_name(key: &EntityKey) -> String {
    match key {
        EntityKey::Element { id, .. }
        | EntityKey::Instance { id, .. }
        | EntityKey::Assignment { id, .. } => id.clone(),
        other => other.to_string(),
    }
}

fn class_label(class: SourceClass) -> &'static str {
    match class {
        SourceClass::Column => "Column",
        SourceClass::Beam => "Beam",
        SourceClass::Brace => "Brace",
        SourceClass::OtherFrame => "Other frame",
        SourceClass::FloorShell => "Floor shell",
        SourceClass::PanelShell => "Wall panel",
        SourceClass::OtherShell => "Other shell",
    }
}
fn plural(class: SourceClass) -> &'static str {
    match class {
        SourceClass::Column => "Columns",
        SourceClass::Beam => "Beams",
        SourceClass::Brace => "Braces",
        SourceClass::OtherFrame => "Other frames",
        SourceClass::FloorShell => "Floors",
        SourceClass::PanelShell => "Walls",
        SourceClass::OtherShell => "Other shells",
    }
}
