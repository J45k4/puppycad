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
