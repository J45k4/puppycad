//! Symmetry relations and the Mirror tool, ported from `src/sketch-mirror.ts`
//! and the `symmetric` / `mirror` relations of `src/sketch-solver.ts`.

use super::*;

/// Reflect `point` across the infinite line through `a` and `b`.
pub fn reflect_point(point: Point, a: Point, b: Point) -> Option<Point> {
    let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
    let squared = dx * dx + dy * dy;
    if !squared.is_finite() || squared < 1e-18 {
        return None;
    }
    let t = ((point.x - a.x) as f64 * dx + (point.y - a.y) as f64 * dy) / squared;
    Some(Point {
        x: (2.0 * (a.x as f64 + t * dx) - point.x as f64) as f32,
        y: (2.0 * (a.y as f64 + t * dy) - point.y as f64) as f32,
    })
}

/// Rotation in degrees mirrored across an axis at `axis` radians, wrapped to
/// (-180, 180].
fn mirror_degrees(rotation: f32, axis: f64) -> f32 {
    let value = 2.0 * axis.to_degrees() - rotation as f64;
    let wrapped = (value + 180.0).rem_euclid(360.0) - 180.0;
    (if wrapped <= -180.0 {
        wrapped + 360.0
    } else {
        wrapped
    }) as f32
}

/// Mirror image of `shape` across the line through `a` and `b`, keeping its
/// id. Like the web app, a corner rectangle becomes a rotated (center)
/// rectangle; every other shape keeps its kind.
pub fn mirror_shape(shape: &Shape, a: Point, b: Point) -> Option<Shape> {
    let reflect = |p: Point| reflect_point(p, a, b);
    let axis = ((b.y - a.y) as f64).atan2((b.x - a.x) as f64);
    let mut result = shape.clone();
    match &mut result {
        Shape::Line { a: p, b: q, .. } => {
            *p = reflect(*p)?;
            *q = reflect(*q)?;
        }
        Shape::Point { at, .. } => *at = reflect(*at)?,
        Shape::Circle { center, .. } => *center = reflect(*center)?,
        Shape::Arc {
            center,
            start_angle,
            sweep,
            ..
        } => {
            *center = reflect(*center)?;
            *start_angle = 2.0 * axis - *start_angle;
            *sweep = -*sweep;
        }
        Shape::Ellipse {
            center, rotation, ..
        } => {
            *center = reflect(*center)?;
            *rotation = mirror_degrees(*rotation, axis);
        }
        Shape::EllipticArc {
            center,
            rotation,
            start_angle,
            sweep,
            ..
        } => {
            *center = reflect(*center)?;
            *rotation = mirror_degrees(*rotation, axis);
            *start_angle = -*start_angle;
            *sweep = -*sweep;
        }
        Shape::Spline { points, .. } => {
            for point in points.iter_mut() {
                *point = reflect(*point)?;
            }
        }
        Shape::Polygon {
            center, rotation, ..
        }
        | Shape::RotatedRectangle {
            center, rotation, ..
        } => {
            *center = reflect(*center)?;
            *rotation = mirror_degrees(*rotation, axis);
        }
        Shape::Capsule { from, to, .. } => {
            *from = reflect(*from)?;
            *to = reflect(*to)?;
        }
        Shape::Rectangle { id, a: p, b: q } => {
            let center = Point {
                x: (p.x + q.x) * 0.5,
                y: (p.y + q.y) * 0.5,
            };
            return Some(Shape::RotatedRectangle {
                id: *id,
                center: reflect(center)?,
                width: (q.x - p.x).abs(),
                height: (q.y - p.y).abs(),
                rotation: mirror_degrees(0.0, axis),
            });
        }
    }
    Some(result)
}

/// Shapes with a single center symmetry anchor (or line/spline anchors).
pub(super) fn center_anchored(shape: &Shape) -> bool {
    matches!(
        shape,
        Shape::Line { .. }
            | Shape::Point { .. }
            | Shape::Circle { .. }
            | Shape::Arc { .. }
            | Shape::Ellipse { .. }
            | Shape::EllipticArc { .. }
            | Shape::Spline { .. }
    )
}

