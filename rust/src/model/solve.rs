use super::*;

/// User intent kept exactly while solving: a whole shape or one anchor
/// (line endpoint 0/1, sketch point 0, circle/arc center 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pin {
    Shape(u32),
    Anchor(u32, usize),
}

/// Result of [`Document::analyze_constraints`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SketchAnalysis {
    pub parameters: usize,
    pub rank: usize,
    pub dof: usize,
    /// Shapes in a relation whose residual is not met.
    pub conflicting: HashSet<u32>,
    /// Shapes in a locally redundant relation.
    pub redundant: HashSet<u32>,
    /// Shapes whose solver parameters are all determined.
    pub fully_constrained: HashSet<u32>,
}

impl SketchAnalysis {
    /// Header label in the web app's spirit.
    pub fn label(&self) -> String {
        if !self.conflicting.is_empty() {
            "CONFLICT".into()
        } else if !self.redundant.is_empty() {
            "OVER-CONSTRAINED".into()
        } else {
            format!("DOF {}", self.dof)
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum SolverEntity {
    Line(solver::LineVar),
    Point(solver::PointVar),
    Circle(solver::CircleVar),
    Arc(solver::ArcVar),
    Ellipse(solver::EllipseVar),
    Spline(solver::SplineVar),
    /// Rectangle, rotated rectangle, polygon or slot.
    Shape(solver::ShapeVar),
}

impl SolverEntity {
    pub(super) fn curve(&self) -> Option<solver::Curve> {
        Some(match self {
            Self::Line(line) => solver::Curve::Line(*line),
            Self::Circle(circle) => solver::Curve::Circle(*circle),
            Self::Arc(arc) => solver::Curve::Arc(*arc),
            Self::Ellipse(ellipse) => solver::Curve::Ellipse(*ellipse),
            Self::Spline(spline) => solver::Curve::Spline(spline.clone()),
            Self::Point(_) | Self::Shape(_) => return None,
        })
    }

    pub(super) fn center(&self) -> Option<solver::PointVar> {
        match self {
            Self::Circle(circle) => Some(circle.center),
            Self::Arc(arc) => Some(arc.circle.center),
            Self::Ellipse(ellipse) => Some(ellipse.center),
            _ => None,
        }
    }
}

/// Builds solver parameters for document shapes on demand.
pub(super) struct SolverBuilder<'a> {
    shapes: &'a [Shape],
    pins: &'a HashSet<Pin>,
    /// Pinned shapes that are relation targets: the edit is kept as a strong
    /// preference so the relation can project the shape back onto its source.
    softened: &'a HashSet<u32>,
    /// Lines whose second endpoint follows a relation (weak second endpoint).
    dependents: &'a HashSet<u32>,
    /// Curves whose radius or axes a relation may change; other curves keep
    /// their size, so tangency and point relations move them rigidly.
    sized: &'a HashSet<u32>,
    /// Curves whose sweep or rotation a relation may change.
    angled: &'a HashSet<u32>,
    /// Anchor being dragged: a strong soft target instead of a hard pin.
    drag: Option<(u32, usize)>,
    problem: solver::Problem,
    entities: HashMap<u32, SolverEntity>,
}

impl SolverBuilder<'_> {
    pub(super) fn param(&mut self, value: f64, locked: bool, weight: f64) -> usize {
        let var = self.problem.param(value, locked);
        self.problem
            .soft
            .push(solver::Soft::Target { var, value, weight });
        var
    }

    pub(super) fn entity(&mut self, id: u32) -> Option<SolverEntity> {
        use solver::{ArcVar, CircleVar, EllipseVar, LineVar, PointVar, SplineVar};
        if let Some(found) = self.entities.get(&id) {
            return Some(found.clone());
        }
        let shape = self.shapes.iter().find(|shape| shape.id() == id)?.clone();
        let softened = self.softened.contains(&id);
        let shape_locked = !softened && self.pins.contains(&Pin::Shape(id));
        let (position, size) = if softened { (10.0, 100.0) } else { (1.0, 0.5) };
        let pins = self.pins;
        let anchor_locked = |index| shape_locked || pins.contains(&Pin::Anchor(id, index));
        let size_locked = shape_locked || !self.sized.contains(&id);
        let angle_locked = shape_locked || !self.angled.contains(&id);
        let drag = self.drag;
        let point = |builder: &mut Self, at: Point, index: usize, weight: f64| {
            let weight = if drag == Some((id, index)) {
                1e3
            } else {
                weight * position
            };
            PointVar {
                x: builder.param(at.x as f64, anchor_locked(index), weight),
                y: builder.param(at.y as f64, anchor_locked(index), weight),
            }
        };
        let created = match shape {
            Shape::Rectangle { a, b, .. } => SolverEntity::Shape(solver::ShapeVar::Rectangle {
                a: point(self, a, 0, 1.0),
                b: point(self, b, 1, 1.0),
            }),
            Shape::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => {
                let scale = ((width + height) as f64 * 0.25).max(1.0) * position;
                let center = point(self, center, 0, 1.0);
                SolverEntity::Shape(solver::ShapeVar::RotatedRectangle {
                    center,
                    half_width: self.param(width as f64 * 0.5, size_locked, size),
                    half_height: self.param(height as f64 * 0.5, size_locked, size),
                    rotation: self.param(
                        rotation as f64,
                        angle_locked,
                        scale * std::f64::consts::PI / 180.0 * 10.0,
                    ),
                })
            }
            Shape::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            } => {
                let scale = (radius as f64).max(1.0) * position;
                let center = point(self, center, 0, 1.0);
                SolverEntity::Shape(solver::ShapeVar::Polygon {
                    center,
                    radius: self.param(radius as f64, size_locked, size),
                    rotation: self.param(
                        rotation as f64,
                        angle_locked,
                        scale * std::f64::consts::PI / 180.0 * 10.0,
                    ),
                    sides,
                })
            }
            Shape::Capsule {
                from, to, width, ..
            } => SolverEntity::Shape(solver::ShapeVar::Slot {
                a: point(self, from, 0, 1.0),
                b: point(self, to, 1, 1.0),
                width: self.param(width as f64, size_locked, size),
            }),
            // Relation targets rotate/stretch about their first endpoint;
            // other lines (relation sources, free lines) hold both ends.
            Shape::Line { a, b, .. } => {
                let weak = if self.dependents.contains(&id) {
                    1e-3
                } else {
                    1.0
                };
                SolverEntity::Line(LineVar {
                    a: point(self, a, 0, 1.0),
                    b: point(self, b, 1, weak),
                })
            }
            Shape::Point { at, .. } => SolverEntity::Point(point(self, at, 0, 1.0)),
            Shape::Circle { center, radius, .. } => {
                let center = point(self, center, 0, 1.0);
                let radius = self.param(radius as f64, size_locked, size);
                SolverEntity::Circle(CircleVar { center, radius })
            }
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => {
                let center = point(self, center, 0, 1.0);
                let angular = (radius as f64).max(1.0) * position;
                let radius = self.param(radius as f64, size_locked, size);
                SolverEntity::Arc(ArcVar {
                    circle: CircleVar { center, radius },
                    start: self.param(start_angle, angle_locked, angular),
                    sweep: self.param(sweep, angle_locked, angular * 10.0),
                })
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
                let scale = ((rx + ry) as f64 * 0.5).max(1.0) * position;
                let center = point(self, center, 0, 1.0);
                let rx = self.param(rx as f64, size_locked, size);
                let ry = self.param(ry as f64, size_locked, size);
                let rotation = self.param(
                    rotation as f64,
                    angle_locked,
                    scale * std::f64::consts::PI / 180.0 * 10.0,
                );
                let arc = match shape {
                    Shape::EllipticArc {
                        start_angle, sweep, ..
                    } => Some((
                        self.param(start_angle, angle_locked, scale),
                        self.param(sweep, angle_locked, scale * 10.0),
                    )),
                    _ => None,
                };
                SolverEntity::Ellipse(EllipseVar {
                    center,
                    rx,
                    ry,
                    rotation,
                    arc,
                })
            }
            Shape::Spline {
                mode,
                ref points,
                closed,
                ..
            } => SolverEntity::Spline(SplineVar {
                points: points
                    .iter()
                    .enumerate()
                    .map(|(index, &at)| point(self, at, index, 1.0))
                    .collect(),
                control: mode == SplineMode::Control,
                closed,
            }),
        };
        self.entities.insert(id, created.clone());
        Some(created)
    }

