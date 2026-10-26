//! Explicit read-only cached-result artifact viewer, without project operations.
#[cfg(target_os = "windows")]
#[path = "native_results/data.rs"]
mod data;
#[cfg(target_os = "windows")]
#[path = "native_results/fixture.rs"]
mod fixture;
#[cfg(target_os = "windows")]
#[path = "native_results/host.rs"]
mod host;
#[cfg(target_os = "windows")]
#[path = "support/webview_host.rs"]
mod webview_host;

#[cfg(target_os = "windows")]
fn main() -> eframe::Result {
    host::run()
}
#[cfg(not(target_os = "windows"))]
fn main() {}
