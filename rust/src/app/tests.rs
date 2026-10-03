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
