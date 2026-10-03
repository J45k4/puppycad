//! Revolve (key 6), loft (key 7) and edge chamfer/fillet (keys 8 / 9) in
//! the native app, plus selecting and editing those features.

use super::*;
use project::{EdgeRef, FinishKind, SolidEdge, Tri};

/// Angle committed when Enter is pressed on an empty revolve prompt.
pub(super) const DEFAULT_ANGLE: f32 = 360.0;
/// Size committed when Enter is pressed on an empty edge finish prompt.
pub(super) const DEFAULT_FINISH: f32 = 2.0;
/// Screen distance within which an edge is hovered.
const EDGE_PICK_PIXELS: f32 = 10.0;

/// Key 6: pick an axis in the sketch, then type the angle.
pub(super) struct RevolveDraft {
    pub(super) axis: Option<(Point, Point)>,
    pub(super) buffer: String,
    pub(super) operation: &'static str,
    pub(super) ghost: Result<Vec<Tri>, String>,
}

impl RevolveDraft {
    pub(super) fn angle(&self) -> Option<f32> {
        if self.buffer.is_empty() {
            return Some(DEFAULT_ANGLE);
        }
        self.buffer
            .parse::<f32>()
            .ok()
            .filter(|angle| angle.is_finite() && *angle > 0.0 && *angle <= 360.0)
    }
}

/// Keys 8 / 9: edges clicked so far and the typed size.
pub(super) struct EdgePick {
    pub(super) kind: FinishKind,
    pub(super) edges: Vec<EdgeRef>,
    pub(super) buffer: String,
}

impl EdgePick {
    pub(super) fn size(&self) -> Option<f32> {
        if self.buffer.is_empty() {
            return Some(DEFAULT_FINISH);
        }
        self.buffer
            .parse::<f32>()
            .ok()
            .filter(|size| size.is_finite() && *size > 0.0)
    }

    fn label(&self) -> &'static str {
        match self.kind {
            FinishKind::Chamfer => "DISTANCE",
            FinishKind::Fillet => "RADIUS",
        }
    }
}

