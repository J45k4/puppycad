//! Quick actions: the NEXT section of the Properties column. A rule table
//! over the app state proposes the most likely next registry commands;
//! every row is a command that is available right now. Rows run on click
//! or with Ctrl+1..7, and Ctrl+Enter finishes the open sketch.

use super::*;
use std::rc::Rc;

/// One NEXT row.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Suggestion {
    pub(super) cmd: Cmd,
    pub(super) label: String,
    /// First chord of the command's shortcut text ("" for none).
    pub(super) shortcut: &'static str,
    /// Why it is suggested, drawn dimmed under the label.
    pub(super) reason: String,
}

/// State the suggestions depend on; the list is rebuilt only when it changes
/// (edits, selection, hover target kind), not on every pointer move.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct QuickKey {
    revision: u64,
    features: usize,
    sketch: usize,
    solid_preview: bool,
    drafts: (bool, bool, bool),
    tool: Tool,
    selected: Vec<u32>,
    dimension: bool,
    hover_shape: Option<u32>,
    /// Hover target kind + id in 3D (face label, topology element).
    hovered_face: Option<String>,
    topo_hover: Option<TopoRef>,
    topo: Vec<TopoRef>,
    selected_solid: usize,
    assembly: Option<(Option<usize>, u64)>,
    read_only: bool,
    has_project: bool,
}

pub(super) type QuickCache = std::cell::RefCell<Option<(QuickKey, Rc<Vec<Suggestion>>)>>;

/// Most rows shown; short windows show fewer (see `quick_row_cap`).
pub(super) const QUICK_MAX_ROWS: usize = 7;
const QUICK_SHORT_ROWS: usize = 4;
const QUICK_HEADER: f32 = 22.0;
const QUICK_ROW: f32 = 31.0;

struct Rules<'a> {
    app: &'a App,
    items: Vec<Suggestion>,
}

impl Rules<'_> {
    fn push(&mut self, cmd: Cmd, label: &str, reason: impl Into<String>) {
        let command = command(cmd);
        if self.items.iter().any(|item| item.cmd == cmd) || !self.app.command_available(command) {
            return;
        }
        self.items.push(Suggestion {
            cmd,
            label: label.to_string(),
            shortcut: command.shortcut.split(" / ").next().unwrap_or_default(),
            reason: reason.into(),
        });
    }
}

/// Ranked next actions (3..=7) for the current state.
pub(super) fn suggest(app: &App) -> Vec<Suggestion> {
    let mut rules = Rules {
        app,
        items: Vec::new(),
    };
    if let Some(reason) = app.project.as_ref().and_then(ProjectFile::read_only_reason) {
        rules.push(
            Cmd::OpenProject,
            "Open project",
            format!("Read only: {reason}"),
        );
    }
    if let Some(view) = &app.assembly {
        assembly_rules(&mut rules, view.selected.is_some());
    } else if app.extrude_draft.is_some()
        || app.revolve_draft.as_ref().is_some_and(|d| d.axis.is_some())
        || app.edge_pick.is_some()
    {
        rules.push(Cmd::DraftCommit, "Commit", "create the feature");
        rules.push(Cmd::DraftFlip, "Flip", "extrude the other way");
        rules.push(
            Cmd::DraftEndCondition,
            "End condition",
            "blind, through all, up to",
        );
        rules.push(Cmd::DraftCancel, "Cancel", "close the prompt");
    } else if app.solid_preview {
        solid_rules(&mut rules);
    } else {
        sketch_rules(&mut rules);
    }
    // Always offer at least three ways forward.
    rules.push(
        Cmd::CommandPalette,
        "Search commands",
        "every command by name",
    );
    rules.push(Cmd::ShortcutOverlay, "Keyboard shortcuts", "cheat sheet");
    rules.push(Cmd::Save, "Save project", "write the .pcad file");
    rules.items.truncate(QUICK_MAX_ROWS);
    rules.items
}

fn assembly_rules(rules: &mut Rules, instance_selected: bool) {
    if instance_selected {
        rules.push(
            Cmd::MateInstances,
            "Mate",
            "hover a face of another instance",
        );
        rules.push(
            Cmd::FastenInstance,
            "Fasten to world",
            "lock the selected instance",
        );
        rules.push(
            Cmd::DeleteInstance,
            "Delete instance",
            "remove the selected instance",
        );
    } else {
        rules.push(Cmd::InsertInstance, "Insert part", "place a part instance");
        rules.push(Cmd::ArmMate, "Mate", "select a face, then hover another");
    }
    rules.push(Cmd::ExportStl, "Export STL", "every instance in place");
    rules.push(Cmd::AssemblyView, "Close assembly", "back to the part");
}