    pub(super) fn line(&mut self, id: u32) -> Option<solver::LineVar> {
        match self.entity(id)? {
            SolverEntity::Line(line) => Some(line),
            _ => None,
        }
    }

    pub(super) fn point(&mut self, id: u32) -> Option<solver::PointVar> {
        match self.entity(id)? {
            SolverEntity::Point(point) => Some(point),
            _ => None,
        }
    }

    /// Circle or arc (radius relations treat both alike).
    pub(super) fn circle(&mut self, id: u32) -> Option<solver::CircleVar> {
        match self.entity(id)? {
            SolverEntity::Circle(circle) => Some(circle),
            SolverEntity::Arc(arc) => Some(arc.circle),
            _ => None,
        }
    }

    pub(super) fn ellipse(&mut self, id: u32) -> Option<solver::EllipseVar> {
        match self.entity(id)? {
            SolverEntity::Ellipse(ellipse) => Some(ellipse),
            _ => None,
        }
    }

    pub(super) fn curve(&mut self, id: u32) -> Option<solver::Curve> {
        self.entity(id)?.curve()
    }

    /// Auxiliary spline parameter for a point that must lie on `curve`.
    pub(super) fn curve_param(
        &mut self,
        curve: &solver::Curve,
        near: solver::PointVar,
    ) -> Option<usize> {
        let solver::Curve::Spline(spline) = curve else {
            return None;
        };
        let p = &self.problem.params;
        let u = solver::spline_parameter(p, spline, (p[near.x], p[near.y]));
        Some(self.param(u, false, 1e-3))
    }
}

impl SolverEntity {
    pub(super) fn vars(&self) -> Vec<usize> {
        match self {
            SolverEntity::Line(l) => vec![l.a.x, l.a.y, l.b.x, l.b.y],
            SolverEntity::Point(p) => vec![p.x, p.y],
            SolverEntity::Circle(c) => vec![c.center.x, c.center.y, c.radius],
            SolverEntity::Arc(a) => vec![
                a.circle.center.x,
                a.circle.center.y,
                a.circle.radius,
                a.start,
                a.sweep,
            ],
            SolverEntity::Ellipse(e) => {
                let mut vars = vec![e.center.x, e.center.y, e.rx, e.ry, e.rotation];
                if let Some((start, sweep)) = e.arc {
                    vars.extend([start, sweep]);
                }
                vars
            }
            SolverEntity::Spline(s) => s.points.iter().flat_map(|p| [p.x, p.y]).collect(),
            SolverEntity::Shape(shape) => shape.vars(),
        }
    }
}

