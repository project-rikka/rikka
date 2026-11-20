use glam::{Mat3, Vec2, Vec3, Vec4};

pub fn normalize(vector: Vec3) -> Vec3 {
    let squared = vector.length_squared();
    match squared > 1.0e-35 {
        true => vector * (1.0 / squared.sqrt()),
        false => Vec3::ZERO,
    }
}

pub fn normalize_with_length(vector: Vec3) -> (Vec3, f32) {
    let squared = vector.length_squared();
    match squared > 1.0e-35 {
        true => {
            let length = squared.sqrt();
            (vector * (1.0 / length), length)
        }
        false => (Vec3::ZERO, 0.0),
    }
}

pub fn normalize_2d_length(vector: Vec2) -> (Vec2, f32) {
    let squared = vector.dot(vector);
    match squared > 1.0e-35 {
        true => {
            let length = squared.sqrt();
            (vector * (1.0 / length), length)
        }
        false => (Vec2::ZERO, 0.0),
    }
}

pub fn newell_cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        (a.y - b.y) * (a.z + b.z),
        (a.z - b.z) * (a.x + b.x),
        (a.x - b.x) * (a.y + b.y),
    )
}

pub fn normal_tri(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    normalize((a - b).cross(b - c))
}

pub fn normal_quad(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> Vec3 {
    normalize((a - c).cross(b - d))
}

pub fn newell_from(last: Vec3, points: impl Iterator<Item = Vec3>) -> Vec3 {
    let mut previous = last;
    let mut normal = Vec3::ZERO;
    for current in points {
        normal.x += (previous.y - current.y) * (previous.z + current.z);
        normal.y += (previous.z - current.z) * (previous.x + current.x);
        normal.z += (previous.x - current.x) * (previous.y + current.y);
        previous = current;
    }
    normal
}
pub fn angle_normalized(a: Vec3, b: Vec3) -> f32 {
    if a.dot(b) >= 0.0 {
        2.0 * safe_asin((b - a).length() / 2.0)
    } else {
        std::f32::consts::PI - 2.0 * safe_asin((-b - a).length() / 2.0)
    }
}

pub fn safe_asin(x: f32) -> f32 {
    x.clamp(-1.0, 1.0).asin()
}

pub fn angle_three(a: Vec3, b: Vec3, c: Vec3) -> f32 {
    angle_vectors(a - b, c - b)
}

pub fn ortho_basis(normal: Vec3) -> (Vec3, Vec3) {
    let squared = normal.x * normal.x + normal.y * normal.y;
    if squared > f32::EPSILON {
        let scale = 1.0 / squared.sqrt();
        let first = Vec3::new(normal.y * scale, -normal.x * scale, 0.0);
        let second = normal.cross(first);
        (first, second)
    } else {
        let sign = if normal.z < 0.0 { -1.0 } else { 1.0 };
        (Vec3::new(sign, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AxisProjection {
    x: Vec3,
    y: Vec3,
    z: Vec3,
}

impl AxisProjection {
    #[must_use]
    pub fn new(normal: Vec3) -> AxisProjection {
        let (x, y) = ortho_basis(normal);
        AxisProjection { x, y, z: normal }
    }

    #[must_use]
    pub fn project(&self, point: Vec3) -> Vec2 {
        Vec2::new(self.x.dot(point), self.y.dot(point))
    }

    #[must_use]
    pub fn project_3d(&self, point: Vec3) -> Vec3 {
        Vec3::new(self.x.dot(point), self.y.dot(point), self.z.dot(point))
    }
}

pub fn ortho_vector(vector: Vec3) -> Vec3 {
    let axis = vector.abs();
    if axis.x <= axis.y && axis.x <= axis.z {
        Vec3::new(0.0, -vector.z, vector.y)
    } else if axis.y <= axis.z {
        Vec3::new(-vector.z, 0.0, vector.x)
    } else {
        Vec3::new(-vector.y, vector.x, 0.0)
    }
}

pub fn line_point_factor(point: Vec3, start: Vec3, end: Vec3) -> f32 {
    let along = end - start;
    let squared = along.length_squared();
    match squared != 0.0 {
        true => (point - start).dot(along) / squared,
        false => 0.0,
    }
}

pub fn angle_vectors(a: Vec3, b: Vec3) -> f32 {
    angle_normalized(normalize(a), normalize(b))
}

pub fn angle_signed_on_axis(v1: Vec3, v2: Vec3, v3: Vec3, axis: Vec3) -> f32 {
    angle_signed_on_axis_vectors(v1 - v2, v3 - v2, axis)
}

pub fn angle_signed_on_axis_vectors(a: Vec3, b: Vec3, axis: Vec3) -> f32 {
    let first = project_plane_normalized(a, axis);
    let second = project_plane_normalized(b, axis);
    let mut angle = angle_vectors(first, second);
    if second.cross(first).dot(axis) < 0.0 {
        angle = std::f32::consts::TAU - angle;
    }
    angle
}

const SEGMENT_EPSILON: f32 = 1.0e-6;

pub fn intersect_segments_2d(v0: Vec2, v1: Vec2, v2: Vec2, v3: Vec2, bias: f32) -> Option<Vec2> {
    let s10 = v1 - v0;
    let s32 = v3 - v2;
    let s30 = v3 - v0;
    let d = cross_2d(s10, s32);
    let (low, high) = (-bias, bias + 1.0);
    if d != 0.0 {
        let u = cross_2d(s30, s32) / d;
        let v = cross_2d(s10, s30) / d;
        if (low..=high).contains(&u) && (low..=high).contains(&v) {
            let point = v0 + s10 * u;
            let v = s32.dot(point - v2) / s32.dot(s32);
            if (low..=high).contains(&v) {
                return Some(point);
            }
        }
        return None;
    }
    if cross_2d(s10, s30) == 0.0 && cross_2d(s32, s30) == 0.0 {
        let (mut v0, mut v1, mut v2, mut v3) = (v0, v1, v2, v3);
        let mut s10 = s10;
        let mut s30 = s30;
        if v0 == v1 {
            if v2.distance_squared(v3) > SEGMENT_EPSILON * SEGMENT_EPSILON {
                std::mem::swap(&mut v0, &mut v2);
                std::mem::swap(&mut v1, &mut v3);
                s10 = v1 - v0;
                s30 = v3 - v0;
            } else {
                return (v0 == v2).then_some(v0);
            }
        }
        let s20 = v2 - v0;
        let mut u_a = s20.dot(s10) / s10.dot(s10);
        let mut u_b = s30.dot(s10) / s10.dot(s10);
        if u_a > u_b {
            std::mem::swap(&mut u_a, &mut u_b);
        }
        if u_a > high || u_b < low {
            return None;
        }
        if u_a.max(0.0) == u_b.min(1.0) {
            return Some(v0 + s10 * u_a.max(0.0));
        }
    }
    None
}

pub fn normalize_2d(vector: Vec2) -> Vec2 {
    normalize_2d_length(vector).0
}

pub fn cross_2d(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

pub fn line_point_factor_2d(point: Vec2, start: Vec2, end: Vec2) -> f32 {
    let along = end - start;
    let squared = along.length_squared();
    match squared != 0.0 {
        true => (point - start).dot(along) / squared,
        false => 0.0,
    }
}

pub fn project_plane_normalized(vector: Vec3, normal: Vec3) -> Vec3 {
    vector - normal * vector.dot(normal)
}

pub fn axis_dominant(normal: Vec3) -> Mat3 {
    let (first, second) = ortho_basis(normal);
    Mat3::from_cols(first, second, normal).transpose()
}

pub fn project_2d(matrix: &Mat3, point: Vec3) -> Vec2 {
    let projected = *matrix * point;
    Vec2::new(projected.x, projected.y)
}

pub fn cross_tri(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    (a - b).cross(b - c)
}

pub fn cross_tri_2d(a: Vec2, b: Vec2, c: Vec2) -> f32 {
    (a.x - b.x) * (b.y - c.y) + (a.y - b.y) * (c.x - b.x)
}

pub fn triangle_normal(a: Vec3, b: Vec3, c: Vec3) -> (Vec3, f32) {
    normalize_with_length(cross_tri(a, b, c))
}

pub fn quad_flip_flags(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> (bool, bool) {
    let d_ab = a - b;
    let d_bc = b - c;
    let d_cd = c - d;
    let d_da = d - a;
    (
        d_ab.cross(d_bc).dot(d_cd.cross(d_da)) < 0.0,
        d_bc.cross(d_cd).dot(d_da.cross(d_ab)) < 0.0,
    )
}

pub fn quad_rotate_beauty_2d(v1: Vec2, v2: Vec2, v3: Vec2, v4: Vec2, lock_degenerate: bool) -> f32 {
    const ZERO_AREA: f32 = 1.0e-12;
    let area_234 = cross_tri_2d(v2, v3, v4);
    let area_241 = cross_tri_2d(v2, v4, v1);
    let area_123 = cross_tri_2d(v1, v2, v3);
    let area_134 = cross_tri_2d(v1, v3, v4);
    if (area_123 >= 0.0) != (area_134 >= 0.0) {
        return f32::MAX;
    }
    if area_123.abs() <= ZERO_AREA || area_134.abs() <= ZERO_AREA {
        return f32::MAX;
    }
    if (area_234 >= 0.0) != (area_241 >= 0.0) {
        return match lock_degenerate {
            true => f32::MAX,
            false => -f32::MAX,
        };
    }
    if area_234.abs() <= ZERO_AREA || area_241.abs() <= ZERO_AREA {
        return -f32::MAX;
    }
    let len_12 = v1.distance(v2);
    let len_23 = v2.distance(v3);
    let len_34 = v3.distance(v4);
    let len_41 = v4.distance(v1);
    let len_13 = v1.distance(v3);
    let len_24 = v2.distance(v4);
    let fac_24 =
        area_234.abs() / (len_23 + len_34 + len_24) + area_241.abs() / (len_41 + len_12 + len_24);
    let fac_13 =
        area_123.abs() / (len_12 + len_23 + len_13) + area_134.abs() / (len_34 + len_41 + len_13);
    fac_24 - fac_13
}

fn signum_epsilon(value: f32, epsilon: f32) -> i32 {
    if value > epsilon {
        1
    } else if value < -epsilon {
        -1
    } else {
        0
    }
}

pub fn rotate_beauty_area(v1: Vec3, v2: Vec3, v3: Vec3, v4: Vec3, lock_degenerate: bool) -> f32 {
    const EPSILON: f32 = 1.0e-5;
    let (normal, scale) = normalize_with_length(cross_tri(v2, v3, v4) + cross_tri(v2, v4, v1));
    if scale == 0.0 {
        return f32::MAX;
    }
    let projection = AxisProjection::new(normal);
    let [p1, p2, p3, p4] = [v1, v2, v3, v4].map(|point| projection.project(point));
    if signum_epsilon(cross_tri_2d(p2, p3, p4) / scale, EPSILON)
        + signum_epsilon(cross_tri_2d(p2, p4, p1) / scale, EPSILON)
        == 0
    {
        return f32::MAX;
    }
    quad_rotate_beauty_2d(p1, p2, p3, p4, lock_degenerate)
}

pub fn rotate_beauty_angle(v1: Vec3, v2: Vec3, v3: Vec3, v4: Vec3) -> f32 {
    let (normal_a, _) = triangle_normal(v2, v3, v4);
    let (normal_b, _) = triangle_normal(v2, v4, v1);
    let angle_24 = angle_normalized(normal_a, normal_b);
    let (normal_a, length_a) = triangle_normal(v1, v2, v3);
    let (normal_b, length_b) = triangle_normal(v1, v3, v4);
    if length_a == 0.0 || length_b == 0.0 {
        return f32::MAX;
    }
    angle_normalized(normal_a, normal_b) - angle_24
}

pub(crate) const NOISE_HASH: [u8; 256] = [
    0xA2, 0xA0, 0x19, 0x3B, 0xF8, 0xEB, 0xAA, 0xEE, 0xF3, 0x1C, 0x67, 0x28, 0x1D, 0xED, 0x00, 0xDE,
    0x95, 0x2E, 0xDC, 0x3F, 0x3A, 0x82, 0x35, 0x4D, 0x6C, 0xBA, 0x36, 0xD0, 0xF6, 0x0C, 0x79, 0x32,
    0xD1, 0x59, 0xF4, 0x08, 0x8B, 0x63, 0x89, 0x2F, 0xB8, 0xB4, 0x97, 0x83, 0xF2, 0x8F, 0x18, 0xC7,
    0x51, 0x14, 0x65, 0x87, 0x48, 0x20, 0x42, 0xA8, 0x80, 0xB5, 0x40, 0x13, 0xB2, 0x22, 0x7E, 0x57,
    0xBC, 0x7F, 0x6B, 0x9D, 0x86, 0x4C, 0xC8, 0xDB, 0x7C, 0xD5, 0x25, 0x4E, 0x5A, 0x55, 0x74, 0x50,
    0xCD, 0xB3, 0x7A, 0xBB, 0xC3, 0xCB, 0xB6, 0xE2, 0xE4, 0xEC, 0xFD, 0x98, 0x0B, 0x96, 0xD3, 0x9E,
    0x5C, 0xA1, 0x64, 0xF1, 0x81, 0x61, 0xE1, 0xC4, 0x24, 0x72, 0x49, 0x8C, 0x90, 0x4B, 0x84, 0x34,
    0x38, 0xAB, 0x78, 0xCA, 0x1F, 0x01, 0xD7, 0x93, 0x11, 0xC1, 0x58, 0xA9, 0x31, 0xF9, 0x44, 0x6D,
    0xBF, 0x33, 0x9C, 0x5F, 0x09, 0x94, 0xA3, 0x85, 0x06, 0xC6, 0x9A, 0x1E, 0x7B, 0x46, 0x15, 0x30,
    0x27, 0x2B, 0x1B, 0x71, 0x3C, 0x5B, 0xD6, 0x6F, 0x62, 0xAC, 0x4F, 0xC2, 0xC0, 0x0E, 0xB1, 0x23,
    0xA7, 0xDF, 0x47, 0xB0, 0x77, 0x69, 0x05, 0xE9, 0xE6, 0xE7, 0x76, 0x73, 0x0F, 0xFE, 0x6E, 0x9B,
    0x56, 0xEF, 0x12, 0xA5, 0x37, 0xFC, 0xAE, 0xD9, 0x03, 0x8E, 0xDD, 0x10, 0xB9, 0xCE, 0xC9, 0x8D,
    0xDA, 0x2A, 0xBD, 0x68, 0x17, 0x9F, 0xBE, 0xD4, 0x0A, 0xCC, 0xD2, 0xE8, 0x43, 0x3D, 0x70, 0xB7,
    0x02, 0x7D, 0x99, 0xD8, 0x0D, 0x60, 0x8A, 0x04, 0x2C, 0x3E, 0x92, 0xE5, 0xAF, 0x53, 0x07, 0xE0,
    0x29, 0xA6, 0xC5, 0xE3, 0xF5, 0xF7, 0x4A, 0x41, 0x26, 0x6A, 0x16, 0x5E, 0x52, 0x2D, 0x21, 0xAD,
    0xF0, 0x91, 0xFF, 0xEA, 0x54, 0xFA, 0x66, 0x1A, 0x45, 0x39, 0xCF, 0x75, 0xA4, 0x88, 0xFB, 0x5D,
];

fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(t: f32, a: f32, b: f32) -> f32 {
    a + t * (b - a)
}

fn gradient(hash: i32, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

fn improved_perlin(x: f32, y: f32, z: f32) -> f32 {
    let (u, v, w) = (x.floor(), y.floor(), z.floor());
    let cx = (u as i32) & 255;
    let cy = (v as i32) & 255;
    let cz = (w as i32) & 255;
    let (x, y, z) = (x - u, y - v, z - w);
    let (u, v, w) = (fade(x), fade(y), fade(z));
    let table = |index: i32| i32::from(NOISE_HASH[(index & 255) as usize]);
    let a = table(cx) + cy;
    let aa = table(a) + cz;
    let ab = table(a + 1) + cz;
    let b = table(cx + 1) + cy;
    let ba = table(b) + cz;
    let bb = table(b + 1) + cz;
    lerp(
        w,
        lerp(
            v,
            lerp(
                u,
                gradient(table(aa), x, y, z),
                gradient(table(ba), x - 1.0, y, z),
            ),
            lerp(
                u,
                gradient(table(ab), x, y - 1.0, z),
                gradient(table(bb), x - 1.0, y - 1.0, z),
            ),
        ),
        lerp(
            v,
            lerp(
                u,
                gradient(table(aa + 1), x, y, z - 1.0),
                gradient(table(ba + 1), x - 1.0, y, z - 1.0),
            ),
            lerp(
                u,
                gradient(table(ab + 1), x, y - 1.0, z - 1.0),
                gradient(table(bb + 1), x - 1.0, y - 1.0, z - 1.0),
            ),
        ),
    )
}

pub fn turbulence(x: f32, y: f32, z: f32, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amplitude = 1.0;
    let mut scale = 1.0;
    for _ in 0..=octaves {
        let noise = 0.5 + 0.5 * improved_perlin(scale * x, scale * y, scale * z);
        sum += noise * amplitude;
        amplitude *= 0.5;
        scale *= 2.0;
    }
    sum * (1u32 << octaves) as f32 / ((1u32 << (octaves + 1)) - 1) as f32
}

pub fn uv_equal(a: Vec2, b: Vec2) -> bool {
    a.distance_squared(b) < 0.00001
}

pub fn color_equal(a: Vec4, b: Vec4) -> bool {
    let difference = a - b;
    difference.dot(difference) < 0.001
}

pub fn compare_vec3(a: Vec3, b: Vec3, limit: f32) -> bool {
    (a.x - b.x).abs() <= limit && (a.y - b.y).abs() <= limit && (a.z - b.z).abs() <= limit
}

pub fn project_plane(vector: Vec3, normal: Vec3) -> Vec3 {
    vector - normal * (vector.dot(normal) / normal.dot(normal))
}

pub fn closest_to_line(point: Vec3, start: Vec3, end: Vec3) -> (Vec3, f32) {
    let direction = end - start;
    if direction == Vec3::ZERO {
        return (start, 0.0);
    }
    let lambda = direction.dot(point - start) / direction.dot(direction);
    (start + direction * lambda, lambda)
}

pub fn angle_signed_2d(a: Vec2, b: Vec2) -> f32 {
    let perp_dot = a.y * b.x - a.x * b.y;
    perp_dot.atan2(a.dot(b))
}

pub fn intersect_line_triangle(p1: Vec3, p2: Vec3, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<f32> {
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
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = f * d.dot(q);
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(f * e2.dot(q))
}
