#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

#[path = "webview_stage_a/protocol.rs"]
mod protocol;

#[cfg(target_os = "windows")]
mod windows {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    };

    use crate::protocol::{
        SelectionState, Story, host_selection_script, is_chart_ready, parse_chart_selection,
    };
    use base64::Engine as _;
    use eframe::egui;
    use wry::dpi::{PhysicalPosition, PhysicalSize};
    use wry::raw_window_handle::HasWindowHandle;
    use wry::{NewWindowResponse, Rect, WebView, WebViewBuilder};

    const CHART_TEMPLATE: &str = include_str!("webview_stage_a/chart.html");
    #[expect(
        clippy::large_include_file,
        reason = "Bundle the verified Apache ECharts runtime for offline use"
    )]
    const ECHARTS_RUNTIME: &str = include_str!("../assets/vendor/echarts/echarts.min.js");
    const ECHARTS_MARKER: &str = "/*__ECHARTS_RUNTIME__*/";

    pub fn run() -> eframe::Result {
        env_logger::init();
        let smoke_report = std::env::var_os("WEBVIEW_SMOKE_REPORT").map(std::path::PathBuf::from);
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
        )
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
        path: std::path::PathBuf,
        started: std::time::Instant,
        phase: u8,
        query: Option<Receiver<String>>,
        query_value: Option<String>,
        diagnostic: Option<Receiver<String>>,
        diagnostic_value: Option<String>,
    }

    impl WebviewStageA {
        fn new(
            context: &eframe::CreationContext<'_>,
            smoke_report: Option<std::path::PathBuf>,
        ) -> Self {
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
                smoke: smoke_report.map(|path| Smoke {
                    path,
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
            let Some(smoke) = &mut self.smoke else {
                return;
            };
            let mut finished = None;
            if smoke.diagnostic.is_none()
                && smoke.started.elapsed() > std::time::Duration::from_secs(2)
                && let Some(webview) = &self.webview
            {
                let (sender, receiver) = mpsc::channel();
                if webview.webview.evaluate_script_with_callback("JSON.stringify({url:location.href,state:document.readyState,bridge:typeof window.hostBridge,echarts:typeof window.echarts,scripts:document.scripts.length,error:window.__stageError,marker:window.__stageMarker})", move |value| { drop(sender.send(value)); }).is_ok() {
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
                if webview.loaded.load(Ordering::Acquire) {
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
                                } else if webview.webview.evaluate_script("window.ipc.postMessage(JSON.stringify({version:1,type:'select_story',story:2}));").is_ok() {
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
                    "navigation_attempts": self.webview.as_ref().and_then(|webview| webview.navigation.lock().ok().map(|items| items.clone())),
                });
                if let Err(error) = std::fs::write(&smoke.path, report.to_string()) {
                    log::error!("write smoke report: {error}");
                }
                self.smoke = None;
                context.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
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
        webview: WebView,
        messages: Receiver<String>,
        last_bounds: Option<PhysicalBounds>,
        last_visible: bool,
        sent_story: Option<Story>,
        loaded: Arc<AtomicBool>,
        navigation: Arc<Mutex<Vec<&'static str>>>,
    }

    impl StandaloneWebView {
        fn new(parent: &impl HasWindowHandle, context: &egui::Context) -> Result<Self, String> {
            let (sender, messages) = mpsc::sync_channel(16);
            let repaint = context.clone();
            let loaded = Arc::new(AtomicBool::new(false));
            let load_signal = Arc::clone(&loaded);
            let navigation = Arc::new(Mutex::new(Vec::new()));
            let navigation_log = Arc::clone(&navigation);
            let html = chart_html()?;
            let navigation_policy = NavigationPolicy::new(&html);
            let webview = WebViewBuilder::new()
                .with_html(html)
                .with_bounds(PhysicalBounds::hidden().into())
                .with_visible(false)
                .with_focused(false)
                .with_incognito(true)
                .with_clipboard(false)
                .with_autoplay(false)
                .with_hotkeys_zoom(false)
                .with_devtools(false)
                .with_general_autofill_enabled(false)
                .with_back_forward_navigation_gestures(false)
                .with_drag_drop_handler(|_| true)
                .with_download_started_handler(|_, _| false)
                .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
                .with_navigation_handler(move |url| {
                    if let Ok(mut entries) = navigation_log.lock()
                        && entries.len() < 16
                    {
                        entries.push(if navigation_policy.allows(&url) {
                            "allowed bootstrap"
                        } else {
                            "denied navigation"
                        });
                    }
                    navigation_policy.allows(&url)
                })
                .with_ipc_handler(move |request| {
                    if is_chart_ready(request.body()) {
                        load_signal.store(true, Ordering::Release);
                    } else if request.body().len() <= 1024 {
                        drop(sender.try_send(request.body().clone()));
                    }
                    repaint.request_repaint();
                })
                .build_as_child(parent)
                .map_err(|error| format!("create WebView2 child: {error}"))?;
            Ok(Self {
                webview,
                messages,
                last_bounds: None,
                last_visible: false,
                sent_story: None,
                loaded,
                navigation,
            })
        }

        fn pending_messages(&self) -> Vec<String> {
            self.messages.try_iter().collect()
        }

        fn sync_selection(&mut self, story: Story) -> Result<(), String> {
            // Do not mark a selection sent before the document's bridge exists.
            if !self.loaded.load(Ordering::Acquire) || self.sent_story == Some(story) {
                return Ok(());
            }
            let script = host_selection_script(story)
                .map_err(|error| format!("serialize host selection: {error}"))?;
            self.webview
                .evaluate_script(&script)
                .map_err(|error| format!("send selection to chart: {error}"))?;
            self.sent_story = Some(story);
            Ok(())
        }

        fn place(
            &mut self,
            rect: Option<egui::Rect>,
            pixels_per_point: f32,
            visible: bool,
        ) -> Result<(), String> {
            let bounds = rect.and_then(|rect| PhysicalBounds::from_egui(rect, pixels_per_point));
            let should_show = visible && bounds.is_some();
            if let Some(bounds) = bounds
                && self.last_bounds != Some(bounds)
            {
                self.webview
                    .set_bounds(bounds.into())
                    .map_err(|error| format!("resize WebView2 child: {error}"))?;
                self.last_bounds = Some(bounds);
            }
            if self.last_visible != should_show {
                self.webview
                    .set_visible(should_show)
                    .map_err(|error| format!("change WebView2 visibility: {error}"))?;
                self.last_visible = should_show;
            }
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct PhysicalBounds {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    }

    impl PhysicalBounds {
        const fn hidden() -> Self {
            Self {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            }
        }

        #[expect(clippy::cast_possible_truncation)]
        fn from_egui(rect: egui::Rect, pixels_per_point: f32) -> Option<Self> {
            if !rect.is_finite() || !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
                return None;
            }
            let min = rect.min * pixels_per_point;
            let size = rect.size() * pixels_per_point;
            if size.x < 1.0 || size.y < 1.0 {
                return None;
            }
            Some(Self {
                x: min.x.round() as i32,
                y: min.y.round() as i32,
                width: size.x.round() as u32,
                height: size.y.round() as u32,
            })
        }
    }

    impl From<PhysicalBounds> for Rect {
        fn from(bounds: PhysicalBounds) -> Self {
            Self {
                position: PhysicalPosition::new(bounds.x, bounds.y).into(),
                size: PhysicalSize::new(bounds.width, bounds.height).into(),
            }
        }
    }

    fn chart_html() -> Result<String, String> {
        if CHART_TEMPLATE.matches(ECHARTS_MARKER).count() != 1 {
            return Err("chart template must contain one ECharts runtime marker".to_owned());
        }
        Ok(CHART_TEMPLATE.replacen(ECHARTS_MARKER, ECHARTS_RUNTIME, 1))
    }

    struct NavigationPolicy {
        bootstrap_url: String,
    }

    impl NavigationPolicy {
        fn new(html: &str) -> Self {
            Self {
                bootstrap_url: format!(
                    "data:text/html;charset=utf-8;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(html.as_bytes())
                ),
            }
        }

        fn allows(&self, url: &str) -> bool {
            url == "about:blank" || url == self.bootstrap_url
        }
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

        #[test]
        fn navigation_policy_allows_only_internal_bootstrap_page() {
            let policy = NavigationPolicy::new("<p>authored bootstrap</p>");
            assert!(policy.allows("about:blank"));
            assert!(policy.allows(&policy.bootstrap_url));
            assert!(!policy.allows(&NavigationPolicy::new("<p>different bytes</p>").bootstrap_url));
            assert!(!policy.allows("https://example.com"));
            assert!(!policy.allows("file:///untrusted-input"));
            assert!(!policy.allows("javascript:alert(1)"));
        }

        #[test]
        fn egui_points_are_converted_to_physical_child_pixels() {
            let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(300.0, 200.0));
            for (scale, x, y, width, height) in [
                (1.0, 10, 20, 300, 200),
                (1.25, 13, 25, 375, 250),
                (1.5, 15, 30, 450, 300),
            ] {
                assert_eq!(
                    PhysicalBounds::from_egui(rect, scale),
                    Some(PhysicalBounds {
                        x,
                        y,
                        width,
                        height
                    })
                );
            }
        }

        #[test]
        fn invalid_or_empty_bounds_hide_the_child() {
            let empty = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO);
            assert_eq!(PhysicalBounds::from_egui(empty, 1.0), None);
            assert_eq!(PhysicalBounds::from_egui(egui::Rect::NAN, 1.0), None);
            assert_eq!(
                PhysicalBounds::from_egui(egui::Rect::EVERYTHING, f32::NAN),
                None
            );
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
