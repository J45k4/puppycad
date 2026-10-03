//! The Offset tool, ported from `src/sketch-offset.ts` (the web `offset`
//! relation) and `src/sketch-offset-chain.ts` (the web `offsetChain`
//! relation for mitered line chains).

use super::*;
use solver::Segment;

/// Endpoints closer than this (mm) count as joined when collecting and
/// ordering an offset chain.
const CHAIN_TOLERANCE: f32 = 1e-3;

/// Directed chain sources, whether the chain is closed, and its side.
pub type OrderedChain = (Vec<(u32, bool)>, bool, i8);

/// Mitered offset of a connected line chain (web `offsetChain`).
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetChain {
    /// Source lines in chain order; `true` when the line runs backwards
    /// (`b` to `a`) along the chain.
    pub sources: Vec<(u32, bool)>,
    /// Offset lines, one per source in the same order.
    pub targets: Vec<u32>,
    pub closed: bool,
    /// 1 or -1: positive distances move to the left of the ordered chain,
    /// or outward for a closed chain.
    pub side: i8,
    /// Signed offset distance in mm (the driving value).
    pub distance: f32,
}

/// Offset of a single shape, keeping its id. Positive distances move lines
/// to the left of their direction and grow circles and arcs. Like the web
/// app, ellipses and splines have no offset curve yet.
pub fn offset_shape(shape: &Shape, distance: f32) -> Result<Shape, String> {
    if !distance.is_finite() {
        return Err("Offset distance must be finite".into());
    }
    let mut result = shape.clone();
    match &mut result {
        Shape::Line { a, b, .. } => {
            if a.distance(*b) < 1e-6 {
                return Err("Cannot offset a zero-length line".into());
            }
            let ((p, q), (r, s)) = solver::offset_segment(
                ((a.x as f64, a.y as f64), (b.x as f64, b.y as f64)),
                distance as f64,
            );
            *a = Point {
                x: p as f32,
                y: q as f32,
            };
            *b = Point {
                x: r as f32,
                y: s as f32,
            };
        }
        Shape::Circle { radius, .. } | Shape::Arc { radius, .. } => {
            *radius += distance;
            if radius.is_nan() || *radius <= 1e-6 {
                return Err("Offset radius must stay positive".into());
            }
        }
        Shape::Ellipse { .. } | Shape::EllipticArc { .. } => {
            return Err(
                "Ellipse offset requires a general offset curve and is not supported yet".into(),
            );
        }
        Shape::Polygon { radius, sides, .. } => {
            *radius += distance / (std::f32::consts::PI / (*sides).max(3) as f32).cos();
            if radius.is_nan() || *radius <= 1e-6 {
                return Err("Offset would collapse the polygon".into());
            }
        }
        Shape::RotatedRectangle { width, height, .. } => {
            *width += 2.0 * distance;
            *height += 2.0 * distance;
            if width.is_nan() || height.is_nan() || width.min(*height) <= 1e-6 {
                return Err("Offset would collapse the rectangle".into());
            }
        }
        Shape::Capsule { width, .. } => {
            *width += 2.0 * distance;
            if width.is_nan() || *width <= 1e-6 {
                return Err("Offset would collapse the slot".into());
            }
        }
        Shape::Rectangle { a, b, .. } => {
            let (sx, sy) = ((b.x - a.x).signum(), (b.y - a.y).signum());
            if (b.x - a.x).abs() + 2.0 * distance <= 1e-6
                || (b.y - a.y).abs() + 2.0 * distance <= 1e-6
            {
                return Err("Offset would collapse the rectangle".into());
            }
            a.x -= sx * distance;
            b.x += sx * distance;
            a.y -= sy * distance;
            b.y += sy * distance;
        }
        Shape::Spline { .. } => return Err("Spline offset is not supported yet".into()),
        Shape::Point { .. } => return Err("A sketch point has no offset curve".into()),
    }
    Ok(result)
}

