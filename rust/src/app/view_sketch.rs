use super::*;

impl App {
    /// A face sketch's supporting body (web `sketchSupportEdges`), drawn
    /// dashed between the grid and the sketch geometry.
    pub(super) fn paint_reference_edges(&self, painter: &mut Painter, layout: Layout) {
        if self.document.reference_edges.is_empty() {
            return;
        }
        let origin = self.viewport_origin();
        let color: Color = [0.42, 0.55, 0.66, 1.0];
        for &(a, b) in &self.document.reference_edges {
            let (a, b) = (
                self.document.world_to_screen(a, origin),
                self.document.world_to_screen(b, origin),
            );
            let length = a.distance(b);
            let steps = (length / 9.0).ceil().max(1.0) as usize;
            for step in (0..steps).step_by(2) {
                let t0 = step as f32 / steps as f32;
                let t1 = ((step + 1) as f32 / steps as f32).min(1.0);
                painter.line(
                    a.x + (b.x - a.x) * t0,
                    a.y + (b.y - a.y) * t0,
                    a.x + (b.x - a.x) * t1,
                    a.y + (b.y - a.y) * t1,
                    1.5,
                    color,
                );
            }
        }
        painter.text(
            layout.viewport.x + 18.0,
            layout.viewport.y + layout.viewport.h - 24.0,
            "FACE SKETCH  |  5 HIDE BODY CONTEXT",
            1.0,
            MUTED,
        );
    }

    pub(super) fn paint_grid(&self, painter: &mut Painter, layout: Layout) {
        let viewport = layout.viewport;
        let origin = self.viewport_origin();
        let spacing = 25.0 * self.document.zoom;
        if spacing > 8.0 {
            let first_x =
                viewport.x + (origin.x + self.document.pan.x - viewport.x).rem_euclid(spacing);
            let first_y =
                viewport.y + (origin.y + self.document.pan.y - viewport.y).rem_euclid(spacing);
            let grid: Color = [0.13, 0.16, 0.20, 1.0];
            let mut x = first_x;
            while x < viewport.x + viewport.w {
                painter.line(x, viewport.y, x, viewport.y + viewport.h, 1.0, grid);
                x += spacing;
            }
            let mut y = first_y;
            while y < viewport.y + viewport.h {
                painter.line(viewport.x, y, viewport.x + viewport.w, y, 1.0, grid);
                y += spacing;
            }
        }
        let zero = self
            .document
            .world_to_screen(Point { x: 0.0, y: 0.0 }, origin);
        if zero.x >= viewport.x && zero.x <= viewport.x + viewport.w {
            painter.line(
                zero.x,
                viewport.y,
                zero.x,
                viewport.y + viewport.h,
                1.5,
                BORDER,
            );
        }
        if zero.y >= viewport.y && zero.y <= viewport.y + viewport.h {
            painter.line(
                viewport.x,
                zero.y,
                viewport.x + viewport.w,
                zero.y,
                1.5,
                BORDER,
            );
        }
    }

    /// Small squares on the selected shape's draggable anchors; the hovered
    /// or dragged one is filled.
    pub(super) fn paint_anchor_handles(&self, painter: &mut Painter, layout: Layout) {
        if self.document.tool != Tool::Select || self.read_only() {
            return;
        }
        let Some(id) = self.document.selected else {
            return;
        };
        let origin = self.viewport_origin();
        let active = self.document.dragged_anchor().or_else(|| {
            (!self.document.dragging() && layout.viewport.contains(self.cursor.x, self.cursor.y))
                .then(|| {
                    self.document
                        .anchor_at(self.document.screen_to_world(self.cursor, origin))
                })
                .flatten()
        });
        for (index, point) in self.document.drag_anchors(id) {
            let at = self.document.world_to_screen(point, origin);
            if !layout.viewport.contains(at.x, at.y) {
                continue;
            }
            if active == Some((id, index)) {
                let rect = Rect {
                    x: at.x - 5.0,
                    y: at.y - 5.0,
                    w: 10.0,
                    h: 10.0,
                };
                painter.rect(rect, ACCENT);
            } else {
                let rect = Rect {
                    x: at.x - 3.5,
                    y: at.y - 3.5,
                    w: 7.0,
                    h: 7.0,
                };
                painter.outline(rect, 1.5, ACCENT);
            }
        }
    }

