//! Command registry: every user command with its label, shortcut text,
//! category, availability and the `App` action it runs. Keyboard shortcuts,
//! the MORE menu, the command palette and the shortcut overlay all go
//! through this table, so the cheat sheet cannot drift from the bindings.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(super) enum Category {
    Draw,
    Edit,
    Constrain,
    Dimension,
    Solid,
    View,
    Project,
    Assembly,
    File,
}

impl Category {
    pub(super) const ALL: [Category; 9] = [
        Category::Draw,
        Category::Edit,
        Category::Constrain,
        Category::Dimension,
        Category::Solid,
        Category::View,
        Category::Project,
        Category::Assembly,
        Category::File,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Category::Draw => "Draw",
            Category::Edit => "Edit",
            Category::Constrain => "Constrain",
            Category::Dimension => "Dimension",
            Category::Solid => "Solid",
            Category::View => "View",
            Category::Project => "Project",
            Category::Assembly => "Assembly",
            Category::File => "File",
        }
    }
}

/// Where a command (and its shortcut) applies. `Sketch` and `Solid` are the
/// two views; the remaining scopes are modal contexts inside the sketch view
/// whose bindings take precedence over the plain sketch bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scope {
    Any,
    Sketch,
    Solid,
    /// The center-arc tool is active.
    CenterArcTool,
    /// A fit or control spline is being placed.
    SplineTool,
    /// A dimension label is selected.
    DimensionSelected,
    /// The 3D preview waits for an up-to-body pick (U).
    BodyPick,
    /// The assembly view is open; its bindings replace the view-wide ones.
    Assembly,
}

impl Scope {
    /// Whether one key may not be bound twice across these scopes.
    #[cfg(test)]
    pub(super) fn overlaps(self, other: Scope) -> bool {
        match (self, other) {
            (a, b) if a == b => true,
            // Modal scopes take precedence over the view-wide bindings.
            (a, b) if a.is_modal() || b.is_modal() => false,
            (Scope::Any, _) | (_, Scope::Any) => true,
            _ => false,
        }
    }

