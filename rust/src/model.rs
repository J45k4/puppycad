use crate::solver;
use crate::spline::{self, SplineMode};
use std::collections::{HashMap, HashSet};

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

fn segment_distance(point: Point, a: Point, b: Point) -> f32 {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Line,
    Rectangle,
    CenterRectangle,
    ThreePointRectangle,
    ThreePointCenterRectangle,
    Circle,
    ThreePointCircle,
    Ellipse,
    FitSpline,
    ControlSpline,
    Arc,
    CenterArc,
    Point,
    Polygon,
    Slot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    Anchor,
    Midpoint,
    Intersection,
    Origin,
    Grid,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SnapResult {
    pub point: Point,
    pub kind: SnapKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DimensionField {
    LineLength,
    RectangleWidth,
    RectangleHeight,
    CircleDiameter,
    CurveRadius,
    ArcSweep,
    Rotation,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineRelationKind {
    Parallel,
    Perpendicular,
    Collinear,
    Angle(f32),
}

impl LineRelationKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Parallel => "parallel",
            Self::Perpendicular => "perpendicular",
            Self::Collinear => "collinear",
            Self::Angle(_) => "angle",
        }
    }

    pub fn angle_value(self) -> Option<f32> {
        if let Self::Angle(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisConstraint {
    Horizontal,
    Vertical,
}

impl AxisConstraint {
    pub fn name(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

#[derive(Clone, Debug)]
struct DocumentState {
    shapes: Vec<Shape>,
    driving_dimensions: HashMap<(u32, DimensionField), f32>,
    construction: HashSet<u32>,
    axis_constraints: HashMap<u32, AxisConstraint>,
    point_links: HashMap<(u32, u8), u32>,
    equal_lines: HashMap<u32, u32>,
    equal_radii: HashMap<u32, u32>,
    radius_differences: HashMap<u32, (u32, f32)>,
    internal_tangents: HashMap<u32, u32>,
    external_tangents: HashMap<u32, u32>,
    midpoint_links: HashMap<u32, u32>,
    point_curve_links: HashMap<u32, u32>,
    normal_links: HashMap<u32, (u32, u8)>,
    endpoint_tangents: HashMap<u32, (u32, u8, i8, i8)>,
    equal_ellipses: HashMap<u32, u32>,
    shared_ellipses: HashMap<u32, u32>,
    concentric: HashMap<u32, u32>,
    line_relations: HashMap<u32, (u32, LineRelationKind)>,
    fixed_anchors: HashMap<(u32, usize), Point>,
}

#[derive(Clone, Debug)]
struct DragState {
    id: u32,
    start: Point,
    original: Shape,
    before: DocumentState,
    spline_point: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub shapes: Vec<Shape>,
    pub selected: Option<u32>,
    pub selected_spline_point: Option<usize>,
    pub tool: Tool,
    pub zoom: f32,
    pub pan: Point,
    pub draft_start: Option<Point>,
    pub draft_end: Option<Point>,
    pub arc_clockwise: bool,
    pub driving_dimensions: HashMap<(u32, DimensionField), f32>,
    pub construction: HashSet<u32>,
    pub axis_constraints: HashMap<u32, AxisConstraint>,
    pub point_links: HashMap<(u32, u8), u32>,
    pub equal_lines: HashMap<u32, u32>,
    pub equal_radii: HashMap<u32, u32>,
    pub radius_differences: HashMap<u32, (u32, f32)>,
    pub internal_tangents: HashMap<u32, u32>,
    pub external_tangents: HashMap<u32, u32>,
    pub midpoint_links: HashMap<u32, u32>,
    pub point_curve_links: HashMap<u32, u32>,
    pub normal_links: HashMap<u32, (u32, u8)>,
    pub endpoint_tangents: HashMap<u32, (u32, u8, i8, i8)>,
    pub equal_ellipses: HashMap<u32, u32>,
    pub shared_ellipses: HashMap<u32, u32>,
    pub concentric: HashMap<u32, u32>,
    pub line_relations: HashMap<u32, (u32, LineRelationKind)>,
    pub fixed_anchors: HashMap<(u32, usize), Point>,
    next_id: u32,
    history: Vec<DocumentState>,
    future: Vec<DocumentState>,
    drag: Option<DragState>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            shapes: Vec::new(),
            selected: None,
            selected_spline_point: None,
            tool: Tool::Select,
            zoom: 1.0,
            pan: Point { x: 0.0, y: 0.0 },
            draft_start: None,
            draft_end: None,
            arc_clockwise: false,
            driving_dimensions: HashMap::new(),
            construction: HashSet::new(),
            axis_constraints: HashMap::new(),
            point_links: HashMap::new(),
            equal_lines: HashMap::new(),
            equal_radii: HashMap::new(),
            radius_differences: HashMap::new(),
            internal_tangents: HashMap::new(),
            external_tangents: HashMap::new(),
            midpoint_links: HashMap::new(),
            point_curve_links: HashMap::new(),
            normal_links: HashMap::new(),
            endpoint_tangents: HashMap::new(),
            equal_ellipses: HashMap::new(),
            shared_ellipses: HashMap::new(),
            concentric: HashMap::new(),
            line_relations: HashMap::new(),
            fixed_anchors: HashMap::new(),
            next_id: 1,
            history: Vec::new(),
            future: Vec::new(),
            drag: None,
        }
    }
}

impl Document {
    pub fn fixed_anchor_position(&self, id: u32, index: usize) -> Option<Point> {
        self.shapes
            .iter()
            .find(|shape| shape.id() == id)
            .and_then(|shape| match shape {
                Shape::Point { at, .. } if index == 0 => Some(*at),
                Shape::Line { a, b, .. } | Shape::Rectangle { a, b, .. } => match index {
                    0 => Some(*a),
                    1 => Some(*b),
                    _ => None,
                },
                Shape::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep,
                    ..
                } => match index {
                    0 => Some(*center),
                    1 => Some(Shape::arc_point(
                        *center,
                        *radius,
                        *start_angle,
                        *sweep,
                        0.0,
                    )),
                    2 => Some(Shape::arc_point(
                        *center,
                        *radius,
                        *start_angle,
                        *sweep,
                        1.0,
                    )),
                    _ => None,
                },
                Shape::EllipticArc {
                    center,
                    rx,
                    ry,
                    rotation,
                    start_angle,
                    sweep,
                    ..
                } => match index {
                    0 => Some(*center),
                    1 => Some(Shape::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        0.0,
                    )),
                    2 => Some(Shape::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        1.0,
                    )),
                    _ => None,
                },
                Shape::Circle { center, .. } if index == 0 => Some(*center),
                Shape::Ellipse {
                    center,
                    rx,
                    ry,
                    rotation,
                    ..
                } => match index {
                    0 => Some(*center),
                    1..=4 => Some(Shape::ellipse_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        (index - 1) as f32 / 4.0,
                    )),
                    _ => None,
                },
                Shape::RotatedRectangle {
                    center,
                    width,
                    height,
                    rotation,
                    ..
                } => match index {
                    0 => Some(*center),
                    1..=4 => Some(
                        Shape::rotated_rectangle_corners(*center, *width, *height, *rotation)
                            [(index + 1) % 4],
                    ),
                    _ => None,
                },
                Shape::Polygon {
                    center,
                    radius,
                    sides,
                    rotation,
                    ..
                } => match index {
                    0 => Some(*center),
                    n if n <= *sides as usize => Some(Shape::polygon_vertex(
                        *center,
                        *radius,
                        *sides,
                        *rotation,
                        (n - 1) as u8,
                    )),
                    _ => None,
                },
                Shape::Capsule { from, to, .. } => match index {
                    0 => Some(*from),
                    1 => Some(*to),
                    _ => None,
                },
                Shape::Spline { points, .. } => points.get(index).copied(),
                _ => None,
            })
    }

    pub fn toggle_fixed_line_endpoint_at(&mut self, cursor: Point) -> Result<bool, String> {
        let id = self
            .selected
            .ok_or("Select a line endpoint or rectangle corner to fix")?;
        let (a, b) = match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Line { a, b, .. }) | Some(Shape::Rectangle { a, b, .. }) => (*a, *b),
            _ => return Err("Select a line endpoint or rectangle corner to fix".into()),
        };
        if self.normal_links.contains_key(&id) {
            return Err("Line endpoint is driven by Normal".into());
        }
        let (index, position) = if cursor.distance(a) <= cursor.distance(b) {
            (0, a)
        } else {
            (1, b)
        };
        if cursor.distance(position) > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto a line endpoint".into());
        }
        if (index == 1
            && (self.equal_lines.contains_key(&id) || self.line_relations.contains_key(&id)))
            || self
                .line_relations
                .get(&id)
                .is_some_and(|(_, kind)| *kind == LineRelationKind::Collinear)
        {
            return Err("Line end is driven by a relation".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        Ok(if self.fixed_anchors.remove(&(id, index)).is_some() {
            false
        } else {
            self.fixed_anchors.insert((id, index), position);
            true
        })
    }

    pub fn toggle_fixed_arc_anchor_at(&mut self, cursor: Point) -> Result<(usize, bool), String> {
        let id = self.selected.ok_or("Select an arc")?;
        let shape = self
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .ok_or("Selected arc is missing")?;
        if !matches!(shape, Shape::Arc { .. } | Shape::EllipticArc { .. }) {
            return Err("Select an arc".into());
        }
        let tolerance = 8.0 / self.zoom.max(0.2);
        let index = [1, 2]
            .into_iter()
            .filter_map(|index| {
                self.fixed_anchor_position(id, index)
                    .map(|at| (index, at.distance(cursor)))
            })
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(index, _)| index)
            .unwrap_or(0);
        let position = self
            .fixed_anchor_position(id, index)
            .ok_or("Arc anchor is missing")?;
        if index > 0
            && (self.equal_radii.contains_key(&id) || self.equal_ellipses.contains_key(&id))
            && !self.fixed_anchors.contains_key(&(id, index))
        {
            return Err("Arc endpoint is driven by Equal".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let enabled = if self.fixed_anchors.remove(&(id, index)).is_some() {
            false
        } else {
            self.fixed_anchors.insert((id, index), position);
            true
        };
        Ok((index, enabled))
    }

    pub fn toggle_fixed_shape_anchor_at(&mut self, cursor: Point) -> Result<(usize, bool), String> {
        let id = self
            .selected
            .ok_or("Select an ellipse, polygon, rectangle, or slot")?;
        let shape = self
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .ok_or("Selected shape is missing")?;
        let (first, count, center_fallback) = match shape {
            Shape::Ellipse { .. } | Shape::RotatedRectangle { .. } => (1, 4, true),
            Shape::Polygon { sides, .. } => (1, *sides as usize, true),
            Shape::Capsule { .. } => (0, 2, false),
            _ => return Err("Select an ellipse, polygon, rectangle, or slot".into()),
        };
        let tolerance = 8.0 / self.zoom.max(0.2);
        let nearest = (first..first + count)
            .filter_map(|index| {
                self.fixed_anchor_position(id, index)
                    .map(|at| (index, at.distance(cursor)))
            })
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|left, right| left.1.total_cmp(&right.1));
        let index = nearest
            .map(|(index, _)| index)
            .or_else(|| center_fallback.then_some(0))
            .ok_or("Move the pointer onto a shape anchor")?;
        let position = self
            .fixed_anchor_position(id, index)
            .ok_or("Shape anchor is missing")?;
        if index > 0
            && self.equal_ellipses.contains_key(&id)
            && !self.fixed_anchors.contains_key(&(id, index))
        {
            return Err("Ellipse axis is driven by Equal".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let enabled = if self.fixed_anchors.remove(&(id, index)).is_some() {
            false
        } else {
            self.fixed_anchors.insert((id, index), position);
            true
        };
        Ok((index, enabled))
    }

    pub fn toggle_fixed_anchor(&mut self) -> Result<bool, String> {
        let id = self
            .selected
            .ok_or("Select a point or spline handle to fix")?;
        if self.midpoint_links.contains_key(&id) || self.point_curve_links.contains_key(&id) {
            return Err("Remove the point's curve relation before fixing it".into());
        }
        let index = match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Point { .. })
            | Some(Shape::Circle { .. })
            | Some(Shape::Arc { .. })
            | Some(Shape::Ellipse { .. })
            | Some(Shape::EllipticArc { .. })
            | Some(Shape::Polygon { .. })
            | Some(Shape::RotatedRectangle { .. }) => 0,
            Some(Shape::Spline { .. }) => self
                .selected_spline_point
                .ok_or("Select a spline handle to fix")?,
            _ => return Err("Select a point or spline handle to fix".into()),
        };
        let position = self
            .fixed_anchor_position(id, index)
            .ok_or("Anchor is missing")?;
        self.history.push(self.snapshot());
        self.future.clear();
        let enabled = if self.fixed_anchors.remove(&(id, index)).is_some() {
            false
        } else {
            self.fixed_anchors.insert((id, index), position);
            true
        };
        Ok(enabled)
    }

    fn has_fixed_anchor(&self, id: u32) -> bool {
        self.fixed_anchors
            .keys()
            .any(|(shape_id, _)| *shape_id == id)
    }

    pub fn snap_point(&self, raw: Point, grid: bool, geometry: bool) -> Option<SnapResult> {
        if !raw.x.is_finite() || !raw.y.is_finite() {
            return None;
        }
        let mut best = None;
        let mut distance = 8.0 / self.zoom.max(0.2);
        if geometry {
            for candidate in self.shapes.iter().flat_map(Shape::snap_points) {
                let d = raw.distance(candidate.point);
                if d < distance {
                    best = Some(candidate);
                    distance = d;
                }
            }
            for (index, shape) in self.shapes.iter().enumerate() {
                for other in self.shapes.iter().skip(index + 1) {
                    for point in finite_intersections(shape, other) {
                        let d = raw.distance(point);
                        if d <= distance {
                            best = Some(SnapResult {
                                point,
                                kind: SnapKind::Intersection,
                            });
                            distance = d;
                        }
                    }
                }
            }
            let origin = Point { x: 0.0, y: 0.0 };
            if raw.distance(origin) < distance {
                return Some(SnapResult {
                    point: origin,
                    kind: SnapKind::Origin,
                });
            }
        }
        best.or_else(|| {
            grid.then(|| SnapResult {
                point: Point {
                    x: raw.x.round(),
                    y: raw.y.round(),
                },
                kind: SnapKind::Grid,
            })
        })
    }

    pub fn fit_shapes(&mut self, viewport_width: f32, viewport_height: f32) -> bool {
        let mut bounds = self.shapes.iter().filter_map(Shape::bounds);
        let Some((mut min, mut max)) = bounds.next() else {
            return false;
        };
        for (lower, upper) in bounds {
            min.x = min.x.min(lower.x);
            min.y = min.y.min(lower.y);
            max.x = max.x.max(upper.x);
            max.y = max.y.max(upper.y);
        }
        if !viewport_width.is_finite()
            || !viewport_height.is_finite()
            || viewport_width <= 96.0
            || viewport_height <= 96.0
        {
            return false;
        }
        let span_x = (max.x - min.x).max(1.0);
        let span_y = (max.y - min.y).max(1.0);
        let zoom = ((viewport_width - 96.0) / span_x)
            .min((viewport_height - 96.0) / span_y)
            .clamp(0.2, 8.0);
        let center = Point {
            x: min.x * 0.5 + max.x * 0.5,
            y: min.y * 0.5 + max.y * 0.5,
        };
        if !zoom.is_finite() || !center.x.is_finite() || !center.y.is_finite() {
            return false;
        }
        self.zoom = zoom;
        self.pan = Point {
            x: -center.x * zoom,
            y: center.y * zoom,
        };
        true
    }

    fn snapshot(&self) -> DocumentState {
        DocumentState {
            shapes: self.shapes.clone(),
            driving_dimensions: self.driving_dimensions.clone(),
            construction: self.construction.clone(),
            axis_constraints: self.axis_constraints.clone(),
            point_links: self.point_links.clone(),
            equal_lines: self.equal_lines.clone(),
            equal_radii: self.equal_radii.clone(),
            radius_differences: self.radius_differences.clone(),
            internal_tangents: self.internal_tangents.clone(),
            external_tangents: self.external_tangents.clone(),
            midpoint_links: self.midpoint_links.clone(),
            point_curve_links: self.point_curve_links.clone(),
            normal_links: self.normal_links.clone(),
            endpoint_tangents: self.endpoint_tangents.clone(),
            equal_ellipses: self.equal_ellipses.clone(),
            shared_ellipses: self.shared_ellipses.clone(),
            concentric: self.concentric.clone(),
            line_relations: self.line_relations.clone(),
            fixed_anchors: self.fixed_anchors.clone(),
        }
    }

    fn restore(&mut self, state: DocumentState) {
        self.shapes = state.shapes;
        self.driving_dimensions = state.driving_dimensions;
        self.construction = state.construction;
        self.axis_constraints = state.axis_constraints;
        self.point_links = state.point_links;
        self.equal_lines = state.equal_lines;
        self.equal_radii = state.equal_radii;
        self.radius_differences = state.radius_differences;
        self.internal_tangents = state.internal_tangents;
        self.external_tangents = state.external_tangents;
        self.midpoint_links = state.midpoint_links;
        self.point_curve_links = state.point_curve_links;
        self.normal_links = state.normal_links;
        self.endpoint_tangents = state.endpoint_tangents;
        self.equal_ellipses = state.equal_ellipses;
        self.shared_ellipses = state.shared_ellipses;
        self.concentric = state.concentric;
        self.line_relations = state.line_relations;
        self.fixed_anchors = state.fixed_anchors;
        self.selected = None;
        self.selected_spline_point = None;
        self.draft_start = None;
        self.draft_end = None;
        self.drag = None;
    }

    pub fn from_shapes(shapes: Vec<Shape>) -> Self {
        let next_id = shapes
            .iter()
            .map(Shape::id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        Self {
            shapes,
            next_id,
            ..Self::default()
        }
    }

    pub fn screen_to_world(&self, point: Point, viewport_origin: Point) -> Point {
        Point {
            x: (point.x - viewport_origin.x - self.pan.x) / self.zoom,
            y: -(point.y - viewport_origin.y - self.pan.y) / self.zoom,
        }
    }

    pub fn world_to_screen(&self, point: Point, viewport_origin: Point) -> Point {
        Point {
            x: viewport_origin.x + self.pan.x + point.x * self.zoom,
            y: viewport_origin.y + self.pan.y - point.y * self.zoom,
        }
    }

    pub fn zoom_at(&mut self, cursor: Point, viewport_origin: Point, factor: f32) {
        let anchor = self.screen_to_world(cursor, viewport_origin);
        self.zoom = (self.zoom * factor).clamp(0.2, 8.0);
        self.pan.x = cursor.x - viewport_origin.x - anchor.x * self.zoom;
        self.pan.y = cursor.y - viewport_origin.y + anchor.y * self.zoom;
    }

    pub fn select_at(&mut self, point: Point) {
        let tolerance = 7.0 / self.zoom.max(0.2);
        if let Some(Shape::Spline { points, .. }) = self
            .selected
            .and_then(|id| self.shapes.iter().find(|shape| shape.id() == id))
        {
            if let Some((index, _)) = points
                .iter()
                .enumerate()
                .filter(|(_, handle)| handle.distance(point) <= tolerance)
                .min_by(|left, right| left.1.distance(point).total_cmp(&right.1.distance(point)))
            {
                self.selected_spline_point = Some(index);
                return;
            }
        }
        self.selected = self
            .shapes
            .iter()
            .rev()
            .find(|shape| shape.hit(point, 5.0 / self.zoom))
            .map(Shape::id);
        self.selected_spline_point = self.selected.and_then(|id| {
            self.shapes
                .iter()
                .find(|shape| shape.id() == id)
                .and_then(|shape| {
                    if let Shape::Spline { points, .. } = shape {
                        points
                            .iter()
                            .enumerate()
                            .find(|(_, handle)| handle.distance(point) <= tolerance)
                            .map(|(index, _)| index)
                    } else {
                        None
                    }
                })
        });
    }

    pub fn set_selected_spline_coordinate(
        &mut self,
        x_axis: bool,
        value: f32,
    ) -> Result<(), String> {
        if !value.is_finite() {
            return Err("Spline coordinate must be finite".into());
        }
        let id = self.selected.ok_or("Select a spline point")?;
        let handle = self.selected_spline_point.ok_or("Select a spline point")?;
        if self.fixed_anchors.contains_key(&(id, handle)) {
            return Err("Spline point is fixed".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected spline is missing")?;
        let Shape::Spline {
            mode,
            ref points,
            closed,
            ..
        } = self.shapes[index]
        else {
            return Err("Select a spline point".into());
        };
        let mut changed = points.clone();
        let point = changed.get_mut(handle).ok_or("Spline point is missing")?;
        if x_axis {
            point.x = value;
        } else {
            point.y = value;
        }
        spline::beziers(mode, &changed, closed)?;
        if changed == *points {
            return Ok(());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if let Shape::Spline { points, .. } = &mut self.shapes[index] {
            *points = changed;
        }
        // Linked points and curve relations follow the edited handle.
        self.solve_or_rollback(&[Pin::Anchor(id, handle)])
    }

    pub fn set_selected_point_coordinate(
        &mut self,
        x_axis: bool,
        value: f32,
    ) -> Result<(), String> {
        if !value.is_finite() {
            return Err("Point coordinate must be finite".into());
        }
        let id = self.selected.ok_or("Select a sketch point")?;
        if self.midpoint_links.contains_key(&id) {
            return Err("Point is driven by Midpoint".into());
        }
        let Shape::Point { at, .. } = self
            .shapes
            .iter()
            .find(|shape| shape.id() == id)
            .ok_or("Selected point is missing")?
        else {
            return Err("Select a sketch point".into());
        };
        let delta = if x_axis {
            Point {
                x: value - at.x,
                y: 0.0,
            }
        } else {
            Point {
                x: 0.0,
                y: value - at.y,
            }
        };
        if delta.x == 0.0 && delta.y == 0.0 {
            return Ok(());
        }
        if self.move_selected(delta) {
            Ok(())
        } else {
            Err("Point has a driving linked line or invalid coordinate".into())
        }
    }

    pub fn begin_drag(&mut self, start: Point) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let Some(original) = self.shapes.iter().find(|shape| shape.id() == id).cloned() else {
            return false;
        };
        if self.midpoint_links.contains_key(&id) {
            return false;
        }
        if self.has_fixed_anchor(id) {
            return false;
        }
        if self
            .shared_ellipse_partners(id)
            .into_iter()
            .any(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        if self
            .concentric_partner(id)
            .is_some_and(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        self.drag = Some(DragState {
            id,
            start,
            original,
            before: self.snapshot(),
            spline_point: self.selected_spline_point,
        });
        true
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub fn drag_to(&mut self, point: Point) -> bool {
        let Some(drag) = &self.drag else {
            return false;
        };
        let delta = Point {
            x: point.x - drag.start.x,
            y: point.y - drag.start.y,
        };
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return false;
        }
        let mut next = if drag.start.distance(point) > 2.0 / self.zoom {
            if let (
                Shape::Spline {
                    mode,
                    points,
                    closed,
                    ..
                },
                Some(handle),
            ) = (&drag.original, drag.spline_point)
            {
                let mut next = drag.original.clone();
                if let Shape::Spline {
                    points: changed, ..
                } = &mut next
                {
                    changed[handle] = Point {
                        x: points[handle].x + delta.x,
                        y: points[handle].y + delta.y,
                    };
                }
                if spline::beziers(
                    *mode,
                    match &next {
                        Shape::Spline { points, .. } => points,
                        _ => unreachable!(),
                    },
                    *closed,
                )
                .is_err()
                {
                    return false;
                }
                next
            } else {
                drag.original.translated(delta)
            }
        } else {
            drag.original.clone()
        };
        if let Some(&(source_id, kind)) = self.line_relations.get(&drag.id) {
            if kind == LineRelationKind::Collinear {
                let Some(source) = self.shapes.iter().find(|shape| shape.id() == source_id) else {
                    return false;
                };
                let Some(projected) = Self::line_relation_shape(source, &next, kind) else {
                    return false;
                };
                next = projected;
            }
        }
        let Some(index) = self.shapes.iter().position(|shape| shape.id() == drag.id) else {
            return false;
        };
        if self.shapes[index] == next {
            return false;
        }
        let frame_before = self.shapes.clone();
        self.shapes[index] = next;
        let id = drag.id;
        if self.solve_constraints(&[Pin::Shape(id)]).is_err() {
            self.shapes = frame_before;
            return false;
        }
        true
    }

    pub fn finish_drag(&mut self) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        let changed = self
            .shapes
            .iter()
            .any(|shape| shape.id() == drag.id && *shape != drag.original);
        if changed {
            self.history.push(drag.before);
            self.future.clear();
        }
        changed
    }

    pub fn cancel_drag(&mut self) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        self.restore(drag.before);
        true
    }

    pub fn finish_draft(&mut self, end: Point) -> bool {
        if matches!(
            self.tool,
            Tool::ThreePointRectangle | Tool::ThreePointCenterRectangle
        ) {
            let Some(first) = self.draft_start else {
                return false;
            };
            if let Some(second) = self.draft_end {
                let shape = match self.tool {
                    Tool::ThreePointRectangle => {
                        Shape::rectangle_from_three_points(self.next_id, first, second, end)
                    }
                    _ => Shape::rectangle_from_center_side(self.next_id, first, second, end),
                };
                let Some(shape) = shape else {
                    return false;
                };
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if first.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::CenterArc {
            let Some(center) = self.draft_start else {
                return false;
            };
            if let Some(start) = self.draft_end {
                let Some(shape) = Shape::arc_from_center_points(
                    self.next_id,
                    center,
                    start,
                    end,
                    self.arc_clockwise,
                ) else {
                    return false;
                };
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if center.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::ThreePointCircle {
            let Some(first) = self.draft_start else {
                return false;
            };
            if let Some(second) = self.draft_end {
                let Some(shape) = Shape::circle_from_three_points(self.next_id, first, second, end)
                else {
                    return false;
                };
                if let Shape::Circle { radius, .. } = shape {
                    if radius <= 2.0 / self.zoom {
                        return false;
                    }
                }
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if first.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::Slot {
            let Some(from) = self.draft_start else {
                return false;
            };
            if let Some(to) = self.draft_end {
                let Some(shape) = Shape::capsule_from_three_points(self.next_id, from, to, end)
                else {
                    return false;
                };
                if let Shape::Capsule { width, .. } = &shape {
                    if *width <= 2.0 / self.zoom {
                        return false;
                    }
                }
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if from.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::Ellipse {
            let Some(center) = self.draft_start else {
                return false;
            };
            if let Some(first_axis) = self.draft_end {
                let Some(shape) =
                    Shape::ellipse_from_three_points(self.next_id, center, first_axis, end)
                else {
                    return false;
                };
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if center.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        if self.tool == Tool::Arc {
            let Some(start) = self.draft_start else {
                return false;
            };
            if let Some(arc_end) = self.draft_end {
                let Some(shape) = Shape::arc_from_three_points(self.next_id, start, arc_end, end)
                else {
                    return false;
                };
                self.history.push(self.snapshot());
                self.future.clear();
                self.selected = Some(self.next_id);
                self.next_id += 1;
                self.shapes.push(shape);
                self.draft_start = None;
                self.draft_end = None;
                return true;
            }
            if start.distance(end) > 2.0 / self.zoom {
                self.draft_end = Some(end);
            }
            return false;
        }
        let Some(start) = self.draft_start.take() else {
            return false;
        };
        let minimum = 2.0 / self.zoom;
        let shape = match self.tool {
            Tool::Line if start.distance(end) > minimum => Shape::Line {
                id: self.next_id,
                a: start,
                b: end,
            },
            Tool::Rectangle
                if (end.x - start.x).abs() > minimum && (end.y - start.y).abs() > minimum =>
            {
                Shape::Rectangle {
                    id: self.next_id,
                    a: start,
                    b: end,
                }
            }
            Tool::CenterRectangle
                if (end.x - start.x).abs() > minimum * 0.5
                    && (end.y - start.y).abs() > minimum * 0.5 =>
            {
                Shape::Rectangle {
                    id: self.next_id,
                    a: Point {
                        x: start.x - (end.x - start.x),
                        y: start.y - (end.y - start.y),
                    },
                    b: end,
                }
            }
            Tool::Circle if start.distance(end) > minimum => Shape::Circle {
                id: self.next_id,
                center: start,
                radius: start.distance(end),
                segments: 64,
            },
            Tool::Polygon if start.distance(end) > minimum => Shape::Polygon {
                id: self.next_id,
                center: start,
                radius: start.distance(end),
                sides: 6,
                rotation: (end.y - start.y).atan2(end.x - start.x).to_degrees(),
            },
            Tool::Point => return false,
            Tool::ThreePointCircle => return false,
            Tool::Slot => return false,
            Tool::Ellipse => return false,
            Tool::Arc => return false,
            Tool::CenterArc => return false,
            Tool::ThreePointRectangle | Tool::ThreePointCenterRectangle => return false,
            _ => return false,
        };
        self.history.push(self.snapshot());
        self.future.clear();
        if let Shape::Line { id, a, b } = &shape {
            for (endpoint, position) in [(0, *a), (1, *b)] {
                if let Some(point_id) = self.shapes.iter().find_map(|existing| match existing {
                    Shape::Point { id: point_id, at } if at.distance(position) < 1e-5 => {
                        Some(*point_id)
                    }
                    _ => None,
                }) {
                    self.point_links.insert((*id, endpoint), point_id);
                }
            }
        }
        self.selected = Some(self.next_id);
        self.next_id += 1;
        self.shapes.push(shape);
        true
    }

    pub fn add_point(&mut self, at: Point) -> bool {
        if !at.x.is_finite() || !at.y.is_finite() {
            return false;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.selected = Some(self.next_id);
        self.shapes.push(Shape::Point {
            id: self.next_id,
            at,
        });
        self.next_id += 1;
        true
    }

    pub fn add_spline(
        &mut self,
        mode: SplineMode,
        points: Vec<Point>,
        closed: bool,
    ) -> Result<u32, String> {
        spline::beziers(mode, &points, closed)?;
        let id = self.next_id;
        self.history.push(self.snapshot());
        self.future.clear();
        self.selected = Some(id);
        self.selected_spline_point = None;
        if !closed {
            for (endpoint, position) in [(0, points[0]), (1, *points.last().unwrap())] {
                if let Some(point_id) = self.shapes.iter().find_map(|shape| match shape {
                    Shape::Point { id, at } if at.distance(position) < 1e-5 => Some(*id),
                    _ => None,
                }) {
                    self.point_links.insert((id, endpoint), point_id);
                }
            }
        }
        self.shapes.push(Shape::Spline {
            id,
            mode,
            points,
            closed,
        });
        self.next_id = self.next_id.saturating_add(1);
        Ok(id)
    }

    pub fn set_polygon_sides(&mut self, sides: u8) -> Result<(), String> {
        if !(3..=64).contains(&sides) {
            return Err("polygon sides must be between 3 and 64".into());
        }
        let id = self.selected.ok_or("select a polygon first")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("selected geometry no longer exists")?;
        let Shape::Polygon { sides: current, .. } = &self.shapes[index] else {
            return Err("select a polygon first".into());
        };
        if *current != sides {
            if self
                .fixed_anchors
                .keys()
                .any(|(shape_id, anchor)| *shape_id == id && *anchor > 0)
            {
                return Err("polygon vertex is fixed".into());
            }
            self.history.push(self.snapshot());
            self.future.clear();
            if let Shape::Polygon { sides: current, .. } = &mut self.shapes[index] {
                *current = sides;
            }
        }
        Ok(())
    }

    pub fn set_capsule_width(&mut self, width: f32) -> Result<(), String> {
        if !width.is_finite() || width <= 0.0 {
            return Err("slot width must be positive".into());
        }
        let id = self.selected.ok_or("select a slot first")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("selected geometry no longer exists")?;
        let Shape::Capsule { width: current, .. } = &self.shapes[index] else {
            return Err("select a slot first".into());
        };
        if *current != width {
            self.history.push(self.snapshot());
            self.future.clear();
            if let Shape::Capsule { width: current, .. } = &mut self.shapes[index] {
                *current = width;
            }
        }
        Ok(())
    }

    pub fn toggle_axis_constraint(&mut self, axis: AxisConstraint) -> Result<bool, String> {
        let id = self.selected.ok_or("select a line first")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("selected geometry no longer exists")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("select a line first".into());
        };
        if self.normal_links.contains_key(&id) && self.axis_constraints.get(&id) != Some(&axis) {
            return Err("Line direction is driven by a relation".into());
        }
        if self.axis_constraints.get(&id) == Some(&axis) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.axis_constraints.remove(&id);
            return Ok(false);
        }
        let length = a.distance(b);
        let next = match axis {
            AxisConstraint::Horizontal => Point {
                x: a.x + length.copysign(if b.x == a.x { 1.0 } else { b.x - a.x }),
                y: a.y,
            },
            AxisConstraint::Vertical => Point {
                x: a.x,
                y: a.y + length.copysign(if b.y == a.y { 1.0 } else { b.y - a.y }),
            },
        };
        self.history.push(self.snapshot());
        self.future.clear();
        // Keep the fixed or linked second end as the solver's initial guess
        // when it cannot move; the solve then decides how the line aligns.
        if !self.fixed_anchors.contains_key(&(id, 1)) && !self.point_links.contains_key(&(id, 1)) {
            self.shapes[index] = Shape::Line { id, a, b: next };
        }
        self.axis_constraints.insert(id, axis);
        self.solve_or_rollback(&[Pin::Anchor(id, 0)])?;
        Ok(true)
    }

    pub fn toggle_equal_line_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a line to match")?;
        if self.normal_links.contains_key(&target_id) {
            return Err("Remove Normal before Equal".into());
        }
        if target_id == source_id {
            return Err("Hover a different line".into());
        }
        if self.equal_lines.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.equal_lines.remove(&target_id);
            return Ok(false);
        }
        if self.equal_lines.contains_key(&target_id) {
            return Err("Selected line already has an equal relation".into());
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Source line is missing")?;
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected line is missing")?;
        let (
            Shape::Line {
                a: source_a,
                b: source_b,
                ..
            },
            Shape::Line { a, b, .. },
        ) = (source, &self.shapes[target_index])
        else {
            return Err("Equal requires two lines".into());
        };
        let length = source_a.distance(*source_b);
        let old_length = a.distance(*b);
        if !length.is_finite() || length <= 1e-6 || !old_length.is_finite() || old_length <= 1e-6 {
            return Err("Equal requires nonzero finite lines".into());
        }
        let next_b = Point {
            x: a.x + (b.x - a.x) * length / old_length,
            y: a.y + (b.y - a.y) * length / old_length,
        };
        if !next_b.x.is_finite() || !next_b.y.is_finite() {
            return Err("Equal line length is out of range".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if !self.fixed_anchors.contains_key(&(target_id, 1))
            && !self.point_links.contains_key(&(target_id, 1))
        {
            if let Shape::Line { b, .. } = &mut self.shapes[target_index] {
                *b = next_b;
            }
        }
        self.equal_lines.insert(target_id, source_id);
        self.solve_or_rollback(&[Pin::Shape(source_id)])?;
        Ok(true)
    }

    fn line_relation_shape(
        source: &Shape,
        target: &Shape,
        kind: LineRelationKind,
    ) -> Option<Shape> {
        let (
            Shape::Line {
                a: source_a,
                b: source_b,
                ..
            },
            Shape::Line { id, a, b },
        ) = (source, target)
        else {
            return None;
        };
        let source_delta = Point {
            x: source_b.x - source_a.x,
            y: source_b.y - source_a.y,
        };
        let target_delta = Point {
            x: b.x - a.x,
            y: b.y - a.y,
        };
        let source_length = source_a.distance(*source_b);
        let target_length = a.distance(*b);
        if !source_length.is_finite()
            || !target_length.is_finite()
            || source_length <= 1e-6
            || target_length <= 1e-6
        {
            return None;
        }
        let direction = match kind {
            LineRelationKind::Parallel | LineRelationKind::Collinear => Point {
                x: source_delta.x / source_length,
                y: source_delta.y / source_length,
            },
            LineRelationKind::Perpendicular => Point {
                x: -source_delta.y / source_length,
                y: source_delta.x / source_length,
            },
            LineRelationKind::Angle(value) => {
                let (sine, cosine) = value.to_radians().sin_cos();
                Point {
                    x: (source_delta.x * cosine - source_delta.y * sine) / source_length,
                    y: (source_delta.x * sine + source_delta.y * cosine) / source_length,
                }
            }
        };
        let sign = if !matches!(kind, LineRelationKind::Angle(_))
            && direction.x * target_delta.x + direction.y * target_delta.y < 0.0
        {
            -1.0
        } else {
            1.0
        };
        let next_a = if kind == LineRelationKind::Collinear {
            let offset = (a.x - source_a.x) * direction.x + (a.y - source_a.y) * direction.y;
            Point {
                x: source_a.x + direction.x * offset,
                y: source_a.y + direction.y * offset,
            }
        } else {
            *a
        };
        let next_b = Point {
            x: next_a.x + direction.x * target_length * sign,
            y: next_a.y + direction.y * target_length * sign,
        };
        (next_a.x.is_finite()
            && next_a.y.is_finite()
            && next_b.x.is_finite()
            && next_b.y.is_finite())
        .then_some(Shape::Line {
            id: *id,
            a: next_a,
            b: next_b,
        })
    }

    pub fn toggle_line_relation_with(
        &mut self,
        source_id: u32,
        kind: LineRelationKind,
    ) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a line")?;
        if self.normal_links.contains_key(&target_id) || self.normal_links.contains_key(&source_id)
        {
            return Err("Remove Normal before line direction relation".into());
        }
        if target_id == source_id {
            return Err("Hover a different line".into());
        }
        if self
            .line_relations
            .get(&target_id)
            .is_some_and(|(source, existing)| {
                *source == source_id && existing.name() == kind.name()
            })
        {
            self.history.push(self.snapshot());
            self.future.clear();
            self.line_relations.remove(&target_id);
            return Ok(false);
        }
        if kind.angle_value().is_some_and(|value| !value.is_finite()) {
            return Err("Angle must be finite".into());
        }
        if self.line_relations.contains_key(&target_id) {
            return Err("Selected line already has a direction relation".into());
        }
        let target_constrained = self.has_fixed_anchor(target_id)
            || self.axis_constraints.contains_key(&target_id)
            || self.point_links.keys().any(|(id, _)| *id == target_id);
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Source line is missing")?;
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected line is missing")?;
        let next = Self::line_relation_shape(source, &self.shapes[target_index], kind)
            .ok_or("Direction relation requires two nonzero finite lines")?;
        self.history.push(self.snapshot());
        self.future.clear();
        if !target_constrained {
            self.shapes[target_index] = next;
        }
        self.line_relations.insert(target_id, (source_id, kind));
        self.solve_or_rollback(&[Pin::Shape(source_id)])?;
        Ok(true)
    }

    pub fn toggle_line_angle_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a target line")?;
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Source line is missing")?;
        let target = self
            .shapes
            .iter()
            .find(|shape| shape.id() == target_id)
            .ok_or("Target line is missing")?;
        let (Shape::Line { a: sa, b: sb, .. }, Shape::Line { a: ta, b: tb, .. }) = (source, target)
        else {
            return Err("Angle requires two lines".into());
        };
        let source_delta = Point {
            x: sb.x - sa.x,
            y: sb.y - sa.y,
        };
        let target_delta = Point {
            x: tb.x - ta.x,
            y: tb.y - ta.y,
        };
        if sa.distance(*sb) <= 1e-6 || ta.distance(*tb) <= 1e-6 {
            return Err("Angle requires nonzero lines".into());
        }
        let value = (source_delta.x * target_delta.y - source_delta.y * target_delta.x)
            .atan2(source_delta.x * target_delta.x + source_delta.y * target_delta.y)
            .to_degrees();
        self.toggle_line_relation_with(source_id, LineRelationKind::Angle(value))
    }

    pub fn set_line_relation_angle(&mut self, value: f32) -> Result<(), String> {
        if !value.is_finite() {
            return Err("Angle must be finite".into());
        }
        let target_id = self.selected.ok_or("Select an angle target line")?;
        let &(source_id, LineRelationKind::Angle(old_value)) = self
            .line_relations
            .get(&target_id)
            .ok_or("Selected line has no angle relation")?
        else {
            return Err("Selected line has no angle relation".into());
        };
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Angle source is missing")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Angle target is missing")?;
        let next =
            Self::line_relation_shape(source, &self.shapes[index], LineRelationKind::Angle(value))
                .ok_or("Angle target is invalid")?;
        if (old_value - value).abs() <= 1e-6 && self.shapes[index] == next {
            return Ok(());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = next;
        self.line_relations
            .insert(target_id, (source_id, LineRelationKind::Angle(value)));
        self.solve_or_rollback(&[Pin::Shape(source_id)])
    }

    pub fn toggle_equal_radius_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a circle or arc to match")?;
        if self.internal_tangent_member(target_id) || self.internal_tangent_member(source_id) {
            return Err("Remove internal tangent before Equal radius".into());
        }
        if target_id == source_id {
            return Err("Hover a different circle or arc".into());
        }
        if self.equal_radii.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.equal_radii.remove(&target_id);
            return Ok(false);
        }
        if self.equal_radii.iter().any(|(&target, &source)| {
            target == target_id || source == target_id || target == source_id || source == source_id
        }) {
            return Err("A circle or arc already has an equal relation".into());
        }
        if self
            .radius_differences
            .iter()
            .any(|(&target, &(source, _))| {
                [source, target].contains(&source_id) || [source, target].contains(&target_id)
            })
        {
            return Err("A circle already has a radial distance relation".into());
        }
        let source_radius = match self.shapes.iter().find(|shape| shape.id() == source_id) {
            Some(Shape::Circle { radius, .. } | Shape::Arc { radius, .. }) => *radius,
            _ => return Err("Equal radius requires circles or arcs".into()),
        };
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected geometry is missing")?;
        if !matches!(
            self.shapes[target_index],
            Shape::Circle { .. } | Shape::Arc { .. }
        ) {
            return Err("Equal radius requires circles or arcs".into());
        }
        if !source_radius.is_finite() || source_radius <= 1e-6 {
            return Err("Equal requires a positive finite radius".into());
        }
        if matches!(self.shapes[target_index], Shape::Arc { .. })
            && ((1..=2).any(|anchor| self.fixed_anchors.contains_key(&(target_id, anchor))))
        {
            return Err("Target arc endpoint is fixed".into());
        }
        if self
            .driving_dimensions
            .contains_key(&(target_id, DimensionField::CircleDiameter))
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::CurveRadius))
        {
            return Err("Target radius is constrained".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        match &mut self.shapes[target_index] {
            Shape::Circle { radius, .. } | Shape::Arc { radius, .. } => *radius = source_radius,
            _ => unreachable!(),
        }
        self.equal_radii.insert(target_id, source_id);
        Ok(true)
    }

    pub fn toggle_radius_difference_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a target circle")?;
        if self.internal_tangent_member(target_id) || self.internal_tangent_member(source_id) {
            return Err("Remove internal tangent before radial distance".into());
        }
        if source_id == target_id {
            return Err("Hover another circle".into());
        }
        if self
            .radius_differences
            .get(&target_id)
            .is_some_and(|(source, _)| *source == source_id)
        {
            self.history.push(self.snapshot());
            self.future.clear();
            self.radius_differences.remove(&target_id);
            return Ok(false);
        }
        if self
            .radius_differences
            .iter()
            .any(|(&target, &(source, _))| {
                [source, target].contains(&source_id) || [source, target].contains(&target_id)
            })
            || self.equal_radii.iter().any(|(&target, &source)| {
                [source, target].contains(&source_id) || [source, target].contains(&target_id)
            })
        {
            return Err("A circle already has a radius relation".into());
        }
        let source_radius = match self.shapes.iter().find(|shape| shape.id() == source_id) {
            Some(Shape::Circle { radius, .. }) => *radius,
            _ => return Err("Radial distance requires two circles".into()),
        };
        let target_radius = match self.shapes.iter().find(|shape| shape.id() == target_id) {
            Some(Shape::Circle { radius, .. }) => *radius,
            _ => return Err("Radial distance requires two circles".into()),
        };
        if !source_radius.is_finite()
            || !target_radius.is_finite()
            || source_radius <= 1e-6
            || target_radius <= 1e-6
        {
            return Err("Circle radii must be positive and finite".into());
        }
        if self
            .driving_dimensions
            .contains_key(&(target_id, DimensionField::CircleDiameter))
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::CurveRadius))
        {
            return Err("Target radius already has a driving dimension".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.radius_differences
            .insert(target_id, (source_id, source_radius - target_radius));
        Ok(true)
    }

    pub fn set_radius_difference(&mut self, value: f32) -> Result<(), String> {
        if !value.is_finite() {
            return Err("Radial distance must be finite".into());
        }
        let target_id = self.selected.ok_or("Select a radial distance target")?;
        let &(source_id, old_value) = self
            .radius_differences
            .get(&target_id)
            .ok_or("Selected circle has no radial distance")?;
        let source_radius = match self.shapes.iter().find(|shape| shape.id() == source_id) {
            Some(Shape::Circle { radius, .. }) => *radius,
            _ => return Err("Source circle is missing".into()),
        };
        let target_radius = source_radius - value;
        if !target_radius.is_finite() || target_radius <= 1e-6 {
            return Err("Target radius must stay positive".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Target circle is missing")?;
        if !matches!(self.shapes[index], Shape::Circle { .. }) {
            return Err("Target is not a circle".into());
        }
        if (old_value - value).abs() <= 1e-6 {
            return Ok(());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if let Shape::Circle { radius, .. } = &mut self.shapes[index] {
            *radius = target_radius;
        }
        self.radius_differences
            .insert(target_id, (source_id, value));
        Ok(())
    }

    fn circular_data(shape: &Shape) -> Option<(Point, f32, Option<(f64, f64)>)> {
        match shape {
            Shape::Circle { center, radius, .. } => Some((*center, *radius, None)),
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => Some((*center, *radius, Some((*start_angle, *sweep)))),
            _ => None,
        }
    }

    fn internal_tangent_target(outer: &Shape, inner: &Shape) -> Option<Shape> {
        let (outer_center, outer_radius, outer_arc) = Self::circular_data(outer)?;
        let (inner_center, inner_radius, inner_arc) = Self::circular_data(inner)?;
        let distance = outer_radius - inner_radius;
        if !distance.is_finite() || distance <= 1e-6 {
            return None;
        }
        let preferred = ((inner_center.y - outer_center.y) as f64)
            .atan2((inner_center.x - outer_center.x) as f64);
        let mut candidates = vec![preferred];
        for (start, sweep) in [outer_arc, inner_arc].into_iter().flatten() {
            candidates.extend([
                start,
                start + sweep,
                start + sweep * 0.5,
                start + sweep * 1e-5,
                start + sweep * (1.0 - 1e-5),
            ]);
        }
        candidates.sort_by(|a, b| {
            let error = |angle: f64| {
                (angle - preferred)
                    .sin()
                    .atan2((angle - preferred).cos())
                    .abs()
            };
            error(*a).total_cmp(&error(*b))
        });
        for angle in candidates {
            let unit = Point {
                x: angle.cos() as f32,
                y: angle.sin() as f32,
            };
            let center = Point {
                x: outer_center.x + unit.x * distance,
                y: outer_center.y + unit.y * distance,
            };
            let contact = Point {
                x: outer_center.x + unit.x * outer_radius,
                y: outer_center.y + unit.y * outer_radius,
            };
            if ![center.x, center.y, contact.x, contact.y]
                .into_iter()
                .all(f32::is_finite)
            {
                continue;
            }
            if outer_arc
                .is_some_and(|(start, sweep)| !on_arc_sweep(contact, outer_center, start, sweep))
                || inner_arc
                    .is_some_and(|(start, sweep)| !on_arc_sweep(contact, center, start, sweep))
            {
                continue;
            }
            let delta = Point {
                x: center.x - inner_center.x,
                y: center.y - inner_center.y,
            };
            return Some(inner.translated(delta));
        }
        None
    }

    fn external_tangent_target(source: &Shape, target: &Shape) -> Option<Shape> {
        let line_data = |shape: &Shape| match shape {
            Shape::Line { a, b, .. } => {
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let length = (dx * dx + dy * dy).sqrt();
                (length > 1e-6).then_some((
                    *a,
                    Point {
                        x: -dy / length,
                        y: dx / length,
                    },
                ))
            }
            _ => None,
        };
        let ellipse_data = |shape: &Shape| match shape {
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            } => Some((*center, *rx, *ry, *rotation, None)),
            Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                ..
            } => Some((*center, *rx, *ry, *rotation, Some((*start_angle, *sweep)))),
            _ => None,
        };
        let ellipse_support = |rx: f32, ry: f32, rotation: f32, normal: Point| {
            let (sin, cos) = rotation.to_radians().sin_cos();
            let nx = normal.x * cos + normal.y * sin;
            let ny = -normal.x * sin + normal.y * cos;
            let extent = ((rx * nx).powi(2) + (ry * ny).powi(2)).sqrt();
            if extent <= 1e-6 {
                return None;
            }
            let x = rx * rx * nx / extent;
            let y = ry * ry * ny / extent;
            Some((
                extent,
                Point {
                    x: x * cos - y * sin,
                    y: x * sin + y * cos,
                },
            ))
        };
        let arc_contact = |center: Point,
                           rx: f32,
                           ry: f32,
                           rotation: f32,
                           arc: Option<(f64, f64)>,
                           contact: Point| {
            arc.is_none_or(|(start, sweep)| {
                let angle = Shape::ellipse_parameter_angle(center, rx, ry, rotation, contact);
                Shape::elliptic_arc_progress(angle, start, sweep) <= 1.0 + 1e-5
            })
        };
        if let (Some((line_start, normal)), Some((center, rx, ry, rotation, arc))) =
            (line_data(source), ellipse_data(target))
        {
            let (extent, support) = ellipse_support(rx, ry, rotation, normal)?;
            let signed =
                (center.x - line_start.x) * normal.x + (center.y - line_start.y) * normal.y;
            for side in [
                if signed < 0.0 { -1.0 } else { 1.0 },
                if signed < 0.0 { 1.0 } else { -1.0 },
            ] {
                let translation = side * extent - signed;
                let next_center = Point {
                    x: center.x + normal.x * translation,
                    y: center.y + normal.y * translation,
                };
                let contact = Point {
                    x: next_center.x - side * support.x,
                    y: next_center.y - side * support.y,
                };
                if !arc_contact(next_center, rx, ry, rotation, arc, contact) {
                    continue;
                }
                return Some(target.translated(Point {
                    x: normal.x * translation,
                    y: normal.y * translation,
                }));
            }
            return None;
        }
        if let (Some((center, rx, ry, rotation, arc)), Some((line_start, normal))) =
            (ellipse_data(source), line_data(target))
        {
            let (extent, support) = ellipse_support(rx, ry, rotation, normal)?;
            let signed =
                (center.x - line_start.x) * normal.x + (center.y - line_start.y) * normal.y;
            for side in [
                if signed < 0.0 { -1.0 } else { 1.0 },
                if signed < 0.0 { 1.0 } else { -1.0 },
            ] {
                let contact = Point {
                    x: center.x - side * support.x,
                    y: center.y - side * support.y,
                };
                if !arc_contact(center, rx, ry, rotation, arc, contact) {
                    continue;
                }
                let translation = signed - side * extent;
                return Some(target.translated(Point {
                    x: normal.x * translation,
                    y: normal.y * translation,
                }));
            }
            return None;
        }
        if let (Some((line_start, normal)), Some((center, radius, arc))) =
            (line_data(source), Self::circular_data(target))
        {
            let signed =
                (center.x - line_start.x) * normal.x + (center.y - line_start.y) * normal.y;
            for side in [
                if signed < 0.0 { -1.0 } else { 1.0 },
                if signed < 0.0 { 1.0 } else { -1.0 },
            ] {
                let translation = side * radius - signed;
                let next_center = Point {
                    x: center.x + normal.x * translation,
                    y: center.y + normal.y * translation,
                };
                let contact = Point {
                    x: next_center.x - normal.x * side * radius,
                    y: next_center.y - normal.y * side * radius,
                };
                if arc
                    .is_some_and(|(start, sweep)| !on_arc_sweep(contact, next_center, start, sweep))
                {
                    continue;
                }
                return Some(target.translated(Point {
                    x: normal.x * translation,
                    y: normal.y * translation,
                }));
            }
            return None;
        }
        if let (Some((center, radius, arc)), Some((line_start, normal))) =
            (Self::circular_data(source), line_data(target))
        {
            let signed =
                (center.x - line_start.x) * normal.x + (center.y - line_start.y) * normal.y;
            for side in [
                if signed < 0.0 { -1.0 } else { 1.0 },
                if signed < 0.0 { 1.0 } else { -1.0 },
            ] {
                let contact = Point {
                    x: center.x - normal.x * side * radius,
                    y: center.y - normal.y * side * radius,
                };
                if arc.is_some_and(|(start, sweep)| !on_arc_sweep(contact, center, start, sweep)) {
                    continue;
                }
                let translation = signed - side * radius;
                return Some(target.translated(Point {
                    x: normal.x * translation,
                    y: normal.y * translation,
                }));
            }
            return None;
        }
        let (source_center, source_radius, source_arc) = Self::circular_data(source)?;
        let (target_center, target_radius, target_arc) = Self::circular_data(target)?;
        let distance = source_radius + target_radius;
        if !distance.is_finite() || distance <= 1e-6 {
            return None;
        }
        let preferred = ((target_center.y - source_center.y) as f64)
            .atan2((target_center.x - source_center.x) as f64);
        let mut candidates = vec![preferred];
        for (arc, offset) in [(source_arc, 0.0), (target_arc, -std::f64::consts::PI)] {
            if let Some((start, sweep)) = arc {
                candidates.extend([
                    start + offset,
                    start + sweep + offset,
                    start + sweep * 0.5 + offset,
                    start + sweep * 1e-5 + offset,
                    start + sweep * (1.0 - 1e-5) + offset,
                ]);
            }
        }
        candidates.sort_by(|a, b| {
            let error = |angle: f64| {
                (angle - preferred)
                    .sin()
                    .atan2((angle - preferred).cos())
                    .abs()
            };
            error(*a).total_cmp(&error(*b))
        });
        for angle in candidates {
            let unit = Point {
                x: angle.cos() as f32,
                y: angle.sin() as f32,
            };
            let center = Point {
                x: source_center.x + unit.x * distance,
                y: source_center.y + unit.y * distance,
            };
            let contact = Point {
                x: source_center.x + unit.x * source_radius,
                y: source_center.y + unit.y * source_radius,
            };
            if ![center.x, center.y, contact.x, contact.y]
                .into_iter()
                .all(f32::is_finite)
            {
                continue;
            }
            if source_arc
                .is_some_and(|(start, sweep)| !on_arc_sweep(contact, source_center, start, sweep))
                || target_arc
                    .is_some_and(|(start, sweep)| !on_arc_sweep(contact, center, start, sweep))
            {
                continue;
            }
            return Some(target.translated(Point {
                x: center.x - target_center.x,
                y: center.y - target_center.y,
            }));
        }
        None
    }

    fn internal_tangent_member(&self, id: u32) -> bool {
        self.internal_tangents.contains_key(&id)
            || self.internal_tangents.values().any(|source| *source == id)
            || self.external_tangents.contains_key(&id)
            || self.external_tangents.values().any(|source| *source == id)
    }

    pub fn curve_midpoint(shape: &Shape) -> Option<Point> {
        match shape {
            Shape::Line { a, b, .. } => Some(Point {
                x: (a.x + b.x) * 0.5,
                y: (a.y + b.y) * 0.5,
            }),
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                ..
            } => Some(Shape::arc_point(
                *center,
                *radius,
                *start_angle,
                *sweep,
                0.5,
            )),
            Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                ..
            } => Some(Shape::elliptic_arc_point(
                *center,
                *rx,
                *ry,
                *rotation,
                *start_angle,
                *sweep,
                0.5,
            )),
            _ => None,
        }
    }

    pub fn closest_curve_point(shape: &Shape, point: Point) -> Option<Point> {
        if !point.x.is_finite() || !point.y.is_finite() {
            return None;
        }
        match shape {
            Shape::Line { a, b, .. } => {
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let length_squared = dx * dx + dy * dy;
                if length_squared <= 1e-12 {
                    return None;
                }
                let t = (((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared)
                    .clamp(0.0, 1.0);
                Some(Point {
                    x: a.x + t * dx,
                    y: a.y + t * dy,
                })
            }
            Shape::Circle { center, radius, .. } | Shape::Arc { center, radius, .. } => {
                let angle = ((point.y - center.y) as f64).atan2((point.x - center.x) as f64);
                let radial = Point {
                    x: center.x + *radius * angle.cos() as f32,
                    y: center.y + *radius * angle.sin() as f32,
                };
                if let Shape::Arc {
                    start_angle, sweep, ..
                } = shape
                {
                    if on_arc_sweep(radial, *center, *start_angle, *sweep) {
                        return Some(radial);
                    }
                    let start = Shape::arc_point(*center, *radius, *start_angle, *sweep, 0.0);
                    let end = Shape::arc_point(*center, *radius, *start_angle, *sweep, 1.0);
                    Some(if point.distance(start) <= point.distance(end) {
                        start
                    } else {
                        end
                    })
                } else {
                    Some(radial)
                }
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
                if *rx <= 0.0 || *ry <= 0.0 {
                    return None;
                }
                let (start, sweep) = match shape {
                    Shape::EllipticArc {
                        start_angle, sweep, ..
                    } => (*start_angle, *sweep),
                    _ => (0.0, std::f64::consts::TAU),
                };
                let samples = 256_usize;
                let at = |t: f64| {
                    Shape::elliptic_arc_point(*center, *rx, *ry, *rotation, start, sweep, t)
                };
                let distance = |t: f64| {
                    let p = at(t);
                    (p.x - point.x).powi(2) + (p.y - point.y).powi(2)
                };
                let mut best_i = 0;
                let mut best = f32::INFINITY;
                for i in 0..=samples {
                    let d = distance(i as f64 / samples as f64);
                    if d < best {
                        best = d;
                        best_i = i;
                    }
                }
                let mut left = (best_i.saturating_sub(1)) as f64 / samples as f64;
                let mut right = (best_i + 1).min(samples) as f64 / samples as f64;
                for _ in 0..32 {
                    let a = left + (right - left) / 3.0;
                    let b = right - (right - left) / 3.0;
                    if distance(a) <= distance(b) {
                        right = b;
                    } else {
                        left = a;
                    }
                }
                Some(at((left + right) * 0.5))
            }
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                let curves = spline::beziers(*mode, points, *closed).ok()?;
                let hit = spline::nearest(&curves, point).ok()?;
                Some(spline::point(&curves[hit.segment], hit.t))
            }
            _ => None,
        }
    }

    pub fn normal_line_target(source: &Shape, target: &Shape, endpoint: u8) -> Option<Shape> {
        let Shape::Line { id, a, b } = target else {
            return None;
        };
        if endpoint > 1 {
            return None;
        }
        let (anchor, other) = if endpoint == 0 { (*a, *b) } else { (*b, *a) };
        let length = anchor.distance(other);
        if !length.is_finite() || length <= 1e-6 {
            return None;
        }
        let contact = Self::closest_curve_point(source, anchor)?;
        let normal = match source {
            Shape::Line { a, b, .. } => Point {
                x: a.y - b.y,
                y: b.x - a.x,
            },
            Shape::Circle { center, .. } | Shape::Arc { center, .. } => Point {
                x: contact.x - center.x,
                y: contact.y - center.y,
            },
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
                let (sin, cos) = rotation.to_radians().sin_cos();
                let dx = contact.x - center.x;
                let dy = contact.y - center.y;
                let x = (dx * cos + dy * sin) / (rx * rx);
                let y = (-dx * sin + dy * cos) / (ry * ry);
                Point {
                    x: x * cos - y * sin,
                    y: x * sin + y * cos,
                }
            }
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                let curves = spline::beziers(*mode, points, *closed).ok()?;
                let hit = spline::nearest(&curves, contact).ok()?;
                let c = curves.get(hit.segment)?;
                let t = hit.t;
                let tangent = Point {
                    x: 3.0 * (1.0 - t).powi(2) * (c[1].x - c[0].x)
                        + 6.0 * (1.0 - t) * t * (c[2].x - c[1].x)
                        + 3.0 * t.powi(2) * (c[3].x - c[2].x),
                    y: 3.0 * (1.0 - t).powi(2) * (c[1].y - c[0].y)
                        + 6.0 * (1.0 - t) * t * (c[2].y - c[1].y)
                        + 3.0 * t.powi(2) * (c[3].y - c[2].y),
                };
                Point {
                    x: -tangent.y,
                    y: tangent.x,
                }
            }
            _ => return None,
        };
        let magnitude = (normal.x * normal.x + normal.y * normal.y).sqrt();
        if !magnitude.is_finite() || magnitude <= 1e-8 {
            return None;
        }
        let mut unit = Point {
            x: normal.x / magnitude,
            y: normal.y / magnitude,
        };
        if (other.x - anchor.x) * unit.x + (other.y - anchor.y) * unit.y < 0.0 {
            unit.x = -unit.x;
            unit.y = -unit.y;
        }
        let other = Point {
            x: contact.x + length * unit.x,
            y: contact.y + length * unit.y,
        };
        if ![other.x, other.y, contact.x, contact.y]
            .into_iter()
            .all(f32::is_finite)
        {
            return None;
        }
        Some(if endpoint == 0 {
            Shape::Line {
                id: *id,
                a: contact,
                b: other,
            }
        } else {
            Shape::Line {
                id: *id,
                a: other,
                b: contact,
            }
        })
    }

    pub fn toggle_point_on_curve_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a sketch point")?;
        if target_id == source_id {
            return Err("Hover another curve".into());
        }
        if self.point_curve_links.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.point_curve_links.remove(&target_id);
            return Ok(false);
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected point is missing")?;
        let Shape::Point { at, .. } = self.shapes[index] else {
            return Err("Point on curve target must be a sketch point".into());
        };
        let projected = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .and_then(|shape| Self::closest_curve_point(shape, at))
            .ok_or("Point on curve source must be a supported curve")?;
        if self.fixed_anchors.contains_key(&(target_id, 0))
            || self
                .point_links
                .values()
                .any(|point_id| *point_id == target_id)
            || self.midpoint_links.contains_key(&target_id)
            || self.point_curve_links.contains_key(&target_id)
        {
            return Err("Point already has a driving relation".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::Point {
            id: target_id,
            at: projected,
        };
        self.point_curve_links.insert(target_id, source_id);
        Ok(true)
    }

    pub fn toggle_normal_with(&mut self, source_id: u32, cursor: Point) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a line")?;
        if target_id == source_id {
            return Err("Hover a separate curve".into());
        }
        if self
            .normal_links
            .get(&target_id)
            .is_some_and(|(source, _)| *source == source_id)
        {
            self.history.push(self.snapshot());
            self.future.clear();
            self.normal_links.remove(&target_id);
            return Ok(false);
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected line is missing")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("Normal target must be a line".into());
        };
        let endpoint = if cursor.distance(a) <= cursor.distance(b) {
            0
        } else {
            1
        };
        if self.has_fixed_anchor(target_id)
            || self.normal_links.contains_key(&target_id)
            || self
                .normal_links
                .values()
                .any(|(source, _)| *source == target_id)
            || self.normal_links.contains_key(&source_id)
            || self
                .midpoint_links
                .values()
                .any(|source| *source == target_id)
            || self
                .point_curve_links
                .values()
                .any(|source| *source == target_id)
            || self.line_relations.contains_key(&target_id)
            || self.equal_lines.contains_key(&target_id)
            || self.external_tangents.contains_key(&target_id)
            || self.point_links.keys().any(|(id, _)| *id == target_id)
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::Rotation))
        {
            return Err("Line already has a conflicting relation".into());
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Hovered curve is missing")?;
        let next = Self::normal_line_target(source, &self.shapes[index], endpoint)
            .ok_or("No valid normal contact for this curve")?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = next;
        self.normal_links.insert(target_id, (source_id, endpoint));
        Ok(true)
    }

    pub fn toggle_midpoint_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a sketch point")?;
        if target_id == source_id {
            return Err("Hover another line or arc".into());
        }
        if self.midpoint_links.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.midpoint_links.remove(&target_id);
            return Ok(false);
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected point is missing")?;
        if !matches!(self.shapes[index], Shape::Point { .. }) {
            return Err("Midpoint target must be a sketch point".into());
        }
        let midpoint = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .and_then(Self::curve_midpoint)
            .ok_or("Midpoint source must be a line or arc")?;
        if self.fixed_anchors.contains_key(&(target_id, 0))
            || self
                .point_links
                .values()
                .any(|point_id| *point_id == target_id)
        {
            return Err("Point already has a fixed or coincident relation".into());
        }
        if self.midpoint_links.contains_key(&target_id)
            || self.point_curve_links.contains_key(&target_id)
        {
            return Err("Point already has a curve relation".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::Point {
            id: target_id,
            at: midpoint,
        };
        self.midpoint_links.insert(target_id, source_id);
        Ok(true)
    }

    pub fn toggle_external_tangent_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a line or curve")?;
        if self.normal_links.contains_key(&target_id) || self.normal_links.contains_key(&source_id)
        {
            return Err("Remove Normal before Tangent".into());
        }
        if target_id == source_id {
            return Err("Hover another line or curve".into());
        }
        if self.external_tangents.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.external_tangents.remove(&target_id);
            return Ok(false);
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Hovered curve is missing")?;
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected curve is missing")?;
        if self.internal_tangent_member(target_id) || self.internal_tangent_member(source_id) {
            return Err("A curve already has a tangent relation".into());
        }
        if self.has_fixed_anchor(target_id) {
            return Err("Target curve has a fixed anchor".into());
        }
        let next = Self::external_tangent_target(source, &self.shapes[target_index])
            .ok_or("No valid external tangent contact for these curves")?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[target_index] = next;
        self.external_tangents.insert(target_id, source_id);
        Ok(true)
    }

    pub fn toggle_internal_tangent_with(&mut self, other_id: u32) -> Result<bool, String> {
        let selected_id = self.selected.ok_or("Select a circle or arc")?;
        if selected_id == other_id {
            return Err("Hover another circle or arc".into());
        }
        let selected = self
            .shapes
            .iter()
            .find(|shape| shape.id() == selected_id)
            .ok_or("Selected curve is missing")?;
        let other = self
            .shapes
            .iter()
            .find(|shape| shape.id() == other_id)
            .ok_or("Hovered curve is missing")?;
        let (_, selected_radius, _) =
            Self::circular_data(selected).ok_or("Internal tangent requires circles or arcs")?;
        let (_, other_radius, _) =
            Self::circular_data(other).ok_or("Internal tangent requires circles or arcs")?;
        let (outer_id, inner_id) = if selected_radius >= other_radius {
            (selected_id, other_id)
        } else {
            (other_id, selected_id)
        };
        if self.internal_tangents.get(&inner_id) == Some(&outer_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.internal_tangents.remove(&inner_id);
            return Ok(false);
        }
        if self.equal_radii.iter().any(|(&target, &source)| {
            [target, source].contains(&selected_id) || [target, source].contains(&other_id)
        }) || self
            .radius_differences
            .iter()
            .any(|(&target, &(source, _))| {
                [target, source].contains(&selected_id) || [target, source].contains(&other_id)
            })
            || self.concentric_partner(selected_id).is_some()
            || self.concentric_partner(other_id).is_some()
        {
            return Err("Remove conflicting radius or center relations first".into());
        }
        if self.internal_tangent_member(selected_id) || self.internal_tangent_member(other_id) {
            return Err("A curve already has a tangent relation".into());
        }
        if self.has_fixed_anchor(inner_id) {
            return Err("Inner curve has a fixed anchor".into());
        }
        let outer = self
            .shapes
            .iter()
            .find(|shape| shape.id() == outer_id)
            .unwrap();
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == inner_id)
            .unwrap();
        let next = Self::internal_tangent_target(outer, &self.shapes[index])
            .ok_or("No valid internal tangent contact for these curves")?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = next;
        self.internal_tangents.insert(inner_id, outer_id);
        Ok(true)
    }

    pub fn toggle_equal_ellipse_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select an ellipse or elliptic arc")?;
        if target_id == source_id {
            return Err("Hover a different ellipse or elliptic arc".into());
        }
        if self.equal_ellipses.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.equal_ellipses.remove(&target_id);
            return Ok(false);
        }
        if self.shared_ellipse_partner(target_id).is_some()
            || self.shared_ellipse_partner(source_id).is_some()
        {
            return Err("Shared ellipse relation already controls these curves".into());
        }
        if self.equal_ellipses.iter().any(|(&target, &source)| {
            target == target_id || source == target_id || target == source_id || source == source_id
        }) {
            return Err("An ellipse already has an equal relation".into());
        }
        let source_axes = match self.shapes.iter().find(|shape| shape.id() == source_id) {
            Some(Shape::Ellipse { rx, ry, .. } | Shape::EllipticArc { rx, ry, .. }) => (*rx, *ry),
            _ => return Err("Equal requires ellipses or elliptic arcs".into()),
        };
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected ellipse is missing")?;
        if !matches!(
            self.shapes[target_index],
            Shape::Ellipse { .. } | Shape::EllipticArc { .. }
        ) {
            return Err("Equal requires ellipses or elliptic arcs".into());
        }
        if !source_axes.0.is_finite()
            || !source_axes.1.is_finite()
            || source_axes.0 <= 1e-6
            || source_axes.1 <= 1e-6
        {
            return Err("Equal requires positive finite axes".into());
        }
        if self
            .fixed_anchors
            .keys()
            .any(|(id, anchor)| *id == target_id && *anchor > 0)
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::RectangleWidth))
            || self
                .driving_dimensions
                .contains_key(&(target_id, DimensionField::RectangleHeight))
        {
            return Err("Target ellipse axes are constrained".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        match &mut self.shapes[target_index] {
            Shape::Ellipse { rx, ry, .. } | Shape::EllipticArc { rx, ry, .. } => {
                *rx = source_axes.0;
                *ry = source_axes.1;
            }
            _ => unreachable!(),
        }
        self.equal_ellipses.insert(target_id, source_id);
        Ok(true)
    }

    fn concentric_partner(&self, id: u32) -> Option<u32> {
        self.concentric.get(&id).copied().or_else(|| {
            self.concentric
                .iter()
                .find_map(|(&target, &source)| (source == id).then_some(target))
        })
    }

    fn shared_ellipse_partner(&self, id: u32) -> Option<u32> {
        self.shared_ellipse_partners(id).into_iter().next()
    }

    fn shared_ellipse_partners(&self, id: u32) -> Vec<u32> {
        let root = self.shared_ellipses.get(&id).copied().unwrap_or(id);
        let mut group: Vec<_> = self
            .shared_ellipses
            .iter()
            .filter_map(|(&target, &source)| (source == root && target != id).then_some(target))
            .collect();
        if root != id {
            group.push(root);
        }
        group
    }

    pub fn toggle_concentric_with(&mut self, source_id: u32) -> Result<bool, String> {
        let target_id = self.selected.ok_or("Select a circular or elliptic curve")?;
        if self.internal_tangent_member(target_id) || self.internal_tangent_member(source_id) {
            return Err("Remove internal tangent before Concentric".into());
        }
        if target_id == source_id {
            return Err("Hover a different curve".into());
        }
        if self.concentric.get(&target_id) == Some(&source_id) {
            self.history.push(self.snapshot());
            self.future.clear();
            self.concentric.remove(&target_id);
            return Ok(false);
        }
        if self.shared_ellipse_partner(target_id).is_some()
            || self.shared_ellipse_partner(source_id).is_some()
        {
            return Err("Shared ellipse relation already controls these curves".into());
        }
        if self.concentric_partner(target_id).is_some()
            || self.concentric_partner(source_id).is_some()
        {
            return Err("A curve already has a concentric relation".into());
        }
        let source = self
            .shapes
            .iter()
            .find(|shape| shape.id() == source_id)
            .ok_or("Source curve is missing")?;
        let target_index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == target_id)
            .ok_or("Selected curve is missing")?;
        let valid = |shape: &Shape| {
            matches!(
                shape,
                Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
            )
        };
        if !valid(source) || !valid(&self.shapes[target_index]) {
            return Err("Concentric requires circular or elliptic curves".into());
        }
        let source_center = source.center();
        let target_center = self.shapes[target_index].center();
        let delta = Point {
            x: source_center.x - target_center.x,
            y: source_center.y - target_center.y,
        };
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return Err("Concentric center is invalid".into());
        }
        if self.has_fixed_anchor(target_id) && target_center.distance(source_center) > 1e-6 {
            return Err("Target curve has a fixed anchor".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[target_index] = self.shapes[target_index].translated(delta);
        self.concentric.insert(target_id, source_id);
        Ok(true)
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let Some(index) = self.shapes.iter().position(|shape| shape.id() == id) else {
            return false;
        };
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes.remove(index);
        self.equal_lines
            .retain(|target, source| *target != id && *source != id);
        self.line_relations
            .retain(|target, (source, _)| *target != id && *source != id);
        self.equal_radii
            .retain(|target, source| *target != id && *source != id);
        self.radius_differences
            .retain(|target, (source, _)| *target != id && *source != id);
        self.internal_tangents
            .retain(|target, source| *target != id && *source != id);
        self.external_tangents
            .retain(|target, source| *target != id && *source != id);
        self.midpoint_links
            .retain(|target, source| *target != id && *source != id);
        self.point_curve_links
            .retain(|target, source| *target != id && *source != id);
        self.normal_links
            .retain(|target, (source, _)| *target != id && *source != id);
        self.endpoint_tangents
            .retain(|target, (source, _, _, _)| *target != id && *source != id);
        self.equal_ellipses
            .retain(|target, source| *target != id && *source != id);
        self.shared_ellipses
            .retain(|target, source| *target != id && *source != id);
        self.concentric
            .retain(|target, source| *target != id && *source != id);
        self.construction.remove(&id);
        self.axis_constraints.remove(&id);
        self.point_links
            .retain(|(line_id, _), point_id| *line_id != id && *point_id != id);
        self.driving_dimensions
            .retain(|(shape_id, _), _| *shape_id != id);
        self.fixed_anchors
            .retain(|(shape_id, _), _| *shape_id != id);
        self.selected = None;
        true
    }

    pub fn split_selected_line(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select a line to split")?;
        if self.equal_lines.contains_key(&id)
            || self.equal_lines.values().any(|source| *source == id)
            || self.line_relations.contains_key(&id)
            || self
                .line_relations
                .values()
                .any(|(source, _)| *source == id)
        {
            return Err("Remove Equal before splitting the line".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected line is missing")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("Select a line to split".into());
        };
        if !click.x.is_finite() || !click.y.is_finite() {
            return Err("Split position must be finite".into());
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length_squared = dx * dx + dy * dy;
        if length_squared <= 1e-12 {
            return Err("Cannot split a zero-length line".into());
        }
        let t = ((click.x - a.x) * dx + (click.y - a.y) * dy) / length_squared;
        let split = Point {
            x: a.x + t * dx,
            y: a.y + t * dy,
        };
        let minimum = 2.0 / self.zoom.max(0.2);
        if !(0.0..1.0).contains(&t) || a.distance(split) <= minimum || b.distance(split) <= minimum
        {
            return Err("Choose a point inside the line, away from its endpoints".into());
        }
        if click.distance(split) > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected line".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::Line { id, a, b: split };
        self.shapes.insert(
            index + 1,
            Shape::Line {
                id: new_id,
                a: split,
                b,
            },
        );
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        if let Some(axis) = self.axis_constraints.get(&id).copied() {
            self.axis_constraints.insert(new_id, axis);
        }
        self.driving_dimensions
            .remove(&(id, DimensionField::LineLength));
        if let Some(point_id) = self.point_links.remove(&(id, 1)) {
            self.point_links.insert((new_id, 1), point_id);
        }
        if let Some(position) = self.fixed_anchors.remove(&(id, 1)) {
            self.fixed_anchors.insert((new_id, 1), position);
        }
        self.selected = Some(new_id);
        Ok(new_id)
    }

    pub fn split_selected_curve(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select a curve to split")?;
        if self.internal_tangent_member(id)
            || self.midpoint_links.values().any(|source| *source == id)
            || self.point_curve_links.values().any(|source| *source == id)
            || self.normal_links.contains_key(&id)
            || self.normal_links.values().any(|(source, _)| *source == id)
        {
            return Err("Remove dependent relation before splitting".into());
        }
        match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Line { .. }) => self.split_selected_line(click),
            Some(Shape::Arc { .. }) => self.split_selected_arc(click),
            Some(Shape::EllipticArc { .. }) => self.split_selected_elliptic_arc(click),
            Some(Shape::Spline { .. }) => self.split_selected_spline(click),
            _ => Err("Select a curve to split".into()),
        }
    }

    pub fn split_selected_spline(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select a spline to split")?;
        if self.has_fixed_anchor(id) {
            return Err("Remove fixed spline points before splitting".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected spline is missing")?;
        let Shape::Spline {
            mode,
            ref points,
            closed,
            ..
        } = self.shapes[index]
        else {
            return Err("Select a spline to split".into());
        };
        let curves = spline::beziers(mode, points, closed)?;
        let nearest = spline::nearest(&curves, click)?;
        if nearest.distance > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected spline".into());
        }
        if closed {
            let opened = spline::open_periodic(&curves, nearest.segment, nearest.t)?;
            self.history.push(self.snapshot());
            self.future.clear();
            self.shapes[index] = Shape::Spline {
                id,
                mode: SplineMode::Control,
                points: opened,
                closed: false,
            };
            self.selected_spline_point = None;
            return Ok(id);
        }
        let (first, second) = spline::split_open(&curves, nearest.segment, nearest.t)?;
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::Spline {
            id,
            mode: SplineMode::Control,
            points: first,
            closed: false,
        };
        self.shapes.insert(
            index + 1,
            Shape::Spline {
                id: new_id,
                mode: SplineMode::Control,
                points: second,
                closed: false,
            },
        );
        if let Some(point_id) = self.point_links.remove(&(id, 1)) {
            self.point_links.insert((new_id, 1), point_id);
        }
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.selected = Some(new_id);
        self.selected_spline_point = None;
        Ok(new_id)
    }

    pub fn split_selected_elliptic_arc(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select an elliptic arc to split")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected elliptic arc is missing")?;
        let Shape::EllipticArc {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an elliptic arc to split".into());
        };
        if ![click.x, click.y, center.x, center.y, rx, ry, rotation]
            .into_iter()
            .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep == 0.0
        {
            return Err("Elliptic arc or split point is invalid".into());
        }
        let angle = Shape::ellipse_parameter_angle(center, rx, ry, rotation, click);
        let fraction = Shape::elliptic_arc_progress(angle, start_angle, sweep);
        let on_curve =
            Shape::elliptic_arc_point(center, rx, ry, rotation, start_angle, sweep, fraction);
        let minimum = (2.0 / self.zoom.max(0.2)) as f64 / (rx.min(ry) as f64 * sweep.abs());
        if fraction <= minimum
            || fraction >= 1.0 - minimum
            || click.distance(on_curve) > 8.0 / self.zoom.max(0.2)
        {
            return Err("Choose an interior point on the elliptic arc".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        let first_sweep = sweep * fraction;
        self.shapes[index] = Shape::EllipticArc {
            id,
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep: first_sweep,
            segments,
        };
        self.shapes.insert(
            index + 1,
            Shape::EllipticArc {
                id: new_id,
                center,
                rx,
                ry,
                rotation,
                start_angle: start_angle + first_sweep,
                sweep: sweep - first_sweep,
                segments,
            },
        );
        let root = self.shared_ellipses.get(&id).copied().unwrap_or(id);
        self.shared_ellipses.insert(new_id, root);
        if let Some(position) = self.fixed_anchors.remove(&(id, 2)) {
            self.fixed_anchors.insert((new_id, 2), position);
        }
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.selected = Some(new_id);
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(new_id)
    }

    pub fn split_selected_ellipse(&mut self, first: Point, second: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select an ellipse to split")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected ellipse is missing")?;
        let Shape::Ellipse {
            center,
            rx,
            ry,
            rotation,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an ellipse to split".into());
        };
        if ![
            first.x, first.y, second.x, second.y, center.x, center.y, rx, ry, rotation,
        ]
        .into_iter()
        .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
        {
            return Err("Ellipse or split positions are invalid".into());
        }
        let a = Shape::ellipse_parameter_angle(center, rx, ry, rotation, first);
        let b = Shape::ellipse_parameter_angle(center, rx, ry, rotation, second);
        let on_curve = |point: Point, angle: f64| {
            point.distance(Shape::elliptic_arc_point(
                center, rx, ry, rotation, angle, 0.0, 0.0,
            )) <= 8.0 / self.zoom.max(0.2)
        };
        if !on_curve(first, a) || !on_curve(second, b) {
            return Err("Choose two points on the ellipse".into());
        }
        let (start, end) = if a < b { (a, b) } else { (b, a) };
        let minimum = (2.0 / self.zoom.max(0.2)) as f64 / rx.min(ry) as f64;
        let tau = std::f64::consts::TAU;
        if end - start <= minimum || tau - (end - start) <= minimum {
            return Err("Choose two distinct split points".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::EllipticArc {
            id,
            center,
            rx,
            ry,
            rotation,
            start_angle: start,
            sweep: end - start,
            segments,
        };
        self.shapes.insert(
            index + 1,
            Shape::EllipticArc {
                id: new_id,
                center,
                rx,
                ry,
                rotation,
                start_angle: end,
                sweep: start + tau - end,
                segments,
            },
        );
        let root = self.shared_ellipses.get(&id).copied().unwrap_or(id);
        self.shared_ellipses.insert(new_id, root);
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.driving_dimensions
            .remove(&(id, DimensionField::RectangleWidth));
        self.driving_dimensions
            .remove(&(id, DimensionField::RectangleHeight));
        self.selected = Some(new_id);
        Ok(new_id)
    }

    pub fn split_selected_arc(&mut self, click: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select an arc to split")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected arc is missing")?;
        let Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an arc to split".into());
        };
        if !click.x.is_finite()
            || !click.y.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep.abs() <= 1e-9
        {
            return Err("Arc or split position is invalid".into());
        }
        let angle = ((click.y - center.y) as f64).atan2((click.x - center.x) as f64);
        let traveled = if sweep > 0.0 {
            (angle - start_angle).rem_euclid(std::f64::consts::TAU)
        } else {
            (start_angle - angle).rem_euclid(std::f64::consts::TAU)
        };
        let fraction = traveled / sweep.abs();
        let minimum = (2.0 / self.zoom.max(0.2)) as f64 / (radius as f64 * sweep.abs());
        if fraction <= minimum
            || fraction >= 1.0 - minimum
            || (click.distance(center) - radius).abs() > 8.0 / self.zoom.max(0.2)
        {
            return Err("Choose an interior point on the arc".into());
        }
        let first_sweep = sweep * fraction;
        let second_start = start_angle + first_sweep;
        self.history.push(self.snapshot());
        self.future.clear();
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::Arc {
            id,
            center,
            radius,
            start_angle,
            sweep: first_sweep,
            segments,
        };
        self.shapes.insert(
            index + 1,
            Shape::Arc {
                id: new_id,
                center,
                radius,
                start_angle: second_start,
                sweep: sweep - first_sweep,
                segments,
            },
        );
        if let Some(position) = self.fixed_anchors.remove(&(id, 2)) {
            self.fixed_anchors.insert((new_id, 2), position);
        }
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.selected = Some(new_id);
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(new_id)
    }

    pub fn split_selected_circle(&mut self, first: Point, second: Point) -> Result<u32, String> {
        let id = self.selected.ok_or("Select a circle to split")?;
        if self.internal_tangent_member(id)
            || self.midpoint_links.values().any(|source| *source == id)
            || self.point_curve_links.values().any(|source| *source == id)
            || self.normal_links.contains_key(&id)
            || self.normal_links.values().any(|(source, _)| *source == id)
        {
            return Err("Remove dependent relation before splitting".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected circle is missing")?;
        let Shape::Circle {
            center,
            radius,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select a circle to split".into());
        };
        if !radius.is_finite()
            || radius <= 0.0
            || ![first.x, first.y, second.x, second.y]
                .into_iter()
                .all(f32::is_finite)
        {
            return Err("Circle or split positions are invalid".into());
        }
        let tolerance = 8.0 / self.zoom.max(0.2);
        if [first, second]
            .into_iter()
            .any(|point| (point.distance(center) - radius).abs() > tolerance)
        {
            return Err("Choose two points on the circle".into());
        }
        let tau = std::f64::consts::TAU;
        let angle = |point: Point| {
            ((point.y - center.y) as f64)
                .atan2((point.x - center.x) as f64)
                .rem_euclid(tau)
        };
        let (a, b) = (angle(first), angle(second));
        let (start, end) = if a < b { (a, b) } else { (b, a) };
        let minimum = (2.0 / self.zoom.max(0.2)) as f64 / radius as f64;
        if end - start <= minimum || tau - (end - start) <= minimum {
            return Err("Choose two distinct split points".into());
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.radius_differences
            .retain(|target, (source, _)| *target != id && *source != id);
        let new_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.shapes[index] = Shape::Arc {
            id,
            center,
            radius,
            start_angle: start,
            sweep: end - start,
            segments,
        };
        self.shapes.insert(
            index + 1,
            Shape::Arc {
                id: new_id,
                center,
                radius,
                start_angle: end,
                sweep: start + tau - end,
                segments,
            },
        );
        if self.construction.contains(&id) {
            self.construction.insert(new_id);
        }
        self.driving_dimensions
            .remove(&(id, DimensionField::CircleDiameter));
        self.selected = Some(new_id);
        Ok(new_id)
    }

    pub fn trim_selected_line(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select a line to trim")?;
        if self.equal_lines.contains_key(&id)
            || self.equal_lines.values().any(|source| *source == id)
            || self.line_relations.contains_key(&id)
            || self
                .line_relations
                .values()
                .any(|(source, _)| *source == id)
        {
            return Err("Remove Equal before trimming the line".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected line is missing")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("Select a line to trim".into());
        };
        if !click.x.is_finite() || !click.y.is_finite() {
            return Err("Trim position must be finite".into());
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length_squared = dx * dx + dy * dy;
        if length_squared <= 1e-12 {
            return Err("Cannot trim a zero-length line".into());
        }
        let parameter =
            |point: Point| ((point.x - a.x) * dx + (point.y - a.y) * dy) / length_squared;
        let t = parameter(click);
        let projected = Point {
            x: a.x + t * dx,
            y: a.y + t * dy,
        };
        if !(0.0..=1.0).contains(&t) || click.distance(projected) > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected line".into());
        }
        let mut cuts: Vec<f32> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(parameter)
            .filter(|cut| cut.is_finite() && *cut > 1e-5 && *cut < 1.0 - 1e-5)
            .collect();
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-5);
        let lower = cuts
            .iter()
            .copied()
            .filter(|cut| *cut < t)
            .next_back()
            .unwrap_or(0.0);
        let upper = cuts.iter().copied().find(|cut| *cut > t).unwrap_or(1.0);
        let intervals: Vec<(f32, f32)> = [(0.0, lower), (upper, 1.0)]
            .into_iter()
            .filter(|(start, end)| (end - start) * length_squared.sqrt() > 1e-5)
            .collect();
        if intervals.is_empty() {
            self.delete_selected();
            return Ok(0);
        }
        let before = self.snapshot();
        let old_start_link = self.point_links.remove(&(id, 0));
        let old_end_link = self.point_links.remove(&(id, 1));
        let old_start_fixed = self.fixed_anchors.remove(&(id, 0));
        let old_end_fixed = self.fixed_anchors.remove(&(id, 1));
        let axis = self.axis_constraints.get(&id).copied();
        let construction = self.construction.contains(&id);
        self.driving_dimensions
            .remove(&(id, DimensionField::LineLength));
        let mut pieces = Vec::new();
        for (piece_index, (start, end)) in intervals.iter().copied().enumerate() {
            let piece_id = if piece_index == 0 {
                id
            } else {
                let next = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                next
            };
            let at = |fraction: f32| Point {
                x: a.x + fraction * dx,
                y: a.y + fraction * dy,
            };
            pieces.push(Shape::Line {
                id: piece_id,
                a: at(start),
                b: at(end),
            });
            if construction {
                self.construction.insert(piece_id);
            }
            if let Some(axis) = axis {
                self.axis_constraints.insert(piece_id, axis);
            }
            if start == 0.0 {
                if let Some(point_id) = old_start_link {
                    self.point_links.insert((piece_id, 0), point_id);
                }
                if let Some(position) = old_start_fixed {
                    self.fixed_anchors.insert((piece_id, 0), position);
                }
            }
            if end == 1.0 {
                if let Some(point_id) = old_end_link {
                    self.point_links.insert((piece_id, 1), point_id);
                }
                if let Some(position) = old_end_fixed {
                    self.fixed_anchors.insert((piece_id, 1), position);
                }
            }
        }
        self.shapes.splice(index..=index, pieces);
        self.history.push(before);
        self.future.clear();
        self.selected = Some(id);
        Ok(intervals.len())
    }

    pub fn trim_selected_curve(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select a curve to trim")?;
        if self.internal_tangent_member(id)
            || self.midpoint_links.values().any(|source| *source == id)
            || self.point_curve_links.values().any(|source| *source == id)
            || self.normal_links.contains_key(&id)
            || self.normal_links.values().any(|(source, _)| *source == id)
        {
            return Err("Remove dependent relation before trimming".into());
        }
        match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Line { .. }) => self.trim_selected_line(click),
            Some(Shape::Arc { .. }) => self.trim_selected_arc(click),
            Some(Shape::Circle { .. }) => self.trim_selected_circle(click),
            Some(Shape::Ellipse { .. }) => self.trim_selected_ellipse(click),
            Some(Shape::EllipticArc { .. }) => self.trim_selected_elliptic_arc(click),
            Some(Shape::Spline { .. }) => self.trim_selected_spline(click),
            _ => Err("Select a curve to trim".into()),
        }
    }

    pub fn trim_selected_spline(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select a spline to trim")?;
        if self.has_fixed_anchor(id) {
            return Err("Remove fixed spline points before trimming".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected spline is missing")?;
        let Shape::Spline {
            mode,
            ref points,
            closed,
            ..
        } = self.shapes[index]
        else {
            return Err("Select a spline to trim".into());
        };
        let curves = spline::beziers(mode, points, closed)?;
        let target = spline::nearest(&curves, click)?;
        if target.distance > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected spline".into());
        }
        let position = target.segment as f32 + target.t;
        let boundary_hits: Vec<Vec<(usize, f32, Point)>> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .map(|shape| match shape {
                Shape::Spline {
                    mode,
                    points,
                    closed,
                    ..
                } => {
                    let other = spline::beziers(*mode, points, *closed)?;
                    spline::spline_intersections(&curves, &other)
                }
                _ => Ok(spline_boundary_intersections(&curves, shape)),
            })
            .collect::<Result<_, String>>()?;
        let mut cuts: Vec<f32> = boundary_hits
            .into_iter()
            .flatten()
            .map(|(segment, t, _)| segment as f32 + t)
            .filter(|cut| *cut > 1e-5 && *cut < curves.len() as f32 - 1e-5)
            .collect();
        cuts.sort_by(f32::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
        let length = curves.len() as f32;
        let intervals = if closed {
            if cuts.len() < 2 {
                Vec::new()
            } else {
                let lower = cuts
                    .iter()
                    .copied()
                    .filter(|cut| *cut <= position)
                    .next_back()
                    .unwrap_or(cuts[cuts.len() - 1] - length);
                let upper = cuts
                    .iter()
                    .copied()
                    .find(|cut| *cut > position)
                    .unwrap_or(cuts[0] + length);
                vec![(upper, lower + length)]
            }
        } else {
            let lower = cuts
                .iter()
                .copied()
                .filter(|cut| *cut < position)
                .next_back()
                .unwrap_or(0.0);
            let upper = cuts
                .iter()
                .copied()
                .find(|cut| *cut > position)
                .unwrap_or(length);
            [(0.0, lower), (upper, length)]
                .into_iter()
                .filter(|(a, b)| b - a > 1e-5)
                .collect()
        };
        if intervals.is_empty() {
            self.delete_selected();
            return Ok(0);
        }
        let before = self.snapshot();
        let mut pieces = Vec::new();
        for (piece_index, (start, end)) in intervals.iter().copied().enumerate() {
            let points = spline::portion(&curves, start, end, closed)?;
            let piece_id = if piece_index == 0 {
                id
            } else {
                let next = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                next
            };
            pieces.push(Shape::Spline {
                id: piece_id,
                mode: SplineMode::Control,
                points,
                closed: false,
            });
            if self.construction.contains(&id) {
                self.construction.insert(piece_id);
            }
        }
        let start_link = self.point_links.remove(&(id, 0));
        let end_link = self.point_links.remove(&(id, 1));
        for (piece, &(start, end)) in pieces.iter().zip(intervals.iter()) {
            if start == 0.0 {
                if let Some(point_id) = start_link {
                    self.point_links.insert((piece.id(), 0), point_id);
                }
            }
            if end == length {
                if let Some(point_id) = end_link {
                    self.point_links.insert((piece.id(), 1), point_id);
                }
            }
        }
        self.shapes.splice(index..=index, pieces);
        self.history.push(before);
        self.future.clear();
        self.selected_spline_point = None;
        self.selected = Some(id);
        Ok(intervals.len())
    }

    pub fn trim_selected_ellipse(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select an ellipse to trim")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected ellipse is missing")?;
        let Shape::Ellipse {
            center,
            rx,
            ry,
            rotation,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an ellipse to trim".into());
        };
        if ![click.x, click.y, center.x, center.y, rx, ry, rotation]
            .into_iter()
            .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
        {
            return Err("Ellipse or trim position is invalid".into());
        }
        let angle = |point| Shape::ellipse_parameter_angle(center, rx, ry, rotation, point);
        let clicked = angle(click);
        if click.distance(Shape::elliptic_arc_point(
            center, rx, ry, rotation, clicked, 0.0, 0.0,
        )) > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected ellipse".into());
        }
        let tau = std::f64::consts::TAU;
        let mut cuts: Vec<f64> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(angle)
            .filter(|value| value.is_finite())
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-8);
        if cuts.len() < 2 {
            self.delete_selected();
            return Ok(0);
        }
        let lower = cuts
            .iter()
            .copied()
            .filter(|value| *value <= clicked)
            .next_back()
            .unwrap_or(cuts[cuts.len() - 1] - tau);
        let upper = cuts
            .iter()
            .copied()
            .find(|value| *value > clicked)
            .unwrap_or(cuts[0] + tau);
        let retained_sweep = lower + tau - upper;
        if retained_sweep <= 1e-8 {
            self.delete_selected();
            return Ok(0);
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::EllipticArc {
            id,
            center,
            rx,
            ry,
            rotation,
            start_angle: upper.rem_euclid(tau),
            sweep: retained_sweep,
            segments,
        };
        self.driving_dimensions
            .remove(&(id, DimensionField::RectangleWidth));
        self.driving_dimensions
            .remove(&(id, DimensionField::RectangleHeight));
        Ok(1)
    }

    pub fn trim_selected_elliptic_arc(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select an elliptic arc to trim")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected elliptic arc is missing")?;
        let Shape::EllipticArc {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an elliptic arc to trim".into());
        };
        if ![click.x, click.y, center.x, center.y, rx, ry, rotation]
            .into_iter()
            .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep == 0.0
        {
            return Err("Elliptic arc or trim position is invalid".into());
        }
        let parameter = |point| {
            Shape::elliptic_arc_progress(
                Shape::ellipse_parameter_angle(center, rx, ry, rotation, point),
                start_angle,
                sweep,
            )
        };
        let t = parameter(click);
        if !(0.0..=1.0).contains(&t)
            || click.distance(Shape::elliptic_arc_point(
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                t,
            )) > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected elliptic arc".into());
        }
        let mut cuts: Vec<f64> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(parameter)
            .filter(|cut| cut.is_finite() && *cut > 1e-8 && *cut < 1.0 - 1e-8)
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-8);
        let lower = cuts
            .iter()
            .copied()
            .filter(|cut| *cut < t)
            .next_back()
            .unwrap_or(0.0);
        let upper = cuts.iter().copied().find(|cut| *cut > t).unwrap_or(1.0);
        let intervals: Vec<(f64, f64)> = [(0.0, lower), (upper, 1.0)]
            .into_iter()
            .filter(|(start, end)| (end - start) * sweep.abs() * rx.min(ry) as f64 > 1e-5)
            .collect();
        if intervals.is_empty() {
            self.delete_selected();
            return Ok(0);
        }
        let before = self.snapshot();
        let construction = self.construction.contains(&id);
        let start_fixed = self.fixed_anchors.remove(&(id, 1));
        let end_fixed = self.fixed_anchors.remove(&(id, 2));
        let mut pieces = Vec::new();
        for (piece_index, (start, end)) in intervals.iter().copied().enumerate() {
            let piece_id = if piece_index == 0 {
                id
            } else {
                let next = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                next
            };
            pieces.push(Shape::EllipticArc {
                id: piece_id,
                center,
                rx,
                ry,
                rotation,
                start_angle: start_angle + sweep * start,
                sweep: sweep * (end - start),
                segments,
            });
            if start == 0.0 {
                if let Some(position) = start_fixed {
                    self.fixed_anchors.insert((piece_id, 1), position);
                }
            }
            if end == 1.0 {
                if let Some(position) = end_fixed {
                    self.fixed_anchors.insert((piece_id, 2), position);
                }
            }
            if construction {
                self.construction.insert(piece_id);
            }
        }
        self.shapes.splice(index..=index, pieces);
        self.history.push(before);
        self.future.clear();
        self.selected = Some(id);
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(intervals.len())
    }

    pub fn trim_selected_circle(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select a circle to trim")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected circle is missing")?;
        let Shape::Circle {
            center,
            radius,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select a circle to trim".into());
        };
        if !click.x.is_finite() || !click.y.is_finite() || !radius.is_finite() || radius <= 0.0 {
            return Err("Circle or trim position is invalid".into());
        }
        if (click.distance(center) - radius).abs() > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected circle".into());
        }
        let tau = std::f64::consts::TAU;
        let angle = |point: Point| {
            ((point.y - center.y) as f64)
                .atan2((point.x - center.x) as f64)
                .rem_euclid(tau)
        };
        let clicked = angle(click);
        let mut cuts: Vec<f64> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(angle)
            .filter(|value| value.is_finite())
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-8);
        if cuts.len() < 2 {
            self.delete_selected();
            return Ok(0);
        }
        let lower = cuts
            .iter()
            .copied()
            .filter(|value| *value <= clicked)
            .next_back()
            .unwrap_or(cuts[cuts.len() - 1] - tau);
        let upper = cuts
            .iter()
            .copied()
            .find(|value| *value > clicked)
            .unwrap_or(cuts[0] + tau);
        let retained_sweep = lower + tau - upper;
        if retained_sweep <= 1e-8 {
            self.delete_selected();
            return Ok(0);
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.radius_differences
            .retain(|target, (source, _)| *target != id && *source != id);
        self.shapes[index] = Shape::Arc {
            id,
            center,
            radius,
            start_angle: upper,
            sweep: retained_sweep,
            segments,
        };
        self.driving_dimensions
            .remove(&(id, DimensionField::CircleDiameter));
        Ok(1)
    }

    pub fn trim_selected_arc(&mut self, click: Point) -> Result<usize, String> {
        let id = self.selected.ok_or("Select an arc to trim")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected arc is missing")?;
        let Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an arc to trim".into());
        };
        if !click.x.is_finite()
            || !click.y.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep.abs() <= 1e-9
        {
            return Err("Arc or trim position is invalid".into());
        }
        let t = arc_progress(click, center, start_angle, sweep);
        if !(0.0..=1.0).contains(&t)
            || (click.distance(center) - radius).abs() > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected arc".into());
        }
        let mut cuts: Vec<f64> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&self.shapes[index], shape))
            .map(|point| arc_progress(point, center, start_angle, sweep))
            .filter(|cut| cut.is_finite() && *cut > 1e-8 && *cut < 1.0 - 1e-8)
            .collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|left, right| (*left - *right).abs() < 1e-8);
        let lower = cuts
            .iter()
            .copied()
            .filter(|cut| *cut < t)
            .next_back()
            .unwrap_or(0.0);
        let upper = cuts.iter().copied().find(|cut| *cut > t).unwrap_or(1.0);
        let intervals: Vec<(f64, f64)> = [(0.0, lower), (upper, 1.0)]
            .into_iter()
            .filter(|(start, end)| (end - start) * sweep.abs() * radius as f64 > 1e-5)
            .collect();
        if intervals.is_empty() {
            self.delete_selected();
            return Ok(0);
        }
        let before = self.snapshot();
        let construction = self.construction.contains(&id);
        let start_fixed = self.fixed_anchors.remove(&(id, 1));
        let end_fixed = self.fixed_anchors.remove(&(id, 2));
        let mut pieces = Vec::new();
        for (piece_index, (start, end)) in intervals.iter().copied().enumerate() {
            let piece_id = if piece_index == 0 {
                id
            } else {
                let next = self.next_id;
                self.next_id = self.next_id.saturating_add(1);
                next
            };
            pieces.push(Shape::Arc {
                id: piece_id,
                center,
                radius,
                start_angle: start_angle + sweep * start,
                sweep: sweep * (end - start),
                segments,
            });
            if start == 0.0 {
                if let Some(position) = start_fixed {
                    self.fixed_anchors.insert((piece_id, 1), position);
                }
            }
            if end == 1.0 {
                if let Some(position) = end_fixed {
                    self.fixed_anchors.insert((piece_id, 2), position);
                }
            }
            if construction {
                self.construction.insert(piece_id);
            }
        }
        self.shapes.splice(index..=index, pieces);
        self.history.push(before);
        self.future.clear();
        self.selected = Some(id);
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(intervals.len())
    }

    pub fn extend_selected_line(&mut self, click: Point) -> Result<Point, String> {
        let id = self.selected.ok_or("Select a line to extend")?;
        if self.equal_lines.contains_key(&id)
            || self.equal_lines.values().any(|source| *source == id)
            || self.line_relations.contains_key(&id)
            || self
                .line_relations
                .values()
                .any(|(source, _)| *source == id)
        {
            return Err("Remove Equal before extending the line".into());
        }
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected line is missing")?;
        let Shape::Line { a, b, .. } = self.shapes[index] else {
            return Err("Select a line to extend".into());
        };
        if !click.x.is_finite() || !click.y.is_finite() {
            return Err("Extend position must be finite".into());
        }
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let length = dx.hypot(dy);
        if length <= 1e-6 {
            return Err("Cannot extend a zero-length line".into());
        }
        let t = ((click.x - a.x) * dx + (click.y - a.y) * dy) / (length * length);
        let projected = Point {
            x: a.x + t * dx,
            y: a.y + t * dy,
        };
        if !(0.0..=1.0).contains(&t) || click.distance(projected) > 8.0 / self.zoom.max(0.2) {
            return Err("Move the pointer onto the selected line".into());
        }
        let start = t < 0.5;
        if self
            .fixed_anchors
            .contains_key(&(id, if start { 0 } else { 1 }))
        {
            return Err("line endpoint is fixed".into());
        }
        let endpoint = if start { a } else { b };
        let direction = if start {
            Point {
                x: -dx / length,
                y: -dy / length,
            }
        } else {
            Point {
                x: dx / length,
                y: dy / length,
            }
        };
        let others: Vec<_> = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .collect();
        let reach = others
            .iter()
            .filter_map(|shape| shape.bounds())
            .flat_map(|(min, max)| {
                [
                    min,
                    max,
                    Point { x: min.x, y: max.y },
                    Point { x: max.x, y: min.y },
                ]
            })
            .map(|point| endpoint.distance(point))
            .fold(0.0_f32, f32::max)
            + length
            + 1.0;
        if !reach.is_finite() || others.is_empty() {
            return Err("No boundary found beyond this endpoint".into());
        }
        let far = Point {
            x: endpoint.x + direction.x * reach,
            y: endpoint.y + direction.y * reach,
        };
        let ray = Shape::Line {
            id: 0,
            a: endpoint,
            b: far,
        };
        let target = others
            .into_iter()
            .flat_map(|shape| finite_intersections(&ray, shape))
            .filter(|point| point.distance(endpoint) > 1e-5)
            .min_by(|left, right| left.distance(endpoint).total_cmp(&right.distance(endpoint)))
            .ok_or("No boundary found beyond this endpoint")?;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = if start {
            Shape::Line { id, a: target, b }
        } else {
            Shape::Line { id, a, b: target }
        };
        self.point_links.remove(&(id, if start { 0 } else { 1 }));
        self.driving_dimensions
            .remove(&(id, DimensionField::LineLength));
        Ok(target)
    }

    pub fn extend_selected_curve(&mut self, click: Point) -> Result<Point, String> {
        let id = self.selected.ok_or("Select an open curve to extend")?;
        if self.internal_tangent_member(id)
            || self.midpoint_links.values().any(|source| *source == id)
            || self.point_curve_links.values().any(|source| *source == id)
            || self.normal_links.contains_key(&id)
            || self.normal_links.values().any(|(source, _)| *source == id)
        {
            return Err("Remove dependent relation before extending".into());
        }
        match self.shapes.iter().find(|shape| shape.id() == id) {
            Some(Shape::Line { .. }) => self.extend_selected_line(click),
            Some(Shape::Arc { .. }) => self.extend_selected_arc(click),
            Some(Shape::EllipticArc { .. }) => self.extend_selected_elliptic_arc(click),
            _ => Err("Select an open curve to extend".into()),
        }
    }

    pub fn extend_selected_elliptic_arc(&mut self, click: Point) -> Result<Point, String> {
        let id = self.selected.ok_or("Select an elliptic arc to extend")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected elliptic arc is missing")?;
        let Shape::EllipticArc {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an elliptic arc to extend".into());
        };
        if ![click.x, click.y, center.x, center.y, rx, ry, rotation]
            .into_iter()
            .all(f32::is_finite)
            || rx <= 0.0
            || ry <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep == 0.0
            || sweep.abs() >= std::f64::consts::TAU
        {
            return Err("Elliptic arc or extend position is invalid".into());
        }
        let t = Shape::elliptic_arc_progress(
            Shape::ellipse_parameter_angle(center, rx, ry, rotation, click),
            start_angle,
            sweep,
        );
        if !(0.0..=1.0).contains(&t)
            || click.distance(Shape::elliptic_arc_point(
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                t,
            )) > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected elliptic arc".into());
        }
        let start = t < 0.5;
        if self
            .fixed_anchors
            .contains_key(&(id, if start { 1 } else { 2 }))
        {
            return Err("arc endpoint is fixed".into());
        }
        let direction = sweep.signum();
        let endpoint_angle = if start {
            start_angle
        } else {
            start_angle + sweep
        };
        let max_extra = std::f64::consts::TAU - sweep.abs();
        if max_extra <= 1e-8 {
            return Err("Elliptic arc is already a full ellipse".into());
        }
        let ellipse = Shape::Ellipse {
            id: 0,
            center,
            rx,
            ry,
            rotation,
            segments,
        };
        let target = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&ellipse, shape))
            .filter_map(|point| {
                let angle = Shape::ellipse_parameter_angle(center, rx, ry, rotation, point);
                let delta = if start {
                    (direction * (endpoint_angle - angle)).rem_euclid(std::f64::consts::TAU)
                } else {
                    (direction * (angle - endpoint_angle)).rem_euclid(std::f64::consts::TAU)
                };
                (delta > 1e-6 && delta <= max_extra + 1e-8).then_some((delta, point))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .ok_or("No boundary found beyond this endpoint")?;
        let extra = direction * target.0;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::EllipticArc {
            id,
            center,
            rx,
            ry,
            rotation,
            start_angle: if start {
                start_angle - extra
            } else {
                start_angle
            },
            sweep: sweep + extra,
            segments,
        };
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(target.1)
    }

    pub fn extend_selected_arc(&mut self, click: Point) -> Result<Point, String> {
        let id = self.selected.ok_or("Select an arc to extend")?;
        let index = self
            .shapes
            .iter()
            .position(|shape| shape.id() == id)
            .ok_or("Selected arc is missing")?;
        let Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            segments,
            ..
        } = self.shapes[index]
        else {
            return Err("Select an arc to extend".into());
        };
        if !click.x.is_finite()
            || !click.y.is_finite()
            || !radius.is_finite()
            || radius <= 0.0
            || !start_angle.is_finite()
            || !sweep.is_finite()
            || sweep.abs() <= 1e-9
        {
            return Err("Arc or extend position is invalid".into());
        }
        let t = arc_progress(click, center, start_angle, sweep);
        if !(0.0..=1.0).contains(&t)
            || (click.distance(center) - radius).abs() > 8.0 / self.zoom.max(0.2)
        {
            return Err("Move the pointer onto the selected arc".into());
        }
        let start = t < 0.5;
        if self
            .fixed_anchors
            .contains_key(&(id, if start { 1 } else { 2 }))
        {
            return Err("arc endpoint is fixed".into());
        }
        let direction = sweep.signum();
        let endpoint_angle = if start {
            start_angle
        } else {
            start_angle + sweep
        };
        let max_extra = std::f64::consts::TAU - sweep.abs();
        if max_extra <= 1e-8 {
            return Err("Arc is already a full circle".into());
        }
        let circle = Shape::Circle {
            id: 0,
            center,
            radius,
            segments,
        };
        let target = self
            .shapes
            .iter()
            .filter(|shape| shape.id() != id)
            .flat_map(|shape| finite_intersections(&circle, shape))
            .filter_map(|point| {
                let angle = ((point.y - center.y) as f64).atan2((point.x - center.x) as f64);
                let delta = if start {
                    (direction * (endpoint_angle - angle)).rem_euclid(std::f64::consts::TAU)
                } else {
                    (direction * (angle - endpoint_angle)).rem_euclid(std::f64::consts::TAU)
                };
                (delta > 1e-6 && delta <= max_extra + 1e-8).then_some((delta, point))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0))
            .ok_or("No boundary found beyond this endpoint")?;
        let extra = direction * target.0;
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = Shape::Arc {
            id,
            center,
            radius,
            start_angle: if start {
                start_angle - extra
            } else {
                start_angle
            },
            sweep: sweep + extra,
            segments,
        };
        self.driving_dimensions
            .remove(&(id, DimensionField::ArcSweep));
        Ok(target.1)
    }

    pub fn copy_selected(&mut self, delta: Point) -> bool {
        if !delta.x.is_finite() || !delta.y.is_finite() {
            return false;
        }
        let Some(source) = self
            .selected
            .and_then(|id| self.shapes.iter().find(|shape| shape.id() == id))
            .cloned()
        else {
            return false;
        };
        let source_id = source.id();
        let id = self.next_id;
        let copied_dimensions: Vec<_> = self
            .driving_dimensions
            .iter()
            .filter(|((shape_id, _), _)| *shape_id == source_id)
            .map(|((_, field), value)| ((*field), *value))
            .collect();
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes.push(source.translated(delta).with_id(id));
        if self.construction.contains(&source_id) {
            self.construction.insert(id);
        }
        if let Some(axis) = self.axis_constraints.get(&source_id).copied() {
            self.axis_constraints.insert(id, axis);
        }
        for (field, value) in copied_dimensions {
            self.driving_dimensions.insert((id, field), value);
        }
        self.selected = Some(id);
        self.selected_spline_point = None;
        self.next_id = self.next_id.saturating_add(1);
        true
    }

    pub fn move_selected(&mut self, delta: Point) -> bool {
        if !delta.x.is_finite() || !delta.y.is_finite() || (delta.x == 0.0 && delta.y == 0.0) {
            return false;
        }
        let Some(index) = self
            .selected
            .and_then(|id| self.shapes.iter().position(|shape| shape.id() == id))
        else {
            return false;
        };
        if self.midpoint_links.contains_key(&self.shapes[index].id()) {
            return false;
        }
        if self.has_fixed_anchor(self.shapes[index].id()) {
            return false;
        }
        if self
            .shared_ellipse_partners(self.shapes[index].id())
            .into_iter()
            .any(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        if self
            .concentric_partner(self.shapes[index].id())
            .is_some_and(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        let mut next = self.shapes[index].translated(delta);
        if let Some(&(source_id, kind)) = self.line_relations.get(&next.id()) {
            if kind == LineRelationKind::Collinear {
                let Some(source) = self.shapes.iter().find(|shape| shape.id() == source_id) else {
                    return false;
                };
                let Some(projected) = Self::line_relation_shape(source, &next, kind) else {
                    return false;
                };
                next = projected;
            }
        }
        self.history.push(self.snapshot());
        self.future.clear();
        self.shapes[index] = next;
        let id = self.shapes[index].id();
        self.solve_or_rollback(&[Pin::Shape(id)]).is_ok()
    }

    pub fn scale_selected(&mut self, factor: f32, center: Point, copy: bool) -> bool {
        let Some(index) = self
            .selected
            .and_then(|id| self.shapes.iter().position(|shape| shape.id() == id))
        else {
            return false;
        };
        let source = &self.shapes[index];
        if !copy && self.midpoint_links.contains_key(&source.id()) {
            return false;
        }
        if !copy && self.has_fixed_anchor(source.id()) {
            return false;
        }
        if !copy
            && self
                .shared_ellipse_partners(source.id())
                .into_iter()
                .any(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        if !copy
            && self
                .shared_ellipse_partners(source.id())
                .into_iter()
                .any(|partner| {
                    [
                        DimensionField::RectangleWidth,
                        DimensionField::RectangleHeight,
                    ]
                    .into_iter()
                    .any(|field| self.driving_dimensions.contains_key(&(partner, field)))
                })
        {
            return false;
        }
        if !copy
            && self
                .concentric_partner(source.id())
                .is_some_and(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        let Some(mut scaled) = source.scaled_about(factor, center) else {
            return false;
        };
        if !copy {
            if let Some(&(source_id, kind)) = self.line_relations.get(&source.id()) {
                if kind == LineRelationKind::Collinear {
                    let Some(relation_source) =
                        self.shapes.iter().find(|shape| shape.id() == source_id)
                    else {
                        return false;
                    };
                    let Some(projected) = Self::line_relation_shape(relation_source, &scaled, kind)
                    else {
                        return false;
                    };
                    scaled = projected;
                }
            }
        }
        if scaled == *source {
            return false;
        }
        let id = source.id();
        let dimensions: Vec<_> = self
            .driving_dimensions
            .iter()
            .filter(|((shape_id, _), _)| *shape_id == id)
            .map(|(key, value)| {
                (
                    *key,
                    if matches!(key.1, DimensionField::ArcSweep | DimensionField::Rotation) {
                        *value
                    } else {
                        *value * factor
                    },
                )
            })
            .collect();
        if dimensions.iter().any(|(_, value)| !value.is_finite()) {
            return false;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if copy {
            let new_id = self.next_id;
            self.shapes.push(scaled.with_id(new_id));
            if self.construction.contains(&id) {
                self.construction.insert(new_id);
            }
            if let Some(axis) = self.axis_constraints.get(&id).copied() {
                self.axis_constraints.insert(new_id, axis);
            }
            for ((_, field), value) in dimensions {
                self.driving_dimensions.insert((new_id, field), value);
            }
            self.selected = Some(new_id);
            self.next_id = self.next_id.saturating_add(1);
        } else {
            self.shapes[index] = scaled;
            for (key, value) in dimensions {
                self.driving_dimensions.insert(key, value);
            }
        }
        let pinned = if copy {
            self.selected.unwrap_or(id)
        } else {
            id
        };
        self.solve_or_rollback(&[Pin::Shape(pinned)]).is_ok()
    }

    pub fn rotate_selected(&mut self, degrees: f32, center: Point, copy: bool) -> bool {
        if !copy && degrees % 360.0 == 0.0 {
            return false;
        }
        let Some(index) = self
            .selected
            .and_then(|id| self.shapes.iter().position(|shape| shape.id() == id))
        else {
            return false;
        };
        if !copy && self.midpoint_links.contains_key(&self.shapes[index].id()) {
            return false;
        }
        let source = &self.shapes[index];
        if !copy && self.has_fixed_anchor(source.id()) {
            return false;
        }
        if !copy
            && self
                .shared_ellipse_partners(source.id())
                .into_iter()
                .any(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        if !copy
            && self
                .shared_ellipse_partners(source.id())
                .into_iter()
                .any(|partner| {
                    self.driving_dimensions
                        .contains_key(&(partner, DimensionField::Rotation))
                })
        {
            return false;
        }
        if !copy
            && self
                .concentric_partner(source.id())
                .is_some_and(|partner| self.has_fixed_anchor(partner))
        {
            return false;
        }
        let Some(rotated) = source.rotated_about(degrees, center) else {
            return false;
        };
        if !copy && rotated == *source {
            return false;
        }
        let source_id = source.id();
        let axis_after_rotation = if let Some(axis) = self.axis_constraints.get(&source_id).copied()
        {
            let quarter_turns = degrees / 90.0;
            if (quarter_turns - quarter_turns.round()).abs() > 1e-4 {
                return false;
            }
            if quarter_turns.round() as i32 % 2 == 0 {
                Some(axis)
            } else {
                Some(match axis {
                    AxisConstraint::Horizontal => AxisConstraint::Vertical,
                    AxisConstraint::Vertical => AxisConstraint::Horizontal,
                })
            }
        } else {
            None
        };
        let copied_dimensions: Vec<_> = self
            .driving_dimensions
            .iter()
            .filter(|((id, _), _)| *id == source_id)
            .map(|((_, field), value)| {
                (
                    *field,
                    if *field == DimensionField::Rotation {
                        *value + degrees
                    } else {
                        *value
                    },
                )
            })
            .collect();
        if copied_dimensions
            .iter()
            .any(|(_, value)| !value.is_finite())
        {
            return false;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        if copy {
            let id = self.next_id;
            self.shapes.push(rotated.with_id(id));
            if self.construction.contains(&source_id) {
                self.construction.insert(id);
            }
            if let Some(axis) = axis_after_rotation {
                self.axis_constraints.insert(id, axis);
            }
            for (field, value) in copied_dimensions {
                self.driving_dimensions.insert((id, field), value);
            }
            self.selected = Some(id);
            self.next_id = self.next_id.saturating_add(1);
        } else {
            self.shapes[index] = rotated;
            if let Some(axis) = axis_after_rotation {
                self.axis_constraints.insert(source_id, axis);
            }
            for (field, value) in copied_dimensions {
                self.driving_dimensions.insert((source_id, field), value);
            }
        }
        let pinned = if copy {
            self.selected.unwrap_or(source_id)
        } else {
            source_id
        };
        self.solve_or_rollback(&[Pin::Shape(pinned)]).is_ok()
    }

    pub fn undo(&mut self) -> bool {
        if self.drag.is_some() {
            return self.cancel_drag();
        }
        let Some(previous) = self.history.pop() else {
            return false;
        };
        self.future.push(self.snapshot());
        self.restore(previous);
        true
    }

    pub fn redo(&mut self) -> bool {
        if self.drag.is_some() {
            return self.cancel_drag();
        }
        let Some(next) = self.future.pop() else {
            return false;
        };
        self.history.push(self.snapshot());
        self.restore(next);
        true
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.future.clear();
        self.selected = None;
        self.drag = None;
    }

    pub fn toggle_construction(&mut self) -> Option<bool> {
        let id = self.selected?;
        if !self.shapes.iter().any(|shape| shape.id() == id) {
            return None;
        }
        self.history.push(self.snapshot());
        self.future.clear();
        let enabled = if self.construction.remove(&id) {
            false
        } else {
            self.construction.insert(id);
            true
        };
        Some(enabled)
    }

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

/// User intent kept exactly while solving: a whole shape or one anchor
/// (line endpoint 0/1, sketch point 0, circle/arc center 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pin {
    Shape(u32),
    Anchor(u32, usize),
}

#[derive(Clone, Debug)]
enum SolverEntity {
    Line(solver::LineVar),
    Point(solver::PointVar),
    Circle(solver::CircleVar),
    Arc(solver::ArcVar),
    Ellipse(solver::EllipseVar),
    Spline(solver::SplineVar),
}

impl SolverEntity {
    fn curve(&self) -> Option<solver::Curve> {
        Some(match self {
            Self::Line(line) => solver::Curve::Line(*line),
            Self::Circle(circle) => solver::Curve::Circle(*circle),
            Self::Arc(arc) => solver::Curve::Arc(*arc),
            Self::Ellipse(ellipse) => solver::Curve::Ellipse(*ellipse),
            Self::Spline(spline) => solver::Curve::Spline(spline.clone()),
            Self::Point(_) => return None,
        })
    }

    fn center(&self) -> Option<solver::PointVar> {
        match self {
            Self::Circle(circle) => Some(circle.center),
            Self::Arc(arc) => Some(arc.circle.center),
            Self::Ellipse(ellipse) => Some(ellipse.center),
            _ => None,
        }
    }
}

/// Builds solver parameters for document shapes on demand.
struct SolverBuilder<'a> {
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
    problem: solver::Problem,
    entities: HashMap<u32, SolverEntity>,
}

impl SolverBuilder<'_> {
    fn param(&mut self, value: f64, locked: bool, weight: f64) -> usize {
        let var = self.problem.param(value, locked);
        self.problem
            .soft
            .push(solver::Soft::Target { var, value, weight });
        var
    }

    fn entity(&mut self, id: u32) -> Option<SolverEntity> {
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
        let point = |builder: &mut Self, at: Point, index: usize, weight: f64| PointVar {
            x: builder.param(at.x as f64, anchor_locked(index), weight * position),
            y: builder.param(at.y as f64, anchor_locked(index), weight * position),
        };
        let created = match shape {
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
            _ => return None,
        };
        self.entities.insert(id, created.clone());
        Some(created)
    }

    fn line(&mut self, id: u32) -> Option<solver::LineVar> {
        match self.entity(id)? {
            SolverEntity::Line(line) => Some(line),
            _ => None,
        }
    }

    fn point(&mut self, id: u32) -> Option<solver::PointVar> {
        match self.entity(id)? {
            SolverEntity::Point(point) => Some(point),
            _ => None,
        }
    }

    /// Circle or arc (radius relations treat both alike).
    fn circle(&mut self, id: u32) -> Option<solver::CircleVar> {
        match self.entity(id)? {
            SolverEntity::Circle(circle) => Some(circle),
            SolverEntity::Arc(arc) => Some(arc.circle),
            _ => None,
        }
    }

    fn ellipse(&mut self, id: u32) -> Option<solver::EllipseVar> {
        match self.entity(id)? {
            SolverEntity::Ellipse(ellipse) => Some(ellipse),
            _ => None,
        }
    }

    fn curve(&mut self, id: u32) -> Option<solver::Curve> {
        self.entity(id)?.curve()
    }

    /// Auxiliary spline parameter for a point that must lie on `curve`.
    fn curve_param(&mut self, curve: &solver::Curve, near: solver::PointVar) -> Option<usize> {
        let solver::Curve::Spline(spline) = curve else {
            return None;
        };
        let p = &self.problem.params;
        let u = solver::spline_parameter(p, spline, (p[near.x], p[near.y]));
        Some(self.param(u, false, 1e-3))
    }
}

impl Document {
    /// Curves whose size (radius, axes) and whose angles (sweep, rotation)
    /// some relation or driving value may change during a solve.
    fn solver_free_curve_parameters(&self) -> (HashSet<u32>, HashSet<u32>) {
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
            .chain(self.endpoint_tangents.keys().copied())
            .collect();
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
    fn projected_targets(&self) -> HashSet<u32> {
        self.point_curve_links
            .keys()
            .chain(self.normal_links.keys())
            .chain(self.endpoint_tangents.keys())
            .chain(self.internal_tangents.keys())
            .chain(self.external_tangents.keys())
            .copied()
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
            .copied()
            .chain(self.projected_targets())
            .collect();
        let sources: Vec<u32> = self
            .shapes
            .iter()
            .map(Shape::id)
            .filter(|id| !followers.contains(id))
            .collect();
        let explicit: HashSet<Pin> = pins.iter().copied().collect();
        let projected = self.projected_targets();
        let softened: HashSet<u32> = explicit
            .iter()
            .filter_map(|pin| match pin {
                Pin::Shape(id) if projected.contains(id) => Some(*id),
                _ => None,
            })
            .collect();
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
                if let (Some(source), Some(index)) = (source, index) {
                    if let Some(next) = Self::line_relation_shape(source, &self.shapes[index], kind)
                    {
                        self.shapes[index] = next;
                    }
                }
            }
        }
        let mut error = String::new();
        for tier in 0..3 {
            let mut tier_pins = explicit.clone();
            match tier {
                0 => tier_pins.extend(sources.iter().map(|&id| Pin::Shape(id))),
                1 => tier_pins.extend(sources.iter().map(|&id| Pin::Anchor(id, 0))),
                _ => {}
            }
            match self.solve_once(&tier_pins, &softened) {
                Ok(()) => return Ok(()),
                Err(message) => error = message,
            }
        }
        self.shapes = original;
        Err(error)
    }

    fn solve_once(&mut self, pins: &HashSet<Pin>, softened: &HashSet<u32>) -> Result<(), String> {
        use solver::{Constraint, Curve, PointVar, Soft, Value};
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
        let (sized, angled) = self.solver_free_curve_parameters();
        let mut b = SolverBuilder {
            shapes: &self.shapes,
            pins,
            softened,
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
        for (&target, &(source, endpoint, line_sign, sweep_sign)) in &self.endpoint_tangents {
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
        if constraints.is_empty() {
            return Ok(());
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
        let SolverBuilder {
            mut problem,
            entities,
            ..
        } = b;
        problem.constraints = constraints;
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
        if p.iter().any(|value| !(value.abs() < f32::MAX as f64)) {
            return Err("Solved geometry is out of range".into());
        }
        // Round-off below the solve tolerance around zero is stored as zero.
        let p: Vec<f64> = p
            .iter()
            .map(|&v| if v.abs() < 1e-9 { 0.0 } else { v })
            .collect();
        let p = &p;
        let read = |var: PointVar| Point {
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
                _ => {}
            }
        }
        Ok(())
    }

    /// Run the solver after an edit that already pushed its undo snapshot;
    /// on failure the whole edit is rolled back.
    fn solve_or_rollback(&mut self, pins: &[Pin]) -> Result<(), String> {
        if let Err(error) = self.solve_constraints(pins) {
            if let Some(state) = self.history.pop() {
                let selected = (self.selected, self.selected_spline_point);
                self.restore(state);
                (self.selected, self.selected_spline_point) = selected;
            }
            return Err(error);
        }
        Ok(())
    }
}

fn finite_intersections(first: &Shape, second: &Shape) -> Vec<Point> {
    if let Shape::Spline {
        mode,
        points,
        closed,
        ..
    } = first
    {
        if let Ok(curves) = spline::beziers(*mode, points, *closed) {
            let hits = spline_boundary_intersections(&curves, second);
            if !hits.is_empty()
                || matches!(
                    second,
                    Shape::Circle { .. }
                        | Shape::Arc { .. }
                        | Shape::Ellipse { .. }
                        | Shape::EllipticArc { .. }
                        | Shape::Spline { .. }
                )
            {
                return hits.into_iter().map(|(_, _, point)| point).collect();
            }
        }
    }
    if matches!(second, Shape::Spline { .. })
        && matches!(
            first,
            Shape::Circle { .. }
                | Shape::Arc { .. }
                | Shape::Ellipse { .. }
                | Shape::EllipticArc { .. }
        )
    {
        return finite_intersections(second, first);
    }
    if let (
        Shape::Spline {
            mode,
            points,
            closed,
            ..
        },
        Shape::Line { a, b, .. },
    ) = (first, second)
    {
        return spline::beziers(*mode, points, *closed)
            .map(|curves| {
                spline::line_intersections(&curves, *a, *b)
                    .into_iter()
                    .map(|(_, _, point)| point)
                    .collect()
            })
            .unwrap_or_default();
    }
    if matches!(second, Shape::Spline { .. }) && matches!(first, Shape::Line { .. }) {
        return finite_intersections(second, first);
    }
    if let Shape::EllipticArc {
        center,
        rx,
        ry,
        rotation,
        start_angle,
        sweep,
        segments,
        ..
    } = first
    {
        let ellipse = Shape::Ellipse {
            id: 0,
            center: *center,
            rx: *rx,
            ry: *ry,
            rotation: *rotation,
            segments: *segments,
        };
        return finite_intersections(&ellipse, second)
            .into_iter()
            .filter(|point| {
                let angle = Shape::ellipse_parameter_angle(*center, *rx, *ry, *rotation, *point);
                let fraction = Shape::elliptic_arc_progress(angle, *start_angle, *sweep);
                fraction <= 1.0 + 1e-6
                    || point.distance(Shape::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        1.0,
                    )) <= 1e-4
            })
            .collect();
    }
    if matches!(second, Shape::EllipticArc { .. }) {
        return finite_intersections(second, first);
    }
    if let Some(curves) = capsule_boundary(first) {
        let mut contacts = Vec::new();
        for curve in curves {
            for point in finite_intersections(&curve, second) {
                if !contacts
                    .iter()
                    .any(|other: &Point| other.distance(point) < 1e-4)
                {
                    contacts.push(point);
                }
            }
        }
        return contacts;
    }
    if matches!(second, Shape::Capsule { .. }) {
        return finite_intersections(second, first);
    }
    if let Some(edges) = boundary_edges(first) {
        let mut contacts = Vec::new();
        for (a, b) in edges {
            let edge = Shape::Line { id: 0, a, b };
            for point in finite_intersections(&edge, second) {
                if !contacts
                    .iter()
                    .any(|other: &Point| other.distance(point) < 1e-4)
                {
                    contacts.push(point);
                }
            }
        }
        return contacts;
    }
    if boundary_edges(second).is_some() {
        return finite_intersections(second, first);
    }
    if let Shape::Arc {
        center,
        radius,
        start_angle,
        sweep,
        ..
    } = first
    {
        let circle = Shape::Circle {
            id: 0,
            center: *center,
            radius: *radius,
            segments: 64,
        };
        return finite_intersections(&circle, second)
            .into_iter()
            .filter(|point| on_arc_sweep(*point, *center, *start_angle, *sweep))
            .collect();
    }
    if matches!(second, Shape::Arc { .. }) {
        return finite_intersections(second, first);
    }
    match (first, second) {
        (Shape::Line { a, b, .. }, Shape::Line { a: c, b: d, .. }) => {
            let (rx, ry) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
            let (sx, sy) = ((d.x - c.x) as f64, (d.y - c.y) as f64);
            let cross = rx * sy - ry * sx;
            if cross.abs() <= 1e-12 {
                return Vec::new();
            }
            let (qx, qy) = ((c.x - a.x) as f64, (c.y - a.y) as f64);
            let t = (qx * sy - qy * sx) / cross;
            let u = (qx * ry - qy * rx) / cross;
            if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
                return Vec::new();
            }
            vec![Point {
                x: (a.x as f64 + t * rx) as f32,
                y: (a.y as f64 + t * ry) as f32,
            }]
        }
        (Shape::Line { a, b, .. }, Shape::Circle { center, radius, .. })
        | (Shape::Circle { center, radius, .. }, Shape::Line { a, b, .. }) => {
            let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
            let length_squared = dx * dx + dy * dy;
            if length_squared <= 1e-12 || !radius.is_finite() || *radius <= 0.0 {
                return Vec::new();
            }
            let (ox, oy) = ((a.x - center.x) as f64, (a.y - center.y) as f64);
            let projection = -(ox * dx + oy * dy) / length_squared;
            let closest_squared = (ox + projection * dx).powi(2) + (oy + projection * dy).powi(2);
            let offset_squared = ((*radius as f64).powi(2) - closest_squared) / length_squared;
            if offset_squared < -1e-12 {
                return Vec::new();
            }
            let offset = offset_squared.max(0.0).sqrt();
            let mut result = Vec::new();
            for t in [projection - offset, projection + offset] {
                if (0.0..=1.0).contains(&t)
                    && result.iter().all(|p: &Point| {
                        (p.x as f64 - (a.x as f64 + t * dx)).abs() > 1e-5
                            || (p.y as f64 - (a.y as f64 + t * dy)).abs() > 1e-5
                    })
                {
                    result.push(Point {
                        x: (a.x as f64 + t * dx) as f32,
                        y: (a.y as f64 + t * dy) as f32,
                    });
                }
            }
            result
        }
        (
            Shape::Line { a, b, .. },
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            },
        )
        | (
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                ..
            },
            Shape::Line { a, b, .. },
        ) => {
            if !rx.is_finite()
                || !ry.is_finite()
                || !rotation.is_finite()
                || *rx <= 0.0
                || *ry <= 0.0
            {
                return Vec::new();
            }
            let (sin, cos) = (*rotation as f64).to_radians().sin_cos();
            let local = |point: Point| {
                let (x, y) = ((point.x - center.x) as f64, (point.y - center.y) as f64);
                (
                    (x * cos + y * sin) / *rx as f64,
                    (-x * sin + y * cos) / *ry as f64,
                )
            };
            let (ax, ay) = local(*a);
            let (bx, by) = local(*b);
            let (dx, dy) = (bx - ax, by - ay);
            let length_squared = dx * dx + dy * dy;
            if length_squared <= 1e-18 {
                return Vec::new();
            }
            let projection = -(ax * dx + ay * dy) / length_squared;
            let closest_squared = (ax + projection * dx).powi(2) + (ay + projection * dy).powi(2);
            let offset_squared = (1.0 - closest_squared) / length_squared;
            if offset_squared < -1e-12 {
                return Vec::new();
            }
            let offset = offset_squared.max(0.0).sqrt();
            let mut contacts = Vec::new();
            for t in [projection - offset, projection + offset] {
                if (0.0..=1.0).contains(&t) {
                    let point = Point {
                        x: (a.x as f64 + t * (b.x - a.x) as f64) as f32,
                        y: (a.y as f64 + t * (b.y - a.y) as f64) as f32,
                    };
                    if !contacts
                        .iter()
                        .any(|other: &Point| other.distance(point) < 1e-5)
                    {
                        contacts.push(point);
                    }
                }
            }
            contacts
        }
        (
            Shape::Circle { center, radius, .. },
            Shape::Ellipse {
                center: ellipse_center,
                rx,
                ry,
                rotation,
                ..
            },
        )
        | (
            Shape::Ellipse {
                center: ellipse_center,
                rx,
                ry,
                rotation,
                ..
            },
            Shape::Circle { center, radius, .. },
        ) => circle_ellipse_intersections(*center, *radius, *ellipse_center, *rx, *ry, *rotation),
        (Shape::Ellipse { .. }, Shape::Ellipse { .. }) => {
            ellipse_ellipse_intersections(first, second)
        }
        (
            Shape::Circle {
                center: a,
                radius: ra,
                ..
            },
            Shape::Circle {
                center: b,
                radius: rb,
                ..
            },
        ) => {
            let (dx, dy) = ((b.x - a.x) as f64, (b.y - a.y) as f64);
            let distance = dx.hypot(dy);
            let (ra, rb) = (*ra as f64, *rb as f64);
            if !distance.is_finite()
                || !ra.is_finite()
                || !rb.is_finite()
                || ra <= 0.0
                || rb <= 0.0
                || distance <= 1e-12
                || distance > ra + rb + 1e-9
                || distance < (ra - rb).abs() - 1e-9
            {
                return Vec::new();
            }
            let along = (ra * ra - rb * rb + distance * distance) / (2.0 * distance);
            let perpendicular = (ra * ra - along * along).max(0.0).sqrt();
            let (mx, my) = (
                a.x as f64 + along * dx / distance,
                a.y as f64 + along * dy / distance,
            );
            let offset = (
                perpendicular * -dy / distance,
                perpendicular * dx / distance,
            );
            let mut result = vec![Point {
                x: (mx + offset.0) as f32,
                y: (my + offset.1) as f32,
            }];
            if perpendicular > 1e-6 {
                result.push(Point {
                    x: (mx - offset.0) as f32,
                    y: (my - offset.1) as f32,
                });
            }
            result
        }
        _ => Vec::new(),
    }
}

fn spline_boundary_intersections(
    curves: &[spline::Cubic],
    boundary: &Shape,
) -> Vec<(usize, f32, Point)> {
    match boundary {
        Shape::Line { a, b, .. } => spline::line_intersections(curves, *a, *b),
        Shape::Circle { center, radius, .. } => {
            spline::circle_intersections(curves, *center, *radius)
        }
        Shape::Arc {
            center,
            radius,
            start_angle,
            sweep,
            ..
        } => spline::circle_intersections(curves, *center, *radius)
            .into_iter()
            .filter(|(_, _, point)| on_arc_sweep(*point, *center, *start_angle, *sweep))
            .collect(),
        Shape::Ellipse {
            center,
            rx,
            ry,
            rotation,
            ..
        } => spline::ellipse_intersections(curves, *center, *rx, *ry, *rotation),
        Shape::EllipticArc {
            center,
            rx,
            ry,
            rotation,
            start_angle,
            sweep,
            ..
        } => spline::ellipse_intersections(curves, *center, *rx, *ry, *rotation)
            .into_iter()
            .filter(|(_, _, point)| {
                let angle = Shape::ellipse_parameter_angle(*center, *rx, *ry, *rotation, *point);
                Shape::elliptic_arc_progress(angle, *start_angle, *sweep) <= 1.0 + 1e-6
            })
            .collect(),
        Shape::Spline {
            mode,
            points,
            closed,
            ..
        } => spline::beziers(*mode, points, *closed)
            .and_then(|other| spline::spline_intersections(curves, &other))
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn bounded_polynomial_roots(input: &[f64]) -> Vec<f64> {
    let scale = input
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()));
    if !scale.is_finite() || scale < 1e-14 {
        return Vec::new();
    }
    let mut coefficients: Vec<f64> = input.iter().map(|value| value / scale).collect();
    while coefficients.len() > 1 && coefficients.last().is_some_and(|value| value.abs() < 1e-14) {
        coefficients.pop();
    }
    if coefficients.len() < 2 {
        return Vec::new();
    }
    let evaluate = |x: f64| {
        coefficients
            .iter()
            .rev()
            .fold(0.0, |value, coefficient| value * x + coefficient)
    };
    let derivative: Vec<f64> = coefficients
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, value)| value * index as f64)
        .collect();
    let critical = if coefficients.len() == 2 {
        Vec::new()
    } else {
        bounded_polynomial_roots(&derivative)
    };
    let mut divisions = vec![-1.0];
    divisions.extend(
        critical
            .into_iter()
            .filter(|value| *value > -1.0 && *value < 1.0),
    );
    divisions.push(1.0);
    divisions.sort_by(f64::total_cmp);
    let mut roots: Vec<f64> = divisions
        .iter()
        .copied()
        .filter(|value| evaluate(*value).abs() < 1e-12)
        .collect();
    for pair in divisions.windows(2) {
        let (mut lo, mut hi) = (pair[0], pair[1]);
        let mut low_value = evaluate(lo);
        let high_value = evaluate(hi);
        if low_value.abs() < 1e-12 || high_value.abs() < 1e-12 || low_value * high_value >= 0.0 {
            continue;
        }
        for _ in 0..70 {
            let mid = (lo + hi) * 0.5;
            let value = evaluate(mid);
            if value == 0.0 {
                lo = mid;
                hi = mid;
                break;
            }
            if low_value * value <= 0.0 {
                hi = mid;
            } else {
                lo = mid;
                low_value = value;
            }
        }
        roots.push((lo + hi) * 0.5);
    }
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
    roots
}

