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
