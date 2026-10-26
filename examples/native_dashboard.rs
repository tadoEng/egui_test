//! Synthetic DASH-01 composite. No project/backend/persistence access.
#[cfg(target_os = "windows")]
#[path = "native_dashboard/host.rs"]
mod host;
#[cfg(target_os = "windows")]
#[path = "native_dashboard/model.rs"]
mod model;
#[cfg(target_os = "windows")]
#[path = "support/native_viewport.rs"]
mod native_viewport;
#[cfg(target_os = "windows")]
#[path = "support/webview_host.rs"]
mod webview_host;

#[cfg(target_os = "windows")]
fn main() -> eframe::Result {
    host::run()
}
#[cfg(not(target_os = "windows"))]
fn main() {}
