use super::*;
use manifold_rust::linalg::Vec3;

fn box_scene() -> SolidScene {
    SolidScene::from_manifolds(&[Manifold::cube(Vec3::new(20.0, 10.0, 5.0), false)])
}

#[test]
fn box_has_eight_vertices_twelve_edges_six_faces() {
    let topology = Topology::from_scene(&box_scene());
    assert_eq!(topology.vertices.len(), 8);
    assert_eq!(topology.edges.len(), 12);
    assert_eq!(topology.faces.len(), 6);
    assert!(topology.faces.iter().all(|face| face.planar));
    let area: f32 = topology.faces.iter().map(|face| face.area).sum();
    assert!((area - 2.0 * (200.0 + 100.0 + 50.0)).abs() < 1e-3, "{area}");
    assert!(topology.edges.iter().all(|edge| edge.solid.is_some()));
}

#[test]
fn cylinder_has_two_caps_one_curved_face_and_two_loops() {
    let scene = SolidScene::from_manifolds(&[Manifold::cylinder(20.0, 10.0, 10.0, 48)]);
    let topology = Topology::from_scene(&scene);
    assert_eq!(topology.faces.len(), 3);
    assert_eq!(topology.faces.iter().filter(|face| face.planar).count(), 2);
    let lens: Vec<_> = topology
        .edges
        .iter()
        .map(|e| (e.points.len(), e.closed, e.solid.is_some()))
        .collect();
    assert_eq!(topology.edges.len(), 2, "{lens:?}");
    assert!(topology.edges.iter().all(|edge| edge.closed));
    assert!(topology.vertices.is_empty());
    let (center, axis) = topology.edge_axis(0);
    assert!(axis[2].abs() > 0.999, "{axis:?}");
    assert!(center[0].abs() < 1e-3 && center[1].abs() < 1e-3);
    let curved = topology.faces.iter().find(|face| !face.planar).unwrap();
    // 48-gon prism side: perimeter * height.
    let side = 48.0 * 2.0 * 10.0 * (std::f32::consts::PI / 48.0).sin() * 20.0;
    assert!((curved.area - side).abs() < 1e-2, "{}", curved.area);
}

#[test]
fn cache_returns_the_same_topology_for_the_same_scene() {
    let scene = box_scene();
    let (a, first) = cached_topology(&scene);
    let (b, second) = cached_topology(&box_scene());
    assert_eq!(a, b);
    assert!(std::rc::Rc::ptr_eq(&first, &second));
}

#[test]
fn measures_the_box_diagonal_and_parallel_faces_and_edges() {
    let scene = box_scene();
    let topology = Topology::from_scene(&scene);
    let vertex =
        |p: [f32; 3]| TopoRef::Vertex(topology.vertices.iter().position(|v| v.point == p).unwrap());
    let diagonal = topology.measure(&scene, vertex([0.0; 3]), vertex([20.0, 10.0, 5.0]));
    assert!((diagonal.distance.unwrap() - 525f32.sqrt()).abs() < 1e-4);
    assert_eq!(diagonal.angle, None);
    let face = |n: [f32; 3]| {
        TopoRef::Face(
            topology
                .faces
                .iter()
                .position(|f| dot3(f.normal, n) > 0.999)
                .unwrap(),
        )
    };
    let caps = topology.measure(&scene, face([0.0, 0.0, 1.0]), face([0.0, 0.0, -1.0]));
    assert!((caps.distance.unwrap() - 5.0).abs() < 1e-4);
    assert!((caps.angle.unwrap() - 180.0).abs() < 1e-3);
    let corner = topology.measure(&scene, face([0.0, 0.0, 1.0]), face([1.0, 0.0, 0.0]));
    assert_eq!(corner.distance, None);
    assert!((corner.angle.unwrap() - 90.0).abs() < 1e-3);
    let to_face = topology.measure(&scene, vertex([0.0; 3]), face([1.0, 0.0, 0.0]));
    assert!((to_face.distance.unwrap() - 20.0).abs() < 1e-4);
    let x_edges: Vec<usize> = (0..topology.edges.len())
        .filter(|&i| topology.edge_direction(i).unwrap()[0].abs() > 0.999)
        .collect();
    assert_eq!(x_edges.len(), 4);
    let edges = topology.measure(&scene, TopoRef::Edge(x_edges[0]), TopoRef::Edge(x_edges[1]));
    let d = edges.distance.unwrap();
    assert!(
        [5.0f32, 10.0, 125f32.sqrt()]
            .iter()
            .any(|e| (d - e).abs() < 1e-4),
        "{d}"
    );
    assert!(edges.angle.unwrap().abs() < 1e-3);
    let lines = super::topology_view::measure_lines(diagonal);
    assert_eq!(lines, vec!["DIST  22.913 mm".to_string()]);
}

use super::input::KeyPress;
use winit::dpi::PhysicalPosition;
use winit::event::DeviceId;

fn key(app: &mut App, code: KeyCode) {
    app.handle_key(KeyPress {
        physical_key: PhysicalKey::Code(code),
        text: None,
    });
}

