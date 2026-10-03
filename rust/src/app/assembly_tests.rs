//! Headless assembly view tests on a copy of `fixtures/assembly.pcad`.

use super::input::KeyPress;
use super::*;
use winit::dpi::PhysicalPosition;
use winit::event::DeviceId;

fn key_text(app: &mut App, code: KeyCode, text: Option<&str>) {
    app.handle_key(KeyPress {
        physical_key: PhysicalKey::Code(code),
        text: text.map(Into::into),
    });
}

fn key(app: &mut App, code: KeyCode) {
    key_text(app, code, None);
}

fn key_with(app: &mut App, modifiers: ModifiersState, code: KeyCode) {
    let previous = app.modifiers;
    app.modifiers = modifiers;
    key(app, code);
    app.modifiers = previous;
}

fn move_to(app: &mut App, point: Point) {
    app.handle_window_event(WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(point.x as f64, point.y as f64),
    });
}

fn click(app: &mut App, point: Point) {
    move_to(app, point);
    for state in [ElementState::Pressed, ElementState::Released] {
        app.handle_window_event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: MouseButton::Left,
        });
    }
}

/// Copies the fixture into a fresh directory and opens its assembly.
fn open_fixture(name: &str) -> (App, PathBuf) {
    let dir = std::env::temp_dir().join(format!("puppycad-asm-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("robot.pcad");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/assembly.pcad"),
        &path,
    )
    .unwrap();
    let mut app = App::with_project(&path).unwrap();
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyB);
    assert!(app.assembly.is_some(), "{}", app.status_message);
    (app, dir)
}

/// A screen point over a face of `instance` whose pick hits that instance.
fn point_on(app: &App, instance: usize) -> Point {
    let layout = app.layout();
    let (scene, projector) = app.assembly_projector(layout).unwrap();
    scene
        .triangles
        .iter()
        .zip(&scene.triangle_body)
        .filter(|(_, body)| **body == instance)
        .map(|(tri, _)| {
            let s = tri.map(|p| projector.screen(p));
            Point {
                x: (s[0][0] + s[1][0] + s[2][0]) / 3.0,
                y: (s[0][1] + s[1][1] + s[2][1]) / 3.0,
            }
        })
        .find(|p| {
            app.assembly_face_at(layout, *p)
                .is_some_and(|pick| pick.instance == instance)
        })
        .expect("instance visible")
}

fn view(app: &App) -> &AssemblyView {
    app.assembly.as_ref().unwrap()
}

