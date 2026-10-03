//! The part feature tree: an ordered list of solid features (sketches are not
//! features) with per-feature suppression and a part-level rollback bar, and
//! the native revolve, loft and edge fillet/chamfer features evaluated with
//! `manifold-rust`.
//!
//! Order is the order of feature nodes in `data.cad.nodes` (mirrored in
//! `data.tree.orderedNodeIds` and `data.cad.rootNodeIds`). Suppression is
//! `"suppressed": true` on the graph node and the rollback bar is
//! `data.tree.rollbackIndex` (number of features before the bar). Both are
//! native extensions the web loader ignores.
//!
//! Feature kinds (`FeatureKind`): web `extrude` nodes plus three native
//! extension node types the web loader ignores (the web `chamfer` node
//! references web topology edge ids the native app cannot resolve):
//!
//! * `{"type":"revolve", sketchId, profileId, axis:{a:{x,y}, b:{x,y}}, angle, operation}`
//!   with the axis in sketch coordinates and the angle in degrees (0, 360].
//! * `{"type":"loft", profiles:[{sketchId, profileId}, {sketchId, profileId}], operation}`
//! * `{"type":"edgeFinish", kind:"chamfer"|"fillet", distance|radius, edges:[{a:[x,y,z], b:[x,y,z], normals:[[..],[..]]}]}`
//!   Edges are identified by their world endpoints and the outward normals of
//!   the two faces that meet there, and re-identified on every evaluation.
//!
//! Evaluation walks the features in order up to the bar, skipping suppressed
//! ones. A feature whose inputs are missing (sketch or profile gone, target
//! body suppressed or failed, face source inactive, finished edge gone) or
//! whose Boolean fails is marked `Error` and skipped instead of failing the
//! whole part. Results are memoized per feature: a feature is rebuilt only
//! when it, or anything before it, changed.

use std::cell::{Cell, RefCell};

use super::*;

use crate::stl::manifold_from_triangles;

pub type Tri = [[f32; 3]; 3];

/// Node types that build or modify solid bodies, in graph order.
pub(super) const SOLID_NODE_TYPES: [&str; 4] = ["extrude", "revolve", "loft", "edgeFinish"];
/// Solid node types that are native extensions.
pub(super) const EXTENSION_NODE_TYPES: [&str; 3] = ["revolve", "loft", "edgeFinish"];

/// Revolve segments for a full turn (the circle segment convention).
pub const REVOLVE_SEGMENTS: usize = 32;
/// Minimum angle between face normals for an edge to count as sharp.
pub const SHARP_EDGE_DEGREES: f32 = 20.0;

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f32; 3], s: f32) -> [f32; 3] {
    a.map(|c| c * s)
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

fn unit(a: [f32; 3]) -> Option<[f32; 3]> {
    let l = length(a);
    (l > 1e-9).then(|| scale(a, 1.0 / l))
}

/// Triangulates loops (outer first, then holes) given in 2D; every returned
/// triangle is counter-clockwise.
fn triangulate(loops: &[Vec<Point>]) -> Result<Vec<[usize; 3]>, String> {
    let mut coordinates = Vec::new();
    let mut holes = Vec::new();
    let mut points = Vec::new();
    for (index, loop_points) in loops.iter().enumerate() {
        if index > 0 {
            holes.push(points.len());
        }
        for p in loop_points {
            coordinates.extend([p.x as f64, p.y as f64]);
            points.push(*p);
        }
    }
    let indices = earcutr::earcut(&coordinates, &holes, 2)
        .map_err(|_| "profile cannot be triangulated".to_string())?;
    Ok(indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|c| {
            let [a, b, d] = [points[c[0]], points[c[1]], points[c[2]]];
            if (b.x - a.x) * (d.y - a.y) - (b.y - a.y) * (d.x - a.x) < 0.0 {
                [c[0], c[2], c[1]]
            } else {
                [c[0], c[1], c[2]]
            }
        })
        .collect())
}

/// Sweeps a profile (with holes) around `axis` (two points in sketch
/// coordinates) by `angle` degrees. The profile must stay on one side of
/// the axis; points on the axis are allowed.
pub fn revolve_triangles(
    loops: &ProfileLoops,
    frame: &Frame,
    axis: (Point, Point),
    angle: f32,
) -> Result<Vec<Tri>, String> {
    if !angle.is_finite() || angle <= 0.0 || angle > 360.0 {
        return Err("revolve angle must be between 0 and 360 degrees".into());
    }
    let (a, b) = axis;
    let axis_length = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    if !axis_length.is_finite() || axis_length < 1e-6 {
        return Err("revolve axis has no length".into());
    }
    let u = Point {
        x: (b.x - a.x) / axis_length,
        y: (b.y - a.y) / axis_length,
    };
    let v = Point { x: -u.y, y: u.x };
    let all: Vec<&Vec<Point>> = std::iter::once(&loops.outer).chain(&loops.holes).collect();
    let coords: Vec<Vec<(f32, f32)>> = all
        .iter()
        .map(|l| {
            l.iter()
                .map(|p| {
                    let d = Point {
                        x: p.x - a.x,
                        y: p.y - a.y,
                    };
                    (d.x * u.x + d.y * u.y, d.x * v.x + d.y * v.y)
                })
                .collect()
        })
        .collect();
    let extent = coords
        .iter()
        .flatten()
        .fold(1.0f32, |m, (t, s)| m.max(t.abs()).max(s.abs()));
    let eps = extent * 1e-5;
    let (min_s, max_s) = coords
        .iter()
        .flatten()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), (_, s)| {
            (lo.min(*s), hi.max(*s))
        });
    if min_s < -eps && max_s > eps {
        return Err("profile crosses the revolve axis".into());
    }
    if max_s <= eps && min_s >= -eps {
        return Err("profile lies on the revolve axis".into());
    }
    let sigma = if max_s > eps { 1.0 } else { -1.0 };
    // (t, s') with s' >= 0; outer loop CCW, holes CW.
    let local: Vec<Vec<Point>> = coords
        .iter()
        .enumerate()
        .map(|(index, l)| {
            let points: Vec<Point> = l
                .iter()
                .map(|(t, s)| {
                    let s = s * sigma;
                    Point {
                        x: *t,
                        y: if s < eps { 0.0 } else { s },
                    }
                })
                .collect();
            orient_loop(points, index == 0)
        })
        .collect();
    let full = angle >= 360.0 - 1e-3;
    let segments = ((REVOLVE_SEGMENTS as f32 * angle / 360.0).ceil() as usize).max(2);
    let rings = if full { segments } else { segments + 1 };
    let radians = angle.to_radians();
    let position = |p: Point, k: usize| -> [f32; 3] {
        let phi = radians * (k % rings) as f32 / segments as f32;
        let (sin, cos) = if p.y == 0.0 {
            (0.0, 0.0)
        } else {
            phi.sin_cos()
        };
        let r = sigma * p.y;
        frame.world(
            Point {
                x: a.x + p.x * u.x + r * cos * v.x,
                y: a.y + p.x * u.y + r * cos * v.y,
            },
            r * sin,
        )
    };
    let mut triangles = Vec::new();
    for l in &local {
        for i in 0..l.len() {
            let j = (i + 1) % l.len();
            for k in 0..segments {
                let k1 = k + 1;
                let [pik, pjk, pjk1, pik1] = [
                    position(l[i], k),
                    position(l[j], k),
                    position(l[j], k1),
                    position(l[i], k1),
                ];
                triangles.push([pik, pjk, pjk1]);
                triangles.push([pik, pjk1, pik1]);
            }
        }
    }
    if !full {
        let points: Vec<Point> = local.iter().flatten().copied().collect();
        for [i, j, k] in triangulate(&local)? {
            let end = |n: usize| [i, j, k].map(|index| position(points[index], n));
            let [p, q, r] = end(0);
            triangles.push([p, r, q]);
            triangles.push(end(segments));
        }
    }
    Ok(triangles)
}

/// Total arc-length fraction of each vertex of a closed loop.
fn loop_parameters(points: &[[f32; 3]]) -> Vec<f32> {
    let mut lengths = vec![0.0];
    for i in 1..points.len() {
        let last = lengths[i - 1];
        lengths.push(last + length(sub(points[i], points[i - 1])));
    }
    let total = lengths[points.len() - 1] + length(sub(points[0], points[points.len() - 1]));
    lengths.iter().map(|l| l / total.max(1e-12)).collect()
}

