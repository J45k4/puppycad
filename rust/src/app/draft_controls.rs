//! Mouse controls for an open extrude / revolve / edge finish draft: the
//! clickable rows and COMMIT / CANCEL buttons of the Properties block, the
//! clickable prompt box, and dragging the extrusion ghost's end cap.

use super::*;
use project::{EndCondition, FinishKind, Tri};

/// One clickable control of the draft Properties block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum DraftControl {
    /// DEPTH / ANGLE / DISTANCE / RADIUS: focuses the prompt.
    Value,
    End,
    Flip,
    Op,
    /// Up to body target: arms a body pick.
    Target,
    Commit,
    Cancel,
}

/// Left-drag on the extrusion ghost's end cap.
pub(super) struct DepthDrag {
    start: Point,
    depth: f32,
    /// Screen pixels per millimeter along the drag direction.
    axis: [f32; 2],
    /// 2 for symmetric (the cap moves half the total depth).
    scale: f32,
}

/// The ghost's end cap on screen.
pub(super) struct CapHandle {
    pub(super) centre: [f32; 2],
    /// Screen pixels of one millimeter of depth.
    pub(super) axis: [f32; 2],
    pub(super) polygon: Vec<[f32; 2]>,
}

const ROW_TOP: f32 = 105.0;
const ROW_PITCH: f32 = 30.0;
const ROW_HEIGHT: f32 = 25.0;

fn inside_polygon(point: [f32; 2], polygon: &[[f32; 2]]) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);
        if (a[1] > point[1]) != (b[1] > point[1])
            && point[0] < a[0] + (point[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0])
        {
            inside = !inside;
        }
    }
    inside
}

impl App {
    /// True while a draft that owns the Properties block is open.
    pub(super) fn draft_panel_open(&self) -> bool {
        self.extrude_draft.is_some()
            || self
                .revolve_draft
                .as_ref()
                .is_some_and(|draft| draft.axis.is_some())
            || self.edge_pick.is_some()
    }

    /// Controls of the draft Properties block with their rects and labels.
    pub(super) fn draft_controls(&self, layout: Layout) -> Vec<(DraftControl, Rect, String)> {
        let caret = |buffer: &str, default: f32| {
            if buffer.is_empty() {
                format!("{default:.0}_")
            } else {
                format!("{buffer}_")
            }
        };
        let mut rows: Vec<(DraftControl, String)> = Vec::new();
        if let Some(draft) = &self.extrude_draft {
            let depth = if self.draft_focus {
                caret(&draft.buffer, DEFAULT_DEPTH)
            } else {
                draft
                    .ghost
                    .as_ref()
                    .map_or_else(|_| "--".to_string(), |solid| format!("{:.1}", solid.depth))
            };
            rows.push((DraftControl::Value, format!("DEPTH   {depth} mm")));
            rows.push((
                DraftControl::End,
                format!("END     {}", draft.options.end.label()),
            ));
            rows.push((
                DraftControl::Flip,
                format!("FLIP    {}", if draft.options.flip { "ON" } else { "OFF" }),
            ));
            rows.push((
                DraftControl::Op,
                format!("OP      {}", draft.operation.to_uppercase()),
            ));
            if let EndCondition::UpToBody(target) = &draft.options.end {
                let label = if self.body_pick {
                    "CLICK A BODY".to_string()
                } else {
                    clip_text(target, 14, false)
                };
                rows.push((DraftControl::Target, format!("TARGET  {label}")));
            }
        } else if let Some(draft) = self.revolve_draft.as_ref().filter(|d| d.axis.is_some()) {
            let angle = if self.draft_focus {
                caret(&draft.buffer, DEFAULT_ANGLE)
            } else {
                draft
                    .angle()
                    .map_or_else(|| "--".to_string(), |angle| format!("{angle:.1}"))
            };
            rows.push((DraftControl::Value, format!("ANGLE   {angle} deg")));
            rows.push((
                DraftControl::Op,
                format!("OP      {}", draft.operation.to_uppercase()),
            ));
        } else if let Some(pick) = &self.edge_pick {
            let label = match pick.kind {
                FinishKind::Chamfer => "DISTANCE",
                FinishKind::Fillet => "RADIUS",
            };
            let size = if self.draft_focus {
                caret(&pick.buffer, DEFAULT_FINISH)
            } else {
                pick.size()
                    .map_or_else(|| "--".to_string(), |size| format!("{size:.1}"))
            };
            rows.push((DraftControl::Value, format!("{label:<8}{size} mm")));
        } else {
            return Vec::new();
        }
        let x = layout.right.x + 10.0;
        let w = layout.right.w - 20.0;
        let mut controls: Vec<(DraftControl, Rect, String)> = rows
            .into_iter()
            .enumerate()
            .map(|(row, (control, text))| {
                let rect = Rect {
                    x,
                    y: layout.right.y + ROW_TOP + row as f32 * ROW_PITCH,
                    w,
                    h: ROW_HEIGHT,
                };
                (control, rect, text)
            })
            .collect();
        let buttons = layout.right.y + self.draft_hint_top() + 4.0 * 22.0 + 12.0;
        for (row, (control, text)) in [
            (DraftControl::Commit, "COMMIT  ENTER"),
            (DraftControl::Cancel, "CANCEL  ESC"),
        ]
        .into_iter()
        .enumerate()
        {
            controls.push((
                control,
                Rect {
                    x,
                    y: buttons + row as f32 * 36.0,
                    w,
                    h: 30.0,
                },
                text.to_string(),
            ));
        }
        controls
    }

