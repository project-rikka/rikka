use glam::camera::rh::proj::opengl;
use glam::camera::rh::view::look_at_mat4;
use glam::{Affine3A, Vec2, Vec3};
use rikka_spatial::{
    ClipSpace, Containment, Culling, Error, Ray, SegmentBvh, TriangleBvh, TriangleIntersection,
    View, Viewport, closest_points_ray_segment, intersect_ray_triangle,
};

use crate::support::{
    Rng, segment_distance_reference, torus, triangle_distance_reference, uv_sphere,
};

#[test]
fn closest_point_on_mesh_matches_brute_force() {
    let mesh = torus(48, 24, 1.0, 0.35);
    let bvh = TriangleBvh::new(mesh.positions.clone(), mesh.triangles.clone()).unwrap();
    let mut rng = Rng::new(70);
    for _ in 0..400 {
        let point = rng.vec3(-2.0, 2.0);
        let expected = (0..mesh.triangles.len())
            .map(|index| triangle_distance_reference(point, mesh.triangle(index)))
            .fold(f64::INFINITY, f64::min);
        let nearest = bvh.closest_point(point, f32::INFINITY).unwrap();
        assert!((f64::from(nearest.distance) - expected).abs() < 1e-5);
        let [a, b, c] = bvh.triangle(nearest.triangle);
        let rebuilt =
            a * nearest.barycentric.x + b * nearest.barycentric.y + c * nearest.barycentric.z;
        assert!(rebuilt.distance(nearest.point) < 1e-5);
    }
    assert!(bvh.closest_point(Vec3::splat(10.0), 1.0).is_none());
}

#[test]
fn mesh_ray_casts_match_brute_force() {
    let mesh = uv_sphere(64, 32, 1.0);
    let bvh = TriangleBvh::new(mesh.positions.clone(), mesh.triangles.clone()).unwrap();
    let mut rng = Rng::new(71);
    for _ in 0..500 {
        let origin = rng.direction() * 3.0;
        let target = rng.direction() * rng.range(0.0, 0.9);
        let ray = Ray::new(origin, target - origin).unwrap();
        let expected = (0..mesh.triangles.len())
            .filter_map(|index| {
                intersect_ray_triangle(&ray, mesh.triangle(index), f32::INFINITY, Culling::None)
            })
            .map(|hit| hit.distance)
            .fold(f32::INFINITY, f32::min);
        let hit = bvh.ray_cast(&ray, f32::INFINITY, Culling::None).unwrap();
        assert_eq!(hit.distance, expected);
        assert!(hit.normal.dot(ray.direction()) < 0.0);
        assert!((hit.point.length() - 1.0).abs() < 5e-3);
        let all = bvh.ray_cast_all(&ray, f32::INFINITY, Culling::None);
        assert!(all.len() >= 2);
        assert!(
            all.windows(2)
                .all(|pair| pair[0].distance <= pair[1].distance)
        );
        assert_eq!(all[0].distance, hit.distance);
        let back = bvh.ray_cast(&ray, f32::INFINITY, Culling::Front).unwrap();
        assert!(back.normal.dot(ray.direction()) > 0.0);
        assert!(bvh.ray_cast_any(&ray, f32::INFINITY, Culling::None));
        assert!(!bvh.ray_cast_any(&ray, hit.distance * 0.99, Culling::None));
    }
    let inside = Ray::new(Vec3::ZERO, Vec3::new(0.3, 0.2, 0.9)).unwrap();
    assert!(
        (bvh.ray_cast(&inside, f32::INFINITY, Culling::None)
            .unwrap()
            .distance
            - 1.0)
            .abs()
            < 5e-3
    );
    assert!(
        bvh.ray_cast(&inside, f32::INFINITY, Culling::Back)
            .is_none()
    );
}

#[test]
fn sphere_and_frustum_triangle_queries_are_exact() {
    let mesh = uv_sphere(32, 16, 1.0);
    let bvh = TriangleBvh::new(mesh.positions.clone(), mesh.triangles.clone()).unwrap();
    let center = Vec3::new(0.0, 0.0, 1.2);
    let mut found = Vec::new();
    bvh.for_each_in_sphere(center, 0.5, |triangle| found.push(triangle));
    found.sort_unstable();
    let expected: Vec<u32> = (0..mesh.triangles.len() as u32)
        .filter(|&index| triangle_distance_reference(center, mesh.triangle(index as usize)) <= 0.5)
        .collect();
    assert_eq!(found, expected);
    let view = View::new(
        opengl::orthographic(-2.0, 2.0, -2.0, 2.0, 0.1, 10.0)
            * look_at_mat4(Vec3::Z * 5.0, Vec3::ZERO, Vec3::Y),
        Viewport::from_size(Vec2::splat(400.0)),
        ClipSpace::OPENGL,
    )
    .unwrap();
    let frustum = view.region_frustum(Vec2::new(200.0, 200.0), Vec2::new(300.0, 100.0));
    let mut found = Vec::new();
    bvh.for_each_in_frustum(&frustum, |triangle, containment| {
        found.push((triangle, containment))
    });
    found.sort_unstable_by_key(|(triangle, _)| *triangle);
    let expected: Vec<(u32, Containment)> = (0..mesh.triangles.len() as u32)
        .map(|index| {
            (
                index,
                frustum.classify_triangle(mesh.triangle(index as usize)),
            )
        })
        .filter(|(_, containment)| *containment != Containment::Outside)
        .collect();
    assert_eq!(found, expected);
    assert!(
        found
            .iter()
            .any(|(_, containment)| *containment == Containment::Inside)
    );
}

