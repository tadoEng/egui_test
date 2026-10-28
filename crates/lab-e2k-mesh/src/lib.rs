//! REAL-02 first slice: pure, immutable GPU-ready display buffers, not a renderer.
//! Source keys/coordinates come from pinned ext-e2k. Only inline rectangular
//! frame sections use source dimensions; every other frame body is a labelled
//! approximation. Shell surfaces are zero thickness. No engineering
//! calculations or result joins.
use ext_e2k::{
    EntityKey, ForceUnit, FrameKind, LengthUnit, ScenePoint, ShellInsertionPoint, ShellKind,
    SourceMap, TempUnit,
};
use lab_fixture::{Bounds, Geometry, PRISM_FACE_RINGS, Point};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeSet, fmt, ops::Range};
const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const CONTRACT_VERSION: u32 = 1;
const EPS: f64 = 1.0e-7;
mod openings;
pub mod pick;
pub mod public_fixture;
/// Fixed limitations, never inferred model-specific engineering status.
pub const FIDELITY_LIMITATIONS: [&str; 7] = [
    "project_scene_geometry_assumptions_not_certified",
    "frame_fidelity_is_a_native_mirror_of_ext_api_not_the_authority",
    "frame_local_axis_rotation_unparsed_default_csi_axes_assumed",
    "frame_cardinal_insertion_and_end_offsets_not_applied",
    "shell_thickness_and_insertion_not_drawn",
    "shell_void_subtraction_unavailable",
    "no_group_or_member_result_inference",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    pub proxy_half_extents_m: [f64; 2],
    pub max_entities: usize,
    pub max_buffer_bytes: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            proxy_half_extents_m: [0.05; 2],
            max_entities: 100_000,
            max_buffer_bytes: 256 * 1024 * 1024,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision {
    pub source_sha256: [u8; 32],
    pub geometry_sha256: [u8; 32],
    pub contract_version: u32,
    pub proxy_bits: [u64; 2],
}
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "render-layout", derive(bytemuck::Pod, bytemuck::Zeroable))]
#[repr(C)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceClass {
    Column,
    Beam,
    Brace,
    OtherFrame,
    FloorShell,
    PanelShell,
    OtherShell,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    /// A swept rectangle; whether its size is the source section is
    /// [`FrameSource::fidelity`], never implied by this mode.
    RectangularFrame,
    ZeroThicknessSurface,
    Withheld,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limitation {
    DegenerateFrame,
    OpeningVoid,
    /// The pinned `ext-api` host rule: a declared opening lies within this
    /// coplanar same-story shell, and the projection does not subtract it.
    HostOpeningSubtractionUnavailable,
    /// Not a host by the `ext-api` rule, but coplanar with and sharing area with
    /// a declared opening (or touching one that has no plane). Withheld by this
    /// viewer rather than painted over the void; the backend draws it.
    OverlapsUnsubtractedOpening,
    InvalidOrConcaveShell,
}
/// The `ext-api` `SceneGeometryFidelity` tiers a frame can take.
///
/// Mirrors `model_inspect/fidelity.rs::classify_frame` at 9eb1c879. The backend
/// classification is the authority; this mirror exists only until the native
/// host can consume it (#18 holds `ext-api`), and must never be more optimistic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeometryFidelity {
    Resolved,
    Approximate,
    Symbolic,
}
/// `ext-api` `SceneFidelityReason` names for frames, plus one native-only
/// reason. Declaration order matches the backend's, which publishes reasons
/// sorted in that order and deduplicated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrameReason {
    SectionNotAssigned,
    SectionDefinitionMissing,
    SectionDimensionsMissing,
    SectionDimensionsInLibrary,
    RectangularProfileEnvelope,
    ProfileShapeUnknown,
    LocalAxisRotationUnavailable,
    CardinalPointOffsetIgnored,
    FrameJointOffsetsIgnored,
    SectionMirrorIgnored,
    /// Native only: where the backend's viewer draws a bare centreline, this
    /// one draws the configured fixed proxy so the member stays pickable.
    FixedDisplayProxy,
}
/// Source frame attributes for an inspector, in metres where dimensional.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameSource {
    pub section: Option<String>,
    pub material: Option<String>,
    pub shape: Option<String>,
    pub depth_m: Option<f64>,
    pub width_m: Option<f64>,
    pub cardinal_point: Option<i32>,
    pub fidelity: GeometryFidelity,
    pub reasons: Vec<FrameReason>,
}
impl FrameSource {
    /// Whether the body is swept from the source depth × width.
    pub fn swept(&self) -> bool {
        self.fidelity != GeometryFidelity::Symbolic
    }
}
/// Source shell attributes for an inspector, in metres where dimensional.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellSource {
    pub section: Option<String>,
    pub material: Option<String>,
    pub thickness_m: Option<f64>,
    pub insertion_point: Option<String>,
    /// The source `OPENING` flag. `None` means the row said nothing; like the
    /// pinned `ext-api`, only an explicit `true` makes a void, so such a shell
    /// is drawn and this field is what discloses that nothing was declared.
    pub opening: Option<bool>,
    /// Surfaces are zero thickness at the connectivity plane; always false today.
    pub thickness_drawn: bool,
}
#[derive(Clone, Debug)]
pub struct EntityMesh {
    pub key: EntityKey,
    pub story: String,
    pub class: SourceClass,
    pub indices: Range<u32>,
    pub mode: DisplayMode,
    pub limitation: Option<Limitation>,
    pub source_centerline_m: Option<[Point; 2]>,
    pub frame: Option<FrameSource>,
    pub shell: Option<ShellSource>,
}
#[derive(Clone, Debug)]
pub struct Story {
    pub name: String,
    pub elevation_m: f64,
}
pub struct Mesh {
    revision: Revision,
    origin_m: Point,
    bounds: Bounds,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    entities: Vec<EntityMesh>,
    stories: Vec<Story>,
    source_map: SourceMap,
    projection_skips: Vec<ext_e2k::SkippedEntity>,
}
/// Ownership transfer for a renderer; the original arrays are moved, not copied.
pub struct RenderParts {
    pub revision: Revision,
    pub origin_m: Point,
    pub bounds: Bounds,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    /// Source identity/story/index ranges are moved with the arrays. They are
    /// display attribution, never inferred member engineering results.
    pub entities: Vec<EntityMesh>,
}
impl Mesh {
    pub fn into_render_parts(self) -> RenderParts {
        RenderParts {
            revision: self.revision,
            origin_m: self.origin_m,
            bounds: self.bounds,
            vertices: self.vertices,
            indices: self.indices,
            entities: self.entities,
        }
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
    pub fn origin_m(&self) -> Point {
        self.origin_m
    }
    pub fn local_bounds_m(&self) -> Bounds {
        self.bounds
    }
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
    pub fn entities(&self) -> &[EntityMesh] {
        &self.entities
    }
    pub fn stories(&self) -> &[Story] {
        &self.stories
    }
    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }
    pub fn projection_skips(&self) -> &[ext_e2k::SkippedEntity] {
        &self.projection_skips
    }
    /// CPU vertex/index storage only; no GPU resource or draw-call observation.
    pub fn buffer_bytes(&self) -> usize {
        self.vertices.len() * size_of::<Vertex>() + self.indices.len() * size_of::<u32>()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    SourceBound,
    InvalidUnits,
    UnsupportedVersion,
    Parse,
    InvalidGeometry,
    DuplicateIdentity,
    Budget,
    NoGeometry,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E2K display mesh refused: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Parse only bounded caller-owned bytes. No filesystem, process, GPU, network,
/// logging or export. A digest identifies source bytes, not analysis freshness.
///
/// # Errors
/// Refuses unsupported units/version, malformed or empty geometry, duplicate keys
/// and configured source/mesh bounds. Does not certify engineering fidelity.
pub fn build(source: &[u8], config: Config) -> Result<Mesh, Error> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(Error::SourceBound);
    }
    if config
        .proxy_half_extents_m
        .iter()
        .any(|v| !v.is_finite() || *v <= EPS || *v > 10.0)
        || config.max_entities == 0
        || config.max_entities > 200_000
        || config.max_buffer_bytes == 0
        || config.max_buffer_bytes > 256 * 1024 * 1024
    {
        return Err(Error::Budget);
    }
    let text = std::str::from_utf8(source).map_err(|_private_error| Error::Parse)?;
    let length = declared_length(text)?;
    let factor = match length {
        LengthUnit::Ft => 0.3048,
        LengthUnit::In => 0.0254,
        LengthUnit::M => 1.0,
        LengthUnit::Cm => 0.01,
        LengthUnit::Mm => 0.001,
    };
    let (model, source_map) =
        ext_e2k::parse_str_with_sources(text).map_err(|_private_error| Error::Parse)?;
    let major = model
        .program
        .version
        .as_ref()
        .and_then(|s| s.as_str().split('.').next())
        .and_then(|s| s.parse::<u32>().ok());
    if !major.is_some_and(|v| (17..=23).contains(&v)) {
        return Err(Error::UnsupportedVersion);
    }
    let scene = ext_e2k::project_scene(&model);
    if scene
        .frames
        .len()
        .saturating_add(scene.shells.len())
        .saturating_add(scene.skipped.len())
        > config.max_entities
    {
        return Err(Error::Budget);
    }
    let points: Vec<_> = scene
        .frames
        .iter()
        .flat_map(|f| [f.i, f.j])
        .chain(scene.shells.iter().flat_map(|s| s.points.iter().copied()))
        .map(|p| world(p, factor))
        .collect::<Result<_, _>>()?;
    let bounds = point_bounds(&points).ok_or(Error::NoGeometry)?;
    let origin_m: Point = std::array::from_fn(|i| bounds.min[i].midpoint(bounds.max[i]));
    if (0..3).any(|i| bounds.max[i] - bounds.min[i] > 1.0e6) {
        return Err(Error::InvalidGeometry);
    }
    let mut builder = Builder {
        vertices: Vec::new(),
        indices: Vec::new(),
        limit: config.max_buffer_bytes,
        entities: Vec::new(),
        seen: BTreeSet::new(),
        origin_m,
    };
    for frame in &scene.frames {
        let section = frame
            .section
            .as_ref()
            .and_then(|section| model.find_frame_section(section.as_str()));
        builder.frame(frame, section, factor, config.proxy_half_extents_m)?;
    }
    builder.shells(&scene.shells, factor)?;
    if builder.indices.is_empty() {
        return Err(Error::NoGeometry);
    }
    let stories = source_stories(&model, factor)?;
    let local_points: Vec<_> = builder
        .vertices
        .iter()
        .map(|v| v.position.map(f64::from))
        .collect();
    Ok(Mesh {
        revision: Revision {
            source_sha256: Sha256::digest(source).into(),
            geometry_sha256: geometry_digest(origin_m, &builder.vertices, &builder.indices),
            contract_version: CONTRACT_VERSION,
            proxy_bits: config.proxy_half_extents_m.map(f64::to_bits),
        },
        origin_m,
        bounds: point_bounds(&local_points).ok_or(Error::NoGeometry)?,
        vertices: builder.vertices,
        indices: builder.indices,
        entities: builder.entities,
        stories,
        source_map,
        projection_skips: scene.skipped,
    })
}
fn geometry_digest(origin: Point, vertices: &[Vertex], indices: &[u32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(CONTRACT_VERSION.to_le_bytes());
    for value in origin {
        hash.update(value.to_le_bytes());
    }
    hash.update(
        u64::try_from(vertices.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for vertex in vertices {
        let values = vertex
            .position
            .into_iter()
            .chain(vertex.normal)
            .chain(vertex.color);
        let mut bytes = [0u8; 36];
        for (target, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
            *target = value.to_le_bytes();
        }
        hash.update(bytes);
    }
    hash.update(
        u64::try_from(indices.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for index in indices {
        hash.update(index.to_le_bytes());
    }
    hash.finalize().into()
}
fn source_stories(model: &ext_e2k::BuildingModel, factor: f64) -> Result<Vec<Story>, Error> {
    let source_stories = model.story_stack();
    if source_stories.is_empty()
        || source_stories.len() != model.stories.len()
        || source_stories
            .iter()
            .map(|s| s.name.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != source_stories.len()
    {
        return Err(Error::InvalidGeometry);
    }
    let stories = source_stories
        .iter()
        .map(|s| {
            let elevation = s.elevation.value() * factor;
            if !elevation.is_finite() {
                return Err(Error::InvalidGeometry);
            }
            Ok(Story {
                name: s.name.as_str().into(),
                elevation_m: elevation,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(stories)
}
struct Builder {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    limit: usize,
    entities: Vec<EntityMesh>,
    seen: BTreeSet<String>,
    origin_m: Point,
}
impl Builder {
    fn frame(
        &mut self,
        frame: &ext_e2k::FrameSegment,
        section: Option<&ext_e2k::FrameSectionDef>,
        factor: f64,
        proxy_half_extents: [f64; 2],
    ) -> Result<(), Error> {
        if !self.seen.insert(frame.key.as_stable_str()) {
            return Err(Error::DuplicateIdentity);
        }
        let ends = [world(frame.i, factor)?, world(frame.j, factor)?];
        let start = local(ends[0], self.origin_m);
        let end = local(ends[1], self.origin_m);
        let class = match frame.kind {
            FrameKind::Column => SourceClass::Column,
            FrameKind::Beam => SourceClass::Beam,
            FrameKind::Brace => SourceClass::Brace,
            _ => SourceClass::OtherFrame,
        };
        let source = frame_source(frame, section, factor);
        let half_extents = if source.swept() {
            [source.depth_m, source.width_m].map(|v| v.unwrap_or(0.0) / 2.0)
        } else {
            proxy_half_extents
        };
        let first = self.index()?;
        let limitation = if let Some(axis) = unit(sub(end, start)) {
            let corners = Geometry::OrientedPrism {
                start,
                end,
                transverse_y: default_local_two(axis),
                half_extents,
            }
            .control_points();
            for ring in PRISM_FACE_RINGS {
                let face = ring.map(|i| corners[i]);
                let n = unit(cross(sub(face[1], face[0]), sub(face[2], face[0])))
                    .ok_or(Error::InvalidGeometry)?;
                self.polygon(&face, n, color(class))?;
            }
            None
        } else {
            Some(Limitation::DegenerateFrame)
        };
        self.entities.push(EntityMesh {
            key: frame.key.clone(),
            story: frame.story.as_str().into(),
            class,
            indices: first..self.index()?,
            mode: if limitation.is_none() {
                DisplayMode::RectangularFrame
            } else {
                DisplayMode::Withheld
            },
            limitation,
            source_centerline_m: Some(ends),
            frame: Some(source),
            shell: None,
        });

        Ok(())
    }
    /// Opening attribution needs every ring first, so shells are built together.
    fn shells(&mut self, shells: &[ext_e2k::ShellPolygon], factor: f64) -> Result<(), Error> {
        let rings = shells
            .iter()
            .map(|shell| {
                shell
                    .points
                    .iter()
                    .map(|p| world(*p, factor).map(|p| local(p, self.origin_m)))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let openings: Vec<_> = shells
            .iter()
            .zip(&rings)
            .filter(|(shell, _)| shell.opening == Some(true))
            .map(|(_, ring)| openings::Opening::new(ring))
            .collect();
        let hosts = openings::authoritative_hosts(shells);
        for (index, (shell, ring)) in shells.iter().zip(&rings).enumerate() {
            let opening = if shell.opening == Some(true) {
                None
            } else if hosts.contains(&index) {
                Some(Limitation::HostOpeningSubtractionUnavailable)
            } else if openings.iter().any(|opening| opening.withholds(ring)) {
                Some(Limitation::OverlapsUnsubtractedOpening)
            } else {
                None
            };
            self.shell(shell, ring, factor, opening)?;
        }
        Ok(())
    }
    fn shell(
        &mut self,
        shell: &ext_e2k::ShellPolygon,
        points: &[Point],
        factor: f64,
        opening_limitation: Option<Limitation>,
    ) -> Result<(), Error> {
        if !self.seen.insert(shell.key.as_stable_str()) {
            return Err(Error::DuplicateIdentity);
        }
        let class = match shell.kind {
            ShellKind::Floor => SourceClass::FloorShell,
            ShellKind::Panel => SourceClass::PanelShell,
            _ => SourceClass::OtherShell,
        };
        let first = self.index()?;
        let limitation = if shell.opening == Some(true) {
            Some(Limitation::OpeningVoid)
        } else if opening_limitation.is_some() {
            opening_limitation
        } else if let Some(normal) = convex_normal(points) {
            self.polygon(points, normal, color(class))?;
            None
        } else {
            Some(Limitation::InvalidOrConcaveShell)
        };
        self.entities.push(EntityMesh {
            key: shell.key.clone(),
            story: shell.story.as_str().into(),
            class,
            indices: first..self.index()?,
            mode: if limitation.is_none() {
                DisplayMode::ZeroThicknessSurface
            } else {
                DisplayMode::Withheld
            },
            limitation,
            source_centerline_m: None,
            frame: None,
            shell: Some(ShellSource {
                section: shell.section.as_ref().map(|s| s.as_str().into()),
                material: shell.material.as_ref().map(|m| m.as_str().into()),
                thickness_m: shell.thickness.map(|t| t * factor),
                insertion_point: shell.insertion_point.as_ref().map(|p| match p {
                    ShellInsertionPoint::Top => "TOP".into(),
                    ShellInsertionPoint::Middle => "MIDDLE".into(),
                    ShellInsertionPoint::Bottom => "BOTTOM".into(),
                    ShellInsertionPoint::Other(value) => value.as_str().into(),
                }),
                opening: shell.opening,
                thickness_drawn: false,
            }),
        });

        Ok(())
    }
    fn index(&self) -> Result<u32, Error> {
        u32::try_from(self.indices.len()).map_err(|_private_error| Error::Budget)
    }
    fn polygon(&mut self, points: &[Point], normal: Point, color: [f32; 3]) -> Result<(), Error> {
        let count = points
            .len()
            .checked_sub(2)
            .ok_or(Error::InvalidGeometry)?
            .checked_mul(3)
            .ok_or(Error::Budget)?;
        let vertices = self
            .vertices
            .len()
            .checked_add(points.len())
            .ok_or(Error::Budget)?;
        let indices = self.indices.len().checked_add(count).ok_or(Error::Budget)?;
        let bytes = vertices
            .checked_mul(size_of::<Vertex>())
            .and_then(|v| {
                let i = indices.checked_mul(4)?;
                v.checked_add(i)
            })
            .ok_or(Error::Budget)?;
        if bytes > self.limit || vertices > u32::MAX as usize || indices > u32::MAX as usize {
            return Err(Error::Budget);
        }
        self.vertices
            .try_reserve(points.len())
            .map_err(|_allocation| Error::Budget)?;
        self.indices
            .try_reserve(count)
            .map_err(|_allocation| Error::Budget)?;
        let start = u32::try_from(self.vertices.len()).map_err(|_private_error| Error::Budget)?;
        for point in points {
            self.vertices.push(Vertex {
                position: to_f32(*point)?,
                normal: to_f32(normal)?,
                color,
            });
        }
        for i in 1..points.len() - 1 {
            let i = u32::try_from(i).map_err(|_private_error| Error::Budget)?;
            self.indices
                .extend_from_slice(&[start, start + i, start + i + 1]);
        }
        Ok(())
    }
}
/// Largest source section dimension accepted as drawable, in metres.
const MAX_SECTION_M: f64 = 20.0;
/// `ext-api` `RECTANGULAR_SHAPES` and `DOUBLY_SYMMETRIC_SHAPES` at 9eb1c879.
const RECTANGULAR_SHAPES: [&str; 2] = ["concrete rectangular", "rectangular"];
const DOUBLY_SYMMETRIC_SHAPES: [&str; 1] = ["concrete rectangular"];
/// Mirrors `ext-api` `classify_frame`; see [`GeometryFidelity`].
fn frame_source(
    frame: &ext_e2k::FrameSegment,
    section: Option<&ext_e2k::FrameSectionDef>,
    factor: f64,
) -> FrameSource {
    let dimension = |v: Option<f64>| {
        v.map(|v| v * factor)
            .filter(|v| v.is_finite() && *v > EPS && *v <= MAX_SECTION_M)
    };
    let (depth_m, width_m) = (dimension(frame.depth), dimension(frame.width));
    let shape = section
        .and_then(|s| s.shape.as_ref())
        .map(|s| s.as_str().trim());
    let normalized = shape.map(str::to_ascii_lowercase);
    let symmetric = normalized
        .as_deref()
        .is_some_and(|s| DOUBLY_SYMMETRIC_SHAPES.contains(&s));
    let mut reasons = Vec::new();
    // A box is swept only when BOTH dimensions resolved, as in the backend; an
    // out-of-range source value is treated as unresolved, never drawn.
    let swept = depth_m.is_some() && width_m.is_some();
    let mut fidelity = if swept {
        reasons.push(FrameReason::LocalAxisRotationUnavailable);
        match normalized.as_deref() {
            Some(s) if RECTANGULAR_SHAPES.contains(&s) => GeometryFidelity::Resolved,
            Some(_) => {
                reasons.push(FrameReason::RectangularProfileEnvelope);
                GeometryFidelity::Approximate
            }
            None => {
                reasons.push(FrameReason::ProfileShapeUnknown);
                GeometryFidelity::Approximate
            }
        }
    } else {
        reasons.push(match (&frame.section, section) {
            (None, _) => FrameReason::SectionNotAssigned,
            (Some(_), None) => FrameReason::SectionDefinitionMissing,
            (Some(_), Some(d)) if d.section_file.is_some() => {
                FrameReason::SectionDimensionsInLibrary
            }
            (Some(_), Some(_)) => FrameReason::SectionDimensionsMissing,
        });
        reasons.push(FrameReason::FixedDisplayProxy);
        GeometryFidelity::Symbolic
    };
    // CSI cardinal 10 (or the absent default) is the centroid; 5 and 11 are
    // only where a proven doubly symmetric family makes them coincide.
    let displaced = match frame.cardinal_point {
        None | Some(10) => false,
        Some(5 | 11) => !symmetric,
        Some(_) => true,
    };
    let mut degrade = |reason| {
        reasons.push(reason);
        fidelity = fidelity.max(GeometryFidelity::Approximate);
    };
    if swept && displaced {
        degrade(FrameReason::CardinalPointOffsetIgnored);
    }
    if frame.insertion.is_displaced() {
        degrade(FrameReason::FrameJointOffsetsIgnored);
    }
    if swept && frame.insertion.is_mirrored() && !symmetric {
        degrade(FrameReason::SectionMirrorIgnored);
    }
    reasons.sort_unstable();
    reasons.dedup();
    FrameSource {
        section: frame.section.as_ref().map(|s| s.as_str().into()),
        material: frame.material.as_ref().map(|m| m.as_str().into()),
        shape: shape.map(Into::into),
        depth_m,
        width_m,
        cardinal_point: frame.cardinal_point,
        fidelity,
        reasons,
    }
}
/// CSI default frame local 2 axis for a unit local 1 axis: global +X for a
/// vertical member (sine of the angle to Z below 1e-3), otherwise the upward
/// direction in the vertical plane containing local 1. Local 3 is 1×2. Section
/// rotation (`ANG`) is not parsed by the pinned source, so it is never applied.
fn default_local_two(axis: Point) -> Point {
    if axis[0].hypot(axis[1]) < 1.0e-3 {
        return [1.0, 0.0, 0.0];
    }
    unit([
        -axis[2] * axis[0],
        -axis[2] * axis[1],
        1.0 - axis[2] * axis[2],
    ])
    .unwrap_or([0.0, 0.0, 1.0])
}
fn point_bounds(points: &[Point]) -> Option<Bounds> {
    let first = *points.first()?;
    let mut result = Bounds {
        min: first,
        max: first,
    };
    for p in points {
        for (i, v) in p.iter().enumerate() {
            result.min[i] = result.min[i].min(*v);
            result.max[i] = result.max[i].max(*v);
        }
    }
    Some(result)
}
fn world(p: ScenePoint, factor: f64) -> Result<Point, Error> {
    let p = [p.x * factor, p.y * factor, p.z * factor];
    if p.iter().all(|v| v.is_finite() && v.abs() <= 1.0e12) {
        Ok(p)
    } else {
        Err(Error::InvalidGeometry)
    }
}
fn local(p: Point, origin: Point) -> Point {
    sub(p, origin)
}
fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: Point, b: Point) -> f64 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(p: Point) -> Option<Point> {
    let length = dot(p, p).sqrt();
    (length > EPS && length.is_finite()).then(|| p.map(|v| v / length))
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "Finite normalized display coordinates only; authoritative world origin/centrelines remain f64"
)]
fn to_f32(p: Point) -> Result<[f32; 3], Error> {
    if !p.iter().all(|v| v.is_finite() && v.abs() <= 1.0e6) {
        return Err(Error::InvalidGeometry);
    }
    Ok(p.map(|v| v as f32))
}
fn convex_normal(points: &[Point]) -> Option<Point> {
    if !(3..=128).contains(&points.len()) {
        return None;
    }
    let normal = unit(cross(sub(points[1], points[0]), sub(points[2], points[0])))?;
    for i in 0..points.len() {
        let edge = sub(points[(i + 1) % points.len()], points[i]);
        let next = sub(
            points[(i + 2) % points.len()],
            points[(i + 1) % points.len()],
        );
        if dot(cross(edge, next), normal) <= EPS
            || dot(sub(points[i], points[0]), normal).abs() > EPS
        {
            return None;
        }
        for point in points {
            if dot(cross(edge, sub(*point, points[i])), normal) < -EPS {
                return None;
            }
        }
    }
    Some(normal)
}
fn color(kind: SourceClass) -> [f32; 3] {
    match kind {
        SourceClass::Column => [0.25, 0.52, 0.8],
        SourceClass::Beam => [0.25, 0.7, 0.6],
        SourceClass::Brace => [0.75, 0.55, 0.3],
        SourceClass::FloorShell => [0.6, 0.45, 0.3],
        SourceClass::PanelShell => [0.55, 0.65, 0.75],
        _ => [0.6; 3],
    }
}
fn declared_length(text: &str) -> Result<LengthUnit, Error> {
    // Reject only: upstream sections.rs retains defaults for unknown unit tokens.
    // Geometry/sections/connectivity parsing remains solely in ext-e2k.
    let mut controls = false;
    let mut prior = None;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.starts_with('$') {
            controls = line.eq_ignore_ascii_case("$ CONTROLS");
        } else if controls
            && line
                .split_whitespace()
                .next()
                .is_some_and(|s| s.eq_ignore_ascii_case("UNITS"))
        {
            let parts: Vec<_> = line.split('"').skip(1).step_by(2).collect();
            let units = (
                parts
                    .first()
                    .and_then(|s| ForceUnit::from_e2k(s))
                    .ok_or(Error::InvalidUnits)?,
                parts
                    .get(1)
                    .and_then(|s| LengthUnit::from_e2k(s))
                    .ok_or(Error::InvalidUnits)?,
                parts
                    .get(2)
                    .and_then(|s| TempUnit::from_e2k(s))
                    .ok_or(Error::InvalidUnits)?,
            );
            if prior.is_some_and(|v| v != units) {
                return Err(Error::InvalidUnits);
            }
            prior = Some(units);
        }
    }
    prior
        .map(|(_, length, _)| length)
        .ok_or(Error::InvalidUnits)
}
#[cfg(test)]
mod tests;
