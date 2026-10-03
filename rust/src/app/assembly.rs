//! The assembly view: every instance drawn with its part's evaluated bodies
//! placed by the instance transform, per-instance picking and hover,
//! a Properties panel with editable position/rotation, insert/delete,
//! the mate gesture and per-instance export. Each edit is one undo step and
//! re-solves the mates (`puppycad::assembly::solver`).

use super::*;

use puppycad::assembly::{self as asm, Assembly, MateKind, MateRef, Pose, Transform, V3, solver};
use std::collections::HashMap;

/// Instance colors (cycled); selected and hovered instances are tinted.
const INSTANCE_COLORS: [[f32; 3]; 6] = [
    [0.48, 0.82, 1.0],
    [1.0, 0.72, 0.42],
    [0.62, 0.92, 0.55],
    [0.92, 0.58, 0.88],
    [0.95, 0.88, 0.45],
    [0.55, 0.70, 0.98],
];

/// Properties rows that edit the selected instance's transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AsmField {
    X,
    Y,
    Z,
    Rx,
    Ry,
    Rz,
}

impl AsmField {
    const ALL: [AsmField; 6] = [
        AsmField::X,
        AsmField::Y,
        AsmField::Z,
        AsmField::Rx,
        AsmField::Ry,
        AsmField::Rz,
    ];

    fn label(self) -> &'static str {
        match self {
            AsmField::X => "X",
            AsmField::Y => "Y",
            AsmField::Z => "Z",
            AsmField::Rx => "RX",
            AsmField::Ry => "RY",
            AsmField::Rz => "RZ",
        }
    }

    fn get(self, t: &Transform) -> f64 {
        match self {
            AsmField::X => t.translation[0],
            AsmField::Y => t.translation[1],
            AsmField::Z => t.translation[2],
            AsmField::Rx => t.rotation[0],
            AsmField::Ry => t.rotation[1],
            AsmField::Rz => t.rotation[2],
        }
    }

    fn set(self, t: &mut Transform, value: f64) {
        match self {
            AsmField::X => t.translation[0] = value,
            AsmField::Y => t.translation[1] = value,
            AsmField::Z => t.translation[2] = value,
            AsmField::Rx => t.rotation[0] = value,
            AsmField::Ry => t.rotation[1] = value,
            AsmField::Rz => t.rotation[2] = value,
        }
    }
}

/// A local face or edge pick on an instance, in part coordinates: for a
/// face the hit point and outward normal, for a sharp edge its midpoint and
/// direction (the axis of a revolute or slider mate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct FacePick {
    pub(super) instance: usize,
    pub(super) point: V3,
    pub(super) normal: V3,
    pub(super) edge: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum AsmPrompt {
    /// Choose the part to insert (digit keys).
    Insert,
    /// Choose the mate kind between the selected face and `b` (digit keys).
    MateKind { a: FacePick, b: FacePick },
    /// Typing a value for a Properties field.
    Field { field: AsmField, buffer: String },
    /// Typing the servo angle (degrees) of a revolute mate.
    Servo { mate_id: String, buffer: String },
}

pub(super) struct AssemblyView {
    /// Project item index of the assembly document.
    pub(super) item: usize,
    pub(super) asm: Assembly,
    /// Part-local bodies by part id.
    bodies: HashMap<String, Result<Vec<stl::Body>, String>>,
    /// Part-local sharp edges by part id (the part view's edge overlay).
    edges: HashMap<String, Vec<project::SolidEdge>>,
    pub(super) selected: Option<usize>,
    pub(super) hovered: Option<usize>,
    /// Sharp edge under the cursor (world space), highlighted and used as
    /// the mate axis by J.
    pub(super) hovered_edge: Option<project::SolidEdge>,
    /// Face clicked on the selected instance (mate side A).
    pub(super) pick_a: Option<FacePick>,
    /// Mate id selected in the Properties mate list (Delete removes it).
    pub(super) selected_mate: Option<String>,
    pub(super) prompt: Option<AsmPrompt>,
    undo: Vec<Assembly>,
    redo: Vec<Assembly>,
}

/// Instance-colored scene: `triangle_body` holds the instance index.
pub(super) fn assembly_scene(view: &AssemblyView) -> SolidScene {
    let mut triangles = Vec::new();
    let mut triangle_body = Vec::new();
    let mut edges = Vec::new();
    for (index, instance) in view.asm.instances.iter().enumerate() {
        let transform = instance.transform();
        let pose = transform.pose();
        for edge in view.edges.get(&instance.part_id).into_iter().flatten() {
            edges.push((
                index,
                edge.transformed(
                    |p| transform.apply(p.map(f64::from)).map(|c| c as f32),
                    |v| pose.vector(v.map(f64::from)).map(|c| c as f32),
                ),
            ));
        }
        let Some(Ok(bodies)) = view.bodies.get(&instance.part_id) else {
            continue;
        };
        for body in bodies {
            for triangle in &body.triangles {
                triangles
                    .push(triangle.map(|p| transform.apply(p.map(f64::from)).map(|c| c as f32)));
                triangle_body.push(index);
            }
        }
    }
    SolidScene {
        bodies: view.asm.instances.len(),
        vertices: triangles.iter().flatten().copied().collect(),
        triangles,
        triangle_body,
        edges,
    }
}

/// Sharp edges of a part's evaluated bodies (rebuilt as manifolds).
fn part_edges(bodies: &Result<Vec<stl::Body>, String>) -> Vec<project::SolidEdge> {
    bodies
        .iter()
        .flatten()
        .filter_map(|body| stl::manifold_from_triangles(&body.triangles, &body.name).ok())
        .flat_map(|solid| project::sharp_edges(&solid))
        .collect()
}

/// File-name-safe form of an instance name.
pub(super) fn file_stem_part(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "instance".into()
    } else {
        trimmed.to_string()
    }
}

