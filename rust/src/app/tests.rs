use super::*;

#[test]
fn layout_keeps_three_columns_and_toolbar_separate() {
    let layout = Layout::new(1280.0, 800.0);
    assert_eq!(layout.viewport.x, layout.left.w);
    assert_eq!(layout.viewport.x + layout.viewport.w, layout.right.x);
    assert!(layout.viewport.h > 0.0);
    assert!(layout.tool_button(2).contains(265.0, 55.0));
    assert!(layout.tool_button(5).contains(675.0, 55.0));
    assert!(layout.tool_button(9).contains(1150.0, 55.0));
    assert!(layout.more_item(1).contains(1150.0, 145.0));
    assert!(layout.more_item(2).contains(1150.0, 180.0));
    // MORE rows past the 18th open a second column that stays on screen.
    for index in 0..20 {
        let item = layout.more_item(index);
        assert!(
            item.y + item.h <= 800.0 && item.x >= 0.0,
            "{index}: {item:?}"
        );
    }
    assert!(layout.more_item(18).contains(1030.0, 110.0));
    assert!(layout.more_item(19).contains(1030.0, 146.0));
    assert!(layout.sketch_row(1, 0).contains(100.0, 187.0));
    assert!(layout.row(0, 0, 2).contains(100.0, 227.0));
    assert!(layout.part_row(1).contains(100.0, 187.0));
    assert!(layout.sketch_row(0, 2).contains(100.0, 222.0));
}

#[test]
fn clip_text_keeps_path_tails_and_name_heads() {
    assert_eq!(clip_text("/home/user/cad", 20, true), "/home/user/cad");
    assert_eq!(clip_text("/home/user/cad", 8, true), "...r/cad");
    assert_eq!(clip_text("Long project name", 8, false), "Long ...");
}

#[test]
fn breadcrumbs_lay_out_in_order_and_truncate_long_names() {
    let crumbs = layout_breadcrumbs(
        vec![
            (Crumb::Project, "two-parts".into()),
            (Crumb::Part, "A very long part name that keeps going".into()),
            (Crumb::Sketch, "Sketch One".into()),
        ],
        2000.0,
    );
    assert_eq!(crumbs.len(), 3);
    assert_eq!(crumbs[0].2.x, CRUMB_X);
    assert!(crumbs[1].2.x > crumbs[0].2.x + crumbs[0].2.w);
    assert!(crumbs[2].2.x > crumbs[1].2.x + crumbs[1].2.w);
    assert_eq!(crumbs[1].1.chars().count(), CRUMB_MAX_CHARS);
    assert!(crumbs[1].1.ends_with("..."));
    assert!(crumbs[0].2.contains(CRUMB_X + 5.0, CRUMB_Y + 5.0));

    let narrow = layout_breadcrumbs(
        vec![
            (Crumb::Project, "two-parts".into()),
            (Crumb::Part, "Part One".into()),
        ],
        CRUMB_X + 120.0,
    );
    assert_eq!(narrow.len(), 1);
}

#[test]
fn breadcrumbs_follow_selection_and_navigate_up() {
    let mut app = App::with_project(Path::new("fixtures/segmented-primitives.pcad")).unwrap();
    app.solid_preview = false;
    let layout = Layout::new(1280.0, 800.0);
    let kinds = |app: &App| {
        app.breadcrumbs(layout)
            .into_iter()
            .map(|(crumb, _, _)| crumb)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        kinds(&app),
        [Crumb::Projects, Crumb::Project, Crumb::Part, Crumb::Sketch]
    );
    assert_eq!(app.breadcrumbs(layout)[1].1, "segmented-primitives");

    app.document.selected = app.document.shapes.first().map(Shape::id);
    assert!(app.document.selected.is_some());
    assert_eq!(
        kinds(&app),
        [
            Crumb::Projects,
            Crumb::Project,
            Crumb::Part,
            Crumb::Sketch,
            Crumb::Shape
        ]
    );
    let (sketch, shape) = {
        let crumbs = app.breadcrumbs(layout);
        (crumbs[3].2, crumbs[4].2)
    };
    let at = |rect: Rect| Point {
        x: rect.x + 2.0,
        y: rect.y + 2.0,
    };
    assert_eq!(app.breadcrumb_at(layout, at(shape)), None);
    assert_eq!(app.breadcrumb_at(layout, at(sketch)), Some(Crumb::Sketch));
    app.open_breadcrumb(Crumb::Sketch);
    assert_eq!(app.document.selected, None);
    assert_eq!(
        kinds(&app),
        [Crumb::Projects, Crumb::Project, Crumb::Part, Crumb::Sketch]
    );
}

#[test]
fn part_breadcrumb_opens_solid_preview_when_part_has_extrusions() {
    let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    app.solid_preview = false;
    let layout = Layout::new(1280.0, 800.0);
    let crumbs = app.breadcrumbs(layout);
    let (part, sketch) = (crumbs[2].2, crumbs[3].2);
    let at = |rect: Rect| Point {
        x: rect.x + 2.0,
        y: rect.y + 2.0,
    };
    assert_eq!(app.breadcrumb_at(layout, at(sketch)), None);
    assert_eq!(app.breadcrumb_at(layout, at(part)), Some(Crumb::Part));
    app.open_breadcrumb(Crumb::Part);
    assert!(app.solid_preview);
    let crumbs = app.breadcrumbs(layout);
    assert_eq!(crumbs.last().map(|crumb| crumb.0), Some(Crumb::Solid));
}

