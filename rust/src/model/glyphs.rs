//! Viewport feedback: relation glyphs drawn beside sketch entities and
//! inference hints while placing a segment (web `sketch-workspace.ts`
//! constraint glyphs and "horizontal"/"parallel"/"tangent" hints).
//!
//! Glyph text uses the 8x8 `BASIC_FONTS` (ASCII only), so parallel is
//! `//`, perpendicular `T` and tangent `TAN`.

use super::*;

/// Pixel size of one glyph character (8x8 font at scale 1).
pub const GLYPH_CHAR: f32 = 8.0;
/// Glyph box padding around the text, in pixels.
pub const GLYPH_PAD: f32 = 2.0;
/// Gap between the entity and its first glyph, and between stacked glyphs.
const GLYPH_GAP: f32 = 4.0;
/// Inference tolerance in degrees.
pub const INFERENCE_DEGREES: f32 = 2.0;

/// One relation as seen from one entity.
#[derive(Clone, Debug, PartialEq)]
pub struct RelationGlyph {
    pub label: &'static str,
    /// Entity the glyph is drawn beside (and selected by a glyph click).
    pub owner: u32,
    /// Every entity of the relation (highlighted when the glyph is hovered).
    pub partners: Vec<u32>,
    /// Fixed anchor: the glyph sits at this anchor instead of the stack.
    pub anchor: Option<usize>,
}

/// A glyph with its screen box (`x`, `y` top-left; `w`, `h`).
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedGlyph {
    pub glyph: RelationGlyph,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl PlacedGlyph {
    pub fn contains(&self, point: Point) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.w
            && point.y >= self.y
            && point.y <= self.y + self.h
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.x < other.x + other.w
            && other.x < self.x + self.w
            && self.y < other.y + other.h
            && other.y < self.y + self.h
    }
}

/// What a segment being placed lines up with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InferenceKind {
    Horizontal,
    Vertical,
    Parallel(u32),
    Perpendicular(u32),
    Tangent(u32),
}

impl InferenceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Horizontal => "H",
            Self::Vertical => "V",
            Self::Parallel(_) => "//",
            Self::Perpendicular(_) => "T",
            Self::Tangent(_) => "TAN",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
            Self::Parallel(_) => "parallel",
            Self::Perpendicular(_) => "perpendicular",
            Self::Tangent(_) => "tangent",
        }
    }
}

/// Inference hint: its kind and a world-space dashed guide segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inference {
    pub kind: InferenceKind,
    pub guide: (Point, Point),
}

fn glyph_size(label: &str) -> (f32, f32) {
    (
        label.chars().count() as f32 * GLYPH_CHAR + GLYPH_PAD * 2.0,
        GLYPH_CHAR + GLYPH_PAD * 2.0,
    )
}

/// Angle difference folded into 0..=90 degrees.
fn fold_degrees(a: f32, b: f32) -> f32 {
    let diff = (a - b).to_degrees().rem_euclid(180.0);
    diff.min(180.0 - diff)
}

