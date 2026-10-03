use super::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn distance(self, other: Self) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line {
        id: u32,
        a: Point,
        b: Point,
    },
    Rectangle {
        id: u32,
        a: Point,
        b: Point,
    },
    RotatedRectangle {
        id: u32,
        center: Point,
        width: f32,
        height: f32,
        rotation: f32,
    },
    Circle {
        id: u32,
        center: Point,
        radius: f32,
        segments: u16,
    },
    Ellipse {
        id: u32,
        center: Point,
        rx: f32,
        ry: f32,
        rotation: f32,
        segments: u16,
    },
    EllipticArc {
        id: u32,
        center: Point,
        rx: f32,
        ry: f32,
        rotation: f32,
        start_angle: f64,
        sweep: f64,
        segments: u16,
    },
    Spline {
        id: u32,
        mode: SplineMode,
        points: Vec<Point>,
        closed: bool,
    },
    Arc {
        id: u32,
        center: Point,
        radius: f32,
        start_angle: f64,
        sweep: f64,
        segments: u16,
    },
    Point {
        id: u32,
        at: Point,
    },
    Polygon {
        id: u32,
        center: Point,
        radius: f32,
        sides: u8,
        rotation: f32,
    },
    Capsule {
        id: u32,
        from: Point,
        to: Point,
        width: f32,
        arc_segments: u16,
    },
}

