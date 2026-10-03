use super::*;

#[test]
fn center_arc_uses_start_radius_and_clockwise_sweep() {
    let center = Point { x: 0.0, y: 0.0 };
    let start = Point { x: 10.0, y: 0.0 };
    let end = Point { x: 0.0, y: 20.0 };
    let ccw = Shape::arc_from_center_points(1, center, start, end, false).unwrap();
    let cw = Shape::arc_from_center_points(2, center, start, end, true).unwrap();
    assert!(
        matches!(ccw, Shape::Arc { radius: 10.0, sweep, .. } if (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-6)
    );
    assert!(
        matches!(cw, Shape::Arc { radius: 10.0, sweep, .. } if (sweep + 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-6)
    );
    assert!(Shape::arc_from_center_points(3, center, start, start, false).is_none());
    let mut doc = Document {
        tool: Tool::CenterArc,
        arc_clockwise: true,
        ..Document::default()
    };
    doc.draft_start = Some(center);
    assert!(!doc.finish_draft(start));
    assert!(doc.finish_draft(end));
    assert!(matches!(doc.shapes[0], Shape::Arc { sweep, .. } if sweep < -4.0));
}

#[test]
fn three_point_circle_constructs_circumcircle_and_rejects_collinear_points() {
    let first = Point { x: 0.0, y: 10.0 };
    let second = Point { x: 10.0, y: 0.0 };
    let third = Point { x: 0.0, y: -10.0 };
    let mut doc = Document {
        tool: Tool::ThreePointCircle,
        ..Document::default()
    };
    doc.draft_start = Some(first);
    assert!(!doc.finish_draft(second));
    assert!(doc.finish_draft(third));
    assert!(matches!(
        doc.shapes[0],
        Shape::Circle {
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
            ..
        }
    ));
    assert!(doc.undo());
    assert!(doc.shapes.is_empty());
    assert!(Shape::circle_from_three_points(1, first, Point { x: 0.0, y: 5.0 }, third).is_none());
}

#[test]
fn axis_constraint_snaps_preserves_length_and_survives_copy_undo() {
    let mut doc = Document::from_shapes(vec![Shape::Line {
        id: 1,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 3.0, y: 4.0 },
    }]);
    doc.selected = Some(1);
    assert_eq!(
        doc.toggle_axis_constraint(AxisConstraint::Horizontal),
        Ok(true)
    );
    assert_eq!(
        doc.shapes[0],
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 5.0, y: 0.0 }
        }
    );
    assert!(doc.copy_selected(Point { x: 10.0, y: 0.0 }));
    assert_eq!(
        doc.axis_constraints.get(&2),
        Some(&AxisConstraint::Horizontal)
    );
    assert!(!doc.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
    assert!(doc.rotate_selected(90.0, Point { x: 0.0, y: 0.0 }, false));
    assert_eq!(
        doc.axis_constraints.get(&2),
        Some(&AxisConstraint::Vertical)
    );
    assert!(doc.undo());
    assert_eq!(
        doc.axis_constraints.get(&2),
        Some(&AxisConstraint::Horizontal)
    );
    assert!(doc.undo());
    assert!(!doc.axis_constraints.contains_key(&2));
    assert!(doc.undo());
    assert!(doc.axis_constraints.is_empty());
    assert_eq!(
        doc.shapes[0],
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 3.0, y: 4.0 }
        }
    );
}

#[test]
fn construction_toggle_is_undoable_and_copied() {
    let mut doc = Document::from_shapes(vec![Shape::Circle {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        radius: 10.0,
        segments: 64,
    }]);
    doc.selected = Some(1);
    assert_eq!(doc.toggle_construction(), Some(true));
    assert!(doc.construction.contains(&1));
    assert!(doc.copy_selected(Point { x: 30.0, y: 0.0 }));
    assert!(doc.construction.contains(&2));
    assert!(doc.undo());
    assert!(!doc.construction.contains(&2));
    assert!(doc.undo());
    assert!(!doc.construction.contains(&1));
    assert!(doc.redo());
    assert!(doc.construction.contains(&1));
}

#[test]
fn viewport_round_trip_and_anchor_zoom() {
    let mut doc = Document::default();
    let origin = Point { x: 220.0, y: 80.0 };
    let screen = Point { x: 500.0, y: 340.0 };
    let before = doc.screen_to_world(screen, origin);
    doc.zoom_at(screen, origin, 1.5);
    assert_eq!(doc.screen_to_world(screen, origin), before);
    assert_eq!(doc.world_to_screen(before, origin), screen);
    assert_eq!(
        doc.world_to_screen(Point { x: 0.0, y: 10.0 }, origin).y,
        origin.y - 10.0 * doc.zoom + doc.pan.y
    );
}

#[test]
fn fit_shapes_centers_all_geometry_and_respects_viewport_margin() {
    let mut doc = Document::from_shapes(vec![
        Shape::Circle {
            id: 1,
            center: Point {
                x: 1000.0,
                y: -500.0,
            },
            radius: 100.0,
            segments: 64,
        },
        Shape::Rectangle {
            id: 2,
            a: Point {
                x: 1300.0,
                y: -400.0,
            },
            b: Point {
                x: 1500.0,
                y: -200.0,
            },
        },
    ]);
    assert!(doc.fit_shapes(800.0, 600.0));
    let origin = Point { x: 400.0, y: 300.0 };
    for shape in &doc.shapes {
        let (min, max) = shape.bounds().unwrap();
        for corner in [min, max] {
            let screen = doc.world_to_screen(corner, origin);
            assert!((48.0..=752.0).contains(&screen.x));
            assert!((48.0..=552.0).contains(&screen.y));
        }
    }
    let before = (doc.zoom, doc.pan);
    assert!(!doc.fit_shapes(50.0, 600.0));
    assert_eq!((doc.zoom, doc.pan), before);
    assert!(!Document::default().fit_shapes(800.0, 600.0));
}

#[test]
fn snap_prefers_nearby_geometry_then_origin_and_grid() {
    let mut doc = Document::from_shapes(vec![Shape::Line {
        id: 1,
        a: Point { x: 10.25, y: 20.5 },
        b: Point { x: 30.25, y: 20.5 },
    }]);
    assert_eq!(
        doc.snap_point(Point { x: 12.0, y: 21.0 }, true, true),
        Some(SnapResult {
            point: Point { x: 10.25, y: 20.5 },
            kind: SnapKind::Anchor
        })
    );
    assert_eq!(
        doc.snap_point(Point { x: 20.2, y: 19.9 }, true, true),
        Some(SnapResult {
            point: Point { x: 20.25, y: 20.5 },
            kind: SnapKind::Midpoint
        })
    );
    assert_eq!(
        doc.snap_point(Point { x: 0.1, y: -0.2 }, true, true),
        Some(SnapResult {
            point: Point { x: 0.0, y: 0.0 },
            kind: SnapKind::Origin
        })
    );
    assert_eq!(
        doc.snap_point(Point { x: 12.0, y: 21.0 }, true, false),
        Some(SnapResult {
            point: Point { x: 12.0, y: 21.0 },
            kind: SnapKind::Grid
        })
    );
    assert_eq!(
        doc.snap_point(Point { x: 12.0, y: 21.0 }, false, false),
        None
    );
    doc.zoom = 8.0;
    assert_eq!(
        doc.snap_point(Point { x: 12.0, y: 21.0 }, false, true),
        None
    );
}

#[test]
fn snaps_to_finite_line_intersection() {
    let doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 10.0, y: 10.0 },
            b: Point { x: 30.0, y: 30.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 10.0, y: 30.0 },
            b: Point { x: 30.0, y: 10.0 },
        },
    ]);
    assert_eq!(
        doc.snap_point(Point { x: 21.0, y: 20.5 }, false, true),
        Some(SnapResult {
            point: Point { x: 20.0, y: 20.0 },
            kind: SnapKind::Intersection,
        })
    );
    assert_eq!(
        doc.snap_point(Point { x: 21.0, y: 20.5 }, false, false),
        None
    );
    let outside = Shape::Line {
        id: 3,
        a: Point { x: 40.0, y: 10.0 },
        b: Point { x: 40.0, y: 30.0 },
    };
    assert!(finite_intersections(&doc.shapes[0], &outside).is_empty());
    let parallel = Shape::Line {
        id: 4,
        a: Point { x: 10.0, y: 11.0 },
        b: Point { x: 30.0, y: 31.0 },
    };
    assert!(finite_intersections(&doc.shapes[0], &parallel).is_empty());
}

#[test]
fn line_spline_intersection_snaps_and_respects_finite_line() {
    let spline = Shape::Spline {
        id: 1,
        mode: SplineMode::Control,
        points: vec![
            Point { x: 10.0, y: 10.0 },
            Point { x: 10.0, y: 30.0 },
            Point { x: 30.0, y: 30.0 },
            Point { x: 30.0, y: 10.0 },
        ],
        closed: false,
    };
    let line = Shape::Line {
        id: 2,
        a: Point { x: 20.0, y: 0.0 },
        b: Point { x: 20.0, y: 30.0 },
    };
    let doc = Document::from_shapes(vec![spline.clone(), line.clone()]);
    assert_eq!(finite_intersections(&spline, &line).len(), 1);
    assert_eq!(finite_intersections(&line, &spline).len(), 1);
    assert_eq!(
        doc.snap_point(Point { x: 20.4, y: 24.7 }, false, true),
        Some(SnapResult {
            point: Point { x: 20.0, y: 25.0 },
            kind: SnapKind::Intersection
        })
    );
    let short = Shape::Line {
        id: 2,
        a: Point { x: 20.0, y: 0.0 },
        b: Point { x: 20.0, y: 20.0 },
    };
    assert!(finite_intersections(&spline, &short).is_empty());
}

#[test]
fn snaps_to_finite_line_circle_intersections_and_tangent() {
    let line = Shape::Line {
        id: 1,
        a: Point { x: 10.0, y: 20.0 },
        b: Point { x: 30.0, y: 20.0 },
    };
    let circle = Shape::Circle {
        id: 2,
        center: Point { x: 20.0, y: 20.0 },
        radius: 6.0,
        segments: 64,
    };
    let doc = Document::from_shapes(vec![line.clone(), circle.clone()]);
    assert_eq!(
        doc.snap_point(Point { x: 14.4, y: 20.3 }, false, true),
        Some(SnapResult {
            point: Point { x: 14.0, y: 20.0 },
            kind: SnapKind::Intersection,
        })
    );
    assert_eq!(
        doc.snap_point(Point { x: 25.6, y: 20.3 }, false, true),
        Some(SnapResult {
            point: Point { x: 26.0, y: 20.0 },
            kind: SnapKind::Intersection,
        })
    );
    let tangent = Shape::Line {
        id: 3,
        a: Point { x: 10.0, y: 26.0 },
        b: Point { x: 30.0, y: 26.0 },
    };
    assert_eq!(
        finite_intersections(&tangent, &circle),
        vec![Point { x: 20.0, y: 26.0 }]
    );
    let short = Shape::Line {
        id: 4,
        a: Point { x: 1.0, y: 20.0 },
        b: Point { x: 11.0, y: 20.0 },
    };
    assert!(finite_intersections(&short, &circle).is_empty());
}

#[test]
fn circular_intersections_respect_tangency_and_arc_sweeps() {
    let left = Shape::Circle {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        radius: 5.0,
        segments: 64,
    };
    let right = Shape::Circle {
        id: 2,
        center: Point { x: 6.0, y: 0.0 },
        radius: 5.0,
        segments: 64,
    };
    assert_eq!(
        finite_intersections(&left, &right),
        vec![Point { x: 3.0, y: 4.0 }, Point { x: 3.0, y: -4.0 }]
    );
    let tangent = Shape::Circle {
        id: 3,
        center: Point { x: 10.0, y: 0.0 },
        radius: 5.0,
        segments: 64,
    };
    assert_eq!(
        finite_intersections(&left, &tangent),
        vec![Point { x: 5.0, y: 0.0 }]
    );
    let coincident = Shape::Circle {
        id: 4,
        center: Point { x: 0.0, y: 0.0 },
        radius: 5.0,
        segments: 64,
    };
    assert!(finite_intersections(&left, &coincident).is_empty());
    let upper_arc = Shape::Arc {
        id: 5,
        center: Point { x: 0.0, y: 0.0 },
        radius: 5.0,
        start_angle: 0.0,
        sweep: std::f64::consts::PI,
        segments: 64,
    };
    assert_eq!(
        finite_intersections(&upper_arc, &right),
        vec![Point { x: 3.0, y: 4.0 }]
    );
    let crossing = Shape::Line {
        id: 6,
        a: Point { x: 3.0, y: -8.0 },
        b: Point { x: 3.0, y: 8.0 },
    };
    assert_eq!(
        finite_intersections(&crossing, &upper_arc),
        vec![Point { x: 3.0, y: 4.0 }]
    );
}

#[test]
fn composite_shape_boundaries_have_finite_deduplicated_intersections() {
    let rectangle = Shape::Rectangle {
        id: 1,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 10.0, y: 10.0 },
    };
    let through_corner = Shape::Line {
        id: 2,
        a: Point { x: -5.0, y: -5.0 },
        b: Point { x: 0.0, y: 0.0 },
    };
    assert_eq!(
        finite_intersections(&rectangle, &through_corner),
        vec![Point { x: 0.0, y: 0.0 }]
    );
    let through_middle = Shape::Line {
        id: 3,
        a: Point { x: -5.0, y: 5.0 },
        b: Point { x: 15.0, y: 5.0 },
    };
    assert_eq!(
        finite_intersections(&rectangle, &through_middle),
        vec![Point { x: 10.0, y: 5.0 }, Point { x: 0.0, y: 5.0 }]
    );
    let rotated = Shape::RotatedRectangle {
        id: 4,
        center: Point { x: 0.0, y: 0.0 },
        width: 10.0,
        height: 10.0,
        rotation: 45.0,
    };
    assert_eq!(
        finite_intersections(
            &rotated,
            &Shape::Line {
                id: 5,
                a: Point { x: -10.0, y: 0.0 },
                b: Point { x: 10.0, y: 0.0 }
            }
        )
        .len(),
        2
    );
    let triangle = Shape::Polygon {
        id: 6,
        center: Point { x: 0.0, y: 0.0 },
        radius: 10.0,
        sides: 3,
        rotation: 0.0,
    };
    assert_eq!(
        finite_intersections(
            &triangle,
            &Shape::Line {
                id: 7,
                a: Point { x: 0.0, y: -20.0 },
                b: Point { x: 0.0, y: 20.0 }
            }
        )
        .len(),
        2
    );
}

#[test]
fn slot_intersections_follow_straight_sides_and_round_caps() {
    let slot = Shape::Capsule {
        id: 1,
        from: Point { x: 0.0, y: 0.0 },
        to: Point { x: 20.0, y: 0.0 },
        width: 10.0,
        arc_segments: 48,
    };
    let middle = Shape::Line {
        id: 2,
        a: Point { x: 10.0, y: -10.0 },
        b: Point { x: 10.0, y: 10.0 },
    };
    assert_eq!(
        finite_intersections(&slot, &middle),
        vec![Point { x: 10.0, y: 5.0 }, Point { x: 10.0, y: -5.0 }]
    );
    let cap = Shape::Line {
        id: 3,
        a: Point { x: 23.0, y: -10.0 },
        b: Point { x: 23.0, y: 10.0 },
    };
    assert_eq!(
        finite_intersections(&slot, &cap),
        vec![Point { x: 23.0, y: -4.0 }, Point { x: 23.0, y: 4.0 }]
    );
    let junction = Shape::Line {
        id: 4,
        a: Point { x: 20.0, y: 5.0 },
        b: Point { x: 20.0, y: 10.0 },
    };
    assert_eq!(
        finite_intersections(&slot, &junction),
        vec![Point { x: 20.0, y: 5.0 }]
    );
    let none = Shape::Line {
        id: 5,
        a: Point { x: 26.0, y: -10.0 },
        b: Point { x: 26.0, y: 10.0 },
    };
    assert!(finite_intersections(&slot, &none).is_empty());
}

#[test]
fn finite_line_ellipse_intersections_include_rotated_axes_and_tangent() {
    let ellipse = Shape::Ellipse {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        rx: 10.0,
        ry: 5.0,
        rotation: 0.0,
        segments: 64,
    };
    let crossing = Shape::Line {
        id: 2,
        a: Point { x: -20.0, y: 0.0 },
        b: Point { x: 20.0, y: 0.0 },
    };
    assert_eq!(
        finite_intersections(&crossing, &ellipse),
        vec![Point { x: -10.0, y: 0.0 }, Point { x: 10.0, y: 0.0 }]
    );
    let tangent = Shape::Line {
        id: 3,
        a: Point { x: -20.0, y: 5.0 },
        b: Point { x: 20.0, y: 5.0 },
    };
    assert_eq!(
        finite_intersections(&ellipse, &tangent),
        vec![Point { x: 0.0, y: 5.0 }]
    );
    let short = Shape::Line {
        id: 4,
        a: Point { x: 11.0, y: 0.0 },
        b: Point { x: 20.0, y: 0.0 },
    };
    assert!(finite_intersections(&short, &ellipse).is_empty());
    let rotated = Shape::Ellipse {
        id: 6,
        center: Point { x: 0.0, y: 0.0 },
        rx: 10.0,
        ry: 5.0,
        rotation: 45.0,
        segments: 64,
    };
    let diagonal = Shape::Line {
        id: 5,
        a: Point { x: -20.0, y: -20.0 },
        b: Point { x: 20.0, y: 20.0 },
    };
    let hits = finite_intersections(&diagonal, &rotated);
    assert_eq!(hits.len(), 2);
    let expected = 10.0 / 2.0_f32.sqrt();
    assert!((hits[0].x + expected).abs() < 1e-4 && (hits[0].y + expected).abs() < 1e-4);
    assert!((hits[1].x - expected).abs() < 1e-4 && (hits[1].y - expected).abs() < 1e-4);
}

#[test]
fn circle_ellipse_contacts_include_tangent_and_respect_arc_sweep() {
    let ellipse = Shape::Ellipse {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        rx: 100.0,
        ry: 50.0,
        rotation: 0.0,
        segments: 128,
    };
    let circle = Shape::Circle {
        id: 2,
        center: Point { x: 50.0, y: 0.0 },
        radius: 50.0,
        segments: 64,
    };
    let hits = finite_intersections(&circle, &ellipse);
    assert_eq!(hits.len(), 3);
    assert!(
        hits.iter()
            .any(|point| point.distance(Point { x: 100.0, y: 0.0 }) < 1e-3)
    );
    assert!(hits.iter().any(|point| point.distance(Point {
        x: 100.0 / 3.0,
        y: 50.0 * (8.0_f32 / 9.0).sqrt()
    }) < 1e-3));
    assert!(hits.iter().any(|point| point.distance(Point {
        x: 100.0 / 3.0,
        y: -50.0 * (8.0_f32 / 9.0).sqrt()
    }) < 1e-3));
    let upper_arc = Shape::Arc {
        id: 3,
        center: Point { x: 50.0, y: 0.0 },
        radius: 50.0,
        start_angle: 0.0,
        sweep: std::f64::consts::PI,
        segments: 64,
    };
    assert_eq!(finite_intersections(&upper_arc, &ellipse).len(), 2);
    let disjoint = Shape::Circle {
        id: 4,
        center: Point { x: 200.0, y: 0.0 },
        radius: 10.0,
        segments: 64,
    };
    assert!(finite_intersections(&disjoint, &ellipse).is_empty());
    let coincident = Shape::Ellipse {
        id: 5,
        center: Point { x: 0.0, y: 0.0 },
        rx: 10.0,
        ry: 10.0,
        rotation: 0.0,
        segments: 64,
    };
    let same_circle = Shape::Circle {
        id: 6,
        center: Point { x: 0.0, y: 0.0 },
        radius: 10.0,
        segments: 64,
    };
    assert!(finite_intersections(&same_circle, &coincident).is_empty());
}

#[test]
fn ellipse_ellipse_contacts_include_four_crossings_tangent_and_coincident() {
    let first = Shape::Ellipse {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        rx: 10.0,
        ry: 5.0,
        rotation: 0.0,
        segments: 128,
    };
    let perpendicular = Shape::Ellipse {
        id: 2,
        center: Point { x: 0.0, y: 0.0 },
        rx: 5.0,
        ry: 10.0,
        rotation: 0.0,
        segments: 128,
    };
    let hits = finite_intersections(&first, &perpendicular);
    assert_eq!(hits.len(), 4);
    for hit in &hits {
        assert!(
            (hit.x.abs() - 20.0_f32.sqrt()).abs() < 1e-3
                && (hit.y.abs() - 20.0_f32.sqrt()).abs() < 1e-3
        );
    }
    let tangent = Shape::Ellipse {
        id: 3,
        center: Point { x: 20.0, y: 0.0 },
        rx: 10.0,
        ry: 5.0,
        rotation: 0.0,
        segments: 128,
    };
    assert_eq!(
        finite_intersections(&first, &tangent),
        vec![Point { x: 10.0, y: 0.0 }]
    );
    let same = Shape::Ellipse {
        id: 4,
        center: Point { x: 0.0, y: 0.0 },
        rx: 10.0,
        ry: 5.0,
        rotation: 0.0,
        segments: 128,
    };
    assert!(finite_intersections(&first, &same).is_empty());
}

