use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use hyper::body::to_bytes;
use hyper::header::CONTENT_TYPE;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::args::ApiArgs;

use puppycad_core::{
	build_model_state,
	build_render_state,
	codegen,
	eval::Evaluator,
	parser::parse_pcad,
	feature_graph::FeatureGraph,
	types::{ErrorLevel, LangError},
};

#[derive(Clone)]
struct Session {
	id: String,
	token: String,
	root_dir: PathBuf,
	entry_file: String,
	watch_files: bool,
}

#[derive(Clone)]
struct ApiState {
	sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl ApiState {
	fn new() -> Self {
		Self {
			sessions: Arc::new(RwLock::new(HashMap::new())),
		}
	}
}

enum ApiError {
	BadRequest(String),
	Unauthorized,
	NotFound(String),
	Conflict(String),
	Internal(String),
}

impl ApiError {
	fn into_response(self, request_id: Option<&str>) -> Response<Body> {
		let (status, message) = match self {
			Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
			Self::Unauthorized => (StatusCode::UNAUTHORIZED, "missing or invalid bearer token".to_string()),
			Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
			Self::Conflict(message) => (StatusCode::CONFLICT, message),
			Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
		};
		let body = json!({"ok": false, "error": message, "requestId": request_id});
		json_response(status, body)
	}
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateSessionRequest {
	root_dir: String,
	entry_file: String,
	watch_files: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileSetRequest {
	path: String,
	text: String,
	expected_etag: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FilePatchRequest {
	path: String,
	kind: Option<String>,
	text: Option<String>,
	patch: Option<Value>,
	expected_etag: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckRequest {
	entry_file: Option<String>,
	strict: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenderRequest {
	entry_file: Option<String>,
	build_id: Option<String>,
	target: Option<String>,
	size: Option<[u32; 2]>,
	cameras: Option<Vec<Value>>,
	outputs: Option<Vec<String>>,
	background: Option<String>,
	out_dir: Option<String>,
	iterations: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CameraSpec {
	eye: [f32; 3],	
	look: [f32; 3],
	up: Option<[f32; 3]>,
	fov: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportRequest {
	entry_file: Option<String>,
	target: Option<String>,
	format: String,
	quality: Option<String>,
	out_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactRef {
	kind: String,
	path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateSessionResponse {
	session_id: String,
	token: String,
	entry_file: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileGetResponse {
	path: String,
	text: String,
	etag: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileSetResponse {
	ok: bool,
	etag: String,
	diagnostics: Vec<Diagnostic>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckResponse {
	ok: bool,
	diagnostics: Vec<Diagnostic>,
	stats: CheckStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CheckStats {
	parse_ms: u128,
	resolve_ms: u128,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderResponse {
	ok: bool,
	diagnostics: Vec<Diagnostic>,
	frames: Map<String, Value>,
	stats: RenderStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderStats {
	render_ms: u128,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportResponse {
	ok: bool,
	artifact: ArtifactRef,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Diagnostic {
	level: String,
	code: String,
	message: String,
	span: Span,
	node: Option<String>,
	details: HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Span {
	start: Position,
	end: Position,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Position {
	line: usize,
	col: usize,
}

#[derive(Clone)]
struct ResolvedCamera {
	name: String,
	eye: [f32; 3],
	look: [f32; 3],
	fov: f32,
}

struct BuiltRuntime {
	render_state: puppycad_core::RenderState,
}

#[derive(Clone, Copy)]
struct ScenePointLightSpec {
	name: &'static str,
	position: [f32; 3],
	color: [f32; 3],
	intensity: f32,
}

enum ApiRenderEvent {
	LoadRenderState(puppycad_core::RenderState),
	RequestScreenshot(String),
}

struct ApiRenderStateApp {
	camera_position: Option<[f32; 3]>,
	camera_look_at: Option<[f32; 3]>,
	camera_fov_deg: f32,
	screenshot_path: Option<String>,
	screenshot_frame: u64,
	next_frame: u64,
	screenshot_requested: bool,
	scene_id: Option<pge::ArenaId<pge::Scene>>,
	mesh_node_ids: Vec<pge::ArenaId<pge::Node>>,
	mesh_ids: Vec<pge::ArenaId<pge::Mesh>>,
	camera_id: Option<pge::ArenaId<pge::Camera>>,
	camera_node_id: Option<pge::ArenaId<pge::Node>>,
	light_node_ids: Vec<pge::ArenaId<pge::Node>>,
	point_light_ids: Vec<pge::ArenaId<pge::PointLight>>,
	window_id: Option<pge::ArenaId<pge::Window>>,
	gui_id: Option<pge::ArenaId<pge::GUIElement>>,
}

impl ApiRenderStateApp {
	fn new(
		camera_position: Option<[f32; 3]>,
		camera_look_at: Option<[f32; 3]>,
		camera_fov_deg: f32,
		screenshot_path: Option<String>,
	) -> Self {
		Self {
			camera_position,
			camera_look_at,
			camera_fov_deg,
			screenshot_path,
			screenshot_frame: 0,
			next_frame: 0,
			screenshot_requested: false,
			scene_id: None,
			mesh_node_ids: Vec::new(),
			mesh_ids: Vec::new(),
			camera_id: None,
			camera_node_id: None,
			light_node_ids: Vec::new(),
			point_light_ids: Vec::new(),
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

		let fov_radians = self.camera_fov_deg.to_radians();
		let distance = if max_size > 0.0 {
			(max_size / 2.0) / fov_radians.tan()
		} else {
			3.0
		};
		let target = self.camera_look_at.unwrap_or([center.x, center.y, center.z]);
		let default_camera_position = [center.x, center.y, center.z + distance.max(0.1)];
		let camera_position = self.camera_position.unwrap_or(default_camera_position);

		let scene_center = [center.x, center.y, center.z];
		let light_distance = max_size.max(1.0);
		let mut add_light = |name: &str, position: [f32; 3], color: [f32; 3], intensity: f32| {
			let mut light_node = pge::Node::new();
			light_node.name = Some(name.to_string());
			light_node.translation = pge::Vec3::new(position[0], position[1], position[2]);
			light_node.parent = pge::NodeParent::Scene(scene_id);
			let light_node_id = state.nodes.insert(light_node);
			self.light_node_ids.push(light_node_id);

			let mut light = pge::PointLight::new();
			light.node_id = Some(light_node_id);
			light.color = color;
			light.intensity = intensity;
			let point_light_id = state.point_lights.insert(light);
			self.point_light_ids.push(point_light_id);
		};

		for spec in scene_point_light_specs(scene_center, light_distance) {
			add_light(spec.name, spec.position, spec.color, spec.intensity);
		}

		let mut camera_node = pge::Node::new();
		camera_node.parent = pge::NodeParent::Scene(scene_id);
		camera_node.translation = pge::Vec3::new(camera_position[0], camera_position[1], camera_position[2]);
		camera_node.looking_at(target[0], target[1], target[2]);
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

fn scene_point_light_specs(
	scene_center: [f32; 3],
	light_distance: f32,
) -> [ScenePointLightSpec; 3] {
	[
		ScenePointLightSpec {
			name: "MainLight",
			position: [
				scene_center[0] + light_distance,
				scene_center[1] + light_distance * 0.6,
				scene_center[2] + light_distance,
			],
			color: [1.0, 0.98, 0.9],
			intensity: 2.5,
		},
		ScenePointLightSpec {
			name: "FillLight",
			position: [
				scene_center[0] - light_distance * 0.8,
				scene_center[1] + light_distance * 0.4,
				scene_center[2] + light_distance * 0.4,
			],
			color: [0.7, 0.8, 1.0],
			intensity: 1.0,
		},
		ScenePointLightSpec {
			name: "RimLight",
			position: [
				scene_center[0],
				scene_center[1] - light_distance * 0.7,
				scene_center[2] - light_distance * 0.9,
			],
			color: [0.75, 0.85, 1.0],
			intensity: 0.7,
		},
	]
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn scene_point_lights_are_outside_unit_cube_bounds() {
		let specs = scene_point_light_specs([0.5, 0.5, 0.5], 1.0);
		assert_eq!(specs.len(), 3);

		for spec in specs {
			let p = spec.position;
			let outside_x = p[0] < 0.0 || p[0] > 1.0;
			let outside_y = p[1] < 0.0 || p[1] > 1.0;
			let outside_z = p[2] < 0.0 || p[2] > 1.0;
			assert!(
				outside_x || outside_y || outside_z,
				"light '{}' is not outside cube bounds: {:?}",
				spec.name,
				p
			);
		}
	}
}

impl pge::App<ApiRenderEvent> for ApiRenderStateApp {
	fn on_create(&mut self, _state: &mut pge::State) {}

	fn on_event(&mut self, event: ApiRenderEvent, state: &mut pge::State) {
		match event {
			ApiRenderEvent::LoadRenderState(render_state) => self.load_render_state(render_state, state),
			ApiRenderEvent::RequestScreenshot(path) => {
				self.screenshot_path = Some(path);
				self.screenshot_requested = false;
				self.next_frame = 0;
				self.screenshot_frame = 0;
			}
		}
	}

	fn on_process(&mut self, state: &mut pge::State, _delta: f32) {
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

pub fn run_api(args: ApiArgs) -> ExitCode {
	let ip = parse_host(&args.host)
		.ok_or_else(|| {
			eprintln!("invalid host: {}", args.host);
			ExitCode::FAILURE
		})
		.and_then(|ip| {
			if !args.allow_remote && !is_loopback_host(&ip) {
				eprintln!("refusing remote bind without --allow-remote");
				Err(ExitCode::FAILURE)
			} else {
				Ok(ip)
			}
		});

	let ip = match ip {
		Ok(ip) => ip,
		Err(code) => return code,
	};

	let state = ApiState::new();
	let addr = SocketAddr::new(ip, args.port);
	let runtime = match tokio::runtime::Runtime::new() {
		Ok(runtime) => runtime,
		Err(err) => {
			eprintln!("failed to start runtime: {err}");
			return ExitCode::FAILURE;
		}
	};

	println!("puppycad api listening on http://{addr}/api");
	runtime.block_on(async {
		if let Err(err) = run_http_server(addr, state).await {
			eprintln!("api error: {err}");
		}
	});

	ExitCode::SUCCESS
}

async fn run_http_server(addr: SocketAddr, state: ApiState) -> Result<(), String> {
	let make_service = make_service_fn(move |_conn| {
		let state = state.clone();
		async move {
			Ok::<_, hyper::Error>(service_fn(move |req| {
				let state = state.clone();
				async move {
					let request_id = Uuid::new_v4().to_string();
					let response = match handle_request(req, state).await {
						Ok(response) => response,
						Err(error) => error.into_response(Some(&request_id)),
					};
					Ok::<_, hyper::Error>(add_request_id(response, request_id))
				}
			}))
		}
	});

	Server::bind(&addr)
		.serve(make_service)
		.await
		.map_err(|err| err.to_string())
}

async fn handle_request(req: Request<Body>, state: ApiState) -> Result<Response<Body>, ApiError> {
	let segments: Vec<&str> = req
		.uri()
		.path()
		.trim_start_matches('/')
		.split('/')
		.filter(|part| !part.is_empty())
		.collect();

	if segments.len() < 1 || segments[0] != "api" {
		return Err(ApiError::NotFound("unknown endpoint".to_string()));
	}

	match (req.method(), segments.as_slice()) {
		(&Method::GET, ["api", "health"]) => Ok(json_response(
			StatusCode::OK,
			json!({
				"ok": true,
				"version": env!("CARGO_PKG_VERSION"),
				"engine": { "kernel": "occt", "renderer": "wgpu" },
			}),
		)),
		(&Method::POST, ["api", "sessions"]) => {
			let body = parse_json::<CreateSessionRequest>(req).await?;
			create_session(body, state).await
		}
		(&Method::DELETE, ["api", "sessions", session_id]) => {
			ensure_authorized_session(&state, session_id, &req).await?;
			delete_session(&state, session_id).await
		}
		(&Method::GET, ["api", "sessions", session_id, "files"]) => {
			let session = ensure_authorized_session(&state, session_id, &req).await?;
			let path = require_query_param(req.uri().query(), "path")
				.ok_or_else(|| ApiError::BadRequest("path query parameter is required".to_string()))?;
			get_file(session, path).await
		}
		(&Method::PUT, ["api", "sessions", session_id, "files"]) => {
			let session = ensure_authorized_session(&state, session_id, &req).await?;
			let body = parse_json::<FileSetRequest>(req).await?;
			set_file(session, body).await
		}
		(&Method::POST, ["api", "sessions", session_id, "files", "patch"]) => {
			let session = ensure_authorized_session(&state, session_id, &req).await?;
			let body = parse_json::<FilePatchRequest>(req).await?;
			patch_file(session, body).await
		}
		(&Method::POST, ["api", "sessions", session_id, "check"]) => {
			let session = ensure_authorized_session(&state, session_id, &req).await?;
			let body = parse_json::<CheckRequest>(req).await?;
			check_model(session, body.entry_file, body.strict.unwrap_or(false)).await
		}
		(&Method::POST, ["api", "sessions", session_id, "render"]) => {
			let session = ensure_authorized_session(&state, session_id, &req).await?;
			let body = parse_json::<RenderRequest>(req).await?;
			render_models(session, body).await
		}
		(&Method::POST, ["api", "sessions", session_id, "export"]) => {
			let session = ensure_authorized_session(&state, session_id, &req).await?;
			let body = parse_json::<ExportRequest>(req).await?;
			export_model(session, body).await
		}
		_ => Err(ApiError::NotFound("unknown endpoint".to_string())),
	}
}

async fn create_session(request: CreateSessionRequest, state: ApiState) -> Result<Response<Body>, ApiError> {
	let root_dir = if request.root_dir.trim().is_empty() {
		return Err(ApiError::BadRequest("rootDir is required".to_string()));
	} else {
		std::env::current_dir()
			.map_err(|err| ApiError::Internal(format!("failed to resolve cwd: {err}")))?
			.join(request.root_dir)
	};
	let canonical_root = root_dir
		.canonicalize()
		.map_err(|err| ApiError::BadRequest(format!("failed to resolve rootDir: {err}")))?;
	if !canonical_root.is_dir() {
		return Err(ApiError::BadRequest("rootDir must be a directory".to_string()));
	}

	let entry_file = request.entry_file.trim();
	if entry_file.is_empty() {
		return Err(ApiError::BadRequest("entryFile is required".to_string()));
	}

	let session = Session {
		id: Uuid::new_v4().to_string(),
		token: Uuid::new_v4().to_string(),
		root_dir: canonical_root,
		entry_file: entry_file.to_string(),
		watch_files: request.watch_files,
	};

	state.sessions.write().await.insert(session.id.clone(), session.clone());

	let response = CreateSessionResponse {
		session_id: session.id,
		token: session.token,
		entry_file: session.entry_file,
	};
	Ok(json_response(StatusCode::OK, response))
}

async fn delete_session(state: &ApiState, session_id: &str) -> Result<Response<Body>, ApiError> {
	let removed = state.sessions.write().await.remove(session_id);
	if removed.is_none() {
		return Err(ApiError::NotFound(format!("session '{session_id}' does not exist")));
	}
	Ok(json_response(StatusCode::OK, json!({"ok": true})))
}

async fn get_file(session: Session, path: String) -> Result<Response<Body>, ApiError> {
	let resolved = resolve_session_file(&session, &path)?;
	let text = std::fs::read_to_string(&resolved)
		.map_err(|err| ApiError::BadRequest(format!("failed to read file: {err}")))?;
	let etag = compute_etag(text.as_bytes());
	Ok(json_response(
		StatusCode::OK,
		FileGetResponse {
			path,
			text,
			etag,
		},
	))
}

async fn set_file(session: Session, request: FileSetRequest) -> Result<Response<Body>, ApiError> {
	let resolved = resolve_session_file(&session, &request.path)?;
	if let Some(parent) = resolved.parent() {
		std::fs::create_dir_all(parent)
			.map_err(|err| ApiError::Internal(format!("failed to create parent directories: {err}")))?;
	}
	if resolved.exists() {
		let existing = std::fs::read_to_string(&resolved)
			.map_err(|err| ApiError::Internal(format!("failed to read existing file: {err}")))?;
		let existing_etag = compute_etag(existing.as_bytes());
		if let Some(expected) = request.expected_etag.as_deref() {
			if expected != existing_etag {
				return Err(ApiError::Conflict("expectedEtag mismatch".to_string()));
			}
		}
	}

	let diagnostics = validate_source(&request.text);
	if diagnostics.iter().all(|diagnostic| diagnostic.level != "error") {
		std::fs::write(&resolved, &request.text)
			.map_err(|err| ApiError::Internal(format!("failed to write file: {err}")))?;
	}

	let ok = diagnostics.iter().all(|diagnostic| diagnostic.level != "error");
	Ok(json_response(
		if ok { StatusCode::OK } else { StatusCode::BAD_REQUEST },
		FileSetResponse {
			ok,
			etag: compute_etag(request.text.as_bytes()),
			diagnostics,
		},
	))
}

async fn patch_file(session: Session, request: FilePatchRequest) -> Result<Response<Body>, ApiError> {
	let kind = request.kind.unwrap_or_else(|| "text".to_string());
	match kind.as_str() {
		"text" => {
			let text = request
				.text
				.ok_or_else(|| ApiError::BadRequest("kind=text requires text field".to_string()))?;
			set_file(
				session,
				FileSetRequest {
					path: request.path,
					text,
					expected_etag: request.expected_etag,
				},
			)
			.await
		}
		"json_patch" | "ast_patch" => {
			Err(ApiError::BadRequest(
				"json_patch and ast_patch support not implemented in v0.1 daemon".to_string(),
			))
		}
		_ => Err(ApiError::BadRequest(format!("unknown patch kind '{kind}'"))),
	}
}

async fn check_model(
	session: Session,
	entry_file: Option<String>,
	_strict: bool,
) -> Result<Response<Body>, ApiError> {
	let entry = entry_file.unwrap_or_else(|| session.entry_file.clone());
	let resolved = resolve_session_file(&session, &entry)?;
	let source = std::fs::read_to_string(&resolved)
		.map_err(|err| ApiError::BadRequest(format!("failed to read entry file: {err}")))?;

	let parse_started = Instant::now();
	let ast = parse_pcad(&source)
		.map_err(|err| ApiError::BadRequest(err.message.clone()))?;
	let parse_ms = parse_started.elapsed().as_millis();

	let resolve_started = Instant::now();
	let mut evaluator = Evaluator::new(&ast);
	let mut diagnostics = Vec::new();
	evaluator.build().err().iter().for_each(|error| diagnostics.push(to_diagnostic(error)));
	let resolve_ms = resolve_started.elapsed().as_millis();

	let ok = diagnostics.is_empty();
	Ok(json_response(
		StatusCode::OK,
		CheckResponse {
			ok,
			diagnostics,
			stats: CheckStats {
				parse_ms,
				resolve_ms,
			},
		},
	))
}

async fn render_models(session: Session, request: RenderRequest) -> Result<Response<Body>, ApiError> {
	let _ = (request.build_id, request.target, request.size, request.outputs, request.background);
	let entry = request.entry_file.unwrap_or_else(|| session.entry_file.clone());
	let resolved = resolve_session_file(&session, &entry)?;
	let source = std::fs::read_to_string(&resolved)
		.map_err(|err| ApiError::BadRequest(format!("failed to read entry file: {err}")))?;
	let runtime = build_runtime(&source)?;

	let out_dir = request.out_dir.unwrap_or_else(|| "out/renders".to_string());
	let root_out = resolve_session_file(&session, &out_dir)?;
	let cameras = resolve_render_cameras(request.cameras)?;
	let iterations = request.iterations.unwrap_or(1);

	let mut frames = Map::new();
	let render_started = Instant::now();
	for camera in cameras {
		let out_path = root_out.join(format!("{}.png", sanitize_filename(&camera.name)));
		if let Some(parent) = out_path.parent() {
			std::fs::create_dir_all(parent)
				.map_err(|err| ApiError::Internal(format!("failed to create render directory: {err}")))?;
		}
		render_single_frame(&runtime, &camera, &out_path, iterations)?;
		let rel = out_path
			.strip_prefix(&session.root_dir)
			.unwrap_or(&out_path)
			.to_string_lossy()
			.to_string();
		frames.insert(
			camera.name,
			json!({
				"color": { "kind": "file", "path": rel },
			}),
		);
	}
	let render_ms = render_started.elapsed().as_millis();

	Ok(json_response(
		StatusCode::OK,
		RenderResponse {
			ok: true,
			diagnostics: vec![],
			frames,
			stats: RenderStats { render_ms },
		},
	))
}

async fn export_model(session: Session, request: ExportRequest) -> Result<Response<Body>, ApiError> {
	let _ = (request.target, request.quality);
	if request.format.as_str() != "glb" {
		return Err(ApiError::BadRequest("only glb export is currently supported".to_string()));
	}
	let entry = request.entry_file.unwrap_or_else(|| session.entry_file.clone());
	let source = read_session_text(&session, &entry)?;
	let ast = parse_pcad(&source).map_err(|err| ApiError::BadRequest(err.message.clone()))?;
	let output_data = codegen::compile_to_three_json(&ast)
		.map_err(|err| ApiError::Internal(format!("failed to serialize glb placeholder: {err}")))?;

	let out_path = resolve_session_file(&session, &request.out_path)?;
	if let Some(parent) = out_path.parent() {
		std::fs::create_dir_all(parent)
			.map_err(|err| ApiError::Internal(format!("failed to create export directory: {err}")))?;
	}
	std::fs::write(&out_path, output_data)
		.map_err(|err| ApiError::Internal(format!("failed to write export: {err}")))?;
	let rel = out_path
		.strip_prefix(&session.root_dir)
		.unwrap_or(&out_path)
		.to_string_lossy()
		.to_string();

	Ok(json_response(
		StatusCode::OK,
		ExportResponse {
			ok: true,
			artifact: ArtifactRef {
				kind: "file".to_string(),
				path: rel,
			},
		},
	))
}

async fn parse_json<T: DeserializeOwned>(req: Request<Body>) -> Result<T, ApiError> {
	let bytes = to_bytes(req.into_body())
		.await
		.map_err(|err| ApiError::BadRequest(format!("invalid request body: {err}")))?;
	serde_json::from_slice::<T>(&bytes).map_err(|err| ApiError::BadRequest(format!("invalid request JSON: {err}")))
}

async fn ensure_authorized_session(
	state: &ApiState,
	session_id: &str,
	req: &Request<Body>,
) -> Result<Session, ApiError> {
	let token = extract_token(req).ok_or(ApiError::Unauthorized)?;
	let sessions = state.sessions.read().await;
	let session = sessions
		.get(session_id)
		.filter(|session| session.token == token)
		.cloned()
		.ok_or(ApiError::Unauthorized)?;
	Ok(session)
}

fn extract_token(req: &Request<Body>) -> Option<String> {
	let header = req.headers().get("authorization")?;
	let value = header.to_str().ok()?;
	if let Some(token) = value.strip_prefix("Bearer ") {
		Some(token.to_string())
	} else {
		value.strip_prefix("bearer ").map(str::to_string)
	}
}

fn require_query_param(query: Option<&str>, key: &str) -> Option<String> {
	let query = query?;
	for pair in query.split('&') {
		let mut parts = pair.splitn(2, '=');
		if parts.next()? == key {
			return Some(parts.next().unwrap_or("").replace('+', " "));
		}
	}
	None
}

fn validate_source(source: &str) -> Vec<Diagnostic> {
	let ast = match parse_pcad(source) {
		Ok(ast) => ast,
		Err(err) => return vec![to_diagnostic(&err)],
	};
	let mut evaluator = Evaluator::new(&ast);
	match evaluator.build() {
		Ok(_nodes) => Vec::new(),
		Err(err) => vec![to_diagnostic(&err)],
	}
}

fn build_runtime(source: &str) -> Result<BuiltRuntime, ApiError> {
	let ast = parse_pcad(source).map_err(|err| ApiError::BadRequest(err.message.clone()))?;
	let graph = FeatureGraph::new(&ast);
	let model = build_model_state(&graph).map_err(|err| ApiError::BadRequest(err.message.clone()))?;
	let render_state = build_render_state(&model);
	Ok(BuiltRuntime { render_state })
}

fn render_single_frame(
	runtime: &BuiltRuntime,
	camera: &ResolvedCamera,
	output: &Path,
	iterations: u64,
) -> Result<(), ApiError> {
	let app = ApiRenderStateApp::new(
		Some(camera.eye),
		Some(camera.look),
		camera.fov,
		Some(output.to_string_lossy().to_string()),
	);

	unsafe {
		std::env::set_var("HEADLESS", "1");
		std::env::set_var("SCREENSHOT", "1");
		std::env::set_var("ITERATIONS", iterations.to_string());
	}
	let app_result = pge::run_with_event_sender(app, |sender| {
		let _ = sender.send(ApiRenderEvent::LoadRenderState(runtime.render_state.clone()));
		let _ = sender.send(ApiRenderEvent::RequestScreenshot(
			output.to_string_lossy().to_string(),
		));
	});
	app_result.map_err(|err| ApiError::Internal(format!("render failed: {err}")))?;
	if !output.exists() {
		return Err(ApiError::Internal(format!(
			"render completed but output file was not written: {}",
			output.to_string_lossy()
		)));
	}
	Ok(())
}

fn resolve_render_cameras(raw: Option<Vec<Value>>) -> Result<Vec<ResolvedCamera>, ApiError> {
	let requested = match raw {
		Some(cameras) => cameras,
		None => vec![Value::String("iso".to_string())],
	};

	let mut out = Vec::new();
	for entry in requested {
		if let Some(name) = entry.as_str() {
			if let Some(camera) = named_camera(name) {
				out.push(camera);
				continue;
			}
			return Err(ApiError::BadRequest(format!("unknown camera '{name}'")));
		}
		if let Some(object) = entry.as_object() {
			let name = object
				.get("name")
				.and_then(Value::as_str)
				.unwrap_or("custom")
				.to_string();
			let spec_value = object.get("spec").cloned().ok_or_else(|| {
				ApiError::BadRequest("camera object must include spec".to_string())
			})?;
			let spec: CameraSpec = serde_json::from_value(spec_value)
				.map_err(|err| ApiError::BadRequest(format!("invalid camera spec: {err}")))?;
			out.push(ResolvedCamera {
				name,
				eye: spec.eye,
				look: spec.look,
				fov: spec.fov.unwrap_or(35.0),
			});
			continue;
		}
		return Err(ApiError::BadRequest("invalid camera definition".to_string()));
	}

	if out.is_empty() {
		out.push(named_camera("iso").expect("named camera iso exists"));
	}
	Ok(out)
}

fn named_camera(name: &str) -> Option<ResolvedCamera> {
	match name {
		"iso" => Some(ResolvedCamera {
			name: name.to_string(),
			eye: [200.0, 200.0, 120.0],
			look: [70.0, 45.0, 30.0],
			fov: 35.0,
		}),
		"front" => Some(ResolvedCamera {
			name: name.to_string(),
			eye: [0.0, -150.0, 40.0],
			look: [0.0, 0.0, 0.0],
			fov: 35.0,
		}),
		"right" => Some(ResolvedCamera {
			name: name.to_string(),
			eye: [150.0, 0.0, 40.0],
			look: [0.0, 0.0, 0.0],
			fov: 35.0,
		}),
		"top" => Some(ResolvedCamera {
			name: name.to_string(),
			eye: [0.0, 0.0, 200.0],
			look: [0.0, 0.0, 0.0],
			fov: 35.0,
		}),
		_ => None,
	}
}

fn resolve_session_file(session: &Session, requested: &str) -> Result<PathBuf, ApiError> {
	let requested = requested.trim();
	let path = Path::new(requested);
	if requested.is_empty() {
		return Err(ApiError::BadRequest("path is required".to_string()));
	}
	if path.is_absolute() {
		return Err(ApiError::BadRequest("path must be relative to session root".to_string()));
	}
	if path.components().any(|component| matches!(component, Component::ParentDir)) {
		return Err(ApiError::BadRequest("path cannot contain ..".to_string()));
	}
	let full = session.root_dir.join(path);
	if !full.starts_with(&session.root_dir) {
		return Err(ApiError::BadRequest(
			"resolved file path is outside of session root".to_string(),
		));
	}
	Ok(full)
}

fn sanitize_filename(name: &str) -> String {
	name.chars()
		.map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
		.collect()
}

fn compute_etag(bytes: &[u8]) -> String {
	let digest = Sha256::digest(bytes);
	format!("sha256:{}", hex::encode(digest))
}

fn read_session_text(session: &Session, file: &str) -> Result<String, ApiError> {
	let resolved = resolve_session_file(session, file)?;
	std::fs::read_to_string(&resolved)
		.map_err(|err| ApiError::BadRequest(format!("failed to read file '{file}': {err}")))
}

fn to_diagnostic(err: &LangError) -> Diagnostic {
	let mut details = HashMap::new();
	for (name, value) in &err.details {
		details.insert(name.clone(), value.clone());
	}

	Diagnostic {
		level: match err.level {
			ErrorLevel::Error => "error".to_string(),
			ErrorLevel::Warning => "warning".to_string(),
		},
		code: err.code.as_str().to_string(),
		message: err.message.clone(),
		span: Span {
			start: Position {
				line: err.span.start.line,
				col: err.span.start.col,
			},
			end: Position {
				line: err.span.end.line,
				col: err.span.end.col,
			},
		},
		node: err.node.clone(),
		details,
	}
}

fn parse_host(host: &str) -> Option<IpAddr> {
	match host {
		"localhost" => Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
		"::1" => Some(IpAddr::V6(Ipv6Addr::LOCALHOST)),
		other => other.parse::<IpAddr>().ok(),
	}
}

fn is_loopback_host(ip: &IpAddr) -> bool {
	match ip {
		IpAddr::V4(v4) => v4.is_loopback(),
		IpAddr::V6(v6) => v6.is_loopback(),
	}
}

fn json_response<T: Serialize>(status: StatusCode, body: T) -> Response<Body> {
	let payload = serde_json::to_string(&body).unwrap_or_else(|_| "{}".to_string());
	let mut response = Response::new(Body::from(payload));
	*response.status_mut() = status;
	response
		.headers_mut()
		.insert(CONTENT_TYPE, "application/json".parse().expect("valid application/json header value"));
	response
}

fn add_request_id(mut response: Response<Body>, request_id: String) -> Response<Body> {
	response
		.headers_mut()
		.insert("x-request-id", request_id.parse().expect("request id must be valid header value"));
	response
}