fn next_operation(operation: &'static str) -> &'static str {
    match operation {
        "newBody" => "cut",
        "cut" => "join",
        _ => "newBody",
    }
}

fn segment_distance(p: Point, a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dy * dy;
    let t = if length < 1e-9 {
        0.0
    } else {
        (((p.x - a[0]) * dx + (p.y - a[1]) * dy) / length).clamp(0.0, 1.0)
    };
    ((p.x - a[0] - t * dx).powi(2) + (p.y - a[1] - t * dy).powi(2)).sqrt()
}

impl App {
    /// Key 6 / MORE → REVOLVE: starts the axis pick on the active sketch.
    pub(super) fn begin_revolve(&mut self) {
        self.document.finish_drag();
        self.more_open = false;
        self.editing = None;
        if self.project.is_none() {
            self.status_message = "Open a .pcad project to revolve".into();
            return;
        }
        if self.solid_preview {
            self.status_message = "Revolve: Tab to the sketch, select a profile, press 6".into();
            return;
        }
        let Some(profiles) = profiles_from_document(&self.document) else {
            self.status_message = "Revolve: the sketch has no closed profile".into();
            return;
        };
        self.selected_profile = self.selected_profile.min(profiles.len().saturating_sub(1));
        self.revolve_draft = Some(RevolveDraft {
            axis: None,
            buffer: String::new(),
            operation: "newBody",
            ghost: Err("pick an axis".into()),
        });
        self.status_message =
            "Revolve: click an axis line, or X / Y for the sketch axes  |  Esc cancel".into();
    }

    fn set_revolve_axis(&mut self, axis: (Point, Point)) {
        let Some(mut draft) = self.revolve_draft.take() else {
            return;
        };
        draft.axis = Some(axis);
        self.refresh_revolve_ghost(&mut draft);
        self.solid_preview = true;
        self.status_message = Self::revolve_hint(&draft);
        self.revolve_draft = Some(draft);
    }

    fn revolve_hint(draft: &RevolveDraft) -> String {
        match &draft.ghost {
            Ok(_) => format!(
                "Revolve {}: type angle, Enter commits (empty = {DEFAULT_ANGLE:.0} deg)  |  O operation  |  Esc cancel",
                draft.operation
            ),
            Err(error) => format!("Revolve preview: {error}"),
        }
    }

    fn refresh_revolve_ghost(&self, draft: &mut RevolveDraft) {
        let (Some(project), Some(axis)) = (self.project.as_ref(), draft.axis) else {
            draft.ghost = Err("pick an axis".into());
            return;
        };
        draft.ghost = match draft.angle() {
            Some(angle) => {
                project.ghost_revolve(&self.document, self.selected_profile, axis, angle)
            }
            None => Err("enter an angle between 0 and 360 degrees".into()),
        };
    }

    /// Viewport click while the revolve axis is being picked: a sketch line.
    pub(super) fn pick_revolve_axis(&mut self, point: Point) {
        let world = self.document.screen_to_world(point, self.viewport_origin());
        let line = self.document.shapes.iter().find_map(|shape| match shape {
            Shape::Line { a, b, .. } if shape.hit(world, 7.0 / self.document.zoom.max(0.2)) => {
                Some((*a, *b))
            }
            _ => None,
        });
        match line {
            Some(axis) => self.set_revolve_axis(axis),
            None => {
                self.status_message =
                    "Revolve: click a line (or press X / Y for the sketch axes)".into()
            }
        }
    }

    pub(super) fn handle_revolve_key(&mut self, code: KeyCode, text: Option<&str>) {
        let Some(mut draft) = self.revolve_draft.take() else {
            return;
        };
        if draft.axis.is_none() {
            let origin = Point { x: 0.0, y: 0.0 };
            match code {
                KeyCode::Escape => self.status_message = "Revolve cancelled".into(),
                KeyCode::KeyX => {
                    self.revolve_draft = Some(draft);
                    self.set_revolve_axis((origin, Point { x: 1.0, y: 0.0 }));
                }
                KeyCode::KeyY => {
                    self.revolve_draft = Some(draft);
                    self.set_revolve_axis((origin, Point { x: 0.0, y: 1.0 }));
                }
                _ => self.revolve_draft = Some(draft),
            }
            return;
        }
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                self.commit_revolve(draft);
                return;
            }
            KeyCode::Escape => {
                self.solid_preview = false;
                self.status_message = "Revolve cancelled".into();
                return;
            }
            KeyCode::Backspace => {
                draft.buffer.pop();
            }
            KeyCode::KeyO => draft.operation = next_operation(draft.operation),
            _ => {
                for ch in text.unwrap_or_default().chars() {
                    if (ch.is_ascii_digit() || ch == '.') && draft.buffer.len() < 12 {
                        draft.buffer.push(ch);
                    }
                }
            }
        }
        self.refresh_revolve_ghost(&mut draft);
        self.status_message = Self::revolve_hint(&draft);
        self.revolve_draft = Some(draft);
    }

    fn commit_revolve(&mut self, draft: RevolveDraft) {
        let (Some(angle), Some(axis)) = (draft.angle(), draft.axis) else {
            self.status_message = "Revolve: enter an angle between 0 and 360 degrees".into();
            self.revolve_draft = Some(draft);
            return;
        };
        let project = self.project.as_mut().expect("draft has project");
        match project.create_revolve(
            &self.document,
            self.selected_profile,
            axis,
            angle,
            draft.operation,
        ) {
            Ok(id) => {
                self.selected_feature = Some(id);
                self.solid_preview = true;
                self.status_message = format!(
                    "Created {angle:.1} deg {} revolve  |  Ctrl+S to save  |  Tab: sketch",
                    draft.operation
                );
            }
            Err(error) => {
                self.status_message = format!("Revolve: {error}");
                self.revolve_draft = Some(draft);
            }
        }
    }

    /// Key 7 (Shift+7 joins): first press remembers the selected profile,
    /// the second press on another sketch lofts to its selected profile.
    pub(super) fn loft_step(&mut self, join: bool) {
        self.more_open = false;
        self.document.finish_drag();
        let Some(project) = self.project.as_mut() else {
            self.status_message = "Open a .pcad project to loft".into();
            return;
        };
        if self.solid_preview {
            self.status_message = "Loft: Tab to a sketch, select a profile, press 7".into();
            return;
        }
        if profiles_from_document(&self.document).is_none() {
            self.status_message = "Loft: the sketch has no closed profile".into();
            return;
        }
        let here = (project.active_sketch(), self.selected_profile);
        match self.loft_start {
            Some(start) if start.0 != here.0 => {
                let operation = if join { "join" } else { "newBody" };
                match project.create_loft(&self.document, start, here.1, operation) {
                    Ok(id) => {
                        self.loft_start = None;
                        self.selected_feature = Some(id);
                        self.solid_preview = true;
                        self.status_message =
                            format!("Created {operation} loft  |  Ctrl+S to save  |  Tab: sketch");
                    }
                    Err(error) => self.status_message = format!("Loft: {error}"),
                }
            }
            _ => {
                self.loft_start = Some(here);
                self.status_message = format!(
                    "Loft from {} profile {}: open another sketch, select a profile, press 7 (Shift+7 joins)  |  Esc cancel",
                    project.sketch_name(here.0).unwrap_or("sketch"),
                    here.1 + 1
                );
            }
        }
    }

    /// Keys 8 / 9 / MORE: arms edge picking in the 3D preview.
    pub(super) fn begin_edge_finish(&mut self, kind: FinishKind) {
        self.more_open = false;
        self.editing = None;
        let has_solid = self
            .project
            .as_ref()
            .and_then(ProjectFile::evaluated_solids)
            .is_some_and(|solids| !solids.is_empty());
        if !has_solid {
            self.status_message = format!("Edge {}: create a solid first", kind.name());
            return;
        }
        self.solid_preview = true;
        self.edge_pick = Some(EdgePick {
            kind,
            edges: Vec::new(),
            buffer: String::new(),
        });
        self.status_message = format!(
            "Edge {}: click edges, type the size, Enter commits  |  Esc cancel",
            kind.name()
        );
    }

    /// The sharp edge nearest the screen point, within a few pixels.
    pub(super) fn edge_near(
        &self,
        projector: &SolidProjector,
        edges: &[(usize, SolidEdge)],
        point: Point,
    ) -> Option<SolidEdge> {
        edges
            .iter()
            .map(|(_, edge)| {
                let distance =
                    segment_distance(point, projector.screen(edge.a), projector.screen(edge.b));
                (distance, edge)
            })
            .filter(|(distance, _)| *distance <= EDGE_PICK_PIXELS)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, edge)| edge.clone())
    }

    /// Viewport click while edge picking: toggles the edge under the cursor.
    pub(super) fn toggle_edge_at(&mut self, layout: Layout, point: Point) {
        let Some((_, projector)) = self.solid_projector(layout) else {
            return;
        };
        let edges = self
            .project
            .as_ref()
            .map(ProjectFile::solid_edges)
            .unwrap_or_default();
        let Some(edge) = self.edge_near(&projector, &edges, point) else {
            self.status_message = "No sharp edge under the cursor".into();
            return;
        };
        let Some(pick) = self.edge_pick.as_mut() else {
            return;
        };
        if !edge.straight {
            self.status_message = "Only straight edges between planar faces can be finished".into();
            return;
        }
        let reference = edge.reference();
        if let Some(index) = pick.edges.iter().position(|e| *e == reference) {
            pick.edges.remove(index);
        } else {
            pick.edges.push(reference);
        }
        self.status_message = format!(
            "{} edge{} picked  |  type {}, Enter commits  |  Esc cancel",
            pick.edges.len(),
            if pick.edges.len() == 1 { "" } else { "s" },
            pick.label().to_lowercase()
        );
    }

    pub(super) fn handle_edge_key(&mut self, code: KeyCode, text: Option<&str>) {
        let Some(mut pick) = self.edge_pick.take() else {
            return;
        };
        match code {
            KeyCode::Escape => {
                self.status_message = format!("Edge {} cancelled", pick.kind.name());
                return;
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let Some(size) = pick.size() else {
                    self.status_message = "Enter a positive size in millimeters".into();
                    self.edge_pick = Some(pick);
                    return;
                };
                let project = self.project.as_mut().expect("edge pick has project");
                match project.create_edge_finish(pick.kind, size, &pick.edges) {
                    Ok(id) => {
                        self.selected_feature = Some(id);
                        self.status_message = format!(
                            "Created {size:.1} mm {} on {} edge{}  |  Ctrl+S to save",
                            pick.kind.name(),
                            pick.edges.len(),
                            if pick.edges.len() == 1 { "" } else { "s" }
                        );
                    }
                    Err(error) => {
                        self.status_message = format!("Edge {}: {error}", pick.kind.name());
                        self.edge_pick = Some(pick);
                    }
                }
                return;
            }
            KeyCode::Backspace => {
                pick.buffer.pop();
            }
            _ => {
                for ch in text.unwrap_or_default().chars() {
                    if (ch.is_ascii_digit() || ch == '.') && pick.buffer.len() < 12 {
                        pick.buffer.push(ch);
                    }
                }
            }
        }
        self.edge_pick = Some(pick);
    }

    /// Selected revolve/loft/edge finish, when one is selected.
    pub(super) fn selected_extension(&self) -> Option<project::FeatureInfo> {
        let id = self.selected_feature.as_ref()?;
        self.project
            .as_ref()?
            .extension_features()
            .into_iter()
            .find(|feature| &feature.id == id)
    }

    /// Edits a revolve angle or edge finish size as one undo step.
    pub(super) fn set_feature_value(&mut self, value: f32) -> Result<(), String> {
        let id = self.selected_feature.clone().ok_or("no feature selected")?;
        let project = self.project.as_mut().ok_or("no project")?;
        let node = project.solid_node(&id).ok_or("feature node is missing")?;
        project.set_feature_value(&id, value)?;
        self.solid_undo.push(SolidUndo { index: 0, node });
        self.solid_redo.clear();
        Ok(())
    }

    pub(super) fn delete_selected_feature(&mut self) {
        let (Some(id), Some(project)) = (self.selected_feature.clone(), self.project.as_mut())
        else {
            return;
        };
        self.status_message = match project.delete_feature(&id) {
            Ok(()) => {
                self.selected_feature = None;
                self.solid_preview = project.evaluated_solids().is_some();
                "Feature removed  |  Ctrl+S to save".into()
            }
            Err(error) => format!("Delete feature: {error}"),
        };
    }

    /// Translucent ghost triangles over the current bodies, plus a prompt.
    fn paint_ghost(
        &self,
        painter: &mut Painter,
        layout: Layout,
        ghost: &[Tri],
        cut: bool,
        prompt: &str,
    ) {
        let solids = self
            .project
            .as_ref()
            .and_then(ProjectFile::evaluated_solids)
            .unwrap_or_default();
        let scene = SolidScene::from_manifolds(&solids);
        let mut vertices = scene.vertices.clone();
        vertices.extend(ghost.iter().flatten().copied());
        painter.rect(layout.viewport, [0.065, 0.085, 0.115, 1.0]);
        if let Some(projector) = SolidProjector::new(self.solid_view, &vertices, layout.viewport) {
            self.paint_boolean_preview(painter, layout, &scene, &projector, scene.bodies);
            let color: Color = if cut {
                [0.95, 0.42, 0.32, 0.38]
            } else {
                [0.98, 0.72, 0.30, 0.38]
            };
            let mut faces: Vec<(f32, [[f32; 2]; 3])> = ghost
                .iter()
                .map(|&[a, b, c]| {
                    let mid = std::array::from_fn(|axis| (a[axis] + b[axis] + c[axis]) / 3.0);
                    (projector.depth(mid), [a, b, c].map(|p| projector.screen(p)))
                })
                .collect();
            faces.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, [a, b, c]) in faces {
                painter.triangle(a, b, c, color);
            }
        }
        let rect = Rect {
            x: layout.viewport.x + 18.0,
            y: layout.viewport.y + layout.viewport.h - 58.0,
            w: 460.0,
            h: 40.0,
        };
        painter.rect(rect, PANEL_LIGHT);
        painter.outline(rect, 1.0, ACCENT);
        painter.text(rect.x + 12.0, rect.y + 13.0, prompt, 1.2, ACCENT);
    }

    /// Revolve ghost and ANGLE prompt (after the axis is picked).
    pub(super) fn paint_revolve_draft(&self, painter: &mut Painter, layout: Layout) {
        let Some(draft) = &self.revolve_draft else {
            return;
        };
        let ghost = draft.ghost.as_ref().map(Vec::as_slice).unwrap_or_default();
        let prompt = format!(
            "ANGLE  {}_ deg   {}",
            if draft.buffer.is_empty() {
                format!("{DEFAULT_ANGLE:.0}")
            } else {
                draft.buffer.clone()
            },
            draft.operation.to_uppercase()
        );
        self.paint_ghost(painter, layout, ghost, draft.operation == "cut", &prompt);
    }

    /// Axis pick prompt over the sketch view.
    pub(super) fn paint_revolve_axis_prompt(&self, painter: &mut Painter, layout: Layout) {
        if self
            .revolve_draft
            .as_ref()
            .is_none_or(|draft| draft.axis.is_some())
        {
            return;
        }
        let rect = Rect {
            x: layout.viewport.x + 18.0,
            y: layout.viewport.y + layout.viewport.h - 58.0,
            w: 520.0,
            h: 40.0,
        };
        painter.rect(rect, PANEL_LIGHT);
        painter.outline(rect, 1.0, ACCENT);
        painter.text(
            rect.x + 12.0,
            rect.y + 13.0,
            "REVOLVE  CLICK AXIS LINE  /  X OR Y SKETCH AXIS",
            1.2,
            ACCENT,
        );
    }

    /// Picked and hovered edges plus the size prompt.
    pub(super) fn paint_edge_pick(
        &self,
        painter: &mut Painter,
        layout: Layout,
        projector: &SolidProjector,
    ) {
        let Some(pick) = &self.edge_pick else {
            return;
        };
        let edges = self
            .project
            .as_ref()
            .map(ProjectFile::solid_edges)
            .unwrap_or_default();
        for (_, edge) in &edges {
            let (a, b) = (projector.screen(edge.a), projector.screen(edge.b));
            if pick.edges.contains(&edge.reference()) {
                painter.line(a[0], a[1], b[0], b[1], 4.0, ACCENT);
            }
        }
        if layout.viewport.contains(self.cursor.x, self.cursor.y)
            && let Some(edge) = self.edge_near(projector, &edges, self.cursor)
        {
            let (a, b) = (projector.screen(edge.a), projector.screen(edge.b));
            let color = if edge.straight { FACE_HOVER } else { MUTED };
            painter.line(a[0], a[1], b[0], b[1], 3.0, color);
        }
        let rect = Rect {
            x: layout.viewport.x + 18.0,
            y: layout.viewport.y + layout.viewport.h - 58.0,
            w: 520.0,
            h: 40.0,
        };
        painter.rect(rect, PANEL_LIGHT);
        painter.outline(rect, 1.0, ACCENT);
        painter.text(
            rect.x + 12.0,
            rect.y + 13.0,
            &format!(
                "{}  {}  {}_ mm   EDGES {}",
                pick.kind.name().to_uppercase(),
                pick.label(),
                if pick.buffer.is_empty() {
                    format!("{DEFAULT_FINISH:.0}")
                } else {
                    pick.buffer.clone()
                },
                pick.edges.len()
            ),
            1.2,
            ACCENT,
        );
    }

    /// Properties panel for a selected revolve, loft or edge finish.
    pub(super) fn paint_feature_panel(&self, painter: &mut Painter, layout: Layout) -> bool {
        let Some(feature) = self.selected_extension().filter(|_| self.solid_preview) else {
            return false;
        };
        let x = layout.right.x + 18.0;
        let y = layout.right.y;
        painter.text(x, y + 66.0, &feature.name, 1.7, ACCENT);
        if let Some((label, unit)) = feature.value_label {
            painter.text(
                x,
                y + 113.0,
                &format!("{label:<8} {:.1} {unit}", feature.value),
                1.25,
                TEXT,
            );
        }
        painter.text(
            x,
            y + 143.0,
            &format!("TYPE    {}", feature.kind.to_uppercase()),
            1.25,
            TEXT,
        );
        painter.text(
            x,
            y + 173.0,
            &format!("OP      {}", feature.operation.to_uppercase()),
            1.25,
            TEXT,
        );
        for (row, line) in [
            "CLICK THE VALUE TO EDIT",
            "CTRL+Z UNDO  DELETE REMOVES",
            "TAB  Sketch view",
        ]
        .into_iter()
        .enumerate()
        {
            painter.text(x, y + 245.0 + row as f32 * 22.0, line, 1.0, MUTED);
        }
        true
    }
}
