use rikka_physics_collision::{QueryFilter, Ray};
use rikka_physics_geometry::{Capsule, Cuboid, MeshShape, Sphere, TriangleMesh};
use rikka_physics_math::{Quat, Vec3};
use rikka_physics_rigid::{Body, RayHit, RigidWorld, ShapeDef};

fn scene() -> RigidWorld {
    let mut world = RigidWorld::builder().build().unwrap();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -0.5, 0.0))
                .shape(Cuboid::new(Vec3::new(20.0, 0.5, 20.0))),
        )
        .unwrap();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(12.0, 0.2, 12.0))
                .shape(MeshShape::new(TriangleMesh::grid(6, 6, 1.0, 0).unwrap())),
        )
        .unwrap();
    for i in 0..60 {
        let angle = i as f32 * 0.61;
        let radius = 2.0 + (i % 7) as f32 * 1.3;
        let position = Vec3::new(
            angle.cos() * radius,
            0.6 + (i % 5) as f32 * 0.9,
            angle.sin() * radius,
        );
        let shape: ShapeDef = match i % 3 {
            0 => Sphere::new(0.3 + 0.05 * (i % 4) as f32).into(),
            1 => Cuboid::new(Vec3::new(0.3, 0.4, 0.25)).into(),
            _ => Capsule::new(0.2, 0.35).into(),
        };
        world
            .create_body(
                Body::dynamic()
                    .position(position)
                    .rotation(Quat::from_rotation_y(angle))
                    .shape(shape),
            )
            .unwrap();
    }
    world.step(1.0 / 60.0);
    world
}

fn rays() -> Vec<Ray> {
    (0..203)
        .map(|i| {
            let angle = i as f32 * 0.37;
            let height = 0.3 + (i % 9) as f32 * 0.5;
            Ray {
                origin: Vec3::new(angle.cos() * 18.0, height, angle.sin() * 18.0),
                translation: Vec3::new(
                    -angle.cos() * 30.0,
                    -1.2 * height + (i % 3) as f32 * 0.1,
                    -angle.sin() * 30.0 + (i % 5) as f32,
                ),
            }
        })
        .collect()
}

#[test]
fn ray_packets_match_scalar_rays() {
    let world = scene();
    let rays = rays();
    let mut packed: Vec<Option<RayHit>> = vec![None; rays.len()];
    world.cast_rays_closest(&rays, QueryFilter::default(), |index, hit| {
        packed[index] = Some(*hit);
    });
    let mut hits = 0;
    for (ray, packed) in rays.iter().zip(&packed) {
        let scalar = world.cast_ray_closest(ray.origin, ray.translation, QueryFilter::default());
        match (scalar, packed) {
            (None, None) => {}
            (Some(scalar), Some(packed)) => {
                hits += 1;
                assert_eq!(scalar.shape, packed.shape);
                assert!((scalar.fraction - packed.fraction).abs() < 1e-5);
                assert!((scalar.point - packed.point).length() < 1e-4);
                assert_eq!(
                    (scalar.triangle, scalar.child),
                    (packed.triangle, packed.child)
                );
            }
            (scalar, packed) => panic!("scalar {scalar:?} packet {packed:?}"),
        }
    }
    assert!(hits > 150, "only {hits} rays hit");
}

#[test]
fn mesh_ray_hits_report_triangles() {
    let world = scene();
    let hit = world
        .cast_ray_closest(
            Vec3::new(12.3, 5.0, 12.4),
            Vec3::new(0.0, -10.0, 0.0),
            QueryFilter::default(),
        )
        .unwrap();
    assert!((hit.point.y - 0.2).abs() < 1e-4);
    assert_ne!(hit.triangle, u32::MAX);
}