impl AssemblyView {
    /// Every instance's bodies in world coordinates, one body per instance
    /// named after it.
    pub(super) fn world_bodies(&self) -> Result<Vec<stl::Body>, String> {
        let mut out = Vec::new();
        for instance in &self.asm.instances {
            let transform = instance.transform();
            let bodies = match self.bodies.get(&instance.part_id) {
                Some(Ok(bodies)) => bodies,
                Some(Err(error)) => return Err(format!("{}: {error}", instance.label())),
                None => return Err(format!("{}: part not evaluated", instance.label())),
            };
            let triangles = bodies
                .iter()
                .flat_map(|body| &body.triangles)
                .map(|triangle| {
                    triangle.map(|p| transform.apply(p.map(f64::from)).map(|c| c as f32))
                })
                .collect();
            out.push(stl::Body::new(instance.label(), triangles));
        }
        if out.is_empty() {
            return Err("Assembly has no instances to export".into());
        }
        Ok(out)
    }
}

impl App {
    /// Opens the assembly at project item `item` (solving its mates).
    pub(super) fn open_assembly(&mut self, item: usize) {
        self.document.finish_drag();
        self.editing = None;
        self.more_open = false;
        let Some(project) = &self.project else {
            return;
        };
        let Some(mut assembly) = project.assembly_at(item) else {
            self.status_message = "Assembly does not exist".into();
            return;
        };
        let mut bodies = HashMap::new();
        for instance in &assembly.instances {
            bodies
                .entry(instance.part_id.clone())
                .or_insert_with(|| project.part_bodies(&instance.part_id, &self.document));
        }
        let edges = bodies
            .iter()
            .map(|(id, bodies)| (id.clone(), part_edges(bodies)))
            .collect();
        let solve = if assembly.read_only_reason.is_none() {
            solver::apply(&mut assembly).err()
        } else {
            None
        };
        self.status_message = match (&assembly.read_only_reason, solve) {
            (Some(reason), _) => format!("READ ONLY assembly: {reason}"),
            (None, Some(error)) => format!("Assembly {}: {error}", assembly.name),
            (None, None) => format!(
                "Assembly: {}  |  click selects  |  I insert  |  J mate  |  Ctrl+B back to part",
                assembly.name
            ),
        };
        self.assembly = Some(AssemblyView {
            item,
            asm: assembly,
            bodies,
            edges,
            selected: None,
            hovered: None,
            hovered_edge: None,
            pick_a: None,
            selected_mate: None,
            prompt: None,
            undo: Vec::new(),
            redo: Vec::new(),
        });
        self.solid_drag = None;
    }

    pub(super) fn close_assembly(&mut self) {
        if self.assembly.take().is_some() {
            self.status_message = self.project.as_ref().map_or_else(String::new, |project| {
                format!("Part: {}", project.part_name())
            });
        }
    }

    /// Ctrl+B: close the assembly view, or open the first assembly
    /// (creating one when the project has none).
    pub(super) fn toggle_assembly_view(&mut self) {
        self.more_open = false;
        if self.assembly.is_some() {
            self.close_assembly();
            return;
        }
        let Some(project) = &mut self.project else {
            self.status_message = "Open a .pcad project to use assemblies".into();
            return;
        };
        let item = match project.assembly_indices().first() {
            Some(&item) => item,
            None => match project.create_assembly() {
                Ok(item) => item,
                Err(error) => {
                    self.status_message = format!("New assembly: {error}");
                    return;
                }
            },
        };
        self.open_assembly(item);
    }

    /// Runs one edit as an undo step: on success the result is solved and
    /// written to the project, on failure everything is restored.
    fn assembly_edit(
        &mut self,
        label: &str,
        edit: impl FnOnce(&mut Assembly) -> Result<(), String>,
    ) -> bool {
        let Some(view) = &mut self.assembly else {
            return false;
        };
        if let Some(reason) = &view.asm.read_only_reason {
            self.status_message = format!("READ ONLY assembly: {reason}");
            return false;
        }
        let before = view.asm.clone();
        let result = edit(&mut view.asm).and_then(|()| solver::apply(&mut view.asm).map(|_| ()));
        let result = result.and_then(|()| match &mut self.project {
            Some(project) => project.set_assembly(view.item, &view.asm),
            None => Err("no project".into()),
        });
        match result {
            Ok(()) => {
                view.undo.push(before);
                view.redo.clear();
                self.status_message = format!("{label}  |  Ctrl+Z to undo  |  Ctrl+S to save");
                true
            }
            Err(error) => {
                view.asm = before;
                self.status_message = format!("{label} failed: {error}");
                false
            }
        }
    }

    pub(super) fn assembly_undo(&mut self, redo: bool) {
        let Some(view) = &mut self.assembly else {
            return;
        };
        let (from, to) = if redo {
            (&mut view.redo, &mut view.undo)
        } else {
            (&mut view.undo, &mut view.redo)
        };
        let Some(state) = from.pop() else {
            self.status_message = if redo {
                "Nothing to redo"
            } else {
                "Nothing to undo"
            }
            .into();
            return;
        };
        to.push(std::mem::replace(&mut view.asm, state));
        if view
            .selected
            .is_some_and(|index| index >= view.asm.instances.len())
        {
            view.selected = None;
        }
        view.pick_a = None;
        if let Some(id) = &view.selected_mate
            && !view.asm.mates.iter().any(|m| &m.id == id)
        {
            view.selected_mate = None;
        }
        if let Some(project) = &mut self.project {
            let _ = project.set_assembly(view.item, &view.asm);
        }
        self.status_message = if redo { "Redo" } else { "Undo" }.into();
    }

    /// Scene and projector of the open assembly.
    pub(super) fn assembly_projector(
        &self,
        layout: Layout,
    ) -> Option<(SolidScene, SolidProjector)> {
        let scene = assembly_scene(self.assembly.as_ref()?);
        let projector = SolidProjector::new(self.solid_view, &scene.vertices, layout.viewport)?;
        Some((scene, projector))
    }