fn solid_rules(rules: &mut Rules) {
    let app = rules.app;
    let selection: Vec<TopoRef> = app
        .current_topology_selection()
        .map(|(_, _, picks)| picks.iter().map(|pick| pick.element).collect())
        .unwrap_or_default();
    let faces = selection
        .iter()
        .filter(|e| matches!(e, TopoRef::Face(_)))
        .count();
    let edges = selection
        .iter()
        .filter(|e| matches!(e, TopoRef::Edge(_)))
        .count();
    let measure_reason = if selection.len() == 2 {
        "distance and angle of the two"
    } else {
        "Shift+click another element"
    };
    let plural = |n: usize| if n == 1 { "" } else { "s" };
    if edges > 0 {
        let reason = format!("{edges} edge{} selected", plural(edges));
        rules.push(Cmd::EdgeChamfer, "Chamfer", reason.clone());
        rules.push(Cmd::EdgeFillet, "Fillet", reason);
        rules.push(Cmd::Measure, "Measure", measure_reason);
    }
    if faces > 0 {
        rules.push(
            Cmd::SketchOnFace,
            "Sketch on face",
            "new sketch on the selected face",
        );
        rules.push(Cmd::Measure, "Measure", measure_reason);
        if app.project_into_sketch_available() {
            rules.push(
                Cmd::ProjectIntoSketch,
                "Project into sketch",
                "copy the face outline into a face sketch",
            );
        }
    }
    if !selection.is_empty() {
        rules.push(Cmd::Measure, "Measure", measure_reason);
        rules.push(Cmd::Cancel, "Clear selection", "Esc");
    }
    let face_sketch = app
        .project
        .as_ref()
        .is_some_and(|project| project.sketch_face(project.active_sketch()).is_some());
    if face_sketch && !app.read_only() && app.sketch_closed() {
        rules.push(
            Cmd::ExtrudeNew,
            "Extrude face sketch",
            "the open face sketch has a profile",
        );
    }
    if selection.is_empty() {
        let hovered_edge = matches!(app.topo_hover, Some(TopoRef::Edge(_)));
        if hovered_edge {
            rules.push(
                Cmd::EdgeChamfer,
                "Chamfer",
                "1 edge hovered - click to select it",
            );
        }
        let face_reason = match &app.hovered_face {
            Some(face) => format!("Sketch on face - {}", face.label),
            None => "hover a face first".to_string(),
        };
        rules.push(Cmd::SketchOnFace, "Sketch on face", face_reason);
        rules.push(Cmd::SketchTop, "New sketch on TOP", "start a new profile");
        if let Some(label) = app.feature_value_label() {
            rules.push(
                Cmd::EditFeatureValue,
                label,
                "the selected feature's main value",
            );
        }
        if app.project.as_ref().map_or(0, ProjectFile::feature_count) > 1 {
            rules.push(Cmd::FeatureUp, "Reorder feature", "Ctrl+Up / Ctrl+Down");
        }
        rules.push(
            Cmd::SuppressFeature,
            "Suppress feature",
            "selected feature row",
        );
        rules.push(
            Cmd::RollbackUp,
            "Roll back here",
            "edit before this feature",
        );
        rules.push(Cmd::ExportStl, "Export STL", "for printing");
        rules.push(Cmd::AssemblyView, "Assembly view", "combine parts");
        rules.push(Cmd::TogglePreview, "Back to sketch", "Tab");
    }
}