impl Document {
    /// Curves whose size (radius, axes) and whose angles (sweep, rotation)
    /// some relation or driving value may change during a solve.
    pub(super) fn solver_free_curve_parameters(&self) -> (HashSet<u32>, HashSet<u32>) {
        let mut sized: HashSet<u32> = self
            .equal_radii
            .iter()
            .chain(self.equal_ellipses.iter())
            .chain(self.shared_ellipses.iter())
            .flat_map(|(&a, &b)| [a, b])
            .chain(
                self.radius_differences
                    .iter()
                    .flat_map(|(&target, &(source, _))| [target, source]),
            )
            .collect();
        let mut angled: HashSet<u32> = self
            .shared_ellipses
            .iter()
            .flat_map(|(&a, &b)| [a, b])
            .chain(self.endpoint_tangents.keys().map(|(id, _)| *id))
            .collect();
        for (&(follower, _), &(source, _)) in &self.smooth_joins {
            sized.extend([follower, source]);
            angled.extend([follower, source]);
        }
        for (&target, &(source, _)) in &self.mirrors {
            sized.extend([target, source]);
            angled.extend([target, source]);
        }
        for id in self.offset_curves() {
            sized.insert(id);
            angled.insert(id);
        }
        for &(id, field) in self.driving_dimensions.keys() {
            match field {
                DimensionField::CircleDiameter
                | DimensionField::CurveRadius
                | DimensionField::RectangleWidth
                | DimensionField::RectangleHeight => {
                    sized.insert(id);
                }
                DimensionField::ArcSweep | DimensionField::Rotation => {
                    angled.insert(id);
                }
                _ => {}
            }
        }
        for &(id, index) in self.fixed_anchors.keys() {
            if index > 0 {
                sized.insert(id);
                angled.insert(id);
            }
        }
        (sized, angled)
    }

    /// Shapes that are the target of a relation that projects them onto
    /// their source. When such a shape is the edited one, its pin is softened
    /// so the relation can pull it back (the old "snap the target" behavior).
    pub(super) fn projected_targets(&self) -> HashSet<u32> {
        self.point_curve_links
            .keys()
            .chain(self.normal_links.keys())
            .chain(self.internal_tangents.keys())
            .chain(self.external_tangents.keys())
            .copied()
            .chain(self.endpoint_tangents.keys().map(|(id, _)| *id))
            .chain(self.smooth_joins.keys().map(|(id, _)| *id))
            .collect()
    }

    /// Re-solve every relation after an edit. Pinned shapes and anchors stay
    /// exactly where the edit put them (pinned targets of projecting
    /// relations such as point-on-curve or tangency are kept as a strong
    /// preference instead); everything else moves as little as possible.
    /// Intent is applied as a priority cascade that keeps the old directed
    /// propagation where it still works:
    ///
    /// 1. relation sources (shapes that are not the target of a directed
    ///    relation or a point link) stay put and only targets move;
    /// 2. otherwise sources may move but keep their first endpoint;
    /// 3. otherwise only the explicit pins are held.
    ///
    /// Geometry is untouched when every tier fails.
    pub(crate) fn solve_constraints(&mut self, pins: &[Pin]) -> Result<(), String> {
        let followers: HashSet<u32> = self
            .equal_lines
            .keys()
            .chain(self.line_relations.keys())
            .chain(self.equal_radii.keys())
            .chain(self.radius_differences.keys())
            .chain(self.concentric.keys())
            .chain(self.midpoint_links.keys())
            .chain(self.equal_ellipses.keys())
            .chain(self.shared_ellipses.keys())
            .chain(self.point_links.values())
            .chain(self.chamfers.keys())
            .chain(
                self.dims
                    .point_dimensions
                    .iter()
                    .filter(|(id, _)| !self.dims.references.contains(&DimensionKey::Point(**id)))
                    .map(|(_, d)| &d.b.0),
            )
            .copied()
            .chain(self.symmetry_followers())
            .chain(self.offset_followers())
            .chain(self.pattern_followers())
            .chain(self.endpoint_joins.values().map(|(id, _)| *id))
            .chain(self.projected_targets())
            .collect();
        let sources: Vec<u32> = self
            .shapes
            .iter()
            .map(Shape::id)
            .filter(|id| !followers.contains(id))
            .collect();
        let explicit: HashSet<Pin> = pins.iter().copied().collect();
        // Seed direction targets with the old closed-form rule (rotate about
        // the first endpoint, keep length). It is only an initial guess, but
        // it keeps quarter-turn edits away from degenerate local minima.
        let original = self.shapes.clone();
        let pinned = |id: u32| explicit.contains(&Pin::Shape(id));
        let mut relations: Vec<_> = self.line_relations.iter().map(|(&t, &r)| (t, r)).collect();
        relations.sort_by_key(|(target, _)| *target);
        for _ in 0..2 {
            for &(target, (source, kind)) in &relations {
                if pinned(target) {
                    continue;
                }
                let source = self.shapes.iter().find(|shape| shape.id() == source);
                let index = self.shapes.iter().position(|shape| shape.id() == target);
                if let (Some(source), Some(index)) = (source, index)
                    && let Some(next) = Self::line_relation_shape(source, &self.shapes[index], kind)
                {
                    self.shapes[index] = next;
                }
            }
        }
        let mut error = String::new();
        // Mirror lines stay put while either side of a symmetry is edited
        // (otherwise moving the axis is often the cheapest solution).
        let axes: Vec<u32> = self
            .mirrors
            .values()
            .map(|&(_, axis)| axis)
            .chain(self.symmetric_points.iter().map(|pair| pair.axis))
            .collect();
        for tier in 0..4 {
            if tier == 2 && axes.is_empty() {
                continue;
            }
            let mut tier_pins = explicit.clone();
            match tier {
                0 => tier_pins.extend(sources.iter().map(|&id| Pin::Shape(id))),
                1 => tier_pins.extend(sources.iter().map(|&id| Pin::Anchor(id, 0))),
                2 => tier_pins.extend(axes.iter().map(|&id| Pin::Shape(id))),
                _ => {}
            }
            match self.solve_once(&tier_pins, None) {
                Ok(()) => return Ok(()),
                Err(message) => error = message,
            }
        }
        self.shapes = original;
        Err(error)
    }

