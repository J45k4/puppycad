//! NEXT quick actions: rule table, availability, Ctrl+n and Finish sketch.

use super::input::KeyPress;
use super::*;

fn key_with(app: &mut App, modifiers: ModifiersState, code: KeyCode) {
    app.modifiers = modifiers;
    app.handle_key(KeyPress {
        physical_key: PhysicalKey::Code(code),
        text: None,
    });
    app.modifiers = ModifiersState::empty();
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "puppycad-next-{name}-{}-{}.pcad",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn draw_rectangle(app: &mut App) {
    app.document.tool = Tool::Rectangle;
    app.document.draft_start = Some(Point { x: 0.0, y: 0.0 });
    assert!(app.document.finish_draft(Point { x: 40.0, y: 20.0 }));
    app.document.tool = Tool::Select;
    app.document.clear_selection();
}

fn top(app: &App) -> Cmd {
    app.quick_suggestions()[0].cmd
}

fn assert_all_available(app: &App) {
    let items = app.quick_suggestions();
    assert!((3..=QUICK_MAX_ROWS).contains(&items.len()), "{items:?}");
    for item in items.iter() {
        assert!(
            app.command_available(command(item.cmd)),
            "{:?} suggested but unavailable",
            item.cmd
        );
        assert!(!item.reason.is_empty());
    }
}

fn block_app(name: &str) -> (App, PathBuf) {
    let path = temp_path(name);
    let mut app = App::with_project(&path).unwrap();
    draw_rectangle(&mut app);
    app.project
        .as_mut()
        .unwrap()
        .create_extrusion_for_profile(&app.document, 10.0, 0)
        .unwrap();
    app.solid_preview = true;
    (app, path)
}

#[test]
fn empty_sketch_suggests_drawing_a_rectangle() {
    let path = temp_path("empty");
    let app = App::with_project(&path).unwrap();
    assert_eq!(top(&app), Cmd::Rectangle);
    assert_eq!(app.quick_suggestions()[0].label, "Draw rectangle");
    assert_all_available(&app);
    let _ = std::fs::remove_file(path);
}

#[test]
fn closed_rectangle_suggests_finish_sketch_and_open_geometry_does_not_lead_with_it() {
    let path = temp_path("closed");
    let mut app = App::with_project(&path).unwrap();
    draw_rectangle(&mut app);
    assert_eq!(top(&app), Cmd::FinishSketch);
    let items = app.quick_suggestions();
    assert!(items.iter().any(|item| item.cmd == Cmd::ExtrudeNew));
    assert!(
        items
            .iter()
            .any(|item| item.cmd == Cmd::DimensionTool && item.label.contains("DOF remaining"))
    );
    assert_all_available(&app);

    let mut open = App::with_project(&temp_path("open")).unwrap();
    open.document.tool = Tool::Line;
    open.document.draft_start = Some(Point { x: 0.0, y: 0.0 });
    assert!(open.document.finish_draft(Point { x: 40.0, y: 0.0 }));
    open.document.tool = Tool::Select;
    open.document.draft_start = None;
    open.document.clear_selection();
    assert_eq!(top(&open), Cmd::Line);
    assert_all_available(&open);
    let _ = std::fs::remove_file(path);
}

#[test]
fn finish_sketch_with_profile_and_no_body_opens_the_extrude_draft() {
    let path = temp_path("finish-draft");
    let mut app = App::with_project(&path).unwrap();
    draw_rectangle(&mut app);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::Enter);
    assert!(app.extrude_draft.is_some() && app.solid_preview);
    // Draft open: Commit leads, and Ctrl+1 commits.
    assert_eq!(top(&app), Cmd::DraftCommit);
    let labels: Vec<String> = app
        .quick_suggestions()
        .iter()
        .map(|item| item.label.clone())
        .collect();
    assert!(labels.starts_with(&["Commit".into(), "Flip".into(), "End condition".into()]));
    assert!(labels.contains(&"Cancel".to_string()));
    assert_all_available(&app);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::Digit1);
    assert!(app.extrude_draft.is_none() && app.solid_preview);
    assert_eq!(
        app.project
            .as_ref()
            .unwrap()
            .extrusion_previews()
            .unwrap()
            .len(),
        1
    );
    assert_all_available(&app);
    let _ = std::fs::remove_file(path);
}