fn sketch_rules(rules: &mut Rules) {
    let app = rules.app;
    let document = &app.document;
    let has_body = app
        .project
        .as_ref()
        .and_then(ProjectFile::extrusion_previews)
        .is_some_and(|bodies| !bodies.is_empty());
    let locked = app
        .project
        .as_ref()
        .is_some_and(ProjectFile::active_sketch_has_extrusion);
    if locked {
        rules.push(
            Cmd::FinishSketch,
            "Finish sketch",
            "this sketch is extruded and locked",
        );
        rules.push(Cmd::TogglePreview, "Tab to 3D", "see the solid");
        rules.push(Cmd::SketchTop, "New sketch on TOP", "start another profile");
        return;
    }
    if app.selected_dimension.is_some() {
        rules.push(
            Cmd::EditDimensionValue,
            "Edit dimension value",
            "type a new value",
        );
        rules.push(
            Cmd::ToggleReference,
            "Make reference / driving",
            "toggle the selected dimension",
        );
        rules.push(
            Cmd::Delete,
            "Delete dimension",
            "remove the selected dimension",
        );
        rules.push(Cmd::Cancel, "Deselect", "Esc");
        return;
    }
    let selected_line = app.selected_shape_matches(|shape| matches!(shape, Shape::Line { .. }));
    let hovered_line = app.hover.shape.is_some_and(|id| {
        Some(id) != document.selected
            && document
                .shapes
                .iter()
                .any(|shape| shape.id() == id && matches!(shape, Shape::Line { .. }))
    });
    if selected_line && hovered_line {
        rules.push(
            Cmd::Parallel,
            "Parallel",
            "selected line with the hovered one",
        );
        rules.push(
            Cmd::Perpendicular,
            "Perpendicular",
            "selected line with the hovered one",
        );
        rules.push(Cmd::Equal, "Equal", "same length as the hovered line");
    }
    if document.shapes.is_empty() {
        rules.push(
            Cmd::Rectangle,
            "Draw rectangle",
            "the quickest closed profile",
        );
        rules.push(Cmd::Circle, "Draw circle", "a round profile");
        rules.push(Cmd::Line, "Draw line", "click points, close the loop");
        if has_body {
            rules.push(Cmd::FinishSketch, "Finish sketch", "back to the 3D view");
        }
        let plane = app
            .project
            .as_ref()
            .and_then(|project| project.sketch_plane(project.active_sketch()));
        if plane == Some(SketchPlane::Front) {
            rules.push(Cmd::SketchTop, "New sketch on TOP", "draw on another plane");
        } else {
            rules.push(
                Cmd::SketchFront,
                "New sketch on FRONT",
                "draw on another plane",
            );
        }
        return;
    }
    let analysis = app.sketch_analysis().clone();
    let closed = app.sketch_closed();
    if !analysis.conflicting.is_empty() {
        rules.push(
            Cmd::Undo,
            "Fix conflict - see red rows",
            "undo or delete a red constraint",
        );
    }
    if closed {
        rules.push(Cmd::FinishSketch, "Finish sketch", "closed profile ready");
        rules.push(Cmd::ExtrudeNew, "Extrude", "pull the profile into a solid");
        if analysis.dof > 0 {
            let label = format!("Add dimension - {} DOF remaining", analysis.dof);
            rules.push(Cmd::DimensionTool, &label, "fix sizes before extruding");
        } else {
            rules.push(
                Cmd::DimensionTool,
                "Add dimension",
                "sizes are driven by dimensions",
            );
        }
        rules.push(Cmd::Revolve, "Revolve", "spin the profile around an axis");
    }
    if !document.selected_ids().is_empty() {
        rules.push(Cmd::Horizontal, "Horizontal", "constrain the selection");
        rules.push(Cmd::Vertical, "Vertical", "constrain the selection");
    }
    if !closed {
        rules.push(
            Cmd::Line,
            "Line - continue or close",
            "no closed profile yet",
        );
        rules.push(Cmd::DimensionTool, "Add dimension", "fix sizes");
        rules.push(Cmd::FitView, "Fit", "show all geometry");
        if document.tool == Tool::Select && document.selected_ids().is_empty() {
            rules.push(Cmd::FinishSketch, "Finish sketch", "leave the sketch");
        }
    }
}

impl App {
    fn quick_key(&self) -> QuickKey {
        QuickKey {
            revision: self.document.revision(),
            features: self.project.as_ref().map_or(0, ProjectFile::feature_count),
            sketch: self.project.as_ref().map_or(0, ProjectFile::active_sketch),
            solid_preview: self.solid_preview,
            drafts: (
                self.extrude_draft.is_some(),
                self.revolve_draft
                    .as_ref()
                    .is_some_and(|d| d.axis.is_some()),
                self.edge_pick.is_some(),
            ),
            tool: self.document.tool,
            selected: self.document.selected_ids(),
            dimension: self.selected_dimension.is_some(),
            hover_shape: self.hover.shape,
            hovered_face: self.hovered_face.as_ref().map(|face| face.label.clone()),
            topo_hover: self.topo_hover.filter(|_| self.solid_preview),
            topo: self
                .topo_selection
                .iter()
                .map(|pick| pick.element)
                .collect(),
            selected_solid: self.selected_solid,
            assembly: self
                .assembly
                .as_ref()
                .map(|view| (view.selected, view.revision)),
            read_only: self.read_only(),
            has_project: self.project.is_some(),
        }
    }