    /// The instance face under `point`, in that instance's local frame.
    pub(super) fn assembly_face_at(&self, layout: Layout, point: Point) -> Option<FacePick> {
        if !layout.viewport.contains(point.x, point.y) {
            return None;
        }
        let view = self.assembly.as_ref()?;
        let (scene, projector) = self.assembly_projector(layout)?;
        let (triangle, _, hit) = projector.pick_triangle(&scene, point)?;
        let [a, b, c] = scene.triangles[triangle];
        let normal = triangle_normal(a, b, c)?;
        let instance = scene.triangle_body[triangle];
        let transform = view.asm.instances[instance].transform();
        let pose = transform.pose().inverse();
        let local = pose.point(hit.map(f64::from));
        Some(FacePick {
            instance,
            point: std::array::from_fn(|i| local[i] / transform.scale[i]),
            normal: asm::v_unit(pose.vector(normal.map(f64::from)))?,
            edge: false,
        })
    }

    /// The visible sharp edge near `point` as a local pick, with the world
    /// edge for highlighting.
    pub(super) fn assembly_edge_at(
        &self,
        layout: Layout,
        point: Point,
    ) -> Option<(FacePick, project::SolidEdge)> {
        if !layout.viewport.contains(point.x, point.y) {
            return None;
        }
        let view = self.assembly.as_ref()?;
        let (scene, projector) = self.assembly_projector(layout)?;
        let edge = self.edge_near(&projector, &scene, point)?;
        let instance = scene
            .edges
            .iter()
            .find(|(_, e)| e.a == edge.a && e.b == edge.b)?
            .0;
        let transform = view.asm.instances[instance].transform();
        let pose = transform.pose().inverse();
        let mid: V3 = std::array::from_fn(|i| f64::from(edge.a[i] + edge.b[i]) * 0.5);
        let local = pose.point(mid);
        let direction = asm::v_sub(edge.b.map(f64::from), edge.a.map(f64::from));
        Some((
            FacePick {
                instance,
                point: std::array::from_fn(|i| local[i] / transform.scale[i]),
                normal: asm::v_unit(pose.vector(direction))?,
                edge: true,
            },
            edge,
        ))
    }

    /// A sharp edge under the cursor wins over the face behind it.
    pub(super) fn assembly_pick_at(&self, layout: Layout, point: Point) -> Option<FacePick> {
        self.assembly_edge_at(layout, point)
            .map(|(pick, _)| pick)
            .or_else(|| self.assembly_face_at(layout, point))
    }

    /// Recompute the hovered instance; true when it changed.
    pub(super) fn update_assembly_hover(&mut self) -> bool {
        let layout = self.layout();
        let edge = self.assembly_edge_at(layout, self.cursor);
        let hovered = match &edge {
            Some((pick, _)) => Some(pick.instance),
            None => self
                .assembly_face_at(layout, self.cursor)
                .map(|pick| pick.instance),
        };
        let edge = edge.map(|(_, edge)| edge);
        match &mut self.assembly {
            Some(view) if view.hovered != hovered || view.hovered_edge != edge => {
                view.hovered = hovered;
                view.hovered_edge = edge;
                true
            }
            _ => false,
        }
    }

    /// Left click in assembly mode.
    pub(super) fn assembly_click(&mut self, layout: Layout, point: Point) {
        // Project panel rows: parts and assemblies, then instances.
        let rows = self.project_rows();
        for (row, item) in rows.iter().enumerate() {
            if layout.part_row(row).contains(point.x, point.y) {
                self.open_project_row(*item);
                return;
            }
        }
        let instance_count = self.assembly.as_ref().map_or(0, |v| v.asm.instances.len());
        for index in 0..instance_count {
            if layout
                .part_row(rows.len() + 1 + index)
                .contains(point.x, point.y)
            {
                if let Some(view) = &mut self.assembly {
                    view.selected = Some(index);
                    view.pick_a = None;
                    view.selected_mate = None;
                    view.prompt = None;
                }
                self.describe_selection();
                return;
            }
        }
        if self.assembly_mate_row_click(layout, point) {
            return;
        }
        for (index, field) in AsmField::ALL.into_iter().enumerate() {
            if assembly_property_row(layout, index + 2).contains(point.x, point.y) {
                self.begin_assembly_field(field);
                return;
            }
        }
        if !layout.viewport.contains(point.x, point.y) {
            return;
        }
        let pick = self.assembly_pick_at(layout, point);
        if let Some(view) = &mut self.assembly {
            view.prompt = None;
            view.selected = pick.map(|pick| pick.instance);
            view.pick_a = pick;
            view.selected_mate = None;
        }
        self.describe_selection();
    }

    fn describe_selection(&mut self) {
        let Some(view) = &self.assembly else {
            return;
        };
        self.status_message = match view.selected.and_then(|i| view.asm.instances.get(i)) {
            Some(instance) => format!(
                "Selected {}  |  hover another instance and press J to mate  |  Delete removes",
                instance.label()
            ),
            None => "Nothing selected".into(),
        };
    }

    fn begin_assembly_field(&mut self, field: AsmField) {
        let Some(view) = &mut self.assembly else {
            return;
        };
        if view.selected.is_none() {
            self.status_message = "Select an instance first".into();
            return;
        }
        view.prompt = Some(AsmPrompt::Field {
            field,
            buffer: String::new(),
        });
        self.status_message = format!(
            "{}: type a value ({}), Enter applies and re-solves  |  Esc cancels",
            field.label(),
            if matches!(field, AsmField::X | AsmField::Y | AsmField::Z) {
                "mm"
            } else {
                "degrees"
            }
        );
    }

    /// I: choose a part to insert.
    pub(super) fn begin_insert_instance(&mut self) {
        let parts = self
            .project
            .as_ref()
            .map(ProjectFile::parts)
            .unwrap_or_default();
        let Some(view) = &mut self.assembly else {
            return;
        };
        if parts.is_empty() {
            self.status_message = "The project has no parts to insert".into();
            return;
        }
        view.prompt = Some(AsmPrompt::Insert);
        self.status_message = "Insert: press the number of a part  |  Esc cancels".into();
    }

