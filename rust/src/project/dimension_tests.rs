//! Project round-trips of dimension objects (label positions, point
//! dimensions, reference dimensions, expressions and sketch variables).

use super::*;
use crate::model::{PointAxis, Tool};
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

#[test]
fn point_dimensions_round_trip_as_web_distance_relations() {
    let path = test_path("distance");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    document.add_point(Point { x: 10.0, y: 5.0 });
    let point = document.shapes.last().unwrap().id();
    let line = draw_line(
        &mut document,
        Point { x: 40.0, y: 45.0 },
        Point { x: 90.0, y: 45.0 },
    );
    let distance = document
        .add_point_dimension((point, 0), Some((line, 0)), None)
        .unwrap();
    document.set_dimension_value(distance, 60.0).unwrap();
    let dx = document
        .add_point_dimension((point, 0), Some((line, 1)), Some(PointAxis::X))
        .unwrap();
    let absolute_y = document
        .add_point_dimension((point, 0), None, Some(PointAxis::Y))
        .unwrap();
    document
        .set_dimension_label_position(dx, Some(Point { x: 1.0, y: 2.0 }))
        .unwrap();
    project.save(&document).unwrap();
    let relations = sketch_relations(&project);
    let distances: Vec<&Value> = relations
        .iter()
        .filter(|relation| relation["type"] == "distance")
        .collect();
    assert_eq!(distances.len(), 3);
    assert_eq!(distances[0]["a"]["point"], "center");
    assert_eq!(distances[0]["b"]["point"], "p0");
    assert!((distances[0]["value"].as_f64().unwrap() - 60.0).abs() < 1e-4);
    assert!(distances[0].get("axis").is_none());
    assert_eq!(distances[1]["axis"], "x");
    assert_eq!(distances[1]["b"]["point"], "p1");
    assert_eq!(distances[1]["labelPosition"], json!({"x": 1.0, "y": 2.0}));
    assert_eq!(distances[2]["axis"], "y");
    assert_eq!(distances[2]["value"], 5.0);
    // The origin anchor is a fixed construction point.
    assert!(relations.iter().any(|relation| relation["type"] == "fixed"
        && relation["anchor"]["entityId"] == distances[2]["a"]["entityId"]));

    let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
    assert_eq!(reopened.read_only_reason(), None);
    assert_eq!(loaded.dims.point_dimensions.len(), 3);
    assert_eq!(loaded.dimension_keys().len(), 3);
    assert!(loaded.dims.label_positions.contains_key(&dx));
    let y = loaded.dimension_keys()[2];
    assert_eq!(loaded.dimension_value(y), Some(5.0));
    loaded.set_dimension_value(y, 8.0).unwrap();
    reopened.save(&loaded).unwrap();
    let relations = sketch_relations(&reopened);
    let ids: Vec<&Value> = relations
        .iter()
        .filter(|relation| relation["type"] == "distance")
        .map(|relation| &relation["id"])
        .collect();
    let original: Vec<&Value> = distances.iter().map(|relation| &relation["id"]).collect();
    assert_eq!(ids, original);
    let _ = absolute_y;
    fs::remove_file(path).unwrap();
}

#[test]
fn unsupported_distance_relation_keeps_sketch_read_only() {
    let path = test_path("distance-direction");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    document.add_point(Point { x: 0.0, y: 0.0 });
    document.add_point(Point { x: 3.0, y: 4.0 });
    document
        .add_point_dimension((1, 0), Some((2, 0)), None)
        .unwrap();
    project.save(&document).unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let sketch = value["items"][0]["data"]["cad"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["type"] == "sketch")
        .unwrap();
    sketch["relations"][0]["direction"] = json!({"x": 1.0, "y": 0.0});
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let (reopened, _) = ProjectFile::open(&path).unwrap();
    assert!(reopened.read_only_reason().is_some());
    fs::remove_file(path).unwrap();
}

#[test]
fn reference_dimensions_round_trip_with_web_reference_flag() {
    let path = test_path("reference");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    let line = draw_line(
        &mut document,
        Point { x: 0.0, y: 0.0 },
        Point { x: 40.0, y: 0.0 },
    );
    let length = document.add_entity_dimension(line).unwrap();
    document.toggle_reference(length).unwrap();
    document.add_point(Point { x: 0.0, y: 30.0 });
    let point = document.shapes.last().unwrap().id();
    let distance = document
        .add_point_dimension((line, 1), Some((point, 0)), None)
        .unwrap();
    document.toggle_reference(distance).unwrap();
    project.save(&document).unwrap();
    let relations = sketch_relations(&project);
    let saved_length = relations
        .iter()
        .find(|relation| relation["type"] == "length")
        .unwrap();
    assert_eq!(saved_length["reference"], true);
    assert_eq!(saved_length["value"], 40.0);
    let saved_distance = relations
        .iter()
        .find(|relation| relation["type"] == "distance")
        .unwrap();
    assert_eq!(saved_distance["reference"], true);
    assert_eq!(saved_distance["value"], 50.0);

    let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
    assert_eq!(reopened.read_only_reason(), None);
    assert!(loaded.is_reference(length));
    assert!(loaded.driving_dimensions.is_empty());
    let distance = loaded
        .dimension_keys()
        .into_iter()
        .find(|key| matches!(key, DimensionKey::Point(_)))
        .unwrap();
    assert!(loaded.is_reference(distance));
    // Back to driving: the reference flag is removed in place.
    loaded.toggle_reference(length).unwrap();
    reopened.save(&loaded).unwrap();
    let relations = sketch_relations(&reopened);
    let lengths: Vec<&Value> = relations
        .iter()
        .filter(|relation| relation["type"] == "length")
        .collect();
    assert_eq!(lengths.len(), 1);
    assert!(lengths[0].get("reference").is_none());
    assert_eq!(lengths[0]["id"], saved_length["id"]);
    fs::remove_file(path).unwrap();
}

