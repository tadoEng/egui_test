//! Native UI-only structural viewer experiment.
//!
//! All engineering data here is synthetic. This is deliberately not an ETABS
//! parser, geometry kernel, wgpu CAD renderer, or ECharts/WebView integration.

const STORY_COUNT: usize = 6;
const STORY_HEIGHT_M: f32 = 4.0;
const PLAN: [[f32; 2]; 4] = [[-3.0, -2.5], [3.0, -2.5], [3.0, 2.5], [-3.0, 2.5]];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MemberKind {
    Column,
    Beam,
}

impl MemberKind {
    fn label(self) -> &'static str {
        match self {
            Self::Column => "Column",
            Self::Beam => "Beam",
        }
    }
}

#[derive(Clone, Debug)]
struct Member {
    key: String,
    story: usize,
    kind: MemberKind,
    start: [f32; 3],
    end: [f32; 3],
}

/// Stable synthetic member identities: 4 columns + 4 beams on every story.
fn sample_members() -> Vec<Member> {
    let mut members = Vec::with_capacity(STORY_COUNT * 8);
    for story in 1..=STORY_COUNT {
        let lower = (story - 1) as f32 * STORY_HEIGHT_M;
        let upper = story as f32 * STORY_HEIGHT_M;
        for (index, [x, y]) in PLAN.iter().copied().enumerate() {
            members.push(Member {
                key: format!("C{}@S{story}", index + 1),
                story,
                kind: MemberKind::Column,
                start: [x, y, lower],
                end: [x, y, upper],
            });
        }
        for index in 0..PLAN.len() {
            let a = PLAN[index];
            let b = PLAN[(index + 1) % PLAN.len()];
            members.push(Member {
                key: format!("B{}@S{story}", index + 1),
                story,
                kind: MemberKind::Beam,
                start: [a[0], a[1], upper],
                end: [b[0], b[1], upper],
            });
        }
    }
    members
}

fn project(point: [f32; 3], origin: egui::Pos2, yaw: f32, scale: f32) -> egui::Pos2 {
    let (sin, cos) = yaw.sin_cos();
    let horizontal_x = point[0] * cos - point[1] * sin;
    let horizontal_y = point[0] * sin + point[1] * cos;
    egui::pos2(
        origin.x + horizontal_x * scale,
        origin.y + (horizontal_y * 0.45 - point[2] * 0.8) * scale,
    )
}

fn distance_to_segment_squared(p: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let segment = b - a;
    let length_sq = segment.length_sq();
    if length_sq <= f32::EPSILON {
        return (p - a).length_sq();
    }
    let t = ((p - a).dot(segment) / length_sq).clamp(0.0, 1.0);
    (p - (a + segment * t)).length_sq()
}

/// A deliberately small state model suitable for studying egui behavior.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TemplateApp {
    selected_story: usize,
    selected_member: Option<String>,
    show_columns: bool,
    show_beams: bool,
    isolate_story: bool,
    yaw: f32,
    zoom: f32,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            selected_story: 3,
            selected_member: None,
            show_columns: true,
            show_beams: true,
            isolate_story: false,
            yaw: 0.65,
            zoom: 15.0,
        }
    }
}

impl TemplateApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let mut app: Self = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        // Never trust persisted UI state after upgrading a prototype.
        app.selected_story = app.selected_story.clamp(1, STORY_COUNT);
        app.zoom = if app.zoom.is_finite() {
            app.zoom.clamp(5.0, 40.0)
        } else {
            15.0
        };
        if !app.yaw.is_finite() {
            app.yaw = 0.65;
        }
        app
    }

    fn visible(&self, member: &Member) -> bool {
        (!self.isolate_story || member.story == self.selected_story)
            && match member.kind {
                MemberKind::Column => self.show_columns,
                MemberKind::Beam => self.show_beams,
            }
    }

    fn draw_viewport(&mut self, ui: &mut egui::Ui, members: &[Member]) {
        let area = ui.available_size();
        let size = egui::vec2(area.x.max(100.0), area.y.max(200.0));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(18, 25, 35));

        if response.dragged() {
            self.yaw += response.drag_delta().x * 0.008;
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > f32::EPSILON {
                self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(5.0, 40.0);
            }
        }

        let origin = rect.center() + egui::vec2(0.0, 100.0);
        let mut closest: Option<(f32, String)> = None;
        let pointer = if response.clicked() {
            response.interact_pointer_pos()
        } else {
            None
        };

        for member in members.iter().filter(|member| self.visible(member)) {
            let start = project(member.start, origin, self.yaw, self.zoom);
            let end = project(member.end, origin, self.yaw, self.zoom);
            let selected = self.selected_member.as_deref() == Some(member.key.as_str());
            let color = if selected {
                egui::Color32::from_rgb(255, 177, 88)
            } else if member.story == self.selected_story {
                egui::Color32::from_rgb(116, 216, 194)
            } else {
                match member.kind {
                    MemberKind::Column => egui::Color32::from_rgb(92, 151, 210),
                    MemberKind::Beam => egui::Color32::from_rgb(133, 159, 181),
                }
            };
            painter.line_segment(
                [start, end],
                egui::Stroke::new(if selected { 4.0 } else { 2.0 }, color),
            );

            if let Some(pos) = pointer {
                let distance = distance_to_segment_squared(pos, start, end);
                if distance <= 9.0 * 9.0
                    && closest.as_ref().is_none_or(|(best, _)| distance < *best)
                {
                    closest = Some((distance, member.key.clone()));
                }
            }
        }

        if response.clicked() {
            self.selected_member = closest.map(|(_, key)| key);
        }

        painter.text(
            rect.left_top() + egui::vec2(16.0, 14.0),
            egui::Align2::LEFT_TOP,
            "SYNTHETIC STRUCTURAL MODEL  |  ORTHOGRAPHIC PREVIEW",
            egui::FontId::proportional(13.0),
            egui::Color32::from_rgb(204, 216, 229),
        );
        painter.text(
            rect.left_bottom() + egui::vec2(16.0, -15.0),
            egui::Align2::LEFT_BOTTOM,
            "Drag: rotate  ·  Wheel: zoom  ·  Click: inspect member  ·  Rust egui::Painter, not wgpu 3D",
            egui::FontId::proportional(12.0),
            egui::Color32::from_rgb(155, 169, 186),
        );
    }
}

