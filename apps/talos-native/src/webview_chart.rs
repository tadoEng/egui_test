//! Windows adapter for the existing bounded offline child WebView host.
#[path = "../../../examples/support/webview_host.rs"]
mod webview_host;

use crate::chart_bridge::{Bridge, Content, EmbeddedChart, is_ready};
use crate::state::Action;
use std::time::{Duration, Instant};
use webview_host::ChildWebView;
use wry::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};

const MARKER: &str = "/*__ECHARTS_RUNTIME__*/";
#[expect(
    clippy::large_include_file,
    reason = "Reuse the pinned offline ECharts asset"
)]
const ECHARTS: &str = include_str!("../../../assets/vendor/echarts/echarts.min.js");
const PAGE: &str = include_str!("../assets/results_chart.html");

pub struct WebViewChart {
    // Dropping a failed child removes its HWND even if hiding failed.
    view: Option<ChildWebView>,
    bridge: Bridge,
    failed: Option<String>,
    parent: isize,
    visible: bool,
    started: Instant,
}

impl WebViewChart {
    /// Create the hidden child using the actual native parent, not CreationContext.
    /// # Errors
    /// No Windows parent, missing WebView2 runtime, or child creation failure.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let parent = cc
            .winit_window
            .ok_or("WebView2 requires a native parent window")?;
        let RawWindowHandle::Win32(handle) =
            parent.window_handle().map_err(|e| e.to_string())?.as_raw()
        else {
            return Err("WebView2 requires a Windows parent".into());
        };
        let html = PAGE.replacen(MARKER, ECHARTS, 1);
        let view = ChildWebView::new(parent, &cc.egui_ctx, html, is_ready)?;
        Ok(Self {
            view: Some(view),
            bridge: Bridge::default(),
            failed: None,
            parent: handle.hwnd.get(),
            visible: false,
            started: Instant::now(),
        })
    }

    fn fail(&mut self, error: String) {
        self.failed = Some(error);
        self.view = None;
        self.visible = false;
        // Generation is never reused; no failed/hidden chart can issue an action.
        let _cleared = self.bridge.update(None);
    }

    #[expect(
        unsafe_code,
        reason = "Read focus ownership within our own native parent only"
    )]
    fn child_has_focus(&self) -> bool {
        use windows_sys::Win32::UI::{
            Input::KeyboardAndMouse::GetFocus, WindowsAndMessaging::IsChild,
        };
        let parent = self.parent as windows_sys::Win32::Foundation::HWND;
        let focus = unsafe { GetFocus() };
        !focus.is_null() && unsafe { IsChild(parent, focus) } != 0
    }

    fn sync_checked(
        &mut self,
        ctx: &egui::Context,
        area: Option<egui::Rect>,
        content: Option<Content>,
    ) -> Result<Vec<Action>, String> {
        // Invalidate the old frame before draining queued messages. This also
        // refuses a late selection or shortcut on Model or a changed query.
        self.bridge.update(content)?;
        let restore_focus = self.visible && area.is_none() && self.child_has_focus();
        let Some(view) = &mut self.view else {
            return Ok(Vec::new());
        };
        if view
            .navigation
            .lock()
            .map_err(|_| "WebView2 navigation state unavailable")?
            .contains(&"denied navigation")
        {
            return Err("WebView2 refused unexpected navigation".into());
        }
        if !view.is_ready() {
            if self.started.elapsed() > Duration::from_secs(30) {
                return Err("ECharts / WebView2 offline bootstrap timed out".into());
            }
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        let mut actions = Vec::new();
        for raw in view.pending_messages() {
            if serde_json::from_str::<serde_json::Value>(&raw)
                .is_ok_and(|v| v == serde_json::json!({"protocol":1,"type":"chart_error"}))
            {
                return Err("ECharts reported a browser rendering error".into());
            }
            if let Ok(action) = self.bridge.accept(&raw) {
                let navigates = matches!(
                    action,
                    Action::SelectWorkspace(_) | Action::NextWorkspace | Action::PreviousWorkspace
                );
                actions.push(action);
                if navigates {
                    break;
                }
            }
        }
        if view.is_ready()
            && let Some(frame) = self.bridge.outgoing()
        {
            let json = frame?;
            view.webview
                .evaluate_script(&format!("window.talosChart.apply({json});"))
                .map_err(|e| format!("Send ECharts frame: {e}"))?;
        }
        let visible = area.is_some() && view.is_ready() && self.bridge.frame().is_some();
        view.place(area, ctx.pixels_per_point(), visible)?;
        if restore_focus {
            view.webview
                .focus_parent()
                .map_err(|e| format!("Restore native parent focus: {e}"))?;
        }
        self.visible = visible;
        Ok(actions)
    }
}

impl EmbeddedChart for WebViewChart {
    fn available(&self) -> bool {
        self.failed.is_none()
    }
    fn error(&self) -> Option<&str> {
        self.failed.as_deref()
    }
    fn sync(
        &mut self,
        ctx: &egui::Context,
        area: Option<egui::Rect>,
        content: Option<Content>,
    ) -> Vec<Action> {
        if self.failed.is_some() {
            return Vec::new();
        }
        match self.sync_checked(ctx, area, content) {
            Ok(actions) => actions,
            Err(error) => {
                self.fail(error);
                ctx.request_repaint();
                Vec::new()
            }
        }
    }
}
