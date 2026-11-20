use glam::Vec3;
use rikka_mesh_edit::math::closest_to_line;

pub(crate) enum LineLine {
    None,
    Point(Vec3),
    Nearest(Vec3, Vec3),
}

pub(crate) fn intersect_line_line(
    v1: Vec3,
    v2: Vec3,
    v3: Vec3,
    v4: Vec3,
    epsilon: f32,
) -> LineLine {
    let c = v3 - v1;
    let a = v2 - v1;
    let b = v4 - v3;
    let ab = a.cross(b);
    let d = c.dot(ab);
    let div = ab.dot(ab);
    if div == 0.0 {
        return LineLine::None;
    }
    if d.abs() <= epsilon {
        let cb = c.cross(b);
        return LineLine::Point(v1 + a * (cb.dot(ab) / div));
    }
    let n = a.cross(b);
    let t = v1 - v3;
    let t = n * (t.dot(n) / n.dot(n));
    let v3t = v3 + t;
    let v4t = v4 + t;
    let c = v3t - v1;
    let b = v4t - v3t;
    let ab = a.cross(b);
    let cb = c.cross(b);
    let first = v1 + a * (cb.dot(ab) / ab.dot(ab));
    LineLine::Nearest(first, first - t)
}

pub(crate) fn intersect_segment_triangle(
    p1: Vec3,
    p2: Vec3,
    triangle: [Vec3; 3],
    epsilon: f32,
) -> Option<f32> {
    let [v0, v1, v2] = triangle;
    let e1 = v1 - v0;
    let e2 = v2 - v0;
    let d = p2 - p1;
    let p = d.cross(e2);
    let a = e1.dot(p);
    if a == 0.0 {
        return None;
    }
    let f = 1.0 / a;
    let s = p1 - v0;
    let u = f * s.dot(p);
    if u < -epsilon || u > 1.0 + epsilon {
        return None;
    }
    let q = s.cross(e1);
    let v = f * d.dot(q);
    if v < -epsilon || u + v > 1.0 + epsilon {
        return None;
    }
    let lambda = f * e2.dot(q);
    (0.0..=1.0).contains(&lambda).then_some(lambda)
}

fn point_in_slice(point: Vec3, v1: Vec3, l1: Vec3, l2: Vec3) -> bool {
    let (closest, _) = closest_to_line(v1, l1, l2);
    let q = closest - v1;
    let length = q.dot(q);
    if length == 0.0 {
        return false;
    }
    let h = q.dot(point - v1) / length;
    (0.0..=1.0).contains(&h)
}

pub(crate) fn point_in_triangle_prism(point: Vec3, triangle: [Vec3; 3]) -> Option<Vec3> {
    let [v1, v2, v3] = triangle;
    if !(point_in_slice(point, v1, v2, v3)
        && point_in_slice(point, v2, v3, v1)
        && point_in_slice(point, v3, v1, v2))
    {
        return None;
    }
    let normal = (v1 - v2).cross(v2 - v3);
    let length = normal.length_squared();
    match length > 0.0 {
        true => Some(point - normal * ((point - v1).dot(normal) / length)),
        false => None,
    }
}

pub(crate) fn scale_triangle(triangle: [Vec3; 3], factor: f32) -> [Vec3; 3] {
    let center = (triangle[0] + triangle[1] + triangle[2]) / 3.0;
    triangle.map(|corner| center + (corner - center) * factor)
}

pub(crate) fn triangle_normal(triangle: [Vec3; 3]) -> Vec3 {
    let [a, b, c] = triangle;
    (b - a).cross(c - a).normalize_or_zero()
}
