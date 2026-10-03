//! Feature tree: order, suppression, rollback, error states and incremental
//! re-evaluation.

use super::*;
use crate::model::Tool;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "puppycad-tree-{name}-{}-{}.pcad",
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

fn p(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn symmetric() -> ExtrudeOptions {
    ExtrudeOptions {
        end: EndCondition::Symmetric,
        flip: false,
    }
}

/// Block 100 x 100 x 50 (z -25..25), a 50 x 50 join boss z 0..40 and a
/// 20 x 20 symmetric 200 mm cut through the middle, in that order.
fn block_join_cut(path: &Path) -> (ProjectFile, Document) {
    let (mut project, mut base) = ProjectFile::open(path).unwrap();
    rectangle(&mut base, p(0.0, 0.0), p(100.0, 100.0));
    project
        .create_extrusion_with_options(&base, 50.0, 0, "newBody", &symmetric())
        .unwrap();
    let mut boss = project
        .create_sketch_on_plane(&base, SketchPlane::Top)
        .unwrap();
    rectangle(&mut boss, p(25.0, 25.0), p(75.0, 75.0));
    project
        .create_extrusion_with_operation(&boss, 40.0, 0, "join")
        .unwrap();
    let mut hole = project
        .create_sketch_on_plane(&boss, SketchPlane::Top)
        .unwrap();
    rectangle(&mut hole, p(40.0, 40.0), p(60.0, 60.0));
    project
        .create_extrusion_with_options(&hole, 200.0, 0, "cut", &symmetric())
        .unwrap();
    (project, hole)
}

fn volume(project: &ProjectFile) -> f64 {
    project
        .evaluated_solids()
        .unwrap()
        .iter()
        .map(Manifold::volume)
        .sum()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0
}

fn feature_ids(project: &ProjectFile) -> Vec<String> {
    project
        .feature_nodes()
        .iter()
        .map(|node| node["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn reorder_changes_the_boolean_result() {
    let path = test_path("reorder");
    let (mut project, _) = block_join_cut(&path);
    // Cut after the join removes the hole through the boss too.
    assert!(close(volume(&project), 511_500.0), "{}", volume(&project));
    assert_eq!(project.move_feature(2, -1), Ok(1));
    assert_eq!(
        feature_ids(&project),
        ["native-extrude-1", "native-extrude-3", "native-extrude-2"]
    );
    // The join now refills the hole between z 0 and 25.
    assert!(close(volume(&project), 527_500.0), "{}", volume(&project));
    let tree = &project.value["items"][0]["data"]["tree"]["orderedNodeIds"];
    let order: Vec<&str> = tree
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| id.starts_with("native-extrude"))
        .collect();
    assert_eq!(
        order,
        ["native-extrude-1", "native-extrude-3", "native-extrude-2"]
    );
    assert_eq!(project.move_feature(1, 1), Ok(2));
    assert!(close(volume(&project), 511_500.0));
}

#[test]
fn reorder_across_a_dependency_is_refused() {
    let path = test_path("refused");
    let (mut project, _) = block_join_cut(&path);
    let before = feature_ids(&project);
    let error = project.move_feature(1, -1).unwrap_err();
    assert!(error.contains("body"), "{error}");
    assert!(project.move_feature(0, 1).is_err());
    assert!(project.move_feature(2, 1).is_err());
    assert_eq!(feature_ids(&project), before);
}

#[test]
fn suppress_removes_a_cut_and_unsuppress_restores_it() {
    let path = test_path("suppress");
    let (mut project, _) = block_join_cut(&path);
    project.set_feature_suppressed(2, true).unwrap();
    assert!(project.feature_suppressed(2));
    assert_eq!(project.feature_statuses()[2], FeatureStatus::Suppressed);
    assert!(close(volume(&project), 537_500.0));
    project.set_feature_suppressed(2, false).unwrap();
    assert!(close(volume(&project), 511_500.0));
    // Suppressing the body leaves its cut and join in error, not fatal.
    project.set_feature_suppressed(0, true).unwrap();
    let statuses = project.feature_statuses();
    assert!(matches!(statuses[1], FeatureStatus::Error(_)));
    assert!(matches!(statuses[2], FeatureStatus::Error(_)));
    assert!(project.evaluated_solids().unwrap().is_empty());
}

#[test]
fn rollback_hides_later_features_and_export_respects_it() {
    let path = test_path("rollback");
    let (mut project, _) = block_join_cut(&path);
    project.set_rollback_index(Some(2)).unwrap();
    assert_eq!(project.rollback_index(), Some(2));
    assert_eq!(project.feature_statuses()[2], FeatureStatus::RolledBack);
    assert!(close(volume(&project), 537_500.0));
    let bodies = crate::stl::project_bodies(&project).unwrap();
    let exported: f64 = bodies
        .iter()
        .flat_map(|body| body.triangles.iter())
        .map(|[a, b, c]| {
            let cross = [
                b[1] * c[2] - b[2] * c[1],
                b[2] * c[0] - b[0] * c[2],
                b[0] * c[1] - b[1] * c[0],
            ];
            f64::from(a[0] * cross[0] + a[1] * cross[1] + a[2] * cross[2]) / 6.0
        })
        .sum();
    assert!(close(exported, 537_500.0), "{exported}");
    project.set_rollback_index(Some(1)).unwrap();
    assert!(close(volume(&project), 500_000.0));
    let bodies = crate::stl::project_bodies(&project).unwrap();
    assert_eq!(bodies.len(), 1);
    project.set_rollback_index(Some(3)).unwrap();
    assert_eq!(project.rollback_index(), None);
    assert!(close(volume(&project), 511_500.0));
}

#[test]
fn deleted_sketch_marks_its_feature_as_error() {
    let path = test_path("error");
    let (mut project, _) = block_join_cut(&path);
    let boss = project.feature_nodes()[1]["sketchId"]
        .as_str()
        .unwrap()
        .to_string();
    project.sketches.retain(|sketch| sketch.id != boss);
    let statuses = project.feature_statuses();
    assert!(
        matches!(&statuses[1], FeatureStatus::Error(reason) if reason.contains("sketch")),
        "{statuses:?}"
    );
    assert_eq!(statuses[2], FeatureStatus::Active);
    assert!(close(volume(&project), 480_000.0));
    assert_eq!(project.extrusion_previews().unwrap().len(), 3);
}

#[test]
fn order_suppression_and_rollback_round_trip() {
    let path = test_path("serialize");
    let (mut project, hole) = block_join_cut(&path);
    project.move_feature(2, -1).unwrap();
    project.set_feature_suppressed(1, true).unwrap();
    project.set_rollback_index(Some(2)).unwrap();
    project.save(&hole).unwrap();
    let (reopened, _) = ProjectFile::open(&path).unwrap();
    assert!(reopened.read_only_reason().is_none());
    assert_eq!(
        feature_ids(&reopened),
        ["native-extrude-1", "native-extrude-3", "native-extrude-2"]
    );
    assert!(reopened.feature_suppressed(1));
    assert!(!reopened.feature_suppressed(2));
    assert_eq!(reopened.rollback_index(), Some(2));
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["items"][0]["data"]["tree"]["rollbackIndex"], 2);
    assert_eq!(
        saved["items"][0]["data"]["cad"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|node| node["suppressed"] == true)
            .count(),
        1
    );
    fs::remove_file(path).unwrap();
}

#[test]
fn feature_tree_fixture_evaluates_natively() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/feature-tree.pcad");
    let (project, _) = ProjectFile::open(&fixture).unwrap();
    assert!(project.read_only_reason().is_none());
    assert_eq!(
        project.feature_statuses(),
        [
            FeatureStatus::Active,
            FeatureStatus::Suppressed,
            FeatureStatus::Active,
        ]
    );
    assert!(close(volume(&project), 537_500.0));
}

