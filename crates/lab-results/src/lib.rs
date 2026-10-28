//! Read-only presentation of cached `EtabExtension` result DTOs.
//!
//! [`data::Snapshot`] deserializes the authoritative `CalcOutput`,
//! `VersionManifest` and `SourceUnitMetadata` of one version directory (or of
//! bytes a caller already holds) through the pinned `ext-calc` / `ext-core`
//! types, and projects stored fields for plotting. Nothing is recalculated: no
//! envelope, maximum, sign change or unit conversion is inferred.
//!
//! The same source files are compiled into the Windows `native_results` and
//! `native_workbench` examples through `#[path]`, so there is one adapter.
//!
//! Native only: on wasm32 the crate is empty, as `ext-calc` cannot build there.
#![cfg(not(target_arch = "wasm32"))]
pub mod data;
pub mod fixture;
pub mod folder;
pub mod response;
