use super::*;
use crate::geometry::{cross, dot, subtract};

fn truth() -> Fixture {
    Fixture::generate(Preset::TruthTable).expect("authored truth fixture is valid")
}
fn id(kind: Kind, story: u16, slot: u32) -> LabId {
    LabId {
        kind,
        story: StoryId(story),
        slot,
    }
}

#[test]
fn generations_match_entire_data_and_indexes_at_every_scale() {
    for preset in Preset::ALL {
        let a = Fixture::generate(preset).expect("first generation");
        let b = Fixture::generate(preset).expect("second generation");
        assert_eq!(
            a, b,
            "all keys, coordinates, normals, revisions, bounds, counts and membership must agree for {preset:?}"
        );
    }
}

#[test]
fn grid_counts_follow_independent_closed_formula() {
    for (preset, nx, ny, total) in [
        (Preset::Thousand, 1, 1, 1035),
        (Preset::TenThousand, 5, 6, 10281),
        (Preset::FiftyThousand, 13, 13, 50715),
        (Preset::HundredThousand, 18, 19, 99981),
    ] {
        let f = Fixture::generate(preset).expect("grid generation");
        let columns = (nx + 1) * (ny + 1) * 69;
        let beams = (nx * (ny + 1) + ny * (nx + 1)) * 69;
        let slabs = nx * ny * 69;
        let walls = 6 * 69;
        assert_eq!(f.entities().len(), total, "structural element count");
        assert_eq!(
            f.counts().by_kind,
            BTreeMap::from([
                (Kind::Beam, beams),
                (Kind::Column, columns),
                (Kind::Wall, walls),
                (Kind::Slab, slabs)
            ]),
            "kind counts"
        );
        assert!(
            f.counts().by_story.values().all(|&n| n == total / 69),
            "equal floor counts"
        );
        assert_eq!(
            f.counts().control_points,
            8 * (columns + beams) + 4 * (walls + slabs),
            "geometry control points"
        );
        assert_eq!(
            f.counts().reference_mesh_vertices,
            24 * (columns + beams) + 4 * (walls + slabs),
            "flat prism face reference vertices"
        );
        assert_eq!(
            f.counts().triangles,
            12 * (columns + beams) + 2 * (walls + slabs),
            "reference triangles, not entities"
        );
        assert_eq!(
            f.bounds(),
            Bounds {
                min: [-0.2, -0.2, 0.0],
                max: [nx as f64 * 4.0 + 0.2, ny as f64 * 4.0 + 0.2, 207.15]
            },
            "metre bounds"
        );
    }
}

#[test]
fn truth_counts_story_attribution_and_group_members_are_explicit() {
    let f = truth();
    assert_eq!(
        f.counts().by_kind,
        BTreeMap::from([
            (Kind::Beam, 8),
            (Kind::Column, 8),
            (Kind::Wall, 4),
            (Kind::Slab, 3)
        ]),
        "truth kinds"
    );
    assert_eq!(
        f.counts().by_story,
        BTreeMap::from([(StoryId(1), 11), (StoryId(2), 12)]),
        "sloped slab belongs to authored upper floor"
    );
    assert_eq!(
        (
            f.counts().control_points,
            f.counts().reference_mesh_vertices,
            f.counts().triangles
        ),
        (157, 413, 207),
        "truth reference topology"
    );
    assert_eq!(
        f.group_members()[&Group::Core],
        vec![id(Kind::Wall, 1, 0), id(Kind::Wall, 2, 0)],
        "two core walls"
    );
    assert_eq!(
        f.group_members()[&Group::Empty],
        vec![],
        "explicit zero-member group"
    );
    let expected_perimeter: Vec<_> = [Kind::Beam, Kind::Column]
        .into_iter()
        .flat_map(|kind| {
            (1..=2).flat_map(move |story| (0..4).map(move |slot| id(kind, story, slot)))
        })
        .collect();
    assert_eq!(
        f.group_members()[&Group::Perimeter],
        expected_perimeter,
        "perimeter exact set, not just count"
    );
    assert!(
        f.group_members()[&Group::LateralSystem].contains(&id(Kind::Wall, 1, 1)),
        "separate shear wall"
    );
    assert!(
        f.group_members()[&Group::ZoneA].contains(&id(Kind::Wall, 2, 0))
            && f.group_members()[&Group::ZoneB].contains(&id(Kind::Wall, 2, 0)),
        "zones overlap across floors"
    );
    assert_eq!(
        f.bounds(),
        Bounds {
            min: [-0.2, -0.2, 0.0],
            max: [12.0, 4.2, 7.0]
        },
        "irregular sloped slab extends fixture bounds"
    );
}

