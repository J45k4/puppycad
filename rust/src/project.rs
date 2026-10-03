use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use manifold_rust::{manifold::Manifold, types::Error as ManifoldError};
use serde_json::{Value, json};

use crate::model::{
    AxisConstraint, ChamferMode, ChamferSpec, DimensionField, DimensionKey, Document,
    LineRelationKind, Point, Shape,
};
use crate::spline::{self, SplineMode};

fn line_relation_matches(source: &Shape, target: &Shape, kind: LineRelationKind) -> bool {
    let (Shape::Line { a: a0, b: a1, .. }, Shape::Line { a: b0, b: b1, .. }) = (source, target)
    else {
        return false;
    };
    let (ax, ay) = (a1.x - a0.x, a1.y - a0.y);
    let (bx, by) = (b1.x - b0.x, b1.y - b0.y);
    let norm = (ax * ax + ay * ay).sqrt() * (bx * bx + by * by).sqrt();
    if !norm.is_finite() || norm <= 1e-6 {
        return false;
    }
    let cross = ax * by - ay * bx;
    let dot = ax * bx + ay * by;
    match kind {
        LineRelationKind::Parallel => cross.abs() / norm < 1e-3,
        LineRelationKind::Perpendicular => dot.abs() / norm < 1e-3,
        LineRelationKind::Collinear => {
            cross.abs() / norm < 1e-3
                && ((b0.x - a0.x) * ay - (b0.y - a0.y) * ax).abs() / (ax * ax + ay * ay).sqrt()
                    < 1e-3
        }
        LineRelationKind::Angle(value) => {
            let delta = cross.atan2(dot) - value.to_radians();
            delta.sin().atan2(delta.cos()).abs() < 1e-3
        }
    }
}

fn circular_tangent_matches(outer: &Shape, inner: &Shape, internal: bool) -> bool {
    let circular = |shape: &Shape| match shape {
        Shape::Circle { center, radius, .. } => Some((*center, *radius, None)),
        Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            ..
        } => Some((*center, *radius, Some((*start_angle, *sweep)))),
        _ => None,
    };
    let Some((outer_center, outer_radius, outer_arc)) = circular(outer) else {
        return false;
    };
    let Some((inner_center, inner_radius, inner_arc)) = circular(inner) else {
        return false;
    };
    let distance = outer_center.distance(inner_center);
    let expected = if internal {
        outer_radius - inner_radius
    } else {
        outer_radius + inner_radius
    };
    if !distance.is_finite()
        || expected <= 1e-6
        || distance <= 1e-6
        || (distance - expected).abs() >= 1e-3
    {
        return false;
    }
    let unit = Point {
        x: (inner_center.x - outer_center.x) / distance,
        y: (inner_center.y - outer_center.y) / distance,
    };
    let contact = Point {
        x: outer_center.x + unit.x * outer_radius,
        y: outer_center.y + unit.y * outer_radius,
    };
    let on_arc = |arc: Option<(f64, f64)>, center: Point| {
        arc.is_none_or(|(start, sweep)| {
            if !start.is_finite() || !sweep.is_finite() || sweep.abs() <= 1e-9 {
                return false;
            }
            let angle = ((contact.y - center.y) as f64).atan2((contact.x - center.x) as f64);
            let traveled = if sweep > 0.0 {
                (angle - start).rem_euclid(std::f64::consts::TAU)
            } else {
                (start - angle).rem_euclid(std::f64::consts::TAU)
            };
            traveled <= sweep.abs() + 1e-5
        })
    };
    on_arc(outer_arc, outer_center) && on_arc(inner_arc, inner_center)
}

fn internal_tangent_matches(outer: &Shape, inner: &Shape) -> bool {
    circular_tangent_matches(outer, inner, true)
}

fn external_tangent_matches(source: &Shape, target: &Shape) -> bool {
    let line_and_ellipse = match (source, target) {
        (
            Shape::Line { a, b, .. },
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            }
            | Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                ..
            },
        ) => Some((*a, *b, *center, *rx, *ry, *rotation, target)),
        (
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            }
            | Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                ..
            },
            Shape::Line { a, b, .. },
        ) => Some((*a, *b, *center, *rx, *ry, *rotation, source)),
        _ => None,
    };
    if let Some((a, b, center, rx, ry, rotation, ellipse)) = line_and_ellipse {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= 1e-6 {
            return false;
        }
        let normal = Point {
            x: -dy / length,
            y: dx / length,
        };
        let (sin, cos) = rotation.to_radians().sin_cos();
        let nx = normal.x * cos + normal.y * sin;
        let ny = -normal.x * sin + normal.y * cos;
        let extent = ((rx * nx).powi(2) + (ry * ny).powi(2)).sqrt();
        if extent <= 1e-6 {
            return false;
        }
        let support_x = rx * rx * nx / extent;
        let support_y = ry * ry * ny / extent;
        let support = Point {
            x: support_x * cos - support_y * sin,
            y: support_x * sin + support_y * cos,
        };
        let signed = (center.x - a.x) * normal.x + (center.y - a.y) * normal.y;
        if (signed.abs() - extent).abs() >= 1e-3 {
            return false;
        }
        let side = if signed < 0.0 { -1.0 } else { 1.0 };
        let contact = Point {
            x: center.x - side * support.x,
            y: center.y - side * support.y,
        };
        return match ellipse {
            Shape::EllipticArc {
                start_angle, sweep, ..
            } => {
                let angle = Shape::ellipse_parameter_angle(center, rx, ry, rotation, contact);
                Shape::elliptic_arc_progress(angle, *start_angle, *sweep) <= 1.0 + 1e-5
            }
            _ => true,
        };
    }
    let line_and_curve = match (source, target) {
        (
            Shape::Line { a, b, .. },
            Shape::Circle { center, radius, .. } | Shape::Arc { center, radius, .. },
        ) => Some((*a, *b, *center, *radius, target)),
        (
            Shape::Circle { center, radius, .. } | Shape::Arc { center, radius, .. },
            Shape::Line { a, b, .. },
        ) => Some((*a, *b, *center, *radius, source)),
        _ => None,
    };
    if let Some((a, b, center, radius, curve)) = line_and_curve {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let length = (dx * dx + dy * dy).sqrt();
        if length <= 1e-6 {
            return false;
        }
        let normal = Point {
            x: -dy / length,
            y: dx / length,
        };
        let signed = (center.x - a.x) * normal.x + (center.y - a.y) * normal.y;
        if (signed.abs() - radius).abs() >= 1e-3 {
            return false;
        }
        let contact = Point {
            x: center.x - normal.x * signed,
            y: center.y - normal.y * signed,
        };
        return match curve {
            Shape::Arc {
                start_angle, sweep, ..
            } => {
                let angle = ((contact.y - center.y) as f64).atan2((contact.x - center.x) as f64);
                let traveled = if *sweep > 0.0 {
                    (angle - start_angle).rem_euclid(std::f64::consts::TAU)
                } else {
                    (start_angle - angle).rem_euclid(std::f64::consts::TAU)
                };
                traveled <= sweep.abs() + 1e-5
            }
            _ => true,
        };
    }
    circular_tangent_matches(source, target, false)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchPlane {
    Top,
    Front,
    Right,
}

impl SketchPlane {
    fn from_graph_node(node: &Value) -> Option<Self> {
        if node.get("type").and_then(Value::as_str) != Some("referencePlane") {
            return None;
        }
        match node.get("plane").and_then(Value::as_str)? {
            "XY" => Some(Self::Top),
            "XZ" => Some(Self::Front),
            "YZ" => Some(Self::Right),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Top => "TOP",
            Self::Front => "FRONT",
            Self::Right => "RIGHT",
        }
    }
}

/// A v4 PuppyCAD project. Unknown documents and graph nodes are retained verbatim.
pub struct ProjectFile {
    path: PathBuf,
    value: Value,
    part_index: usize,
    sketches: Vec<SketchWorkspace>,
    active_sketch: usize,
    read_only_reason: Option<String>,
    disk_snapshot: Option<Vec<u8>>,
    parked_parts: HashMap<usize, PartWorkspace>,
    metadata_dirty: bool,
    /// Serialized project as of the last open or save, for unsaved-change checks.
    clean_value: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Parent,
    Folder,
    Project,
}

/// One row on the native projects page: the parent folder, a subfolder, or a `.pcad` file.
#[derive(Clone, Debug)]
pub struct ProjectSummary {
    pub kind: EntryKind,
    pub path: PathBuf,
    pub name: String,
    pub parts: usize,
    pub revision: u64,
    pub error: Option<String>,
}

impl ProjectSummary {
    fn folder(kind: EntryKind, path: PathBuf, name: String) -> Self {
        Self {
            kind,
            path,
            name,
            parts: 0,
            revision: 0,
            error: None,
        }
    }
}

/// Lists the parent folder, visible subfolders and `.pcad` files in `dir`, in that order and
/// sorted by name. Unreadable project files are kept with an error.
pub fn list_projects(dir: &Path) -> Result<Vec<ProjectSummary>, String> {
    let entries = fs::read_dir(dir).map_err(|error| format!("read {}: {error}", dir.display()))?;
    let mut folders = Vec::new();
    let mut projects = Vec::new();
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        let Some(name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if path.is_dir() {
            if !name.starts_with('.') {
                folders.push(ProjectSummary::folder(EntryKind::Folder, path, name));
            }
            continue;
        }
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "pcad") {
            continue;
        }
        let name = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or(name);
        let parsed = fs::read(&path)
            .map_err(|error| error.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<Value>(&bytes).map_err(|error| error.to_string())
            });
        let (parts, revision, error) = match parsed {
            Ok(value) => (
                value["items"].as_array().map_or(0, |items| {
                    items.iter().filter(|item| item["type"] == "part").count()
                }),
                value["revision"].as_u64().unwrap_or(0),
                None,
            ),
            Err(error) => (0, 0, Some(error)),
        };
        projects.push(ProjectSummary {
            kind: EntryKind::Project,
            path,
            name,
            parts,
            revision,
            error,
        });
    }
    folders.sort_by_key(|folder| folder.name.to_lowercase());
    projects.sort_by_key(|project| project.name.to_lowercase());
    let parent = dir.parent().filter(|parent| !parent.as_os_str().is_empty());
    Ok(parent
        .map(|parent| ProjectSummary::folder(EntryKind::Parent, parent.to_path_buf(), "..".into()))
        .into_iter()
        .chain(folders)
        .chain(projects)
        .collect())
}

type StagedProject = (Value, Vec<SketchWorkspace>, HashMap<usize, PartWorkspace>);

#[derive(Clone)]
struct PartWorkspace {
    sketches: Vec<SketchWorkspace>,
    active_sketch: usize,
    read_only_reason: Option<String>,
}

#[derive(Clone)]
struct SketchWorkspace {
    id: String,
    name: String,
    plane: Option<SketchPlane>,
    document: Document,
    entity_ids: HashMap<u32, String>,
    original_entities: HashMap<u32, Value>,
}

pub struct SolidPreview {
    pub name: String,
    pub profile: Vec<Point>,
    pub holes: Vec<Vec<Point>>,
    pub depth: f32,
    pub plane: SketchPlane,
}

impl SolidPreview {
    pub fn world_point(&self, point: Point, offset: f32) -> [f32; 3] {
        match self.plane {
            SketchPlane::Top => [point.x, point.y, offset],
            SketchPlane::Front => [point.x, offset, -point.y],
            SketchPlane::Right => [offset, point.x, point.y],
        }
    }

    pub fn normal(&self) -> [f32; 3] {
        match self.plane {
            SketchPlane::Top => [0.0, 0.0, 1.0],
            SketchPlane::Front => [0.0, 1.0, 0.0],
            SketchPlane::Right => [1.0, 0.0, 0.0],
        }
    }

    pub fn loops(&self) -> impl Iterator<Item = &[Point]> {
        std::iter::once(self.profile.as_slice()).chain(self.holes.iter().map(Vec::as_slice))
    }

    pub fn cap_mesh(&self) -> Option<(Vec<Point>, Vec<[usize; 3]>)> {
        let mut points = Vec::new();
        let mut holes = Vec::new();
        for (index, loop_points) in self.loops().enumerate() {
            if loop_points.len() < 3 {
                return None;
            }
            if index > 0 {
                holes.push(points.len());
            }
            points.extend_from_slice(loop_points);
        }
        let coordinates: Vec<f64> = points
            .iter()
            .flat_map(|p| [p.x as f64, p.y as f64])
            .collect();
        let indices = earcutr::earcut(&coordinates, &holes, 2).ok()?;
        if indices.len() % 3 != 0 {
            return None;
        }
        let triangles: Vec<[usize; 3]> = indices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|chunk| [chunk[0], chunk[1], chunk[2]])
            .collect();
        let triangles = split_cap_t_junctions(&mut points, &triangles);
        let expected = signed_area(&self.profile).abs()
            - self
                .holes
                .iter()
                .map(|hole| signed_area(hole).abs())
                .sum::<f32>();
        let actual: f32 = triangles
            .iter()
            .map(|[a, b, c]| {
                let [a, b, c] = [points[*a], points[*b], points[*c]];
                ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).abs() * 0.5
            })
            .sum();
        if expected <= 1e-5 || !actual.is_finite() || (actual - expected).abs() > expected * 0.002 {
            return None;
        }
        Some((points, triangles))
    }
}

fn split_cap_t_junctions(points: &mut Vec<Point>, triangles: &[[usize; 3]]) -> Vec<[usize; 3]> {
    let boundary = points.clone();
    let mut split = Vec::new();
    for triangle in triangles {
        let mut perimeter = Vec::new();
        for edge in 0..3 {
            let start_index = triangle[edge];
            let end_index = triangle[(edge + 1) % 3];
            let a = points[start_index];
            let b = points[end_index];
            let dx = (b.x - a.x) as f64;
            let dy = (b.y - a.y) as f64;
            let length_squared = dx * dx + dy * dy;
            perimeter.push(start_index);
            if length_squared <= 1e-20 {
                continue;
            }
            let mut on_edge = Vec::new();
            for (index, point) in boundary.iter().enumerate() {
                if index == start_index || index == end_index {
                    continue;
                }
                let px = (point.x - a.x) as f64;
                let py = (point.y - a.y) as f64;
                let t = (px * dx + py * dy) / length_squared;
                if !(1e-7..1.0 - 1e-7).contains(&t) {
                    continue;
                }
                let cross = px * dy - py * dx;
                if cross.abs() <= 1e-8 * length_squared {
                    on_edge.push((t, index));
                }
            }
            on_edge.sort_by(|a, b| a.0.total_cmp(&b.0));
            perimeter.extend(on_edge.into_iter().map(|(_, index)| index));
        }
        if perimeter.len() == 3 {
            split.push(*triangle);
            continue;
        }
        let center = Point {
            x: triangle.iter().map(|index| points[*index].x).sum::<f32>() / 3.0,
            y: triangle.iter().map(|index| points[*index].y).sum::<f32>() / 3.0,
        };
        let center_index = points.len();
        points.push(center);
        for index in 0..perimeter.len() {
            split.push([
                center_index,
                perimeter[index],
                perimeter[(index + 1) % perimeter.len()],
            ]);
        }
    }
    split
}

fn signed_area(points: &[Point]) -> f32 {
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let q = points[(i + 1) % points.len()];
            p.x * q.y - q.x * p.y
        })
        .sum::<f32>()
        * 0.5
}

fn orient_loop(mut points: Vec<Point>, positive: bool) -> Vec<Point> {
    if (signed_area(&points) > 0.0) != positive {
        points.reverse();
    }
    points
}

fn cross(a: Point, b: Point, c: Point) -> f64 {
    (b.x - a.x) as f64 * (c.y - a.y) as f64 - (b.y - a.y) as f64 * (c.x - a.x) as f64
}

fn point_on_segment(point: Point, a: Point, b: Point) -> bool {
    cross(a, b, point).abs() <= 1e-6
        && point.x >= a.x.min(b.x) - 1e-6
        && point.x <= a.x.max(b.x) + 1e-6
        && point.y >= a.y.min(b.y) - 1e-6
        && point.y <= a.y.max(b.y) + 1e-6
}

fn segments_intersect(a: Point, b: Point, c: Point, d: Point) -> bool {
    let ab_c = cross(a, b, c);
    let ab_d = cross(a, b, d);
    let cd_a = cross(c, d, a);
    let cd_b = cross(c, d, b);
    (ab_c.abs() <= 1e-6 && point_on_segment(c, a, b))
        || (ab_d.abs() <= 1e-6 && point_on_segment(d, a, b))
        || (cd_a.abs() <= 1e-6 && point_on_segment(a, c, d))
        || (cd_b.abs() <= 1e-6 && point_on_segment(b, c, d))
        || ((ab_c > 0.0) != (ab_d > 0.0) && (cd_a > 0.0) != (cd_b > 0.0))
}

fn simple_loop(points: &[Point]) -> bool {
    if points.len() < 3 || signed_area(points).abs() <= 1e-5 {
        return false;
    }
    for i in 0..points.len() {
        let next = (i + 1) % points.len();
        if points[i].distance(points[next]) <= 1e-6 {
            return false;
        }
        for j in i + 1..points.len() {
            let next_j = (j + 1) % points.len();
            if i == j || next == j || next_j == i {
                continue;
            }
            if segments_intersect(points[i], points[next], points[j], points[next_j]) {
                return false;
            }
        }
    }
    true
}

fn point_inside_loop(point: Point, polygon: &[Point]) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        if point_on_segment(point, a, b) {
            return false;
        }
        if (a.y > point.y) != (b.y > point.y)
            && (point.x as f64)
                < (b.x - a.x) as f64 * (point.y - a.y) as f64 / (b.y - a.y) as f64 + a.x as f64
        {
            inside = !inside;
        }
    }
    inside
}

fn loops_intersect(a: &[Point], b: &[Point]) -> bool {
    a.iter().enumerate().any(|(i, start_a)| {
        let end_a = a[(i + 1) % a.len()];
        b.iter()
            .enumerate()
            .any(|(j, start_b)| segments_intersect(*start_a, end_a, *start_b, b[(j + 1) % b.len()]))
    })
}

pub struct ProfileLoops {
    pub outer: Vec<Point>,
    pub holes: Vec<Vec<Point>>,
}

/// Web anchor name of a line/arc (`p0`/`p1`) or open spline
/// (`point0`/`pointN`) endpoint.
fn curve_end_name(shapes: &[Shape], (id, end): (u32, u8)) -> Option<String> {
    match shapes.iter().find(|shape| shape.id() == id)? {
        Shape::Line { .. } | Shape::Arc { .. } => Some(if end == 0 { "p0" } else { "p1" }.into()),
        Shape::Spline {
            points,
            closed: false,
            ..
        } => Some(if end == 0 {
            "point0".into()
        } else {
            format!("point{}", points.len().checked_sub(1)?)
        }),
        _ => None,
    }
}

/// Endpoint index (0 start, 1 end) of a web anchor name on a line, arc or
/// open spline.
fn curve_end_index(shape: &Shape, name: &str) -> Option<u8> {
    match shape {
        Shape::Line { .. } | Shape::Arc { .. } => match name {
            "p0" => Some(0),
            "p1" => Some(1),
            _ => None,
        },
        Shape::Spline {
            points,
            closed: false,
            ..
        } => {
            if name == "point0" {
                Some(0)
            } else if name == format!("point{}", points.len().checked_sub(1)?) {
                Some(1)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
fn profile_from_document(document: &Document) -> Option<ProfileLoops> {
    let mut profiles = profiles_from_document(document)?;
    (profiles.len() == 1).then(|| profiles.remove(0))
}

pub fn profiles_from_document(document: &Document) -> Option<Vec<ProfileLoops>> {
    let mut curves = Vec::new();
    let mut loops = Vec::new();
    for shape in document
        .shapes
        .iter()
        .filter(|shape| !document.construction.contains(&shape.id()))
    {
        if matches!(
            shape,
            Shape::Line { .. }
                | Shape::Arc { .. }
                | Shape::EllipticArc { .. }
                | Shape::Spline { .. }
        ) {
            curves.push(shape);
        } else if matches!(shape, Shape::Point { .. }) {
            continue;
        } else {
            loops.push(shape_profile(shape)?);
        }
    }
    if !curves.is_empty() {
        loops.extend(segment_loop_profiles(&curves)?);
    }
    if loops.is_empty() {
        return None;
    }
    if loops.iter().any(|loop_points| !simple_loop(loop_points)) {
        return None;
    }
    for i in 0..loops.len() {
        for j in i + 1..loops.len() {
            if loops_intersect(&loops[i], &loops[j]) {
                return None;
            }
        }
    }
    let areas = loops
        .iter()
        .map(|loop_points| signed_area(loop_points).abs())
        .collect::<Vec<_>>();
    let parents = (0..loops.len())
        .map(|index| {
            (0..loops.len())
                .filter(|candidate| {
                    *candidate != index
                        && areas[*candidate] > areas[index]
                        && point_inside_loop(loops[index][0], &loops[*candidate])
                })
                .min_by(|a, b| areas[*a].total_cmp(&areas[*b]))
        })
        .collect::<Vec<_>>();
    let mut profiles = Vec::new();
    for index in 0..loops.len() {
        let mut depth = 0;
        let mut ancestor = parents[index];
        while let Some(parent) = ancestor {
            depth += 1;
            ancestor = parents[parent];
        }
        if depth % 2 != 0 {
            continue;
        }
        let outer = orient_loop(loops[index].clone(), true);
        let holes = (0..loops.len())
            .filter(|candidate| parents[*candidate] == Some(index))
            .map(|candidate| orient_loop(loops[candidate].clone(), false))
            .collect();
        profiles.push(ProfileLoops { outer, holes });
    }
    Some(profiles)
}

fn shape_profile(shape: &Shape) -> Option<Vec<Point>> {
    Some(match shape {
        Shape::Rectangle { a, b, .. } => vec![
            Point { x: a.x, y: a.y },
            Point { x: b.x, y: a.y },
            *b,
            Point { x: a.x, y: b.y },
        ],
        Shape::RotatedRectangle {
            center,
            width,
            height,
            rotation,
            ..
        } => Shape::rotated_rectangle_corners(*center, *width, *height, *rotation).to_vec(),
        Shape::Circle {
            center,
            radius,
            segments,
            ..
        } => (0..*segments)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / *segments as f32;
                Point {
                    x: center.x + radius * angle.cos(),
                    y: center.y + radius * angle.sin(),
                }
            })
            .collect(),
        Shape::Ellipse {
            center,
            rx,
            ry,
            rotation,
            segments,
            ..
        } => (0..*segments)
            .map(|i| {
                Shape::ellipse_point(*center, *rx, *ry, *rotation, i as f32 / *segments as f32)
            })
            .collect(),
        Shape::Polygon {
            center,
            radius,
            sides,
            rotation,
            ..
        } => (0..*sides)
            .map(|i| {
                let angle =
                    rotation.to_radians() + i as f32 * std::f32::consts::TAU / *sides as f32;
                Point {
                    x: center.x + radius * angle.cos(),
                    y: center.y + radius * angle.sin(),
                }
            })
            .collect(),
        Shape::Capsule {
            from,
            to,
            width,
            arc_segments,
            ..
        } => Shape::capsule_outline(*from, *to, *width, *arc_segments as usize),
        _ => return None,
    })
}

fn segment_loop_profiles(shapes: &[&Shape]) -> Option<Vec<Vec<Point>>> {
    if shapes.is_empty() {
        return None;
    }
    let mut segments = Vec::new();
    for shape in shapes {
        match shape {
            Shape::Line { a, b, .. } => {
                if a.distance(*b) > 1e-6 {
                    segments.push((*a, *b));
                }
            }
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                segments: sample_segments,
                ..
            } => {
                let count = ((sweep.abs() / std::f64::consts::TAU) * *sample_segments as f64)
                    .ceil()
                    .max(2.0) as usize;
                let points = (0..=count)
                    .map(|index| {
                        Shape::arc_point(
                            *center,
                            *radius,
                            *start_angle,
                            *sweep,
                            index as f64 / count as f64,
                        )
                    })
                    .collect::<Vec<_>>();
                segments.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
            }
            Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                segments: sample_segments,
                ..
            } => {
                let count = ((sweep.abs() / std::f64::consts::TAU) * *sample_segments as f64)
                    .ceil()
                    .max(2.0) as usize;
                let points = (0..=count)
                    .map(|index| {
                        Shape::elliptic_arc_point(
                            *center,
                            *rx,
                            *ry,
                            *rotation,
                            *start_angle,
                            *sweep,
                            index as f64 / count as f64,
                        )
                    })
                    .collect::<Vec<_>>();
                segments.extend(points.windows(2).map(|pair| (pair[0], pair[1])));
            }
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                let curves = spline::beziers(*mode, points, *closed).ok()?;
                let sampled = spline::samples(&curves, 0.01).ok()?;
                segments.extend(sampled.windows(2).map(|pair| (pair[0], pair[1])));
            }
            _ => return None,
        }
    }
    let near = |a: Point, b: Point| a.distance(b) <= 1e-6;
    let mut visited = vec![false; segments.len()];
    let mut profiles = Vec::new();
    while let Some(first) = visited.iter().position(|done| !done) {
        let mut component = Vec::new();
        let mut pending = vec![first];
        visited[first] = true;
        while let Some(index) = pending.pop() {
            component.push(index);
            let (a, b) = segments[index];
            for (other_index, (c, d)) in segments.iter().enumerate() {
                if !visited[other_index]
                    && (near(a, *c) || near(a, *d) || near(b, *c) || near(b, *d))
                {
                    visited[other_index] = true;
                    pending.push(other_index);
                }
            }
        }
        let closed = component.iter().all(|index| {
            let (a, b) = segments[*index];
            [a, b].iter().all(|point| {
                component
                    .iter()
                    .map(|other| {
                        let (c, d) = segments[*other];
                        usize::from(near(*point, c)) + usize::from(near(*point, d))
                    })
                    .sum::<usize>()
                    == 2
            })
        });
        if !closed {
            continue;
        }
        let mut used = vec![false; segments.len()];
        used[first] = true;
        let start = segments[first].0;
        let mut current = segments[first].1;
        let mut profile = vec![start, current];
        while !near(current, start) {
            let next = segments
                .iter()
                .enumerate()
                .filter(|(index, (a, b))| {
                    component.contains(index)
                        && !used[*index]
                        && (near(*a, current) || near(*b, current))
                })
                .collect::<Vec<_>>();
            if next.len() != 1 {
                return None;
            }
            let (index, (a, b)) = next[0];
            used[index] = true;
            current = if near(*a, current) { *b } else { *a };
            if !near(current, start) {
                profile.push(current);
            }
        }
        if used.iter().filter(|used| **used).count() != component.len() || !simple_loop(&profile) {
            return None;
        }
        profiles.push(profile);
    }
    Some(profiles)
}

impl ProjectFile {
    pub fn extrusion_preview(&self) -> Option<SolidPreview> {
        let mut previews = self.extrusion_previews()?;
        (previews.len() == 1).then(|| previews.remove(0))
    }

    pub fn extrusion_previews(&self) -> Option<Vec<SolidPreview>> {
        let nodes = self
            .value
            .pointer(&format!("/items/{}/data/cad/nodes", self.part_index))?
            .as_array()?;
        let extrudes: Vec<_> = nodes
            .iter()
            .filter(|node| node.get("type").and_then(Value::as_str) == Some("extrude"))
            .collect();
        if extrudes.is_empty()
            || nodes
                .iter()
                .any(|node| node.get("type").and_then(Value::as_str) == Some("chamfer"))
        {
            return None;
        }
        extrudes
            .into_iter()
            .map(|extrude| self.preview_for_extrude(extrude))
            .collect()
    }

    pub fn extrusion_operation_at(&self, index: usize) -> Option<&str> {
        self.value
            .pointer(&format!("/items/{}/data/cad/nodes", self.part_index))?
            .as_array()?
            .iter()
            .filter(|node| node["type"] == "extrude")
            .nth(index)?
            .get("operation")?
            .as_str()
    }

    pub fn evaluated_solids(&self) -> Option<Vec<Manifold>> {
        let nodes = self
            .value
            .pointer(&format!("/items/{}/data/cad/nodes", self.part_index))?
            .as_array()?;
        let extrudes: Vec<_> = nodes
            .iter()
            .filter(|node| node["type"] == "extrude")
            .collect();
        let previews = self.extrusion_previews()?;
        let mut bodies: Vec<Manifold> = Vec::new();
        for (node, preview) in extrudes.into_iter().zip(previews.iter()) {
            let tool = crate::stl::manifold_from_solid(preview).ok()?;
            match node["operation"].as_str()? {
                "newBody" => bodies.push(tool),
                "cut" | "join" => {
                    let previous = bodies.last_mut()?;
                    let result = if node["operation"] == "cut" {
                        previous.difference(&tool)
                    } else {
                        previous.union(&tool)
                    };
                    if result.status() != ManifoldError::NoError || result.is_empty() {
                        return None;
                    }
                    *previous = result;
                }
                _ => return None,
            }
        }
        Some(bodies)
    }

    fn preview_for_extrude(&self, extrude: &Value) -> Option<SolidPreview> {
        if !matches!(
            extrude.get("operation")?.as_str()?,
            "newBody" | "join" | "cut"
        ) {
            return None;
        }
        let sketch_id = extrude.get("sketchId")?.as_str()?;
        let profile_index = extrude
            .get("profileId")?
            .as_str()?
            .strip_prefix(&format!("{sketch_id}-profile-"))?
            .parse::<usize>()
            .ok()?
            .checked_sub(1)?;
        let depth = positive(extrude.get("depth")?)?;
        let workspace = self.sketches.iter().find(|sketch| sketch.id == sketch_id)?;
        let plane = workspace.plane?;
        let loops = profiles_from_document(&workspace.document)?
            .into_iter()
            .nth(profile_index)?;
        let solid = SolidPreview {
            name: extrude
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Extrusion")
                .to_string(),
            profile: loops.outer,
            holes: loops.holes,
            depth,
            plane,
        };
        crate::stl::manifold_from_solid(&solid).ok()?;
        Some(solid)
    }

    #[cfg(test)]
    pub fn has_extrusion(&self) -> bool {
        self.value
            .pointer(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array)
            .is_some_and(|nodes| {
                nodes
                    .iter()
                    .any(|node| node.get("type").and_then(Value::as_str) == Some("extrude"))
            })
    }

    pub fn active_sketch_has_extrusion(&self) -> bool {
        let id = &self.sketches[self.active_sketch].id;
        self.value
            .pointer(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array)
            .is_some_and(|nodes| {
                nodes.iter().any(|node| {
                    node.get("type").and_then(Value::as_str) == Some("extrude")
                        && node.get("sketchId").and_then(Value::as_str) == Some(id)
                })
            })
    }

    #[cfg(test)]
    pub fn set_extrusion_depth(&mut self, depth: f32) -> Result<(), String> {
        self.set_extrusion_depth_at(0, depth)
    }

