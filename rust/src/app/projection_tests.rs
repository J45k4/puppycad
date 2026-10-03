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
