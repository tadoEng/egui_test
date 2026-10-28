//! Explicit read-only cached-result artifact viewer, without project operations.
#[cfg(target_os = "windows")]
#[path = "../crates/lab-results/src/data.rs"]
mod data;
#[cfg(target_os = "windows")]
#[path = "../crates/lab-results/src/fixture.rs"]
mod fixture;
#[cfg(target_os = "windows")]
#[path = "native_results/host.rs"]
mod host;
#[cfg(all(target_os = "windows", feature = "snapshot-preflight"))]
#[path = "native_results/preflight.rs"]
#[expect(
    dead_code,
    reason = "The shared guarded bundle is consumed by native_workbench; this example uses the categorical preflight CLI"
)]
mod preflight;
#[cfg(all(target_os = "windows", feature = "snapshot-preflight"))]
#[path = "native_results/preflight_public.rs"]
mod preflight_public;
#[cfg(target_os = "windows")]
#[path = "support/webview_host.rs"]
mod webview_host;

#[cfg(target_os = "windows")]
fn main() -> eframe::Result {
    host::run()
}
#[cfg(not(target_os = "windows"))]
fn main() {}