    /// Build the solver problem for the current document. With `everything`,
    /// every line, point, circle and arc becomes an entity (for DOF
    /// analysis) instead of only those that take part in a relation.
    /// `drag` is an anchor that follows the cursor as a strong soft target
    /// instead of a hard pin, so it slides along what the relations allow.
    /// `drag` is an anchor that follows the cursor as a strong soft target
    /// instead of a hard pin, so it slides along what the relations allow.
    pub(super) fn build_problem(
        &self,
        pins: &HashSet<Pin>,
        everything: bool,
        drag: Option<(u32, usize)>,
    ) -> (solver::Problem, HashMap<u32, SolverEntity>) {
        use solver::{Constraint, Curve, Soft, Value};
        let dependents: HashSet<u32> = self
            .equal_lines
            .keys()
            .chain(self.line_relations.keys())
            .chain(self.axis_constraints.keys())
            .chain(self.normal_links.keys())
            .copied()
            .chain(
                self.driving_dimensions
                    .keys()
                    .filter(|(_, field)| {
                        matches!(field, DimensionField::LineLength | DimensionField::Rotation)
                    })
                    .map(|(id, _)| *id),
            )
            .collect();
        let (mut sized, mut angled) = self.solver_free_curve_parameters();
        if everything {
            // DOF analysis counts every curve parameter as free.
            sized.extend(self.shapes.iter().map(Shape::id));
            angled.extend(self.shapes.iter().map(Shape::id));
        }
        if let Some((id, _)) = drag {
            // A dragged anchor may resize or turn its shape.
            sized.insert(id);
            angled.insert(id);
        }
        let projected = self.projected_targets();
        let softened: HashSet<u32> = pins
            .iter()
            .filter_map(|pin| match pin {
                Pin::Shape(id) if projected.contains(id) => Some(*id),
                _ => None,
            })
            .collect();
        let mut b = SolverBuilder {
            shapes: &self.shapes,
            pins,
            softened: &softened,
            drag,
            dependents: &dependents,
            sized: &sized,
            angled: &angled,
            problem: solver::Problem::default(),
            entities: HashMap::new(),
        };
        let mut keep_length: HashSet<u32> = HashSet::new();
        let mut constraints = Vec::new();
        let is_line = |id: u32| {
            self.shapes
                .iter()
                .any(|shape| shape.id() == id && matches!(shape, Shape::Line { .. }))
        };
        if everything {
            for shape in &self.shapes {
                b.entity(shape.id());
            }
        }
        for (&id, &axis) in &self.axis_constraints {
            if let Some(line) = b.line(id) {
                keep_length.insert(id);
                constraints.push(match axis {
                    AxisConstraint::Horizontal => Constraint::Horizontal(line),
                    AxisConstraint::Vertical => Constraint::Vertical(line),
                });
            }
        }
        for (&target, &source) in &self.equal_lines {
            if let (Some(t), Some(s)) = (b.line(target), b.line(source)) {
                constraints.push(Constraint::EqualLength(s, t));
            }
        }
        for (&target, &(source, kind)) in &self.line_relations {
            let (Some(t), Some(s)) = (b.line(target), b.line(source)) else {
                continue;
            };
            keep_length.extend([target, source]);
            constraints.push(match kind {
                LineRelationKind::Parallel => Constraint::Parallel(s, t),
                LineRelationKind::Perpendicular => Constraint::Perpendicular(s, t),
                LineRelationKind::Collinear => Constraint::Collinear(s, t),
                LineRelationKind::Angle(degrees) => Constraint::Angle {
                    from: s,
                    to: t,
                    degrees: Value::Const(degrees as f64),
                },
            });
        }
        for (&(id, field), &value) in &self.driving_dimensions {
            let value = value as f64;
            match field {
                DimensionField::LineLength => {
                    if let Some(line) = b.line(id) {
                        constraints.push(Constraint::Length {
                            line,
                            value: Value::Const(value),
                        });
                    }
                }
                DimensionField::Rotation if is_line(id) => {
                    if let Some(line) = b.line(id) {
                        keep_length.insert(id);
                        constraints.push(Constraint::Rotation {
                            line,
                            degrees: Value::Const(value),
                        });
                    }
                }
                DimensionField::CircleDiameter | DimensionField::CurveRadius => {
                    if let Some(circle) = b.circle(id) {
                        let value = if field == DimensionField::CircleDiameter {
                            value * 0.5
                        } else {
                            value
                        };
                        constraints.push(Constraint::Radius {
                            circle,
                            value: Value::Const(value),
                        });
                    }
                }
                _ => {}
            }
        }
        for (id, dimension) in &self.dims.point_dimensions {
            if self.dims.references.contains(&DimensionKey::Point(*id)) {
                continue;
            }
            let anchor = |b: &mut SolverBuilder, (id, index): (u32, usize)| match b.entity(id)? {
                SolverEntity::Line(line) => match index {
                    0 => Some(line.a),
                    1 => Some(line.b),
                    _ => None,
                },
                SolverEntity::Point(point) if index == 0 => Some(point),
                SolverEntity::Spline(spline) => spline.points.get(index).copied(),
                found if index == 0 => found.center(),
                _ => None,
            };
            let (Some(a), Some(p)) = (anchor(&mut b, dimension.a), anchor(&mut b, dimension.b))
            else {
                continue;
            };
            let value = Value::Const(dimension.value as f64);
            constraints.push(match dimension.axis {
                None => Constraint::PointDistance { a, b: p, value },
                Some(axis) => Constraint::AxisDistance {
                    a,
                    b: p,
                    vertical: axis == PointAxis::Y,
                    value,
                },
            });
        }
        for (&(curve, endpoint), &point_id) in &self.point_links {
            let end = match b.entity(curve) {
                Some(SolverEntity::Line(line)) => Some(if endpoint == 0 { line.a } else { line.b }),
                Some(SolverEntity::Spline(spline)) if !spline.closed => {
                    if endpoint == 0 {
                        spline.points.first().copied()
                    } else {
                        spline.points.last().copied()
                    }
                }
                _ => None,
            };
            if let (Some(end), Some(point)) = (end, b.point(point_id)) {
                constraints.push(Constraint::Coincident(end, point));
            }
        }
        for (&point_id, &source) in &self.midpoint_links {
            let (Some(curve), Some(point)) = (b.curve(source), b.point(point_id)) else {
                continue;
            };
            constraints.push(match curve {
                Curve::Line(line) => Constraint::Midpoint { point, line },
                curve => Constraint::CurveMidpoint { point, curve },
            });
        }
        for (&point_id, &source) in &self.point_curve_links {
            let (Some(curve), Some(point)) = (b.curve(source), b.point(point_id)) else {
                continue;
            };
            let param = b.curve_param(&curve, point);
            constraints.push(Constraint::PointOnCurve {
                point,
                curve,
                param,
            });
        }
        for (&target, &(source, endpoint)) in &self.normal_links {
            let (Some(line), Some(curve)) = (b.line(target), b.curve(source)) else {
                continue;
            };
            keep_length.insert(target);
            let param = b.curve_param(&curve, if endpoint == 0 { line.a } else { line.b });
            constraints.push(Constraint::Normal {
                line,
                endpoint,
                curve,
                param,
            });
        }
        for (&(target, endpoint), &(source, line_sign, sweep_sign)) in &self.endpoint_tangents {
            if let (Some(SolverEntity::Arc(arc)), Some(line)) = (b.entity(target), b.line(source)) {
                keep_length.insert(source);
                constraints.push(Constraint::EndpointTangent {
                    line,
                    arc,
                    endpoint,
                    line_sign: line_sign as f64,
                    sweep_sign: sweep_sign as f64,
                });
            }
        }
        for (&(line_id, line_end), &(other, other_end)) in &self.endpoint_joins {
            let Some(line) = b.line(line_id) else {
                continue;
            };
            let end = if line_end == 0 { line.a } else { line.b };
            match b.entity(other) {
                Some(SolverEntity::Line(other)) => {
                    let other = if other_end == 0 { other.a } else { other.b };
                    constraints.push(Constraint::Coincident(end, other));
                }
                Some(SolverEntity::Arc(arc)) => constraints.push(Constraint::ArcEndpoint {
                    arc,
                    endpoint: other_end,
                    point: end,
                }),
                _ => {}
            }
        }
        for (&(follower, follower_end), &(source, source_end)) in &self.smooth_joins {
            if let (Some(a), Some(b_curve)) = (b.curve(source), b.curve(follower)) {
                constraints.push(Constraint::SmoothJoin {
                    a,
                    a_end: source_end,
                    b: b_curve,
                    b_end: follower_end,
                });
            }
        }
        for spec in self.chamfers.values() {
            if let (Some(a), Some(line_b)) = (b.line(spec.a.0), b.line(spec.b.0)) {
                constraints.push(Constraint::Chamfer {
                    a,
                    a_end: spec.a.1,
                    b: line_b,
                    b_end: spec.b.1,
                    first: spec.first as f64,
                    second: spec.second as f64,
                    angle: spec.mode == ChamferMode::DistanceAngle,
                });
            }
        }
        for (&inner, &outer) in &self.internal_tangents {
            if let (Some(outer), Some(inner)) = (b.curve(outer), b.curve(inner)) {
                constraints.push(Constraint::InternalTangent { outer, inner });
            }
        }
        for (&target, &source) in &self.external_tangents {
            let (Some(first), Some(second)) = (b.curve(source), b.curve(target)) else {
                continue;
            };
            // The curve stays on the side of the line it is on now.
            let p = &b.problem.params;
            let side = match (&first, &second) {
                (Curve::Line(line), other) | (other, Curve::Line(line)) => {
                    let center = match other {
                        Curve::Circle(circle) => Some(circle.center),
                        Curve::Arc(arc) => Some(arc.circle.center),
                        Curve::Ellipse(ellipse) => Some(ellipse.center),
                        _ => None,
                    };
                    center.map_or(1.0, |c| {
                        let (dx, dy) = (p[line.b.x] - p[line.a.x], p[line.b.y] - p[line.a.y]);
                        let signed = (p[c.x] - p[line.a.x]) * -dy + (p[c.y] - p[line.a.y]) * dx;
                        if signed < 0.0 { -1.0 } else { 1.0 }
                    })
                }
                _ => 1.0,
            };
            if matches!(first, Curve::Line(_)) {
                keep_length.insert(source);
            }
            if matches!(second, Curve::Line(_)) {
                keep_length.insert(target);
            }
            constraints.push(Constraint::ExternalTangent {
                first,
                second,
                side,
            });
        }
        for (&target, &source) in &self.equal_radii {
            if let (Some(t), Some(s)) = (b.circle(target), b.circle(source)) {
                constraints.push(Constraint::EqualRadius(s, t));
            }
        }
        for (&target, &(source, difference)) in &self.radius_differences {
            if let (Some(t), Some(s)) = (b.circle(target), b.circle(source)) {
                constraints.push(Constraint::RadiusDifference {
                    source: s,
                    target: t,
                    value: Value::Const(difference as f64),
                });
            }
        }
        for (&target, &source) in &self.concentric {
            let t = b.entity(target).and_then(|found| found.center());
            let s = b.entity(source).and_then(|found| found.center());
            if let (Some(t), Some(s)) = (t, s) {
                constraints.push(Constraint::Coincident(s, t));
            }
        }
        self.symmetry_constraints(&mut b, &mut constraints);
        self.offset_constraints(&mut b, &mut constraints);
        for (&target, &source) in &self.equal_ellipses {
            if let (Some(t), Some(s)) = (b.ellipse(target), b.ellipse(source)) {
                constraints.push(Constraint::SameValue(s.rx, t.rx));
                constraints.push(Constraint::SameValue(s.ry, t.ry));
            }
        }
        for (&target, &root) in &self.shared_ellipses {
            if let (Some(t), Some(s)) = (b.ellipse(target), b.ellipse(root)) {
                constraints.extend([
                    Constraint::Coincident(s.center, t.center),
                    Constraint::SameValue(s.rx, t.rx),
                    Constraint::SameValue(s.ry, t.ry),
                    Constraint::SameValue(s.rotation, t.rotation),
                ]);
            }
        }
        // Angular and axis dimensions of curves that already take part in a
        // relation (curves outside relations keep their direct edits).
        for (&(id, field), &value) in &self.driving_dimensions {
            let value = value as f64;
            let constraint = match (b.entities.get(&id), field) {
                (Some(SolverEntity::Arc(arc)), DimensionField::ArcSweep) => Constraint::Equal {
                    var: arc.sweep,
                    value: Value::Const(value.to_radians()),
                },
                (Some(SolverEntity::Ellipse(e)), DimensionField::ArcSweep) => match e.arc {
                    Some((_, sweep)) => Constraint::Equal {
                        var: sweep,
                        value: Value::Const(value.to_radians()),
                    },
                    None => continue,
                },
                (Some(SolverEntity::Ellipse(e)), DimensionField::RectangleWidth) => {
                    Constraint::Equal {
                        var: e.rx,
                        value: Value::Const(value * 0.5),
                    }
                }
                (Some(SolverEntity::Ellipse(e)), DimensionField::RectangleHeight) => {
                    Constraint::Equal {
                        var: e.ry,
                        value: Value::Const(value * 0.5),
                    }
                }
                (Some(SolverEntity::Ellipse(e)), DimensionField::Rotation) => Constraint::Equal {
                    var: e.rotation,
                    value: Value::Const(value),
                },
                (Some(SolverEntity::Shape(shape)), field) => {
                    use solver::ShapeVar as S;
                    let p = &b.problem.params;
                    match (*shape, field) {
                        (
                            S::Rectangle { a, b: c },
                            DimensionField::RectangleWidth | DimensionField::RectangleHeight,
                        ) => {
                            let vertical = field == DimensionField::RectangleHeight;
                            let current = if vertical {
                                p[c.y] - p[a.y]
                            } else {
                                p[c.x] - p[a.x]
                            };
                            Constraint::AxisDistance {
                                a,
                                b: c,
                                vertical,
                                value: Value::Const(if current < 0.0 { -value } else { value }),
                            }
                        }
                        (
                            S::RotatedRectangle { half_width, .. },
                            DimensionField::RectangleWidth,
                        ) => Constraint::Equal {
                            var: half_width,
                            value: Value::Const(value * 0.5),
                        },
                        (
                            S::RotatedRectangle { half_height, .. },
                            DimensionField::RectangleHeight,
                        ) => Constraint::Equal {
                            var: half_height,
                            value: Value::Const(value * 0.5),
                        },
                        (
                            S::RotatedRectangle { rotation, .. } | S::Polygon { rotation, .. },
                            DimensionField::Rotation,
                        ) => Constraint::Equal {
                            var: rotation,
                            value: Value::Const(value),
                        },
                        (S::Polygon { radius, .. }, DimensionField::CurveRadius) => {
                            Constraint::Equal {
                                var: radius,
                                value: Value::Const(value),
                            }
                        }
                        _ => continue,
                    }
                }
                _ => continue,
            };
            constraints.push(constraint);
        }
        // Fixed anchors constrain only shapes that already take part in a
        // relation; isolated shapes keep their edit guards.
        for (&(id, index), &at) in &self.fixed_anchors {
            let Some(found) = b.entities.get(&id).cloned() else {
                continue;
            };
            let at = (at.x as f64, at.y as f64);
            if let SolverEntity::Shape(shape) = found {
                constraints.push(Constraint::AnchorAt { shape, index, at });
                continue;
            }
            let anchor = match (&found, index) {
                (SolverEntity::Line(line), 0) => Some(line.a),
                (SolverEntity::Line(line), 1) => Some(line.b),
                (SolverEntity::Point(point), 0) => Some(*point),
                (SolverEntity::Spline(spline), index) => spline.points.get(index).copied(),
                (found, 0) => found.center(),
                _ => None,
            };
            if let Some(point) = anchor {
                constraints.push(Constraint::Fixed { point, at });
                continue;
            }
            let fraction = match (&found, index) {
                (SolverEntity::Arc(_), 1 | 2) => (index - 1) as f64,
                (SolverEntity::Ellipse(e), 1 | 2) if e.arc.is_some() => (index - 1) as f64,
                (SolverEntity::Ellipse(e), 1..=4) if e.arc.is_none() => (index - 1) as f64 / 4.0,
                _ => continue,
            };
            if let Some(curve) = found.curve() {
                constraints.push(Constraint::FixedOnCurve {
                    curve,
                    fraction,
                    at,
                });
            }
        }
        // Lines whose length is decided by Equal or a length dimension do not
        // get a keep-length preference, which would fight that relation.
        keep_length.retain(|id| {
            !self.equal_lines.contains_key(id)
                && !self.equal_lines.values().any(|source| source == id)
                && !self
                    .driving_dimensions
                    .contains_key(&(*id, DimensionField::LineLength))
        });
        for id in keep_length {
            if let Some(SolverEntity::Line(line)) = b.entities.get(&id) {
                let p = &b.problem.params;
                let length = (p[line.b.x] - p[line.a.x]).hypot(p[line.b.y] - p[line.a.y]);
                b.problem.soft.push(Soft::KeepLength {
                    line: *line,
                    length,
                    weight: 1.0,
                });
            }
        }
        // A dragged corner, vertex or rotated-rectangle anchor is derived
        // from the shape parameters: follow it as a strong soft target.
        if let Some((id, index)) = drag
            && let Some(SolverEntity::Shape(shape)) = b.entities.get(&id).cloned()
        {
            let point_param = match shape {
                solver::ShapeVar::Rectangle { .. } | solver::ShapeVar::Slot { .. } => index <= 1,
                _ => index == 0,
            };
            if !point_param && let Some(at) = solver::shape_anchor(&b.problem.params, &shape, index)
            {
                b.problem.soft.push(Soft::Anchor {
                    shape,
                    index,
                    at,
                    weight: 1e3,
                });
            }
        }
        let SolverBuilder {
            mut problem,
            entities,
            ..
        } = b;
        // Projected entities follow the body, not the solver.
        for id in self.projections.keys() {
            if let Some(found) = entities.get(id) {
                for var in found.vars() {
                    problem.locked[var] = true;
                }
            }
        }
        problem.constraints = constraints;
        (problem, entities)
    }