#[test]
fn line_endpoint_linked_to_point_follows_move_drag_and_history() {
    let mut doc = Document::default();
    let at = Point { x: 10.0, y: 20.0 };
    assert!(doc.add_point(at));
    doc.tool = Tool::Line;
    doc.draft_start = Some(at);
    assert!(doc.finish_draft(Point { x: 50.0, y: 20.0 }));
    assert_eq!(doc.point_links.get(&(2, 0)), Some(&1));
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 5.0, y: -3.0 }));
    assert!(matches!(
        doc.shapes[1],
        Shape::Line {
            a: Point { x: 15.0, y: 17.0 },
            ..
        }
    ));
    assert!(doc.undo());
    assert_eq!(doc.point_links.get(&(2, 0)), Some(&1));
    assert!(matches!(doc.shapes[1], Shape::Line { a, .. } if a == at));
    assert!(doc.redo());
    doc.selected = Some(1);
    assert!(doc.begin_drag(Point { x: 15.0, y: 17.0 }));
    assert!(doc.drag_to(Point { x: 25.0, y: 27.0 }));
    assert!(matches!(
        doc.shapes[1],
        Shape::Line {
            a: Point { x: 25.0, y: 27.0 },
            ..
        }
    ));
    assert!(doc.cancel_drag());
    assert!(matches!(
        doc.shapes[1],
        Shape::Line {
            a: Point { x: 15.0, y: 17.0 },
            ..
        }
    ));
    // Moving the line carries the linked point with it in one undo step.
    doc.selected = Some(2);
    assert!(doc.move_selected(Point { x: 1.0, y: 0.0 }));
    assert!(matches!(doc.shapes[0], Shape::Point { at, .. } if at == Point { x: 16.0, y: 17.0 }));
    assert!(doc.undo());
    assert!(matches!(doc.shapes[0], Shape::Point { at, .. } if at == Point { x: 15.0, y: 17.0 }));
    doc.selected = Some(2);
    assert!(doc.delete_selected());
    assert!(doc.point_links.is_empty());
}

#[test]
fn spline_endpoints_follow_linked_points_and_split_keeps_original_ends() {
    let mut doc = Document::default();
    let start = Point { x: 0.0, y: 0.0 };
    let end = Point { x: 30.0, y: 0.0 };
    assert!(doc.add_point(start));
    assert!(doc.add_point(end));
    let spline_id = doc
        .add_spline(
            SplineMode::Control,
            vec![
                start,
                Point { x: 10.0, y: 12.0 },
                Point { x: 20.0, y: 12.0 },
                end,
            ],
            false,
        )
        .unwrap();
    assert_eq!(doc.point_links.get(&(spline_id, 0)), Some(&1));
    assert_eq!(doc.point_links.get(&(spline_id, 1)), Some(&2));
    doc.selected = Some(spline_id);
    doc.selected_spline_point = Some(0);
    // Editing a linked end handle carries the point along.
    assert_eq!(doc.set_selected_spline_coordinate(true, 2.0), Ok(()));
    assert_eq!(doc.shapes[0].center(), Point { x: 2.0, y: 0.0 });
    assert!(doc.undo());
    assert_eq!(doc.shapes[0].center(), start);
    doc.selected = Some(spline_id);
    doc.selected_spline_point = Some(0);
    assert!(doc.begin_drag(start));
    assert!(doc.drag_to(Point { x: 1.0, y: 4.0 }));
    assert_eq!(doc.shapes[0].center(), Point { x: 1.0, y: 4.0 });
    assert!(doc.cancel_drag());
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 2.0, y: 3.0 }));
    assert!(
        matches!(&doc.shapes[2], Shape::Spline { points, .. } if points[0] == Point { x: 2.0, y: 3.0 })
    );
    assert!(doc.undo());
    assert!(doc.redo());
    doc.selected = Some(spline_id);
    let new_id = doc
        .split_selected_spline(Point { x: 15.0, y: 9.0 })
        .unwrap();
    assert_eq!(doc.point_links.get(&(spline_id, 0)), Some(&1));
    assert_eq!(doc.point_links.get(&(new_id, 1)), Some(&2));
    assert!(!doc.point_links.contains_key(&(spline_id, 1)));
}

#[test]
fn point_coordinate_edits_move_linked_line_and_respect_driving_length() {
    let mut document = Document::from_shapes(vec![
        Shape::Point {
            id: 1,
            at: Point { x: 10.0, y: 20.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 10.0, y: 20.0 },
            b: Point { x: 50.0, y: 20.0 },
        },
    ]);
    document.point_links.insert((2, 0), 1);
    document.selected = Some(1);
    assert!(
        document
            .set_selected_point_coordinate(true, f32::NAN)
            .is_err()
    );
    assert_eq!(document.set_selected_point_coordinate(true, 15.0), Ok(()));
    assert_eq!(document.set_selected_point_coordinate(false, -5.0), Ok(()));
    assert!(
        matches!(document.shapes[0], Shape::Point { at, .. } if at == Point { x: 15.0, y: -5.0 })
    );
    assert!(matches!(document.shapes[1], Shape::Line { a, .. } if a == Point { x: 15.0, y: -5.0 }));
    assert!(document.undo());
    assert!(
        matches!(document.shapes[0], Shape::Point { at, .. } if at == Point { x: 15.0, y: 20.0 })
    );
    assert!(document.undo());
    assert!(
        matches!(document.shapes[0], Shape::Point { at, .. } if at == Point { x: 10.0, y: 20.0 })
    );
    document
        .driving_dimensions
        .insert((2, DimensionField::LineLength), 40.0);
    assert!(document.set_selected_point_coordinate(true, 20.0).is_err());
    assert!(
        matches!(document.shapes[0], Shape::Point { at, .. } if at == Point { x: 10.0, y: 20.0 })
    );
}

#[test]
fn fixed_point_and_spline_handle_block_edits_and_undo_cleanly() {
    let mut document = Document::from_shapes(vec![
        Shape::Point {
            id: 1,
            at: Point { x: 10.0, y: 20.0 },
        },
        Shape::Spline {
            id: 2,
            mode: SplineMode::Control,
            closed: false,
            points: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 20.0 },
                Point { x: 20.0, y: -20.0 },
                Point { x: 30.0, y: 0.0 },
            ],
        },
    ]);
    document.selected = Some(1);
    assert_eq!(document.toggle_fixed_anchor(), Ok(true));
    assert_eq!(
        document.fixed_anchors.get(&(1, 0)),
        Some(&Point { x: 10.0, y: 20.0 })
    );
    assert!(document.set_selected_point_coordinate(true, 15.0).is_err());
    assert!(!document.begin_drag(Point { x: 10.0, y: 20.0 }));
    assert!(!document.move_selected(Point { x: 1.0, y: 0.0 }));
    assert!(!document.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
    assert!(!document.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
    assert!(document.copy_selected(Point { x: 5.0, y: 0.0 }));
    assert!(!document.fixed_anchors.contains_key(&(3, 0)));
    assert!(document.undo());
    assert_eq!(
        document.fixed_anchors.get(&(1, 0)),
        Some(&Point { x: 10.0, y: 20.0 })
    );
    document.selected = Some(2);
    document.selected_spline_point = Some(1);
    assert_eq!(document.toggle_fixed_anchor(), Ok(true));
    assert!(
        document
            .set_selected_spline_coordinate(false, 25.0)
            .is_err()
    );
    assert!(!document.begin_drag(Point { x: 10.0, y: 20.0 }));
    assert!(
        document
            .split_selected_spline(Point { x: 15.0, y: 0.0 })
            .is_err()
    );
    assert!(
        document
            .trim_selected_spline(Point { x: 15.0, y: 0.0 })
            .is_err()
    );
    assert!(document.undo());
    assert!(!document.fixed_anchors.contains_key(&(2, 1)));
    document.selected = Some(2);
    document.selected_spline_point = Some(1);
    assert_eq!(document.toggle_fixed_anchor(), Ok(true));
    assert_eq!(document.toggle_fixed_anchor(), Ok(false));
    assert!(document.set_selected_spline_coordinate(false, 25.0).is_ok());
    document.selected = Some(1);
    assert!(document.delete_selected());
    assert!(!document.fixed_anchors.contains_key(&(1, 0)));
}

#[test]
fn fixed_line_endpoints_block_edits_and_follow_split() {
    let mut doc = Document::from_shapes(vec![Shape::Line {
        id: 1,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 40.0, y: 0.0 },
    }]);
    doc.selected = Some(1);
    assert!(
        doc.toggle_fixed_line_endpoint_at(Point { x: 20.0, y: 0.0 })
            .is_err()
    );
    assert_eq!(
        doc.toggle_fixed_line_endpoint_at(Point { x: 40.0, y: 0.0 }),
        Ok(true)
    );
    assert_eq!(
        doc.fixed_anchors.get(&(1, 1)),
        Some(&Point { x: 40.0, y: 0.0 })
    );
    assert!(!doc.move_selected(Point { x: 1.0, y: 0.0 }));
    assert!(doc.set_dimension(DimensionField::LineLength, 50.0).is_err());
    assert_eq!(doc.split_selected_line(Point { x: 15.0, y: 0.0 }), Ok(2));
    assert_eq!(
        doc.fixed_anchors.get(&(2, 1)),
        Some(&Point { x: 40.0, y: 0.0 })
    );
    assert!(!doc.fixed_anchors.contains_key(&(1, 1)));
    assert!(doc.undo());
    assert_eq!(
        doc.fixed_anchors.get(&(1, 1)),
        Some(&Point { x: 40.0, y: 0.0 })
    );
    doc.selected = Some(1);
    assert_eq!(
        doc.toggle_fixed_line_endpoint_at(Point { x: 40.0, y: 0.0 }),
        Ok(false)
    );
    assert!(doc.move_selected(Point { x: 1.0, y: 0.0 }));
}

#[test]
fn fixed_curve_centers_keep_radius_edits_but_block_transforms() {
    let mut doc = Document::from_shapes(vec![Shape::Circle {
        id: 1,
        center: Point { x: 12.0, y: -5.0 },
        radius: 10.0,
        segments: 64,
    }]);
    doc.selected = Some(1);
    assert_eq!(doc.toggle_fixed_anchor(), Ok(true));
    assert_eq!(
        doc.fixed_anchors.get(&(1, 0)),
        Some(&Point { x: 12.0, y: -5.0 })
    );
    assert!(!doc.move_selected(Point { x: 2.0, y: 0.0 }));
    assert!(!doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
    assert_eq!(
        doc.set_dimension(DimensionField::CircleDiameter, 30.0),
        Ok(())
    );
    assert!(matches!(
        doc.shapes[0],
        Shape::Circle {
            center: Point { x: 12.0, y: -5.0 },
            radius: 15.0,
            ..
        }
    ));
    assert!(doc.undo());
    assert_eq!(
        doc.fixed_anchors.get(&(1, 0)),
        Some(&Point { x: 12.0, y: -5.0 })
    );
    doc.selected = Some(1);
    assert_eq!(doc.toggle_fixed_anchor(), Ok(false));
    assert!(doc.move_selected(Point { x: 2.0, y: 0.0 }));
}

#[test]
fn fixed_arc_endpoint_transfers_on_split_and_blocks_extension() {
    let mut doc = Document::from_shapes(vec![Shape::Arc {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        radius: 10.0,
        start_angle: 0.0,
        sweep: std::f64::consts::FRAC_PI_2,
        segments: 64,
    }]);
    doc.selected = Some(1);
    let end = doc.fixed_anchor_position(1, 2).unwrap();
    assert_eq!(
        doc.toggle_fixed_arc_anchor_at(Point { x: 0.0, y: 10.0 }),
        Ok((2, true))
    );
    assert_eq!(doc.fixed_anchors.get(&(1, 2)), Some(&end));
    assert!(doc.extend_selected_arc(Point { x: 0.0, y: 10.0 }).is_err());
    let split = Point {
        x: 10.0 / 2.0_f32.sqrt(),
        y: 10.0 / 2.0_f32.sqrt(),
    };
    assert_eq!(doc.split_selected_arc(split), Ok(2));
    assert_eq!(doc.fixed_anchors.get(&(2, 2)), Some(&end));
    assert!(!doc.fixed_anchors.contains_key(&(1, 2)));
    assert!(doc.undo());
    assert_eq!(doc.fixed_anchors.get(&(1, 2)), Some(&end));
    doc.selected = Some(1);
    assert_eq!(
        doc.toggle_fixed_arc_anchor_at(Point { x: 0.0, y: 10.0 }),
        Ok((2, false))
    );
}

#[test]
fn fixed_elliptic_arc_end_transfers_on_split_and_drops_when_trimmed() {
    let ellipse_arc = Shape::EllipticArc {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        rx: 20.0,
        ry: 10.0,
        rotation: 0.0,
        start_angle: 0.0,
        sweep: std::f64::consts::FRAC_PI_2,
        segments: 64,
    };
    let boundary = Shape::Line {
        id: 2,
        a: Point { x: -30.0, y: 5.0 },
        b: Point { x: 30.0, y: 5.0 },
    };
    let mut doc = Document::from_shapes(vec![ellipse_arc, boundary]);
    doc.selected = Some(1);
    let end = doc.fixed_anchor_position(1, 2).unwrap();
    assert_eq!(doc.toggle_fixed_arc_anchor_at(end), Ok((2, true)));
    assert!(doc.extend_selected_elliptic_arc(end).is_err());
    let split = Point {
        x: 20.0 / 2.0_f32.sqrt(),
        y: 10.0 / 2.0_f32.sqrt(),
    };
    assert_eq!(doc.split_selected_elliptic_arc(split), Ok(3));
    assert_eq!(doc.fixed_anchors.get(&(3, 2)), Some(&end));
    assert!(doc.undo());
    doc.selected = Some(1);
    assert_eq!(
        doc.trim_selected_elliptic_arc(Point { x: 10.0, y: 8.66 }),
        Ok(1)
    );
    assert!(!doc.fixed_anchors.contains_key(&(1, 2)));
}

#[test]
fn named_shape_anchors_guard_only_edits_that_move_them() {
    let mut doc = Document::from_shapes(vec![
        Shape::Ellipse {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 0.0,
            segments: 64,
        },
        Shape::RotatedRectangle {
            id: 2,
            center: Point { x: 60.0, y: 0.0 },
            width: 20.0,
            height: 10.0,
            rotation: 30.0,
        },
        Shape::Polygon {
            id: 3,
            center: Point { x: 100.0, y: 0.0 },
            radius: 10.0,
            sides: 5,
            rotation: 0.0,
        },
        Shape::Capsule {
            id: 4,
            from: Point { x: 140.0, y: 0.0 },
            to: Point { x: 160.0, y: 0.0 },
            width: 10.0,
            arc_segments: 16,
        },
    ]);
    doc.selected = Some(1);
    assert_eq!(
        doc.toggle_fixed_shape_anchor_at(Point { x: 20.0, y: 0.0 }),
        Ok((1, true))
    );
    assert!(
        doc.set_dimension(DimensionField::RectangleWidth, 50.0)
            .is_err()
    );
    assert_eq!(
        doc.set_dimension(DimensionField::RectangleHeight, 30.0),
        Ok(())
    );
    doc.selected = Some(2);
    let corner = doc.fixed_anchor_position(2, 1).unwrap();
    assert_eq!(doc.toggle_fixed_shape_anchor_at(corner), Ok((1, true)));
    assert!(
        doc.set_dimension(DimensionField::RectangleWidth, 30.0)
            .is_err()
    );
    doc.selected = Some(3);
    assert_eq!(
        doc.toggle_fixed_shape_anchor_at(Point { x: 110.0, y: 0.0 }),
        Ok((1, true))
    );
    assert!(doc.set_polygon_sides(6).is_err());
    doc.selected = Some(4);
    assert_eq!(
        doc.toggle_fixed_shape_anchor_at(Point { x: 160.0, y: 0.0 }),
        Ok((1, true))
    );
    assert_eq!(doc.set_capsule_width(12.0), Ok(()));
    assert!(!doc.move_selected(Point { x: 1.0, y: 0.0 }));
}

#[test]
fn fixed_corner_rectangle_resizes_about_anchored_corner() {
    let mut doc = Document::from_shapes(vec![Shape::Rectangle {
        id: 1,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 20.0, y: 10.0 },
    }]);
    doc.selected = Some(1);
    assert_eq!(
        doc.toggle_fixed_line_endpoint_at(Point { x: 0.0, y: 0.0 }),
        Ok(true)
    );
    assert_eq!(
        doc.set_dimension(DimensionField::RectangleWidth, 30.0),
        Ok(())
    );
    assert_eq!(
        doc.set_dimension(DimensionField::RectangleHeight, 15.0),
        Ok(())
    );
    assert_eq!(
        doc.shapes[0],
        Shape::Rectangle {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 30.0, y: 15.0 }
        }
    );
    assert_eq!(
        doc.fixed_anchors.get(&(1, 0)),
        Some(&Point { x: 0.0, y: 0.0 })
    );
    assert_eq!(
        doc.toggle_fixed_line_endpoint_at(Point { x: 30.0, y: 15.0 }),
        Ok(true)
    );
    assert!(
        doc.set_dimension(DimensionField::RectangleWidth, 40.0)
            .is_err()
    );
    assert!(
        doc.set_dimension(DimensionField::RectangleHeight, 20.0)
            .is_err()
    );
    assert!(!doc.move_selected(Point { x: 1.0, y: 1.0 }));
}

#[test]
fn equal_lines_follow_source_length_and_undo_as_one_edit() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: 20.0 },
            b: Point { x: 0.0, y: 30.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_equal_line_with(1), Ok(true));
    assert_eq!(doc.equal_lines.get(&2), Some(&1));
    assert!(matches!(
        doc.shapes[1],
        Shape::Line {
            b: Point { x: 0.0, y: 40.0 },
            ..
        }
    ));
    // A length on the Equal target now drives the source through the solver.
    assert_eq!(doc.set_dimension(DimensionField::LineLength, 25.0), Ok(()));
    assert!(matches!(
        doc.shapes[0],
        Shape::Line { a: Point { x: 0.0, y: 0.0 }, b, .. } if (b.x - 25.0).abs() < 1e-4 && b.y.abs() < 1e-4
    ));
    assert!(doc.undo());
    doc.selected = Some(2);
    assert!(doc.split_selected_line(Point { x: 0.0, y: 30.0 }).is_err());
    doc.selected = Some(1);
    assert_eq!(doc.set_dimension(DimensionField::LineLength, 30.0), Ok(()));
    assert!(matches!(
        doc.shapes[1],
        Shape::Line {
            b: Point { x: 0.0, y: 50.0 },
            ..
        }
    ));
    assert!(doc.undo());
    assert!(matches!(
        doc.shapes[0],
        Shape::Line {
            b: Point { x: 20.0, y: 0.0 },
            ..
        }
    ));
    assert!(matches!(
        doc.shapes[1],
        Shape::Line {
            b: Point { x: 0.0, y: 40.0 },
            ..
        }
    ));
    doc.selected = Some(2);
    assert_eq!(doc.toggle_equal_line_with(1), Ok(false));
    assert!(doc.equal_lines.is_empty());
    assert_eq!(doc.set_dimension(DimensionField::LineLength, 15.0), Ok(()));
}

