use super::*;

impl Document {
    pub fn set_dimension(&mut self, field: DimensionField, value: f32) -> Result<(), String> {
        if !value.is_finite() {
            return Err("dimension must be finite".into());
        }
        if field == DimensionField::ArcSweep {
            if value == 0.0 || value.abs() >= 360.0 {
                return Err(
                    "arc sweep must be between -360 and 360 degrees, excluding zero".into(),
                );
            }
        } else if field != DimensionField::Rotation && value <= 0.0 {
            return Err("dimension must be positive".into());
        }
        let id = self.selected.ok_or("select geometry first")?;
        if matches!(
            field,
            DimensionField::CircleDiameter | DimensionField::CurveRadius
        ) && self.equal_radii.contains_key(&id)
        {
            return Err("Radius is driven by Equal".into());
        }
        if matches!(
            field,
            DimensionField::CircleDiameter | DimensionField::CurveRadius
        ) && self.radius_differences.contains_key(&id)
        {
            return Err("Radius is driven by Radial distance".into());
        }
        if field == DimensionField::CircleDiameter
            && self
                .driving_dimensions
                .get(&(id, DimensionField::CurveRadius))
                .is_some_and(|radius| (*radius - value * 0.5).abs() > 1e-5)
        {
            return Err("Circle radius has another driving dimension".into());
        }
        if field == DimensionField::CurveRadius
            && self
                .driving_dimensions
                .get(&(id, DimensionField::CircleDiameter))
                .is_some_and(|diameter| (*diameter * 0.5 - value).abs() > 1e-5)
        {
            return Err("Circle diameter has another driving dimension".into());
        }
        if matches!(
            field,
            DimensionField::RectangleWidth | DimensionField::RectangleHeight
        ) && self.equal_ellipses.contains_key(&id)
        {
            return Err("Ellipse axes are driven by Equal".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("selected geometry no longer exists")?;
        let mut next = self.shapes[index].clone();
        match (&mut next, field) {
            (Shape::Line { a, b, .. }, DimensionField::LineLength) => {
                if self.fixed_anchors.contains_key(&(id, 1)) {
                    return Err("line end is fixed".into());
                }
                let old = a.distance(*b);
                if old <= f32::EPSILON {
                    return Err("cannot resize a zero-length line".into());
                }
                let scale = value / old;
                b.x = a.x + (b.x - a.x) * scale;
                b.y = a.y + (b.y - a.y) * scale;
            }
            (Shape::Rectangle { a, b, .. }, DimensionField::RectangleWidth) => {
                let direction = if b.x >= a.x { 1.0 } else { -1.0 };
                match (
                    self.fixed_anchors.contains_key(&(id, 0)),
                    self.fixed_anchors.contains_key(&(id, 1)),
                ) {
                    (true, true) => return Err("both rectangle corners are fixed".into()),
                    (true, false) => b.x = a.x + direction * value,
                    (false, true) => a.x = b.x - direction * value,
                    (false, false) => {
                        let center = (a.x + b.x) * 0.5;
                        a.x = center - direction * value * 0.5;
                        b.x = center + direction * value * 0.5;
                    }
                }
            }
            (Shape::Rectangle { a, b, .. }, DimensionField::RectangleHeight) => {
                let direction = if b.y >= a.y { 1.0 } else { -1.0 };
                match (
                    self.fixed_anchors.contains_key(&(id, 0)),
                    self.fixed_anchors.contains_key(&(id, 1)),
                ) {
                    (true, true) => return Err("both rectangle corners are fixed".into()),
                    (true, false) => b.y = a.y + direction * value,
                    (false, true) => a.y = b.y - direction * value,
                    (false, false) => {
                        let center = (a.y + b.y) * 0.5;
                        a.y = center - direction * value * 0.5;
                        b.y = center + direction * value * 0.5;
                    }
                }
            }
            (Shape::RotatedRectangle { width, .. }, DimensionField::RectangleWidth) => {
                if self
                    .fixed_anchors
                    .keys()
                    .any(|(shape_id, anchor)| *shape_id == id && *anchor > 0)
                {
                    return Err("rectangle corner is fixed".into());
                }
                *width = value
            }
            (Shape::RotatedRectangle { height, .. }, DimensionField::RectangleHeight) => {
                if self
                    .fixed_anchors
                    .keys()
                    .any(|(shape_id, anchor)| *shape_id == id && *anchor > 0)
                {
                    return Err("rectangle corner is fixed".into());
                }
                *height = value
            }
            (Shape::Circle { radius, .. }, DimensionField::CircleDiameter) => {
                *radius = value * 0.5;
            }
            (Shape::Circle { radius, .. }, DimensionField::CurveRadius) => {
                *radius = value;
            }
            (Shape::Arc { radius, .. }, DimensionField::CurveRadius) => {
                if self.fixed_anchors.contains_key(&(id, 1))
                    || self.fixed_anchors.contains_key(&(id, 2))
                {
                    return Err("arc endpoint is fixed".into());
                }
                *radius = value;
            }
            (Shape::Polygon { radius, .. }, DimensionField::CurveRadius) => {
                if self
                    .fixed_anchors
                    .keys()
                    .any(|(shape_id, anchor)| *shape_id == id && *anchor > 0)
                {
                    return Err("polygon vertex is fixed".into());
                }
                *radius = value;
            }
            (
                Shape::Arc { sweep, .. } | Shape::EllipticArc { sweep, .. },
                DimensionField::ArcSweep,
            ) => {
                if self.fixed_anchors.contains_key(&(id, 2)) {
                    return Err("arc end is fixed".into());
                }
                *sweep = (value as f64).to_radians();
            }
            (Shape::Line { a, b, .. }, DimensionField::Rotation) => {
                if self.fixed_anchors.contains_key(&(id, 1)) || self.normal_links.contains_key(&id)
                {
                    return Err("line direction is constrained".into());
                }
                let length = a.distance(*b);
                if length <= 1e-6 || !length.is_finite() {
                    return Err("line length is invalid".into());
                }
                let (sin, cos) = value.to_radians().sin_cos();
                *b = Point {
                    x: a.x + cos * length,
                    y: a.y + sin * length,
                };
                if !b.x.is_finite() || !b.y.is_finite() {
                    return Err("line end is out of range".into());
                }
            }
            (
                Shape::RotatedRectangle { rotation, .. }
                | Shape::Ellipse { rotation, .. }
                | Shape::EllipticArc { rotation, .. }
                | Shape::Polygon { rotation, .. },
                DimensionField::Rotation,
            ) => {
                if self
                    .fixed_anchors
                    .keys()
                    .any(|(shape_id, anchor)| *shape_id == id && *anchor > 0)
                {
                    return Err("shape anchor is fixed".into());
                }
                *rotation = value;
            }
            (Shape::Ellipse { rx, .. }, DimensionField::RectangleWidth) => {
                if self.fixed_anchors.contains_key(&(id, 1))
                    || self.fixed_anchors.contains_key(&(id, 3))
                {
                    return Err("ellipse width axis is fixed".into());
                }
                *rx = value * 0.5;
            }
            (Shape::Ellipse { ry, .. }, DimensionField::RectangleHeight) => {
                if self.fixed_anchors.contains_key(&(id, 2))
                    || self.fixed_anchors.contains_key(&(id, 4))
                {
                    return Err("ellipse height axis is fixed".into());
                }
                *ry = value * 0.5;
            }
            (Shape::EllipticArc { rx, .. }, DimensionField::RectangleWidth) => {
                if self
                    .fixed_anchors
                    .keys()
                    .any(|(shape_id, anchor)| *shape_id == id && *anchor > 0)
                {
                    return Err("elliptic arc endpoint is fixed".into());
                }
                *rx = value * 0.5;
            }
            (Shape::EllipticArc { ry, .. }, DimensionField::RectangleHeight) => {
                if self
                    .fixed_anchors
                    .keys()
                    .any(|(shape_id, anchor)| *shape_id == id && *anchor > 0)
                {
                    return Err("elliptic arc endpoint is fixed".into());
                }
                *ry = value * 0.5;
            }
            _ => return Err("dimension does not apply to selected geometry".into()),
        }
        if matches!(
            field,
            DimensionField::RectangleWidth
                | DimensionField::RectangleHeight
                | DimensionField::Rotation
        ) && self
            .shared_ellipse_partners(id)
            .into_iter()
            .any(|partner| self.driving_dimensions.contains_key(&(partner, field)))
        {
            return Err("Shared ellipse partner already has a driving dimension".into());
        }
        // Relation partners (Equal, radial distance, tangency, shared
        // ellipses, ...) follow through the solver.
        if next != self.shapes[index] || self.driving_dimensions.get(&(id, field)) != Some(&value) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.shapes[index] = next;
            self.driving_dimensions.insert((id, field), value);
            let pin = if matches!(self.shapes[index], Shape::Line { .. }) {
                Pin::Anchor(id, 0)
            } else {
                Pin::Shape(id)
            };
            self.solve_or_rollback(&[pin])?;
        }
        Ok(())
    }
}
