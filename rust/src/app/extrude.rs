//! Extrude options in the solid preview (end condition, flip, up-to-body
//! pick, undo) and the ghost preview shown before an extrusion is committed.

use super::*;
use project::{EndCondition, ExtrudeOptions};
use serde_json::Value;

/// Depth committed when Enter is pressed on an empty prompt.
pub(super) const DEFAULT_DEPTH: f32 = 50.0;

/// X / Shift+X / Shift+J pressed: an extrusion waiting for Enter.
pub(super) struct ExtrudeDraft {
    pub(super) operation: &'static str,
    pub(super) buffer: String,
    pub(super) options: ExtrudeOptions,
    /// Tool solid of the would-be extrusion, or why it cannot be built.
    pub(super) ghost: Result<SolidPreview, String>,
}

impl ExtrudeDraft {
    pub(super) fn depth(&self) -> Option<f32> {
        if self.buffer.is_empty() {
            return Some(DEFAULT_DEPTH);
        }
        self.buffer
            .parse::<f32>()
            .ok()
            .filter(|depth| depth.is_finite() && *depth > 0.0)
    }
}

/// What an undoable solid step restores: one extrusion node, or the whole
/// feature tree (reorder, suppress, rollback, revolve/loft/finish edits).
pub(super) enum SolidUndoKind {
    Node { index: usize, node: Value },
    Tree { snapshot: Value, selected: usize },
}

/// One undoable solid change.
pub(super) struct SolidUndo {
    pub(super) kind: SolidUndoKind,
    /// History panel label ("Extrude 50 mm").
    pub(super) label: String,
    /// Ordering stamp shared with sketch steps (`model::next_step_stamp`).
    pub(super) stamp: u64,
}

impl SolidUndo {
    pub(super) fn new(index: usize, node: Value, label: String) -> Self {
        Self {
            kind: SolidUndoKind::Node { index, node },
            label,
            stamp: model::next_step_stamp(),
        }
    }

    pub(super) fn tree(snapshot: Value, selected: usize, label: String) -> Self {
        Self {
            kind: SolidUndoKind::Tree { snapshot, selected },
            label,
            stamp: model::next_step_stamp(),
        }
    }
}

/// Blind -> Symmetric -> Through all -> Up to body -> Blind. Through all and
/// up to body are skipped for new bodies or when there is no body yet.
pub(super) fn next_end_condition(
    end: &EndCondition,
    operation: &str,
    target: Option<&str>,
) -> EndCondition {
    let bodies = operation != "newBody" && target.is_some();
    match end {
        EndCondition::Blind => EndCondition::Symmetric,
        EndCondition::Symmetric if bodies => EndCondition::ThroughAll,
        EndCondition::ThroughAll => {
            EndCondition::UpToBody(target.expect("bodies checked").to_string())
        }
        _ => EndCondition::Blind,
    }
}

impl App {
    /// Last body created before extrusion `index` (or before a new one).
    fn default_target(&self, index: Option<usize>) -> Option<String> {
        let project = self.project.as_ref()?;
        let count = project
            .extrusion_previews()
            .map_or(0, |solids| solids.len());
        (0..index.unwrap_or(count).min(count))
            .filter(|&i| project.extrusion_operation_at(i) == Some("newBody"))
            .filter_map(|i| project.extrusion_id_at(i).map(str::to_string))
            .next_back()
    }

    pub(super) fn begin_extrude(&mut self, operation: &'static str) {
        self.document.finish_drag();
        self.more_open = false;
        self.editing = None;
        if self.project.is_none() {
            self.status_message = "Open a .pcad project to create an extrusion".into();
            return;
        }
        if let Some(profiles) = profiles_from_document(&self.document) {
            self.selected_profile = self.selected_profile.min(profiles.len().saturating_sub(1));
        }
        let mut draft = ExtrudeDraft {
            operation,
            buffer: String::new(),
            options: ExtrudeOptions {
                end: EndCondition::Blind,
                flip: false,
            },
            ghost: Err(String::new()),
        };
        self.refresh_ghost(&mut draft);
        if let Err(error) = &draft.ghost {
            self.status_message = format!("Extrude: {error}");
            return;
        }
        self.extrude_draft = Some(draft);
        self.body_pick = false;
        self.solid_preview = true;
        self.status_message = Self::draft_hint(operation);
    }

