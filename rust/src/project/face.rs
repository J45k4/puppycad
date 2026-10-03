//! Sketches on planar faces of extrusions.
//!
//! A face sketch targets a graph `face` node `{sourceId: extrudeId, faceId}`.
//! Its plane frame is derived exactly as the web app's
//! `getExtrudedFaceDescriptors` (`src/cad/extrude.ts`) does: from the
//! supporting extrusion's own prism (sketch frame, profile loops, depth), not
//! from the evaluated Boolean body. Face ids are `<extrudeId>-solid-face-<n>`:
//! one side face per profile loop point (outer loop first, then holes), then
//! the bottom cap, then the top cap. Because the frame is recomputed from the
//! graph on every evaluation, a face sketch follows its source when the source
//! depth changes ("source-depth following", as in the web app).

use super::*;

/// Right- or left-handed placement of 2D sketch coordinates in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: [f32; 3],
    pub x_axis: [f32; 3],
    pub y_axis: [f32; 3],
    pub normal: [f32; 3],
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(v: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot(v, v).sqrt();
    (length > 1e-9).then(|| v.map(|c| c / length))
}

impl Frame {
    pub fn world(&self, point: Point, offset: f32) -> [f32; 3] {
        std::array::from_fn(|i| {
            self.origin[i]
                + self.x_axis[i] * point.x
                + self.y_axis[i] * point.y
                + self.normal[i] * offset
        })
    }

    /// Sketch coordinates of `world` and its signed distance along the normal.
    pub fn local(&self, world: [f32; 3]) -> (Point, f32) {
        let d = std::array::from_fn(|i| world[i] - self.origin[i]);
        (
            Point {
                x: dot(d, self.x_axis),
                y: dot(d, self.y_axis),
            },
            dot(d, self.normal),
        )
    }

    /// True when `x × y` points against the normal (a web bottom-face frame).
    pub fn mirrored(&self) -> bool {
        dot(cross3(self.x_axis, self.y_axis), self.normal) < 0.0
    }
}

impl SketchPlane {
    /// The web `getPlaneSketchFrame` frames for XY, XZ and YZ.
    pub fn frame(self) -> Frame {
        let origin = [0.0; 3];
        match self {
            Self::Top => Frame {
                origin,
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 1.0, 0.0],
                normal: [0.0, 0.0, 1.0],
            },
            Self::Front => Frame {
                origin,
                x_axis: [1.0, 0.0, 0.0],
                y_axis: [0.0, 0.0, -1.0],
                normal: [0.0, 1.0, 0.0],
            },
            Self::Right => Frame {
                origin,
                x_axis: [0.0, 1.0, 0.0],
                y_axis: [0.0, 0.0, 1.0],
                normal: [1.0, 0.0, 0.0],
            },
        }
    }
}

/// Web `FaceReference` (`extrudeFace`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceRef {
    pub extrude_id: String,
    pub face_id: String,
}

impl FaceRef {
    pub fn node_id(&self) -> String {
        format!("face-{}-{}", self.extrude_id, self.face_id)
    }
}

/// One planar face of an extrusion's prism, as the web describes it.
#[derive(Clone, Debug)]
pub struct FaceDescriptor {
    pub face_id: String,
    pub label: String,
    pub frame: Frame,
    /// Face region in `frame` coordinates: outer loop, then holes.
    pub loops: Vec<Vec<Point>>,
}

impl FaceDescriptor {
    fn contains(&self, point: Point) -> bool {
        point_inside_loop(point, &self.loops[0])
            && !self.loops[1..]
                .iter()
                .any(|hole| point_inside_loop(point, hole))
    }
}

/// The web prism of one extrusion: sketch frame, profile loops and depth.
pub(super) struct ExtrusionBase {
    pub frame: Frame,
    pub loops: Vec<Vec<Point>>,
    pub depth: f32,
}

const MAX_FACE_CHAIN: usize = 32;

pub(super) fn profile_index(extrude: &Value, sketch_id: &str) -> Option<usize> {
    extrude
        .get("profileId")?
        .as_str()?
        .strip_prefix(&format!("{sketch_id}-profile-"))?
        .parse::<usize>()
        .ok()?
        .checked_sub(1)
}

