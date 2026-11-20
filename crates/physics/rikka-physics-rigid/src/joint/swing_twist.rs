use rikka_physics_math::{Quat, QuatW, Vec3, Vec3W, Wide};

const SINGULAR: f32 = 1.0e-9;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SwingTwistW<W: Wide> {
    pub swing_x: Vec3W<W>,
    pub swing_y: Vec3W<W>,
    pub twist: Vec3W<W>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ConeW<W: Wide> {
    pub direction_x: W,
    pub direction_y: W,
    pub jacobian: Vec3W<W>,
}

#[inline(always)]
fn gradient_to_rate<W: Wide>(q: QuatW<W>, gradient: Vec3W<W>, gradient_s: W) -> Vec3W<W> {
    gradient.scale(q.s) - q.v.scale(gradient_s) + q.v.cross(gradient)
}

#[inline(always)]
pub(crate) fn swing_twist<W: Wide>(q: QuatW<W>) -> SwingTwistW<W> {
    let (x, y, z, w) = (q.v.x, q.v.y, q.v.z, q.s);
    let two = W::splat(2.0);
    let d = (z * z + w * w).max(W::splat(SINGULAR));
    let inverse_d = W::one() / d;
    let twist = Vec3W::new(
        (x * z + y * w) * inverse_d,
        (y * z - x * w) * inverse_d,
        W::one(),
    );
    let nx = w * x - y * z;
    let ny = w * y + x * z;
    let swing_x = {
        let gradient = Vec3W::new(d * w, -(d * z), -(d * y) - two * nx * z);
        let gradient_s = d * x - two * nx * w;
        gradient_to_rate(q, gradient, gradient_s).scale(W::one() / (d * d + nx * nx))
    };
    let swing_y = {
        let gradient = Vec3W::new(d * z, d * w, d * x - two * ny * z);
        let gradient_s = d * y - two * ny * w;
        gradient_to_rate(q, gradient, gradient_s).scale(W::one() / (d * d + ny * ny))
    };
    SwingTwistW {
        swing_x,
        swing_y,
        twist,
    }
}

#[inline(always)]
pub(crate) fn twist_angle<W: Wide>(q: QuatW<W>) -> W {
    let flip = q.s.simd_lt(W::default());
    W::splat(2.0) * W::select(flip, -q.v.z, q.v.z).atan2(q.s.abs())
}

#[inline(always)]
pub(crate) fn swing_angles<W: Wide>(q: QuatW<W>) -> [W; 2] {
    let (x, y, z, w) = (q.v.x, q.v.y, q.v.z, q.s);
    let two = W::splat(2.0);
    let d = (z * z + w * w).max(W::splat(SINGULAR));
    [
        two * (w * x - y * z).atan2(d),
        two * (w * y + x * z).atan2(d),
    ]
}

#[inline(always)]
pub(crate) fn cone_angle<W: Wide>(q: QuatW<W>) -> W {
    let (x, y, z, w) = (q.v.x, q.v.y, q.v.z, q.s);
    W::splat(2.0) * (x * x + y * y).sqrt().atan2((z * z + w * w).sqrt())
}

#[inline(always)]
pub(crate) fn cone<W: Wide>(q: QuatW<W>) -> ConeW<W> {
    let (x, y, z, w) = (q.v.x, q.v.y, q.v.z, q.s);
    let floor = W::splat(SINGULAR);
    let sine = (x * x + y * y).sqrt();
    let cosine = (z * z + w * w).sqrt();
    let safe_sine = sine.max(floor);
    let safe_cosine = cosine.max(floor);
    let ratio = cosine / safe_sine;
    let inverse_ratio = sine / safe_cosine;
    let gradient = Vec3W::new(x * ratio, y * ratio, -(z * inverse_ratio));
    let gradient_s = -(w * inverse_ratio);
    let jacobian = Vec3W::select(
        sine.simd_gt(floor),
        gradient_to_rate(q, gradient, gradient_s),
        Vec3W::zero(),
    );
    let nx = w * x - y * z;
    let ny = w * y + x * z;
    let length = (nx * nx + ny * ny).sqrt().max(floor);
    ConeW {
        direction_x: nx / length,
        direction_y: ny / length,
        jacobian,
    }
}

#[inline(always)]
pub(crate) fn elliptic_limit<W: Wide>(limit_x: W, limit_y: W, direction_x: W, direction_y: W) -> W {
    let u = limit_y * direction_x;
    let v = limit_x * direction_y;
    let denominator = (u * u + v * v).sqrt();
    W::select(
        denominator.simd_gt(W::splat(f32::EPSILON)),
        limit_x * limit_y / denominator.max(W::splat(f32::EPSILON)),
        limit_x.max(limit_y),
    )
}

pub(crate) fn angles(q: Quat) -> Vec3 {
    let q = if q.w < 0.0 { -q } else { q };
    let d = (q.z * q.z + q.w * q.w).max(SINGULAR);
    Vec3::new(
        2.0 * rikka_physics_math::atan2(q.w * q.x - q.y * q.z, d),
        2.0 * rikka_physics_math::atan2(q.w * q.y + q.x * q.z, d),
        2.0 * rikka_physics_math::atan2(q.z, q.w),
    )
}

pub(crate) fn compose(angles: Vec3) -> Quat {
    let swing =
        Quat::from_xyzw((0.5 * angles.x).tan(), (0.5 * angles.y).tan(), 0.0, 1.0).normalize();
    swing * Quat::from_rotation_z(angles.z)
}