#[test]
fn equal_radius_tracks_circle_diameter_and_undo() {
    let mut doc = Document::from_shapes(vec![
        Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        },
        Shape::Arc {
            id: 2,
            center: Point { x: 40.0, y: 0.0 },
            radius: 5.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 24,
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_equal_radius_with(1), Ok(true));
    assert!(matches!(doc.shapes[1], Shape::Arc { radius: 10.0, .. }));
    doc.selected = Some(1);
    assert_eq!(
        doc.set_dimension(DimensionField::CircleDiameter, 30.0),
        Ok(())
    );
    assert!(matches!(doc.shapes[1], Shape::Arc { radius: 15.0, .. }));
    assert!(doc.undo());
    assert!(matches!(doc.shapes[1], Shape::Arc { radius: 10.0, .. }));
    doc.selected = Some(2);
    assert_eq!(doc.toggle_equal_radius_with(1), Ok(false));
}

#[test]
fn arc_radius_drives_equal_circle_and_fixed_end_blocks_edit() {
    let mut doc = Document::from_shapes(vec![
        Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 32,
        },
        Shape::Circle {
            id: 2,
            center: Point { x: 40.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_equal_radius_with(1), Ok(true));
    assert!(
        doc.set_dimension(DimensionField::CircleDiameter, 30.0)
            .is_err()
    );
    doc.selected = Some(1);
    assert_eq!(doc.set_dimension(DimensionField::CurveRadius, 20.0), Ok(()));
    assert!(matches!(doc.shapes[1], Shape::Circle { radius: 20.0, .. }));
    assert!(doc.undo());
    assert!(matches!(doc.shapes[1], Shape::Circle { radius: 10.0, .. }));
    doc.selected = Some(1);
    doc.fixed_anchors.insert((1, 1), Point { x: 10.0, y: 0.0 });
    assert!(
        doc.set_dimension(DimensionField::CurveRadius, 30.0)
            .is_err()
    );
}

#[test]
fn polygon_radius_is_editable_until_vertex_is_fixed() {
    let mut doc = Document::from_shapes(vec![Shape::Polygon {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        radius: 10.0,
        sides: 6,
        rotation: 0.0,
    }]);
    doc.selected = Some(1);
    assert_eq!(doc.set_dimension(DimensionField::CurveRadius, 25.0), Ok(()));
    assert!(matches!(doc.shapes[0], Shape::Polygon { radius: 25.0, .. }));
    doc.fixed_anchors.insert((1, 1), Point { x: 25.0, y: 0.0 });
    assert!(
        doc.set_dimension(DimensionField::CurveRadius, 30.0)
            .is_err()
    );
}

#[test]
fn signed_arc_sweep_and_shape_rotation_are_undoable_dimensions() {
    let mut doc = Document::from_shapes(vec![
        Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 20.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 32,
        },
        Shape::Polygon {
            id: 2,
            center: Point { x: 60.0, y: 0.0 },
            radius: 10.0,
            sides: 6,
            rotation: 0.0,
        },
    ]);
    doc.selected = Some(1);
    assert_eq!(doc.set_dimension(DimensionField::ArcSweep, -90.0), Ok(()));
    assert!(
        matches!(doc.shapes[0], Shape::Arc { sweep, .. } if (sweep + std::f64::consts::FRAC_PI_2).abs() < 1e-6)
    );
    assert!(doc.set_dimension(DimensionField::ArcSweep, 0.0).is_err());
    assert!(doc.set_dimension(DimensionField::ArcSweep, 360.0).is_err());
    doc.fixed_anchors.insert((1, 2), Point { x: 0.0, y: 0.0 });
    assert!(doc.set_dimension(DimensionField::ArcSweep, 45.0).is_err());
    doc.selected = Some(2);
    assert_eq!(doc.set_dimension(DimensionField::Rotation, -30.0), Ok(()));
    assert!(matches!(
        doc.shapes[1],
        Shape::Polygon {
            rotation: -30.0,
            ..
        }
    ));
    assert!(doc.undo());
    assert!(matches!(
        doc.shapes[1],
        Shape::Polygon { rotation: 0.0, .. }
    ));
}

#[test]
fn scale_preserves_angular_dimensions_and_rotate_updates_rotation() {
    let mut doc = Document::from_shapes(vec![Shape::EllipticArc {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        rx: 20.0,
        ry: 10.0,
        rotation: 0.0,
        start_angle: 0.0,
        sweep: 1.0,
        segments: 32,
    }]);
    doc.selected = Some(1);
    assert_eq!(doc.set_dimension(DimensionField::ArcSweep, -90.0), Ok(()));
    assert_eq!(doc.set_dimension(DimensionField::Rotation, -20.0), Ok(()));
    assert!(doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
    assert_eq!(
        doc.driving_dimensions.get(&(1, DimensionField::ArcSweep)),
        Some(&-90.0)
    );
    assert_eq!(
        doc.driving_dimensions.get(&(1, DimensionField::Rotation)),
        Some(&-20.0)
    );
    assert!(doc.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
    assert_eq!(
        doc.driving_dimensions.get(&(1, DimensionField::Rotation)),
        Some(&10.0)
    );
    assert_eq!(
        doc.driving_dimensions.get(&(1, DimensionField::ArcSweep)),
        Some(&-90.0)
    );
}

#[test]
fn line_rotation_dimension_updates_parallel_target() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: 30.0 },
            b: Point { x: 10.0, y: 30.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(
        doc.toggle_line_relation_with(1, LineRelationKind::Parallel),
        Ok(true)
    );
    doc.selected = Some(1);
    assert_eq!(doc.set_dimension(DimensionField::Rotation, -90.0), Ok(()));
    assert!(
        matches!(doc.shapes[0], Shape::Line { b: Point { x, y }, .. } if x.abs() < 1e-4 && (y + 20.0).abs() < 1e-4)
    );
    assert!(
        matches!(doc.shapes[1], Shape::Line { b: Point { x, y }, .. } if x.abs() < 1e-4 && ((y - 20.0).abs() < 1e-4 || (y - 40.0).abs() < 1e-4))
    );
}

#[test]
fn equal_ellipse_axes_follow_source_dimensions_and_undo() {
    let mut doc = Document::from_shapes(vec![
        Shape::Ellipse {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 0.0,
            segments: 64,
        },
        Shape::EllipticArc {
            id: 2,
            center: Point { x: 50.0, y: 0.0 },
            rx: 8.0,
            ry: 6.0,
            rotation: 20.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 32,
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_equal_ellipse_with(1), Ok(true));
    assert!(matches!(
        doc.shapes[1],
        Shape::EllipticArc {
            rx: 20.0,
            ry: 10.0,
            ..
        }
    ));
    assert!(
        doc.set_dimension(DimensionField::RectangleWidth, 50.0)
            .is_err()
    );
    doc.selected = Some(1);
    assert_eq!(
        doc.set_dimension(DimensionField::RectangleWidth, 60.0),
        Ok(())
    );
    assert!(matches!(
        doc.shapes[1],
        Shape::EllipticArc {
            rx: 30.0,
            ry: 10.0,
            ..
        }
    ));
    assert_eq!(
        doc.set_dimension(DimensionField::RectangleHeight, 30.0),
        Ok(())
    );
    assert!(matches!(
        doc.shapes[1],
        Shape::EllipticArc {
            rx: 30.0,
            ry: 15.0,
            ..
        }
    ));
    assert!(doc.undo());
    assert!(matches!(
        doc.shapes[1],
        Shape::EllipticArc {
            rx: 30.0,
            ry: 10.0,
            ..
        }
    ));
    doc.selected = Some(2);
    assert_eq!(doc.toggle_equal_ellipse_with(1), Ok(false));
    assert!(doc.equal_ellipses.is_empty());
    assert_eq!(
        doc.set_dimension(DimensionField::RectangleWidth, 40.0),
        Ok(())
    );
}

#[test]
fn concentric_curves_move_together_and_undo() {
    let mut doc = Document::from_shapes(vec![
        Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        },
        Shape::Ellipse {
            id: 2,
            center: Point { x: 30.0, y: 10.0 },
            rx: 20.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 64,
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_concentric_with(1), Ok(true));
    assert_eq!(doc.shapes[1].center(), Point { x: 0.0, y: 0.0 });
    assert_eq!(doc.concentric.get(&2), Some(&1));
    assert!(doc.move_selected(Point { x: 5.0, y: -2.0 }));
    assert_eq!(doc.shapes[0].center(), Point { x: 5.0, y: -2.0 });
    assert_eq!(doc.shapes[1].center(), Point { x: 5.0, y: -2.0 });
    assert!(doc.undo());
    assert_eq!(doc.shapes[0].center(), Point { x: 0.0, y: 0.0 });
    doc.selected = Some(1);
    assert!(doc.begin_drag(Point { x: 0.0, y: 0.0 }));
    assert!(doc.drag_to(Point { x: 10.0, y: 10.0 }));
    assert!(doc.finish_drag());
    assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 10.0 });
    assert!(doc.undo());
    doc.selected = Some(2);
    assert_eq!(doc.toggle_concentric_with(1), Ok(false));
    assert!(doc.concentric.is_empty());
}

#[test]
fn parallel_and_perpendicular_lines_follow_source_direction() {
    for kind in [
        LineRelationKind::Parallel,
        LineRelationKind::Perpendicular,
        LineRelationKind::Collinear,
    ] {
        let mut doc = Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 20.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 30.0 },
                b: Point { x: 10.0, y: 40.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_line_relation_with(1, kind), Ok(true));
        assert_eq!(doc.line_relations.get(&2), Some(&(1, kind)));
        let target = &doc.shapes[1];
        let Shape::Line { a, b, .. } = target else {
            panic!("target line missing")
        };
        let length = a.distance(*b);
        assert!((length - 2.0_f32.sqrt() * 10.0).abs() < 1e-4);
        assert!(match kind {
            LineRelationKind::Parallel => (b.y - a.y).abs() < 1e-4,
            LineRelationKind::Collinear => (b.y - a.y).abs() < 1e-4 && a.y.abs() < 1e-4,
            LineRelationKind::Perpendicular => (b.x - a.x).abs() < 1e-4,
            LineRelationKind::Angle(_) => unreachable!(),
        });
        // Rotating the target turns the source with it.
        assert!(doc.rotate_selected(30.0, Point { x: 0.0, y: 0.0 }, false));
        let (Shape::Line { a: sa, b: sb, .. }, Shape::Line { a: ta, b: tb, .. }) =
            (&doc.shapes[0], &doc.shapes[1])
        else {
            panic!()
        };
        let source = (
            (sb.x - sa.x) / sa.distance(*sb),
            (sb.y - sa.y) / sa.distance(*sb),
        );
        let target = (
            (tb.x - ta.x) / ta.distance(*tb),
            (tb.y - ta.y) / ta.distance(*tb),
        );
        let cross = source.0 * target.1 - source.1 * target.0;
        let dot = source.0 * target.0 + source.1 * target.1;
        assert!(match kind {
            LineRelationKind::Perpendicular => dot.abs() < 1e-4,
            _ => cross.abs() < 1e-4,
        });
        assert!(
            (target.1.atan2(target.0).to_degrees()
                - match kind {
                    LineRelationKind::Perpendicular => 120.0,
                    _ => 30.0,
                })
            .abs()
                < 1e-3
        );
        assert!(doc.undo());
        doc.selected = Some(1);
        assert!(doc.rotate_selected(45.0, Point { x: 0.0, y: 0.0 }, false));
        let Shape::Line {
            a: source_a,
            b: source_b,
            ..
        } = doc.shapes[0]
        else {
            panic!()
        };
        let Shape::Line {
            a: target_a,
            b: target_b,
            ..
        } = doc.shapes[1]
        else {
            panic!()
        };
        let sx = source_b.x - source_a.x;
        let sy = source_b.y - source_a.y;
        let tx = target_b.x - target_a.x;
        let ty = target_b.y - target_a.y;
        assert!(match kind {
            LineRelationKind::Parallel => (sx * ty - sy * tx).abs() < 1e-3,
            LineRelationKind::Collinear =>
                (sx * ty - sy * tx).abs() < 1e-3
                    && ((target_a.x - source_a.x) * sy - (target_a.y - source_a.y) * sx).abs()
                        < 1e-3,
            LineRelationKind::Perpendicular => (sx * tx + sy * ty).abs() < 1e-3,
            LineRelationKind::Angle(_) => unreachable!(),
        });
        assert!(doc.undo());
        assert!(matches!(
            doc.shapes[0],
            Shape::Line {
                b: Point { x: 20.0, y: 0.0 },
                ..
            }
        ));
        doc.selected = Some(1);
        assert_eq!(
            doc.toggle_axis_constraint(AxisConstraint::Vertical),
            Ok(true)
        );
        let Shape::Line {
            a: source_a,
            b: source_b,
            ..
        } = doc.shapes[0]
        else {
            panic!()
        };
        let Shape::Line {
            a: target_a,
            b: target_b,
            ..
        } = doc.shapes[1]
        else {
            panic!()
        };
        let sx = source_b.x - source_a.x;
        let sy = source_b.y - source_a.y;
        let tx = target_b.x - target_a.x;
        let ty = target_b.y - target_a.y;
        assert!(match kind {
            LineRelationKind::Parallel => (sx * ty - sy * tx).abs() < 1e-3,
            LineRelationKind::Collinear =>
                (sx * ty - sy * tx).abs() < 1e-3
                    && ((target_a.x - source_a.x) * sy - (target_a.y - source_a.y) * sx).abs()
                        < 1e-3,
            LineRelationKind::Perpendicular => (sx * tx + sy * ty).abs() < 1e-3,
            LineRelationKind::Angle(_) => unreachable!(),
        });
        assert!(doc.undo());
        doc.selected = Some(2);
        assert_eq!(doc.toggle_line_relation_with(1, kind), Ok(false));
        assert!(doc.line_relations.is_empty());
    }
}

#[test]
fn collinear_lines_follow_source_translation_and_target_stays_on_line() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 5.0, y: 10.0 },
            b: Point { x: 15.0, y: 20.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(
        doc.toggle_line_relation_with(1, LineRelationKind::Collinear),
        Ok(true)
    );
    assert!(
        matches!(doc.shapes[1], Shape::Line { a: Point { y, .. }, b: Point { y: end, .. }, .. } if y.abs() < 1e-4 && end.abs() < 1e-4)
    );
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 0.0, y: 12.0 }));
    assert!(
        matches!(doc.shapes[1], Shape::Line { a: Point { y, .. }, b: Point { y: end, .. }, .. } if (y - 12.0).abs() < 1e-4 && (end - 12.0).abs() < 1e-4)
    );
    doc.selected = Some(2);
    assert!(doc.move_selected(Point { x: 5.0, y: 8.0 }));
    assert!(
        matches!(doc.shapes[1], Shape::Line { a: Point { x, y }, b: Point { y: end, .. }, .. } if (x - 10.0).abs() < 1e-4 && (y - 12.0).abs() < 1e-4 && (end - 12.0).abs() < 1e-4)
    );
    assert!(doc.undo());
    assert!(
        matches!(doc.shapes[1], Shape::Line { a: Point { x, y }, .. } if (x - 5.0).abs() < 1e-4 && (y - 12.0).abs() < 1e-4)
    );
}

#[test]
fn signed_line_angle_drives_target_and_follows_source_rotation() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: 30.0 },
            b: Point { x: 10.0, y: 40.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_line_angle_with(1), Ok(true));
    assert!(
        matches!(doc.line_relations.get(&2), Some((1, LineRelationKind::Angle(value))) if (*value - 45.0).abs() < 1e-4)
    );
    assert_eq!(doc.set_line_relation_angle(-30.0), Ok(()));
    assert!(
        matches!(doc.shapes[1], Shape::Line { a, b, .. } if ((b.y - a.y) / (b.x - a.x) + 30.0_f32.to_radians().tan()).abs() < 1e-4)
    );
    doc.selected = Some(1);
    assert!(doc.rotate_selected(90.0, Point { x: 0.0, y: 0.0 }, false));
    assert!(
        matches!(doc.shapes[1], Shape::Line { a, b, .. } if ((b.y - a.y) / (b.x - a.x) - 60.0_f32.to_radians().tan()).abs() < 1e-4)
    );
    assert!(doc.undo());
    assert!(
        matches!(doc.line_relations.get(&2), Some((1, LineRelationKind::Angle(value))) if (*value + 30.0).abs() < 1e-4)
    );
    doc.selected = Some(2);
    assert_eq!(doc.toggle_line_angle_with(1), Ok(false));
}

#[test]
fn signed_radius_difference_drives_circle_and_survives_undo() {
    let mut doc = Document::from_shapes(vec![
        Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 20.0,
            segments: 64,
        },
        Shape::Circle {
            id: 2,
            center: Point { x: 60.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_radius_difference_with(1), Ok(true));
    assert_eq!(doc.radius_differences.get(&2), Some(&(1, 10.0)));
    assert_eq!(doc.set_radius_difference(-5.0), Ok(()));
    assert!(matches!(doc.shapes[1], Shape::Circle { radius, .. } if (radius - 25.0).abs() < 1e-4));
    assert_eq!(
        doc.set_dimension(DimensionField::CircleDiameter, 30.0),
        Err("Radius is driven by Radial distance".into())
    );
    doc.selected = Some(1);
    assert_eq!(
        doc.set_dimension(DimensionField::CircleDiameter, 60.0),
        Ok(())
    );
    assert!(matches!(doc.shapes[1], Shape::Circle { radius, .. } if (radius - 35.0).abs() < 1e-4));
    assert!(doc.undo());
    assert!(matches!(doc.shapes[1], Shape::Circle { radius, .. } if (radius - 25.0).abs() < 1e-4));
    doc.selected = Some(2);
    assert_eq!(doc.toggle_radius_difference_with(1), Ok(false));
}

#[test]
fn internal_tangent_circle_and_arc_contacts_follow_radius_and_move() {
    let mut doc = Document::from_shapes(vec![
        Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 20.0,
            segments: 64,
        },
        Shape::Circle {
            id: 2,
            center: Point { x: 15.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_internal_tangent_with(1), Ok(true));
    assert_eq!(doc.internal_tangents.get(&2), Some(&1));
    assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 10.0).abs() < 1e-4);
    doc.selected = Some(1);
    assert_eq!(
        doc.set_dimension(DimensionField::CircleDiameter, 50.0),
        Ok(())
    );
    assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 15.0).abs() < 1e-4);
    doc.selected = Some(2);
    assert!(doc.move_selected(Point { x: 5.0, y: 5.0 }));
    assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 15.0).abs() < 1e-4);
    assert!(doc.undo());
    doc.selected = Some(2);
    assert_eq!(doc.toggle_internal_tangent_with(1), Ok(false));

    let mut arcs = Document::from_shapes(vec![
        Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 20.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
        Shape::Arc {
            id: 2,
            center: Point { x: 15.0, y: 0.0 },
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
    ]);
    arcs.selected = Some(2);
    assert_eq!(arcs.toggle_internal_tangent_with(1), Ok(true));
    assert!((arcs.shapes[0].center().distance(arcs.shapes[1].center()) - 10.0).abs() < 1e-4);
}

#[test]
fn external_tangent_circles_follow_source_and_reject_invalid_arc_contact() {
    let mut doc = Document::from_shapes(vec![
        Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 20.0,
            segments: 64,
        },
        Shape::Circle {
            id: 2,
            center: Point { x: 40.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_external_tangent_with(1), Ok(true));
    assert_eq!(doc.external_tangents.get(&2), Some(&1));
    assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 30.0).abs() < 1e-4);
    doc.selected = Some(1);
    assert_eq!(
        doc.set_dimension(DimensionField::CircleDiameter, 50.0),
        Ok(())
    );
    assert!((doc.shapes[0].center().distance(doc.shapes[1].center()) - 35.0).abs() < 1e-4);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_external_tangent_with(1), Ok(false));

    let mut arc_doc = Document::from_shapes(vec![
        Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 20.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
        Shape::Arc {
            id: 2,
            center: Point { x: 30.0, y: 0.0 },
            radius: 10.0,
            start_angle: std::f64::consts::PI,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
    ]);
    arc_doc.selected = Some(2);
    assert_eq!(arc_doc.toggle_external_tangent_with(1), Ok(true));
    assert!(
        (arc_doc.shapes[0]
            .center()
            .distance(arc_doc.shapes[1].center())
            - 30.0)
            .abs()
            < 1e-4
    );
}

#[test]
fn line_circle_tangent_moves_selected_shape_and_follows_source() {
    let line = Shape::Line {
        id: 1,
        a: Point { x: -20.0, y: 0.0 },
        b: Point { x: 20.0, y: 0.0 },
    };
    let circle = Shape::Circle {
        id: 2,
        center: Point { x: 0.0, y: 20.0 },
        radius: 10.0,
        segments: 64,
    };
    let mut doc = Document::from_shapes(vec![line.clone(), circle.clone()]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_external_tangent_with(1), Ok(true));
    assert!((doc.shapes[1].center().y - 10.0).abs() < 1e-4);
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 0.0, y: 5.0 }));
    assert!((doc.shapes[1].center().y - 15.0).abs() < 1e-4);

    let mut reverse = Document::from_shapes(vec![line, circle]);
    reverse.selected = Some(1);
    assert_eq!(reverse.toggle_external_tangent_with(2), Ok(true));
    assert!((reverse.shapes[0].center().y - 10.0).abs() < 1e-4);
    reverse.selected = Some(2);
    assert!(reverse.move_selected(Point { x: 0.0, y: 5.0 }));
    assert!((reverse.shapes[0].center().y - 15.0).abs() < 1e-4);
}

#[test]
fn line_ellipse_tangent_uses_rotated_support_and_arc_sweep() {
    let line = Shape::Line {
        id: 1,
        a: Point { x: -40.0, y: 0.0 },
        b: Point { x: 40.0, y: 0.0 },
    };
    let ellipse = Shape::Ellipse {
        id: 2,
        center: Point { x: 0.0, y: 30.0 },
        rx: 20.0,
        ry: 10.0,
        rotation: 0.0,
        segments: 128,
    };
    let mut doc = Document::from_shapes(vec![line.clone(), ellipse.clone()]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_external_tangent_with(1), Ok(true));
    assert!((doc.shapes[1].center().y - 10.0).abs() < 1e-4);
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 0.0, y: 5.0 }));
    assert!((doc.shapes[1].center().y - 15.0).abs() < 1e-4);

    let mut reverse = Document::from_shapes(vec![line, ellipse]);
    reverse.selected = Some(1);
    assert_eq!(reverse.toggle_external_tangent_with(2), Ok(true));
    assert!((reverse.shapes[0].center().y - 20.0).abs() < 1e-4);

    let mut arc_doc = Document::from_shapes(vec![
        reverse.shapes[0].clone(),
        Shape::EllipticArc {
            id: 2,
            center: Point { x: 0.0, y: 30.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 0.0,
            start_angle: std::f64::consts::PI,
            sweep: std::f64::consts::PI,
            segments: 128,
        },
    ]);
    arc_doc.selected = Some(2);
    assert_eq!(arc_doc.toggle_external_tangent_with(1), Ok(true));
}

