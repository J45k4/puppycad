//! One logical undo history across the sketch and solid views, and the
//! HISTORY panel (Ctrl+Shift+H).
//!
//! The sketch keeps its snapshot stack in [`model::Document`] and the solid
//! view keeps `solid_undo` / `solid_redo`; every step on either stack carries
//! a stamp from [`model::next_step_stamp`]. The stamps are the ordering
//! journal: undo reverts the newest applied step of either stack (switching
//! view as needed), redo reapplies the oldest undone one.

use super::*;

/// Panel row height and the y of the first row below the title.
const HISTORY_ROW_H: f32 = 20.0;
const HISTORY_TOP: f32 = 44.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StepSide {
    Sketch,
    Solid,
}

impl StepSide {
    pub(super) fn name(self) -> &'static str {
        match self {
            StepSide::Sketch => "SKETCH",
            StepSide::Solid => "SOLID",
        }
    }
}

/// One step of the combined history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TimelineStep {
    pub(super) side: StepSide,
    pub(super) label: String,
    pub(super) stamp: u64,
    /// Applied (undoable) rather than undone (redoable).
    pub(super) applied: bool,
}

/// One row of the HISTORY panel, newest first.
#[derive(Clone, Debug)]
pub(super) struct HistoryRow {
    pub(super) rect: Rect,
    /// `None` for the "Start" row (everything undone).
    pub(super) side: Option<StepSide>,
    pub(super) label: String,
    /// Number of applied steps after clicking this row.
    pub(super) target: usize,
    pub(super) current: bool,
    pub(super) redo: bool,
}

impl App {
    /// Applied steps oldest first, then undone steps in redo order.
    pub(super) fn history_timeline(&self) -> Vec<TimelineStep> {
        let step = |side, label: &str, stamp, applied| TimelineStep {
            side,
            label: label.to_string(),
            stamp,
            applied,
        };
        let mut applied: Vec<TimelineStep> = self
            .document
            .undo_steps()
            .iter()
            .map(|s| step(StepSide::Sketch, &s.label, s.stamp, true))
            .chain(
                self.solid_undo
                    .iter()
                    .map(|s| step(StepSide::Solid, &s.label, s.stamp, true)),
            )
            .collect();
        applied.sort_by_key(|step| step.stamp);
        let mut undone: Vec<TimelineStep> = self
            .document
            .redo_steps()
            .iter()
            .map(|s| step(StepSide::Sketch, &s.label, s.stamp, false))
            .chain(
                self.solid_redo
                    .iter()
                    .map(|s| step(StepSide::Solid, &s.label, s.stamp, false)),
            )
            .collect();
        undone.sort_by_key(|step| step.stamp);
        applied.extend(undone);
        applied
    }

    fn applied_steps(&self) -> usize {
        self.document.undo_steps().len() + self.solid_undo.len()
    }

    /// The stack holding the newest applied step. Sketch steps of a
    /// read-only sketch cannot be undone and are skipped.
    fn undo_side(&self) -> Option<StepSide> {
        let sketch = self.document.undo_stamp().filter(|_| !self.read_only());
        let solid = self.solid_undo.last().map(|step| step.stamp);
        match (sketch, solid) {
            (Some(a), Some(b)) => Some(if a > b {
                StepSide::Sketch
            } else {
                StepSide::Solid
            }),
            (Some(_), None) => Some(StepSide::Sketch),
            (None, Some(_)) => Some(StepSide::Solid),
            (None, None) => None,
        }
    }

    /// The stack holding the oldest undone step.
    fn redo_side(&self) -> Option<StepSide> {
        let sketch = self.document.redo_stamp().filter(|_| !self.read_only());
        let solid = self.solid_redo.last().map(|step| step.stamp);
        match (sketch, solid) {
            (Some(a), Some(b)) => Some(if a < b {
                StepSide::Sketch
            } else {
                StepSide::Solid
            }),
            (Some(_), None) => Some(StepSide::Sketch),
            (None, Some(_)) => Some(StepSide::Solid),
            (None, None) => None,
        }
    }

    fn show_view(&mut self, side: StepSide) {
        match side {
            StepSide::Sketch if self.solid_preview => {
                self.solid_preview = false;
                self.body_pick = false;
                self.refresh_reference_edges();
            }
            StepSide::Solid if !self.solid_preview => {
                let previews = self
                    .project
                    .as_ref()
                    .and_then(ProjectFile::extrusion_previews)
                    .is_some();
                if previews {
                    self.document.cancel_drag();
                    self.solid_preview = true;
                }
            }
            _ => {}
        }
    }

    /// Ctrl+Z: undo the most recent step of either view.
    pub(super) fn undo_any(&mut self) -> bool {
        if !self.solid_preview && self.document.dragging() {
            return self.document.undo();
        }
        self.step_any(false)
    }

    /// Ctrl+Y / Ctrl+Shift+Z: redo the oldest undone step of either view.
    pub(super) fn redo_any(&mut self) -> bool {
        if !self.solid_preview && self.document.dragging() {
            return self.document.redo();
        }
        self.step_any(true)
    }

