use std::num::NonZeroUsize;
use std::ops::ControlFlow;

use glam::camera::rh::proj::opengl;
use glam::camera::rh::view::look_at_mat4;
use glam::{Affine3A, Quat, Vec2, Vec3};
use rikka_spatial::{
    Aabb, Bvh, BvhUpdate, ClipSpace, Containment, Culling, Plane, Ray, View, Viewport,
    intersect_ray_triangle,
};

use crate::support::{Rng, triangle_soup};

fn bounds_of(triangles: &[[Vec3; 3]]) -> Vec<Aabb> {
    triangles
        .iter()
        .map(|triangle| Aabb::from_triangle(*triangle))
        .collect()
}

fn brute_ray(triangles: &[[Vec3; 3]], ray: &Ray, max_distance: f32) -> Option<(u32, f32)> {
    let mut best: Option<(u32, f32)> = None;
    for (index, triangle) in triangles.iter().enumerate() {
        if let Some(hit) = intersect_ray_triangle(ray, *triangle, max_distance, Culling::None)
            && best.is_none_or(|(_, distance)| hit.distance < distance)
        {
            best = Some((index as u32, hit.distance));
        }
    }
    best
}

fn cast(bvh: &Bvh, triangles: &[[Vec3; 3]], ray: &Ray, max_distance: f32) -> Option<(u32, f32)> {
    #[derive(Clone, Copy)]
    struct Hit(u32, f32);
    impl rikka_spatial::Candidate for Hit {
        fn distance(&self) -> f32 {
            self.1
        }
    }
    bvh.ray_cast(ray, max_distance, |index, limit| {
        intersect_ray_triangle(ray, triangles[index as usize], limit, Culling::None)
            .map(|hit| Hit(index, hit.distance))
    })
    .map(|hit| (hit.0, hit.1))
}

#[test]
fn ray_queries_match_brute_force() {
    let mut rng = Rng::new(60);
    let triangles = triangle_soup(&mut rng, 3_000, 4.0, 0.4);
    let bvh = Bvh::new(&bounds_of(&triangles));
    assert_eq!(bvh.len(), triangles.len());
    for case in 0..2_000 {
        let origin = rng.vec3(-6.0, 6.0);
        let ray = Ray::new(origin, rng.vec3(-4.0, 4.0) - origin).unwrap();
        let limit = if case % 3 == 0 { 3.0 } else { f32::INFINITY };
        let expected = brute_ray(&triangles, &ray, limit);
        let actual = cast(&bvh, &triangles, &ray, limit);
        assert_eq!(
            actual.map(|hit| hit.1),
            expected.map(|hit| hit.1),
            "case {case}"
        );
        let any = bvh.ray_cast_any(&ray, limit, |index, limit| {
            intersect_ray_triangle(&ray, triangles[index as usize], limit, Culling::None).is_some()
        });
        assert_eq!(any, expected.is_some());
        let mut candidates = Vec::new();
        bvh.ray_cast_all(&ray, limit, |index| candidates.push(index));
        let mut hit: Vec<u32> = candidates
            .into_iter()
            .filter(|&index| {
                intersect_ray_triangle(&ray, triangles[index as usize], limit, Culling::None)
                    .is_some()
            })
            .collect();
        hit.sort_unstable();
        let brute: Vec<u32> = (0..triangles.len() as u32)
            .filter(|&index| {
                intersect_ray_triangle(&ray, triangles[index as usize], limit, Culling::None)
                    .is_some()
            })
            .collect();
        assert_eq!(hit, brute);
    }
}

