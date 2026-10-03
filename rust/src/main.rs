mod renderer;
mod view3d;

use puppycad::{model, project, spline, stl};

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use manifold_rust::manifold::Manifold;
use model::{
    AxisConstraint, DimensionField, Document, LineRelationKind, Point, Shape, SnapKind, Tool,
};
use project::{
    EntryKind, ProjectFile, ProjectSummary, SketchPlane, SolidPreview, list_projects,
    profiles_from_document,
};
use renderer::{Color, Painter, Rect, Renderer};
use view3d::OrbitView;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Window, WindowId},
};

const BG: Color = [0.055, 0.070, 0.096, 1.0];
const PANEL: Color = [0.090, 0.111, 0.145, 1.0];
const PANEL_LIGHT: Color = [0.124, 0.150, 0.190, 1.0];
const BORDER: Color = [0.20, 0.24, 0.29, 1.0];
const TEXT: Color = [0.88, 0.91, 0.94, 1.0];
const MUTED: Color = [0.50, 0.58, 0.65, 1.0];
const ACCENT: Color = [0.32, 0.78, 0.93, 1.0];
const SHAPE: Color = [0.96, 0.72, 0.38, 1.0];

#[derive(Clone, Copy, Debug)]
struct Layout {
    viewport: Rect,
    left: Rect,
    right: Rect,
    top: Rect,
    bottom: Rect,
}

impl Layout {
    fn new(width: f32, height: f32) -> Self {
        let left_width = 236.0_f32.min(width * 0.25);
        let right_width = 270.0_f32.min(width * 0.28);
        let top_height = 94.0;
        let bottom_height = 31.0;
        Self {
            viewport: Rect {
                x: left_width,
                y: top_height,
                w: (width - left_width - right_width).max(1.0),
                h: (height - top_height - bottom_height).max(1.0),
            },
            left: Rect {
                x: 0.0,
                y: top_height,
                w: left_width,
                h: height - top_height - bottom_height,
            },
            right: Rect {
                x: width - right_width,
                y: top_height,
                w: right_width,
                h: height - top_height - bottom_height,
            },
            top: Rect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: top_height,
            },
            bottom: Rect {
                x: 0.0,
                y: height - bottom_height,
                w: width,
                h: bottom_height,
            },
        }
    }

    fn projection_button(self, perspective: bool) -> Rect {
        Rect {
            x: self.top.w - 258.0 + if perspective { 0.0 } else { 152.0 },
            y: 8.0,
            w: if perspective { 148.0 } else { 94.0 },
            h: 28.0,
        }
    }

    fn tool_button(self, index: usize) -> Rect {
        Rect {
            x: 20.0 + index as f32 * 120.0,
            y: 47.0,
            w: 111.0,
            h: 32.0,
        }
    }

    fn more_item(self, index: usize) -> Rect {
        let button = self.tool_button(9);
        Rect {
            x: button.x,
            y: 96.0 + index as f32 * 36.0,
            w: button.w,
            h: 32.0,
        }
    }

    fn part_row(self, index: usize) -> Rect {
        Rect {
            x: 12.0,
            y: self.left.y + 43.0 + index as f32 * 35.0,
            w: self.left.w - 24.0,
            h: 31.0,
        }
    }

    fn sketch_row(self, index: usize, part_count: usize) -> Rect {
        self.part_row(index + part_count)
    }

    fn row(self, index: usize, part_count: usize, sketch_count: usize) -> Rect {
        Rect {
            x: 12.0,
            y: self.left.y
                + 87.0
                + (part_count + sketch_count.saturating_sub(1) + index) as f32 * 35.0,
            w: self.left.w - 24.0,
            h: 31.0,
        }
    }

    fn solid_row(self, index: usize, part_count: usize, sketch_count: usize) -> Rect {
        self.row(index, part_count, sketch_count)
    }
}

fn triangle_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<[f32; 3]> {
    let u = view3d::sub(b, a);
    let v = view3d::sub(c, a);
    let normal = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    (normal.iter().map(|c| c * c).sum::<f32>() > 1e-12).then_some(normal)
}

/// Evaluated solid bodies as world-space triangles (CAD millimeters, Z up).
struct SolidScene {
    bodies: usize,
    vertices: Vec<[f32; 3]>,
    triangles: Vec<[[f32; 3]; 3]>,
}

impl SolidScene {
    fn from_manifolds(solids: &[Manifold]) -> Self {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        for mesh in solids.iter().map(|solid| solid.get_mesh_gl(0)) {
            let points: Vec<[f32; 3]> = mesh
                .vert_properties
                .chunks_exact(mesh.num_prop as usize)
                .map(|p| [p[0], p[1], p[2]])
                .collect();
            for indices in mesh.tri_verts.as_chunks::<3>().0 {
                triangles.push(std::array::from_fn(|corner| {
                    points[indices[corner] as usize]
                }));
            }
            vertices.extend(points);
        }
        Self {
            bodies: solids.len(),
            vertices,
            triangles,
        }
    }
}

/// Projects the orbit view using a scene-fitted scale and optional perspective.
struct SolidProjector {
    view: OrbitView,
    center: [f32; 3],
    scale: f32,
    origin: [f32; 2],
    camera_distance: f32,
}

impl SolidProjector {
    fn new(view: OrbitView, vertices: &[[f32; 3]], viewport: Rect) -> Option<Self> {
        if vertices.is_empty() {
            return None;
        }
        let min: [f32; 3] = std::array::from_fn(|axis| {
            vertices
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min)
        });
        let max: [f32; 3] = std::array::from_fn(|axis| {
            vertices
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max)
        });
        let center = std::array::from_fn(|axis| (min[axis] + max[axis]) * 0.5);
        let radius = (0..3)
            .map(|axis| (max[axis] - min[axis]) * 0.5)
            .map(|half| half * half)
            .sum::<f32>()
            .sqrt()
            .max(1.0);
        Some(Self {
            view,
            center,
            camera_distance: radius * 4.0,
            scale: viewport.w.min(viewport.h) * 0.42 / radius * view.zoom,
            origin: [viewport.x + viewport.w * 0.5, viewport.y + viewport.h * 0.5],
        })
    }

    fn view_point(&self, world: [f32; 3]) -> [f32; 3] {
        self.view
            .model_to_view(view3d::cad_to_model(view3d::sub(world, self.center)))
    }

    fn perspective_factor(&self, depth: f32) -> f32 {
        if self.view.perspective {
            self.camera_distance / (self.camera_distance - depth).max(self.camera_distance * 0.01)
        } else {
            1.0
        }
    }

    fn screen(&self, world: [f32; 3]) -> [f32; 2] {
        let v = self.view_point(world);
        [
            self.origin[0] + v[0] * self.scale * self.perspective_factor(v[2]),
            self.origin[1] - v[1] * self.scale * self.perspective_factor(v[2]),
        ]
    }

    /// View-space depth; larger is nearer the viewer.
    fn depth(&self, world: [f32; 3]) -> f32 {
        self.view_point(world)[2]
    }

    /// Cosine between a world-space normal and the direction toward the viewer.
    fn facing(&self, normal: [f32; 3]) -> f32 {
        let n = self.view.rotate(view3d::cad_to_model(normal));
        n[2] / (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6)
    }

    fn facing_at(&self, normal: [f32; 3], world: [f32; 3]) -> f32 {
        if !self.view.perspective {
            return self.facing(normal);
        }
        let n = self.view.rotate(view3d::cad_to_model(normal));
        let p = self.view_point(world);
        let toward = [-p[0], -p[1], self.camera_distance - p[2]];
        let dot: f32 = (0..3).map(|i| n[i] * toward[i]).sum();
        let length = |v: [f32; 3]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (length(n) * length(toward)).max(1e-6)
    }

    fn units_per_pixel(&self) -> f32 {
        1.0 / (self.scale * self.perspective_factor(self.view.model_to_view(self.view.pivot)[2]))
    }

    /// Model point under screen `point` at view-space `depth`.
    fn model_at(&self, point: Point, depth: f32) -> [f32; 3] {
        self.view.view_to_model([
            (point.x - self.origin[0]) / (self.scale * self.perspective_factor(depth)),
            (self.origin[1] - point.y) / (self.scale * self.perspective_factor(depth)),
            depth,
        ])
    }

    /// Orbit pivot for a drag starting at `point`: the nearest surface under the cursor,
    /// or the current pivot depth when the cursor misses (web: `getOrbitAnchorPoint`).
    fn pick_pivot(&self, scene: &SolidScene, point: Point) -> [f32; 3] {
        let mut nearest: Option<f32> = None;
        for &[a, b, c] in &scene.triangles {
            let [pa, pb, pc] = [a, b, c].map(|p| (self.screen(p), self.depth(p)));
            let det = (pb.0[1] - pc.0[1]) * (pa.0[0] - pc.0[0])
                + (pc.0[0] - pb.0[0]) * (pa.0[1] - pc.0[1]);
            if det.abs() < 1e-9 {
                continue;
            }
            let wa = ((pb.0[1] - pc.0[1]) * (point.x - pc.0[0])
                + (pc.0[0] - pb.0[0]) * (point.y - pc.0[1]))
                / det;
            let wb = ((pc.0[1] - pa.0[1]) * (point.x - pc.0[0])
                + (pa.0[0] - pc.0[0]) * (point.y - pc.0[1]))
                / det;
            let wc = 1.0 - wa - wb;
            if wa < -1e-4 || wb < -1e-4 || wc < -1e-4 {
                continue;
            }
            let depth = if self.view.perspective {
                self.camera_distance
                    - 1.0
                        / (wa / (self.camera_distance - pa.1)
                            + wb / (self.camera_distance - pb.1)
                            + wc / (self.camera_distance - pc.1))
            } else {
                wa * pa.1 + wb * pb.1 + wc * pc.1
            };
            if nearest.is_none_or(|best| depth > best) {
                nearest = Some(depth);
            }
        }
        let depth = nearest.unwrap_or_else(|| view3d::add(self.view.pan, self.view.pivot)[2]);
        self.model_at(point, depth)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SolidDrag {
    Rotate,
    Pan { units_per_pixel: f32 },
}

const CRUMB_X: f32 = 172.0;
const CRUMB_Y: f32 = 17.0;
const CRUMB_SCALE: f32 = 1.4;
const CRUMB_SEPARATOR: &str = " / ";
const CRUMB_MAX_CHARS: usize = 24;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Crumb {
    Projects,
    Project,
    Part,
    Sketch,
    Solid,
    Shape,
}

/// Lays out breadcrumb labels left to right, truncating long names and
/// dropping segments that would run past `max_x`.
fn layout_breadcrumbs(segments: Vec<(Crumb, String)>, max_x: f32) -> Vec<(Crumb, String, Rect)> {
    let char_width = 8.0 * CRUMB_SCALE;
    let separator_width = CRUMB_SEPARATOR.chars().count() as f32 * char_width;
    let mut x = CRUMB_X;
    let mut crumbs = Vec::new();
    for (crumb, label) in segments {
        let label = if label.chars().count() > CRUMB_MAX_CHARS {
            format!(
                "{}...",
                label.chars().take(CRUMB_MAX_CHARS - 3).collect::<String>()
            )
        } else {
            label
        };
        let w = label.chars().count() as f32 * char_width;
        if x + w > max_x {
            break;
        }
        crumbs.push((
            crumb,
            label,
            Rect {
                x,
                y: CRUMB_Y - 4.0,
                w,
                h: 8.0 * CRUMB_SCALE + 8.0,
            },
        ));
        x += w + separator_width;
    }
    crumbs
}

const PROJECT_ROW_TOP: f32 = 176.0;
const PROJECT_ROW_STEP: f32 = 58.0;
const PROJECT_ROW_HEIGHT: f32 = 52.0;
const PROJECT_COLUMN_MAX: f32 = 960.0;

/// Left edge and width of the centered projects page column.
fn project_page_column(layout: Layout) -> (f32, f32) {
    let w = (layout.top.w - 40.0).clamp(1.0, PROJECT_COLUMN_MAX);
    ((layout.top.w - w) * 0.5, w)
}

/// Row `index` on screen of the projects page (after scrolling).
fn project_page_row(layout: Layout, index: usize) -> Rect {
    let (x, w) = project_page_column(layout);
    Rect {
        x,
        y: PROJECT_ROW_TOP + index as f32 * PROJECT_ROW_STEP,
        w,
        h: PROJECT_ROW_HEIGHT,
    }
}

fn project_page_visible_rows(layout: Layout) -> usize {
    ((layout.bottom.y - 12.0 - PROJECT_ROW_TOP + PROJECT_ROW_STEP - PROJECT_ROW_HEIGHT)
        / PROJECT_ROW_STEP)
        .floor()
        .max(1.0) as usize
}

/// Shortens `text` to `max` characters, keeping the end (for paths) or the start.
fn clip_text(text: &str, max: usize, keep_end: bool) -> String {
    let count = text.chars().count();
    if count <= max || max < 4 {
        return text.to_string();
    }
    if keep_end {
        format!(
            "...{}",
            text.chars().skip(count + 3 - max).collect::<String>()
        )
    } else {
        format!("{}...", text.chars().take(max - 3).collect::<String>())
    }
}

struct ProjectsPage {
    dir: PathBuf,
    entries: Vec<ProjectSummary>,
    scroll: usize,
    selected: usize,
    /// Project the user picked once while the open project had unsaved changes.
    pending_discard: Option<PathBuf>,
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    document: Document,
    project: Option<ProjectFile>,
    status_message: String,
    cursor: Point,
    pan_drag: bool,
    modifiers: ModifiersState,
    editing: Option<DimensionEdit>,
    name_edit: Option<NameEdit>,
    more_open: bool,
    move_offset: Point,
    scale_factor: f32,
    rotate_angle: f32,
    transform_rotate: bool,
    transform_center: Point,
    solid_preview: bool,
    solid_drag: Option<SolidDrag>,
    solid_view: OrbitView,
    selected_solid: usize,
    selected_profile: usize,
    snap_grid: bool,
    snap_geometry: bool,
    split_circle_start: Option<(u32, Point)>,
    spline_points: Vec<Point>,
    projects_page: Option<ProjectsPage>,
}

struct DimensionEdit {
    field: EditField,
    buffer: String,
}

struct NameEdit {
    target: NameTarget,
    buffer: String,
}

#[derive(Clone, Copy)]
enum NameTarget {
    Part,
    Sketch,
}

#[derive(Clone, Copy)]
enum EditField {
    ExtrusionDepth,
    Dimension(DimensionField),
    PolygonSides,
    SlotWidth,
    MoveX,
    MoveY,
    ScaleFactor,
    RotateAngle,
    CenterX,
    CenterY,
    SplineX,
    SplineY,
    PointX,
    PointY,
    RelationAngle,
    RadialDifference,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            renderer: None,
            document: Document::default(),
            project: None,
            status_message: String::new(),
            cursor: Point { x: 0.0, y: 0.0 },
            pan_drag: false,
            modifiers: ModifiersState::empty(),
            editing: None,
            name_edit: None,
            more_open: false,
            move_offset: Point { x: 10.0, y: -10.0 },
            scale_factor: 1.5,
            rotate_angle: 30.0,
            transform_rotate: false,
            transform_center: Point { x: 0.0, y: 0.0 },
            solid_preview: false,
            solid_drag: None,
            solid_view: OrbitView::default(),
            selected_solid: 0,
            selected_profile: 0,
            snap_grid: true,
            snap_geometry: true,
            split_circle_start: None,
            spline_points: Vec::new(),
            projects_page: None,
        }
    }
}

impl App {
    fn switch_part(&mut self, direction: isize) {
        let Some(project) = &self.project else {
            return;
        };
        let indices = project.part_indices();
        if indices.len() < 2 {
            return;
        }
        let current = indices
            .iter()
            .position(|index| *index == project.active_part_index())
            .unwrap_or(0);
        let target =
            indices[(current as isize + direction).rem_euclid(indices.len() as isize) as usize];
        self.switch_part_to(target);
    }

    fn switch_part_to(&mut self, target: usize) {
        self.document.finish_drag();
        self.split_circle_start = None;
        let Some(project) = &mut self.project else {
            return;
        };
        match project.switch_part(target, &self.document) {
            Ok(document) => {
                self.document = document;
                self.selected_solid = 0;
                self.selected_profile = 0;
                self.solid_preview = project.extrusion_previews().is_some();
                self.editing = None;
                self.status_message = format!("Part: {}  |  Ctrl+S to save", project.part_name());
            }
            Err(error) => self.status_message = format!("Switch part: {error}"),
        }
    }

    fn export_stl(&mut self) {
        self.status_message = match &self.project {
            Some(project) => match project.extrusion_previews() {
                Some(solids) => {
                    let mut paths = Vec::new();
                    let mut error = None;
                    let meshes = project.evaluated_solids();
                    for index in 0..meshes.as_ref().map_or(solids.len(), Vec::len) {
                        let result = if meshes
                            .as_ref()
                            .is_some_and(|items| items.len() < solids.len())
                        {
                            stl::write_mesh_export(project.path(), &meshes.as_ref().unwrap()[index])
                        } else {
                            stl::write_export(project.path(), &solids[index])
                        };
                        match result {
                            Ok(path) => paths.push(path),
                            Err(reason) => {
                                error = Some(reason);
                                break;
                            }
                        }
                    }
                    match error {
                        Some(reason) => {
                            format!("STL export failed after {} bodies: {reason}", paths.len())
                        }
                        None => format!(
                            "Exported {} STL {}",
                            paths.len(),
                            if paths.len() == 1 { "body" } else { "bodies" }
                        ),
                    }
                }
                None => "No supported solid to export".into(),
            },
            None => "Open a .pcad project to export STL".into(),
        };
    }

    fn create_extrusion(&mut self, operation: &str) {
        self.document.finish_drag();
        if let Some(profiles) = profiles_from_document(&self.document) {
            self.selected_profile = self.selected_profile.min(profiles.len().saturating_sub(1));
        }
        self.status_message = match &mut self.project {
            Some(project) => match if operation == "newBody" {
                project.create_extrusion_for_profile(&self.document, 50.0, self.selected_profile)
            } else {
                project.create_extrusion_with_operation(
                    &self.document,
                    50.0,
                    self.selected_profile,
                    operation,
                )
            } {
                Ok(()) => {
                    self.selected_solid = project
                        .extrusion_previews()
                        .map_or(0, |solids| solids.len() - 1);
                    self.solid_preview = true;
                    format!(
                        "Created 50 mm {operation} extrusion  |  Ctrl+S to save  |  Tab: sketch"
                    )
                }
                Err(error) => format!("Extrude: {error}"),
            },
            None => "Open a .pcad project to create an extrusion".into(),
        };
        self.more_open = false;
    }

    fn select_tool(&mut self, tool: Tool) {
        self.document.finish_drag();
        self.split_circle_start = None;
        if self.document.tool != tool {
            self.document.draft_start = None;
            self.document.draft_end = None;
            self.spline_points.clear();
        }
        self.document.tool = tool;
        self.more_open = false;
    }

    fn finish_spline(&mut self, closed: bool) {
        let mode = match self.document.tool {
            Tool::FitSpline => spline::SplineMode::Fit,
            Tool::ControlSpline => spline::SplineMode::Control,
            _ => return,
        };
        match self
            .document
            .add_spline(mode, self.spline_points.clone(), closed)
        {
            Ok(_) => {
                self.spline_points.clear();
                self.status_message = "Spline created  |  Ctrl+Z to undo  |  Ctrl+S to save".into();
            }
            Err(error) => self.status_message = format!("Spline: {error}"),
        }
    }

    fn breadcrumbs(&self, layout: Layout) -> Vec<(Crumb, String, Rect)> {
        let mut segments = vec![(Crumb::Projects, "Projects".to_string())];
        if self.projects_page.is_some() {
            return layout_breadcrumbs(segments, layout.top.w - 20.0);
        }
        let project_name = self
            .project
            .as_ref()
            .and_then(|project| project.path().file_stem())
            .and_then(|name| name.to_str())
            .unwrap_or("UNTITLED");
        segments.push((Crumb::Project, project_name.to_string()));
        if let Some(project) = &self.project {
            segments.push((Crumb::Part, project.part_name().to_string()));
        }
        if self.solid_preview {
            if let Some(solid) = self
                .project
                .as_ref()
                .and_then(ProjectFile::extrusion_previews)
                .and_then(|solids| solids.into_iter().nth(self.selected_solid))
            {
                segments.push((Crumb::Solid, solid.name));
            }
        } else {
            let sketch_name = self
                .project
                .as_ref()
                .and_then(|project| project.sketch_name(project.active_sketch()))
                .unwrap_or("Sketch");
            segments.push((Crumb::Sketch, sketch_name.to_string()));
            if let Some(shape) = self
                .document
                .selected
                .and_then(|id| self.document.shapes.iter().find(|shape| shape.id() == id))
            {
                segments.push((Crumb::Shape, format!("{} {}", shape.name(), shape.id())));
            }
        }
        layout_breadcrumbs(
            segments,
            if self.solid_preview {
                layout.top.w - 270.0
            } else {
                layout.top.w - 20.0
            },
        )
    }

