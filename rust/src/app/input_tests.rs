//! Headless input tests: drive `App` through `handle_window_event` and
//! `handle_key` the way the `winit` handler does. Without a window,
//! `App::layout()` is the default 1280x800 layout and `redraw` is a no-op.

use super::input::KeyPress;
use super::*;
use winit::dpi::PhysicalPosition;
use winit::event::{DeviceId, TouchPhase};

fn device() -> DeviceId {
    DeviceId::dummy()
}

fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyPress {
        physical_key: PhysicalKey::Code(code),
        text: None,
    });
}

fn key_with(app: &mut App, modifiers: ModifiersState, code: KeyCode) {
    let previous = app.modifiers;
    app.modifiers = modifiers;
    key(app, code);
    app.modifiers = previous;
}

fn type_text(app: &mut App, text: &str) {
    for ch in text.chars() {
        app.handle_key(KeyPress {
            physical_key: PhysicalKey::Code(char_code(ch)),
            text: Some(ch.to_string().into()),
        });
    }
}

/// A plausible physical key for a typed character. Prompts read the text,
/// so any code other than Enter/Escape/Backspace/Tab works.
fn char_code(ch: char) -> KeyCode {
    match ch {
        '0' => KeyCode::Digit0,
        '1'..='9' => KeyCode::Digit1,
        '-' => KeyCode::Minus,
        '.' => KeyCode::Period,
        _ => KeyCode::KeyQ,
    }
}

fn move_to(app: &mut App, point: Point) {
    let exit = app.handle_window_event(WindowEvent::CursorMoved {
        device_id: device(),
        position: PhysicalPosition::new(point.x as f64, point.y as f64),
    });
    assert!(!exit);
}

fn mouse(app: &mut App, state: ElementState, button: MouseButton) {
    app.handle_window_event(WindowEvent::MouseInput {
        device_id: device(),
        state,
        button,
    });
}

fn click(app: &mut App, point: Point) {
    move_to(app, point);
    mouse(app, ElementState::Pressed, MouseButton::Left);
    mouse(app, ElementState::Released, MouseButton::Left);
}

fn wheel(app: &mut App, lines: f32) {
    app.handle_window_event(WindowEvent::MouseWheel {
        device_id: device(),
        delta: MouseScrollDelta::LineDelta(0.0, lines),
        phase: TouchPhase::Moved,
    });
}

/// A point inside the sketch viewport, offset from its top-left corner.
fn canvas(dx: f32, dy: f32) -> Point {
    let viewport = App::default().layout().viewport;
    Point {
        x: viewport.x + dx,
        y: viewport.y + dy,
    }
}

fn center(rect: Rect) -> Point {
    Point {
        x: rect.x + rect.w / 2.0,
        y: rect.y + rect.h / 2.0,
    }
}

/// Draw a line between two canvas offsets with the Line tool.
fn draw_line(app: &mut App, a: (f32, f32), b: (f32, f32)) {
    key(app, KeyCode::KeyL);
    click(app, canvas(a.0, a.1));
    click(app, canvas(b.0, b.1));
}

/// The screen position of a line's midpoint.
fn line_mid(app: &App, index: usize) -> Point {
    let Shape::Line { a, b, .. } = &app.document.shapes[index] else {
        panic!("shape {index} is not a line");
    };
    let mid = Point {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    };
    app.document.world_to_screen(mid, app.viewport_origin())
}

fn line_length(app: &App, index: usize) -> f32 {
    let Shape::Line { a, b, .. } = &app.document.shapes[index] else {
        panic!("shape {index} is not a line");
    };
    a.distance(*b)
}

fn ctrl() -> ModifiersState {
    ModifiersState::CONTROL
}

fn read_only_app() -> App {
    // An extruded sketch is read-only in the sketch view.
    let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    key(&mut app, KeyCode::Tab);
    assert!(!app.solid_preview);
    assert!(app.read_only());
    app
}

