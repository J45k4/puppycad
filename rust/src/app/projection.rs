//! Use edges / Project into sketch: body edges and face loops projected
//! into a face sketch as ordinary entities with a native `projected`
//! relation, re-projected whenever the body they come from changes.

use super::*;
use model::{EdgeKey, ProjectedCurve, ProjectionSource, classify_polyline};
use project::Frame;

/// Plane distance within which an edge lies on a face.
const ON_PLANE: f32 = 1e-3;
/// Screen distance within which a click hits a dashed reference edge.
const REFERENCE_PICK_PIXELS: f32 = 6.0;

/// 3D elements picked for projection before the sketch exists.
#[derive(Clone, Debug)]
pub(super) enum PendingProjection {
    Edge {
        key: EdgeKey,
        points: Vec<[f32; 3]>,
        closed: bool,
    },
    Face(String),
}

/// A source, its current curves (None: gone) and which are taken.
type SourceGroup = (
    ProjectionSource,
    Option<(Vec<ProjectedCurve>, ProjectionSource)>,
    Vec<bool>,
);

/// The bodies a face sketch sees, in its frame.
pub(super) struct ProjectionContext {
    pub(super) frame: Frame,
    pub(super) topology: Topology,
}

fn near(a: [f32; 3], b: [f32; 3], tolerance: f32) -> bool {
    norm3(sub3(a, b)) <= tolerance
}

fn edge_key(edge: &TopoEdge) -> EdgeKey {
    let a = edge.points[0];
    let b = *edge.points.last().unwrap();
    let via = if edge.points.len() == 2 {
        std::array::from_fn(|i| (a[i] + b[i]) * 0.5)
    } else {
        edge.points[edge.points.len() / 2]
    };
    EdgeKey {
        a,
        b,
        via,
        normals: edge.solid.as_ref().map_or([[0.0; 3]; 2], |s| s.normals),
    }
}

/// Edges of `topology` bounding the face in the plane through `origin`
/// with outward `normal`.
fn plane_edges(topology: &Topology, origin: [f32; 3], normal: [f32; 3]) -> Vec<usize> {
    let in_plane = |p: [f32; 3]| dot3(sub3(p, origin), normal).abs() < ON_PLANE;
    let faces: Vec<&TopoFace> = topology
        .faces
        .iter()
        .filter(|face| face.planar && dot3(face.normal, normal) > 0.9999)
        .filter(|face| face.outline.first().is_some_and(|s| in_plane(s[0])))
        .collect();
    let on_face_outline = |p: [f32; 3], q: [f32; 3]| {
        faces.iter().any(|face| {
            face.outline.iter().any(|s| {
                (near(s[0], p, 1e-4) && near(s[1], q, 1e-4))
                    || (near(s[0], q, 1e-4) && near(s[1], p, 1e-4))
            })
        })
    };
    (0..topology.edges.len())
        .filter(|&i| {
            let edge = &topology.edges[i];
            if !edge.points.iter().all(|p| in_plane(*p)) {
                return false;
            }
            match &edge.solid {
                Some(solid) => solid.normals.iter().any(|n| dot3(*n, normal) > 0.9999),
                None => on_face_outline(edge.points[0], edge.points[1]),
            }
        })
        .collect()
}

/// A 3D polyline projected along the sketch normal and recognized.
fn polyline_curves(points: &[[f32; 3]], closed: bool, frame: &Frame) -> Vec<ProjectedCurve> {
    let local: Vec<Point> = points.iter().map(|p| frame.local(*p).0).collect();
    classify_polyline(&local, closed)
}