fn segment(a: Point, b: Point) -> Segment {
    ((a.x as f64, a.y as f64), (b.x as f64, b.y as f64))
}

fn point((x, y): (f64, f64)) -> Point {
    Point {
        x: x as f32,
        y: y as f32,
    }
}

fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

/// Validated mitered offset of directed chain segments, with the checks of
/// the web `offsetLineChain`: no reversing corner, no collapsed or reversed
/// edge, and no self-intersection.
pub fn offset_chain_segments(
    source: &[Segment],
    closed: bool,
    distance: f64,
) -> Result<Vec<Segment>, String> {
    let (lines, ok) = solver::mitered_offset(source, closed, distance);
    if !ok {
        return Err("Offset cannot join a reversing corner".into());
    }
    for (line, original) in lines.iter().zip(source) {
        let (a, b) = (sub(line.1, line.0), sub(original.1, original.0));
        if a.0 * b.0 + a.1 * b.1 <= 1e-8 {
            return Err("Offset collapses or reverses an edge. Reduce the distance".into());
        }
    }
    for i in 0..lines.len() {
        for j in i + 2..lines.len() {
            if closed && i == 0 && j == lines.len() - 1 {
                continue;
            }
            let (a, b) = (lines[i], lines[j]);
            let (da, db) = (sub(a.1, a.0), sub(b.1, b.0));
            let d = cross(da, db);
            if d.abs() < 1e-12 {
                continue;
            }
            let delta = sub(b.0, a.0);
            let (t, u) = (cross(delta, db) / d, cross(delta, da) / d);
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                return Err("Offset would intersect itself. Reduce the distance".into());
            }
        }
    }
    Ok(lines)
}

impl Document {
    fn line_points(&self, id: u32) -> Option<(Point, Point)> {
        match self.shapes.iter().find(|shape| shape.id() == id)? {
            Shape::Line { a, b, .. } => Some((*a, *b)),
            _ => None,
        }
    }

    /// Directed source segments of an offset chain.
    pub(super) fn chain_segments(&self, sources: &[(u32, bool)]) -> Option<Vec<Segment>> {
        sources
            .iter()
            .map(|&(id, reversed)| {
                let (a, b) = self.line_points(id)?;
                Some(if reversed {
                    segment(b, a)
                } else {
                    segment(a, b)
                })
            })
            .collect()
    }

