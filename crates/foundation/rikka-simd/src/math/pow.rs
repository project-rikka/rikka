use super::constants::MathElement;
use super::exp::exp2_split;
use super::kernel::{Elem, constant, negative_sign, two_product, when};
use super::log::log2_split;
use crate::Float;

#[inline(always)]
pub(super) fn pow<V: Float>(x: V, y: V) -> V {
    let zero = V::default();
    let one = constant::<V>(Elem::<V>::ONE);
    let infinity = constant::<V>(Elem::<V>::INFINITY);
    let nan = constant::<V>(Elem::<V>::NAN);
    let magnitude = x.abs();

    let (log_high, log_low) = log2_split(magnitude);
    let (product, product_low) = two_product(y, log_high);
    let product_low = y.mul_add(log_low, product_low);
    let bounded = product.abs().simd_lt(constant(Elem::<V>::EXP2_HIGH)) & product_low.is_finite();
    let result = exp2_split(product, when(bounded, product_low, zero));

    let integral = y.trunc().simd_eq(y);
    let halved = y * constant(Elem::<V>::HALF);
    let odd = integral & halved.trunc().simd_ne(halved);
    let negative = x.simd_lt(zero);
    let result = when(negative & !integral, nan, result);
    let result = when(negative & odd, -result, result);

    let sign = when(odd, x, one);
    let towards_zero = y.simd_lt(zero);
    let at_zero = when(towards_zero, infinity, zero).copysign(sign);
    let result = when(x.simd_eq(zero), at_zero, result);
    let at_infinity = when(towards_zero, zero, infinity);
    let at_infinity = when(negative_sign(x) & odd, -at_infinity, at_infinity);
    let result = when(magnitude.simd_eq(infinity), at_infinity, result);
    let result = when(
        magnitude.simd_eq(one) & y.abs().simd_eq(infinity),
        one,
        result,
    );
    let result = when(x.is_nan() | y.is_nan(), nan, result);
    when(y.simd_eq(zero) | x.simd_eq(one), one, result)
}
