//! Revolve, loft and edge fillet/chamfer features, evaluated with
//! `manifold-rust`.
//!
//! None of these exist as web graph nodes (the web has a `chamfer` node that
//! references web topology edge ids, which the native app cannot resolve), so
//! they are stored as native extension nodes the web loader ignores:
//!
//! * `{"type":"revolve", sketchId, profileId, axis:{a:{x,y}, b:{x,y}}, angle, operation}`
//!   with the axis in sketch coordinates and the angle in degrees (0, 360].
//! * `{"type":"loft", profiles:[{sketchId, profileId}, {sketchId, profileId}], operation}`
//! * `{"type":"edgeFinish", kind:"chamfer"|"fillet", distance|radius, edges:[{a:[x,y,z], b:[x,y,z], normals:[[..],[..]]}]}`
//!   Edges are identified by their world endpoints and the outward normals of
//!   the two faces that meet there, and re-identified on every evaluation.

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
    pub(super) fn solid_nodes(&self) -> Vec<&Value> {
        self.graph_nodes()
            .map(|nodes| {
                nodes
                    .iter()
                    .filter(|node| {
                        node.get("type")
                            .and_then(Value::as_str)
                            .is_some_and(|t| SOLID_NODE_TYPES.contains(&t))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// True when the part has revolve, loft or edge finish nodes.
    pub fn has_extension_features(&self) -> bool {
        self.graph_nodes().is_some_and(|nodes| {
            nodes.iter().any(|node| {
                node.get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|t| EXTENSION_NODE_TYPES.contains(&t))
            })
        })
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

    /// Applies one revolve, loft or edge finish node to the bodies.
    pub(super) fn apply_extension_feature(
        &self,
        node: &Value,
        bodies: &mut Vec<Manifold>,
        body_ids: &mut Vec<String>,
    ) -> Result<(), String> {
        let kind = node.get("type").and_then(Value::as_str).unwrap_or_default();
        let id = node
            .get("id")
            .and_then(Value::as_str)
            .ok_or("feature has no id")?;
        if kind == "edgeFinish" {
            return self.apply_edge_finish(node, bodies);
        }
        let operation = node
            .get("operation")
            .and_then(Value::as_str)
            .ok_or("feature has no operation")?;
        let allowed: &[&str] = if kind == "loft" {
            &["newBody", "join"]
        } else {
            &["newBody", "cut", "join"]
        };
        if !allowed.contains(&operation) {
            return Err(format!("unsupported {kind} operation \"{operation}\""));
        }
        let triangles = if kind == "revolve" {
            self.revolve_tool(node)?
        } else {
            self.loft_tool(node)?
        };
        let tool = manifold_from_triangles(&triangles, kind)?;
        if kind == "loft" && tool.has_self_intersections() {
            return Err("loft surface intersects itself".into());
        }
        if operation == "newBody" {
            bodies.push(tool);
            body_ids.push(id.to_string());
            return Ok(());
        }
        let previous = bodies
            .last_mut()
            .ok_or("cut and join require an existing solid")?;
        let result = if operation == "cut" {
            previous.difference(&tool)
        } else {
            previous.union(&tool)
        };
        if result.status() != ManifoldError::NoError || result.is_empty() {
            return Err(format!("{kind} {operation} leaves no valid solid"));
        }
        *previous = result;
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
        self.nodes_mut()?.push(node);
        if let Err(error) = self.evaluate_solids() {
            self.nodes_mut()?.pop();
            return Err(error);
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
        if let Err(error) = self.evaluate_solids() {
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
        let node = nodes.remove(position);
        if self.has_solid_nodes() && self.evaluate_solids().is_err() {
            self.nodes_mut()?.insert(position, node);
            return Err("later features depend on this feature".into());
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
}
