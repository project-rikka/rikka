use super::constants::MathElement;
use crate::element::FloatElement;
use crate::{Float, Mask, Simd, Vector};

pub(super) type Elem<V> = <V as Vector>::Elem;

#[inline(always)]
pub(super) fn constant<V: Float>(value: Elem<V>) -> V {
    V::splat(value)
}

#[inline(always)]
pub(super) fn bits<V: Float>(value: u64) -> V::Bits {
    V::Bits::splat(<Elem<V> as FloatElement>::bits(value))
}

#[inline(always)]
pub(super) fn poly<V: Float>(x: V, coefficients: &[Elem<V>]) -> V {
    let mut sum = V::splat(coefficients[0]);
    for &coefficient in &coefficients[1..] {
        sum = sum.mul_add(x, V::splat(coefficient));
    }
    sum
}

#[inline(always)]
pub(super) fn integer_bits<V: Float>(integral: V) -> V::Bits {
    let magic = constant::<V>(Elem::<V>::ROUNDING_MAGIC);
    (integral + magic).to_bits() - magic.to_bits()
}

#[inline(always)]
pub(super) fn small_integer<V: Float>(value: V::Bits) -> V {
    let scale = constant::<V>(Elem::<V>::MANTISSA_SCALE);
    V::from_bits(value | scale.to_bits()) - scale
}

#[inline(always)]
pub(super) fn power_of_two<V: Float>(integral: V) -> V {
    let biased = integer_bits(integral) + bits::<V>(Elem::<V>::BIAS);
    V::from_bits(biased << Elem::<V>::MANTISSA)
}

#[inline(always)]
pub(super) fn scale<V: Float>(value: V, integral: V) -> V {
    let half = (integral * constant(Elem::<V>::HALF)).trunc();
    value * power_of_two(half) * power_of_two(integral - half)
}

#[inline(always)]
pub(super) fn two_sum<V: Float>(a: V, b: V) -> (V, V) {
    let sum = a + b;
    let b_part = sum - a;
    (sum, (a - (sum - b_part)) + (b - b_part))
}

#[inline(always)]
pub(super) fn fast_two_sum<V: Float>(a: V, b: V) -> (V, V) {
    let sum = a + b;
    (sum, b - (sum - a))
}

#[inline(always)]
fn split<V: Float>(a: V) -> (V, V) {
    let scaled = a * constant(Elem::<V>::SPLITTER);
    let high = scaled - (scaled - a);
    (high, a - high)
}

#[inline(always)]
pub(super) fn two_product<V: Float>(a: V, b: V) -> (V, V) {
    let product = a * b;
    if <V::Simd as Simd>::AVX2 {
        return (product, a.mul_sub(b, product));
    }
    let (a_high, a_low) = split(a);
    let (b_high, b_low) = split(b);
    let error = ((a_high * b_high - product) + a_high * b_low + a_low * b_high) + a_low * b_low;
    (product, error)
}

#[inline(always)]
pub(super) fn negative_sign<V: Float>(value: V) -> V::Mask {
    constant::<V>(Elem::<V>::ONE)
        .copysign(value)
        .simd_lt(V::default())
}

#[inline(always)]
pub(super) fn when<V: Float>(mask: V::Mask, if_true: V, if_false: V) -> V {
    mask.select(if_true, if_false)
}