#[test]
fn intersections_between_meshes_lie_on_both_surfaces() {
    let sphere = uv_sphere(24, 12, 1.0);
    let first = TriangleBvh::new(sphere.positions.clone(), sphere.triangles.clone()).unwrap();
    let second = first.clone();
    let transform = Affine3A::from_translation(Vec3::new(1.0, 0.0, 0.0));
    let mut pairs = Vec::new();
    first.for_each_intersection(&second, &transform, |pair| pairs.push(pair));
    assert!(pairs.len() > 20);
    for pair in &pairs {
        let TriangleIntersection::Segment { start, end } = pair.intersection else {
            panic!("spheres do not share planes");
        };
        let moved = second
            .triangle(pair.second)
            .map(|vertex| transform.transform_point3(vertex));
        for point in [start, end] {
            assert!(triangle_distance_reference(point, first.triangle(pair.first)) < 1e-4);
            assert!(triangle_distance_reference(point, moved) < 1e-4);
            assert!((point.x - 0.5).abs() < 0.1);
        }
    }
    let mut own = 0;
    first.for_each_self_intersection(|_| own += 1);
    assert_eq!(own, 0);
    let doughnut = torus(32, 16, 1.0, 0.3);
    let ring = TriangleBvh::new(doughnut.positions, doughnut.triangles).unwrap();
    ring.for_each_self_intersection(|_| own += 1);
    assert_eq!(own, 0);
    let crossing = TriangleBvh::from_triangles([
        [
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ],
        [
            Vec3::new(0.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, 2.0, 0.0),
        ],
    ])
    .unwrap();
    let mut pairs = Vec::new();
    crossing.for_each_self_intersection(|pair| pairs.push((pair.first, pair.second)));
    assert_eq!(pairs, vec![(0, 1)]);
}

#[test]
fn moving_vertices_updates_mesh_queries() {
    let mesh = uv_sphere(32, 16, 1.0);
    let mut bvh = TriangleBvh::new(mesh.positions.clone(), mesh.triangles.clone()).unwrap();
    let ray = Ray::new(Vec3::new(5.0, 0.01, 0.02), Vec3::NEG_X).unwrap();
    for scale in [1.5f32, 0.5, 3.0] {
        let moved: Vec<Vec3> = mesh
            .positions
            .iter()
            .map(|position| *position * scale)
            .collect();
        bvh.set_positions(&moved);
        let hit = bvh.ray_cast(&ray, f32::INFINITY, Culling::None).unwrap();
        assert!((hit.distance - (5.0 - scale)).abs() < 1e-2 * scale);
    }
}

#[test]
fn patched_bvhs_answer_like_rebuilt_ones() {
    let mesh = torus(40, 20, 1.0, 0.3);
    let edges = mesh.edges();
    let mut triangles = TriangleBvh::new(mesh.positions.clone(), mesh.triangles.clone()).unwrap();
    let mut segments = SegmentBvh::new(mesh.positions.clone(), edges.clone()).unwrap();
    let mut rng = Rng::new(73);
    let mut positions = mesh.positions.clone();
    for _ in 0..4 {
        let moved: Vec<(u32, Vec3)> = (0..40)
            .map(|_| {
                let vertex = (rng.unit() * positions.len() as f32) as u32 % positions.len() as u32;
                (vertex, rng.vec3(-1.5, 1.5))
            })
            .collect();
        for &(vertex, position) in &moved {
            positions[vertex as usize] = position;
        }
        let touched = |corners: &[u32]| {
            corners
                .iter()
                .any(|corner| moved.iter().any(|(vertex, _)| vertex == corner))
        };
        let refit_triangles: Vec<u32> = (0..mesh.triangles.len() as u32)
            .filter(|&triangle| touched(&mesh.triangles[triangle as usize]))
            .collect();
        let refit_segments: Vec<u32> = (0..edges.len() as u32)
            .filter(|&segment| touched(&edges[segment as usize]))
            .collect();
        triangles.patch(&moved, &[], &refit_triangles).unwrap();
        segments.patch(&moved, &refit_segments).unwrap();
        let rebuilt_triangles =
            TriangleBvh::new(positions.clone(), mesh.triangles.clone()).unwrap();
        let rebuilt_segments = SegmentBvh::new(positions.clone(), edges.clone()).unwrap();
        for _ in 0..200 {
            let point = rng.vec3(-2.0, 2.0);
            let patched = triangles.closest_point(point, f32::INFINITY).unwrap();
            let fresh = rebuilt_triangles
                .closest_point(point, f32::INFINITY)
                .unwrap();
            assert!((patched.distance - fresh.distance).abs() < 1e-6);
            let patched = segments.nearest_to_point(point, f32::INFINITY).unwrap();
            let fresh = rebuilt_segments
                .nearest_to_point(point, f32::INFINITY)
                .unwrap();
            assert!((patched.distance - fresh.distance).abs() < 1e-6);
        }
    }
    assert!(
        triangles
            .patch(&[(u32::MAX, Vec3::ZERO)], &[], &[])
            .is_err()
    );
}

