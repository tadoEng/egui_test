use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Bounds, Counts, Entity, Fixture, FixtureError, Geometry, GeometryRevision, Group, Kind, LabId,
    OcclusionOracle, Point, Preset, Story, StoryId,
};

fn prism(min: Point, max: Point) -> Geometry {
    Geometry::Prism(Bounds { min, max })
}

fn shell(boundary: Vec<Point>) -> Geometry {
    let cross = crate::geometry::cross(
        crate::geometry::subtract(boundary[1], boundary[0]),
        crate::geometry::subtract(boundary[2], boundary[0]),
    );
    let length = crate::geometry::dot(cross, cross).sqrt();
    Geometry::Shell {
        boundary,
        normal: cross.map(|v| v / length),
        openings: Vec::new(),
    }
}

fn wall(a: [f64; 2], b: [f64; 2], lower: f64, upper: f64) -> Geometry {
    shell(vec![
        [a[0], a[1], lower],
        [b[0], b[1], lower],
        [b[0], b[1], upper],
        [a[0], a[1], upper],
    ])
}

fn entity(
    kind: Kind,
    story: StoryId,
    slot: u32,
    geometry: Geometry,
    perimeter: bool,
    core: bool,
    middle: f64,
) -> Entity {
    let points = geometry.control_points();
    let center_x = points.iter().map(|p| p[0]).sum::<f64>() / points.len() as f64;
    let mut groups = Vec::new();
    if core {
        groups.push(Group::Core);
    }
    if perimeter {
        groups.push(Group::Perimeter);
    }
    if center_x <= middle + 2.0 {
        groups.push(Group::ZoneA);
    }
    if center_x >= middle - 2.0 {
        groups.push(Group::ZoneB);
    }
    if perimeter || kind == Kind::Wall {
        groups.push(Group::LateralSystem);
    }
    Entity {
        id: LabId { kind, story, slot },
        geometry,
        groups,
    }
}

pub(crate) fn fixture(preset: Preset) -> Result<Fixture, FixtureError> {
    let (mut entities, stories) = if let Some((nx, ny)) = preset.grid() {
        grid(nx, ny, 69)
    } else {
        grid(1, 1, 2)
    };
    let occlusion = if preset == Preset::TruthTable {
        // Truth table uses 6x4 metre perimeter, a core wall and a separate shear wall.
        entities.clear();
        for story in &stories {
            let upper = story.elevation_m;
            let lower = upper - 3.0;
            for (slot, (x, y)) in (0_u32..).zip([(0.0, 0.0), (6.0, 0.0), (0.0, 4.0), (6.0, 4.0)]) {
                entities.push(entity(
                    Kind::Column,
                    story.id,
                    slot,
                    prism([x - 0.2, y - 0.2, lower], [x + 0.2, y + 0.2, upper]),
                    true,
                    false,
                    3.0,
                ));
            }
            for (slot, (min, max)) in (0_u32..).zip([
                ([0.0, -0.15, upper - 0.15], [6.0, 0.15, upper + 0.15]),
                ([0.0, 3.85, upper - 0.15], [6.0, 4.15, upper + 0.15]),
                ([-0.15, 0.0, upper - 0.15], [0.15, 4.0, upper + 0.15]),
                ([5.85, 0.0, upper - 0.15], [6.15, 4.0, upper + 0.15]),
            ]) {
                entities.push(entity(
                    Kind::Beam,
                    story.id,
                    slot,
                    prism(min, max),
                    true,
                    false,
                    3.0,
                ));
            }
            entities.push(entity(
                Kind::Wall,
                story.id,
                0,
                wall([2.0, 2.0], [4.0, 2.0], lower, upper),
                false,
                true,
                3.0,
            ));
            entities.push(entity(
                Kind::Wall,
                story.id,
                1,
                wall([5.0, 1.0], [5.0, 3.0], lower, upper),
                false,
                false,
                3.0,
            ));
            entities.push(entity(
                Kind::Slab,
                story.id,
                0,
                shell(vec![
                    [0.0, 0.0, upper],
                    [6.0, 0.0, upper],
                    [6.0, 4.0, upper],
                    [0.0, 4.0, upper],
                ]),
                false,
                false,
                3.0,
            ));
        }
        let boundary = [(0.0, 0.0), (3.0, 0.0), (4.0, 2.0), (2.0, 4.0), (0.0, 3.0)]
            .map(|(x, y)| [8.0 + x, y, 6.0 + 0.2 * x + 0.1 * y])
            .to_vec();
        entities.push(entity(
            Kind::Slab,
            StoryId(2),
            1,
            shell(boundary),
            false,
            false,
            3.0,
        ));
        entities.push(inclined_beam());
        Some(truth_occlusion())
    } else {
        None
    };
    validate(preset, entities, stories, occlusion)
}