#[test]
fn tool_shortcuts_select_tools() {
    let mut app = App::default();
    let plain = ModifiersState::empty();
    let shift = ModifiersState::SHIFT;
    for (modifiers, code, tool) in [
        (plain, KeyCode::KeyL, Tool::Line),
        (plain, KeyCode::KeyR, Tool::Rectangle),
        (plain, KeyCode::KeyC, Tool::Circle),
        (plain, KeyCode::KeyA, Tool::Arc),
        (shift, KeyCode::KeyA, Tool::CenterArc),
        (plain, KeyCode::KeyT, Tool::ThreePointRectangle),
        (plain, KeyCode::KeyG, Tool::Polygon),
        (plain, KeyCode::KeyP, Tool::Point),
        (plain, KeyCode::KeyD, Tool::Dimension),
        (plain, KeyCode::KeyV, Tool::Select),
    ] {
        key_with(&mut app, modifiers, code);
        assert_eq!(app.document.tool, tool, "{code:?} with {modifiers:?}");
    }
}

#[test]
fn switching_tools_drops_an_unfinished_draft() {
    let mut app = App::default();
    key(&mut app, KeyCode::KeyL);
    click(&mut app, canvas(100.0, 100.0));
    assert!(app.document.draft_start.is_some());
    key(&mut app, KeyCode::KeyR);
    assert!(app.document.draft_start.is_none());
    assert!(app.document.shapes.is_empty());
}

#[test]
fn two_click_line_creates_one_shape_and_one_undo_step() {
    let mut app = App::default();
    draw_line(&mut app, (100.0, 100.0), (250.0, 100.0));
    assert_eq!(app.document.shapes.len(), 1);
    assert!(matches!(app.document.shapes[0], Shape::Line { .. }));
    assert!(app.document.draft_start.is_none());
    assert!(app.document.undo());
    assert!(app.document.shapes.is_empty());
    assert!(!app.document.undo(), "line creation is a single undo step");
}

#[test]
fn escape_cancels_placement_without_an_undo_entry() {
    let mut app = App::default();
    key(&mut app, KeyCode::KeyL);
    click(&mut app, canvas(100.0, 100.0));
    assert!(app.document.draft_start.is_some());
    key(&mut app, KeyCode::Escape);
    assert!(app.document.draft_start.is_none());
    assert!(app.document.draft_end.is_none());
    assert!(app.document.shapes.is_empty());
    assert!(!app.document.undo());
    // The tool stays active after Esc.
    assert_eq!(app.document.tool, Tool::Line);
}

#[test]
fn ctrl_z_and_ctrl_y_round_trip() {
    let mut app = App::default();
    draw_line(&mut app, (100.0, 100.0), (250.0, 100.0));
    draw_line(&mut app, (100.0, 200.0), (250.0, 200.0));
    assert_eq!(app.document.shapes.len(), 2);
    key_with(&mut app, ctrl(), KeyCode::KeyZ);
    assert_eq!(app.document.shapes.len(), 1);
    key_with(&mut app, ctrl(), KeyCode::KeyZ);
    assert!(app.document.shapes.is_empty());
    key_with(&mut app, ctrl(), KeyCode::KeyY);
    assert_eq!(app.document.shapes.len(), 1);
    key_with(&mut app, ctrl() | ModifiersState::SHIFT, KeyCode::KeyZ);
    assert_eq!(app.document.shapes.len(), 2);
    // Plain Z without Ctrl is not undo.
    key(&mut app, KeyCode::KeyZ);
    assert_eq!(app.document.shapes.len(), 2);
}

#[test]
fn middle_drag_pans_until_release() {
    let mut app = App::default();
    let start = canvas(300.0, 300.0);
    move_to(&mut app, start);
    let pan = app.document.pan;
    mouse(&mut app, ElementState::Pressed, MouseButton::Middle);
    move_to(
        &mut app,
        Point {
            x: start.x + 30.0,
            y: start.y - 10.0,
        },
    );
    assert!((app.document.pan.x - pan.x - 30.0).abs() < 1e-4);
    assert!((app.document.pan.y - pan.y + 10.0).abs() < 1e-4);
    mouse(&mut app, ElementState::Released, MouseButton::Middle);
    let after = app.document.pan;
    move_to(&mut app, start);
    assert_eq!(app.document.pan, after);
}

#[test]
fn plain_cursor_motion_does_not_pan() {
    let mut app = App::default();
    let pan = app.document.pan;
    move_to(&mut app, canvas(10.0, 10.0));
    move_to(&mut app, canvas(90.0, 60.0));
    assert_eq!(app.document.pan, pan);
}

