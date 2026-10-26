//! Native UI-only structural viewer experiment.
//!
//! Cached synthetic stress fixtures and optional offline E2K connectivity preview.

use crate::scene::{Member, MemberKind, Projection, SceneModel, SyntheticPreset};
#[cfg(test)]
use crate::scene::{STORY_COUNT, STORY_HEIGHT_M, sample_members};
use std::sync::Arc;

const DEFAULT_YAW: f32 = 0.65;
const DEFAULT_PITCH: f32 = 0.5;
const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

#[cfg(test)]
fn project(point: [f32; 3], origin: egui::Pos2, yaw: f32, pitch: f32, scale: f32) -> egui::Pos2 {
    Projection::new(
        [0.0, 0.0, STORY_COUNT as f32 * STORY_HEIGHT_M * 0.5],
        origin,
        yaw,
        pitch,
        scale,
    )
    .project(point)
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

/// Measurements for the most recent UI frame (CPU only; excludes GPU rendering).
#[derive(Clone, Copy, Debug, Default)]
pub struct SceneStats {
    pub members: usize,
    pub frames: usize,
    pub shells: usize,
    pub joints: usize,
    pub skipped: usize,
    pub warnings: usize,
    pub drawn_segments: usize,
    pub build_ms: f64,
    pub viewport_ms: f64,
    pub ui_ms: f64,
}

/// State for studying egui geometry and interaction capacity.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct TemplateApp {
    selected_story: usize,
    selected_member: Option<String>,
    show_columns: bool,
    show_beams: bool,
    isolate_story: bool,
    yaw: f32,
    pitch: f32,
    zoom: f32,
    preset: SyntheticPreset,
    show_shells: bool,
    show_other_frames: bool,
    continuous_redraw: bool,
    #[serde(skip)]
    model: Arc<SceneModel>,
    #[serde(skip)]
    stats: SceneStats,
    #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
    #[serde(skip)]
    import_path: String,
    #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
    #[serde(skip)]
    import_error: Option<String>,
    #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
    #[serde(skip)]
    pending_import: Option<std::sync::mpsc::Receiver<Result<SceneModel, String>>>,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            selected_story: 3,
            selected_member: None,
            show_columns: true,
            show_beams: true,
            isolate_story: false,
            yaw: DEFAULT_YAW,
            pitch: DEFAULT_PITCH,
            zoom: 15.0,
            preset: SyntheticPreset::Demo,
            show_shells: true,
            show_other_frames: true,
            continuous_redraw: false,
            model: Arc::default(),
            stats: SceneStats::default(),
            #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
            import_path: String::new(),
            #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
            import_error: None,
            #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
            pending_import: None,
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
        app.model = Arc::new(SceneModel::synthetic(app.preset));
        app.selected_story = app.selected_story.clamp(1, app.model.stories.len());
        if !app
            .model
            .members
            .iter()
            .any(|m| Some(m.key.as_str()) == app.selected_member.as_deref())
        {
            app.selected_member = None;
        }
        app.zoom = if app.zoom.is_finite() {
            app.zoom.clamp(5.0, 40.0)
        } else {
            15.0
        };
        if !app.yaw.is_finite() {
            app.yaw = DEFAULT_YAW;
        }
        app.yaw = app.yaw.rem_euclid(std::f32::consts::TAU);
        app.pitch = if app.pitch.is_finite() {
            app.pitch.clamp(-MAX_PITCH, MAX_PITCH)
        } else {
            DEFAULT_PITCH
        };
        app
    }

    /// Build a cached synthetic scene. Geometry is generated once per preset change.
    pub fn with_preset(preset: SyntheticPreset) -> Self {
        Self {
            preset,
            model: Arc::new(SceneModel::synthetic(preset)),
            ..Self::default()
        }
    }

    /// Load an offline ETABS 17–23 E2K export without starting ETABS.
    ///
    /// # Errors
    /// Returns an error for unreadable files, unsupported writers, missing units,
    /// or exports containing no usable frame/shell geometry.
    #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
    pub fn from_e2k(path: impl AsRef<std::path::Path>) -> Result<Self, String> {
        let mut app = Self::default();
        app.accept_model(crate::e2k_import::load(path.as_ref())?);
        Ok(app)
    }

    pub fn statistics(&self) -> SceneStats {
        self.stats
    }

    fn accept_model(&mut self, model: SceneModel) {
        self.model = Arc::new(model);
        self.selected_member = None;
        self.selected_story = self.selected_story.clamp(1, self.model.stories.len());
        self.zoom = 15.0;
        self.stats = SceneStats::default();
        #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
        {
            self.pending_import = None;
        }
    }

    fn projection(&self, rect: egui::Rect) -> Projection {
        let fit = (rect.width().min(rect.height()) * 0.42 / self.model.radius / 15.0).min(1.0);
        Projection::new(
            self.model.center,
            rect.center(),
            self.yaw,
            self.pitch,
            self.zoom * fit,
        )
    }

    fn visible(&self, member: &Member) -> bool {
        (!self.isolate_story || member.story == self.selected_story)
            && match member.kind {
                MemberKind::Column => self.show_columns,
                MemberKind::Beam => self.show_beams,
                MemberKind::OtherFrame => self.show_other_frames,
                MemberKind::Shell => self.show_shells,
            }
    }

    fn draw_viewport(&mut self, ui: &mut egui::Ui, members: &[Member]) {
        let started = web_time::Instant::now();
        let area = ui.available_size();
        let size = egui::vec2(area.x.max(100.0), area.y.max(200.0));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        let response = response.accessible_name("Structural viewport");
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(18, 25, 35));

        if response.dragged() {
            let delta = response.drag_delta();
            self.yaw = (self.yaw + delta.x * 0.008).rem_euclid(std::f32::consts::TAU);
            self.pitch = (self.pitch + delta.y * 0.008).clamp(-MAX_PITCH, MAX_PITCH);
        }
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            if scroll.abs() > f32::EPSILON {
                self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(5.0, 40.0);
            }
        }

        let projection = self.projection(rect);
        let mut closest: Option<(f32, usize)> = None;
        let mut drawn_segments = 0;
        let pointer = if response.clicked() {
            response.interact_pointer_pos()
        } else {
            None
        };

        for (index, member) in members
            .iter()
            .enumerate()
            .filter(|(_, member)| self.visible(member))
        {
            let selected = self.selected_member.as_deref() == Some(member.key.as_str());
            let color = if selected {
                egui::Color32::from_rgb(255, 177, 88)
            } else if member.story == self.selected_story {
                egui::Color32::from_rgb(116, 216, 194)
            } else {
                match member.kind {
                    MemberKind::Column => egui::Color32::from_rgb(92, 151, 210),
                    MemberKind::Beam => egui::Color32::from_rgb(133, 159, 181),
                    MemberKind::OtherFrame => egui::Color32::from_rgb(177, 147, 205),
                    MemberKind::Shell => egui::Color32::from_rgb(121, 134, 158),
                }
            };
            for segment in member.segments() {
                let start = projection.project(segment[0]);
                let end = projection.project(segment[1]);
                drawn_segments += 1;
                painter.line_segment(
                    [start, end],
                    egui::Stroke::new(if selected { 4.0 } else { 2.0 }, color),
                );

                if let Some(pos) = pointer {
                    let distance = distance_to_segment_squared(pos, start, end);
                    if distance <= 9.0 * 9.0
                        && closest.as_ref().is_none_or(|(best, _)| distance < *best)
                    {
                        closest = Some((distance, index));
                    }
                }
            }
        }

        if response.clicked() {
            self.selected_member = closest.map(|(_, index)| members[index].key.clone());
        }

        painter.text(
            rect.left_top() + egui::vec2(16.0, 14.0),
            egui::Align2::LEFT_TOP,
            format!(
                "{}  |  {} entities  |  ORTHOGRAPHIC WIREFRAME",
                self.model.source,
                members.len()
            ),
            egui::FontId::proportional(13.0),
            egui::Color32::from_rgb(204, 216, 229),
        );
        painter.text(
            rect.left_bottom() + egui::vec2(16.0, -15.0),
            egui::Align2::LEFT_BOTTOM,
            "Drag left/right + up/down: orbit  ·  Wheel: zoom  ·  Click: inspect member",
            egui::FontId::proportional(12.0),
            egui::Color32::from_rgb(155, 169, 186),
        );
        self.stats = SceneStats {
            members: members.len(),
            frames: self.model.frames,
            shells: self.model.shells,
            joints: self.model.joints,
            skipped: self.model.skipped,
            warnings: self.model.warnings,
            build_ms: self.model.build_ms,
            drawn_segments,
            viewport_ms: started.elapsed().as_secs_f64() * 1000.0,
            ui_ms: self.stats.ui_ms,
        };
    }
}

