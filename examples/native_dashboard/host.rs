use crate::{
    model::{self, Case, Config, Direction, Packet, State, Units},
    native_viewport::NativeViewport,
    webview_host::ChildWebView,
    windows_probe::WindowsProbe,
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

const TEMPLATE: &str = include_str!("chart.html");
#[expect(
    clippy::large_include_file,
    reason = "The verified offline ECharts runtime is shared by both examples"
)]
const ECHARTS: &str = include_str!("../../assets/vendor/echarts/echarts.min.js");
const MARKER: &str = "/*__ECHARTS_RUNTIME__*/";
fn html() -> Result<String, String> {
    if TEMPLATE.matches(MARKER).count() != 1 {
        return Err("Dashboard template must contain exactly one runtime marker".into());
    }
    Ok(TEMPLATE.replacen(MARKER, ECHARTS, 1))
}

pub(crate) fn run() -> eframe::Result {
    env_logger::init();
    let smoke = std::env::var_os("DASHBOARD_SMOKE_REPORT").map(PathBuf::from);
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 24,
        viewport: egui::ViewportBuilder::default()
            .with_app_id("talos-synthetic-dashboard-lab")
            .with_visible(smoke.is_none())
            .with_inner_size([1360.0, 920.0])
            .with_min_inner_size([1000.0, 720.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TALOS | synthetic native dashboard",
        options,
        Box::new(move |cc| {
            Ok(Box::new(
                Dashboard::new(cc, smoke).map_err(std::io::Error::other)?,
            ))
        }),
    )
}

struct Dashboard {
    state: State,
    packet: Packet,
    viewport: NativeViewport,
    chart: Option<ChildWebView>,
    visible: bool,
    sent: Option<(u64, Option<u16>)>,
    error: Option<String>,
    rejected: u64,
    smoke: Option<Smoke>,
    windows_probe: WindowsProbe,
}
struct Smoke {
    path: PathBuf,
    started: Instant,
    phase: u8,
    query: Option<Receiver<String>>,
    snapshots: Vec<serde_json::Value>,
}

impl Dashboard {
    fn new(cc: &eframe::CreationContext<'_>, smoke: Option<PathBuf>) -> Result<Self, String> {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let state = State::default();
        let packet = state.packet();
        let (chart, error) = match Self::create_chart(cc, &cc.egui_ctx) {
            Ok(chart) => (Some(chart), None),
            Err(e) => (None, Some(e)),
        };
        Ok(Self {
            state,
            packet,
            viewport: NativeViewport::new(cc)?,
            chart,
            visible: true,
            sent: None,
            error,
            rejected: 0,
            windows_probe: WindowsProbe::default(),
            smoke: smoke.map(|path| Smoke {
                path,
                started: Instant::now(),
                phase: 0,
                query: None,
                snapshots: Vec::new(),
            }),
        })
    }
    fn create_chart(
        parent: &impl wry::raw_window_handle::HasWindowHandle,
        ctx: &egui::Context,
    ) -> Result<ChildWebView, String> {
        ChildWebView::new(parent, ctx, html()?, model::is_ready)
    }
    fn select(&mut self, story: Option<u16>) {
        match self.state.select(story) {
            Ok(_) => self.packet.selected_story = self.state.selected(),
            Err(e) => self.error = Some(e.into()),
        }
    }
    fn configure(&mut self, config: Config) {
        match self.state.reconfigure(config) {
            Ok(true) => {
                self.packet = self.state.packet();
                self.error = None;
            }
            Ok(false) => {}
            Err(e) => self.error = Some(e.into()),
        }
    }
    fn poll(&mut self) {
        let messages = self
            .chart
            .as_ref()
            .map(ChildWebView::pending_messages)
            .unwrap_or_default();
        for message in messages {
            match self.state.accept(&message) {
                Ok(_) => self.packet.selected_story = self.state.selected(),
                Err(_) => self.rejected = self.rejected.saturating_add(1),
            }
        }
    }
    fn sync(&mut self, rect: Option<egui::Rect>, ctx: &egui::Context) {
        let Some(chart) = &mut self.chart else {
            return;
        };
        let key = (self.packet.generation, self.packet.selected_story);
        if chart.is_ready() && self.sent != Some(key) {
            match model::packet_script(&self.packet)
                .map_err(|e| e.to_string())
                .and_then(|script| {
                    chart
                        .webview
                        .evaluate_script(&script)
                        .map_err(|e| e.to_string())
                }) {
                Ok(()) => self.sent = Some(key),
                Err(e) => self.error = Some(e),
            }
        }
        let minimized = ctx.input(|i| i.viewport().minimized == Some(true));
        if let Err(e) = chart.place(rect, ctx.pixels_per_point(), self.visible && !minimized) {
            self.error = Some(e);
        }
    }
    fn controls(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let mut config = self.state.config();
        egui::Panel::top("dashboard-controls").show(ui, |ui| {
            ui.heading("Native dashboard · SYNTHETIC FIXTURES ONLY");
            ui.horizontal_wrapped(|ui| {
                ui.label("Story range");
                ui.add(egui::Slider::new(&mut config.first, 1..=model::STORIES).text("from"));
                ui.add(egui::Slider::new(&mut config.last, 1..=model::STORIES).text("to"));
                ui.selectable_value(&mut config.case, Case::A, "Fixture A");
                ui.selectable_value(&mut config.case, Case::B, "Fixture B");
                ui.selectable_value(&mut config.direction, Direction::X, "X");
                ui.selectable_value(&mut config.direction, Direction::Y, "Y");
                ui.selectable_value(&mut config.units, Units::MillimetresKilonewtons, "mm / kN");
                ui.selectable_value(&mut config.units, Units::MetresNewtons, "m / N");
            });
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.visible, "Charts visible");
                if self.chart.is_some() {
                    if ui.button("Destroy charts").clicked() {
                        self.chart = None;
                        self.sent = None;
                    }
                } else if ui.button("Recreate charts").clicked() {
                    match Self::create_chart(frame, ui.ctx()) {
                        Ok(chart) => {
                            self.chart = Some(chart);
                            self.sent = None;
                            self.error = None;
                        }
                        Err(e) => self.error = Some(e),
                    }
                }
                if ui.button("Fit 3D core").clicked() {
                    self.viewport.fit();
                }
                if ui.button("Clear selection").clicked() {
                    self.select(None);
                }
                self.windows_probe.controls(ui);
                let status = if self.error.is_some() {
                    "Error"
                } else if self.chart.is_none() {
                    "Closed"
                } else if !self.visible {
                    "Hidden"
                } else if self.chart.as_ref().is_some_and(ChildWebView::is_ready) {
                    "Ready"
                } else {
                    "Loading"
                };
                ui.label(format!(
                    "{status} · {} r{} · generation {} · rejected {}",
                    self.packet.dataset,
                    self.packet.revision,
                    self.packet.generation,
                    self.rejected
                ));
            });
            if let Some(e) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, e);
            }
        });
        self.configure(config);
    }
    fn results(&mut self, ui: &mut egui::Ui) {
        let mut requested = None;
        egui::Panel::bottom("dashboard-results")
            .resizable(true)
            .default_size(210.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Read-only synthetic results");
                    ui.label(format!(
                        "{} rows · drift {} · shear {}",
                        self.packet.rows.len(),
                        self.packet.drift_unit,
                        self.packet.shear_unit
                    ));
                });
                egui::ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("dashboard-table")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Story");
                            ui.strong("Fixture drift");
                            ui.strong("Fixture shear");
                            ui.end_row();
                            for row in &self.packet.rows {
                                if ui
                                    .selectable_label(
                                        self.packet.selected_story == Some(row.story),
                                        row.story.to_string(),
                                    )
                                    .clicked()
                                {
                                    requested = Some(row.story);
                                }
                                ui.label(format!("{:.4}", row.drift));
                                ui.label(format!("{:.2}", row.shear));
                                ui.end_row();
                            }
                        });
                });
            });
        if let Some(story) = requested {
            self.select(Some(story));
        }
    }
    fn query_snapshot(chart: &ChildWebView, smoke: &mut Smoke) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        chart
            .webview
            .evaluate_script_with_callback("window.dashboardBridge.snapshot()", move |value| {
                drop(tx.send(value));
            })
            .map_err(|e| e.to_string())?;
        smoke.query = Some(rx);
        Ok(())
    }
    fn take_snapshot(smoke: &mut Smoke) -> Option<serde_json::Value> {
        let raw = smoke.query.as_ref()?.try_recv().ok()?;
        smoke.query = None;
        let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
        if let serde_json::Value::String(json) = value {
            serde_json::from_str(&json).ok()
        } else {
            Some(value)
        }
    }
    fn run_smoke(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(mut smoke) = self.smoke.take() else {
            return;
        };
        let mut done = None;
        if smoke.started.elapsed() > Duration::from_secs(30) || self.error.is_some() {
            done = Some(false);
        } else if let Some(chart) = &self.chart
            && chart.is_ready()
        {
            match smoke.phase {
                0 => {
                    self.select(Some(59));
                    smoke.phase = 1;
                }
                1 if self.sent == Some((1, Some(59))) => {
                    if smoke.query.is_none() {
                        if let Err(e) = Self::query_snapshot(chart, &mut smoke) {
                            self.error = Some(e);
                        }
                    } else if let Some(snapshot) = Self::take_snapshot(&mut smoke) {
                        let valid = snapshot["selected_story"] == 59
                            && snapshot["rows"] == 69
                            && snapshot["plots"] == 2;
                        smoke.snapshots.push(snapshot);
                        if valid {
                            self.configure(Config {
                                first: 10,
                                last: 12,
                                case: Case::B,
                                direction: Direction::Y,
                                units: Units::MetresNewtons,
                            });
                            smoke.phase = 2;
                        } else {
                            done = Some(false);
                        }
                    }
                }
                2 if self.sent == Some((2, Some(10))) => {
                    if smoke.query.is_none() {
                        if let Err(e) = Self::query_snapshot(chart, &mut smoke) {
                            self.error = Some(e);
                        }
                    } else if let Some(snapshot) = Self::take_snapshot(&mut smoke) {
                        let valid = snapshot["generation"] == 2
                            && snapshot["rows"] == 3
                            && snapshot["drift"][0][0]
                                .as_f64()
                                .is_some_and(|v| (v - 0.001).abs() < 1.0e-12)
                            && snapshot["shear"][0][0] == 600000.0;
                        smoke.snapshots.push(snapshot);
                        if valid {
                            let script = "for(const generation of [1,2])window.ipc.postMessage(JSON.stringify({version:2,type:'select_story',dataset:'LAB-DASH-01',revision:1,generation,plot:'shear',story:11}));";
                            if let Err(e) = chart.webview.evaluate_script(script) {
                                self.error = Some(e.to_string());
                            }
                            smoke.phase = 3;
                        } else {
                            done = Some(false);
                        }
                    }
                }
                3 if self.state.selected() == Some(11) && self.rejected >= 1 => {
                    self.chart = None;
                    self.sent = None;
                    match Self::create_chart(frame, ctx) {
                        Ok(chart) => self.chart = Some(chart),
                        Err(e) => self.error = Some(e),
                    }
                    smoke.phase = 4;
                }
                4 if self.sent == Some((2, Some(11))) => {
                    if smoke.query.is_none() {
                        if let Err(e) = Self::query_snapshot(chart, &mut smoke) {
                            self.error = Some(e);
                        }
                    } else if let Some(snapshot) = Self::take_snapshot(&mut smoke) {
                        done = Some(
                            snapshot["selected_story"] == 11
                                && snapshot["rows"] == 3
                                && snapshot["plots"] == 2,
                        );
                        smoke.snapshots.push(snapshot);
                    }
                }
                _ => {}
            }
        }
        if let Some(passed) = done {
            self.finish_smoke(ctx, smoke, passed);
        } else {
            self.smoke = Some(smoke);
            ctx.request_repaint_after(Duration::from_millis(10));
        }
    }
    fn finish_smoke(&self, ctx: &egui::Context, smoke: Smoke, passed: bool) {
        let (vertices, triangles) = self.viewport.mesh_counts();
        let callback_paints = self.viewport.callback_paints();
        let passed = passed && callback_paints > 0;
        let report = serde_json::json!({"assessment":if passed{"PASS"}else{"FAIL"},"scope":"hidden composite programmatic two-chart options/readback/IPC and one WebView recreation; not manual input/presentation/network capture",
                "webview2":wry::webview_version().ok(),"phase":smoke.phase,"rejected":self.rejected,"selected_story":self.state.selected(),"error":self.error,
                "native_core_vertices":vertices,"native_core_triangles":triangles,"snapshots":smoke.snapshots,"manual_input_tested":false,"network_capture_performed":false,
                "native_callback_paints":callback_paints,"presented_frames_measured":false,
                "navigation_attempts":self.chart.as_ref().and_then(|chart|chart.navigation.lock().ok().map(|entries|entries.clone()))});
        if let Err(e) = std::fs::write(smoke.path, report.to_string()) {
            log::error!("write dashboard smoke report: {e}");
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

impl eframe::App for Dashboard {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.poll();
        self.controls(ui, frame);
        egui::Panel::left("dashboard-explorer").default_size(205.0).show(ui,|ui|{
            ui.heading("Story explorer");ui.small("Charts/table: LAB-DASH-01 r1");
            let config=self.state.config();
            egui::ScrollArea::vertical().max_height(240.0).show(ui,|ui|{for story in config.first..=config.last{if ui.selectable_label(self.state.selected()==Some(story),format!("Story {story}")).clicked(){self.select(Some(story));}}});
            ui.separator();ui.heading("Selection inspector");ui.label(format!("Story {:?}",self.state.selected()));ui.label(format!("{} · {}",self.packet.case,self.packet.direction));
            let peak=self.packet.rows.iter().fold(0.0_f64,|a,r|a.max(r.drift));
            ui.group(|ui|{ui.label("Peak fixture drift");ui.strong(format!("{peak:.4} {}",self.packet.drift_unit));});
            ui.group(|ui|{ui.label("Visible fixture stories");ui.strong(self.packet.rows.len().to_string());});
            ui.separator();ui.small("3D core: separate LAB-CORE-01 preview. Story/element picking and 3D cross-highlights are not connected. No engineering results or source authority.");
        });
        self.results(ui);
        let mut chart_rect = None;
        egui::CentralPanel::default().show(ui, |ui| {
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.strong("Native wgpu core · LAB-CORE-01");
                    let (v, t) = self.viewport.mesh_counts();
                    ui.small(format!(
                        "{v} vertices / {t} triangles · boxes + zero-thickness shell"
                    ));
                    self.viewport.show(ui);
                });
                cols[1].vertical(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 0.0, egui::Color32::from_rgb(18, 25, 35));
                    chart_rect = Some(rect.shrink(2.0));
                });
            });
        });
        self.sync(chart_rect, ui.ctx());
        // Intentionally keep the child visible while testing parent overlays.
        self.windows_probe.show(ui.ctx(), chart_rect);
        self.run_smoke(ui.ctx(), frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_charts_use_one_embedded_runtime_and_no_remote_assets() {
        let source = html().expect("authored template");
        assert!(!source.contains(MARKER));
        assert!(source.contains("connect-src 'none'"));
        assert!(source.contains("charts={drift:"));
        assert!(source.contains("shear:echarts.init"));
        assert!(source.contains("row.drift:row.shear"));
        assert!(source.contains("dataset:active.dataset"));
    }
}