fn circle_ellipse_intersections(
    circle_center: Point,
    circle_radius: f32,
    ellipse_center: Point,
    rx: f32,
    ry: f32,
    rotation: f32,
) -> Vec<Point> {
    if ![circle_radius, rx, ry]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
        || !rotation.is_finite()
    {
        return Vec::new();
    }
    let (rx, ry, radius) = (rx as f64, ry as f64, circle_radius as f64);
    let (sin, cos) = (rotation as f64).to_radians().sin_cos();
    let (dx, dy) = (
        (circle_center.x - ellipse_center.x) as f64,
        (circle_center.y - ellipse_center.y) as f64,
    );
    if dx.hypot(dy) > rx.max(ry) + radius {
        return Vec::new();
    }
    let length = rx.max(ry).max(radius).max(dx.abs()).max(dy.abs());
    let (x, y, r) = (
        (dx * cos + dy * sin) / length,
        (-dx * sin + dy * cos) / length,
        radius / length,
    );
    let mut points = Vec::new();
    for sign in [1.0, -1.0] {
        let a = sign * rx / length - x;
        let b = -sign * rx / length - x;
        let d = 2.0 * sign * ry / length;
        let coefficients = [
            a * a + y * y - r * r,
            -2.0 * y * d,
            2.0 * a * b + d * d + 2.0 * y * y - 2.0 * r * r,
            -2.0 * y * d,
            b * b + y * y - r * r,
        ];
        for t in bounded_polynomial_roots(&coefficients) {
            let px = sign * rx * (1.0 - t * t) / (1.0 + t * t);
            let py = sign * ry * 2.0 * t / (1.0 + t * t);
            let point = Point {
                x: (ellipse_center.x as f64 + px * cos - py * sin) as f32,
                y: (ellipse_center.y as f64 + px * sin + py * cos) as f32,
            };
            if !points
                .iter()
                .any(|other: &Point| other.distance(point) < 1e-5 * rx.max(ry) as f32)
            {
                points.push(point);
            }
        }
    }
    points
}