#[test]
fn segment_queries_match_brute_force() {
    let mesh = torus(40, 20, 1.0, 0.3);
    let edges = mesh.edges();
    let bvh = SegmentBvh::new(mesh.positions.clone(), edges.clone()).unwrap();
    let segment = |index: usize| edges[index].map(|vertex| mesh.positions[vertex as usize]);
    let mut rng = Rng::new(72);
    for _ in 0..300 {
        let point = rng.vec3(-1.6, 1.6);
        let expected = (0..edges.len())
            .map(|index| {
                let [start, end] = segment(index);
                segment_distance_reference(point.as_dvec3(), start.as_dvec3(), end.as_dvec3())
            })
            .fold(f64::INFINITY, f64::min);
        let nearest = bvh.nearest_to_point(point, f32::INFINITY).unwrap();
        assert!((f64::from(nearest.distance) - expected).abs() < 1e-5);
        let origin = rng.direction() * 4.0;
        let ray = Ray::new(origin, rng.vec3(-1.0, 1.0) - origin).unwrap();
        let expected = (0..edges.len())
            .map(|index| {
                closest_points_ray_segment(&ray, f32::INFINITY, segment(index))
                    .distance_squared
                    .sqrt()
            })
            .fold(f32::INFINITY, f32::min);
        let nearest = bvh
            .nearest_to_ray(&ray, f32::INFINITY, f32::INFINITY)
            .unwrap();
        assert!((nearest.distance - expected).abs() < 1e-5);
        let short = bvh.nearest_to_ray(&ray, 2.0, f32::INFINITY).unwrap();
        assert!(short.ray_distance <= 2.0 && short.distance >= nearest.distance - 1e-5);
    }
}

#[test]
fn projected_segment_picking_finds_edges_under_the_cursor() {
    let mesh = torus(40, 20, 1.0, 0.3);
    let edges = mesh.edges();
    let bvh = SegmentBvh::new(mesh.positions.clone(), edges.clone()).unwrap();
    let view = View::new(
        opengl::perspective(0.9, 1.0, 0.1, 50.0)
            * look_at_mat4(Vec3::new(0.0, -3.0, 3.0), Vec3::ZERO, Vec3::Z),
        Viewport::from_size(Vec2::splat(600.0)),
        ClipSpace::OPENGL,
    )
    .unwrap();
    let mut rng = Rng::new(73);
    for _ in 0..200 {
        let screen = rng.vec2(50.0, 550.0);
        let expected = (0..edges.len())
            .filter_map(|index| {
                let [start, end] = edges[index].map(|vertex| mesh.positions[vertex as usize]);
                let flat = view.project_segment(start, end)?;
                let direction = flat.end - flat.start;
                let t = (direction.dot(screen - flat.start) / direction.length_squared())
                    .clamp(0.0, 1.0);
                Some((flat.start + direction * t).distance(screen))
            })
            .fold(f32::INFINITY, f32::min);
        let nearest = bvh.nearest_projected(&view, screen, 1e4).unwrap();
        assert!((nearest.distance - expected).abs() < 1e-3);
        let [start, end] = bvh.segment(nearest.segment);
        let world = start.lerp(end, nearest.parameter);
        let projected = view.project(world).unwrap().position;
        assert!(projected.distance(nearest.screen_point) < 1e-2);
    }
}

#[test]
fn out_of_range_indices_are_rejected() {
    let positions = vec![Vec3::ZERO, Vec3::X, Vec3::Y];
    assert_eq!(
        TriangleBvh::new(positions.clone(), vec![[0, 1, 2], [0, 3, 1]]).unwrap_err(),
        Error::IndexOutOfRange {
            element: 1,
            index: 3,
            len: 3
        }
    );
    assert_eq!(
        SegmentBvh::new(positions, vec![[0, 1], [7, 1]]).unwrap_err(),
        Error::IndexOutOfRange {
            element: 1,
            index: 7,
            len: 3
        }
    );
}