    fn open_projects_page(&mut self) {
        self.document.finish_drag();
        self.editing = None;
        self.name_edit = None;
        self.more_open = false;
        self.solid_drag = None;
        self.split_circle_start = None;
        let open_path = self
            .project
            .as_ref()
            .map(|project| project.path().to_path_buf());
        let dir = match open_path.as_deref().and_then(Path::parent) {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        self.show_folder(&dir, open_path.as_deref());
    }

    /// Lists `dir` on the projects page, selecting `focus` when it is listed there.
    fn show_folder(&mut self, dir: &Path, focus: Option<&Path>) {
        let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        let entries = match list_projects(&dir) {
            Ok(entries) => entries,
            Err(error) => {
                self.status_message = format!("Projects: {error}");
                return;
            }
        };
        let focus = focus.and_then(|focus| focus.canonicalize().ok());
        let selected = entries
            .iter()
            .position(|entry| {
                focus.is_some()
                    && entry.kind != EntryKind::Parent
                    && entry.path.canonicalize().ok() == focus
            })
            .or_else(|| {
                entries
                    .iter()
                    .position(|entry| entry.kind == EntryKind::Project)
            })
            .unwrap_or(0);
        self.status_message =
            "Click or Enter: open  |  Up/Down: select  |  Backspace: parent folder  |  Esc: back"
                .into();
        self.projects_page = Some(ProjectsPage {
            dir,
            entries,
            scroll: 0,
            selected,
            pending_discard: None,
        });
        if let Some(window) = &self.window {
            let size = window.inner_size();
            self.reveal_selected_project(Layout::new(size.width as f32, size.height as f32));
        }
    }

    fn show_parent_folder(&mut self) {
        let Some(page) = &self.projects_page else {
            return;
        };
        if let Some(parent) = page.dir.parent().map(Path::to_path_buf) {
            let from = page.dir.clone();
            self.show_folder(&parent, Some(&from));
        }
    }

    fn close_projects_page(&mut self) {
        if self.projects_page.take().is_some() {
            self.status_message = match &self.project {
                Some(project) => format!("Project: {}", project.path().display()),
                None => String::new(),
            };
        }
    }

    fn has_unsaved_work(&self) -> bool {
        match &self.project {
            Some(project) => project.has_unsaved_changes(&self.document),
            None => !self.document.shapes.is_empty(),
        }
    }

    fn is_open_project(&self, path: &Path) -> bool {
        self.project.as_ref().is_some_and(|project| {
            project.path() == path
                || project
                    .path()
                    .canonicalize()
                    .ok()
                    .zip(path.canonicalize().ok())
                    .is_some_and(|(open, listed)| open == listed)
        })
    }

    fn open_listed_project(&mut self, index: usize) {
        let Some(entry) = self
            .projects_page
            .as_ref()
            .and_then(|page| page.entries.get(index))
            .cloned()
        else {
            return;
        };
        if let Some(page) = &mut self.projects_page {
            page.selected = index;
        }
        match entry.kind {
            EntryKind::Parent => {
                self.show_parent_folder();
                return;
            }
            EntryKind::Folder => {
                self.show_folder(&entry.path, None);
                return;
            }
            EntryKind::Project => {}
        }
        if let Some(error) = &entry.error {
            self.status_message = format!("Cannot open {}: {error}", entry.name);
            return;
        }
        if self.is_open_project(&entry.path) {
            self.close_projects_page();
            return;
        }
        let confirmed = self
            .projects_page
            .as_ref()
            .and_then(|page| page.pending_discard.as_deref())
            == Some(entry.path.as_path());
        if self.has_unsaved_work() && !confirmed {
            if let Some(page) = &mut self.projects_page {
                page.pending_discard = Some(entry.path.clone());
            }
            self.status_message = format!(
                "Unsaved changes  |  Ctrl+S to save, or open {} again to discard them",
                entry.name
            );
            return;
        }
        self.load_project(&entry.path);
    }

    fn solid_projector(&self, layout: Layout) -> Option<(SolidScene, SolidProjector)> {
        let solids = self.project.as_ref()?.evaluated_solids()?;
        let scene = SolidScene::from_manifolds(&solids);
        let projector = SolidProjector::new(self.solid_view, &scene.vertices, layout.viewport)?;
        Some((scene, projector))
    }

    /// Starts a 3D preview drag for a mouse button pressed at `point`, like the web part
    /// view: right-drag orbits around the surface under the cursor, middle-drag pans.
    fn begin_solid_drag(&mut self, layout: Layout, point: Point, button: MouseButton) -> bool {
        self.solid_drag = None;
        if !self.solid_preview
            || self.projects_page.is_some()
            || !layout.viewport.contains(point.x, point.y)
        {
            return false;
        }
        let Some((scene, projector)) = self.solid_projector(layout) else {
            return false;
        };
        self.solid_drag = match button {
            MouseButton::Right => {
                self.solid_view
                    .set_pivot(projector.pick_pivot(&scene, point));
                Some(SolidDrag::Rotate)
            }
            MouseButton::Middle => Some(SolidDrag::Pan {
                units_per_pixel: projector.units_per_pixel(),
            }),
            _ => None,
        };
        self.solid_drag.is_some()
    }

    fn select_project_row(&mut self, step: isize, layout: Layout) {
        if let Some(page) = &mut self.projects_page {
            if page.entries.is_empty() {
                return;
            }
            page.selected = page
                .selected
                .saturating_add_signed(step)
                .min(page.entries.len() - 1);
        }
        self.reveal_selected_project(layout);
    }

    fn reveal_selected_project(&mut self, layout: Layout) {
        if let Some(page) = &mut self.projects_page {
            let visible = project_page_visible_rows(layout);
            if page.selected < page.scroll {
                page.scroll = page.selected;
            } else if page.selected >= page.scroll + visible {
                page.scroll = page.selected + 1 - visible;
            }
        }
    }

    /// Replaces the open project with `path`, keeping the window and input state.
    fn load_project(&mut self, path: &Path) {
        match App::with_project(path) {
            Ok(mut next) => {
                next.window = self.window.take();
                next.renderer = self.renderer.take();
                next.cursor = self.cursor;
                next.modifiers = self.modifiers;
                *self = next;
            }
            Err(error) => self.status_message = format!("Open failed: {error}"),
        }
    }

    fn save_project(&mut self) {
        self.status_message = match &mut self.project {
            Some(project) => match project.save(&self.document) {
                Ok(()) => format!("Saved {}", project.path().display()),
                Err(error) => format!("Save failed: {error}"),
            },
            None => "Start with: puppycad FILE.pcad to save".into(),
        };
        if let Some(page) = &mut self.projects_page {
            page.pending_discard = None;
        }
    }

    fn scroll_projects_page(&mut self, rows: isize, layout: Layout) {
        if let Some(page) = &mut self.projects_page {
            let max = page
                .entries
                .len()
                .saturating_sub(project_page_visible_rows(layout));
            page.scroll = page.scroll.saturating_add_signed(rows).min(max);
        }
    }

    fn projects_page_entry_at(&self, layout: Layout, point: Point) -> Option<usize> {
        let page = self.projects_page.as_ref()?;
        (0..project_page_visible_rows(layout))
            .find(|row| project_page_row(layout, *row).contains(point.x, point.y))
            .map(|row| row + page.scroll)
            .filter(|index| *index < page.entries.len())
    }

    /// Returns the ancestor breadcrumb under `point`; the last (current) crumb is not a link.
    fn breadcrumb_at(&self, layout: Layout, point: Point) -> Option<Crumb> {
        let crumbs = self.breadcrumbs(layout);
        let last = crumbs.len().saturating_sub(1);
        crumbs
            .into_iter()
            .take(last)
            .find(|(_, _, rect)| rect.contains(point.x, point.y))
            .map(|(crumb, _, _)| crumb)
    }

    fn open_breadcrumb(&mut self, crumb: Crumb) {
        if crumb == Crumb::Projects {
            self.open_projects_page();
            return;
        }
        self.document.finish_drag();
        self.editing = None;
        self.split_circle_start = None;
        self.document.selected = None;
        self.document.selected_spline_point = None;
        let has_solids = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .is_some();
        self.status_message = match (crumb, &self.project) {
            (Crumb::Project, Some(project)) => {
                format!("Project: {}", project.path().display())
            }
            (Crumb::Project, None) => "Untitled sketch  |  open a .pcad project to save".into(),
            (Crumb::Part, Some(project)) => {
                self.solid_preview = has_solids;
                if has_solids {
                    format!("Part: {}  |  Tab: sketch", project.part_name())
                } else {
                    format!("Part: {}", project.part_name())
                }
            }
            (Crumb::Sketch, _) => {
                self.solid_preview = false;
                format!(
                    "Editing {}",
                    self.project
                        .as_ref()
                        .and_then(|project| project.sketch_name(project.active_sketch()))
                        .unwrap_or("Sketch")
                )
            }
            _ => self.status_message.clone(),
        };
    }

    fn sketch_count(&self) -> usize {
        self.project.as_ref().map_or(1, ProjectFile::sketch_count)
    }

    fn part_rows(&self) -> Vec<usize> {
        self.project
            .as_ref()
            .map(ProjectFile::part_indices)
            .filter(|indices| indices.len() > 1)
            .unwrap_or_default()
    }

    fn create_sketch(&mut self, plane: SketchPlane) {
        self.split_circle_start = None;
        if self
            .project
            .as_ref()
            .is_some_and(|project| project.read_only_reason().is_some())
        {
            self.status_message = "READ ONLY: solid geometry is locked".into();
            return;
        }
        self.document.finish_drag();
        if let Some(project) = &mut self.project {
            match project.create_sketch_on_plane(&self.document, plane) {
                Ok(document) => {
                    self.document = document;
                    self.solid_preview = false;
                    self.selected_profile = 0;
                    self.editing = None;
                    self.status_message = format!(
                        "Created {}  |  Ctrl+S to save",
                        project
                            .sketch_name(project.active_sketch())
                            .unwrap_or("Sketch")
                    );
                }
                Err(error) => self.status_message = error,
            }
        } else {
            self.status_message = "Open a .pcad project to add a sketch".into();
        }
    }

    fn create_part(&mut self) {
        self.document.finish_drag();
        self.status_message = match &mut self.project {
            Some(project) => match project.create_part(&self.document) {
                Ok(document) => {
                    self.document = document;
                    self.solid_preview = false;
                    self.selected_profile = 0;
                    self.editing = None;
                    format!("Created {}  |  Ctrl+S to save", project.part_name())
                }
                Err(error) => format!("New part: {error}"),
            },
            None => "Open a .pcad project to add a part".into(),
        };
        self.more_open = false;
    }

    fn begin_rename(&mut self, target: NameTarget) {
        if self.project.is_none() {
            self.status_message = "Open a .pcad project to rename a part or sketch".into();
            return;
        }
        self.editing = None;
        self.name_edit = Some(NameEdit {
            target,
            buffer: String::new(),
        });
    }

    fn finish_rename(&mut self) {
        let Some(edit) = self.name_edit.take() else {
            return;
        };
        self.status_message = match &mut self.project {
            Some(project) => {
                let result = match edit.target {
                    NameTarget::Part => project.rename_part(&edit.buffer),
                    NameTarget::Sketch => project.rename_sketch(&edit.buffer),
                };
                match result {
                    Ok(()) => format!("Renamed to {}  |  Ctrl+S to save", edit.buffer.trim()),
                    Err(error) => format!("Rename failed: {error}"),
                }
            }
            None => "Open a .pcad project to rename".into(),
        };
    }

    fn with_project(path: &Path) -> Result<Self, String> {
        let (project, document) = ProjectFile::open(path)?;
        let solid_preview = project.extrusion_previews().is_some();
        let status_message = if solid_preview {
            "3D extrusion preview  |  Tab: sketch".into()
        } else {
            project
                .read_only_reason()
                .map(|reason| format!("READ ONLY: {reason}"))
                .unwrap_or_else(|| format!("Project: {}", project.path().display()))
        };
        Ok(Self {
            document,
            project: Some(project),
            status_message,
            solid_preview,
            ..Self::default()
        })
    }

    fn read_only(&self) -> bool {
        self.project.as_ref().is_some_and(|project| {
            project.read_only_reason().is_some() || project.active_sketch_has_extrusion()
        })
    }

    fn cycle_solid(&mut self, direction: isize) {
        let Some(solids) = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
        else {
            return;
        };
        self.selected_solid =
            (self.selected_solid as isize + direction).rem_euclid(solids.len() as isize) as usize;
        self.status_message = format!(
            "Selected {}  |  Click DEPTH to edit",
            solids[self.selected_solid].name
        );
    }

    fn editable_field_at(&self, point: Point) -> Option<EditField> {
        let layout = self.layout();
        if !layout.right.contains(point.x, point.y) {
            return None;
        }
        if self.solid_preview
            && self
                .project
                .as_ref()
                .and_then(ProjectFile::extrusion_previews)
                .is_some()
        {
            return (layout.right.y + 105.0..layout.right.y + 130.0)
                .contains(&point.y)
                .then_some(EditField::ExtrusionDepth);
        }
        let shape = self
            .document
            .shapes
            .iter()
            .find(|shape| Some(shape.id()) == self.document.selected)?;
        if matches!(shape, Shape::Point { .. }) {
            if (layout.right.y + 105.0..layout.right.y + 130.0).contains(&point.y) {
                return Some(EditField::PointX);
            }
            if (layout.right.y + 135.0..layout.right.y + 160.0).contains(&point.y) {
                return Some(EditField::PointY);
            }
        }
        if matches!(shape, Shape::Spline { .. }) && self.document.selected_spline_point.is_some() {
            if (layout.right.y + 195.0..layout.right.y + 220.0).contains(&point.y) {
                return Some(EditField::SplineX);
            }
            if (layout.right.y + 225.0..layout.right.y + 250.0).contains(&point.y) {
                return Some(EditField::SplineY);
            }
        }
        if self
            .document
            .line_relations
            .get(&shape.id())
            .is_some_and(|(_, kind)| matches!(kind, LineRelationKind::Angle(_)))
            && (layout.right.y + 312.0..layout.right.y + 337.0).contains(&point.y)
        {
            return Some(EditField::RelationAngle);
        }
        if self.document.radius_differences.contains_key(&shape.id())
            && (layout.right.y + 312.0..layout.right.y + 337.0).contains(&point.y)
        {
            return Some(EditField::RadialDifference);
        }
        if (layout.right.y + 365.0..layout.right.y + 390.0).contains(&point.y) {
            return Some(EditField::MoveX);
        }
        if (layout.right.y + 395.0..layout.right.y + 420.0).contains(&point.y) {
            return Some(EditField::MoveY);
        }
        if (layout.right.y + 550.0..layout.right.y + 575.0).contains(&point.y) {
            return Some(if point.x < layout.right.x + layout.right.w * 0.5 {
                EditField::CenterX
            } else {
                EditField::CenterY
            });
        }
        if (layout.right.y + 580.0..layout.right.y + 605.0).contains(&point.y) {
            return Some(if self.transform_rotate {
                EditField::RotateAngle
            } else {
                EditField::ScaleFactor
            });
        }
        let row = ((point.y - layout.right.y - 105.0) / 30.0).floor() as i32;
        let row_top = layout.right.y + 105.0 + row as f32 * 30.0;
        if point.y < row_top || point.y >= row_top + 25.0 {
            return None;
        }
        match (shape, row) {
            (Shape::Line { .. }, 0) => Some(EditField::Dimension(DimensionField::LineLength)),
            (Shape::Rectangle { .. } | Shape::RotatedRectangle { .. }, 0) => {
                Some(EditField::Dimension(DimensionField::RectangleWidth))
            }
            (Shape::Rectangle { .. } | Shape::RotatedRectangle { .. }, 1) => {
                Some(EditField::Dimension(DimensionField::RectangleHeight))
            }
            (Shape::Ellipse { .. } | Shape::EllipticArc { .. }, 0) => {
                Some(EditField::Dimension(DimensionField::RectangleWidth))
            }
            (Shape::Ellipse { .. } | Shape::EllipticArc { .. }, 1) => {
                Some(EditField::Dimension(DimensionField::RectangleHeight))
            }
            (
                Shape::RotatedRectangle { .. }
                | Shape::Ellipse { .. }
                | Shape::EllipticArc { .. }
                | Shape::Polygon { .. },
                2,
            ) => Some(EditField::Dimension(DimensionField::Rotation)),
            (Shape::Circle { .. }, 0) => Some(EditField::Dimension(DimensionField::CircleDiameter)),
            (Shape::Arc { .. }, 0) => Some(EditField::Dimension(DimensionField::CurveRadius)),
            (Shape::Arc { .. }, 1) | (Shape::EllipticArc { .. }, 4) => {
                Some(EditField::Dimension(DimensionField::ArcSweep))
            }
            (Shape::Polygon { .. }, 0) => Some(EditField::PolygonSides),
            (Shape::Polygon { .. }, 1) => Some(EditField::Dimension(DimensionField::CurveRadius)),
            (Shape::Capsule { .. }, 0) => Some(EditField::SlotWidth),
            _ => None,
        }
    }

    fn finish_dimension_edit(&mut self) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        self.status_message = match edit.field {
            EditField::ExtrusionDepth => match edit.buffer.parse::<f32>() {
                Ok(depth) => match self
                    .project
                    .as_mut()
                    .expect("solid editor has project")
                    .set_extrusion_depth_at(self.selected_solid, depth)
                {
                    Ok(()) => format!("Extrusion depth set to {depth:.1} mm  |  Ctrl+S to save"),
                    Err(error) => format!("Extrusion error: {error}"),
                },
                Err(_) => "Extrusion error: enter a positive depth in millimeters".into(),
            },
            EditField::Dimension(field) => match edit.buffer.parse::<f32>() {
                Ok(value) => match self.document.set_dimension(field, value) {
                    Ok(()) => format!("Dimension set to {value:.1} mm  |  Ctrl+S to save"),
                    Err(error) => format!("Dimension error: {error}"),
                },
                Err(_) => "Dimension error: enter a number in millimeters".into(),
            },
            EditField::RelationAngle => match edit.buffer.parse::<f32>() {
                Ok(value) => match self.document.set_line_relation_angle(value) {
                    Ok(()) => format!("Line angle set to {value:.1} deg  |  Ctrl+S to save"),
                    Err(error) => format!("Angle error: {error}"),
                },
                Err(_) => "Angle error: enter signed degrees".into(),
            },
            EditField::RadialDifference => match edit.buffer.parse::<f32>() {
                Ok(value) => match self.document.set_radius_difference(value) {
                    Ok(()) => format!("Radial distance set to {value:.1} mm  |  Ctrl+S to save"),
                    Err(error) => format!("Radial distance error: {error}"),
                },
                Err(_) => "Radial distance error: enter a signed number".into(),
            },
            EditField::PolygonSides => match edit.buffer.parse::<u8>() {
                Ok(sides) => match self.document.set_polygon_sides(sides) {
                    Ok(()) => format!("Polygon set to {sides} sides  |  Ctrl+S to save"),
                    Err(error) => format!("Polygon error: {error}"),
                },
                Err(_) => "Polygon error: enter an integer from 3 to 64".into(),
            },
            EditField::SlotWidth => match edit.buffer.parse::<f32>() {
                Ok(width) => match self.document.set_capsule_width(width) {
                    Ok(()) => format!("Slot width set to {width:.1} mm  |  Ctrl+S to save"),
                    Err(error) => format!("Slot error: {error}"),
                },
                Err(_) => "Slot error: enter a width in millimeters".into(),
            },
            EditField::MoveX | EditField::MoveY => match edit.buffer.parse::<f32>() {
                Ok(value) if value.is_finite() => {
                    if matches!(edit.field, EditField::MoveX) {
                        self.move_offset.x = value;
                    } else {
                        self.move_offset.y = value;
                    }
                    format!(
                        "Move offset: X {:.1}, Y {:.1} mm",
                        self.move_offset.x, self.move_offset.y
                    )
                }
                _ => "Move offset error: enter a finite number".into(),
            },
            EditField::ScaleFactor => match edit.buffer.parse::<f32>() {
                Ok(value) if value.is_finite() && value > 0.0 => {
                    self.scale_factor = value;
                    format!("Scale factor: {value:.2}")
                }
                _ => "Scale error: enter a positive finite factor".into(),
            },
            EditField::RotateAngle => match edit.buffer.parse::<f32>() {
                Ok(value) if value.is_finite() => {
                    self.rotate_angle = value;
                    format!("Rotate angle: {value:.1} degrees")
                }
                _ => "Rotate error: enter a finite angle".into(),
            },
            EditField::CenterX | EditField::CenterY => match edit.buffer.parse::<f32>() {
                Ok(value) if value.is_finite() => {
                    if matches!(edit.field, EditField::CenterX) {
                        self.transform_center.x = value;
                    } else {
                        self.transform_center.y = value;
                    }
                    format!(
                        "Transform center: X {:.1}, Y {:.1} mm",
                        self.transform_center.x, self.transform_center.y
                    )
                }
                _ => "Center error: enter a finite coordinate".into(),
            },
            EditField::SplineX | EditField::SplineY => match edit.buffer.parse::<f32>() {
                Ok(value) => match self
                    .document
                    .set_selected_spline_coordinate(matches!(edit.field, EditField::SplineX), value)
                {
                    Ok(()) => {
                        format!("Spline point coordinate set to {value:.1} mm  |  Ctrl+S to save")
                    }
                    Err(error) => format!("Spline point error: {error}"),
                },
                Err(_) => "Spline point error: enter a finite coordinate".into(),
            },
            EditField::PointX | EditField::PointY => match edit.buffer.parse::<f32>() {
                Ok(value) => match self
                    .document
                    .set_selected_point_coordinate(matches!(edit.field, EditField::PointX), value)
                {
                    Ok(()) => format!("Point coordinate set to {value:.1} mm  |  Ctrl+S to save"),
                    Err(error) => format!("Point error: {error}"),
                },
                Err(_) => "Point error: enter a finite coordinate".into(),
            },
        };
    }

