use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;

use crate::{
    args::{ParseArgs, RenderArgs, ValidateArgs},
    parser::parse_pcad,
};
use serde_json::Value as JsonValue;

#[derive(Clone)]
struct RenderPrimitive {
	name: String,
	scale: [f32; 3],
	translation: [f32; 3],
}

struct RenderApp {
	primitives: Vec<RenderPrimitive>,
	camera_position: Option<[f32; 3]>,
	camera_look_at: Option<[f32; 3]>,
	screenshot_path: Option<String>,
	screenshot_frame: u64,
	next_frame: u64,
	screenshot_requested: bool,
}

impl pge::App for RenderApp {
	fn on_create(&mut self, state: &mut pge::State) {
		if self.primitives.is_empty() {
			return;
		}

		let scene_id = state.scenes.insert(pge::Scene::new());
		let cube_mesh = state.meshes.insert(pge::cube(0.5));

		for primitive in &self.primitives {
			let mut node = pge::Node::new();
			node.name = Some(primitive.name.clone());
			node.parent = pge::NodeParent::Scene(scene_id);
			node.mesh = Some(cube_mesh);
			node.set_translation(
				primitive.translation[0],
				primitive.translation[1],
				primitive.translation[2],
			);
			node.scale(
				primitive.scale[0],
				primitive.scale[1],
				primitive.scale[2],
			);
			node.global_transform = node.matrix();
			state.nodes.insert(node);
		}

		let scene_bounding_box = state.get_scene_bounding_box(scene_id);
		let center = (scene_bounding_box.min + scene_bounding_box.max) * 0.5;
		let size = scene_bounding_box.max - scene_bounding_box.min;
		let max_size = size.x.max(size.y).max(size.z);

		let fov_degrees = 60.0_f32;
		let fov_radians = fov_degrees.to_radians();
		let distance = if max_size > 0.0 {
			(max_size / 2.0) / fov_radians.tan()
		} else {
			3.0
		};
		let target = self.camera_look_at.unwrap_or([center.x, center.y, center.z]);
		let default_camera_position = [center.x, center.y, center.z + distance.max(0.1)];
		let camera_position = self.camera_position.unwrap_or(default_camera_position);

		let mut light_node = pge::Node::new();
		light_node.name = Some("Light".to_string());
		light_node.translation = pge::Vec3::new(0.0, 5.0, -5.0);
		light_node.parent = pge::NodeParent::Scene(scene_id);
		let light_node_id = state.nodes.insert(light_node);
		let mut light = pge::PointLight::new();
		light.node_id = Some(light_node_id);
		state.point_lights.insert(light);

		let mut camera_node = pge::Node::new();
		camera_node.parent = pge::NodeParent::Scene(scene_id);
		camera_node.translation = pge::Vec3::new(camera_position[0], camera_position[1], camera_position[2]);
		camera_node.looking_at(target[0], target[1], target[2]);
		let camera_node_id = state.nodes.insert(camera_node);

		let mut camera = pge::Camera::new();
		camera.fovy = fov_radians;
		camera.node_id = Some(camera_node_id);
		let camera_id = state.cameras.insert(camera);

		let gui_id = state
			.guis
			.insert(pge::camera_view(camera_id));
		let window = pge::Window::new().title("render").ui(gui_id);
		state.windows.insert(window);
	}

	fn on_process(&mut self, state: &mut pge::State, _delta: f32) {
		if self.screenshot_path.is_none() || self.screenshot_requested {
			self.next_frame += 1;
			return;
		}

		if self.next_frame >= self.screenshot_frame {
			if let Some((window_id, _)) = state.windows.iter().next() {
				state.screenshot_request = Some((
					window_id.clone(),
					self.screenshot_path.clone().expect("screenshot path exists"),
				));
				self.screenshot_requested = true;
			}
		}

		self.next_frame += 1;
	}
}

