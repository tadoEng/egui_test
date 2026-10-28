//! Results: chart-first, with contextual selectors and no 3D viewport.
//!
//! The selectors edit a typed [`ResultsQuery`]. No value is shown unless an
//! analyzed result set for exactly the query's version answers it; the
//! synthetic demo has none, so the page says so instead of plotting anything.
use crate::document::Document;
use crate::state::{Action, Direction, Metric, ResultsQuery, State, StoryRange};

pub fn show(ui: &mut egui::Ui, state: &State) -> Vec<Action> {
    let mut actions = Vec::new();
    let Some(document) = state.session.ready() else {
        egui::CentralPanel::no_frame().show(ui, |ui| super::not_ready(ui, state));
        return actions;
    };
    let query = &state.results.query;
    egui::Panel::top("results-selectors")
        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 8)))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| selectors(ui, document, query, &mut actions));
            if let Some(notice) = &state.results.notice {
                let tokens = crate::theme::Tokens::of(ui);
                ui.label(
                    egui::RichText::new(notice)
                        .small()
                        .color(tokens.warning_text),
                );
            }
        });
    egui::CentralPanel::no_frame().show(ui, |ui| chart_area(ui, document, query));
    actions
}

fn selectors(
    ui: &mut egui::Ui,
    document: &Document,
    query: &ResultsQuery,
    actions: &mut Vec<Action>,
) {
    egui::ComboBox::from_label("Metric")
        .selected_text(query.metric.label())
        .show_ui(ui, |ui| {
            for metric in Metric::ALL {
                if ui
                    .selectable_label(query.metric == metric, metric.label())
                    .clicked()
                {
                    actions.push(Action::SetMetric(metric));
                }
            }
        });
    ui.separator();
    for (direction, label) in [(Direction::X, "X"), (Direction::Y, "Y")] {
        if ui
            .selectable_label(query.direction == direction, label)
            .on_hover_text(format!("Direction {label}"))
            .clicked()
        {
            actions.push(Action::SetDirection(direction));
        }
    }
    ui.separator();
    ui.add_enabled_ui(false, |ui| {
        egui::ComboBox::from_label("Case")
            .selected_text("No analyzed cases")
            .show_ui(ui, |_| {});
    })
    .response
    .on_disabled_hover_text(
        "Load cases and combinations come from an analyzed result set. This version has none.",
    );
    ui.separator();
    stories(ui, document, query.stories.as_ref(), actions);
    ui.separator();
    ui.label(format!("for {}", document.version()))
        .on_hover_text("Results follow the version selected in the header.");
}

/// Story names from top to bottom by elevation.
fn ordered_stories(document: &Document) -> Vec<&str> {
    let mut stories: Vec<_> = document.mesh().stories().iter().collect();
    stories.sort_by(|a, b| b.elevation_m.total_cmp(&a.elevation_m));
    stories
        .into_iter()
        .map(|story| story.name.as_str())
        .collect()
}

fn stories(
    ui: &mut egui::Ui,
    document: &Document,
    range: Option<&StoryRange>,
    actions: &mut Vec<Action>,
) {
    let names = ordered_stories(document);
    let (Some(first), Some(last)) = (names.first(), names.last()) else {
        ui.label("No stories");
        return;
    };
    let (top, bottom) = range.map_or((*first, *last), |r| (r.top.as_str(), r.bottom.as_str()));
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap_or(0);
    let mut pick = |ui: &mut egui::Ui, label: &str, current: &str, is_top: bool| {
        egui::ComboBox::from_label(label)
            .selected_text(current)
            .show_ui(ui, |ui| {
                for name in &names {
                    if ui.selectable_label(*name == current, *name).clicked() {
                        // Keep the range ordered: moving one end past the other moves both.
                        let (new_top, new_bottom) = if is_top {
                            (*name, if at(name) > at(bottom) { *name } else { bottom })
                        } else {
                            (if at(name) < at(top) { *name } else { top }, *name)
                        };
                        let whole = new_top == *first && new_bottom == *last;
                        actions.push(Action::SetStoryRange((!whole).then(|| StoryRange {
                            top: new_top.to_owned(),
                            bottom: new_bottom.to_owned(),
                        })));
                    }
                }
            });
    };
    pick(ui, "From", top, true);
    pick(ui, "to", bottom, false);
}

fn chart_area(ui: &mut egui::Ui, document: &Document, query: &ResultsQuery) {
    let tokens = crate::theme::Tokens::of(ui);
    let rect = ui.available_rect_before_wrap().shrink(12.0);
    ui.painter()
        .rect_filled(rect, tokens.radius, tokens.viewport_background);
    let layout = egui::Layout::top_down(egui::Align::Center);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(layout), |ui| {
        ui.add_space(rect.height() * 0.36);
        ui.label(
            egui::RichText::new(format!("No analyzed results for {}", document.version()))
                .heading(),
        );
        ui.label(
            egui::RichText::new(
                "This synthetic snapshot has not been analyzed, so there is nothing to plot. \
                 The chart surface mounts here in APP-01C.",
            )
            .color(tokens.muted),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(format!("Query: {}", describe(query)))
                .small()
                .monospace()
                .color(tokens.muted),
        );
    });
}

/// The resolved query, e.g. `Story drift · X · no case · all stories · main/v2`.
pub fn describe(query: &ResultsQuery) -> String {
    let direction = match query.direction {
        Direction::X => "X",
        Direction::Y => "Y",
    };
    let stories = query.stories.as_ref().map_or_else(
        || "all stories".to_owned(),
        |range| format!("{}–{}", range.top, range.bottom),
    );
    let version = query
        .version
        .as_ref()
        .map_or_else(|| "no version".to_owned(), ToString::to_string);
    format!(
        "{} · {direction} · {} · {stories} · {version}",
        query.metric.label(),
        query.case.as_deref().unwrap_or("no case"),
    )
}