#[test]
fn expressions_and_variables_round_trip_as_web_fields() {
    let path = test_path("expressions");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    let line = draw_line(
        &mut document,
        Point { x: 0.0, y: 0.0 },
        Point { x: 40.0, y: 0.0 },
    );
    document.define_variable(None, "w", "1 in").unwrap();
    let length = document.add_entity_dimension(line).unwrap();
    document.set_dimension_expression(length, "#w * 2").unwrap();
    document.add_point(Point { x: 0.0, y: 20.0 });
    let point = document.shapes.last().unwrap().id();
    let distance = document
        .add_point_dimension((line, 0), Some((point, 0)), Some(PointAxis::Y))
        .unwrap();
    document
        .set_dimension_expression(distance, "#w + 5 mm")
        .unwrap();
    project.save(&document).unwrap();
    let sketch = project.value["items"][0]["data"]["cad"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["type"] == "sketch")
        .unwrap()
        .clone();
    assert_eq!(
        sketch["variables"],
        json!([{"name": "w", "kind": "length", "expression": "1 in"}])
    );
    let relations = sketch_relations(&project);
    let saved_length = relations
        .iter()
        .find(|relation| relation["type"] == "length")
        .unwrap();
    assert_eq!(saved_length["expression"], "#w * 2");
    // Exactly the evaluated value, as the web loader requires.
    assert_eq!(saved_length["value"], json!(50.8));
    let saved_distance = relations
        .iter()
        .find(|relation| relation["type"] == "distance")
        .unwrap();
    assert_eq!(saved_distance["expression"], "#w + 5 mm");
    assert_eq!(saved_distance["value"], json!(30.4));

    let (mut reopened, mut loaded) = ProjectFile::open(&path).unwrap();
    assert_eq!(reopened.read_only_reason(), None);
    assert_eq!(loaded.dims.variables.len(), 1);
    assert_eq!(loaded.dimension_expression(length), Some("#w * 2"));
    loaded.define_variable(Some(0), "w", "10").unwrap();
    assert_eq!(loaded.dimension_value(length), Some(20.0));
    reopened.save(&loaded).unwrap();
    let relations = sketch_relations(&reopened);
    let saved_length = relations
        .iter()
        .find(|relation| relation["type"] == "length")
        .unwrap();
    assert_eq!(saved_length["value"], json!(20.0));
    fs::remove_file(&path).unwrap();
}

#[test]
fn mismatched_expression_or_bad_variables_keep_sketch_read_only() {
    let path = test_path("bad-expression");
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    let line = draw_line(
        &mut document,
        Point { x: 0.0, y: 0.0 },
        Point { x: 40.0, y: 0.0 },
    );
    let length = document.add_entity_dimension(line).unwrap();
    document.set_dimension_expression(length, "2 * 20").unwrap();
    project.save(&document).unwrap();
    for (pointer, replacement) in [
        ("/expression", json!("3 * 20")),
        (
            "/variables",
            json!([{"name": "1x", "kind": "length", "expression": "1"}]),
        ),
    ] {
        let mut value: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let sketch = value["items"][0]["data"]["cad"]["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["type"] == "sketch")
            .unwrap();
        if pointer == "/variables" {
            sketch["variables"] = replacement;
        } else {
            let relation = sketch["relations"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|relation| relation["type"] == "length")
                .unwrap();
            relation["expression"] = replacement;
        }
        let broken = test_path("bad-expression-copy");
        fs::write(&broken, serde_json::to_vec(&value).unwrap()).unwrap();
        let (reopened, _) = ProjectFile::open(&broken).unwrap();
        assert!(reopened.read_only_reason().is_some(), "{pointer}");
        fs::remove_file(broken).unwrap();
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn dimensions_fixture_opens_editable_and_resaves_every_dimension_kind() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/dimensions.pcad");
    let path = test_path("fixture");
    fs::copy(&fixture, &path).unwrap();
    let (mut project, mut document) = ProjectFile::open(&path).unwrap();
    assert_eq!(project.read_only_reason(), None);
    assert_eq!(document.dimension_keys().len(), 4);
    assert_eq!(document.dims.variables.len(), 1);
    assert_eq!(
        document
            .dimension_keys()
            .iter()
            .filter(|key| document.is_reference(**key))
            .count(),
        1
    );
    // Edit the variable so every relation is rewritten by the native app.
    document.define_variable(Some(0), "w", "1 in").unwrap();
    let line = document.dimension_keys()[0];
    document
        .set_dimension_expression(line, "#w * 2 + 1 mm")
        .unwrap();
    project.save(&document).unwrap();
    let relations = sketch_relations(&project);
    let find = |id: &str| {
        relations
            .iter()
            .find(|relation| relation["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(find("len")["expression"], "#w * 2 + 1 mm");
    assert_eq!(find("len")["value"], json!(51.8));
    assert_eq!(find("len")["labelPosition"], json!({"x": 25.0, "y": -12.0}));
    assert_eq!(find("dist")["reference"], true);
    assert_eq!(find("dx")["axis"], "x");
    assert_eq!(find("oy")["expression"], "#w + 4.6 mm");
    fs::remove_file(path).unwrap();
}