#[test]
fn upstream_sketch_edit_rebuilds_only_from_the_changed_feature() {
    let path = test_path("incremental");
    let (mut project, hole) = block_join_cut(&path);
    volume(&project);
    let start = project.feature_evaluations();
    volume(&project);
    assert_eq!(
        project.feature_evaluations(),
        start,
        "unchanged tree is cached"
    );
    // Edit the cut's depth: only the cut is rebuilt.
    project.set_extrusion_depth_at(2, 150.0).unwrap();
    volume(&project);
    let after_cut = project.feature_evaluations();
    assert_eq!(after_cut - start, 1);
    // Edit the boss sketch: the join and the cut after it are rebuilt.
    project.switch_sketch(1, &hole).unwrap();
    let mut boss = Document::default();
    rectangle(&mut boss, p(20.0, 20.0), p(80.0, 80.0));
    project.sync_active_document(&boss);
    let edited = volume(&project);
    assert_eq!(project.feature_evaluations() - after_cut, 2);
    // 60 x 60 boss, 15 mm above the block, minus the 20 x 20 hole.
    assert!(
        close(edited, 500_000.0 + 3600.0 * 15.0 - 400.0 * 65.0),
        "{edited}"
    );
}

#[test]
fn revolve_loft_and_edge_finish_are_tree_features() {
    let (mut project, _) =
        ProjectFile::open(Path::new("fixtures/revolve-loft-finish.pcad")).unwrap();
    assert_eq!(project.feature_count(), 4);
    assert_eq!(
        project
            .feature_nodes()
            .iter()
            .filter_map(|node| FeatureKind::from_node(node))
            .collect::<Vec<_>>(),
        vec![
            FeatureKind::Extrude,
            FeatureKind::EdgeFinish,
            FeatureKind::Revolve,
            FeatureKind::Loft
        ]
    );
    assert!(
        project
            .feature_statuses()
            .iter()
            .all(FeatureStatus::is_active)
    );
    let volume = |project: &ProjectFile| -> f64 {
        project
            .evaluated_solids()
            .unwrap()
            .iter()
            .map(Manifold::volume)
            .sum()
    };
    let full = volume(&project);
    assert_eq!(project.evaluated_solids().unwrap().len(), 3);
    // Suppressing the chamfer adds material back; the rest stays cached.
    let evaluations = project.feature_evaluations();
    project.set_feature_suppressed(1, true).unwrap();
    assert!(volume(&project) > full);
    assert_eq!(project.feature_statuses()[1], FeatureStatus::Suppressed);
    assert_eq!(project.feature_evaluations() - evaluations, 3);
    project.set_feature_suppressed(1, false).unwrap();
    // Rolling back above the revolve drops the revolve and loft bodies.
    project.set_rollback_index(Some(2)).unwrap();
    assert_eq!(project.evaluated_solids().unwrap().len(), 1);
    assert_eq!(project.feature_statuses()[3], FeatureStatus::RolledBack);
    project.set_rollback_index(None).unwrap();
    assert!((volume(&project) - full).abs() < 1e-3);
    // A revolve whose profile is gone is an error; the part still evaluates.
    let revolve = project.feature_index("native-revolve-1").unwrap();
    let mut node = project.feature_node(revolve).unwrap();
    node["profileId"] = json!("native-sketch-2-profile-9");
    project
        .restore_feature_tree({
            let mut snapshot = project.feature_tree_snapshot();
            for slot in snapshot["nodes"].as_array_mut().unwrap() {
                if slot["id"] == "native-revolve-1" {
                    *slot = node.clone();
                }
            }
            snapshot
        })
        .unwrap();
    assert!(matches!(
        project.feature_statuses()[revolve],
        FeatureStatus::Error(_)
    ));
    assert_eq!(project.evaluated_solids().unwrap().len(), 2);
}
