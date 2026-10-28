//! Nearest-hit entity picking and the scoped selection it feeds.
//!
//! Along one camera ray, distance order is depth order, so the nearest accepted
//! two-sided hit is the surface a two-sided depth-tested draw shows at that
//! pixel. Withheld entities have no triangles and can never be hit; a caller
//! filter excludes hidden or isolated-away entities.
use crate::{EntityMesh, Revision, Vertex};

/// A ray in the mesh's local display frame (metres, origin-recentred), with the
/// accepted distance window, e.g. the camera's near/far planes along this ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: [f64; 3],
    /// Unit direction.
    pub direction: [f64; 3],
    pub min_distance: f64,
    pub max_distance: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// Index into the mesh entities.
    pub entity: usize,
    pub distance: f64,
    /// Other visible entities hit at the same depth (coplanar overlaps). The
    /// winner is then the lowest entity index, which a GPU depth test does not
    /// guarantee, so a caller must disclose the ambiguity.
    pub coincident: usize,
}
/// Per-entity bounds, computed once per immutable mesh.
#[derive(Clone, Debug, Default)]
pub struct PickIndex {
    bounds: Vec<Option<[[f64; 3]; 2]>>,
}
impl PickIndex {
    pub fn new(vertices: &[Vertex], indices: &[u32], entities: &[EntityMesh]) -> Self {
        let bounds = entities
            .iter()
            .map(|entity| {
                let mut result = None::<[[f64; 3]; 2]>;
                for p in triangles(vertices, indices, entity)?.flatten() {
                    let [lo, hi] = result.get_or_insert([p, p]);
                    for axis in 0..3 {
                        lo[axis] = lo[axis].min(p[axis]);
                        hi[axis] = hi[axis].max(p[axis]);
                    }
                }
                result
            })
            .collect();
        Self { bounds }
    }
    /// Nearest visible hit in the ray's distance window. Never panics on a bad
    /// range: an entity whose range is outside the buffers is unpickable.
    pub fn pick(
        &self,
        vertices: &[Vertex],
        indices: &[u32],
        entities: &[EntityMesh],
        ray: &Ray,
        visible: impl Fn(usize) -> bool,
    ) -> Option<Hit> {
        let mut hits = Vec::new();
        for (index, (entity, bounds)) in entities.iter().zip(&self.bounds).enumerate() {
            let Some(bounds) = bounds else { continue };
            if !visible(index) || !slab(ray, bounds) {
                continue;
            }
            let nearest = triangles(vertices, indices, entity)
                .into_iter()
                .flatten()
                .filter_map(|t| intersect(ray, t))
                .fold(f64::INFINITY, f64::min);
            if nearest.is_finite() {
                hits.push((index, nearest));
            }
        }
        let &(entity, distance) = hits
            .iter()
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))?;
        let tolerance = COINCIDENT_M + distance * 1.0e-6;
        Some(Hit {
            entity,
            distance,
            coincident: hits
                .iter()
                .filter(|(i, d)| *i != entity && (d - distance).abs() <= tolerance)
                .count(),
        })
    }
}
/// Depth band in metres within which two surfaces are treated as coincident.
const COINCIDENT_M: f64 = 1.0e-5;

fn triangles<'a>(
    vertices: &'a [Vertex],
    indices: &'a [u32],
    entity: &EntityMesh,
) -> Option<impl Iterator<Item = [[f64; 3]; 3]> + 'a> {
    let range = indices.get(entity.indices.start as usize..entity.indices.end as usize)?;
    if range.len() % 3 != 0 || range.iter().any(|&i| i as usize >= vertices.len()) {
        return None;
    }
    Some(
        range
            .as_chunks::<3>()
            .0
            .iter()
            .map(|t| t.map(|i| vertices[i as usize].position.map(f64::from))),
    )
}
fn slab(ray: &Ray, [lo, hi]: &[[f64; 3]; 2]) -> bool {
    let (mut near, mut far) = (ray.min_distance, ray.max_distance);
    for axis in 0..3 {
        let (o, d) = (ray.origin[axis], ray.direction[axis]);
        // Pad by the coincidence band so flat boxes (zero-thickness shells) hit.
        let (lo, hi) = (lo[axis] - COINCIDENT_M, hi[axis] + COINCIDENT_M);
        if d.abs() < 1.0e-15 {
            if o < lo || o > hi {
                return false;
            }
        } else {
            let (a, b) = ((lo - o) / d, (hi - o) / d);
            near = near.max(a.min(b));
            far = far.min(a.max(b));
            if near > far {
                return false;
            }
        }
    }
    true
}
/// Two-sided Möller–Trumbore; matches `cull_mode: None`.
fn intersect(ray: &Ray, [a, b, c]: [[f64; 3]; 3]) -> Option<f64> {
    let sub = |p: [f64; 3], q: [f64; 3]| [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
    let cross = |p: [f64; 3], q: [f64; 3]| {
        [
            p[1] * q[2] - p[2] * q[1],
            p[2] * q[0] - p[0] * q[2],
            p[0] * q[1] - p[1] * q[0],
        ]
    };
    let dot = |p: [f64; 3], q: [f64; 3]| p[0] * q[0] + p[1] * q[1] + p[2] * q[2];
    let (e1, e2) = (sub(b, a), sub(c, a));
    let p = cross(ray.direction, e2);
    let det = dot(e1, p);
    if det.abs() < 1.0e-14 {
        return None;
    }
    let s = sub(ray.origin, a);
    let u = dot(s, p) / det;
    let q = cross(s, e1);
    let v = dot(ray.direction, q) / det;
    let t = dot(e2, q) / det;
    let edge = -1.0e-9;
    (u >= edge
        && v >= edge
        && u + v <= 1.0 - edge
        && t >= ray.min_distance
        && t <= ray.max_distance)
        .then_some(t)
}

/// Selection scope: the mesh revision plus a caller load generation, so a
/// reload of identical bytes as another version still expires the selection.
pub type Scope = (Revision, u64);

/// Individual entity selection, valid only for the scope it was made in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    scope: Option<Scope>,
    entities: Vec<usize>,
}
impl Selection {
    /// Click semantics: a hit replaces, `toggle` adds/removes one entity, an
    /// empty-space click clears unless toggling. A different scope starts over.
    pub fn click(&mut self, scope: Scope, hit: Option<usize>, toggle: bool) {
        if self.scope != Some(scope) {
            *self = Self {
                scope: Some(scope),
                entities: Vec::new(),
            };
        }
        match (hit, toggle) {
            (Some(entity), true) => match self.entities.binary_search(&entity) {
                Ok(at) => {
                    self.entities.remove(at);
                }
                Err(at) => self.entities.insert(at, entity),
            },
            (Some(entity), false) => self.entities = vec![entity],
            (None, false) => self.entities.clear(),
            (None, true) => {}
        }
    }
    /// Selected entity indices in `scope`; empty for any other scope.
    pub fn entities(&self, scope: Scope) -> &[usize] {
        if self.scope == Some(scope) {
            &self.entities
        } else {
            &[]
        }
    }
    /// Drop entities that became hidden or filtered out.
    pub fn retain_visible(&mut self, visible: impl Fn(usize) -> bool) {
        self.entities.retain(|&entity| visible(entity));
    }
}