    /// Degrees-of-freedom and relation diagnostics for the current geometry.
    /// Covers every solver-modeled shape: lines, points, circles, arcs,
    /// ellipses, elliptic arcs and splines. Run once per committed edit.
    pub fn analyze_constraints(&self) -> SketchAnalysis {
        let (mut problem, entities) = self.build_problem(&HashSet::new(), true, None);
        // Pattern copies follow their sources (they are regenerated after
        // every solve), so their parameters are not free.
        let instances: HashSet<u32> = self.pattern_followers().collect();
        for id in &instances {
            if let Some(found) = entities.get(id) {
                for var in found.vars() {
                    problem.locked[var] = true;
                }
            }
        }
        let analysis = problem.analyze(&problem.params, 1e-3);
        let mut owner = vec![0u32; problem.params.len()];
        for (&id, found) in &entities {
            for var in found.vars() {
                owner[var] = id;
            }
        }
        let mut result = SketchAnalysis {
            parameters: analysis.parameters,
            rank: analysis.rank,
            dof: analysis.dof,
            ..SketchAnalysis::default()
        };
        for (constraint, state) in problem.constraints.iter().zip(&analysis.relations) {
            let target = match state {
                solver::RelationState::Satisfied => continue,
                solver::RelationState::Conflicting => &mut result.conflicting,
                solver::RelationState::Redundant => &mut result.redundant,
            };
            target.extend(constraint.vars().into_iter().map(|var| owner[var]));
        }
        for (&id, found) in &entities {
            let vars = found.vars();
            if vars.iter().all(|&var| analysis.fixed[var]) {
                result.fully_constrained.insert(id);
            }
        }
        // A copy is as constrained as its pattern's sources.
        for pattern in &self.patterns {
            let sources_fixed = pattern
                .sources
                .iter()
                .all(|id| result.fully_constrained.contains(id));
            for id in pattern.instances.iter().flatten() {
                if !sources_fixed {
                    result.fully_constrained.remove(id);
                }
            }
        }
        result
    }

