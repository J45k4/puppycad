use super::*;

fn info() -> StatusInfo {
    StatusInfo {
        cursor: Some((12.7, -25.4)),
        units: Units::Mm,
        tool: "LINE",
        dof: Some("DOF 3".into()),
        snap_geometry: true,
        snap_grid: false,
        zoom_percent: 100,
    }
}

#[test]
fn status_text_changes_only_when_values_change() {
    let mut cache = StatusCache::default();
    assert!(cache.update(info()));
    assert!(!cache.update(info()));
    assert_eq!(cache.lines()[0], "X 12.7 Y -25.4 MM");
    assert_eq!(cache.lines()[1], "LINE  DOF 3  SNAP GEOM");
    assert!(cache.update(StatusInfo {
        units: Units::In,
        ..info()
    }));
    assert_eq!(cache.lines()[0], "X 0.500 Y -1.000 IN");
    assert!(cache.update(StatusInfo {
        snap_grid: true,
        ..info()
    }));
    assert!(cache.lines()[1].ends_with("SNAP GEOM+GRID"));
    assert!(!cache.update(StatusInfo {
        snap_grid: true,
        ..info()
    }));
}

#[test]
fn status_info_follows_app_state() {
    let mut app = App::default();
    let layout = app.layout();
    app.cursor = Point { x: 5.0, y: 5.0 };
    assert_eq!(app.status_info(layout).cursor, None);
    app.cursor = app.viewport_origin();
    let center = app.status_info(layout);
    let (x, y) = center.cursor.unwrap();
    assert!(x.abs() < 1e-3 && y.abs() < 1e-3);
    assert_eq!(center.tool, "SELECT");
    assert_eq!(center.dof.as_deref(), Some("DOF 0"));
    app.snap_grid = false;
    assert_ne!(app.status_info(layout), center);
}

#[test]
fn tooltips_name_command_and_shortcut() {
    assert_eq!(
        tooltip_text(HoverTarget::Tool(1)).as_deref(),
        Some("Rectangle  [R]")
    );
    assert_eq!(
        tooltip_text(HoverTarget::More(21)).as_deref(),
        Some("Settings  [Ctrl+,]")
    );
    assert_eq!(
        tooltip_text(HoverTarget::More(5)).as_deref(),
        Some("Three-point circle")
    );
    assert_eq!(tooltip_text(HoverTarget::More(99)), None);
    let layout = Layout::new(1280.0, 800.0);
    assert_eq!(
        layout.hover_target(Point { x: 265.0, y: 55.0 }, false),
        Some(HoverTarget::Tool(2))
    );
    assert_eq!(
        layout.hover_target(
            Point {
                x: 1150.0,
                y: 145.0
            },
            true
        ),
        Some(HoverTarget::More(1))
    );
    assert_eq!(
        layout.hover_target(
            Point {
                x: 1150.0,
                y: 145.0
            },
            false
        ),
        None
    );
}

#[test]
fn glyphs_are_sixteen_by_sixteen_and_fit_buttons() {
    let layout = Layout::new(1280.0, 800.0);
    for index in 0..10 {
        let glyph = tool_glyph(index);
        assert!(glyph.iter().all(|row| row.len() == 16));
        assert!(glyph.iter().any(|row| row.contains('#')));
        assert!(layout.tool_button(index).h >= 8.0 + 16.0);
    }
}
