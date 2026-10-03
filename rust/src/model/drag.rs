use super::*;

impl Document {
    pub fn begin_drag(&mut self, start: Point) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        if let Some((anchor_id, index)) = self.anchor_at(start) {
            // Anchors that cannot be dragged on their own fall back to
            // moving the whole shape.
            if self.begin_anchor_drag(anchor_id, index, start) {
                return true;
            }
            if self.fixed_anchors.contains_key(&(anchor_id, index)) {
                return false;
            }
        }
        let Some(original) = self.shapes.iter().find(|shape| shape.id() == id).cloned() else {
            return false;
        };
        if self.midpoint_links.contains_key(&id) {
            return false;
        }
        if self.has_fixed_anchor(id) {
            return false;
        }
        if self
            .shared_ellipse_partners(id)
            .into_iter()
            .any(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        if self
            .concentric_partner(id)
            .is_some_and(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        self.drag = Some(DragState {
            id,
            start,
            original,
            before: self.snapshot(),
            spline_point: self.selected_spline_point,
            anchor: None,
        });
        true
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub fn drag_to(&mut self, point: Point) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        let delta = Point {
            x: point.x - drag.start.x,
            y: point.y - drag.start.y,
        };
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return false;
        }
        if let Some(index) = drag.anchor {
            return self.drag_anchor_to(index, point);
        }
        let mut next = if drag.start.distance(point) > 2.0 / self.zoom {
            if let (
                Shape::Spline {
                    mode,
                    points,
                    closed,
                    ..
                },
                Some(handle),
            ) = (&drag.original, drag.spline_point)
            {
                let mut next = drag.original.clone();
                if let Shape::Spline {
                    points: changed, ..
                } = &mut next
                {
                    changed[handle] = Point {
                        x: points[handle].x + delta.x,
                        y: points[handle].y + delta.y,
                    };
                }
                if spline::beziers(
                    *mode,
                    match &next {
                        Shape::Spline { points, .. } => points,
                        _ => unreachable!(),
                    },
                    *closed,
                )
                .is_err()
                {
                    return false;
                }
                next
            } else {
                drag.original.translated(delta)
            }
        } else {
            drag.original.clone()
        };
        if let Some(&(source_id, kind)) = self.line_relations.get(&drag.id) {
            if kind == LineRelationKind::Collinear {
                let Some(source) = self.shapes.iter().find(|shape| shape.id() == source_id) else {
                    return false;
                };
                let Some(projected) = Self::line_relation_shape(source, &next, kind) else {
                    return false;
                };
                next = projected;
            }
        }
        let Some(index) = self.shapes.iter().position(|shape| shape.id() == drag.id) else {
            return false;
        };
        if self.shapes[index] == next {
            return false;
        }
        let frame_before = self.shapes.clone();
        self.shapes[index] = next;
        let id = drag.id;
        if self.solve_constraints(&[Pin::Shape(id)]).is_err() {
            self.shapes = frame_before;
            return false;
        }
        true
    }

    pub fn finish_drag(&mut self) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        let changed = self
            .shapes
            .iter()
            .any(|shape| shape.id() == drag.id && *shape != drag.original);
        if changed {
            self.history.push(drag.before);
            self.future.clear();
        }
        self.revision.set(next_revision());
        changed
    }

    pub fn cancel_drag(&mut self) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        self.restore(drag.before);
        true
    }
}

