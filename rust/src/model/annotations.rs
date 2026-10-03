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
}

/// Per-dimension annotation state kept with the document (and its undo
/// history).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DimensionMeta {
    /// Absolute label positions in sketch coordinates (web `labelPosition`).
    pub label_positions: BTreeMap<DimensionKey, Point>,
}

impl DimensionMeta {
    /// Whether the dimension carries state a graph `sketchConstraint` node
    /// cannot store.
    pub fn has_annotation(&self, key: DimensionKey) -> bool {
        self.label_positions.contains_key(&key)
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
            .chain(
                self.line_relations
                    .iter()
                    .filter(|(_, (_, kind))| matches!(kind, LineRelationKind::Angle(_)))
                    .map(|(&target, _)| DimensionKey::Angle(target)),
            )
            .filter(|key| self.dimension_shape(*key).is_some())
            .collect();
        keys.sort();
        keys
    }

    fn dimension_shape(&self, key: DimensionKey) -> Option<&Shape> {
        let id = match key {
            DimensionKey::Field(id, _) | DimensionKey::Angle(id) => id,
        };
        self.shapes.iter().find(|shape| shape.id() == id)
    }

    /// Stored (driving) value of a dimension.
    pub fn dimension_value(&self, key: DimensionKey) -> Option<f32> {
        match key {
            DimensionKey::Field(id, field) => self.driving_dimensions.get(&(id, field)).copied(),
            DimensionKey::Angle(id) => self.line_relations.get(&id)?.1.angle_value(),
        }
    }

    /// Whether a dimension measures an angle (degrees) rather than a length.
    pub fn dimension_is_angle(key: DimensionKey) -> bool {
        matches!(
            key,
            DimensionKey::Angle(_)
                | DimensionKey::Field(_, DimensionField::ArcSweep | DimensionField::Rotation)
        )
    }

    pub fn dimension_text(&self, key: DimensionKey) -> Option<String> {
        let value = format_value(self.dimension_value(key)?);
        Some(match key {
            DimensionKey::Field(_, DimensionField::CircleDiameter) => format!("D{value}"),
            DimensionKey::Field(_, DimensionField::CurveRadius) => format!("R{value}"),
            key if Self::dimension_is_angle(key) => format!("{value}deg"),
            _ => value,
        })
    }

    /// Default label placement and dimension lines. Offsets are a fixed
    /// number of screen pixels so labels stay readable at any zoom.
    fn default_label(&self, key: DimensionKey) -> Option<LabelPlacement> {
        let offset = 14.0 / self.zoom.max(0.2);
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
        };
        self.selected = selected;
        self.selected_spline_point = spline_point;
        result
    }

    /// Drops annotation state of dimensions that no longer exist.
    pub(super) fn prune_dimension_meta(&mut self) {
        let keys: HashSet<DimensionKey> = self.dimension_keys().into_iter().collect();
        self.dims
            .label_positions
            .retain(|key, _| keys.contains(key));
    }
}