#[test]
fn midpoint_point_follows_line_move_and_length_edit() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Point {
            id: 2,
            at: Point { x: 3.0, y: 5.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_midpoint_with(1), Ok(true));
    assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 0.0 });
    assert_eq!(
        doc.toggle_fixed_anchor(),
        Err("Remove the point's curve relation before fixing it".into())
    );
    assert_eq!(
        doc.set_selected_point_coordinate(true, 15.0),
        Err("Point is driven by Midpoint".into())
    );
    assert!(!doc.move_selected(Point { x: 5.0, y: 0.0 }));
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 0.0, y: 10.0 }));
    assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 10.0 });
    assert_eq!(doc.set_dimension(DimensionField::LineLength, 40.0), Ok(()));
    assert_eq!(doc.shapes[1].center(), Point { x: 20.0, y: 10.0 });
    assert!(doc.undo());
    assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 10.0 });
    doc.selected = Some(2);
    assert_eq!(doc.toggle_midpoint_with(1), Ok(false));
}

#[test]
fn midpoint_point_follows_axis_constraint() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 10.0 },
        },
        Shape::Point {
            id: 2,
            at: Point { x: 0.0, y: 0.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_midpoint_with(1), Ok(true));
    doc.selected = Some(1);
    assert_eq!(
        doc.toggle_axis_constraint(AxisConstraint::Horizontal),
        Ok(true)
    );
    assert!(
        doc.shapes[1]
            .center()
            .distance(Document::curve_midpoint(&doc.shapes[0]).unwrap())
            < 1e-4
    );
}

#[test]
fn midpoint_point_follows_circular_and_elliptic_arcs() {
    for curve in [
        Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 20.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        },
        Shape::EllipticArc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 30.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        },
    ] {
        let mut doc = Document::from_shapes(vec![
            curve,
            Shape::Point {
                id: 2,
                at: Point { x: 5.0, y: 5.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_midpoint_with(1), Ok(true));
        assert!(
            doc.shapes[1]
                .center()
                .distance(Document::curve_midpoint(&doc.shapes[0]).unwrap())
                < 1e-4
        );
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 10.0, y: 5.0 }));
        assert!(
            doc.shapes[1]
                .center()
                .distance(Document::curve_midpoint(&doc.shapes[0]).unwrap())
                < 1e-4
        );
    }
}

#[test]
fn point_on_curve_projects_to_finite_line_and_tracks_source() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Point {
            id: 2,
            at: Point { x: 30.0, y: 10.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_point_on_curve_with(1), Ok(true));
    assert_eq!(doc.shapes[1].center(), Point { x: 20.0, y: 0.0 });
    assert!(doc.move_selected(Point { x: -10.0, y: 7.0 }));
    assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 0.0 });
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 0.0, y: 5.0 }));
    assert_eq!(doc.shapes[1].center(), Point { x: 10.0, y: 5.0 });
    doc.selected = Some(2);
    assert_eq!(doc.toggle_point_on_curve_with(1), Ok(false));
}

#[test]
fn point_on_curve_projects_circle_arc_ellipse_and_spline() {
    let curves = [
        Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        },
        Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
        Shape::Ellipse {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 30.0,
            segments: 128,
        },
        Shape::EllipticArc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 30.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 128,
        },
        Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: vec![
                Point { x: 0.0, y: 0.0 },
                Point { x: 10.0, y: 20.0 },
                Point { x: 20.0, y: 20.0 },
                Point { x: 30.0, y: 0.0 },
            ],
            closed: false,
        },
    ];
    for curve in curves {
        let mut doc = Document::from_shapes(vec![
            curve,
            Shape::Point {
                id: 2,
                at: Point { x: 15.0, y: 15.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_point_on_curve_with(1), Ok(true));
        let point = doc.shapes[1].center();
        assert!(
            point.distance(Document::closest_curve_point(&doc.shapes[0], point).unwrap()) < 1e-3
        );
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 5.0, y: 3.0 }));
        let point = doc.shapes[1].center();
        assert!(
            point.distance(Document::closest_curve_point(&doc.shapes[0], point).unwrap()) < 1e-3
        );
    }
}

#[test]
fn normal_line_endpoint_follows_circle_and_ellipse() {
    for curve in [
        Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        },
        Shape::Ellipse {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 20.0,
            ry: 10.0,
            rotation: 30.0,
            segments: 128,
        },
    ] {
        let mut doc = Document::from_shapes(vec![
            curve,
            Shape::Line {
                id: 2,
                a: Point { x: 35.0, y: 5.0 },
                b: Point { x: 15.0, y: 7.0 },
            },
        ]);
        doc.selected = Some(2);
        assert_eq!(
            doc.toggle_normal_with(1, Point { x: 15.0, y: 7.0 }),
            Ok(true)
        );
        assert_eq!(doc.normal_links.get(&2), Some(&(1, 1)));
        assert!(
            doc.toggle_axis_constraint(AxisConstraint::Horizontal)
                .is_err()
        );
        assert!(
            doc.toggle_fixed_line_endpoint_at(doc.shapes[1].center())
                .is_err()
        );
        let Shape::Line { b, .. } = doc.shapes[1] else {
            unreachable!()
        };
        assert!(b.distance(Document::closest_curve_point(&doc.shapes[0], b).unwrap()) < 1e-3);
        doc.selected = Some(1);
        assert!(doc.move_selected(Point { x: 5.0, y: 4.0 }));
        let Shape::Line { b, .. } = doc.shapes[1] else {
            unreachable!()
        };
        assert!(b.distance(Document::closest_curve_point(&doc.shapes[0], b).unwrap()) < 1e-3);
        doc.selected = Some(2);
        assert_eq!(doc.toggle_normal_with(1, b), Ok(false));
    }
}

#[test]
fn splitting_line_preserves_endpoint_links_and_axis_but_removes_length() {
    let mut doc = Document::from_shapes(vec![
        Shape::Point {
            id: 1,
            at: Point { x: 20.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
    ]);
    doc.selected = Some(2);
    doc.point_links.insert((2, 1), 1);
    doc.axis_constraints.insert(2, AxisConstraint::Horizontal);
    doc.driving_dimensions
        .insert((2, DimensionField::LineLength), 20.0);
    assert!(doc.split_selected_line(Point { x: 10.0, y: 9.0 }).is_err());
    assert_eq!(doc.shapes.len(), 2);
    let new_id = doc.split_selected_line(Point { x: 10.0, y: 2.0 }).unwrap();
    assert_eq!(new_id, 3);
    assert_eq!(
        doc.shapes[1],
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 0.0 }
        }
    );
    assert_eq!(
        doc.shapes[2],
        Shape::Line {
            id: 3,
            a: Point { x: 10.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 }
        }
    );
    assert_eq!(doc.point_links.get(&(3, 1)), Some(&1));
    assert!(!doc.point_links.contains_key(&(2, 1)));
    assert_eq!(
        doc.axis_constraints.get(&3),
        Some(&AxisConstraint::Horizontal)
    );
    assert!(
        !doc.driving_dimensions
            .contains_key(&(2, DimensionField::LineLength))
    );
    assert!(doc.undo());
    assert_eq!(doc.shapes.len(), 2);
    assert_eq!(doc.point_links.get(&(2, 1)), Some(&1));
    assert!(doc.redo());
    assert_eq!(doc.shapes.len(), 3);
}

#[test]
fn splitting_ellipse_and_elliptic_arc_preserves_exact_parameters() {
    let center = Point { x: 5.0, y: -3.0 };
    let mut doc = Document::from_shapes(vec![Shape::Ellipse {
        id: 1,
        center,
        rx: 20.0,
        ry: 10.0,
        rotation: 30.0,
        segments: 64,
    }]);
    doc.selected = Some(1);
    let first = Shape::elliptic_arc_point(center, 20.0, 10.0, 30.0, 0.0, 0.0, 0.0);
    let second = Shape::elliptic_arc_point(
        center,
        20.0,
        10.0,
        30.0,
        std::f64::consts::FRAC_PI_2,
        0.0,
        0.0,
    );
    assert_eq!(
        doc.split_selected_ellipse(first, first),
        Err("Choose two distinct split points".into())
    );
    assert_eq!(doc.split_selected_ellipse(first, second), Ok(2));
    assert_eq!(doc.shared_ellipses.get(&2), Some(&1));
    assert!(
        matches!(doc.shapes[0], Shape::EllipticArc { rx: 20.0, ry: 10.0, rotation: 30.0, sweep, .. } if (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-5)
    );
    assert!(
        matches!(doc.shapes[1], Shape::EllipticArc { sweep, .. } if (sweep - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-5)
    );
    let middle =
        Shape::elliptic_arc_point(center, 20.0, 10.0, 30.0, std::f64::consts::PI, 0.0, 0.0);
    assert_eq!(doc.split_selected_curve(middle), Ok(3));
    assert_eq!(doc.shared_ellipses.get(&3), Some(&1));
    assert_eq!(doc.shapes.len(), 3);
    doc.selected = Some(2);
    assert!(doc.move_selected(Point { x: 7.0, y: 2.0 }));
    assert!(
        doc.shapes
            .iter()
            .all(|shape| shape.center() == Point { x: 12.0, y: -1.0 })
    );
    assert_eq!(
        doc.set_dimension(DimensionField::RectangleWidth, 50.0),
        Ok(())
    );
    assert!(
        doc.shapes.iter().all(
            |shape| matches!(shape, Shape::EllipticArc { rx, .. } if (*rx - 25.0).abs() < 1e-4)
        )
    );
    assert_eq!(doc.set_dimension(DimensionField::Rotation, 45.0), Ok(()));
    assert!(doc.shapes.iter().all(|shape| matches!(shape, Shape::EllipticArc { rotation, .. } if (*rotation - 45.0).abs() < 1e-4)));
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(doc.undo());
    assert_eq!(doc.shapes.len(), 2);
    assert!(doc.undo());
    assert!(matches!(doc.shapes[0], Shape::Ellipse { .. }));
}

#[test]
fn elliptic_arc_bounds_and_intersections_follow_retained_sweep() {
    let arc = Shape::EllipticArc {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        rx: 20.0,
        ry: 10.0,
        rotation: 0.0,
        start_angle: 0.0,
        sweep: std::f64::consts::PI,
        segments: 64,
    };
    let line = Shape::Line {
        id: 2,
        a: Point { x: 0.0, y: -20.0 },
        b: Point { x: 0.0, y: 20.0 },
    };
    let contacts = finite_intersections(&arc, &line);
    assert_eq!(contacts.len(), 1);
    assert!(contacts[0].distance(Point { x: 0.0, y: 10.0 }) < 1e-5);
    assert!(arc.hit(Point { x: 0.0, y: 10.0 }, 0.1));
    assert!(!arc.hit(Point { x: 0.0, y: -10.0 }, 0.1));
    let (min, max) = arc.bounds().unwrap();
    assert!((min.x + 20.0).abs() < 1e-5 && (max.x - 20.0).abs() < 1e-5);
    assert!(min.y.abs() < 1e-5 && (max.y - 10.0).abs() < 1e-5);
}

#[test]
fn spline_authoring_validates_modes_and_is_undoable() {
    let mut document = Document::default();
    let points = vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 20.0, y: 0.0 },
        Point { x: 20.0, y: 20.0 },
        Point { x: 0.0, y: 20.0 },
    ];
    assert!(
        document
            .add_spline(SplineMode::Control, points[..3].to_vec(), false)
            .is_err()
    );
    assert!(document.shapes.is_empty());
    assert_eq!(
        document.add_spline(SplineMode::Fit, points.clone(), true),
        Ok(1)
    );
    assert!(matches!(
        document.shapes[0],
        Shape::Spline {
            mode: SplineMode::Fit,
            closed: true,
            ..
        }
    ));
    assert_eq!(
        document.add_spline(SplineMode::Control, points.clone(), false),
        Ok(2)
    );
    assert!(document.undo());
    assert_eq!(document.shapes.len(), 1);
    assert!(document.redo());
    assert_eq!(document.shapes.len(), 2);
}

#[test]
fn selected_spline_handle_drag_moves_only_that_point_and_undoes() {
    let original = vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 20.0 },
        Point { x: 20.0, y: -20.0 },
        Point { x: 30.0, y: 0.0 },
    ];
    let mut document = Document::from_shapes(vec![Shape::Spline {
        id: 1,
        mode: SplineMode::Control,
        points: original.clone(),
        closed: false,
    }]);
    document.selected = Some(1);
    document.select_at(original[1]);
    assert_eq!(document.selected_spline_point, Some(1));
    assert!(document.begin_drag(original[1]));
    assert!(document.drag_to(Point { x: 15.0, y: 25.0 }));
    assert!(document.finish_drag());
    assert!(matches!(&document.shapes[0], Shape::Spline { points, .. }
            if points[0] == original[0] && points[1] == Point { x: 15.0, y: 25.0 } && points[2] == original[2]));
    assert!(document.undo());
    assert!(matches!(&document.shapes[0], Shape::Spline { points, .. } if *points == original));

    let fit = vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 0.0 },
        Point { x: 10.0, y: 10.0 },
    ];
    let mut periodic = Document::from_shapes(vec![Shape::Spline {
        id: 1,
        mode: SplineMode::Fit,
        points: fit.clone(),
        closed: true,
    }]);
    periodic.selected = Some(1);
    periodic.select_at(fit[1]);
    assert!(periodic.begin_drag(fit[1]));
    assert!(!periodic.drag_to(fit[0]));
    assert!(periodic.drag_to(Point { x: 15.0, y: 0.0 }));
    assert!(periodic.finish_drag());
    assert!(
        matches!(&periodic.shapes[0], Shape::Spline { closed: true, points, .. } if points[1].x == 15.0)
    );
}

#[test]
fn selected_spline_point_coordinates_edit_independently_and_undo() {
    let original = vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 20.0 },
        Point { x: 20.0, y: -20.0 },
        Point { x: 30.0, y: 0.0 },
    ];
    let mut document = Document::from_shapes(vec![Shape::Spline {
        id: 1,
        mode: SplineMode::Control,
        points: original.clone(),
        closed: false,
    }]);
    document.selected = Some(1);
    assert!(document.set_selected_spline_coordinate(true, 15.0).is_err());
    document.selected_spline_point = Some(1);
    assert!(
        document
            .set_selected_spline_coordinate(true, f32::NAN)
            .is_err()
    );
    assert_eq!(document.set_selected_spline_coordinate(true, 15.0), Ok(()));
    assert_eq!(document.set_selected_spline_coordinate(false, 25.0), Ok(()));
    assert!(matches!(&document.shapes[0], Shape::Spline { points, .. }
            if points[1] == Point { x: 15.0, y: 25.0 } && points[0] == original[0] && points[2] == original[2]));
    assert!(document.undo());
    assert!(matches!(&document.shapes[0], Shape::Spline { points, .. }
            if points[1] == Point { x: 15.0, y: 20.0 }));
    assert!(document.undo());
    assert!(matches!(&document.shapes[0], Shape::Spline { points, .. } if *points == original));
    assert!(document.redo());
    assert!(document.redo());
    assert!(matches!(&document.shapes[0], Shape::Spline { points, .. }
            if points[1] == Point { x: 15.0, y: 25.0 }));
}

#[test]
fn splitting_open_spline_preserves_curve_construction_and_history() {
    let points = vec![
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 30.0 },
        Point { x: 20.0, y: -30.0 },
        Point { x: 30.0, y: 0.0 },
    ];
    let curve = spline::beziers(SplineMode::Control, &points, false).unwrap()[0];
    let mut document = Document::from_shapes(vec![Shape::Spline {
        id: 1,
        mode: SplineMode::Control,
        points: points.clone(),
        closed: false,
    }]);
    document.selected = Some(1);
    document.construction.insert(1);
    assert!(
        document
            .split_selected_curve(Point { x: 100.0, y: 100.0 })
            .is_err()
    );
    assert_eq!(document.shapes.len(), 1);
    let middle = spline::point(&curve, 0.5);
    assert_eq!(document.split_selected_curve(middle), Ok(2));
    assert_eq!(document.selected, Some(2));
    assert!(document.construction.contains(&2));
    let Shape::Spline { points: first, .. } = &document.shapes[0] else {
        panic!()
    };
    let Shape::Spline { points: second, .. } = &document.shapes[1] else {
        panic!()
    };
    assert!(first.last().unwrap().distance(second[0]) < 1e-4);
    let pieces = [
        spline::beziers(SplineMode::Control, first, false).unwrap()[0],
        spline::beziers(SplineMode::Control, second, false).unwrap()[0],
    ];
    for step in 0..=20 {
        let t = step as f32 / 20.0;
        let piece = if t <= 0.5 {
            spline::point(&pieces[0], t * 2.0)
        } else {
            spline::point(&pieces[1], (t - 0.5) * 2.0)
        };
        assert!(piece.distance(spline::point(&curve, t)) < 1e-3);
    }
    assert!(document.undo());
    assert!(
        matches!(&document.shapes[0], Shape::Spline { points: restored, .. } if *restored == points)
    );
    assert!(document.redo());
    assert_eq!(document.shapes.len(), 2);
}

#[test]
fn splitting_periodic_fit_spline_opens_one_control_chain() {
    let points = vec![
        Point { x: -20.0, y: -20.0 },
        Point { x: 20.0, y: -20.0 },
        Point { x: 20.0, y: 20.0 },
        Point { x: -20.0, y: 20.0 },
    ];
    let original = spline::beziers(SplineMode::Fit, &points, true).unwrap();
    let mut document = Document::from_shapes(vec![Shape::Spline {
        id: 1,
        mode: SplineMode::Fit,
        points: points.clone(),
        closed: true,
    }]);
    document.selected = Some(1);
    document.construction.insert(1);
    assert_eq!(
        document.split_selected_curve(spline::point(&original[1], 0.5)),
        Ok(1)
    );
    let Shape::Spline {
        mode,
        points: opened,
        closed,
        ..
    } = &document.shapes[0]
    else {
        panic!()
    };
    assert_eq!(*mode, SplineMode::Control);
    assert!(!closed);
    assert!(opened[0].distance(*opened.last().unwrap()) < 1e-4);
    assert!(document.construction.contains(&1));
    assert_eq!(spline::beziers(*mode, opened, false).unwrap().len(), 5);
    assert!(document.undo());
    assert!(
        matches!(&document.shapes[0], Shape::Spline { mode: SplineMode::Fit, points: restored, closed: true, .. } if *restored == points)
    );
}

#[test]
fn trimming_spline_between_lines_keeps_exact_outer_cubic_pieces() {
    let points = vec![
        Point { x: -20.0, y: 0.0 },
        Point { x: -20.0, y: 20.0 },
        Point { x: 20.0, y: 20.0 },
        Point { x: 20.0, y: 0.0 },
    ];
    let original = spline::beziers(SplineMode::Control, &points, false).unwrap()[0];
    let mut document = Document::from_shapes(vec![
        Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: points.clone(),
            closed: false,
        },
        Shape::Line {
            id: 2,
            a: Point { x: -10.0, y: -5.0 },
            b: Point { x: -10.0, y: 25.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: 10.0, y: -5.0 },
            b: Point { x: 10.0, y: 25.0 },
        },
    ]);
    document.selected = Some(1);
    document.construction.insert(1);
    assert_eq!(
        document.trim_selected_curve(spline::point(&original, 0.5)),
        Ok(2)
    );
    assert_eq!(document.shapes.len(), 4);
    assert!(document.construction.contains(&4));
    let Shape::Spline { points: first, .. } = &document.shapes[0] else {
        panic!()
    };
    let Shape::Spline { points: second, .. } = &document.shapes[1] else {
        panic!()
    };
    let first_curve = spline::beziers(SplineMode::Control, first, false).unwrap()[0];
    let second_curve = spline::beziers(SplineMode::Control, second, false).unwrap()[0];
    assert_eq!(first[0], points[0]);
    assert_eq!(*second.last().unwrap(), points[3]);
    assert!(first.last().unwrap().x < -9.9);
    assert!(second[0].x > 9.9);
    let left_cut = spline::line_intersections(
        &[original],
        Point { x: -10.0, y: -5.0 },
        Point { x: -10.0, y: 25.0 },
    )[0]
    .1;
    let right_cut = spline::line_intersections(
        &[original],
        Point { x: 10.0, y: -5.0 },
        Point { x: 10.0, y: 25.0 },
    )[0]
    .1;
    assert!(
        spline::point(&first_curve, 0.5).distance(spline::point(&original, left_cut * 0.5)) < 1e-4
    );
    assert!(
        spline::point(&second_curve, 0.5)
            .distance(spline::point(&original, (right_cut + 1.0) * 0.5))
            < 1e-4
    );
    assert!(document.undo());
    assert_eq!(document.shapes.len(), 3);
    assert!(
        matches!(&document.shapes[0], Shape::Spline { points: restored, .. } if *restored == points)
    );
    assert!(document.redo());
    assert_eq!(document.shapes.len(), 4);
}

#[test]
fn trimming_closed_fit_spline_opens_retained_wraparound() {
    let points = vec![
        Point { x: -20.0, y: -20.0 },
        Point { x: 20.0, y: -20.0 },
        Point { x: 20.0, y: 20.0 },
        Point { x: -20.0, y: 20.0 },
    ];
    let original = spline::beziers(SplineMode::Fit, &points, true).unwrap();
    let mut document = Document::from_shapes(vec![
        Shape::Spline {
            id: 1,
            mode: SplineMode::Fit,
            points: points.clone(),
            closed: true,
        },
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: -30.0 },
            b: Point { x: 0.0, y: 30.0 },
        },
    ]);
    document.selected = Some(1);
    assert_eq!(
        document.trim_selected_curve(spline::point(&original[1], 0.5)),
        Ok(1)
    );
    let Shape::Spline {
        mode,
        points: retained,
        closed,
        ..
    } = &document.shapes[0]
    else {
        panic!()
    };
    assert_eq!(*mode, SplineMode::Control);
    assert!(!closed);
    assert!(
        retained
            .first()
            .unwrap()
            .distance(*retained.last().unwrap())
            > 1.0
    );
    assert!(document.undo());
    assert!(
        matches!(&document.shapes[0], Shape::Spline { mode: SplineMode::Fit, points: restored, closed: true, .. } if *restored == points)
    );
}