impl eframe::App for TemplateApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let members = sample_members();

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("TALOS / egui Structural Lab");
                ui.separator();
                ui.label("Native UI experiment");
                ui.separator();
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::Panel::left("model_explorer")
            .default_size(200.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.heading("Model Explorer");
                ui.label("Synthetic 6-storey frame");
                ui.separator();
                ui.label("Select story");
                for story in (1..=STORY_COUNT).rev() {
                    let label = format!("Story {story}");
                    if ui
                        .selectable_label(self.selected_story == story, label)
                        .clicked()
                    {
                        self.selected_story = story;
                        self.selected_member = None;
                    }
                }
                ui.separator();
                ui.heading("Visibility");
                ui.checkbox(&mut self.show_columns, "Columns");
                ui.checkbox(&mut self.show_beams, "Beams");
                ui.checkbox(&mut self.isolate_story, "Isolate selected story");
                if ui.button("Reset view").clicked() {
                    self.yaw = 0.65;
                    self.zoom = 15.0;
                }
            });

        egui::Panel::right("properties")
            .default_size(225.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.heading("Inspector");
                ui.label(format!("Active story: {}", self.selected_story));
                ui.separator();
                if let Some(key) = &self.selected_member {
                    if let Some(member) = members.iter().find(|member| &member.key == key) {
                        ui.strong(&member.key);
                        ui.label(format!("Kind: {}", member.kind.label()));
                        ui.label(format!("Story: {}", member.story));
                        ui.label(format!(
                            "Start: ({:.1}, {:.1}, {:.1}) m",
                            member.start[0], member.start[1], member.start[2]
                        ));
                        ui.label(format!(
                            "End: ({:.1}, {:.1}, {:.1}) m",
                            member.end[0], member.end[1], member.end[2]
                        ));
                    } else {
                        ui.label("Selected key not found.");
                    }
                } else {
                    ui.label("Click a member in the viewport.");
                }
                ui.separator();
                ui.label(
                    "ECharts + Wry will be tested in a separate Windows-native integration slice.",
                );
                ui.small("No ETABS model or calculation data is loaded.");
            });

        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_viewport(ui, &members);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generated_member_keys_are_unique_and_story_scoped() {
        let members = sample_members();
        assert_eq!(members.len(), STORY_COUNT * 8);
        let keys: HashSet<_> = members.iter().map(|member| &member.key).collect();
        assert_eq!(keys.len(), members.len());
        assert!(
            members
                .iter()
                .all(|member| (1..=STORY_COUNT).contains(&member.story))
        );
    }

    #[test]
    fn selection_distance_handles_segments_and_points() {
        let a = egui::pos2(0.0, 0.0);
        let b = egui::pos2(10.0, 0.0);
        assert!((distance_to_segment_squared(egui::pos2(5.0, 3.0), a, b) - 9.0).abs() < 1e-5);
        assert!((distance_to_segment_squared(egui::pos2(2.0, 0.0), a, a) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn visibility_respects_story_and_element_filters() {
        let members = sample_members();
        let mut app = TemplateApp {
            isolate_story: true,
            selected_story: 2,
            ..Default::default()
        };
        assert_eq!(
            members.iter().filter(|member| app.visible(member)).count(),
            8
        );
        app.show_beams = false;
        assert_eq!(
            members.iter().filter(|member| app.visible(member)).count(),
            4
        );
        app.show_columns = false;
        assert!(!members.iter().any(|member| app.visible(member)));
    }

    #[test]
    fn projection_is_finite_and_rotates_with_yaw() {
        let point = [2.0, 1.0, 8.0];
        let origin = egui::pos2(100.0, 100.0);
        let before = project(point, origin, 0.0, 10.0);
        let after = project(point, origin, 1.0, 10.0);
        assert!(after.x.is_finite() && after.y.is_finite());
        assert!((before.x - after.x).abs() > 0.1);
    }
}