#[test]
fn finish_sketch_without_profile_stays_and_explains() {
    let path = temp_path("finish-empty");
    let mut app = App::with_project(&path).unwrap();
    app.run_command(Cmd::FinishSketch);
    assert!(!app.solid_preview && app.extrude_draft.is_none());
    assert!(
        app.status_message
            .starts_with("Sketch has no closed profile to extrude"),
        "{}",
        app.status_message
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn finish_sketch_with_a_body_returns_to_3d() {
    let (mut app, path) = block_app("finish-body");
    key_with(&mut app, ModifiersState::empty(), KeyCode::Tab);
    assert!(!app.solid_preview);
    // The extruded sketch is locked: Finish sketch leads.
    assert_eq!(top(&app), Cmd::FinishSketch);
    assert_all_available(&app);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::Enter);
    assert!(app.solid_preview && app.extrude_draft.is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn ctrl_n_runs_the_nth_row() {
    let path = temp_path("ctrl-n");
    let mut app = App::with_project(&path).unwrap();
    let items = app.quick_suggestions();
    assert_eq!(items[1].cmd, Cmd::Circle);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::Digit2);
    assert_eq!(app.document.tool, Tool::Circle);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::Digit3);
    assert_eq!(app.document.tool, Tool::Line);
    // Ctrl+1 is not the plain 1 (new sketch on TOP).
    let sketches = app.sketch_count();
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::Digit1);
    assert_eq!(app.document.tool, Tool::Rectangle);
    assert_eq!(app.sketch_count(), sketches);
    let _ = std::fs::remove_file(path);
}

#[test]
fn clicking_a_next_row_runs_it_and_the_toggle_hides_the_band() {
    let path = temp_path("click");
    let mut app = App::with_project(&path).unwrap();
    let layout = app.layout();
    let (band, rows) = app.quick_band(layout).unwrap();
    // The default 800 px window is short: four rows.
    assert_eq!(rows, 4);
    assert!(band.y + band.h <= layout.right.y + layout.right.h + 0.5);
    let row = App::quick_row(band, 1);
    app.cursor = Point {
        x: row.x + 20.0,
        y: row.y + 8.0,
    };
    app.handle_left_press();
    assert_eq!(app.document.tool, Tool::Circle);
    app.run_command(Cmd::ToggleQuickActions);
    assert!(!app.settings.quick_actions);
    assert!(app.quick_band(layout).is_none());
    let _ = std::fs::remove_file(path);
}

#[test]
fn face_and_edge_selections_lead_with_sketch_on_face_and_chamfer() {
    let (mut app, path) = block_app("topo");
    assert_all_available(&app);
    let layout = app.layout();
    let (_, _, hash, topology) = app.topology_context(layout).unwrap();
    let face = topology
        .faces
        .iter()
        .position(|face| face.normal[2] > 0.99)
        .unwrap();
    app.topo_key = hash;
    app.topo_selection = vec![TopoPick {
        element: TopoRef::Face(face),
        point: [0.0; 3],
        triangle: Some(topology.faces[face].triangles[0]),
    }];
    assert_eq!(top(&app), Cmd::SketchOnFace);
    assert_all_available(&app);
    app.topo_selection = vec![TopoPick {
        element: TopoRef::Edge(0),
        point: [0.0; 3],
        triangle: None,
    }];
    assert_eq!(top(&app), Cmd::EdgeChamfer);
    assert!(
        app.quick_suggestions()
            .iter()
            .any(|item| item.cmd == Cmd::EdgeFillet)
    );
    assert_all_available(&app);
    let _ = std::fs::remove_file(path);
}

#[test]
fn read_only_project_leads_with_open_project() {
    let path = temp_path("read-only");
    let (mut project, document) = ProjectFile::open(&path).unwrap();
    project.save(&document).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let sketch = value["items"][0]["data"]["cad"]["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|node| node["type"] == "sketch")
        .unwrap();
    sketch["dimensions"] = serde_json::json!([{"id": "legacy"}]);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let app = App::with_project(&path).unwrap();
    assert!(app.read_only());
    assert_eq!(top(&app), Cmd::OpenProject);
    assert!(app.quick_suggestions()[0].reason.starts_with("Read only: "));
    assert_all_available(&app);
    let _ = std::fs::remove_file(path);
}

#[test]
fn assembly_and_dimension_states_offer_their_commands() {
    let path = temp_path("dimension");
    let mut app = App::with_project(&path).unwrap();
    draw_rectangle(&mut app);
    app.run_command(Cmd::AssemblyView);
    if app.assembly.is_some() {
        assert_eq!(top(&app), Cmd::InsertInstance);
        assert_all_available(&app);
    }
    let _ = std::fs::remove_file(path);
}

/// Small primitives (rows, glyph pixels) drawn inside the Properties column
/// whose bottom reaches below `bottom`.
fn props_rows_below(app: &App, layout: Layout, bottom: f32) -> usize {
    let mut painter = Painter::new(layout.top.w, layout.bottom.y + layout.bottom.h);
    painter.set_scroll_region(App::props_region(layout));
    app.paint_panels(&mut painter, layout);
    app.props_extent.set(painter.scroll_extent());
    painter
        .logical_triangles()
        .iter()
        .filter(|tri| {
            let min_x = tri.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
            let min_y = tri.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min);
            let max_y = tri.iter().map(|p| p[1]).fold(f32::NEG_INFINITY, f32::max);
            min_x > layout.right.x + 2.0
                && max_y - min_y < 40.0
                && max_y > bottom + 0.5
                && min_y < layout.right.y + layout.right.h
        })
        .count()
}

