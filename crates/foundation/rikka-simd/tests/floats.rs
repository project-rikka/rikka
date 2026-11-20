use std::ops::{Add, Div, Mul, Neg, Sub};

use rikka_simd::prelude::*;

use crate::support::{Sample, WINDOWS, assert_lanes, each_level, lanes, window};

pub(crate) trait FloatRef:
    Sample
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    type Int: Sample;
    type Bits: Sample;
    const BLOCK: usize;
    const ONE: Self;
    const ESTIMATE_ERROR: Self;

    fn sqrt(self) -> Self;
    fn floor(self) -> Self;
    fn ceil(self) -> Self;
    fn round(self) -> Self;
    fn trunc(self) -> Self;
    fn abs(self) -> Self;
    fn copysign(self, sign: Self) -> Self;
    fn fused(self, b: Self, c: Self) -> Self;
    fn is_nan(self) -> bool;
    fn is_finite(self) -> bool;
    fn is_normal(self) -> bool;
    fn bits(self) -> Self::Bits;
    fn int_trunc(self) -> Self::Int;
    fn int_round(self) -> Self::Int;
    fn from_int(value: Self::Int) -> Self;
    fn index(bits: Self::Bits, len: usize) -> (Self::Bits, usize);
}

macro_rules! float_ref {
    ($element:ty, $int:ty, $bits:ty, $block:expr) => {
        impl FloatRef for $element {
            type Int = $int;
            type Bits = $bits;
            const BLOCK: usize = $block;
            const ONE: Self = 1.0;
            const ESTIMATE_ERROR: Self = 1.0 / 2048.0;

            fn sqrt(self) -> Self {
                <$element>::sqrt(self)
            }

            fn floor(self) -> Self {
                <$element>::floor(self)
            }

            fn ceil(self) -> Self {
                <$element>::ceil(self)
            }

            fn round(self) -> Self {
                self.round_ties_even()
            }

            fn trunc(self) -> Self {
                <$element>::trunc(self)
            }

            fn abs(self) -> Self {
                <$element>::abs(self)
            }

            fn copysign(self, sign: Self) -> Self {
                <$element>::copysign(self, sign)
            }

            fn fused(self, b: Self, c: Self) -> Self {
                self.mul_add(b, c)
            }

            fn is_nan(self) -> bool {
                <$element>::is_nan(self)
            }

            fn is_finite(self) -> bool {
                <$element>::is_finite(self)
            }

            fn is_normal(self) -> bool {
                <$element>::is_normal(self)
            }

            fn bits(self) -> $bits {
                self.to_bits()
            }

            fn int_trunc(self) -> $int {
                let limit = -(<$int>::MIN as $element);
                if self.is_nan() || self >= limit || self < -limit {
                    <$int>::MIN
                } else {
                    self as $int
                }
            }

            fn int_round(self) -> $int {
                self.round_ties_even().int_trunc()
            }

            fn from_int(value: $int) -> Self {
                value as $element
            }

            fn index(bits: $bits, len: usize) -> ($bits, usize) {
                let index = bits as usize % len;
                (index as $bits, index)
            }
        }
    };
}

float_ref!(f32, i32, u32, 4);
float_ref!(f64, i64, u64, 2);

fn x86_min<E: FloatRef>(a: E, b: E) -> E {
    if a < b { a } else { b }
}

fn x86_max<E: FloatRef>(a: E, b: E) -> E {
    if a > b { a } else { b }
}

fn halving<E: Copy>(values: &[E], op: &impl Fn(E, E) -> E) -> E {
    if values.len() == 1 {
        return values[0];
    }
    let half = values.len() / 2;
    let folded: Vec<E> = (0..half)
        .map(|lane| op(values[lane], values[lane + half]))
        .collect();
    halving(&folded, op)
}

