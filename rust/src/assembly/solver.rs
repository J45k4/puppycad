//! Rigid-body placement of assembly instances from their mates.
//!
//! Two stages:
//!
//! 1. A port of `solveFixedAssembly` (`src/assembly-solver.ts`): fasten
//!    mates form a graph; poses propagate breadth-first from the world, then
//!    from root instances (never the `b` side of a fasten mate), so fastened
//!    instances land exactly where the web app puts them. A loop whose
//!    poses disagree is a conflict.
//! 2. The web app stores revolute, prismatic, planar and ball mates without
//!    solving them; natively each fasten-connected group of instances is a
//!    rigid body with 6 DOF and a damped Gauss–Newton (Levenberg–Marquardt,
//!    numeric Jacobian, like `crate::solver`) moves the free bodies until
//!    every other mate holds. Small steps keep the free DOF (a revolute's
//!    angle, a slider's offset) near the current placement. The group of the
//!    first instance and every group fastened to the world are grounded. A
//!    servo actuator on a revolute mate fixes its angle (`homeDeg`, measured
//!    about the `a` frame's Z from its X axis to the `b` frame's X axis).

use std::collections::{HashMap, HashSet};

use super::*;

/// Largest residual (mm or unit-vector difference) that counts as solved.
pub const TOLERANCE: f64 = 1e-6;

#[derive(Clone, Debug)]
pub struct Solution {
    /// World pose of every instance, by index.
    pub poses: Vec<Pose>,
    /// Instances that some mate positions.
    pub mated: Vec<bool>,
    /// Largest remaining mate residual.
    pub residual: f64,
}

struct Link {
    to: Option<String>,
    delta: Pose,
    mate_id: String,
}

fn connector_frame(asm: &Assembly, r: &MateRef) -> Result<Pose, String> {
    asm.connector(r)
        .map(Connector::frame)
        .ok_or_else(|| format!("Unknown connector: {}", r.connector_id))
}

/// Stage 1: the web fasten solver. Returns poses for instances in the
/// fasten graph.
fn solve_fixed(asm: &Assembly) -> Result<HashMap<Option<String>, Pose>, String> {
    let fixed: Vec<&Mate> = asm
        .mates
        .iter()
        .filter(|m| m.kind == MateKind::Fasten)
        .collect();
    let mut graph: HashMap<Option<String>, Vec<Link>> = HashMap::new();
    let mut order: Vec<Option<String>> = Vec::new();
    for mate in &fixed {
        if mate.a.instance_id == mate.b.instance_id {
            return Err(format!("Mate {} must connect distinct instances.", mate.id));
        }
        let delta = connector_frame(asm, &mate.a)?.mul(&connector_frame(asm, &mate.b)?.inverse());
        for (from, to, delta) in [
            (&mate.a.instance_id, &mate.b.instance_id, delta),
            (&mate.b.instance_id, &mate.a.instance_id, delta.inverse()),
        ] {
            if !graph.contains_key(from) {
                order.push(from.clone());
            }
            graph.entry(from.clone()).or_default().push(Link {
                to: to.clone(),
                delta,
                mate_id: mate.id.clone(),
            });
        }
    }
    for id in order.iter().flatten() {
        if asm.instance(id).is_some_and(|i| !i.transform().unscaled()) {
            return Err("Fixed connections require unscaled instances.".into());
        }
    }
    let mut poses: HashMap<Option<String>, Pose> = HashMap::new();
    let solve_from =
        |root: Option<String>, poses: &mut HashMap<Option<String>, Pose>| -> Result<(), String> {
            if poses.contains_key(&root) {
                return Ok(());
            }
            let start = match &root {
                None => Pose::IDENTITY,
                Some(id) => asm
                    .instance(id)
                    .map(|i| i.transform().pose())
                    .unwrap_or_default(),
            };
            poses.insert(root.clone(), start);
            let mut queue = vec![root];
            let mut index = 0;
            while index < queue.len() {
                let from = queue[index].clone();
                index += 1;
                for link in graph.get(&from).into_iter().flatten() {
                    let expected = poses[&from].mul(&link.delta);
                    if let Some(existing) = poses.get(&link.to) {
                        if existing.max_difference(&expected) > TOLERANCE {
                            return Err(format!(
                                "Conflicting fixed constraints at mate \"{}\".",
                                link.mate_id
                            ));
                        }
                    } else {
                        poses.insert(link.to.clone(), expected);
                        queue.push(link.to.clone());
                    }
                }
            }
            Ok(())
        };
    if graph.contains_key(&None) {
        solve_from(None, &mut poses)?;
    }
    let children: HashSet<&Option<String>> = fixed.iter().map(|m| &m.b.instance_id).collect();
    for instance in &asm.instances {
        let key = Some(instance.id.clone());
        if !children.contains(&key) && graph.contains_key(&key) {
            solve_from(key, &mut poses)?;
        }
    }
    for instance in &asm.instances {
        let key = Some(instance.id.clone());
        if graph.contains_key(&key) {
            solve_from(key, &mut poses)?;
        }
    }
    Ok(poses)
}