/// The topology edge `key` names: the same ends and middle, else (after
/// the body changed) the nearest straight edge between the same two planes
/// or the nearest curved edge of similar size.
fn find_edge(topology: &Topology, key: &EdgeKey) -> Option<usize> {
    let exact = (0..topology.edges.len()).find(|&i| {
        let k = edge_key(&topology.edges[i]);
        near(k.via, key.via, 1e-3)
            && ((near(k.a, key.a, 1e-3) && near(k.b, key.b, 1e-3))
                || (near(k.a, key.b, 1e-3) && near(k.b, key.a, 1e-3)))
    });
    if exact.is_some() {
        return exact;
    }
    let straight = norm3(key.normals[0]) > 0.5;
    let same_normals = |normals: [[f32; 3]; 2]| {
        let same = |a: [f32; 3], b: [f32; 3]| dot3(a, b) > 0.9999;
        (same(normals[0], key.normals[0]) && same(normals[1], key.normals[1]))
            || (same(normals[0], key.normals[1]) && same(normals[1], key.normals[0]))
    };
    let span = norm3(sub3(key.a, key.b)).max(norm3(sub3(key.a, key.via)));
    (0..topology.edges.len())
        .filter(|&i| {
            let edge = &topology.edges[i];
            match (&edge.solid, straight) {
                (Some(solid), true) => same_normals(solid.normals),
                (None, false) => {
                    let k = edge_key(edge);
                    let other = norm3(sub3(k.a, k.b)).max(norm3(sub3(k.a, k.via)));
                    (other - span).abs() <= 0.5 * span.max(1e-3)
                }
                _ => false,
            }
        })
        .min_by(|&i, &j| {
            let d = |index: usize| norm3(sub3(edge_key(&topology.edges[index]).via, key.via));
            d(i).total_cmp(&d(j))
        })
}

impl ProjectionContext {
    /// Curves of `source` in the sketch, with the source as it now is
    /// (edge keys follow the body). None when the source is gone.
    fn curves(
        &self,
        project: &ProjectFile,
        source: &ProjectionSource,
    ) -> Option<(Vec<ProjectedCurve>, ProjectionSource)> {
        match source {
            ProjectionSource::Face(face_id) => {
                let plane = project.face_frame(face_id)?;
                if dot3(plane.normal, self.frame.normal).abs() < 1e-3 {
                    return None;
                }
                let edges = plane_edges(&self.topology, plane.origin, plane.normal);
                let curves: Vec<ProjectedCurve> = edges
                    .iter()
                    .flat_map(|&i| {
                        let edge = &self.topology.edges[i];
                        polyline_curves(&edge.points, edge.closed, &self.frame)
                    })
                    .collect();
                (!curves.is_empty()).then(|| (curves, source.clone()))
            }
            ProjectionSource::Edge(key) => {
                let index = find_edge(&self.topology, key)?;
                let edge = &self.topology.edges[index];
                let curves = polyline_curves(&edge.points, edge.closed, &self.frame);
                (!curves.is_empty()).then(|| (curves, ProjectionSource::Edge(edge_key(edge))))
            }
        }
    }

    /// Moves every projected shape of `document` onto its source. Returns
    /// how many lost their source (they stay as plain entities).
    pub(super) fn reproject(&self, project: &ProjectFile, document: &mut Document) -> usize {
        let mut ids: Vec<u32> = document.projections.keys().copied().collect();
        ids.sort_unstable();
        // Sources with their candidate curves and which are taken.
        let mut groups: Vec<SourceGroup> = Vec::new();
        let mut lost = 0;
        for id in ids {
            let source = document.projections[&id].clone();
            let group = match groups.iter().position(|(s, _, _)| *s == source) {
                Some(group) => group,
                None => {
                    let curves = self.curves(project, &source);
                    let taken = vec![false; curves.as_ref().map_or(0, |c| c.0.len())];
                    groups.push((source, curves, taken));
                    groups.len() - 1
                }
            };
            let (_, curves, taken) = &mut groups[group];
            let shape = document
                .shapes
                .iter()
                .find(|shape| shape.id() == id)
                .cloned();
            let best = curves.as_ref().zip(shape).and_then(|((curves, _), shape)| {
                curves
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !taken[*i])
                    .filter_map(|(i, curve)| curve.distance_to(&shape).map(|d| (i, d)))
                    .min_by(|a, b| a.1.total_cmp(&b.1))
            });
            match (best, curves) {
                (Some((i, _)), Some((curves, now))) => {
                    taken[i] = true;
                    document.reproject(id, &curves[i]);
                    document.projections.insert(id, now.clone());
                }
                _ => {
                    document.unlink_projection(id);
                    lost += 1;
                }
            }
        }
        lost
    }
}

impl App {
    /// The bodies face sketch `index` sees, or None for plane sketches.
    pub(super) fn projection_context(&self, index: usize) -> Option<ProjectionContext> {
        let project = self.project.as_ref()?;
        project.sketch_face(index)?;
        let frame = project.sketch_frame(&project.sketch_id(index)?)?;
        let bodies = project.context_bodies(index);
        let scene = SolidScene::from_manifolds(&bodies);
        Some(ProjectionContext {
            frame,
            topology: Topology::from_scene(&scene),
        })
    }

