use super::*;

impl Document {
    pub fn delete_selected(&mut self) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let Some(index) = self.shapes.iter().position(|shape| shape.id() == id) else {
            return false;
        };
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes.remove(index);
        self.equal_lines
            .retain(|target, source| *target != id && *source != id);
        self.line_relations
            .retain(|target, (source, _)| *target != id && *source != id);
        self.equal_radii
            .retain(|target, source| *target != id && *source != id);
        self.radius_differences
            .retain(|target, (source, _)| *target != id && *source != id);
        self.internal_tangents
            .retain(|target, source| *target != id && *source != id);
        self.external_tangents
            .retain(|target, source| *target != id && *source != id);
        self.midpoint_links
            .retain(|target, source| *target != id && *source != id);
        self.point_curve_links
            .retain(|target, source| *target != id && *source != id);
        self.normal_links
            .retain(|target, (source, _)| *target != id && *source != id);
        self.endpoint_tangents
            .retain(|(target, _), (source, _, _)| *target != id && *source != id);
        self.endpoint_joins
            .retain(|(line, _), (other, _)| *line != id && *other != id);
        self.chamfers
            .retain(|line, spec| *line != id && spec.a.0 != id && spec.b.0 != id);
        self.equal_ellipses
            .retain(|target, source| *target != id && *source != id);
        self.shared_ellipses
            .retain(|target, source| *target != id && *source != id);
        self.concentric
            .retain(|target, source| *target != id && *source != id);
        self.construction.remove(&id);
        self.axis_constraints.remove(&id);
        self.point_links
            .retain(|(line_id, _), point_id| *line_id != id && *point_id != id);
        self.driving_dimensions
            .retain(|(shape_id, _), _| *shape_id != id);
        self.fixed_anchors
            .retain(|(shape_id, _), _| *shape_id != id);
        self.selected = None;
        true
    }

    pub fn split_selected_line(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select a line to split")?;
        if self.equal_lines.contains_key(&id)
            || self.equal_lines.values().any(|source| *source == id)
            || self.line_relations.contains_key(&id)
            || self
                .line_relations
                .values()
                .any(|(source, _)| *source == id)
        {
            return Err("Remove Equal before splitting the line".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected line is missing")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("Select a line to split".into());
        };
        if !click.x.is_finite() || !click.y.is_finite() {
            return Err("Split position must be finite".into());
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length_squared = dx * dx + dy * dy;
        if length_squared <= 1e-12 {
            return Err("Cannot split a zero-length line".into());
        }
        let t = ((click.x - a.x) * dx + (click.y - a.y) * dy) / length_squared;
        let split = Point {
            x: a.x + t * dx,
            y: a.y + t * dy,
        };
        let minimum = 2.0 / self.zoom.max(0.2);
        if !(0.0..1.0).contains(&t) || a.distance(split) <= minimum || b.distance(split) <= minimum
        {
            return Err("Choose a point inside the line, away from its endpoints".into());
        }
        if click.distance(split) > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected line".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::Line { id, a, b: split };
        self.shapes.insert(
            index + 1,
            Shape::Line {
                id: new_id,
                a: split,
                b,
            },
        );
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        if let Some(axis) = self.axis_constraints.get(&id).copied() {
            self.axis_constraints.insert(new_id, axis);
        }
        self.driving_dimensions
            .remove(&(id, DimensionField::LineLength));
        let relations = self.detach_endpoint_relations((id, 1));
        self.attach_endpoint_relations((new_id, 1), relations);
        self.selected = Some(new_id);
        Ok(new_id)
    }

    pub fn split_selected_curve(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select a curve to split")?;
        if self.internal_tangent_member(id)
            || self.midpoint_links.values().any(|source| *source == id)
            || self.point_curve_links.values().any(|source| *source == id)
            || self.normal_links.contains_key(&id)
            || self.normal_links.values().any(|(source, _)| *source == id)
        {
            return Err("Remove dependent relation before splitting".into());
        }
        match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Line { .. }) => self.split_selected_line(click),
            Some(Shape::Arc { .. }) => self.split_selected_arc(click),
            Some(Shape::EllipticArc { .. }) => self.split_selected_elliptic_arc(click),
            Some(Shape::Spline { .. }) => self.split_selected_spline(click),
            _ => Err("Select a curve to split".into()),
        }
    }

    pub fn split_selected_spline(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select a spline to split")?;
        if self.has_fixed_anchor(id) {
            return Err("Remove fixed spline points before splitting".into());
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
            return Err("Select a spline to split".into());
        };
        let curves = spline::beziers(mode, points, closed)?;
        let nearest = spline::nearest(&curves, click)?;
        if nearest.distance > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected spline".into());
        }
        if closed {
            let opened = spline::open_periodic(&curves, nearest.segment, nearest.t)?;
            self.history.push(self.snapshot());
            self.future.clear();
            self.shapes[index] = Shape::Spline {
                id,
                mode: SplineMode::Control,
                points: opened,
                closed: false,
            };
            self.selected_spline_point = None;
            return Ok(id);
        }
        let (first, second) = spline::split_open(&curves, nearest.segment, nearest.t)?;
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::Spline {
            id,
            mode: SplineMode::Control,
            points: first,
            closed: false,
        };
        self.shapes.insert(
            index + 1,
            Shape::Spline {
                id: new_id,
                mode: SplineMode::Control,
                points: second,
                closed: false,
            },
        );
        if let Some(point_id) = self.point_links.remove(&(id, 1)) {
            self.point_links.insert((new_id, 1), point_id);
        }
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.selected = Some(new_id);
        self.selected_spline_point = None;
        Ok(new_id)
    }

    pub fn split_selected_elliptic_arc(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select an elliptic arc to split")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected elliptic arc is missing")?;
        let Shape::EllipticArc {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an elliptic arc to split".into());
        };
        if ![click.x, click.y, center.x, center.y, rx, ry, rotation]
            .into_iter()
            .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep == 0.0
        {
            return Err("Elliptic arc or split point is invalid".into());
        }
        let angle = Shape::ellipse_parameter_angle(center, rx, ry, rotation, click);
        let fraction = Shape::elliptic_arc_progress(angle, start_angle, sweep);
        let on_curve =
            Shape::elliptic_arc_point(center, rx, ry, rotation, start_angle, sweep, fraction);
        let minimum = (2.0 / self.zoom.max(0.2)) as f64 / (rx.min(ry) as f64 * sweep.abs());
        if fraction <= minimum
            || fraction >= 1.0 - minimum
            || click.distance(on_curve) > 8.0 / self.zoom.max(0.2)
        {
            return Err("Choose an interior point on the elliptic arc".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let first_sweep = sweep * fraction;
        self.shapes[index] = Shape::EllipticArc {
            id,
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep: first_sweep,
            segments,
        };
        self.shapes.insert(
            index + 1,
            Shape::EllipticArc {
                id: new_id,
                center,
                rx,
                ry,
                rotation,
                start_angle: start_angle + first_sweep,
                sweep: sweep - first_sweep,
                segments,
            },
        );
        let root = self.shared_ellipses.get(&id).copied().unwrap_or(id);
        self.shared_ellipses.insert(new_id, root);
        if let Some(position) = self.fixed_anchors.remove(&(id, 2)) {
            self.fixed_anchors.insert((new_id, 2), position);
        }
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.selected = Some(new_id);
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(new_id)
    }

    pub fn split_selected_ellipse(&mut self, first: Point, second: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select an ellipse to split")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected ellipse is missing")?;
        let Shape::Ellipse {
            center,
            rx,
            ry,
            rotation,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an ellipse to split".into());
        };
        if ![
            first.x, first.y, second.x, second.y, center.x, center.y, rx, ry, rotation,
        ]
        .into_iter()
        .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
        {
            return Err("Ellipse or split positions are invalid".into());
        }
        let a = Shape::ellipse_parameter_angle(center, rx, ry, rotation, first);
        let b = Shape::ellipse_parameter_angle(center, rx, ry, rotation, second);
        let on_curve = |point: Point, angle: f64| {
            point.distance(Shape::elliptic_arc_point(
                center, rx, ry, rotation, angle, 0.0, 0.0,
            )) <= 8.0 / self.zoom.max(0.2)
        };
        if !on_curve(first, a) || !on_curve(second, b) {
            return Err("Choose two points on the ellipse".into());
        }
        let (start, end) = if a < b { (a, b) } else { (b, a) };
        let minimum = (2.0 / self.zoom.max(0.2)) as f64 / rx.min(ry) as f64;
        let tau = std::f64::consts::TAU;
        if end - start <= minimum || tau - (end - start) <= minimum {
            return Err("Choose two distinct split points".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::EllipticArc {
            id,
            center,
            rx,
            ry,
            rotation,
            start_angle: start,
            sweep: end - start,
            segments,
        };
        self.shapes.insert(
            index + 1,
            Shape::EllipticArc {
                id: new_id,
                center,
                rx,
                ry,
                rotation,
                start_angle: end,
                sweep: start + tau - end,
                segments,
            },
        );
        let root = self.shared_ellipses.get(&id).copied().unwrap_or(id);
        self.shared_ellipses.insert(new_id, root);
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.driving_dimensions
            .remove(&(id, DimensionField::RectangleWidth));
        self.driving_dimensions
            .remove(&(id, DimensionField::RectangleHeight));
        self.selected = Some(new_id);
        Ok(new_id)
    }

    pub fn split_selected_arc(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select an arc to split")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected arc is missing")?;
        let Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an arc to split".into());
        };
        if !click.x.is_finite()
            || !click.y.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep.abs() <= 1e-9
        {
            return Err("Arc or split position is invalid".into());
        }
        let angle = ((click.y - center.y) as f64).atan2((click.x - center.x) as f64);
        let traveled = if sweep > 0.0 {
            (angle - start_angle).rem_euclid(std::f64::consts::TAU)
        } else {
            (start_angle - angle).rem_euclid(std::f64::consts::TAU)
        };
        let fraction = traveled / sweep.abs();
        let minimum = (2.0 / self.zoom.max(0.2)) as f64 / (radius as f64 * sweep.abs());
        if fraction <= minimum
            || fraction >= 1.0 - minimum
            || (click.distance(center) - radius).abs() > 8.0 / self.zoom.max(0.2)
        {
            return Err("Choose an interior point on the arc".into());
        }
        let first_sweep = sweep * fraction;
        let second_start = start_angle + first_sweep;
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::Arc {
            id,
            center,
            radius,
            start_angle,
            sweep: first_sweep,
            segments,
        };
        self.shapes.insert(
            index + 1,
            Shape::Arc {
                id: new_id,
                center,
                radius,
                start_angle: second_start,
                sweep: sweep - first_sweep,
                segments,
            },
        );
        if let Some(position) = self.fixed_anchors.remove(&(id, 2)) {
            self.fixed_anchors.insert((new_id, 2), position);
        }
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.selected = Some(new_id);
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(new_id)
    }

    pub fn split_selected_circle(&mut self, first: Point, second: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select a circle to split")?;
        if self.internal_tangent_member(id)
            || self.midpoint_links.values().any(|source| *source == id)
            || self.point_curve_links.values().any(|source| *source == id)
            || self.normal_links.contains_key(&id)
            || self.normal_links.values().any(|(source, _)| *source == id)
        {
            return Err("Remove dependent relation before splitting".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected circle is missing")?;
        let Shape::Circle {
            center,
            radius,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select a circle to split".into());
        };
        if !radius.is_finite()
            || radius <= 0.0
            || ![first.x, first.y, second.x, second.y]
                .into_iter()
                .all(f32::is_finite)
        {
            return Err("Circle or split positions are invalid".into());
        }
        let tolerance = 8.0 / self.zoom.max(0.2);
        if [first, second]
            .into_iter()
            .any(|point| (point.distance(center) - radius).abs() > tolerance)
        {
            return Err("Choose two points on the circle".into());
        }
        let tau = std::f64::consts::TAU;
        let angle = |point: Point| {
            ((point.y - center.y) as f64)
                .atan2((point.x - center.x) as f64)
                .rem_euclid(tau)
        };
        let (a, b) = (angle(first), angle(second));
        let (start, end) = if a < b { (a, b) } else { (b, a) };
        let minimum = (2.0 / self.zoom.max(0.2)) as f64 / radius as f64;
        if end - start <= minimum || tau - (end - start) <= minimum {
            return Err("Choose two distinct split points".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.radius_differences
            .retain(|target, (source, _)| *target != id && *source != id);
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::Arc {
            id,
            center,
            radius,
            start_angle: start,
            sweep: end - start,
            segments,
        };
        self.shapes.insert(
            index + 1,
            Shape::Arc {
                id: new_id,
                center,
                radius,
                start_angle: end,
                sweep: start + tau - end,
                segments,
            },
        );
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.driving_dimensions
            .remove(&(id, DimensionField::CircleDiameter));
        self.selected = Some(new_id);
        Ok(new_id)
    }

    pub fn trim_selected_line(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select a line to trim")?;
        if self.equal_lines.contains_key(&id)
            || self.equal_lines.values().any(|source| *source == id)
            || self.line_relations.contains_key(&id)
            || self
                .line_relations
                .values()
                .any(|(source, _)| *source == id)
        {
            return Err("Remove Equal before trimming the line".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected line is missing")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("Select a line to trim".into());
        };
        if !click.x.is_finite() || !click.y.is_finite() {
            return Err("Trim position must be finite".into());
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length_squared = dx * dx + dy * dy;
        if length_squared <= 1e-12 {
            return Err("Cannot trim a zero-length line".into());
        }
        let parameter =
            |point: Point| ((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared;
        let t = parameter(click);
        let projected = Point {
            x: a.x + t * dx,
            y: a.y + t * dy,
        };
        if !(0.0..=1.0).contains(&t) || click.distance(projected) > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected line".into());
        }
        let mut cuts: Vec<f32> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(parameter)
            .filter(|cut| cut.is_finite() && *cut > 1e-5 && *cut < 1.0 - 1e-5)
            .collect();
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-5);
        let lower = cuts.iter().copied().rfind(|cut| *cut < t).unwrap_or(0.0);
        let upper = cuts.iter().copied().find(|cut| *cut > t).unwrap_or(1.0);
        let intervals: Vec<(f32, f32)> = [(0.0, lower), (upper, 1.0)]
            .into_iter()
            .filter(|(start, end)| (end - start) * length_squared.sqrt() > 1e-5)
            .collect();
        if intervals.is_empty() {
            self.delete_selected();
            return Ok(0);
        }
        let before = self.snapshot();
        if intervals[0].0 != 0.0 {
            self.drop_endpoint_relations((id, 0));
        }
        if intervals.last().is_some_and(|(_, end)| *end == 1.0) {
            if intervals.len() > 1 {
                let relations = self.detach_endpoint_relations((id, 1));
                self.attach_endpoint_relations(
                    (self.next_id + intervals.len() as u32 - 2, 1),
                    relations,
                );
            }
        } else {
            self.drop_endpoint_relations((id, 1));
        }
        let old_start_link = self.point_links.remove(&(id, 0));
        let old_end_link = self.point_links.remove(&(id, 1));
        let old_start_fixed = self.fixed_anchors.remove(&(id, 0));
        let old_end_fixed = self.fixed_anchors.remove(&(id, 1));
        let axis = self.axis_constraints.get(&id).copied();
        let construction = self.construction.contains(&id);
        self.driving_dimensions
            .remove(&(id, DimensionField::LineLength));
        let mut pieces = Vec::new();
        for (piece_index, (start, end)) in intervals.iter().copied().enumerate() {
            let piece_id = if piece_index == 0 {
                id
            } else {
                let next = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                next
            };
            let at = |fraction: f32| Point {
                x: a.x + fraction * dx,
                y: a.y + fraction * dy,
            };
            pieces.push(Shape::Line {
                id: piece_id,
                a: at(start),
                b: at(end),
            });
            if construction {
                self.construction.insert(piece_id);
            }
            if let Some(axis) = axis {
                self.axis_constraints.insert(piece_id, axis);
            }
            if start == 0.0 {
                if let Some(point_id) = old_start_link {
                    self.point_links.insert((piece_id, 0), point_id);
                }
                if let Some(position) = old_start_fixed {
                    self.fixed_anchors.insert((piece_id, 0), position);
                }
            }
            if end == 1.0 {
                if let Some(point_id) = old_end_link {
                    self.point_links.insert((piece_id, 1), point_id);
                }
                if let Some(position) = old_end_fixed {
                    self.fixed_anchors.insert((piece_id, 1), position);
                }
            }
        }
        self.shapes.splice(index..=index, pieces);
        self.history.push(before);
        self.future.clear();
        self.selected = Some(id);
        Ok(intervals.len())
    }

    pub fn trim_selected_curve(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select a curve to trim")?;
        if self.internal_tangent_member(id)
            || self.midpoint_links.values().any(|source| *source == id)
            || self.point_curve_links.values().any(|source| *source == id)
            || self.normal_links.contains_key(&id)
            || self.normal_links.values().any(|(source, _)| *source == id)
        {
            return Err("Remove dependent relation before trimming".into());
        }
        match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Line { .. }) => self.trim_selected_line(click),
            Some(Shape::Arc { .. }) => self.trim_selected_arc(click),
            Some(Shape::Circle { .. }) => self.trim_selected_circle(click),
            Some(Shape::Ellipse { .. }) => self.trim_selected_ellipse(click),
            Some(Shape::EllipticArc { .. }) => self.trim_selected_elliptic_arc(click),
            Some(Shape::Spline { .. }) => self.trim_selected_spline(click),
            _ => Err("Select a curve to trim".into()),
        }
    }

    pub fn trim_selected_spline(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select a spline to trim")?;
        if self.has_fixed_anchor(id) {
            return Err("Remove fixed spline points before trimming".into());
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
            return Err("Select a spline to trim".into());
        };
        let curves = spline::beziers(mode, points, closed)?;
        let target = spline::nearest(&curves, click)?;
        if target.distance > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected spline".into());
        }
        let position = target.segment as f32 + target.t;
        let boundary_hits: Vec<Vec<(usize, f32, Point)>> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .map(|shape| match shape {
                Shape::Spline {
                    mode,
                    points,
                    closed,
                    ..
                } => {
                    let other = spline::beziers(*mode, points, *closed)?;
                    spline::spline_intersections(&curves, &other)
                }
                _ => Ok(spline_boundary_intersections(&curves, shape)),
            })
            .collect::<Result<_, String>>()?;
        let mut cuts: Vec<f32> = boundary_hits
            .into_iter()
            .flatten()
            .map(|(segment, t, _)| segment as f32 + t)
            .filter(|cut| *cut > 1e-5 && *cut < curves.len() as f32 - 1e-5)
            .collect();
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
        let length = curves.len() as f32;
        let intervals = if closed {
            if cuts.len() < 2 {
                Vec::new()
            } else {
                let lower = cuts
                    .iter()
                    .copied()
                    .rfind(|cut| *cut <= position)
                    .unwrap_or(cuts[cuts.len() - 1] - length);
                let upper = cuts
                    .iter()
                    .copied()
                    .find(|cut| *cut > position)
                    .unwrap_or(cuts[0] + length);
                vec![(upper, lower + length)]
            }
        } else {
            let lower = cuts
                .iter()
                .copied()
                .rfind(|cut| *cut < position)
                .unwrap_or(0.0);
            let upper = cuts
                .iter()
                .copied()
                .find(|cut| *cut > position)
                .unwrap_or(length);
            [(0.0, lower), (upper, length)]
                .into_iter()
                .filter(|(a, b)| b - a > 1e-5)
                .collect()
        };
        if intervals.is_empty() {
            self.delete_selected();
            return Ok(0);
        }
        let before = self.snapshot();
        let mut pieces = Vec::new();
        for (piece_index, (start, end)) in intervals.iter().copied().enumerate() {
            let points = spline::portion(&curves, start, end, closed)?;
            let piece_id = if piece_index == 0 {
                id
            } else {
                let next = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                next
            };
            pieces.push(Shape::Spline {
                id: piece_id,
                mode: SplineMode::Control,
                points,
                closed: false,
            });
            if self.construction.contains(&id) {
                self.construction.insert(piece_id);
            }
        }
        let start_link = self.point_links.remove(&(id, 0));
        let end_link = self.point_links.remove(&(id, 1));
        for (piece, &(start, end)) in pieces.iter().zip(intervals.iter()) {
            if start == 0.0
                && let Some(point_id) = start_link
            {
                self.point_links.insert((piece.id(), 0), point_id);
            }
            if end == length
                && let Some(point_id) = end_link
            {
                self.point_links.insert((piece.id(), 1), point_id);
            }
        }
        self.shapes.splice(index..=index, pieces);
        self.history.push(before);
        self.future.clear();
        self.selected_spline_point = None;
        self.selected = Some(id);
        Ok(intervals.len())
    }

    pub fn trim_selected_ellipse(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select an ellipse to trim")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected ellipse is missing")?;
        let Shape::Ellipse {
            center,
            rx,
            ry,
            rotation,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an ellipse to trim".into());
        };
        if ![click.x, click.y, center.x, center.y, rx, ry, rotation]
            .into_iter()
            .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
        {
            return Err("Ellipse or trim position is invalid".into());
        }
        let angle = |point| Shape::ellipse_parameter_angle(center, rx, ry, rotation, point);
        let clicked = angle(click);
        if click.distance(Shape::elliptic_arc_point(
            center, rx, ry, rotation, clicked, 0.0, 0.0,
        )) > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected ellipse".into());
        }
        let tau = std::f64::consts::TAU;
        let mut cuts: Vec<f64> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(angle)
            .filter(|value| value.is_finite())
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-8);
        if cuts.len() < 2 {
            self.delete_selected();
            return Ok(0);
        }
        let lower = cuts
            .iter()
            .copied()
            .rfind(|value| *value <= clicked)
            .unwrap_or(cuts[cuts.len() - 1] - tau);
        let upper = cuts
            .iter()
            .copied()
            .find(|value| *value > clicked)
            .unwrap_or(cuts[0] + tau);
        let retained_sweep = lower + tau - upper;
        if retained_sweep <= 1e-8 {
            self.delete_selected();
            return Ok(0);
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::EllipticArc {
            id,
            center,
            rx,
            ry,
            rotation,
            start_angle: upper.rem_euclid(tau),
            sweep: retained_sweep,
            segments,
        };
        self.driving_dimensions
            .remove(&(id, DimensionField::RectangleWidth));
        self.driving_dimensions
            .remove(&(id, DimensionField::RectangleHeight));
        Ok(1)
    }

    pub fn trim_selected_elliptic_arc(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select an elliptic arc to trim")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected elliptic arc is missing")?;
        let Shape::EllipticArc {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an elliptic arc to trim".into());
        };
        if ![click.x, click.y, center.x, center.y, rx, ry, rotation]
            .into_iter()
            .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep == 0.0
        {
            return Err("Elliptic arc or trim position is invalid".into());
        }
        let parameter = |point| {
            Shape::elliptic_arc_progress(
                Shape::ellipse_parameter_angle(center, rx, ry, rotation, point),
                start_angle,
                sweep,
            )
        };
        let t = parameter(click);
        if !(0.0..=1.0).contains(&t)
            || click.distance(Shape::elliptic_arc_point(
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                t,
            )) > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected elliptic arc".into());
        }
        let mut cuts: Vec<f64> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(parameter)
            .filter(|cut| cut.is_finite() && *cut > 1e-8 && *cut < 1.0 - 1e-8)
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-8);
        let lower = cuts.iter().copied().rfind(|cut| *cut < t).unwrap_or(0.0);
        let upper = cuts.iter().copied().find(|cut| *cut > t).unwrap_or(1.0);
        let intervals: Vec<(f64, f64)> = [(0.0, lower), (upper, 1.0)]
            .into_iter()
            .filter(|(start, end)| (end - start) * sweep.abs() * rx.min(ry) as f64 > 1e-5)
            .collect();
        if intervals.is_empty() {
            self.delete_selected();
            return Ok(0);
        }
        let before = self.snapshot();
        let construction = self.construction.contains(&id);
        let start_fixed = self.fixed_anchors.remove(&(id, 1));
        let end_fixed = self.fixed_anchors.remove(&(id, 2));
        let mut pieces = Vec::new();
        for (piece_index, (start, end)) in intervals.iter().copied().enumerate() {
            let piece_id = if piece_index == 0 {
                id
            } else {
                let next = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                next
            };
            pieces.push(Shape::EllipticArc {
                id: piece_id,
                center,
                rx,
                ry,
                rotation,
                start_angle: start_angle + sweep * start,
                sweep: sweep * (end - start),
                segments,
            });
            if start == 0.0
                && let Some(position) = start_fixed
            {
                self.fixed_anchors.insert((piece_id, 1), position);
            }
            if end == 1.0
                && let Some(position) = end_fixed
            {
                self.fixed_anchors.insert((piece_id, 2), position);
            }
            if construction {
                self.construction.insert(piece_id);
            }
        }
        self.shapes.splice(index..=index, pieces);
        self.history.push(before);
        self.future.clear();
        self.selected = Some(id);
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(intervals.len())
    }

    pub fn trim_selected_circle(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select a circle to trim")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected circle is missing")?;
        let Shape::Circle {
            center,
            radius,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select a circle to trim".into());
        };
        if !click.x.is_finite() || !click.y.is_finite() || !radius.is_finite() || radius <= 0.0 {
            return Err("Circle or trim position is invalid".into());
        }
        if (click.distance(center) - radius).abs() > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected circle".into());
        }
        let tau = std::f64::consts::TAU;
        let angle = |point: Point| {
            ((point.y - center.y) as f64)
                .atan2((point.x - center.x) as f64)
                .rem_euclid(tau)
        };
        let clicked = angle(click);
        let mut cuts: Vec<f64> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(angle)
            .filter(|value| value.is_finite())
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-8);
        if cuts.len() < 2 {
            self.delete_selected();
            return Ok(0);
        }
        let lower = cuts
            .iter()
            .copied()
            .rfind(|value| *value <= clicked)
            .unwrap_or(cuts[cuts.len() - 1] - tau);
        let upper = cuts
            .iter()
            .copied()
            .find(|value| *value > clicked)
            .unwrap_or(cuts[0] + tau);
        let retained_sweep = lower + tau - upper;
        if retained_sweep <= 1e-8 {
            self.delete_selected();
            return Ok(0);
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.radius_differences
            .retain(|target, (source, _)| *target != id && *source != id);
        self.shapes[index] = Shape::Arc {
            id,
            center,
            radius,
            start_angle: upper,
            sweep: retained_sweep,
            segments,
        };
        self.driving_dimensions
            .remove(&(id, DimensionField::CircleDiameter));
        Ok(1)
    }

    pub fn trim_selected_arc(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select an arc to trim")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected arc is missing")?;
        let Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an arc to trim".into());
        };
        if !click.x.is_finite()
            || !click.y.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep.abs() <= 1e-9
        {
            return Err("Arc or trim position is invalid".into());
        }
        let t = arc_progress(click, center, start_angle, sweep);
        if !(0.0..=1.0).contains(&t)
            || (click.distance(center) - radius).abs() > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected arc".into());
        }
        let mut cuts: Vec<f64> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(|point| arc_progress(point, center, start_angle, sweep))
            .filter(|cut| cut.is_finite() && *cut > 1e-8 && *cut < 1.0 - 1e-8)
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-8);
        let lower = cuts.iter().copied().rfind(|cut| *cut < t).unwrap_or(0.0);
        let upper = cuts.iter().copied().find(|cut| *cut > t).unwrap_or(1.0);
        let intervals: Vec<(f64, f64)> = [(0.0, lower), (upper, 1.0)]
            .into_iter()
            .filter(|(start, end)| (end - start) * sweep.abs() * radius as f64 > 1e-5)
            .collect();
        if intervals.is_empty() {
            self.delete_selected();
            return Ok(0);
        }
        let before = self.snapshot();
        let construction = self.construction.contains(&id);
        let start_fixed = self.fixed_anchors.remove(&(id, 1));
        let end_fixed = self.fixed_anchors.remove(&(id, 2));
        let mut pieces = Vec::new();
        for (piece_index, (start, end)) in intervals.iter().copied().enumerate() {
            let piece_id = if piece_index == 0 {
                id
            } else {
                let next = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                next
            };
            pieces.push(Shape::Arc {
                id: piece_id,
                center,
                radius,
                start_angle: start_angle + sweep * start,
                sweep: sweep * (end - start),
                segments,
            });
            if start == 0.0
                && let Some(position) = start_fixed
            {
                self.fixed_anchors.insert((piece_id, 1), position);
            }
            if end == 1.0
                && let Some(position) = end_fixed
            {
                self.fixed_anchors.insert((piece_id, 2), position);
            }
            if construction {
                self.construction.insert(piece_id);
            }
        }
        self.shapes.splice(index..=index, pieces);
        self.history.push(before);
        self.future.clear();
        self.selected = Some(id);
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(intervals.len())
    }

    pub fn extend_selected_line(&mut self, click: Point) -> Result<Point, String> {
        let id = self.selected.ok_or("Select a line to extend")?;
        if self.equal_lines.contains_key(&id)
            || self.equal_lines.values().any(|source| *source == id)
            || self.line_relations.contains_key(&id)
            || self
                .line_relations
                .values()
                .any(|(source, _)| *source == id)
        {
            return Err("Remove Equal before extending the line".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected line is missing")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("Select a line to extend".into());
        };
        if !click.x.is_finite() || !click.y.is_finite() {
            return Err("Extend position must be finite".into());
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length = dx.hypot(dy);
        if length <= 1e-6 {
            return Err("Cannot extend a zero-length line".into());
        }
        let t = ((click.x - a.x) * dx + (click.y - a.y) * dy) / (length * length);
        let projected = Point {
            x: a.x + t * dx,
            y: a.y + t * dy,
        };
        if !(0.0..=1.0).contains(&t) || click.distance(projected) > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected line".into());
        }
        let start = t < 0.5;
        if self
            .fixed_anchors
            .contains_key(&(id, if start { 0 } else { 1 }))
        {
            return Err("line endpoint is fixed".into());
        }
        let endpoint = if start { a } else { b };
        let direction = if start {
            Point {
                x: -dx / length,
                y: -dy / length,
            }
        } else {
            Point {
                x: dx / length,
                y: dy / length,
            }
        };
        let others: Vec<_> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .collect();
        let reach = others
            .iter()
            .filter_map(|shape| shape.bounds())
            .flat_map(|(min, max)| {
                [
                    min,
                    max,
                    Point { x: min.x, y: max.y },
                    Point { x: max.x, y: min.y },
                ]
            })
            .map(|point| endpoint.distance(point))
            .fold(0.0_f32, f32::max)
            + length
            + 1.0;
        if !reach.is_finite() || others.is_empty() {
            return Err("No boundary found beyond this endpoint".into());
        }
        let far = Point {
            x: endpoint.x + direction.x * reach,
            y: endpoint.y + direction.y * reach,
        };
        let ray = Shape::Line {
            id: 0,
            a: endpoint,
            b: far,
        };
        let target = others
            .into_iter()
            .flat_map(|shape| finite_intersections(&ray, shape))
            .filter(|point| point.distance(endpoint) > 1e-5)
            .min_by(|left, right| left.distance(endpoint).total_cmp(&right.distance(endpoint)))
            .ok_or("No boundary found beyond this endpoint")?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = if start {
            Shape::Line { id, a: target, b }
        } else {
            Shape::Line { id, a, b: target }
        };
        self.point_links.remove(&(id, if start { 0 } else { 1 }));
        self.driving_dimensions
            .remove(&(id, DimensionField::LineLength));
        Ok(target)
    }

    pub fn extend_selected_curve(&mut self, click: Point) -> Result<Point, String> {
        let id = self.selected.ok_or("Select an open curve to extend")?;
        if self.internal_tangent_member(id)
            || self.midpoint_links.values().any(|source| *source == id)
            || self.point_curve_links.values().any(|source| *source == id)
            || self.normal_links.contains_key(&id)
            || self.normal_links.values().any(|(source, _)| *source == id)
        {
            return Err("Remove dependent relation before extending".into());
        }
        match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Line { .. }) => self.extend_selected_line(click),
            Some(Shape::Arc { .. }) => self.extend_selected_arc(click),
            Some(Shape::EllipticArc { .. }) => self.extend_selected_elliptic_arc(click),
            _ => Err("Select an open curve to extend".into()),
        }
    }

    pub fn extend_selected_elliptic_arc(&mut self, click: Point) -> Result<Point, String> {
        let id = self.selected.ok_or("Select an elliptic arc to extend")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected elliptic arc is missing")?;
        let Shape::EllipticArc {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an elliptic arc to extend".into());
        };
        if ![click.x, click.y, center.x, center.y, rx, ry, rotation]
            .into_iter()
            .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep == 0.0
            || sweep.abs() >= std::f64::consts::TAU
        {
            return Err("Elliptic arc or extend position is invalid".into());
        }
        let t = Shape::elliptic_arc_progress(
            Shape::ellipse_parameter_angle(center, rx, ry, rotation, click),
            start_angle,
            sweep,
        );
        if !(0.0..=1.0).contains(&t)
            || click.distance(Shape::elliptic_arc_point(
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                t,
            )) > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected elliptic arc".into());
        }
        let start = t < 0.5;
        if self
            .fixed_anchors
            .contains_key(&(id, if start { 1 } else { 2 }))
        {
            return Err("arc endpoint is fixed".into());
        }
        let direction = sweep.signum();
        let endpoint_angle = if start {
            start_angle
        } else {
            start_angle + sweep
        };
        let max_extra = std::f64::consts::TAU - sweep.abs();
        if max_extra <= 1e-8 {
            return Err("Elliptic arc is already a full ellipse".into());
        }
        let ellipse = Shape::Ellipse {
            id: 0,
            center,
            rx,
            ry,
            rotation,
            segments,
        };
        let target = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&ellipse, shape))
            .filter_map(|point| {
                let angle = Shape::ellipse_parameter_angle(center, rx, ry, rotation, point);
                let delta = if start {
                    (direction * (endpoint_angle - angle)).rem_euclid(std::f64::consts::TAU)
                } else {
                    (direction * (angle - endpoint_angle)).rem_euclid(std::f64::consts::TAU)
                };
                (delta > 1e-6 && delta <= max_extra + 1e-8).then_some((delta, point))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .ok_or("No boundary found beyond this endpoint")?;
        let extra = direction * target.0;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::EllipticArc {
            id,
            center,
            rx,
            ry,
            rotation,
            start_angle: if start {
                start_angle - extra
            } else {
                start_angle
            },
            sweep: sweep + extra,
            segments,
        };
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(target.1)
    }

    pub fn extend_selected_arc(&mut self, click: Point) -> Result<Point, String> {
        let id = self.selected.ok_or("Select an arc to extend")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected arc is missing")?;
        let Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an arc to extend".into());
        };
        if !click.x.is_finite()
            || !click.y.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep.abs() <= 1e-9
        {
            return Err("Arc or extend position is invalid".into());
        }
        let t = arc_progress(click, center, start_angle, sweep);
        if !(0.0..=1.0).contains(&t)
            || (click.distance(center) - radius).abs() > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected arc".into());
        }
        let start = t < 0.5;
        if self
            .fixed_anchors
            .contains_key(&(id, if start { 1 } else { 2 }))
        {
            return Err("arc endpoint is fixed".into());
        }
        let direction = sweep.signum();
        let endpoint_angle = if start {
            start_angle
        } else {
            start_angle + sweep
        };
        let max_extra = std::f64::consts::TAU - sweep.abs();
        if max_extra <= 1e-8 {
            return Err("Arc is already a full circle".into());
        }
        let circle = Shape::Circle {
            id: 0,
            center,
            radius,
            segments,
        };
        let target = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&circle, shape))
            .filter_map(|point| {
                let angle = ((point.y - center.y) as f64).atan2((point.x - center.x) as f64);
                let delta = if start {
                    (direction * (endpoint_angle - angle)).rem_euclid(std::f64::consts::TAU)
                } else {
                    (direction * (angle - endpoint_angle)).rem_euclid(std::f64::consts::TAU)
                };
                (delta > 1e-6 && delta <= max_extra + 1e-8).then_some((delta, point))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .ok_or("No boundary found beyond this endpoint")?;
        let extra = direction * target.0;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::Arc {
            id,
            center,
            radius,
            start_angle: if start {
                start_angle - extra
            } else {
                start_angle
            },
            sweep: sweep + extra,
            segments,
        };
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(target.1)
    }

    pub fn copy_selected(&mut self, delta: Point) -> bool {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return false;
        }
        let Some(source) = self
            .selected
            .and_then(|id| self.shapes.iter().find(|shape| shape.id() == id))
            .cloned()
        else {
            return false;
        };
        let source_id = source.id();
        let id = self.next_id;
        let copied_dimensions: Vec<_> = self
            .driving_dimensions
            .iter()
            .filter(|((shape_id, _), _)| *shape_id == source_id)
            .map(|((_, field), value)| ((*field), *value))
            .collect();
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes.push(source.translated(delta).with_id(id));
        if self.construction.contains(&source_id) {
            self.construction.insert(id);
        }
        if let Some(axis) = self.axis_constraints.get(&source_id).copied() {
            self.axis_constraints.insert(id, axis);
        }
        for (field, value) in copied_dimensions {
            self.driving_dimensions.insert((id, field), value);
        }
        self.selected = Some(id);
        self.selected_spline_point = None;
        self.next_id = self.next_id.saturating_add(1);
        true
    }

    pub fn move_selected(&mut self, delta: Point) -> bool {
        if !delta.x.is_finite() || !delta.y.is_finite() || (delta.x == 0.0 && delta.y == 0.0) {
            return false;
        }
        let Some(index) = self
            .selected
            .and_then(|id| self.shapes.iter().position(|shape| shape.id() == id))
        else {
            return false;
        };
        if self.midpoint_links.contains_key(&self.shapes[index].id()) {
            return false;
        }
        if self.has_fixed_anchor(self.shapes[index].id()) {
            return false;
        }
        if self
            .shared_ellipse_partners(self.shapes[index].id())
            .into_iter()
            .any(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        if self
            .concentric_partner(self.shapes[index].id())
            .is_some_and(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        let mut next = self.shapes[index].translated(delta);
        if let Some(&(source_id, kind)) = self.line_relations.get(&next.id())
            && kind == LineRelationKind::Collinear
        {
            let Some(source) = self.shapes.iter().find(|shape| shape.id() == source_id) else {
                return false;
            };
            let Some(projected) = Self::line_relation_shape(source, &next, kind) else {
                return false;
            };
            next = projected;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = next;
        let id = self.shapes[index].id();
        self.solve_or_rollback(&[Pin::Shape(id)]).is_ok()
    }

    pub fn scale_selected(&mut self, factor: f32, center: Point, copy: bool) -> bool {
        let Some(index) = self
            .selected
            .and_then(|id| self.shapes.iter().position(|shape| shape.id() == id))
        else {
            return false;
        };
        let source = &self.shapes[index];
        if !copy && self.midpoint_links.contains_key(&source.id()) {
            return false;
        }
        if !copy && self.has_fixed_anchor(source.id()) {
            return false;
        }
        if !copy
            && self
                .shared_ellipse_partners(source.id())
                .into_iter()
                .any(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        if !copy
            && self
                .shared_ellipse_partners(source.id())
                .into_iter()
                .any(|partner| {
                    [
                        DimensionField::RectangleWidth,
                        DimensionField::RectangleHeight,
                    ]
                    .into_iter()
                    .any(|field| self.driving_dimensions.contains_key(&(partner, field)))
                })
        {
            return false;
        }
        if !copy
            && self
                .concentric_partner(source.id())
                .is_some_and(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        let Some(mut scaled) = source.scaled_about(factor, center) else {
            return false;
        };
        if !copy
            && let Some(&(source_id, kind)) = self.line_relations.get(&source.id())
            && kind == LineRelationKind::Collinear
        {
            let Some(relation_source) = self.shapes.iter().find(|shape| shape.id() == source_id)
            else {
                return false;
            };
            let Some(projected) = Self::line_relation_shape(relation_source, &scaled, kind) else {
                return false;
            };
            scaled = projected;
        }
        if scaled == *source {
            return false;
        }
        let id = source.id();
        let dimensions: Vec<_> = self
            .driving_dimensions
            .iter()
            .filter(|((shape_id, _), _)| *shape_id == id)
            .map(|(key, value)| {
                (
                    *key,
                    if matches!(key.1, DimensionField::ArcSweep | DimensionField::Rotation) {
                        *value
                    } else {
                        *value * factor
                    },
                )
            })
            .collect();
        if dimensions.iter().any(|(_, value)| !value.is_finite()) {
            return false;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if copy {
            let new_id = self.next_id;
            self.shapes.push(scaled.with_id(new_id));
            if self.construction.contains(&id) {
                self.construction.insert(new_id);
            }
            if let Some(axis) = self.axis_constraints.get(&id).copied() {
                self.axis_constraints.insert(new_id, axis);
            }
            for ((_, field), value) in dimensions {
                self.driving_dimensions.insert((new_id, field), value);
            }
            self.selected = Some(new_id);
            self.next_id = self.next_id.saturating_add(1);
        } else {
            self.shapes[index] = scaled;
            for (key, value) in dimensions {
                self.driving_dimensions.insert(key, value);
            }
        }
        let pinned = if copy {
            self.selected.unwrap_or(id)
        } else {
            id
        };
        self.solve_or_rollback(&[Pin::Shape(pinned)]).is_ok()
    }

    pub fn rotate_selected(&mut self, degrees: f32, center: Point, copy: bool) -> bool {
        if !copy && degrees % 360.0 == 0.0 {
            return false;
        }
        let Some(index) = self
            .selected
            .and_then(|id| self.shapes.iter().position(|shape| shape.id() == id))
        else {
            return false;
        };
        if !copy && self.midpoint_links.contains_key(&self.shapes[index].id()) {
            return false;
        }
        let source = &self.shapes[index];
        if !copy && self.has_fixed_anchor(source.id()) {
            return false;
        }
        if !copy
            && self
                .shared_ellipse_partners(source.id())
                .into_iter()
                .any(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        if !copy
            && self
                .shared_ellipse_partners(source.id())
                .into_iter()
                .any(|partner| {
                    self.driving_dimensions
                        .contains_key(&(partner, DimensionField::Rotation))
                })
        {
            return false;
        }
        if !copy
            && self
                .concentric_partner(source.id())
                .is_some_and(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        let Some(rotated) = source.rotated_about(degrees, center) else {
            return false;
        };
        if !copy && rotated == *source {
            return false;
        }
        let source_id = source.id();
        let axis_after_rotation = if let Some(axis) = self.axis_constraints.get(&source_id).copied()
        {
            let quarter_turns = degrees / 90.0;
            if (quarter_turns - quarter_turns.round()).abs() > 1e-4 {
                return false;
            }
            if quarter_turns.round() as i32 % 2 == 0 {
                Some(axis)
            } else {
                Some(match axis {
                    AxisConstraint::Horizontal => AxisConstraint::Vertical,
                    AxisConstraint::Vertical => AxisConstraint::Horizontal,
                })
            }
        } else {
            None
        };
        let copied_dimensions: Vec<_> = self
            .driving_dimensions
            .iter()
            .filter(|((id, _), _)| *id == source_id)
            .map(|((_, field), value)| {
                (
                    *field,
                    if *field == DimensionField::Rotation {
                        *value + degrees
                    } else {
                        *value
                    },
                )
            })
            .collect();
        if copied_dimensions
            .iter()
            .any(|(_, value)| !value.is_finite())
        {
            return false;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if copy {
            let id = self.next_id;
            self.shapes.push(rotated.with_id(id));
            if self.construction.contains(&source_id) {
                self.construction.insert(id);
            }
            if let Some(axis) = axis_after_rotation {
                self.axis_constraints.insert(id, axis);
            }
            for (field, value) in copied_dimensions {
                self.driving_dimensions.insert((id, field), value);
            }
            self.selected = Some(id);
            self.next_id = self.next_id.saturating_add(1);
        } else {
            self.shapes[index] = rotated;
            if let Some(axis) = axis_after_rotation {
                self.axis_constraints.insert(source_id, axis);
            }
            for (field, value) in copied_dimensions {
                self.driving_dimensions.insert((source_id, field), value);
            }
        }
        let pinned = if copy {
            self.selected.unwrap_or(source_id)
        } else {
            source_id
        };
        self.solve_or_rollback(&[Pin::Shape(pinned)]).is_ok()
    }
}
