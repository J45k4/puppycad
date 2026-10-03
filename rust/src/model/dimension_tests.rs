//! Dimension objects: labels, hit-testing, point dimensions, reference
//! dimensions, expressions and variables.

use super::*;

fn p(x: f32, y: f32) -> Point {
    Point { x, y }
}

fn line(document: &mut Document, a: Point, b: Point) -> u32 {
    document.tool = Tool::Line;
    document.draft_start = Some(a);
    assert!(document.finish_draft(b));
    document.shapes.last().unwrap().id()
}

fn circle(document: &mut Document, center: Point, rim: Point) -> u32 {
    document.tool = Tool::Circle;
    document.draft_start = Some(center);
    assert!(document.finish_draft(rim));
    document.shapes.last().unwrap().id()
}

const ORIGIN: Point = Point { x: 400.0, y: 300.0 };

#[test]
fn every_driving_dimension_has_a_label_near_its_entity() {
    let mut doc = Document::default();
    let l = line(&mut doc, p(0.0, 0.0), p(40.0, 0.0));
    let c = circle(&mut doc, p(100.0, 0.0), p(110.0, 0.0));
    doc.selected = Some(l);
    doc.set_dimension(DimensionField::LineLength, 50.0).unwrap();
    doc.selected = Some(c);
    doc.set_dimension(DimensionField::CircleDiameter, 30.0)
        .unwrap();
    let labels = doc.dimension_labels();
    assert_eq!(labels.len(), 2);
    let length = &labels[0];
    assert_eq!(
        length.key,
        DimensionKey::Field(l, DimensionField::LineLength)
    );
    assert_eq!(length.text, "50");
    // Offset to one side of the line's midpoint.
    assert!((length.at.x - 25.0).abs() < 1e-3);
    assert!(length.at.y.abs() > 5.0 && length.at.y.abs() < 40.0);
    let diameter = &labels[1];
    assert_eq!(diameter.text, "D30");
    // Leader through the circle, label outside its rim.
    assert!(diameter.at.distance(p(100.0, 0.0)) > 15.0);
    assert_eq!(diameter.lines.len(), 1);
}

#[test]
fn label_hit_testing_uses_screen_label_boxes() {
    let mut doc = Document::default();
    let l = line(&mut doc, p(0.0, 0.0), p(40.0, 0.0));
    doc.selected = Some(l);
    doc.set_dimension(DimensionField::LineLength, 40.0).unwrap();
    let label = doc.dimension_labels().remove(0);
    let center = doc.world_to_screen(label.at, ORIGIN);
    let key = DimensionKey::Field(l, DimensionField::LineLength);
    assert_eq!(doc.dimension_label_at(center, ORIGIN), Some(key));
    assert_eq!(
        doc.dimension_label_at(p(center.x + 9.0, center.y + 5.0), ORIGIN),
        Some(key)
    );
    assert_eq!(
        doc.dimension_label_at(p(center.x + 40.0, center.y), ORIGIN),
        None
    );
    assert_eq!(
        doc.dimension_label_at(p(center.x, center.y + 20.0), ORIGIN),
        None
    );
}

#[test]
fn moving_a_label_is_one_undo_step_and_editing_by_key_keeps_selection() {
    let mut doc = Document::default();
    let l = line(&mut doc, p(0.0, 0.0), p(40.0, 0.0));
    let other = line(&mut doc, p(0.0, 20.0), p(10.0, 20.0));
    doc.selected = Some(l);
    doc.set_dimension(DimensionField::LineLength, 40.0).unwrap();
    let key = DimensionKey::Field(l, DimensionField::LineLength);
    doc.set_dimension_label_position(key, Some(p(5.0, -30.0)))
        .unwrap();
    assert_eq!(doc.dimension_labels()[0].at, p(5.0, -30.0));
    doc.undo();
    assert!(doc.dims.label_positions.is_empty());
    doc.selected = Some(other);
    doc.set_dimension_value(key, 60.0).unwrap();
    assert_eq!(doc.selected, Some(other));
    assert!(matches!(doc.shapes[0], Shape::Line { b, .. } if (b.x - 60.0).abs() < 1e-3));
    doc.set_dimension_label_position(key, Some(p(1.0, 1.0)))
        .unwrap();
    doc.selected = Some(l);
    assert!(doc.delete_selected());
    assert!(doc.dims.label_positions.is_empty());
}

fn point(document: &mut Document, at: Point) -> u32 {
    document.add_point(at);
    document.shapes.last().unwrap().id()
}