fn inclined_beam() -> Entity {
    let length = 13.0_f64.sqrt();
    entity(
        Kind::Beam,
        StoryId(2),
        4,
        Geometry::OrientedPrism {
            start: [8.0, 1.0, 3.5],
            end: [11.0, 3.0, 5.5],
            transverse_y: [-2.0 / length, 3.0 / length, 0.0],
            half_extents: [0.15, 0.2],
        },
        false,
        false,
        3.0,
    )
}

fn truth_occlusion() -> OcclusionOracle {
    OcclusionOracle {
        ray_origin: [0.0, -10.0, 1.5],
        ray_direction: [0.0, 1.0, 0.0],
        near: LabId {
            kind: Kind::Column,
            story: StoryId(1),
            slot: 0,
        },
        far: LabId {
            kind: Kind::Column,
            story: StoryId(1),
            slot: 2,
        },
        near_position: [0.0, -0.2, 1.5],
        far_position: [0.0, 3.8, 1.5],
    }
}

fn grid(nx: u16, ny: u16, levels: u16) -> (Vec<Entity>, Vec<Story>) {
    let mut entities = Vec::new();
    let stories: Vec<_> = (1..=levels)
        .map(|s| Story {
            id: StoryId(s),
            elevation_m: f64::from(s) * 3.0,
        })
        .collect();
    let width = f64::from(nx) * 4.0;
    let depth = f64::from(ny) * 4.0;
    for story in &stories {
        let upper = story.elevation_m;
        let lower = upper - 3.0;
        for x in 0..=nx {
            for y in 0..=ny {
                let px = f64::from(x) * 4.0;
                let py = f64::from(y) * 4.0;
                let edge = x == 0 || x == nx || y == 0 || y == ny;
                entities.push(entity(
                    Kind::Column,
                    story.id,
                    u32::from(x) * u32::from(ny + 1) + u32::from(y),
                    prism([px - 0.2, py - 0.2, lower], [px + 0.2, py + 0.2, upper]),
                    edge,
                    false,
                    width / 2.0,
                ));
            }
        }
        let mut slot = 0;
        for x in 0..nx {
            for y in 0..=ny {
                let px = f64::from(x) * 4.0;
                let py = f64::from(y) * 4.0;
                entities.push(entity(
                    Kind::Beam,
                    story.id,
                    slot,
                    prism(
                        [px, py - 0.15, upper - 0.15],
                        [px + 4.0, py + 0.15, upper + 0.15],
                    ),
                    y == 0 || y == ny,
                    false,
                    width / 2.0,
                ));
                slot += 1;
            }
        }
        for x in 0..=nx {
            for y in 0..ny {
                let px = f64::from(x) * 4.0;
                let py = f64::from(y) * 4.0;
                entities.push(entity(
                    Kind::Beam,
                    story.id,
                    slot,
                    prism(
                        [px - 0.15, py, upper - 0.15],
                        [px + 0.15, py + 4.0, upper + 0.15],
                    ),
                    x == 0 || x == nx,
                    false,
                    width / 2.0,
                ));
                slot += 1;
            }
        }
        for x in 0..nx {
            for y in 0..ny {
                let px = f64::from(x) * 4.0;
                let py = f64::from(y) * 4.0;
                entities.push(entity(
                    Kind::Slab,
                    story.id,
                    u32::from(x) * u32::from(ny) + u32::from(y),
                    shell(vec![
                        [px, py, upper],
                        [px + 4.0, py, upper],
                        [px + 4.0, py + 4.0, upper],
                        [px, py + 4.0, upper],
                    ]),
                    false,
                    false,
                    width / 2.0,
                ));
            }
        }
        add_walls(&mut entities, story.id, width, depth, lower, upper);
    }
    (entities, stories)
}