fn ellipse_ellipse_intersections(first: &Shape, second: &Shape) -> Vec<Point> {
    let (
        Shape::Ellipse {
            center: a,
            rx: arx,
            ry: ary,
            rotation: arotation,
            ..
        },
        Shape::Ellipse {
            center: b,
            rx: brx,
            ry: bry,
            rotation: brotation,
            ..
        },
    ) = (first, second)
    else {
        return Vec::new();
    };
    if ![*arx, *ary, *brx, *bry]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0)
        || !arotation.is_finite()
        || !brotation.is_finite()
    {
        return Vec::new();
    }
    let (arx, ary, brx, bry) = (*arx as f64, *ary as f64, *brx as f64, *bry as f64);
    let (asine, acosine) = (*arotation as f64).to_radians().sin_cos();
    let (bsine, bcosine) = (*brotation as f64).to_radians().sin_cos();
    let (dx, dy) = ((a.x - b.x) as f64, (a.y - b.y) as f64);
    if dx.hypot(dy) > arx.max(ary) + brx.max(bry) {
        return Vec::new();
    }
    let center = (
        (dx * bcosine + dy * bsine) / brx,
        (-dx * bsine + dy * bcosine) / bry,
    );
    let u = (
        arx * (acosine * bcosine + asine * bsine) / brx,
        arx * (-acosine * bsine + asine * bcosine) / bry,
    );
    let v = (
        ary * (-asine * bcosine + acosine * bsine) / brx,
        ary * (asine * bsine + acosine * bcosine) / bry,
    );
    let mut contacts = Vec::new();
    for sign in [1.0, -1.0] {
        let x = [
            center.0 + sign * u.0,
            2.0 * sign * v.0,
            center.0 - sign * u.0,
        ];
        let y = [
            center.1 + sign * u.1,
            2.0 * sign * v.1,
            center.1 - sign * u.1,
        ];
        let mut coefficients = [-1.0, 0.0, -2.0, 0.0, -1.0];
        for i in 0..3 {
            for j in 0..3 {
                coefficients[i + j] += x[i] * x[j] + y[i] * y[j];
            }
        }
        for t in bounded_polynomial_roots(&coefficients) {
            let px = sign * arx * (1.0 - t * t) / (1.0 + t * t);
            let py = sign * ary * 2.0 * t / (1.0 + t * t);
            let point = Point {
                x: (a.x as f64 + px * acosine - py * asine) as f32,
                y: (a.y as f64 + px * asine + py * acosine) as f32,
            };
            if !contacts
                .iter()
                .any(|other: &Point| other.distance(point) < 1e-5 * arx.max(ary) as f32)
            {
                contacts.push(point);
            }
        }
    }
    contacts
}

