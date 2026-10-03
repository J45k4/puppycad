//! Dimensions as sketch objects: canvas labels, label placement and
//! hit-testing, and editing a dimension by its key.

use super::*;

/// Text scale of dimension labels in the viewport (8 px glyphs).
pub const LABEL_SCALE: f32 = 1.25;

/// One dimension in the sketch, independent of the selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DimensionKey {
    /// A per-shape dimension (`length`, `width`, `diameter`, ...).
    Field(u32, DimensionField),
    /// The signed angle relation whose target line is this shape.
    Angle(u32),
    /// A point dimension (`DimensionMeta::point_dimensions`).
    Point(u32),
}

/// Axis of a point X / Y dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointAxis {
    X,
    Y,
}

impl PointAxis {
    pub fn name(self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
        }
    }
}

/// Web `distance` relation between two anchors: aligned distance, or the
/// signed `b - a` coordinate difference along `axis`. Absolute X / Y
/// dimensions measure from a fixed sketch point at the origin.
#[derive(Clone, Debug, PartialEq)]
pub struct PointDimension {
    /// Shape id and anchor index (see [`Document::dimension_anchor`]).
    pub a: (u32, usize),
    pub b: (u32, usize),
    pub axis: Option<PointAxis>,
    pub value: f32,
    /// Relation id in the project file, kept across saves.
    pub graph_id: Option<String>,
}

/// Per-dimension annotation state kept with the document (and its undo
/// history).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DimensionMeta {
    /// Absolute label positions in sketch coordinates (web `labelPosition`).
    pub label_positions: BTreeMap<DimensionKey, Point>,
    pub point_dimensions: BTreeMap<u32, PointDimension>,
    /// Reference dimensions: measured, shown in parentheses, never solved
    /// (web `reference: true`). Reference `Field` keys are not in
    /// `driving_dimensions`.
    pub references: BTreeSet<DimensionKey>,
}

impl DimensionMeta {
    /// Whether the dimension carries state a graph `sketchConstraint` node
    /// cannot store.
    pub fn has_annotation(&self, key: DimensionKey) -> bool {
        self.label_positions.contains_key(&key) || self.references.contains(&key)
    }
}

/// A dimension label ready to paint, in sketch coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionLabel {
    pub key: DimensionKey,
    /// Center of the label text.
    pub at: Point,
    /// Point of the geometry the label belongs to; a leader is drawn from
    /// here when the label sits away from its default place.
    pub attach: Point,
    /// Dimension and extension lines.
    pub lines: Vec<(Point, Point)>,
    pub text: String,
}

/// Label center, attach point and dimension lines.
type LabelPlacement = (Point, Point, Vec<(Point, Point)>);

