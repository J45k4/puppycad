//! Assembly documents: part instances placed by rigid transforms, connector
//! frames on instances (or on the world), mates between connector pairs and
//! servo actuators. The JSON shape is the web `Assembly` type
//! (`src/contract.ts`), stored as the `data` of a top-level `.pcad` item with
//! `"type": "assembly"`, so files stay readable by both apps.
//!
//! Native extension: an instance may carry an optional `name`; the web
//! loader ignores it.
//!
//! Rotations are Euler angles in degrees applied in three.js `XYZ` order,
//! matching `transformMatrix` in `src/assembly-solver.ts`.

use std::collections::HashSet;

use serde_json::{Map, Value, json};

pub mod solver;

pub type V3 = [f64; 3];
pub type M3 = [[f64; 3]; 3];

pub fn v_add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn v_sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn v_scale(a: V3, s: f64) -> V3 {
    a.map(|c| c * s)
}
pub fn v_dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub fn v_cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub fn v_len(a: V3) -> f64 {
    v_dot(a, a).sqrt()
}
pub fn v_unit(a: V3) -> Option<V3> {
    let l = v_len(a);
    (l > 1e-12).then(|| v_scale(a, 1.0 / l))
}

fn m_mul(a: M3, b: M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()))
}
fn m_vec(a: M3, v: V3) -> V3 {
    std::array::from_fn(|i| (0..3).map(|k| a[i][k] * v[k]).sum())
}
fn m_transpose(a: M3) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| a[j][i]))
}
const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Rotation matrix of three.js `Euler(x, y, z, "XYZ")` (radians).
pub fn euler_matrix(r: V3) -> M3 {
    let (b, a) = r[0].sin_cos();
    let (d, c) = r[1].sin_cos();
    let (f, e) = r[2].sin_cos();
    let (ae, af, be, bf) = (a * e, a * f, b * e, b * f);
    [
        [c * e, -c * f, d],
        [af + be * d, ae - bf * d, -b * c],
        [bf - ae * d, be + af * d, a * c],
    ]
}

/// three.js `Euler.setFromRotationMatrix` with order `XYZ` (radians).
pub fn matrix_euler(m: M3) -> V3 {
    let y = m[0][2].clamp(-1.0, 1.0).asin();
    if m[0][2].abs() < 0.9999999 {
        [(-m[1][2]).atan2(m[2][2]), y, (-m[0][1]).atan2(m[0][0])]
    } else {
        [m[2][1].atan2(m[1][1]), y, 0.0]
    }
}

/// Rotation about a unit-free axis vector `w` by `|w|` radians (Rodrigues).
pub fn axis_angle_matrix(w: V3) -> M3 {
    let angle = v_len(w);
    if angle < 1e-15 {
        return IDENTITY;
    }
    let k = v_scale(w, 1.0 / angle);
    let (s, c) = angle.sin_cos();
    let t = 1.0 - c;
    [
        [
            c + k[0] * k[0] * t,
            k[0] * k[1] * t - k[2] * s,
            k[0] * k[2] * t + k[1] * s,
        ],
        [
            k[1] * k[0] * t + k[2] * s,
            c + k[1] * k[1] * t,
            k[1] * k[2] * t - k[0] * s,
        ],
        [
            k[2] * k[0] * t - k[1] * s,
            k[2] * k[1] * t + k[0] * s,
            c + k[2] * k[2] * t,
        ],
    ]
}

/// A rigid transform: `world = r * local + t`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub r: M3,
    pub t: V3,
}

impl Default for Pose {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Pose {
    pub const IDENTITY: Self = Self {
        r: IDENTITY,
        t: [0.0; 3],
    };

    /// Translation plus Euler XYZ rotation in degrees.
    pub fn from_parts(translation: V3, rotation_deg: V3) -> Self {
        Self {
            r: euler_matrix(rotation_deg.map(f64::to_radians)),
            t: translation,
        }
    }

    pub fn rotation_deg(&self) -> V3 {
        matrix_euler(self.r).map(f64::to_degrees)
    }

    /// `self ∘ other`: applies `other` first.
    pub fn mul(&self, other: &Pose) -> Pose {
        Pose {
            r: m_mul(self.r, other.r),
            t: v_add(m_vec(self.r, other.t), self.t),
        }
    }

    pub fn inverse(&self) -> Pose {
        let rt = m_transpose(self.r);
        Pose {
            r: rt,
            t: v_scale(m_vec(rt, self.t), -1.0),
        }
    }

    pub fn point(&self, p: V3) -> V3 {
        v_add(m_vec(self.r, p), self.t)
    }

