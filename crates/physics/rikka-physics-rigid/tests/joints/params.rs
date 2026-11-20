use rikka_physics_geometry::Sphere;
use rikka_physics_math::{Quat, Transform, Vec3};
use rikka_physics_rigid::{Body, BodyId, D6, Error, Motion, RigidWorld, ShapeDef, Spherical};

fn world() -> (RigidWorld, BodyId, BodyId) {
    let mut world = RigidWorld::builder().build().unwrap();
    let a = world.create_body(Body::fixed()).unwrap();
    let b = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, -1.0, 0.0))
                .shape(ShapeDef::new(Sphere::new(0.2))),
        )
        .unwrap();
    (world, a, b)
}

#[test]
fn revision_counts_parameter_changes() {
    let (mut world, a, b) = world();
    let joint = world
        .create_joint(D6::new(a, b).angular(Motion::Free))
        .unwrap();
    assert_eq!(world.joint_revision(joint).unwrap(), 0);
    world.step(1.0 / 60.0);
    assert_eq!(world.joint_revision(joint).unwrap(), 0);
    world
        .joint_mut::<D6>(joint)
        .unwrap()
        .set_target(Transform::from_rotation(Quat::from_rotation_z(0.5)));
    assert_eq!(world.joint_revision(joint).unwrap(), 1);
    assert!(matches!(
        world.joint_mut::<Spherical>(joint),
        Err(Error::JointKind(id)) if id == joint
    ));
    assert_eq!(world.joint_revision(joint).unwrap(), 1);
    world.joint_mut::<D6>(joint).unwrap();
    assert_eq!(world.joint_revision(joint).unwrap(), 2);
}