impl Document {
    /// Every relation, once per entity it involves, in a stable order.
    pub fn relation_glyphs(&self) -> Vec<RelationGlyph> {
        let mut out = Vec::new();
        let mut both = |label: &'static str, ids: Vec<u32>| {
            let mut unique: Vec<u32> = Vec::new();
            for id in ids {
                if !unique.contains(&id) {
                    unique.push(id);
                }
            }
            for &owner in &unique {
                out.push(RelationGlyph {
                    label,
                    owner,
                    partners: unique.clone(),
                    anchor: None,
                });
            }
        };
        let sorted = |map: &HashMap<u32, u32>| {
            let mut pairs: Vec<(u32, u32)> = map.iter().map(|(&a, &b)| (a, b)).collect();
            pairs.sort();
            pairs
        };
        let mut axes: Vec<_> = self.axis_constraints.iter().collect();
        axes.sort_by_key(|(id, _)| **id);
        for (&id, axis) in axes {
            both(
                match axis {
                    AxisConstraint::Horizontal => "H",
                    AxisConstraint::Vertical => "V",
                },
                vec![id],
            );
        }
        let mut lines: Vec<_> = self.line_relations.iter().collect();
        lines.sort_by_key(|(id, _)| **id);
        for (&target, &(source, kind)) in lines {
            let label = match kind {
                LineRelationKind::Parallel => "//",
                LineRelationKind::Perpendicular => "T",
                LineRelationKind::Collinear => "COL",
                LineRelationKind::Angle(_) => "ANG",
            };
            both(label, vec![target, source]);
        }
        for map in [&self.equal_lines, &self.equal_radii, &self.equal_ellipses] {
            for (a, b) in sorted(map) {
                both("=", vec![a, b]);
            }
        }
        for (a, b) in sorted(&self.concentric) {
            both("O", vec![a, b]);
        }
        for map in [&self.internal_tangents, &self.external_tangents] {
            for (a, b) in sorted(map) {
                both("TAN", vec![a, b]);
            }
        }
        let mut tangents: Vec<_> = self
            .endpoint_tangents
            .iter()
            .map(|(&(arc, _), &(line, _, _))| (arc, line))
            .collect();
        tangents.sort();
        for (a, b) in tangents {
            both("TAN", vec![a, b]);
        }
        for (a, b) in sorted(&self.midpoint_links) {
            both("MID", vec![a, b]);
        }
        for (a, b) in sorted(&self.point_curve_links) {
            both("POC", vec![a, b]);
        }
        let mut normals: Vec<_> = self
            .normal_links
            .iter()
            .map(|(&a, &(b, _))| (a, b))
            .collect();
        normals.sort();
        for (a, b) in normals {
            both("NORM", vec![a, b]);
        }
        let mut joins: Vec<_> = self
            .smooth_joins
            .iter()
            .map(|(&(a, _), &(b, _))| (a, b))
            .collect();
        joins.sort();
        for (a, b) in joins {
            both("TJ", vec![a, b]);
        }
        for (a, b) in sorted(&self.shared_ellipses) {
            both("SE", vec![a, b]);
        }
        let mut differences: Vec<_> = self
            .radius_differences
            .iter()
            .map(|(&a, &(b, _))| (a, b))
            .collect();
        differences.sort();
        for (a, b) in differences {
            both("RD", vec![a, b]);
        }
        for pair in &self.symmetric_points {
            both("SYM", vec![pair.a.0, pair.b.0, pair.axis]);
        }
        let mut mirrors: Vec<_> = self
            .mirrors
            .iter()
            .map(|(&t, &(s, axis))| (t, s, axis))
            .collect();
        mirrors.sort();
        for (target, source, axis) in mirrors {
            both("SYM", vec![target, source, axis]);
        }
        let mut offsets: Vec<_> = self.offsets.iter().map(|(&t, &(s, _))| (t, s)).collect();
        offsets.sort();
        for (target, source) in offsets {
            both("OFS", vec![target, source]);
        }
        for chain in &self.offset_chains {
            let ids = chain
                .sources
                .iter()
                .map(|(id, _)| *id)
                .chain(chain.targets.iter().copied())
                .collect();
            both("OFS", ids);
        }
        for pattern in &self.patterns {
            let ids = pattern
                .sources
                .iter()
                .copied()
                .chain(pattern.instances.iter().flatten().copied())
                .collect();
            both("PAT", ids);
        }
        let mut fixed: Vec<_> = self.fixed_anchors.keys().copied().collect();
        fixed.sort();
        for (id, index) in fixed {
            out.push(RelationGlyph {
                label: "FIX",
                owner: id,
                partners: vec![id],
                anchor: Some(index),
            });
        }
        out.retain(|glyph| self.shapes.iter().any(|shape| shape.id() == glyph.owner));
        out
    }

    /// Screen base point and outward unit normal for an entity's glyph stack.
    fn glyph_base(&self, shape: &Shape, origin: Point) -> (Point, Point) {
        let screen = |p: Point| self.world_to_screen(p, origin);
        let radial = |center: Point, at: Point| {
            let (c, p) = (screen(center), screen(at));
            let length = c.distance(p).max(1e-6);
            (
                p,
                Point {
                    x: (p.x - c.x) / length,
                    y: (p.y - c.y) / length,
                },
            )
        };
        let diagonal = Point {
            x: std::f32::consts::FRAC_1_SQRT_2,
            y: -std::f32::consts::FRAC_1_SQRT_2,
        };
        match *shape {
            Shape::Line { a, b, .. } => {
                let (a, b) = (screen(a), screen(b));
                let length = a.distance(b).max(1e-6);
                let mut normal = Point {
                    x: -(b.y - a.y) / length,
                    y: (b.x - a.x) / length,
                };
                // Prefer above, then right of the line.
                if normal.y > 1e-4 || (normal.y.abs() <= 1e-4 && normal.x < 0.0) {
                    normal = Point {
                        x: -normal.x,
                        y: -normal.y,
                    };
                }
                (
                    Point {
                        x: (a.x + b.x) * 0.5,
                        y: (a.y + b.y) * 0.5,
                    },
                    normal,
                )
            }
            Shape::Circle { center, radius, .. } => {
                let at = Point {
                    x: center.x + radius * std::f32::consts::FRAC_1_SQRT_2,
                    y: center.y + radius * std::f32::consts::FRAC_1_SQRT_2,
                };
                radial(center, at)
            }
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => radial(
                center,
                Shape::arc_point(center, radius, start_angle, sweep, 0.5),
            ),
            Shape::Point { at, .. } => (screen(at), diagonal),
            _ => match shape.bounds() {
                Some((min, max)) => {
                    let (a, b) = (screen(min), screen(max));
                    (
                        Point {
                            x: (a.x + b.x) * 0.5,
                            y: a.y.min(b.y),
                        },
                        Point { x: 0.0, y: -1.0 },
                    )
                }
                None => (screen(Point { x: 0.0, y: 0.0 }), diagonal),
            },
        }
    }

    /// Relation glyphs with screen boxes. Each entity's glyphs stack outward
    /// along its normal (perpendicular to a line, radial for circles and
    /// arcs) so they never overlap one another; FIX sits at its anchor.
    pub fn placed_glyphs(&self, origin: Point) -> Vec<PlacedGlyph> {
        let mut placed = Vec::new();
        let mut stacks: HashMap<u32, (Point, Point, f32)> = HashMap::new();
        for glyph in self.relation_glyphs() {
            let Some(shape) = self.shapes.iter().find(|s| s.id() == glyph.owner) else {
                continue;
            };
            let (w, h) = glyph_size(glyph.label);
            if let Some(index) = glyph.anchor {
                let Some(at) = self
                    .drag_anchors(glyph.owner)
                    .into_iter()
                    .find(|(i, _)| *i == index)
                    .map(|(_, p)| self.world_to_screen(p, origin))
                else {
                    continue;
                };
                placed.push(PlacedGlyph {
                    glyph,
                    x: at.x + 6.0,
                    y: at.y + 6.0,
                    w,
                    h,
                });
                continue;
            }
            let (base, normal, distance) = *stacks.entry(glyph.owner).or_insert_with(|| {
                let (base, normal) = self.glyph_base(shape, origin);
                (base, normal, GLYPH_GAP)
            });
            // Distance along the normal that clears a w x h box.
            let extent = |w: f32, h: f32| {
                let along_x = if normal.x.abs() > 1e-4 {
                    w / normal.x.abs()
                } else {
                    f32::INFINITY
                };
                let along_y = if normal.y.abs() > 1e-4 {
                    h / normal.y.abs()
                } else {
                    f32::INFINITY
                };
                along_x.min(along_y)
            };
            let center_distance = distance + extent(w, h) * 0.5;
            let center = Point {
                x: base.x + normal.x * center_distance,
                y: base.y + normal.y * center_distance,
            };
            placed.push(PlacedGlyph {
                glyph,
                x: center.x - w * 0.5,
                y: center.y - h * 0.5,
                w,
                h,
            });
            stacks.insert(
                shape.id(),
                (
                    base,
                    normal,
                    center_distance + extent(w, h) * 0.5 + GLYPH_GAP * 0.5,
                ),
            );
        }
        placed
    }

    /// Topmost placed glyph under `screen`.
    pub fn glyph_at(&self, screen: Point, origin: Point) -> Option<PlacedGlyph> {
        self.placed_glyphs(origin)
            .into_iter()
            .rev()
            .find(|glyph| glyph.contains(screen))
    }

    /// What a segment from `start` to `end` lines up with: horizontal or
    /// vertical within [`INFERENCE_DEGREES`], tangent to a circle/arc it
    /// starts at or ends on, or parallel/perpendicular to a line within
    /// 30 px of `end`.
    pub fn infer_segment(&self, start: Point, end: Point) -> Option<Inference> {
        let length = start.distance(end);
        if length <= 2.0 / self.zoom {
            return None;
        }
        let angle = (end.y - start.y).atan2(end.x - start.x);
        let near = |a: f32, b: f32| fold_degrees(a, b) <= INFERENCE_DEGREES;
        let perpendicular = |a: f32, b: f32| (90.0 - fold_degrees(a, b)).abs() <= INFERENCE_DEGREES;
        let reach = length.max(30.0 / self.zoom);
        if near(angle, 0.0) {
            return Some(Inference {
                kind: InferenceKind::Horizontal,
                guide: (
                    Point {
                        x: start.x - reach * 0.25,
                        y: start.y,
                    },
                    Point {
                        x: end.x + (end.x - start.x).signum() * reach * 0.25,
                        y: start.y,
                    },
                ),
            });
        }
        if near(angle, std::f32::consts::FRAC_PI_2) {
            return Some(Inference {
                kind: InferenceKind::Vertical,
                guide: (
                    Point {
                        x: start.x,
                        y: start.y - reach * 0.25,
                    },
                    Point {
                        x: start.x,
                        y: end.y + (end.y - start.y).signum() * reach * 0.25,
                    },
                ),
            });
        }
        let tolerance = 6.0 / self.zoom;
        for shape in self.shapes.iter().rev() {
            let tangent = match *shape {
                // Ending on a circle at a right angle to its radius.
                Shape::Circle { center, radius, .. }
                    if (end.distance(center) - radius).abs() <= tolerance =>
                {
                    Some((end.y - center.y).atan2(end.x - center.x) + std::f32::consts::FRAC_PI_2)
                }
                // Starting at an arc end along the arc's tangent.
                Shape::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep,
                    ..
                } => [0.0, 1.0].into_iter().find_map(|fraction| {
                    let at = Shape::arc_point(center, radius, start_angle, sweep, fraction);
                    (at.distance(start) <= tolerance).then(|| {
                        (at.y - center.y).atan2(at.x - center.x) + std::f32::consts::FRAC_PI_2
                    })
                }),
                _ => None,
            };
            if let Some(direction) = tangent
                && near(angle, direction)
            {
                return Some(Inference {
                    kind: InferenceKind::Tangent(shape.id()),
                    guide: (start, end),
                });
            }
        }
        let near_end = 30.0 / self.zoom;
        for shape in self.shapes.iter().rev() {
            let Shape::Line { id, a, b } = *shape else {
                continue;
            };
            if a.distance(start) < 1e-5 && b.distance(end) < 1e-5 || !shape.hit(end, near_end) {
                continue;
            }
            let other = (b.y - a.y).atan2(b.x - a.x);
            let kind = if near(angle, other) {
                InferenceKind::Parallel(id)
            } else if perpendicular(angle, other) {
                InferenceKind::Perpendicular(id)
            } else {
                continue;
            };
            return Some(Inference {
                kind,
                guide: (a, b),
            });
        }
        None
    }

    /// Finish a line (or any draft) at `end`; with `auto` a line whose
    /// placement inferred horizontal/vertical gets that relation in the
    /// same undo step (web "Automatic constraints"). Returns whether a
    /// shape was committed and the relation added, if any.
    pub fn finish_draft_auto(&mut self, end: Point, auto: bool) -> (bool, Option<AxisConstraint>) {
        let inference = (self.tool == Tool::Line)
            .then_some(self.draft_start)
            .flatten()
            .and_then(|start| self.infer_segment(start, end));
        if !self.finish_draft(end) {
            return (false, None);
        }
        let axis = match inference.map(|i| i.kind) {
            Some(InferenceKind::Horizontal) => AxisConstraint::Horizontal,
            Some(InferenceKind::Vertical) => AxisConstraint::Vertical,
            _ => return (true, None),
        };
        let Some(id) = self.selected.filter(|_| auto) else {
            return (true, None);
        };
        if self.point_links.contains_key(&(id, 1)) {
            return (true, None);
        }
        let Some(index) = self.shapes.iter().position(|s| s.id() == id) else {
            return (true, None);
        };
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return (true, None);
        };
        let length = a.distance(b);
        let b = match axis {
            AxisConstraint::Horizontal => Point {
                x: a.x + length.copysign(b.x - a.x),
                y: a.y,
            },
            AxisConstraint::Vertical => Point {
                x: a.x,
                y: a.y + length.copysign(b.y - a.y),
            },
        };
        self.shapes[index] = Shape::Line { id, a, b };
        self.axis_constraints.insert(id, axis);
        self.revision.set(next_revision());
        (true, Some(axis))
    }
}