fn sample_loop(points: &[[f32; 3]], parameters: &[f32], t: f32) -> [f32; 3] {
    let n = points.len();
    let index = parameters.partition_point(|p| *p <= t).saturating_sub(1);
    let start = parameters[index];
    let end = if index + 1 < n {
        parameters[index + 1]
    } else {
        1.0
    };
    let f = if end - start > 1e-12 {
        (t - start) / (end - start)
    } else {
        0.0
    };
    let (p, q) = (points[index], points[(index + 1) % n]);
    add(p, scale(sub(q, p), f))
}

fn newell(points: &[[f32; 3]]) -> [f32; 3] {
    let mut normal = [0.0; 3];
    for i in 0..points.len() {
        let (p, q) = (points[i], points[(i + 1) % points.len()]);
        normal[0] += (p[1] - q[1]) * (p[2] + q[2]);
        normal[1] += (p[2] - q[2]) * (p[0] + q[0]);
        normal[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    normal
}

fn centroid(points: &[[f32; 3]]) -> [f32; 3] {
    scale(
        points.iter().fold([0.0; 3], |sum, p| add(sum, *p)),
        1.0 / points.len() as f32,
    )
}

/// Ruled loft between two closed outer loops on (usually parallel) planes.
/// Both loops are resampled at the union of their arc-length parameters, so
/// every original vertex is kept, after aligning the start of the second loop
/// to the point nearest the first loop's start.
pub fn loft_triangles(
    first: &[Point],
    first_frame: &Frame,
    second: &[Point],
    second_frame: &Frame,
) -> Result<Vec<Tri>, String> {
    if first.len() < 3 || second.len() < 3 {
        return Err("loft profiles need at least three points".into());
    }
    let a: Vec<[f32; 3]> = first.iter().map(|p| first_frame.world(*p, 0.0)).collect();
    let mut b: Vec<[f32; 3]> = second.iter().map(|p| second_frame.world(*p, 0.0)).collect();
    let (ca, cb) = (centroid(&a), centroid(&b));
    let direction = sub(cb, ca);
    if length(direction) < 1e-4 {
        return Err("loft profiles must not share a centre".into());
    }
    let (na, nb) = (dot(newell(&a), direction), dot(newell(&b), direction));
    if na.abs() < 1e-6 || nb.abs() < 1e-6 {
        return Err("a loft profile is edge-on to the loft direction".into());
    }
    if (na > 0.0) != (nb > 0.0) {
        b.reverse();
    }
    let start = (0..b.len())
        .min_by(|&i, &j| length(sub(b[i], a[0])).total_cmp(&length(sub(b[j], a[0]))))
        .unwrap_or(0);
    b.rotate_left(start);
    let (pa, pb) = (loop_parameters(&a), loop_parameters(&b));
    let mut parameters: Vec<f32> = pa.iter().chain(&pb).copied().collect();
    parameters.sort_by(f32::total_cmp);
    parameters.dedup_by(|x, y| (*x - *y).abs() < 1e-6);
    let ra: Vec<[f32; 3]> = parameters
        .iter()
        .map(|t| sample_loop(&a, &pa, *t))
        .collect();
    let rb: Vec<[f32; 3]> = parameters
        .iter()
        .map(|t| sample_loop(&b, &pb, *t))
        .collect();
    let m = ra.len();
    let mut triangles = Vec::new();
    for i in 0..m {
        let j = (i + 1) % m;
        if na > 0.0 {
            triangles.push([ra[i], ra[j], rb[j]]);
            triangles.push([ra[i], rb[j], rb[i]]);
        } else {
            triangles.push([ra[j], ra[i], rb[i]]);
            triangles.push([ra[j], rb[i], rb[j]]);
        }
    }
    for (ring, frame, outward) in [
        (&ra, first_frame, scale(direction, -1.0)),
        (&rb, second_frame, direction),
    ] {
        let flat: Vec<Point> = ring.iter().map(|p| frame.local(*p).0).collect();
        for [i, j, k] in triangulate(&[flat])? {
            let [p, q, r] = [ring[i], ring[j], ring[k]];
            if dot(cross3(sub(q, p), sub(r, p)), outward) < 0.0 {
                triangles.push([p, r, q]);
            } else {
                triangles.push([p, q, r]);
            }
        }
    }
    Ok(triangles)
}

/// A straight run of sharp mesh edges between two faces of a body.
#[derive(Clone, Debug, PartialEq)]
pub struct SolidEdge {
    pub a: [f32; 3],
    pub b: [f32; 3],
    /// Outward normals of the two faces.
    pub normals: [[f32; 3]; 2],
    /// Unit directions inside each face, perpendicular to the edge and
    /// pointing away from it.
    inward: [[f32; 3]; 2],
    /// False when the edge is one facet of a tessellated curve.
    pub straight: bool,
}

/// Stored identity of a finished edge.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeRef {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub normals: [[f32; 3]; 2],
}

impl SolidEdge {
    pub fn reference(&self) -> EdgeRef {
        EdgeRef {
            a: self.a,
            b: self.b,
            normals: self.normals,
        }
    }

    pub fn length(&self) -> f32 {
        length(sub(self.b, self.a))
    }

    /// Convex edges get material removed; concave edges get material added.
    pub fn convex(&self) -> bool {
        dot(self.inward[0], self.normals[1]) < 0.0
    }
}

impl EdgeRef {
    pub fn to_json(&self) -> Value {
        json!({"a": self.a, "b": self.b, "normals": self.normals})
    }

    pub fn from_json(value: &Value) -> Option<Self> {
        let vector = |value: &Value| -> Option<[f32; 3]> {
            let array = value.as_array().filter(|a| a.len() == 3)?;
            let v: Vec<f32> = array
                .iter()
                .map(|c| c.as_f64().map(|c| c as f32).filter(|c| c.is_finite()))
                .collect::<Option<_>>()?;
            Some([v[0], v[1], v[2]])
        };
        let normals = value.get("normals")?.as_array().filter(|n| n.len() == 2)?;
        Some(Self {
            a: vector(value.get("a")?)?,
            b: vector(value.get("b")?)?,
            normals: [vector(&normals[0])?, vector(&normals[1])?],
        })
    }
}

/// Positions and triangles of a manifold with coincident vertices merged.
fn welded_mesh(body: &Manifold) -> (Vec<[f32; 3]>, Vec<[usize; 3]>) {
    let mesh = body.get_mesh_gl(0);
    let mut points = Vec::new();
    let mut index = HashMap::new();
    let remap: Vec<usize> = mesh
        .vert_properties
        .chunks_exact(mesh.num_prop as usize)
        .map(|p| {
            let point = [p[0], p[1], p[2]];
            *index
                .entry(point.map(|c| (c + 0.0).to_bits()))
                .or_insert_with(|| {
                    points.push(point);
                    points.len() - 1
                })
        })
        .collect();
    let triangles = mesh
        .tri_verts
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| t.map(|i| remap[i as usize]))
        .collect();
    (points, triangles)
}

