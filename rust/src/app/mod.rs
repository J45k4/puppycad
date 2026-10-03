mod corner;
mod input;
mod panels;
mod pattern;
mod project_panel;
mod solid;
mod view_sketch;

#[cfg(test)]
mod tests;

use corner::*;
use pattern::*;
use project_panel::*;
use solid::*;

use puppycad::{model, project, spline, stl};

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::renderer::{Color, Painter, Rect, Renderer};
use crate::view3d::{self, OrbitView};
use manifold_rust::manifold::Manifold;
use model::{
    AxisConstraint, ChamferMode, DimensionField, Document, LineRelationKind, Point,
    SelectionFilter, Shape, SketchAnalysis, SnapKind, Tool,
};
use project::{
    EntryKind, ProjectFile, ProjectSummary, SketchPlane, SolidPreview, list_projects,
    profiles_from_document,
};
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
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
/// Constraint-state colors (web intent: fully constrained reads as settled,
/// conflicts as errors, redundancy as a warning).
const FULLY_CONSTRAINED: Color = [0.46, 0.84, 0.52, 1.0];
const CONFLICT: Color = [0.96, 0.36, 0.36, 1.0];
const REDUNDANT: Color = [0.78, 0.56, 0.96, 1.0];

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

    /// Selection filter toggles in the status bar: 0 geometry,
    /// 1 construction, 2 points.
    fn filter_button(self, index: usize) -> Rect {
        Rect {
            x: self.top.w - 420.0 + index as f32 * 98.0,
            y: self.bottom.y + 4.0,
            w: 92.0,
            h: 23.0,
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

pub(crate) struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    document: Document,
    project: Option<ProjectFile>,
    status_message: String,
    cursor: Point,
    pan_drag: bool,
    /// Screen point where a Select-mode rubber band started.
    box_start: Option<Point>,
    /// Selection filter shared by every sketch (copied into the document).
    selection_filter: SelectionFilter,
    modifiers: ModifiersState,
    editing: Option<DimensionEdit>,
    name_edit: Option<NameEdit>,
    more_open: bool,
    /// Multi-click gesture in progress (symmetry or mirror).
    pick: Option<Pick>,
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
    /// MORE → FILLET/CHAMFER waiting for the corner's lines to be clicked.
    corner_pick: Option<CornerKind>,
    /// Lines of the corner whose fillet/chamfer prompt is open.
    corner_pair: Option<(u32, u32)>,
    corner_values: CornerValues,
    /// Linear/circular pattern prompt in progress.
    pattern_prompt: Option<PatternPrompt>,
    pattern_values: PatternValues,
    /// Constraint analysis keyed by `Document::revision`; refreshed once per
    /// committed edit, never during a drag.
    analysis: std::cell::RefCell<Option<(u64, SketchAnalysis)>>,
}

/// Viewport clicks a relation gesture is waiting for.
#[derive(Clone, Debug, PartialEq)]
enum Pick {
    /// Shift+S with a mirror line selected: click the first shape, then the
    /// second one.
    Symmetry { axis: u32, first: Option<Point> },
    /// Mirror tool: click the mirror line for these shapes.
    Mirror { sources: Vec<u32> },
    /// Circular pattern: click the rotation center for these shapes.
    PatternCenter { sources: Vec<u32> },
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
    FilletRadius,
    ChamferFirst,
    ChamferSecond,
    /// One step of the linear/circular pattern prompt.
    PatternValue,
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
            box_start: None,
            selection_filter: SelectionFilter::default(),
            modifiers: ModifiersState::empty(),
            editing: None,
            name_edit: None,
            more_open: false,
            pick: None,
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
            corner_pick: None,
            corner_pair: None,
            corner_values: CornerValues::default(),
            pattern_prompt: None,
            pattern_values: PatternValues::default(),
            analysis: Default::default(),
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
        self.pick = None;
    }

    /// Shift+S: arm the symmetry gesture with the selected line as axis.
    fn begin_symmetry(&mut self) {
        let axis = self.document.selected.filter(|&id| {
            self.document
                .shapes
                .iter()
                .any(|shape| shape.id() == id && matches!(shape, Shape::Line { .. }))
        });
        match axis {
            Some(axis) => {
                self.document.finish_drag();
                self.pick = Some(Pick::Symmetry { axis, first: None });
                self.status_message =
                    "Symmetry: click the first shape, then its mirror  |  Esc to cancel".into();
            }
            None => {
                self.status_message = "Select a mirror line, then Shift+S".into();
            }
        }
    }

    /// Mirror tool (Shift+I or MORE > MIRROR): mirror the selection across
    /// the next clicked line.
    fn begin_mirror(&mut self) {
        self.more_open = false;
        let sources: Vec<u32> = self.document.selected.into_iter().collect();
        if sources.is_empty() {
            self.status_message = "Select geometry to mirror first".into();
            return;
        }
        self.document.finish_drag();
        self.select_tool(Tool::Select);
        self.pick = Some(Pick::Mirror { sources });
        self.status_message = "Mirror: click the mirror line  |  Esc to cancel".into();
    }

    /// Handle a viewport click while a pick gesture is armed.
    fn handle_pick(&mut self, world: Point) {
        let tolerance = 7.0 / self.document.zoom.max(0.2);
        match self.pick.take() {
            Some(Pick::Symmetry { axis, first: None }) => {
                self.pick = Some(Pick::Symmetry {
                    axis,
                    first: Some(world),
                });
                self.status_message = "Symmetry: click the mirrored shape  |  Esc to cancel".into();
            }
            Some(Pick::Symmetry {
                axis,
                first: Some(first),
            }) => {
                self.status_message = match self
                    .document
                    .toggle_symmetry_at(axis, first, world, tolerance)
                {
                    Ok(true) => "Symmetry set  |  Ctrl+S to save".into(),
                    Ok(false) => "Symmetry removed".into(),
                    Err(error) => format!("Symmetry: {error}"),
                };
            }
            Some(Pick::Mirror { sources }) => {
                let axis = self
                    .document
                    .shapes
                    .iter()
                    .rev()
                    .find(|shape| {
                        matches!(shape, Shape::Line { .. })
                            && !sources.contains(&shape.id())
                            && shape.hit(world, tolerance)
                    })
                    .map(Shape::id);
                self.status_message = match axis {
                    Some(axis) => match self.document.mirror_shapes(&sources, axis) {
                        Ok(created) => format!(
                            "Mirrored {} shape{}  |  Ctrl+S to save",
                            created.len(),
                            if created.len() == 1 { "" } else { "s" }
                        ),
                        Err(error) => format!("Mirror: {error}"),
                    },
                    None => {
                        self.pick = Some(Pick::Mirror { sources });
                        "Mirror: click a line to mirror across  |  Esc to cancel".into()
                    }
                };
            }
            Some(Pick::PatternCenter { sources }) => {
                self.status_message = self.pattern_center_click(sources, world);
            }
            None => {}
        }
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

    pub(crate) fn with_project(path: &Path) -> Result<Self, String> {
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

    fn layout(&self) -> Layout {
        // Headless (tests): the default window size.
        self.window
            .as_ref()
            .map_or(Layout::new(1280.0, 800.0), |window| {
                let size = window.inner_size();
                Layout::new(size.width as f32, size.height as f32)
            })
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
                && let Some((scene, projector)) = self.solid_projector(layout)
            {
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
            "MIRROR [SHIFT I]",
            "FILLET [SHIFT W]",
            "CHAMFER [SHIFT H]",
            "LIN PATTERN [SHIFT Z]",
            "CIRC PATTERN [SHIFT U]",
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
}
