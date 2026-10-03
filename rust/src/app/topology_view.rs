//! Hover, selection and painting of vertices, edges and faces in the part
//! 3D view and the assembly view.

use super::*;
use std::rc::Rc;

/// Side of the hovered vertex square in pixels (selected: +2).
const VERTEX_SQUARE: f32 = 9.0;

impl App {
    /// Topology picking is live: the part 3D view with no gesture armed,
    /// or the assembly view.
    pub(super) fn topology_active(&self) -> bool {
        self.projects_page.is_none()
            && (self.assembly.is_some()
                || (self.solid_preview
                    && self.extrude_draft.is_none()
                    && self.revolve_draft.is_none()
                    && self.edge_pick.is_none()
                    && !self.body_pick))
    }

    /// Scene, projector, scene hash and topology of the current 3D view.
    pub(super) fn topology_context(
        &self,
        layout: Layout,
    ) -> Option<(Rc<SolidScene>, SolidProjector, u64, Rc<Topology>)> {
        let (scene, projector) = if self.assembly.is_some() {
            self.assembly_projector(layout)?
        } else {
            let (scene, projector) = self.solid_projector(layout)?;
            (Rc::new(scene), projector)
        };
        let (hash, topology) = cached_topology(&scene);
        Some((scene, projector, hash, topology))
    }

    /// The element under screen `point` (vertex > edge > face).
    pub(super) fn topology_pick_at(&self, layout: Layout, point: Point) -> Option<TopoPick> {
        if !self.topology_active() || !layout.viewport.contains(point.x, point.y) {
            return None;
        }
        let (scene, projector, _, topology) = self.topology_context(layout)?;
        topology.pick(&scene, &projector, point)
    }

    /// The selection, when it was made on the scene with hash `hash`.
    pub(super) fn topology_selection_for(&self, hash: u64) -> &[TopoPick] {
        if hash == self.topo_key {
            &self.topo_selection
        } else {
            &[]
        }
    }

    /// The current selection with the scene it refers to.
    pub(super) fn current_topology_selection(
        &self,
    ) -> Option<(Rc<SolidScene>, Rc<Topology>, Vec<TopoPick>)> {
        if self.topo_selection.is_empty() {
            return None;
        }
        let (scene, _, hash, topology) = self.topology_context(self.layout())?;
        let selection = self.topology_selection_for(hash).to_vec();
        (!selection.is_empty()).then_some((scene, topology, selection))
    }

    /// Outward normal at a pick: the hit triangle's normal for a face, an
    /// adjacent planar face's normal for a vertex.
    pub(super) fn pick_normal(
        scene: &SolidScene,
        topology: &Topology,
        pick: &TopoPick,
    ) -> Option<[f32; 3]> {
        match (pick.element, pick.triangle) {
            (_, Some(triangle)) => {
                let [a, b, c] = scene.triangles[triangle];
                triangle_normal(a, b, c).map(|n| {
                    let length = norm3(n);
                    n.map(|c| c / length)
                })
            }
            (TopoRef::Face(face), None) => Some(topology.faces[face].normal),
            (TopoRef::Vertex(vertex), None) => topology.vertex_normal(scene, vertex),
            _ => None,
        }
    }

    /// Web face reference and label of a picked face (part view only).
    pub(super) fn pick_web_face(
        &self,
        scene: &SolidScene,
        topology: &Topology,
        pick: &TopoPick,
    ) -> Option<(project::FaceRef, String)> {
        if self.assembly.is_some() || !matches!(pick.element, TopoRef::Face(_)) {
            return None;
        }
        let normal = Self::pick_normal(scene, topology, pick)?;
        self.project.as_ref()?.face_at(pick.point, normal)
    }

    fn face_name(&self, scene: &SolidScene, topology: &Topology, pick: &TopoPick) -> String {
        match (self.pick_web_face(scene, topology, pick), pick.element) {
            (Some((_, label)), _) => {
                let lower = label.to_lowercase();
                lower.strip_suffix(" face").unwrap_or(&lower).to_string()
            }
            (None, TopoRef::Face(index)) => format!("#{index}"),
            _ => String::new(),
        }
    }

