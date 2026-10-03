//! Sketch fillet and chamfer of a line-line corner (web
//! `sketch-fillet.ts` / `sketch-chamfer.ts`), plus the endpoint relation
//! bookkeeping shared with split and trim.

use super::*;

/// How a chamfer's second value is read (web `ChamferMode`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChamferMode {
    /// `first` and `second` are the setbacks along line `a` and line `b`.
    TwoDistances,
    /// `first` is the setback along line `a`, `second` the angle in degrees
    /// between line `a` (towards the corner) and the chamfer.
    DistanceAngle,
}

impl ChamferMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::TwoDistances => "two-distances",
            Self::DistanceAngle => "distance-angle",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "two-distances" => Some(Self::TwoDistances),
            "distance-angle" => Some(Self::DistanceAngle),
            _ => None,
        }
    }
}

/// Driving values of a chamfer line (web `chamfer` relation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChamferSpec {
    /// Trimmed line endpoint joined to the chamfer's first endpoint.
    pub a: (u32, u8),
    /// Trimmed line endpoint joined to the chamfer's second endpoint.
    pub b: (u32, u8),
    pub first: f32,
    pub second: f32,
    pub mode: ChamferMode,
}

/// A line-line corner: the near endpoints, the intersection and the unit
/// directions from the intersection towards each far endpoint.
struct Corner {
    a: u32,
    a_end: u8,
    b: u32,
    b_end: u8,
    corner: Point,
    u: (f32, f32, f32),
    v: (f32, f32, f32),
    /// Angle between `u` and `v` in radians.
    angle: f32,
}

fn line_end(shape: &Shape, end: u8) -> Option<(Point, Point)> {
    match shape {
        Shape::Line { a, b, .. } => Some(if end == 0 { (*a, *b) } else { (*b, *a) }),
        _ => None,
    }
}

impl Document {
    fn line_corner(&self, first: u32, second: u32) -> Result<Corner, String> {
        let find = |id| self.shapes.iter().find(|shape| shape.id() == id);
        let (Some(Shape::Line { .. }), Some(Shape::Line { .. })) = (find(first), find(second))
        else {
            return Err("Select two lines".into());
        };
        if first == second {
            return Err("Select two different lines".into());
        }
        if self.has_offset(first) || self.has_offset(second) {
            return Err("Remove the offset before a fillet or chamfer".into());
        }
        if self.construction.contains(&first) != self.construction.contains(&second) {
            return Err("Select lines with matching construction status".into());
        }
        let (sa, sb) = (find(first).unwrap(), find(second).unwrap());
        let (a_end, b_end) = [(0, 0), (0, 1), (1, 0), (1, 1)]
            .into_iter()
            .min_by(|x, y| {
                let d = |(ea, eb): (u8, u8)| {
                    line_end(sa, ea)
                        .unwrap()
                        .0
                        .distance(line_end(sb, eb).unwrap().0)
                };
                d(*x).total_cmp(&d(*y))
            })
            .unwrap();
        let (an, af) = line_end(sa, a_end).unwrap();
        let (bn, bf) = line_end(sb, b_end).unwrap();
        let tolerance = 8.0 / self.zoom.max(0.2);
        if an.distance(bn) > tolerance {
            return Err("The lines must meet at a shared corner".into());
        }
        let da = (an.x - af.x, an.y - af.y);
        let db = (bn.x - bf.x, bn.y - bf.y);
        let (la, lb) = (da.0.hypot(da.1), db.0.hypot(db.1));
        if la <= 1e-6 || lb <= 1e-6 {
            return Err("Corner requires nonzero lines".into());
        }
        let cross = da.0 * db.1 - da.1 * db.0;
        if (cross / (la * lb)).abs() < 1e-5 {
            return Err("The lines are parallel".into());
        }
        let t = ((bf.x - af.x) * db.1 - (bf.y - af.y) * db.0) / cross;
        let corner = Point {
            x: af.x + da.0 * t,
            y: af.y + da.1 * t,
        };
        if corner.distance(an) > 2.0 * tolerance || corner.distance(bn) > 2.0 * tolerance {
            return Err("The lines must meet at a shared corner".into());
        }
        let direction = |far: Point| {
            let (x, y) = (far.x - corner.x, far.y - corner.y);
            let length = x.hypot(y);
            (x / length, y / length, length)
        };
        let (u, v) = (direction(af), direction(bf));
        if !(u.2 > 1e-6 && v.2 > 1e-6) || u.0 * da.0 + u.1 * da.1 > 0.0 {
            return Err("The lines must meet at a shared corner".into());
        }
        if v.0 * db.0 + v.1 * db.1 > 0.0 {
            return Err("The lines must meet at a shared corner".into());
        }
        let angle = (u.0 * v.1 - u.1 * v.0).abs().atan2(u.0 * v.0 + u.1 * v.1);
        Ok(Corner {
            a: first,
            a_end,
            b: second,
            b_end,
            corner,
            u,
            v,
            angle,
        })
    }