/// Residuals of one non-fasten (or loop-closing) mate between world frames.
fn mate_residuals(kind: MateKind, fa: &Pose, fb: &Pose, servo: Option<f64>, out: &mut Vec<f64>) {
    let (oa, ob) = (fa.t, fb.t);
    let (xa, ya, za) = (fa.axis(0), fa.axis(1), fa.axis(2));
    let (xb, zb) = (fb.axis(0), fb.axis(2));
    let d = v_sub(ob, oa);
    match kind {
        MateKind::Fasten => {
            out.extend(d);
            out.extend(v_sub(za, zb));
            out.extend(v_sub(xa, xb));
        }
        MateKind::Revolute => {
            out.extend(d);
            out.extend(v_sub(za, zb));
            if let Some(degrees) = servo {
                let (s, c) = degrees.to_radians().sin_cos();
                out.extend(v_sub(xb, v_add(v_scale(xa, c), v_scale(ya, s))));
            }
        }
        MateKind::Prismatic => {
            out.extend(v_sub(za, zb));
            out.extend(v_sub(xa, xb));
            out.push(v_dot(d, xa));
            out.push(v_dot(d, ya));
        }
        MateKind::Planar => {
            out.extend(v_sub(za, zb));
            out.push(v_dot(d, za));
        }
        MateKind::Ball => out.extend(d),
    }
}

/// Signed revolute angle (degrees) of `fb` relative to `fa` about `fa`'s Z.
pub fn revolute_angle(fa: &Pose, fb: &Pose) -> f64 {
    let xb = fb.axis(0);
    v_dot(xb, fa.axis(1))
        .atan2(v_dot(xb, fa.axis(0)))
        .to_degrees()
}