#[test]
fn wheel_zooms_about_the_cursor() {
    let mut app = App::default();
    let cursor = canvas(220.0, 140.0);
    move_to(&mut app, cursor);
    let before = app.document.screen_to_world(cursor, app.viewport_origin());
    let zoom = app.document.zoom;
    wheel(&mut app, 1.0);
    assert!(app.document.zoom > zoom);
    let after = app.document.screen_to_world(cursor, app.viewport_origin());
    assert!(before.distance(after) < 1e-3, "{before:?} -> {after:?}");
    wheel(&mut app, -2.0);
    assert!(app.document.zoom < zoom);
    let after = app.document.screen_to_world(cursor, app.viewport_origin());
    assert!(before.distance(after) < 1e-3);
}

#[test]
fn wheel_outside_viewport_does_not_zoom() {
    let mut app = App::default();
    move_to(&mut app, Point { x: 5.0, y: 5.0 });
    let zoom = app.document.zoom;
    wheel(&mut app, 3.0);
    assert_eq!(app.document.zoom, zoom);
}

#[test]
fn tab_switches_views_only_when_a_solid_exists() {
    let mut app = App::with_project(Path::new("fixtures/extruded-rectangle.pcad")).unwrap();
    assert!(app.solid_preview);
    key(&mut app, KeyCode::Tab);
    assert!(!app.solid_preview);
    assert!(app.status_message.starts_with("Sketch view"));
    key(&mut app, KeyCode::Tab);
    assert!(app.solid_preview);
    assert!(app.status_message.starts_with("3D extrusion preview"));

    let mut flat = App::with_project(Path::new("fixtures/two-parts.pcad")).unwrap();
    assert!(!flat.solid_preview);
    key(&mut flat, KeyCode::Tab);
    assert!(!flat.solid_preview);
}

#[test]
fn click_selects_and_shift_click_toggles_selection() {
    let mut app = App::default();
    draw_line(&mut app, (100.0, 100.0), (250.0, 100.0));
    draw_line(&mut app, (100.0, 200.0), (250.0, 200.0));
    let (first, second) = (app.document.shapes[0].id(), app.document.shapes[1].id());
    key(&mut app, KeyCode::KeyV);

    let mid = line_mid(&app, 0);

    click(&mut app, mid);
    assert_eq!(app.document.selected, Some(first));
    assert_eq!(app.document.selection_count(), 1);

    app.modifiers = ModifiersState::SHIFT;
    let mid = line_mid(&app, 1);
    click(&mut app, mid);
    assert_eq!(app.document.selection_count(), 2);
    assert!(app.document.is_selected(first) && app.document.is_selected(second));
    let mid = line_mid(&app, 0);
    click(&mut app, mid);
    assert_eq!(app.document.selection_count(), 1);
    assert!(!app.document.is_selected(first));
    assert!(app.document.is_selected(second));
    app.modifiers = ModifiersState::empty();

    // A plain click replaces the selection.
    let mid = line_mid(&app, 0);
    click(&mut app, mid);
    assert_eq!(app.document.selected_ids(), vec![first]);
}

#[test]
fn box_select_by_press_move_release() {
    let mut app = App::default();
    draw_line(&mut app, (100.0, 100.0), (250.0, 100.0));
    draw_line(&mut app, (100.0, 200.0), (250.0, 200.0));
    draw_line(&mut app, (500.0, 400.0), (600.0, 400.0));
    key(&mut app, KeyCode::KeyV);
    move_to(&mut app, canvas(60.0, 60.0));
    mouse(&mut app, ElementState::Pressed, MouseButton::Left);
    assert!(app.box_start.is_some());
    move_to(&mut app, canvas(300.0, 260.0));
    mouse(&mut app, ElementState::Released, MouseButton::Left);
    assert!(app.box_start.is_none());
    assert_eq!(app.document.selection_count(), 2);
    let third = app.document.shapes[2].id();
    assert!(!app.document.is_selected(third));
}

#[test]
fn empty_click_clears_selection_and_escape_does_too() {
    let mut app = App::default();
    draw_line(&mut app, (100.0, 100.0), (250.0, 100.0));
    key(&mut app, KeyCode::KeyV);
    let mid = line_mid(&app, 0);
    click(&mut app, mid);
    assert!(app.document.selected.is_some());
    click(&mut app, canvas(500.0, 450.0));
    assert_eq!(app.document.selection_count(), 0);

    let mid = line_mid(&app, 0);

    click(&mut app, mid);
    key(&mut app, KeyCode::Escape);
    assert_eq!(app.document.selection_count(), 0);
}