    pub fn set_extrusion_depth_at(&mut self, index: usize, depth: f32) -> Result<(), String> {
        if !depth.is_finite() || depth <= 0.0 {
            return Err("extrusion depth must be positive".into());
        }
        if self
            .extrusion_previews()
            .is_none_or(|solids| index >= solids.len())
        {
            return Err("this extrusion cannot be edited natively".into());
        }
        let nodes = self
            .value
            .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array_mut)
            .ok_or("project CAD graph changed unexpectedly")?;
        let extrude = nodes
            .iter_mut()
            .filter(|node| node.get("type").and_then(Value::as_str) == Some("extrude"))
            .nth(index)
            .ok_or("extrusion node is missing")?;
        let old_depth = extrude["depth"].clone();
        extrude["depth"] = json!(depth);
        if self.evaluated_solids().is_none() {
            self.value
                .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))
                .and_then(Value::as_array_mut)
                .unwrap()
                .iter_mut()
                .filter(|node| node["type"] == "extrude")
                .nth(index)
                .unwrap()["depth"] = old_depth;
            return Err("depth would make the evaluated solid invalid".into());
        }
        Ok(())
    }

    pub fn delete_extrusion_at(&mut self, index: usize) -> Result<(), String> {
        if self
            .extrusion_previews()
            .is_none_or(|solids| index >= solids.len())
        {
            return Err("this extrusion cannot be deleted natively".into());
        }
        if self
            .read_only_reason
            .as_deref()
            .is_some_and(|reason| reason != "solid features need native support")
        {
            return Err("project has unsupported features".into());
        }
        let data = &self.value["items"][self.part_index]["data"];
        if ["features", "solidSteps", "solids"].iter().any(|key| {
            data.get(*key)
                .and_then(Value::as_array)
                .is_some_and(|values| !values.is_empty())
        }) {
            return Err("project has legacy solid features".into());
        }
        let nodes = data["cad"]["nodes"]
            .as_array()
            .ok_or("project CAD graph is invalid")?;
        let id = nodes
            .iter()
            .filter(|node| node.get("type").and_then(Value::as_str) == Some("extrude"))
            .nth(index)
            .and_then(|node| node.get("id"))
            .and_then(Value::as_str)
            .ok_or("extrusion node is missing")?
            .to_string();
        if nodes
            .iter()
            .filter(|node| node["type"] == "extrude")
            .skip(index + 1)
            .take_while(|node| node["operation"] != "newBody")
            .any(|node| node["operation"] == "cut" || node["operation"] == "join")
        {
            return Err("later cut or join features depend on this extrusion".into());
        }
        if nodes.iter().any(|node| {
            node.get("sourceId").and_then(Value::as_str) == Some(&id)
                || node.get("targetId").and_then(Value::as_str) == Some(&id)
        }) {
            return Err("other features depend on this extrusion".into());
        }
        if !data["cad"]["rootNodeIds"].is_array() || !data["tree"]["orderedNodeIds"].is_array() {
            return Err("project graph or tree is invalid".into());
        }
        self.value["items"][self.part_index]["data"]["cad"]["nodes"]
            .as_array_mut()
            .expect("nodes checked above")
            .retain(|node| node.get("id").and_then(Value::as_str) != Some(&id));
        for path in ["cad/rootNodeIds", "tree/orderedNodeIds"] {
            self.value
                .pointer_mut(&format!("/items/{}/data/{path}", self.part_index))
                .and_then(Value::as_array_mut)
                .expect("lists checked above")
                .retain(|entry| entry.as_str() != Some(&id));
        }
        if !self.has_any_extrusion()
            && self.read_only_reason.as_deref() == Some("solid features need native support")
        {
            self.read_only_reason = None;
        }
        Ok(())
    }

    fn has_any_extrusion(&self) -> bool {
        self.value
            .pointer(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array)
            .is_some_and(|nodes| {
                nodes
                    .iter()
                    .any(|node| node.get("type").and_then(Value::as_str) == Some("extrude"))
            })
    }

    fn all_extrusions_registered(&self) -> bool {
        let data = &self.value["items"][self.part_index]["data"];
        let Some(nodes) = data["cad"]["nodes"].as_array() else {
            return false;
        };
        let Some(roots) = data["cad"]["rootNodeIds"].as_array() else {
            return false;
        };
        let Some(tree) = data["tree"]["orderedNodeIds"].as_array() else {
            return false;
        };
        nodes
            .iter()
            .filter(|node| node.get("type").and_then(Value::as_str) == Some("extrude"))
            .all(|node| {
                node.get("id").and_then(Value::as_str).is_some_and(|id| {
                    roots.iter().any(|entry| entry.as_str() == Some(id))
                        && tree.iter().any(|entry| entry.as_str() == Some(id))
                })
            })
    }

    #[cfg(test)]
    pub fn create_extrusion(&mut self, current: &Document, depth: f32) -> Result<(), String> {
        self.create_extrusion_for_profile(current, depth, 0)
    }

    pub fn create_extrusion_for_profile(
        &mut self,
        current: &Document,
        depth: f32,
        profile_index: usize,
    ) -> Result<(), String> {
        self.create_extrusion_with_operation(current, depth, profile_index, "newBody")
    }

    pub fn create_extrusion_with_operation(
        &mut self,
        current: &Document,
        depth: f32,
        profile_index: usize,
        operation: &str,
    ) -> Result<(), String> {
        if !matches!(operation, "newBody" | "cut" | "join") {
            return Err("unsupported extrusion operation".into());
        }
        if operation != "newBody" && self.evaluated_solids().is_none() {
            return Err("cut and join require an existing solid".into());
        }
        if let Some(reason) = &self.read_only_reason {
            return Err(format!("project is read-only: {reason}"));
        }
        if !depth.is_finite() || depth <= 0.0 {
            return Err("extrusion depth must be positive".into());
        }
        if self.value["items"][self.part_index]["data"]["features"]
            .as_array()
            .is_some_and(|features| !features.is_empty())
        {
            return Err("existing solid features need native support".into());
        }
        if self.value["items"][self.part_index]["data"]["cad"]["rootNodeIds"]
            .as_array()
            .is_none()
            || self.value["items"][self.part_index]["data"]["tree"]["orderedNodeIds"]
                .as_array()
                .is_none()
        {
            return Err("project graph or tree is invalid".into());
        }
        let workspace = &mut self.sketches[self.active_sketch];
        if workspace.plane.is_none()
            || profiles_from_document(current)
                .and_then(|profiles| profiles.get(profile_index).map(|_| ()))
                .is_none()
        {
            return Err("select a valid closed profile on a reference-plane sketch".into());
        }
        workspace.document = current.clone();
        let sketch_id = workspace.id.clone();
        let profile_id = format!("{sketch_id}-profile-{}", profile_index + 1);
        let nodes = self
            .value
            .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array_mut)
            .ok_or("project CAD graph changed unexpectedly")?;
        if nodes.iter().any(|node| {
            node.get("type").and_then(Value::as_str) == Some("extrude")
                && node.get("sketchId").and_then(Value::as_str) == Some(&sketch_id)
                && node.get("profileId").and_then(Value::as_str) == Some(&profile_id)
        }) {
            return Err("this profile already has an extrusion".into());
        }
        let mut index = 1;
        let id = loop {
            let candidate = format!("native-extrude-{index}");
            if !nodes
                .iter()
                .any(|node| node.get("id").and_then(Value::as_str) == Some(candidate.as_str()))
            {
                break candidate;
            }
            index += 1;
        };
        nodes.push(json!({"id":id,"type":"extrude","name":format!("Extrusion {index}"),"sketchId":sketch_id,"profileId":profile_id,"operation":operation,"depth":depth}));
        if self.evaluated_solids().is_none() {
            self.value
                .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))
                .and_then(Value::as_array_mut)
                .expect("nodes checked above")
                .pop();
            return Err("selected sketch does not form a supported closed profile".into());
        }
        self.value["items"][self.part_index]["data"]["cad"]["rootNodeIds"]
            .as_array_mut()
            .ok_or("project root node list is invalid")?
            .push(json!(id));
        self.value["items"][self.part_index]["data"]["tree"]["orderedNodeIds"]
            .as_array_mut()
            .ok_or("project tree is invalid")?
            .push(json!(id));
        Ok(())
    }
    pub fn open(path: impl AsRef<Path>) -> Result<(Self, Document), String> {
        let path = path.as_ref().to_path_buf();
        if path.exists() {
            let source = fs::read(&path).map_err(|error| format!("read project: {error}"))?;
            let value: Value = serde_json::from_slice(&source)
                .map_err(|error| format!("parse project JSON: {error}"))?;
            Self::from_value(path, value, Some(source))
        } else {
            let name = path
                .file_stem()
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("Untitled");
            let sketch_id = "native-sketch-1".to_string();
            let value = json!({
                "version": 4,
                "revision": 0,
                "items": [{
                    "id": "native-part-1",
                    "type": "part",
                    "name": name,
                    "data": {
                        "features": [],
                        "cad": {
                            "nodes": [
                                {"id":"plane-top","type":"referencePlane","name":"Top","plane":"XY"},
                                {"id":"plane-front","type":"referencePlane","name":"Front","plane":"XZ"},
                                {"id":"plane-right","type":"referencePlane","name":"Right","plane":"YZ"},
                                {"id":sketch_id,"type":"sketch","name":"Sketch 1","targetId":"plane-top","relations":[],"variables":[],"dimensions":[]}
                            ],
                            "rootNodeIds": ["plane-top","plane-front","plane-right",sketch_id]
                        },
                        "tree": {"orderedNodeIds": [sketch_id], "dirtySketchIds": []}
                    }
                }],
                "selectedPath": [0]
            });
            Self::from_value(path, value, None)
        }
    }

    fn from_value(
        path: PathBuf,
        value: Value,
        disk_snapshot: Option<Vec<u8>>,
    ) -> Result<(Self, Document), String> {
        if value.get("version").and_then(Value::as_u64) != Some(4) {
            return Err("only PuppyCAD v4 project files are supported".into());
        }
        let items = value
            .get("items")
            .and_then(Value::as_array)
            .ok_or("project has no items array")?;
        let selected_part = value
            .get("selectedPath")
            .and_then(Value::as_array)
            .filter(|path| path.len() == 1)
            .and_then(|path| path[0].as_u64())
            .and_then(|index| usize::try_from(index).ok())
            .filter(|&index| {
                items
                    .get(index)
                    .and_then(|item| item.get("type"))
                    .and_then(Value::as_str)
                    == Some("part")
            });
        let part_index = selected_part
            .or_else(|| {
                items
                    .iter()
                    .position(|item| item.get("type").and_then(Value::as_str) == Some("part"))
            })
            .ok_or("project has no part document")?;
        let part = &items[part_index];
        let data = part.get("data").ok_or("part has no data")?;
        let nodes = data
            .pointer("/cad/nodes")
            .and_then(Value::as_array)
            .ok_or("part has no CAD graph")?;
        let sketches: Vec<&Value> = nodes
            .iter()
            .filter(|node| node.get("type").and_then(Value::as_str) == Some("sketch"))
            .collect();
        if sketches.is_empty() {
            return Err("part has no sketch node".into());
        }

        let mut read_only_reason = None;
        if data
            .get("features")
            .and_then(Value::as_array)
            .is_some_and(|features| !features.is_empty())
            || data
                .get("solidSteps")
                .and_then(Value::as_array)
                .is_some_and(|steps| !steps.is_empty())
            || data
                .get("solids")
                .and_then(Value::as_array)
                .is_some_and(|solids| !solids.is_empty())
            || nodes.iter().any(|node| {
                matches!(
                    node.get("type").and_then(Value::as_str),
                    Some("extrude" | "chamfer")
                )
            })
        {
            read_only_reason = Some("solid features need native support".into());
        }

        let mut workspaces = Vec::new();
        for sketch in sketches {
            let sketch_id = sketch
                .get("id")
                .and_then(Value::as_str)
                .ok_or("sketch has no id")?;
            let sketch_name = sketch
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Sketch");
            let target_id = sketch.get("targetId").and_then(Value::as_str);
            let plane = target_id
                .and_then(|target_id| {
                    nodes
                        .iter()
                        .find(|node| node.get("id").and_then(Value::as_str) == Some(target_id))
                })
                .and_then(SketchPlane::from_graph_node);
            if plane.is_none() {
                read_only_reason =
                    Some("face-based or unsupported sketch plane needs native 3D support".into());
            }
            if sketch
                .get("dimensions")
                .and_then(Value::as_array)
                .is_some_and(|entries| !entries.is_empty())
            {
                read_only_reason = Some("legacy sketch dimensions need native support".into());
            }
            let mut shapes = Vec::new();
            let mut construction = HashSet::new();
            let mut entity_ids = HashMap::new();
            let mut original_entities = HashMap::new();
            for node in nodes {
                if node.get("sketchId").and_then(Value::as_str) != Some(sketch_id) {
                    continue;
                }
                if matches!(
                    node.get("type").and_then(Value::as_str),
                    Some("extrude" | "sketchConstraint")
                ) {
                    continue;
                }
                let id = shapes.len() as u32 + 1;
                match read_shape(node, id) {
                    Some(shape) => {
                        if is_construction_node(node) {
                            construction.insert(id);
                        }
                        let graph_id = node
                            .get("id")
                            .and_then(Value::as_str)
                            .ok_or("sketch entity has no id")?;
                        entity_ids.insert(id, graph_id.to_string());
                        original_entities.insert(id, node.clone());
                        shapes.push(shape);
                    }
                    None => {
                        read_only_reason = Some("unsupported sketch geometry or constraints".into())
                    }
                }
            }

            let mut document = Document::from_shapes(shapes);
            match read_variables(sketch.get("variables")) {
                Ok(variables) => document.dims.variables = variables,
                Err(error) => read_only_reason = Some(error),
            }
            document.construction = construction;
            if let Some(relations) = sketch.get("relations") {
                if let Some(relations) = relations.as_array() {
                    let mut seen = HashSet::new();
                    for relation in relations {
                        if relation.get("type").and_then(Value::as_str) == Some("fixed") {
                            let graph_id =
                                relation.pointer("/anchor/entityId").and_then(Value::as_str);
                            let shape_id = graph_id.and_then(|graph_id| {
                                entity_ids
                                    .iter()
                                    .find_map(|(id, value)| (value == graph_id).then_some(*id))
                            });
                            let index = shape_id.and_then(|id| {
                                let shape =
                                    document.shapes.iter().find(|shape| shape.id() == id)?;
                                let anchor = relation.pointer("/anchor/point")?.as_str()?;
                                match shape {
                                    Shape::Point { .. } if anchor == "center" => Some(0),
                                    Shape::Arc { .. } | Shape::EllipticArc { .. } => match anchor {
                                        "center" => Some(0),
                                        "p0" => Some(1),
                                        "p1" => Some(2),
                                        _ => None,
                                    },
                                    Shape::Circle { .. } if anchor == "center" => Some(0),
                                    Shape::Ellipse { .. } | Shape::RotatedRectangle { .. } => {
                                        if anchor == "center" {
                                            Some(0)
                                        } else {
                                            anchor
                                                .strip_prefix('p')?
                                                .parse::<usize>()
                                                .ok()
                                                .filter(|index| *index < 4)
                                                .map(|index| index + 1)
                                        }
                                    }
                                    Shape::Polygon { sides, .. } => {
                                        if anchor == "center" {
                                            Some(0)
                                        } else {
                                            anchor
                                                .strip_prefix("vertex")?
                                                .parse::<usize>()
                                                .ok()
                                                .filter(|index| *index < *sides as usize)
                                                .map(|index| index + 1)
                                        }
                                    }
                                    Shape::Capsule { .. } => match anchor {
                                        "from" => Some(0),
                                        "to" => Some(1),
                                        _ => None,
                                    },
                                    Shape::Line { .. } | Shape::Rectangle { .. } => match anchor {
                                        "p0" => Some(0),
                                        "p1" => Some(1),
                                        _ => None,
                                    },
                                    Shape::Spline { points, .. } => anchor
                                        .strip_prefix("point")?
                                        .parse::<usize>()
                                        .ok()
                                        .filter(|index| *index < points.len()),
                                    _ => None,
                                }
                            });
                            let position = relation.get("position").and_then(read_point);
                            let valid = shape_id.zip(index).zip(position).is_some_and(
                                |((id, index), position)| {
                                    document
                                        .fixed_anchor_position(id, index)
                                        .is_some_and(|actual| actual.distance(position) <= 1e-4)
                                },
                            );
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || relation
                                    .get("fixation")
                                    .and_then(Value::as_str)
                                    .is_some_and(|fixation| Some(fixation) != graph_id)
                                || !valid
                            {
                                read_only_reason = Some("unsupported fixed point relation".into());
                            } else if let Some(((id, index), position)) =
                                shape_id.zip(index).zip(position)
                                && document
                                    .fixed_anchors
                                    .insert((id, index), position)
                                    .is_some()
                            {
                                read_only_reason = Some("duplicate fixed point relation".into());
                            }
                            continue;
                        }
                        let graph_shape = |graph_id: Option<&str>| -> Option<u32> {
                            let graph_id = graph_id?;
                            entity_ids
                                .iter()
                                .find_map(|(id, value)| (value == graph_id).then_some(*id))
                        };
                        let end_index = |point: Option<&str>| match point {
                            Some("p0") => Some(0u8),
                            Some("p1") => Some(1u8),
                            _ => None,
                        };
                        if relation.get("type").and_then(Value::as_str) == Some("smoothJoin") {
                            let side = |key: &str| {
                                let id = graph_shape(
                                    relation
                                        .pointer(&format!("/{key}/entityId"))
                                        .and_then(Value::as_str),
                                )?;
                                let shape = document.shapes.iter().find(|s| s.id() == id)?;
                                let name = relation
                                    .pointer(&format!("/{key}/point"))
                                    .and_then(Value::as_str)?;
                                curve_end_index(shape, name).map(|end| (id, end))
                            };
                            match side("a").zip(side("b")) {
                                Some((a, b))
                                    if a.0 != b.0
                                        && relation.get("id").and_then(Value::as_str).is_some()
                                        && !document.smooth_joins.contains_key(&b) =>
                                {
                                    document.smooth_joins.insert(b, a);
                                }
                                _ => {
                                    read_only_reason =
                                        Some("unsupported smooth join relation".into());
                                }
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("endpointTangent") {
                            let arc = graph_shape(
                                relation.pointer("/a/entityId").and_then(Value::as_str),
                            );
                            let endpoint =
                                end_index(relation.pointer("/a/point").and_then(Value::as_str));
                            let line = graph_shape(relation.get("b").and_then(Value::as_str));
                            let sign = |key: &str| {
                                relation
                                    .pointer(&format!("/orientation/{key}"))
                                    .and_then(Value::as_f64)
                                    .map(|value| if value < 0.0 { -1i8 } else { 1i8 })
                            };
                            let shape = |id: Option<u32>| {
                                id.and_then(|id| {
                                    document.shapes.iter().find(|shape| shape.id() == id)
                                })
                            };
                            match (
                                shape(arc),
                                endpoint,
                                shape(line),
                                sign("lineSign"),
                                sign("sweepSign"),
                            ) {
                                (
                                    Some(Shape::Arc { id: arc, .. }),
                                    Some(endpoint),
                                    Some(Shape::Line { id: line, .. }),
                                    Some(line_sign),
                                    Some(sweep_sign),
                                ) if relation.get("id").and_then(Value::as_str).is_some()
                                    && !document
                                        .endpoint_tangents
                                        .contains_key(&(*arc, endpoint)) =>
                                {
                                    document
                                        .endpoint_tangents
                                        .insert((*arc, endpoint), (*line, line_sign, sweep_sign));
                                }
                                _ => {
                                    read_only_reason =
                                        Some("unsupported smooth join relation".into());
                                }
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("chamfer") {
                            let line =
                                graph_shape(relation.get("chamferId").and_then(Value::as_str));
                            let side = |key: &str| {
                                graph_shape(
                                    relation
                                        .pointer(&format!("/{key}/entityId"))
                                        .and_then(Value::as_str),
                                )
                                .zip(end_index(
                                    relation
                                        .pointer(&format!("/{key}/point"))
                                        .and_then(Value::as_str),
                                ))
                            };
                            let first = relation.get("value").and_then(Value::as_f64);
                            let second = relation
                                .get("secondValue")
                                .and_then(Value::as_f64)
                                .or(first);
                            let mode = match relation.get("mode").and_then(Value::as_str) {
                                None => Some(ChamferMode::TwoDistances),
                                Some(name) => ChamferMode::from_name(name),
                            };
                            let is_line = |id: u32| {
                                document.shapes.iter().any(|shape| {
                                    shape.id() == id && matches!(shape, Shape::Line { .. })
                                })
                            };
                            match (line, side("a"), side("b"), first, second, mode) {
                                (
                                    Some(line),
                                    Some(a),
                                    Some(b),
                                    Some(first),
                                    Some(second),
                                    Some(mode),
                                ) if [line, a.0, b.0].into_iter().all(is_line)
                                    && first.is_finite()
                                    && second.is_finite()
                                    && first > 0.0
                                    && second > 0.0
                                    && relation.get("id").and_then(Value::as_str).is_some()
                                    && relation.get("expression").is_none()
                                    && relation.get("reference") != Some(&Value::Bool(true))
                                    && !document.chamfers.contains_key(&line) =>
                                {
                                    document.chamfers.insert(
                                        line,
                                        ChamferSpec {
                                            a,
                                            b,
                                            first: first as f32,
                                            second: second as f32,
                                            mode,
                                        },
                                    );
                                }
                                _ => {
                                    read_only_reason = Some("unsupported chamfer relation".into());
                                }
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("coincident")
                            && relation.pointer("/a/point").and_then(Value::as_str)
                                != Some("center")
                            && relation.pointer("/b/point").and_then(Value::as_str)
                                != Some("center")
                        {
                            let side = |key: &str| {
                                let id = graph_shape(
                                    relation
                                        .pointer(&format!("/{key}/entityId"))
                                        .and_then(Value::as_str),
                                )?;
                                let end = end_index(
                                    relation
                                        .pointer(&format!("/{key}/point"))
                                        .and_then(Value::as_str),
                                )?;
                                let shape =
                                    document.shapes.iter().find(|shape| shape.id() == id)?;
                                let at = match shape {
                                    Shape::Line { a, b, .. } => {
                                        if end == 0 {
                                            *a
                                        } else {
                                            *b
                                        }
                                    }
                                    Shape::Arc {
                                        center,
                                        radius,
                                        start_angle,
                                        sweep,
                                        ..
                                    } => {
                                        let angle =
                                            start_angle + if end == 0 { 0.0 } else { *sweep };
                                        Point {
                                            x: center.x + radius * angle.cos() as f32,
                                            y: center.y + radius * angle.sin() as f32,
                                        }
                                    }
                                    _ => return None,
                                };
                                Some(((id, end), at, matches!(shape, Shape::Line { .. })))
                            };
                            let join = side("a").zip(side("b")).and_then(|(a, b)| {
                                if a.1.distance(b.1) > 1e-3 || a.0.0 == b.0.0 {
                                    return None;
                                }
                                [(a, b), (b, a)].into_iter().find_map(|(key, value)| {
                                    (key.2 && !document.endpoint_joins.contains_key(&key.0))
                                        .then_some((key.0, value.0))
                                })
                            });
                            match join {
                                Some((key, value))
                                    if relation.get("id").and_then(Value::as_str).is_some()
                                        && relation.get("reference")
                                            != Some(&Value::Bool(true))
                                        && relation.get("expression").is_none() =>
                                {
                                    document.endpoint_joins.insert(key, value);
                                }
                                _ => {
                                    read_only_reason =
                                        Some("unsupported sketch coincidence relation".into());
                                }
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("distance") {
                            if let Err(error) =
                                load_point_dimension(&mut document, relation, &entity_ids)
                            {
                                read_only_reason = Some(error);
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("coincident") {
                            let read_anchor = |side: &str| -> Option<(u32, &str)> {
                                let graph_id = relation.get(side)?.get("entityId")?.as_str()?;
                                let shape_id = entity_ids
                                    .iter()
                                    .find_map(|(id, value)| (value == graph_id).then_some(*id))?;
                                let point = relation.get(side)?.get("point")?.as_str()?;
                                Some((shape_id, point))
                            };
                            let pair = read_anchor("a").zip(read_anchor("b"));
                            let link = pair.and_then(|(a, b)| {
                                for (line, point) in [(a, b), (b, a)] {
                                    if point.1 != "center" {
                                        continue;
                                    }
                                    let line_shape = document
                                        .shapes
                                        .iter()
                                        .find(|shape| shape.id() == line.0)?;
                                    let point_shape = document
                                        .shapes
                                        .iter()
                                        .find(|shape| shape.id() == point.0)?;
                                    let endpoint_position = match line_shape {
                                        Shape::Line { a, b, .. } => match line.1 {
                                            "p0" => Some((0, *a)),
                                            "p1" => Some((1, *b)),
                                            _ => None,
                                        },
                                        Shape::Spline {
                                            points,
                                            closed: false,
                                            ..
                                        } => {
                                            if line.1 == "point0" {
                                                points.first().copied().map(|at| (0, at))
                                            } else if line.1 == format!("point{}", points.len() - 1)
                                            {
                                                points.last().copied().map(|at| (1, at))
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    };
                                    if let (Some((endpoint, position)), Shape::Point { at, .. }) =
                                        (endpoint_position, point_shape)
                                        && position.distance(*at) < 1e-3
                                    {
                                        return Some(((line.0, endpoint), point.0));
                                    }
                                }
                                None
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || link.is_none()
                            {
                                read_only_reason =
                                    Some("unsupported sketch coincidence relation".into());
                            } else if let Some((key, point_id)) = link
                                && document.point_links.insert(key, point_id).is_some()
                            {
                                read_only_reason =
                                    Some("duplicate sketch coincidence relation".into());
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("normal") {
                            let target_graph_id = relation
                                .get("a")
                                .and_then(|anchor| anchor.get("entityId"))
                                .and_then(Value::as_str);
                            let source_graph_id = relation.get("b").and_then(Value::as_str);
                            let target_id = target_graph_id.and_then(|id| {
                                entity_ids.iter().find_map(|(shape_id, graph_id)| {
                                    (graph_id == id).then_some(*shape_id)
                                })
                            });
                            let source_id = source_graph_id.and_then(|id| {
                                entity_ids.iter().find_map(|(shape_id, graph_id)| {
                                    (graph_id == id).then_some(*shape_id)
                                })
                            });
                            let endpoint = match relation
                                .get("a")
                                .and_then(|anchor| anchor.get("point"))
                                .and_then(Value::as_str)
                            {
                                Some("p0") => Some(0),
                                Some("p1") => Some(1),
                                _ => None,
                            };
                            let valid = target_id.zip(source_id).zip(endpoint).is_some_and(
                                |((target, source), endpoint)| {
                                    target != source
                                        && !document.normal_links.contains_key(&target)
                                        && document
                                            .shapes
                                            .iter()
                                            .find(|shape| shape.id() == source)
                                            .zip(
                                                document
                                                    .shapes
                                                    .iter()
                                                    .find(|shape| shape.id() == target),
                                            )
                                            .and_then(|(source_shape, target_shape)| {
                                                Document::normal_line_target(
                                                    source_shape,
                                                    target_shape,
                                                    endpoint,
                                                )
                                                .map(|adjusted| (target_shape, adjusted))
                                            })
                                            .is_some_and(|(original, adjusted)| {
                                                match (original, adjusted) {
                                                    (
                                                        Shape::Line { a, b, .. },
                                                        Shape::Line {
                                                            a: next_a,
                                                            b: next_b,
                                                            ..
                                                        },
                                                    ) => {
                                                        a.distance(next_a) < 1e-3
                                                            && b.distance(next_b) < 1e-3
                                                    }
                                                    _ => false,
                                                }
                                            })
                                },
                            );
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || !valid
                            {
                                read_only_reason = Some("unsupported normal relation".into());
                            } else if let Some(((target, source), endpoint)) =
                                target_id.zip(source_id).zip(endpoint)
                            {
                                document.normal_links.insert(target, (source, endpoint));
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("pointOnCurve") {
                            let point_graph_id = relation
                                .get("a")
                                .and_then(|anchor| anchor.get("entityId"))
                                .and_then(Value::as_str);
                            let curve_graph_id = relation.get("b").and_then(Value::as_str);
                            let point_id = point_graph_id.and_then(|graph_id| {
                                entity_ids.iter().find_map(|(shape_id, id)| {
                                    (id == graph_id).then_some(*shape_id)
                                })
                            });
                            let curve_id = curve_graph_id.and_then(|graph_id| {
                                entity_ids.iter().find_map(|(shape_id, id)| {
                                    (id == graph_id).then_some(*shape_id)
                                })
                            });
                            let valid = point_id.zip(curve_id).is_some_and(|(point_id, curve_id)| {
                                if point_id == curve_id || document.point_curve_links.contains_key(&point_id) { return false; }
                                let point = document.shapes.iter().find(|shape| shape.id() == point_id);
                                let curve = document.shapes.iter().find(|shape| shape.id() == curve_id);
                                matches!(point, Some(Shape::Point { at, .. }) if curve.and_then(|curve| Document::closest_curve_point(curve, *at)).is_some_and(|projected| at.distance(projected) < 1e-3))
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || relation
                                    .get("a")
                                    .and_then(|anchor| anchor.get("point"))
                                    .and_then(Value::as_str)
                                    != Some("center")
                                || !valid
                            {
                                read_only_reason =
                                    Some("unsupported point on curve relation".into());
                            } else if let Some((point_id, curve_id)) = point_id.zip(curve_id) {
                                document.point_curve_links.insert(point_id, curve_id);
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("midpoint") {
                            let point_graph_id = relation
                                .get("a")
                                .and_then(|anchor| anchor.get("entityId"))
                                .and_then(Value::as_str);
                            let curve_graph_id = relation.get("b").and_then(Value::as_str);
                            let point_id = point_graph_id.and_then(|graph_id| {
                                entity_ids.iter().find_map(|(shape_id, id)| {
                                    (id == graph_id).then_some(*shape_id)
                                })
                            });
                            let curve_id = curve_graph_id.and_then(|graph_id| {
                                entity_ids.iter().find_map(|(shape_id, id)| {
                                    (id == graph_id).then_some(*shape_id)
                                })
                            });
                            let valid = point_id.zip(curve_id).is_some_and(|(point_id, curve_id)| {
                                if point_id == curve_id || document.midpoint_links.contains_key(&point_id) { return false; }
                                let point = document.shapes.iter().find(|shape| shape.id() == point_id);
                                let curve = document.shapes.iter().find(|shape| shape.id() == curve_id);
                                matches!(point, Some(Shape::Point { at, .. }) if curve.and_then(Document::curve_midpoint).is_some_and(|middle| at.distance(middle) < 1e-3))
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || relation
                                    .get("a")
                                    .and_then(|anchor| anchor.get("point"))
                                    .and_then(Value::as_str)
                                    != Some("center")
                                || !valid
                            {
                                read_only_reason = Some("unsupported midpoint relation".into());
                            } else if let Some((point_id, curve_id)) = point_id.zip(curve_id) {
                                document.midpoint_links.insert(point_id, curve_id);
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("sameEllipse") {
                            let resolve = |side: &str| {
                                relation.get(side)?.as_str().and_then(|graph_id| {
                                    entity_ids.iter().find_map(|(shape_id, id)| {
                                        (id == graph_id).then_some(*shape_id)
                                    })
                                })
                            };
                            let pair = resolve("a").zip(resolve("b"));
                            let valid = pair.is_some_and(|(source, target)| {
                                if source == target || document.shared_ellipses.contains_key(&target)
                                    || document.shared_ellipses.values().any(|old_source| *old_source == target) { return false; }
                                matches!((document.shapes.iter().find(|shape| shape.id() == source), document.shapes.iter().find(|shape| shape.id() == target)),
                                    (Some(Shape::Ellipse { center: ac, rx: ax, ry: ay, rotation: ar, .. } | Shape::EllipticArc { center: ac, rx: ax, ry: ay, rotation: ar, .. }),
                                     Some(Shape::Ellipse { center: bc, rx: bx, ry: by, rotation: br, .. } | Shape::EllipticArc { center: bc, rx: bx, ry: by, rotation: br, .. }))
                                    if ac.distance(*bc) < 1e-3 && (*ax - *bx).abs() < 1e-3 && (*ay - *by).abs() < 1e-3
                                        && ((*ar - *br).to_radians().sin()).atan2((*ar - *br).to_radians().cos()).abs() < 1e-3)
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || !valid
                            {
                                read_only_reason =
                                    Some("unsupported shared ellipse relation".into());
                            } else if let Some((source, target)) = pair {
                                let root = document
                                    .shared_ellipses
                                    .get(&source)
                                    .copied()
                                    .unwrap_or(source);
                                document.shared_ellipses.insert(target, root);
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("tangent") {
                            let resolve = |side: &str| {
                                relation.get(side)?.as_str().and_then(|graph_id| {
                                    entity_ids.iter().find_map(|(shape_id, id)| {
                                        (id == graph_id).then_some(*shape_id)
                                    })
                                })
                            };
                            let pair = resolve("a").zip(resolve("b"));
                            let valid = pair.is_some_and(|(source, target)| {
                                source != target
                                    && !document.external_tangents.iter().any(
                                        |(&old_target, &old_source)| {
                                            [old_target, old_source].contains(&source)
                                                || [old_target, old_source].contains(&target)
                                        },
                                    )
                                    && document
                                        .shapes
                                        .iter()
                                        .find(|shape| shape.id() == source)
                                        .zip(
                                            document
                                                .shapes
                                                .iter()
                                                .find(|shape| shape.id() == target),
                                        )
                                        .is_some_and(|(a, b)| external_tangent_matches(a, b))
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || !valid
                            {
                                read_only_reason =
                                    Some("unsupported external tangent relation".into());
                            } else if let Some((source, target)) = pair {
                                document.external_tangents.insert(target, source);
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("internalTangent") {
                            let resolve = |side: &str| {
                                relation.get(side)?.as_str().and_then(|graph_id| {
                                    entity_ids.iter().find_map(|(shape_id, id)| {
                                        (id == graph_id).then_some(*shape_id)
                                    })
                                })
                            };
                            let pair = resolve("a").zip(resolve("b"));
                            let valid = pair.is_some_and(|(outer, inner)| {
                                outer != inner
                                    && !document.internal_tangents.iter().any(
                                        |(&old_inner, &old_outer)| {
                                            [old_inner, old_outer].contains(&outer)
                                                || [old_inner, old_outer].contains(&inner)
                                        },
                                    )
                                    && document
                                        .shapes
                                        .iter()
                                        .find(|shape| shape.id() == outer)
                                        .zip(
                                            document
                                                .shapes
                                                .iter()
                                                .find(|shape| shape.id() == inner),
                                        )
                                        .is_some_and(|(a, b)| internal_tangent_matches(a, b))
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || !valid
                            {
                                read_only_reason =
                                    Some("unsupported internal tangent relation".into());
                            } else if let Some((outer, inner)) = pair {
                                document.internal_tangents.insert(inner, outer);
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("radiusDifference")
                        {
                            let resolve = |side: &str| {
                                relation.get(side)?.as_str().and_then(|graph_id| {
                                    entity_ids.iter().find_map(|(shape_id, id)| {
                                        (id == graph_id).then_some(*shape_id)
                                    })
                                })
                            };
                            let pair = resolve("a").zip(resolve("b"));
                            let value = relation
                                .get("value")
                                .and_then(Value::as_f64)
                                .filter(|value| {
                                    value.is_finite()
                                        && *value >= f32::MIN as f64
                                        && *value <= f32::MAX as f64
                                })
                                .map(|value| value as f32);
                            let valid = pair.zip(value).is_some_and(|((source, target), value)| {
                                source != target
                                    && !document.radius_differences.iter().any(|(&old_target, &(old_source, _))| [old_source, old_target].contains(&source) || [old_source, old_target].contains(&target))
                                    && !document.equal_radii.iter().any(|(&old_target, &old_source)| [old_source, old_target].contains(&source) || [old_source, old_target].contains(&target))
                                    && matches!((document.shapes.iter().find(|shape| shape.id() == source), document.shapes.iter().find(|shape| shape.id() == target)),
                                        (Some(Shape::Circle { radius: a, .. }), Some(Shape::Circle { radius: b, .. })) if (*a - *b - value).abs() < 1e-3)
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || !valid
                            {
                                read_only_reason =
                                    Some("unsupported radial distance relation".into());
                            } else if let Some(((source, target), value)) = pair.zip(value) {
                                document.radius_differences.insert(target, (source, value));
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("equal") {
                            let resolve = |side: &str| {
                                relation.get(side)?.as_str().and_then(|graph_id| {
                                    entity_ids.iter().find_map(|(shape_id, id)| {
                                        (id == graph_id).then_some(*shape_id)
                                    })
                                })
                            };
                            let pair = resolve("a").zip(resolve("b"));
                            let valid = pair.is_some_and(|(source, target)| {
                                if source == target || document.equal_lines.iter().chain(document.equal_radii.iter()).chain(document.equal_ellipses.iter()).any(|(&old_target, &old_source)|
                                    source == old_source || source == old_target || target == old_source || target == old_target) {
                                    return false;
                                }
                                let source_shape = document.shapes.iter().find(|shape| shape.id() == source);
                                let target_shape = document.shapes.iter().find(|shape| shape.id() == target);
                                matches!((source_shape, target_shape),
                                    (Some(Shape::Line { a, b, .. }), Some(Shape::Line { a: c, b: d, .. }))
                                    if (a.distance(*b) - c.distance(*d)).abs() < 1e-3)
                                    || matches!((source_shape, target_shape),
                                        (Some(Shape::Circle { radius: a, .. } | Shape::Arc { radius: a, .. }), Some(Shape::Circle { radius: b, .. } | Shape::Arc { radius: b, .. })) if (*a - *b).abs() < 1e-3)
                                    || matches!((source_shape, target_shape),
                                        (Some(Shape::Ellipse { rx: ax, ry: ay, .. } | Shape::EllipticArc { rx: ax, ry: ay, .. }), Some(Shape::Ellipse { rx: bx, ry: by, .. } | Shape::EllipticArc { rx: bx, ry: by, .. })) if (*ax - *bx).abs() < 1e-3 && (*ay - *by).abs() < 1e-3)
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || !valid
                            {
                                read_only_reason = Some("unsupported equal relation".into());
                            } else if let Some((source, target)) = pair {
                                if document.shapes.iter().any(|shape| {
                                    shape.id() == target && matches!(shape, Shape::Line { .. })
                                }) {
                                    document.equal_lines.insert(target, source);
                                } else if document.shapes.iter().any(|shape| {
                                    shape.id() == target
                                        && matches!(shape, Shape::Circle { .. } | Shape::Arc { .. })
                                }) {
                                    document.equal_radii.insert(target, source);
                                } else {
                                    document.equal_ellipses.insert(target, source);
                                }
                            }
                            continue;
                        }
                        if matches!(
                            relation.get("type").and_then(Value::as_str),
                            Some("offset" | "offsetChain")
                        ) {
                            if !load_offset_relation(relation, &entity_ids, &mut document) {
                                read_only_reason = Some("unsupported offset relation".into());
                            }
                            continue;
                        }
                        if matches!(
                            relation.get("type").and_then(Value::as_str),
                            Some("symmetric" | "mirror")
                        ) {
                            let graph_shape = |graph_id: Option<&str>| {
                                graph_id.and_then(|graph_id| {
                                    entity_ids.iter().find_map(|(shape_id, id)| {
                                        (id == graph_id).then_some(*shape_id)
                                    })
                                })
                            };
                            let axis =
                                graph_shape(relation.get("symmetryLine").and_then(Value::as_str));
                            let plain = relation.get("id").and_then(Value::as_str).is_some()
                                && relation.get("reference") != Some(&Value::Bool(true))
                                && relation.get("expression").is_none();
                            let loaded = if relation["type"] == "symmetric" {
                                let read_anchor = |side: &str| {
                                    let id = graph_shape(
                                        relation.pointer(&format!("/{side}/entityId"))?.as_str(),
                                    )?;
                                    let name =
                                        relation.pointer(&format!("/{side}/point"))?.as_str()?;
                                    let shape =
                                        document.shapes.iter().find(|shape| shape.id() == id)?;
                                    Some((id, symmetry_anchor_index(shape, name)?))
                                };
                                match (read_anchor("a"), read_anchor("b"), axis) {
                                    (Some(a), Some(b), Some(axis))
                                        if plain && a != b && a.0 != axis && b.0 != axis =>
                                    {
                                        document
                                            .symmetric_points
                                            .push(crate::model::SymmetricPoints { a, b, axis });
                                        true
                                    }
                                    _ => false,
                                }
                            } else {
                                let source = graph_shape(relation.get("a").and_then(Value::as_str));
                                let target = graph_shape(relation.get("b").and_then(Value::as_str));
                                match (source, target, axis) {
                                    (Some(source), Some(target), Some(axis))
                                        if plain
                                            && source != target
                                            && source != axis
                                            && target != axis
                                            && !document.mirrors.contains_key(&target)
                                            && mirror_kinds_match(
                                                &document, source, target, axis,
                                            ) =>
                                    {
                                        document.mirrors.insert(target, (source, axis));
                                        true
                                    }
                                    _ => false,
                                }
                            };
                            if !loaded {
                                read_only_reason = Some("unsupported symmetry relation".into());
                            }
                            continue;
                        }
                        if relation.get("type").and_then(Value::as_str) == Some("concentric") {
                            let resolve = |side: &str| {
                                relation.get(side)?.as_str().and_then(|graph_id| {
                                    entity_ids.iter().find_map(|(shape_id, id)| {
                                        (id == graph_id).then_some(*shape_id)
                                    })
                                })
                            };
                            let pair = resolve("a").zip(resolve("b"));
                            let valid = pair.is_some_and(|(source, target)| {
                                if source == target
                                    || document.concentric.iter().any(
                                        |(&old_target, &old_source)| {
                                            source == old_source
                                                || source == old_target
                                                || target == old_source
                                                || target == old_target
                                        },
                                    )
                                {
                                    return false;
                                }
                                let source_shape =
                                    document.shapes.iter().find(|shape| shape.id() == source);
                                let target_shape =
                                    document.shapes.iter().find(|shape| shape.id() == target);
                                let circular = |shape: &Shape| {
                                    matches!(
                                        shape,
                                        Shape::Circle { .. }
                                            | Shape::Arc { .. }
                                            | Shape::Ellipse { .. }
                                            | Shape::EllipticArc { .. }
                                    )
                                };
                                source_shape.zip(target_shape).is_some_and(|(a, b)| {
                                    circular(a)
                                        && circular(b)
                                        && a.center().distance(b.center()) < 1e-3
                                })
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || !valid
                            {
                                read_only_reason = Some("unsupported concentric relation".into());
                            } else if let Some((source, target)) = pair {
                                document.concentric.insert(target, source);
                            }
                            continue;
                        }
                        if let Some(kind) = match relation.get("type").and_then(Value::as_str) {
                            Some("parallel") => Some(LineRelationKind::Parallel),
                            Some("perpendicular") => Some(LineRelationKind::Perpendicular),
                            Some("collinear") => Some(LineRelationKind::Collinear),
                            Some("angle") => relation
                                .get("value")
                                .and_then(Value::as_f64)
                                .filter(|value| {
                                    value.is_finite()
                                        && *value >= f32::MIN as f64
                                        && *value <= f32::MAX as f64
                                })
                                .map(|value| LineRelationKind::Angle(value as f32)),
                            _ => None,
                        } {
                            let resolve = |side: &str| {
                                relation.get(side)?.as_str().and_then(|graph_id| {
                                    entity_ids.iter().find_map(|(shape_id, id)| {
                                        (id == graph_id).then_some(*shape_id)
                                    })
                                })
                            };
                            let pair = resolve("a").zip(resolve("b"));
                            let valid = pair.is_some_and(|(source, target)| {
                                if source == target
                                    || document.line_relations.iter().any(
                                        |(&old_target, &(old_source, _))| {
                                            source == old_source
                                                || source == old_target
                                                || target == old_source
                                                || target == old_target
                                        },
                                    )
                                {
                                    return false;
                                }
                                let a = document.shapes.iter().find(|shape| shape.id() == source);
                                let b = document.shapes.iter().find(|shape| shape.id() == target);
                                a.zip(b)
                                    .is_some_and(|(a, b)| line_relation_matches(a, b, kind))
                            });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || (relation.get("expression").is_some()
                                    && kind.angle_value().is_none())
                                || !valid
                            {
                                read_only_reason =
                                    Some("unsupported line direction relation".into());
                            } else if let Some((source, target)) = pair {
                                document.line_relations.insert(target, (source, kind));
                            }
                            continue;
                        }
                        if let Some(axis) = match relation.get("type").and_then(Value::as_str) {
                            Some("horizontal") => Some(AxisConstraint::Horizontal),
                            Some("vertical") => Some(AxisConstraint::Vertical),
                            _ => None,
                        } {
                            let graph_id = relation.get("entityId").and_then(Value::as_str);
                            let shape_id = graph_id.and_then(|graph_id| {
                                entity_ids.iter().find_map(|(shape_id, id)| {
                                    (id == graph_id).then_some(*shape_id)
                                })
                            });
                            let valid = shape_id
                                .and_then(|id| {
                                    document.shapes.iter().find(|shape| shape.id() == id)
                                })
                                .is_some_and(|shape| match shape {
                                    Shape::Line { a, b, .. } => match axis {
                                        AxisConstraint::Horizontal => (a.y - b.y).abs() < 1e-3,
                                        AxisConstraint::Vertical => (a.x - b.x).abs() < 1e-3,
                                    },
                                    _ => false,
                                });
                            if relation.get("id").and_then(Value::as_str).is_none()
                                || relation.get("reference") == Some(&Value::Bool(true))
                                || relation.get("expression").is_some()
                                || !valid
                                || shape_id
                                    .is_some_and(|id| document.axis_constraints.contains_key(&id))
                            {
                                read_only_reason = Some("unsupported sketch axis relation".into());
                            } else if let Some(id) = shape_id {
                                document.axis_constraints.insert(id, axis);
                            }
                            continue;
                        }
                        let field = relation
                            .get("type")
                            .and_then(Value::as_str)
                            .and_then(relation_field);
                        let graph_id = relation.get("entityId").and_then(Value::as_str);
                        let value = relation.get("value").and_then(Value::as_f64);
                        let shape_id = graph_id.and_then(|graph_id| {
                            entity_ids
                                .iter()
                                .find_map(|(shape_id, id)| (id == graph_id).then_some(*shape_id))
                        });
                        if relation.get("reference") == Some(&Value::Bool(true)) {
                            // Reference dimensions only measure.
                            let key = field
                                .zip(shape_id)
                                .map(|(field, id)| DimensionKey::Field(id, field));
                            let valid = relation.get("id").and_then(Value::as_str).is_some()
                                && relation.get("expression").is_none()
                                && value.is_some_and(f64::is_finite)
                                && field.zip(shape_id).is_some_and(|(field, id)| {
                                    !document.driving_dimensions.contains_key(&(id, field))
                                        && document
                                            .measure_dimension(DimensionKey::Field(id, field))
                                            .is_some()
                                });
                            match key {
                                Some(key) if valid && document.dims.references.insert(key) => {}
                                _ => {
                                    read_only_reason =
                                        Some("unsupported reference dimension".into())
                                }
                            }
                            continue;
                        }
                        if relation
                            .get("reference")
                            .is_some_and(|value| value != false)
                            || relation.get("id").and_then(Value::as_str).is_none()
                            || field.is_none()
                            || shape_id.is_none()
                            || value.is_none_or(|value| {
                                !value.is_finite()
                                    || match field {
                                        Some(DimensionField::ArcSweep) => {
                                            value == 0.0 || value.abs() >= 360.0
                                        }
                                        Some(DimensionField::Rotation) => false,
                                        _ => value <= 0.0,
                                    }
                            })
                        {
                            read_only_reason = Some("unsupported sketch relation".into());
                            continue;
                        }
                        let field = field.expect("checked above");
                        let shape_id = shape_id.expect("checked above");
                        if !seen.insert((shape_id, field)) {
                            read_only_reason = Some("duplicate sketch dimension relation".into());
                            continue;
                        }
                        document.selected = Some(shape_id);
                        if document
                            .set_dimension(field, value.expect("checked above") as f32)
                            .is_err()
                        {
                            read_only_reason = Some("unsupported sketch relation target".into());
                        }
                    }
                } else {
                    read_only_reason = Some("invalid sketch relations".into());
                }
            }
            if let Some(relations) = sketch.get("relations").and_then(Value::as_array) {
                let graph_shape_ids: HashMap<&str, u32> = entity_ids
                    .iter()
                    .map(|(shape_id, graph_id)| (graph_id.as_str(), *shape_id))
                    .collect();
                if let Err(error) = load_dimension_meta(&mut document, relations, &graph_shape_ids)
                {
                    read_only_reason = Some(error);
                }
            }
            let mut seen_graph_constraints = HashSet::new();
            for node in nodes.iter().filter(|node| {
                node.get("type").and_then(Value::as_str) == Some("sketchConstraint")
                    && node.get("sketchId").and_then(Value::as_str) == Some(sketch_id)
            }) {
                let constraint = &node["constraint"];
                let kind = constraint.get("type").and_then(Value::as_str);
                let graph_id = constraint.get("entityId").and_then(Value::as_str);
                let shape_id = graph_id.and_then(|graph_id| {
                    entity_ids
                        .iter()
                        .find_map(|(shape_id, id)| (id == graph_id).then_some(*shape_id))
                });
                let Some((kind, shape_id)) = kind.zip(shape_id) else {
                    read_only_reason = Some("unsupported graph sketch constraint".into());
                    continue;
                };
                if node.get("id").and_then(Value::as_str).is_none()
                    || !seen_graph_constraints.insert((shape_id, kind))
                {
                    read_only_reason = Some("duplicate or invalid graph sketch constraint".into());
                    continue;
                }
                if let Some(axis) = match kind {
                    "horizontal" => Some(AxisConstraint::Horizontal),
                    "vertical" => Some(AxisConstraint::Vertical),
                    _ => None,
                } {
                    let valid = document
                        .shapes
                        .iter()
                        .find(|shape| shape.id() == shape_id)
                        .is_some_and(|shape| match shape {
                            Shape::Line { a, b, .. } => match axis {
                                AxisConstraint::Horizontal => (a.y - b.y).abs() < 1e-3,
                                AxisConstraint::Vertical => (a.x - b.x).abs() < 1e-3,
                            },
                            _ => false,
                        });
                    if !valid
                        || document
                            .axis_constraints
                            .get(&shape_id)
                            .is_some_and(|existing| *existing != axis)
                    {
                        read_only_reason = Some("unsupported graph axis constraint".into());
                    } else {
                        document.axis_constraints.insert(shape_id, axis);
                    }
                } else if let Some(field) = graph_constraint_field(kind) {
                    let value = constraint.get("value").and_then(Value::as_f64);
                    if value.is_none_or(|value| !value.is_finite() || value <= 0.0) {
                        read_only_reason = Some("invalid graph dimension constraint".into());
                        continue;
                    }
                    document.selected = Some(shape_id);
                    if document
                        .set_dimension(field, value.unwrap() as f32)
                        .is_err()
                    {
                        read_only_reason = Some("unsupported graph dimension target".into());
                    }
                } else {
                    read_only_reason = Some("unsupported graph sketch constraint".into());
                }
            }
            if document.equal_lines.iter().any(|(&target, &source)| {
                document.point_links.keys().any(|(id, _)| *id == target || *id == source)
                    || document.fixed_anchors.contains_key(&(target, 1))
                    || document.driving_dimensions.contains_key(&(target, DimensionField::LineLength))
                    || !matches!(
                        (document.shapes.iter().find(|shape| shape.id() == source), document.shapes.iter().find(|shape| shape.id() == target)),
                        (Some(Shape::Line { a, b, .. }), Some(Shape::Line { a: c, b: d, .. })) if (a.distance(*b) - c.distance(*d)).abs() < 1e-3
                    )
            }) {
                read_only_reason = Some("unsupported equal line dependency".into());
            }
            if document.equal_radii.iter().any(|(&target, &source)| {
                document.fixed_anchors.keys().any(|(id, anchor)| *id == target && *anchor > 0)
                    || document.driving_dimensions.contains_key(&(target, DimensionField::CircleDiameter))
                    || document.driving_dimensions.contains_key(&(target, DimensionField::CurveRadius))
                    || !matches!((document.shapes.iter().find(|shape| shape.id() == source), document.shapes.iter().find(|shape| shape.id() == target)),
                        (Some(Shape::Circle { radius: a, .. } | Shape::Arc { radius: a, .. }), Some(Shape::Circle { radius: b, .. } | Shape::Arc { radius: b, .. })) if (*a - *b).abs() < 1e-3)
            }) { read_only_reason = Some("unsupported equal radius dependency".into()); }
            if document.radius_differences.iter().any(|(&target, &(source, value))| {
                document.driving_dimensions.contains_key(&(target, DimensionField::CircleDiameter))
                    || document.driving_dimensions.contains_key(&(target, DimensionField::CurveRadius))
                    || !matches!((document.shapes.iter().find(|shape| shape.id() == source), document.shapes.iter().find(|shape| shape.id() == target)),
                        (Some(Shape::Circle { radius: a, .. }), Some(Shape::Circle { radius: b, .. })) if (*a - *b - value).abs() < 1e-3)
            }) { read_only_reason = Some("unsupported radial distance dependency".into()); }
            if document.external_tangents.iter().any(|(&target, &source)| {
                document
                    .shapes
                    .iter()
                    .find(|shape| shape.id() == source)
                    .zip(document.shapes.iter().find(|shape| shape.id() == target))
                    .is_none_or(|(a, b)| !external_tangent_matches(a, b))
            }) {
                read_only_reason = Some("unsupported external tangent dependency".into());
            }
            if document.internal_tangents.iter().any(|(&inner, &outer)| {
                document
                    .shapes
                    .iter()
                    .find(|shape| shape.id() == outer)
                    .zip(document.shapes.iter().find(|shape| shape.id() == inner))
                    .is_none_or(|(a, b)| !internal_tangent_matches(a, b))
            }) {
                read_only_reason = Some("unsupported internal tangent dependency".into());
            }
            if document.midpoint_links.iter().any(|(&point_id, &curve_id)| {
                document.fixed_anchors.contains_key(&(point_id, 0))
                    || document.point_curve_links.contains_key(&point_id)
                    || document.point_links.values().any(|id| *id == point_id)
                    || document.shapes.iter().find(|shape| shape.id() == point_id)
                        .zip(document.shapes.iter().find(|shape| shape.id() == curve_id))
                        .is_none_or(|(point, curve)| !matches!(point, Shape::Point { at, .. } if Document::curve_midpoint(curve).is_some_and(|middle| at.distance(middle) < 1e-3)))
            }) {
                read_only_reason = Some("unsupported midpoint dependency".into());
            }
            if document.point_curve_links.iter().any(|(&point_id, &curve_id)| {
                document.fixed_anchors.contains_key(&(point_id, 0))
                    || document.midpoint_links.contains_key(&point_id)
                    || document.point_links.values().any(|id| *id == point_id)
                    || document.shapes.iter().find(|shape| shape.id() == point_id)
                        .zip(document.shapes.iter().find(|shape| shape.id() == curve_id))
                        .is_none_or(|(point, curve)| !matches!(point, Shape::Point { at, .. } if Document::closest_curve_point(curve, *at).is_some_and(|projected| at.distance(projected) < 1e-3)))
            }) {
                read_only_reason = Some("unsupported point on curve dependency".into());
            }
            if document.normal_links.iter().any(|(&target, &(source, endpoint))| {
                document.fixed_anchors.keys().any(|(id, _)| *id == target)
                    || document.point_links.keys().any(|(id, _)| *id == target)
                    || document.line_relations.contains_key(&target)
                    || document.equal_lines.contains_key(&target)
                    || document.external_tangents.contains_key(&target)
                    || document.normal_links.contains_key(&source)
                    || document.shapes.iter().find(|shape| shape.id() == source)
                        .zip(document.shapes.iter().find(|shape| shape.id() == target))
                        .and_then(|(source_shape, target_shape)| Document::normal_line_target(source_shape, target_shape, endpoint).map(|adjusted| (target_shape, adjusted)))
                        .is_none_or(|(original, adjusted)| !matches!((original, adjusted), (Shape::Line { a, b, .. }, Shape::Line { a: next_a, b: next_b, .. }) if a.distance(next_a) < 1e-3 && b.distance(next_b) < 1e-3))
            }) {
                read_only_reason = Some("unsupported normal dependency".into());
            }
            if document.equal_ellipses.iter().any(|(&target, &source)| {
                document.fixed_anchors.keys().any(|(id, anchor)| *id == target && *anchor > 0)
                    || document.driving_dimensions.contains_key(&(target, DimensionField::RectangleWidth))
                    || document.driving_dimensions.contains_key(&(target, DimensionField::RectangleHeight))
                    || !matches!((document.shapes.iter().find(|shape| shape.id() == source), document.shapes.iter().find(|shape| shape.id() == target)),
                        (Some(Shape::Ellipse { rx: ax, ry: ay, .. } | Shape::EllipticArc { rx: ax, ry: ay, .. }), Some(Shape::Ellipse { rx: bx, ry: by, .. } | Shape::EllipticArc { rx: bx, ry: by, .. })) if (*ax - *bx).abs() < 1e-3 && (*ay - *by).abs() < 1e-3)
            }) { read_only_reason = Some("unsupported equal ellipse dependency".into()); }
            if document.concentric.iter().any(|(&target, &source)| {
                !matches!((document.shapes.iter().find(|shape| shape.id() == source), document.shapes.iter().find(|shape| shape.id() == target)),
                        (Some(a), Some(b)) if matches!(a, Shape::Circle { .. } | Shape::Arc { .. } | Shape::Ellipse { .. } | Shape::EllipticArc { .. }) && matches!(b, Shape::Circle { .. } | Shape::Arc { .. } | Shape::Ellipse { .. } | Shape::EllipticArc { .. }) && a.center().distance(b.center()) < 1e-3)
            }) { read_only_reason = Some("unsupported concentric dependency".into()); }
            if document.shared_ellipses.iter().any(|(&target, &source)| {
                !matches!((document.shapes.iter().find(|shape| shape.id() == source), document.shapes.iter().find(|shape| shape.id() == target)),
                    (Some(Shape::Ellipse { center: ac, rx: ax, ry: ay, rotation: ar, .. } | Shape::EllipticArc { center: ac, rx: ax, ry: ay, rotation: ar, .. }),
                     Some(Shape::Ellipse { center: bc, rx: bx, ry: by, rotation: br, .. } | Shape::EllipticArc { center: bc, rx: bx, ry: by, rotation: br, .. }))
                    if ac.distance(*bc) < 1e-3 && (*ax - *bx).abs() < 1e-3 && (*ay - *by).abs() < 1e-3
                        && ((*ar - *br).to_radians().sin()).atan2((*ar - *br).to_radians().cos()).abs() < 1e-3)
            }) { read_only_reason = Some("unsupported shared ellipse dependency".into()); }
            if document
                .line_relations
                .iter()
                .any(|(&target, &(source, kind))| {
                    document
                        .point_links
                        .keys()
                        .any(|(id, _)| *id == target || *id == source)
                        || document.fixed_anchors.contains_key(&(target, 1))
                        || (kind == LineRelationKind::Collinear
                            && document.fixed_anchors.keys().any(|(id, _)| *id == target))
                        || document.axis_constraints.contains_key(&target)
                        || !document
                            .shapes
                            .iter()
                            .find(|shape| shape.id() == source)
                            .zip(document.shapes.iter().find(|shape| shape.id() == target))
                            .is_some_and(|(a, b)| line_relation_matches(a, b, kind))
                })
            {
                read_only_reason = Some("unsupported line direction dependency".into());
            }
            document.clear_history();
            workspaces.push(SketchWorkspace {
                id: sketch_id.to_string(),
                name: sketch_name.to_string(),
                plane,
                document,
                entity_ids,
                original_entities,
            });
        }
        let document = workspaces[0].document.clone();

        let mut project = Self {
            path,
            value,
            part_index,
            sketches: workspaces,
            active_sketch: 0,
            read_only_reason,
            disk_snapshot,
            parked_parts: HashMap::new(),
            metadata_dirty: false,
            clean_value: Value::Null,
        };
        if project.read_only_reason.as_deref() == Some("solid features need native support")
            && project.evaluated_solids().is_some()
            && project.all_extrusions_registered()
            && ["features", "solidSteps", "solids"].iter().all(|key| {
                project.value["items"][project.part_index]["data"]
                    .get(*key)
                    .and_then(Value::as_array)
                    .is_none_or(Vec::is_empty)
            })
        {
            project.read_only_reason = None;
        }
        project.clean_value = project
            .staged(&document)
            .map_or_else(|_| project.value.clone(), |(value, _, _)| value);
        Ok((project, document))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn part_name(&self) -> &str {
        self.value["items"][self.part_index]["name"]
            .as_str()
            .unwrap_or("Part")
    }

    pub fn part_indices(&self) -> Vec<usize> {
        self.value["items"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(index, item)| (item["type"] == "part").then_some(index))
            .collect()
    }

    pub fn part_name_at(&self, index: usize) -> Option<&str> {
        self.value["items"].get(index).and_then(|item| {
            (item.get("type")?.as_str()? == "part")
                .then(|| item.get("name").and_then(Value::as_str).unwrap_or("Part"))
        })
    }

    pub fn rename_part(&mut self, name: &str) -> Result<(), String> {
        let name = valid_name(name)?;
        self.value["items"][self.part_index]["name"] = json!(name);
        self.metadata_dirty = true;
        Ok(())
    }

    pub fn rename_sketch(&mut self, name: &str) -> Result<(), String> {
        let name = valid_name(name)?;
        let sketch_id = self.sketches[self.active_sketch].id.clone();
        let nodes = self
            .value
            .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array_mut)
            .ok_or("project CAD graph changed unexpectedly")?;
        let sketch = nodes
            .iter_mut()
            .find(|node| node.get("id").and_then(Value::as_str) == Some(&sketch_id))
            .ok_or("sketch node is missing")?;
        sketch["name"] = json!(name);
        self.sketches[self.active_sketch].name = name.to_string();
        self.metadata_dirty = true;
        Ok(())
    }

    pub fn active_part_index(&self) -> usize {
        self.part_index
    }

    pub fn create_part(&mut self, current: &Document) -> Result<Document, String> {
        let items = self.value["items"]
            .as_array()
            .ok_or("project has no items array")?;
        let mut number = 1;
        while items.iter().any(|item| {
            item.get("id").and_then(Value::as_str) == Some(format!("native-part-{number}").as_str())
        }) {
            number += 1;
        }
        let part_id = format!("native-part-{number}");
        let sketch_id = "native-sketch-1";
        let item = json!({
            "id": part_id,
            "type": "part",
            "name": format!("Part {number}"),
            "data": {
                "features": [],
                "cad": {
                    "nodes": [
                        {"id":"plane-top","type":"referencePlane","name":"Top","plane":"XY"},
                        {"id":"plane-front","type":"referencePlane","name":"Front","plane":"XZ"},
                        {"id":"plane-right","type":"referencePlane","name":"Right","plane":"YZ"},
                        {"id":sketch_id,"type":"sketch","name":"Sketch 1","targetId":"plane-top","relations":[],"variables":[],"dimensions":[]}
                    ],
                    "rootNodeIds": ["plane-top","plane-front","plane-right",sketch_id]
                },
                "tree": {"orderedNodeIds": [sketch_id], "dirtySketchIds": []}
            }
        });
        let target = items.len();
        self.value["items"]
            .as_array_mut()
            .expect("items checked above")
            .push(item);
        match self.switch_part(target, current) {
            Ok(document) => Ok(document),
            Err(error) => {
                self.value["items"]
                    .as_array_mut()
                    .expect("items checked above")
                    .pop();
                Err(error)
            }
        }
    }

    pub fn switch_part(&mut self, target: usize, current: &Document) -> Result<Document, String> {
        if target == self.part_index {
            return Ok(current.clone());
        }
        if !self.part_indices().contains(&target) {
            return Err("part does not exist".into());
        }
        let loaded = if self.parked_parts.contains_key(&target) {
            None
        } else {
            let mut value = self.value.clone();
            value["selectedPath"] = json!([target]);
            Some(Self::from_value(
                self.path.clone(),
                value,
                self.disk_snapshot.clone(),
            )?)
        };
        self.sketches[self.active_sketch].document = current.clone();
        let old = PartWorkspace {
            sketches: std::mem::take(&mut self.sketches),
            active_sketch: self.active_sketch,
            read_only_reason: self.read_only_reason.take(),
        };
        self.parked_parts.insert(self.part_index, old);
        if let Some((next, _)) = loaded {
            self.sketches = next.sketches;
            self.active_sketch = next.active_sketch;
            self.read_only_reason = next.read_only_reason;
        } else {
            let next = self
                .parked_parts
                .remove(&target)
                .expect("parked part checked above");
            self.sketches = next.sketches;
            self.active_sketch = next.active_sketch;
            self.read_only_reason = next.read_only_reason;
        }
        self.part_index = target;
        self.value["selectedPath"] = json!([target]);
        Ok(self.sketches[self.active_sketch].document.clone())
    }

    pub fn read_only_reason(&self) -> Option<&str> {
        self.read_only_reason.as_deref()
    }

    pub fn sketch_count(&self) -> usize {
        self.sketches.len()
    }

    pub fn sketch_name(&self, index: usize) -> Option<&str> {
        self.sketches.get(index).map(|sketch| sketch.name.as_str())
    }

    pub fn sketch_plane(&self, index: usize) -> Option<SketchPlane> {
        self.sketches.get(index).and_then(|sketch| sketch.plane)
    }

    pub fn active_sketch(&self) -> usize {
        self.active_sketch
    }

    pub fn switch_sketch(&mut self, index: usize, current: &Document) -> Result<Document, String> {
        if index >= self.sketches.len() {
            return Err("sketch does not exist".into());
        }
        self.sketches[self.active_sketch].document = current.clone();
        self.active_sketch = index;
        Ok(self.sketches[index].document.clone())
    }

    pub fn create_sketch_on_plane(
        &mut self,
        current: &Document,
        plane: SketchPlane,
    ) -> Result<Document, String> {
        if let Some(reason) = &self.read_only_reason {
            return Err(format!("project is read-only: {reason}"));
        }
        self.sketches[self.active_sketch].document = current.clone();
        let nodes = self
            .value
            .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array_mut)
            .ok_or("project CAD graph changed unexpectedly")?;
        let target_id = nodes
            .iter()
            .find(|node| SketchPlane::from_graph_node(node) == Some(plane))
            .and_then(|node| node.get("id"))
            .and_then(Value::as_str)
            .ok_or("project has no matching reference plane")?
            .to_string();
        let mut number = self.sketches.len() + 1;
        let id = loop {
            let candidate = format!("native-sketch-{number}");
            if !nodes
                .iter()
                .any(|node| node.get("id").and_then(Value::as_str) == Some(&candidate))
            {
                break candidate;
            }
            number += 1;
        };
        let name = format!("Sketch {number}");
        nodes.push(json!({"id":id,"type":"sketch","name":name,"targetId":target_id,"relations":[],"variables":[],"dimensions":[]}));
        self.value["items"][self.part_index]["data"]["cad"]["rootNodeIds"]
            .as_array_mut()
            .ok_or("project root node list is invalid")?
            .push(json!(id));
        self.value["items"][self.part_index]["data"]["tree"]["orderedNodeIds"]
            .as_array_mut()
            .ok_or("project tree is invalid")?
            .push(json!(id));
        let document = Document::default();
        self.sketches.push(SketchWorkspace {
            id,
            name,
            plane: Some(plane),
            document: document.clone(),
            entity_ids: HashMap::new(),
            original_entities: HashMap::new(),
        });
        self.active_sketch = self.sketches.len() - 1;
        Ok(document)
    }

    pub fn save(&mut self, document: &Document) -> Result<(), String> {
        if let Some(reason) = &self.read_only_reason
            && !self.metadata_dirty
            && self.extrusion_preview().is_none()
            && !self
                .parked_parts
                .values()
                .any(|part| part.read_only_reason.is_none())
        {
            return Err(format!("project is read-only: {reason}"));
        }
        let current = match fs::read(&self.path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("read project before save: {error}")),
        };
        if current != self.disk_snapshot {
            return Err("project changed on disk; reopen before saving".into());
        }
        let (mut next, staged_sketches, staged_parked) = self.staged(document)?;
        let revision = next.get("revision").and_then(Value::as_u64).unwrap_or(0);
        next["revision"] = json!(revision.saturating_add(1));
        let bytes = serde_json::to_vec_pretty(&next)
            .map_err(|error| format!("serialize project: {error}"))?;
        atomic_write(&self.path, &bytes)?;
        self.value = next;
        self.sketches = staged_sketches;
        self.parked_parts = staged_parked;
        self.disk_snapshot = Some(bytes);
        self.metadata_dirty = false;
        self.clean_value = self.value.clone();
        Ok(())
    }

    /// True when saving would write something different from the last open or save.
    pub fn has_unsaved_changes(&self, document: &Document) -> bool {
        self.metadata_dirty
            || self
                .staged(document)
                .map_or(true, |(value, _, _)| value != self.clean_value)
    }

    /// Writes the project as it would be saved to `output`, without touching the
    /// project path or bumping the revision.
    pub fn export_to(&self, document: &Document, output: &Path) -> Result<(), String> {
        let (next, _, _) = self.staged(document)?;
        let bytes = serde_json::to_vec_pretty(&next)
            .map_err(|error| format!("serialize project: {error}"))?;
        atomic_write(output, &bytes)
    }

    fn staged(&self, document: &Document) -> Result<StagedProject, String> {
        let mut staged_sketches = self.sketches.clone();
        staged_sketches[self.active_sketch].document = document.clone();
        let mut next = self.value.clone();
        if self.read_only_reason.is_none() {
            serialize_sketches(&mut next, self.part_index, &mut staged_sketches)?;
        }
        let mut staged_parked = self.parked_parts.clone();
        for (index, part) in &mut staged_parked {
            if part.read_only_reason.is_none() {
                serialize_sketches(&mut next, *index, &mut part.sketches)?;
            }
        }
        Ok((next, staged_sketches, staged_parked))
    }
}

fn serialize_sketches(
    next: &mut Value,
    part_index: usize,
    sketches: &mut [SketchWorkspace],
) -> Result<(), String> {
    let nodes = next
        .pointer_mut(&format!("/items/{}/data/cad/nodes", part_index))
        .and_then(Value::as_array_mut)
        .ok_or("project CAD graph changed unexpectedly")?;
    let mut removed_constraint_ids = HashSet::new();
    for workspace in sketches {
        let document = &workspace.document;
        for (&(id, index), &fixed) in &document.fixed_anchors {
            let current = document
                .fixed_anchor_position(id, index)
                .ok_or("fixed point references deleted geometry")?;
            if current.distance(fixed) > 1e-4 {
                return Err("fixed point moved from constrained position".into());
            }
        }
        let old_ids: HashSet<&str> = workspace.entity_ids.values().map(String::as_str).collect();
        let mut used_ids: HashSet<String> = nodes
            .iter()
            .filter_map(|node| node.get("id").and_then(Value::as_str))
            .filter(|id| !old_ids.contains(id))
            .map(str::to_string)
            .collect();
        let mut next_ids = HashMap::new();
        let mut next_originals = HashMap::new();
        let mut replacements = HashMap::new();
        let mut additions = Vec::new();
        for shape in &document.shapes {
            let preferred = workspace
                .entity_ids
                .get(&shape.id())
                .cloned()
                .unwrap_or_else(|| format!("native-entity-{}", shape.id()));
            let mut graph_id = preferred.clone();
            let mut suffix = 2;
            while !used_ids.insert(graph_id.clone()) {
                graph_id = format!("{preferred}-{suffix}");
                suffix += 1;
            }
            let mut node = match workspace.original_entities.get(&shape.id()) {
                Some(original)
                    if graph_id == preferred
                        && read_shape(original, shape.id()).as_ref() == Some(shape)
                        && is_construction_node(original)
                            == document.construction.contains(&shape.id()) =>
                {
                    original.clone()
                }
                Some(original) => update_shape(original.clone(), shape, &graph_id)?,
                None => write_shape(shape, &graph_id, &workspace.id)?,
            };
            apply_construction(&mut node, document.construction.contains(&shape.id()))?;
            if let Some(old_id) = workspace.entity_ids.get(&shape.id()) {
                replacements.insert(old_id.clone(), node.clone());
            } else {
                additions.push(node.clone());
            }
            next_ids.insert(shape.id(), graph_id);
            next_originals.insert(shape.id(), node);
        }
        let originals = std::mem::take(nodes);
        for original in originals {
            if let Some(id) = original.get("id").and_then(Value::as_str)
                && old_ids.contains(id)
            {
                if let Some(replacement) = replacements.remove(id) {
                    nodes.push(replacement);
                }
                continue;
            }
            nodes.push(original);
        }
        nodes.extend(additions);
        let old_graph_shape_ids: HashMap<&str, u32> = workspace
            .entity_ids
            .iter()
            .map(|(shape_id, graph_id)| (graph_id.as_str(), *shape_id))
            .collect();
        let mut graph_backed = HashSet::new();
        nodes.retain_mut(|node| {
            if node.get("type").and_then(Value::as_str) != Some("sketchConstraint")
                || node.get("sketchId").and_then(Value::as_str) != Some(&workspace.id)
            {
                return true;
            }
            let kind = node
                .pointer("/constraint/type")
                .and_then(Value::as_str)
                .map(str::to_string);
            let graph_id = node.pointer("/constraint/entityId").and_then(Value::as_str);
            let shape_id = graph_id.and_then(|id| old_graph_shape_ids.get(id)).copied();
            let keep = match (kind.as_deref(), shape_id) {
                (Some("horizontal" | "vertical"), Some(shape_id)) => {
                    let axis = document.axis_constraints.get(&shape_id);
                    if axis.is_some_and(|axis| Some(axis.name()) == kind.as_deref()) {
                        graph_backed.insert((shape_id, kind.clone().unwrap()));
                        if let Some(new_id) = next_ids.get(&shape_id) {
                            node["constraint"]["entityId"] = json!(new_id);
                        }
                        true
                    } else {
                        false
                    }
                }
                (Some(kind), Some(shape_id)) if graph_constraint_field(kind).is_some() => {
                    let field = graph_constraint_field(kind).unwrap();
                    // Annotated dimensions move to a sketch relation, which
                    // can carry the annotation.
                    if let Some(value) = document.driving_dimensions.get(&(shape_id, field))
                        && !document
                            .dims
                            .has_annotation(DimensionKey::Field(shape_id, field))
                    {
                        graph_backed.insert((shape_id, kind.to_string()));
                        if let Some(new_id) = next_ids.get(&shape_id) {
                            node["constraint"]["entityId"] = json!(new_id);
                        }
                        node["constraint"]["value"] = json!(value);
                        true
                    } else {
                        false
                    }
                }
                _ => false,
            };
            if !keep && let Some(id) = node.get("id").and_then(Value::as_str) {
                removed_constraint_ids.insert(id.to_string());
            }
            keep
        });
        let sketch = nodes
            .iter_mut()
            .find(|node| node.get("id").and_then(Value::as_str) == Some(&workspace.id))
            .ok_or("project sketch node changed unexpectedly")?;
        if !sketch.get("relations").is_some_and(Value::is_array) {
            sketch["relations"] = json!([]);
        }
        sketch["variables"] = Value::Array(
            document
                .dims
                .variables
                .iter()
                .map(|variable| {
                    json!({
                        "name": variable.name,
                        "kind": variable.kind.name(),
                        "expression": variable.expression,
                    })
                })
                .collect(),
        );
        let relations = sketch["relations"]
            .as_array_mut()
            .ok_or("project sketch relations changed unexpectedly")?;
        let graph_shape_ids: HashMap<&str, u32> = next_ids
            .iter()
            .map(|(shape_id, graph_id)| (graph_id.as_str(), *shape_id))
            .collect();
        relations.retain_mut(|relation| {
            let Some(kind) = relation.get("type").and_then(Value::as_str) else {
                return true;
            };
            if kind == "distance" {
                // Point dimensions are rewritten from the document below.
                return false;
            }
            if kind == "fixed" {
                let graph_id = relation
                    .pointer("/anchor/entityId")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let shape_id = graph_id
                    .as_deref()
                    .and_then(|id| {
                        old_graph_shape_ids
                            .get(id)
                            .or_else(|| graph_shape_ids.get(id))
                    })
                    .copied();
                let point = relation.pointer("/anchor/point").and_then(Value::as_str);
                let index = shape_id.zip(point).and_then(|(id, point)| {
                    match document.shapes.iter().find(|shape| shape.id() == id)? {
                        Shape::Point { .. } if point == "center" => Some(0),
                        Shape::Arc { .. } | Shape::EllipticArc { .. } => match point {
                            "center" => Some(0),
                            "p0" => Some(1),
                            "p1" => Some(2),
                            _ => None,
                        },
                        Shape::Circle { .. } if point == "center" => Some(0),
                        Shape::Ellipse { .. } | Shape::RotatedRectangle { .. } => {
                            if point == "center" {
                                Some(0)
                            } else {
                                point
                                    .strip_prefix('p')?
                                    .parse::<usize>()
                                    .ok()
                                    .filter(|index| *index < 4)
                                    .map(|index| index + 1)
                            }
                        }
                        Shape::Polygon { sides, .. } => {
                            if point == "center" {
                                Some(0)
                            } else {
                                point
                                    .strip_prefix("vertex")?
                                    .parse::<usize>()
                                    .ok()
                                    .filter(|index| *index < *sides as usize)
                                    .map(|index| index + 1)
                            }
                        }
                        Shape::Capsule { .. } => match point {
                            "from" => Some(0),
                            "to" => Some(1),
                            _ => None,
                        },
                        Shape::Line { .. } | Shape::Rectangle { .. } => match point {
                            "p0" => Some(0),
                            "p1" => Some(1),
                            _ => None,
                        },
                        Shape::Spline { points, .. } => point
                            .strip_prefix("point")?
                            .parse::<usize>()
                            .ok()
                            .filter(|index| *index < points.len()),
                        _ => None,
                    }
                });
                if let Some((shape_id, index)) = shape_id.zip(index)
                    && document.fixed_anchors.contains_key(&(shape_id, index))
                    && let Some(new_id) = next_ids.get(&shape_id)
                {
                    relation["anchor"]["entityId"] = json!(new_id);
                    if relation.get("fixation").is_some() {
                        relation["fixation"] = json!(new_id);
                    }
                    return true;
                }
                return false;
            }
            if kind == "normal" {
                let target = relation
                    .get("a")
                    .and_then(|anchor| anchor.get("entityId"))
                    .and_then(Value::as_str)
                    .and_then(|id| {
                        old_graph_shape_ids
                            .get(id)
                            .or_else(|| graph_shape_ids.get(id))
                    })
                    .copied();
                let source = relation
                    .get("b")
                    .and_then(Value::as_str)
                    .and_then(|id| {
                        old_graph_shape_ids
                            .get(id)
                            .or_else(|| graph_shape_ids.get(id))
                    })
                    .copied();
                let endpoint = match relation
                    .get("a")
                    .and_then(|anchor| anchor.get("point"))
                    .and_then(Value::as_str)
                {
                    Some("p0") => Some(0),
                    Some("p1") => Some(1),
                    _ => None,
                };
                if let Some(((target, source), endpoint)) = target.zip(source).zip(endpoint)
                    && document.normal_links.get(&target) == Some(&(source, endpoint))
                    && let Some((target_id, source_id)) =
                        next_ids.get(&target).zip(next_ids.get(&source))
                {
                    relation["a"]["entityId"] = json!(target_id);
                    relation["b"] = json!(source_id);
                    return true;
                }
                return false;
            }
            if kind == "pointOnCurve" {
                let point = relation
                    .get("a")
                    .and_then(|anchor| anchor.get("entityId"))
                    .and_then(Value::as_str)
                    .and_then(|id| {
                        old_graph_shape_ids
                            .get(id)
                            .or_else(|| graph_shape_ids.get(id))
                    })
                    .copied();
                let curve = relation
                    .get("b")
                    .and_then(Value::as_str)
                    .and_then(|id| {
                        old_graph_shape_ids
                            .get(id)
                            .or_else(|| graph_shape_ids.get(id))
                    })
                    .copied();
                if let Some((point, curve)) = point.zip(curve)
                    && document.point_curve_links.get(&point) == Some(&curve)
                    && let Some((point_id, curve_id)) =
                        next_ids.get(&point).zip(next_ids.get(&curve))
                {
                    relation["a"]["entityId"] = json!(point_id);
                    relation["b"] = json!(curve_id);
                    return true;
                }
                return false;
            }
            if kind == "midpoint" {
                let point = relation
                    .get("a")
                    .and_then(|anchor| anchor.get("entityId"))
                    .and_then(Value::as_str)
                    .and_then(|id| {
                        old_graph_shape_ids
                            .get(id)
                            .or_else(|| graph_shape_ids.get(id))
                    })
                    .copied();
                let curve = relation
                    .get("b")
                    .and_then(Value::as_str)
                    .and_then(|id| {
                        old_graph_shape_ids
                            .get(id)
                            .or_else(|| graph_shape_ids.get(id))
                    })
                    .copied();
                if let Some((point, curve)) = point.zip(curve)
                    && document.midpoint_links.get(&point) == Some(&curve)
                    && let Some((point_id, curve_id)) =
                        next_ids.get(&point).zip(next_ids.get(&curve))
                {
                    relation["a"]["entityId"] = json!(point_id);
                    relation["b"] = json!(curve_id);
                    return true;
                }
                return false;
            }
            if kind == "tangent" {
                let resolve = |side: &str| -> Option<u32> {
                    let id = relation.get(side)?.as_str()?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                if let Some((source, target)) = resolve("a").zip(resolve("b"))
                    && document.external_tangents.get(&target) == Some(&source)
                    && let Some((source_id, target_id)) =
                        next_ids.get(&source).zip(next_ids.get(&target))
                {
                    relation["a"] = json!(source_id);
                    relation["b"] = json!(target_id);
                    return true;
                }
                return false;
            }
            if kind == "internalTangent" {
                let resolve = |side: &str| -> Option<u32> {
                    let id = relation.get(side)?.as_str()?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                if let Some((outer, inner)) = resolve("a").zip(resolve("b"))
                    && document.internal_tangents.get(&inner) == Some(&outer)
                    && let Some((outer_id, inner_id)) =
                        next_ids.get(&outer).zip(next_ids.get(&inner))
                {
                    relation["a"] = json!(outer_id);
                    relation["b"] = json!(inner_id);
                    return true;
                }
                return false;
            }
            if kind == "radiusDifference" {
                let resolve = |side: &str| -> Option<u32> {
                    let id = relation.get(side)?.as_str()?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                if let Some((source, target)) = resolve("a").zip(resolve("b"))
                    && let Some(&(old_source, value)) = document.radius_differences.get(&target)
                    && old_source == source
                    && let Some((source_id, target_id)) =
                        next_ids.get(&source).zip(next_ids.get(&target))
                {
                    relation["a"] = json!(source_id);
                    relation["b"] = json!(target_id);
                    relation["value"] = json!(value);
                    return true;
                }
                return false;
            }
            if kind == "sameEllipse" {
                let resolve = |side: &str| -> Option<u32> {
                    let id = relation.get(side)?.as_str()?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                if let Some((source, target)) = resolve("a").zip(resolve("b")) {
                    let root = document
                        .shared_ellipses
                        .get(&source)
                        .copied()
                        .unwrap_or(source);
                    if document.shared_ellipses.get(&target) == Some(&root)
                        && let Some((source_id, target_id)) =
                            next_ids.get(&source).zip(next_ids.get(&target))
                    {
                        relation["a"] = json!(source_id);
                        relation["b"] = json!(target_id);
                        return true;
                    }
                }
                return false;
            }
            if kind == "parallel"
                || kind == "perpendicular"
                || kind == "collinear"
                || kind == "angle"
            {
                let resolve = |side: &str| -> Option<u32> {
                    let id = relation.get(side)?.as_str()?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                if let Some((source, target)) = resolve("a").zip(resolve("b"))
                    && let Some(&(old_source, relation_kind)) = document.line_relations.get(&target)
                {
                    if old_source != source || relation_kind.name() != kind {
                        return false;
                    }
                    if let Some((source_id, target_id)) =
                        next_ids.get(&source).zip(next_ids.get(&target))
                    {
                        relation["a"] = json!(source_id);
                        relation["b"] = json!(target_id);
                        if let Some(value) = relation_kind.angle_value() {
                            relation["value"] = json!(value);
                        }
                        return true;
                    }
                }
                return false;
            }
            if kind == "offset" || kind == "offsetChain" {
                let resolve = |id: Option<&str>| -> Option<u32> {
                    let id = id?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                if kind == "offset" {
                    let source = resolve(relation.get("a").and_then(Value::as_str));
                    let target = resolve(relation.get("b").and_then(Value::as_str));
                    if let (Some(source), Some(target)) = (source, target)
                        && let Some(&(old_source, distance)) = document.offsets.get(&target)
                        && old_source == source
                        && let Some((source_id, target_id)) =
                            next_ids.get(&source).zip(next_ids.get(&target))
                    {
                        relation["a"] = json!(source_id);
                        relation["b"] = json!(target_id);
                        relation["value"] = json!(distance);
                        return true;
                    }
                    return false;
                }
                let targets: Option<Vec<u32>> = relation
                    .get("targets")
                    .and_then(Value::as_array)
                    .and_then(|targets| {
                        targets
                            .iter()
                            .map(|id| resolve(id.as_str()))
                            .collect::<Option<Vec<u32>>>()
                    });
                let Some(chain) = targets.and_then(|targets| {
                    document
                        .offset_chains
                        .iter()
                        .find(|chain| chain.targets == targets)
                }) else {
                    return false;
                };
                let Some(value) = offset_chain_json(chain, &relation["id"], &next_ids) else {
                    return false;
                };
                *relation = value;
                return true;
            }
            if kind == "symmetric" || kind == "mirror" {
                let resolve = |id: Option<&str>| -> Option<u32> {
                    let id = id?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                let Some(axis) = resolve(relation.get("symmetryLine").and_then(Value::as_str))
                else {
                    return false;
                };
                let Some(axis_id) = next_ids.get(&axis) else {
                    return false;
                };
                if kind == "mirror" {
                    let source = resolve(relation.get("a").and_then(Value::as_str));
                    let target = resolve(relation.get("b").and_then(Value::as_str));
                    if let (Some(source), Some(target)) = (source, target)
                        && document.mirrors.get(&target) == Some(&(source, axis))
                        && let Some((source_id, target_id)) =
                            next_ids.get(&source).zip(next_ids.get(&target))
                    {
                        relation["a"] = json!(source_id);
                        relation["b"] = json!(target_id);
                        relation["symmetryLine"] = json!(axis_id);
                        return true;
                    }
                    return false;
                }
                let read = |side: &str| -> Option<(u32, usize)> {
                    let id = resolve(relation.pointer(&format!("/{side}/entityId"))?.as_str())?;
                    let name = relation.pointer(&format!("/{side}/point"))?.as_str()?;
                    let shape = document.shapes.iter().find(|shape| shape.id() == id)?;
                    Some((id, symmetry_anchor_index(shape, name)?))
                };
                if let (Some(a), Some(b)) = (read("a"), read("b"))
                    && document
                        .symmetric_points
                        .contains(&crate::model::SymmetricPoints { a, b, axis })
                    && let Some((a_id, b_id)) = next_ids.get(&a.0).zip(next_ids.get(&b.0))
                {
                    relation["a"]["entityId"] = json!(a_id);
                    relation["b"]["entityId"] = json!(b_id);
                    relation["symmetryLine"] = json!(axis_id);
                    return true;
                }
                return false;
            }
            if kind == "concentric" {
                let resolve = |side: &str| -> Option<u32> {
                    let id = relation.get(side)?.as_str()?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                if let Some((source, target)) = resolve("a").zip(resolve("b"))
                    && document.concentric.get(&target) == Some(&source)
                    && let Some((source_id, target_id)) =
                        next_ids.get(&source).zip(next_ids.get(&target))
                {
                    relation["a"] = json!(source_id);
                    relation["b"] = json!(target_id);
                    return true;
                }
                return false;
            }
            if kind == "equal" {
                let resolve = |side: &str| -> Option<u32> {
                    let id = relation.get(side)?.as_str()?;
                    old_graph_shape_ids
                        .get(id)
                        .or_else(|| graph_shape_ids.get(id))
                        .copied()
                };
                if let Some((source, target)) = resolve("a").zip(resolve("b"))
                    && (document.equal_lines.get(&target) == Some(&source)
                        || document.equal_radii.get(&target) == Some(&source)
                        || document.equal_ellipses.get(&target) == Some(&source))
                    && let Some((source_id, target_id)) =
                        next_ids.get(&source).zip(next_ids.get(&target))
                {
                    relation["a"] = json!(source_id);
                    relation["b"] = json!(target_id);
                    return true;
                }
                return false;
            }
            let resolve_id = |id: Option<&str>| -> Option<u32> {
                let id = id?;
                old_graph_shape_ids
                    .get(id)
                    .or_else(|| graph_shape_ids.get(id))
                    .copied()
            };
            let end_index = |point: Option<&str>| match point {
                Some("p0") => Some(0u8),
                Some("p1") => Some(1u8),
                _ => None,
            };
            let anchor_of = |relation: &Value, key: &str| {
                resolve_id(
                    relation
                        .pointer(&format!("/{key}/entityId"))
                        .and_then(Value::as_str),
                )
                .zip(end_index(
                    relation
                        .pointer(&format!("/{key}/point"))
                        .and_then(Value::as_str),
                ))
            };
            if kind == "smoothJoin" {
                let side = |key: &str| {
                    let id = resolve_id(
                        relation
                            .pointer(&format!("/{key}/entityId"))
                            .and_then(Value::as_str),
                    )?;
                    let shape = document.shapes.iter().find(|s| s.id() == id)?;
                    let name = relation
                        .pointer(&format!("/{key}/point"))
                        .and_then(Value::as_str)?;
                    curve_end_index(shape, name).map(|end| (id, end))
                };
                if let Some((a, b)) = side("a").zip(side("b"))
                    && document.smooth_joins.get(&b) == Some(&a)
                    && let (Some(a_id), Some(b_id)) = (next_ids.get(&a.0), next_ids.get(&b.0))
                    && let (Some(a_name), Some(b_name)) = (
                        curve_end_name(&document.shapes, a),
                        curve_end_name(&document.shapes, b),
                    )
                {
                    relation["a"] = json!({"entityId": a_id, "point": a_name});
                    relation["b"] = json!({"entityId": b_id, "point": b_name});
                    return true;
                }
                return false;
            }
            if kind == "endpointTangent" {
                let arc = anchor_of(relation, "a");
                let line = resolve_id(relation.get("b").and_then(Value::as_str));
                if let Some((arc, line)) = arc.zip(line)
                    && let Some(&(stored, line_sign, sweep_sign)) =
                        document.endpoint_tangents.get(&arc)
                    && stored == line
                    && let Some((arc_id, line_id)) = next_ids.get(&arc.0).zip(next_ids.get(&line))
                {
                    relation["a"]["entityId"] = json!(arc_id);
                    relation["b"] = json!(line_id);
                    relation["orientation"] =
                        json!({"lineSign": line_sign, "sweepSign": sweep_sign});
                    return true;
                }
                return false;
            }
            if kind == "chamfer" {
                let line = resolve_id(relation.get("chamferId").and_then(Value::as_str));
                if let Some(line) = line
                    && let Some(spec) = document.chamfers.get(&line)
                    && anchor_of(relation, "a") == Some(spec.a)
                    && anchor_of(relation, "b") == Some(spec.b)
                    && let (Some(line_id), Some(a_id), Some(b_id)) = (
                        next_ids.get(&line),
                        next_ids.get(&spec.a.0),
                        next_ids.get(&spec.b.0),
                    )
                {
                    relation["chamferId"] = json!(line_id);
                    relation["a"]["entityId"] = json!(a_id);
                    relation["b"]["entityId"] = json!(b_id);
                    relation["value"] = json!(spec.first);
                    relation["secondValue"] = json!(spec.second);
                    relation["mode"] = json!(spec.mode.name());
                    return true;
                }
                return false;
            }
            if kind == "coincident"
                && relation.pointer("/a/point").and_then(Value::as_str) != Some("center")
                && relation.pointer("/b/point").and_then(Value::as_str) != Some("center")
            {
                if let Some((a, b)) = anchor_of(relation, "a").zip(anchor_of(relation, "b"))
                    && (document.endpoint_joins.get(&a) == Some(&b)
                        || document.endpoint_joins.get(&b) == Some(&a))
                    && let Some((a_id, b_id)) = next_ids.get(&a.0).zip(next_ids.get(&b.0))
                {
                    relation["a"]["entityId"] = json!(a_id);
                    relation["b"]["entityId"] = json!(b_id);
                    return true;
                }
                return false;
            }
            if kind == "coincident" {
                let read = |side: &str| -> Option<(u32, &str)> {
                    let id = relation.get(side)?.get("entityId")?.as_str()?;
                    Some((
                        *graph_shape_ids.get(id)?,
                        relation.get(side)?.get("point")?.as_str()?,
                    ))
                };
                if let Some((a, b)) = read("a").zip(read("b")) {
                    for (line, point) in [(a, b), (b, a)] {
                        if point.1 == "center" {
                            let endpoint = document
                                .shapes
                                .iter()
                                .find(|shape| shape.id() == line.0)
                                .and_then(|shape| match shape {
                                    Shape::Line { .. } => match line.1 {
                                        "p0" => Some(0),
                                        "p1" => Some(1),
                                        _ => None,
                                    },
                                    Shape::Spline {
                                        points,
                                        closed: false,
                                        ..
                                    } => {
                                        if line.1 == "point0" {
                                            Some(0)
                                        } else if line.1 == format!("point{}", points.len() - 1) {
                                            Some(1)
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                });
                            if endpoint.is_some_and(|end| {
                                document.point_links.get(&(line.0, end)) == Some(&point.0)
                            }) {
                                return true;
                            }
                        }
                    }
                }
                return false;
            }
            if matches!(kind, "horizontal" | "vertical") {
                return relation
                    .get("entityId")
                    .and_then(Value::as_str)
                    .and_then(|id| graph_shape_ids.get(id))
                    .and_then(|shape_id| document.axis_constraints.get(shape_id))
                    .is_some_and(|axis| axis.name() == kind);
            }
            let Some(field) = relation_field(kind) else {
                return true;
            };
            relation
                .get("entityId")
                .and_then(Value::as_str)
                .and_then(|id| graph_shape_ids.get(id))
                .is_some_and(|shape_id| {
                    document
                        .driving_dimensions
                        .contains_key(&(*shape_id, field))
                        || document.is_reference(DimensionKey::Field(*shape_id, field))
                })
        });
        let mut relation_ids: HashSet<String> = relations
            .iter()
            .filter_map(|relation| {
                relation
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect();
        let mut dimensions: Vec<_> = document.driving_dimensions.iter().collect();
        dimensions.sort_by_key(|((shape_id, field), _)| (*shape_id, relation_name(*field)));
        for ((shape_id, field), value) in dimensions {
            let graph_id = next_ids
                .get(shape_id)
                .ok_or("driving dimension references deleted geometry")?;
            let kind = relation_name(*field);
            let graph_kind = match field {
                DimensionField::LineLength => Some("lineLength"),
                DimensionField::RectangleWidth => Some("rectangleWidth"),
                DimensionField::RectangleHeight => Some("rectangleHeight"),
                DimensionField::CircleDiameter => None,
                DimensionField::CurveRadius => None,
                DimensionField::ArcSweep | DimensionField::Rotation => None,
            };
            if graph_kind.is_some_and(|kind| graph_backed.contains(&(*shape_id, kind.to_string())))
            {
                continue;
            }
            if let Some(existing) = relations.iter_mut().find(|relation| {
                relation.get("type").and_then(Value::as_str) == Some(kind)
                    && relation.get("entityId").and_then(Value::as_str) == Some(graph_id)
            }) {
                existing["value"] = json!(value);
            } else {
                let base = format!("native-dimension-{graph_id}-{kind}");
                let mut relation_id = base.clone();
                let mut suffix = 2;
                while !relation_ids.insert(relation_id.clone()) {
                    relation_id = format!("{base}-{suffix}");
                    suffix += 1;
                }
                relations.push(json!({
                    "id": relation_id,
                    "type": kind,
                    "entityId": graph_id,
                    "value": value
                }));
            }
        }
        for key in document.dims.references.iter().copied() {
            let DimensionKey::Field(shape_id, field) = key else {
                continue;
            };
            let Some(graph_id) = next_ids.get(&shape_id) else {
                continue;
            };
            let kind = relation_name(field);
            if relations.iter().any(|relation| {
                relation.get("type").and_then(Value::as_str) == Some(kind)
                    && relation.get("entityId").and_then(Value::as_str) == Some(graph_id)
            }) {
                continue;
            }
            let base = format!("native-dimension-{graph_id}-{kind}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            // Value and the reference flag are written by the annotations.
            relations.push(json!({
                "id": relation_id,
                "type": kind,
                "entityId": graph_id,
                "value": 0
            }));
        }
        let mut axes: Vec<_> = document.axis_constraints.iter().collect();
        axes.sort_by_key(|(shape_id, _)| **shape_id);
        for (shape_id, axis) in axes {
            let graph_id = next_ids
                .get(shape_id)
                .ok_or("axis constraint references deleted geometry")?;
            if graph_backed.contains(&(*shape_id, axis.name().to_string())) {
                continue;
            }
            if relations.iter().any(|relation| {
                relation.get("type").and_then(Value::as_str) == Some(axis.name())
                    && relation.get("entityId").and_then(Value::as_str) == Some(graph_id)
            }) {
                continue;
            }
            let base = format!("native-axis-{graph_id}-{}", axis.name());
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":axis.name(),"entityId":graph_id}));
        }
        let mut links: Vec<_> = document.point_links.iter().collect();
        links.sort_by_key(|((line_id, endpoint), point_id)| (*line_id, *endpoint, **point_id));
        for (&(line_id, endpoint), &point_id) in links {
            let line_graph_id = next_ids
                .get(&line_id)
                .ok_or("coincidence references deleted line")?;
            let point_graph_id = next_ids
                .get(&point_id)
                .ok_or("coincidence references deleted point")?;
            let curve = document
                .shapes
                .iter()
                .find(|shape| shape.id() == line_id)
                .ok_or("coincidence references deleted curve")?;
            let anchor = match curve {
                Shape::Line { .. } => {
                    if endpoint == 0 {
                        "p0".to_string()
                    } else {
                        "p1".to_string()
                    }
                }
                Shape::Spline {
                    points,
                    closed: false,
                    ..
                } => {
                    if endpoint == 0 {
                        "point0".to_string()
                    } else {
                        format!("point{}", points.len() - 1)
                    }
                }
                _ => return Err("coincidence references unsupported curve".into()),
            };
            if relations.iter().any(|relation| {
                relation["type"] == "coincident"
                    && ((relation["a"]["entityId"] == *line_graph_id
                        && relation["a"]["point"] == anchor
                        && relation["b"]["entityId"] == *point_graph_id
                        && relation["b"]["point"] == "center")
                        || (relation["b"]["entityId"] == *line_graph_id
                            && relation["b"]["point"] == anchor
                            && relation["a"]["entityId"] == *point_graph_id
                            && relation["a"]["point"] == "center"))
            }) {
                continue;
            }
            let base = format!("native-coincident-{line_graph_id}-{anchor}-{point_graph_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"coincident",
                "a":{"entityId":line_graph_id,"point":anchor},
                "b":{"entityId":point_graph_id,"point":"center"}}));
        }
        let anchor_name = |end: u8| if end == 0 { "p0" } else { "p1" };
        let mut joins: Vec<_> = document.endpoint_joins.iter().collect();
        joins.sort_by_key(|(key, value)| (**key, **value));
        for (&(line, line_end), &(other, other_end)) in joins {
            let line_id = next_ids
                .get(&line)
                .ok_or("coincidence references deleted line")?;
            let other_id = next_ids
                .get(&other)
                .ok_or("coincidence references deleted curve")?;
            let (a, b) = (anchor_name(line_end), anchor_name(other_end));
            if relations.iter().any(|relation| {
                relation["type"] == "coincident"
                    && ((relation["a"]["entityId"] == *line_id
                        && relation["a"]["point"] == a
                        && relation["b"]["entityId"] == *other_id
                        && relation["b"]["point"] == b)
                        || (relation["b"]["entityId"] == *line_id
                            && relation["b"]["point"] == a
                            && relation["a"]["entityId"] == *other_id
                            && relation["a"]["point"] == b))
            }) {
                continue;
            }
            let base = format!("native-join-{line_id}-{a}-{other_id}-{b}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"coincident",
                "a":{"entityId":line_id,"point":a},
                "b":{"entityId":other_id,"point":b}}));
        }
        let mut tangents: Vec<_> = document.endpoint_tangents.iter().collect();
        tangents.sort_by_key(|(key, _)| **key);
        for (&(arc, end), &(line, line_sign, sweep_sign)) in tangents {
            let arc_id = next_ids
                .get(&arc)
                .ok_or("smooth join references deleted arc")?;
            let line_id = next_ids
                .get(&line)
                .ok_or("smooth join references deleted line")?;
            let point = anchor_name(end);
            if relations.iter().any(|relation| {
                relation["type"] == "endpointTangent"
                    && relation["a"]["entityId"] == *arc_id
                    && relation["a"]["point"] == point
            }) {
                continue;
            }
            let base = format!("native-tangent-{arc_id}-{point}-{line_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"endpointTangent",
                "a":{"entityId":arc_id,"point":point},"b":line_id,
                "orientation":{"lineSign":line_sign,"sweepSign":sweep_sign}}));
        }
        let mut smooth: Vec<_> = document.smooth_joins.iter().collect();
        smooth.sort_by_key(|(key, _)| **key);
        for (&follower, &source) in smooth {
            let a_id = next_ids
                .get(&source.0)
                .ok_or("smooth join references deleted curve")?;
            let b_id = next_ids
                .get(&follower.0)
                .ok_or("smooth join references deleted curve")?;
            let a_name = curve_end_name(&document.shapes, source)
                .ok_or("smooth join references unsupported curve")?;
            let b_name = curve_end_name(&document.shapes, follower)
                .ok_or("smooth join references unsupported curve")?;
            if relations.iter().any(|relation| {
                relation["type"] == "smoothJoin"
                    && relation["b"]["entityId"] == *b_id
                    && relation["b"]["point"] == b_name.as_str()
            }) {
                continue;
            }
            let base = format!("native-smooth-join-{b_id}-{b_name}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"smoothJoin",
                "a":{"entityId":a_id,"point":a_name},
                "b":{"entityId":b_id,"point":b_name}}));
        }
        let mut chamfers: Vec<_> = document.chamfers.iter().collect();
        chamfers.sort_by_key(|(line, _)| **line);
        for (&line, spec) in chamfers {
            let line_id = next_ids
                .get(&line)
                .ok_or("chamfer references deleted line")?;
            let a_id = next_ids
                .get(&spec.a.0)
                .ok_or("chamfer references deleted line")?;
            let b_id = next_ids
                .get(&spec.b.0)
                .ok_or("chamfer references deleted line")?;
            if relations
                .iter()
                .any(|relation| relation["type"] == "chamfer" && relation["chamferId"] == *line_id)
            {
                continue;
            }
            let base = format!("native-chamfer-{line_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(
                json!({"id":relation_id,"type":"chamfer","chamferId":line_id,
                "a":{"entityId":a_id,"point":anchor_name(spec.a.1)},
                "b":{"entityId":b_id,"point":anchor_name(spec.b.1)},
                "value":spec.first,"secondValue":spec.second,"mode":spec.mode.name()}),
            );
        }
        let mut equal: Vec<_> = document
            .equal_lines
            .iter()
            .chain(document.equal_radii.iter())
            .chain(document.equal_ellipses.iter())
            .collect();
        equal.sort_by_key(|(target, source)| (**target, **source));
        for (&target, &source) in equal {
            let source_id = next_ids.get(&source).ok_or("Equal source is missing")?;
            let target_id = next_ids.get(&target).ok_or("Equal target is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "equal"
                    && relation["a"] == *source_id
                    && relation["b"] == *target_id
            }) {
                continue;
            }
            let base = format!("native-equal-{source_id}-{target_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"equal","a":source_id,"b":target_id}));
        }
        let mut normal_links: Vec<_> = document.normal_links.iter().collect();
        normal_links.sort_by_key(|(target, (source, endpoint))| (**target, *source, *endpoint));
        for (&target, &(source, endpoint)) in normal_links {
            let target_id = next_ids.get(&target).ok_or("Normal line is missing")?;
            let source_id = next_ids.get(&source).ok_or("Normal source is missing")?;
            let anchor = if endpoint == 0 { "p0" } else { "p1" };
            if relations.iter().any(|relation| {
                relation["type"] == "normal"
                    && relation["a"]["entityId"] == *target_id
                    && relation["a"]["point"] == anchor
                    && relation["b"] == *source_id
            }) {
                continue;
            }
            let base = format!("native-normal-{target_id}-{source_id}-{anchor}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"normal","a":{"entityId":target_id,"point":anchor},"b":source_id}));
        }
        let mut point_curve_links: Vec<_> = document.point_curve_links.iter().collect();
        point_curve_links.sort_by_key(|(point, curve)| (**point, **curve));
        for (&point, &curve) in point_curve_links {
            let point_id = next_ids
                .get(&point)
                .ok_or("Point on curve point is missing")?;
            let curve_id = next_ids
                .get(&curve)
                .ok_or("Point on curve source is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "pointOnCurve"
                    && relation["a"]["entityId"] == *point_id
                    && relation["b"] == *curve_id
            }) {
                continue;
            }
            let base = format!("native-point-on-curve-{point_id}-{curve_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"pointOnCurve","a":{"entityId":point_id,"point":"center"},"b":curve_id}));
        }
        let mut midpoint_links: Vec<_> = document.midpoint_links.iter().collect();
        midpoint_links.sort_by_key(|(point, curve)| (**point, **curve));
        for (&point, &curve) in midpoint_links {
            let point_id = next_ids.get(&point).ok_or("Midpoint point is missing")?;
            let curve_id = next_ids.get(&curve).ok_or("Midpoint curve is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "midpoint"
                    && relation["a"]["entityId"] == *point_id
                    && relation["b"] == *curve_id
            }) {
                continue;
            }
            let base = format!("native-midpoint-{point_id}-{curve_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"midpoint","a":{"entityId":point_id,"point":"center"},"b":curve_id}));
        }
        let mut external_tangents: Vec<_> = document.external_tangents.iter().collect();
        external_tangents.sort_by_key(|(target, source)| (**target, **source));
        for (&target, &source) in external_tangents {
            let source_id = next_ids
                .get(&source)
                .ok_or("External tangent source curve is missing")?;
            let target_id = next_ids
                .get(&target)
                .ok_or("External tangent target curve is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "tangent"
                    && relation["a"] == *source_id
                    && relation["b"] == *target_id
            }) {
                continue;
            }
            let base = format!("native-external-tangent-{source_id}-{target_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"tangent","a":source_id,"b":target_id}));
        }
        let mut internal_tangents: Vec<_> = document.internal_tangents.iter().collect();
        internal_tangents.sort_by_key(|(inner, outer)| (**inner, **outer));
        for (&inner, &outer) in internal_tangents {
            let outer_id = next_ids
                .get(&outer)
                .ok_or("Internal tangent outer curve is missing")?;
            let inner_id = next_ids
                .get(&inner)
                .ok_or("Internal tangent inner curve is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "internalTangent"
                    && relation["a"] == *outer_id
                    && relation["b"] == *inner_id
            }) {
                continue;
            }
            let base = format!("native-internal-tangent-{outer_id}-{inner_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations
                .push(json!({"id":relation_id,"type":"internalTangent","a":outer_id,"b":inner_id}));
        }
        let mut radial: Vec<_> = document.radius_differences.iter().collect();
        radial.sort_by_key(|(target, (source, _))| (**target, *source));
        for (&target, &(source, value)) in radial {
            let source_id = next_ids
                .get(&source)
                .ok_or("Radial distance source is missing")?;
            let target_id = next_ids
                .get(&target)
                .ok_or("Radial distance target is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "radiusDifference"
                    && relation["a"] == *source_id
                    && relation["b"] == *target_id
            }) {
                continue;
            }
            let base = format!("native-radius-difference-{source_id}-{target_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"radiusDifference","a":source_id,"b":target_id,"value":value}));
        }
        let mut shared: Vec<_> = document.shared_ellipses.iter().collect();
        shared.sort_by_key(|(target, source)| (**target, **source));
        for (&target, &source) in shared {
            let source_id = next_ids
                .get(&source)
                .ok_or("Shared ellipse source is missing")?;
            let target_id = next_ids
                .get(&target)
                .ok_or("Shared ellipse target is missing")?;
            if relations
                .iter()
                .any(|relation| relation["type"] == "sameEllipse" && relation["b"] == *target_id)
            {
                continue;
            }
            let base = format!("native-same-ellipse-{source_id}-{target_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations
                .push(json!({"id":relation_id,"type":"sameEllipse","a":source_id,"b":target_id}));
        }
        let mut line_relations: Vec<_> = document.line_relations.iter().collect();
        line_relations.sort_by_key(|(target, (source, _))| (**target, *source));
        for (&target, &(source, kind)) in line_relations {
            let source_id = next_ids.get(&source).ok_or("Direction source is missing")?;
            let target_id = next_ids.get(&target).ok_or("Direction target is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == kind.name()
                    && relation["a"] == *source_id
                    && relation["b"] == *target_id
            }) {
                continue;
            }
            let base = format!("native-{}-{source_id}-{target_id}", kind.name());
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            let mut relation =
                json!({"id":relation_id,"type":kind.name(),"a":source_id,"b":target_id});
            if let Some(value) = kind.angle_value() {
                relation["value"] = json!(value);
            }
            relations.push(relation);
        }
        let mut concentric: Vec<_> = document.concentric.iter().collect();
        concentric.sort_by_key(|(target, source)| (**target, **source));
        for (&target, &source) in concentric {
            let source_id = next_ids
                .get(&source)
                .ok_or("Concentric source is missing")?;
            let target_id = next_ids
                .get(&target)
                .ok_or("Concentric target is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "concentric"
                    && relation["a"] == *source_id
                    && relation["b"] == *target_id
            }) {
                continue;
            }
            let base = format!("native-concentric-{source_id}-{target_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations
                .push(json!({"id":relation_id,"type":"concentric","a":source_id,"b":target_id}));
        }
        let mut mirrors: Vec<_> = document.mirrors.iter().collect();
        mirrors.sort_by_key(|(target, _)| **target);
        for (&target, &(source, axis)) in mirrors {
            let source_id = next_ids.get(&source).ok_or("Mirror source is missing")?;
            let target_id = next_ids.get(&target).ok_or("Mirror target is missing")?;
            let axis_id = next_ids.get(&axis).ok_or("Mirror line is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "mirror"
                    && relation["a"] == *source_id
                    && relation["b"] == *target_id
                    && relation["symmetryLine"] == *axis_id
            }) {
                continue;
            }
            let base = format!("native-mirror-{source_id}-{target_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({
                "id": relation_id,
                "type": "mirror",
                "a": source_id,
                "b": target_id,
                "symmetryLine": axis_id
            }));
        }
        let mut offsets: Vec<_> = document.offsets.iter().collect();
        offsets.sort_by_key(|(target, _)| **target);
        for (&target, &(source, distance)) in offsets {
            let source_id = next_ids.get(&source).ok_or("Offset source is missing")?;
            let target_id = next_ids.get(&target).ok_or("Offset target is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "offset"
                    && relation["a"] == *source_id
                    && relation["b"] == *target_id
            }) {
                continue;
            }
            let base = format!("native-offset-{source_id}-{target_id}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({
                "id": relation_id,
                "type": "offset",
                "a": source_id,
                "b": target_id,
                "value": distance
            }));
        }
        for chain in &document.offset_chains {
            let targets: Vec<Value> = chain
                .targets
                .iter()
                .map(|id| {
                    next_ids
                        .get(id)
                        .map(|id| json!(id))
                        .ok_or("Offset chain target is missing")
                })
                .collect::<Result<_, _>>()?;
            if relations.iter().any(|relation| {
                relation["type"] == "offsetChain"
                    && relation["targets"].as_array() == Some(&targets)
            }) {
                continue;
            }
            let base = format!(
                "native-offset-chain-{}",
                targets[0].as_str().unwrap_or_default()
            );
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(
                offset_chain_json(chain, &json!(relation_id), &next_ids)
                    .ok_or("Offset chain source is missing")?,
            );
        }
        for pair in &document.symmetric_points {
            let anchor = |(id, index): (u32, usize)| -> Result<Value, String> {
                let graph_id = next_ids.get(&id).ok_or("Symmetric point is missing")?;
                let shape = document
                    .shapes
                    .iter()
                    .find(|shape| shape.id() == id)
                    .ok_or("Symmetric point is missing")?;
                let name = symmetry_anchor_name(shape, index).ok_or("Invalid symmetric point")?;
                Ok(json!({"entityId": graph_id, "point": name}))
            };
            let (a, b) = (anchor(pair.a)?, anchor(pair.b)?);
            let axis_id = next_ids.get(&pair.axis).ok_or("Symmetry line is missing")?;
            if relations.iter().any(|relation| {
                relation["type"] == "symmetric"
                    && relation["a"] == a
                    && relation["b"] == b
                    && relation["symmetryLine"] == *axis_id
            }) {
                continue;
            }
            let base = format!(
                "native-symmetric-{}-{}",
                a["entityId"].as_str().unwrap_or(""),
                b["entityId"].as_str().unwrap_or("")
            );
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({
                "id": relation_id,
                "type": "symmetric",
                "a": a,
                "b": b,
                "symmetryLine": axis_id
            }));
        }
        let mut fixed: Vec<_> = document.fixed_anchors.iter().collect();
        fixed.sort_by_key(|((shape_id, index), _)| (*shape_id, *index));
        for (&(shape_id, index), &position) in fixed {
            let graph_id = next_ids
                .get(&shape_id)
                .ok_or("fixed point references deleted geometry")?;
            let anchor = match document.shapes.iter().find(|shape| shape.id() == shape_id) {
                Some(Shape::Point { .. }) if index == 0 => "center".to_string(),
                Some(Shape::Arc { .. }) | Some(Shape::EllipticArc { .. }) if index <= 2 => {
                    if index == 0 {
                        "center".to_string()
                    } else {
                        format!("p{}", index - 1)
                    }
                }
                Some(Shape::Circle { .. }) if index == 0 => "center".to_string(),
                Some(Shape::Ellipse { .. }) | Some(Shape::RotatedRectangle { .. })
                    if index <= 4 =>
                {
                    if index == 0 {
                        "center".to_string()
                    } else {
                        format!("p{}", index - 1)
                    }
                }
                Some(Shape::Polygon { sides, .. }) if index <= *sides as usize => {
                    if index == 0 {
                        "center".to_string()
                    } else {
                        format!("vertex{}", index - 1)
                    }
                }
                Some(Shape::Capsule { .. }) if index < 2 => {
                    if index == 0 {
                        "from".to_string()
                    } else {
                        "to".to_string()
                    }
                }
                Some(Shape::Line { .. }) | Some(Shape::Rectangle { .. }) if index < 2 => {
                    format!("p{index}")
                }
                Some(Shape::Spline { points, .. }) if index < points.len() => {
                    format!("point{index}")
                }
                _ => return Err("fixed point has unsupported anchor".into()),
            };
            if relations.iter().any(|relation| {
                relation["type"] == "fixed"
                    && relation["anchor"]["entityId"] == *graph_id
                    && relation["anchor"]["point"] == anchor
            }) {
                continue;
            }
            let base = format!("native-fixed-{graph_id}-{anchor}");
            let mut relation_id = base.clone();
            let mut suffix = 2;
            while !relation_ids.insert(relation_id.clone()) {
                relation_id = format!("{base}-{suffix}");
                suffix += 1;
            }
            relations.push(json!({"id":relation_id,"type":"fixed",
                "anchor":{"entityId":graph_id,"point":anchor},
                "position":{"x":position.x,"y":position.y},"fixation":graph_id}));
        }
        for (&id, dimension) in &document.dims.point_dimensions {
            let anchor = |(shape_id, index): (u32, usize)| -> Option<Value> {
                let shape = document
                    .shapes
                    .iter()
                    .find(|shape| shape.id() == shape_id)?;
                Some(json!({
                    "entityId": next_ids.get(&shape_id)?,
                    "point": dimension_anchor_name(shape, index)?,
                }))
            };
            let (Some(a), Some(b)) = (anchor(dimension.a), anchor(dimension.b)) else {
                // Geometry of a pruned dimension is gone; drop it.
                continue;
            };
            let relation_id = match &dimension.graph_id {
                Some(graph_id) if relation_ids.insert(graph_id.clone()) => graph_id.clone(),
                _ => {
                    let base = format!("native-distance-{id}");
                    let mut relation_id = base.clone();
                    let mut suffix = 2;
                    while !relation_ids.insert(relation_id.clone()) {
                        relation_id = format!("{base}-{suffix}");
                        suffix += 1;
                    }
                    relation_id
                }
            };
            let mut relation = json!({
                "id": relation_id,
                "type": "distance",
                "a": a,
                "b": b,
                "value": dimension.value,
            });
            if let Some(axis) = dimension.axis {
                relation["axis"] = json!(axis.name());
            }
            annotate_dimension(document, &mut relation, DimensionKey::Point(id));
            relations.push(relation);
        }
        save_dimension_meta(document, relations, &graph_shape_ids);
        workspace.entity_ids = next_ids;
        workspace.original_entities = next_originals;
    }
    if !removed_constraint_ids.is_empty() {
        for path in ["cad/rootNodeIds", "tree/orderedNodeIds"] {
            if let Some(entries) = next
                .pointer_mut(&format!("/items/{part_index}/data/{path}"))
                .and_then(Value::as_array_mut)
            {
                entries.retain(|entry| {
                    entry
                        .as_str()
                        .is_none_or(|id| !removed_constraint_ids.contains(id))
                });
            }
        }
    }
    Ok(())
}

/// Dimension a web relation describes, if the document has it.
fn relation_dimension_key(
    document: &Document,
    relation: &Value,
    graph_shape_ids: &HashMap<&str, u32>,
) -> Option<DimensionKey> {
    let kind = relation.get("type").and_then(Value::as_str)?;
    let shape = |field: &str| {
        relation
            .get(field)
            .and_then(Value::as_str)
            .and_then(|id| graph_shape_ids.get(id))
            .copied()
    };
    let key = if kind == "angle" {
        DimensionKey::Angle(shape("b")?)
    } else {
        DimensionKey::Field(shape("entityId")?, relation_field(kind)?)
    };
    document.dimension_keys().contains(&key).then_some(key)
}

/// Reads `labelPosition` of dimension relations.
fn load_dimension_meta(
    document: &mut Document,
    relations: &[Value],
    graph_shape_ids: &HashMap<&str, u32>,
) -> Result<(), String> {
    for relation in relations {
        let Some(key) = relation_dimension_key(document, relation, graph_shape_ids) else {
            continue;
        };
        if let Some(position) = relation.get("labelPosition") {
            let at = read_point(position)
                .filter(|at| at.x.is_finite() && at.y.is_finite())
                .ok_or("invalid dimension label position")?;
            document.dims.label_positions.insert(key, at);
        }
        load_expression(document, relation, key)?;
    }
    Ok(())
}

/// Reads a dimension relation's `expression`; it must evaluate to the
/// stored value.
fn load_expression(
    document: &mut Document,
    relation: &Value,
    key: DimensionKey,
) -> Result<(), String> {
    let Some(expression) = relation.get("expression") else {
        return Ok(());
    };
    let expression = expression.as_str().ok_or("invalid dimension expression")?;
    document
        .dims
        .expressions
        .insert(key, expression.to_string());
    if document.dimension_expression(key).is_none() {
        return Err("dimension expression does not match its value".into());
    }
    Ok(())
}

/// Sketch `variables` (`normalizeSketchVariables`).
fn read_variables(
    value: Option<&Value>,
) -> Result<Vec<crate::sketch_value::SketchVariable>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or("sketch variables must be an array")?;
    let variables = entries
        .iter()
        .map(|entry| {
            Some(crate::sketch_value::SketchVariable {
                name: entry.get("name")?.as_str()?.to_string(),
                kind: crate::sketch_value::ValueKind::from_name(entry.get("kind")?.as_str()?)?,
                expression: entry.get("expression")?.as_str()?.to_string(),
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or("invalid sketch variable definition")?;
    crate::sketch_value::evaluate_variables(&variables)?;
    Ok(variables)
}

/// Writes `labelPosition` of dimension relations.
fn save_dimension_meta(
    document: &Document,
    relations: &mut [Value],
    graph_shape_ids: &HashMap<&str, u32>,
) {
    for relation in relations.iter_mut() {
        if let Some(key) = relation_dimension_key(document, relation, graph_shape_ids) {
            annotate_dimension(document, relation, key);
        }
    }
}

/// Writes a dimension's annotations onto its web relation.
fn annotate_dimension(document: &Document, relation: &mut Value, key: DimensionKey) {
    let Some(object) = relation.as_object_mut() else {
        return;
    };
    match document.dims.label_positions.get(&key) {
        Some(at) => {
            object.insert("labelPosition".into(), json!({"x": at.x, "y": at.y}));
        }
        None => {
            object.remove("labelPosition");
        }
    }
    match document.dimension_expression(key).and_then(|expression| {
        let value = document
            .evaluate_expression(expression, Document::dimension_value_kind(key))
            .ok()?;
        Some((expression, value))
    }) {
        Some((expression, value)) => {
            // The exact evaluated value, as the web loader checks it.
            object.insert("expression".into(), json!(expression));
            object.insert("value".into(), json!(value));
        }
        None => {
            object.remove("expression");
        }
    }
    if document.is_reference(key) {
        object.insert("reference".into(), json!(true));
        if let Some(value) = document.measure_dimension(key) {
            object.insert("value".into(), json!(value));
        }
    } else {
        object.remove("reference");
    }
}

/// Web anchor name of a dimensionable anchor (see
/// `Document::dimension_anchor`).
fn dimension_anchor_name(shape: &Shape, index: usize) -> Option<String> {
    Some(match shape {
        Shape::Point { .. } if index == 0 => "center".into(),
        Shape::Line { .. } if index < 2 => format!("p{index}"),
        Shape::Circle { .. }
        | Shape::Arc { .. }
        | Shape::Ellipse { .. }
        | Shape::EllipticArc { .. }
            if index == 0 =>
        {
            "center".into()
        }
        Shape::Spline { points, .. } if index < points.len() => format!("point{index}"),
        _ => return None,
    })
}

fn dimension_anchor_index(shape: &Shape, name: &str) -> Option<usize> {
    let index = match shape {
        Shape::Line { .. } => name.strip_prefix('p')?.parse().ok()?,
        Shape::Spline { .. } => name.strip_prefix("point")?.parse().ok()?,
        _ if name == "center" => 0,
        _ => return None,
    };
    (dimension_anchor_name(shape, index).as_deref() == Some(name)).then_some(index)
}

/// Loads a web `distance` relation as a point dimension.
fn load_point_dimension(
    document: &mut Document,
    relation: &Value,
    entity_ids: &HashMap<u32, String>,
) -> Result<(), String> {
    let unsupported = || "unsupported distance dimension".to_string();
    let id = relation
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(unsupported)?;
    let anchor = |side: &str| -> Option<(u32, usize)> {
        let graph_id = relation.pointer(&format!("/{side}/entityId"))?.as_str()?;
        let name = relation.pointer(&format!("/{side}/point"))?.as_str()?;
        let shape_id = entity_ids
            .iter()
            .find_map(|(shape_id, id)| (id == graph_id).then_some(*shape_id))?;
        let shape = document
            .shapes
            .iter()
            .find(|shape| shape.id() == shape_id)?;
        Some((shape_id, dimension_anchor_index(shape, name)?))
    };
    let (a, b) = anchor("a").zip(anchor("b")).ok_or_else(unsupported)?;
    let axis = match relation.get("axis") {
        None => None,
        Some(axis) if axis == "x" => Some(crate::model::PointAxis::X),
        Some(axis) if axis == "y" => Some(crate::model::PointAxis::Y),
        Some(_) => return Err(unsupported()),
    };
    let reference = relation.get("reference") == Some(&Value::Bool(true));
    let value = relation
        .get("value")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && value.abs() <= f32::MAX as f64)
        .ok_or_else(unsupported)? as f32;
    if relation.get("direction").is_some()
        || relation
            .get("reference")
            .is_some_and(|reference| !reference.is_boolean())
        || (reference && relation.get("expression").is_some())
        || a == b
        || (axis.is_none() && value <= 0.0 && !reference)
    {
        return Err(unsupported());
    }
    let key = document
        .dims
        .point_dimensions
        .keys()
        .next_back()
        .map_or(1, |last| last + 1);
    document.dims.point_dimensions.insert(
        key,
        crate::model::PointDimension {
            a,
            b,
            axis,
            value,
            graph_id: Some(id.to_string()),
        },
    );
    if reference {
        document.dims.references.insert(DimensionKey::Point(key));
    }
    load_expression(document, relation, DimensionKey::Point(key))?;
    if let Some(position) = relation.get("labelPosition") {
        let at = read_point(position).ok_or("invalid dimension label position")?;
        document
            .dims
            .label_positions
            .insert(DimensionKey::Point(key), at);
    }
    Ok(())
}

fn is_construction_node(node: &Value) -> bool {
    node.get("construction").and_then(Value::as_bool) == Some(true)
        || node
            .pointer("/primitive/construction")
            .and_then(Value::as_bool)
            == Some(true)
        || node.pointer("/arc/construction").and_then(Value::as_bool) == Some(true)
        || node
            .pointer("/spline/construction")
            .and_then(Value::as_bool)
            == Some(true)
}

fn apply_construction(node: &mut Value, enabled: bool) -> Result<(), String> {
    let kind = node
        .get("type")
        .and_then(Value::as_str)
        .ok_or("sketch entity has no type")?;
    let target = match kind {
        "sketchPrimitive" => node.get_mut("primitive"),
        "sketchArc" => node.get_mut("arc"),
        "sketchSpline" => node.get_mut("spline"),
        _ => Some(node),
    }
    .and_then(Value::as_object_mut)
    .ok_or("sketch entity has no geometry")?;
    if enabled {
        target.insert("construction".into(), Value::Bool(true));
    } else {
        target.remove("construction");
    }
    Ok(())
}

/// Web anchor name -> native symmetry anchor index (see
/// `Document::symmetry_anchor_point`).
fn symmetry_anchor_index(shape: &Shape, name: &str) -> Option<usize> {
    match shape {
        Shape::Line { .. } => match name {
            "p0" => Some(0),
            "p1" => Some(1),
            _ => None,
        },
        Shape::Point { .. }
        | Shape::Circle { .. }
        | Shape::Arc { .. }
        | Shape::Ellipse { .. }
        | Shape::EllipticArc { .. } => (name == "center").then_some(0),
        Shape::Spline { points, .. } => name
            .strip_prefix("point")?
            .parse::<usize>()
            .ok()
            .filter(|index| *index < points.len()),
        _ => None,
    }
}

fn symmetry_anchor_name(shape: &Shape, index: usize) -> Option<String> {
    match (shape, index) {
        (Shape::Line { .. }, 0) => Some("p0".into()),
        (Shape::Line { .. }, 1) => Some("p1".into()),
        (
            Shape::Point { .. }
            | Shape::Circle { .. }
            | Shape::Arc { .. }
            | Shape::Ellipse { .. }
            | Shape::EllipticArc { .. },
            0,
        ) => Some("center".into()),
        (Shape::Spline { points, .. }, index) if index < points.len() => {
            Some(format!("point{index}"))
        }
        _ => None,
    }
}

/// Web `offsetChain` relation JSON for a native offset chain.
fn offset_chain_json(
    chain: &crate::model::OffsetChain,
    id: &Value,
    next_ids: &HashMap<u32, String>,
) -> Option<Value> {
    let sources = chain
        .sources
        .iter()
        .map(|&(source, reversed)| {
            next_ids
                .get(&source)
                .map(|id| json!({"entityId": id, "reversed": reversed}))
        })
        .collect::<Option<Vec<_>>>()?;
    let targets = chain
        .targets
        .iter()
        .map(|target| next_ids.get(target).map(|id| json!(id)))
        .collect::<Option<Vec<_>>>()?;
    Some(json!({
        "id": id,
        "type": "offsetChain",
        "sources": sources,
        "closed": chain.closed,
        "side": chain.side,
        "targets": targets,
        "value": chain.distance
    }))
}

/// Load a web `offset` or `offsetChain` relation into `document`. Returns
/// false when the relation is invalid or its geometry does not match.
fn load_offset_relation(
    relation: &Value,
    entity_ids: &HashMap<u32, String>,
    document: &mut Document,
) -> bool {
    let shape_of = |graph_id: Option<&str>| {
        graph_id.and_then(|graph_id| {
            entity_ids
                .iter()
                .find_map(|(shape_id, id)| (id == graph_id).then_some(*shape_id))
        })
    };
    let plain = relation.get("id").and_then(Value::as_str).is_some()
        && relation.get("reference") != Some(&Value::Bool(true))
        && relation.get("expression").is_none();
    let value = relation
        .get("value")
        .and_then(Value::as_f64)
        .filter(|value| value.is_finite() && value.abs() <= f32::MAX as f64)
        .map(|value| value as f32);
    let (true, Some(value)) = (plain, value) else {
        return false;
    };
    let taken = |document: &Document, id: u32| {
        document.offsets.contains_key(&id)
            || document
                .offset_chains
                .iter()
                .any(|chain| chain.targets.contains(&id))
    };
    let find = |document: &Document, id: u32| -> Option<Shape> {
        document
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .cloned()
    };
    if relation["type"] == "offset" {
        let source = shape_of(relation.get("a").and_then(Value::as_str));
        let target = shape_of(relation.get("b").and_then(Value::as_str));
        let (Some(source), Some(target)) = (source, target) else {
            return false;
        };
        if source == target || taken(document, target) {
            return false;
        }
        let (Some(source_shape), Some(target_shape)) =
            (find(document, source), find(document, target))
        else {
            return false;
        };
        let Ok(expected) = crate::model::offset_shape(&source_shape, value) else {
            return false;
        };
        if !offset_geometry_matches(&expected, &target_shape) {
            return false;
        }
        document.offsets.insert(target, (source, value));
        return true;
    }
    let sources: Option<Vec<(u32, bool)>> = relation
        .get("sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources
                .iter()
                .map(|source| {
                    Some((
                        shape_of(source.get("entityId")?.as_str())?,
                        source.get("reversed")?.as_bool()?,
                    ))
                })
                .collect()
        });
    let targets: Option<Vec<u32>> = relation
        .get("targets")
        .and_then(Value::as_array)
        .and_then(|targets| targets.iter().map(|id| shape_of(id.as_str())).collect());
    let closed = relation.get("closed").and_then(Value::as_bool);
    let side = relation.get("side").and_then(Value::as_i64);
    let (Some(sources), Some(targets), Some(closed), Some(side @ (1 | -1))) =
        (sources, targets, closed, side)
    else {
        return false;
    };
    let mut ids: HashSet<u32> = sources.iter().map(|(id, _)| *id).collect();
    ids.extend(targets.iter().copied());
    if sources.len() < 2
        || targets.len() != sources.len()
        || ids.len() != sources.len() * 2
        || targets.iter().any(|&id| taken(document, id))
    {
        return false;
    }
    let segment = |shape: Option<Shape>, reversed: bool| match shape {
        Some(Shape::Line { a, b, .. }) => {
            let (a, b) = if reversed { (b, a) } else { (a, b) };
            Some(((a.x as f64, a.y as f64), (b.x as f64, b.y as f64)))
        }
        _ => None,
    };
    let Some(source_segments) = sources
        .iter()
        .map(|&(id, reversed)| segment(find(document, id), reversed))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let Some(target_segments) = targets
        .iter()
        .map(|&id| segment(find(document, id), false))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    let (expected, _) =
        crate::solver::mitered_offset(&source_segments, closed, value as f64 * side as f64);
    let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1) < 1e-3;
    if !expected
        .iter()
        .zip(&target_segments)
        .all(|(e, t)| near(e.0, t.0) && near(e.1, t.1))
    {
        return false;
    }
    document.offset_chains.push(crate::model::OffsetChain {
        sources,
        targets,
        closed,
        side: side as i8,
        distance: value,
    });
    true
}

/// Whether `actual` matches the `expected` offset geometry.
fn offset_geometry_matches(expected: &Shape, actual: &Shape) -> bool {
    match (expected, actual) {
        (Shape::Line { a, b, .. }, Shape::Line { a: c, b: d, .. }) => {
            a.distance(*c) < 1e-3 && b.distance(*d) < 1e-3
        }
        (
            Shape::Circle { center, radius, .. },
            Shape::Circle {
                center: c,
                radius: q,
                ..
            },
        ) => center.distance(*c) < 1e-3 && (radius - q).abs() < 1e-3,
        (
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            },
            Shape::Arc {
                center: c,
                radius: q,
                start_angle: t,
                sweep: v,
                ..
            },
        ) => {
            center.distance(*c) < 1e-3
                && (radius - q).abs() < 1e-3
                && (start_angle - t).abs() < 1e-6
                && (sweep - v).abs() < 1e-6
        }
        _ => false,
    }
}

/// Whether `target` has the kind a mirror of `source` across line `axis` has.
fn mirror_kinds_match(document: &Document, source: u32, target: u32, axis: u32) -> bool {
    let find = |id: u32| document.shapes.iter().find(|shape| shape.id() == id);
    let (Some(source), Some(target), Some(Shape::Line { a, b, .. })) =
        (find(source), find(target), find(axis))
    else {
        return false;
    };
    crate::model::mirror_shape(source, *a, *b)
        .is_some_and(|expected| std::mem::discriminant(&expected) == std::mem::discriminant(target))
}

fn read_point(value: &Value) -> Option<Point> {
    let x = value.get("x")?.as_f64()? as f32;
    let y = value.get("y")?.as_f64()? as f32;
    (x.is_finite() && y.is_finite()).then_some(Point { x, y })
}

fn read_shape(node: &Value, id: u32) -> Option<Shape> {
    match node.get("type")?.as_str()? {
        "sketchLine" => Some(Shape::Line {
            id,
            a: point(node.get("p0")?)?,
            b: point(node.get("p1")?)?,
        }),
        "sketchCornerRectangle" => Some(Shape::Rectangle {
            id,
            a: point(node.get("p0")?)?,
            b: point(node.get("p1")?)?,
        }),
        "sketchArc" => {
            let arc = node.get("arc")?;
            if arc.get("type")?.as_str()? != "arc" {
                return None;
            }
            let radius = positive(arc.get("radius")?)?;
            let start_angle = arc.get("startAngle")?.as_f64()?;
            let sweep = arc.get("sweep")?.as_f64()?;
            let segments = arc.get("segments")?.as_u64()?;
            if !start_angle.is_finite()
                || !sweep.is_finite()
                || sweep.abs() < 1e-9
                || sweep.abs() >= std::f64::consts::TAU
                || !(8..=4096).contains(&segments)
            {
                return None;
            }
            Some(Shape::Arc {
                id,
                center: point(arc.get("center")?)?,
                radius,
                start_angle,
                sweep,
                segments: segments as u16,
            })
        }
        "sketchSpline" => {
            let source = node.get("spline")?;
            if source.get("type")?.as_str()? != "spline"
                || source.get("id")?.as_str()? != node.get("id")?.as_str()?
            {
                return None;
            }
            let mode = match source.get("mode")?.as_str()? {
                "fit" => SplineMode::Fit,
                "control" => SplineMode::Control,
                _ => return None,
            };
            let closed = source
                .get("closed")
                .map(Value::as_bool)
                .unwrap_or(Some(false))?;
            let points = source
                .get("points")?
                .as_array()?
                .iter()
                .map(point)
                .collect::<Option<Vec<_>>>()?;
            spline::beziers(mode, &points, closed).ok()?;
            Some(Shape::Spline {
                id,
                mode,
                points,
                closed,
            })
        }
        "sketchPrimitive" => {
            let primitive = node.get("primitive")?;
            match primitive.get("type")?.as_str()? {
                "circle" => {
                    let radius = positive(primitive.get("radius")?)?;
                    let segments = primitive.get("segments")?.as_u64()?;
                    if !(3..=4096).contains(&segments) {
                        return None;
                    }
                    Some(Shape::Circle {
                        id,
                        center: point(primitive.get("center")?)?,
                        radius,
                        segments: segments as u16,
                    })
                }
                "ellipse" => {
                    let rotation = primitive.get("rotation")?.as_f64()? as f32;
                    let segments = primitive.get("segments")?.as_u64()?;
                    if !rotation.is_finite() || !(8..=4096).contains(&segments) {
                        return None;
                    }
                    Some(Shape::Ellipse {
                        id,
                        center: point(primitive.get("center")?)?,
                        rx: positive(primitive.get("width")?)? * 0.5,
                        ry: positive(primitive.get("height")?)? * 0.5,
                        rotation,
                        segments: segments as u16,
                    })
                }
                "ellipticArc" => {
                    let rotation = primitive.get("rotation")?.as_f64()? as f32;
                    let start_angle = primitive.get("startAngle")?.as_f64()?;
                    let sweep = primitive.get("sweep")?.as_f64()?;
                    let segments = primitive.get("segments")?.as_u64()?;
                    if !rotation.is_finite()
                        || !start_angle.is_finite()
                        || !sweep.is_finite()
                        || sweep == 0.0
                        || sweep.abs() > std::f64::consts::TAU
                        || !(8..=4096).contains(&segments)
                    {
                        return None;
                    }
                    Some(Shape::EllipticArc {
                        id,
                        center: point(primitive.get("center")?)?,
                        rx: positive(primitive.get("width")?)? * 0.5,
                        ry: positive(primitive.get("height")?)? * 0.5,
                        rotation,
                        start_angle,
                        sweep,
                        segments: segments as u16,
                    })
                }
                "point" => Some(Shape::Point {
                    id,
                    at: point(primitive.get("center")?)?,
                }),
                "polygon" => {
                    let radius = positive(primitive.get("radius")?)?;
                    let rotation = primitive.get("rotation")?.as_f64()? as f32;
                    let sides = primitive.get("sides")?.as_u64()?;
                    if !rotation.is_finite() || !(3..=64).contains(&sides) {
                        return None;
                    }
                    Some(Shape::Polygon {
                        id,
                        center: point(primitive.get("center")?)?,
                        radius,
                        sides: sides as u8,
                        rotation,
                    })
                }
                "capsule" => {
                    let from = point(primitive.get("from")?)?;
                    let to = point(primitive.get("to")?)?;
                    let width = positive(primitive.get("width")?)?;
                    let arc_segments = primitive.get("arcSegments")?.as_u64()?;
                    if from.distance(to) <= 1e-6 || !(1..=4096).contains(&arc_segments) {
                        return None;
                    }
                    Some(Shape::Capsule {
                        id,
                        from,
                        to,
                        width,
                        arc_segments: arc_segments as u16,
                    })
                }
                "rectangle" => {
                    let center = point(primitive.get("center")?)?;
                    let width = positive(primitive.get("width")?)?;
                    let height = positive(primitive.get("height")?)?;
                    let rotation = primitive.get("rotation")?.as_f64()? as f32;
                    if !rotation.is_finite() {
                        return None;
                    }
                    Some(Shape::RotatedRectangle {
                        id,
                        center,
                        width,
                        height,
                        rotation,
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn relation_field(kind: &str) -> Option<DimensionField> {
    match kind {
        "length" => Some(DimensionField::LineLength),
        "width" => Some(DimensionField::RectangleWidth),
        "height" => Some(DimensionField::RectangleHeight),
        "diameter" => Some(DimensionField::CircleDiameter),
        "radius" => Some(DimensionField::CurveRadius),
        "arcSweep" => Some(DimensionField::ArcSweep),
        "rotation" => Some(DimensionField::Rotation),
        _ => None,
    }
}

fn graph_constraint_field(kind: &str) -> Option<DimensionField> {
    match kind {
        "lineLength" => Some(DimensionField::LineLength),
        "rectangleWidth" => Some(DimensionField::RectangleWidth),
        "rectangleHeight" => Some(DimensionField::RectangleHeight),
        _ => None,
    }
}

fn relation_name(field: DimensionField) -> &'static str {
    match field {
        DimensionField::LineLength => "length",
        DimensionField::RectangleWidth => "width",
        DimensionField::RectangleHeight => "height",
        DimensionField::CircleDiameter => "diameter",
        DimensionField::CurveRadius => "radius",
        DimensionField::ArcSweep => "arcSweep",
        DimensionField::Rotation => "rotation",
    }
}

fn write_shape(shape: &Shape, graph_id: &str, sketch_id: &str) -> Result<Value, String> {
    let finite = |n: f32| n.is_finite();
    match shape {
        Shape::Spline {
            mode,
            points,
            closed,
            ..
        } if spline::beziers(*mode, points, *closed).is_ok() => {
            let mode = match mode {
                SplineMode::Fit => "fit",
                SplineMode::Control => "control",
            };
            Ok(
                json!({"id":graph_id,"type":"sketchSpline","sketchId":sketch_id,
                "spline":{"id":graph_id,"type":"spline","mode":mode,"points":points.iter().map(|point| json!({"x":point.x,"y":point.y})).collect::<Vec<_>>(),"closed":closed}}),
            )
        }
        Shape::Line { a, b, .. }
            if [a.x, a.y, b.x, b.y].into_iter().all(finite) && a.distance(*b) > 0.0 =>
        {
            Ok(
                json!({"id":graph_id,"type":"sketchLine","sketchId":sketch_id,"p0":{"x":a.x,"y":a.y},"p1":{"x":b.x,"y":b.y}}),
            )
        }
        Shape::Rectangle { a, b, .. }
            if [a.x, a.y, b.x, b.y].into_iter().all(finite)
                && (a.x - b.x).abs() > 0.0
                && (a.y - b.y).abs() > 0.0 =>
        {
            Ok(
                json!({"id":graph_id,"type":"sketchCornerRectangle","sketchId":sketch_id,"p0":{"x":a.x,"y":a.y},"p1":{"x":b.x,"y":b.y}}),
            )
        }
        Shape::RotatedRectangle {
            center,
            width,
            height,
            rotation,
            ..
        } if [center.x, center.y, *width, *height, *rotation]
            .into_iter()
            .all(finite)
            && *width > 0.0
            && *height > 0.0 =>
        {
            Ok(
                json!({"id":graph_id,"type":"sketchPrimitive","sketchId":sketch_id,
                    "primitive":{"id":graph_id,"type":"rectangle","center":{"x":center.x,"y":center.y},"width":width,"height":height,"rotation":rotation}
                }),
            )
        }
        Shape::Circle {
            center,
            radius,
            segments,
            ..
        } if [center.x, center.y, *radius].into_iter().all(finite)
            && *radius > 0.0
            && (3..=4096).contains(segments) =>
        {
            Ok(json!({
                "id":graph_id,"type":"sketchPrimitive","sketchId":sketch_id,
                "primitive":{"id":graph_id,"type":"circle","center":{"x":center.x,"y":center.y},"radius":radius,"segments":segments}
            }))
        }
        Shape::Ellipse {
            center,
            rx,
            ry,
            rotation,
            segments,
            ..
        } if [center.x, center.y, *rx, *ry, *rotation]
            .into_iter()
            .all(finite)
            && *rx > 0.0
            && *ry > 0.0
            && (8..=4096).contains(segments) =>
        {
            Ok(json!({
                "id":graph_id,"type":"sketchPrimitive","sketchId":sketch_id,
                "primitive":{"id":graph_id,"type":"ellipse","center":{"x":center.x,"y":center.y},"width":rx*2.0,"height":ry*2.0,"rotation":rotation,"segments":segments}
            }))
        }
        Shape::EllipticArc {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep,
            segments,
            ..
        } if [center.x, center.y, *rx, *ry, *rotation]
            .into_iter()
            .all(finite)
            && start_angle.is_finite()
            && sweep.is_finite()
            && *rx > 0.0
            && *ry > 0.0
            && *sweep != 0.0
            && sweep.abs() <= std::f64::consts::TAU
            && (8..=4096).contains(segments) =>
        {
            Ok(json!({
                "id":graph_id,"type":"sketchPrimitive","sketchId":sketch_id,
                "primitive":{"id":graph_id,"type":"ellipticArc","center":{"x":center.x,"y":center.y},"width":rx*2.0,"height":ry*2.0,"rotation":rotation,"startAngle":start_angle,"sweep":sweep,"segments":segments}
            }))
        }
        Shape::Point { at, .. } if [at.x, at.y].into_iter().all(finite) => Ok(json!({
            "id":graph_id,"type":"sketchPrimitive","sketchId":sketch_id,
            "primitive":{"id":graph_id,"type":"point","center":{"x":at.x,"y":at.y}}
        })),
        Shape::Polygon {
            center,
            radius,
            sides,
            rotation,
            ..
        } if [center.x, center.y, *radius, *rotation]
            .into_iter()
            .all(finite)
            && *radius > 0.0
            && (3..=64).contains(sides) =>
        {
            Ok(json!({
                "id":graph_id,"type":"sketchPrimitive","sketchId":sketch_id,
                "primitive":{"id":graph_id,"type":"polygon","center":{"x":center.x,"y":center.y},"radius":radius,"sides":sides,"rotation":rotation}
            }))
        }
        Shape::Capsule {
            from,
            to,
            width,
            arc_segments,
            ..
        } if [from.x, from.y, to.x, to.y, *width].into_iter().all(finite)
            && from.distance(*to) > 0.0
            && *width > 0.0
            && (1..=4096).contains(arc_segments) =>
        {
            Ok(json!({
                "id":graph_id,"type":"sketchPrimitive","sketchId":sketch_id,
                "primitive":{"id":graph_id,"type":"capsule","from":{"x":from.x,"y":from.y},"to":{"x":to.x,"y":to.y},"width":width,"arcSegments":arc_segments}
            }))
        }
        Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            segments,
            ..
        } if [center.x, center.y, *radius].into_iter().all(finite)
            && start_angle.is_finite()
            && sweep.is_finite()
            && *radius > 0.0
            && sweep.abs() >= 1e-9
            && sweep.abs() < std::f64::consts::TAU =>
        {
            Ok(json!({
                "id":graph_id,"type":"sketchArc","sketchId":sketch_id,
                "arc":{"id":graph_id,"type":"arc","center":{"x":center.x,"y":center.y},"radius":radius,"startAngle":start_angle,"sweep":sweep,"segments":segments}
            }))
        }
        _ => Err("cannot save invalid sketch geometry".into()),
    }
}

fn update_shape(mut original: Value, shape: &Shape, graph_id: &str) -> Result<Value, String> {
    let sketch_id = original
        .get("sketchId")
        .and_then(Value::as_str)
        .ok_or("original sketch entity has no sketchId")?
        .to_string();
    // Run the same geometry validation as a newly authored entity.
    write_shape(shape, graph_id, &sketch_id)?;
    original["id"] = json!(graph_id);
    match (
        original.get("type").and_then(Value::as_str),
        original
            .pointer("/primitive/type")
            .or_else(|| original.pointer("/arc/type"))
            .or_else(|| original.pointer("/spline/type"))
            .and_then(Value::as_str),
        shape,
    ) {
        (Some("sketchLine"), _, Shape::Line { a, b, .. }) => {
            original["p0"] = json!({"x":a.x,"y":a.y});
            original["p1"] = json!({"x":b.x,"y":b.y});
        }
        (Some("sketchCornerRectangle"), _, Shape::Rectangle { a, b, .. }) => {
            original["p0"] = json!({"x":a.x,"y":a.y});
            original["p1"] = json!({"x":b.x,"y":b.y});
        }
        (Some("sketchCornerRectangle"), _, Shape::RotatedRectangle { .. }) => {
            let mut converted = write_shape(shape, graph_id, &sketch_id)?;
            if let Some(construction) = original.get("construction") {
                converted["primitive"]["construction"] = construction.clone();
            }
            return Ok(converted);
        }
        (
            Some("sketchPrimitive"),
            Some("circle"),
            Shape::Circle {
                center,
                radius,
                segments,
                ..
            },
        ) => {
            original["primitive"]["id"] = json!(graph_id);
            original["primitive"]["center"] = json!({"x":center.x,"y":center.y});
            original["primitive"]["radius"] = json!(radius);
            original["primitive"]["segments"] = json!(segments);
        }
        (
            Some("sketchPrimitive"),
            Some("ellipse"),
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                segments,
                ..
            },
        ) => {
            original["primitive"]["id"] = json!(graph_id);
            original["primitive"]["center"] = json!({"x":center.x,"y":center.y});
            original["primitive"]["width"] = json!(rx * 2.0);
            original["primitive"]["height"] = json!(ry * 2.0);
            original["primitive"]["rotation"] = json!(rotation);
            original["primitive"]["segments"] = json!(segments);
        }
        (
            Some("sketchPrimitive"),
            Some("ellipticArc"),
            Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                segments,
                ..
            },
        ) => {
            original["primitive"]["id"] = json!(graph_id);
            original["primitive"]["center"] = json!({"x":center.x,"y":center.y});
            original["primitive"]["width"] = json!(rx * 2.0);
            original["primitive"]["height"] = json!(ry * 2.0);
            original["primitive"]["rotation"] = json!(rotation);
            original["primitive"]["startAngle"] = json!(start_angle);
            original["primitive"]["sweep"] = json!(sweep);
            original["primitive"]["segments"] = json!(segments);
        }
        (Some("sketchPrimitive"), Some("point"), Shape::Point { at, .. }) => {
            original["primitive"]["id"] = json!(graph_id);
            original["primitive"]["center"] = json!({"x":at.x,"y":at.y});
        }
        (
            Some("sketchPrimitive"),
            Some("polygon"),
            Shape::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            },
        ) => {
            original["primitive"]["id"] = json!(graph_id);
            original["primitive"]["center"] = json!({"x":center.x,"y":center.y});
            original["primitive"]["radius"] = json!(radius);
            original["primitive"]["sides"] = json!(sides);
            original["primitive"]["rotation"] = json!(rotation);
        }
        (
            Some("sketchPrimitive"),
            Some("capsule"),
            Shape::Capsule {
                from,
                to,
                width,
                arc_segments,
                ..
            },
        ) => {
            original["primitive"]["id"] = json!(graph_id);
            original["primitive"]["from"] = json!({"x":from.x,"y":from.y});
            original["primitive"]["to"] = json!({"x":to.x,"y":to.y});
            original["primitive"]["width"] = json!(width);
            original["primitive"]["arcSegments"] = json!(arc_segments);
        }
        (Some("sketchPrimitive"), Some("rectangle"), Shape::Rectangle { a, b, .. }) => {
            original["primitive"]["id"] = json!(graph_id);
            original["primitive"]["center"] = json!({"x":(a.x+b.x)/2.0,"y":(a.y+b.y)/2.0});
            original["primitive"]["width"] = json!((a.x - b.x).abs());
            original["primitive"]["height"] = json!((a.y - b.y).abs());
        }
        (
            Some("sketchSpline"),
            Some("spline"),
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            },
        ) => {
            original["spline"]["id"] = json!(graph_id);
            original["spline"]["mode"] = json!(match mode {
                SplineMode::Fit => "fit",
                SplineMode::Control => "control",
            });
            original["spline"]["points"] = json!(
                points
                    .iter()
                    .map(|point| json!({"x":point.x,"y":point.y}))
                    .collect::<Vec<_>>()
            );
            original["spline"]["closed"] = json!(closed);
        }
        (
            Some("sketchPrimitive"),
            Some("rectangle"),
            Shape::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            },
        ) => {
            original["primitive"]["id"] = json!(graph_id);
            original["primitive"]["center"] = json!({"x":center.x,"y":center.y});
            original["primitive"]["width"] = json!(width);
            original["primitive"]["height"] = json!(height);
            original["primitive"]["rotation"] = json!(rotation);
        }
        (
            Some("sketchArc"),
            Some("arc"),
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                segments,
                ..
            },
        ) => {
            original["arc"]["id"] = json!(graph_id);
            original["arc"]["center"] = json!({"x":center.x,"y":center.y});
            original["arc"]["radius"] = json!(radius);
            original["arc"]["startAngle"] = json!(start_angle);
            original["arc"]["sweep"] = json!(sweep);
            original["arc"]["segments"] = json!(segments);
        }
        _ => return Err("original sketch entity type no longer matches its geometry".into()),
    }
    Ok(original)
}

fn point(value: &Value) -> Option<Point> {
    let x = value.get("x")?.as_f64()? as f32;
    let y = value.get("y")?.as_f64()? as f32;
    (x.is_finite() && y.is_finite()).then_some(Point { x, y })
}

fn valid_name(name: &str) -> Result<&str, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 80 || trimmed.chars().any(char::is_control) {
        return Err("name must contain 1 to 80 visible characters".into());
    }
    Ok(trimmed)
}

fn positive(value: &Value) -> Option<f32> {
    let number = value.as_f64()? as f32;
    (number.is_finite() && number > 0.0).then_some(number)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock: {error}"))?
        .as_nanos();
    let temporary = path.with_extension(format!("pcad.{}.{}.tmp", std::process::id(), stamp));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("create temporary project: {error}"))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("write temporary project: {error}"))?;
        fs::rename(&temporary, path).map_err(|error| format!("replace project: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "puppycad-{name}-{}-{}.pcad",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn new_project_saves_and_reopens_native_sketch_entities() {
        let path = test_path("roundtrip");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 10.0, y: 20.0 });
        assert!(document.finish_draft(Point { x: 40.0, y: 20.0 }));
        project.save(&document).unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["version"], 4);
        assert_eq!(value["revision"], 1);
        assert_eq!(
            value["items"][0]["data"]["cad"]["nodes"][4]["primitive"]["type"],
            "circle"
        );
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(loaded.shapes, document.shapes);
        assert_eq!(reopened.read_only_reason(), None);
        loaded.selected = Some(1);
        assert!(loaded.delete_selected());
        reopened.save(&loaded).unwrap();
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_spline_nodes_round_trip_and_closed_fit_has_profile() {
        let path = test_path("spline-roundtrip");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let fit = Shape::Spline {
            id: 1,
            mode: SplineMode::Fit,
            closed: true,
            points: vec![
                Point { x: -20.0, y: -20.0 },
                Point { x: 20.0, y: -20.0 },
                Point { x: 20.0, y: 20.0 },
                Point { x: -20.0, y: 20.0 },
            ],
        };
        let control = Shape::Spline {
            id: 2,
            mode: SplineMode::Control,
            closed: false,
            points: vec![
                Point { x: 40.0, y: 0.0 },
                Point { x: 50.0, y: -20.0 },
                Point { x: 60.0, y: 20.0 },
                Point { x: 70.0, y: 0.0 },
            ],
        };
        let mut document = Document::from_shapes(vec![fit.clone(), control.clone()]);
        document.construction.insert(2);
        project.save(&document).unwrap();
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes, vec![fit, control]);
        assert!(loaded.construction.contains(&2));
        assert!(profiles_from_document(&loaded).is_some());
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node["type"] == "sketchSpline")
                .count(),
            2
        );
        assert_eq!(
            nodes
                .iter()
                .find(|node| node["type"] == "sketchSpline" && node["spline"]["mode"] == "fit")
                .unwrap()["spline"]["closed"],
            true
        );
        let mut moved = loaded.clone();
        moved.selected = Some(2);
        assert!(moved.move_selected(Point { x: 5.0, y: 3.0 }));
        reopened.save(&moved).unwrap();
        let (mut moved_project, mut moved_loaded) = ProjectFile::open(&path).unwrap();
        assert!(
            matches!(&moved_loaded.shapes[1], Shape::Spline { points, .. }
            if points[0] == Point { x: 45.0, y: 3.0 })
        );
        moved_loaded.selected = Some(2);
        moved_loaded.select_at(Point { x: 55.0, y: -17.0 });
        assert!(moved_loaded.begin_drag(Point { x: 55.0, y: -17.0 }));
        assert!(moved_loaded.drag_to(Point { x: 60.0, y: -12.0 }));
        assert!(moved_loaded.finish_drag());
        moved_project.save(&moved_loaded).unwrap();
        let (_, reloaded) = ProjectFile::open(&path).unwrap();
        assert!(matches!(&reloaded.shapes[1], Shape::Spline { points, .. }
            if points[0] == Point { x: 45.0, y: 3.0 } && points[1] == Point { x: 60.0, y: -12.0 }));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn numeric_spline_point_coordinates_survive_project_reopen() {
        let path = test_path("spline-point-coordinates");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            closed: false,
            points: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 20.0 },
                Point { x: 20.0, y: -20.0 },
                Point { x: 30.0, y: 0.0 },
            ],
        }]);
        document.selected = Some(1);
        document.selected_spline_point = Some(1);
        document.set_selected_spline_coordinate(true, 15.0).unwrap();
        document
            .set_selected_spline_coordinate(false, 25.0)
            .unwrap();
        project.save(&document).unwrap();
        let (_, loaded) = ProjectFile::open(&path).unwrap();
        assert!(matches!(&loaded.shapes[0], Shape::Spline { points, .. }
            if points[1] == Point { x: 15.0, y: 25.0 }));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn fixed_point_and_spline_anchor_relations_open_save_and_unlock() {
        let path = test_path("fixed-anchors");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Point {
                id: 1,
                at: Point { x: 5.0, y: 7.0 },
            },
            Shape::Spline {
                id: 2,
                mode: SplineMode::Control,
                closed: false,
                points: vec![
                    Point { x: 0.0, y: 0.0 },
                    Point { x: 10.0, y: 20.0 },
                    Point { x: 20.0, y: -20.0 },
                    Point { x: 30.0, y: 0.0 },
                ],
            },
        ]);
        document.selected = Some(1);
        document.toggle_fixed_anchor().unwrap();
        document.selected = Some(2);
        document.selected_spline_point = Some(1);
        document.toggle_fixed_anchor().unwrap();
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.fixed_anchors.len(), 2);
        let relations = reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
            .as_array()
            .unwrap();
        assert_eq!(
            relations
                .iter()
                .filter(|relation| relation["type"] == "fixed")
                .count(),
            2
        );
        loaded.selected = Some(2);
        loaded.selected_spline_point = Some(1);
        assert!(loaded.set_selected_spline_coordinate(true, 15.0).is_err());
        let mut invalid = loaded.clone();
        if let Shape::Spline { points, .. } = &mut invalid.shapes[1] {
            points[1].x = 15.0;
        }
        assert!(reopened.save(&invalid).is_err());
        loaded.toggle_fixed_anchor().unwrap();
        reopened.save(&loaded).unwrap();
        let (mut again, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(loaded.fixed_anchors.len(), 1);
        loaded.selected = Some(1);
        assert_eq!(loaded.toggle_fixed_anchor(), Ok(false));
        again.save(&loaded).unwrap();
        let (_, final_document) = ProjectFile::open(&path).unwrap();
        assert!(final_document.fixed_anchors.is_empty());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_fixed_anchor_relation_keeps_sketch_read_only() {
        let path = test_path("invalid-fixed-anchor");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let document = Document::from_shapes(vec![Shape::Point {
            id: 1,
            at: Point { x: 5.0, y: 7.0 },
        }]);
        project.save(&document).unwrap();
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["items"][0]["data"]["cad"]["nodes"][3]["relations"] = json!([{
            "id":"web-fixed","type":"fixed",
            "anchor":{"entityId":"native-entity-1","point":"center"},
            "position":{"x":100.0,"y":7.0},"fixation":"native-entity-1"
        }]);
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let (mut loaded, document) = ProjectFile::open(&path).unwrap();
        assert!(loaded.read_only_reason().is_some());
        assert!(loaded.save(&document).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn split_spline_pieces_save_and_reopen_as_editable_web_nodes() {
        let path = test_path("split-spline-roundtrip");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 20.0 },
            Point { x: 20.0, y: -20.0 },
            Point { x: 30.0, y: 0.0 },
        ];
        let mut document = Document::from_shapes(vec![Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: points.clone(),
            closed: false,
        }]);
        document.selected = Some(1);
        document.construction.insert(1);
        let middle = spline::point(
            &spline::beziers(SplineMode::Control, &points, false).unwrap()[0],
            0.5,
        );
        document.split_selected_curve(middle).unwrap();
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(loaded.shapes, document.shapes);
        assert_eq!(loaded.construction, document.construction);
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|node| node["type"] == "sketchSpline")
                .count(),
            2
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn trimmed_spline_pieces_reopen_as_control_nodes() {
        let path = test_path("trim-spline-roundtrip");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Spline {
                id: 1,
                mode: SplineMode::Control,
                points: vec![
                    Point { x: -20.0, y: 0.0 },
                    Point { x: -20.0, y: 20.0 },
                    Point { x: 20.0, y: 20.0 },
                    Point { x: 20.0, y: 0.0 },
                ],
                closed: false,
            },
            Shape::Line {
                id: 2,
                a: Point { x: -10.0, y: -5.0 },
                b: Point { x: -10.0, y: 25.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: 10.0, y: -5.0 },
                b: Point { x: 10.0, y: 25.0 },
            },
        ]);
        document.selected = Some(1);
        document
            .trim_selected_curve(Point { x: 0.0, y: 15.0 })
            .unwrap();
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(loaded.shapes.len(), document.shapes.len());
        for (saved, original) in loaded.shapes.iter().zip(document.shapes.iter()) {
            match (saved, original) {
                (
                    Shape::Spline {
                        mode: saved_mode,
                        points: saved_points,
                        closed: saved_closed,
                        ..
                    },
                    Shape::Spline {
                        mode: original_mode,
                        points: original_points,
                        closed: original_closed,
                        ..
                    },
                ) => {
                    assert_eq!(
                        (saved_mode, saved_points, saved_closed),
                        (original_mode, original_points, original_closed)
                    );
                }
                (
                    Shape::Line {
                        a: saved_a,
                        b: saved_b,
                        ..
                    },
                    Shape::Line {
                        a: original_a,
                        b: original_b,
                        ..
                    },
                ) => {
                    assert_eq!((saved_a, saved_b), (original_a, original_b));
                }
                _ => panic!("shape changed type"),
            }
        }
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|node| node["type"] == "sketchSpline")
                .count(),
            2
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn preserves_other_documents_and_refuses_unsupported_feature_edits() {
        let path = test_path("readonly");
        let (mut project, document) = ProjectFile::open(&path).unwrap();
        project.value["items"].as_array_mut().unwrap().push(json!({
            "id":"other","type":"diagram","name":"Other"
        }));
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.value["items"][1]["id"], "other");
        let mut unsafe_value = reopened.value.clone();
        unsafe_value["items"][0]["data"]["features"] = json!([{"type":"extrude"}]);
        let (mut read_only, loaded) =
            ProjectFile::from_value(path.clone(), unsafe_value, None).unwrap();
        assert!(read_only.read_only_reason().is_some());
        assert!(read_only.save(&loaded).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn opens_selected_top_level_part_and_saves_only_that_part() {
        let path = test_path("selected-part");
        let (project, _) = ProjectFile::open(&path).unwrap();
        let first = project.value["items"][0].clone();
        let mut second = first.clone();
        second["id"] = json!("native-part-2");
        second["name"] = json!("Second part");
        let mut value = project.value;
        value["items"].as_array_mut().unwrap().push(second);
        value["selectedPath"] = json!([1]);
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let (mut reopened, mut document) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.part_index, 1);
        assert_eq!(reopened.part_name(), "Second part");
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 10.0, y: 0.0 }));
        reopened.save(&document).unwrap();
        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["items"][0], first);
        assert_eq!(saved["selectedPath"], json!([1]));
        assert!(
            saved["items"][1]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|node| node["type"] == "sketchPrimitive")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn lists_parent_folders_then_pcad_projects() {
        let fixtures = fs::canonicalize("fixtures").unwrap();
        let projects = list_projects(&fixtures).unwrap();
        assert_eq!(projects[0].kind, EntryKind::Parent);
        assert_eq!(projects[0].path, fixtures.parent().unwrap());
        let names = projects[1..]
            .iter()
            .inspect(|project| assert_eq!(project.kind, EntryKind::Project))
            .map(|project| project.name.as_str())
            .collect::<Vec<_>>();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        let two_parts = projects
            .iter()
            .find(|project| project.name == "two-parts")
            .unwrap();
        assert_eq!(two_parts.parts, 2);
        assert!(two_parts.error.is_none());

        let dir = std::env::temp_dir().join(format!("puppycad-list-{}", std::process::id()));
        fs::create_dir_all(dir.join("Designs")).unwrap();
        fs::create_dir_all(dir.join(".hidden")).unwrap();
        fs::write(dir.join("broken.pcad"), b"not json").unwrap();
        fs::write(dir.join("notes.txt"), b"ignored").unwrap();
        let listed = list_projects(&dir).unwrap();
        let kinds = listed.iter().map(|entry| entry.kind).collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [EntryKind::Parent, EntryKind::Folder, EntryKind::Project]
        );
        assert_eq!(listed[1].name, "Designs");
        assert!(listed[2].error.is_some());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unsaved_changes_track_edits_and_clear_on_save() {
        for entry in fs::read_dir("fixtures").unwrap() {
            let fixture = entry.unwrap().path();
            let (project, document) = ProjectFile::open(&fixture).unwrap();
            assert!(
                !project.has_unsaved_changes(&document),
                "{} opens dirty",
                fixture.display()
            );
        }

        let path = test_path("unsaved-changes");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        assert!(!project.has_unsaved_changes(&document));
        document.tool = crate::model::Tool::Rectangle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 20.0, y: 10.0 }));
        assert!(project.has_unsaved_changes(&document));
        project.save(&document).unwrap();
        assert!(!project.has_unsaved_changes(&document));
        project.rename_part("Renamed").unwrap();
        assert!(project.has_unsaved_changes(&document));
        project.save(&document).unwrap();
        assert!(!project.has_unsaved_changes(&document));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn switching_parts_retains_unsaved_sketches_and_saves_both() {
        let path = test_path("switch-parts");
        let (mut project, mut first) = ProjectFile::open(&path).unwrap();
        let mut second_item = project.value["items"][0].clone();
        second_item["id"] = json!("native-part-2");
        second_item["name"] = json!("Second part");
        project.value["items"]
            .as_array_mut()
            .unwrap()
            .push(second_item);
        first.tool = crate::model::Tool::Rectangle;
        first.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(first.finish_draft(Point { x: 20.0, y: 10.0 }));
        let mut second = project.switch_part(1, &first).unwrap();
        second.tool = crate::model::Tool::Circle;
        second.draft_start = Some(Point { x: 50.0, y: 0.0 });
        assert!(second.finish_draft(Point { x: 60.0, y: 0.0 }));
        let first_again = project.switch_part(0, &second).unwrap();
        assert!(matches!(first_again.shapes[0], Shape::Rectangle { .. }));
        assert!(matches!(
            project.switch_part(1, &first_again).unwrap().shapes[0],
            Shape::Circle { .. }
        ));
        project.save(&second).unwrap();
        let (mut reopened, loaded_second) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.active_part_index(), 1);
        assert!(matches!(loaded_second.shapes[0], Shape::Circle { .. }));
        let loaded_first = reopened.switch_part(0, &loaded_second).unwrap();
        assert!(matches!(loaded_first.shapes[0], Shape::Rectangle { .. }));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_external_changes_before_overwriting() {
        let path = test_path("external-change");
        let (mut project, document) = ProjectFile::open(&path).unwrap();
        fs::write(&path, b"external contents").unwrap();
        assert!(
            project
                .save(&document)
                .unwrap_err()
                .contains("changed on disk")
        );
        assert_eq!(fs::read(&path).unwrap(), b"external contents");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn line_round_trips_as_web_sketch_line() {
        let path = test_path("line-roundtrip");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: -12.0, y: 4.0 });
        assert!(document.finish_draft(Point { x: 30.0, y: 18.0 }));
        project.save(&document).unwrap();
        let (_, reopened) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.shapes, document.shapes);
        assert_eq!(
            project.value["items"][0]["data"]["cad"]["nodes"][4]["type"],
            "sketchLine"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn three_point_circle_saves_as_web_circle_primitive() {
        let path = test_path("three-point-circle");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::ThreePointCircle;
        document.draft_start = Some(Point { x: 0.0, y: 20.0 });
        assert!(!document.finish_draft(Point { x: 20.0, y: 0.0 }));
        assert!(document.finish_draft(Point { x: 0.0, y: -20.0 }));
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert!(matches!(
            loaded.shapes[0],
            Shape::Circle {
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                ..
            }
        ));
        let node = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["type"] == "sketchPrimitive")
            .unwrap();
        assert_eq!(node["primitive"]["type"], "circle");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn center_arc_saves_clockwise_web_arc() {
        let path = test_path("center-arc");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::CenterArc;
        document.arc_clockwise = true;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!document.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert!(document.finish_draft(Point { x: 0.0, y: 20.0 }));
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert!(matches!(loaded.shapes[0], Shape::Arc { radius: 10.0, sweep, .. } if sweep < -4.0));
        let node = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["type"] == "sketchArc")
            .unwrap();
        assert_eq!(node["arc"]["type"], "arc");
        assert!(node["arc"]["sweep"].as_f64().unwrap() < -4.0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn split_arc_round_trips_as_two_editable_web_arcs() {
        let path = test_path("split-arc");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::CenterArc;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!document.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert!(document.finish_draft(Point { x: -10.0, y: 0.0 }));
        assert_eq!(
            document.split_selected_curve(Point { x: 0.0, y: 10.0 }),
            Ok(2)
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes.len(), 2);
        assert!(loaded.shapes.iter().all(|shape| matches!(shape, Shape::Arc { radius: 10.0, sweep, .. } if (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9)));
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node["type"] == "sketchArc")
                .count(),
            2
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn trimmed_arc_round_trips_with_curved_gap() {
        let path = test_path("trim-arc");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::CenterArc;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!document.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert!(document.finish_draft(Point { x: -10.0, y: 0.0 }));
        document.tool = crate::model::Tool::Line;
        for x in [5.0, -5.0] {
            document.draft_start = Some(Point { x, y: 0.0 });
            assert!(document.finish_draft(Point { x, y: 20.0 }));
        }
        document.selected = Some(1);
        assert_eq!(
            document.trim_selected_curve(Point { x: 0.0, y: 10.0 }),
            Ok(2)
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded
                .shapes
                .iter()
                .filter(|shape| matches!(shape, Shape::Arc { .. }))
                .count(),
            2
        );
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node["type"] == "sketchArc")
                .count(),
            2
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn split_ellipse_round_trips_as_editable_elliptic_arcs() {
        let path = test_path("split-ellipse");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        let center = Point { x: 10.0, y: 20.0 };
        document.tool = crate::model::Tool::Ellipse;
        document.draft_start = Some(center);
        assert!(!document.finish_draft(Point { x: 30.0, y: 20.0 }));
        assert!(document.finish_draft(Point { x: 10.0, y: 30.0 }));
        document.selected = Some(1);
        assert_eq!(
            document.split_selected_ellipse(Point { x: 30.0, y: 20.0 }, Point { x: 10.0, y: 30.0 }),
            Ok(2)
        );
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes.len(), 2);
        assert_eq!(loaded.shared_ellipses.get(&2), Some(&1));
        assert!(loaded.shapes.iter().all(|shape| matches!(
            shape,
            Shape::EllipticArc {
                rx: 20.0,
                ry: 10.0,
                ..
            }
        )));
        assert!(profiles_from_document(&loaded).is_some());
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node["primitive"]["type"] == "ellipticArc")
                .count(),
            2
        );
        assert_eq!(
            nodes.iter().find(|node| node["type"] == "sketch").unwrap()["relations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|relation| relation["type"] == "sameEllipse")
                .count(),
            1
        );
        loaded.selected = Some(2);
        assert_eq!(
            loaded.set_dimension(DimensionField::RectangleWidth, 50.0),
            Ok(())
        );
        assert_eq!(loaded.set_dimension(DimensionField::Rotation, 30.0), Ok(()));
        assert!(loaded.move_selected(Point { x: 5.0, y: -4.0 }));
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.shared_ellipses.get(&2), Some(&1));
        assert!(final_document.shapes.iter().all(|shape| matches!(shape, Shape::EllipticArc { center: Point { x, y }, rx, rotation, .. } if (x - 15.0).abs() < 1e-4 && (y - 16.0).abs() < 1e-4 && (rx - 25.0).abs() < 1e-4 && (rotation - 30.0).abs() < 1e-4)));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn trimmed_ellipse_round_trips_as_open_elliptic_arc() {
        let path = test_path("trim-ellipse");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Ellipse;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!document.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert!(document.finish_draft(Point { x: 0.0, y: 5.0 }));
        document.tool = crate::model::Tool::Line;
        for x in [5.0, -5.0] {
            document.draft_start = Some(Point { x, y: -10.0 });
            assert!(document.finish_draft(Point { x, y: 10.0 }));
        }
        document.selected = Some(1);
        assert_eq!(
            document.trim_selected_curve(Point { x: 0.0, y: 5.0 }),
            Ok(1)
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(
            matches!(loaded.shapes[0], Shape::EllipticArc { rx: 10.0, ry: 5.0, sweep, .. }
            if (sweep - 5.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
        );
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node["primitive"]["type"] == "ellipticArc")
                .count(),
            1
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn extended_elliptic_arc_round_trips_to_next_boundary() {
        let path = test_path("extend-elliptic-arc");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Ellipse;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!document.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert!(document.finish_draft(Point { x: 0.0, y: 5.0 }));
        document.selected = Some(1);
        assert_eq!(
            document.split_selected_ellipse(Point { x: 10.0, y: 0.0 }, Point { x: 0.0, y: 5.0 }),
            Ok(2)
        );
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: -5.0, y: -10.0 });
        assert!(document.finish_draft(Point { x: -5.0, y: 10.0 }));
        document.selected = Some(1);
        assert!(
            document
                .extend_selected_curve(Point { x: 0.0, y: 5.0 })
                .is_ok()
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(matches!(loaded.shapes[0], Shape::EllipticArc { sweep, .. }
            if (sweep - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn trimmed_circle_round_trips_as_editable_web_arc() {
        let path = test_path("trim-circle");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 10.0, y: 0.0 }));
        document.tool = crate::model::Tool::Line;
        for x in [5.0, -5.0] {
            document.draft_start = Some(Point { x, y: -20.0 });
            assert!(document.finish_draft(Point { x, y: 20.0 }));
        }
        document.selected = Some(1);
        assert_eq!(
            document.trim_selected_curve(Point { x: 0.0, y: 10.0 }),
            Ok(1)
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(
            matches!(loaded.shapes[0], Shape::Arc { sweep, .. } if (sweep - 5.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
        );
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node["type"] == "sketchArc")
                .count(),
            1
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn split_circle_round_trips_as_two_web_arcs() {
        let path = test_path("split-circle");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert_eq!(
            document.split_selected_circle(Point { x: 10.0, y: 0.0 }, Point { x: 0.0, y: 10.0 }),
            Ok(2)
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes.len(), 2);
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node["type"] == "sketchArc")
                .count(),
            2
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn extended_arc_round_trips_to_next_boundary() {
        let path = test_path("extend-arc");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::CenterArc;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!document.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert!(document.finish_draft(Point { x: 0.0, y: 10.0 }));
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: -5.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: -5.0, y: 20.0 }));
        document.selected = Some(1);
        document
            .extend_selected_curve(Point { x: 0.0, y: 10.0 })
            .unwrap();
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(
            matches!(loaded.shapes[0], Shape::Arc { sweep, .. } if (sweep - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6)
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn web_graph_dimension_constraint_edits_and_deletes_without_duplication() {
        let path = test_path("graph-constraint");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 30.0, y: 40.0 }));
        project.save(&document).unwrap();
        let mut value = project.value;
        value["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id":"graph-length","type":"sketchConstraint","sketchId":"native-sketch-1",
                "constraint":{"type":"lineLength","entityId":"native-entity-1","value":50}
            }));
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded
                .driving_dimensions
                .get(&(1, DimensionField::LineLength)),
            Some(&50.0)
        );
        loaded.selected = Some(1);
        loaded
            .set_dimension(DimensionField::LineLength, 60.0)
            .unwrap();
        reopened.save(&loaded).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        let constraint = nodes
            .iter()
            .find(|node| node["id"] == "graph-length")
            .unwrap();
        assert_eq!(constraint["constraint"]["value"], 60.0);
        assert!(
            reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        loaded.selected = Some(1);
        assert!(loaded.delete_selected());
        reopened.save(&loaded).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert!(
            reopened.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|node| node["id"] != "graph-length")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn web_graph_axis_constraint_opens_and_can_be_replaced() {
        let path = test_path("graph-axis-constraint");
        fs::write(
            &path,
            include_str!("../fixtures/graph-constrained-line.pcad"),
        )
        .unwrap();
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        assert_eq!(project.read_only_reason(), None);
        assert_eq!(
            document.axis_constraints.get(&1),
            Some(&AxisConstraint::Horizontal)
        );
        document.selected = Some(1);
        assert_eq!(
            document.toggle_axis_constraint(AxisConstraint::Vertical),
            Ok(true)
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded.axis_constraints.get(&1),
            Some(&AxisConstraint::Vertical)
        );
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert!(nodes.iter().all(|node| node["id"] != "horizontal-1"));
        assert_eq!(
            nodes.iter().find(|node| node["id"] == "length-1").unwrap()["constraint"]["value"],
            100.0
        );
        assert!(
            nodes[3]["relations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|relation| relation["type"] == "vertical")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn web_graph_rectangle_dimensions_remain_graph_backed() {
        let path = test_path("graph-rectangle-dimensions");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Rectangle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 40.0, y: 20.0 }));
        project.save(&document).unwrap();
        let mut value = project.value;
        let nodes = value["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap();
        for (id, kind, size) in [
            ("graph-width", "rectangleWidth", 40),
            ("graph-height", "rectangleHeight", 20),
        ] {
            nodes.push(
                json!({"id":id,"type":"sketchConstraint","sketchId":"native-sketch-1",
                "constraint":{"type":kind,"entityId":"native-entity-1","value":size}}),
            );
        }
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let (mut project, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(project.read_only_reason(), None);
        loaded.selected = Some(1);
        loaded
            .set_dimension(DimensionField::RectangleWidth, 50.0)
            .unwrap();
        project.save(&loaded).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .find(|node| node["id"] == "graph-width")
                .unwrap()["constraint"]["value"],
            50.0
        );
        assert_eq!(
            nodes
                .iter()
                .find(|node| node["id"] == "graph-height")
                .unwrap()["constraint"]["value"],
            20.0
        );
        assert!(nodes[3]["relations"].as_array().unwrap().is_empty());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn horizontal_and_vertical_relations_round_trip_with_copies() {
        let path = test_path("axis-relations");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 30.0, y: 20.0 }));
        assert_eq!(
            document.toggle_axis_constraint(AxisConstraint::Horizontal),
            Ok(true)
        );
        assert!(document.copy_selected(Point { x: 50.0, y: 0.0 }));
        assert_eq!(
            document.toggle_axis_constraint(AxisConstraint::Vertical),
            Ok(true)
        );
        project.save(&document).unwrap();
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded.axis_constraints.get(&1),
            Some(&AxisConstraint::Horizontal)
        );
        assert_eq!(
            loaded.axis_constraints.get(&2),
            Some(&AxisConstraint::Vertical)
        );
        let relations = reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
            .as_array()
            .unwrap();
        assert_eq!(relations.len(), 2);
        assert!(relations.iter().any(|r| r["type"] == "horizontal"));
        assert!(relations.iter().any(|r| r["type"] == "vertical"));
        let mut loaded = loaded;
        loaded.selected = Some(2);
        assert!(loaded.delete_selected());
        reopened.save(&loaded).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn tangent_arc_and_smooth_join_round_trip_as_web_relations() {
        let path = test_path("tangent-arc");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 40.0, y: 0.0 }));
        let arc = document
            .add_tangent_arc((1, 1), Point { x: 60.0, y: 20.0 })
            .unwrap();
        let spline = document
            .add_spline(
                crate::spline::SplineMode::Fit,
                vec![
                    Point { x: 61.0, y: 21.0 },
                    Point { x: 70.0, y: 40.0 },
                    Point { x: 50.0, y: 60.0 },
                ],
                false,
            )
            .unwrap();
        document.selected = Some(arc);
        assert!(document.toggle_smooth_join_with(spline).unwrap());
        assert_eq!(document.smooth_joins.len(), 2);
        project.save(&document).unwrap();
        if let Ok(target) = std::env::var("PUPPYCAD_WRITE_TANGENT_FIXTURE") {
            fs::copy(&path, target).unwrap();
        }
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.smooth_joins, document.smooth_joins);
        let relations = |project: &ProjectFile| {
            project.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["type"] == "sketch")
                .unwrap()["relations"]
                .as_array()
                .unwrap()
                .clone()
        };
        let saved = relations(&reopened);
        let joins: Vec<_> = saved.iter().filter(|r| r["type"] == "smoothJoin").collect();
        assert_eq!(joins.len(), 2);
        assert!(
            joins
                .iter()
                .any(|r| r["a"]["point"] == "p1" && r["b"]["point"] == "p0")
        );
        assert!(joins.iter().any(|r| r["b"]["point"] == "point0"));
        // Saving again keeps the same relations (ids stable).
        reopened.save(&loaded).unwrap();
        assert_eq!(relations(&reopened), saved);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn tangent_arc_fixture_loads_smooth_joins() {
        let (project, document) =
            ProjectFile::open(Path::new("fixtures/tangent-arc.pcad")).unwrap();
        assert_eq!(project.read_only_reason(), None);
        assert_eq!(document.smooth_joins.len(), 2);
    }

    #[test]
    fn fillet_and_chamfer_round_trip_as_web_relations() {
        let path = test_path("fillet-chamfer");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        for (from, to) in [
            ((40.0, 0.0), (0.0, 0.0)),
            ((0.0, 0.0), (0.0, 40.0)),
            ((100.0, 0.0), (60.0, 0.0)),
            ((60.0, 0.0), (60.0, 40.0)),
        ] {
            document.draft_start = Some(Point {
                x: from.0,
                y: from.1,
            });
            assert!(document.finish_draft(Point { x: to.0, y: to.1 }));
        }
        let arc = document.fillet_lines(1, 2, 5.0).unwrap();
        let chamfer = document
            .chamfer_lines(3, 4, ChamferMode::DistanceAngle, 4.0, 45.0)
            .unwrap();
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes.len(), 6);
        assert_eq!(loaded.endpoint_joins, document.endpoint_joins);
        assert_eq!(loaded.endpoint_tangents, document.endpoint_tangents);
        assert_eq!(loaded.chamfers, document.chamfers);
        assert_eq!(
            loaded
                .driving_dimensions
                .get(&(arc, DimensionField::CurveRadius)),
            Some(&5.0)
        );
        let relations = |project: &ProjectFile| {
            project.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["type"] == "sketch")
                .unwrap()["relations"]
                .as_array()
                .unwrap()
                .clone()
        };
        let saved = relations(&reopened);
        let count = |kind: &str| saved.iter().filter(|r| r["type"] == kind).count();
        assert_eq!(count("coincident"), 4);
        assert_eq!(count("endpointTangent"), 2);
        assert_eq!(count("chamfer"), 1);
        assert_eq!(count("radius"), 1);
        let chamfer_relation = saved.iter().find(|r| r["type"] == "chamfer").unwrap();
        assert_eq!(chamfer_relation["mode"], "distance-angle");
        assert_eq!(chamfer_relation["secondValue"], 45.0);
        assert!(saved
            .iter()
            .any(|r| r["type"] == "endpointTangent" && r["orientation"]["lineSign"].is_number()));
        // Edit the driving radius and chamfer after reopening; ids stay put.
        loaded.selected = Some(arc);
        loaded
            .set_dimension(DimensionField::CurveRadius, 7.0)
            .unwrap();
        loaded.selected = Some(chamfer);
        loaded
            .set_chamfer(ChamferMode::TwoDistances, 3.0, 6.0)
            .unwrap();
        reopened.save(&loaded).unwrap();
        let (again, geometry) = ProjectFile::open(&path).unwrap();
        assert_eq!(again.read_only_reason(), None);
        assert_eq!(
            geometry.chamfers.get(&chamfer).map(|spec| spec.mode),
            Some(ChamferMode::TwoDistances)
        );
        let ids = |list: &[Value]| {
            let mut ids: Vec<String> = list
                .iter()
                .filter(|r| r["type"] != "radius")
                .map(|r| r["id"].as_str().unwrap().to_string())
                .collect();
            ids.sort();
            ids
        };
        assert_eq!(ids(&relations(&again)), ids(&saved));
        assert!(matches!(
            geometry.shapes.iter().find(|shape| shape.id() == arc),
            Some(Shape::Arc { radius, .. }) if (radius - 7.0).abs() < 1e-3
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn snapped_point_line_coincidence_round_trips_and_follows_point_move() {
        let path = test_path("point-line-coincidence");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        let at = Point { x: 10.0, y: 20.0 };
        assert!(document.add_point(at));
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(at);
        assert!(document.finish_draft(Point { x: 50.0, y: 20.0 }));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.point_links.get(&(2, 0)), Some(&1));
        let relation =
            reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0].clone();
        assert_eq!(relation["type"], "coincident");
        assert_eq!(relation["a"]["point"], "p0");
        assert_eq!(relation["b"]["point"], "center");
        loaded.selected = Some(1);
        assert!(loaded.move_selected(Point { x: 5.0, y: 7.0 }));
        reopened.save(&loaded).unwrap();
        let (mut moved, mut geometry) = ProjectFile::open(&path).unwrap();
        assert_eq!(moved.read_only_reason(), None);
        assert!(matches!(
            geometry.shapes[1],
            Shape::Line {
                a: Point { x: 15.0, y: 27.0 },
                ..
            }
        ));
        assert_eq!(
            moved.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0]["id"],
            relation["id"]
        );
        geometry.selected = Some(1);
        assert!(geometry.delete_selected());
        moved.save(&geometry).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert!(final_document.point_links.is_empty());
        assert!(
            final_project.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn spline_endpoint_coincidence_round_trips_and_follows_point_move() {
        let path = test_path("point-spline-coincidence");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        let start = Point { x: 10.0, y: 20.0 };
        assert!(document.add_point(start));
        document
            .add_spline(
                crate::spline::SplineMode::Control,
                vec![
                    start,
                    Point { x: 20.0, y: 30.0 },
                    Point { x: 30.0, y: 30.0 },
                    Point { x: 40.0, y: 20.0 },
                ],
                false,
            )
            .unwrap();
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.point_links.get(&(2, 0)), Some(&1));
        let relation = &reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0];
        assert_eq!(relation["a"]["point"], "point0");
        assert_eq!(relation["b"]["point"], "center");
        loaded.selected = Some(1);
        assert!(loaded.move_selected(Point { x: 5.0, y: -2.0 }));
        reopened.save(&loaded).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.point_links.get(&(2, 0)), Some(&1));
        assert!(
            matches!(&loaded.shapes[1], Shape::Spline { points, .. } if points[0] == Point { x: 15.0, y: 18.0 })
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn fixed_line_endpoint_relation_round_trips_and_unlocks() {
        let path = test_path("fixed-line-endpoint");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 40.0, y: 0.0 }));
        assert_eq!(
            document.toggle_fixed_line_endpoint_at(Point { x: 40.0, y: 0.0 }),
            Ok(true)
        );
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded.fixed_anchors.get(&(1, 1)),
            Some(&Point { x: 40.0, y: 0.0 })
        );
        let relations = reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
            .as_array()
            .unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0]["type"], "fixed");
        assert_eq!(relations[0]["anchor"]["point"], "p1");
        loaded.selected = Some(1);
        assert_eq!(
            loaded.toggle_fixed_line_endpoint_at(Point { x: 40.0, y: 0.0 }),
            Ok(false)
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert!(final_document.fixed_anchors.is_empty());
        assert!(
            final_project.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn fixed_circle_center_relation_round_trips_with_radius_edit() {
        let path = test_path("fixed-circle-center");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 20.0, y: 30.0 });
        assert!(document.finish_draft(Point { x: 30.0, y: 30.0 }));
        assert_eq!(document.toggle_fixed_anchor(), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 20.0, y: 30.0 })
        );
        let relation = &reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0];
        assert_eq!(relation["type"], "fixed");
        assert_eq!(relation["anchor"]["point"], "center");
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::CircleDiameter, 40.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(
            final_document.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 20.0, y: 30.0 })
        );
        assert!(matches!(
            final_document.shapes[0],
            Shape::Circle { radius: 20.0, .. }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn fixed_arc_endpoint_relation_round_trips_and_follows_split_piece() {
        let path = test_path("fixed-arc-endpoint");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        }]);
        document.selected = Some(1);
        let end = document.fixed_anchor_position(1, 2).unwrap();
        assert_eq!(
            document.toggle_fixed_arc_anchor_at(Point { x: 0.0, y: 10.0 }),
            Ok((2, true))
        );
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.fixed_anchors.get(&(1, 2)), Some(&end));
        let relation = &reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0];
        assert_eq!(relation["anchor"]["point"], "p1");
        loaded.selected = Some(1);
        let split = Point {
            x: 10.0 / 2.0_f32.sqrt(),
            y: 10.0 / 2.0_f32.sqrt(),
        };
        assert_eq!(loaded.split_selected_arc(split), Ok(2));
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.fixed_anchors.get(&(2, 2)), Some(&end));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn ellipse_polygon_rectangle_and_slot_fixed_anchors_round_trip() {
        let path = test_path("named-fixed-anchors");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Ellipse {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 0.0,
                segments: 64,
            },
            Shape::Polygon {
                id: 2,
                center: Point { x: 100.0, y: 0.0 },
                radius: 10.0,
                sides: 5,
                rotation: 0.0,
            },
            Shape::RotatedRectangle {
                id: 3,
                center: Point { x: 150.0, y: 0.0 },
                width: 20.0,
                height: 10.0,
                rotation: 0.0,
            },
            Shape::Capsule {
                id: 4,
                from: Point { x: 200.0, y: 0.0 },
                to: Point { x: 220.0, y: 0.0 },
                width: 10.0,
                arc_segments: 16,
            },
        ]);
        for (id, cursor) in [
            (1, Point { x: 20.0, y: 0.0 }),
            (2, Point { x: 110.0, y: 0.0 }),
            (3, Point { x: 160.0, y: 5.0 }),
            (4, Point { x: 220.0, y: 0.0 }),
        ] {
            document.selected = Some(id);
            assert!(document.toggle_fixed_shape_anchor_at(cursor).unwrap().1);
        }
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.fixed_anchors.len(), 4);
        let relations = reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
            .as_array()
            .unwrap();
        let names: Vec<_> = relations
            .iter()
            .map(|relation| relation["anchor"]["point"].as_str().unwrap())
            .collect();
        for expected in ["p0", "vertex0", "to"] {
            assert!(names.contains(&expected));
        }
        assert_eq!(names.iter().filter(|name| **name == "p0").count(), 2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn fixed_corner_rectangle_relation_round_trips_and_resizes() {
        let path = test_path("fixed-corner-rectangle");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![Shape::Rectangle {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 10.0 },
        }]);
        document.selected = Some(1);
        assert_eq!(
            document.toggle_fixed_line_endpoint_at(Point { x: 0.0, y: 0.0 }),
            Ok(true)
        );
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 0.0, y: 0.0 })
        );
        let relation = &reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0];
        assert_eq!(relation["anchor"]["point"], "p0");
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::RectangleWidth, 30.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(
            final_document.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 0.0, y: 0.0 })
        );
        assert!(matches!(
            final_document.shapes[0],
            Shape::Rectangle {
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 30.0, y: 10.0 },
                ..
            }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn equal_line_relation_round_trips_and_updates_dependent_length() {
        let path = test_path("equal-lines");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 20.0 },
                b: Point { x: 0.0, y: 30.0 },
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_equal_line_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.equal_lines.get(&2), Some(&1));
        let relation = &reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0];
        assert_eq!(relation["type"], "equal");
        assert_eq!(relation["a"], "native-entity-1");
        assert_eq!(relation["b"], "native-entity-2");
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::LineLength, 30.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.equal_lines.get(&2), Some(&1));
        assert!(matches!(
            final_document.shapes[1],
            Shape::Line {
                b: Point { x: 0.0, y: 50.0 },
                ..
            }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn equal_radius_relation_round_trips() {
        let path = test_path("equal-radius");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 40.0, y: 0.0 },
                radius: 5.0,
                segments: 64,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_equal_radius_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.equal_radii.get(&2), Some(&1));
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::CircleDiameter, 30.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (_, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_document.equal_radii.get(&2), Some(&1));
        assert!(matches!(
            final_document.shapes[1],
            Shape::Circle { radius: 15.0, .. }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn arc_and_polygon_radius_dimensions_round_trip_with_equal_circle() {
        let path = test_path("curve-radius");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 32,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 50.0, y: 0.0 },
                radius: 5.0,
                segments: 64,
            },
            Shape::Polygon {
                id: 3,
                center: Point { x: 90.0, y: 0.0 },
                radius: 12.0,
                sides: 6,
                rotation: 0.0,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_equal_radius_with(1), Ok(true));
        document.selected = Some(1);
        assert_eq!(
            document.set_dimension(DimensionField::CurveRadius, 20.0),
            Ok(())
        );
        document.selected = Some(3);
        assert_eq!(
            document.set_dimension(DimensionField::CurveRadius, 24.0),
            Ok(())
        );
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.equal_radii.get(&2), Some(&1));
        assert!(matches!(
            loaded.shapes[1],
            Shape::Circle { radius: 20.0, .. }
        ));
        assert!(matches!(
            loaded.shapes[2],
            Shape::Polygon { radius: 24.0, .. }
        ));
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::CurveRadius, 30.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert!(matches!(
            final_document.shapes[1],
            Shape::Circle { radius: 30.0, .. }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn signed_sweep_and_rotation_relations_round_trip() {
        let path = test_path("angular-dimensions");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 32,
            },
            Shape::Polygon {
                id: 2,
                center: Point { x: 60.0, y: 0.0 },
                radius: 10.0,
                sides: 6,
                rotation: 0.0,
            },
            Shape::EllipticArc {
                id: 3,
                center: Point { x: 100.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 0.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 32,
            },
        ]);
        document.selected = Some(1);
        assert_eq!(
            document.set_dimension(DimensionField::ArcSweep, -90.0),
            Ok(())
        );
        document.selected = Some(2);
        assert_eq!(
            document.set_dimension(DimensionField::Rotation, -30.0),
            Ok(())
        );
        document.selected = Some(3);
        assert_eq!(
            document.set_dimension(DimensionField::ArcSweep, 120.0),
            Ok(())
        );
        assert_eq!(
            document.set_dimension(DimensionField::Rotation, 45.0),
            Ok(())
        );
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded
                .driving_dimensions
                .get(&(1, DimensionField::ArcSweep)),
            Some(&-90.0)
        );
        assert_eq!(
            loaded
                .driving_dimensions
                .get(&(2, DimensionField::Rotation)),
            Some(&-30.0)
        );
        assert_eq!(
            loaded
                .driving_dimensions
                .get(&(3, DimensionField::ArcSweep)),
            Some(&120.0)
        );
        loaded.selected = Some(2);
        assert!(loaded.rotate_selected(15.0, Point { x: 60.0, y: 0.0 }, false));
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(
            final_document
                .driving_dimensions
                .get(&(2, DimensionField::Rotation)),
            Some(&-15.0)
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn equal_ellipse_relation_round_trips_and_updates_both_axes() {
        let path = test_path("equal-ellipses");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Ellipse {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 0.0,
                segments: 64,
            },
            Shape::EllipticArc {
                id: 2,
                center: Point { x: 60.0, y: 0.0 },
                rx: 8.0,
                ry: 6.0,
                rotation: 30.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 64,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_equal_ellipse_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.equal_ellipses.get(&2), Some(&1));
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::RectangleWidth, 60.0),
            Ok(())
        );
        assert_eq!(
            loaded.set_dimension(DimensionField::RectangleHeight, 40.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.equal_ellipses.get(&2), Some(&1));
        assert!(matches!(
            final_document.shapes[1],
            Shape::EllipticArc {
                rx: 30.0,
                ry: 20.0,
                ..
            }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn elliptic_arc_can_drive_equal_ellipse_width_after_reopen() {
        let path = test_path("equal-elliptic-arc-source");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::EllipticArc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 0.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 32,
            },
            Shape::Ellipse {
                id: 2,
                center: Point { x: 60.0, y: 0.0 },
                rx: 8.0,
                ry: 6.0,
                rotation: 0.0,
                segments: 64,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_equal_ellipse_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::RectangleWidth, 70.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.equal_ellipses.get(&2), Some(&1));
        assert!(matches!(
            final_document.shapes[1],
            Shape::Ellipse {
                rx: 35.0,
                ry: 10.0,
                ..
            }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn concentric_relation_round_trips_and_moves_both_centers() {
        let path = test_path("concentric");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
            Shape::Ellipse {
                id: 2,
                center: Point { x: 30.0, y: 0.0 },
                rx: 15.0,
                ry: 5.0,
                rotation: 0.0,
                segments: 64,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_concentric_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.concentric.get(&2), Some(&1));
        loaded.selected = Some(1);
        assert!(loaded.move_selected(Point { x: 5.0, y: 8.0 }));
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.concentric.get(&2), Some(&1));
        assert_eq!(final_document.shapes[0].center(), Point { x: 5.0, y: 8.0 });
        assert_eq!(final_document.shapes[1].center(), Point { x: 5.0, y: 8.0 });
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn mirror_and_symmetric_relations_round_trip_with_the_web_format() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/mirror-symmetry.pcad");
        let path = test_path("mirror-symmetry");
        fs::copy(&fixture, &path).unwrap();
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        assert_eq!(project.read_only_reason(), None);
        assert_eq!(document.mirrors.get(&3), Some(&(2, 1)));
        assert_eq!(
            document.symmetric_points,
            vec![crate::model::SymmetricPoints {
                a: (4, 0),
                b: (5, 0),
                axis: 1
            }]
        );
        // Moving the source circle moves its mirror image; moving the point
        // moves the symmetric line end.
        document.selected = Some(2);
        assert!(document.move_selected(Point { x: 5.0, y: 0.0 }));
        document.selected = Some(4);
        assert!(document.move_selected(Point { x: 0.0, y: 4.0 }));
        // A mirror copy made natively is saved as a web mirror relation.
        let copy = document.mirror_shapes(&[5], 1).unwrap()[0];
        project.save(&document).unwrap();
        let saved: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let relations = saved
            .pointer("/items/0/data/cad/nodes")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .find(|node| node["type"] == "sketch")
            .unwrap()["relations"]
            .as_array()
            .unwrap()
            .clone();
        assert!(relations.iter().any(|r| r["id"] == "mirror-1"
            && r["a"] == "native-entity-2"
            && r["b"] == "native-entity-3"
            && r["symmetryLine"] == "native-entity-1"));
        assert!(relations.iter().any(|r| r["id"] == "symmetric-1"
            && r["a"] == json!({"entityId": "native-entity-4", "point": "center"})
            && r["b"] == json!({"entityId": "native-entity-5", "point": "p0"})));
        assert_eq!(
            relations.iter().filter(|r| r["type"] == "mirror").count(),
            2
        );
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.mirrors.len(), 2);
        assert_eq!(loaded.mirrors.get(&copy), Some(&(5, 1)));
        assert_eq!(loaded.symmetric_points.len(), 1);
        assert_eq!(loaded.shapes[2].center(), Point { x: -35.0, y: 10.0 });
        assert!(matches!(
            loaded.shapes[4],
            Shape::Line {
                a: Point { x: -20.0, y: -26.0 },
                ..
            }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn offset_relations_round_trip_with_the_web_format() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/offset.pcad");
        let path = test_path("offset");
        fs::copy(&fixture, &path).unwrap();
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        assert_eq!(project.read_only_reason(), None);
        assert_eq!(document.offsets.get(&6), Some(&(5, 3.0)));
        assert_eq!(document.offsets.get(&8), Some(&(7, -4.0)));
        assert_eq!(
            document.offset_chains,
            vec![crate::model::OffsetChain {
                sources: vec![(1, false), (2, false)],
                targets: vec![3, 4],
                closed: false,
                side: 1,
                distance: 5.0,
            }]
        );
        // Edit a loaded distance and add a native offset, then save.
        document.selected = Some(8);
        document.set_offset_distance(-6.0).unwrap();
        document.selected = Some(5);
        let copy = document.offset_selected(-2.0).unwrap()[0];
        project.save(&document).unwrap();
        let saved: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let relations = saved
            .pointer("/items/0/data/cad/nodes")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .find(|node| node["type"] == "sketch")
            .unwrap()["relations"]
            .as_array()
            .unwrap()
            .clone();
        assert!(relations.iter().any(|r| r["id"] == "offset-chain-1"
            && r["type"] == "offsetChain"
            && r["sources"]
                == json!([
                    {"entityId": "native-entity-1", "reversed": false},
                    {"entityId": "native-entity-2", "reversed": false}
                ])
            && r["targets"] == json!(["native-entity-3", "native-entity-4"])
            && r["closed"] == false
            && r["side"] == 1
            && r["value"] == 5.0));
        assert!(relations.iter().any(|r| r["id"] == "offset-distance-2"
            && r["a"] == "native-entity-7"
            && r["b"] == "native-entity-8"
            && r["value"] == -6.0));
        assert_eq!(
            relations.iter().filter(|r| r["type"] == "offset").count(),
            3
        );
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.offsets.len(), 3);
        assert_eq!(loaded.offsets.get(&copy), Some(&(5, -2.0)));
        assert_eq!(loaded.offsets.get(&8), Some(&(7, -6.0)));
        assert_eq!(loaded.offset_chains.len(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn natively_made_offset_chains_save_as_web_offset_chains() {
        let path = test_path("offset-chain-native");
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/offset.pcad");
        fs::copy(&fixture, &path).unwrap();
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        // Drop the loaded chain, then offset the L again natively.
        document.selected = Some(3);
        assert!(document.delete_selected());
        document.selected = Some(4);
        assert!(document.delete_selected());
        assert!(document.offset_chains.is_empty());
        document.selected = Some(2);
        let targets = document.offset_selected(-3.0).unwrap();
        project.save(&document).unwrap();
        let (_, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(loaded.offset_chains.len(), 1);
        let chain = &loaded.offset_chains[0];
        assert_eq!(chain.sources, vec![(1, false), (2, false)]);
        assert_eq!(chain.distance, -3.0);
        assert_eq!(chain.targets.len(), targets.len());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn line_direction_relations_round_trip() {
        for kind in [
            LineRelationKind::Parallel,
            LineRelationKind::Perpendicular,
            LineRelationKind::Collinear,
            LineRelationKind::Angle(45.0),
        ] {
            let path = test_path(kind.name());
            let (mut project, _) = ProjectFile::open(&path).unwrap();
            let mut document = Document::from_shapes(vec![
                Shape::Line {
                    id: 1,
                    a: Point { x: 0.0, y: 0.0 },
                    b: Point { x: 20.0, y: 0.0 },
                },
                Shape::Line {
                    id: 2,
                    a: Point { x: 0.0, y: 20.0 },
                    b: Point { x: 10.0, y: 30.0 },
                },
            ]);
            document.selected = Some(2);
            assert_eq!(document.toggle_line_relation_with(1, kind), Ok(true));
            project.save(&document).unwrap();
            let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
            assert_eq!(reopened.read_only_reason(), None);
            assert_eq!(loaded.line_relations.get(&2), Some(&(1, kind)));
            let expected_kind = if matches!(kind, LineRelationKind::Angle(_)) {
                loaded.selected = Some(2);
                assert_eq!(loaded.set_line_relation_angle(-30.0), Ok(()));
                LineRelationKind::Angle(-30.0)
            } else {
                kind
            };
            loaded.selected = Some(1);
            assert!(loaded.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
            reopened.save(&loaded).unwrap();
            let (final_project, final_document) = ProjectFile::open(&path).unwrap();
            assert_eq!(final_project.read_only_reason(), None);
            assert_eq!(
                final_document.line_relations.get(&2),
                Some(&(1, expected_kind))
            );
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn signed_radial_distance_round_trips_and_updates_target() {
        let path = test_path("radius-difference");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                segments: 64,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 60.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_radius_difference_with(1), Ok(true));
        assert_eq!(document.set_radius_difference(-5.0), Ok(()));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.radius_differences.get(&2), Some(&(1, -5.0)));
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::CircleDiameter, 60.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.radius_differences.get(&2), Some(&(1, -5.0)));
        assert!(
            matches!(final_document.shapes[1], Shape::Circle { radius, .. } if (radius - 35.0).abs() < 1e-4)
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn internal_tangent_round_trips_and_follows_outer_radius() {
        let path = test_path("internal-tangent");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                segments: 64,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 15.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_internal_tangent_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.internal_tangents.get(&2), Some(&1));
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::CircleDiameter, 50.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.internal_tangents.get(&2), Some(&1));
        assert!(
            (final_document.shapes[0]
                .center()
                .distance(final_document.shapes[1].center())
                - 15.0)
                .abs()
                < 1e-4
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn external_tangent_round_trips_and_follows_source_radius() {
        let path = test_path("external-tangent");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                segments: 64,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 40.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_external_tangent_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.external_tangents.get(&2), Some(&1));
        loaded.selected = Some(1);
        assert_eq!(
            loaded.set_dimension(DimensionField::CircleDiameter, 50.0),
            Ok(())
        );
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.external_tangents.get(&2), Some(&1));
        assert!(
            (final_document.shapes[0]
                .center()
                .distance(final_document.shapes[1].center())
                - 35.0)
                .abs()
                < 1e-4
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn line_circle_tangent_round_trips() {
        let path = test_path("line-circle-tangent");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: -20.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 0.0, y: 20.0 },
                radius: 10.0,
                segments: 64,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_external_tangent_with(1), Ok(true));
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.external_tangents.get(&2), Some(&1));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn line_ellipse_tangent_round_trips() {
        let path = test_path("line-ellipse-tangent");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: -40.0, y: 0.0 },
                b: Point { x: 40.0, y: 0.0 },
            },
            Shape::Ellipse {
                id: 2,
                center: Point { x: 0.0, y: 30.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 30.0,
                segments: 128,
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_external_tangent_with(1), Ok(true));
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.external_tangents.get(&2), Some(&1));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn midpoint_point_round_trips_and_follows_line() {
        let path = test_path("midpoint-point");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Point {
                id: 2,
                at: Point { x: 5.0, y: 5.0 },
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_midpoint_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.midpoint_links.get(&2), Some(&1));
        loaded.selected = Some(1);
        assert!(loaded.move_selected(Point { x: 0.0, y: 10.0 }));
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.midpoint_links.get(&2), Some(&1));
        assert_eq!(
            final_document.shapes[1].center(),
            Point { x: 10.0, y: 10.0 }
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn point_on_curve_round_trips_and_follows_circle() {
        let path = test_path("point-on-curve");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                segments: 64,
            },
            Shape::Point {
                id: 2,
                at: Point { x: 25.0, y: 4.0 },
            },
        ]);
        document.selected = Some(2);
        assert_eq!(document.toggle_point_on_curve_with(1), Ok(true));
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.point_curve_links.get(&2), Some(&1));
        loaded.selected = Some(1);
        assert!(loaded.move_selected(Point { x: 10.0, y: 5.0 }));
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.point_curve_links.get(&2), Some(&1));
        let point = final_document.shapes[1].center();
        assert!(
            point
                .distance(Document::closest_curve_point(&final_document.shapes[0], point).unwrap())
                < 1e-3
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn normal_line_round_trips_and_follows_circle() {
        let path = test_path("normal-line");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
            Shape::Line {
                id: 2,
                a: Point { x: 25.0, y: 4.0 },
                b: Point { x: 11.0, y: 3.0 },
            },
        ]);
        document.selected = Some(2);
        assert_eq!(
            document.toggle_normal_with(1, Point { x: 11.0, y: 3.0 }),
            Ok(true)
        );
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.normal_links.get(&2), Some(&(1, 1)));
        loaded.selected = Some(1);
        assert!(loaded.move_selected(Point { x: 5.0, y: 3.0 }));
        reopened.save(&loaded).unwrap();
        let (final_project, final_document) = ProjectFile::open(&path).unwrap();
        assert_eq!(final_project.read_only_reason(), None);
        assert_eq!(final_document.normal_links.get(&2), Some(&(1, 1)));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn split_line_round_trips_as_two_editable_web_lines() {
        let path = test_path("split-line");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 40.0, y: 0.0 }));
        assert_eq!(
            document.toggle_axis_constraint(AxisConstraint::Horizontal),
            Ok(true)
        );
        assert_eq!(
            document.split_selected_line(Point { x: 15.0, y: 0.0 }),
            Ok(2)
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes.len(), 2);
        assert_eq!(
            loaded.shapes[0],
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 15.0, y: 0.0 }
            }
        );
        assert_eq!(
            loaded.shapes[1],
            Shape::Line {
                id: 2,
                a: Point { x: 15.0, y: 0.0 },
                b: Point { x: 40.0, y: 0.0 }
            }
        );
        assert_eq!(loaded.axis_constraints.len(), 2);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn trimmed_line_round_trips_with_gap_and_boundaries() {
        let path = test_path("trim-line");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        for (a, b) in [
            (Point { x: 0.0, y: 0.0 }, Point { x: 40.0, y: 0.0 }),
            (Point { x: 10.0, y: -10.0 }, Point { x: 10.0, y: 10.0 }),
            (Point { x: 30.0, y: -10.0 }, Point { x: 30.0, y: 10.0 }),
        ] {
            document.draft_start = Some(a);
            assert!(document.finish_draft(b));
        }
        document.selected = Some(1);
        assert_eq!(
            document.trim_selected_line(Point { x: 20.0, y: 0.0 }),
            Ok(2)
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes.len(), 4);
        assert_eq!(
            loaded.shapes[0],
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 10.0, y: 0.0 }
            }
        );
        assert_eq!(
            loaded.shapes[1],
            Shape::Line {
                id: 2,
                a: Point { x: 30.0, y: 0.0 },
                b: Point { x: 40.0, y: 0.0 }
            }
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn extended_line_round_trips_to_next_boundary() {
        let path = test_path("extend-line");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        for (a, b) in [
            (Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 0.0 }),
            (Point { x: 20.0, y: -10.0 }, Point { x: 20.0, y: 10.0 }),
        ] {
            document.draft_start = Some(a);
            assert!(document.finish_draft(b));
        }
        document.selected = Some(1);
        assert_eq!(
            document.extend_selected_line(Point { x: 9.0, y: 0.0 }),
            Ok(Point { x: 20.0, y: 0.0 })
        );
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded.shapes[0],
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 }
            }
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn driving_dimension_relation_survives_reopen_update_and_delete() {
        let path = test_path("driving-dimension");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 20.0, y: 0.0 }));
        document
            .set_dimension(DimensionField::CircleDiameter, 50.0)
            .unwrap();
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(
            loaded
                .driving_dimensions
                .get(&(1, DimensionField::CircleDiameter)),
            Some(&50.0)
        );
        let relation = &reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0];
        assert_eq!(relation["type"], "diameter");
        assert_eq!(relation["value"], 50.0);
        let relation_id = relation["id"].as_str().unwrap().to_string();
        loaded.selected = Some(1);
        loaded
            .set_dimension(DimensionField::CircleDiameter, 60.0)
            .unwrap();
        reopened.save(&loaded).unwrap();
        let relations = reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
            .as_array()
            .unwrap();
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0]["id"], relation_id);
        assert_eq!(relations[0]["value"], 60.0);
        assert!(loaded.undo());
        reopened.save(&loaded).unwrap();
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0]["value"],
            50.0
        );
        assert!(loaded.redo());
        loaded.selected = Some(1);
        reopened.save(&loaded).unwrap();
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"][0]["value"],
            60.0
        );
        loaded.delete_selected();
        reopened.save(&loaded).unwrap();
        assert!(
            reopened.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn undoing_new_dimension_removes_saved_relation() {
        let path = test_path("undo-new-dimension");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 20.0, y: 0.0 }));
        document
            .set_dimension(DimensionField::CircleDiameter, 50.0)
            .unwrap();
        project.save(&document).unwrap();
        assert!(document.undo());
        project.save(&document).unwrap();
        assert!(
            project.value["items"][0]["data"]["cad"]["nodes"][3]["relations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let (_, reopened) = ProjectFile::open(&path).unwrap();
        assert!(reopened.driving_dimensions.is_empty());
        assert!(matches!(
            reopened.shapes[0],
            Shape::Circle { radius: 20.0, .. }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn copied_and_scaled_geometry_keep_driving_dimensions_after_reopen() {
        let path = test_path("copy-dimensions");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 20.0, y: 0.0 }));
        document
            .set_dimension(DimensionField::CircleDiameter, 50.0)
            .unwrap();
        assert!(document.copy_selected(Point { x: 80.0, y: 0.0 }));
        assert!(document.scale_selected(0.5, Point { x: 80.0, y: 0.0 }, true));
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes.len(), 3);
        for (id, diameter) in [(1, 50.0), (2, 50.0), (3, 25.0)] {
            assert_eq!(
                loaded
                    .driving_dimensions
                    .get(&(id, DimensionField::CircleDiameter)),
                Some(&diameter)
            );
        }
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rotating_native_corner_rectangle_saves_as_web_rotated_rectangle() {
        let path = test_path("rotate-rectangle");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Rectangle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 40.0, y: 20.0 }));
        document
            .set_dimension(DimensionField::RectangleWidth, 40.0)
            .unwrap();
        project.save(&document).unwrap();
        assert!(document.rotate_selected(30.0, Point { x: 20.0, y: 10.0 }, false));
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(matches!(
            loaded.shapes[0],
            Shape::RotatedRectangle {
                rotation: 30.0,
                width: 40.0,
                height: 20.0,
                ..
            }
        ));
        assert_eq!(
            loaded
                .driving_dimensions
                .get(&(1, DimensionField::RectangleWidth)),
            Some(&40.0)
        );
        let node = &reopened.value["items"][0]["data"]["cad"]["nodes"][4];
        assert_eq!(node["primitive"]["type"], "rectangle");
        assert_eq!(node["primitive"]["rotation"], 30.0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn three_point_rectangle_modes_save_as_editable_web_rectangles() {
        let path = test_path("three-point-rectangles");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        for (tool, first, second, third) in [
            (
                crate::model::Tool::ThreePointRectangle,
                Point { x: 0.0, y: 0.0 },
                Point { x: 30.0, y: 40.0 },
                Point { x: -40.0, y: 30.0 },
            ),
            (
                crate::model::Tool::ThreePointCenterRectangle,
                Point { x: 100.0, y: 100.0 },
                Point { x: 130.0, y: 140.0 },
                Point { x: 60.0, y: 130.0 },
            ),
        ] {
            document.tool = tool;
            document.draft_start = Some(first);
            assert!(!document.finish_draft(second));
            assert!(document.finish_draft(third));
        }
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(loaded.shapes, document.shapes);
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node["primitive"]["type"] == "rectangle")
                .count(),
            2
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn single_profile_extrusion_has_guarded_native_preview() {
        let path = test_path("extrusion-preview");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Rectangle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 40.0, y: 20.0 }));
        project.save(&document).unwrap();
        let mut value = project.value;
        let nodes = value["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap();
        nodes.push(json!({"id":"extrude-1","type":"extrude","name":"Block","sketchId":"native-sketch-1","profileId":"native-sketch-1-profile-1","operation":"newBody","depth":25}));
        let (loaded, _) = ProjectFile::from_value(path.clone(), value.clone(), None).unwrap();
        assert!(loaded.read_only_reason().is_some());
        let preview = loaded.extrusion_preview().unwrap();
        assert_eq!(preview.name, "Block");
        assert_eq!(preview.depth, 25.0);
        assert_eq!(preview.profile.len(), 4);
        value["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()["profileId"] = json!("missing-profile");
        let (invalid, _) = ProjectFile::from_value(path.clone(), value, None).unwrap();
        assert!(invalid.extrusion_preview().is_none());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn extrusion_preview_rejects_construction_and_multiple_solids() {
        let path = test_path("extrusion-guards");
        let mut value: Value =
            serde_json::from_str(include_str!("../fixtures/extruded-rectangle.pcad")).unwrap();
        value["items"][0]["data"]["cad"]["nodes"][4]["construction"] = json!(true);
        let (construction, _) = ProjectFile::from_value(path.clone(), value.clone(), None).unwrap();
        assert!(construction.extrusion_preview().is_none());
        value["items"][0]["data"]["cad"]["nodes"][4]
            .as_object_mut()
            .unwrap()
            .remove("construction");
        value["items"][0]["data"]["cad"]["nodes"].as_array_mut().unwrap().push(json!({"id":"extrude-2","type":"extrude","sketchId":"native-sketch-1","profileId":"native-sketch-1-profile-1","operation":"newBody","depth":10}));
        let (multiple, _) = ProjectFile::from_value(path, value, None).unwrap();
        assert!(multiple.extrusion_preview().is_none());
        assert_eq!(multiple.extrusion_previews().unwrap().len(), 2);
    }

    #[test]
    fn two_reference_plane_sketches_create_two_new_bodies() {
        let path = test_path("two-new-bodies");
        let (mut project, mut first) = ProjectFile::open(&path).unwrap();
        first.tool = crate::model::Tool::Rectangle;
        first.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(first.finish_draft(Point { x: 20.0, y: 10.0 }));
        project.create_extrusion(&first, 15.0).unwrap();
        assert!(project.active_sketch_has_extrusion());
        let mut second = project
            .create_sketch_on_plane(&first, SketchPlane::Front)
            .unwrap();
        assert!(!project.active_sketch_has_extrusion());
        second.tool = crate::model::Tool::Rectangle;
        second.draft_start = Some(Point { x: 30.0, y: 0.0 });
        assert!(second.finish_draft(Point { x: 50.0, y: 20.0 }));
        project.create_extrusion(&second, 25.0).unwrap();
        project.set_extrusion_depth_at(1, 35.0).unwrap();
        assert!(project.set_extrusion_depth_at(2, 40.0).is_err());
        project.save(&second).unwrap();
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        let solids = reopened.extrusion_previews().unwrap();
        assert_eq!(solids.len(), 2);
        assert_eq!(solids[0].depth, 15.0);
        assert_eq!(solids[0].plane, SketchPlane::Top);
        assert_eq!(solids[1].depth, 35.0);
        assert_eq!(solids[1].plane, SketchPlane::Front);
        assert_eq!(reopened.read_only_reason(), None);
        reopened
            .create_sketch_on_plane(&loaded, SketchPlane::Right)
            .unwrap();
        let _ = fs::remove_file(path);
    }

    #[test]
    fn cut_extrusion_saves_reopens_and_reduces_evaluated_body_volume() {
        let path = test_path("native-cut");
        let (mut project, mut base) = ProjectFile::open(&path).unwrap();
        base.tool = crate::model::Tool::Rectangle;
        base.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(base.finish_draft(Point { x: 100.0, y: 100.0 }));
        project.create_extrusion(&base, 50.0).unwrap();
        let mut pocket = project
            .create_sketch_on_plane(&base, SketchPlane::Top)
            .unwrap();
        pocket.tool = crate::model::Tool::Rectangle;
        pocket.draft_start = Some(Point { x: 25.0, y: 25.0 });
        assert!(pocket.finish_draft(Point { x: 75.0, y: 75.0 }));
        project
            .create_extrusion_with_operation(&pocket, 50.0, 0, "cut")
            .unwrap();
        let solids = project.evaluated_solids().unwrap();
        assert_eq!(solids.len(), 1);
        assert!((solids[0].volume() - 375000.0).abs() < 1.0);
        assert_eq!(project.extrusion_operation_at(1), Some("cut"));
        assert!(project.delete_extrusion_at(0).is_err());
        project.save(&pocket).unwrap();
        let (mut reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(reopened.extrusion_previews().unwrap().len(), 2);
        assert!((reopened.evaluated_solids().unwrap()[0].volume() - 375000.0).abs() < 1.0);
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|node| node["operation"] == "cut")
                .count(),
            1
        );
        reopened.delete_extrusion_at(1).unwrap();
        assert!((reopened.evaluated_solids().unwrap()[0].volume() - 500000.0).abs() < 1.0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn join_extrusion_saves_reopens_as_one_evaluated_body() {
        let path = test_path("native-join");
        let (mut project, mut base) = ProjectFile::open(&path).unwrap();
        base.tool = crate::model::Tool::Rectangle;
        base.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(base.finish_draft(Point { x: 100.0, y: 100.0 }));
        assert!(
            project
                .create_extrusion_with_operation(&base, 50.0, 0, "join")
                .is_err()
        );
        project.create_extrusion(&base, 50.0).unwrap();
        let mut extension = project
            .create_sketch_on_plane(&base, SketchPlane::Top)
            .unwrap();
        extension.tool = crate::model::Tool::Rectangle;
        extension.draft_start = Some(Point { x: 75.0, y: 0.0 });
        assert!(extension.finish_draft(Point { x: 175.0, y: 100.0 }));
        project
            .create_extrusion_with_operation(&extension, 50.0, 0, "join")
            .unwrap();
        assert_eq!(project.extrusion_operation_at(1), Some("join"));
        let solids = project.evaluated_solids().unwrap();
        assert_eq!(solids.len(), 1);
        assert!((solids[0].volume() - 875000.0).abs() < 1.0);
        assert!(project.delete_extrusion_at(0).is_err());
        project.save(&extension).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(reopened.extrusion_previews().unwrap().len(), 2);
        assert!((reopened.evaluated_solids().unwrap()[0].volume() - 875000.0).abs() < 1.0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn deleting_extrusion_cleans_graph_and_unlocks_sketch_after_reopen() {
        let path = test_path("delete-extrusion");
        fs::write(&path, include_str!("../fixtures/extruded-rectangle.pcad")).unwrap();
        let (mut project, document) = ProjectFile::open(&path).unwrap();
        assert_eq!(project.read_only_reason(), None);
        assert!(project.active_sketch_has_extrusion());
        project.delete_extrusion_at(0).unwrap();
        assert!(!project.active_sketch_has_extrusion());
        assert!(project.extrusion_previews().is_none());
        assert!(project.delete_extrusion_at(0).is_err());
        project.save(&document).unwrap();
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        let data = &reopened.value["items"][0]["data"];
        assert!(
            data["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|node| node["type"] != "extrude")
        );
        assert!(
            !data["cad"]["rootNodeIds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == "extrude-1")
        );
        assert!(
            !data["tree"]["orderedNodeIds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == "extrude-1")
        );
        reopened.create_extrusion(&loaded, 12.0).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn construction_flag_round_trips_and_prevents_extrusion() {
        let path = test_path("construction");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Rectangle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 20.0, y: 10.0 }));
        assert_eq!(document.toggle_construction(), Some(true));
        assert!(project.create_extrusion(&document, 20.0).is_err());
        project.save(&document).unwrap();
        let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
        assert!(loaded.construction.contains(&1));
        let node = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["type"] == "sketchCornerRectangle")
            .unwrap();
        assert_eq!(node["construction"], true);
        loaded.selected = Some(1);
        assert_eq!(loaded.toggle_construction(), Some(false));
        reopened.save(&loaded).unwrap();
        let (mut normal, normal_document) = ProjectFile::open(&path).unwrap();
        assert!(!normal_document.construction.contains(&1));
        normal.create_extrusion(&normal_document, 20.0).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_extrusion_creation_saves_web_graph_and_reopens_preview() {
        let path = test_path("create-extrusion");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Rectangle;
        document.draft_start = Some(Point { x: -20.0, y: -10.0 });
        assert!(document.finish_draft(Point { x: 20.0, y: 10.0 }));
        project.create_extrusion(&document, 35.0).unwrap();
        assert_eq!(project.extrusion_preview().unwrap().depth, 35.0);
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(reopened.extrusion_preview().unwrap().profile.len(), 4);
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        let extrude = nodes.iter().find(|node| node["type"] == "extrude").unwrap();
        assert_eq!(extrude["operation"], "newBody");
        assert_eq!(extrude["profileId"], "native-sketch-1-profile-1");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn extrusion_ignores_construction_guides() {
        let path = test_path("extrude-with-guide");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: -50.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 50.0, y: 0.0 }));
        assert_eq!(document.toggle_construction(), Some(true));
        document.tool = crate::model::Tool::Rectangle;
        document.draft_start = Some(Point { x: -20.0, y: -10.0 });
        assert!(document.finish_draft(Point { x: 20.0, y: 10.0 }));
        project.create_extrusion(&document, 30.0).unwrap();
        assert_eq!(project.extrusion_preview().unwrap().profile.len(), 4);
        project.save(&document).unwrap();
        let (reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.extrusion_preview().unwrap().depth, 30.0);
        assert!(loaded.construction.contains(&1));
        assert!(!loaded.construction.contains(&2));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn convex_and_concave_line_loops_extrude_but_open_or_crossed_loops_are_rejected() {
        let path = test_path("line-loop-extrusion");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let p = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 40.0, y: 0.0 },
            Point { x: 40.0, y: 20.0 },
            Point { x: 0.0, y: 20.0 },
        ];
        let lines = vec![
            Shape::Line {
                id: 1,
                a: p[0],
                b: p[1],
            },
            Shape::Line {
                id: 2,
                a: p[2],
                b: p[1],
            },
            Shape::Line {
                id: 3,
                a: p[2],
                b: p[3],
            },
            Shape::Line {
                id: 4,
                a: p[0],
                b: p[3],
            },
        ];
        let document = Document::from_shapes(lines.clone());
        assert_eq!(profile_from_document(&document).unwrap().outer.len(), 4);
        project.create_extrusion(&document, 15.0).unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.extrusion_preview().unwrap().depth, 15.0);
        let open = Document::from_shapes(lines[..3].to_vec());
        assert!(profile_from_document(&open).is_none());
        let concave = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: p[0],
                b: p[1],
            },
            Shape::Line {
                id: 2,
                a: p[1],
                b: Point { x: 20.0, y: 5.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: 20.0, y: 5.0 },
                b: p[2],
            },
            Shape::Line {
                id: 4,
                a: p[2],
                b: p[3],
            },
            Shape::Line {
                id: 5,
                a: p[3],
                b: p[0],
            },
        ]);
        assert_eq!(profile_from_document(&concave).unwrap().outer.len(), 5);
        let concave_solid = SolidPreview {
            name: "Concave".into(),
            profile: profile_from_document(&concave).unwrap().outer,
            holes: Vec::new(),
            depth: 15.0,
            plane: SketchPlane::Top,
        };
        assert!(concave_solid.cap_mesh().is_some());
        let crossed = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: p[0],
                b: p[2],
            },
            Shape::Line {
                id: 2,
                a: p[2],
                b: p[1],
            },
            Shape::Line {
                id: 3,
                a: p[1],
                b: p[3],
            },
            Shape::Line {
                id: 4,
                a: p[3],
                b: p[0],
            },
        ]);
        assert!(profile_from_document(&crossed).is_none());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn separate_line_loops_make_a_through_hole_and_reopen() {
        let path = test_path("line-hole-extrusion");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let outer = [
            Point { x: -50.0, y: -40.0 },
            Point { x: 50.0, y: -40.0 },
            Point { x: 50.0, y: 40.0 },
            Point { x: -50.0, y: 40.0 },
        ];
        let inner = [
            Point { x: -10.0, y: -10.0 },
            Point { x: 10.0, y: -10.0 },
            Point { x: 10.0, y: 10.0 },
            Point { x: -10.0, y: 10.0 },
        ];
        let lines = [outer, inner]
            .iter()
            .flat_map(|points| (0..4).map(move |index| (points[index], points[(index + 1) % 4])))
            .enumerate()
            .map(|(index, (a, b))| Shape::Line {
                id: index as u32 + 1,
                a,
                b,
            })
            .collect::<Vec<_>>();
        let document = Document::from_shapes(lines);
        let profile = profile_from_document(&document).unwrap();
        assert_eq!(profile.outer.len(), 4);
        assert_eq!(profile.holes.len(), 1);
        assert_eq!(profile.holes[0].len(), 4);
        project.create_extrusion(&document, 20.0).unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        let solid = reopened.extrusion_preview().unwrap();
        assert_eq!(solid.holes.len(), 1);
        assert!(solid.cap_mesh().is_some());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn slot_extrusion_uses_the_web_capsule_outline() {
        let path = test_path("slot-extrusion");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let document = Document::from_shapes(vec![Shape::Capsule {
            id: 1,
            from: Point { x: -25.0, y: 0.0 },
            to: Point { x: 25.0, y: 0.0 },
            width: 20.0,
            arc_segments: 12,
        }]);
        let profile = profile_from_document(&document).unwrap();
        assert_eq!(profile.outer.len(), 26);
        assert!(profile.holes.is_empty());
        project.create_extrusion(&document, 12.0).unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        let solid = reopened.extrusion_preview().unwrap();
        assert_eq!(solid.profile.len(), 26);
        assert_eq!(solid.depth, 12.0);
        assert!(solid.cap_mesh().is_some());
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["type"] == "sketchPrimitive")
                .unwrap()["primitive"]["arcSegments"],
            12
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn arc_and_line_close_a_semicircular_extrusion_with_preserved_segments() {
        let path = test_path("arc-line-extrusion");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let document = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                start_angle: 0.0,
                sweep: std::f64::consts::PI,
                segments: 32,
            },
            Shape::Line {
                id: 2,
                a: Point { x: -10.0, y: 0.0 },
                b: Point { x: 10.0, y: 0.0 },
            },
        ]);
        assert_eq!(profile_from_document(&document).unwrap().outer.len(), 17);
        project.create_extrusion(&document, 15.0).unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.extrusion_preview().unwrap().profile.len(), 17);
        assert_eq!(
            reopened.value["items"][0]["data"]["cad"]["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .find(|node| node["type"] == "sketchArc")
                .unwrap()["arc"]["segments"],
            32
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn circle_and_ellipse_extrusions_keep_their_web_segment_counts() {
        for (name, shape, expected) in [
            (
                "circle-12-segments",
                Shape::Circle {
                    id: 1,
                    center: Point { x: 0.0, y: 0.0 },
                    radius: 20.0,
                    segments: 12,
                },
                12,
            ),
            (
                "ellipse-16-segments",
                Shape::Ellipse {
                    id: 1,
                    center: Point { x: 0.0, y: 0.0 },
                    rx: 30.0,
                    ry: 10.0,
                    rotation: 15.0,
                    segments: 16,
                },
                16,
            ),
        ] {
            let path = test_path(name);
            let (mut project, _) = ProjectFile::open(&path).unwrap();
            let document = Document::from_shapes(vec![shape]);
            project.create_extrusion(&document, 10.0).unwrap();
            project.save(&document).unwrap();
            let (reopened, loaded) = ProjectFile::open(&path).unwrap();
            assert_eq!(
                reopened.extrusion_preview().unwrap().profile.len(),
                expected
            );
            assert_eq!(
                profile_from_document(&loaded).unwrap().outer.len(),
                expected
            );
            assert_eq!(
                reopened.value["items"][0]["data"]["cad"]["nodes"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|node| node["type"] == "sketchPrimitive")
                    .unwrap()["primitive"]["segments"],
                expected
            );
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn disconnected_regions_have_separate_web_profile_ids() {
        let path = test_path("two-regions");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let document = Document::from_shapes(vec![
            Shape::Rectangle {
                id: 1,
                a: Point { x: -80.0, y: -20.0 },
                b: Point { x: -40.0, y: 20.0 },
            },
            Shape::Rectangle {
                id: 2,
                a: Point { x: 30.0, y: -10.0 },
                b: Point { x: 90.0, y: 10.0 },
            },
        ]);
        assert_eq!(profiles_from_document(&document).unwrap().len(), 2);
        assert!(profile_from_document(&document).is_none());
        project
            .create_extrusion_for_profile(&document, 25.0, 1)
            .unwrap();
        assert!(
            project
                .create_extrusion_for_profile(&document, 30.0, 1)
                .is_err()
        );
        project
            .create_extrusion_for_profile(&document, 30.0, 0)
            .unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        let solids = reopened.extrusion_previews().unwrap();
        assert_eq!(solids.len(), 2);
        assert_eq!(solids[0].profile.len(), 4);
        assert_eq!(solids[0].depth, 25.0);
        assert_eq!(solids[1].depth, 30.0);
        let profile_ids: Vec<_> = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|node| node["type"] == "extrude")
            .map(|node| node["profileId"].as_str().unwrap())
            .collect();
        assert_eq!(
            profile_ids,
            ["native-sketch-1-profile-2", "native-sketch-1-profile-1"]
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn island_inside_a_hole_is_a_second_profile() {
        let document = Document::from_shapes(vec![
            Shape::Rectangle {
                id: 1,
                a: Point { x: -50.0, y: -50.0 },
                b: Point { x: 50.0, y: 50.0 },
            },
            Shape::Rectangle {
                id: 2,
                a: Point { x: -30.0, y: -30.0 },
                b: Point { x: 30.0, y: 30.0 },
            },
            Shape::Circle {
                id: 3,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 16,
            },
        ]);
        let profiles = profiles_from_document(&document).unwrap();
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].outer.len(), 4);
        assert_eq!(profiles[0].holes.len(), 1);
        assert_eq!(profiles[1].outer.len(), 16);
        assert!(profiles[1].holes.is_empty());
    }

    #[test]
    fn creating_a_part_preserves_unsaved_old_geometry_and_reopens_both_parts() {
        let path = test_path("create-part");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let old_document = Document::from_shapes(vec![Shape::Rectangle {
            id: 1,
            a: Point { x: -20.0, y: -10.0 },
            b: Point { x: 20.0, y: 10.0 },
        }]);
        let new_document = project.create_part(&old_document).unwrap();
        assert!(new_document.shapes.is_empty());
        assert_eq!(project.part_indices(), vec![0, 1]);
        assert_eq!(project.part_name(), "Part 2");
        let mut new_document = new_document;
        new_document.tool = crate::model::Tool::Circle;
        new_document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(new_document.finish_draft(Point { x: 10.0, y: 0.0 }));
        project.save(&new_document).unwrap();
        let (mut reopened, loaded_new) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.active_part_index(), 1);
        assert!(matches!(loaded_new.shapes[0], Shape::Circle { .. }));
        let loaded_old = reopened.switch_part(0, &loaded_new).unwrap();
        assert!(matches!(loaded_old.shapes[0], Shape::Rectangle { .. }));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn part_and_sketch_rename_round_trip_without_changing_geometry() {
        let path = test_path("rename-part-and-sketch");
        let (mut project, document) = ProjectFile::open(&path).unwrap();
        assert!(project.rename_part("   ").is_err());
        assert!(project.rename_sketch("\n").is_err());
        project.rename_part("Frame").unwrap();
        project.rename_sketch("Base profile").unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.part_name(), "Frame");
        assert_eq!(reopened.sketch_name(0), Some("Base profile"));
        assert_eq!(reopened.sketch_plane(0), Some(SketchPlane::Top));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn front_and_right_plane_extrusions_use_web_coordinate_frames() {
        for plane in [SketchPlane::Front, SketchPlane::Right] {
            let path = test_path(plane.name());
            let (mut project, top) = ProjectFile::open(&path).unwrap();
            let mut document = project.create_sketch_on_plane(&top, plane).unwrap();
            document.tool = crate::model::Tool::Rectangle;
            document.draft_start = Some(Point { x: 10.0, y: 20.0 });
            assert!(document.finish_draft(Point { x: 50.0, y: 40.0 }));
            project.create_extrusion(&document, 30.0).unwrap();
            project.save(&document).unwrap();
            let (reopened, _) = ProjectFile::open(&path).unwrap();
            let solid = reopened.extrusion_preview().unwrap();
            assert_eq!(solid.plane, plane);
            let mapped = solid.world_point(Point { x: 10.0, y: 20.0 }, 30.0);
            assert_eq!(
                mapped,
                if plane == SketchPlane::Front {
                    [10.0, 30.0, -20.0]
                } else {
                    [30.0, 10.0, 20.0]
                }
            );
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn nested_circle_is_a_hole_in_the_extruded_profile() {
        let path = test_path("extruded-hole");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Rectangle;
        document.draft_start = Some(Point { x: -50.0, y: -40.0 });
        assert!(document.finish_draft(Point { x: 50.0, y: 40.0 }));
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 15.0, y: 0.0 }));
        project.create_extrusion(&document, 25.0).unwrap();
        let preview = project.extrusion_preview().unwrap();
        assert_eq!(preview.holes.len(), 1);
        assert_eq!(preview.holes[0].len(), 64);
        assert!(preview.cap_mesh().is_some());
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.extrusion_preview().unwrap().holes.len(), 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn two_separate_holes_round_trip_and_overlapping_holes_are_rejected() {
        let path = test_path("two-holes");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let outer = Shape::Rectangle {
            id: 1,
            a: Point { x: -60.0, y: -40.0 },
            b: Point { x: 60.0, y: 40.0 },
        };
        let left = Shape::Circle {
            id: 2,
            center: Point { x: -25.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        };
        let right = Shape::Circle {
            id: 3,
            center: Point { x: 25.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        };
        let document = Document::from_shapes(vec![outer.clone(), left.clone(), right]);
        assert_eq!(profile_from_document(&document).unwrap().holes.len(), 2);
        project.create_extrusion(&document, 20.0).unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.extrusion_preview().unwrap().holes.len(), 2);
        let overlapping = Document::from_shapes(vec![
            outer.clone(),
            left.clone(),
            Shape::Circle {
                id: 3,
                center: Point { x: -15.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
        ]);
        assert!(profile_from_document(&overlapping).is_none());
        let nested = Document::from_shapes(vec![
            outer,
            left,
            Shape::Circle {
                id: 3,
                center: Point { x: -25.0, y: 0.0 },
                radius: 5.0,
                segments: 64,
            },
        ]);
        assert!(profile_from_document(&nested).is_none());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn extrusion_depth_save_preserves_other_graph_nodes_and_checks_disk() {
        let path = test_path("edit-extrusion-depth");
        fs::write(&path, include_str!("../fixtures/extruded-rectangle.pcad")).unwrap();
        let (mut project, document) = ProjectFile::open(&path).unwrap();
        let original = project.value.clone();
        assert!(project.set_extrusion_depth(0.0).is_err());
        assert!(project.set_extrusion_depth(f32::NAN).is_err());
        project.set_extrusion_depth(80.0).unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.extrusion_preview().unwrap().depth, 80.0);
        let mut expected = original;
        let nodes = expected["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap();
        nodes
            .iter_mut()
            .find(|node| node["type"] == "extrude")
            .unwrap()["depth"] = json!(80.0);
        let revision = expected["revision"].as_u64().unwrap_or(0);
        expected["revision"] = json!(revision + 1);
        assert_eq!(reopened.value, expected);
        fs::write(&path, "externally changed").unwrap();
        project.set_extrusion_depth(90.0).unwrap();
        assert!(
            project
                .save(&document)
                .unwrap_err()
                .contains("changed on disk")
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "externally changed");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn extrusion_creation_rejects_open_geometry_without_mutating_graph() {
        let path = test_path("bad-extrusion");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Line;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert!(project.create_extrusion(&document, 20.0).is_err());
        assert!(!project.has_extrusion());
        assert!(project.extrusion_preview().is_none());
    }

    #[test]
    fn separate_open_line_and_point_do_not_block_a_closed_extrusion() {
        let path = test_path("extrude-with-open-geometry");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let document = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 80.0, y: 0.0 },
                b: Point { x: 100.0, y: 0.0 },
            },
            Shape::Point {
                id: 2,
                at: Point { x: 0.0, y: 0.0 },
            },
            Shape::Rectangle {
                id: 3,
                a: Point { x: -20.0, y: -10.0 },
                b: Point { x: 20.0, y: 10.0 },
            },
        ]);
        assert_eq!(profile_from_document(&document).unwrap().outer.len(), 4);
        project.create_extrusion(&document, 20.0).unwrap();
        project.save(&document).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.extrusion_preview().unwrap().profile.len(), 4);
        fs::remove_file(path).unwrap();

        let attached_branch = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 20.0, y: 0.0 },
                b: Point { x: 20.0, y: 20.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: 20.0, y: 20.0 },
                b: Point { x: 0.0, y: 20.0 },
            },
            Shape::Line {
                id: 4,
                a: Point { x: 0.0, y: 20.0 },
                b: Point { x: 0.0, y: 0.0 },
            },
            Shape::Line {
                id: 5,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: -10.0, y: 0.0 },
            },
        ]);
        assert!(profile_from_document(&attached_branch).is_none());
    }

    #[test]
    fn ellipse_round_trips_with_web_primitive_and_dimensions() {
        let path = test_path("ellipse");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Ellipse;
        document.draft_start = Some(Point { x: 10.0, y: 20.0 });
        assert!(!document.finish_draft(Point { x: 30.0, y: 40.0 }));
        assert!(document.finish_draft(Point { x: -5.0, y: 35.0 }));
        document
            .set_dimension(DimensionField::RectangleWidth, 60.0)
            .unwrap();
        project.save(&document).unwrap();
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(
            matches!(loaded.shapes[0], Shape::Ellipse { center: Point { x: 10.0, y: 20.0 }, rx, ry, rotation, .. } if (rx - 30.0).abs() < 1e-4 && (ry - 15.0 * 2.0_f32.sqrt()).abs() < 1e-4 && (rotation - 45.0).abs() < 1e-4)
        );
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(nodes[4]["primitive"]["type"], "ellipse");
        assert_eq!(nodes[4]["primitive"]["width"], 60.0);
        assert_eq!(nodes[4]["primitive"]["rotation"], 45.0);
        assert_eq!(nodes[4]["primitive"]["segments"], 128);
        assert_eq!(nodes[3]["relations"][0]["type"], "width");
        reopened.save(&loaded).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn three_point_arc_round_trips_in_web_graph() {
        let path = test_path("three-point-arc");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Arc;
        document.draft_start = Some(Point { x: 10.0, y: 0.0 });
        assert!(!document.finish_draft(Point { x: 0.0, y: 10.0 }));
        assert!(document.finish_draft(Point { x: -10.0, y: 0.0 }));
        project.save(&document).unwrap();
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(
            matches!(loaded.shapes[0], Shape::Arc { radius, sweep, .. } if (radius - 10.0).abs() < 1e-5 && (sweep + 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-5)
        );
        let arc = &reopened.value["items"][0]["data"]["cad"]["nodes"][4];
        assert_eq!(arc["type"], "sketchArc");
        assert_eq!(arc["arc"]["type"], "arc");
        assert_eq!(arc["arc"]["segments"], 128);
        reopened.save(&loaded).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn point_and_polygon_round_trip_with_side_count() {
        let path = test_path("point-polygon");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.add_point(Point { x: 5.0, y: 6.0 });
        document.tool = crate::model::Tool::Polygon;
        document.draft_start = Some(Point { x: 20.0, y: 30.0 });
        assert!(document.finish_draft(Point { x: 40.0, y: 30.0 }));
        document.set_polygon_sides(5).unwrap();
        project.save(&document).unwrap();
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(matches!(loaded.shapes[0], Shape::Point { .. }));
        assert!(matches!(
            loaded.shapes[1],
            Shape::Polygon {
                sides: 5,
                radius: 20.0,
                ..
            }
        ));
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(nodes[4]["primitive"]["type"], "point");
        assert_eq!(nodes[5]["primitive"]["type"], "polygon");
        assert_eq!(nodes[5]["primitive"]["sides"], 5);
        reopened.save(&loaded).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn numeric_point_coordinates_survive_project_reopen() {
        let path = test_path("point-coordinates");
        let (mut project, _) = ProjectFile::open(&path).unwrap();
        let mut document = Document::from_shapes(vec![Shape::Point {
            id: 1,
            at: Point { x: 10.0, y: 20.0 },
        }]);
        document.selected = Some(1);
        document.set_selected_point_coordinate(true, -15.0).unwrap();
        document.set_selected_point_coordinate(false, 25.0).unwrap();
        project.save(&document).unwrap();
        let (_, reopened) = ProjectFile::open(&path).unwrap();
        assert!(matches!(reopened.shapes[0], Shape::Point { at, .. }
            if at == Point { x: -15.0, y: 25.0 }));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn slot_round_trips_as_capsule_primitive() {
        let path = test_path("slot");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Slot;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!document.finish_draft(Point { x: 20.0, y: 0.0 }));
        assert!(document.finish_draft(Point { x: 10.0, y: 5.0 }));
        document.set_capsule_width(14.0).unwrap();
        project.save(&document).unwrap();
        let (mut reopened, loaded) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert!(matches!(
            loaded.shapes[0],
            Shape::Capsule { width: 14.0, .. }
        ));
        let primitive = &reopened.value["items"][0]["data"]["cad"]["nodes"][4]["primitive"];
        assert_eq!(primitive["type"], "capsule");
        assert_eq!(primitive["width"], 14.0);
        assert_eq!(primitive["arcSegments"], 48);
        reopened.save(&loaded).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn multiple_sketches_keep_independent_unsaved_edits_and_reopen() {
        let path = test_path("multiple-sketches");
        let (mut project, mut first) = ProjectFile::open(&path).unwrap();
        first.tool = crate::model::Tool::Circle;
        first.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(first.finish_draft(Point { x: 10.0, y: 0.0 }));
        let mut second = project
            .create_sketch_on_plane(&first, SketchPlane::Front)
            .unwrap();
        assert_eq!(project.sketch_count(), 2);
        assert_eq!(project.sketch_name(1), Some("Sketch 2"));
        assert_eq!(project.sketch_plane(1), Some(SketchPlane::Front));
        second.tool = crate::model::Tool::Line;
        second.draft_start = Some(Point { x: 2.0, y: 3.0 });
        assert!(second.finish_draft(Point { x: 12.0, y: 13.0 }));
        let mut third = project
            .create_sketch_on_plane(&second, SketchPlane::Right)
            .unwrap();
        third.tool = crate::model::Tool::Rectangle;
        third.draft_start = Some(Point { x: 3.0, y: 5.0 });
        assert!(third.finish_draft(Point { x: 13.0, y: 15.0 }));
        let first = project.switch_sketch(0, &third).unwrap();
        assert!(matches!(first.shapes[0], Shape::Circle { .. }));
        project.save(&first).unwrap();
        let (mut reopened, first) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(reopened.sketch_count(), 3);
        assert!(matches!(first.shapes[0], Shape::Circle { .. }));
        let second = reopened.switch_sketch(1, &first).unwrap();
        assert!(matches!(second.shapes[0], Shape::Line { .. }));
        let third = reopened.switch_sketch(2, &second).unwrap();
        assert!(matches!(third.shapes[0], Shape::Rectangle { .. }));
        assert_eq!(reopened.sketch_plane(2), Some(SketchPlane::Right));
        let nodes = reopened.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        assert!(
            nodes
                .iter()
                .any(|node| node["type"] == "sketchLine" && node["sketchId"] == "native-sketch-2")
        );
        assert!(
            nodes
                .iter()
                .any(|node| node["id"] == "native-sketch-2" && node["targetId"] == "plane-front")
        );
        assert!(
            nodes
                .iter()
                .any(|node| node["id"] == "native-sketch-3" && node["targetId"] == "plane-right")
        );
        assert!(
            nodes
                .iter()
                .any(|node| node["type"] == "sketchPrimitive"
                    && node["sketchId"] == "native-sketch-1")
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unsupported_relations_keep_project_read_only() {
        let path = test_path("unsupported-relation");
        let (project, _) = ProjectFile::open(&path).unwrap();
        let mut value = project.value;
        value["items"][0]["data"]["cad"]["nodes"][3]["relations"] = json!([{
            "id":"relation-1","type":"coincident","a":{"entityId":"a","point":"p0"},"b":{"entityId":"b","point":"p1"}
        }]);
        let (mut loaded, document) = ProjectFile::from_value(path.clone(), value, None).unwrap();
        assert!(loaded.read_only_reason().is_some());
        assert!(loaded.save(&document).is_err());
        loaded.rename_part("Renamed read-only part").unwrap();
        loaded.save(&document).unwrap();
        let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["items"][0]["name"], "Renamed read-only part");
        assert_eq!(
            saved["items"][0]["data"]["cad"]["nodes"][3]["relations"][0]["type"],
            "coincident"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn duplicate_driving_relations_do_not_get_silently_rewritten() {
        let path = test_path("duplicate-dimension");
        let (mut project, mut document) = ProjectFile::open(&path).unwrap();
        document.tool = crate::model::Tool::Circle;
        document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(document.finish_draft(Point { x: 10.0, y: 0.0 }));
        project.save(&document).unwrap();
        let mut value = project.value;
        value["items"][0]["data"]["cad"]["nodes"][3]["relations"] = json!([
            {"id":"one","type":"diameter","entityId":"native-entity-1","value":20},
            {"id":"two","type":"diameter","entityId":"native-entity-1","value":30}
        ]);
        let (mut loaded, doc) = ProjectFile::from_value(path.clone(), value, None).unwrap();
        assert_eq!(
            loaded.read_only_reason(),
            Some("duplicate sketch dimension relation")
        );
        assert!(loaded.save(&doc).is_err());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn retains_native_primitive_parameters_on_resave() {
        let path = test_path("primitive-preservation");
        let (project, _) = ProjectFile::open(&path).unwrap();
        let mut value = project.value;
        let nodes = value["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap();
        nodes.push(json!({
            "id":"existing-rect","type":"sketchPrimitive","sketchId":"native-sketch-1",
            "primitive":{"id":"existing-rect","type":"rectangle","center":{"x":10,"y":20},"width":30,"height":40,"rotation":30,"construction":true}
        }));
        nodes.push(json!({
            "id":"existing-circle","type":"sketchPrimitive","sketchId":"native-sketch-1",
            "primitive":{"id":"existing-circle","type":"circle","center":{"x":50,"y":60},"radius":12,"segments":32,"construction":true}
        }));
        let (mut loaded, mut document) =
            ProjectFile::from_value(path.clone(), value, None).unwrap();
        assert_eq!(document.shapes.len(), 2);
        assert_eq!(loaded.read_only_reason(), None);
        assert!(matches!(
            document.shapes[0],
            Shape::RotatedRectangle { rotation: 30.0, .. }
        ));
        document.selected = Some(1);
        document
            .set_dimension(DimensionField::RectangleWidth, 50.0)
            .unwrap();
        loaded.save(&document).unwrap();
        let saved_nodes = loaded.value["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap();
        let rectangle = saved_nodes
            .iter()
            .find(|node| node["id"] == "existing-rect")
            .unwrap();
        assert_eq!(rectangle["primitive"]["type"], "rectangle");
        assert_eq!(rectangle["primitive"]["construction"], true);
        assert_eq!(rectangle["primitive"]["rotation"], 30.0);
        assert_eq!(rectangle["primitive"]["width"], 50.0);
        let circle = saved_nodes
            .iter()
            .find(|node| node["id"] == "existing-circle")
            .unwrap();
        assert_eq!(circle["primitive"]["segments"], 32);
        assert_eq!(circle["primitive"]["construction"], true);
        fs::remove_file(path).unwrap();
    }
}

#[cfg(test)]
#[path = "project/dimension_tests.rs"]
mod dimension_tests;