    /// Fixed-anchor index of endpoint 0 for a shape kind: line endpoints
    /// are anchors 0/1, arc endpoints 1/2 (0 is the center).
    fn endpoint_anchor_offset(&self, id: u32) -> Option<usize> {
        match self.shapes.iter().find(|shape| shape.id() == id)? {
            Shape::Line { .. } => Some(0),
            Shape::Arc { .. } => Some(1),
            _ => None,
        }
    }

    /// Detach every relation at one curve endpoint (line, arc or open
    /// spline), returning what was there so
    /// [`Self::attach_endpoint_relations`] can put it on another endpoint.
    pub(super) fn detach_endpoint_relations(&mut self, at: (u32, u8)) -> EndpointRelations {
        let joins_as_key = self.endpoint_joins.remove(&at);
        let joins_as_value: Vec<(u32, u8)> = self
            .endpoint_joins
            .iter()
            .filter(|(_, other)| **other == at)
            .map(|(key, _)| *key)
            .collect();
        for key in &joins_as_value {
            self.endpoint_joins.remove(key);
        }
        // Arcs joined at this endpoint whose smooth join follows this line.
        let tangent_arcs: Vec<(u32, u8)> = joins_as_key
            .into_iter()
            .chain(joins_as_value.iter().copied())
            .filter(|arc_end| {
                self.endpoint_tangents
                    .get(arc_end)
                    .is_some_and(|(line, _, _)| *line == at.0)
            })
            .collect();
        let chamfers: Vec<(u32, bool)> = self
            .chamfers
            .iter()
            .filter_map(|(line, spec)| {
                (spec.a == at)
                    .then_some((*line, true))
                    .or((spec.b == at).then_some((*line, false)))
            })
            .collect();
        let fixed = self.endpoint_anchor_offset(at.0).and_then(|offset| {
            self.fixed_anchors
                .remove(&(at.0, offset + at.1 as usize))
                .map(|position| (offset, position))
        });
        let smooth_as_key = self.smooth_joins.remove(&at);
        let smooth_as_value: Vec<(u32, u8)> = self
            .smooth_joins
            .iter()
            .filter(|(_, source)| **source == at)
            .map(|(key, _)| *key)
            .collect();
        for key in &smooth_as_value {
            self.smooth_joins.remove(key);
        }
        EndpointRelations {
            point_link: self.point_links.remove(&at),
            fixed,
            joins_as_key,
            joins_as_value,
            chamfers,
            tangent: self.endpoint_tangents.remove(&at),
            tangent_arcs,
            smooth_as_key,
            smooth_as_value,
        }
    }

