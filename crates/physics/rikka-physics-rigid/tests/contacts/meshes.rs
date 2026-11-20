use rikka_physics_collision::Material;
use rikka_physics_geometry::{Cuboid, MeshShape, Sphere, TriangleMesh};
use rikka_physics_math::Vec3;
use rikka_physics_rigid::{Body, RigidWorld, ShapeDef};

#[test]
fn mesh_contact_has_one_manifold_per_material() {
    let mut world = RigidWorld::builder().sleeping(false).build().unwrap();
    let mesh = MeshShape::new(TriangleMesh::grid(8, 8, 1.0, 2).unwrap());
    world
        .create_body(Body::fixed().shape(ShapeDef::new(mesh).materials([
            Material::default().user_id(1),
            Material::default().user_id(2),
        ])))
        .unwrap();
    let crate_box = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 0.8, 0.0))
                .shape(Cuboid::new(Vec3::splat(0.6))),
        )
        .unwrap();
    let ball = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(2.5, 1.0, 2.5))
                .shape(Sphere::new(0.3)),
        )
        .unwrap();
    for _ in 0..120 {
        world.step(1.0 / 60.0);
    }
    let pose = world.pose(crate_box).unwrap();
    assert!((pose.p.y - 0.6).abs() < 0.02, "box at {:?}", pose.p);
    assert!(world.linear_velocity(crate_box).unwrap().length() < 0.02);
    assert!((world.pose(ball).unwrap().p.y - 0.3).abs() < 0.02);
    let contacts = world.body_contacts(crate_box).unwrap();
    let manifolds: Vec<_> = contacts
        .iter()
        .flat_map(|contact| contact.manifolds.iter())
        .collect();
    assert!(manifolds.len() >= 2, "{} manifolds", manifolds.len());
    let users: std::collections::BTreeSet<_> = manifolds
        .iter()
        .map(|manifold| manifold.material.user_a.max(manifold.material.user_b))
        .collect();
    assert_eq!(users.into_iter().collect::<Vec<_>>(), [1, 2]);
}
