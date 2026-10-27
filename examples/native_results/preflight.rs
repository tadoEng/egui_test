//! REAL-01: bounded, read-only snapshot checks. Reports contain categories only.
//! Geometry/result consistency is not proof of committed source/analysis freshness.
use crate::data::{Direction, Snapshot};
use ext_calc::source_units::SourceUnitMetadata;
use ext_e2k::{ForceUnit, LengthUnit, TempUnit};
use ext_sidecar::ModelMetadata;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::os::windows::fs::OpenOptionsExt as _;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{Read as _, Seek as _, Write as _},
    path::{Path, PathBuf},
    time::SystemTime,
};

const MAX_BYTES: u64 = 64 * 1024 * 1024;
// Original I/O/parser errors may embed private paths or values; deliberately emit only categorical codes.
type Refusal = &'static str;

pub(crate) struct Options {
    pub(crate) root: PathBuf,
    pub(crate) e2k: Option<PathBuf>,
    pub(crate) allow_external: bool,
    pub(crate) expected_sha256: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct Report {
    pub(crate) refusal: Option<&'static str>,
    pub(crate) geometry_ready: bool,
    pub(crate) result_dto_ready: bool,
    pub(crate) supported_plots_ready: bool,
    pub(crate) manifest_analysis: &'static str,
    pub(crate) edb_presence: &'static str,
    pub(crate) e2k_state: &'static str,
    pub(crate) e2k_scope: &'static str,
    pub(crate) metadata_analysis: &'static str,
    pub(crate) metadata_chronology: &'static str,
    pub(crate) metadata_story_coherence: &'static str,
    pub(crate) analysis_execution_evidence: &'static str,
    pub(crate) metadata_units: &'static str,
    pub(crate) analysis_state_relation: &'static str,
    pub(crate) engineering_check_status: &'static str,
    pub(crate) unit_coherence: &'static str,
    pub(crate) story_coherence: &'static str,
    pub(crate) same_snapshot_linkage: &'static str,
    pub(crate) source_current_certified: bool,
    pub(crate) load_integrity: &'static str,
    pub(crate) fidelity_limitations: [&'static str; 3],
}
impl Default for Report {
    fn default() -> Self {
        Self {
            refusal: None,
            geometry_ready: false,
            result_dto_ready: false,
            supported_plots_ready: false,
            manifest_analysis: "unknown",
            edb_presence: "unknown",
            e2k_state: "not_checked",
            e2k_scope: "not_checked",
            metadata_analysis: "missing",
            metadata_chronology: "unknown",
            metadata_story_coherence: "unknown",
            analysis_execution_evidence: "cached_artifact_only_not_execution_proof",
            metadata_units: "unknown",
            analysis_state_relation: "unknown",
            engineering_check_status: "unknown",
            unit_coherence: "unknown",
            story_coherence: "unknown",
            same_snapshot_linkage: "unverified",
            source_current_certified: false,
            load_integrity: "not_verified",
            fidelity_limitations: [
                "project_scene_connectivity_is_approximate",
                "sections_offsets_axes_openings_not_certified",
                "no_member_results_or_source_current_certification",
            ],
        }
    }
}
impl Report {
    pub(crate) fn can_show_uncertified_preview(&self) -> bool {
        self.refusal.is_none()
            && self.geometry_ready
            && self.result_dto_ready
            && self.supported_plots_ready
            && self.unit_coherence == "compatible"
            && self.same_snapshot_linkage == "unverified"
            && self.load_integrity == "verified_at_load"
    }
}

// Open the verified canonical target, not the original joined path. Retain a
// cooperative Windows FILE_SHARE_READ-only handle through the whole inspection.
// Hostile directory/reparse replacement is not a certified security boundary.
struct Artifact {
    path: PathBuf,
    resolved: PathBuf,
    file: File,
    bytes: Vec<u8>,
    modified: SystemTime,
}
impl Artifact {
    fn read(path: &Path, root: &Path, external: bool) -> Result<Self, Refusal> {
        Self::read_shared(path, root, external, 1)
    }
    fn read_shared(path: &Path, root: &Path, external: bool, share: u32) -> Result<Self, Refusal> {
        let resolved = path
            .canonicalize()
            .map_err(|_private_error| "artifact_missing_or_unresolvable")?;
        if !external && !resolved.starts_with(root) {
            return Err("artifact_escape_refused");
        }
        let mut file = OpenOptions::new()
            .read(true)
            .share_mode(share)
            .open(&resolved)
            .map_err(|_private_error| "artifact_read_refused")?;
        let meta = file
            .metadata()
            .map_err(|_private_error| "artifact_metadata_refused")?;
        if !meta.is_file() || meta.len() > MAX_BYTES {
            return Err("artifact_missing_or_oversized");
        }
        let modified = meta
            .modified()
            .map_err(|_private_error| "artifact_time_unavailable")?;
        let bytes = read_bytes(&mut file)?;
        if bytes.len() as u64 != meta.len() {
            return Err("artifact_changed_during_read");
        }
        Ok(Self {
            path: path.to_path_buf(),
            resolved,
            file,
            bytes,
            modified,
        })
    }
    fn verify(&mut self) -> Result<(), Refusal> {
        if self
            .path
            .canonicalize()
            .map_err(|_private_error| "artifact_changed_during_read")?
            != self.resolved
        {
            return Err("artifact_changed_during_read");
        }
        let meta = self
            .file
            .metadata()
            .map_err(|_private_error| "artifact_changed_during_read")?;
        if meta.modified().ok() != Some(self.modified) || meta.len() != self.bytes.len() as u64 {
            return Err("artifact_changed_during_read");
        }
        self.file
            .rewind()
            .map_err(|_private_error| "artifact_changed_during_read")?;
        if read_bytes(&mut self.file)? != self.bytes {
            return Err("artifact_changed_during_read");
        }
        let mut current = OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&self.resolved)
            .map_err(|_private_error| "artifact_changed_during_read")?;
        if read_bytes(&mut current)? != self.bytes {
            return Err("artifact_changed_during_read");
        }
        Ok(())
    }
}
fn read_bytes(file: &mut File) -> Result<Vec<u8>, Refusal> {
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_private_error| "artifact_read_refused")?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("artifact_missing_or_oversized");
    }
    Ok(bytes)
}