#[test]
fn projects_crumb_opens_page_listing_the_project_folder() {
    let mut app = App::with_project(Path::new("fixtures/two-parts.pcad")).unwrap();
    let layout = Layout::new(1280.0, 800.0);
    let crumbs = app.breadcrumbs(layout);
    assert_eq!(crumbs[0].0, Crumb::Projects);
    assert_eq!(crumbs[1].0, Crumb::Project);
    let at = Point {
        x: crumbs[0].2.x + 2.0,
        y: crumbs[0].2.y + 2.0,
    };
    assert_eq!(app.breadcrumb_at(layout, at), Some(Crumb::Projects));
    app.open_breadcrumb(Crumb::Projects);

    let page = app.projects_page.as_ref().unwrap();
    assert_eq!(page.dir, Path::new("fixtures").canonicalize().unwrap());
    assert_eq!(page.entries[0].kind, EntryKind::Parent);
    assert_eq!(page.entries[page.selected].name, "two-parts");
    assert!(page.entries.iter().any(|entry| entry.name == "two-parts"));
    let kinds = app
        .breadcrumbs(layout)
        .into_iter()
        .map(|(crumb, _, _)| crumb)
        .collect::<Vec<_>>();
    assert_eq!(kinds, [Crumb::Projects]);
    assert_eq!(app.breadcrumb_at(layout, at), None);

    let row = project_page_row(layout, 0);
    assert_eq!(
        app.projects_page_entry_at(
            layout,
            Point {
                x: row.x + 5.0,
                y: row.y + 5.0
            }
        ),
        Some(0)
    );

    let own = page
        .entries
        .iter()
        .position(|entry| entry.name == "two-parts")
        .unwrap();
    app.open_listed_project(own);
    assert!(app.projects_page.is_none());
    assert_eq!(app.project.as_ref().unwrap().part_name(), "Part One");

    app.open_projects_page();
    app.close_projects_page();
    assert!(app.projects_page.is_none());
}

#[test]
fn projects_page_navigates_folders_and_keeps_selection_in_view() {
    let mut app = App::default();
    app.open_projects_page();
    let fixtures = app
        .projects_page
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .position(|entry| entry.kind == EntryKind::Folder && entry.name == "fixtures")
        .unwrap();
    app.open_listed_project(fixtures);
    let page = app.projects_page.as_ref().unwrap();
    assert!(page.dir.ends_with("fixtures"));
    assert_eq!(page.entries[page.selected].kind, EntryKind::Project);

    app.show_parent_folder();
    let page = app.projects_page.as_ref().unwrap();
    assert_eq!(page.entries[page.selected].name, "fixtures");

    let short = Layout::new(1280.0, 420.0);
    let visible = project_page_visible_rows(short);
    let total = app.projects_page.as_ref().unwrap().entries.len();
    app.select_project_row(total as isize, short);
    let page = app.projects_page.as_ref().unwrap();
    assert_eq!(page.selected, total - 1);
    assert!(page.selected < page.scroll + visible);
    app.select_project_row(-(total as isize), short);
    assert_eq!(app.projects_page.as_ref().unwrap().scroll, 0);
}

#[test]
fn opening_another_project_asks_before_discarding_unsaved_work() {
    let mut app = App::with_project(Path::new("fixtures/two-parts.pcad")).unwrap();
    app.open_projects_page();
    let target = app
        .projects_page
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .position(|entry| entry.name == "segmented-primitives")
        .unwrap();

    app.document.tool = Tool::Rectangle;
    app.document.draft_start = Some(Point { x: 0.0, y: 0.0 });
    assert!(app.document.finish_draft(Point { x: 20.0, y: 10.0 }));
    assert!(app.has_unsaved_work());

    app.open_listed_project(target);
    assert!(app.projects_page.is_some());
    assert!(
        app.project
            .as_ref()
            .unwrap()
            .path()
            .ends_with("two-parts.pcad")
    );
    assert!(app.status_message.contains("Unsaved changes"));

    app.open_listed_project(target);
    assert!(app.projects_page.is_none());
    assert!(
        app.project
            .as_ref()
            .unwrap()
            .path()
            .ends_with("segmented-primitives.pcad")
    );
    assert!(!app.has_unsaved_work());
}

#[test]
fn projection_switch_preserves_camera_and_only_handles_solid_header() {
    let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    let layout = Layout::new(1280.0, 800.0);
    app.solid_view.rotate_drag(12.0, -8.0);
    app.solid_view.pan_drag(10.0, 5.0, 0.5);
    app.solid_view.zoom_wheel(-100.0);
    let original = app.solid_view;
    let button = layout.projection_button(true);
    let point = Point {
        x: button.x + 10.0,
        y: button.y + 10.0,
    };
    assert!(app.select_projection(layout, point));
    assert!(app.solid_view.perspective);
    assert_eq!(
        app.solid_view,
        OrbitView {
            perspective: true,
            ..original
        }
    );
    assert!(
        app.breadcrumbs(layout)
            .iter()
            .all(|(_, _, rect)| rect.x + rect.w < button.x)
    );
    let button = layout.projection_button(false);
    assert!(app.select_projection(
        layout,
        Point {
            x: button.x + 10.0,
            y: button.y + 10.0
        }
    ));
    assert_eq!(app.solid_view, original);
    app.solid_preview = false;
    assert!(!app.select_projection(layout, point));
    app.solid_preview = true;
    app.open_projects_page();
    assert!(!app.select_projection(layout, point));
}

