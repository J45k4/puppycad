use crate::model::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplineMode {
    Fit,
    Control,
}

pub type Cubic = [Point; 4];

#[derive(Clone, Copy, Debug)]
pub struct Nearest {
    pub segment: usize,
    pub t: f32,
    pub distance: f32,
}

fn lerp(a: Point, b: Point, t: f32) -> Point {
    Point {
        x: a.x + (b.x - a.x) * t,
        y: a.y + (b.y - a.y) * t,
    }
}

pub fn point(curve: &Cubic, t: f32) -> Point {
    let a = lerp(curve[0], curve[1], t);
    let b = lerp(curve[1], curve[2], t);
    let c = lerp(curve[2], curve[3], t);
    lerp(lerp(a, b, t), lerp(b, c, t), t)
}

pub fn split(curve: &Cubic, t: f32) -> [Cubic; 2] {
    let a = lerp(curve[0], curve[1], t);
    let b = lerp(curve[1], curve[2], t);
    let c = lerp(curve[2], curve[3], t);
    let d = lerp(a, b, t);
    let e = lerp(b, c, t);
    let p = lerp(d, e, t);
    [[curve[0], a, d, p], [p, e, c, curve[3]]]
}

/// Intersect a finite line with cubic segments, including tangent contacts.
pub fn line_intersections(curves: &[Cubic], a: Point, b: Point) -> Vec<(usize, f32, Point)> {
    let dx = (b.x - a.x) as f64;
    let dy = (b.y - a.y) as f64;
    let length_sq = dx * dx + dy * dy;
    if length_sq <= 1e-18 || !length_sq.is_finite() {
        return Vec::new();
    }
    let mut hits: Vec<(usize, f32, Point)> = Vec::new();
    for (segment, curve) in curves.iter().enumerate() {
        let signed = curve.map(|p| dx * (p.y - a.y) as f64 - dy * (p.x - a.x) as f64);
        if signed.iter().all(|value| value.abs() < 1e-10) {
            continue;
        }
        let coefficients = [
            signed[0],
            3.0 * (signed[1] - signed[0]),
            3.0 * (signed[2] - 2.0 * signed[1] + signed[0]),
            signed[3] - 3.0 * signed[2] + 3.0 * signed[1] - signed[0],
        ];
        let evaluate = |t: f64| {
            ((coefficients[3] * t + coefficients[2]) * t + coefficients[1]) * t + coefficients[0]
        };
        let mut boundaries = vec![0.0_f64, 1.0];
        let (qa, qb, qc) = (
            3.0 * coefficients[3],
            2.0 * coefficients[2],
            coefficients[1],
        );
        if qa.abs() > 1e-12 {
            let discriminant = qb * qb - 4.0 * qa * qc;
            if discriminant >= 0.0 {
                let root = discriminant.sqrt();
                boundaries.extend([(-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)]);
            }
        } else if qb.abs() > 1e-12 {
            boundaries.push(-qc / qb);
        }
        boundaries.retain(|t| *t >= 0.0 && *t <= 1.0 && t.is_finite());
        boundaries.sort_by(f64::total_cmp);
        boundaries.dedup_by(|x, y| (*x - *y).abs() < 1e-10);
        let scale = signed.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
        let mut roots: Vec<f64> = boundaries
            .iter()
            .copied()
            .filter(|t| evaluate(*t).abs() <= scale * 1e-10)
            .collect();
        for pair in boundaries.windows(2) {
            let (mut left, mut right) = (pair[0], pair[1]);
            let left_value = evaluate(left);
            if left_value * evaluate(right) >= 0.0 {
                continue;
            }
            for _ in 0..50 {
                let middle = (left + right) * 0.5;
                if evaluate(middle) * left_value > 0.0 {
                    left = middle;
                } else {
                    right = middle;
                }
            }
            roots.push((left + right) * 0.5);
        }
        for t in roots {
            let candidate = point(curve, t as f32);
            let along =
                ((candidate.x - a.x) as f64 * dx + (candidate.y - a.y) as f64 * dy) / length_sq;
            if (-1e-6..=1.0 + 1e-6).contains(&along)
                && hits.iter().all(|(_, _, p)| p.distance(candidate) > 1e-4)
            {
                hits.push((segment, t as f32, candidate));
            }
        }
    }
    hits
}

