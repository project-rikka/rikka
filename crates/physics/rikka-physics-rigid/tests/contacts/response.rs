use rikka_physics_collision::{Material, QueryFilter};
use rikka_physics_geometry::{Cuboid, Sphere};
use rikka_physics_math::Vec3;
use rikka_physics_rigid::{Body, RigidWorld, ShapeDef};

#[test]
fn shape_without_response_passes_through_but_is_queryable() {
    let mut world = RigidWorld::builder().build().unwrap();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -0.5, 0.0))
                .shape(Cuboid::new(Vec3::new(5.0, 0.5, 5.0))),
        )
        .unwrap();
    let ghost = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(-2.0, 1.0, 0.0))
                .shape(ShapeDef::new(Sphere::new(0.25)).contact_response(false)),
        )
        .unwrap();
    let solid = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(2.0, 1.0, 0.0))
                .shape(Sphere::new(0.25)),
        )
        .unwrap();
    for _ in 0..120 {
        world.step(1.0 / 60.0);
    }
    assert!(world.pose(ghost).unwrap().p.y < -2.0);
    assert!((world.pose(solid).unwrap().p.y - 0.25).abs() < 0.05);

    let ghost_height = world.pose(ghost).unwrap().p.y;
    let hit = world
        .cast_ray_closest(
            Vec3::new(-2.0, ghost_height + 0.6, 0.0),
            Vec3::new(0.0, -1.0, 0.0),
            QueryFilter::default(),
        )
        .unwrap();
    assert_eq!(hit.body, ghost);
}

#[test]
fn rolling_resistance_clamps_and_decelerates_both_light_and_heavy_spheres() {
    for (density, resistance, initial_speed) in [(1.0f32, 0.2f32, 4.0f32), (200.0, 0.8, 6.0)] {
        let mut world = RigidWorld::builder().sleeping(false).build().unwrap();
        world
            .create_body(
                Body::fixed().position(Vec3::new(0.0, -0.5, 0.0)).shape(
                    ShapeDef::new(Cuboid::new(Vec3::new(100.0, 0.5, 10.0)))
                        .material(Material::default().friction(0.8)),
                ),
            )
            .unwrap();
        let radius = 0.5;
        let damped = world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(0.0, radius, -2.0))
                    .linear_velocity(Vec3::new(initial_speed, 0.0, 0.0))
                    .angular_velocity(Vec3::new(0.0, 0.0, -initial_speed / radius))
                    .shape(
                        ShapeDef::new(Sphere::new(radius))
                            .density(density)
                            .material(
                                Material::default()
                                    .friction(0.8)
                                    .rolling_resistance(resistance),
                            ),
                    ),
            )
            .unwrap();
        let free = world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(0.0, radius, 2.0))
                    .linear_velocity(Vec3::new(initial_speed, 0.0, 0.0))
                    .angular_velocity(Vec3::new(0.0, 0.0, -initial_speed / radius))
                    .shape(
                        ShapeDef::new(Sphere::new(radius))
                            .density(density)
                            .material(Material::default().friction(0.8).rolling_resistance(0.0)),
                    ),
            )
            .unwrap();
        let mut previous_speed = initial_speed + 0.1;
        for step in 0..360 {
            world.step(1.0 / 60.0);
            let speed = world.linear_velocity(damped).unwrap().length();
            let spin = world.angular_velocity(damped).unwrap().length();
            assert!(
                speed.is_finite() && spin.is_finite(),
                "density={density} step={step}"
            );
            assert!(
                speed <= previous_speed + 0.05,
                "density={density} step={step} speed={speed} prev={previous_speed}"
            );
            previous_speed = speed;
        }
        let damped_speed = world.linear_velocity(damped).unwrap().length();
        let free_speed = world.linear_velocity(free).unwrap().length();
        assert!(
            damped_speed < 0.05,
            "density={density} damped_speed={damped_speed}"
        );
        assert!(
            free_speed > 0.5 * initial_speed,
            "density={density} free_speed={free_speed}"
        );
    }
}
