use super::solver::{apply, revolute_angle, solve};
use super::*;

fn close(a: V3, b: V3) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
}

/// Two unit-scale instances, `a` grounded at the origin and `b` placed
/// elsewhere, with one connector on each.
fn pair(kind: MateKind, ca: (V3, V3), cb: (V3, V3), b_at: Transform) -> Assembly {
    let data = json!({
        "id": "asm", "name": "Asm",
        "instances": [
            {"id": "a", "partId": "p"},
            {"id": "b", "partId": "p", "transform": {
                "translation": vec3_json(b_at.translation),
                "rotation": vec3_json(b_at.rotation)}}
        ],
        "connectors": [
            {"id": "ca", "instanceId": "a", "position": vec3_json(ca.0), "rotation": vec3_json(ca.1)},
            {"id": "cb", "instanceId": "b", "position": vec3_json(cb.0), "rotation": vec3_json(cb.1)}
        ],
        "mates": [{"id": "m", "type": kind.name(),
            "a": {"instanceId": "a", "connectorId": "ca"},
            "b": {"instanceId": "b", "connectorId": "cb"}}]
    });
    let asm = Assembly::from_json(Some(&data), "x", "X");
    assert_eq!(asm.read_only_reason, None);
    asm
}

fn world_frames(asm: &Assembly) -> (Pose, Pose) {
    let pose = |id: &str| asm.instance(id).unwrap().transform().pose();
    let frame = |id: &str| asm.connectors.iter().find(|c| c.id == id).unwrap().frame();
    (pose("a").mul(&frame("ca")), pose("b").mul(&frame("cb")))
}

fn away() -> Transform {
    Transform {
        translation: [30.0, -12.0, 7.0],
        rotation: [10.0, 20.0, 35.0],
        scale: [1.0; 3],
    }
}

