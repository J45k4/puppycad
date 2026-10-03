//! Tangent arc tool and the smooth join (web `smoothJoin`) relation between
//! line, arc and open spline endpoints (web `sketch-tangent-arc.ts`).

use super::*;

impl Document {
    /// Endpoint (0 start, 1 end) of a line, arc or open spline and the unit
    /// tangent pointing outward from the curve there (web
    /// `sketchEndpointDirection`).
    pub fn endpoint_direction(shape: &Shape, end: u8) -> Result<(Point, (f64, f64)), String> {
        let (at, direction) = match shape {
            Shape::Line { a, b, .. } => {
                let (near, far) = if end == 0 { (*a, *b) } else { (*b, *a) };
                (near, ((near.x - far.x) as f64, (near.y - far.y) as f64))
            }
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => {
                let angle = start_angle + if end == 0 { 0.0 } else { *sweep };
                let sign = sweep.signum() * if end == 0 { -1.0 } else { 1.0 };
                (
                    Point {
                        x: center.x + radius * angle.cos() as f32,
                        y: center.y + radius * angle.sin() as f32,
                    },
                    (-angle.sin() * sign, angle.cos() * sign),
                )
            }
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                if *closed {
                    return Err("A closed spline has no endpoints".into());
                }
                let curves = spline::beziers(*mode, points, false)?;
                let c = if end == 0 {
                    curves.first()
                } else {
                    curves.last()
                }
                .ok_or("Spline has no segments")?;
                let (near, next) = if end == 0 { (c[0], c[1]) } else { (c[3], c[2]) };
                (near, ((near.x - next.x) as f64, (near.y - next.y) as f64))
            }
            _ => return Err("Choose a line, arc or open spline endpoint".into()),
        };
        let length = direction.0.hypot(direction.1);
        if length.is_nan() || length <= 1e-9 {
            return Err(match shape {
                Shape::Spline { .. } => "Tangency requires a nonzero spline endpoint tangent",
                _ => "Tangency requires a nonzero curve",
            }
            .into());
        }
        Ok((at, (direction.0 / length, direction.1 / length)))
    }

    /// Nearest line, arc or open spline endpoint within 8 screen pixels.
    pub fn curve_endpoint_at(&self, cursor: Point) -> Option<(u32, u8)> {
        let tolerance = 8.0 / self.zoom.max(0.2);
        let mut best = None;
        let mut distance = tolerance;
        for shape in self.shapes.iter().rev() {
            if !matches!(
                shape,
                Shape::Line { .. } | Shape::Arc { .. } | Shape::Spline { closed: false, .. }
            ) {
                continue;
            }
            for end in [0u8, 1] {
                let at = match shape {
                    Shape::Spline { points, .. } => {
                        if end == 0 {
                            points.first().copied()
                        } else {
                            points.last().copied()
                        }
                    }
                    _ => Self::endpoint_direction(shape, end).ok().map(|(at, _)| at),
                };
                if let Some(at) = at
                    && at.distance(cursor) <= distance
                {
                    distance = at.distance(cursor);
                    best = Some((shape.id(), end));
                }
            }
        }
        best
    }

    /// Arc starting at `source` tangent to it and ending at `end` (web
    /// `tangentArc`). The arc gets id `id`.
    pub fn tangent_arc_shape(
        &self,
        source: (u32, u8),
        end: Point,
        id: u32,
    ) -> Result<Shape, String> {
        let shape = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source.0)
            .ok_or("Tangent arc source is missing")?;
        let (start, tangent) = Self::endpoint_direction(shape, source.1)?;
        let normal = (-tangent.1, tangent.0);
        let (dx, dy) = ((end.x - start.x) as f64, (end.y - start.y) as f64);
        let projection = dx * normal.0 + dy * normal.1;
        if !end.x.is_finite()
            || !end.y.is_finite()
            || dx.hypot(dy) < 1e-6
            || projection.abs() < 1e-6 * dx.hypot(dy).max(1.0)
        {
            return Err("Choose an endpoint away from the tangent line".into());
        }
        let offset = (dx * dx + dy * dy) / (2.0 * projection);
        let center = (
            start.x as f64 + normal.0 * offset,
            start.y as f64 + normal.1 * offset,
        );
        let start_angle = (start.y as f64 - center.1).atan2(start.x as f64 - center.0);
        let end_angle = (end.y as f64 - center.1).atan2(end.x as f64 - center.0);
        let tau = std::f64::consts::TAU;
        let sweep = if offset > 0.0 {
            (end_angle - start_angle).rem_euclid(tau)
        } else {
            -(start_angle - end_angle).rem_euclid(tau)
        };
        if sweep.abs() < 1e-9 || !sweep.is_finite() {
            return Err("Cannot create a tangent arc at those points".into());
        }
        Ok(Shape::Arc {
            id,
            center: Point {
                x: center.0 as f32,
                y: center.1 as f32,
            },
            radius: offset.abs() as f32,
            start_angle,
            sweep,
            segments: 128,
        })
    }

    /// Check that `source` can start a tangent arc (nonzero tangent).
    pub fn tangent_arc_source(&self, source: (u32, u8)) -> Result<Point, String> {
        let shape = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source.0)
            .ok_or("Tangent arc source is missing")?;
        Self::endpoint_direction(shape, source.1).map(|(at, _)| at)
    }

    /// Add a tangent arc from `source` to `end`, smoothly joined to the
    /// source at its start. Returns the arc id.
    pub fn add_tangent_arc(&mut self, source: (u32, u8), end: Point) -> Result<u32, String> {
        let id = self.next_id;
        let arc = self.tangent_arc_shape(source, end, id)?;
        let before = self.snapshot();
        self.next_id = self.next_id.saturating_add(1);
        self.shapes.push(arc);
        if self.construction.contains(&source.0) {
            self.construction.insert(id);
        }
        self.smooth_joins.insert((id, 0), source);
        self.record_undo(before, "Draw tangent arc");
        self.future.clear();
        self.selected = Some(id);
        self.draft_start = None;
        self.draft_end = None;
        self.tangent_source = None;
        Ok(id)
    }

    /// Smooth join gesture: join the nearest endpoint pair of the selected
    /// curve and `hovered` (line, arc or open spline). The hovered curve
    /// follows. With a smooth join already between them, remove it.
    pub fn toggle_smooth_join_with(&mut self, hovered: u32) -> Result<bool, String> {
        let selected = self.selected.ok_or("Select a line, arc or spline")?;
        if selected == hovered {
            return Err("Hover a different curve".into());
        }
        let existing: Vec<(u32, u8)> = self
            .smooth_joins
            .iter()
            .filter(|((follower, _), (source, _))| {
                (*follower == hovered && *source == selected)
                    || (*follower == selected && *source == hovered)
            })
            .map(|(key, _)| *key)
            .collect();
        if !existing.is_empty() {
            self.record_undo(self.snapshot(), "Remove Smooth");
            self.future.clear();
            for key in existing {
                self.smooth_joins.remove(&key);
            }
            return Ok(false);
        }
        let find = |id| self.shapes.iter().find(|shape| shape.id() == id);
        let (Some(first), Some(second)) = (find(selected), find(hovered)) else {
            return Err("Select a line, arc or spline".into());
        };
        let mut best: Option<((u8, u8), f32)> = None;
        for a_end in [0u8, 1] {
            for b_end in [0u8, 1] {
                let (Ok((a, _)), Ok((b, _))) = (
                    Self::endpoint_direction(first, a_end),
                    Self::endpoint_direction(second, b_end),
                ) else {
                    continue;
                };
                let distance = a.distance(b);
                if best.is_none_or(|(_, d)| distance < d) {
                    best = Some(((a_end, b_end), distance));
                }
            }
        }
        let ((a_end, b_end), _) = best
            .ok_or("Smooth join needs line, arc or open spline endpoints with nonzero tangents")?;
        let (source, follower) = ((selected, a_end), (hovered, b_end));
        let busy = |end: (u32, u8)| {
            self.smooth_joins.contains_key(&end) || self.smooth_joins.values().any(|v| *v == end)
        };
        if busy(source) || busy(follower) {
            return Err("Endpoint already has a smooth join".into());
        }
        let before = self.snapshot();
        self.smooth_joins.insert(follower, source);
        if let Err(error) = self.solve_constraints(&[Pin::Shape(selected)]) {
            self.restore(before);
            self.selected = Some(selected);
            return Err(error);
        }
        self.record_undo(before, "Add Smooth");
        self.future.clear();
        self.selected = Some(selected);
        Ok(true)
    }

    /// Shapes in a smooth join, for relation glyphs and the project rows.
    pub fn smooth_join_member(&self, id: u32) -> bool {
        self.smooth_joins
            .iter()
            .any(|((follower, _), (source, _))| *follower == id || *source == id)
    }
}
