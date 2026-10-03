use super::*;

impl App {
    pub(super) fn editable_field_at(&self, point: Point) -> Option<EditField> {
        let layout = self.layout();
        if !layout.right.contains(point.x, point.y) || self.draft_panel_open() {
            return None;
        }
        if self.solid_preview && self.selected_extension().is_some() {
            return (layout.right.y + 105.0..layout.right.y + 130.0)
                .contains(&point.y)
                .then_some(EditField::FeatureValue);
        }
        if self.solid_preview
            && self
                .project
                .as_ref()
                .and_then(ProjectFile::extrusion_previews)
                .is_some_and(|solids| !solids.is_empty())
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
        if self.document.offset_distance(shape.id()).is_some()
            && (layout.right.y + 312.0..layout.right.y + 337.0).contains(&point.y)
        {
            return Some(EditField::OffsetDistance);
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

    pub(super) fn finish_dimension_edit(&mut self) {
        let Some(mut edit) = self.editing.take() else {
            return;
        };
        edit.buffer = self.entry_to_mm(edit.field, &edit.buffer);
        self.status_message = match edit.field {
            EditField::FilletRadius | EditField::ChamferFirst | EditField::ChamferSecond => {
                self.finish_corner_edit(edit.field, &edit.buffer)
            }
            EditField::OffsetCreate => self.finish_offset_create(&edit.buffer),
            EditField::OffsetDistance => match edit.buffer.parse::<f32>() {
                Ok(value) => match self.document.set_offset_distance(value) {
                    Ok(()) => format!("Offset distance set to {value:.1} mm  |  Ctrl+S to save"),
                    Err(error) => format!("Offset error: {error}"),
                },
                Err(_) => "Offset error: enter a signed distance in mm".into(),
            },
            EditField::PatternValue => self.finish_pattern_edit(&edit.buffer),
            EditField::ExtrusionDepth => match edit.buffer.parse::<f32>() {
                Ok(depth) => match self.set_solid_depth(depth) {
                    Ok(()) => format!("Extrusion depth set to {depth:.1} mm  |  Ctrl+S to save"),
                    Err(error) => format!("Extrusion error: {error}"),
                },
                Err(_) => "Extrusion error: enter a positive depth in millimeters".into(),
            },
            EditField::FeatureValue => match edit.buffer.parse::<f32>() {
                Ok(value) => match self.set_feature_value(value) {
                    Ok(()) => format!("Feature value set to {value:.1}  |  Ctrl+S to save"),
                    Err(error) => format!("Feature error: {error}"),
                },
                Err(_) => "Feature error: enter a positive number".into(),
            },
            EditField::Dimension(field) => match self.document.selected {
                Some(id) => self.apply_dimension_text(DimensionKey::Field(id, field), &edit.buffer),
                None => "Dimension error: select geometry first".into(),
            },
            EditField::Label(key) => self.apply_dimension_text(key, &edit.buffer),
            EditField::Variable(index) => self.apply_variable_text(index, &edit.buffer),
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

    /// Rows of the dimension Properties panel (y offsets from the panel top).
    pub(super) const DIMENSION_VALUE_ROW: f32 = 105.0;
    pub(super) const DIMENSION_MODE_ROW: f32 = 175.0;

    fn paint_dimension_panel(&self, painter: &mut Painter, layout: Layout, key: DimensionKey) {
        let x = layout.right.x + 18.0;
        let kind = match key {
            DimensionKey::Field(_, field) => match field {
                DimensionField::LineLength => "LENGTH",
                DimensionField::RectangleWidth => "WIDTH",
                DimensionField::RectangleHeight => "HEIGHT",
                DimensionField::CircleDiameter => "DIAMETER",
                DimensionField::CurveRadius => "RADIUS",
                DimensionField::ArcSweep => "ARC SWEEP",
                DimensionField::Rotation => "ROTATION",
            },
            DimensionKey::Angle(_) => "ANGLE",
            DimensionKey::Point(id) => match self
                .document
                .dims
                .point_dimensions
                .get(&id)
                .and_then(|dimension| dimension.axis)
            {
                Some(model::PointAxis::X) => "X DISTANCE",
                Some(model::PointAxis::Y) => "Y DISTANCE",
                None => "DISTANCE",
            },
        };
        painter.text(x, layout.right.y + 66.0, kind, 1.7, ACCENT);
        let reference = self.document.is_reference(key);
        painter.text(
            x,
            layout.right.y + 90.0,
            if reference {
                "REFERENCE DIMENSION"
            } else {
                "DRIVING DIMENSION"
            },
            1.0,
            MUTED,
        );
        let angle = Document::dimension_is_angle(key);
        let unit = if angle { "deg" } else { self.units().name() };
        let value = self
            .document
            .displayed_dimension_value(key)
            .map(|value| {
                if angle {
                    model::format_value(value)
                } else {
                    model::format_value(self.units().mm_to_display(value))
                }
            })
            .unwrap_or_default();
        painter.text(
            x,
            layout.right.y + Self::DIMENSION_VALUE_ROW + 8.0,
            &if reference {
                format!("VALUE   ({value}) {unit}")
            } else {
                format!("VALUE   {value} {unit}")
            },
            1.25,
            TEXT,
        );
        let button = Rect {
            x: layout.right.x + 10.0,
            y: layout.right.y + Self::DIMENSION_MODE_ROW,
            w: layout.right.w - 20.0,
            h: 28.0,
        };
        painter.rect(button, PANEL_LIGHT);
        painter.outline(button, 1.0, BORDER);
        painter.text(
            button.x + 8.0,
            button.y + 9.0,
            if reference {
                "[R] MAKE DRIVING"
            } else {
                "[R] MAKE REFERENCE"
            },
            1.1,
            TEXT,
        );
        painter.text(
            x,
            layout.right.y + 225.0,
            "CLICK VALUE OR LABEL TO EDIT",
            1.0,
            MUTED,
        );
        painter.text(
            x,
            layout.right.y + 245.0,
            "DEL  REMOVE DIMENSION",
            1.0,
            MUTED,
        );
    }

    /// Applies a typed dimension value or expression; returns the status.
    fn apply_dimension_text(&mut self, key: DimensionKey, text: &str) -> String {
        match self.document.set_dimension_expression(key, text) {
            Ok(value) => format!(
                "Dimension set to {}  |  Ctrl+S to save",
                model::format_value(value)
            ),
            Err(error) => format!("Dimension error: {error}"),
        }
    }

    /// Applies `name = expression` to a sketch variable (`None` adds one);
    /// an empty expression deletes it.
    fn apply_variable_text(&mut self, index: Option<usize>, text: &str) -> String {
        let (name, expression) = match text.split_once('=') {
            Some((name, expression)) => (name.trim(), expression.trim()),
            None => match index.and_then(|index| self.document.dims.variables.get(index)) {
                // Only a new expression typed for an existing variable.
                Some(variable) => (variable.name.as_str(), text.trim()),
                None => return "Variable error: type name = expression".into(),
            },
        };
        let name = name.to_string();
        let result = match index {
            Some(index) if expression.is_empty() => self.document.delete_variable(index),
            _ => self.document.define_variable(index, &name, expression),
        };
        match result {
            Ok(()) if expression.is_empty() => format!("Removed #{name}  |  Ctrl+S to save"),
            Ok(()) => format!("#{name} = {expression}  |  Ctrl+S to save"),
            Err(error) => format!("Variable error: {error}"),
        }
    }

    /// Variables section of the Properties panel when nothing is selected:
    /// one row per variable, then an add button.
    pub(super) fn variable_row(layout: Layout, index: usize) -> Rect {
        Rect {
            x: layout.right.x + 10.0,
            y: layout.right.y + 175.0 + index as f32 * 30.0,
            w: layout.right.w - 20.0,
            h: 25.0,
        }
    }

    fn paint_variables(&self, painter: &mut Painter, layout: Layout) {
        let x = layout.right.x + 18.0;
        painter.text(x, layout.right.y + 150.0, "SKETCH VARIABLES", 1.2, ACCENT);
        let values =
            sketch_value::evaluate_variables(&self.document.dims.variables).unwrap_or_default();
        let count = self.document.dims.variables.len();
        for index in 0..=count {
            let row = Self::variable_row(layout, index);
            if row.y + row.h >= layout.bottom.y {
                break;
            }
            let editing = self.editing.as_ref().filter(|edit| {
                matches!(edit.field, EditField::Variable(target)
                    if target == (index < count).then_some(index))
            });
            painter.rect(
                row,
                if editing.is_some() {
                    ACTIVE_ROW
                } else {
                    PANEL_LIGHT
                },
            );
            painter.outline(row, 1.0, if editing.is_some() { ACCENT } else { BORDER });
            let text = if let Some(edit) = editing {
                format!("> {}_", edit.buffer)
            } else if let Some(variable) = self.document.dims.variables.get(index) {
                let value = values
                    .get(&variable.name)
                    .map_or(String::new(), |quantity| {
                        let unit = match quantity.kind {
                            sketch_value::ValueKind::Length => " mm",
                            sketch_value::ValueKind::Angle => " deg",
                            sketch_value::ValueKind::Scalar => "",
                        };
                        format!("  ({}{unit})", model::format_value(quantity.value as f32))
                    });
                format!("#{} = {}{value}", variable.name, variable.expression)
            } else {
                "+ ADD VARIABLE (name = expr)".into()
            };
            painter.text(row.x + 8.0, row.y + 8.0, &text, 1.0, TEXT);
        }
    }

    /// Toggles driving / reference for the selected dimension.
    pub(super) fn toggle_selected_reference(&mut self) {
        let Some(key) = self.selected_dimension else {
            return;
        };
        if self.read_only() {
            self.status_message = "READ ONLY: this sketch has unsupported native features".into();
            return;
        }
        self.editing = None;
        self.status_message = match self.document.toggle_reference(key) {
            Ok(true) => "Reference dimension  |  Ctrl+S to save".into(),
            Ok(false) => "Driving dimension  |  Ctrl+S to save".into(),
            Err(error) => format!("Dimension: {error}"),
        };
    }

    pub(super) fn paint_panels(&self, painter: &mut Painter, layout: Layout) {
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
        let analysis = (!self.solid_preview).then(|| self.sketch_analysis().clone());
        if let Some(analysis) = &analysis {
            let label = analysis.label();
            let color = if !analysis.conflicting.is_empty() {
                CONFLICT
            } else if !analysis.redundant.is_empty() {
                REDUNDANT
            } else if analysis.dof == 0 {
                FULLY_CONSTRAINED
            } else {
                MUTED
            };
            painter.text(
                layout.top.w - 20.0 - label.chars().count() as f32 * 8.0 * 1.6,
                15.0,
                &label,
                1.6,
                color,
            );
        }
        if self.solid_preview {
            for (perspective, label) in [(true, "PERSPECTIVE"), (false, "ORTHO")] {
                let button = layout.projection_button(perspective);
                let active = self.solid_view.perspective == perspective;
                painter.rect(button, if active { ACTIVE } else { PANEL_LIGHT });
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
                    ACTIVE
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
            paint_glyph(painter, button.x + 4.0, button.y + 8.0, index, ACCENT);
            painter.text(button.x + 22.0, button.y + 12.0, label, 1.0, TEXT);
        }
        for (index, label) in ["TOP [1]", "FRONT [2]", "RIGHT [3]"]
            .into_iter()
            .enumerate()
        {
            let button = layout.tool_button(index + 5);
            painter.rect(button, PANEL_LIGHT);
            painter.outline(button, 1.0, BORDER);
            paint_glyph(painter, button.x + 4.0, button.y + 8.0, index + 5, ACCENT);
            painter.text(button.x + 22.0, button.y + 12.0, label, 1.0, TEXT);
        }
        let more = layout.tool_button(9);
        painter.rect(
            more,
            if self.more_open || matches!(self.document.tool, Tool::Point | Tool::Polygon) {
                ACTIVE
            } else {
                PANEL_LIGHT
            },
        );
        painter.outline(more, 1.0, if self.more_open { ACCENT } else { BORDER });
        paint_glyph(painter, more.x + 4.0, more.y + 8.0, 9, ACCENT);
        painter.text(more.x + 22.0, more.y + 12.0, "MORE [M]", 1.0, TEXT);
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
                if !Self::panel_row_shown(layout, row) {
                    continue;
                }
                let active = *part_index == project.active_part_index();
                if active {
                    painter.rect(row, ACTIVE_ROW);
                }
                let kind = if project.part_name_at(*part_index).is_some() {
                    "PART"
                } else {
                    "ASM"
                };
                let name = project.item_name_at(*part_index).unwrap_or("Part");
                let display_name = if name.chars().count() > 13 {
                    format!("{}...", name.chars().take(13).collect::<String>())
                } else {
                    name.to_string()
                };
                painter.text(
                    row.x + 9.0,
                    row.y + 9.0,
                    &format!(
                        "{} {kind} / {}",
                        if active { ">" } else { "v" },
                        display_name
                    ),
                    1.0,
                    if active { TEXT } else { MUTED },
                );
            }
        }
        for index in 0..self.sketch_count() {
            let row = layout.sketch_row(index, part_rows.len());
            if !Self::panel_row_shown(layout, row) {
                continue;
            }
            let active = self
                .project
                .as_ref()
                .is_none_or(|project| project.active_sketch() == index);
            if active {
                painter.rect(row, ACTIVE_ROW);
            }
            let name = self
                .project
                .as_ref()
                .and_then(|project| project.sketch_name(index))
                .unwrap_or("Sketch 1");
            let plane = self
                .project
                .as_ref()
                .and_then(|project| {
                    project
                        .sketch_plane(index)
                        .map(SketchPlane::name)
                        .or(project.sketch_face(index).map(|_| "FACE"))
                })
                .unwrap_or(if self.project.is_some() {
                    "OTHER"
                } else {
                    "TOP"
                });
            painter.text(
                row.x + 9.0,
                row.y + 9.0,
                &clip_text(
                    &format!("{} {name} / {plane}", if active { ">" } else { "v" }),
                    ((row.w - 18.0) / 8.8) as usize,
                    false,
                ),
                1.1,
                if active { TEXT } else { MUTED },
            );
        }
        self.paint_feature_tree(painter, layout);
        let tree_rows = self.tree_row_count();
        let selected_ids = self.document.selected_ids();
        for (index, shape) in self.document.shapes.iter().enumerate() {
            let row = layout.row(index + tree_rows, part_rows.len(), self.sketch_count());
            if row.y + row.h >= layout.bottom.y {
                break;
            }
            if !Self::panel_row_shown(layout, row) {
                continue;
            }
            if selected_ids.contains(&shape.id()) {
                painter.rect(row, ACTIVE_ROW);
            }
            painter.text(
                row.x + 10.0,
                row.y + 9.0,
                &format!(
                    "{} {}{}{}{}{}{}{}{}{}{}",
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
                        .unwrap_or(""),
                    if self.document.has_symmetry(shape.id()) {
                        " [SYM]"
                    } else if self.document.has_offset(shape.id()) {
                        " [OFS]"
                    } else {
                        ""
                    },
                    if self.document.smooth_join_member(shape.id()) {
                        " [TJ]"
                    } else {
                        ""
                    },
                    if self.document.pattern_of(shape.id()).is_some() {
                        " [PAT]"
                    } else {
                        ""
                    },
                    analysis.as_ref().map_or("", |analysis| {
                        if analysis.conflicting.contains(&shape.id()) {
                            " [!]"
                        } else if analysis.redundant.contains(&shape.id()) {
                            " [R]"
                        } else {
                            ""
                        }
                    })
                ),
                1.3,
                match analysis.as_ref() {
                    Some(analysis) if analysis.conflicting.contains(&shape.id()) => CONFLICT,
                    Some(analysis) if analysis.redundant.contains(&shape.id()) => REDUNDANT,
                    _ if self.document.construction.contains(&shape.id()) => MUTED,
                    _ => TEXT,
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
        if self.draft_panel_open() {
            self.paint_draft_panel(painter, layout);
        } else if self.paint_topology_panel(painter, layout)
            || self.paint_feature_panel(painter, layout)
        {
        } else if let Some(solids) = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .filter(|solids| self.solid_preview && !solids.is_empty())
        {
            let selected = self.selected_solid.min(solids.len() - 1);
            let options = self
                .project
                .as_ref()
                .and_then(|project| project.extrusion_options_at(selected));
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
                &format!("DEPTH   {}", self.len(solids[selected].depth)),
                1.25,
                TEXT,
            );
            if let Some(options) = &options {
                let target = match &options.end {
                    project::EndCondition::UpToBody(target) => Some(target.as_str()),
                    _ => None,
                };
                for (row, line) in [
                    Some(format!("END     {}   [E]", options.end.label())),
                    Some(format!(
                        "FLIP    {}   [F]",
                        if options.flip { "ON" } else { "OFF" }
                    )),
                    target.map(|target| {
                        format!(
                            "TARGET  {target}   {}",
                            if self.body_pick { "PICK..." } else { "[U]" }
                        )
                    }),
                ]
                .into_iter()
                .enumerate()
                {
                    if let Some(line) = line {
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 143.0 + row as f32 * 30.0,
                            &line,
                            1.25,
                            TEXT,
                        );
                    }
                }
            }
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 245.0,
                if solids.len() == 1 {
                    "CLICK A ROW TO EDIT"
                } else {
                    "[ ]  SELECT FEATURE"
                },
                1.0,
                MUTED,
            );
            if solids.len() > 1 {
                painter.text(
                    layout.right.x + 18.0,
                    layout.right.y + 265.0,
                    &format!("FEATURE {} / {}", selected + 1, solids.len()),
                    1.0,
                    MUTED,
                );
            }
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 285.0,
                "TAB  Sketch view",
                1.2,
                MUTED,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 320.0,
                "CTRL+E  EXPORT STL",
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 340.0,
                "CTRL+ALT+E  EXPORT 3MF",
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 360.0,
                "DEL  REMOVE FEATURE",
                1.2,
                MUTED,
            );
        } else if let Some(key) = self
            .selected_dimension
            .filter(|key| self.document.dimension_keys().contains(key))
        {
            self.paint_dimension_panel(painter, layout, key);
        } else if let Some(shape) = self
            .document
            .shapes
            .iter()
            .find(|shape| Some(shape.id()) == self.document.selected)
        {
            let count = self.document.selection_count();
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 66.0,
                &if count > 1 {
                    format!("{}  ({count} selected)", shape.name())
                } else {
                    shape.name().to_string()
                },
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
                        format!("LENGTH  {}", self.len(a.distance(*b))),
                        format!("START X {}", self.len(a.x)),
                        format!("START Y {}", self.len(a.y)),
                        format!("END X   {}", self.len(b.x)),
                        format!("END Y   {}", self.len(b.y)),
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
                        format!("WIDTH   {}", self.len((a.x - b.x).abs())),
                        format!("HEIGHT  {}", self.len((a.y - b.y).abs())),
                        format!("X       {}", self.len(a.x.min(b.x))),
                        format!("Y       {}", self.len(a.y.min(b.y))),
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
                        format!("WIDTH   {}", self.len(*width)),
                        format!("HEIGHT  {}", self.len(*height)),
                        format!("ANGLE   {:.1} deg", rotation),
                        format!("CENTER X {}", self.len(center.x)),
                        format!("CENTER Y {}", self.len(center.y)),
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
                        format!("DIAMETER {}", self.len(radius * 2.0)),
                        format!("CENTER X {}", self.len(center.x)),
                        format!("CENTER Y {}", self.len(center.y)),
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
                        format!("WIDTH   {}", self.len(rx * 2.0)),
                        format!("HEIGHT  {}", self.len(ry * 2.0)),
                        format!("ANGLE   {:.1} deg", rotation),
                        format!("CENTER X {}", self.len(center.x)),
                        format!("CENTER Y {}", self.len(center.y)),
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
                        format!("WIDTH   {}", self.len(rx * 2.0)),
                        format!("HEIGHT  {}", self.len(ry * 2.0)),
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
                    if let Some(index) = self.document.selected_spline_point
                        && let Some(point) = points.get(index)
                    {
                        let fixed = self
                            .document
                            .fixed_anchors
                            .contains_key(&(shape.id(), index));
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 193.0,
                            &format!("POINT {} X {}", index + 1, self.len(point.x)),
                            1.1,
                            if fixed { MUTED } else { ACCENT },
                        );
                        painter.text(
                            layout.right.x + 18.0,
                            layout.right.y + 223.0,
                            &format!("POINT {} Y {}", index + 1, self.len(point.y)),
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
                Shape::Arc {
                    center,
                    radius,
                    start_angle,
                    sweep,
                    ..
                } => {
                    for (index, line) in [
                        format!("RADIUS  {}", self.len(*radius)),
                        format!("SWEEP   {:.1} deg", sweep.to_degrees()),
                        format!("START   {:.1} deg", start_angle.to_degrees()),
                        format!("CENTER X {}", self.len(center.x)),
                        format!("CENTER Y {}", self.len(center.y)),
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
                    for (index, line) in [
                        format!("X {}", self.len(at.x)),
                        format!("Y {}", self.len(at.y)),
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
                        format!("RADIUS   {}", self.len(*radius)),
                        format!("ANGLE    {:.1} deg", rotation),
                        format!("CENTER X {}", self.len(center.x)),
                        format!("CENTER Y {}", self.len(center.y)),
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
                        format!("WIDTH    {}", self.len(*width)),
                        format!("DISTANCE {}", self.len(from.distance(*to))),
                        format!("FROM X   {}", self.len(from.x)),
                        format!("FROM Y   {}", self.len(from.y)),
                        format!("TO X     {}", self.len(to.x)),
                        format!("TO Y     {}", self.len(to.y)),
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
            if let Some(value) = self.document.offset_distance(shape.id()) {
                painter.text(
                    layout.right.x + 18.0,
                    layout.right.y + 325.0,
                    &format!("OFFSET {}", self.len(value).to_uppercase()),
                    1.0,
                    ACCENT,
                );
            } else if let Some((_, value)) = self.document.radius_differences.get(&shape.id()) {
                painter.text(
                    layout.right.x + 18.0,
                    layout.right.y + 325.0,
                    &format!("DIFF {}", self.len(*value).to_uppercase()),
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
                &format!("MOVE X   {}", self.len(self.move_offset.x)),
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + 18.0,
                layout.right.y + 402.0,
                &format!("MOVE Y   {}", self.len(self.move_offset.y)),
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
                &format!("CX {}", self.len(self.transform_center.x)),
                1.2,
                TEXT,
            );
            painter.text(
                layout.right.x + layout.right.w * 0.5 + 8.0,
                layout.right.y + 562.0,
                &format!("CY {}", self.len(self.transform_center.y)),
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
            if !self.solid_preview {
                self.paint_variables(painter, layout);
            }
        }
        if let Some(edit) = &self.editing
            && !matches!(edit.field, EditField::Label(_) | EditField::Variable(_))
        {
            let row = match edit.field {
                EditField::ExtrusionDepth | EditField::FeatureValue => 0,
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
                | EditField::RadialDifference
                | EditField::FilletRadius
                | EditField::OffsetCreate
                | EditField::OffsetDistance
                | EditField::PatternValue
                | EditField::ChamferFirst
                | EditField::ChamferSecond
                | EditField::Label(_)
                | EditField::Variable(_) => 0,
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
                EditField::RadialDifference | EditField::OffsetDistance => layout.right.y + 312.0,
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
            painter.rect(field, ACTIVE_ROW);
            painter.outline(field, 1.0, ACCENT);
            painter.text(
                field.x + 8.0,
                field.y + 7.0,
                &if matches!(edit.field, EditField::OffsetCreate) {
                    format!("OFFSET > {}_ {}", edit.buffer, self.units().name())
                } else if matches!(edit.field, EditField::PatternValue) {
                    self.pattern_prompt_text(&edit.buffer)
                } else if matches!(edit.field, EditField::FilletRadius) {
                    format!("FILLET R > {}_ {}", edit.buffer, self.units().name())
                } else if matches!(edit.field, EditField::ChamferFirst) {
                    format!("CHAMFER D1 > {}_ {}", edit.buffer, self.units().name())
                } else if matches!(edit.field, EditField::ChamferSecond) {
                    if self.corner_values.mode == ChamferMode::DistanceAngle {
                        format!("CHAMFER ANGLE > {}_ deg", edit.buffer)
                    } else {
                        format!("CHAMFER D2 > {}_ {}", edit.buffer, self.units().name())
                    }
                } else if matches!(edit.field, EditField::PolygonSides) {
                    format!("> {}_ sides", edit.buffer)
                } else if matches!(edit.field, EditField::ScaleFactor) {
                    format!("> {}_ factor", edit.buffer)
                } else if matches!(edit.field, EditField::RotateAngle) {
                    format!("> {}_ deg", edit.buffer)
                } else if center_half {
                    format!("> {}_", edit.buffer)
                } else {
                    format!("> {}_ {}", edit.buffer, self.units().name())
                },
                1.25,
                TEXT,
            );
        }
        if let Some(edit) = &self.editing
            && matches!(
                edit.field,
                EditField::ChamferFirst | EditField::ChamferSecond
            )
        {
            let row = Rect {
                x: layout.right.x + 10.0,
                y: layout.right.y + 135.0,
                w: layout.right.w - 20.0,
                h: 25.0,
            };
            painter.rect(row, PANEL_LIGHT);
            painter.outline(row, 1.0, BORDER);
            painter.text(
                row.x + 8.0,
                row.y + 7.0,
                match self.corner_values.mode {
                    ChamferMode::TwoDistances => "MODE: TWO SETBACKS [TAB]",
                    ChamferMode::DistanceAngle => "MODE: DISTANCE-ANGLE [TAB]",
                },
                1.1,
                TEXT,
            );
        }
        let tool_status = match self.document.tool {
            Tool::Select => {
                "Select: click, Shift+click toggle, drag box (right: enclosed, left: crossing)"
            }
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
            Tool::Dimension => {
                "Dimension: click an entity, or two points (Shift: X, Ctrl: Y; origin for absolute)"
            }
            Tool::Polygon => "Polygon: click center, then vertex  |  Esc: cancel",
            Tool::Slot => "Slot: click two centers, then side wall  |  Esc: cancel",
            Tool::TangentArc => {
                "Tangent arc: click a line, arc or spline endpoint, then the arc end"
            }
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
        let shown = self.shown_status();
        let status = if let Some(name_status) = &name_status {
            name_status.as_str()
        } else if shown.is_empty() {
            tool_status
        } else {
            &shown
        };
        // Keep the message left of the status fields (cursor, tool, snap);
        // a long message narrows the fields to get more room.
        let info = layout.status_info_for(status.chars().count(), self.solid_preview);
        let toggles = if self.solid_preview {
            0.0
        } else {
            STATUS_TOGGLES
        };
        let room = ((info.x - toggles - 25.0) / 9.6).max(0.0) as usize;
        let status: String = if status.chars().count() > room {
            status
                .chars()
                .take(room.saturating_sub(3))
                .chain("...".chars())
                .collect()
        } else {
            status.to_string()
        };
        painter.text(15.0, layout.bottom.y + 10.0, &status, 1.2, MUTED);
        self.paint_status_info(painter, layout, info);
        if !self.solid_preview {
            let filter = self.selection_filter;
            for (index, (label, on)) in [
                ("GEOMETRY", filter.geometry),
                ("CONSTR", filter.construction),
                ("POINTS", filter.points),
            ]
            .into_iter()
            .enumerate()
            {
                let button = layout.filter_button(index);
                painter.rect(button, if on { ACTIVE } else { PANEL_LIGHT });
                painter.outline(button, 1.0, if on { ACCENT } else { BORDER });
                painter.text(
                    button.x + 8.0,
                    button.y + 7.0,
                    label,
                    1.1,
                    if on { TEXT } else { MUTED },
                );
            }
        }
        if !self.solid_preview {
            painter.text(
                info.x - 96.0,
                layout.bottom.y + 10.0,
                if self.auto_constraints {
                    "AUTO"
                } else {
                    "auto"
                },
                1.1,
                if self.auto_constraints { ACCENT } else { MUTED },
            );
            painter.text(
                info.x - 50.0,
                layout.bottom.y + 10.0,
                if self.show_glyphs { "GLY" } else { "gly" },
                1.1,
                if self.show_glyphs { ACCENT } else { MUTED },
            );
        }
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