    /// Lines joined end to end with line `id` (through vertices where exactly
    /// two lines meet), in document order. Coincident links keep joined ends
    /// together, so joins are found from endpoint positions.
    pub fn connected_line_chain(&self, id: u32) -> Vec<u32> {
        // Offset results are never part of a source chain.
        let results: HashSet<u32> = self.offset_followers().collect();
        let lines: Vec<(u32, Point, Point)> = self
            .shapes
            .iter()
            .filter_map(|shape| match shape {
                Shape::Line { id, a, b } if !results.contains(id) => Some((*id, *a, *b)),
                _ => None,
            })
            .collect();
        if !lines.iter().any(|line| line.0 == id) {
            return vec![id];
        }
        let at = |p: Point| -> Vec<u32> {
            lines
                .iter()
                .filter(|(_, a, b)| {
                    a.distance(p) < CHAIN_TOLERANCE || b.distance(p) < CHAIN_TOLERANCE
                })
                .map(|line| line.0)
                .collect()
        };
        let mut found = HashSet::from([id]);
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let Some(&(_, a, b)) = lines.iter().find(|line| line.0 == current) else {
                continue;
            };
            for end in [a, b] {
                let meeting = at(end);
                if meeting.len() != 2 {
                    continue;
                }
                for other in meeting {
                    if found.insert(other) {
                        stack.push(other);
                    }
                }
            }
        }
        lines
            .iter()
            .map(|line| line.0)
            .filter(|id| found.contains(id))
            .collect()
    }

    /// Order one unbranched chain like the web `orderOffsetChain`; returns
    /// the directed sources, whether the chain is closed and its side.
    pub fn order_offset_chain(&self, ids: &[u32]) -> Result<OrderedChain, String> {
        let lines: Vec<(u32, Point, Point)> = ids
            .iter()
            .map(|&id| self.line_points(id).map(|(a, b)| (id, a, b)))
            .collect::<Option<_>>()
            .ok_or("Select a connected chain of at least two lines")?;
        if lines.len() < 2 {
            return Err("Select a connected chain of at least two lines".into());
        }
        let mut vertices: Vec<(Point, Vec<usize>)> = Vec::new();
        let mut ends = Vec::new();
        for (i, &(_, a, b)) in lines.iter().enumerate() {
            let mut pair = [0; 2];
            for (slot, p) in [a, b].into_iter().enumerate() {
                let index = match vertices
                    .iter()
                    .position(|(v, _)| v.distance(p) < CHAIN_TOLERANCE)
                {
                    Some(index) => index,
                    None => {
                        vertices.push((p, Vec::new()));
                        vertices.len() - 1
                    }
                };
                vertices[index].1.push(i);
                pair[slot] = index;
            }
            ends.push(pair);
        }
        if vertices.iter().any(|(_, edges)| edges.len() > 2)
            || lines
                .iter()
                .any(|(_, a, b)| a.distance(*b) < CHAIN_TOLERANCE)
        {
            return Err("Offset chains cannot branch or contain zero-length lines".into());
        }
        let loose: Vec<usize> = (0..vertices.len())
            .filter(|&i| vertices[i].1.len() == 1)
            .collect();
        if !loose.is_empty() && loose.len() != 2 {
            return Err("Select one connected chain".into());
        }
        let mut vertex = loose.first().copied().unwrap_or(ends[0][0]);
        let mut used = HashSet::new();
        let mut sources = Vec::new();
        while sources.len() < lines.len() {
            let next = vertices[vertex]
                .1
                .iter()
                .copied()
                .find(|i| !used.contains(i))
                .ok_or("Select one connected chain")?;
            let reversed = ends[next][1] == vertex;
            sources.push((lines[next].0, reversed));
            used.insert(next);
            vertex = ends[next][if reversed { 0 } else { 1 }];
        }
        let closed = loose.is_empty();
        let directed = self
            .chain_segments(&sources)
            .ok_or("Offset source must be a line")?;
        let area: f64 = directed.iter().map(|&(a, b)| cross(a, b)).sum();
        if closed && area.abs() < 1e-8 {
            return Err("Closed offset chain must enclose an area".into());
        }
        Ok((sources, closed, if closed && area > 0.0 { -1 } else { 1 }))
    }

    /// Whether shape `id` is a source or target of an offset relation.
    pub fn has_offset(&self, id: u32) -> bool {
        self.offsets
            .iter()
            .any(|(&target, &(source, _))| target == id || source == id)
            || self.offset_chains.iter().any(|chain| {
                chain.targets.contains(&id) || chain.sources.iter().any(|(s, _)| *s == id)
            })
    }

    /// Driving offset distance of an offset target.
    pub fn offset_distance(&self, target: u32) -> Option<f32> {
        self.offsets
            .get(&target)
            .map(|(_, distance)| *distance)
            .or_else(|| {
                self.offset_chains
                    .iter()
                    .find(|chain| chain.targets.contains(&target))
                    .map(|chain| chain.distance)
            })
    }

    /// Offset tool: offset the selected shape by `distance` (signed; see
    /// [`offset_shape`]). A selected line joined end to end with other lines
    /// offsets the whole chain with mitered joins. The new shapes are linked
    /// to their sources by the web `offset` / `offsetChain` relation and the
    /// last one is selected. One undo step. Returns the new shape ids.
    pub fn offset_selected(&mut self, distance: f32) -> Result<Vec<u32>, String> {
        let id = self.selected.ok_or("Select a shape to offset")?;
        if !distance.is_finite() || distance.abs() < 1e-6 {
            return Err("Offset distance must be non-zero".into());
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .ok_or("Selected shape is missing")?;
        if matches!(source, Shape::Line { .. }) {
            let chain = self.connected_line_chain(id);
            if chain.len() >= 2 {
                return self.offset_chain(&chain, distance);
            }
        }
        let copy = offset_shape(source, distance)?.with_id(self.next_id);
        self.history.push(self.snapshot());
        self.future.clear();
        let target = copy.id();
        self.next_id = self.next_id.saturating_add(1);
        if self.construction.contains(&id) {
            self.construction.insert(target);
        }
        self.shapes.push(copy);
        self.offsets.insert(target, (id, distance));
        self.selected = Some(target);
        self.selected_spline_point = None;
        self.solve_or_rollback(&[Pin::Shape(id)])?;
        Ok(vec![target])
    }

    /// Mitered offset of the chain formed by lines `ids`.
    pub fn offset_chain(&mut self, ids: &[u32], distance: f32) -> Result<Vec<u32>, String> {
        if !distance.is_finite() || distance.abs() < 1e-6 {
            return Err("Offset distance must be non-zero".into());
        }
        let (sources, closed, side) = self.order_offset_chain(ids)?;
        let segments = self
            .chain_segments(&sources)
            .ok_or("Offset source must be a line")?;
        let lines = offset_chain_segments(&segments, closed, distance as f64 * side as f64)?;
        self.history.push(self.snapshot());
        self.future.clear();
        let mut targets = Vec::new();
        for (&(source, _), (a, b)) in sources.iter().zip(lines) {
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            if self.construction.contains(&source) {
                self.construction.insert(id);
            }
            self.shapes.push(Shape::Line {
                id,
                a: point(a),
                b: point(b),
            });
            targets.push(id);
        }
        let pins: Vec<Pin> = sources.iter().map(|&(id, _)| Pin::Shape(id)).collect();
        self.offset_chains.push(OffsetChain {
            sources,
            targets: targets.clone(),
            closed,
            side,
            distance,
        });
        self.selected = targets.last().copied();
        self.selected_spline_point = None;
        self.solve_or_rollback(&pins)?;
        Ok(targets)
    }

    /// Change the driving distance of the selected offset target (single or
    /// chain). Zero distances are rejected. One undo step.
    pub fn set_offset_distance(&mut self, distance: f32) -> Result<(), String> {
        if !distance.is_finite() || distance.abs() < 1e-6 {
            return Err("Offset distance must be non-zero".into());
        }
        let target = self.selected.ok_or("Select an offset shape")?;
        if let Some(&(source, old)) = self.offsets.get(&target) {
            if (old - distance).abs() <= 1e-6 {
                return Ok(());
            }
            let shape = self
                .shapes
                .iter()
                .find(|shape| shape.id() == source)
                .ok_or("Offset source is missing")?;
            let next = offset_shape(shape, distance)?.with_id(target);
            let index = self
                .shapes
                .iter()
                .position(|shape| shape.id() == target)
                .ok_or("Offset shape is missing")?;
            self.history.push(self.snapshot());
            self.future.clear();
            self.shapes[index] = next;
            self.offsets.insert(target, (source, distance));
            return self.solve_or_rollback(&[Pin::Shape(source)]);
        }
        let chain_index = self
            .offset_chains
            .iter()
            .position(|chain| chain.targets.contains(&target))
            .ok_or("Selected shape is not an offset")?;
        let chain = self.offset_chains[chain_index].clone();
        if (chain.distance - distance).abs() <= 1e-6 {
            return Ok(());
        }
        let segments = self
            .chain_segments(&chain.sources)
            .ok_or("Offset source is missing")?;
        let lines =
            offset_chain_segments(&segments, chain.closed, distance as f64 * chain.side as f64)?;
        self.history.push(self.snapshot());
        self.future.clear();
        for (&id, (a, b)) in chain.targets.iter().zip(lines) {
            if let Some(shape) = self.shapes.iter_mut().find(|shape| shape.id() == id) {
                *shape = Shape::Line {
                    id,
                    a: point(a),
                    b: point(b),
                };
            }
        }
        self.offset_chains[chain_index].distance = distance;
        let pins: Vec<Pin> = chain
            .sources
            .iter()
            .map(|&(id, _)| Pin::Shape(id))
            .collect();
        self.selected = Some(target);
        self.solve_or_rollback(&pins)
    }

    /// Shapes whose geometry follows an offset relation.
    pub(super) fn offset_followers(&self) -> impl Iterator<Item = u32> + '_ {
        self.offsets.keys().copied().chain(
            self.offset_chains
                .iter()
                .flat_map(|chain| chain.targets.iter().copied()),
        )
    }

    /// Curves and outlines whose size and angles an offset relation drives.
    pub(super) fn offset_curves(&self) -> impl Iterator<Item = u32> + '_ {
        self.offsets
            .iter()
            .flat_map(|(&target, &(source, _))| [target, source])
    }

    /// Remove every offset relation that refers to shape `id`.
    pub(super) fn forget_offsets(&mut self, id: u32) {
        self.offsets
            .retain(|&target, &mut (source, _)| target != id && source != id);
        self.offset_chains.retain(|chain| {
            !chain.targets.contains(&id) && !chain.sources.iter().any(|(s, _)| *s == id)
        });
    }

    /// Solver constraints for every offset relation.
    pub(super) fn offset_constraints(
        &self,
        b: &mut super::solve::SolverBuilder<'_>,
        constraints: &mut Vec<solver::Constraint>,
    ) {
        use super::solve::SolverEntity as E;
        use solver::{Constraint, Value};
        let mut offsets: Vec<_> = self.offsets.iter().map(|(&t, &r)| (t, r)).collect();
        offsets.sort_by_key(|(target, _)| *target);
        for (target, (source, distance)) in offsets {
            let distance = distance as f64;
            match (b.entity(source), b.entity(target)) {
                (Some(E::Line(s)), Some(E::Line(t))) => constraints.push(Constraint::OffsetLine {
                    source: s,
                    target: t,
                    distance,
                }),
                (Some(E::Circle(s)), Some(E::Circle(t))) => constraints.extend([
                    Constraint::Concentric(s, t),
                    Constraint::RadiusDifference {
                        source: s,
                        target: t,
                        value: Value::Const(-distance),
                    },
                ]),
                (Some(E::Arc(s)), Some(E::Arc(t))) => constraints.extend([
                    Constraint::Concentric(s.circle, t.circle),
                    Constraint::RadiusDifference {
                        source: s.circle,
                        target: t.circle,
                        value: Value::Const(-distance),
                    },
                    Constraint::SameValue(s.start, t.start),
                    Constraint::SameValue(s.sweep, t.sweep),
                ]),
                (Some(E::Shape(s)), Some(E::Shape(t))) => {
                    constraints.push(Constraint::OffsetShape {
                        source: s,
                        target: t,
                        distance,
                    })
                }
                _ => {}
            }
        }
        for chain in &self.offset_chains {
            let sources: Option<Vec<_>> = chain
                .sources
                .iter()
                .map(|&(id, reversed)| b.line(id).map(|line| (line, reversed)))
                .collect();
            let targets: Option<Vec<_>> = chain.targets.iter().map(|&id| b.line(id)).collect();
            if let (Some(sources), Some(targets)) = (sources, targets) {
                constraints.push(Constraint::OffsetChain {
                    sources,
                    targets,
                    closed: chain.closed,
                    distance: chain.distance as f64 * chain.side as f64,
                });
            }
        }
    }
}
