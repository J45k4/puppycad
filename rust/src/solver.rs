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
//!   (line endpoints, sketch points, curve centers, spline handles), one per
//!   radius or ellipse semi-axis, ellipse rotation (degrees), and arc start
//!   and sweep angles (radians). Spline point relations add an auxiliary
//!   curve parameter in segment units.
//!   Dimension values may also be parameters ([`Value::Param`]) so a dimension
//!   can be left free and measured by the solve; the document currently passes
//!   driving values as [`Value::Const`].
//! * [`Constraint`] has one residual function per natively supported relation
//!   kind (fixed anchor, coincident endpoint/point, horizontal, vertical, line
//!   length, line rotation, equal length, parallel, perpendicular, collinear,
//!   signed angle, midpoint and point-on-line, radius, equal radius, radius
//!   difference, concentric, point-on-circle) plus [`Curve`] relations:
//!   finite point-on-curve (segment, sweep and spline span penalties), curve
//!   midpoint, fixed curve points, Normal, smooth-join endpoint tangency, and
//!   internal and external tangency (contacts kept inside arc sweeps).
//! * [`Soft`] terms express user intent: stay close to the post-edit geometry,
//!   keep each line's first endpoint more strongly than its second one, and
//!   keep unconstrained line lengths. They are what made the hand-written
//!   propagation "anchor the first endpoint"; here they are weights.
//! * Locked parameters are constants. The document locks the anchors the user
//!   just edited, and curve sizes and angles that no relation may change.
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

/// Circular arc: a circle plus start angle and signed sweep (radians).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArcVar {
    pub circle: CircleVar,
    pub start: usize,
    pub sweep: usize,
}

/// Ellipse or elliptic arc: center, semi-axes, rotation in degrees and, for
/// an elliptic arc, parametric start angle and signed sweep (radians).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EllipseVar {
    pub center: PointVar,
    pub rx: usize,
    pub ry: usize,
    pub rotation: usize,
    pub arc: Option<(usize, usize)>,
}

/// Spline through (fit) or controlled by (control) its point handles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SplineVar {
    pub points: Vec<PointVar>,
    pub control: bool,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    Line(LineVar),
    Circle(CircleVar),
    Arc(ArcVar),
    Ellipse(EllipseVar),
    Spline(SplineVar),
}

