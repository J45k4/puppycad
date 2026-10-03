//! Face sketch body context and projected ("Use edges") geometry.

use super::*;

fn copy_fixture(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "puppycad-projection-{name}-{}-{}.pcad",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::copy(Path::new("fixtures").join(format!("{name}.pcad")), &path).unwrap();
    path
}

/// Topology of the part's evaluated bodies.
fn body_topology(app: &App) -> (SolidScene, Topology) {
    let bodies = app.project.as_ref().unwrap().evaluated_solids().unwrap();
    let scene = SolidScene::from_manifolds(&bodies);
    let topology = Topology::from_scene(&scene);
    (scene, topology)
}

/// The outermost planar face facing `normal` and a point inside it.
fn extreme_face(scene: &SolidScene, topology: &Topology, normal: [f32; 3]) -> ([f32; 3], usize) {
    let (index, face) = topology
        .faces
        .iter()
        .enumerate()
        .filter(|(_, face)| face.planar && dot3(face.normal, normal) > 0.9999)
        .max_by(|a, b| {
            let height = |face: &TopoFace| dot3(face.outline[0][0], normal);
            height(a.1).total_cmp(&height(b.1))
        })
        .unwrap();
    let [a, b, c] = scene.triangles[face.triangles[0]];
    (std::array::from_fn(|i| (a[i] + b[i] + c[i]) / 3.0), index)
}

/// Opens a face sketch on the face of `app`'s bodies facing `normal`.
fn sketch_on(app: &mut App, normal: [f32; 3]) -> usize {
    let (scene, topology) = body_topology(app);
    let (point, face) = extreme_face(&scene, &topology, normal);
    let project = app.project.as_mut().unwrap();
    let (face_ref, _) = project
        .face_at(point, normal)
        .expect("the outer face resolves to a web face");
    app.document = project
        .create_sketch_on_face(&app.document, &face_ref)
        .unwrap();
    app.face_context = true;
    app.solid_preview = false;
    face
}

/// Every boundary segment of the topology faces coplanar with `face` lies
/// on a reference edge, and the reference edges have the same total length.
fn assert_context_matches(app: &App, face: usize) {
    let (_, topology) = body_topology(app);
    let project = app.project.as_ref().unwrap();
    let frame = project
        .sketch_frame(&project.sketch_id(project.active_sketch()).unwrap())
        .unwrap();
    let normal = topology.faces[face].normal;
    let level = dot3(topology.faces[face].outline[0][0], normal);
    let outline: Vec<(Point, Point)> = topology
        .faces
        .iter()
        .filter(|f| {
            f.planar
                && dot3(f.normal, normal) > 0.9999
                && (dot3(f.outline[0][0], normal) - level).abs() < 1e-3
        })
        .flat_map(|f| f.outline.iter())
        .map(|[a, b]| (frame.local(*a).0, frame.local(*b).0))
        .collect();
    let edges = &app.document.reference_edges;
    let on_edge = |p: Point| {
        edges.iter().any(|&(a, b)| {
            let (d, _) = topology::segment_distance(p, [a.x, a.y], [b.x, b.y]);
            d < 1e-3
        })
    };
    for &(a, b) in &outline {
        assert!(
            on_edge(a) && on_edge(b),
            "face boundary {a:?}-{b:?} is not in the context {edges:?}"
        );
    }
    let length =
        |segments: &[(Point, Point)]| -> f32 { segments.iter().map(|(a, b)| a.distance(*b)).sum() };
    assert!(
        (length(&outline) - length(edges)).abs() < 1e-3,
        "{} != {}",
        length(&outline),
        length(edges)
    );
}

