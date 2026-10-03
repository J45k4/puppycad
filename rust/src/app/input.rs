use super::*;

impl App {
    /// Pointer move. Panning, dragging, placing and the 3D view repaint on
    /// every move; idle Select mode inside the viewport repaints only when
    /// the hovered shape, anchor or glyph changes.
    pub(super) fn pointer_moved(&mut self, next: Point) {
        self.document.grid_step = self.settings.grid_mm;
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
        self.context_menu_hover();
        if let Some(drag) = &mut self.label_drag
            && drag.start.distance(next) > 3.0
        {
            drag.moved = true;
        }
        if self.document.dragging() && !self.pan_drag && self.solid_drag.is_none() {
            let world = self.document.screen_to_world(next, self.viewport_origin());
            self.document.drag_to(world);
        }
        if self.assembly.is_some() {
            self.update_assembly_hover();
        } else if self.solid_preview {
            self.update_face_hover();
        }
        if self.solid_drag.is_none() && !self.more_open {
            self.update_topology_hover();
        }
        let hover_changed = self.update_hover() | self.update_tooltip_hover();
        let layout = self.layout();
        let idle_select = self.document.tool == Tool::Select
            && !self.solid_preview
            && !self.pan_drag
            && self.solid_drag.is_none()
            && self.box_start.is_none()
            && self.label_drag.is_none()
            && self.pick.is_none()
            && !self.document.dragging()
            && self.projects_page.is_none()
            && !self.more_open
            && layout.viewport.contains(next.x, next.y);
        if hover_changed || !idle_select {
            self.redraw();
        }
    }

    /// Recompute [`Hover`]; true when it changed.
    pub(super) fn update_hover(&mut self) -> bool {
        let layout = self.layout();
        let mut next = Hover::default();
        if self.document.tool == Tool::Select
            && !self.solid_preview
            && self.projects_page.is_none()
            && !self.document.dragging()
            && layout.viewport.contains(self.cursor.x, self.cursor.y)
        {
            let origin = self.viewport_origin();
            let world = self.document.screen_to_world(self.cursor, origin);
            next.glyph = self
                .show_glyphs
                .then(|| self.document.glyph_at(self.cursor, origin))
                .flatten()
                .map(|placed| placed.glyph);
            if next.glyph.is_none() {
                self.document.selection_filter = self.selection_filter;
                next.shape = self.document.pick_at(world);
                next.anchor = self.document.anchor_at(world);
            }
        }
        let changed = next != self.hover;
        self.hover = next;
        changed
    }

    pub(super) fn handle_left_press(&mut self) {
        self.handle_left_press_at();
        self.follow_props_edit();
    }