#[test]
fn circle_boundaries_trim_spline_and_arc_sweep_filters_contacts() {
    let points = vec![
        Point { x: -20.0, y: 0.0 },
        Point { x: -20.0, y: 20.0 },
        Point { x: 20.0, y: 20.0 },
        Point { x: 20.0, y: 0.0 },
    ];
    let spline = Shape::Spline {
        id: 1,
        mode: SplineMode::Control,
        points,
        closed: false,
    };
    let circle = Shape::Circle {
        id: 2,
        center: Point { x: 0.0, y: 0.0 },
        radius: 17.0,
        segments: 64,
    };
    let hits = finite_intersections(&spline, &circle);
    assert_eq!(hits.len(), 2);
    assert_eq!(finite_intersections(&circle, &spline).len(), 2);
    let upper_arc = Shape::Arc {
        id: 2,
        center: Point { x: 0.0, y: 0.0 },
        radius: 17.0,
        start_angle: 0.0,
        sweep: std::f64::consts::PI,
        segments: 64,
    };
    assert_eq!(finite_intersections(&spline, &upper_arc).len(), 2);
    let lower_arc = Shape::Arc {
        id: 2,
        center: Point { x: 0.0, y: 0.0 },
        radius: 17.0,
        start_angle: std::f64::consts::PI,
        sweep: std::f64::consts::PI,
        segments: 64,
    };
    assert!(finite_intersections(&spline, &lower_arc).is_empty());
    let mut document = Document::from_shapes(vec![spline, circle]);
    document.selected = Some(1);
    assert_eq!(
        document.trim_selected_curve(Point { x: 0.0, y: 15.0 }),
        Ok(2)
    );
    assert_eq!(document.shapes.len(), 3);
    assert!(document.undo());
    assert_eq!(document.shapes.len(), 2);
}