#[test]
fn point_distance_dimension_drives_the_second_point() {
    let mut doc = Document::default();
    let a = point(&mut doc, p(0.0, 0.0));
    let l = line(&mut doc, p(30.0, 40.0), p(80.0, 40.0));
    let key = doc.add_point_dimension((a, 0), Some((l, 0)), None).unwrap();
    assert_eq!(doc.dimension_value(key), Some(50.0));
    assert_eq!(doc.dimension_labels()[0].text, "50");
    doc.set_dimension_value(key, 100.0).unwrap();
    let start = doc.dimension_anchor((l, 0)).unwrap();
    assert!((start.distance(p(0.0, 0.0)) - 100.0).abs() < 1e-2);
    assert_eq!(doc.dimension_anchor((a, 0)), Some(p(0.0, 0.0)));
    doc.undo();
    assert_eq!(doc.dimension_value(key), Some(50.0));
    doc.undo();
    assert!(doc.dimension_keys().is_empty());
    assert!(doc.dimension_value(key).is_none());
    assert!(doc.add_point_dimension((a, 0), Some((a, 0)), None).is_err());
    assert!(doc.set_dimension_value(key, -1.0).is_err());
}

#[test]
fn axis_dimensions_between_points_and_from_origin() {
    let mut doc = Document::default();
    let a = point(&mut doc, p(10.0, 5.0));
    let b = point(&mut doc, p(30.0, 25.0));
    let dx = doc
        .add_point_dimension((a, 0), Some((b, 0)), Some(PointAxis::X))
        .unwrap();
    assert_eq!(doc.dimension_value(dx), Some(20.0));
    doc.set_dimension_value(dx, -15.0).unwrap();
    let at = doc.dimension_anchor((b, 0)).unwrap();
    assert!((at.x + 5.0).abs() < 1e-3 && (at.y - 25.0).abs() < 1e-3);
    // Absolute Y from the origin creates one fixed origin point, reused.
    let y = doc
        .add_point_dimension((a, 0), None, Some(PointAxis::Y))
        .unwrap();
    let x = doc
        .add_point_dimension((a, 0), None, Some(PointAxis::X))
        .unwrap();
    let origins = doc
        .shapes
        .iter()
        .filter(|shape| matches!(shape, Shape::Point { at, .. } if *at == p(0.0, 0.0)))
        .count();
    assert_eq!(origins, 1);
    assert_eq!(doc.dimension_text(y).unwrap(), "Y5");
    doc.set_dimension_value(y, 12.0).unwrap();
    doc.set_dimension_value(x, 7.0).unwrap();
    assert!(doc.dimension_anchor((a, 0)).unwrap().distance(p(7.0, 12.0)) < 1e-3);
    let origin_id = doc.dims.point_dimensions[&match y {
        DimensionKey::Point(id) => id,
        _ => unreachable!(),
    }]
        .a
        .0;
    assert_eq!(doc.dimension_anchor((origin_id, 0)), Some(p(0.0, 0.0)));
    // Deleting a dimensioned point removes its dimensions.
    doc.selected = Some(a);
    assert!(doc.delete_selected());
    assert!(doc.dims.point_dimensions.is_empty());
}

#[test]
fn conflicting_point_dimension_rolls_back() {
    let mut doc = Document::default();
    let l = line(&mut doc, p(0.0, 0.0), p(40.0, 0.0));
    doc.selected = Some(l);
    doc.set_dimension(DimensionField::LineLength, 40.0).unwrap();
    let key = doc.add_point_dimension((l, 0), Some((l, 1)), None).unwrap();
    assert!(doc.set_dimension_value(key, 60.0).is_err());
    assert_eq!(doc.dimension_value(key), Some(40.0));
    assert!(matches!(doc.shapes[0], Shape::Line { b, .. } if (b.x - 40.0).abs() < 1e-3));
    doc.delete_dimension(key).unwrap();
    assert_eq!(doc.dimension_keys().len(), 1);
}

#[test]
fn entity_dimension_picks_the_next_missing_field() {
    let mut doc = Document {
        tool: Tool::Rectangle,
        draft_start: Some(p(0.0, 0.0)),
        ..Document::default()
    };
    assert!(doc.finish_draft(p(30.0, 20.0)));
    let rect = doc.shapes[0].id();
    assert_eq!(
        doc.add_entity_dimension(rect),
        Ok(DimensionKey::Field(rect, DimensionField::RectangleWidth))
    );
    assert_eq!(
        doc.add_entity_dimension(rect),
        Ok(DimensionKey::Field(rect, DimensionField::RectangleHeight))
    );
    assert!(doc.add_entity_dimension(rect).is_err());
    assert_eq!(
        doc.dimension_value(DimensionKey::Field(rect, DimensionField::RectangleHeight)),
        Some(20.0)
    );
    let a = point(&mut doc, p(50.0, 50.0));
    assert!(doc.add_entity_dimension(a).is_err());
    assert_eq!(doc.dimension_anchor_near(p(50.5, 50.0), 2.0), Some((a, 0)));
    assert_eq!(doc.dimension_anchor_near(p(55.0, 50.0), 2.0), None);
}
