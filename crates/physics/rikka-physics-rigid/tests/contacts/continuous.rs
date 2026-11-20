use rikka_physics_geometry::{Cuboid, MeshShape, Sphere, TriangleMesh};
use rikka_physics_math::Vec3;
use rikka_physics_rigid::{Body, BodyId, RigidWorld, ShapeDef};

const DT: f32 = 1.0 / 60.0;

fn drop_ball(continuous: bool, floor: ShapeDef) -> (RigidWorld, BodyId) {
    let mut world = RigidWorld::builder()
        .continuous(continuous)
        .sleeping(false)
        .build()
        .unwrap();
    world.create_body(Body::fixed().shape(floor)).unwrap();
    let ball = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.3, 3.0, 0.2))
                .linear_velocity(Vec3::new(0.0, -300.0, 0.0))
                .shape(Sphere::new(0.1)),
        )
        .unwrap();
    (world, ball)
}

fn thin_box() -> ShapeDef {
    ShapeDef::new(Cuboid::new(Vec3::new(4.0, 0.02, 4.0)))
}

fn mesh_floor() -> ShapeDef {
    ShapeDef::new(MeshShape::new(TriangleMesh::grid(8, 8, 1.0, 0).unwrap()))
}

fn settle(world: &mut RigidWorld, ball: BodyId) -> f32 {
    let mut lowest = f32::MAX;
    for _ in 0..90 {
        world.step(DT);
        lowest = lowest.min(world.pose(ball).unwrap().p.y);
    }
    lowest
}

#[test]
fn fast_sphere_stops_on_thin_box() {
    let (mut world, ball) = drop_ball(true, thin_box());
    let lowest = settle(&mut world, ball);
    assert!(lowest > 0.0, "tunnelled to {lowest}");
    let y = world.pose(ball).unwrap().p.y;
    assert!((y - 0.12).abs() < 0.03, "rests at {y}");
}

#[test]
fn fast_sphere_stops_on_mesh() {
    let (mut world, ball) = drop_ball(true, mesh_floor());
    let lowest = settle(&mut world, ball);
    assert!(lowest > -0.05, "tunnelled to {lowest}");
    let y = world.pose(ball).unwrap().p.y;
    assert!((y - 0.1).abs() < 0.03, "rests at {y}");
}