    fn insert_part(&mut self, choice: usize) {
        let parts = self
            .project
            .as_ref()
            .map(ProjectFile::parts)
            .unwrap_or_default();
        let Some((part_id, part_name)) = parts.get(choice).cloned() else {
            return;
        };
        let bodies = match &self.project {
            Some(project) => project.part_bodies(&part_id, &self.document),
            None => return,
        };
        let layout = self.layout();
        // Place it beside the current scene's bounding box.
        let max_x = self
            .assembly_projector(layout)
            .map(|(scene, _)| {
                scene
                    .vertices
                    .iter()
                    .map(|v| v[0])
                    .fold(f32::NEG_INFINITY, f32::max)
            })
            .filter(|x| x.is_finite());
        let min_x = bodies.as_ref().ok().and_then(|bodies| {
            bodies
                .iter()
                .flat_map(|b| &b.triangles)
                .flatten()
                .map(|v| v[0])
                .reduce(f32::min)
        });
        let x = match (max_x, min_x) {
            (Some(max), Some(min)) => f64::from(max - min + 10.0),
            _ => 0.0,
        };
        if let Some(view) = &mut self.assembly {
            view.prompt = None;
            view.edges.insert(part_id.clone(), part_edges(&bodies));
            view.bodies.insert(part_id.clone(), bodies);
        }
        let count = self.assembly.as_ref().map_or(0, |v| {
            v.asm
                .instances
                .iter()
                .filter(|i| i.part_id == part_id)
                .count()
        });
        let name = format!("{part_name} {}", count + 1);
        let mut inserted = None;
        if self.assembly_edit(&format!("Inserted {name}"), |asm| {
            inserted = Some(asm.insert_instance(
                &part_id,
                &name,
                Transform {
                    translation: [x, 0.0, 0.0],
                    ..Transform::default()
                },
            ));
            Ok(())
        }) && let Some(view) = &mut self.assembly
        {
            view.selected = inserted;
            view.pick_a = None;
        }
    }

    /// Delete: remove the selected instance and its mates.
    pub(super) fn delete_selected_instance(&mut self) {
        if let Some(mate_id) = self.assembly.as_ref().and_then(|v| v.selected_mate.clone()) {
            self.delete_mate(&mate_id);
            return;
        }
        let Some(index) = self.assembly.as_ref().and_then(|view| view.selected) else {
            self.status_message = "Select an instance to delete".into();
            return;
        };
        let name = self
            .assembly
            .as_ref()
            .map_or_else(String::new, |v| v.asm.instances[index].label().to_string());
        if self.assembly_edit(&format!("Deleted {name}"), |asm| {
            asm.remove_instance(index)
                .map(|_| ())
                .ok_or_else(|| "instance does not exist".to_string())
        }) && let Some(view) = &mut self.assembly
        {
            view.selected = None;
            view.pick_a = None;
            view.hovered = None;
        }
    }

    /// J: mate the face clicked on the selected instance with the face under
    /// the cursor on another instance; a prompt then asks for the kind.
    pub(super) fn begin_mate(&mut self) {
        let layout = self.layout();
        let b = self.assembly_pick_at(layout, self.cursor);
        let Some(view) = &mut self.assembly else {
            return;
        };
        if b.is_none()
            && let Some(selected) = view.selected
        {
            self.fasten_to_world(selected);
            return;
        }
        let Some(a) = view.pick_a.filter(|a| Some(a.instance) == view.selected) else {
            self.status_message = "Mate: click a face of the first instance".into();
            return;
        };
        let Some(b) = b.filter(|b| b.instance != a.instance) else {
            self.status_message = "Mate: hover a face of another instance, then press J".into();
            return;
        };
        view.prompt = Some(AsmPrompt::MateKind { a, b });
        self.status_message =
            "Mate: 1 fastened  2 revolute  3 slider  4 planar  5 ball  |  Esc cancels".into();
    }

    fn create_mate(&mut self, a: FacePick, b: FacePick, kind: MateKind) {
        let Some(view) = &self.assembly else {
            return;
        };
        let ids = [a.instance, b.instance].map(|i| view.asm.instances[i].id.clone());
        let poses = [a.instance, b.instance].map(|i| view.asm.instances[i].transform().pose());
        let names = [a.instance, b.instance].map(|i| view.asm.instances[i].label().to_string());
        if let Some(view) = &mut self.assembly {
            view.prompt = None;
        }
        // Frame A's Z is A's outward face normal; frame B's Z points into B,
        // so coinciding frames put the faces against each other. An edge
        // pick uses the edge direction instead, turned to agree with the
        // other side's axis so the mate does not flip the instance.
        let mut za = a.normal;
        let mut zb = if b.edge {
            b.normal
        } else {
            asm::v_scale(b.normal, -1.0)
        };
        if asm::v_dot(poses[0].vector(za), poses[1].vector(zb)) < 0.0 {
            if b.edge {
                zb = asm::v_scale(zb, -1.0);
            } else if a.edge {
                za = asm::v_scale(za, -1.0);
            }
        }
        self.assembly_edit(
            &format!("{} mate {} - {}", kind.label(), names[0], names[1]),
            |assembly| {
                let ca = assembly.add_connector(Some(&ids[0]), &Pose::from_normal(a.point, za));
                let cb = assembly.add_connector(Some(&ids[1]), &Pose::from_normal(b.point, zb));
                assembly.add_mate(
                    kind,
                    MateRef {
                        instance_id: Some(ids[0].clone()),
                        connector_id: ca,
                    },
                    MateRef {
                        instance_id: Some(ids[1].clone()),
                        connector_id: cb,
                    },
                );
                Ok(())
            },
        );
    }

    fn finish_assembly_field(&mut self, field: AsmField, buffer: &str) {
        let Ok(value) = buffer.trim().parse::<f64>() else {
            self.status_message = format!("{}: enter a number", field.label());
            return;
        };
        if !value.is_finite() {
            return;
        }
        let Some(index) = self.assembly.as_ref().and_then(|v| v.selected) else {
            return;
        };
        self.assembly_edit(&format!("Set {} to {value}", field.label()), |asm| {
            let instance = &mut asm.instances[index];
            let mut transform = instance.transform();
            field.set(&mut transform, value);
            instance.transform = Some(transform);
            Ok(())
        });
    }

