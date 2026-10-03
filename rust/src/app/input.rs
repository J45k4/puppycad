use super::*;

impl App {
    pub(super) fn handle_left_press(&mut self) {
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
            if layout.more_item(3).contains(point.x, point.y) {
                self.begin_extrude("newBody");
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
                self.begin_extrude("join");
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
            if layout.more_item(13).contains(point.x, point.y) {
                if self.read_only() {
                    self.status_message =
                        "READ ONLY: this sketch has unsupported native features".into();
                } else {
                    self.begin_mirror();
                }
                self.redraw();
                return;
            }
            if layout.more_item(16).contains(point.x, point.y) {
                if self.read_only() {
                    self.status_message =
                        "READ ONLY: this sketch has unsupported native features".into();
                } else {
                    self.begin_offset();
                }
                self.redraw();
                return;
            }
            for (index, kind) in [(14, CornerKind::Fillet), (15, CornerKind::Chamfer)] {
                if layout.more_item(index).contains(point.x, point.y) {
                    if self.read_only() {
                        self.status_message =
                            "READ ONLY: this sketch has unsupported native features".into();
                    } else {
                        self.start_corner_pick(kind);
                    }
                    self.redraw();
                    return;
                }
            }
            if layout.more_item(20).contains(point.x, point.y) {
                self.sketch_on_face();
                self.redraw();
                return;
            }
            if layout.more_item(17).contains(point.x, point.y) {
                self.start_tangent_arc();
                self.redraw();
                return;
            }
            for (index, tool) in [(18, PatternTool::Linear), (19, PatternTool::Circular)] {
                if layout.more_item(index).contains(point.x, point.y) {
                    self.begin_pattern(tool);
                    self.redraw();
                    return;
                }
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
                            self.document.reference_edges = if self.face_context {
                                project.sketch_support_edges(project.active_sketch())
                            } else {
                                Vec::new()
                            };
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
            // Left-click in the 3D view is for selection in the web app; it never orbits.
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
                if let Some(drag) = &mut self.label_drag
                    && drag.start.distance(next) > 3.0
                {
                    drag.moved = true;
                }
                if self.document.dragging() && !self.pan_drag && self.solid_drag.is_none() {
                    let world = self.document.screen_to_world(next, self.viewport_origin());
                    self.document.drag_to(world);
                }
                if self.solid_preview {
                    self.update_face_hover();
                }
                self.redraw();
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
                                if let (Some(editing), Some(text)) =
                                    (&mut self.editing, &event.text)
                                {
                                    for ch in text.chars() {
                                        if matches!(
                                            editing.field,
                                            EditField::Label(_)
                                                | EditField::Dimension(_)
                                                | EditField::Variable(_)
                                        ) {
                                            let allowed = expression_char(ch)
                                                || (ch == '='
                                                    && matches!(
                                                        editing.field,
                                                        EditField::Variable(_)
                                                    ));
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
                                                )
                                                || (matches!(
                                                    editing.field,
                                                    EditField::PatternValue
                                                ) && pattern_signed)))
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
                    match code {
                        KeyCode::Escape if self.body_pick => {
                            self.body_pick = false;
                            self.status_message = "Body pick cancelled".into();
                        }
                        KeyCode::KeyE
                            if self.solid_preview
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.modifiers.shift_key() =>
                        {
                            self.cycle_solid_end_condition();
                        }
                        KeyCode::KeyF
                            if self.solid_preview
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.modifiers.shift_key() =>
                        {
                            self.toggle_solid_flip();
                        }
                        KeyCode::KeyU
                            if self.solid_preview
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.modifiers.shift_key() =>
                        {
                            self.arm_body_pick();
                        }
                        KeyCode::KeyZ
                            if self.solid_preview
                                && (self.modifiers.control_key() || self.modifiers.super_key()) =>
                        {
                            self.solid_undo_step(self.modifiers.shift_key());
                        }
                        KeyCode::KeyY
                            if self.solid_preview
                                && (self.modifiers.control_key() || self.modifiers.super_key()) =>
                        {
                            self.solid_undo_step(true);
                        }
                        KeyCode::Escape if self.pick.is_some() => {
                            self.pick = None;
                            self.status_message = "Cancelled".into();
                        }
                        KeyCode::KeyS
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.read_only()
                                && !self.solid_preview =>
                        {
                            self.begin_symmetry();
                        }
                        KeyCode::KeyI
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.read_only()
                                && !self.solid_preview =>
                        {
                            self.begin_mirror();
                        }
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
                        KeyCode::Tab => self.toggle_solid_view(),
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
                        KeyCode::KeyU
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.read_only()
                                && !self.solid_preview =>
                        {
                            self.begin_offset();
                        }
                        KeyCode::Comma
                            if !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview =>
                        {
                            self.start_tangent_arc();
                        }
                        KeyCode::Period
                            if !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.read_only()
                                && !self.solid_preview =>
                        {
                            self.smooth_join_key();
                        }
                        KeyCode::KeyZ
                            if self.modifiers.shift_key()
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview =>
                        {
                            self.begin_pattern(PatternTool::Linear);
                        }
                        KeyCode::Semicolon
                            if !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.solid_preview =>
                        {
                            self.begin_pattern(PatternTool::Circular);
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
                        KeyCode::KeyW | KeyCode::KeyH
                            if self.modifiers.shift_key()
                                && self.document.tool != Tool::CenterArc
                                && !self.modifiers.control_key()
                                && !self.modifiers.super_key()
                                && !self.read_only()
                                && !self.solid_preview =>
                        {
                            self.corner_key(if code == KeyCode::KeyW {
                                CornerKind::Fillet
                            } else {
                                CornerKind::Chamfer
                            });
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
                        KeyCode::BracketLeft | KeyCode::BracketRight
                            if self.solid_preview
                                && (self.modifiers.control_key() || self.modifiers.super_key()) =>
                        {
                            self.move_rollback(if code == KeyCode::BracketLeft { -1 } else { 1 });
                        }
                        KeyCode::ArrowUp | KeyCode::ArrowDown
                            if self.solid_preview
                                && (self.modifiers.control_key() || self.modifiers.super_key()) =>
                        {
                            self.move_selected_feature(if code == KeyCode::ArrowUp {
                                -1
                            } else {
                                1
                            });
                        }
                        KeyCode::KeyH
                            if self.solid_preview
                                && (self.modifiers.control_key() || self.modifiers.super_key()) =>
                        {
                            self.toggle_selected_suppressed();
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
                            self.begin_extrude("cut");
                        }
                        KeyCode::KeyJ if self.modifiers.shift_key() => {
                            self.begin_extrude("join");
                        }
                        KeyCode::KeyX
                            if !self.modifiers.control_key() && !self.modifiers.super_key() =>
                        {
                            self.begin_extrude("newBody");
                        }
                        KeyCode::KeyL if !self.modifiers.shift_key() => {
                            self.select_tool(Tool::Line)
                        }
                        KeyCode::KeyR
                            if self.selected_dimension.is_some()
                                && !self.modifiers.shift_key()
                                && !self.solid_preview =>
                        {
                            self.toggle_selected_reference();
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
                        KeyCode::KeyD => self.select_tool(Tool::Dimension),
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
                        KeyCode::Digit4 => self.sketch_on_face(),
                        KeyCode::Digit5 => self.toggle_face_context(),
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
                                if let Some(key) = self.selected_dimension.take() {
                                    self.status_message = match self.document.delete_dimension(key)
                                    {
                                        Ok(()) => "Dimension removed  |  Ctrl+S to save".into(),
                                        Err(error) => format!("Dimension: {error}"),
                                    };
                                } else {
                                    self.document.delete_selected();
                                }
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
                            if self.modifiers.alt_key() {
                                self.export_3mf();
                            } else if self.modifiers.shift_key() {
                                self.export_stl(stl::StlFormat::Ascii);
                            } else {
                                self.export_stl(stl::StlFormat::Binary);
                            }
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
                            if (self.modifiers.control_key() || self.modifiers.super_key())
                                && !self.read_only() =>
                        {
                            self.document.redo();
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