#[test]
fn perspective_foreshortens_and_unprojects_the_picked_surface() {
    let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    let layout = Layout::new(1280.0, 800.0);
    app.solid_view.perspective = true;
    let (scene, projector) = app.solid_projector(layout).unwrap();
    assert!(projector.perspective_factor(10.0) > projector.perspective_factor(-10.0));
    for world in &scene.vertices {
        let screen = projector.screen(*world);
        let model = projector.model_at(
            Point {
                x: screen[0],
                y: screen[1],
            },
            projector.depth(*world),
        );
        let expected = view3d::cad_to_model(view3d::sub(*world, projector.center));
        assert!((0..3).all(|i| (model[i] - expected[i]).abs() < 1e-3));
    }
    let point = Point {
        x: projector.origin[0] + 20.0,
        y: projector.origin[1] + 15.0,
    };
    let pivot = projector.pick_pivot(&scene, point);
    // The fixture is a rectangular prism: a picked point must lie on a face.
    let cad = [
        -pivot[0] + projector.center[0],
        pivot[2] + projector.center[1],
        pivot[1] + projector.center[2],
    ];
    assert!((0..3).any(|axis| {
        let lo = scene
            .vertices
            .iter()
            .map(|p| p[axis])
            .fold(f32::INFINITY, f32::min);
        let hi = scene
            .vertices
            .iter()
            .map(|p| p[axis])
            .fold(f32::NEG_INFINITY, f32::max);
        (cad[axis] - lo).abs() < 1e-3 || (cad[axis] - hi).abs() < 1e-3
    }));
    app.solid_view.set_pivot(pivot);
    app.solid_view.rotate_drag(15.0, -10.0);
    let (_, rotated) = app.solid_projector(layout).unwrap();
    let screen = rotated.screen(cad);
    assert!((screen[0] - point.x).abs() < 1e-3 && (screen[1] - point.y).abs() < 1e-3);
    let before = screen;
    app.solid_view
        .pan_drag(25.0, -12.0, rotated.units_per_pixel());
    let (_, panned) = app.solid_projector(layout).unwrap();
    let after = panned.screen(cad);
    assert!((after[0] - before[0] - 25.0).abs() < 1e-3);
    assert!((after[1] - before[1] + 12.0).abs() < 1e-3);
}

#[test]
fn right_drag_orbits_around_the_surface_under_the_cursor() {
    let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    assert!(app.solid_preview);
    let layout = Layout::new(1280.0, 800.0);
    let (scene, projector) = app.solid_projector(layout).unwrap();
    let center = Point {
        x: layout.viewport.x + layout.viewport.w * 0.5,
        y: layout.viewport.y + layout.viewport.h * 0.5,
    };
    let pivot = projector.pick_pivot(&scene, center);
    let at = projector.view.model_to_view(pivot);
    assert!(
        at[2] > 0.0,
        "the pivot is on the near surface, not the scene center"
    );

    assert!(app.begin_solid_drag(layout, center, MouseButton::Right));
    assert_eq!(app.solid_drag, Some(SolidDrag::Rotate));
    let (_, projector) = app.solid_projector(layout).unwrap();
    let before = projector.screen(projector.center);
    app.solid_view.rotate_drag(30.0, 10.0);
    let (_, rotated) = app.solid_projector(layout).unwrap();
    assert_ne!(rotated.screen(projector.center), before);
    let pivot_screen = |projector: &SolidProjector| {
        let v = projector.view.model_to_view(projector.view.pivot);
        [
            projector.origin[0] + v[0] * projector.scale,
            projector.origin[1] - v[1] * projector.scale,
        ]
    };
    let (a, b) = (pivot_screen(&projector), pivot_screen(&rotated));
    assert!((a[0] - b[0]).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01);
    assert!(
        (a[0] - center.x).abs() < 0.5 && (a[1] - center.y).abs() < 0.5,
        "the pivot stays under the cursor"
    );
    assert_eq!(
        projector.scale, rotated.scale,
        "orbiting never refits the size"
    );
}

#[test]
fn solid_drags_follow_web_buttons() {
    let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    let layout = Layout::new(1280.0, 800.0);
    let inside = Point {
        x: layout.viewport.x + 40.0,
        y: layout.viewport.y + 40.0,
    };
    assert!(!app.begin_solid_drag(layout, inside, MouseButton::Left));
    assert!(app.begin_solid_drag(layout, inside, MouseButton::Middle));
    let Some(SolidDrag::Pan { units_per_pixel }) = app.solid_drag else {
        panic!("middle drag pans");
    };
    let (_, projector) = app.solid_projector(layout).unwrap();
    let corner = projector.center;
    let before = projector.screen(corner);
    app.solid_view.pan_drag(25.0, -12.0, units_per_pixel);
    let (_, panned) = app.solid_projector(layout).unwrap();
    let after = panned.screen(corner);
    assert!((after[0] - before[0] - 25.0).abs() < 0.01);
    assert!((after[1] - before[1] + 12.0).abs() < 0.01);

    assert!(!app.begin_solid_drag(layout, Point { x: 5.0, y: 300.0 }, MouseButton::Right));
    app.solid_preview = false;
    assert!(!app.begin_solid_drag(layout, inside, MouseButton::Right));
    app.solid_preview = true;
    app.open_projects_page();
    assert!(!app.begin_solid_drag(layout, inside, MouseButton::Right));
}

#[test]
fn default_3d_view_shows_top_and_both_positive_sides() {
    let app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    let (_, projector) = app.solid_projector(Layout::new(1280.0, 800.0)).unwrap();
    assert!(projector.facing([0.0, 0.0, 1.0]) > 0.0);
    assert!(projector.facing([1.0, 0.0, 0.0]) > 0.0);
    assert!(projector.facing([0.0, 1.0, 0.0]) > 0.0);
    let top = projector.screen(view3d::add(projector.center, [0.0, 0.0, 10.0]));
    let bottom = projector.screen(view3d::add(projector.center, [0.0, 0.0, -10.0]));
    assert!(top[1] < bottom[1], "CAD +Z is up on screen");
}

