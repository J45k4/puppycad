//! Project round-trips of dimension objects (label positions, point
//! dimensions, reference dimensions, expressions and sketch variables).

use super::*;
use crate::model::Tool;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "puppycad-dims-{name}-{}-{}.pcad",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn draw_line(document: &mut Document, a: Point, b: Point) -> u32 {
    document.tool = Tool::Line;
    document.draft_start = Some(a);
    assert!(document.finish_draft(b));
    document.shapes.last().unwrap().id()
}

fn sketch_relations(project: &ProjectFile) -> Vec<Value> {
    project.value["items"][0]["data"]["cad"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["type"] == "sketch")
        .unwrap()["relations"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn dimension_label_position_round_trips_as_web_label_position() {
    let path = test_path("label");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    let line = draw_line(
        &mut document,
        Point { x: 0.0, y: 0.0 },
        Point { x: 40.0, y: 0.0 },
    );
    document.selected = Some(line);
    document
        .set_dimension(DimensionField::LineLength, 50.0)
        .unwrap();
    let key = DimensionKey::Field(line, DimensionField::LineLength);
    document
        .set_dimension_label_position(key, Some(Point { x: 25.0, y: 12.0 }))
        .unwrap();
    project.save(&document).unwrap();
    let (reopened, loaded) = ProjectFile::open(&path).unwrap();
    assert_eq!(reopened.read_only_reason(), None);
    let relation = sketch_relations(&reopened)
        .into_iter()
        .find(|relation| relation["type"] == "length")
        .unwrap();
    assert_eq!(relation["labelPosition"], json!({"x": 25.0, "y": 12.0}));
    assert_eq!(
        loaded.dims.label_positions.get(&key),
        Some(&Point { x: 25.0, y: 12.0 })
    );
    fs::remove_file(path).unwrap();
}
