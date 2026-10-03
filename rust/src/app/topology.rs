//! Vertices, edges and faces of the evaluated bodies, derived from a
//! [`SolidScene`] for hover, selection and measurement in the 3D views.
//!
//! Ids are indices into [`Topology`]'s lists. They are stable for one scene
//! (the derivation is deterministic) and are re-derived when the bodies
//! change; a selection remembers the scene hash it was made on.

use super::*;
use project::SolidEdge;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// Screen distance within which a vertex is hovered.
pub(super) const VERTEX_PICK_PIXELS: f32 = 6.0;
/// Screen distance within which an edge is hovered.
pub(super) const TOPO_EDGE_PICK_PIXELS: f32 = 4.0;

type Key = [i64; 3];

/// Welding key: positions equal to 1e-4 mm are the same corner.
fn key(p: [f32; 3]) -> Key {
    p.map(|c| (c * 1e4).round() as i64)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum TopoRef {
    Vertex(usize),
    Edge(usize),
    Face(usize),
}

/// A picked element and the world point under the cursor (the vertex, the
/// closest edge point or the ray hit on the face).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TopoPick {
    pub(super) element: TopoRef,
    pub(super) point: [f32; 3],
    /// Scene triangle hit, for a face pick.
    pub(super) triangle: Option<usize>,
}

#[derive(Clone, Debug)]
pub(super) struct TopoVertex {
    pub(super) body: usize,
    pub(super) point: [f32; 3],
}

#[derive(Clone, Debug)]
pub(super) struct TopoEdge {
    pub(super) body: usize,
    /// Polyline; two points for a straight edge.
    pub(super) points: Vec<[f32; 3]>,
    /// The polyline returns to its start (a circle or other curved loop).
    pub(super) closed: bool,
    /// The straight sharp edge (what edge finishes act on).
    pub(super) solid: Option<SolidEdge>,
}

#[derive(Clone, Debug)]
pub(super) struct TopoFace {
    pub(super) body: usize,
    pub(super) triangles: Vec<usize>,
    pub(super) planar: bool,
    /// Area-weighted mean outward normal (exact for a planar face).
    pub(super) normal: [f32; 3],
    pub(super) area: f32,
    /// Boundary segments (triangle edges used once inside the face).
    pub(super) outline: Vec<[[f32; 3]; 2]>,
}

#[derive(Debug, Default)]
pub(super) struct Topology {
    pub(super) vertices: Vec<TopoVertex>,
    pub(super) edges: Vec<TopoEdge>,
    pub(super) faces: Vec<TopoFace>,
    /// Face index of each scene triangle.
    pub(super) triangle_face: Vec<usize>,
}

/// Hash of a scene's triangles: the cache and selection key.
pub(super) fn scene_hash(scene: &SolidScene) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    scene.triangles.len().hash(&mut hasher);
    for triangle in &scene.triangles {
        for p in triangle {
            p.map(f32::to_bits).hash(&mut hasher);
        }
    }
    scene.triangle_body.hash(&mut hasher);
    hasher.finish()
}

thread_local! {
    static CACHE: std::cell::RefCell<Option<(u64, std::rc::Rc<Topology>)>> =
        const { std::cell::RefCell::new(None) };
}

/// The topology of `scene`, derived once per scene content.
pub(super) fn cached_topology(scene: &SolidScene) -> (u64, std::rc::Rc<Topology>) {
    let hash = scene_hash(scene);
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((cached, topology)) = &*cache
            && *cached == hash
        {
            return (hash, topology.clone());
        }
        let topology = std::rc::Rc::new(Topology::from_scene(scene));
        *cache = Some((hash, topology.clone()));
        (hash, topology)
    })
}

pub(super) fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub(super) fn norm3(v: [f32; 3]) -> f32 {
    dot3(v, v).sqrt()
}

