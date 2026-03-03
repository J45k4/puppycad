use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use puppycad_core::{NodeGraph, NodeGraphEdge, build_node_graph, parser::parse_pcad};

const NODE_WIDTH: f32 = 2.8;
const NODE_HEIGHT: f32 = 1.2;
const NODE_DEPTH: f32 = 0.22;
const EDGE_THICKNESS: f32 = 0.1;

enum EditorEvent {
    LoadGraph(NodeGraph),
}

struct NodeGraphApp {
    camera_position: Option<[f32; 3]>,
    camera_look_at: Option<[f32; 3]>,
    screenshot_path: Option<String>,
    screenshot_frame: u64,
    next_frame: u64,
    screenshot_requested: bool,
    free_fly_controller: pge::FreeFlyController,
    right_button_down: bool,
    move_left: bool,
    move_right: bool,
    move_forward_w: bool,
    move_forward_f: bool,
    move_backward: bool,
    move_up: bool,
    move_down: bool,
    move_fast: bool,
    rotate_left: bool,
    rotate_right: bool,
    rotate_up: bool,
    rotate_down: bool,
    scene_id: Option<pge::ArenaId<pge::Scene>>,
    mesh_node_ids: Vec<pge::ArenaId<pge::Node>>,
    mesh_ids: Vec<pge::ArenaId<pge::Mesh>>,
    camera_id: Option<pge::ArenaId<pge::Camera>>,
    camera_node_id: Option<pge::ArenaId<pge::Node>>,
    light_node_ids: Vec<pge::ArenaId<pge::Node>>,
    point_light_ids: Vec<pge::ArenaId<pge::PointLight>>,
    material_ids: Vec<pge::ArenaId<pge::Material>>,
    window_id: Option<pge::ArenaId<pge::Window>>,
    gui_id: Option<pge::ArenaId<pge::GUIElement>>,
}

impl Default for NodeGraphApp {
    fn default() -> Self {
        Self {
            camera_position: None,
            camera_look_at: None,
            screenshot_path: None,
            screenshot_frame: 0,
            next_frame: 0,
            screenshot_requested: false,
            free_fly_controller: pge::FreeFlyController::default(),
            right_button_down: false,
            move_left: false,
            move_right: false,
            move_forward_w: false,
            move_forward_f: false,
            move_backward: false,
            move_up: false,
            move_down: false,
            move_fast: false,
            rotate_left: false,
            rotate_right: false,
            rotate_up: false,
            rotate_down: false,
            scene_id: None,
            mesh_node_ids: Vec::new(),
            mesh_ids: Vec::new(),
            camera_id: None,
            camera_node_id: None,
            light_node_ids: Vec::new(),
            point_light_ids: Vec::new(),
            material_ids: Vec::new(),
            window_id: None,
            gui_id: None,
        }
    }
}

impl NodeGraphApp {
    fn clear_previous_scene(&mut self, state: &mut pge::State) {
        for node_id in self.mesh_node_ids.drain(..) {
            state.nodes.remove(&node_id);
        }
        for mesh_id in self.mesh_ids.drain(..) {
            state.meshes.remove(&mesh_id);
        }
        for material_id in self.material_ids.drain(..) {
            state.materials.remove(&material_id);
        }
        if let Some(camera_node_id) = self.camera_node_id.take() {
            state.nodes.remove(&camera_node_id);
        }
        for light_node_id in self.light_node_ids.drain(..) {
            state.nodes.remove(&light_node_id);
        }
        for point_light_id in self.point_light_ids.drain(..) {
            state.point_lights.remove(&point_light_id);
        }
        if let Some(camera_id) = self.camera_id.take() {
            state.cameras.remove(&camera_id);
        }
        if let Some(gui_id) = self.gui_id.take() {
            state.guis.remove(&gui_id);
        }
        if let Some(window_id) = self.window_id.take() {
            state.windows.remove(&window_id);
        }
        if let Some(scene_id) = self.scene_id.take() {
            state.scenes.remove(&scene_id);
        }
    }