    fn handle_left_press_at(&mut self) {
        self.name_edit = None;
        let layout = self.layout();
        let point = self.cursor;
        if self.context_menu_click(point) {
            self.redraw();
            return;
        }
        self.document.grid_step = self.settings.grid_mm;
        if self.overlay_click(layout, point) || self.settings_click(layout, point) {
            self.redraw();
            return;
        }
        if self.history_click(layout, point) {
            self.redraw();
            return;
        }
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
        if self.body_pick && layout.viewport.contains(point.x, point.y) {
            self.pick_target_body(layout, point);
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
            for (index, (command, _)) in MORE_MENU.iter().enumerate() {
                if layout.more_item(index).contains(point.x, point.y) {
                    self.run_command(*command);
                    self.redraw();
                    return;
                }
            }
            self.more_open = false;
        }
        if self.quick_actions_click(layout, point) {
            self.redraw();
            return;
        }
        // The lower part of Properties scrolls: map to unscrolled rows.
        let point = Self::props_point(layout, point);
        if self.assembly.is_some() {
            self.assembly_click(layout, point);
            self.redraw();
            return;
        }
        if self.edge_pick.is_some() && layout.viewport.contains(point.x, point.y) {
            self.toggle_edge_at(layout, point);
            self.redraw();
            return;
        }
        if self
            .revolve_draft
            .as_ref()
            .is_some_and(|draft| draft.axis.is_none())
            && layout.viewport.contains(point.x, point.y)
        {
            self.pick_revolve_axis(point);
            self.redraw();
            return;
        }
        if self.use_mode && !self.solid_preview && layout.viewport.contains(point.x, point.y) {
            if self.use_click(point) {
                self.redraw();
                return;
            }
            self.use_mode = false;
        }
        if !self.solid_preview
            && self.document.selected.is_none()
            && !self
                .selected_dimension
                .is_some_and(|key| self.document.dimension_keys().contains(&key))
            && let Some(index) = (0..=self.document.dims.variables.len())
                .find(|index| Self::variable_row(layout, *index).contains(point.x, point.y))
        {
            if self.read_only() {
                self.status_message =
                    "READ ONLY: this sketch has unsupported native features".into();
            } else {
                let variable = self.document.dims.variables.get(index);
                self.editing = Some(DimensionEdit {
                    field: EditField::Variable(variable.map(|_| index)),
                    buffer: variable
                        .map(|variable| format!("{} = {}", variable.name, variable.expression))
                        .unwrap_or_default(),
                });
                self.status_message =
                    "Type name = expression (units, #variables), Enter  |  empty: delete  |  Esc"
                        .into();
            }
            self.redraw();
            return;
        }
        if !self.solid_preview
            && layout.right.contains(point.x, point.y)
            && let Some(key) = self
                .selected_dimension
                .filter(|key| self.document.dimension_keys().contains(key))
        {
            let y = point.y - layout.right.y;
            if (Self::DIMENSION_VALUE_ROW..Self::DIMENSION_VALUE_ROW + 25.0).contains(&y) {
                if self.read_only() {
                    self.status_message =
                        "READ ONLY: this sketch has unsupported native features".into();
                } else {
                    self.begin_label_edit(key);
                }
            } else if (Self::DIMENSION_MODE_ROW..Self::DIMENSION_MODE_ROW + 28.0).contains(&y) {
                self.toggle_selected_reference();
            }
            self.redraw();
            return;
        }
        if self.solid_option_row_click(layout, point) {
            self.redraw();
            return;
        }
        if let Some(field) = self.editable_field_at(point) {
            if self.read_only()
                && !matches!(field, EditField::ExtrusionDepth | EditField::FeatureValue)
            {
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
                    EditField::FeatureValue => {
                        "Type the new angle (deg) or size (mm), then Enter  |  Esc to cancel"
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
                    EditField::OffsetDistance => {
                        "Type signed offset distance in mm, then Enter  |  Esc to cancel"
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
        if layout.left.contains(point.x, point.y) && point.y < layout.left.y + PANEL_ROWS_TOP {
            return;
        }
        let part_rows = self.part_rows();
        for (row_index, target) in part_rows.iter().enumerate() {
            if layout.part_row(row_index).contains(point.x, point.y) {
                self.open_project_row(*target);
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
                            self.document.set_reference_context(if self.face_context {
                                project.sketch_context(project.active_sketch())
                            } else {
                                Default::default()
                            });
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
        if self.click_feature_tree(layout, point) {
            self.clear_topology_selection();
            self.redraw();
            return;
        }
        let solid_count = self.tree_row_count();
        for (index, shape) in self.document.shapes.iter().enumerate() {
            if layout
                .row(index + solid_count, part_rows.len(), self.sketch_count())
                .contains(point.x, point.y)
            {
                if self.modifiers.shift_key() {
                    self.document.toggle_selected(shape.id());
                } else {
                    self.document.select_only(Some(shape.id()));
                }
                self.solid_preview = false;
                self.select_tool(Tool::Select);
                self.redraw();
                return;
            }
        }
        if !self.solid_preview {
            for index in 0..3 {
                if layout.filter_button(index).contains(point.x, point.y) {
                    let filter = &mut self.selection_filter;
                    let (flag, label) = match index {
                        0 => (&mut filter.geometry, "Geometry"),
                        1 => (&mut filter.construction, "Construction"),
                        _ => (&mut filter.points, "Point"),
                    };
                    *flag = !*flag;
                    self.status_message =
                        format!("{label} selection {}", if *flag { "on" } else { "off" });
                    self.document.selection_filter = self.selection_filter;
                    self.redraw();
                    return;
                }
            }
        }
        if !layout.viewport.contains(point.x, point.y) {
            return;
        }
        if self.solid_preview {
            // Left-click in the 3D view selects vertices, edges and faces;
            // it never orbits.
            if self.topology_active() {
                self.topology_click(layout, point);
                self.redraw();
            }
            return;
        }
        let raw_world = self.document.screen_to_world(point, self.viewport_origin());
        if self.pick.is_some() && !self.read_only() {
            self.handle_pick(raw_world);
            self.redraw();
            return;
        }
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
        if self.document.tool == Tool::Select && !self.read_only() && self.corner_pick_click() {
            self.redraw();
            return;
        }
        match self.document.tool {
            Tool::Dimension => self.dimension_click(point, raw_world),
            Tool::Select => {
                self.split_circle_start = None;
                if let Some(key) = self
                    .document
                    .dimension_label_at(point, self.viewport_origin())
                {
                    self.selected_dimension = Some(key);
                    self.document.clear_selection();
                    self.label_drag = Some(LabelDrag {
                        key,
                        start: point,
                        moved: false,
                    });
                    self.redraw();
                    return;
                }
                if self.show_glyphs
                    && let Some(placed) = self.document.glyph_at(point, self.viewport_origin())
                {
                    // A relation glyph click selects the entity it sits beside.
                    self.selected_dimension = None;
                    self.document.select_only(Some(placed.glyph.owner));
                    self.status_message = format!(
                        "{} relation of {}",
                        placed.glyph.label,
                        placed.glyph.partners.len()
                    );
                    self.redraw();
                    return;
                }
                self.selected_dimension = None;
                self.document.selection_filter = self.selection_filter;
                let hit = self.document.pick_at(world);
                let on_anchor = self.document.handle_at(world);
                if hit.is_none() && !on_anchor {
                    // Empty canvas: start a rubber band; release selects.
                    self.box_start = Some(point);
                } else if self.modifiers.shift_key() {
                    if let Some(id) = hit {
                        self.document.toggle_selected(id);
                    }
                } else {
                    // Pressing on a member of a multi-selection keeps it so
                    // the whole group can be dragged.
                    let keep_group = !on_anchor
                        && hit.is_some_and(|id| self.document.is_selected(id))
                        && self.document.selection_count() > 1;
                    if !keep_group {
                        self.document.select_at(world);
                    }
                    if !self.read_only() {
                        self.document.begin_drag(world);
                    }
                }
            }
            Tool::TangentArc => self.tangent_arc_click(raw_world, world),
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
                    let (finished, axis) = self
                        .document
                        .finish_draft_auto(world, self.auto_constraints);
                    if let Some(axis) = axis {
                        self.status_message =
                            format!("Auto {}  |  7: automatic constraints", axis.name());
                    } else if finished {
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
}

/// Characters accepted while typing a dimension expression.
pub(super) fn expression_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || " .+-*/()#_".contains(ch)
}

impl App {
    /// A label click opens the inline editor; a drag moves the label.
    fn release_label(&mut self, drag: LabelDrag) {
        if self.read_only() {
            self.status_message = "READ ONLY: this sketch has unsupported native features".into();
            return;
        }
        if drag.moved {
            let at = self
                .document
                .screen_to_world(self.cursor, self.viewport_origin());
            self.status_message = match self
                .document
                .set_dimension_label_position(drag.key, Some(at))
            {
                Ok(()) => "Moved dimension label  |  Ctrl+S to save".into(),
                Err(error) => format!("Label: {error}"),
            };
        } else {
            self.begin_label_edit(drag.key);
        }
    }

    /// D tool: an entity gets its own dimension; two anchors (or an anchor
    /// and the origin) get a distance, Shift for X and Ctrl for Y. The new
    /// dimension's label opens for typing its value.
    fn dimension_click(&mut self, screen: Point, world: Point) {
        let tolerance = 8.0 / self.document.zoom.max(0.2);
        let anchor = self.document.dimension_anchor_near(world, tolerance);
        let origin = world.distance(Point { x: 0.0, y: 0.0 }) <= tolerance;
        let Some(start) = self.dimension_start else {
            if let Some(anchor) = anchor {
                self.dimension_start = Some(anchor);
                self.status_message =
                    "Dimension: pick the second point or the origin  |  Shift: X  Ctrl: Y  |  Esc: cancel"
                        .into();
            } else if let Some(key) = self
                .document
                .dimension_label_at(screen, self.viewport_origin())
            {
                self.begin_label_edit(key);
            } else if let Some(id) = self
                .document
                .shapes
                .iter()
                .rev()
                .find(|shape| shape.hit(world, 5.0 / self.document.zoom.max(0.2)))
                .map(Shape::id)
            {
                match self.document.add_entity_dimension(id) {
                    Ok(key) => self.begin_label_edit(key),
                    Err(error) => self.status_message = format!("Dimension: {error}"),
                }
            } else {
                self.status_message =
                    "Dimension: click an entity, a point, line end or curve center".into();
            }
            return;
        };
        let axis = if self.modifiers.shift_key() {
            Some(model::PointAxis::X)
        } else if self.modifiers.control_key() {
            Some(model::PointAxis::Y)
        } else {
            None
        };
        let target = match anchor {
            Some(anchor) => Some(anchor),
            None if origin => None,
            None => {
                self.status_message =
                    "Dimension: pick a point, line end, curve center or the origin".into();
                return;
            }
        };
        self.dimension_start = None;
        match self.document.add_point_dimension(start, target, axis) {
            Ok(key) => self.begin_label_edit(key),
            Err(error) => self.status_message = format!("Dimension: {error}"),
        }
    }

    pub(super) fn begin_label_edit(&mut self, key: DimensionKey) {
        self.selected_dimension = Some(key);
        self.document.selected = None;
        if self.document.is_reference(key) {
            self.editing = None;
            self.status_message =
                "Reference dimensions are measured  |  R: make driving to edit".into();
            return;
        }
        self.editing = Some(DimensionEdit {
            field: EditField::Label(key),
            buffer: String::new(),
        });
        self.status_message = "Type a value, then Enter  |  Esc to cancel".into();
    }
}

impl ApplicationHandler for App {
    fn new_events(&mut self, _: &ActiveEventLoop, cause: winit::event::StartCause) {
        // The tooltip timer fired: repaint once to show it.
        if matches!(cause, winit::event::StartCause::ResumeTimeReached { .. }) {
            self.redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(match self.tooltip_deadline() {
            Some(deadline) => winit::event_loop::ControlFlow::WaitUntil(deadline),
            None => winit::event_loop::ControlFlow::Wait,
        });
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        self.settings = Settings::load();
        self.apply_settings();
        let attributes = self.window_attributes();
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
        self.init_window_scale(&window);
        window.request_redraw();
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.window.as_ref().is_none_or(|window| window.id() != id) {
            return;
        }
        let exit = match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.handle_key(KeyPress {
                        physical_key: event.physical_key,
                        text: event.text,
                    });
                }
                false
            }
            event => self.handle_window_event(event),
        };
        if exit {
            event_loop.exit();
        }
    }
}

/// The parts of a `winit` key press the app reads. `winit::event::KeyEvent`
/// cannot be built outside winit, so tests drive `App::handle_key` with this.
pub(super) struct KeyPress {
    pub physical_key: PhysicalKey,
    pub text: Option<winit::keyboard::SmolStr>,
}

impl App {
    /// Handle every window event except key presses (see `handle_key`).
    /// Returns true when the event loop should exit.
    pub(super) fn handle_window_event(&mut self, event: WindowEvent) -> bool {
        match event {
            WindowEvent::CloseRequested => {
                self.save_window_geometry();
                return true;
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
                self.redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                // Physical pixels to layout units (UI scale).
                self.pointer_moved(Point {
                    x: position.x as f32 / self.ui_scale,
                    y: position.y as f32 / self.ui_scale,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => match (state, button) {
                (ElementState::Pressed, MouseButton::Left) => self.handle_left_press(),
                (ElementState::Released, MouseButton::Left) => {
                    if std::mem::take(&mut self.rollback_drag) {
                        self.drop_rollback(self.cursor.y);
                    }
                    if let Some(start) = self.box_start.take() {
                        self.finish_box_select(start, self.cursor);
                    }
                    if let Some(drag) = self.label_drag.take() {
                        self.release_label(drag);
                    }
                    if self.document.finish_drag() {
                        self.status_message = "Moved geometry  |  Ctrl+S to save".into();
                    }
                    self.redraw();
                }
                (ElementState::Pressed, button @ (MouseButton::Right | MouseButton::Middle)) => {
                    let layout = self.layout();
                    self.context_menu = None;
                    self.right_press = (button == MouseButton::Right).then_some(self.cursor);
                    if !self.begin_solid_drag(layout, self.cursor, button)
                        && button == MouseButton::Middle
                        && !self.solid_preview
                    {
                        self.pan_drag = true;
                    }
                }
                (ElementState::Released, button @ (MouseButton::Right | MouseButton::Middle)) => {
                    self.solid_drag = None;
                    self.pan_drag = false;
                    if button == MouseButton::Right
                        && let Some(start) = self.right_press.take()
                    {
                        self.right_release(start, self.cursor);
                    }
                    self.redraw();
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
                } else if self.layout().left.contains(self.cursor.x, self.cursor.y) {
                    let rows = match delta {
                        MouseScrollDelta::LineDelta(_, y) => -y,
                        MouseScrollDelta::PixelDelta(position) => {
                            -position.y as f32 / PANEL_ROW_STEP
                        }
                    };
                    self.scroll_panel(rows);
                    self.redraw();
                } else if self.props_hit(self.layout(), self.cursor) {
                    let rows = match delta {
                        MouseScrollDelta::LineDelta(_, y) => -y,
                        MouseScrollDelta::PixelDelta(position) => -position.y as f32 / 30.0,
                    };
                    self.scroll_props(rows);
                    self.redraw();
                } else if (self.solid_preview || self.assembly.is_some())
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
            WindowEvent::RedrawRequested => {
                let painter = self.paint();
                if let Some(renderer) = &mut self.renderer {
                    match renderer.draw(painter) {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            let size = self.window.as_ref().expect("window exists").inner_size();
                            renderer.resize(size.width, size.height);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => return true,
                        Err(error) => eprintln!("render: {error}"),
                    }
                }
            }
            _ => {}
        }
        false
    }

    pub(super) fn handle_key(&mut self, event: KeyPress) {
        self.handle_key_press(event);
        self.follow_panel_selection();
        self.follow_props_edit();
    }

    fn handle_key_press(&mut self, event: KeyPress) {
        if let PhysicalKey::Code(code) = event.physical_key {
            if self.context_menu_key(code)
                || self.palette_key(code, event.text.as_deref())
                || self.shortcut_overlay_key(code)
                || self.settings_key(code)
            {
                self.redraw();
                return;
            }
            if self.projects_page.is_none()
                && self.name_edit.is_none()
                && self.editing.is_none()
                && let Some(command) = self.quick_action_chord(code)
            {
                self.run_command(command);
                self.redraw();
                return;
            }
            if self.projects_page.is_some() {
                let layout = self.layout();
                match code {
                    KeyCode::Escape => self.close_projects_page(),
                    KeyCode::ArrowDown => self.select_project_row(1, layout),
                    KeyCode::ArrowUp => self.select_project_row(-1, layout),
                    KeyCode::Enter | KeyCode::NumpadEnter => {
                        if let Some(index) = self.projects_page.as_ref().map(|page| page.selected) {
                            self.open_listed_project(index);
                        }
                    }
                    KeyCode::Backspace => self.show_parent_folder(),
                    KeyCode::KeyS if self.modifiers.control_key() || self.modifiers.super_key() => {
                        self.save_project()
                    }
                    KeyCode::PageDown => {
                        self.select_project_row(project_page_visible_rows(layout) as isize, layout)
                    }
                    KeyCode::PageUp => self
                        .select_project_row(-(project_page_visible_rows(layout) as isize), layout),
                    _ => {}
                }
                self.redraw();
                return;
            }
            if self.assembly.is_some() && self.assembly_prompt_key(code, event.text.as_deref()) {
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
                    KeyCode::KeyA if self.modifiers.control_key() || self.modifiers.super_key() => {
                        if let Some(edit) = &mut self.name_edit {
                            edit.buffer.clear();
                        }
                    }
                    _ if !self.modifiers.control_key() && !self.modifiers.super_key() => {
                        if let (Some(edit), Some(text)) = (&mut self.name_edit, &event.text) {
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
                        self.corner_pair = None;
                        self.pattern_prompt = None;
                        self.status_message = "Dimension edit cancelled".into();
                    }
                    KeyCode::Tab
                        if self.editing.as_ref().is_some_and(|edit| {
                            matches!(
                                edit.field,
                                EditField::ChamferFirst | EditField::ChamferSecond
                            )
                        }) =>
                    {
                        self.toggle_chamfer_mode();
                        if let Some(edit) = &mut self.editing {
                            edit.field = EditField::ChamferFirst;
                            edit.buffer.clear();
                        }
                        self.status_message = format!(
                            "Chamfer mode: {}  |  type the first setback in mm",
                            self.corner_values.mode.name()
                        );
                    }
                    KeyCode::Backspace => {
                        if let Some(editing) = &mut self.editing {
                            editing.buffer.pop();
                        }
                    }
                    _ if !self.modifiers.control_key() && !self.modifiers.super_key() => {
                        let pattern_signed = self.pattern_step_signed();
                        if let (Some(editing), Some(text)) = (&mut self.editing, &event.text) {
                            for ch in text.chars() {
                                if matches!(
                                    editing.field,
                                    EditField::Label(_)
                                        | EditField::Dimension(_)
                                        | EditField::Variable(_)
                                ) {
                                    let allowed = expression_char(ch)
                                        || (ch == '='
                                            && matches!(editing.field, EditField::Variable(_)));
                                    if allowed && editing.buffer.len() < 64 {
                                        editing.buffer.push(ch);
                                    }
                                    continue;
                                }
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
                                                | EditField::OffsetCreate
                                                | EditField::OffsetDistance
                                                | EditField::Dimension(DimensionField::ArcSweep)
                                                | EditField::Dimension(DimensionField::Rotation)
                                                | EditField::CenterX
                                                | EditField::CenterY
                                                | EditField::SplineX
                                                | EditField::SplineY
                                                | EditField::PointX
                                                | EditField::PointY
                                        )
                                        || (matches!(editing.field, EditField::PatternValue)
                                            && pattern_signed)))
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
            if self.extrude_draft.is_some() {
                self.handle_draft_key(code, event.text.as_deref());
                self.redraw();
                return;
            }
            if self.revolve_draft.is_some() {
                self.handle_revolve_key(code, event.text.as_deref());
                self.redraw();
                return;
            }
            if self.edge_pick.is_some() {
                self.handle_edge_key(code, event.text.as_deref());
                self.redraw();
                return;
            }
            if let Some(command) = self.command_for_key(code) {
                self.run_command(command);
            }
            self.redraw();
        }
    }
}

impl App {
    /// Finish a Select-mode rubber band from screen `start` to `end`. A
    /// near-zero box is a click on empty canvas: it clears the selection
    /// unless Shift is held.
    pub(super) fn finish_box_select(&mut self, start: Point, end: Point) {
        let shift = self.modifiers.shift_key();
        if start.distance(end) < 3.0 {
            if !shift {
                self.document.clear_selection();
            }
            return;
        }
        let origin = self.viewport_origin();
        let from = self.document.screen_to_world(start, origin);
        let to = self.document.screen_to_world(end, origin);
        self.document.selection_filter = self.selection_filter;
        self.document.box_select(from, to, shift);
        let count = self.document.selection_count();
        self.status_message = format!(
            "{count} selected  |  {}",
            if end.x >= start.x {
                "window: fully enclosed"
            } else {
                "crossing: touched"
            }
        );
    }
}
