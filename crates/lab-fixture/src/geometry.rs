use crate::{FixtureError, Kind, LabId};

pub type Point = [f64; 3];
const EPS: f64 = 1.0e-9;

/// Outward counterclockwise faces using `Geometry::control_points` prism order.
/// Each quad is split [0,1,2], [0,2,3]; no GPU resource is created here.
pub const PRISM_FACE_RINGS: [[usize; 4]; 6] = [
    [0, 3, 2, 1],
    [4, 5, 6, 7],
    [0, 1, 5, 4],
    [3, 7, 6, 2],
    [0, 4, 7, 3],
    [1, 2, 6, 5],
];

/// World-axis bounds in metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}

impl Bounds {
    pub(crate) fn extend(&mut self, point: Point) {
        for (axis, value) in point.into_iter().enumerate() {
            self.min[axis] = self.min[axis].min(value);
            self.max[axis] = self.max[axis].max(value);
        }
    }
}

/// Authored test shapes only. Shells are planar convex single surfaces.
/// Openings, non-planar/concave surfaces and real engineering sections are unsupported.
#[derive(Clone, Debug, PartialEq)]
pub enum Geometry {
    Prism(Bounds),
    Shell {
        boundary: Vec<Point>,
        normal: Point,
        openings: Vec<Vec<Point>>,
    },
}

pub(crate) fn subtract(a: Point, b: Point) -> Point {
    std::array::from_fn(|i| a[i] - b[i])
}
pub(crate) fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn dot(a: Point, b: Point) -> f64 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}

impl Geometry {
    pub fn control_points(&self) -> Vec<Point> {
        match self {
            Self::Prism(Bounds { min, max }) => vec![
                *min,
                [max[0], min[1], min[2]],
                [max[0], max[1], min[2]],
                [min[0], max[1], min[2]],
                [min[0], min[1], max[2]],
                [max[0], min[1], max[2]],
                *max,
                [min[0], max[1], max[2]],
            ],
            Self::Shell { boundary, .. } => boundary.clone(),
        }
    }

    pub(crate) fn validate(&self, id: LabId) -> Result<(), FixtureError> {
        let points = self.control_points();
        if !points
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() <= 1.0e6)
        {
            return Err(FixtureError::InvalidCoordinate(id));
        }
        match self {
            Self::Prism(Bounds { min, max }) => {
                if !matches!(id.kind, Kind::Beam | Kind::Column) {
                    return Err(FixtureError::KindGeometryMismatch(id));
                }
                if !(0..3).all(|i| max[i] - min[i] > EPS) {
                    return Err(FixtureError::DegenerateGeometry(id));
                }
            }
            Self::Shell {
                boundary,
                normal,
                openings,
            } => {
                if !matches!(id.kind, Kind::Wall | Kind::Slab) {
                    return Err(FixtureError::KindGeometryMismatch(id));
                }
                if !openings.is_empty() {
                    return Err(FixtureError::UnsupportedOpening(id));
                }
                if boundary.len() < 3 {
                    return Err(FixtureError::DegenerateGeometry(id));
                }
                if !normal.iter().all(|v| v.is_finite())
                    || (dot(*normal, *normal) - 1.0).abs() > EPS
                {
                    return Err(FixtureError::IncorrectNormal(id));
                }
                for i in 0..boundary.len() {
                    let edge = subtract(boundary[(i + 1) % boundary.len()], boundary[i]);
                    let next = subtract(
                        boundary[(i + 2) % boundary.len()],
                        boundary[(i + 1) % boundary.len()],
                    );
                    let area_normal = cross(edge, next);
                    if dot(area_normal, area_normal) <= EPS * EPS {
                        return Err(FixtureError::DegenerateGeometry(id));
                    }
                    if dot(area_normal, *normal) <= EPS {
                        return Err(FixtureError::IncorrectNormal(id));
                    }
                    if dot(subtract(boundary[i], boundary[0]), *normal).abs() > EPS {
                        return Err(FixtureError::NonPlanarOrConcave(id));
                    }
                    // Every other vertex must lie on the inner side of every edge.
                    // This rejects concavity and self-intersections, including star polygons.
                    for point in boundary {
                        if dot(cross(edge, subtract(*point, boundary[i])), *normal) < -EPS {
                            return Err(FixtureError::NonPlanarOrConcave(id));
                        }
                    }
                }
                for i in 1..boundary.len() - 1 {
                    if dot(
                        cross(
                            subtract(boundary[i], boundary[0]),
                            subtract(boundary[i + 1], boundary[0]),
                        ),
                        *normal,
                    ) <= EPS
                    {
                        return Err(FixtureError::DegenerateGeometry(id));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn reference_mesh_counts(&self) -> (usize, usize) {
        match self {
            Self::Prism(_) => (24, 12),
            Self::Shell { boundary, .. } => (boundary.len(), boundary.len().saturating_sub(2)),
        }
    }
}
