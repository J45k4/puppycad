mod assembly;
mod commands;
mod corner;
mod extrude;
mod feature_tree;
mod features;
mod history;
mod input;
mod palette;
mod panels;
mod pattern;
mod project_panel;
mod settings;
mod shortcut_overlay;
mod solid;
mod status;
mod tangent;
mod topology;
mod view_sketch;

#[cfg(test)]
mod assembly_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod input_tests;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod status_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod topology_tests;

use assembly::*;
pub(crate) use commands::shortcuts_markdown;
use commands::*;
use corner::*;
use extrude::*;
use features::*;
use palette::*;
use pattern::*;
use project_panel::*;
use settings::*;
#[cfg(test)]
use shortcut_overlay::*;
use solid::*;
use status::*;
use topology::*;

use puppycad::{model, project, sketch_value, spline, stl, threemf};

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::renderer::{Color, Painter, Rect, Renderer};
use crate::view3d::{self, OrbitView};
use manifold_rust::manifold::Manifold;
use model::{
    AxisConstraint, ChamferMode, DimensionField, DimensionKey, Document, LineRelationKind, Point,
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

const BG: Color = DARK.bg;
const PANEL: Color = DARK.panel;
const PANEL_LIGHT: Color = DARK.panel_light;
const BORDER: Color = DARK.border;
const TEXT: Color = DARK.text;
const MUTED: Color = DARK.muted;
const ACCENT: Color = DARK.accent;
const SHAPE: Color = DARK.shape;
/// Constraint-state colors (web intent: fully constrained reads as settled,
/// conflicts as errors, redundancy as a warning).
const FULLY_CONSTRAINED: Color = DARK.fully_constrained;
const CONFLICT: Color = DARK.conflict;
const REDUNDANT: Color = DARK.redundant;
/// Driving dimension labels and their dimension lines.
const DIMENSION: Color = DARK.dimension;
/// Select-mode hover pre-highlight (a lighter accent).
const HOVER: Color = DARK.hover;
/// Inference hints while placing.
const HINT: Color = DARK.hint;
/// Hovered planar face outline in the 3D preview.
const FACE_HOVER: Color = DARK.face_hover;
/// Active button fill and selected row fill.
const ACTIVE: Color = DARK.active;
const ACTIVE_ROW: Color = DARK.active_row;
const GRID: Color = DARK.grid;

#[derive(Clone, Copy, Debug)]
struct Layout {
    viewport: Rect,
    left: Rect,
    right: Rect,
    top: Rect,
    bottom: Rect,
    /// UI scale: physical pixels per layout unit. Every rect is in logical
    /// units; `physical` multiplies by this.
    scale: f32,
    /// Pixels the Project panel rows are scrolled up by.
    panel_scroll: f32,
}

const MORE_COLUMN_ROWS: usize = 18;

impl Layout {
    /// Layout for a physical surface at a UI scale: the logical canvas is
    /// the physical size divided by `scale`.
    fn scaled(physical_width: f32, physical_height: f32, scale: f32) -> Self {
        let scale = scale.max(0.1);
        Self {
            scale,
            ..Self::new(physical_width / scale, physical_height / scale)
        }
    }

    /// A layout rect in physical pixels.
    #[cfg(test)]
    fn physical(self, rect: Rect) -> Rect {
        Rect {
            x: rect.x * self.scale,
            y: rect.y * self.scale,
            w: rect.w * self.scale,
            h: rect.h * self.scale,
        }
    }

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
            scale: 1.0,
            panel_scroll: 0.0,
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
        // Eighteen rows fill the 800 px window; later items open a second
        // column to the left of the first rows.
        let (column, row) = if index < MORE_COLUMN_ROWS {
            (0.0, index)
        } else {
            (1.0, index - MORE_COLUMN_ROWS)
        };
        // Both columns are wider than the MORE button so labels such as
        // "CHAMFER [SHIFT H]" stay inside; the first column keeps the
        // button's x origin and grows to the right.
        let width = button.w + if column == 0.0 { 30.0 } else { 90.0 };
        Rect {
            x: button.x - column * (width + 4.0),
            y: 96.0 + row as f32 * 36.0,
            w: width,
            h: 32.0,
        }
    }

    fn part_row(self, index: usize) -> Rect {
        Rect {
            x: 12.0,
            y: self.left.y + 43.0 + index as f32 * 35.0 - self.panel_scroll,
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
                + (part_count + sketch_count.saturating_sub(1) + index) as f32 * 35.0
                - self.panel_scroll,
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
    /// The ROLLBACK row is being dragged in the Project panel.
    rollback_drag: bool,
    solid_view: OrbitView,
    /// Face under the cursor at the last pointer move in the 3D preview
    /// (kept while the MORE menu is open, for MORE → SKETCH ON FACE).
    hovered_face: Option<FaceHover>,
    /// Show (and snap to) a face sketch's supporting body; key 5 toggles.
    face_context: bool,
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
    /// Distance the Offset prompt starts from (and remembers).
    offset_value: f32,
    /// Linear/circular pattern prompt in progress.
    pattern_prompt: Option<PatternPrompt>,
    pattern_values: PatternValues,
    /// Constraint analysis keyed by `Document::revision`; refreshed once per
    /// committed edit, never during a drag.
    analysis: std::cell::RefCell<Option<(u64, SketchAnalysis)>>,
    /// Dimension picked by clicking its label.
    selected_dimension: Option<DimensionKey>,
    /// Label press in progress: a click edits it, a drag moves it.
    label_drag: Option<LabelDrag>,
    /// First anchor picked with the D tool.
    dimension_start: Option<(u32, usize)>,
    /// Open extrude prompt with its ghost preview.
    extrude_draft: Option<ExtrudeDraft>,
    /// Extrusion option/depth edits, newest last.
    solid_undo: Vec<SolidUndo>,
    solid_redo: Vec<SolidUndo>,
    /// Next viewport click picks the up-to-body target.
    body_pick: bool,
    /// Select-mode hover target; the viewport only repaints on pointer
    /// moves when this changes (see `pointer_moved`).
    hover: Hover,
    /// Draw relation glyphs beside entities; key 6 toggles.
    show_glyphs: bool,
    /// Add H/V to a line committed while its inference hint was showing
    /// (web "Automatic constraints"); key 7 toggles.
    auto_constraints: bool,
    /// Redraw requests, counted for tests.
    redraw_requests: std::cell::Cell<u64>,
    /// Command palette (Ctrl+K) while open.
    palette: Option<Palette>,
    /// Shortcut cheat sheet (? / F1) is shown.
    shortcut_overlay: bool,
    /// Persisted user settings (units, grid, theme, UI scale, window).
    settings: Settings,
    settings_open: bool,
    /// Active UI scale and the monitor's rounded scale factor.
    ui_scale: f32,
    /// Project panel scroll in pixels (clamped in `layout`).
    panel_scroll: f32,
    /// Row last scrolled into view: (3D view, feature, sketch).
    panel_followed: (bool, usize, usize),
    monitor_scale: f32,
    status_cache: std::cell::RefCell<StatusCache>,
    /// Hovered toolbar button / MORE row and when the hover began.
    tip_hover: Option<(HoverTarget, std::time::Instant)>,
    /// Revolve in progress: axis pick, then the angle prompt and ghost.
    revolve_draft: Option<RevolveDraft>,
    /// First loft profile (sketch index, profile index).
    loft_start: Option<(usize, usize)>,
    /// Edge chamfer/fillet picking in the 3D preview.
    edge_pick: Option<EdgePick>,
    /// HISTORY panel over the Project panel (Ctrl+Shift+H).
    history_open: bool,
    /// Open assembly view (replaces the sketch / part views while open).
    assembly: Option<AssemblyView>,
}

/// What the pointer is over in Select mode.
#[derive(Clone, Debug, Default, PartialEq)]
struct Hover {
    shape: Option<u32>,
    anchor: Option<(u32, usize)>,
    glyph: Option<model::RelationGlyph>,
}

#[derive(Clone, Copy)]
struct LabelDrag {
    key: DimensionKey,
    start: Point,
    moved: bool,
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
    /// Offset tool prompt: signed distance for a new offset.
    OffsetCreate,
    /// Driving distance of the selected offset shape.
    OffsetDistance,
    /// Inline edit of a dimension label in the viewport.
    Label(DimensionKey),
    /// Sketch variable row (`None`: add a variable).
    Variable(Option<usize>),
    /// One step of the linear/circular pattern prompt.
    PatternValue,
    /// Revolve angle or edge finish size of the selected feature.
    FeatureValue,
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
            rollback_drag: false,
            solid_view: OrbitView::default(),
            hovered_face: None,
            face_context: true,
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
            offset_value: 5.0,
            pattern_prompt: None,
            pattern_values: PatternValues::default(),
            analysis: Default::default(),
            selected_dimension: None,
            label_drag: None,
            dimension_start: None,
            extrude_draft: None,
            solid_undo: Vec::new(),
            solid_redo: Vec::new(),
            body_pick: false,
            hover: Hover::default(),
            show_glyphs: true,
            auto_constraints: true,
            redraw_requests: Default::default(),
            palette: None,
            shortcut_overlay: false,
            settings: Settings::default(),
            settings_open: false,
            ui_scale: 1.0,
            panel_scroll: 0.0,
            panel_followed: (false, 0, 0),
            monitor_scale: 1.0,
            status_cache: Default::default(),
            tip_hover: None,
            revolve_draft: None,
            loft_start: None,
            edge_pick: None,
            history_open: false,
            assembly: None,
        }
    }
}

