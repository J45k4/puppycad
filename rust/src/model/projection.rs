//! Projected body geometry ("Use edges"): ordinary sketch entities locked to
//! a body face loop or edge. The solver treats them as fixed; they move only
//! when the body is re-projected (see `app/projection.rs`).

use super::*;

/// What a projected entity was taken from.
#[derive(Clone, Debug, PartialEq)]
pub enum ProjectionSource {
    /// A boundary loop curve of a face, by web face id.
    Face(String),
    /// One body edge.
    Edge(EdgeKey),
}

/// Identity of a body edge: its ends, a point halfway along it and the
/// outward normals of its two faces (zero for curved edges).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeKey {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub via: [f32; 3],
    pub normals: [[f32; 3]; 2],
}

/// A curve recognized in a projected polyline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProjectedCurve {
    Line(Point, Point),
    Circle {
        center: Point,
        radius: f32,
        segments: u16,
    },
    Arc {
        center: Point,
        radius: f32,
        start_angle: f64,
        sweep: f64,
        segments: u16,
    },
}

fn circle_through(a: Point, b: Point, c: Point) -> Option<(Point, f32)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let (a2, b2, c2) = (
        a.x * a.x + a.y * a.y,
        b.x * b.x + b.y * b.y,
        c.x * c.x + c.y * c.y,
    );
    let center = Point {
        x: (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        y: (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    };
    Some((center, center.distance(a)))
}

fn on_circle(points: &[Point], center: Point, radius: f32) -> bool {
    let tolerance = 1e-3 * radius.max(1.0);
    points
        .iter()
        .all(|p| (p.distance(center) - radius).abs() <= tolerance)
}

/// Lines for straight runs, a circle for a closed loop on one circle, an
/// arc for an open run on one circle and line segments (a polyline) for any
/// other curve.
pub fn classify_polyline(points: &[Point], closed: bool) -> Vec<ProjectedCurve> {
    let mut points: Vec<Point> = points.to_vec();
    points.dedup_by(|a, b| a.distance(*b) < 1e-5);
    if closed && points.len() > 1 && points[0].distance(*points.last().unwrap()) < 1e-5 {
        points.pop();
    }
    let n = points.len();
    if n < 2 {
        return Vec::new();
    }
    if n >= 6 {
        let (a, b, c) = (points[0], points[n / 3], points[2 * n / 3]);
        if let Some((center, radius)) = circle_through(a, b, c)
            && on_circle(&points, center, radius)
        {
            let segments = n.clamp(3, u16::MAX as usize) as u16;
            if closed {
                return vec![ProjectedCurve::Circle {
                    center,
                    radius,
                    segments,
                }];
            }
            let angle = |p: Point| ((p.y - center.y) as f64).atan2((p.x - center.x) as f64);
            let mut sweep = 0.0;
            for pair in points.windows(2) {
                let mut delta = angle(pair[1]) - angle(pair[0]);
                while delta > std::f64::consts::PI {
                    delta -= std::f64::consts::TAU;
                }
                while delta < -std::f64::consts::PI {
                    delta += std::f64::consts::TAU;
                }
                sweep += delta;
            }
            return vec![ProjectedCurve::Arc {
                center,
                radius,
                start_angle: angle(points[0]),
                sweep,
                segments: (n - 1).clamp(1, u16::MAX as usize) as u16,
            }];
        }
    }
    let mut lines = Vec::new();
    let count = if closed { n } else { n - 1 };
    for i in 0..count {
        let (a, b) = (points[i], points[(i + 1) % n]);
        // Merge collinear runs into one line.
        if let Some(ProjectedCurve::Line(start, end)) = lines.last_mut() {
            let (u, v) = (*start, *end);
            let cross = (v.x - u.x) * (b.y - a.y) - (v.y - u.y) * (b.x - a.x);
            if end.distance(a) < 1e-5
                && cross.abs() <= 1e-4 * u.distance(v).max(1.0) * a.distance(b).max(1.0)
            {
                *end = b;
                continue;
            }
        }
        lines.push(ProjectedCurve::Line(a, b));
    }
    lines
}

impl ProjectedCurve {
    pub fn shape(&self, id: u32) -> Shape {
        match *self {
            Self::Line(a, b) => Shape::Line { id, a, b },
            Self::Circle {
                center,
                radius,
                segments,
            } => Shape::Circle {
                id,
                center,
                radius,
                segments,
            },
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep,
                segments,
            } => Shape::Arc {
                id,
                center,
                radius,
                start_angle,
                sweep,
                segments,
            },
        }
    }

    /// Distance between this curve and a shape of the same kind (None for
    /// another kind): how far a re-projection would move the shape.
    pub fn distance_to(&self, shape: &Shape) -> Option<f32> {
        match (*self, shape) {
            (Self::Line(a, b), Shape::Line { a: p, b: q, .. }) => {
                let same = a.distance(*p) + b.distance(*q);
                let swapped = a.distance(*q) + b.distance(*p);
                Some(same.min(swapped))
            }
            (
                Self::Circle { center, radius, .. },
                Shape::Circle {
                    center: c,
                    radius: r,
                    ..
                },
            ) => Some(center.distance(*c) + (radius - r).abs()),
            (Self::Arc { .. }, Shape::Arc { .. }) => {
                let ends = |s: &Shape| {
                    let points = s.snap_points();
                    points.iter().map(|p| p.point).collect::<Vec<_>>()
                };
                let mine = ends(&self.shape(0));
                let theirs = ends(shape);
                Some(mine.iter().zip(&theirs).map(|(a, b)| a.distance(*b)).sum())
            }
            _ => None,
        }
    }
}

