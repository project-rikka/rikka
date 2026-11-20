use std::f32::consts::PI;

use glam::{Mat4, Vec2, Vec3, Vec4};
pub(crate) use rikka_mesh_edit::math::{
    angle_normalized, angle_three, angle_vectors, compare_vec3, normalize, normalize_2d,
    normalize_with_length, safe_asin,
};

pub(crate) const EPSILON: f32 = 1.0e-6;
pub(crate) const EPSILON_D: f64 = 1.0e-6;
pub(crate) const EPSILON_SQ: f32 = 1.0e-12;
pub(crate) const EPSILON_BIG: f32 = 1.0e-4;
pub(crate) const EPSILON_ANG: f32 = 2.0 * (std::f64::consts::PI / 180.0) as f32;
pub(crate) const SMALL_ANG: f32 = 10.0 * (std::f64::consts::PI / 180.0) as f32;
const LINE_EPSILON: f32 = 1.0e-6;

pub(crate) fn small_angle_dot() -> f32 {
    1.0 - SMALL_ANG.cos()
}

pub(crate) fn epsilon_angle_dot() -> f32 {
    1.0 - EPSILON_ANG.cos()
}

pub(crate) fn angle_2d(a: Vec2, b: Vec2) -> f32 {
    let a = normalize_2d(a);
    let b = normalize_2d(b);
    if a.dot(b) >= 0.0 {
        2.0 * safe_asin(a.distance(b) / 2.0)
    } else {
        PI - 2.0 * safe_asin(a.distance(-b) / 2.0)
    }
}

pub(crate) fn nearly_parallel(a: Vec3, b: Vec3) -> bool {
    let between = angle_vectors(a, b);
    between.abs() < EPSILON_ANG || (between - PI).abs() < EPSILON_ANG
}

pub(crate) fn nearly_parallel_normalized(a: Vec3, b: Vec3) -> bool {
    (a.dot(b).abs() - 1.0).abs() <= epsilon_angle_dot()
}

