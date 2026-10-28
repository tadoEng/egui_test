use super::*;
use crate::data::Snapshot;
use serde_json::{Value, json};
use std::fs;

fn load(edit: impl FnOnce(&mut Value)) -> Result<Snapshot, String> {
    let (root, mut calc) = crate::fixture::fixture().expect("owned authored fixture");
    edit(&mut calc);
    fs::write(
        root.path().join("results/calc_output.json"),
        calc.to_string(),
    )
    .expect("authored calc JSON");
    Snapshot::load(root.path())
}

#[test]
fn only_stored_responses_are_offered() {
    let snapshot = load(|_| {}).expect("public fixture");
    assert_eq!(
        snapshot.responses(),
        [
            Response::WindDrift,
            Response::StoryShear,
            Response::StoryMoment
        ],
        "no seismic drift or displacement is stored, so none is offered"
    );
    assert!(
        snapshot
            .response(Response::SeismicDrift, Direction::X, Some("G"), "W")
            .is_err()
    );
    let choices = snapshot
        .response_choices(Response::WindDrift, Direction::X)
        .expect("drift");
    assert_eq!(
        (choices.groups, choices.cases),
        (vec!["G".into()], vec!["W".into()])
    );
    let choices = snapshot
        .response_choices(Response::StoryShear, Direction::Y)
        .expect("profiles");
    assert_eq!(choices.cases, ["FX"]);
    assert!(choices.groups.is_empty());
}

#[test]
fn drift_keeps_stored_order_signs_and_reference() {
    let snapshot = load(|_| {}).expect("public fixture");
    let plot = snapshot
        .response(Response::WindDrift, Direction::X, Some("G"), "W")
        .expect("drift");
    assert_eq!(plot.stories, ["Roof", "L1"]);
    assert_eq!(plot.series[0].values, [Some(0.002), Some(0.001)]);
    assert_eq!(plot.series[1].values, [Some(-0.0025), Some(-0.0015)]);
    assert_eq!(plot.reference, Some(0.01));
    assert_eq!(plot.unit, "ratio");
    assert!(
        snapshot
            .response(Response::WindDrift, Direction::X, None, "W")
            .is_err(),
        "a group is required, never guessed"
    );
    assert!(
        snapshot
            .response(Response::WindDrift, Direction::X, Some("G"), "missing")
            .is_err()
    );
}

#[test]
fn profiles_read_the_named_stored_field() {
    let snapshot = load(|_| {}).expect("public fixture");
    let shear = snapshot
        .response(Response::StoryShear, Direction::X, None, "FX")
        .expect("shear");
    assert_eq!(shear.series[0].values, [Some(7.0), Some(-2.0)]);
    assert_eq!(shear.series[0].name, "Vx (stored)");
    assert_eq!(shear.unit, "kip");
    let moment = snapshot
        .response(Response::StoryMoment, Direction::Y, None, "FX")
        .expect("moment");
    assert_eq!(moment.series[0].name, "My (stored)");
    assert_eq!(moment.unit, "kip·ft");
}

#[test]
fn a_missing_story_is_a_gap_and_an_unknown_one_is_refused() {
    let snapshot = load(|calc| {
        let rows = calc["driftWind"]["x"]["rows"].as_array_mut().expect("rows");
        rows.retain(|row| row["story"] != "L1");
    })
    .expect("fixture without the L1 drift row");
    let plot = snapshot
        .response(Response::WindDrift, Direction::X, Some("G"), "W")
        .expect("drift");
    assert_eq!(
        plot.series[0].values,
        [Some(0.002), None],
        "a gap, not a zero"
    );

    let snapshot = load(|calc| {
        calc["storyForces"]["xProfiles"][0]["rows"][0]["story"] = json!("Mezzanine");
    })
    .expect("loads; the profile is refused when read");
    assert!(
        snapshot
            .response(Response::StoryShear, Direction::X, None, "FX")
            .is_err()
    );
}

/// Opt-in, local only: `LAB_RESULTS_VERSION=<version folder> cargo test -p
/// lab-results -- --ignored --nocapture`. Prints only which responses load,
/// never values or names, so it is safe to run on private folders.
#[test]
#[ignore = "reads a local folder named by LAB_RESULTS_VERSION"]
fn a_named_local_version_folder_loads_through_the_same_checks() {
    let Ok(folder) = std::env::var("LAB_RESULTS_VERSION") else {
        return;
    };
    let snapshot = Snapshot::load(std::path::Path::new(&folder)).expect("cached results load");
    for response in Response::ALL {
        for direction in [Direction::X, Direction::Y] {
            let status = snapshot.response_choices(response, direction).map_or_else(
                |_| "absent".to_owned(),
                |choices| {
                    let ok = choices.cases.iter().all(|case| {
                        let groups: Vec<Option<&str>> = if response.grouped() {
                            choices.groups.iter().map(|g| Some(g.as_str())).collect()
                        } else {
                            vec![None]
                        };
                        groups.iter().any(|group| {
                            snapshot.response(response, direction, *group, case).is_ok()
                        })
                    });
                    if ok {
                        "every case plots"
                    } else {
                        "some case refused"
                    }
                    .to_owned()
                },
            );
            println!("{response:?} {direction:?}: {status}");
        }
    }
}
