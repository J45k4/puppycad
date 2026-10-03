//! Multi-selection: an ordered set of shape ids whose last member is the
//! primary `Document::selected`. Code that assigns `selected` directly
//! collapses the selection to that single shape, because the extra members
//! are only honoured while `selected == selection_primary`.

use super::*;

/// Kinds of shapes click and box selection may pick. Filtered-out shapes
/// still render; they just cannot be selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionFilter {
    /// Regular (non-construction) shapes other than sketch points.
    pub geometry: bool,
    /// Construction shapes other than sketch points.
    pub construction: bool,
    /// Sketch points (`Shape::Point`), construction or not.
    pub points: bool,
}

impl Default for SelectionFilter {
    fn default() -> Self {
        Self {
            geometry: true,
            construction: true,
            points: true,
        }
    }
}

impl Document {
    /// Selected shape ids in selection order; the primary (`selected`) is last.
    pub fn selected_ids(&self) -> Vec<u32> {
        let exists = |id: &u32| self.shapes.iter().any(|shape| shape.id() == *id);
        let mut ids = Vec::new();
        if self.selected.is_some() && self.selected == self.selection_primary {
            ids.extend(
                self.selection_extra
                    .iter()
                    .copied()
                    .filter(|id| Some(*id) != self.selected && exists(id)),
            );
        }
        if let Some(id) = self.selected.filter(exists) {
            ids.push(id);
        }
        ids
    }

    pub fn selection_count(&self) -> usize {
        self.selected_ids().len()
    }

    pub fn is_selected(&self, id: u32) -> bool {
        self.selected_ids().contains(&id)
    }

    /// Replace the selection with `ids`; the last one becomes primary.
    pub fn set_selection(&mut self, ids: &[u32]) {
        let mut unique: Vec<u32> = Vec::new();
        for id in ids {
            unique.retain(|existing| existing != id);
            unique.push(*id);
        }
        self.selected = unique.pop();
        self.selection_primary = self.selected;
        self.selection_extra = unique;
        self.selected_spline_point = None;
    }

    pub fn select_only(&mut self, id: Option<u32>) {
        self.set_selection(&id.into_iter().collect::<Vec<_>>());
    }

    pub fn clear_selection(&mut self) {
        self.set_selection(&[]);
    }

    /// Add `ids` to the selection; the last added becomes primary.
    pub fn add_to_selection(&mut self, ids: &[u32]) {
        let mut all = self.selected_ids();
        all.extend_from_slice(ids);
        self.set_selection(&all);
    }

    /// Shift+click: add `id` (making it primary) or remove it if selected.
    pub fn toggle_selected(&mut self, id: u32) {
        let mut all = self.selected_ids();
        if all.contains(&id) {
            all.retain(|existing| *existing != id);
        } else {
            all.push(id);
        }
        self.set_selection(&all);
    }

    /// Whether the selection filter lets `shape` be picked.
    pub fn selectable(&self, shape: &Shape) -> bool {
        let filter = self.selection_filter;
        if matches!(shape, Shape::Point { .. }) {
            filter.points
        } else if self.construction.contains(&shape.id()) {
            filter.construction
        } else {
            filter.geometry
        }
    }

    /// Topmost selectable shape under `point`.
    pub fn pick_at(&self, point: Point) -> Option<u32> {
        self.shapes
            .iter()
            .rev()
            .find(|shape| self.selectable(shape) && shape.hit(point, 5.0 / self.zoom))
            .map(Shape::id)
    }

    /// Whether `point` is on a drag handle of the primary selection: an
    /// anchor or a spline control/fit point. A press there edits the shape
    /// instead of starting a box selection.
    pub fn handle_at(&self, point: Point) -> bool {
        if self.anchor_at(point).is_some() {
            return true;
        }
        let tolerance = 7.0 / self.zoom.max(0.2);
        matches!(
            self.selected
                .and_then(|id| self.shapes.iter().find(|shape| shape.id() == id)),
            Some(Shape::Spline { points, .. })
                if points.iter().any(|handle| handle.distance(point) <= tolerance)
        )
    }

