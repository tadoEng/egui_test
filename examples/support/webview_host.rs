//! Shared bounded offline child `WebView` transport and physical placement policy.
use base64::Engine as _;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};
use wry::dpi::{PhysicalPosition, PhysicalSize};
use wry::raw_window_handle::HasWindowHandle;
use wry::{NewWindowResponse, Rect, WebView, WebViewBuilder};
pub(crate) struct ChildWebView {
    pub(crate) webview: WebView,
    messages: Receiver<String>,
    last_bounds: Option<PhysicalBounds>,
    last_visible: bool,
    loaded: Arc<AtomicBool>,
    pub(crate) navigation: Arc<Mutex<Vec<&'static str>>>,
}

impl ChildWebView {
    pub(crate) fn is_ready(&self) -> bool {
        self.loaded.load(Ordering::Acquire)
    }
    pub(crate) fn new(
        parent: &impl HasWindowHandle,
        context: &egui::Context,
        html: String,
        ready: fn(&str) -> bool,
    ) -> Result<Self, String> {
        let (sender, messages) = mpsc::sync_channel(16);
        let repaint = context.clone();
        let loaded = Arc::new(AtomicBool::new(false));
        let load_signal = Arc::clone(&loaded);
        let navigation = Arc::new(Mutex::new(Vec::new()));
        let navigation_log = Arc::clone(&navigation);
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
                if request.body().len() > 1024 {
                    return;
                }
                if ready(request.body()) {
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
            loaded,
            navigation,
        })
    }

    pub(crate) fn pending_messages(&self) -> Vec<String> {
        self.messages.try_iter().take(16).collect()
    }

    pub(crate) fn place(
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_allows_only_exact_authored_bootstrap() {
        let policy = NavigationPolicy::new("<p>authored bootstrap</p>");
        assert!(policy.allows("about:blank"));
        assert!(policy.allows(&policy.bootstrap_url));
        for url in [
            "https://example.com",
            "file:///untrusted-input",
            "javascript:alert(1)",
            &NavigationPolicy::new("other").bootstrap_url,
        ] {
            assert!(!policy.allows(url), "reject untrusted URL");
        }
    }
    #[test]
    fn physical_bounds_follow_egui_scale() {
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
    fn invalid_or_empty_geometry_hides_child() {
        let rect = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(300.0, 200.0));
        assert_eq!(PhysicalBounds::from_egui(egui::Rect::NAN, 1.0), None);
        assert_eq!(
            PhysicalBounds::from_egui(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ZERO),
                1.0
            ),
            None
        );
        assert_eq!(PhysicalBounds::from_egui(rect, f32::NAN), None);
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