fn capsule_boundary(shape: &Shape) -> Option<[Shape; 4]> {
    let Shape::Capsule {
        from,
        to,
        width,
        arc_segments,
        ..
    } = shape
    else {
        return None;
    };
    if !width.is_finite() || *width <= 0.0 || from.distance(*to) <= 1e-6 {
        return None;
    }
    let angle = ((to.y - from.y) as f64).atan2((to.x - from.x) as f64);
    let radius = *width * 0.5;
    let to_arc = Shape::Arc {
        id: 0,
        center: *to,
        radius,
        start_angle: angle - std::f64::consts::FRAC_PI_2,
        sweep: std::f64::consts::PI,
        segments: *arc_segments,
    };
    let from_arc = Shape::Arc {
        id: 0,
        center: *from,
        radius,
        start_angle: angle + std::f64::consts::FRAC_PI_2,
        sweep: std::f64::consts::PI,
        segments: *arc_segments,
    };
    let line_one = Shape::Line {
        id: 0,
        a: Shape::arc_point(*to, radius, angle + std::f64::consts::FRAC_PI_2, 0.0, 0.0),
        b: Shape::arc_point(*from, radius, angle + std::f64::consts::FRAC_PI_2, 0.0, 0.0),
    };
    let line_two = Shape::Line {
        id: 0,
        a: Shape::arc_point(*from, radius, angle - std::f64::consts::FRAC_PI_2, 0.0, 0.0),
        b: Shape::arc_point(*to, radius, angle - std::f64::consts::FRAC_PI_2, 0.0, 0.0),
    };
    Some([to_arc, line_one, from_arc, line_two])
}