pub(crate) fn compare(a: f32, b: f32, limit: f32) -> bool {
    (a - b).abs() <= limit
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum LineMeet {
    Parallel,
    Point(Vec3),
    Skew(Vec3, Vec3),
}

impl LineMeet {
    pub(crate) fn first(self) -> Option<Vec3> {
        match self {
            LineMeet::Parallel => None,
            LineMeet::Point(point) | LineMeet::Skew(point, _) => Some(point),
        }
    }
}

fn project_onto(point: Vec3, onto: Vec3) -> Vec3 {
    if onto == Vec3::ZERO {
        return Vec3::ZERO;
    }
    onto * (point.dot(onto) / onto.dot(onto))
}

pub(crate) fn line_line(v1: Vec3, v2: Vec3, v3: Vec3, v4: Vec3) -> LineMeet {
    let c = v3 - v1;
    let a = v2 - v1;
    let b = v4 - v3;
    let ab = a.cross(b);
    let d = c.dot(ab);
    let div = ab.dot(ab);
    if div == 0.0 {
        return LineMeet::Parallel;
    }
    if d.abs() <= LINE_EPSILON {
        let cb = c.cross(b);
        let point = v1 + a * (cb.dot(ab) / div);
        return match point.is_finite() {
            true => LineMeet::Point(point),
            false => LineMeet::Parallel,
        };
    }
    let t = project_onto(v1 - v3, a.cross(b));
    let v3t = v3 + t;
    let v4t = v4 + t;
    let c = v3t - v1;
    let a = v2 - v1;
    let b = v4t - v3t;
    let ab = a.cross(b);
    let cb = c.cross(b);
    let first = v1 + a * (cb.dot(ab) / ab.dot(ab));
    match first.is_finite() && t.is_finite() {
        true => LineMeet::Skew(first, first - t),
        false => LineMeet::Parallel,
    }
}

pub(crate) fn line_plane(l1: Vec3, l2: Vec3, plane_co: Vec3, plane_no: Vec3) -> Option<Vec3> {
    let u = l2 - l1;
    let h = l1 - plane_co;
    let dot = plane_no.dot(u);
    if dot.abs() > f32::EPSILON {
        let lambda = -plane_no.dot(h) / dot;
        Some(l1 + u * lambda)
    } else {
        None
    }
}

pub(crate) fn closest_to_line(point: Vec3, l1: Vec3, l2: Vec3) -> (Vec3, f32) {
    let direction = l2 - l1;
    if direction == Vec3::ZERO {
        return (l1, 0.0);
    }
    let lambda = direction.dot(point - l1) / direction.dot(direction);
    (l1 + direction * lambda, lambda)
}

pub(crate) fn closest_to_segment(point: Vec3, l1: Vec3, l2: Vec3) -> Vec3 {
    let (closest, lambda) = closest_to_line(point, l1, l2);
    if lambda <= 0.0 {
        l1
    } else if lambda >= 1.0 {
        l2
    } else {
        closest
    }
}

pub(crate) fn distance_squared_to_segment(point: Vec3, l1: Vec3, l2: Vec3) -> f32 {
    closest_to_segment(point, l1, l2).distance_squared(point)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Plane {
    normal: Vec3,
    offset: f32,
}

impl Plane {
    pub(crate) fn new(point: Vec3, normal: Vec3) -> Plane {
        Plane {
            normal,
            offset: -normal.dot(point),
        }
    }

    fn side(self, point: Vec3) -> f32 {
        point.dot(self.normal) + self.offset
    }

    pub(crate) fn closest(self, point: Vec3) -> Vec3 {
        let length_sq = self.normal.length_squared();
        point + self.normal * (-self.side(point) / length_sq)
    }

    pub(crate) fn closest_normalized(self, point: Vec3) -> Vec3 {
        point + self.normal * -self.side(point)
    }

    pub(crate) fn distance_squared(self, point: Vec3) -> f32 {
        let length_sq = self.normal.length_squared();
        let factor = self.side(point) / length_sq;
        length_sq * (factor * factor)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AxisProjection {
    x: Vec3,
    y: Vec3,
}

impl AxisProjection {
    pub(crate) fn new(normal: Vec3) -> AxisProjection {
        let length_sq = normal.x * normal.x + normal.y * normal.y;
        if length_sq > f32::EPSILON {
            let inverse = 1.0 / length_sq.sqrt();
            let x = Vec3::new(normal.y * inverse, -normal.x * inverse, 0.0);
            let y = Vec3::new(
                -normal.z * x.y,
                normal.z * x.x,
                normal.x * x.y - normal.y * x.x,
            );
            AxisProjection { x, y }
        } else {
            let sign = match normal.z < 0.0 {
                true => -1.0,
                false => 1.0,
            };
            AxisProjection {
                x: Vec3::new(sign, 0.0, 0.0),
                y: Vec3::new(0.0, 1.0, 0.0),
            }
        }
    }

    pub(crate) fn apply(self, point: Vec3) -> Vec2 {
        Vec2::new(self.x.dot(point), self.y.dot(point))
    }
}

pub(crate) fn polygon_area(points: &[Vec2]) -> f32 {
    let Some(&last) = points.last() else {
        return 0.0;
    };
    let mut previous = last;
    let mut cross = 0.0;
    for &current in points {
        cross += (previous.x - current.x) * (current.y + previous.y);
        previous = current;
    }
    (0.5 * cross).abs()
}

pub(crate) fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    let count = polygon.len();
    let mut j = count.wrapping_sub(1);
    for i in 0..count {
        let (a, b) = (polygon[i], polygon[j]);
        if ((a.y > point.y) != (b.y > point.y))
            && (point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

pub(crate) fn bilinear_quad(quad: [Vec3; 4], u: f32, v: f32) -> Vec3 {
    let mut result = quad[0] * ((1.0 - u) * (1.0 - v));
    result += quad[1] * (u * (1.0 - v));
    result += quad[2] * (u * v);
    result += quad[3] * ((1.0 - u) * v);
    result
}

pub(crate) fn lerp(a: Vec3, b: Vec3, factor: f32) -> Vec3 {
    let inverse = 1.0 - factor;
    Vec3::new(
        inverse * a.x + factor * b.x,
        inverse * a.y + factor * b.y,
        inverse * a.z + factor * b.z,
    )
}

pub(crate) fn midpoint(a: Vec3, b: Vec3) -> Vec3 {
    (a + b) * 0.5
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Affine {
    columns: [Vec3; 4],
}

impl Affine {
    pub(crate) fn transform(self, point: Vec3) -> Vec3 {
        let [c0, c1, c2, c3] = self.columns;
        Vec3::new(
            point.x * c0.x + point.y * c1.x + point.z * c2.x + c3.x,
            point.x * c0.y + point.y * c1.y + point.z * c2.y + c3.y,
            point.x * c0.z + point.y * c1.z + point.z * c2.z + c3.z,
        )
    }

    pub(crate) fn inverse(self) -> Option<Affine> {
        let [c0, c1, c2, c3] = self.columns;
        let matrix = Mat4::from_cols(
            Vec4::from((c0, 0.0)),
            Vec4::from((c1, 0.0)),
            Vec4::from((c2, 0.0)),
            Vec4::from((c3, 1.0)),
        );
        let determinant = matrix.determinant();
        if determinant == 0.0 || !determinant.is_finite() {
            return None;
        }
        let inverse = matrix.inverse();
        Some(Affine {
            columns: [
                inverse.x_axis.truncate(),
                inverse.y_axis.truncate(),
                inverse.z_axis.truncate(),
                inverse.w_axis.truncate(),
            ],
        })
    }

    pub(crate) fn unit_square(va: Vec3, vmid: Vec3, vb: Vec3) -> Option<Affine> {
        let va_vmid = vmid - va;
        let vb_vmid = vmid - vb;
        if va_vmid == Vec3::ZERO || vb_vmid == Vec3::ZERO {
            return None;
        }
        if (angle_vectors(va_vmid, vb_vmid) - PI).abs() <= EPSILON_ANG {
            return None;
        }
        let vo = va - vb_vmid;
        let vddir = normalize(vb_vmid.cross(va_vmid));
        let vd = vo + vddir;
        Some(Affine {
            columns: [vmid - va, vmid - vb, vmid + vd - va - vb, va + vb - vmid],
        })
    }

    pub(crate) fn unit_cube(va: Vec3, vb: Vec3, vc: Vec3, vd: Vec3) -> Affine {
        Affine {
            columns: [
                (va - vb - vc + vd) * 0.5,
                (vb - va - vc + vd) * 0.5,
                (vc - va - vb + vd) * 0.5,
                (va + vb + vc - vd) * 0.5,
            ],
        }
    }
}

pub(crate) fn safe_divide(a: f32, b: f32) -> f32 {
    match b == 0.0 {
        true => 0.0,
        false => a / b,
    }
}
