use std::collections::HashMap;
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use bytemuck::{Pod, Zeroable};
use eframe::egui;
use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, Vec2};
use eframe::{egui_wgpu, wgpu};
use puppycad_core::{
    DeclKind, FeatureGraph, File, NodeGraph, RenderState, build_model_state, build_node_graph,
    build_render_state, parser::parse_pcad,
};

const WINDOW_TITLE: &str = "PuppyCAD Project Items";
const NODE_BOX_WIDTH: f32 = 2.8;
const NODE_BOX_HEIGHT: f32 = 1.2;
const NODE_BOX_DEPTH: f32 = 0.7;
const VIEW_FOV_Y_RADIANS: f32 = std::f32::consts::FRAC_PI_3;
const VIEW_NEAR_PLANE: f32 = 0.05;
const VIEW_FAR_PLANE: f32 = 10_000.0;
const WGPU_CLEAR_COLOR: Color32 = Color32::from_rgb(22, 25, 32);
const RESULT_LIGHT_DIR: [f32; 3] = [0.45, 0.75, 0.55];
const RESULT_WGPU_SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    light_dir: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    output.normal = input.normal;
    output.color = input.color;
    return output;
}

@fragment
fn fs_solid(input: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(input.normal);
    let light = normalize(uniforms.light_dir.xyz);
    let brightness = clamp(max(dot(n, light), 0.0) * 0.7 + 0.25, 0.0, 1.0);
    return vec4<f32>(input.color * brightness, 1.0);
}