#[test]
fn fillet_and_chamfer_prompts_use_defaults_and_modes() {
    let line = |id, a: (f32, f32), b: (f32, f32)| Shape::Line {
        id,
        a: Point { x: a.0, y: a.1 },
        b: Point { x: b.0, y: b.1 },
    };
    let mut app = App {
        document: Document::from_shapes(vec![
            line(1, (40.0, 0.0), (0.0, 0.0)),
            line(2, (0.0, 0.0), (0.0, 40.0)),
            line(3, (100.0, 0.0), (60.0, 0.0)),
            line(4, (60.0, 0.0), (60.0, 40.0)),
        ]),
        ..App::default()
    };
    let hover = |app: &mut App, at: Point| {
        app.cursor = app.document.world_to_screen(at, app.viewport_origin());
    };
    // Shift+W without a hovered second line only explains the gesture.
    app.document.selected = Some(1);
    hover(&mut app, Point { x: 30.0, y: 30.0 });
    app.corner_key(CornerKind::Fillet);
    assert!(app.editing.is_none());
    hover(&mut app, Point { x: 0.0, y: 20.0 });
    app.corner_key(CornerKind::Fillet);
    assert!(matches!(
        app.editing.as_ref().map(|e| e.field),
        Some(EditField::FilletRadius)
    ));
    app.finish_dimension_edit();
    assert!(
        app.document
            .shapes
            .iter()
            .any(|s| matches!(s, Shape::Arc { radius, .. } if *radius == 5.0))
    );

    // MORE -> CHAMFER: click both lines, Tab to distance-angle, keep defaults.
    app.document.selected = None;
    app.start_corner_pick(CornerKind::Chamfer);
    hover(&mut app, Point { x: 80.0, y: 0.0 });
    assert!(app.corner_pick_click());
    assert_eq!(app.document.selected, Some(3));
    hover(&mut app, Point { x: 60.0, y: 20.0 });
    assert!(app.corner_pick_click());
    assert!(matches!(
        app.editing.as_ref().map(|e| e.field),
        Some(EditField::ChamferFirst)
    ));
    app.toggle_chamfer_mode();
    app.editing.as_mut().unwrap().buffer = "4".into();
    app.finish_dimension_edit();
    assert!(matches!(
        app.editing.as_ref().map(|e| e.field),
        Some(EditField::ChamferSecond)
    ));
    app.finish_dimension_edit();
    let chamfer = app.document.selected.unwrap();
    let spec = app.document.chamfers[&chamfer];
    assert_eq!(
        (spec.mode, spec.first, spec.second),
        (ChamferMode::DistanceAngle, 4.0, 45.0)
    );
}

#[test]
fn offset_prompt_creates_and_edits_an_offset() {
    let line = |id, a: (f32, f32), b: (f32, f32)| Shape::Line {
        id,
        a: Point { x: a.0, y: a.1 },
        b: Point { x: b.0, y: b.1 },
    };
    let mut app = App {
        document: Document::from_shapes(vec![
            line(1, (0.0, 0.0), (40.0, 0.0)),
            line(2, (40.0, 0.0), (40.0, 30.0)),
        ]),
        ..App::default()
    };
    // Nothing selected: the command only explains itself.
    app.begin_offset();
    assert!(app.editing.is_none());
    app.document.selected = Some(1);
    app.begin_offset();
    assert!(matches!(
        app.editing.as_ref().map(|e| e.field),
        Some(EditField::OffsetCreate)
    ));
    // Enter keeps the 5 mm default and offsets the whole L chain.
    app.finish_dimension_edit();
    assert_eq!(app.document.offset_chains.len(), 1);
    assert_eq!(app.document.shapes.len(), 4);
    // The driving distance is edited from Properties.
    app.editing = Some(DimensionEdit {
        field: EditField::OffsetDistance,
        buffer: "-2".into(),
    });
    app.finish_dimension_edit();
    assert_eq!(app.document.offset_chains[0].distance, -2.0);
    // A zero distance is rejected without creating anything.
    app.document.selected = Some(1);
    app.begin_offset();
    app.editing.as_mut().unwrap().buffer = "0".into();
    app.finish_dimension_edit();
    assert_eq!(app.document.shapes.len(), 4);
    assert!(app.status_message.contains("non-zero"));
}

#[test]
fn tangent_arc_tool_and_smooth_join_key() {
    let line = |id, a: (f32, f32), b: (f32, f32)| Shape::Line {
        id,
        a: Point { x: a.0, y: a.1 },
        b: Point { x: b.0, y: b.1 },
    };
    let mut app = App {
        document: Document::from_shapes(vec![
            line(1, (0.0, 0.0), (40.0, 0.0)),
            line(2, (41.0, 3.0), (60.0, 20.0)),
        ]),
        ..App::default()
    };
    app.start_tangent_arc();
    assert_eq!(app.document.tool, Tool::TangentArc);
    // A click away from any endpoint is ignored.
    let away = Point { x: 20.0, y: 20.0 };
    app.tangent_arc_click(away, away);
    assert!(app.document.draft_start.is_none());
    let near = Point { x: 1.0, y: 1.0 };
    app.tangent_arc_click(near, near);
    assert_eq!(app.document.tangent_source, Some((1, 0)));
    let end = Point { x: -20.0, y: 20.0 };
    app.tangent_arc_click(end, end);
    let arc = app.document.selected.unwrap();
    assert_eq!(app.document.smooth_joins.get(&(arc, 0)), Some(&(1, 0)));
    // ".": select line 1, hover line 2 near its endpoint.
    app.select_tool(Tool::Select);
    app.document.selected = Some(1);
    app.cursor = app
        .document
        .world_to_screen(Point { x: 50.0, y: 11.5 }, app.viewport_origin());
    app.smooth_join_key();
    assert_eq!(app.document.smooth_joins.get(&(2, 0)), Some(&(1, 1)));
    let Some(Shape::Line { a, b, .. }) = app.document.shapes.iter().find(|s| s.id() == 2) else {
        unreachable!()
    };
    let middle = Point {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    };
    app.cursor = app.document.world_to_screen(middle, app.viewport_origin());
    app.smooth_join_key();
    assert!(!app.document.smooth_joins.contains_key(&(2, 0)));
}

