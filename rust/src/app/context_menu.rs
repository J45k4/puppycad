//! Right-click context menu of the part 3D view and the assembly view.
//! Items dispatch through the command registry.

use super::*;

/// Right press-release within this many pixels is a click, not an orbit.
pub(super) const RIGHT_CLICK_PIXELS: f32 = 4.0;
const ITEM_H: f32 = 24.0;
const MENU_W: f32 = 210.0;

pub(super) struct ContextMenu {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) items: Vec<(Cmd, &'static str)>,
    pub(super) hovered: usize,
}

impl ContextMenu {
    pub(super) fn item_rect(&self, index: usize) -> Rect {
        Rect {
            x: self.x,
            y: self.y + 4.0 + index as f32 * ITEM_H,
            w: MENU_W,
            h: ITEM_H,
        }
    }

    fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: MENU_W,
            h: self.items.len() as f32 * ITEM_H + 8.0,
        }
    }
}

impl App {
    /// Right button released at `point` after pressing at `start`.
    pub(super) fn right_release(&mut self, start: Point, point: Point) {
        if start.distance(point) >= RIGHT_CLICK_PIXELS {
            return;
        }
        self.open_context_menu(point);
    }

    /// Selects the element under `point` (keeping the selection when it is
    /// already part of it) and opens the menu for it.
    pub(super) fn open_context_menu(&mut self, point: Point) {
        let layout = self.layout();
        if !self.topology_active() || !layout.viewport.contains(point.x, point.y) {
            return;
        }
        let pick = self.topology_pick_at(layout, point);
        let kept = pick.is_some_and(|pick| {
            self.current_topology_selection()
                .is_some_and(|(_, _, selection)| {
                    selection.iter().any(|p| p.element == pick.element)
                })
        });
        if !kept && pick.is_some() {
            let shift = self.modifiers;
            self.modifiers = ModifiersState::empty();
            if self.assembly.is_some() {
                self.assembly_click(layout, point);
            } else {
                self.topology_click(layout, point);
            }
            self.modifiers = shift;
        }
        let mut items: Vec<(Cmd, &'static str)> = Vec::new();
        match (self.assembly.is_some(), pick.map(|pick| pick.element)) {
            (true, Some(_)) => {
                items.push((Cmd::ArmMate, "Mate..."));
                items.push((Cmd::FastenInstance, "Fasten to world"));
                items.push((Cmd::DeleteInstance, "Delete instance"));
            }
            (false, Some(TopoRef::Face(_))) => {
                items.push((Cmd::SketchOnFace, "Sketch on face"));
                items.push((Cmd::SuppressFeature, "Suppress / unsuppress"));
                items.push((Cmd::DeleteFeature, "Delete feature"));
            }
            (false, Some(TopoRef::Edge(_))) => {
                items.push((Cmd::EdgeChamfer, "Chamfer..."));
                items.push((Cmd::EdgeFillet, "Fillet..."));
            }
            (_, Some(TopoRef::Vertex(_))) | (true, None) | (false, None) => {}
        }
        if pick.is_some() {
            items.push((Cmd::Measure, "Measure"));
        } else {
            items.push((Cmd::FitSolid, "Fit all"));
            items.push((Cmd::ToggleProjection, "Toggle perspective"));
            items.push((Cmd::ExportStl, "Export STL"));
            items.push((Cmd::Export3mf, "Export 3MF"));
        }
        let menu = ContextMenu {
            x: point.x.min(layout.viewport.x + layout.viewport.w - MENU_W),
            y: point
                .y
                .min(layout.viewport.y + layout.viewport.h - items.len() as f32 * ITEM_H - 8.0),
            items,
            hovered: 0,
        };
        self.context_menu = Some(menu);
    }

    /// Whether a menu item can run now (dimmed otherwise).
    pub(super) fn context_item_available(&self, id: Cmd) -> bool {
        match id {
            Cmd::Measure => self
                .current_topology_selection()
                .is_some_and(|(_, _, selection)| selection.len() == 2),
            Cmd::SuppressFeature | Cmd::DeleteFeature => self.selected_feature_row().is_some(),
            _ => self.command_available(command(id)),
        }
    }

    /// Runs the item at `index` when available; closes the menu.
    fn run_context_item(&mut self, index: usize) {
        let Some(menu) = self.context_menu.take() else {
            return;
        };
        if let Some(&(id, _)) = menu.items.get(index) {
            if self.context_item_available(id) {
                self.run_command(id);
            } else {
                self.context_menu = Some(menu);
            }
        }
    }

    /// Up/Down/Enter/Esc while the menu is open; true when consumed.
    pub(super) fn context_menu_key(&mut self, code: KeyCode) -> bool {
        let Some(menu) = &mut self.context_menu else {
            return false;
        };
        let count = menu.items.len().max(1);
        match code {
            KeyCode::ArrowDown => menu.hovered = (menu.hovered + 1) % count,
            KeyCode::ArrowUp => menu.hovered = (menu.hovered + count - 1) % count,
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let index = menu.hovered;
                self.run_context_item(index);
            }
            KeyCode::Escape => self.context_menu = None,
            _ => {}
        }
        true
    }