@fragment
fn fs_lines(input: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewerMode {
    Graph,
    Result,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultRenderMode {
    Wireframe,
    Solid,
}

#[derive(Debug, Clone, PartialEq)]
struct ProjectItem {
    id: String,
    kind: DeclKind,
    op: String,
    dependencies: Vec<String>,
}

impl ProjectItem {
    fn from_node(node: &puppycad_core::NodeGraphNode) -> Self {
        Self {
            id: node.id.clone(),
            kind: node.kind,
            op: node.op.clone(),
            dependencies: node.dependencies.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Bounds3 {
    min: [f32; 3],
    max: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
struct CameraBasis {
    position: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    forward: [f32; 3],
}

#[derive(Debug, Clone, Copy)]
struct ProjectedPoint {
    pos: Pos2,
    depth: f32,
}

#[derive(Debug, Clone)]
struct ViewerState {
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: [f32; 3],
}

impl ViewerState {
    fn from_bounds(bounds: Bounds3) -> Self {
        let target = [
            (bounds.min[0] + bounds.max[0]) * 0.5,
            (bounds.min[1] + bounds.max[1]) * 0.5,
            (bounds.min[2] + bounds.max[2]) * 0.5,
        ];
        let size = [
            bounds.max[0] - bounds.min[0],
            bounds.max[1] - bounds.min[1],
            bounds.max[2] - bounds.min[2],
        ];
        let max_size = size[0].abs().max(size[1].abs()).max(size[2].abs());
        let distance = (max_size * 1.8).max(12.0);

        Self {
            yaw: 0.65,
            pitch: 0.45,
            distance,
            target,
        }
    }

    fn camera_basis(&self) -> CameraBasis {
        let cos_pitch = self.pitch.cos();
        let sin_pitch = self.pitch.sin();
        let cos_yaw = self.yaw.cos();
        let sin_yaw = self.yaw.sin();
        let offset = [
            self.distance * cos_pitch * sin_yaw,
            self.distance * sin_pitch,
            self.distance * cos_pitch * cos_yaw,
        ];
        let position = add3(self.target, offset);
        let forward = normalize3(sub3(self.target, position));
        let mut right = normalize3(cross3(forward, [0.0, 1.0, 0.0]));
        if length3(right) <= f32::EPSILON {
            right = [1.0, 0.0, 0.0];
        }
        let up = normalize3(cross3(right, forward));
        CameraBasis {
            position,
            right,
            up,
            forward,
        }
    }

    fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * 0.01;
        self.pitch = (self.pitch + delta.y * 0.01).clamp(-1.45, 1.45);
    }

    fn pan(&mut self, delta: Vec2) {
        let basis = self.camera_basis();
        let pan_scale = self.distance * 0.002;
        let right_offset = mul3(basis.right, -delta.x * pan_scale);
        let up_offset = mul3(basis.up, delta.y * pan_scale);
        self.target = add3(self.target, add3(right_offset, up_offset));
    }

    fn zoom(&mut self, scroll_y: f32) {
        let factor = (1.0 - scroll_y * 0.001).clamp(0.75, 1.25);
        self.distance = (self.distance * factor).clamp(2.0, 300.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WgpuResultDrawMode {
    Wireframe,
    Solid,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ResultWgpuVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
    _padding: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ResultWgpuUniforms {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _padding: f32,
}

#[derive(Clone)]
struct ResultWgpuScene {
    draw_mode: WgpuResultDrawMode,
    uniforms: ResultWgpuUniforms,
    triangle_vertices: Vec<ResultWgpuVertex>,
    triangle_indices: Vec<u32>,
    line_vertices: Vec<ResultWgpuVertex>,
    line_indices: Vec<u32>,
}

struct ResultWgpuPaintCallback {
    target_format: wgpu::TextureFormat,
    scene: ResultWgpuScene,
}

struct ResultWgpuResources {
    target_format: wgpu::TextureFormat,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    triangle_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    triangle_vertex_buffer: wgpu::Buffer,
    triangle_vertex_capacity: u64,
    triangle_index_buffer: wgpu::Buffer,
    triangle_index_capacity: u64,
    line_vertex_buffer: wgpu::Buffer,
    line_vertex_capacity: u64,
    line_index_buffer: wgpu::Buffer,
    line_index_capacity: u64,
}

#[derive(Debug, Clone)]
struct RenderPreview {
    state: RenderState,
    warning: Option<String>,
}

#[derive(Debug)]
struct ProjectItemsApp {
    graph: NodeGraph,
    render_state: Option<RenderState>,
    render_warning: Option<String>,
    render_error: Option<String>,
    position_by_id: HashMap<String, [f32; 2]>,
    items: Vec<ProjectItem>,
    filter_query: String,
    selected_index: Option<usize>,
    viewer_mode: ViewerMode,
    result_render_mode: ResultRenderMode,
    graph_viewer: ViewerState,
    result_viewer: ViewerState,
}

impl ProjectItemsApp {
    fn new(graph: NodeGraph, render_state_result: Result<RenderPreview, String>) -> Self {
        let position_by_id = graph
            .nodes
            .iter()
            .map(|node| (node.id.clone(), node.position))
            .collect::<HashMap<_, _>>();
        let items = graph
            .nodes
            .iter()
            .map(ProjectItem::from_node)
            .collect::<Vec<_>>();
        let selected_index = if items.is_empty() { None } else { Some(0) };

        let graph_viewer = ViewerState::from_bounds(graph_bounds(&graph));

        let (render_state, render_warning, render_error, result_viewer) = match render_state_result
        {
            Ok(preview) => {
                let bounds = render_bounds(&preview.state).unwrap_or_else(|| graph_bounds(&graph));
                (
                    Some(preview.state),
                    preview.warning,
                    None,
                    ViewerState::from_bounds(bounds),
                )
            }
            Err(err) => (None, None, Some(err), graph_viewer.clone()),
        };

        Self {
            graph,
            render_state,
            render_warning,
            render_error,
            position_by_id,
            items,
            filter_query: String::new(),
            selected_index,
            viewer_mode: ViewerMode::Graph,
            result_render_mode: ResultRenderMode::Wireframe,
            graph_viewer,
            result_viewer,
        }
    }

    fn filtered_indices(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| {
                item_matches_filter(item, &self.filter_query).then_some(index)
            })
            .collect()
    }

    fn ensure_valid_selection(&mut self, filtered: &[usize]) {
        if filtered.is_empty() {
            self.selected_index = None;
            return;
        }
        match self.selected_index {
            Some(selected) if filtered.contains(&selected) => {}
            _ => {
                self.selected_index = Some(filtered[0]);
            }
        }
    }

    fn move_selection(&mut self, filtered: &[usize], delta: i32) {
        if filtered.is_empty() {
            self.selected_index = None;
            return;
        }

        let current_position = self
            .selected_index
            .and_then(|selected| filtered.iter().position(|index| *index == selected))
            .unwrap_or(0) as i32;
        let max_position = filtered.len().saturating_sub(1) as i32;
        let next_position = (current_position + delta).clamp(0, max_position) as usize;
        self.selected_index = Some(filtered[next_position]);
    }

    fn selected_item(&self) -> Option<&ProjectItem> {
        self.selected_index.and_then(|index| self.items.get(index))
    }

    fn selected_item_id(&self) -> Option<&str> {
        self.selected_item().map(|item| item.id.as_str())
    }

    fn selected_label(item: &ProjectItem) -> String {
        format!("{}: {} {}", kind_label(item.kind), item.id, item.op)
    }

    fn draw_viewer_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("3D Viewer");
            ui.separator();
            ui.label("RMB orbit, MMB pan, mouse wheel zoom");
            ui.separator();
            ui.label("View:");
            ui.selectable_value(&mut self.viewer_mode, ViewerMode::Graph, "Graph");
            ui.selectable_value(&mut self.viewer_mode, ViewerMode::Result, "Result");
            if self.viewer_mode == ViewerMode::Result {
                ui.separator();
                ui.label("Mode:");
                ui.selectable_value(
                    &mut self.result_render_mode,
                    ResultRenderMode::Wireframe,
                    "Wireframe",
                );
                ui.selectable_value(
                    &mut self.result_render_mode,
                    ResultRenderMode::Solid,
                    "Solid",
                );
            }
        });
        if self.viewer_mode == ViewerMode::Result
            && let Some(warning) = &self.render_warning
        {
            ui.colored_label(
                Color32::from_rgb(255, 196, 120),
                format!("Preview warning: {warning}"),
            );
        }
        ui.separator();
    }

    fn draw_graph_viewer(&mut self, ui: &mut egui::Ui) {
        let selected_id = self.selected_item_id().map(str::to_owned);
        let (graph, position_by_id, viewer) =
            (&self.graph, &self.position_by_id, &mut self.graph_viewer);

        let desired = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(desired, Sense::drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, Color32::from_rgb(22, 25, 32));

        if response.hovered() {
            let pointer_delta = ui.ctx().input(|input| input.pointer.delta());
            if response.dragged_by(egui::PointerButton::Secondary) {
                viewer.orbit(pointer_delta);
                ui.ctx().request_repaint();
            }
            if response.dragged_by(egui::PointerButton::Middle) {
                viewer.pan(pointer_delta);
                ui.ctx().request_repaint();
            }

            let scroll_y = ui.ctx().input(|input| input.smooth_scroll_delta.y);
            if scroll_y.abs() > f32::EPSILON {
                viewer.zoom(scroll_y);
                ui.ctx().request_repaint();
            }
        }

        let camera = viewer.camera_basis();

        for edge in &graph.edges {
            let Some(from) = position_by_id.get(&edge.from) else {
                continue;
            };
            let Some(to) = position_by_id.get(&edge.to) else {
                continue;
            };
            let start = [from[0] + NODE_BOX_WIDTH * 0.5, from[1], 0.0];
            let end = [to[0] - NODE_BOX_WIDTH * 0.5, to[1], 0.0];

            if let (Some(a), Some(b)) = (
                project_point_to_rect(start, &camera, rect),
                project_point_to_rect(end, &camera, rect),
            ) {
                painter.line_segment([a, b], Stroke::new(1.5, Color32::from_gray(145)));
            }
        }

        for node in &graph.nodes {
            let center = [node.position[0], node.position[1], 0.0];
            let selected = selected_id.as_deref() == Some(node.id.as_str());
            let color = if selected {
                Color32::from_rgb(255, 214, 102)
            } else {
                match node.kind {
                    DeclKind::Solid => Color32::from_rgb(86, 156, 214),
                    DeclKind::Feature => Color32::from_rgb(231, 125, 64),
                }
            };

            draw_box_wireframe(
                &painter,
                rect,
                &camera,
                center,
                [NODE_BOX_WIDTH, NODE_BOX_HEIGHT, NODE_BOX_DEPTH],
                color,
                if selected { 2.4 } else { 1.4 },
            );

            if let Some(center_2d) = project_point_to_rect(center, &camera, rect) {
                painter.text(
                    center_2d + Vec2::new(8.0, -8.0),
                    Align2::LEFT_TOP,
                    format!("{} ({})", node.id, node.op),
                    FontId::monospace(12.0),
                    Color32::from_gray(235),
                );
            }
        }
    }

    fn draw_result_viewer(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let desired = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(desired, Sense::drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, WGPU_CLEAR_COLOR);

        if response.hovered() {
            let pointer_delta = ui.ctx().input(|input| input.pointer.delta());
            if response.dragged_by(egui::PointerButton::Secondary) {
                self.result_viewer.orbit(pointer_delta);
                ui.ctx().request_repaint();
            }
            if response.dragged_by(egui::PointerButton::Middle) {
                self.result_viewer.pan(pointer_delta);
                ui.ctx().request_repaint();
            }

            let scroll_y = ui.ctx().input(|input| input.smooth_scroll_delta.y);
            if scroll_y.abs() > f32::EPSILON {
                self.result_viewer.zoom(scroll_y);
                ui.ctx().request_repaint();
            }
        }

        if let Some(err) = &self.render_error {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("Render result is unavailable:\n{err}"),
                FontId::monospace(14.0),
                Color32::from_rgb(255, 190, 190),
            );
            return;
        }

        let Some(render_state) = &self.render_state else {
            painter.text(
                rect.center(),
                Align2::CENTER_CENTER,
                "No render result available",
                FontId::monospace(14.0),
                Color32::from_gray(210),
            );
            return;
        };

        let selected_id = self.selected_item_id().map(str::to_owned);
        let camera = self.result_viewer.camera_basis();
        if let Some(wgpu_render_state) = frame.wgpu_render_state() {
            let scene = build_result_wgpu_scene(
                render_state,
                rect,
                &camera,
                self.result_render_mode,
                selected_id.as_deref(),
            );
            painter.add(egui::Shape::Callback(
                egui_wgpu::Callback::new_paint_callback(
                    rect,
                    ResultWgpuPaintCallback {
                        target_format: wgpu_render_state.target_format,
                        scene,
                    },
                ),
            ));
            return;
        }

        match self.result_render_mode {
            ResultRenderMode::Wireframe => {
                draw_result_wireframe(
                    render_state,
                    &camera,
                    rect,
                    &painter,
                    selected_id.as_deref(),
                );
            }
            ResultRenderMode::Solid => {
                draw_result_solid(
                    render_state,
                    &camera,
                    rect,
                    &painter,
                    selected_id.as_deref(),
                );
            }
        }
    }
}

impl eframe::App for ProjectItemsApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let filtered = self.filtered_indices();
        self.ensure_valid_selection(&filtered);

        if ctx.input(|input| input.key_pressed(egui::Key::ArrowDown)) {
            self.move_selection(&filtered, 1);
        }
        if ctx.input(|input| input.key_pressed(egui::Key::ArrowUp)) {
            self.move_selection(&filtered, -1);
        }

        egui::TopBottomPanel::top("project_items_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(WINDOW_TITLE);
                ui.separator();
                ui.label(format!("{} item(s)", self.items.len()));
                ui.separator();
                ui.label(format!("{} visible", filtered.len()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        egui::SidePanel::left("project_items_list")
            .resizable(true)
            .default_width(340.0)
            .min_width(260.0)
            .show(ctx, |ui| {
                ui.label("Filter");
                ui.add(
                    egui::TextEdit::singleline(&mut self.filter_query)
                        .hint_text("id, op, dependency, kind"),
                );
                ui.separator();

                if filtered.is_empty() {
                    ui.label("No items match the current filter.");
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        for index in &filtered {
                            let item = &self.items[*index];
                            let selected = self.selected_index == Some(*index);
                            let response =
                                ui.selectable_label(selected, Self::selected_label(item));
                            if response.clicked() {
                                self.selected_index = Some(*index);
                            }
                        }
                    });
            });

        egui::SidePanel::right("project_items_3d")
            .resizable(true)
            .default_width(640.0)
            .min_width(360.0)
            .show(ctx, |ui| {
                self.draw_viewer_header(ui);
                match self.viewer_mode {
                    ViewerMode::Graph => self.draw_graph_viewer(ui),
                    ViewerMode::Result => self.draw_result_viewer(ui, frame),
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Details");
            ui.separator();
            match self.selected_item() {
                Some(item) => {
                    ui.label(format!("ID: {}", item.id));
                    ui.label(format!("Kind: {}", kind_label(item.kind)));
                    ui.label(format!("Operation: {}", item.op));
                    ui.separator();
                    ui.label("Dependencies");
                    if item.dependencies.is_empty() {
                        ui.label("None");
                    } else {
                        for dependency in &item.dependencies {
                            ui.label(format!("- {}", dependency));
                        }
                    }
                }
                None => {
                    ui.label("No item selected.");
                }
            }
        });
    }
}

pub fn run_project_items_ui(input_path: Option<PathBuf>) -> ExitCode {
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
    let render_state_result = build_render_preview(&ast);

    match run_project_items_window(graph, render_state_result) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

fn run_project_items_window(
    graph: NodeGraph,
    render_state_result: Result<RenderPreview, String>,
) -> Result<(), String> {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        depth_buffer: 32,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([980.0, 580.0]),
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        native_options,
        Box::new(move |_cc| Ok(Box::new(ProjectItemsApp::new(graph, render_state_result)))),
    )
    .map_err(|err| format!("failed to open project items UI: {err}"))
}

impl ResultWgpuResources {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("project_items_result_shader"),
            source: wgpu::ShaderSource::Wgsl(RESULT_WGPU_SHADER.into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("project_items_result_uniform_buffer"),
            size: size_of::<ResultWgpuUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("project_items_result_uniform_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("project_items_result_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("project_items_result_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let triangle_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("project_items_result_triangle_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Self::vertex_buffer_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_solid"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview: None,
            cache: None,
        });

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("project_items_result_line_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Self::vertex_buffer_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_lines"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            multiview: None,
            cache: None,
        });

        Self {
            target_format,
            uniform_buffer,
            uniform_bind_group,
            triangle_pipeline,
            line_pipeline,
            triangle_vertex_buffer: create_data_buffer(
                device,
                "project_items_result_triangle_vertices",
                4,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            triangle_vertex_capacity: 4,
            triangle_index_buffer: create_data_buffer(
                device,
                "project_items_result_triangle_indices",
                4,
                wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            ),
            triangle_index_capacity: 4,
            line_vertex_buffer: create_data_buffer(
                device,
                "project_items_result_line_vertices",
                4,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            line_vertex_capacity: 4,
            line_index_buffer: create_data_buffer(
                device,
                "project_items_result_line_indices",
                4,
                wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            ),
            line_index_capacity: 4,
        }
    }

    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
        wgpu::VertexBufferLayout {
            array_stride: size_of::<ResultWgpuVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }

    fn upload_uniforms(&self, queue: &wgpu::Queue, uniforms: &ResultWgpuUniforms) {
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(uniforms));
    }

    fn upload_triangles(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[ResultWgpuVertex],
        indices: &[u32],
    ) {
        Self::upload_geometry(
            device,
            queue,
            "project_items_result_triangle_vertices",
            "project_items_result_triangle_indices",
            vertices,
            indices,
            &mut self.triangle_vertex_buffer,
            &mut self.triangle_vertex_capacity,
            &mut self.triangle_index_buffer,
            &mut self.triangle_index_capacity,
        );
    }

    fn upload_lines(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[ResultWgpuVertex],
        indices: &[u32],
    ) {
        Self::upload_geometry(
            device,
            queue,
            "project_items_result_line_vertices",
            "project_items_result_line_indices",
            vertices,
            indices,
            &mut self.line_vertex_buffer,
            &mut self.line_vertex_capacity,
            &mut self.line_index_buffer,
            &mut self.line_index_capacity,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn upload_geometry(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertex_label: &str,
        index_label: &str,
        vertices: &[ResultWgpuVertex],
        indices: &[u32],
        vertex_buffer: &mut wgpu::Buffer,
        vertex_capacity: &mut u64,
        index_buffer: &mut wgpu::Buffer,
        index_capacity: &mut u64,
    ) {
        let vertex_bytes = (vertices.len() * size_of::<ResultWgpuVertex>()) as u64;
        let index_bytes = (indices.len() * size_of::<u32>()) as u64;

        ensure_data_buffer_capacity(
            device,
            vertex_buffer,
            vertex_capacity,
            vertex_label,
            vertex_bytes,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );
        ensure_data_buffer_capacity(
            device,
            index_buffer,
            index_capacity,
            index_label,
            index_bytes,
            wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        );

        if !vertices.is_empty() {
            queue.write_buffer(vertex_buffer, 0, bytemuck::cast_slice(vertices));
        }
        if !indices.is_empty() {
            queue.write_buffer(index_buffer, 0, bytemuck::cast_slice(indices));
        }
    }
}

impl egui_wgpu::CallbackTrait for ResultWgpuPaintCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _egui_encoder: &mut wgpu::CommandEncoder,
        callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources = callback_resources
            .entry::<ResultWgpuResources>()
            .or_insert_with(|| ResultWgpuResources::new(device, self.target_format));
        if resources.target_format != self.target_format {
            *resources = ResultWgpuResources::new(device, self.target_format);
        }

        resources.upload_uniforms(queue, &self.scene.uniforms);
        resources.upload_triangles(
            device,
            queue,
            &self.scene.triangle_vertices,
            &self.scene.triangle_indices,
        );
        resources.upload_lines(
            device,
            queue,
            &self.scene.line_vertices,
            &self.scene.line_indices,
        );

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let Some(resources) = callback_resources.get::<ResultWgpuResources>() else {
            return;
        };

        if self.scene.draw_mode == WgpuResultDrawMode::Solid
            && !self.scene.triangle_indices.is_empty()
        {
            render_pass.set_pipeline(&resources.triangle_pipeline);
            render_pass.set_bind_group(0, &resources.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, resources.triangle_vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                resources.triangle_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..self.scene.triangle_indices.len() as u32, 0, 0..1);
        }

        if !self.scene.line_indices.is_empty() {
            render_pass.set_pipeline(&resources.line_pipeline);
            render_pass.set_bind_group(0, &resources.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, resources.line_vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                resources.line_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..self.scene.line_indices.len() as u32, 0, 0..1);
        }
    }
}

fn build_result_wgpu_scene(
    render_state: &RenderState,
    rect: Rect,
    camera: &CameraBasis,
    mode: ResultRenderMode,
    selected_id: Option<&str>,
) -> ResultWgpuScene {
    let selected_fill = color32_to_rgb(Color32::from_rgb(255, 202, 92));
    let default_fill = color32_to_rgb(Color32::from_rgb(114, 152, 208));
    let selected_wire = color32_to_rgb(Color32::from_rgb(255, 214, 102));
    let default_wire = color32_to_rgb(Color32::from_gray(175));
    let selected_overlay = color32_to_rgb(Color32::from_rgb(255, 245, 196));
    let line_normal = [0.0, 0.0, 1.0];

    let mut triangle_vertices = Vec::new();
    let mut triangle_indices = Vec::new();
    let mut line_vertices = Vec::new();
    let mut line_indices = Vec::new();

    match mode {
        ResultRenderMode::Wireframe => {
            for mesh in &render_state.meshes {
                let color = if selected_id == Some(mesh.decl_id.as_str()) {
                    selected_wire
                } else {
                    default_wire
                };
                for tri in mesh.indices.chunks_exact(3) {
                    let Some((a, b, c)) = triangle_positions(mesh, tri) else {
                        continue;
                    };
                    push_line_segment(
                        &mut line_vertices,
                        &mut line_indices,
                        a,
                        b,
                        line_normal,
                        color,
                    );
                    push_line_segment(
                        &mut line_vertices,
                        &mut line_indices,
                        b,
                        c,
                        line_normal,
                        color,
                    );
                    push_line_segment(
                        &mut line_vertices,
                        &mut line_indices,
                        c,
                        a,
                        line_normal,
                        color,
                    );
                }
            }
        }
        ResultRenderMode::Solid => {
            for mesh in &render_state.meshes {
                let color = if selected_id == Some(mesh.decl_id.as_str()) {
                    selected_fill
                } else {
                    default_fill
                };
                for tri in mesh.indices.chunks_exact(3) {
                    let (Ok(i0), Ok(i1), Ok(i2)) = (
                        usize::try_from(tri[0]),
                        usize::try_from(tri[1]),
                        usize::try_from(tri[2]),
                    ) else {
                        continue;
                    };
                    let Some(a) = mesh.positions.get(i0).copied() else {
                        continue;
                    };
                    let Some(b) = mesh.positions.get(i1).copied() else {
                        continue;
                    };
                    let Some(c) = mesh.positions.get(i2).copied() else {
                        continue;
                    };

                    let fallback_normal = normalize3(cross3(sub3(b, a), sub3(c, a)));
                    let n0 = mesh
                        .normals
                        .get(i0)
                        .copied()
                        .map(normalize3)
                        .filter(|n| length3(*n) > f32::EPSILON)
                        .unwrap_or(fallback_normal);
                    let n1 = mesh
                        .normals
                        .get(i1)
                        .copied()
                        .map(normalize3)
                        .filter(|n| length3(*n) > f32::EPSILON)
                        .unwrap_or(fallback_normal);
                    let n2 = mesh
                        .normals
                        .get(i2)
                        .copied()
                        .map(normalize3)
                        .filter(|n| length3(*n) > f32::EPSILON)
                        .unwrap_or(fallback_normal);

                    let Ok(base) = u32::try_from(triangle_vertices.len()) else {
                        continue;
                    };
                    triangle_vertices.push(ResultWgpuVertex {
                        position: a,
                        normal: n0,
                        color,
                        _padding: 0.0,
                    });
                    triangle_vertices.push(ResultWgpuVertex {
                        position: b,
                        normal: n1,
                        color,
                        _padding: 0.0,
                    });
                    triangle_vertices.push(ResultWgpuVertex {
                        position: c,
                        normal: n2,
                        color,
                        _padding: 0.0,
                    });
                    triangle_indices.extend_from_slice(&[base, base + 1, base + 2]);
                }
            }

            if let Some(selected_id) = selected_id {
                for edges in &render_state.edges {
                    if edges.decl_id != selected_id {
                        continue;
                    }
                    for segment in edges.indices.chunks_exact(2) {
                        let (Ok(i0), Ok(i1)) =
                            (usize::try_from(segment[0]), usize::try_from(segment[1]))
                        else {
                            continue;
                        };
                        let Some(a) = edges.positions.get(i0).copied() else {
                            continue;
                        };
                        let Some(b) = edges.positions.get(i1).copied() else {
                            continue;
                        };
                        push_line_segment(
                            &mut line_vertices,
                            &mut line_indices,
                            a,
                            b,
                            line_normal,
                            selected_overlay,
                        );
                    }
                }
            }
        }
    }

    let aspect = (rect.width() / rect.height().max(1.0)).max(0.1);
    let draw_mode = match mode {
        ResultRenderMode::Wireframe => WgpuResultDrawMode::Wireframe,
        ResultRenderMode::Solid => WgpuResultDrawMode::Solid,
    };

    ResultWgpuScene {
        draw_mode,
        uniforms: ResultWgpuUniforms {
            view_proj: view_projection_matrix(camera, aspect),
            light_dir: normalize3(RESULT_LIGHT_DIR),
            _padding: 0.0,
        },
        triangle_vertices,
        triangle_indices,
        line_vertices,
        line_indices,
    }
}

fn create_data_buffer(
    device: &wgpu::Device,
    label: &str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(4),
        usage,
        mapped_at_creation: false,
    })
}

fn ensure_data_buffer_capacity(
    device: &wgpu::Device,
    buffer: &mut wgpu::Buffer,
    capacity: &mut u64,
    label: &str,
    needed_bytes: u64,
    usage: wgpu::BufferUsages,
) {
    if needed_bytes <= *capacity {
        return;
    }

    let next = needed_bytes.next_power_of_two().max(256);
    *buffer = create_data_buffer(device, label, next, usage);
    *capacity = next;
}

fn push_line_segment(
    vertices: &mut Vec<ResultWgpuVertex>,
    indices: &mut Vec<u32>,
    a: [f32; 3],
    b: [f32; 3],
    normal: [f32; 3],
    color: [f32; 3],
) {
    let Ok(base) = u32::try_from(vertices.len()) else {
        return;
    };
    vertices.push(ResultWgpuVertex {
        position: a,
        normal,
        color,
        _padding: 0.0,
    });
    vertices.push(ResultWgpuVertex {
        position: b,
        normal,
        color,
        _padding: 0.0,
    });
    indices.extend_from_slice(&[base, base + 1]);
}

fn color32_to_rgb(color: Color32) -> [f32; 3] {
    [
        color.r() as f32 / 255.0,
        color.g() as f32 / 255.0,
        color.b() as f32 / 255.0,
    ]
}

fn view_projection_matrix(camera: &CameraBasis, aspect: f32) -> [[f32; 4]; 4] {
    let f = 1.0 / (VIEW_FOV_Y_RADIANS * 0.5).tan();
    let near = VIEW_NEAR_PLANE;
    let far = VIEW_FAR_PLANE;
    let a = far / (far - near);
    let b = -(near * far) / (far - near);

    let view = [
        [
            camera.right[0],
            camera.right[1],
            camera.right[2],
            -dot3(camera.right, camera.position),
        ],
        [
            camera.up[0],
            camera.up[1],
            camera.up[2],
            -dot3(camera.up, camera.position),
        ],
        [
            camera.forward[0],
            camera.forward[1],
            camera.forward[2],
            -dot3(camera.forward, camera.position),
        ],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let projection = [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, a, b],
        [0.0, 0.0, 1.0, 0.0],
    ];

    mat4_transpose(mat4_mul(projection, view))
}

fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            out[row][col] = a[row][0] * b[0][col]
                + a[row][1] * b[1][col]
                + a[row][2] * b[2][col]
                + a[row][3] * b[3][col];
        }
    }
    out
}