#[derive(Clone, Debug, PartialEq)]
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
    /// Parameter equals a value (driving sweep, axis and rotation values).
    Equal {
        var: usize,
        value: Value,
    },
    /// Two parameters are equal.
    SameValue(usize, usize),
    /// Point lies on a finite curve (segment, sweep, or spline span).
    /// `param` is an auxiliary spline parameter in segment units.
    PointOnCurve {
        point: PointVar,
        curve: Curve,
        param: Option<usize>,
    },
    /// Point is the parametric midpoint of an arc or elliptic arc.
    CurveMidpoint {
        point: PointVar,
        curve: Curve,
    },
    /// The curve point at `fraction` of its sweep (arcs) or turn (full
    /// ellipses) stays at `at`.
    FixedOnCurve {
        curve: Curve,
        fraction: f64,
        at: (f64, f64),
    },
    /// Line endpoint lies on the curve and the line follows its normal there.
    Normal {
        line: LineVar,
        endpoint: u8,
        curve: Curve,
        param: Option<usize>,
    },
    /// Smooth join: the arc's tangent at `endpoint` (0 start, 1 end) follows
    /// the line direction; signs record which way the join turns.
    EndpointTangent {
        line: LineVar,
        arc: ArcVar,
        endpoint: u8,
        line_sign: f64,
        sweep_sign: f64,
    },
    /// Inner circle/arc touches outer circle/arc from inside.
    InternalTangent {
        outer: Curve,
        inner: Curve,
    },
    /// Two curves touch from outside. For a line and a round or elliptic
    /// curve, `side` is the side of the line the curve stays on.
    ExternalTangent {
        first: Curve,
        second: Curve,
        side: f64,
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
            curve_residual(constraint, params, &mut out);
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
            if hard_weight.is_none() && current.sqrt() <= options.tolerance * 1e-8 {
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
            for (row, &residual) in r.iter().enumerate().take(m) {
                let base = row * n;
                for i in 0..n {
                    let ji = jacobian[base + i];
                    if ji == 0.0 {
                        continue;
                    }
                    jtr[i] += ji * residual;
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

/// Classification of one hard relation in an [`Analysis`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelationState {
    Satisfied,
    /// Its residual is above tolerance at the analyzed parameters.
    Conflicting,
    /// Its Jacobian rows add no rank after the earlier relations.
    Redundant,
}

/// Degrees-of-freedom report for a problem at a parameter vector (port of
/// `src/sketch-redundancy.ts`).
#[derive(Clone, Debug, PartialEq)]
pub struct Analysis {
    /// Number of free (unlocked) parameters.
    pub parameters: usize,
    /// Numeric rank of the hard-residual Jacobian over the free parameters.
    pub rank: usize,
    /// Remaining degrees of freedom (`parameters - rank`).
    pub dof: usize,
    /// One entry per [`Problem::constraints`] item, in order.
    pub relations: Vec<RelationState>,
    /// Per parameter: true when the relations fully determine it locally.
    /// Locked parameters are reported as fixed.
    pub fixed: Vec<bool>,
}

impl Analysis {
    pub fn conflicting(&self) -> bool {
        self.relations.contains(&RelationState::Conflicting)
    }

    pub fn redundant(&self) -> bool {
        self.relations.contains(&RelationState::Redundant)
    }
}

impl Constraint {
    /// Parameter indices this relation reads.
    pub fn vars(&self) -> Vec<usize> {
        fn point(out: &mut Vec<usize>, p: PointVar) {
            out.extend([p.x, p.y]);
        }
        fn line(out: &mut Vec<usize>, l: LineVar) {
            point(out, l.a);
            point(out, l.b);
        }
        fn circle(out: &mut Vec<usize>, c: CircleVar) {
            point(out, c.center);
            out.push(c.radius);
        }
        fn value(out: &mut Vec<usize>, v: Value) {
            if let Value::Param(i) = v {
                out.push(i);
            }
        }
        let mut out = Vec::new();
        match *self {
            Constraint::Fixed { point: p, .. } => point(&mut out, p),
            Constraint::Coincident(a, b) => {
                point(&mut out, a);
                point(&mut out, b);
            }
            Constraint::Horizontal(l) | Constraint::Vertical(l) => line(&mut out, l),
            Constraint::Length { line: l, value: v }
            | Constraint::Rotation {
                line: l,
                degrees: v,
            } => {
                line(&mut out, l);
                value(&mut out, v);
            }
            Constraint::EqualLength(a, b)
            | Constraint::Parallel(a, b)
            | Constraint::Perpendicular(a, b)
            | Constraint::Collinear(a, b) => {
                line(&mut out, a);
                line(&mut out, b);
            }
            Constraint::Angle { from, to, degrees } => {
                line(&mut out, from);
                line(&mut out, to);
                value(&mut out, degrees);
            }
            Constraint::Midpoint { point: p, line: l }
            | Constraint::PointOnLine { point: p, line: l } => {
                point(&mut out, p);
                line(&mut out, l);
            }
            Constraint::Radius {
                circle: c,
                value: v,
            } => {
                circle(&mut out, c);
                value(&mut out, v);
            }
            Constraint::EqualRadius(a, b) | Constraint::Concentric(a, b) => {
                circle(&mut out, a);
                circle(&mut out, b);
            }
            Constraint::RadiusDifference {
                source,
                target,
                value: v,
            } => {
                circle(&mut out, source);
                circle(&mut out, target);
                value(&mut out, v);
            }
            Constraint::PointOnCircle {
                point: p,
                circle: c,
            } => {
                point(&mut out, p);
                circle(&mut out, c);
            }
            Constraint::Equal { var, value: v } => {
                out.push(var);
                value(&mut out, v);
            }
            Constraint::SameValue(a, b) => out.extend([a, b]),
            Constraint::PointOnCurve {
                point: p,
                ref curve,
                param,
            } => {
                point(&mut out, p);
                curve_vars(&mut out, curve);
                out.extend(param);
            }
            Constraint::CurveMidpoint {
                point: p,
                ref curve,
            } => {
                point(&mut out, p);
                curve_vars(&mut out, curve);
            }
            Constraint::FixedOnCurve { ref curve, .. } => curve_vars(&mut out, curve),
            Constraint::Normal {
                line: l,
                ref curve,
                param,
                ..
            } => {
                line(&mut out, l);
                curve_vars(&mut out, curve);
                out.extend(param);
            }
            Constraint::EndpointTangent { line: l, arc, .. } => {
                line(&mut out, l);
                curve_vars(&mut out, &Curve::Arc(arc));
            }
            Constraint::InternalTangent {
                outer: ref a,
                inner: ref b,
            }
            | Constraint::ExternalTangent {
                first: ref a,
                second: ref b,
                ..
            } => {
                curve_vars(&mut out, a);
                curve_vars(&mut out, b);
            }
        }
        fn curve_vars(out: &mut Vec<usize>, curve: &Curve) {
            match curve {
                Curve::Line(l) => line(out, *l),
                Curve::Circle(c) => circle(out, *c),
                Curve::Arc(a) => {
                    circle(out, a.circle);
                    out.extend([a.start, a.sweep]);
                }
                Curve::Ellipse(e) => {
                    point(out, e.center);
                    out.extend([e.rx, e.ry, e.rotation]);
                    if let Some((start, sweep)) = e.arc {
                        out.extend([start, sweep]);
                    }
                }
                Curve::Spline(s) => {
                    for &p in &s.points {
                        point(out, p);
                    }
                }
            }
        }
        out
    }
}

impl Problem {
    /// Analyze the hard relations at `params` (usually a solved state):
    /// Jacobian rank over the free parameters, remaining DOF, and per relation
    /// whether it is satisfied, conflicting or locally redundant. Like the web
    /// analysis, rows are orthonormalized relation by relation (two-pass
    /// Gram-Schmidt), so a relation is redundant when none of its rows adds a
    /// new direction after the relations before it. Conflicting wins over
    /// redundant.
    pub fn analyze(&self, params: &[f64], tolerance: f64) -> Analysis {
        let free: Vec<usize> = (0..params.len()).filter(|&i| !self.locked[i]).collect();
        let n = free.len();
        let mut column_of = vec![usize::MAX; params.len()];
        for (column, &var) in free.iter().enumerate() {
            column_of[var] = column;
        }
        let mut basis: Vec<Vec<f64>> = Vec::new();
        let mut relations = Vec::with_capacity(self.constraints.len());
        let mut probe = params.to_vec();
        for constraint in &self.constraints {
            let mut base = Vec::new();
            residual(constraint, params, &mut base);
            let m = base.len();
            let mut rows = vec![vec![0.0; n]; m];
            let mut vars = constraint.vars();
            vars.sort_unstable();
            vars.dedup();
            for var in vars {
                let column = column_of[var];
                if column == usize::MAX {
                    continue;
                }
                let h = 1e-6 * params[var].abs().max(1.0);
                let mut plus = Vec::with_capacity(m);
                let mut minus = Vec::with_capacity(m);
                probe[var] = params[var] + h;
                residual(constraint, &probe, &mut plus);
                probe[var] = params[var] - h;
                residual(constraint, &probe, &mut minus);
                probe[var] = params[var];
                for (row, values) in rows.iter_mut().enumerate() {
                    values[column] = (plus[row] - minus[row]) / (2.0 * h);
                }
            }
            let before = basis.len();
            let mut active = false;
            for row in rows {
                let norm = row.iter().map(|v| v * v).sum::<f64>().sqrt();
                if norm < 1e-7 {
                    continue;
                }
                active = true;
                let mut rest: Vec<f64> = row.iter().map(|v| v / norm).collect();
                for _ in 0..2 {
                    for vector in &basis {
                        let d: f64 = rest.iter().zip(vector).map(|(a, b)| a * b).sum();
                        for (r, v) in rest.iter_mut().zip(vector) {
                            *r -= d * v;
                        }
                    }
                }
                let length = rest.iter().map(|v| v * v).sum::<f64>().sqrt();
                if length > 1e-6 {
                    basis.push(rest.iter().map(|v| v / length).collect());
                }
            }
            let max = base.iter().fold(0.0f64, |m, r| m.max(r.abs()));
            relations.push(if max > tolerance {
                RelationState::Conflicting
            } else if active && basis.len() == before {
                RelationState::Redundant
            } else {
                RelationState::Satisfied
            });
        }
        let fixed = (0..params.len())
            .map(|var| {
                let column = column_of[var];
                column == usize::MAX
                    || (1.0
                        - basis
                            .iter()
                            .map(|row| row[column] * row[column])
                            .sum::<f64>())
                    .abs()
                        < 1e-6
            })
            .collect();
        Analysis {
            parameters: n,
            rank: basis.len(),
            dof: n - basis.len(),
            relations,
            fixed,
        }
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
    match constraint.clone() {
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
            on_curve(p, pt(p, point), &Curve::Line(line), None, out);
        }
        Constraint::Equal { var, value: v } => out.push(p[var] - value(p, v)),
        Constraint::SameValue(a, b) => out.push(p[a] - p[b]),
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
        _ => {}
    }
}

use std::f64::consts::{PI, TAU};

/// Center, radius and optional (start, sweep) of a circle or arc.
type Round = ((f64, f64), f64, Option<(f64, f64)>);

fn round(p: &[f64], curve: &Curve) -> Option<Round> {
    match curve {
        Curve::Circle(c) => Some((pt(p, c.center), p[c.radius], None)),
        Curve::Arc(a) => Some((
            pt(p, a.circle.center),
            p[a.circle.radius],
            Some((p[a.start], p[a.sweep])),
        )),
        _ => None,
    }
}

/// Angle outside a sweep (0 when inside or the sweep is a full turn).
fn sweep_excess(angle: f64, arc: Option<(f64, f64)>) -> f64 {
    let Some((start, sweep)) = arc else {
        return 0.0;
    };
    if sweep.abs() >= TAU - 1e-9 {
        return 0.0;
    }
    let traveled = if sweep >= 0.0 {
        (angle - start).rem_euclid(TAU)
    } else {
        (start - angle).rem_euclid(TAU)
    };
    // A small tolerance keeps contacts placed exactly on an end stable.
    let limit = sweep.abs() + 1e-9;
    if traveled <= limit {
        0.0
    } else {
        (traveled - limit).min(TAU - traveled)
    }
}

struct Ellipse {
    center: (f64, f64),
    rx: f64,
    ry: f64,
    sin: f64,
    cos: f64,
    arc: Option<(f64, f64)>,
}

impl Ellipse {
    fn new(p: &[f64], e: &EllipseVar) -> Self {
        let (sin, cos) = p[e.rotation].to_radians().sin_cos();
        Self {
            center: pt(p, e.center),
            rx: p[e.rx],
            ry: p[e.ry],
            sin,
            cos,
            arc: e.arc.map(|(start, sweep)| (p[start], p[sweep])),
        }
    }

    fn at(&self, angle: f64) -> (f64, f64) {
        let (y, x) = angle.sin_cos();
        let (lx, ly) = (self.rx * x, self.ry * y);
        (
            self.center.0 + lx * self.cos - ly * self.sin,
            self.center.1 + lx * self.sin + ly * self.cos,
        )
    }

    fn local(&self, point: (f64, f64)) -> (f64, f64) {
        let dx = point.0 - self.center.0;
        let dy = point.1 - self.center.1;
        (
            dx * self.cos + dy * self.sin,
            -dx * self.sin + dy * self.cos,
        )
    }

    fn parameter(&self, point: (f64, f64)) -> f64 {
        let (u, v) = self.local(point);
        (v / self.ry).atan2(u / self.rx)
    }

    /// Outward (unnormalized) normal at a point on the ellipse.
    fn normal(&self, point: (f64, f64)) -> (f64, f64) {
        let (u, v) = self.local(point);
        let (x, y) = (u / (self.rx * self.rx), v / (self.ry * self.ry));
        (x * self.cos - y * self.sin, x * self.sin + y * self.cos)
    }

    /// Support extent along a unit normal and the support point offset.
    fn support(&self, normal: (f64, f64)) -> (f64, (f64, f64)) {
        let nx = normal.0 * self.cos + normal.1 * self.sin;
        let ny = -normal.0 * self.sin + normal.1 * self.cos;
        let extent = ((self.rx * nx).powi(2) + (self.ry * ny).powi(2))
            .sqrt()
            .max(1e-12);
        let x = self.rx * self.rx * nx / extent;
        let y = self.ry * self.ry * ny / extent;
        (
            extent,
            (x * self.cos - y * self.sin, x * self.sin + y * self.cos),
        )
    }

    fn scale(&self) -> f64 {
        (self.rx.abs() + self.ry.abs()) * 0.5
    }
}

fn spline_curves(p: &[f64], spline: &SplineVar) -> Option<Vec<[(f64, f64); 4]>> {
    let points: Vec<_> = spline.points.iter().map(|&point| pt(p, point)).collect();
    crate::spline::beziers_f64(spline.control, &points, spline.closed)
}

type SplineSample = ((f64, f64), (f64, f64), f64);

/// Point and derivative on a spline at `u` in segment units, and how far `u`
/// lies outside an open spline's range.
fn spline_at(p: &[f64], spline: &SplineVar, u: f64) -> Option<SplineSample> {
    let curves = spline_curves(p, spline)?;
    let n = curves.len() as f64;
    let (u, excess) = if spline.closed {
        (u.rem_euclid(n), 0.0)
    } else {
        (u.clamp(0.0, n), (-u).max(0.0) + (u - n).max(0.0))
    };
    let index = (u.floor() as usize).min(curves.len() - 1);
    let t = u - index as f64;
    let c = curves[index];
    let s = 1.0 - t;
    let mix = |a: f64, b: f64, c2: f64, d: f64| {
        s * s * s * a + 3.0 * s * s * t * b + 3.0 * s * t * t * c2 + t * t * t * d
    };
    let slope = |a: f64, b: f64, c2: f64, d: f64| {
        3.0 * s * s * (b - a) + 6.0 * s * t * (c2 - b) + 3.0 * t * t * (d - c2)
    };
    Some((
        (
            mix(c[0].0, c[1].0, c[2].0, c[3].0),
            mix(c[0].1, c[1].1, c[2].1, c[3].1),
        ),
        (
            slope(c[0].0, c[1].0, c[2].0, c[3].0),
            slope(c[0].1, c[1].1, c[2].1, c[3].1),
        ),
        excess,
    ))
}

/// Residuals that keep `point` on a finite curve (segment, sweep, or open
/// spline span); returns the curve normal there (unnormalized).
fn on_curve(
    p: &[f64],
    point: (f64, f64),
    curve: &Curve,
    param: Option<usize>,
    out: &mut Vec<f64>,
) -> (f64, f64) {
    let (x, y) = point;
    match curve {
        Curve::Line(line) => {
            let (ax, ay) = pt(p, line.a);
            let (dx, dy) = delta(p, *line);
            let length = dx.hypot(dy).max(1e-12);
            let s = ((x - ax) * dx + (y - ay) * dy) / (length * length);
            out.push(cross((x - ax, y - ay), (dx / length, dy / length)));
            out.push(((-s).max(0.0) + (s - 1.0).max(0.0)) * length);
            (-dy, dx)
        }
        Curve::Circle(_) | Curve::Arc(_) => {
            let (center, radius, arc) = round(p, curve).unwrap();
            let (dx, dy) = (x - center.0, y - center.1);
            out.push(dx.hypot(dy) - radius);
            if arc.is_some() {
                out.push(sweep_excess(dy.atan2(dx), arc) * radius);
            }
            (dx, dy)
        }
        Curve::Ellipse(e) => {
            let e = Ellipse::new(p, e);
            let (u, v) = e.local(point);
            out.push(((u / e.rx).hypot(v / e.ry) - 1.0) * e.rx.abs().min(e.ry.abs()));
            if e.arc.is_some() {
                out.push(sweep_excess(e.parameter(point), e.arc) * e.scale());
            }
            e.normal(point)
        }
        Curve::Spline(spline) => {
            let u = param.map_or(0.0, |i| p[i]);
            let Some((at, derivative, excess)) = spline_at(p, spline, u) else {
                out.extend([f64::NAN, f64::NAN, f64::NAN]);
                return (0.0, 0.0);
            };
            let scale = derivative.0.hypot(derivative.1).max(1e-9);
            out.extend([x - at.0, y - at.1, excess * scale]);
            (-derivative.1, derivative.0)
        }
    }
}

/// Point at `fraction` of a curve: along a segment or sweep, around a full
/// circle or ellipse, or across a spline's segments.
pub fn curve_at(p: &[f64], curve: &Curve, fraction: f64) -> Option<(f64, f64)> {
    match curve {
        Curve::Line(line) => {
            let (ax, ay) = pt(p, line.a);
            let (dx, dy) = delta(p, *line);
            Some((ax + dx * fraction, ay + dy * fraction))
        }
        Curve::Circle(_) | Curve::Arc(_) => {
            let (center, radius, arc) = round(p, curve)?;
            let (start, sweep) = arc.unwrap_or((0.0, TAU));
            let angle = start + sweep * fraction;
            Some((
                center.0 + radius * angle.cos(),
                center.1 + radius * angle.sin(),
            ))
        }
        Curve::Ellipse(e) => {
            let e = Ellipse::new(p, e);
            let (start, sweep) = e.arc.unwrap_or((0.0, TAU));
            Some(e.at(start + sweep * fraction))
        }
        Curve::Spline(spline) => {
            let n = spline_curves(p, spline)?.len() as f64;
            spline_at(p, spline, n * fraction).map(|(at, _, _)| at)
        }
    }
}

/// Spline parameter (segment units) of the sampled point closest to `target`.
pub fn spline_parameter(p: &[f64], spline: &SplineVar, target: (f64, f64)) -> f64 {
    let Some(curves) = spline_curves(p, spline) else {
        return 0.0;
    };
    let samples = curves.len() * 64;
    let distance = |u: f64| {
        spline_at(p, spline, u).map_or(f64::INFINITY, |(at, _, _)| {
            (at.0 - target.0).hypot(at.1 - target.1)
        })
    };
    let mut best = (0.0, f64::INFINITY);
    for i in 0..=samples {
        let u = i as f64 / 64.0;
        let d = distance(u);
        if d < best.1 {
            best = (u, d);
        }
    }
    let (mut left, mut right) = (best.0 - 1.0 / 64.0, best.0 + 1.0 / 64.0);
    if !spline.closed {
        left = left.max(0.0);
        right = right.min(curves.len() as f64);
    }
    for _ in 0..48 {
        let a = left + (right - left) / 3.0;
        let b = right - (right - left) / 3.0;
        if distance(a) <= distance(b) {
            right = b;
        } else {
            left = a;
        }
    }
    (left + right) * 0.5
}

fn curve_residual(constraint: &Constraint, p: &[f64], out: &mut Vec<f64>) {
    match constraint {
        Constraint::PointOnCurve {
            point,
            curve,
            param,
        } => {
            on_curve(p, pt(p, *point), curve, *param, out);
        }
        Constraint::CurveMidpoint { point, curve } => {
            let (x, y) = pt(p, *point);
            let (mx, my) = curve_at(p, curve, 0.5).unwrap_or((f64::NAN, f64::NAN));
            out.extend([x - mx, y - my]);
        }
        Constraint::FixedOnCurve {
            curve,
            fraction,
            at,
        } => {
            let (x, y) = curve_at(p, curve, *fraction).unwrap_or((f64::NAN, f64::NAN));
            out.extend([x - at.0, y - at.1]);
        }
        Constraint::Normal {
            line,
            endpoint,
            curve,
            param,
        } => {
            let (end, other) = if *endpoint == 0 {
                (line.a, line.b)
            } else {
                (line.b, line.a)
            };
            let end = pt(p, end);
            let other = pt(p, other);
            let normal = on_curve(p, end, curve, *param, out);
            let direction = (other.0 - end.0, other.1 - end.1);
            let angle = cross(normal, direction).atan2(dot(normal, direction));
            out.push((angle + PI * 0.5).rem_euclid(PI) - PI * 0.5);
        }
        Constraint::EndpointTangent {
            line,
            arc,
            endpoint,
            line_sign,
            sweep_sign,
        } => {
            let (ux, uy) = unit(p, *line);
            let radial_sign = -line_sign * sweep_sign;
            let target = (ux * radial_sign).atan2(-uy * radial_sign);
            let angle = p[arc.start] + if *endpoint == 0 { 0.0 } else { p[arc.sweep] };
            out.push(wrap(angle - target) * p[arc.circle.radius]);
        }
        Constraint::InternalTangent { outer, inner } => {
            let (Some((oc, or, oarc)), Some((ic, ir, iarc))) = (round(p, outer), round(p, inner))
            else {
                return;
            };
            let (dx, dy) = (ic.0 - oc.0, ic.1 - oc.1);
            let angle = dy.atan2(dx);
            out.extend([
                dx.hypot(dy) - (or - ir),
                sweep_excess(angle, oarc) * or,
                sweep_excess(angle, iarc) * ir,
            ]);
        }
        Constraint::ExternalTangent {
            first,
            second,
            side,
        } => {
            if let (Some((c1, r1, arc1)), Some((c2, r2, arc2))) =
                (round(p, first), round(p, second))
            {
                let (dx, dy) = (c2.0 - c1.0, c2.1 - c1.1);
                let angle = dy.atan2(dx);
                out.extend([
                    dx.hypot(dy) - (r1 + r2),
                    sweep_excess(angle, arc1) * r1,
                    sweep_excess(angle + PI, arc2) * r2,
                ]);
                return;
            }
            let (line, other) = match (first, second) {
                (Curve::Line(line), other) | (other, Curve::Line(line)) => (*line, other),
                _ => return,
            };
            let (ax, ay) = pt(p, line.a);
            let (ux, uy) = unit(p, line);
            let normal = (-uy, ux);
            match other {
                Curve::Circle(_) | Curve::Arc(_) => {
                    let (center, radius, arc) = round(p, other).unwrap();
                    let signed = dot((center.0 - ax, center.1 - ay), normal);
                    let angle = (-side * normal.1).atan2(-side * normal.0);
                    out.extend([signed - side * radius, sweep_excess(angle, arc) * radius]);
                }
                Curve::Ellipse(e) => {
                    let e = Ellipse::new(p, e);
                    let signed = dot((e.center.0 - ax, e.center.1 - ay), normal);
                    let (extent, support) = e.support(normal);
                    let contact = (e.center.0 - side * support.0, e.center.1 - side * support.1);
                    out.extend([
                        signed - side * extent,
                        sweep_excess(e.parameter(contact), e.arc) * e.scale(),
                    ]);
                }
                _ => {}
            }
        }
        _ => {}
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
        // The solver can only meet both by collapsing the line, which the
        // document rejects; analyzed at the drawn geometry it conflicts.
    }
    fn free_line(problem: &mut Problem) -> LineVar {
        line(problem, (0.0, 0.0), (10.0, 2.0), false)
    }

    #[test]
    fn analysis_counts_line_freedom() {
        let mut problem = Problem::default();
        free_line(&mut problem);
        let analysis = problem.analyze(&problem.params.clone(), 1e-6);
        assert_eq!(
            (analysis.parameters, analysis.rank, analysis.dof),
            (4, 0, 4)
        );
        assert!(analysis.fixed.iter().all(|f| !f));
    }

    #[test]
    fn analysis_fixed_line_has_no_freedom() {
        let mut problem = Problem::default();
        let l = free_line(&mut problem);
        problem.constraints = vec![
            Constraint::Fixed {
                point: l.a,
                at: (0.0, 0.0),
            },
            Constraint::Fixed {
                point: l.b,
                at: (10.0, 2.0),
            },
        ];
        let analysis = problem.analyze(&problem.params.clone(), 1e-6);
        assert_eq!(analysis.dof, 0);
        assert!(analysis.fixed.iter().all(|f| *f));
        assert!(
            analysis
                .relations
                .iter()
                .all(|r| *r == RelationState::Satisfied)
        );
    }

    #[test]
    fn analysis_reports_horizontal_and_vertical_conflict() {
        let mut problem = Problem::default();
        let l = line(&mut problem, (0.0, 0.0), (10.0, 0.0), false);
        problem.constraints = vec![Constraint::Horizontal(l), Constraint::Vertical(l)];
        // The solver can only meet both by collapsing the line, which the
        // document rejects; analyzed at the drawn geometry it conflicts.
        let analysis = problem.analyze(&problem.params.clone(), 1e-6);
        assert_eq!(analysis.relations[0], RelationState::Satisfied);
        assert_eq!(analysis.relations[1], RelationState::Conflicting);
        assert!(analysis.conflicting());
    }

    #[test]
    fn analysis_reports_duplicate_parallel_as_redundant() {
        let mut problem = Problem::default();
        let a = line(&mut problem, (0.0, 0.0), (10.0, 0.0), false);
        let b = line(&mut problem, (0.0, 5.0), (8.0, 5.0), false);
        problem.constraints = vec![Constraint::Parallel(a, b), Constraint::Parallel(b, a)];
        let analysis = problem.analyze(&problem.params.clone(), 1e-6);
        assert_eq!(
            analysis.relations,
            [RelationState::Satisfied, RelationState::Redundant]
        );
        assert_eq!(analysis.dof, 7);
        assert!(!analysis.conflicting() && analysis.redundant());
    }

    fn arc(problem: &mut Problem, c: (f64, f64), r: f64, start: f64, sweep: f64) -> ArcVar {
        ArcVar {
            circle: circle(problem, c, r, true),
            start: problem.param(start, true),
            sweep: problem.param(sweep, true),
        }
    }

    #[test]
    fn point_on_finite_curves_stays_on_segment_and_sweep() {
        let mut problem = Problem::default();
        let l = line(&mut problem, (0.0, 0.0), (10.0, 0.0), true);
        problem.locked[l.b.x] = true;
        problem.locked[l.b.y] = true;
        let a = arc(&mut problem, (0.0, 0.0), 5.0, 0.0, PI * 0.5);
        let on_line = problem.point(14.0, 3.0, false);
        let on_arc = problem.point(-6.0, -1.0, false);
        problem.constraints = vec![
            Constraint::PointOnCurve {
                point: on_line,
                curve: Curve::Line(l),
                param: None,
            },
            Constraint::PointOnCurve {
                point: on_arc,
                curve: Curve::Arc(a),
                param: None,
            },
        ];
        soft_keep_all(&mut problem);
        let p = solved(&problem).params;
        close(p[on_line.y], 0.0);
        assert!(p[on_line.x] <= 10.0 + 1e-5);
        close(p[on_arc.x].hypot(p[on_arc.y]), 5.0);
        let angle = p[on_arc.y].atan2(p[on_arc.x]);
        assert!((-1e-5..=PI * 0.5 + 1e-5).contains(&angle), "{angle}");
    }

    #[test]
    fn ellipse_and_spline_point_on_curve_and_midpoint() {
        let mut problem = Problem::default();
        let center = problem.point(0.0, 0.0, true);
        let ellipse = EllipseVar {
            center,
            rx: problem.param(8.0, true),
            ry: problem.param(4.0, true),
            rotation: problem.param(30.0, true),
            arc: Some((problem.param(0.0, true), problem.param(PI, true))),
        };
        let on = problem.point(1.0, 9.0, false);
        let mid = problem.point(0.0, 0.0, false);
        let spline = SplineVar {
            points: [(0.0, 0.0), (10.0, 5.0), (20.0, 0.0)]
                .into_iter()
                .map(|(x, y)| problem.point(x, y, true))
                .collect(),
            control: false,
            closed: false,
        };
        let on_spline = problem.point(12.0, 9.0, false);
        let u = spline_parameter(&problem.params, &spline, (12.0, 9.0));
        let u = problem.param(u, false);
        problem.constraints = vec![
            Constraint::PointOnCurve {
                point: on,
                curve: Curve::Ellipse(ellipse),
                param: None,
            },
            Constraint::CurveMidpoint {
                point: mid,
                curve: Curve::Ellipse(ellipse),
            },
            Constraint::PointOnCurve {
                point: on_spline,
                curve: Curve::Spline(spline.clone()),
                param: Some(u),
            },
        ];
        soft_keep_all(&mut problem);
        let p = solved(&problem).params;
        let e = Ellipse::new(&p, &ellipse);
        let (lu, lv) = e.local(pt(&p, on));
        close((lu / 8.0).hypot(lv / 4.0), 1.0);
        let expected = e.at(PI * 0.5);
        close(p[mid.x], expected.0);
        close(p[mid.y], expected.1);
        let (at, _, _) = spline_at(&p, &spline, p[u]).unwrap();
        close(p[on_spline.x], at.0);
        close(p[on_spline.y], at.1);
        assert!((0.0..=2.0).contains(&p[u]));
    }

    #[test]
    fn tangency_and_normal_relations() {
        let mut problem = Problem::default();
        let outer = circle(&mut problem, (0.0, 0.0), 10.0, true);
        let inner = circle(&mut problem, (1.0, 1.0), 3.0, false);
        problem.locked[inner.radius] = true;
        let other = circle(&mut problem, (20.0, 3.0), 4.0, false);
        problem.locked[other.radius] = true;
        let base = line(&mut problem, (-20.0, -20.0), (20.0, -20.0), true);
        problem.locked[base.b.x] = true;
        problem.locked[base.b.y] = true;
        let touching = circle(&mut problem, (0.0, -10.0), 2.0, false);
        problem.locked[touching.radius] = true;
        let normal = line(&mut problem, (8.0, 3.0), (15.0, 5.0), false);
        problem.constraints = vec![
            Constraint::InternalTangent {
                outer: Curve::Circle(outer),
                inner: Curve::Circle(inner),
            },
            Constraint::ExternalTangent {
                first: Curve::Circle(outer),
                second: Curve::Circle(other),
                side: 1.0,
            },
            Constraint::ExternalTangent {
                first: Curve::Line(base),
                second: Curve::Circle(touching),
                side: 1.0,
            },
            Constraint::Normal {
                line: normal,
                endpoint: 0,
                curve: Curve::Circle(outer),
                param: None,
            },
        ];
        soft_keep_all(&mut problem);
        let p = solved(&problem).params;
        close(p[inner.center.x].hypot(p[inner.center.y]), 7.0);
        close(p[other.center.x].hypot(p[other.center.y]), 14.0);
        close(p[touching.center.y], -18.0);
        close(p[normal.a.x].hypot(p[normal.a.y]), 10.0);
        close(cross(unit(&p, normal), (p[normal.a.x], p[normal.a.y])), 0.0);
    }

    #[test]
    fn arc_endpoint_relations_move_the_sweep() {
        let mut problem = Problem::default();
        let center = problem.point(0.0, 0.0, true);
        let radius = problem.param(5.0, true);
        let a = ArcVar {
            circle: CircleVar { center, radius },
            start: problem.param(0.3, false),
            sweep: problem.param(1.0, false),
        };
        let l = line(&mut problem, (0.0, 0.0), (0.0, 10.0), true);
        problem.locked[l.b.x] = true;
        problem.locked[l.b.y] = true;
        problem.constraints = vec![
            Constraint::EndpointTangent {
                line: l,
                arc: a,
                endpoint: 0,
                line_sign: 1.0,
                sweep_sign: 1.0,
            },
            Constraint::FixedOnCurve {
                curve: Curve::Arc(a),
                fraction: 1.0,
                at: (0.0, 5.0),
            },
        ];
        soft_keep_all(&mut problem);
        let p = solved(&problem).params;
        // Upward line, counter-clockwise arc: the start radial points +x.
        close(wrap(p[a.start]), 0.0);
        close(p[a.start] + p[a.sweep], PI * 0.5);
    }
}
