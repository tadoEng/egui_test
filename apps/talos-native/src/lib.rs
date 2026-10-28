//! TALOS native application shell (APP-01A): an experimental, read-only,
//! tab-first engineering application on public synthetic data.
//!
//! Engineering authority stays in the `EtabExtension` Rust crates: geometry,
//! source attribution and fidelity come from `lab-e2k-mesh` over the pinned
//! `ext-e2k`. This crate owns only application state, routing and layout.
pub mod app;
pub mod document;
pub mod loader;
pub mod native_view;
pub mod pages;
pub mod project;
pub mod state;
pub mod tabs;
pub mod theme;
pub mod view;

#[cfg(test)]
mod tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod gpu_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod ui_tests;