    /// The current suggestions, rebuilt only when [`QuickKey`] changes.
    pub(super) fn quick_suggestions(&self) -> Rc<Vec<Suggestion>> {
        let key = self.quick_key();
        if let Some((cached, items)) = &*self.quick_cache.borrow()
            && *cached == key
        {
            return items.clone();
        }
        self.quick_builds.set(self.quick_builds.get() + 1);
        let items = Rc::new(suggest(self));
        *self.quick_cache.borrow_mut() = Some((key, items.clone()));
        items
    }

    /// Rows that fit: 7, or 4 when the window is short.
    pub(super) fn quick_row_cap(layout: Layout) -> usize {
        if layout.right.h < 760.0 {
            QUICK_SHORT_ROWS
        } else {
            QUICK_MAX_ROWS
        }
    }

    /// Height of the NEXT band: fixed by the column height (header plus
    /// 4 or 7 rows), so Properties can reserve the space.
    pub(super) fn quick_band_height(layout: Layout) -> f32 {
        QUICK_HEADER + Self::quick_row_cap(layout) as f32 * QUICK_ROW + 6.0
    }

    fn quick_shown(&self) -> bool {
        self.settings.quick_actions && self.projects_page.is_none()
    }

    /// Bottom of the Properties area for a base layout (NEXT band top when
    /// the band is shown).
    pub(super) fn props_bottom(&self, layout: Layout) -> f32 {
        let column_bottom = layout.right.y + layout.right.h;
        if self.quick_shown() {
            column_bottom - Self::quick_band_height(layout)
        } else {
            column_bottom
        }
    }

    /// How far the lower part of Properties can scroll: until the lowest
    /// drawn row (or the open edit field) sits above the band.
    pub(super) fn max_props_scroll(&self, layout: Layout) -> f32 {
        let field = self
            .editing
            .as_ref()
            .and_then(|edit| self.edit_field_top(layout, edit.field))
            .map_or(0.0, |top| top + 25.0);
        let content = self.props_extent.get().max(field);
        (content + 8.0 - layout.props_bottom).max(0.0)
    }

    /// The scrolled part of the Properties column for the painter.
    pub(super) fn props_region(layout: Layout) -> Option<ScrollRegion> {
        Some(ScrollRegion {
            left: layout.right.x,
            from: layout.right.y + PROPS_SCROLL_TOP,
            until: layout.right.y + layout.right.h,
            offset: layout.props_scroll,
            bottom: layout.props_bottom,
        })
    }

    /// True when `point` is over the Properties rows (not the NEXT band).
    pub(super) fn props_hit(&self, layout: Layout, point: Point) -> bool {
        layout.right.contains(point.x, point.y) && point.y < layout.props_bottom
    }

    /// Screen point to unscrolled Properties coordinates (identity outside
    /// the scrolled part).
    pub(super) fn props_point(layout: Layout, point: Point) -> Point {
        if point.x >= layout.right.x
            && point.y >= layout.right.y + PROPS_SCROLL_TOP
            && point.y < layout.props_bottom
        {
            Point {
                x: point.x,
                y: point.y + layout.props_scroll,
            }
        } else {
            point
        }
    }

    /// Wheel over Properties: `rows` positive scrolls down (30 px rows).
    pub(super) fn scroll_props(&mut self, rows: f32) {
        let layout = self.layout();
        self.props_scroll =
            (layout.props_scroll + rows * 30.0).clamp(0.0, self.max_props_scroll(layout));
    }

    /// Unscrolled top of an edit field drawn in the scrollable part.
    pub(super) fn edit_field_top(&self, layout: Layout, field: EditField) -> Option<f32> {
        let offset = match field {
            EditField::MoveX => 365.0,
            EditField::MoveY => 395.0,
            EditField::CenterX | EditField::CenterY => 550.0,
            EditField::ScaleFactor | EditField::RotateAngle => 580.0,
            EditField::Variable(index) => {
                175.0 + index.unwrap_or(self.document.dims.variables.len()) as f32 * 30.0
            }
            _ => return None,
        };
        Some(layout.right.y + offset)
    }

