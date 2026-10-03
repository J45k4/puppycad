use super::*;

pub(super) fn finite_intersections(first: &Shape, second: &Shape) -> Vec<Point> {
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

pub(super) fn spline_boundary_intersections(
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

pub(super) fn bounded_polynomial_roots(input: &[f64]) -> Vec<f64> {
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

pub(super) fn circle_ellipse_intersections(
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

pub(super) fn ellipse_ellipse_intersections(first: &Shape, second: &Shape) -> Vec<Point> {
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

pub(super) fn capsule_boundary(shape: &Shape) -> Option<[Shape; 4]> {
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

pub(super) fn boundary_edges(shape: &Shape) -> Option<Vec<(Point, Point)>> {
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

pub(super) fn arc_progress(point: Point, center: Point, start: f64, sweep: f64) -> f64 {
    let angle = ((point.y - center.y) as f64).atan2((point.x - center.x) as f64);
    let traveled = if sweep > 0.0 {
        (angle - start).rem_euclid(std::f64::consts::TAU)
    } else {
        (start - angle).rem_euclid(std::f64::consts::TAU)
    };
    traveled / sweep.abs()
}

pub(super) fn on_arc_sweep(point: Point, center: Point, start: f64, sweep: f64) -> bool {
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
