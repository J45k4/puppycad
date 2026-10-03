//! Face sketches: web plane frames, face picking, cuts and round-trips.

use super::*;

fn test_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "puppycad-face-{name}-{}-{}.pcad",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn rectangle(document: &mut Document, a: Point, b: Point) {
    document.tool = crate::model::Tool::Rectangle;
    document.draft_start = Some(a);
    assert!(document.finish_draft(b));
}

fn p(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn assert_frame(actual: &Frame, origin: [f32; 3], x: [f32; 3], y: [f32; 3], normal: [f32; 3]) {
    for (got, want) in [
        (actual.origin, origin),
        (actual.x_axis, x),
        (actual.y_axis, y),
        (actual.normal, normal),
    ] {
        for axis in 0..3 {
            assert!(
                (got[axis] - want[axis]).abs() < 1e-5,
                "{actual:?} != {origin:?} {x:?} {y:?} {normal:?}"
            );
        }
    }
}

fn volume(project: &ProjectFile) -> f64 {
    project
        .evaluated_solids()
        .unwrap()
        .iter()
        .map(Manifold::volume)
        .sum()
}

/// A block extruded from `plane`: the project, the extrusion id, the extruded
/// sketch document and the project path.
fn block(
    name: &str,
    plane: SketchPlane,
    a: Point,
    b: Point,
    depth: f32,
) -> (ProjectFile, String, Document, PathBuf) {
    let path = test_path(name);
    let (mut project, document) = ProjectFile::open(&path).unwrap();
    let mut document = if plane == SketchPlane::Top {
        document
    } else {
        project.create_sketch_on_plane(&document, plane).unwrap()
    };
    rectangle(&mut document, a, b);
    project.create_extrusion(&document, depth).unwrap();
    let id = project
        .graph_nodes()
        .unwrap()
        .iter()
        .rev()
        .find(|node| node["type"] == "extrude")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    (project, id, document, path)
}

/// Expected frames written from `getExtrudedFaceDescriptors` in
/// `src/cad/extrude.ts`: side face i starts at profile point i with x along
/// the edge, y along the extrusion normal and normal = x × y; the bottom cap
/// keeps the sketch axes with a negated normal; the top cap moves by the depth.
/// Corner rectangles loop (min,min) → (max,min) → (max,max) → (min,max).
#[test]
fn face_frames_match_web_descriptors_for_each_reference_plane() {
    let (project, id, _, _) = block("top", SketchPlane::Top, p(0.0, 0.0), p(20.0, 10.0), 15.0);
    let faces = project.face_descriptors(&id).unwrap();
    let ids: Vec<_> = faces.iter().map(|face| face.face_id.clone()).collect();
    assert_eq!(
        ids,
        (1..=6)
            .map(|i| format!("{id}-solid-face-{i}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(faces[0].label, "Side Face 1");
    assert_eq!(faces[4].label, "Bottom Face");
    assert_eq!(faces[5].label, "Top Face");
    let (x, y, z) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let neg = |v: [f32; 3]| v.map(|c| -c);
    assert_frame(&faces[0].frame, [0.0; 3], x, z, neg(y));
    assert_frame(&faces[1].frame, [20.0, 0.0, 0.0], y, z, x);
    assert_frame(&faces[2].frame, [20.0, 10.0, 0.0], neg(x), z, y);
    assert_frame(&faces[3].frame, [0.0, 10.0, 0.0], neg(y), z, neg(x));
    assert_frame(&faces[4].frame, [0.0; 3], x, y, neg(z));
    assert_frame(&faces[5].frame, [0.0, 0.0, 15.0], x, y, z);

    // XZ: sketch (u, v) -> world (u, 0, -v), extruded along +Y.
    let (project, id, _, _) = block(
        "front",
        SketchPlane::Front,
        p(30.0, 0.0),
        p(50.0, 20.0),
        25.0,
    );
    let faces = project.face_descriptors(&id).unwrap();
    assert_frame(&faces[0].frame, [30.0, 0.0, 0.0], x, y, z);
    assert_frame(&faces[1].frame, [50.0, 0.0, 0.0], neg(z), y, x);
    assert_frame(&faces[4].frame, [0.0; 3], x, neg(z), neg(y));
    assert_frame(&faces[5].frame, [0.0, 25.0, 0.0], x, neg(z), y);

    // YZ: sketch (u, v) -> world (0, u, v), extruded along +X.
    let (project, id, _, _) = block("right", SketchPlane::Right, p(0.0, 0.0), p(10.0, 5.0), 7.0);
    let faces = project.face_descriptors(&id).unwrap();
    assert_frame(&faces[0].frame, [0.0; 3], y, x, neg(z));
    assert_frame(&faces[1].frame, [0.0, 10.0, 0.0], z, x, y);
    assert_frame(&faces[4].frame, [0.0; 3], y, z, neg(x));
    assert_frame(&faces[5].frame, [7.0, 0.0, 0.0], y, z, x);
}

#[test]
fn face_at_finds_caps_and_sides_by_point_and_normal() {
    let (project, id, _, _) = block("pick", SketchPlane::Top, p(0.0, 0.0), p(20.0, 10.0), 15.0);
    let (top, label) = project.face_at([10.0, 5.0, 15.0], [0.0, 0.0, 2.0]).unwrap();
    assert_eq!(top.face_id, format!("{id}-solid-face-6"));
    assert_eq!(label, "Top Face");
    let (side, _) = project.face_at([20.0, 5.0, 7.0], [1.0, 0.0, 0.0]).unwrap();
    assert_eq!(side.face_id, format!("{id}-solid-face-2"));
    assert!(
        project
            .face_at([10.0, 5.0, 15.0], [1.0, 0.0, 0.0])
            .is_none()
    );
    assert!(
        project
            .face_at([30.0, 5.0, 15.0], [0.0, 0.0, 1.0])
            .is_none()
    );
}

#[test]
fn face_sketch_cut_reduces_volume_follows_source_and_round_trips() {
    let (mut project, id, document, path) =
        block("cut", SketchPlane::Top, p(0.0, 0.0), p(20.0, 10.0), 15.0);
    assert!((volume(&project) - 3000.0).abs() < 1e-2);
    let (top, _) = project.face_at([10.0, 5.0, 15.0], [0.0, 0.0, 1.0]).unwrap();
    let mut sketch = project.create_sketch_on_face(&document, &top).unwrap();
    // The supporting block's outline, projected into the face plane.
    assert_eq!(
        sketch.reference_edges.len(),
        4,
        "{:?}",
        sketch.reference_edges
    );
    rectangle(&mut sketch, p(5.0, 2.0), p(15.0, 8.0));
    project
        .create_extrusion_with_operation(&sketch, 5.0, 0, "cut")
        .unwrap();
    let cut = volume(&project);
    assert!((cut - (3000.0 - 300.0)).abs() < 1e-2, "{cut}");
    let bounds = project.evaluated_solids().unwrap()[0].bounding_box();
    assert!((bounds.max.z - 15.0).abs() < 1e-4, "cut runs into the body");

    // Source-depth following: the face sketch moves with its source's top.
    project.set_extrusion_depth_at(0, 20.0).unwrap();
    let frame = project.sketch_frame(&project.sketches[1].id).unwrap();
    assert_eq!(frame.origin, [0.0, 0.0, 20.0]);
    assert!((volume(&project) - (4000.0 - 300.0)).abs() < 1e-2);

    project.save(&sketch).unwrap();
    let value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let nodes = value["items"][0]["data"]["cad"]["nodes"]
        .as_array()
        .unwrap();
    let face_id = format!("{id}-solid-face-6");
    let face_node_id = format!("face-{id}-{face_id}");
    assert!(nodes.iter().any(|node| node["id"] == face_node_id.as_str()
        && node["type"] == "face"
        && node["sourceId"] == id.as_str()
        && node["faceId"] == face_id.as_str()));
    assert!(
        nodes
            .iter()
            .any(|node| node["type"] == "sketch" && node["targetId"] == face_node_id.as_str())
    );

    let (reopened, _) = ProjectFile::open(&path).unwrap();
    assert_eq!(reopened.read_only_reason(), None);
    assert_eq!(reopened.sketch_face(1), Some(&top));
    assert!((volume(&reopened) - (4000.0 - 300.0)).abs() < 1e-2);
    let (copy, document) = ProjectFile::from_value(path.clone(), value.clone(), None).unwrap();
    assert_eq!(copy.staged(&document).unwrap().0, value);

    // A face that no longer exists keeps the project read-only.
    let mut missing = value;
    for node in missing["items"][0]["data"]["cad"]["nodes"]
        .as_array_mut()
        .unwrap()
    {
        if node["type"] == "face" {
            node["faceId"] = json!(format!("{id}-solid-face-99"));
        }
    }
    let (broken, _) = ProjectFile::from_value(path.clone(), missing, None).unwrap();
    assert!(broken.read_only_reason().is_some());
    let _ = fs::remove_file(path);
}

#[test]
fn bottom_face_sketch_joins_outward_with_valid_winding() {
    let (mut project, id, document, _) =
        block("bottom", SketchPlane::Top, p(0.0, 0.0), p(20.0, 10.0), 15.0);
    let bottom = FaceRef {
        extrude_id: id.clone(),
        face_id: format!("{id}-solid-face-5"),
    };
    let mut sketch = project.create_sketch_on_face(&document, &bottom).unwrap();
    rectangle(&mut sketch, p(0.0, 0.0), p(10.0, 10.0));
    project
        .create_extrusion_with_operation(&sketch, 4.0, 0, "join")
        .unwrap();
    let solids = project.evaluated_solids().unwrap();
    let total: f64 = solids.iter().map(Manifold::volume).sum();
    assert!((total - (3000.0 + 400.0)).abs() < 1e-2, "{total}");
    assert!((solids[0].bounding_box().min.z + 4.0).abs() < 1e-4);
}

/// Graph written by the web `createPartRuntimeState` for an XY block with a
/// rectangle sketched on its top face (`test/rust-roundtrip.test.ts`).
#[test]
fn web_saved_face_sketch_opens_editable() {
    let value = json!({"version":4,"revision":0,"items":[{"id":"web-part","type":"part","name":"Web part","data":{"features":[],"cad":{"nodes":[
        {"id":"plane-top","type":"referencePlane","name":"Top","plane":"XZ"},
        {"id":"plane-front","type":"referencePlane","name":"Front","plane":"XY"},
        {"id":"plane-right","type":"referencePlane","name":"Right","plane":"YZ"},
        {"id":"base","type":"sketch","name":"Base","targetId":"plane-front","relations":[],"dimensions":[]},
        {"id":"rect","type":"sketchPrimitive","sketchId":"base","primitive":{"id":"rect","type":"rectangle","center":{"x":10,"y":5},"width":20,"height":10,"rotation":0}},
        {"id":"ext","type":"extrude","sketchId":"base","profileId":"base-profile-1","operation":"newBody","depth":15},
        {"id":"face-ext-ext-solid-face-6","type":"face","sourceId":"ext","faceId":"ext-solid-face-6"},
        {"id":"on-top","type":"sketch","name":"Base","targetId":"face-ext-ext-solid-face-6","relations":[],"dimensions":[]},
        {"id":"pocket","type":"sketchPrimitive","sketchId":"on-top","primitive":{"id":"pocket","type":"rectangle","center":{"x":10,"y":5},"width":10,"height":6,"rotation":0}}
    ],"rootNodeIds":["plane-top","plane-front","plane-right","ext"]},"tree":{"orderedNodeIds":["base","ext","on-top"],"dirtySketchIds":[]}}}],"selectedPath":[0]});
    let path = test_path("web");
    let (mut project, base) = ProjectFile::from_value(path, value, None).unwrap();
    assert_eq!(project.read_only_reason(), None);
    assert_eq!(
        project.sketch_frame("on-top").unwrap().origin,
        [0.0, 0.0, 15.0]
    );
    let document = project.switch_sketch(1, &base).unwrap();
    project
        .create_extrusion_with_operation(&document, 4.0, 0, "cut")
        .unwrap();
    assert!((volume(&project) - (3000.0 - 240.0)).abs() < 1e-2);
}

#[test]
fn web_face_sketch_fixture_opens_editable() {
    let (project, _) = ProjectFile::open("fixtures/face-sketch.pcad").unwrap();
    assert_eq!(project.read_only_reason(), None);
    assert_eq!(project.sketch_count(), 3);
    assert_eq!(
        project.sketch_face(1).map(|face| face.face_id.as_str()),
        Some("extrude-1-solid-face-6")
    );
    let side = project.sketch_frame("native-sketch-3").unwrap();
    assert_frame(
        &side,
        [40.0, -30.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 0.0],
    );
    // 80 × 60 × 20 block, a 32-gon pocket (r 10, 8 deep), a 20 × 10 × 6 boss.
    let pocket = 16.0 * 100.0 * (std::f64::consts::PI / 16.0).sin() * 8.0;
    let expected = 96000.0 - pocket + 1200.0;
    assert!(
        (volume(&project) - expected).abs() < 0.1,
        "{}",
        volume(&project)
    );
    assert_eq!(project.sketch_support_edges(2).len(), 4);
}

#[test]
fn face_sketch_extrusions_honor_end_conditions_and_flip() {
    use crate::project::{EndCondition, ExtrudeOptions};
    let (mut project, _, document, path) = block(
        "face-options",
        SketchPlane::Top,
        p(0.0, 0.0),
        p(20.0, 10.0),
        15.0,
    );
    let (top, _) = project.face_at([10.0, 5.0, 15.0], [0.0, 0.0, 1.0]).unwrap();
    let mut sketch = project.create_sketch_on_face(&document, &top).unwrap();
    rectangle(&mut sketch, p(5.0, 2.0), p(15.0, 8.0));
    project
        .create_extrusion_with_operation(&sketch, 5.0, 0, "cut")
        .unwrap();
    let set = |project: &mut ProjectFile, end, flip| {
        project
            .set_extrusion_options_at(1, &ExtrudeOptions { end, flip })
            .unwrap();
        let tool = project.extrusion_previews().unwrap().swap_remove(1);
        (tool.start, tool.depth, volume(project))
    };
    // Flipped cut grows outward from the face: nothing is removed.
    let (start, depth, flipped) = set(&mut project, EndCondition::Blind, true);
    assert_eq!((start, depth), (0.0, 5.0));
    assert!((flipped - 3000.0).abs() < 1e-2, "{flipped}");
    // Symmetric straddles the face: half the depth is cut.
    let (start, _, symmetric) = set(&mut project, EndCondition::Symmetric, false);
    assert_eq!(start, -2.5);
    assert!((symmetric - (3000.0 - 150.0)).abs() < 1e-2, "{symmetric}");
    // Through all spans the whole block measured from the face origin.
    let (start, depth, through) = set(&mut project, EndCondition::ThroughAll, false);
    assert!(start < -15.0 && start + depth > 0.0, "{start} {depth}");
    assert!((through - (3000.0 - 900.0)).abs() < 1e-2, "{through}");
    let _ = fs::remove_file(path);
}