fn bounded_roots(input: &[f64]) -> Vec<f64> {
    let scale = input.iter().copied().map(f64::abs).fold(0.0, f64::max);
    if scale < 1e-14 || !scale.is_finite() {
        return Vec::new();
    }
    let mut coefficients: Vec<f64> = input.iter().map(|value| value / scale).collect();
    while coefficients.len() > 1 && coefficients.last().unwrap().abs() < 1e-14 {
        coefficients.pop();
    }
    if coefficients.len() < 2 {
        return Vec::new();
    }
    let evaluate = |x: f64| {
        coefficients
            .iter()
            .rev()
            .fold(0.0, |value, c| value * x + c)
    };
    let mut divisions = vec![0.0, 1.0];
    if coefficients.len() > 2 {
        let derivative: Vec<_> = coefficients
            .iter()
            .enumerate()
            .skip(1)
            .map(|(index, value)| *value * index as f64)
            .collect();
        divisions.extend(bounded_roots(&derivative));
    }
    divisions.sort_by(f64::total_cmp);
    divisions.dedup_by(|a, b| (*a - *b).abs() < 1e-11);
    let mut roots: Vec<_> = divisions
        .iter()
        .copied()
        .filter(|x| evaluate(*x).abs() < 1e-11)
        .collect();
    for pair in divisions.windows(2) {
        let (mut low, mut high) = (pair[0], pair[1]);
        let mut low_value = evaluate(low);
        if low_value * evaluate(high) >= 0.0 {
            continue;
        }
        for _ in 0..65 {
            let middle = (low + high) * 0.5;
            let value = evaluate(middle);
            if low_value * value <= 0.0 {
                high = middle;
            } else {
                low = middle;
                low_value = value;
            }
        }
        roots.push((low + high) * 0.5);
    }
    roots.sort_by(f64::total_cmp);
    roots.dedup_by(|a, b| (*a - *b).abs() < 1e-8);
    roots
}

/// Intersect cubic segments with a supporting circle, retaining cubic parameters.
pub fn circle_intersections(
    curves: &[Cubic],
    center: Point,
    radius: f32,
) -> Vec<(usize, f32, Point)> {
    if !radius.is_finite() || radius <= 0.0 || !center.x.is_finite() || !center.y.is_finite() {
        return Vec::new();
    }
    let mut hits: Vec<(usize, f32, Point)> = Vec::new();
    for (segment, curve) in curves.iter().enumerate() {
        let scale = curve.iter().fold(radius as f64, |maximum, p| {
            maximum
                .max((p.x - center.x).abs() as f64)
                .max((p.y - center.y).abs() as f64)
        });
        let mut coefficients = [0.0_f64; 7];
        for axis in 0..2 {
            let values = curve.map(|p| {
                if axis == 0 {
                    (p.x - center.x) as f64 / scale
                } else {
                    (p.y - center.y) as f64 / scale
                }
            });
            let power = [
                values[0],
                3.0 * (values[1] - values[0]),
                3.0 * (values[2] - 2.0 * values[1] + values[0]),
                values[3] - 3.0 * values[2] + 3.0 * values[1] - values[0],
            ];
            for i in 0..4 {
                for j in 0..4 {
                    coefficients[i + j] += power[i] * power[j];
                }
            }
        }
        coefficients[0] -= (radius as f64 / scale).powi(2);
        for root in bounded_roots(&coefficients) {
            let candidate = point(curve, root as f32);
            if hits.iter().all(|(_, _, p)| p.distance(candidate) > 1e-4) {
                hits.push((segment, root as f32, candidate));
            }
        }
    }
    hits
}

/// Affine normalization reduces rotated ellipse contacts to circle contacts.
pub fn ellipse_intersections(
    curves: &[Cubic],
    center: Point,
    rx: f32,
    ry: f32,
    rotation: f32,
) -> Vec<(usize, f32, Point)> {
    if !rx.is_finite()
        || !ry.is_finite()
        || !rotation.is_finite()
        || rx <= 0.0
        || ry <= 0.0
        || !center.x.is_finite()
        || !center.y.is_finite()
    {
        return Vec::new();
    }
    let (sin, cos) = (rotation as f64).to_radians().sin_cos();
    let normalized: Vec<Cubic> = curves
        .iter()
        .map(|curve| {
            curve.map(|p| {
                let x = (p.x - center.x) as f64;
                let y = (p.y - center.y) as f64;
                Point {
                    x: ((x * cos + y * sin) / rx as f64) as f32,
                    y: ((-x * sin + y * cos) / ry as f64) as f32,
                }
            })
        })
        .collect();
    circle_intersections(&normalized, Point { x: 0.0, y: 0.0 }, 1.0)
        .into_iter()
        .map(|(segment, t, _)| (segment, t, point(&curves[segment], t)))
        .collect()
}

fn cubic_derivative(curve: &Cubic, t: f32) -> Point {
    let u = 1.0 - t;
    Point {
        x: 3.0
            * (u * u * (curve[1].x - curve[0].x)
                + 2.0 * u * t * (curve[2].x - curve[1].x)
                + t * t * (curve[3].x - curve[2].x)),
        y: 3.0
            * (u * u * (curve[1].y - curve[0].y)
                + 2.0 * u * t * (curve[2].y - curve[1].y)
                + t * t * (curve[3].y - curve[2].y)),
    }
}