fn unit3(v: [f32; 3]) -> [f32; 3] {
    let length = norm3(v).max(1e-12);
    v.map(|c| c / length)
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

impl Topology {
    pub(super) fn from_scene(scene: &SolidScene) -> Self {
        let mut topology = Topology::default();
        topology.build_edges(scene);
        topology.build_vertices();
        topology.build_faces(scene);
        topology
    }

    /// Straight sharp edges as they are; facets of tessellated curves
    /// chained into one edge between corners (or into a closed loop).
    fn build_edges(&mut self, scene: &SolidScene) {
        // A sharp edge is a facet of a tessellated curve when the mesh says
        // so, or when another sharp edge leaves one of its ends at a small,
        // non-zero angle (the last facets of a closed loop).
        let threshold = project::SHARP_EDGE_DEGREES.to_radians().cos();
        let mut ends_at: HashMap<(usize, Key), Vec<usize>> = HashMap::new();
        for (index, (body, edge)) in scene.edges.iter().enumerate() {
            ends_at.entry((*body, key(edge.a))).or_default().push(index);
            ends_at.entry((*body, key(edge.b))).or_default().push(index);
        }
        let direction = |index: usize| {
            let edge = &scene.edges[index].1;
            unit3(sub3(edge.b, edge.a))
        };
        let facet = |index: usize| {
            let (body, edge) = &scene.edges[index];
            !edge.straight
                || [edge.a, edge.b].iter().any(|p| {
                    ends_at[&(*body, key(*p))].iter().any(|&other| {
                        let cos = dot3(direction(index), direction(other)).abs();
                        other != index && cos > threshold && cos < 0.99999
                    })
                })
        };
        let mut straight_ends: HashSet<(usize, Key)> = HashSet::new();
        let mut curved: Vec<(usize, [[f32; 3]; 2])> = Vec::new();
        for (index, (body, edge)) in scene.edges.iter().enumerate() {
            if !facet(index) {
                straight_ends.insert((*body, key(edge.a)));
                straight_ends.insert((*body, key(edge.b)));
                self.edges.push(TopoEdge {
                    body: *body,
                    points: vec![edge.a, edge.b],
                    closed: false,
                    solid: Some(edge.clone()),
                });
            } else {
                curved.push((*body, [edge.a, edge.b]));
            }
        }
        let mut at: HashMap<(usize, Key), Vec<usize>> = HashMap::new();
        for (index, (body, [a, b])) in curved.iter().enumerate() {
            at.entry((*body, key(*a))).or_default().push(index);
            at.entry((*body, key(*b))).or_default().push(index);
        }
        // A chain continues through a point shared by exactly two curved
        // facets and no straight edge.
        let passes = |body: usize, p: [f32; 3]| {
            !straight_ends.contains(&(body, key(p)))
                && at.get(&(body, key(p))).is_some_and(|list| list.len() == 2)
        };
        let mut used = vec![false; curved.len()];
        for start in 0..curved.len() {
            if used[start] {
                continue;
            }
            used[start] = true;
            let (body, [a, b]) = curved[start];
            let mut points = std::collections::VecDeque::from([a, b]);
            let mut closed = false;
            for forward in [true, false] {
                loop {
                    let end = if forward {
                        *points.back().unwrap()
                    } else {
                        *points.front().unwrap()
                    };
                    if !passes(body, end) {
                        break;
                    }
                    let Some(&next) = at[&(body, key(end))].iter().find(|&&i| !used[i]) else {
                        let other = if forward {
                            *points.front().unwrap()
                        } else {
                            *points.back().unwrap()
                        };
                        closed = points.len() > 2 && key(other) == key(end);
                        break;
                    };
                    used[next] = true;
                    let [p, q] = curved[next].1;
                    let far = if key(p) == key(end) { q } else { p };
                    if forward {
                        points.push_back(far);
                    } else {
                        points.push_front(far);
                    }
                }
                if closed {
                    break;
                }
            }
            self.edges.push(TopoEdge {
                body,
                points: points.into_iter().collect(),
                closed,
                solid: None,
            });
        }
    }

    /// Edge ends (corners); closed loops have none.
    fn build_vertices(&mut self) {
        let mut seen = HashSet::new();
        for edge in &self.edges {
            if edge.closed {
                continue;
            }
            for p in [edge.points[0], *edge.points.last().unwrap()] {
                if seen.insert((edge.body, key(p))) {
                    self.vertices.push(TopoVertex {
                        body: edge.body,
                        point: p,
                    });
                }
            }
        }
    }

    /// Triangles joined across edges whose faces meet at less than the
    /// sharp-edge angle: planar faces and whole curved faces.
    fn build_faces(&mut self, scene: &SolidScene) {
        let count = scene.triangles.len();
        let normals: Vec<Option<[f32; 3]>> = scene
            .triangles
            .iter()
            .map(|&[a, b, c]| triangle_normal(a, b, c).map(unit3))
            .collect();
        let edge_key = |body: usize, p: [f32; 3], q: [f32; 3]| {
            let (u, v) = (key(p), key(q));
            (body, if u <= v { (u, v) } else { (v, u) })
        };
        let mut by_edge: HashMap<_, Vec<usize>> = HashMap::new();
        for (index, &[a, b, c]) in scene.triangles.iter().enumerate() {
            let body = scene.triangle_body[index];
            for (p, q) in [(a, b), (b, c), (c, a)] {
                by_edge.entry(edge_key(body, p, q)).or_default().push(index);
            }
        }
        let threshold = project::SHARP_EDGE_DEGREES.to_radians().cos();
        let smooth = |i: usize, j: usize| match (normals[i], normals[j]) {
            (Some(n), Some(m)) => dot3(n, m) >= threshold,
            _ => true,
        };
        let mut face_of = vec![usize::MAX; count];
        for seed in 0..count {
            if face_of[seed] != usize::MAX {
                continue;
            }
            let face = self.faces.len();
            let body = scene.triangle_body[seed];
            let mut members = vec![seed];
            face_of[seed] = face;
            let mut stack = vec![seed];
            while let Some(index) = stack.pop() {
                let [a, b, c] = scene.triangles[index];
                for (p, q) in [(a, b), (b, c), (c, a)] {
                    let neighbors = &by_edge[&edge_key(body, p, q)];
                    if neighbors.len() != 2 {
                        continue;
                    }
                    for &other in neighbors {
                        if face_of[other] == usize::MAX && smooth(index, other) {
                            face_of[other] = face;
                            members.push(other);
                            stack.push(other);
                        }
                    }
                }
            }
            members.sort_unstable();
            let mut sum = [0.0f32; 3];
            let mut area = 0.0;
            for &index in &members {
                let [a, b, c] = scene.triangles[index];
                let n = cross(sub3(b, a), sub3(c, a));
                area += norm3(n) * 0.5;
                sum = std::array::from_fn(|i| sum[i] + n[i] * 0.5);
            }
            let normal = unit3(sum);
            let planar = members
                .iter()
                .all(|&i| normals[i].is_none_or(|n| dot3(n, normal) > 0.9999));
            let mut uses: HashMap<_, (usize, [[f32; 3]; 2])> = HashMap::new();
            for &index in &members {
                let [a, b, c] = scene.triangles[index];
                for (p, q) in [(a, b), (b, c), (c, a)] {
                    uses.entry(edge_key(body, p, q)).or_insert((0, [p, q])).0 += 1;
                }
            }
            let mut outline: Vec<_> = uses
                .into_values()
                .filter(|(n, _)| *n == 1)
                .map(|(_, s)| s)
                .collect();
            outline.sort_by(|a, b| key(a[0]).cmp(&key(b[0])).then(key(a[1]).cmp(&key(b[1]))));
            self.faces.push(TopoFace {
                body,
                triangles: members,
                planar,
                normal,
                area,
                outline,
            });
        }
        self.triangle_face = face_of;
    }

    pub(super) fn body_of(&self, element: TopoRef) -> usize {
        match element {
            TopoRef::Vertex(i) => self.vertices[i].body,
            TopoRef::Edge(i) => self.edges[i].body,
            TopoRef::Face(i) => self.faces[i].body,
        }
    }

    pub(super) fn exists(&self, element: TopoRef) -> bool {
        match element {
            TopoRef::Vertex(i) => i < self.vertices.len(),
            TopoRef::Edge(i) => i < self.edges.len(),
            TopoRef::Face(i) => i < self.faces.len(),
        }
    }

    /// Hover priority: a visible vertex within [`VERTEX_PICK_PIXELS`], then
    /// a visible edge within [`TOPO_EDGE_PICK_PIXELS`], then the face hit
    /// by the ray.
    pub(super) fn pick(
        &self,
        scene: &SolidScene,
        projector: &SolidProjector,
        point: Point,
    ) -> Option<TopoPick> {
        let distance = |s: [f32; 2]| ((s[0] - point.x).powi(2) + (s[1] - point.y).powi(2)).sqrt();
        let mut vertices: Vec<(f32, usize)> = self
            .vertices
            .iter()
            .enumerate()
            .map(|(i, v)| (distance(projector.screen(v.point)), i))
            .filter(|(d, _)| *d <= VERTEX_PICK_PIXELS)
            .collect();
        vertices.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, i)) = vertices
            .into_iter()
            .find(|(_, i)| projector.visible(scene, self.vertices[*i].point))
        {
            return Some(TopoPick {
                element: TopoRef::Vertex(i),
                point: self.vertices[i].point,
                triangle: None,
            });
        }
        let mut edges: Vec<(f32, usize, [f32; 3])> = Vec::new();
        for (i, edge) in self.edges.iter().enumerate() {
            let best = edge
                .points
                .windows(2)
                .map(|pair| {
                    let (d, t) = segment_distance(
                        point,
                        projector.screen(pair[0]),
                        projector.screen(pair[1]),
                    );
                    (d, lerp3(pair[0], pair[1], t))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((d, at)) = best.filter(|(d, _)| *d <= TOPO_EDGE_PICK_PIXELS) {
                edges.push((d, i, at));
            }
        }
        edges.sort_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, i, at)) = edges
            .into_iter()
            .find(|(_, _, at)| projector.visible(scene, *at))
        {
            return Some(TopoPick {
                element: TopoRef::Edge(i),
                point: at,
                triangle: None,
            });
        }
        let (triangle, _, hit) = projector.pick_triangle(scene, point)?;
        Some(TopoPick {
            element: TopoRef::Face(*self.triangle_face.get(triangle)?),
            point: hit,
            triangle: Some(triangle),
        })
    }

    pub(super) fn edge_length(&self, index: usize) -> f32 {
        self.edges[index]
            .points
            .windows(2)
            .map(|pair| norm3(sub3(pair[1], pair[0])))
            .sum()
    }

    /// Unit direction of a straight edge.
    pub(super) fn edge_direction(&self, index: usize) -> Option<[f32; 3]> {
        let edge = &self.edges[index];
        (edge.points.len() == 2).then(|| unit3(sub3(edge.points[1], edge.points[0])))
    }

    /// Centre and plane normal of a curved edge (Newell's method); the
    /// axis a curved edge stands for in an assembly mate.
    pub(super) fn edge_axis(&self, index: usize) -> ([f32; 3], [f32; 3]) {
        let edge = &self.edges[index];
        // A closed loop repeats its first point at the end.
        let points = &edge.points[..edge.points.len() - usize::from(edge.closed)];
        let n = points.len() as f32;
        let center = std::array::from_fn(|i| points.iter().map(|p| p[i]).sum::<f32>() / n);
        if let Some(direction) = self.edge_direction(index) {
            return (center, direction);
        }
        let mut normal = [0.0f32; 3];
        for (k, p) in points.iter().enumerate() {
            let q = points[(k + 1) % points.len()];
            normal[0] += (p[1] - q[1]) * (p[2] + q[2]);
            normal[1] += (p[2] - q[2]) * (p[0] + q[0]);
            normal[2] += (p[0] - q[0]) * (p[1] + q[1]);
        }
        (center, unit3(normal))
    }

    /// Normal of a planar face touching the vertex (any face if none is
    /// planar).
    pub(super) fn vertex_normal(&self, scene: &SolidScene, index: usize) -> Option<[f32; 3]> {
        let vertex = &self.vertices[index];
        let k = key(vertex.point);
        let touching: Vec<&TopoFace> = self
            .faces
            .iter()
            .filter(|face| face.body == vertex.body)
            .filter(|face| {
                face.triangles
                    .iter()
                    .any(|&t| scene.triangles[t].iter().any(|p| key(*p) == k))
            })
            .collect();
        touching
            .iter()
            .find(|face| face.planar)
            .or(touching.first())
            .map(|face| face.normal)
    }

    /// One status-bar line for a hovered or selected element.
    pub(super) fn describe(&self, element: TopoRef, face_name: &str) -> String {
        match element {
            TopoRef::Vertex(i) => {
                let [x, y, z] = self.vertices[i].point;
                format!("Vertex ({x:.2}, {y:.2}, {z:.2})")
            }
            TopoRef::Edge(i) => format!(
                "Edge{} L={:.2} mm",
                if self.edges[i].closed { " loop" } else { "" },
                self.edge_length(i)
            ),
            TopoRef::Face(i) => format!("Face {face_name} A={:.2} mm²", self.faces[i].area),
        }
    }

    /// Properties lines for one selected element.
    pub(super) fn properties(&self, element: TopoRef, face_id: Option<&str>) -> Vec<String> {
        let fmt = |p: [f32; 3]| format!("{:.2}, {:.2}, {:.2}", p[0], p[1], p[2]);
        match element {
            TopoRef::Vertex(i) => vec![
                format!("VERTEX #{i}"),
                format!("AT   {}", fmt(self.vertices[i].point)),
            ],
            TopoRef::Edge(i) => {
                let edge = &self.edges[i];
                let mut lines = vec![
                    format!(
                        "EDGE #{i}{}",
                        if edge.solid.is_some() {
                            ""
                        } else if edge.closed {
                            " (CURVED LOOP)"
                        } else {
                            " (CURVED)"
                        }
                    ),
                    format!("LEN  {:.2} mm", self.edge_length(i)),
                ];
                if !edge.closed {
                    lines.push(format!("A    {}", fmt(edge.points[0])));
                    lines.push(format!("B    {}", fmt(*edge.points.last().unwrap())));
                }
                if let Some(d) = self.edge_direction(i) {
                    let angle = |c: f32| c.abs().min(1.0).acos().to_degrees();
                    lines.push(format!(
                        "TO X {:.1}  Y {:.1}  Z {:.1} deg",
                        angle(d[0]),
                        angle(d[1]),
                        angle(d[2])
                    ));
                }
                lines
            }
            TopoRef::Face(i) => {
                let face = &self.faces[i];
                let mut lines = vec![
                    format!("FACE #{i}{}", if face.planar { "" } else { " (CURVED)" }),
                    format!("AREA {:.2} mm²", face.area),
                ];
                if face.planar {
                    lines.push(format!("N    {}", fmt(face.normal)));
                }
                if let Some(id) = face_id {
                    lines.push(format!("ID   {id}"));
                }
                lines
            }
        }
    }
}