#[test]
fn ctrl_b_opens_the_solved_assembly_and_closes_again() {
    let (mut app, dir) = open_fixture("open");
    let asm = &view(&app).asm;
    assert_eq!(asm.instances.len(), 3);
    assert!((asm.instances[1].transform().translation[2] - 10.0).abs() < 1e-6);
    assert!(app.part_rows().len() == 3, "two parts and the assembly");
    let crumbs: Vec<String> = app
        .breadcrumbs(app.layout())
        .into_iter()
        .map(|c| c.1)
        .collect();
    assert!(crumbs.contains(&"Arm assembly".to_string()), "{crumbs:?}");
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyB);
    assert!(app.assembly.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn click_selects_and_hover_highlights_an_instance() {
    let (mut app, dir) = open_fixture("pick");
    let target = point_on(&app, 2);
    move_to(&mut app, target);
    assert_eq!(view(&app).hovered, Some(2));
    click(&mut app, target);
    assert_eq!(view(&app).selected, Some(2));
    assert_eq!(view(&app).asm.mates_of(2).len(), 1);
    let painter = app.paint();
    drop(painter);
    key(&mut app, KeyCode::Escape);
    assert_eq!(view(&app).selected, None);
    key(&mut app, KeyCode::Escape);
    assert!(app.assembly.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn delete_and_undo_restore_instance_and_mates() {
    let (mut app, dir) = open_fixture("delete");
    let p = point_on(&app, 2);
    click(&mut app, p);
    key(&mut app, KeyCode::Delete);
    assert_eq!(view(&app).asm.instances.len(), 2);
    assert_eq!(view(&app).asm.mates.len(), 1);
    assert!(view(&app).asm.actuators.is_empty());
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyZ);
    assert_eq!(view(&app).asm.instances.len(), 3);
    assert_eq!(view(&app).asm.mates.len(), 2);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyY);
    assert_eq!(view(&app).asm.instances.len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn insert_then_mate_fastened_and_save() {
    let (mut app, dir) = open_fixture("mate");
    key(&mut app, KeyCode::KeyI);
    key_text(&mut app, KeyCode::Digit1, Some("1"));
    assert_eq!(view(&app).asm.instances.len(), 4);
    assert_eq!(view(&app).selected, Some(3));
    assert_eq!(view(&app).asm.instances[3].part_id, "part-base");
    let p = point_on(&app, 0);
    click(&mut app, p);
    let p = point_on(&app, 3);
    move_to(&mut app, p);
    key(&mut app, KeyCode::KeyJ);
    assert!(
        matches!(view(&app).prompt, Some(AsmPrompt::MateKind { .. })),
        "{}",
        app.status_message
    );
    key_text(&mut app, KeyCode::Digit1, Some("1"));
    assert_eq!(view(&app).asm.mates.len(), 3, "{}", app.status_message);
    let solution = puppycad::assembly::solver::solve(&view(&app).asm).unwrap();
    assert!(solution.residual < 1e-6);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyS);
    let (project, _) = ProjectFile::open(dir.join("robot.pcad")).unwrap();
    let saved = project.assembly_at(project.assembly_indices()[0]).unwrap();
    assert_eq!(saved.instances.len(), 4);
    assert_eq!(saved.mates.len(), 3);
    assert_eq!(saved.read_only_reason, None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn properties_edit_moves_an_unmated_instance() {
    let (mut app, dir) = open_fixture("props");
    key(&mut app, KeyCode::KeyI);
    key_text(&mut app, KeyCode::Digit2, Some("2"));
    let layout = app.layout();
    let row = assembly_property_row(layout, 7);
    click(
        &mut app,
        Point {
            x: row.x + 5.0,
            y: row.y + 5.0,
        },
    );
    for ch in ["4", "5"] {
        key_text(&mut app, KeyCode::Digit4, Some(ch));
    }
    key(&mut app, KeyCode::Enter);
    let rotation = view(&app).asm.instances[3].transform().rotation;
    assert!((rotation[2] - 45.0).abs() < 1e-9, "{rotation:?}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn exports_one_stl_per_instance_and_one_3mf() {
    let (mut app, dir) = open_fixture("export");
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyE);
    for name in ["Base", "Upper-arm", "Forearm"] {
        let path = dir.join(format!("robot-{name}.stl"));
        let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("{}", app.status_message));
        assert_eq!(bytes.len(), 84 + 12 * 50, "{name}: a box has 12 triangles");
    }
    key_with(
        &mut app,
        ModifiersState::CONTROL | ModifiersState::ALT,
        KeyCode::KeyE,
    );
    let package = std::fs::read(dir.join("robot.3mf")).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(package)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("3D/3dmodel.model").unwrap(), &mut xml).unwrap();
    assert_eq!(xml.matches("<object ").count(), 3);
    assert!(xml.contains("name=\"Forearm\""));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn instances_carry_transformed_sharp_edges() {
    let (app, dir) = open_fixture("edges");
    let (scene, projector) = app.assembly_projector(app.layout()).unwrap();
    // Three boxes, twelve sharp edges each.
    assert_eq!(scene.edges.len(), 36);
    // Edges follow the instance transform: the forearm's edges lie in its
    // world-space bounding box.
    let lo = |axis: usize| {
        scene
            .triangles
            .iter()
            .zip(&scene.triangle_body)
            .filter(|(_, b)| **b == 2)
            .flat_map(|(t, _)| t.iter().map(move |p| p[axis]))
            .fold(f32::INFINITY, f32::min)
    };
    for (_, edge) in scene.edges.iter().filter(|(i, _)| *i == 2) {
        for axis in 0..3 {
            assert!(edge.a[axis] >= lo(axis) - 1e-3);
        }
    }
    let visible = scene
        .edges
        .iter()
        .filter(|(_, e)| {
            projector.visible(&scene, std::array::from_fn(|i| (e.a[i] + e.b[i]) * 0.5))
        })
        .count();
    assert!(visible > 0 && visible < scene.edges.len(), "{visible}");
    let _ = std::fs::remove_dir_all(dir);
}

fn click_rect(app: &mut App, rect: Rect) {
    click(
        app,
        Point {
            x: rect.x + 5.0,
            y: rect.y + rect.h * 0.5,
        },
    );
}

/// World frame of connector `id` (on an instance).
fn world_frame(asm: &puppycad::assembly::Assembly, id: &str) -> puppycad::assembly::Pose {
    let connector = asm.connectors.iter().find(|c| c.id == id).unwrap();
    let instance = asm
        .instance(connector.instance_id.as_deref().unwrap())
        .unwrap();
    instance.transform().pose().mul(&connector.frame())
}

#[test]
fn mate_row_selects_and_delete_removes_mate_with_undo() {
    let (mut app, dir) = open_fixture("mate-delete");
    let p = point_on(&app, 2);
    click(&mut app, p);
    let layout = app.layout();
    click_rect(&mut app, assembly_mate_row(layout, 0));
    assert_eq!(view(&app).selected_mate.as_deref(), Some("mate-elbow"));
    let painter = app.paint();
    drop(painter);
    key(&mut app, KeyCode::Delete);
    let asm = &view(&app).asm;
    assert_eq!(asm.instances.len(), 3, "the instance stays");
    assert_eq!(asm.mates.len(), 1);
    assert!(asm.actuators.is_empty());
    assert!(
        !asm.connectors
            .iter()
            .any(|c| c.id == "arm1-end" || c.id == "arm2-pin")
    );
    assert_eq!(view(&app).selected_mate, None);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyZ);
    let asm = &view(&app).asm;
    assert_eq!(asm.mates.len(), 2);
    assert_eq!(asm.connectors.len(), 4);
    assert_eq!(asm.servo_angles(), vec![("mate-elbow".to_string(), 30.0)]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn servo_edit_re_solves_the_revolute_angle() {
    let (mut app, dir) = open_fixture("servo");
    let p = point_on(&app, 2);
    click(&mut app, p);
    let layout = app.layout();
    click_rect(&mut app, servo_value_rect(assembly_mate_row(layout, 0)));
    assert!(matches!(view(&app).prompt, Some(AsmPrompt::Servo { .. })));
    for ch in ["6", "0"] {
        key_text(&mut app, KeyCode::Digit6, Some(ch));
    }
    key(&mut app, KeyCode::Enter);
    let asm = &view(&app).asm;
    assert_eq!(asm.servo_angles(), vec![("mate-elbow".to_string(), 60.0)]);
    let angle = puppycad::assembly::solver::revolute_angle(
        &world_frame(asm, "arm1-end"),
        &world_frame(asm, "arm2-pin"),
    );
    assert!((angle - 60.0).abs() < 1e-3, "{angle}");
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyZ);
    assert_eq!(
        view(&app).asm.servo_angles(),
        vec![("mate-elbow".to_string(), 30.0)]
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn j_over_empty_space_fastens_the_selection_to_the_world() {
    let (mut app, dir) = open_fixture("world");
    key(&mut app, KeyCode::KeyI);
    key_text(&mut app, KeyCode::Digit1, Some("1"));
    let before = view(&app).asm.instances[3].transform();
    let viewport = app.layout().viewport;
    move_to(
        &mut app,
        Point {
            x: viewport.x + 4.0,
            y: viewport.y + viewport.h - 4.0,
        },
    );
    assert_eq!(view(&app).hovered, None);
    key(&mut app, KeyCode::KeyJ);
    let asm = &view(&app).asm;
    assert_eq!(asm.mates.len(), 3, "{}", app.status_message);
    let mate = asm.mates.last().unwrap();
    assert_eq!(mate.kind, puppycad::assembly::MateKind::Fasten);
    assert_eq!(mate.a.instance_id, None);
    let after = asm.instances[3].transform();
    assert!(after.pose().max_difference(&before.pose()) < 1e-6);
    key_with(&mut app, ModifiersState::CONTROL, KeyCode::KeyZ);
    assert_eq!(view(&app).asm.mates.len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn hovered_edge_becomes_the_revolute_axis() {
    let (mut app, dir) = open_fixture("edge-axis");
    key(&mut app, KeyCode::KeyI);
    key_text(&mut app, KeyCode::Digit1, Some("1"));
    let p = point_on(&app, 0);
    click(&mut app, p);
    // Hover the midpoint of a visible edge of the inserted instance.
    let (scene, projector) = app.assembly_projector(app.layout()).unwrap();
    let target = scene
        .edges
        .iter()
        .filter(|(i, _)| *i == 3)
        .find_map(|(_, e)| {
            let mid = std::array::from_fn(|i| (e.a[i] + e.b[i]) * 0.5);
            let s = projector.screen(mid);
            let point = Point { x: s[0], y: s[1] };
            app.assembly_edge_at(app.layout(), point)
                .filter(|(pick, _)| pick.instance == 3)
                .map(|(_, edge)| (point, edge))
        })
        .expect("a visible edge");
    drop(scene);
    move_to(&mut app, target.0);
    let hovered = view(&app).hovered_edge.clone().expect("edge highlighted");
    assert_eq!(hovered.a, target.1.a);
    let painter = app.paint();
    drop(painter);
    let before = view(&app).asm.instances[3].transform().pose();
    key(&mut app, KeyCode::KeyJ);
    match view(&app).prompt {
        Some(AsmPrompt::MateKind { b, .. }) => assert!(b.edge),
        ref other => panic!("{other:?} {}", app.status_message),
    }
    key_text(&mut app, KeyCode::Digit2, Some("2"));
    let asm = &view(&app).asm;
    assert_eq!(asm.mates.len(), 3, "{}", app.status_message);
    let mate = asm.mates.last().unwrap();
    assert_eq!(mate.kind, puppycad::assembly::MateKind::Revolute);
    let connector = asm
        .connectors
        .iter()
        .find(|c| c.id == mate.b.connector_id)
        .unwrap();
    // The connector's Z is parallel to the edge (in part coordinates).
    let local = before.inverse();
    let direction = local.vector(std::array::from_fn(|i| {
        f64::from(hovered.b[i] - hovered.a[i])
    }));
    let z = connector.frame().axis(2);
    let cross = puppycad::assembly::v_cross(z, puppycad::assembly::v_unit(direction).unwrap());
    assert!(
        puppycad::assembly::v_len(cross) < 1e-6,
        "{z:?} {direction:?}"
    );
    let _ = std::fs::remove_dir_all(dir);
}