    /// Prompt keys in assembly mode; true when consumed.
    pub(super) fn assembly_prompt_key(&mut self, code: KeyCode, text: Option<&str>) -> bool {
        let Some(prompt) = self.assembly.as_ref().and_then(|v| v.prompt.clone()) else {
            return false;
        };
        if code == KeyCode::Escape {
            if let Some(view) = &mut self.assembly {
                view.prompt = None;
            }
            self.status_message = "Cancelled".into();
            return true;
        }
        let digit = match code {
            KeyCode::Digit1 | KeyCode::Numpad1 => Some(0),
            KeyCode::Digit2 | KeyCode::Numpad2 => Some(1),
            KeyCode::Digit3 | KeyCode::Numpad3 => Some(2),
            KeyCode::Digit4 | KeyCode::Numpad4 => Some(3),
            KeyCode::Digit5 | KeyCode::Numpad5 => Some(4),
            KeyCode::Digit6 | KeyCode::Numpad6 => Some(5),
            KeyCode::Digit7 | KeyCode::Numpad7 => Some(6),
            KeyCode::Digit8 | KeyCode::Numpad8 => Some(7),
            KeyCode::Digit9 | KeyCode::Numpad9 => Some(8),
            _ => None,
        };
        match prompt {
            AsmPrompt::Insert => {
                if let Some(choice) = digit {
                    self.insert_part(choice);
                }
            }
            AsmPrompt::MateKind { a, b } => {
                if let Some(kind) = digit.and_then(|d| MateKind::ALL.get(d).copied()) {
                    self.create_mate(a, b, kind);
                }
            }
            AsmPrompt::Field { field, mut buffer } => {
                if edit_number(&mut buffer, code, text) {
                    if let Some(view) = &mut self.assembly {
                        view.prompt = None;
                    }
                    self.finish_assembly_field(field, &buffer);
                } else if let Some(view) = &mut self.assembly {
                    view.prompt = Some(AsmPrompt::Field { field, buffer });
                }
            }
            AsmPrompt::Servo {
                mate_id,
                mut buffer,
            } => {
                if edit_number(&mut buffer, code, text) {
                    if let Some(view) = &mut self.assembly {
                        view.prompt = None;
                    }
                    self.finish_servo(&mate_id, &buffer);
                } else if let Some(view) = &mut self.assembly {
                    view.prompt = Some(AsmPrompt::Servo { mate_id, buffer });
                }
            }
        }
        true
    }

    /// Click on a Properties mate row: the DEG value of a revolute mate
    /// starts a servo edit, anywhere else selects the mate. True when hit.
    fn assembly_mate_row_click(&mut self, layout: Layout, point: Point) -> bool {
        let Some(view) = &mut self.assembly else {
            return false;
        };
        let Some(selected) = view.selected else {
            return false;
        };
        let rows: Vec<(String, MateKind)> = view
            .asm
            .mates_of(selected)
            .into_iter()
            .map(|m| (m.id.clone(), m.kind))
            .collect();
        for (index, (id, kind)) in rows.into_iter().enumerate() {
            let row = assembly_mate_row(layout, index);
            if !row.contains(point.x, point.y) {
                continue;
            }
            view.selected_mate = Some(id.clone());
            view.pick_a = None;
            if kind == MateKind::Revolute && servo_value_rect(row).contains(point.x, point.y) {
                view.prompt = Some(AsmPrompt::Servo {
                    mate_id: id,
                    buffer: String::new(),
                });
                self.status_message =
                    "Servo angle: type degrees, Enter applies and re-solves  |  Esc cancels".into();
            } else {
                view.prompt = None;
                self.status_message = format!(
                    "Selected {} mate  |  Delete removes it{}",
                    kind.label(),
                    if kind == MateKind::Revolute {
                        "  |  click its DEG value to drive it"
                    } else {
                        ""
                    }
                );
            }
            return true;
        }
        false
    }

    fn finish_servo(&mut self, mate_id: &str, buffer: &str) {
        let Some(value) = buffer.trim().parse::<f64>().ok().filter(|v| v.is_finite()) else {
            self.status_message = "Servo angle: enter a number".into();
            return;
        };
        self.assembly_edit(&format!("Servo angle {value} deg"), |asm| {
            asm.set_servo_angle(mate_id, value);
            Ok(())
        });
    }

    /// Delete with a mate selected: remove it, its servo and its now-unused
    /// connectors.
    fn delete_mate(&mut self, mate_id: &str) {
        let label = self
            .assembly
            .as_ref()
            .and_then(|v| v.asm.mates.iter().find(|m| m.id == mate_id))
            .map_or("mate", |m| m.kind.label())
            .to_string();
        if self.assembly_edit(&format!("Deleted {label} mate"), |asm| {
            asm.remove_mate(mate_id)
                .map(|_| ())
                .ok_or_else(|| "mate does not exist".to_string())
        }) && let Some(view) = &mut self.assembly
        {
            view.selected_mate = None;
        }
    }

    /// J with nothing hovered: fasten the selected instance to the world at
    /// its current placement.
    fn fasten_to_world(&mut self, index: usize) {
        let Some(view) = &mut self.assembly else {
            return;
        };
        view.prompt = None;
        let instance = &view.asm.instances[index];
        let (id, name, pose) = (
            instance.id.clone(),
            instance.label().to_string(),
            instance.transform().pose(),
        );
        self.assembly_edit(&format!("FASTENED {name} to the world"), |asm| {
            let ground = asm.add_connector(None, &pose);
            let local = asm.add_connector(Some(&id), &Pose::IDENTITY);
            asm.add_mate(
                MateKind::Fasten,
                MateRef {
                    instance_id: None,
                    connector_id: ground,
                },
                MateRef {
                    instance_id: Some(id.clone()),
                    connector_id: local,
                },
            );
            Ok(())
        });
    }