    /// Re-projects the projected entities of every face sketch (the open
    /// one included) after the bodies may have changed.
    pub(super) fn reproject_all(&mut self) {
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let active = project.active_sketch();
        if project.read_only_reason().is_none() {
            project.sync_active_document(&self.document);
        }
        let mut lost = 0;
        for index in 0..project.sketch_count() {
            let has = if index == active {
                !self.document.projections.is_empty()
            } else {
                self.project
                    .as_ref()
                    .and_then(|p| p.sketch_document(index))
                    .is_some_and(|d| !d.projections.is_empty())
            };
            if !has {
                continue;
            }
            let Some(context) = self.projection_context(index) else {
                continue;
            };
            let project = self.project.as_mut().unwrap();
            if index == active {
                lost += context.reproject(project, &mut self.document);
                project.sync_active_document(&self.document);
            } else {
                let mut document = project.sketch_document(index).unwrap().clone();
                lost += context.reproject(project, &mut document);
                project.set_sketch_document(index, document);
            }
        }
        if lost > 0 {
            self.status_message = format!(
                "{lost} projected entit{} lost {} source and became plain geometry",
                if lost == 1 { "y" } else { "ies" },
                if lost == 1 { "its" } else { "their" }
            );
        }
    }

    /// Topology selection as projection sources, leaving out `exclude`
    /// (the face a new sketch is placed on).
    pub(super) fn selection_projections(
        &self,
        exclude: Option<&project::FaceRef>,
    ) -> Vec<PendingProjection> {
        let Some((scene, topology, selection)) = self.current_topology_selection() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for pick in &selection {
            match pick.element {
                TopoRef::Edge(i) => {
                    let edge = &topology.edges[i];
                    out.push(PendingProjection::Edge {
                        key: edge_key(edge),
                        points: edge.points.clone(),
                        closed: edge.closed,
                    });
                }
                TopoRef::Face(i) => match self.pick_web_face(&scene, &topology, pick) {
                    Some((face, _)) if Some(&face) == exclude => {}
                    Some((face, _)) => out.push(PendingProjection::Face(face.face_id)),
                    None if topology.faces[i].planar => {
                        let face = &topology.faces[i];
                        let origin = face.outline[0][0];
                        for e in plane_edges(&topology, origin, face.normal) {
                            let edge = &topology.edges[e];
                            out.push(PendingProjection::Edge {
                                key: edge_key(edge),
                                points: edge.points.clone(),
                                closed: edge.closed,
                            });
                        }
                    }
                    None => {}
                },
                TopoRef::Vertex(_) => {}
            }
        }
        out
    }

    /// Use edges (O / MORE → USE EDGES) in a face sketch: projects the 3D
    /// selection taken into the sketch, else the sketch face's loops, and
    /// arms clicking dashed reference edges. With projected entities
    /// selected it toggles their construction state instead.
    pub(super) fn use_edges(&mut self) {
        self.more_open = false;
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let active = project.active_sketch();
        if project.sketch_face(active).is_none() {
            self.status_message = "Use edges: open a sketch on a face (4 on a body face)".into();
            return;
        }
        if self.read_only() {
            self.status_message = "READ ONLY: this sketch has unsupported native features".into();
            return;
        }
        if let Some(enabled) = self.document.toggle_projected_construction() {
            self.status_message = format!(
                "Projected geometry {}",
                if enabled {
                    "is construction"
                } else {
                    "is profile geometry"
                }
            );
            return;
        }
        let pending = std::mem::take(&mut self.pending_projection);
        self.project_pending(&pending, pending.is_empty());
    }

    /// Projects `pending` (and the sketch face's loops when `own_face`)
    /// into the open face sketch.
    fn project_pending(&mut self, pending: &[PendingProjection], own_face: bool) {
        let Some(project) = self.project.as_ref() else {
            return;
        };
        let active = project.active_sketch();
        let Some(face) = project.sketch_face(active).cloned() else {
            return;
        };
        let Some(context) = self.projection_context(active) else {
            self.status_message = "Use edges: the sketch face has no body".into();
            return;
        };
        let mut curves: Vec<(ProjectedCurve, ProjectionSource)> = Vec::new();
        let add_face = |face_id: &str, curves: &mut Vec<_>| {
            if let Some((found, source)) =
                context.curves(project, &ProjectionSource::Face(face_id.to_string()))
            {
                curves.extend(found.into_iter().map(|c| (c, source.clone())));
            }
        };
        if own_face {
            add_face(&face.face_id, &mut curves);
        }
        for item in pending {
            match item {
                PendingProjection::Face(face_id) => add_face(face_id, &mut curves),
                PendingProjection::Edge {
                    key,
                    points,
                    closed,
                } => curves.extend(
                    polyline_curves(points, *closed, &context.frame)
                        .into_iter()
                        .map(|c| (c, ProjectionSource::Edge(*key))),
                ),
            }
        }
        let ids = self.document.add_projected(&curves, "Use edges");
        self.use_mode = true;
        self.status_message = if ids.is_empty() {
            "Use edges: nothing new to project  |  click a dashed edge to project it".into()
        } else {
            format!(
                "Projected {} entit{}  |  click a dashed edge to add it  |  O again: construction",
                ids.len(),
                if ids.len() == 1 { "y" } else { "ies" }
            )
        };
    }

