//! Relation glyph placement, inference hints and automatic constraints.

use super::*;

fn line(id: u32, a: (f32, f32), b: (f32, f32)) -> Shape {
    Shape::Line {
        id,
        a: Point { x: a.0, y: a.1 },
        b: Point { x: b.0, y: b.1 },
    }
}

const ORIGIN: Point = Point { x: 400.0, y: 300.0 };

#[test]
fn horizontal_line_glyph_sits_beside_midpoint_clear_of_its_length_label() {
    let mut document = Document::from_shapes(vec![line(1, (0.0, 0.0), (100.0, 0.0))]);
    document.selected = Some(1);
    document
        .toggle_axis_constraint(AxisConstraint::Horizontal)
        .unwrap();
    document
        .set_dimension(DimensionField::LineLength, 100.0)
        .unwrap();
    let glyphs = document.placed_glyphs(ORIGIN);
    assert_eq!(glyphs.len(), 1);
    let glyph = &glyphs[0];
    assert_eq!(glyph.glyph.label, "H");
    let mid = document.world_to_screen(Point { x: 50.0, y: 0.0 }, ORIGIN);
    // Centered on the midpoint, offset perpendicular to the line.
    assert!((glyph.x + glyph.w * 0.5 - mid.x).abs() < 1e-3);
    assert!(glyph.y + glyph.h < mid.y || glyph.y > mid.y);
    let labels = document.dimension_labels();
    assert_eq!(labels.len(), 1);
    for label in labels {
        let center = document.world_to_screen(label.at, ORIGIN);
        let (w, h) = label_size(&label.text);
        let label_box = PlacedGlyph {
            glyph: glyph.glyph.clone(),
            x: center.x - w * 0.5,
            y: center.y - h * 0.5,
            w,
            h,
        };
        assert!(!glyph.overlaps(&label_box), "glyph overlaps {label:?}");
    }
}

#[test]
fn circle_tangent_and_concentric_glyphs_stack_without_overlap() {
    let circle = |id, x: f32, radius| Shape::Circle {
        id,
        center: Point { x, y: 0.0 },
        radius,
        segments: 64,
    };
    let mut document = Document::from_shapes(vec![
        circle(1, 0.0, 20.0),
        circle(2, 0.0, 10.0),
        circle(3, 40.0, 20.0),
    ]);
    document.selected = Some(2);
    document.toggle_concentric_with(1).unwrap();
    document.selected = Some(1);
    document.toggle_external_tangent_with(3).unwrap();
    let glyphs = document.placed_glyphs(ORIGIN);
    let mine: Vec<_> = glyphs.iter().filter(|g| g.glyph.owner == 1).collect();
    let mut labels: Vec<_> = mine.iter().map(|g| g.glyph.label).collect();
    labels.sort();
    assert_eq!(labels, ["O", "TAN"]);
    for (index, a) in glyphs.iter().enumerate() {
        for b in &glyphs[index + 1..] {
            if a.glyph.owner == b.glyph.owner {
                assert!(!a.overlaps(b), "{a:?} overlaps {b:?}");
            }
        }
    }
    // Hit-testing finds the glyph and reports both partners.
    let tan = mine.iter().find(|g| g.glyph.label == "TAN").unwrap();
    let hit = document
        .glyph_at(
            Point {
                x: tan.x + 2.0,
                y: tan.y + 2.0,
            },
            ORIGIN,
        )
        .unwrap();
    assert_eq!(hit.glyph.label, "TAN");
    assert_eq!(hit.glyph.partners.len(), 2);
}

