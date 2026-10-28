//! Model: an immersive view with an optional explorer and inspector.
//!
//! The native GPU view fills the page. Explorer picks and view picks both
//! become the same [`Action::Pick`], and the story highlight is
//! [`Action::SelectStory`], so the shell is the only owner of what is selected.
use crate::document::{self, Document};
use crate::state::{Action, State};
use crate::view::{ModelView, ViewFrame, ViewIntent};
use lab_e2k_mesh::pick::Scope;
use lab_e2k_mesh::{DisplayMode, EntityKey, EntityMesh, GeometryFidelity, SourceClass};

pub fn show(ui: &mut egui::Ui, state: &State, view: &mut dyn ModelView) -> Vec<Action> {
    let mut actions = Vec::new();
    egui::Panel::top("model-toolbar")
        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 6)))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(state.model.explorer_open, "Explorer")
                    .on_hover_text("Show or hide the source explorer (E)")
                    .clicked()
                {
                    actions.push(Action::ToggleExplorer);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .selectable_label(state.model.inspector_open, "Inspector")
                        .on_hover_text("Show or hide the inspector (I)")
                        .clicked()
                    {
                        actions.push(Action::ToggleInspector);
                    }
                });
            });
        });
    let (Some(document), Some(scope)) = (state.session.ready(), state.session.scope()) else {
        egui::CentralPanel::no_frame().show(ui, |ui| super::not_ready(ui, state));
        return actions;
    };
    let selected = state.model.selected(scope);
    egui::Panel::bottom("model-footer")
        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 4)))
        .show(ui, |ui| footer(ui, document));
    if state.model.explorer_open {
        egui::Panel::left("model-explorer")
            .resizable(true)
            .default_size(240.0)
            .size_range(180.0..=420.0)
            .show(ui, |ui| {
                explorer(
                    ui,
                    document,
                    scope,
                    selected,
                    state.model.story.as_deref(),
                    &mut actions,
                );
            });
    }
    if state.model.inspector_open {
        egui::Panel::right("model-inspector")
            .resizable(true)
            .default_size(320.0)
            .size_range(240.0..=520.0)
            .show(ui, |ui| {
                inspector(ui, document, selected, state.model.ambiguous)
            });
    }
    egui::CentralPanel::no_frame().show(ui, |ui| {
        let frame = ViewFrame {
            document,
            scope,
            selected,
            story: state.model.story.as_deref(),
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
    });
    actions
}

fn explorer(
    ui: &mut egui::Ui,
    document: &Document,
    scope: Scope,
    selected: &[usize],
    story: Option<&str>,
    actions: &mut Vec<Action>,
) {
    let tokens = crate::theme::Tokens::of(ui);
    ui.add_space(6.0);
    ui.strong("Explorer");
    ui.label(
        egui::RichText::new("Click a story to highlight it · Ctrl+click adds an entity")
            .small()
            .color(tokens.muted),
    );
    ui.separator();
    let entities = document.mesh().entities();
    egui::CollapsingHeader::new("Stories")
        .default_open(true)
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("explorer-stories")
                .max_height(ui.available_height() * 0.4)
                .show(ui, |ui| {
                    for name in document.story_names() {
                        let count = entities.iter().filter(|e| e.story == name).count();
                        let on = story == Some(name);
                        if ui
                            .selectable_label(on, format!("{name}  ·  {count}"))
                            .clicked()
                        {
                            actions.push(Action::SelectStory((!on).then(|| name.to_owned())));
                        }
                    }
                });
        });
    let rows: Vec<usize> = (0..entities.len())
        .filter(|&index| story.is_none_or(|story| entities[index].story == story))
        .collect();
    ui.label(
        egui::RichText::new(match story {
            Some(story) => format!("Entities on {story} ({})", rows.len()),
            None => format!("All entities ({})", rows.len()),
        })
        .small()
        .color(tokens.muted),
    );
    let row_height = ui.spacing().interact_size.y;
    egui::ScrollArea::vertical()
        .id_salt("explorer-entities")
        .auto_shrink(false)
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
                let response = ui.selectable_label(selected.contains(&index), text);
                if response.clicked() {
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
}

pub(crate) fn inspector(
    ui: &mut egui::Ui,
    document: &Document,
    selected: &[usize],
    ambiguous: usize,
) {
    let tokens = crate::theme::Tokens::of(ui);
    ui.add_space(6.0);
    ui.strong("Inspector");
    ui.separator();
    if selected.is_empty() {
        ui.label(
            egui::RichText::new("Nothing selected. Click the model or an entity in the explorer.")
                .color(tokens.muted),
        );
        return;
    }
    if selected.len() > 1 {
        ui.label(format!("{} entities selected", selected.len()));
    }
    if ambiguous > 0 {
        ui.label(
            egui::RichText::new(format!(
                "{ambiguous} other entit{} at the same depth: this pick is ambiguous.",
                if ambiguous == 1 { "y" } else { "ies" }
            ))
            .color(tokens.warning_text),
        );
    }
    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for &index in selected {
                if let Some(entity) = document.mesh().entities().get(index) {
                    details(ui, document, entity);
                    ui.add_space(8.0);
                }
            }
        });
}

