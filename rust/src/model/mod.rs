use crate::solver;
use crate::spline::{self, SplineMode};
use std::collections::{HashMap, HashSet};
mod dimensions;
mod drag;
mod edit;
mod geometry;
mod relations;
mod shapes;
mod solve;
#[cfg(test)]
mod tests;

use geometry::*;
pub use shapes::{Point, Shape};
pub use solve::{Pin, SketchAnalysis};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Line,
    Rectangle,
    CenterRectangle,
    ThreePointRectangle,
    ThreePointCenterRectangle,
    Circle,
    ThreePointCircle,
    Ellipse,
    FitSpline,
    ControlSpline,
    Arc,
    CenterArc,
    Point,
    Polygon,
    Slot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    Anchor,
    Midpoint,
    Intersection,
    Origin,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapResult {
    pub point: Point,
    pub kind: SnapKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DimensionField {
    LineLength,
    RectangleWidth,
    RectangleHeight,
    CircleDiameter,
    CurveRadius,
    ArcSweep,
    Rotation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineRelationKind {
    Parallel,
    Perpendicular,
    Collinear,
    Angle(f32),
}

impl LineRelationKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Parallel => "parallel",
            Self::Perpendicular => "perpendicular",
            Self::Collinear => "collinear",
            Self::Angle(_) => "angle",
        }
    }

    pub fn angle_value(self) -> Option<f32> {
        if let Self::Angle(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisConstraint {
    Horizontal,
    Vertical,
}

impl AxisConstraint {
    pub fn name(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

#[derive(Clone, Debug)]
struct DocumentState {
    shapes: Vec<Shape>,
    driving_dimensions: HashMap<(u32, DimensionField), f32>,
    construction: HashSet<u32>,
    axis_constraints: HashMap<u32, AxisConstraint>,
    point_links: HashMap<(u32, u8), u32>,
    equal_lines: HashMap<u32, u32>,
    equal_radii: HashMap<u32, u32>,
    radius_differences: HashMap<u32, (u32, f32)>,
    internal_tangents: HashMap<u32, u32>,
    external_tangents: HashMap<u32, u32>,
    midpoint_links: HashMap<u32, u32>,
    point_curve_links: HashMap<u32, u32>,
    normal_links: HashMap<u32, (u32, u8)>,
    endpoint_tangents: HashMap<u32, (u32, u8, i8, i8)>,
    equal_ellipses: HashMap<u32, u32>,
    shared_ellipses: HashMap<u32, u32>,
    concentric: HashMap<u32, u32>,
    line_relations: HashMap<u32, (u32, LineRelationKind)>,
    fixed_anchors: HashMap<(u32, usize), Point>,
}

#[derive(Clone, Debug)]
struct DragState {
    id: u32,
    start: Point,
    original: Shape,
    before: DocumentState,
    spline_point: Option<usize>,
    /// Single anchor being dragged (see `Document::drag_anchors`).
    anchor: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub shapes: Vec<Shape>,
    pub selected: Option<u32>,
    pub selected_spline_point: Option<usize>,
    pub tool: Tool,
    pub zoom: f32,
    pub pan: Point,
    pub draft_start: Option<Point>,
    pub draft_end: Option<Point>,
    pub arc_clockwise: bool,
    pub driving_dimensions: HashMap<(u32, DimensionField), f32>,
    pub construction: HashSet<u32>,
    pub axis_constraints: HashMap<u32, AxisConstraint>,
    pub point_links: HashMap<(u32, u8), u32>,
    pub equal_lines: HashMap<u32, u32>,
    pub equal_radii: HashMap<u32, u32>,
    pub radius_differences: HashMap<u32, (u32, f32)>,
    pub internal_tangents: HashMap<u32, u32>,
    pub external_tangents: HashMap<u32, u32>,
    pub midpoint_links: HashMap<u32, u32>,
    pub point_curve_links: HashMap<u32, u32>,
    pub normal_links: HashMap<u32, (u32, u8)>,
    pub endpoint_tangents: HashMap<u32, (u32, u8, i8, i8)>,
    pub equal_ellipses: HashMap<u32, u32>,
    pub shared_ellipses: HashMap<u32, u32>,
    pub concentric: HashMap<u32, u32>,
    pub line_relations: HashMap<u32, (u32, LineRelationKind)>,
    pub fixed_anchors: HashMap<(u32, usize), Point>,
    next_id: u32,
    history: Vec<DocumentState>,
    future: Vec<DocumentState>,
    drag: Option<DragState>,
    /// Bumped whenever an edit is committed or undone; see [`Document::revision`].
    revision: std::cell::Cell<u64>,
}

fn next_revision() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl Default for Document {
    fn default() -> Self {
        Self {
            shapes: Vec::new(),
            selected: None,
            selected_spline_point: None,
            tool: Tool::Select,
            zoom: 1.0,
            pan: Point { x: 0.0, y: 0.0 },
            draft_start: None,
            draft_end: None,
            arc_clockwise: false,
            driving_dimensions: HashMap::new(),
            construction: HashSet::new(),
            axis_constraints: HashMap::new(),
            point_links: HashMap::new(),
            equal_lines: HashMap::new(),
            equal_radii: HashMap::new(),
            radius_differences: HashMap::new(),
            internal_tangents: HashMap::new(),
            external_tangents: HashMap::new(),
            midpoint_links: HashMap::new(),
            point_curve_links: HashMap::new(),
            normal_links: HashMap::new(),
            endpoint_tangents: HashMap::new(),
            equal_ellipses: HashMap::new(),
            shared_ellipses: HashMap::new(),
            concentric: HashMap::new(),
            line_relations: HashMap::new(),
            fixed_anchors: HashMap::new(),
            next_id: 1,
            history: Vec::new(),
            future: Vec::new(),
            drag: None,
            revision: std::cell::Cell::new(next_revision()),
        }
    }
}

impl Document {
    pub fn snap_point(&self, raw: Point, grid: bool, geometry: bool) -> Option<SnapResult> {
        if !raw.x.is_finite() || !raw.y.is_finite() {
            return None;
        }
        let mut best = None;
        let mut distance = 8.0 / self.zoom.max(0.2);
        if geometry {
            for candidate in self.shapes.iter().flat_map(Shape::snap_points) {
                let d = raw.distance(candidate.point);
                if d < distance {
                    best = Some(candidate);
                    distance = d;
                }
            }
            for (index, shape) in self.shapes.iter().enumerate() {
                for other in self.shapes.iter().skip(index + 1) {
                    for point in finite_intersections(shape, other) {
                        let d = raw.distance(point);
                        if d <= distance {
                            best = Some(SnapResult {
                                point,
                                kind: SnapKind::Intersection,
                            });
                            distance = d;
                        }
                    }
                }
            }
            let origin = Point { x: 0.0, y: 0.0 };
            if raw.distance(origin) < distance {
                return Some(SnapResult {
                    point: origin,
                    kind: SnapKind::Origin,
                });
            }
        }
        best.or_else(|| {
            grid.then(|| SnapResult {
                point: Point {
                    x: raw.x.round(),
                    y: raw.y.round(),
                },
                kind: SnapKind::Grid,
            })
        })
    }

    pub fn fit_shapes(&mut self, viewport_width: f32, viewport_height: f32) -> bool {
        let mut bounds = self.shapes.iter().filter_map(Shape::bounds);
        let Some((mut min, mut max)) = bounds.next() else {
            return false;
        };
        for (lower, upper) in bounds {
            min.x = min.x.min(lower.x);
            min.y = min.y.min(lower.y);
            max.x = max.x.max(upper.x);
            max.y = max.y.max(upper.y);
        }
        if !viewport_width.is_finite()
            || !viewport_height.is_finite()
            || viewport_width <= 96.0
            || viewport_height <= 96.0
        {
            return false;
        }
        let span_x = (max.x - min.x).max(1.0);
        let span_y = (max.y - min.y).max(1.0);
        let zoom = ((viewport_width - 96.0) / span_x)
            .min((viewport_height - 96.0) / span_y)
            .clamp(0.2, 8.0);
        let center = Point {
            x: min.x * 0.5 + max.x * 0.5,
            y: min.y * 0.5 + max.y * 0.5,
        };
        if !zoom.is_finite() || !center.x.is_finite() || !center.y.is_finite() {
            return false;
        }
        self.zoom = zoom;
        self.pan = Point {
            x: -center.x * zoom,
            y: center.y * zoom,
        };
        true
    }

    /// Changes (globally uniquely) whenever an edit is about to be
    /// committed, is finished by a drag, or is undone/redone, so callers can
    /// cache per-edit analysis such as [`Document::analyze_constraints`].
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    fn snapshot(&self) -> DocumentState {
        self.revision.set(next_revision());
        DocumentState {
            shapes: self.shapes.clone(),
            driving_dimensions: self.driving_dimensions.clone(),
            construction: self.construction.clone(),
            axis_constraints: self.axis_constraints.clone(),
            point_links: self.point_links.clone(),
            equal_lines: self.equal_lines.clone(),
            equal_radii: self.equal_radii.clone(),
            radius_differences: self.radius_differences.clone(),
            internal_tangents: self.internal_tangents.clone(),
            external_tangents: self.external_tangents.clone(),
            midpoint_links: self.midpoint_links.clone(),
            point_curve_links: self.point_curve_links.clone(),
            normal_links: self.normal_links.clone(),
            endpoint_tangents: self.endpoint_tangents.clone(),
            equal_ellipses: self.equal_ellipses.clone(),
            shared_ellipses: self.shared_ellipses.clone(),
            concentric: self.concentric.clone(),
            line_relations: self.line_relations.clone(),
            fixed_anchors: self.fixed_anchors.clone(),
        }
    }

    fn restore(&mut self, state: DocumentState) {
        self.revision.set(next_revision());
        self.shapes = state.shapes;
        self.driving_dimensions = state.driving_dimensions;
        self.construction = state.construction;
        self.axis_constraints = state.axis_constraints;
        self.point_links = state.point_links;
        self.equal_lines = state.equal_lines;
        self.equal_radii = state.equal_radii;
        self.radius_differences = state.radius_differences;
        self.internal_tangents = state.internal_tangents;
        self.external_tangents = state.external_tangents;
        self.midpoint_links = state.midpoint_links;
        self.point_curve_links = state.point_curve_links;
        self.normal_links = state.normal_links;
        self.endpoint_tangents = state.endpoint_tangents;
        self.equal_ellipses = state.equal_ellipses;
        self.shared_ellipses = state.shared_ellipses;
        self.concentric = state.concentric;
        self.line_relations = state.line_relations;
        self.fixed_anchors = state.fixed_anchors;
        self.selected = None;
        self.selected_spline_point = None;
        self.draft_start = None;
        self.draft_end = None;
        self.drag = None;
    }

    pub fn from_shapes(shapes: Vec<Shape>) -> Self {
        let next_id = shapes
            .iter()
            .map(Shape::id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        Self {
            shapes,
            next_id,
            ..Self::default()
        }
    }

    pub fn screen_to_world(&self, point: Point, viewport_origin: Point) -> Point {
        Point {
            x: (point.x - viewport_origin.x - self.pan.x) / self.zoom,
            y: -(point.y - viewport_origin.y - self.pan.y) / self.zoom,
        }
    }

    pub fn world_to_screen(&self, point: Point, viewport_origin: Point) -> Point {
        Point {
            x: viewport_origin.x + self.pan.x + point.x * self.zoom,
            y: viewport_origin.y + self.pan.y - point.y * self.zoom,
        }
    }

    pub fn zoom_at(&mut self, cursor: Point, viewport_origin: Point, factor: f32) {
        let anchor = self.screen_to_world(cursor, viewport_origin);
        self.zoom = (self.zoom * factor).clamp(0.2, 8.0);
        self.pan.x = cursor.x - viewport_origin.x - anchor.x * self.zoom;
        self.pan.y = cursor.y - viewport_origin.y + anchor.y * self.zoom;
    }

    pub fn select_at(&mut self, point: Point) {
        let tolerance = 7.0 / self.zoom.max(0.2);
        if self.anchor_at(point).is_some() {
            self.selected_spline_point = None;
            return;
        }
        if let Some(Shape::Spline { points, .. }) = self
            .selected
            .and_then(|id| self.shapes.iter().find(|shape| shape.id() == id))
            && let Some((index, _)) = points
                .iter()
                .enumerate()
                .filter(|(_, handle)| handle.distance(point) <= tolerance)
                .min_by(|left, right| left.1.distance(point).total_cmp(&right.1.distance(point)))
        {
            self.selected_spline_point = Some(index);
            return;
        }
        self.selected = self
            .shapes
            .iter()
            .rev()
            .find(|shape| shape.hit(point, 5.0 / self.zoom))
            .map(Shape::id);
        self.selected_spline_point = self.selected.and_then(|id| {
            self.shapes
                .iter()
                .find(|shape| shape.id() == id)
                .and_then(|shape| {
                    if let Shape::Spline { points, .. } = shape {
                        points
                            .iter()
                            .enumerate()
                            .find(|(_, handle)| handle.distance(point) <= tolerance)
                            .map(|(index, _)| index)
                    } else {
                        None
                    }
                })
        });
    }

    pub fn set_selected_spline_coordinate(
        &mut self,
        x_axis: bool,
        value: f32,
    ) -> Result<(), String> {
        if !value.is_finite() {
            return Err("Spline coordinate must be finite".into());
        }
        let id = self.selected.ok_or("Select a spline point")?;
        let handle = self.selected_spline_point.ok_or("Select a spline point")?;
        if self.fixed_anchors.contains_key(&(id, handle)) {
            return Err("Spline point is fixed".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected spline is missing")?;
        let Shape::Spline {
            mode,
            ref points,
            closed,
            ..
        } = self.shapes[index]
        else {
            return Err("Select a spline point".into());
        };
        let mut changed = points.clone();
        let point = changed.get_mut(handle).ok_or("Spline point is missing")?;
        if x_axis {
            point.x = value;
        } else {
            point.y = value;
        }
        spline::beziers(mode, &changed, closed)?;
        if changed == *points {
            return Ok(());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if let Shape::Spline { points, .. } = &mut self.shapes[index] {
            *points = changed;
        }
        // Linked points and curve relations follow the edited handle.
        self.solve_or_rollback(&[Pin::Anchor(id, handle)])
    }

    pub fn set_selected_point_coordinate(
        &mut self,
        x_axis: bool,
        value: f32,
    ) -> Result<(), String> {
        if !value.is_finite() {
            return Err("Point coordinate must be finite".into());
        }
        let id = self.selected.ok_or("Select a sketch point")?;
        if self.midpoint_links.contains_key(&id) {
            return Err("Point is driven by Midpoint".into());
        }
        let Shape::Point { at, .. } = self
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .ok_or("Selected point is missing")?
        else {
            return Err("Select a sketch point".into());
        };
        let delta = if x_axis {
            Point {
                x: value - at.x,
                y: 0.0,
            }
        } else {
            Point {
                x: 0.0,
                y: value - at.y,
            }
        };
        if delta.x == 0.0 && delta.y == 0.0 {
            return Ok(());
        }
        if self.move_selected(delta) {
            Ok(())
        } else {
            Err("Point has a driving linked line or invalid coordinate".into())
        }
    }

    pub fn finish_draft(&mut self, end: Point) -> bool {
        if matches!(
            self.tool,
            Tool::ThreePointRectangle | Tool::ThreePointCenterRectangle
        ) {
            let Some(first) = self.draft_start else {
                return false;
            };
            if let Some(second) = self.draft_end {
                let shape = match self.tool {
                    Tool::ThreePointRectangle => {
                        Shape::rectangle_from_three_points(self.next_id, first, second, end)
                    }
                    _ => Shape::rectangle_from_center_side(self.next_id, first, second, end),
                };
                let Some(shape) = shape else {
                    return false;
                };
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if first.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::CenterArc {
            let Some(center) = self.draft_start else {
                return false;
            };
            if let Some(start) = self.draft_end {
                let Some(shape) = Shape::arc_from_center_points(
                    self.next_id,
                    center,
                    start,
                    end,
                    self.arc_clockwise,
                ) else {
                    return false;
                };
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if center.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::ThreePointCircle {
            let Some(first) = self.draft_start else {
                return false;
            };
            if let Some(second) = self.draft_end {
                let Some(shape) = Shape::circle_from_three_points(self.next_id, first, second, end)
                else {
                    return false;
                };
                if let Shape::Circle { radius, .. } = shape
                    && radius <= 2.0 / self.zoom
                {
                    return false;
                }
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if first.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::Slot {
            let Some(from) = self.draft_start else {
                return false;
            };
            if let Some(to) = self.draft_end {
                let Some(shape) = Shape::capsule_from_three_points(self.next_id, from, to, end)
                else {
                    return false;
                };
                if let Shape::Capsule { width, .. } = &shape
                    && *width <= 2.0 / self.zoom
                {
                    return false;
                }
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if from.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::Ellipse {
            let Some(center) = self.draft_start else {
                return false;
            };
            if let Some(first_axis) = self.draft_end {
                let Some(shape) =
                    Shape::ellipse_from_three_points(self.next_id, center, first_axis, end)
                else {
                    return false;
                };
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if center.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::Arc {
            let Some(start) = self.draft_start else {
                return false;
            };
            if let Some(arc_end) = self.draft_end {
                let Some(shape) = Shape::arc_from_three_points(self.next_id, start, arc_end, end)
                else {
                    return false;
                };
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if start.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        let Some(start) = self.draft_start.take() else {
            return false;
        };
        let minimum = 2.0 / self.zoom;
        let shape = match self.tool {
            Tool::Line if start.distance(end) > minimum => Shape::Line {
                id: self.next_id,
                a: start,
                b: end,
            },
            Tool::Rectangle
                if (end.x - start.x).abs() > minimum && (end.y - start.y).abs() > minimum =>
            {
                Shape::Rectangle {
                    id: self.next_id,
                    a: start,
                    b: end,
                }
            }
            Tool::CenterRectangle
                if (end.x - start.x).abs() > minimum * 0.5
                    && (end.y - start.y).abs() > minimum * 0.5 =>
            {
                Shape::Rectangle {
                    id: self.next_id,
                    a: Point {
                        x: start.x - (end.x - start.x),
                        y: start.y - (end.y - start.y),
                    },
                    b: end,
                }
            }
            Tool::Circle if start.distance(end) > minimum => Shape::Circle {
                id: self.next_id,
                center: start,
                radius: start.distance(end),
                segments: 64,
            },
            Tool::Polygon if start.distance(end) > minimum => Shape::Polygon {
                id: self.next_id,
                center: start,
                radius: start.distance(end),
                sides: 6,
                rotation: (end.y - start.y).atan2(end.x - start.x).to_degrees(),
            },
            Tool::Point => return false,
            Tool::ThreePointCircle => return false,
            Tool::Slot => return false,
            Tool::Ellipse => return false,
            Tool::Arc => return false,
            Tool::CenterArc => return false,
            Tool::ThreePointRectangle | Tool::ThreePointCenterRectangle => return false,
            _ => return false,
        };
        self.history.push(self.snapshot());
        self.future.clear();
        if let Shape::Line { id, a, b } = &shape {
            for (endpoint, position) in [(0, *a), (1, *b)] {
                if let Some(point_id) = self.shapes.iter().find_map(|existing| match existing {
                    Shape::Point { id: point_id, at } if at.distance(position) < 1e-5 => {
                        Some(*point_id)
                    }
                    _ => None,
                }) {
                    self.point_links.insert((*id, endpoint), point_id);
                }
            }
        }
        self.selected = Some(self.next_id);
        self.next_id += 1;
        self.shapes.push(shape);
        true
    }

    pub fn add_point(&mut self, at: Point) -> bool {
        if !at.x.is_finite() || !at.y.is_finite() {
            return false;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.selected = Some(self.next_id);
        self.shapes.push(Shape::Point {
            id: self.next_id,
            at,
        });
        self.next_id += 1;
        true
    }

    pub fn add_spline(
        &mut self,
        mode: SplineMode,
        points: Vec<Point>,
        closed: bool,
    ) -> Result<u32, String> {
        spline::beziers(mode, &points, closed)?;
        let id = self.next_id;
        self.history.push(self.snapshot());
        self.future.clear();
        self.selected = Some(id);
        self.selected_spline_point = None;
        if !closed {
            for (endpoint, position) in [(0, points[0]), (1, *points.last().unwrap())] {
                if let Some(point_id) = self.shapes.iter().find_map(|shape| match shape {
                    Shape::Point { id, at } if at.distance(position) < 1e-5 => Some(*id),
                    _ => None,
                }) {
                    self.point_links.insert((id, endpoint), point_id);
                }
            }
        }
        self.shapes.push(Shape::Spline {
            id,
            mode,
            points,
            closed,
        });
        self.next_id = self.next_id.saturating_add(1);
        Ok(id)
    }

    pub fn set_polygon_sides(&mut self, sides: u8) -> Result<(), String> {
        if !(3..=64).contains(&sides) {
            return Err("polygon sides must be between 3 and 64".into());
        }
        let id = self.selected.ok_or("select a polygon first")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("selected geometry no longer exists")?;
        let Shape::Polygon { sides: current, .. } = &self.shapes[index] else {
            return Err("select a polygon first".into());
        };
        if *current != sides {
            if self
                .fixed_anchors
                .keys()
                .any(|(shape_id, anchor)| *shape_id == id && *anchor > 0)
            {
                return Err("polygon vertex is fixed".into());
            }
            self.history.push(self.snapshot());
            self.future.clear();
            if let Shape::Polygon { sides: current, .. } = &mut self.shapes[index] {
                *current = sides;
            }
        }
        Ok(())
    }

    pub fn set_capsule_width(&mut self, width: f32) -> Result<(), String> {
        if !width.is_finite() || width <= 0.0 {
            return Err("slot width must be positive".into());
        }
        let id = self.selected.ok_or("select a slot first")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("selected geometry no longer exists")?;
        let Shape::Capsule { width: current, .. } = &self.shapes[index] else {
            return Err("select a slot first".into());
        };
        if *current != width {
            self.history.push(self.snapshot());
            self.future.clear();
            if let Shape::Capsule { width: current, .. } = &mut self.shapes[index] {
                *current = width;
            }
        }
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        if self.drag.is_some() {
            return self.cancel_drag();
        }
        let Some(previous) = self.history.pop() else {
            return false;
        };
        self.future.push(self.snapshot());
        self.restore(previous);
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.drag.is_some() {
            return self.cancel_drag();
        }
        let Some(next) = self.future.pop() else {
            return false;
        };
        self.history.push(self.snapshot());
        self.restore(next);
        true
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.future.clear();
        self.selected = None;
        self.drag = None;
    }

    pub fn toggle_construction(&mut self) -> Option<bool> {
        let id = self.selected?;
        if !self.shapes.iter().any(|shape| shape.id() == id) {
            return None;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let enabled = if self.construction.remove(&id) {
            false
        } else {
            self.construction.insert(id);
            true
        };
        Some(enabled)
    }
}
