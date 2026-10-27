//! REAL-02 first slice: pure, immutable GPU-ready display buffers, not a renderer.
//! Source keys/coordinates come from pinned ext-e2k. All frame solids are proxies;
//! shell surfaces are zero thickness. No engineering calculations or result joins.
use ext_e2k::{
    EntityKey, ForceUnit, FrameKind, LengthUnit, ScenePoint, ShellKind, SourceMap, TempUnit,
};
use lab_fixture::{Bounds, Geometry, PRISM_FACE_RINGS, Point};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeSet, fmt, ops::Range};
const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const CONTRACT_VERSION: u32 = 1;
const EPS: f64 = 1.0e-7;
pub mod public_fixture;
/// Fixed limitations, never inferred model-specific engineering status.
pub const FIDELITY_LIMITATIONS: [&str; 4] = [
    "project_scene_geometry_assumptions_not_certified",
    "proxy_sections_local_axes_offsets_and_insertion_not_applied",
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
    RectangularProxy,
    ZeroThicknessSurface,
    Withheld,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Limitation {
    DegenerateFrame,
    OpeningVoid,
    UnknownOpeningState,
    HostOpeningSubtractionUnavailable,
    InvalidOrConcaveShell,
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
        builder.frame(frame, factor, config.proxy_half_extents_m)?;
    }
    let has_void = scene.shells.iter().any(|s| s.opening == Some(true));
    for shell in &scene.shells {
        builder.shell(shell, factor, has_void)?;
    }
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
        factor: f64,
        half_extents: [f64; 2],
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
        let first = self.index()?;
        let limitation = if let Some(axis) = unit(sub(end, start)) {
            let helper = if axis[2].abs() < 0.9 {
                [0.0, 0.0, 1.0]
            } else {
                [1.0, 0.0, 0.0]
            };
            let y = unit(cross(helper, axis)).ok_or(Error::InvalidGeometry)?;
            let corners = Geometry::OrientedPrism {
                start,
                end,
                transverse_y: y,
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
                DisplayMode::RectangularProxy
            } else {
                DisplayMode::Withheld
            },
            limitation,
            source_centerline_m: Some(ends),
        });

        Ok(())
    }
    fn shell(
        &mut self,
        shell: &ext_e2k::ShellPolygon,
        factor: f64,
        has_void: bool,
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
        let points = shell
            .points
            .iter()
            .map(|p| world(*p, factor).map(|p| local(p, self.origin_m)))
            .collect::<Result<Vec<_>, _>>()?;
        let limitation = if shell.opening == Some(true) {
            Some(Limitation::OpeningVoid)
        } else if has_void {
            Some(Limitation::HostOpeningSubtractionUnavailable)
        } else if shell.opening.is_none() {
            Some(Limitation::UnknownOpeningState)
        } else if let Some(normal) = convex_normal(&points) {
            self.polygon(&points, normal, color(class))?;
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
