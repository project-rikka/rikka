use super::constants::MathElement;
use super::kernel::{
    Elem, bits, constant, fast_two_sum, poly, small_integer, two_product, two_sum, when,
};
use crate::Float;

#[inline(always)]
fn normalize<V: Float>(x: V) -> (V, V) {
    let one = constant::<V>(Elem::<V>::ONE);
    let subnormal = x.simd_lt(constant(Elem::<V>::MIN_POSITIVE));
    let scaled = when(subnormal, x * constant(Elem::<V>::SUBNORMAL_SCALE), x);
    let raw = scaled.to_bits();
    let exponent_bits = (raw >> Elem::<V>::MANTISSA) & bits::<V>(Elem::<V>::EXPONENT_MASK);
    let mantissa =
        V::from_bits((raw & bits::<V>(Elem::<V>::MANTISSA_MASK)) | bits::<V>(Elem::<V>::ONE_BITS));
    let large = mantissa.simd_gt(constant(Elem::<V>::SQRT_2));
    let mantissa = when(large, mantissa * constant(Elem::<V>::HALF), mantissa);
    let bias = small_integer::<V>(bits::<V>(Elem::<V>::BIAS));
    let shift = small_integer::<V>(bits::<V>(Elem::<V>::SUBNORMAL_SHIFT));
    let exponent = small_integer::<V>(exponent_bits) - bias + when(large, one, V::default())
        - when(subnormal, shift, V::default());
    (exponent, mantissa)
}

#[inline(always)]
fn log_parts<V: Float>(x: V) -> (V, V) {
    let (exponent, mantissa) = normalize(x);
    let fraction = mantissa - constant(Elem::<V>::ONE);
    let ratio = fraction / (fraction + constant(Elem::<V>::TWO));
    let square = ratio * ratio;
    let part = (ratio * square).mul_add(poly(square, Elem::<V>::LN_SERIES), ratio + ratio);
    (exponent, part)
}

#[inline(always)]
fn with_specials<V: Float>(x: V, result: V) -> V {
    let infinity = constant::<V>(Elem::<V>::INFINITY);
    let result = when(x.simd_eq(V::default()), -infinity, result);
    let result = when(
        x.simd_lt(V::default()) | x.is_nan(),
        constant(Elem::<V>::NAN),
        result,
    );
    when(x.simd_eq(infinity), infinity, result)
}

#[inline(always)]
pub(super) fn ln<V: Float>(x: V) -> V {
    let (exponent, part) = log_parts(x);
    let low = exponent.mul_add(constant(Elem::<V>::LN2_LOW), part);
    with_specials(x, exponent.mul_add(constant(Elem::<V>::LN2_HIGH), low))
}

#[inline(always)]
pub(super) fn log2<V: Float>(x: V) -> V {
    let (exponent, part) = log_parts(x);
    with_specials(x, part.mul_add(constant(Elem::<V>::LOG2_E), exponent))
}

#[inline(always)]
pub(super) fn log10<V: Float>(x: V) -> V {
    let (exponent, part) = log_parts(x);
    let scaled = part * constant(Elem::<V>::LOG10_E);
    with_specials(x, exponent.mul_add(constant(Elem::<V>::LOG10_2), scaled))
}

#[inline(always)]
pub(super) fn log2_split<V: Float>(x: V) -> (V, V) {
    let one = constant::<V>(Elem::<V>::ONE);
    let (exponent, mantissa) = normalize(x);
    let numerator = mantissa - one;
    let (denominator, denominator_low) = two_sum(mantissa, one);
    let ratio = numerator / denominator;
    let (product, product_low) = two_product(ratio, denominator);
    let residual = ((numerator - product) - product_low) - ratio * denominator_low;
    let ratio_low = residual / denominator;
    let square = ratio * ratio;
    let tail = (ratio * square) * poly(square, Elem::<V>::LN_SERIES);
    let (high, low) = fast_two_sum(ratio + ratio, (ratio_low + ratio_low) + tail);
    let log2_e = constant::<V>(Elem::<V>::LOG2_E_HIGH);
    let (scaled, scaled_low) = two_product(high, log2_e);
    let scaled_low = scaled_low + high.mul_add(constant(Elem::<V>::LOG2_E_LOW), low * log2_e);
    let (sum, sum_low) = two_sum(exponent, scaled);
    fast_two_sum(sum, sum_low + scaled_low)
}