    pub(super) fn paint_shapes(&self, painter: &mut Painter, layout: Layout) {
        let origin = self.viewport_origin();
        let selected = self.document.selected_ids();
        for shape in &self.document.shapes {
            self.paint_shape(painter, shape, origin, selected.contains(&shape.id()));
        }
        self.paint_anchor_handles(painter, layout);
        self.paint_box_select(painter, layout);
        self.paint_dimension_labels(painter, layout);
        if let Some(start) = self.document.draft_start
            && layout.viewport.contains(self.cursor.x, self.cursor.y)
        {
            let raw_end = self.document.screen_to_world(self.cursor, origin);
            let end = self
                .document
                .snap_point(raw_end, self.snap_grid, self.snap_geometry)
                .map_or(raw_end, |snap| snap.point);
            let draft = match self.document.tool {
                Tool::Line => Some(Shape::Line {
                    id: 0,
                    a: start,
                    b: end,
                }),
                Tool::Rectangle => Some(Shape::Rectangle {
                    id: 0,
                    a: start,
                    b: end,
                }),
                Tool::CenterRectangle => Some(Shape::Rectangle {
                    id: 0,
                    a: Point {
                        x: start.x * 2.0 - end.x,
                        y: start.y * 2.0 - end.y,
                    },
                    b: end,
                }),
                Tool::ThreePointRectangle => match self.document.draft_end {
                    Some(second) => Shape::rectangle_from_three_points(0, start, second, end),
                    None => Some(Shape::Line {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                },
                Tool::ThreePointCenterRectangle => match self.document.draft_end {
                    Some(midpoint) => Shape::rectangle_from_center_side(0, start, midpoint, end),
                    None => Some(Shape::Line {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                },
                Tool::Circle => Some(Shape::Circle {
                    id: 0,
                    center: start,
                    radius: start.distance(end),
                    segments: 64,
                }),
                Tool::ThreePointCircle => match self.document.draft_end {
                    Some(second) => Shape::circle_from_three_points(0, start, second, end),
                    None => Some(Shape::Line {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                },
                Tool::Ellipse => match self.document.draft_end {
                    Some(first_axis) => Shape::ellipse_from_three_points(0, start, first_axis, end),
                    None => Some(Shape::Line {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                },
                Tool::Arc => match self.document.draft_end {
                    Some(arc_end) => Shape::arc_from_three_points(0, start, arc_end, end),
                    None => Some(Shape::Line {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                },
                Tool::CenterArc => match self.document.draft_end {
                    Some(radial_start) => Shape::arc_from_center_points(
                        0,
                        start,
                        radial_start,
                        end,
                        self.document.arc_clockwise,
                    ),
                    None => Some(Shape::Line {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                },
                Tool::Polygon => Some(Shape::Polygon {
                    id: 0,
                    center: start,
                    radius: start.distance(end),
                    sides: 6,
                    rotation: (end.y - start.y).atan2(end.x - start.x).to_degrees(),
                }),
                Tool::Slot => match self.document.draft_end {
                    Some(to) => Shape::capsule_from_three_points(0, start, to, end),
                    None => Some(Shape::Line {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                },
                Tool::TangentArc => self
                    .document
                    .tangent_source
                    .and_then(|source| self.document.tangent_arc_shape(source, end, 0).ok()),
                Tool::Point => None,
                Tool::FitSpline | Tool::ControlSpline => None,
                Tool::Select | Tool::Dimension => None,
            };
            if let Some(shape) = draft {
                self.paint_shape(painter, &shape, origin, true);
            }
        }
        if matches!(self.document.tool, Tool::FitSpline | Tool::ControlSpline)
            && !self.spline_points.is_empty()
        {
            let mode = if self.document.tool == Tool::FitSpline {
                spline::SplineMode::Fit
            } else {
                spline::SplineMode::Control
            };
            if spline::beziers(mode, &self.spline_points, false).is_ok() {
                self.paint_shape(
                    painter,
                    &Shape::Spline {
                        id: 0,
                        mode,
                        points: self.spline_points.clone(),
                        closed: false,
                    },
                    origin,
                    true,
                );
            } else {
                for pair in self.spline_points.windows(2) {
                    let a = self.document.world_to_screen(pair[0], origin);
                    let b = self.document.world_to_screen(pair[1], origin);
                    painter.line(a.x, a.y, b.x, b.y, 1.5, ACCENT);
                }
            }
            if layout.viewport.contains(self.cursor.x, self.cursor.y) {
                let last = self
                    .document
                    .world_to_screen(*self.spline_points.last().unwrap(), origin);
                painter.line(last.x, last.y, self.cursor.x, self.cursor.y, 1.0, ACCENT);
            }
        }
        if self.document.tool != Tool::Select
            && layout.viewport.contains(self.cursor.x, self.cursor.y)
        {
            let raw = self.document.screen_to_world(self.cursor, origin);
            if let Some(snap) = self
                .document
                .snap_point(raw, self.snap_grid, self.snap_geometry)
            {
                let p = self.document.world_to_screen(snap.point, origin);
                let size = if snap.kind == SnapKind::Grid {
                    3.0
                } else {
                    5.0
                };
                painter.outline(
                    Rect {
                        x: p.x - size,
                        y: p.y - size,
                        w: size * 2.0,
                        h: size * 2.0,
                    },
                    1.2,
                    ACCENT,
                );
            }
        }
    }

    pub(super) fn paint_shape(
        &self,
        painter: &mut Painter,
        shape: &Shape,
        origin: Point,
        selected: bool,
    ) {
        let analysis = self.sketch_analysis();
        let id = shape.id();
        let color = if selected {
            ACCENT
        } else if analysis.conflicting.contains(&id) {
            CONFLICT
        } else if analysis.redundant.contains(&id) {
            REDUNDANT
        } else if self.document.construction.contains(&id) {
            MUTED
        } else if analysis.fully_constrained.contains(&id) {
            FULLY_CONSTRAINED
        } else {
            SHAPE
        };
        drop(analysis);
        match shape {
            Shape::Line { a, b, .. } => {
                let a = self.document.world_to_screen(*a, origin);
                let b = self.document.world_to_screen(*b, origin);
                painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                if selected {
                    for (index, endpoint) in [a, b].iter().enumerate() {
                        let fixed = self
                            .document
                            .fixed_anchors
                            .contains_key(&(shape.id(), index));
                        painter.circle(
                            endpoint.x,
                            endpoint.y,
                            if fixed { 5.0 } else { 3.5 },
                            1.5,
                            if fixed { ACCENT } else { MUTED },
                        );
                    }
                }
            }
            Shape::Rectangle { a, b, .. } => {
                let a = self.document.world_to_screen(*a, origin);
                let b = self.document.world_to_screen(*b, origin);
                painter.outline(
                    Rect {
                        x: a.x.min(b.x),
                        y: a.y.min(b.y),
                        w: (a.x - b.x).abs(),
                        h: (a.y - b.y).abs(),
                    },
                    if selected { 2.5 } else { 1.8 },
                    color,
                );
            }
            Shape::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => {
                let corners = Shape::rotated_rectangle_corners(*center, *width, *height, *rotation);
                for index in 0..4 {
                    let a = self.document.world_to_screen(corners[index], origin);
                    let b = self
                        .document
                        .world_to_screen(corners[(index + 1) % 4], origin);
                    painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                }
            }
            Shape::Circle { center, radius, .. } => {
                let center = self.document.world_to_screen(*center, origin);
                painter.circle(
                    center.x,
                    center.y,
                    radius * self.document.zoom,
                    if selected { 2.5 } else { 1.8 },
                    color,
                );
            }
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                segments,
                ..
            } => {
                let mut previous = self.document.world_to_screen(
                    Shape::ellipse_point(*center, *rx, *ry, *rotation, 0.0),
                    origin,
                );
                for step in 1..=*segments {
                    let next = self.document.world_to_screen(
                        Shape::ellipse_point(
                            *center,
                            *rx,
                            *ry,
                            *rotation,
                            step as f32 / *segments as f32,
                        ),
                        origin,
                    );
                    painter.line(
                        previous.x,
                        previous.y,
                        next.x,
                        next.y,
                        if selected { 2.5 } else { 1.8 },
                        color,
                    );
                    previous = next;
                }
            }
            Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                segments,
                ..
            } => {
                let count = ((sweep.abs() / std::f64::consts::TAU) * *segments as f64)
                    .ceil()
                    .max(2.0) as usize;
                let mut previous = self.document.world_to_screen(
                    Shape::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        0.0,
                    ),
                    origin,
                );
                for index in 1..=count {
                    let next = self.document.world_to_screen(
                        Shape::elliptic_arc_point(
                            *center,
                            *rx,
                            *ry,
                            *rotation,
                            *start_angle,
                            *sweep,
                            index as f64 / count as f64,
                        ),
                        origin,
                    );
                    painter.line(
                        previous.x,
                        previous.y,
                        next.x,
                        next.y,
                        if selected { 2.5 } else { 1.8 },
                        color,
                    );
                    previous = next;
                }
            }
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                if let Ok(sampled) = spline::beziers(*mode, points, *closed)
                    .and_then(|curves| spline::samples(&curves, 0.01))
                {
                    for pair in sampled.windows(2) {
                        let a = self.document.world_to_screen(pair[0], origin);
                        let b = self.document.world_to_screen(pair[1], origin);
                        painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                    }
                }
                if selected {
                    if *mode == spline::SplineMode::Control {
                        for pair in points.windows(2) {
                            let a = self.document.world_to_screen(pair[0], origin);
                            let b = self.document.world_to_screen(pair[1], origin);
                            let steps = (a.distance(b) / 12.0).ceil().max(1.0) as usize;
                            for step in (0..steps).step_by(2) {
                                let along = |fraction: f32| Point {
                                    x: a.x + (b.x - a.x) * fraction,
                                    y: a.y + (b.y - a.y) * fraction,
                                };
                                let from = along(step as f32 / steps as f32);
                                let to = along(((step + 1).min(steps)) as f32 / steps as f32);
                                painter.line(from.x, from.y, to.x, to.y, 1.0, MUTED);
                            }
                        }
                    }
                    for (index, point) in points.iter().enumerate() {
                        let at = self.document.world_to_screen(*point, origin);
                        painter.circle(
                            at.x,
                            at.y,
                            if self.document.selected_spline_point == Some(index) {
                                5.0
                            } else {
                                3.5
                            },
                            1.5,
                            color,
                        );
                    }
                }
            }
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                segments,
                ..
            } => {
                let count = ((sweep.abs() / std::f64::consts::TAU) * *segments as f64)
                    .ceil()
                    .max(2.0) as usize;
                let mut previous = self.document.world_to_screen(
                    Shape::arc_point(*center, *radius, *start_angle, *sweep, 0.0),
                    origin,
                );
                for index in 1..=count {
                    let next = self.document.world_to_screen(
                        Shape::arc_point(
                            *center,
                            *radius,
                            *start_angle,
                            *sweep,
                            index as f64 / count as f64,
                        ),
                        origin,
                    );
                    painter.line(
                        previous.x,
                        previous.y,
                        next.x,
                        next.y,
                        if selected { 2.5 } else { 1.8 },
                        color,
                    );
                    previous = next;
                }
            }
            Shape::Point { at, .. } => {
                let center = self.document.world_to_screen(*at, origin);
                painter.line(
                    center.x - 5.0,
                    center.y,
                    center.x + 5.0,
                    center.y,
                    2.0,
                    color,
                );
                painter.line(
                    center.x,
                    center.y - 5.0,
                    center.x,
                    center.y + 5.0,
                    2.0,
                    color,
                );
            }
            Shape::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            } => {
                for index in 0..*sides {
                    let a = self.document.world_to_screen(
                        Shape::polygon_vertex(*center, *radius, *sides, *rotation, index),
                        origin,
                    );
                    let b = self.document.world_to_screen(
                        Shape::polygon_vertex(
                            *center,
                            *radius,
                            *sides,
                            *rotation,
                            (index + 1) % sides,
                        ),
                        origin,
                    );
                    painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                }
            }
            Shape::Capsule {
                from,
                to,
                width,
                arc_segments,
                ..
            } => {
                let outline = Shape::capsule_outline(*from, *to, *width, *arc_segments as usize);
                for index in 0..outline.len() {
                    let a = self.document.world_to_screen(outline[index], origin);
                    let b = self
                        .document
                        .world_to_screen(outline[(index + 1) % outline.len()], origin);
                    painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                }
            }
        }
        if selected
            && self.document.fixed_anchors.contains_key(&(shape.id(), 0))
            && matches!(
                shape,
                Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
                    | Shape::Polygon { .. }
                    | Shape::RotatedRectangle { .. }
            )
            && let Some(center) = self.document.fixed_anchor_position(shape.id(), 0)
        {
            let at = self.document.world_to_screen(center, origin);
            painter.circle(at.x, at.y, 5.0, 1.5, ACCENT);
        }
        if selected && matches!(shape, Shape::Arc { .. } | Shape::EllipticArc { .. }) {
            for index in [1, 2] {
                if self
                    .document
                    .fixed_anchors
                    .contains_key(&(shape.id(), index))
                    && let Some(point) = self.document.fixed_anchor_position(shape.id(), index)
                {
                    let at = self.document.world_to_screen(point, origin);
                    painter.circle(at.x, at.y, 5.0, 1.5, ACCENT);
                }
            }
        }
        if selected {
            let range = match shape {
                Shape::Ellipse { .. } | Shape::RotatedRectangle { .. } => Some(1..5),
                Shape::Polygon { sides, .. } => Some(1..*sides as usize + 1),
                Shape::Capsule { .. } => Some(0..2),
                _ => None,
            };
            if let Some(range) = range {
                for index in range {
                    if self
                        .document
                        .fixed_anchors
                        .contains_key(&(shape.id(), index))
                        && let Some(point) = self.document.fixed_anchor_position(shape.id(), index)
                    {
                        let at = self.document.world_to_screen(point, origin);
                        painter.circle(at.x, at.y, 5.0, 1.5, ACCENT);
                    }
                }
            }
        }
        if selected && matches!(shape, Shape::Rectangle { .. }) {
            for index in [0, 1] {
                if self
                    .document
                    .fixed_anchors
                    .contains_key(&(shape.id(), index))
                    && let Some(point) = self.document.fixed_anchor_position(shape.id(), index)
                {
                    let at = self.document.world_to_screen(point, origin);
                    painter.circle(at.x, at.y, 5.0, 1.5, ACCENT);
                }
            }
        }
    }

    /// Current constraint diagnostics, recomputed only when the document
    /// revision changed and no drag is in progress.
    pub(super) fn sketch_analysis(&self) -> std::cell::Ref<'_, SketchAnalysis> {
        {
            let mut cache = self.analysis.borrow_mut();
            let revision = self.document.revision();
            let stale = cache.as_ref().is_none_or(|(at, _)| *at != revision);
            if stale && (!self.document.dragging() || cache.is_none()) {
                *cache = Some((revision, self.document.analyze_constraints()));
            }
        }
        std::cell::Ref::map(self.analysis.borrow(), |cache| {
            &cache.as_ref().expect("analysis cached above").1
        })
    }

    /// Dimension labels with their dimension lines; the label being edited
    /// shows its input buffer in place.
    pub(super) fn paint_dimension_labels(&self, painter: &mut Painter, layout: Layout) {
        let origin = self.viewport_origin();
        let editing = self.editing.as_ref().and_then(|edit| match edit.field {
            EditField::Label(key) => Some((key, edit.buffer.as_str())),
            _ => None,
        });
        if let Some(start) = self
            .dimension_start
            .and_then(|anchor| self.document.dimension_anchor(anchor))
        {
            let at = self.document.world_to_screen(start, origin);
            painter.circle(at.x, at.y, 6.0, 2.0, ACCENT);
            if layout.viewport.contains(self.cursor.x, self.cursor.y) {
                painter.line(at.x, at.y, self.cursor.x, self.cursor.y, 1.0, ACCENT);
            }
        }
        for label in self.document.dimension_labels() {
            let dragged = self
                .label_drag
                .filter(|drag| drag.key == label.key && drag.moved)
                .map(|_| self.cursor);
            let at = dragged.unwrap_or_else(|| self.document.world_to_screen(label.at, origin));
            let selected = self.selected_dimension == Some(label.key);
            let color = if selected {
                ACCENT
            } else if self.document.is_reference(label.key) {
                MUTED
            } else {
                DIMENSION
            };
            for (p, q) in &label.lines {
                let p = self.document.world_to_screen(*p, origin);
                let q = self.document.world_to_screen(*q, origin);
                painter.line(p.x, p.y, q.x, q.y, 1.0, color);
            }
            let attach = self.document.world_to_screen(label.attach, origin);
            if dragged.is_some() || self.document.dims.label_positions.contains_key(&label.key) {
                painter.line(attach.x, attach.y, at.x, at.y, 1.0, color);
            }
            let text = match editing {
                Some((key, buffer)) if key == label.key => format!("{buffer}_"),
                _ => label.text.clone(),
            };
            let (w, h) = model::label_size(&text);
            let rect = Rect {
                x: at.x - w * 0.5,
                y: at.y - h * 0.5,
                w,
                h,
            };
            if !layout.viewport.contains(at.x, at.y) {
                continue;
            }
            let edited = editing.is_some_and(|(key, _)| key == label.key);
            painter.rect(rect, if edited { [0.13, 0.29, 0.37, 1.0] } else { BG });
            painter.outline(rect, 1.0, if edited || selected { ACCENT } else { BORDER });
            painter.text(
                rect.x + 4.0,
                rect.y + 4.0,
                &text,
                model::LABEL_SCALE,
                if edited { TEXT } else { color },
            );
        }
    }
}

impl App {
    /// Rubber band of a Select-mode box drag: solid for a left-to-right
    /// window (fully enclosed), green for a right-to-left crossing box.
    fn paint_box_select(&self, painter: &mut Painter, layout: Layout) {
        let Some(start) = self.box_start else {
            return;
        };
        let end = Point {
            x: self
                .cursor
                .x
                .clamp(layout.viewport.x, layout.viewport.x + layout.viewport.w),
            y: self
                .cursor
                .y
                .clamp(layout.viewport.y, layout.viewport.y + layout.viewport.h),
        };
        let rect = Rect {
            x: start.x.min(end.x),
            y: start.y.min(end.y),
            w: (end.x - start.x).abs(),
            h: (end.y - start.y).abs(),
        };
        let (fill, edge) = if end.x >= start.x {
            ([0.32, 0.78, 0.93, 0.10], ACCENT)
        } else {
            ([0.46, 0.84, 0.52, 0.10], FULLY_CONSTRAINED)
        };
        painter.rect(rect, fill);
        painter.outline(rect, 1.0, edge);
    }
}