    /// Top of the key hint lines, below the value rows.
    fn draft_hint_top(&self) -> f32 {
        let rows = match (&self.extrude_draft, &self.revolve_draft) {
            (Some(draft), _) => {
                4 + usize::from(matches!(draft.options.end, EndCondition::UpToBody(_)))
            }
            (_, Some(_)) => 2,
            _ => 2,
        };
        ROW_TOP + rows as f32 * ROW_PITCH + 15.0
    }

    /// The prompt box at the bottom left of the viewport.
    pub(super) fn draft_prompt_rect(&self, layout: Layout) -> Rect {
        Rect {
            x: layout.viewport.x + 18.0,
            y: layout.viewport.y + layout.viewport.h - 58.0,
            w: if self.edge_pick.is_some() {
                520.0
            } else {
                460.0
            },
            h: 40.0,
        }
    }

    fn focus_draft_prompt(&mut self) {
        self.draft_focus = true;
        let what = if self.extrude_draft.is_some() {
            "depth in mm"
        } else if self.revolve_draft.is_some() {
            "angle in degrees"
        } else {
            "size in mm"
        };
        self.status_message = format!("Type the {what}, Enter commits  |  Esc cancel");
    }

    /// Feed a key to whichever draft is open.
    fn draft_key(&mut self, code: KeyCode) {
        if self.extrude_draft.is_some() {
            self.handle_draft_key(code, None);
        } else if self.revolve_draft.is_some() {
            self.handle_revolve_key(code, None);
        } else if self.edge_pick.is_some() {
            self.handle_edge_key(code, None);
        }
    }

    /// Left click on a draft control or on the prompt box.
    pub(super) fn draft_control_click(&mut self, layout: Layout, point: Point) -> bool {
        if !self.draft_panel_open() {
            return false;
        }
        if self.draft_prompt_rect(layout).contains(point.x, point.y) {
            self.focus_draft_prompt();
            return true;
        }
        let Some(control) = self
            .draft_controls(layout)
            .into_iter()
            .find(|(_, rect, _)| rect.contains(point.x, point.y))
            .map(|(control, _, _)| control)
        else {
            return false;
        };
        self.body_pick = false;
        match control {
            DraftControl::Value => self.focus_draft_prompt(),
            DraftControl::End => self.draft_key(KeyCode::KeyE),
            DraftControl::Flip => self.draft_key(KeyCode::KeyF),
            DraftControl::Op => self.draft_key(KeyCode::KeyO),
            DraftControl::Target => {
                self.body_pick = true;
                self.status_message = "Click the target body  |  Esc to cancel".into();
            }
            DraftControl::Commit => self.draft_key(KeyCode::Enter),
            DraftControl::Cancel => self.draft_key(KeyCode::Escape),
        }
        if !self.draft_panel_open() {
            self.draft_focus = false;
        }
        true
    }

