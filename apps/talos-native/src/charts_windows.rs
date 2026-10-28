//! Windows adapter over the existing offline, bounded child-WebView host.
use super::{Packet, Scope};
use crate::state::{Action, State, Workspace};
use serde::Deserialize;
use std::{
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};
use wry::{
    WebViewExtWindows as _,
    raw_window_handle::{HasWindowHandle as _, RawWindowHandle},
};

#[path = "../../../examples/support/smoke_receipt.rs"]
mod smoke_receipt;
#[path = "../../../examples/support/webview_host.rs"]
mod webview_host;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    #[serde(rename = "type")]
    kind: String,
    label: String,
    scope: Option<Scope>,
    selected: Option<String>,
    stories: Vec<String>,
    values: Vec<Vec<Option<f64>>>,
    width: f64,
    height: f64,
}
pub(super) struct Host {
    child: webview_host::ChildWebView,
    payload: String,
    visible: bool,
    rect: Option<egui::Rect>,
    scale: f32,
    parent: isize,
    started: Instant,
    probe: Option<Probe>,
    browser_error: Option<String>,
    pub(super) smoke: Option<PublicSmoke>,
}
fn ready(raw: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(raw)
        .is_ok_and(|value| value == serde_json::json!({"protocol":1,"type":"chart_ready"}))
}
impl Host {
    pub(super) fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        if std::env::var_os("TALOS_PUBLIC_ECHARTS_FORCE_FAILURE").is_some() {
            return Err("Public smoke control: WebView2 creation failed".into());
        }
        let window = cc
            .winit_window
            .ok_or("WebView2 requires the native parent window")?;
        let RawWindowHandle::Win32(handle) =
            window.window_handle().map_err(|e| e.to_string())?.as_raw()
        else {
            return Err("WebView2 requires a Windows parent".into());
        };
        let html = include_str!("chart.html").replacen(
            "/*__ECHARTS_RUNTIME__*/",
            include_str!("../../../assets/vendor/echarts/echarts.min.js"),
            1,
        );
        let child = webview_host::ChildWebView::new(window, &cc.egui_ctx, html, ready)?;
        Ok(Self {
            child,
            payload: String::new(),
            visible: false,
            rect: None,
            scale: 1.0,
            parent: handle.hwnd.get(),
            started: Instant::now(),
            probe: None,
            browser_error: None,
            smoke: None,
        })
    }
    pub(super) fn fail_smoke(&mut self, ctx: &egui::Context, error: &str) {
        if let Some(mut smoke) = self.smoke.take() {
            smoke.finish(ctx, false, error);
        }
    }
    pub(super) fn messages(&self) -> Vec<String> {
        self.child.pending_messages()
    }
    pub(super) fn accept_probe(&mut self, raw: &str) -> bool {
        if let Ok(probe) = serde_json::from_str::<Probe>(raw) {
            if probe.kind == "probe" && self.smoke.is_some() {
                self.probe = Some(probe);
            }
            return true;
        }
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw)
            && value.get("type").and_then(|v| v.as_str()) == Some("chart_error")
        {
            self.browser_error = Some("ECharts reported a browser rendering error".into());
            return true;
        }
        false
    }
    fn execute(&self, script: &str) -> Result<(), String> {
        self.child
            .webview
            .evaluate_script(script)
            .map_err(|e| format!("ECharts script failed: {e}"))
    }
    pub(super) fn update(
        &mut self,
        packet: Option<&Packet>,
        rect: Option<egui::Rect>,
        scale: f32,
        visible: bool,
    ) -> Result<(), String> {
        if self
            .child
            .navigation
            .lock()
            .map_err(|_| "WebView2 navigation state unavailable")?
            .contains(&"denied navigation")
        {
            return Err("WebView2 refused unexpected navigation".into());
        }
        if let Some(error) = &self.browser_error {
            return Err(error.clone());
        }
        if !self.child.is_ready() && self.started.elapsed() > Duration::from_secs(30) {
            return Err(
                "ECharts/WebView2 did not complete its offline bootstrap within 30 seconds".into(),
            );
        }
        // Hiding the HWND can clear Windows focus. Record ownership before
        // hiding so Model reliably regains focus without stealing it from
        // another application.
        let restore_focus = self.visible && !visible && self.own_child_has_focus();
        self.child.place(rect, scale, visible)?;
        self.visible = visible;
        self.rect = rect;
        self.scale = scale;
        if restore_focus {
            self.child
                .webview
                .focus_parent()
                .map_err(|e| format!("Return WebView2 focus to native parent: {e}"))?;
        }
        if self.child.is_ready()
            && let Some(packet) = packet
        {
            let payload = serde_json::to_string(packet).map_err(|e| e.to_string())?;
            if payload != self.payload {
                self.execute(&format!("window.talos.receive({payload});"))?;
                self.payload = payload;
            }
        }
        Ok(())
    }
    #[expect(unsafe_code, reason = "Query focus only within our own parent HWND")]
    fn own_child_has_focus(&self) -> bool {
        use windows_sys::Win32::UI::{
            Input::KeyboardAndMouse::GetFocus, WindowsAndMessaging::IsChild,
        };
        let parent = self.parent as windows_sys::Win32::Foundation::HWND;
        let focus = unsafe { GetFocus() };
        !focus.is_null() && unsafe { IsChild(parent, focus) } != 0
    }
    // These read-only Windows queries concern only this app's HWND/controller.
    #[expect(
        unsafe_code,
        reason = "Public-only Windows probe queries our own child visibility, hit testing and focus"
    )]
    fn native_check(&self) -> Result<(bool, bool, bool), String> {
        use windows_sys::Win32::{
            Foundation::POINT,
            UI::{
                Input::KeyboardAndMouse::GetFocus,
                WindowsAndMessaging::{
                    CWP_SKIPDISABLED, CWP_SKIPINVISIBLE, CWP_SKIPTRANSPARENT,
                    ChildWindowFromPointEx,
                },
            },
        };
        let mut visible = Default::default();
        unsafe { self.child.webview.controller().IsVisible(&mut visible) }
            .map_err(|e| e.to_string())?;
        let center = self
            .rect
            .unwrap_or(egui::Rect::from_min_size(
                egui::pos2(200.0, 200.0),
                egui::vec2(200.0, 200.0),
            ))
            .center()
            * self.scale;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Validated owned-window client coordinates"
        )]
        let point = POINT {
            x: center.x.round() as i32,
            y: center.y.round() as i32,
        };
        let parent = self.parent as windows_sys::Win32::Foundation::HWND;
        let hit = unsafe {
            ChildWindowFromPointEx(
                parent,
                point,
                CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT,
            )
        };
        let focus = unsafe { GetFocus() };
        Ok((visible.as_bool(), hit == parent, focus == parent))
    }
    pub(super) fn smoke_step(
        &mut self,
        state: &State,
        ctx: &egui::Context,
        rejected: u64,
    ) -> Vec<Action> {
        let Some(mut smoke) = self.smoke.take() else {
            return Vec::new();
        };
        let result = smoke.step(self, state, ctx, rejected);
        match result {
            Ok(actions) if smoke.receipt.is_some() => {
                self.smoke = Some(smoke);
                actions
            }
            Ok(actions) => actions,
            Err(error) => {
                smoke.finish(ctx, false, &error);
                Vec::new()
            }
        }
    }
}

