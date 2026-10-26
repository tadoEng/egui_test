//! Read-only adapter from ext-e2k's projected connectivity to lab geometry.

use crate::scene::{Member, MemberKind, SceneModel};
use ext_e2k::{BuildingModel, FrameKind, LengthUnit, ScenePoint};
use std::{collections::HashMap, path::Path};

pub(crate) fn load(path: &Path) -> Result<SceneModel, String> {
    let started = web_time::Instant::now();
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("Cannot read E2K: {error}"))?;
    validate_length_units(&source)?;
    let parsed =
        ext_e2k::parse_str(&source).map_err(|error| format!("E2K import failed: {error}"))?;
    let mut model = convert(&parsed)?;
    model.build_ms = started.elapsed().as_secs_f64() * 1000.0;
    Ok(model)
}

// ext-e2k's lenient reader retains a default on unknown unit tokens. Validate
// this small header contract before accepting coordinates as metres.
fn validate_length_units(source: &str) -> Result<(), String> {
    let mut in_controls = false;
    let mut declared = None;
    for line in source.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.starts_with('$') {
            in_controls = line.eq_ignore_ascii_case("$ CONTROLS");
        } else if in_controls && line.split_whitespace().next() == Some("UNITS") {
            let units = line
                .split('"')
                .skip(1)
                .step_by(2)
                .nth(1)
                .and_then(LengthUnit::from_e2k)
                .ok_or("E2K has no recognized declared length unit; refusing to guess scale.")?;
            if declared.is_some_and(|previous| previous != units) {
                return Err("E2K has conflicting length unit declarations.".into());
            }
            declared = Some(units);
        }
    }
    declared
        .map(|_| ())
        .ok_or_else(|| "E2K has no declared length units.".into())
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Validated finite coordinates are reduced to f32 only for the wireframe renderer"
)]
fn point(point: ScenePoint, factor: f64) -> Option<[f32; 3]> {
    let point = [point.x * factor, point.y * factor, point.z * factor];
    point
        .iter()
        .all(|v| v.is_finite() && v.abs() <= 1.0e12)
        .then(|| point.map(|v| v as f32))
}

