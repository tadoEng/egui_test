//! Results: chart-first, with contextual selectors and a story table; no 3D.
//!
//! The selectors edit a typed [`ResultsQuery`], reconciled by the state to
//! what the current version's cached results hold. The chart and the table
//! show one stored profile from `lab-results` (values as written, gaps kept,
//! nothing recalculated); with no cached results, or refused ones, the card
//! says why and plots nothing.
use crate::document::{Document, Results};
use crate::icons::{self, Icon};
use crate::state::{Action, Direction, Metric, ResultsQuery, State, StoryRange};
use crate::theme::{self, Tokens};
use lab_results::data::Plot;

/// Below this page width the table goes under the chart instead of beside it.
const SIDE_TABLE: f32 = 1000.0;
const TABLE_WIDTH: f32 = 340.0;

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
            ui.horizontal_wrapped(|ui| selectors(ui, state, document, query, &mut actions));
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
            match state.profile() {
                Ok(plot) => {
                    let plot = visible(plot, query.stories.as_ref());
                    let selected = state.results.story.as_deref();
                    let area = ui.available_rect_before_wrap();
                    let (chart, table) = if area.width() >= SIDE_TABLE {
                        let split = area.right() - TABLE_WIDTH - 14.0;
                        (
                            egui::Rect::from_min_max(area.min, egui::pos2(split, area.max.y)),
                            egui::Rect::from_min_max(
                                egui::pos2(split + 14.0, area.min.y),
                                area.max,
                            ),
                        )
                    } else {
                        let split = area.top() + area.height() * 0.6;
                        (
                            egui::Rect::from_min_max(area.min, egui::pos2(area.max.x, split)),
                            egui::Rect::from_min_max(
                                egui::pos2(area.min.x, split + 14.0),
                                area.max,
                            ),
                        )
                    };
                    chart_card(ui, chart, document, &plot, selected, &mut actions);
                    table_card(ui, table, &plot, selected, &mut actions);
                }
                Err(_) => empty_card(ui, document, query),
            }
        });
    actions
}

/// A selector group: a muted label before its control, in one pill.
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

/// A labelled combo of stored strings; picking one pushes `action(value)`.
fn choice(
    ui: &mut egui::Ui,
    label: &str,
    current: Option<&str>,
    options: &[String],
    action: fn(String) -> Action,
    actions: &mut Vec<Action>,
) {
    group(ui, label, |ui| {
        let enabled = !options.is_empty();
        ui.add_enabled_ui(enabled, |ui| {
            let combo = egui::ComboBox::from_id_salt(("results-choice", label))
                .selected_text(
                    egui::RichText::new(current.unwrap_or("None stored")).family(theme::semibold()),
                )
                .show_ui(ui, |ui| {
                    for option in options {
                        if ui
                            .selectable_label(current == Some(option.as_str()), option)
                            .clicked()
                        {
                            actions.push(action(option.clone()));
                        }
                    }
                })
                .response;
            ui.ctx()
                .accesskit_node_builder(combo.id, |node| node.set_label(label.to_owned()));
        });
    });
}

