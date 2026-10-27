use super::*;
use serde_json::{Value, json};

// Independently authored two-story connectivity, never copied from a client.
const E2K: &str = include_str!("../../assets/fixtures/real01.e2k");
fn options_for(root: &Path) -> Options {
    Options {
        root: root.to_owned(),
        e2k: None,
        allow_external: false,
        expected_sha256: None,
    }
}
fn write_json(root: &Path, name: &str, data: &Value) {
    fs::write(root.join(name), data.to_string()).expect("authored JSON");
}
fn fixture(with_e2k: bool) -> (tempfile::TempDir, Value) {
    let (root, calc) = crate::fixture::fixture().expect("owned authored cache");
    fs::write(root.path().join("model.edb"), b"").expect("empty owned EDB marker, not parsed");
    write_json(
        root.path(),
        "model-metadata.json",
        &json!({"schemaVersion":2,"isAnalyzed":false,"units":"kip-ft-F","collectedAt":"2025-12-31T00:00:00Z","stories":[{"name":"Roof","elevation":10.0},{"name":"L1","elevation":0.0}]}),
    );
    if with_e2k {
        fs::write(root.path().join("authored.e2k"), E2K).expect("authored E2K");
    }
    (root, calc)
}
#[test]
fn valid_public_geometry_and_signed_cached_results_remain_uncertified() {
    let (root, _) = fixture(true);
    let before = fs::read(root.path().join("results/calc_output.json")).expect("before");
    let report = inspect(&options_for(root.path()));
    assert!(
        report.can_show_uncertified_preview(),
        "{:?}",
        report.refusal
    );
    assert_eq!(report.unit_coherence, "compatible");
    assert_eq!(report.story_coherence, "matching_explicit_order");
    assert_eq!(report.same_snapshot_linkage, "unverified");
    assert!(!report.source_current_certified);
    assert_eq!(
        before,
        fs::read(root.path().join("results/calc_output.json")).expect("after")
    );
}
#[test]
fn edb_only_keeps_result_dto_ready_and_blocks_geometry() {
    let (root, _) = fixture(false);
    let report = inspect(&options_for(root.path()));
    assert!(report.result_dto_ready);
    assert!(!report.geometry_ready);
    assert_eq!(report.e2k_state, "absent_edb_is_not_e2k");
    assert_eq!(report.edb_presence, "present_not_read");
    assert_eq!(report.load_integrity, "verified_at_load");
}
#[test]
fn multiple_exports_require_explicit_selected_version_choice() {
    let (root, _) = fixture(true);
    fs::write(root.path().join("second.e2k"), E2K).expect("second authored file");
    let mut opts = options_for(root.path());
    assert_eq!(
        inspect(&opts).e2k_state,
        "ambiguous_explicit_choice_required"
    );
    opts.e2k = Some("second.e2k".into());
    assert!(inspect(&opts).can_show_uncertified_preview());
}
#[test]
fn partial_missing_manifest_and_version_mismatch_refuse_pairing() {
    let (root, mut calc) = fixture(true);
    fs::write(root.path().join(".partial"), b"").expect("owned sentinel");
    assert_eq!(
        inspect(&options_for(root.path())).refusal,
        Some("partial_version_refused")
    );
    fs::remove_file(root.path().join(".partial")).expect("remove own sentinel");
    calc["meta"]["versionId"] = json!("v2");
    write_json(root.path(), "results/calc_output.json", &calc);
    assert_eq!(
        inspect(&options_for(root.path())).refusal,
        Some("cached_identity_schema_or_units_refused")
    );
    fs::remove_file(root.path().join("manifest.json")).expect("remove own manifest");
    assert!(!inspect(&options_for(root.path())).result_dto_ready);
}
#[test]
fn unknown_conflicting_and_mismatched_units_are_refused() {
    for source in [
        E2K.replace("\"ft\"", "\"unknown\""),
        E2K.replace("\"Kip\"", "\"bad\""),
        E2K.replace("\"F\"", "\"bad\""),
        E2K.replace("\"ft\"", "\"m\""),
        E2K.replace("$ STORIES", " UNITS \"Kip\" \"in\" \"F\"\n$ STORIES"),
    ] {
        let (root, _) = fixture(true);
        fs::write(root.path().join("authored.e2k"), source).expect("authored invalid units");
        let report = inspect(&options_for(root.path()));
        assert!(report.refusal.is_some());
        assert!(!report.can_show_uncertified_preview());
    }
}
#[test]
fn malformed_and_unresolved_connectivity_cannot_be_ready() {
    for source in [
        "not an E2K document".to_owned(),
        E2K.replace("23.2.0", "9.7.0"),
        E2K.replace("POINT \"1\"", "POINT \"missing\"")
            .replace("POINT \"2\"", "POINT \"also_missing\""),
    ] {
        let (root, _) = fixture(true);
        fs::write(root.path().join("authored.e2k"), source).expect("authored malformed source");
        assert!(!inspect(&options_for(root.path())).can_show_uncertified_preview());
    }
}
#[test]
fn metadata_time_flags_and_engineering_failure_are_independent() {
    let (root, mut calc) = fixture(true);
    calc["summary"]["overallStatus"] = json!("fail");
    calc["summary"]["checkCount"] = json!(1);
    calc["summary"]["failCount"] = json!(1);
    write_json(root.path(), "results/calc_output.json", &calc);
    let report = inspect(&options_for(root.path()));
    assert!(report.can_show_uncertified_preview());
    assert_eq!(report.metadata_chronology, "metadata_predates_results");
    assert_eq!(report.manifest_analysis, "not_marked_analyzed");
    assert_eq!(report.metadata_analysis, "observed_not_analyzed");
    assert_eq!(report.engineering_check_status, "fail");
    assert!(!report.source_current_certified);
}
#[test]
fn conflicting_analysis_flags_do_not_discard_valid_cached_output() {
    let (root, _) = fixture(true);
    let path = root.path().join("model-metadata.json");
    let mut metadata: Value =
        serde_json::from_slice(&fs::read(&path).expect("fixture")).expect("JSON");
    metadata["isAnalyzed"] = json!(true);
    write_json(root.path(), "model-metadata.json", &metadata);
    let report = inspect(&options_for(root.path()));
    assert!(report.can_show_uncertified_preview());
    assert!(report.result_dto_ready);
    assert_eq!(
        report.analysis_state_relation,
        "flags_disagree_reconcile_provenance"
    );
}
#[test]
fn missing_unknown_duplicate_and_wrongly_ordered_result_stories_refuse_linking() {
    for order in [
        json!(["Roof", "UNKNOWN"]),
        json!(["Roof", "Roof"]),
        json!(["L1", "Roof"]),
        json!([]),
    ] {
        let (root, mut calc) = fixture(true);
        calc["driftWind"]["x"]["storyOrder"] = order;
        write_json(root.path(), "results/calc_output.json", &calc);
        assert!(!inspect(&options_for(root.path())).can_show_uncertified_preview());
    }
}
#[test]
fn valid_missing_rows_keep_gaps_and_explicit_subset_order() {
    let (root, mut calc) = fixture(true);
    calc["driftWind"]["x"]["rows"]
        .as_array_mut()
        .expect("rows")
        .remove(0);
    write_json(root.path(), "results/calc_output.json", &calc);
    assert!(inspect(&options_for(root.path())).can_show_uncertified_preview());
    for direction in ["x", "y"] {
        calc["driftWind"][direction]["storyOrder"] = json!(["Roof"]);
        calc["driftWind"][direction]["rows"]
            .as_array_mut()
            .expect("rows")
            .retain(|row| row["story"] == "Roof");
    }
    write_json(root.path(), "results/calc_output.json", &calc);
    assert_eq!(
        inspect(&options_for(root.path())).story_coherence,
        "compatible_explicit_subset"
    );
}
#[test]
fn external_export_requires_opt_in_and_hash_but_stays_unlinked() {
    let (root, _) = fixture(false);
    let external = tempfile::tempdir().expect("owned sibling");
    let path = external.path().join("outside.e2k");
    fs::write(&path, E2K).expect("public source");
    let mut opts = options_for(root.path());
    opts.e2k = Some(path);
    assert_eq!(
        inspect(&opts).refusal,
        Some("external_e2k_requires_opt_in_and_fingerprint")
    );
    opts.allow_external = true;
    opts.expected_sha256 = Some("0".repeat(64));
    assert_eq!(inspect(&opts).refusal, Some("e2k_fingerprint_mismatch"));
    opts.expected_sha256 = Some(format!("{:x}", Sha256::digest(E2K.as_bytes())));
    let report = inspect(&opts);
    assert!(report.geometry_ready);
    assert!(!report.can_show_uncertified_preview());
    assert_eq!(report.same_snapshot_linkage, "external_unverified");
}
#[test]
fn manifest_size_mismatch_catches_replaced_or_stale_export() {
    let (root, _) = fixture(true);
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(root.path().join("manifest.json")).expect("fixture"))
            .expect("JSON");
    manifest["e2kSizeBytes"] = json!(1);
    write_json(root.path(), "manifest.json", &manifest);
    assert_eq!(
        inspect(&options_for(root.path())).refusal,
        Some("manifest_e2k_size_mismatch")
    );
}
#[test]
fn cooperative_windows_writers_are_excluded_and_detectable_source_changes_fail() {
    let (root, _) = fixture(true);
    let path = root.path().join("authored.e2k");
    let canonical = root.path().canonicalize().expect("canonical owned root");
    let mut held = Artifact::read(&path, &canonical, false).expect("read guard");
    assert!(fs::write(&path, b"changed").is_err());
    assert!(held.verify().is_ok());
    drop(held);
    // Simulate a source which is not protected by cooperative sharing. Same
    // verification routine must catch changed bytes, not merely mtime.
    let mut observed =
        Artifact::read_shared(&path, &canonical, false, 7).expect("test shared read");
    fs::write(&path, E2K.replace("23.2.0", "23.3.0")).expect("owned source replacement");
    assert_eq!(observed.verify(), Err("artifact_changed_during_read"));
}
#[test]
fn bounded_file_reader_and_edb_as_text_are_refused() {
    let (root, _) = fixture(false);
    let path = root.path().join("too_large.e2k");
    File::create(&path)
        .expect("owned file")
        .set_len(MAX_BYTES + 1)
        .expect("owned length");
    let canonical = root.path().canonicalize().expect("canonical owned root");
    assert_eq!(
        Artifact::read(&path, &canonical, false).err(),
        Some("artifact_missing_or_oversized")
    );
    let mut opts = options_for(root.path());
    opts.e2k = Some("model.edb".into());
    assert_eq!(
        inspect(&opts).refusal,
        Some("e2k_extension_required_edb_is_not_e2k")
    );
}
#[test]
fn categorical_report_contains_no_paths_hashes_names_counts_or_private_errors() {
    let (root, _) = fixture(true);
    let report = inspect(&options_for(root.path()));
    let value = serde_json::to_string(&report).expect("categories");
    assert!(!value.contains(root.path().to_str().expect("owned path")));
    assert!(!value.contains("Roof"));
    assert!(!value.contains("SHA256"));
    assert!(!value.contains("collectedAt"));
    for key in [
        "frames",
        "shells",
        "triangles",
        "fingerprint",
        "filePath",
        "warning_text",
    ] {
        assert!(
            serde_json::to_value(&report)
                .expect("report")
                .get(key)
                .is_none()
        );
    }
}
#[test]
fn cli_options_reject_ambient_scan_and_incomplete_external_authorization() {
    assert!(options(&[]).is_err());
    assert!(options(&["--scan".into()]).is_err());
    assert!(options(&["fixture".into(), "--allow-external-e2k".into()]).is_err());
    assert!(
        options(&[
            "fixture".into(),
            "--e2k".into(),
            "x.e2k".into(),
            "--e2k-sha256".into(),
            "bad".into()
        ])
        .is_err()
    );
}
#[test]
fn empty_cached_sections_are_valid_dtos_but_not_fabricated_supported_plots() {
    let (root, mut calc) = fixture(true);
    calc["driftWind"] = Value::Null;
    calc["storyForces"] = Value::Null;
    write_json(root.path(), "results/calc_output.json", &calc);
    let report = inspect(&options_for(root.path()));
    assert!(report.result_dto_ready);
    assert!(!report.supported_plots_ready);
    assert!(!report.can_show_uncertified_preview());
    assert_eq!(report.story_coherence, "result_story_order_unavailable");
}
#[test]
fn omitted_observation_is_unknown_and_duplicate_metadata_keys_are_refused() {
    let (root, _) = fixture(true);
    let path = root.path().join("model-metadata.json");
    let mut value: Value = serde_json::from_slice(&fs::read(path).expect("fixture")).expect("JSON");
    value.as_object_mut().expect("object").remove("isAnalyzed");
    write_json(root.path(), "model-metadata.json", &value);
    let report = inspect(&options_for(root.path()));
    assert_eq!(report.metadata_analysis, "flag_not_recorded");
    assert_eq!(report.analysis_state_relation, "unknown_observation");
    let duplicate = value["stories"][0].clone();
    value["stories"]
        .as_array_mut()
        .expect("stories")
        .push(duplicate);
    write_json(root.path(), "model-metadata.json", &value);
    assert_eq!(
        inspect(&options_for(root.path())).refusal,
        Some("metadata_duplicate_story")
    );
}
#[test]
fn source_documented_metadata_versions_are_supported_and_future_versions_refuse_linkage() {
    for version in [0, 2, 3] {
        let (root, _) = fixture(true);
        let path = root.path().join("model-metadata.json");
        let mut metadata: Value =
            serde_json::from_slice(&fs::read(path).expect("authored metadata")).expect("JSON");
        metadata["schemaVersion"] = json!(version);
        write_json(root.path(), "model-metadata.json", &metadata);
        let report = inspect(&options_for(root.path()));
        assert!(report.result_dto_ready);
        if version == 3 {
            assert_eq!(report.refusal, Some("metadata_schema_unsupported"));
        } else {
            assert!(report.can_show_uncertified_preview());
        }
    }
}