    /// Keep the field being edited inside the visible Properties area.
    pub(super) fn follow_props_edit(&mut self) {
        let key = (
            self.document.selected,
            self.solid_preview,
            self.selected_solid,
            self.selected_dimension.is_some(),
        );
        if key != self.props_followed {
            self.props_followed = key;
            self.props_scroll = 0.0;
        }
        let layout = self.layout();
        let Some(top) = self
            .editing
            .as_ref()
            .and_then(|edit| self.edit_field_top(layout, edit.field))
        else {
            return;
        };
        let from = layout.right.y + PROPS_SCROLL_TOP;
        if top < from {
            return;
        }
        let max = self.max_props_scroll(layout);
        let shown = top - layout.props_scroll;
        if shown < from {
            self.props_scroll = (top - from).clamp(0.0, max);
        } else if shown + 25.0 > layout.props_bottom - 4.0 {
            self.props_scroll = (top + 29.0 - layout.props_bottom).clamp(0.0, max);
        }
    }

    /// NEXT band at the bottom of the Properties column, if shown.
    pub(super) fn quick_band(&self, layout: Layout) -> Option<(Rect, usize)> {
        if !self.quick_shown() {
            return None;
        }
        let rows = self
            .quick_suggestions()
            .len()
            .min(Self::quick_row_cap(layout));
        Some((
            Rect {
                x: layout.right.x + 1.0,
                y: layout.props_bottom,
                w: layout.right.w - 1.0,
                h: layout.right.y + layout.right.h - layout.props_bottom,
            },
            rows,
        ))
    }

    pub(super) fn quick_row(band: Rect, index: usize) -> Rect {
        Rect {
            x: band.x + 8.0,
            y: band.y + QUICK_HEADER + index as f32 * QUICK_ROW,
            w: band.w - 16.0,
            h: QUICK_ROW - 3.0,
        }
    }

    pub(super) fn paint_quick_actions(&self, painter: &mut Painter, layout: Layout) {
        let Some((band, rows)) = self.quick_band(layout) else {
            return;
        };
        painter.rect(band, PANEL);
        painter.line(band.x, band.y, band.x + band.w, band.y, 1.0, BORDER);
        painter.text(band.x + 10.0, band.y + 7.0, "NEXT", 1.2, ACCENT);
        painter.text(
            band.x + band.w - 120.0,
            band.y + 8.0,
            &format!("CTRL+1..{rows}"),
            0.9,
            MUTED,
        );
        let items = self.quick_suggestions();
        for (index, item) in items.iter().take(rows).enumerate() {
            let row = Self::quick_row(band, index);
            painter.rect(row, PANEL_LIGHT);
            let chip_text = item.shortcut.to_uppercase();
            let chip_w = (chip_text.chars().count() as f32 * 7.2 + 8.0).min(90.0);
            let max_label = ((row.w - chip_w - 26.0) / 8.0).max(4.0) as usize;
            painter.text(
                row.x + 4.0,
                row.y + 4.0,
                &format!("{}", index + 1),
                1.0,
                MUTED,
            );
            painter.text(
                row.x + 16.0,
                row.y + 4.0,
                &truncate(&item.label, max_label),
                1.0,
                TEXT,
            );
            painter.text(
                row.x + 16.0,
                row.y + 16.0,
                &truncate(&item.reason, ((row.w - 20.0) / 7.2) as usize),
                0.9,
                MUTED,
            );
            if !chip_text.is_empty() {
                let chip = Rect {
                    x: row.x + row.w - chip_w - 3.0,
                    y: row.y + 2.0,
                    w: chip_w,
                    h: 12.0,
                };
                painter.outline(chip, 1.0, BORDER);
                painter.text(
                    chip.x + 4.0,
                    chip.y + 2.0,
                    &truncate(&chip_text, ((chip_w - 8.0) / 7.2 + 0.5) as usize),
                    0.9,
                    MUTED,
                );
            }
        }
    }

    /// A click on a NEXT row runs it; true when the band took the click.
    pub(super) fn quick_actions_click(&mut self, layout: Layout, point: Point) -> bool {
        let Some((band, rows)) = self.quick_band(layout) else {
            return false;
        };
        if !band.contains(point.x, point.y) {
            return false;
        }
        if let Some(index) =
            (0..rows).find(|&i| Self::quick_row(band, i).contains(point.x, point.y))
        {
            self.run_quick_action(index);
        }
        true
    }