fn selectors(
    ui: &mut egui::Ui,
    state: &State,
    document: &Document,
    query: &ResultsQuery,
    actions: &mut Vec<Action>,
) {
    let tokens = Tokens::of(ui);
    let cached = state.cached();
    // Only the responses these cached results hold; all of them, disabled,
    // when there are none, so the request is visible but nothing is implied.
    let offered = cached.map_or_else(|| Metric::ALL.to_vec(), |c| c.responses());
    group(ui, "Metric", |ui| {
        ui.add_enabled_ui(cached.is_some(), |ui| {
            let combo = egui::ComboBox::from_id_salt("results-metric")
                .selected_text(egui::RichText::new(query.metric.label()).family(theme::semibold()))
                .show_ui(ui, |ui| {
                    for metric in &offered {
                        if ui
                            .selectable_label(query.metric == *metric, metric.label())
                            .clicked()
                        {
                            actions.push(Action::SetMetric(*metric));
                        }
                    }
                })
                .response;
            ui.ctx()
                .accesskit_node_builder(combo.id, |node| node.set_label("Metric"));
        });
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
    let choices = cached
        .and_then(|c| {
            c.response_choices(query.metric, query.direction.cached())
                .ok()
        })
        .unwrap_or_default();
    if query.metric.grouped() {
        choice(
            ui,
            "Group",
            query.group.as_deref(),
            &choices.groups,
            Action::SetGroup,
            actions,
        );
    }
    choice(
        ui,
        "Case",
        query.case.as_deref(),
        &choices.cases,
        Action::SetCase,
        actions,
    );
    group(ui, "Stories", |ui| {
        stories(ui, document, query.stories.as_ref(), actions)
    });
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let authority = state.session.project.authority;
        theme::pill(
            ui,
            authority.label(),
            tokens.synthetic,
            tokens.synthetic_weak,
            authority.detail(),
        );
        if cached.is_some() {
            theme::pill(
                ui,
                "Cached · freshness not verified",
                tokens.warning_text,
                tokens.warning_fill,
                "Stored result values as written in results/calc_output.json. They are not \
                 recalculated, and nothing proves they match the current model.",
            );
        }
        ui.label(
            egui::RichText::new(document.version().to_string())
                .family(theme::semibold())
                .color(tokens.text),
        )
        .on_hover_text("Results follow the version selected in the header.");
        ui.label(egui::RichText::new("for").color(tokens.muted));
    });
}

/// Stories from top to bottom by elevation.
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

/// The profile cut to the story range, by position in the stored order. A
/// range whose ends the profile does not name shows the whole profile.
fn visible(mut plot: Plot, range: Option<&StoryRange>) -> Plot {
    let Some(range) = range else { return plot };
    let at = |name: &str| plot.stories.iter().position(|s| s == name);
    let (Some(a), Some(b)) = (at(&range.top), at(&range.bottom)) else {
        return plot;
    };
    let (from, to) = (a.min(b), a.max(b));
    plot.stories = plot.stories[from..=to].to_vec();
    for series in &mut plot.series {
        series.values = series.values[from..=to].to_vec();
    }
    plot
}

fn card(ui: &mut egui::Ui, rect: egui::Rect, add: impl FnOnce(&mut egui::Ui)) {
    let tokens = Tokens::of(ui);
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::Frame::new()
                .fill(tokens.card)
                .corner_radius(tokens.card_radius)
                .inner_margin(egui::Margin::symmetric(18, 14))
                .show(ui, |ui| {
                    ui.set_min_size(rect.size() - egui::vec2(36.0, 28.0));
                    ui.set_max_size(rect.size() - egui::vec2(36.0, 28.0));
                    add(ui);
                });
        },
    );
}

/// The series palette: accent, amber, green, then muted.
fn palette(tokens: &Tokens, index: usize) -> egui::Color32 {
    [tokens.accent, tokens.approx, tokens.exact, tokens.muted][index % 4]
}
fn reference_color(tokens: &Tokens) -> egui::Color32 {
    egui::Color32::from_rgb(220, 70, 70).lerp_to_gamma(tokens.text, 0.1)
}