impl TemplateApp {
    fn show_explorer(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("model_explorer")
            .default_size(200.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("explorer_scroll")
                    .show(ui, |ui| {
                        ui.heading("Model Explorer (profile 2)");
                        ui.label(format!(
                            "{} · {} entities",
                            self.model.source,
                            self.model.members.len()
                        ));
                        ui.label("Stress preset");
                        for preset in [
                            SyntheticPreset::Demo,
                            SyntheticPreset::FiftyThousand,
                            SyntheticPreset::HundredThousand,
                        ] {
                            if ui
                                .selectable_label(
                                    self.model.source == "Synthetic" && self.preset == preset,
                                    preset.label(),
                                )
                                .clicked()
                            {
                                self.preset = preset;
                                self.accept_model(SceneModel::synthetic(preset));
                            }
                        }
                        #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
                        self.import_controls(ui);
                        ui.separator();
                        ui.label("Select story");
                        egui::ScrollArea::vertical()
                            .id_salt("stories")
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for story in (1..=self.model.stories.len()).rev() {
                                    let label = &self.model.stories[story - 1];
                                    if ui
                                        .selectable_label(self.selected_story == story, label)
                                        .clicked()
                                    {
                                        self.selected_story = story;
                                        self.selected_member = None;
                                    }
                                }
                            });
                        ui.separator();
                        ui.heading("Visibility");
                        ui.checkbox(&mut self.show_columns, "Columns");
                        ui.checkbox(&mut self.show_beams, "Beams");
                        if self.model.shells > 0 {
                            ui.checkbox(&mut self.show_shells, "Shell outlines");
                        }
                        if self
                            .model
                            .members
                            .iter()
                            .any(|m| m.kind == MemberKind::OtherFrame)
                        {
                            ui.checkbox(&mut self.show_other_frames, "Other frames");
                        }
                        ui.checkbox(&mut self.isolate_story, "Isolate selected story");
                        if ui.button("Reset view").clicked() {
                            self.yaw = DEFAULT_YAW;
                            self.pitch = DEFAULT_PITCH;
                            self.zoom = 15.0;
                        }
                        ui.checkbox(&mut self.continuous_redraw, "Continuous redraw");
                        ui.small("Enable redraw to stress the renderer while idle.");
                    });
            });
    }
    fn show_properties(&self, ui: &mut egui::Ui, model: &SceneModel) {
        let members = &model.members;
        egui::Panel::right("properties")
            .default_size(225.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("inspector_scroll").show(ui, |ui| {
                ui.heading("Inspector");
                let story_label = if model.source == "Synthetic" { self.selected_story.to_string() }
                    else { model.stories[self.selected_story - 1].clone() };
                ui.label(format!("Active story: {story_label}"));
                ui.separator();
                if let Some(key) = &self.selected_member {
                    if let Some(member) = members.iter().find(|member| &member.key == key) {
                        ui.strong(&member.key);
                        ui.label(format!("Kind: {}", member.kind.label()));
                        ui.label(format!("Story: {}", model.stories[member.story - 1]));
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
                ui.heading("Stress measurements");
                ui.label(format!("Frames: {} · Shells: {}", model.frames, model.shells));
                ui.label(format!("Segments drawn: {}", self.stats.drawn_segments));
                ui.label(format!("Build / import: {:.2} ms", model.build_ms));
                ui.label(format!("Viewport CPU: {:.2} ms", self.stats.viewport_ms));
                ui.label(format!("UI CPU: {:.2} ms", self.stats.ui_ms));
                ui.small("CPU construction times from previous frame. GPU time and presentation FPS are not measured.");
                if model.source != "Synthetic" {
                    ui.separator();
                    ui.label(format!("Joints: {} · Scene notes: {} · Warnings: {}", model.joints, model.skipped, model.warnings));
                    ui.small("Offline connectivity preview in metres. Joint offsets, insertion, thickness and opening cuts are not applied; braces follow the parser's planar projection. No analysis results are loaded.");
                }
                });
            });
    }
    #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
    fn import_controls(&mut self, ui: &mut egui::Ui) {
        #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
        {
            ui.separator();
            ui.label("Local E2K path");
            ui.text_edit_singleline(&mut self.import_path)
                .accessible_name("Local E2K path");
            if ui
                .add_enabled(
                    self.pending_import.is_none() && !self.import_path.trim().is_empty(),
                    egui::Button::new("Load E2K"),
                )
                .clicked()
            {
                let path = std::path::PathBuf::from(self.import_path.trim());
                let (sender, receiver) = std::sync::mpsc::channel();
                self.pending_import = Some(receiver);
                self.import_error = None;
                std::thread::spawn(move || {
                    drop(sender.send(crate::e2k_import::load(&path)));
                });
                ui.ctx().request_repaint();
            }
            if self.pending_import.is_some() {
                ui.label("Loading offline E2K…");
            }
            if let Some(error) = &self.import_error {
                ui.colored_label(egui::Color32::LIGHT_RED, error);
            }
        }
    }
    #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
    fn poll_import(&mut self, ui: &egui::Ui) {
        #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
        if let Some(receiver) = &self.pending_import {
            match receiver.try_recv() {
                Ok(Ok(model)) => {
                    self.accept_model(model);
                    self.import_error = None;
                }
                Ok(Err(error)) => {
                    self.pending_import = None;
                    self.import_error = Some(error);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ui
                    .ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.pending_import = None;
                    self.import_error = Some("Import worker stopped unexpectedly.".into());
                }
            }
        }
    }
}

