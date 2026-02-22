use std::process::ExitCode;
use std::path::Path;

use crate::{
	args::{ParseArgs, ParseOutput, RenderArgs, ValidateArgs},
	read_source,
};
use puppycad_core::{
	build_model_state, codegen, eval::Evaluator, parser::parse_pcad,
	build_render_state,
};

enum RenderEvent {
	LoadRenderState(puppycad_core::RenderState),
	RequestScreenshot(String),
}

struct RenderStateApp {
	camera_position: Option<[f32; 3]>,
	camera_look_at: Option<[f32; 3]>,
	screenshot_path: Option<String>,
	screenshot_frame: u64,
	next_frame: u64,
	screenshot_requested: bool,
	orbit_controller: pge::OrbitController,
	right_button_down: bool,
	middle_button_down: bool,
	scene_id: Option<pge::ArenaId<pge::Scene>>,
	mesh_node_ids: Vec<pge::ArenaId<pge::Node>>,
	mesh_ids: Vec<pge::ArenaId<pge::Mesh>>,
	camera_id: Option<pge::ArenaId<pge::Camera>>,
	camera_node_id: Option<pge::ArenaId<pge::Node>>,
	light_node_id: Option<pge::ArenaId<pge::Node>>,
	window_id: Option<pge::ArenaId<pge::Window>>,
	gui_id: Option<pge::ArenaId<pge::GUIElement>>,
}

impl RenderStateApp {
	fn new(
		camera_position: Option<[f32; 3]>,
		camera_look_at: Option<[f32; 3]>,
		screenshot_path: Option<String>,
	) -> Self {
		Self {
			camera_position,
			camera_look_at,
			screenshot_path,
			screenshot_frame: 0,
			next_frame: 0,
			screenshot_requested: false,
			orbit_controller: pge::OrbitController::default(),
			right_button_down: false,
			middle_button_down: false,
			scene_id: None,
			mesh_node_ids: Vec::new(),
			mesh_ids: Vec::new(),
			camera_id: None,
			camera_node_id: None,
			light_node_id: None,
			window_id: None,
			gui_id: None,
		}
	}

