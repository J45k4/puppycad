//! Tangent arc tool (`,`, MORE → TANGENT ARC) and the smooth join
//! gesture (`.`).

use super::*;

impl App {
    pub(super) fn start_tangent_arc(&mut self) {
        if self.read_only() {
            self.status_message = "READ ONLY: this sketch has unsupported native features".into();
            return;
        }
        self.select_tool(Tool::TangentArc);
        self.status_message =
            "Tangent arc: click a line, arc or spline endpoint  |  Esc: cancel".into();
    }

    /// First click picks the source endpoint (endpoints only, 8 px), the
    /// second click places the arc end (`world` is the snapped cursor).
    pub(super) fn tangent_arc_click(&mut self, raw: Point, world: Point) {
        let source = self
            .document
            .tangent_source
            .filter(|_| self.document.draft_start.is_some());
        let Some(source) = source else {
            let Some(end) = self.document.curve_endpoint_at(raw) else {
                self.status_message =
                    "Tangent arc: click a line, arc or open spline endpoint first".into();
                return;
            };
            match self.document.tangent_arc_source(end) {
                Ok(at) => {
                    self.document.tangent_source = Some(end);
                    self.document.draft_start = Some(at);
                    self.status_message = "Tangent arc: click the arc end  |  Esc: cancel".into();
                }
                Err(error) => self.status_message = format!("Tangent arc: {error}"),
            }
            return;
        };
        self.status_message = match self.document.add_tangent_arc(source, world) {
            Ok(_) => "Tangent arc added  |  Ctrl+Z to undo  |  Ctrl+S to save".into(),
            Err(error) => format!("Tangent arc: {error}"),
        };
    }

    /// `.`: smooth join the selected curve and the hovered one at their
    /// nearest endpoints, or remove the join between them.
    pub(super) fn smooth_join_key(&mut self) {
        let world = self
            .document
            .screen_to_world(self.cursor, self.viewport_origin());
        let selected = self.document.selected;
        let hovered = self
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
                            | Shape::Spline { closed: false, .. }
                    )
                    && shape.hit(world, 7.0 / self.document.zoom.max(0.2))
            })
            .map(Shape::id);
        self.status_message = match hovered {
            Some(hovered) => match self.document.toggle_smooth_join_with(hovered) {
                Ok(true) => "Smooth join set  |  Ctrl+Z to undo  |  Ctrl+S to save".into(),
                Ok(false) => "Smooth join removed".into(),
                Err(error) => format!("Smooth join: {error}"),
            },
            None => "Smooth join: select a curve, hover another near its endpoint".into(),
        };
    }
}