    fn layout(&self) -> Layout {
        let size = self.window.as_ref().expect("window exists").inner_size();
        Layout::new(size.width as f32, size.height as f32)
    }

    fn redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn viewport_origin(&self) -> Point {
        let layout = self.layout();
        Point {
            x: layout.viewport.x + layout.viewport.w * 0.5,
            y: layout.viewport.y + layout.viewport.h * 0.5,
        }
    }

    fn select_projection(&mut self, layout: Layout, point: Point) -> bool {
        if !self.solid_preview || self.projects_page.is_some() {
            return false;
        }
        for perspective in [true, false] {
            if layout
                .projection_button(perspective)
                .contains(point.x, point.y)
            {
                self.solid_view.perspective = perspective;
                self.solid_drag = None;
                self.status_message = if perspective {
                    "Perspective view"
                } else {
                    "Orthographic view"
                }
                .into();
                return true;
            }
        }
        false
    }

    fn handle_left_press(&mut self) {
        self.name_edit = None;
        let layout = self.layout();
        let point = self.cursor;
        if self.select_projection(layout, point) {
            self.redraw();
            return;
        }
        if let Some(crumb) = self.breadcrumb_at(layout, point) {
            self.more_open = false;
            self.open_breadcrumb(crumb);
            self.redraw();
            return;
        }
        if self.projects_page.is_some() {
            if let Some(index) = self.projects_page_entry_at(layout, point) {
                self.open_listed_project(index);
            }
            self.redraw();
            return;
        }
        if layout.tool_button(9).contains(point.x, point.y) {
            self.more_open = !self.more_open;
            self.redraw();
            return;
        }
        if self.more_open {
            if layout.more_item(3).contains(point.x, point.y) {
                self.create_extrusion("newBody");
                self.redraw();
                return;
            }
            if layout.more_item(4).contains(point.x, point.y) {
                self.create_part();
                self.redraw();
                return;
            }
            if layout.more_item(5).contains(point.x, point.y) {
                self.select_tool(Tool::ThreePointCircle);
                self.redraw();
                return;
            }
            if layout.more_item(6).contains(point.x, point.y) {
                self.select_tool(Tool::CenterArc);
                self.redraw();
                return;
            }
            if layout.more_item(7).contains(point.x, point.y) {
                self.select_tool(Tool::CenterRectangle);
                self.redraw();
                return;
            }
            if layout.more_item(8).contains(point.x, point.y) {
                self.select_tool(Tool::ThreePointRectangle);
                self.redraw();
                return;
            }
            if layout.more_item(9).contains(point.x, point.y) {
                self.select_tool(Tool::ThreePointCenterRectangle);
                self.redraw();
                return;
            }
            if layout.more_item(10).contains(point.x, point.y) {
                self.create_extrusion("join");
                self.redraw();
                return;
            }
            if layout.more_item(11).contains(point.x, point.y) {
                self.select_tool(Tool::FitSpline);
                self.redraw();
                return;
            }
            if layout.more_item(12).contains(point.x, point.y) {
                self.select_tool(Tool::ControlSpline);
                self.redraw();
                return;
            }
            for (index, tool) in [Tool::Point, Tool::Polygon, Tool::Slot]
                .into_iter()
                .enumerate()
            {
                if layout.more_item(index).contains(point.x, point.y) {
                    self.select_tool(tool);
                    self.redraw();
                    return;
                }
            }
            self.more_open = false;
        }
        if let Some(field) = self.editable_field_at(point) {
            if self.read_only() && !matches!(field, EditField::ExtrusionDepth) {
                self.status_message =
                    "READ ONLY: this sketch has unsupported native features".into();
            } else {
                self.editing = Some(DimensionEdit {
                    field,
                    buffer: String::new(),
                });
                self.status_message = match field {
                    EditField::ExtrusionDepth => {
                        "Type positive depth in mm, then Enter  |  Esc to cancel"
                    }
                    EditField::PolygonSides => "Type 3-64 sides, then Enter  |  Esc to cancel",
                    EditField::SlotWidth => "Type slot width in mm, then Enter  |  Esc to cancel",
                    EditField::MoveX | EditField::MoveY => {
                        "Type signed offset in mm, then Enter  |  Esc to cancel"
                    }
                    EditField::ScaleFactor => {
                        "Type positive scale factor, then Enter  |  Esc to cancel"
                    }
                    EditField::RotateAngle => {
                        "Type signed angle in degrees, then Enter  |  Esc to cancel"
                    }
                    EditField::RelationAngle => {
                        "Type signed line angle in degrees, then Enter  |  Esc to cancel"
                    }
                    EditField::RadialDifference => {
                        "Type signed radius difference in mm, then Enter  |  Esc to cancel"
                    }
                    EditField::CenterX | EditField::CenterY => {
                        "Type signed center coordinate in mm, then Enter  |  Esc to cancel"
                    }
                    EditField::SplineX | EditField::SplineY => {
                        "Type signed spline point coordinate in mm, then Enter  |  Esc to cancel"
                    }
                    EditField::PointX | EditField::PointY => {
                        "Type signed point coordinate in mm, then Enter  |  Esc to cancel"
                    }
                    _ => "Type a dimension in mm, then Enter  |  Esc to cancel",
                }
                .into();
            }
            self.redraw();
            return;
        }
        if self.document.selected.is_some() && layout.right.contains(point.x, point.y) {
            if (layout.right.y + 515.0..layout.right.y + 550.0).contains(&point.y) {
                self.transform_rotate = point.x >= layout.right.x + layout.right.w * 0.5;
                self.editing = None;
                self.redraw();
                return;
            }
            let action = if (layout.right.y + 430.0..layout.right.y + 460.0).contains(&point.y) {
                Some(false)
            } else if (layout.right.y + 465.0..layout.right.y + 495.0).contains(&point.y) {
                Some(true)
            } else {
                None
            };
            if let Some(copy) = action {
                if !self.read_only() {
                    let changed = if copy {
                        self.document.copy_selected(self.move_offset)
                    } else {
                        self.document.move_selected(self.move_offset)
                    };
                    if changed {
                        self.status_message = if copy {
                            "Copied geometry"
                        } else {
                            "Moved geometry"
                        }
                        .into();
                    }
                }
                self.redraw();
                return;
            }
            let scale_action =
                if (layout.right.y + 610.0..layout.right.y + 640.0).contains(&point.y) {
                    Some(false)
                } else if (layout.right.y + 640.0..layout.right.y + 670.0).contains(&point.y) {
                    Some(true)
                } else {
                    None
                };
            if let Some(copy) = scale_action {
                if !self.read_only() {
                    let changed = if self.transform_rotate {
                        self.document.rotate_selected(
                            self.rotate_angle,
                            self.transform_center,
                            copy,
                        )
                    } else {
                        self.document
                            .scale_selected(self.scale_factor, self.transform_center, copy)
                    };
                    if changed {
                        self.status_message = match (self.transform_rotate, copy) {
                            (true, true) => "Rotated copy",
                            (true, false) => "Rotated geometry",
                            (false, true) => "Scaled copy",
                            (false, false) => "Scaled geometry",
                        }
                        .into();
                    }
                }
                self.redraw();
                return;
            }
        }
        self.editing = None;
        for (index, plane) in [SketchPlane::Top, SketchPlane::Front, SketchPlane::Right]
            .into_iter()
            .enumerate()
        {
            if layout.tool_button(index + 5).contains(point.x, point.y) {
                self.create_sketch(plane);
                self.redraw();
                return;
            }
        }
        for (index, tool) in [
            (0, Tool::Select),
            (1, Tool::Rectangle),
            (2, Tool::Circle),
            (3, Tool::Line),
            (4, Tool::Ellipse),
            (8, Tool::Arc),
        ] {
            if layout.tool_button(index).contains(point.x, point.y) {
                self.select_tool(tool);
                self.redraw();
                return;
            }
        }
        let part_rows = self.part_rows();
        for (row_index, target) in part_rows.iter().enumerate() {
            if layout.part_row(row_index).contains(point.x, point.y) {
                self.switch_part_to(*target);
                self.redraw();
                return;
            }
        }
        if let Some(project) = &mut self.project {
            for index in 0..project.sketch_count() {
                if layout
                    .sketch_row(index, part_rows.len())
                    .contains(point.x, point.y)
                {
                    self.document.finish_drag();
                    match project.switch_sketch(index, &self.document) {
                        Ok(document) => {
                            self.document = document;
                            self.solid_preview = false;
                            self.selected_profile = 0;
                            self.editing = None;
                            self.status_message = format!(
                                "Editing {}",
                                project.sketch_name(index).unwrap_or("Sketch")
                            );
                        }
                        Err(error) => self.status_message = error,
                    }
                    self.redraw();
                    return;
                }
            }
        }
        let solid_count = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .map_or(0, |solids| solids.len());
        for index in 0..solid_count {
            if layout
                .solid_row(index, part_rows.len(), self.sketch_count())
                .contains(point.x, point.y)
            {
                self.selected_solid = index;
                self.solid_preview = true;
                self.editing = None;
                self.status_message =
                    format!("Selected extrusion {}  |  Delete to remove", index + 1);
                self.redraw();
                return;
            }
        }
        for (index, shape) in self.document.shapes.iter().enumerate() {
            if layout
                .row(index + solid_count, part_rows.len(), self.sketch_count())
                .contains(point.x, point.y)
            {
                self.document.selected = Some(shape.id());
                self.document.selected_spline_point = None;
                self.solid_preview = false;
                self.select_tool(Tool::Select);
                self.redraw();
                return;
            }
        }
        if !layout.viewport.contains(point.x, point.y) {
            return;
        }
        if self.solid_preview {
            // Left-click in the 3D view is for selection in the web app; it never orbits.
            return;
        }
        let raw_world = self.document.screen_to_world(point, self.viewport_origin());
        let world = if self.document.tool == Tool::Select {
            raw_world
        } else {
            self.document
                .snap_point(raw_world, self.snap_grid, self.snap_geometry)
                .map_or(raw_world, |snap| snap.point)
        };
        if self.read_only() && self.document.tool != Tool::Select {
            self.status_message = "READ ONLY: this sketch has unsupported native features".into();
            self.redraw();
            return;
        }
        match self.document.tool {
            Tool::Select => {
                self.split_circle_start = None;
                self.document.select_at(world);
                if !self.read_only() {
                    self.document.begin_drag(world);
                }
            }
            Tool::Point => {
                self.document.add_point(world);
                self.status_message.clear();
            }
            Tool::FitSpline | Tool::ControlSpline => {
                self.spline_points.push(world);
                self.status_message = format!(
                    "Spline: {} points  |  Enter: finish  |  Shift+Enter: close fit",
                    self.spline_points.len()
                );
            }
            Tool::Rectangle
            | Tool::CenterRectangle
            | Tool::ThreePointRectangle
            | Tool::ThreePointCenterRectangle
            | Tool::Circle
            | Tool::ThreePointCircle
            | Tool::Line
            | Tool::Ellipse
            | Tool::Arc
            | Tool::CenterArc
            | Tool::Polygon
            | Tool::Slot => {
                if self.document.draft_start.is_some() {
                    let finished = self.document.finish_draft(world);
                    if finished {
                        self.status_message.clear();
                    } else if self.document.draft_end.is_some() {
                        self.status_message = match self.document.tool {
                            Tool::Arc => "Arc: choose a point on the curve  |  Esc: cancel".into(),
                            Tool::CenterArc => {
                                "Center arc: choose end direction  |  W: clockwise".into()
                            }
                            Tool::ThreePointCircle => {
                                "Circle: choose third point  |  Esc: cancel".into()
                            }
                            Tool::Ellipse => "Ellipse: choose height  |  Esc: cancel".into(),
                            Tool::Slot => "Slot: choose side wall  |  Esc: cancel".into(),
                            Tool::ThreePointRectangle | Tool::ThreePointCenterRectangle => {
                                "Rectangle: choose perpendicular height  |  Esc: cancel".into()
                            }
                            _ => self.status_message.clone(),
                        };
                    }
                } else {
                    self.document.draft_start = Some(world);
                    if matches!(
                        self.document.tool,
                        Tool::Arc
                            | Tool::CenterArc
                            | Tool::Ellipse
                            | Tool::Slot
                            | Tool::ThreePointCircle
                            | Tool::ThreePointRectangle
                            | Tool::ThreePointCenterRectangle
                    ) {
                        self.status_message = if self.document.tool == Tool::Arc {
                            "Arc: choose end point"
                        } else if self.document.tool == Tool::CenterArc {
                            "Center arc: choose radial start"
                        } else if self.document.tool == Tool::ThreePointCircle {
                            "Circle: choose second point"
                        } else if self.document.tool == Tool::Slot {
                            "Slot: choose second end-cap center"
                        } else if self.document.tool == Tool::ThreePointRectangle {
                            "Rectangle: choose first edge endpoint"
                        } else if self.document.tool == Tool::ThreePointCenterRectangle {
                            "Rectangle: choose side midpoint"
                        } else {
                            "Ellipse: choose axis endpoint"
                        }
                        .into();
                    }
                }
            }
        }
        self.redraw();
    }