fn cubic_box(curve: &Cubic) -> (f32, f32, f32, f32) {
    curve.iter().fold(
        (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ),
        |(min_x, max_x, min_y, max_y), p| {
            (
                min_x.min(p.x),
                max_x.max(p.x),
                min_y.min(p.y),
                max_y.max(p.y),
            )
        },
    )
}

fn straight_controls(curve: &Cubic) -> Option<(Point, Point)> {
    let (a, b) = (curve[0], curve[3]);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length_sq = dx * dx + dy * dy;
    if length_sq <= 1e-12 {
        return None;
    }
    let mut previous = 0.0;
    for p in &curve[1..3] {
        let cross = dx * (p.y - a.y) - dy * (p.x - a.x);
        let along = ((p.x - a.x) * dx + (p.y - a.y) * dy) / length_sq;
        if cross.abs() > length_sq * 1e-6 || along < previous || along > 1.0 {
            return None;
        }
        previous = along;
    }
    Some((a, b))
}

/// Isolated spline contacts, preserving parameters on the first spline.
pub fn spline_intersections(
    first: &[Cubic],
    second: &[Cubic],
) -> Result<Vec<(usize, f32, Point)>, String> {
    let mut hits: Vec<(usize, f32, Point)> = Vec::new();
    let mut visits = 0_usize;
    let mut candidates = 0_usize;
    for (segment, a) in first.iter().enumerate() {
        for b in second {
            let x = cubic_box(a);
            let y = cubic_box(b);
            if x.1 < y.0 || y.1 < x.0 || x.3 < y.2 || y.3 < x.2 {
                continue;
            }
            candidates += 1;
            if candidates > 4096 {
                return Err("Spline intersections exceed candidate-pair limit".into());
            }
            if a == b || a.iter().zip(b.iter().rev()).all(|(p, q)| p == q) {
                continue;
            }
            if let Some((start, end)) = straight_controls(b) {
                for (_, t, p) in line_intersections(&[*a], start, end) {
                    if hits.iter().all(|(old_segment, old_t, _)| {
                        *old_segment != segment || (*old_t - t).abs() > 1e-4
                    }) {
                        hits.push((segment, t, p));
                    }
                }
                continue;
            }
            if let Some((start, end)) = straight_controls(a) {
                for (_, _, p) in line_intersections(&[*b], start, end) {
                    if let Ok(nearest) = nearest(&[*a], p) {
                        let t = nearest.t;
                        if hits.iter().all(|(old_segment, old_t, _)| {
                            *old_segment != segment || (*old_t - t).abs() > 1e-4
                        }) {
                            hits.push((segment, t, p));
                        }
                    }
                }
                continue;
            }
            let mut stack = vec![(*a, *b, 0.0_f32, 1.0_f32, 0.0_f32, 1.0_f32, 0_u8)];
            while let Some((left, right, t0, t1, u0, u1, depth)) = stack.pop() {
                visits += 1;
                if visits > 100_000 {
                    return Err("Spline intersections exceed subdivision limit".into());
                }
                let x = cubic_box(&left);
                let y = cubic_box(&right);
                let rounding = 1e-5_f32;
                if x.1 + rounding < y.0
                    || y.1 + rounding < x.0
                    || x.3 + rounding < y.2
                    || y.3 + rounding < x.2
                {
                    continue;
                }
                let size_a = (x.1 - x.0).hypot(x.3 - x.2);
                let size_b = (y.1 - y.0).hypot(y.3 - y.2);
                if size_a.max(size_b) <= 1e-3 || depth >= 48 {
                    let (mut t, mut u) = ((t0 + t1) * 0.5, (u0 + u1) * 0.5);
                    for _ in 0..16 {
                        let p = point(a, t);
                        let q = point(b, u);
                        let da = cubic_derivative(a, t);
                        let db = cubic_derivative(b, u);
                        let det = da.x * db.y - da.y * db.x;
                        if det.abs() < 1e-10 {
                            break;
                        }
                        let (dx, dy) = (q.x - p.x, q.y - p.y);
                        let dt = (dx * db.y - dy * db.x) / det;
                        let du = (dx * da.y - dy * da.x) / det;
                        if dt.abs().max(du.abs()) > 0.1 {
                            break;
                        }
                        t = (t + dt).clamp(0.0, 1.0);
                        u = (u + du).clamp(0.0, 1.0);
                        if dt.abs().max(du.abs()) < 1e-7 {
                            break;
                        }
                    }
                    let p = point(a, t);
                    if p.distance(point(b, u)) <= 2e-3
                        && hits.iter().all(|(old_segment, old_t, _)| {
                            *old_segment != segment || (*old_t - t).abs() > 1e-3
                        })
                    {
                        hits.push((segment, t, p));
                    }
                    continue;
                }
                if size_a >= size_b {
                    let [one, two] = split(&left, 0.5);
                    let middle = (t0 + t1) * 0.5;
                    stack.push((two, right, middle, t1, u0, u1, depth + 1));
                    stack.push((one, right, t0, middle, u0, u1, depth + 1));
                } else {
                    let [one, two] = split(&right, 0.5);
                    let middle = (u0 + u1) * 0.5;
                    stack.push((left, two, t0, t1, middle, u1, depth + 1));
                    stack.push((left, one, t0, t1, u0, middle, depth + 1));
                }
            }
        }
    }
    hits.sort_by(|a, b| (a.0 as f32 + a.1).total_cmp(&(b.0 as f32 + b.1)));
    Ok(hits)
}