	fn clear_previous_scene(&mut self, state: &mut pge::State) {
		for node_id in self.mesh_node_ids.drain(..) {
			state.nodes.remove(&node_id);
		}
		for mesh_id in self.mesh_ids.drain(..) {
			state.meshes.remove(&mesh_id);
		}
		if let Some(camera_node_id) = self.camera_node_id.take() {
			state.nodes.remove(&camera_node_id);
		}
		if let Some(light_node_id) = self.light_node_id.take() {
			state.nodes.remove(&light_node_id);
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

	fn load_render_state(&mut self, render_state: puppycad_core::RenderState, state: &mut pge::State) {
		self.clear_previous_scene(state);

		let scene_id = state.scenes.insert(pge::Scene::new());
		self.scene_id = Some(scene_id);

		for mesh in render_state.meshes {
			let mut node = pge::Node::new();
			node.name = Some(mesh.decl_id.clone());
			node.parent = pge::NodeParent::Scene(scene_id);

			let mut pge_mesh = pge::Mesh::new();
			let mut primitive = pge::Primitive::new(pge::PrimitiveTopology::TriangleList);
			primitive.vertices = mesh.positions;
			primitive.indices = to_u16_indices(&mesh.indices, &mesh.decl_id);
			let mut normals = mesh.normals;
			if normals.len() != primitive.vertices.len() {
				normals.resize(primitive.vertices.len(), [0.0, 0.0, 1.0]);
			}
			primitive.normals = normals;
			primitive.tex_coords = vec![[0.0, 0.0]; primitive.vertices.len()];
			pge_mesh.primitives.push(primitive);
			let mesh_id = state.meshes.insert(pge_mesh);

			node.mesh = Some(mesh_id);
			node.global_transform = node.matrix();
			let node_id = state.nodes.insert(node);
			self.mesh_ids.push(mesh_id);
			self.mesh_node_ids.push(node_id);
		}

		let scene_bounding_box = state.get_scene_bounding_box(scene_id);
		let center = (scene_bounding_box.min + scene_bounding_box.max) * 0.5;
		let size = scene_bounding_box.max - scene_bounding_box.min;
		let max_size = size.x.abs().max(size.y.abs()).max(size.z.abs());

		let fov_degrees = 60.0_f32;
		let fov_radians = fov_degrees.to_radians();
		let distance = if max_size > 0.0 {
			(max_size / 2.0) / fov_radians.tan()
		} else {
			3.0
		};
		let target = self
			.camera_look_at
			.unwrap_or([center.x, center.y, center.z]);
		let default_camera_position = [center.x, center.y, center.z + distance.max(0.1)];
		let camera_position = self.camera_position.unwrap_or(default_camera_position);
		let target = pge::Vec3::new(target[0], target[1], target[2]);
		let camera_position = pge::Vec3::new(
			camera_position[0],
			camera_position[1],
			camera_position[2],
		);
		let mut orbit_controller = pge::OrbitController::default();
		orbit_controller.set_from_target_and_position(target, camera_position);

		let mut light_node = pge::Node::new();
		light_node.name = Some("Light".to_string());
		light_node.translation = pge::Vec3::new(0.0, 5.0, -5.0);
		light_node.parent = pge::NodeParent::Scene(scene_id);
		let light_node_id = state.nodes.insert(light_node);
		let mut light = pge::PointLight::new();
		light.node_id = Some(light_node_id);
		state.point_lights.insert(light);
		self.light_node_id = Some(light_node_id);

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
		let window_id = state.windows.insert(pge::Window::new().title("render").ui(gui_id));
		self.window_id = Some(window_id);

		self.orbit_controller = orbit_controller;
		self.right_button_down = false;
		self.middle_button_down = false;

		self.next_frame = 0;
		self.screenshot_requested = false;
	}
}

fn to_u16_indices(indices: &[u32], mesh_name: &str) -> Vec<u16> {
	let mut out = Vec::with_capacity(indices.len());
	for index in indices {
		match u16::try_from(*index) {
			Ok(index) => out.push(index),
			Err(_) => {
				eprintln!("render mesh '{mesh_name}' has index {index} outside u16 range");
				return Vec::new();
			}
		}
	}
	out
}

impl pge::App<RenderEvent> for RenderStateApp {
	fn on_create(&mut self, _state: &mut pge::State) {}

	fn on_event(&mut self, event: RenderEvent, state: &mut pge::State) {
		match event {
			RenderEvent::LoadRenderState(render_state) => self.load_render_state(render_state, state),
			RenderEvent::RequestScreenshot(path) => {
				self.screenshot_path = Some(path);
				self.screenshot_requested = false;
				self.next_frame = 0;
				self.screenshot_frame = 0;
			}
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
				let delta = pge::Vec2::new(dx, dy);
				if self.right_button_down {
					self.orbit_controller.orbit(delta);
				} else if self.middle_button_down {
					self.orbit_controller.pan(delta);
				}
			}
			pge::MouseEvent::Pressed { button } => match button {
				pge::MouseButton::Right => self.right_button_down = true,
				pge::MouseButton::Middle => self.middle_button_down = true,
				_ => {}
			},
			pge::MouseEvent::Released { button } => match button {
				pge::MouseButton::Right => self.right_button_down = false,
				pge::MouseButton::Middle => self.middle_button_down = false,
				_ => {}
			},
			pge::MouseEvent::Wheel { dy, .. } => {
				self.orbit_controller.zoom(dy);
			}
		}
	}

	fn on_process(&mut self, state: &mut pge::State, _delta: f32) {
		if let Some(camera_node_id) = self.camera_node_id {
			self.orbit_controller
				.process(state, camera_node_id, _delta);
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

pub fn run_parse(args: ParseArgs) -> ExitCode {
	let source = match read_source(args.input.as_deref()) {
		Ok(source) => source,
		Err(err) => {
			eprintln!("{err}");
			return ExitCode::FAILURE;
		}
	};

	match parse_pcad(&source) {
		Ok(ast) => {
			let output = if args.ast {
				ParseOutput::Ast
			} else if args.json || args.feature_script {
				ParseOutput::FeatureScript
			} else if args.model_state {
				ParseOutput::ModelState
			} else if args.render_state {
				ParseOutput::RenderState
			} else {
				args.output
			};

			match output {
				ParseOutput::Summary => {
					println!("Parsed {} declaration(s)", ast.decls.len());
				}
				ParseOutput::Ast => {
					println!("{ast:#?}");
				}
				ParseOutput::FeatureScript => {
					match codegen::compile_to_three_json(&ast) {
						Ok(json) => {
							println!("{json}");
						}
						Err(err) => {
							eprintln!("{err}");
							return ExitCode::FAILURE;
						}
					}
				}
				ParseOutput::ModelState => match build_model_state(&puppycad_core::FeatureGraph::new(&ast)) {
					Ok(state) => {
						println!("{state:#?}");
					}
					Err(err) => {
						eprintln!("{err}");
						return ExitCode::FAILURE;
					}
				},
				ParseOutput::RenderState => match build_model_state(&puppycad_core::FeatureGraph::new(&ast)) {
					Ok(state) => {
						let render_state = build_render_state(&state);
						println!("{render_state:#?}");
					}
					Err(err) => {
						eprintln!("{err}");
						return ExitCode::FAILURE;
					}
				},
			}
			ExitCode::SUCCESS
		}
		Err(err) => {
			eprintln!("{err}");
			ExitCode::FAILURE
		}
	}
}

pub fn run_validate(args: ValidateArgs) -> ExitCode {
	let source = match read_source(args.input.as_deref()) {
		Ok(source) => source,
		Err(err) => {
			eprintln!("{err}");
			return ExitCode::FAILURE;
		}
	};

	match parse_pcad(&source) {
		Ok(ast) => {
			let mut evaluator = Evaluator::new(&ast);
			match evaluator.build() {
				Ok(_) => {
					println!("pcad file is valid");
					ExitCode::SUCCESS
				}
				Err(err) => {
					eprintln!("{err}");
					ExitCode::FAILURE
				}
			}
		}
		Err(err) => {
			eprintln!("{err}");
			ExitCode::FAILURE
		}
	}
}

pub fn run_render(args: RenderArgs) -> ExitCode {
	let source = match read_source(args.input.as_deref()) {
		Ok(source) => source,
		Err(err) => {
			eprintln!("{err}");
			return ExitCode::FAILURE;
		}
	};

	let ast = match parse_pcad(&source) {
		Ok(ast) => ast,
		Err(err) => {
			eprintln!("{err}");
			return ExitCode::FAILURE;
		}
	};

	let state = match build_model_state(&puppycad_core::FeatureGraph::new(&ast)) {
		Ok(state) => state,
		Err(err) => {
			eprintln!("model-state failed: {err}");
			return ExitCode::FAILURE;
		}
	};
	let render_state = build_render_state(&state);
	if render_state.meshes.is_empty() {
		eprintln!("render warning: no supported renderable mesh found");
	}

	let screenshot_path = resolve_screenshot_path(args.output.as_deref(), args.output_dir.as_deref());
	if let Some(path) = screenshot_path.as_deref() {
		if let Some(parent) = Path::new(path).parent() {
			if parent.components().next().is_some() {
				if let Err(err) = std::fs::create_dir_all(parent) {
					eprintln!("failed to create screenshot directory '{}': {err}", parent.to_string_lossy());
					return ExitCode::FAILURE;
				}
			}
		}
	}

	let app = RenderStateApp::new(
		parse_vec3_arg(args.camera.as_deref()),
		parse_vec3_arg(args.look_at.as_deref()),
		screenshot_path.clone(),
	);

	let should_run_headless = args.headless || screenshot_path.is_some();
	let iterations = args.iterations.or(if should_run_headless { Some(1) } else { None });

	if should_run_headless {
		unsafe { std::env::set_var("HEADLESS", "1") };
	}
	if let Some(iterations) = iterations {
		unsafe { std::env::set_var("ITERATIONS", iterations.to_string()) };
	}

	if screenshot_path.is_some() {
		unsafe { std::env::set_var("SCREENSHOT", "1") };
	}

	let app_result = pge::run_with_event_sender(app, |sender| {
		_ = sender.send(RenderEvent::LoadRenderState(render_state));
		if let Some(path) = screenshot_path {
			_ = sender.send(RenderEvent::RequestScreenshot(path));
		}
	});

	match app_result {
		Ok(()) => ExitCode::SUCCESS,
		Err(err) => {
			eprintln!("render failed: {err}");
			ExitCode::FAILURE
		}
	}
}

fn parse_vec3_arg(values: Option<&[f32]>) -> Option<[f32; 3]> {
	let values = values?;
	if values.len() != 3 {
		return None;
	}

	Some([values[0], values[1], values[2]])
}

fn resolve_screenshot_path(output: Option<&Path>, output_dir: Option<&Path>) -> Option<String> {
	if let Some(directory) = output_dir {
		return Some(directory.join("frame_0.png").to_string_lossy().to_string());
	}

	match output {
		Some(path) if path.exists() && path.is_dir() => {
			Some(path.join("frame_0.png").to_string_lossy().to_string())
		}
		Some(path) => Some(path.to_string_lossy().to_string()),
		None => None,
	}
}

#[cfg(test)]
mod tests {
	use std::{fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};

	use super::*;

	#[test]
	fn validates_a_valid_file() {
		let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
		let input = manifest_dir.join("../examples/puppybot.pcad");
		let exit = run_validate(ValidateArgs { input: Some(input) });
		assert_eq!(exit, ExitCode::SUCCESS);
	}

	#[test]
	fn validates_reports_semantic_errors() {
		let invalid = std::env::temp_dir().join(format!(
			"puppycad-invalid-{}.pcad",
			SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
		));
		let bad_source = r#"
solid body = box {
  w: 1;
  h: 2;
  d: 3;
}

feature bad = hole {
  target: body.unknown;
  x: 1;
  y: 1;
  d: 1;
}
"#;

		fs::write(&invalid, bad_source).unwrap();
		let exit = run_validate(ValidateArgs {
			input: Some(invalid.clone()),
		});
		let _ = fs::remove_file(&invalid);
		assert_eq!(exit, ExitCode::FAILURE);
	}

	#[test]
	fn resolves_frame_output_path_in_directory_argument() {
		assert_eq!(
			resolve_screenshot_path(None, Some(Path::new("tmp-captures"))),
			Some("tmp-captures/frame_0.png".to_string())
		);
	}

	#[test]
	fn parses_three_component_camera_and_look_at_args() {
		let camera = vec![0.0, 3.5, -12.0];
		let look_at = vec![1.0, 0.0, 0.0];

		assert_eq!(
			parse_vec3_arg(Some(&camera)),
			Some([0.0, 3.5, -12.0])
		);
		assert_eq!(parse_vec3_arg(Some(&look_at)), Some([1.0, 0.0, 0.0]));
	}

	#[test]
	fn ignores_camera_and_look_at_args_without_three_components() {
		let partial = vec![1.0, 2.0];
		assert_eq!(parse_vec3_arg(Some(&partial)), None);
	}
}
