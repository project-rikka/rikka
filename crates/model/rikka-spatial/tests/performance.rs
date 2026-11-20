use std::num::NonZeroUsize;
use std::time::Instant;

use glam::Vec3;
use rikka_spatial::{Aabb, Bvh, Culling, PointTree, Ray, TriangleBvh};

use crate::support::{Rng, grid_mesh};

#[test]
fn million_triangle_build_and_queries() {
    let mesh = grid_mesh(708, 707, |u, v| {
        Vec3::new(
            u * 20.0 - 10.0,
            v * 20.0 - 10.0,
            (u * 31.0).sin() * (v * 17.0).cos() * 0.5,
        )
    });
    let count = mesh.triangles.len();
    assert!(count >= 1_000_000);
    let bounds: Vec<Aabb> = (0..count)
        .map(|index| Aabb::from_triangle(mesh.triangle(index)))
        .collect();

    let started = Instant::now();
    let single = Bvh::builder().threads(NonZeroUsize::MIN).build(&bounds);
    let single_build = started.elapsed();
    let started = Instant::now();
    let parallel = Bvh::new(&bounds);
    let parallel_build = started.elapsed();
    assert_eq!(single.node_count(), parallel.node_count());

    let started = Instant::now();
    let mut bvh = TriangleBvh::new(mesh.positions.clone(), mesh.triangles.clone()).unwrap();
    let triangle_build = started.elapsed();

    let mut rng = Rng::new(90);
    let rays: Vec<Ray> = (0..100_000)
        .map(|_| {
            let origin = rng.vec3(-10.0, 10.0) + Vec3::Z * 15.0;
            Ray::new(
                origin,
                Vec3::new(rng.range(-9.0, 9.0), rng.range(-9.0, 9.0), -1.0) - origin,
            )
            .unwrap()
        })
        .collect();
    let started = Instant::now();
    let hits = rays
        .iter()
        .filter(|ray| bvh.ray_cast(ray, f32::INFINITY, Culling::None).is_some())
        .count();
    let ray_time = started.elapsed();
    assert_eq!(hits, rays.len());

    let points: Vec<Vec3> = (0..100_000)
        .map(|_| {
            Vec3::new(
                rng.range(-10.0, 10.0),
                rng.range(-10.0, 10.0),
                rng.range(-1.0, 1.0),
            )
        })
        .collect();
    let started = Instant::now();
    let found = points
        .iter()
        .filter(|point| bvh.closest_point(**point, f32::INFINITY).is_some())
        .count();
    let closest_time = started.elapsed();
    assert_eq!(found, points.len());

    let moved: Vec<Vec3> = mesh
        .positions
        .iter()
        .map(|position| *position + Vec3::Z * position.x.sin() * 0.05)
        .collect();
    let started = Instant::now();
    let update = bvh.set_positions(&moved);
    let update_time = started.elapsed();

    let started = Instant::now();
    let tree = PointTree::new(mesh.positions.iter().copied());
    let point_build = started.elapsed();
    let started = Instant::now();
    let duplicates = tree.find_duplicates(1e-4);
    let duplicates_time = started.elapsed();
    assert_eq!(duplicates.merged_count(), 0);

    println!(
        "triangles {count}: bvh build single {single_build:?}, parallel {parallel_build:?}, triangle bvh {triangle_build:?}, \
         nodes {}, depth {}, sah {:.2}; 100k rays {ray_time:?}; 100k closest {closest_time:?}; set_positions {update_time:?} \
         ({update:?}); point tree {} points {point_build:?}; duplicates {duplicates_time:?}",
        single.node_count(),
        single.depth(),
        single.cost(),
        tree.len(),
    );
}
