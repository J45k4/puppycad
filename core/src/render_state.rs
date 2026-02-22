use crate::types::{ModelNode, ModelState, Value};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum RenderQuality {
	Draft,
	Normal,
	High,
}

#[derive(Clone, Debug)]
pub struct ViewParams {
	pub camera_pos: [f32; 3],
	pub view_proj: [[f32; 4]; 4],
	pub viewport_px: (u32, u32),
	pub quality: RenderQuality,
	pub max_chord_error_px: f32,
}

impl Default for ViewParams {
	fn default() -> Self {
		Self {
			camera_pos: [0.0, 0.0, 0.0],
			view_proj: [
				[1.0, 0.0, 0.0, 0.0],
				[0.0, 1.0, 0.0, 0.0],
				[0.0, 0.0, 1.0, 0.0],
				[0.0, 0.0, 0.0, 1.0],
			],
			viewport_px: (800, 600),
			quality: RenderQuality::Normal,
			max_chord_error_px: 1.0,
		}
	}
}

pub type PickKey = u32;

#[derive(Clone, Copy, Debug)]
pub enum PickKind {
	Face,
	Edge,
}

#[derive(Clone, Debug)]
pub struct PickRecord {
	pub decl_id: String,
	pub kind: PickKind,
	pub hint: String,
	pub pick: PickKey,
}

#[derive(Clone, Debug)]
pub struct FaceKeyRange {
	pub pick: PickKey,
	pub start_index: u32,
	pub index_count: u32,
}

#[derive(Clone, Debug)]
pub struct EdgeKeyRange {
	pub pick: PickKey,
	pub start_index: u32,
	pub index_count: u32,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
enum HoleTarget {
	Top,
	Bottom,
	Left,
	Right,
	Front,
	Back,
}

impl HoleTarget {
	fn from_anchor(anchor: &str) -> Option<Self> {
		match anchor {
			"top" => Some(Self::Top),
			"bottom" => Some(Self::Bottom),
			"left" => Some(Self::Left),
			"right" => Some(Self::Right),
			"front" => Some(Self::Front),
			"back" => Some(Self::Back),
			_ => None,
		}
	}

	fn opposite(self) -> Self {
		match self {
			Self::Top => Self::Bottom,
			Self::Bottom => Self::Top,
			Self::Left => Self::Right,
			Self::Right => Self::Left,
			Self::Front => Self::Back,
			Self::Back => Self::Front,
		}
	}

	fn axis_index(self) -> usize {
		match self {
			Self::Left | Self::Right => 0,
			Self::Front | Self::Back => 1,
			Self::Top | Self::Bottom => 2,
		}
	}

	fn axis_sign(self) -> f32 {
		match self {
			Self::Right | Self::Front | Self::Top => 1.0,
			Self::Left | Self::Back | Self::Bottom => -1.0,
		}
	}

	fn plane_uv_axes(self) -> (usize, usize) {
		match self {
			Self::Top | Self::Bottom => (0, 1),
			Self::Left | Self::Right => (1, 2),
			Self::Front | Self::Back => (0, 2),
		}
	}

	fn normal(self) -> [f32; 3] {
		match self {
			Self::Top => [0.0, 0.0, 1.0],
			Self::Bottom => [0.0, 0.0, -1.0],
			Self::Left => [-1.0, 0.0, 0.0],
			Self::Right => [1.0, 0.0, 0.0],
			Self::Front => [0.0, 1.0, 0.0],
			Self::Back => [0.0, -1.0, 0.0],
		}
	}