    pub(super) fn solve_once(
        &mut self,
        pins: &HashSet<Pin>,
        drag: Option<(u32, usize)>,
    ) -> Result<(), String> {
        let (problem, entities) = self.build_problem(pins, false, drag);
        if problem.constraints.is_empty() {
            self.propagate_patterns();
            return Ok(());
        }
        // Stored geometry is f32, so pinned input carries ~1e-6 relative
        // round-off; accept residuals at that scale.
        let result = problem.solve(solver::Options {
            tolerance: 1e-4,
            ..solver::Options::default()
        });
        if !result.converged {
            return Err(format!(
                "Constraints cannot be satisfied (residual {:.3e})",
                result.residual_norm
            ));
        }
        let p = &result.params;
        if p.iter()
            .any(|value| !value.is_finite() || value.abs() >= f32::MAX as f64)
        {
            return Err("Solved geometry is out of range".into());
        }
        // Round-off below the solve tolerance around zero is stored as zero.
        let p: Vec<f64> = p
            .iter()
            .map(|&v| if v.abs() < 1e-9 { 0.0 } else { v })
            .collect();
        let p = &p;
        let read = |var: solver::PointVar| Point {
            x: p[var.x] as f32,
            y: p[var.y] as f32,
        };
        for found in entities.values() {
            match found {
                SolverEntity::Circle(circle) if p[circle.radius] <= 1e-6 => {
                    return Err("Solved radius is not positive".into());
                }
                SolverEntity::Arc(arc)
                    if p[arc.circle.radius] <= 1e-6
                        || p[arc.sweep].abs() <= 1e-9
                        || p[arc.sweep].abs() >= std::f64::consts::TAU =>
                {
                    return Err("Solved arc is invalid".into());
                }
                SolverEntity::Ellipse(e) if p[e.rx] <= 1e-6 || p[e.ry] <= 1e-6 => {
                    return Err("Solved ellipse axes are not positive".into());
                }
                SolverEntity::Shape(shape) => {
                    use solver::ShapeVar as S;
                    let valid = match *shape {
                        S::Rectangle { a, b } => {
                            (p[b.x] - p[a.x]).abs() > 1e-4 && (p[b.y] - p[a.y]).abs() > 1e-4
                        }
                        S::RotatedRectangle {
                            half_width,
                            half_height,
                            ..
                        } => p[half_width] > 1e-6 && p[half_height] > 1e-6,
                        S::Polygon { radius, .. } => p[radius] > 1e-6,
                        S::Slot { a, b, width } => {
                            p[width] > 1e-6 && (p[b.x] - p[a.x]).hypot(p[b.y] - p[a.y]) > 1e-4
                        }
                    };
                    if !valid {
                        return Err("Constraints would collapse a shape".into());
                    }
                }
                SolverEntity::Spline(spline) => {
                    let points: Vec<Point> = spline.points.iter().map(|&v| read(v)).collect();
                    let mode = if spline.control {
                        SplineMode::Control
                    } else {
                        SplineMode::Fit
                    };
                    spline::beziers(mode, &points, spline.closed)?;
                }
                // Direction residuals are trivially met by a collapsed line;
                // treat that as a conflict rather than a solution.
                SolverEntity::Line(line)
                    if (p[line.b.x] - p[line.a.x]).hypot(p[line.b.y] - p[line.a.y]) <= 1e-3 =>
                {
                    return Err("Constraints would collapse a line".into());
                }
                _ => {}
            }
        }

        for shape in &mut self.shapes {
            let Some(found) = entities.get(&shape.id()) else {
                continue;
            };
            match (shape, found) {
                (Shape::Line { a, b, .. }, SolverEntity::Line(line)) => {
                    *a = read(line.a);
                    *b = read(line.b);
                }
                (Shape::Point { at, .. }, SolverEntity::Point(point)) => *at = read(*point),
                (Shape::Circle { center, radius, .. }, SolverEntity::Circle(circle)) => {
                    *center = read(circle.center);
                    *radius = p[circle.radius] as f32;
                }
                (
                    Shape::Arc {
                        center,
                        radius,
                        start_angle,
                        sweep,
                        ..
                    },
                    SolverEntity::Arc(arc),
                ) => {
                    *center = read(arc.circle.center);
                    *radius = p[arc.circle.radius] as f32;
                    *start_angle = p[arc.start];
                    *sweep = p[arc.sweep];
                }
                (
                    Shape::Ellipse {
                        center,
                        rx,
                        ry,
                        rotation,
                        ..
                    },
                    SolverEntity::Ellipse(e),
                ) => {
                    *center = read(e.center);
                    *rx = p[e.rx] as f32;
                    *ry = p[e.ry] as f32;
                    *rotation = p[e.rotation] as f32;
                }
                (
                    Shape::EllipticArc {
                        center,
                        rx,
                        ry,
                        rotation,
                        start_angle,
                        sweep,
                        ..
                    },
                    SolverEntity::Ellipse(e),
                ) => {
                    *center = read(e.center);
                    *rx = p[e.rx] as f32;
                    *ry = p[e.ry] as f32;
                    *rotation = p[e.rotation] as f32;
                    if let Some((start, arc_sweep)) = e.arc {
                        *start_angle = p[start];
                        *sweep = p[arc_sweep];
                    }
                }
                (Shape::Spline { points, .. }, SolverEntity::Spline(spline)) => {
                    *points = spline.points.iter().map(|&v| read(v)).collect();
                }
                (shape, SolverEntity::Shape(var)) => {
                    use solver::ShapeVar as S;
                    match (shape, *var) {
                        (Shape::Rectangle { a, b, .. }, S::Rectangle { a: va, b: vb }) => {
                            *a = read(va);
                            *b = read(vb);
                        }
                        (
                            Shape::RotatedRectangle {
                                center,
                                width,
                                height,
                                rotation,
                                ..
                            },
                            S::RotatedRectangle {
                                center: vc,
                                half_width,
                                half_height,
                                rotation: vr,
                            },
                        ) => {
                            *center = read(vc);
                            *width = (p[half_width] * 2.0) as f32;
                            *height = (p[half_height] * 2.0) as f32;
                            *rotation = p[vr] as f32;
                        }
                        (
                            Shape::Polygon {
                                center,
                                radius,
                                rotation,
                                ..
                            },
                            S::Polygon {
                                center: vc,
                                radius: vradius,
                                rotation: vr,
                                ..
                            },
                        ) => {
                            *center = read(vc);
                            *radius = p[vradius] as f32;
                            *rotation = p[vr] as f32;
                        }
                        (
                            Shape::Capsule {
                                from, to, width, ..
                            },
                            S::Slot {
                                a: va,
                                b: vb,
                                width: vw,
                            },
                        ) => {
                            *from = read(va);
                            *to = read(vb);
                            *width = p[vw] as f32;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        self.propagate_patterns();
        Ok(())
    }

    /// Run the solver after an edit that already pushed its undo snapshot;
    /// on failure the whole edit is rolled back.
    pub(super) fn solve_or_rollback(&mut self, pins: &[Pin]) -> Result<(), String> {
        if let Err(error) = self.solve_constraints(pins) {
            if let Some(state) = self.history.pop() {
                let selected = (self.selected, self.selected_spline_point);
                let selection = self.selected_ids();
                self.restore(state);
                self.set_selection(&selection);
                (self.selected, self.selected_spline_point) = selected;
            }
            return Err(error);
        }
        Ok(())
    }
}