fn selected_rectangle_app(name: &str) -> (App, PathBuf) {
    let path = temp_path(name);
    let mut app = App::with_project(&path).unwrap();
    draw_rectangle(&mut app);
    let id = app.document.shapes[0].id();
    app.document.selected = Some(id);
    (app, path)
}

#[test]
fn next_band_never_overlaps_a_visible_properties_row() {
    let (mut app, path) = selected_rectangle_app("overlap");
    let layout = app.layout();
    let (band, _) = app.quick_band(layout).unwrap();
    assert_eq!(band.y, layout.props_bottom);
    // Fixed height: header + 4 rows on the 800 px window.
    assert_eq!(band.h, App::quick_band_height(layout));
    assert_eq!(props_rows_below(&app, layout, band.y), 0);
    // The transform rows below the band are reachable by scrolling.
    let max = app.max_props_scroll(app.layout());
    assert!(max > 100.0, "{max}");
    app.scroll_props(100.0);
    let scrolled = app.layout();
    assert_eq!(scrolled.props_scroll, max);
    assert_eq!(props_rows_below(&app, scrolled, band.y), 0);
    // Top rows stay put; a click on the scrolled part maps to its row.
    let top = Point {
        x: layout.right.x + 50.0,
        y: layout.right.y + 120.0,
    };
    assert_eq!(App::props_point(scrolled, top), top);
    let lower = Point {
        x: top.x,
        y: layout.right.y + 400.0,
    };
    assert_eq!(App::props_point(scrolled, lower).y, lower.y + max);
    // Hidden NEXT: Properties gets the whole column again.
    app.settings.quick_actions = false;
    let hidden = app.layout();
    assert_eq!(hidden.props_bottom, hidden.right.y + hidden.right.h);
    let _ = std::fs::remove_file(path);
}

#[test]
fn properties_scroll_keeps_the_edited_row_visible() {
    let (mut app, path) = selected_rectangle_app("follow");
    let layout = app.layout();
    props_rows_below(&app, layout, layout.props_bottom);
    for field in [EditField::ScaleFactor, EditField::MoveX, EditField::CenterY] {
        app.editing = Some(DimensionEdit {
            field,
            buffer: String::new(),
        });
        app.follow_props_edit();
        let layout = app.layout();
        let top = app.edit_field_top(layout, field).unwrap() - layout.props_scroll;
        assert!(top >= layout.right.y + PROPS_SCROLL_TOP);
        assert!(top + 25.0 <= layout.props_bottom, "{top}");
    }
    let _ = std::fs::remove_file(path);
}

fn click_at(app: &mut App, x: f32, y: f32) {
    app.pointer_moved(Point { x, y });
    app.handle_left_press();
    app.document.finish_drag();
    let _ = app.paint();
}

/// The UI smoke "author splines" steps, headless, with NEXT shown.
#[test]
fn smoke_author_splines_sequence_places_both_splines() {
    let path = temp_path("splines");
    let mut app = App::with_project(&path).unwrap();
    assert!(app.settings.quick_actions);
    key_with(&mut app, ModifiersState::SHIFT, KeyCode::KeyG);
    click_at(&mut app, 1150.0, 63.0);
    click_at(&mut app, 1150.0, 508.0);
    assert_eq!(app.document.tool, Tool::FitSpline);
    for (x, y) in [
        (420.0, 330.0),
        (520.0, 330.0),
        (520.0, 430.0),
        (420.0, 430.0),
    ] {
        click_at(&mut app, x, y);
    }
    key_with(&mut app, ModifiersState::SHIFT, KeyCode::Enter);
    click_at(&mut app, 1150.0, 63.0);
    click_at(&mut app, 1150.0, 544.0);
    assert_eq!(app.document.tool, Tool::ControlSpline);
    for (x, y) in [
        (690.0, 380.0),
        (730.0, 300.0),
        (790.0, 460.0),
        (840.0, 380.0),
    ] {
        click_at(&mut app, x, y);
    }
    assert!(
        app.status_message.contains("4 points"),
        "{}",
        app.status_message
    );
    // Profile detection is cached per revision (paint and NEXT share it).
    assert!(std::rc::Rc::ptr_eq(
        &app.sketch_profiles(),
        &app.sketch_profiles()
    ));
    key_with(&mut app, ModifiersState::empty(), KeyCode::Enter);
    let splines = app
        .document
        .shapes
        .iter()
        .filter(|s| matches!(s, Shape::Spline { .. }))
        .count();
    assert_eq!(splines, 2, "{}", app.status_message);
    let _ = std::fs::remove_file(path);
}