	fn base_face_corner_indices(self) -> [u32; 4] {
		match self {
			Self::Top => [4, 5, 6, 7],
			Self::Bottom => [0, 3, 2, 1],
			Self::Left => [0, 4, 7, 3],
			Self::Right => [1, 2, 6, 5],
			Self::Front => [3, 7, 6, 2],
			Self::Back => [0, 1, 5, 4],
		}
	}
}

#[derive(Clone)]
struct HoleSpec {
	decl_id: String,
	target_node: String,
	radius: f32,
	u: f32,
	v: f32,
	target: HoleTarget,
	through: bool,
}

#[derive(Clone)]
struct BoxSpec {
	node_id: String,
	offset: [f32; 3],
	width: f32,
	height: f32,
	depth: f32,
	edge_index: u32,
}

#[derive(Clone, Debug)]
pub struct Mesh {
	pub decl_id: String,
	pub positions: Vec<[f32; 3]>,
	pub normals: Vec<[f32; 3]>,
	pub indices: Vec<u32>,
	pub tri_face_ids: Vec<FaceKeyRange>,
	pub bounds: Aabb,
}

#[derive(Clone, Debug)]
pub struct Edges {
	pub decl_id: String,
	pub positions: Vec<[f32; 3]>,
	pub indices: Vec<u32>,
	pub edge_ids: Vec<EdgeKeyRange>,
}

#[derive(Clone, Debug)]
pub struct RenderState {
	pub meshes: Vec<Mesh>,
	pub edges: Vec<Edges>,
	pub pick_map: Vec<PickRecord>,
}

impl Default for RenderState {
	fn default() -> Self {
		Self {
			meshes: Vec::new(),
			edges: Vec::new(),
			pick_map: Vec::new(),
		}
	}
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Aabb {
	pub min: [f32; 3],
	pub max: [f32; 3],
}

impl Aabb {
	fn grow_point(&mut self, point: [f32; 3]) {
		self.min = [
			self.min[0].min(point[0]),
			self.min[1].min(point[1]),
			self.min[2].min(point[2]),
		];
		self.max = [
			self.max[0].max(point[0]),
			self.max[1].max(point[1]),
			self.max[2].max(point[2]),
		];
	}
}

fn push_triangle(
	a: u32,
	b: u32,
	c: u32,
	kind: PickKind,
	decl_id: &str,
	hint: String,
	next_pick_key: &mut PickKey,
	indices: &mut Vec<u32>,
	tri_face_ids: &mut Vec<FaceKeyRange>,
	pick_map: &mut Vec<PickRecord>,
	tri_start_index: &mut u32,
) {
	let pick = *next_pick_key;
	*next_pick_key = next_pick_key.saturating_add(1);
	indices.push(a);
	indices.push(b);
	indices.push(c);
	tri_face_ids.push(FaceKeyRange {
		pick,
		start_index: *tri_start_index,
		index_count: 3,
	});
	pick_map.push(PickRecord {
		decl_id: decl_id.to_owned(),
		kind,
		hint,
		pick,
	});
	*tri_start_index = tri_start_index.saturating_add(3);
}

fn emit_quad(
	a: u32,
	b: u32,
	c: u32,
	d: u32,
	kind: PickKind,
	decl_id: &str,
	prefix: &str,
	next_pick_key: &mut PickKey,
	indices: &mut Vec<u32>,
	tri_face_ids: &mut Vec<FaceKeyRange>,
	pick_map: &mut Vec<PickRecord>,
	tri_start_index: &mut u32,
) {
	let tri_a = format!("{prefix}.0");
	let tri_b = format!("{prefix}.1");
	push_triangle(
		a,
		b,
		c,
		kind,
		decl_id,
		tri_a,
		next_pick_key,
		indices,
		tri_face_ids,
		pick_map,
		tri_start_index,
	);
	push_triangle(
		a,
		c,
		d,
		kind,
		decl_id,
		tri_b,
		next_pick_key,
		indices,
		tri_face_ids,
		pick_map,
		tri_start_index,
	);
}

fn push_face_vertex(
	point: [f32; 3],
	positions: &mut Vec<[f32; 3]>,
	normals: &mut Vec<[f32; 3]>,
	face: HoleTarget,
) -> u32 {
	let idx = positions.len() as u32;
	positions.push(point);
	normals.push(face.normal());
	idx
}

fn face_normal_from_points(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
	let ux = b[0] - a[0];
	let uy = b[1] - a[1];
	let uz = b[2] - a[2];
	let vx = c[0] - a[0];
	let vy = c[1] - a[1];
	let vz = c[2] - a[2];

	let nx = uy * vz - uz * vy;
	let ny = uz * vx - ux * vz;
	let nz = ux * vy - uy * vx;
	let length = (nx * nx + ny * ny + nz * nz).sqrt();
	if length <= f32::EPSILON {
		return [0.0, 0.0, 0.0];
	}

	let inv_length = 1.0 / length;
	[nx * inv_length, ny * inv_length, nz * inv_length]
}

pub fn build_render_state(model: &ModelState) -> RenderState {
	build_render_state_with_view(model, &ViewParams::default())
}

pub fn build_render_state_with_view(model: &ModelState, _view: &ViewParams) -> RenderState {
	let mut state = RenderState::default();
	let mut next_pick_key = 1_u32;
	let mut translations: HashMap<String, [f32; 3]> = HashMap::new();
	let mut edge_nodes = 0_u32;
	let mut skipped_ops = Vec::<String>::new();
	let mut boxes = Vec::<BoxSpec>::new();
	let mut holes_by_target = HashMap::<String, Vec<HoleSpec>>::new();

	for node_id in &model.execution_order {
		let node = match model.nodes.get(node_id) {
			Some(node) => node,
			None => continue,
		};

		match node.op.as_str() {
			"box" => {
				let local = local_translation(node);
				let parent = translations.get(node_id).copied().unwrap_or([0.0, 0.0, 0.0]);
				let offset = [
					parent[0] + local[0],
					parent[1] + local[1],
					parent[2] + local[2],
				];
				let box_spec = BoxSpec {
					node_id: node_id.to_owned(),
					offset,
					width: node.fields.get("w").and_then(field_to_f32).unwrap_or(0.0),
					height: node.fields.get("h").and_then(field_to_f32).unwrap_or(0.0),
					depth: node.fields.get("d").and_then(field_to_f32).unwrap_or(0.0),
					edge_index: edge_nodes,
				};
				edge_nodes += 1;
				boxes.push(box_spec);
			}
			"translate" => {
				apply_translate(node, node_id, &mut translations);
			}
			"hole" => {
				match parse_hole(node, node_id, &model.nodes) {
					Ok(Some(spec)) => {
						let target = spec.target_node.clone();
						let entry = holes_by_target.entry(target).or_default();
						entry.push(spec);
					}
					Ok(None) => {}
					Err(message) => {
						eprintln!("render_state: skipped hole '{node_id}': {message}");
					}
				}
			}
			_ => skipped_ops.push(node.op.clone()),
		}
	}

	for spec in holes_by_target.values() {
		for hole in spec {
			if !model.nodes.contains_key(&hole.target_node) {
				eprintln!(
					"render_state: skipped hole '{}': target '{}' is unknown",
					hole.decl_id, hole.target_node
				);
			}
		}
	}

	for box_spec in boxes {
		let hole_specs = holes_by_target.remove(&box_spec.node_id).unwrap_or_default();
		let (cpu_mesh, cpu_edges, picks) = render_box_mesh(&box_spec, &hole_specs, &mut next_pick_key);
		state.meshes.push(cpu_mesh);
		if !cpu_edges.positions.is_empty() {
			state.edges.push(cpu_edges);
		}
		state.pick_map.extend(picks);
	}

	if !skipped_ops.is_empty() {
		let mut counts = HashMap::new();
		for op in skipped_ops {
			let count = counts.entry(op).or_insert(0_u32);
			*count += 1;
		}
		let mut messages = Vec::new();
		for (op, count) in counts {
			messages.push(format!("{op} x{count}"));
		}
		eprintln!("render_state: skipped unsupported op(s): {}", messages.join(", "));
	}

	state
}

fn render_box_mesh(
	spec: &BoxSpec,
	hole_specs: &[HoleSpec],
	next_pick_key: &mut PickKey,
) -> (Mesh, Edges, Vec<PickRecord>) {
	let node_id = spec.node_id.as_str();
	let width = spec.width;
	let height = spec.height;
	let depth = spec.depth;
	let offset = spec.offset;

	let min = [offset[0], offset[1], offset[2]];
	let max = [offset[0] + width, offset[1] + height, offset[2] + depth];

	let corner_positions = [
		min,
		[max[0], min[1], min[2]],
		[max[0], max[1], min[2]],
		[min[0], max[1], min[2]],
		[min[0], min[1], max[2]],
		[max[0], min[1], max[2]],
		[max[0], max[1], max[2]],
		[min[0], max[1], max[2]],
	];

	let mut positions = Vec::new();
	let mut normals = Vec::new();
	let mut indices = Vec::<u32>::new();
	let mut tri_face_ids = Vec::new();
	let mut pick_map = Vec::new();
	let mut tri_start_index = 0_u32;

	let mut hole_by_target: HashMap<HoleTarget, Vec<&HoleSpec>> = HashMap::new();
	let mut through_holes = Vec::<&HoleSpec>::new();
	for hole in hole_specs {
		let target = hole.target;
		hole_by_target.entry(target).or_default().push(hole);
		if hole.through {
			let opposite = target.opposite();
			hole_by_target.entry(opposite).or_default().push(hole);
			through_holes.push(hole);
		}
	}

	let mut emit_face = |face: HoleTarget, hole_spec: Option<&HoleSpec>| {
		let corners = face.base_face_corner_indices();
		let mut emit_corner = |corner: u32| -> u32 {
			let point = corner_positions[corner as usize];
			push_face_vertex(point, &mut positions, &mut normals, face)
		};
		let mut emit_solid_face = |kind: PickKind, decl_id: &str, hint: &str, next_pick_key: &mut PickKey| {
			let a = emit_corner(corners[0]);
			let b = emit_corner(corners[1]);
			let c = emit_corner(corners[2]);
			let d = emit_corner(corners[3]);
			emit_quad(
				a,
				b,
				c,
				d,
				kind,
				decl_id,
				hint,
				next_pick_key,
				&mut indices,
				&mut tri_face_ids,
				&mut pick_map,
				&mut tri_start_index,
			);
		};

	let Some(hole) = hole_spec else {
		let hint = format!("face.{face:?}");
		emit_solid_face(PickKind::Face, node_id, &hint, next_pick_key);
		return;
	};

		let (u_axis, v_axis) = face.plane_uv_axes();
		let min_u = min[u_axis];
		let max_u = max[u_axis];
		let min_v = min[v_axis];
		let max_v = max[v_axis];
		let n_axis = face.axis_index();
		let n_coord = if face.axis_sign() > 0.0 { max[n_axis] } else { min[n_axis] };

		let hole_u0 = (hole.u - hole.radius).clamp(min_u, max_u);
		let hole_u1 = (hole.u + hole.radius).clamp(min_u, max_u);
		let hole_v0 = (hole.v - hole.radius).clamp(min_v, max_v);
		let hole_v1 = (hole.v + hole.radius).clamp(min_v, max_v);

		if !(hole_u0 < hole_u1 && hole_v0 < hole_v1) {
			let hint = format!("face.{face:?}.fallback");
			emit_solid_face(PickKind::Face, node_id, &hint, next_pick_key);
			return;
		}

		let mut corner_point = |u: f32, v: f32| -> u32 {
			let mut point = [0.0; 3];
			point[u_axis] = u;
			point[v_axis] = v;
			point[n_axis] = n_coord;
			push_face_vertex(point, &mut positions, &mut normals, face)
		};

		let p0 = corner_point(min_u, min_v);
		let p1 = corner_point(max_u, min_v);
		let p2 = corner_point(max_u, max_v);
		let p3 = corner_point(min_u, max_v);
		let h0 = corner_point(hole_u0, hole_v0);
		let h1 = corner_point(hole_u1, hole_v0);
		let h2 = corner_point(hole_u1, hole_v1);
		let h3 = corner_point(hole_u0, hole_v1);

		if min_u < hole_u0 {
			emit_quad(
				p0,
				p1,
				h1,
				h0,
				PickKind::Face,
				hole.decl_id.as_str(),
				&format!("hole.{face:?}.bottom"),
				next_pick_key,
				&mut indices,
				&mut tri_face_ids,
				&mut pick_map,
				&mut tri_start_index,
			);
		}
		if hole_u1 < max_u {
			emit_quad(
				p1,
				p2,
				h2,
				h1,
				PickKind::Face,
				hole.decl_id.as_str(),
				&format!("hole.{face:?}.right"),
				next_pick_key,
				&mut indices,
				&mut tri_face_ids,
				&mut pick_map,
				&mut tri_start_index,
			);
		}
		if hole_v0 < max_v {
			emit_quad(
				p2,
				p3,
				h3,
				h2,
				PickKind::Face,
				hole.decl_id.as_str(),
				&format!("hole.{face:?}.top"),
				next_pick_key,
				&mut indices,
				&mut tri_face_ids,
				&mut pick_map,
				&mut tri_start_index,
			);
		}
		if min_v < hole_v1 {
			emit_quad(
				p3,
				p0,
				h0,
				h3,
				PickKind::Face,
				hole.decl_id.as_str(),
				&format!("hole.{face:?}.left"),
				next_pick_key,
				&mut indices,
				&mut tri_face_ids,
				&mut pick_map,
				&mut tri_start_index,
			);
		}
	};

	for face in [
		HoleTarget::Top,
		HoleTarget::Bottom,
		HoleTarget::Left,
		HoleTarget::Right,
		HoleTarget::Front,
		HoleTarget::Back,
	] {
		let target_holes = hole_by_target.remove(&face).unwrap_or_default();
		emit_face(face, target_holes.first().copied());
	}

	for hole in through_holes {
		let base_face = hole.target;
		let (u_axis, v_axis) = base_face.plane_uv_axes();
		let min_u = min[u_axis];
		let max_u = max[u_axis];
		let min_v = min[v_axis];
		let max_v = max[v_axis];
		let n_axis = base_face.axis_index();
		let base_n = if base_face.axis_sign() > 0.0 { max[n_axis] } else { min[n_axis] };
		let opposite = base_face.opposite();
		let opp_n = if opposite.axis_sign() > 0.0 { max[n_axis] } else { min[n_axis] };

		let hole_u0 = (hole.u - hole.radius).clamp(min_u, max_u);
		let hole_u1 = (hole.u + hole.radius).clamp(min_u, max_u);
		let hole_v0 = (hole.v - hole.radius).clamp(min_v, max_v);
		let hole_v1 = (hole.v + hole.radius).clamp(min_v, max_v);

		if !(hole_u0 < hole_u1 && hole_v0 < hole_v1) {
			continue;
		}

		let wall_point = |u: f32, v: f32, n: f32| -> [f32; 3] {
			let mut point = [0.0; 3];
			point[u_axis] = u;
			point[v_axis] = v;
			point[n_axis] = n;
			point
		};

		let b0 = wall_point(hole_u0, hole_v0, base_n);
		let b1 = wall_point(hole_u1, hole_v0, base_n);
		let b2 = wall_point(hole_u1, hole_v1, base_n);
		let b3 = wall_point(hole_u0, hole_v1, base_n);
		let o0 = wall_point(hole_u0, hole_v0, opp_n);
		let o1 = wall_point(hole_u1, hole_v0, opp_n);
		let o2 = wall_point(hole_u1, hole_v1, opp_n);
		let o3 = wall_point(hole_u0, hole_v1, opp_n);

		let mut emit_wall_quad = |a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], suffix: &str| {
			let normal = face_normal_from_points(a, b, c);
			let mut push_wall_vertex = |point: [f32; 3]| -> u32 {
				let idx = positions.len() as u32;
				positions.push(point);
				normals.push(normal);
				idx
			};
			let ia = push_wall_vertex(a);
			let ib = push_wall_vertex(b);
			let ic = push_wall_vertex(c);
			let id = push_wall_vertex(d);
			emit_quad(
				ia,
				ib,
				ic,
				id,
				PickKind::Face,
				&hole.decl_id,
				&format!("hole-wall.{base_face:?}.{suffix}"),
				next_pick_key,
				&mut indices,
				&mut tri_face_ids,
				&mut pick_map,
				&mut tri_start_index,
			);
		};

		emit_wall_quad(
			b0,
			b1,
			o1,
			o0,
			"0",
		);

		emit_wall_quad(
			b1,
			b2,
			o2,
			o1,
			"1",
		);

		emit_wall_quad(
			b2,
			b3,
			o3,
			o2,
			"2",
		);

		emit_wall_quad(
			b3,
			b0,
			o0,
			o3,
			"3",
		);
	}