/// Read-only measurement between two selected elements.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Measure {
    /// Minimum distance in millimeters, when it is defined (vertex to
    /// anything, parallel edges, parallel faces, an edge parallel to a face).
    pub(super) distance: Option<f32>,
    /// Angle in degrees between two edges (0-90), two planar faces (between
    /// normals, 0-180) or an edge and a planar face (0-90).
    pub(super) angle: Option<f32>,
}

enum Shape3 {
    Point([f32; 3]),
    Line([f32; 3], [f32; 3]),
    Curve(usize),
    Plane([f32; 3], [f32; 3], usize),
    Surface(usize),
}

/// Distance from `p` to segment `a`-`b`.
fn point_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = sub3(b, a);
    let length = dot3(d, d);
    let t = if length < 1e-12 {
        0.0
    } else {
        (dot3(sub3(p, a), d) / length).clamp(0.0, 1.0)
    };
    norm3(sub3(p, lerp3(a, b, t)))
}

/// Distance from `p` to triangle `a b c`.
fn point_triangle(p: [f32; 3], [a, b, c]: [[f32; 3]; 3]) -> f32 {
    let n = cross(sub3(b, a), sub3(c, a));
    if dot3(n, n) > 1e-12 {
        let n = unit3(n);
        let height = dot3(sub3(p, a), n);
        let q = sub3(p, n.map(|c| c * height));
        let inside = [(a, b), (b, c), (c, a)]
            .iter()
            .all(|&(u, v)| dot3(cross(sub3(v, u), sub3(q, u)), n) >= -1e-6);
        if inside {
            return height.abs();
        }
    }
    point_segment(p, a, b)
        .min(point_segment(p, b, c))
        .min(point_segment(p, c, a))
}