impl App {
    fn switch_part(&mut self, direction: isize) {
        self.close_assembly();
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
                self.document.reference_edges = if self.face_context {
                    project.sketch_support_edges(project.active_sketch())
                } else {
                    Vec::new()
                };
                self.selected_solid = 0;
                self.selected_profile = 0;
                self.solid_preview = project.extrusion_previews().is_some();
                self.editing = None;
                self.status_message = format!("Part: {}  |  Ctrl+S to save", project.part_name());
            }
            Err(error) => self.status_message = format!("Switch part: {error}"),
        }
    }

    fn export_stl(&mut self, format: stl::StlFormat) {
        if self.assembly.is_some() {
            self.export_assembly_stl(format);
            return;
        }
        self.status_message = match &self.project {
            Some(project) => match stl::project_bodies(project).and_then(|bodies| {
                stl::write_stl_bodies(project.path(), project.part_name(), &bodies, format)
            }) {
                Ok(paths) => format!(
                    "Exported {} {} STL {}",
                    paths.len(),
                    if format == stl::StlFormat::Ascii {
                        "ASCII"
                    } else {
                        "binary"
                    },
                    if paths.len() == 1 { "body" } else { "bodies" }
                ),
                Err(reason) => reason,
            },
            None => "Open a .pcad project to export STL".into(),
        };
    }

    fn export_3mf(&mut self) {
        if self.assembly.is_some() {
            self.export_assembly_3mf();
            return;
        }
        self.status_message = match &self.project {
            Some(project) => match stl::project_bodies(project).and_then(|bodies| {
                threemf::write_3mf(project.path(), project.part_name(), &bodies)
                    .map(|path| (path, bodies.len()))
            }) {
                Ok((path, count)) => format!(
                    "Exported {count} {} to {}",
                    if count == 1 { "body" } else { "bodies" },
                    path.file_name()
                        .map_or_else(String::new, |name| name.to_string_lossy().into())
                ),
                Err(reason) => format!("3MF export failed: {reason}"),
            },
            None => "Open a .pcad project to export 3MF".into(),
        };
    }

    fn select_tool(&mut self, tool: Tool) {
        self.document.finish_drag();
        self.split_circle_start = None;
        self.dimension_start = None;
        if self.document.tool != tool {
            self.document.draft_start = None;
            self.document.draft_end = None;
            self.document.tangent_source = None;
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

    /// Offset tool (Shift+U or MORE > OFFSET): prompt for the signed
    /// distance, then offset the selection (a line chain when the selected
    /// line joins others end to end).
    fn begin_offset(&mut self) {
        self.more_open = false;
        if self.document.selected.is_none() {
            self.status_message = "Select a line, circle or arc to offset first".into();
            return;
        }
        self.document.finish_drag();
        self.editing = Some(DimensionEdit {
            field: EditField::OffsetCreate,
            buffer: String::new(),
        });
        self.status_message = format!(
            "Offset distance in mm, sign picks the side (Enter keeps {:.1})  |  Esc to cancel",
            self.offset_value
        );
    }

    /// Finish the Offset prompt; returns the status message.
    fn finish_offset_create(&mut self, buffer: &str) -> String {
        let distance = if buffer.is_empty() {
            self.offset_value
        } else {
            match buffer.parse::<f32>() {
                Ok(value) if value.is_finite() && value != 0.0 => value,
                _ => return "Offset: enter a non-zero distance".into(),
            }
        };
        self.offset_value = distance;
        match self.document.offset_selected(distance) {
            Ok(created) => format!(
                "Offset {} shape{} by {distance:.1} mm  |  Ctrl+Z to undo  |  Ctrl+S to save",
                created.len(),
                if created.len() == 1 { "" } else { "s" }
            ),
            Err(error) => format!("Offset: {error}"),
        }
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

    /// Project panel rows (parts and assemblies), shown when there is more
    /// than one.
    fn part_rows(&self) -> Vec<usize> {
        Some(self.project_rows())
            .filter(|rows| rows.len() > 1)
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
        let mut app = Self {
            document,
            project: Some(project),
            status_message,
            solid_preview,
            ..Self::default()
        };
        app.refresh_reference_edges();
        Ok(app)
    }

    /// Loads the active face sketch's supporting-body edges into the document
    /// (drawn behind the sketch and used as snap targets) unless key 5 hid them.
    fn refresh_reference_edges(&mut self) {
        self.document.reference_edges = match &self.project {
            Some(project) if self.face_context => {
                project.sketch_support_edges(project.active_sketch())
            }
            _ => Vec::new(),
        };
    }

    fn toggle_glyphs(&mut self) {
        self.show_glyphs = !self.show_glyphs;
        self.update_hover();
        self.status_message = format!(
            "Relation glyphs {}",
            if self.show_glyphs { "shown" } else { "hidden" }
        );
    }

    fn toggle_auto_constraints(&mut self) {
        self.auto_constraints = !self.auto_constraints;
        self.status_message = format!(
            "Automatic constraints {}",
            if self.auto_constraints { "on" } else { "off" }
        );
    }

    /// Inference hint for the segment being placed at the cursor, if any.
    fn placing_inference(&self) -> Option<model::Inference> {
        let start = self.document.draft_start?;
        let segment_phase = match self.document.tool {
            Tool::Line => true,
            Tool::ThreePointRectangle
            | Tool::ThreePointCenterRectangle
            | Tool::Arc
            | Tool::Slot => self.document.draft_end.is_none(),
            _ => false,
        };
        if !segment_phase
            || !self
                .layout()
                .viewport
                .contains(self.cursor.x, self.cursor.y)
        {
            return None;
        }
        let raw = self
            .document
            .screen_to_world(self.cursor, self.viewport_origin());
        let end = self
            .document
            .snap_point(raw, self.snap_grid, self.snap_geometry)
            .map_or(raw, |snap| snap.point);
        self.document.infer_segment(start, end)
    }

    fn toggle_face_context(&mut self) {
        let on_face = self
            .project
            .as_ref()
            .is_some_and(|project| project.sketch_face(project.active_sketch()).is_some());
        if !on_face || self.solid_preview {
            self.status_message = "Body context: open a face sketch first".into();
            return;
        }
        self.face_context = !self.face_context;
        self.refresh_reference_edges();
        self.status_message = format!(
            "Supporting body context {}",
            if self.face_context { "shown" } else { "hidden" }
        );
    }

    fn read_only(&self) -> bool {
        self.project.as_ref().is_some_and(|project| {
            project.read_only_reason().is_some() || project.active_sketch_has_extrusion()
        })
    }

    fn layout(&self) -> Layout {
        // Headless (tests): the default window size.
        let base = self
            .window
            .as_ref()
            .map_or(Layout::new(1280.0, 800.0), |window| {
                let size = window.inner_size();
                Layout::scaled(size.width as f32, size.height as f32, self.ui_scale)
            });
        Layout {
            panel_scroll: self.panel_scroll.clamp(0.0, self.max_panel_scroll(base)),
            ..base
        }
    }

    fn redraw(&self) {
        self.redraw_requests.set(self.redraw_requests.get() + 1);
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
        if (!self.solid_preview && self.assembly.is_none()) || self.projects_page.is_some() {
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
        painter.set_scale(layout.scale);
        painter.set_remap(self.settings.theme.palette().remap());
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
        if self.assembly.is_some() {
            self.paint_assembly(&mut painter, layout);
            self.paint_assembly_panels(&mut painter, layout);
            self.paint_more_menu(&mut painter, layout);
            self.paint_settings(&mut painter, layout);
            self.paint_palette(&mut painter, layout);
            self.paint_shortcut_overlay(&mut painter, layout);
            self.paint_tooltip(&mut painter, layout);
            return painter;
        }
        if self.extrude_draft.is_some() {
            self.paint_extrude_draft(&mut painter, layout);
        } else if self
            .revolve_draft
            .as_ref()
            .is_some_and(|draft| draft.axis.is_some())
        {
            self.paint_revolve_draft(&mut painter, layout);
        } else if self.solid_preview {
            if self
                .project
                .as_ref()
                .and_then(ProjectFile::active_extrusion_previews)
                .is_some()
                && let Some((scene, projector)) = self.solid_projector(layout)
            {
                self.paint_boolean_preview(&mut painter, layout, &scene, &projector, scene.bodies);
                if self.edge_pick.is_some() {
                    self.paint_edge_pick(&mut painter, layout, &scene, &projector);
                } else {
                    self.paint_face_hover(&mut painter, layout, &projector);
                }
            }
        } else {
            self.paint_grid(&mut painter, layout);
            self.paint_reference_edges(&mut painter, layout);
            self.paint_shapes(&mut painter, layout);
            self.paint_profile_choice(&mut painter, layout);
            self.paint_revolve_axis_prompt(&mut painter, layout);
        }
        self.paint_panels(&mut painter, layout);
        self.paint_history_panel(&mut painter, layout);
        self.paint_more_menu(&mut painter, layout);
        self.paint_settings(&mut painter, layout);
        self.paint_palette(&mut painter, layout);
        self.paint_shortcut_overlay(&mut painter, layout);
        self.paint_tooltip(&mut painter, layout);
        painter
    }

    fn paint_more_menu(&self, painter: &mut Painter, layout: Layout) {
        if !self.more_open {
            return;
        }
        for (index, (_, label)) in MORE_MENU.iter().enumerate() {
            let row = layout.more_item(index);
            painter.rect(row, PANEL_LIGHT);
            painter.outline(row, 1.0, ACCENT);
            painter.text(row.x + 6.0, row.y + 9.0, label, 1.0, TEXT);
        }
    }
}