    pub(super) fn attach_endpoint_relations(
        &mut self,
        to: (u32, u8),
        relations: EndpointRelations,
    ) {
        if let Some(point_id) = relations.point_link {
            self.point_links.insert(to, point_id);
        }
        if let Some((offset, position)) = relations.fixed {
            self.fixed_anchors
                .insert((to.0, offset + to.1 as usize), position);
        }
        if let Some(other) = relations.joins_as_key {
            self.endpoint_joins.insert(to, other);
        }
        for key in relations.joins_as_value {
            self.endpoint_joins.insert(key, to);
        }
        for (line, first) in relations.chamfers {
            if let Some(spec) = self.chamfers.get_mut(&line) {
                if first {
                    spec.a = to;
                } else {
                    spec.b = to;
                }
            }
        }
        if let Some(tangent) = relations.tangent {
            self.endpoint_tangents.insert(to, tangent);
        }
        for arc_end in relations.tangent_arcs {
            if let Some((line, _, _)) = self.endpoint_tangents.get_mut(&arc_end) {
                *line = to.0;
            }
        }
        if let Some(source) = relations.smooth_as_key {
            self.smooth_joins.insert(to, source);
        }
        for key in relations.smooth_as_value {
            self.smooth_joins.insert(key, to);
        }
    }

    /// Drop relations at an endpoint that is cut away (web
    /// `removeCornerRelations`): links, fixes, joins, smooth joins through a
    /// joined arc, and chamfers measured from it.
    pub(super) fn drop_endpoint_relations(&mut self, at: (u32, u8)) {
        let joined: Vec<(u32, u8)> = self
            .endpoint_joins
            .iter()
            .filter(|(key, other)| **key == at || **other == at)
            .flat_map(|(key, other)| [*key, *other])
            .filter(|end| *end != at)
            .collect();
        self.endpoint_tangents
            .retain(|arc_end, (line, _, _)| !(*line == at.0 && joined.contains(arc_end)));
        let relations = self.detach_endpoint_relations(at);
        for (line, _) in relations.chamfers {
            self.chamfers.remove(&line);
        }
    }

    fn cut_corner(&mut self, corner: &Corner) {
        self.drop_endpoint_relations((corner.a, corner.a_end));
        self.drop_endpoint_relations((corner.b, corner.b_end));
        for id in [corner.a, corner.b] {
            self.driving_dimensions
                .remove(&(id, DimensionField::LineLength));
        }
    }

    fn set_line_end(&mut self, id: u32, end: u8, at: Point) {
        if let Some(Shape::Line { a, b, .. }) = self.shapes.iter_mut().find(|s| s.id() == id) {
            if end == 0 {
                *a = at;
            } else {
                *b = at;
            }
        }
    }

