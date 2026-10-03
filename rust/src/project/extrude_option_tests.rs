//! Extrusion end conditions and flip: serialization, mesh bounds and
//! Boolean results.

use super::*;
use crate::model::Tool;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "puppycad-extrude-{name}-{}-{}.pcad",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn rectangle(document: &mut Document, a: Point, b: Point) {
    document.tool = Tool::Rectangle;
    document.draft_start = Some(a);
    assert!(document.finish_draft(b));
}

fn opts(end: EndCondition, flip: bool) -> ExtrudeOptions {
    ExtrudeOptions { end, flip }
}

/// 100 x 100 block, symmetric 50 mm (z -25..25), plus a 50 x 50 pocket sketch
/// on the top plane cut 10 mm blind.
fn block_with_pocket(name: &str) -> (PathBuf, ProjectFile, Document) {
    let path = test_path(name);
    let (mut project, mut base) = ProjectFile::open(&path).unwrap();
    rectangle(
        &mut base,
        Point { x: 0.0, y: 0.0 },
        Point { x: 100.0, y: 100.0 },
    );
    project
        .create_extrusion_with_options(
            &base,
            50.0,
            0,
            "newBody",
            &opts(EndCondition::Symmetric, false),
        )
        .unwrap();
    let mut pocket = project
        .create_sketch_on_plane(&base, SketchPlane::Top)
        .unwrap();
    rectangle(
        &mut pocket,
        Point { x: 25.0, y: 25.0 },
        Point { x: 75.0, y: 75.0 },
    );
    project
        .create_extrusion_with_operation(&pocket, 10.0, 0, "cut")
        .unwrap();
    (path, project, pocket)
}

fn z_bounds(project: &ProjectFile) -> (f64, f64) {
    let bounds = project.evaluated_solids().unwrap()[0].bounding_box();
    (bounds.min.z, bounds.max.z)
}