#[test]
fn ray_ties_resolve_to_lowest_primitive() {
    let triangle = [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let triangles = vec![triangle; 40];
    let bvh = Bvh::builder()
        .max_leaf_size(1)
        .build(&bounds_of(&triangles));
    let ray = Ray::new(Vec3::new(0.0, 0.0, 2.0), Vec3::NEG_Z).unwrap();
    assert_eq!(
        cast(&bvh, &triangles, &ray, f32::INFINITY).map(|hit| hit.0),
        Some(0)
    );
    assert!(bvh.depth() <= 70);
}

#[test]
fn nearest_and_range_queries_match_brute_force() {
    let mut rng = Rng::new(61);
    let triangles = triangle_soup(&mut rng, 2_000, 3.0, 0.3);
    let bounds = bounds_of(&triangles);
    let bvh = Bvh::new(&bounds);
    for _ in 0..500 {
        let point = rng.vec3(-4.0, 4.0);
        let expected = bounds
            .iter()
            .enumerate()
            .map(|(index, bounds)| (bounds.distance_squared_to_point(point).sqrt(), index as u32))
            .min_by(|left, right| left.0.total_cmp(&right.0).then(left.1.cmp(&right.1)))
            .unwrap();
        let actual = bvh.nearest_to_point(point, f32::INFINITY, |index, _| {
            Some(
                bounds[index as usize]
                    .distance_squared_to_point(point)
                    .sqrt(),
            )
        });
        assert_eq!(actual, Some(expected.0));
        let radius = rng.range(0.1, 1.5);
        let mut found = Vec::new();
        bvh.for_each_in_sphere(point, radius, |index| found.push(index));
        found.sort_unstable();
        let brute: Vec<u32> = (0..bounds.len() as u32)
            .filter(|&index| bounds[index as usize].intersects_sphere(point, radius))
            .collect();
        assert_eq!(found, brute);
        let query = Aabb::new(point, point + rng.vec3(0.0, 2.0));
        let mut found = Vec::new();
        bvh.for_each_in_aabb(&query, |index| found.push(index));
        found.sort_unstable();
        let brute: Vec<u32> = (0..bounds.len() as u32)
            .filter(|&index| bounds[index as usize].intersects(&query))
            .collect();
        assert_eq!(found, brute);
        let plane = Plane::from_point_normal(point * 0.5, rng.direction()).unwrap();
        let mut found = Vec::new();
        bvh.for_each_crossing_plane(&plane, |index| found.push(index));
        found.sort_unstable();
        let brute: Vec<u32> = (0..bounds.len() as u32)
            .filter(|&index| {
                let distances = bounds[index as usize]
                    .corners()
                    .map(|corner| plane.signed_distance(corner));
                distances.iter().any(|distance| *distance <= 1e-6)
                    && distances.iter().any(|distance| *distance >= -1e-6)
            })
            .collect();
        assert_eq!(found, brute);
    }
    assert_eq!(
        bvh.nearest_to_point(Vec3::splat(100.0), 1.0, |_, _| Some(0.0f32)),
        None
    );
}

#[test]
fn frustum_query_matches_brute_force() {
    let mut rng = Rng::new(62);
    let triangles = triangle_soup(&mut rng, 3_000, 3.0, 0.2);
    let bounds = bounds_of(&triangles);
    let bvh = Bvh::new(&bounds);
    let eye = look_at_mat4(Vec3::new(0.0, -9.0, 4.0), Vec3::ZERO, Vec3::Z);
    let view = View::new(
        opengl::perspective(0.8, 1.0, 0.1, 100.0) * eye,
        Viewport::from_size(Vec2::splat(512.0)),
        ClipSpace::OPENGL,
    )
    .unwrap();
    for _ in 0..200 {
        let corner = rng.vec2(0.0, 512.0);
        let frustum = view.region_frustum(corner, corner + rng.vec2(-200.0, 200.0));
        let mut found = Vec::new();
        bvh.for_each_in_frustum(&frustum, |index, containment| {
            if containment == Containment::Inside {
                assert_eq!(
                    frustum.classify_aabb(&bounds[index as usize]),
                    Containment::Inside
                );
            }
            found.push(index);
        });
        found.sort_unstable();
        let brute: Vec<u32> = (0..bounds.len() as u32)
            .filter(|&index| frustum.classify_aabb(&bounds[index as usize]) != Containment::Outside)
            .collect();
        assert_eq!(found, brute);
    }
}

#[test]
fn traversal_prunes_and_stops_early() {
    let mut rng = Rng::new(63);
    let triangles = triangle_soup(&mut rng, 1_000, 3.0, 0.2);
    let bounds = bounds_of(&triangles);
    let bvh = Bvh::new(&bounds);
    let point = Vec3::new(3.0, 3.0, 3.0);
    let mut visited = Vec::new();
    let flow = bvh.traverse(
        |node| Some(node.distance_squared_to_point(point)),
        |index| {
            visited.push(index);
            match visited.len() == 10 {
                true => ControlFlow::Break(()),
                false => ControlFlow::Continue(()),
            }
        },
    );
    assert!(flow.is_break());
    assert_eq!(visited.len(), 10);
    let mut all = 0;
    let _ = bvh.traverse(
        |_| Some(0.0),
        |_| {
            all += 1;
            ControlFlow::Continue(())
        },
    );
    assert_eq!(all, triangles.len());
    let mut pruned = 0;
    let _ = bvh.traverse(
        |node| node.contains_point(Vec3::splat(50.0)).then_some(0.0),
        |_| {
            pruned += 1;
            ControlFlow::Continue(())
        },
    );
    assert_eq!(pruned, 0);
}

fn brute_pairs(first: &[Aabb], second: &[Aabb], transform: &Affine3A) -> Vec<(u32, u32)> {
    let mut pairs = Vec::new();
    for (i, a) in first.iter().enumerate() {
        for (j, b) in second.iter().enumerate() {
            if a.intersects(&b.transformed(transform)) {
                pairs.push((i as u32, j as u32));
            }
        }
    }
    pairs
}

#[test]
fn overlap_queries_report_every_overlapping_pair() {
    let mut rng = Rng::new(64);
    let first = bounds_of(&triangle_soup(&mut rng, 800, 2.0, 0.3));
    let second = bounds_of(&triangle_soup(&mut rng, 600, 2.0, 0.3));
    let first_bvh = Bvh::new(&first);
    let second_bvh = Bvh::new(&second);
    for transform in [
        Affine3A::IDENTITY,
        Affine3A::from_scale_rotation_translation(
            Vec3::splat(1.2),
            Quat::from_rotation_z(0.0),
            Vec3::new(0.5, 0.0, 0.0),
        ),
    ] {
        let mut pairs = Vec::new();
        first_bvh.for_each_overlap(&second_bvh, &transform, |a, b| pairs.push((a, b)));
        pairs.sort_unstable();
        let brute = brute_pairs(&first, &second, &transform);
        assert_eq!(pairs, brute);
    }
    let rotation =
        Affine3A::from_rotation_translation(Quat::from_rotation_y(0.7), Vec3::new(0.2, 0.1, 0.0));
    let mut pairs = Vec::new();
    first_bvh.for_each_overlap(&second_bvh, &rotation, |a, b| pairs.push((a, b)));
    pairs.sort_unstable();
    let brute = brute_pairs(&first, &second, &rotation);
    assert!(pairs.iter().all(|pair| brute.binary_search(pair).is_ok()));
    let exact: Vec<(u32, u32)> = brute
        .iter()
        .copied()
        .filter(|&(a, b)| {
            second[b as usize].intersects(&first[a as usize].transformed(&rotation.inverse()))
        })
        .collect();
    assert_eq!(pairs, exact);
    let mut self_pairs = Vec::new();
    first_bvh.for_each_self_overlap(|a, b| self_pairs.push((a, b)));
    self_pairs.sort_unstable();
    let brute_self: Vec<(u32, u32)> = brute_pairs(&first, &first, &Affine3A::IDENTITY)
        .into_iter()
        .filter(|(a, b)| a < b)
        .collect();
    assert_eq!(self_pairs, brute_self);
}

#[test]
fn refit_and_update_keep_queries_exact() {
    let mut rng = Rng::new(65);
    let mut triangles = triangle_soup(&mut rng, 2_000, 3.0, 0.2);
    let mut bvh = Bvh::new(&bounds_of(&triangles));
    let mut rebuilt = false;
    for frame in 0..12 {
        for triangle in triangles.iter_mut() {
            let offset = match frame < 6 {
                true => Vec3::new(triangle[0].y.sin(), triangle[0].z.cos(), 0.0) * 0.3,
                false => rng.vec3(-1.0, 1.0),
            };
            *triangle = triangle.map(|vertex| vertex + offset);
        }
        let bounds = bounds_of(&triangles);
        match frame % 2 == 0 {
            true => bvh.refit(&bounds),
            false => rebuilt |= bvh.update(&bounds) == BvhUpdate::Rebuilt,
        }
        assert!(
            bvh.bounds()
                .contains(&Aabb::from_points(triangles.iter().flatten().copied()))
        );
        for _ in 0..100 {
            let origin = rng.vec3(-8.0, 8.0);
            let ray = Ray::new(origin, rng.vec3(-4.0, 4.0) - origin).unwrap();
            assert_eq!(
                cast(&bvh, &triangles, &ray, f32::INFINITY).map(|hit| hit.1),
                brute_ray(&triangles, &ray, f32::INFINITY).map(|hit| hit.1)
            );
        }
    }
    assert!(rebuilt);
}

#[test]
fn parallel_build_is_deterministic() {
    let mut rng = Rng::new(66);
    let triangles = triangle_soup(&mut rng, 100_000, 10.0, 0.1);
    let bounds = bounds_of(&triangles);
    let single = Bvh::builder().threads(NonZeroUsize::MIN).build(&bounds);
    let parallel = Bvh::builder()
        .threads(NonZeroUsize::new(8).unwrap())
        .build(&bounds);
    assert_eq!(single.node_count(), parallel.node_count());
    assert_eq!(single.cost(), parallel.cost());
    assert_eq!(single.depth(), parallel.depth());
    for _ in 0..200 {
        let origin = rng.vec3(-12.0, 12.0);
        let ray = Ray::new(origin, rng.vec3(-10.0, 10.0) - origin).unwrap();
        assert_eq!(
            cast(&single, &triangles, &ray, f32::INFINITY),
            cast(&parallel, &triangles, &ray, f32::INFINITY)
        );
    }
}

#[test]
fn degenerate_inputs_build_valid_trees() {
    let empty = Bvh::new(&[]);
    assert!(empty.is_empty() && empty.bounds().is_empty());
    let ray = Ray::new(Vec3::ZERO, Vec3::X).unwrap();
    assert!(
        empty
            .ray_cast(&ray, f32::INFINITY, |_, _| Some(0.0f32))
            .is_none()
    );
    let mut count = 0;
    empty.for_each_self_overlap(|_, _| count += 1);
    assert_eq!(count, 0);
    let point = Aabb::from_point(Vec3::ONE);
    let same = Bvh::new(&vec![point; 10_000]);
    assert!(same.depth() < 80);
    let mut found = 0;
    same.for_each_in_sphere(Vec3::ONE, 0.0, |_| found += 1);
    assert_eq!(found, 10_000);
    let invalid = [
        Aabb::new(Vec3::NAN, Vec3::ONE),
        Aabb::EMPTY,
        Aabb::new(Vec3::ZERO, Vec3::ONE),
    ];
    let mixed = Bvh::new(&invalid);
    let mut found = Vec::new();
    mixed.for_each_in_aabb(&Aabb::new(Vec3::splat(-10.0), Vec3::splat(10.0)), |index| {
        found.push(index)
    });
    assert_eq!(found, vec![2]);
    let mut exponential: Vec<Aabb> = (0..64)
        .map(|power| Aabb::from_point(Vec3::X * 2f32.powi(power)))
        .collect();
    exponential.extend((0..64).map(|power| Aabb::from_point(Vec3::Y * 0.5f32.powi(power))));
    let skewed = Bvh::new(&exponential);
    let mut found = 0;
    skewed.for_each_in_aabb(&Aabb::new(Vec3::splat(-1.0), Vec3::splat(f32::MAX)), |_| {
        found += 1
    });
    assert_eq!(found, exponential.len());
}
