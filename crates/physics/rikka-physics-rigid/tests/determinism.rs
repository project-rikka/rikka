use rikka_physics_geometry::{Capsule, Cuboid, Sphere};
use rikka_physics_math::{Quat, Transform, Vec3};
use rikka_physics_rigid::{Body, RigidWorld, Spherical, Weld};

fn scene(workers: usize) -> RigidWorld {
    let mut world = RigidWorld::builder().workers(workers).build().unwrap();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -1.0, 0.0))
                .shape(Cuboid::new(Vec3::new(30.0, 1.0, 30.0))),
        )
        .unwrap();
    for pile in 0..4 {
        for i in 0..6 {
            for j in i..6 {
                let x = pile as f32 * 8.0 - 12.0 + (i as f32 + 1.0) * 0.5 + (j - i) as f32;
                world
                    .create_body(
                        Body::dynamic()
                            .position(Vec3::new(x, 0.5 + i as f32, 0.0))
                            .shape(Cuboid::new(Vec3::splat(0.5))),
                    )
                    .unwrap();
            }
        }
    }
    for k in 0..40 {
        let angle = k as f32 * 0.7;
        world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(
                        angle.cos() * 5.0,
                        8.0 + k as f32 * 0.6,
                        angle.sin() * 5.0,
                    ))
                    .rotation(Quat::from_rotation_z(angle))
                    .shape(if k % 2 == 0 {
                        Sphere::new(0.35).into()
                    } else {
                        rikka_physics_rigid::ShapeDef::new(Capsule::new(0.25, 0.4))
                    }),
            )
            .unwrap();
    }
    let anchor = world
        .create_body(Body::fixed().position(Vec3::new(0.0, 12.0, -6.0)))
        .unwrap();
    let mut previous = anchor;
    for link in 0..10 {
        let body = world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(link as f32 * 0.5 + 0.5, 12.0, -6.0))
                    .shape(Sphere::new(0.2)),
            )
            .unwrap();
        let frame = Transform::from_translation(Vec3::new(link as f32 * 0.5 + 0.25, 12.0, -6.0));
        if link % 3 == 2 {
            world
                .create_joint(Weld::new(previous, body).frame(frame))
                .unwrap();
        } else {
            world
                .create_joint(Spherical::new(previous, body).frame(frame).cone(0.6))
                .unwrap();
        }
        previous = body;
    }
    for k in 0..12 {
        let x = k as f32 * 2.0 - 11.0;
        world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(x, 15.0, 4.0))
                    .linear_velocity(Vec3::new(0.0, -250.0, 0.0))
                    .bullet(k % 2 == 0)
                    .shape(Sphere::new(0.15)),
            )
            .unwrap();
    }
    world
        .create_body(Body::fixed().position(Vec3::new(0.0, 1.5, 4.0)).shape(
            rikka_physics_rigid::ShapeDef::new(Cuboid::new(Vec3::new(12.0, 1.0, 1.0))).sensor(true),
        ))
        .unwrap();
    world
}

fn run(workers: usize) -> Vec<(u64, Vec<(u32, u32)>)> {
    let mut world = scene(workers);
    (0..120)
        .map(|_| {
            world.step(1.0 / 60.0);
            let sensed = world
                .sensor_begin_events()
                .iter()
                .map(|event| (event.visitor.index, 0))
                .chain(
                    world
                        .sensor_end_events()
                        .iter()
                        .map(|event| (event.visitor.index, 1)),
                )
                .collect();
            (world.state_hash(), sensed)
        })
        .collect()
}

#[test]
fn state_hash_matches_across_worker_counts() {
    let serial = run(1);
    assert_eq!(serial, run(1), "repeated serial runs differ");
    for workers in [2, 8] {
        assert_eq!(serial, run(workers), "{workers} workers differ from one");
    }
}