/// Single-anchor dragging. Anchor indices match `fixed_anchor_position`
/// where both exist; axis-aligned rectangles add the two derived corners as
/// indices 2 and 3.
impl Document {
    pub(super) fn shape_anchors(shape: &Shape) -> Vec<(usize, Point)> {
        match *shape {
            Shape::Line { a, b, .. } | Shape::Capsule { from: a, to: b, .. } => {
                vec![(0, a), (1, b)]
            }
            Shape::Point { at, .. } => vec![(0, at)],
            Shape::Circle { center, .. } | Shape::Arc { center, .. } => vec![(0, center)],
            Shape::Rectangle { a, b, .. } => vec![
                (0, a),
                (1, b),
                (2, Point { x: a.x, y: b.y }),
                (3, Point { x: b.x, y: a.y }),
            ],
            Shape::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => {
                let corners = Shape::rotated_rectangle_corners(center, width, height, rotation);
                (1..=4).map(|i| (i, corners[(i + 1) % 4])).collect()
            }
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            } => (1..=4)
                .map(|i| {
                    let fraction = (i - 1) as f32 / 4.0;
                    (i, Shape::ellipse_point(center, rx, ry, rotation, fraction))
                })
                .collect(),
            Shape::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            } => (1..=sides as usize)
                .map(|i| {
                    let at = Shape::polygon_vertex(center, radius, sides, rotation, (i - 1) as u8);
                    (i, at)
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Draggable anchors of shape `id` in world coordinates.
    pub fn drag_anchors(&self, id: u32) -> Vec<(usize, Point)> {
        self.shapes
            .iter()
            .find(|shape| shape.id() == id)
            .map(Self::shape_anchors)
            .unwrap_or_default()
    }

    /// Anchor of the selected shape under `point`, using the spline handle
    /// pick radius.
    pub fn anchor_at(&self, point: Point) -> Option<(u32, usize)> {
        let id = self.selected?;
        let tolerance = 7.0 / self.zoom.max(0.2);
        self.drag_anchors(id)
            .into_iter()
            .map(|(index, at)| (index, at.distance(point)))
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(index, _)| (id, index))
    }

    /// Shape id and anchor index of the anchor drag in progress.
    pub fn dragged_anchor(&self) -> Option<(u32, usize)> {
        self.drag
            .as_ref()
            .and_then(|drag| drag.anchor.map(|index| (drag.id, index)))
    }

    pub(super) fn solver_shape(shape: &Shape) -> bool {
        matches!(
            shape,
            Shape::Line { .. } | Shape::Point { .. } | Shape::Circle { .. } | Shape::Arc { .. }
        )
    }

    /// Start dragging one anchor. Refused for fixed anchors; every relation
    /// the shape takes part in follows through the solver.
    pub fn begin_anchor_drag(&mut self, id: u32, index: usize, start: Point) -> bool {
        let Some(original) = self.shapes.iter().find(|shape| shape.id() == id).cloned() else {
            return false;
        };
        if !Self::shape_anchors(&original)
            .iter()
            .any(|(anchor, _)| *anchor == index)
        {
            return false;
        }
        let fixed = if matches!(original, Shape::Line { .. }) {
            self.fixed_anchors.contains_key(&(id, index))
        } else {
            self.has_fixed_anchor(id)
        };
        if fixed {
            return false;
        }
        self.selected = Some(id);
        self.selected_spline_point = None;
        self.drag = Some(DragState {
            id,
            start,
            original,
            before: self.snapshot(),
            spline_point: None,
            anchor: Some(index),
        });
        true
    }

    pub(super) fn driving(&self, id: u32, field: DimensionField) -> Option<f32> {
        self.driving_dimensions.get(&(id, field)).copied()
    }

    /// `shape` with anchor `index` moved to `target`. Driving dimensions of
    /// non-solver shapes are kept: the anchor slides along what they allow.
    pub(super) fn with_anchor(&self, shape: &Shape, index: usize, target: Point) -> Option<Shape> {
        let id = shape.id();
        let mut next = shape.clone();
        let along = |moving: f32, opposite: f32, length: Option<f32>| match length {
            Some(length) => opposite + if moving >= opposite { length } else { -length },
            None => moving,
        };
        match &mut next {
            Shape::Line { a, b, .. } | Shape::Capsule { from: a, to: b, .. } => {
                *(if index == 0 { a } else { b }) = target;
            }
            Shape::Point { at, .. } => *at = target,
            Shape::Circle { center, .. } | Shape::Arc { center, .. } => *center = target,
            Shape::Rectangle { a, b, .. } => {
                let width = self.driving(id, DimensionField::RectangleWidth);
                let height = self.driving(id, DimensionField::RectangleHeight);
                // Which of a/b owns the dragged corner's x and y.
                let (x_on_a, y_on_a) = match index {
                    0 => (true, true),
                    1 => (false, false),
                    2 => (true, false),
                    _ => (false, true),
                };
                if x_on_a {
                    a.x = along(target.x, b.x, width);
                } else {
                    b.x = along(target.x, a.x, width);
                }
                if y_on_a {
                    a.y = along(target.y, b.y, height);
                } else {
                    b.y = along(target.y, a.y, height);
                }
                if (a.x - b.x).abs() <= 1e-4 || (a.y - b.y).abs() <= 1e-4 {
                    return None;
                }
            }
            Shape::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => {
                let corners = Shape::rotated_rectangle_corners(*center, *width, *height, *rotation);
                let corner = (index + 1) % 4;
                let opposite = corners[(corner + 2) % 4];
                let (sx, sy) = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)][corner];
                let (s, c) = rotation.to_radians().sin_cos();
                let (dx, dy) = (target.x - opposite.x, target.y - opposite.y);
                let new_width = self
                    .driving(id, DimensionField::RectangleWidth)
                    .unwrap_or((dx * c + dy * s) * sx);
                let new_height = self
                    .driving(id, DimensionField::RectangleHeight)
                    .unwrap_or((-dx * s + dy * c) * sy);
                if new_width <= 1e-4 || new_height <= 1e-4 {
                    return None;
                }
                let (hx, hy) = (sx * new_width * 0.5, sy * new_height * 0.5);
                *center = Point {
                    x: opposite.x + c * hx - s * hy,
                    y: opposite.y + s * hx + c * hy,
                };
                *width = new_width;
                *height = new_height;
            }
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            } => {
                let (dx, dy) = (target.x - center.x, target.y - center.y);
                let quarter = (index - 1) as f32 * 90.0;
                let distance = if let Some(fixed) = self.driving(id, DimensionField::Rotation) {
                    *rotation = fixed;
                    let (s, c) = (fixed + quarter).to_radians().sin_cos();
                    dx * c + dy * s
                } else {
                    *rotation = dy.atan2(dx).to_degrees() - quarter;
                    dx.hypot(dy)
                };
                let (axis, field) = if index % 2 == 1 {
                    (rx, DimensionField::RectangleWidth)
                } else {
                    (ry, DimensionField::RectangleHeight)
                };
                if self.driving(id, field).is_none() {
                    if distance <= 1e-4 {
                        return None;
                    }
                    *axis = distance;
                }
            }
            Shape::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            } => {
                let (dx, dy) = (target.x - center.x, target.y - center.y);
                if self.driving(id, DimensionField::CurveRadius).is_none() {
                    let distance = dx.hypot(dy);
                    if distance <= 1e-4 {
                        return None;
                    }
                    *radius = distance;
                }
                if self.driving(id, DimensionField::Rotation).is_none() {
                    *rotation =
                        dy.atan2(dx).to_degrees() - (index - 1) as f32 * 360.0 / *sides as f32;
                }
            }
            _ => return None,
        }
        Some(next)
    }

    pub(super) fn drag_anchor_to(&mut self, index: usize, point: Point) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        let id = drag.id;
        let Some(&(_, origin)) = Self::shape_anchors(&drag.original)
            .iter()
            .find(|(anchor, _)| *anchor == index)
        else {
            return false;
        };
        let target = Point {
            x: origin.x + point.x - drag.start.x,
            y: origin.y + point.y - drag.start.y,
        };
        if !target.x.is_finite() || !target.y.is_finite() {
            return false;
        }
        let Some(position) = self.shapes.iter().position(|shape| shape.id() == id) else {
            return false;
        };
        let Some(next) = self.with_anchor(&self.shapes[position], index, target) else {
            return false;
        };
        let solver_shape = Self::solver_shape(&next);
        let frame_before = self.shapes.clone();
        self.shapes[position] = next;
        let solved = if solver_shape {
            self.solve_anchor_drag(id, index, target)
        } else {
            self.solve_constraints(&[Pin::Shape(id)])
        };
        if solved.is_err() {
            self.shapes = frame_before;
            return false;
        }
        self.shapes != frame_before
    }

    pub(super) fn relation_sources(&self) -> Vec<u32> {
        let followers: HashSet<u32> = self
            .equal_lines
            .keys()
            .chain(self.line_relations.keys())
            .chain(self.equal_radii.keys())
            .chain(self.radius_differences.keys())
            .chain(self.concentric.keys())
            .chain(self.midpoint_links.keys())
            .chain(self.point_curve_links.keys())
            .chain(self.point_links.values())
            .copied()
            .collect();
        self.shapes
            .iter()
            .map(Shape::id)
            .filter(|id| !followers.contains(id))
            .collect()
    }

    /// Solve with the dragged anchor as a strong soft target. A line keeps
    /// its other endpoint. Other relation sources are held first; when that
    /// stops the anchor from following, they are released.
    pub(super) fn solve_anchor_drag(
        &mut self,
        id: u32,
        index: usize,
        target: Point,
    ) -> Result<(), String> {
        let mut explicit = HashSet::new();
        if matches!(
            self.shapes.iter().find(|shape| shape.id() == id),
            Some(Shape::Line { .. })
        ) {
            explicit.insert(Pin::Anchor(id, 1 - index));
        }
        let distance = |document: &Self| {
            document
                .drag_anchors(id)
                .into_iter()
                .find(|(anchor, _)| *anchor == index)
                .map_or(f32::INFINITY, |(_, at)| at.distance(target))
        };
        let start = self.shapes.clone();
        let mut held = explicit.clone();
        held.extend(
            self.relation_sources()
                .into_iter()
                .filter(|source| *source != id)
                .map(Pin::Shape),
        );
        let first = self
            .solve_once(&held, Some((id, index)))
            .map(|()| (self.shapes.clone(), distance(self)));
        if let Ok((_, gap)) = &first {
            if *gap <= 1e-3 {
                return Ok(());
            }
        }
        self.shapes = start.clone();
        let second = self
            .solve_once(&explicit, Some((id, index)))
            .map(|()| (self.shapes.clone(), distance(self)));
        let best = match (first, second) {
            (Ok(a), Ok(b)) => Ok(if b.1 + 1e-4 < a.1 { b } else { a }),
            (Ok(a), Err(_)) => Ok(a),
            (Err(_), second) => second,
        };
        match best {
            Ok((shapes, _)) => {
                self.shapes = shapes;
                Ok(())
            }
            Err(error) => {
                self.shapes = start;
                Err(error)
            }
        }
    }
}