#[test]
fn euler_matches_three_js_round_trip() {
    for r in [[10.0, 20.0, 30.0], [-45.0, 80.0, 170.0], [0.0, 0.0, -90.0]] {
        let pose = Pose::from_parts([0.0; 3], r);
        assert!(close(pose.rotation_deg(), r), "{r:?}");
    }
    // Rotating +X by 90° about Z gives +Y.
    let pose = Pose::from_parts([0.0; 3], [0.0, 0.0, 90.0]);
    assert!(close(pose.vector([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
}

#[test]
fn json_round_trips_and_keeps_unknown_payloads() {
    let data = json!({
        "id": "asm", "name": "Robot",
        "instances": [{"id": "i1", "partId": "p1", "name": "Base",
            "transform": {"translation": {"x": 1, "y": 2, "z": 3}}}],
        "connectors": [{"id": "c1", "instanceId": null, "position": {"x": 0, "y": 0, "z": 0}}],
        "mates": [],
        "actuators": [{"id": "s", "type": "servo", "mateId": "nope", "homeDeg": 0,
            "commandRange": {"minDeg": -90, "maxDeg": 90}}],
        "variables": {"gap": 2}
    });
    let asm = Assembly::from_json(Some(&data), "d", "D");
    assert_eq!(asm.read_only_reason, None);
    assert_eq!(asm.instances[0].label(), "Base");
    assert_eq!(asm.instances[0].transform().translation, [1.0, 2.0, 3.0]);
    assert_eq!(asm.connectors[0].instance_id, None);
    let again = Assembly::from_json(Some(&asm.to_json()), "d", "D");
    assert_eq!(again, asm);
    assert_eq!(asm.to_json()["variables"], json!({"gap": 2}));
    assert_eq!(asm.to_json()["actuators"][0]["id"], "s");
}

#[test]
fn unsupported_mate_type_is_read_only() {
    let data = json!({
        "instances": [{"id": "a", "partId": "p"}, {"id": "b", "partId": "p"}],
        "connectors": [
            {"id": "ca", "instanceId": "a", "position": {"x": 0, "y": 0, "z": 0}},
            {"id": "cb", "instanceId": "b", "position": {"x": 0, "y": 0, "z": 0}}],
        "mates": [{"id": "m", "type": "gear",
            "a": {"instanceId": "a", "connectorId": "ca"},
            "b": {"instanceId": "b", "connectorId": "cb"}}]
    });
    let asm = Assembly::from_json(Some(&data), "d", "D");
    assert!(
        asm.read_only_reason
            .unwrap()
            .contains("unsupported type \"gear\"")
    );
}

#[test]
fn fasten_makes_frames_coincide() {
    let mut asm = pair(
        MateKind::Fasten,
        ([0.0, 0.0, 10.0], [0.0; 3]),
        ([5.0, 0.0, 0.0], [0.0, 0.0, 90.0]),
        away(),
    );
    apply(&mut asm).unwrap();
    let (fa, fb) = world_frames(&asm);
    assert!(fa.max_difference(&fb) < 1e-9);
    assert_eq!(
        asm.instance("a").unwrap().transform(),
        Transform::default(),
        "root keeps its pose"
    );
}

#[test]
fn revolute_shares_axis_and_keeps_free_angle_or_servo() {
    let mut asm = pair(
        MateKind::Revolute,
        ([0.0, 10.0, 5.0], [0.0; 3]),
        ([0.0, -10.0, 0.0], [0.0; 3]),
        away(),
    );
    apply(&mut asm).unwrap();
    let (fa, fb) = world_frames(&asm);
    assert!(close(fa.t, fb.t));
    assert!(close(fa.axis(2), fb.axis(2)));
    asm.set_servo_angle("m", 40.0);
    apply(&mut asm).unwrap();
    let (fa, fb) = world_frames(&asm);
    assert!(close(fa.t, fb.t));
    assert!((revolute_angle(&fa, &fb) - 40.0).abs() < 1e-4);
}

#[test]
fn prismatic_slides_on_shared_axis() {
    let mut asm = pair(
        MateKind::Prismatic,
        ([0.0; 3], [0.0; 3]),
        ([0.0; 3], [0.0; 3]),
        away(),
    );
    apply(&mut asm).unwrap();
    let (fa, fb) = world_frames(&asm);
    let d = v_sub(fb.t, fa.t);
    assert!(v_dot(d, fa.axis(0)).abs() < 1e-5 && v_dot(d, fa.axis(1)).abs() < 1e-5);
    assert!(fa.max_difference(&Pose { t: fa.t, ..fb }) < 1e-5);
    // The free offset along Z stays (no reason to collapse it).
    assert!(v_dot(d, fa.axis(2)).abs() > 1e-3);
}

#[test]
fn planar_keeps_frames_on_one_plane() {
    let mut asm = pair(
        MateKind::Planar,
        ([0.0, 0.0, 10.0], [0.0; 3]),
        ([0.0; 3], [0.0; 3]),
        away(),
    );
    apply(&mut asm).unwrap();
    let (fa, fb) = world_frames(&asm);
    assert!(close(fa.axis(2), fb.axis(2)));
    assert!(v_dot(v_sub(fb.t, fa.t), fa.axis(2)).abs() < 1e-5);
}

#[test]
fn ball_shares_origin() {
    let mut asm = pair(
        MateKind::Ball,
        ([3.0, 4.0, 5.0], [0.0; 3]),
        ([-1.0, 0.0, 2.0], [0.0; 3]),
        away(),
    );
    apply(&mut asm).unwrap();
    let (fa, fb) = world_frames(&asm);
    assert!(close(fa.t, fb.t));
}

#[test]
fn chain_of_three_fasten_then_revolute() {
    let data = json!({
        "instances": [
            {"id": "base", "partId": "p"},
            {"id": "arm", "partId": "p", "transform": {"translation": {"x": 50, "y": 0, "z": 0}}},
            {"id": "tip", "partId": "p", "transform": {"translation": {"x": -40, "y": 9, "z": 1}}}
        ],
        "connectors": [
            {"id": "base-top", "instanceId": "base", "position": {"x": 0, "y": 0, "z": 10}},
            {"id": "arm-bottom", "instanceId": "arm", "position": {"x": 0, "y": 0, "z": 0}},
            {"id": "arm-end", "instanceId": "arm", "position": {"x": 0, "y": 15, "z": 5}},
            {"id": "tip-pin", "instanceId": "tip", "position": {"x": 0, "y": -15, "z": 0}}
        ],
        "mates": [
            {"id": "m1", "type": "fasten", "a": {"instanceId": "base", "connectorId": "base-top"},
                "b": {"instanceId": "arm", "connectorId": "arm-bottom"}},
            {"id": "m2", "type": "revolute", "a": {"instanceId": "arm", "connectorId": "arm-end"},
                "b": {"instanceId": "tip", "connectorId": "tip-pin"}}
        ],
        "actuators": [{"id": "s1", "type": "servo", "mateId": "m2", "homeDeg": 30,
            "commandRange": {"minDeg": -90, "maxDeg": 90}}]
    });
    let mut asm = Assembly::from_json(Some(&data), "d", "D");
    apply(&mut asm).unwrap();
    let t = |id: &str| asm.instance(id).unwrap().transform();
    assert!(close(t("arm").translation, [0.0, 0.0, 10.0]));
    assert!(close(
        t("tip").translation,
        [-7.5, 15.0 + 15.0 * 30f64.to_radians().cos(), 15.0]
    ));
    assert!(close(t("tip").rotation, [0.0, 0.0, 30.0]));
}

#[test]
fn conflicting_fasten_loop_is_reported() {
    let data = json!({
        "instances": [{"id": "a", "partId": "p"}, {"id": "b", "partId": "p"}],
        "connectors": [
            {"id": "a1", "instanceId": "a", "position": {"x": 0, "y": 0, "z": 0}},
            {"id": "a2", "instanceId": "a", "position": {"x": 5, "y": 0, "z": 0}},
            {"id": "b1", "instanceId": "b", "position": {"x": 0, "y": 0, "z": 0}}],
        "mates": [
            {"id": "m1", "type": "fasten", "a": {"instanceId": "a", "connectorId": "a1"},
                "b": {"instanceId": "b", "connectorId": "b1"}},
            {"id": "m2", "type": "fasten", "a": {"instanceId": "a", "connectorId": "a2"},
                "b": {"instanceId": "b", "connectorId": "b1"}}]
    });
    let asm = Assembly::from_json(Some(&data), "d", "D");
    assert!(solve(&asm).unwrap_err().contains("Conflicting"));
}

/// Instance transforms the web loader (`normalizeProjectFile` →
/// `solveFixedAssembly`) produces for `fixtures/assembly.pcad`; the same
/// numbers are asserted in `test/rust-roundtrip.test.ts`.
const WEB_FIXTURE_TRANSFORMS: [(&str, V3, V3); 3] = [
    ("base-1", [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
    ("arm-1", [0.0, 0.0, 10.0], [0.0, 0.0, 0.0]),
    ("arm-2", [-7.5, 27.990381056766577, 15.0], [0.0, 0.0, 30.0]),
];

#[test]
fn fixture_solves_like_the_web_loader() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/assembly.pcad");
    let (project, document) = crate::project::ProjectFile::open(&path).unwrap();
    let index = project.assembly_indices()[0];
    let mut asm = project.assembly_at(index).unwrap();
    assert_eq!(asm.read_only_reason, None);
    assert_eq!(asm.instances.len(), 3);
    apply(&mut asm).unwrap();
    for (id, translation, rotation) in WEB_FIXTURE_TRANSFORMS {
        let t = asm.instance(id).unwrap().transform();
        assert!(
            (0..3).all(|i| (t.translation[i] - translation[i]).abs() < 1e-3
                && (t.rotation[i] - rotation[i]).abs() < 1e-3),
            "{id}: {t:?}"
        );
    }
    let bodies = project.part_bodies("part-arm", &document).unwrap();
    assert_eq!(bodies.len(), 1);
    assert!(!bodies[0].triangles.is_empty());
}

#[test]
fn removing_an_instance_drops_its_mates_and_servos() {
    let mut asm = pair(
        MateKind::Revolute,
        ([0.0; 3], [0.0; 3]),
        ([0.0; 3], [0.0; 3]),
        away(),
    );
    asm.set_servo_angle("m", 10.0);
    asm.remove_instance(1);
    assert!(asm.mates.is_empty() && asm.actuators.is_empty());
    assert_eq!(asm.connectors.len(), 1);
}

#[test]
fn remove_mate_drops_its_servo_and_unused_connectors() {
    let mut asm = pair(
        MateKind::Revolute,
        ([0.0; 3], [0.0; 3]),
        ([0.0; 3], [0.0; 3]),
        away(),
    );
    asm.set_servo_angle("m", 30.0);
    // A second mate sharing both connectors keeps them alive.
    asm.add_mate(
        MateKind::Ball,
        MateRef {
            instance_id: Some("a".into()),
            connector_id: "ca".into(),
        },
        MateRef {
            instance_id: Some("b".into()),
            connector_id: "cb".into(),
        },
    );
    assert!(asm.remove_mate("missing").is_none());
    assert_eq!(asm.remove_mate("m").unwrap().kind, MateKind::Revolute);
    assert!(asm.actuators.is_empty());
    assert_eq!(asm.connectors.len(), 2, "still used by the ball mate");
    let ball = asm.mates[0].id.clone();
    asm.remove_mate(&ball);
    assert!(asm.mates.is_empty());
    assert!(asm.connectors.is_empty());
}