    /// Viewport click with Up to body armed during an extrude draft.
    pub(super) fn pick_draft_target(&mut self, layout: Layout, point: Point) {
        self.body_pick = false;
        let body = self
            .solid_projector(layout)
            .and_then(|(scene, projector)| projector.pick_body(&scene, point));
        let id = body.and_then(|body| {
            self.project
                .as_ref()
                .and_then(|project| project.body_ids().get(body).cloned())
        });
        let Some(id) = id else {
            self.status_message = "No body under the cursor".into();
            return;
        };
        let Some(mut draft) = self.extrude_draft.take() else {
            return;
        };
        draft.options.end = EndCondition::UpToBody(id);
        self.refresh_ghost(&mut draft);
        self.status_message = match &draft.ghost {
            Ok(_) => "Target set  |  Enter commit  |  Esc cancel".into(),
            Err(error) => format!("Extrude preview: {error}"),
        };
        self.extrude_draft = Some(draft);
    }

    /// Projector the ghost is painted with (bodies plus ghost fit the view).
    pub(super) fn ghost_projector(&self, layout: Layout, ghost: &[Tri]) -> Option<SolidProjector> {
        let solids = self
            .project
            .as_ref()
            .and_then(ProjectFile::evaluated_solids)
            .unwrap_or_default();
        let scene = SolidScene::from_manifolds(&solids);
        let mut vertices = scene.vertices.clone();
        vertices.extend(ghost.iter().flatten().copied());
        SolidProjector::new(self.solid_view, &vertices, layout.viewport)
    }

    /// Screen centre of the draggable end cap and the screen vector of one
    /// millimeter of depth, when the draft is blind or symmetric.
    pub(super) fn draft_cap_handle(&self, layout: Layout) -> Option<CapHandle> {
        let draft = self.extrude_draft.as_ref()?;
        let solid = draft.ghost.as_ref().ok()?;
        let flipped = match draft.options.end {
            EndCondition::Blind => draft.options.flip,
            EndCondition::Symmetric => false,
            _ => return None,
        };
        let offset = if flipped {
            solid.start
        } else {
            solid.start + solid.depth
        };
        let ghost = stl::solid_triangles(solid).ok()?;
        let projector = self.ghost_projector(layout, &ghost)?;
        let cap: Vec<[f32; 3]> = solid
            .profile
            .iter()
            .map(|p| solid.frame.world(*p, offset))
            .collect();
        if cap.is_empty() {
            return None;
        }
        let centre: [f32; 3] =
            std::array::from_fn(|i| cap.iter().map(|p| p[i]).sum::<f32>() / cap.len() as f32);
        let sign = if flipped { -1.0 } else { 1.0 };
        let tip: [f32; 3] = std::array::from_fn(|i| centre[i] + solid.frame.normal[i] * sign);
        let (a, b) = (projector.screen(centre), projector.screen(tip));
        let axis = [b[0] - a[0], b[1] - a[1]];
        let polygon = cap.into_iter().map(|p| projector.screen(p)).collect();
        Some(CapHandle {
            centre: a,
            axis,
            polygon,
        })
    }

    /// Left press in the viewport during an extrude draft: grab the end cap.
    pub(super) fn begin_depth_drag(&mut self, layout: Layout, point: Point) -> bool {
        if self.body_pick || !layout.viewport.contains(point.x, point.y) {
            return false;
        }
        let Some(CapHandle {
            centre,
            axis,
            polygon,
        }) = self.draft_cap_handle(layout)
        else {
            return false;
        };
        let near_centre = (point.x - centre[0]).hypot(point.y - centre[1]) <= 14.0;
        if !(near_centre || inside_polygon([point.x, point.y], &polygon))
            || axis[0].hypot(axis[1]) < 1e-3
        {
            return false;
        }
        let draft = self.extrude_draft.as_ref().expect("cap handle has a draft");
        let depth = draft
            .ghost
            .as_ref()
            .map_or(DEFAULT_DEPTH, |solid| solid.depth);
        let scale = if draft.options.end == EndCondition::Symmetric {
            2.0
        } else {
            1.0
        };
        self.depth_drag = Some(DepthDrag {
            start: point,
            depth,
            axis,
            scale,
        });
        self.status_message = "Drag the end cap: depth snaps to 1 mm, Shift 0.1 mm".into();
        true
    }