#[test]
fn face_context_matches_the_top_face_of_extrude_options() {
    let path = copy_fixture("extrude-options");
    let mut app = App::with_project(&path).unwrap();
    let face = sketch_on(&mut app, [0.0, 0.0, 1.0]);
    // The symmetric block's top is at z = 30 (not the web prism's z = 60)
    // and has the through hole.
    let project = app.project.as_ref().unwrap();
    let frame = project
        .sketch_frame(&project.sketch_id(project.active_sketch()).unwrap())
        .unwrap();
    assert!((frame.origin[2] - 30.0).abs() < 1e-4, "{frame:?}");
    assert_eq!(app.document.reference_edges.len(), 8);
    // The lower step shows in the dimmer silhouette.
    assert!(!app.document.reference_silhouette.is_empty());
    assert_context_matches(&app, face);
    let _ = std::fs::remove_file(path);
}

#[test]
fn face_context_matches_a_side_face_of_extrude_options() {
    let path = copy_fixture("extrude-options");
    let mut app = App::with_project(&path).unwrap();
    let face = sketch_on(&mut app, [0.0, -1.0, 0.0]);
    assert_context_matches(&app, face);
    let _ = std::fs::remove_file(path);
}

#[test]
fn face_context_matches_the_top_face_of_face_sketch() {
    let path = copy_fixture("face-sketch");
    let mut app = App::with_project(&path).unwrap();
    let face = sketch_on(&mut app, [0.0, 0.0, 1.0]);
    assert_context_matches(&app, face);
    // The pocket from the earlier face sketch is a hole loop here.
    assert!(app.document.reference_edges.len() > 4);
    let _ = std::fs::remove_file(path);
}

#[test]
fn face_context_matches_the_top_face_of_feature_tree() {
    let path = copy_fixture("feature-tree");
    let mut app = App::with_project(&path).unwrap();
    let face = sketch_on(&mut app, [0.0, 0.0, 1.0]);
    assert_context_matches(&app, face);
    let _ = std::fs::remove_file(path);
}

#[test]
fn context_of_a_sketch_ignores_its_own_features() {
    // face-sketch.pcad: the top face sketch carries the pocket; its own
    // context is the plain block top.
    let path = copy_fixture("face-sketch");
    let (project, _) = project::ProjectFile::open(&path).unwrap();
    let index = (0..project.sketch_count())
        .find(|&i| project.sketch_name(i) == Some("Top face sketch"))
        .unwrap();
    assert_eq!(project.sketch_context(index).edges.len(), 4);
    let _ = std::fs::remove_file(path);
}

fn lines(document: &Document) -> Vec<(Point, Point)> {
    document
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Line { a, b, .. } => Some((*a, *b)),
            _ => None,
        })
        .collect()
}

fn p(x: f32, y: f32) -> Point {
    Point { x, y }
}

#[test]
fn use_edges_projects_the_top_face_as_four_fixed_lines() {
    let path = copy_fixture("extruded-rectangle");
    let mut app = App::with_project(&path).unwrap();
    sketch_on(&mut app, [0.0, 0.0, 1.0]);
    let before = app.document.analyze_constraints().dof;
    app.use_edges();
    assert_eq!(app.document.shapes.len(), 4, "{}", app.status_message);
    assert_eq!(lines(&app.document).len(), 4);
    assert_eq!(app.document.projections.len(), 4);
    assert!(app.document.projections.values().all(|source| matches!(
        source,
        model::ProjectionSource::Face(id) if id == "extrude-1-solid-face-6"
    )));
    // Locked by the solver: no new degrees of freedom.
    assert_eq!(app.document.analyze_constraints().dof, before);
    // Corners at the block outline.
    let xs: Vec<f32> = lines(&app.document)
        .iter()
        .flat_map(|(a, b)| [a.x.abs(), b.x.abs()])
        .collect();
    assert!(xs.iter().all(|x| (x - 80.0).abs() < 1e-3), "{xs:?}");
    // Projected geometry cannot be dragged.
    let id = app.document.shapes[0].id();
    app.document.select_only(Some(id));
    assert!(!app.document.begin_drag(p(80.0, 0.0)));
    // O again toggles construction; undo removes the projection.
    app.use_edges();
    assert!(app.document.construction.contains(&id));
    app.use_edges();
    assert!(!app.document.construction.contains(&id));
    app.document.undo();
    app.document.undo();
    app.document.undo();
    assert!(app.document.shapes.is_empty());
    assert!(app.document.projections.is_empty());
    let _ = std::fs::remove_file(path);
}