/// Explicit authored-fixture verification; never attaches to a version folder.
pub struct PublicSmoke {
    receipt: Option<smoke_receipt::Receipt>,
    phase: u8,
    started: Instant,
    before_rejected: u64,
    checks: Vec<&'static str>,
    old_scope: Option<Scope>,
}
impl PublicSmoke {
    /// Reserve a fresh owned receipt before creating either native window.
    /// # Errors
    /// Missing/invalid report path or a previously existing file.
    pub fn reserve(path: &Path) -> Result<(Self, mpsc::Receiver<bool>), String> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let control = smoke_receipt::Control::parse(
            std::env::var_os("TALOS_PUBLIC_SMOKE_CONTROL").as_deref(),
            true,
        )?;
        let receipt = smoke_receipt::Receipt::create(path, control, sender)
            .map_err(|e| format!("Reserve public ECharts report: {e}"))?;
        Ok((
            Self {
                receipt: Some(receipt),
                phase: 0,
                started: Instant::now(),
                before_rejected: 0,
                checks: Vec::new(),
                old_scope: None,
            },
            receiver,
        ))
    }
    /// # Errors
    /// The window closed without a completed passing receipt.
    pub fn require_pass(verdict: Option<bool>) -> Result<(), &'static str> {
        smoke_receipt::require_pass(verdict)
    }
    fn finish(&mut self, ctx: &egui::Context, passed: bool, error: &str) {
        if let Some(receipt) = self.receipt.take() {
            let passed = receipt.qualify_pass(passed);
            let report = serde_json::json!({"assessment":if passed{"PASS"}else{"FAIL"},"error":error,
                "checks":self.checks,"controlled_failure":receipt.forced_failure(),
                "fixture":"LAB-REAL01","physical_input_tested":false,"manual_dpi_tested":false,
                "browser_process_cleanup_certified":false});
            let _written = receipt.finish(report.to_string().as_bytes(), passed);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    fn step(
        &mut self,
        host: &mut Host,
        state: &State,
        ctx: &egui::Context,
        rejected: u64,
    ) -> Result<Vec<Action>, String> {
        if self.started.elapsed() > Duration::from_secs(40) {
            return Err(format!("Public ECharts phase {} timed out", self.phase));
        }
        ctx.request_repaint_after(Duration::from_millis(20));
        match self.phase {
            0 if state.session.ready().is_some() => {
                self.phase = 1;
                return Ok(vec![Action::SelectWorkspace(Workspace::Results)]);
            }
            1 if host.child.is_ready() && !host.payload.is_empty() => {
                host.execute("window.talos.probe('rendered')")?;
                self.phase = 2;
            }
            2 => {
                if let Some(probe) = host.probe.take() {
                    if probe.label != "rendered"
                        || probe.stories != ["Roof", "L1"]
                        || probe.values
                            != [
                                vec![Some(0.002), Some(0.001)],
                                vec![Some(-0.0025), Some(-0.0015)],
                            ]
                        || probe.width < 10.0
                        || probe.height < 10.0
                    {
                        return Err("ECharts did not render the stored fixture profile".into());
                    }
                    self.old_scope = probe.scope;
                    self.checks
                        .push("actual ECharts stored values and dimensions");
                    host.execute("window.talos.click('Roof')")?;
                    self.phase = 3;
                }
            }
            3 if state.results.story.as_deref() == Some("Roof") => {
                self.checks
                    .push("scripted DOM chart click selects native story/table");
                self.before_rejected = rejected;
                host.execute("window.talos.stale()")?;
                self.phase = 4;
            }
            4 if rejected > self.before_rejected => {
                self.checks.push("actual stale WebView IPC refused");
                self.phase = 5;
                return Ok(vec![Action::SelectResultStory(Some("L1".into()))]);
            }
            5 if state.results.story.as_deref() == Some("L1") => {
                host.execute("window.talos.probe('table')")?;
                self.phase = 6;
            }
            6 => {
                if let Some(probe) = host.probe.take() {
                    if probe.label != "table" || probe.selected.as_deref() != Some("L1") {
                        return Err("Native selection did not update ECharts marker".into());
                    }
                    host.child.webview.focus().map_err(|e| e.to_string())?;
                    self.checks
                        .push("native table authority updates ECharts marker");
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        940.0, 720.0,
                    )));
                    self.phase = 7;
                }
            }
            7 if ctx.input(|i| i.content_rect().width()) < 1000.0 => {
                host.execute("window.talos.probe('resized')")?;
                self.phase = 8;
            }
            8 => {
                if let Some(probe) = host.probe.take() {
                    if probe.label != "resized" || probe.width < 10.0 || probe.height < 10.0 {
                        return Err("Resized ECharts is empty".into());
                    }
                    let bounds = host.child.webview.bounds().map_err(|e| e.to_string())?;
                    let physical = bounds.size.to_physical::<u32>(1.0);
                    if (f64::from(physical.width) / f64::from(host.scale) - probe.width).abs() > 2.0
                        || (f64::from(physical.height) / f64::from(host.scale) - probe.height).abs()
                            > 2.0
                    {
                        // ResizeObserver and the HWND resize are asynchronous. Wait for
                        // the actual chart to catch up; the overall deadline still applies.
                        host.execute("window.talos.probe('resized')")?;
                        return Ok(Vec::new());
                    }
                    self.checks
                        .push("native resize matches actual ECharts canvas dimensions");
                    self.phase = 9;
                    return Ok(vec![Action::SelectWorkspace(Workspace::Model)]);
                }
            }
            9 if state.workspace == Workspace::Model => {
                let (visible, parent_hit, focus_parent) = host.native_check()?;
                if visible || !parent_hit || !focus_parent {
                    return Err(
                        "Hidden WebView2 still visible, hit-testable or focused in Model".into(),
                    );
                }
                self.checks.push("Model: controller hidden, native child hit-test returns parent, focus restored");
                self.phase = 10;
                return Ok(vec![Action::SelectWorkspace(Workspace::Results)]);
            }
            10 if state.workspace == Workspace::Results && host.visible => {
                host.execute("window.talos.probe('returned')")?;
                self.phase = 11;
            }
            11 => {
                if let Some(probe) = host.probe.take() {
                    if probe.label != "returned"
                        || probe.selected.as_deref() != Some("L1")
                        || probe.scope == self.old_scope
                    {
                        return Err("Results return did not restore scoped chart state".into());
                    }
                    self.checks
                        .push("Model to Results restores selection with a fresh chart generation");
                    self.phase = 12;
                    return Ok(vec![Action::SelectWorkspace(Workspace::Model)]);
                }
            }
            12 if state.workspace == Workspace::Model => {
                let (visible, parent_hit, focus_parent) = host.native_check()?;
                if visible || !parent_hit || !focus_parent {
                    return Err("Second Model return is intercepted by WebView2".into());
                }
                self.checks
                    .push("Results to Model again: child hidden and no input interception");
                self.finish(ctx, true, "");
            }
            _ => {}
        }
        Ok(Vec::new())
    }
}