fn boundary_edges(shape: &Shape) -> Option<Vec<(Point, Point)>> {
    let points: Vec<Point> = match shape {
        Shape::Rectangle { a, b, .. } => {
            vec![*a, Point { x: b.x, y: a.y }, *b, Point { x: a.x, y: b.y }]
        }
        Shape::RotatedRectangle {
            center,
            width,
            height,
            rotation,
            ..
        } => Shape::rotated_rectangle_corners(*center, *width, *height, *rotation).to_vec(),
        Shape::Polygon {
            center,
            radius,
            sides,
            rotation,
            ..
        } if *sides >= 3 => (0..*sides)
            .map(|index| Shape::polygon_vertex(*center, *radius, *sides, *rotation, index))
            .collect(),
        _ => return None,
    };
    Some(
        (0..points.len())
            .map(|index| (points[index], points[(index + 1) % points.len()]))
            .collect(),
    )
}

fn arc_progress(point: Point, center: Point, start: f64, sweep: f64) -> f64 {
    let angle = ((point.y - center.y) as f64).atan2((point.x - center.x) as f64);
    let traveled = if sweep > 0.0 {
        (angle - start).rem_euclid(std::f64::consts::TAU)
    } else {
        (start - angle).rem_euclid(std::f64::consts::TAU)
    };
    traveled / sweep.abs()
}

