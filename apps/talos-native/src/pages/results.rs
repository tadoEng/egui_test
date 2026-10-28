//! Results: chart-first, with contextual selectors and no 3D viewport.
//!
//! The selectors edit a typed [`ResultsQuery`]. No value is shown unless an
//! analyzed result set for exactly the query's version answers it; the
//! synthetic demo has none, so the chart card draws only the real story axes
//! and says so instead of plotting anything.
use crate::document::Document;
use crate::icons::{self, Icon};
use crate::state::{Action, Direction, Metric, ResultsQuery, State, StoryRange};
use crate::theme::{self, Tokens};

pub fn show(ui: &mut egui::Ui, state: &State) -> Vec<Action> {
    let mut actions = Vec::new();
    let tokens = Tokens::of(ui);
    let Some(document) = state.session.ready() else {
        egui::CentralPanel::no_frame().show(ui, |ui| super::not_ready(ui, state));
        return actions;
    };
    let query = &state.results.query;
    egui::CentralPanel::default()
        .frame(
            egui::Frame::new()
                .fill(tokens.page)
                .inner_margin(egui::Margin::symmetric(22, 18)),
        )
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| selectors(ui, document, query, &mut actions));
            if let Some(notice) = &state.results.notice {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    icons::show(ui, Icon::Warning, 14.0, tokens.warning_text);
                    ui.label(
                        egui::RichText::new(notice)
                            .size(theme::SMALL)
                            .color(tokens.warning_text),
                    );
                });
            }
            ui.add_space(12.0);
            chart_card(ui, document, query);
        });
    actions
}

/// A selector group: a muted label before its control, in one bordered pill.
fn group<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let tokens = Tokens::of(ui);
    egui::Frame::new()
        .fill(tokens.card)
        .corner_radius(10)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).color(tokens.muted));
                add(ui)
            })
            .inner
        })
        .inner
}

fn selectors(
    ui: &mut egui::Ui,
    document: &Document,
    query: &ResultsQuery,
    actions: &mut Vec<Action>,
) {
    let tokens = Tokens::of(ui);
    group(ui, "Metric", |ui| {
        let combo = egui::ComboBox::from_id_salt("results-metric")
            .selected_text(egui::RichText::new(query.metric.label()).family(theme::semibold()))
            .show_ui(ui, |ui| {
                for metric in Metric::ALL {
                    if ui
                        .selectable_label(query.metric == metric, metric.label())
                        .clicked()
                    {
                        actions.push(Action::SetMetric(metric));
                    }
                }
            })
            .response;
        ui.ctx()
            .accesskit_node_builder(combo.id, |node| node.set_label("Metric"));
    });
    group(ui, "Direction", |ui| {
        for (direction, label) in [(Direction::X, "X"), (Direction::Y, "Y")] {
            if ui
                .selectable_label(query.direction == direction, label)
                .on_hover_text(format!("Direction {label}"))
                .clicked()
            {
                actions.push(Action::SetDirection(direction));
            }
        }
    });
    group(ui, "Case", |ui| {
        ui.add_enabled_ui(false, |ui| {
            let combo = egui::ComboBox::from_id_salt("results-case")
                .selected_text("No analyzed cases")
                .show_ui(ui, |_| {})
                .response;
            ui.ctx()
                .accesskit_node_builder(combo.id, |node| node.set_label("Case"));
        })
        .response
        .on_disabled_hover_text(
            "Load cases and combinations come from an analyzed result set. This version has none.",
        );
    });
    group(ui, "Stories", |ui| {
        stories(ui, document, query.stories.as_ref(), actions);
    });
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        theme::pill(
            ui,
            "Synthetic",
            tokens.synthetic,
            tokens.synthetic_weak,
            "A public synthetic snapshot, not an analyzed EtabExtension version.",
        );
        ui.label(
            egui::RichText::new(document.version().to_string())
                .family(theme::semibold())
                .color(tokens.text),
        )
        .on_hover_text("Results follow the version selected in the header.");
        ui.label(egui::RichText::new("for").color(tokens.muted));
    });
}

/// Stories from top to bottom by elevation, with their elevations in metres.
fn ordered_stories(document: &Document) -> Vec<(&str, f64)> {
    let mut stories: Vec<_> = document.mesh().stories().iter().collect();
    stories.sort_by(|a, b| b.elevation_m.total_cmp(&a.elevation_m));
    stories
        .into_iter()
        .map(|story| (story.name.as_str(), story.elevation_m))
        .collect()
}

