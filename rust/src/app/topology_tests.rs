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