    /// Esc without a prompt: clear the selection, then close the view.
    pub(super) fn assembly_cancel(&mut self) {
        match &mut self.assembly {
            Some(view) if view.selected.is_some() => {
                view.selected = None;
                view.pick_a = None;
                self.status_message = "Selection cleared  |  Esc again returns to the part".into();
            }
            Some(_) => self.close_assembly(),
            None => {}
        }
    }

    /// Ctrl+E / Ctrl+Shift+E in the assembly view: one STL per instance,
    /// `<project>-<instance>.stl`.
    pub(super) fn export_assembly_stl(&mut self, format: stl::StlFormat) {
        let (Some(project), Some(view)) = (&self.project, &self.assembly) else {
            return;
        };
        self.status_message = match view.world_bodies().and_then(|bodies| {
            let stem = project
                .path()
                .file_stem()
                .map_or_else(|| "assembly".into(), |s| s.to_string_lossy().into_owned());
            bodies
                .iter()
                .map(|body| {
                    let path = project
                        .path()
                        .with_file_name(format!("{stem}-{}.stl", file_stem_part(&body.name)));
                    let bytes = match format {
                        stl::StlFormat::Ascii => stl::ascii_stl(&body.triangles).into_bytes(),
                        stl::StlFormat::Binary => stl::binary_stl(
                            &format!("{} / {}", view.asm.name, body.name),
                            &body.triangles,
                        ),
                    };
                    std::fs::write(&path, bytes)
                        .map_err(|error| format!("write {}: {error}", path.display()))
                })
                .collect::<Result<Vec<()>, String>>()
        }) {
            Ok(written) => format!(
                "Exported {} instance STL file{}",
                written.len(),
                if written.len() == 1 { "" } else { "s" }
            ),
            Err(error) => format!("Assembly STL export failed: {error}"),
        };
    }

