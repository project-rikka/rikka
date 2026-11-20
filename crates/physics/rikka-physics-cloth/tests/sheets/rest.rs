use crate::support::Sheet;
use glam::{Mat4, Quat, Vec3};
use rikka_model_core::{NodeId, Transform};
use rikka_physics_cloth::Cloth;
use rikka_physics_tailor::PieceKind;

#[test]
fn resting_drape_matches_skinned_mesh() {
    let sheet = Sheet::new(7, 9, 0.03, PieceKind::Sheet);
    let mut cloth = crate::support::cloth(Cloth::builder().gravity(Vec3::ZERO));
    let figure = cloth
        .add_styled_figure(sheet.asset.clone(), [crate::support::style()])
        .expect("a figure");
    let mut instance = sheet.instance();
    let placement =
        Mat4::from_rotation_translation(Quat::from_rotation_z(0.4), Vec3::new(0.5, 1.0, -0.2));
    instance.set_transform(placement);
    let posed = Transform {
        translation: Vec3::new(0.0, 0.1, 0.05),
        rotation: Quat::from_rotation_x(0.7) * Quat::from_rotation_y(-0.3),
        scale: Vec3::ONE,
    };
    instance.pose_mut().set_local(NodeId(0), posed);
    instance.refresh();
    for frame in 0..20 {
        crate::support::step(&mut cloth, figure, &instance, frame);
    }
    let skinned = placement * posed.to_matrix();
    let drapes = cloth.drapes(figure).expect("drapes");
    assert_eq!(drapes.len(), 1);
    assert_eq!(drapes[0].node, NodeId(0));
    let draped = crate::support::draped(&drapes[0]);
    let normal = skinned.transform_vector3(Vec3::Z).normalize();
    let tangent = skinned.transform_vector3(Vec3::X).normalize();
    for (vertex, rest) in sheet.rest().iter().enumerate() {
        let expected = skinned.transform_point3(*rest);
        let got = &draped[vertex];
        assert!(
            got.position.distance(expected) < 1.0e-4,
            "vertex {vertex}: {} against {expected}",
            got.position
        );
        assert_eq!(got.blend, 1.0);
        assert!(
            got.normal.distance(normal) < 1.0e-3,
            "{} against {normal}",
            got.normal
        );
        assert!(got.tangent.truncate().distance(tangent) < 1.0e-3);
        assert_eq!(got.tangent.w, 1.0);
    }
}
