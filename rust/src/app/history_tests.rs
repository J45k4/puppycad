//! Combined undo history and the HISTORY panel (`history.rs`).

use super::history::StepSide;
use super::input::KeyPress;
use super::*;
use winit::dpi::PhysicalPosition;
use winit::event::DeviceId;

fn click(app: &mut App, point: Point) {
    app.handle_window_event(WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(point.x as f64, point.y as f64),
    });
    for state in [ElementState::Pressed, ElementState::Released] {
        app.handle_window_event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: MouseButton::Left,
        });
    }
}

fn ctrl_key(app: &mut App, shift: bool, code: KeyCode) {
    let previous = app.modifiers;
    app.modifiers = if shift {
        ModifiersState::CONTROL | ModifiersState::SHIFT
    } else {
        ModifiersState::CONTROL
    };
    app.handle_key(KeyPress {
        physical_key: PhysicalKey::Code(code),
        text: None,
    });
    app.modifiers = previous;
}

fn draw_line(app: &mut App, y: f32) {
    app.document.tool = Tool::Line;
    app.document.draft_start = Some(Point { x: 0.0, y });
    assert!(app.document.finish_draft(Point { x: 40.0, y }));
}

fn labels(app: &App) -> Vec<String> {
    app.history_timeline()
        .into_iter()
        .map(|step| step.label)
        .collect()
}

fn depth(app: &App, index: usize) -> f32 {
    app.project.as_ref().unwrap().extrusion_previews().unwrap()[index].depth
}

#[test]
fn ctrl_shift_h_toggles_the_history_panel() {
    let mut app = App::default();
    ctrl_key(&mut app, true, KeyCode::KeyH);
    assert!(app.history_open);
    ctrl_key(&mut app, true, KeyCode::KeyH);
    assert!(!app.history_open);
}

#[test]
fn panel_click_undoes_to_and_redoes_to_a_step() {
    let mut app = App::default();
    for y in [0.0, 20.0, 40.0] {
        draw_line(&mut app, y);
    }
    app.history_open = true;
    let layout = app.layout();
    let rows = app.history_rows(layout);
    let row_labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(row_labels, ["Draw line", "Draw line", "Draw line", "Start"]);
    assert!(rows[0].current);
    assert_eq!(rows[0].side, Some(StepSide::Sketch));
    // The second step from the bottom: undo back to two lines.
    let second = rows[1].rect;
    click(
        &mut app,
        Point {
            x: second.x + 10.0,
            y: second.y + 5.0,
        },
    );
    assert_eq!(app.document.shapes.len(), 2);
    let rows = app.history_rows(layout);
    assert!(rows[1].current);
    assert!(rows[0].redo);
    // Start undoes everything; the top (redo) row redoes everything.
    let start = rows[3].rect;
    click(
        &mut app,
        Point {
            x: start.x + 10.0,
            y: start.y + 5.0,
        },
    );
    assert!(app.document.shapes.is_empty());
    let top = app.history_rows(layout)[0].rect;
    click(
        &mut app,
        Point {
            x: top.x + 10.0,
            y: top.y + 5.0,
        },
    );
    assert_eq!(app.document.shapes.len(), 3);
    assert!(app.history_rows(layout)[0].current);
}

#[test]
fn undo_interleaves_sketch_and_solid_steps_across_views() {
    let mut app = App::with_project(Path::new("fixtures/extrude-options.pcad")).unwrap();
    app.selected_solid = 0;
    app.set_solid_depth(40.0).unwrap();
    app.create_sketch(SketchPlane::Top);
    assert!(!app.solid_preview);
    assert!(!app.read_only());
    draw_line(&mut app, 0.0);
    app.set_solid_depth(30.0).unwrap();
    assert_eq!(
        labels(&app),
        ["Extrude 40 mm", "Draw line", "Extrude 30 mm"]
    );

    // From the sketch view Ctrl+Z first reverts the newer solid edit.
    ctrl_key(&mut app, false, KeyCode::KeyZ);
    assert!(app.solid_preview, "undo switched to the solid view");
    assert_eq!(depth(&app, 0), 40.0);
    assert_eq!(app.document.shapes.len(), 1);
    ctrl_key(&mut app, false, KeyCode::KeyZ);
    assert!(!app.solid_preview, "undo switched back to the sketch");
    assert!(app.document.shapes.is_empty());
    assert_eq!(app.status_message, "Undo: Draw line");
    ctrl_key(&mut app, false, KeyCode::KeyZ);
    assert_eq!(depth(&app, 0), 60.0);
    ctrl_key(&mut app, false, KeyCode::KeyZ);
    assert_eq!(app.status_message, "Nothing to undo");

    // Redo replays in the original order.
    ctrl_key(&mut app, false, KeyCode::KeyY);
    assert_eq!(depth(&app, 0), 40.0);
    assert!(app.document.shapes.is_empty());
    ctrl_key(&mut app, true, KeyCode::KeyZ);
    assert_eq!(app.document.shapes.len(), 1);
    assert!(!app.solid_preview);
    ctrl_key(&mut app, false, KeyCode::KeyY);
    assert_eq!(depth(&app, 0), 30.0);
    assert!(app.history_timeline().iter().all(|step| step.applied));
}

#[test]
fn app_undo_restores_the_selection() {
    let mut app = App::default();
    draw_line(&mut app, 0.0);
    draw_line(&mut app, 20.0);
    let ids: Vec<u32> = app.document.shapes.iter().map(Shape::id).collect();
    app.document.set_selection(&ids);
    assert!(app.document.move_selected(Point { x: 5.0, y: 0.0 }));
    app.document.clear_selection();
    ctrl_key(&mut app, false, KeyCode::KeyZ);
    assert_eq!(app.status_message, "Undo: Move 2 shapes");
    let mut selected = app.document.selected_ids();
    selected.sort_unstable();
    assert_eq!(selected, ids);
}

#[test]
fn feature_tree_and_sketch_steps_undo_in_stamp_order() {
    let mut app = App::with_project(Path::new("fixtures/feature-tree.pcad")).unwrap();
    let suppressed = |app: &App| app.project.as_ref().unwrap().feature_suppressed(1);
    assert!(suppressed(&app));
    app.selected_solid = 1;
    app.toggle_selected_suppressed();
    assert!(!suppressed(&app));
    app.create_sketch(SketchPlane::Top);
    assert!(!app.read_only());
    draw_line(&mut app, 0.0);
    let timeline = labels(&app);
    assert_eq!(timeline.len(), 2);
    assert!(timeline[0].starts_with("Unsuppress "), "{timeline:?}");
    assert_eq!(timeline[1], "Draw line");

    // The newer sketch step goes first, then the feature tree snapshot.
    assert!(app.undo_any());
    assert!(app.document.shapes.is_empty());
    assert!(!suppressed(&app));
    assert!(app.undo_any());
    assert!(suppressed(&app));
    assert!(app.solid_preview, "tree undo shows the 3D preview");
    assert!(app.status_message.starts_with("Undo: Unsuppress "));
    assert!(!app.undo_any());

    assert!(app.redo_any());
    assert!(!suppressed(&app));
    assert!(app.document.shapes.is_empty());
    assert!(app.redo_any());
    assert_eq!(app.document.shapes.len(), 1);
}