    /// Ctrl+Alt+E in the assembly view: one 3MF, one object per instance.
    pub(super) fn export_assembly_3mf(&mut self) {
        let (Some(project), Some(view)) = (&self.project, &self.assembly) else {
            return;
        };
        self.status_message = match view.world_bodies().and_then(|bodies| {
            threemf::write_3mf(project.path(), &view.asm.name, &bodies).map(|p| (p, bodies.len()))
        }) {
            Ok((path, count)) => format!(
                "Exported {count} instance{} to {}",
                if count == 1 { "" } else { "s" },
                path.file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into())
            ),
            Err(error) => format!("3MF export failed: {error}"),
        };
    }

    /// Part and assembly item indices shown as Project panel rows.
    pub(super) fn project_rows(&self) -> Vec<usize> {
        let Some(project) = &self.project else {
            return Vec::new();
        };
        let mut rows = project.part_indices();
        rows.extend(project.assembly_indices());
        rows.sort_unstable();
        rows
    }

    /// Click on a Project panel row: open a part or an assembly.
    pub(super) fn open_project_row(&mut self, item: usize) {
        let is_assembly = self
            .project
            .as_ref()
            .is_some_and(|project| project.assembly_indices().contains(&item));
        if is_assembly {
            self.open_assembly(item);
        } else {
            self.close_assembly();
            self.switch_part_to(item);
        }
    }

    pub(super) fn paint_assembly(&self, painter: &mut Painter, layout: Layout) {
        let Some(view) = &self.assembly else {
            return;
        };
        painter.rect(layout.viewport, [0.065, 0.085, 0.115, 1.0]);
        if let Some((scene, projector)) = self.assembly_projector(layout) {
            let mut faces = Vec::new();
            for (index, &[a, b, c]) in scene.triangles.iter().enumerate() {
                let Some(normal) = triangle_normal(a, b, c) else {
                    continue;
                };
                let facing = projector.facing_at(normal, a);
                if facing <= 0.001 {
                    continue;
                }
                let instance = scene.triangle_body[index];
                let base = if view.selected == Some(instance) {
                    [0.35, 1.0, 0.75]
                } else {
                    INSTANCE_COLORS[instance % INSTANCE_COLORS.len()]
                };
                let mut shade = (0.35 + facing * 0.16).clamp(0.22, 0.55);
                if view.hovered == Some(instance) {
                    shade += 0.2;
                }
                let color = [base[0] * shade, base[1] * shade, base[2] * shade, 1.0];
                let mid = std::array::from_fn(|axis| (a[axis] + b[axis] + c[axis]) / 3.0);
                faces.push((
                    projector.depth(mid),
                    [a, b, c].map(|p| projector.screen(p)),
                    color,
                ));
            }
            faces.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, points, color) in faces {
                painter.triangle(points[0], points[1], points[2], color);
            }
            paint_visible_edges(painter, &scene, &projector);
            if let Some(edge) = &view.hovered_edge {
                let (s, e) = (projector.screen(edge.a), projector.screen(edge.b));
                painter.line(s[0], s[1], e[0], e[1], 3.0, FACE_HOVER);
            }
            if let Some(pick) = view.pick_a {
                let transform = view.asm.instances[pick.instance].transform();
                let p = transform.apply(pick.point).map(|c| c as f32);
                let n = transform.pose().vector(pick.normal).map(|c| c as f32);
                let tip =
                    std::array::from_fn(|i| p[i] + n[i] * 8.0 * projector.units_per_pixel() * 4.0);
                let (s, e) = (projector.screen(p), projector.screen(tip));
                painter.line(s[0], s[1], e[0], e[1], 2.5, FACE_HOVER);
            }
        }
        let viewport = layout.viewport;
        painter.text(
            viewport.x + 18.0,
            viewport.y + 18.0,
            &format!(
                "ASSEMBLY / {} / {} INSTANCE{}",
                view.asm.name.to_uppercase(),
                view.asm.instances.len(),
                if view.asm.instances.len() == 1 {
                    ""
                } else {
                    "S"
                }
            ),
            1.3,
            TEXT,
        );
        painter.text(
            viewport.x + 18.0,
            viewport.y + 43.0,
            "CLICK SELECT / I INSERT / J MATE (EMPTY: GROUND) / DEL DELETE / CTRL+B PART",
            1.0,
            MUTED,
        );
        if let Some(reason) = &view.asm.read_only_reason {
            painter.text(
                viewport.x + 18.0,
                viewport.y + 62.0,
                &clip_text(&format!("READ ONLY: {reason}"), 90, false),
                1.0,
                CONFLICT,
            );
        }
        let prompt_lines: Vec<String> = match &view.prompt {
            Some(AsmPrompt::Insert) => std::iter::once("INSERT PART:".to_string())
                .chain(
                    self.project
                        .as_ref()
                        .map(ProjectFile::parts)
                        .unwrap_or_default()
                        .iter()
                        .take(9)
                        .enumerate()
                        .map(|(i, (_, name))| format!("{}  {}", i + 1, name)),
                )
                .collect(),
            Some(AsmPrompt::MateKind { .. }) => std::iter::once("MATE KIND:".to_string())
                .chain(
                    MateKind::ALL
                        .iter()
                        .enumerate()
                        .map(|(i, kind)| format!("{}  {}", i + 1, kind.label())),
                )
                .collect(),
            _ => Vec::new(),
        };
        if !prompt_lines.is_empty() {
            let width = prompt_lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0) as f32
                * 8.0
                + 24.0;
            let rect = Rect {
                x: viewport.x + 18.0,
                y: viewport.y + 84.0,
                w: width,
                h: prompt_lines.len() as f32 * 18.0 + 12.0,
            };
            painter.rect(rect, PANEL_LIGHT);
            painter.outline(rect, 1.0, ACCENT);
            for (i, line) in prompt_lines.iter().enumerate() {
                painter.text(
                    rect.x + 12.0,
                    rect.y + 9.0 + i as f32 * 18.0,
                    line,
                    1.0,
                    if i == 0 { ACCENT } else { TEXT },
                );
            }
        }
    }

    /// Top bar, Project panel, Properties panel and status line of the
    /// assembly view.
    pub(super) fn paint_assembly_panels(&self, painter: &mut Painter, layout: Layout) {
        let Some(view) = &self.assembly else {
            return;
        };
        painter.rect(layout.top, PANEL);
        painter.rect(layout.left, PANEL);
        painter.rect(layout.right, PANEL);
        painter.rect(layout.bottom, PANEL);
        painter.line(0.0, 94.0, layout.top.w, 94.0, 1.0, BORDER);
        painter.line(
            layout.left.w,
            94.0,
            layout.left.w,
            layout.bottom.y,
            1.0,
            BORDER,
        );
        painter.line(
            layout.right.x,
            94.0,
            layout.right.x,
            layout.bottom.y,
            1.0,
            BORDER,
        );
        painter.text(20.0, 13.0, "PUPPYCAD", 2.0, TEXT);
        self.paint_breadcrumbs(painter, layout);
        for (perspective, label) in [(true, "PERSPECTIVE"), (false, "ORTHO")] {
            let button = layout.projection_button(perspective);
            let active = self.solid_view.perspective == perspective;
            painter.rect(button, if active { ACTIVE } else { PANEL_LIGHT });
            painter.outline(button, 1.0, if active { ACCENT } else { BORDER });
            painter.text(
                button.x + 10.0,
                button.y + 8.0,
                label,
                1.4,
                if active { ACCENT } else { TEXT },
            );
        }
        let more = layout.tool_button(9);
        painter.rect(more, if self.more_open { ACTIVE } else { PANEL_LIGHT });
        painter.outline(more, 1.0, if self.more_open { ACCENT } else { BORDER });
        painter.text(more.x + 22.0, more.y + 12.0, "MORE [M]", 1.0, TEXT);
        // Project panel: parts and assemblies, then this assembly's instances.
        painter.text(17.0, layout.left.y + 19.0, "PROJECT", 1.5, TEXT);
        let rows = self.project_rows();
        let fits = |row: Rect| row.y + row.h < layout.bottom.y;
        if let Some(project) = &self.project {
            for (index, item) in rows.iter().enumerate() {
                let row = layout.part_row(index);
                if !fits(row) {
                    break;
                }
                let active = *item == view.item;
                if active {
                    painter.rect(row, ACTIVE_ROW);
                }
                let kind = if project.assembly_indices().contains(item) {
                    "ASM"
                } else {
                    "PART"
                };
                painter.text(
                    row.x + 9.0,
                    row.y + 9.0,
                    &format!(
                        "{} {kind} / {}",
                        if active { ">" } else { "v" },
                        clip_text(project.item_name_at(*item).unwrap_or("?"), 13, false)
                    ),
                    1.0,
                    if active { TEXT } else { MUTED },
                );
            }
        }
        let header = layout.part_row(rows.len());
        if fits(header) {
            painter.text(header.x, header.y + 12.0, "INSTANCES", 1.1, MUTED);
        }
        for (index, instance) in view.asm.instances.iter().enumerate() {
            let row = layout.part_row(rows.len() + 1 + index);
            if !fits(row) {
                break;
            }
            if view.selected == Some(index) {
                painter.rect(row, ACTIVE_ROW);
            }
            painter.text(
                row.x + 9.0,
                row.y + 9.0,
                &clip_text(instance.label(), 24, false),
                1.0,
                if view.selected == Some(index) {
                    TEXT
                } else {
                    MUTED
                },
            );
        }
        // Properties.
        painter.text(
            layout.right.x + 17.0,
            layout.right.y + 19.0,
            "PROPERTIES",
            1.5,
            TEXT,
        );
        if let Some(instance) = view.selected.and_then(|i| view.asm.instances.get(i)) {
            let part = self
                .project
                .as_ref()
                .and_then(|project| {
                    project
                        .parts()
                        .into_iter()
                        .find(|(id, _)| *id == instance.part_id)
                        .map(|(_, name)| name)
                })
                .unwrap_or_else(|| instance.part_id.clone());
            let transform = instance.transform();
            let lines = [("NAME", instance.label().to_string()), ("PART", part)];
            for (index, (label, value)) in lines.iter().enumerate() {
                let row = assembly_property_row(layout, index);
                painter.text(row.x, row.y + 9.0, label, 1.0, MUTED);
                painter.text(
                    row.x + 60.0,
                    row.y + 9.0,
                    &clip_text(value, 22, false),
                    1.0,
                    TEXT,
                );
            }
            for (offset, field) in AsmField::ALL.into_iter().enumerate() {
                let row = assembly_property_row(layout, offset + 2);
                let editing = match &view.prompt {
                    Some(AsmPrompt::Field { field: f, buffer }) if *f == field => Some(buffer),
                    _ => None,
                };
                painter.rect(
                    row,
                    if editing.is_some() {
                        ACTIVE
                    } else {
                        PANEL_LIGHT
                    },
                );
                painter.outline(row, 1.0, if editing.is_some() { ACCENT } else { BORDER });
                painter.text(row.x + 8.0, row.y + 9.0, field.label(), 1.0, MUTED);
                let value = match editing {
                    Some(buffer) => format!("{buffer}_"),
                    None => format!(
                        "{:.2}{}",
                        Some(field.get(&transform))
                            .map(|v| if v.abs() < 0.005 { 0.0 } else { v })
                            .unwrap_or_default(),
                        if matches!(field, AsmField::X | AsmField::Y | AsmField::Z) {
                            " mm"
                        } else {
                            " deg"
                        }
                    ),
                };
                painter.text(row.x + 60.0, row.y + 9.0, &value, 1.0, TEXT);
            }
            let mates = view.asm.mates_of(view.selected.unwrap_or(0));
            let header = assembly_property_row(layout, 8);
            painter.text(
                header.x,
                header.y + 12.0,
                &format!("MATES ({})", mates.len()),
                1.1,
                MUTED,
            );
            let servos: HashMap<String, f64> = view.asm.servo_angles().into_iter().collect();
            for (index, mate) in mates.iter().enumerate() {
                let row = assembly_mate_row(layout, index);
                if !fits(row) {
                    break;
                }
                let chosen = view.selected_mate.as_deref() == Some(mate.id.as_str());
                if chosen {
                    painter.rect(row, ACTIVE_ROW);
                    painter.outline(row, 1.0, ACCENT);
                }
                let other = [&mate.a, &mate.b]
                    .into_iter()
                    .find(|r| r.instance_id.as_deref() != Some(instance.id.as_str()))
                    .map(|r| {
                        r.instance_id
                            .as_deref()
                            .and_then(|id| view.asm.instance(id))
                            .map_or("WORLD", |i| i.label())
                    })
                    .unwrap_or("?");
                painter.text(
                    row.x + 6.0,
                    row.y + 9.0,
                    &clip_text(&format!("{} - {other}", mate.kind.label()), 20, false),
                    1.0,
                    TEXT,
                );
                if mate.kind == MateKind::Revolute {
                    let value = servo_value_rect(row);
                    let editing = match &view.prompt {
                        Some(AsmPrompt::Servo { mate_id, buffer }) if *mate_id == mate.id => {
                            Some(buffer)
                        }
                        _ => None,
                    };
                    painter.rect(
                        value,
                        if editing.is_some() {
                            ACTIVE
                        } else {
                            PANEL_LIGHT
                        },
                    );
                    painter.outline(value, 1.0, if editing.is_some() { ACCENT } else { BORDER });
                    let text = match (editing, servos.get(&mate.id)) {
                        (Some(buffer), _) => format!("{buffer}_"),
                        (None, Some(deg)) => format!("{deg:.0}DEG"),
                        (None, None) => "-- DEG".into(),
                    };
                    painter.text(value.x + 6.0, value.y + 7.0, &text, 1.0, TEXT);
                }
            }
        } else {
            let row = assembly_property_row(layout, 0);
            painter.text(row.x, row.y + 9.0, "CLICK AN INSTANCE", 1.0, MUTED);
        }
        let room = ((layout.top.w - 140.0) / 9.6).max(0.0) as usize;
        painter.text(
            15.0,
            layout.bottom.y + 10.0,
            &clip_text(&self.status_message, room, false),
            1.2,
            MUTED,
        );
        painter.text(
            layout.top.w - 110.0,
            layout.bottom.y + 10.0,
            &format!("{:.0}%", self.solid_view.zoom * 100.0),
            1.2,
            TEXT,
        );
    }

    /// Instance name / selected instance crumbs.
    pub(super) fn assembly_breadcrumbs(&self) -> Option<Vec<(Crumb, String)>> {
        let view = self.assembly.as_ref()?;
        let mut segments = vec![(Crumb::Part, view.asm.name.clone())];
        if let Some(instance) = view.selected.and_then(|i| view.asm.instances.get(i)) {
            segments.push((Crumb::Shape, instance.label().to_string()));
        }
        Some(segments)
    }
}