    fn draft_hint(operation: &str) -> String {
        format!(
            "Extrude {operation}: type depth, Enter commits (empty = {DEFAULT_DEPTH:.0} mm)  |  E end  F flip  |  Esc cancel"
        )
    }

    fn refresh_ghost(&mut self, draft: &mut ExtrudeDraft) {
        let Some(project) = self.project.as_mut() else {
            draft.ghost = Err("no project".into());
            return;
        };
        draft.ghost = match draft.depth() {
            Some(depth) => project.ghost_extrusion(
                &self.document,
                depth,
                self.selected_profile,
                draft.operation,
                &draft.options,
            ),
            None => Err("enter a positive depth in millimeters".into()),
        };
    }

    /// Keys while the extrude prompt is open; the prompt takes them all.
    pub(super) fn handle_draft_key(&mut self, code: KeyCode, text: Option<&str>) {
        let Some(mut draft) = self.extrude_draft.take() else {
            return;
        };
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter => {
                self.commit_draft(draft);
                return;
            }
            KeyCode::Escape => {
                self.solid_preview = false;
                self.status_message = "Extrude cancelled".into();
                return;
            }
            KeyCode::Backspace => {
                draft.buffer.pop();
            }
            KeyCode::KeyE => {
                let target = self.default_target(None);
                draft.options.end =
                    next_end_condition(&draft.options.end, draft.operation, target.as_deref());
            }
            KeyCode::KeyF => draft.options.flip = !draft.options.flip,
            _ => {
                for ch in text.unwrap_or_default().chars() {
                    if (ch.is_ascii_digit() || ch == '.') && draft.buffer.len() < 12 {
                        draft.buffer.push(ch);
                    }
                }
            }
        }
        self.refresh_ghost(&mut draft);
        self.status_message = match &draft.ghost {
            Ok(_) => Self::draft_hint(draft.operation),
            Err(error) => format!("Extrude preview: {error}"),
        };
        self.extrude_draft = Some(draft);
    }

    fn commit_draft(&mut self, draft: ExtrudeDraft) {
        let Some(depth) = draft.depth() else {
            self.status_message = "Extrude: enter a positive depth in millimeters".into();
            self.extrude_draft = Some(draft);
            return;
        };
        let project = self.project.as_mut().expect("draft has project");
        match project.create_extrusion_with_options(
            &self.document,
            depth,
            self.selected_profile,
            draft.operation,
            &draft.options,
        ) {
            Ok(()) => {
                self.selected_solid = project
                    .extrusion_previews()
                    .map_or(0, |solids| solids.len() - 1);
                self.solid_preview = true;
                self.status_message = format!(
                    "Created {depth:.1} mm {} {} extrusion  |  Ctrl+S to save  |  Tab: sketch",
                    draft.options.end.label().to_lowercase(),
                    draft.operation
                );
            }
            Err(error) => {
                self.status_message = format!("Extrude: {error}");
                self.extrude_draft = Some(draft);
            }
        }
    }

    /// Apply new options to the selected extrusion as one undo step.
    pub(super) fn apply_solid_options(&mut self, options: ExtrudeOptions) {
        let index = self.selected_solid;
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let Some(node) = project.extrude_node_snapshot(index) else {
            return;
        };
        let label = format!(
            "Set end {}{}",
            options.end.label(),
            if options.flip { ", flipped" } else { "" }
        );
        self.status_message = match project.set_extrusion_options_at(index, &options) {
            Ok(()) => {
                self.solid_undo.push(SolidUndo::new(index, node, label));
                self.solid_redo.clear();
                self.reproject_all();
                format!(
                    "{}{}  |  Ctrl+Z to undo  |  Ctrl+S to save",
                    options.end.label(),
                    if options.flip { ", flipped" } else { "" }
                )
            }
            Err(error) => format!("Extrusion error: {error}"),
        };
    }

    /// Record the node before a depth edit so it is one undo step.
    pub(super) fn set_solid_depth(&mut self, depth: f32) -> Result<(), String> {
        let index = self.selected_solid;
        let project = self.project.as_mut().ok_or("no project")?;
        let node = project
            .extrude_node_snapshot(index)
            .ok_or("extrusion node is missing")?;
        project.set_extrusion_depth_at(index, depth)?;
        let label = format!("Extrude {} mm", model::history_number_label(depth));
        self.solid_undo.push(SolidUndo::new(index, node, label));
        self.solid_redo.clear();
        self.reproject_all();
        Ok(())
    }

    pub(super) fn cycle_solid_end_condition(&mut self) {
        let index = self.selected_solid;
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let (Some(mut options), Some(operation)) = (
            project.extrusion_options_at(index),
            project.extrusion_operation_at(index).map(str::to_string),
        ) else {
            return;
        };
        let target = self.default_target(Some(index));
        options.end = next_end_condition(&options.end, &operation, target.as_deref());
        let up_to = matches!(options.end, EndCondition::UpToBody(_));
        self.apply_solid_options(options);
        if up_to
            && self.solid_undo.last().is_some_and(
                |undo| matches!(&undo.kind, SolidUndoKind::Node { index: i, .. } if *i == index),
            )
        {
            self.status_message
                .push_str("  |  U then click a body to change the target");
        }
    }

    pub(super) fn toggle_solid_flip(&mut self) {
        let Some(mut options) = self
            .project
            .as_ref()
            .and_then(|project| project.extrusion_options_at(self.selected_solid))
        else {
            return;
        };
        options.flip = !options.flip;
        self.apply_solid_options(options);
    }

    pub(super) fn arm_body_pick(&mut self) {
        let up_to = self
            .project
            .as_ref()
            .and_then(|project| project.extrusion_options_at(self.selected_solid))
            .is_some_and(|options| matches!(options.end, EndCondition::UpToBody(_)));
        if up_to {
            self.body_pick = true;
            self.status_message = "Click the target body  |  Esc to cancel".into();
        } else {
            self.status_message = "Body pick needs an UP TO BODY extrusion (E to cycle)".into();
        }
    }

    /// Viewport click while a body pick is armed.
    pub(super) fn pick_target_body(&mut self, layout: Layout, point: Point) {
        self.body_pick = false;
        let Some((scene, projector)) = self.solid_projector(layout) else {
            return;
        };
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let (Some(body), Some(mut options)) = (
            projector.pick_body(&scene, point),
            project.extrusion_options_at(self.selected_solid),
        ) else {
            self.status_message = "No body under the cursor".into();
            return;
        };
        let Some(id) = project.body_ids().get(body).cloned() else {
            return;
        };
        options.end = EndCondition::UpToBody(id);
        self.apply_solid_options(options);
    }

    /// Click on the END / FLIP / TARGET rows of the solid Properties panel.
    pub(super) fn solid_option_row_click(&mut self, layout: Layout, point: Point) -> bool {
        if !self.solid_preview
            || self.selected_feature_id().is_some()
            || self.extrude_draft.is_some()
            || !layout.right.contains(point.x, point.y)
            || self
                .project
                .as_ref()
                .and_then(ProjectFile::extrusion_previews)
                .is_none()
        {
            return false;
        }
        let y = point.y - layout.right.y;
        if (135.0..160.0).contains(&y) {
            self.cycle_solid_end_condition();
        } else if (165.0..190.0).contains(&y) {
            self.toggle_solid_flip();
        } else if (195.0..220.0).contains(&y) {
            self.arm_body_pick();
        } else {
            return false;
        }
        true
    }

    pub(super) fn solid_undo_step(&mut self, redo: bool) {
        let (from, to) = if redo {
            (&mut self.solid_redo, &mut self.solid_undo)
        } else {
            (&mut self.solid_undo, &mut self.solid_redo)
        };
        let Some(step) = from.pop() else {
            self.status_message = if redo {
                "Nothing to redo"
            } else {
                "Nothing to undo"
            }
            .into();
            return;
        };
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let SolidUndo { kind, label, stamp } = step;
        let (index, node) = match kind {
            SolidUndoKind::Node { index, node } => (index, node),
            SolidUndoKind::Tree { snapshot, selected } => {
                let current = project.feature_tree_snapshot();
                self.status_message = match project.restore_feature_tree(snapshot) {
                    Ok(()) => {
                        let message = format!("{}: {}", if redo { "Redo" } else { "Undo" }, label);
                        to.push(SolidUndo {
                            kind: SolidUndoKind::Tree {
                                snapshot: current,
                                selected: self.selected_solid,
                            },
                            label,
                            stamp,
                        });
                        self.selected_solid = selected;
                        message
                    }
                    Err(error) => format!("Undo: {error}"),
                };
                return;
            }
        };
        let Some(current) = project.extrude_node_snapshot(index) else {
            return;
        };
        self.status_message = match project.restore_extrude_node(index, node) {
            Ok(()) => {
                let message = format!("{}: {}", if redo { "Redo" } else { "Undo" }, label);
                to.push(SolidUndo {
                    kind: SolidUndoKind::Node {
                        index,
                        node: current,
                    },
                    label,
                    stamp,
                });
                self.selected_solid = index;
                message
            }
            Err(error) => format!("Undo: {error}"),
        };
    }

    /// Existing bodies plus the translucent would-be solid.
    pub(super) fn paint_extrude_draft(&self, painter: &mut Painter, layout: Layout) {
        let Some(draft) = &self.extrude_draft else {
            return;
        };
        let solids = self
            .project
            .as_ref()
            .and_then(ProjectFile::evaluated_solids)
            .unwrap_or_default();
        let scene = SolidScene::from_manifolds(&solids);
        let ghost = draft
            .ghost
            .as_ref()
            .ok()
            .and_then(|solid| stl::solid_triangles(solid).ok())
            .unwrap_or_default();
        let mut vertices = scene.vertices.clone();
        vertices.extend(ghost.iter().flatten().copied());
        painter.rect(layout.viewport, [0.065, 0.085, 0.115, 1.0]);
        let Some(projector) = SolidProjector::new(self.solid_view, &vertices, layout.viewport)
        else {
            return;
        };
        self.paint_boolean_preview(painter, layout, &scene, &projector, scene.bodies);
        let color: Color = if draft.operation == "cut" {
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
        if let Ok(solid) = &draft.ghost {
            let edge = [color[0], color[1], color[2], 1.0];
            for loop_points in solid.loops() {
                for offset in [0.0, solid.depth] {
                    for index in 0..loop_points.len() {
                        let a = projector.screen(solid.world_point(loop_points[index], offset));
                        let b = projector.screen(
                            solid.world_point(loop_points[(index + 1) % loop_points.len()], offset),
                        );
                        painter.line(a[0], a[1], b[0], b[1], 1.5, edge);
                    }
                }
            }
        }
        let prompt = Rect {
            x: layout.viewport.x + 18.0,
            y: layout.viewport.y + layout.viewport.h - 58.0,
            w: 420.0,
            h: 40.0,
        };
        painter.rect(prompt, PANEL_LIGHT);
        painter.outline(prompt, 1.0, ACCENT);
        painter.text(
            prompt.x + 12.0,
            prompt.y + 13.0,
            &format!(
                "DEPTH  {}_ mm   {}{}",
                if draft.buffer.is_empty() {
                    format!("{DEFAULT_DEPTH:.0}")
                } else {
                    draft.buffer.clone()
                },
                draft.options.end.label(),
                if draft.options.flip { "  FLIPPED" } else { "" }
            ),
            1.2,
            ACCENT,
        );
    }

    /// Properties panel while the extrude prompt is open.
    pub(super) fn paint_draft_panel(&self, painter: &mut Painter, layout: Layout) {
        let Some(draft) = &self.extrude_draft else {
            return;
        };
        let x = layout.right.x + 18.0;
        let y = layout.right.y;
        painter.text(x, y + 66.0, "NEW EXTRUSION", 1.7, ACCENT);
        let resolved = draft
            .ghost
            .as_ref()
            .map_or_else(|_| "--".to_string(), |solid| format!("{:.1}", solid.depth));
        for (row, line) in [
            format!("DEPTH   {resolved} mm"),
            format!("END     {}", draft.options.end.label()),
            format!("FLIP    {}", if draft.options.flip { "ON" } else { "OFF" }),
            format!("OP      {}", draft.operation.to_uppercase()),
        ]
        .into_iter()
        .enumerate()
        {
            painter.text(x, y + 113.0 + row as f32 * 30.0, &line, 1.25, TEXT);
        }
        for (row, line) in [
            "TYPE DEPTH, ENTER COMMIT",
            "EMPTY ENTER = 50 MM",
            "E END CONDITION  F FLIP",
            "ESC CANCEL",
        ]
        .into_iter()
        .enumerate()
        {
            painter.text(x, y + 250.0 + row as f32 * 22.0, line, 1.0, MUTED);
        }
        if let Err(error) = &draft.ghost {
            painter.text(x, y + 345.0, &error.to_uppercase(), 0.9, ACCENT);
        }
    }
}