fn chart_card(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    document: &Document,
    plot: &Plot,
    selected: Option<&str>,
    actions: &mut Vec<Action>,
) {
    card(ui, rect, |ui| {
        let tokens = Tokens::of(ui);
        ui.label(
            egui::RichText::new(&plot.title)
                .family(theme::semibold())
                .size(theme::TITLE)
                .color(tokens.text),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} · {} stories · {}",
                    document.version(),
                    plot.stories.len(),
                    plot.unit
                ))
                .color(tokens.muted),
            );
            for (index, series) in plot.series.iter().enumerate() {
                legend(ui, palette(&tokens, index), &series.name, false);
            }
            if let Some(reference) = plot.reference {
                legend(
                    ui,
                    reference_color(&tokens),
                    &format!("Stored allowable {}", number(reference, &plot.unit)),
                    true,
                );
            }
        });
        let area = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(area, egui::Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::accesskit::Role::Image,
                true,
                format!("Chart: {}", plot.title),
            )
        });
        let hovered = draw_chart(ui, area, plot, selected, response.hover_pos());
        if let Some(story) = &hovered {
            response.clone().on_hover_ui_at_pointer(|ui| {
                ui.label(egui::RichText::new(story).family(theme::semibold()));
                let at = plot.stories.iter().position(|s| s == story).unwrap_or(0);
                for series in &plot.series {
                    ui.label(format!(
                        "{}: {}",
                        series.name,
                        series.values[at].map_or_else(
                            || "no stored value".to_owned(),
                            |v| number(v, &plot.unit)
                        )
                    ));
                }
            });
        }
        if response.clicked() {
            actions.push(Action::SelectResultStory(
                hovered.filter(|story| Some(story.as_str()) != selected),
            ));
        }
    });
}

fn legend(ui: &mut egui::Ui, color: egui::Color32, text: &str, dashed: bool) {
    let tokens = Tokens::of(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 10.0), egui::Sense::hover());
        let y = rect.center().y;
        if dashed {
            ui.painter().add(egui::Shape::dashed_line(
                &[egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                egui::Stroke::new(1.5, color),
                4.0,
                3.0,
            ));
        } else {
            ui.painter()
                .hline(rect.x_range(), y, egui::Stroke::new(2.0, color));
            ui.painter().circle_filled(rect.center(), 2.5, color);
        }
        ui.label(
            egui::RichText::new(text)
                .size(theme::SMALL)
                .color(tokens.muted),
        );
    });
}