/// Row of the `index`th mate in the Properties mate list.
pub(super) fn assembly_mate_row(layout: Layout, index: usize) -> Rect {
    assembly_property_row(layout, 9 + index)
}

/// The clickable servo DEG value at the right of a revolute mate row.
pub(super) fn servo_value_rect(row: Rect) -> Rect {
    Rect {
        x: row.x + row.w - 74.0,
        y: row.y + 3.0,
        w: 70.0,
        h: row.h - 6.0,
    }
}

/// Number-entry key handling; true on Enter.
fn edit_number(buffer: &mut String, code: KeyCode, text: Option<&str>) -> bool {
    match code {
        KeyCode::Enter | KeyCode::NumpadEnter => return true,
        KeyCode::Backspace => {
            buffer.pop();
        }
        _ => {
            for ch in text.unwrap_or("").chars() {
                if (ch.is_ascii_digit() || ch == '.' || (ch == '-' && buffer.is_empty()))
                    && buffer.len() < 24
                {
                    buffer.push(ch);
                }
            }
        }
    }
    false
}

/// Row `index` of the assembly Properties panel.
pub(super) fn assembly_property_row(layout: Layout, index: usize) -> Rect {
    Rect {
        x: layout.right.x + 14.0,
        y: layout.right.y + 43.0 + index as f32 * 33.0,
        w: layout.right.w - 28.0,
        h: 29.0,
    }
}