fn solve_tridiagonal(mut diagonal: Vec<f64>, mut rhs: Vec<f64>) -> Vec<f64> {
    for index in 1..rhs.len() {
        let factor = 1.0 / diagonal[index - 1];
        diagonal[index] -= factor;
        rhs[index] -= factor * rhs[index - 1];
    }
    for index in (0..rhs.len()).rev() {
        rhs[index] = (rhs[index] - rhs.get(index + 1).copied().unwrap_or(0.0)) / diagonal[index];
    }
    rhs
}

/// Double-precision Bezier segments for the constraint solver (same
/// construction as [`beziers`]); `None` for invalid input.
pub fn beziers_f64(
    control: bool,
    points: &[(f64, f64)],
    closed: bool,
) -> Option<Vec<[(f64, f64); 4]>> {
    let count = points.len();
    if count < 2 || points.iter().any(|p| !p.0.is_finite() || !p.1.is_finite()) {
        return None;
    }
    if control {
        if closed || count < 4 || (count - 1) % 3 != 0 {
            return None;
        }
        return Some(
            points
                .windows(4)
                .step_by(3)
                .map(|part| [part[0], part[1], part[2], part[3]])
                .collect(),
        );
    }
    if closed && count < 3 {
        return None;
    }
    let coordinate = |i: usize, axis: usize| if axis == 0 { points[i].0 } else { points[i].1 };
    let mut second = vec![[0.0_f64; 2]; count];
    for axis in 0..2 {
        if closed {
            let values = (0..count)
                .map(|i| {
                    6.0 * (coordinate((i + 1) % count, axis) - 2.0 * coordinate(i, axis)
                        + coordinate((i + count - 1) % count, axis))
                })
                .collect::<Vec<_>>();
            let mut diagonal = vec![4.0; count];
            diagonal[0] = 8.0;
            diagonal[count - 1] = 4.25;
            let solved = solve_tridiagonal(diagonal.clone(), values);
            let mut correction_rhs = vec![0.0; count];
            correction_rhs[0] = -4.0;
            correction_rhs[count - 1] = 1.0;
            let correction = solve_tridiagonal(diagonal, correction_rhs);
            let factor = (solved[0] - solved[count - 1] / 4.0)
                / (1.0 + correction[0] - correction[count - 1] / 4.0);
            for i in 0..count {
                second[i][axis] = solved[i] - factor * correction[i];
            }
        } else if count > 2 {
            let rhs = (1..count - 1)
                .map(|i| {
                    6.0 * (coordinate(i + 1, axis) - 2.0 * coordinate(i, axis)
                        + coordinate(i - 1, axis))
                })
                .collect::<Vec<_>>();
            let solved = solve_tridiagonal(vec![4.0; count - 2], rhs);
            for (i, value) in solved.into_iter().enumerate() {
                second[i + 1][axis] = value;
            }
        }
    }
    let segments = if closed { count } else { count - 1 };
    Some(
        (0..segments)
            .map(|i| {
                let j = (i + 1) % count;
                let handle = |axis: usize, end: bool| {
                    let delta = coordinate(j, axis) - coordinate(i, axis);
                    if end {
                        coordinate(j, axis)
                            - (delta + (second[i][axis] + 2.0 * second[j][axis]) / 6.0) / 3.0
                    } else {
                        coordinate(i, axis)
                            + (delta - (2.0 * second[i][axis] + second[j][axis]) / 6.0) / 3.0
                    }
                };
                [
                    points[i],
                    (handle(0, false), handle(1, false)),
                    (handle(0, true), handle(1, true)),
                    points[j],
                ]
            })
            .collect(),
    )
}