fn stories(
    ui: &mut egui::Ui,
    document: &Document,
    range: Option<&StoryRange>,
    actions: &mut Vec<Action>,
) {
    let names: Vec<&str> = ordered_stories(document)
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    let (Some(first), Some(last)) = (names.first(), names.last()) else {
        ui.label("No stories");
        return;
    };
    let (top, bottom) = range.map_or((*first, *last), |r| (r.top.as_str(), r.bottom.as_str()));
    let at = |name: &str| names.iter().position(|n| *n == name).unwrap_or(0);
    let mut pick = |ui: &mut egui::Ui, id: &str, label: &str, current: &str, is_top: bool| {
        let combo = egui::ComboBox::from_id_salt(id)
            .selected_text(egui::RichText::new(current).family(theme::semibold()))
            .width(64.0)
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
            })
            .response;
        ui.ctx()
            .accesskit_node_builder(combo.id, |node| node.set_label(label.to_owned()));
    };
    pick(ui, "results-story-top", "From", top, true);
    ui.label(egui::RichText::new("→").color(Tokens::of(ui).muted));
    pick(ui, "results-story-bottom", "to", bottom, false);
}

fn chart_card(ui: &mut egui::Ui, document: &Document, query: &ResultsQuery) {
    let tokens = Tokens::of(ui);
    egui::Frame::new()
        .fill(tokens.card)
        .corner_radius(tokens.card_radius)
        .inner_margin(egui::Margin::symmetric(22, 18))
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            let direction = match query.direction {
                Direction::X => "X",
                Direction::Y => "Y",
            };
            ui.label(
                egui::RichText::new(format!("{} · {direction}", query.metric.label()))
                    .family(theme::semibold())
                    .size(theme::TITLE)
                    .color(tokens.text),
            );
            let stories = ordered_stories(document);
            ui.label(
                egui::RichText::new(format!(
                    "{} · {} stories from the source",
                    document.version(),
                    stories.len()
                ))
                .color(tokens.muted),
            );
            let plot = ui.available_rect_before_wrap();
            axes(ui, plot, query, &stories, &tokens);
            empty_state(ui, plot, document, query);
        });
}

/// Story axes at real elevations; no data line, no ticks, no values.
fn axes(
    ui: &egui::Ui,
    plot: egui::Rect,
    query: &ResultsQuery,
    stories: &[(&str, f64)],
    tokens: &Tokens,
) {
    let painter = ui.painter_at(plot);
    let (left, right) = (plot.left() + 64.0, plot.right() - 8.0);
    let (top, bottom) = (plot.top() + 16.0, plot.bottom() - 40.0);
    let shown: Vec<_> = stories
        .iter()
        .filter(|(name, _)| {
            query.stories.as_ref().is_none_or(|range| {
                let index = |n: &str| stories.iter().position(|(s, _)| *s == n);
                let at = index(name);
                at >= index(&range.top) && at <= index(&range.bottom)
            })
        })
        .collect();
    let (high, low) = shown.iter().fold((f64::MIN, f64::MAX), |(h, l), (_, e)| {
        (h.max(*e), l.min(*e))
    });
    let span = (high - low).max(1e-9);
    #[expect(clippy::cast_possible_truncation, reason = "screen coordinates")]
    let y = |elevation: f64| bottom - ((elevation - low) / span) as f32 * (bottom - top);
    for (name, elevation) in &shown {
        let at = y(*elevation);
        painter.hline(left..=right, at, egui::Stroke::new(1.0, tokens.border));
        painter.text(
            egui::pos2(left - 12.0, at),
            egui::Align2::RIGHT_CENTER,
            *name,
            egui::FontId::proportional(theme::SMALL),
            tokens.muted,
        );
    }
    painter.vline(left, top..=bottom, egui::Stroke::new(1.0, tokens.muted));
    painter.text(
        egui::pos2(f32::midpoint(left, right), bottom + 24.0),
        egui::Align2::CENTER_CENTER,
        query.metric.label(),
        egui::FontId::proportional(theme::SMALL),
        tokens.muted,
    );
}

fn empty_state(ui: &mut egui::Ui, plot: egui::Rect, document: &Document, query: &ResultsQuery) {
    let tokens = Tokens::of(ui);
    let size = egui::vec2(520.0_f32.min(plot.width() - 32.0), 190.0);
    let rect = egui::Rect::from_center_size(plot.center() - egui::vec2(0.0, 20.0), size);
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
        |ui| {
            egui::Frame::new()
                .fill(tokens.card)
                .corner_radius(tokens.card_radius)
                .shadow(tokens.shadow)
                .inner_margin(egui::Margin::symmetric(24, 18))
                .show(ui, |ui| {
                    ui.set_width(size.x - 48.0);
                    ui.vertical_centered(|ui| {
                        icons::show(ui, Icon::Results, 22.0, tokens.accent);
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "No analyzed results for {}",
                                document.version()
                            ))
                            .family(theme::semibold())
                            .size(theme::TITLE)
                            .color(tokens.text),
                        );
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(
                                    "This synthetic snapshot has not been analyzed, so there is \
                                     nothing to plot. The chart appears here when an analyzed \
                                     result set exists for exactly this version (APP-01C).",
                                )
                                .color(tokens.muted),
                            )
                            .wrap(),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("Query: {}", describe(query)))
                                .size(theme::SMALL)
                                .color(tokens.faint),
                        );
                    });
                });
        },
    );
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