    pub fn vector(&self, v: V3) -> V3 {
        m_vec(self.r, v)
    }

    /// Column `i` of the rotation: the frame's local axis in world space.
    pub fn axis(&self, i: usize) -> V3 {
        [self.r[0][i], self.r[1][i], self.r[2][i]]
    }

    pub fn max_difference(&self, other: &Pose) -> f64 {
        let mut max: f64 = 0.0;
        for i in 0..3 {
            max = max.max((self.t[i] - other.t[i]).abs());
            for j in 0..3 {
                max = max.max((self.r[i][j] - other.r[i][j]).abs());
            }
        }
        max
    }

    /// Frame with origin `origin` whose Z axis is `normal` (X picked stably).
    pub fn from_normal(origin: V3, normal: V3) -> Self {
        let z = v_unit(normal).unwrap_or([0.0, 0.0, 1.0]);
        let helper = if z[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let x = v_unit(v_sub(helper, v_scale(z, v_dot(helper, z)))).unwrap_or([1.0, 0.0, 0.0]);
        let y = v_cross(z, x);
        Self {
            r: [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]],
            t: origin,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Transform {
    pub translation: V3,
    /// Euler XYZ, degrees.
    pub rotation: V3,
    pub scale: V3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0; 3],
            rotation: [0.0; 3],
            scale: [1.0; 3],
        }
    }
}

impl Transform {
    pub fn pose(&self) -> Pose {
        Pose::from_parts(self.translation, self.rotation)
    }

    pub fn with_pose(&self, pose: &Pose) -> Self {
        Self {
            translation: pose.t,
            rotation: pose.rotation_deg(),
            scale: self.scale,
        }
    }

    pub fn unscaled(&self) -> bool {
        self.scale.iter().all(|s| (s - 1.0).abs() <= 1e-6)
    }

