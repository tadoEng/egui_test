use crate::{
    data::{Choices, Config, Direction, Packet, Snapshot},
    webview_host::ChildWebView,
};
use std::{io::Write as _, path::PathBuf};

const MARKER: &str = "/*__ECHARTS_RUNTIME__*/";
#[expect(
    clippy::large_include_file,
    reason = "Reuse the existing pinned offline ECharts asset"
)]
const ECHARTS: &str = include_str!("../../assets/vendor/echarts/echarts.min.js");
fn html() -> String {
    include_str!("chart.html").replacen(MARKER, ECHARTS, 1)
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
fn failure(msg: impl Into<String>) -> eframe::Error {
    eframe::Error::AppCreation(Box::new(std::io::Error::other(msg.into())))
}

// This mode accepts NO source path: only the embedded independently authored
// fixture can enter its reports. Private artifacts have no export mode.
fn public_fixture_probe() -> Result<(), String> {
    let (input, mut calc) = crate::fixture::fixture().map_err(|e| e.to_string())?;
    // Authored gap variant: omit L1's drift row before reading the fixture.
    // Both drift series must retain that story as a gap; shear stays signed.
    calc["driftWind"]["x"]["rows"] = serde_json::json!([calc["driftWind"]["x"]["rows"][1].clone()]);
    std::fs::write(
        input.path().join("results/calc_output.json"),
        serde_json::to_vec(&calc).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let paths = [
        "manifest.json",
        "results/calc_output.json",
        "results/source-units.json",
    ];
    let before = paths
        .iter()
        .map(|p| std::fs::read(input.path().join(p)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let snapshot = Snapshot::load(input.path())?;
    let config = snapshot.initial_config(Direction::X)?;
    let packet = snapshot.packet(&config, 2, Some("Roof".into()))?;
    let after = paths
        .iter()
        .map(|p| std::fs::read(input.path().join(p)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    if before != after {
        return Err("Public fixture was modified".into());
    }
    let report = tempfile::Builder::new()
        .prefix("egui-cached-public-probe-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    std::fs::write(
        report.path().join("packet.json"),
        serde_json::to_vec_pretty(&packet).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let status = serde_json::json!({"assessment":"PASS","fixture":"LAB-CACHED-DTO-01-gap-r1","inputs_unchanged":true,"source":snapshot.fingerprint,"analysis_freshness_verified":false,"scope":"Owned authored gap fixture -> authoritative DTOs -> native Rust chart packet; no GUI or private input"});
    std::fs::write(
        report.path().join("status.json"),
        serde_json::to_vec_pretty(&status).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let receipt = serde_json::json!({"assessment":"PASS","report_dir":report.path(),"owned_input_dir":input.path()});
    let mut out = std::io::stdout().lock();
    out.write_all(receipt.to_string().as_bytes())
        .and_then(|()| out.write_all(b"\n"))
        .and_then(|()| out.flush())
        .map_err(|e| e.to_string())?;
    let _kept = report.keep();
    // Input TempDir is removed on return; the caller verifies disappearance and
    // keeps/copies only the public report before deleting that owned directory.
    Ok(())
}

pub(crate) fn run() -> eframe::Result {
    // Explicit artifact input only; no persistence, default project or API context.
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() == 1 && args[0] == "--public-fixture-probe" {
        return public_fixture_probe().map_err(failure);
    }
    let check_only = args.iter().any(|a| a == "--check-only");
    let paths = args
        .iter()
        .filter(|a| *a != "--check-only")
        .collect::<Vec<_>>();
    if paths.len() != 1 {
        return Err(failure(
            "Usage: native_results <version-directory> [--check-only]",
        ));
    }
    let snapshot = Snapshot::load(&PathBuf::from(paths[0])).map_err(failure)?;
    let config = snapshot.initial_config(Direction::X).map_err(failure)?;
    let packet = snapshot.packet(&config, 1, None).map_err(failure)?;
    if check_only {
        return Ok(());
    }
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        persist_window: false,
        viewport: egui::ViewportBuilder::default()
            .with_app_id("talos-read-only-cached-results")
            .with_inner_size([1380.0, 900.0]),
        ..Default::default()
    };
    let choices = [
        snapshot.choices(Direction::X),
        snapshot.choices(Direction::Y),
    ];
    eframe::run_native(
        "TALOS | read-only cached results",
        options,
        Box::new(move |cc| {
            let chart = ChildWebView::new(cc, &cc.egui_ctx, html(), ready)
                .map_err(std::io::Error::other)?;
            Ok(Box::new(Viewer {
                snapshot,
                config,
                packet,
                chart,
                last_sent: None,
                error: None,
                rejected: 0,
                choices,
            }))
        }),
    )
}
struct Viewer {
    snapshot: Snapshot,
    config: Config,
    packet: Packet,
    chart: ChildWebView,
    last_sent: Option<(u64, Option<String>)>,
    error: Option<String>,
    rejected: u64,
    choices: [Result<Choices, String>; 2],
}
impl Viewer {
    fn select(&mut self, story: Option<String>) {
        self.packet.selected_story = story;
    }
    fn configure(&mut self, config: Config) {
        if config == self.config {
            return;
        }
        let Some(generation) = self.packet.generation.checked_add(1) else {
            self.error = Some("Selection generation exhausted".into());
            return;
        };
        match self.snapshot.packet(&config, generation, None) {
            Ok(packet) => {
                self.config = config;
                self.packet = packet;
                self.error = None;
            }
            Err(error) => self.error = Some(error),
        }
    }
}
fn choose(ui: &mut egui::Ui, label: &str, value: &mut String, choices: &[String]) {
    egui::ComboBox::from_id_salt(label)
        .selected_text(format!("{label}: {value}"))
        .show_ui(ui, |ui| {
            for choice in choices {
                ui.selectable_value(value, choice.clone(), choice);
            }
        });
}
impl eframe::App for Viewer {
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        for message in self.chart.pending_messages() {
            match self.packet.accept(&message) {
                Ok(story) => self.select(story),
                Err(_) => self.rejected = self.rejected.saturating_add(1),
            }
        }
        let mut config = self.config.clone();
        egui::Panel::top("cached-results-controls").show(ui,|ui|{
            ui.heading("Read-only cached result artifacts · no recalculation / no live ETABS");
            ui.label(format!("{} / {} · generated {} · source {}…",self.snapshot.manifest.branch,self.snapshot.manifest.id,self.snapshot.calc.meta.generated_at,&self.snapshot.fingerprint[..12]));
            ui.colored_label(egui::Color32::YELLOW,"Cached snapshot only: file identity/units checked; analysis freshness and source-model parity are NOT established.");
            if !self.snapshot.manifest.is_analyzed {ui.colored_label(egui::Color32::YELLOW,"The stored manifest is not marked analyzed. These are existing cached values, not verified current analysis.");}
            ui.horizontal_wrapped(|ui|{
                ui.selectable_value(&mut config.direction,Direction::X,"X");ui.selectable_value(&mut config.direction,Direction::Y,"Y");
                if config.direction!=self.config.direction && let Ok(initial)=self.snapshot.initial_config(config.direction){config=initial;}
                if let Ok(choices)=&self.choices[usize::from(config.direction!=Direction::X)]{
                    choose(ui,"Drift group",&mut config.drift_group,&choices.groups);choose(ui,"Drift case",&mut config.drift_case,&choices.cases);choose(ui,"Shear case",&mut config.shear_case,&choices.forces);
                }
                if ui.button("Clear selection").clicked(){self.select(None);}
            });
            ui.small(format!("Generation {} · rejected {} · missing data remains gaps; signed values and stored story order preserved",self.packet.generation,self.rejected));
            if let Some(error)=&self.error {ui.colored_label(egui::Color32::LIGHT_RED,error);}
            if self.chart.navigation.lock().is_ok_and(|entries|entries.contains(&"denied navigation")) {ui.colored_label(egui::Color32::YELLOW,"An external navigation attempt was blocked.");}
        });
        self.configure(config);
        let mut requested = None;
        egui::Panel::left("cached-story-table")
            .default_size(350.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.heading("Cached story values");
                ui.label(format!(
                    "Selected: {}",
                    self.packet.selected_story.as_deref().unwrap_or("None")
                ));
                ui.small(format!(
                    "Drift ratio (+ / −) · signed shear {}",
                    self.packet.shear.unit
                ));
                egui::ScrollArea::both().show(ui, |ui| {
                    egui::Grid::new("cached-values")
                        .striped(true)
                        .show(ui, |ui| {
                            ui.strong("Story");
                            ui.strong("Drift +");
                            ui.strong("Drift −");
                            ui.strong("Shear");
                            ui.end_row();
                            for (i, story) in self.packet.drift.stories.iter().enumerate() {
                                if ui
                                    .selectable_label(
                                        self.packet.selected_story.as_ref() == Some(story),
                                        story,
                                    )
                                    .clicked()
                                {
                                    requested = Some(story.clone());
                                }
                                for series in &self.packet.drift.series {
                                    ui.label(
                                        series.values[i]
                                            .map_or_else(|| "—".into(), |v| format!("{v:.6}")),
                                    );
                                }
                                let force = self
                                    .packet
                                    .shear
                                    .stories
                                    .iter()
                                    .position(|s| s == story)
                                    .and_then(|j| self.packet.shear.series[0].values[j]);
                                ui.label(force.map_or_else(|| "—".into(), |v| format!("{v:.3}")));
                                ui.end_row();
                            }
                        });
                });
            });
        if let Some(story) = requested {
            self.select(Some(story));
        }
        let mut bounds = None;
        egui::CentralPanel::default().show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
            bounds = Some(rect.shrink(2.0));
        });
        self.sync(bounds, ui.ctx());
    }
}
impl Viewer {
    fn sync(&mut self, bounds: Option<egui::Rect>, ctx: &egui::Context) {
        let key = (self.packet.generation, self.packet.selected_story.clone());
        if self.chart.is_ready() && self.last_sent.as_ref() != Some(&key) {
            match serde_json::to_string(&self.packet)
                .map_err(|e| e.to_string())
                .and_then(|json| {
                    self.chart
                        .webview
                        .evaluate_script(&format!("window.resultBridge.apply({json});"))
                        .map_err(|e| e.to_string())
                }) {
                Ok(()) => self.last_sent = Some(key),
                Err(error) => self.error = Some(error),
            }
        }
        let minimized = ctx.input(|i| i.viewport().minimized == Some(true));
        if let Err(error) = self.chart.place(bounds, ctx.pixels_per_point(), !minimized) {
            self.error = Some(error);
        }
    }
}
