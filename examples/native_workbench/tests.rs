use super::*;
use std::fs;
fn session() -> (tempfile::TempDir, Session) {
    let input = public_fixture().expect("owned public fixture");
    let session = Session::load(input.path()).expect("checked public version");
    (input, session)
}
fn select_event(session: &Session, generation: u64, story: &str) -> String {
    serde_json::json!({"version":1,"type":"select_story","source":session.packet.source,"generation":generation,"plot":"drift","story":story}).to_string()
}
#[test]
fn same_checked_bytes_preserve_geometry_and_stored_signed_values() {
    let (input, session) = session();
    let ordinary = Snapshot::load(input.path()).expect("legacy reader");
    assert_eq!(ordinary.fingerprint, session.snapshot.fingerprint);
    assert_eq!(session.packet.drift.stories, ["Roof", "L1"]);
    assert_eq!(
        session.packet.drift.series[0].values,
        [Some(0.002), Some(0.001)]
    );
    assert_eq!(
        session.packet.drift.series[1].values,
        [Some(-0.0025), Some(-0.0015)]
    );
    assert_eq!(
        session.packet.shear.series[0].values,
        [Some(7.0), Some(-2.0)]
    );
    let mesh = session.mesh.as_ref().expect("unmoved mesh");
    assert_eq!(mesh.entities().len(), 3);
    assert_eq!(mesh.vertices().len(), 52);
    assert_eq!(mesh.indices().len() / 3, 26);
    assert!(
        !session.report.source_current_certified,
        "DTO/geometry agreement cannot certify freshness"
    );
    assert_eq!(session.report.manifest_analysis, "not_marked_analyzed");
}
#[test]
fn equal_version_names_and_bytes_in_two_projects_cannot_cross_select() {
    let (_first, a) = session();
    let (_second, mut b) = session();
    assert_eq!(a.snapshot.fingerprint, b.snapshot.fingerprint);
    assert_ne!(
        a.packet.source, b.packet.source,
        "project directory namespace must contribute"
    );
    assert!(b.accept(&select_event(&a, 1, "Roof")).is_err());
    assert!(b.packet.selected_story.is_none());
}
#[test]
fn geometry_bytes_change_selection_identity_even_if_buffers_are_identical() {
    let (input, a) = session();
    let path = input.path().join("authored.e2k");
    let mut bytes = fs::read(&path).expect("public bytes");
    bytes.extend_from_slice(b"\n$ authored comment-only revision\n");
    fs::write(path, bytes).expect("owned fixture edit");
    let mut b = Session::load(input.path()).expect("new explicit load");
    assert_ne!(a.packet.source, b.packet.source);
    assert_eq!(
        a.mesh.as_ref().expect("mesh").revision().geometry_sha256,
        b.mesh.as_ref().expect("mesh").revision().geometry_sha256
    );
    assert!(b.accept(&select_event(&a, 1, "Roof")).is_err());
}
#[test]
fn chart_context_a_b_a_expires_old_generation_and_selection_is_idempotent() {
    let (_input, mut s) = session();
    let old = select_event(&s, 1, "Roof");
    s.accept(&old).expect("first selection");
    s.accept(&old).expect("idempotent selection");
    assert_eq!(s.packet.generation, 1);
    s.configure(s.snapshot.initial_config(Direction::Y).expect("Y"))
        .expect("configure Y");
    assert_eq!(s.packet.generation, 2);
    assert!(s.packet.selected_story.is_none());
    s.configure(s.snapshot.initial_config(Direction::X).expect("X"))
        .expect("configure X");
    assert_eq!(s.packet.generation, 3);
    assert!(s.accept(&old).is_err());
    let fresh = select_event(&s, 3, "L1");
    s.accept(&fresh).expect("fresh selection");
    assert_eq!(s.packet.selected_story.as_deref(), Some("L1"));
}
#[test]
fn exact_geometry_and_both_plot_membership_are_required() {
    let (_input, mut s) = session();
    assert!(
        s.select(Some("roof".into())).is_err(),
        "no case-folded join"
    );
    s.packet.shear.stories.retain(|name| name != "L1");
    assert!(
        s.select(Some("L1".into())).is_err(),
        "one plot alone is insufficient"
    );
    s.packet.drift.stories.push("Ghost".into());
    s.packet.shear.stories.push("Ghost".into());
    assert!(
        s.select(Some("Ghost".into())).is_err(),
        "charts cannot invent geometry story membership"
    );
}
#[test]
fn failed_recheck_retains_last_geometry_but_irrevocably_unlinks_session() {
    let (input, mut s) = session();
    let old = select_event(&s, 1, "Roof");
    s.accept(&old).expect("select");
    let pointer = s.mesh.as_ref().expect("mesh").vertices().as_ptr();
    let path = input.path().join("results/calc_output.json");
    let original = fs::read(&path).expect("public bytes");
    fs::write(&path, b"{}").expect("owned invalidation");
    assert!(s.recheck().is_err());
    assert!(!s.linked);
    assert_eq!(
        s.mesh
            .as_ref()
            .expect("last valid mesh")
            .vertices()
            .as_ptr(),
        pointer
    );
    assert_eq!(s.packet.shear.series[0].values, [Some(7.0), Some(-2.0)]);
    assert!(s.packet.selected_story.is_none());
    assert_eq!(s.packet.generation, 2);
    fs::write(&path, original).expect("restore owned bytes");
    assert!(s.recheck().is_err(), "A->B->A requires explicit reopening");
    assert!(s.accept(&old).is_err());
    assert!(s.select(Some("Roof".into())).is_err());
}
#[test]
fn preflight_guard_keeps_all_artifacts_open_until_mesh_packet_preparation_finishes() {
    let input = public_fixture().expect("owned fixture");
    let (mut checked, _report) =
        preflight::load_checked_preview(&options(input.path())).expect("guarded bundle");
    for name in [
        "manifest.json",
        "results/calc_output.json",
        "results/source-units.json",
        "authored.e2k",
    ] {
        assert!(
            fs::OpenOptions::new()
                .write(true)
                .open(input.path().join(name))
                .is_err(),
            "cooperative Windows write must be refused while held"
        );
    }
    checked.verify_inputs().expect("same held bytes");
    fs::write(input.path().join(".partial"), b"owned test marker").expect("owned marker");
    assert!(
        checked.verify_inputs().is_err(),
        "partial version appearing during preparation must refuse publication"
    );
}
#[test]
fn missing_e2k_ambiguous_e2k_and_result_version_mismatch_fail_closed() {
    let input = public_fixture().expect("owned fixture");
    let path = input.path().join("authored.e2k");
    let e2k = fs::read(&path).expect("public E2K");
    fs::remove_file(&path).expect("owned input removal");
    assert!(Session::load(input.path()).is_err());
    fs::write(&path, &e2k).expect("restore owned E2K");
    fs::write(input.path().join("extra.e2k"), &e2k).expect("ambiguity fixture");
    assert!(Session::load(input.path()).is_err());
    fs::remove_file(input.path().join("extra.e2k")).expect("owned ambiguity removal");
    let path = input.path().join("manifest.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("manifest")).expect("public JSON");
    value["id"] = serde_json::json!("v2");
    fs::write(&path, value.to_string()).expect("owned mismatch");
    assert!(Session::load(input.path()).is_err());
}

#[test]
fn guarded_optional_artifact_and_export_set_cannot_change_during_preparation() {
    let input = public_fixture().expect("owned fixture");
    let (mut checked, _) =
        preflight::load_checked_preview(&options(input.path())).expect("guarded bundle");
    let extra = input.path().join("second.e2k");
    fs::write(&extra, b"owned ambiguity marker").expect("owned extra entry");
    assert!(checked.verify_inputs().is_err());
    fs::remove_file(extra).expect("remove owned extra entry");
    checked.verify_inputs().expect("same export set again");
    fs::write(input.path().join("model-metadata.json"), b"{}").expect("owned optional artifact");
    assert!(checked.verify_inputs().is_err());
}
