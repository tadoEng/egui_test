use super::*;
use lab_fixture::Preset;

#[test]
fn truth_manifest_has_exact_totals_and_source_keyed_contiguous_ranges() {
    let fixture = Fixture::generate(Preset::TruthTable).expect("fixture");
    let mesh = build(&fixture, Config::default()).expect("mesh");
    assert_eq!(
        (
            mesh.entities.len(),
            mesh.vertices.len(),
            mesh.indices.len() / 3
        ),
        (24, 437, 219)
    );
    assert_eq!(mesh.buffer_bytes(), 437 * 36 + 219 * 3 * 4);
    assert_eq!(mesh.revision, fixture.revision());
    assert_eq!(mesh.revision.formula_version, 2);
    let mut end = 0;
    for (range, entity) in mesh.entities.iter().zip(fixture.entities()) {
        assert_eq!(range.id, entity.id());
        assert_eq!(range.indices.start, end);
        assert!(range.indices.end > end);
        assert_eq!(
            range.indices.len() / 3,
            entity.geometry().reference_mesh_counts().1
        );
        end = range.indices.end;
    }
    assert_eq!(end as usize, mesh.indices.len());
    assert_eq!(size_of::<Vertex>(), 36);
}

#[test]
fn every_scale_matches_all_four_kinds_manifest_bounds_and_planned_batch_limits() {
    for (preset, total) in [
        (Preset::Thousand, 1035),
        (Preset::TenThousand, 10281),
        (Preset::FiftyThousand, 50715),
        (Preset::HundredThousand, 99981),
    ] {
        let fixture = Fixture::generate(preset).expect("fixture");
        let config = Config::default();
        let mesh = build(&fixture, config).expect("mesh");
        assert_eq!(mesh.entities.len(), total);
        assert_eq!(
            mesh.vertices.len(),
            fixture.counts().reference_mesh_vertices
        );
        assert_eq!(mesh.indices.len() / 3, fixture.counts().triangles);
        assert_eq!(mesh.by_kind.len(), 4);
        assert_eq!(
            mesh.by_kind.values().map(|c| c.entities).sum::<usize>(),
            total
        );
        assert_eq!(
            mesh.by_kind.values().map(|c| c.vertices).sum::<usize>(),
            mesh.vertices.len()
        );
        assert_eq!(
            mesh.by_kind.values().map(|c| c.triangles).sum::<usize>(),
            mesh.indices.len() / 3
        );
        for kind in Kind::ALL {
            assert_eq!(
                mesh.by_kind[&kind].entities,
                fixture.counts().by_kind[&kind]
            );
        }
        assert!(
            mesh.indices
                .iter()
                .all(|i| (*i as usize) < mesh.vertices.len())
        );
        let mut end = 0;
        for batch in &mesh.batches {
            assert_eq!(batch.start, end);
            assert_eq!(batch.start % 3, 0);
            assert_eq!(batch.end % 3, 0);
            assert!(!batch.is_empty() && batch.len() <= config.max_batch_indices);
            end = batch.end;
        }
        assert_eq!(end as usize, mesh.indices.len());
        assert!(
            mesh.batches.len() < 20,
            "planned batches must not scale one per entity"
        );
        let bounds = fixture.bounds();
        for i in 0..3 {
            assert!((mesh.local_bounds.min[i] + mesh.origin_m[i] - bounds.min[i]).abs() < 1.0e-4);
            assert!((mesh.local_bounds.max[i] + mesh.origin_m[i] - bounds.max[i]).abs() < 1.0e-4);
        }
    }
}

#[test]
fn every_truth_triangle_has_unit_normal_consistent_winding_and_outward_prism_orientation() {
    let fixture = Fixture::generate(Preset::TruthTable).expect("fixture");
    let mesh = build(&fixture, Config::default()).expect("mesh");
    for (range, entity) in mesh.entities.iter().zip(fixture.entities()) {
        let points = entity.geometry().control_points();
        let center: Point =
            std::array::from_fn(|i| points.iter().map(|p| p[i]).sum::<f64>() / points.len() as f64);
        for triangle in mesh.indices[range.indices.start as usize..range.indices.end as usize]
            .as_chunks::<3>()
            .0
        {
            let v = [triangle[0], triangle[1], triangle[2]].map(|i| mesh.vertices[i as usize]);
            let p = v.map(|v| v.position.map(f64::from));
            let n = v[0].normal.map(f64::from);
            assert!((dot(n, n) - 1.0).abs() < 1.0e-6);
            assert!(dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), n) > 0.0);
            assert!(v.iter().all(|vertex| vertex.normal == v[0].normal));
            if !matches!(entity.geometry(), Geometry::Shell { .. }) {
                let world: Point = std::array::from_fn(|i| p[0][i] + mesh.origin_m[i]);
                assert!(
                    dot(sub(world, center), n) > 0.0,
                    "outward prism faces for {:?}",
                    entity.id()
                );
            }
        }
    }
}