pub fn beziers(mode: SplineMode, points: &[Point], closed: bool) -> Result<Vec<Cubic>, String> {
    if points.len() < 2
        || points.len() > 4096
        || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite())
    {
        return Err("spline requires 2–4096 finite points".into());
    }
    if mode == SplineMode::Control {
        if closed || points.len() < 4 || (points.len() - 1) % 3 != 0 {
            return Err("control spline requires 3n + 1 points and cannot be closed".into());
        }
        return Ok(points
            .windows(4)
            .step_by(3)
            .map(|part| [part[0], part[1], part[2], part[3]])
            .collect());
    }
    let count = points.len();
    if closed && count < 3 {
        return Err("periodic spline requires at least three points".into());
    }
    if (0..if closed { count } else { count - 1 }).any(|i| points[i] == points[(i + 1) % count]) {
        return Err("adjacent fit points must be distinct".into());
    }
    let mut second = vec![[0.0_f64; 2]; count];
    for axis in 0..2 {
        let coordinate = |i: usize| {
            if axis == 0 {
                points[i].x as f64
            } else {
                points[i].y as f64
            }
        };
        if closed {
            let values = (0..count)
                .map(|i| {
                    6.0 * (coordinate((i + 1) % count) - 2.0 * coordinate(i)
                        + coordinate((i + count - 1) % count))
                })
                .collect::<Vec<_>>();
            let mut diagonal = vec![4.0; count];
            diagonal[0] = 8.0;
            diagonal[count - 1] = 4.25;
            let solved = solve_tridiagonal(diagonal.clone(), values);
            let mut correction_rhs = vec![0.0; count];
            correction_rhs[0] = -4.0;
            correction_rhs[count - 1] = 1.0;
            let correction = solve_tridiagonal(diagonal, correction_rhs);
            let factor = (solved[0] - solved[count - 1] / 4.0)
                / (1.0 + correction[0] - correction[count - 1] / 4.0);
            for i in 0..count {
                second[i][axis] = solved[i] - factor * correction[i];
            }
        } else {
            let mut diagonal = vec![4.0; count - 2];
            let mut rhs = (1..count - 1)
                .map(|i| 6.0 * (coordinate(i + 1) - 2.0 * coordinate(i) + coordinate(i - 1)))
                .collect::<Vec<_>>();
            for i in 1..rhs.len() {
                let factor = 1.0 / diagonal[i - 1];
                diagonal[i] -= factor;
                rhs[i] -= factor * rhs[i - 1];
            }
            for i in (0..rhs.len()).rev() {
                rhs[i] = (rhs[i] - rhs.get(i + 1).copied().unwrap_or(0.0)) / diagonal[i];
                second[i + 1][axis] = rhs[i];
            }
        }
    }
    let count_segments = if closed { count } else { count - 1 };
    let mut curves = Vec::with_capacity(count_segments);
    for i in 0..count_segments {
        let j = (i + 1) % count;
        let tangent_start = |axis| {
            let delta = if axis == 0 {
                (points[j].x - points[i].x) as f64
            } else {
                (points[j].y - points[i].y) as f64
            };
            delta - (2.0 * second[i][axis] + second[j][axis]) / 6.0
        };
        let tangent_end = |axis| {
            let delta = if axis == 0 {
                (points[j].x - points[i].x) as f64
            } else {
                (points[j].y - points[i].y) as f64
            };
            delta + (second[i][axis] + 2.0 * second[j][axis]) / 6.0
        };
        curves.push([
            points[i],
            Point {
                x: (points[i].x as f64 + tangent_start(0) / 3.0) as f32,
                y: (points[i].y as f64 + tangent_start(1) / 3.0) as f32,
            },
            Point {
                x: (points[j].x as f64 - tangent_end(0) / 3.0) as f32,
                y: (points[j].y as f64 - tangent_end(1) / 3.0) as f32,
            },
            points[j],
        ]);
    }
    Ok(curves)
}

pub fn samples(curves: &[Cubic], tolerance: f32) -> Result<Vec<Point>, String> {
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err("spline tolerance must be finite and positive".into());
    }
    fn visit(
        curve: &Cubic,
        depth: usize,
        tolerance: f32,
        output: &mut Vec<Point>,
    ) -> Result<(), String> {
        if output.len() >= 65536 {
            return Err("spline sampling exceeds point limit".into());
        }
        let chord = curve[3].distance(curve[0]);
        let distance = |point: Point| {
            if chord <= f32::EPSILON {
                return point.distance(curve[0]);
            }
            let dx = curve[3].x - curve[0].x;
            let dy = curve[3].y - curve[0].y;
            let t = (((point.x - curve[0].x) * dx + (point.y - curve[0].y) * dy) / (chord * chord))
                .clamp(0.0, 1.0);
            point.distance(Point {
                x: curve[0].x + t * dx,
                y: curve[0].y + t * dy,
            })
        };
        if distance(curve[1]).max(distance(curve[2])) <= tolerance {
            output.push(curve[3]);
        } else if depth >= 20 {
            return Err("spline sampling cannot meet tolerance".into());
        } else {
            let [left, right] = split(curve, 0.5);
            visit(&left, depth + 1, tolerance, output)?;
            visit(&right, depth + 1, tolerance, output)?;
        }
        Ok(())
    }
    let mut output = Vec::new();
    if let Some(first) = curves.first() {
        output.push(first[0]);
    }
    for curve in curves {
        visit(curve, 0, tolerance, &mut output)?;
    }
    Ok(output)
}