/// Web `getExtrudedFaceDescriptors` for an extrusion named `extrude_id`.
pub(super) fn descriptors(extrude_id: &str, base: &ExtrusionBase) -> Option<Vec<FaceDescriptor>> {
    let mut faces = Vec::new();
    let mut index = 0;
    let face_id = |index: usize| format!("{extrude_id}-solid-face-{index}");
    for loop_points in &base.loops {
        for i in 0..loop_points.len() {
            index += 1;
            let start = base.frame.world(loop_points[i], 0.0);
            let end = base
                .frame
                .world(loop_points[(i + 1) % loop_points.len()], 0.0);
            let edge = std::array::from_fn(|axis| end[axis] - start[axis]);
            let length = dot(edge, edge).sqrt();
            let x_axis = normalize(edge)?;
            let y_axis = normalize(base.frame.normal)?;
            let normal = normalize(cross3(x_axis, y_axis))?;
            faces.push(FaceDescriptor {
                face_id: face_id(index),
                label: format!("Side Face {index}"),
                frame: Frame {
                    origin: start,
                    x_axis,
                    y_axis,
                    normal,
                },
                loops: vec![vec![
                    Point { x: 0.0, y: 0.0 },
                    Point { x: length, y: 0.0 },
                    Point {
                        x: length,
                        y: base.depth,
                    },
                    Point {
                        x: 0.0,
                        y: base.depth,
                    },
                ]],
            });
        }
    }
    faces.push(FaceDescriptor {
        face_id: face_id(index + 1),
        label: "Bottom Face".into(),
        frame: Frame {
            normal: base.frame.normal.map(|c| -c),
            ..base.frame
        },
        loops: base.loops.clone(),
    });
    faces.push(FaceDescriptor {
        face_id: face_id(index + 2),
        label: "Top Face".into(),
        frame: Frame {
            origin: base.frame.world(Point { x: 0.0, y: 0.0 }, base.depth),
            ..base.frame
        },
        loops: base.loops.clone(),
    });
    Some(faces)
}

impl ProjectFile {
    pub(super) fn graph_nodes(&self) -> Option<&Vec<Value>> {
        self.value
            .pointer(&format!("/items/{}/data/cad/nodes", self.part_index))?
            .as_array()
    }

    fn extrude_node(&self, id: &str) -> Option<&Value> {
        self.graph_nodes()?
            .iter()
            .find(|node| node["type"] == "extrude" && node["id"].as_str() == Some(id))
    }

    /// World frame of a sketch: its reference plane or its resolved face.
    pub fn sketch_frame(&self, sketch_id: &str) -> Option<Frame> {
        self.sketch_frame_guarded(sketch_id, 0)
    }

    fn sketch_frame_guarded(&self, sketch_id: &str, chain: usize) -> Option<Frame> {
        if chain > MAX_FACE_CHAIN {
            return None;
        }
        let workspace = self.sketches.iter().find(|sketch| sketch.id == sketch_id)?;
        if let Some(plane) = workspace.plane {
            return Some(plane.frame());
        }
        let face = workspace.face.as_ref()?;
        self.face_descriptors_guarded(&face.extrude_id, chain + 1)?
            .into_iter()
            .find(|descriptor| descriptor.face_id == face.face_id)
            .map(|descriptor| descriptor.frame)
    }

    pub(super) fn extrusion_base(&self, extrude: &Value, chain: usize) -> Option<ExtrusionBase> {
        let sketch_id = extrude.get("sketchId")?.as_str()?;
        let index = profile_index(extrude, sketch_id)?;
        let depth = positive(extrude.get("depth")?)?;
        let workspace = self.sketches.iter().find(|sketch| sketch.id == sketch_id)?;
        let profile = profiles_from_document(&workspace.document)?
            .into_iter()
            .nth(index)?;
        let mut frame = self.sketch_frame_guarded(sketch_id, chain)?;
        // Native placement of blind and symmetric prisms (the web treats
        // every extrusion as blind from the sketch plane): the faces must
        // lie where the evaluated body has them.
        let start = match extrude_options(extrude) {
            Ok(ExtrudeOptions {
                end: EndCondition::Symmetric,
                ..
            }) => -depth / 2.0,
            Ok(ExtrudeOptions {
                end: EndCondition::Blind,
                flip: true,
            }) => -depth,
            _ => 0.0,
        };
        frame.origin = frame.world(Point { x: 0.0, y: 0.0 }, start);
        Some(ExtrusionBase {
            frame,
            loops: std::iter::once(profile.outer)
                .chain(profile.holes)
                .collect(),
            depth,
        })
    }

    /// The web face descriptors of extrusion `extrude_id`.
    pub fn face_descriptors(&self, extrude_id: &str) -> Option<Vec<FaceDescriptor>> {
        self.face_descriptors_guarded(extrude_id, 0)
    }

