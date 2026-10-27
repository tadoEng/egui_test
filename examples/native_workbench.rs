//! One explicit read-only version: checked E2K buffers + stored engineering DTOs.
//! No project operations, recalculation, live ETABS, persistence or source certification.
#[cfg(target_os = "windows")]
#[path = "native_results/data.rs"]
#[expect(
    dead_code,
    reason = "The standalone path reader remains in native_results; this host constructs the same DTOs from preflight-held bytes"
)]
mod data;
#[cfg(target_os = "windows")]
#[path = "native_results/fixture.rs"]
mod fixture;
#[cfg(target_os = "windows")]
#[path = "native_workbench/host.rs"]
mod host;
#[cfg(target_os = "windows")]
#[path = "native_workbench/model.rs"]
mod model;
#[cfg(target_os = "windows")]
#[path = "support/native_viewport.rs"]
#[expect(
    dead_code,
    reason = "Legacy synthetic constructor/callback counter remain available in the standalone renderer/dashboard"
)]
mod native_viewport;
#[cfg(target_os = "windows")]
#[path = "native_results/preflight.rs"]
#[expect(
    dead_code,
    reason = "Shared categorical CLI lives in native_results; this host consumes its guarded byte bundle"
)]
mod preflight;
#[cfg(target_os = "windows")]
#[path = "support/webview_host.rs"]
mod webview_host;

#[cfg(target_os = "windows")]
fn main() -> eframe::Result {
    host::run()
}
#[cfg(not(target_os = "windows"))]
fn main() {}