#[test]
fn hovering_picks_the_top_face_and_key_4_sketches_on_it() {
    let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    let layout = app.layout();
    let (scene, projector) = app.solid_projector(layout).unwrap();
    let top = scene
        .vertices
        .iter()
        .map(|v| v[2])
        .fold(f32::NEG_INFINITY, f32::max);
    let cap: Vec<_> = scene
        .vertices
        .iter()
        .filter(|v| (v[2] - top).abs() < 1e-4)
        .collect();
    let center: [f32; 3] =
        std::array::from_fn(|axis| cap.iter().map(|v| v[axis]).sum::<f32>() / cap.len() as f32);
    let screen = projector.screen(center);
    app.cursor = Point {
        x: screen[0],
        y: screen[1],
    };
    let (index, _, hit) = projector.pick_triangle(&scene, app.cursor).unwrap();
    assert!((hit[2] - top).abs() < 1e-3, "ray hits the top cap: {hit:?}");
    let [a, b, c] = scene.triangles[index];
    assert!(triangle_normal(a, b, c).unwrap()[2] > 0.0);
    let (group, outline) = coplanar_face(&scene, index);
    assert!(group.len() >= 2);
    assert!(
        outline
            .iter()
            .all(|[a, b]| (a[2] - top).abs() < 1e-4 && (b[2] - top).abs() < 1e-4)
    );
    let hover = app.face_hover(layout).unwrap();
    assert_eq!(hover.label, "TOP FACE");
    let face = hover.face.clone().unwrap();
    assert!(face.face_id.ends_with("-solid-face-6"));
    app.sketch_on_face();
    assert!(!app.solid_preview, "{}", app.status_message);
    let project = app.project.as_ref().unwrap();
    assert_eq!(project.sketch_face(project.active_sketch()), Some(&face));
    assert_eq!(app.document.reference_edges.len(), outline.len());
}

fn temp_project(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "puppycad-app-{name}-{}-{}.pcad",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn extrude_prompt_previews_a_ghost_and_commits_on_enter() {
    let path = temp_project("draft");
    let mut app = App::with_project(&path).unwrap();
    app.document.tool = Tool::Rectangle;
    app.document.draft_start = Some(Point { x: 0.0, y: 0.0 });
    assert!(app.document.finish_draft(Point { x: 40.0, y: 20.0 }));
    app.begin_extrude("newBody");
    assert!(app.solid_preview);
    let ghost = |app: &App| {
        let g = app.extrude_draft.as_ref().unwrap().ghost.as_ref().unwrap();
        (g.start, g.depth)
    };
    assert_eq!(ghost(&app), (0.0, 50.0));
    assert!(app.project.as_ref().unwrap().extrusion_previews().is_none());
    app.handle_draft_key(KeyCode::Digit2, Some("2"));
    app.handle_draft_key(KeyCode::Digit5, Some("5"));
    assert_eq!(ghost(&app).1, 25.0);
    app.handle_draft_key(KeyCode::KeyE, None);
    assert_eq!(ghost(&app), (-12.5, 25.0));
    app.handle_draft_key(KeyCode::Escape, None);
    assert!(app.extrude_draft.is_none() && !app.solid_preview);
    assert!(app.project.as_ref().unwrap().extrusion_previews().is_none());

    app.begin_extrude("newBody");
    app.handle_draft_key(KeyCode::KeyF, None);
    app.handle_draft_key(KeyCode::Enter, None);
    assert!(app.extrude_draft.is_none() && app.solid_preview);
    let solids = app.project.as_ref().unwrap().extrusion_previews().unwrap();
    assert_eq!((solids[0].start, solids[0].depth), (-50.0, 50.0));
    let _ = std::fs::remove_file(path);
}

#[test]
fn extrusion_option_edits_are_single_undo_steps() {
    let mut app = App::with_project(Path::new("fixtures/extrude-options.pcad")).unwrap();
    let end = |app: &App, index| {
        app.project
            .as_ref()
            .unwrap()
            .extrusion_options_at(index)
            .unwrap()
            .end
    };
    let volume = |app: &App| app.project.as_ref().unwrap().evaluated_solids().unwrap()[0].volume();
    let original = volume(&app);
    app.selected_solid = 1;
    app.cycle_solid_end_condition();
    assert_eq!(
        end(&app, 1),
        project::EndCondition::UpToBody("extrude-1".into())
    );
    assert!(volume(&app) > original);
    app.toggle_solid_flip();
    app.solid_undo_step(false);
    app.solid_undo_step(false);
    assert_eq!(end(&app, 1), project::EndCondition::ThroughAll);
    assert!((volume(&app) - original).abs() < 1e-3);
    app.solid_undo_step(true);
    assert_eq!(
        end(&app, 1),
        project::EndCondition::UpToBody("extrude-1".into())
    );
    app.selected_solid = 0;
    app.set_solid_depth(40.0).unwrap();
    app.solid_undo_step(false);
    assert_eq!(
        app.project.as_ref().unwrap().extrusion_previews().unwrap()[0].depth,
        60.0
    );
}

#[test]
fn armed_body_pick_sets_the_up_to_body_target() {
    let mut app = App::with_project(Path::new("fixtures/extrude-options.pcad")).unwrap();
    let layout = Layout::new(1280.0, 800.0);
    app.selected_solid = 2;
    app.arm_body_pick();
    assert!(app.body_pick);
    let (scene, projector) = app.solid_projector(layout).unwrap();
    let mid = scene.triangles[0];
    let point = projector.screen(std::array::from_fn(|axis| {
        (mid[0][axis] + mid[1][axis] + mid[2][axis]) / 3.0
    }));
    let point = Point {
        x: point[0],
        y: point[1],
    };
    assert_eq!(projector.pick_body(&scene, point), Some(0));
    app.pick_target_body(layout, point);
    assert!(!app.body_pick);
    assert_eq!(app.solid_undo.len(), 1);
}

#[test]
fn hover_redraws_only_when_the_hovered_shape_changes() {
    let mut app = App {
        document: Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 100.0, y: 0.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 60.0 },
                b: Point { x: 100.0, y: 60.0 },
            },
        ]),
        ..App::default()
    };
    let at = |app: &App, x: f32, y: f32| {
        app.document
            .world_to_screen(Point { x, y }, app.viewport_origin())
    };
    let empty = at(&app, 50.0, 30.0);
    app.pointer_moved(empty);
    let base = app.redraw_requests.get();
    // Moving over empty canvas: no repaint.
    app.pointer_moved(at(&app, 52.0, 31.0));
    assert_eq!(app.redraw_requests.get(), base);
    // Entering line 1: exactly one repaint, and it is pre-highlighted.
    app.pointer_moved(at(&app, 40.0, 1.0));
    assert_eq!(app.redraw_requests.get(), base + 1);
    assert_eq!(app.hover.shape, Some(1));
    assert_eq!(app.hovered_ids(), vec![1]);
    // Sliding along the same line: no repaint.
    app.pointer_moved(at(&app, 45.0, 0.5));
    app.pointer_moved(at(&app, 60.0, -1.0));
    assert_eq!(app.redraw_requests.get(), base + 1);
    // Switching to line 2, then leaving it: one repaint each.
    app.pointer_moved(at(&app, 40.0, 60.0));
    assert_eq!(app.redraw_requests.get(), base + 2);
    assert_eq!(app.hover.shape, Some(2));
    app.pointer_moved(empty);
    assert_eq!(app.redraw_requests.get(), base + 3);
    assert_eq!(app.hover.shape, None);
    // Filtered-out geometry is not pre-highlighted.
    app.selection_filter.geometry = false;
    app.pointer_moved(at(&app, 40.0, 1.0));
    assert_eq!(app.hover.shape, None);
    // Placing tools repaint on every move (the draft follows the cursor).
    app.document.tool = Tool::Line;
    let before = app.redraw_requests.get();
    app.pointer_moved(at(&app, 41.0, 1.0));
    assert_eq!(app.redraw_requests.get(), before + 1);
}