    fn load_graph(&mut self, graph: NodeGraph, state: &mut pge::State) {
        self.clear_previous_scene(state);

        let edge_material_id = {
            let mut material = pge::Material::default();
            material.base_color_factor = [0.58, 0.65, 0.73, 1.0];
            material.roughness_factor = 0.95;
            state.materials.insert(material)
        };
        let solid_material_id = {
            let mut material = pge::Material::default();
            material.base_color_factor = [0.18, 0.45, 0.82, 1.0];
            material.roughness_factor = 0.8;
            state.materials.insert(material)
        };
        let feature_material_id = {
            let mut material = pge::Material::default();
            material.base_color_factor = [0.95, 0.47, 0.15, 1.0];
            material.roughness_factor = 0.82;
            state.materials.insert(material)
        };
        self.material_ids = vec![edge_material_id, solid_material_id, feature_material_id];

        let scene_id = state.scenes.insert(pge::Scene::new());
        self.scene_id = Some(scene_id);

        let position_by_id: HashMap<&str, [f32; 2]> = graph
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.position))
            .collect();

        if let Some(edge_mesh) = build_edge_mesh(&graph.edges, &position_by_id, edge_material_id) {
            let edge_mesh_id = state.meshes.insert(edge_mesh);
            let mut edge_node = pge::Node::new();
            edge_node.name = Some("dependencies".to_owned());
            edge_node.parent = pge::NodeParent::Scene(scene_id);
            edge_node.mesh = Some(edge_mesh_id);
            edge_node.global_transform = edge_node.matrix();
            let edge_node_id = state.nodes.insert(edge_node);
            self.mesh_ids.push(edge_mesh_id);
            self.mesh_node_ids.push(edge_node_id);
        }

        for node in &graph.nodes {
            let material_id = match node.kind {
                puppycad_core::DeclKind::Solid => solid_material_id,
                puppycad_core::DeclKind::Feature => feature_material_id,
            };
            let mesh = build_box_mesh(NODE_WIDTH, NODE_HEIGHT, NODE_DEPTH, material_id);
            let mesh_id = state.meshes.insert(mesh);
            let mut graph_node = pge::Node::new();
            graph_node.name = Some(format!("{} {}", node.id, node.op));
            graph_node.parent = pge::NodeParent::Scene(scene_id);
            graph_node.translation = pge::Vec3::new(node.position[0], node.position[1], 0.0);
            graph_node.mesh = Some(mesh_id);
            graph_node.global_transform = graph_node.matrix();
            let graph_node_id = state.nodes.insert(graph_node);
            self.mesh_ids.push(mesh_id);
            self.mesh_node_ids.push(graph_node_id);
        }

        let scene_bounding_box = state.get_scene_bounding_box(scene_id);
        let center = (scene_bounding_box.min + scene_bounding_box.max) * 0.5;
        let size = scene_bounding_box.max - scene_bounding_box.min;
        let max_size = size.x.abs().max(size.y.abs()).max(size.z.abs());

        let fov_degrees = 55.0_f32;
        let fov_radians = fov_degrees.to_radians();
        let distance = if max_size > 0.0 {
            (max_size / 2.0) / (fov_radians * 0.5).tan()
        } else {
            8.0
        };

        let target = self
            .camera_look_at
            .unwrap_or([center.x, center.y, center.z]);
        let default_camera_position = [center.x, center.y, center.z + distance.max(0.5)];
        let camera_position = self.camera_position.unwrap_or(default_camera_position);
        let target = pge::Vec3::new(target[0], target[1], target[2]);
        let camera_position =
            pge::Vec3::new(camera_position[0], camera_position[1], camera_position[2]);

        let mut free_fly_controller = pge::FreeFlyController::default();
        free_fly_controller.set_from_target_and_position(target, camera_position);

        let light_distance = max_size.max(2.0) * 1.2;
        let mut add_light = |name: &str, position: [f32; 3], color: [f32; 3], intensity: f32| {
            let mut light_node = pge::Node::new();
            light_node.name = Some(name.to_owned());
            light_node.translation = pge::Vec3::new(position[0], position[1], position[2]);
            light_node.parent = pge::NodeParent::Scene(scene_id);
            let light_node_id = state.nodes.insert(light_node);
            self.light_node_ids.push(light_node_id);

            let mut point_light = pge::PointLight::new();
            point_light.node_id = Some(light_node_id);
            point_light.color = color;
            point_light.intensity = intensity;
            let point_light_id = state.point_lights.insert(point_light);
            self.point_light_ids.push(point_light_id);
        };

        let view_dir = (target - camera_position).normalize_or_zero();
        let world_up = pge::Vec3::new(0.0, 1.0, 0.0);
        let mut right = view_dir.cross(world_up);
        if right.length_squared() <= f32::EPSILON {
            right = pge::Vec3::new(1.0, 0.0, 0.0);
        }
        right = right.normalize_or_zero();
        let up = right.cross(view_dir).normalize_or_zero();

        let key_position = target - (view_dir * light_distance)
            + (right * (light_distance * 0.45))
            + (up * (light_distance * 0.5));
        let fill_position = target + (view_dir * (light_distance * 0.2))
            - (right * (light_distance * 0.8))
            + (up * (light_distance * 0.2));

        add_light(
            "NodeKeyLight",
            [key_position.x, key_position.y, key_position.z],
            [1.0, 1.0, 1.0],
            1.8,
        );
        add_light(
            "NodeFillLight",
            [fill_position.x, fill_position.y, fill_position.z],
            [1.0, 1.0, 1.0],
            0.9,
        );

        let mut camera_node = pge::Node::new();
        camera_node.parent = pge::NodeParent::Scene(scene_id);
        camera_node.translation = camera_position;
        camera_node.looking_at(target.x, target.y, target.z);
        let camera_node_id = state.nodes.insert(camera_node);
        self.camera_node_id = Some(camera_node_id);

        let mut camera = pge::Camera::new();
        camera.fovy = fov_radians;
        camera.node_id = Some(camera_node_id);
        let camera_id = state.cameras.insert(camera);
        self.camera_id = Some(camera_id);

        let gui_id = state.guis.insert(pge::camera_view(camera_id));
        self.gui_id = Some(gui_id);
        let window_id = state
            .windows
            .insert(pge::Window::new().title("PuppyCAD Node Editor").ui(gui_id));
        self.window_id = Some(window_id);

        self.free_fly_controller = free_fly_controller;
        self.clear_input_state();
        self.next_frame = 0;
        self.screenshot_requested = false;
    }

    fn clear_input_state(&mut self) {
        self.right_button_down = false;
        self.move_left = false;
        self.move_right = false;
        self.move_forward_w = false;
        self.move_forward_f = false;
        self.move_backward = false;
        self.move_up = false;
        self.move_down = false;
        self.move_fast = false;
        self.rotate_left = false;
        self.rotate_right = false;
        self.rotate_up = false;
        self.rotate_down = false;
    }

    fn on_keyboard_state_change(&mut self, key: pge::KeyboardKey, action: pge::KeyAction) {
        let pressed = matches!(action, pge::KeyAction::Pressed);
        match key {
            pge::KeyboardKey::ControlLeft => self.move_down = pressed,
            pge::KeyboardKey::A => self.move_left = pressed,
            pge::KeyboardKey::D => self.move_right = pressed,
            pge::KeyboardKey::W => self.move_forward_w = pressed,
            pge::KeyboardKey::F => self.move_forward_f = pressed,
            pge::KeyboardKey::S => self.move_backward = pressed,
            pge::KeyboardKey::Space => self.move_up = pressed,
            pge::KeyboardKey::ShiftLeft => self.move_fast = pressed,
            pge::KeyboardKey::Left => self.rotate_left = pressed,
            pge::KeyboardKey::Right => self.rotate_right = pressed,
            pge::KeyboardKey::Up => self.rotate_up = pressed,
            pge::KeyboardKey::Down => self.rotate_down = pressed,
            _ => {}
        }
    }

    fn keyboard_move_input(&self) -> pge::FreeFlyMoveInput {
        let mut input = pge::FreeFlyMoveInput::default();
        if self.move_left {
            input.right -= 1.0;
        }
        if self.move_right {
            input.right += 1.0;
        }
        if self.move_forward_w || self.move_forward_f {
            input.forward += 1.0;
        }
        if self.move_backward {
            input.forward -= 1.0;
        }
        if self.move_up {
            input.up += 1.0;
        }
        if self.move_down {
            input.up -= 1.0;
        }
        input.fast = self.move_fast;
        input
    }

    fn keyboard_look_input(&self) -> (f32, f32) {
        let mut yaw = 0.0;
        let mut pitch = 0.0;
        if self.rotate_left {
            yaw -= 1.0;
        }
        if self.rotate_right {
            yaw += 1.0;
        }
        if self.rotate_up {
            pitch -= 1.0;
        }
        if self.rotate_down {
            pitch += 1.0;
        }
        (yaw, pitch)
    }
}

