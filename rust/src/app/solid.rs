use super::*;

pub(super) fn triangle_normal(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> Option<[f32; 3]> {
    let u = view3d::sub(b, a);
    let v = view3d::sub(c, a);
    let normal = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    (normal.iter().map(|c| c * c).sum::<f32>() > 1e-12).then_some(normal)
}

/// Evaluated solid bodies as world-space triangles (CAD millimeters, Z up).
pub(super) struct SolidScene {
    pub(super) bodies: usize,
    pub(super) vertices: Vec<[f32; 3]>,
    pub(super) triangles: Vec<[[f32; 3]; 3]>,
    /// Body index of each triangle.
    pub(super) triangle_body: Vec<usize>,
}

impl SolidScene {
    pub(super) fn from_manifolds(solids: &[Manifold]) -> Self {
        let mut vertices = Vec::new();
        let mut triangles = Vec::new();
        let mut triangle_body = Vec::new();
        for (body, mesh) in solids.iter().map(|solid| solid.get_mesh_gl(0)).enumerate() {
            let points: Vec<[f32; 3]> = mesh
                .vert_properties
                .chunks_exact(mesh.num_prop as usize)
                .map(|p| [p[0], p[1], p[2]])
                .collect();
            for indices in mesh.tri_verts.as_chunks::<3>().0 {
                triangle_body.push(body);
                triangles.push(std::array::from_fn(|corner| {
                    points[indices[corner] as usize]
                }));
            }
            vertices.extend(points);
        }
        Self {
            bodies: solids.len(),
            vertices,
            triangles,
            triangle_body,
        }
    }
}

/// Projects the orbit view using a scene-fitted scale and optional perspective.
pub(super) struct SolidProjector {
    pub(super) view: OrbitView,
    pub(super) center: [f32; 3],
    pub(super) scale: f32,
    pub(super) origin: [f32; 2],
    pub(super) camera_distance: f32,
}

impl SolidProjector {
    pub(super) fn new(view: OrbitView, vertices: &[[f32; 3]], viewport: Rect) -> Option<Self> {
        if vertices.is_empty() {
            return None;
        }
        let min: [f32; 3] = std::array::from_fn(|axis| {
            vertices
                .iter()
                .map(|p| p[axis])
                .fold(f32::INFINITY, f32::min)
        });
        let max: [f32; 3] = std::array::from_fn(|axis| {
            vertices
                .iter()
                .map(|p| p[axis])
                .fold(f32::NEG_INFINITY, f32::max)
        });
        let center = std::array::from_fn(|axis| (min[axis] + max[axis]) * 0.5);
        let radius = (0..3)
            .map(|axis| (max[axis] - min[axis]) * 0.5)
            .map(|half| half * half)
            .sum::<f32>()
            .sqrt()
            .max(1.0);
        Some(Self {
            view,
            center,
            camera_distance: radius * 4.0,
            scale: viewport.w.min(viewport.h) * 0.42 / radius * view.zoom,
            origin: [viewport.x + viewport.w * 0.5, viewport.y + viewport.h * 0.5],
        })
    }

    pub(super) fn view_point(&self, world: [f32; 3]) -> [f32; 3] {
        self.view
            .model_to_view(view3d::cad_to_model(view3d::sub(world, self.center)))
    }

    pub(super) fn perspective_factor(&self, depth: f32) -> f32 {
        if self.view.perspective {
            self.camera_distance / (self.camera_distance - depth).max(self.camera_distance * 0.01)
        } else {
            1.0
        }
    }

    pub(super) fn screen(&self, world: [f32; 3]) -> [f32; 2] {
        let v = self.view_point(world);
        [
            self.origin[0] + v[0] * self.scale * self.perspective_factor(v[2]),
            self.origin[1] - v[1] * self.scale * self.perspective_factor(v[2]),
        ]
    }

    /// View-space depth; larger is nearer the viewer.
    pub(super) fn depth(&self, world: [f32; 3]) -> f32 {
        self.view_point(world)[2]
    }

    /// Cosine between a world-space normal and the direction toward the viewer.
    pub(super) fn facing(&self, normal: [f32; 3]) -> f32 {
        let n = self.view.rotate(view3d::cad_to_model(normal));
        n[2] / (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6)
    }

    pub(super) fn facing_at(&self, normal: [f32; 3], world: [f32; 3]) -> f32 {
        if !self.view.perspective {
            return self.facing(normal);
        }
        let n = self.view.rotate(view3d::cad_to_model(normal));
        let p = self.view_point(world);
        let toward = [-p[0], -p[1], self.camera_distance - p[2]];
        let dot: f32 = (0..3).map(|i| n[i] * toward[i]).sum();
        let length = |v: [f32; 3]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (length(n) * length(toward)).max(1e-6)
    }

    pub(super) fn units_per_pixel(&self) -> f32 {
        1.0 / (self.scale * self.perspective_factor(self.view.model_to_view(self.view.pivot)[2]))
    }

    /// Model point under screen `point` at view-space `depth`.
    pub(super) fn model_at(&self, point: Point, depth: f32) -> [f32; 3] {
        self.view.view_to_model([
            (point.x - self.origin[0]) / (self.scale * self.perspective_factor(depth)),
            (self.origin[1] - point.y) / (self.scale * self.perspective_factor(depth)),
            depth,
        ])
    }

    /// Nearest scene triangle under screen `point`: its index, view depth and the
    /// world-space hit point (perspective-correct barycentric interpolation).
    pub(super) fn pick_triangle(
        &self,
        scene: &SolidScene,
        point: Point,
    ) -> Option<(usize, f32, [f32; 3])> {
        let mut nearest: Option<(usize, f32, [f32; 3])> = None;
        for (index, &[a, b, c]) in scene.triangles.iter().enumerate() {
            let [pa, pb, pc] = [a, b, c].map(|p| (self.screen(p), self.depth(p)));
            let det = (pb.0[1] - pc.0[1]) * (pa.0[0] - pc.0[0])
                + (pc.0[0] - pb.0[0]) * (pa.0[1] - pc.0[1]);
            if det.abs() < 1e-9 {
                continue;
            }
            let wa = ((pb.0[1] - pc.0[1]) * (point.x - pc.0[0])
                + (pc.0[0] - pb.0[0]) * (point.y - pc.0[1]))
                / det;
            let wb = ((pc.0[1] - pa.0[1]) * (point.x - pc.0[0])
                + (pa.0[0] - pc.0[0]) * (point.y - pc.0[1]))
                / det;
            let wc = 1.0 - wa - wb;
            if wa < -1e-4 || wb < -1e-4 || wc < -1e-4 {
                continue;
            }
            let mut weights = [wa, wb, wc];
            let depth = if self.view.perspective {
                let inverse = [pa.1, pb.1, pc.1].map(|d| 1.0 / (self.camera_distance - d));
                let sum: f32 = (0..3).map(|i| weights[i] * inverse[i]).sum();
                for i in 0..3 {
                    weights[i] = weights[i] * inverse[i] / sum;
                }
                self.camera_distance - 1.0 / sum
            } else {
                wa * pa.1 + wb * pb.1 + wc * pc.1
            };
            if nearest.is_none_or(|(_, best, _)| depth > best) {
                let hit = std::array::from_fn(|axis| {
                    weights[0] * a[axis] + weights[1] * b[axis] + weights[2] * c[axis]
                });
                nearest = Some((index, depth, hit));
            }
        }
        nearest
    }

    /// Body under screen `point`.
    pub(super) fn pick_body(&self, scene: &SolidScene, point: Point) -> Option<usize> {
        self.pick_triangle(scene, point)
            .map(|(index, _, _)| scene.triangle_body[index])
    }

    /// Orbit pivot for a drag starting at `point`: the nearest surface under the cursor,
    /// or the current pivot depth when the cursor misses (web: `getOrbitAnchorPoint`).
    pub(super) fn pick_pivot(&self, scene: &SolidScene, point: Point) -> [f32; 3] {
        let depth = self.pick_triangle(scene, point).map_or_else(
            || view3d::add(self.view.pan, self.view.pivot)[2],
            |hit| hit.1,
        );
        self.model_at(point, depth)
    }
}

/// Planar face under the cursor in the 3D preview.
#[derive(Clone, Debug)]
pub(super) struct FaceHover {
    /// Web face reference, when the face belongs to an extrusion prism.
    pub(super) face: Option<project::FaceRef>,
    pub(super) label: String,
    /// Boundary segments of the coplanar, connected triangles that were hit.
    pub(super) outline: Vec<[[f32; 3]; 2]>,
}

/// The coplanar, edge-connected triangles around `seed` and their boundary
/// segments (edges used by one triangle of the group).
pub(super) fn coplanar_face(scene: &SolidScene, seed: usize) -> (Vec<usize>, Vec<[[f32; 3]; 2]>) {
    let [a, b, c] = scene.triangles[seed];
    let Some(normal) = triangle_normal(a, b, c).map(unit) else {
        return (vec![seed], Vec::new());
    };
    let offset = dot3(normal, a);
    let coplanar: Vec<usize> = (0..scene.triangles.len())
        .filter(|&index| {
            let [p, q, r] = scene.triangles[index];
            triangle_normal(p, q, r).is_some_and(|n| dot3(unit(n), normal) > 0.9999)
                && [p, q, r]
                    .iter()
                    .all(|v| (dot3(normal, *v) - offset).abs() < 1e-3)
        })
        .collect();
    let key = |p: [f32; 3]| p.map(f32::to_bits);
    let edge_key = |p: [f32; 3], q: [f32; 3]| {
        let (u, v) = (key(p), key(q));
        if u <= v { (u, v) } else { (v, u) }
    };
    let mut group = vec![seed];
    let mut in_group = std::collections::HashSet::from([seed]);
    let mut edges = std::collections::HashSet::new();
    for [p, q, r] in [scene.triangles[seed]] {
        for (u, v) in [(p, q), (q, r), (r, p)] {
            edges.insert(edge_key(u, v));
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for &index in &coplanar {
            if in_group.contains(&index) {
                continue;
            }
            let [p, q, r] = scene.triangles[index];
            if [(p, q), (q, r), (r, p)]
                .iter()
                .any(|&(u, v)| edges.contains(&edge_key(u, v)))
            {
                in_group.insert(index);
                group.push(index);
                for (u, v) in [(p, q), (q, r), (r, p)] {
                    edges.insert(edge_key(u, v));
                }
                changed = true;
            }
        }
    }
    let mut counts: std::collections::HashMap<_, (usize, [[f32; 3]; 2])> =
        std::collections::HashMap::new();
    for &index in &group {
        let [p, q, r] = scene.triangles[index];
        for (u, v) in [(p, q), (q, r), (r, p)] {
            counts.entry(edge_key(u, v)).or_insert((0, [u, v])).0 += 1;
        }
    }
    let outline = counts
        .into_values()
        .filter(|(count, _)| *count == 1)
        .map(|(_, segment)| segment)
        .collect();
    (group, outline)
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(v: [f32; 3]) -> [f32; 3] {
    let length = dot3(v, v).sqrt().max(1e-12);
    v.map(|c| c / length)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum SolidDrag {
    Rotate,
    Pan { units_per_pixel: f32 },
}

impl App {
    pub(super) fn solid_projector(&self, layout: Layout) -> Option<(SolidScene, SolidProjector)> {
        let solids = self.project.as_ref()?.evaluated_solids()?;
        let scene = SolidScene::from_manifolds(&solids);
        let projector = SolidProjector::new(self.solid_view, &scene.vertices, layout.viewport)?;
        Some((scene, projector))
    }

    /// Starts a 3D preview drag for a mouse button pressed at `point`, like the web part
    /// view: right-drag orbits around the surface under the cursor, middle-drag pans.
    pub(super) fn begin_solid_drag(
        &mut self,
        layout: Layout,
        point: Point,
        button: MouseButton,
    ) -> bool {
        self.solid_drag = None;
        if !self.solid_preview
            || self.projects_page.is_some()
            || !layout.viewport.contains(point.x, point.y)
        {
            return false;
        }
        let Some((scene, projector)) = self.solid_projector(layout) else {
            return false;
        };
        self.solid_drag = match button {
            MouseButton::Right => {
                self.solid_view
                    .set_pivot(projector.pick_pivot(&scene, point));
                Some(SolidDrag::Rotate)
            }
            MouseButton::Middle => Some(SolidDrag::Pan {
                units_per_pixel: projector.units_per_pixel(),
            }),
            _ => None,
        };
        self.solid_drag.is_some()
    }

    /// The planar face under the cursor in the 3D preview.
    pub(super) fn face_hover(&self, layout: Layout) -> Option<FaceHover> {
        if !self.solid_preview
            || self.projects_page.is_some()
            || !layout.viewport.contains(self.cursor.x, self.cursor.y)
        {
            return None;
        }
        let (scene, projector) = self.solid_projector(layout)?;
        let (index, _, hit) = projector.pick_triangle(&scene, self.cursor)?;
        let [a, b, c] = scene.triangles[index];
        let normal = triangle_normal(a, b, c)?;
        let (_, outline) = coplanar_face(&scene, index);
        let picked = self.project.as_ref()?.face_at(hit, normal);
        Some(FaceHover {
            label: picked.as_ref().map_or_else(
                || "FACE (NOT ON AN EXTRUSION PRISM)".into(),
                |(_, label)| label.to_uppercase(),
            ),
            face: picked.map(|(face, _)| face),
            outline,
        })
    }

    pub(super) fn update_face_hover(&mut self) {
        if self.more_open || self.solid_drag.is_some() {
            return;
        }
        self.hovered_face = self.face_hover(self.layout());
    }

    pub(super) fn paint_face_hover(
        &self,
        painter: &mut Painter,
        layout: Layout,
        projector: &SolidProjector,
    ) {
        let Some(hover) = self
            .face_hover(layout)
            .or_else(|| self.hovered_face.clone().filter(|_| self.more_open))
        else {
            return;
        };
        let color = if hover.face.is_some() {
            FACE_HOVER
        } else {
            MUTED
        };
        for [a, b] in &hover.outline {
            let (a, b) = (projector.screen(*a), projector.screen(*b));
            painter.line(a[0], a[1], b[0], b[1], 3.0, color);
        }
        painter.text(
            layout.viewport.x + 18.0,
            layout.viewport.y + 68.0,
            &if hover.face.is_some() {
                format!("{}  |  4 SKETCH ON FACE", hover.label)
            } else {
                hover.label
            },
            1.0,
            color,
        );
    }

    /// Creates a sketch on the hovered extrusion face and opens it (key 4).
    pub(super) fn sketch_on_face(&mut self) {
        self.more_open = false;
        if !self.solid_preview {
            self.status_message = "Sketch on face: hover a face in the 3D preview (Tab)".into();
            return;
        }
        let hover = self
            .face_hover(self.layout())
            .or_else(|| self.hovered_face.clone());
        let Some(face) = hover.as_ref().and_then(|hover| hover.face.clone()) else {
            self.status_message = "Sketch on face: hover a planar face of an extrusion".into();
            return;
        };
        self.document.finish_drag();
        let Some(project) = &mut self.project else {
            return;
        };
        match project.create_sketch_on_face(&self.document, &face) {
            Ok(document) => {
                self.document = document;
                self.solid_preview = false;
                self.selected_profile = 0;
                self.editing = None;
                self.hovered_face = None;
                self.face_context = true;
                self.status_message = format!(
                    "Created {} on {}  |  5 toggles body context  |  Ctrl+S to save",
                    project
                        .sketch_name(project.active_sketch())
                        .unwrap_or("Sketch"),
                    hover.map_or_else(String::new, |hover| hover.label.to_lowercase())
                );
            }
            Err(error) => self.status_message = format!("Sketch on face: {error}"),
        }
    }

    pub(super) fn cycle_solid(&mut self, direction: isize) {
        let Some(solids) = self
            .project
            .as_ref()
            .and_then(ProjectFile::extrusion_previews)
        else {
            return;
        };
        if solids.is_empty() {
            return;
        }
        self.selected_feature = None;
        self.selected_solid =
            (self.selected_solid as isize + direction).rem_euclid(solids.len() as isize) as usize;
        self.status_message = format!(
            "Selected {}  |  Click DEPTH to edit",
            solids[self.selected_solid].name
        );
    }

    pub(super) fn paint_profile_choice(&self, painter: &mut Painter, layout: Layout) {
        if self.read_only() {
            return;
        }
        let Some(profiles) = profiles_from_document(&self.document) else {
            return;
        };
        if profiles.len() < 2 {
            return;
        }
        let index = self.selected_profile.min(profiles.len() - 1);
        let profile = &profiles[index];
        let origin = self.viewport_origin();
        for loop_points in std::iter::once(&profile.outer).chain(profile.holes.iter()) {
            for edge in 0..loop_points.len() {
                let a = self.document.world_to_screen(loop_points[edge], origin);
                let b = self
                    .document
                    .world_to_screen(loop_points[(edge + 1) % loop_points.len()], origin);
                painter.line(a.x, a.y, b.x, b.y, 3.0, ACCENT);
            }
        }
        painter.text(
            layout.viewport.x + 18.0,
            layout.viewport.y + 18.0,
            &format!(
                "PROFILE {} / {}  [ ] SELECT  X EXTRUDE",
                index + 1,
                profiles.len()
            ),
            1.0,
            ACCENT,
        );
    }

    pub(super) fn cycle_profile(&mut self, direction: isize) {
        let Some(profiles) = profiles_from_document(&self.document) else {
            self.status_message = "No closed profile in this sketch".into();
            return;
        };
        if profiles.len() < 2 {
            self.status_message = format!("{} closed profile", profiles.len());
            return;
        }
        self.selected_profile = (self.selected_profile as isize + direction)
            .rem_euclid(profiles.len() as isize) as usize;
        self.status_message = format!(
            "Profile {} of {} selected  |  X to extrude",
            self.selected_profile + 1,
            profiles.len()
        );
    }

    pub(super) fn paint_solid_labels(&self, painter: &mut Painter, layout: Layout, count: usize) {
        let viewport = layout.viewport;
        painter.text(
            viewport.x + 18.0,
            viewport.y + 18.0,
            &format!("3D / {count} SOLID{}", if count == 1 { "" } else { "S" }),
            1.3,
            TEXT,
        );
        painter.text(
            viewport.x + 18.0,
            viewport.y + 43.0,
            "RIGHT DRAG ORBIT  /  MIDDLE PAN  /  WHEEL ZOOM  /  TAB SKETCH",
            1.0,
            MUTED,
        );
    }

    pub(super) fn paint_boolean_preview(
        &self,
        painter: &mut Painter,
        layout: Layout,
        scene: &SolidScene,
        projector: &SolidProjector,
        count: usize,
    ) {
        painter.rect(layout.viewport, [0.065, 0.085, 0.115, 1.0]);
        let mut faces = Vec::new();
        for &[a, b, c] in &scene.triangles {
            let Some(normal) = triangle_normal(a, b, c) else {
                continue;
            };
            let facing = projector.facing_at(normal, a);
            if facing <= 0.001 {
                continue;
            }
            let shade = (0.35 + facing * 0.16).clamp(0.22, 0.55);
            let color = [shade * 0.48, shade * 0.82, shade, 1.0];
            let mid = std::array::from_fn(|axis| (a[axis] + b[axis] + c[axis]) / 3.0);
            faces.push((
                projector.depth(mid),
                [a, b, c].map(|p| projector.screen(p)),
                color,
            ));
        }
        faces.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, points, color) in faces {
            painter.triangle(points[0], points[1], points[2], color);
        }
        self.paint_solid_labels(painter, layout, count);
    }

    pub(super) fn paint_solid_preview(
        &self,
        painter: &mut Painter,
        layout: Layout,
        solids: &[SolidPreview],
        projector: &SolidProjector,
    ) {
        painter.rect(layout.viewport, [0.065, 0.085, 0.115, 1.0]);
        let screen = |solid: &SolidPreview, point: Point, offset: f32| {
            projector.screen(solid.world_point(point, offset))
        };
        let mut faces: Vec<(f32, Vec<[f32; 2]>, Color, bool)> = Vec::new();
        for solid in solids {
            for (loop_index, loop_points) in solid.loops().enumerate() {
                for index in 0..loop_points.len() {
                    let a = loop_points[index];
                    let b = loop_points[(index + 1) % loop_points.len()];
                    let a0 = solid.world_point(a, 0.0);
                    let b0 = solid.world_point(b, 0.0);
                    let b1 = solid.world_point(b, solid.depth);
                    let Some(normal) = triangle_normal(a0, b0, b1) else {
                        continue;
                    };
                    let facing = projector.facing_at(normal, a0);
                    if facing <= 0.001 {
                        continue;
                    }
                    let shade = (0.35 + facing * 0.16).clamp(0.22, 0.55);
                    let color = [shade * 0.48, shade * 0.82, shade, 1.0];
                    let mid = std::array::from_fn(|axis| (a0[axis] + b1[axis]) * 0.5);
                    faces.push((
                        projector.depth(mid),
                        vec![
                            screen(solid, a, 0.0),
                            screen(solid, b, 0.0),
                            screen(solid, b, solid.depth),
                            screen(solid, a, solid.depth),
                        ],
                        color,
                        loop_index == 0,
                    ));
                }
            }
            let cap_offset = if projector.facing_at(
                solid.normal(),
                solid.world_point(Point { x: 0.0, y: 0.0 }, solid.depth),
            ) >= 0.0
            {
                solid.depth
            } else {
                0.0
            };
            if let Some((cap_points, cap_triangles)) = solid.cap_mesh() {
                for [i, j, k] in cap_triangles {
                    let vertices = [cap_points[i], cap_points[j], cap_points[k]];
                    let mut mid = [0.0; 3];
                    for point in vertices {
                        let world = solid.world_point(point, cap_offset);
                        for axis in 0..3 {
                            mid[axis] += world[axis] / 3.0;
                        }
                    }
                    faces.push((
                        projector.depth(mid),
                        vertices
                            .map(|point| screen(solid, point, cap_offset))
                            .to_vec(),
                        [0.28, 0.60, 0.72, 1.0],
                        false,
                    ));
                }
            }
        }
        faces.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, points, color, outline) in faces {
            for index in 1..points.len() - 1 {
                painter.triangle(points[0], points[index], points[index + 1], color);
            }
            if outline {
                for index in 0..points.len() {
                    let a = points[index];
                    let b = points[(index + 1) % points.len()];
                    painter.line(a[0], a[1], b[0], b[1], 1.2, ACCENT);
                }
            }
        }
        for solid in solids {
            let cap_offset = if projector.facing_at(
                solid.normal(),
                solid.world_point(Point { x: 0.0, y: 0.0 }, solid.depth),
            ) >= 0.0
            {
                solid.depth
            } else {
                0.0
            };
            for loop_points in solid.loops() {
                for index in 0..loop_points.len() {
                    let a = screen(solid, loop_points[index], cap_offset);
                    let b = screen(
                        solid,
                        loop_points[(index + 1) % loop_points.len()],
                        cap_offset,
                    );
                    painter.line(a[0], a[1], b[0], b[1], 1.2, ACCENT);
                }
            }
        }
        self.paint_solid_labels(painter, layout, solids.len());
    }
}
