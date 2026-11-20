use glam::{Mat3, Vec2, Vec3};
use rikka_model_core::polygon;
use rikka_spatial::{
    Candidate, Culling, PointTree, Ray, SegmentBvh, TriangleBvh, closest_point_on_segment,
};

use crate::kdop::{KdopHit, KdopRay};
use crate::math::normalize;
use crate::target::Target;

const LINE_EPSILON: f32 = 0.000001;

#[derive(Clone, Copy, Debug)]
pub(super) struct RayHit {
    pub(super) primitive: u32,
    pub(super) distance: f32,
    pub(super) point: Vec3,
}

impl Candidate for RayHit {
    fn distance(&self) -> f32 {
        self.distance
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NearestHit {
    pub(super) primitive: u32,
    pub(super) distance: f32,
    pub(super) point: Vec3,
}

pub(super) fn nearest_point(
    tree: &PointTree,
    point: Vec3,
    max_distance: f32,
) -> Option<NearestHit> {
    let found = tree.nearest(point)?;
    let distance_squared = found.position.distance_squared(point);
    (distance_squared <= max_distance * max_distance).then(|| NearestHit {
        primitive: found.index,
        distance: distance_squared.sqrt(),
        point: found.position,
    })
}

pub(super) fn nearest_segment(
    tree: &SegmentBvh,
    point: Vec3,
    max_distance: f32,
) -> Option<NearestHit> {
    let found = tree.nearest_to_point(point, max_distance)?;
    (found.distance <= max_distance).then_some(NearestHit {
        primitive: found.segment,
        distance: found.distance,
        point: found.point,
    })
}

pub(super) fn nearest_triangle(
    tree: &TriangleBvh,
    point: Vec3,
    max_distance: f32,
) -> Option<NearestHit> {
    let found = tree.closest_point(point, max_distance)?;
    (found.distance <= max_distance).then_some(NearestHit {
        primitive: found.triangle,
        distance: found.distance,
        point: found.point,
    })
}

fn cast_triangles(
    tree: &TriangleBvh,
    origin: Vec3,
    direction: Vec3,
    radius: f32,
    max_distance: f32,
) -> Option<RayHit> {
    let ray = Ray::new(origin, direction)?;
    if radius == 0.0 {
        return tree
            .ray_cast(&ray, max_distance, Culling::None)
            .filter(|hit| hit.distance < max_distance)
            .map(|hit| RayHit {
                primitive: hit.triangle,
                distance: hit.distance,
                point: ray.at(hit.distance),
            });
    }
    tree.bvh()
        .sphere_cast(&ray, radius, max_distance, |triangle, limit| {
            let [a, b, c] = tree.triangle(triangle);
            let end = origin + direction * limit;
            let lambda = sweeping_sphere_triangle(origin, end, radius, a, b, c)?;
            let distance = lambda * limit;
            (distance >= 0.0 && distance < limit).then(|| RayHit {
                primitive: triangle,
                distance,
                point: origin + direction * distance,
            })
        })
}

pub(super) fn raycast_triangles(
    tree: &TriangleBvh,
    origin: Vec3,
    normal: Vec3,
    radius: f32,
    max_distance: f32,
) -> Option<RayHit> {
    let forward = cast_triangles(tree, origin, normal, radius, max_distance);
    let limit = forward.map_or(max_distance, |hit| hit.distance);
    let backward = cast_triangles(tree, origin, -normal, radius, limit);
    let best = match (forward, backward) {
        (_, Some(back)) if forward.is_none_or(|front| back.distance < front.distance) => Some(back),
        (front, _) => front,
    };
    best.filter(|hit| hit.distance <= max_distance)
}

pub(super) fn raycast_edges(
    target: &Target,
    origin: Vec3,
    normal: Vec3,
    radius: f32,
    max_distance: f32,
) -> Option<RayHit> {
    let mesh = target.mesh();
    let tree = target.edge_tree();
    let cast = |direction: Vec3, hit: &mut KdopHit| {
        let ray = KdopRay {
            origin,
            direction,
            radius,
        };
        tree.ray_cast(&ray, hit, &mut |edge, ray, hit| {
            let [a, b] = mesh.edges[edge as usize];
            let (v1, v2) = (mesh.positions[a as usize], mesh.positions[b as usize]);
            if let Some((distance, point)) =
                edge_spherecast(v1, v2, ray.origin, ray.direction, ray.radius, hit.distance)
            {
                *hit = KdopHit {
                    primitive: Some(edge),
                    distance,
                    point,
                };
            }
        });
    };
    let mut hit = KdopHit {
        primitive: None,
        distance: max_distance,
        point: Vec3::ZERO,
    };
    cast(normal, &mut hit);
    let mut reverse = hit;
    cast(-normal, &mut reverse);
    if reverse.distance < hit.distance {
        hit = reverse;
    }
    let primitive = hit.primitive?;
    (hit.distance <= max_distance).then_some(RayHit {
        primitive,
        distance: hit.distance,
        point: hit.point,
    })
}

fn edge_spherecast(
    v1: Vec3,
    v2: Vec3,
    origin: Vec3,
    direction: Vec3,
    radius: f32,
    limit: f32,
) -> Option<(f32, Vec3)> {
    let radius_squared = radius * radius;
    let r2 = origin + direction;
    if v1 == v2 {
        let closest = closest_point_on_segment(v1, origin, r2).point;
        if dot_from(origin, closest, r2) >= 0.0 {
            let distance = origin.distance(closest);
            if distance < limit {
                return Some((distance, closest));
            }
        }
        return None;
    }
    let (mut on_edge, on_ray) = intersect_lines(v1, v2, origin, r2)?;
    if dot_from(origin, on_ray, r2) < 0.0 {
        return None;
    }
    let distance = origin.distance(on_ray);
    if distance >= limit {
        return None;
    }
    let factor = line_point_factor(on_edge, v1, v2);
    if factor < 0.0 {
        on_edge = v1;
    } else if factor > 1.0 {
        on_edge = v2;
    }
    (on_edge.distance_squared(on_ray) <= radius_squared).then_some((distance, on_ray))
}

fn dot_from(origin: Vec3, a: Vec3, b: Vec3) -> f32 {
    let first = a - origin;
    let second = b - origin;
    if first == Vec3::ZERO || second == Vec3::ZERO {
        return 0.0;
    }
    first.dot(second)
}

pub(super) fn line_point_factor(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    let along = end - start;
    let squared = along.length_squared();
    match squared > 0.0 {
        true => along.dot(point - start) / squared,
        false => 0.0,
    }
}

fn intersect_lines(v1: Vec3, v2: Vec3, v3: Vec3, v4: Vec3) -> Option<(Vec3, Vec3)> {
    let c = v3 - v1;
    let a = v2 - v1;
    let b = v4 - v3;
    let ab = a.cross(b);
    let d = c.dot(ab);
    let div = ab.dot(ab);
    if div == 0.0 {
        return None;
    }
    if d.abs() <= LINE_EPSILON {
        let cb = c.cross(b);
        let point = v1 + a * (cb.dot(ab) / div);
        return Some((point, point));
    }
    let t = project(v1 - v3, a.cross(b));
    let v3t = v3 + t;
    let v4t = v4 + t;
    let c = v3t - v1;
    let b = v4t - v3t;
    let ab = a.cross(b);
    let cb = c.cross(b);
    let first = v1 + a * (cb.dot(ab) / ab.dot(ab));
    Some((first, first - t))
}

fn project(vector: Vec3, onto: Vec3) -> Vec3 {
    if onto == Vec3::ZERO {
        return Vec3::ZERO;
    }
    onto * (vector.dot(onto) / onto.dot(onto))
}

fn lowest_root(a: f32, b: f32, c: f32, max: f32) -> Option<f32> {
    let determinant = b * b - 4.0 * a * c;
    if determinant < 0.0 {
        return None;
    }
    let root = determinant.sqrt();
    let mut r1 = (-b - root) / (2.0 * a);
    let mut r2 = (-b + root) / (2.0 * a);
    if r1 > r2 {
        std::mem::swap(&mut r1, &mut r2);
    }
    if r1 > 0.0 && r1 < max {
        return Some(r1);
    }
    (r2 > 0.0 && r2 < max).then_some(r2)
}

fn sweeping_sphere_triangle(
    p1: Vec3,
    p2: Vec3,
    radius: f32,
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
) -> Option<f32> {
    let radius_squared = radius * radius;
    let e1 = v1 - v0;
    let e2 = v2 - v0;
    let velocity = p2 - p1;
    let mut normal = normalize(e1.cross(e2));
    if normal.dot(velocity) > 0.0 {
        normal = -normal;
    }
    let plane = p1.dot(normal) - v0.dot(normal);
    let normal_velocity = normal.dot(velocity);
    if normal_velocity.abs() < 0.000001 {
        if plane.abs() >= radius {
            return None;
        }
    } else {
        let mut t0 = (-plane + radius) / normal_velocity;
        let mut t1 = (-plane - radius) / normal_velocity;
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
        }
        if t0 > 1.0 || t1 < 0.0 {
            return None;
        }
        let t0 = t0.clamp(0.0, 1.0);
        let point = p1 + velocity * t0 - normal * radius;
        let a = e1.dot(e1);
        let b = e1.dot(e2);
        let c = e2.dot(e2);
        let offset = point - v0;
        let d = offset.dot(e1);
        let e = offset.dot(e2);
        let x = d * c - e * b;
        let y = e * a - d * b;
        let z = x + y - (a * c - b * b);
        if z <= 0.0 && x >= 0.0 && y >= 0.0 {
            return Some(t0);
        }
    }
    let mut lambda = 1.0f32;
    let mut found = false;
    let speed = velocity.dot(velocity);
    for vertex in [v0, v1, v2] {
        let offset = p1 - vertex;
        let b = 2.0 * velocity.dot(offset);
        let c = offset.dot(offset) - radius_squared;
        if let Some(root) = lowest_root(speed, b, c, lambda) {
            lambda = root;
            found = true;
        }
    }
    let e3 = v2 - v1;
    for (edge, start) in [(e1, v0), (e2, v0), (e3, v1)] {
        let base = start - p1;
        let length = edge.dot(edge);
        let along_velocity = edge.dot(velocity);
        let along_base = edge.dot(base);
        let a = length * -speed + along_velocity * along_velocity;
        let b = 2.0 * (length * velocity.dot(base) - along_velocity * along_base);
        let c = length * (radius_squared - base.dot(base)) + along_base * along_base;
        if let Some(root) = lowest_root(a, b, c, lambda) {
            let factor = (along_velocity * root - along_base) / length;
            if (0.0..=1.0).contains(&factor) {
                lambda = root;
                found = true;
            }
        }
    }
    found.then_some(lambda)
}

pub(super) fn slerp_safe(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    if let Some(result) = slerp(a, b, t) {
        return result;
    }
    let dominant = {
        let v = a.abs();
        if v.x > v.y {
            if v.x > v.z { 0 } else { 2 }
        } else if v.y > v.z {
            1
        } else {
            2
        }
    };
    let ortho = normalize(match dominant {
        0 => Vec3::new(-a.y - a.z, a.x, a.x),
        1 => Vec3::new(a.y, -a.x - a.z, a.y),
        _ => Vec3::new(a.z, a.z, -a.x - a.y),
    });
    match t < 0.5 {
        true => slerp(a, ortho, t * 2.0).unwrap_or(a),
        false => slerp(ortho, b, (t - 0.5) * 2.0).unwrap_or(b),
    }
}

fn slerp(a: Vec3, b: Vec3, t: f32) -> Option<Vec3> {
    let cosine = a.dot(b);
    if cosine < -1.0 + f32::EPSILON {
        return None;
    }
    let (w0, w1) = if cosine.abs() < 1.0 - 1.0e-4 {
        let omega = cosine.acos();
        let sine = omega.sin();
        (((1.0 - t) * omega).sin() / sine, (t * omega).sin() / sine)
    } else {
        (1.0 - t, t)
    };
    Some(a * w0 + b * w1)
}

pub(super) fn face_center(positions: &[Vec3], points: &[u32]) -> Vec3 {
    let sum = points
        .iter()
        .fold(Vec3::ZERO, |sum, &point| sum + positions[point as usize]);
    sum * (1.0 / points.len() as f32)
}

pub(super) fn dominant_axis_matrix(normal: Vec3) -> Mat3 {
    let flat = normal.x * normal.x + normal.y * normal.y;
    let (first, second) = if flat > f32::EPSILON {
        let d = 1.0 / flat.sqrt();
        let first = Vec3::new(normal.y * d, -normal.x * d, 0.0);
        let second = Vec3::new(
            -normal.z * first.y,
            normal.z * first.x,
            normal.x * first.y - normal.y * first.x,
        );
        (first, second)
    } else {
        (
            Vec3::new(if normal.z < 0.0 { -1.0 } else { 1.0 }, 0.0, 0.0),
            Vec3::Y,
        )
    };
    Mat3::from_cols(first, second, normal).transpose()
}

pub(super) fn signed_area_2d(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    0.5 * ((a.x - b.x) * (b.y - c.y) + (a.y - b.y) * (c.x - b.x))
}

pub(super) fn polygon_area_2d(points: &[Vec2]) -> f32 {
    let mut cross = 0.0f32;
    let mut previous = points[points.len() - 1];
    for &point in points {
        cross += (point.x - previous.x) * (point.y + previous.y);
        previous = point;
    }
    (cross * 0.5).abs()
}

pub(super) fn fill_polygon_2d(points: &[Vec2]) -> Vec<[u32; 3]> {
    match points.len() {
        0..=2 => Vec::new(),
        3 => vec![[2, 1, 0]],
        4 => vec![[0, 1, 2], [0, 2, 3]],
        _ => polygon::triangulate_flat(points),
    }
}

pub(super) struct Random {
    state: u64,
}

impl Random {
    pub(super) fn new(seed: u32) -> Random {
        Random {
            state: (u64::from(seed) << 16) | 0x330E,
        }
    }

    fn next_float(&mut self) -> f32 {
        self.state =
            0x5DEE_CE66D_u64.wrapping_mul(self.state).wrapping_add(0xB) & 0x0000_FFFF_FFFF_FFFF;
        ((self.state >> 17) as i32) as f32 / 2_147_483_648.0
    }

    pub(super) fn triangle_sample(&mut self, a: Vec2, b: Vec2, c: Vec2) -> Vec2 {
        let mut u = self.next_float();
        let mut v = self.next_float();
        if u + v > 1.0 {
            u = 1.0 - u;
            v = 1.0 - v;
        }
        a + (b - a) * u + (c - a) * v
    }
}