#[test]
fn hovering_a_glyph_highlights_partners_and_clicking_selects_its_entity() {
    let mut app = App {
        document: Document::from_shapes(vec![
            Shape::Line {
                id: 1,
                a: Point { x: 0.0, y: 0.0 },
                b: Point { x: 100.0, y: 20.0 },
            },
            Shape::Line {
                id: 2,
                a: Point { x: 0.0, y: 60.0 },
                b: Point { x: 100.0, y: 70.0 },
            },
        ]),
        ..App::default()
    };
    app.document.selected = Some(2);
    app.document
        .toggle_line_relation_with(1, LineRelationKind::Parallel)
        .unwrap();
    app.document.clear_selection();
    let origin = app.viewport_origin();
    let glyph = app
        .document
        .placed_glyphs(origin)
        .into_iter()
        .find(|g| g.glyph.owner == 1)
        .unwrap();
    assert_eq!(glyph.glyph.label, "//");
    let inside = Point {
        x: glyph.x + 3.0,
        y: glyph.y + 3.0,
    };
    app.pointer_moved(inside);
    let mut partners = app.hovered_ids();
    partners.sort();
    assert_eq!(partners, vec![1, 2]);
    app.handle_left_press();
    assert_eq!(app.document.selected, Some(1));
    assert!(!app.document.dragging());
    // Key 6 hides the glyphs: no hit, no hover.
    app.toggle_glyphs();
    assert!(!app.show_glyphs);
    app.pointer_moved(inside);
    assert!(app.hover.glyph.is_none());
}

#[test]
fn line_commit_adds_horizontal_only_with_automatic_constraints() {
    let mut app = App::default();
    let at = |app: &App, x: f32, y: f32| {
        app.document
            .world_to_screen(Point { x, y }, app.viewport_origin())
    };
    app.snap_grid = false;
    app.snap_geometry = false;
    app.document.tool = Tool::Line;
    app.cursor = at(&app, 3.0, 3.0);
    app.handle_left_press();
    app.pointer_moved(at(&app, 103.0, 4.5));
    assert_eq!(
        app.placing_inference().map(|i| i.kind),
        Some(model::InferenceKind::Horizontal)
    );
    app.handle_left_press();
    assert_eq!(app.document.axis_constraints.len(), 1);
    app.toggle_auto_constraints();
    assert!(!app.auto_constraints);
    app.cursor = at(&app, 3.0, 40.0);
    app.handle_left_press();
    app.cursor = at(&app, 103.0, 41.5);
    app.handle_left_press();
    assert_eq!(app.document.shapes.len(), 2);
    assert_eq!(app.document.axis_constraints.len(), 1);
}

#[test]
fn registry_has_unique_ids_and_no_duplicate_shortcuts() {
    let mut seen: Vec<(String, Scope, &str)> = Vec::new();
    let mut ids = std::collections::HashSet::new();
    let mut duplicates = Vec::new();
    for command in COMMANDS {
        assert!(ids.insert(command.id), "{:?} registered twice", command.id);
        assert!(!command.label.is_empty());
        for chord in shortcut_chords(command.shortcut) {
            if let Some((_, _, other)) = seen.iter().find(|(seen_chord, scope, _)| {
                *seen_chord == chord && scope.overlaps(command.scope)
            }) {
                duplicates.push(format!("{chord}: {other} / {}", command.label));
            }
            seen.push((chord, command.scope, command.label));
        }
    }
    assert!(duplicates.is_empty(), "duplicate shortcuts: {duplicates:?}");
    assert_eq!(
        shortcut_chords("Ctrl+Shift+E"),
        shortcut_chords("Shift+Ctrl+E")
    );
    assert_eq!(shortcut_chords("Ctrl+Y / Ctrl+Shift+Z").len(), 2);
}