	let mut bounds = Aabb {
		min: [f32::INFINITY; 3],
		max: [f32::NEG_INFINITY; 3],
	};
	for point in &positions {
		bounds.grow_point(*point);
	}

	let edge_pairs = [
		(0, 1),
		(1, 2),
		(2, 3),
		(3, 0),
		(4, 5),
		(5, 6),
		(6, 7),
		(7, 4),
		(0, 4),
		(1, 5),
		(2, 6),
		(3, 7),
	];

	let mut edge_positions = Vec::new();
	let mut edge_indices = Vec::new();
	let mut edge_key_ranges = Vec::new();
	for (edge_idx, (a, b)) in edge_pairs.iter().enumerate() {
		let start = edge_positions.len() as u32;
		edge_positions.push(corner_positions[*a]);
		edge_positions.push(corner_positions[*b]);
		edge_indices.push(start);
		edge_indices.push(start + 1);

		let pick = *next_pick_key;
		*next_pick_key = next_pick_key.saturating_add(1);
		edge_key_ranges.push(EdgeKeyRange {
			pick,
			start_index: start,
			index_count: 2,
		});
		pick_map.push(PickRecord {
			decl_id: node_id.to_owned(),
			kind: PickKind::Edge,
			hint: format!("edge.{}.{}", spec.edge_index, edge_idx),
			pick,
		});
	}

