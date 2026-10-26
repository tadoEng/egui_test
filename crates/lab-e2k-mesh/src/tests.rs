use super::*;
const SOURCE: &str = r#"$ PROGRAM INFORMATION
 PROGRAM "ETABS" VERSION "23.2.0"
$ CONTROLS
 UNITS "Kip" "ft" "F"
$ STORIES - IN SEQUENCE FROM TOP
 STORY "Roof" HEIGHT 10
 STORY "Base" ELEV 0
$ POINT COORDINATES
 POINT "1" 0 0
 POINT "2" 10 0
 POINT "3" 10 10
 POINT "4" 0 10
$ LINE CONNECTIVITIES
 LINE "C" COLUMN "1" "1" 1
 LINE "B" BEAM "1" "2" 0
$ AREA CONNECTIVITIES
 AREA "S" FLOOR 4 "1" "2" "3" "4" 0 0 0 0
$ LINE ASSIGNS
 LINEASSIGN "C" "Roof"
 LINEASSIGN "B" "Roof"
$ AREA ASSIGNS
 AREAASSIGN "S" "Roof" OPENING "NO"
$ END OF MODEL FILE
"#;
#[test]
fn known_source_keys_story_order_metre_centerlines_and_batch_counts_are_preserved() {
    let mesh = build(SOURCE.as_bytes(), Config::default()).expect("authored source");
    assert_eq!(mesh.entities().len(), 3);
    assert_eq!(mesh.vertices().len(), 52);
    assert_eq!(mesh.indices().len(), 78);
    assert_eq!(mesh.buffer_bytes(), 52 * 36 + 78 * 4);
    assert_eq!(
        mesh.stories()
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["Roof", "Base"]
    );
    assert!((mesh.stories()[0].elevation_m - 3.048).abs() < 1e-12);
    let column = mesh
        .entities()
        .iter()
        .find(|e| e.class == SourceClass::Column)
        .expect("column");
    let ends = column.source_centerline_m.expect("source centerline");
    assert!(((ends[1][2] - ends[0][2]).abs() - 3.048).abs() < 1e-12);
    let scene = ext_e2k::project_scene(&ext_e2k::parse_str(SOURCE).expect("same authority"));
    assert_eq!(mesh.entities()[0].key, scene.frames[0].key);
    assert_eq!(mesh.entities()[2].key, scene.shells[0].key);
    assert_eq!(mesh.entities()[2].mode, DisplayMode::ZeroThicknessSurface);
    assert!(
        mesh.entities()
            .windows(2)
            .all(|w| w[0].indices.end <= w[1].indices.start)
    );
    assert!(
        mesh.indices()
            .iter()
            .all(|i| (*i as usize) < mesh.vertices().len())
    );
}
#[test]
fn every_triangle_winds_with_its_unit_normal_including_inclined_source_frame() {
    let source = SOURCE.replace("COLUMN \"1\" \"1\"", "COLUMN \"1\" \"2\"");
    let mesh = build(source.as_bytes(), Config::default()).expect("inclined source column");
    for triangle in mesh.indices().as_chunks::<3>().0 {
        let v: [Vertex; 3] = std::array::from_fn(|i| mesh.vertices()[triangle[i] as usize]);
        let points = v.map(|v| v.position.map(f64::from));
        let normal = v[0].normal.map(f64::from);
        assert!((dot(normal, normal) - 1.0).abs() < 1e-6);
        assert!(
            dot(
                cross(sub(points[1], points[0]), sub(points[2], points[0])),
                normal
            ) > 0.0
        );
    }
}
#[test]
fn unknown_units_missing_units_and_unsupported_versions_are_refused() {
    for source in [
        SOURCE.replace("\"ft\"", "\"unknown\""),
        SOURCE.replace("\"Kip\"", "\"unknown\""),
        SOURCE.replace("UNITS \"Kip\" \"ft\" \"F\"", ""),
        SOURCE.replace("23.2.0", "9.7.0"),
    ] {
        assert!(build(source.as_bytes(), Config::default()).is_err());
    }
}
#[test]
fn source_and_configuration_changes_invalidate_full_revision() {
    let first = build(SOURCE.as_bytes(), Config::default()).expect("fixture");
    let same = build(SOURCE.as_bytes(), Config::default()).expect("same");
    assert_eq!(first.revision(), same.revision());
    let changed = build(
        format!("{SOURCE}\n$ AUTHORED COMMENT\n").as_bytes(),
        Config::default(),
    )
    .expect("changed bytes");
    assert_ne!(first.revision(), changed.revision());
    assert_eq!(
        first.revision().geometry_sha256,
        changed.revision().geometry_sha256
    );
    let different = build(
        SOURCE.as_bytes(),
        Config {
            proxy_half_extents_m: [0.1, 0.05],
            ..Config::default()
        },
    )
    .expect("different proxy");
    assert_ne!(first.revision(), different.revision());
    assert_ne!(
        first.revision().geometry_sha256,
        different.revision().geometry_sha256
    );
    assert_eq!(
        first.revision().source_sha256,
        <[u8; 32]>::from(Sha256::digest(SOURCE.as_bytes()))
    );
}
#[test]
fn openings_and_unknown_flags_never_become_filled_material() {
    let unknown = build(
        SOURCE.replace(" OPENING \"NO\"", "").as_bytes(),
        Config::default(),
    )
    .expect("frames remain");
    assert_eq!(
        unknown.entities()[2].limitation,
        Some(Limitation::UnknownOpeningState)
    );
    assert!(unknown.entities()[2].indices.is_empty());
    let opening = build(
        SOURCE
            .replace("OPENING \"NO\"", "OPENING \"YES\"")
            .as_bytes(),
        Config::default(),
    )
    .expect("frames remain");
    assert_eq!(
        opening.entities()[2].limitation,
        Some(Limitation::OpeningVoid)
    );
    assert_eq!(opening.indices().len(), 72);
    let extra = SOURCE
        .replace(
            "$ LINE ASSIGNS",
            " AREA \"VOID\" FLOOR 4 \"1\" \"2\" \"3\" \"4\" 0 0 0 0\n$ LINE ASSIGNS",
        )
        .replace(
            "$ END OF MODEL FILE",
            " AREAASSIGN \"VOID\" \"Roof\" OPENING \"YES\"\n$ END OF MODEL FILE",
        );
    let host = build(extra.as_bytes(), Config::default()).expect("frames remain");
    assert!(
        host.entities()
            .iter()
            .any(|e| e.limitation == Some(Limitation::HostOpeningSubtractionUnavailable))
    );
    assert_eq!(host.indices().len(), 72);
}
#[test]
fn invalid_concave_nonplanar_and_self_intersecting_shells_are_withheld() {
    for points in [
        vec![
            [0., 0., 0.],
            [2., 0., 0.],
            [1., 1., 0.],
            [2., 2., 0.],
            [0., 2., 0.],
        ],
        vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 1.], [0., 2., 0.]],
        vec![[0., 0., 0.], [2., 2., 0.], [0., 2., 0.], [2., 0., 0.]],
    ] {
        assert!(convex_normal(&points).is_none());
    }
    let source = SOURCE.replace("POINT \"3\" 10 10", "POINT \"3\" 2 2");
    let mesh = build(source.as_bytes(), Config::default()).expect("frames remain");
    assert_eq!(
        mesh.entities()[2].limitation,
        Some(Limitation::InvalidOrConcaveShell)
    );
}
#[test]
fn bad_connectivity_is_explicit_in_source_projection_skips() {
    let source = SOURCE.replace(
        "LINE \"B\" BEAM \"1\" \"2\"",
        "LINE \"B\" BEAM \"1\" \"MISSING\"",
    );
    let mesh = build(source.as_bytes(), Config::default()).expect("remaining geometry");
    assert_eq!(mesh.projection_skips().len(), 1);
}
#[test]
fn buffer_entity_and_proxy_limits_fail_before_unbounded_output() {
    assert_eq!(
        build(
            SOURCE.as_bytes(),
            Config {
                max_buffer_bytes: 64,
                ..Config::default()
            }
        )
        .err(),
        Some(Error::Budget)
    );
    assert_eq!(
        build(
            SOURCE.as_bytes(),
            Config {
                max_entities: 1,
                ..Config::default()
            }
        )
        .err(),
        Some(Error::Budget)
    );
    assert_eq!(
        build(
            SOURCE.as_bytes(),
            Config {
                proxy_half_extents_m: [f64::NAN, 0.1],
                ..Config::default()
            }
        )
        .err(),
        Some(Error::Budget)
    );
}
#[test]
fn all_supported_length_units_are_converted_to_metres() {
    for (name, factor) in [
        ("ft", 0.3048),
        ("in", 0.0254),
        ("m", 1.0),
        ("cm", 0.01),
        ("mm", 0.001),
    ] {
        let source = SOURCE.replace("\"ft\"", &format!("\"{name}\""));
        let mesh = build(source.as_bytes(), Config::default()).expect("supported source units");
        assert!((mesh.stories()[0].elevation_m - 10.0 * factor).abs() < 1e-12);
    }
}
#[test]
fn large_world_origin_is_retained_in_f64_and_gpu_vertices_are_recentered() {
    let source = SOURCE
        .replace("POINT \"1\" 0 0", "POINT \"1\" 1000000000 0")
        .replace("POINT \"2\" 10 0", "POINT \"2\" 1000000010 0")
        .replace("POINT \"3\" 10 10", "POINT \"3\" 1000000010 10")
        .replace("POINT \"4\" 0 10", "POINT \"4\" 1000000000 10");
    let mesh = build(source.as_bytes(), Config::default()).expect("translated public fixture");
    assert!(mesh.origin_m()[0] > 1.0e8);
    assert!(
        mesh.vertices()
            .iter()
            .flat_map(|v| v.position)
            .all(|v| v.abs() < 4.0)
    );
    let centreline = mesh.entities()[0]
        .source_centerline_m
        .expect("source positions");
    assert!((centreline[0][0] - 304800000.0).abs() < 1.0e-6);
}

#[test]
fn proxy_face_normals_point_away_from_the_independent_source_centerline() {
    let mesh = build(SOURCE.as_bytes(), Config::default()).expect("public source");
    for entity in mesh
        .entities()
        .iter()
        .filter(|e| e.mode == DisplayMode::RectangularProxy)
    {
        let ends = entity.source_centerline_m.expect("source endpoints");
        let center = local(
            std::array::from_fn(|i| ends[0][i].midpoint(ends[1][i])),
            mesh.origin_m(),
        );
        for triangle in mesh.indices()[entity.indices.start as usize..entity.indices.end as usize]
            .as_chunks::<3>()
            .0
        {
            let vertices: [Vertex; 3] = triangle.map(|index| mesh.vertices()[index as usize]);
            let face_center: Point = std::array::from_fn(|i| {
                vertices
                    .iter()
                    .map(|v| f64::from(v.position[i]))
                    .sum::<f64>()
                    / 3.0
            });
            assert!(dot(sub(face_center, center), vertices[0].normal.map(f64::from)) > 0.0);
        }
    }
}