fn prefix<E: FloatRef>(values: &[E]) -> Vec<E> {
    if values.len() <= E::BLOCK {
        let mut sums = values.to_vec();
        let mut distance = 1;
        while distance < values.len() {
            let previous = sums.clone();
            for lane in 0..values.len() {
                let shifted = lane
                    .checked_sub(distance)
                    .map_or_else(E::default, |source| previous[source]);
                sums[lane] = previous[lane] + shifted;
            }
            distance *= 2;
        }
        return sums;
    }
    let half = values.len() / 2;
    let mut sums = prefix(&values[..half]);
    let carry = sums[half - 1];
    sums.extend(
        prefix(&values[half..])
            .into_iter()
            .map(|value| value + carry),
    );
    sums
}

fn bits_of<E: Copy>(values: &[E], test: impl Fn(E) -> bool) -> u64 {
    values.iter().enumerate().fold(0, |bits, (lane, &value)| {
        bits | u64::from(test(value)) << lane
    })
}

fn check_estimate<E: FloatRef>(context: &str, inputs: &[E], got: &[E], exact: impl Fn(E) -> E) {
    for (lane, (&input, &estimate)) in inputs.iter().zip(got).enumerate() {
        let expected = exact(input);
        if !input.is_normal() || !expected.is_normal() {
            continue;
        }
        let error = ((estimate - expected) / expected).abs();
        assert!(
            error <= E::ESTIMATE_ERROR,
            "{context}: lane {lane} estimates {estimate:?} for {input:?}, exactly {expected:?}"
        );
    }
}