#[test]
fn rotated_ellipse_and_elliptic_arc_intersect_and_trim_spline() {
    let spline = Shape::Spline {
        id: 1,
        mode: SplineMode::Control,
        points: vec![
            Point { x: -20.0, y: 0.0 },
            Point { x: -10.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
        ],
        closed: false,
    };
    let ellipse = Shape::Ellipse {
        id: 2,
        center: Point { x: 0.0, y: 0.0 },
        rx: 10.0,
        ry: 5.0,
        rotation: 90.0,
        segments: 64,
    };
    assert_eq!(finite_intersections(&spline, &ellipse).len(), 2);
    assert_eq!(finite_intersections(&ellipse, &spline).len(), 2);
    let arc = Shape::EllipticArc {
        id: 2,
        center: Point { x: 0.0, y: 0.0 },
        rx: 10.0,
        ry: 5.0,
        rotation: 90.0,
        start_angle: -std::f64::consts::PI,
        sweep: std::f64::consts::PI,
        segments: 64,
    };
    assert_eq!(finite_intersections(&spline, &arc).len(), 1);
    assert_eq!(finite_intersections(&arc, &spline).len(), 1);
    let mut document = Document::from_shapes(vec![spline, ellipse]);
    document.selected = Some(1);
    assert_eq!(
        document.trim_selected_curve(Point { x: 0.0, y: 0.0 }),
        Ok(2)
    );
    assert_eq!(document.shapes.len(), 3);
    assert!(document.undo());
    assert_eq!(document.shapes.len(), 2);
}

#[test]
fn another_spline_can_bound_trim_and_intersection_snap() {
    let arch = Shape::Spline {
        id: 1,
        mode: SplineMode::Control,
        points: vec![
            Point { x: -20.0, y: 0.0 },
            Point { x: -20.0, y: 20.0 },
            Point { x: 20.0, y: 20.0 },
            Point { x: 20.0, y: 0.0 },
        ],
        closed: false,
    };
    let horizontal = Shape::Spline {
        id: 2,
        mode: SplineMode::Control,
        points: vec![
            Point { x: -30.0, y: 10.0 },
            Point { x: -10.0, y: 10.0 },
            Point { x: 10.0, y: 10.0 },
            Point { x: 30.0, y: 10.0 },
        ],
        closed: false,
    };
    let hits = finite_intersections(&arch, &horizontal);
    assert_eq!(hits.len(), 2);
    assert_eq!(finite_intersections(&horizontal, &arch).len(), 2);
    let mut document = Document::from_shapes(vec![arch, horizontal]);
    document.selected = Some(1);
    assert_eq!(
        document.trim_selected_curve(Point { x: 0.0, y: 15.0 }),
        Ok(2)
    );
    assert_eq!(document.shapes.len(), 3);
    assert!(document.undo());
    assert_eq!(document.shapes.len(), 2);
}

#[test]
fn spline_trim_work_limit_leaves_selected_geometry_untouched() {
    let make_points = |offset: f32| {
        let mut points = vec![Point { x: 0.0, y: offset }];
        for _ in 0..65 {
            points.extend([
                Point {
                    x: 10.0,
                    y: 20.0 + offset,
                },
                Point {
                    x: -10.0,
                    y: 20.0 + offset,
                },
                Point { x: 0.0, y: offset },
            ]);
        }
        points
    };
    let mut document = Document::from_shapes(vec![
        Shape::Spline {
            id: 1,
            mode: SplineMode::Control,
            points: make_points(0.0),
            closed: false,
        },
        Shape::Spline {
            id: 2,
            mode: SplineMode::Control,
            points: make_points(1.0),
            closed: false,
        },
    ]);
    let original = document.shapes.clone();
    document.selected = Some(1);
    assert!(
        document
            .trim_selected_curve(Point { x: 0.0, y: 15.0 })
            .is_err()
    );
    assert_eq!(document.shapes, original);
}

#[test]
fn trim_ellipse_and_elliptic_arc_keep_exact_curve_fragments() {
    let center = Point { x: 0.0, y: 0.0 };
    let lines = [5.0, -5.0].map(|x| Shape::Line {
        id: if x > 0.0 { 2 } else { 3 },
        a: Point { x, y: -10.0 },
        b: Point { x, y: 10.0 },
    });
    let mut ellipse = Document::from_shapes(vec![
        Shape::Ellipse {
            id: 1,
            center,
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 64,
        },
        lines[0].clone(),
        lines[1].clone(),
    ]);
    ellipse.selected = Some(1);
    ellipse.construction.insert(1);
    assert_eq!(ellipse.trim_selected_curve(Point { x: 0.0, y: 5.0 }), Ok(1));
    assert!(
        matches!(ellipse.shapes[0], Shape::EllipticArc { rx: 10.0, ry: 5.0, start_angle, sweep, .. }
            if (start_angle - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6
                && (sweep - 5.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
    );
    assert!(ellipse.construction.contains(&1));
    assert!(ellipse.undo());
    assert!(matches!(ellipse.shapes[0], Shape::Ellipse { .. }));

    let mut arc = Document::from_shapes(vec![
        Shape::EllipticArc {
            id: 1,
            center,
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            start_angle: 0.0,
            sweep: std::f64::consts::PI,
            segments: 64,
        },
        lines[0].clone(),
        lines[1].clone(),
    ]);
    arc.selected = Some(1);
    assert_eq!(arc.trim_selected_curve(Point { x: 0.0, y: 5.0 }), Ok(2));
    assert!(
        matches!(arc.shapes[0], Shape::EllipticArc { sweep, .. } if (sweep - std::f64::consts::PI / 3.0).abs() < 1e-6)
    );
    assert!(
        matches!(arc.shapes[1], Shape::EllipticArc { start_angle, sweep, .. }
            if (start_angle - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6
                && (sweep - std::f64::consts::PI / 3.0).abs() < 1e-6)
    );
    assert!(arc.undo());
    assert_eq!(arc.shapes.len(), 3);
}

#[test]
fn extend_elliptic_arc_reaches_next_boundary_in_both_directions() {
    let center = Point { x: 0.0, y: 0.0 };
    let boundary = Shape::Line {
        id: 2,
        a: Point { x: -5.0, y: -10.0 },
        b: Point { x: -5.0, y: 10.0 },
    };
    let mut doc = Document::from_shapes(vec![
        Shape::EllipticArc {
            id: 1,
            center,
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
        boundary.clone(),
    ]);
    doc.selected = Some(1);
    assert!(doc.extend_selected_curve(Point { x: 0.0, y: 5.0 }).is_ok());
    assert!(
        matches!(doc.shapes[0], Shape::EllipticArc { start_angle, sweep, .. }
            if start_angle.abs() < 1e-8 && (sweep - 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
    );
    assert!(doc.undo());
    assert!(matches!(doc.shapes[0], Shape::EllipticArc { sweep, .. }
            if (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-8));

    let mut clockwise = Document::from_shapes(vec![
        Shape::EllipticArc {
            id: 1,
            center,
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            start_angle: 0.0,
            sweep: -std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
        boundary,
    ]);
    clockwise.selected = Some(1);
    assert!(
        clockwise
            .extend_selected_curve(Point { x: 0.0, y: -5.0 })
            .is_ok()
    );
    assert!(
        matches!(clockwise.shapes[0], Shape::EllipticArc { sweep, .. }
            if (sweep + 2.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
    );
}

#[test]
fn splitting_arc_keeps_center_radius_sweep_and_construction() {
    let center = Point { x: 0.0, y: 0.0 };
    let mut doc = Document::from_shapes(vec![Shape::Arc {
        id: 1,
        center,
        radius: 10.0,
        start_angle: 0.0,
        sweep: std::f64::consts::PI,
        segments: 64,
    }]);
    doc.selected = Some(1);
    doc.construction.insert(1);
    assert_eq!(doc.set_dimension(DimensionField::ArcSweep, 180.0), Ok(()));
    assert!(
        doc.split_selected_curve(Point { x: 0.0, y: -10.0 })
            .is_err()
    );
    assert_eq!(doc.split_selected_curve(Point { x: 0.0, y: 10.0 }), Ok(2));
    assert!(
        !doc.driving_dimensions
            .contains_key(&(1, DimensionField::ArcSweep))
    );
    assert!(
        matches!(doc.shapes[0], Shape::Arc { center: c, radius: 10.0, sweep, .. } if c == center && (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9)
    );
    assert!(
        matches!(doc.shapes[1], Shape::Arc { start_angle, sweep, .. } if (start_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9 && (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9)
    );
    assert!(doc.construction.contains(&2));
    assert!(doc.undo());
    assert_eq!(doc.shapes.len(), 1);
    assert_eq!(
        doc.driving_dimensions.get(&(1, DimensionField::ArcSweep)),
        Some(&180.0)
    );
    assert!(doc.redo());
    assert_eq!(doc.shapes.len(), 2);
    assert!(
        !doc.driving_dimensions
            .contains_key(&(1, DimensionField::ArcSweep))
    );
    let mut clockwise = Document::from_shapes(vec![Shape::Arc {
        id: 1,
        center,
        radius: 10.0,
        start_angle: 0.0,
        sweep: -std::f64::consts::PI,
        segments: 64,
    }]);
    clockwise.selected = Some(1);
    assert_eq!(
        clockwise.split_selected_arc(Point { x: 0.0, y: -10.0 }),
        Ok(2)
    );
    assert!(
        matches!(clockwise.shapes[1], Shape::Arc { start_angle, sweep, .. } if (start_angle + std::f64::consts::FRAC_PI_2).abs() < 1e-9 && (sweep + std::f64::consts::FRAC_PI_2).abs() < 1e-9)
    );
}

#[test]
fn splitting_circle_at_two_points_creates_complementary_arcs() {
    let mut doc = Document::from_shapes(vec![Shape::Circle {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        radius: 10.0,
        segments: 64,
    }]);
    doc.selected = Some(1);
    doc.construction.insert(1);
    doc.driving_dimensions
        .insert((1, DimensionField::CircleDiameter), 20.0);
    assert!(
        doc.split_selected_circle(Point { x: 10.0, y: 0.0 }, Point { x: 10.0, y: 0.0 })
            .is_err()
    );
    assert_eq!(
        doc.split_selected_circle(Point { x: 0.0, y: 10.0 }, Point { x: 10.0, y: 0.0 }),
        Ok(2)
    );
    assert!(
        matches!(doc.shapes[0], Shape::Arc { start_angle, sweep, .. } if start_angle.abs() < 1e-9 && (sweep - std::f64::consts::FRAC_PI_2).abs() < 1e-9)
    );
    assert!(
        matches!(doc.shapes[1], Shape::Arc { start_angle, sweep, .. } if (start_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-9 && (sweep - 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-9)
    );
    assert!(doc.construction.contains(&2));
    assert!(
        !doc.driving_dimensions
            .contains_key(&(1, DimensionField::CircleDiameter))
    );
    assert!(doc.undo());
    assert!(matches!(doc.shapes[0], Shape::Circle { .. }));
    assert!(doc.redo());
    assert_eq!(doc.shapes.len(), 2);
}

#[test]
fn trim_line_removes_clicked_span_and_retains_endpoint_relations() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 40.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 10.0, y: -10.0 },
            b: Point { x: 10.0, y: 10.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: 30.0, y: -10.0 },
            b: Point { x: 30.0, y: 10.0 },
        },
        Shape::Point {
            id: 4,
            at: Point { x: 40.0, y: 0.0 },
        },
    ]);
    doc.selected = Some(1);
    doc.axis_constraints.insert(1, AxisConstraint::Horizontal);
    doc.construction.insert(1);
    doc.point_links.insert((1, 1), 4);
    doc.driving_dimensions
        .insert((1, DimensionField::LineLength), 40.0);
    assert_eq!(doc.trim_selected_line(Point { x: 20.0, y: 1.0 }), Ok(2));
    assert_eq!(
        doc.shapes[0],
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 0.0 }
        }
    );
    assert_eq!(
        doc.shapes[1],
        Shape::Line {
            id: 5,
            a: Point { x: 30.0, y: 0.0 },
            b: Point { x: 40.0, y: 0.0 }
        }
    );
    assert_eq!(doc.point_links.get(&(5, 1)), Some(&4));
    assert!(doc.construction.contains(&5));
    assert_eq!(
        doc.axis_constraints.get(&5),
        Some(&AxisConstraint::Horizontal)
    );
    assert!(
        !doc.driving_dimensions
            .contains_key(&(1, DimensionField::LineLength))
    );
    assert!(doc.undo());
    assert_eq!(doc.shapes.len(), 4);
    assert_eq!(doc.point_links.get(&(1, 1)), Some(&4));
    assert!(doc.redo());
    assert_eq!(doc.shapes.len(), 5);
}

#[test]
fn trim_arc_removes_section_between_crossings_and_preserves_sweep_direction() {
    let center = Point { x: 0.0, y: 0.0 };
    let arc = Shape::Arc {
        id: 1,
        center,
        radius: 10.0,
        start_angle: 0.0,
        sweep: std::f64::consts::PI,
        segments: 64,
    };
    let boundaries = [
        Shape::Line {
            id: 2,
            a: Point { x: 5.0, y: 0.0 },
            b: Point { x: 5.0, y: 20.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: -5.0, y: 0.0 },
            b: Point { x: -5.0, y: 20.0 },
        },
    ];
    let mut doc = Document::from_shapes(vec![arc, boundaries[0].clone(), boundaries[1].clone()]);
    doc.selected = Some(1);
    doc.construction.insert(1);
    assert_eq!(doc.trim_selected_curve(Point { x: 0.0, y: 10.0 }), Ok(2));
    assert!(
        matches!(doc.shapes[0], Shape::Arc { start_angle, sweep, .. } if start_angle.abs() < 1e-9 && (sweep - std::f64::consts::FRAC_PI_3).abs() < 1e-6)
    );
    assert!(
        matches!(doc.shapes[1], Shape::Arc { start_angle, sweep, .. } if (start_angle - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6 && (sweep - std::f64::consts::FRAC_PI_3).abs() < 1e-6)
    );
    assert!(doc.construction.contains(&4));
    assert!(doc.undo());
    assert_eq!(doc.shapes.len(), 3);
    assert!(doc.redo());
    assert_eq!(doc.shapes.len(), 4);
    let mut clockwise = Document::from_shapes(vec![
        Shape::Arc {
            id: 1,
            center,
            radius: 10.0,
            start_angle: 0.0,
            sweep: -std::f64::consts::PI,
            segments: 64,
        },
        Shape::Line {
            id: 2,
            a: Point { x: 5.0, y: -20.0 },
            b: Point { x: 5.0, y: 0.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: -5.0, y: -20.0 },
            b: Point { x: -5.0, y: 0.0 },
        },
    ]);
    clockwise.selected = Some(1);
    assert_eq!(
        clockwise.trim_selected_arc(Point { x: 0.0, y: -10.0 }),
        Ok(2)
    );
    assert!(
        matches!(clockwise.shapes[0], Shape::Arc { sweep, .. } if (sweep + std::f64::consts::FRAC_PI_3).abs() < 1e-6)
    );
}

#[test]
fn trim_circle_keeps_complementary_arc_and_drops_diameter_dimension() {
    let mut doc = Document::from_shapes(vec![
        Shape::Circle {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            segments: 64,
        },
        Shape::Line {
            id: 2,
            a: Point { x: 5.0, y: -20.0 },
            b: Point { x: 5.0, y: 20.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: -5.0, y: -20.0 },
            b: Point { x: -5.0, y: 20.0 },
        },
    ]);
    doc.selected = Some(1);
    doc.construction.insert(1);
    doc.driving_dimensions
        .insert((1, DimensionField::CircleDiameter), 20.0);
    assert_eq!(doc.trim_selected_curve(Point { x: 0.0, y: 10.0 }), Ok(1));
    assert!(
        matches!(doc.shapes[0], Shape::Arc { radius: 10.0, start_angle, sweep, .. } if (start_angle - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6 && (sweep - 5.0 * std::f64::consts::PI / 3.0).abs() < 1e-6)
    );
    assert!(doc.construction.contains(&1));
    assert!(
        !doc.driving_dimensions
            .contains_key(&(1, DimensionField::CircleDiameter))
    );
    assert!(doc.undo());
    assert!(matches!(doc.shapes[0], Shape::Circle { .. }));
    assert!(doc.redo());
    assert!(matches!(doc.shapes[0], Shape::Arc { .. }));
    let mut isolated = Document::from_shapes(vec![Shape::Circle {
        id: 1,
        center: Point { x: 0.0, y: 0.0 },
        radius: 10.0,
        segments: 64,
    }]);
    isolated.selected = Some(1);
    assert_eq!(
        isolated.trim_selected_circle(Point { x: 0.0, y: 10.0 }),
        Ok(0)
    );
    assert!(isolated.shapes.is_empty());
}

#[test]
fn trim_line_with_no_crossings_removes_it_and_one_crossing_keeps_one_side() {
    let mut doc = Document::from_shapes(vec![Shape::Line {
        id: 1,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 20.0, y: 0.0 },
    }]);
    doc.selected = Some(1);
    assert_eq!(doc.trim_selected_line(Point { x: 10.0, y: 0.0 }), Ok(0));
    assert!(doc.shapes.is_empty());
    assert!(doc.undo());
    doc.shapes.push(Shape::Line {
        id: 2,
        a: Point { x: 8.0, y: -5.0 },
        b: Point { x: 8.0, y: 5.0 },
    });
    doc.selected = Some(1);
    assert_eq!(doc.trim_selected_line(Point { x: 3.0, y: 0.0 }), Ok(1));
    assert_eq!(
        doc.shapes[0],
        Shape::Line {
            id: 1,
            a: Point { x: 8.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 }
        }
    );
}

#[test]
fn extend_line_finds_nearest_boundary_in_endpoint_direction() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 20.0, y: -5.0 },
            b: Point { x: 20.0, y: 5.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: 30.0, y: -5.0 },
            b: Point { x: 30.0, y: 5.0 },
        },
        Shape::Line {
            id: 4,
            a: Point { x: -15.0, y: -5.0 },
            b: Point { x: -15.0, y: 5.0 },
        },
        Shape::Point {
            id: 5,
            at: Point { x: 10.0, y: 0.0 },
        },
    ]);
    doc.selected = Some(1);
    doc.point_links.insert((1, 1), 5);
    doc.driving_dimensions
        .insert((1, DimensionField::LineLength), 10.0);
    doc.axis_constraints.insert(1, AxisConstraint::Horizontal);
    assert_eq!(
        doc.extend_selected_line(Point { x: 9.0, y: 0.0 }),
        Ok(Point { x: 20.0, y: 0.0 })
    );
    assert_eq!(
        doc.shapes[0],
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 }
        }
    );
    assert!(!doc.point_links.contains_key(&(1, 1)));
    assert!(
        !doc.driving_dimensions
            .contains_key(&(1, DimensionField::LineLength))
    );
    assert_eq!(
        doc.axis_constraints.get(&1),
        Some(&AxisConstraint::Horizontal)
    );
    assert!(doc.undo());
    assert_eq!(doc.point_links.get(&(1, 1)), Some(&5));
    doc.selected = Some(1);
    assert_eq!(
        doc.extend_selected_line(Point { x: 1.0, y: 0.0 }),
        Ok(Point { x: -15.0, y: 0.0 })
    );
    assert!(matches!(
        doc.shapes[0],
        Shape::Line {
            a: Point { x: -15.0, y: 0.0 },
            ..
        }
    ));
    doc.selected = Some(2);
    assert!(doc.extend_selected_line(Point { x: 20.0, y: 0.0 }).is_err());
}

#[test]
fn extend_arc_reaches_nearest_boundary_outside_its_sweep() {
    let center = Point { x: 0.0, y: 0.0 };
    let mut doc = Document::from_shapes(vec![
        Shape::Arc {
            id: 1,
            center,
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
        Shape::Line {
            id: 2,
            a: Point { x: -5.0, y: 0.0 },
            b: Point { x: -5.0, y: 20.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: -8.0, y: 0.0 },
            b: Point { x: -8.0, y: 20.0 },
        },
        Shape::Line {
            id: 4,
            a: Point { x: 5.0, y: -20.0 },
            b: Point { x: 5.0, y: 0.0 },
        },
    ]);
    doc.selected = Some(1);
    let target = doc
        .extend_selected_curve(Point { x: 0.0, y: 10.0 })
        .unwrap();
    assert!((target.x + 5.0).abs() < 1e-4 && (target.y - 75.0_f32.sqrt()).abs() < 1e-4);
    assert!(
        matches!(doc.shapes[0], Shape::Arc { sweep, .. } if (sweep - 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6)
    );
    assert!(doc.undo());
    doc.selected = Some(1);
    doc.extend_selected_arc(Point { x: 10.0, y: 0.0 }).unwrap();
    assert!(
        matches!(doc.shapes[0], Shape::Arc { start_angle, sweep, .. } if (start_angle + std::f64::consts::FRAC_PI_3).abs() < 1e-6 && (sweep - 5.0 * std::f64::consts::PI / 6.0).abs() < 1e-6)
    );
    let mut clockwise = Document::from_shapes(vec![
        Shape::Arc {
            id: 1,
            center,
            radius: 10.0,
            start_angle: 0.0,
            sweep: -std::f64::consts::FRAC_PI_2,
            segments: 64,
        },
        Shape::Line {
            id: 2,
            a: Point { x: -5.0, y: -20.0 },
            b: Point { x: -5.0, y: 0.0 },
        },
    ]);
    clockwise.selected = Some(1);
    clockwise
        .extend_selected_arc(Point { x: 0.0, y: -10.0 })
        .unwrap();
    assert!(
        matches!(clockwise.shapes[0], Shape::Arc { sweep, .. } if (sweep + 2.0 * std::f64::consts::FRAC_PI_3).abs() < 1e-6)
    );
    let mut no_boundary = Document::from_shapes(vec![Shape::Arc {
        id: 1,
        center,
        radius: 10.0,
        start_angle: 0.0,
        sweep: std::f64::consts::FRAC_PI_2,
        segments: 64,
    }]);
    no_boundary.selected = Some(1);
    assert!(
        no_boundary
            .extend_selected_arc(Point { x: 0.0, y: 10.0 })
            .is_err()
    );
}

#[test]
fn linked_line_end_edits_carry_the_point_or_are_rejected() {
    let mut doc = Document::default();
    let at = Point { x: 50.0, y: 20.0 };
    assert!(doc.add_point(at));
    doc.tool = Tool::Line;
    doc.draft_start = Some(Point { x: 10.0, y: 10.0 });
    assert!(doc.finish_draft(at));
    assert_eq!(doc.point_links.get(&(2, 1)), Some(&1));
    doc.selected = Some(2);
    // Length and axis edits now solve with the linked point following.
    assert_eq!(doc.set_dimension(DimensionField::LineLength, 60.0), Ok(()));
    let (Shape::Point { at: point, .. }, Shape::Line { a, b, .. }) =
        (&doc.shapes[0], &doc.shapes[1])
    else {
        panic!()
    };
    assert_eq!(*a, Point { x: 10.0, y: 10.0 });
    assert!((a.distance(*b) - 60.0).abs() < 1e-3);
    assert!(point.distance(*b) < 1e-4);
    assert!(doc.undo());
    doc.selected = Some(2);
    assert_eq!(
        doc.toggle_axis_constraint(AxisConstraint::Horizontal),
        Ok(true)
    );
    assert!(matches!(doc.shapes[0], Shape::Point { at, .. } if (at.y - 10.0).abs() < 1e-4));
    assert!(doc.undo());
    // A fixed linked point makes the same edits conflict, and they roll back.
    doc.fixed_anchors.insert((1, 0), at);
    doc.selected = Some(2);
    assert!(doc.set_dimension(DimensionField::LineLength, 60.0).is_err());
    assert!(
        doc.toggle_axis_constraint(AxisConstraint::Horizontal)
            .is_err()
    );
    assert!(doc.axis_constraints.is_empty());
    assert!(
        !doc.driving_dimensions
            .contains_key(&(2, DimensionField::LineLength))
    );
    assert!(!doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
    assert!(!doc.rotate_selected(20.0, Point { x: 0.0, y: 0.0 }, false));
    assert_eq!(doc.point_links.get(&(2, 1)), Some(&1));
    assert!(matches!(doc.shapes[1], Shape::Line { b, .. } if b == at));
}

#[test]
fn arc_bounds_include_swept_quadrant_extrema() {
    let arc = Shape::Arc {
        id: 1,
        center: Point { x: 10.0, y: 20.0 },
        radius: 10.0,
        start_angle: 0.0,
        sweep: -std::f64::consts::PI * 1.5,
        segments: 64,
    };
    let (min, max) = arc.bounds().unwrap();
    assert!((min.x - 0.0).abs() < 1e-4);
    assert!((min.y - 10.0).abs() < 1e-4);
    assert!((max.x - 20.0).abs() < 1e-4);
    assert!((max.y - 30.0).abs() < 1e-4);
}

#[test]
fn creation_selection_delete_and_undo() {
    let mut doc = Document {
        tool: Tool::Rectangle,
        ..Document::default()
    };
    doc.draft_start = Some(Point { x: 10.0, y: 20.0 });
    assert!(doc.finish_draft(Point { x: 80.0, y: 60.0 }));
    assert_eq!(doc.selected, Some(1));
    doc.select_at(Point { x: 30.0, y: 30.0 });
    assert_eq!(doc.selected, Some(1));
    assert!(doc.delete_selected());
    assert!(doc.shapes.is_empty());
    assert!(doc.undo());
    assert_eq!(doc.shapes.len(), 1);
    assert!(doc.undo());
    assert!(doc.shapes.is_empty());
}

#[test]
fn center_rectangle_uses_first_click_as_center_and_is_undoable() {
    let mut doc = Document {
        tool: Tool::CenterRectangle,
        ..Document::default()
    };
    let center = Point { x: 10.0, y: 20.0 };
    doc.draft_start = Some(center);
    assert!(!doc.finish_draft(Point { x: 10.0, y: 20.0 }));
    doc.draft_start = Some(center);
    assert!(doc.finish_draft(Point { x: 40.0, y: 35.0 }));
    assert_eq!(doc.shapes[0].center(), center);
    assert_eq!(
        doc.shapes[0],
        Shape::Rectangle {
            id: 1,
            a: Point { x: -20.0, y: 5.0 },
            b: Point { x: 40.0, y: 35.0 },
        }
    );
    assert!(doc.undo());
    assert!(doc.shapes.is_empty());
}

#[test]
fn three_point_rectangles_follow_edge_and_center_placement() {
    let first = Point { x: 0.0, y: 0.0 };
    let second = Point { x: 30.0, y: 40.0 };
    let side = Point { x: -40.0, y: 30.0 };
    let shape = Shape::rectangle_from_three_points(1, first, second, side).unwrap();
    assert!(
        matches!(shape, Shape::RotatedRectangle { width, height, rotation, .. }
            if (width - 50.0).abs() < 1e-5 && (height - 50.0).abs() < 1e-5 && (rotation - 53.1301).abs() < 1e-3)
    );
    assert_eq!(shape.center(), Point { x: -5.0, y: 35.0 });
    let centered = Shape::rectangle_from_center_side(2, first, second, side).unwrap();
    assert!(
        matches!(centered, Shape::RotatedRectangle { width, height, .. }
            if (width - 100.0).abs() < 1e-5 && (height - 100.0).abs() < 1e-5)
    );
    assert_eq!(centered.center(), first);
    assert!(Shape::rectangle_from_three_points(3, first, second, second).is_none());
    assert!(Shape::rectangle_from_center_side(4, first, second, second).is_none());

    let mut doc = Document {
        tool: Tool::ThreePointRectangle,
        ..Document::default()
    };
    doc.draft_start = Some(first);
    assert!(!doc.finish_draft(second));
    assert!(doc.finish_draft(side));
    assert_eq!(doc.shapes[0], shape);
    assert!(doc.undo());
    assert!(doc.shapes.is_empty());
}

#[test]
fn copying_selected_shape_is_undoable_and_copies_its_dimension() {
    let original = Shape::Circle {
        id: 7,
        center: Point { x: 2.0, y: 3.0 },
        radius: 5.0,
        segments: 64,
    };
    let mut doc = Document::from_shapes(vec![original.clone()]);
    doc.selected = Some(7);
    doc.driving_dimensions
        .insert((7, DimensionField::CircleDiameter), 10.0);
    assert!(doc.copy_selected(Point { x: 10.0, y: -10.0 }));
    assert_eq!(doc.shapes[0], original);
    assert_eq!(
        doc.shapes[1],
        Shape::Circle {
            id: 8,
            center: Point { x: 12.0, y: -7.0 },
            radius: 5.0,
            segments: 64,
        }
    );
    assert_eq!(doc.selected, Some(8));
    assert_eq!(
        doc.driving_dimensions
            .get(&(8, DimensionField::CircleDiameter)),
        Some(&10.0)
    );
    assert!(doc.undo());
    assert_eq!(doc.shapes, vec![original]);
    assert!(doc.redo());
    assert_eq!(doc.shapes.len(), 2);
    assert!(!doc.copy_selected(Point {
        x: f32::NAN,
        y: 0.0
    }));
}

#[test]
fn numeric_move_is_one_undo_step_and_keeps_driving_dimension() {
    let original = Shape::Line {
        id: 4,
        a: Point { x: 1.0, y: 2.0 },
        b: Point { x: 11.0, y: 2.0 },
    };
    let mut doc = Document::from_shapes(vec![original.clone()]);
    doc.selected = Some(4);
    doc.driving_dimensions
        .insert((4, DimensionField::LineLength), 10.0);
    assert!(!doc.move_selected(Point { x: 0.0, y: 0.0 }));
    assert!(doc.move_selected(Point { x: -3.0, y: 7.0 }));
    assert_eq!(
        doc.shapes[0],
        Shape::Line {
            id: 4,
            a: Point { x: -2.0, y: 9.0 },
            b: Point { x: 8.0, y: 9.0 }
        }
    );
    assert_eq!(
        doc.driving_dimensions.get(&(4, DimensionField::LineLength)),
        Some(&10.0)
    );
    assert!(doc.undo());
    assert_eq!(doc.shapes, vec![original]);
    assert!(!doc.undo());
}

#[test]
fn scaling_changes_geometry_and_dimensions_and_copy_has_new_id() {
    let mut doc = Document::from_shapes(vec![Shape::Circle {
        id: 3,
        center: Point { x: 20.0, y: -4.0 },
        radius: 5.0,
        segments: 64,
    }]);
    doc.selected = Some(3);
    doc.driving_dimensions
        .insert((3, DimensionField::CircleDiameter), 10.0);
    assert!(!doc.scale_selected(-1.0, Point { x: 20.0, y: -4.0 }, false));
    assert!(doc.scale_selected(2.0, Point { x: 20.0, y: -4.0 }, false));
    assert_eq!(
        doc.shapes[0],
        Shape::Circle {
            id: 3,
            center: Point { x: 20.0, y: -4.0 },
            radius: 10.0,
            segments: 64,
        }
    );
    assert_eq!(
        doc.driving_dimensions
            .get(&(3, DimensionField::CircleDiameter)),
        Some(&20.0)
    );
    assert!(doc.scale_selected(0.5, Point { x: 20.0, y: -4.0 }, true));
    assert_eq!(
        doc.shapes[1],
        Shape::Circle {
            id: 4,
            center: Point { x: 20.0, y: -4.0 },
            radius: 5.0,
            segments: 64,
        }
    );
    assert_eq!(
        doc.driving_dimensions
            .get(&(4, DimensionField::CircleDiameter)),
        Some(&10.0)
    );
    assert!(doc.undo());
    assert_eq!(doc.shapes.len(), 1);
    assert!(doc.undo());
    assert_eq!(
        doc.driving_dimensions
            .get(&(3, DimensionField::CircleDiameter)),
        Some(&10.0)
    );
}

#[test]
fn scaling_supported_shape_types_preserves_their_centers() {
    let shapes = vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 8.0, y: 0.0 },
        },
        Shape::Rectangle {
            id: 2,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 8.0, y: 6.0 },
        },
        Shape::Circle {
            id: 3,
            center: Point { x: 4.0, y: 3.0 },
            radius: 4.0,
            segments: 64,
        },
        Shape::Ellipse {
            id: 4,
            center: Point { x: 4.0, y: 3.0 },
            rx: 4.0,
            ry: 2.0,
            rotation: 30.0,
            segments: 128,
        },
        Shape::Arc {
            id: 5,
            center: Point { x: 4.0, y: 3.0 },
            radius: 4.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 128,
        },
        Shape::Polygon {
            id: 6,
            center: Point { x: 4.0, y: 3.0 },
            radius: 4.0,
            sides: 5,
            rotation: 30.0,
        },
        Shape::Capsule {
            id: 7,
            from: Point { x: 0.0, y: 3.0 },
            to: Point { x: 8.0, y: 3.0 },
            width: 4.0,
            arc_segments: 48,
        },
    ];
    for shape in shapes {
        let center = shape.center();
        let scaled = shape.scaled_about(0.5, center).expect("valid scale");
        assert_eq!(scaled.center(), center);
        assert_eq!(scaled.id(), shape.id());
        assert_ne!(scaled, shape);
    }
}

#[test]
fn rotated_rectangle_hit_move_and_scale_preserve_orientation() {
    let shape = Shape::RotatedRectangle {
        id: 9,
        center: Point { x: 10.0, y: 20.0 },
        width: 40.0,
        height: 10.0,
        rotation: 45.0,
    };
    let corners = Shape::rotated_rectangle_corners(Point { x: 10.0, y: 20.0 }, 40.0, 10.0, 45.0);
    assert!(shape.hit(Point { x: 10.0, y: 20.0 }, 0.0));
    assert!(shape.hit(corners[0], 0.001));
    assert!(!shape.hit(Point { x: 30.0, y: 20.0 }, 0.0));
    let moved = shape.translated(Point { x: -5.0, y: 3.0 });
    assert!(matches!(
        moved,
        Shape::RotatedRectangle {
            center: Point { x: 5.0, y: 23.0 },
            rotation: 45.0,
            ..
        }
    ));
    let scaled = shape.scaled_about(2.0, shape.center()).unwrap();
    assert!(matches!(
        scaled,
        Shape::RotatedRectangle {
            width: 80.0,
            height: 20.0,
            rotation: 45.0,
            ..
        }
    ));
}

#[test]
fn rotating_rectangle_converts_to_primitive_and_is_undoable() {
    let original = Shape::Rectangle {
        id: 2,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 40.0, y: 20.0 },
    };
    let mut doc = Document::from_shapes(vec![original.clone()]);
    doc.selected = Some(2);
    doc.driving_dimensions
        .insert((2, DimensionField::RectangleWidth), 40.0);
    assert!(!doc.rotate_selected(360.0, Point { x: 20.0, y: 10.0 }, false));
    assert!(doc.rotate_selected(30.0, Point { x: 20.0, y: 10.0 }, false));
    assert!(matches!(
        doc.shapes[0],
        Shape::RotatedRectangle {
            center: Point { x: 20.0, y: 10.0 },
            width: 40.0,
            height: 20.0,
            rotation: 30.0,
            ..
        }
    ));
    assert_eq!(
        doc.driving_dimensions
            .get(&(2, DimensionField::RectangleWidth)),
        Some(&40.0)
    );
    assert!(doc.rotate_selected(-30.0, Point { x: 20.0, y: 10.0 }, true));
    assert!(matches!(
        doc.shapes[1],
        Shape::RotatedRectangle {
            id: 3,
            rotation: 0.0,
            ..
        }
    ));
    assert_eq!(
        doc.driving_dimensions
            .get(&(3, DimensionField::RectangleWidth)),
        Some(&40.0)
    );
    assert!(doc.undo());
    assert_eq!(doc.shapes.len(), 1);
    assert!(doc.undo());
    assert_eq!(doc.shapes[0], original);
}

#[test]
fn rotating_line_and_arc_uses_degrees_for_geometry_and_radians_for_arc_start() {
    let line = Shape::Line {
        id: 1,
        a: Point { x: 1.0, y: 0.0 },
        b: Point { x: 3.0, y: 0.0 },
    };
    let turned = line.rotated_about(90.0, Point { x: 0.0, y: 0.0 }).unwrap();
    assert!(
        matches!(turned, Shape::Line { a, b, .. } if a.x.abs() < 1e-5 && (a.y-1.0).abs() < 1e-5 && b.x.abs() < 1e-5 && (b.y-3.0).abs() < 1e-5)
    );
    let arc = Shape::Arc {
        id: 2,
        center: Point { x: 0.0, y: 0.0 },
        radius: 5.0,
        start_angle: 0.0,
        sweep: 1.0,
        segments: 128,
    };
    let turned = arc.rotated_about(90.0, Point { x: 0.0, y: 0.0 }).unwrap();
    assert!(
        matches!(turned, Shape::Arc { start_angle, sweep: 1.0, .. } if (start_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-5)
    );
}

#[test]
fn scale_and_rotate_use_requested_center() {
    let mut doc = Document::from_shapes(vec![Shape::Circle {
        id: 1,
        center: Point { x: 10.0, y: 0.0 },
        radius: 2.0,
        segments: 64,
    }]);
    doc.selected = Some(1);
    assert!(doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
    assert!(matches!(
        doc.shapes[0],
        Shape::Circle {
            center: Point { x: 20.0, y: 0.0 },
            radius: 4.0,
            ..
        }
    ));
    assert!(doc.rotate_selected(90.0, Point { x: 0.0, y: 0.0 }, false));
    assert!(
        matches!(doc.shapes[0], Shape::Circle { center, radius: 4.0, .. } if center.x.abs() < 1e-5 && (center.y - 20.0).abs() < 1e-5)
    );
    assert!(doc.undo());
    assert!(matches!(
        doc.shapes[0],
        Shape::Circle {
            center: Point { x: 20.0, y: 0.0 },
            ..
        }
    ));
}

#[test]
fn rejects_degenerate_shapes() {
    let mut doc = Document {
        tool: Tool::Circle,
        ..Document::default()
    };
    doc.draft_start = Some(Point { x: 0.0, y: 0.0 });
    assert!(!doc.finish_draft(Point { x: 0.5, y: 0.0 }));
    assert!(doc.shapes.is_empty());
    assert!(!doc.undo());
}

#[test]
fn line_hit_uses_finite_segment_distance() {
    let line = Shape::Line {
        id: 7,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 10.0, y: 0.0 },
    };
    assert!(line.hit(Point { x: 5.0, y: 0.4 }, 0.5));
    assert!(!line.hit(Point { x: 20.0, y: 0.0 }, 0.5));
}

#[test]
fn ellipse_creation_selection_and_dimensions() {
    let mut doc = Document {
        tool: Tool::Ellipse,
        ..Document::default()
    };
    doc.draft_start = Some(Point { x: 10.0, y: 20.0 });
    assert!(!doc.finish_draft(Point { x: 40.0, y: 20.0 }));
    assert!(doc.finish_draft(Point { x: 10.0, y: 35.0 }));
    doc.select_at(Point { x: 40.0, y: 20.0 });
    assert_eq!(doc.selected, Some(1));
    doc.set_dimension(DimensionField::RectangleWidth, 100.0)
        .unwrap();
    doc.set_dimension(DimensionField::RectangleHeight, 80.0)
        .unwrap();
    assert!(matches!(
        doc.shapes[0],
        Shape::Ellipse {
            rx: 50.0,
            ry: 40.0,
            ..
        }
    ));
    assert!(doc.undo());
    assert!(matches!(
        doc.shapes[0],
        Shape::Ellipse {
            rx: 50.0,
            ry: 15.0,
            ..
        }
    ));
}

#[test]
fn rotated_ellipse_uses_axis_direction_and_perpendicular_height() {
    let ellipse = Shape::ellipse_from_three_points(
        4,
        Point { x: 0.0, y: 0.0 },
        Point { x: 10.0, y: 10.0 },
        Point { x: -5.0, y: 5.0 },
    )
    .unwrap();
    assert!(
        matches!(ellipse, Shape::Ellipse { rotation, rx, ry, .. } if (rotation - 45.0).abs() < 1e-4 && (rx - 2.0_f32.sqrt()*10.0).abs() < 1e-4 && (ry - 2.0_f32.sqrt()*5.0).abs() < 1e-4)
    );
    assert!(ellipse.hit(Point { x: 10.0, y: 10.0 }, 0.1));
    assert!(
        Shape::ellipse_from_three_points(
            4,
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 5.0, y: 0.0 }
        )
        .is_none()
    );
}

#[test]
fn three_point_arc_keeps_major_sweep_and_is_undoable() {
    let start = Point { x: 10.0, y: 0.0 };
    let end = Point { x: 0.0, y: 10.0 };
    let through = Point { x: -10.0, y: 0.0 };
    let mut doc = Document {
        tool: Tool::Arc,
        ..Document::default()
    };
    doc.draft_start = Some(start);
    assert!(!doc.finish_draft(end));
    assert_eq!(doc.draft_end, Some(end));
    assert!(!doc.finish_draft(Point { x: 5.0, y: 5.0 }));
    assert!(doc.shapes.is_empty());
    assert_eq!(doc.draft_end, Some(end));
    assert!(doc.finish_draft(through));
    assert!(
        matches!(doc.shapes[0], Shape::Arc { sweep, .. } if (sweep + 3.0 * std::f64::consts::FRAC_PI_2).abs() < 1e-5)
    );
    assert!(doc.shapes[0].hit(through, 0.1));
    assert!(!doc.shapes[0].hit(Point { x: 0.0, y: 0.0 }, 0.1));
    assert!(doc.undo());
    assert!(doc.shapes.is_empty());
    assert!(doc.redo());
    assert_eq!(doc.shapes.len(), 1);
}

#[test]
fn three_point_arc_rejects_collinear_points() {
    assert!(
        Shape::arc_from_three_points(
            1,
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 0.0 },
            Point { x: 5.0, y: 0.0 }
        )
        .is_none()
    );
}

#[test]
fn point_and_polygon_creation_selection_side_edit_and_undo() {
    let mut doc = Document::default();
    assert!(doc.add_point(Point { x: 5.0, y: 6.0 }));
    assert!(matches!(
        doc.shapes[0],
        Shape::Point {
            at: Point { x: 5.0, y: 6.0 },
            ..
        }
    ));
    doc.tool = Tool::Polygon;
    doc.draft_start = Some(Point { x: 30.0, y: 40.0 });
    assert!(doc.finish_draft(Point { x: 50.0, y: 40.0 }));
    assert!(matches!(
        doc.shapes[1],
        Shape::Polygon {
            radius: 20.0,
            sides: 6,
            ..
        }
    ));
    doc.select_at(Point { x: 30.0, y: 40.0 });
    assert_eq!(doc.selected, Some(2));
    doc.set_polygon_sides(5).unwrap();
    assert!(matches!(doc.shapes[1], Shape::Polygon { sides: 5, .. }));
    assert!(doc.set_polygon_sides(2).is_err());
    assert!(doc.undo());
    assert!(matches!(doc.shapes[1], Shape::Polygon { sides: 6, .. }));
    assert!(doc.redo());
    assert!(matches!(doc.shapes[1], Shape::Polygon { sides: 5, .. }));
    doc.select_at(Point { x: 5.0, y: 6.0 });
    assert_eq!(doc.selected, Some(1));
}

#[test]
fn three_point_slot_width_hit_and_undo() {
    let mut doc = Document {
        tool: Tool::Slot,
        ..Document::default()
    };
    doc.draft_start = Some(Point { x: 0.0, y: 0.0 });
    assert!(!doc.finish_draft(Point { x: 20.0, y: 0.0 }));
    assert_eq!(doc.draft_end, Some(Point { x: 20.0, y: 0.0 }));
    assert!(!doc.finish_draft(Point { x: 10.0, y: 0.0 }));
    assert!(doc.finish_draft(Point { x: 10.0, y: 5.0 }));
    assert!(matches!(doc.shapes[0], Shape::Capsule { width: 10.0, .. }));
    assert!(doc.shapes[0].hit(Point { x: 10.0, y: 4.0 }, 0.1));
    assert!(!doc.shapes[0].hit(Point { x: 10.0, y: 8.0 }, 0.1));
    doc.set_capsule_width(20.0).unwrap();
    assert!(matches!(doc.shapes[0], Shape::Capsule { width: 20.0, .. }));
    assert!(doc.undo());
    assert!(matches!(doc.shapes[0], Shape::Capsule { width: 10.0, .. }));
    assert_eq!(
        Shape::capsule_outline(
            Point { x: 0.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            10.0,
            48
        )
        .len(),
        98
    );
}

#[test]
fn dragging_is_one_undoable_translation_and_preserves_dimensions() {
    let mut doc = Document::from_shapes(vec![Shape::Line {
        id: 1,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 3.0, y: 4.0 },
    }]);
    doc.selected = Some(1);
    doc.set_dimension(DimensionField::LineLength, 10.0).unwrap();
    // Grab the line body, outside the endpoint anchor pick radius.
    doc.zoom = 4.0;
    assert!(doc.begin_drag(Point { x: 3.0, y: 4.0 }));
    assert!(!doc.drag_to(Point { x: 3.0, y: 4.0 }));
    assert!(doc.drag_to(Point { x: 13.0, y: -1.0 }));
    assert!(doc.drag_to(Point { x: 15.0, y: -1.0 }));
    assert!(doc.finish_drag());
    assert!(matches!(
        doc.shapes[0],
        Shape::Line {
            a: Point { x: 12.0, y: -5.0 },
            b: Point { x: 18.0, y: 3.0 },
            ..
        }
    ));
    assert_eq!(
        doc.driving_dimensions.get(&(1, DimensionField::LineLength)),
        Some(&10.0)
    );
    assert!(doc.undo());
    assert!(matches!(
        doc.shapes[0],
        Shape::Line {
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 6.0, y: 8.0 },
            ..
        }
    ));
    assert!(doc.redo());
    assert!(matches!(
        doc.shapes[0],
        Shape::Line {
            a: Point { x: 12.0, y: -5.0 },
            ..
        }
    ));
}