    /// Click in the sketch while Use edges is armed: projects the body edge
    /// under a dashed reference line. False when no dashed line is hit.
    pub(super) fn use_click(&mut self, point: Point) -> bool {
        let origin = self.viewport_origin();
        let world = self.document.screen_to_world(point, origin);
        let hit = self
            .document
            .reference_edges
            .iter()
            .chain(&self.document.reference_silhouette)
            .any(|&(a, b)| {
                let (a, b) = (
                    self.document.world_to_screen(a, origin),
                    self.document.world_to_screen(b, origin),
                );
                topology::segment_distance(point, [a.x, a.y], [b.x, b.y]).0 <= REFERENCE_PICK_PIXELS
            });
        if !hit {
            return false;
        }
        let Some(project) = self.project.as_ref() else {
            return false;
        };
        let Some(context) = self.projection_context(project.active_sketch()) else {
            return false;
        };
        let tolerance = REFERENCE_PICK_PIXELS / self.document.zoom.max(0.2);
        let best = context
            .topology
            .edges
            .iter()
            .map(|edge| {
                let local: Vec<Point> = edge
                    .points
                    .iter()
                    .map(|p| context.frame.local(*p).0)
                    .collect();
                let distance = local
                    .windows(2)
                    .map(|pair| {
                        topology::segment_distance(
                            world,
                            [pair[0].x, pair[0].y],
                            [pair[1].x, pair[1].y],
                        )
                        .0
                    })
                    .fold(f32::INFINITY, f32::min);
                // Prefer the edge nearest the sketch plane among overlaps.
                let depth = edge
                    .points
                    .iter()
                    .map(|p| context.frame.local(*p).1.abs())
                    .fold(f32::INFINITY, f32::min);
                (distance, depth, edge)
            })
            .filter(|(distance, _, _)| *distance <= tolerance)
            .min_by(|a, b| (a.0 + a.1 * 1e-3).total_cmp(&(b.0 + b.1 * 1e-3)));
        let Some((_, _, edge)) = best else {
            return false;
        };
        let pending = [PendingProjection::Edge {
            key: edge_key(edge),
            points: edge.points.clone(),
            closed: edge.closed,
        }];
        self.project_pending(&pending, false);
        true
    }

    /// Context menu → Project into sketch: the selected edges and faces go
    /// into the open face sketch, or into a new sketch on the clicked face.
    pub(super) fn project_into_sketch(&mut self) {
        let into_open = self
            .project
            .as_ref()
            .is_some_and(|project| project.sketch_face(project.active_sketch()).is_some())
            && !self.read_only();
        if into_open {
            let pending = self.selection_projections(None);
            if pending.is_empty() {
                self.status_message = "Project into sketch: select edges or faces".into();
                return;
            }
            self.sync_sketch_to_project();
            self.solid_preview = false;
            self.refresh_reference_edges();
            self.project_pending(&pending, false);
            return;
        }
        // A new sketch on the selected face, with its loops projected.
        self.sketch_on_face();
        if self.solid_preview {
            return;
        }
        let pending = std::mem::take(&mut self.pending_projection);
        self.project_pending(&pending, true);
    }

    /// Whether Project into sketch can run for the current selection.
    pub(super) fn project_into_sketch_available(&self) -> bool {
        let into_open = self
            .project
            .as_ref()
            .is_some_and(|project| project.sketch_face(project.active_sketch()).is_some())
            && !self.read_only();
        into_open
            || self
                .current_topology_selection()
                .is_some_and(|(scene, topology, selection)| {
                    selection
                        .iter()
                        .any(|pick| self.pick_web_face(&scene, &topology, pick).is_some())
                })
    }
}
