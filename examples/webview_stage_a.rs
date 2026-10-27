#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

#[cfg(target_os = "windows")]
#[path = "support/smoke_receipt.rs"]
mod smoke_receipt;

#[cfg(target_os = "windows")]
#[path = "support/webview_host.rs"]
mod webview_host;

#[path = "webview_stage_a/protocol.rs"]
mod protocol;

#[cfg(target_os = "windows")]
mod windows {
    use crate::smoke_receipt::{self, Control, Receipt};
    use std::sync::mpsc::{self, Receiver};

    use crate::protocol::{
        SelectionState, Story, host_selection_script, is_chart_ready, parse_chart_selection,
    };

    use eframe::egui;

    use wry::raw_window_handle::HasWindowHandle;

    const CHART_TEMPLATE: &str = include_str!("webview_stage_a/chart.html");
    #[expect(
        clippy::large_include_file,
        reason = "Bundle the verified Apache ECharts runtime for offline use"
    )]
    const ECHARTS_RUNTIME: &str = include_str!("../assets/vendor/echarts/echarts.min.js");
    const ECHARTS_MARKER: &str = "/*__ECHARTS_RUNTIME__*/";
    fn failure(message: impl Into<String>) -> eframe::Error {
        eframe::Error::AppCreation(Box::new(std::io::Error::other(message.into())))
    }

    pub fn run() -> eframe::Result {
        env_logger::init();
        let smoke_report = std::env::var_os("WEBVIEW_SMOKE_REPORT").map(std::path::PathBuf::from);
        let public = smoke_report.is_some();
        let control = Control::parse(
            std::env::var_os("TALOS_PUBLIC_SMOKE_CONTROL").as_deref(),
            public,
        )
        .map_err(failure)?;
        let (sender, verdict) = mpsc::sync_channel(1);
        let smoke_report = smoke_report
            .map(|path| Receipt::create(&path, control, sender))
            .transpose()
            .map_err(|error| failure(error.to_string()))?;
        let options = eframe::NativeOptions {
            renderer: eframe::Renderer::Wgpu,
            viewport: egui::ViewportBuilder::default()
                .with_app_id("talos-egui-webview-stage-a")
                .with_visible(smoke_report.is_none())
                .with_inner_size([1040.0, 700.0])
                .with_min_inner_size([760.0, 520.0]),
            ..Default::default()
        };
        eframe::run_native(
            "TALOS | offline WebView Stage A",
            options,
            Box::new(move |context| Ok(Box::new(WebviewStageA::new(context, smoke_report)))),
        )?;
        if public {
            smoke_receipt::require_pass(verdict.try_recv().ok()).map_err(failure)?;
        }
        Ok(())
    }

    struct WebviewStageA {
        selection: SelectionState,
        webview: Option<StandaloneWebView>,
        chart_visible: bool,
        last_error: Option<String>,
        rejected_messages: u64,
        smoke: Option<Smoke>,
    }

    struct Smoke {
        receipt: Receipt,
        started: std::time::Instant,
        phase: u8,
        query: Option<Receiver<String>>,
        query_value: Option<String>,
        diagnostic: Option<Receiver<String>>,
        diagnostic_value: Option<String>,
    }

    impl WebviewStageA {
        fn new(context: &eframe::CreationContext<'_>, smoke_report: Option<Receipt>) -> Self {
            context.egui_ctx.set_visuals(egui::Visuals::dark());
            let (webview, last_error) = match StandaloneWebView::new(context, &context.egui_ctx) {
                Ok(webview) => (Some(webview), None),
                Err(error) => (None, Some(error)),
            };
            Self {
                selection: SelectionState::default(),
                webview,
                chart_visible: true,
                last_error,
                rejected_messages: 0,
                smoke: smoke_report.map(|receipt| Smoke {
                    receipt,
                    started: std::time::Instant::now(),
                    phase: 0,
                    query: None,
                    query_value: None,
                    diagnostic: None,
                    diagnostic_value: None,
                }),
            }
        }

        fn run_smoke(&mut self, context: &egui::Context) {
            let Some(mut smoke) = self.smoke.take() else {
                return;
            };
            let mut finished = None;
            if smoke.diagnostic.is_none()
                && smoke.started.elapsed() > std::time::Duration::from_secs(2)
                && let Some(webview) = &self.webview
            {
                let (sender, receiver) = mpsc::channel();
                if webview.host.webview.evaluate_script_with_callback("JSON.stringify({scheme:location.protocol,state:document.readyState,bridge:typeof window.hostBridge,echarts:typeof window.echarts,scripts:document.scripts.length,error:window.__stageError,marker:window.__stageMarker})", move |value| { drop(sender.send(value)); }).is_ok() {
                        smoke.diagnostic = Some(receiver);
                    }
            }
            if let Some(result) = smoke
                .diagnostic
                .as_ref()
                .and_then(|receiver| receiver.try_recv().ok())
            {
                smoke.diagnostic_value = Some(result);
            }
            if smoke.started.elapsed() > std::time::Duration::from_secs(30) {
                finished = Some(false);
            } else if let Some(webview) = &mut self.webview {
                if webview.host.is_ready() {
                    match smoke.phase {
                        0 => {
                            if let Ok(story) = Story::try_from(5) {
                                self.selection.select(story);
                                smoke.phase = 1;
                            }
                        }
                        1 if webview.sent_story == Story::try_from(5).ok() => {
                            let (sender, receiver) = mpsc::channel();
                            if webview
                                .host
                                .webview
                                .evaluate_script_with_callback(
                                    "window.hostBridge.selectedStory()",
                                    move |value| {
                                        drop(sender.send(value));
                                    },
                                )
                                .is_ok()
                            {
                                smoke.query = Some(receiver);
                                smoke.phase = 2;
                            }
                        }
                        2 => {
                            if let Some(result) = smoke
                                .query
                                .as_ref()
                                .and_then(|receiver| receiver.try_recv().ok())
                            {
                                smoke.query_value = Some(result.clone());
                                if result.trim() != "5" {
                                    finished = Some(false);
                                } else if webview.host.webview.evaluate_script("window.ipc.postMessage(JSON.stringify({version:1,type:'select_story',story:2}));").is_ok() {
                                    smoke.phase = 3;
                                }
                            }
                        }
                        3 if self.selection.selected().get() == 2 => {
                            finished = Some(true);
                        }
                        _ => {}
                    }
                }
            } else {
                finished = Some(false);
            }
            if let Some(passed) = finished {
                let passed = smoke
                    .receipt
                    .qualify_pass(passed && self.last_error.is_none());
                let report = serde_json::json!({
                    "assessment": if passed { "PASS" } else { "FAIL" },
                    "scope": "programmatic native-to-JS selection readback and JS-to-Rust IPC only",
                    "webview2": wry::webview_version().ok(),
                    "native_to_chart_story": 5,
                    "phase": smoke.phase,
                    "script_result": smoke.query_value,
                    "diagnostic": smoke.diagnostic_value,
                    "chart_to_native_story": self.selection.selected().get(),
                    "manual_input_tested": false,
                    "network_capture_performed": false,
                    "error": self.last_error,
                    "controlled_failure": smoke.receipt.forced_failure(),
                    "navigation_attempts": self.webview.as_ref().and_then(|webview| webview.host.navigation.lock().ok().map(|items| items.clone())),
                });
                if let Err(error) = smoke.receipt.finish(report.to_string().as_bytes(), passed) {
                    log::error!("write smoke report: {error}");
                }
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.smoke = Some(smoke);
                context.request_repaint_after(std::time::Duration::from_millis(10));
            }
        }

        fn poll_chart_messages(&mut self) {
            let Some(webview) = &mut self.webview else {
                return;
            };
            for message in webview.pending_messages() {
                match parse_chart_selection(&message) {
                    Ok(story) => {
                        self.selection.select(story);
                    }
                    Err(error) => {
                        self.rejected_messages = self.rejected_messages.saturating_add(1);
                        self.last_error = Some(error.to_string());
                    }
                }
            }
        }
    }

    impl eframe::App for WebviewStageA {
        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            self.poll_chart_messages();

            egui::Panel::top("stage-a-header").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Offline ECharts / Wry Stage A");
                    ui.separator();
                    ui.label("Rust owns selected-story state");
                });
            });

            egui::Panel::left("stage-a-controls")
                .default_size(230.0)
                .show(ui, |ui| {
                    ui.heading("Story selection");
                    ui.label("Synthetic load case: SYNTH_DBE_X");
                    ui.label("Units: drift ratio");
                    ui.separator();
                    for story in Story::all() {
                        let selected = self.selection.selected() == story;
                        if ui
                            .selectable_label(selected, format!("Story {}", story.get()))
                            .clicked()
                        {
                            self.selection.select(story);
                        }
                    }
                    ui.separator();
                    ui.strong(format!(
                        "Authoritative selection: Story {}",
                        self.selection.selected().get()
                    ));
                    ui.checkbox(&mut self.chart_visible, "Chart visible");

                    if self.webview.is_some() {
                        if ui.button("Close WebView").clicked() {
                            self.webview = None;
                        }
                    } else if ui.button("Reopen WebView").clicked() {
                        match StandaloneWebView::new(frame, ui.ctx()) {
                            Ok(webview) => {
                                self.webview = Some(webview);
                                self.last_error = None;
                            }
                            Err(error) => self.last_error = Some(error),
                        }
                    }

                    ui.label(format!(
                        "Rejected chart messages: {}",
                        self.rejected_messages
                    ));
                    if let Some(error) = &self.last_error {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    }
                    ui.separator();
                    ui.small(
                        "Use mouse tooltip/zoom/click in the chart. Focus the chart and use \
                         Up/Down to change stories.",
                    );
                });

            let mut chart_rect = None;
            egui::CentralPanel::default().show(ui, |ui| {
                let size = ui.available_size().max(egui::vec2(100.0, 100.0));
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 4.0, egui::Color32::from_rgb(18, 25, 35));
                chart_rect = Some(rect.shrink(4.0));
            });

            let minimized = ui.input(|input| input.viewport().minimized == Some(true));
            if let Some(webview) = &mut self.webview {
                if let Err(error) = webview.sync_selection(self.selection.selected()) {
                    self.last_error = Some(error);
                }
                if let Err(error) = webview.place(
                    chart_rect,
                    ui.ctx().pixels_per_point(),
                    self.chart_visible && !minimized,
                ) {
                    self.last_error = Some(error);
                }
            }
            self.run_smoke(ui.ctx());
        }
    }

    struct StandaloneWebView {
        host: crate::webview_host::ChildWebView,
        sent_story: Option<Story>,
    }
    impl StandaloneWebView {
        fn new(parent: &impl HasWindowHandle, context: &egui::Context) -> Result<Self, String> {
            Ok(Self {
                host: crate::webview_host::ChildWebView::new(
                    parent,
                    context,
                    chart_html()?,
                    is_chart_ready,
                )?,
                sent_story: None,
            })
        }
        fn pending_messages(&self) -> Vec<String> {
            self.host.pending_messages()
        }
        fn sync_selection(&mut self, story: Story) -> Result<(), String> {
            if !self.host.is_ready() || self.sent_story == Some(story) {
                return Ok(());
            }
            let script = host_selection_script(story)
                .map_err(|error| format!("serialize host selection: {error}"))?;
            self.host
                .webview
                .evaluate_script(&script)
                .map_err(|error| format!("send selection to chart: {error}"))?;
            self.sent_story = Some(story);
            Ok(())
        }
        fn place(
            &mut self,
            rect: Option<egui::Rect>,
            scale: f32,
            visible: bool,
        ) -> Result<(), String> {
            self.host.place(rect, scale, visible)
        }
    }
    fn chart_html() -> Result<String, String> {
        if CHART_TEMPLATE.matches(ECHARTS_MARKER).count() != 1 {
            return Err("chart template must contain one ECharts runtime marker".to_owned());
        }
        Ok(CHART_TEMPLATE.replacen(ECHARTS_MARKER, ECHARTS_RUNTIME, 1))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn chart_is_fully_embedded_and_csp_denies_network() {
            let html = chart_html().expect("static chart template is valid");
            assert!(html.contains(ECHARTS_RUNTIME));
            assert!(!html.contains(ECHARTS_MARKER));
            assert!(html.contains("connect-src 'none'"));
            assert!(html.contains("SYNTH_DBE_X"));
        }
    }
}

#[cfg(target_os = "windows")]
fn main() -> eframe::Result {
    windows::run()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("webview_stage_a is a Windows-only WebView2 experiment");
}