    /// Ctrl+1..7 (handled before drafts and prompts take the keys).
    pub(super) fn quick_action_chord(&self, code: KeyCode) -> Option<Cmd> {
        let ctrl = self.modifiers.control_key() || self.modifiers.super_key();
        if !ctrl || self.modifiers.shift_key() || self.modifiers.alt_key() {
            return None;
        }
        Some(match code {
            KeyCode::Digit1 => Cmd::QuickAction1,
            KeyCode::Digit2 => Cmd::QuickAction2,
            KeyCode::Digit3 => Cmd::QuickAction3,
            KeyCode::Digit4 => Cmd::QuickAction4,
            KeyCode::Digit5 => Cmd::QuickAction5,
            KeyCode::Digit6 => Cmd::QuickAction6,
            KeyCode::Digit7 => Cmd::QuickAction7,
            _ => return None,
        })
    }

    /// Run NEXT row `index` (0-based) if it is shown and still available.
    pub(super) fn run_quick_action(&mut self, index: usize) {
        let items = self.quick_suggestions();
        let visible = Self::quick_row_cap(self.layout());
        match items.get(index).filter(|_| index < visible) {
            Some(item) if self.command_available(command(item.cmd)) => self.run_command(item.cmd),
            _ => self.status_message = format!("No quick action {}", index + 1),
        }
    }

    pub(super) fn toggle_quick_actions(&mut self) {
        self.settings.quick_actions = !self.settings.quick_actions;
        if self.window.is_some() {
            self.save_settings();
        }
        self.status_message = format!(
            "Quick actions {}",
            if self.settings.quick_actions {
                "shown"
            } else {
                "hidden"
            }
        );
    }

    /// Ctrl+Enter: leave the sketch. With a body the 3D view opens; with a
    /// closed profile but no body yet the extrude prompt opens.
    pub(super) fn finish_sketch(&mut self) {
        if self.solid_preview {
            return;
        }
        self.document.finish_drag();
        self.document.draft_start = None;
        self.document.draft_end = None;
        let has_body = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .is_some_and(|bodies| !bodies.is_empty());
        if has_body {
            self.toggle_solid_view();
            return;
        }
        let closed = self.sketch_closed();
        if closed && self.project.is_some() && !self.read_only() {
            self.begin_extrude("newBody");
            if self.extrude_draft.is_some() {
                return;
            }
        }
        self.status_message =
            "Sketch has no closed profile to extrude - draw a closed shape or press X to extrude"
                .into();
    }

    /// NEXT label for the selected feature's main value: "Edit depth" for
    /// an extrusion, "Edit value" for a revolve / edge finish.
    pub(super) fn feature_value_label(&self) -> Option<&'static str> {
        if !self.solid_preview {
            return None;
        }
        if self.selected_extension().is_some() {
            return Some("Edit value");
        }
        self.project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .is_some_and(|solids| !solids.is_empty())
            .then_some("Edit depth")
    }

    /// Open the Properties prompt for the selected feature's main value
    /// (the same field a click on its DEPTH / value row opens).
    pub(super) fn edit_feature_value(&mut self) {
        let field = match self.feature_value_label() {
            Some("Edit value") => EditField::FeatureValue,
            Some(_) => EditField::ExtrusionDepth,
            None => {
                self.status_message = "No feature value to edit".into();
                return;
            }
        };
        self.editing = Some(DimensionEdit {
            field,
            buffer: String::new(),
        });
        self.status_message = if matches!(field, EditField::FeatureValue) {
            "Type the new angle (deg) or size (mm), then Enter  |  Esc to cancel"
        } else {
            "Type positive depth in mm, then Enter  |  Esc to cancel"
        }
        .into();
    }

    /// Open the value prompt of the selected sketch dimension.
    pub(super) fn edit_selected_dimension(&mut self) {
        if let Some(key) = self.selected_dimension {
            self.begin_label_edit(key);
        }
    }

    /// Draft commands (Enter / F / E / Esc) feed the open prompt.
    pub(super) fn draft_key(&mut self, code: KeyCode) {
        if self.extrude_draft.is_some() {
            self.handle_draft_key(code, None);
        } else if self.revolve_draft.is_some() {
            self.handle_revolve_key(code, None);
        } else if self.edge_pick.is_some() {
            self.handle_edge_key(code, None);
        }
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('~');
    out
}