    fn paint(&self) -> Painter {
        let layout = self.layout();
        let width = layout.top.w;
        let height = layout.bottom.y + layout.bottom.h;
        let mut painter = Painter::new(width, height);
        painter.rect(
            Rect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: height,
            },
            BG,
        );
        if let Some(page) = &self.projects_page {
            self.paint_projects_page(&mut painter, layout, page);
            return painter;
        }
        if self.solid_preview {
            if let Some(solids) = self
                .project
                .as_ref()
                .and_then(ProjectFile::extrusion_previews)
            {
                if let Some((scene, projector)) = self.solid_projector(layout) {
                    if scene.bodies < solids.len() {
                        self.paint_boolean_preview(
                            &mut painter,
                            layout,
                            &scene,
                            &projector,
                            scene.bodies,
                        );
                    } else {
                        self.paint_solid_preview(&mut painter, layout, &solids, &projector);
                    }
                }
            }
        } else {
            self.paint_grid(&mut painter, layout);
            self.paint_shapes(&mut painter, layout);
            self.paint_profile_choice(&mut painter, layout);
        }
        self.paint_panels(&mut painter, layout);
        self.paint_more_menu(&mut painter, layout);
        painter
    }

    fn paint_more_menu(&self, painter: &mut Painter, layout: Layout) {
        if !self.more_open {
            return;
        }
        for (index, label) in [
            "POINT [P]",
            "POLYGON [G]",
            "SLOT [S]",
            "EXTRUDE [X]",
            "NEW PART [B]",
            "3PT CIRCLE",
            "CENTER ARC",
            "CENTER RECT",
            "3PT RECT",
            "3PT CTR RECT",
            "JOIN [SHIFT J]",
            "FIT SPLINE",
            "CTRL SPLINE",
        ]
        .into_iter()
        .enumerate()
        {
            let row = layout.more_item(index);
            painter.rect(row, PANEL_LIGHT);
            painter.outline(row, 1.0, ACCENT);
            painter.text(row.x + 9.0, row.y + 9.0, label, 1.0, TEXT);
        }
    }

    fn paint_profile_choice(&self, painter: &mut Painter, layout: Layout) {
        if self.read_only() {
            return;
        }
        let Some(profiles) = profiles_from_document(&self.document) else {
            return;
        };
        if profiles.len() < 2 {
            return;
        }
        let index = self.selected_profile.min(profiles.len() - 1);
        let profile = &profiles[index];
        let origin = self.viewport_origin();
        for loop_points in std::iter::once(&profile.outer).chain(profile.holes.iter()) {
            for edge in 0..loop_points.len() {
                let a = self.document.world_to_screen(loop_points[edge], origin);
                let b = self
                    .document
                    .world_to_screen(loop_points[(edge + 1) % loop_points.len()], origin);
                painter.line(a.x, a.y, b.x, b.y, 3.0, ACCENT);
            }
        }
        painter.text(
            layout.viewport.x + 18.0,
            layout.viewport.y + 18.0,
            &format!(
                "PROFILE {} / {}  [ ] SELECT  X EXTRUDE",
                index + 1,
                profiles.len()
            ),
            1.0,
            ACCENT,
        );
    }

    fn cycle_profile(&mut self, direction: isize) {
        let Some(profiles) = profiles_from_document(&self.document) else {
            self.status_message = "No closed profile in this sketch".into();
            return;
        };
        if profiles.len() < 2 {
            self.status_message = format!("{} closed profile", profiles.len());
            return;
        }
        self.selected_profile = (self.selected_profile as isize + direction)
            .rem_euclid(profiles.len() as isize) as usize;
        self.status_message = format!(
            "Profile {} of {} selected  |  X to extrude",
            self.selected_profile + 1,
            profiles.len()
        );
    }

    fn paint_solid_labels(&self, painter: &mut Painter, layout: Layout, count: usize) {
        let viewport = layout.viewport;
        painter.text(
            viewport.x + 18.0,
            viewport.y + 18.0,
            &format!("3D / {count} SOLID{}", if count == 1 { "" } else { "S" }),
            1.3,
            TEXT,
        );
        painter.text(
            viewport.x + 18.0,
            viewport.y + 43.0,
            "RIGHT DRAG ORBIT  /  MIDDLE PAN  /  WHEEL ZOOM  /  TAB SKETCH",
            1.0,
            MUTED,
        );
    }

    fn paint_boolean_preview(
        &self,
        painter: &mut Painter,
        layout: Layout,
        scene: &SolidScene,
        projector: &SolidProjector,
        count: usize,
    ) {
        painter.rect(layout.viewport, [0.065, 0.085, 0.115, 1.0]);
        let mut faces = Vec::new();
        for &[a, b, c] in &scene.triangles {
            let Some(normal) = triangle_normal(a, b, c) else {
                continue;
            };
            let facing = projector.facing_at(normal, a);
            if facing <= 0.001 {
                continue;
            }
            let shade = (0.35 + facing * 0.16).clamp(0.22, 0.55);
            let color = [shade * 0.48, shade * 0.82, shade, 1.0];
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
        self.paint_solid_labels(painter, layout, count);
    }

    fn paint_solid_preview(
        &self,
        painter: &mut Painter,
        layout: Layout,
        solids: &[SolidPreview],
        projector: &SolidProjector,
    ) {
        painter.rect(layout.viewport, [0.065, 0.085, 0.115, 1.0]);
        let screen = |solid: &SolidPreview, point: Point, offset: f32| {
            projector.screen(solid.world_point(point, offset))
        };
        let mut faces: Vec<(f32, Vec<[f32; 2]>, Color, bool)> = Vec::new();
        for solid in solids {
            for (loop_index, loop_points) in solid.loops().enumerate() {
                for index in 0..loop_points.len() {
                    let a = loop_points[index];
                    let b = loop_points[(index + 1) % loop_points.len()];
                    let a0 = solid.world_point(a, 0.0);
                    let b0 = solid.world_point(b, 0.0);
                    let b1 = solid.world_point(b, solid.depth);
                    let Some(normal) = triangle_normal(a0, b0, b1) else {
                        continue;
                    };
                    let facing = projector.facing_at(normal, a0);
                    if facing <= 0.001 {
                        continue;
                    }
                    let shade = (0.35 + facing * 0.16).clamp(0.22, 0.55);
                    let color = [shade * 0.48, shade * 0.82, shade, 1.0];
                    let mid = std::array::from_fn(|axis| (a0[axis] + b1[axis]) * 0.5);
                    faces.push((
                        projector.depth(mid),
                        vec![
                            screen(solid, a, 0.0),
                            screen(solid, b, 0.0),
                            screen(solid, b, solid.depth),
                            screen(solid, a, solid.depth),
                        ],
                        color,
                        loop_index == 0,
                    ));
                }
            }
            let cap_offset = if projector.facing_at(
                solid.normal(),
                solid.world_point(Point { x: 0.0, y: 0.0 }, solid.depth),
            ) >= 0.0
            {
                solid.depth
            } else {
                0.0
            };
            if let Some((cap_points, cap_triangles)) = solid.cap_mesh() {
                for [i, j, k] in cap_triangles {
                    let vertices = [cap_points[i], cap_points[j], cap_points[k]];
                    let mut mid = [0.0; 3];
                    for point in vertices {
                        let world = solid.world_point(point, cap_offset);
                        for axis in 0..3 {
                            mid[axis] += world[axis] / 3.0;
                        }
                    }
                    faces.push((
                        projector.depth(mid),
                        vertices
                            .map(|point| screen(solid, point, cap_offset))
                            .to_vec(),
                        [0.28, 0.60, 0.72, 1.0],
                        false,
                    ));
                }
            }
        }
        faces.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, points, color, outline) in faces {
            for index in 1..points.len() - 1 {
                painter.triangle(points[0], points[index], points[index + 1], color);
            }
            if outline {
                for index in 0..points.len() {
                    let a = points[index];
                    let b = points[(index + 1) % points.len()];
                    painter.line(a[0], a[1], b[0], b[1], 1.2, ACCENT);
                }
            }
        }
        for solid in solids {
            let cap_offset = if projector.facing_at(
                solid.normal(),
                solid.world_point(Point { x: 0.0, y: 0.0 }, solid.depth),
            ) >= 0.0
            {
                solid.depth
            } else {
                0.0
            };
            for loop_points in solid.loops() {
                for index in 0..loop_points.len() {
                    let a = screen(solid, loop_points[index], cap_offset);
                    let b = screen(
                        solid,
                        loop_points[(index + 1) % loop_points.len()],
                        cap_offset,
                    );
                    painter.line(a[0], a[1], b[0], b[1], 1.2, ACCENT);
                }
            }
        }
        self.paint_solid_labels(painter, layout, solids.len());
    }

    fn paint_grid(&self, painter: &mut Painter, layout: Layout) {
        let viewport = layout.viewport;
        let origin = self.viewport_origin();
        let spacing = 25.0 * self.document.zoom;
        if spacing > 8.0 {
            let first_x =
                viewport.x + (origin.x + self.document.pan.x - viewport.x).rem_euclid(spacing);
            let first_y =
                viewport.y + (origin.y + self.document.pan.y - viewport.y).rem_euclid(spacing);
            let grid: Color = [0.13, 0.16, 0.20, 1.0];
            let mut x = first_x;
            while x < viewport.x + viewport.w {
                painter.line(x, viewport.y, x, viewport.y + viewport.h, 1.0, grid);
                x += spacing;
            }
            let mut y = first_y;
            while y < viewport.y + viewport.h {
                painter.line(viewport.x, y, viewport.x + viewport.w, y, 1.0, grid);
                y += spacing;
            }
        }
        let zero = self
            .document
            .world_to_screen(Point { x: 0.0, y: 0.0 }, origin);
        if zero.x >= viewport.x && zero.x <= viewport.x + viewport.w {
            painter.line(
                zero.x,
                viewport.y,
                zero.x,
                viewport.y + viewport.h,
                1.5,
                BORDER,
            );
        }
        if zero.y >= viewport.y && zero.y <= viewport.y + viewport.h {
            painter.line(
                viewport.x,
                zero.y,
                viewport.x + viewport.w,
                zero.y,
                1.5,
                BORDER,
            );
        }
    }

    fn paint_shapes(&self, painter: &mut Painter, layout: Layout) {
        let origin = self.viewport_origin();
        for shape in &self.document.shapes {
            self.paint_shape(
                painter,
                shape,
                origin,
                self.document.selected == Some(shape.id()),
            );
        }
        if let Some(start) = self.document.draft_start {
            if layout.viewport.contains(self.cursor.x, self.cursor.y) {
                let raw_end = self.document.screen_to_world(self.cursor, origin);
                let end = self
                    .document
                    .snap_point(raw_end, self.snap_grid, self.snap_geometry)
                    .map_or(raw_end, |snap| snap.point);
                let draft = match self.document.tool {
                    Tool::Line => Some(Shape::Line {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                    Tool::Rectangle => Some(Shape::Rectangle {
                        id: 0,
                        a: start,
                        b: end,
                    }),
                    Tool::CenterRectangle => Some(Shape::Rectangle {
                        id: 0,
                        a: Point {
                            x: start.x * 2.0 - end.x,
                            y: start.y * 2.0 - end.y,
                        },
                        b: end,
                    }),
                    Tool::ThreePointRectangle => match self.document.draft_end {
                        Some(second) => Shape::rectangle_from_three_points(0, start, second, end),
                        None => Some(Shape::Line {
                            id: 0,
                            a: start,
                            b: end,
                        }),
                    },
                    Tool::ThreePointCenterRectangle => match self.document.draft_end {
                        Some(midpoint) => {
                            Shape::rectangle_from_center_side(0, start, midpoint, end)
                        }
                        None => Some(Shape::Line {
                            id: 0,
                            a: start,
                            b: end,
                        }),
                    },
                    Tool::Circle => Some(Shape::Circle {
                        id: 0,
                        center: start,
                        radius: start.distance(end),
                        segments: 64,
                    }),
                    Tool::ThreePointCircle => match self.document.draft_end {
                        Some(second) => Shape::circle_from_three_points(0, start, second, end),
                        None => Some(Shape::Line {
                            id: 0,
                            a: start,
                            b: end,
                        }),
                    },
                    Tool::Ellipse => match self.document.draft_end {
                        Some(first_axis) => {
                            Shape::ellipse_from_three_points(0, start, first_axis, end)
                        }
                        None => Some(Shape::Line {
                            id: 0,
                            a: start,
                            b: end,
                        }),
                    },
                    Tool::Arc => match self.document.draft_end {
                        Some(arc_end) => Shape::arc_from_three_points(0, start, arc_end, end),
                        None => Some(Shape::Line {
                            id: 0,
                            a: start,
                            b: end,
                        }),
                    },
                    Tool::CenterArc => match self.document.draft_end {
                        Some(radial_start) => Shape::arc_from_center_points(
                            0,
                            start,
                            radial_start,
                            end,
                            self.document.arc_clockwise,
                        ),
                        None => Some(Shape::Line {
                            id: 0,
                            a: start,
                            b: end,
                        }),
                    },
                    Tool::Polygon => Some(Shape::Polygon {
                        id: 0,
                        center: start,
                        radius: start.distance(end),
                        sides: 6,
                        rotation: (end.y - start.y).atan2(end.x - start.x).to_degrees(),
                    }),
                    Tool::Slot => match self.document.draft_end {
                        Some(to) => Shape::capsule_from_three_points(0, start, to, end),
                        None => Some(Shape::Line {
                            id: 0,
                            a: start,
                            b: end,
                        }),
                    },
                    Tool::Point => None,
                    Tool::FitSpline | Tool::ControlSpline => None,
                    Tool::Select => None,
                };
                if let Some(shape) = draft {
                    self.paint_shape(painter, &shape, origin, true);
                }
            }
        }
        if matches!(self.document.tool, Tool::FitSpline | Tool::ControlSpline)
            && !self.spline_points.is_empty()
        {
            let mode = if self.document.tool == Tool::FitSpline {
                spline::SplineMode::Fit
            } else {
                spline::SplineMode::Control
            };
            if spline::beziers(mode, &self.spline_points, false).is_ok() {
                self.paint_shape(
                    painter,
                    &Shape::Spline {
                        id: 0,
                        mode,
                        points: self.spline_points.clone(),
                        closed: false,
                    },
                    origin,
                    true,
                );
            } else {
                for pair in self.spline_points.windows(2) {
                    let a = self.document.world_to_screen(pair[0], origin);
                    let b = self.document.world_to_screen(pair[1], origin);
                    painter.line(a.x, a.y, b.x, b.y, 1.5, ACCENT);
                }
            }
            if layout.viewport.contains(self.cursor.x, self.cursor.y) {
                let last = self
                    .document
                    .world_to_screen(*self.spline_points.last().unwrap(), origin);
                painter.line(last.x, last.y, self.cursor.x, self.cursor.y, 1.0, ACCENT);
            }
        }
        if self.document.tool != Tool::Select
            && layout.viewport.contains(self.cursor.x, self.cursor.y)
        {
            let raw = self.document.screen_to_world(self.cursor, origin);
            if let Some(snap) = self
                .document
                .snap_point(raw, self.snap_grid, self.snap_geometry)
            {
                let p = self.document.world_to_screen(snap.point, origin);
                let size = if snap.kind == SnapKind::Grid {
                    3.0
                } else {
                    5.0
                };
                painter.outline(
                    Rect {
                        x: p.x - size,
                        y: p.y - size,
                        w: size * 2.0,
                        h: size * 2.0,
                    },
                    1.2,
                    ACCENT,
                );
            }
        }
    }

    fn paint_shape(&self, painter: &mut Painter, shape: &Shape, origin: Point, selected: bool) {
        let color = if selected {
            ACCENT
        } else if self.document.construction.contains(&shape.id()) {
            MUTED
        } else {
            SHAPE
        };
        match shape {
            Shape::Line { a, b, .. } => {
                let a = self.document.world_to_screen(*a, origin);
                let b = self.document.world_to_screen(*b, origin);
                painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                if selected {
                    for (index, endpoint) in [a, b].iter().enumerate() {
                        let fixed = self
                            .document
                            .fixed_anchors
                            .contains_key(&(shape.id(), index));
                        painter.circle(
                            endpoint.x,
                            endpoint.y,
                            if fixed { 5.0 } else { 3.5 },
                            1.5,
                            if fixed { ACCENT } else { MUTED },
                        );
                    }
                }
            }
            Shape::Rectangle { a, b, .. } => {
                let a = self.document.world_to_screen(*a, origin);
                let b = self.document.world_to_screen(*b, origin);
                painter.outline(
                    Rect {
                        x: a.x.min(b.x),
                        y: a.y.min(b.y),
                        w: (a.x - b.x).abs(),
                        h: (a.y - b.y).abs(),
                    },
                    if selected { 2.5 } else { 1.8 },
                    color,
                );
            }
            Shape::RotatedRectangle {
                center,
                width,
                height,
                rotation,
                ..
            } => {
                let corners = Shape::rotated_rectangle_corners(*center, *width, *height, *rotation);
                for index in 0..4 {
                    let a = self.document.world_to_screen(corners[index], origin);
                    let b = self
                        .document
                        .world_to_screen(corners[(index + 1) % 4], origin);
                    painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                }
            }
            Shape::Circle { center, radius, .. } => {
                let center = self.document.world_to_screen(*center, origin);
                painter.circle(
                    center.x,
                    center.y,
                    radius * self.document.zoom,
                    if selected { 2.5 } else { 1.8 },
                    color,
                );
            }
            Shape::Ellipse {
                center,
                rx,
                ry,
                rotation,
                segments,
                ..
            } => {
                let mut previous = self.document.world_to_screen(
                    Shape::ellipse_point(*center, *rx, *ry, *rotation, 0.0),
                    origin,
                );
                for step in 1..=*segments {
                    let next = self.document.world_to_screen(
                        Shape::ellipse_point(
                            *center,
                            *rx,
                            *ry,
                            *rotation,
                            step as f32 / *segments as f32,
                        ),
                        origin,
                    );
                    painter.line(
                        previous.x,
                        previous.y,
                        next.x,
                        next.y,
                        if selected { 2.5 } else { 1.8 },
                        color,
                    );
                    previous = next;
                }
            }
            Shape::EllipticArc {
                center,
                rx,
                ry,
                rotation,
                start_angle,
                sweep,
                segments,
                ..
            } => {
                let count = ((sweep.abs() / std::f64::consts::TAU) * *segments as f64)
                    .ceil()
                    .max(2.0) as usize;
                let mut previous = self.document.world_to_screen(
                    Shape::elliptic_arc_point(
                        *center,
                        *rx,
                        *ry,
                        *rotation,
                        *start_angle,
                        *sweep,
                        0.0,
                    ),
                    origin,
                );
                for index in 1..=count {
                    let next = self.document.world_to_screen(
                        Shape::elliptic_arc_point(
                            *center,
                            *rx,
                            *ry,
                            *rotation,
                            *start_angle,
                            *sweep,
                            index as f64 / count as f64,
                        ),
                        origin,
                    );
                    painter.line(
                        previous.x,
                        previous.y,
                        next.x,
                        next.y,
                        if selected { 2.5 } else { 1.8 },
                        color,
                    );
                    previous = next;
                }
            }
            Shape::Spline {
                mode,
                points,
                closed,
                ..
            } => {
                if let Ok(sampled) = spline::beziers(*mode, points, *closed)
                    .and_then(|curves| spline::samples(&curves, 0.01))
                {
                    for pair in sampled.windows(2) {
                        let a = self.document.world_to_screen(pair[0], origin);
                        let b = self.document.world_to_screen(pair[1], origin);
                        painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                    }
                }
                if selected {
                    if *mode == spline::SplineMode::Control {
                        for pair in points.windows(2) {
                            let a = self.document.world_to_screen(pair[0], origin);
                            let b = self.document.world_to_screen(pair[1], origin);
                            let steps = (a.distance(b) / 12.0).ceil().max(1.0) as usize;
                            for step in (0..steps).step_by(2) {
                                let along = |fraction: f32| Point {
                                    x: a.x + (b.x - a.x) * fraction,
                                    y: a.y + (b.y - a.y) * fraction,
                                };
                                let from = along(step as f32 / steps as f32);
                                let to = along(((step + 1).min(steps)) as f32 / steps as f32);
                                painter.line(from.x, from.y, to.x, to.y, 1.0, MUTED);
                            }
                        }
                    }
                    for (index, point) in points.iter().enumerate() {
                        let at = self.document.world_to_screen(*point, origin);
                        painter.circle(
                            at.x,
                            at.y,
                            if self.document.selected_spline_point == Some(index) {
                                5.0
                            } else {
                                3.5
                            },
                            1.5,
                            color,
                        );
                    }
                }
            }
            Shape::Arc {
                center,
                radius,
                start_angle,
                sweep,
                segments,
                ..
            } => {
                let count = ((sweep.abs() / std::f64::consts::TAU) * *segments as f64)
                    .ceil()
                    .max(2.0) as usize;
                let mut previous = self.document.world_to_screen(
                    Shape::arc_point(*center, *radius, *start_angle, *sweep, 0.0),
                    origin,
                );
                for index in 1..=count {
                    let next = self.document.world_to_screen(
                        Shape::arc_point(
                            *center,
                            *radius,
                            *start_angle,
                            *sweep,
                            index as f64 / count as f64,
                        ),
                        origin,
                    );
                    painter.line(
                        previous.x,
                        previous.y,
                        next.x,
                        next.y,
                        if selected { 2.5 } else { 1.8 },
                        color,
                    );
                    previous = next;
                }
            }
            Shape::Point { at, .. } => {
                let center = self.document.world_to_screen(*at, origin);
                painter.line(
                    center.x - 5.0,
                    center.y,
                    center.x + 5.0,
                    center.y,
                    2.0,
                    color,
                );
                painter.line(
                    center.x,
                    center.y - 5.0,
                    center.x,
                    center.y + 5.0,
                    2.0,
                    color,
                );
            }
            Shape::Polygon {
                center,
                radius,
                sides,
                rotation,
                ..
            } => {
                for index in 0..*sides {
                    let a = self.document.world_to_screen(
                        Shape::polygon_vertex(*center, *radius, *sides, *rotation, index),
                        origin,
                    );
                    let b = self.document.world_to_screen(
                        Shape::polygon_vertex(
                            *center,
                            *radius,
                            *sides,
                            *rotation,
                            (index + 1) % sides,
                        ),
                        origin,
                    );
                    painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                }
            }
            Shape::Capsule {
                from,
                to,
                width,
                arc_segments,
                ..
            } => {
                let outline = Shape::capsule_outline(*from, *to, *width, *arc_segments as usize);
                for index in 0..outline.len() {
                    let a = self.document.world_to_screen(outline[index], origin);
                    let b = self
                        .document
                        .world_to_screen(outline[(index + 1) % outline.len()], origin);
                    painter.line(a.x, a.y, b.x, b.y, if selected { 2.5 } else { 1.8 }, color);
                }
            }
        }
        if selected
            && self.document.fixed_anchors.contains_key(&(shape.id(), 0))
            && matches!(
                shape,
                Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
                    | Shape::Polygon { .. }
                    | Shape::RotatedRectangle { .. }
            )
        {
            if let Some(center) = self.document.fixed_anchor_position(shape.id(), 0) {
                let at = self.document.world_to_screen(center, origin);
                painter.circle(at.x, at.y, 5.0, 1.5, ACCENT);
            }
        }
        if selected && matches!(shape, Shape::Arc { .. } | Shape::EllipticArc { .. }) {
            for index in [1, 2] {
                if self
                    .document
                    .fixed_anchors
                    .contains_key(&(shape.id(), index))
                {
                    if let Some(point) = self.document.fixed_anchor_position(shape.id(), index) {
                        let at = self.document.world_to_screen(point, origin);
                        painter.circle(at.x, at.y, 5.0, 1.5, ACCENT);
                    }
                }
            }
        }
        if selected {
            let range = match shape {
                Shape::Ellipse { .. } | Shape::RotatedRectangle { .. } => Some(1..5),
                Shape::Polygon { sides, .. } => Some(1..*sides as usize + 1),
                Shape::Capsule { .. } => Some(0..2),
                _ => None,
            };
            if let Some(range) = range {
                for index in range {
                    if self
                        .document
                        .fixed_anchors
                        .contains_key(&(shape.id(), index))
                    {
                        if let Some(point) = self.document.fixed_anchor_position(shape.id(), index)
                        {
                            let at = self.document.world_to_screen(point, origin);
                            painter.circle(at.x, at.y, 5.0, 1.5, ACCENT);
                        }
                    }
                }
            }
        }
        if selected && matches!(shape, Shape::Rectangle { .. }) {
            for index in [0, 1] {
                if self
                    .document
                    .fixed_anchors
                    .contains_key(&(shape.id(), index))
                {
                    if let Some(point) = self.document.fixed_anchor_position(shape.id(), index) {
                        let at = self.document.world_to_screen(point, origin);
                        painter.circle(at.x, at.y, 5.0, 1.5, ACCENT);
                    }
                }
            }
        }
    }

    fn paint_breadcrumbs(&self, painter: &mut Painter, layout: Layout) {
        let crumbs = self.breadcrumbs(layout);
        let last = crumbs.len().saturating_sub(1);
        for (index, (_, label, rect)) in crumbs.iter().enumerate() {
            let hovered = index < last && rect.contains(self.cursor.x, self.cursor.y);
            let color = if hovered {
                ACCENT
            } else if index == last {
                TEXT
            } else {
                MUTED
            };
            painter.text(rect.x, CRUMB_Y, label, CRUMB_SCALE, color);
            if hovered {
                painter.line(
                    rect.x,
                    CRUMB_Y + 8.0 * CRUMB_SCALE + 2.0,
                    rect.x + rect.w,
                    CRUMB_Y + 8.0 * CRUMB_SCALE + 2.0,
                    1.0,
                    ACCENT,
                );
            }
            if index < last {
                painter.text(
                    rect.x + rect.w,
                    CRUMB_Y,
                    CRUMB_SEPARATOR,
                    CRUMB_SCALE,
                    MUTED,
                );
            }
        }
    }

    fn paint_projects_page(&self, painter: &mut Painter, layout: Layout, page: &ProjectsPage) {
        painter.rect(layout.top, PANEL);
        painter.rect(layout.bottom, PANEL);
        painter.line(0.0, 94.0, layout.top.w, 94.0, 1.0, BORDER);
        painter.line(
            0.0,
            layout.bottom.y,
            layout.top.w,
            layout.bottom.y,
            1.0,
            BORDER,
        );
        painter.text(20.0, 13.0, "PUPPYCAD", 2.0, TEXT);
        self.paint_breadcrumbs(painter, layout);

        let (column_x, column_w) = project_page_column(layout);
        painter.text(column_x, 114.0, "PROJECTS", 2.0, TEXT);
        let project_count = page
            .entries
            .iter()
            .filter(|entry| entry.kind == EntryKind::Project)
            .count();
        let count = format!(
            "{project_count} PROJECT{}",
            if project_count == 1 { "" } else { "S" }
        );
        painter.text(
            column_x + column_w - count.chars().count() as f32 * 8.8,
            118.0,
            &count,
            1.1,
            MUTED,
        );
        painter.text(
            column_x,
            144.0,
            &clip_text(
                &page.dir.display().to_string(),
                (column_w / 8.8) as usize,
                true,
            ),
            1.1,
            MUTED,
        );

        let visible = project_page_visible_rows(layout);
        let shown = page.entries.len().saturating_sub(page.scroll).min(visible);
        for (index, entry) in page
            .entries
            .iter()
            .enumerate()
            .skip(page.scroll)
            .take(visible)
        {
            let rect = project_page_row(layout, index - page.scroll);
            let open = entry.kind == EntryKind::Project && self.is_open_project(&entry.path);
            let pending = page.pending_discard.as_deref() == Some(entry.path.as_path());
            let hovered = rect.contains(self.cursor.x, self.cursor.y);
            let selected = index == page.selected;
            painter.rect(
                rect,
                if selected {
                    [0.13, 0.29, 0.37, 1.0]
                } else if hovered {
                    PANEL_LIGHT
                } else {
                    PANEL
                },
            );
            painter.outline(
                rect,
                1.0,
                if pending {
                    SHAPE
                } else if selected || hovered {
                    ACCENT
                } else {
                    BORDER
                },
            );
            let icon_x = rect.x + 18.0;
            let icon_y = rect.y + 17.0;
            if entry.kind == EntryKind::Project {
                painter.outline(
                    Rect {
                        x: icon_x,
                        y: icon_y,
                        w: 18.0,
                        h: 18.0,
                    },
                    1.5,
                    ACCENT,
                );
                painter.rect(
                    Rect {
                        x: icon_x + 5.0,
                        y: icon_y + 5.0,
                        w: 8.0,
                        h: 8.0,
                    },
                    ACCENT,
                );
            } else {
                painter.rect(
                    Rect {
                        x: icon_x,
                        y: icon_y,
                        w: 9.0,
                        h: 4.0,
                    },
                    MUTED,
                );
                painter.outline(
                    Rect {
                        x: icon_x,
                        y: icon_y + 3.0,
                        w: 20.0,
                        h: 15.0,
                    },
                    1.5,
                    MUTED,
                );
            }
            let tag = if pending {
                "OPEN AGAIN TO DISCARD CHANGES"
            } else if open {
                "OPEN"
            } else {
                ""
            };
            let tag_w = tag.chars().count() as f32 * 9.6;
            let text_x = rect.x + 54.0;
            let name_chars = ((rect.x + rect.w - text_x - tag_w - 28.0) / 12.0).max(4.0) as usize;
            let (name, detail) = match entry.kind {
                EntryKind::Parent => (
                    "..".to_string(),
                    format!(
                        "UP TO {}",
                        entry
                            .path
                            .file_name()
                            .map_or_else(|| "/".into(), |name| name.to_string_lossy())
                    ),
                ),
                EntryKind::Folder => (entry.name.clone(), "FOLDER".to_string()),
                EntryKind::Project => (
                    entry.name.clone(),
                    match &entry.error {
                        Some(error) => format!("CANNOT READ: {error}"),
                        None => format!(
                            "{} PART{}  /  REVISION {}",
                            entry.parts,
                            if entry.parts == 1 { "" } else { "S" },
                            entry.revision
                        ),
                    },
                ),
            };
            painter.text(
                text_x,
                rect.y + 11.0,
                &clip_text(&name, name_chars, false),
                1.5,
                TEXT,
            );
            painter.text(
                text_x,
                rect.y + 33.0,
                &clip_text(&detail, name_chars * 3 / 2, false),
                1.0,
                if entry.error.is_some() { SHAPE } else { MUTED },
            );
            painter.text(
                rect.x + rect.w - 16.0 - tag_w,
                rect.y + 20.0,
                tag,
                1.2,
                if pending { SHAPE } else { ACCENT },
            );
        }
        if project_count == 0 {
            painter.text(
                column_x,
                project_page_row(layout, shown).y + 12.0,
                "No .pcad projects in this folder. Open a folder above to look elsewhere.",
                1.1,
                MUTED,
            );
        }
        if page.entries.len() > visible {
            let range = format!(
                "{}-{} OF {}",
                page.scroll + 1,
                (page.scroll + visible).min(page.entries.len()),
                page.entries.len()
            );
            painter.text(
                column_x + column_w - range.chars().count() as f32 * 8.8,
                144.0,
                &range,
                1.1,
                MUTED,
            );
        }
        painter.text(
            15.0,
            layout.bottom.y + 10.0,
            &self.status_message,
            1.2,
            MUTED,
        );
    }

    fn paint_panels(&self, painter: &mut Painter, layout: Layout) {
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
        if self.solid_preview {
            for (perspective, label) in [(true, "PERSPECTIVE"), (false, "ORTHO")] {
                let button = layout.projection_button(perspective);
                let active = self.solid_view.perspective == perspective;
                painter.rect(
                    button,
                    if active {
                        [0.13, 0.31, 0.40, 1.0]
                    } else {
                        PANEL_LIGHT
                    },
                );
                painter.outline(button, 1.0, if active { ACCENT } else { BORDER });
                painter.text(
                    button.x + 10.0,
                    button.y + 8.0,
                    label,
                    1.4,
                    if active { ACCENT } else { TEXT },
                );
            }
        }
        for (index, tool, label) in [
            (0, Tool::Select, "SELECT  [V]"),
            (1, Tool::Rectangle, "RECT  [R]"),
            (2, Tool::Circle, "CIRCLE [C]"),
            (3, Tool::Line, "LINE   [L]"),
            (4, Tool::Ellipse, "ELLIPSE [E]"),
            (8, Tool::Arc, "ARC [A]"),
        ] {
            let button = layout.tool_button(index);
            painter.rect(
                button,
                if self.document.tool == tool {
                    [0.13, 0.31, 0.40, 1.0]
                } else {
                    PANEL_LIGHT
                },
            );
            painter.outline(
                button,
                1.0,
                if self.document.tool == tool {
                    ACCENT
                } else {
                    BORDER
                },
            );
            painter.text(
                button.x + 10.0,
                button.y + 10.0,
                label,
                if tool == Tool::Ellipse { 1.0 } else { 1.25 },
                TEXT,
            );
        }
        for (index, label) in ["+ TOP [1]", "+ FRONT [2]", "+ RIGHT [3]"]
            .into_iter()
            .enumerate()
        {
            let button = layout.tool_button(index + 5);
            painter.rect(button, PANEL_LIGHT);
            painter.outline(button, 1.0, BORDER);
            painter.text(button.x + 8.0, button.y + 10.0, label, 1.0, TEXT);
        }
        let more = layout.tool_button(9);
        painter.rect(
            more,
            if self.more_open || matches!(self.document.tool, Tool::Point | Tool::Polygon) {
                [0.13, 0.31, 0.40, 1.0]
            } else {
                PANEL_LIGHT
            },
        );
        painter.outline(more, 1.0, if self.more_open { ACCENT } else { BORDER });
        painter.text(more.x + 8.0, more.y + 10.0, "MORE [M]", 1.0, TEXT);
        painter.text(17.0, layout.left.y + 19.0, "PROJECT", 1.5, TEXT);
        let part_rows = self.part_rows();
        if !part_rows.is_empty() {
            painter.text(
                layout.left.x + 140.0,
                layout.left.y + 21.0,
                "CTRL+PGUP/DN",
                0.8,
                MUTED,
            );
        }
        if let Some(project) = &self.project {
            for (index, part_index) in part_rows.iter().enumerate() {
                let row = layout.part_row(index);
                if row.y + row.h >= layout.bottom.y {
                    break;
                }
                let active = *part_index == project.active_part_index();
                if active {
                    painter.rect(row, [0.13, 0.29, 0.37, 1.0]);
                }
                let name = project.part_name_at(*part_index).unwrap_or("Part");
                let display_name = if name.chars().count() > 13 {
                    format!("{}...", name.chars().take(13).collect::<String>())
                } else {
                    name.to_string()
                };
                painter.text(
                    row.x + 9.0,
                    row.y + 9.0,
                    &format!("{} PART / {}", if active { ">" } else { "v" }, display_name),
                    1.0,
                    if active { TEXT } else { MUTED },
                );
            }
        }
        for index in 0..self.sketch_count() {
            let row = layout.sketch_row(index, part_rows.len());
            if row.y + row.h >= layout.bottom.y {
                break;
            }
            let active = self
                .project
                .as_ref()
                .is_none_or(|project| project.active_sketch() == index);
            if active {
                painter.rect(row, [0.13, 0.29, 0.37, 1.0]);
            }
            let name = self
                .project
                .as_ref()
                .and_then(|project| project.sketch_name(index))
                .unwrap_or("Sketch 1");
            let plane = self
                .project
                .as_ref()
                .and_then(|project| project.sketch_plane(index))
                .map(SketchPlane::name)
                .unwrap_or(if self.project.is_some() {
                    "OTHER"
                } else {
                    "TOP"
                });
            painter.text(
                row.x + 9.0,
                row.y + 9.0,
                &format!("{} {name} / {plane}", if active { ">" } else { "v" }),
                1.1,
                if active { TEXT } else { MUTED },
            );
        }
        let solids = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .unwrap_or_default();
        for (index, solid) in solids.iter().enumerate() {
            let row = layout.solid_row(index, part_rows.len(), self.sketch_count());
            if row.y + row.h >= layout.bottom.y {
                break;
            }
            let active = self.solid_preview && self.selected_solid == index;
            if active {
                painter.rect(row, [0.13, 0.29, 0.37, 1.0]);
            }
            painter.text(
                row.x + 9.0,
                row.y + 9.0,
                &format!(
                    "{} {} / {}",
                    if active { ">" } else { "v" },
                    self.project
                        .as_ref()
                        .and_then(|project| project.extrusion_operation_at(index))
                        .unwrap_or("EXTRUDE")
                        .to_uppercase(),
                    solid.name
                ),
                1.0,
                if active { TEXT } else { MUTED },
            );
        }
        for (index, shape) in self.document.shapes.iter().enumerate() {
            if layout
                .row(index + solids.len(), part_rows.len(), self.sketch_count())
                .y
                + 31.0
                >= layout.bottom.y
            {
                break;
            }
            let row = layout.row(index + solids.len(), part_rows.len(), self.sketch_count());
            if self.document.selected == Some(shape.id()) {
                painter.rect(row, [0.13, 0.29, 0.37, 1.0]);
            }
            painter.text(
                row.x + 10.0,
                row.y + 9.0,
                &format!(
                    "{} {}{}{}{}{}{}",
                    shape.name(),
                    shape.id(),
                    if self.document.construction.contains(&shape.id()) {
                        " [C]"
                    } else {
                        ""
                    },
                    self.document
                        .axis_constraints
                        .get(&shape.id())
                        .map_or("", |axis| match axis {
                            AxisConstraint::Horizontal => " [H]",
                            AxisConstraint::Vertical => " [V]",
                        }),
                    if self.document.equal_lines.contains_key(&shape.id())
                        || self
                            .document
                            .equal_lines
                            .values()
                            .any(|source| *source == shape.id())
                        || self.document.equal_radii.contains_key(&shape.id())
                        || self
                            .document
                            .equal_radii
                            .values()
                            .any(|source| *source == shape.id())
                        || self.document.equal_ellipses.contains_key(&shape.id())
                        || self
                            .document
                            .equal_ellipses
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        " [=]"
                    } else {
                        ""
                    },
                    if self.document.shared_ellipses.contains_key(&shape.id())
                        || self
                            .document
                            .shared_ellipses
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        " [SE]"
                    } else if self.document.midpoint_links.contains_key(&shape.id())
                        || self
                            .document
                            .midpoint_links
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        " [MID]"
                    } else if self.document.point_curve_links.contains_key(&shape.id())
                        || self
                            .document
                            .point_curve_links
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        " [POC]"
                    } else if self.document.normal_links.contains_key(&shape.id())
                        || self
                            .document
                            .normal_links
                            .values()
                            .any(|(source, _)| *source == shape.id())
                    {
                        " [NORM]"
                    } else if self.document.internal_tangents.contains_key(&shape.id())
                        || self
                            .document
                            .internal_tangents
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        " [IT]"
                    } else if self.document.external_tangents.contains_key(&shape.id())
                        || self
                            .document
                            .external_tangents
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        " [T]"
                    } else if self.document.radius_differences.contains_key(&shape.id())
                        || self
                            .document
                            .radius_differences
                            .values()
                            .any(|(source, _)| *source == shape.id())
                    {
                        " [RD]"
                    } else if self.document.concentric.contains_key(&shape.id())
                        || self
                            .document
                            .concentric
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        " [O]"
                    } else {
                        ""
                    },
                    self.document
                        .line_relations
                        .get(&shape.id())
                        .map(|(_, kind)| match kind {
                            LineRelationKind::Parallel => " [//]",
                            LineRelationKind::Perpendicular => " [T]",
                            LineRelationKind::Collinear => " [CL]",
                            LineRelationKind::Angle(_) => " [A]",
                        })
                        .or_else(|| self.document.line_relations.values().find_map(
                            |(source, kind)| (*source == shape.id()).then_some(match kind {
                                LineRelationKind::Parallel => " [//]",
                                LineRelationKind::Perpendicular => " [T]",
                                LineRelationKind::Collinear => " [CL]",
                                LineRelationKind::Angle(_) => " [A]",
                            })
                        ))
                        .unwrap_or("")
                ),
                1.3,
                if self.document.construction.contains(&shape.id()) {
                    MUTED
                } else {
                    TEXT
                },
            );
        }
        painter.text(
            layout.right.x + 18.0,
            layout.right.y + 19.0,
            "PROPERTIES",
            1.5,
            TEXT,
        );
        if let Some(solids) = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .filter(|_| self.solid_preview)
        {
            let selected = self.selected_solid.min(solids.len() - 1);
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 66.0,
                &solids[selected].name,
                1.7,
                ACCENT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 113.0,
                &format!("DEPTH   {:.1} mm", solids[selected].depth),
                1.25,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 155.0,
                if solids.len() == 1 {
                    "CLICK DEPTH TO EDIT"
                } else {
                    "[ ]  SELECT FEATURE"
                },
                1.0,
                MUTED,
            );
            if solids.len() > 1 {
                painter.text(
                    layout.right.x + 18.0,
                    layout.right.y + 175.0,
                    &format!("FEATURE {} / {}", selected + 1, solids.len()),
                    1.0,
                    MUTED,
                );
            }
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 190.0,
                "TAB  Sketch view",
                1.2,
                MUTED,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 230.0,
                "CTRL+E  EXPORT STL",
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 265.0,
                "DEL  REMOVE FEATURE",
                1.2,
                MUTED,
            );
        } else if let Some(shape) = self
            .document
            .shapes
            .iter()
            .find(|shape| Some(shape.id()) == self.document.selected)
        {
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 66.0,
                shape.name(),
                1.7,
                ACCENT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 90.0,
                if self.document.construction.contains(&shape.id()) {
                    "K  CONSTRUCTION ON"
                } else {
                    "K  CONSTRUCTION OFF"
                },
                1.0,
                MUTED,
            );
            match shape {
                Shape::Line { a, b, .. } => {
                    for (index, line) in [
                        format!("LENGTH  {:.1} mm", a.distance(*b)),
                        format!("START X {:.1} mm", a.x),
                        format!("START Y {:.1} mm", a.y),
                        format!("END X   {:.1} mm", b.x),
                        format!("END Y   {:.1} mm", b.y),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
                Shape::Rectangle { a, b, .. } => {
                    for (index, line) in [
                        format!("WIDTH   {:.1} mm", (a.x - b.x).abs()),
                        format!("HEIGHT  {:.1} mm", (a.y - b.y).abs()),
                        format!("X       {:.1} mm", a.x.min(b.x)),
                        format!("Y       {:.1} mm", a.y.min(b.y)),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
                Shape::RotatedRectangle {
                    center,
                    width,
                    height,
                    rotation,
                    ..
                } => {
                    for (index, line) in [
                        format!("WIDTH   {:.1} mm", width),
                        format!("HEIGHT  {:.1} mm", height),
                        format!("ANGLE   {:.1} deg", rotation),
                        format!("CENTER X {:.1} mm", center.x),
                        format!("CENTER Y {:.1} mm", center.y),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
                Shape::Circle { center, radius, .. } => {
                    for (index, line) in [
                        format!("DIAMETER {:.1} mm", radius * 2.0),
                        format!("CENTER X {:.1} mm", center.x),
                        format!("CENTER Y {:.1} mm", center.y),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
                Shape::Ellipse {
                    center,
                    rx,
                    ry,
                    rotation,
                    ..
                } => {
                    for (index, line) in [
                        format!("WIDTH   {:.1} mm", rx * 2.0),
                        format!("HEIGHT  {:.1} mm", ry * 2.0),
                        format!("ANGLE   {:.1} deg", rotation),
                        format!("CENTER X {:.1} mm", center.x),
                        format!("CENTER Y {:.1} mm", center.y),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
                Shape::EllipticArc {
                    rx,
                    ry,
                    rotation,
                    start_angle,
                    sweep,
                    ..
                } => {
                    for (index, line) in [
                        format!("WIDTH   {:.1} mm", rx * 2.0),
                        format!("HEIGHT  {:.1} mm", ry * 2.0),
                        format!("ANGLE   {:.1} deg", rotation),
                        format!("START   {:.1} deg", start_angle.to_degrees()),
                        format!("SWEEP   {:.1} deg", sweep.to_degrees()),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
                Shape::Spline {
                    mode,
                    points,
                    closed,
                    ..
                } => {
                    for (index, line) in [
                        format!(
                            "{} SPLINE",
                            if *mode == spline::SplineMode::Fit {
                                "FIT"
                            } else {
                                "CONTROL"
                            }
                        ),
                        format!("POINTS  {}", points.len()),
                        format!("CLOSED  {}", if *closed { "YES" } else { "NO" }),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                    if let Some(index) = self.document.selected_spline_point {
                        if let Some(point) = points.get(index) {
                            let fixed = self
                                .document
                                .fixed_anchors
                                .contains_key(&(shape.id(), index));
                            painter.text(
                                layout.right.x + 18.0,
                                layout.right.y + 193.0,
                                &format!("POINT {} X {:.1} mm", index + 1, point.x),
                                1.1,
                                if fixed { MUTED } else { ACCENT },
                            );
                            painter.text(
                                layout.right.x + 18.0,
                                layout.right.y + 223.0,
                                &format!("POINT {} Y {:.1} mm", index + 1, point.y),
                                1.1,
                                if fixed { MUTED } else { ACCENT },
                            );
                            if fixed {
                                painter.text(
                                    layout.right.x + 18.0,
                                    layout.right.y + 253.0,
                                    "FIXED  SHIFT+K TO RELEASE",
                                    1.0,
                                    ACCENT,
                                );
                            }
                        }
                    }
                }
                Shape::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep,
                    ..
                } => {
                    for (index, line) in [
                        format!("RADIUS  {:.1} mm", radius),
                        format!("SWEEP   {:.1} deg", sweep.to_degrees()),
                        format!("START   {:.1} deg", start_angle.to_degrees()),
                        format!("CENTER X {:.1} mm", center.x),
                        format!("CENTER Y {:.1} mm", center.y),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
                Shape::Point { at, .. } => {
                    for (index, line) in [format!("X {:.1} mm", at.x), format!("Y {:.1} mm", at.y)]
                        .into_iter()
                        .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                    if self.document.fixed_anchors.contains_key(&(shape.id(), 0)) {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 173.0,
                            "FIXED  SHIFT+K TO RELEASE",
                            1.0,
                            ACCENT,
                        );
                    }
                }
                Shape::Polygon {
                    center,
                    radius,
                    sides,
                    rotation,
                    ..
                } => {
                    for (index, line) in [
                        format!("SIDES    {sides}"),
                        format!("RADIUS   {:.1} mm", radius),
                        format!("ANGLE    {:.1} deg", rotation),
                        format!("CENTER X {:.1} mm", center.x),
                        format!("CENTER Y {:.1} mm", center.y),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
                Shape::Capsule {
                    from, to, width, ..
                } => {
                    for (index, line) in [
                        format!("WIDTH    {:.1} mm", width),
                        format!("DISTANCE {:.1} mm", from.distance(*to)),
                        format!("FROM X   {:.1} mm", from.x),
                        format!("FROM Y   {:.1} mm", from.y),
                        format!("TO X     {:.1} mm", to.x),
                        format!("TO Y     {:.1} mm", to.y),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 113.0 + index as f32 * 26.0,
                            &line,
                            1.2,
                            TEXT,
                        );
                    }
                }
            }
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 270.0,
                "DEL  Remove",
                1.2,
                MUTED,
            );
            if !matches!(shape, Shape::Arc { .. })
                || self
                    .document
                    .fixed_anchors
                    .keys()
                    .any(|(id, _)| *id == shape.id())
            {
                painter.text(
                    layout.right.x + 18.0,
                    layout.right.y + 306.0,
                    if matches!(shape, Shape::Arc { .. } | Shape::EllipticArc { .. })
                        && self
                            .document
                            .fixed_anchors
                            .keys()
                            .any(|(id, _)| *id == shape.id())
                    {
                        "FIXED ARC ANCHOR; SHIFT+K"
                    } else if matches!(
                        shape,
                        Shape::Ellipse { .. }
                            | Shape::RotatedRectangle { .. }
                            | Shape::Polygon { .. }
                            | Shape::Capsule { .. }
                    ) && self
                        .document
                        .fixed_anchors
                        .keys()
                        .any(|(id, _)| *id == shape.id())
                    {
                        "FIXED ANCHOR; SHIFT+K"
                    } else if self.document.fixed_anchors.contains_key(&(shape.id(), 0))
                        && matches!(
                            shape,
                            Shape::Circle { .. }
                                | Shape::Arc { .. }
                                | Shape::Ellipse { .. }
                                | Shape::EllipticArc { .. }
                                | Shape::Polygon { .. }
                                | Shape::RotatedRectangle { .. }
                        )
                    {
                        "FIXED CENTER; SHIFT+K RELEASE"
                    } else if matches!(shape, Shape::Polygon { .. }) {
                        "CLICK SIDES OR RADIUS TO EDIT"
                    } else if matches!(shape, Shape::Capsule { .. }) {
                        "CLICK WIDTH TO EDIT"
                    } else if self.document.midpoint_links.contains_key(&shape.id()) {
                        "MIDPOINT; SHIFT+M"
                    } else if self.document.point_curve_links.contains_key(&shape.id()) {
                        "POINT ON CURVE; SHIFT+V"
                    } else if self.document.normal_links.contains_key(&shape.id()) {
                        "NORMAL; SHIFT+F"
                    } else if matches!(shape, Shape::Spline { .. }) {
                        "CLICK HANDLE X/Y; SHIFT+K FIX"
                    } else if matches!(shape, Shape::Point { .. }) {
                        "CLICK X/Y; SHIFT+K FIX"
                    } else if self.document.shared_ellipses.contains_key(&shape.id())
                        || self
                            .document
                            .shared_ellipses
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        "SHARED ELLIPSE; LINKED"
                    } else if self.document.midpoint_links.contains_key(&shape.id())
                        || self
                            .document
                            .midpoint_links
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        "MIDPOINT; SHIFT+M"
                    } else if self.document.internal_tangents.contains_key(&shape.id())
                        || self
                            .document
                            .internal_tangents
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        "INTERNAL TANGENT; SHIFT+Y"
                    } else if self.document.external_tangents.contains_key(&shape.id())
                        || self
                            .document
                            .external_tangents
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        "TANGENT; SHIFT+Q"
                    } else if self.document.radius_differences.contains_key(&shape.id())
                        || self
                            .document
                            .radius_differences
                            .values()
                            .any(|(source, _)| *source == shape.id())
                    {
                        "RADIAL DISTANCE; SHIFT+D"
                    } else if self.document.concentric.contains_key(&shape.id())
                        || self
                            .document
                            .concentric
                            .values()
                            .any(|source| *source == shape.id())
                    {
                        "CONCENTRIC; SHIFT+O UNLINK"
                    } else if matches!(shape, Shape::Circle { .. } | Shape::Arc { .. })
                        && (self.document.equal_radii.contains_key(&shape.id())
                            || self
                                .document
                                .equal_radii
                                .values()
                                .any(|source| *source == shape.id()))
                    {
                        "EQUAL RADIUS; SHIFT+E UNLINK"
                    } else if matches!(shape, Shape::Ellipse { .. } | Shape::EllipticArc { .. })
                        && (self.document.equal_ellipses.contains_key(&shape.id())
                            || self
                                .document
                                .equal_ellipses
                                .values()
                                .any(|source| *source == shape.id()))
                    {
                        "EQUAL AXES; SHIFT+E UNLINK"
                    } else if matches!(shape, Shape::Line { .. }) {
                        if let Some((_, kind)) =
                            self.document.line_relations.get(&shape.id()).or_else(|| {
                                self.document
                                    .line_relations
                                    .values()
                                    .find(|(source, _)| *source == shape.id())
                            })
                        {
                            match kind {
                                LineRelationKind::Parallel => "PARALLEL; SHIFT+P UNLINK",
                                LineRelationKind::Perpendicular => "PERPENDICULAR; SHIFT+N UNLINK",
                                LineRelationKind::Collinear => "COLLINEAR; SHIFT+L UNLINK",
                                LineRelationKind::Angle(_) => "ANGLE; SHIFT+B UNLINK",
                            }
                        } else if self.document.equal_lines.contains_key(&shape.id())
                            || self
                                .document
                                .equal_lines
                                .values()
                                .any(|source| *source == shape.id())
                        {
                            "EQUAL LINE; SHIFT+E UNLINK"
                        } else if self
                            .document
                            .fixed_anchors
                            .keys()
                            .any(|(id, _)| *id == shape.id())
                        {
                            "FIXED END; SHIFT+K RELEASE"
                        } else {
                            "HOVER END; SHIFT+K FIX"
                        }
                    } else if matches!(shape, Shape::Rectangle { .. }) {
                        if self
                            .document
                            .fixed_anchors
                            .keys()
                            .any(|(id, _)| *id == shape.id())
                        {
                            "FIXED CORNER; SHIFT+K"
                        } else {
                            "HOVER CORNER; SHIFT+K FIX"
                        }
                    } else {
                        "CLICK DIMENSION TO EDIT"
                    },
                    1.0,
                    MUTED,
                );
            }
            if let Some((_, LineRelationKind::Angle(value))) =
                self.document.line_relations.get(&shape.id())
            {
                painter.text(
                    layout.right.x + 18.0,
                    layout.right.y + 325.0,
                    &format!("ANGLE {:.1} DEG", value),
                    1.0,
                    ACCENT,
                );
            }
            if let Some((_, value)) = self.document.radius_differences.get(&shape.id()) {
                painter.text(
                    layout.right.x + 18.0,
                    layout.right.y + 325.0,
                    &format!("DIFF {:.1} MM", value),
                    1.0,
                    ACCENT,
                );
            }
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 342.0,
                "MOVE / COPY",
                1.2,
                ACCENT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 372.0,
                &format!("MOVE X   {:.1} mm", self.move_offset.x),
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 402.0,
                &format!("MOVE Y   {:.1} mm", self.move_offset.y),
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 440.0,
                "MOVE SELECTED",
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 475.0,
                "COPY SELECTED",
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 532.0,
                "SCALE",
                1.2,
                if self.transform_rotate { MUTED } else { ACCENT },
            );
            painter.text(
                layout.right.x + layout.right.w * 0.5 + 8.0,
                layout.right.y + 532.0,
                "ROTATE",
                1.2,
                if self.transform_rotate { ACCENT } else { MUTED },
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 562.0,
                &format!("CX {:.1}", self.transform_center.x),
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + layout.right.w * 0.5 + 8.0,
                layout.right.y + 562.0,
                &format!("CY {:.1}", self.transform_center.y),
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 592.0,
                &if self.transform_rotate {
                    format!("ANGLE    {:.1} deg", self.rotate_angle)
                } else {
                    format!("FACTOR   {:.2}", self.scale_factor)
                },
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 625.0,
                if self.transform_rotate {
                    "ROTATE SELECTED"
                } else {
                    "SCALE SELECTED"
                },
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 655.0,
                if self.transform_rotate {
                    "ROTATE COPY"
                } else {
                    "SCALE COPY"
                },
                1.2,
                TEXT,
            );
        } else {
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 70.0,
                "Select an entity",
                1.3,
                MUTED,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 105.0,
                "to inspect it.",
                1.3,
                MUTED,
            );
        }
        if let Some(edit) = &self.editing {
            let row = match edit.field {
                EditField::ExtrusionDepth => 0,
                EditField::Dimension(
                    DimensionField::LineLength
                    | DimensionField::RectangleWidth
                    | DimensionField::CircleDiameter,
                )
                | EditField::PolygonSides
                | EditField::SlotWidth => 0,
                EditField::Dimension(DimensionField::CurveRadius) => {
                    if self
                        .document
                        .selected
                        .and_then(|id| self.document.shapes.iter().find(|shape| shape.id() == id))
                        .is_some_and(|shape| matches!(shape, Shape::Polygon { .. }))
                    {
                        1
                    } else {
                        0
                    }
                }
                EditField::Dimension(DimensionField::ArcSweep) => {
                    if self
                        .document
                        .selected
                        .and_then(|id| self.document.shapes.iter().find(|shape| shape.id() == id))
                        .is_some_and(|shape| matches!(shape, Shape::EllipticArc { .. }))
                    {
                        4
                    } else {
                        1
                    }
                }
                EditField::Dimension(DimensionField::Rotation) => 2,
                EditField::Dimension(DimensionField::RectangleHeight) => 1,
                EditField::MoveX
                | EditField::MoveY
                | EditField::ScaleFactor
                | EditField::RotateAngle
                | EditField::CenterX
                | EditField::CenterY
                | EditField::SplineX
                | EditField::SplineY
                | EditField::PointX
                | EditField::PointY
                | EditField::RelationAngle
                | EditField::RadialDifference => 0,
            };
            let field_y = match edit.field {
                EditField::MoveX => layout.right.y + 365.0,
                EditField::MoveY => layout.right.y + 395.0,
                EditField::ScaleFactor | EditField::RotateAngle => layout.right.y + 580.0,
                EditField::CenterX | EditField::CenterY => layout.right.y + 550.0,
                EditField::SplineX => layout.right.y + 195.0,
                EditField::SplineY => layout.right.y + 225.0,
                EditField::PointX => layout.right.y + 105.0,
                EditField::PointY => layout.right.y + 135.0,
                EditField::RelationAngle => layout.right.y + 312.0,
                EditField::RadialDifference => layout.right.y + 312.0,
                _ => layout.right.y + 105.0 + row as f32 * 30.0,
            };
            let center_half = matches!(edit.field, EditField::CenterX | EditField::CenterY);
            let field = Rect {
                x: if matches!(edit.field, EditField::CenterY) {
                    layout.right.x + layout.right.w * 0.5
                } else {
                    layout.right.x + 10.0
                },
                y: field_y,
                w: if center_half {
                    layout.right.w * 0.5 - 10.0
                } else {
                    layout.right.w - 20.0
                },
                h: 25.0,
            };
            painter.rect(field, [0.13, 0.29, 0.37, 1.0]);
            painter.outline(field, 1.0, ACCENT);
            painter.text(
                field.x + 8.0,
                field.y + 7.0,
                &if matches!(edit.field, EditField::PolygonSides) {
                    format!("> {}_ sides", edit.buffer)
                } else if matches!(edit.field, EditField::ScaleFactor) {
                    format!("> {}_ factor", edit.buffer)
                } else if matches!(edit.field, EditField::RotateAngle) {
                    format!("> {}_ deg", edit.buffer)
                } else if center_half {
                    format!("> {}_", edit.buffer)
                } else {
                    format!("> {}_ mm", edit.buffer)
                },
                1.25,
                TEXT,
            );
        }
        let tool_status = match self.document.tool {
            Tool::Select => "Select geometry  |  Middle drag: pan  |  Wheel: zoom",
            Tool::Line => "Line: click start, then end  |  Esc: cancel",
            Tool::Rectangle => "Rectangle: click two corners  |  Esc: cancel",
            Tool::CenterRectangle => "Center rectangle: click center, then corner  |  Esc: cancel",
            Tool::ThreePointRectangle => {
                "3-point rectangle: first edge, then height  |  Esc: cancel"
            }
            Tool::ThreePointCenterRectangle => {
                "3-point center rectangle: center, side midpoint, height"
            }
            Tool::Circle => "Circle: click center, then radius  |  Esc: cancel",
            Tool::ThreePointCircle => {
                "3-point circle: click three points on its rim  |  Esc: cancel"
            }
            Tool::Ellipse => "Ellipse: click center, axis end, then height  |  Esc: cancel",
            Tool::FitSpline => {
                "Fit spline: click points  |  Enter: finish  Shift+Enter: close  Esc: cancel"
            }
            Tool::ControlSpline => {
                "Control spline: click 3n+1 points  |  Enter: finish  Esc: cancel"
            }
            Tool::Arc => "Arc: click start, end, then through point  |  Esc: cancel",
            Tool::CenterArc => "Center arc: center, start, end direction  |  W: clockwise",
            Tool::Point => "Point: click to place  |  Esc: cancel",
            Tool::Polygon => "Polygon: click center, then vertex  |  Esc: cancel",
            Tool::Slot => "Slot: click two centers, then side wall  |  Esc: cancel",
        };
        let name_status = self.name_edit.as_ref().map(|edit| {
            format!(
                "RENAME {}: {}_  |  Enter: apply  Esc: cancel",
                match edit.target {
                    NameTarget::Part => "PART",
                    NameTarget::Sketch => "SKETCH",
                },
                edit.buffer
            )
        });
        let status = if let Some(name_status) = &name_status {
            name_status.as_str()
        } else if self.status_message.is_empty() {
            tool_status
        } else {
            &self.status_message
        };
        painter.text(15.0, layout.bottom.y + 10.0, status, 1.2, MUTED);
        painter.text(
            layout.top.w - 110.0,
            layout.bottom.y + 10.0,
            &format!(
                "{:.0}%",
                if self.solid_preview {
                    self.solid_view.zoom
                } else {
                    self.document.zoom
                } * 100.0
            ),
            1.2,
            TEXT,
        );
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("PuppyCAD")
            .with_inner_size(LogicalSize::new(1280.0, 800.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("window: {error}");
                event_loop.exit();
                return;
            }
        };
        match pollster::block_on(Renderer::new(window.clone())) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => {
                eprintln!("GPU initialization: {error}");
                event_loop.exit();
                return;
            }
        }
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.window.as_ref().is_none_or(|window| window.id() != id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                self.redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let next = Point {
                    x: position.x as f32,
                    y: position.y as f32,
                };
                let (dx, dy) = (next.x - self.cursor.x, next.y - self.cursor.y);
                if self.pan_drag {
                    self.document.pan.x += dx;
                    self.document.pan.y += dy;
                }
                match self.solid_drag {
                    Some(SolidDrag::Rotate) => self.solid_view.rotate_drag(dx, dy),
                    Some(SolidDrag::Pan { units_per_pixel }) => {
                        self.solid_view.pan_drag(dx, dy, units_per_pixel)
                    }
                    None => {}
                }
                self.cursor = next;
                if self.document.dragging() && !self.pan_drag && self.solid_drag.is_none() {
                    let world = self.document.screen_to_world(next, self.viewport_origin());
                    self.document.drag_to(world);
                }
                self.redraw();
            }
            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, MouseButton::Left) => self.handle_left_press(),
                (ElementState::Released, MouseButton::Left) => {
                    if self.document.finish_drag() {
                        self.status_message = "Moved geometry  |  Ctrl+S to save".into();
                    }
                    self.redraw();
                }
                (ElementState::Pressed, button @ (MouseButton::Right | MouseButton::Middle)) => {
                    let layout = self.layout();
                    if !self.begin_solid_drag(layout, self.cursor, button)
                        && button == MouseButton::Middle
                        && !self.solid_preview
                    {
                        self.pan_drag = true;
                    }
                }
                (ElementState::Released, MouseButton::Right | MouseButton::Middle) => {
                    self.solid_drag = None;
                    self.pan_drag = false;
                }
                _ => {}
            },
            WindowEvent::MouseWheel { delta, .. } => {
                if self.projects_page.is_some() {
                    let rows = match delta {
                        MouseScrollDelta::LineDelta(_, y) => -y.signum() as isize,
                        MouseScrollDelta::PixelDelta(position) => {
                            -(position.y / PROJECT_ROW_STEP as f64).round() as isize
                        }
                    };
                    self.scroll_projects_page(rows, self.layout());
                    self.redraw();
                } else if self.solid_preview
                    && self
                        .layout()
                        .viewport
                        .contains(self.cursor.x, self.cursor.y)
                {
                    // Browser wheel deltas: about 100 px per notch, positive when scrolling down.
                    let delta_y = match delta {
                        MouseScrollDelta::LineDelta(_, y) => -y * 100.0,
                        MouseScrollDelta::PixelDelta(position) => -position.y as f32,
                    };
                    self.solid_view.zoom_wheel(delta_y);
                    self.redraw();
                } else if self
                    .layout()
                    .viewport
                    .contains(self.cursor.x, self.cursor.y)
                {
                    let amount = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(position) => position.y as f32 / 100.0,
                    };
                    self.document.zoom_at(
                        self.cursor,
                        self.viewport_origin(),
                        1.12_f32.powf(amount),
                    );
                    self.redraw();
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    if self.projects_page.is_some() {
                        let layout = self.layout();
                        match code {
                            KeyCode::Escape => self.close_projects_page(),
                            KeyCode::ArrowDown => self.select_project_row(1, layout),
                            KeyCode::ArrowUp => self.select_project_row(-1, layout),
                            KeyCode::Enter | KeyCode::NumpadEnter => {
                                if let Some(index) =
                                    self.projects_page.as_ref().map(|page| page.selected)
                                {
                                    self.open_listed_project(index);
                                }
                            }
                            KeyCode::Backspace => self.show_parent_folder(),
                            KeyCode::KeyS
                                if self.modifiers.control_key() || self.modifiers.super_key() =>
                            {
                                self.save_project()
                            }
                            KeyCode::PageDown => self.select_project_row(
                                project_page_visible_rows(layout) as isize,
                                layout,
                            ),
                            KeyCode::PageUp => self.select_project_row(
                                -(project_page_visible_rows(layout) as isize),
                                layout,
                            ),
                            _ => {}
                        }
                        self.redraw();
                        return;
                    }
                    if self.name_edit.is_some() {
                        match code {
                            KeyCode::Enter | KeyCode::NumpadEnter => self.finish_rename(),
                            KeyCode::Escape => {
                                self.name_edit = None;
                                self.status_message = "Rename cancelled".into();
                            }
                            KeyCode::Backspace => {
                                if let Some(edit) = &mut self.name_edit {
                                    edit.buffer.pop();
                                }
                            }
                            KeyCode::KeyA
                                if self.modifiers.control_key() || self.modifiers.super_key() =>
                            {
                                if let Some(edit) = &mut self.name_edit {
                                    edit.buffer.clear();
                                }
                            }
                            _ if !self.modifiers.control_key() && !self.modifiers.super_key() => {
                                if let (Some(edit), Some(text)) = (&mut self.name_edit, &event.text)
                                {
                                    for ch in text.chars() {
                                        if !ch.is_control() && edit.buffer.chars().count() < 80 {
                                            edit.buffer.push(ch);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                        self.redraw();
                        return;
                    }
                    if self.editing.is_some() {
                        match code {
                            KeyCode::Enter | KeyCode::NumpadEnter => self.finish_dimension_edit(),
                            KeyCode::Escape => {
                                self.editing = None;
                                self.status_message = "Dimension edit cancelled".into();
                            }
                            KeyCode::Backspace => {
                                if let Some(editing) = &mut self.editing {
                                    editing.buffer.pop();
                                }
                            }
                            _ if !self.modifiers.control_key() && !self.modifiers.super_key() => {
                                if let (Some(editing), Some(text)) =
                                    (&mut self.editing, &event.text)
                                {
                                    for ch in text.chars() {
                                        if (ch.is_ascii_digit()
                                            || ch == '.'
                                            || ((ch == '-' || ch == '+')
                                                && editing.buffer.is_empty()
                                                && matches!(
                                                    editing.field,
                                                    EditField::MoveX
                                                        | EditField::MoveY
                                                        | EditField::RotateAngle
                                                        | EditField::RelationAngle
                                                        | EditField::RadialDifference
                                                        | EditField::Dimension(
                                                            DimensionField::ArcSweep
                                                        )
                                                        | EditField::Dimension(
                                                            DimensionField::Rotation
                                                        )
                                                        | EditField::CenterX
                                                        | EditField::CenterY
                                                        | EditField::SplineX
                                                        | EditField::SplineY
                                                        | EditField::PointX
                                                        | EditField::PointY
                                                )))
                                            && editing.buffer.len() < 16
                                        {
                                            editing.buffer.push(ch);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                        self.redraw();
                        return;
                    }
                    match code {
                        KeyCode::Enter | KeyCode::NumpadEnter
                            if matches!(
                                self.document.tool,
                                Tool::FitSpline | Tool::ControlSpline
                            ) =>
                        {
                            self.finish_spline(self.modifiers.shift_key());
                        }
                        KeyCode::F2 => self.begin_rename(if self.modifiers.shift_key() {
                            NameTarget::Sketch
                        } else {
                            NameTarget::Part
                        }),
                        KeyCode::Tab => {
                            if self
                                .project
                                .as_ref()
                                .and_then(ProjectFile::extrusion_previews)
                                .is_some()
                            {
                                self.solid_preview = !self.solid_preview;
                                self.status_message = if self.solid_preview {
                                    "3D extrusion preview  |  Tab: sketch"
                                } else {
                                    "Sketch view  |  Tab: 3D preview"
                                }
                                .into();
                            }
                        }
                        KeyCode::KeyD
                            if self.modifiers.control_key() || self.modifiers.super_key() =>
                        {
                            if !self.read_only() && self.document.copy_selected(self.move_offset) {
                                self.status_message = "Copied geometry  |  Ctrl+S to save".into();
                            }
                        }
                        KeyCode::KeyV
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.read_only()
                                && !self.solid_preview =>
                        {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(
                                            shape,
                                            Shape::Line { .. }
                                                | Shape::Circle { .. }
                                                | Shape::Arc { .. }
                                                | Shape::Ellipse { .. }
                                                | Shape::EllipticArc { .. }
                                                | Shape::Spline { .. }
                                        )
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => match self
                                    .document
                                    .toggle_point_on_curve_with(source)
                                {
                                    Ok(true) => "Point on curve linked  |  Ctrl+S to save".into(),
                                    Ok(false) => "Point on curve unlinked".into(),
                                    Err(error) => format!("Point on curve: {error}"),
                                },
                                None => "Hover a line or curve".into(),
                            };
                        }
                        KeyCode::KeyV => self.select_tool(Tool::Select),
                        KeyCode::KeyI if !self.read_only() && !self.solid_preview => {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let closed_curve = self.document.selected.and_then(|id| {
                                self.document
                                    .shapes
                                    .iter()
                                    .find(|shape| shape.id() == id)
                                    .and_then(|shape| {
                                        matches!(
                                            shape,
                                            Shape::Circle { .. } | Shape::Ellipse { .. }
                                        )
                                        .then(|| (id, shape.clone()))
                                    })
                            });
                            if let Some((id, shape)) = closed_curve {
                                let on_curve = match &shape {
                                    Shape::Circle { center, radius, .. } => {
                                        (world.distance(*center) - radius).abs()
                                            <= 8.0 / self.document.zoom.max(0.2)
                                    }
                                    Shape::Ellipse {
                                        center,
                                        rx,
                                        ry,
                                        rotation,
                                        ..
                                    } => {
                                        let angle = Shape::ellipse_parameter_angle(
                                            *center, *rx, *ry, *rotation, world,
                                        );
                                        world.distance(Shape::elliptic_arc_point(
                                            *center, *rx, *ry, *rotation, angle, 0.0, 0.0,
                                        )) <= 8.0 / self.document.zoom.max(0.2)
                                    }
                                    _ => false,
                                };
                                if let Some((_, first)) = self
                                    .split_circle_start
                                    .filter(|(pending_id, _)| *pending_id == id)
                                {
                                    let split = match shape {
                                        Shape::Circle { .. } => {
                                            self.document.split_selected_circle(first, world)
                                        }
                                        Shape::Ellipse { .. } => {
                                            self.document.split_selected_ellipse(first, world)
                                        }
                                        _ => unreachable!(),
                                    };
                                    self.status_message = match split {
                                        Ok(_) => {
                                            self.split_circle_start = None;
                                            "Curve split  |  Ctrl+Z to undo  |  Ctrl+S to save"
                                                .into()
                                        }
                                        Err(error) => format!("Split: {error}"),
                                    };
                                } else if on_curve {
                                    self.split_circle_start = Some((id, world));
                                    self.status_message =
                                        "Curve split: choose second point  |  Esc: cancel".into();
                                } else {
                                    self.status_message =
                                        "Split: move the pointer onto the selected curve".into();
                                }
                            } else {
                                self.split_circle_start = None;
                                self.status_message =
                                    match self.document.split_selected_curve(world) {
                                        Ok(_) => {
                                            "Curve split  |  Ctrl+Z to undo  |  Ctrl+S to save"
                                                .into()
                                        }
                                        Err(error) => format!("Split: {error}"),
                                    };
                            }
                        }
                        KeyCode::KeyQ
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview
                                && !self.read_only() =>
                        {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(
                                            shape,
                                            Shape::Line { .. }
                                                | Shape::Circle { .. }
                                                | Shape::Arc { .. }
                                                | Shape::Ellipse { .. }
                                                | Shape::EllipticArc { .. }
                                        )
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => {
                                    match self.document.toggle_external_tangent_with(source) {
                                        Ok(true) => "Tangent set  |  Ctrl+S to save".into(),
                                        Ok(false) => "Tangent removed".into(),
                                        Err(error) => format!("Tangent: {error}"),
                                    }
                                }
                                None => "Hover a line or curve for tangent".into(),
                            };
                        }
                        KeyCode::KeyQ if !self.read_only() && !self.solid_preview => {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            self.status_message = match self.document.trim_selected_curve(world) {
                                Ok(_) => {
                                    "Curve trimmed  |  Ctrl+Z to undo  |  Ctrl+S to save".into()
                                }
                                Err(error) => format!("Trim: {error}"),
                            };
                        }
                        KeyCode::KeyU if !self.read_only() && !self.solid_preview => {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            self.status_message = match self.document.extend_selected_curve(world) {
                                Ok(_) => {
                                    "Curve extended  |  Ctrl+Z to undo  |  Ctrl+S to save".into()
                                }
                                Err(error) => format!("Extend: {error}"),
                            };
                        }
                        KeyCode::KeyH | KeyCode::KeyJ
                            if !self.read_only()
                                && !self.solid_preview
                                && !self.modifiers.shift_key() =>
                        {
                            let axis = if code == KeyCode::KeyH {
                                AxisConstraint::Horizontal
                            } else {
                                AxisConstraint::Vertical
                            };
                            self.status_message = match self.document.toggle_axis_constraint(axis) {
                                Ok(true) => {
                                    format!("{} constraint set  |  Ctrl+S to save", axis.name())
                                }
                                Ok(false) => format!("{} constraint removed", axis.name()),
                                Err(error) => format!("Constraint: {error}"),
                            };
                        }
                        KeyCode::BracketLeft if !self.solid_preview => {
                            self.cycle_profile(-1);
                        }
                        KeyCode::BracketRight if !self.solid_preview => {
                            self.cycle_profile(1);
                        }
                        KeyCode::BracketLeft if self.solid_preview => self.cycle_solid(-1),
                        KeyCode::BracketRight if self.solid_preview => self.cycle_solid(1),
                        KeyCode::KeyX if self.modifiers.shift_key() => {
                            self.create_extrusion("cut");
                        }
                        KeyCode::KeyJ if self.modifiers.shift_key() => {
                            self.create_extrusion("join");
                        }
                        KeyCode::KeyX
                            if !self.modifiers.control_key() && !self.modifiers.super_key() =>
                        {
                            self.create_extrusion("newBody");
                        }
                        KeyCode::KeyL if !self.modifiers.shift_key() => {
                            self.select_tool(Tool::Line)
                        }
                        KeyCode::KeyR if self.modifiers.shift_key() => {
                            self.select_tool(Tool::CenterRectangle)
                        }
                        KeyCode::KeyR => self.select_tool(Tool::Rectangle),
                        KeyCode::KeyT if self.modifiers.shift_key() => {
                            self.select_tool(Tool::ThreePointCenterRectangle)
                        }
                        KeyCode::KeyT => self.select_tool(Tool::ThreePointRectangle),
                        KeyCode::KeyC if self.modifiers.shift_key() => {
                            self.select_tool(Tool::ThreePointCircle)
                        }
                        KeyCode::KeyC => self.select_tool(Tool::Circle),
                        KeyCode::KeyY
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview
                                && !self.read_only() =>
                        {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(shape, Shape::Circle { .. } | Shape::Arc { .. })
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => match self
                                    .document
                                    .toggle_internal_tangent_with(source)
                                {
                                    Ok(true) => "Internal tangent set  |  Ctrl+S to save".into(),
                                    Ok(false) => "Internal tangent removed".into(),
                                    Err(error) => format!("Internal tangent: {error}"),
                                },
                                None => "Hover another circle or arc for internal tangent".into(),
                            };
                        }
                        KeyCode::KeyD
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview
                                && !self.read_only() =>
                        {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(shape, Shape::Circle { .. })
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => {
                                    match self.document.toggle_radius_difference_with(source) {
                                        Ok(true) => "Radial distance set  |  Ctrl+S to save".into(),
                                        Ok(false) => "Radial distance removed".into(),
                                        Err(error) => format!("Radial distance: {error}"),
                                    }
                                }
                                None => "Hover another circle for radial distance".into(),
                            };
                        }
                        KeyCode::KeyO
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview
                                && !self.read_only() =>
                        {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(
                                            shape,
                                            Shape::Circle { .. }
                                                | Shape::Arc { .. }
                                                | Shape::Ellipse { .. }
                                                | Shape::EllipticArc { .. }
                                        )
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => {
                                    match self.document.toggle_concentric_with(source) {
                                        Ok(true) => "Concentric set  |  Ctrl+S to save".into(),
                                        Ok(false) => "Concentric removed".into(),
                                        Err(error) => format!("Concentric: {error}"),
                                    }
                                }
                                None => "Hover another circle, arc or ellipse".into(),
                            };
                        }
                        KeyCode::KeyE
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview
                                && !self.read_only() =>
                        {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(
                                            shape,
                                            Shape::Line { .. }
                                                | Shape::Circle { .. }
                                                | Shape::Arc { .. }
                                                | Shape::Ellipse { .. }
                                                | Shape::EllipticArc { .. }
                                        )
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => {
                                    let circular = selected
                                        .and_then(|id| {
                                            self.document
                                                .shapes
                                                .iter()
                                                .find(|shape| shape.id() == id)
                                        })
                                        .is_some_and(|shape| {
                                            matches!(
                                                shape,
                                                Shape::Circle { .. } | Shape::Arc { .. }
                                            )
                                        });
                                    let elliptical = selected
                                        .and_then(|id| {
                                            self.document
                                                .shapes
                                                .iter()
                                                .find(|shape| shape.id() == id)
                                        })
                                        .is_some_and(|shape| {
                                            matches!(
                                                shape,
                                                Shape::Ellipse { .. } | Shape::EllipticArc { .. }
                                            )
                                        });
                                    match if elliptical {
                                        self.document.toggle_equal_ellipse_with(source)
                                    } else if circular {
                                        self.document.toggle_equal_radius_with(source)
                                    } else {
                                        self.document.toggle_equal_line_with(source)
                                    } {
                                        Ok(true) => "Equal set  |  Ctrl+S to save".into(),
                                        Ok(false) => "Equal removed".into(),
                                        Err(error) => format!("Equal: {error}"),
                                    }
                                }
                                None => "Hover another compatible curve for Equal".into(),
                            };
                        }
                        KeyCode::KeyE
                            if !self.modifiers.control_key() && !self.modifiers.super_key() =>
                        {
                            self.select_tool(Tool::Ellipse)
                        }
                        KeyCode::KeyA if self.modifiers.shift_key() => {
                            self.select_tool(Tool::CenterArc)
                        }
                        KeyCode::KeyA => self.select_tool(Tool::Arc),
                        KeyCode::KeyW if self.document.tool == Tool::CenterArc => {
                            self.document.arc_clockwise = !self.document.arc_clockwise;
                            self.status_message = if self.document.arc_clockwise {
                                "Center arc: clockwise sweep"
                            } else {
                                "Center arc: counterclockwise sweep"
                            }
                            .into();
                        }
                        code @ (KeyCode::KeyP | KeyCode::KeyN | KeyCode::KeyL | KeyCode::KeyB)
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview
                                && !self.read_only() =>
                        {
                            let kind = match code {
                                KeyCode::KeyP => Some(LineRelationKind::Parallel),
                                KeyCode::KeyN => Some(LineRelationKind::Perpendicular),
                                KeyCode::KeyL => Some(LineRelationKind::Collinear),
                                _ => None,
                            };
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(shape, Shape::Line { .. })
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => match if let Some(kind) = kind {
                                    self.document.toggle_line_relation_with(source, kind)
                                } else {
                                    self.document.toggle_line_angle_with(source)
                                } {
                                    Ok(true) => format!(
                                        "{} set  |  Ctrl+S to save",
                                        kind.map_or("angle", LineRelationKind::name)
                                    ),
                                    Ok(false) => format!(
                                        "{} removed",
                                        kind.map_or("angle", LineRelationKind::name)
                                    ),
                                    Err(error) => format!("Direction: {error}"),
                                },
                                None => "Hover another line for direction relation".into(),
                            };
                        }
                        KeyCode::KeyP => self.select_tool(Tool::Point),
                        KeyCode::KeyG if self.modifiers.shift_key() => {
                            self.snap_grid = !self.snap_grid;
                            self.status_message = format!(
                                "Grid snapping {}",
                                if self.snap_grid { "on" } else { "off" }
                            );
                        }
                        KeyCode::KeyG => self.select_tool(Tool::Polygon),
                        KeyCode::KeyN => {
                            self.snap_geometry = !self.snap_geometry;
                            self.status_message = format!(
                                "Geometry snapping {}",
                                if self.snap_geometry { "on" } else { "off" }
                            );
                        }
                        KeyCode::KeyS
                            if !self.modifiers.control_key() && !self.modifiers.super_key() =>
                        {
                            self.select_tool(Tool::Slot)
                        }
                        KeyCode::KeyM
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.read_only()
                                && !self.solid_preview =>
                        {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(
                                            shape,
                                            Shape::Line { .. }
                                                | Shape::Arc { .. }
                                                | Shape::EllipticArc { .. }
                                        )
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => match self.document.toggle_midpoint_with(source) {
                                    Ok(true) => "Midpoint linked  |  Ctrl+S to save".into(),
                                    Ok(false) => "Midpoint unlinked".into(),
                                    Err(error) => format!("Midpoint: {error}"),
                                },
                                None => "Hover a line or arc midpoint".into(),
                            };
                        }
                        KeyCode::KeyM => self.more_open = !self.more_open,
                        KeyCode::KeyB => self.create_part(),
                        KeyCode::KeyK => {
                            if !self.read_only() {
                                if self.modifiers.shift_key() {
                                    let line_selected = self
                                        .document
                                        .selected
                                        .and_then(|id| {
                                            self.document
                                                .shapes
                                                .iter()
                                                .find(|shape| shape.id() == id)
                                        })
                                        .is_some_and(|shape| {
                                            matches!(
                                                shape,
                                                Shape::Line { .. } | Shape::Rectangle { .. }
                                            )
                                        });
                                    let arc_selected = self
                                        .document
                                        .selected
                                        .and_then(|id| {
                                            self.document
                                                .shapes
                                                .iter()
                                                .find(|shape| shape.id() == id)
                                        })
                                        .is_some_and(|shape| {
                                            matches!(
                                                shape,
                                                Shape::Arc { .. } | Shape::EllipticArc { .. }
                                            )
                                        });
                                    let anchored_shape_selected = self
                                        .document
                                        .selected
                                        .and_then(|id| {
                                            self.document
                                                .shapes
                                                .iter()
                                                .find(|shape| shape.id() == id)
                                        })
                                        .is_some_and(|shape| {
                                            matches!(
                                                shape,
                                                Shape::Ellipse { .. }
                                                    | Shape::RotatedRectangle { .. }
                                                    | Shape::Polygon { .. }
                                                    | Shape::Capsule { .. }
                                            )
                                        });
                                    let result = if line_selected {
                                        let at = self
                                            .document
                                            .screen_to_world(self.cursor, self.viewport_origin());
                                        self.document.toggle_fixed_line_endpoint_at(at)
                                    } else if arc_selected {
                                        let at = self
                                            .document
                                            .screen_to_world(self.cursor, self.viewport_origin());
                                        self.document
                                            .toggle_fixed_arc_anchor_at(at)
                                            .map(|(_, enabled)| enabled)
                                    } else if anchored_shape_selected {
                                        let at = self
                                            .document
                                            .screen_to_world(self.cursor, self.viewport_origin());
                                        self.document
                                            .toggle_fixed_shape_anchor_at(at)
                                            .map(|(_, enabled)| enabled)
                                    } else {
                                        self.document.toggle_fixed_anchor()
                                    };
                                    self.status_message = match result {
                                        Ok(enabled) => format!(
                                            "Fixed anchor {}  |  Ctrl+S to save",
                                            if enabled { "on" } else { "off" }
                                        ),
                                        Err(error) => error,
                                    };
                                } else if let Some(enabled) = self.document.toggle_construction() {
                                    self.status_message = format!(
                                        "Construction {}  |  Ctrl+S to save",
                                        if enabled { "on" } else { "off" }
                                    );
                                }
                            }
                        }
                        KeyCode::Digit1 => self.create_sketch(SketchPlane::Top),
                        KeyCode::Digit2 => self.create_sketch(SketchPlane::Front),
                        KeyCode::Digit3 => self.create_sketch(SketchPlane::Right),
                        KeyCode::KeyF
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.read_only()
                                && !self.solid_preview =>
                        {
                            let world = self
                                .document
                                .screen_to_world(self.cursor, self.viewport_origin());
                            let selected = self.document.selected;
                            let source = self
                                .document
                                .shapes
                                .iter()
                                .rev()
                                .find(|shape| {
                                    Some(shape.id()) != selected
                                        && matches!(
                                            shape,
                                            Shape::Line { .. }
                                                | Shape::Circle { .. }
                                                | Shape::Arc { .. }
                                                | Shape::Ellipse { .. }
                                                | Shape::EllipticArc { .. }
                                                | Shape::Spline { .. }
                                        )
                                        && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
                                })
                                .map(Shape::id);
                            self.status_message = match source {
                                Some(source) => {
                                    match self.document.toggle_normal_with(source, world) {
                                        Ok(true) => "Normal set  |  Ctrl+S to save".into(),
                                        Ok(false) => "Normal removed".into(),
                                        Err(error) => format!("Normal: {error}"),
                                    }
                                }
                                None => "Hover a separate line or curve for Normal".into(),
                            };
                        }
                        KeyCode::KeyF if !self.solid_preview => {
                            self.status_message = if self
                                .document
                                .fit_shapes(self.layout().viewport.w, self.layout().viewport.h)
                            {
                                "Sketch fitted to viewport".into()
                            } else {
                                "Draw geometry before fitting the sketch".into()
                            };
                        }
                        KeyCode::Escape => {
                            self.split_circle_start = None;
                            self.spline_points.clear();
                            if self.document.cancel_drag() {
                                self.status_message = "Move cancelled".into();
                            } else {
                                self.document.draft_start = None;
                                self.document.draft_end = None;
                                self.document.selected = None;
                            }
                        }
                        KeyCode::Delete | KeyCode::Backspace => {
                            if self.solid_preview {
                                if let Some(project) = &mut self.project {
                                    match project.delete_extrusion_at(self.selected_solid) {
                                        Ok(()) => {
                                            let remaining = project
                                                .extrusion_previews()
                                                .map_or(0, |solids| solids.len());
                                            self.selected_solid = self
                                                .selected_solid
                                                .min(remaining.saturating_sub(1));
                                            self.solid_preview = remaining > 0;
                                            self.status_message =
                                                "Extrusion removed  |  Ctrl+S to save".into();
                                        }
                                        Err(error) => {
                                            self.status_message =
                                                format!("Delete extrusion: {error}")
                                        }
                                    }
                                }
                            } else if !self.read_only() {
                                self.document.delete_selected();
                            }
                        }
                        KeyCode::KeyS
                            if self.modifiers.control_key() || self.modifiers.super_key() =>
                        {
                            self.save_project();
                        }
                        KeyCode::PageDown
                            if self.modifiers.control_key() || self.modifiers.super_key() =>
                        {
                            self.switch_part(1);
                        }
                        KeyCode::PageUp
                            if self.modifiers.control_key() || self.modifiers.super_key() =>
                        {
                            self.switch_part(-1);
                        }
                        KeyCode::KeyE
                            if self.modifiers.control_key() || self.modifiers.super_key() =>
                        {
                            self.export_stl();
                        }
                        KeyCode::KeyZ
                            if self.modifiers.control_key() || self.modifiers.super_key() =>
                        {
                            if !self.read_only() {
                                if self.modifiers.shift_key() {
                                    self.document.redo();
                                } else {
                                    self.document.undo();
                                }
                            }
                        }
                        KeyCode::KeyY
                            if self.modifiers.control_key() || self.modifiers.super_key() =>
                        {
                            if !self.read_only() {
                                self.document.redo();
                            }
                        }
                        _ => {}
                    }
                    self.redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let painter = self.paint();
                if let Some(renderer) = &mut self.renderer {
                    match renderer.draw(painter) {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            let size = self.window.as_ref().expect("window exists").inner_size();
                            renderer.resize(size.width, size.height);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(error) => eprintln!("render: {error}"),
                    }
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let argument = std::env::args().nth(1);
    if argument.as_deref() == Some("--help") {
        println!("Usage: puppycad [PROJECT.pcad]\n       puppycad --roundtrip IN.pcad OUT.pcad");
        return Ok(());
    }
    if argument.as_deref() == Some("--roundtrip") {
        let args: Vec<String> = std::env::args().skip(2).collect();
        let [input, output] = args.as_slice() else {
            eprintln!("Usage: puppycad --roundtrip IN.pcad OUT.pcad");
            std::process::exit(2);
        };
        if let Err(error) = puppycad::roundtrip(Path::new(input), Path::new(output)) {
            eprintln!("roundtrip failed: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let mut app = match argument {
        Some(path) => App::with_project(Path::new(&path))?,
        None => App::default(),
    };
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_keeps_three_columns_and_toolbar_separate() {
        let layout = Layout::new(1280.0, 800.0);
        assert_eq!(layout.viewport.x, layout.left.w);
        assert_eq!(layout.viewport.x + layout.viewport.w, layout.right.x);
        assert!(layout.viewport.h > 0.0);
        assert!(layout.tool_button(2).contains(265.0, 55.0));
        assert!(layout.tool_button(5).contains(675.0, 55.0));
        assert!(layout.tool_button(9).contains(1150.0, 55.0));
        assert!(layout.more_item(1).contains(1150.0, 145.0));
        assert!(layout.more_item(2).contains(1150.0, 180.0));
        assert!(layout.sketch_row(1, 0).contains(100.0, 187.0));
        assert!(layout.row(0, 0, 2).contains(100.0, 227.0));
        assert!(layout.part_row(1).contains(100.0, 187.0));
        assert!(layout.sketch_row(0, 2).contains(100.0, 222.0));
    }

    #[test]
    fn clip_text_keeps_path_tails_and_name_heads() {
        assert_eq!(clip_text("/home/user/cad", 20, true), "/home/user/cad");
        assert_eq!(clip_text("/home/user/cad", 8, true), "...r/cad");
        assert_eq!(clip_text("Long project name", 8, false), "Long ...");
    }

    #[test]
    fn breadcrumbs_lay_out_in_order_and_truncate_long_names() {
        let crumbs = layout_breadcrumbs(
            vec![
                (Crumb::Project, "two-parts".into()),
                (Crumb::Part, "A very long part name that keeps going".into()),
                (Crumb::Sketch, "Sketch One".into()),
            ],
            2000.0,
        );
        assert_eq!(crumbs.len(), 3);
        assert_eq!(crumbs[0].2.x, CRUMB_X);
        assert!(crumbs[1].2.x > crumbs[0].2.x + crumbs[0].2.w);
        assert!(crumbs[2].2.x > crumbs[1].2.x + crumbs[1].2.w);
        assert_eq!(crumbs[1].1.chars().count(), CRUMB_MAX_CHARS);
        assert!(crumbs[1].1.ends_with("..."));
        assert!(crumbs[0].2.contains(CRUMB_X + 5.0, CRUMB_Y + 5.0));

        let narrow = layout_breadcrumbs(
            vec![
                (Crumb::Project, "two-parts".into()),
                (Crumb::Part, "Part One".into()),
            ],
            CRUMB_X + 120.0,
        );
        assert_eq!(narrow.len(), 1);
    }

    #[test]
    fn breadcrumbs_follow_selection_and_navigate_up() {
        let mut app = App::with_project(Path::new("fixtures/segmented-primitives.pcad")).unwrap();
        app.solid_preview = false;
        let layout = Layout::new(1280.0, 800.0);
        let kinds = |app: &App| {
            app.breadcrumbs(layout)
                .into_iter()
                .map(|(crumb, _, _)| crumb)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            kinds(&app),
            [Crumb::Projects, Crumb::Project, Crumb::Part, Crumb::Sketch]
        );
        assert_eq!(app.breadcrumbs(layout)[1].1, "segmented-primitives");

        app.document.selected = app.document.shapes.first().map(Shape::id);
        assert!(app.document.selected.is_some());
        assert_eq!(
            kinds(&app),
            [
                Crumb::Projects,
                Crumb::Project,
                Crumb::Part,
                Crumb::Sketch,
                Crumb::Shape
            ]
        );
        let (sketch, shape) = {
            let crumbs = app.breadcrumbs(layout);
            (crumbs[3].2, crumbs[4].2)
        };
        let at = |rect: Rect| Point {
            x: rect.x + 2.0,
            y: rect.y + 2.0,
        };
        assert_eq!(app.breadcrumb_at(layout, at(shape)), None);
        assert_eq!(app.breadcrumb_at(layout, at(sketch)), Some(Crumb::Sketch));
        app.open_breadcrumb(Crumb::Sketch);
        assert_eq!(app.document.selected, None);
        assert_eq!(
            kinds(&app),
            [Crumb::Projects, Crumb::Project, Crumb::Part, Crumb::Sketch]
        );
    }

    #[test]
    fn part_breadcrumb_opens_solid_preview_when_part_has_extrusions() {
        let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
        app.solid_preview = false;
        let layout = Layout::new(1280.0, 800.0);
        let crumbs = app.breadcrumbs(layout);
        let (part, sketch) = (crumbs[2].2, crumbs[3].2);
        let at = |rect: Rect| Point {
            x: rect.x + 2.0,
            y: rect.y + 2.0,
        };
        assert_eq!(app.breadcrumb_at(layout, at(sketch)), None);
        assert_eq!(app.breadcrumb_at(layout, at(part)), Some(Crumb::Part));
        app.open_breadcrumb(Crumb::Part);
        assert!(app.solid_preview);
        let crumbs = app.breadcrumbs(layout);
        assert_eq!(crumbs.last().map(|crumb| crumb.0), Some(Crumb::Solid));
    }

    #[test]
    fn projects_crumb_opens_page_listing_the_project_folder() {
        let mut app = App::with_project(Path::new("fixtures/two-parts.pcad")).unwrap();
        let layout = Layout::new(1280.0, 800.0);
        let crumbs = app.breadcrumbs(layout);
        assert_eq!(crumbs[0].0, Crumb::Projects);
        assert_eq!(crumbs[1].0, Crumb::Project);
        let at = Point {
            x: crumbs[0].2.x + 2.0,
            y: crumbs[0].2.y + 2.0,
        };
        assert_eq!(app.breadcrumb_at(layout, at), Some(Crumb::Projects));
        app.open_breadcrumb(Crumb::Projects);

        let page = app.projects_page.as_ref().unwrap();
        assert_eq!(page.dir, Path::new("fixtures").canonicalize().unwrap());
        assert_eq!(page.entries[0].kind, EntryKind::Parent);
        assert_eq!(page.entries[page.selected].name, "two-parts");
        assert!(page.entries.iter().any(|entry| entry.name == "two-parts"));
        let kinds = app
            .breadcrumbs(layout)
            .into_iter()
            .map(|(crumb, _, _)| crumb)
            .collect::<Vec<_>>();
        assert_eq!(kinds, [Crumb::Projects]);
        assert_eq!(app.breadcrumb_at(layout, at), None);

        let row = project_page_row(layout, 0);
        assert_eq!(
            app.projects_page_entry_at(
                layout,
                Point {
                    x: row.x + 5.0,
                    y: row.y + 5.0
                }
            ),
            Some(0)
        );

        let own = page
            .entries
            .iter()
            .position(|entry| entry.name == "two-parts")
            .unwrap();
        app.open_listed_project(own);
        assert!(app.projects_page.is_none());
        assert_eq!(app.project.as_ref().unwrap().part_name(), "Part One");

        app.open_projects_page();
        app.close_projects_page();
        assert!(app.projects_page.is_none());
    }

    #[test]
    fn projects_page_navigates_folders_and_keeps_selection_in_view() {
        let mut app = App::default();
        app.open_projects_page();
        let fixtures = app
            .projects_page
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .position(|entry| entry.kind == EntryKind::Folder && entry.name == "fixtures")
            .unwrap();
        app.open_listed_project(fixtures);
        let page = app.projects_page.as_ref().unwrap();
        assert!(page.dir.ends_with("fixtures"));
        assert_eq!(page.entries[page.selected].kind, EntryKind::Project);

        app.show_parent_folder();
        let page = app.projects_page.as_ref().unwrap();
        assert_eq!(page.entries[page.selected].name, "fixtures");

        let short = Layout::new(1280.0, 420.0);
        let visible = project_page_visible_rows(short);
        let total = app.projects_page.as_ref().unwrap().entries.len();
        app.select_project_row(total as isize, short);
        let page = app.projects_page.as_ref().unwrap();
        assert_eq!(page.selected, total - 1);
        assert!(page.selected < page.scroll + visible);
        app.select_project_row(-(total as isize), short);
        assert_eq!(app.projects_page.as_ref().unwrap().scroll, 0);
    }

    #[test]
    fn opening_another_project_asks_before_discarding_unsaved_work() {
        let mut app = App::with_project(Path::new("fixtures/two-parts.pcad")).unwrap();
        app.open_projects_page();
        let target = app
            .projects_page
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .position(|entry| entry.name == "segmented-primitives")
            .unwrap();

        app.document.tool = Tool::Rectangle;
        app.document.draft_start = Some(Point { x: 0.0, y: 0.0 });
        assert!(app.document.finish_draft(Point { x: 20.0, y: 10.0 }));
        assert!(app.has_unsaved_work());

        app.open_listed_project(target);
        assert!(app.projects_page.is_some());
        assert!(
            app.project
                .as_ref()
                .unwrap()
                .path()
                .ends_with("two-parts.pcad")
        );
        assert!(app.status_message.contains("Unsaved changes"));

        app.open_listed_project(target);
        assert!(app.projects_page.is_none());
        assert!(
            app.project
                .as_ref()
                .unwrap()
                .path()
                .ends_with("segmented-primitives.pcad")
        );
        assert!(!app.has_unsaved_work());
    }

    #[test]
    fn projection_switch_preserves_camera_and_only_handles_solid_header() {
        let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
        let layout = Layout::new(1280.0, 800.0);
        app.solid_view.rotate_drag(12.0, -8.0);
        app.solid_view.pan_drag(10.0, 5.0, 0.5);
        app.solid_view.zoom_wheel(-100.0);
        let original = app.solid_view;
        let button = layout.projection_button(true);
        let point = Point {
            x: button.x + 10.0,
            y: button.y + 10.0,
        };
        assert!(app.select_projection(layout, point));
        assert!(app.solid_view.perspective);
        assert_eq!(
            app.solid_view,
            OrbitView {
                perspective: true,
                ..original
            }
        );
        assert!(
            app.breadcrumbs(layout)
                .iter()
                .all(|(_, _, rect)| rect.x + rect.w < button.x)
        );
        let button = layout.projection_button(false);
        assert!(app.select_projection(
            layout,
            Point {
                x: button.x + 10.0,
                y: button.y + 10.0
            }
        ));
        assert_eq!(app.solid_view, original);
        app.solid_preview = false;
        assert!(!app.select_projection(layout, point));
        app.solid_preview = true;
        app.open_projects_page();
        assert!(!app.select_projection(layout, point));
    }

    #[test]
    fn perspective_foreshortens_and_unprojects_the_picked_surface() {
        let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
        let layout = Layout::new(1280.0, 800.0);
        app.solid_view.perspective = true;
        let (scene, projector) = app.solid_projector(layout).unwrap();
        assert!(projector.perspective_factor(10.0) > projector.perspective_factor(-10.0));
        for world in &scene.vertices {
            let screen = projector.screen(*world);
            let model = projector.model_at(
                Point {
                    x: screen[0],
                    y: screen[1],
                },
                projector.depth(*world),
            );
            let expected = view3d::cad_to_model(view3d::sub(*world, projector.center));
            assert!((0..3).all(|i| (model[i] - expected[i]).abs() < 1e-3));
        }
        let point = Point {
            x: projector.origin[0] + 20.0,
            y: projector.origin[1] + 15.0,
        };
        let pivot = projector.pick_pivot(&scene, point);
        // The fixture is a rectangular prism: a picked point must lie on a face.
        let cad = [
            -pivot[0] + projector.center[0],
            pivot[2] + projector.center[1],
            pivot[1] + projector.center[2],
        ];
        assert!((0..3).any(|axis| {
            let lo = scene
                .vertices
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min);
            let hi = scene
                .vertices
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max);
            (cad[axis] - lo).abs() < 1e-3 || (cad[axis] - hi).abs() < 1e-3
        }));
        app.solid_view.set_pivot(pivot);
        app.solid_view.rotate_drag(15.0, -10.0);
        let (_, rotated) = app.solid_projector(layout).unwrap();
        let screen = rotated.screen(cad);
        assert!((screen[0] - point.x).abs() < 1e-3 && (screen[1] - point.y).abs() < 1e-3);
        let before = screen;
        app.solid_view
            .pan_drag(25.0, -12.0, rotated.units_per_pixel());
        let (_, panned) = app.solid_projector(layout).unwrap();
        let after = panned.screen(cad);
        assert!((after[0] - before[0] - 25.0).abs() < 1e-3);
        assert!((after[1] - before[1] + 12.0).abs() < 1e-3);
    }

    #[test]
    fn right_drag_orbits_around_the_surface_under_the_cursor() {
        let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
        assert!(app.solid_preview);
        let layout = Layout::new(1280.0, 800.0);
        let (scene, projector) = app.solid_projector(layout).unwrap();
        let center = Point {
            x: layout.viewport.x + layout.viewport.w * 0.5,
            y: layout.viewport.y + layout.viewport.h * 0.5,
        };
        let pivot = projector.pick_pivot(&scene, center);
        let at = projector.view.model_to_view(pivot);
        assert!(
            at[2] > 0.0,
            "the pivot is on the near surface, not the scene center"
        );

        assert!(app.begin_solid_drag(layout, center, MouseButton::Right));
        assert_eq!(app.solid_drag, Some(SolidDrag::Rotate));
        let (_, projector) = app.solid_projector(layout).unwrap();
        let before = projector.screen(projector.center);
        app.solid_view.rotate_drag(30.0, 10.0);
        let (_, rotated) = app.solid_projector(layout).unwrap();
        assert_ne!(rotated.screen(projector.center), before);
        let pivot_screen = |projector: &SolidProjector| {
            let v = projector.view.model_to_view(projector.view.pivot);
            [
                projector.origin[0] + v[0] * projector.scale,
                projector.origin[1] - v[1] * projector.scale,
            ]
        };
        let (a, b) = (pivot_screen(&projector), pivot_screen(&rotated));
        assert!((a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01);
        assert!(
            (a[0] - center.x).abs() < 0.5 && (a[1] - center.y).abs() < 0.5,
            "the pivot stays under the cursor"
        );
        assert_eq!(
            projector.scale, rotated.scale,
            "orbiting never refits the size"
        );
    }

    #[test]
    fn solid_drags_follow_web_buttons() {
        let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
        let layout = Layout::new(1280.0, 800.0);
        let inside = Point {
            x: layout.viewport.x + 40.0,
            y: layout.viewport.y + 40.0,
        };
        assert!(!app.begin_solid_drag(layout, inside, MouseButton::Left));
        assert!(app.begin_solid_drag(layout, inside, MouseButton::Middle));
        let Some(SolidDrag::Pan { units_per_pixel }) = app.solid_drag else {
            panic!("middle drag pans");
        };
        let (_, projector) = app.solid_projector(layout).unwrap();
        let corner = projector.center;
        let before = projector.screen(corner);
        app.solid_view.pan_drag(25.0, -12.0, units_per_pixel);
        let (_, panned) = app.solid_projector(layout).unwrap();
        let after = panned.screen(corner);
        assert!((after[0] - before[0] - 25.0).abs() < 0.01);
        assert!((after[1] - before[1] + 12.0).abs() < 0.01);

        assert!(!app.begin_solid_drag(layout, Point { x: 5.0, y: 300.0 }, MouseButton::Right));
        app.solid_preview = false;
        assert!(!app.begin_solid_drag(layout, inside, MouseButton::Right));
        app.solid_preview = true;
        app.open_projects_page();
        assert!(!app.begin_solid_drag(layout, inside, MouseButton::Right));
    }

    #[test]
    fn default_3d_view_shows_top_and_both_positive_sides() {
        let app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
        let (_, projector) = app.solid_projector(Layout::new(1280.0, 800.0)).unwrap();
        assert!(projector.facing([0.0, 0.0, 1.0]) > 0.0);
        assert!(projector.facing([1.0, 0.0, 0.0]) > 0.0);
        assert!(projector.facing([0.0, 1.0, 0.0]) > 0.0);
        let top = projector.screen(view3d::add(projector.center, [0.0, 0.0, 10.0]));
        let bottom = projector.screen(view3d::add(projector.center, [0.0, 0.0, -10.0]));
        assert!(top[1] < bottom[1], "CAD +Z is up on screen");
    }
}