impl Topology {
    fn shape(&self, element: TopoRef) -> Shape3 {
        match element {
            TopoRef::Vertex(i) => Shape3::Point(self.vertices[i].point),
            TopoRef::Edge(i) => {
                let points = &self.edges[i].points;
                if points.len() == 2 {
                    Shape3::Line(points[0], points[1])
                } else {
                    Shape3::Curve(i)
                }
            }
            TopoRef::Face(i) => {
                let face = &self.faces[i];
                if face.planar {
                    let p = face.outline.first().map_or([0.0; 3], |s| s[0]);
                    Shape3::Plane(p, face.normal, i)
                } else {
                    Shape3::Surface(i)
                }
            }
        }
    }

    fn point_distance(&self, scene: &SolidScene, p: [f32; 3], other: &Shape3) -> f32 {
        let face = |index: usize| {
            self.faces[index]
                .triangles
                .iter()
                .map(|&t| point_triangle(p, scene.triangles[t]))
                .fold(f32::INFINITY, f32::min)
        };
        match *other {
            Shape3::Point(q) => norm3(sub3(p, q)),
            Shape3::Line(a, b) => point_segment(p, a, b),
            Shape3::Curve(index) => self.edges[index]
                .points
                .windows(2)
                .map(|pair| point_segment(p, pair[0], pair[1]))
                .fold(f32::INFINITY, f32::min),
            Shape3::Plane(_, _, index) | Shape3::Surface(index) => face(index),
        }
    }

