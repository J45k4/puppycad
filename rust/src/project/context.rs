//! The body a face sketch is drawn on: the evaluated bodies it sees and
//! their outline projected into the sketch plane (the dashed context).

use super::*;

/// Plane distance within which an edge lies on the sketch face.
const ON_PLANE: f32 = 1e-3;

/// A face sketch's body context in sketch coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SketchContext {
    /// Boundary edges of the face the sketch lies on (every loop: outer
    /// boundary, holes, notches), straight runs merged.
    pub edges: Vec<(Point, Point)>,
    /// The other sharp edges of the bodies, projected along the normal.
    pub silhouette: Vec<(Point, Point)>,
}

fn same_segment(u: (Point, Point), v: (Point, Point)) -> bool {
    let close = |a: Point, b: Point| a.distance(b) < 1e-4;
    (close(u.0, v.0) && close(u.1, v.1)) || (close(u.0, v.1) && close(u.1, v.0))
}

/// Joins collinear segments that continue each other through a shared end.
fn merge_collinear(mut segments: Vec<(Point, Point)>) -> Vec<(Point, Point)> {
    let join = |s: (Point, Point), t: (Point, Point)| -> Option<(Point, Point)> {
        let length = s.0.distance(s.1).max(1e-9);
        let cross =
            |p: Point| ((s.1.x - s.0.x) * (p.y - s.0.y) - (s.1.y - s.0.y) * (p.x - s.0.x)) / length;
        if cross(t.0).abs() > 1e-4 || cross(t.1).abs() > 1e-4 {
            return None;
        }
        for (shared, far) in [(s.0, s.1), (s.1, s.0)] {
            for (other, far2) in [(t.0, t.1), (t.1, t.0)] {
                let continues = (far.x - shared.x) * (far2.x - shared.x)
                    + (far.y - shared.y) * (far2.y - shared.y)
                    < 0.0;
                if shared.distance(other) < 1e-4 && continues {
                    return Some((far, far2));
                }
            }
        }
        None
    };
    loop {
        let mut merged = None;
        'outer: for i in 0..segments.len() {
            for j in i + 1..segments.len() {
                if let Some(joined) = join(segments[i], segments[j]) {
                    merged = Some((i, j, joined));
                    break 'outer;
                }
            }
        }
        let Some((i, j, joined)) = merged else {
            return segments;
        };
        segments[i] = joined;
        segments.swap_remove(j);
    }
}

impl ProjectFile {
    /// Bodies a sketch sees: the result of the active features before the
    /// first feature built from it (all features when none is), so a
    /// sketch's own cuts never feed back into its context or projections.
    pub fn context_bodies(&self, index: usize) -> Vec<Manifold> {
        let Some(sketch) = self.sketches.get(index) else {
            return Vec::new();
        };
        let quoted = format!("\"{}\"", sketch.id);
        let first = self
            .feature_nodes()
            .iter()
            .position(|node| node.to_string().contains(&quoted))
            .unwrap_or(usize::MAX);
        self.bodies_before(first)
    }

    /// Face sketch `index`'s body context; empty for reference-plane
    /// sketches.
    pub fn sketch_context(&self, index: usize) -> SketchContext {
        let Some(sketch) = self.sketches.get(index) else {
            return SketchContext::default();
        };
        if sketch.face.is_none() {
            return SketchContext::default();
        }
        let Some(frame) = self.sketch_frame(&sketch.id) else {
            return SketchContext::default();
        };
        context_in_frame(&self.context_bodies(index), &frame)
    }

    /// The edges of [`ProjectFile::sketch_context`] (kept for callers that
    /// only snap).
    pub fn sketch_support_edges(&self, index: usize) -> Vec<(Point, Point)> {
        self.sketch_context(index).edges
    }
}

/// Splits the sharp edges of `bodies` into the boundary of the face lying
/// in `frame`'s plane (outward normal = frame normal) and the projected rest.
pub fn context_in_frame(bodies: &[Manifold], frame: &Frame) -> SketchContext {
    let normal = frame.normal;
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut face = Vec::new();
    let mut rest: Vec<(Point, Point)> = Vec::new();
    for body in bodies {
        for edge in sharp_edges(body) {
            let (a, da) = frame.local(edge.a);
            let (b, db) = frame.local(edge.b);
            if a.distance(b) < 1e-5 {
                continue;
            }
            let in_plane = da.abs() < ON_PLANE && db.abs() < ON_PLANE;
            if in_plane && edge.normals.iter().any(|n| dot(*n, normal) > 0.9999) {
                face.push((a, b));
            } else if !rest.iter().any(|&s| same_segment(s, (a, b))) {
                rest.push((a, b));
            }
        }
    }
    let edges = merge_collinear(face);
    let silhouette = merge_collinear(rest)
        .into_iter()
        .filter(|&s| !edges.iter().any(|&e| same_segment(e, s)))
        .collect();
    SketchContext { edges, silhouette }
}