/// Sharp edges (adjacent face normals more than 20° apart) of a body, with
/// collinear segments between the same two planes merged into one edge.
pub fn sharp_edges(body: &Manifold) -> Vec<SolidEdge> {
    let (points, triangles) = welded_mesh(body);
    let normals: Vec<Option<[f32; 3]>> = triangles
        .iter()
        .map(|[a, b, c]| {
            unit(cross3(
                sub(points[*b], points[*a]),
                sub(points[*c], points[*a]),
            ))
        })
        .collect();
    let mut edge_faces: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    for (t, tri) in triangles.iter().enumerate() {
        for k in 0..3 {
            let (i, j, third) = (tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]);
            edge_faces
                .entry((i.min(j), i.max(j)))
                .or_default()
                .push((t, third));
        }
    }
    let threshold = SHARP_EDGE_DEGREES.to_radians().cos();
    struct Segment {
        ends: [usize; 2],
        normals: [[f32; 3]; 2],
        thirds: [usize; 2],
        direction: [f32; 3],
    }
    let mut segments = Vec::new();
    let mut keys: Vec<_> = edge_faces.keys().copied().collect();
    keys.sort_unstable();
    for key in keys {
        let faces = &edge_faces[&key];
        if faces.len() != 2 {
            continue;
        }
        let (Some(n1), Some(n2)) = (normals[faces[0].0], normals[faces[1].0]) else {
            continue;
        };
        let Some(direction) = unit(sub(points[key.1], points[key.0])) else {
            continue;
        };
        if dot(n1, n2) < threshold {
            segments.push(Segment {
                ends: [key.0, key.1],
                normals: [n1, n2],
                thirds: [faces[0].1, faces[1].1],
                direction,
            });
        }
    }
    let same_normals = |a: &[[f32; 3]; 2], b: &[[f32; 3]; 2]| {
        (dot(a[0], b[0]) > 0.9999 && dot(a[1], b[1]) > 0.9999)
            || (dot(a[0], b[1]) > 0.9999 && dot(a[1], b[0]) > 0.9999)
    };
    let mut at_vertex: HashMap<usize, Vec<usize>> = HashMap::new();
    for (index, segment) in segments.iter().enumerate() {
        for end in segment.ends {
            at_vertex.entry(end).or_default().push(index);
        }
    }
    let mut used = vec![false; segments.len()];
    let mut edges = Vec::new();
    for start in 0..segments.len() {
        if used[start] {
            continue;
        }
        used[start] = true;
        let seed = &segments[start];
        let mut ends = seed.ends;
        let mut straight = true;
        for end in &mut ends {
            loop {
                let vertex = *end;
                let next = at_vertex[&vertex].iter().copied().find(|&other| {
                    !used[other]
                        && dot(segments[other].direction, seed.direction).abs() > 0.99999
                        && same_normals(&segments[other].normals, &seed.normals)
                });
                let Some(next) = next else {
                    let curved = at_vertex[&vertex].iter().any(|&other| {
                        !used[other]
                            && dot(segments[other].direction, seed.direction).abs() > threshold
                    });
                    straight &= !curved;
                    break;
                };
                used[next] = true;
                let s = &segments[next];
                *end = if s.ends[0] == vertex {
                    s.ends[1]
                } else {
                    s.ends[0]
                };
            }
        }
        let (a, b) = (points[ends[0]], points[ends[1]]);
        let direction = seed.direction;
        let inward = [0, 1].map(|k| {
            let d = sub(points[seed.thirds[k]], a);
            unit(sub(d, scale(direction, dot(d, direction)))).unwrap_or([0.0; 3])
        });
        edges.push(SolidEdge {
            a,
            b,
            normals: seed.normals,
            inward,
            straight,
        });
    }
    edges
}

fn edge_tolerance(body: &Manifold) -> f32 {
    let bounds = body.bounding_box();
    let diagonal = ((bounds.max.x - bounds.min.x).powi(2)
        + (bounds.max.y - bounds.min.y).powi(2)
        + (bounds.max.z - bounds.min.z).powi(2))
    .sqrt() as f32;
    diagonal.max(1.0) * 1e-4
}

/// Finds a stored edge again: same endpoints, or else an edge between the
/// same two planes along the same line (upstream changes may move its ends).
pub fn match_edge<'e>(
    edges: &'e [SolidEdge],
    target: &EdgeRef,
    tolerance: f32,
) -> Option<&'e SolidEdge> {
    let near = |p: [f32; 3], q: [f32; 3]| length(sub(p, q)) <= tolerance;
    if let Some(edge) = edges.iter().find(|e| {
        (near(e.a, target.a) && near(e.b, target.b)) || (near(e.a, target.b) && near(e.b, target.a))
    }) {
        return Some(edge);
    }
    let same_normals = |e: &SolidEdge| {
        let (a, b) = (e.normals, target.normals);
        (dot(a[0], b[0]) > 0.9999 && dot(a[1], b[1]) > 0.9999)
            || (dot(a[0], b[1]) > 0.9999 && dot(a[1], b[0]) > 0.9999)
    };
    let off_line = |e: &SolidEdge, p: [f32; 3]| {
        let Some(direction) = unit(sub(e.b, e.a)) else {
            return f32::INFINITY;
        };
        let d = sub(p, e.a);
        length(sub(d, scale(direction, dot(d, direction))))
    };
    edges
        .iter()
        .filter(|e| same_normals(e))
        .filter(|e| {
            off_line(e, target.a) <= tolerance * 10.0 && off_line(e, target.b) <= tolerance * 10.0
        })
        .max_by(|x, y| x.length().total_cmp(&y.length()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinishKind {
    Chamfer,
    Fillet,
}

impl FinishKind {
    pub fn name(self) -> &'static str {
        match self {
            FinishKind::Chamfer => "chamfer",
            FinishKind::Fillet => "fillet",
        }
    }

    /// JSON field holding the size.
    pub fn field(self) -> &'static str {
        match self {
            FinishKind::Chamfer => "distance",
            FinishKind::Fillet => "radius",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "chamfer" => Some(FinishKind::Chamfer),
            "fillet" => Some(FinishKind::Fillet),
            _ => None,
        }
    }
}

/// Chamfers or fillets one straight edge between two planar faces. Convex
/// edges are cut with a prism along the edge whose cross-section is the
/// material outside the chamfer line or fillet arc; concave edges get the
/// same prism added.
pub fn finish_edge(
    body: &Manifold,
    edge: &SolidEdge,
    kind: FinishKind,
    size: f32,
) -> Result<Manifold, String> {
    if !size.is_finite() || size <= 0.0 {
        return Err(format!("{} size must be positive", kind.name()));
    }
    if !edge.straight {
        return Err("only straight edges between planar faces can be finished".into());
    }
    let [n1, n2] = edge.normals;
    let [w1, w2] = edge.inward;
    let d12 = dot(n1, n2);
    if 1.0 + d12 < 0.05 || length(w1) < 0.5 || length(w2) < 0.5 {
        return Err("edge is too sharp to finish".into());
    }
    let direction = unit(sub(edge.b, edge.a)).ok_or("edge has no length")?;
    let edge_length = edge.length();
    let convex = edge.convex();
    let sign = if convex { 1.0 } else { -1.0 };
    let corner = edge.a;
    let eps = size * 0.05 + 1e-3;
    let bisector = scale(add(n1, n2), 1.0 / (1.0 + d12));
    let mut section = vec![add(corner, scale(bisector, sign * eps))];
    let (t1, t2, arc) = match kind {
        FinishKind::Chamfer => (
            add(corner, scale(w1, size)),
            add(corner, scale(w2, size)),
            Vec::new(),
        ),
        FinishKind::Fillet => {
            let center = sub(corner, scale(bisector, sign * size));
            let (u1, u2) = (scale(n1, sign), scale(n2, sign));
            let angle = dot(u1, u2).clamp(-1.0, 1.0).acos();
            let steps = ((16.0 * angle / std::f32::consts::FRAC_PI_2).ceil() as usize).max(2);
            let sin = angle.sin().max(1e-6);
            let arc = (1..steps)
                .map(|i| {
                    let t = i as f32 / steps as f32;
                    let u = add(
                        scale(u1, ((1.0 - t) * angle).sin() / sin),
                        scale(u2, (t * angle).sin() / sin),
                    );
                    add(center, scale(u, size))
                })
                .collect();
            (
                add(center, scale(u1, size)),
                add(center, scale(u2, size)),
                arc,
            )
        }
    };
    section.push(add(t1, scale(n1, sign * eps)));
    section.push(t1);
    section.extend(arc);
    section.push(t2);
    section.push(add(t2, scale(n2, sign * eps)));
    let extension = edge_length.max(size) * 1e-3;
    let frame = Frame {
        origin: sub(corner, scale(direction, extension)),
        x_axis: n1,
        y_axis: unit(cross3(direction, n1)).ok_or("edge normal is degenerate")?,
        normal: direction,
    };
    let profile: Vec<Point> = section.iter().map(|p| frame.local(*p).0).collect();
    let tool = crate::stl::manifold_from_solid(&SolidPreview {
        name: kind.name().into(),
        profile: orient_loop(profile, true),
        holes: Vec::new(),
        depth: edge_length + 2.0 * extension,
        frame,
        start: 0.0,
    })?;
    let result = if convex {
        body.difference(&tool)
    } else {
        body.union(&tool)
    };
    if result.status() != ManifoldError::NoError || result.is_empty() {
        return Err(format!("{} failed on this edge", kind.name()));
    }
    Ok(result)
}