fn on_arc_sweep(point: Point, center: Point, start: f64, sweep: f64) -> bool {
    if !start.is_finite() || !sweep.is_finite() || sweep.abs() <= 1e-12 {
        return false;
    }
    let angle = ((point.y - center.y) as f64).atan2((point.x - center.x) as f64);
    let tau = std::f64::consts::TAU;
    if sweep.abs() >= tau - 1e-9 {
        return true;
    }
    let traveled = if sweep > 0.0 {
        (angle - start).rem_euclid(tau)
    } else {
        (start - angle).rem_euclid(tau)
    };
    traveled <= sweep.abs() + 1e-8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_arc_uses_start_radius_and_clockwise_sweep() {
        let center = Point { x: 0.0, y: 0.0 };
        let start = Point { x: 10.0, y: 0.0 };
        let end = Point { x: 0.0, y: 20.0 };
        let ccw = Shape::arc_from_center_points(1, center, start, end, false).unwrap();
        let cw = Shape::arc_from_center_points(2, center, start, end, true).unwrap();
        assert!(
            matches!(ccw, Shape::Arc { radius: 10.0, sweep, .. } if (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-6)
        );
        assert!(
            matches!(cw, Shape::Arc { radius: 10.0, sweep, .. } if (sweep + 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-6)
        );
        assert!(Shape::arc_from_center_points(3, center, start, start, false).is_none());
        let mut doc = Document {
            tool: Tool::CenterArc,
            arc_clockwise: true,
            ..Document::default()
        };
        doc.draft_start = Some(center);
        assert!(!doc.finish_draft(start));
        assert!(doc.finish_draft(end));
        assert!(matches!(doc.shapes[0], Shape::Arc { sweep, .. } if sweep < -4.0));
    }

    #[test]
    fn three_point_circle_constructs_circumcircle_and_rejects_collinear_points() {
        let first = Point { x: 0.0, y: 10.0 };
        let second = Point { x: 10.0, y: 0.0 };
        let third = Point { x: 0.0, y: -10.0 };
        let mut doc = Document {
            tool: Tool::ThreePointCircle,
            ..Document::default()
        };
        doc.draft_start = Some(first);
        assert!(!doc.finish_draft(second));
        assert!(doc.finish_draft(third));
        assert!(matches!(
            doc.shapes[0],
            Shape::Circle {
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
                ..
            }
        ));
        assert!(doc.undo());
        assert!(doc.shapes.is_empty());
        assert!(
            Shape::circle_from_three_points(1, first, Point { x: 0.0, y: 5.0 }, third).is_none()
        );
    }

    #[test]
    fn axis_constraint_snaps_preserves_length_and_survives_copy_undo() {
        let mut doc = Document::from_shapes(vec![Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 3.0, y: 4.0 },
        }]);
        doc.selected = Some(1);
        assert_eq!(
            doc.toggle_axis_constraint(AxisConstraint::Horizontal),
            Ok(true)
        );
        assert_eq!(
            doc.shapes[0],
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 5.0, y: 0.0 }
            }
        );
        assert!(doc.copy_selected(Point { x: 10.0, y: 0.0 }));
        assert_eq!(
            doc.axis_constraints.get(&2),
            Some(&AxisConstraint::Horizontal)
        );
        assert!(!doc.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
        assert!(doc.rotate_selected(90.0, Point { x: 0.0, y: 0.0 }, false));
        assert_eq!(
            doc.axis_constraints.get(&2),
            Some(&AxisConstraint::Vertical)
        );
        assert!(doc.undo());
        assert_eq!(
            doc.axis_constraints.get(&2),
            Some(&AxisConstraint::Horizontal)
        );
        assert!(doc.undo());
        assert!(!doc.axis_constraints.contains_key(&2));
        assert!(doc.undo());
        assert!(doc.axis_constraints.is_empty());
        assert_eq!(
            doc.shapes[0],
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 3.0, y: 4.0 }
            }
        );
    }

    #[test]
    fn construction_toggle_is_undoable_and_copied() {
        let mut doc = Document::from_shapes(vec![Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        }]);
        doc.selected = Some(1);
        assert_eq!(doc.toggle_construction(), Some(true));
        assert!(doc.construction.contains(&1));
        assert!(doc.copy_selected(Point { x: 30.0, y: 0.0 }));
        assert!(doc.construction.contains(&2));
        assert!(doc.undo());
        assert!(!doc.construction.contains(&2));
        assert!(doc.undo());
        assert!(!doc.construction.contains(&1));
        assert!(doc.redo());
        assert!(doc.construction.contains(&1));
    }

    #[test]
    fn viewport_round_trip_and_anchor_zoom() {
        let mut doc = Document::default();
        let origin = Point { x: 220.0, y: 80.0 };
        let screen = Point { x: 500.0, y: 340.0 };
        let before = doc.screen_to_world(screen, origin);
        doc.zoom_at(screen, origin, 1.5);
        assert_eq!(doc.screen_to_world(screen, origin), before);
        assert_eq!(doc.world_to_screen(before, origin), screen);
        assert_eq!(
            doc.world_to_screen(Point { x: 0.0, y: 10.0 }, origin).y,
            origin.y - 10.0 * doc.zoom + doc.pan.y
        );
    }

    #[test]
    fn fit_shapes_centers_all_geometry_and_respects_viewport_margin() {
        let mut doc = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point {
                    x: 1000.0,
                    y: -500.0,
                },
                radius: 100.0,
                segments: 64,
            },
            Shape::Rectangle {
                id: 2,
                a: Point {
                    x: 1300.0,
                    y: -400.0,
                },
                b: Point {
                    x: 1500.0,
                    y: -200.0,
                },
            },
        ]);
        assert!(doc.fit_shapes(800.0, 600.0));
        let origin = Point { x: 400.0, y: 300.0 };
        for shape in &doc.shapes {
            let (min, max) = shape.bounds().unwrap();
            for corner in [min, max] {
                let screen = doc.world_to_screen(corner, origin);
                assert!((48.0..=752.0).contains(&screen.x));
                assert!((48.0..=552.0).contains(&screen.y));
            }
        }
        let before = (doc.zoom, doc.pan);
        assert!(!doc.fit_shapes(50.0, 600.0));
        assert_eq!((doc.zoom, doc.pan), before);
        assert!(!Document::default().fit_shapes(800.0, 600.0));
    }

    #[test]
    fn snap_prefers_nearby_geometry_then_origin_and_grid() {
        let mut doc = Document::from_shapes(vec![Shape::Line {
            id: 1,
            a: Point { x: 10.25, y: 20.5 },
            b: Point { x: 30.25, y: 20.5 },
        }]);
        assert_eq!(
            doc.snap_point(Point { x: 12.0, y: 21.0 }, true, true),
            Some(SnapResult {
                point: Point { x: 10.25, y: 20.5 },
                kind: SnapKind::Anchor
            })
        );
        assert_eq!(
            doc.snap_point(Point { x: 20.2, y: 19.9 }, true, true),
            Some(SnapResult {
                point: Point { x: 20.25, y: 20.5 },
                kind: SnapKind::Midpoint
            })
        );
        assert_eq!(
            doc.snap_point(Point { x: 0.1, y: -0.2 }, true, true),
            Some(SnapResult {
                point: Point { x: 0.0, y: 0.0 },
                kind: SnapKind::Origin
            })
        );
        assert_eq!(
            doc.snap_point(Point { x: 12.0, y: 21.0 }, true, false),
            Some(SnapResult {
                point: Point { x: 12.0, y: 21.0 },
                kind: SnapKind::Grid
            })
        );
        assert_eq!(
            doc.snap_point(Point { x: 12.0, y: 21.0 }, false, false),
            None
        );
        doc.zoom = 8.0;
        assert_eq!(
            doc.snap_point(Point { x: 12.0, y: 21.0 }, false, true),
            None
        );
    }

    #[test]
    fn snaps_to_finite_line_intersection() {
        let doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 10.0, y: 10.0 },
                b: Point { x: 30.0, y: 30.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 10.0, y: 30.0 },
                b: Point { x: 30.0, y: 10.0 },
            },
        ]);
        assert_eq!(
            doc.snap_point(Point { x: 21.0, y: 20.5 }, false, true),
            Some(SnapResult {
                point: Point { x: 20.0, y: 20.0 },
                kind: SnapKind::Intersection,
            })
        );
        assert_eq!(
            doc.snap_point(Point { x: 21.0, y: 20.5 }, false, false),
            None
        );
        let outside = Shape::Line {
            id: 3,
            a: Point { x: 40.0, y: 10.0 },
            b: Point { x: 40.0, y: 30.0 },
        };
        assert!(finite_intersections(&doc.shapes[0], &outside).is_empty());
        let parallel = Shape::Line {
            id: 4,
            a: Point { x: 10.0, y: 11.0 },
            b: Point { x: 30.0, y: 31.0 },
        };
        assert!(finite_intersections(&doc.shapes[0], &parallel).is_empty());
    }

    #[test]
    fn line_spline_intersection_snaps_and_respects_finite_line() {
        let spline = Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: vec![
                Point { x: 10.0, y: 10.0 },
                Point { x: 10.0, y: 30.0 },
                Point { x: 30.0, y: 30.0 },
                Point { x: 30.0, y: 10.0 },
            ],
            closed: false,
        };
        let line = Shape::Line {
            id: 2,
            a: Point { x: 20.0, y: 0.0 },
            b: Point { x: 20.0, y: 30.0 },
        };
        let doc = Document::from_shapes(vec![spline.clone(), line.clone()]);
        assert_eq!(finite_intersections(&spline, &line).len(), 1);
        assert_eq!(finite_intersections(&line, &spline).len(), 1);
        assert_eq!(
            doc.snap_point(Point { x: 20.4, y: 24.7 }, false, true),
            Some(SnapResult {
                point: Point { x: 20.0, y: 25.0 },
                kind: SnapKind::Intersection
            })
        );
        let short = Shape::Line {
            id: 2,
            a: Point { x: 20.0, y: 0.0 },
            b: Point { x: 20.0, y: 20.0 },
        };
        assert!(finite_intersections(&spline, &short).is_empty());
    }

    #[test]
    fn snaps_to_finite_line_circle_intersections_and_tangent() {
        let line = Shape::Line {
            id: 1,
            a: Point { x: 10.0, y: 20.0 },
            b: Point { x: 30.0, y: 20.0 },
        };
        let circle = Shape::Circle {
            id: 2,
            center: Point { x: 20.0, y: 20.0 },
            radius: 6.0,
            segments: 64,
        };
        let doc = Document::from_shapes(vec![line.clone(), circle.clone()]);
        assert_eq!(
            doc.snap_point(Point { x: 14.4, y: 20.3 }, false, true),
            Some(SnapResult {
                point: Point { x: 14.0, y: 20.0 },
                kind: SnapKind::Intersection,
            })
        );
        assert_eq!(
            doc.snap_point(Point { x: 25.6, y: 20.3 }, false, true),
            Some(SnapResult {
                point: Point { x: 26.0, y: 20.0 },
                kind: SnapKind::Intersection,
            })
        );
        let tangent = Shape::Line {
            id: 3,
            a: Point { x: 10.0, y: 26.0 },
            b: Point { x: 30.0, y: 26.0 },
        };
        assert_eq!(
            finite_intersections(&tangent, &circle),
            vec![Point { x: 20.0, y: 26.0 }]
        );
        let short = Shape::Line {
            id: 4,
            a: Point { x: 1.0, y: 20.0 },
            b: Point { x: 11.0, y: 20.0 },
        };
        assert!(finite_intersections(&short, &circle).is_empty());
    }

    #[test]
    fn circular_intersections_respect_tangency_and_arc_sweeps() {
        let left = Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        };
        let right = Shape::Circle {
            id: 2,
            center: Point { x: 6.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        };
        assert_eq!(
            finite_intersections(&left, &right),
            vec![Point { x: 3.0, y: 4.0 }, Point { x: 3.0, y: -4.0 }]
        );
        let tangent = Shape::Circle {
            id: 3,
            center: Point { x: 10.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        };
        assert_eq!(
            finite_intersections(&left, &tangent),
            vec![Point { x: 5.0, y: 0.0 }]
        );
        let coincident = Shape::Circle {
            id: 4,
            center: Point { x: 0.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        };
        assert!(finite_intersections(&left, &coincident).is_empty());
        let upper_arc = Shape::Arc {
            id: 5,
            center: Point { x: 0.0, y: 0.0 },
            radius: 5.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        };
        assert_eq!(
            finite_intersections(&upper_arc, &right),
            vec![Point { x: 3.0, y: 4.0 }]
        );
        let crossing = Shape::Line {
            id: 6,
            a: Point { x: 3.0, y: -8.0 },
            b: Point { x: 3.0, y: 8.0 },
        };
        assert_eq!(
            finite_intersections(&crossing, &upper_arc),
            vec![Point { x: 3.0, y: 4.0 }]
        );
    }

    #[test]
    fn composite_shape_boundaries_have_finite_deduplicated_intersections() {
        let rectangle = Shape::Rectangle {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 10.0 },
        };
        let through_corner = Shape::Line {
            id: 2,
            a: Point { x: -5.0, y: -5.0 },
            b: Point { x: 0.0, y: 0.0 },
        };
        assert_eq!(
            finite_intersections(&rectangle, &through_corner),
            vec![Point { x: 0.0, y: 0.0 }]
        );
        let through_middle = Shape::Line {
            id: 3,
            a: Point { x: -5.0, y: 5.0 },
            b: Point { x: 15.0, y: 5.0 },
        };
        assert_eq!(
            finite_intersections(&rectangle, &through_middle),
            vec![Point { x: 10.0, y: 5.0 }, Point { x: 0.0, y: 5.0 }]
        );
        let rotated = Shape::RotatedRectangle {
            id: 4,
            center: Point { x: 0.0, y: 0.0 },
            width: 10.0,
            height: 10.0,
            rotation: 45.0,
        };
        assert_eq!(
            finite_intersections(
                &rotated,
                &Shape::Line {
                    id: 5,
                    a: Point { x: -10.0, y: 0.0 },
                    b: Point { x: 10.0, y: 0.0 }
                }
            )
            .len(),
            2
        );
        let triangle = Shape::Polygon {
            id: 6,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            sides: 3,
            rotation: 0.0,
        };
        assert_eq!(
            finite_intersections(
                &triangle,
                &Shape::Line {
                    id: 7,
                    a: Point { x: 0.0, y: -20.0 },
                    b: Point { x: 0.0, y: 20.0 }
                }
            )
            .len(),
            2
        );
    }

    #[test]
    fn slot_intersections_follow_straight_sides_and_round_caps() {
        let slot = Shape::Capsule {
            id: 1,
            from: Point { x: 0.0, y: 0.0 },
            to: Point { x: 20.0, y: 0.0 },
            width: 10.0,
            arc_segments: 48,
        };
        let middle = Shape::Line {
            id: 2,
            a: Point { x: 10.0, y: -10.0 },
            b: Point { x: 10.0, y: 10.0 },
        };
        assert_eq!(
            finite_intersections(&slot, &middle),
            vec![Point { x: 10.0, y: 5.0 }, Point { x: 10.0, y: -5.0 }]
        );
        let cap = Shape::Line {
            id: 3,
            a: Point { x: 23.0, y: -10.0 },
            b: Point { x: 23.0, y: 10.0 },
        };
        assert_eq!(
            finite_intersections(&slot, &cap),
            vec![Point { x: 23.0, y: -4.0 }, Point { x: 23.0, y: 4.0 }]
        );
        let junction = Shape::Line {
            id: 4,
            a: Point { x: 20.0, y: 5.0 },
            b: Point { x: 20.0, y: 10.0 },
        };
        assert_eq!(
            finite_intersections(&slot, &junction),
            vec![Point { x: 20.0, y: 5.0 }]
        );
        let none = Shape::Line {
            id: 5,
            a: Point { x: 26.0, y: -10.0 },
            b: Point { x: 26.0, y: 10.0 },
        };
        assert!(finite_intersections(&slot, &none).is_empty());
    }

    #[test]
    fn finite_line_ellipse_intersections_include_rotated_axes_and_tangent() {
        let ellipse = Shape::Ellipse {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 64,
        };
        let crossing = Shape::Line {
            id: 2,
            a: Point { x: -20.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        };
        assert_eq!(
            finite_intersections(&crossing, &ellipse),
            vec![Point { x: -10.0, y: 0.0 }, Point { x: 10.0, y: 0.0 }]
        );
        let tangent = Shape::Line {
            id: 3,
            a: Point { x: -20.0, y: 5.0 },
            b: Point { x: 20.0, y: 5.0 },
        };
        assert_eq!(
            finite_intersections(&ellipse, &tangent),
            vec![Point { x: 0.0, y: 5.0 }]
        );
        let short = Shape::Line {
            id: 4,
            a: Point { x: 11.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        };
        assert!(finite_intersections(&short, &ellipse).is_empty());
        let rotated = Shape::Ellipse {
            id: 6,
            center: Point { x: 0.0, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 45.0,
            segments: 64,
        };
        let diagonal = Shape::Line {
            id: 5,
            a: Point { x: -20.0, y: -20.0 },
            b: Point { x: 20.0, y: 20.0 },
        };
        let hits = finite_intersections(&diagonal, &rotated);
        assert_eq!(hits.len(), 2);
        let expected = 10.0 / 2.0_f32.sqrt();
        assert!((hits[0].x + expected).abs() < 1e-4 && (hits[0].y + expected).abs() < 1e-4);
        assert!((hits[1].x - expected).abs() < 1e-4 && (hits[1].y - expected).abs() < 1e-4);
    }

    #[test]
    fn circle_ellipse_contacts_include_tangent_and_respect_arc_sweep() {
        let ellipse = Shape::Ellipse {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 100.0,
            ry: 50.0,
            rotation: 0.0,
            segments: 128,
        };
        let circle = Shape::Circle {
            id: 2,
            center: Point { x: 50.0, y: 0.0 },
            radius: 50.0,
            segments: 64,
        };
        let hits = finite_intersections(&circle, &ellipse);
        assert_eq!(hits.len(), 3);
        assert!(
            hits.iter()
                .any(|point| point.distance(Point { x: 100.0, y: 0.0 }) < 1e-3)
        );
        assert!(hits.iter().any(|point| point.distance(Point {
            x: 100.0 / 3.0,
            y: 50.0 * (8.0_f32 / 9.0).sqrt()
        }) < 1e-3));
        assert!(hits.iter().any(|point| point.distance(Point {
            x: 100.0 / 3.0,
            y: -50.0 * (8.0_f32 / 9.0).sqrt()
        }) < 1e-3));
        let upper_arc = Shape::Arc {
            id: 3,
            center: Point { x: 50.0, y: 0.0 },
            radius: 50.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        };
        assert_eq!(finite_intersections(&upper_arc, &ellipse).len(), 2);
        let disjoint = Shape::Circle {
            id: 4,
            center: Point { x: 200.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        };
        assert!(finite_intersections(&disjoint, &ellipse).is_empty());
        let coincident = Shape::Ellipse {
            id: 5,
            center: Point { x: 0.0, y: 0.0 },
            rx: 10.0,
            ry: 10.0,
            rotation: 0.0,
            segments: 64,
        };
        let same_circle = Shape::Circle {
            id: 6,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        };
        assert!(finite_intersections(&same_circle, &coincident).is_empty());
    }

    #[test]
    fn ellipse_ellipse_contacts_include_four_crossings_tangent_and_coincident() {
        let first = Shape::Ellipse {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 128,
        };
        let perpendicular = Shape::Ellipse {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            rx: 5.0,
            ry: 10.0,
            rotation: 0.0,
            segments: 128,
        };
        let hits = finite_intersections(&first, &perpendicular);
        assert_eq!(hits.len(), 4);
        for hit in &hits {
            assert!(
                (hit.x.abs() - 20.0_f32.sqrt()).abs() < 1e-3
                    && (hit.y.abs() - 20.0_f32.sqrt()).abs() < 1e-3
            );
        }
        let tangent = Shape::Ellipse {
            id: 3,
            center: Point { x: 20.0, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 128,
        };
        assert_eq!(
            finite_intersections(&first, &tangent),
            vec![Point { x: 10.0, y: 0.0 }]
        );
        let same = Shape::Ellipse {
            id: 4,
            center: Point { x: 0.0, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 128,
        };
        assert!(finite_intersections(&first, &same).is_empty());
    }

    #[test]
    fn line_endpoint_linked_to_point_follows_move_drag_and_history() {
        let mut doc = Document::default();
        let at = Point { x: 10.0, y: 20.0 };
        assert!(doc.add_point(at));
        doc.tool = Tool::Line;
        doc.draft_start = Some(at);
        assert!(doc.finish_draft(Point { x: 50.0, y: 20.0 }));
        assert_eq!(doc.point_links.get(&(2, 0)), Some(&1));
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 5.0, y: -3.0 }));
        assert!(matches!(
            doc.shapes[1],
            Shape::Line {
                a: Point { x: 15.0, y: 17.0 },
                ..
            }
        ));
        assert!(doc.undo());
        assert_eq!(doc.point_links.get(&(2, 0)), Some(&1));
        assert!(matches!(doc.shapes[1], Shape::Line { a, .. } if a == at));
        assert!(doc.redo());
        doc.selected = Some(1);
        assert!(doc.begin_drag(Point { x: 15.0, y: 17.0 }));
        assert!(doc.drag_to(Point { x: 25.0, y: 27.0 }));
        assert!(matches!(
            doc.shapes[1],
            Shape::Line {
                a: Point { x: 25.0, y: 27.0 },
                ..
            }
        ));
        assert!(doc.cancel_drag());
        assert!(matches!(
            doc.shapes[1],
            Shape::Line {
                a: Point { x: 15.0, y: 17.0 },
                ..
            }
        ));
        // Moving the line carries the linked point with it in one undo step.
        doc.selected = Some(2);
        assert!(doc.move_selected(Point { x: 1.0, y: 0.0 }));
        assert!(
            matches!(doc.shapes[0], Shape::Point { at, .. } if at == Point { x: 16.0, y: 17.0 })
        );
        assert!(doc.undo());
        assert!(
            matches!(doc.shapes[0], Shape::Point { at, .. } if at == Point { x: 15.0, y: 17.0 })
        );
        doc.selected = Some(2);
        assert!(doc.delete_selected());
        assert!(doc.point_links.is_empty());
    }

    #[test]
    fn spline_endpoints_follow_linked_points_and_split_keeps_original_ends() {
        let mut doc = Document::default();
        let start = Point { x: 0.0, y: 0.0 };
        let end = Point { x: 30.0, y: 0.0 };
        assert!(doc.add_point(start));
        assert!(doc.add_point(end));
        let spline_id = doc
            .add_spline(
                SplineMode::Control,
                vec![
                    start,
                    Point { x: 10.0, y: 12.0 },
                    Point { x: 20.0, y: 12.0 },
                    end,
                ],
                false,
            )
            .unwrap();
        assert_eq!(doc.point_links.get(&(spline_id, 0)), Some(&1));
        assert_eq!(doc.point_links.get(&(spline_id, 1)), Some(&2));
        doc.selected = Some(spline_id);
        doc.selected_spline_point = Some(0);
        // Editing a linked end handle carries the point along.
        assert_eq!(doc.set_selected_spline_coordinate(true, 2.0), Ok(()));
        assert_eq!(doc.shapes[0].center(), Point { x: 2.0, y: 0.0 });
        assert!(doc.undo());
        assert_eq!(doc.shapes[0].center(), start);
        doc.selected = Some(spline_id);
        doc.selected_spline_point = Some(0);
        assert!(doc.begin_drag(start));
        assert!(doc.drag_to(Point { x: 1.0, y: 4.0 }));
        assert_eq!(doc.shapes[0].center(), Point { x: 1.0, y: 4.0 });
        assert!(doc.cancel_drag());
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 2.0, y: 3.0 }));
        assert!(
            matches!(&doc.shapes[2], Shape::Spline { points, .. } if points[0] == Point { x: 2.0, y: 3.0 })
        );
        assert!(doc.undo());
        assert!(doc.redo());
        doc.selected = Some(spline_id);
        let new_id = doc
            .split_selected_spline(Point { x: 15.0, y: 9.0 })
            .unwrap();
        assert_eq!(doc.point_links.get(&(spline_id, 0)), Some(&1));
        assert_eq!(doc.point_links.get(&(new_id, 1)), Some(&2));
        assert!(!doc.point_links.contains_key(&(spline_id, 1)));
    }

    #[test]
    fn point_coordinate_edits_move_linked_line_and_respect_driving_length() {
        let mut document = Document::from_shapes(vec![
            Shape::Point {
                id: 1,
                at: Point { x: 10.0, y: 20.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 10.0, y: 20.0 },
                b: Point { x: 50.0, y: 20.0 },
            },
        ]);
        document.point_links.insert((2, 0), 1);
        document.selected = Some(1);
        assert!(
            document
                .set_selected_point_coordinate(true, f32::NAN)
                .is_err()
        );
        assert_eq!(document.set_selected_point_coordinate(true, 15.0), Ok(()));
        assert_eq!(document.set_selected_point_coordinate(false, -5.0), Ok(()));
        assert!(
            matches!(document.shapes[0], Shape::Point { at, .. } if at == Point { x: 15.0, y: -5.0 })
        );
        assert!(
            matches!(document.shapes[1], Shape::Line { a, .. } if a == Point { x: 15.0, y: -5.0 })
        );
        assert!(document.undo());
        assert!(
            matches!(document.shapes[0], Shape::Point { at, .. } if at == Point { x: 15.0, y: 20.0 })
        );
        assert!(document.undo());
        assert!(
            matches!(document.shapes[0], Shape::Point { at, .. } if at == Point { x: 10.0, y: 20.0 })
        );
        document
            .driving_dimensions
            .insert((2, DimensionField::LineLength), 40.0);
        assert!(document.set_selected_point_coordinate(true, 20.0).is_err());
        assert!(
            matches!(document.shapes[0], Shape::Point { at, .. } if at == Point { x: 10.0, y: 20.0 })
        );
    }

    #[test]
    fn fixed_point_and_spline_handle_block_edits_and_undo_cleanly() {
        let mut document = Document::from_shapes(vec![
            Shape::Point {
                id: 1,
                at: Point { x: 10.0, y: 20.0 },
            },
            Shape::Spline {
                id: 2,
                mode: SplineMode::Control,
                closed: false,
                points: vec![
                    Point { x: 0.0, y: 0.0 },
                    Point { x: 10.0, y: 20.0 },
                    Point { x: 20.0, y: -20.0 },
                    Point { x: 30.0, y: 0.0 },
                ],
            },
        ]);
        document.selected = Some(1);
        assert_eq!(document.toggle_fixed_anchor(), Ok(true));
        assert_eq!(
            document.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 10.0, y: 20.0 })
        );
        assert!(document.set_selected_point_coordinate(true, 15.0).is_err());
        assert!(!document.begin_drag(Point { x: 10.0, y: 20.0 }));
        assert!(!document.move_selected(Point { x: 1.0, y: 0.0 }));
        assert!(!document.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
        assert!(!document.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
        assert!(document.copy_selected(Point { x: 5.0, y: 0.0 }));
        assert!(!document.fixed_anchors.contains_key(&(3, 0)));
        assert!(document.undo());
        assert_eq!(
            document.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 10.0, y: 20.0 })
        );
        document.selected = Some(2);
        document.selected_spline_point = Some(1);
        assert_eq!(document.toggle_fixed_anchor(), Ok(true));
        assert!(
            document
                .set_selected_spline_coordinate(false, 25.0)
                .is_err()
        );
        assert!(!document.begin_drag(Point { x: 10.0, y: 20.0 }));
        assert!(
            document
                .split_selected_spline(Point { x: 15.0, y: 0.0 })
                .is_err()
        );
        assert!(
            document
                .trim_selected_spline(Point { x: 15.0, y: 0.0 })
                .is_err()
        );
        assert!(document.undo());
        assert!(!document.fixed_anchors.contains_key(&(2, 1)));
        document.selected = Some(2);
        document.selected_spline_point = Some(1);
        assert_eq!(document.toggle_fixed_anchor(), Ok(true));
        assert_eq!(document.toggle_fixed_anchor(), Ok(false));
        assert!(document.set_selected_spline_coordinate(false, 25.0).is_ok());
        document.selected = Some(1);
        assert!(document.delete_selected());
        assert!(!document.fixed_anchors.contains_key(&(1, 0)));
    }

    #[test]
    fn fixed_line_endpoints_block_edits_and_follow_split() {
        let mut doc = Document::from_shapes(vec![Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 40.0, y: 0.0 },
        }]);
        doc.selected = Some(1);
        assert!(
            doc.toggle_fixed_line_endpoint_at(Point { x: 20.0, y: 0.0 })
                .is_err()
        );
        assert_eq!(
            doc.toggle_fixed_line_endpoint_at(Point { x: 40.0, y: 0.0 }),
            Ok(true)
        );
        assert_eq!(
            doc.fixed_anchors.get(&(1, 1)),
            Some(&Point { x: 40.0, y: 0.0 })
        );
        assert!(!doc.move_selected(Point { x: 1.0, y: 0.0 }));
        assert!(doc.set_dimension(DimensionField::LineLength, 50.0).is_err());
        assert_eq!(doc.split_selected_line(Point { x: 15.0, y: 0.0 }), Ok(2));
        assert_eq!(
            doc.fixed_anchors.get(&(2, 1)),
            Some(&Point { x: 40.0, y: 0.0 })
        );
        assert!(!doc.fixed_anchors.contains_key(&(1, 1)));
        assert!(doc.undo());
        assert_eq!(
            doc.fixed_anchors.get(&(1, 1)),
            Some(&Point { x: 40.0, y: 0.0 })
        );
        doc.selected = Some(1);
        assert_eq!(
            doc.toggle_fixed_line_endpoint_at(Point { x: 40.0, y: 0.0 }),
            Ok(false)
        );
        assert!(doc.move_selected(Point { x: 1.0, y: 0.0 }));
    }

    #[test]
    fn fixed_curve_centers_keep_radius_edits_but_block_transforms() {
        let mut doc = Document::from_shapes(vec![Shape::Circle {
            id: 1,
            center: Point { x: 12.0, y: -5.0 },
            radius: 10.0,
            segments: 64,
        }]);
        doc.selected = Some(1);
        assert_eq!(doc.toggle_fixed_anchor(), Ok(true));
        assert_eq!(
            doc.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 12.0, y: -5.0 })
        );
        assert!(!doc.move_selected(Point { x: 2.0, y: 0.0 }));
        assert!(!doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
        assert_eq!(
            doc.set_dimension(DimensionField::CircleDiameter, 30.0),
            Ok(())
        );
        assert!(matches!(
            doc.shapes[0],
            Shape::Circle {
                center: Point { x: 12.0, y: -5.0 },
                radius: 15.0,
                ..
            }
        ));
        assert!(doc.undo());
        assert_eq!(
            doc.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 12.0, y: -5.0 })
        );
        doc.selected = Some(1);
        assert_eq!(doc.toggle_fixed_anchor(), Ok(false));
        assert!(doc.move_selected(Point { x: 2.0, y: 0.0 }));
    }

    #[test]
    fn fixed_arc_endpoint_transfers_on_split_and_blocks_extension() {
        let mut doc = Document::from_shapes(vec![Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        }]);
        doc.selected = Some(1);
        let end = doc.fixed_anchor_position(1, 2).unwrap();
        assert_eq!(
            doc.toggle_fixed_arc_anchor_at(Point { x: 0.0, y: 10.0 }),
            Ok((2, true))
        );
        assert_eq!(doc.fixed_anchors.get(&(1, 2)), Some(&end));
        assert!(doc.extend_selected_arc(Point { x: 0.0, y: 10.0 }).is_err());
        let split = Point {
            x: 10.0 / 2.0_f32.sqrt(),
            y: 10.0 / 2.0_f32.sqrt(),
        };
        assert_eq!(doc.split_selected_arc(split), Ok(2));
        assert_eq!(doc.fixed_anchors.get(&(2, 2)), Some(&end));
        assert!(!doc.fixed_anchors.contains_key(&(1, 2)));
        assert!(doc.undo());
        assert_eq!(doc.fixed_anchors.get(&(1, 2)), Some(&end));
        doc.selected = Some(1);
        assert_eq!(
            doc.toggle_fixed_arc_anchor_at(Point { x: 0.0, y: 10.0 }),
            Ok((2, false))
        );
    }

    #[test]
    fn fixed_elliptic_arc_end_transfers_on_split_and_drops_when_trimmed() {
        let ellipse_arc = Shape::EllipticArc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 0.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        };
        let boundary = Shape::Line {
            id: 2,
            a: Point { x: -30.0, y: 5.0 },
            b: Point { x: 30.0, y: 5.0 },
        };
        let mut doc = Document::from_shapes(vec![ellipse_arc, boundary]);
        doc.selected = Some(1);
        let end = doc.fixed_anchor_position(1, 2).unwrap();
        assert_eq!(doc.toggle_fixed_arc_anchor_at(end), Ok((2, true)));
        assert!(doc.extend_selected_elliptic_arc(end).is_err());
        let split = Point {
            x: 20.0 / 2.0_f32.sqrt(),
            y: 10.0 / 2.0_f32.sqrt(),
        };
        assert_eq!(doc.split_selected_elliptic_arc(split), Ok(3));
        assert_eq!(doc.fixed_anchors.get(&(3, 2)), Some(&end));
        assert!(doc.undo());
        doc.selected = Some(1);
        assert_eq!(
            doc.trim_selected_elliptic_arc(Point { x: 10.0, y: 8.66 }),
            Ok(1)
        );
        assert!(!doc.fixed_anchors.contains_key(&(1, 2)));
    }

    #[test]
    fn named_shape_anchors_guard_only_edits_that_move_them() {
        let mut doc = Document::from_shapes(vec![
            Shape::Ellipse {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 0.0,
                segments: 64,
            },
            Shape::RotatedRectangle {
                id: 2,
                center: Point { x: 60.0, y: 0.0 },
                width: 20.0,
                height: 10.0,
                rotation: 30.0,
            },
            Shape::Polygon {
                id: 3,
                center: Point { x: 100.0, y: 0.0 },
                radius: 10.0,
                sides: 5,
                rotation: 0.0,
            },
            Shape::Capsule {
                id: 4,
                from: Point { x: 140.0, y: 0.0 },
                to: Point { x: 160.0, y: 0.0 },
                width: 10.0,
                arc_segments: 16,
            },
        ]);
        doc.selected = Some(1);
        assert_eq!(
            doc.toggle_fixed_shape_anchor_at(Point { x: 20.0, y: 0.0 }),
            Ok((1, true))
        );
        assert!(
            doc.set_dimension(DimensionField::RectangleWidth, 50.0)
                .is_err()
        );
        assert_eq!(
            doc.set_dimension(DimensionField::RectangleHeight, 30.0),
            Ok(())
        );
        doc.selected = Some(2);
        let corner = doc.fixed_anchor_position(2, 1).unwrap();
        assert_eq!(doc.toggle_fixed_shape_anchor_at(corner), Ok((1, true)));
        assert!(
            doc.set_dimension(DimensionField::RectangleWidth, 30.0)
                .is_err()
        );
        doc.selected = Some(3);
        assert_eq!(
            doc.toggle_fixed_shape_anchor_at(Point { x: 110.0, y: 0.0 }),
            Ok((1, true))
        );
        assert!(doc.set_polygon_sides(6).is_err());
        doc.selected = Some(4);
        assert_eq!(
            doc.toggle_fixed_shape_anchor_at(Point { x: 160.0, y: 0.0 }),
            Ok((1, true))
        );
        assert_eq!(doc.set_capsule_width(12.0), Ok(()));
        assert!(!doc.move_selected(Point { x: 1.0, y: 0.0 }));
    }

    #[test]
    fn fixed_corner_rectangle_resizes_about_anchored_corner() {
        let mut doc = Document::from_shapes(vec![Shape::Rectangle {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 10.0 },
        }]);
        doc.selected = Some(1);
        assert_eq!(
            doc.toggle_fixed_line_endpoint_at(Point { x: 0.0, y: 0.0 }),
            Ok(true)
        );
        assert_eq!(
            doc.set_dimension(DimensionField::RectangleWidth, 30.0),
            Ok(())
        );
        assert_eq!(
            doc.set_dimension(DimensionField::RectangleHeight, 15.0),
            Ok(())
        );
        assert_eq!(
            doc.shapes[0],
            Shape::Rectangle {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 30.0, y: 15.0 }
            }
        );
        assert_eq!(
            doc.fixed_anchors.get(&(1, 0)),
            Some(&Point { x: 0.0, y: 0.0 })
        );
        assert_eq!(
            doc.toggle_fixed_line_endpoint_at(Point { x: 30.0, y: 15.0 }),
            Ok(true)
        );
        assert!(
            doc.set_dimension(DimensionField::RectangleWidth, 40.0)
                .is_err()
        );
        assert!(
            doc.set_dimension(DimensionField::RectangleHeight, 20.0)
                .is_err()
        );
        assert!(!doc.move_selected(Point { x: 1.0, y: 1.0 }));
    }

    #[test]
    fn equal_lines_follow_source_length_and_undo_as_one_edit() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 20.0 },
                b: Point { x: 0.0, y: 30.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_equal_line_with(1), Ok(true));
        assert_eq!(doc.equal_lines.get(&2), Some(&1));
        assert!(matches!(
            doc.shapes[1],
            Shape::Line {
                b: Point { x: 0.0, y: 40.0 },
                ..
            }
        ));
        // A length on the Equal target now drives the source through the solver.
        assert_eq!(doc.set_dimension(DimensionField::LineLength, 25.0), Ok(()));
        assert!(matches!(
            doc.shapes[0],
            Shape::Line { a: Point { x: 0.0, y: 0.0 }, b, .. } if (b.x - 25.0).abs() < 1e-4 && b.y.abs() < 1e-4
        ));
        assert!(doc.undo());
        doc.selected = Some(2);
        assert!(doc.split_selected_line(Point { x: 0.0, y: 30.0 }).is_err());
        doc.selected = Some(1);
        assert_eq!(doc.set_dimension(DimensionField::LineLength, 30.0), Ok(()));
        assert!(matches!(
            doc.shapes[1],
            Shape::Line {
                b: Point { x: 0.0, y: 50.0 },
                ..
            }
        ));
        assert!(doc.undo());
        assert!(matches!(
            doc.shapes[0],
            Shape::Line {
                b: Point { x: 20.0, y: 0.0 },
                ..
            }
        ));
        assert!(matches!(
            doc.shapes[1],
            Shape::Line {
                b: Point { x: 0.0, y: 40.0 },
                ..
            }
        ));
        doc.selected = Some(2);
        assert_eq!(doc.toggle_equal_line_with(1), Ok(false));
        assert!(doc.equal_lines.is_empty());
        assert_eq!(doc.set_dimension(DimensionField::LineLength, 15.0), Ok(()));
    }

    #[test]
    fn equal_radius_tracks_circle_diameter_and_undo() {
        let mut doc = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
            Shape::Arc {
                id: 2,
                center: Point { x: 40.0, y: 0.0 },
                radius: 5.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 24,
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_equal_radius_with(1), Ok(true));
        assert!(matches!(doc.shapes[1], Shape::Arc { radius: 10.0, .. }));
        doc.selected = Some(1);
        assert_eq!(
            doc.set_dimension(DimensionField::CircleDiameter, 30.0),
            Ok(())
        );
        assert!(matches!(doc.shapes[1], Shape::Arc { radius: 15.0, .. }));
        assert!(doc.undo());
        assert!(matches!(doc.shapes[1], Shape::Arc { radius: 10.0, .. }));
        doc.selected = Some(2);
        assert_eq!(doc.toggle_equal_radius_with(1), Ok(false));
    }

    #[test]
    fn arc_radius_drives_equal_circle_and_fixed_end_blocks_edit() {
        let mut doc = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 32,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 40.0, y: 0.0 },
                radius: 5.0,
                segments: 64,
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_equal_radius_with(1), Ok(true));
        assert!(
            doc.set_dimension(DimensionField::CircleDiameter, 30.0)
                .is_err()
        );
        doc.selected = Some(1);
        assert_eq!(doc.set_dimension(DimensionField::CurveRadius, 20.0), Ok(()));
        assert!(matches!(doc.shapes[1], Shape::Circle { radius: 20.0, .. }));
        assert!(doc.undo());
        assert!(matches!(doc.shapes[1], Shape::Circle { radius: 10.0, .. }));
        doc.selected = Some(1);
        doc.fixed_anchors.insert((1, 1), Point { x: 10.0, y: 0.0 });
        assert!(
            doc.set_dimension(DimensionField::CurveRadius, 30.0)
                .is_err()
        );
    }

    #[test]
    fn polygon_radius_is_editable_until_vertex_is_fixed() {
        let mut doc = Document::from_shapes(vec![Shape::Polygon {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            sides: 6,
            rotation: 0.0,
        }]);
        doc.selected = Some(1);
        assert_eq!(doc.set_dimension(DimensionField::CurveRadius, 25.0), Ok(()));
        assert!(matches!(doc.shapes[0], Shape::Polygon { radius: 25.0, .. }));
        doc.fixed_anchors.insert((1, 1), Point { x: 25.0, y: 0.0 });
        assert!(
            doc.set_dimension(DimensionField::CurveRadius, 30.0)
                .is_err()
        );
    }

    #[test]
    fn signed_arc_sweep_and_shape_rotation_are_undoable_dimensions() {
        let mut doc = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 32,
            },
            Shape::Polygon {
                id: 2,
                center: Point { x: 60.0, y: 0.0 },
                radius: 10.0,
                sides: 6,
                rotation: 0.0,
            },
        ]);
        doc.selected = Some(1);
        assert_eq!(doc.set_dimension(DimensionField::ArcSweep, -90.0), Ok(()));
        assert!(
            matches!(doc.shapes[0], Shape::Arc { sweep, .. } if (sweep + std::f64::consts::FRAC_PI_2).abs() < 1e-6)
        );
        assert!(doc.set_dimension(DimensionField::ArcSweep, 0.0).is_err());
        assert!(doc.set_dimension(DimensionField::ArcSweep, 360.0).is_err());
        doc.fixed_anchors.insert((1, 2), Point { x: 0.0, y: 0.0 });
        assert!(doc.set_dimension(DimensionField::ArcSweep, 45.0).is_err());
        doc.selected = Some(2);
        assert_eq!(doc.set_dimension(DimensionField::Rotation, -30.0), Ok(()));
        assert!(matches!(
            doc.shapes[1],
            Shape::Polygon {
                rotation: -30.0,
                ..
            }
        ));
        assert!(doc.undo());
        assert!(matches!(
            doc.shapes[1],
            Shape::Polygon { rotation: 0.0, .. }
        ));
    }

    #[test]
    fn scale_preserves_angular_dimensions_and_rotate_updates_rotation() {
        let mut doc = Document::from_shapes(vec![Shape::EllipticArc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 0.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 32,
        }]);
        doc.selected = Some(1);
        assert_eq!(doc.set_dimension(DimensionField::ArcSweep, -90.0), Ok(()));
        assert_eq!(doc.set_dimension(DimensionField::Rotation, -20.0), Ok(()));
        assert!(doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
        assert_eq!(
            doc.driving_dimensions.get(&(1, DimensionField::ArcSweep)),
            Some(&-90.0)
        );
        assert_eq!(
            doc.driving_dimensions.get(&(1, DimensionField::Rotation)),
            Some(&-20.0)
        );
        assert!(doc.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
        assert_eq!(
            doc.driving_dimensions.get(&(1, DimensionField::Rotation)),
            Some(&10.0)
        );
        assert_eq!(
            doc.driving_dimensions.get(&(1, DimensionField::ArcSweep)),
            Some(&-90.0)
        );
    }

    #[test]
    fn line_rotation_dimension_updates_parallel_target() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 30.0 },
                b: Point { x: 10.0, y: 30.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(
            doc.toggle_line_relation_with(1, LineRelationKind::Parallel),
            Ok(true)
        );
        doc.selected = Some(1);
        assert_eq!(doc.set_dimension(DimensionField::Rotation, -90.0), Ok(()));
        assert!(
            matches!(doc.shapes[0], Shape::Line { b: Point { x, y }, .. } if x.abs() < 1e-4 && (y + 20.0).abs() < 1e-4)
        );
        assert!(
            matches!(doc.shapes[1], Shape::Line { b: Point { x, y }, .. } if x.abs() < 1e-4 && ((y - 20.0).abs() < 1e-4 || (y - 40.0).abs() < 1e-4))
        );
    }

    #[test]
    fn equal_ellipse_axes_follow_source_dimensions_and_undo() {
        let mut doc = Document::from_shapes(vec![
            Shape::Ellipse {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 0.0,
                segments: 64,
            },
            Shape::EllipticArc {
                id: 2,
                center: Point { x: 50.0, y: 0.0 },
                rx: 8.0,
                ry: 6.0,
                rotation: 20.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 32,
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_equal_ellipse_with(1), Ok(true));
        assert!(matches!(
            doc.shapes[1],
            Shape::EllipticArc {
                rx: 20.0,
                ry: 10.0,
                ..
            }
        ));
        assert!(
            doc.set_dimension(DimensionField::RectangleWidth, 50.0)
                .is_err()
        );
        doc.selected = Some(1);
        assert_eq!(
            doc.set_dimension(DimensionField::RectangleWidth, 60.0),
            Ok(())
        );
        assert!(matches!(
            doc.shapes[1],
            Shape::EllipticArc {
                rx: 30.0,
                ry: 10.0,
                ..
            }
        ));
        assert_eq!(
            doc.set_dimension(DimensionField::RectangleHeight, 30.0),
            Ok(())
        );
        assert!(matches!(
            doc.shapes[1],
            Shape::EllipticArc {
                rx: 30.0,
                ry: 15.0,
                ..
            }
        ));
        assert!(doc.undo());
        assert!(matches!(
            doc.shapes[1],
            Shape::EllipticArc {
                rx: 30.0,
                ry: 10.0,
                ..
            }
        ));
        doc.selected = Some(2);
        assert_eq!(doc.toggle_equal_ellipse_with(1), Ok(false));
        assert!(doc.equal_ellipses.is_empty());
        assert_eq!(
            doc.set_dimension(DimensionField::RectangleWidth, 40.0),
            Ok(())
        );
    }

    #[test]
    fn concentric_curves_move_together_and_undo() {
        let mut doc = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
            Shape::Ellipse {
                id: 2,
                center: Point { x: 30.0, y: 10.0 },
                rx: 20.0,
                ry: 5.0,
                rotation: 0.0,
                segments: 64,
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_concentric_with(1), Ok(true));
        assert_eq!(doc.shapes[1].center(), Point { x: 0.0, y: 0.0 });
        assert_eq!(doc.concentric.get(&2), Some(&1));
        assert!(doc.move_selected(Point { x: 5.0, y: -2.0 }));
        assert_eq!(doc.shapes[0].center(), Point { x: 5.0, y: -2.0 });
        assert_eq!(doc.shapes[1].center(), Point { x: 5.0, y: -2.0 });
        assert!(doc.undo());
        assert_eq!(doc.shapes[0].center(), Point { x: 0.0, y: 0.0 });
        doc.selected = Some(1);
        assert!(doc.begin_drag(Point { x: 0.0, y: 0.0 }));
        assert!(doc.drag_to(Point { x: 10.0, y: 10.0 }));
        assert!(doc.finish_drag());
        assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 10.0 });
        assert!(doc.undo());
        doc.selected = Some(2);
        assert_eq!(doc.toggle_concentric_with(1), Ok(false));
        assert!(doc.concentric.is_empty());
    }

    #[test]
    fn parallel_and_perpendicular_lines_follow_source_direction() {
        for kind in [
            LineRelationKind::Parallel,
            LineRelationKind::Perpendicular,
            LineRelationKind::Collinear,
        ] {
            let mut doc = Document::from_shapes(vec![
                Shape::Line {
                    id: 1,
                    a: Point { x: 0.0, y: 0.0 },
                    b: Point { x: 20.0, y: 0.0 },
                },
                Shape::Line {
                    id: 2,
                    a: Point { x: 0.0, y: 30.0 },
                    b: Point { x: 10.0, y: 40.0 },
                },
            ]);
            doc.selected = Some(2);
            assert_eq!(doc.toggle_line_relation_with(1, kind), Ok(true));
            assert_eq!(doc.line_relations.get(&2), Some(&(1, kind)));
            let target = &doc.shapes[1];
            let Shape::Line { a, b, .. } = target else {
                panic!("target line missing")
            };
            let length = a.distance(*b);
            assert!((length - 2.0_f32.sqrt() * 10.0).abs() < 1e-4);
            assert!(match kind {
                LineRelationKind::Parallel => (b.y - a.y).abs() < 1e-4,
                LineRelationKind::Collinear => (b.y - a.y).abs() < 1e-4 && a.y.abs() < 1e-4,
                LineRelationKind::Perpendicular => (b.x - a.x).abs() < 1e-4,
                LineRelationKind::Angle(_) => unreachable!(),
            });
            // Rotating the target turns the source with it.
            assert!(doc.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
            let (Shape::Line { a: sa, b: sb, .. }, Shape::Line { a: ta, b: tb, .. }) =
                (&doc.shapes[0], &doc.shapes[1])
            else {
                panic!()
            };
            let source = (
                (sb.x - sa.x) / sa.distance(*sb),
                (sb.y - sa.y) / sa.distance(*sb),
            );
            let target = (
                (tb.x - ta.x) / ta.distance(*tb),
                (tb.y - ta.y) / ta.distance(*tb),
            );
            let cross = source.0 * target.1 - source.1 * target.0;
            let dot = source.0 * target.0 + source.1 * target.1;
            assert!(match kind {
                LineRelationKind::Perpendicular => dot.abs() < 1e-4,
                _ => cross.abs() < 1e-4,
            });
            assert!(
                (target.1.atan2(target.0).to_degrees()
                    - match kind {
                        LineRelationKind::Perpendicular => 120.0,
                        _ => 30.0,
                    })
                .abs()
                    < 1e-3
            );
            assert!(doc.undo());
            doc.selected = Some(1);
            assert!(doc.rotate_selected(45.0, Point { x: 0.0, y: 0.0 }, false));
            let Shape::Line {
                a: source_a,
                b: source_b,
                ..
            } = doc.shapes[0]
            else {
                panic!()
            };
            let Shape::Line {
                a: target_a,
                b: target_b,
                ..
            } = doc.shapes[1]
            else {
                panic!()
            };
            let sx = source_b.x - source_a.x;
            let sy = source_b.y - source_a.y;
            let tx = target_b.x - target_a.x;
            let ty = target_b.y - target_a.y;
            assert!(match kind {
                LineRelationKind::Parallel => (sx * ty - sy * tx).abs() < 1e-3,
                LineRelationKind::Collinear =>
                    (sx * ty - sy * tx).abs() < 1e-3
                        && ((target_a.x - source_a.x) * sy - (target_a.y - source_a.y) * sx).abs()
                            < 1e-3,
                LineRelationKind::Perpendicular => (sx * tx + sy * ty).abs() < 1e-3,
                LineRelationKind::Angle(_) => unreachable!(),
            });
            assert!(doc.undo());
            assert!(matches!(
                doc.shapes[0],
                Shape::Line {
                    b: Point { x: 20.0, y: 0.0 },
                    ..
                }
            ));
            doc.selected = Some(1);
            assert_eq!(
                doc.toggle_axis_constraint(AxisConstraint::Vertical),
                Ok(true)
            );
            let Shape::Line {
                a: source_a,
                b: source_b,
                ..
            } = doc.shapes[0]
            else {
                panic!()
            };
            let Shape::Line {
                a: target_a,
                b: target_b,
                ..
            } = doc.shapes[1]
            else {
                panic!()
            };
            let sx = source_b.x - source_a.x;
            let sy = source_b.y - source_a.y;
            let tx = target_b.x - target_a.x;
            let ty = target_b.y - target_a.y;
            assert!(match kind {
                LineRelationKind::Parallel => (sx * ty - sy * tx).abs() < 1e-3,
                LineRelationKind::Collinear =>
                    (sx * ty - sy * tx).abs() < 1e-3
                        && ((target_a.x - source_a.x) * sy - (target_a.y - source_a.y) * sx).abs()
                            < 1e-3,
                LineRelationKind::Perpendicular => (sx * tx + sy * ty).abs() < 1e-3,
                LineRelationKind::Angle(_) => unreachable!(),
            });
            assert!(doc.undo());
            doc.selected = Some(2);
            assert_eq!(doc.toggle_line_relation_with(1, kind), Ok(false));
            assert!(doc.line_relations.is_empty());
        }
    }

    #[test]
    fn collinear_lines_follow_source_translation_and_target_stays_on_line() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 5.0, y: 10.0 },
                b: Point { x: 15.0, y: 20.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(
            doc.toggle_line_relation_with(1, LineRelationKind::Collinear),
            Ok(true)
        );
        assert!(
            matches!(doc.shapes[1], Shape::Line { a: Point { y, .. }, b: Point { y: end, .. }, .. } if y.abs() < 1e-4 && end.abs() < 1e-4)
        );
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 0.0, y: 12.0 }));
        assert!(
            matches!(doc.shapes[1], Shape::Line { a: Point { y, .. }, b: Point { y: end, .. }, .. } if (y - 12.0).abs() < 1e-4 && (end - 12.0).abs() < 1e-4)
        );
        doc.selected = Some(2);
        assert!(doc.move_selected(Point { x: 5.0, y: 8.0 }));
        assert!(
            matches!(doc.shapes[1], Shape::Line { a: Point { x, y }, b: Point { y: end, .. }, .. } if (x - 10.0).abs() < 1e-4 && (y - 12.0).abs() < 1e-4 && (end - 12.0).abs() < 1e-4)
        );
        assert!(doc.undo());
        assert!(
            matches!(doc.shapes[1], Shape::Line { a: Point { x, y }, .. } if (x - 5.0).abs() < 1e-4 && (y - 12.0).abs() < 1e-4)
        );
    }

    #[test]
    fn signed_line_angle_drives_target_and_follows_source_rotation() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 30.0 },
                b: Point { x: 10.0, y: 40.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_line_angle_with(1), Ok(true));
        assert!(
            matches!(doc.line_relations.get(&2), Some((1, LineRelationKind::Angle(value))) if (*value - 45.0).abs() < 1e-4)
        );
        assert_eq!(doc.set_line_relation_angle(-30.0), Ok(()));
        assert!(
            matches!(doc.shapes[1], Shape::Line { a, b, .. } if ((b.y - a.y) / (b.x - a.x) + 30.0_f32.to_radians().tan()).abs() < 1e-4)
        );
        doc.selected = Some(1);
        assert!(doc.rotate_selected(90.0, Point { x: 0.0, y: 0.0 }, false));
        assert!(
            matches!(doc.shapes[1], Shape::Line { a, b, .. } if ((b.y - a.y) / (b.x - a.x) - 60.0_f32.to_radians().tan()).abs() < 1e-4)
        );
        assert!(doc.undo());
        assert!(
            matches!(doc.line_relations.get(&2), Some((1, LineRelationKind::Angle(value))) if (*value + 30.0).abs() < 1e-4)
        );
        doc.selected = Some(2);
        assert_eq!(doc.toggle_line_angle_with(1), Ok(false));
    }

    #[test]
    fn signed_radius_difference_drives_circle_and_survives_undo() {
        let mut doc = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                segments: 64,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 60.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_radius_difference_with(1), Ok(true));
        assert_eq!(doc.radius_differences.get(&2), Some(&(1, 10.0)));
        assert_eq!(doc.set_radius_difference(-5.0), Ok(()));
        assert!(
            matches!(doc.shapes[1], Shape::Circle { radius, .. } if (radius - 25.0).abs() < 1e-4)
        );
        assert_eq!(
            doc.set_dimension(DimensionField::CircleDiameter, 30.0),
            Err("Radius is driven by Radial distance".into())
        );
        doc.selected = Some(1);
        assert_eq!(
            doc.set_dimension(DimensionField::CircleDiameter, 60.0),
            Ok(())
        );
        assert!(
            matches!(doc.shapes[1], Shape::Circle { radius, .. } if (radius - 35.0).abs() < 1e-4)
        );
        assert!(doc.undo());
        assert!(
            matches!(doc.shapes[1], Shape::Circle { radius, .. } if (radius - 25.0).abs() < 1e-4)
        );
        doc.selected = Some(2);
        assert_eq!(doc.toggle_radius_difference_with(1), Ok(false));
    }

    #[test]
    fn internal_tangent_circle_and_arc_contacts_follow_radius_and_move() {
        let mut doc = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                segments: 64,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 15.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_internal_tangent_with(1), Ok(true));
        assert_eq!(doc.internal_tangents.get(&2), Some(&1));
        assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 10.0).abs() < 1e-4);
        doc.selected = Some(1);
        assert_eq!(
            doc.set_dimension(DimensionField::CircleDiameter, 50.0),
            Ok(())
        );
        assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 15.0).abs() < 1e-4);
        doc.selected = Some(2);
        assert!(doc.move_selected(Point { x: 5.0, y: 5.0 }));
        assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 15.0).abs() < 1e-4);
        assert!(doc.undo());
        doc.selected = Some(2);
        assert_eq!(doc.toggle_internal_tangent_with(1), Ok(false));

        let mut arcs = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                start_angle: 0.0,
                sweep: std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
            Shape::Arc {
                id: 2,
                center: Point { x: 15.0, y: 0.0 },
                radius: 10.0,
                start_angle: 0.0,
                sweep: std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
        ]);
        arcs.selected = Some(2);
        assert_eq!(arcs.toggle_internal_tangent_with(1), Ok(true));
        assert!((arcs.shapes[0].center().distance(arcs.shapes[1].center()) - 10.0).abs() < 1e-4);
    }

    #[test]
    fn external_tangent_circles_follow_source_and_reject_invalid_arc_contact() {
        let mut doc = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                segments: 64,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 40.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_external_tangent_with(1), Ok(true));
        assert_eq!(doc.external_tangents.get(&2), Some(&1));
        assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 30.0).abs() < 1e-4);
        doc.selected = Some(1);
        assert_eq!(
            doc.set_dimension(DimensionField::CircleDiameter, 50.0),
            Ok(())
        );
        assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 35.0).abs() < 1e-4);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_external_tangent_with(1), Ok(false));

        let mut arc_doc = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                start_angle: 0.0,
                sweep: std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
            Shape::Arc {
                id: 2,
                center: Point { x: 30.0, y: 0.0 },
                radius: 10.0,
                start_angle: std::f64::consts::PI,
                sweep: std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
        ]);
        arc_doc.selected = Some(2);
        assert_eq!(arc_doc.toggle_external_tangent_with(1), Ok(true));
        assert!(
            (arc_doc.shapes[0]
                .center()
                .distance(arc_doc.shapes[1].center())
                - 30.0)
                .abs()
                < 1e-4
        );
    }

    #[test]
    fn line_circle_tangent_moves_selected_shape_and_follows_source() {
        let line = Shape::Line {
            id: 1,
            a: Point { x: -20.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        };
        let circle = Shape::Circle {
            id: 2,
            center: Point { x: 0.0, y: 20.0 },
            radius: 10.0,
            segments: 64,
        };
        let mut doc = Document::from_shapes(vec![line.clone(), circle.clone()]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_external_tangent_with(1), Ok(true));
        assert!((doc.shapes[1].center().y - 10.0).abs() < 1e-4);
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 0.0, y: 5.0 }));
        assert!((doc.shapes[1].center().y - 15.0).abs() < 1e-4);

        let mut reverse = Document::from_shapes(vec![line, circle]);
        reverse.selected = Some(1);
        assert_eq!(reverse.toggle_external_tangent_with(2), Ok(true));
        assert!((reverse.shapes[0].center().y - 10.0).abs() < 1e-4);
        reverse.selected = Some(2);
        assert!(reverse.move_selected(Point { x: 0.0, y: 5.0 }));
        assert!((reverse.shapes[0].center().y - 15.0).abs() < 1e-4);
    }

    #[test]
    fn line_ellipse_tangent_uses_rotated_support_and_arc_sweep() {
        let line = Shape::Line {
            id: 1,
            a: Point { x: -40.0, y: 0.0 },
            b: Point { x: 40.0, y: 0.0 },
        };
        let ellipse = Shape::Ellipse {
            id: 2,
            center: Point { x: 0.0, y: 30.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 0.0,
            segments: 128,
        };
        let mut doc = Document::from_shapes(vec![line.clone(), ellipse.clone()]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_external_tangent_with(1), Ok(true));
        assert!((doc.shapes[1].center().y - 10.0).abs() < 1e-4);
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 0.0, y: 5.0 }));
        assert!((doc.shapes[1].center().y - 15.0).abs() < 1e-4);

        let mut reverse = Document::from_shapes(vec![line, ellipse]);
        reverse.selected = Some(1);
        assert_eq!(reverse.toggle_external_tangent_with(2), Ok(true));
        assert!((reverse.shapes[0].center().y - 20.0).abs() < 1e-4);

        let mut arc_doc = Document::from_shapes(vec![
            reverse.shapes[0].clone(),
            Shape::EllipticArc {
                id: 2,
                center: Point { x: 0.0, y: 30.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 0.0,
                start_angle: std::f64::consts::PI,
                sweep: std::f64::consts::PI,
                segments: 128,
            },
        ]);
        arc_doc.selected = Some(2);
        assert_eq!(arc_doc.toggle_external_tangent_with(1), Ok(true));
    }

    #[test]
    fn midpoint_point_follows_line_move_and_length_edit() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Point {
                id: 2,
                at: Point { x: 3.0, y: 5.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_midpoint_with(1), Ok(true));
        assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 0.0 });
        assert_eq!(
            doc.toggle_fixed_anchor(),
            Err("Remove the point's curve relation before fixing it".into())
        );
        assert_eq!(
            doc.set_selected_point_coordinate(true, 15.0),
            Err("Point is driven by Midpoint".into())
        );
        assert!(!doc.move_selected(Point { x: 5.0, y: 0.0 }));
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 0.0, y: 10.0 }));
        assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 10.0 });
        assert_eq!(doc.set_dimension(DimensionField::LineLength, 40.0), Ok(()));
        assert_eq!(doc.shapes[1].center(), Point { x: 20.0, y: 10.0 });
        assert!(doc.undo());
        assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 10.0 });
        doc.selected = Some(2);
        assert_eq!(doc.toggle_midpoint_with(1), Ok(false));
    }

    #[test]
    fn midpoint_point_follows_axis_constraint() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 10.0, y: 10.0 },
            },
            Shape::Point {
                id: 2,
                at: Point { x: 0.0, y: 0.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_midpoint_with(1), Ok(true));
        doc.selected = Some(1);
        assert_eq!(
            doc.toggle_axis_constraint(AxisConstraint::Horizontal),
            Ok(true)
        );
        assert!(
            doc.shapes[1]
                .center()
                .distance(Document::curve_midpoint(&doc.shapes[0]).unwrap())
                < 1e-4
        );
    }

    #[test]
    fn midpoint_point_follows_circular_and_elliptic_arcs() {
        for curve in [
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 20.0,
                start_angle: 0.0,
                sweep: std::f64::consts::PI,
                segments: 64,
            },
            Shape::EllipticArc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 30.0,
                start_angle: 0.0,
                sweep: std::f64::consts::PI,
                segments: 64,
            },
        ] {
            let mut doc = Document::from_shapes(vec![
                curve,
                Shape::Point {
                    id: 2,
                    at: Point { x: 5.0, y: 5.0 },
                },
            ]);
            doc.selected = Some(2);
            assert_eq!(doc.toggle_midpoint_with(1), Ok(true));
            assert!(
                doc.shapes[1]
                    .center()
                    .distance(Document::curve_midpoint(&doc.shapes[0]).unwrap())
                    < 1e-4
            );
            doc.selected = Some(1);
            assert!(doc.move_selected(Point { x: 10.0, y: 5.0 }));
            assert!(
                doc.shapes[1]
                    .center()
                    .distance(Document::curve_midpoint(&doc.shapes[0]).unwrap())
                    < 1e-4
            );
        }
    }

    #[test]
    fn point_on_curve_projects_to_finite_line_and_tracks_source() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Point {
                id: 2,
                at: Point { x: 30.0, y: 10.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_point_on_curve_with(1), Ok(true));
        assert_eq!(doc.shapes[1].center(), Point { x: 20.0, y: 0.0 });
        assert!(doc.move_selected(Point { x: -10.0, y: 7.0 }));
        assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 0.0 });
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 0.0, y: 5.0 }));
        assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 5.0 });
        doc.selected = Some(2);
        assert_eq!(doc.toggle_point_on_curve_with(1), Ok(false));
    }

    #[test]
    fn point_on_curve_projects_circle_arc_ellipse_and_spline() {
        let curves = [
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                start_angle: 0.0,
                sweep: std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
            Shape::Ellipse {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 30.0,
                segments: 128,
            },
            Shape::EllipticArc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 30.0,
                start_angle: 0.0,
                sweep: std::f64::consts::PI,
                segments: 128,
            },
            Shape::Spline {
                id: 1,
                mode: SplineMode::Control,
                points: vec![
                    Point { x: 0.0, y: 0.0 },
                    Point { x: 10.0, y: 20.0 },
                    Point { x: 20.0, y: 20.0 },
                    Point { x: 30.0, y: 0.0 },
                ],
                closed: false,
            },
        ];
        for curve in curves {
            let mut doc = Document::from_shapes(vec![
                curve,
                Shape::Point {
                    id: 2,
                    at: Point { x: 15.0, y: 15.0 },
                },
            ]);
            doc.selected = Some(2);
            assert_eq!(doc.toggle_point_on_curve_with(1), Ok(true));
            let point = doc.shapes[1].center();
            assert!(
                point.distance(Document::closest_curve_point(&doc.shapes[0], point).unwrap())
                    < 1e-3
            );
            doc.selected = Some(1);
            assert!(doc.move_selected(Point { x: 5.0, y: 3.0 }));
            let point = doc.shapes[1].center();
            assert!(
                point.distance(Document::closest_curve_point(&doc.shapes[0], point).unwrap())
                    < 1e-3
            );
        }
    }

    #[test]
    fn normal_line_endpoint_follows_circle_and_ellipse() {
        for curve in [
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
            Shape::Ellipse {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 20.0,
                ry: 10.0,
                rotation: 30.0,
                segments: 128,
            },
        ] {
            let mut doc = Document::from_shapes(vec![
                curve,
                Shape::Line {
                    id: 2,
                    a: Point { x: 35.0, y: 5.0 },
                    b: Point { x: 15.0, y: 7.0 },
                },
            ]);
            doc.selected = Some(2);
            assert_eq!(
                doc.toggle_normal_with(1, Point { x: 15.0, y: 7.0 }),
                Ok(true)
            );
            assert_eq!(doc.normal_links.get(&2), Some(&(1, 1)));
            assert!(
                doc.toggle_axis_constraint(AxisConstraint::Horizontal)
                    .is_err()
            );
            assert!(
                doc.toggle_fixed_line_endpoint_at(doc.shapes[1].center())
                    .is_err()
            );
            let Shape::Line { b, .. } = doc.shapes[1] else {
                unreachable!()
            };
            assert!(b.distance(Document::closest_curve_point(&doc.shapes[0], b).unwrap()) < 1e-3);
            doc.selected = Some(1);
            assert!(doc.move_selected(Point { x: 5.0, y: 4.0 }));
            let Shape::Line { b, .. } = doc.shapes[1] else {
                unreachable!()
            };
            assert!(b.distance(Document::closest_curve_point(&doc.shapes[0], b).unwrap()) < 1e-3);
            doc.selected = Some(2);
            assert_eq!(doc.toggle_normal_with(1, b), Ok(false));
        }
    }

    #[test]
    fn splitting_line_preserves_endpoint_links_and_axis_but_removes_length() {
        let mut doc = Document::from_shapes(vec![
            Shape::Point {
                id: 1,
                at: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
        ]);
        doc.selected = Some(2);
        doc.point_links.insert((2, 1), 1);
        doc.axis_constraints.insert(2, AxisConstraint::Horizontal);
        doc.driving_dimensions
            .insert((2, DimensionField::LineLength), 20.0);
        assert!(doc.split_selected_line(Point { x: 10.0, y: 9.0 }).is_err());
        assert_eq!(doc.shapes.len(), 2);
        let new_id = doc.split_selected_line(Point { x: 10.0, y: 2.0 }).unwrap();
        assert_eq!(new_id, 3);
        assert_eq!(
            doc.shapes[1],
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 10.0, y: 0.0 }
            }
        );
        assert_eq!(
            doc.shapes[2],
            Shape::Line {
                id: 3,
                a: Point { x: 10.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 }
            }
        );
        assert_eq!(doc.point_links.get(&(3, 1)), Some(&1));
        assert!(!doc.point_links.contains_key(&(2, 1)));
        assert_eq!(
            doc.axis_constraints.get(&3),
            Some(&AxisConstraint::Horizontal)
        );
        assert!(
            !doc.driving_dimensions
                .contains_key(&(2, DimensionField::LineLength))
        );
        assert!(doc.undo());
        assert_eq!(doc.shapes.len(), 2);
        assert_eq!(doc.point_links.get(&(2, 1)), Some(&1));
        assert!(doc.redo());
        assert_eq!(doc.shapes.len(), 3);
    }

    #[test]
    fn splitting_ellipse_and_elliptic_arc_preserves_exact_parameters() {
        let center = Point { x: 5.0, y: -3.0 };
        let mut doc = Document::from_shapes(vec![Shape::Ellipse {
            id: 1,
            center,
            rx: 20.0,
            ry: 10.0,
            rotation: 30.0,
            segments: 64,
        }]);
        doc.selected = Some(1);
        let first = Shape::elliptic_arc_point(center, 20.0, 10.0, 30.0, 0.0, 0.0, 0.0);
        let second = Shape::elliptic_arc_point(
            center,
            20.0,
            10.0,
            30.0,
            std::f64::consts::FRAC_PI_2,
            0.0,
            0.0,
        );
        assert_eq!(
            doc.split_selected_ellipse(first, first),
            Err("Choose two distinct split points".into())
        );
        assert_eq!(doc.split_selected_ellipse(first, second), Ok(2));
        assert_eq!(doc.shared_ellipses.get(&2), Some(&1));
        assert!(
            matches!(doc.shapes[0], Shape::EllipticArc { rx: 20.0, ry: 10.0, rotation: 30.0, sweep, .. } if (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-5)
        );
        assert!(
            matches!(doc.shapes[1], Shape::EllipticArc { sweep, .. } if (sweep - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-5)
        );
        let middle =
            Shape::elliptic_arc_point(center, 20.0, 10.0, 30.0, std::f64::consts::PI, 0.0, 0.0);
        assert_eq!(doc.split_selected_curve(middle), Ok(3));
        assert_eq!(doc.shared_ellipses.get(&3), Some(&1));
        assert_eq!(doc.shapes.len(), 3);
        doc.selected = Some(2);
        assert!(doc.move_selected(Point { x: 7.0, y: 2.0 }));
        assert!(
            doc.shapes
                .iter()
                .all(|shape| shape.center() == Point { x: 12.0, y: -1.0 })
        );
        assert_eq!(
            doc.set_dimension(DimensionField::RectangleWidth, 50.0),
            Ok(())
        );
        assert!(doc.shapes.iter().all(
            |shape| matches!(shape, Shape::EllipticArc { rx, .. } if (*rx - 25.0).abs() < 1e-4)
        ));
        assert_eq!(doc.set_dimension(DimensionField::Rotation, 45.0), Ok(()));
        assert!(doc.shapes.iter().all(|shape| matches!(shape, Shape::EllipticArc { rotation, .. } if (*rotation - 45.0).abs() < 1e-4)));
        assert!(doc.undo());
        assert!(doc.undo());
        assert!(doc.undo());
        assert!(doc.undo());
        assert_eq!(doc.shapes.len(), 2);
        assert!(doc.undo());
        assert!(matches!(doc.shapes[0], Shape::Ellipse { .. }));
    }

    #[test]
    fn elliptic_arc_bounds_and_intersections_follow_retained_sweep() {
        let arc = Shape::EllipticArc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 0.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        };
        let line = Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: -20.0 },
            b: Point { x: 0.0, y: 20.0 },
        };
        let contacts = finite_intersections(&arc, &line);
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].distance(Point { x: 0.0, y: 10.0 }) < 1e-5);
        assert!(arc.hit(Point { x: 0.0, y: 10.0 }, 0.1));
        assert!(!arc.hit(Point { x: 0.0, y: -10.0 }, 0.1));
        let (min, max) = arc.bounds().unwrap();
        assert!((min.x + 20.0).abs() < 1e-5 && (max.x - 20.0).abs() < 1e-5);
        assert!(min.y.abs() < 1e-5 && (max.y - 10.0).abs() < 1e-5);
    }

    #[test]
    fn spline_authoring_validates_modes_and_is_undoable() {
        let mut document = Document::default();
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: 0.0, y: 20.0 },
        ];
        assert!(
            document
                .add_spline(SplineMode::Control, points[..3].to_vec(), false)
                .is_err()
        );
        assert!(document.shapes.is_empty());
        assert_eq!(
            document.add_spline(SplineMode::Fit, points.clone(), true),
            Ok(1)
        );
        assert!(matches!(
            document.shapes[0],
            Shape::Spline {
                mode: SplineMode::Fit,
                closed: true,
                ..
            }
        ));
        assert_eq!(
            document.add_spline(SplineMode::Control, points.clone(), false),
            Ok(2)
        );
        assert!(document.undo());
        assert_eq!(document.shapes.len(), 1);
        assert!(document.redo());
        assert_eq!(document.shapes.len(), 2);
    }

    #[test]
    fn selected_spline_handle_drag_moves_only_that_point_and_undoes() {
        let original = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 20.0 },
            Point { x: 20.0, y: -20.0 },
            Point { x: 30.0, y: 0.0 },
        ];
        let mut document = Document::from_shapes(vec![Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: original.clone(),
            closed: false,
        }]);
        document.selected = Some(1);
        document.select_at(original[1]);
        assert_eq!(document.selected_spline_point, Some(1));
        assert!(document.begin_drag(original[1]));
        assert!(document.drag_to(Point { x: 15.0, y: 25.0 }));
        assert!(document.finish_drag());
        assert!(matches!(&document.shapes[0], Shape::Spline { points, .. }
            if points[0] == original[0] && points[1] == Point { x: 15.0, y: 25.0 } && points[2] == original[2]));
        assert!(document.undo());
        assert!(matches!(&document.shapes[0], Shape::Spline { points, .. } if *points == original));

        let fit = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 10.0, y: 10.0 },
        ];
        let mut periodic = Document::from_shapes(vec![Shape::Spline {
            id: 1,
            mode: SplineMode::Fit,
            points: fit.clone(),
            closed: true,
        }]);
        periodic.selected = Some(1);
        periodic.select_at(fit[1]);
        assert!(periodic.begin_drag(fit[1]));
        assert!(!periodic.drag_to(fit[0]));
        assert!(periodic.drag_to(Point { x: 15.0, y: 0.0 }));
        assert!(periodic.finish_drag());
        assert!(
            matches!(&periodic.shapes[0], Shape::Spline { closed: true, points, .. } if points[1].x == 15.0)
        );
    }

    #[test]
    fn selected_spline_point_coordinates_edit_independently_and_undo() {
        let original = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 20.0 },
            Point { x: 20.0, y: -20.0 },
            Point { x: 30.0, y: 0.0 },
        ];
        let mut document = Document::from_shapes(vec![Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: original.clone(),
            closed: false,
        }]);
        document.selected = Some(1);
        assert!(document.set_selected_spline_coordinate(true, 15.0).is_err());
        document.selected_spline_point = Some(1);
        assert!(
            document
                .set_selected_spline_coordinate(true, f32::NAN)
                .is_err()
        );
        assert_eq!(document.set_selected_spline_coordinate(true, 15.0), Ok(()));
        assert_eq!(document.set_selected_spline_coordinate(false, 25.0), Ok(()));
        assert!(matches!(&document.shapes[0], Shape::Spline { points, .. }
            if points[1] == Point { x: 15.0, y: 25.0 } && points[0] == original[0] && points[2] == original[2]));
        assert!(document.undo());
        assert!(matches!(&document.shapes[0], Shape::Spline { points, .. }
            if points[1] == Point { x: 15.0, y: 20.0 }));
        assert!(document.undo());
        assert!(matches!(&document.shapes[0], Shape::Spline { points, .. } if *points == original));
        assert!(document.redo());
        assert!(document.redo());
        assert!(matches!(&document.shapes[0], Shape::Spline { points, .. }
            if points[1] == Point { x: 15.0, y: 25.0 }));
    }

    #[test]
    fn splitting_open_spline_preserves_curve_construction_and_history() {
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 30.0 },
            Point { x: 20.0, y: -30.0 },
            Point { x: 30.0, y: 0.0 },
        ];
        let curve = spline::beziers(SplineMode::Control, &points, false).unwrap()[0];
        let mut document = Document::from_shapes(vec![Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: points.clone(),
            closed: false,
        }]);
        document.selected = Some(1);
        document.construction.insert(1);
        assert!(
            document
                .split_selected_curve(Point { x: 100.0, y: 100.0 })
                .is_err()
        );
        assert_eq!(document.shapes.len(), 1);
        let middle = spline::point(&curve, 0.5);
        assert_eq!(document.split_selected_curve(middle), Ok(2));
        assert_eq!(document.selected, Some(2));
        assert!(document.construction.contains(&2));
        let Shape::Spline { points: first, .. } = &document.shapes[0] else {
            panic!()
        };
        let Shape::Spline { points: second, .. } = &document.shapes[1] else {
            panic!()
        };
        assert!(first.last().unwrap().distance(second[0]) < 1e-4);
        let pieces = [
            spline::beziers(SplineMode::Control, first, false).unwrap()[0],
            spline::beziers(SplineMode::Control, second, false).unwrap()[0],
        ];
        for step in 0..=20 {
            let t = step as f32 / 20.0;
            let piece = if t <= 0.5 {
                spline::point(&pieces[0], t * 2.0)
            } else {
                spline::point(&pieces[1], (t - 0.5) * 2.0)
            };
            assert!(piece.distance(spline::point(&curve, t)) < 1e-3);
        }
        assert!(document.undo());
        assert!(
            matches!(&document.shapes[0], Shape::Spline { points: restored, .. } if *restored == points)
        );
        assert!(document.redo());
        assert_eq!(document.shapes.len(), 2);
    }

    #[test]
    fn splitting_periodic_fit_spline_opens_one_control_chain() {
        let points = vec![
            Point { x: -20.0, y: -20.0 },
            Point { x: 20.0, y: -20.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: -20.0, y: 20.0 },
        ];
        let original = spline::beziers(SplineMode::Fit, &points, true).unwrap();
        let mut document = Document::from_shapes(vec![Shape::Spline {
            id: 1,
            mode: SplineMode::Fit,
            points: points.clone(),
            closed: true,
        }]);
        document.selected = Some(1);
        document.construction.insert(1);
        assert_eq!(
            document.split_selected_curve(spline::point(&original[1], 0.5)),
            Ok(1)
        );
        let Shape::Spline {
            mode,
            points: opened,
            closed,
            ..
        } = &document.shapes[0]
        else {
            panic!()
        };
        assert_eq!(*mode, SplineMode::Control);
        assert!(!closed);
        assert!(opened[0].distance(*opened.last().unwrap()) < 1e-4);
        assert!(document.construction.contains(&1));
        assert_eq!(spline::beziers(*mode, opened, false).unwrap().len(), 5);
        assert!(document.undo());
        assert!(
            matches!(&document.shapes[0], Shape::Spline { mode: SplineMode::Fit, points: restored, closed: true, .. } if *restored == points)
        );
    }

    #[test]
    fn trimming_spline_between_lines_keeps_exact_outer_cubic_pieces() {
        let points = vec![
            Point { x: -20.0, y: 0.0 },
            Point { x: -20.0, y: 20.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: 20.0, y: 0.0 },
        ];
        let original = spline::beziers(SplineMode::Control, &points, false).unwrap()[0];
        let mut document = Document::from_shapes(vec![
            Shape::Spline {
                id: 1,
                mode: SplineMode::Control,
                points: points.clone(),
                closed: false,
            },
            Shape::Line {
                id: 2,
                a: Point { x: -10.0, y: -5.0 },
                b: Point { x: -10.0, y: 25.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: 10.0, y: -5.0 },
                b: Point { x: 10.0, y: 25.0 },
            },
        ]);
        document.selected = Some(1);
        document.construction.insert(1);
        assert_eq!(
            document.trim_selected_curve(spline::point(&original, 0.5)),
            Ok(2)
        );
        assert_eq!(document.shapes.len(), 4);
        assert!(document.construction.contains(&4));
        let Shape::Spline { points: first, .. } = &document.shapes[0] else {
            panic!()
        };
        let Shape::Spline { points: second, .. } = &document.shapes[1] else {
            panic!()
        };
        let first_curve = spline::beziers(SplineMode::Control, first, false).unwrap()[0];
        let second_curve = spline::beziers(SplineMode::Control, second, false).unwrap()[0];
        assert_eq!(first[0], points[0]);
        assert_eq!(*second.last().unwrap(), points[3]);
        assert!(first.last().unwrap().x < -9.9);
        assert!(second[0].x > 9.9);
        let left_cut = spline::line_intersections(
            &[original],
            Point { x: -10.0, y: -5.0 },
            Point { x: -10.0, y: 25.0 },
        )[0]
        .1;
        let right_cut = spline::line_intersections(
            &[original],
            Point { x: 10.0, y: -5.0 },
            Point { x: 10.0, y: 25.0 },
        )[0]
        .1;
        assert!(
            spline::point(&first_curve, 0.5).distance(spline::point(&original, left_cut * 0.5))
                < 1e-4
        );
        assert!(
            spline::point(&second_curve, 0.5)
                .distance(spline::point(&original, (right_cut + 1.0) * 0.5))
                < 1e-4
        );
        assert!(document.undo());
        assert_eq!(document.shapes.len(), 3);
        assert!(
            matches!(&document.shapes[0], Shape::Spline { points: restored, .. } if *restored == points)
        );
        assert!(document.redo());
        assert_eq!(document.shapes.len(), 4);
    }

    #[test]
    fn trimming_closed_fit_spline_opens_retained_wraparound() {
        let points = vec![
            Point { x: -20.0, y: -20.0 },
            Point { x: 20.0, y: -20.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: -20.0, y: 20.0 },
        ];
        let original = spline::beziers(SplineMode::Fit, &points, true).unwrap();
        let mut document = Document::from_shapes(vec![
            Shape::Spline {
                id: 1,
                mode: SplineMode::Fit,
                points: points.clone(),
                closed: true,
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: -30.0 },
                b: Point { x: 0.0, y: 30.0 },
            },
        ]);
        document.selected = Some(1);
        assert_eq!(
            document.trim_selected_curve(spline::point(&original[1], 0.5)),
            Ok(1)
        );
        let Shape::Spline {
            mode,
            points: retained,
            closed,
            ..
        } = &document.shapes[0]
        else {
            panic!()
        };
        assert_eq!(*mode, SplineMode::Control);
        assert!(!closed);
        assert!(
            retained
                .first()
                .unwrap()
                .distance(*retained.last().unwrap())
                > 1.0
        );
        assert!(document.undo());
        assert!(
            matches!(&document.shapes[0], Shape::Spline { mode: SplineMode::Fit, points: restored, closed: true, .. } if *restored == points)
        );
    }

    #[test]
    fn circle_boundaries_trim_spline_and_arc_sweep_filters_contacts() {
        let points = vec![
            Point { x: -20.0, y: 0.0 },
            Point { x: -20.0, y: 20.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: 20.0, y: 0.0 },
        ];
        let spline = Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points,
            closed: false,
        };
        let circle = Shape::Circle {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            radius: 17.0,
            segments: 64,
        };
        let hits = finite_intersections(&spline, &circle);
        assert_eq!(hits.len(), 2);
        assert_eq!(finite_intersections(&circle, &spline).len(), 2);
        let upper_arc = Shape::Arc {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            radius: 17.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        };
        assert_eq!(finite_intersections(&spline, &upper_arc).len(), 2);
        let lower_arc = Shape::Arc {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            radius: 17.0,
            start_angle: std::f64::consts::PI,
            sweep: std::f64::consts::PI,
            segments: 64,
        };
        assert!(finite_intersections(&spline, &lower_arc).is_empty());
        let mut document = Document::from_shapes(vec![spline, circle]);
        document.selected = Some(1);
        assert_eq!(
            document.trim_selected_curve(Point { x: 0.0, y: 15.0 }),
            Ok(2)
        );
        assert_eq!(document.shapes.len(), 3);
        assert!(document.undo());
        assert_eq!(document.shapes.len(), 2);
    }

    #[test]
    fn rotated_ellipse_and_elliptic_arc_intersect_and_trim_spline() {
        let spline = Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: vec![
                Point { x: -20.0, y: 0.0 },
                Point { x: -10.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                Point { x: 20.0, y: 0.0 },
            ],
            closed: false,
        };
        let ellipse = Shape::Ellipse {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 90.0,
            segments: 64,
        };
        assert_eq!(finite_intersections(&spline, &ellipse).len(), 2);
        assert_eq!(finite_intersections(&ellipse, &spline).len(), 2);
        let arc = Shape::EllipticArc {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 90.0,
            start_angle: -std::f64::consts::PI,
            sweep: std::f64::consts::PI,
            segments: 64,
        };
        assert_eq!(finite_intersections(&spline, &arc).len(), 1);
        assert_eq!(finite_intersections(&arc, &spline).len(), 1);
        let mut document = Document::from_shapes(vec![spline, ellipse]);
        document.selected = Some(1);
        assert_eq!(
            document.trim_selected_curve(Point { x: 0.0, y: 0.0 }),
            Ok(2)
        );
        assert_eq!(document.shapes.len(), 3);
        assert!(document.undo());
        assert_eq!(document.shapes.len(), 2);
    }

    #[test]
    fn another_spline_can_bound_trim_and_intersection_snap() {
        let arch = Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: vec![
                Point { x: -20.0, y: 0.0 },
                Point { x: -20.0, y: 20.0 },
                Point { x: 20.0, y: 20.0 },
                Point { x: 20.0, y: 0.0 },
            ],
            closed: false,
        };
        let horizontal = Shape::Spline {
            id: 2,
            mode: SplineMode::Control,
            points: vec![
                Point { x: -30.0, y: 10.0 },
                Point { x: -10.0, y: 10.0 },
                Point { x: 10.0, y: 10.0 },
                Point { x: 30.0, y: 10.0 },
            ],
            closed: false,
        };
        let hits = finite_intersections(&arch, &horizontal);
        assert_eq!(hits.len(), 2);
        assert_eq!(finite_intersections(&horizontal, &arch).len(), 2);
        let mut document = Document::from_shapes(vec![arch, horizontal]);
        document.selected = Some(1);
        assert_eq!(
            document.trim_selected_curve(Point { x: 0.0, y: 15.0 }),
            Ok(2)
        );
        assert_eq!(document.shapes.len(), 3);
        assert!(document.undo());
        assert_eq!(document.shapes.len(), 2);
    }

    #[test]
    fn spline_trim_work_limit_leaves_selected_geometry_untouched() {
        let make_points = |offset: f32| {
            let mut points = vec![Point { x: 0.0, y: offset }];
            for _ in 0..65 {
                points.extend([
                    Point {
                        x: 10.0,
                        y: 20.0 + offset,
                    },
                    Point {
                        x: -10.0,
                        y: 20.0 + offset,
                    },
                    Point { x: 0.0, y: offset },
                ]);
            }
            points
        };
        let mut document = Document::from_shapes(vec![
            Shape::Spline {
                id: 1,
                mode: SplineMode::Control,
                points: make_points(0.0),
                closed: false,
            },
            Shape::Spline {
                id: 2,
                mode: SplineMode::Control,
                points: make_points(1.0),
                closed: false,
            },
        ]);
        let original = document.shapes.clone();
        document.selected = Some(1);
        assert!(
            document
                .trim_selected_curve(Point { x: 0.0, y: 15.0 })
                .is_err()
        );
        assert_eq!(document.shapes, original);
    }

    #[test]
    fn trim_ellipse_and_elliptic_arc_keep_exact_curve_fragments() {
        let center = Point { x: 0.0, y: 0.0 };
        let lines = [5.0, -5.0].map(|x| Shape::Line {
            id: if x > 0.0 { 2 } else { 3 },
            a: Point { x, y: -10.0 },
            b: Point { x, y: 10.0 },
        });
        let mut ellipse = Document::from_shapes(vec![
            Shape::Ellipse {
                id: 1,
                center,
                rx: 10.0,
                ry: 5.0,
                rotation: 0.0,
                segments: 64,
            },
            lines[0].clone(),
            lines[1].clone(),
        ]);
        ellipse.selected = Some(1);
        ellipse.construction.insert(1);
        assert_eq!(ellipse.trim_selected_curve(Point { x: 0.0, y: 5.0 }), Ok(1));
        assert!(
            matches!(ellipse.shapes[0], Shape::EllipticArc { rx: 10.0, ry: 5.0, start_angle, sweep, .. }
            if (start_angle - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6
                && (sweep - 5.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
        );
        assert!(ellipse.construction.contains(&1));
        assert!(ellipse.undo());
        assert!(matches!(ellipse.shapes[0], Shape::Ellipse { .. }));

        let mut arc = Document::from_shapes(vec![
            Shape::EllipticArc {
                id: 1,
                center,
                rx: 10.0,
                ry: 5.0,
                rotation: 0.0,
                start_angle: 0.0,
                sweep: std::f64::consts::PI,
                segments: 64,
            },
            lines[0].clone(),
            lines[1].clone(),
        ]);
        arc.selected = Some(1);
        assert_eq!(arc.trim_selected_curve(Point { x: 0.0, y: 5.0 }), Ok(2));
        assert!(
            matches!(arc.shapes[0], Shape::EllipticArc { sweep, .. } if (sweep - std::f64::consts::PI / 3.0).abs() < 1e-6)
        );
        assert!(
            matches!(arc.shapes[1], Shape::EllipticArc { start_angle, sweep, .. }
            if (start_angle - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6
                && (sweep - std::f64::consts::PI / 3.0).abs() < 1e-6)
        );
        assert!(arc.undo());
        assert_eq!(arc.shapes.len(), 3);
    }

    #[test]
    fn extend_elliptic_arc_reaches_next_boundary_in_both_directions() {
        let center = Point { x: 0.0, y: 0.0 };
        let boundary = Shape::Line {
            id: 2,
            a: Point { x: -5.0, y: -10.0 },
            b: Point { x: -5.0, y: 10.0 },
        };
        let mut doc = Document::from_shapes(vec![
            Shape::EllipticArc {
                id: 1,
                center,
                rx: 10.0,
                ry: 5.0,
                rotation: 0.0,
                start_angle: 0.0,
                sweep: std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
            boundary.clone(),
        ]);
        doc.selected = Some(1);
        assert!(doc.extend_selected_curve(Point { x: 0.0, y: 5.0 }).is_ok());
        assert!(
            matches!(doc.shapes[0], Shape::EllipticArc { start_angle, sweep, .. }
            if start_angle.abs() < 1e-8 && (sweep - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
        );
        assert!(doc.undo());
        assert!(matches!(doc.shapes[0], Shape::EllipticArc { sweep, .. }
            if (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-8));

        let mut clockwise = Document::from_shapes(vec![
            Shape::EllipticArc {
                id: 1,
                center,
                rx: 10.0,
                ry: 5.0,
                rotation: 0.0,
                start_angle: 0.0,
                sweep: -std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
            boundary,
        ]);
        clockwise.selected = Some(1);
        assert!(
            clockwise
                .extend_selected_curve(Point { x: 0.0, y: -5.0 })
                .is_ok()
        );
        assert!(
            matches!(clockwise.shapes[0], Shape::EllipticArc { sweep, .. }
            if (sweep + 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
        );
    }

    #[test]
    fn splitting_arc_keeps_center_radius_sweep_and_construction() {
        let center = Point { x: 0.0, y: 0.0 };
        let mut doc = Document::from_shapes(vec![Shape::Arc {
            id: 1,
            center,
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        }]);
        doc.selected = Some(1);
        doc.construction.insert(1);
        assert_eq!(doc.set_dimension(DimensionField::ArcSweep, 180.0), Ok(()));
        assert!(
            doc.split_selected_curve(Point { x: 0.0, y: -10.0 })
                .is_err()
        );
        assert_eq!(doc.split_selected_curve(Point { x: 0.0, y: 10.0 }), Ok(2));
        assert!(
            !doc.driving_dimensions
                .contains_key(&(1, DimensionField::ArcSweep))
        );
        assert!(
            matches!(doc.shapes[0], Shape::Arc { center: c, radius: 10.0, sweep, .. } if c == center && (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9)
        );
        assert!(
            matches!(doc.shapes[1], Shape::Arc { start_angle, sweep, .. } if (start_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9 && (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9)
        );
        assert!(doc.construction.contains(&2));
        assert!(doc.undo());
        assert_eq!(doc.shapes.len(), 1);
        assert_eq!(
            doc.driving_dimensions.get(&(1, DimensionField::ArcSweep)),
            Some(&180.0)
        );
        assert!(doc.redo());
        assert_eq!(doc.shapes.len(), 2);
        assert!(
            !doc.driving_dimensions
                .contains_key(&(1, DimensionField::ArcSweep))
        );
        let mut clockwise = Document::from_shapes(vec![Shape::Arc {
            id: 1,
            center,
            radius: 10.0,
            start_angle: 0.0,
            sweep: -std::f64::consts::PI,
            segments: 64,
        }]);
        clockwise.selected = Some(1);
        assert_eq!(
            clockwise.split_selected_arc(Point { x: 0.0, y: -10.0 }),
            Ok(2)
        );
        assert!(
            matches!(clockwise.shapes[1], Shape::Arc { start_angle, sweep, .. } if (start_angle + std::f64::consts::FRAC_PI_2).abs() < 1e-9 && (sweep + std::f64::consts::FRAC_PI_2).abs() < 1e-9)
        );
    }

    #[test]
    fn splitting_circle_at_two_points_creates_complementary_arcs() {
        let mut doc = Document::from_shapes(vec![Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        }]);
        doc.selected = Some(1);
        doc.construction.insert(1);
        doc.driving_dimensions
            .insert((1, DimensionField::CircleDiameter), 20.0);
        assert!(
            doc.split_selected_circle(Point { x: 10.0, y: 0.0 }, Point { x: 10.0, y: 0.0 })
                .is_err()
        );
        assert_eq!(
            doc.split_selected_circle(Point { x: 0.0, y: 10.0 }, Point { x: 10.0, y: 0.0 }),
            Ok(2)
        );
        assert!(
            matches!(doc.shapes[0], Shape::Arc { start_angle, sweep, .. } if start_angle.abs() < 1e-9 && (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9)
        );
        assert!(
            matches!(doc.shapes[1], Shape::Arc { start_angle, sweep, .. } if (start_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9 && (sweep - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9)
        );
        assert!(doc.construction.contains(&2));
        assert!(
            !doc.driving_dimensions
                .contains_key(&(1, DimensionField::CircleDiameter))
        );
        assert!(doc.undo());
        assert!(matches!(doc.shapes[0], Shape::Circle { .. }));
        assert!(doc.redo());
        assert_eq!(doc.shapes.len(), 2);
    }

    #[test]
    fn trim_line_removes_clicked_span_and_retains_endpoint_relations() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 40.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 10.0, y: -10.0 },
                b: Point { x: 10.0, y: 10.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: 30.0, y: -10.0 },
                b: Point { x: 30.0, y: 10.0 },
            },
            Shape::Point {
                id: 4,
                at: Point { x: 40.0, y: 0.0 },
            },
        ]);
        doc.selected = Some(1);
        doc.axis_constraints.insert(1, AxisConstraint::Horizontal);
        doc.construction.insert(1);
        doc.point_links.insert((1, 1), 4);
        doc.driving_dimensions
            .insert((1, DimensionField::LineLength), 40.0);
        assert_eq!(doc.trim_selected_line(Point { x: 20.0, y: 1.0 }), Ok(2));
        assert_eq!(
            doc.shapes[0],
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 10.0, y: 0.0 }
            }
        );
        assert_eq!(
            doc.shapes[1],
            Shape::Line {
                id: 5,
                a: Point { x: 30.0, y: 0.0 },
                b: Point { x: 40.0, y: 0.0 }
            }
        );
        assert_eq!(doc.point_links.get(&(5, 1)), Some(&4));
        assert!(doc.construction.contains(&5));
        assert_eq!(
            doc.axis_constraints.get(&5),
            Some(&AxisConstraint::Horizontal)
        );
        assert!(
            !doc.driving_dimensions
                .contains_key(&(1, DimensionField::LineLength))
        );
        assert!(doc.undo());
        assert_eq!(doc.shapes.len(), 4);
        assert_eq!(doc.point_links.get(&(1, 1)), Some(&4));
        assert!(doc.redo());
        assert_eq!(doc.shapes.len(), 5);
    }

    #[test]
    fn trim_arc_removes_section_between_crossings_and_preserves_sweep_direction() {
        let center = Point { x: 0.0, y: 0.0 };
        let arc = Shape::Arc {
            id: 1,
            center,
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        };
        let boundaries = [
            Shape::Line {
                id: 2,
                a: Point { x: 5.0, y: 0.0 },
                b: Point { x: 5.0, y: 20.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: -5.0, y: 0.0 },
                b: Point { x: -5.0, y: 20.0 },
            },
        ];
        let mut doc =
            Document::from_shapes(vec![arc, boundaries[0].clone(), boundaries[1].clone()]);
        doc.selected = Some(1);
        doc.construction.insert(1);
        assert_eq!(doc.trim_selected_curve(Point { x: 0.0, y: 10.0 }), Ok(2));
        assert!(
            matches!(doc.shapes[0], Shape::Arc { start_angle, sweep, .. } if start_angle.abs() < 1e-9 && (sweep - std::f64::consts::FRAC_PI_3).abs() < 1e-6)
        );
        assert!(
            matches!(doc.shapes[1], Shape::Arc { start_angle, sweep, .. } if (start_angle - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6 && (sweep - std::f64::consts::FRAC_PI_3).abs() < 1e-6)
        );
        assert!(doc.construction.contains(&4));
        assert!(doc.undo());
        assert_eq!(doc.shapes.len(), 3);
        assert!(doc.redo());
        assert_eq!(doc.shapes.len(), 4);
        let mut clockwise = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center,
                radius: 10.0,
                start_angle: 0.0,
                sweep: -std::f64::consts::PI,
                segments: 64,
            },
            Shape::Line {
                id: 2,
                a: Point { x: 5.0, y: -20.0 },
                b: Point { x: 5.0, y: 0.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: -5.0, y: -20.0 },
                b: Point { x: -5.0, y: 0.0 },
            },
        ]);
        clockwise.selected = Some(1);
        assert_eq!(
            clockwise.trim_selected_arc(Point { x: 0.0, y: -10.0 }),
            Ok(2)
        );
        assert!(
            matches!(clockwise.shapes[0], Shape::Arc { sweep, .. } if (sweep + std::f64::consts::FRAC_PI_3).abs() < 1e-6)
        );
    }

    #[test]
    fn trim_circle_keeps_complementary_arc_and_drops_diameter_dimension() {
        let mut doc = Document::from_shapes(vec![
            Shape::Circle {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                segments: 64,
            },
            Shape::Line {
                id: 2,
                a: Point { x: 5.0, y: -20.0 },
                b: Point { x: 5.0, y: 20.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: -5.0, y: -20.0 },
                b: Point { x: -5.0, y: 20.0 },
            },
        ]);
        doc.selected = Some(1);
        doc.construction.insert(1);
        doc.driving_dimensions
            .insert((1, DimensionField::CircleDiameter), 20.0);
        assert_eq!(doc.trim_selected_curve(Point { x: 0.0, y: 10.0 }), Ok(1));
        assert!(
            matches!(doc.shapes[0], Shape::Arc { radius: 10.0, start_angle, sweep, .. } if (start_angle - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6 && (sweep - 5.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
        );
        assert!(doc.construction.contains(&1));
        assert!(
            !doc.driving_dimensions
                .contains_key(&(1, DimensionField::CircleDiameter))
        );
        assert!(doc.undo());
        assert!(matches!(doc.shapes[0], Shape::Circle { .. }));
        assert!(doc.redo());
        assert!(matches!(doc.shapes[0], Shape::Arc { .. }));
        let mut isolated = Document::from_shapes(vec![Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        }]);
        isolated.selected = Some(1);
        assert_eq!(
            isolated.trim_selected_circle(Point { x: 0.0, y: 10.0 }),
            Ok(0)
        );
        assert!(isolated.shapes.is_empty());
    }

    #[test]
    fn trim_line_with_no_crossings_removes_it_and_one_crossing_keeps_one_side() {
        let mut doc = Document::from_shapes(vec![Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        }]);
        doc.selected = Some(1);
        assert_eq!(doc.trim_selected_line(Point { x: 10.0, y: 0.0 }), Ok(0));
        assert!(doc.shapes.is_empty());
        assert!(doc.undo());
        doc.shapes.push(Shape::Line {
            id: 2,
            a: Point { x: 8.0, y: -5.0 },
            b: Point { x: 8.0, y: 5.0 },
        });
        doc.selected = Some(1);
        assert_eq!(doc.trim_selected_line(Point { x: 3.0, y: 0.0 }), Ok(1));
        assert_eq!(
            doc.shapes[0],
            Shape::Line {
                id: 1,
                a: Point { x: 8.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 }
            }
        );
    }

    #[test]
    fn extend_line_finds_nearest_boundary_in_endpoint_direction() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 10.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 20.0, y: -5.0 },
                b: Point { x: 20.0, y: 5.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: 30.0, y: -5.0 },
                b: Point { x: 30.0, y: 5.0 },
            },
            Shape::Line {
                id: 4,
                a: Point { x: -15.0, y: -5.0 },
                b: Point { x: -15.0, y: 5.0 },
            },
            Shape::Point {
                id: 5,
                at: Point { x: 10.0, y: 0.0 },
            },
        ]);
        doc.selected = Some(1);
        doc.point_links.insert((1, 1), 5);
        doc.driving_dimensions
            .insert((1, DimensionField::LineLength), 10.0);
        doc.axis_constraints.insert(1, AxisConstraint::Horizontal);
        assert_eq!(
            doc.extend_selected_line(Point { x: 9.0, y: 0.0 }),
            Ok(Point { x: 20.0, y: 0.0 })
        );
        assert_eq!(
            doc.shapes[0],
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 }
            }
        );
        assert!(!doc.point_links.contains_key(&(1, 1)));
        assert!(
            !doc.driving_dimensions
                .contains_key(&(1, DimensionField::LineLength))
        );
        assert_eq!(
            doc.axis_constraints.get(&1),
            Some(&AxisConstraint::Horizontal)
        );
        assert!(doc.undo());
        assert_eq!(doc.point_links.get(&(1, 1)), Some(&5));
        doc.selected = Some(1);
        assert_eq!(
            doc.extend_selected_line(Point { x: 1.0, y: 0.0 }),
            Ok(Point { x: -15.0, y: 0.0 })
        );
        assert!(matches!(
            doc.shapes[0],
            Shape::Line {
                a: Point { x: -15.0, y: 0.0 },
                ..
            }
        ));
        doc.selected = Some(2);
        assert!(doc.extend_selected_line(Point { x: 20.0, y: 0.0 }).is_err());
    }

    #[test]
    fn extend_arc_reaches_nearest_boundary_outside_its_sweep() {
        let center = Point { x: 0.0, y: 0.0 };
        let mut doc = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center,
                radius: 10.0,
                start_angle: 0.0,
                sweep: std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
            Shape::Line {
                id: 2,
                a: Point { x: -5.0, y: 0.0 },
                b: Point { x: -5.0, y: 20.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: -8.0, y: 0.0 },
                b: Point { x: -8.0, y: 20.0 },
            },
            Shape::Line {
                id: 4,
                a: Point { x: 5.0, y: -20.0 },
                b: Point { x: 5.0, y: 0.0 },
            },
        ]);
        doc.selected = Some(1);
        let target = doc
            .extend_selected_curve(Point { x: 0.0, y: 10.0 })
            .unwrap();
        assert!((target.x + 5.0).abs() < 1e-4 && (target.y - 75.0_f32.sqrt()).abs() < 1e-4);
        assert!(
            matches!(doc.shapes[0], Shape::Arc { sweep, .. } if (sweep - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6)
        );
        assert!(doc.undo());
        doc.selected = Some(1);
        doc.extend_selected_arc(Point { x: 10.0, y: 0.0 }).unwrap();
        assert!(
            matches!(doc.shapes[0], Shape::Arc { start_angle, sweep, .. } if (start_angle + std::f64::consts::FRAC_PI_3).abs() < 1e-6 && (sweep - 5.0 * std::f64::consts::PI / 6.0).abs() < 1e-6)
        );
        let mut clockwise = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center,
                radius: 10.0,
                start_angle: 0.0,
                sweep: -std::f64::consts::FRAC_PI_2,
                segments: 64,
            },
            Shape::Line {
                id: 2,
                a: Point { x: -5.0, y: -20.0 },
                b: Point { x: -5.0, y: 0.0 },
            },
        ]);
        clockwise.selected = Some(1);
        clockwise
            .extend_selected_arc(Point { x: 0.0, y: -10.0 })
            .unwrap();
        assert!(
            matches!(clockwise.shapes[0], Shape::Arc { sweep, .. } if (sweep + 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6)
        );
        let mut no_boundary = Document::from_shapes(vec![Shape::Arc {
            id: 1,
            center,
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        }]);
        no_boundary.selected = Some(1);
        assert!(
            no_boundary
                .extend_selected_arc(Point { x: 0.0, y: 10.0 })
                .is_err()
        );
    }

    #[test]
    fn linked_line_end_edits_carry_the_point_or_are_rejected() {
        let mut doc = Document::default();
        let at = Point { x: 50.0, y: 20.0 };
        assert!(doc.add_point(at));
        doc.tool = Tool::Line;
        doc.draft_start = Some(Point { x: 10.0, y: 10.0 });
        assert!(doc.finish_draft(at));
        assert_eq!(doc.point_links.get(&(2, 1)), Some(&1));
        doc.selected = Some(2);
        // Length and axis edits now solve with the linked point following.
        assert_eq!(doc.set_dimension(DimensionField::LineLength, 60.0), Ok(()));
        let (Shape::Point { at: point, .. }, Shape::Line { a, b, .. }) =
            (&doc.shapes[0], &doc.shapes[1])
        else {
            panic!()
        };
        assert_eq!(*a, Point { x: 10.0, y: 10.0 });
        assert!((a.distance(*b) - 60.0).abs() < 1e-3);
        assert!(point.distance(*b) < 1e-4);
        assert!(doc.undo());
        doc.selected = Some(2);
        assert_eq!(
            doc.toggle_axis_constraint(AxisConstraint::Horizontal),
            Ok(true)
        );
        assert!(matches!(doc.shapes[0], Shape::Point { at, .. } if (at.y - 10.0).abs() < 1e-4));
        assert!(doc.undo());
        // A fixed linked point makes the same edits conflict, and they roll back.
        doc.fixed_anchors.insert((1, 0), at);
        doc.selected = Some(2);
        assert!(doc.set_dimension(DimensionField::LineLength, 60.0).is_err());
        assert!(
            doc.toggle_axis_constraint(AxisConstraint::Horizontal)
                .is_err()
        );
        assert!(doc.axis_constraints.is_empty());
        assert!(
            !doc.driving_dimensions
                .contains_key(&(2, DimensionField::LineLength))
        );
        assert!(!doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
        assert!(!doc.rotate_selected(20.0, Point { x: 0.0, y: 0.0 }, false));
        assert_eq!(doc.point_links.get(&(2, 1)), Some(&1));
        assert!(matches!(doc.shapes[1], Shape::Line { b, .. } if b == at));
    }

    #[test]
    fn arc_bounds_include_swept_quadrant_extrema() {
        let arc = Shape::Arc {
            id: 1,
            center: Point { x: 10.0, y: 20.0 },
            radius: 10.0,
            start_angle: 0.0,
            sweep: -std::f64::consts::PI * 1.5,
            segments: 64,
        };
        let (min, max) = arc.bounds().unwrap();
        assert!((min.x - 0.0).abs() < 1e-4);
        assert!((min.y - 10.0).abs() < 1e-4);
        assert!((max.x - 20.0).abs() < 1e-4);
        assert!((max.y - 30.0).abs() < 1e-4);
    }

    #[test]
    fn creation_selection_delete_and_undo() {
        let mut doc = Document {
            tool: Tool::Rectangle,
            ..Document::default()
        };
        doc.draft_start = Some(Point { x: 10.0, y: 20.0 });
        assert!(doc.finish_draft(Point { x: 80.0, y: 60.0 }));
        assert_eq!(doc.selected, Some(1));
        doc.select_at(Point { x: 30.0, y: 30.0 });
        assert_eq!(doc.selected, Some(1));
        assert!(doc.delete_selected());
        assert!(doc.shapes.is_empty());
        assert!(doc.undo());
        assert_eq!(doc.shapes.len(), 1);
        assert!(doc.undo());
        assert!(doc.shapes.is_empty());
    }

    #[test]
    fn center_rectangle_uses_first_click_as_center_and_is_undoable() {
        let mut doc = Document {
            tool: Tool::CenterRectangle,
            ..Document::default()
        };
        let center = Point { x: 10.0, y: 20.0 };
        doc.draft_start = Some(center);
        assert!(!doc.finish_draft(Point { x: 10.0, y: 20.0 }));
        doc.draft_start = Some(center);
        assert!(doc.finish_draft(Point { x: 40.0, y: 35.0 }));
        assert_eq!(doc.shapes[0].center(), center);
        assert_eq!(
            doc.shapes[0],
            Shape::Rectangle {
                id: 1,
                a: Point { x: -20.0, y: 5.0 },
                b: Point { x: 40.0, y: 35.0 },
            }
        );
        assert!(doc.undo());
        assert!(doc.shapes.is_empty());
    }

    #[test]
    fn three_point_rectangles_follow_edge_and_center_placement() {
        let first = Point { x: 0.0, y: 0.0 };
        let second = Point { x: 30.0, y: 40.0 };
        let side = Point { x: -40.0, y: 30.0 };
        let shape = Shape::rectangle_from_three_points(1, first, second, side).unwrap();
        assert!(
            matches!(shape, Shape::RotatedRectangle { width, height, rotation, .. }
            if (width - 50.0).abs() < 1e-5 && (height - 50.0).abs() < 1e-5 && (rotation - 53.1301).abs() < 1e-3)
        );
        assert_eq!(shape.center(), Point { x: -5.0, y: 35.0 });
        let centered = Shape::rectangle_from_center_side(2, first, second, side).unwrap();
        assert!(
            matches!(centered, Shape::RotatedRectangle { width, height, .. }
            if (width - 100.0).abs() < 1e-5 && (height - 100.0).abs() < 1e-5)
        );
        assert_eq!(centered.center(), first);
        assert!(Shape::rectangle_from_three_points(3, first, second, second).is_none());
        assert!(Shape::rectangle_from_center_side(4, first, second, second).is_none());

        let mut doc = Document {
            tool: Tool::ThreePointRectangle,
            ..Document::default()
        };
        doc.draft_start = Some(first);
        assert!(!doc.finish_draft(second));
        assert!(doc.finish_draft(side));
        assert_eq!(doc.shapes[0], shape);
        assert!(doc.undo());
        assert!(doc.shapes.is_empty());
    }

    #[test]
    fn copying_selected_shape_is_undoable_and_copies_its_dimension() {
        let original = Shape::Circle {
            id: 7,
            center: Point { x: 2.0, y: 3.0 },
            radius: 5.0,
            segments: 64,
        };
        let mut doc = Document::from_shapes(vec![original.clone()]);
        doc.selected = Some(7);
        doc.driving_dimensions
            .insert((7, DimensionField::CircleDiameter), 10.0);
        assert!(doc.copy_selected(Point { x: 10.0, y: -10.0 }));
        assert_eq!(doc.shapes[0], original);
        assert_eq!(
            doc.shapes[1],
            Shape::Circle {
                id: 8,
                center: Point { x: 12.0, y: -7.0 },
                radius: 5.0,
                segments: 64,
            }
        );
        assert_eq!(doc.selected, Some(8));
        assert_eq!(
            doc.driving_dimensions
                .get(&(8, DimensionField::CircleDiameter)),
            Some(&10.0)
        );
        assert!(doc.undo());
        assert_eq!(doc.shapes, vec![original]);
        assert!(doc.redo());
        assert_eq!(doc.shapes.len(), 2);
        assert!(!doc.copy_selected(Point {
            x: f32::NAN,
            y: 0.0
        }));
    }

    #[test]
    fn numeric_move_is_one_undo_step_and_keeps_driving_dimension() {
        let original = Shape::Line {
            id: 4,
            a: Point { x: 1.0, y: 2.0 },
            b: Point { x: 11.0, y: 2.0 },
        };
        let mut doc = Document::from_shapes(vec![original.clone()]);
        doc.selected = Some(4);
        doc.driving_dimensions
            .insert((4, DimensionField::LineLength), 10.0);
        assert!(!doc.move_selected(Point { x: 0.0, y: 0.0 }));
        assert!(doc.move_selected(Point { x: -3.0, y: 7.0 }));
        assert_eq!(
            doc.shapes[0],
            Shape::Line {
                id: 4,
                a: Point { x: -2.0, y: 9.0 },
                b: Point { x: 8.0, y: 9.0 }
            }
        );
        assert_eq!(
            doc.driving_dimensions.get(&(4, DimensionField::LineLength)),
            Some(&10.0)
        );
        assert!(doc.undo());
        assert_eq!(doc.shapes, vec![original]);
        assert!(!doc.undo());
    }

    #[test]
    fn scaling_changes_geometry_and_dimensions_and_copy_has_new_id() {
        let mut doc = Document::from_shapes(vec![Shape::Circle {
            id: 3,
            center: Point { x: 20.0, y: -4.0 },
            radius: 5.0,
            segments: 64,
        }]);
        doc.selected = Some(3);
        doc.driving_dimensions
            .insert((3, DimensionField::CircleDiameter), 10.0);
        assert!(!doc.scale_selected(-1.0, Point { x: 20.0, y: -4.0 }, false));
        assert!(doc.scale_selected(2.0, Point { x: 20.0, y: -4.0 }, false));
        assert_eq!(
            doc.shapes[0],
            Shape::Circle {
                id: 3,
                center: Point { x: 20.0, y: -4.0 },
                radius: 10.0,
                segments: 64,
            }
        );
        assert_eq!(
            doc.driving_dimensions
                .get(&(3, DimensionField::CircleDiameter)),
            Some(&20.0)
        );
        assert!(doc.scale_selected(0.5, Point { x: 20.0, y: -4.0 }, true));
        assert_eq!(
            doc.shapes[1],
            Shape::Circle {
                id: 4,
                center: Point { x: 20.0, y: -4.0 },
                radius: 5.0,
                segments: 64,
            }
        );
        assert_eq!(
            doc.driving_dimensions
                .get(&(4, DimensionField::CircleDiameter)),
            Some(&10.0)
        );
        assert!(doc.undo());
        assert_eq!(doc.shapes.len(), 1);
        assert!(doc.undo());
        assert_eq!(
            doc.driving_dimensions
                .get(&(3, DimensionField::CircleDiameter)),
            Some(&10.0)
        );
    }

    #[test]
    fn scaling_supported_shape_types_preserves_their_centers() {
        let shapes = vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 8.0, y: 0.0 },
            },
            Shape::Rectangle {
                id: 2,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 8.0, y: 6.0 },
            },
            Shape::Circle {
                id: 3,
                center: Point { x: 4.0, y: 3.0 },
                radius: 4.0,
                segments: 64,
            },
            Shape::Ellipse {
                id: 4,
                center: Point { x: 4.0, y: 3.0 },
                rx: 4.0,
                ry: 2.0,
                rotation: 30.0,
                segments: 128,
            },
            Shape::Arc {
                id: 5,
                center: Point { x: 4.0, y: 3.0 },
                radius: 4.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 128,
            },
            Shape::Polygon {
                id: 6,
                center: Point { x: 4.0, y: 3.0 },
                radius: 4.0,
                sides: 5,
                rotation: 30.0,
            },
            Shape::Capsule {
                id: 7,
                from: Point { x: 0.0, y: 3.0 },
                to: Point { x: 8.0, y: 3.0 },
                width: 4.0,
                arc_segments: 48,
            },
        ];
        for shape in shapes {
            let center = shape.center();
            let scaled = shape.scaled_about(0.5, center).expect("valid scale");
            assert_eq!(scaled.center(), center);
            assert_eq!(scaled.id(), shape.id());
            assert_ne!(scaled, shape);
        }
    }

    #[test]
    fn rotated_rectangle_hit_move_and_scale_preserve_orientation() {
        let shape = Shape::RotatedRectangle {
            id: 9,
            center: Point { x: 10.0, y: 20.0 },
            width: 40.0,
            height: 10.0,
            rotation: 45.0,
        };
        let corners =
            Shape::rotated_rectangle_corners(Point { x: 10.0, y: 20.0 }, 40.0, 10.0, 45.0);
        assert!(shape.hit(Point { x: 10.0, y: 20.0 }, 0.0));
        assert!(shape.hit(corners[0], 0.001));
        assert!(!shape.hit(Point { x: 30.0, y: 20.0 }, 0.0));
        let moved = shape.translated(Point { x: -5.0, y: 3.0 });
        assert!(matches!(
            moved,
            Shape::RotatedRectangle {
                center: Point { x: 5.0, y: 23.0 },
                rotation: 45.0,
                ..
            }
        ));
        let scaled = shape.scaled_about(2.0, shape.center()).unwrap();
        assert!(matches!(
            scaled,
            Shape::RotatedRectangle {
                width: 80.0,
                height: 20.0,
                rotation: 45.0,
                ..
            }
        ));
    }

    #[test]
    fn rotating_rectangle_converts_to_primitive_and_is_undoable() {
        let original = Shape::Rectangle {
            id: 2,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 40.0, y: 20.0 },
        };
        let mut doc = Document::from_shapes(vec![original.clone()]);
        doc.selected = Some(2);
        doc.driving_dimensions
            .insert((2, DimensionField::RectangleWidth), 40.0);
        assert!(!doc.rotate_selected(360.0, Point { x: 20.0, y: 10.0 }, false));
        assert!(doc.rotate_selected(30.0, Point { x: 20.0, y: 10.0 }, false));
        assert!(matches!(
            doc.shapes[0],
            Shape::RotatedRectangle {
                center: Point { x: 20.0, y: 10.0 },
                width: 40.0,
                height: 20.0,
                rotation: 30.0,
                ..
            }
        ));
        assert_eq!(
            doc.driving_dimensions
                .get(&(2, DimensionField::RectangleWidth)),
            Some(&40.0)
        );
        assert!(doc.rotate_selected(-30.0, Point { x: 20.0, y: 10.0 }, true));
        assert!(matches!(
            doc.shapes[1],
            Shape::RotatedRectangle {
                id: 3,
                rotation: 0.0,
                ..
            }
        ));
        assert_eq!(
            doc.driving_dimensions
                .get(&(3, DimensionField::RectangleWidth)),
            Some(&40.0)
        );
        assert!(doc.undo());
        assert_eq!(doc.shapes.len(), 1);
        assert!(doc.undo());
        assert_eq!(doc.shapes[0], original);
    }

    #[test]
    fn rotating_line_and_arc_uses_degrees_for_geometry_and_radians_for_arc_start() {
        let line = Shape::Line {
            id: 1,
            a: Point { x: 1.0, y: 0.0 },
            b: Point { x: 3.0, y: 0.0 },
        };
        let turned = line.rotated_about(90.0, Point { x: 0.0, y: 0.0 }).unwrap();
        assert!(
            matches!(turned, Shape::Line { a, b, .. } if a.x.abs() < 1e-5 && (a.y-1.0).abs() < 1e-5 && b.x.abs() < 1e-5 && (b.y-3.0).abs() < 1e-5)
        );
        let arc = Shape::Arc {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            radius: 5.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 128,
        };
        let turned = arc.rotated_about(90.0, Point { x: 0.0, y: 0.0 }).unwrap();
        assert!(
            matches!(turned, Shape::Arc { start_angle, sweep: 1.0, .. } if (start_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-5)
        );
    }

    #[test]
    fn scale_and_rotate_use_requested_center() {
        let mut doc = Document::from_shapes(vec![Shape::Circle {
            id: 1,
            center: Point { x: 10.0, y: 0.0 },
            radius: 2.0,
            segments: 64,
        }]);
        doc.selected = Some(1);
        assert!(doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
        assert!(matches!(
            doc.shapes[0],
            Shape::Circle {
                center: Point { x: 20.0, y: 0.0 },
                radius: 4.0,
                ..
            }
        ));
        assert!(doc.rotate_selected(90.0, Point { x: 0.0, y: 0.0 }, false));
        assert!(
            matches!(doc.shapes[0], Shape::Circle { center, radius: 4.0, .. } if center.x.abs() < 1e-5 && (center.y - 20.0).abs() < 1e-5)
        );
        assert!(doc.undo());
        assert!(matches!(
            doc.shapes[0],
            Shape::Circle {
                center: Point { x: 20.0, y: 0.0 },
                ..
            }
        ));
    }

    #[test]
    fn rejects_degenerate_shapes() {
        let mut doc = Document {
            tool: Tool::Circle,
            ..Document::default()
        };
        doc.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!doc.finish_draft(Point { x: 0.5, y: 0.0 }));
        assert!(doc.shapes.is_empty());
        assert!(!doc.undo());
    }

    #[test]
    fn line_hit_uses_finite_segment_distance() {
        let line = Shape::Line {
            id: 7,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 0.0 },
        };
        assert!(line.hit(Point { x: 5.0, y: 0.4 }, 0.5));
        assert!(!line.hit(Point { x: 20.0, y: 0.0 }, 0.5));
    }

    #[test]
    fn ellipse_creation_selection_and_dimensions() {
        let mut doc = Document {
            tool: Tool::Ellipse,
            ..Document::default()
        };
        doc.draft_start = Some(Point { x: 10.0, y: 20.0 });
        assert!(!doc.finish_draft(Point { x: 40.0, y: 20.0 }));
        assert!(doc.finish_draft(Point { x: 10.0, y: 35.0 }));
        doc.select_at(Point { x: 40.0, y: 20.0 });
        assert_eq!(doc.selected, Some(1));
        doc.set_dimension(DimensionField::RectangleWidth, 100.0)
            .unwrap();
        doc.set_dimension(DimensionField::RectangleHeight, 80.0)
            .unwrap();
        assert!(matches!(
            doc.shapes[0],
            Shape::Ellipse {
                rx: 50.0,
                ry: 40.0,
                ..
            }
        ));
        assert!(doc.undo());
        assert!(matches!(
            doc.shapes[0],
            Shape::Ellipse {
                rx: 50.0,
                ry: 15.0,
                ..
            }
        ));
    }

    #[test]
    fn rotated_ellipse_uses_axis_direction_and_perpendicular_height() {
        let ellipse = Shape::ellipse_from_three_points(
            4,
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 10.0 },
            Point { x: -5.0, y: 5.0 },
        )
        .unwrap();
        assert!(
            matches!(ellipse, Shape::Ellipse { rotation, rx, ry, .. } if (rotation - 45.0).abs() < 1e-4 && (rx - 2.0_f32.sqrt()*10.0).abs() < 1e-4 && (ry - 2.0_f32.sqrt()*5.0).abs() < 1e-4)
        );
        assert!(ellipse.hit(Point { x: 10.0, y: 10.0 }, 0.1));
        assert!(
            Shape::ellipse_from_three_points(
                4,
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                Point { x: 5.0, y: 0.0 }
            )
            .is_none()
        );
    }

    #[test]
    fn three_point_arc_keeps_major_sweep_and_is_undoable() {
        let start = Point { x: 10.0, y: 0.0 };
        let end = Point { x: 0.0, y: 10.0 };
        let through = Point { x: -10.0, y: 0.0 };
        let mut doc = Document {
            tool: Tool::Arc,
            ..Document::default()
        };
        doc.draft_start = Some(start);
        assert!(!doc.finish_draft(end));
        assert_eq!(doc.draft_end, Some(end));
        assert!(!doc.finish_draft(Point { x: 5.0, y: 5.0 }));
        assert!(doc.shapes.is_empty());
        assert_eq!(doc.draft_end, Some(end));
        assert!(doc.finish_draft(through));
        assert!(
            matches!(doc.shapes[0], Shape::Arc { sweep, .. } if (sweep + 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-5)
        );
        assert!(doc.shapes[0].hit(through, 0.1));
        assert!(!doc.shapes[0].hit(Point { x: 0.0, y: 0.0 }, 0.1));
        assert!(doc.undo());
        assert!(doc.shapes.is_empty());
        assert!(doc.redo());
        assert_eq!(doc.shapes.len(), 1);
    }

    #[test]
    fn three_point_arc_rejects_collinear_points() {
        assert!(
            Shape::arc_from_three_points(
                1,
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 0.0 },
                Point { x: 5.0, y: 0.0 }
            )
            .is_none()
        );
    }

    #[test]
    fn point_and_polygon_creation_selection_side_edit_and_undo() {
        let mut doc = Document::default();
        assert!(doc.add_point(Point { x: 5.0, y: 6.0 }));
        assert!(matches!(
            doc.shapes[0],
            Shape::Point {
                at: Point { x: 5.0, y: 6.0 },
                ..
            }
        ));
        doc.tool = Tool::Polygon;
        doc.draft_start = Some(Point { x: 30.0, y: 40.0 });
        assert!(doc.finish_draft(Point { x: 50.0, y: 40.0 }));
        assert!(matches!(
            doc.shapes[1],
            Shape::Polygon {
                radius: 20.0,
                sides: 6,
                ..
            }
        ));
        doc.select_at(Point { x: 30.0, y: 40.0 });
        assert_eq!(doc.selected, Some(2));
        doc.set_polygon_sides(5).unwrap();
        assert!(matches!(doc.shapes[1], Shape::Polygon { sides: 5, .. }));
        assert!(doc.set_polygon_sides(2).is_err());
        assert!(doc.undo());
        assert!(matches!(doc.shapes[1], Shape::Polygon { sides: 6, .. }));
        assert!(doc.redo());
        assert!(matches!(doc.shapes[1], Shape::Polygon { sides: 5, .. }));
        doc.select_at(Point { x: 5.0, y: 6.0 });
        assert_eq!(doc.selected, Some(1));
    }

    #[test]
    fn three_point_slot_width_hit_and_undo() {
        let mut doc = Document {
            tool: Tool::Slot,
            ..Document::default()
        };
        doc.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(!doc.finish_draft(Point { x: 20.0, y: 0.0 }));
        assert_eq!(doc.draft_end, Some(Point { x: 20.0, y: 0.0 }));
        assert!(!doc.finish_draft(Point { x: 10.0, y: 0.0 }));
        assert!(doc.finish_draft(Point { x: 10.0, y: 5.0 }));
        assert!(matches!(doc.shapes[0], Shape::Capsule { width: 10.0, .. }));
        assert!(doc.shapes[0].hit(Point { x: 10.0, y: 4.0 }, 0.1));
        assert!(!doc.shapes[0].hit(Point { x: 10.0, y: 8.0 }, 0.1));
        doc.set_capsule_width(20.0).unwrap();
        assert!(matches!(doc.shapes[0], Shape::Capsule { width: 20.0, .. }));
        assert!(doc.undo());
        assert!(matches!(doc.shapes[0], Shape::Capsule { width: 10.0, .. }));
        assert_eq!(
            Shape::capsule_outline(
                Point { x: 0.0, y: 0.0 },
                Point { x: 20.0, y: 0.0 },
                10.0,
                48
            )
            .len(),
            98
        );
    }

    #[test]
    fn dragging_is_one_undoable_translation_and_preserves_dimensions() {
        let mut doc = Document::from_shapes(vec![Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 3.0, y: 4.0 },
        }]);
        doc.selected = Some(1);
        doc.set_dimension(DimensionField::LineLength, 10.0).unwrap();
        assert!(doc.begin_drag(Point { x: 0.0, y: 0.0 }));
        assert!(!doc.drag_to(Point { x: 1.0, y: 0.0 }));
        assert!(doc.drag_to(Point { x: 10.0, y: -5.0 }));
        assert!(doc.drag_to(Point { x: 12.0, y: -5.0 }));
        assert!(doc.finish_drag());
        assert!(matches!(
            doc.shapes[0],
            Shape::Line {
                a: Point { x: 12.0, y: -5.0 },
                b: Point { x: 18.0, y: 3.0 },
                ..
            }
        ));
        assert_eq!(
            doc.driving_dimensions.get(&(1, DimensionField::LineLength)),
            Some(&10.0)
        );
        assert!(doc.undo());
        assert!(matches!(
            doc.shapes[0],
            Shape::Line {
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 6.0, y: 8.0 },
                ..
            }
        ));
        assert!(doc.redo());
        assert!(matches!(
            doc.shapes[0],
            Shape::Line {
                a: Point { x: 12.0, y: -5.0 },
                ..
            }
        ));
    }

    #[test]
    fn translation_moves_each_shape_without_changing_its_size() {
        let delta = Point { x: 7.0, y: -3.0 };
        let shapes = vec![
            Shape::Rectangle {
                id: 1,
                a: Point { x: 1.0, y: 2.0 },
                b: Point { x: 3.0, y: 4.0 },
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 1.0, y: 2.0 },
                radius: 5.0,
                segments: 64,
            },
            Shape::Ellipse {
                id: 3,
                center: Point { x: 1.0, y: 2.0 },
                rx: 5.0,
                ry: 3.0,
                rotation: 30.0,
                segments: 128,
            },
            Shape::Arc {
                id: 4,
                center: Point { x: 1.0, y: 2.0 },
                radius: 5.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 128,
            },
            Shape::Point {
                id: 5,
                at: Point { x: 1.0, y: 2.0 },
            },
            Shape::Polygon {
                id: 6,
                center: Point { x: 1.0, y: 2.0 },
                radius: 5.0,
                sides: 6,
                rotation: 30.0,
            },
            Shape::Capsule {
                id: 7,
                from: Point { x: 1.0, y: 2.0 },
                to: Point { x: 3.0, y: 4.0 },
                width: 5.0,
                arc_segments: 48,
            },
        ];
        for shape in shapes {
            let moved = shape.translated(delta);
            assert_eq!(moved.id(), shape.id());
            assert_eq!(
                moved.translated(Point {
                    x: -delta.x,
                    y: -delta.y
                }),
                shape
            );
        }
    }

    #[test]
    fn dimension_edits_are_centered_or_direction_preserving_and_undoable() {
        let mut document = Document::from_shapes(vec![
            Shape::Rectangle {
                id: 1,
                a: Point { x: 10.0, y: 10.0 },
                b: Point { x: 30.0, y: 20.0 },
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 50.0, y: 50.0 },
                radius: 8.0,
                segments: 64,
            },
            Shape::Line {
                id: 3,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 3.0, y: 4.0 },
            },
        ]);
        document.selected = Some(1);
        document
            .set_dimension(DimensionField::RectangleWidth, 40.0)
            .unwrap();
        assert_eq!(
            document.shapes[0],
            Shape::Rectangle {
                id: 1,
                a: Point { x: 0.0, y: 10.0 },
                b: Point { x: 40.0, y: 20.0 }
            }
        );
        document.selected = Some(2);
        document
            .set_dimension(DimensionField::CircleDiameter, 30.0)
            .unwrap();
        assert!(matches!(
            document.shapes[1],
            Shape::Circle { radius: 15.0, .. }
        ));
        document.selected = Some(3);
        document
            .set_dimension(DimensionField::LineLength, 10.0)
            .unwrap();
        assert!(matches!(
            document.shapes[2],
            Shape::Line {
                b: Point { x: 6.0, y: 8.0 },
                ..
            }
        ));
        assert!(document.undo());
        assert_eq!(
            document
                .driving_dimensions
                .get(&(3, DimensionField::LineLength)),
            None
        );
        assert!(document.redo());
        assert_eq!(
            document
                .driving_dimensions
                .get(&(3, DimensionField::LineLength)),
            Some(&10.0)
        );
        assert!(matches!(
            document.shapes[2],
            Shape::Line {
                b: Point { x: 6.0, y: 8.0 },
                ..
            }
        ));
        assert!(
            document
                .set_dimension(DimensionField::LineLength, 0.0)
                .is_err()
        );
    }

    fn line_ends(doc: &Document, index: usize) -> (Point, Point) {
        match doc.shapes[index] {
            Shape::Line { a, b, .. } => (a, b),
            _ => panic!("expected a line"),
        }
    }

    fn line_cross(first: (Point, Point), second: (Point, Point)) -> f32 {
        let (a, b) = first;
        let (c, d) = second;
        ((b.x - a.x) * (d.y - c.y) - (b.y - a.y) * (d.x - c.x)) / (a.distance(b) * c.distance(d))
    }

    #[test]
    fn solver_composes_equal_parallel_and_point_link_on_one_line() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 30.0 },
                b: Point { x: 10.0, y: 35.0 },
            },
            Shape::Point {
                id: 3,
                at: Point { x: 0.0, y: 30.0 },
            },
        ]);
        doc.point_links.insert((2, 0), 3);
        doc.selected = Some(2);
        // Previously rejected: Equal with endpoint links, and two relations on one line.
        assert_eq!(doc.toggle_equal_line_with(1), Ok(true));
        assert_eq!(
            doc.toggle_line_relation_with(1, LineRelationKind::Parallel),
            Ok(true)
        );
        doc.selected = Some(3);
        assert!(doc.move_selected(Point { x: 7.0, y: -4.0 }));
        let source = line_ends(&doc, 0);
        let target = line_ends(&doc, 1);
        assert_eq!(
            source,
            (Point { x: 0.0, y: 0.0 }, Point { x: 20.0, y: 0.0 })
        );
        assert!(target.0.distance(Point { x: 7.0, y: 26.0 }) < 1e-4);
        assert!((target.0.distance(target.1) - 20.0).abs() < 1e-3);
        assert!(line_cross(source, target).abs() < 1e-4);
        // One undo step restores the whole solved edit.
        assert!(doc.undo());
        assert!(
            matches!(doc.shapes[2], Shape::Point { at, .. } if at == Point { x: 0.0, y: 30.0 })
        );
        assert_eq!(line_ends(&doc, 1).0, Point { x: 0.0, y: 30.0 });
    }

    #[test]
    fn solver_propagates_chained_direction_relations() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 10.0 },
                b: Point { x: 10.0, y: 10.0 },
            },
            Shape::Line {
                id: 3,
                a: Point { x: 0.0, y: 20.0 },
                b: Point { x: 0.0, y: 30.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(
            doc.toggle_line_relation_with(1, LineRelationKind::Parallel),
            Ok(true)
        );
        doc.selected = Some(3);
        assert_eq!(
            doc.toggle_line_relation_with(2, LineRelationKind::Perpendicular),
            Ok(true)
        );
        doc.selected = Some(1);
        assert_eq!(doc.set_dimension(DimensionField::Rotation, 30.0), Ok(()));
        let first = line_ends(&doc, 0);
        let second = line_ends(&doc, 1);
        let third = line_ends(&doc, 2);
        assert!(line_cross(first, second).abs() < 1e-4);
        assert!((line_cross(second, third).abs() - 1.0).abs() < 1e-4);
        // Targets rotate about their first endpoint and keep their length.
        assert_eq!(second.0, Point { x: 0.0, y: 10.0 });
        assert!((second.0.distance(second.1) - 10.0).abs() < 1e-3);
        assert_eq!(third.0, Point { x: 0.0, y: 20.0 });
    }

    #[test]
    fn conflicting_relation_is_rejected_and_rolled_back() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 10.0 },
                b: Point { x: 3.0, y: 20.0 },
            },
        ]);
        doc.selected = Some(1);
        assert_eq!(
            doc.toggle_axis_constraint(AxisConstraint::Horizontal),
            Ok(true)
        );
        doc.selected = Some(2);
        assert_eq!(
            doc.toggle_axis_constraint(AxisConstraint::Vertical),
            Ok(true)
        );
        let before = doc.shapes.clone();
        let history = doc.history.len();
        assert!(
            doc.toggle_line_relation_with(1, LineRelationKind::Parallel)
                .is_err()
        );
        assert!(doc.line_relations.is_empty());
        assert_eq!(doc.shapes, before);
        assert_eq!(doc.history.len(), history);
        assert_eq!(doc.selected, Some(2));
        assert_eq!(
            doc.toggle_line_relation_with(1, LineRelationKind::Perpendicular),
            Ok(true)
        );
    }

    #[test]
    fn dragging_point_on_dimensioned_line_end_rotates_line_under_constraints() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 10.0, y: 0.0 },
            },
            Shape::Point {
                id: 2,
                at: Point { x: 10.0, y: 0.0 },
            },
        ]);
        doc.point_links.insert((1, 1), 2);
        doc.fixed_anchors.insert((1, 0), Point { x: 0.0, y: 0.0 });
        doc.driving_dimensions
            .insert((1, DimensionField::LineLength), 10.0);
        doc.selected = Some(2);
        // Dragging the point is held exactly; the fixed, dimensioned line
        // cannot reach it, so the frame is rejected...
        assert!(doc.begin_drag(Point { x: 10.0, y: 0.0 }));
        assert!(!doc.drag_to(Point { x: 10.0, y: 6.0 }));
        assert!(doc.cancel_drag());
        // ...while the point-coordinate edit along the circle is accepted.
        doc.selected = Some(2);
        doc.driving_dimensions
            .remove(&(1, DimensionField::LineLength));
        assert!(doc.begin_drag(Point { x: 10.0, y: 0.0 }));
        assert!(doc.drag_to(Point { x: 10.0, y: 6.0 }));
        assert!(doc.finish_drag());
        let (a, b) = line_ends(&doc, 0);
        assert_eq!(a, Point { x: 0.0, y: 0.0 });
        assert!(b.distance(Point { x: 10.0, y: 6.0 }) < 1e-4);
    }

    #[test]
    fn point_on_line_stays_on_the_finite_segment() {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Point {
                id: 2,
                at: Point { x: 18.0, y: 3.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_point_on_curve_with(1), Ok(true));
        doc.selected = Some(1);
        assert_eq!(doc.set_dimension(DimensionField::LineLength, 10.0), Ok(()));
        let at = doc.shapes[1].center();
        assert!(at.y.abs() < 1e-4 && at.x <= 10.0 + 1e-4, "{at:?}");
    }

    #[test]
    fn equal_ellipse_scale_and_spline_point_links_follow_transforms() {
        let ellipse = |id, x| Shape::Ellipse {
            id,
            center: Point { x, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 64,
        };
        let mut doc = Document::from_shapes(vec![ellipse(1, 0.0), ellipse(2, 40.0)]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_equal_ellipse_with(1), Ok(true));
        doc.selected = Some(1);
        assert!(doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
        assert!(matches!(
            doc.shapes[1],
            Shape::Ellipse { rx, ry, center, .. }
                if (rx - 20.0).abs() < 1e-4 && (ry - 10.0).abs() < 1e-4 && center.x == 40.0
        ));
        assert!(doc.undo());
        assert!(matches!(doc.shapes[1], Shape::Ellipse { rx: 10.0, .. }));

        let mut doc = Document::default();
        let start = Point { x: 0.0, y: 0.0 };
        assert!(doc.add_point(start));
        let spline = doc
            .add_spline(
                SplineMode::Fit,
                vec![start, Point { x: 10.0, y: 8.0 }, Point { x: 20.0, y: 0.0 }],
                false,
            )
            .unwrap();
        assert_eq!(doc.point_links.get(&(spline, 0)), Some(&1));
        doc.selected = Some(spline);
        assert!(doc.rotate_selected(90.0, Point { x: 10.0, y: 0.0 }, false));
        assert!(doc.shapes[0].center().distance(Point { x: 10.0, y: -10.0 }) < 1e-4);
        assert!(doc.scale_selected(0.5, Point { x: 10.0, y: 0.0 }, false));
        assert!(doc.shapes[0].center().distance(Point { x: 10.0, y: -5.0 }) < 1e-4);
    }

    #[test]
    fn curve_relations_follow_through_the_solver() {
        // Point on an arc stays inside the sweep when the arc turns.
        let mut doc = Document::from_shapes(vec![
            Shape::Arc {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                start_angle: 0.0,
                sweep: std::f64::consts::FRAC_PI_2,
                segments: 32,
            },
            Shape::Point {
                id: 2,
                at: Point { x: 1.0, y: 9.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_point_on_curve_with(1), Ok(true));
        doc.selected = Some(1);
        assert!(doc.rotate_selected(-60.0, Point { x: 0.0, y: 0.0 }, false));
        let at = doc.shapes[1].center();
        let angle = (at.y as f64).atan2(at.x as f64);
        assert!(((at.x.hypot(at.y)) - 10.0).abs() < 1e-3);
        assert!(
            (-std::f64::consts::FRAC_PI_3 - 1e-4..=std::f64::consts::FRAC_PI_6 + 1e-4)
                .contains(&angle),
            "{angle}"
        );

        // Smooth join: turning the line turns the arc's start with it.
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 10.0, y: -10.0 },
                b: Point { x: 10.0, y: 0.0 },
            },
            Shape::Arc {
                id: 2,
                center: Point { x: 0.0, y: 0.0 },
                radius: 10.0,
                start_angle: 0.0,
                sweep: 1.0,
                segments: 32,
            },
        ]);
        doc.endpoint_tangents.insert(2, (1, 0, 1, 1));
        doc.selected = Some(1);
        assert!(doc.rotate_selected(30.0, Point { x: 10.0, y: 0.0 }, false));
        let Shape::Arc { start_angle, .. } = doc.shapes[1] else {
            panic!()
        };
        assert!(
            (start_angle - 30f64.to_radians()).abs() < 1e-4,
            "{start_angle}"
        );

        // Shared ellipse pieces and an ellipse concentric partner follow.
        let mut doc = Document::from_shapes(vec![
            Shape::Ellipse {
                id: 1,
                center: Point { x: 0.0, y: 0.0 },
                rx: 10.0,
                ry: 5.0,
                rotation: 0.0,
                segments: 64,
            },
            Shape::Circle {
                id: 2,
                center: Point { x: 0.0, y: 0.0 },
                radius: 2.0,
                segments: 32,
            },
        ]);
        doc.concentric.insert(2, 1);
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 3.0, y: 4.0 }));
        assert_eq!(doc.shapes[1].center(), Point { x: 3.0, y: 4.0 });
    }
}