fn face_hover(label: &str) -> Option<FaceHover> {
    Some(FaceHover {
        face: None,
        label: label.into(),
        outline: Vec::new(),
    })
}

#[test]
fn hovering_in_3d_updates_row_one_once_per_hover_change() {
    let (mut app, path) = block_app("hover");
    let first = app.quick_suggestions();
    assert_eq!(first[0].cmd, Cmd::SketchOnFace);
    assert_eq!(first[0].reason, "hover a face first");
    let builds = app.quick_builds.get();
    app.hovered_face = face_hover("Top face");
    app.topo_hover = Some(TopoRef::Face(0));
    let items = app.quick_suggestions();
    assert_eq!(items[0].reason, "Sketch on face - Top face");
    assert_eq!(app.quick_builds.get(), builds + 1);
    // More pointer moves over the same face: no rebuild.
    for _ in 0..5 {
        app.hovered_face = face_hover("Top face");
        let _ = app.quick_suggestions();
    }
    assert_eq!(app.quick_builds.get(), builds + 1);
    // Another face: exactly one more rebuild.
    app.hovered_face = face_hover("Side face");
    app.topo_hover = Some(TopoRef::Face(1));
    assert_eq!(
        app.quick_suggestions()[0].reason,
        "Sketch on face - Side face"
    );
    let _ = app.quick_suggestions();
    assert_eq!(app.quick_builds.get(), builds + 2);
    // Hovered edge: rebuild once; Chamfer leads when it can run.
    app.hovered_face = None;
    app.topo_hover = Some(TopoRef::Edge(0));
    let items = app.quick_suggestions();
    assert_eq!(app.quick_builds.get(), builds + 3);
    if app.command_available(command(Cmd::EdgeChamfer)) {
        assert_eq!(items[0].cmd, Cmd::EdgeChamfer);
        assert!(items[0].reason.starts_with("1 edge hovered"));
    }
    let _ = std::fs::remove_file(path);
}

#[test]
fn feature_and_dimension_states_offer_edit_and_reorder_rows() {
    let (mut app, path) = block_app("rows");
    app.settings.quick_actions = false;
    let items = app.quick_suggestions();
    let edit = items
        .iter()
        .find(|item| item.cmd == Cmd::EditFeatureValue)
        .expect("Edit depth row");
    assert_eq!(edit.label, "Edit depth");
    assert!(!items.iter().any(|item| item.cmd == Cmd::FeatureUp));
    assert_all_available(&app);
    app.run_command(Cmd::EditFeatureValue);
    assert!(matches!(
        app.editing.as_ref().map(|edit| edit.field),
        Some(EditField::ExtrusionDepth)
    ));
    app.editing = None;
    // A second feature: the reorder row appears with its Ctrl+Up chip.
    app.solid_preview = false;
    app.run_command(Cmd::SketchFront);
    draw_rectangle(&mut app);
    app.project
        .as_mut()
        .unwrap()
        .create_extrusion_for_profile(&app.document, 5.0, 0)
        .unwrap();
    app.solid_preview = true;
    let items = app.quick_suggestions();
    let reorder = items
        .iter()
        .find(|item| item.cmd == Cmd::FeatureUp)
        .expect("Reorder row");
    assert_eq!(reorder.label, "Reorder feature");
    assert_eq!(reorder.shortcut, "Ctrl+Up");
    let _ = std::fs::remove_file(path);
    // Sketch with a selected dimension: Edit dimension value first.
    let path = temp_path("dim-edit");
    let mut app = App::with_project(&path).unwrap();
    draw_rectangle(&mut app);
    let id = app.document.shapes[0].id();
    app.selected_dimension = Some(DimensionKey::Field(id, DimensionField::RectangleWidth));
    let items = app.quick_suggestions();
    assert_eq!(items[0].cmd, Cmd::EditDimensionValue);
    assert_eq!(items[0].label, "Edit dimension value");
    app.run_quick_action(0);
    assert!(app.editing.is_some());
    for item in items.iter() {
        assert!(!item.label.contains('—') && !item.reason.contains('—'));
    }
    let _ = std::fs::remove_file(path);
}