fn click(app: &mut App, point: Point, shift: bool) {
    app.handle_window_event(WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(point.x as f64, point.y as f64),
    });
    app.modifiers = if shift {
        ModifiersState::SHIFT
    } else {
        ModifiersState::empty()
    };
    for state in [ElementState::Pressed, ElementState::Released] {
        app.handle_window_event(WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: MouseButton::Left,
        });
    }
    app.modifiers = ModifiersState::empty();
}

fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "puppycad-topo-{name}-{}-{}.pcad",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// A 40 x 20 x 10 block in the part 3D view.
fn block_app(name: &str) -> (App, PathBuf) {
    let path = temp_path(name);
    let mut app = App::with_project(&path).unwrap();
    app.document.tool = Tool::Rectangle;
    app.document.draft_start = Some(Point { x: 0.0, y: 0.0 });
    assert!(app.document.finish_draft(Point { x: 40.0, y: 20.0 }));
    app.project
        .as_mut()
        .unwrap()
        .create_extrusion_for_profile(&app.document, 10.0, 0)
        .unwrap();
    app.solid_preview = true;
    (app, path)
}

fn screen_point(projector: &SolidProjector, world: [f32; 3]) -> Point {
    let [x, y] = projector.screen(world);
    Point { x, y }
}

/// A visible corner, a point 10 px along one of its visible edges and the
/// edge index.
fn corner_and_edge(app: &App) -> (usize, Point, usize, Point) {
    let (scene, projector, _, topology) = app.topology_context(app.layout()).unwrap();
    for (v, vertex) in topology.vertices.iter().enumerate() {
        if !projector.visible(&scene, vertex.point) {
            continue;
        }
        for (e, edge) in topology.edges.iter().enumerate() {
            let (a, b) = (edge.points[0], edge.points[1]);
            let far = if a == vertex.point {
                b
            } else if b == vertex.point {
                a
            } else {
                continue;
            };
            let (sa, sb) = (projector.screen(vertex.point), projector.screen(far));
            let length = ((sb[0] - sa[0]).powi(2) + (sb[1] - sa[1]).powi(2)).sqrt();
            if length < 60.0 {
                continue;
            }
            let t = 10.0 / length;
            let along: [f32; 3] =
                std::array::from_fn(|i| vertex.point[i] + (far[i] - vertex.point[i]) * t);
            if projector.visible(&scene, along) {
                return (
                    v,
                    screen_point(&projector, vertex.point),
                    e,
                    screen_point(&projector, along),
                );
            }
        }
    }
    panic!("no visible corner");
}

#[test]
fn pick_priority_is_vertex_then_edge_then_face() {
    let (app, path) = block_app("priority");
    let layout = app.layout();
    let (v, corner, e, on_edge) = corner_and_edge(&app);
    assert_eq!(
        app.topology_pick_at(layout, corner).unwrap().element,
        TopoRef::Vertex(v)
    );
    // 3 px off the corner is still the vertex (6 px tolerance).
    let near = Point {
        x: corner.x + 3.0,
        y: corner.y,
    };
    assert!(matches!(
        app.topology_pick_at(layout, near).unwrap().element,
        TopoRef::Vertex(_)
    ));
    assert_eq!(
        app.topology_pick_at(layout, on_edge).unwrap().element,
        TopoRef::Edge(e)
    );
    let (scene, projector, _, topology) = app.topology_context(layout).unwrap();
    let top = topology
        .faces
        .iter()
        .position(|face| face.normal[2] > 0.99)
        .unwrap();
    let triangle = topology.faces[top].triangles[0];
    let center: [f32; 3] =
        std::array::from_fn(|i| scene.triangles[triangle].iter().map(|p| p[i]).sum::<f32>() / 3.0);
    let pick = app
        .topology_pick_at(layout, screen_point(&projector, center))
        .unwrap();
    assert_eq!(pick.element, TopoRef::Face(top));
    let text = app.describe_pick(&scene, &topology, &pick);
    assert!(text.starts_with("Face top A=800.00"), "{text}");
    let _ = std::fs::remove_file(path);
}

#[test]
fn hidden_back_vertex_is_not_picked() {
    let (app, path) = block_app("occlusion");
    let layout = app.layout();
    let (scene, projector, _, topology) = app.topology_context(layout).unwrap();
    let hidden: Vec<usize> = (0..topology.vertices.len())
        .filter(|&v| !projector.visible(&scene, topology.vertices[v].point))
        .collect();
    assert!(!hidden.is_empty(), "the default view hides a back corner");
    for v in hidden {
        let point = screen_point(&projector, topology.vertices[v].point);
        let pick = app.topology_pick_at(layout, point);
        assert_ne!(pick.map(|p| p.element), Some(TopoRef::Vertex(v)));
    }
    let _ = std::fs::remove_file(path);
}

fn selected(app: &App) -> Vec<TopoRef> {
    app.topo_selection.iter().map(|p| p.element).collect()
}

