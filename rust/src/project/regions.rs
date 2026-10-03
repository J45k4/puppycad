//! Regions of a line arrangement: when sketch curves branch or end on
//! another curve (a line splitting a projected face outline), the bounded
//! faces of the planar arrangement are the profiles.

use super::*;

const WELD: f32 = 1e-5;

fn cross(a: Point, b: Point, c: Point) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Parameter along `a`-`b` of `p` when `p` lies strictly inside it.
fn interior_parameter(p: Point, a: Point, b: Point) -> Option<f32> {
    let length = a.distance(b);
    if length < WELD || p.distance(a) < WELD || p.distance(b) < WELD {
        return None;
    }
    if cross(a, b, p).abs() / length > WELD {
        return None;
    }
    let t = ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / (length * length);
    (t > 0.0 && t < 1.0).then_some(t)
}

/// True when some segment end has more than two segment ends at it or lies
/// inside another segment: the curves form an arrangement, not loops.
pub(super) fn needs_arrangement(segments: &[(Point, Point)]) -> bool {
    let ends: Vec<Point> = segments.iter().flat_map(|&(a, b)| [a, b]).collect();
    ends.iter().any(|p| {
        ends.iter().filter(|q| q.distance(*p) <= WELD).count() > 2
            || segments
                .iter()
                .any(|&(a, b)| interior_parameter(*p, a, b).is_some())
    })
}

/// Bounded faces of the arrangement of `segments` (ends inside other
/// segments and proper crossings split them; dangling parts are dropped),
/// each as a counter-clockwise loop.
pub(super) fn arrangement_faces(segments: &[(Point, Point)]) -> Vec<Vec<Point>> {
    // Split every segment at the ends and crossings of the others.
    let mut pieces: Vec<(Point, Point)> = Vec::new();
    for (i, &(a, b)) in segments.iter().enumerate() {
        let mut cuts = vec![0.0f32, 1.0];
        for (j, &(c, d)) in segments.iter().enumerate() {
            if i == j {
                continue;
            }
            for p in [c, d] {
                if let Some(t) = interior_parameter(p, a, b) {
                    cuts.push(t);
                }
            }
            let denominator = (b.x - a.x) * (d.y - c.y) - (b.y - a.y) * (d.x - c.x);
            if denominator.abs() > 1e-12 {
                let t = ((c.x - a.x) * (d.y - c.y) - (c.y - a.y) * (d.x - c.x)) / denominator;
                let u = ((c.x - a.x) * (b.y - a.y) - (c.y - a.y) * (b.x - a.x)) / denominator;
                if t > 1e-6 && t < 1.0 - 1e-6 && u > 1e-6 && u < 1.0 - 1e-6 {
                    cuts.push(t);
                }
            }
        }
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|x, y| (*x - *y).abs() * a.distance(b) < WELD);
        for pair in cuts.windows(2) {
            let at = |t: f32| Point {
                x: a.x + (b.x - a.x) * t,
                y: a.y + (b.y - a.y) * t,
            };
            pieces.push((at(pair[0]), at(pair[1])));
        }
    }
    // Weld vertices.
    let mut vertices: Vec<Point> = Vec::new();
    let mut vertex = |p: Point| {
        vertices
            .iter()
            .position(|q| q.distance(p) <= WELD * 10.0)
            .unwrap_or_else(|| {
                vertices.push(p);
                vertices.len() - 1
            })
    };
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for (a, b) in pieces {
        let (u, v) = (vertex(a), vertex(b));
        if u != v
            && !edges
                .iter()
                .any(|&(p, q)| (p, q) == (u, v) || (p, q) == (v, u))
        {
            edges.push((u, v));
        }
    }
    // Drop dangling edges.
    loop {
        let degree = |n: usize, edges: &[(usize, usize)]| {
            edges.iter().filter(|&&(u, v)| u == n || v == n).count()
        };
        let before = edges.len();
        let snapshot = edges.clone();
        edges.retain(|&(u, v)| degree(u, &snapshot) > 1 && degree(v, &snapshot) > 1);
        if edges.len() == before {
            break;
        }
    }
    // Outgoing half-edges sorted counter-clockwise at each vertex.
    let angle = |u: usize, v: usize| {
        let (p, q) = (vertices[u], vertices[v]);
        (q.y - p.y).atan2(q.x - p.x)
    };
    let mut outgoing: Vec<Vec<usize>> = vec![Vec::new(); vertices.len()];
    for &(u, v) in &edges {
        outgoing[u].push(v);
        outgoing[v].push(u);
    }
    for (u, list) in outgoing.iter_mut().enumerate() {
        list.sort_by(|&a, &b| angle(u, a).total_cmp(&angle(u, b)));
    }
    let mut used: std::collections::HashSet<(usize, usize)> = Default::default();
    let mut faces = Vec::new();
    for &(u0, v0) in &edges {
        for start in [(u0, v0), (v0, u0)] {
            if used.contains(&start) {
                continue;
            }
            let mut face = Vec::new();
            let (mut u, mut v) = start;
            loop {
                used.insert((u, v));
                face.push(vertices[u]);
                // At v, turn to the edge just clockwise of the way back: the
                // face stays on the left.
                let list = &outgoing[v];
                let back = list.iter().position(|&w| w == u).unwrap();
                let next = list[(back + list.len() - 1) % list.len()];
                (u, v) = (v, next);
                if (u, v) == start || face.len() > edges.len() * 2 {
                    break;
                }
            }
            if signed_area(&face) > 1e-9 {
                faces.push(face);
            }
        }
    }
    faces
}