#[test]
fn partition_settings_reuse_geometry_identity_while_fixture_revision_changes() {
    let fixture = Fixture::generate(Preset::TruthTable).expect("fixture");
    let one = build(&fixture, Config::default()).expect("mesh");
    let many = build(
        &fixture,
        Config {
            max_batch_indices: 6,
            ..Config::default()
        },
    )
    .expect("mesh");
    assert_eq!(one.geometry_sha256, many.geometry_sha256);
    assert_eq!(one.vertices, many.vertices);
    assert_eq!(one.indices, many.indices);
    assert_ne!(one.batches, many.batches);
    assert_eq!(
        one.geometry_sha256,
        build(&fixture, Config::default())
            .expect("repeat")
            .geometry_sha256
    );
    let other = build(
        &Fixture::generate(Preset::Thousand).expect("fixture"),
        Config::default(),
    )
    .expect("mesh");
    assert_ne!(one.revision, other.revision);
    assert_ne!(one.geometry_sha256, other.geometry_sha256);
}

#[test]
fn size_and_misaligned_batch_settings_are_refused_before_conversion() {
    let fixture = Fixture::generate(Preset::TruthTable).expect("fixture");
    assert!(matches!(
        build(
            &fixture,
            Config {
                max_buffer_bytes: 1,
                ..Config::default()
            }
        ),
        Err(Error::BufferBound)
    ));
    for limit in [0, 1, 2, 4, 7] {
        assert!(matches!(
            build(
                &fixture,
                Config {
                    max_batch_indices: limit,
                    ..Config::default()
                }
            ),
            Err(Error::InvalidBatchLimit)
        ));
    }
    let exact = 437 * 36 + 219 * 3 * 4;
    assert!(
        build(
            &fixture,
            Config {
                max_buffer_bytes: exact,
                ..Config::default()
            }
        )
        .is_ok()
    );
    assert!(matches!(
        build(
            &fixture,
            Config {
                max_buffer_bytes: exact - 1,
                ..Config::default()
            }
        ),
        Err(Error::BufferBound)
    ));
}

#[test]
fn defensive_surface_conversion_refuses_void_nonplanar_degenerate_and_nonfinite_shapes() {
    let square = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let valid = Geometry::Shell {
        boundary: square.clone(),
        normal: [0.0, 0.0, 1.0],
        openings: vec![],
    };
    let convert = |g: &Geometry| append(g, [1.0; 3], [0.0; 3], &mut Vec::new(), &mut Vec::new());
    assert!(convert(&valid).is_ok());
    let hole = Geometry::Shell {
        boundary: square.clone(),
        normal: [0.0, 0.0, 1.0],
        openings: vec![square.clone()],
    };
    assert_eq!(convert(&hole), Err(Error::UnsupportedOpening));
    for (boundary, normal) in [
        (vec![], [0.0, 0.0, 1.0]),
        (vec![[0.0; 3]; 3], [0.0, 0.0, 1.0]),
        (
            vec![[0.0; 3], [1.0, 0.0, 0.0], [1.0, 1.0, 0.1]],
            [0.0, 0.0, 1.0],
        ),
        (
            vec![[f64::NAN, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]],
            [0.0, 0.0, 1.0],
        ),
        (square, [0.0, 0.0, 2.0]),
    ] {
        assert_eq!(
            convert(&Geometry::Shell {
                boundary,
                normal,
                openings: vec![]
            }),
            Err(Error::InvalidGeometry)
        );
    }
}

#[test]
fn display_rounding_collapse_is_an_error_instead_of_zero_area_triangles() {
    let points = [
        [1.0e6, 0.0, 0.0],
        [1.0e6 + 0.001, 0.0, 0.0],
        [1.0e6 + 0.001, 1.0, 0.0],
    ];
    assert_eq!(
        surface(
            &points,
            [0.0, 0.0, 1.0],
            [1.0; 3],
            [0.0; 3],
            &mut Vec::new(),
            &mut Vec::new()
        ),
        Err(Error::InvalidGeometry)
    );
}