#[test]
fn every_more_item_maps_to_a_registered_command() {
    let layout = Layout::new(1280.0, 800.0);
    for (index, (id, label)) in MORE_MENU.iter().enumerate() {
        assert_eq!(command(*id).id, *id, "{label}");
        let row = layout.more_item(index);
        assert!(row.y + row.h <= 800.0 && row.x >= 0.0, "{label}: {row:?}");
    }
    let mut app = App {
        more_open: true,
        ..App::default()
    };
    let row = layout.more_item(0);
    app.cursor = Point {
        x: row.x + 5.0,
        y: row.y + 5.0,
    };
    app.handle_left_press();
    assert_eq!(app.document.tool, Tool::Point);
    assert!(!app.more_open);
}

#[test]
fn key_resolution_respects_modifiers_and_context() {
    let mut app = App::default();
    assert_eq!(app.command_for_key(KeyCode::KeyR), Some(Cmd::Rectangle));
    assert_eq!(app.command_for_key(KeyCode::KeyW), None);
    app.modifiers = ModifiersState::SHIFT;
    assert_eq!(
        app.command_for_key(KeyCode::KeyR),
        Some(Cmd::CenterRectangle)
    );
    assert_eq!(app.command_for_key(KeyCode::KeyW), Some(Cmd::FilletCorner));
    assert_eq!(app.command_for_key(KeyCode::KeyL), Some(Cmd::Collinear));
    app.modifiers = ModifiersState::CONTROL;
    assert_eq!(app.command_for_key(KeyCode::KeyS), Some(Cmd::Save));
    assert_eq!(app.command_for_key(KeyCode::KeyZ), Some(Cmd::Undo));
    app.modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
    assert_eq!(app.command_for_key(KeyCode::KeyZ), Some(Cmd::Redo));
    assert_eq!(
        app.command_for_key(KeyCode::KeyE),
        Some(Cmd::ExportAsciiStl)
    );
    app.modifiers = ModifiersState::empty();
    app.select_tool(Tool::CenterArc);
    assert_eq!(app.command_for_key(KeyCode::KeyW), Some(Cmd::ArcDirection));
    app.run_command(Cmd::ArcDirection);
    assert!(app.status_message.contains("sweep"));
    app.run_command(Cmd::Circle);
    assert_eq!(app.document.tool, Tool::Circle);
}

#[test]
fn palette_filters_and_orders_matches() {
    assert_eq!(palette_matches("").len(), COMMANDS.len());
    let labels = |query| -> Vec<&str> {
        palette_matches(query)
            .into_iter()
            .map(|command| command.label)
            .collect()
    };
    assert_eq!(labels("mirr")[0], "Mirror");
    assert_eq!(labels("MIRR")[0], "Mirror");
    // Label prefix beats a word inside the label, ties keep registry order.
    let circle = labels("circle");
    assert_eq!(&circle[..2], ["Circle", "3-point circle"]);
    let ext = labels("ext");
    assert_eq!(&ext[..2], ["Extend curve to cursor", "Extrude new body"]);
    // Category and shortcut text match too, after label matches.
    assert_eq!(labels("ctrl+s")[0], "Save project");
    assert!(labels("constrain").contains(&"Horizontal"));
    // Subsequence fallback.
    assert_eq!(labels("ctrspl")[0], "Control spline");
    assert!(labels("zzzz").is_empty());
}

#[test]
fn palette_skips_unavailable_rows_and_enter_runs_selection() {
    let mut app = App {
        modifiers: ModifiersState::CONTROL,
        ..App::default()
    };
    assert_eq!(
        app.command_for_key(KeyCode::KeyK),
        Some(Cmd::CommandPalette)
    );
    app.modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
    assert_eq!(
        app.command_for_key(KeyCode::KeyP),
        Some(Cmd::CommandPalette)
    );
    app.modifiers = ModifiersState::empty();
    app.run_command(Cmd::CommandPalette);
    assert!(app.palette.is_some());
    // In the sketch view the solid cycling rows are dimmed and skipped.
    app.palette_type("solid");
    let entries = app.palette_entries();
    assert_eq!(entries[0].0.id, Cmd::PreviousSolid);
    assert!(!entries[0].1 && !entries[1].1);
    let selected = app.palette.as_ref().unwrap().selected;
    assert!(selected >= 2 && entries[selected].1);
    app.palette_move(-1);
    assert_eq!(app.palette.as_ref().unwrap().selected, selected);
    app.palette_move(1);
    assert!(app.palette.as_ref().unwrap().selected > selected);
    // Esc closes without running anything.
    assert!(app.palette_key(KeyCode::Escape, None));
    assert!(app.palette.is_none());
    assert!(!app.palette_key(KeyCode::Escape, None));

    // Typed text filters; Enter runs the selected command.
    app.open_palette();
    app.palette_key(KeyCode::KeyC, Some("circ"));
    app.palette_key(KeyCode::KeyX, Some("x"));
    app.palette_key(KeyCode::Backspace, None);
    assert_eq!(app.palette.as_ref().unwrap().query, "circ");
    app.palette_key(KeyCode::ArrowDown, None);
    app.palette_key(KeyCode::Enter, None);
    assert!(app.palette.is_none());
    assert_eq!(app.document.tool, Tool::ThreePointCircle);

    // Mirror needs a selection: dimmed without one, runs with one.
    let mut app = App {
        document: Document::from_shapes(vec![Shape::Line {
            id: 1,
            a: Point { x: 0.0, y: 0.0 },
            b: Point { x: 10.0, y: 0.0 },
        }]),
        ..App::default()
    };
    app.open_palette();
    app.palette_type("mirr");
    assert!(!app.palette_entries()[0].1);
    app.palette = None;
    app.document.selected = Some(1);
    app.open_palette();
    app.palette_type("mirr");
    assert!(app.palette_entries()[0].1);
    app.palette_run();
    assert!(matches!(app.pick, Some(Pick::Mirror { .. })));
}

