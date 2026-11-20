use rikka_physics_geometry::{Cuboid, Sphere};
use rikka_physics_math::Vec3;
use rikka_physics_rigid::{Body, RigidWorld, ShapeDef, ShapeId};

const DT: f32 = 1.0 / 60.0;

struct Scene {
    world: RigidWorld,
    sensor: ShapeId,
    ball: ShapeId,
    quiet: ShapeId,
}

fn scene() -> Scene {
    let mut world = RigidWorld::builder().sleeping(false).build().unwrap();
    let zone = world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, 2.0, 0.0))
                .shape(ShapeDef::new(Cuboid::new(Vec3::splat(1.0))).sensor(true)),
        )
        .unwrap();
    let sensor = world.body_shapes(zone).unwrap().next().unwrap();
    let falling = |x: f32| Body::dynamic().position(Vec3::new(x, 5.0, 0.0));
    let ball_body = world
        .create_body(falling(0.0).shape(Sphere::new(0.25)))
        .unwrap();
    let quiet_body = world
        .create_body(falling(1.0).shape(ShapeDef::new(Sphere::new(0.25)).sensor_events(false)))
        .unwrap();
    let ball = world.body_shapes(ball_body).unwrap().next().unwrap();
    let quiet = world.body_shapes(quiet_body).unwrap().next().unwrap();
    Scene {
        world,
        sensor,
        ball,
        quiet,
    }
}

#[test]
fn sensor_reports_enter_and_exit_without_contact() {
    let Scene {
        mut world,
        sensor,
        ball,
        quiet,
    } = scene();
    let mut began = None;
    let mut ended = None;
    let mut inside = false;
    for step in 0..90 {
        world.step(DT);
        for event in world.sensor_begin_events() {
            assert_eq!(event.sensor, sensor);
            assert_ne!(event.visitor, quiet);
            assert_eq!(event.visitor, ball);
            began.get_or_insert(step);
        }
        for event in world.sensor_end_events() {
            assert_eq!((event.sensor, event.visitor), (sensor, ball));
            ended.get_or_insert(step);
        }
        let overlaps: Vec<_> = world.sensor_overlaps(sensor).unwrap().collect();
        if began.is_some() && ended.is_none() {
            assert_eq!(overlaps, [ball]);
            inside = true;
        }
    }
    let (began, ended) = (began.expect("no begin event"), ended.expect("no end event"));
    assert!(inside && began < ended);
    let ball_body = world.shape_body(ball).unwrap();
    assert!(
        world.pose(ball_body).unwrap().p.y < 0.0,
        "the sensor stopped the ball"
    );
    assert!(world.body_contacts(ball_body).unwrap().is_empty());
}