pub fn nearest(curves: &[Cubic], target: Point) -> Result<Nearest, String> {
    if curves.is_empty() || !target.x.is_finite() || !target.y.is_finite() {
        return Err("spline target and curves must be valid".into());
    }
    let mut best = Nearest {
        segment: 0,
        t: 0.0,
        distance: curves[0][0].distance(target),
    };
    let mut visits = 0_usize;
    fn consider(best: &mut Nearest, curve: &Cubic, segment: usize, t: f32, target: Point) {
        let candidate = point(curve, t);
        let distance = candidate.distance(target);
        if distance < best.distance {
            *best = Nearest {
                segment,
                t,
                distance,
            };
        }
    }
    fn lower_bound(curve: &Cubic, target: Point) -> f32 {
        let min_x = curve.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let max_x = curve.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let min_y = curve.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let max_y = curve.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        (min_x - target.x)
            .max(0.0)
            .max(target.x - max_x)
            .hypot((min_y - target.y).max(0.0).max(target.y - max_y))
    }
    fn visit(
        curve: &Cubic,
        original: &Cubic,
        segment: usize,
        from: f32,
        to: f32,
        depth: usize,
        target: Point,
        best: &mut Nearest,
        visits: &mut usize,
    ) -> Result<(), String> {
        if lower_bound(curve, target) > best.distance + 1e-5 {
            return Ok(());
        }
        *visits += 1;
        if *visits > 200000 {
            return Err("spline targeting exceeds work limit".into());
        }
        let middle = (from + to) * 0.5;
        consider(best, original, segment, middle, target);
        if depth >= 24 || to - from <= 1e-6 {
            return Ok(());
        }
        let [left, right] = split(curve, 0.5);
        if lower_bound(&left, target) <= lower_bound(&right, target) {
            visit(
                &left,
                original,
                segment,
                from,
                middle,
                depth + 1,
                target,
                best,
                visits,
            )?;
            visit(
                &right,
                original,
                segment,
                middle,
                to,
                depth + 1,
                target,
                best,
                visits,
            )?;
        } else {
            visit(
                &right,
                original,
                segment,
                middle,
                to,
                depth + 1,
                target,
                best,
                visits,
            )?;
            visit(
                &left,
                original,
                segment,
                from,
                middle,
                depth + 1,
                target,
                best,
                visits,
            )?;
        }
        Ok(())
    }
    for (segment, curve) in curves.iter().enumerate() {
        consider(&mut best, curve, segment, 0.0, target);
        consider(&mut best, curve, segment, 1.0, target);
        consider(&mut best, curve, segment, 0.5, target);
    }
    for (segment, curve) in curves.iter().enumerate() {
        visit(
            curve,
            curve,
            segment,
            0.0,
            1.0,
            0,
            target,
            &mut best,
            &mut visits,
        )?;
    }
    Ok(best)
}

pub fn chain_points(curves: &[Cubic]) -> Vec<Point> {
    let mut result = Vec::with_capacity(curves.len() * 3 + 1);
    if let Some(first) = curves.first() {
        result.push(first[0]);
    }
    for curve in curves {
        result.extend_from_slice(&curve[1..]);
    }
    result
}

/// Extract a continuous portion in segment coordinates; periodic callers may wrap once.
pub fn portion(
    curves: &[Cubic],
    start: f32,
    end: f32,
    periodic: bool,
) -> Result<Vec<Point>, String> {
    let length = curves.len() as f32;
    if curves.is_empty()
        || !start.is_finite()
        || !end.is_finite()
        || start < 0.0
        || end > length * if periodic { 2.0 } else { 1.0 }
        || end - start <= 1e-6
    {
        return Err("invalid spline portion".into());
    }
    let mut selected = Vec::new();
    for index in start.floor() as usize..end.ceil() as usize {
        let lower = start.max(index as f32) - index as f32;
        let upper = end.min((index + 1) as f32) - index as f32;
        if upper - lower <= 1e-6 {
            continue;
        }
        let mut curve = curves[index % curves.len()];
        if lower > 0.0 {
            curve = split(&curve, lower)[1];
        }
        if upper < 1.0 {
            curve = split(&curve, (upper - lower) / (1.0 - lower))[0];
        }
        selected.push(curve);
    }
    let points = chain_points(&selected);
    beziers(SplineMode::Control, &points, false)?;
    Ok(points)
}

