use super::*;
use serde_json::{Value, json};
fn fixture() -> (tempfile::TempDir, Value) {
    crate::fixture::fixture().expect("owned authored fixture")
}
fn save(root: &Path, value: &Value) {
    fs::write(root.join("results/calc_output.json"), value.to_string())
        .expect("synthetic calc JSON");
}
#[test]
fn cached_projection_preserves_order_signs_reference_units_and_files() {
    let (root, _) = fixture();
    let before = read_bounded(&root.path().join("results/calc_output.json")).expect("before");
    let snapshot = Snapshot::load(root.path()).expect("typed source");
    let config = snapshot.initial_config(Direction::X).expect("config");
    let packet = snapshot.packet(&config, 1, None).expect("packet");
    assert_eq!(packet.drift.stories, ["Roof", "L1"]);
    assert_eq!(packet.drift.series[0].values, [Some(0.002), Some(0.001)]);
    assert_eq!(
        packet.drift.series[1].values,
        [Some(-0.0025), Some(-0.0015)]
    );
    assert_eq!(packet.shear.series[0].values, [Some(7.0), Some(-2.0)]);
    assert_eq!(packet.drift.reference, Some(0.01));
    assert_eq!(packet.shear.unit, "kip");
    assert_eq!(
        before,
        read_bounded(&root.path().join("results/calc_output.json")).expect("after")
    );
    assert!(!snapshot.manifest.is_analyzed);
}
#[test]
fn partial_version_identity_and_conflicting_units_are_refused() {
    let (root, mut calc) = fixture();
    fs::write(root.path().join(".partial"), b"").expect("own sentinel");
    assert!(Snapshot::load(root.path()).is_err());
    fs::remove_file(root.path().join(".partial")).expect("remove own sentinel");
    calc["meta"]["versionId"] = json!("v2");
    save(root.path(), &calc);
    assert!(Snapshot::load(root.path()).is_err());
    calc["meta"]["versionId"] = json!("v1");
    calc["meta"]["units"]["force"] = json!("kN");
    save(root.path(), &calc);
    assert!(Snapshot::load(root.path()).is_err());
}
#[test]
fn missing_rows_stay_null_and_duplicate_stories_fail() {
    let (root, mut calc) = fixture();
    calc["driftWind"]["x"]["rows"]
        .as_array_mut()
        .expect("rows")
        .remove(0);
    save(root.path(), &calc);
    let snapshot = Snapshot::load(root.path()).expect("source");
    let config = snapshot.initial_config(Direction::X).expect("config");
    let packet = snapshot.packet(&config, 1, None).expect("packet");
    assert_eq!(packet.drift.series[0].values, [Some(0.002), None]);
    let duplicate = calc["driftWind"]["x"]["rows"][0].clone();
    calc["driftWind"]["x"]["rows"]
        .as_array_mut()
        .expect("rows")
        .push(duplicate);
    save(root.path(), &calc);
    let snapshot = Snapshot::load(root.path()).expect("source");
    assert!(snapshot.packet(&config, 1, None).is_err());
}
#[test]
fn non_finite_values_and_unknown_cases_are_refused() {
    let (root, _) = fixture();
    let mut snapshot = Snapshot::load(root.path()).expect("source");
    let config = snapshot.initial_config(Direction::X).expect("config");
    snapshot
        .calc
        .drift_wind
        .as_mut()
        .expect("drift")
        .x
        .allowable_ratio = f64::NAN;
    assert!(snapshot.packet(&config, 1, None).is_err());
    let mut bad = config;
    bad.shear_case = "UNKNOWN".into();
    assert!(snapshot.packet(&bad, 1, None).is_err());
}
#[test]
fn selection_rejects_old_source_generation_unknown_story_and_extra_fields() {
    let (root, _) = fixture();
    let snapshot = Snapshot::load(root.path()).expect("source");
    let config = snapshot.initial_config(Direction::X).expect("config");
    let packet = snapshot.packet(&config, 7, None).expect("packet");
    let message = json!({"version":1,"type":"select_story","source":packet.source,"generation":7,"plot":"drift","story":"Roof"});
    assert_eq!(
        packet.accept(&message.to_string()).expect("current"),
        Some("Roof".into())
    );
    for (key, value) in [
        ("source", json!("old")),
        ("generation", json!(6)),
        ("story", json!("UNKNOWN")),
        ("extra", json!(true)),
    ] {
        let mut bad = message.clone();
        bad[key] = value;
        assert!(packet.accept(&bad.to_string()).is_err());
    }
}
