//! Feature rows in the Project panel: status glyphs, reorder (Ctrl+Up/Down),
//! suppress (Ctrl+H) and the draggable ROLLBACK row (Ctrl+[ / Ctrl+]).

use super::*;
use project::FeatureStatus;

const ROLLBACK: Color = [0.96, 0.62, 0.30, 1.0];
const DIM: Color = [0.34, 0.39, 0.45, 1.0];

impl App {
    fn feature_total(&self) -> usize {
        self.project.as_ref().map_or(0, ProjectFile::feature_count)
    }

    fn rollback_position(&self) -> usize {
        let total = self.feature_total();
        self.project
            .as_ref()
            .and_then(ProjectFile::rollback_index)
            .unwrap_or(total)
    }

    /// Rows the feature tree takes: one per feature plus the ROLLBACK row.
    pub(super) fn tree_row_count(&self) -> usize {
        match self.feature_total() {
            0 => 0,
            total => total + 1,
        }
    }

    pub(super) fn feature_row(&self, layout: Layout, index: usize) -> Rect {
        let slot = index + usize::from(index >= self.rollback_position());
        layout.solid_row(slot, self.part_rows().len(), self.sketch_count())
    }

    pub(super) fn rollback_row(&self, layout: Layout) -> Rect {
        layout.solid_row(
            self.rollback_position(),
            self.part_rows().len(),
            self.sketch_count(),
        )
    }

    /// Bring the open sketch's edits into the project so dependent features
    /// re-evaluate in the 3D view.
    pub(super) fn sync_sketch_to_project(&mut self) {
        if let Some(project) = &mut self.project
            && project.read_only_reason().is_none()
        {
            project.sync_active_document(&self.document);
        }
    }

    /// Tab: switch between the sketch and the 3D view. Entering the 3D view
    /// syncs the sketch, so features fed by it re-evaluate (incrementally).
    pub(super) fn toggle_solid_view(&mut self) {
        if self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
            .is_none()
        {
            return;
        }
        if !self.solid_preview {
            self.sync_sketch_to_project();
        }
        self.solid_preview = !self.solid_preview;
        if !self.solid_preview {
            self.refresh_reference_edges();
        }
        self.status_message = if self.solid_preview {
            match self
                .project
                .as_ref()
                .map(ProjectFile::feature_statuses)
                .and_then(|statuses| {
                    statuses
                        .iter()
                        .position(|status| matches!(status, FeatureStatus::Error(_)))
                }) {
                Some(index) => format!("3D preview  |  feature {} has an error", index + 1),
                None => "3D extrusion preview  |  Tab: sketch".into(),
            }
        } else {
            "Sketch view  |  Tab: 3D preview".into()
        };
    }

    /// Run a feature tree edit as one undo step.
    fn tree_edit(
        &mut self,
        label: String,
        edit: impl FnOnce(&mut ProjectFile) -> Result<String, String>,
    ) -> bool {
        let Some(project) = self.project.as_mut() else {
            return false;
        };
        if project.read_only_reason().is_some() {
            self.status_message = "Project is read-only".into();
            return false;
        }
        let snapshot = project.feature_tree_snapshot();
        match edit(project) {
            Ok(message) => {
                self.solid_undo
                    .push(SolidUndo::tree(snapshot, self.selected_solid, label));
                self.solid_redo.clear();
                self.status_message = format!("{message}  |  Ctrl+Z to undo  |  Ctrl+S to save");
                true
            }
            Err(error) => {
                self.status_message = format!("Feature tree: {error}");
                false
            }
        }
    }

    pub(super) fn move_selected_feature(&mut self, delta: isize) {
        let index = self.selected_solid;
        let mut moved = index;
        if self.tree_edit("Reorder feature".into(), |project| {
            moved = project.move_feature(index, delta)?;
            Ok(format!(
                "Moved feature {}",
                if delta < 0 { "earlier" } else { "later" }
            ))
        }) {
            self.selected_solid = moved;
        }
    }

    pub(super) fn toggle_selected_suppressed(&mut self) {
        let index = self.selected_solid;
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let label = format!(
            "{} {}",
            if project.feature_suppressed(index) {
                "Unsuppress"
            } else {
                "Suppress"
            },
            project.feature_label(index)
        );
        self.tree_edit(label, |project| {
            let suppressed = !project.feature_suppressed(index);
            project.set_feature_suppressed(index, suppressed)?;
            Ok(if suppressed {
                "Suppressed feature"
            } else {
                "Unsuppressed feature"
            }
            .into())
        });
    }

