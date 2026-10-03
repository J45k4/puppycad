//! Fillet and chamfer gestures: pick two lines meeting at a corner, then
//! type the radius or chamfer values in the Properties prompt.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CornerKind {
    Fillet,
    Chamfer,
}

impl CornerKind {
    fn name(self) -> &'static str {
        match self {
            Self::Fillet => "Fillet",
            Self::Chamfer => "Chamfer",
        }
    }
}

/// Values the fillet/chamfer prompts start from (and remember).
#[derive(Clone, Copy, Debug)]
pub(super) struct CornerValues {
    pub(super) radius: f32,
    pub(super) mode: ChamferMode,
    pub(super) first: f32,
    pub(super) second: f32,
    pub(super) angle: f32,
}

impl Default for CornerValues {
    fn default() -> Self {
        Self {
            radius: 5.0,
            mode: ChamferMode::TwoDistances,
            first: 5.0,
            second: 5.0,
            angle: 45.0,
        }
    }
}

impl App {
    fn line_at_cursor(&self, exclude: Option<u32>) -> Option<u32> {
        let world = self
            .document
            .screen_to_world(self.cursor, self.viewport_origin());
        self.document
            .shapes
            .iter()
            .rev()
            .find(|shape| {
                Some(shape.id()) != exclude
                    && matches!(shape, Shape::Line { .. })
                    && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
            })
            .map(Shape::id)
    }

    fn selected_line(&self) -> Option<u32> {
        let id = self.document.selected?;
        self.document
            .shapes
            .iter()
            .any(|shape| shape.id() == id && matches!(shape, Shape::Line { .. }))
            .then_some(id)
    }

    /// Shift+W / Shift+H: corner from the selected line and the hovered one.
    /// With a chamfer line selected and nothing hovered, Shift+H edits it.
    pub(super) fn corner_key(&mut self, kind: CornerKind) {
        let selected = self.selected_line();
        let hovered = self.line_at_cursor(selected);
        match (selected, hovered) {
            (Some(first), Some(second)) => self.begin_corner(kind, Some((first, second))),
            (Some(line), None)
                if kind == CornerKind::Chamfer && self.document.chamfers.contains_key(&line) =>
            {
                let spec = self.document.chamfers[&line];
                self.corner_values.mode = spec.mode;
                self.corner_values.first = spec.first;
                if spec.mode == ChamferMode::DistanceAngle {
                    self.corner_values.angle = spec.second;
                } else {
                    self.corner_values.second = spec.second;
                }
                self.begin_corner(kind, None);
            }
            _ => {
                self.status_message = format!(
                    "{}: select a line, hover a second line at the corner",
                    kind.name()
                );
            }
        }
    }

    /// MORE menu entry: the next viewport clicks pick the two lines.
    pub(super) fn start_corner_pick(&mut self, kind: CornerKind) {
        self.select_tool(Tool::Select);
        self.corner_pick = Some(kind);
        self.status_message = format!(
            "{}: click two lines meeting at a corner  |  Esc: cancel",
            kind.name()
        );
    }

    /// Viewport click while picking corner lines. Returns true when handled.
    pub(super) fn corner_pick_click(&mut self) -> bool {
        let Some(kind) = self.corner_pick else {
            return false;
        };
        let selected = self.selected_line();
        let Some(hit) = self.line_at_cursor(selected) else {
            self.status_message = format!("{}: click a line", kind.name());
            return true;
        };
        match selected {
            Some(first) => {
                self.corner_pick = None;
                self.begin_corner(kind, Some((first, hit)));
            }
            None => {
                self.document.selected = Some(hit);
                self.status_message = format!("{}: click the second line", kind.name());
            }
        }
        true
    }

    fn begin_corner(&mut self, kind: CornerKind, pair: Option<(u32, u32)>) {
        self.corner_pair = pair;
        self.editing = Some(DimensionEdit {
            field: match kind {
                CornerKind::Fillet => EditField::FilletRadius,
                CornerKind::Chamfer => EditField::ChamferFirst,
            },
            buffer: String::new(),
        });
        self.status_message = match kind {
            CornerKind::Fillet => format!(
                "Fillet radius in mm (Enter keeps {:.1})  |  Esc to cancel",
                self.corner_values.radius
            ),
            CornerKind::Chamfer => format!(
                "Chamfer first setback in mm (Enter keeps {:.1})  |  Tab: mode  Esc: cancel",
                self.corner_values.first
            ),
        };
    }

    pub(super) fn toggle_chamfer_mode(&mut self) {
        self.corner_values.mode = match self.corner_values.mode {
            ChamferMode::TwoDistances => ChamferMode::DistanceAngle,
            ChamferMode::DistanceAngle => ChamferMode::TwoDistances,
        };
    }

    /// Finish one corner prompt; returns the status message.
    pub(super) fn finish_corner_edit(&mut self, field: EditField, buffer: &str) -> String {
        let fallback = match field {
            EditField::FilletRadius => self.corner_values.radius,
            EditField::ChamferFirst => self.corner_values.first,
            _ if self.corner_values.mode == ChamferMode::DistanceAngle => self.corner_values.angle,
            _ => self.corner_values.second,
        };
        let value = if buffer.is_empty() {
            fallback
        } else {
            match buffer.parse::<f32>() {
                Ok(value) if value.is_finite() && value > 0.0 => value,
                _ => {
                    self.corner_pair = None;
                    return "Corner error: enter a positive number".into();
                }
            }
        };
        match field {
            EditField::FilletRadius => {
                self.corner_values.radius = value;
                let Some((first, second)) = self.corner_pair.take() else {
                    return "Fillet: select two lines".into();
                };
                match self.document.fillet_lines(first, second, value) {
                    Ok(_) => format!("Fillet R{value:.1} mm  |  Ctrl+Z to undo  |  Ctrl+S to save"),
                    Err(error) => format!("Fillet: {error}"),
                }
            }
            EditField::ChamferFirst => {
                self.corner_values.first = value;
                let angle = self.corner_values.mode == ChamferMode::DistanceAngle;
                self.editing = Some(DimensionEdit {
                    field: EditField::ChamferSecond,
                    buffer: String::new(),
                });
                if angle {
                    format!(
                        "Chamfer angle in degrees (Enter keeps {:.1})  |  Esc to cancel",
                        self.corner_values.angle
                    )
                } else {
                    format!(
                        "Chamfer second setback in mm (Enter keeps {:.1})  |  Esc to cancel",
                        self.corner_values.second
                    )
                }
            }
            _ => {
                let mode = self.corner_values.mode;
                if mode == ChamferMode::DistanceAngle {
                    self.corner_values.angle = value;
                } else {
                    self.corner_values.second = value;
                }
                let first = self.corner_values.first;
                let result = match self.corner_pair.take() {
                    Some((a, b)) => self
                        .document
                        .chamfer_lines(a, b, mode, first, value)
                        .map(|_| ()),
                    None => self.document.set_chamfer(mode, first, value),
                };
                match result {
                    Ok(()) => "Chamfer set  |  Ctrl+Z to undo  |  Ctrl+S to save".into(),
                    Err(error) => format!("Chamfer: {error}"),
                }
            }
        }
    }
}