pub fn split_open(
    curves: &[Cubic],
    segment: usize,
    t: f32,
) -> Result<(Vec<Point>, Vec<Point>), String> {
    if curves.is_empty()
        || segment >= curves.len()
        || !t.is_finite()
        || !(0.0..=1.0).contains(&t)
        || (segment == 0 && t <= 1e-6)
        || (segment == curves.len() - 1 && t >= 1.0 - 1e-6)
    {
        return Err("choose a point inside the spline".into());
    }
    let [left, right] = split(&curves[segment], t);
    let before = curves[..segment]
        .iter()
        .copied()
        .chain((t > 1e-6).then_some(left))
        .collect::<Vec<_>>();
    let after = (t < 1.0 - 1e-6)
        .then_some(right)
        .into_iter()
        .chain(curves[segment + 1..].iter().copied())
        .collect::<Vec<_>>();
    let first = chain_points(&before);
    let second = chain_points(&after);
    beziers(SplineMode::Control, &first, false)?;
    beziers(SplineMode::Control, &second, false)?;
    Ok((first, second))
}

pub fn open_periodic(curves: &[Cubic], segment: usize, t: f32) -> Result<Vec<Point>, String> {
    if curves.len() < 3 || segment >= curves.len() || !t.is_finite() || !(0.0..=1.0).contains(&t) {
        return Err("choose a point on the periodic spline".into());
    }
    let [left, right] = split(&curves[segment], t);
    let chain = (t < 1.0 - 1e-6)
        .then_some(right)
        .into_iter()
        .chain(curves[segment + 1..].iter().copied())
        .chain(curves[..segment].iter().copied())
        .chain((t > 1e-6).then_some(left))
        .collect::<Vec<_>>();
    let points = chain_points(&chain);
    beziers(SplineMode::Control, &points, false)?;
    Ok(points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_and_fit_splines_preserve_points_and_periodic_seam() {
        let control = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 10.0, y: 10.0 },
            Point { x: 20.0, y: 10.0 },
        ];
        let curve = beziers(SplineMode::Control, &control, false).unwrap();
        assert_eq!(curve.len(), 1);
        assert_eq!(point(&curve[0], 0.0), control[0]);
        assert_eq!(point(&curve[0], 1.0), control[3]);
        assert!(samples(&curve, 0.01).unwrap().len() > 2);
        let fit = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: 0.0, y: 20.0 },
        ];
        let open = beziers(SplineMode::Fit, &fit, false).unwrap();
        assert_eq!(open.len(), 3);
        for i in 0..3 {
            assert_eq!(open[i][0], fit[i]);
            assert_eq!(open[i][3], fit[i + 1]);
        }
        let periodic = beziers(SplineMode::Fit, &fit, true).unwrap();
        assert_eq!(periodic.len(), 4);
        assert_eq!(periodic[3][3], fit[0]);
        assert!(beziers(SplineMode::Control, &control, true).is_err());
    }

    #[test]
    fn exact_open_split_and_periodic_opening_preserve_curve_locus() {
        let controls = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 20.0 },
            Point { x: 20.0, y: -20.0 },
            Point { x: 30.0, y: 0.0 },
        ];
        let curves = beziers(SplineMode::Control, &controls, false).unwrap();
        let target = point(&curves[0], 0.5);
        let nearest = nearest(&curves, target).unwrap();
        assert!(nearest.distance < 1e-4);
        assert!((nearest.t - 0.5).abs() < 1e-3);
        let (first, second) = split_open(&curves, 0, 0.5).unwrap();
        let a = beziers(SplineMode::Control, &first, false).unwrap();
        let b = beziers(SplineMode::Control, &second, false).unwrap();
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            assert!(point(&a[0], t).distance(point(&curves[0], t * 0.5)) < 1e-5);
            assert!(point(&b[0], t).distance(point(&curves[0], 0.5 + t * 0.5)) < 1e-5);
        }
        let fit = [
            Point { x: -10.0, y: -10.0 },
            Point { x: 10.0, y: -10.0 },
            Point { x: 10.0, y: 10.0 },
            Point { x: -10.0, y: 10.0 },
        ];
        let periodic = beziers(SplineMode::Fit, &fit, true).unwrap();
        let opened = open_periodic(&periodic, 1, 0.5).unwrap();
        assert_eq!(opened.first(), opened.last());
        let chain = beziers(SplineMode::Control, &opened, false).unwrap();
        assert_eq!(chain.len(), periodic.len() + 1);
        assert!(point(&chain[0], 0.5).distance(point(&periodic[1], 0.75)) < 1e-5);
        assert!(point(chain.last().unwrap(), 0.5).distance(point(&periodic[1], 0.25)) < 1e-5);
    }

    #[test]
    fn finite_line_hits_cubic_at_crossing_and_tangent() {
        let curve = [[
            Point { x: -10.0, y: 0.0 },
            Point { x: -10.0, y: 20.0 },
            Point { x: 10.0, y: 20.0 },
            Point { x: 10.0, y: 0.0 },
        ]];
        let crossing =
            line_intersections(&curve, Point { x: 0.0, y: -5.0 }, Point { x: 0.0, y: 20.0 });
        assert_eq!(crossing.len(), 1);
        assert!((crossing[0].1 - 0.5).abs() < 1e-5);
        assert!(crossing[0].2.distance(Point { x: 0.0, y: 15.0 }) < 1e-4);
        let tangent = line_intersections(
            &curve,
            Point { x: -20.0, y: 15.0 },
            Point { x: 20.0, y: 15.0 },
        );
        assert_eq!(tangent.len(), 1);
        assert!((tangent[0].1 - 0.5).abs() < 1e-5);
        assert!(
            line_intersections(
                &curve,
                Point { x: -20.0, y: 20.0 },
                Point { x: 20.0, y: 20.0 },
            )
            .is_empty()
        );
        assert!(
            line_intersections(
                &curve,
                Point { x: 20.0, y: -5.0 },
                Point { x: 20.0, y: 20.0 },
            )
            .is_empty()
        );
    }

    #[test]
    fn circle_hits_cubic_twice_and_at_tangent() {
        let curve = [[
            Point { x: -10.0, y: 0.0 },
            Point { x: -5.0, y: 0.0 },
            Point { x: 5.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
        ]];
        let crossing = circle_intersections(&curve, Point { x: 0.0, y: 0.0 }, 5.0);
        assert_eq!(crossing.len(), 2);
        assert!((crossing[0].2.x.abs() - 5.0).abs() < 1e-4);
        assert!((crossing[1].2.x.abs() - 5.0).abs() < 1e-4);
        let tangent = circle_intersections(&curve, Point { x: 0.0, y: 5.0 }, 5.0);
        assert_eq!(tangent.len(), 1);
        assert!(tangent[0].2.distance(Point { x: 0.0, y: 0.0 }) < 1e-4);
        assert!(circle_intersections(&curve, Point { x: 0.0, y: 10.0 }, 5.0).is_empty());
    }

    #[test]
    fn rotated_ellipse_hits_cubic_with_original_curve_parameters() {
        let curve = [[
            Point { x: -20.0, y: 0.0 },
            Point { x: -10.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
        ]];
        let hits = ellipse_intersections(&curve, Point { x: 0.0, y: 0.0 }, 10.0, 5.0, 90.0);
        assert_eq!(hits.len(), 2);
        assert!((hits[0].2.x.abs() - 5.0).abs() < 1e-4);
        assert!((hits[1].2.x.abs() - 5.0).abs() < 1e-4);
        for (segment, t, p) in hits {
            assert!(p.distance(point(&curve[segment], t)) < 1e-5);
        }
    }

    #[test]
    fn spline_spline_crossings_tangent_and_identical_overlap() {
        let arch = [
            Point { x: -20.0, y: 0.0 },
            Point { x: -20.0, y: 20.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: 20.0, y: 0.0 },
        ];
        let horizontal = |y| {
            [
                Point { x: -30.0, y },
                Point { x: -10.0, y },
                Point { x: 10.0, y },
                Point { x: 30.0, y },
            ]
        };
        let hits = spline_intersections(&[arch], &[horizontal(10.0)]).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits[0].1 < 0.5 && hits[1].1 > 0.5);
        let tangent = spline_intersections(&[arch], &[horizontal(15.0)]).unwrap();
        assert_eq!(tangent.len(), 1);
        assert!(tangent[0].2.distance(Point { x: 0.0, y: 15.0 }) < 2e-3);
        assert!(spline_intersections(&[arch], &[arch]).unwrap().is_empty());
        assert!(
            spline_intersections(&[arch], &[horizontal(30.0)])
                .unwrap()
                .is_empty()
        );
        let inverted = [
            Point { x: -20.0, y: 20.0 },
            Point { x: -20.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 20.0, y: 20.0 },
        ];
        let curved = spline_intersections(&[arch], &[inverted]).unwrap();
        assert_eq!(curved.len(), 2);
        for (_, t, p) in curved {
            assert!(p.distance(point(&arch, t)) < 2e-3);
            assert!(nearest(&[inverted], p).unwrap().distance < 2e-3);
        }
    }

    #[test]
    fn spline_intersection_work_limit_is_an_error() {
        let a = [
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 20.0 },
            Point { x: -10.0, y: 20.0 },
            Point { x: 0.0, y: 0.0 },
        ];
        let b = [
            Point { x: 0.0, y: 1.0 },
            Point { x: 10.0, y: 21.0 },
            Point { x: -10.0, y: 21.0 },
            Point { x: 0.0, y: 1.0 },
        ];
        assert!(spline_intersections(&vec![a; 65], &vec![b; 65]).is_err());
    }
}
