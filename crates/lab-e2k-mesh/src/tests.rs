use super::*;
use crate::public_fixture::E2K as SOURCE;
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
    // As in the pinned ext-api, an undeclared flag is not a void: the shell is
    // drawn, and the undeclared state is disclosed rather than hidden.
    let undeclared = &unknown.entities()[2];
    assert_eq!(
        (undeclared.mode, undeclared.limitation),
        (DisplayMode::ZeroThicknessSurface, None)
    );
    assert!(!undeclared.indices.is_empty());
    assert_eq!(undeclared.shell.as_ref().expect("shell").opening, None);
    assert_eq!(
        build(SOURCE.as_bytes(), Config::default())
            .expect("source")
            .entities()[2]
            .shell
            .as_ref()
            .expect("shell")
            .opening,
        Some(false)
    );
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
    // Nonplanar and self-intersecting rings stay withheld after ear clipping.
    for ring in &[
        vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 1.], [0., 2., 0.]],
        vec![[0., 0., 0.], [2., 2., 0.], [0., 2., 0.], [2., 0., 0.]],
        vec![[0., 0., 0.], [1., 0., 0.], [2., 0., 0.]],
    ] {
        assert!(triangulate::ear_clip(ring).is_none(), "{ring:?}");
    }
    let bowtie = SOURCE
        .replace("POINT \"3\" 10 10", "POINT \"3\" 0 10")
        .replace("POINT \"4\" 0 10", "POINT \"4\" 10 10");
    let mesh = build(bowtie.as_bytes(), Config::default()).expect("frames remain");
    assert_eq!(
        mesh.entities()[2].limitation,
        Some(Limitation::InvalidShellRing)
    );
}
fn ring_area(mesh: &Mesh, e: &EntityMesh) -> f64 {
    mesh.indices()[e.indices.start as usize..e.indices.end as usize]
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| {
            let p: [Point; 3] = t.map(|i| mesh.vertices()[i as usize].position.map(f64::from));
            let n = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            dot(n, n).sqrt() / 2.0
        })
        .sum()
}
#[test]
fn concave_simple_rings_are_ear_clipped_with_exact_area_and_source_winding() {
    // Dart (0,0) (10,0) (2,2) (0,10) ft: area 20 ft^2, concave at (2,2).
    let dart = SOURCE.replace("POINT \"3\" 10 10", "POINT \"3\" 2 2");
    let mesh = build(dart.as_bytes(), Config::default()).expect("source");
    let shell = &mesh.entities()[2];
    assert_eq!(
        (shell.mode, shell.limitation),
        (DisplayMode::ZeroThicknessSurface, None)
    );
    assert!((ring_area(&mesh, shell) - 20.0 * 0.3048 * 0.3048).abs() < 1e-6);
    // L-shape with a collinear mid-edge vertex, plus its reversed winding.
    let l_shape = [
        [0., 0., 0.],
        [2., 0., 0.],
        [4., 0., 0.],
        [4., 2., 0.],
        [2., 2., 0.],
        [2., 4., 0.],
        [0., 4., 0.],
    ];
    for ring in [l_shape.to_vec(), l_shape.iter().rev().copied().collect()] {
        let (normal, triangles) = triangulate::ear_clip(&ring).expect("simple concave ring");
        let area: f64 = triangles
            .iter()
            .map(|t| {
                let n = cross(sub(ring[t[1]], ring[t[0]]), sub(ring[t[2]], ring[t[0]]));
                assert!(
                    dot(n, normal) > 0.0,
                    "every triangle winds with the ring normal"
                );
                dot(n, n).sqrt() / 2.0
            })
            .sum();
        assert!((area - 12.0).abs() < 1e-12, "{area}");
    }
    assert_eq!(
        triangulate::ear_clip(&l_shape).expect("ring").0,
        [0.0, 0.0, 1.0]
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
        .filter(|e| e.mode == DisplayMode::RectangularFrame)
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
/// Two-story public fixture with three independent shells and one explicit void
/// that lies wholly inside floor `S` only. Wall `W` and the roof-level floor `R`
/// share no material with the void.
fn scoped_opening_source() -> String {
    SOURCE
        .replace(
            " POINT \"4\" 0 10\n",
            " POINT \"4\" 0 10\n POINT \"5\" 4 4\n POINT \"6\" 6 4\n POINT \"7\" 6 6\n POINT \"8\" 4 6\n POINT \"9\" 20 0\n POINT \"10\" 30 0\n POINT \"11\" 30 10\n POINT \"12\" 20 10\n",
        )
        .replace(
            "$ LINE ASSIGNS",
            " AREA \"VOID\" FLOOR 4 \"5\" \"6\" \"7\" \"8\" 0 0 0 0\n AREA \"R\" FLOOR 4 \"9\" \"10\" \"11\" \"12\" 0 0 0 0\n AREA \"W\" PANEL 4 \"2\" \"3\" \"3\" \"2\" 0 0 0 0\n$ LINE ASSIGNS",
        )
        .replace(
            "$ END OF MODEL FILE",
            " AREAASSIGN \"VOID\" \"Roof\" OPENING \"YES\"\n AREAASSIGN \"R\" \"Roof\" OPENING \"NO\"\n AREAASSIGN \"W\" \"Roof\" OPENING \"NO\"\n$ END OF MODEL FILE",
        )
}
fn entity<'a>(mesh: &'a Mesh, id: &str) -> &'a EntityMesh {
    mesh.entities()
        .iter()
        .find(|e| matches!(&e.key, EntityKey::Instance { id: key, .. } if key.as_str() == id))
        .unwrap_or_else(|| panic!("{id} should be projected"))
}
#[test]
fn an_opening_withholds_only_coplanar_overlapping_hosts() {
    let mesh = build(scoped_opening_source().as_bytes(), Config::default()).expect("source");
    assert_eq!(
        entity(&mesh, "VOID").limitation,
        Some(Limitation::OpeningVoid)
    );
    let host = entity(&mesh, "S");
    assert_eq!(
        host.limitation,
        Some(Limitation::HostOpeningSubtractionUnavailable),
        "the void lies inside S, so S must not be painted as solid material"
    );
    assert!(host.indices.is_empty());
    for independent in ["R", "W"] {
        let shell = entity(&mesh, independent);
        assert_eq!(
            (shell.mode, shell.limitation),
            (DisplayMode::ZeroThicknessSurface, None),
            "{independent} shares no material with the void and must stay visible"
        );
        assert!(!shell.indices.is_empty());
    }
}
#[test]
fn an_opening_overlap_is_detected_without_any_vertex_strictly_inside() {
    // A full-width strip void whose vertices all lie on the host boundary.
    let source = scoped_opening_source()
        .replace("POINT \"5\" 4 4", "POINT \"5\" 0 0")
        .replace("POINT \"6\" 6 4", "POINT \"6\" 10 0")
        .replace("POINT \"7\" 6 6", "POINT \"7\" 10 5")
        .replace("POINT \"8\" 4 6", "POINT \"8\" 0 5");
    let mesh = build(source.as_bytes(), Config::default()).expect("source");
    assert_eq!(
        entity(&mesh, "S").limitation,
        Some(Limitation::HostOpeningSubtractionUnavailable)
    );
    assert_eq!(entity(&mesh, "R").limitation, None);
}
#[test]
fn an_opening_that_only_touches_a_neighbour_edge_does_not_hide_it() {
    // Void occupies x 10..14, sharing only the x=10 edge with S and lying in
    // the gap before R. Neither neighbour loses material.
    let source = scoped_opening_source()
        .replace("POINT \"5\" 4 4", "POINT \"5\" 10 0")
        .replace("POINT \"6\" 6 4", "POINT \"6\" 14 0")
        .replace("POINT \"7\" 6 6", "POINT \"7\" 14 10")
        .replace("POINT \"8\" 4 6", "POINT \"8\" 10 10");
    let mesh = build(source.as_bytes(), Config::default()).expect("source");
    for neighbour in ["S", "R", "W"] {
        assert_eq!(entity(&mesh, neighbour).limitation, None, "{neighbour}");
    }
}
#[test]
fn a_degenerate_opening_conservatively_withholds_hosts_by_bounds() {
    // Collinear void ring: no plane, so the backend names no host, but overlap
    // cannot be proven absent either.
    let source = scoped_opening_source()
        .replace("POINT \"7\" 6 6", "POINT \"7\" 8 4")
        .replace("POINT \"8\" 4 6", "POINT \"8\" 5 4");
    let mesh = build(source.as_bytes(), Config::default()).expect("source");
    assert_eq!(
        entity(&mesh, "S").limitation,
        Some(Limitation::OverlapsUnsubtractedOpening)
    );
    assert_eq!(entity(&mesh, "R").limitation, None);
}
#[test]
fn a_partial_overlap_is_not_a_backend_host_but_is_never_painted_solid() {
    // Void x 8..12 straddles S's x=10 edge: not contained, so not a host.
    let source = scoped_opening_source()
        .replace("POINT \"5\" 4 4", "POINT \"5\" 8 4")
        .replace("POINT \"6\" 6 4", "POINT \"6\" 12 4")
        .replace("POINT \"7\" 6 6", "POINT \"7\" 12 6")
        .replace("POINT \"8\" 4 6", "POINT \"8\" 8 6");
    let mesh = build(source.as_bytes(), Config::default()).expect("source");
    let host = entity(&mesh, "S");
    assert_eq!(
        host.limitation,
        Some(Limitation::OverlapsUnsubtractedOpening)
    );
    assert!(host.indices.is_empty());
    assert_eq!(entity(&mesh, "R").limitation, None);
    assert_eq!(entity(&mesh, "W").limitation, None);
}
#[test]
fn a_perpendicular_wall_through_the_void_footprint_is_not_its_host() {
    let source = scoped_opening_source()
        .replace(
            " POINT \"9\" 20 0\n",
            " POINT \"9\" 20 0\n POINT \"13\" 5 0\n POINT \"14\" 5 10\n",
        )
        .replace(
            "AREA \"W\" PANEL 4 \"2\" \"3\" \"3\" \"2\"",
            "AREA \"W\" PANEL 4 \"13\" \"14\" \"14\" \"13\"",
        );
    let mesh = build(source.as_bytes(), Config::default()).expect("source");
    assert_eq!(entity(&mesh, "W").limitation, None);
    assert_eq!(
        entity(&mesh, "S").limitation,
        Some(Limitation::HostOpeningSubtractionUnavailable)
    );
}
/// Public fixture with one section per backend fidelity arm. ft units: D 2 B 1
/// is 0.6096 m deep (local 2) by 0.3048 m wide (local 3).
fn sectioned_source() -> String {
    SOURCE
        .replace(
            "$ POINT COORDINATES",
            "$ FRAME SECTIONS\n FRAMESECTION \"RECT\" MATERIAL \"C\" SHAPE \"Concrete Rectangular\" D 2 B 1\n FRAMESECTION \"TUBE\" MATERIAL \"S\" SHAPE \"Steel Tube\" D 1 B 1\n FRAMESECTION \"ROUND\" MATERIAL \"C\" SHAPE \"Concrete Circle\" D 1.5\n FRAMESECTION \"W14\" MATERIAL \"S\" SHAPE \"W14X90\" FILE \"AISC16\"\n$ POINT COORDINATES",
        )
        .replace(
            " LINE \"B\" BEAM \"1\" \"2\" 0\n",
            " LINE \"B\" BEAM \"1\" \"2\" 0\n LINE \"G\" BEAM \"2\" \"3\" 0\n LINE \"K\" BEAM \"3\" \"4\" 0\n LINE \"X\" BRACE \"4\" \"2\" 0\n LINE \"T\" BEAM \"4\" \"1\" 0\n LINE \"N\" BRACE \"1\" \"3\" 0\n",
        )
        .replace(
            " LINEASSIGN \"C\" \"Roof\"\n",
            " LINEASSIGN \"C\" \"Roof\" SECTION \"RECT\"\n",
        )
        .replace(
            " LINEASSIGN \"B\" \"Roof\"\n",
            " LINEASSIGN \"B\" \"Roof\" SECTION \"RECT\" CARDINALPT 8\n LINEASSIGN \"G\" \"Roof\" SECTION \"ROUND\"\n LINEASSIGN \"K\" \"Roof\" SECTION \"W14\"\n LINEASSIGN \"X\" \"Roof\" SECTION \"RECT\" CARDINALPT 5\n LINEASSIGN \"T\" \"Roof\" SECTION \"TUBE\" CARDINALPT 5\n LINEASSIGN \"N\" \"Roof\" SECTION \"NOPE\"\n",
        )
}
fn extent(mesh: &Mesh, e: &EntityMesh) -> [f64; 3] {
    let range = e.indices.start as usize..e.indices.end as usize;
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for &i in &mesh.indices()[range] {
        for axis in 0..3 {
            let v = f64::from(mesh.vertices()[i as usize].position[axis]);
            lo[axis] = lo[axis].min(v);
            hi[axis] = hi[axis].max(v);
        }
    }
    std::array::from_fn(|axis| hi[axis] - lo[axis])
}
fn frame<'a>(mesh: &'a Mesh, id: &str) -> &'a FrameSource {
    entity(mesh, id)
        .frame
        .as_ref()
        .expect("frame source attributes")
}
use FrameReason as R;
#[test]
fn rectangular_source_sections_use_source_depth_and_width_in_default_local_axes() {
    let mesh = build(sectioned_source().as_bytes(), Config::default()).expect("source");
    let near = |a: f64, b: f64| (a - b).abs() < 1e-5;
    let column = frame(&mesh, "C");
    assert_eq!(column.fidelity, GeometryFidelity::Resolved);
    assert_eq!(column.reasons, [R::LocalAxisRotationUnavailable]);
    assert_eq!(column.section.as_deref(), Some("RECT"));
    assert_eq!(column.material.as_deref(), Some("C"));
    assert!(near(column.depth_m.expect("D"), 0.6096) && near(column.width_m.expect("B"), 0.3048));
    // Column: local 2 is global +X (depth), local 3 is +Y (width).
    let [x, y, z] = extent(&mesh, entity(&mesh, "C"));
    assert!(
        near(x, 0.6096) && near(y, 0.3048) && near(z, 3.048),
        "{x} {y} {z}"
    );
    // Beam along +X: local 2 is +Z (depth), local 3 is horizontal (width).
    let [x, y, z] = extent(&mesh, entity(&mesh, "B"));
    assert!(
        near(x, 3.048) && near(y, 0.3048) && near(z, 0.6096),
        "{x} {y} {z}"
    );
}
#[test]
fn diagonal_and_inclined_members_use_the_csi_default_local_two_axis() {
    let mesh = build(sectioned_source().as_bytes(), Config::default()).expect("source");
    // Plan diagonal at roof elevation: local 1 is horizontal, so depth is vertical.
    let [_, _, z] = extent(&mesh, entity(&mesh, "X"));
    assert!((z - 0.6096).abs() < 1e-5, "{z}");
    // Truly inclined local 1: local 2 is unit, perpendicular and upward in the
    // vertical plane through local 1; vertical members take global +X.
    let axis = unit([3.0, 4.0, 5.0]).expect("axis");
    let two = default_local_two(axis);
    assert!((dot(two, two) - 1.0).abs() < 1e-12 && dot(two, axis).abs() < 1e-12);
    assert!(two[2] > 0.0 && dot(cross(axis, [0.0, 0.0, 1.0]), two).abs() < 1e-12);
    assert_eq!(default_local_two([0.0, 0.0, -1.0]), [1.0, 0.0, 0.0]);
    assert_eq!(default_local_two([1.0e-4, 0.0, 1.0]), [1.0, 0.0, 0.0]);
}
#[test]
fn unproven_profiles_and_unresolved_sections_follow_the_backend_reasons() {
    let mesh = build(sectioned_source().as_bytes(), Config::default()).expect("source");
    // A non-rectangular family with both dimensions: the D x B envelope is
    // swept, as the backend viewer does, but never Resolved.
    let tube = frame(&mesh, "T");
    assert_eq!(tube.fidelity, GeometryFidelity::Approximate);
    assert!(tube.swept());
    assert_eq!(
        tube.reasons,
        [
            R::RectangularProfileEnvelope,
            R::LocalAxisRotationUnavailable,
            R::CardinalPointOffsetIgnored
        ],
        "backend declaration order, as ext-api publishes them"
    );
    // Unresolved dimensions: Symbolic, a typed missing-dimension reason, and
    // the labelled fixed proxy in place of the backend's bare centreline.
    for (id, reason) in [
        ("G", R::SectionDimensionsMissing),
        ("K", R::SectionDimensionsInLibrary),
        ("N", R::SectionDefinitionMissing),
    ] {
        let source = frame(&mesh, id);
        assert_eq!(source.fidelity, GeometryFidelity::Symbolic, "{id}");
        assert_eq!(source.reasons, [reason, R::FixedDisplayProxy], "{id}");
        let [_, _, z] = extent(&mesh, entity(&mesh, id));
        assert!((z - 0.1).abs() < 1e-6, "{id} keeps the fixed proxy");
    }
    assert_eq!(
        frame(&mesh, "G").width_m,
        None,
        "a circle declares no width"
    );
    let plain = build(SOURCE.as_bytes(), Config::default()).expect("source");
    let unassigned = frame(&plain, "B");
    assert_eq!(
        unassigned.reasons,
        [R::SectionNotAssigned, R::FixedDisplayProxy]
    );
    assert_eq!(unassigned.section, None);
}
#[test]
fn cardinal_points_joint_offsets_and_mirrors_degrade_like_the_backend() {
    let mesh = build(sectioned_source().as_bytes(), Config::default()).expect("source");
    let beam = frame(&mesh, "B");
    assert_eq!(beam.cardinal_point, Some(8));
    assert_eq!(beam.fidelity, GeometryFidelity::Approximate);
    assert!(beam.reasons.contains(&R::CardinalPointOffsetIgnored));
    // Point 5 coincides with the centroid only on a proven doubly symmetric family.
    assert_eq!(frame(&mesh, "X").fidelity, GeometryFidelity::Resolved);
    assert!(
        frame(&mesh, "T")
            .reasons
            .contains(&R::CardinalPointOffsetIgnored)
    );
    let source = sectioned_source()
        .replace(
            "LINEASSIGN \"C\" \"Roof\" SECTION \"RECT\"",
            "LINEASSIGN \"C\" \"Roof\" SECTION \"RECT\" OFFSETXI 0.5 MIRROR2 \"Yes\"",
        )
        .replace(
            "SECTION \"TUBE\" CARDINALPT 5",
            "SECTION \"TUBE\" MIRROR3 \"Yes\"",
        );
    let scene = ext_e2k::project_scene(&ext_e2k::parse_str(&source).expect("parse"));
    assert_eq!(
        scene
            .frames
            .iter()
            .filter(|f| f.insertion.is_mirrored())
            .count(),
        2,
        "both mirror rows parse"
    );
    let mesh = build(source.as_bytes(), Config::default()).expect("source");
    let column = frame(&mesh, "C");
    assert!(column.reasons.contains(&R::FrameJointOffsetsIgnored));
    assert!(
        !column.reasons.contains(&R::SectionMirrorIgnored),
        "a mirrored rectangle maps onto itself"
    );
    assert!(frame(&mesh, "T").reasons.contains(&R::SectionMirrorIgnored));
}
#[test]
fn a_section_change_between_versions_changes_geometry_and_source_attribution() {
    let v1 = build(sectioned_source().as_bytes(), Config::default()).expect("v1");
    let v2_source = sectioned_source().replace("D 2 B 1\n", "D 3 B 1\n");
    let v2 = build(v2_source.as_bytes(), Config::default()).expect("v2");
    assert_ne!(v1.revision().geometry_sha256, v2.revision().geometry_sha256);
    let depth = |m: &Mesh| frame(m, "B").depth_m;
    assert!((depth(&v2).expect("D") - 0.9144).abs() < 1e-9);
    assert_ne!(depth(&v1), depth(&v2));
}
#[test]
fn shell_source_attributes_carry_section_thickness_and_insertion_without_inferring_kind() {
    let source = SOURCE
        .replace(
            "$ POINT COORDINATES",
            "$ SLAB PROPERTIES\n SHELLPROP \"SL\" PROPTYPE \"Slab\" MATERIAL \"C\" MODELINGTYPE \"ShellThin\" SLABTYPE \"Slab\" SLABTHICKNESS 0.5\n$ POINT COORDINATES",
        )
        .replace(
            "AREAASSIGN \"S\" \"Roof\" OPENING \"NO\"",
            "AREAASSIGN \"S\" \"Roof\" SECTION \"SL\" OPENING \"NO\" CARDINALPOINT \"TOP\"",
        );
    let mesh = build(source.as_bytes(), Config::default()).expect("source");
    let shell = entity(&mesh, "S")
        .shell
        .as_ref()
        .expect("shell source attributes");
    assert_eq!(shell.section.as_deref(), Some("SL"));
    assert!((shell.thickness_m.expect("thickness") - 0.1524).abs() < 1e-9);
    assert_eq!(shell.insertion_point.as_deref(), Some("TOP"));
    assert!(!shell.thickness_drawn, "surfaces stay zero thickness");
    assert!(entity(&mesh, "B").shell.is_none());
}
mod picking {
    use super::*;
    use crate::pick::{PickIndex, Ray, Selection};
    /// Column at plan (0,0), beam along y=0 at roof, roof floor 0..10 ft square,
    /// wall on x=10 ft. Metres: roof 3.048, plan edge 3.048, proxy 0.1 square.
    fn source() -> String {
        SOURCE
            .replace(
                "$ LINE ASSIGNS",
                " AREA \"W\" PANEL 4 \"2\" \"3\" \"3\" \"2\" 0 0 0 0\n$ LINE ASSIGNS",
            )
            .replace(
                "$ END OF MODEL FILE",
                " AREAASSIGN \"W\" \"Roof\" OPENING \"NO\"\n$ END OF MODEL FILE",
            )
    }
    struct Scene {
        mesh: Mesh,
        index: PickIndex,
    }
    impl Scene {
        fn new(source: &str) -> Self {
            let mesh = build(source.as_bytes(), Config::default()).expect("source");
            let index = PickIndex::new(mesh.vertices(), mesh.indices(), mesh.entities());
            Self { mesh, index }
        }
        /// World-metre origin and unit direction; returns the picked source id.
        fn pick_window(
            &self,
            from: Point,
            towards: Point,
            window: [f64; 2],
            hidden: &[&str],
        ) -> Option<(String, usize)> {
            let origin = sub(from, self.mesh.origin_m());
            let ray = Ray {
                origin,
                direction: unit(towards).expect("direction"),
                min_distance: window[0],
                max_distance: window[1],
            };
            let entities = self.mesh.entities();
            self.index
                .pick(
                    self.mesh.vertices(),
                    self.mesh.indices(),
                    entities,
                    &ray,
                    |i| !hidden.contains(&id(&entities[i])),
                )
                .map(|hit| (id(&entities[hit.entity]).to_owned(), hit.coincident))
        }
        fn pick(&self, from: Point, towards: Point, hidden: &[&str]) -> Option<String> {
            self.pick_window(from, towards, [0.0, 1.0e3], hidden)
                .map(|(id, _)| id)
        }
    }
    fn id(entity: &EntityMesh) -> &str {
        match &entity.key {
            EntityKey::Instance { id, .. } => id.as_str(),
            _ => "",
        }
    }
    const DOWN: Point = [0.0, 0.0, -1.0];
    const EAST: Point = [1.0, 0.0, 0.0];
    #[test]
    fn each_family_is_picked_at_its_own_surface() {
        let s = Scene::new(&source());
        assert_eq!(s.pick([1.5, 1.5, 10.0], DOWN, &[]).as_deref(), Some("S"));
        assert_eq!(s.pick([1.5, 0.0, 10.0], DOWN, &[]).as_deref(), Some("B"));
        assert_eq!(s.pick([-5.0, 0.0, 1.5], EAST, &[]).as_deref(), Some("C"));
        assert_eq!(s.pick([-5.0, 1.5, 1.5], EAST, &[]).as_deref(), Some("W"));
    }
    #[test]
    fn the_nearest_visible_surface_wins_and_back_faces_are_pickable() {
        let s = Scene::new(&source());
        // Beam top (3.098) is above the floor plane (3.048): beam occludes floor.
        assert_eq!(s.pick([1.5, 0.0, 10.0], DOWN, &[]).as_deref(), Some("B"));
        // 0.2 m off the 0.1 m beam: the floor.
        assert_eq!(s.pick([1.5, 0.2, 10.0], DOWN, &[]).as_deref(), Some("S"));
        // Column at x=0 occludes the wall at x=3.048 along the same ray.
        assert_eq!(s.pick([-5.0, 0.0, 1.5], EAST, &[]).as_deref(), Some("C"));
        // Wall seen from behind, and the floor from below: two-sided like the GPU.
        assert_eq!(
            s.pick([10.0, 1.5, 1.5], [-1.0, 0.0, 0.0], &[]).as_deref(),
            Some("W")
        );
        assert_eq!(
            s.pick([1.5, 1.5, -5.0], [0.0, 0.0, 1.0], &[]).as_deref(),
            Some("S")
        );
    }
    #[test]
    fn empty_space_hidden_and_clipped_surfaces_are_never_picked() {
        let s = Scene::new(&source());
        assert_eq!(s.pick([10.0, 10.0, 10.0], DOWN, &[]), None);
        assert_eq!(
            s.pick([1.5, 1.5, 10.0], [0.0, 0.0, 1.0], &[]),
            None,
            "behind ray"
        );
        assert_eq!(s.pick([1.5, 0.0, 10.0], DOWN, &["B"]).as_deref(), Some("S"));
        assert_eq!(s.pick([1.5, 1.5, 10.0], DOWN, &["S"]), None);
        // Near plane past the beam's top face (t 6.902) but before the floor (6.952).
        assert_eq!(
            s.pick_window([1.5, 0.0, 10.0], DOWN, [6.92, 1.0e3], &[]),
            Some(("S".to_owned(), 0))
        );
        // Far plane before every surface.
        assert_eq!(s.pick_window([1.5, 1.5, 10.0], DOWN, [0.0, 6.0], &[]), None);
    }
    #[test]
    fn withheld_entities_and_corrupt_ranges_are_unpickable_without_panicking() {
        let withheld = Scene::new(&source().replace(
            "AREAASSIGN \"S\" \"Roof\" OPENING \"NO\"",
            "AREAASSIGN \"S\" \"Roof\" OPENING \"YES\"",
        ));
        assert_eq!(withheld.pick([1.5, 1.5, 10.0], DOWN, &[]), None);
        let s = Scene::new(&source());
        let mut entities = s.mesh.entities().to_vec();
        for entity in &mut entities {
            entity.indices = 0..u32::MAX;
        }
        let index = PickIndex::new(s.mesh.vertices(), s.mesh.indices(), &entities);
        let ray = Ray {
            origin: sub([1.5, 1.5, 10.0], s.mesh.origin_m()),
            direction: DOWN,
            min_distance: 0.0,
            max_distance: 1.0e3,
        };
        assert_eq!(
            index.pick(s.mesh.vertices(), s.mesh.indices(), &entities, &ray, |_| {
                true
            }),
            None
        );
    }
    #[test]
    fn coplanar_overlaps_are_deterministic_and_disclosed() {
        let s = Scene::new(
            &source()
                .replace(
                    "$ LINE ASSIGNS",
                    " AREA \"T\" FLOOR 4 \"1\" \"2\" \"3\" \"4\" 0 0 0 0\n$ LINE ASSIGNS",
                )
                .replace(
                    "$ END OF MODEL FILE",
                    " AREAASSIGN \"T\" \"Roof\" OPENING \"NO\"\n$ END OF MODEL FILE",
                ),
        );
        let first = s.pick_window([1.5, 1.5, 10.0], DOWN, [0.0, 1.0e3], &[]);
        assert_eq!(
            first,
            Some(("S".to_owned(), 1)),
            "lowest index, one coincident"
        );
        assert_eq!(
            first,
            s.pick_window([1.5, 1.5, 10.0], DOWN, [0.0, 1.0e3], &[])
        );
    }
    #[test]
    fn selection_replaces_toggles_clears_and_expires_with_its_scope() {
        let v1 = build(source().as_bytes(), Config::default()).expect("v1");
        let v2 = build(sectioned_source().as_bytes(), Config::default()).expect("v2");
        let (a, b) = ((v1.revision(), 1), (v2.revision(), 2));
        let mut selection = Selection::default();
        selection.click(a, Some(2), false);
        assert_eq!(selection.entities(a), [2]);
        selection.click(a, Some(0), false);
        assert_eq!(selection.entities(a), [0], "click replaces");
        selection.click(a, Some(3), true);
        assert_eq!(selection.entities(a), [0, 3], "ctrl adds");
        selection.click(a, None, true);
        assert_eq!(selection.entities(a), [0, 3], "ctrl on empty space keeps");
        selection.click(a, Some(0), true);
        assert_eq!(selection.entities(a), [3], "ctrl toggles off");
        selection.retain_visible(|i| i != 3);
        assert!(selection.entities(a).is_empty(), "hidden entities drop out");
        selection.click(a, Some(1), false);
        assert!(
            selection.entities(b).is_empty(),
            "other version sees nothing"
        );
        assert!(
            selection.entities((v1.revision(), 3)).is_empty(),
            "same bytes, new load"
        );
        selection.click(b, Some(4), true);
        assert_eq!(selection.entities(b), [4]);
        assert!(selection.entities(a).is_empty(), "old scope expired");
        selection.click(b, None, false);
        assert!(selection.entities(b).is_empty(), "empty click clears");
    }
}
/// Story "Mezz" sits 0.001 ft (0.3 mm) above "Roof": inside the overlap band,
/// but a distinct source-story identity.
fn cross_story_source(void_story: &str) -> String {
    scoped_opening_source()
        .replace(
            " STORY \"Roof\" HEIGHT 10\n",
            " STORY \"Mezz\" HEIGHT 0.001\n STORY \"Roof\" HEIGHT 10\n",
        )
        .replace(
            "AREAASSIGN \"VOID\" \"Roof\" OPENING \"YES\"",
            &format!("AREAASSIGN \"VOID\" \"{void_story}\" OPENING \"YES\""),
        )
}
#[test]
fn an_opening_on_another_story_never_withholds_a_near_coincident_shell() {
    let mesh = build(cross_story_source("Mezz").as_bytes(), Config::default()).expect("source");
    assert_eq!(
        entity(&mesh, "VOID").limitation,
        Some(Limitation::OpeningVoid)
    );
    for shell in ["S", "R", "W"] {
        assert_eq!(entity(&mesh, shell).limitation, None, "{shell}");
    }
    // Same geometry with a same-story partial overlap is still withheld.
    let partial = cross_story_source("Roof")
        .replace("POINT \"5\" 4 4", "POINT \"5\" 8 4")
        .replace("POINT \"6\" 6 4", "POINT \"6\" 12 4")
        .replace("POINT \"7\" 6 6", "POINT \"7\" 12 6")
        .replace("POINT \"8\" 4 6", "POINT \"8\" 8 6");
    let mesh = build(partial.as_bytes(), Config::default()).expect("source");
    assert_eq!(
        entity(&mesh, "S").limitation,
        Some(Limitation::OverlapsUnsubtractedOpening)
    );
}
