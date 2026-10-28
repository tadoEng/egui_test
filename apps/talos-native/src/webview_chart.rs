//! Windows: the Results chart as ECharts in a `WebView2` child window.
//!
//! The transport is the bounded offline child `WebView` the Windows examples
//! already use (`examples/support/webview_host.rs`): authored HTML only, no
//! navigation, no new windows, no downloads, IPC messages of at most 1 KiB.
//! The ECharts runtime is the pinned offline asset in `assets/vendor/echarts`.
//! What the chart is sent and what it may answer is [`crate::chart_bridge`].
#[path = "../../../examples/support/webview_host.rs"]
mod webview_host;

use crate::chart_bridge::{Bridge, Content, EmbeddedChart, is_ready};
use crate::state::Action;
use webview_host::ChildWebView;

const MARKER: &str = "/*__ECHARTS_RUNTIME__*/";
#[expect(
    clippy::large_include_file,
    reason = "the pinned offline ECharts asset, as in native_results"
)]
const ECHARTS: &str = include_str!("../../../assets/vendor/echarts/echarts.min.js");
const PAGE: &str = include_str!("../assets/results_chart.html");

pub struct WebViewChart {
    view: ChildWebView,
    bridge: Bridge,
    /// Set once the child window failed; the page then draws the chart.
    failed: Option<String>,
    navigation_reported: bool,
}

impl WebViewChart {
    /// Creates the hidden child window over the application window.
    ///
    /// # Errors
    /// `WebView2` could not be created (for example, no runtime installed).
    pub fn new(cc: &eframe::CreationContext<'_>) -> Result<Self, String> {
        let html = PAGE.replacen(MARKER, ECHARTS, 1);
        let view = ChildWebView::new(cc, &cc.egui_ctx, html, is_ready)?;
        Ok(Self {
            view,
            bridge: Bridge::default(),
            failed: None,
            navigation_reported: false,
        })
    }

    fn fail(&mut self, error: String) {
        log::warn!("ECharts chart disabled, the native chart is used: {error}");
        // Best effort: a child window that cannot be placed should not stay up.
        if let Err(hide) = self.view.webview.set_visible(false) {
            log::warn!("hide the chart window: {hide}");
        }
        self.failed = Some(error);
    }
}

impl EmbeddedChart for WebViewChart {
    fn available(&self) -> bool {
        self.failed.is_none()
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
        // Messages are checked against the frame they were sent about, before
        // this frame's content replaces it.
        let mut actions = Vec::new();
        for message in self.view.pending_messages() {
            match self.bridge.accept(&message) {
                Ok(action) => actions.push(action),
                Err(refused) => log::debug!(
                    "chart message refused ({refused:?}; {} so far)",
                    self.bridge.refused()
                ),
            }
        }
        if let Err(error) = self.bridge.update(content) {
            self.fail(error);
            return actions;
        }
        if self.view.is_ready()
            && let Some(frame) = self.bridge.outgoing()
        {
            let sent = frame.and_then(|json| {
                self.view
                    .webview
                    .evaluate_script(&format!("window.talosChart.apply({json});"))
                    .map_err(|error| format!("send chart frame: {error}"))
            });
            if let Err(error) = sent {
                log::warn!("{error}; retrying");
                self.bridge.resend();
            }
        }
        if !self.navigation_reported
            && self
                .view
                .navigation
                .lock()
                .is_ok_and(|entries| entries.contains(&"denied navigation"))
        {
            log::warn!("the chart page attempted a navigation; it was blocked");
            self.navigation_reported = true;
        }
        // Shown only once the page is ready and has been sent the frame, so
        // it never flashes an empty page over the card.
        let visible = area.is_some() && self.view.is_ready() && self.bridge.frame().is_some();
        if let Err(error) = self.view.place(area, ctx.pixels_per_point(), visible) {
            self.fail(error);
        }
        actions
    }
}