impl Shape {
    pub fn snap_points(&self) -> Vec<SnapResult> {
        let anchor = |point| SnapResult {
            point,
            kind: SnapKind::Anchor,
        };
        let midpoint = |point| SnapResult {
            point,
            kind: SnapKind::Midpoint,
        };
        match self {
            Self::Line { a, b, .. } => vec![
                anchor(*a),
                anchor(*b),
                midpoint(Point {
                    x: (a.x + b.x) * 0.5,
                    y: (a.y + b.y) * 0.5,
                }),
            ],
            Self::Rectangle { a, b, .. } => vec![
                anchor(*a),
                anchor(*b),
                anchor(Point { x: a.x, y: b.y }),
                anchor(Point { x: b.x, y: a.y }),
                anchor(Point {
                    x: (a.x + b.x) * 0.5,
                    y: (a.y + b.y) * 0.5,
                }),
            ],
            Self::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => {
                let mut result =
                    Self::rotated_rectangle_corners(*center, *width, *height, *rotation)
                        .into_iter()
                        .map(anchor)
                        .collect::<Vec<_>>();
                result.push(anchor(*center));
                result
            }
            Self::Circle { center, radius, .. } => vec![
                anchor(*center),
                anchor(Point {
                    x: center.x + radius,
                    y: center.y,
                }),
                anchor(Point {
                    x: center.x - radius,
                    y: center.y,
                }),
                anchor(Point {
                    x: center.x,
                    y: center.y + radius,
                }),
                anchor(Point {
                    x: center.x,
                    y: center.y - radius,
                }),
            ],
            Self::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            } => {
                let a = rotation.to_radians();
                vec![
                    anchor(*center),
                    anchor(Point {
                        x: center.x + rx * a.cos(),
                        y: center.y + rx * a.sin(),
                    }),
                    anchor(Point {
                        x: center.x - rx * a.cos(),
                        y: center.y - rx * a.sin(),
                    }),
                    anchor(Point {
                        x: center.x - ry * a.sin(),
                        y: center.y + ry * a.cos(),
                    }),
                    anchor(Point {
                        x: center.x + ry * a.sin(),
                        y: center.y - ry * a.cos(),
                    }),
                ]
            }
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => vec![
                anchor(*center),
                anchor(Self::arc_point(*center, *radius, *start_angle, *sweep, 0.0)),
                anchor(Self::arc_point(*center, *radius, *start_angle, *sweep, 1.0)),
                midpoint(Self::arc_point(*center, *radius, *start_angle, *sweep, 0.5)),
            ],
            Self::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                ..
            } => vec![
                anchor(*center),
                anchor(Self::elliptic_arc_point(
                    *center,
                    *rx,
                    *ry,
                    *rotation,
                    *start_angle,
                    *sweep,
                    0.0,
                )),
                anchor(Self::elliptic_arc_point(
                    *center,
                    *rx,
                    *ry,
                    *rotation,
                    *start_angle,
                    *sweep,
                    1.0,
                )),
                midpoint(Self::elliptic_arc_point(
                    *center,
                    *rx,
                    *ry,
                    *rotation,
                    *start_angle,
                    *sweep,
                    0.5,
                )),
            ],
            Self::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                let mut result = points.iter().copied().map(anchor).collect::<Vec<_>>();
                if let Ok(curves) = spline::beziers(*mode, points, *closed) {
                    result.extend(
                        curves
                            .iter()
                            .map(|curve| midpoint(spline::point(curve, 0.5))),
                    );
                }
                result
            }
            Self::Point { at, .. } => vec![anchor(*at)],
            Self::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            } => {
                let mut result = vec![anchor(*center)];
                result.extend(
                    (0..*sides).map(|i| {
                        anchor(Self::polygon_vertex(*center, *radius, *sides, *rotation, i))
                    }),
                );
                result
            }
            Self::Capsule { from, to, .. } => vec![
                anchor(*from),
                anchor(*to),
                midpoint(Point {
                    x: (from.x + to.x) * 0.5,
                    y: (from.y + to.y) * 0.5,
                }),
            ],
        }
    }

    pub fn bounds(&self) -> Option<(Point, Point)> {
        let points: Vec<Point> = match self {
            Self::Line { a, b, .. } | Self::Rectangle { a, b, .. } => vec![*a, *b],
            Self::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => Self::rotated_rectangle_corners(*center, *width, *height, *rotation).to_vec(),
            Self::Circle { center, radius, .. } => vec![
                Point {
                    x: center.x - radius,
                    y: center.y - radius,
                },
                Point {
                    x: center.x + radius,
                    y: center.y + radius,
                },
            ],
            Self::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            } => {
                let angle = rotation.to_radians();
                let x = ((rx * angle.cos()).powi(2) + (ry * angle.sin()).powi(2)).sqrt();
                let y = ((rx * angle.sin()).powi(2) + (ry * angle.cos()).powi(2)).sqrt();
                vec![
                    Point {
                        x: center.x - x,
                        y: center.y - y,
                    },
                    Point {
                        x: center.x + x,
                        y: center.y + y,
                    },
                ]
            }
            Self::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => {
                let mut points = vec![
                    Self::arc_point(*center, *radius, *start_angle, *sweep, 0.0),
                    Self::arc_point(*center, *radius, *start_angle, *sweep, 1.0),
                ];
                for quarter in 0..4 {
                    let angle = quarter as f64 * std::f64::consts::FRAC_PI_2;
                    let along = if *sweep >= 0.0 {
                        (angle - start_angle).rem_euclid(std::f64::consts::TAU) / sweep
                    } else {
                        -((start_angle - angle).rem_euclid(std::f64::consts::TAU)) / sweep
                    };
                    if (0.0..=1.0).contains(&along) {
                        points.push(Self::arc_point(
                            *center,
                            *radius,
                            *start_angle,
                            *sweep,
                            along,
                        ));
                    }
                }
                points
            }
            Self::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                ..
            } => {
                let mut points = vec![
                    Self::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        0.0,
                    ),
                    Self::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        1.0,
                    ),
                ];
                let angle = rotation.to_radians() as f64;
                let x_extreme = (-(*ry as f64) * angle.sin()).atan2(*rx as f64 * angle.cos());
                let y_extreme = (*ry as f64 * angle.cos()).atan2(*rx as f64 * angle.sin());
                for candidate in [
                    x_extreme,
                    x_extreme + std::f64::consts::PI,
                    y_extreme,
                    y_extreme + std::f64::consts::PI,
                ] {
                    let progress = Self::elliptic_arc_progress(candidate, *start_angle, *sweep);
                    if (0.0..=1.0).contains(&progress) {
                        points.push(Self::elliptic_arc_point(
                            *center,
                            *rx,
                            *ry,
                            *rotation,
                            *start_angle,
                            *sweep,
                            progress,
                        ));
                    }
                }
                points
            }
            Self::Spline {
                mode,
                points,
                closed,
                ..
            } => spline::beziers(*mode, points, *closed)
                .ok()
                .and_then(|curves| spline::samples(&curves, 0.01).ok())?,
            Self::Point { at, .. } => vec![*at],
            Self::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            } => (0..*sides)
                .map(|index| Self::polygon_vertex(*center, *radius, *sides, *rotation, index))
                .collect(),
            Self::Capsule {
                from,
                to,
                width,
                arc_segments,
                ..
            } => Self::capsule_outline(*from, *to, *width, *arc_segments as usize),
        };
        if points.is_empty() || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return None;
        }
        let mut min = points[0];
        let mut max = points[0];
        for point in &points[1..] {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
        }
        Some((min, max))
    }

    pub fn rectangle_from_three_points(
        id: u32,
        first: Point,
        second: Point,
        side: Point,
    ) -> Option<Self> {
        let dx = second.x - first.x;
        let dy = second.y - first.y;
        let width = dx.hypot(dy);
        let signed_height = (dx * (side.y - first.y) - dy * (side.x - first.x)) / width;
        if ![
            first.x,
            first.y,
            second.x,
            second.y,
            side.x,
            side.y,
            width,
            signed_height,
        ]
        .into_iter()
        .all(f32::is_finite)
            || width <= 1e-6
            || signed_height.abs() <= 1e-6
        {
            return None;
        }
        Some(Self::RotatedRectangle {
            id,
            center: Point {
                x: (first.x + second.x) * 0.5 - dy / width * signed_height * 0.5,
                y: (first.y + second.y) * 0.5 + dx / width * signed_height * 0.5,
            },
            width,
            height: signed_height.abs(),
            rotation: dy.atan2(dx).to_degrees() + if signed_height < 0.0 { 180.0 } else { 0.0 },
        })
    }

    pub fn rectangle_from_center_side(
        id: u32,
        center: Point,
        midpoint: Point,
        adjacent: Point,
    ) -> Option<Self> {
        let dx = midpoint.x - center.x;
        let dy = midpoint.y - center.y;
        let half_width = dx.hypot(dy);
        let half_height =
            ((dx * (adjacent.y - center.y) - dy * (adjacent.x - center.x)) / half_width).abs();
        if ![
            center.x,
            center.y,
            midpoint.x,
            midpoint.y,
            adjacent.x,
            adjacent.y,
            half_width,
            half_height,
        ]
        .into_iter()
        .all(f32::is_finite)
            || half_width <= 1e-6
            || half_height <= 1e-6
        {
            return None;
        }
        Some(Self::RotatedRectangle {
            id,
            center,
            width: half_width * 2.0,
            height: half_height * 2.0,
            rotation: dy.atan2(dx).to_degrees(),
        })
    }

    pub fn rotated_rectangle_corners(
        center: Point,
        width: f32,
        height: f32,
        rotation: f32,
    ) -> [Point; 4] {
        let angle = rotation.to_radians();
        let (s, c) = angle.sin_cos();
        [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(sx, sy)| {
            let x = sx * width * 0.5;
            let y = sy * height * 0.5;
            Point {
                x: center.x + c * x - s * y,
                y: center.y + s * x + c * y,
            }
        })
    }

    pub fn center(&self) -> Point {
        match self {
            Self::Line { a, b, .. } | Self::Rectangle { a, b, .. } => Point {
                x: (a.x + b.x) * 0.5,
                y: (a.y + b.y) * 0.5,
            },
            Self::RotatedRectangle { center, .. }
            | Self::Circle { center, .. }
            | Self::Ellipse { center, .. }
            | Self::EllipticArc { center, .. }
            | Self::Arc { center, .. }
            | Self::Polygon { center, .. } => *center,
            Self::Point { at, .. } => *at,
            Self::Spline { points, .. } => {
                let sum = points.iter().fold(Point { x: 0.0, y: 0.0 }, |a, p| Point {
                    x: a.x + p.x,
                    y: a.y + p.y,
                });
                Point {
                    x: sum.x / points.len() as f32,
                    y: sum.y / points.len() as f32,
                }
            }
            Self::Capsule { from, to, .. } => Point {
                x: (from.x + to.x) * 0.5,
                y: (from.y + to.y) * 0.5,
            },
        }
    }

    pub fn scaled_about(&self, factor: f32, center: Point) -> Option<Self> {
        if !factor.is_finite() || factor <= 0.0 || !center.x.is_finite() || !center.y.is_finite() {
            return None;
        }
        let scale_point = |point: &mut Point| {
            point.x = center.x + (point.x - center.x) * factor;
            point.y = center.y + (point.y - center.y) * factor;
        };
        let mut next = self.clone();
        match &mut next {
            Self::Line { a, b, .. } | Self::Rectangle { a, b, .. } => {
                scale_point(a);
                scale_point(b);
            }
            Self::RotatedRectangle {
                center,
                width,
                height,
                ..
            } => {
                scale_point(center);
                *width *= factor;
                *height *= factor;
            }
            Self::Circle { center, radius, .. }
            | Self::Arc { center, radius, .. }
            | Self::Polygon { center, radius, .. } => {
                scale_point(center);
                *radius *= factor;
            }
            Self::Ellipse { center, rx, ry, .. } | Self::EllipticArc { center, rx, ry, .. } => {
                scale_point(center);
                *rx *= factor;
                *ry *= factor;
            }
            Self::Point { at, .. } => scale_point(at),
            Self::Spline { points, .. } => points.iter_mut().for_each(scale_point),
            Self::Capsule {
                from, to, width, ..
            } => {
                scale_point(from);
                scale_point(to);
                *width *= factor;
            }
        }
        let valid = match &next {
            Self::Line { a, b, .. } | Self::Rectangle { a, b, .. } => {
                [a.x, a.y, b.x, b.y].iter().all(|value| value.is_finite())
            }
            Self::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => [center.x, center.y, *width, *height, *rotation]
                .iter()
                .all(|value| value.is_finite()),
            Self::Circle { center, radius, .. }
            | Self::Arc { center, radius, .. }
            | Self::Polygon { center, radius, .. } => [center.x, center.y, *radius]
                .iter()
                .all(|value| value.is_finite()),
            Self::Ellipse { center, rx, ry, .. } | Self::EllipticArc { center, rx, ry, .. } => {
                [center.x, center.y, *rx, *ry]
                    .iter()
                    .all(|value| value.is_finite())
            }
            Self::Point { at, .. } => [at.x, at.y].iter().all(|value| value.is_finite()),
            Self::Spline {
                mode,
                points,
                closed,
                ..
            } => spline::beziers(*mode, points, *closed).is_ok(),
            Self::Capsule {
                from, to, width, ..
            } => [from.x, from.y, to.x, to.y, *width]
                .iter()
                .all(|value| value.is_finite()),
        };
        let nondegenerate = match &next {
            Self::Line { a, b, .. } => a.distance(*b) > 0.0,
            Self::Rectangle { a, b, .. } => a.x != b.x && a.y != b.y,
            Self::RotatedRectangle { width, height, .. } => *width > 0.0 && *height > 0.0,
            Self::Circle { radius, .. }
            | Self::Arc { radius, .. }
            | Self::Polygon { radius, .. } => *radius > 0.0,
            Self::Ellipse { rx, ry, .. } | Self::EllipticArc { rx, ry, .. } => {
                *rx > 0.0 && *ry > 0.0
            }
            Self::Capsule {
                from, to, width, ..
            } => from.distance(*to) > 0.0 && *width > 0.0,
            Self::Point { .. } => true,
            Self::Spline { .. } => true,
        };
        (valid && nondegenerate).then_some(next)
    }

    pub fn rotated_about(&self, degrees: f32, center: Point) -> Option<Self> {
        if !degrees.is_finite() || !center.x.is_finite() || !center.y.is_finite() {
            return None;
        }
        let radians = degrees.to_radians();
        if !radians.is_finite() {
            return None;
        }
        let (s, c) = radians.sin_cos();
        let rotate_point = |point: &mut Point| {
            let x = point.x - center.x;
            let y = point.y - center.y;
            point.x = center.x + c * x - s * y;
            point.y = center.y + s * x + c * y;
        };
        let mut next = self.clone();
        match &mut next {
            Self::Line { a, b, .. } => {
                rotate_point(a);
                rotate_point(b);
            }
            Self::Rectangle { id, a, b } => {
                let mut shape_center = Point {
                    x: (a.x + b.x) * 0.5,
                    y: (a.y + b.y) * 0.5,
                };
                rotate_point(&mut shape_center);
                next = Self::RotatedRectangle {
                    id: *id,
                    center: shape_center,
                    width: (b.x - a.x).abs(),
                    height: (b.y - a.y).abs(),
                    rotation: degrees,
                };
            }
            Self::RotatedRectangle {
                center: shape_center,
                rotation,
                ..
            }
            | Self::Ellipse {
                center: shape_center,
                rotation,
                ..
            }
            | Self::EllipticArc {
                center: shape_center,
                rotation,
                ..
            }
            | Self::Polygon {
                center: shape_center,
                rotation,
                ..
            } => {
                rotate_point(shape_center);
                *rotation += degrees;
            }
            Self::Circle {
                center: shape_center,
                ..
            } => rotate_point(shape_center),
            Self::Arc {
                center: shape_center,
                start_angle,
                ..
            } => {
                rotate_point(shape_center);
                *start_angle += radians as f64;
            }
            Self::Point { at, .. } => rotate_point(at),
            Self::Spline { points, .. } => points.iter_mut().for_each(rotate_point),
            Self::Capsule { from, to, .. } => {
                rotate_point(from);
                rotate_point(to);
            }
        }
        next.scaled_about(1.0, next.center())
    }

    pub fn with_id(mut self, new_id: u32) -> Self {
        match &mut self {
            Self::Line { id, .. }
            | Self::Rectangle { id, .. }
            | Self::RotatedRectangle { id, .. }
            | Self::Circle { id, .. }
            | Self::Ellipse { id, .. }
            | Self::EllipticArc { id, .. }
            | Self::Spline { id, .. }
            | Self::Arc { id, .. }
            | Self::Point { id, .. }
            | Self::Polygon { id, .. }
            | Self::Capsule { id, .. } => *id = new_id,
        }
        self
    }

    pub fn translated(&self, delta: Point) -> Self {
        let mut next = self.clone();
        let shift = |point: &mut Point| {
            point.x += delta.x;
            point.y += delta.y;
        };
        match &mut next {
            Self::Line { a, b, .. } | Self::Rectangle { a, b, .. } => {
                shift(a);
                shift(b);
            }
            Self::RotatedRectangle { center, .. }
            | Self::Circle { center, .. }
            | Self::Ellipse { center, .. }
            | Self::EllipticArc { center, .. }
            | Self::Arc { center, .. }
            | Self::Polygon { center, .. } => shift(center),
            Self::Point { at, .. } => shift(at),
            Self::Spline { points, .. } => points.iter_mut().for_each(shift),
            Self::Capsule { from, to, .. } => {
                shift(from);
                shift(to);
            }
        }
        next
    }

    pub fn id(&self) -> u32 {
        match self {
            Self::Line { id, .. }
            | Self::Rectangle { id, .. }
            | Self::RotatedRectangle { id, .. }
            | Self::Circle { id, .. }
            | Self::Ellipse { id, .. }
            | Self::EllipticArc { id, .. }
            | Self::Spline { id, .. }
            | Self::Arc { id, .. }
            | Self::Point { id, .. }
            | Self::Polygon { id, .. }
            | Self::Capsule { id, .. } => *id,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Line { .. } => "Line",
            Self::Rectangle { .. } => "Rectangle",
            Self::RotatedRectangle { .. } => "Rectangle",
            Self::Circle { .. } => "Circle",
            Self::Ellipse { .. } => "Ellipse",
            Self::EllipticArc { .. } => "Elliptic arc",
            Self::Spline { .. } => "Spline",
            Self::Arc { .. } => "Arc",
            Self::Point { .. } => "Point",
            Self::Polygon { .. } => "Polygon",
            Self::Capsule { .. } => "Slot",
        }
    }

    pub fn hit(&self, point: Point, tolerance: f32) -> bool {
        match self {
            Self::Line { a, b, .. } => {
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let length_squared = dx * dx + dy * dy;
                if length_squared < f32::EPSILON {
                    return point.distance(*a) <= tolerance;
                }
                let projection = (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared)
                    .clamp(0.0, 1.0);
                point.distance(Point {
                    x: a.x + projection * dx,
                    y: a.y + projection * dy,
                }) <= tolerance
            }
            Self::Rectangle { a, b, .. } => {
                let left = a.x.min(b.x);
                let right = a.x.max(b.x);
                let top = a.y.min(b.y);
                let bottom = a.y.max(b.y);
                point.x >= left - tolerance
                    && point.x <= right + tolerance
                    && point.y >= top - tolerance
                    && point.y <= bottom + tolerance
            }
            Self::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => {
                let angle = rotation.to_radians();
                let (s, c) = angle.sin_cos();
                let dx = point.x - center.x;
                let dy = point.y - center.y;
                let local_x = c * dx + s * dy;
                let local_y = -s * dx + c * dy;
                local_x.abs() <= width * 0.5 + tolerance
                    && local_y.abs() <= height * 0.5 + tolerance
            }
            Self::Circle { center, radius, .. } => point.distance(*center) <= radius + tolerance,
            Self::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            } => {
                let angle = rotation.to_radians();
                let dx = point.x - center.x;
                let dy = point.y - center.y;
                let x = (dx * angle.cos() + dy * angle.sin()) / (rx + tolerance);
                let y = (-dx * angle.sin() + dy * angle.cos()) / (ry + tolerance);
                x * x + y * y <= 1.0
            }
            Self::Arc {
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
                (0..count).any(|index| {
                    let a = Self::arc_point(
                        *center,
                        *radius,
                        *start_angle,
                        *sweep,
                        index as f64 / count as f64,
                    );
                    let b = Self::arc_point(
                        *center,
                        *radius,
                        *start_angle,
                        *sweep,
                        (index + 1) as f64 / count as f64,
                    );
                    segment_distance(point, a, b) <= tolerance
                })
            }
            Self::EllipticArc {
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
                (0..count).any(|index| {
                    let a = Self::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        index as f64 / count as f64,
                    );
                    let b = Self::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        (index + 1) as f64 / count as f64,
                    );
                    segment_distance(point, a, b) <= tolerance
                })
            }
            Self::Point { at, .. } => point.distance(*at) <= tolerance,
            Self::Spline {
                mode,
                points,
                closed,
                ..
            } => spline::beziers(*mode, points, *closed)
                .ok()
                .and_then(|curves| spline::samples(&curves, tolerance.max(0.01) * 0.25).ok())
                .is_some_and(|sampled| {
                    sampled
                        .windows(2)
                        .any(|pair| segment_distance(point, pair[0], pair[1]) <= tolerance)
                }),
            Self::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            } => {
                let vertices: Vec<_> = (0..*sides)
                    .map(|index| Self::polygon_vertex(*center, *radius, *sides, *rotation, index))
                    .collect();
                if vertices.iter().enumerate().any(|(index, a)| {
                    segment_distance(point, *a, vertices[(index + 1) % vertices.len()]) <= tolerance
                }) {
                    return true;
                }
                let mut inside = false;
                for (index, a) in vertices.iter().enumerate() {
                    let b = vertices[(index + 1) % vertices.len()];
                    if (a.y > point.y) != (b.y > point.y)
                        && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
                    {
                        inside = !inside;
                    }
                }
                inside
            }
            Self::Capsule {
                from, to, width, ..
            } => segment_distance(point, *from, *to) <= width * 0.5 + tolerance,
        }
    }

    pub fn arc_point(
        center: Point,
        radius: f32,
        start_angle: f64,
        sweep: f64,
        fraction: f64,
    ) -> Point {
        let angle = start_angle + sweep * fraction;
        Point {
            x: (center.x as f64 + radius as f64 * angle.cos()) as f32,
            y: (center.y as f64 + radius as f64 * angle.sin()) as f32,
        }
    }

    pub fn elliptic_arc_point(
        center: Point,
        rx: f32,
        ry: f32,
        rotation: f32,
        start_angle: f64,
        sweep: f64,
        fraction: f64,
    ) -> Point {
        let angle = start_angle + sweep * fraction;
        let (sin, cos) = (rotation as f64).to_radians().sin_cos();
        let (y, x) = angle.sin_cos();
        Point {
            x: (center.x as f64 + rx as f64 * x * cos - ry as f64 * y * sin) as f32,
            y: (center.y as f64 + rx as f64 * x * sin + ry as f64 * y * cos) as f32,
        }
    }

    pub fn elliptic_arc_progress(angle: f64, start_angle: f64, sweep: f64) -> f64 {
        if sweep > 0.0 {
            (angle - start_angle).rem_euclid(std::f64::consts::TAU) / sweep
        } else {
            (start_angle - angle).rem_euclid(std::f64::consts::TAU) / -sweep
        }
    }

    pub fn ellipse_parameter_angle(
        center: Point,
        rx: f32,
        ry: f32,
        rotation: f32,
        point: Point,
    ) -> f64 {
        let (sin, cos) = (rotation as f64).to_radians().sin_cos();
        let dx = (point.x - center.x) as f64;
        let dy = (point.y - center.y) as f64;
        let x = (dx * cos + dy * sin) / rx as f64;
        let y = (-dx * sin + dy * cos) / ry as f64;
        let angle = y.atan2(x).rem_euclid(std::f64::consts::TAU);
        if angle > std::f64::consts::TAU - 1e-7 {
            0.0
        } else {
            angle
        }
    }

    pub fn ellipse_point(center: Point, rx: f32, ry: f32, rotation: f32, fraction: f32) -> Point {
        let angle = fraction * std::f32::consts::TAU;
        let rotation = rotation.to_radians();
        let x = rx * angle.cos();
        let y = ry * angle.sin();
        Point {
            x: center.x + x * rotation.cos() - y * rotation.sin(),
            y: center.y + x * rotation.sin() + y * rotation.cos(),
        }
    }

    pub fn polygon_vertex(
        center: Point,
        radius: f32,
        sides: u8,
        rotation: f32,
        index: u8,
    ) -> Point {
        let angle = rotation.to_radians() + index as f32 * std::f32::consts::TAU / sides as f32;
        Point {
            x: center.x + radius * angle.cos(),
            y: center.y + radius * angle.sin(),
        }
    }

    pub fn capsule_outline(from: Point, to: Point, width: f32, arc_segments: usize) -> Vec<Point> {
        let axis_angle = (to.y - from.y).atan2(to.x - from.x);
        let radius = width * 0.5;
        let mut outline = Vec::with_capacity((arc_segments + 1) * 2);
        for index in 0..=arc_segments {
            let angle = axis_angle - std::f32::consts::FRAC_PI_2
                + index as f32 * std::f32::consts::PI / arc_segments as f32;
            outline.push(Point {
                x: to.x + radius * angle.cos(),
                y: to.y + radius * angle.sin(),
            });
        }
        for index in 0..=arc_segments {
            let angle = axis_angle
                + std::f32::consts::FRAC_PI_2
                + index as f32 * std::f32::consts::PI / arc_segments as f32;
            outline.push(Point {
                x: from.x + radius * angle.cos(),
                y: from.y + radius * angle.sin(),
            });
        }
        outline
    }

    pub fn capsule_from_three_points(id: u32, from: Point, to: Point, side: Point) -> Option<Self> {
        if ![from.x, from.y, to.x, to.y, side.x, side.y]
            .into_iter()
            .all(f32::is_finite)
        {
            return None;
        }
        let length = from.distance(to);
        if length <= 1e-6 {
            return None;
        }
        let width = 2.0
            * ((to.x - from.x) * (side.y - from.y) - (to.y - from.y) * (side.x - from.x)).abs()
            / length;
        (width.is_finite() && width > 1e-6).then_some(Self::Capsule {
            id,
            from,
            to,
            width,
            arc_segments: 48,
        })
    }

    pub fn ellipse_from_three_points(
        id: u32,
        center: Point,
        first_axis: Point,
        second_axis: Point,
    ) -> Option<Self> {
        if ![
            center.x,
            center.y,
            first_axis.x,
            first_axis.y,
            second_axis.x,
            second_axis.y,
        ]
        .into_iter()
        .all(f32::is_finite)
        {
            return None;
        }
        let dx = first_axis.x - center.x;
        let dy = first_axis.y - center.y;
        let rx = center.distance(first_axis);
        if rx <= 1e-6 {
            return None;
        }
        let ry = (((second_axis.x - center.x) * -dy + (second_axis.y - center.y) * dx) / rx).abs();
        let rotation = dy.atan2(dx).to_degrees();
        if !ry.is_finite() || ry <= 1e-6 || !rotation.is_finite() {
            return None;
        }
        Some(Self::Ellipse {
            id,
            center,
            rx,
            ry,
            rotation,
            segments: 128,
        })
    }

    pub fn arc_from_three_points(
        id: u32,
        start: Point,
        end: Point,
        through: Point,
    ) -> Option<Self> {
        if ![start.x, start.y, end.x, end.y, through.x, through.y]
            .into_iter()
            .all(f32::is_finite)
        {
            return None;
        }
        let bx = end.x - start.x;
        let by = end.y - start.y;
        let cx = through.x - start.x;
        let cy = through.y - start.y;
        let det = 2.0 * (bx * cy - by * cx);
        if det.abs() <= 1e-6 * (bx * bx + by * by).max(cx * cx + cy * cy) {
            return None;
        }
        let b2 = bx * bx + by * by;
        let c2 = cx * cx + cy * cy;
        let center = Point {
            x: start.x + (cy * b2 - by * c2) / det,
            y: start.y + (bx * c2 - cx * b2) / det,
        };
        let radius = start.distance(center);
        let angle = |point: Point| ((point.y - center.y) as f64).atan2((point.x - center.x) as f64);
        let start_angle = angle(start);
        let end_delta = (angle(end) - start_angle).rem_euclid(std::f64::consts::TAU);
        let mid_delta = (angle(through) - start_angle).rem_euclid(std::f64::consts::TAU);
        let sweep = if mid_delta < end_delta {
            end_delta
        } else {
            end_delta - std::f64::consts::TAU
        };
        if ![center.x, center.y, radius].into_iter().all(f32::is_finite)
            || ![start_angle, sweep].into_iter().all(f64::is_finite)
            || radius <= 0.0
            || sweep.abs() < 1e-6
            || sweep.abs() >= std::f64::consts::TAU
        {
            return None;
        }
        Some(Self::Arc {
            id,
            center,
            radius,
            start_angle,
            sweep,
            segments: 128,
        })
    }

    pub fn circle_from_three_points(
        id: u32,
        first: Point,
        second: Point,
        third: Point,
    ) -> Option<Self> {
        let Self::Arc { center, radius, .. } =
            Self::arc_from_three_points(id, first, second, third)?
        else {
            return None;
        };
        Some(Self::Circle {
            id,
            center,
            radius,
            segments: 64,
        })
    }

    pub fn arc_from_center_points(
        id: u32,
        center: Point,
        start: Point,
        end: Point,
        clockwise: bool,
    ) -> Option<Self> {
        if ![center.x, center.y, start.x, start.y, end.x, end.y]
            .into_iter()
            .all(f32::is_finite)
        {
            return None;
        }
        let radius = center.distance(start);
        if radius <= 1e-6 || center.distance(end) <= 1e-6 {
            return None;
        }
        let start_angle = ((start.y - center.y) as f64).atan2((start.x - center.x) as f64);
        let end_angle = ((end.y - center.y) as f64).atan2((end.x - center.x) as f64);
        let positive = (end_angle - start_angle).rem_euclid(std::f64::consts::TAU);
        let sweep = if clockwise {
            positive - std::f64::consts::TAU
        } else {
            positive
        };
        if sweep.abs() < 1e-6 || sweep.abs() >= std::f64::consts::TAU - 1e-6 {
            return None;
        }
        Some(Self::Arc {
            id,
            center,
            radius,
            start_angle,
            sweep,
            segments: 128,
        })
    }
}

pub(super) fn segment_distance(point: Point, a: Point, b: Point) -> f32 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length_squared = dx * dx + dy * dy;
    if length_squared < f32::EPSILON {
        return point.distance(a);
    }
    let t = (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared).clamp(0.0, 1.0);
    point.distance(Point {
        x: a.x + t * dx,
        y: a.y + t * dy,
    })
}