    pub(super) fn describe_pick(
        &self,
        scene: &SolidScene,
        topology: &Topology,
        pick: &TopoPick,
    ) -> String {
        topology.describe(pick.element, &self.face_name(scene, topology, pick))
    }

    /// Left click in a 3D view: select the hovered element; Shift adds or
    /// removes it; empty space clears (unless Shift is held).
    pub(super) fn topology_click(&mut self, layout: Layout, point: Point) -> Option<TopoPick> {
        let (scene, projector, hash, topology) = self.topology_context(layout)?;
        let pick = topology.pick(&scene, &projector, point);
        let shift = self.modifiers.shift_key();
        if hash != self.topo_key {
            self.topo_selection.clear();
            self.topo_key = hash;
        }
        match pick {
            Some(pick) if shift => {
                if let Some(index) = self
                    .topo_selection
                    .iter()
                    .position(|p| p.element == pick.element)
                {
                    self.topo_selection.remove(index);
                } else {
                    self.topo_selection.push(pick);
                }
            }
            Some(pick) => self.topo_selection = vec![pick],
            None if shift => {}
            None => self.topo_selection.clear(),
        }
        self.status_message = match self.topo_selection.as_slice() {
            [] => "Selection cleared".into(),
            [one] => format!(
                "Selected {}  |  Shift+click adds  |  Esc clears",
                self.describe_pick(&scene, &topology, one)
            ),
            [a, b] => {
                let measure = measure_lines(topology.measure(&scene, a.element, b.element));
                format!(
                    "2 elements selected{}  |  Shift+click adds or removes  |  Esc clears",
                    measure
                        .iter()
                        .map(|line| format!(
                            "  {}",
                            line.split_whitespace().collect::<Vec<_>>().join(" ")
                        ))
                        .collect::<String>()
                )
            }
            many => format!(
                "{} elements selected  |  Shift+click adds or removes  |  Esc clears",
                many.len()
            ),
        };
        pick
    }

    /// Clears the topology selection; true when something was selected.
    pub(super) fn clear_topology_selection(&mut self) -> bool {
        let had = !self.topo_selection.is_empty();
        self.topo_selection.clear();
        had
    }

    /// Track the hovered element; its description goes to the status line
    /// when it changes.
    pub(super) fn update_topology_hover(&mut self) {
        let layout = self.layout();
        let hovered = self.topology_pick_at(layout, self.cursor).and_then(|pick| {
            let (scene, _, _, topology) = self.topology_context(layout)?;
            Some((pick, self.describe_pick(&scene, &topology, &pick)))
        });
        let element = hovered.as_ref().map(|(pick, _)| pick.element);
        if element != self.topo_hover {
            self.topo_hover = element;
            if let Some((_, text)) = hovered {
                self.status_message = text;
            }
        }
    }

    /// Selected elements in the accent color, then the hovered element.
    pub(super) fn paint_topology(
        &self,
        painter: &mut Painter,
        layout: Layout,
        scene: &SolidScene,
        projector: &SolidProjector,
    ) {
        let (hash, topology) = cached_topology(scene);
        for pick in self.topology_selection_for(hash) {
            if topology.exists(pick.element) {
                paint_element(painter, projector, &topology, pick.element, ACCENT, true);
            }
        }
        let Some(pick) = self.topology_pick_at(layout, self.cursor) else {
            if self.assembly.is_none() {
                self.paint_face_hover(painter, layout, projector);
            }
            return;
        };
        let web = self.pick_web_face(scene, &topology, &pick);
        let part_face = matches!(pick.element, TopoRef::Face(_)) && self.assembly.is_none();
        let color = if part_face && web.is_none() {
            MUTED
        } else {
            FACE_HOVER
        };
        paint_element(painter, projector, &topology, pick.element, color, false);
        let label = match &web {
            Some((_, label)) => format!("{}  |  4 SKETCH ON FACE", label.to_uppercase()),
            None if part_face => "FACE (NOT ON AN EXTRUSION PRISM)".into(),
            None => self.describe_pick(scene, &topology, &pick).to_uppercase(),
        };
        let y = if self.assembly.is_some() { 64.0 } else { 68.0 };
        painter.text(
            layout.viewport.x + 18.0,
            layout.viewport.y + y,
            &label,
            1.0,
            color,
        );
    }
}