fn convert(parsed: &BuildingModel) -> Result<SceneModel, String> {
    let major = parsed
        .program
        .version
        .as_ref()
        .and_then(|v| v.as_str().split('.').next())
        .and_then(|v| v.parse::<u32>().ok());
    if !major.is_some_and(|v| (17..=23).contains(&v)) {
        return Err("This parser adapter supports declared ETABS versions 17–23 only.".into());
    }
    if !parsed.controls.units_declared {
        return Err("E2K has no declared units; refusing to guess geometry scale.".into());
    }
    let factor = match parsed.controls.length_unit {
        LengthUnit::Ft => 0.3048,
        LengthUnit::In => 0.0254,
        LengthUnit::M => 1.0,
        LengthUnit::Cm => 0.01,
        LengthUnit::Mm => 0.001,
    };
    let stories: Vec<String> = parsed
        .stories
        .iter()
        .rev()
        .map(|s| s.name.as_str().to_owned())
        .collect();
    let indices: HashMap<&str, usize> = stories
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i + 1))
        .collect();
    let scene = ext_e2k::project_scene(parsed);
    let mut members = Vec::with_capacity(scene.frames.len() + scene.shells.len());
    let mut rejected = 0;
    for frame in &scene.frames {
        if let (Some(&story), Some(start), Some(end)) = (
            indices.get(frame.story.as_str()),
            point(frame.i, factor),
            point(frame.j, factor),
        ) {
            let kind = match frame.kind {
                FrameKind::Column => MemberKind::Column,
                FrameKind::Beam => MemberKind::Beam,
                _ => MemberKind::OtherFrame,
            };
            members.push(Member {
                key: frame.key.as_stable_str(),
                story,
                kind,
                start,
                end,
                ring: Vec::new(),
            });
        } else {
            rejected += 1;
        }
    }
    for shell in &scene.shells {
        let points: Option<Vec<_>> = shell.points.iter().map(|&p| point(p, factor)).collect();
        if let (Some(&story), Some(ring)) = (indices.get(shell.story.as_str()), points) {
            if ring.len() >= 3 {
                members.push(Member {
                    key: shell.key.as_stable_str(),
                    story,
                    kind: MemberKind::Shell,
                    start: ring[0],
                    end: ring[1],
                    ring,
                });
            } else {
                rejected += 1;
            }
        } else {
            rejected += 1;
        }
    }
    if members.is_empty() || stories.is_empty() {
        return Err("No renderable frame/shell instances were found.".into());
    }
    let mut model = SceneModel::from_members(members, stories, "Local E2K");
    model.joints = scene.joints.len();
    model.skipped = scene.skipped.len() + rejected;
    model.warnings = parsed.warnings.len();
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    const MODEL: &str = "$ PROGRAM INFORMATION\n PROGRAM \"ETABS\" VERSION \"23.2.0\"\n$ CONTROLS\n UNITS \"Kip\" \"ft\" \"F\"\n$ STORIES - IN SEQUENCE FROM TOP\n STORY \"Top\" HEIGHT 10\n STORY \"Base\" ELEV 0\n$ POINT COORDINATES\n POINT \"1\" 0 0\n POINT \"2\" 10 0\n POINT \"3\" 10 10\n POINT \"4\" 0 10\n$ LINE CONNECTIVITIES\n LINE \"C1\" COLUMN \"1\" \"1\" 1\n$ AREA CONNECTIVITIES\n AREA \"F1\" FLOOR 4 \"1\" \"2\" \"3\" \"4\" 0 0 0 0\n$ LINE ASSIGNS\n LINEASSIGN \"C1\" \"Top\"\n$ AREA ASSIGNS\n AREAASSIGN \"F1\" \"Top\"\n$ END OF MODEL FILE\n";

    #[test]
    fn imports_frame_and_shell_instances_with_units_and_distinct_keys() {
        let parsed = ext_e2k::parse_str(MODEL).expect("synthetic E2K parses");
        let model = convert(&parsed).expect("synthetic E2K converts");
        assert_eq!((model.frames, model.shells), (1, 1));
        assert!(((model.members[0].end[2] - model.members[0].start[2]).abs() - 3.048).abs() < 1e-5);
        assert!((model.members[1].end[0] - 3.048).abs() < 1e-5);
        assert_ne!(model.members[0].key, model.members[1].key);
        assert_eq!(model.members[1].segments().count(), 4);
        assert_eq!(model.stories, ["Base", "Top"]);
    }

    #[test]
    fn refuses_unknown_units_and_unsupported_legacy_versions() {
        assert!(validate_length_units(MODEL).is_ok());
        assert!(validate_length_units(&MODEL.replace("\"ft\"", "\"unknown\"")).is_err());
        let mut parsed = ext_e2k::parse_str(MODEL).expect("synthetic E2K parses");
        parsed.controls.units_declared = false;
        assert!(convert(&parsed).is_err());
        let legacy =
            ext_e2k::parse_str(&MODEL.replace("23.2.0", "9.7.2")).expect("legacy syntax parses");
        assert!(convert(&legacy).is_err());
    }

    #[test]
    fn converts_all_supported_length_units_to_metres() {
        for (unit, expected_height) in [
            ("ft", 3.048_f32),
            ("in", 0.254),
            ("m", 10.0),
            ("cm", 0.1),
            ("mm", 0.01),
        ] {
            let source = MODEL.replace("\"ft\"", &format!("\"{unit}\""));
            validate_length_units(&source).expect("known unit");
            let parsed = ext_e2k::parse_str(&source).expect("synthetic E2K parses");
            let model = convert(&parsed).expect("supported unit converts");
            let member = &model.members[0];
            assert!(((member.end[2] - member.start[2]).abs() - expected_height).abs() < 1e-5);
        }
    }
}
