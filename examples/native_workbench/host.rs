use crate::{
    data::Direction,
    model::{self, Session},
    native_viewport::NativeViewport,
    webview_host::ChildWebView,
};
use std::{
    io::Write as _,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[expect(
    clippy::large_include_file,
    reason = "Reuse the pinned offline ECharts asset and accepted cached-result presentation"
)]
const ECHARTS: &str = include_str!("../../assets/vendor/echarts/echarts.min.js");
fn html() -> String {
    include_str!("../native_results/chart.html").replacen("/*__ECHARTS_RUNTIME__*/", ECHARTS, 1)
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Ready {
    version: u8,
    #[serde(rename = "type")]
    kind: String,
}
fn ready(raw: &str) -> bool {
    serde_json::from_str::<Ready>(raw).is_ok_and(|r| r.version == 1 && r.kind == "results_ready")
}
fn failure(message: impl Into<String>) -> eframe::Error {
    eframe::Error::AppCreation(Box::new(std::io::Error::other(message.into())))
}
type SmokeReport = Arc<Mutex<Option<serde_json::Value>>>;

pub(crate) fn run() -> eframe::Result {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let public = args.len() == 1 && args[0] == "--public-workbench-smoke";
    let check = args.len() == 2 && args[0] == "--check-only";
    let owned = if public {
        Some(model::public_fixture().map_err(failure)?)
    } else {
        None
    };
    let root = match args.as_slice() {
        [_] if public => owned
            .as_ref()
            .ok_or_else(|| failure("public_fixture_missing"))?
            .path()
            .to_owned(),
        [flag, path]
            if check && !path.to_string_lossy().starts_with("--") && flag == "--check-only" =>
        {
            PathBuf::from(path)
        }
        [path] if !path.to_string_lossy().starts_with("--") => PathBuf::from(path),
        _ => {
            return Err(failure(
                "Usage: native_workbench <version-directory> | --check-only <version-directory> | --public-workbench-smoke",
            ));
        }
    };
    let mut session = Session::load(&root).map_err(failure)?;
    if check {
        return Ok(());
    }
    let mesh = session.mesh.take().ok_or_else(|| failure("mesh_missing"))?;
    let receipt: SmokeReport = Arc::new(Mutex::new(None));
    let result = Arc::clone(&receipt);
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 24,
        persist_window: false,
        viewport: egui::ViewportBuilder::default()
            .with_app_id("talos-read-only-version-workbench")
            .with_visible(!public)
            .with_inner_size([1540.0, 960.0])
            .with_min_inner_size([960.0, 640.0]),
        ..Default::default()
    };
    eframe::run_native(
        "TALOS | read-only version workbench",
        options,
        Box::new(move |cc| {
            let viewport =
                NativeViewport::new_with_source(cc, Some(mesh)).map_err(std::io::Error::other)?;
            let chart = ChildWebView::new(cc, &cc.egui_ctx, html(), ready)
                .map_err(std::io::Error::other)?;
            Ok(Box::new(Workbench {
                session,
                viewport,
                chart,
                last_sent: None,
                rejected: 0,
                transport_error: false,
                smoke: public.then(|| Smoke {
                    started: Instant::now(),
                    phase: 0,
                    report: result,
                }),
            }))
        }),
    )?;
    if let Some(owned) = owned {
        let path = owned.path().to_owned();
        drop(owned);
        let mut report = receipt
            .lock()
            .map_err(|_poisoned| failure("public_smoke_report_lock"))?
            .take()
            .ok_or_else(|| failure("public_smoke_report_missing"))?;
        report["owned_input_removed"] = serde_json::json!(!path.exists());
        let pass = report["assessment"] == "PASS" && !path.exists();
        if !pass {
            report["assessment"] = serde_json::json!("FAIL");
        }
        let mut out = std::io::stdout().lock();
        serde_json::to_writer_pretty(&mut out, &report)
            .map_err(|_private_error| failure("public_report_write_failed"))?;
        writeln!(out)
            .and_then(|()| out.flush())
            .map_err(|_private_error| failure("public_report_write_failed"))?;
        if !pass {
            return Err(failure("public_workbench_smoke_failed"));
        }
    }
    Ok(())
}

struct Workbench {
    session: Session,
    viewport: NativeViewport,
    chart: ChildWebView,
    last_sent: Option<(u64, Option<String>, bool)>,
    rejected: u64,
    transport_error: bool,
    smoke: Option<Smoke>,
}
impl Workbench {
    fn controls(&mut self, ui: &mut egui::Ui) {
        let mut config = self.session.config.clone();
        egui::Panel::top("version-workbench-controls").show(ui, |ui| {
            ui.heading("Read-only version workbench · E2K display proxies + cached results");
            ui.label(format!("{} / {} · source scope {}… · generation {}", self.session.snapshot.manifest.branch,
                self.session.snapshot.manifest.id, &self.session.packet.source[..12], self.session.packet.generation));
            ui.colored_label(egui::Color32::YELLOW, "Selected files checked together at load; source-model parity and analysis freshness remain UNVERIFIED.");
            ui.horizontal_wrapped(|ui| {
                if ui.button("Recheck selected version").clicked() { let _checked = self.session.recheck(); }
                if ui.button("Fit geometry").clicked() { self.viewport.fit(); }
                ui.add_enabled_ui(self.session.linked, |ui| {
                    if ui.button("Clear selection").clicked() { let _selection = self.session.select(None); }
                    ui.selectable_value(&mut config.direction, Direction::X, "X");
                    ui.selectable_value(&mut config.direction, Direction::Y, "Y");
                    if config.direction != self.session.config.direction
                        && let Ok(initial) = self.session.snapshot.initial_config(config.direction) { config = initial; }
                    let choices = &self.session.choices[usize::from(config.direction != Direction::X)];
                    choose(ui, "Drift group", &mut config.drift_group, &choices.groups);
                    choose(ui, "Drift case", &mut config.drift_case, &choices.cases);
                    choose(ui, "Shear case", &mut config.shear_case, &choices.forces);
                });
            });
            let report = &self.session.report;
            ui.small(format!("Geometry: {} · cached plots: {} · manifest: {} · metadata: {} / {} · QA: {}",
                report.geometry_ready, report.supported_plots_ready, report.manifest_analysis,
                report.metadata_analysis, report.metadata_story_coherence, report.engineering_check_status));
            ui.small("Metres in 3D; stored chart units. Gold = source-assigned story ranges, not a geometric storey cut or member-result contour. No picking in this slice.");
            if !self.session.linked {
                ui.colored_label(egui::Color32::LIGHT_RED, "UNLINKED: selected inputs changed or became unreadable. Last valid display retained; selection disabled. Reopen the version to adopt files.");
            }
            if let Some(code) = self.session.refusal { ui.small(format!("Refusal category: {code}")); }
            if self.transport_error { ui.colored_label(egui::Color32::LIGHT_RED, "Chart transport failed; chart selection is unavailable."); }
            if self.chart.navigation.lock().is_ok_and(|entries| entries.contains(&"denied navigation")) {
                ui.colored_label(egui::Color32::YELLOW, "An external chart navigation attempt was blocked.");
            }
        });
        if config != self.session.config && self.session.configure(config).is_err() {
            self.transport_error = true;
        }
    }
    fn stories(&mut self, ui: &mut egui::Ui) {
        let mut selected = None;
        egui::Panel::left("version-source-stories").default_size(180.0).resizable(true).show(ui, |ui| {
            ui.heading("E2K source stories");
            ui.small("Exact source names / elevations (m)");
            egui::ScrollArea::vertical().show(ui, |ui| {
                for story in &self.session.stories {
                    let enabled = self.session.linked && self.session.common_story(&story.name);
                    ui.add_enabled_ui(enabled, |ui| {
                        if ui.selectable_label(self.session.packet.selected_story.as_ref() == Some(&story.name),
                            format!("{} · {:.3}", story.name, story.elevation_m)).clicked() { selected = Some(story.name.clone()); }
                    });
                }
            });
            ui.separator();
            ui.small(format!("{} entities · {} withheld · {} projection notes",self.session.entity_count,self.session.withheld,self.session.projection_notes));
            ui.small("Rectangular frame proxies; supported zero-thickness shells. Sections, axes, offsets, insertion and opening subtraction are unverified.");
            ui.small("Disabled stories do not occur in both cached plot orders. No group/Wall/Slab authority is inferred.");
        });
        if let Some(story) = selected {
            let _selection = self.session.select(Some(story));
        }
    }
    fn table(&mut self, ui: &mut egui::Ui) {
        let mut requested = None;
        egui::Panel::bottom("version-stored-values")
            .default_size(190.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.label(format!(
                    "Stored values · selected {} · shear {} · drift ratio · rejected {}",
                    self.session
                        .packet
                        .selected_story
                        .as_deref()
                        .unwrap_or("None"),
                    self.session.packet.shear.unit,
                    self.rejected
                ));
                egui::ScrollArea::both().show(ui, |ui| {
                    egui::Grid::new("version-stored-values-grid")
                        .striped(true)
                        .show(ui, |ui| {
                            for name in ["Story", "Drift +", "Drift −", "Signed shear"] {
                                ui.strong(name);
                            }
                            ui.end_row();
                            for (i, story) in self.session.packet.drift.stories.iter().enumerate() {
                                ui.add_enabled_ui(
                                    self.session.linked && self.session.common_story(story),
                                    |ui| {
                                        if ui
                                            .selectable_label(
                                                self.session.packet.selected_story.as_ref()
                                                    == Some(story),
                                                story,
                                            )
                                            .clicked()
                                        {
                                            requested = Some(story.clone());
                                        }
                                    },
                                );
                                for series in &self.session.packet.drift.series {
                                    ui.label(
                                        series.values[i]
                                            .map_or_else(|| "—".into(), |v| format!("{v:.6}")),
                                    );
                                }
                                let shear = self
                                    .session
                                    .packet
                                    .shear
                                    .stories
                                    .iter()
                                    .position(|s| s == story)
                                    .and_then(|j| {
                                        self.session
                                            .packet
                                            .shear
                                            .series
                                            .first()
                                            .and_then(|s| s.values[j])
                                    });
                                ui.label(shear.map_or_else(|| "—".into(), |v| format!("{v:.3}")));
                                ui.end_row();
                            }
                        });
                });
            });
        if let Some(story) = requested {
            let _selection = self.session.select(Some(story));
        }
    }
    fn sync(&mut self, bounds: Option<egui::Rect>, ctx: &egui::Context) {
        let key = (
            self.session.packet.generation,
            self.session.packet.selected_story.clone(),
            self.session.linked,
        );
        if self.chart.is_ready() && self.last_sent.as_ref() != Some(&key) {
            let result = serde_json::to_string(&self.session.packet).map_err(|_private_error| ())
                .and_then(|json| {
                    let linked = self.session.linked;
                    self.chart.webview.evaluate_script(&format!(
                        "window.resultBridge.apply({json}); document.getElementById('plots').style.pointerEvents={}; document.querySelector('.notice').textContent={};",
                        if linked {"'auto'"} else {"'none'"},
                        if linked {"'Cached snapshot only · source parity and analysis freshness unverified.'"} else {"'UNLINKED · selected source changed; previous values retained. Reopen required.'"}
                    )).map_err(|_private_error| ())
                });
            if result.is_ok() {
                self.last_sent = Some(key);
            } else {
                self.transport_error = true;
            }
        }
        let visible = ctx.input(|i| i.viewport().minimized != Some(true));
        if self
            .chart
            .place(bounds, ctx.pixels_per_point(), visible)
            .is_err()
        {
            self.transport_error = true;
        }
    }
}
impl eframe::App for Workbench {
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        for raw in self.chart.pending_messages() {
            if self.smoke.is_some() && self.probe_reply(&raw) {
                continue;
            }
            if self.transport_error || self.session.accept(&raw).is_err() {
                self.rejected = self.rejected.saturating_add(1);
            }
        }
        self.controls(ui);
        self.stories(ui);
        self.table(ui);
        let mut bounds = None;
        egui::Panel::right("version-two-cached-charts")
            .default_size(660.0)
            .resizable(true)
            .show(ui, |ui| {
                let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
                bounds = Some(rect.shrink(2.0));
            });
        egui::CentralPanel::default().show(ui, |ui| {
            self.viewport
                .show(ui, self.session.packet.selected_story.as_deref());
        });
        self.sync(bounds, ui.ctx());
        self.step_smoke(ui.ctx());
    }
}
fn choose(ui: &mut egui::Ui, label: &str, current: &mut String, values: &[String]) {
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!("{label}: {current}"))
        .show_ui(ui, |ui| {
            for value in values {
                ui.selectable_value(current, value.clone(), value);
            }
        });
}

