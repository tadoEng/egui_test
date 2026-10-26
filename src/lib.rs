#![warn(clippy::all, rust_2018_idioms)]

mod app;
#[cfg(all(feature = "e2k", not(target_arch = "wasm32")))]
mod e2k_import;
#[cfg(all(feature = "native_backend", not(target_arch = "wasm32")))]
pub mod native_backend;
mod scene;
pub use app::{SceneStats, TemplateApp};
pub use scene::SyntheticPreset;