	let edges = Edges {
		decl_id: node_id.to_owned(),
		positions: edge_positions,
		indices: edge_indices,
		edge_ids: edge_key_ranges,
	};
	let mesh = Mesh {
		decl_id: node_id.to_owned(),
		positions,
		normals,
		indices,
		tri_face_ids,
		bounds,
	};

	(mesh, edges, pick_map)
}

fn apply_translate(
	node: &ModelNode,
	node_id: &str,
	translations: &mut HashMap<String, [f32; 3]>,
) {
	let target = node.fields.get("of").and_then(as_node_ref);
	let by = as_vec3(node.fields.get("by"));
	let Some(target) = target else {
		return;
	};
	let target_translation = translations.get(target).copied().unwrap_or([0.0, 0.0, 0.0]);
	let Some(by) = by else {
		return;
	};

	let next = [
		target_translation[0] + by[0],
		target_translation[1] + by[1],
		target_translation[2] + by[2],
	];
	translations.insert(node_id.to_owned(), next);
}

fn local_translation(node: &ModelNode) -> [f32; 3] {
	[
		node.fields.get("x").and_then(field_to_f32).unwrap_or(0.0),
		node.fields.get("y").and_then(field_to_f32).unwrap_or(0.0),
		node.fields.get("z").and_then(field_to_f32).unwrap_or(0.0),
	]
}

fn as_node_ref(value: &Value) -> Option<&str> {
	match value {
		Value::NodeRef(node_id) => Some(node_id.as_str()),
		Value::TargetRef { node, .. } => Some(node.as_str()),
		_ => None,
	}
}

fn as_vec3(value: Option<&Value>) -> Option<[f32; 3]> {
	match value {
		Some(Value::Vec3(values)) => Some([values[0] as f32, values[1] as f32, values[2] as f32]),
		_ => None,
	}
}

fn as_bool(value: &Value) -> Option<bool> {
	match value {
		Value::Bool(value) => Some(*value),
		_ => None,
	}
}

fn field_to_f32(value: &Value) -> Option<f32> {
	match value {
		Value::Number(value) => Some(*value as f32),
		_ => None,
	}
}

fn parse_hole(
	node: &ModelNode,
	decl_id: &str,
	nodes: &HashMap<String, ModelNode>,
) -> Result<Option<HoleSpec>, String> {
	let target_value = node.fields.get("target");
	let target_spec = match target_value {
		Some(Value::TargetRef { node: target_node, anchor }) => {
			let Some(target) = HoleTarget::from_anchor(anchor.as_str()) else {
				return Err(format!("unknown target anchor '{anchor}'"));
			};
			(target, target_node.clone())
		}
		Some(_) => return Err("target must be a target reference (e.g. body.top)".to_string()),
		None => return Err("missing target".to_string()),
	};

	let (target, target_node) = target_spec;
	if !nodes.contains_key(&target_node) {
		return Err(format!("unknown target node '{target_node}'"));
	}

	let diameter = match node.fields.get("d").and_then(field_to_f32) {
		Some(d) if d > 0.0 => d,
		Some(d) => return Err(format!("invalid diameter '{d}'")),
		None => return Err("missing diameter 'd'".to_string()),
	};

	let u = node.fields.get("x").and_then(field_to_f32).ok_or_else(|| "missing x".to_string())?;
	let v = node.fields.get("y").and_then(field_to_f32).ok_or_else(|| "missing y".to_string())?;
	let through = node
		.fields
		.get("through")
		.and_then(as_bool)
		.unwrap_or(true);

	Ok(Some(HoleSpec {
		decl_id: decl_id.to_string(),
		target_node,
		radius: diameter / 2.0,
		u,
		v,
		target,
		through,
	}))
}

#[cfg(test)]
mod tests {
	use crate::builder::build_model_state;
	use crate::feature_graph::FeatureGraph;
	use crate::parser::parse_pcad;