#[test]
fn shift_click_multi_selects_and_escape_clears() {
    let (mut app, path) = block_app("multi");
    let (v, corner, e, on_edge) = corner_and_edge(&app);
    click(&mut app, corner, false);
    assert_eq!(selected(&app), vec![TopoRef::Vertex(v)]);
    assert!(
        app.status_message.starts_with("Selected Vertex ("),
        "{}",
        app.status_message
    );
    click(&mut app, on_edge, true);
    assert_eq!(selected(&app), vec![TopoRef::Vertex(v), TopoRef::Edge(e)]);
    let lines = app.topology_panel_lines();
    assert_eq!(lines[0], "2 SELECTED");
    let _ = app.paint();
    // Orbiting keeps the selection (ids belong to the scene, not the view).
    let view = app.solid_view;
    app.solid_view.rotate_drag(30.0, 10.0);
    assert_eq!(app.current_topology_selection().unwrap().2.len(), 2);
    app.solid_view = view;
    click(&mut app, on_edge, true);
    assert_eq!(selected(&app), vec![TopoRef::Vertex(v)]);
    click(&mut app, on_edge, false);
    assert_eq!(selected(&app), vec![TopoRef::Edge(e)]);
    key(&mut app, KeyCode::Escape);
    assert!(app.topo_selection.is_empty());
    assert!(app.solid_preview, "Esc only clears the selection");
    click(&mut app, corner, false);
    let viewport = app.layout().viewport;
    let empty = Point {
        x: viewport.x + 5.0,
        y: viewport.y + viewport.h - 5.0,
    };
    click(&mut app, empty, false);
    assert!(app.topo_selection.is_empty());
    let _ = std::fs::remove_file(path);
}

/// Screen points of `count` visible straight edges whose pick is the edge.
fn edge_points(app: &App, count: usize) -> Vec<(usize, Point)> {
    let layout = app.layout();
    let (_, projector, _, topology) = app.topology_context(layout).unwrap();
    let found: Vec<(usize, Point)> = topology
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            let mid = std::array::from_fn(|i| (edge.points[0][i] + edge.points[1][i]) * 0.5);
            let point = screen_point(&projector, mid);
            (app.topology_pick_at(layout, point).map(|pick| pick.element)
                == Some(TopoRef::Edge(index)))
            .then_some((index, point))
        })
        .take(count)
        .collect();
    assert_eq!(found.len(), count);
    found
}

fn volume(app: &App) -> f64 {
    app.project.as_ref().unwrap().evaluated_solids().unwrap()[0].volume()
}

#[test]
fn chamfer_from_selection_matches_the_click_to_pick_flow() {
    let (mut selected, first) = block_app("chamfer-selected");
    let edges = edge_points(&selected, 2);
    click(&mut selected, edges[0].1, false);
    click(&mut selected, edges[1].1, true);
    key(&mut selected, KeyCode::Digit0);
    let pick = selected.edge_pick.as_ref().expect("chamfer armed");
    assert_eq!(pick.edges.len(), 2, "{}", selected.status_message);
    key(&mut selected, KeyCode::Enter);
    assert!(selected.edge_pick.is_none(), "{}", selected.status_message);

    let (mut picked, second) = block_app("chamfer-picked");
    picked.begin_edge_finish(project::FinishKind::Chamfer);
    for (_, point) in &edges {
        picked.toggle_edge_at(picked.layout(), *point);
    }
    key(&mut picked, KeyCode::Enter);
    assert!(picked.edge_pick.is_none(), "{}", picked.status_message);
    assert!((volume(&selected) - volume(&picked)).abs() < 1e-6);
    assert!(volume(&selected) < 8000.0 - 1.0);
    let _ = std::fs::remove_file(first);
    let _ = std::fs::remove_file(second);
}

#[test]
fn delete_with_a_topology_selection_removes_nothing() {
    let (mut app, path) = block_app("delete");
    let edges = edge_points(&app, 1);
    click(&mut app, edges[0].1, false);
    key(&mut app, KeyCode::Delete);
    assert_eq!(app.status_message, "Select a feature row to delete");
    assert!((volume(&app) - 8000.0).abs() < 1e-3);
    let _ = std::fs::remove_file(path);
}

#[test]
fn key_4_sketches_on_the_selected_face_not_the_hovered_one() {
    let (mut app, path) = block_app("face");
    let layout = app.layout();
    let (scene, projector, _, topology) = app.topology_context(layout).unwrap();
    let top = topology
        .faces
        .iter()
        .position(|face| face.normal[2] > 0.99)
        .unwrap();
    let triangle = topology.faces[top].triangles[0];
    let center: [f32; 3] =
        std::array::from_fn(|i| scene.triangles[triangle].iter().map(|p| p[i]).sum::<f32>() / 3.0);
    drop((scene, topology));
    click(&mut app, screen_point(&projector, center), false);
    let viewport = layout.viewport;
    app.cursor = Point {
        x: viewport.x + 5.0,
        y: viewport.y + viewport.h - 5.0,
    };
    app.sketch_on_face();
    assert!(!app.solid_preview, "{}", app.status_message);
    assert!(
        app.status_message.contains("top face"),
        "{}",
        app.status_message
    );
    let _ = std::fs::remove_file(path);
}