impl pge::App<EditorEvent> for NodeGraphApp {
    fn on_create(&mut self, _state: &mut pge::State) {}

    fn on_event(&mut self, event: EditorEvent, state: &mut pge::State) {
        match event {
            EditorEvent::LoadGraph(graph) => self.load_graph(graph, state),
        }
    }

    fn on_mouse_input(
        &mut self,
        window_id: pge::ArenaId<pge::Window>,
        event: pge::MouseEvent,
        _state: &mut pge::State,
    ) {
        let Some(active_window_id) = self.window_id else {
            return;
        };
        if active_window_id != window_id {
            return;
        }

        match event {
            pge::MouseEvent::Moved { dx, dy } => {
                if self.right_button_down {
                    self.free_fly_controller.look_mouse(pge::Vec2::new(dx, dy));
                }
            }
            pge::MouseEvent::Pressed { button } => {
                if matches!(button, pge::MouseButton::Right) {
                    self.right_button_down = true;
                }
            }
            pge::MouseEvent::Released { button } => {
                if matches!(button, pge::MouseButton::Right) {
                    self.right_button_down = false;
                }
            }
            pge::MouseEvent::Wheel { .. } => {}
        }
    }

    fn on_keyboard_input(
        &mut self,
        window_id: pge::ArenaId<pge::Window>,
        key: pge::KeyboardKey,
        action: pge::KeyAction,
        _state: &mut pge::State,
    ) {
        let Some(active_window_id) = self.window_id else {
            return;
        };
        if active_window_id != window_id {
            return;
        }
        self.on_keyboard_state_change(key, action);
    }