fn check<V>(isa: Isa, name: &str)
where
    V: Float + Gather<Index = V::Bits> + Compress,
    V::Elem: FloatRef<Int = <V::Int as Vector>::Elem, Bits = <V::Bits as Vector>::Elem>,
    <V::Int as Vector>::Elem: Sample,
    <V::Bits as Vector>::Elem: Sample,
{
    let samples = V::Elem::samples();
    for start in 0..WINDOWS {
        let a: V = window(&samples, start);
        let b: V = window(&samples, start * 5 + 11);
        let c: V = window(&samples, start * 7 + 3);
        let (la, lb, lc) = (lanes(a), lanes(b), lanes(c));
        let context = |op: &str| format!("{isa} {name} {op} window {start}");
        let unary = |f: &dyn Fn(V::Elem) -> V::Elem| la.iter().map(|&x| f(x)).collect::<Vec<_>>();
        let binary = |f: &dyn Fn(V::Elem, V::Elem) -> V::Elem| {
            la.iter()
                .zip(&lb)
                .map(|(&x, &y)| f(x, y))
                .collect::<Vec<_>>()
        };

        assert_lanes(&context("add"), &lanes(a + b), &binary(&|x, y| x + y));
        assert_lanes(&context("sub"), &lanes(a - b), &binary(&|x, y| x - y));
        assert_lanes(&context("mul"), &lanes(a * b), &binary(&|x, y| x * y));
        assert_lanes(&context("div"), &lanes(a / b), &binary(&|x, y| x / y));
        assert_lanes(&context("neg"), &lanes(-a), &unary(&|x| -x));
        assert_lanes(&context("min"), &lanes(a.min(b)), &binary(&x86_min));
        assert_lanes(&context("max"), &lanes(a.max(b)), &binary(&x86_max));
        assert_lanes(&context("abs"), &lanes(a.abs()), &unary(&FloatRef::abs));
        assert_lanes(
            &context("copysign"),
            &lanes(a.copysign(b)),
            &binary(&FloatRef::copysign),
        );
        assert_lanes(&context("sqrt"), &lanes(a.sqrt()), &unary(&FloatRef::sqrt));
        assert_lanes(
            &context("floor"),
            &lanes(a.floor()),
            &unary(&FloatRef::floor),
        );
        assert_lanes(&context("ceil"), &lanes(a.ceil()), &unary(&FloatRef::ceil));
        assert_lanes(
            &context("round"),
            &lanes(a.round()),
            &unary(&FloatRef::round),
        );
        assert_lanes(
            &context("trunc"),
            &lanes(a.trunc()),
            &unary(&FloatRef::trunc),
        );

        let fused = isa >= Isa::Avx2;
        let products = |sign: bool| -> Vec<V::Elem> {
            (0..V::LANES)
                .map(|lane| {
                    let addend = if sign { -lc[lane] } else { lc[lane] };
                    if fused {
                        la[lane].fused(lb[lane], addend)
                    } else {
                        la[lane] * lb[lane] + addend
                    }
                })
                .collect()
        };
        assert_lanes(
            &context("mul_add"),
            &lanes(a.mul_add(b, c)),
            &products(false),
        );
        assert_lanes(
            &context("mul_sub"),
            &lanes(a.mul_sub(b, c)),
            &products(true),
        );

        check_estimate(
            &context("recip_estimate"),
            &la,
            &lanes(a.recip_estimate()),
            |x| V::Elem::ONE / x,
        );
        check_estimate(
            &context("rsqrt_estimate"),
            &la,
            &lanes(a.rsqrt_estimate()),
            |x| V::Elem::ONE / x.sqrt(),
        );

        let compare = |got: V::Mask, test: &dyn Fn(V::Elem, V::Elem) -> bool, op: &str| {
            let expected = (0..V::LANES).fold(0, |bits, lane| {
                bits | u64::from(test(la[lane], lb[lane])) << lane
            });
            assert_eq!(got.to_bitmask(), expected, "{}", context(op));
        };
        compare(a.simd_lt(b), &|x, y| x < y, "lt");
        compare(a.simd_le(b), &|x, y| x <= y, "le");
        compare(a.simd_gt(b), &|x, y| x > y, "gt");
        compare(a.simd_ge(b), &|x, y| x >= y, "ge");
        compare(a.simd_eq(b), &|x, y| x == y, "eq");
        compare(a.simd_ne(b), &|x, y| x != y, "ne");
        assert_eq!(
            a.is_nan().to_bitmask(),
            bits_of(&la, FloatRef::is_nan),
            "{}",
            context("is_nan")
        );
        assert_eq!(
            a.is_finite().to_bitmask(),
            bits_of(&la, FloatRef::is_finite),
            "{}",
            context("is_finite")
        );

        let mask = a.simd_lt(b);
        let selected: Vec<_> = (0..V::LANES)
            .map(|lane| {
                if la[lane] < lb[lane] {
                    la[lane]
                } else {
                    lc[lane]
                }
            })
            .collect();
        assert_lanes(&context("select"), &lanes(V::select(mask, a, c)), &selected);

        let finite: Vec<_> = la
            .iter()
            .map(|&x| if x.is_finite() { x } else { V::Elem::ONE })
            .collect();
        let f = V::load(&finite);
        assert!(
            V::Elem::same(f.reduce_add(), halving(&finite, &|x, y| x + y)),
            "{}",
            context("reduce_add")
        );
        assert!(
            V::Elem::same(a.reduce_min(), halving(&la, &x86_min)),
            "{}",
            context("reduce_min")
        );
        assert!(
            V::Elem::same(a.reduce_max(), halving(&la, &x86_max)),
            "{}",
            context("reduce_max")
        );
        assert_lanes(
            &context("prefix_sum"),
            &lanes(f.prefix_sum()),
            &prefix(&finite),
        );

        let truncated: Vec<_> = la.iter().map(|&x| x.int_trunc()).collect();
        assert_lanes(
            &context("to_int_trunc"),
            &lanes(a.to_int_trunc()),
            &truncated,
        );
        let rounded: Vec<_> = la.iter().map(|&x| x.int_round()).collect();
        assert_lanes(&context("to_int_round"), &lanes(a.to_int_round()), &rounded);
        let converted: Vec<_> = truncated.iter().map(|&i| V::Elem::from_int(i)).collect();
        assert_lanes(
            &context("from_int"),
            &lanes(V::from_int(a.to_int_trunc())),
            &converted,
        );
        let bits: Vec<_> = la.iter().map(|&x| x.bits()).collect();
        assert_lanes(&context("to_bits"), &lanes(a.to_bits()), &bits);
        assert_lanes(
            &context("from_bits"),
            &lanes(V::from_bits(a.to_bits())),
            &la,
        );

        let (indices, positions): (Vec<_>, Vec<_>) = lanes(b.to_bits())
            .into_iter()
            .map(|bits| V::Elem::index(bits, samples.len()))
            .unzip();
        let gathered: Vec<_> = positions.iter().map(|&index| samples[index]).collect();
        assert_lanes(
            &context("gather"),
            &lanes(V::gather(&samples, V::Bits::load(&indices))),
            &gathered,
        );

        let mut packed: Vec<_> = (0..V::LANES)
            .filter(|&lane| la[lane] < lb[lane])
            .map(|lane| la[lane])
            .collect();
        packed.resize(V::LANES, V::Elem::default());
        assert_lanes(&context("compress"), &lanes(a.compress(mask)), &packed);

        let rows: Vec<[V::Elem; 4]> = (0..V::LANES)
            .map(|row| {
                std::array::from_fn(|column| samples[(start + row * 4 + column) % samples.len()])
            })
            .collect();
        let sources: Vec<&[V::Elem; 4]> = rows.iter().collect();
        let columns = V::load_transposed(&sources);
        for (column, vector) in columns.iter().enumerate() {
            let expected: Vec<_> = rows.iter().map(|row| row[column]).collect();
            assert_lanes(&context("load_transposed"), &lanes(*vector), &expected);
        }
        let mut written = vec![[V::Elem::default(); 4]; V::LANES];
        let mut targets: Vec<&mut [V::Elem; 4]> = written.iter_mut().collect();
        V::store_transposed(columns, &mut targets);
        for (row, expected) in written.iter().zip(&rows) {
            assert_lanes(&context("store_transposed"), row, expected);
        }
    }
}

