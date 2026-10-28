//! WB-02 CPU conversion of validated immutable lab fixtures, not a GPU renderer.
//! Reuses fixture control points/topology. No parser, I/O, worker or engineering logic.
use lab_fixture::{
    Bounds, Fixture, Geometry, GeometryRevision, Kind, LabId, PRISM_FACE_RINGS, Point,
};
use sha2::{Digest as _, Sha256};
use std::{collections::BTreeMap, fmt, ops::Range};

/// GPU-compatible logical layout, not an allocated GPU resource.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Logical vertex/index lengths only, not parser/process/allocator memory.
    pub max_buffer_bytes: usize,
    /// Triangle-aligned index subranges in the shared stream, not per-entity buffers.
    pub max_batch_indices: usize,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            max_buffer_bytes: 256 * 1024 * 1024,
            max_batch_indices: 300_000,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityRange {
    pub id: LabId,
    pub indices: Range<u32>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub entities: usize,
    pub vertices: usize,
    pub triangles: usize,
}
/// Two buffer streams and planned draw ranges, never one buffer per entity.
pub struct Mesh {
    revision: GeometryRevision,
    geometry_sha256: [u8; 32],
    origin_m: Point,
    local_bounds: Bounds,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    entities: Vec<EntityRange>,
    batches: Vec<Range<u32>>,
    by_kind: BTreeMap<Kind, Counts>,
}
impl Mesh {
    pub fn revision(&self) -> GeometryRevision {
        self.revision
    }
    /// Buffer identity only; selection still needs lab revision/entity scope.
    pub fn geometry_sha256(&self) -> [u8; 32] {
        self.geometry_sha256
    }
    pub fn origin_m(&self) -> Point {
        self.origin_m
    }
    pub fn local_bounds(&self) -> Bounds {
        self.local_bounds
    }
    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }
    pub fn entities(&self) -> &[EntityRange] {
        &self.entities
    }
    pub fn planned_batches(&self) -> &[Range<u32>] {
        &self.batches
    }
    pub fn by_kind(&self) -> &BTreeMap<Kind, Counts> {
        &self.by_kind
    }
    pub fn buffer_bytes(&self) -> usize {
        self.vertices.len() * size_of::<Vertex>() + self.indices.len() * size_of::<u32>()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    BufferBound,
    InvalidBatchLimit,
    Allocation,
    IndexOverflow,
    InvalidGeometry,
    UnsupportedOpening,
    ManifestMismatch,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fixture mesh conversion: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Convert an already validated authored fixture. No mutable/raw fixture input.
///
/// # Errors
/// Refuses logical size/index limits, invalid batch settings, allocation failure,
/// post-conversion degeneracy/nonfinite geometry or disagreement with the manifest.
pub fn build(fixture: &Fixture, config: Config) -> Result<Mesh, Error> {
    if config.max_batch_indices < 3 || !config.max_batch_indices.is_multiple_of(3) {
        return Err(Error::InvalidBatchLimit);
    }
    let counts = fixture.counts();
    let indices_len = counts
        .triangles
        .checked_mul(3)
        .ok_or(Error::IndexOverflow)?;
    u32::try_from(indices_len).map_err(|_overflow| Error::IndexOverflow)?;
    u32::try_from(counts.reference_mesh_vertices).map_err(|_overflow| Error::IndexOverflow)?;
    let vertex_bytes = counts
        .reference_mesh_vertices
        .checked_mul(size_of::<Vertex>())
        .ok_or(Error::BufferBound)?;
    let index_bytes = indices_len
        .checked_mul(size_of::<u32>())
        .ok_or(Error::BufferBound)?;
    let bytes = vertex_bytes
        .checked_add(index_bytes)
        .ok_or(Error::BufferBound)?;
    if bytes > config.max_buffer_bytes {
        return Err(Error::BufferBound);
    }
    let world = fixture.bounds();
    let origin_m = std::array::from_fn(|i| world.min[i] + (world.max[i] - world.min[i]) * 0.5);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut entities = Vec::new();
    vertices
        .try_reserve_exact(counts.reference_mesh_vertices)
        .map_err(|_allocation| Error::Allocation)?;
    indices
        .try_reserve_exact(indices_len)
        .map_err(|_allocation| Error::Allocation)?;
    entities
        .try_reserve_exact(fixture.entities().len())
        .map_err(|_allocation| Error::Allocation)?;
    let mut by_kind: BTreeMap<_, _> = Kind::ALL
        .into_iter()
        .map(|k| (k, Counts::default()))
        .collect();
    for entity in fixture.entities() {
        let start = u32::try_from(indices.len()).map_err(|_overflow| Error::IndexOverflow)?;
        let first_vertex = vertices.len();
        append(
            entity.geometry(),
            color(entity.id().kind),
            origin_m,
            &mut vertices,
            &mut indices,
        )?;
        let end = u32::try_from(indices.len()).map_err(|_overflow| Error::IndexOverflow)?;
        let entry = by_kind
            .get_mut(&entity.id().kind)
            .ok_or(Error::ManifestMismatch)?;
        entry.entities += 1;
        entry.vertices += vertices.len() - first_vertex;
        entry.triangles += (end - start) as usize / 3;
        entities.push(EntityRange {
            id: entity.id(),
            indices: start..end,
        });
    }
    if vertices.len() != counts.reference_mesh_vertices
        || indices.len() != indices_len
        || by_kind
            .iter()
            .any(|(k, c)| counts.by_kind.get(k).copied().unwrap_or(0) != c.entities)
    {
        return Err(Error::ManifestMismatch);
    }
    let local_bounds = display_bounds(&vertices);
    let geometry_sha256 = geometry_key(origin_m, &vertices, &indices);
    let mut batches = Vec::new();
    batches
        .try_reserve_exact(indices.len().div_ceil(config.max_batch_indices))
        .map_err(|_allocation| Error::Allocation)?;
    for start in (0..indices.len()).step_by(config.max_batch_indices) {
        let end = start
            .saturating_add(config.max_batch_indices)
            .min(indices.len());
        batches.push(
            u32::try_from(start).map_err(|_overflow| Error::IndexOverflow)?
                ..u32::try_from(end).map_err(|_overflow| Error::IndexOverflow)?,
        );
    }
    Ok(Mesh {
        revision: fixture.revision(),
        geometry_sha256,
        origin_m,
        local_bounds,
        vertices,
        indices,
        entities,
        batches,
        by_kind,
    })
}
fn display_bounds(vertices: &[Vertex]) -> Bounds {
    let mut bounds = Bounds {
        min: [f64::INFINITY; 3],
        max: [f64::NEG_INFINITY; 3],
    };
    for vertex in vertices {
        for i in 0..3 {
            let v = f64::from(vertex.position[i]);
            bounds.min[i] = bounds.min[i].min(v);
            bounds.max[i] = bounds.max[i].max(v);
        }
    }
    bounds
}
fn geometry_key(origin_m: Point, vertices: &[Vertex], indices: &[u32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"LAB-FIXTURE-MESH-v1");
    for value in origin_m {
        hash.update(value.to_bits().to_le_bytes());
    }
    hash.update((vertices.len() as u64).to_le_bytes());
    for vertex in vertices {
        for value in vertex
            .position
            .into_iter()
            .chain(vertex.normal)
            .chain(vertex.color)
        {
            hash.update(value.to_bits().to_le_bytes());
        }
    }
    hash.update((indices.len() as u64).to_le_bytes());
    for index in indices {
        hash.update(index.to_le_bytes());
    }
    hash.finalize().into()
}
fn color(kind: Kind) -> [f32; 3] {
    match kind {
        Kind::Beam => [0.20, 0.75, 0.60],
        Kind::Column => [0.22, 0.48, 0.90],
        Kind::Wall => [0.80, 0.62, 0.35],
        Kind::Slab => [0.66, 0.40, 0.20],
    }
}
fn sub(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}
fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: Point, b: Point) -> f64 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}
fn normal(points: &[Point]) -> Result<Point, Error> {
    let n = cross(sub(points[1], points[0]), sub(points[2], points[0]));
    let length = dot(n, n).sqrt();
    if !length.is_finite() || length <= 1.0e-12 {
        return Err(Error::InvalidGeometry);
    }
    Ok(n.map(|v| v / length))
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "Fixture world coordinates are validated; finite and winding checks reject display conversion failures"
)]
fn surface(
    points: &[Point],
    normal: Point,
    color: [f32; 3],
    origin: Point,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
) -> Result<(), Error> {
    if points.len() < 3
        || !points
            .iter()
            .flatten()
            .chain(normal.iter())
            .all(|v| v.is_finite())
        || (dot(normal, normal) - 1.0).abs() > 1.0e-7
        || points
            .iter()
            .any(|p| dot(sub(*p, points[0]), normal).abs() > 1.0e-7)
    {
        return Err(Error::InvalidGeometry);
    }
    let start = u32::try_from(vertices.len()).map_err(|_overflow| Error::IndexOverflow)?;
    for point in points {
        let position = sub(*point, origin).map(|v| v as f32);
        if !position.iter().all(|v| v.is_finite()) {
            return Err(Error::InvalidGeometry);
        }
        vertices.push(Vertex {
            position,
            normal: normal.map(|v| v as f32),
            color,
        });
    }
    for i in 1..points.len() - 1 {
        let offsets = [0, i, i + 1];
        let p = offsets.map(|offset| vertices[start as usize + offset].position.map(f64::from));
        if dot(cross(sub(p[1], p[0]), sub(p[2], p[0])), normal) <= 1.0e-12 {
            return Err(Error::InvalidGeometry);
        }
        for offset in offsets {
            indices.push(
                start
                    .checked_add(u32::try_from(offset).map_err(|_overflow| Error::IndexOverflow)?)
                    .ok_or(Error::IndexOverflow)?,
            );
        }
    }
    Ok(())
}
fn append(
    geometry: &Geometry,
    color: [f32; 3],
    origin: Point,
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
) -> Result<(), Error> {
    match geometry {
        Geometry::Shell {
            boundary,
            normal,
            openings,
        } => {
            if !openings.is_empty() {
                return Err(Error::UnsupportedOpening);
            }
            surface(boundary, *normal, color, origin, vertices, indices)
        }
        Geometry::Prism(_) | Geometry::OrientedPrism { .. } => {
            let corners = geometry.control_points();
            for ring in PRISM_FACE_RINGS {
                let points = ring.map(|i| corners[i]);
                surface(&points, normal(&points)?, color, origin, vertices, indices)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