pub fn run_parse(args: ParseArgs) -> ExitCode {
	let source = match crate::read_source(args.input.as_deref()) {
		Ok(source) => source,
		Err(err) => {
			eprintln!("{err}");
			return ExitCode::FAILURE;
		}
	};

	match parse_pcad(&source) {
		Ok(ast) => {
			if args.json {
				match crate::codegen::compile_to_three_json(&ast) {
					Ok(json) => {
						println!("{json}");
					}
					Err(err) => {
						eprintln!("{err}");
						return ExitCode::FAILURE;
					}
				}
			} else if args.ast {
				println!("{ast:#?}");
			} else {
				println!("Parsed {} declaration(s)", ast.decls.len());
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
	let source = match crate::read_source(args.input.as_deref()) {
		Ok(source) => source,
		Err(err) => {
			eprintln!("{err}");
			return ExitCode::FAILURE;
		}
	};

	match parse_pcad(&source) {
		Ok(ast) => {
			let mut evaluator = crate::eval::Evaluator::new(&ast);
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
	let source = match crate::read_source(args.input.as_deref()) {
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

	let mut evaluator = crate::eval::Evaluator::new(&ast);
	let nodes = match evaluator.build() {
		Ok(nodes) => nodes,
		Err(err) => {
			eprintln!("{err}");
			return ExitCode::FAILURE;
		}
	};

	let primitives = extract_render_primitives(&nodes);
	if primitives.is_empty() {
		eprintln!("render warning: no supported renderable nodes found (supported ops: box, translate)");
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

	let app = RenderApp {
		primitives,
		camera_position: parse_vec3_arg(args.camera.as_deref()),
		camera_look_at: parse_vec3_arg(args.look_at.as_deref()),
		screenshot_path: screenshot_path.clone(),
		screenshot_frame: 0,
		next_frame: 0,
		screenshot_requested: false,
	};

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

	match pge::run(app) {
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

fn extract_render_primitives(nodes: &[crate::types::CompiledNode]) -> Vec<RenderPrimitive> {
	let mut primitive_by_id: HashMap<String, RenderPrimitive> = HashMap::new();
	let mut primitives = Vec::new();

	for node in nodes {
		let Some(fields) = node.fields.as_object() else {
			continue;
		};
		let local_translation = read_xyz(fields);

		let primitive = match node.op.as_str() {
			"box" => read_box(fields).map(|scale| RenderPrimitive {
				name: node.id.clone(),
				scale,
				translation: local_translation,
			}),
			"translate" => {
				let source = fields.get("of").and_then(parse_node_ref);
				let by = fields.get("by").and_then(parse_vec3);
				match (source, by) {
					(Some(source), Some(by)) => primitive_by_id.get(source).map(|base| {
						let mut translated = base.clone();
						translated.translation = [
							base.translation[0] + by[0],
							base.translation[1] + by[1],
							base.translation[2] + by[2],
						];
						translated.name = node.id.clone();
						translated
					}),
					_ => None,
				}
			}
			_ => None,
		};

		if let Some(primitive) = primitive {
			primitive_by_id.insert(node.id.clone(), primitive.clone());
			primitives.push(primitive);
		}
	}

	primitives
}

fn read_box(fields: &serde_json::Map<String, JsonValue>) -> Option<[f32; 3]> {
	let w = parse_f32(fields.get("w"))?;
	let h = parse_f32(fields.get("h"))?;
	let d = parse_f32(fields.get("d"))?;
	Some([w, h, d])
}

fn read_xyz(fields: &serde_json::Map<String, JsonValue>) -> [f32; 3] {
	[
		parse_f32(fields.get("x")).unwrap_or(0.0),
		parse_f32(fields.get("y")).unwrap_or(0.0),
		parse_f32(fields.get("z")).unwrap_or(0.0),
	]
}

fn parse_vec3(value: &JsonValue) -> Option<[f32; 3]> {
	let array = value.as_array()?;
	if array.len() != 3 {
		return None;
	}
	let x = parse_f32(array.get(0))?;
	let y = parse_f32(array.get(1))?;
	let z = parse_f32(array.get(2))?;
	Some([x, y, z])
}

fn parse_node_ref(value: &JsonValue) -> Option<&str> {
	match value {
		JsonValue::String(value) => Some(value.as_str()),
		JsonValue::Object(obj) => {
			let kind = obj.get("kind")?.as_str()?;
			if kind != "node" {
				return None;
			}
			obj.get("id")?.as_str()
		}
		_ => None,
	}
}

fn parse_f32(value: Option<&JsonValue>) -> Option<f32> {
	value.and_then(|value| value.as_f64()).and_then(|value| {
		let parsed = value as f32;
		if parsed.is_finite() { Some(parsed) } else { None }
	})
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
	fn extracts_render_primitives_for_boxes_and_translate() {
		let nodes = vec![
			crate::types::CompiledNode {
				id: "base".to_string(),
				kind: "solid".to_string(),
				op: "box".to_string(),
				fields: serde_json::json!({
					"w": 1,
					"h": 2,
					"d": 3,
					"x": 10,
					"y": 20,
					"z": 30,
				}),
			},
			crate::types::CompiledNode {
				id: "shifted".to_string(),
				kind: "solid".to_string(),
				op: "translate".to_string(),
				fields: serde_json::json!({
					"of": {"kind":"node","id":"base"},
					"by": [1, 2, 3],
				}),
			},
		];

		let primitives = super::extract_render_primitives(&nodes);

		assert_eq!(primitives.len(), 2);
		assert_eq!(primitives[0].scale, [1.0, 2.0, 3.0]);
		assert_eq!(primitives[1].translation, [11.0, 22.0, 33.0]);
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