fn options(args: &[OsString]) -> Result<Options, Refusal> {
    let Some(root) = args
        .first()
        .filter(|s| !s.to_string_lossy().starts_with("--"))
    else {
        return Err("invalid_preflight_arguments");
    };
    let mut opts = Options {
        root: root.into(),
        e2k: None,
        allow_external: false,
        expected_sha256: None,
    };
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--allow-external-e2k" && !opts.allow_external {
            opts.allow_external = true;
            i += 1;
        } else if args[i] == "--e2k" && opts.e2k.is_none() {
            opts.e2k = Some(args.get(i + 1).ok_or("invalid_preflight_arguments")?.into());
            i += 2;
        } else if args[i] == "--e2k-sha256" && opts.expected_sha256.is_none() {
            let hash = args
                .get(i + 1)
                .and_then(|s| s.to_str())
                .ok_or("invalid_preflight_arguments")?;
            if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("invalid_e2k_fingerprint");
            }
            opts.expected_sha256 = Some(hash.to_ascii_lowercase());
            i += 2;
        } else {
            return Err("invalid_preflight_arguments");
        }
    }
    if (opts.allow_external || opts.expected_sha256.is_some()) && opts.e2k.is_none() {
        return Err("explicit_e2k_required");
    }
    Ok(opts)
}
pub(crate) fn run_cli(args: &[OsString]) -> Result<(), String> {
    let report = match options(args) {
        Ok(opts) => inspect(&opts),
        Err(code) => Report {
            refusal: Some(code),
            ..Default::default()
        },
    };
    // No filenames, private errors, names, counts, hashes or user values here.
    let mut out = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut out, &report)
        .map_err(|_private_error| "Cannot write categorical preflight".to_owned())?;
    writeln!(out)
        .and_then(|()| out.flush())
        .map_err(|_private_error| "Cannot write categorical preflight".to_owned())?;
    if report.can_show_uncertified_preview() {
        Ok(())
    } else {
        Err("Preflight withheld linked geometry/results; see categorical report. No files modified.".into())
    }
}
pub(crate) fn inspect(opts: &Options) -> Report {
    let mut report = Report::default();
    if let Err(code) = inspect_inner(opts, &mut report) {
        report.refusal = Some(code);
    }
    report
}
fn inspect_inner(opts: &Options, report: &mut Report) -> Result<(), Refusal> {
    let root = opts
        .root
        .canonicalize()
        .map_err(|_private_error| "version_directory_missing")?;
    if !root.is_dir() {
        return Err("version_directory_missing");
    }
    if fs::symlink_metadata(root.join(".partial")).is_ok() {
        return Err("partial_version_refused");
    }
    let mut inputs = Vec::new();
    for name in [
        "manifest.json",
        "results/calc_output.json",
        "results/source-units.json",
    ] {
        inputs.push(Artifact::read(&root.join(name), &root, false)?);
    }
    let snapshot = Snapshot::load(&root)
        .map_err(|_private_error| "cached_identity_schema_or_units_refused")?;
    report.result_dto_ready = true;
    report.manifest_analysis = if snapshot.manifest.is_analyzed {
        "marked_analyzed"
    } else {
        "not_marked_analyzed"
    };
    report.engineering_check_status = snapshot.calc.summary.overall_status.as_str();
    report.supported_plots_ready = [Direction::X, Direction::Y].into_iter().all(|d| {
        snapshot
            .initial_config(d)
            .and_then(|c| snapshot.packet(&c, 1, None))
            .is_ok()
    });
    let source: SourceUnitMetadata = serde_json::from_slice(&inputs[2].bytes)
        .map_err(|_private_error| "source_units_refused")?;
    let metadata_stories = inspect_metadata(&root, &snapshot, &source, &mut inputs, report)?;
    let Some(e2k) = select_export(opts, &root, snapshot.manifest.e2k_size_bytes, report)? else {
        return verify_all(&mut inputs, &root, report);
    };
    check_geometry(&e2k, &snapshot, &source, &metadata_stories, report)?;
    inputs.push(e2k);
    verify_all(&mut inputs, &root, report)
}
fn select_export(
    opts: &Options,
    root: &Path,
    declared_e2k_size: Option<u64>,
    report: &mut Report,
) -> Result<Option<Artifact>, Refusal> {
    let mut e2ks = Vec::new();
    let mut edbs = Vec::new();
    // Only the explicitly selected version's immediate entries; no recursion,
    // project discovery, ambient folder scan or filesystem search.
    for (i, entry) in fs::read_dir(root)
        .map_err(|_private_error| "version_entries_refused")?
        .enumerate()
    {
        if i >= 512 {
            return Err("version_entry_limit");
        }
        let entry = entry.map_err(|_private_error| "version_entries_refused")?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("e2k"))
        {
            e2ks.push(path);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("edb"))
        {
            edbs.push(path);
        }
    }
    report.edb_presence = match edbs.len() {
        0 => "absent",
        1 => "present_not_read",
        _ => "ambiguous_not_read",
    };
    let chosen = if let Some(path) = &opts.e2k {
        if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        }
    } else {
        match e2ks.as_slice() {
            [] => {
                report.e2k_state = "absent_edb_is_not_e2k";
                return Ok(None);
            }
            [path] => path.clone(),
            _ => {
                report.e2k_state = "ambiguous_explicit_choice_required";
                return Ok(None);
            }
        }
    };
    if !chosen
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("e2k"))
    {
        return Err("e2k_extension_required_edb_is_not_e2k");
    }
    let resolved = chosen
        .canonicalize()
        .map_err(|_private_error| "e2k_missing")?;
    let external = !resolved.starts_with(root);
    if external && (!opts.allow_external || opts.expected_sha256.is_none()) {
        return Err("external_e2k_requires_opt_in_and_fingerprint");
    }
    report.e2k_scope = if external {
        "external_unverified"
    } else {
        "selected_version_directory"
    };
    if external {
        report.same_snapshot_linkage = "external_unverified";
    }
    let e2k = Artifact::read(&chosen, root, external)?;
    if opts
        .expected_sha256
        .as_ref()
        .is_some_and(|expected| format!("{:x}", Sha256::digest(&e2k.bytes)) != *expected)
    {
        return Err("e2k_fingerprint_mismatch");
    }
    if !external && declared_e2k_size.is_some_and(|size| size != e2k.bytes.len() as u64) {
        report.same_snapshot_linkage = "conflict";
        return Err("manifest_e2k_size_mismatch");
    }
    Ok(Some(e2k))
}
fn check_geometry(
    e2k: &Artifact,
    snapshot: &Snapshot,
    source: &SourceUnitMetadata,
    metadata_stories: &[String],
    report: &mut Report,
) -> Result<(), Refusal> {
    let text = std::str::from_utf8(&e2k.bytes).map_err(|_private_error| "e2k_utf8_required")?;
    let declared = strict_units(text)?;
    let (model, _sources) =
        ext_e2k::parse_str_with_sources(text).map_err(|_private_error| "e2k_parse_refused")?;
    let major = model
        .program
        .version
        .as_ref()
        .and_then(|v| v.as_str().split('.').next())
        .and_then(|v| v.parse::<u32>().ok());
    if !major.is_some_and(|v| (17..=23).contains(&v)) {
        return Err("e2k_version_unsupported");
    }
    report.unit_coherence = if declared
        == (
            ForceUnit::from_e2k(&source.force).ok_or("source_force_unsupported")?,
            LengthUnit::from_e2k(&source.length).ok_or("source_length_unsupported")?,
            TempUnit::from_e2k(&source.temperature).ok_or("source_temperature_unsupported")?,
        ) {
        "compatible"
    } else {
        "conflict"
    };
    if report.unit_coherence != "compatible" {
        report.same_snapshot_linkage = "conflict";
        return Err("e2k_source_units_mismatch");
    }
    let scene = ext_e2k::project_scene(&model);
    if model.stories.is_empty()
        || model.stories.len() > 10000
        || (scene.frames.is_empty() && scene.shells.is_empty())
    {
        return Err("e2k_no_projected_geometry");
    }
    if scene.frames.iter().any(|f| {
        [f.i, f.j]
            .iter()
            .any(|p| ![p.x, p.y, p.z].iter().all(|v| v.is_finite()))
    }) || scene
        .shells
        .iter()
        .flat_map(|s| &s.points)
        .any(|p| ![p.x, p.y, p.z].iter().all(|v| v.is_finite()))
    {
        return Err("e2k_nonfinite_geometry");
    }
    report.geometry_ready = true;
    report.e2k_state = if scene.skipped.is_empty() {
        "parsed_connectivity_limited"
    } else {
        "parsed_connectivity_with_skips"
    };
    check_story_orders(&model, snapshot, metadata_stories, report)
}
fn check_story_orders(
    model: &ext_e2k::BuildingModel,
    snapshot: &Snapshot,
    metadata_stories: &[String],
    report: &mut Report,
) -> Result<(), Refusal> {
    let order: Vec<&str> = model.stories.iter().map(|s| s.name.as_str()).collect();
    if order.iter().copied().collect::<BTreeSet<_>>().len() != order.len() {
        return Err("duplicate_e2k_story");
    }
    let source_stories = order.iter().copied().collect::<BTreeSet<_>>();
    let observed_stories = metadata_stories
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    report.metadata_story_coherence = if observed_stories.is_empty() {
        "unavailable"
    } else if observed_stories == source_stories {
        "matching_names_observational"
    } else if observed_stories.is_subset(&source_stories) {
        "explicit_metadata_subset_observational"
    } else {
        "observational_mismatch_needs_reconciliation"
    };
    let mut result_orders = Vec::new();
    if let Some(drift) = &snapshot.calc.drift_wind {
        result_orders.push(&drift.x.story_order);
        result_orders.push(&drift.y.story_order);
    }
    if let Some(forces) = &snapshot.calc.story_forces {
        result_orders.push(&forces.story_order);
    }
    if result_orders.is_empty() {
        report.story_coherence = "result_story_order_unavailable";
        return Ok(());
    }
    let positions: BTreeMap<&str, usize> = order.iter().enumerate().map(|(i, s)| (*s, i)).collect();
    let mut subset = false;
    for result in result_orders {
        let indexes = result
            .iter()
            .map(|story| positions.get(story.as_str()).copied())
            .collect::<Option<Vec<_>>>()
            .ok_or("result_story_absent_from_e2k")?;
        if indexes.is_empty() || indexes.windows(2).any(|w| w[0] >= w[1]) {
            report.same_snapshot_linkage = "conflict";
            return Err("result_story_order_or_duplicates_mismatch");
        }
        subset |= indexes.len() != order.len();
    }
    report.story_coherence = if subset {
        "compatible_explicit_subset"
    } else {
        "matching_explicit_order"
    };
    Ok(())
}
fn inspect_metadata(
    root: &Path,
    snapshot: &Snapshot,
    source: &SourceUnitMetadata,
    inputs: &mut Vec<Artifact>,
    report: &mut Report,
) -> Result<Vec<String>, Refusal> {
    let path = root.join("model-metadata.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let metadata = Artifact::read(&path, root, false)?;
    // Authoritative DTO only. No SidecarClient, process, transport or COM use.
    let observed: ModelMetadata = serde_json::from_slice(&metadata.bytes)
        .map_err(|_private_error| "metadata_schema_refused")?;
    // Pinned commands.rs tests document legacy v0 and current v2 metadata.
    if !matches!(observed.schema_version, 0 | 2) {
        return Err("metadata_schema_unsupported");
    }
    // Preserve omission separately from ModelMetadata's legacy false default.
    #[derive(serde::Deserialize)]
    struct RecordedFlag {
        #[serde(rename = "isAnalyzed")]
        analyzed: Option<bool>,
    }
    let flag: RecordedFlag = serde_json::from_slice(&metadata.bytes)
        .map_err(|_private_error| "metadata_schema_refused")?;
    report.metadata_analysis = if flag.analyzed.is_none() {
        "flag_not_recorded"
    } else if observed.is_analyzed {
        "observed_analyzed"
    } else {
        "observed_not_analyzed"
    };
    report.analysis_state_relation = if flag.analyzed.is_none() {
        "unknown_observation"
    } else if !observed.is_analyzed && !snapshot.manifest.is_analyzed {
        "cached_results_but_flags_not_analyzed"
    } else if observed.is_analyzed == snapshot.manifest.is_analyzed {
        "flags_agree_freshness_unverified"
    } else {
        "flags_disagree_reconcile_provenance"
    };
    report.metadata_chronology = match chrono::DateTime::parse_from_rfc3339(&observed.collected_at)
    {
        Ok(time) if time < snapshot.calc.meta.generated_at => "metadata_predates_results",
        Ok(_) => "metadata_not_earlier_than_results",
        Err(_) => "collection_time_unverified",
    };
    report.metadata_units = match observed.units.as_deref() {
        Some(preset)
            if SourceUnitMetadata::from_actual(
                preset,
                &source.force,
                &source.length,
                &source.temperature,
            )
            .is_ok() =>
        {
            "compatible"
        }
        Some(_) => "observational_label_conflict",
        None => "missing",
    };
    if observed
        .stories
        .iter()
        .any(|s| !s.elevation.is_finite() || s.height.is_some_and(|v| !v.is_finite()))
    {
        return Err("metadata_nonfinite_story");
    }
    if observed
        .stories
        .iter()
        .map(|s| &s.name)
        .collect::<BTreeSet<_>>()
        .len()
        != observed.stories.len()
    {
        return Err("metadata_duplicate_story");
    }
    let names = observed
        .stories
        .into_iter()
        .map(|story| story.name)
        .collect();
    inputs.push(metadata);
    Ok(names)
}
fn verify_all(inputs: &mut [Artifact], root: &Path, report: &mut Report) -> Result<(), Refusal> {
    for artifact in inputs {
        artifact.verify()?;
    }
    if fs::symlink_metadata(root.join(".partial")).is_ok() {
        return Err("version_became_partial");
    }
    report.load_integrity = "verified_at_load";
    Ok(())
}
fn strict_units(text: &str) -> Result<(ForceUnit, LengthUnit, TempUnit), Refusal> {
    // The pinned parser silently retains defaults for unknown UNITS tokens.
    // This is only a rejecting header guard using its official unit parsers.
    let mut controls = false;
    let mut declared = None;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.starts_with('$') {
            controls = line.eq_ignore_ascii_case("$ CONTROLS");
        } else if controls
            && line
                .split_whitespace()
                .next()
                .is_some_and(|w| w.eq_ignore_ascii_case("UNITS"))
        {
            let values: Vec<_> = line.split('"').skip(1).step_by(2).collect();
            let unit = (
                values
                    .first()
                    .and_then(|s| ForceUnit::from_e2k(s))
                    .ok_or("e2k_force_not_declared_or_unknown")?,
                values
                    .get(1)
                    .and_then(|s| LengthUnit::from_e2k(s))
                    .ok_or("e2k_length_not_declared_or_unknown")?,
                values
                    .get(2)
                    .and_then(|s| TempUnit::from_e2k(s))
                    .ok_or("e2k_temperature_not_declared_or_unknown")?,
            );
            if declared.is_some_and(|prior| prior != unit) {
                return Err("e2k_conflicting_units");
            }
            declared = Some(unit);
        }
    }
    declared.ok_or("e2k_units_missing")
}

#[cfg(test)]
#[path = "preflight_tests.rs"]
mod tests;
