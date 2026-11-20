use std::f32::consts::PI;

use glam::{Vec2, Vec3};
pub(crate) use rikka_mesh_edit::math::{
    AxisProjection, angle_normalized, angle_signed_on_axis_vectors, angle_vectors, compare_vec3,
    cross_tri_2d, line_point_factor, normal_quad, normal_tri, normalize, normalize_with_length,
    project_plane, project_plane_normalized, quad_flip_flags,
};

pub(crate) const RIGHT_ANGLE: f32 = PI / 2.0;

pub(crate) fn angle_at(a: Vec3, corner: Vec3, c: Vec3) -> f32 {
    angle_normalized(normalize(corner - a), normalize(corner - c))
}

pub(crate) fn cos_at(a: Vec3, corner: Vec3, c: Vec3) -> f32 {
    normalize(corner - a).dot(normalize(corner - c))
}

pub(crate) fn angle_on_axis(a: Vec3, corner: Vec3, c: Vec3, axis: Vec3) -> f32 {
    angle_vectors(
        project_plane_normalized(a - corner, axis),
        project_plane_normalized(c - corner, axis),
    )
}

pub(crate) fn rotate_normalized(point: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let cos = angle.cos();
    let sin = angle.sin();
    let rest = 1.0 - cos;
    Vec3::new(
        ((cos + rest * axis.x * axis.x) * point.x)
            + ((rest * axis.x * axis.y - axis.z * sin) * point.y)
            + ((rest * axis.x * axis.z + axis.y * sin) * point.z),
        ((rest * axis.x * axis.y + axis.z * sin) * point.x)
            + ((cos + rest * axis.y * axis.y) * point.y)
            + ((rest * axis.y * axis.z - axis.x * sin) * point.z),
        ((rest * axis.x * axis.z - axis.y * sin) * point.x)
            + ((rest * axis.y * axis.z + axis.x * sin) * point.y)
            + ((cos + rest * axis.z * axis.z) * point.z),
    )
}

pub(crate) fn area_tri(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    (a - b).cross(b - c).length() * 0.5
}

pub(crate) fn is_quad_flip(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    let (first, second) = quad_flip_flags(a, b, c, d);
    first || second
}

pub(crate) fn is_quad_convex(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> bool {
    let plane = (a - c).cross(b - d);
    let epsilon = 1.0e-8f32 * 1.0e-8;
    if plane.length_squared() < epsilon {
        return false;
    }
    let projected = [a, b, c, d].map(|corner| project_plane(corner, plane));
    let directions: [Vec3; 4] =
        std::array::from_fn(|index| projected[index] - projected[(index + 3) % 4]);
    (0..4).all(|index| {
        directions[index]
            .cross(directions[(index + 1) % 4])
            .dot(plane)
            > 0.0
    })
}

fn line_point_side(a: Vec2, b: Vec2, point: Vec2) -> f32 {
    ((a.x - point.x) * (b.y - point.y)) - ((b.x - point.x) * (a.y - point.y))
}

pub(crate) fn is_point_in_tri_cw(point: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
    line_point_side(a, b, point) >= 0.0
        && line_point_side(b, c, point) >= 0.0
        && line_point_side(c, a, point) >= 0.0
}

pub(crate) fn is_point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    let count = polygon.len();
    let mut previous = count.wrapping_sub(1);
    for index in 0..count {
        let (a, b) = (polygon[index], polygon[previous]);
        if ((a.y > point.y) != (b.y > point.y))
            && (point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x)
        {
            inside = !inside;
        }
        previous = index;
    }
    inside
}

pub(crate) fn intersect_rays(
    origin_a: Vec3,
    direction_a: Vec3,
    origin_b: Vec3,
    direction_b: Vec3,
    epsilon: f32,
) -> Option<(f32, f32)> {
    let normal = direction_b.cross(direction_a);
    let length = normal.length_squared();
    if length < epsilon {
        return None;
    }
    let offset = origin_b - origin_a;
    let c = normal - offset;
    let lambda_a = c.cross(direction_b).dot(normal) / length;
    let lambda_b = c.cross(direction_a).dot(normal) / length;
    Some((lambda_a, lambda_b))
}

pub(crate) fn ray_point_factor(point: Vec3, origin: Vec3, direction: Vec3, fallback: f32) -> f32 {
    let offset = point - origin;
    let squared = direction.length_squared();
    match squared > 0.0 {
        true => direction.dot(offset) / squared,
        false => fallback,
    }
}

pub(crate) fn interpf(target: f32, origin: f32, factor: f32) -> f32 {
    (factor * target) + (1.0 - factor) * origin
}