    fn on_process(&mut self, state: &mut pge::State, delta: f32) {
        if let Some(camera_node_id) = self.camera_node_id {
            self.free_fly_controller
                .move_local(self.keyboard_move_input(), delta);
            let (yaw, pitch) = self.keyboard_look_input();
            self.free_fly_controller.look_keyboard(yaw, pitch, delta);
            self.free_fly_controller
                .apply_to_node(state, camera_node_id);
        }

        if self.screenshot_path.is_none() || self.screenshot_requested {
            return;
        }

        if self.next_frame >= self.screenshot_frame {
            let Some(window_id) = self.window_id else {
                return;
            };
            if let Some(path) = self.screenshot_path.clone() {
                state.screenshot_request = Some((window_id, path));
                self.screenshot_requested = true;
            }
        }

        self.next_frame = self.next_frame.saturating_add(1);
    }
}

fn build_box_mesh(
    width: f32,
    height: f32,
    depth: f32,
    material_id: pge::ArenaId<pge::Material>,
) -> pge::Mesh {
    let hx = width * 0.5;
    let hy = height * 0.5;
    let hz = depth * 0.5;
    let vertices = vec![
        [-hx, -hy, -hz],
        [hx, -hy, -hz],
        [hx, hy, -hz],
        [-hx, hy, -hz],
        [-hx, -hy, hz],
        [hx, -hy, hz],
        [hx, hy, hz],
        [-hx, hy, hz],
    ];
    let indices: Vec<u16> = vec![
        0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6, 0, 4, 5, 0, 5, 1, 1, 5, 6, 1, 6, 2, 2, 6, 7, 2, 7, 3,
        3, 7, 4, 3, 4, 0,
    ];
    let mut primitive = pge::Primitive::new(pge::PrimitiveTopology::TriangleList);
    primitive.vertices = vertices.clone();
    primitive.normals = vec![[0.0, 0.0, 1.0]; vertices.len()];
    primitive.tex_coords = vec![[0.0, 0.0]; vertices.len()];
    primitive.indices = indices;
    primitive.material = Some(material_id);

    let mut mesh = pge::Mesh::new();
    mesh.primitives.push(primitive);
    mesh
}