    fn face_descriptors_guarded(
        &self,
        extrude_id: &str,
        chain: usize,
    ) -> Option<Vec<FaceDescriptor>> {
        let extrude = self.extrude_node(extrude_id)?;
        descriptors(extrude_id, &self.extrusion_base(extrude, chain)?)
    }

    /// True when the extrusion's native solid is its web prism. Cuts from a
    /// face sketch run inward natively, so their web face ids would not name
    /// the faces the user sees; they are not offered for face sketches.
    fn extrusion_matches_web(&self, extrude: &Value) -> bool {
        extrude["operation"] != "cut"
            || extrude["sketchId"].as_str().is_some_and(|id| {
                self.sketches
                    .iter()
                    .any(|sketch| sketch.id == id && sketch.plane.is_some())
            })
    }

    /// The extrusion face containing world point `hit` on a surface with
    /// outward `normal`, newest extrusion first.
    pub fn face_at(&self, hit: [f32; 3], normal: [f32; 3]) -> Option<(FaceRef, String)> {
        let normal = normalize(normal)?;
        let nodes = self.graph_nodes()?;
        for extrude in nodes.iter().rev().filter(|node| node["type"] == "extrude") {
            if !self.extrusion_matches_web(extrude) {
                continue;
            }
            let Some(id) = extrude["id"].as_str() else {
                continue;
            };
            let Some(faces) = self.face_descriptors(id) else {
                continue;
            };
            for face in faces {
                let (local, distance) = face.frame.local(hit);
                if dot(face.frame.normal, normal) > 0.999
                    && distance.abs() < 1e-2
                    && face.contains(local)
                {
                    return Some((
                        FaceRef {
                            extrude_id: id.to_string(),
                            face_id: face.face_id,
                        },
                        face.label,
                    ));
                }
            }
        }
        None
    }

    /// True when every face sketch's face still exists.
    pub(super) fn face_sketches_resolve(&self) -> bool {
        self.sketches
            .iter()
            .filter(|sketch| sketch.face.is_some())
            .all(|sketch| self.sketch_frame(&sketch.id).is_some())
    }

    pub fn sketch_face(&self, index: usize) -> Option<&FaceRef> {
        self.sketches.get(index)?.face.as_ref()
    }

    /// Adds an empty sketch on an extrusion face and makes it active.
    pub fn create_sketch_on_face(
        &mut self,
        current: &Document,
        face: &FaceRef,
    ) -> Result<Document, String> {
        if let Some(reason) = &self.read_only_reason {
            return Err(format!("project is read-only: {reason}"));
        }
        if self
            .face_descriptors(&face.extrude_id)
            .is_none_or(|faces| faces.iter().all(|f| f.face_id != face.face_id))
        {
            return Err("face does not exist on its extrusion".into());
        }
        self.sketches[self.active_sketch].document = current.clone();
        let face_node_id = face.node_id();
        let nodes = self
            .value
            .pointer_mut(&format!("/items/{}/data/cad/nodes", self.part_index))
            .and_then(Value::as_array_mut)
            .ok_or("project CAD graph changed unexpectedly")?;
        if !nodes
            .iter()
            .any(|node| node["id"].as_str() == Some(&face_node_id))
        {
            nodes.push(json!({"id":face_node_id,"type":"face","sourceId":face.extrude_id,"faceId":face.face_id}));
        }
        let mut number = self.sketches.len() + 1;
        let id = loop {
            let candidate = format!("native-sketch-{number}");
            if !nodes
                .iter()
                .any(|node| node.get("id").and_then(Value::as_str) == Some(&candidate))
            {
                break candidate;
            }
            number += 1;
        };
        let name = format!("Sketch {number}");
        nodes.push(json!({"id":id,"type":"sketch","name":name,"targetId":face_node_id,"relations":[],"variables":[],"dimensions":[]}));
        for path in ["cad/rootNodeIds", "tree/orderedNodeIds"] {
            self.value
                .pointer_mut(&format!("/items/{}/data/{path}", self.part_index))
                .and_then(Value::as_array_mut)
                .ok_or("project graph or tree is invalid")?
                .push(json!(id));
        }
        self.sketches.push(SketchWorkspace {
            id,
            name,
            plane: None,
            face: Some(face.clone()),
            document: Document::default(),
            entity_ids: HashMap::new(),
            original_entities: HashMap::new(),
        });
        self.active_sketch = self.sketches.len() - 1;
        let mut document = Document::default();
        let context = self.sketch_context(self.active_sketch);
        document.reference_edges = context.edges;
        document.reference_silhouette = context.silhouette;
        Ok(document)
    }
}