    pub(super) fn set_rollback(&mut self, position: usize) {
        let total = self.feature_total();
        let position = position.min(total);
        if position == self.rollback_position() {
            return;
        }
        self.tree_edit("Move rollback".into(), |project| {
            project.set_rollback_index((position < total).then_some(position))?;
            Ok(if position < total {
                format!("Rolled back to {position} of {total} features")
            } else {
                "Rollback bar at the end".into()
            })
        });
    }

    pub(super) fn move_rollback(&mut self, delta: isize) {
        let position = self.rollback_position() as isize + delta;
        if position >= 0 {
            self.set_rollback(position as usize);
        }
    }

    /// Drop the dragged ROLLBACK row between the feature rows nearest `y`.
    pub(super) fn drop_rollback(&mut self, y: f32) {
        let layout = self.layout();
        let position = (0..self.feature_total())
            .filter(|&index| {
                let row = self.feature_row(layout, index);
                row.y + row.h * 0.5 < y
            })
            .count();
        self.set_rollback(position);
    }

    /// Clicks on feature rows and the ROLLBACK row; true when handled.
    pub(super) fn click_feature_tree(&mut self, layout: Layout, point: Point) -> bool {
        if self.feature_total() == 0 {
            return false;
        }
        if self.rollback_row(layout).contains(point.x, point.y) {
            self.rollback_drag = true;
            self.status_message = "Drag ROLLBACK between features  |  Ctrl+[ / Ctrl+]".into();
            return true;
        }
        for index in 0..self.feature_total() {
            if self.feature_row(layout, index).contains(point.x, point.y) {
                self.sync_sketch_to_project();
                self.selected_solid = index;
                self.solid_preview = true;
                self.editing = None;
                self.status_message = format!(
                    "Selected feature {}  |  Ctrl+Up/Down reorder  |  Ctrl+H suppress  |  Delete",
                    index + 1
                );
                if let Some(error) = self.selected_feature_error() {
                    self.status_message = format!("Feature {} error: {error}", index + 1);
                }
                return true;
            }
        }
        false
    }

    pub(super) fn paint_feature_tree(&self, painter: &mut Painter, layout: Layout) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let total = project.feature_count();
        if total == 0 {
            return;
        }
        let previews = project.extrusion_previews().unwrap_or_default();
        let statuses = project.feature_statuses();
        for index in 0..total {
            let row = self.feature_row(layout, index);
            if row.y + row.h >= layout.bottom.y {
                continue;
            }
            let active = self.solid_preview && self.selected_solid == index;
            if active {
                painter.rect(row, ACTIVE_ROW);
            }
            let status = statuses
                .get(index)
                .cloned()
                .unwrap_or(FeatureStatus::Active);
            let (glyph, color) = match &status {
                FeatureStatus::Active => ("", if active { TEXT } else { MUTED }),
                FeatureStatus::Suppressed => (" [S]", DIM),
                FeatureStatus::RolledBack => ("", DIM),
                FeatureStatus::Error(_) => (" [!]", CONFLICT),
            };
            let name = previews
                .get(index)
                .map_or("Feature", |preview| preview.name.as_str());
            painter.text(
                row.x + 9.0,
                row.y + 9.0,
                &format!(
                    "{} {} / {name}{glyph}",
                    if active { ">" } else { "v" },
                    project.feature_label(index).to_uppercase(),
                ),
                1.0,
                color,
            );
        }
        let row = self.rollback_row(layout);
        if row.y + row.h < layout.bottom.y {
            let y = if self.rollback_drag {
                self.cursor.y.clamp(row.y - 200.0, row.y + 200.0) - row.h * 0.5
            } else {
                row.y
            };
            let bar = Rect {
                x: row.x,
                y: y + row.h * 0.5 - 1.5,
                w: row.w,
                h: 3.0,
            };
            painter.rect(bar, ROLLBACK);
            let label = Rect {
                x: row.x + 8.0,
                y: y + 6.0,
                w: 86.0,
                h: row.h - 12.0,
            };
            painter.rect(label, PANEL);
            painter.text(label.x + 6.0, label.y + 4.0, "ROLLBACK", 1.0, ROLLBACK);
        }
    }

    /// Message for the selected feature's error, if any.
    pub(super) fn selected_feature_error(&self) -> Option<String> {
        match self
            .project
            .as_ref()?
            .feature_statuses()
            .get(self.selected_solid)?
        {
            FeatureStatus::Error(reason) => Some(reason.clone()),
            _ => None,
        }
    }
}
