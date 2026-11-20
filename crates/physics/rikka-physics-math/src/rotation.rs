use glam::{Mat3, Quat, Vec3};

#[inline]
pub fn perpendicular(v: Vec3) -> Vec3 {
    if v.x.abs() >= 0.57735 {
        Vec3::new(v.y, -v.x, 0.0).normalize()
    } else {
        Vec3::new(0.0, v.z, -v.y).normalize()
    }
}

pub fn quat_between(from: Vec3, to: Vec3) -> Quat {
    let middle = from.lerp(to, 0.5);
    let tolerance = 100.0 * f32::EPSILON;
    let unnormalized = if middle.length_squared() > tolerance * tolerance {
        let axis = from.cross(middle);
        Quat::from_xyzw(axis.x, axis.y, axis.z, from.dot(middle))
    } else if from.x.abs() > 0.5 {
        Quat::from_xyzw(from.y, -from.x, 0.0, 0.0)
    } else {
        Quat::from_xyzw(0.0, from.z, -from.y, 0.0)
    };
    unnormalized.normalize()
}

pub fn quat_from_basis(basis: Mat3) -> Quat {
    let c1 = basis.x_axis;
    let c2 = basis.y_axis;
    let c3 = basis.z_axis;
    let trace = c1.x + c2.y + c3.z;
    let q = if trace >= 0.0 {
        Quat::from_xyzw(c2.z - c3.y, c3.x - c1.z, c1.y - c2.x, trace + 1.0)
    } else if c1.x > c2.y && c1.x > c3.z {
        Quat::from_xyzw(
            c1.x - c2.y - c3.z + 1.0,
            c2.x + c1.y,
            c3.x + c1.z,
            c2.z - c3.y,
        )
    } else if c2.y > c3.z {
        Quat::from_xyzw(
            c1.y + c2.x,
            c2.y - c3.z - c1.x + 1.0,
            c3.y + c2.z,
            c3.x - c1.z,
        )
    } else {
        Quat::from_xyzw(
            c1.z + c3.x,
            c2.z + c3.y,
            c3.z - c1.x - c2.y + 1.0,
            c1.y - c2.x,
        )
    };
    q.normalize()
}

#[inline]
pub fn nlerp(a: Quat, b: Quat, alpha: f32) -> Quat {
    let b = if a.dot(b) < 0.0 { -b } else { b };
    (a * (1.0 - alpha) + b * alpha).normalize()
}

#[inline]
pub fn twist_angle(q: Quat) -> f32 {
    2.0 * crate::atan2(q.z, q.w)
}

pub fn axis_angle(q: Quat) -> (Vec3, f32) {
    let q = if q.w < 0.0 { -q } else { q };
    let sine = Vec3::new(q.x, q.y, q.z).length();
    let angle = 2.0 * crate::atan2(sine, q.w);
    if sine > 1e-9 {
        (Vec3::new(q.x, q.y, q.z) / sine, angle)
    } else {
        (Vec3::X, 0.0)
    }
}

pub fn align_rotation(rotation: Quat, from: Vec3, to: Vec3) -> Quat {
    quat_between(from, to) * rotation
}
