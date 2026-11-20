use super::constants::MathElement;
use super::kernel::{Elem, constant, poly, scale, when};
use crate::Float;

#[inline(always)]
pub(super) fn exp<V: Float>(x: V) -> V {
    let clamped = x
        .max(constant(Elem::<V>::EXP_LOW))
        .min(constant(Elem::<V>::EXP_HIGH));
    let integral = (clamped * constant(Elem::<V>::LOG2_E)).round();
    let reduced = integral.neg_mul_add(constant(Elem::<V>::LN2_HIGH), clamped);
    let reduced = integral.neg_mul_add(constant(Elem::<V>::LN2_LOW), reduced);
    let result = scale(poly(reduced, Elem::<V>::EXP_SERIES), integral);
    when(x.is_nan(), x, result)
}

#[inline(always)]
pub(super) fn exp2<V: Float>(x: V) -> V {
    exp2_split(x, V::default())
}

#[inline(always)]
pub(super) fn exp2_split<V: Float>(high: V, low: V) -> V {
    let clamped = high
        .max(constant(Elem::<V>::EXP2_LOW))
        .min(constant(Elem::<V>::EXP2_HIGH));
    let integral = clamped.round();
    let fraction = (clamped - integral) + low;
    let reduced = fraction * constant(Elem::<V>::LN2);
    let result = scale(poly(reduced, Elem::<V>::EXP_SERIES), integral);
    when(high.is_nan(), high, result)
}