/// Draws the profile (stories top to bottom in stored order, values along
/// x) and returns the story row under `pointer`, if any.
fn draw_chart(
    ui: &egui::Ui,
    area: egui::Rect,
    plot: &Plot,
    selected: Option<&str>,
    pointer: Option<egui::Pos2>,
) -> Option<String> {
    let tokens = Tokens::of(ui);
    let painter = ui.painter_at(area);
    let frame = egui::Rect::from_min_max(
        area.min + egui::vec2(72.0, 8.0),
        area.max - egui::vec2(16.0, 34.0),
    );
    let count = plot.stories.len().max(1);
    #[expect(clippy::cast_precision_loss, reason = "story counts are small")]
    let row = frame.height() / count as f32;
    #[expect(clippy::cast_precision_loss, reason = "story counts are small")]
    let y_of = |index: usize| frame.top() + row * (index as f32 + 0.5);
    let values = plot
        .series
        .iter()
        .flat_map(|s| s.values.iter().flatten().copied())
        .chain(plot.reference)
        .chain(std::iter::once(0.0));
    let (low, high) = values.fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(v), h.max(v)));
    let (low, high) = if (high - low).abs() < f64::EPSILON {
        (low - 1.0, high + 1.0)
    } else {
        let pad = (high - low) * 0.06;
        (low - pad, high + pad)
    };
    #[expect(clippy::cast_possible_truncation, reason = "screen coordinates")]
    let x_of = |value: f64| frame.left() + ((value - low) / (high - low)) as f32 * frame.width();

    // Value grid and labels.
    for tick in ticks(low, high) {
        let x = x_of(tick);
        painter.vline(x, frame.y_range(), egui::Stroke::new(1.0, tokens.border));
        painter.text(
            egui::pos2(x, frame.bottom() + 6.0),
            egui::Align2::CENTER_TOP,
            number(tick, &plot.unit),
            egui::FontId::proportional(theme::SMALL),
            tokens.muted,
        );
    }
    painter.vline(
        x_of(0.0),
        frame.y_range(),
        egui::Stroke::new(1.0, tokens.muted),
    );

    // Story rows, the hovered/selected band, and labels.
    let hovered = pointer
        .filter(|p| frame.y_range().contains(p.y))
        .and_then(|p| {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a row index"
            )]
            let index = ((p.y - frame.top()) / row).floor() as usize;
            plot.stories.get(index).cloned()
        });
    for (index, story) in plot.stories.iter().enumerate() {
        let y = y_of(index);
        let band = egui::Rect::from_center_size(
            egui::pos2(area.center().x, y),
            egui::vec2(area.width(), row.min(28.0)),
        );
        if selected == Some(story.as_str()) {
            painter.rect_filled(band, 4.0, tokens.accent_weak);
        } else if hovered.as_deref() == Some(story.as_str()) {
            painter.rect_filled(band, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        // Label every row when they fit, otherwise every nth.
        let every = (14.0 / row).ceil().max(1.0);
        #[expect(clippy::cast_precision_loss, reason = "story counts are small")]
        if (index as f32 % every) == 0.0 || selected == Some(story.as_str()) {
            painter.text(
                egui::pos2(frame.left() - 10.0, y),
                egui::Align2::RIGHT_CENTER,
                story,
                egui::FontId::proportional(theme::SMALL),
                tokens.muted,
            );
        }
    }

    if let Some(reference) = plot.reference {
        let x = x_of(reference);
        painter.add(egui::Shape::dashed_line(
            &[egui::pos2(x, frame.top()), egui::pos2(x, frame.bottom())],
            egui::Stroke::new(1.5, reference_color(&tokens)),
            6.0,
            4.0,
        ));
    }
    for (index, series) in plot.series.iter().enumerate() {
        let color = palette(&tokens, index);
        // A gap breaks the line; nothing is interpolated across it.
        let mut run: Vec<egui::Pos2> = Vec::new();
        for (at, value) in series.values.iter().enumerate() {
            match value {
                Some(v) => run.push(egui::pos2(x_of(*v), y_of(at))),
                None => {
                    painter.add(egui::Shape::line(
                        std::mem::take(&mut run),
                        egui::Stroke::new(2.0, color),
                    ));
                }
            }
        }
        painter.add(egui::Shape::line(run, egui::Stroke::new(2.0, color)));
        for (at, value) in series.values.iter().enumerate() {
            if let Some(v) = value {
                painter.circle_filled(egui::pos2(x_of(*v), y_of(at)), 2.5, color);
            }
        }
    }
    hovered
}

/// About five round steps between `low` and `high`.
fn ticks(low: f64, high: f64) -> Vec<f64> {
    let span = high - low;
    if !(span.is_finite() && span > 0.0) {
        return Vec::new();
    }
    let raw = span / 5.0;
    let magnitude = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 2.5, 5.0, 10.0]
        .into_iter()
        .map(|m| m * magnitude)
        .find(|s| *s >= raw)
        .unwrap_or(raw);
    let mut tick = (low / step).ceil() * step;
    let mut out = Vec::new();
    while tick <= high && out.len() < 12 {
        out.push(if tick.abs() < step * 1e-9 { 0.0 } else { tick });
        tick += step;
    }
    out
}

/// A stored value as text, with no rounding of meaning: ratios to five
/// decimals, lengths to four, forces and moments to one.
fn number(value: f64, unit: &str) -> String {
    match unit {
        "ratio" => format!("{value:.5}"),
        "ft" | "in" | "m" | "mm" => format!("{value:.4}"),
        _ => format!("{value:.1}"),
    }
}