    pub(super) fn measure(&self, scene: &SolidScene, a: TopoRef, b: TopoRef) -> Measure {
        let degrees = |cos: f32| cos.clamp(-1.0, 1.0).acos().to_degrees();
        let (sa, sb) = (self.shape(a), self.shape(b));
        match (&sa, &sb) {
            (Shape3::Point(p), other) | (other, Shape3::Point(p)) => Measure {
                distance: Some(self.point_distance(scene, *p, other)),
                angle: None,
            },
            (Shape3::Line(a0, a1), Shape3::Line(b0, b1)) => {
                let (u, v) = (unit3(sub3(*a1, *a0)), unit3(sub3(*b1, *b0)));
                let cos = dot3(u, v).abs();
                let distance = (cos > 0.9999).then(|| {
                    let w = sub3(*b0, *a0);
                    norm3(sub3(w, u.map(|c| c * dot3(w, u))))
                });
                Measure {
                    distance,
                    angle: Some(degrees(cos)),
                }
            }
            (Shape3::Plane(p, n, _), Shape3::Plane(q, m, _)) => Measure {
                distance: (dot3(*n, *m).abs() > 0.9999).then(|| dot3(*n, sub3(*q, *p)).abs()),
                angle: Some(degrees(dot3(*n, *m))),
            },
            (Shape3::Line(a0, a1), Shape3::Plane(p, n, _))
            | (Shape3::Plane(p, n, _), Shape3::Line(a0, a1)) => {
                let sin = dot3(unit3(sub3(*a1, *a0)), *n).abs();
                Measure {
                    distance: (sin < 1e-4).then(|| dot3(*n, sub3(*a0, *p)).abs()),
                    angle: Some(sin.clamp(0.0, 1.0).asin().to_degrees()),
                }
            }
            _ => Measure::default(),
        }
    }
}

/// Screen distance from `p` to segment `a`-`b` and the segment parameter
/// of the closest point.
pub(super) fn segment_distance(p: Point, a: [f32; 2], b: [f32; 2]) -> (f32, f32) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dy * dy;
    let t = if length < 1e-9 {
        0.0
    } else {
        (((p.x - a[0]) * dx + (p.y - a[1]) * dy) / length).clamp(0.0, 1.0)
    };
    (
        ((p.x - a[0] - t * dx).powi(2) + (p.y - a[1] - t * dy).powi(2)).sqrt(),
        t,
    )
}
