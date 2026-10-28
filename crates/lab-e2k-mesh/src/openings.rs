//! Opening→host attribution. The pinned projection binds no void to a host and
//! subtracts nothing, so two questions are answered separately:
//!
//! * [`authoritative_hosts`] mirrors the pinned `ext-api` rule
//!   (`model_inspect/shell_openings.rs` at 9eb1c879, unchanged at production
//!   HEAD): same story, both rings planar, coincident planes, and every opening
//!   vertex inside or on the host ring, with the same relative tolerance in
//!   source units. A partial overlap is deliberately not a host there.
//! * [`Opening::withholds`] is this viewer's own conservative band: a shell that
//!   is coplanar with, and shares interior area with, an opening (or whose
//!   bounds touch an opening without a plane) is not drawn as solid material,
//!   because doing so would paint over a declared void.
use super::{EPS, Point, cross, dot, sub, unit};
use ext_e2k::{ScenePoint, ShellPolygon};

/// Distance from the opening plane still treated as the same plane, in metres.
const PLANE_TOLERANCE_M: f64 = 1.0e-3;
/// Boundary band in metres: touching an edge is not sharing material.
const EDGE_TOLERANCE_M: f64 = 1.0e-6;

pub(super) struct Opening {
    points: Vec<Point>,
    normal: Option<Point>,
    bounds: [Point; 2],
}
impl Opening {
    pub(super) fn new(points: &[Point]) -> Self {
        Self {
            points: points.to_vec(),
            normal: newell_normal(points),
            bounds: bounds(points),
        }
    }
    /// Whether `host` may contain material this opening removes.
    pub(super) fn withholds(&self, host: &[Point]) -> bool {
        let host_bounds = bounds(host);
        if host.is_empty()
            || self.points.is_empty()
            || (0..3).any(|i| {
                host_bounds[0][i] > self.bounds[1][i] + PLANE_TOLERANCE_M
                    || self.bounds[0][i] > host_bounds[1][i] + PLANE_TOLERANCE_M
            })
        {
            return false;
        }
        let Some(normal) = self.normal else {
            return true;
        };
        // A host without a plane is withheld by its own validity check instead.
        let Some(host_normal) = newell_normal(host) else {
            return false;
        };
        if dot(cross(normal, host_normal), cross(normal, host_normal)).sqrt() > 1.0e-6
            || host
                .iter()
                .any(|p| dot(sub(*p, self.points[0]), normal).abs() > PLANE_TOLERANCE_M)
        {
            return false;
        }
        let helper = if normal[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let Some(u) = unit(cross(normal, helper)) else {
            return true;
        };
        let v = cross(normal, u);
        let flat = |points: &[Point]| -> Vec<[f64; 2]> {
            points.iter().map(|p| [dot(*p, u), dot(*p, v)]).collect()
        };
        overlaps(&flat(&self.points), &flat(host))
    }
}

fn bounds(points: &[Point]) -> [Point; 2] {
    let mut result = [[f64::INFINITY; 3], [f64::NEG_INFINITY; 3]];
    for p in points {
        for i in 0..3 {
            result[0][i] = result[0][i].min(p[i]);
            result[1][i] = result[1][i].max(p[i]);
        }
    }
    result
}
/// Newell's method: a unit normal for convex and concave rings alike.
fn newell_normal(points: &[Point]) -> Option<Point> {
    if points.len() < 3 {
        return None;
    }
    let mut n = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let area_scale = points
        .iter()
        .map(|p| dot(sub(*p, points[0]), sub(*p, points[0])))
        .fold(0.0, f64::max);
    (dot(n, n).sqrt() > EPS * area_scale.max(1.0))
        .then(|| unit(n))
        .flatten()
}
fn overlaps(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    let edges = |p: &[[f64; 2]]| -> Vec<([f64; 2], [f64; 2])> {
        (0..p.len()).map(|i| (p[i], p[(i + 1) % p.len()])).collect()
    };
    let (ea, eb) = (edges(a), edges(b));
    ea.iter()
        .any(|&(p, q)| eb.iter().any(|&(r, s)| properly_cross(p, q, r, s)))
        || samples(a).iter().any(|p| strictly_inside(*p, b))
        || samples(b).iter().any(|p| strictly_inside(*p, a))
}
/// Vertices, edge midpoints and the vertex centroid: enough to catch overlaps
/// whose vertices all lie on the other boundary, such as identical rings.
fn samples(p: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let n = p.len() as f64;
    let centroid = p
        .iter()
        .fold([0.0; 2], |c, q| [c[0] + q[0] / n, c[1] + q[1] / n]);
    p.iter()
        .copied()
        .chain((0..p.len()).map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            [a[0].midpoint(b[0]), a[1].midpoint(b[1])]
        }))
        .chain([centroid])
        .collect()
}
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
fn length(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}
fn side(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> i8 {
    let o = orient(a, b, c);
    if o.abs() <= EDGE_TOLERANCE_M * length(a, b) {
        0
    } else if o > 0.0 {
        1
    } else {
        -1
    }
}
fn properly_cross(p: [f64; 2], q: [f64; 2], r: [f64; 2], s: [f64; 2]) -> bool {
    side(p, q, r) * side(p, q, s) < 0 && side(r, s, p) * side(r, s, q) < 0
}
fn distance_to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    length(p, [a[0] + t * ab[0], a[1] + t * ab[1]])
}
/// Even-odd containment that treats the boundary band as outside.
fn strictly_inside(p: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if distance_to_segment(p, a, b) <= EDGE_TOLERANCE_M {
            return false;
        }
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
        {
            inside = !inside;
        }
    }
    inside
}