#[test]
fn vertical_line_glyphs_stack_sideways_without_overlap() {
    let mut document = Document::from_shapes(vec![
        line(1, (0.0, 0.0), (0.0, 80.0)),
        line(2, (30.0, 0.0), (30.0, 60.0)),
    ]);
    document.selected = Some(1);
    document
        .toggle_axis_constraint(AxisConstraint::Vertical)
        .unwrap();
    document.selected = Some(2);
    document
        .toggle_line_relation_with(1, LineRelationKind::Parallel)
        .unwrap();
    document.equal_lines.insert(2, 1);
    let glyphs = document.placed_glyphs(ORIGIN);
    let mine: Vec<_> = glyphs.iter().filter(|g| g.glyph.owner == 1).collect();
    assert_eq!(mine.len(), 3);
    for (index, a) in mine.iter().enumerate() {
        for b in &mine[index + 1..] {
            assert!(!a.overlaps(b), "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn fixed_anchor_glyph_sits_at_the_anchor() {
    let mut document = Document::from_shapes(vec![line(1, (0.0, 0.0), (50.0, 20.0))]);
    document
        .fixed_anchors
        .insert((1, 0), Point { x: 0.0, y: 0.0 });
    let glyphs = document.placed_glyphs(ORIGIN);
    assert_eq!(glyphs.len(), 1);
    assert_eq!(glyphs[0].glyph.label, "FIX");
    assert_eq!(glyphs[0].glyph.anchor, Some(0));
    assert!((glyphs[0].x - (ORIGIN.x + 6.0)).abs() < 1e-3);
}

#[test]
fn inference_detects_near_horizontal_vertical_parallel_and_perpendicular() {
    let document = Document::from_shapes(vec![line(1, (0.0, 0.0), (100.0, 50.0))]);
    // 1 degree off horizontal.
    let start = Point { x: 0.0, y: 100.0 };
    let end = Point {
        x: 100.0,
        y: 100.0 + 100.0 * 1f32.to_radians().tan(),
    };
    assert_eq!(
        document.infer_segment(start, end).map(|i| i.kind),
        Some(InferenceKind::Horizontal)
    );
    assert_eq!(
        document
            .infer_segment(Point { x: 1.5, y: 120.0 }, Point { x: 0.0, y: 30.0 })
            .map(|i| i.kind),
        Some(InferenceKind::Vertical)
    );
    // Parallel to line 1, ending next to it.
    let inference = document
        .infer_segment(Point { x: -20.0, y: 5.0 }, Point { x: 60.0, y: 45.0 })
        .unwrap();
    assert_eq!(inference.kind, InferenceKind::Parallel(1));
    assert_eq!(
        inference.guide,
        (Point { x: 0.0, y: 0.0 }, Point { x: 100.0, y: 50.0 })
    );
    // Perpendicular to line 1 ending on it.
    assert_eq!(
        document
            .infer_segment(Point { x: 80.0, y: -10.0 }, Point { x: 60.0, y: 30.0 })
            .map(|i| i.kind),
        Some(InferenceKind::Perpendicular(1))
    );
    // Off by 10 degrees and far from geometry: no hint.
    let start = Point { x: 200.0, y: 200.0 };
    let end = Point {
        x: 300.0,
        y: 200.0 + 100.0 * 10f32.to_radians().tan(),
    };
    assert_eq!(document.infer_segment(start, end), None);
}

#[test]
fn inference_detects_tangent_to_circle() {
    let document = Document::from_shapes(vec![Shape::Circle {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        radius: 20.0,
        segments: 64,
    }]);
    let at = Point { x: 12.0, y: 16.0 };
    let start = Point {
        x: at.x - 0.8 * 50.0,
        y: at.y + 0.6 * 50.0,
    };
    assert_eq!(
        document.infer_segment(start, at).map(|i| i.kind),
        Some(InferenceKind::Tangent(1))
    );
}

#[test]
fn auto_constraints_add_horizontal_to_a_near_horizontal_line() {
    let mut document = Document {
        tool: Tool::Line,
        ..Document::default()
    };
    document.draft_start = Some(Point { x: 0.0, y: 0.0 });
    let end = Point { x: 100.0, y: 1.5 };
    let (finished, axis) = document.finish_draft_auto(end, true);
    assert!(finished);
    assert_eq!(axis, Some(AxisConstraint::Horizontal));
    let id = document.selected.unwrap();
    assert_eq!(
        document.axis_constraints.get(&id),
        Some(&AxisConstraint::Horizontal)
    );
    let Shape::Line { a, b, .. } = document.shapes[0] else {
        panic!("expected a line");
    };
    assert_eq!(a.y, b.y);
    // One undo removes the line and its relation together.
    document.undo();
    assert!(document.shapes.is_empty());
    assert!(document.axis_constraints.is_empty());
    // Disabled: the line keeps its tilt and gets no relation.
    document.draft_start = Some(Point { x: 0.0, y: 0.0 });
    let (finished, axis) = document.finish_draft_auto(end, false);
    assert!(finished && axis.is_none());
    assert!(document.axis_constraints.is_empty());
    // A clearly diagonal line never gets an axis relation.
    document.draft_start = Some(Point { x: 0.0, y: 50.0 });
    let (_, axis) = document.finish_draft_auto(Point { x: 50.0, y: 100.0 }, true);
    assert!(axis.is_none());
}