/// Solves every mate; instances no mate touches keep their transforms.
pub fn solve(asm: &Assembly) -> Result<Solution, String> {
    let fixed = solve_fixed(asm)?;
    let n = asm.instances.len();
    let mut poses: Vec<Pose> = asm
        .instances
        .iter()
        .map(|i| {
            fixed
                .get(&Some(i.id.clone()))
                .copied()
                .unwrap_or_else(|| i.transform().pose())
        })
        .collect();
    let mut mated = vec![false; n];
    for mate in &asm.mates {
        for r in [&mate.a, &mate.b] {
            if let Some(index) = r
                .instance_id
                .as_deref()
                .and_then(|id| asm.instance_index(id))
            {
                mated[index] = true;
            }
        }
    }
    // Rigid groups: union-find over fasten mates between instances.
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        let mut root = i;
        while parent[root] != root {
            root = parent[root];
        }
        parent[i] = root;
        root
    }
    let index_of = |r: &MateRef| {
        r.instance_id
            .as_deref()
            .and_then(|id| asm.instance_index(id))
    };
    let mut grounded = HashSet::new();
    for mate in asm.mates.iter().filter(|m| m.kind == MateKind::Fasten) {
        match (index_of(&mate.a), index_of(&mate.b)) {
            (Some(a), Some(b)) => {
                let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
                parent[ra] = rb;
            }
            (Some(i), None) | (None, Some(i)) => {
                grounded.insert(i);
            }
            _ => {}
        }
    }
    let group: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();
    let mut grounded: HashSet<usize> = grounded.into_iter().map(|i| group[i]).collect();
    if n > 0 {
        grounded.insert(group[0]);
    }
    let soft: Vec<&Mate> = asm
        .mates
        .iter()
        .filter(|m| m.kind != MateKind::Fasten)
        .collect();
    let servos: HashMap<String, f64> = asm.servo_angles().into_iter().collect();
    let frames: Vec<(Pose, Pose)> = soft
        .iter()
        .map(|m| Ok((connector_frame(asm, &m.a)?, connector_frame(asm, &m.b)?)))
        .collect::<Result<_, String>>()?;
    let mut free: Vec<usize> = Vec::new();
    for mate in &soft {
        for i in [index_of(&mate.a), index_of(&mate.b)].into_iter().flatten() {
            let g = group[i];
            if !grounded.contains(&g) && !free.contains(&g) {
                free.push(g);
            }
        }
    }
    for (i, instance) in asm.instances.iter().enumerate() {
        if free.contains(&group[i]) && !instance.transform().unscaled() {
            return Err("Mates require unscaled instances.".into());
        }
    }
    let residuals = |poses: &[Pose]| {
        let mut out = Vec::new();
        for (mate, (ca, cb)) in soft.iter().zip(&frames) {
            let world = |r: &MateRef, c: &Pose| match index_of(r) {
                Some(i) => poses[i].mul(c),
                None => *c,
            };
            mate_residuals(
                mate.kind,
                &world(&mate.a, ca),
                &world(&mate.b, cb),
                (mate.kind == MateKind::Revolute)
                    .then(|| servos.get(&mate.id).copied())
                    .flatten(),
                &mut out,
            );
        }
        out
    };
    // Per-group pivot (mean instance origin) for better-conditioned rotations.
    let pivots: Vec<V3> = free
        .iter()
        .map(|&g| {
            let members: Vec<usize> = (0..n).filter(|&i| group[i] == g).collect();
            let sum = members
                .iter()
                .fold([0.0; 3], |acc, &i| v_add(acc, poses[i].t));
            v_scale(sum, 1.0 / members.len().max(1) as f64)
        })
        .collect();
    let step = |poses: &[Pose], delta: &[f64]| -> Vec<Pose> {
        let mut next = poses.to_vec();
        for (k, &g) in free.iter().enumerate() {
            let w = [delta[6 * k], delta[6 * k + 1], delta[6 * k + 2]];
            let v = [delta[6 * k + 3], delta[6 * k + 4], delta[6 * k + 5]];
            let r = axis_angle_matrix(w);
            let c = pivots[k];
            let motion = Pose {
                r,
                t: v_add(v_sub(c, m_vec(r, c)), v),
            };
            for i in 0..n {
                if group[i] == g {
                    next[i] = motion.mul(&poses[i]);
                }
            }
        }
        next
    };
    let norm = |r: &[f64]| r.iter().map(|x| x * x).sum::<f64>();
    let params = free.len() * 6;
    let mut current = residuals(&poses);
    let mut lambda = 1e-6;
    for _ in 0..200 {
        if params == 0 || current.iter().all(|x| x.abs() <= TOLERANCE * 1e-3) {
            break;
        }
        let h = 1e-7;
        let mut jacobian = vec![vec![0.0; params]; current.len()];
        for p in 0..params {
            let mut delta = vec![0.0; params];
            delta[p] = h;
            let shifted = residuals(&step(&poses, &delta));
            for (row, value) in shifted.iter().enumerate() {
                jacobian[row][p] = (value - current[row]) / h;
            }
        }
        let mut jtj = vec![vec![0.0; params]; params];
        let mut jtr = vec![0.0; params];
        for (row, r) in jacobian.iter().zip(&current) {
            for i in 0..params {
                jtr[i] += row[i] * r;
                for j in 0..params {
                    jtj[i][j] += row[i] * row[j];
                }
            }
        }
        let mut improved = false;
        for _ in 0..12 {
            let mut a = jtj.clone();
            for (i, row) in a.iter_mut().enumerate() {
                row[i] += lambda;
            }
            let Some(delta) = solve_linear(a, jtr.iter().map(|x| -x).collect()) else {
                lambda *= 10.0;
                continue;
            };
            let candidate = step(&poses, &delta);
            let next = residuals(&candidate);
            if norm(&next) < norm(&current) {
                poses = candidate;
                current = next;
                lambda = (lambda * 0.3).max(1e-12);
                improved = true;
                break;
            }
            lambda *= 10.0;
        }
        if !improved {
            break;
        }
    }
    let residual = current.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    Ok(Solution {
        poses,
        mated,
        residual,
    })
}

/// Solves and writes the transforms of mated instances back into `asm`.
pub fn apply(asm: &mut Assembly) -> Result<f64, String> {
    let solution = solve(asm)?;
    if solution.residual > 1e-4 {
        return Err(format!(
            "mates conflict (residual {:.3})",
            solution.residual
        ));
    }
    for (index, instance) in asm.instances.iter_mut().enumerate() {
        if solution.mated[index] {
            instance.transform = Some(instance.transform().with_pose(&solution.poses[index]));
        }
    }
    Ok(solution.residual)
}

/// Gaussian elimination with partial pivoting.
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-300 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            if factor == 0.0 {
                continue;
            }
            let pivot_row = a[col].clone();
            for (target, source) in a[row].iter_mut().zip(pivot_row).skip(col) {
                *target -= factor * source;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}