/// `ext-api`'s relative tolerance, scaled by each ring's own extent.
const RELATIVE_EPSILON: f64 = 1e-9;

/// Indices of shells the pinned `ext-api` rule names as hosting an opening the
/// projection did not subtract. Source units, exactly as the backend runs it.
pub(super) fn authoritative_hosts(shells: &[ShellPolygon]) -> std::collections::BTreeSet<usize> {
    let rings: Vec<_> = shells.iter().map(|s| PlanarRing::of(&s.points)).collect();
    let mut hosts = std::collections::BTreeSet::new();
    for (index, (candidate, host)) in shells.iter().zip(&rings).enumerate() {
        let Some(host) = host.as_ref().filter(|_| candidate.opening != Some(true)) else {
            continue;
        };
        if shells.iter().zip(&rings).any(|(opening, ring)| {
            opening.opening == Some(true)
                && opening.story == candidate.story
                && ring.as_ref().is_some_and(|ring| host.contains(ring))
        }) {
            hosts.insert(index);
        }
    }
    hosts
}
struct PlanarRing {
    normal: Point,
    offset: f64,
    drop_axis: usize,
    points: Vec<Point>,
    projected: Vec<[f64; 2]>,
    tolerance: f64,
}
impl PlanarRing {
    fn of(points: &[ScenePoint]) -> Option<Self> {
        let points: Vec<Point> = points.iter().map(|p| [p.x, p.y, p.z]).collect();
        if points.len() < 3 || !points.iter().flatten().all(|v| v.is_finite()) {
            return None;
        }
        let [lo, hi] = bounds(&points);
        let scale = (0..3).map(|i| hi[i] - lo[i]).fold(0.0, f64::max).max(1.0);
        let tolerance = RELATIVE_EPSILON * scale;
        let raw = newell(&points);
        let length = dot(raw, raw).sqrt();
        if length <= RELATIVE_EPSILON * scale * scale {
            return None;
        }
        let normal = raw.map(|v| v / length);
        let offset = points.iter().map(|p| dot(normal, *p)).sum::<f64>() / points.len() as f64;
        if points
            .iter()
            .any(|p| (dot(normal, *p) - offset).abs() > tolerance)
        {
            return None;
        }
        let drop_axis = (0..3)
            .max_by(|a, b| normal[*a].abs().total_cmp(&normal[*b].abs()))
            .unwrap_or(2);
        let projected = points.iter().map(|p| drop(*p, drop_axis)).collect();
        Some(Self {
            normal,
            offset,
            drop_axis,
            points,
            projected,
            tolerance,
        })
    }
    fn contains(&self, opening: &Self) -> bool {
        if dot(self.normal, opening.normal).abs() < 1.0 - RELATIVE_EPSILON {
            return false;
        }
        let tolerance = self.tolerance.max(opening.tolerance);
        opening
            .points
            .iter()
            .all(|p| (dot(self.normal, *p) - self.offset).abs() <= tolerance)
            && opening
                .points
                .iter()
                .all(|p| inside_or_on(&self.projected, drop(*p, self.drop_axis), tolerance))
    }
}
fn newell(points: &[Point]) -> Point {
    let mut n = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        n[0] += (a[1] - b[1]) * (a[2] + b[2]);
        n[1] += (a[2] - b[2]) * (a[0] + b[0]);
        n[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    n
}
fn drop(p: Point, axis: usize) -> [f64; 2] {
    match axis {
        0 => [p[1], p[2]],
        1 => [p[0], p[2]],
        _ => [p[0], p[1]],
    }
}
/// Even-odd containment where the boundary counts as inside: a full-height
/// wall void meets its panel's top and bottom edges by construction.
fn inside_or_on(polygon: &[[f64; 2]], p: [f64; 2], tolerance: f64) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if distance_to_segment(p, a, b) <= tolerance {
            return true;
        }
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
        {
            inside = !inside;
        }
    }
    inside
}