fn build_edge_mesh(
    edges: &[NodeGraphEdge],
    positions: &HashMap<&str, [f32; 2]>,
    material_id: pge::ArenaId<pge::Material>,
) -> Option<pge::Mesh> {
    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut tex_coords: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    for edge in edges {
        let Some(from) = positions.get(edge.from.as_str()) else {
            continue;
        };
        let Some(to) = positions.get(edge.to.as_str()) else {
            continue;
        };

        let start = [from[0] + NODE_WIDTH * 0.5, from[1], -0.04];
        let end = [to[0] - NODE_WIDTH * 0.5, to[1], -0.04];
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let length = (dx * dx + dy * dy).sqrt();
        if length < 0.01 {
            continue;
        }
        let nx = (-dy / length) * EDGE_THICKNESS * 0.5;
        let ny = (dx / length) * EDGE_THICKNESS * 0.5;

        let base = match u16::try_from(vertices.len()) {
            Ok(value) => value,
            Err(_) => break,
        };

        vertices.push([start[0] + nx, start[1] + ny, start[2]]);
        vertices.push([start[0] - nx, start[1] - ny, start[2]]);
        vertices.push([end[0] - nx, end[1] - ny, end[2]]);
        vertices.push([end[0] + nx, end[1] + ny, end[2]]);
        for _ in 0..4 {
            normals.push([0.0, 0.0, 1.0]);
            tex_coords.push([0.0, 0.0]);
        }
        indices.extend_from_slice(&[
            base,
            base.saturating_add(1),
            base.saturating_add(2),
            base,
            base.saturating_add(2),
            base.saturating_add(3),
        ]);
    }

    if vertices.is_empty() {
        return None;
    }

    let mut primitive = pge::Primitive::new(pge::PrimitiveTopology::TriangleList);
    primitive.vertices = vertices;
    primitive.normals = normals;
    primitive.tex_coords = tex_coords;
    primitive.indices = indices;
    primitive.material = Some(material_id);

    let mut mesh = pge::Mesh::new();
    mesh.primitives.push(primitive);
    Some(mesh)
}

fn read_source(path: Option<&Path>) -> Result<String, String> {
    match path {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read '{}': {err}", path.to_string_lossy())),
        None => Ok(String::new()),
    }
}

fn parse_screenshot_path() -> Option<String> {
    std::env::var("PUPPYCAD_EDITOR_SCREENSHOT").ok()
}

pub fn run_node_editor(input_path: Option<PathBuf>) -> ExitCode {
    let source = match read_source(input_path.as_deref()) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let ast = match parse_pcad(&source) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let graph = build_node_graph(&ast);
    println!(
        "Loaded declaration graph: {} node(s), {} edge(s)",
        graph.nodes.len(),
        graph.edges.len()
    );

    let mut app = NodeGraphApp::default();
    app.screenshot_path = parse_screenshot_path();

    let app_result = pge::run_with_event_sender(app, |sender| {
        let _ = sender.send(EditorEvent::LoadGraph(graph));
    });
    match app_result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("failed to open node editor: {err}");
            ExitCode::FAILURE
        }
    }
}
