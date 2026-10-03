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

#[test]
fn reference_dimensions_measure_and_add_no_constraint() {
    let mut doc = Document::default();
    let l = line(&mut doc, p(0.0, 0.0), p(40.0, 0.0));
    let key = doc.add_entity_dimension(l).unwrap();
    assert_eq!(doc.toggle_reference(key), Ok(true));
    assert!(doc.driving_dimensions.is_empty());
    assert_eq!(doc.dimension_keys(), vec![key]);
    assert_eq!(doc.dimension_text(key).unwrap(), "(40)");
    assert!(doc.set_dimension_value(key, 10.0).is_err());
    // The line is free again: moving its end changes the measurement.
    doc.selected = Some(l);
    doc.set_dimension(DimensionField::LineLength, 55.0).unwrap();
    assert!(!doc.is_reference(key));
    doc.toggle_reference(key).unwrap();
    if let Shape::Line { b, .. } = &mut doc.shapes[0] {
        b.x = 70.0;
    }
    assert_eq!(doc.dimension_text(key).unwrap(), "(70)");
    // Back to driving at the measured value; one undo step each.
    assert_eq!(doc.toggle_reference(key), Ok(false));
    assert_eq!(doc.dimension_value(key), Some(70.0));
    doc.undo();
    assert!(doc.is_reference(key));
    assert!(doc.toggle_reference(DimensionKey::Angle(99)).is_err());
}

#[test]
fn reference_point_dimension_is_skipped_by_the_solver() {
    let mut doc = Document::default();
    let a = point(&mut doc, p(0.0, 0.0));
    let b = point(&mut doc, p(3.0, 4.0));
    let key = doc.add_point_dimension((a, 0), Some((b, 0)), None).unwrap();
    doc.toggle_reference(key).unwrap();
    let (problem, _) = doc.build_problem(&HashSet::new(), true, None);
    assert!(problem.constraints.is_empty());
    assert_eq!(doc.dimension_text(key).unwrap(), "(5)");
    doc.toggle_reference(key).unwrap();
    let (problem, _) = doc.build_problem(&HashSet::new(), true, None);
    assert_eq!(problem.constraints.len(), 1);
}

#[test]
fn dimension_expressions_use_units_and_variables() {
    let mut doc = Document::default();
    let l = line(&mut doc, p(0.0, 0.0), p(40.0, 0.0));
    let key = doc.add_entity_dimension(l).unwrap();
    let depth = doc.history.len();
    assert_eq!(doc.set_dimension_expression(key, "1 in + 4.6mm"), Ok(30.0));
    assert_eq!(doc.history.len(), depth + 1);
    assert_eq!(doc.dimension_expression(key), Some("1 in + 4.6mm"));
    assert_eq!(doc.dimension_text(key).unwrap(), "1 in + 4.6mm = 30");
    assert!(doc.set_dimension_expression(key, "10 deg").is_err());
    assert!(doc.set_dimension_expression(key, "#missing").is_err());
    assert_eq!(doc.dimension_value(key), Some(30.0));
    // A plain number drops the expression.
    doc.set_dimension_expression(key, "25").unwrap();
    assert!(doc.dims.expressions.is_empty());

    doc.define_variable(None, "w", "20 mm").unwrap();
    doc.define_variable(None, "#half", "#w / 2").unwrap();
    assert_eq!(
        doc.dims.variables[1].kind,
        crate::sketch_value::ValueKind::Length
    );
    doc.set_dimension_expression(key, "#w * 2 + #half").unwrap();
    assert_eq!(doc.dimension_value(key), Some(50.0));
    assert!(matches!(doc.shapes[0], Shape::Line { b, .. } if (b.x - 50.0).abs() < 1e-3));

    // Changing a variable re-evaluates dependents as one undo step.
    let depth = doc.history.len();
    doc.define_variable(Some(0), "w", "30").unwrap();
    assert_eq!(doc.history.len(), depth + 1);
    assert_eq!(doc.dimension_value(key), Some(75.0));
    assert!(matches!(doc.shapes[0], Shape::Line { b, .. } if (b.x - 75.0).abs() < 1e-3));
    doc.undo();
    assert_eq!(doc.dimension_value(key), Some(50.0));
    assert_eq!(doc.dims.variables[0].expression, "20 mm");

    // Renaming rewrites references; deleting a used variable is refused.
    doc.define_variable(Some(0), "width", "20 mm").unwrap();
    assert_eq!(doc.dimension_expression(key), Some("#width * 2 + #half"));
    assert_eq!(doc.dims.variables[1].expression, "#width / 2");
    assert!(doc.delete_variable(0).is_err());
    assert!(doc.define_variable(None, "width", "1").is_err());
    assert!(doc.define_variable(None, "loop", "#loop").is_err());
    // A variable change that breaks geometry rolls back completely.
    doc.selected = Some(l);
    assert!(doc.define_variable(Some(0), "width", "-40 mm").is_err());
    assert_eq!(doc.dims.variables[0].expression, "20 mm");
    assert_eq!(doc.dimension_value(key), Some(50.0));
}

#[test]
fn reference_toggle_drops_expression() {
    let mut doc = Document::default();
    let l = line(&mut doc, p(0.0, 0.0), p(40.0, 0.0));
    let key = doc.add_entity_dimension(l).unwrap();
    doc.set_dimension_expression(key, "2 * 20").unwrap();
    doc.toggle_reference(key).unwrap();
    assert!(doc.dims.expressions.is_empty());
}