    /// Left click while the menu is open: run the item under it, or close.
    pub(super) fn context_menu_click(&mut self, point: Point) -> bool {
        let Some(menu) = &self.context_menu else {
            return false;
        };
        match (0..menu.items.len()).find(|&i| menu.item_rect(i).contains(point.x, point.y)) {
            Some(index) => self.run_context_item(index),
            None => self.context_menu = None,
        }
        true
    }

    pub(super) fn context_menu_hover(&mut self) {
        let cursor = self.cursor;
        if let Some(menu) = &mut self.context_menu
            && let Some(index) =
                (0..menu.items.len()).find(|&i| menu.item_rect(i).contains(cursor.x, cursor.y))
        {
            menu.hovered = index;
        }
    }

    pub(super) fn paint_context_menu(&self, painter: &mut Painter) {
        let Some(menu) = &self.context_menu else {
            return;
        };
        painter.rect(menu.rect(), PANEL_LIGHT);
        painter.outline(menu.rect(), 1.0, ACCENT);
        for (index, (id, label)) in menu.items.iter().enumerate() {
            let row = menu.item_rect(index);
            if index == menu.hovered {
                painter.rect(row, ACTIVE);
            }
            let color = if self.context_item_available(*id) {
                TEXT
            } else {
                MUTED
            };
            painter.text(row.x + 10.0, row.y + 8.0, label, 1.0, color);
        }
    }

    /// Feature tree row index when a feature row is selected (part view).
    fn selected_feature_row(&self) -> Option<usize> {
        let project = self.project.as_ref()?;
        project
            .feature_node(self.selected_solid)
            .map(|_| self.selected_solid)
    }

    /// Measure: the distance / angle of the two selected elements.
    pub(super) fn measure_selection(&mut self) {
        let Some((scene, topology, selection)) = self.current_topology_selection() else {
            self.status_message = "Measure: select two vertices, edges or faces".into();
            return;
        };
        self.status_message = match selection.as_slice() {
            [a, b] => format!(
                "Measure: {}",
                topology_view::measure_lines(topology.measure(&scene, a.element, b.element))
                    .join("  ")
            ),
            _ => "Measure: Shift+click a second element".into(),
        };
    }

    /// Delete feature (context menu): removes the selected feature row
    /// even while vertices, edges or faces are selected.
    pub(super) fn delete_feature_command(&mut self) {
        self.clear_topology_selection();
        self.run_command(Cmd::Delete);
    }

    /// Fit all in a 3D view: default zoom and pan, same orientation.
    pub(super) fn fit_solid(&mut self) {
        let fresh = view3d::OrbitView::default();
        self.solid_view.zoom = fresh.zoom;
        self.solid_view.pan = fresh.pan;
        self.solid_view.pivot = fresh.pivot;
        self.status_message = "Fitted the 3D view".into();
    }

    pub(super) fn toggle_projection(&mut self) {
        self.solid_view.perspective = !self.solid_view.perspective;
        self.status_message = if self.solid_view.perspective {
            "Perspective view"
        } else {
            "Orthographic view"
        }
        .into();
    }
}