    fn step_any(&mut self, redo: bool) -> bool {
        let verb = if redo { "Redo" } else { "Undo" };
        let side = if redo {
            self.redo_side()
        } else {
            self.undo_side()
        };
        let Some(side) = side else {
            self.status_message = if self.read_only() && self.document.undo_stamp().is_some() {
                "Sketch is read-only".into()
            } else {
                format!("Nothing to {}", verb.to_lowercase())
            };
            return false;
        };
        self.show_view(side);
        match side {
            StepSide::Sketch => {
                let label = if redo {
                    self.document.redo_steps().first().map(|s| s.label.clone())
                } else {
                    self.document.undo_steps().last().map(|s| s.label.clone())
                }
                .unwrap_or_default();
                let done = if redo {
                    self.document.redo()
                } else {
                    self.document.undo()
                };
                if done {
                    self.selected_dimension = None;
                    self.status_message = format!("{verb}: {label}");
                }
                done
            }
            StepSide::Solid => {
                let before = self.applied_steps();
                self.solid_undo_step(redo);
                self.applied_steps() != before
            }
        }
    }

    /// Undo or redo until `target` steps are applied (panel click).
    pub(super) fn history_jump(&mut self, target: usize) {
        loop {
            let applied = self.applied_steps();
            let moved = if applied > target {
                self.undo_any()
            } else if applied < target {
                self.redo_any()
            } else {
                break;
            };
            if !moved {
                break;
            }
        }
    }

    pub(super) fn toggle_history_panel(&mut self) {
        self.history_open = !self.history_open;
        self.status_message = if self.history_open {
            "History  |  click a step to undo or redo to it  |  Ctrl+Shift+H: close"
        } else {
            "History closed"
        }
        .into();
    }

    pub(super) fn history_rows(&self, layout: Layout) -> Vec<HistoryRow> {
        let timeline = self.history_timeline();
        let applied = timeline.iter().filter(|step| step.applied).count();
        // Newest first: undone steps (furthest redo on top), applied steps,
        // then the "Start" row.
        let mut rows: Vec<(Option<StepSide>, String, usize, bool)> = timeline
            .iter()
            .enumerate()
            .rev()
            .map(|(index, step)| {
                (
                    Some(step.side),
                    step.label.clone(),
                    index + 1,
                    !step.applied,
                )
            })
            .collect();
        rows.push((None, "Start".into(), 0, false));
        let visible = ((layout.left.h - HISTORY_TOP - 8.0) / HISTORY_ROW_H)
            .floor()
            .max(1.0) as usize;
        let current = rows
            .iter()
            .position(|row| row.2 == applied)
            .unwrap_or_default();
        let first = current
            .saturating_sub(visible / 2)
            .min(rows.len().saturating_sub(visible));
        rows.into_iter()
            .skip(first)
            .take(visible)
            .enumerate()
            .map(|(slot, (side, label, target, redo))| HistoryRow {
                rect: Rect {
                    x: layout.left.x + 8.0,
                    y: layout.left.y + HISTORY_TOP + slot as f32 * HISTORY_ROW_H,
                    w: layout.left.w - 16.0,
                    h: HISTORY_ROW_H - 2.0,
                },
                side,
                label,
                target,
                current: target == applied,
                redo,
            })
            .collect()
    }

    /// Clicks inside the open panel; returns whether the click was taken.
    pub(super) fn history_click(&mut self, layout: Layout, point: Point) -> bool {
        if !self.history_open || !layout.left.contains(point.x, point.y) {
            return false;
        }
        if let Some(row) = self
            .history_rows(layout)
            .into_iter()
            .find(|row| row.rect.contains(point.x, point.y))
        {
            self.history_jump(row.target);
        }
        true
    }

    pub(super) fn paint_history_panel(&self, painter: &mut Painter, layout: Layout) {
        if !self.history_open {
            return;
        }
        let panel = layout.left;
        painter.rect(panel, PANEL);
        painter.outline(panel, 1.0, BORDER);
        painter.text(17.0, panel.y + 19.0, "HISTORY", 1.5, TEXT);
        painter.text(panel.x + 140.0, panel.y + 21.0, "CTRL+SHIFT+H", 0.8, MUTED);
        let label_chars = ((panel.w - 90.0) / 8.0).max(4.0) as usize;
        for row in self.history_rows(layout) {
            if row.current {
                painter.rect(row.rect, [0.13, 0.29, 0.37, 1.0]);
                painter.outline(row.rect, 1.0, ACCENT);
            }
            let color = if row.redo {
                MUTED
            } else if row.current {
                TEXT
            } else {
                SHAPE
            };
            let marker = if row.current { ">" } else { " " };
            painter.text(
                row.rect.x + 4.0,
                row.rect.y + 5.0,
                &format!("{marker} {}", clip_text(&row.label, label_chars, false)),
                1.0,
                color,
            );
            if let Some(side) = row.side {
                let name = side.name();
                painter.text(
                    row.rect.x + row.rect.w - name.len() as f32 * 6.4 - 4.0,
                    row.rect.y + 6.0,
                    name,
                    0.8,
                    if side == StepSide::Solid {
                        ACCENT
                    } else {
                        MUTED
                    },
                );
            }
        }
    }
}
