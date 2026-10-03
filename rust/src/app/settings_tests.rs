use super::*;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("puppycad-settings-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn defaults_when_missing_or_invalid() {
    let dir = temp_dir("missing");
    assert_eq!(
        Settings::load_from(&dir.join("settings.json")),
        Settings::default()
    );
    let defaults = Settings::default();
    assert_eq!(defaults.units, Units::Mm);
    assert_eq!(defaults.grid_mm, 1.0);
    assert_eq!(defaults.theme, Theme::Dark);
    assert_eq!(defaults.ui_scale, None);
    assert_eq!(Settings::from_json("not json"), Settings::default());
    assert_eq!(
        Settings::from_json(r#"{"units":"furlong","grid_mm":-1,"theme":"pink"}"#),
        Settings::default()
    );
}

#[test]
fn save_and_load_round_trip() {
    let dir = temp_dir("roundtrip");
    let path = dir.join("nested/settings.json");
    let settings = Settings {
        units: Units::In,
        grid_mm: 2.0,
        theme: Theme::Light,
        ui_scale: Some(1.5),
        window: Some(WindowGeometry {
            width: 1400.0,
            height: 900.0,
            x: 10,
            y: 20,
        }),
    };
    settings.save_to(&path).unwrap();
    assert_eq!(Settings::load_from(&path), settings);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"units\": \"in\""), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ui_scale_rounds_to_supported_steps() {
    assert_eq!(round_ui_scale(1.0), 1.0);
    assert_eq!(round_ui_scale(1.2), 1.0);
    assert_eq!(round_ui_scale(1.25), 1.5);
    assert_eq!(round_ui_scale(1.6), 1.5);
    assert_eq!(round_ui_scale(2.0), 2.0);
    assert_eq!(round_ui_scale(3.0), 2.0);
    assert_eq!(
        Settings::from_json(r#"{"ui_scale":1.4}"#).ui_scale,
        Some(1.5)
    );
}

#[test]
fn unit_conversion_both_ways() {
    let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
    for (units, mm, shown) in [
        (Units::Mm, 12.5, 12.5),
        (Units::Cm, 12.5, 1.25),
        (Units::Cm, -40.0, -4.0),
        (Units::In, 25.4, 1.0),
        (Units::In, 12.7, 0.5),
        (Units::In, 0.0, 0.0),
    ] {
        assert!(close(units.mm_to_display(mm), shown), "{units:?} {mm}");
        assert!(close(units.display_to_mm(shown), mm), "{units:?} {shown}");
    }
    assert_eq!(Units::In.format(12.7), "0.500 in");
    assert_eq!(Units::Cm.format(12.5), "1.25 cm");
    assert_eq!(Units::Mm.format(12.5), "12.5 mm");
}

#[test]
fn typed_values_are_converted_to_millimeters() {
    let mut app = App::default();
    assert_eq!(app.entry_to_mm(EditField::PointX, "2"), "2");
    app.settings.units = Units::In;
    assert_eq!(app.entry_to_mm(EditField::PointX, "2"), "50.8");
    assert_eq!(app.entry_to_mm(EditField::RotateAngle, "2"), "2");
    // Dimension text goes through the expression parser with the unit.
    let key = DimensionKey::Field(1, DimensionField::LineLength);
    assert_eq!(app.entry_to_mm(EditField::Label(key), "2"), "2in");
    assert_eq!(app.entry_to_mm(EditField::Label(key), "10mm"), "10mm");
    assert_eq!(app.display_label(key, "25.4"), "1.000in");
    app.settings.units = Units::Cm;
    assert_eq!(app.entry_to_mm(EditField::MoveX, "1.5"), "15");
}

#[test]
fn layout_scales_linearly() {
    let base = Layout::new(1280.0, 800.0);
    for scale in UI_SCALES {
        let scaled = Layout::scaled(1280.0 * scale, 800.0 * scale, scale);
        let mut pairs = vec![
            (base.viewport, scaled.viewport),
            (base.left, scaled.left),
            (base.right, scaled.right),
            (base.top, scaled.top),
            (base.bottom, scaled.bottom),
            (base.settings_panel(), scaled.settings_panel()),
            (base.status_info(), scaled.status_info()),
        ];
        for index in 0..10 {
            pairs.push((base.tool_button(index), scaled.tool_button(index)));
        }
        for index in 0..MORE_TIPS.len() {
            pairs.push((base.more_item(index), scaled.more_item(index)));
        }
        for index in 0..3 {
            pairs.push((base.filter_button(index), scaled.filter_button(index)));
        }
        for (a, b) in pairs {
            let physical = scaled.physical(b);
            for (x, y) in [
                (a.x * scale, physical.x),
                (a.y * scale, physical.y),
                (a.w * scale, physical.w),
                (a.h * scale, physical.h),
            ] {
                assert!((x - y).abs() < 1e-3, "{scale}: {a:?} vs {physical:?}");
            }
        }
    }
}

#[test]
fn more_items_do_not_overlap() {
    let layout = Layout::new(1280.0, 800.0);
    let items: Vec<Rect> = (0..MORE_TIPS.len())
        .map(|index| layout.more_item(index))
        .collect();
    for (i, a) in items.iter().enumerate() {
        assert!(a.y + a.h <= 800.0 && a.x >= 0.0, "{i}: {a:?}");
        for b in &items[i + 1..] {
            let overlap = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
            assert!(!overlap, "{a:?} overlaps {b:?}");
        }
    }
}

#[test]
fn palettes_have_distinct_foreground_and_background() {
    for palette in [&DARK, &LIGHT] {
        let luma = |c: Color| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        assert!((luma(palette.text) - luma(palette.bg)).abs() > 0.5);
        assert!((luma(palette.text) - luma(palette.panel)).abs() > 0.5);
        assert_ne!(palette.accent, palette.bg);
    }
    assert!(DARK.remap().is_empty());
    let remap = LIGHT.remap();
    assert!(remap.contains(&(BG, LIGHT.bg)));
    assert!(remap.contains(&(TEXT, LIGHT.text)));
    let mut painter = Painter::new(10.0, 10.0);
    painter.set_remap(remap);
    assert_eq!(painter.map_color(TEXT), LIGHT.text);
}

#[test]
fn grid_setting_reaches_the_document() {
    let mut app = App::default();
    assert!(app.setting_value(0).contains("MM"));
    app.settings.units = Units::Cm;
    assert!(app.setting_value(0).contains("CM"));
    app.settings.grid_mm = 2.0;
    app.apply_settings();
    assert_eq!(app.document.grid_step, 2.0);
    let snapped = app
        .document
        .snap_point(Point { x: 2.9, y: -1.2 }, true, false)
        .unwrap();
    assert_eq!((snapped.point.x, snapped.point.y), (2.0, -2.0));
}