#[test]
fn use_edges_projects_a_hole_loop_as_a_circle() {
    let path = copy_fixture("face-sketch");
    let mut app = App::with_project(&path).unwrap();
    sketch_on(&mut app, [0.0, 0.0, 1.0]);
    app.use_edges();
    let circles: Vec<f32> = app
        .document
        .shapes
        .iter()
        .filter_map(|shape| match shape {
            Shape::Circle { radius, .. } => Some(*radius),
            _ => None,
        })
        .collect();
    assert_eq!(circles.len(), 1, "{:?}", app.document.shapes);
    assert!((circles[0] - 10.0).abs() < 1e-3, "{circles:?}");
    assert_eq!(lines(&app.document).len(), 4);
    let _ = std::fs::remove_file(path);
}

#[test]
fn mixed_projected_and_drawn_profile_extrudes_a_cut() {
    let path = copy_fixture("extruded-rectangle");
    let mut app = App::with_project(&path).unwrap();
    sketch_on(&mut app, [0.0, 0.0, 1.0]);
    app.use_edges();
    // The split line ends exactly on the projected edges (snapped).
    let top = app.document.snap_point(p(0.2, 50.1), false, true).unwrap();
    let bottom = app
        .document
        .snap_point(p(-0.2, -50.1), false, true)
        .unwrap();
    assert_eq!((top.point, bottom.point), (p(0.0, 50.0), p(0.0, -50.0)));
    app.document.tool = Tool::Line;
    app.document.draft_start = Some(top.point);
    assert!(app.document.finish_draft(bottom.point));
    let profiles = project::profiles_from_document(&app.document).unwrap();
    assert_eq!(profiles.len(), 2, "two halves");
    let project = app.project.as_mut().unwrap();
    let full = 160.0 * 100.0 * 60.0;
    project
        .create_extrusion_with_operation(&app.document, 10.0, 0, "cut")
        .unwrap();
    let volume: f64 = project
        .evaluated_solids()
        .unwrap()
        .iter()
        .map(|body| body.volume())
        .sum();
    assert!(
        (volume - (full - 80.0 * 100.0 * 10.0)).abs() < 1e-1,
        "{volume}"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn projected_lines_follow_a_depth_change() {
    let path = copy_fixture("extruded-rectangle");
    let mut app = App::with_project(&path).unwrap();
    sketch_on(&mut app, [0.0, -1.0, 0.0]);
    app.use_edges();
    assert_eq!(lines(&app.document).len(), 4);
    let top = |app: &App| {
        lines(&app.document)
            .iter()
            .flat_map(|(a, b)| [a.y.abs(), b.y.abs()])
            .fold(0.0f32, f32::max)
    };
    assert!((top(&app) - 60.0).abs() < 1e-3);
    app.solid_preview = true;
    app.selected_solid = 0;
    app.set_solid_depth(80.0).unwrap();
    assert!(
        (top(&app) - 80.0).abs() < 1e-3,
        "{:?}",
        lines(&app.document)
    );
    assert_eq!(app.document.projections.len(), 4);
    let _ = std::fs::remove_file(path);
}

#[test]
fn projections_round_trip_and_relink_on_open() {
    let path = copy_fixture("extruded-rectangle");
    let mut app = App::with_project(&path).unwrap();
    sketch_on(&mut app, [0.0, 0.0, 1.0]);
    app.use_edges();
    let edge = app.document.shapes[0].id();
    app.document.select_only(Some(edge));
    app.use_edges(); // construction
    app.project.as_mut().unwrap().save(&app.document).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches("\"projected\"").count(), 4);
    assert!(text.contains("\"sourceFaceId\""));

    let reopened = App::with_project(&path).unwrap();
    let project = reopened.project.as_ref().unwrap();
    let index = project.sketch_count() - 1;
    let document = if project.active_sketch() == index {
        reopened.document.clone()
    } else {
        project.sketch_document(index).unwrap().clone()
    };
    assert_eq!(document.projections.len(), 4);
    assert_eq!(document.construction.len(), 1);
    assert_eq!(lines(&document).len(), 4);

    // A source that no longer exists leaves plain entities.
    let broken = text.replace(
        "\"sourceFaceId\": \"extrude-1-solid-face-6\"",
        "\"sourceFaceId\": \"extrude-1-solid-face-99\"",
    );
    std::fs::write(&path, broken).unwrap();
    let app = App::with_project(&path).unwrap();
    let project = app.project.as_ref().unwrap();
    let index = project.sketch_count() - 1;
    let document = if project.active_sketch() == index {
        app.document.clone()
    } else {
        project.sketch_document(index).unwrap().clone()
    };
    assert!(document.projections.is_empty());
    assert_eq!(lines(&document).len(), 4);
    let _ = std::fs::remove_file(path);
}

#[test]
fn clicking_a_dashed_edge_projects_only_that_edge() {
    let path = copy_fixture("extruded-rectangle");
    let mut app = App::with_project(&path).unwrap();
    sketch_on(&mut app, [0.0, 0.0, 1.0]);
    app.use_mode = true;
    let origin = app.viewport_origin();
    let on_edge = app.document.world_to_screen(p(0.0, 50.0), origin);
    assert!(app.use_click(on_edge));
    assert_eq!(app.document.shapes.len(), 1);
    assert!(matches!(
        app.document.projections.values().next(),
        Some(model::ProjectionSource::Edge(_))
    ));
    let (a, b) = lines(&app.document)[0];
    assert!((a.y - 50.0).abs() < 1e-3 && (b.y - 50.0).abs() < 1e-3);
    let empty = app.document.world_to_screen(p(10.0, 10.0), origin);
    assert!(!app.use_click(empty));
    let _ = std::fs::remove_file(path);
}

#[test]
fn dashed_reference_edges_snap_where_drawn_geometry_crosses_them() {
    let mut document = Document::default();
    document.reference_edges = vec![(p(-50.0, 0.0), p(50.0, 0.0))];
    document.tool = Tool::Line;
    document.draft_start = Some(p(10.0, -20.0));
    assert!(document.finish_draft(p(10.0, 20.0)));
    let snap = document.snap_point(p(10.3, 0.2), false, true).unwrap();
    assert_eq!(snap.kind, model::SnapKind::Intersection);
    assert!(snap.point.distance(p(10.0, 0.0)) < 1e-4);
}

#[test]
fn context_menu_offers_project_into_sketch_on_edges_and_faces() {
    let path = copy_fixture("extruded-rectangle");
    let mut app = App::with_project(&path).unwrap();
    sketch_on(&mut app, [0.0, 0.0, 1.0]);
    // Back in 3D with a topology edge selected: the open face sketch takes it.
    app.solid_preview = true;
    let layout = app.layout();
    let (scene, projector, hash, topology) = app.topology_context(layout).unwrap();
    let edge = topology
        .edges
        .iter()
        .position(|edge| edge.points.iter().all(|q| (q[2] - 60.0).abs() < 1e-3))
        .unwrap();
    let mid = topology.edges[edge].points[0];
    let screen = projector.screen(mid);
    let _ = scene;
    app.topo_key = hash;
    app.topo_selection = vec![TopoPick {
        element: TopoRef::Edge(edge),
        point: mid,
        triangle: None,
    }];
    assert!(app.project_into_sketch_available());
    let _ = screen;
    app.project_into_sketch();
    assert!(!app.solid_preview);
    assert_eq!(app.document.shapes.len(), 1, "{}", app.status_message);
    let _ = std::fs::remove_file(path);
}
