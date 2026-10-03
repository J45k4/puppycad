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