fn add_walls(
    entities: &mut Vec<Entity>,
    story: StoryId,
    width: f64,
    depth: f64,
    lower: f64,
    upper: f64,
) {
    let corners = [
        [width * 0.25, depth * 0.25],
        [width * 0.75, depth * 0.25],
        [width * 0.75, depth * 0.75],
        [width * 0.25, depth * 0.75],
    ];
    for (slot, corner) in (0_u32..).zip(0..4) {
        entities.push(entity(
            Kind::Wall,
            story,
            slot,
            wall(corners[corner], corners[(corner + 1) % 4], lower, upper),
            false,
            true,
            width / 2.0,
        ));
    }
    for (slot, fraction) in [(4, 0.125), (5, 0.875)] {
        entities.push(entity(
            Kind::Wall,
            story,
            slot,
            wall(
                [width * fraction, depth * 0.2],
                [width * fraction, depth * 0.8],
                lower,
                upper,
            ),
            false,
            false,
            width / 2.0,
        ));
    }
}

pub(crate) fn validate(
    preset: Preset,
    mut entities: Vec<Entity>,
    stories: Vec<Story>,
    occlusion: Option<OcclusionOracle>,
) -> Result<Fixture, FixtureError> {
    if entities.is_empty() {
        return Err(FixtureError::Empty);
    }
    let mut previous = 0;
    let mut elevation = 0.0;
    for story in &stories {
        if story.id.0 != previous + 1
            || !story.elevation_m.is_finite()
            || story.elevation_m <= elevation
        {
            return Err(FixtureError::InvalidStory);
        }
        previous = story.id.0;
        elevation = story.elevation_m;
    }
    let mut story_members: BTreeMap<_, Vec<_>> =
        stories.iter().map(|s| (s.id, Vec::new())).collect();
    let mut group_members: BTreeMap<_, Vec<_>> =
        Group::ALL.into_iter().map(|g| (g, Vec::new())).collect();
    let mut counts = Counts {
        by_kind: Kind::ALL.into_iter().map(|k| (k, 0)).collect(),
        by_story: BTreeMap::new(),
        by_group: BTreeMap::new(),
        control_points: 0,
        reference_mesh_vertices: 0,
        triangles: 0,
    };
    let mut bounds = Bounds {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    entities.sort_by_key(|e| e.id);
    let mut previous = None;
    for e in &entities {
        if previous == Some(e.id) {
            return Err(FixtureError::DuplicateId(e.id));
        }
        previous = Some(e.id);
        let members = story_members
            .get_mut(&e.id.story)
            .ok_or(FixtureError::InvalidStory)?;
        members.push(e.id);
        if e.groups.contains(&Group::Empty)
            || e.groups.iter().copied().collect::<BTreeSet<_>>().len() != e.groups.len()
        {
            return Err(FixtureError::InvalidGroup(e.id));
        }
        e.geometry.validate(e.id)?;
        for g in &e.groups {
            if let Some(members) = group_members.get_mut(g) {
                members.push(e.id);
            }
        }
        *counts.by_kind.entry(e.id.kind).or_default() += 1;
        let points = e.geometry.control_points();
        counts.control_points += points.len();
        for p in points {
            bounds.extend(p);
        }
        let (vertices, triangles) = e.geometry.reference_mesh_counts();
        counts.reference_mesh_vertices += vertices;
        counts.triangles += triangles;
    }
    counts.by_story = story_members.iter().map(|(k, v)| (*k, v.len())).collect();
    counts.by_group = group_members.iter().map(|(k, v)| (*k, v.len())).collect();
    Ok(Fixture {
        revision: GeometryRevision {
            preset,
            formula_version: if preset == Preset::TruthTable { 2 } else { 1 },
        },
        entities,
        stories,
        bounds,
        counts,
        story_members,
        group_members,
        occlusion,
    })
}
