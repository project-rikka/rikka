use rikka_physics_collision::{Filter, Material};
use rikka_physics_geometry::{
    Capsule, Compound, CompoundChild, ConvexHull, Cuboid, MeshShape, Sphere, TriangleMesh,
};
use rikka_physics_math::{Quat, Transform, Vec3};
use rikka_physics_rigid::{AxisLocks, Body, RigidWorld, ShapeDef};

fn body() -> Body {
    let hull = ConvexHull::new(&[
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.7, 0.7, 0.7),
    ])
    .unwrap()
    .rounded(0.05);
    let compound = Compound::build(&[
        CompoundChild::new(Sphere::new(0.2), Transform::from_translation(Vec3::X)),
        CompoundChild::new(Capsule::new(0.1, 0.3), Transform::IDENTITY),
    ])
    .unwrap();
    Body::dynamic()
        .pose(Transform::new(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::from_rotation_y(0.4),
        ))
        .linear_velocity(Vec3::new(0.5, -1.0, 2.0))
        .angular_velocity(Vec3::new(0.0, 3.0, 0.0))
        .damping(0.1, 0.2)
        .gravity_scale(0.5)
        .locks(AxisLocks {
            linear: [false, true, false],
            angular: [true, false, true],
        })
        .bullet(true)
        .sleep_threshold(0.3)
        .user_data(77)
        .mass(12.0)
        .shape(
            ShapeDef::new(hull)
                .materials([
                    Material::default().friction(0.3),
                    Material::default().user_id(9),
                ])
                .filter(Filter::default().category(4).mask(6).group(-2))
                .density(700.0)
                .hit_events(true),
        )
        .shape(ShapeDef::new(Cuboid::new(Vec3::new(0.5, 0.2, 0.1))).sensor(true))
        .shape(ShapeDef::new(Sphere::new(0.1)).contact_response(false))
        .shape(compound)
        .shape(MeshShape::new(TriangleMesh::grid(2, 2, 1.0, 1).unwrap()))
}

#[test]
fn body_def_round_trips() {
    let original = body();
    let bytes = original.to_bytes();
    let decoded = Body::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.to_bytes(), bytes);
    let mut world = RigidWorld::builder().build().unwrap();
    let a = world.create_body(original).unwrap();
    let b = world.create_body(decoded).unwrap();
    assert_eq!(world.mass(a).unwrap(), world.mass(b).unwrap());
    let shapes_a: Vec<_> = world.body_shapes(a).unwrap().collect();
    let shapes_b: Vec<_> = world.body_shapes(b).unwrap().collect();
    for (x, y) in shapes_a.iter().zip(&shapes_b) {
        assert_eq!(
            world.shape_def(*x).unwrap().to_bytes(),
            world.shape_def(*y).unwrap().to_bytes()
        );
        assert_eq!(
            world.shape_materials(*x).unwrap(),
            world.shape_materials(*y).unwrap()
        );
        assert_eq!(
            world.shape_filter(*x).unwrap(),
            world.shape_filter(*y).unwrap()
        );
    }
}

#[test]
fn malformed_bytes_are_rejected() {
    let bytes = body().to_bytes();
    for cut in [0, 1, 20, bytes.len() / 2, bytes.len() - 1] {
        assert!(Body::from_bytes(&bytes[..cut]).is_err(), "cut at {cut}");
    }
    let mut extra = bytes;
    extra.push(0);
    assert!(Body::from_bytes(&extra).is_err());
    let mut kind = body().to_bytes();
    kind[0] = 99;
    assert!(Body::from_bytes(&kind).is_err());
}

#[test]
fn joint_defs_round_trip() {
    use rikka_physics_rigid::{
        D6, Distance, FilterJoint, JointDef, Motor, Parallel, Prismatic, Revolute, Spherical,
        Spring, Weld, Wheel,
    };
    let mut world = RigidWorld::builder().build().unwrap();
    let a = world
        .create_body(Body::dynamic().shape(Sphere::new(0.5)))
        .unwrap();
    let b = world
        .create_body(Body::dynamic().position(Vec3::X).shape(Sphere::new(0.5)))
        .unwrap();
    let frame = Transform::from_translation(Vec3::new(0.5, 0.0, 0.0));
    let defs: Vec<JointDef> = vec![
        Spherical::new(a, b).frame(frame).cone(0.4).into(),
        Weld::new(a, b)
            .frame(frame)
            .softness(Spring::hertz(3.0))
            .into(),
        Revolute::new(a, b).frame(frame).motor(2.0, 5.0).into(),
        Prismatic::new(a, b).frame(frame).into(),
        Wheel::new(a, b).frame(frame).into(),
        Motor::new(a, b).into(),
        Parallel::new(a, b).into(),
        D6::new(a, b).frame(frame).into(),
        Distance::new(a, b).break_force(10.0).into(),
        FilterJoint::new(a, b).user_data(5).into(),
    ];
    for def in defs {
        let bytes = def.to_bytes();
        let decoded = JointDef::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, def);
        let joint = world.create_joint(decoded).unwrap();
        assert_eq!(world.joint_bodies(joint).unwrap(), (a, b));
        let stored = world.joint_def(joint).unwrap();
        assert_eq!(world.joint_kind(joint).unwrap(), {
            let other = world.create_joint(stored).unwrap();
            world.joint_kind(other).unwrap()
        });
        assert!(JointDef::from_bytes(&bytes[..bytes.len() - 1]).is_err());
    }
}