impl Document {
    fn shape(&self, id: u32) -> Option<&Shape> {
        self.shapes.iter().find(|shape| shape.id() == id)
    }

    fn axis_points(&self, axis: u32) -> Result<(Point, Point), String> {
        match self.shape(axis) {
            Some(Shape::Line { a, b, .. }) if a.distance(*b) > 1e-6 => Ok((*a, *b)),
            Some(Shape::Line { .. }) => Err("Mirror line has zero length".into()),
            _ => Err("Pick a line as the mirror axis".into()),
        }
    }

    /// Point of a solver anchor (line ends 0/1, sketch point and curve
    /// center 0, spline handle index).
    pub fn symmetry_anchor_point(&self, (id, index): (u32, usize)) -> Option<Point> {
        match (self.shape(id)?, index) {
            (Shape::Line { a, .. }, 0) => Some(*a),
            (Shape::Line { b, .. }, 1) => Some(*b),
            (Shape::Point { at, .. }, 0) => Some(*at),
            (
                Shape::Circle { center, .. }
                | Shape::Arc { center, .. }
                | Shape::Ellipse { center, .. }
                | Shape::EllipticArc { center, .. },
                0,
            ) => Some(*center),
            (Shape::Spline { points, .. }, index) => points.get(index).copied(),
            _ => None,
        }
    }