#[test]
fn exact_group_counts_match_authored_manifest_at_all_scales() {
    for (preset, core, perimeter, zone_a, zone_b, lateral) in [
        (Preset::TruthTable, 2, 16, 16, 17, 20),
        (Preset::Thousand, 276, 552, 1035, 1035, 966),
        (Preset::TenThousand, 276, 3036, 6555, 6555, 3450),
        (Preset::FiftyThousand, 276, 7176, 28221, 28221, 7590),
        (Preset::HundredThousand, 276, 10212, 54096, 54096, 10626),
    ] {
        let f = Fixture::generate(preset).expect("valid preset");
        assert_eq!(
            f.counts().by_group,
            BTreeMap::from([
                (Group::Core, core),
                (Group::Perimeter, perimeter),
                (Group::ZoneA, zone_a),
                (Group::ZoneB, zone_b),
                (Group::LateralSystem, lateral),
                (Group::Empty, 0)
            ]),
            "authored group counts for {preset:?}"
        );
        for members in f.group_members().values() {
            assert!(
                members.windows(2).all(|w| w[0] < w[1]),
                "sorted, unique group IDs"
            );
        }
    }
}

#[test]
fn prism_face_triangles_have_correct_outward_normals_for_every_frame() {
    let f = truth();
    for e in f.entities() {
        if let Geometry::Prism(bounds) = e.geometry() {
            let points = e.geometry().control_points();
            let center: Point = std::array::from_fn(|i| bounds.min[i].midpoint(bounds.max[i]));
            for face in PRISM_FACE_RINGS {
                for [a, b, c] in [[face[0], face[1], face[2]], [face[0], face[2], face[3]]] {
                    let n = cross(
                        subtract(points[b], points[a]),
                        subtract(points[c], points[a]),
                    );
                    assert!(
                        dot(n, subtract(points[a], center)) > 1.0e-9,
                        "face must point outward for {}",
                        e.id()
                    );
                }
            }
        }
    }
}

#[test]
fn sloped_polygon_is_planar_with_positive_fan_area_and_authored_kind() {
    let f = truth();
    let e = f
        .entities()
        .iter()
        .find(|e| e.id() == id(Kind::Slab, 2, 1))
        .expect("sloped slab");
    let Geometry::Shell {
        boundary, normal, ..
    } = e.geometry()
    else {
        panic!("slab must be a shell");
    };
    let expected = [-0.2, -0.1, 1.0];
    let length = dot(expected, expected).sqrt();
    for i in 0..3 {
        assert!(
            (normal[i] - expected[i] / length).abs() < 1.0e-12,
            "authored slope normal"
        );
    }
    assert_eq!(boundary.len(), 5, "irregular convex pentagon");
    assert_eq!(
        e.geometry().reference_mesh_counts(),
        (5, 3),
        "pentagon fan topology"
    );
}

#[test]
fn known_near_far_positions_lie_on_distinct_depth_ordered_column_surfaces() {
    let f = truth();
    let oracle = f.occlusion_oracle().expect("truth ray oracle");
    for (key, hit) in [
        (oracle.near, oracle.near_position),
        (oracle.far, oracle.far_position),
    ] {
        let e = f
            .entities()
            .iter()
            .find(|e| e.id() == key)
            .expect("referenced column exists");
        let Geometry::Prism(bounds) = e.geometry() else {
            panic!("oracle needs column prism");
        };
        assert_eq!(hit[1], bounds.min[1], "front surface on +Y ray");
        assert!(
            (0..3).all(|i| bounds.min[i] <= hit[i] && hit[i] <= bounds.max[i]),
            "hit inside face extents"
        );
    }
    assert!(
        oracle.near_position[1] < oracle.far_position[1],
        "far column obscured by near column"
    );
    assert_eq!(oracle.ray_origin, [0.0, -10.0, 1.5], "authored ray origin");
    assert_eq!(oracle.ray_direction, [0.0, 1.0, 0.0], "authored unit ray");
}

fn reject(mutator: impl FnOnce(&mut Vec<Entity>, &mut Vec<Story>), expected: FixtureError) {
    let f = truth();
    let mut entities = f.entities;
    let mut stories = f.stories;
    mutator(&mut entities, &mut stories);
    assert_eq!(
        generate::validate(Preset::TruthTable, entities, stories, None)
            .expect_err("malformed data must fail"),
        expected,
        "typed refusal"
    );
}

#[test]
fn malformed_identity_story_and_group_data_are_refused() {
    let first = id(Kind::Beam, 1, 0);
    reject(
        |e, _| e.push(e[0].clone()),
        FixtureError::DuplicateId(first),
    );
    reject(
        |e, _| e[0].id.story = StoryId(0),
        FixtureError::InvalidStory,
    );
    reject(|_, s| s[1].id = StoryId(1), FixtureError::InvalidStory);
    reject(
        |_, s| s[0].elevation_m = f64::NAN,
        FixtureError::InvalidStory,
    );
    reject(
        |e, _| e[0].groups.push(Group::Empty),
        FixtureError::InvalidGroup(first),
    );
    reject(
        |e, _| {
            let group = e[0].groups[0];
            e[0].groups.push(group);
        },
        FixtureError::InvalidGroup(first),
    );
}