fn table_card(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    plot: &Plot,
    selected: Option<&str>,
    actions: &mut Vec<Action>,
) {
    card(ui, rect, |ui| {
        let tokens = Tokens::of(ui);
        ui.horizontal(|ui| {
            theme::section(ui, "Story table");
            ui.label(
                egui::RichText::new(format!("stored values · {}", plot.unit))
                    .size(theme::SMALL)
                    .color(tokens.faint),
            );
        });
        let columns = 1 + plot.series.len();
        #[expect(clippy::cast_precision_loss, reason = "a handful of columns")]
        let width = (ui.available_width() - 8.0) / columns as f32;
        let header = |ui: &mut egui::Ui, text: &str| {
            ui.add_sized(
                egui::vec2(width, 20.0),
                egui::Label::new(
                    egui::RichText::new(text)
                        .size(theme::SMALL)
                        .family(theme::semibold())
                        .color(tokens.muted),
                )
                .truncate(),
            )
            .on_hover_text(text);
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            header(ui, "Story");
            for series in &plot.series {
                header(ui, &series.name);
            }
        });
        let row_height = 24.0;
        egui::ScrollArea::vertical()
            .id_salt("results-table")
            .auto_shrink(false)
            .show_rows(ui, row_height, plot.stories.len(), |ui, range| {
                for at in range {
                    let story = &plot.stories[at];
                    let on = selected == Some(story.as_str());
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), row_height),
                        egui::Sense::click(),
                    );
                    let label = std::iter::once(story.clone())
                        .chain(plot.series.iter().map(|s| {
                            s.values[at].map_or_else(|| "—".to_owned(), |v| number(v, &plot.unit))
                        }))
                        .collect::<Vec<_>>();
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::accesskit::Role::Row,
                            true,
                            on,
                            label.join("  ·  "),
                        )
                    });
                    if on || response.hovered() {
                        let fill = if on {
                            tokens.accent_weak
                        } else {
                            ui.visuals().widgets.hovered.weak_bg_fill
                        };
                        ui.painter().rect_filled(rect, 4.0, fill);
                    }
                    for (column, text) in label.iter().enumerate() {
                        #[expect(clippy::cast_precision_loss, reason = "a handful of columns")]
                        let left = rect.left() + width * column as f32;
                        let (align, x) = if column == 0 {
                            (egui::Align2::LEFT_CENTER, left + 6.0)
                        } else {
                            (egui::Align2::RIGHT_CENTER, left + width - 8.0)
                        };
                        ui.painter().text(
                            egui::pos2(x, rect.center().y),
                            align,
                            text,
                            egui::FontId::proportional(theme::BODY),
                            if column == 0 || text != "—" {
                                tokens.text
                            } else {
                                tokens.faint
                            },
                        );
                    }
                    if response.clicked() {
                        actions.push(Action::SelectResultStory((!on).then(|| story.clone())));
                    }
                }
            });
    });
}

/// No profile to show: why, from the version's own state.
fn empty_card(ui: &mut egui::Ui, document: &Document, query: &ResultsQuery) {
    let tokens = Tokens::of(ui);
    let (title, body, warn) = match document.results() {
        Results::Absent(reason) => (
            format!("No cached results for {}", document.version()),
            format!(
                "{reason} The chart and the story table appear when a version holds cached \
                 results; nothing is plotted or estimated without them."
            ),
            false,
        ),
        Results::Refused(reason) => (
            format!("Cached results for {} were refused", document.version()),
            format!("{reason}. Nothing from them is shown."),
            true,
        ),
        Results::Loaded(_) => (
            format!("Nothing stored for this query in {}", document.version()),
            "These cached results hold no profile for the chosen metric, direction, group \
             and case."
                .to_owned(),
            false,
        ),
    };
    let area = ui.available_rect_before_wrap();
    card(ui, area, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space((ui.available_height() * 0.3).max(0.0));
            icons::show(
                ui,
                if warn { Icon::Warning } else { Icon::Results },
                22.0,
                if warn {
                    tokens.warning_text
                } else {
                    tokens.accent
                },
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(title)
                    .family(theme::semibold())
                    .size(theme::TITLE)
                    .color(tokens.text),
            );
            ui.add(egui::Label::new(egui::RichText::new(body).color(tokens.muted)).wrap());
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("Query: {}", describe(query)))
                    .size(theme::SMALL)
                    .color(tokens.faint),
            );
        });
    });
}

/// The resolved query, e.g. `Story drift · wind · X · W · all stories · main/v1`.
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