    /// Local point to world, including scale.
    pub fn apply(&self, p: V3) -> V3 {
        self.pose().point([
            p[0] * self.scale[0],
            p[1] * self.scale[1],
            p[2] * self.scale[2],
        ])
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub id: String,
    pub part_id: String,
    /// Native extension; the web app shows instances by id.
    pub name: Option<String>,
    pub transform: Option<Transform>,
}

impl Instance {
    pub fn transform(&self) -> Transform {
        self.transform.clone().unwrap_or_default()
    }

    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Connector {
    pub id: String,
    pub name: Option<String>,
    /// `None`: a world (ground) connector.
    pub instance_id: Option<String>,
    pub position: V3,
    /// Euler XYZ, degrees.
    pub rotation: Option<V3>,
}

impl Connector {
    /// The connector frame in its instance's local coordinates (or world).
    pub fn frame(&self) -> Pose {
        Pose::from_parts(self.position, self.rotation.unwrap_or([0.0; 3]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MateKind {
    /// Frames coincide.
    Fasten,
    /// Shared origin and Z axis; free angle about Z.
    Revolute,
    /// Same orientation, origins on a shared Z axis; free offset along Z.
    Prismatic,
    /// Z axes parallel and both origins on the same plane; free in-plane
    /// translation and rotation about Z.
    Planar,
    /// Shared origin; free rotation.
    Ball,
}

impl MateKind {
    pub const ALL: [MateKind; 5] = [
        MateKind::Fasten,
        MateKind::Revolute,
        MateKind::Prismatic,
        MateKind::Planar,
        MateKind::Ball,
    ];

    pub fn name(self) -> &'static str {
        match self {
            MateKind::Fasten => "fasten",
            MateKind::Revolute => "revolute",
            MateKind::Prismatic => "prismatic",
            MateKind::Planar => "planar",
            MateKind::Ball => "ball",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MateKind::Fasten => "FASTENED",
            MateKind::Revolute => "REVOLUTE",
            MateKind::Prismatic => "SLIDER",
            MateKind::Planar => "PLANAR",
            MateKind::Ball => "BALL",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MateRef {
    pub instance_id: Option<String>,
    pub connector_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mate {
    pub id: String,
    pub name: Option<String>,
    pub kind: MateKind,
    pub a: MateRef,
    pub b: MateRef,
    /// Web `params` (variables), kept verbatim.
    pub params: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Assembly {
    pub id: String,
    pub name: String,
    pub instances: Vec<Instance>,
    pub connectors: Vec<Connector>,
    pub mates: Vec<Mate>,
    /// Web actuators, kept verbatim; servos drive their mate's free angle.
    pub actuators: Vec<Value>,
    pub variables: Option<Value>,
    /// Set when the document uses something the native app cannot edit
    /// (an unknown mate type, a dangling reference); saving leaves it untouched.
    pub read_only_reason: Option<String>,
}

fn vec3(value: &Value) -> Option<V3> {
    let get = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_f64)
            .filter(|v| v.is_finite())
    };
    Some([get("x")?, get("y")?, get("z")?])
}

fn vec3_json(v: V3) -> Value {
    json!({"x": v[0], "y": v[1], "z": v[2]})
}

fn non_empty(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn parse_transform(value: &Value) -> Option<Transform> {
    let translation = value.get("translation").and_then(vec3);
    let rotation = value.get("rotation").and_then(vec3);
    let scale = value.get("scale").and_then(vec3);
    if translation.is_none() && rotation.is_none() && scale.is_none() {
        return None;
    }
    Some(Transform {
        translation: translation.unwrap_or([0.0; 3]),
        rotation: rotation.unwrap_or([0.0; 3]),
        scale: scale.unwrap_or([1.0; 3]),
    })
}

fn parse_ref(value: Option<&Value>) -> Option<MateRef> {
    let value = value?;
    let instance_id = match value.get("instanceId") {
        Some(Value::Null) => None,
        other => Some(non_empty(other)?),
    };
    Some(MateRef {
        instance_id,
        connector_id: non_empty(value.get("connectorId"))?,
    })
}

impl Assembly {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            instances: Vec::new(),
            connectors: Vec::new(),
            mates: Vec::new(),
            actuators: Vec::new(),
            variables: None,
            read_only_reason: None,
        }
    }

    /// Parses the `data` of an assembly item. A missing `data` is an empty
    /// assembly. Anything the native app cannot represent sets
    /// `read_only_reason` instead of failing.
    pub fn from_json(data: Option<&Value>, default_id: &str, default_name: &str) -> Self {
        let mut assembly = Self::new(default_id, default_name);
        let Some(data) = data.filter(|data| data.is_object()) else {
            return assembly;
        };
        let mut problems = Vec::new();
        if let Some(id) = non_empty(data.get("id")) {
            assembly.id = id;
        }
        if let Some(name) = non_empty(data.get("name")) {
            assembly.name = name;
        }
        let mut ids = HashSet::new();
        for raw in data
            .get("instances")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(id), Some(part_id)) =
                (non_empty(raw.get("id")), non_empty(raw.get("partId")))
            else {
                problems.push("an instance has no id or part".to_string());
                continue;
            };
            if !ids.insert(id.clone()) {
                problems.push(format!("duplicate instance \"{id}\""));
                continue;
            }
            let transform = raw.get("transform").and_then(parse_transform);
            if transform
                .as_ref()
                .is_some_and(|t| t.scale.iter().any(|s| *s <= 0.0))
            {
                problems.push(format!("instance \"{id}\" has a non-positive scale"));
            }
            assembly.instances.push(Instance {
                id,
                part_id,
                name: non_empty(raw.get("name")),
                transform,
            });
        }
        let mut connector_ids = HashSet::new();
        for raw in data
            .get("connectors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = non_empty(raw.get("id"));
            let instance_id = match raw.get("instanceId") {
                Some(Value::Null) => Some(None),
                other => non_empty(other).map(Some),
            };
            let position = raw.get("position").and_then(vec3);
            match (id, instance_id, position) {
                (Some(id), Some(instance_id), Some(position))
                    if connector_ids.insert(id.clone())
                        && instance_id.as_ref().is_none_or(|i| ids.contains(i)) =>
                {
                    assembly.connectors.push(Connector {
                        id,
                        name: non_empty(raw.get("name")),
                        instance_id,
                        position,
                        rotation: raw.get("rotation").and_then(vec3),
                    })
                }
                _ => problems.push("a connector is invalid".into()),
            }
        }
        let mut mate_ids = HashSet::new();
        for (index, raw) in data
            .get("mates")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            let id = non_empty(raw.get("id")).unwrap_or_else(|| format!("mate-{}", index + 1));
            let type_name = raw.get("type").and_then(Value::as_str).unwrap_or("");
            let Some(kind) = MateKind::parse(type_name) else {
                problems.push(format!(
                    "mate \"{id}\" has unsupported type \"{type_name}\""
                ));
                continue;
            };
            let (Some(a), Some(b)) = (parse_ref(raw.get("a")), parse_ref(raw.get("b"))) else {
                problems.push(format!("mate \"{id}\" has an invalid reference"));
                continue;
            };
            let valid = |r: &MateRef| {
                assembly
                    .connectors
                    .iter()
                    .any(|c| c.id == r.connector_id && c.instance_id == r.instance_id)
            };
            if !valid(&a) || !valid(&b) {
                problems.push(format!("mate \"{id}\" references a missing connector"));
                continue;
            }
            if !mate_ids.insert(id.clone()) {
                problems.push(format!("duplicate mate \"{id}\""));
                continue;
            }
            assembly.mates.push(Mate {
                id,
                name: non_empty(raw.get("name")),
                kind,
                a,
                b,
                params: raw.get("params").filter(|p| p.is_object()).cloned(),
            });
        }
        if let Some(actuators) = data.get("actuators").and_then(Value::as_array) {
            assembly.actuators = actuators.clone();
        }
        assembly.variables = data.get("variables").filter(|v| v.is_object()).cloned();
        if !problems.is_empty() {
            assembly.read_only_reason = Some(problems.join("; "));
        }
        assembly
    }

    /// The web `Assembly` JSON.
    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        out.insert("id".into(), json!(self.id));
        out.insert("name".into(), json!(self.name));
        out.insert(
            "instances".into(),
            Value::Array(
                self.instances
                    .iter()
                    .map(|instance| {
                        let mut item = Map::new();
                        item.insert("id".into(), json!(instance.id));
                        item.insert("partId".into(), json!(instance.part_id));
                        if let Some(name) = &instance.name {
                            item.insert("name".into(), json!(name));
                        }
                        if let Some(t) = &instance.transform {
                            item.insert(
                                "transform".into(),
                                json!({
                                    "translation": vec3_json(t.translation),
                                    "rotation": vec3_json(t.rotation),
                                    "scale": vec3_json(t.scale),
                                }),
                            );
                        }
                        Value::Object(item)
                    })
                    .collect(),
            ),
        );
        if !self.connectors.is_empty() {
            out.insert(
                "connectors".into(),
                Value::Array(
                    self.connectors
                        .iter()
                        .map(|c| {
                            let mut item = Map::new();
                            item.insert("id".into(), json!(c.id));
                            if let Some(name) = &c.name {
                                item.insert("name".into(), json!(name));
                            }
                            item.insert("instanceId".into(), json!(c.instance_id));
                            item.insert("position".into(), vec3_json(c.position));
                            if let Some(rotation) = c.rotation {
                                item.insert("rotation".into(), vec3_json(rotation));
                            }
                            Value::Object(item)
                        })
                        .collect(),
                ),
            );
        }
        if !self.mates.is_empty() {
            let reference =
                |r: &MateRef| json!({"instanceId": r.instance_id, "connectorId": r.connector_id});
            out.insert(
                "mates".into(),
                Value::Array(
                    self.mates
                        .iter()
                        .map(|m| {
                            let mut item = Map::new();
                            item.insert("id".into(), json!(m.id));
                            if let Some(name) = &m.name {
                                item.insert("name".into(), json!(name));
                            }
                            item.insert("type".into(), json!(m.kind.name()));
                            item.insert("a".into(), reference(&m.a));
                            item.insert("b".into(), reference(&m.b));
                            if let Some(params) = &m.params {
                                item.insert("params".into(), params.clone());
                            }
                            Value::Object(item)
                        })
                        .collect(),
                ),
            );
        }
        if !self.actuators.is_empty() {
            out.insert("actuators".into(), Value::Array(self.actuators.clone()));
        }
        if let Some(variables) = &self.variables {
            out.insert("variables".into(), variables.clone());
        }
        Value::Object(out)
    }

    pub fn instance(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub fn instance_index(&self, id: &str) -> Option<usize> {
        self.instances.iter().position(|i| i.id == id)
    }

    pub fn connector(&self, r: &MateRef) -> Option<&Connector> {
        self.connectors
            .iter()
            .find(|c| c.id == r.connector_id && c.instance_id == r.instance_id)
    }

    /// Servo home angles (degrees) keyed by the mate they drive.
    pub fn servo_angles(&self) -> Vec<(String, f64)> {
        self.actuators
            .iter()
            .filter(|a| a.get("type").and_then(Value::as_str) == Some("servo"))
            .filter_map(|a| {
                Some((
                    a.get("mateId")?.as_str()?.to_string(),
                    a.get("homeDeg")?.as_f64()?,
                ))
            })
            .collect()
    }

    /// Sets the home angle of the servo driving `mate_id`, adding one when
    /// missing (range ±180°).
    pub fn set_servo_angle(&mut self, mate_id: &str, degrees: f64) {
        if let Some(servo) = self.actuators.iter_mut().find(|a| {
            a.get("type").and_then(Value::as_str) == Some("servo")
                && a.get("mateId").and_then(Value::as_str) == Some(mate_id)
        }) {
            servo["homeDeg"] = json!(degrees);
            let min = servo
                .pointer("/commandRange/minDeg")
                .and_then(Value::as_f64)
                .unwrap_or(-180.0);
            let max = servo
                .pointer("/commandRange/maxDeg")
                .and_then(Value::as_f64)
                .unwrap_or(180.0);
            servo["commandRange"] = json!({"minDeg": min.min(degrees), "maxDeg": max.max(degrees)});
            return;
        }
        let id = self.unique_id("servo", |a, id| {
            a.actuators
                .iter()
                .any(|s| s.get("id").and_then(Value::as_str) == Some(id))
        });
        self.actuators.push(json!({
            "id": id, "type": "servo", "mateId": mate_id, "homeDeg": degrees,
            "commandRange": {"minDeg": (-180.0f64).min(degrees), "maxDeg": 180.0f64.max(degrees)}
        }));
    }

    pub fn unique_id(&self, prefix: &str, taken: impl Fn(&Self, &str) -> bool) -> String {
        (1..)
            .map(|n| format!("{prefix}-{n}"))
            .find(|id| !taken(self, id))
            .expect("unbounded")
    }

    /// Adds an instance of `part_id` at `transform`; returns its index.
    pub fn insert_instance(&mut self, part_id: &str, name: &str, transform: Transform) -> usize {
        let id = self.unique_id("instance", |a, id| a.instance(id).is_some());
        self.instances.push(Instance {
            id,
            part_id: part_id.into(),
            name: Some(name.into()),
            transform: Some(transform),
        });
        self.instances.len() - 1
    }

    /// Removes an instance with its connectors, the mates on them and the
    /// actuators of those mates.
    pub fn remove_instance(&mut self, index: usize) -> Option<Instance> {
        if index >= self.instances.len() {
            return None;
        }
        let removed = self.instances.remove(index);
        let id = Some(removed.id.clone());
        self.connectors.retain(|c| c.instance_id != id);
        let gone: Vec<String> = self
            .mates
            .iter()
            .filter(|m| m.a.instance_id == id || m.b.instance_id == id)
            .map(|m| m.id.clone())
            .collect();
        self.mates
            .retain(|m| m.a.instance_id != id && m.b.instance_id != id);
        self.actuators.retain(|a| {
            !a.get("mateId")
                .and_then(Value::as_str)
                .is_some_and(|m| gone.iter().any(|g| g == m))
        });
        Some(removed)
    }

    /// Removes a mate, the actuators driving it and the connectors it used
    /// that no other mate references.
    pub fn remove_mate(&mut self, mate_id: &str) -> Option<Mate> {
        let index = self.mates.iter().position(|m| m.id == mate_id)?;
        let removed = self.mates.remove(index);
        self.actuators
            .retain(|a| a.get("mateId").and_then(Value::as_str) != Some(mate_id));
        for r in [&removed.a, &removed.b] {
            let used = self
                .mates
                .iter()
                .any(|m| [&m.a, &m.b].into_iter().any(|o| o == r));
            if !used {
                self.connectors
                    .retain(|c| !(c.id == r.connector_id && c.instance_id == r.instance_id));
            }
        }
        Some(removed)
    }

    /// Adds a connector frame (local to `instance_id`) and returns its id.
    pub fn add_connector(&mut self, instance_id: Option<&str>, frame: &Pose) -> String {
        let id = self.unique_id("connector", |a, id| a.connectors.iter().any(|c| c.id == id));
        self.connectors.push(Connector {
            id: id.clone(),
            name: None,
            instance_id: instance_id.map(str::to_string),
            position: frame.t,
            rotation: Some(frame.rotation_deg()),
        });
        id
    }

    pub fn add_mate(&mut self, kind: MateKind, a: MateRef, b: MateRef) -> String {
        let id = self.unique_id("mate", |asm, id| asm.mates.iter().any(|m| m.id == id));
        self.mates.push(Mate {
            id: id.clone(),
            name: None,
            kind,
            a,
            b,
            params: None,
        });
        id
    }

    /// Mates that touch the instance at `index`.
    pub fn mates_of(&self, index: usize) -> Vec<&Mate> {
        let Some(id) = self.instances.get(index).map(|i| Some(i.id.clone())) else {
            return Vec::new();
        };
        self.mates
            .iter()
            .filter(|m| m.a.instance_id == id || m.b.instance_id == id)
            .collect()
    }
}

#[cfg(test)]
mod tests;