    /// Anchor of shape `id` closest to `near`.
    pub(super) fn nearest_symmetry_anchor(&self, id: u32, near: Point) -> Option<(u32, usize)> {
        let count = match self.shape(id)? {
            Shape::Line { .. } => 2,
            Shape::Spline { points, .. } => points.len(),
            shape if center_anchored(shape) => 1,
            _ => 0,
        };
        (0..count)
            .filter_map(|index| {
                self.symmetry_anchor_point((id, index))
                    .map(|p| (index, p.distance(near)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(index, _)| (id, index))
    }

    /// Whether shape `id` is either side of a symmetry or mirror relation.
    pub fn has_symmetry(&self, id: u32) -> bool {
        self.mirrors
            .iter()
            .any(|(&target, &(source, _))| target == id || source == id)
            || self
                .symmetric_points
                .iter()
                .any(|pair| pair.a.0 == id || pair.b.0 == id)
    }

    /// Toggle point symmetry of two anchors about `axis`.
    pub fn toggle_symmetric_points(
        &mut self,
        a: (u32, usize),
        b: (u32, usize),
        axis: u32,
    ) -> Result<bool, String> {
        if let Some(index) = self.symmetric_points.iter().position(|pair| {
            pair.axis == axis && ((pair.a == a && pair.b == b) || (pair.a == b && pair.b == a))
        }) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.symmetric_points.remove(index);
            return Ok(false);
        }
        self.axis_points(axis)?;
        if a == b {
            return Err("Choose two distinct points".into());
        }
        if a.0 == axis || b.0 == axis {
            return Err("The mirror line cannot be one of the symmetric shapes".into());
        }
        if self.symmetry_anchor_point(a).is_none() || self.symmetry_anchor_point(b).is_none() {
            return Err("Pick line ends, points, centers or spline points".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.symmetric_points.push(SymmetricPoints { a, b, axis });
        self.solve_or_rollback(&[Pin::Shape(a.0)])?;
        Ok(true)
    }

    /// Toggle whole-shape symmetry: `target` is kept the mirror image of
    /// `source` across `axis`.
    pub fn toggle_mirror_relation(
        &mut self,
        source: u32,
        target: u32,
        axis: u32,
    ) -> Result<bool, String> {
        let existing = self.mirrors.get(&target).copied();
        if existing.is_some_and(|(s, a)| s == source && a == axis) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.mirrors.remove(&target);
            return Ok(false);
        }
        if let Some((s, a)) = self.mirrors.get(&source).copied()
            && s == target
            && a == axis
        {
            self.history.push(self.snapshot());
            self.future.clear();
            self.mirrors.remove(&source);
            return Ok(false);
        }
        let (start, end) = self.axis_points(axis)?;
        if source == target || source == axis || target == axis {
            return Err("Pick two shapes other than the mirror line".into());
        }
        if existing.is_some() {
            return Err("Shape is already a mirror copy".into());
        }
        let source_shape = self.shape(source).ok_or("Source shape is missing")?;
        let target_shape = self.shape(target).ok_or("Target shape is missing")?;
        let expected = mirror_shape(source_shape, start, end).ok_or("Cannot mirror shape")?;
        if std::mem::discriminant(&expected) != std::mem::discriminant(target_shape) {
            return Err("Mirror needs two shapes of the same kind".into());
        }
        if let (Shape::Spline { points: p, .. }, Shape::Spline { points: q, .. }) =
            (&expected, target_shape)
            && p.len() != q.len()
        {
            return Err("Mirrored splines need the same number of points".into());
        }
        if let (Shape::Polygon { sides: p, .. }, Shape::Polygon { sides: q, .. }) =
            (&expected, target_shape)
            && p != q
        {
            return Err("Mirrored polygons need the same number of sides".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.mirrors.insert(target, (source, axis));
        self.solve_or_rollback(&[Pin::Shape(source)])?;
        Ok(true)
    }

    /// The Shift+S gesture: with a mirror line picked, toggle symmetry
    /// between the shapes under `first` and `second`. Two shapes of the same
    /// kind (other than sketch points) become whole-shape symmetric (web
    /// `mirror`); otherwise their nearest anchors become symmetric points.
    pub fn toggle_symmetry_at(
        &mut self,
        axis: u32,
        first: Point,
        second: Point,
        tolerance: f32,
    ) -> Result<bool, String> {
        let pick = |at: Point| {
            self.shapes
                .iter()
                .rev()
                .find(|shape| shape.id() != axis && shape.hit(at, tolerance))
                .map(Shape::id)
        };
        let source = pick(first).ok_or("Click a shape for the first side")?;
        let target = self
            .shapes
            .iter()
            .rev()
            .find(|shape| {
                shape.id() != axis && shape.id() != source && shape.hit(second, tolerance)
            })
            .map(Shape::id)
            .ok_or("Click a different shape for the second side")?;
        let (source_shape, target_shape) = (
            self.shape(source).ok_or("Shape is missing")?,
            self.shape(target).ok_or("Shape is missing")?,
        );
        let points = matches!(source_shape, Shape::Point { .. })
            || matches!(target_shape, Shape::Point { .. });
        let same_kind = std::mem::discriminant(source_shape)
            == std::mem::discriminant(target_shape)
            || (matches!(source_shape, Shape::Rectangle { .. })
                && matches!(target_shape, Shape::RotatedRectangle { .. }));
        if same_kind && !points {
            return self.toggle_mirror_relation(source, target, axis);
        }
        let a = self
            .nearest_symmetry_anchor(source, first)
            .ok_or("First shape has no point to mirror")?;
        let b = self
            .nearest_symmetry_anchor(target, second)
            .ok_or("Second shape has no point to mirror")?;
        self.toggle_symmetric_points(a, b, axis)
    }

    /// Mirror tool: add a mirror copy of every shape in `ids` across `axis`,
    /// each linked to its source by a mirror relation. One undo step.
    /// Returns the new shape ids.
    pub fn mirror_shapes(&mut self, ids: &[u32], axis: u32) -> Result<Vec<u32>, String> {
        let (start, end) = self.axis_points(axis)?;
        let ids: Vec<u32> = ids.iter().copied().filter(|&id| id != axis).collect();
        if ids.is_empty() {
            return Err("Select geometry other than the mirror line".into());
        }
        let mut copies = Vec::new();
        for &id in &ids {
            let shape = self.shape(id).ok_or("Selected shape is missing")?;
            let next = self.next_id.saturating_add(copies.len() as u32);
            copies.push(
                mirror_shape(shape, start, end)
                    .ok_or("Cannot mirror shape")?
                    .with_id(next),
            );
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let mut created = Vec::new();
        for (source, copy) in ids.into_iter().zip(copies) {
            let id = copy.id();
            if self.construction.contains(&source) {
                self.construction.insert(id);
            }
            self.shapes.push(copy);
            self.mirrors.insert(id, (source, axis));
            created.push(id);
        }
        self.next_id = self.next_id.saturating_add(created.len() as u32);
        self.selected = created.last().copied();
        self.selected_spline_point = None;
        self.solve_or_rollback(&[])?;
        Ok(created)
    }

    /// Solver constraints for every symmetry and mirror relation.
    pub(super) fn symmetry_constraints(
        &self,
        b: &mut super::solve::SolverBuilder<'_>,
        constraints: &mut Vec<solver::Constraint>,
    ) {
        use solver::Constraint;
        let anchor = |b: &mut super::solve::SolverBuilder<'_>, (id, index): (u32, usize)| match (
            b.entity(id)?,
            index,
        ) {
            (super::solve::SolverEntity::Line(line), 0) => Some(line.a),
            (super::solve::SolverEntity::Line(line), 1) => Some(line.b),
            (super::solve::SolverEntity::Point(point), 0) => Some(point),
            (super::solve::SolverEntity::Spline(spline), index) => {
                spline.points.get(index).copied()
            }
            (found, 0) => found.center(),
            _ => None,
        };
        for pair in &self.symmetric_points {
            if let (Some(axis), Some(a), Some(c)) =
                (b.line(pair.axis), anchor(b, pair.a), anchor(b, pair.b))
            {
                constraints.push(Constraint::Symmetric { a, b: c, axis });
            }
        }
        let mut mirrors: Vec<_> = self.mirrors.iter().map(|(&t, &r)| (t, r)).collect();
        mirrors.sort_by_key(|(target, _)| *target);
        for (target, (source, axis_id)) in mirrors {
            let (Some(axis), Some(s), Some(t)) =
                (b.line(axis_id), b.entity(source), b.entity(target))
            else {
                continue;
            };
            let sym = |a, c| Constraint::Symmetric { a, b: c, axis };
            use super::solve::SolverEntity as E;
            match (s, t) {
                (E::Line(s), E::Line(t)) => {
                    constraints.extend([sym(s.a, t.a), sym(s.b, t.b)]);
                }
                (E::Point(s), E::Point(t)) => constraints.push(sym(s, t)),
                (E::Circle(s), E::Circle(t)) => constraints.extend([
                    sym(s.center, t.center),
                    Constraint::SameValue(s.radius, t.radius),
                ]),
                (E::Arc(s), E::Arc(t)) => constraints.extend([
                    sym(s.circle.center, t.circle.center),
                    Constraint::SameValue(s.circle.radius, t.circle.radius),
                    Constraint::MirrorAngle {
                        source: s.start,
                        target: t.start,
                        axis,
                        degrees: false,
                    },
                    Constraint::Opposite(s.sweep, t.sweep),
                ]),
                (E::Ellipse(s), E::Ellipse(t)) => {
                    constraints.extend([
                        sym(s.center, t.center),
                        Constraint::SameValue(s.rx, t.rx),
                        Constraint::SameValue(s.ry, t.ry),
                        Constraint::MirrorAngle {
                            source: s.rotation,
                            target: t.rotation,
                            axis,
                            degrees: true,
                        },
                    ]);
                    if let (Some((ss, sw)), Some((ts, tw))) = (s.arc, t.arc) {
                        constraints
                            .extend([Constraint::Opposite(ss, ts), Constraint::Opposite(sw, tw)]);
                    }
                }
                (E::Shape(source), E::Shape(target)) => {
                    constraints.push(Constraint::MirrorShape {
                        source,
                        target,
                        axis,
                    });
                }
                (E::Spline(s), E::Spline(t)) if s.points.len() == t.points.len() => {
                    constraints.extend(s.points.iter().zip(&t.points).map(|(&a, &c)| sym(a, c)));
                }
                _ => {}
            }
        }
    }

    /// Shapes whose position follows a symmetry relation.
    pub(super) fn symmetry_followers(&self) -> impl Iterator<Item = u32> + '_ {
        self.mirrors
            .keys()
            .copied()
            .chain(self.symmetric_points.iter().map(|pair| pair.b.0))
    }

    /// Remove every symmetry relation that refers to shape `id`.
    pub(super) fn forget_symmetry(&mut self, id: u32) {
        self.mirrors
            .retain(|&target, &mut (source, axis)| target != id && source != id && axis != id);
        self.symmetric_points
            .retain(|pair| pair.a.0 != id && pair.b.0 != id && pair.axis != id);
    }
}