#[test]
fn shortcut_overlay_toggles_and_swallows_keys() {
    let mut app = App {
        modifiers: ModifiersState::SHIFT,
        ..App::default()
    };
    assert_eq!(
        app.command_for_key(KeyCode::Slash),
        Some(Cmd::ShortcutOverlay)
    );
    app.modifiers = ModifiersState::empty();
    assert_eq!(app.command_for_key(KeyCode::F1), Some(Cmd::ShortcutOverlay));
    app.run_command(Cmd::ShortcutOverlay);
    assert!(app.shortcut_overlay);
    // Other keys do nothing while it is shown.
    assert!(app.shortcut_overlay_key(KeyCode::KeyC));
    assert!(app.shortcut_overlay);
    assert_ne!(app.document.tool, Tool::Circle);
    assert!(app.shortcut_overlay_key(KeyCode::F1));
    assert!(!app.shortcut_overlay);
    assert!(!app.shortcut_overlay_key(KeyCode::F1));
    app.run_command(Cmd::ShortcutOverlay);
    app.modifiers = ModifiersState::SHIFT;
    app.shortcut_overlay_key(KeyCode::Slash);
    assert!(!app.shortcut_overlay);
    app.run_command(Cmd::ShortcutOverlay);
    app.shortcut_overlay_key(KeyCode::Escape);
    assert!(!app.shortcut_overlay);
    // The palette and the overlay replace each other.
    app.run_command(Cmd::CommandPalette);
    app.run_command(Cmd::ShortcutOverlay);
    assert!(app.palette.is_none() && app.shortcut_overlay);
    app.run_command(Cmd::CommandPalette);
    assert!(app.palette.is_some() && !app.shortcut_overlay);
}

#[test]
fn shortcut_overlay_lists_every_shortcut_and_fits_the_window() {
    for (width, height) in [(1280.0, 800.0), (1600.0, 900.0), (1920.0, 1080.0)] {
        let lines = shortcut_overlay_lines(width, height);
        let column = overlay_column_width(width);
        for line in &lines {
            let chars = match &line.text {
                OverlayText::Header(name) => name.len(),
                OverlayText::Entry { keys, label } => 15 + label.chars().count().max(keys.len()),
            };
            assert!(
                line.x >= 0.0 && line.x + chars as f32 * 8.0 <= width,
                "{line:?} at {width}"
            );
            assert!(line.y + 8.0 <= height, "{line:?} at {height}");
            assert!(chars as f32 * 8.0 <= column, "{line:?} wider than a column");
        }
        for category in Category::ALL {
            assert!(
                lines
                    .iter()
                    .any(|line| line.text == OverlayText::Header(category.name())),
                "{category:?}"
            );
        }
        // Each shortcut command shows its label once.
        for command in COMMANDS
            .iter()
            .filter(|command| !command.shortcut.is_empty())
        {
            let shown = lines.iter().any(|line| match &line.text {
                OverlayText::Entry { label, .. } => {
                    command.label.starts_with(label.trim_end_matches("...")) && !label.is_empty()
                }
                OverlayText::Header(_) => false,
            });
            assert!(shown, "{} missing", command.label);
        }
        // A header is never the last line of a column.
        for pair in lines.windows(2) {
            if matches!(pair[0].text, OverlayText::Header(_)) {
                assert_eq!(pair[0].x, pair[1].x);
            }
        }
    }
    // At 1280 px the sheet wraps into several columns.
    let columns = |width: f32| {
        let mut xs: Vec<i32> = shortcut_overlay_lines(width, 800.0)
            .iter()
            .map(|line| line.x as i32)
            .collect();
        xs.dedup();
        xs.len()
    };
    assert!(columns(1280.0) >= 2);
    assert!(columns(1280.0) <= 4);
}

#[test]
fn solid_and_view_keys_resolve_through_the_registry() {
    let mut app = App::default();
    assert_eq!(app.command_for_key(KeyCode::KeyE), Some(Cmd::Ellipse));
    assert_eq!(app.command_for_key(KeyCode::KeyF), Some(Cmd::FitView));
    assert_eq!(
        app.command_for_key(KeyCode::Digit6),
        Some(Cmd::ToggleGlyphs)
    );
    assert_eq!(
        app.command_for_key(KeyCode::Digit7),
        Some(Cmd::ToggleAutoConstraints)
    );
    app.solid_preview = true;
    assert_eq!(
        app.command_for_key(KeyCode::KeyE),
        Some(Cmd::SolidEndCondition)
    );
    assert_eq!(app.command_for_key(KeyCode::KeyF), Some(Cmd::SolidFlip));
    assert_eq!(app.command_for_key(KeyCode::KeyU), Some(Cmd::BodyPick));
    app.modifiers = ModifiersState::CONTROL;
    assert_eq!(app.command_for_key(KeyCode::KeyY), Some(Cmd::Redo));
    app.modifiers = ModifiersState::empty();
    app.body_pick = true;
    assert_eq!(
        app.command_for_key(KeyCode::Escape),
        Some(Cmd::CancelBodyPick)
    );
    app.run_command(Cmd::CancelBodyPick);
    assert!(!app.body_pick);
    let glyphs = app.show_glyphs;
    app.run_command(Cmd::ToggleGlyphs);
    assert_ne!(app.show_glyphs, glyphs);
}