impl eframe::App for TemplateApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let started = web_time::Instant::now();
        #[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
        self.poll_import(ui);

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("TALOS / egui Structural Lab");
                ui.separator();
                ui.label("Native UI experiment");
                ui.separator();
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        self.show_explorer(ui);

        let model = Arc::clone(&self.model);
        let members = &model.members;
        self.show_properties(ui, &model);

        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_viewport(ui, members);
        });
        self.stats.ui_ms = started.elapsed().as_secs_f64() * 1000.0;
        if self.continuous_redraw {
            ui.ctx().request_repaint();
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod ui_tests;

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
        let before = project(point, origin, 0.0, DEFAULT_PITCH, 10.0);
        let after = project(point, origin, 1.0, DEFAULT_PITCH, 10.0);
        assert!(after.x.is_finite() && after.y.is_finite());
        assert!((before.x - after.x).abs() > 0.1);
    }

    #[test]
    fn orbit_keeps_the_model_center_at_the_viewport_center() {
        let center = [0.0, 0.0, STORY_COUNT as f32 * STORY_HEIGHT_M * 0.5];
        let origin = egui::pos2(320.0, 240.0);
        for yaw in [0.0, 1.0, 3.0] {
            for pitch in [-MAX_PITCH, 0.0, MAX_PITCH] {
                assert_eq!(project(center, origin, yaw, pitch, 15.0), origin);
            }
        }
    }

    #[test]
    fn pitch_changes_elevation_in_both_directions_without_changing_horizontal_position() {
        let point = [2.0, 3.0, STORY_COUNT as f32 * STORY_HEIGHT_M * 0.5];
        let origin = egui::pos2(320.0, 240.0);
        let above = project(point, origin, 0.0, 0.5, 15.0);
        let below = project(point, origin, 0.0, -0.5, 15.0);
        assert_eq!(above.x, below.x);
        assert!(above.y > origin.y && below.y < origin.y);
    }
}