fn read_point2(value: &Value) -> Option<Point> {
    let p = Point {
        x: value.get("x")?.as_f64()? as f32,
        y: value.get("y")?.as_f64()? as f32,
    };
    (p.x.is_finite() && p.y.is_finite()).then_some(p)
}

/// The kinds of graph nodes that are features, evaluated by
/// `ProjectFile::evaluate_feature`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureKind {
    Extrude,
    Revolve,
    Loft,
    EdgeFinish,
}

impl FeatureKind {
    pub fn from_node(node: &Value) -> Option<Self> {
        match node.get("type")?.as_str()? {
            "extrude" => Some(Self::Extrude),
            "revolve" => Some(Self::Revolve),
            "loft" => Some(Self::Loft),
            "edgeFinish" => Some(Self::EdgeFinish),
            _ => None,
        }
    }

    /// Native extension node types (not web graph nodes).
    pub fn is_extension(self) -> bool {
        self != Self::Extrude
    }

    pub fn type_name(self) -> &'static str {
        match self {
            Self::Extrude => "extrude",
            Self::Revolve => "revolve",
            Self::Loft => "loft",
            Self::EdgeFinish => "edgeFinish",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FeatureStatus {
    Active,
    Suppressed,
    /// After the rollback bar.
    RolledBack,
    Error(String),
}

impl FeatureStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active)
    }
}

/// Body the next cut/join applies to: the id of the nearest earlier new-body
/// feature and its index in the evaluated bodies (None when it is inactive).
type BodyTarget = Option<(String, Option<usize>)>;

#[derive(Clone)]
struct CacheEntry {
    key: String,
    status: FeatureStatus,
    preview: SolidPreview,
    bodies: Vec<Manifold>,
    body_ids: Vec<String>,
    target: BodyTarget,
}

/// Memoized per-feature evaluation; see the module docs.
#[derive(Default)]
pub(super) struct FeatureCache {
    entries: RefCell<Vec<CacheEntry>>,
    evaluations: Cell<usize>,
}

pub(super) struct TreeEvaluation {
    pub previews: Vec<SolidPreview>,
    pub statuses: Vec<FeatureStatus>,
    pub bodies: Vec<Manifold>,
}

fn placeholder(node: &Value) -> SolidPreview {
    SolidPreview {
        name: node
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Feature")
            .to_string(),
        profile: Vec::new(),
        holes: Vec::new(),
        depth: node.get("depth").and_then(Value::as_f64).unwrap_or(0.0) as f32,
        frame: SketchPlane::Top.frame(),
        start: 0.0,
    }
}

fn preview_key(preview: &SolidPreview) -> String {
    format!(
        "{:?}|{:?}|{:?}|{}",
        preview.profile, preview.holes, preview.frame, preview.depth
    )
}

