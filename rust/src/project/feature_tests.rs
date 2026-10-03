//! Revolve, loft and edge chamfer/fillet: volumes, re-evaluation,
//! serialization and read-only reasons.

use super::*;
use std::f64::consts::PI;

fn test_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "puppycad-feature-{name}-{}-{}.pcad",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn p(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn rectangle(document: &mut Document, a: Point, b: Point) {
    document.tool = crate::model::Tool::Rectangle;
    document.draft_start = Some(a);
    assert!(document.finish_draft(b));
}

fn circle(document: &mut Document, center: Point, radius: f32) {
    document.tool = crate::model::Tool::Circle;
    document.draft_start = Some(center);
    assert!(document.finish_draft(p(center.x + radius, center.y)));
}

fn volumes(project: &ProjectFile) -> Vec<f64> {
    project
        .evaluated_solids()
        .unwrap()
        .iter()
        .map(Manifold::volume)
        .collect()
}

fn close(actual: f64, expected: f64, fraction: f64) {
    assert!(
        (actual - expected).abs() <= expected.abs() * fraction,
        "{actual} is not within {}% of {expected}",
        fraction * 100.0
    );
}

const Y_AXIS: (Point, Point) = (Point { x: 0.0, y: 0.0 }, Point { x: 0.0, y: 1.0 });

#[test]
fn revolving_an_offset_rectangle_makes_a_ring() {
    let path = test_path("ring");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    rectangle(&mut document, p(10.0, 0.0), p(20.0, 10.0));
    project
        .create_revolve(&document, 0, Y_AXIS, 360.0, "newBody")
        .unwrap();
    let ring = PI * (20.0f64.powi(2) - 10.0f64.powi(2)) * 10.0;
    let solids = project.evaluated_solids().unwrap();
    assert_eq!(solids.len(), 1);
    assert_eq!(solids[0].genus(), 1, "a ring has one hole");
    close(volumes(&project)[0], ring, 0.02);
    // Top sketch: the Y axis is world Y, so the ring lies around world Y.
    let bounds = solids[0].bounding_box();
    assert!((bounds.max.y - 10.0).abs() < 1e-4 && bounds.min.y.abs() < 1e-4);
    assert!((bounds.max.z - 20.0).abs() < 0.1 && (bounds.min.x + 20.0).abs() < 0.1);
    assert_eq!(project.body_ids().len(), 1);

    project.save(&document).unwrap();
    let (reopened, _) = ProjectFile::open(&path).unwrap();
    assert_eq!(reopened.read_only_reason(), None);
    close(volumes(&reopened)[0], ring, 0.02);
    let features = reopened.extension_features();
    assert_eq!(features.len(), 1);
    assert_eq!(features[0].kind, "revolve");
    assert_eq!(features[0].value, 360.0);
    let _ = fs::remove_file(path);
}

#[test]
fn partial_revolve_is_a_quarter_ring_and_angle_is_editable() {
    let path = test_path("quarter");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    rectangle(&mut document, p(10.0, 0.0), p(20.0, 10.0));
    let id = project
        .create_revolve(&document, 0, Y_AXIS, 90.0, "newBody")
        .unwrap();
    let ring = PI * 300.0 * 10.0;
    close(volumes(&project)[0], ring / 4.0, 0.02);
    project.set_feature_value(&id, 180.0).unwrap();
    close(volumes(&project)[0], ring / 2.0, 0.02);
    assert!(project.set_feature_value(&id, 400.0).is_err());
    let before = project.solid_node(&id).unwrap();
    project.set_feature_value(&id, 45.0).unwrap();
    project.restore_solid_node(before).unwrap();
    close(volumes(&project)[0], ring / 2.0, 0.02);
    let _ = fs::remove_file(path);
}

#[test]
fn revolve_rejects_profiles_crossing_the_axis_and_can_cut() {
    let path = test_path("cross");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    rectangle(&mut document, p(-5.0, 0.0), p(5.0, 10.0));
    let error = project
        .create_revolve(&document, 0, Y_AXIS, 360.0, "newBody")
        .unwrap_err();
    assert!(error.contains("crosses"), "{error}");
    assert!(project.extension_features().is_empty());

    // A profile touching the axis revolves into a solid cylinder, then a
    // revolve cut from a second sketch takes a ring groove out of it.
    let mut document = Document::default();
    rectangle(&mut document, p(0.0, 0.0), p(20.0, 10.0));
    project
        .create_revolve(&document, 0, Y_AXIS, 360.0, "newBody")
        .unwrap();
    close(volumes(&project)[0], PI * 400.0 * 10.0, 0.02);
    let mut groove = project
        .create_sketch_on_plane(&document, SketchPlane::Top)
        .unwrap();
    rectangle(&mut groove, p(15.0, 2.0), p(25.0, 8.0));
    project
        .create_revolve(&groove, 0, Y_AXIS, 360.0, "cut")
        .unwrap();
    close(
        volumes(&project)[0],
        PI * 400.0 * 10.0 - PI * (400.0 - 225.0) * 6.0,
        0.02,
    );
    let _ = fs::remove_file(path);
}

#[test]
fn loft_from_rectangle_to_circle_is_manifold_with_bounded_volume() {
    let path = test_path("loft");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    rectangle(&mut document, p(0.0, 0.0), p(20.0, 20.0));
    project.create_extrusion(&document, 30.0).unwrap();
    let (top, _) = project
        .face_at([10.0, 10.0, 30.0], [0.0, 0.0, 1.0])
        .unwrap();
    let mut face = project.create_sketch_on_face(&document, &top).unwrap();
    circle(&mut face, p(10.0, 10.0), 10.0);
    let error = project
        .create_loft(&face, (1, 0), 0, "newBody")
        .unwrap_err();
    assert!(error.contains("different sketch"), "{error}");
    let id = project.create_loft(&face, (0, 0), 0, "newBody").unwrap();
    let volumes = volumes(&project);
    assert_eq!(volumes.len(), 2);
    let loft = volumes[1];
    let circle_area = PI * 100.0 * 0.99;
    assert!(loft > 30.0 * circle_area && loft < 30.0 * 400.0, "{loft}");
    let body = &project.evaluated_solids().unwrap()[1];
    assert_eq!(body.status(), ManifoldError::NoError);
    assert_eq!(body.genus(), 0);
    assert!(!body.has_self_intersections());
    assert!(project.create_loft(&face, (0, 0), 0, "cut").is_err());

    project.save(&face).unwrap();
    let (reopened, _) = ProjectFile::open(&path).unwrap();
    assert_eq!(reopened.read_only_reason(), None);
    assert!((reopened.evaluated_solids().unwrap()[1].volume() - loft).abs() < 1e-3);
    let node = reopened.solid_node(&id).unwrap();
    assert_eq!(node["profiles"].as_array().unwrap().len(), 2);
    let _ = fs::remove_file(path);
}

/// A 20 mm cube and the bottom front edge along X (y = 0, z = 0).
fn cube(name: &str) -> (ProjectFile, Document, PathBuf, EdgeRef) {
    let path = test_path(name);
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    rectangle(&mut document, p(0.0, 0.0), p(20.0, 20.0));
    project.create_extrusion(&document, 20.0).unwrap();
    let edges = project.solid_edges();
    assert_eq!(edges.len(), 12, "a cube has twelve sharp edges");
    let edge = edges
        .iter()
        .map(|(_, edge)| edge)
        .find(|e| {
            e.a[1].abs() < 1e-4 && e.b[1].abs() < 1e-4 && e.a[2].abs() < 1e-4 && e.b[2].abs() < 1e-4
        })
        .unwrap();
    assert!(edge.straight && edge.convex());
    assert!((edge.length() - 20.0).abs() < 1e-4);
    (project, document, path, edge.reference())
}

#[test]
fn chamfer_removes_a_triangular_prism() {
    let (mut project, document, path, edge) = cube("chamfer");
    let id = project
        .create_edge_finish(FinishKind::Chamfer, 2.0, std::slice::from_ref(&edge))
        .unwrap();
    close(volumes(&project)[0], 8000.0 - 2.0 * 2.0 / 2.0 * 20.0, 1e-4);
    project.set_feature_value(&id, 4.0).unwrap();
    close(volumes(&project)[0], 8000.0 - 4.0 * 4.0 / 2.0 * 20.0, 1e-4);

    // Upstream change: a deeper block keeps the bottom edge, so the chamfer
    // is re-identified and re-applied.
    project.set_extrusion_depth_at(0, 30.0).unwrap();
    close(volumes(&project)[0], 12000.0 - 8.0 * 20.0, 1e-4);

    project.save(&document).unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let node = saved["items"][0]["data"]["cad"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == id.as_str())
        .unwrap()
        .clone();
    assert_eq!(node["type"], "edgeFinish");
    assert_eq!(node["kind"], "chamfer");
    assert_eq!(node["distance"], 4.0);
    assert_eq!(EdgeRef::from_json(&node["edges"][0]), Some(edge));
    let (reopened, reopened_document) = ProjectFile::open(&path).unwrap();
    assert_eq!(reopened.read_only_reason(), None);
    close(volumes(&reopened)[0], 12000.0 - 8.0 * 20.0, 1e-4);
    assert_eq!(reopened.staged(&reopened_document).unwrap().0, saved);
    let _ = fs::remove_file(path);
}

#[test]
fn fillet_removes_the_corner_outside_the_arc() {
    let (mut project, _, path, edge) = cube("fillet");
    project
        .create_edge_finish(FinishKind::Fillet, 2.0, std::slice::from_ref(&edge))
        .unwrap();
    let removed = 8000.0 - volumes(&project)[0];
    close(removed, (1.0 - PI / 4.0) * 4.0 * 20.0, 0.02);
    let _ = fs::remove_file(path);
}

#[test]
fn chamfer_of_two_edges_sharing_a_corner_and_concave_fill() {
    let (mut project, _, path, first) = cube("two-edges");
    let second = project
        .solid_edges()
        .into_iter()
        .map(|(_, edge)| edge)
        .find(|e| {
            e.a[0].abs() < 1e-4 && e.b[0].abs() < 1e-4 && e.a[2].abs() < 1e-4 && e.b[2].abs() < 1e-4
        })
        .unwrap()
        .reference();
    project
        .create_edge_finish(FinishKind::Chamfer, 2.0, &[first, second])
        .unwrap();
    let removed = 8000.0 - volumes(&project)[0];
    // Two 40 mm³ prisms overlapping in a small corner pyramid.
    assert!(removed > 75.0 && removed < 80.0, "{removed}");
    let _ = fs::remove_file(path);
}

#[test]
fn concave_edge_fillet_adds_material() {
    let path = test_path("concave");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    rectangle(&mut document, p(0.0, 0.0), p(20.0, 20.0));
    project.create_extrusion(&document, 10.0).unwrap();
    let mut step = Document::default();
    rectangle(&mut step, p(0.0, 0.0), p(10.0, 20.0));
    let mut on_top = project
        .create_sketch_on_face(
            &document,
            &project
                .face_at([10.0, 10.0, 10.0], [0.0, 0.0, 1.0])
                .unwrap()
                .0,
        )
        .unwrap();
    on_top.shapes = step.shapes;
    project
        .create_extrusion_with_operation(&on_top, 10.0, 0, "join")
        .unwrap();
    let before = volumes(&project)[0];
    close(before, 4000.0 + 2000.0, 1e-4);
    let inner = project
        .solid_edges()
        .into_iter()
        .map(|(_, edge)| edge)
        .find(|e| !e.convex())
        .expect("the step has a concave edge");
    assert!((inner.a[0] - 10.0).abs() < 1e-4 && (inner.a[2] - 10.0).abs() < 1e-4);
    project
        .create_edge_finish(FinishKind::Fillet, 2.0, &[inner.reference()])
        .unwrap();
    let added = volumes(&project)[0] - before;
    close(added, (1.0 - PI / 4.0) * 4.0 * 20.0, 0.02);
    let _ = fs::remove_file(path);
}

#[test]
fn invalid_extension_nodes_open_read_only_with_a_reason() {
    let path = test_path("invalid");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    rectangle(&mut document, p(10.0, 0.0), p(20.0, 10.0));
    project
        .create_revolve(&document, 0, Y_AXIS, 360.0, "newBody")
        .unwrap();
    project.save(&document).unwrap();
    let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let with = |edit: &dyn Fn(&mut Value)| {
        let mut value = value.clone();
        for node in value["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap()
        {
            if node["type"] == "revolve" {
                edit(node);
            }
        }
        ProjectFile::from_value(path.clone(), value, None)
            .unwrap()
            .0
            .read_only_reason()
            .map(str::to_string)
    };
    assert_eq!(with(&|_| {}), None);
    let reason = with(&|node| node["angle"] = json!(400.0)).unwrap();
    assert!(reason.contains("angle"), "{reason}");
    let reason = with(&|node| {
        node["axis"] = json!({"a": {"x": 15.0, "y": 0.0}, "b": {"x": 15.0, "y": 1.0}})
    })
    .unwrap();
    assert!(reason.contains("crosses"), "{reason}");
    let reason = with(&|node| node["operation"] = json!("intersect")).unwrap();
    assert!(reason.contains("operation"), "{reason}");
    let reason = with(&|node| {
        *node = json!({"id": node["id"], "type": "edgeFinish", "name": "Fillet 1", "kind": "fillet", "radius": 1.0,
            "edges": [{"a": [100.0, 0.0, 0.0], "b": [100.0, 5.0, 0.0], "normals": [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0]]}]})
    })
    .unwrap();
    assert!(reason.contains("no longer exists"), "{reason}");
    // A web chamfer node references web topology edge ids.
    let reason = with(&|node| {
        *node = json!({"id": node["id"], "type": "chamfer", "edgeId": "edge-1", "d1": 1.0})
    })
    .unwrap();
    assert!(reason.contains("chamfer"), "{reason}");
    let _ = fs::remove_file(path);
}

#[test]
fn deleting_features_and_curved_edges() {
    let path = test_path("delete");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    circle(&mut document, p(0.0, 0.0), 10.0);
    project.create_extrusion(&document, 10.0).unwrap();
    let curved = project
        .solid_edges()
        .into_iter()
        .map(|(_, edge)| edge)
        .find(|edge| !edge.straight)
        .expect("a cylinder rim is curved");
    let error = project
        .create_edge_finish(FinishKind::Chamfer, 1.0, &[curved.reference()])
        .unwrap_err();
    assert!(error.contains("straight"), "{error}");
    let mut second = Document::default();
    rectangle(&mut second, p(20.0, 0.0), p(30.0, 10.0));
    let id = project
        .create_revolve(&second, 0, Y_AXIS, 90.0, "join")
        .unwrap();
    project.delete_feature(&id).unwrap();
    assert!(project.extension_features().is_empty());
    assert!(
        !project
            .graph_nodes()
            .unwrap()
            .iter()
            .any(|node| node["id"] == id.as_str())
    );
    let _ = fs::remove_file(path);
}
