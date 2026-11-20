use super::constants::MathElement;
use super::kernel::{Elem, constant, negative_sign, poly, when};
use crate::Float;

#[inline(always)]
fn atan_magnitude<V: Float>(magnitude: V) -> V {
    let one = constant::<V>(Elem::<V>::ONE);
    let beyond = magnitude.simd_gt(constant(Elem::<V>::TAN_3PI_8));
    let middle = magnitude.simd_gt(constant(Elem::<V>::TAN_PI_8)) & !beyond;
    let reduced = when(
        beyond,
        -(one / magnitude),
        when(middle, (magnitude - one) / (magnitude + one), magnitude),
    );
    let (offset_high, offset_low) = (
        when(
            beyond,
            constant(Elem::<V>::PI_2_HIGH),
            when(middle, constant(Elem::<V>::PI_4_HIGH), V::default()),
        ),
        when(
            beyond,
            constant(Elem::<V>::PI_2_LOW),
            when(middle, constant(Elem::<V>::PI_4_LOW), V::default()),
        ),
    );
    let square = reduced * reduced;
    let series = (reduced * square).mul_add(poly(square, Elem::<V>::ATAN_SERIES), reduced);
    offset_high + (series + offset_low)
}

#[inline(always)]
pub(super) fn atan<V: Float>(x: V) -> V {
    atan_magnitude(x.abs()).copysign(x)
}

#[inline(always)]
pub(super) fn atan2<V: Float>(y: V, x: V) -> V {
    let zero = V::default();
    let (across, along) = (y.abs(), x.abs());
    let steep = across.simd_gt(along);
    let ratio = when(steep, along, across) / when(steep, across, along);
    let angle = atan_magnitude(ratio);
    let angle = when(
        steep,
        (constant::<V>(Elem::<V>::PI_2_HIGH) - angle) + constant(Elem::<V>::PI_2_LOW),
        angle,
    );
    let backwards = negative_sign(x);
    let angle = when(
        backwards,
        (constant::<V>(Elem::<V>::PI_HIGH) - angle) + constant(Elem::<V>::PI_LOW),
        angle,
    );
    let infinity = constant::<V>(Elem::<V>::INFINITY);
    let both_zero = across.simd_eq(zero) & along.simd_eq(zero);
    let angle = when(
        both_zero,
        when(backwards, constant(Elem::<V>::PI_HIGH), zero),
        angle,
    );
    let quarter = constant::<V>(Elem::<V>::PI_4_HIGH);
    let both_infinite = across.simd_eq(infinity) & along.simd_eq(infinity);
    let three_quarters = quarter * constant(Elem::<V>::TWO) + quarter;
    let angle = when(
        both_infinite,
        when(backwards, three_quarters, quarter),
        angle,
    );
    let angle = when(x.is_nan() | y.is_nan(), x + y, angle);
    angle.copysign(y)
}

#[inline(always)]
fn complement<V: Float>(x: V) -> V {
    let one = constant::<V>(Elem::<V>::ONE);
    ((one - x) * (one + x)).sqrt()
}

#[inline(always)]
pub(super) fn asin<V: Float>(x: V) -> V {
    atan2(x, complement(x))
}

#[inline(always)]
pub(super) fn acos<V: Float>(x: V) -> V {
    atan2(complement(x), x)
}