#[test]
fn translation_moves_each_shape_without_changing_its_size() {
    let delta = Point { x: 7.0, y: -3.0 };
    let shapes = vec![
        Shape::Rectangle {
            id: 1,
            a: Point { x: 1.0, y: 2.0 },
            b: Point { x: 3.0, y: 4.0 },
        },
        Shape::Circle {
            id: 2,
            center: Point { x: 1.0, y: 2.0 },
            radius: 5.0,
            segments: 64,
        },
        Shape::Ellipse {
            id: 3,
            center: Point { x: 1.0, y: 2.0 },
            rx: 5.0,
            ry: 3.0,
            rotation: 30.0,
            segments: 128,
        },
        Shape::Arc {
            id: 4,
            center: Point { x: 1.0, y: 2.0 },
            radius: 5.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 128,
        },
        Shape::Point {
            id: 5,
            at: Point { x: 1.0, y: 2.0 },
        },
        Shape::Polygon {
            id: 6,
            center: Point { x: 1.0, y: 2.0 },
            radius: 5.0,
            sides: 6,
            rotation: 30.0,
        },
        Shape::Capsule {
            id: 7,
            from: Point { x: 1.0, y: 2.0 },
            to: Point { x: 3.0, y: 4.0 },
            width: 5.0,
            arc_segments: 48,
        },
    ];
    for shape in shapes {
        let moved = shape.translated(delta);
        assert_eq!(moved.id(), shape.id());
        assert_eq!(
            moved.translated(Point {
                x: -delta.x,
                y: -delta.y
            }),
            shape
        );
    }
}

#[test]
fn dimension_edits_are_centered_or_direction_preserving_and_undoable() {
    let mut document = Document::from_shapes(vec![
        Shape::Rectangle {
            id: 1,
            a: Point { x: 10.0, y: 10.0 },
            b: Point { x: 30.0, y: 20.0 },
        },
        Shape::Circle {
            id: 2,
            center: Point { x: 50.0, y: 50.0 },
            radius: 8.0,
            segments: 64,
        },
        Shape::Line {
            id: 3,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 3.0, y: 4.0 },
        },
    ]);
    document.selected = Some(1);
    document
        .set_dimension(DimensionField::RectangleWidth, 40.0)
        .unwrap();
    assert_eq!(
        document.shapes[0],
        Shape::Rectangle {
            id: 1,
            a: Point { x: 0.0, y: 10.0 },
            b: Point { x: 40.0, y: 20.0 }
        }
    );
    document.selected = Some(2);
    document
        .set_dimension(DimensionField::CircleDiameter, 30.0)
        .unwrap();
    assert!(matches!(
        document.shapes[1],
        Shape::Circle { radius: 15.0, .. }
    ));
    document.selected = Some(3);
    document
        .set_dimension(DimensionField::LineLength, 10.0)
        .unwrap();
    assert!(matches!(
        document.shapes[2],
        Shape::Line {
            b: Point { x: 6.0, y: 8.0 },
            ..
        }
    ));
    assert!(document.undo());
    assert_eq!(
        document
            .driving_dimensions
            .get(&(3, DimensionField::LineLength)),
        None
    );
    assert!(document.redo());
    assert_eq!(
        document
            .driving_dimensions
            .get(&(3, DimensionField::LineLength)),
        Some(&10.0)
    );
    assert!(matches!(
        document.shapes[2],
        Shape::Line {
            b: Point { x: 6.0, y: 8.0 },
            ..
        }
    ));
    assert!(
        document
            .set_dimension(DimensionField::LineLength, 0.0)
            .is_err()
    );
}

fn line_ends(doc: &Document, index: usize) -> (Point, Point) {
    match doc.shapes[index] {
        Shape::Line { a, b, .. } => (a, b),
        _ => panic!("expected a line"),
    }
}

fn line_cross(first: (Point, Point), second: (Point, Point)) -> f32 {
    let (a, b) = first;
    let (c, d) = second;
    ((b.x - a.x) * (d.y - c.y) - (b.y - a.y) * (d.x - c.x)) / (a.distance(b) * c.distance(d))
}

#[test]
fn solver_composes_equal_parallel_and_point_link_on_one_line() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: 30.0 },
            b: Point { x: 10.0, y: 35.0 },
        },
        Shape::Point {
            id: 3,
            at: Point { x: 0.0, y: 30.0 },
        },
    ]);
    doc.point_links.insert((2, 0), 3);
    doc.selected = Some(2);
    // Previously rejected: Equal with endpoint links, and two relations on one line.
    assert_eq!(doc.toggle_equal_line_with(1), Ok(true));
    assert_eq!(
        doc.toggle_line_relation_with(1, LineRelationKind::Parallel),
        Ok(true)
    );
    doc.selected = Some(3);
    assert!(doc.move_selected(Point { x: 7.0, y: -4.0 }));
    let source = line_ends(&doc, 0);
    let target = line_ends(&doc, 1);
    assert_eq!(
        source,
        (Point { x: 0.0, y: 0.0 }, Point { x: 20.0, y: 0.0 })
    );
    assert!(target.0.distance(Point { x: 7.0, y: 26.0 }) < 1e-4);
    assert!((target.0.distance(target.1) - 20.0).abs() < 1e-3);
    assert!(line_cross(source, target).abs() < 1e-4);
    // One undo step restores the whole solved edit.
    assert!(doc.undo());
    assert!(matches!(doc.shapes[2], Shape::Point { at, .. } if at == Point { x: 0.0, y: 30.0 }));
    assert_eq!(line_ends(&doc, 1).0, Point { x: 0.0, y: 30.0 });
}

#[test]
fn solver_propagates_chained_direction_relations() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: 10.0 },
            b: Point { x: 10.0, y: 10.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: 0.0, y: 20.0 },
            b: Point { x: 0.0, y: 30.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(
        doc.toggle_line_relation_with(1, LineRelationKind::Parallel),
        Ok(true)
    );
    doc.selected = Some(3);
    assert_eq!(
        doc.toggle_line_relation_with(2, LineRelationKind::Perpendicular),
        Ok(true)
    );
    doc.selected = Some(1);
    assert_eq!(doc.set_dimension(DimensionField::Rotation, 30.0), Ok(()));
    let first = line_ends(&doc, 0);
    let second = line_ends(&doc, 1);
    let third = line_ends(&doc, 2);
    assert!(line_cross(first, second).abs() < 1e-4);
    assert!((line_cross(second, third).abs() - 1.0).abs() < 1e-4);
    // Targets rotate about their first endpoint and keep their length.
    assert_eq!(second.0, Point { x: 0.0, y: 10.0 });
    assert!((second.0.distance(second.1) - 10.0).abs() < 1e-3);
    assert_eq!(third.0, Point { x: 0.0, y: 20.0 });
}

#[test]
fn conflicting_relation_is_rejected_and_rolled_back() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Line {
            id: 2,
            a: Point { x: 0.0, y: 10.0 },
            b: Point { x: 3.0, y: 20.0 },
        },
    ]);
    doc.selected = Some(1);
    assert_eq!(
        doc.toggle_axis_constraint(AxisConstraint::Horizontal),
        Ok(true)
    );
    doc.selected = Some(2);
    assert_eq!(
        doc.toggle_axis_constraint(AxisConstraint::Vertical),
        Ok(true)
    );
    let before = doc.shapes.clone();
    let history = doc.history.len();
    assert!(
        doc.toggle_line_relation_with(1, LineRelationKind::Parallel)
            .is_err()
    );
    assert!(doc.line_relations.is_empty());
    assert_eq!(doc.shapes, before);
    assert_eq!(doc.history.len(), history);
    assert_eq!(doc.selected, Some(2));
    assert_eq!(
        doc.toggle_line_relation_with(1, LineRelationKind::Perpendicular),
        Ok(true)
    );
}

#[test]
fn dragging_point_on_dimensioned_line_end_rotates_line_under_constraints() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 0.0 },
        },
        Shape::Point {
            id: 2,
            at: Point { x: 10.0, y: 0.0 },
        },
    ]);
    doc.point_links.insert((1, 1), 2);
    doc.fixed_anchors.insert((1, 0), Point { x: 0.0, y: 0.0 });
    doc.driving_dimensions
        .insert((1, DimensionField::LineLength), 10.0);
    doc.selected = Some(2);
    // Dragging the point is a soft target: the fixed, dimensioned line
    // cannot reach it, so the point slides on the length circle...
    assert!(doc.begin_drag(Point { x: 10.0, y: 0.0 }));
    assert!(doc.drag_to(Point { x: 10.0, y: 6.0 }));
    let (a, b) = line_ends(&doc, 0);
    assert!(a.distance(Point { x: 0.0, y: 0.0 }) < 1e-4);
    assert!((a.distance(b) - 10.0).abs() < 1e-3);
    let scale = 10.0 / 10.0f32.hypot(6.0);
    assert!(
        b.distance(Point {
            x: 10.0 * scale,
            y: 6.0 * scale
        }) < 1e-2
    );
    assert!(doc.cancel_drag());
    // ...while the point-coordinate edit along the circle is accepted.
    doc.selected = Some(2);
    doc.driving_dimensions
        .remove(&(1, DimensionField::LineLength));
    assert!(doc.begin_drag(Point { x: 10.0, y: 0.0 }));
    assert!(doc.drag_to(Point { x: 10.0, y: 6.0 }));
    assert!(doc.finish_drag());
    let (a, b) = line_ends(&doc, 0);
    assert_eq!(a, Point { x: 0.0, y: 0.0 });
    assert!(b.distance(Point { x: 10.0, y: 6.0 }) < 1e-4);
}
fn line_doc(lines: &[(Point, Point)]) -> Document {
    let shapes = lines
        .iter()
        .enumerate()
        .map(|(index, &(a, b))| Shape::Line {
            id: index as u32 + 1,
            a,
            b,
        })
        .collect();
    Document::from_shapes(shapes)
}

fn pt(x: f32, y: f32) -> Point {
    Point { x, y }
}

#[test]
fn constraint_analysis_counts_dof_and_flags_relations() {
    let mut doc = line_doc(&[(pt(0.0, 0.0), pt(10.0, 2.0))]);
    let analysis = doc.analyze_constraints();
    assert_eq!(analysis.dof, 4);
    assert_eq!(analysis.label(), "DOF 4");
    doc.fixed_anchors.insert((1, 0), pt(0.0, 0.0));
    doc.fixed_anchors.insert((1, 1), pt(10.0, 2.0));
    let analysis = doc.analyze_constraints();
    assert_eq!(analysis.dof, 0);
    assert!(analysis.fully_constrained.contains(&1));

    let mut doc = line_doc(&[(pt(0.0, 0.0), pt(10.0, 0.0))]);
    doc.axis_constraints.insert(1, AxisConstraint::Horizontal);
    assert_eq!(doc.analyze_constraints().dof, 3);
    // A stored conflicting state (e.g. from a file) is diagnosed.
    doc.driving_dimensions
        .insert((1, DimensionField::Rotation), 90.0);
    let analysis = doc.analyze_constraints();
    assert!(analysis.conflicting.contains(&1));
    assert_eq!(analysis.label(), "CONFLICT");

    let mut doc = line_doc(&[(pt(0.0, 0.0), pt(10.0, 0.0)), (pt(0.0, 5.0), pt(8.0, 5.0))]);
    doc.axis_constraints.insert(1, AxisConstraint::Horizontal);
    doc.axis_constraints.insert(2, AxisConstraint::Horizontal);
    doc.line_relations
        .insert(2, (1, LineRelationKind::Parallel));
    let analysis = doc.analyze_constraints();
    assert_eq!(analysis.label(), "OVER-CONSTRAINED");
    assert!(!analysis.redundant.is_empty());
    assert!(analysis.conflicting.is_empty());
}

#[test]
fn constraint_analysis_counts_curve_parameters() {
    let doc = Document::from_shapes(vec![
        Shape::Ellipse {
            id: 1,
            center: pt(0.0, 0.0),
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 32,
        },
        Shape::Arc {
            id: 2,
            center: pt(30.0, 0.0),
            radius: 5.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 16,
        },
    ]);
    // Ellipse: center, rx, ry, rotation; arc: center, radius, start, sweep.
    assert_eq!(doc.analyze_constraints().dof, 10);
}

#[test]
fn dragging_circle_center_keeps_external_tangency() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: pt(-50.0, 0.0),
            b: pt(50.0, 0.0),
        },
        Shape::Circle {
            id: 2,
            center: pt(0.0, 5.0),
            radius: 5.0,
            segments: 32,
        },
    ]);
    doc.external_tangents.insert(2, 1);
    assert!(doc.begin_anchor_drag(2, 0, pt(0.0, 5.0)));
    assert!(doc.drag_to(pt(10.0, 8.0)));
    assert!(doc.finish_drag());
    let Shape::Circle { center, radius, .. } = doc.shapes[1] else {
        panic!("expected a circle");
    };
    let (a, b) = line_ends(&doc, 0);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let distance = ((center.x - a.x) * dy - (center.y - a.y) * dx).abs() / dx.hypot(dy);
    assert!((distance - radius).abs() < 1e-2, "{distance} vs {radius}");
    assert!(center.x > 5.0);
}

#[test]
fn revision_changes_on_commit_and_undo() {
    let mut doc = Document::default();
    let start = doc.revision();
    assert!(doc.add_point(pt(1.0, 1.0)));
    let added = doc.revision();
    assert_ne!(start, added);
    assert_eq!(doc.revision(), added);
    doc.undo();
    assert_ne!(doc.revision(), added);
}

#[test]
fn dragging_line_endpoint_with_length_dimension_moves_on_circle() {
    let mut doc = Document::from_shapes(vec![Shape::Line {
        id: 1,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 10.0, y: 0.0 },
    }]);
    doc.driving_dimensions
        .insert((1, DimensionField::LineLength), 10.0);
    doc.selected = Some(1);
    assert_eq!(doc.anchor_at(Point { x: 10.0, y: 1.0 }), Some((1, 1)));
    let before = doc.shapes.clone();
    assert!(doc.begin_drag(Point { x: 10.0, y: 0.0 }));
    for target in [Point { x: 5.0, y: 5.0 }, Point { x: 0.0, y: 20.0 }] {
        assert!(doc.drag_to(target));
        let (a, b) = line_ends(&doc, 0);
        assert!(a.distance(Point { x: 0.0, y: 0.0 }) < 1e-4);
        assert!((a.distance(b) - 10.0).abs() < 1e-3);
        // The end lands where the cursor direction meets the circle.
        let scale = 10.0 / target.x.hypot(target.y);
        let expected = Point {
            x: target.x * scale,
            y: target.y * scale,
        };
        assert!(b.distance(expected) < 1e-2, "{b:?} vs {expected:?}");
    }
    assert!(doc.finish_drag());
    assert!(doc.undo());
    assert_eq!(doc.shapes, before);
    // Esc restores the frame-zero geometry.
    doc.selected = Some(1);
    assert!(doc.begin_drag(Point { x: 10.0, y: 0.0 }));
    assert!(doc.drag_to(Point { x: 0.0, y: 10.0 }));
    assert!(doc.cancel_drag());
    assert_eq!(doc.shapes, before);
}