fn details(ui: &mut egui::Ui, document: &Document, entity: &EntityMesh) {
    let units = document.mesh().source_units();
    let length = |value: Option<f64>| {
        value.map_or_else(|| "none".to_owned(), |m| document::length(m, units))
    };
    let text = |value: Option<&String>| value.cloned().unwrap_or_else(|| "none".to_owned());
    let mut rows: Vec<(&str, String)> = vec![
        ("Source key", entity.key.to_string()),
        ("Story", entity.story.clone()),
        ("Class", class_label(entity.class).to_owned()),
        ("Version", document.version().to_string()),
    ];
    if let Some(frame) = &entity.frame {
        rows.extend([
            ("Section", text(frame.section.as_ref())),
            ("Material", text(frame.material.as_ref())),
            ("Shape", text(frame.shape.as_ref())),
            ("Depth", length(frame.depth_m)),
            ("Width", length(frame.width_m)),
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
        ]);
    }
    if let Some(shell) = &entity.shell {
        rows.extend([
            ("Section", text(shell.section.as_ref())),
            ("Material", text(shell.material.as_ref())),
            ("Thickness", length(shell.thickness_m)),
            ("Insertion", text(shell.insertion_point.as_ref())),
            ("Placement", format!("{:?}", shell.placement)),
            (
                "Native display",
                shell.placement.native_display().to_owned(),
            ),
            ("Backend fidelity", fidelity(shell.fidelity, &shell.reasons)),
        ]);
    }
    if let Some(limitation) = entity.limitation {
        rows.push(("Withheld because", format!("{limitation:?}")));
    }
    for (name, value) in rows {
        property(ui, name, &value);
    }
}

/// Below this content width a property stacks its name above its value.
const STACK_BELOW: f32 = 300.0;

/// One inspector property, laid out within the width the panel gives it:
/// a narrow name column and a value that wraps (long identifiers break
/// mid-token rather than widen the panel). Nothing is elided.
fn property(ui: &mut egui::Ui, name: &str, value: &str) {
    let tokens = crate::theme::Tokens::of(ui);
    let width = ui.available_width();
    let name = egui::Label::new(egui::RichText::new(name).small().color(tokens.muted)).wrap();
    let value = egui::Label::new(value).wrap();
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
}

/// The backend tier and reasons this native viewer mirrors; never upgraded by
/// what the native view draws.
fn fidelity<T: std::fmt::Debug>(tier: GeometryFidelity, reasons: &[T]) -> String {
    if reasons.is_empty() {
        format!("{tier:?}")
    } else {
        format!("{tier:?} {reasons:?}")
    }
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

fn footer(ui: &mut egui::Ui, document: &Document) {
    let tokens = crate::theme::Tokens::of(ui);
    let units = document.mesh().source_units();
    let [force, length, temperature] = document::unit_names(units);
    let conversion = if length == "m" {
        "lengths shown in metres as declared".to_owned()
    } else {
        format!(
            "lengths shown in metres, converted at {} m/{length}",
            units.metres_per_length_unit()
        )
    };
    let mut counts = [0usize; 5];
    for entity in document.mesh().entities() {
        let tier = entity
            .frame
            .as_ref()
            .map(|f| f.fidelity)
            .or_else(|| entity.shell.as_ref().map(|s| s.fidelity));
        let slot = match (entity.mode, tier) {
            (DisplayMode::Withheld, _) => 4,
            (_, Some(GeometryFidelity::Exact)) => 0,
            (_, Some(GeometryFidelity::Resolved)) => 1,
            (_, Some(GeometryFidelity::Approximate)) => 2,
            (_, Some(GeometryFidelity::Symbolic) | None) => 3,
        };
        counts[slot] += 1;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "Source units {force} · {length} · {temperature} — {conversion}"
            ))
            .small()
            .color(tokens.muted),
        );
        ui.separator();
        ui.label(
            egui::RichText::new(format!(
                "Backend fidelity: {} exact · {} resolved · {} approximate · {} symbolic · {} withheld",
                counts[0], counts[1], counts[2], counts[3], counts[4]
            ))
            .small()
            .color(tokens.muted),
        )
        .on_hover_text(
            "Mirror of the EtabExtension backend classification; withheld entities are not drawn.",
        );
    });
}