#[test]
fn delete_removes_the_selected_shape() {
    let mut app = App::default();
    draw_line(&mut app, (100.0, 100.0), (250.0, 100.0));
    key(&mut app, KeyCode::KeyV);
    let mid = line_mid(&app, 0);
    click(&mut app, mid);
    key(&mut app, KeyCode::Delete);
    assert!(app.document.shapes.is_empty());
    key_with(&mut app, ctrl(), KeyCode::KeyZ);
    assert_eq!(app.document.shapes.len(), 1);
}

#[test]
fn read_only_sketch_rejects_drawing_with_status() {
    let mut app = read_only_app();
    let shapes = app.document.shapes.len();
    key(&mut app, KeyCode::KeyL);
    click(&mut app, canvas(100.0, 100.0));
    click(&mut app, canvas(250.0, 100.0));
    assert_eq!(app.document.shapes.len(), shapes);
    assert!(app.document.draft_start.is_none());
    assert!(
        app.status_message.starts_with("READ ONLY"),
        "{}",
        app.status_message
    );
}

#[test]
fn read_only_sketch_ignores_delete_and_undo() {
    let mut app = read_only_app();
    let shapes = app.document.shapes.len();
    assert!(shapes > 0);
    app.document.selected = app.document.shapes.first().map(Shape::id);
    key(&mut app, KeyCode::Delete);
    assert_eq!(app.document.shapes.len(), shapes);
    key_with(&mut app, ctrl(), KeyCode::KeyZ);
    key_with(&mut app, ctrl(), KeyCode::KeyY);
    assert_eq!(app.document.shapes.len(), shapes);
}

#[test]
fn read_only_more_menu_edits_report_read_only() {
    let mut app = read_only_app();
    let layout = app.layout();
    app.more_open = true;
    click(&mut app, center(layout.more_item(16)));
    assert!(
        app.status_message.starts_with("READ ONLY"),
        "{}",
        app.status_message
    );
}

#[test]
fn more_button_and_m_key_toggle_the_menu() {
    let mut app = App::default();
    let layout = app.layout();
    click(&mut app, center(layout.tool_button(9)));
    assert!(app.more_open);
    click(&mut app, center(layout.tool_button(9)));
    assert!(!app.more_open);
    key(&mut app, KeyCode::KeyM);
    assert!(app.more_open);
    key(&mut app, KeyCode::KeyM);
    assert!(!app.more_open);
}

#[test]
fn more_menu_items_dispatch_tools_and_close() {
    let mut app = App::default();
    let layout = app.layout();
    for (index, tool) in [
        (0, Tool::Point),
        (1, Tool::Polygon),
        (2, Tool::Slot),
        (5, Tool::ThreePointCircle),
        (6, Tool::CenterArc),
        (7, Tool::CenterRectangle),
        (9, Tool::ThreePointCenterRectangle),
        (11, Tool::FitSpline),
        (12, Tool::ControlSpline),
    ] {
        click(&mut app, center(layout.tool_button(9)));
        assert!(app.more_open);
        click(&mut app, center(layout.more_item(index)));
        assert_eq!(app.document.tool, tool, "MORE item {index}");
        assert!(!app.more_open);
    }
}

#[test]
fn more_menu_closes_on_a_click_elsewhere() {
    let mut app = App::default();
    let layout = app.layout();
    click(&mut app, center(layout.tool_button(9)));
    let tool = app.document.tool;
    click(&mut app, canvas(20.0, 400.0));
    assert!(!app.more_open);
    assert_eq!(app.document.tool, tool);
}

#[test]
fn spline_tool_collects_points_and_enter_finishes() {
    let mut app = App::default();
    let layout = app.layout();
    click(&mut app, center(layout.tool_button(9)));
    click(&mut app, center(layout.more_item(11)));
    for (x, y) in [(100.0, 100.0), (200.0, 160.0), (300.0, 100.0)] {
        click(&mut app, canvas(x, y));
    }
    assert_eq!(app.spline_points.len(), 3);
    key(&mut app, KeyCode::Enter);
    assert!(app.spline_points.is_empty());
    assert_eq!(app.document.shapes.len(), 1);
}