#[test]
fn malformed_frame_coordinates_and_dimensions_are_refused() {
    let first = id(Kind::Beam, 1, 0);
    reject(
        |e, _| {
            if let Geometry::Prism(b) = &mut e[0].geometry {
                b.min[0] = f64::NAN;
            }
        },
        FixtureError::InvalidCoordinate(first),
    );
    reject(
        |e, _| {
            if let Geometry::Prism(b) = &mut e[0].geometry {
                b.max[2] = b.min[2];
            }
        },
        FixtureError::DegenerateGeometry(first),
    );
    reject(
        |e, _| {
            if let Geometry::Prism(b) = &mut e[0].geometry {
                b.max[0] = f64::INFINITY;
            }
        },
        FixtureError::InvalidCoordinate(first),
    );
}

#[test]
fn malformed_shells_and_unsupported_openings_are_refused() {
    let key = id(Kind::Wall, 1, 0);
    let mutate_shell = |e: &mut Vec<Entity>, action: &dyn Fn(&mut Geometry)| {
        let e = e.iter_mut().find(|e| e.id == key).expect("wall");
        action(&mut e.geometry);
    };
    reject(
        |e, _| {
            mutate_shell(e, &|g| {
                if let Geometry::Shell { normal, .. } = g {
                    *normal = normal.map(|v| -v);
                }
            });
        },
        FixtureError::IncorrectNormal(key),
    );
    reject(
        |e, _| {
            mutate_shell(e, &|g| {
                if let Geometry::Shell { boundary, .. } = g {
                    boundary[1] = boundary[0];
                }
            });
        },
        FixtureError::DegenerateGeometry(key),
    );
    reject(
        |e, _| {
            mutate_shell(e, &|g| {
                if let Geometry::Shell { boundary, .. } = g {
                    boundary[3][1] += 0.2;
                }
            });
        },
        FixtureError::NonPlanarOrConcave(key),
    );
    reject(
        |e, _| {
            mutate_shell(e, &|g| {
                if let Geometry::Shell { openings, .. } = g {
                    openings.push(vec![[2.5, 2.0, 1.0], [3.5, 2.0, 1.0], [3.5, 2.0, 2.0]]);
                }
            });
        },
        FixtureError::UnsupportedOpening(key),
    );
    reject(
        |e, _| {
            mutate_shell(e, &|g| {
                *g = Geometry::Prism(Bounds {
                    min: [0.0; 3],
                    max: [1.0; 3],
                });
            });
        },
        FixtureError::KindGeometryMismatch(key),
    );
}

#[test]
fn concave_and_self_intersecting_shells_cannot_produce_a_valid_fixture() {
    let key = id(Kind::Slab, 1, 0);
    for boundary in [
        vec![
            [0.0, 0.0, 3.0],
            [4.0, 0.0, 3.0],
            [1.0, 1.0, 3.0],
            [4.0, 4.0, 3.0],
            [0.0, 4.0, 3.0],
        ],
        vec![
            [0.0, 0.0, 3.0],
            [4.0, 4.0, 3.0],
            [0.0, 4.0, 3.0],
            [4.0, 0.0, 3.0],
        ],
    ] {
        let f = truth();
        let mut entities = f.entities;
        let e = entities.iter_mut().find(|e| e.id == key).expect("slab");
        e.geometry = Geometry::Shell {
            boundary,
            normal: [0.0, 0.0, 1.0],
            openings: vec![],
        };
        assert!(
            generate::validate(Preset::TruthTable, entities, f.stories, None).is_err(),
            "no silent triangulation of concavity or bow tie"
        );
    }
}

#[test]
fn empty_fixture_and_nonunit_normals_are_refused() {
    assert_eq!(
        generate::validate(Preset::TruthTable, vec![], vec![], None).expect_err("empty fixture"),
        FixtureError::Empty,
        "empty cannot have bounds"
    );
    let f = truth();
    let mut entities = f.entities;
    let key = id(Kind::Slab, 1, 0);
    let e = entities.iter_mut().find(|e| e.id == key).expect("slab");
    if let Geometry::Shell { normal, .. } = &mut e.geometry {
        *normal = [0.0, 0.0, 2.0];
    }
    assert_eq!(
        generate::validate(Preset::TruthTable, entities, f.stories, None)
            .expect_err("nonunit normal"),
        FixtureError::IncorrectNormal(key),
        "normal cannot conceal a bad coordinate transform"
    );
}