    #[cfg(test)]
    fn is_modal(self) -> bool {
        matches!(
            self,
            Scope::CenterArcTool
                | Scope::SplineTool
                | Scope::DimensionSelected
                | Scope::BodyPick
                | Scope::Assembly
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Cmd {
    Select,
    Line,
    Rectangle,
    CenterRectangle,
    ThreePointRectangle,
    ThreePointCenterRectangle,
    Circle,
    ThreePointCircle,
    Ellipse,
    Arc,
    CenterArc,
    ArcDirection,
    TangentArc,
    Point,
    Polygon,
    Slot,
    FitSpline,
    ControlSpline,
    FinishSpline,
    FinishClosedSpline,
    Split,
    Trim,
    Extend,
    Mirror,
    Offset,
    FilletCorner,
    ChamferCorner,
    FilletPick,
    ChamferPick,
    LinearPattern,
    CircularPattern,
    CopySelected,
    Delete,
    Construction,
    Undo,
    Redo,
    Cancel,
    Horizontal,
    Vertical,
    Symmetry,
    PointOnCurve,
    Tangent,
    InternalTangent,
    Concentric,
    Equal,
    Parallel,
    Perpendicular,
    Collinear,
    Midpoint,
    Normal,
    FixedAnchor,
    SmoothJoin,
    DimensionTool,
    ToggleReference,
    LineAngle,
    RadialDistance,
    ExtrudeNew,
    ExtrudeJoin,
    ExtrudeCut,
    SketchOnFace,
    FaceContext,
    TogglePreview,
    SolidEndCondition,
    SolidFlip,
    BodyPick,
    CancelBodyPick,
    ToggleGlyphs,
    ToggleAutoConstraints,
    PreviousProfile,
    NextProfile,
    PreviousSolid,
    NextSolid,
    FitView,
    GridSnap,
    GeometrySnap,
    MoreMenu,
    CommandPalette,
    ShortcutOverlay,
    HistoryPanel,
    NewPart,
    SketchTop,
    SketchFront,
    SketchRight,
    NextPart,
    PreviousPart,
    RenamePart,
    RenameSketch,
    Save,
    ExportStl,
    ExportAsciiStl,
    Export3mf,
    Settings,
    FeatureUp,
    FeatureDown,
    SuppressFeature,
    RollbackUp,
    RollbackDown,
    Revolve,
    Loft,
    LoftJoin,
    EdgeChamfer,
    EdgeFillet,
    AssemblyView,
    InsertInstance,
    DeleteInstance,
    MateInstances,
    AssemblyUndo,
    AssemblyRedo,
    AssemblyCancel,
    Measure,
    DeleteFeature,
    FitSolid,
    ToggleProjection,
    ArmMate,
    FastenInstance,
}

pub(super) struct Command {
    pub(super) id: Cmd,
    pub(super) label: &'static str,
    /// Display text; alternatives are separated by " / ". Empty: none.
    pub(super) shortcut: &'static str,
    pub(super) category: Category,
    pub(super) scope: Scope,
    /// Changes the sketch, so it is unavailable in read-only sketches.
    pub(super) edits: bool,
    /// Needs a selected shape or dimension.
    pub(super) needs_selection: bool,
    pub(super) run: fn(&mut App),
}

const fn cmd(
    id: Cmd,
    label: &'static str,
    shortcut: &'static str,
    category: Category,
    scope: Scope,
    run: fn(&mut App),
) -> Command {
    Command {
        id,
        label,
        shortcut,
        category,
        scope,
        edits: false,
        needs_selection: false,
        run,
    }
}

/// Sketch edit: needs an editable sketch.
const fn edit(
    id: Cmd,
    label: &'static str,
    shortcut: &'static str,
    category: Category,
    needs_selection: bool,
    run: fn(&mut App),
) -> Command {
    Command {
        id,
        label,
        shortcut,
        category,
        scope: Scope::Sketch,
        edits: true,
        needs_selection,
        run,
    }
}

use Category as C;
use Scope as S;

pub(super) static COMMANDS: &[Command] = &[
    // Draw
    cmd(Cmd::Select, "Select", "V", C::Draw, S::Sketch, |app| {
        app.select_tool(Tool::Select)
    }),
    cmd(Cmd::Line, "Line", "L", C::Draw, S::Sketch, |app| {
        app.select_tool(Tool::Line)
    }),
    cmd(
        Cmd::Rectangle,
        "Rectangle",
        "R",
        C::Draw,
        S::Sketch,
        |app| app.select_tool(Tool::Rectangle),
    ),
    cmd(
        Cmd::CenterRectangle,
        "Center rectangle",
        "Shift+R",
        C::Draw,
        S::Sketch,
        |app| app.select_tool(Tool::CenterRectangle),
    ),
    cmd(
        Cmd::ThreePointRectangle,
        "3-point rectangle",
        "T",
        C::Draw,
        S::Sketch,
        |app| app.select_tool(Tool::ThreePointRectangle),
    ),
    cmd(
        Cmd::ThreePointCenterRectangle,
        "3-point center rectangle",
        "Shift+T",
        C::Draw,
        S::Sketch,
        |app| app.select_tool(Tool::ThreePointCenterRectangle),
    ),
    cmd(Cmd::Circle, "Circle", "C", C::Draw, S::Sketch, |app| {
        app.select_tool(Tool::Circle)
    }),
    cmd(
        Cmd::ThreePointCircle,
        "3-point circle",
        "Shift+C",
        C::Draw,
        S::Sketch,
        |app| app.select_tool(Tool::ThreePointCircle),
    ),
    cmd(Cmd::Ellipse, "Ellipse", "E", C::Draw, S::Sketch, |app| {
        app.select_tool(Tool::Ellipse)
    }),
    cmd(Cmd::Arc, "Arc (3-point)", "A", C::Draw, S::Sketch, |app| {
        app.select_tool(Tool::Arc)
    }),
    cmd(
        Cmd::CenterArc,
        "Center arc",
        "Shift+A",
        C::Draw,
        S::Sketch,
        |app| app.select_tool(Tool::CenterArc),
    ),
    cmd(
        Cmd::ArcDirection,
        "Flip center arc direction",
        "W",
        C::Draw,
        S::CenterArcTool,
        App::flip_arc_direction,
    ),
    cmd(
        Cmd::TangentArc,
        "Tangent arc",
        ",",
        C::Draw,
        S::Sketch,
        App::start_tangent_arc,
    ),
    cmd(Cmd::Point, "Point", "P", C::Draw, S::Sketch, |app| {
        app.select_tool(Tool::Point)
    }),
    cmd(Cmd::Polygon, "Polygon", "G", C::Draw, S::Sketch, |app| {
        app.select_tool(Tool::Polygon)
    }),
    cmd(Cmd::Slot, "Slot", "S", C::Draw, S::Sketch, |app| {
        app.select_tool(Tool::Slot)
    }),
    cmd(
        Cmd::FitSpline,
        "Fit spline",
        "",
        C::Draw,
        S::Sketch,
        |app| app.select_tool(Tool::FitSpline),
    ),
    cmd(
        Cmd::ControlSpline,
        "Control spline",
        "",
        C::Draw,
        S::Sketch,
        |app| app.select_tool(Tool::ControlSpline),
    ),
    cmd(
        Cmd::FinishSpline,
        "Finish spline",
        "Enter",
        C::Draw,
        S::SplineTool,
        |app| app.finish_spline(false),
    ),
    cmd(
        Cmd::FinishClosedSpline,
        "Finish closed spline",
        "Shift+Enter",
        C::Draw,
        S::SplineTool,
        |app| app.finish_spline(true),
    ),
    // Edit
    edit(
        Cmd::Split,
        "Split curve at cursor",
        "I",
        C::Edit,
        true,
        App::split_key,
    ),
    edit(
        Cmd::Trim,
        "Trim curve at cursor",
        "Q",
        C::Edit,
        true,
        App::trim_key,
    ),
    edit(
        Cmd::Extend,
        "Extend curve to cursor",
        "U",
        C::Edit,
        true,
        App::extend_key,
    ),
    edit(Cmd::Mirror, "Mirror", "Shift+I", C::Edit, true, |app| {
        app.guard_read_only(App::begin_mirror)
    }),
    edit(Cmd::Offset, "Offset", "Shift+U", C::Edit, true, |app| {
        app.guard_read_only(App::begin_offset)
    }),
    edit(
        Cmd::FilletCorner,
        "Fillet selected + hovered line",
        "Shift+W",
        C::Edit,
        true,
        |app| app.corner_key(CornerKind::Fillet),
    ),
    edit(
        Cmd::ChamferCorner,
        "Chamfer selected + hovered line",
        "Shift+H",
        C::Edit,
        true,
        |app| app.corner_key(CornerKind::Chamfer),
    ),
    edit(
        Cmd::FilletPick,
        "Fillet (pick two lines)",
        "",
        C::Edit,
        false,
        |app| app.guard_read_only(|app| app.start_corner_pick(CornerKind::Fillet)),
    ),
    edit(
        Cmd::ChamferPick,
        "Chamfer (pick two lines)",
        "",
        C::Edit,
        false,
        |app| app.guard_read_only(|app| app.start_corner_pick(CornerKind::Chamfer)),
    ),
    Command {
        needs_selection: true,
        ..cmd(
            Cmd::LinearPattern,
            "Linear pattern",
            "Shift+Z",
            C::Edit,
            S::Sketch,
            |app| app.begin_pattern(PatternTool::Linear),
        )
    },
    Command {
        needs_selection: true,
        ..cmd(
            Cmd::CircularPattern,
            "Circular pattern",
            ";",
            C::Edit,
            S::Sketch,
            |app| app.begin_pattern(PatternTool::Circular),
        )
    },
    Command {
        edits: true,
        needs_selection: true,
        ..cmd(
            Cmd::CopySelected,
            "Copy selected",
            "Ctrl+D",
            C::Edit,
            S::Any,
            App::copy_selected_key,
        )
    },
    cmd(
        Cmd::Delete,
        "Delete selection / extrusion",
        "Delete / Backspace",
        C::Edit,
        S::Any,
        App::delete_key,
    ),
    Command {
        edits: true,
        needs_selection: true,
        ..cmd(
            Cmd::Construction,
            "Toggle construction",
            "K",
            C::Edit,
            S::Any,
            App::toggle_construction_key,
        )
    },
    // Undo / redo walk one history across both views (`history.rs`).
    cmd(Cmd::Undo, "Undo", "Ctrl+Z", C::Edit, S::Any, |app| {
        app.undo_any();
    }),
    cmd(
        Cmd::Redo,
        "Redo",
        "Ctrl+Y / Ctrl+Shift+Z",
        C::Edit,
        S::Any,
        |app| {
            app.redo_any();
        },
    ),
    cmd(
        Cmd::Cancel,
        "Cancel / clear selection",
        "Esc",
        C::Edit,
        S::Any,
        App::cancel_key,
    ),
    // Constrain
    edit(
        Cmd::Horizontal,
        "Horizontal",
        "H",
        C::Constrain,
        true,
        |app| app.axis_key(AxisConstraint::Horizontal),
    ),
    edit(Cmd::Vertical, "Vertical", "J", C::Constrain, true, |app| {
        app.axis_key(AxisConstraint::Vertical)
    }),
    edit(
        Cmd::Symmetry,
        "Symmetric about selected line",
        "Shift+S",
        C::Constrain,
        true,
        App::begin_symmetry,
    ),
    edit(
        Cmd::PointOnCurve,
        "Point on hovered curve",
        "Shift+V",
        C::Constrain,
        true,
        App::point_on_curve_key,
    ),
    edit(
        Cmd::Tangent,
        "Tangent to hovered curve",
        "Shift+Q",
        C::Constrain,
        true,
        App::tangent_key,
    ),
    edit(
        Cmd::InternalTangent,
        "Internal tangent",
        "Shift+Y",
        C::Constrain,
        true,
        App::internal_tangent_key,
    ),
    edit(
        Cmd::Concentric,
        "Concentric",
        "Shift+O",
        C::Constrain,
        true,
        App::concentric_key,
    ),
    edit(Cmd::Equal, "Equal", "Shift+E", C::Constrain, true, |app| {
        app.equal_key()
    }),
    edit(
        Cmd::Parallel,
        "Parallel",
        "Shift+P",
        C::Constrain,
        true,
        |app| app.line_relation_key(Some(LineRelationKind::Parallel)),
    ),
    edit(
        Cmd::Perpendicular,
        "Perpendicular",
        "Shift+N",
        C::Constrain,
        true,
        |app| app.line_relation_key(Some(LineRelationKind::Perpendicular)),
    ),
    edit(
        Cmd::Collinear,
        "Collinear",
        "Shift+L",
        C::Constrain,
        true,
        |app| app.line_relation_key(Some(LineRelationKind::Collinear)),
    ),
    edit(
        Cmd::Midpoint,
        "Midpoint of hovered line/arc",
        "Shift+M",
        C::Constrain,
        true,
        App::midpoint_key,
    ),
    edit(
        Cmd::Normal,
        "Normal to hovered curve",
        "Shift+F",
        C::Constrain,
        true,
        App::normal_key,
    ),
    Command {
        edits: true,
        needs_selection: true,
        ..cmd(
            Cmd::FixedAnchor,
            "Fix anchor at cursor",
            "Shift+K",
            C::Constrain,
            S::Any,
            App::fixed_anchor_key,
        )
    },
    edit(
        Cmd::SmoothJoin,
        "Smooth join (G1)",
        ".",
        C::Constrain,
        true,
        App::smooth_join_key,
    ),
    // Dimension
    cmd(
        Cmd::DimensionTool,
        "Dimension tool",
        "D",
        C::Dimension,
        S::Sketch,
        |app| app.select_tool(Tool::Dimension),
    ),
    Command {
        needs_selection: true,
        ..cmd(
            Cmd::ToggleReference,
            "Toggle driving / reference",
            "R",
            C::Dimension,
            S::DimensionSelected,
            App::toggle_selected_reference,
        )
    },
    edit(
        Cmd::LineAngle,
        "Angle to hovered line",
        "Shift+B",
        C::Dimension,
        true,
        |app| app.line_relation_key(None),
    ),
    edit(
        Cmd::RadialDistance,
        "Radial distance to hovered circle",
        "Shift+D",
        C::Dimension,
        true,
        App::radial_distance_key,
    ),
    // Solid
    cmd(
        Cmd::ExtrudeNew,
        "Extrude new body",
        "X",
        C::Solid,
        S::Any,
        |app| app.begin_extrude("newBody"),
    ),
    cmd(
        Cmd::ExtrudeJoin,
        "Extrude join",
        "Shift+J",
        C::Solid,
        S::Any,
        |app| app.begin_extrude("join"),
    ),
    cmd(
        Cmd::ExtrudeCut,
        "Extrude cut",
        "Shift+X",
        C::Solid,
        S::Any,
        |app| app.begin_extrude("cut"),
    ),
    cmd(
        Cmd::SketchOnFace,
        "Sketch on hovered face",
        "4",
        C::Solid,
        S::Any,
        App::sketch_on_face,
    ),
    cmd(
        Cmd::FaceContext,
        "Toggle face body context",
        "5",
        C::Solid,
        S::Any,
        App::toggle_face_context,
    ),
    cmd(
        Cmd::SolidEndCondition,
        "Cycle extrusion end condition",
        "E",
        C::Solid,
        S::Solid,
        App::cycle_solid_end_condition,
    ),
    cmd(
        Cmd::SolidFlip,
        "Flip extrusion direction",
        "F",
        C::Solid,
        S::Solid,
        App::toggle_solid_flip,
    ),
    cmd(
        Cmd::BodyPick,
        "Pick up-to body",
        "U",
        C::Solid,
        S::Solid,
        App::arm_body_pick,
    ),
    cmd(
        Cmd::CancelBodyPick,
        "Cancel body pick",
        "Esc",
        C::Solid,
        S::BodyPick,
        |app| {
            app.body_pick = false;
            app.status_message = "Body pick cancelled".into();
        },
    ),
    cmd(
        Cmd::TogglePreview,
        "Toggle 3D preview",
        "Tab",
        C::Solid,
        S::Any,
        App::toggle_solid_preview,
    ),
    cmd(
        Cmd::PreviousProfile,
        "Previous profile",
        "[",
        C::Solid,
        S::Sketch,
        |app| app.cycle_profile(-1),
    ),
    cmd(
        Cmd::NextProfile,
        "Next profile",
        "]",
        C::Solid,
        S::Sketch,
        |app| app.cycle_profile(1),
    ),
    cmd(
        Cmd::PreviousSolid,
        "Previous solid",
        "[",
        C::Solid,
        S::Solid,
        |app| app.cycle_solid(-1),
    ),
    cmd(
        Cmd::NextSolid,
        "Next solid",
        "]",
        C::Solid,
        S::Solid,
        |app| app.cycle_solid(1),
    ),
    // View
    cmd(
        Cmd::ToggleGlyphs,
        "Toggle relation glyphs",
        "6",
        C::View,
        S::Any,
        App::toggle_glyphs,
    ),
    cmd(
        Cmd::ToggleAutoConstraints,
        "Toggle automatic constraints",
        "7",
        C::View,
        S::Any,
        App::toggle_auto_constraints,
    ),
    cmd(
        Cmd::FitView,
        "Fit sketch to view",
        "F",
        C::View,
        S::Sketch,
        App::fit_view_key,
    ),
    cmd(
        Cmd::GridSnap,
        "Toggle grid snapping",
        "Shift+G",
        C::View,
        S::Any,
        App::toggle_grid_snap,
    ),
    cmd(
        Cmd::GeometrySnap,
        "Toggle geometry snapping",
        "N",
        C::View,
        S::Any,
        App::toggle_geometry_snap,
    ),
    cmd(Cmd::MoreMenu, "MORE menu", "M", C::View, S::Any, |app| {
        app.more_open = !app.more_open
    }),
    cmd(
        Cmd::CommandPalette,
        "Command palette",
        "Ctrl+K / Ctrl+Shift+P",
        C::View,
        S::Any,
        App::open_palette,
    ),
    cmd(
        Cmd::ShortcutOverlay,
        "Keyboard shortcuts",
        "? / F1",
        C::View,
        S::Any,
        App::toggle_shortcut_overlay,
    ),
    cmd(
        Cmd::HistoryPanel,
        "Undo history panel",
        "Ctrl+Shift+H",
        C::View,
        S::Any,
        App::toggle_history_panel,
    ),
    // Project
    cmd(Cmd::NewPart, "New part", "B", C::Project, S::Any, |app| {
        app.create_part()
    }),
    cmd(
        Cmd::SketchTop,
        "New sketch on TOP",
        "1",
        C::Project,
        S::Any,
        |app| app.create_sketch(SketchPlane::Top),
    ),
    cmd(
        Cmd::SketchFront,
        "New sketch on FRONT",
        "2",
        C::Project,
        S::Any,
        |app| app.create_sketch(SketchPlane::Front),
    ),
    cmd(
        Cmd::SketchRight,
        "New sketch on RIGHT",
        "3",
        C::Project,
        S::Any,
        |app| app.create_sketch(SketchPlane::Right),
    ),
    cmd(
        Cmd::NextPart,
        "Next part",
        "Ctrl+PageDown",
        C::Project,
        S::Any,
        |app| app.switch_part(1),
    ),
    cmd(
        Cmd::PreviousPart,
        "Previous part",
        "Ctrl+PageUp",
        C::Project,
        S::Any,
        |app| app.switch_part(-1),
    ),
    cmd(
        Cmd::RenamePart,
        "Rename part",
        "F2",
        C::Project,
        S::Any,
        |app| app.begin_rename(NameTarget::Part),
    ),
    cmd(
        Cmd::RenameSketch,
        "Rename sketch",
        "Shift+F2",
        C::Project,
        S::Any,
        |app| app.begin_rename(NameTarget::Sketch),
    ),
    // File
    cmd(
        Cmd::Save,
        "Save project",
        "Ctrl+S",
        C::File,
        S::Any,
        |app| app.save_project(),
    ),
    cmd(
        Cmd::ExportStl,
        "Export binary STL",
        "Ctrl+E",
        C::File,
        S::Any,
        |app| app.export_stl(stl::StlFormat::Binary),
    ),
    cmd(
        Cmd::ExportAsciiStl,
        "Export ASCII STL",
        "Ctrl+Shift+E",
        C::File,
        S::Any,
        |app| app.export_stl(stl::StlFormat::Ascii),
    ),
    cmd(
        Cmd::Export3mf,
        "Export 3MF",
        "Ctrl+Alt+E",
        C::File,
        S::Any,
        |app| app.export_3mf(),
    ),
    cmd(
        Cmd::FeatureUp,
        "Move feature up",
        "Ctrl+Up",
        C::Solid,
        S::Solid,
        |app| app.move_selected_feature(-1),
    ),
    cmd(
        Cmd::FeatureDown,
        "Move feature down",
        "Ctrl+Down",
        C::Solid,
        S::Solid,
        |app| app.move_selected_feature(1),
    ),
    cmd(
        Cmd::SuppressFeature,
        "Suppress / unsuppress feature",
        "Ctrl+H",
        C::Solid,
        S::Solid,
        |app| app.toggle_selected_suppressed(),
    ),
    cmd(
        Cmd::RollbackUp,
        "Roll back one feature",
        "Ctrl+[",
        C::Solid,
        S::Solid,
        |app| app.move_rollback(-1),
    ),
    cmd(
        Cmd::RollbackDown,
        "Roll forward one feature",
        "Ctrl+]",
        C::Solid,
        S::Solid,
        |app| app.move_rollback(1),
    ),
    cmd(
        Cmd::Revolve,
        "Revolve profile",
        "8",
        C::Solid,
        S::Sketch,
        App::begin_revolve,
    ),
    cmd(
        Cmd::Loft,
        "Loft (new body)",
        "9",
        C::Solid,
        S::Sketch,
        |app| app.loft_step(false),
    ),
    cmd(
        Cmd::LoftJoin,
        "Loft (join)",
        "Shift+9",
        C::Solid,
        S::Sketch,
        |app| app.loft_step(true),
    ),
    cmd(
        Cmd::EdgeChamfer,
        "Edge chamfer",
        "0",
        C::Solid,
        S::Any,
        |app| app.begin_edge_finish(project::FinishKind::Chamfer),
    ),
    cmd(
        Cmd::EdgeFillet,
        "Edge fillet",
        "Shift+0",
        C::Solid,
        S::Any,
        |app| app.begin_edge_finish(project::FinishKind::Fillet),
    ),
    cmd(
        Cmd::Settings,
        "Settings",
        "Ctrl+,",
        C::View,
        S::Any,
        |app| app.open_settings(),
    ),
    // Assembly
    cmd(
        Cmd::AssemblyView,
        "Open / close assembly view",
        "Ctrl+B",
        C::Assembly,
        S::Any,
        App::toggle_assembly_view,
    ),
    cmd(
        Cmd::InsertInstance,
        "Insert part instance",
        "I",
        C::Assembly,
        S::Assembly,
        App::begin_insert_instance,
    ),
    cmd(
        Cmd::DeleteInstance,
        "Delete instance / selected mate",
        "Delete / Backspace",
        C::Assembly,
        S::Assembly,
        App::delete_selected_instance,
    ),
    cmd(
        Cmd::MateInstances,
        "Mate selected face/edge to hovered face/edge (empty space: fasten to world)",
        "J",
        C::Assembly,
        S::Assembly,
        App::begin_mate,
    ),
    cmd(
        Cmd::ArmMate,
        "Mate: hover a face/edge/vertex of another instance, then J",
        "",
        C::Assembly,
        S::Assembly,
        |app| {
            app.status_message =
                "Mate: hover a vertex, edge or face of another instance, then press J".into()
        },
    ),
    cmd(
        Cmd::FastenInstance,
        "Fasten selected instance to world",
        "",
        C::Assembly,
        S::Assembly,
        App::fasten_selected,
    ),
    cmd(
        Cmd::Measure,
        "Measure two selected elements",
        "",
        C::Solid,
        S::Any,
        App::measure_selection,
    ),
    cmd(
        Cmd::DeleteFeature,
        "Delete selected feature",
        "",
        C::Solid,
        S::Solid,
        App::delete_feature_command,
    ),
    cmd(
        Cmd::FitSolid,
        "Fit all (3D view)",
        "",
        C::View,
        S::Any,
        App::fit_solid,
    ),
    cmd(
        Cmd::ToggleProjection,
        "Toggle perspective / orthographic",
        "",
        C::View,
        S::Any,
        App::toggle_projection,
    ),
    cmd(
        Cmd::AssemblyUndo,
        "Undo assembly edit",
        "Ctrl+Z",
        C::Assembly,
        S::Assembly,
        |app| app.assembly_undo(false),
    ),
    cmd(
        Cmd::AssemblyRedo,
        "Redo assembly edit",
        "Ctrl+Y / Ctrl+Shift+Z",
        C::Assembly,
        S::Assembly,
        |app| app.assembly_undo(true),
    ),
    cmd(
        Cmd::AssemblyCancel,
        "Clear selection / leave assembly",
        "Escape",
        C::Assembly,
        S::Assembly,
        App::assembly_cancel,
    ),
];

/// MORE menu rows (index = `Layout::more_item` slot) and their labels.
pub(super) const MORE_MENU: [(Cmd, &str); 28] = [
    (Cmd::Point, "POINT [P]"),
    (Cmd::Polygon, "POLYGON [G]"),
    (Cmd::Slot, "SLOT [S]"),
    (Cmd::ExtrudeNew, "EXTRUDE [X]"),
    (Cmd::NewPart, "NEW PART [B]"),
    (Cmd::ThreePointCircle, "3PT CIRCLE"),
    (Cmd::CenterArc, "CENTER ARC"),
    (Cmd::CenterRectangle, "CENTER RECT"),
    (Cmd::ThreePointRectangle, "3PT RECT"),
    (Cmd::ThreePointCenterRectangle, "3PT CTR RECT"),
    (Cmd::ExtrudeJoin, "JOIN [SHIFT J]"),
    (Cmd::FitSpline, "FIT SPLINE"),
    (Cmd::ControlSpline, "CTRL SPLINE"),
    (Cmd::Mirror, "MIRROR [SHIFT I]"),
    (Cmd::FilletPick, "FILLET [SHIFT W]"),
    (Cmd::ChamferPick, "CHAMFER [SHIFT H]"),
    (Cmd::Offset, "OFFSET [SHIFT U]"),
    (Cmd::TangentArc, "TANGENT ARC [,]"),
    (Cmd::LinearPattern, "LIN PATTERN [SHIFT Z]"),
    (Cmd::CircularPattern, "CIRC PATTERN [;]"),
    (Cmd::SketchOnFace, "SKETCH ON FACE [4]"),
    (Cmd::Settings, "SETTINGS [CTRL ,]"),
    (Cmd::Revolve, "REVOLVE [8]"),
    (Cmd::Loft, "LOFT [9]"),
    (Cmd::EdgeChamfer, "EDGE CHAMFER [0]"),
    (Cmd::EdgeFillet, "EDGE FILLET [SHIFT 0]"),
    (Cmd::LoftJoin, "LOFT JOIN [SHIFT 9]"),
    (Cmd::AssemblyView, "ASSEMBLY [CTRL B]"),
];

/// Toolbar buttons 0..=9 (`Layout::tool_button` slot) and their commands.
pub(super) const TOOLBAR: [Cmd; 10] = [
    Cmd::Select,
    Cmd::Rectangle,
    Cmd::Circle,
    Cmd::Line,
    Cmd::Ellipse,
    Cmd::SketchTop,
    Cmd::SketchFront,
    Cmd::SketchRight,
    Cmd::Arc,
    Cmd::MoreMenu,
];

/// The registry as Markdown tables, one per category (`--print-shortcuts`).
pub(crate) fn shortcuts_markdown() -> String {
    let mut out = String::new();
    for category in Category::ALL {
        out.push_str(&format!(
            "### {}\n\n| Shortcut | Command | When |\n| --- | --- | --- |\n",
            category.name()
        ));
        for command in COMMANDS
            .iter()
            .filter(|command| command.category == category)
        {
            let keys = if command.shortcut.is_empty() {
                "palette / MORE".to_string()
            } else {
                command
                    .shortcut
                    .split(" / ")
                    .map(|chord| format!("`{chord}`"))
                    .collect::<Vec<_>>()
                    .join(" / ")
            };
            let mut when = match command.scope {
                Scope::Any => "any view",
                Scope::Sketch => "sketch",
                Scope::Solid => "3D preview",
                Scope::CenterArcTool => "center-arc tool",
                Scope::SplineTool => "placing a spline",
                Scope::DimensionSelected => "dimension selected",
                Scope::BodyPick => "picking an up-to body",
                Scope::Assembly => "assembly view",
            }
            .to_string();
            if command.edits {
                when.push_str(", editable");
            }
            if command.needs_selection {
                when.push_str(", selection");
            }
            out.push_str(&format!("| {keys} | {} | {when} |\n", command.label));
        }
        out.push('\n');
    }
    out
}

pub(super) fn command(id: Cmd) -> &'static Command {
    COMMANDS
        .iter()
        .find(|command| command.id == id)
        .expect("every Cmd is registered")
}

#[cfg(test)]
/// Normalized key chords of a shortcut text ("Ctrl+Y / Ctrl+Shift+Z").
pub(super) fn shortcut_chords(text: &str) -> Vec<String> {
    text.split(" / ")
        .map(str::trim)
        .filter(|chord| !chord.is_empty())
        .map(|chord| {
            // "?" is Shift+/ on the keyboard.
            let chord = if chord == "?" { "Shift+/" } else { chord };
            let mut parts: Vec<String> = chord.split('+').map(str::to_lowercase).collect();
            let key = parts.pop().unwrap_or_default();
            parts.sort();
            parts.push(key);
            parts.join("+")
        })
        .collect()
}

impl App {
    pub(super) fn run_command(&mut self, id: Cmd) {
        (command(id).run)(self);
    }

    /// Whether a command can run in the current state (palette dimming).
    pub(super) fn command_available(&self, command: &Command) -> bool {
        if self.assembly.is_some() {
            return matches!(command.scope, Scope::Any | Scope::Assembly)
                && !matches!(
                    command.id,
                    Cmd::NewPart
                        | Cmd::SketchTop
                        | Cmd::SketchFront
                        | Cmd::SketchRight
                        | Cmd::RenamePart
                        | Cmd::RenameSketch
                        | Cmd::EdgeChamfer
                        | Cmd::EdgeFillet
                );
        }
        let scope = match command.scope {
            Scope::Any => true,
            Scope::Sketch => !self.solid_preview,
            Scope::Solid => self.solid_preview,
            Scope::CenterArcTool => !self.solid_preview && self.document.tool == Tool::CenterArc,
            Scope::SplineTool => {
                !self.solid_preview
                    && matches!(self.document.tool, Tool::FitSpline | Tool::ControlSpline)
            }
            Scope::DimensionSelected => !self.solid_preview && self.selected_dimension.is_some(),
            Scope::BodyPick => self.body_pick,
            Scope::Assembly => false,
        };
        let selection = !command.needs_selection
            || !self.document.selected_ids().is_empty()
            || self.selected_dimension.is_some();
        scope && selection && !(command.edits && self.read_only())
    }

    /// Key binding resolution: the guards encode precedence between
    /// overlapping bindings, the actions live in [`COMMANDS`].
    pub(super) fn command_for_key(&self, code: KeyCode) -> Option<Cmd> {
        let ctrl = self.modifiers.control_key() || self.modifiers.super_key();
        let shift = self.modifiers.shift_key();
        let plain_shift = shift && !ctrl;
        let read_only = self.read_only();
        let sketch_edit = !read_only && !self.solid_preview;
        if self.assembly.is_some() {
            // Only view-wide commands that make sense here, plus the
            // assembly bindings.
            return Some(match code {
                KeyCode::KeyK if ctrl => Cmd::CommandPalette,
                KeyCode::KeyP if ctrl && shift => Cmd::CommandPalette,
                KeyCode::Comma if ctrl => Cmd::Settings,
                KeyCode::Slash if plain_shift => Cmd::ShortcutOverlay,
                KeyCode::F1 => Cmd::ShortcutOverlay,
                KeyCode::KeyB if ctrl => Cmd::AssemblyView,
                KeyCode::KeyM if !ctrl => Cmd::MoreMenu,
                KeyCode::KeyI if !ctrl && !shift => Cmd::InsertInstance,
                KeyCode::KeyJ if !ctrl && !shift => Cmd::MateInstances,
                KeyCode::Delete | KeyCode::Backspace => Cmd::DeleteInstance,
                KeyCode::Escape => Cmd::AssemblyCancel,
                KeyCode::KeyS if ctrl => Cmd::Save,
                KeyCode::PageDown if ctrl => Cmd::NextPart,
                KeyCode::PageUp if ctrl => Cmd::PreviousPart,
                KeyCode::KeyE if ctrl && self.modifiers.alt_key() => Cmd::Export3mf,
                KeyCode::KeyE if ctrl && shift => Cmd::ExportAsciiStl,
                KeyCode::KeyE if ctrl => Cmd::ExportStl,
                KeyCode::KeyZ if ctrl && shift => Cmd::AssemblyRedo,
                KeyCode::KeyZ if ctrl => Cmd::AssemblyUndo,
                KeyCode::KeyY if ctrl => Cmd::AssemblyRedo,
                _ => return None,
            });
        }
        Some(match code {
            KeyCode::KeyB if ctrl => Cmd::AssemblyView,
            KeyCode::KeyK if ctrl => Cmd::CommandPalette,
            KeyCode::Comma if ctrl => Cmd::Settings,
            KeyCode::ArrowUp if ctrl && self.solid_preview => Cmd::FeatureUp,
            KeyCode::ArrowDown if ctrl && self.solid_preview => Cmd::FeatureDown,
            KeyCode::KeyH if ctrl && self.solid_preview => Cmd::SuppressFeature,
            KeyCode::BracketLeft if ctrl && self.solid_preview => Cmd::RollbackUp,
            KeyCode::BracketRight if ctrl && self.solid_preview => Cmd::RollbackDown,
            KeyCode::KeyP if ctrl && shift => Cmd::CommandPalette,
            KeyCode::Slash if plain_shift => Cmd::ShortcutOverlay,
            KeyCode::F1 => Cmd::ShortcutOverlay,
            KeyCode::KeyH if ctrl && shift => Cmd::HistoryPanel,
            KeyCode::Escape if self.body_pick => Cmd::CancelBodyPick,
            KeyCode::KeyE if self.solid_preview && !ctrl && !shift => Cmd::SolidEndCondition,
            KeyCode::KeyF if self.solid_preview && !ctrl && !shift => Cmd::SolidFlip,
            KeyCode::KeyU if self.solid_preview && !ctrl && !shift => Cmd::BodyPick,
            KeyCode::KeyZ if ctrl && self.solid_preview => {
                if shift {
                    Cmd::Redo
                } else {
                    Cmd::Undo
                }
            }
            KeyCode::KeyY if ctrl && self.solid_preview => Cmd::Redo,
            KeyCode::Escape if self.pick.is_some() => Cmd::Cancel,
            KeyCode::KeyS if plain_shift && sketch_edit => Cmd::Symmetry,
            KeyCode::KeyI if plain_shift && sketch_edit => Cmd::Mirror,
            KeyCode::Enter | KeyCode::NumpadEnter
                if matches!(self.document.tool, Tool::FitSpline | Tool::ControlSpline) =>
            {
                if shift {
                    Cmd::FinishClosedSpline
                } else {
                    Cmd::FinishSpline
                }
            }
            KeyCode::F2 if shift => Cmd::RenameSketch,
            KeyCode::F2 => Cmd::RenamePart,
            KeyCode::Tab => Cmd::TogglePreview,
            KeyCode::KeyD if ctrl => Cmd::CopySelected,
            KeyCode::KeyV if plain_shift && sketch_edit => Cmd::PointOnCurve,
            KeyCode::KeyV => Cmd::Select,
            KeyCode::KeyI if sketch_edit => Cmd::Split,
            KeyCode::KeyQ if plain_shift && sketch_edit => Cmd::Tangent,
            KeyCode::KeyQ if sketch_edit => Cmd::Trim,
            KeyCode::KeyU if plain_shift && sketch_edit => Cmd::Offset,
            KeyCode::Comma if !ctrl && !self.solid_preview => Cmd::TangentArc,
            KeyCode::Period if !ctrl && sketch_edit => Cmd::SmoothJoin,
            KeyCode::KeyZ if plain_shift && !self.solid_preview => Cmd::LinearPattern,
            KeyCode::Semicolon if !ctrl && !self.solid_preview => Cmd::CircularPattern,
            KeyCode::KeyU if sketch_edit => Cmd::Extend,
            KeyCode::KeyW
                if plain_shift && self.document.tool != Tool::CenterArc && sketch_edit =>
            {
                Cmd::FilletCorner
            }
            KeyCode::KeyH
                if plain_shift && self.document.tool != Tool::CenterArc && sketch_edit =>
            {
                Cmd::ChamferCorner
            }
            KeyCode::KeyH if sketch_edit && !shift => Cmd::Horizontal,
            KeyCode::KeyJ if sketch_edit && !shift => Cmd::Vertical,
            KeyCode::BracketLeft if !self.solid_preview => Cmd::PreviousProfile,
            KeyCode::BracketRight if !self.solid_preview => Cmd::NextProfile,
            KeyCode::BracketLeft => Cmd::PreviousSolid,
            KeyCode::BracketRight => Cmd::NextSolid,
            KeyCode::KeyX if shift => Cmd::ExtrudeCut,
            KeyCode::KeyJ if shift => Cmd::ExtrudeJoin,
            KeyCode::KeyX if !ctrl => Cmd::ExtrudeNew,
            KeyCode::KeyL if !shift => Cmd::Line,
            KeyCode::KeyR if self.selected_dimension.is_some() && !shift && !self.solid_preview => {
                Cmd::ToggleReference
            }
            KeyCode::KeyR if shift => Cmd::CenterRectangle,
            KeyCode::KeyR => Cmd::Rectangle,
            KeyCode::KeyT if shift => Cmd::ThreePointCenterRectangle,
            KeyCode::KeyT => Cmd::ThreePointRectangle,
            KeyCode::KeyC if shift => Cmd::ThreePointCircle,
            KeyCode::KeyC => Cmd::Circle,
            KeyCode::KeyY if plain_shift && sketch_edit => Cmd::InternalTangent,
            KeyCode::KeyD if plain_shift && sketch_edit => Cmd::RadialDistance,
            KeyCode::KeyO if plain_shift && sketch_edit => Cmd::Concentric,
            KeyCode::KeyE if plain_shift && sketch_edit => Cmd::Equal,
            KeyCode::KeyE if !ctrl => Cmd::Ellipse,
            KeyCode::KeyA if shift => Cmd::CenterArc,
            KeyCode::KeyA => Cmd::Arc,
            KeyCode::KeyW if self.document.tool == Tool::CenterArc => Cmd::ArcDirection,
            KeyCode::KeyP if plain_shift && sketch_edit => Cmd::Parallel,
            KeyCode::KeyN if plain_shift && sketch_edit => Cmd::Perpendicular,
            KeyCode::KeyL if plain_shift && sketch_edit => Cmd::Collinear,
            KeyCode::KeyB if plain_shift && sketch_edit => Cmd::LineAngle,
            KeyCode::KeyP => Cmd::Point,
            KeyCode::KeyD => Cmd::DimensionTool,
            KeyCode::KeyG if shift => Cmd::GridSnap,
            KeyCode::KeyG => Cmd::Polygon,
            KeyCode::KeyN => Cmd::GeometrySnap,
            KeyCode::KeyS if !ctrl => Cmd::Slot,
            KeyCode::KeyM if plain_shift && sketch_edit => Cmd::Midpoint,
            KeyCode::KeyM => Cmd::MoreMenu,
            KeyCode::KeyB => Cmd::NewPart,
            KeyCode::KeyK if shift => Cmd::FixedAnchor,
            KeyCode::KeyK => Cmd::Construction,
            KeyCode::Digit1 => Cmd::SketchTop,
            KeyCode::Digit2 => Cmd::SketchFront,
            KeyCode::Digit3 => Cmd::SketchRight,
            KeyCode::Digit4 => Cmd::SketchOnFace,
            KeyCode::Digit5 => Cmd::FaceContext,
            KeyCode::Digit6 => Cmd::ToggleGlyphs,
            KeyCode::Digit7 => Cmd::ToggleAutoConstraints,
            KeyCode::Digit8 => Cmd::Revolve,
            KeyCode::Digit9 if shift => Cmd::LoftJoin,
            KeyCode::Digit9 => Cmd::Loft,
            KeyCode::Digit0 if shift => Cmd::EdgeFillet,
            KeyCode::Digit0 => Cmd::EdgeChamfer,
            KeyCode::KeyF if plain_shift && sketch_edit => Cmd::Normal,
            KeyCode::KeyF if !self.solid_preview => Cmd::FitView,
            KeyCode::Escape => Cmd::Cancel,
            KeyCode::Delete | KeyCode::Backspace => Cmd::Delete,
            KeyCode::KeyS if ctrl => Cmd::Save,
            KeyCode::PageDown if ctrl => Cmd::NextPart,
            KeyCode::PageUp if ctrl => Cmd::PreviousPart,
            KeyCode::KeyE if ctrl && self.modifiers.alt_key() => Cmd::Export3mf,
            KeyCode::KeyE if ctrl && shift => Cmd::ExportAsciiStl,
            KeyCode::KeyE if ctrl => Cmd::ExportStl,
            KeyCode::KeyZ if ctrl && shift => Cmd::Redo,
            KeyCode::KeyZ if ctrl => Cmd::Undo,
            KeyCode::KeyY if ctrl => Cmd::Redo,
            _ => return None,
        })
    }

    /// Commands that refuse read-only sketches with a status message
    /// (used by the MORE menu entries).
    fn guard_read_only(&mut self, action: fn(&mut App)) {
        if self.read_only() {
            self.status_message = "READ ONLY: this sketch has unsupported native features".into();
        } else {
            action(self);
        }
    }

    fn cursor_world(&self) -> Point {
        self.document
            .screen_to_world(self.cursor, self.viewport_origin())
    }

    /// Topmost shape under the cursor (other than the selected one) that
    /// `accept` allows: the second operand of hover relations.
    fn hovered_source(&self, accept: fn(&Shape) -> bool) -> Option<u32> {
        let world = self.cursor_world();
        let selected = self.document.selected;
        self.document
            .shapes
            .iter()
            .rev()
            .find(|shape| {
                Some(shape.id()) != selected
                    && accept(shape)
                    && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
            })
            .map(Shape::id)
    }

    fn selected_shape_matches(&self, accept: fn(&Shape) -> bool) -> bool {
        self.document
            .selected
            .and_then(|id| self.document.shapes.iter().find(|shape| shape.id() == id))
            .is_some_and(accept)
    }

    fn flip_arc_direction(&mut self) {
        self.document.arc_clockwise = !self.document.arc_clockwise;
        self.status_message = if self.document.arc_clockwise {
            "Center arc: clockwise sweep"
        } else {
            "Center arc: counterclockwise sweep"
        }
        .into();
    }

    fn toggle_solid_preview(&mut self) {
        self.toggle_solid_view();
    }

    fn copy_selected_key(&mut self) {
        if !self.read_only() && self.document.copy_selected(self.move_offset) {
            self.status_message = "Copied geometry  |  Ctrl+S to save".into();
        }
    }

    fn point_on_curve_key(&mut self) {
        let source = self.hovered_source(|shape| {
            matches!(
                shape,
                Shape::Line { .. }
                    | Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
                    | Shape::Spline { .. }
            )
        });
        self.status_message = match source {
            Some(source) => match self.document.toggle_point_on_curve_with(source) {
                Ok(true) => "Point on curve linked  |  Ctrl+S to save".into(),
                Ok(false) => "Point on curve unlinked".into(),
                Err(error) => format!("Point on curve: {error}"),
            },
            None => "Hover a line or curve".into(),
        };
    }

    fn split_key(&mut self) {
        let world = self.cursor_world();
        let closed_curve = self.document.selected.and_then(|id| {
            self.document
                .shapes
                .iter()
                .find(|shape| shape.id() == id)
                .and_then(|shape| {
                    matches!(shape, Shape::Circle { .. } | Shape::Ellipse { .. })
                        .then(|| (id, shape.clone()))
                })
        });
        if let Some((id, shape)) = closed_curve {
            let on_curve = match &shape {
                Shape::Circle { center, radius, .. } => {
                    (world.distance(*center) - radius).abs() <= 8.0 / self.document.zoom.max(0.2)
                }
                Shape::Ellipse {
                    center,
                    rx,
                    ry,
                    rotation,
                    ..
                } => {
                    let angle = Shape::ellipse_parameter_angle(*center, *rx, *ry, *rotation, world);
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
                    Shape::Circle { .. } => self.document.split_selected_circle(first, world),
                    Shape::Ellipse { .. } => self.document.split_selected_ellipse(first, world),
                    _ => unreachable!(),
                };
                self.status_message = match split {
                    Ok(_) => {
                        self.split_circle_start = None;
                        "Curve split  |  Ctrl+Z to undo  |  Ctrl+S to save".into()
                    }
                    Err(error) => format!("Split: {error}"),
                };
            } else if on_curve {
                self.split_circle_start = Some((id, world));
                self.status_message = "Curve split: choose second point  |  Esc: cancel".into();
            } else {
                self.status_message = "Split: move the pointer onto the selected curve".into();
            }
        } else {
            self.split_circle_start = None;
            self.status_message = match self.document.split_selected_curve(world) {
                Ok(_) => "Curve split  |  Ctrl+Z to undo  |  Ctrl+S to save".into(),
                Err(error) => format!("Split: {error}"),
            };
        }
    }

    fn tangent_key(&mut self) {
        let source = self.hovered_source(|shape| {
            matches!(
                shape,
                Shape::Line { .. }
                    | Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
            )
        });
        self.status_message = match source {
            Some(source) => match self.document.toggle_external_tangent_with(source) {
                Ok(true) => "Tangent set  |  Ctrl+S to save".into(),
                Ok(false) => "Tangent removed".into(),
                Err(error) => format!("Tangent: {error}"),
            },
            None => "Hover a line or curve for tangent".into(),
        };
    }

    fn trim_key(&mut self) {
        let world = self.cursor_world();
        self.status_message = match self.document.trim_selected_curve(world) {
            Ok(_) => "Curve trimmed  |  Ctrl+Z to undo  |  Ctrl+S to save".into(),
            Err(error) => format!("Trim: {error}"),
        };
    }

    fn extend_key(&mut self) {
        let world = self.cursor_world();
        self.status_message = match self.document.extend_selected_curve(world) {
            Ok(_) => "Curve extended  |  Ctrl+Z to undo  |  Ctrl+S to save".into(),
            Err(error) => format!("Extend: {error}"),
        };
    }

    fn axis_key(&mut self, axis: AxisConstraint) {
        self.status_message = match self.document.toggle_axis_constraint(axis) {
            Ok(true) => format!("{} constraint set  |  Ctrl+S to save", axis.name()),
            Ok(false) => format!("{} constraint removed", axis.name()),
            Err(error) => format!("Constraint: {error}"),
        };
    }

    fn internal_tangent_key(&mut self) {
        let source =
            self.hovered_source(|shape| matches!(shape, Shape::Circle { .. } | Shape::Arc { .. }));
        self.status_message = match source {
            Some(source) => match self.document.toggle_internal_tangent_with(source) {
                Ok(true) => "Internal tangent set  |  Ctrl+S to save".into(),
                Ok(false) => "Internal tangent removed".into(),
                Err(error) => format!("Internal tangent: {error}"),
            },
            None => "Hover another circle or arc for internal tangent".into(),
        };
    }

    fn radial_distance_key(&mut self) {
        let source = self.hovered_source(|shape| matches!(shape, Shape::Circle { .. }));
        self.status_message = match source {
            Some(source) => match self.document.toggle_radius_difference_with(source) {
                Ok(true) => "Radial distance set  |  Ctrl+S to save".into(),
                Ok(false) => "Radial distance removed".into(),
                Err(error) => format!("Radial distance: {error}"),
            },
            None => "Hover another circle for radial distance".into(),
        };
    }

    fn concentric_key(&mut self) {
        let source = self.hovered_source(|shape| {
            matches!(
                shape,
                Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
            )
        });
        self.status_message = match source {
            Some(source) => match self.document.toggle_concentric_with(source) {
                Ok(true) => "Concentric set  |  Ctrl+S to save".into(),
                Ok(false) => "Concentric removed".into(),
                Err(error) => format!("Concentric: {error}"),
            },
            None => "Hover another circle, arc or ellipse".into(),
        };
    }

    fn equal_key(&mut self) {
        let source = self.hovered_source(|shape| {
            matches!(
                shape,
                Shape::Line { .. }
                    | Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
            )
        });
        self.status_message = match source {
            Some(source) => {
                let circular = self.selected_shape_matches(|shape| {
                    matches!(shape, Shape::Circle { .. } | Shape::Arc { .. })
                });
                let elliptical = self.selected_shape_matches(|shape| {
                    matches!(shape, Shape::Ellipse { .. } | Shape::EllipticArc { .. })
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

    /// Shift+P/N/L: parallel/perpendicular/collinear; `None` (Shift+B):
    /// angle between lines.
    fn line_relation_key(&mut self, kind: Option<LineRelationKind>) {
        let source = self.hovered_source(|shape| matches!(shape, Shape::Line { .. }));
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
                Ok(false) => format!("{} removed", kind.map_or("angle", LineRelationKind::name)),
                Err(error) => format!("Direction: {error}"),
            },
            None => "Hover another line for direction relation".into(),
        };
    }

    fn toggle_grid_snap(&mut self) {
        self.snap_grid = !self.snap_grid;
        self.status_message = format!(
            "Grid snapping {}",
            if self.snap_grid { "on" } else { "off" }
        );
    }

    fn toggle_geometry_snap(&mut self) {
        self.snap_geometry = !self.snap_geometry;
        self.status_message = format!(
            "Geometry snapping {}",
            if self.snap_geometry { "on" } else { "off" }
        );
    }

    fn midpoint_key(&mut self) {
        let source = self.hovered_source(|shape| {
            matches!(
                shape,
                Shape::Line { .. } | Shape::Arc { .. } | Shape::EllipticArc { .. }
            )
        });
        self.status_message = match source {
            Some(source) => match self.document.toggle_midpoint_with(source) {
                Ok(true) => "Midpoint linked  |  Ctrl+S to save".into(),
                Ok(false) => "Midpoint unlinked".into(),
                Err(error) => format!("Midpoint: {error}"),
            },
            None => "Hover a line or arc midpoint".into(),
        };
    }

    fn fixed_anchor_key(&mut self) {
        if self.read_only() {
            return;
        }
        let line_selected = self.selected_shape_matches(|shape| {
            matches!(shape, Shape::Line { .. } | Shape::Rectangle { .. })
        });
        let arc_selected = self.selected_shape_matches(|shape| {
            matches!(shape, Shape::Arc { .. } | Shape::EllipticArc { .. })
        });
        let anchored_shape_selected = self.selected_shape_matches(|shape| {
            matches!(
                shape,
                Shape::Ellipse { .. }
                    | Shape::RotatedRectangle { .. }
                    | Shape::Polygon { .. }
                    | Shape::Capsule { .. }
            )
        });
        let result = if line_selected {
            let at = self.cursor_world();
            self.document.toggle_fixed_line_endpoint_at(at)
        } else if arc_selected {
            let at = self.cursor_world();
            self.document
                .toggle_fixed_arc_anchor_at(at)
                .map(|(_, enabled)| enabled)
        } else if anchored_shape_selected {
            let at = self.cursor_world();
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
    }

    fn toggle_construction_key(&mut self) {
        if !self.read_only()
            && let Some(enabled) = self.document.toggle_construction()
        {
            self.status_message = format!(
                "Construction {}  |  Ctrl+S to save",
                if enabled { "on" } else { "off" }
            );
        }
    }

    fn normal_key(&mut self) {
        let world = self.cursor_world();
        let source = self.hovered_source(|shape| {
            matches!(
                shape,
                Shape::Line { .. }
                    | Shape::Circle { .. }
                    | Shape::Arc { .. }
                    | Shape::Ellipse { .. }
                    | Shape::EllipticArc { .. }
                    | Shape::Spline { .. }
            )
        });
        self.status_message = match source {
            Some(source) => match self.document.toggle_normal_with(source, world) {
                Ok(true) => "Normal set  |  Ctrl+S to save".into(),
                Ok(false) => "Normal removed".into(),
                Err(error) => format!("Normal: {error}"),
            },
            None => "Hover a separate line or curve for Normal".into(),
        };
    }

    fn fit_view_key(&mut self) {
        self.status_message = if self
            .document
            .fit_shapes(self.layout().viewport.w, self.layout().viewport.h)
        {
            "Sketch fitted to viewport".into()
        } else {
            "Draw geometry before fitting the sketch".into()
        };
    }

    fn cancel_key(&mut self) {
        if self.loft_start.take().is_some() {
            self.status_message = "Loft cancelled".into();
            return;
        }
        if self.pick.is_some() {
            self.pick = None;
            self.status_message = "Cancelled".into();
            return;
        }
        if self.solid_preview && self.clear_topology_selection() {
            self.status_message = "Selection cleared".into();
            return;
        }
        self.box_start = None;
        self.split_circle_start = None;
        self.corner_pick = None;
        self.dimension_start = None;
        self.selected_dimension = None;
        self.spline_points.clear();
        if self.document.cancel_drag() {
            self.status_message = "Move cancelled".into();
        } else {
            self.document.draft_start = None;
            self.document.draft_end = None;
            self.document.clear_selection();
        }
    }

    fn delete_key(&mut self) {
        if self.solid_preview && self.current_topology_selection().is_some() {
            // Vertices, edges and faces are never deleted.
            self.status_message = "Select a feature row to delete".into();
            return;
        }
        if self.selected_feature_id().is_some() {
            self.delete_selected_feature();
        } else if self.solid_preview {
            if let Some(project) = &mut self.project {
                match project.delete_extrusion_at(self.selected_solid) {
                    Ok(()) => {
                        let remaining = project
                            .extrusion_previews()
                            .map_or(0, |solids| solids.len());
                        self.selected_solid = self.selected_solid.min(remaining.saturating_sub(1));
                        self.solid_preview = remaining > 0;
                        self.status_message = "Extrusion removed  |  Ctrl+S to save".into();
                    }
                    Err(error) => self.status_message = format!("Delete extrusion: {error}"),
                }
            }
        } else if !self.read_only() {
            if let Some(key) = self.selected_dimension.take() {
                self.status_message = match self.document.delete_dimension(key) {
                    Ok(()) => "Dimension removed  |  Ctrl+S to save".into(),
                    Err(error) => format!("Dimension: {error}"),
                };
            } else {
                self.document.delete_selected();
            }
        }
    }
}