    fn fresh_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Round the corner shared by two lines with a tangent arc of `radius`.
    /// Both lines are trimmed; the arc is joined to them with coincident and
    /// smooth-join relations, and its radius is a driving dimension.
    pub fn fillet_lines(&mut self, first: u32, second: u32, radius: f32) -> Result<u32, String> {
        if !radius.is_finite() || radius <= 0.0 {
            return Err("Fillet radius must be positive".into());
        }
        let corner = self.line_corner(first, second)?;
        let half = corner.angle * 0.5;
        if half < 1e-5 || std::f32::consts::FRAC_PI_2 - half < 1e-5 {
            return Err("Fillet requires a non-collinear corner".into());
        }
        let setback = radius / half.tan();
        if setback >= corner.u.2.min(corner.v.2) - 1e-5 {
            return Err("Fillet radius is too large for these lines".into());
        }
        let c = corner.corner;
        let (u, v) = (corner.u, corner.v);
        let bisector = ((u.0 + v.0), (u.1 + v.1));
        let bisector_length = bisector.0.hypot(bisector.1);
        let center_distance = radius / half.sin();
        let center = Point {
            x: c.x + bisector.0 / bisector_length * center_distance,
            y: c.y + bisector.1 / bisector_length * center_distance,
        };
        let start = Point {
            x: c.x + u.0 * setback,
            y: c.y + u.1 * setback,
        };
        let end = Point {
            x: c.x + v.0 * setback,
            y: c.y + v.1 * setback,
        };
        let angle_of = |p: Point| ((p.y - center.y) as f64).atan2((p.x - center.x) as f64);
        let start_angle = angle_of(start);
        let delta = angle_of(end) - start_angle;
        let sweep = delta.sin().atan2(delta.cos());
        let before = self.snapshot();
        self.cut_corner(&corner);
        self.set_line_end(corner.a, corner.a_end, start);
        self.set_line_end(corner.b, corner.b_end, end);
        let arc = self.fresh_id();
        self.shapes.push(Shape::Arc {
            id: arc,
            center,
            radius,
            start_angle,
            sweep,
            segments: 128,
        });
        if self.construction.contains(&corner.a) {
            self.construction.insert(arc);
        }
        let sweep_sign: i8 = if sweep > 0.0 { 1 } else { -1 };
        for (line, line_end_index, arc_end) in
            [(corner.a, corner.a_end, 0u8), (corner.b, corner.b_end, 1u8)]
        {
            let phi = start_angle + if arc_end == 0 { 0.0 } else { sweep };
            let tangent = (
                -phi.sin() * sweep_sign as f64,
                phi.cos() * sweep_sign as f64,
            );
            let direction = match self.shapes.iter().find(|s| s.id() == line) {
                Some(Shape::Line { a, b, .. }) => ((b.x - a.x) as f64, (b.y - a.y) as f64),
                _ => (0.0, 0.0),
            };
            let line_sign: i8 = if tangent.0 * direction.0 + tangent.1 * direction.1 >= 0.0 {
                1
            } else {
                -1
            };
            self.endpoint_joins
                .insert((line, line_end_index), (arc, arc_end));
            self.endpoint_tangents
                .insert((arc, arc_end), (line, line_sign, sweep_sign));
        }
        self.driving_dimensions
            .insert((arc, DimensionField::CurveRadius), radius);
        self.history.push(before);
        self.future.clear();
        self.selected = Some(arc);
        Ok(arc)
    }

    fn chamfer_second_setback(
        corner: &Corner,
        first: f32,
        mode: ChamferMode,
        second: f32,
    ) -> Result<f32, String> {
        match mode {
            ChamferMode::TwoDistances => Ok(second),
            ChamferMode::DistanceAngle => {
                let alpha = second.to_radians();
                if !second.is_finite()
                    || alpha <= 0.0
                    || alpha + corner.angle >= std::f32::consts::PI - 1e-6
                {
                    return Err("Chamfer angle must fit inside the corner triangle".into());
                }
                Ok(first * alpha.sin() / (alpha + corner.angle).sin())
            }
        }
    }

    /// Cut the corner shared by two lines with a straight chamfer line.
    pub fn chamfer_lines(
        &mut self,
        first_line: u32,
        second_line: u32,
        mode: ChamferMode,
        first: f32,
        second: f32,
    ) -> Result<u32, String> {
        if !(first.is_finite() && first > 0.0 && second.is_finite() && second > 0.0) {
            return Err("Chamfer values must be positive".into());
        }
        let corner = self.line_corner(first_line, second_line)?;
        if corner.angle < 1e-5 || std::f32::consts::PI - corner.angle < 1e-5 {
            return Err("Chamfer requires a non-collinear corner".into());
        }
        let second_setback = Self::chamfer_second_setback(&corner, first, mode, second)?;
        if first >= corner.u.2 - 1e-5 || second_setback >= corner.v.2 - 1e-5 {
            return Err("Chamfer setback is too large for these lines".into());
        }
        let c = corner.corner;
        let start = Point {
            x: c.x + corner.u.0 * first,
            y: c.y + corner.u.1 * first,
        };
        let end = Point {
            x: c.x + corner.v.0 * second_setback,
            y: c.y + corner.v.1 * second_setback,
        };
        let before = self.snapshot();
        self.cut_corner(&corner);
        self.set_line_end(corner.a, corner.a_end, start);
        self.set_line_end(corner.b, corner.b_end, end);
        let line = self.fresh_id();
        self.shapes.push(Shape::Line {
            id: line,
            a: start,
            b: end,
        });
        if self.construction.contains(&corner.a) {
            self.construction.insert(line);
        }
        self.endpoint_joins
            .insert((corner.a, corner.a_end), (line, 0));
        self.endpoint_joins
            .insert((corner.b, corner.b_end), (line, 1));
        self.chamfers.insert(
            line,
            ChamferSpec {
                a: (corner.a, corner.a_end),
                b: (corner.b, corner.b_end),
                first,
                second,
                mode,
            },
        );
        self.history.push(before);
        self.future.clear();
        self.selected = Some(line);
        Ok(line)
    }