	const HOLE_MODEL: &str = r#"
solid body = box {
  w: 20;
  h: 20;
  d: 20;
}

feature hole1 = hole {
  let cx = body.w / 2;
  let cy = body.h / 2;

  target: body.top;
  x: cx;
  y: cy;
  d: 6;
}
"#;

	fn build_render_state_for_example() -> super::RenderState {
		let file = parse_pcad(HOLE_MODEL).expect("source parses");
		let graph = FeatureGraph::new(&file);
		let model = build_model_state(&graph).expect("model builds");
		super::build_render_state(&model)
	}

	#[test]
	fn build_render_state_does_not_panic_on_through_hole() {
		let state = build_render_state_for_example();
		assert!(
			!state.meshes.is_empty(),
			"expected at least one mesh"
		);
	}

	#[test]
	fn normals_align_with_positions_and_indices_for_hole_mesh() {
		let state = build_render_state_for_example();
		let mesh = state.meshes[0].clone();

		assert_eq!(
			mesh.positions.len(),
			mesh.normals.len(),
			"every vertex must have a normal"
		);
		assert_eq!(
			mesh.tri_face_ids.len(),
			(mesh.indices.len() / 3) as usize
		);
		for face_range in mesh.tri_face_ids {
			let start = face_range.start_index as usize;
			let end = start + face_range.index_count as usize;
			assert!(
				end <= mesh.indices.len(),
				"face index range must not exceed triangle index buffer"
			);
		}
		for pick_key in [
			"body",
			"hole1",
		] {
			assert!(
				state.pick_map.iter().any(|entry| entry.decl_id == pick_key),
				"expected pick record for {pick_key}"
			);
		}
	}
}
