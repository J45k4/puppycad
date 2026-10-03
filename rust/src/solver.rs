//! General least-squares constraint solver for the native sketch model.
//!
//! # Design
//!
//! The web app (`src/sketch-solver.ts`) flattens every entity into a vector of
//! scalar parameters (point coordinates, radii, ...), expresses every driving
//! relation as a small residual vector that is zero when the relation holds,
//! and minimizes the sum of squares with numeric Jacobians. This module ports
//! that structure for the Rust model:
//!
//! * [`Problem::params`] is the parameter vector. The bridge in `model.rs`
//!   (`Document::solve_constraints`) adds two parameters per anchor point
//!   (line endpoints, sketch points, circle/arc centers) and one per radius.
//!   Dimension values may also be parameters ([`Value::Param`]) so a dimension
//!   can be left free and measured by the solve; the document currently passes
//!   driving values as [`Value::Const`].
//! * [`Constraint`] has one residual function per natively supported relation
//!   kind (fixed anchor, coincident endpoint/point, horizontal, vertical, line
//!   length, line rotation, equal length, parallel, perpendicular, collinear,
//!   signed angle, midpoint and point-on-line, radius, equal radius, radius
//!   difference, concentric, point-on-circle).
//! * [`Soft`] terms express user intent: stay close to the post-edit geometry,
//!   keep each line's first endpoint more strongly than its second one, and
//!   keep unconstrained line lengths. They are what made the hand-written
//!   propagation "anchor the first endpoint"; here they are weights.
//! * Locked parameters are constants. The document locks the anchors the user
//!   just edited and every entity that participates in a relation the solver
//!   does not model (those keep their hand-written propagation).
//!
//! Solving is Levenberg–Marquardt with central-difference Jacobians, run as a
//! short continuation: soft terms are first solved together with heavily
//! weighted hard residuals, then a final pass solves the hard residuals alone
//! while damping each step with the soft weights, so the converged result is
//! the weighted closest solution instead of an arbitrary one. The result
//! reports the hard residual norm, the iteration count and a `converged` flag;
//! conflicting constraints simply fail to converge.

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)] // `Param` is exercised by tests; the document passes constants.
pub enum Value {
    Const(f64),
    Param(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointVar {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineVar {
    pub a: PointVar,
    pub b: PointVar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CircleVar {
    pub center: PointVar,
    pub radius: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Constraint {
    Fixed {
        point: PointVar,
        at: (f64, f64),
    },
    Coincident(PointVar, PointVar),
    Horizontal(LineVar),
    Vertical(LineVar),
    Length {
        line: LineVar,
        value: Value,
    },
    /// Absolute line direction in degrees (atan2 of b - a).
    Rotation {
        line: LineVar,
        degrees: Value,
    },
    EqualLength(LineVar, LineVar),
    Parallel(LineVar, LineVar),
    Perpendicular(LineVar, LineVar),
    Collinear(LineVar, LineVar),
    /// Signed angle from the first line to the second, in degrees.
    Angle {
        from: LineVar,
        to: LineVar,
        degrees: Value,
    },
    Midpoint {
        point: PointVar,
        line: LineVar,
    },
    PointOnLine {
        point: PointVar,
        line: LineVar,
    },
    Radius {
        circle: CircleVar,
        value: Value,
    },
    EqualRadius(CircleVar, CircleVar),
    /// `source.radius - target.radius == value`.
    RadiusDifference {
        source: CircleVar,
        target: CircleVar,
        value: Value,
    },
    Concentric(CircleVar, CircleVar),
    PointOnCircle {
        point: PointVar,
        circle: CircleVar,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Soft {
    /// Keep parameter `var` near `value` with the given weight.
    Target { var: usize, value: f64, weight: f64 },
    /// Keep a line's length near `length`.
    KeepLength {
        line: LineVar,
        length: f64,
        weight: f64,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Problem {
    pub params: Vec<f64>,
    pub locked: Vec<bool>,
    pub constraints: Vec<Constraint>,
    pub soft: Vec<Soft>,
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub tolerance: f64,
    pub max_iterations: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            tolerance: 1e-7,
            max_iterations: 100,
        }
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // Diagnostics are reported to callers that want them.
pub struct SolveResult {
    pub params: Vec<f64>,
    /// Euclidean norm of the hard residual vector at `params`.
    pub residual_norm: f64,
    pub max_residual: f64,
    pub iterations: usize,
    pub converged: bool,
}

impl Problem {
    pub fn param(&mut self, value: f64, locked: bool) -> usize {
        self.params.push(value);
        self.locked.push(locked);
        self.params.len() - 1
    }

    pub fn point(&mut self, x: f64, y: f64, locked: bool) -> PointVar {
        PointVar {
            x: self.param(x, locked),
            y: self.param(y, locked),
        }
    }

    pub fn residuals(&self, params: &[f64]) -> Vec<f64> {
        let mut out = Vec::new();
        for constraint in &self.constraints {
            residual(constraint, params, &mut out);
        }
        out
    }

    pub fn solve(&self, options: Options) -> SolveResult {
        let mut params = self.params.clone();
        let free: Vec<usize> = (0..params.len()).filter(|&i| !self.locked[i]).collect();
        let mut iterations = 0;
        let damping = self.damping_weights();
        if !free.is_empty() && !self.constraints.is_empty() {
            for hard_weight in [1e2, 1e4] {
                iterations += self.levenberg_marquardt(
                    &mut params,
                    &free,
                    Some(hard_weight),
                    &damping,
                    options,
                );
            }
            iterations += self.levenberg_marquardt(&mut params, &free, None, &damping, options);
            // Round-off cleanup: parameters the soft phases only nudged go back
            // to their exact input values, and the rest are re-solved around
            // them, so untouched geometry stays bit-identical.
            let snapped: Vec<usize> = free
                .iter()
                .copied()
                .filter(|&i| {
                    params[i] != self.params[i]
                        && (params[i] - self.params[i]).abs()
                            <= 1e-4 * self.params[i].abs().max(1.0)
                })
                .collect();
            if !snapped.is_empty() {
                let mut retry = params.clone();
                for &i in &snapped {
                    retry[i] = self.params[i];
                }
                let rest: Vec<usize> = free
                    .iter()
                    .copied()
                    .filter(|i| !snapped.contains(i))
                    .collect();
                iterations += self.levenberg_marquardt(&mut retry, &rest, None, &damping, options);
                if self.max_residual(&retry) <= options.tolerance {
                    params = retry;
                }
            }
        }
        let residuals = self.residuals(&params);
        let residual_norm = residuals.iter().map(|r| r * r).sum::<f64>().sqrt();
        let max_residual = residuals.iter().fold(0.0f64, |m, r| m.max(r.abs()));
        SolveResult {
            converged: params.iter().all(|p| p.is_finite()) && max_residual <= options.tolerance,
            params,
            residual_norm,
            max_residual,
            iterations,
        }
    }

    fn max_residual(&self, params: &[f64]) -> f64 {
        self.residuals(params)
            .iter()
            .fold(0.0f64, |m, r| m.max(r.abs()))
    }

    fn damping_weights(&self) -> Vec<f64> {
        let mut weights = vec![1e-4; self.params.len()];
        for soft in &self.soft {
            if let Soft::Target { var, weight, .. } = *soft {
                weights[var] += weight * weight;
            }
        }
        weights
    }

    /// Weighted residual vector: hard residuals (scaled by `hard_weight`) and,
    /// when `hard_weight` is set, the soft terms.
    fn objective(&self, params: &[f64], hard_weight: Option<f64>) -> Vec<f64> {
        let mut out = self.residuals(params);
        if let Some(weight) = hard_weight {
            for value in &mut out {
                *value *= weight;
            }
            for soft in &self.soft {
                match *soft {
                    Soft::Target { var, value, weight } => out.push(weight * (params[var] - value)),
                    Soft::KeepLength {
                        line,
                        length,
                        weight,
                    } => out.push(weight * (line_length(params, line) - length)),
                }
            }
        }
        out
    }

    fn levenberg_marquardt(
        &self,
        params: &mut [f64],
        free: &[usize],
        hard_weight: Option<f64>,
        damping: &[f64],
        options: Options,
    ) -> usize {
        let cost = |p: &[f64]| {
            self.objective(p, hard_weight)
                .iter()
                .map(|r| r * r)
                .sum::<f64>()
        };
        let mut current = cost(params);
        let mut lambda = 1e-3;
        let n = free.len();
        for iteration in 0..options.max_iterations {
            if hard_weight.is_none() && current.sqrt() <= options.tolerance * 1e-2 {
                return iteration;
            }
            let r = self.objective(params, hard_weight);
            let m = r.len();
            // Central-difference Jacobian over the free parameters.
            let mut jacobian = vec![0.0; m * n];
            let mut probe = params.to_vec();
            for (column, &var) in free.iter().enumerate() {
                let h = 1e-6 * params[var].abs().max(1.0);
                probe[var] = params[var] + h;
                let plus = self.objective(&probe, hard_weight);
                probe[var] = params[var] - h;
                let minus = self.objective(&probe, hard_weight);
                probe[var] = params[var];
                for row in 0..m {
                    jacobian[row * n + column] = (plus[row] - minus[row]) / (2.0 * h);
                }
            }
            let mut jtj = vec![0.0; n * n];
            let mut jtr = vec![0.0; n];
            for row in 0..m {
                let base = row * n;
                for i in 0..n {
                    let ji = jacobian[base + i];
                    if ji == 0.0 {
                        continue;
                    }
                    jtr[i] += ji * r[row];
                    for j in 0..n {
                        jtj[i * n + j] += ji * jacobian[base + j];
                    }
                }
            }
            let mut improved = false;
            for _ in 0..12 {
                let mut system = jtj.clone();
                for (i, &var) in free.iter().enumerate() {
                    // Hard-only pass: damp with the soft weights so the step is
                    // the weighted minimum-norm correction. Soft pass: Marquardt.
                    let scale = if hard_weight.is_none() {
                        damping[var]
                    } else {
                        jtj[i * n + i].max(1e-9)
                    };
                    system[i * n + i] += lambda * scale;
                }
                let rhs: Vec<f64> = jtr.iter().map(|v| -v).collect();
                let Some(step) = solve_linear(system, rhs, n) else {
                    lambda *= 10.0;
                    continue;
                };
                let mut trial = params.to_vec();
                for (i, &var) in free.iter().enumerate() {
                    trial[var] += step[i];
                }
                let trial_cost = cost(&trial);
                if trial_cost.is_finite() && trial_cost < current {
                    let step_norm = step.iter().map(|s| s * s).sum::<f64>().sqrt();
                    params.copy_from_slice(&trial);
                    let relative = (current - trial_cost) / current.max(1e-300);
                    current = trial_cost;
                    lambda = (lambda / 3.0).max(1e-12);
                    improved = true;
                    if step_norm < 1e-12 || (hard_weight.is_some() && relative < 1e-12) {
                        return iteration + 1;
                    }
                    break;
                }
                lambda *= 4.0;
            }
            if !improved {
                return iteration + 1;
            }
        }
        options.max_iterations
    }
}

fn solve_linear(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    for column in 0..n {
        let pivot = (column..n)
            .max_by(|&i, &j| a[i * n + column].abs().total_cmp(&a[j * n + column].abs()))?;
        if a[pivot * n + column].abs() < 1e-300 {
            return None;
        }
        if pivot != column {
            for k in 0..n {
                a.swap(pivot * n + k, column * n + k);
            }
            b.swap(pivot, column);
        }
        for row in column + 1..n {
            let factor = a[row * n + column] / a[column * n + column];
            if factor == 0.0 {
                continue;
            }
            for k in column..n {
                a[row * n + k] -= factor * a[column * n + k];
            }
            b[row] -= factor * b[column];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut sum = b[row];
        for k in row + 1..n {
            sum -= a[row * n + k] * x[k];
        }
        x[row] = sum / a[row * n + row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

fn value(params: &[f64], value: Value) -> f64 {
    match value {
        Value::Const(v) => v,
        Value::Param(i) => params[i],
    }
}

fn pt(params: &[f64], p: PointVar) -> (f64, f64) {
    (params[p.x], params[p.y])
}

fn delta(params: &[f64], line: LineVar) -> (f64, f64) {
    let (ax, ay) = pt(params, line.a);
    let (bx, by) = pt(params, line.b);
    (bx - ax, by - ay)
}

fn line_length(params: &[f64], line: LineVar) -> f64 {
    let (dx, dy) = delta(params, line);
    dx.hypot(dy)
}

fn unit(params: &[f64], line: LineVar) -> (f64, f64) {
    let (dx, dy) = delta(params, line);
    let length = dx.hypot(dy).max(1e-12);
    (dx / length, dy / length)
}

fn wrap(angle: f64) -> f64 {
    angle.sin().atan2(angle.cos())
}

/// Undirected angle between two lines minus `offset`, wrapped to
/// [-pi/2, pi/2). Unlike the web solver's cross/dot residuals this has a
/// nonzero gradient even when the lines start exactly 90 degrees from the
/// target orientation, so a quarter-turn edit cannot stall at a saddle.
fn line_angle(p: &[f64], first: LineVar, second: LineVar, offset: f64) -> f64 {
    let a = unit(p, first);
    let b = unit(p, second);
    let half = std::f64::consts::FRAC_PI_2;
    (cross(a, b).atan2(dot(a, b)) - offset + half).rem_euclid(std::f64::consts::PI) - half
}

fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn residual(constraint: &Constraint, p: &[f64], out: &mut Vec<f64>) {
    match *constraint {
        Constraint::Fixed { point, at } => {
            let (x, y) = pt(p, point);
            out.extend([x - at.0, y - at.1]);
        }
        Constraint::Coincident(first, second) => {
            let (ax, ay) = pt(p, first);
            let (bx, by) = pt(p, second);
            out.extend([ax - bx, ay - by]);
        }
        Constraint::Horizontal(line) => out.push(delta(p, line).1),
        Constraint::Vertical(line) => out.push(delta(p, line).0),
        Constraint::Length { line, value: v } => out.push(line_length(p, line) - value(p, v)),
        Constraint::Rotation { line, degrees } => {
            let (dx, dy) = delta(p, line);
            out.push(wrap(dy.atan2(dx) - value(p, degrees).to_radians()) * line_length(p, line));
        }
        Constraint::EqualLength(first, second) => {
            out.push(line_length(p, first) - line_length(p, second))
        }
        Constraint::Parallel(first, second) => out.push(line_angle(p, first, second, 0.0)),
        Constraint::Perpendicular(first, second) => {
            out.push(line_angle(p, first, second, std::f64::consts::FRAC_PI_2))
        }
        Constraint::Collinear(first, second) => {
            let direction = unit(p, first);
            let (ax, ay) = pt(p, first.a);
            let (bx, by) = pt(p, second.a);
            out.extend([
                line_angle(p, first, second, 0.0),
                cross((bx - ax, by - ay), direction),
            ]);
        }
        Constraint::Angle { from, to, degrees } => {
            let a = unit(p, from);
            let b = unit(p, to);
            out.push(wrap(
                cross(a, b).atan2(dot(a, b)) - value(p, degrees).to_radians(),
            ));
        }
        Constraint::Midpoint { point, line } => {
            let (x, y) = pt(p, point);
            let (ax, ay) = pt(p, line.a);
            let (bx, by) = pt(p, line.b);
            out.extend([x - (ax + bx) * 0.5, y - (ay + by) * 0.5]);
        }
        Constraint::PointOnLine { point, line } => {
            let (x, y) = pt(p, point);
            let (ax, ay) = pt(p, line.a);
            out.push(cross((x - ax, y - ay), unit(p, line)));
        }
        Constraint::Radius { circle, value: v } => out.push(p[circle.radius] - value(p, v)),
        Constraint::EqualRadius(first, second) => out.push(p[first.radius] - p[second.radius]),
        Constraint::RadiusDifference {
            source,
            target,
            value: v,
        } => out.push(p[source.radius] - p[target.radius] - value(p, v)),
        Constraint::Concentric(first, second) => {
            let (ax, ay) = pt(p, first.center);
            let (bx, by) = pt(p, second.center);
            out.extend([ax - bx, ay - by]);
        }
        Constraint::PointOnCircle { point, circle } => {
            let (x, y) = pt(p, point);
            let (cx, cy) = pt(p, circle.center);
            out.push((x - cx).hypot(y - cy) - p[circle.radius]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(problem: &mut Problem, a: (f64, f64), b: (f64, f64), lock_a: bool) -> LineVar {
        LineVar {
            a: problem.point(a.0, a.1, lock_a),
            b: problem.point(b.0, b.1, false),
        }
    }

    fn circle(problem: &mut Problem, c: (f64, f64), r: f64, locked: bool) -> CircleVar {
        CircleVar {
            center: problem.point(c.0, c.1, locked),
            radius: problem.param(r, locked),
        }
    }

    fn soft_keep_all(problem: &mut Problem) {
        for var in 0..problem.params.len() {
            let value = problem.params[var];
            problem.soft.push(Soft::Target {
                var,
                value,
                weight: 1e-2,
            });
        }
    }

    fn solved(problem: &Problem) -> SolveResult {
        let result = problem.solve(Options::default());
        assert!(result.converged, "{result:?}");
        assert!(result.residual_norm <= 1e-6);
        result
    }

    fn close(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-5, "{a} != {b}");
    }

    #[test]
    fn equal_parallel_and_coincident_lines_solve_consistently() {
        let mut problem = Problem::default();
        let first = line(&mut problem, (0.0, 0.0), (10.0, 0.0), true);
        problem.locked[first.b.x] = true;
        problem.locked[first.b.y] = true;
        let second = line(&mut problem, (3.0, 5.0), (7.0, 9.0), false);
        let point = problem.point(2.0, 6.0, false);
        problem.constraints = vec![
            Constraint::EqualLength(first, second),
            Constraint::Parallel(first, second),
            Constraint::Coincident(second.a, point),
        ];
        soft_keep_all(&mut problem);
        let result = solved(&problem);
        let p = &result.params;
        close(line_length(p, second), 10.0);
        close(delta(p, second).1, 0.0);
        close(p[second.a.x], p[point.x]);
        close(p[second.a.y], p[point.y]);
    }

    #[test]
    fn conflicting_constraints_report_non_convergence() {
        let mut problem = Problem::default();
        let l = line(&mut problem, (0.0, 0.0), (10.0, 1.0), true);
        problem.constraints = vec![
            Constraint::Horizontal(l),
            Constraint::Vertical(l),
            Constraint::Length {
                line: l,
                value: Value::Const(5.0),
            },
        ];
        let result = problem.solve(Options::default());
        assert!(!result.converged);
        assert!(result.residual_norm > 1e-3);
    }

    #[test]
    fn fixed_and_coincident_points() {
        let mut problem = Problem::default();
        let a = problem.point(1.0, 1.0, false);
        let b = problem.point(4.0, 4.0, false);
        problem.constraints = vec![
            Constraint::Fixed {
                point: a,
                at: (2.0, 3.0),
            },
            Constraint::Coincident(a, b),
        ];
        let p = solved(&problem).params;
        close(p[b.x], 2.0);
        close(p[b.y], 3.0);
    }

    #[test]
    fn axis_and_length_keep_first_endpoint() {
        let mut problem = Problem::default();
        let h = line(&mut problem, (0.0, 0.0), (3.0, 4.0), true);
        let v = line(&mut problem, (0.0, 0.0), (3.0, 4.0), true);
        problem.constraints = vec![
            Constraint::Horizontal(h),
            Constraint::Length {
                line: h,
                value: Value::Const(5.0),
            },
            Constraint::Vertical(v),
            Constraint::Length {
                line: v,
                value: Value::Const(8.0),
            },
        ];
        let p = solved(&problem).params;
        close(p[h.b.x], 5.0);
        close(p[h.b.y], 0.0);
        close(p[v.b.x], 0.0);
        close(p[v.b.y], 8.0);
    }

    #[test]
    fn free_dimension_parameter_is_measured() {
        let mut problem = Problem::default();
        let l = line(&mut problem, (0.0, 0.0), (6.0, 8.0), true);
        problem.locked[l.b.x] = true;
        problem.locked[l.b.y] = true;
        let length = problem.param(1.0, false);
        problem.constraints = vec![Constraint::Length {
            line: l,
            value: Value::Param(length),
        }];
        close(solved(&problem).params[length], 10.0);
    }

    #[test]
    fn perpendicular_angle_and_rotation() {
        let mut problem = Problem::default();
        let base = line(&mut problem, (0.0, 0.0), (10.0, 0.0), true);
        problem.locked[base.b.x] = true;
        problem.locked[base.b.y] = true;
        let perpendicular = line(&mut problem, (0.0, 0.0), (5.0, 5.0), true);
        let angled = line(&mut problem, (0.0, 0.0), (5.0, 1.0), true);
        let rotated = line(&mut problem, (0.0, 0.0), (5.0, 1.0), true);
        problem.constraints = vec![
            Constraint::Perpendicular(base, perpendicular),
            Constraint::Angle {
                from: base,
                to: angled,
                degrees: Value::Const(30.0),
            },
            Constraint::Rotation {
                line: rotated,
                degrees: Value::Const(-45.0),
            },
        ];
        soft_keep_all(&mut problem);
        let p = solved(&problem).params;
        close(dot(unit(&p, base), unit(&p, perpendicular)), 0.0);
        let d = delta(&p, angled);
        close(d.1.atan2(d.0).to_degrees(), 30.0);
        let d = delta(&p, rotated);
        close(d.1.atan2(d.0).to_degrees(), -45.0);
    }

    #[test]
    fn collinear_moves_target_onto_source_line() {
        let mut problem = Problem::default();
        let source = line(&mut problem, (0.0, 0.0), (10.0, 10.0), true);
        problem.locked[source.b.x] = true;
        problem.locked[source.b.y] = true;
        let target = line(&mut problem, (20.0, 18.0), (30.0, 31.0), false);
        problem.constraints = vec![Constraint::Collinear(source, target)];
        soft_keep_all(&mut problem);
        let p = solved(&problem).params;
        close(p[target.a.x], p[target.a.y]);
        close(p[target.b.x], p[target.b.y]);
    }

    #[test]
    fn midpoint_and_point_on_line() {
        let mut problem = Problem::default();
        let l = line(&mut problem, (0.0, 0.0), (10.0, 4.0), true);
        problem.locked[l.b.x] = true;
        problem.locked[l.b.y] = true;
        let mid = problem.point(1.0, 1.0, false);
        let on = problem.point(3.0, 7.0, false);
        problem.constraints = vec![
            Constraint::Midpoint {
                point: mid,
                line: l,
            },
            Constraint::PointOnLine { point: on, line: l },
        ];
        soft_keep_all(&mut problem);
        let p = solved(&problem).params;
        close(p[mid.x], 5.0);
        close(p[mid.y], 2.0);
        close(p[on.y], p[on.x] * 0.4);
    }

    #[test]
    fn circular_relations() {
        let mut problem = Problem::default();
        let source = circle(&mut problem, (0.0, 0.0), 5.0, true);
        let equal = circle(&mut problem, (20.0, 0.0), 3.0, false);
        let smaller = circle(&mut problem, (1.0, 1.0), 1.0, false);
        let sized = circle(&mut problem, (-20.0, 0.0), 1.0, false);
        let on = problem.point(0.0, 9.0, false);
        problem.constraints = vec![
            Constraint::EqualRadius(source, equal),
            Constraint::RadiusDifference {
                source,
                target: smaller,
                value: Value::Const(2.0),
            },
            Constraint::Concentric(source, smaller),
            Constraint::Radius {
                circle: sized,
                value: Value::Const(4.0),
            },
            Constraint::PointOnCircle {
                point: on,
                circle: source,
            },
        ];
        soft_keep_all(&mut problem);
        let p = solved(&problem).params;
        close(p[equal.radius], 5.0);
        close(p[smaller.radius], 3.0);
        close(p[smaller.center.x], 0.0);
        close(p[sized.radius], 4.0);
        close(p[on.x].hypot(p[on.y]), 5.0);
    }

    #[test]
    fn locked_parameters_never_move() {
        let mut problem = Problem::default();
        let a = problem.point(1.0, 2.0, true);
        let b = problem.point(5.0, 5.0, false);
        problem.constraints = vec![Constraint::Coincident(a, b)];
        let p = solved(&problem).params;
        assert_eq!((p[a.x], p[a.y]), (1.0, 2.0));
    }

    #[test]
    fn locked_conflict_does_not_converge() {
        let mut problem = Problem::default();
        let a = problem.point(1.0, 2.0, true);
        problem.constraints = vec![Constraint::Fixed {
            point: a,
            at: (0.0, 0.0),
        }];
        assert!(!problem.solve(Options::default()).converged);
    }
}
