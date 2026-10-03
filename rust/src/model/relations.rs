use super::*;

/// Center, radius and, for arcs, `(start, sweep)` of a circle or arc.
pub(super) type CircularData = (Point, f32, Option<(f64, f64)>);

impl Document {
    pub fn fixed_anchor_position(&self, id: u32, index: usize) -> Option<Point> {
        self.shapes
            .iter()
            .find(|shape| shape.id() == id)
            .and_then(|shape| match shape {
                Shape::Point { at, .. } if index == 0 => Some(*at),
                Shape::Line { a, b, .. } | Shape::Rectangle { a, b, .. } => match index {
                    0 => Some(*a),
                    1 => Some(*b),
                    _ => None,
                },
                Shape::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep,
                    ..
                } => match index {
                    0 => Some(*center),
                    1 => Some(Shape::arc_point(
                        *center,
                        *radius,
                        *start_angle,
                        *sweep,
                        0.0,
                    )),
                    2 => Some(Shape::arc_point(
                        *center,
                        *radius,
                        *start_angle,
                        *sweep,
                        1.0,
                    )),
                    _ => None,
                },
                Shape::EllipticArc {
                    center,
                    rx,
                    ry,
                    rotation,
                    start_angle,
                    sweep,
                    ..
                } => match index {
                    0 => Some(*center),
                    1 => Some(Shape::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        0.0,
                    )),
                    2 => Some(Shape::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        1.0,
                    )),
                    _ => None,
                },
                Shape::Circle { center, .. } if index == 0 => Some(*center),
                Shape::Ellipse {
                    center,
                    rx,
                    ry,
                    rotation,
                    ..
                } => match index {
                    0 => Some(*center),
                    1..=4 => Some(Shape::ellipse_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        (index - 1) as f32 / 4.0,
                    )),
                    _ => None,
                },
                Shape::RotatedRectangle {
                    center,
                    width,
                    height,
                    rotation,
                    ..
                } => match index {
                    0 => Some(*center),
                    1..=4 => Some(
                        Shape::rotated_rectangle_corners(*center, *width, *height, *rotation)
                            [(index + 1) % 4],
                    ),
                    _ => None,
                },
                Shape::Polygon {
                    center,
                    radius,
                    sides,
                    rotation,
                    ..
                } => match index {
                    0 => Some(*center),
                    n if n <= *sides as usize => Some(Shape::polygon_vertex(
                        *center,
                        *radius,
                        *sides,
                        *rotation,
                        (n - 1) as u8,
                    )),
                    _ => None,
                },
                Shape::Capsule { from, to, .. } => match index {
                    0 => Some(*from),
                    1 => Some(*to),
                    _ => None,
                },
                Shape::Spline { points, .. } => points.get(index).copied(),
                _ => None,
            })
    }

    pub fn toggle_fixed_line_endpoint_at(&mut self, cursor: Point) -> Result<bool, String> {
        let id = self
            .selected
            .ok_or("Select a line endpoint or rectangle corner to fix")?;
        let (a, b) = match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Line { a, b, .. }) | Some(Shape::Rectangle { a, b, .. }) => (*a, *b),
            _ => return Err("Select a line endpoint or rectangle corner to fix".into()),
        };
        if self.normal_links.contains_key(&id) {
            return Err("Line endpoint is driven by Normal".into());
        }
        let (index, position) = if cursor.distance(a) <= cursor.distance(b) {
            (0, a)
        } else {
            (1, b)
        };
        if cursor.distance(position) > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto a line endpoint".into());
        }
        if (index == 1
            && (self.equal_lines.contains_key(&id) || self.line_relations.contains_key(&id)))
            || self
                .line_relations
                .get(&id)
                .is_some_and(|(_, kind)| *kind == LineRelationKind::Collinear)
        {
            return Err("Line end is driven by a relation".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        Ok(if self.fixed_anchors.remove(&(id, index)).is_some() {
            false
        } else {
            self.fixed_anchors.insert((id, index), position);
            true
        })
    }

    pub fn toggle_fixed_arc_anchor_at(&mut self, cursor: Point) -> Result<(usize, bool), String> {
        let id = self.selected.ok_or("Select an arc")?;
        let shape = self
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .ok_or("Selected arc is missing")?;
        if !matches!(shape, Shape::Arc { .. } | Shape::EllipticArc { .. }) {
            return Err("Select an arc".into());
        }
        let tolerance = 8.0 / self.zoom.max(0.2);
        let index = [1, 2]
            .into_iter()
            .filter_map(|index| {
                self.fixed_anchor_position(id, index)
                    .map(|at| (index, at.distance(cursor)))
            })
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(index, _)| index)
            .unwrap_or(0);
        let position = self
            .fixed_anchor_position(id, index)
            .ok_or("Arc anchor is missing")?;
        if index > 0
            && (self.equal_radii.contains_key(&id) || self.equal_ellipses.contains_key(&id))
            && !self.fixed_anchors.contains_key(&(id, index))
        {
            return Err("Arc endpoint is driven by Equal".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let enabled = if self.fixed_anchors.remove(&(id, index)).is_some() {
            false
        } else {
            self.fixed_anchors.insert((id, index), position);
            true
        };
        Ok((index, enabled))
    }

    pub fn toggle_fixed_shape_anchor_at(&mut self, cursor: Point) -> Result<(usize, bool), String> {
        let id = self
            .selected
            .ok_or("Select an ellipse, polygon, rectangle, or slot")?;
        let shape = self
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .ok_or("Selected shape is missing")?;
        let (first, count, center_fallback) = match shape {
            Shape::Ellipse { .. } | Shape::RotatedRectangle { .. } => (1, 4, true),
            Shape::Polygon { sides, .. } => (1, *sides as usize, true),
            Shape::Capsule { .. } => (0, 2, false),
            _ => return Err("Select an ellipse, polygon, rectangle, or slot".into()),
        };
        let tolerance = 8.0 / self.zoom.max(0.2);
        let nearest = (first..first + count)
            .filter_map(|index| {
                self.fixed_anchor_position(id, index)
                    .map(|at| (index, at.distance(cursor)))
            })
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|left, right| left.1.total_cmp(&right.1));
        let index = nearest
            .map(|(index, _)| index)
            .or_else(|| center_fallback.then_some(0))
            .ok_or("Move the pointer onto a shape anchor")?;
        let position = self
            .fixed_anchor_position(id, index)
            .ok_or("Shape anchor is missing")?;
        if index > 0
            && self.equal_ellipses.contains_key(&id)
            && !self.fixed_anchors.contains_key(&(id, index))
        {
            return Err("Ellipse axis is driven by Equal".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let enabled = if self.fixed_anchors.remove(&(id, index)).is_some() {
            false
        } else {
            self.fixed_anchors.insert((id, index), position);
            true
        };
        Ok((index, enabled))
    }

    pub fn toggle_fixed_anchor(&mut self) -> Result<bool, String> {
        let id = self
            .selected
            .ok_or("Select a point or spline handle to fix")?;
        if self.midpoint_links.contains_key(&id) || self.point_curve_links.contains_key(&id) {
            return Err("Remove the point's curve relation before fixing it".into());
        }
        let index = match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Point { .. })
            | Some(Shape::Circle { .. })
            | Some(Shape::Arc { .. })
            | Some(Shape::Ellipse { .. })
            | Some(Shape::EllipticArc { .. })
            | Some(Shape::Polygon { .. })
            | Some(Shape::RotatedRectangle { .. }) => 0,
            Some(Shape::Spline { .. }) => self
                .selected_spline_point
                .ok_or("Select a spline handle to fix")?,
            _ => return Err("Select a point or spline handle to fix".into()),
        };
        let position = self
            .fixed_anchor_position(id, index)
            .ok_or("Anchor is missing")?;
        self.history.push(self.snapshot());
        self.future.clear();
        let enabled = if self.fixed_anchors.remove(&(id, index)).is_some() {
            false
        } else {
            self.fixed_anchors.insert((id, index), position);
            true
        };
        Ok(enabled)
    }

    pub(super) fn has_fixed_anchor(&self, id: u32) -> bool {
        self.fixed_anchors
            .keys()
            .any(|(shape_id, _)| *shape_id == id)
    }

    pub fn toggle_axis_constraint(&mut self, axis: AxisConstraint) -> Result<bool, String> {
        let id = self.selected.ok_or("select a line first")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("selected geometry no longer exists")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("select a line first".into());
        };
        if self.normal_links.contains_key(&id) && self.axis_constraints.get(&id) != Some(&axis) {
            return Err("Line direction is driven by a relation".into());
        }
        if self.axis_constraints.get(&id) == Some(&axis) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.axis_constraints.remove(&id);
            return Ok(false);
        }
        let length = a.distance(b);
        let next = match axis {
            AxisConstraint::Horizontal => Point {
                x: a.x + length.copysign(if b.x == a.x { 1.0 } else { b.x - a.x }),
                y: a.y,
            },
            AxisConstraint::Vertical => Point {
                x: a.x,
                y: a.y + length.copysign(if b.y == a.y { 1.0 } else { b.y - a.y }),
            },
        };
        self.history.push(self.snapshot());
        self.future.clear();
        // Keep the fixed or linked second end as the solver's initial guess
        // when it cannot move; the solve then decides how the line aligns.
        if !self.fixed_anchors.contains_key(&(id, 1)) && !self.point_links.contains_key(&(id, 1)) {
            self.shapes[index] = Shape::Line { id, a, b: next };
        }
        self.axis_constraints.insert(id, axis);
        self.solve_or_rollback(&[Pin::Anchor(id, 0)])?;
        Ok(true)
    }

    pub fn toggle_equal_line_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a line to match")?;
        if self.normal_links.contains_key(&target_id) {
            return Err("Remove Normal before Equal".into());
        }
        if target_id == source_id {
            return Err("Hover a different line".into());
        }
        if self.equal_lines.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.equal_lines.remove(&target_id);
            return Ok(false);
        }
        if self.equal_lines.contains_key(&target_id) {
            return Err("Selected line already has an equal relation".into());
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Source line is missing")?;
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected line is missing")?;
        let (
            Shape::Line {
                a: source_a,
                b: source_b,
                ..
            },
            Shape::Line { a, b, .. },
        ) = (source, &self.shapes[target_index])
        else {
            return Err("Equal requires two lines".into());
        };
        let length = source_a.distance(*source_b);
        let old_length = a.distance(*b);
        if !length.is_finite() || length <= 1e-6 || !old_length.is_finite() || old_length <= 1e-6 {
            return Err("Equal requires nonzero finite lines".into());
        }
        let next_b = Point {
            x: a.x + (b.x - a.x) * length / old_length,
            y: a.y + (b.y - a.y) * length / old_length,
        };
        if !next_b.x.is_finite() || !next_b.y.is_finite() {
            return Err("Equal line length is out of range".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if !self.fixed_anchors.contains_key(&(target_id, 1))
            && !self.point_links.contains_key(&(target_id, 1))
            && let Shape::Line { b, .. } = &mut self.shapes[target_index]
        {
            *b = next_b;
        }
        self.equal_lines.insert(target_id, source_id);
        self.solve_or_rollback(&[Pin::Shape(source_id)])?;
        Ok(true)
    }

    pub(super) fn line_relation_shape(
        source: &Shape,
        target: &Shape,
        kind: LineRelationKind,
    ) -> Option<Shape> {
        let (
            Shape::Line {
                a: source_a,
                b: source_b,
                ..
            },
            Shape::Line { id, a, b },
        ) = (source, target)
        else {
            return None;
        };
        let source_delta = Point {
            x: source_b.x - source_a.x,
            y: source_b.y - source_a.y,
        };
        let target_delta = Point {
            x: b.x - a.x,
            y: b.y - a.y,
        };
        let source_length = source_a.distance(*source_b);
        let target_length = a.distance(*b);
        if !source_length.is_finite()
            || !target_length.is_finite()
            || source_length <= 1e-6
            || target_length <= 1e-6
        {
            return None;
        }
        let direction = match kind {
            LineRelationKind::Parallel | LineRelationKind::Collinear => Point {
                x: source_delta.x / source_length,
                y: source_delta.y / source_length,
            },
            LineRelationKind::Perpendicular => Point {
                x: -source_delta.y / source_length,
                y: source_delta.x / source_length,
            },
            LineRelationKind::Angle(value) => {
                let (sine, cosine) = value.to_radians().sin_cos();
                Point {
                    x: (source_delta.x * cosine - source_delta.y * sine) / source_length,
                    y: (source_delta.x * sine + source_delta.y * cosine) / source_length,
                }
            }
        };
        let sign = if !matches!(kind, LineRelationKind::Angle(_))
            && direction.x * target_delta.x + direction.y * target_delta.y < 0.0
        {
            -1.0
        } else {
            1.0
        };
        let next_a = if kind == LineRelationKind::Collinear {
            let offset = (a.x - source_a.x) * direction.x + (a.y - source_a.y) * direction.y;
            Point {
                x: source_a.x + direction.x * offset,
                y: source_a.y + direction.y * offset,
            }
        } else {
            *a
        };
        let next_b = Point {
            x: next_a.x + direction.x * target_length * sign,
            y: next_a.y + direction.y * target_length * sign,
        };
        (next_a.x.is_finite()
            && next_a.y.is_finite()
            && next_b.x.is_finite()
            && next_b.y.is_finite())
        .then_some(Shape::Line {
            id: *id,
            a: next_a,
            b: next_b,
        })
    }

    pub fn toggle_line_relation_with(
        &mut self,
        source_id: u32,
        kind: LineRelationKind,
    ) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a line")?;
        if self.normal_links.contains_key(&target_id) || self.normal_links.contains_key(&source_id)
        {
            return Err("Remove Normal before line direction relation".into());
        }
        if target_id == source_id {
            return Err("Hover a different line".into());
        }
        if self
            .line_relations
            .get(&target_id)
            .is_some_and(|(source, existing)| {
                *source == source_id && existing.name() == kind.name()
            })
        {
            self.history.push(self.snapshot());
            self.future.clear();
            self.line_relations.remove(&target_id);
            return Ok(false);
        }
        if kind.angle_value().is_some_and(|value| !value.is_finite()) {
            return Err("Angle must be finite".into());
        }
        if self.line_relations.contains_key(&target_id) {
            return Err("Selected line already has a direction relation".into());
        }
        let target_constrained = self.has_fixed_anchor(target_id)
            || self.axis_constraints.contains_key(&target_id)
            || self.point_links.keys().any(|(id, _)| *id == target_id);
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Source line is missing")?;
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected line is missing")?;
        let next = Self::line_relation_shape(source, &self.shapes[target_index], kind)
            .ok_or("Direction relation requires two nonzero finite lines")?;
        self.history.push(self.snapshot());
        self.future.clear();
        if !target_constrained {
            self.shapes[target_index] = next;
        }
        self.line_relations.insert(target_id, (source_id, kind));
        self.solve_or_rollback(&[Pin::Shape(source_id)])?;
        Ok(true)
    }

    pub fn toggle_line_angle_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a target line")?;
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Source line is missing")?;
        let target = self
            .shapes
            .iter()
            .find(|shape| shape.id() == target_id)
            .ok_or("Target line is missing")?;
        let (Shape::Line { a: sa, b: sb, .. }, Shape::Line { a: ta, b: tb, .. }) = (source, target)
        else {
            return Err("Angle requires two lines".into());
        };
        let source_delta = Point {
            x: sb.x - sa.x,
            y: sb.y - sa.y,
        };
        let target_delta = Point {
            x: tb.x - ta.x,
            y: tb.y - ta.y,
        };
        if sa.distance(*sb) <= 1e-6 || ta.distance(*tb) <= 1e-6 {
            return Err("Angle requires nonzero lines".into());
        }
        let value = (source_delta.x * target_delta.y - source_delta.y * target_delta.x)
            .atan2(source_delta.x * target_delta.x + source_delta.y * target_delta.y)
            .to_degrees();
        self.toggle_line_relation_with(source_id, LineRelationKind::Angle(value))
    }

    pub fn set_line_relation_angle(&mut self, value: f32) -> Result<(), String> {
        if !value.is_finite() {
            return Err("Angle must be finite".into());
        }
        let target_id = self.selected.ok_or("Select an angle target line")?;
        let &(source_id, LineRelationKind::Angle(old_value)) = self
            .line_relations
            .get(&target_id)
            .ok_or("Selected line has no angle relation")?
        else {
            return Err("Selected line has no angle relation".into());
        };
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Angle source is missing")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Angle target is missing")?;
        let next =
            Self::line_relation_shape(source, &self.shapes[index], LineRelationKind::Angle(value))
                .ok_or("Angle target is invalid")?;
        if (old_value - value).abs() <= 1e-6 && self.shapes[index] == next {
            return Ok(());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = next;
        self.line_relations
            .insert(target_id, (source_id, LineRelationKind::Angle(value)));
        self.solve_or_rollback(&[Pin::Shape(source_id)])
    }

    pub fn toggle_equal_radius_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a circle or arc to match")?;
        if self.internal_tangent_member(target_id) || self.internal_tangent_member(source_id) {
            return Err("Remove internal tangent before Equal radius".into());
        }
        if target_id == source_id {
            return Err("Hover a different circle or arc".into());
        }
        if self.equal_radii.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.equal_radii.remove(&target_id);
            return Ok(false);
        }
        if self.equal_radii.iter().any(|(&target, &source)| {
            target == target_id || source == target_id || target == source_id || source == source_id
        }) {
            return Err("A circle or arc already has an equal relation".into());
        }
        if self
            .radius_differences
            .iter()
            .any(|(&target, &(source, _))| {
                [source, target].contains(&source_id) || [source, target].contains(&target_id)
            })
        {
            return Err("A circle already has a radial distance relation".into());
        }
        let source_radius = match self.shapes.iter().find(|shape| shape.id() == source_id) {
            Some(Shape::Circle { radius, .. } | Shape::Arc { radius, .. }) => *radius,
            _ => return Err("Equal radius requires circles or arcs".into()),
        };
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected geometry is missing")?;
        if !matches!(
            self.shapes[target_index],
            Shape::Circle { .. } | Shape::Arc { .. }
        ) {
            return Err("Equal radius requires circles or arcs".into());
        }
        if !source_radius.is_finite() || source_radius <= 1e-6 {
            return Err("Equal requires a positive finite radius".into());
        }
        if matches!(self.shapes[target_index], Shape::Arc { .. })
            && ((1..=2).any(|anchor| self.fixed_anchors.contains_key(&(target_id, anchor))))
        {
            return Err("Target arc endpoint is fixed".into());
        }
        if self
            .driving_dimensions
            .contains_key(&(target_id, DimensionField::CircleDiameter))
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::CurveRadius))
        {
            return Err("Target radius is constrained".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        match &mut self.shapes[target_index] {
            Shape::Circle { radius, .. } | Shape::Arc { radius, .. } => *radius = source_radius,
            _ => unreachable!(),
        }
        self.equal_radii.insert(target_id, source_id);
        Ok(true)
    }

    pub fn toggle_radius_difference_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a target circle")?;
        if self.internal_tangent_member(target_id) || self.internal_tangent_member(source_id) {
            return Err("Remove internal tangent before radial distance".into());
        }
        if source_id == target_id {
            return Err("Hover another circle".into());
        }
        if self
            .radius_differences
            .get(&target_id)
            .is_some_and(|(source, _)| *source == source_id)
        {
            self.history.push(self.snapshot());
            self.future.clear();
            self.radius_differences.remove(&target_id);
            return Ok(false);
        }
        if self
            .radius_differences
            .iter()
            .any(|(&target, &(source, _))| {
                [source, target].contains(&source_id) || [source, target].contains(&target_id)
            })
            || self.equal_radii.iter().any(|(&target, &source)| {
                [source, target].contains(&source_id) || [source, target].contains(&target_id)
            })
        {
            return Err("A circle already has a radius relation".into());
        }
        let source_radius = match self.shapes.iter().find(|shape| shape.id() == source_id) {
            Some(Shape::Circle { radius, .. }) => *radius,
            _ => return Err("Radial distance requires two circles".into()),
        };
        let target_radius = match self.shapes.iter().find(|shape| shape.id() == target_id) {
            Some(Shape::Circle { radius, .. }) => *radius,
            _ => return Err("Radial distance requires two circles".into()),
        };
        if !source_radius.is_finite()
            || !target_radius.is_finite()
            || source_radius <= 1e-6
            || target_radius <= 1e-6
        {
            return Err("Circle radii must be positive and finite".into());
        }
        if self
            .driving_dimensions
            .contains_key(&(target_id, DimensionField::CircleDiameter))
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::CurveRadius))
        {
            return Err("Target radius already has a driving dimension".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.radius_differences
            .insert(target_id, (source_id, source_radius - target_radius));
        Ok(true)
    }

    pub fn set_radius_difference(&mut self, value: f32) -> Result<(), String> {
        if !value.is_finite() {
            return Err("Radial distance must be finite".into());
        }
        let target_id = self.selected.ok_or("Select a radial distance target")?;
        let &(source_id, old_value) = self
            .radius_differences
            .get(&target_id)
            .ok_or("Selected circle has no radial distance")?;
        let source_radius = match self.shapes.iter().find(|shape| shape.id() == source_id) {
            Some(Shape::Circle { radius, .. }) => *radius,
            _ => return Err("Source circle is missing".into()),
        };
        let target_radius = source_radius - value;
        if !target_radius.is_finite() || target_radius <= 1e-6 {
            return Err("Target radius must stay positive".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Target circle is missing")?;
        if !matches!(self.shapes[index], Shape::Circle { .. }) {
            return Err("Target is not a circle".into());
        }
        if (old_value - value).abs() <= 1e-6 {
            return Ok(());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if let Shape::Circle { radius, .. } = &mut self.shapes[index] {
            *radius = target_radius;
        }
        self.radius_differences
            .insert(target_id, (source_id, value));
        Ok(())
    }

    pub(super) fn circular_data(shape: &Shape) -> Option<CircularData> {
        match shape {
            Shape::Circle { center, radius, .. } => Some((*center, *radius, None)),
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => Some((*center, *radius, Some((*start_angle, *sweep)))),
            _ => None,
        }
    }

    pub(super) fn internal_tangent_target(outer: &Shape, inner: &Shape) -> Option<Shape> {
        let (outer_center, outer_radius, outer_arc) = Self::circular_data(outer)?;
        let (inner_center, inner_radius, inner_arc) = Self::circular_data(inner)?;
        let distance = outer_radius - inner_radius;
        if !distance.is_finite() || distance <= 1e-6 {
            return None;
        }
        let preferred = ((inner_center.y - outer_center.y) as f64)
            .atan2((inner_center.x - outer_center.x) as f64);
        let mut candidates = vec![preferred];
        for (start, sweep) in [outer_arc, inner_arc].into_iter().flatten() {
            candidates.extend([
                start,
                start + sweep,
                start + sweep * 0.5,
                start + sweep * 1e-5,
                start + sweep * (1.0 - 1e-5),
            ]);
        }
        candidates.sort_by(|a, b| {
            let error = |angle: f64| {
                (angle - preferred)
                    .sin()
                    .atan2((angle - preferred).cos())
                    .abs()
            };
            error(*a).total_cmp(&error(*b))
        });
        for angle in candidates {
            let unit = Point {
                x: angle.cos() as f32,
                y: angle.sin() as f32,
            };
            let center = Point {
                x: outer_center.x + unit.x * distance,
                y: outer_center.y + unit.y * distance,
            };
            let contact = Point {
                x: outer_center.x + unit.x * outer_radius,
                y: outer_center.y + unit.y * outer_radius,
            };
            if ![center.x, center.y, contact.x, contact.y]
                .into_iter()
                .all(f32::is_finite)
            {
                continue;
            }
            if outer_arc
                .is_some_and(|(start, sweep)| !on_arc_sweep(contact, outer_center, start, sweep))
                || inner_arc
                    .is_some_and(|(start, sweep)| !on_arc_sweep(contact, center, start, sweep))
            {
                continue;
            }
            let delta = Point {
                x: center.x - inner_center.x,
                y: center.y - inner_center.y,
            };
            return Some(inner.translated(delta));
        }
        None
    }

    pub(super) fn external_tangent_target(source: &Shape, target: &Shape) -> Option<Shape> {
        let line_data = |shape: &Shape| match shape {
            Shape::Line { a, b, .. } => {
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let length = (dx * dx + dy * dy).sqrt();
                (length > 1e-6).then_some((
                    *a,
                    Point {
                        x: -dy / length,
                        y: dx / length,
                    },
                ))
            }
            _ => None,
        };
        let ellipse_data = |shape: &Shape| match shape {
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            } => Some((*center, *rx, *ry, *rotation, None)),
            Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                ..
            } => Some((*center, *rx, *ry, *rotation, Some((*start_angle, *sweep)))),
            _ => None,
        };
        let ellipse_support = |rx: f32, ry: f32, rotation: f32, normal: Point| {
            let (sin, cos) = rotation.to_radians().sin_cos();
            let nx = normal.x * cos + normal.y * sin;
            let ny = -normal.x * sin + normal.y * cos;
            let extent = ((rx * nx).powi(2) + (ry * ny).powi(2)).sqrt();
            if extent <= 1e-6 {
                return None;
            }
            let x = rx * rx * nx / extent;
            let y = ry * ry * ny / extent;
            Some((
                extent,
                Point {
                    x: x * cos - y * sin,
                    y: x * sin + y * cos,
                },
            ))
        };
        let arc_contact = |center: Point,
                           rx: f32,
                           ry: f32,
                           rotation: f32,
                           arc: Option<(f64, f64)>,
                           contact: Point| {
            arc.is_none_or(|(start, sweep)| {
                let angle = Shape::ellipse_parameter_angle(center, rx, ry, rotation, contact);
                Shape::elliptic_arc_progress(angle, start, sweep) <= 1.0 + 1e-5
            })
        };
        if let (Some((line_start, normal)), Some((center, rx, ry, rotation, arc))) =
            (line_data(source), ellipse_data(target))
        {
            let (extent, support) = ellipse_support(rx, ry, rotation, normal)?;
            let signed =
                (center.x - line_start.x) * normal.x + (center.y - line_start.y) * normal.y;
            for side in [
                if signed < 0.0 { -1.0 } else { 1.0 },
                if signed < 0.0 { 1.0 } else { -1.0 },
            ] {
                let translation = side * extent - signed;
                let next_center = Point {
                    x: center.x + normal.x * translation,
                    y: center.y + normal.y * translation,
                };
                let contact = Point {
                    x: next_center.x - side * support.x,
                    y: next_center.y - side * support.y,
                };
                if !arc_contact(next_center, rx, ry, rotation, arc, contact) {
                    continue;
                }
                return Some(target.translated(Point {
                    x: normal.x * translation,
                    y: normal.y * translation,
                }));
            }
            return None;
        }
        if let (Some((center, rx, ry, rotation, arc)), Some((line_start, normal))) =
            (ellipse_data(source), line_data(target))
        {
            let (extent, support) = ellipse_support(rx, ry, rotation, normal)?;
            let signed =
                (center.x - line_start.x) * normal.x + (center.y - line_start.y) * normal.y;
            for side in [
                if signed < 0.0 { -1.0 } else { 1.0 },
                if signed < 0.0 { 1.0 } else { -1.0 },
            ] {
                let contact = Point {
                    x: center.x - side * support.x,
                    y: center.y - side * support.y,
                };
                if !arc_contact(center, rx, ry, rotation, arc, contact) {
                    continue;
                }
                let translation = signed - side * extent;
                return Some(target.translated(Point {
                    x: normal.x * translation,
                    y: normal.y * translation,
                }));
            }
            return None;
        }
        if let (Some((line_start, normal)), Some((center, radius, arc))) =
            (line_data(source), Self::circular_data(target))
        {
            let signed =
                (center.x - line_start.x) * normal.x + (center.y - line_start.y) * normal.y;
            for side in [
                if signed < 0.0 { -1.0 } else { 1.0 },
                if signed < 0.0 { 1.0 } else { -1.0 },
            ] {
                let translation = side * radius - signed;
                let next_center = Point {
                    x: center.x + normal.x * translation,
                    y: center.y + normal.y * translation,
                };
                let contact = Point {
                    x: next_center.x - normal.x * side * radius,
                    y: next_center.y - normal.y * side * radius,
                };
                if arc
                    .is_some_and(|(start, sweep)| !on_arc_sweep(contact, next_center, start, sweep))
                {
                    continue;
                }
                return Some(target.translated(Point {
                    x: normal.x * translation,
                    y: normal.y * translation,
                }));
            }
            return None;
        }
        if let (Some((center, radius, arc)), Some((line_start, normal))) =
            (Self::circular_data(source), line_data(target))
        {
            let signed =
                (center.x - line_start.x) * normal.x + (center.y - line_start.y) * normal.y;
            for side in [
                if signed < 0.0 { -1.0 } else { 1.0 },
                if signed < 0.0 { 1.0 } else { -1.0 },
            ] {
                let contact = Point {
                    x: center.x - normal.x * side * radius,
                    y: center.y - normal.y * side * radius,
                };
                if arc.is_some_and(|(start, sweep)| !on_arc_sweep(contact, center, start, sweep)) {
                    continue;
                }
                let translation = signed - side * radius;
                return Some(target.translated(Point {
                    x: normal.x * translation,
                    y: normal.y * translation,
                }));
            }
            return None;
        }
        let (source_center, source_radius, source_arc) = Self::circular_data(source)?;
        let (target_center, target_radius, target_arc) = Self::circular_data(target)?;
        let distance = source_radius + target_radius;
        if !distance.is_finite() || distance <= 1e-6 {
            return None;
        }
        let preferred = ((target_center.y - source_center.y) as f64)
            .atan2((target_center.x - source_center.x) as f64);
        let mut candidates = vec![preferred];
        for (arc, offset) in [(source_arc, 0.0), (target_arc, -std::f64::consts::PI)] {
            if let Some((start, sweep)) = arc {
                candidates.extend([
                    start + offset,
                    start + sweep + offset,
                    start + sweep * 0.5 + offset,
                    start + sweep * 1e-5 + offset,
                    start + sweep * (1.0 - 1e-5) + offset,
                ]);
            }
        }
        candidates.sort_by(|a, b| {
            let error = |angle: f64| {
                (angle - preferred)
                    .sin()
                    .atan2((angle - preferred).cos())
                    .abs()
            };
            error(*a).total_cmp(&error(*b))
        });
        for angle in candidates {
            let unit = Point {
                x: angle.cos() as f32,
                y: angle.sin() as f32,
            };
            let center = Point {
                x: source_center.x + unit.x * distance,
                y: source_center.y + unit.y * distance,
            };
            let contact = Point {
                x: source_center.x + unit.x * source_radius,
                y: source_center.y + unit.y * source_radius,
            };
            if ![center.x, center.y, contact.x, contact.y]
                .into_iter()
                .all(f32::is_finite)
            {
                continue;
            }
            if source_arc
                .is_some_and(|(start, sweep)| !on_arc_sweep(contact, source_center, start, sweep))
                || target_arc
                    .is_some_and(|(start, sweep)| !on_arc_sweep(contact, center, start, sweep))
            {
                continue;
            }
            return Some(target.translated(Point {
                x: center.x - target_center.x,
                y: center.y - target_center.y,
            }));
        }
        None
    }

    pub(super) fn internal_tangent_member(&self, id: u32) -> bool {
        self.internal_tangents.contains_key(&id)
            || self.internal_tangents.values().any(|source| *source == id)
            || self.external_tangents.contains_key(&id)
            || self.external_tangents.values().any(|source| *source == id)
    }

    pub fn curve_midpoint(shape: &Shape) -> Option<Point> {
        match shape {
            Shape::Line { a, b, .. } => Some(Point {
                x: (a.x + b.x) * 0.5,
                y: (a.y + b.y) * 0.5,
            }),
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => Some(Shape::arc_point(
                *center,
                *radius,
                *start_angle,
                *sweep,
                0.5,
            )),
            Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                ..
            } => Some(Shape::elliptic_arc_point(
                *center,
                *rx,
                *ry,
                *rotation,
                *start_angle,
                *sweep,
                0.5,
            )),
            _ => None,
        }
    }

    pub fn closest_curve_point(shape: &Shape, point: Point) -> Option<Point> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return None;
        }
        match shape {
            Shape::Line { a, b, .. } => {
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let length_squared = dx * dx + dy * dy;
                if length_squared <= 1e-12 {
                    return None;
                }
                let t = (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared)
                    .clamp(0.0, 1.0);
                Some(Point {
                    x: a.x + t * dx,
                    y: a.y + t * dy,
                })
            }
            Shape::Circle { center, radius, .. } | Shape::Arc { center, radius, .. } => {
                let angle = ((point.y - center.y) as f64).atan2((point.x - center.x) as f64);
                let radial = Point {
                    x: center.x + *radius * angle.cos() as f32,
                    y: center.y + *radius * angle.sin() as f32,
                };
                if let Shape::Arc {
                    start_angle, sweep, ..
                } = shape
                {
                    if on_arc_sweep(radial, *center, *start_angle, *sweep) {
                        return Some(radial);
                    }
                    let start = Shape::arc_point(*center, *radius, *start_angle, *sweep, 0.0);
                    let end = Shape::arc_point(*center, *radius, *start_angle, *sweep, 1.0);
                    Some(if point.distance(start) <= point.distance(end) {
                        start
                    } else {
                        end
                    })
                } else {
                    Some(radial)
                }
            }
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
            } => {
                if *rx <= 0.0 || *ry <= 0.0 {
                    return None;
                }
                let (start, sweep) = match shape {
                    Shape::EllipticArc {
                        start_angle, sweep, ..
                    } => (*start_angle, *sweep),
                    _ => (0.0, std::f64::consts::TAU),
                };
                let samples = 256_usize;
                let at = |t: f64| {
                    Shape::elliptic_arc_point(*center, *rx, *ry, *rotation, start, sweep, t)
                };
                let distance = |t: f64| {
                    let p = at(t);
                    (p.x - point.x).powi(2) + (p.y - point.y).powi(2)
                };
                let mut best_i = 0;
                let mut best = f32::INFINITY;
                for i in 0..=samples {
                    let d = distance(i as f64 / samples as f64);
                    if d < best {
                        best = d;
                        best_i = i;
                    }
                }
                let mut left = (best_i.saturating_sub(1)) as f64 / samples as f64;
                let mut right = (best_i + 1).min(samples) as f64 / samples as f64;
                for _ in 0..32 {
                    let a = left + (right - left) / 3.0;
                    let b = right - (right - left) / 3.0;
                    if distance(a) <= distance(b) {
                        right = b;
                    } else {
                        left = a;
                    }
                }
                Some(at((left + right) * 0.5))
            }
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                let curves = spline::beziers(*mode, points, *closed).ok()?;
                let hit = spline::nearest(&curves, point).ok()?;
                Some(spline::point(&curves[hit.segment], hit.t))
            }
            _ => None,
        }
    }

    pub fn normal_line_target(source: &Shape, target: &Shape, endpoint: u8) -> Option<Shape> {
        let Shape::Line { id, a, b } = target else {
            return None;
        };
        if endpoint > 1 {
            return None;
        }
        let (anchor, other) = if endpoint == 0 { (*a, *b) } else { (*b, *a) };
        let length = anchor.distance(other);
        if !length.is_finite() || length <= 1e-6 {
            return None;
        }
        let contact = Self::closest_curve_point(source, anchor)?;
        let normal = match source {
            Shape::Line { a, b, .. } => Point {
                x: a.y - b.y,
                y: b.x - a.x,
            },
            Shape::Circle { center, .. } | Shape::Arc { center, .. } => Point {
                x: contact.x - center.x,
                y: contact.y - center.y,
            },
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
            } => {
                let (sin, cos) = rotation.to_radians().sin_cos();
                let dx = contact.x - center.x;
                let dy = contact.y - center.y;
                let x = (dx * cos + dy * sin) / (rx * rx);
                let y = (-dx * sin + dy * cos) / (ry * ry);
                Point {
                    x: x * cos - y * sin,
                    y: x * sin + y * cos,
                }
            }
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                let curves = spline::beziers(*mode, points, *closed).ok()?;
                let hit = spline::nearest(&curves, contact).ok()?;
                let c = curves.get(hit.segment)?;
                let t = hit.t;
                let tangent = Point {
                    x: 3.0 * (1.0 - t).powi(2) * (c[1].x - c[0].x)
                        + 6.0 * (1.0 - t) * t * (c[2].x - c[1].x)
                        + 3.0 * t.powi(2) * (c[3].x - c[2].x),
                    y: 3.0 * (1.0 - t).powi(2) * (c[1].y - c[0].y)
                        + 6.0 * (1.0 - t) * t * (c[2].y - c[1].y)
                        + 3.0 * t.powi(2) * (c[3].y - c[2].y),
                };
                Point {
                    x: -tangent.y,
                    y: tangent.x,
                }
            }
            _ => return None,
        };
        let magnitude = (normal.x * normal.x + normal.y * normal.y).sqrt();
        if !magnitude.is_finite() || magnitude <= 1e-8 {
            return None;
        }
        let mut unit = Point {
            x: normal.x / magnitude,
            y: normal.y / magnitude,
        };
        if (other.x - anchor.x) * unit.x + (other.y - anchor.y) * unit.y < 0.0 {
            unit.x = -unit.x;
            unit.y = -unit.y;
        }
        let other = Point {
            x: contact.x + length * unit.x,
            y: contact.y + length * unit.y,
        };
        if ![other.x, other.y, contact.x, contact.y]
            .into_iter()
            .all(f32::is_finite)
        {
            return None;
        }
        Some(if endpoint == 0 {
            Shape::Line {
                id: *id,
                a: contact,
                b: other,
            }
        } else {
            Shape::Line {
                id: *id,
                a: other,
                b: contact,
            }
        })
    }

    pub fn toggle_point_on_curve_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a sketch point")?;
        if target_id == source_id {
            return Err("Hover another curve".into());
        }
        if self.point_curve_links.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.point_curve_links.remove(&target_id);
            return Ok(false);
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected point is missing")?;
        let Shape::Point { at, .. } = self.shapes[index] else {
            return Err("Point on curve target must be a sketch point".into());
        };
        let projected = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .and_then(|shape| Self::closest_curve_point(shape, at))
            .ok_or("Point on curve source must be a supported curve")?;
        if self.fixed_anchors.contains_key(&(target_id, 0))
            || self
                .point_links
                .values()
                .any(|point_id| *point_id == target_id)
            || self.midpoint_links.contains_key(&target_id)
            || self.point_curve_links.contains_key(&target_id)
        {
            return Err("Point already has a driving relation".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::Point {
            id: target_id,
            at: projected,
        };
        self.point_curve_links.insert(target_id, source_id);
        Ok(true)
    }

    pub fn toggle_normal_with(&mut self, source_id: u32, cursor: Point) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a line")?;
        if target_id == source_id {
            return Err("Hover a separate curve".into());
        }
        if self
            .normal_links
            .get(&target_id)
            .is_some_and(|(source, _)| *source == source_id)
        {
            self.history.push(self.snapshot());
            self.future.clear();
            self.normal_links.remove(&target_id);
            return Ok(false);
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected line is missing")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("Normal target must be a line".into());
        };
        let endpoint = if cursor.distance(a) <= cursor.distance(b) {
            0
        } else {
            1
        };
        if self.has_fixed_anchor(target_id)
            || self.normal_links.contains_key(&target_id)
            || self
                .normal_links
                .values()
                .any(|(source, _)| *source == target_id)
            || self.normal_links.contains_key(&source_id)
            || self
                .midpoint_links
                .values()
                .any(|source| *source == target_id)
            || self
                .point_curve_links
                .values()
                .any(|source| *source == target_id)
            || self.line_relations.contains_key(&target_id)
            || self.equal_lines.contains_key(&target_id)
            || self.external_tangents.contains_key(&target_id)
            || self.point_links.keys().any(|(id, _)| *id == target_id)
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::Rotation))
        {
            return Err("Line already has a conflicting relation".into());
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Hovered curve is missing")?;
        let next = Self::normal_line_target(source, &self.shapes[index], endpoint)
            .ok_or("No valid normal contact for this curve")?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = next;
        self.normal_links.insert(target_id, (source_id, endpoint));
        Ok(true)
    }

    pub fn toggle_midpoint_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a sketch point")?;
        if target_id == source_id {
            return Err("Hover another line or arc".into());
        }
        if self.midpoint_links.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.midpoint_links.remove(&target_id);
            return Ok(false);
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected point is missing")?;
        if !matches!(self.shapes[index], Shape::Point { .. }) {
            return Err("Midpoint target must be a sketch point".into());
        }
        let midpoint = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .and_then(Self::curve_midpoint)
            .ok_or("Midpoint source must be a line or arc")?;
        if self.fixed_anchors.contains_key(&(target_id, 0))
            || self
                .point_links
                .values()
                .any(|point_id| *point_id == target_id)
        {
            return Err("Point already has a fixed or coincident relation".into());
        }
        if self.midpoint_links.contains_key(&target_id)
            || self.point_curve_links.contains_key(&target_id)
        {
            return Err("Point already has a curve relation".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::Point {
            id: target_id,
            at: midpoint,
        };
        self.midpoint_links.insert(target_id, source_id);
        Ok(true)
    }

    pub fn toggle_external_tangent_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a line or curve")?;
        if self.normal_links.contains_key(&target_id) || self.normal_links.contains_key(&source_id)
        {
            return Err("Remove Normal before Tangent".into());
        }
        if target_id == source_id {
            return Err("Hover another line or curve".into());
        }
        if self.external_tangents.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.external_tangents.remove(&target_id);
            return Ok(false);
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Hovered curve is missing")?;
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected curve is missing")?;
        if self.internal_tangent_member(target_id) || self.internal_tangent_member(source_id) {
            return Err("A curve already has a tangent relation".into());
        }
        if self.has_fixed_anchor(target_id) {
            return Err("Target curve has a fixed anchor".into());
        }
        let next = Self::external_tangent_target(source, &self.shapes[target_index])
            .ok_or("No valid external tangent contact for these curves")?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[target_index] = next;
        self.external_tangents.insert(target_id, source_id);
        Ok(true)
    }

    pub fn toggle_internal_tangent_with(&mut self, other_id: u32) -> Result<bool, String> {
        let selected_id = self.selected.ok_or("Select a circle or arc")?;
        if selected_id == other_id {
            return Err("Hover another circle or arc".into());
        }
        let selected = self
            .shapes
            .iter()
            .find(|shape| shape.id() == selected_id)
            .ok_or("Selected curve is missing")?;
        let other = self
            .shapes
            .iter()
            .find(|shape| shape.id() == other_id)
            .ok_or("Hovered curve is missing")?;
        let (_, selected_radius, _) =
            Self::circular_data(selected).ok_or("Internal tangent requires circles or arcs")?;
        let (_, other_radius, _) =
            Self::circular_data(other).ok_or("Internal tangent requires circles or arcs")?;
        let (outer_id, inner_id) = if selected_radius >= other_radius {
            (selected_id, other_id)
        } else {
            (other_id, selected_id)
        };
        if self.internal_tangents.get(&inner_id) == Some(&outer_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.internal_tangents.remove(&inner_id);
            return Ok(false);
        }
        if self.equal_radii.iter().any(|(&target, &source)| {
            [target, source].contains(&selected_id) || [target, source].contains(&other_id)
        }) || self
            .radius_differences
            .iter()
            .any(|(&target, &(source, _))| {
                [target, source].contains(&selected_id) || [target, source].contains(&other_id)
            })
            || self.concentric_partner(selected_id).is_some()
            || self.concentric_partner(other_id).is_some()
        {
            return Err("Remove conflicting radius or center relations first".into());
        }
        if self.internal_tangent_member(selected_id) || self.internal_tangent_member(other_id) {
            return Err("A curve already has a tangent relation".into());
        }
        if self.has_fixed_anchor(inner_id) {
            return Err("Inner curve has a fixed anchor".into());
        }
        let outer = self
            .shapes
            .iter()
            .find(|shape| shape.id() == outer_id)
            .unwrap();
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == inner_id)
            .unwrap();
        let next = Self::internal_tangent_target(outer, &self.shapes[index])
            .ok_or("No valid internal tangent contact for these curves")?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = next;
        self.internal_tangents.insert(inner_id, outer_id);
        Ok(true)
    }

    pub fn toggle_equal_ellipse_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select an ellipse or elliptic arc")?;
        if target_id == source_id {
            return Err("Hover a different ellipse or elliptic arc".into());
        }
        if self.equal_ellipses.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.equal_ellipses.remove(&target_id);
            return Ok(false);
        }
        if self.shared_ellipse_partner(target_id).is_some()
            || self.shared_ellipse_partner(source_id).is_some()
        {
            return Err("Shared ellipse relation already controls these curves".into());
        }
        if self.equal_ellipses.iter().any(|(&target, &source)| {
            target == target_id || source == target_id || target == source_id || source == source_id
        }) {
            return Err("An ellipse already has an equal relation".into());
        }
        let source_axes = match self.shapes.iter().find(|shape| shape.id() == source_id) {
            Some(Shape::Ellipse { rx, ry, .. } | Shape::EllipticArc { rx, ry, .. }) => (*rx, *ry),
            _ => return Err("Equal requires ellipses or elliptic arcs".into()),
        };
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected ellipse is missing")?;
        if !matches!(
            self.shapes[target_index],
            Shape::Ellipse { .. } | Shape::EllipticArc { .. }
        ) {
            return Err("Equal requires ellipses or elliptic arcs".into());
        }
        if !source_axes.0.is_finite()
            || !source_axes.1.is_finite()
            || source_axes.0 <= 1e-6
            || source_axes.1 <= 1e-6
        {
            return Err("Equal requires positive finite axes".into());
        }
        if self
            .fixed_anchors
            .keys()
            .any(|(id, anchor)| *id == target_id && *anchor > 0)
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::RectangleWidth))
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::RectangleHeight))
        {
            return Err("Target ellipse axes are constrained".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        match &mut self.shapes[target_index] {
            Shape::Ellipse { rx, ry, .. } | Shape::EllipticArc { rx, ry, .. } => {
                *rx = source_axes.0;
                *ry = source_axes.1;
            }
            _ => unreachable!(),
        }
        self.equal_ellipses.insert(target_id, source_id);
        Ok(true)
    }

    pub(super) fn concentric_partner(&self, id: u32) -> Option<u32> {
        self.concentric.get(&id).copied().or_else(|| {
            self.concentric
                .iter()
                .find_map(|(&target, &source)| (source == id).then_some(target))
        })
    }

    pub(super) fn shared_ellipse_partner(&self, id: u32) -> Option<u32> {
        self.shared_ellipse_partners(id).into_iter().next()
    }

    pub(super) fn shared_ellipse_partners(&self, id: u32) -> Vec<u32> {
        let root = self.shared_ellipses.get(&id).copied().unwrap_or(id);
        let mut group: Vec<_> = self
            .shared_ellipses
            .iter()
            .filter_map(|(&target, &source)| (source == root && target != id).then_some(target))
            .collect();
        if root != id {
            group.push(root);
        }
        group
    }

    pub fn toggle_concentric_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a circular or elliptic curve")?;
        if self.internal_tangent_member(target_id) || self.internal_tangent_member(source_id) {
            return Err("Remove internal tangent before Concentric".into());
        }
        if target_id == source_id {
            return Err("Hover a different curve".into());
        }
        if self.concentric.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.concentric.remove(&target_id);
            return Ok(false);
        }
        if self.shared_ellipse_partner(target_id).is_some()
            || self.shared_ellipse_partner(source_id).is_some()
        {
            return Err("Shared ellipse relation already controls these curves".into());
        }
        if self.concentric_partner(target_id).is_some()
            || self.concentric_partner(source_id).is_some()
        {
            return Err("A curve already has a concentric relation".into());
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Source curve is missing")?;
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected curve is missing")?;
        let valid = |shape: &Shape| {
            matches!(
                shape,
                Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
            )
        };
        if !valid(source) || !valid(&self.shapes[target_index]) {
            return Err("Concentric requires circular or elliptic curves".into());
        }
        let source_center = source.center();
        let target_center = self.shapes[target_index].center();
        let delta = Point {
            x: source_center.x - target_center.x,
            y: source_center.y - target_center.y,
        };
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return Err("Concentric center is invalid".into());
        }
        if self.has_fixed_anchor(target_id) && target_center.distance(source_center) > 1e-6 {
            return Err("Target curve has a fixed anchor".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[target_index] = self.shapes[target_index].translated(delta);
        self.concentric.insert(target_id, source_id);
        Ok(true)
    }
}