struct Smoke {
    started: Instant,
    phase: u8,
    report: SmokeReport,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    version: u8,
    #[serde(rename = "type")]
    kind: String,
    stage: u8,
    source: String,
    generation: u64,
    story: Option<String>,
    drift: Option<String>,
    shear: Option<String>,
    positive: Option<f64>,
    negative: Option<f64>,
    force: Option<f64>,
}
impl Workbench {
    fn probe_reply(&mut self, raw: &str) -> bool {
        let Ok(probe) = serde_json::from_str::<Probe>(raw) else {
            return false;
        };
        if probe.version != 1 || probe.kind != "workbench_probe" {
            return false;
        }
        let Some(smoke) = &mut self.smoke else {
            return false;
        };
        let expected = match (smoke.phase, probe.stage) {
            (2, 1) => "Roof",
            (6, 2) => "L1",
            _ => {
                smoke.phase = 250;
                return true;
            }
        };
        let pass = probe.source == self.session.packet.source
            && probe.generation == self.session.packet.generation
            && probe.story.as_deref() == Some(expected)
            && probe.drift.as_deref() == Some(expected)
            && probe.shear.as_deref() == Some(expected)
            && probe.positive == Some(0.002)
            && probe.negative == Some(-0.0025)
            && probe.force == Some(7.0);
        if !pass {
            smoke.phase = 250;
            return true;
        }
        if probe.stage == 1 {
            match self
                .session
                .snapshot
                .initial_config(Direction::Y)
                .and_then(|c| self.session.configure(c).map_err(str::to_owned))
            {
                Ok(()) => smoke.phase = 3,
                Err(_private_error) => smoke.phase = 250,
            }
        } else {
            smoke.phase = 5;
        }
        true
    }
    fn request_probe(&self, stage: u8) -> bool {
        self.chart.webview.evaluate_script(&format!("window.ipc.postMessage(JSON.stringify({{version:1,type:'workbench_probe',stage:{stage},source:active.source,generation:active.generation,story:active.selected_story,drift:charts.drift.getOption().series[0].markLine.data[0]?.yAxis??null,shear:charts.shear.getOption().series[0].markLine.data[0]?.yAxis??null,positive:active.drift.series[0].values[0],negative:active.drift.series[1].values[0],force:active.shear.series[0].values[0]}}));")).is_ok()
    }
    fn step_smoke(&mut self, ctx: &egui::Context) {
        let Some(mut smoke) = self.smoke.take() else {
            return;
        };
        let stats = self.viewport.gpu_stats();
        let current_sent = self.last_sent.as_ref().is_some_and(|(g, s, linked)| {
            *g == self.session.packet.generation
                && *s == self.session.packet.selected_story
                && *linked
        });
        match smoke.phase {
            0 if self.chart.is_ready() && stats.uploads == 1 => {
                smoke.phase = if self.session.select(Some("Roof".into())).is_ok() {
                    1
                } else {
                    250
                };
            }
            1 if current_sent && stats.selection_draw_records > 0 => {
                smoke.phase = if self.request_probe(1) { 2 } else { 250 };
            }
            3 if current_sent => {
                let event = |generation, story| serde_json::json!({"version":1,"type":"select_story","source":self.session.packet.source,"generation":generation,"plot":"shear","story":story});
                let old = event(self.session.packet.generation - 1, "Roof").to_string();
                let fresh = event(self.session.packet.generation, "L1").to_string();
                smoke.phase = if self.chart.webview.evaluate_script(&format!("window.ipc.postMessage(JSON.stringify({old}));window.ipc.postMessage(JSON.stringify({fresh}));")).is_ok() { 4 } else { 250 };
            }
            4 if current_sent
                && self.session.packet.selected_story.as_deref() == Some("L1")
                && self.rejected == 1
                && stats.selection_bytes == 0 =>
            {
                // Avoid issuing another readback while waiting for its reply.
                if !self.request_probe(2) {
                    smoke.phase = 250;
                } else {
                    smoke.phase = 6;
                }
            }
            _ => {}
        }
        // Readback stage2 is requested once in phase6; the reply transitions to5.
        if smoke.started.elapsed() > Duration::from_secs(25) {
            smoke.phase = 250;
        }
        if self.transport_error {
            smoke.phase = 250;
        }
        if matches!(smoke.phase, 5 | 250) {
            let pass = smoke.phase == 5
                && self.session.linked
                && !self.session.report.source_current_certified
                && self.session.entity_count == 3
                && self.session.withheld == 0
                && stats.uploads == 1
                && stats.vertex_bytes + stats.index_bytes == self.session.mesh_bytes as u64
                && stats.selection_uploads == 1
                && stats.selection_draw_records > 0
                && self.rejected == 1;
            let value = serde_json::json!({"assessment":if pass {"PASS"} else {"FAIL"},"fixture":"LAB-REAL02-WORKBENCH-01-r1",
                "same_guarded_version":true,"native_to_two_chart_markers":pass,"typed_chart_to_native":pass,"stale_selection_refused":self.rejected,
                "source_entities":self.session.entity_count,"withheld":self.session.withheld,"vertices":self.viewport.mesh_counts().0,"triangles":self.viewport.mesh_counts().1,
                "mesh_uploads":stats.uploads,"vertex_bytes":stats.vertex_bytes,"index_bytes":stats.index_bytes,"selection_uploads":stats.selection_uploads,
                "selection_draw_records":stats.selection_draw_records,"draw_records":stats.draw_records,"selected_story":self.session.packet.selected_story,
                "manual_input_verified":false,"presented_frame_verified":false,"source_current_certified":false,"analysis_freshness_verified":false,
                "engineering_fidelity_certified":false,"browser_process_cleanup_verified":false});
            if let Ok(mut report) = smoke.report.lock() {
                *report = Some(value);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        self.smoke = Some(smoke);
        ctx.request_repaint_after(Duration::from_millis(25));
    }
}
