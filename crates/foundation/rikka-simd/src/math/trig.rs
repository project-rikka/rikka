use super::constants::MathElement;
use super::kernel::{Elem, bits, constant, integer_bits, poly, when};
use crate::{Float, Vector};

struct Reduced<V: Float> {
    sin: V,
    cos: V,
    swapped: V::Mask,
    quadrant: V::Bits,
}

#[inline(always)]
fn reduce<V: Float>(x: V) -> Reduced<V> {
    let quarter_turns = (x * constant(Elem::<V>::TWO_OVER_PI)).round();
    let mut reduced = x;
    for part in Elem::<V>::PI_2_PARTS {
        reduced = quarter_turns.neg_mul_add(constant(part), reduced);
    }
    let square = reduced * reduced;
    let sin = (reduced * square).mul_add(poly(square, Elem::<V>::SIN_SERIES), reduced);
    let cos = square.mul_add(
        poly(square, Elem::<V>::COS_SERIES),
        constant(Elem::<V>::ONE),
    );
    let quadrant = integer_bits(quarter_turns);
    let first_bit = bits::<V>(1);
    Reduced {
        sin,
        cos,
        swapped: (quadrant & first_bit).simd_eq(first_bit),
        quadrant,
    }
}

#[inline(always)]
fn negated_when<V: Float>(quadrant: V::Bits, value: V) -> V {
    let second_bit = bits::<V>(2);
    when((quadrant & second_bit).simd_eq(second_bit), -value, value)
}

#[inline(always)]
fn keep_zero<V: Float>(x: V, result: V) -> V {
    when(x.simd_eq(V::default()), x, result)
}

#[inline(always)]
pub(super) fn sin<V: Float>(x: V) -> V {
    let reduced = reduce(x);
    let value = when(reduced.swapped, reduced.cos, reduced.sin);
    keep_zero(x, negated_when(reduced.quadrant, value))
}

#[inline(always)]
pub(super) fn cos<V: Float>(x: V) -> V {
    let reduced = reduce(x);
    let value = when(reduced.swapped, reduced.sin, reduced.cos);
    negated_when(reduced.quadrant + bits::<V>(1), value)
}

#[inline(always)]
pub(super) fn sin_cos<V: Float>(x: V) -> (V, V) {
    let reduced = reduce(x);
    let sin = when(reduced.swapped, reduced.cos, reduced.sin);
    let cos = when(reduced.swapped, reduced.sin, reduced.cos);
    (
        keep_zero(x, negated_when(reduced.quadrant, sin)),
        negated_when(reduced.quadrant + bits::<V>(1), cos),
    )
}

#[inline(always)]
pub(super) fn tan<V: Float>(x: V) -> V {
    let reduced = reduce(x);
    let numerator = when(reduced.swapped, -reduced.cos, reduced.sin);
    let denominator = when(reduced.swapped, reduced.sin, reduced.cos);
    keep_zero(x, numerator / denominator)
}