fn check_widen<V>(isa: Isa, name: &str)
where
    V: Float<Elem = f32> + Widen,
    V::Wide: Narrow<Narrow = V> + Vector<Elem = f64>,
{
    let samples = f32::samples();
    for start in 0..WINDOWS {
        let a: V = window(&samples, start);
        let la = lanes(a);
        let half = V::LANES / 2;
        let wide = |values: &[f32]| values.iter().map(|&x| f64::from(x)).collect::<Vec<_>>();
        let context = |op: &str| format!("{isa} {name} {op} window {start}");
        assert_lanes(
            &context("widen_low"),
            &lanes(a.widen_low()),
            &wide(&la[..half]),
        );
        assert_lanes(
            &context("widen_high"),
            &lanes(a.widen_high()),
            &wide(&la[half..]),
        );
        let narrowed = <V::Wide as Narrow>::narrow(a.widen_low(), a.widen_high());
        assert_lanes(&context("narrow"), &lanes(narrowed), &la);
    }
}

#[inline(always)]
fn floats<S: Simd>(_: S) {
    let isa = S::ISA;
    check::<S::F32x1>(isa, "F32x1");
    check::<S::F32x4>(isa, "F32x4");
    check::<S::F32x8>(isa, "F32x8");
    check::<S::F32x16>(isa, "F32x16");
    check::<S::F32s>(isa, "F32s");
    check::<S::F64x1>(isa, "F64x1");
    check::<S::F64x2>(isa, "F64x2");
    check::<S::F64x4>(isa, "F64x4");
    check::<S::F64x8>(isa, "F64x8");
    check::<S::F64s>(isa, "F64s");
    check_widen::<S::F32x4>(isa, "F32x4");
    check_widen::<S::F32x8>(isa, "F32x8");
    check_widen::<S::F32x16>(isa, "F32x16");
}

#[test]
fn float_lanes_match_scalar() {
    each_level!(floats);
}
