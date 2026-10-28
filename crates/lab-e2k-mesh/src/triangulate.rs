//! Ear-clipping for planar, simple (possibly concave) shell rings.
//!
//! Convex rings keep the fan in `Builder::polygon`. A ring reaches this module
//! only when it is not strictly convex; it is triangulated only if it is planar
//! and simple, so a self-intersecting or warped ring is still withheld rather
//! than drawn as a guess. Every triangle winds counterclockwise about the
//! returned unit normal, which is the ring's own Newell orientation.
use super::{Point, cross, dot, sub, unit};

/// Largest ring this viewer triangulates; `convex_normal` uses the same bound.
const MAX_VERTICES: usize = 128;

/// Unit normal and triangles (indices into `points`), or `None` for a ring
/// that is degenerate, nonplanar or self-intersecting.
pub(super) fn ear_clip(points: &[Point]) -> Option<(Point, Vec<[usize; 3]>)> {
    let n = points.len();
    if !(3..=MAX_VERTICES).contains(&n) {
        return None;
    }
    let mut newell = [0.0; 3];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % n];
        newell[0] += (a[1] - b[1]) * (a[2] + b[2]);
        newell[1] += (a[2] - b[2]) * (a[0] + b[0]);
        newell[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let normal = unit(newell)?;
    let extent = points
        .iter()
        .map(|p| dot(sub(*p, points[0]), sub(*p, points[0])).sqrt())
        .fold(0.0, f64::max);
    let tolerance = 1.0e-6 * extent.max(1.0);
    if points
        .iter()
        .any(|p| dot(sub(*p, points[0]), normal).abs() > tolerance)
    {
        return None;
    }
    let helper = if normal[2].abs() < 0.9 {
        [0.0, 0.0, 1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let u = unit(cross(helper, normal))?;
    let v = cross(normal, u);
    // (u, v, normal) is right-handed, so the Newell orientation is CCW in 2D.
    let flat: Vec<[f64; 2]> = points.iter().map(|p| [dot(*p, u), dot(*p, v)]).collect();
    if !simple(&flat, tolerance) {
        return None;
    }
    let area_floor = tolerance * tolerance;
    let mut ring: Vec<usize> = (0..n).collect();
    // A vertex on a straight run is never an ear; dropping it keeps the area.
    while ring.len() > 3 {
        let m = ring.len();
        let Some(i) = (0..m).find(|&i| {
            let (a, b, c) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            orient(flat[a], flat[b], flat[c]).abs() <= area_floor
        }) else {
            break;
        };
        ring.remove(i);
    }
    let mut triangles = Vec::with_capacity(n - 2);
    while ring.len() > 3 {
        let m = ring.len();
        let ear = (0..m).find(|&i| {
            let (a, b, c) = (ring[(i + m - 1) % m], ring[i], ring[(i + 1) % m]);
            orient(flat[a], flat[b], flat[c]) > area_floor
                && ring
                    .iter()
                    .filter(|&&k| k != a && k != b && k != c)
                    .all(|&k| !in_triangle(flat[k], flat[a], flat[b], flat[c]))
        })?;
        triangles.push([ring[(ear + m - 1) % m], ring[ear], ring[(ear + 1) % m]]);
        ring.remove(ear);
    }
    if orient(flat[ring[0]], flat[ring[1]], flat[ring[2]]) <= area_floor {
        return None;
    }
    triangles.push([ring[0], ring[1], ring[2]]);
    Some((normal, triangles))
}
fn orient(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
/// Closed-triangle containment, so a vertex touching an ear blocks it.
fn in_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    orient(a, b, p) >= 0.0 && orient(b, c, p) >= 0.0 && orient(c, a, p) >= 0.0
}
/// No repeated vertex and no two non-adjacent edges that touch or cross.
fn simple(p: &[[f64; 2]], tolerance: f64) -> bool {
    let n = p.len();
    let segment_distance = |a: [f64; 2], b: [f64; 2], q: [f64; 2]| {
        let ab = [b[0] - a[0], b[1] - a[1]];
        let len2 = ab[0] * ab[0] + ab[1] * ab[1];
        let t = if len2 > 0.0 {
            (((q[0] - a[0]) * ab[0] + (q[1] - a[1]) * ab[1]) / len2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        (q[0] - a[0] - t * ab[0]).hypot(q[1] - a[1] - t * ab[1])
    };
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        if (b[0] - a[0]).hypot(b[1] - a[1]) <= tolerance {
            return false;
        }
        for j in i + 1..n {
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (c, d) = (p[j], p[(j + 1) % n]);
            let crosses =
                orient(a, b, c) * orient(a, b, d) < 0.0 && orient(c, d, a) * orient(c, d, b) < 0.0;
            let touches = segment_distance(a, b, c) <= tolerance
                || segment_distance(a, b, d) <= tolerance
                || segment_distance(c, d, a) <= tolerance
                || segment_distance(c, d, b) <= tolerance;
            if crosses || touches {
                return false;
            }
        }
    }
    true
}