    /// Change the driving values of the selected chamfer line.
    pub fn set_chamfer(
        &mut self,
        mode: ChamferMode,
        first: f32,
        second: f32,
    ) -> Result<(), String> {
        let id = self.selected.ok_or("Select a chamfer line")?;
        let spec = *self.chamfers.get(&id).ok_or("Select a chamfer line")?;
        if !(first.is_finite() && first > 0.0 && second.is_finite() && second > 0.0) {
            return Err("Chamfer values must be positive".into());
        }
        if mode == ChamferMode::DistanceAngle && second >= 180.0 {
            return Err("Chamfer angle must fit inside the corner triangle".into());
        }
        let before = self.snapshot();
        self.chamfers.insert(
            id,
            ChamferSpec {
                first,
                second,
                mode,
                ..spec
            },
        );
        if let Err(error) = self.solve_constraints(&[]) {
            self.restore(before);
            self.selected = Some(id);
            return Err(error);
        }
        self.history.push(before);
        self.future.clear();
        self.selected = Some(id);
        Ok(())
    }

    /// Current chamfer setbacks `(first, second)` and the angle in degrees
    /// between line `a` (towards the corner) and the chamfer line.
    pub fn chamfer_measure(&self, id: u32) -> Option<(f32, f32, f32)> {
        let spec = self.chamfers.get(&id)?;
        let find = |id| self.shapes.iter().find(|shape| shape.id() == id);
        let (an, af) = line_end(find(spec.a.0)?, spec.a.1)?;
        let (bn, bf) = line_end(find(spec.b.0)?, spec.b.1)?;
        let da = (an.x - af.x, an.y - af.y);
        let db = (bn.x - bf.x, bn.y - bf.y);
        let cross = da.0 * db.1 - da.1 * db.0;
        if cross.abs() < 1e-9 {
            return None;
        }
        let t = ((bf.x - af.x) * db.1 - (bf.y - af.y) * db.0) / cross;
        let c = Point {
            x: af.x + da.0 * t,
            y: af.y + da.1 * t,
        };
        let w = (bn.x - an.x, bn.y - an.y);
        let angle = (da.0 * w.1 - da.1 * w.0)
            .abs()
            .atan2(da.0 * w.0 + da.1 * w.1)
            .to_degrees();
        Some((an.distance(c), bn.distance(c), angle))
    }
}

/// Relations detached from one curve endpoint.
pub(super) struct EndpointRelations {
    point_link: Option<u32>,
    /// Fixed-anchor index offset of the shape kind and the fixed position.
    fixed: Option<(usize, Point)>,
    joins_as_key: Option<(u32, u8)>,
    joins_as_value: Vec<(u32, u8)>,
    /// Chamfer line and whether the endpoint was its `a` side.
    chamfers: Vec<(u32, bool)>,
    /// Smooth join of this arc endpoint to a line (`endpoint_tangents`).
    tangent: Option<(u32, i8, i8)>,
    /// Arc endpoints joined here whose `endpoint_tangents` line is this one.
    tangent_arcs: Vec<(u32, u8)>,
    smooth_as_key: Option<(u32, u8)>,
    smooth_as_value: Vec<(u32, u8)>,
}