#[test]
fn dragging_coincident_line_endpoint_carries_linked_point() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 0.0 },
        },
        Shape::Point {
            id: 2,
            at: Point { x: 10.0, y: 0.0 },
        },
    ]);
    doc.point_links.insert((1, 1), 2);
    doc.selected = Some(1);
    assert!(doc.begin_drag(Point { x: 10.0, y: 0.0 }));
    assert!(doc.drag_to(Point { x: 12.0, y: 7.0 }));
    assert!(doc.finish_drag());
    let (a, b) = line_ends(&doc, 0);
    assert_eq!(a, Point { x: 0.0, y: 0.0 });
    assert!(b.distance(Point { x: 12.0, y: 7.0 }) < 1e-3);
    let Shape::Point { at, .. } = doc.shapes[1] else {
        panic!("expected a point");
    };
    assert!(at.distance(b) < 1e-3);
    assert!(doc.undo());
    assert_eq!(line_ends(&doc, 0).1, Point { x: 10.0, y: 0.0 });
}

#[test]
fn fixed_line_endpoint_refuses_anchor_drag() {
    let mut doc = Document::from_shapes(vec![Shape::Line {
        id: 1,
        a: Point { x: 0.0, y: 0.0 },
        b: Point { x: 10.0, y: 0.0 },
    }]);
    doc.fixed_anchors.insert((1, 1), Point { x: 10.0, y: 0.0 });
    doc.selected = Some(1);
    assert!(!doc.begin_anchor_drag(1, 1, Point { x: 10.0, y: 0.0 }));
    assert!(!doc.begin_drag(Point { x: 10.0, y: 0.0 }));
    assert!(!doc.dragging());
    // The free end still drags while the fixed end stays.
    assert!(doc.begin_drag(Point { x: 0.0, y: 0.0 }));
    assert!(doc.drag_to(Point { x: -3.0, y: 4.0 }));
    assert!(doc.finish_drag());
    assert_eq!(line_ends(&doc, 0).1, Point { x: 10.0, y: 0.0 });
    assert_eq!(line_ends(&doc, 0).0, Point { x: -3.0, y: 4.0 });
}

#[test]
fn shape_anchor_drags_respect_driving_dimensions() {
    let mut doc = Document::from_shapes(vec![
        Shape::Rectangle {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 5.0 },
        },
        Shape::Polygon {
            id: 2,
            center: Point { x: 50.0, y: 0.0 },
            radius: 10.0,
            sides: 6,
            rotation: 0.0,
        },
    ]);
    doc.driving_dimensions
        .insert((1, DimensionField::RectangleWidth), 10.0);
    doc.selected = Some(1);
    // Corner (b.x, a.y): width is held, height follows the cursor.
    assert!(doc.begin_drag(Point { x: 10.0, y: 0.0 }));
    assert!(doc.drag_to(Point { x: 14.0, y: -3.0 }));
    assert!(doc.finish_drag());
    assert_eq!(
        doc.shapes[0],
        Shape::Rectangle {
            id: 1,
            a: Point { x: 0.0, y: -3.0 },
            b: Point { x: 10.0, y: 5.0 },
        }
    );
    doc.selected = Some(2);
    assert!(doc.begin_drag(Point { x: 60.0, y: 0.0 }));
    assert!(doc.drag_to(Point { x: 50.0, y: 20.0 }));
    assert!(doc.finish_drag());
    let Shape::Polygon {
        radius, rotation, ..
    } = doc.shapes[1]
    else {
        panic!("expected a polygon");
    };
    assert!((radius - 20.0).abs() < 1e-4);
    assert!((rotation - 90.0).abs() < 1e-3);
}

#[test]
fn point_on_line_stays_on_the_finite_segment() {
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 20.0, y: 0.0 },
        },
        Shape::Point {
            id: 2,
            at: Point { x: 18.0, y: 3.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_point_on_curve_with(1), Ok(true));
    doc.selected = Some(1);
    assert_eq!(doc.set_dimension(DimensionField::LineLength, 10.0), Ok(()));
    let at = doc.shapes[1].center();
    assert!(at.y.abs() < 1e-4 && at.x <= 10.0 + 1e-4, "{at:?}");
}

#[test]
fn equal_ellipse_scale_and_spline_point_links_follow_transforms() {
    let ellipse = |id, x| Shape::Ellipse {
        id,
        center: Point { x, y: 0.0 },
        rx: 10.0,
        ry: 5.0,
        rotation: 0.0,
        segments: 64,
    };
    let mut doc = Document::from_shapes(vec![ellipse(1, 0.0), ellipse(2, 40.0)]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_equal_ellipse_with(1), Ok(true));
    doc.selected = Some(1);
    assert!(doc.scale_selected(2.0, Point { x: 0.0, y: 0.0 }, false));
    assert!(matches!(
        doc.shapes[1],
        Shape::Ellipse { rx, ry, center, .. }
            if (rx - 20.0).abs() < 1e-4 && (ry - 10.0).abs() < 1e-4 && center.x == 40.0
    ));
    assert!(doc.undo());
    assert!(matches!(doc.shapes[1], Shape::Ellipse { rx: 10.0, .. }));

    let mut doc = Document::default();
    let start = Point { x: 0.0, y: 0.0 };
    assert!(doc.add_point(start));
    let spline = doc
        .add_spline(
            SplineMode::Fit,
            vec![start, Point { x: 10.0, y: 8.0 }, Point { x: 20.0, y: 0.0 }],
            false,
        )
        .unwrap();
    assert_eq!(doc.point_links.get(&(spline, 0)), Some(&1));
    doc.selected = Some(spline);
    assert!(doc.rotate_selected(90.0, Point { x: 10.0, y: 0.0 }, false));
    assert!(doc.shapes[0].center().distance(Point { x: 10.0, y: -10.0 }) < 1e-4);
    assert!(doc.scale_selected(0.5, Point { x: 10.0, y: 0.0 }, false));
    assert!(doc.shapes[0].center().distance(Point { x: 10.0, y: -5.0 }) < 1e-4);
}

#[test]
fn curve_relations_follow_through_the_solver() {
    // Point on an arc stays inside the sweep when the arc turns.
    let mut doc = Document::from_shapes(vec![
        Shape::Arc {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            start_angle: 0.0,
            sweep: std::f64::consts::FRAC_PI_2,
            segments: 32,
        },
        Shape::Point {
            id: 2,
            at: Point { x: 1.0, y: 9.0 },
        },
    ]);
    doc.selected = Some(2);
    assert_eq!(doc.toggle_point_on_curve_with(1), Ok(true));
    doc.selected = Some(1);
    assert!(doc.rotate_selected(-60.0, Point { x: 0.0, y: 0.0 }, false));
    let at = doc.shapes[1].center();
    let angle = (at.y as f64).atan2(at.x as f64);
    assert!(((at.x.hypot(at.y)) - 10.0).abs() < 1e-3);
    assert!(
        (-std::f64::consts::FRAC_PI_3 - 1e-4..=std::f64::consts::FRAC_PI_6 + 1e-4).contains(&angle),
        "{angle}"
    );

    // Smooth join: turning the line turns the arc's start with it.
    let mut doc = Document::from_shapes(vec![
        Shape::Line {
            id: 1,
            a: Point { x: 10.0, y: -10.0 },
            b: Point { x: 10.0, y: 0.0 },
        },
        Shape::Arc {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            radius: 10.0,
            start_angle: 0.0,
            sweep: 1.0,
            segments: 32,
        },
    ]);
    doc.endpoint_tangents.insert(2, (1, 0, 1, 1));
    doc.selected = Some(1);
    assert!(doc.rotate_selected(30.0, Point { x: 10.0, y: 0.0 }, false));
    let Shape::Arc { start_angle, .. } = doc.shapes[1] else {
        panic!()
    };
    assert!(
        (start_angle - 30f64.to_radians()).abs() < 1e-4,
        "{start_angle}"
    );

    // Shared ellipse pieces and an ellipse concentric partner follow.
    let mut doc = Document::from_shapes(vec![
        Shape::Ellipse {
            id: 1,
            center: Point { x: 0.0, y: 0.0 },
            rx: 10.0,
            ry: 5.0,
            rotation: 0.0,
            segments: 64,
        },
        Shape::Circle {
            id: 2,
            center: Point { x: 0.0, y: 0.0 },
            radius: 2.0,
            segments: 32,
        },
    ]);
    doc.concentric.insert(2, 1);
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 3.0, y: 4.0 }));
    assert_eq!(doc.shapes[1].center(), Point { x: 3.0, y: 4.0 });
}

fn axis_line(id: u32) -> Shape {
    // Vertical mirror line x = 10.
    Shape::Line {
        id,
        a: Point { x: 10.0, y: -50.0 },
        b: Point { x: 10.0, y: 50.0 },
    }
}

fn near(a: Point, b: Point) -> bool {
    a.distance(b) < 1e-3
}

/// Every mirror relation holds: target equals the mirror image of source.
fn assert_mirrored(doc: &Document) {
    for (&target, &(source, axis)) in &doc.mirrors {
        let find = |id: u32| doc.shapes.iter().find(|s| s.id() == id).unwrap().clone();
        let Shape::Line { a, b, .. } = find(axis) else {
            panic!("axis is not a line");
        };
        let expected = mirror_shape(&find(source), a, b).unwrap().with_id(target);
        let actual = find(target);
        let (p, q) = (expected.snap_points(), actual.snap_points());
        assert!(
            p.len() == q.len() && p.iter().all(|p| q.iter().any(|q| near(p.point, q.point))),
            "{expected:?} != {actual:?}"
        );
    }
}

#[test]
fn mirror_shape_reflects_each_kind() {
    let a = Point { x: 0.0, y: 0.0 };
    let b = Point { x: 0.0, y: 10.0 };
    let line = Shape::Line {
        id: 1,
        a: Point { x: 1.0, y: 2.0 },
        b: Point { x: 3.0, y: 5.0 },
    };
    assert_eq!(
        mirror_shape(&line, a, b),
        Some(Shape::Line {
            id: 1,
            a: Point { x: -1.0, y: 2.0 },
            b: Point { x: -3.0, y: 5.0 },
        })
    );
    let arc = Shape::Arc {
        id: 2,
        center: Point { x: 5.0, y: 0.0 },
        radius: 2.0,
        start_angle: 0.0,
        sweep: 1.0,
        segments: 32,
    };
    let Some(Shape::Arc {
        center,
        start_angle,
        sweep,
        ..
    }) = mirror_shape(&arc, a, b)
    else {
        panic!("arc");
    };
    assert!(near(center, Point { x: -5.0, y: 0.0 }));
    assert!((start_angle - std::f64::consts::PI).abs() < 1e-9 && sweep == -1.0);
    let ellipse = Shape::EllipticArc {
        id: 3,
        center: Point { x: 4.0, y: 1.0 },
        rx: 3.0,
        ry: 1.0,
        rotation: 30.0,
        start_angle: 0.5,
        sweep: 1.0,
        segments: 32,
    };
    let mirrored = mirror_shape(&ellipse, a, b).unwrap();
    assert!(matches!(
        mirrored,
        Shape::EllipticArc { rotation, start_angle, sweep, .. }
            if (rotation - 150.0).abs() < 1e-4 && start_angle == -0.5 && sweep == -1.0
    ));
    // Every anchor of the source maps onto an anchor of the mirror image.
    for shape in [
        ellipse,
        Shape::Polygon {
            id: 4,
            center: Point { x: 6.0, y: 2.0 },
            radius: 3.0,
            sides: 5,
            rotation: 10.0,
        },
        Shape::Rectangle {
            id: 5,
            a: Point { x: 2.0, y: 1.0 },
            b: Point { x: 6.0, y: 3.0 },
        },
        Shape::Capsule {
            id: 6,
            from: Point { x: 2.0, y: 1.0 },
            to: Point { x: 6.0, y: 3.0 },
            width: 2.0,
            arc_segments: 8,
        },
        Shape::Spline {
            id: 7,
            mode: crate::spline::SplineMode::Fit,
            points: vec![
                Point { x: 1.0, y: 1.0 },
                Point { x: 2.0, y: 3.0 },
                Point { x: 4.0, y: 2.0 },
            ],
            closed: false,
        },
    ] {
        let mirrored = mirror_shape(&shape, a, b).unwrap();
        for snap in shape.snap_points() {
            let image = reflect_point(snap.point, a, b).unwrap();
            assert!(
                mirrored.snap_points().iter().any(|s| near(s.point, image)),
                "{shape:?} -> {mirrored:?} misses {image:?}"
            );
        }
    }
    assert!(matches!(
        mirror_shape(
            &Shape::Rectangle {
                id: 5,
                a: Point { x: 2.0, y: 1.0 },
                b: Point { x: 6.0, y: 3.0 },
            },
            a,
            b
        ),
        Some(Shape::RotatedRectangle { .. })
    ));
}

#[test]
fn mirror_tool_copies_each_kind_with_one_undo_step() {
    let shapes = vec![
        axis_line(1),
        Shape::Line {
            id: 2,
            a: Point { x: 20.0, y: 0.0 },
            b: Point { x: 30.0, y: 5.0 },
        },
        Shape::Point {
            id: 3,
            at: Point { x: 25.0, y: 25.0 },
        },
        Shape::Circle {
            id: 4,
            center: Point { x: 40.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        },
        Shape::Arc {
            id: 5,
            center: Point { x: 40.0, y: 20.0 },
            radius: 5.0,
            start_angle: 0.3,
            sweep: 1.5,
            segments: 32,
        },
        Shape::Ellipse {
            id: 6,
            center: Point { x: 30.0, y: -20.0 },
            rx: 6.0,
            ry: 3.0,
            rotation: 20.0,
            segments: 64,
        },
        Shape::Rectangle {
            id: 7,
            a: Point { x: 15.0, y: 30.0 },
            b: Point { x: 25.0, y: 40.0 },
        },
        Shape::Polygon {
            id: 8,
            center: Point { x: 50.0, y: 30.0 },
            radius: 4.0,
            sides: 6,
            rotation: 15.0,
        },
        Shape::Spline {
            id: 9,
            mode: crate::spline::SplineMode::Fit,
            points: vec![
                Point { x: 20.0, y: -30.0 },
                Point { x: 25.0, y: -25.0 },
                Point { x: 30.0, y: -32.0 },
            ],
            closed: false,
        },
    ];
    for id in 2..=9 {
        let mut doc = Document::from_shapes(shapes.clone());
        let created = doc.mirror_shapes(&[id], 1).unwrap();
        assert_eq!(created.len(), 1);
        let copy = created[0];
        assert_eq!(doc.mirrors.get(&copy), Some(&(id, 1)));
        assert!(doc.has_symmetry(copy) && doc.has_symmetry(id));
        assert_eq!(doc.shapes.len(), shapes.len() + 1);
        // The source is unchanged and the copy is its exact mirror image.
        assert_eq!(doc.shapes[id as usize - 1], shapes[id as usize - 1]);
        assert_mirrored(&doc);
        assert!(doc.undo());
        assert_eq!(doc.shapes, shapes);
        assert!(doc.mirrors.is_empty());
    }
}

#[test]
fn mirrored_copy_follows_either_side_and_the_axis() {
    let mut doc = Document::from_shapes(vec![
        axis_line(1),
        Shape::Circle {
            id: 2,
            center: Point { x: 30.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        },
        Shape::Arc {
            id: 3,
            center: Point { x: 30.0, y: 20.0 },
            radius: 4.0,
            start_angle: 0.2,
            sweep: 1.2,
            segments: 32,
        },
        Shape::Polygon {
            id: 4,
            center: Point { x: 30.0, y: -20.0 },
            radius: 4.0,
            sides: 5,
            rotation: 0.0,
        },
    ]);
    let circle = doc.mirror_shapes(&[2], 1).unwrap()[0];
    doc.mirror_shapes(&[3], 1).unwrap();
    let polygon = doc.mirror_shapes(&[4], 1).unwrap()[0];
    let center =
        |doc: &Document, id: u32| doc.shapes.iter().find(|s| s.id() == id).unwrap().center();
    assert!(near(center(&doc, circle), Point { x: -10.0, y: 0.0 }));
    // Move the source: the copy follows.
    doc.selected = Some(2);
    assert!(doc.move_selected(Point { x: 5.0, y: 3.0 }));
    assert!(near(center(&doc, circle), Point { x: -15.0, y: 3.0 }));
    // Resize the source: the copy's radius follows.
    assert_eq!(doc.set_dimension(DimensionField::CurveRadius, 8.0), Ok(()));
    assert!(matches!(
        doc.shapes.iter().find(|s| s.id() == circle),
        Some(Shape::Circle { radius, .. }) if (radius - 8.0).abs() < 1e-3
    ));
    // Move the copy: the source follows.
    doc.selected = Some(circle);
    assert!(doc.move_selected(Point { x: -5.0, y: 2.0 }));
    assert!(near(center(&doc, circle), Point { x: -20.0, y: 5.0 }));
    assert!(near(center(&doc, 2), Point { x: 40.0, y: 5.0 }));
    // Rotate the arc source: the mirrored arc keeps the mirrored sweep.
    doc.selected = Some(3);
    assert!(doc.rotate_selected(30.0, Point { x: 30.0, y: 20.0 }, false));
    assert_mirrored(&doc);
    // Move the axis: every copy follows.
    doc.selected = Some(1);
    assert!(doc.move_selected(Point { x: 10.0, y: 0.0 }));
    assert_mirrored(&doc);
    assert!(near(center(&doc, polygon), Point { x: 10.0, y: -20.0 }));
    // Deleting the axis drops the relations.
    doc.selected = Some(1);
    assert!(doc.delete_selected());
    assert!(doc.mirrors.is_empty());
}

#[test]
fn symmetric_points_hold_after_moving_either_side() {
    let mut doc = Document::from_shapes(vec![
        axis_line(1),
        Shape::Point {
            id: 2,
            at: Point { x: 20.0, y: 5.0 },
        },
        Shape::Line {
            id: 3,
            a: Point { x: -5.0, y: 0.0 },
            b: Point { x: -20.0, y: -10.0 },
        },
    ]);
    // Point 2 and the first end of line 3 become symmetric about x = 10.
    assert_eq!(
        doc.toggle_symmetry_at(
            1,
            Point { x: 20.0, y: 5.0 },
            Point { x: -5.5, y: -0.3 },
            1.0
        ),
        Ok(true)
    );
    assert_eq!(doc.symmetric_points.len(), 1);
    assert_eq!(doc.symmetric_points[0].a, (2, 0));
    assert_eq!(doc.symmetric_points[0].b, (3, 0));
    let end = |doc: &Document| match doc.shapes[2] {
        Shape::Line { a, .. } => a,
        _ => unreachable!(),
    };
    let point = |doc: &Document| doc.shapes[1].center();
    let symmetric = |doc: &Document| {
        let (p, q) = (point(doc), end(doc));
        ((p.x + q.x) * 0.5 - 10.0).abs() < 1e-3 && (p.y - q.y).abs() < 1e-3
    };
    assert!(symmetric(&doc));
    assert!(near(point(&doc), Point { x: 20.0, y: 5.0 }));
    doc.selected = Some(2);
    assert!(doc.move_selected(Point { x: 3.0, y: 4.0 }));
    assert!(symmetric(&doc));
    assert!(near(end(&doc), Point { x: -3.0, y: 9.0 }));
    doc.selected = Some(3);
    assert!(doc.move_selected(Point { x: -2.0, y: 1.0 }));
    assert!(symmetric(&doc));
    assert!(doc.undo());
    assert!(doc.undo());
    assert!(near(point(&doc), Point { x: 20.0, y: 5.0 }));
    // Toggling again removes the relation.
    assert_eq!(
        doc.toggle_symmetry_at(1, Point { x: 20.0, y: 5.0 }, Point { x: 0.0, y: 5.0 }, 1.0),
        Ok(false)
    );
    assert!(doc.symmetric_points.is_empty());
    assert!(doc.undo());
    assert_eq!(doc.symmetric_points.len(), 1);
}

#[test]
fn symmetry_gesture_links_two_existing_shapes_of_the_same_kind() {
    let mut doc = Document::from_shapes(vec![
        axis_line(1),
        Shape::Circle {
            id: 2,
            center: Point { x: 30.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        },
        Shape::Circle {
            id: 3,
            center: Point { x: -12.0, y: 4.0 },
            radius: 3.0,
            segments: 64,
        },
    ]);
    assert_eq!(
        doc.toggle_symmetry_at(1, Point { x: 35.0, y: 0.0 }, Point { x: -9.0, y: 4.0 }, 0.5),
        Ok(true)
    );
    assert_eq!(doc.mirrors.get(&3), Some(&(2, 1)));
    assert_mirrored(&doc);
    assert!(near(doc.shapes[1].center(), Point { x: 30.0, y: 0.0 }));
    let analysis = doc.analyze_constraints();
    assert!(analysis.conflicting.is_empty() && analysis.redundant.is_empty());
    assert!(doc.undo());
    assert!(doc.mirrors.is_empty());
    assert!(near(doc.shapes[2].center(), Point { x: -12.0, y: 4.0 }));
    // Different kinds are not mirror images; the axis cannot be a side.
    let mut doc = Document::from_shapes(vec![
        axis_line(1),
        Shape::Circle {
            id: 2,
            center: Point { x: 30.0, y: 0.0 },
            radius: 5.0,
            segments: 64,
        },
        Shape::Polygon {
            id: 3,
            center: Point { x: -12.0, y: 4.0 },
            radius: 3.0,
            sides: 5,
            rotation: 0.0,
        },
    ]);
    assert!(doc.toggle_mirror_relation(2, 3, 1).is_err());
    assert!(doc.toggle_mirror_relation(1, 2, 1).is_err());
}