#[test]
fn every_end_condition_round_trips_through_the_web_graph() {
    let (path, mut project, pocket) = block_with_pocket("roundtrip");
    let cases = [
        (opts(EndCondition::Blind, false), None),
        (opts(EndCondition::Blind, true), None),
        (opts(EndCondition::Symmetric, false), Some("symmetric")),
        (opts(EndCondition::ThroughAll, false), Some("through-all")),
        (
            opts(EndCondition::UpToBody("native-extrude-1".into()), true),
            Some("up-to-body"),
        ),
    ];
    for (options, written) in cases {
        project.set_extrusion_options_at(1, &options).unwrap();
        project.save(&pocket).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        assert_eq!(reopened.read_only_reason(), None);
        assert_eq!(reopened.extrusion_options_at(1), Some(options.clone()));
        let node = reopened.extrude_node_at(1).unwrap();
        assert_eq!(node["endCondition"].as_str(), written);
        assert_eq!(node.get("flip").is_some(), options.flip);
        assert_eq!(
            node.get("targetId").is_some(),
            written == Some("up-to-body")
        );
        assert!(node["depth"].as_f64().unwrap() > 0.0);
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn symmetric_and_flipped_extrusions_have_expected_bounds() {
    let (path, mut project, _) = block_with_pocket("bounds");
    assert_eq!(z_bounds(&project), (-25.0, 25.0));
    project
        .set_extrusion_options_at(0, &opts(EndCondition::Blind, true))
        .unwrap();
    assert_eq!(z_bounds(&project), (-50.0, 0.0));
    project
        .set_extrusion_options_at(0, &opts(EndCondition::Blind, false))
        .unwrap();
    assert_eq!(z_bounds(&project), (0.0, 50.0));
    // Flip has no effect on symmetric.
    project
        .set_extrusion_options_at(0, &opts(EndCondition::Symmetric, true))
        .unwrap();
    assert_eq!(z_bounds(&project), (-25.0, 25.0));
    let preview = &project.extrusion_previews().unwrap()[0];
    assert_eq!((preview.start, preview.depth), (-25.0, 50.0));
    let _ = fs::remove_file(path);
}

#[test]
fn through_all_and_up_to_body_cuts_remove_known_volumes() {
    let (path, mut project, _) = block_with_pocket("cuts");
    let full = 100.0 * 100.0 * 50.0;
    let pocket = 50.0 * 50.0;
    let volume = |project: &ProjectFile| project.evaluated_solids().unwrap()[0].volume();
    assert!((volume(&project) - (full - pocket * 10.0)).abs() < 1.0);
    project
        .set_extrusion_options_at(1, &opts(EndCondition::ThroughAll, false))
        .unwrap();
    assert!((volume(&project) - (full - pocket * 50.0)).abs() < 1.0);
    let target = EndCondition::UpToBody("native-extrude-1".into());
    project
        .set_extrusion_options_at(1, &opts(target.clone(), false))
        .unwrap();
    assert!((volume(&project) - (full - pocket * 25.0)).abs() < 1.0);
    let preview = &project.extrusion_previews().unwrap()[1];
    assert_eq!((preview.start, preview.depth), (0.0, 25.0));
    project
        .set_extrusion_options_at(1, &opts(target, true))
        .unwrap();
    let preview = &project.extrusion_previews().unwrap()[1];
    assert_eq!((preview.start, preview.depth), (-25.0, 25.0));
    assert!((volume(&project) - (full - pocket * 25.0)).abs() < 1.0);
    // New bodies cannot be through-all; unknown targets are rejected and
    // leave the graph unchanged.
    assert!(
        project
            .set_extrusion_options_at(0, &opts(EndCondition::ThroughAll, false))
            .is_err()
    );
    assert!(
        project
            .set_extrusion_options_at(1, &opts(EndCondition::UpToBody("nope".into()), false))
            .is_err()
    );
    assert_eq!(
        project.extrusion_options_at(1).unwrap().end,
        EndCondition::UpToBody("native-extrude-1".into())
    );
    let _ = fs::remove_file(path);
}

#[test]
fn extrude_options_fixture_evaluates_natively() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/extrude-options.pcad");
    let (project, _) = ProjectFile::open(&fixture).unwrap();
    assert_eq!(project.read_only_reason(), None);
    let solids = project.evaluated_solids().unwrap();
    assert_eq!(solids.len(), 1);
    let expected = 160.0 * 100.0 * 60.0 - 40.0 * 40.0 * 60.0 - 20.0 * 80.0 * 30.0;
    assert!((solids[0].volume() - expected).abs() < 1.0);
    let bounds = solids[0].bounding_box();
    assert_eq!((bounds.min.z, bounds.max.z), (-30.0, 30.0));
}

#[test]
fn unsupported_end_conditions_open_read_only_with_a_reason() {
    let (path, mut project, pocket) = block_with_pocket("unsupported");
    project.save(&pocket).unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for (field, content, reason) in [
        ("endCondition", json!("up-to-next"), "up-to-next"),
        ("endCondition", json!("up-to-body"), "no target body"),
        ("flip", json!("yes"), "boolean"),
    ] {
        let mut broken = value.clone();
        broken["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .rfind(|node| node["type"] == "extrude")
            .unwrap()[field] = content;
        fs::write(&path, serde_json::to_vec(&broken).unwrap()).unwrap();
        let (reopened, _) = ProjectFile::open(&path).unwrap();
        let message = reopened.read_only_reason().unwrap();
        assert!(message.contains(reason), "{message}");
    }
    value["items"][0]["data"]["cad"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["type"] == "extrude")
        .unwrap()["endCondition"] = json!("through-all");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let (reopened, _) = ProjectFile::open(&path).unwrap();
    assert!(
        reopened
            .read_only_reason()
            .unwrap()
            .contains("needs a cut or join")
    );
    fs::remove_file(path).unwrap();
}
