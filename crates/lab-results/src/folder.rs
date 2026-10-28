//! One version folder, read in place: never written, never scanned beyond the
//! fixed relative paths below.
use crate::data::{Snapshot, read_bounded};
use ext_core::version::VersionManifest;
use std::path::Path;

/// The model export a version folder must hold.
pub const MODEL: &str = "model.e2k";
/// The cached results a version folder may hold.
pub const RESULTS: &str = "results/calc_output.json";

/// The folder's `manifest.json`, through the pinned `ext-core` type.
///
/// # Errors
/// A missing, oversized, partial or invalid manifest.
pub fn read_manifest(folder: &Path) -> Result<VersionManifest, String> {
    refuse_partial(folder)?;
    let bytes = read_bounded(&inside(folder, "manifest.json")?)?;
    serde_json::from_slice(&bytes).map_err(|e| format!("Version manifest: {e}"))
}

/// The folder's `model.e2k` bytes.
///
/// # Errors
/// A missing, oversized or partial version, or a model outside the folder.
pub fn read_model(folder: &Path) -> Result<Vec<u8>, String> {
    refuse_partial(folder)?;
    read_bounded(&inside(folder, MODEL)?)
}

/// What a version folder holds in the way of cached results.
pub enum Cached {
    /// No `results/calc_output.json`: nothing to show, nothing refused.
    Absent,
    /// Read and checked by [`Snapshot::load`].
    Loaded(Box<Snapshot>),
    /// Present but refused; the reason is shown, nothing is plotted.
    Refused(String),
}

/// The folder's cached results, if it holds any.
#[must_use]
pub fn read_results(folder: &Path) -> Cached {
    if !folder.join(RESULTS).exists() {
        return Cached::Absent;
    }
    match Snapshot::load(folder) {
        Ok(snapshot) => Cached::Loaded(Box::new(snapshot)),
        Err(reason) => Cached::Refused(reason),
    }
}

fn refuse_partial(folder: &Path) -> Result<(), String> {
    if folder.join(".partial").exists() {
        return Err("Refusing an incomplete version (.partial)".into());
    }
    Ok(())
}

fn inside(folder: &Path, relative: &str) -> Result<std::path::PathBuf, String> {
    let root = folder
        .canonicalize()
        .map_err(|e| format!("Resolve version folder: {e}"))?;
    let path = root
        .join(relative)
        .canonicalize()
        .map_err(|e| format!("{relative}: {e}"))?;
    if !path.starts_with(&root) {
        return Err(format!("{relative} resolves outside the version folder"));
    }
    Ok(path)
}