/// Formats a dimension value the way the labels show it.
pub fn format_value(value: f32) -> String {
    let text = format!("{value:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" {
        "0".into()
    } else {
        text.into()
    }
}

/// Label box size in screen pixels.
pub fn label_size(text: &str) -> (f32, f32) {
    (
        text.chars().count() as f32 * 8.0 * LABEL_SCALE + 8.0,
        8.0 * LABEL_SCALE + 8.0,
    )
}

fn add(a: Point, b: Point) -> Point {
    Point {
        x: a.x + b.x,
        y: a.y + b.y,
    }
}

fn scaled(a: Point, s: f32) -> Point {
    Point {
        x: a.x * s,
        y: a.y * s,
    }
}

fn direction(degrees: f32) -> Point {
    let (s, c) = degrees.to_radians().sin_cos();
    Point { x: c, y: s }
}

/// Dimension line parallel to `p`-`q`, pushed `offset` away from `away`.
fn edge_dimension(p: Point, q: Point, away: Point, offset: f32) -> (Point, Vec<(Point, Point)>) {
    let length = p.distance(q).max(1e-6);
    let mut normal = Point {
        x: -(q.y - p.y) / length,
        y: (q.x - p.x) / length,
    };
    let mid = scaled(add(p, q), 0.5);
    if (mid.x - away.x) * normal.x + (mid.y - away.y) * normal.y < 0.0 {
        normal = scaled(normal, -1.0);
    }
    let shift = scaled(normal, offset);
    let (pp, qq) = (add(p, shift), add(q, shift));
    (
        add(mid, scaled(normal, offset * 1.9)),
        vec![(p, pp), (q, qq), (pp, qq)],
    )
}

impl Document {
    /// Every dimension shown in the viewport, in a stable order.
    pub fn dimension_keys(&self) -> Vec<DimensionKey> {
        let mut keys: Vec<DimensionKey> = self
            .driving_dimensions
            .keys()
            .map(|&(id, field)| DimensionKey::Field(id, field))
            .chain(self.dims.references.iter().copied().filter(|key| {
                matches!(key, DimensionKey::Field(..)) && self.measure_dimension(*key).is_some()
            }))
            .chain(
                self.line_relations
                    .iter()
                    .filter(|(_, (_, kind))| matches!(kind, LineRelationKind::Angle(_)))
                    .map(|(&target, _)| DimensionKey::Angle(target)),
            )
            .filter(|key| self.dimension_shape(*key).is_some())
            .chain(
                self.dims
                    .point_dimensions
                    .iter()
                    .filter(|(_, dimension)| {
                        self.dimension_anchor(dimension.a).is_some()
                            && self.dimension_anchor(dimension.b).is_some()
                    })
                    .map(|(&id, _)| DimensionKey::Point(id)),
            )
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }

    fn dimension_shape(&self, key: DimensionKey) -> Option<&Shape> {
        let id = match key {
            DimensionKey::Field(id, _) | DimensionKey::Angle(id) => id,
            DimensionKey::Point(id) => self.dims.point_dimensions.get(&id)?.a.0,
        };
        self.shapes.iter().find(|shape| shape.id() == id)
    }

    /// Stored (driving) value of a dimension.
    pub fn dimension_value(&self, key: DimensionKey) -> Option<f32> {
        match key {
            DimensionKey::Field(id, field) => self.driving_dimensions.get(&(id, field)).copied(),
            DimensionKey::Angle(id) => self.line_relations.get(&id)?.1.angle_value(),
            DimensionKey::Point(id) => Some(self.dims.point_dimensions.get(&id)?.value),
        }
    }

    /// Position of a dimension anchor. Only anchors the solver models as
    /// points can be dimensioned: sketch points, line endpoints, curve
    /// centers and spline handles.
    pub fn dimension_anchor(&self, (id, index): (u32, usize)) -> Option<Point> {
        let shape = self.shapes.iter().find(|shape| shape.id() == id)?;
        match shape {
            Shape::Point { at, .. } if index == 0 => Some(*at),
            Shape::Line { a, b, .. } => match index {
                0 => Some(*a),
                1 => Some(*b),
                _ => None,
            },
            Shape::Circle { center, .. }
            | Shape::Arc { center, .. }
            | Shape::Ellipse { center, .. }
            | Shape::EllipticArc { center, .. }
                if index == 0 =>
            {
                Some(*center)
            }
            Shape::Spline { points, .. } => points.get(index).copied(),
            _ => None,
        }
    }

    /// Dimensionable anchor nearest to `point` within `tolerance`.
    pub fn dimension_anchor_near(&self, point: Point, tolerance: f32) -> Option<(u32, usize)> {
        let mut best = None;
        let mut distance = tolerance;
        for shape in &self.shapes {
            let count = match shape {
                Shape::Line { .. } => 2,
                Shape::Spline { points, .. } => points.len(),
                _ => 1,
            };
            for index in 0..count {
                let anchor = (shape.id(), index);
                if let Some(at) = self.dimension_anchor(anchor)
                    && at.distance(point) <= distance
                {
                    distance = at.distance(point);
                    best = Some(anchor);
                }
            }
        }
        best
    }

    /// Current measured value of a point dimension.
    pub fn measure_point_dimension(&self, dimension: &PointDimension) -> Option<f32> {
        let a = self.dimension_anchor(dimension.a)?;
        let b = self.dimension_anchor(dimension.b)?;
        Some(match dimension.axis {
            None => a.distance(b),
            Some(PointAxis::X) => b.x - a.x,
            Some(PointAxis::Y) => b.y - a.y,
        })
    }

    /// The fixed construction point at the sketch origin that absolute X / Y
    /// dimensions measure from, created when missing. Call inside an edit
    /// whose snapshot is already on the undo stack.
    fn origin_anchor(&mut self) -> (u32, usize) {
        let origin = Point { x: 0.0, y: 0.0 };
        if let Some(id) = self.shapes.iter().find_map(|shape| match shape {
            Shape::Point { id, at }
                if *at == origin && self.fixed_anchors.get(&(*id, 0)) == Some(&origin) =>
            {
                Some(*id)
            }
            _ => None,
        }) {
            return (id, 0);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.shapes.push(Shape::Point { id, at: origin });
        self.construction.insert(id);
        self.fixed_anchors.insert((id, 0), origin);
        (id, 0)
    }

    /// Adds a point dimension at the measured value; `b == None` measures
    /// from the sketch origin. One undo step.
    pub fn add_point_dimension(
        &mut self,
        a: (u32, usize),
        b: Option<(u32, usize)>,
        axis: Option<PointAxis>,
    ) -> Result<DimensionKey, String> {
        self.dimension_anchor(a)
            .ok_or("Pick a point, line end or curve center")?;
        if let Some(b) = b {
            self.dimension_anchor(b)
                .ok_or("Pick a point, line end or curve center")?;
            if a == b {
                return Err("Pick two different points".into());
            }
        }
        let before = self.snapshot();
        let (a, b) = match b {
            Some(b) => (a, b),
            None => {
                let origin = self.origin_anchor();
                (origin, a)
            }
        };
        if self.dims.point_dimensions.values().any(|existing| {
            existing.axis == axis
                && ((existing.a, existing.b) == (a, b) || (existing.a, existing.b) == (b, a))
        }) {
            self.restore_keeping_selection(before);
            return Err("These points already have that dimension".into());
        }
        let mut dimension = PointDimension {
            a,
            b,
            axis,
            value: 0.0,
            graph_id: None,
        };
        let value = self
            .measure_point_dimension(&dimension)
            .ok_or("Dimension anchor is missing")?;
        if axis.is_none() && value <= 1e-6 {
            self.restore_keeping_selection(before);
            return Err("Points coincide; a distance needs two separate points".into());
        }
        dimension.value = value;
        let id = self
            .dims
            .point_dimensions
            .keys()
            .next_back()
            .map_or(1, |last| last + 1);
        self.dims.point_dimensions.insert(id, dimension);
        self.history.push(before);
        self.future.clear();
        Ok(DimensionKey::Point(id))
    }

    fn restore_keeping_selection(&mut self, state: DocumentState) {
        let selected = (self.selected, self.selected_spline_point);
        self.restore(state);
        (self.selected, self.selected_spline_point) = selected;
    }

    fn set_point_dimension(&mut self, id: u32, value: f32) -> Result<(), String> {
        let dimension = self
            .dims
            .point_dimensions
            .get(&id)
            .ok_or("Dimension no longer exists")?;
        if !value.is_finite() {
            return Err("dimension must be finite".into());
        }
        if dimension.axis.is_none() && value <= 0.0 {
            return Err("dimension must be positive".into());
        }
        if dimension.value == value {
            return Ok(());
        }
        let pin = Pin::Anchor(dimension.a.0, dimension.a.1);
        self.history.push(self.snapshot());
        self.future.clear();
        if let Some(dimension) = self.dims.point_dimensions.get_mut(&id) {
            dimension.value = value;
        }
        self.solve_or_rollback(&[pin])
    }

    /// Whether a dimension measures an angle (degrees) rather than a length.
    pub fn dimension_is_angle(key: DimensionKey) -> bool {
        matches!(
            key,
            DimensionKey::Angle(_)
                | DimensionKey::Field(_, DimensionField::ArcSweep | DimensionField::Rotation)
        )
    }

    pub fn is_reference(&self, key: DimensionKey) -> bool {
        self.dims.references.contains(&key)
    }

    /// Value shown for a dimension: the driving value, or the measurement
    /// of a reference dimension.
    pub fn displayed_dimension_value(&self, key: DimensionKey) -> Option<f32> {
        if self.is_reference(key) {
            self.measure_dimension(key)
        } else {
            self.dimension_value(key)
        }
    }

    /// Current geometric value of a dimension (web
    /// `measureSketchDimension`).
    pub fn measure_dimension(&self, key: DimensionKey) -> Option<f32> {
        let shape = |id: u32| self.shapes.iter().find(|shape| shape.id() == id);
        Some(match key {
            DimensionKey::Point(id) => {
                self.measure_point_dimension(self.dims.point_dimensions.get(&id)?)?
            }
            DimensionKey::Angle(target) => {
                let (source, _) = self.line_relations.get(&target)?;
                let angle = |id: u32| match shape(id)? {
                    Shape::Line { a, b, .. } => Some((b.y - a.y).atan2(b.x - a.x)),
                    _ => None,
                };
                let delta = angle(target)? - angle(*source)?;
                delta.sin().atan2(delta.cos()).to_degrees()
            }
            DimensionKey::Field(id, field) => match (shape(id)?, field) {
                (Shape::Line { a, b, .. }, DimensionField::LineLength) => a.distance(*b),
                (Shape::Line { a, b, .. }, DimensionField::Rotation) => {
                    (b.y - a.y).atan2(b.x - a.x).to_degrees()
                }
                (Shape::Rectangle { a, b, .. }, DimensionField::RectangleWidth) => {
                    (b.x - a.x).abs()
                }
                (Shape::Rectangle { a, b, .. }, DimensionField::RectangleHeight) => {
                    (b.y - a.y).abs()
                }
                (Shape::RotatedRectangle { width, .. }, DimensionField::RectangleWidth) => *width,
                (Shape::RotatedRectangle { height, .. }, DimensionField::RectangleHeight) => {
                    *height
                }
                (
                    Shape::Ellipse { rx, .. } | Shape::EllipticArc { rx, .. },
                    DimensionField::RectangleWidth,
                ) => rx * 2.0,
                (
                    Shape::Ellipse { ry, .. } | Shape::EllipticArc { ry, .. },
                    DimensionField::RectangleHeight,
                ) => ry * 2.0,
                (Shape::Circle { radius, .. }, DimensionField::CircleDiameter) => radius * 2.0,
                (
                    Shape::Circle { radius, .. }
                    | Shape::Arc { radius, .. }
                    | Shape::Polygon { radius, .. },
                    DimensionField::CurveRadius,
                ) => *radius,
                (
                    Shape::Arc { sweep, .. } | Shape::EllipticArc { sweep, .. },
                    DimensionField::ArcSweep,
                ) => sweep.to_degrees() as f32,
                (
                    Shape::RotatedRectangle { rotation, .. }
                    | Shape::Ellipse { rotation, .. }
                    | Shape::EllipticArc { rotation, .. }
                    | Shape::Polygon { rotation, .. },
                    DimensionField::Rotation,
                ) => *rotation,
                _ => return None,
            },
        })
    }

    /// Switches a dimension between driving and reference. A reference
    /// keeps measuring; a driving dimension starts from the measured value.
    /// One undo step; returns whether the dimension is now a reference.
    pub fn toggle_reference(&mut self, key: DimensionKey) -> Result<bool, String> {
        if !self.dimension_keys().contains(&key) {
            return Err("Dimension no longer exists".into());
        }
        if matches!(key, DimensionKey::Angle(_)) {
            return Err("Angle relations cannot be reference dimensions".into());
        }
        let measured = self
            .measure_dimension(key)
            .ok_or("Dimension cannot be measured")?;
        let before = self.snapshot();
        let depth = self.history.len();
        let reference = !self.is_reference(key);
        let result = if reference {
            if let DimensionKey::Field(id, field) = key {
                self.driving_dimensions.remove(&(id, field));
            }
            self.dims.references.insert(key);
            Ok(())
        } else {
            self.dims.references.remove(&key);
            match key {
                DimensionKey::Field(..) => {
                    if measured <= 0.0
                        && !matches!(
                            key,
                            DimensionKey::Field(
                                _,
                                DimensionField::Rotation | DimensionField::ArcSweep
                            )
                        )
                    {
                        Err("A zero-size dimension cannot drive geometry".to_string())
                    } else {
                        self.set_dimension_value(key, measured)
                    }
                }
                DimensionKey::Point(id) => {
                    if let Some(dimension) = self.dims.point_dimensions.get_mut(&id) {
                        dimension.value = measured;
                    }
                    self.solve_constraints(&[])
                }
                DimensionKey::Angle(_) => unreachable!("rejected above"),
            }
        };
        self.history.truncate(depth);
        match result {
            Ok(()) => {
                self.history.push(before);
                self.future.clear();
                Ok(reference)
            }
            Err(error) => {
                self.restore_keeping_selection(before);
                Err(error)
            }
        }
    }

    pub fn dimension_text(&self, key: DimensionKey) -> Option<String> {
        let value = format_value(self.displayed_dimension_value(key)?);
        let text = match key {
            DimensionKey::Field(_, DimensionField::CircleDiameter) => format!("D{value}"),
            DimensionKey::Field(_, DimensionField::CurveRadius) => format!("R{value}"),
            DimensionKey::Point(id) => match self.dims.point_dimensions.get(&id)?.axis {
                Some(PointAxis::X) => format!("X{value}"),
                Some(PointAxis::Y) => format!("Y{value}"),
                None => value,
            },
            key if Self::dimension_is_angle(key) => format!("{value}deg"),
            _ => value,
        };
        Some(if self.is_reference(key) {
            format!("({text})")
        } else {
            text
        })
    }

    /// Default label placement and dimension lines. Offsets are a fixed
    /// number of screen pixels so labels stay readable at any zoom.
    fn default_label(&self, key: DimensionKey) -> Option<LabelPlacement> {
        let offset = 14.0 / self.zoom.max(0.2);
        if let DimensionKey::Point(id) = key {
            let dimension = self.dims.point_dimensions.get(&id)?;
            let a = self.dimension_anchor(dimension.a)?;
            let b = self.dimension_anchor(dimension.b)?;
            return Some(match dimension.axis {
                None => {
                    let (at, lines) = edge_dimension(
                        a,
                        b,
                        Point {
                            x: (a.x + b.x) * 0.5 + (a.y - b.y),
                            y: (a.y + b.y) * 0.5 + (b.x - a.x),
                        },
                        offset,
                    );
                    (at, scaled(add(a, b), 0.5), lines)
                }
                Some(PointAxis::X) => {
                    // Horizontal dimension line at the higher point.
                    let y = a.y.max(b.y) + offset * 1.5;
                    let (pa, pb) = (Point { x: a.x, y }, Point { x: b.x, y });
                    (
                        Point {
                            x: (a.x + b.x) * 0.5,
                            y: y + offset,
                        },
                        scaled(add(pa, pb), 0.5),
                        vec![(a, pa), (b, pb), (pa, pb)],
                    )
                }
                Some(PointAxis::Y) => {
                    let x = a.x.max(b.x) + offset * 1.5;
                    let (pa, pb) = (Point { x, y: a.y }, Point { x, y: b.y });
                    (
                        Point {
                            x: x + offset * 2.5,
                            y: (a.y + b.y) * 0.5,
                        },
                        scaled(add(pa, pb), 0.5),
                        vec![(a, pa), (b, pb), (pa, pb)],
                    )
                }
            });
        }
        let shape = self.dimension_shape(key)?;
        let center = shape.center();
        let ray = |dir: Point, from: f32, to: f32, label: f32| {
            let start = add(center, scaled(dir, from));
            let end = add(center, scaled(dir, to));
            (add(center, scaled(dir, label)), end, vec![(start, end)])
        };
        Some(match (key, shape) {
            (DimensionKey::Angle(target), _) => {
                let (source, _) = self.line_relations.get(&target)?;
                let line = |id: u32| match self.shapes.iter().find(|shape| shape.id() == id) {
                    Some(Shape::Line { a, b, .. }) => Some((*a, *b)),
                    _ => None,
                };
                let (sa, sb) = line(*source)?;
                let (ta, tb) = line(target)?;
                let d1 = Point {
                    x: sb.x - sa.x,
                    y: sb.y - sa.y,
                };
                let d2 = Point {
                    x: tb.x - ta.x,
                    y: tb.y - ta.y,
                };
                let denominator = d1.x * d2.y - d1.y * d2.x;
                let vertex = if denominator.abs() > 1e-9 {
                    let t = ((ta.x - sa.x) * d2.y - (ta.y - sa.y) * d2.x) / denominator;
                    add(sa, scaled(d1, t))
                } else {
                    ta
                };
                let unit = |d: Point| scaled(d, 1.0 / d.x.hypot(d.y).max(1e-6));
                let bisector = add(unit(d1), unit(d2));
                let length = bisector.x.hypot(bisector.y);
                let dir = if length > 1e-4 {
                    scaled(bisector, 1.0 / length)
                } else {
                    Point {
                        x: -unit(d1).y,
                        y: unit(d1).x,
                    }
                };
                let arc_start = add(vertex, scaled(unit(d1), offset * 1.5));
                let arc_end = add(vertex, scaled(unit(d2), offset * 1.5));
                (
                    add(vertex, scaled(dir, offset * 3.0)),
                    vertex,
                    vec![(vertex, arc_start), (vertex, arc_end)],
                )
            }
            (DimensionKey::Field(_, DimensionField::LineLength), Shape::Line { a, b, .. }) => {
                let (at, lines) = edge_dimension(
                    *a,
                    *b,
                    Point {
                        x: center.x + (a.y - b.y),
                        y: center.y + (b.x - a.x),
                    },
                    offset,
                );
                (at, center, lines)
            }
            (
                DimensionKey::Field(
                    _,
                    field @ (DimensionField::RectangleWidth | DimensionField::RectangleHeight),
                ),
                Shape::Rectangle { a, b, .. },
            ) => {
                let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
                let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
                let (p, q) = if field == DimensionField::RectangleWidth {
                    (Point { x: x0, y: y0 }, Point { x: x1, y: y0 })
                } else {
                    (Point { x: x1, y: y0 }, Point { x: x1, y: y1 })
                };
                let (at, lines) = edge_dimension(p, q, center, offset);
                (at, scaled(add(p, q), 0.5), lines)
            }
            (
                DimensionKey::Field(
                    _,
                    field @ (DimensionField::RectangleWidth | DimensionField::RectangleHeight),
                ),
                Shape::RotatedRectangle {
                    center,
                    width,
                    height,
                    rotation,
                    ..
                },
            ) => {
                let corners = Shape::rotated_rectangle_corners(*center, *width, *height, *rotation);
                let (p, q) = if field == DimensionField::RectangleWidth {
                    (corners[0], corners[1])
                } else {
                    (corners[1], corners[2])
                };
                let (at, lines) = edge_dimension(p, q, *center, offset);
                (at, scaled(add(p, q), 0.5), lines)
            }
            (
                DimensionKey::Field(
                    _,
                    field @ (DimensionField::RectangleWidth | DimensionField::RectangleHeight),
                ),
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
            ) => {
                let (along, across, extent, side) = if field == DimensionField::RectangleWidth {
                    (direction(*rotation), direction(*rotation + 90.0), *rx, *ry)
                } else {
                    (direction(*rotation + 90.0), direction(*rotation), *ry, *rx)
                };
                let base = add(*center, scaled(across, -side));
                let p = add(base, scaled(along, -extent));
                let q = add(base, scaled(along, extent));
                let (at, lines) = edge_dimension(p, q, *center, offset);
                (at, base, lines)
            }
            (
                DimensionKey::Field(_, DimensionField::CircleDiameter),
                Shape::Circle { center, radius, .. },
            ) => {
                let dir = direction(45.0);
                let (at, end, _) = ray(dir, 0.0, *radius, *radius + offset * 2.0);
                let start = add(*center, scaled(dir, -*radius));
                (at, end, vec![(start, end)])
            }
            (DimensionKey::Field(_, DimensionField::CurveRadius), shape) => {
                let (radius, dir) = match shape {
                    Shape::Circle { radius, .. } => (*radius, direction(45.0)),
                    Shape::Arc {
                        radius,
                        start_angle,
                        sweep,
                        ..
                    } => (
                        *radius,
                        direction((start_angle + sweep * 0.5).to_degrees() as f32),
                    ),
                    Shape::Polygon {
                        radius, rotation, ..
                    } => (*radius, direction(*rotation)),
                    _ => return None,
                };
                ray(dir, 0.0, radius, radius + offset * 2.0)
            }
            (DimensionKey::Field(_, DimensionField::ArcSweep), shape) => {
                let point = match shape {
                    Shape::Arc {
                        center,
                        radius,
                        start_angle,
                        sweep,
                        ..
                    } => Shape::arc_point(*center, radius * 0.55, *start_angle, *sweep, 0.5),
                    Shape::EllipticArc {
                        center,
                        rx,
                        ry,
                        rotation,
                        start_angle,
                        sweep,
                        ..
                    } => Shape::elliptic_arc_point(
                        *center,
                        rx * 0.55,
                        ry * 0.55,
                        *rotation,
                        *start_angle,
                        *sweep,
                        0.5,
                    ),
                    _ => return None,
                };
                (point, center, Vec::new())
            }
            (DimensionKey::Field(_, DimensionField::Rotation), shape) => {
                let pivot = match shape {
                    Shape::Line { a, .. } => *a,
                    _ => center,
                };
                (
                    add(
                        pivot,
                        Point {
                            x: offset * 2.5,
                            y: -offset * 1.5,
                        },
                    ),
                    pivot,
                    Vec::new(),
                )
            }
            _ => return None,
        })
    }

    /// Labels for every dimension, at their stored or default position.
    pub fn dimension_labels(&self) -> Vec<DimensionLabel> {
        self.dimension_keys()
            .into_iter()
            .filter_map(|key| {
                let (at, attach, lines) = self.default_label(key)?;
                Some(DimensionLabel {
                    key,
                    at: self.dims.label_positions.get(&key).copied().unwrap_or(at),
                    attach,
                    lines,
                    text: self.dimension_text(key)?,
                })
            })
            .collect()
    }

    /// Label under a screen point (topmost wins).
    pub fn dimension_label_at(
        &self,
        screen: Point,
        viewport_origin: Point,
    ) -> Option<DimensionKey> {
        self.dimension_labels()
            .into_iter()
            .rev()
            .find(|label| {
                let center = self.world_to_screen(label.at, viewport_origin);
                let (w, h) = label_size(&label.text);
                (screen.x - center.x).abs() <= w * 0.5 && (screen.y - center.y).abs() <= h * 0.5
            })
            .map(|label| label.key)
    }

    /// Moves a label (`None` restores its default place). One undo step.
    pub fn set_dimension_label_position(
        &mut self,
        key: DimensionKey,
        at: Option<Point>,
    ) -> Result<(), String> {
        if !self.dimension_keys().contains(&key) {
            return Err("Dimension no longer exists".into());
        }
        if at.is_some_and(|at| !at.x.is_finite() || !at.y.is_finite()) {
            return Err("Label position must be finite".into());
        }
        if self.dims.label_positions.get(&key).copied() == at {
            return Ok(());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        match at {
            Some(at) => self.dims.label_positions.insert(key, at),
            None => self.dims.label_positions.remove(&key),
        };
        Ok(())
    }

    /// Sets a dimension's driving value; the geometry follows. The selection
    /// is kept.
    pub fn set_dimension_value(&mut self, key: DimensionKey, value: f32) -> Result<(), String> {
        if self.is_reference(key) {
            return Err("Reference dimensions are measured; make it driving first".into());
        }
        let selected = self.selected;
        let spline_point = self.selected_spline_point;
        let result = match key {
            DimensionKey::Field(id, field) => {
                self.selected = Some(id);
                self.set_dimension(field, value)
            }
            DimensionKey::Angle(id) => {
                self.selected = Some(id);
                self.set_line_relation_angle(value)
            }
            DimensionKey::Point(id) => self.set_point_dimension(id, value),
        };
        self.selected = selected;
        self.selected_spline_point = spline_point;
        result
    }

    /// Drops annotation state of dimensions that no longer exist.
    pub(super) fn prune_dimension_meta(&mut self) {
        let points: Vec<u32> = self
            .dims
            .point_dimensions
            .iter()
            .filter(|(_, dimension)| {
                self.dimension_anchor(dimension.a).is_none()
                    || self.dimension_anchor(dimension.b).is_none()
            })
            .map(|(&id, _)| id)
            .collect();
        for id in points {
            self.dims.point_dimensions.remove(&id);
        }
        let keys: HashSet<DimensionKey> = self.dimension_keys().into_iter().collect();
        self.dims
            .label_positions
            .retain(|key, _| keys.contains(key));
        self.dims.references.retain(|key| keys.contains(key));
    }

    /// Deletes a dimension and its annotations. One undo step.
    pub fn delete_dimension(&mut self, key: DimensionKey) -> Result<(), String> {
        if !self.dimension_keys().contains(&key) {
            return Err("Dimension no longer exists".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        match key {
            DimensionKey::Field(id, field) => {
                self.driving_dimensions.remove(&(id, field));
            }
            DimensionKey::Angle(id) => {
                self.line_relations.remove(&id);
            }
            DimensionKey::Point(id) => {
                self.dims.point_dimensions.remove(&id);
            }
        }
        self.dims.references.remove(&key);
        self.prune_dimension_meta();
        Ok(())
    }

    /// Adds the next missing driving dimension of a shape at its current
    /// size (web "Add driving dimension"): length, diameter, radius, or
    /// width then height.
    pub fn add_entity_dimension(&mut self, id: u32) -> Result<DimensionKey, String> {
        let shape = self
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .ok_or("Shape no longer exists")?;
        let candidates: Vec<(DimensionField, f32)> = match shape {
            Shape::Line { a, b, .. } => vec![(DimensionField::LineLength, a.distance(*b))],
            Shape::Circle { radius, .. } => vec![(DimensionField::CircleDiameter, radius * 2.0)],
            Shape::Arc { radius, .. } | Shape::Polygon { radius, .. } => {
                vec![(DimensionField::CurveRadius, *radius)]
            }
            Shape::Rectangle { a, b, .. } => vec![
                (DimensionField::RectangleWidth, (b.x - a.x).abs()),
                (DimensionField::RectangleHeight, (b.y - a.y).abs()),
            ],
            Shape::RotatedRectangle { width, height, .. } => vec![
                (DimensionField::RectangleWidth, *width),
                (DimensionField::RectangleHeight, *height),
            ],
            Shape::Ellipse { rx, ry, .. } | Shape::EllipticArc { rx, ry, .. } => vec![
                (DimensionField::RectangleWidth, rx * 2.0),
                (DimensionField::RectangleHeight, ry * 2.0),
            ],
            _ => return Err("Pick two points to dimension this entity".into()),
        };
        let (field, value) = candidates
            .into_iter()
            .find(|(field, _)| !self.driving_dimensions.contains_key(&(id, *field)))
            .ok_or("Entity is already dimensioned")?;
        let key = DimensionKey::Field(id, field);
        self.set_dimension_value(key, value)?;
        Ok(key)
    }
}