    /// Shift+click at `point`: toggle the shape under it. Returns whether a
    /// shape was hit.
    pub fn toggle_at(&mut self, point: Point) -> bool {
        match self.pick_at(point) {
            Some(id) => {
                self.toggle_selected(id);
                true
            }
            None => false,
        }
    }

    /// Selectable shapes picked by a box dragged from `from` to `to` (world
    /// coordinates). Dragging rightwards (window) picks only fully enclosed
    /// shapes; dragging leftwards (crossing) picks anything the box touches.
    pub fn shapes_in_box(&self, from: Point, to: Point) -> Vec<u32> {
        let min = Point {
            x: from.x.min(to.x),
            y: from.y.min(to.y),
        };
        let max = Point {
            x: from.x.max(to.x),
            y: from.y.max(to.y),
        };
        let window = to.x >= from.x;
        self.shapes
            .iter()
            .filter(|shape| self.selectable(shape))
            .filter(|shape| {
                if window {
                    shape.bounds().is_some_and(|(lo, hi)| {
                        lo.x >= min.x && lo.y >= min.y && hi.x <= max.x && hi.y <= max.y
                    })
                } else {
                    box_crosses(shape, min, max)
                }
            })
            .map(Shape::id)
            .collect()
    }

    /// Finish a box selection; `additive` (Shift) keeps the current selection.
    /// Returns how many shapes the box picked.
    pub fn box_select(&mut self, from: Point, to: Point, additive: bool) -> usize {
        let picked = self.shapes_in_box(from, to);
        if additive {
            self.add_to_selection(&picked);
        } else {
            self.set_selection(&picked);
        }
        picked.len()
    }

    /// Run a single-shape edit for every selected shape as one undo step.
    /// The edit sees each shape as the sole selection; the ids it leaves
    /// selected form the new selection. All-or-nothing: if any shape's edit
    /// fails, everything is rolled back.
    pub(super) fn for_each_selected(&mut self, mut edit: impl FnMut(&mut Self) -> bool) -> bool {
        let ids = self.selected_ids();
        if ids.len() <= 1 {
            return edit(self);
        }
        let before = self.snapshot();
        let history_len = self.history.len();
        let mut result = Vec::new();
        for id in &ids {
            self.select_only(Some(*id));
            if !edit(self) {
                self.history.truncate(history_len);
                self.restore(before);
                self.set_selection(&ids);
                return false;
            }
            if let Some(selected) = self.selected {
                result.push(selected);
            }
        }
        self.history.truncate(history_len);
        self.history.push(before);
        self.future.clear();
        self.set_selection(&result);
        true
    }
}

/// Whether any part of `shape` lies inside the box `min`..`max`.
fn box_crosses(shape: &Shape, min: Point, max: Point) -> bool {
    let Some((lo, hi)) = shape.bounds() else {
        return false;
    };
    if hi.x < min.x || lo.x > max.x || hi.y < min.y || lo.y > max.y {
        return false;
    }
    if lo.x >= min.x && hi.x <= max.x && lo.y >= min.y && hi.y <= max.y {
        return true;
    }
    // Sample a grid over the box: any outline hit counts as crossing. Filled
    // hit areas (circles, rectangles, ...) count only near their edge, so a
    // box wholly inside a circle does not pick it.
    let steps = 24;
    let tolerance = ((max.x - min.x).max(max.y - min.y) / steps as f32).max(1e-3);
    for i in 0..=steps {
        for j in 0..=steps {
            let p = Point {
                x: min.x + (max.x - min.x) * i as f32 / steps as f32,
                y: min.y + (max.y - min.y) * j as f32 / steps as f32,
            };
            if shape.hit(p, tolerance) && !shape.hit(p, -tolerance) {
                return true;
            }
        }
    }
    false
}