fn line_length_prompt() -> App {
    let mut app = App::default();
    draw_line(&mut app, (100.0, 100.0), (250.0, 100.0));
    app.document.selected = Some(app.document.shapes[0].id());
    app.editing = Some(DimensionEdit {
        field: EditField::Dimension(DimensionField::LineLength),
        buffer: String::new(),
    });
    app
}

fn prompt_buffer(app: &App) -> &str {
    &app.editing.as_ref().expect("prompt open").buffer
}

#[test]
fn dimension_prompt_accepts_digits_and_backspace_then_enter_applies() {
    let mut app = line_length_prompt();
    type_text(&mut app, "427");
    assert_eq!(prompt_buffer(&app), "427");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(prompt_buffer(&app), "42");
    key(&mut app, KeyCode::Enter);
    assert!(app.editing.is_none());
    assert!((line_length(&app, 0) - 42.0).abs() < 1e-3);
}

#[test]
fn dimension_prompt_escape_discards_the_value() {
    let mut app = line_length_prompt();
    let length = line_length(&app, 0);
    type_text(&mut app, "5");
    key(&mut app, KeyCode::Escape);
    assert!(app.editing.is_none());
    assert_eq!(app.status_message, "Dimension edit cancelled");
    assert_eq!(line_length(&app, 0), length);
}

#[test]
fn dimension_prompt_swallows_tool_shortcuts() {
    let mut app = line_length_prompt();
    // Letters are expression characters, not tool shortcuts, while editing.
    app.handle_key(KeyPress {
        physical_key: PhysicalKey::Code(KeyCode::KeyR),
        text: Some("r".into()),
    });
    assert_eq!(app.document.tool, Tool::Line);
    assert_eq!(prompt_buffer(&app), "r");
    // Characters outside the expression set are dropped.
    type_text(&mut app, "!?");
    assert_eq!(prompt_buffer(&app), "r");
}

#[test]
fn numeric_prompt_accepts_only_digits_and_dot() {
    let mut app = App {
        editing: Some(DimensionEdit {
            field: EditField::FilletRadius,
            buffer: String::new(),
        }),
        ..App::default()
    };
    type_text(&mut app, "-1a.5");
    assert_eq!(prompt_buffer(&app), "1.5");
}

#[test]
fn move_prompt_accepts_a_leading_sign_only() {
    let mut app = App {
        editing: Some(DimensionEdit {
            field: EditField::MoveX,
            buffer: String::new(),
        }),
        ..App::default()
    };
    type_text(&mut app, "-3-");
    assert_eq!(prompt_buffer(&app), "-3");
}

#[test]
fn rename_prompt_types_and_escape_cancels() {
    let mut app = App::with_project(Path::new("fixtures/two-parts.pcad")).unwrap();
    key(&mut app, KeyCode::F2);
    assert!(app.name_edit.is_some());
    type_text(&mut app, "Bracket");
    assert_eq!(app.name_edit.as_ref().unwrap().buffer, "Bracket");
    key(&mut app, KeyCode::Backspace);
    assert_eq!(app.name_edit.as_ref().unwrap().buffer, "Bracke");
    key(&mut app, KeyCode::Escape);
    assert!(app.name_edit.is_none());
    assert_eq!(app.status_message, "Rename cancelled");
}

#[test]
fn rename_without_project_reports_status() {
    let mut app = App::default();
    key(&mut app, KeyCode::F2);
    assert!(app.name_edit.is_none());
    assert!(app.status_message.contains("Open a .pcad project"));
}

#[test]
fn close_requests_exit_and_redraw_is_headless_safe() {
    let mut app = App::default();
    assert!(app.handle_window_event(WindowEvent::CloseRequested));
    assert!(!app.handle_window_event(WindowEvent::RedrawRequested));
}

#[test]
fn ui_colors_are_distinct() {
    let colors = [
        BG,
        PANEL,
        PANEL_LIGHT,
        BORDER,
        TEXT,
        MUTED,
        ACCENT,
        SHAPE,
        FULLY_CONSTRAINED,
        CONFLICT,
        REDUNDANT,
        DIMENSION,
        FACE_HOVER,
    ];
    for (i, a) in colors.iter().enumerate() {
        assert!(a.iter().all(|c| (0.0..=1.0).contains(c)));
        for b in &colors[i + 1..] {
            assert_ne!(a, b);
        }
    }
}
