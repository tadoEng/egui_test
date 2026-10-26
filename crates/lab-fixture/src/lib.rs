//! Authored synthetic geometry in metres, Z up. No ETABS, UI or GPU dependencies.
//!
//! Identities and revisions are lab-only. Rectangular prisms and zero-thickness
//! convex shells are test geometry, not engineering sections or source metadata.

use std::{collections::BTreeMap, error::Error, fmt};

mod generate;
mod geometry;

pub use geometry::{Bounds, Geometry, PRISM_FACE_RINGS, Point};

/// Explicit fixture classification; never inferred from a surface orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Beam,
    Column,
    Wall,
    Slab,
}

impl Kind {
    pub const ALL: [Self; 4] = [Self::Beam, Self::Column, Self::Wall, Self::Slab];
}

/// Authored, overlapping groups. `Empty` deliberately has no members.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Core,
    Perimeter,
    ZoneA,
    ZoneB,
    LateralSystem,
    Empty,
}

impl Group {
    pub const ALL: [Self; 6] = [
        Self::Core,
        Self::Perimeter,
        Self::ZoneA,
        Self::ZoneB,
        Self::LateralSystem,
        Self::Empty,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Core => "Core",
            Self::Perimeter => "Perimeter",
            Self::ZoneA => "Zone A",
            Self::ZoneB => "Zone B",
            Self::LateralSystem => "Lateral System",
            Self::Empty => "Empty",
        }
    }
}

/// One-based authored floor ID. A vertical member belongs to its upper floor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StoryId(pub u16);

/// Stable within a preset; selection must pair it with `GeometryRevision`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LabId {
    pub kind: Kind,
    pub story: StoryId,
    pub slot: u32,
}

impl fmt::Display for LabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "LAB/{:?}/S{:02}/{:05}",
            self.kind, self.story.0, self.slot
        )
    }
}

/// Public generation formula, no random seed or external input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    TruthTable,
    Thousand,
    TenThousand,
    FiftyThousand,
    HundredThousand,
}

impl Preset {
    pub const ALL: [Self; 5] = [
        Self::TruthTable,
        Self::Thousand,
        Self::TenThousand,
        Self::FiftyThousand,
        Self::HundredThousand,
    ];

    pub fn grid(self) -> Option<(u16, u16)> {
        match self {
            Self::TruthTable => None,
            Self::Thousand => Some((1, 1)),
            Self::TenThousand => Some((5, 6)),
            Self::FiftyThousand => Some((13, 13)),
            Self::HundredThousand => Some((18, 19)),
        }
    }
}

/// Namespaced immutable identity of an authored formula, not a source fingerprint.
/// Any change to geometry/keys/story attribution MUST bump the formula version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeometryRevision {
    pub preset: Preset,
    pub formula_version: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Story {
    pub id: StoryId,
    pub elevation_m: f64,
}

/// Immutable after fixture validation. Groups may overlap across kinds/floors.
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    id: LabId,
    geometry: Geometry,
    groups: Vec<Group>,
}

impl Entity {
    pub fn id(&self) -> LabId {
        self.id
    }
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
    }
    pub fn groups(&self) -> &[Group] {
        &self.groups
    }
}

/// Counts from validated geometry, not GPU allocations or draw calls.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Counts {
    pub by_kind: BTreeMap<Kind, usize>,
    pub by_story: BTreeMap<StoryId, usize>,
    pub by_group: BTreeMap<Group, usize>,
    pub control_points: usize,
    /// Flat-shaded prism face vertices plus shell boundary vertices, no sharing.
    pub reference_mesh_vertices: usize,
    pub triangles: usize,
}

/// Authored ray oracle for future picking tests; no picking implementation here.
#[derive(Clone, Debug, PartialEq)]
pub struct OcclusionOracle {
    pub ray_origin: Point,
    pub ray_direction: Point,
    pub near: LabId,
    pub far: LabId,
    pub near_position: Point,
    pub far_position: Point,
}

/// Validated immutable fixture and sorted membership indexes.
#[derive(Clone, Debug, PartialEq)]
pub struct Fixture {
    revision: GeometryRevision,
    entities: Vec<Entity>,
    stories: Vec<Story>,
    bounds: Bounds,
    counts: Counts,
    story_members: BTreeMap<StoryId, Vec<LabId>>,
    group_members: BTreeMap<Group, Vec<LabId>>,
    occlusion: Option<OcclusionOracle>,
}

impl Fixture {
    /// Generate and validate the complete authored fixture, without I/O.
    ///
    /// # Errors
    /// Returns a typed validation error if the authored formula violates its contract.
    pub fn generate(preset: Preset) -> Result<Self, FixtureError> {
        generate::fixture(preset)
    }
    pub fn revision(&self) -> GeometryRevision {
        self.revision
    }
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }
    pub fn stories(&self) -> &[Story] {
        &self.stories
    }
    pub fn bounds(&self) -> Bounds {
        self.bounds
    }
    pub fn counts(&self) -> &Counts {
        &self.counts
    }
    pub fn story_members(&self) -> &BTreeMap<StoryId, Vec<LabId>> {
        &self.story_members
    }
    pub fn group_members(&self) -> &BTreeMap<Group, Vec<LabId>> {
        &self.group_members
    }
    pub fn occlusion_oracle(&self) -> Option<&OcclusionOracle> {
        self.occlusion.as_ref()
    }
}

/// Unsupported geometry is an error, never silently flattened or filled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixtureError {
    Empty,
    DuplicateId(LabId),
    InvalidStory,
    InvalidGroup(LabId),
    InvalidCoordinate(LabId),
    DegenerateGeometry(LabId),
    UnsupportedOpening(LabId),
    NonPlanarOrConcave(LabId),
    IncorrectNormal(LabId),
    InvalidFrameAxes(LabId),
    KindGeometryMismatch(LabId),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid lab fixture: {self:?}")
    }
}
impl Error for FixtureError {}

#[cfg(test)]
mod tests;