fn mat4_transpose(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for row in 0..4 {
        for col in 0..4 {
            out[row][col] = m[col][row];
        }
    }
    out
}

fn draw_box_wireframe(
    painter: &Painter,
    rect: Rect,
    camera: &CameraBasis,
    center: [f32; 3],
    size: [f32; 3],
    color: Color32,
    thickness: f32,
) {
    let corners = box_corners(center, size);

    const BOX_EDGES: [(usize, usize); 12] = [
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

    for (a, b) in BOX_EDGES {
        if let (Some(pa), Some(pb)) = (
            project_point_to_rect(corners[a], camera, rect),
            project_point_to_rect(corners[b], camera, rect),
        ) {
            painter.line_segment([pa, pb], Stroke::new(thickness, color));
        }
    }
}

fn box_corners(center: [f32; 3], size: [f32; 3]) -> [[f32; 3]; 8] {
    let hx = size[0] * 0.5;
    let hy = size[1] * 0.5;
    let hz = size[2] * 0.5;

    [
        [center[0] - hx, center[1] - hy, center[2] - hz],
        [center[0] + hx, center[1] - hy, center[2] - hz],
        [center[0] + hx, center[1] + hy, center[2] - hz],
        [center[0] - hx, center[1] + hy, center[2] - hz],
        [center[0] - hx, center[1] - hy, center[2] + hz],
        [center[0] + hx, center[1] - hy, center[2] + hz],
        [center[0] + hx, center[1] + hy, center[2] + hz],
        [center[0] - hx, center[1] + hy, center[2] + hz],
    ]
}

fn project_point_to_rect(point: [f32; 3], camera: &CameraBasis, rect: Rect) -> Option<Pos2> {
    project_point_with_depth(point, camera, rect).map(|point| point.pos)
}

fn project_point_with_depth(
    point: [f32; 3],
    camera: &CameraBasis,
    rect: Rect,
) -> Option<ProjectedPoint> {
    let rel = sub3(point, camera.position);
    let view_x = dot3(rel, camera.right);
    let view_y = dot3(rel, camera.up);
    let view_z = dot3(rel, camera.forward);
    if view_z <= 0.01 {
        return None;
    }

    let aspect = (rect.width() / rect.height().max(1.0)).max(0.1);
    let f = 1.0 / (VIEW_FOV_Y_RADIANS * 0.5).tan();

    let ndc_x = (view_x / view_z) * (f / aspect);
    let ndc_y = (view_y / view_z) * f;

    Some(ProjectedPoint {
        pos: Pos2::new(
            rect.center().x + ndc_x * rect.width() * 0.5,
            rect.center().y - ndc_y * rect.height() * 0.5,
        ),
        depth: view_z,
    })
}

fn graph_bounds(graph: &NodeGraph) -> Bounds3 {
    let mut min = [0.0, 0.0, -NODE_BOX_DEPTH * 0.5];
    let mut max = [0.0, 0.0, NODE_BOX_DEPTH * 0.5];

    let Some(first) = graph.nodes.first() else {
        return Bounds3 { min, max };
    };

    min[0] = first.position[0] - NODE_BOX_WIDTH * 0.5;
    max[0] = first.position[0] + NODE_BOX_WIDTH * 0.5;
    min[1] = first.position[1] - NODE_BOX_HEIGHT * 0.5;
    max[1] = first.position[1] + NODE_BOX_HEIGHT * 0.5;

    for node in &graph.nodes {
        min[0] = min[0].min(node.position[0] - NODE_BOX_WIDTH * 0.5);
        max[0] = max[0].max(node.position[0] + NODE_BOX_WIDTH * 0.5);
        min[1] = min[1].min(node.position[1] - NODE_BOX_HEIGHT * 0.5);
        max[1] = max[1].max(node.position[1] + NODE_BOX_HEIGHT * 0.5);
    }

    Bounds3 { min, max }
}

fn render_bounds(render_state: &RenderState) -> Option<Bounds3> {
    let first = render_state.meshes.first()?;
    let mut min = first.bounds.min;
    let mut max = first.bounds.max;

    for mesh in &render_state.meshes {
        min[0] = min[0].min(mesh.bounds.min[0]);
        min[1] = min[1].min(mesh.bounds.min[1]);
        min[2] = min[2].min(mesh.bounds.min[2]);
        max[0] = max[0].max(mesh.bounds.max[0]);
        max[1] = max[1].max(mesh.bounds.max[1]);
        max[2] = max[2].max(mesh.bounds.max[2]);
    }

    Some(Bounds3 { min, max })
}

fn draw_result_wireframe(
    render_state: &RenderState,
    camera: &CameraBasis,
    rect: Rect,
    painter: &Painter,
    selected_id: Option<&str>,
) {
    for mesh in &render_state.meshes {
        let selected = selected_id == Some(mesh.decl_id.as_str());
        let color = if selected {
            Color32::from_rgb(255, 214, 102)
        } else {
            Color32::from_gray(175)
        };
        let thickness = if selected { 1.8 } else { 0.8 };

        for tri in mesh.indices.chunks_exact(3) {
            let Some((a3, b3, c3)) = triangle_positions(mesh, tri) else {
                continue;
            };

            let Some(a) = project_point_to_rect(a3, camera, rect) else {
                continue;
            };
            let Some(b) = project_point_to_rect(b3, camera, rect) else {
                continue;
            };
            let Some(c) = project_point_to_rect(c3, camera, rect) else {
                continue;
            };

            painter.line_segment([a, b], Stroke::new(thickness, color));
            painter.line_segment([b, c], Stroke::new(thickness, color));
            painter.line_segment([c, a], Stroke::new(thickness, color));
        }
    }
}

fn draw_result_solid(
    render_state: &RenderState,
    camera: &CameraBasis,
    rect: Rect,
    painter: &Painter,
    selected_id: Option<&str>,
) {
    let light_dir = normalize3([0.45, 0.75, 0.55]);

    struct Triangle2d {
        points: [Pos2; 3],
        depth: f32,
        fill: Color32,
    }

    let mut triangles: Vec<Triangle2d> = Vec::new();
    for mesh in &render_state.meshes {
        let selected = selected_id == Some(mesh.decl_id.as_str());
        let base_color = if selected {
            Color32::from_rgb(255, 202, 92)
        } else {
            Color32::from_rgb(114, 152, 208)
        };

        for tri in mesh.indices.chunks_exact(3) {
            let Some((a3, b3, c3)) = triangle_positions(mesh, tri) else {
                continue;
            };

            let Some(a) = project_point_with_depth(a3, camera, rect) else {
                continue;
            };
            let Some(b) = project_point_with_depth(b3, camera, rect) else {
                continue;
            };
            let Some(c) = project_point_with_depth(c3, camera, rect) else {
                continue;
            };

            let screen_area = (b.pos.x - a.pos.x) * (c.pos.y - a.pos.y)
                - (b.pos.y - a.pos.y) * (c.pos.x - a.pos.x);
            if screen_area.abs() < 0.01 {
                continue;
            }

            let normal = normalize3(cross3(sub3(b3, a3), sub3(c3, a3)));
            let centroid = mul3(add3(add3(a3, b3), c3), 1.0 / 3.0);
            let to_camera = normalize3(sub3(camera.position, centroid));
            // Cull backfaces so openings (like holes) remain visible in solid mode.
            if dot3(normal, to_camera) <= 0.0 {
                continue;
            }

            let brightness = (dot3(normal, light_dir).max(0.0) * 0.7 + 0.25).clamp(0.0, 1.0);
            triangles.push(Triangle2d {
                points: [a.pos, b.pos, c.pos],
                depth: (a.depth + b.depth + c.depth) / 3.0,
                fill: shade_color(base_color, brightness),
            });
        }
    }

    triangles.sort_by(|a, b| {
        b.depth
            .partial_cmp(&a.depth)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for tri in triangles {
        painter.add(egui::Shape::convex_polygon(
            tri.points.to_vec(),
            tri.fill,
            Stroke::NONE,
        ));
    }

    if let Some(selected_id) = selected_id {
        for edges in &render_state.edges {
            if edges.decl_id != selected_id {
                continue;
            }
            for segment in edges.indices.chunks_exact(2) {
                let (Ok(i0), Ok(i1)) = (usize::try_from(segment[0]), usize::try_from(segment[1]))
                else {
                    continue;
                };
                let Some(a3) = edges.positions.get(i0).copied() else {
                    continue;
                };
                let Some(b3) = edges.positions.get(i1).copied() else {
                    continue;
                };
                let Some(a2) = project_point_to_rect(a3, camera, rect) else {
                    continue;
                };
                let Some(b2) = project_point_to_rect(b3, camera, rect) else {
                    continue;
                };
                painter.line_segment([a2, b2], Stroke::new(1.6, Color32::from_rgb(255, 245, 196)));
            }
        }
    }
}

fn triangle_positions(
    mesh: &puppycad_core::Mesh,
    tri: &[u32],
) -> Option<([f32; 3], [f32; 3], [f32; 3])> {
    let (Ok(i0), Ok(i1), Ok(i2)) = (
        usize::try_from(tri[0]),
        usize::try_from(tri[1]),
        usize::try_from(tri[2]),
    ) else {
        return None;
    };
    let a = *mesh.positions.get(i0)?;
    let b = *mesh.positions.get(i1)?;
    let c = *mesh.positions.get(i2)?;
    Some((a, b, c))
}

fn shade_color(color: Color32, factor: f32) -> Color32 {
    let factor = factor.clamp(0.0, 1.0);
    let [r, g, b, _] = color.to_array();
    let scale = |value: u8| -> u8 {
        let scaled = (value as f32 * factor).round().clamp(0.0, 255.0);
        scaled as u8
    };
    Color32::from_rgb(scale(r), scale(g), scale(b))
}

fn build_render_preview(ast: &File) -> Result<RenderPreview, String> {
    let feature_graph = FeatureGraph::new(ast);
    let model = build_model_state(&feature_graph)
        .map_err(|err| format!("failed to build model state: {err}"))?;
    Ok(RenderPreview {
        state: build_render_state(&model),
        warning: summarize_unsupported_preview_ops(&model),
    })
}

fn summarize_unsupported_preview_ops(model: &puppycad_core::ModelState) -> Option<String> {
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for node_id in &model.execution_order {
        let Some(node) = model.nodes.get(node_id) else {
            continue;
        };
        if matches!(node.op.as_str(), "box" | "translate" | "hole") {
            continue;
        }
        *counts.entry(node.op.clone()).or_insert(0) += 1;
    }

    if counts.is_empty() {
        return None;
    }

    let parts = counts
        .into_iter()
        .map(|(op, count)| format!("{op} x{count}"))
        .collect::<Vec<_>>();
    Some(format!("skipped unsupported op(s): {}", parts.join(", ")))
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul3(v: [f32; 3], scalar: f32) -> [f32; 3] {
    [v[0] * scalar, v[1] * scalar, v[2] * scalar]
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length3(v: [f32; 3]) -> f32 {
    dot3(v, v).sqrt()
}

fn normalize3(v: [f32; 3]) -> [f32; 3] {
    let len = length3(v);
    if len <= f32::EPSILON {
        [0.0, 0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len, v[2] / len]
    }
}

fn read_source(path: Option<&Path>) -> Result<String, String> {
    match path {
        Some(path) => std::fs::read_to_string(path)
            .map_err(|err| format!("failed to read '{}': {err}", path.to_string_lossy())),
        None => Ok(String::new()),
    }
}

fn kind_label(kind: DeclKind) -> &'static str {
    match kind {
        DeclKind::Solid => "solid",
        DeclKind::Feature => "feature",
    }
}

fn item_matches_filter(item: &ProjectItem, filter_query: &str) -> bool {
    let query = filter_query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return true;
    }

    item.id.to_ascii_lowercase().contains(&query)
        || item.op.to_ascii_lowercase().contains(&query)
        || kind_label(item.kind).contains(&query)
        || item
            .dependencies
            .iter()
            .any(|dependency| dependency.to_ascii_lowercase().contains(&query))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_graph() -> NodeGraph {
        NodeGraph {
            nodes: vec![
                puppycad_core::NodeGraphNode {
                    id: "base".to_owned(),
                    kind: DeclKind::Solid,
                    op: "box".to_owned(),
                    dependencies: vec![],
                    position: [0.0, 0.0],
                },
                puppycad_core::NodeGraphNode {
                    id: "hole_1".to_owned(),
                    kind: DeclKind::Feature,
                    op: "hole".to_owned(),
                    dependencies: vec!["base".to_owned()],
                    position: [5.0, 0.0],
                },
            ],
            edges: vec![puppycad_core::NodeGraphEdge {
                from: "base".to_owned(),
                to: "hole_1".to_owned(),
            }],
        }
    }

    fn fixture_items() -> Vec<ProjectItem> {
        fixture_graph()
            .nodes
            .iter()
            .map(ProjectItem::from_node)
            .collect()
    }

    #[test]
    fn filter_matches_id_op_kind_and_dependencies() {
        let items = fixture_items();
        assert!(item_matches_filter(&items[0], "base"));
        assert!(item_matches_filter(&items[0], "box"));
        assert!(item_matches_filter(&items[0], "solid"));
        assert!(item_matches_filter(&items[1], "feature"));
        assert!(item_matches_filter(&items[1], "base"));
        assert!(!item_matches_filter(&items[0], "missing"));
    }

    #[test]
    fn move_selection_stays_in_filtered_bounds() {
        let mut app = ProjectItemsApp::new(fixture_graph(), Err("no render".to_owned()));
        app.filter_query = String::new();
        app.selected_index = Some(0);

        let filtered = app.filtered_indices();
        app.move_selection(&filtered, 1);
        assert_eq!(app.selected_index, Some(1));

        app.move_selection(&filtered, 1);
        assert_eq!(app.selected_index, Some(1));

        app.move_selection(&filtered, -1);
        assert_eq!(app.selected_index, Some(0));
    }

    #[test]
    fn graph_bounds_contains_all_nodes() {
        let graph = fixture_graph();
        let bounds = graph_bounds(&graph);

        assert!(bounds.min[0] <= -NODE_BOX_WIDTH * 0.5);
        assert!(bounds.max[0] >= 5.0 + NODE_BOX_WIDTH * 0.5);
        assert!(bounds.min[1] <= -NODE_BOX_HEIGHT * 0.5);
        assert!(bounds.max[1] >= NODE_BOX_HEIGHT * 0.5);
    }

    #[test]
    fn viewer_initial_distance_is_positive() {
        let viewer = ViewerState::from_bounds(graph_bounds(&fixture_graph()));
        assert!(viewer.distance > 0.0);
    }

    #[test]
    fn defaults_to_graph_view_mode() {
        let app = ProjectItemsApp::new(fixture_graph(), Err("no render".to_owned()));
        assert_eq!(app.viewer_mode, ViewerMode::Graph);
        assert_eq!(app.result_render_mode, ResultRenderMode::Wireframe);
    }

    fn fixture_render_state() -> RenderState {
        RenderState {
            meshes: vec![puppycad_core::Mesh {
                decl_id: "base".to_owned(),
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
                tri_face_ids: vec![],
                bounds: puppycad_core::Aabb {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 0.0],
                },
            }],
            edges: vec![puppycad_core::Edges {
                decl_id: "base".to_owned(),
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
                indices: vec![0, 1],
                edge_ids: vec![],
            }],
            pick_map: vec![],
        }
    }

    #[test]
    fn wgpu_scene_contains_geometry_for_solid_mode() {
        let render_state = fixture_render_state();
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(640.0, 480.0));
        let viewer = ViewerState::from_bounds(Bounds3 {
            min: [0.0, 0.0, 0.0],
            max: [1.0, 1.0, 1.0],
        });
        let camera = viewer.camera_basis();

        let scene = build_result_wgpu_scene(
            &render_state,
            rect,
            &camera,
            ResultRenderMode::Solid,
            Some("base"),
        );

        assert_eq!(scene.draw_mode, WgpuResultDrawMode::Solid);
        assert_eq!(scene.triangle_indices.len(), 3);
        assert_eq!(scene.line_indices.len(), 2);
    }

    #[test]
    fn view_projection_matrix_is_finite() {
        let viewer = ViewerState::from_bounds(Bounds3 {
            min: [-1.0, -1.0, -1.0],
            max: [1.0, 1.0, 1.0],
        });
        let camera = viewer.camera_basis();
        let matrix = view_projection_matrix(&camera, 1.6);
        assert!(
            matrix
                .iter()
                .flatten()
                .all(|value| value.is_finite() && !value.is_nan())
        );
    }
}