impl Document {
    /// Replaces the display-only face sketch context.
    pub fn set_reference_context(&mut self, context: crate::project::SketchContext) {
        self.reference_edges = context.edges;
        self.reference_silhouette = context.silhouette;
    }

    /// Adds `curves` as projected entities from `source` (one undo step).
    /// Curves that coincide with an existing projected entity are skipped.
    /// Returns the new ids.
    pub fn add_projected(
        &mut self,
        curves: &[(ProjectedCurve, ProjectionSource)],
        label: &str,
    ) -> Vec<u32> {
        let fresh: Vec<&(ProjectedCurve, ProjectionSource)> = curves
            .iter()
            .filter(|(curve, _)| {
                !self.projections.keys().any(|id| {
                    self.shapes
                        .iter()
                        .find(|shape| shape.id() == *id)
                        .and_then(|shape| curve.distance_to(shape))
                        .is_some_and(|d| d < 1e-4)
                })
            })
            .collect();
        if fresh.is_empty() {
            return Vec::new();
        }
        self.record_undo(self.snapshot(), label);
        self.future.clear();
        let mut ids = Vec::new();
        for (curve, source) in fresh {
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            self.shapes.push(curve.shape(id));
            self.projections.insert(id, source.clone());
            ids.push(id);
        }
        self.set_selection(&ids);
        ids
    }

    /// Moves projected shape `id` onto `curve` (no undo step: re-projection
    /// follows the body). False when the kinds differ.
    pub fn reproject(&mut self, id: u32, curve: &ProjectedCurve) -> bool {
        let Some(index) = self.shapes.iter().position(|shape| shape.id() == id) else {
            return false;
        };
        if curve.distance_to(&self.shapes[index]).is_none() {
            return false;
        }
        let moved = curve.shape(id);
        if moved != self.shapes[index] {
            self.shapes[index] = moved;
            self.revision.set(next_revision());
        }
        true
    }

    /// Drops the projection of `id`, leaving a plain entity.
    pub fn unlink_projection(&mut self, id: u32) {
        self.projections.remove(&id);
    }

    pub fn is_projected(&self, id: u32) -> bool {
        self.projections.contains_key(&id)
    }

    /// Toggles construction of the selected projected shapes (one undo step).
    /// None when no projected shape is selected.
    pub fn toggle_projected_construction(&mut self) -> Option<bool> {
        let ids: Vec<u32> = self
            .selected_ids()
            .into_iter()
            .filter(|id| self.projections.contains_key(id))
            .collect();
        if ids.is_empty() {
            return None;
        }
        self.record_undo(self.snapshot(), "Toggle projected construction");
        self.future.clear();
        let enabled = !ids.iter().all(|id| self.construction.contains(id));
        for id in ids {
            if enabled {
                self.construction.insert(id);
            } else {
                self.construction.remove(&id);
            }
        }
        Some(enabled)
    }
}
