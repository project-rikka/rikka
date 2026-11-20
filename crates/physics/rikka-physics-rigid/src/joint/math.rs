use rikka_physics_math::{Mat3W, QuatW, Vec3, Vec3W, Wide};

use super::model::JointFrameW;
use crate::solver::lanes::BodyW;
use crate::solver::sym::SymW;

#[inline(always)]
pub(crate) fn skew<W: Wide>(r: Vec3W<W>) -> Mat3W<W> {
    let zero = W::default();
    Mat3W::from_cols(
        Vec3W::new(zero, r.z, -r.y),
        Vec3W::new(-r.z, zero, r.x),
        Vec3W::new(r.y, -r.x, zero),
    )
}

#[inline(always)]
pub(crate) fn to_mat<W: Wide>(s: &SymW<W>) -> Mat3W<W> {
    Mat3W::from_cols(
        Vec3W::new(s.xx, s.xy, s.xz),
        Vec3W::new(s.xy, s.yy, s.yz),
        Vec3W::new(s.xz, s.yz, s.zz),
    )
}

#[inline(always)]
pub(crate) fn sandwich<W: Wide>(r: Vec3W<W>, m: &SymW<W>) -> Mat3W<W> {
    let s = skew(r);
    s.mul_mat(&to_mat(m).mul_mat(&s.transpose()))
}

#[inline(always)]
pub(crate) fn point_mass<W: Wide>(
    frame: &JointFrameW<W>,
    r_a: Vec3W<W>,
    r_b: Vec3W<W>,
) -> Mat3W<W> {
    let k_a = sandwich(r_a, &frame.inv_i_a);
    let k_b = sandwich(r_b, &frame.inv_i_b);
    let m = frame.inv_mass_a + frame.inv_mass_b;
    let mut k = k_a + k_b;
    k.x_axis.x += m;
    k.y_axis.y += m;
    k.z_axis.z += m;
    k
}

#[inline(always)]
pub(crate) fn apply_linear<W: Wide>(
    frame: &JointFrameW<W>,
    a: &mut BodyW<W>,
    b: &mut BodyW<W>,
    r_a: Vec3W<W>,
    r_b: Vec3W<W>,
    impulse: Vec3W<W>,
) {
    a.v = impulse.mul_add(-frame.inv_mass_a, a.v);
    a.w -= frame.inv_i_a.mul(r_a.cross(impulse));
    b.v = impulse.mul_add(frame.inv_mass_b, b.v);
    b.w += frame.inv_i_b.mul(r_b.cross(impulse));
}

#[inline(always)]
pub(crate) fn apply_angular<W: Wide>(
    frame: &JointFrameW<W>,
    a: &mut BodyW<W>,
    b: &mut BodyW<W>,
    impulse: Vec3W<W>,
) {
    a.w -= frame.inv_i_a.mul(impulse);
    b.w += frame.inv_i_b.mul(impulse);
}

#[inline(always)]
pub(crate) fn inverse_mul<W: Wide>(a: QuatW<W>, b: QuatW<W>) -> QuatW<W> {
    a.conjugate() * b
}

#[inline(always)]
pub(crate) fn delta_rotation<W: Wide>(q: QuatW<W>, target: QuatW<W>) -> Vec3W<W> {
    let flip = q.dot(target).simd_lt(W::default());
    let s = QuatW::select(flip, -q, q);
    let diff = QuatW::new(target.v - s.v, target.s - s.s);
    let product = diff * s.conjugate();
    product.v.scale(W::splat(2.0))
}

#[inline(always)]
pub(crate) fn axis<W: Wide>(q: QuatW<W>, local: Vec3) -> Vec3W<W> {
    q.rotate(Vec3W::splat(local))
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RowSoftness<W: Wide> {
    pub bias: W,
    pub mass_scale: W,
    pub impulse_scale: W,
}

#[inline(always)]
pub(crate) fn limit_softness<W: Wide>(
    c: W,
    frame: &JointFrameW<W>,
    inv_h: f32,
    use_bias: bool,
) -> RowSoftness<W> {
    let zero = W::default();
    let speculative = c.simd_gt(zero);
    let (bias, mass_scale, impulse_scale) = if use_bias {
        (frame.bias_rate * c, frame.mass_scale, frame.impulse_scale)
    } else {
        (zero, W::one(), zero)
    };
    RowSoftness {
        bias: W::select(speculative, c * W::splat(inv_h), bias),
        mass_scale: W::select(speculative, W::one(), mass_scale),
        impulse_scale: W::select(speculative, zero, impulse_scale),
    }
}