    /// Pointer move while dragging the end cap.
    pub(super) fn drag_depth_to(&mut self, point: Point) {
        let Some(drag) = &self.depth_drag else {
            return;
        };
        let delta = [point.x - drag.start.x, point.y - drag.start.y];
        let length2 = drag.axis[0] * drag.axis[0] + drag.axis[1] * drag.axis[1];
        let along = (delta[0] * drag.axis[0] + delta[1] * drag.axis[1]) / length2;
        let step = if self.modifiers.shift_key() { 0.1 } else { 1.0 };
        let depth = ((drag.depth + along * drag.scale) / step).round() * step;
        let depth = depth.max(step);
        let Some(mut draft) = self.extrude_draft.take() else {
            self.depth_drag = None;
            return;
        };
        draft.buffer = if step < 1.0 {
            format!("{depth:.1}")
        } else {
            format!("{depth:.0}")
        };
        self.refresh_ghost(&mut draft);
        self.status_message = format!(
            "Depth {} mm  |  release, then Enter commits  |  Esc cancel",
            draft.buffer
        );
        self.extrude_draft = Some(draft);
    }

    /// Left release after an end cap drag: the draft stays open.
    pub(super) fn finish_depth_drag(&mut self) {
        if self.depth_drag.take().is_some()
            && let Some(draft) = &self.extrude_draft
        {
            self.status_message = format!(
                "Depth {} mm  |  Enter commit  |  Esc cancel",
                if draft.buffer.is_empty() {
                    format!("{DEFAULT_DEPTH:.0}")
                } else {
                    draft.buffer.clone()
                }
            );
        }
    }

    /// Properties block of an open draft: rows, hints and buttons.
    pub(super) fn paint_draft_panel(&self, painter: &mut Painter, layout: Layout) {
        let x = layout.right.x + 18.0;
        let y = layout.right.y;
        let title = match self.draft_kind() {
            Some("Extrusion") => "NEW EXTRUSION",
            Some("Revolve") => "NEW REVOLVE",
            Some("Chamfer") => "NEW CHAMFER",
            Some("Fillet") => "NEW FILLET",
            _ => return,
        };
        painter.text(x, y + 66.0, title, 1.7, ACCENT);
        for (control, rect, text) in self.draft_controls(layout) {
            let hovered = rect.contains(self.cursor.x, self.cursor.y);
            let button = matches!(control, DraftControl::Commit | DraftControl::Cancel);
            let focused = control == DraftControl::Value && self.draft_focus;
            if hovered || button || focused {
                painter.rect(rect, if hovered { PANEL_LIGHT } else { PANEL });
                painter.outline(rect, 1.0, if hovered || focused { ACCENT } else { MUTED });
            }
            let text = if button { format!("[ {text} ]") } else { text };
            painter.text(
                rect.x + 8.0,
                rect.y + if button { 10.0 } else { 8.0 },
                &text,
                1.25,
                if control == DraftControl::Commit {
                    ACCENT
                } else {
                    TEXT
                },
            );
        }
        let hints: [&str; 4] = if self.extrude_draft.is_some() {
            [
                "CLICK A ROW OR TYPE DEPTH",
                "E END  F FLIP  O OPERATION",
                "DRAG THE END CAP FOR DEPTH",
                "EMPTY ENTER = 50 MM",
            ]
        } else if self.revolve_draft.is_some() {
            [
                "CLICK A ROW OR TYPE ANGLE",
                "O OPERATION",
                "EMPTY ENTER = 360 DEG",
                "",
            ]
        } else {
            [
                "CLICK EDGES IN THE VIEW",
                "TYPE THE SIZE",
                "EMPTY ENTER = 2 MM",
                "",
            ]
        };
        let top = y + self.draft_hint_top();
        for (row, line) in hints.into_iter().enumerate() {
            painter.text(x, top + row as f32 * 22.0, line, 1.0, MUTED);
        }
        let error = match (&self.extrude_draft, &self.revolve_draft) {
            (Some(draft), _) => draft.ghost.as_ref().err(),
            (_, Some(draft)) => draft.ghost.as_ref().err(),
            _ => None,
        };
        if let Some(error) = error.filter(|error| !error.is_empty())
            && let Some((_, rect, _)) = self.draft_controls(layout).last()
        {
            painter.text(
                x,
                rect.y + rect.h + 14.0,
                &error.to_uppercase(),
                0.9,
                ACCENT,
            );
        }
    }
}
