use std::f32::consts::{FRAC_2_PI, PI, TAU};

use glam::Vec3;

pub(crate) fn normalize(vector: Vec3) -> Vec3 {
    let squared = vector.dot(vector);
    match squared > 1.0e-35 {
        true => vector * (1.0 / squared.sqrt()),
        false => Vec3::ZERO,
    }
}

pub(crate) fn triangle_normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    normalize((a - b).cross(b - c))
}

pub(crate) fn interpolation_weights(triangle: [Vec3; 3], point: Vec3) -> Vec3 {
    let [a, b, c] = triangle;
    let normal = triangle_normal(a, b, c).abs();
    let (i, j) = if normal.z >= normal.x && normal.z >= normal.y {
        (0, 1)
    } else if normal.y >= normal.x && normal.y >= normal.z {
        (0, 2)
    } else {
        (1, 2)
    };
    let area = |p: Vec3, q: Vec3, r: Vec3| {
        0.5 * ((p[i] - q[i]) * (q[j] - r[j]) + (p[j] - q[j]) * (r[i] - q[i]))
    };
    let weights = Vec3::new(area(b, c, point), area(c, a, point), area(a, b, point));
    let total = weights.x + weights.y + weights.z;
    let scaled = weights * (1.0 / total);
    match scaled.is_finite() {
        true => scaled,
        false => Vec3::splat(1.0 / 3.0),
    }
}

pub(crate) fn manhattan_length(vector: Vec3) -> f32 {
    vector.abs().element_sum()
}

pub(crate) fn safe_asin(value: f32) -> f32 {
    value.clamp(-1.0, 1.0).asin()
}

pub(crate) fn angle_normalized(first: Vec3, second: Vec3) -> f32 {
    match first.dot(second) >= 0.0 {
        true => 2.0 * safe_asin(first.distance(second) / 2.0),
        false => PI - 2.0 * safe_asin(first.distance(-second) / 2.0),
    }
}

pub(crate) fn angle(first: Vec3, second: Vec3) -> f32 {
    angle_normalized(first.normalize_or_zero(), second.normalize_or_zero())
}

pub(crate) fn project_plane_normalized(vector: Vec3, plane: Vec3) -> Vec3 {
    vector - plane * vector.dot(plane)
}

pub(crate) fn angle_signed_on_axis(first: Vec3, second: Vec3, axis: Vec3) -> f32 {
    let first = project_plane_normalized(first, axis);
    let second = project_plane_normalized(second, axis);
    let angle = angle(first, second);
    match second.cross(first).dot(axis) < 0.0 {
        true => TAU - angle,
        false => angle,
    }
}

pub(crate) fn mid_angle_weighted(first: Vec3, second: Vec3) -> Vec3 {
    let sum = first + second;
    let length = sum.length();
    let direction = match length * length > 1.0e-35 {
        true => sum / length,
        false => Vec3::ZERO,
    };
    direction * (FRAC_2_PI * 2.0 * (length / 2.0).acos())
}

pub(crate) fn shell_distance(first: Vec3, second: Vec3) -> f32 {
    let cosine = first.dot(second).abs();
    match cosine < 1.0e-8 {
        true => 1.0,
        false => 1.0 / cosine,
    }
}