impl App {
    /// Properties lines for the topology selection: each element, then
    /// totals for several.
    pub(super) fn topology_panel_lines(&self) -> Vec<String> {
        let Some((scene, topology, selection)) = self.current_topology_selection() else {
            return Vec::new();
        };
        let mut lines = Vec::new();
        if selection.len() > 1 {
            let edges: f32 = selection
                .iter()
                .filter_map(|p| match p.element {
                    TopoRef::Edge(i) => Some(topology.edge_length(i)),
                    _ => None,
                })
                .sum();
            let faces: f32 = selection
                .iter()
                .filter_map(|p| match p.element {
                    TopoRef::Face(i) => Some(topology.faces[i].area),
                    _ => None,
                })
                .sum();
            lines.push(format!("{} SELECTED", selection.len()));
            if let [a, b] = selection.as_slice() {
                lines.extend(measure_lines(
                    topology.measure(&scene, a.element, b.element),
                ));
            }
            if edges > 0.0 {
                lines.push(format!("TOTAL LEN  {edges:.2} mm"));
            }
            if faces > 0.0 {
                lines.push(format!("TOTAL AREA {faces:.2} mm²"));
            }
            lines.push(String::new());
        }
        for pick in selection.iter().take(4) {
            let web = self.pick_web_face(&scene, &topology, pick);
            let id = web.as_ref().map(|(face, _)| face.face_id.as_str());
            lines.extend(topology.properties(pick.element, id));
        }
        if selection.len() > 4 {
            lines.push(format!("+{} MORE", selection.len() - 4));
        }
        lines
    }

    /// Properties panel of the part view while elements are selected.
    pub(super) fn paint_topology_panel(&self, painter: &mut Painter, layout: Layout) -> bool {
        if !self.topology_active() {
            return false;
        }
        let lines = self.topology_panel_lines();
        if lines.is_empty() {
            return false;
        }
        let x = layout.right.x + 18.0;
        painter.text(x, layout.right.y + 66.0, "SELECTION", 1.7, ACCENT);
        paint_lines(painter, x, layout.right.y + 104.0, &lines, layout.right.w);
        true
    }
}

/// "DIST" / "ANGLE" lines of a two-element measurement.
pub(super) fn measure_lines(measure: Measure) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(distance) = measure.distance {
        lines.push(format!("DIST  {distance:.3} mm"));
    }
    if let Some(angle) = measure.angle {
        lines.push(format!("ANGLE {angle:.2} deg"));
    }
    if lines.is_empty() {
        lines.push("NO MEASUREMENT".into());
    }
    lines
}

/// Properties lines, clipped to the panel width.
pub(super) fn paint_lines(painter: &mut Painter, x: f32, y: f32, lines: &[String], width: f32) {
    let room = ((width - 30.0) / 8.0).max(8.0) as usize;
    for (row, line) in lines.iter().enumerate() {
        painter.text(
            x,
            y + row as f32 * 20.0,
            &clip_text(line, room, false),
            1.0,
            TEXT,
        );
    }
}

/// A vertex as a filled square, an edge as a thick polyline, a face as its
/// outline.
fn paint_element(
    painter: &mut Painter,
    projector: &SolidProjector,
    topology: &Topology,
    element: TopoRef,
    color: Color,
    selected: bool,
) {
    match element {
        TopoRef::Vertex(index) => {
            let [x, y] = projector.screen(topology.vertices[index].point);
            let size = VERTEX_SQUARE + if selected { 2.0 } else { 0.0 };
            painter.rect(
                Rect {
                    x: x - size * 0.5,
                    y: y - size * 0.5,
                    w: size,
                    h: size,
                },
                color,
            );
        }
        TopoRef::Edge(index) => {
            let width = if selected { 4.5 } else { 4.0 };
            for pair in topology.edges[index].points.windows(2) {
                let (a, b) = (projector.screen(pair[0]), projector.screen(pair[1]));
                painter.line(a[0], a[1], b[0], b[1], width, color);
            }
        }
        TopoRef::Face(index) => {
            for [a, b] in &topology.faces[index].outline {
                let (a, b) = (projector.screen(*a), projector.screen(*b));
                painter.line(a[0], a[1], b[0], b[1], 3.0, color);
            }
        }
    }
}