impl ProjectFile {
    pub(super) fn feature_nodes(&self) -> Vec<&Value> {
        self.graph_nodes()
            .map(|nodes| {
                nodes
                    .iter()
                    .filter(|node| FeatureKind::from_node(node).is_some())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn feature_count(&self) -> usize {
        self.feature_nodes().len()
    }

    /// Rollback bar position: the number of features evaluated, or None
    /// when the bar is at the end.
    pub fn rollback_index(&self) -> Option<usize> {
        let count = self.feature_count();
        self.value
            .pointer(&format!(
                "/items/{}/data/tree/rollbackIndex",
                self.part_index
            ))
            .and_then(Value::as_u64)
            .map(|index| index as usize)
            .filter(|&index| index < count)
    }

    pub fn set_rollback_index(&mut self, index: Option<usize>) -> Result<(), String> {
        let count = self.feature_count();
        let tree = self
            .value
            .pointer_mut(&format!("/items/{}/data/tree", self.part_index))
            .and_then(Value::as_object_mut)
            .ok_or("project tree is invalid")?;
        match index.filter(|&index| index < count) {
            Some(index) => {
                tree.insert("rollbackIndex".into(), json!(index));
            }
            None => {
                tree.remove("rollbackIndex");
            }
        }
        Ok(())
    }

    pub fn feature_suppressed(&self, index: usize) -> bool {
        self.feature_nodes()
            .get(index)
            .is_some_and(|node| node.get("suppressed").and_then(Value::as_bool) == Some(true))
    }

    pub fn set_feature_suppressed(&mut self, index: usize, suppressed: bool) -> Result<(), String> {
        let node = self
            .feature_node_mut(index)
            .ok_or("feature node is missing")?;
        let object = node.as_object_mut().ok_or("feature node is invalid")?;
        if suppressed {
            object.insert("suppressed".into(), json!(true));
        } else {
            object.remove("suppressed");
        }
        Ok(())
    }

    fn feature_node_mut(&mut self, index: usize) -> Option<&mut Value> {
        self.value
            .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))?
            .as_array_mut()?
            .iter_mut()
            .filter(|node| FeatureKind::from_node(node).is_some())
            .nth(index)
    }

    /// Status of every feature, in order.
    pub fn feature_statuses(&self) -> Vec<FeatureStatus> {
        self.evaluate_tree()
            .map(|tree| tree.statuses)
            .unwrap_or_default()
    }

    /// Previews of the features that contribute to the evaluated part.
    pub fn active_extrusion_previews(&self) -> Option<Vec<SolidPreview>> {
        let tree = self.evaluate_tree()?;
        Some(
            tree.previews
                .into_iter()
                .zip(tree.statuses)
                .filter(|(_, status)| status.is_active())
                .map(|(preview, _)| preview)
                .collect(),
        )
    }

    /// Number of active features in error; None when the tree does not
    /// evaluate at all.
    pub(super) fn feature_error_count(&self) -> Option<usize> {
        Some(
            self.evaluate_tree()?
                .statuses
                .iter()
                .filter(|status| matches!(status, FeatureStatus::Error(_)))
                .count(),
        )
    }

    /// True when the tree evaluates with no more errors than `before`.
    pub(super) fn no_new_errors(&self, before: Option<usize>) -> bool {
        self.feature_error_count()
            .is_some_and(|after| after <= before.unwrap_or(0))
    }

    /// How many features have been (re)built since the project was opened.
    pub fn feature_evaluations(&self) -> usize {
        self.feature_cache.evaluations.get()
    }

    /// Ids a feature must come after, given the feature order `nodes`:
    /// explicit references plus the body a cut/join implicitly applies to.
    fn feature_dependencies(&self, nodes: &[&Value], index: usize) -> Vec<String> {
        let node = nodes[index];
        let mut deps = Vec::new();
        if matches!(node["operation"].as_str(), Some("cut" | "join"))
            && let Some(body) = nodes[..index]
                .iter()
                .rev()
                .find(|node| node["operation"] == "newBody")
                .and_then(|node| node["id"].as_str())
        {
            deps.push(body.to_string());
        }
        if let Ok(ExtrudeOptions {
            end: EndCondition::UpToBody(target),
            ..
        }) = extrude_options(node)
        {
            deps.push(target);
        }
        if let Some(face) = node["sketchId"]
            .as_str()
            .and_then(|id| self.sketches.iter().find(|sketch| sketch.id == id))
            .and_then(|sketch| sketch.face.as_ref())
        {
            deps.push(face.extrude_id.clone());
        }
        for key in ["sourceId", "targetId"] {
            if let Some(id) = node.get(key).and_then(Value::as_str) {
                deps.push(id.to_string());
            }
        }
        deps
    }

    /// Move feature `index` by `delta` places. Refused when a feature would
    /// come before something it depends on, or a cut/join would change the
    /// body it applies to. Returns the new index.
    pub fn move_feature(&mut self, index: usize, delta: isize) -> Result<usize, String> {
        let nodes = self.feature_nodes();
        let target = index as isize + delta;
        if index >= nodes.len() || target < 0 || target as usize >= nodes.len() {
            return Err("feature cannot move further".into());
        }
        let target = target as usize;
        let mut order: Vec<&Value> = nodes.clone();
        let moved = order.remove(index);
        order.insert(target, moved);
        for (position, node) in order.iter().enumerate() {
            let before: Vec<&str> = order[..position]
                .iter()
                .filter_map(|node| node["id"].as_str())
                .collect();
            let old_position = nodes
                .iter()
                .position(|candidate| std::ptr::eq(*candidate, *node))
                .expect("same nodes");
            let old_deps = self.feature_dependencies(&nodes, old_position);
            let new_deps = self.feature_dependencies(&order, position);
            let name = node["name"].as_str().unwrap_or("feature");
            if let Some(missing) = new_deps.iter().find(|id| !before.contains(&id.as_str())) {
                return Err(format!("{name} depends on {missing}"));
            }
            if old_deps.first() != new_deps.first()
                && matches!(node["operation"].as_str(), Some("cut" | "join"))
            {
                return Err(format!("{name} would apply to a different body"));
            }
        }
        let ids: Vec<String> = order
            .iter()
            .filter_map(|node| node["id"].as_str().map(str::to_string))
            .collect();
        let errors = self.feature_error_count();
        let snapshot = self.feature_tree_snapshot();
        self.apply_feature_order(&ids)?;
        if !self.no_new_errors(errors) {
            self.restore_feature_tree(snapshot)?;
            return Err("the reordered features would not evaluate".into());
        }
        Ok(target)
    }

    fn apply_feature_order(&mut self, ids: &[String]) -> Result<(), String> {
        let data = self
            .value
            .pointer_mut(&format!("/items/{}/data", self.part_index))
            .ok_or("part data is missing")?;
        let nodes = data["cad"]["nodes"]
            .as_array_mut()
            .ok_or("project CAD graph is invalid")?;
        let slots: Vec<usize> = (0..nodes.len())
            .filter(|&i| FeatureKind::from_node(&nodes[i]).is_some())
            .collect();
        let mut taken: Vec<Value> = slots.iter().map(|&i| nodes[i].clone()).collect();
        for (slot, id) in slots.iter().zip(ids) {
            let position = taken
                .iter()
                .position(|node| node["id"].as_str() == Some(id))
                .ok_or("feature order is inconsistent")?;
            nodes[*slot] = taken.remove(position);
        }
        for path in ["/cad/rootNodeIds", "/tree/orderedNodeIds"] {
            let Some(list) = data.pointer_mut(path).and_then(Value::as_array_mut) else {
                continue;
            };
            let slots: Vec<usize> = (0..list.len())
                .filter(|&i| {
                    list[i]
                        .as_str()
                        .is_some_and(|entry| ids.iter().any(|id| id == entry))
                })
                .collect();
            let present: Vec<&String> = ids
                .iter()
                .filter(|id| slots.iter().any(|&i| list[i].as_str() == Some(id.as_str())))
                .collect();
            for (slot, id) in slots.iter().zip(present) {
                list[*slot] = json!(id);
            }
        }
        Ok(())
    }

    /// Graph nodes, roots and tree of the active part, for undo.
    pub fn feature_tree_snapshot(&self) -> Value {
        let data = &self.value["items"][self.part_index]["data"];
        json!({
            "nodes": data["cad"]["nodes"].clone(),
            "roots": data["cad"]["rootNodeIds"].clone(),
            "tree": data["tree"].clone(),
        })
    }

    pub fn restore_feature_tree(&mut self, snapshot: Value) -> Result<(), String> {
        let data = self
            .value
            .pointer_mut(&format!("/items/{}/data", self.part_index))
            .ok_or("part data is missing")?;
        if !snapshot["nodes"].is_array() {
            return Err("feature snapshot is invalid".into());
        }
        data["cad"]["nodes"] = snapshot["nodes"].clone();
        data["cad"]["rootNodeIds"] = snapshot["roots"].clone();
        data["tree"] = snapshot["tree"].clone();
        Ok(())
    }

    /// Copy the open sketch into its workspace so the next evaluation sees
    /// edits made since the last switch (re-evaluates dependent features).
    pub fn sync_active_document(&mut self, current: &Document) {
        self.sketches[self.active_sketch].document = current.clone();
    }

    /// Evaluate every feature in order. None when the part has no features
    /// or contains nodes the native evaluator does not support.
    pub(super) fn evaluate_tree(&self) -> Option<TreeEvaluation> {
        let nodes = self.graph_nodes()?;
        if nodes
            .iter()
            .any(|node| node.get("type").and_then(Value::as_str) == Some("chamfer"))
        {
            return None;
        }
        let features = self.feature_nodes();
        if features.is_empty()
            || features.iter().any(|node| {
                FeatureKind::from_node(node) == Some(FeatureKind::Extrude)
                    && !matches!(
                        node.get("operation").and_then(Value::as_str),
                        Some("newBody" | "cut" | "join")
                    )
            })
        {
            return None;
        }
        let rollback = self.rollback_index().unwrap_or(features.len());
        let mut cache = self.feature_cache.entries.borrow_mut();
        let mut bodies: Vec<Manifold> = Vec::new();
        let mut body_ids: Vec<String> = Vec::new();
        let mut target: BodyTarget = None;
        let mut fresh = true;
        let mut out = TreeEvaluation {
            previews: Vec::new(),
            statuses: Vec::new(),
            bodies: Vec::new(),
        };
        for (index, node) in features.iter().enumerate() {
            let kind = FeatureKind::from_node(node).unwrap_or(FeatureKind::Extrude);
            let preview = match kind {
                FeatureKind::Extrude => self.preview_for_extrude(node),
                _ => Some(placeholder(node)),
            };
            let inputs = match kind {
                FeatureKind::Extrude => preview.as_ref().map(preview_key).unwrap_or_default(),
                _ => self.extension_inputs_key(node),
            };
            let key = format!("{}|{}|{}", index < rollback, node, inputs);
            if fresh
                && let Some(entry) = cache.get(index)
                && entry.key == key
            {
                bodies.clone_from(&entry.bodies);
                body_ids.clone_from(&entry.body_ids);
                target.clone_from(&entry.target);
                out.previews.push(entry.preview.clone());
                out.statuses.push(entry.status.clone());
                continue;
            }
            fresh = false;
            cache.truncate(index);
            self.feature_cache
                .evaluations
                .set(self.feature_cache.evaluations.get() + 1);
            let (status, preview) = self.evaluate_feature(
                node,
                preview,
                index < rollback,
                &out.statuses,
                &features,
                &mut bodies,
                &mut body_ids,
                &mut target,
            );
            cache.push(CacheEntry {
                key,
                status: status.clone(),
                preview: preview.clone(),
                bodies: bodies.clone(),
                body_ids: body_ids.clone(),
                target: target.clone(),
            });
            out.previews.push(preview);
            out.statuses.push(status);
        }
        cache.truncate(features.len());
        out.bodies = bodies;
        Some(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn evaluate_feature(
        &self,
        node: &Value,
        preview: Option<SolidPreview>,
        before_rollback: bool,
        statuses: &[FeatureStatus],
        features: &[&Value],
        bodies: &mut Vec<Manifold>,
        body_ids: &mut Vec<String>,
        target: &mut BodyTarget,
    ) -> (FeatureStatus, SolidPreview) {
        let kind = FeatureKind::from_node(node).unwrap_or(FeatureKind::Extrude);
        let operation = node["operation"].as_str().unwrap_or_default();
        let id = node["id"].as_str().unwrap_or_default().to_string();
        if operation == "newBody" {
            // Later cuts/joins apply to this body even when it is inactive.
            *target = Some((id.clone(), None));
        }
        let fallback = || placeholder(node);
        if !before_rollback {
            return (FeatureStatus::RolledBack, preview.unwrap_or_else(fallback));
        }
        if node.get("suppressed").and_then(Value::as_bool) == Some(true) {
            return (FeatureStatus::Suppressed, preview.unwrap_or_else(fallback));
        }
        let error = |reason: &str, preview: Option<SolidPreview>| {
            (
                FeatureStatus::Error(reason.to_string()),
                preview.unwrap_or_else(fallback),
            )
        };
        if kind.is_extension() {
            let preview = preview.unwrap_or_else(fallback);
            return match self.apply_extension_feature(node, kind, bodies, body_ids, target) {
                Ok(()) => (FeatureStatus::Active, preview),
                Err(reason) => (FeatureStatus::Error(reason), preview),
            };
        }
        let Some(mut preview) = preview else {
            return error("sketch or profile is missing", None);
        };
        if let Some(face) = node["sketchId"]
            .as_str()
            .and_then(|id| self.sketches.iter().find(|sketch| sketch.id == id))
            .and_then(|sketch| sketch.face.as_ref())
        {
            let source = features
                .iter()
                .position(|node| node["id"].as_str() == Some(&face.extrude_id));
            if source.is_none_or(|source| statuses.get(source).is_none_or(|s| !s.is_active())) {
                return error("the sketch face's feature is not active", Some(preview));
            }
        }
        let mut options = match extrude_options(node) {
            Ok(options) => options,
            Err(reason) => return error(&reason, Some(preview)),
        };
        // A cut from a face sketch runs into the body (against the face
        // normal) by default; new bodies and joins grow outward. Flip
        // inverts that default.
        if operation == "cut" && self.is_face_sketch(node) {
            options.flip = !options.flip;
        }
        let body = match operation {
            "newBody" => None,
            _ => match target {
                Some((_, Some(body))) => Some(*body),
                Some((_, None)) => return error("the target body is not active", Some(preview)),
                None => return error("there is no earlier body", Some(preview)),
            },
        };
        if resolve_extent(&mut preview, &options, bodies, body_ids).is_none() {
            return error("the end condition does not reach a body", Some(preview));
        }
        let Ok(tool) = crate::stl::manifold_from_solid(&preview) else {
            return error("the profile does not form a valid solid", Some(preview));
        };
        match body {
            None => {
                bodies.push(tool);
                body_ids.push(id.clone());
                *target = Some((id, Some(bodies.len() - 1)));
            }
            Some(body) => {
                let result = if operation == "cut" {
                    bodies[body].difference(&tool)
                } else {
                    bodies[body].union(&tool)
                };
                if result.status() != ManifoldError::NoError || result.is_empty() {
                    return error("the Boolean result is empty or invalid", Some(preview));
                }
                bodies[body] = result;
            }
        }
        (FeatureStatus::Active, preview)
    }
}

/// Native extension feature summary for the tree and Properties panel.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureInfo {
    pub id: String,
    pub name: String,
    pub kind: &'static str,
    pub operation: String,
    /// Editable value label (ANGLE, RADIUS, DISTANCE) and unit.
    pub value_label: Option<(&'static str, &'static str)>,
    pub value: f32,
}

impl ProjectFile {
    /// True when the part has revolve, loft or edge finish nodes.
    pub fn has_extension_features(&self) -> bool {
        self.feature_nodes()
            .iter()
            .any(|node| FeatureKind::from_node(node).is_some_and(FeatureKind::is_extension))
    }

    /// Feature nodes (every `FeatureKind`), in graph order.
    pub(super) fn solid_nodes(&self) -> Vec<&Value> {
        self.feature_nodes()
    }

    /// Cache key part for an extension feature's sketch inputs.
    fn extension_inputs_key(&self, node: &Value) -> String {
        let profile = |node: &Value| {
            self.profile_ref(node)
                .map(|(loops, frame)| format!("{:?}|{:?}|{:?}", loops.outer, loops.holes, frame))
                .unwrap_or_default()
        };
        match FeatureKind::from_node(node) {
            Some(FeatureKind::Revolve) => profile(node),
            Some(FeatureKind::Loft) => node["profiles"]
                .as_array()
                .map(|profiles| profiles.iter().map(profile).collect::<Vec<_>>().join("/"))
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn sketch_loops(
        &self,
        sketch_id: &str,
        profile: usize,
    ) -> Result<(ProfileLoops, Frame), String> {
        let workspace = self
            .sketches
            .iter()
            .find(|sketch| sketch.id == sketch_id)
            .ok_or_else(|| format!("sketch \"{sketch_id}\" does not exist"))?;
        let loops = profiles_from_document(&workspace.document)
            .and_then(|profiles| profiles.into_iter().nth(profile))
            .ok_or("profile is not a closed region")?;
        let frame = self
            .sketch_frame(sketch_id)
            .ok_or("sketch plane does not resolve")?;
        Ok((loops, frame))
    }

    fn profile_ref(&self, node: &Value) -> Result<(ProfileLoops, Frame), String> {
        let sketch_id = node
            .get("sketchId")
            .and_then(Value::as_str)
            .ok_or("feature has no sketch")?;
        let index = face::profile_index(node, sketch_id).ok_or("feature has no valid profile")?;
        self.sketch_loops(sketch_id, index)
    }

    fn revolve_tool(&self, node: &Value) -> Result<Vec<Tri>, String> {
        let (loops, frame) = self.profile_ref(node)?;
        let axis = node.get("axis").ok_or("revolve has no axis")?;
        let (Some(a), Some(b)) = (
            axis.get("a").and_then(read_point2),
            axis.get("b").and_then(read_point2),
        ) else {
            return Err("revolve axis is invalid".into());
        };
        let angle = node
            .get("angle")
            .and_then(Value::as_f64)
            .ok_or("revolve has no angle")? as f32;
        revolve_triangles(&loops, &frame, (a, b), angle)
    }

    fn loft_tool(&self, node: &Value) -> Result<Vec<Tri>, String> {
        let profiles = node
            .get("profiles")
            .and_then(Value::as_array)
            .filter(|p| p.len() == 2)
            .ok_or("loft needs exactly two profiles")?;
        let (first, first_frame) = self.profile_ref(&profiles[0])?;
        let (second, second_frame) = self.profile_ref(&profiles[1])?;
        if !first.holes.is_empty() || !second.holes.is_empty() {
            return Err("loft profiles cannot have holes".into());
        }
        loft_triangles(&first.outer, &first_frame, &second.outer, &second_frame)
    }

    /// Applies one revolve, loft or edge finish node to the bodies. Cuts and
    /// joins apply to the tree's current target body (the nearest earlier
    /// new body), like extrusions.
    pub(super) fn apply_extension_feature(
        &self,
        node: &Value,
        kind: FeatureKind,
        bodies: &mut Vec<Manifold>,
        body_ids: &mut Vec<String>,
        target: &mut BodyTarget,
    ) -> Result<(), String> {
        let name = kind.type_name();
        let id = node
            .get("id")
            .and_then(Value::as_str)
            .ok_or("feature has no id")?;
        if kind == FeatureKind::EdgeFinish {
            return self.apply_edge_finish(node, bodies);
        }
        let operation = node
            .get("operation")
            .and_then(Value::as_str)
            .ok_or("feature has no operation")?;
        let allowed: &[&str] = if kind == FeatureKind::Loft {
            &["newBody", "join"]
        } else {
            &["newBody", "cut", "join"]
        };
        if !allowed.contains(&operation) {
            return Err(format!("unsupported {name} operation \"{operation}\""));
        }
        let triangles = if kind == FeatureKind::Revolve {
            self.revolve_tool(node)?
        } else {
            self.loft_tool(node)?
        };
        let tool = manifold_from_triangles(&triangles, name)?;
        if kind == FeatureKind::Loft && tool.has_self_intersections() {
            return Err("loft surface intersects itself".into());
        }
        if operation == "newBody" {
            bodies.push(tool);
            body_ids.push(id.to_string());
            *target = Some((id.to_string(), Some(bodies.len() - 1)));
            return Ok(());
        }
        let body = match target {
            Some((_, Some(body))) => *body,
            Some((_, None)) => return Err("the target body is not active".into()),
            None => return Err("cut and join require an existing solid".into()),
        };
        let result = if operation == "cut" {
            bodies[body].difference(&tool)
        } else {
            bodies[body].union(&tool)
        };
        if result.status() != ManifoldError::NoError || result.is_empty() {
            return Err(format!("{name} {operation} leaves no valid solid"));
        }
        bodies[body] = result;
        Ok(())
    }

    fn apply_edge_finish(&self, node: &Value, bodies: &mut [Manifold]) -> Result<(), String> {
        let kind = node
            .get("kind")
            .and_then(Value::as_str)
            .and_then(FinishKind::parse)
            .ok_or("edge finish kind must be chamfer or fillet")?;
        let size = node
            .get(kind.field())
            .and_then(Value::as_f64)
            .map(|v| v as f32)
            .filter(|v| v.is_finite() && *v > 0.0)
            .ok_or_else(|| format!("{} needs a positive {}", kind.name(), kind.field()))?;
        let edges: Vec<EdgeRef> = node
            .get("edges")
            .and_then(Value::as_array)
            .filter(|edges| !edges.is_empty())
            .ok_or("edge finish has no edges")?
            .iter()
            .map(EdgeRef::from_json)
            .collect::<Option<_>>()
            .ok_or("edge finish edge reference is invalid")?;
        let body_index = (0..bodies.len())
            .rev()
            .find(|&index| {
                let candidates = sharp_edges(&bodies[index]);
                match_edge(&candidates, &edges[0], edge_tolerance(&bodies[index])).is_some()
            })
            .ok_or("finished edge no longer exists")?;
        let mut body = bodies[body_index].clone();
        for edge in &edges {
            let candidates = sharp_edges(&body);
            let found = match_edge(&candidates, edge, edge_tolerance(&body))
                .ok_or("finished edge no longer exists")?;
            body = finish_edge(&body, found, kind, size)?;
        }
        bodies[body_index] = body;
        Ok(())
    }

    /// Sharp edges of every evaluated body, as (body index, edge).
    pub fn solid_edges(&self) -> Vec<(usize, SolidEdge)> {
        self.evaluated_solids()
            .unwrap_or_default()
            .iter()
            .enumerate()
            .flat_map(|(index, body)| sharp_edges(body).into_iter().map(move |e| (index, e)))
            .collect()
    }

    /// Revolve, loft and edge finish nodes in graph order.
    pub fn extension_features(&self) -> Vec<FeatureInfo> {
        self.solid_nodes()
            .into_iter()
            .filter_map(|node| {
                let kind = match node.get("type")?.as_str()? {
                    "revolve" => "revolve",
                    "loft" => "loft",
                    "edgeFinish" => "edgeFinish",
                    _ => return None,
                };
                let finish = node
                    .get("kind")
                    .and_then(Value::as_str)
                    .and_then(FinishKind::parse);
                let (value_label, value) = match (kind, finish) {
                    ("revolve", _) => (
                        Some(("ANGLE", "deg")),
                        node.get("angle").and_then(Value::as_f64).unwrap_or(0.0),
                    ),
                    ("edgeFinish", Some(FinishKind::Chamfer)) => (
                        Some(("DISTANCE", "mm")),
                        node.get("distance").and_then(Value::as_f64).unwrap_or(0.0),
                    ),
                    ("edgeFinish", Some(FinishKind::Fillet)) => (
                        Some(("RADIUS", "mm")),
                        node.get("radius").and_then(Value::as_f64).unwrap_or(0.0),
                    ),
                    _ => (None, 0.0),
                };
                Some(FeatureInfo {
                    id: node.get("id")?.as_str()?.to_string(),
                    name: node
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(kind)
                        .to_string(),
                    kind,
                    operation: match finish {
                        Some(finish) => finish.name().to_string(),
                        None => node
                            .get("operation")
                            .and_then(Value::as_str)
                            .unwrap_or("newBody")
                            .to_string(),
                    },
                    value_label,
                    value: value as f32,
                })
            })
            .collect()
    }

    fn nodes_mut(&mut self) -> Result<&mut Vec<Value>, String> {
        self.value
            .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array_mut)
            .ok_or_else(|| "project CAD graph changed unexpectedly".to_string())
    }

    fn check_writable(&self) -> Result<(), String> {
        if let Some(reason) = &self.read_only_reason {
            return Err(format!("project is read-only: {reason}"));
        }
        let data = &self.value["items"][self.part_index]["data"];
        if data["features"].as_array().is_some_and(|f| !f.is_empty()) {
            return Err("existing solid features need native support".into());
        }
        if !data["cad"]["rootNodeIds"].is_array() || !data["tree"]["orderedNodeIds"].is_array() {
            return Err("project graph or tree is invalid".into());
        }
        Ok(())
    }

    fn fresh_id(&self, prefix: &str) -> (String, usize) {
        let nodes = self.graph_nodes().cloned().unwrap_or_default();
        let mut index = 1;
        loop {
            let candidate = format!("{prefix}-{index}");
            if !nodes.iter().any(|node| node["id"] == candidate.as_str()) {
                return (candidate, index);
            }
            index += 1;
        }
    }

    /// Appends a node and registers it, or leaves the graph unchanged and
    /// returns why it does not evaluate.
    fn push_solid_node(&mut self, node: Value) -> Result<String, String> {
        let id = node["id"].as_str().unwrap_or_default().to_string();
        // A new feature goes after the rollback bar's features: move the bar
        // to the end so it is evaluated.
        let snapshot = self.feature_tree_snapshot();
        self.nodes_mut()?.push(node);
        self.set_rollback_index(None)?;
        if let Some(reason) = self.feature_error(&id) {
            self.restore_feature_tree(snapshot)?;
            return Err(reason);
        }
        for path in ["cad/rootNodeIds", "tree/orderedNodeIds"] {
            self.value
                .pointer_mut(&format!("/items/{}/data/{path}", self.part_index))
                .and_then(Value::as_array_mut)
                .ok_or("project tree is invalid")?
                .push(json!(id));
        }
        Ok(id)
    }

    fn revolve_node(
        &self,
        profile_index: usize,
        axis: (Point, Point),
        angle: f32,
        operation: &str,
    ) -> Value {
        let sketch_id = &self.sketches[self.active_sketch].id;
        let (id, index) = self.fresh_id("native-revolve");
        json!({
            "id": id, "type": "revolve", "name": format!("Revolve {index}"),
            "sketchId": sketch_id, "profileId": format!("{sketch_id}-profile-{}", profile_index + 1),
            "axis": {"a": {"x": axis.0.x, "y": axis.0.y}, "b": {"x": axis.1.x, "y": axis.1.y}},
            "angle": angle, "operation": operation
        })
    }

    fn check_operation(&self, operation: &str, allowed: &[&str]) -> Result<(), String> {
        if !allowed.contains(&operation) {
            return Err(format!("unsupported operation \"{operation}\""));
        }
        if operation != "newBody" && self.evaluated_solids().is_none_or(|b| b.is_empty()) {
            return Err("cut and join require an existing solid".into());
        }
        Ok(())
    }

    /// Revolves a profile of the active sketch around `axis` (sketch coords).
    pub fn create_revolve(
        &mut self,
        current: &Document,
        profile_index: usize,
        axis: (Point, Point),
        angle: f32,
        operation: &str,
    ) -> Result<String, String> {
        self.check_writable()?;
        self.check_operation(operation, &["newBody", "cut", "join"])?;
        let previous = std::mem::replace(
            &mut self.sketches[self.active_sketch].document,
            current.clone(),
        );
        let node = self.revolve_node(profile_index, axis, angle, operation);
        let result = self.push_solid_node(node);
        if result.is_err() {
            self.sketches[self.active_sketch].document = previous;
        }
        result
    }

    /// The would-be revolve surface, without changing the project.
    pub fn ghost_revolve(
        &self,
        current: &Document,
        profile_index: usize,
        axis: (Point, Point),
        angle: f32,
    ) -> Result<Vec<Tri>, String> {
        let workspace = &self.sketches[self.active_sketch];
        let loops = profiles_from_document(current)
            .and_then(|profiles| profiles.into_iter().nth(profile_index))
            .ok_or("select a closed profile")?;
        let frame = self
            .sketch_frame(&workspace.id)
            .ok_or("sketch plane does not resolve")?;
        revolve_triangles(&loops, &frame, axis, angle)
    }

    /// Lofts from profile `from.1` of sketch `from.0` to profile
    /// `profile_index` of the active sketch.
    pub fn create_loft(
        &mut self,
        current: &Document,
        from: (usize, usize),
        profile_index: usize,
        operation: &str,
    ) -> Result<String, String> {
        self.check_writable()?;
        self.check_operation(operation, &["newBody", "join"])?;
        if from.0 == self.active_sketch {
            return Err("pick the second profile on a different sketch".into());
        }
        let first = self
            .sketches
            .get(from.0)
            .ok_or("first loft sketch no longer exists")?
            .id
            .clone();
        let second = self.sketches[self.active_sketch].id.clone();
        let previous = std::mem::replace(
            &mut self.sketches[self.active_sketch].document,
            current.clone(),
        );
        let (id, index) = self.fresh_id("native-loft");
        let node = json!({
            "id": id, "type": "loft", "name": format!("Loft {index}"),
            "profiles": [
                {"sketchId": first, "profileId": format!("{first}-profile-{}", from.1 + 1)},
                {"sketchId": second, "profileId": format!("{second}-profile-{}", profile_index + 1)}
            ],
            "operation": operation
        });
        let result = self.push_solid_node(node);
        if result.is_err() {
            self.sketches[self.active_sketch].document = previous;
        }
        result
    }

    /// Chamfers or fillets `edges` (all on one body) by `size` mm.
    pub fn create_edge_finish(
        &mut self,
        kind: FinishKind,
        size: f32,
        edges: &[EdgeRef],
    ) -> Result<String, String> {
        self.check_writable()?;
        if edges.is_empty() {
            return Err("pick at least one edge".into());
        }
        if !size.is_finite() || size <= 0.0 {
            return Err(format!("{} must be positive", kind.field()));
        }
        let (id, index) = self.fresh_id("native-edge-finish");
        let label = match kind {
            FinishKind::Chamfer => "Chamfer",
            FinishKind::Fillet => "Fillet",
        };
        let mut node = json!({
            "id": id, "type": "edgeFinish", "name": format!("{label} {index}"),
            "kind": kind.name(),
            "edges": edges.iter().map(EdgeRef::to_json).collect::<Vec<_>>()
        });
        node[kind.field()] = json!(size);
        self.push_solid_node(node)
    }

    /// Copy of a solid node by id, for undo.
    pub fn solid_node(&self, id: &str) -> Option<Value> {
        self.solid_nodes()
            .into_iter()
            .find(|node| node["id"] == id)
            .cloned()
    }

    /// Replaces the node with the same id; unchanged when it would not
    /// evaluate.
    pub fn restore_solid_node(&mut self, node: Value) -> Result<(), String> {
        let id = node["id"].as_str().ok_or("node has no id")?.to_string();
        let nodes = self.nodes_mut()?;
        let slot = nodes
            .iter_mut()
            .find(|candidate| candidate["id"] == id.as_str())
            .ok_or("feature node is missing")?;
        let old = std::mem::replace(slot, node);
        if let Some(error) = self.feature_error(&id) {
            *self
                .nodes_mut()?
                .iter_mut()
                .find(|candidate| candidate["id"] == id.as_str())
                .expect("node replaced above") = old;
            return Err(error);
        }
        Ok(())
    }

    /// Sets a revolve angle or an edge finish size.
    pub fn set_feature_value(&mut self, id: &str, value: f32) -> Result<(), String> {
        if !value.is_finite() || value <= 0.0 {
            return Err("value must be positive".into());
        }
        let mut node = self.solid_node(id).ok_or("feature node is missing")?;
        let field = match node["type"].as_str() {
            Some("revolve") => {
                if value > 360.0 {
                    return Err("revolve angle must be at most 360 degrees".into());
                }
                "angle"
            }
            Some("edgeFinish") => match node["kind"].as_str().and_then(FinishKind::parse) {
                Some(kind) => kind.field(),
                None => return Err("edge finish kind is invalid".into()),
            },
            _ => return Err("this feature has no editable value".into()),
        };
        node[field] = json!(value);
        self.restore_solid_node(node)
    }

    /// Removes a revolve, loft or edge finish node when nothing after it
    /// stops evaluating.
    pub fn delete_feature(&mut self, id: &str) -> Result<(), String> {
        self.check_writable()?;
        let nodes = self.nodes_mut()?;
        let position = nodes
            .iter()
            .position(|node| {
                node["id"] == id
                    && node["type"]
                        .as_str()
                        .is_some_and(|t| EXTENSION_NODE_TYPES.contains(&t))
            })
            .ok_or("feature node is missing")?;
        let errors = self.feature_error_count();
        let rollback = self.rollback_index();
        let feature_position = self
            .feature_nodes()
            .iter()
            .position(|node| node["id"] == id)
            .ok_or("feature node is missing")?;
        let node = self.nodes_mut()?.remove(position);
        if self.has_solid_nodes() && !self.no_new_errors(errors) {
            self.nodes_mut()?.insert(position, node);
            return Err("later features depend on this feature".into());
        }
        // Keep the rollback bar below the same features.
        if let Some(bar) = rollback {
            self.set_rollback_index(Some(if feature_position < bar { bar - 1 } else { bar }))?;
        }
        for path in ["cad/rootNodeIds", "tree/orderedNodeIds"] {
            if let Some(list) = self
                .value
                .pointer_mut(&format!("/items/{}/data/{path}", self.part_index))
                .and_then(Value::as_array_mut)
            {
                list.retain(|entry| entry.as_str() != Some(id));
            }
        }
        Ok(())
    }

    pub(super) fn has_solid_nodes(&self) -> bool {
        !self.solid_nodes().is_empty()
    }

    /// Why feature `id` does not evaluate (None when it is active, or
    /// suppressed/rolled back). A tree that does not evaluate at all counts
    /// as an error.
    fn feature_error(&self, id: &str) -> Option<String> {
        let Some(tree) = self.evaluate_tree() else {
            return Some("the part does not evaluate".into());
        };
        let index = self
            .feature_nodes()
            .iter()
            .position(|node| node["id"] == id)?;
        match tree.statuses.get(index) {
            Some(FeatureStatus::Error(reason)) => Some(reason.clone()),
            _ => None,
        }
    }

    /// Evaluate the tree; an error when it does not evaluate or a feature is
    /// in error ("<name>: <reason>"). Used to decide read-only on open.
    pub(super) fn evaluate_solids(&self) -> Result<(Vec<SolidPreview>, Vec<Manifold>), String> {
        if self.graph_nodes().is_some_and(|nodes| {
            nodes
                .iter()
                .any(|node| node.get("type").and_then(Value::as_str) == Some("chamfer"))
        }) {
            return Err("web edge chamfers need native support".into());
        }
        let tree = self
            .evaluate_tree()
            .ok_or("part has no supported solid features")?;
        let nodes = self.feature_nodes();
        if let Some((index, FeatureStatus::Error(reason))) = tree
            .statuses
            .iter()
            .enumerate()
            .find(|(_, status)| matches!(status, FeatureStatus::Error(_)))
        {
            let name = nodes
                .get(index)
                .and_then(|node| node["name"].as_str())
                .unwrap_or("feature");
            return Err(format!("{name}: {reason}"));
        }
        Ok((tree.previews, tree.bodies))
    }
}

impl ProjectFile {
    /// Feature `index` (tree order), cloned.
    pub fn feature_node(&self, index: usize) -> Option<Value> {
        self.feature_nodes().get(index).map(|node| (*node).clone())
    }

    /// Row label for feature `index`: the extrusion operation, or the kind
    /// (with the operation) for revolve, loft and edge finish features.
    pub fn feature_label(&self, index: usize) -> String {
        let Some(node) = self.feature_nodes().get(index).copied() else {
            return "feature".into();
        };
        let operation = node["operation"].as_str().unwrap_or("newBody");
        match FeatureKind::from_node(node) {
            Some(FeatureKind::Revolve) => format!("revolve {operation}"),
            Some(FeatureKind::Loft) => format!("loft {operation}"),
            Some(FeatureKind::EdgeFinish) => node["kind"].as_str().unwrap_or("finish").to_string(),
            _ => operation.to_string(),
        }
    }

    /// Tree index of the feature with this node id.
    pub fn feature_index(&self, id: &str) -> Option<usize> {
        self.feature_nodes()
            .iter()
            .position(|node| node["id"] == id)
    }
}
