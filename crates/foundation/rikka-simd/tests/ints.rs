use rikka_simd::prelude::*;

use crate::support::{Sample, WINDOWS, assert_lanes, each_level, lanes, window};

pub(crate) trait IntRef: Sample + Ord {
    const BITS: u32;

    fn add(self, other: Self) -> Self;
    fn sub(self, other: Self) -> Self;
    fn mul(self, other: Self) -> Self;
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;
    fn xor(self, other: Self) -> Self;
    fn not(self) -> Self;
    fn shift_left(self, count: u32) -> Self;
    fn shift_right(self, count: u32) -> Self;
    fn as_count(self) -> u32;
    fn from_count(count: u32) -> Self;
    fn saturating_add(self, other: Self) -> Self;
    fn saturating_sub(self, other: Self) -> Self;
    fn count_ones(self) -> Self;
}

pub(crate) trait SignedRef: IntRef {
    fn neg(self) -> Self;
    fn abs(self) -> Self;
}

macro_rules! int_ref {
    ($($element:ty => $unsigned:ty),*) => {
        $(
            impl IntRef for $element {
                const BITS: u32 = <$element>::BITS;

                fn add(self, other: Self) -> Self {
                    self.wrapping_add(other)
                }

                fn sub(self, other: Self) -> Self {
                    self.wrapping_sub(other)
                }

                fn mul(self, other: Self) -> Self {
                    self.wrapping_mul(other)
                }

                fn and(self, other: Self) -> Self {
                    self & other
                }

                fn or(self, other: Self) -> Self {
                    self | other
                }

                fn xor(self, other: Self) -> Self {
                    self ^ other
                }

                fn not(self) -> Self {
                    !self
                }

                fn shift_left(self, count: u32) -> Self {
                    self.checked_shl(count).unwrap_or(0)
                }

                fn shift_right(self, count: u32) -> Self {
                    if <$element>::MIN == 0 {
                        self.checked_shr(count).unwrap_or(0)
                    } else {
                        self >> count.min(Self::BITS - 1)
                    }
                }

                fn as_count(self) -> u32 {
                    u32::try_from(self as $unsigned).unwrap_or(u32::MAX)
                }

                fn from_count(count: u32) -> Self {
                    count as $element
                }

                fn saturating_add(self, other: Self) -> Self {
                    <$element>::saturating_add(self, other)
                }

                fn saturating_sub(self, other: Self) -> Self {
                    <$element>::saturating_sub(self, other)
                }

                fn count_ones(self) -> Self {
                    <$element>::count_ones(self) as $element
                }
            }
        )*
    };
}

int_ref!(i8 => u8, u8 => u8, i16 => u16, u16 => u16, i32 => u32, u32 => u32, i64 => u64, u64 => u64);

macro_rules! signed_ref {
    ($($element:ty),*) => {
        $(
            impl SignedRef for $element {
                fn neg(self) -> Self {
                    self.wrapping_neg()
                }

                fn abs(self) -> Self {
                    self.wrapping_abs()
                }
            }
        )*
    };
}

signed_ref!(i8, i16, i32, i64);

fn bits_from(test: impl Fn(usize) -> bool, lanes: usize) -> u64 {
    (0..lanes).fold(0, |bits, lane| bits | u64::from(test(lane)) << lane)
}

fn check<V: Int>(isa: Isa, name: &str)
where
    V::Elem: IntRef,
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

        assert_lanes(&context("add"), &lanes(a + b), &binary(&IntRef::add));
        assert_lanes(&context("sub"), &lanes(a - b), &binary(&IntRef::sub));
        assert_lanes(&context("mul"), &lanes(a * b), &binary(&IntRef::mul));
        assert_lanes(&context("and"), &lanes(a & b), &binary(&IntRef::and));
        assert_lanes(&context("or"), &lanes(a | b), &binary(&IntRef::or));
        assert_lanes(&context("xor"), &lanes(a ^ b), &binary(&IntRef::xor));
        assert_lanes(&context("not"), &lanes(!a), &unary(&IntRef::not));
        assert_lanes(&context("min"), &lanes(a.min(b)), &binary(&Ord::min));
        assert_lanes(&context("max"), &lanes(a.max(b)), &binary(&Ord::max));
        assert_lanes(
            &context("saturating_add"),
            &lanes(a.saturating_add(b)),
            &binary(&IntRef::saturating_add),
        );
        assert_lanes(
            &context("saturating_sub"),
            &lanes(a.saturating_sub(b)),
            &binary(&IntRef::saturating_sub),
        );
        assert_lanes(
            &context("count_ones"),
            &lanes(a.count_ones()),
            &unary(&IntRef::count_ones),
        );

        let bits = V::Elem::BITS;
        for count in [0, 1, 3, bits / 2, bits - 1, bits, bits + 5, 1000] {
            assert_lanes(
                &context(&format!("shl {count}")),
                &lanes(a << count),
                &unary(&|x| x.shift_left(count)),
            );
            assert_lanes(
                &context(&format!("shr {count}")),
                &lanes(a >> count),
                &unary(&|x| x.shift_right(count)),
            );
        }
        let counts: Vec<_> = lb
            .iter()
            .map(|&x| V::Elem::from_count(x.as_count() % (bits + 4)))
            .collect();
        let shifts = V::load(&counts);
        let by_lane = |f: &dyn Fn(V::Elem, u32) -> V::Elem| {
            la.iter()
                .zip(&counts)
                .map(|(&x, &n)| f(x, n.as_count()))
                .collect::<Vec<_>>()
        };
        assert_lanes(
            &context("shl each"),
            &lanes(a << shifts),
            &by_lane(&IntRef::shift_left),
        );
        assert_lanes(
            &context("shr each"),
            &lanes(a >> shifts),
            &by_lane(&IntRef::shift_right),
        );
        assert_lanes(
            &context("shl wide"),
            &lanes(a << b),
            &binary(&|x, y| x.shift_left(y.as_count())),
        );
        assert_lanes(
            &context("shr wide"),
            &lanes(a >> b),
            &binary(&|x, y| x.shift_right(y.as_count())),
        );

        let compare = |got: V::Mask, test: &dyn Fn(V::Elem, V::Elem) -> bool, op: &str| {
            let expected = bits_from(|lane| test(la[lane], lb[lane]), V::LANES);
            assert_eq!(got.to_bitmask(), expected, "{}", context(op));
        };
        compare(a.simd_lt(b), &|x, y| x < y, "lt");
        compare(a.simd_le(b), &|x, y| x <= y, "le");
        compare(a.simd_gt(b), &|x, y| x > y, "gt");
        compare(a.simd_ge(b), &|x, y| x >= y, "ge");
        compare(a.simd_eq(b), &|x, y| x == y, "eq");
        compare(a.simd_ne(b), &|x, y| x != y, "ne");
        compare(a.simd_eq(a), &|_, _| true, "eq self");

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
        assert_lanes(
            &context("mask select"),
            &lanes(mask.select(a, c)),
            &selected,
        );

        let total = la.iter().fold(V::Elem::default(), |sum, &x| sum.add(x));
        assert_eq!(a.reduce_add(), total, "{}", context("reduce_add"));
        assert_eq!(
            a.reduce_min(),
            *la.iter().min().unwrap(),
            "{}",
            context("reduce_min")
        );
        assert_eq!(
            a.reduce_max(),
            *la.iter().max().unwrap(),
            "{}",
            context("reduce_max")
        );
        let running: Vec<_> = la
            .iter()
            .scan(V::Elem::default(), |sum, &x| {
                *sum = sum.add(x);
                Some(*sum)
            })
            .collect();
        assert_lanes(&context("prefix_sum"), &lanes(a.prefix_sum()), &running);

        let reversed: Vec<_> = la.iter().rev().copied().collect();
        assert_lanes(&context("reverse"), &lanes(a.reverse()), &reversed);
        let zipped: Vec<_> = la.iter().zip(&lb).flat_map(|(&x, &y)| [x, y]).collect();
        let (low, high) = a.interleave(b);
        assert_lanes(&context("interleave low"), &lanes(low), &zipped[..V::LANES]);
        assert_lanes(
            &context("interleave high"),
            &lanes(high),
            &zipped[V::LANES..],
        );
        let joined: Vec<_> = la.iter().chain(&lb).copied().collect();
        let evens: Vec<_> = joined.iter().step_by(2).copied().collect();
        let odds: Vec<_> = joined.iter().skip(1).step_by(2).copied().collect();
        let (even, odd) = a.deinterleave(b);
        assert_lanes(&context("deinterleave even"), &lanes(even), &evens);
        assert_lanes(&context("deinterleave odd"), &lanes(odd), &odds);

        for len in [
            0,
            1,
            V::LANES / 2,
            V::LANES.saturating_sub(1),
            V::LANES,
            V::LANES + 3,
        ] {
            let source: Vec<_> = la.iter().cycle().take(len).copied().collect();
            let mut expected = source.clone();
            expected.resize(V::LANES, V::Elem::default());
            expected.truncate(V::LANES);
            assert_lanes(
                &context(&format!("load_partial {len}")),
                &lanes(V::load_partial(&source)),
                &expected,
            );
            let mut target = lc.clone();
            target.resize(len.max(V::LANES), V::Elem::default());
            target.truncate(len);
            let mut expected = target.clone();
            let copied = len.min(V::LANES);
            expected[..copied].copy_from_slice(&la[..copied]);
            a.store_partial(&mut target);
            assert_lanes(
                &context(&format!("store_partial {len}")),
                &target,
                &expected,
            );
        }

        let bitmask = mask.to_bitmask();
        assert_eq!(
            V::Mask::from_bitmask(bitmask),
            mask,
            "{}",
            context("from_bitmask")
        );
        assert_eq!(mask.any(), bitmask != 0, "{}", context("any"));
        assert_eq!(
            mask.all(),
            bitmask == bits_from(|_| true, V::LANES),
            "{}",
            context("all")
        );
        assert_eq!(mask.count(), bitmask.count_ones(), "{}", context("count"));
        assert_eq!(
            mask.first(),
            (bitmask != 0).then(|| bitmask.trailing_zeros() as usize),
            "{}",
            context("first")
        );
        assert_eq!(
            (!mask).to_bitmask(),
            !bitmask & bits_from(|_| true, V::LANES),
            "{}",
            context("mask not")
        );
        let other = a.simd_le(c);
        assert_eq!(
            (mask & other).to_bitmask(),
            bitmask & other.to_bitmask(),
            "{}",
            context("mask and")
        );
        assert_eq!(
            (mask | other).to_bitmask(),
            bitmask | other.to_bitmask(),
            "{}",
            context("mask or")
        );
        assert_eq!(
            (mask ^ other).to_bitmask(),
            bitmask ^ other.to_bitmask(),
            "{}",
            context("mask xor")
        );
        assert_eq!(
            V::Mask::splat(true).count() as usize,
            V::LANES,
            "{}",
            context("mask splat")
        );
    }
}

fn check_signed<V: SignedInt>(isa: Isa, name: &str)
where
    V::Elem: SignedRef,
{
    let samples = V::Elem::samples();
    for start in 0..WINDOWS {
        let a: V = window(&samples, start);
        let la = lanes(a);
        let context = |op: &str| format!("{isa} {name} {op} window {start}");
        let neg: Vec<_> = la.iter().map(|&x| x.neg()).collect();
        let abs: Vec<_> = la.iter().map(|&x| x.abs()).collect();
        assert_lanes(&context("neg"), &lanes(-a), &neg);
        assert_lanes(&context("abs"), &lanes(a.abs()), &abs);
    }
}

fn check_widen<V: Widen>(isa: Isa, name: &str, widen: impl Fn(V::Elem) -> <V::Wide as Vector>::Elem)
where
    V::Elem: Sample,
    <V::Wide as Vector>::Elem: Sample,
{
    let samples = V::Elem::samples();
    for start in 0..WINDOWS {
        let a: V = window(&samples, start);
        let la = lanes(a);
        let half = V::LANES / 2;
        let wide = |values: &[V::Elem]| values.iter().map(|&x| widen(x)).collect::<Vec<_>>();
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
    }
}

fn check_narrow<V: Vector, N: Vector>(
    isa: Isa,
    name: &str,
    narrow: impl Fn(V, V) -> N,
    reference: impl Fn(V::Elem) -> N::Elem,
) where
    V::Elem: Sample,
    N::Elem: Sample,
{
    let samples = V::Elem::samples();
    for start in 0..WINDOWS {
        let a: V = window(&samples, start);
        let b: V = window(&samples, start * 3 + 1);
        let expected: Vec<_> = lanes(a)
            .into_iter()
            .chain(lanes(b))
            .map(&reference)
            .collect();
        assert_lanes(
            &format!("{isa} {name} window {start}"),
            &lanes(narrow(a, b)),
            &expected,
        );
    }
}

fn check_gather<V: Gather + Compress>(
    isa: Isa,
    name: &str,
    index: impl Fn(<V::Index as Vector>::Elem, usize) -> (<V::Index as Vector>::Elem, usize),
) where
    V::Elem: IntRef,
    <V::Index as Vector>::Elem: Sample,
{
    let samples = V::Elem::samples();
    let positions_source = <V::Index as Vector>::Elem::samples();
    for start in 0..WINDOWS {
        let a: V = window(&samples, start);
        let b: V = window(&samples, start + 9);
        let raw: V::Index = window(&positions_source, start);
        let (indices, positions): (Vec<_>, Vec<_>) = lanes(raw)
            .into_iter()
            .map(|bits| index(bits, samples.len()))
            .unzip();
        let gathered: Vec<_> = positions
            .iter()
            .map(|&position| samples[position])
            .collect();
        let context = |op: &str| format!("{isa} {name} {op} window {start}");
        assert_lanes(
            &context("gather"),
            &lanes(V::gather(&samples, V::Index::load(&indices))),
            &gathered,
        );
        let (la, lb) = (lanes(a), lanes(b));
        let mask = V::Mask::from_fn(|lane| la[lane] < lb[lane]);
        let mut packed: Vec<_> = (0..V::LANES)
            .filter(|&lane| la[lane] < lb[lane])
            .map(|lane| la[lane])
            .collect();
        packed.resize(V::LANES, V::Elem::default());
        assert_lanes(&context("compress"), &lanes(a.compress(mask)), &packed);
    }
}

#[test]
#[should_panic(expected = "out of bounds")]
fn gather_past_slice_panics() {
    rikka_simd::dispatch!(rikka_simd::Level::detect(), simd => gather_out_of_range(simd));
}

#[inline(always)]
fn gather_out_of_range<S: Simd>(_: S) {
    let values = [1u32, 2, 3];
    let indices = S::U32x4::from_array([0, 1, 2, 3]);
    let _ = S::U32x4::gather(&values, indices);
}

fn check_swizzle<V: Swizzle + Vector<Elem = u8>>(isa: Isa, name: &str) {
    let samples = u8::samples();
    for start in 0..WINDOWS {
        let table: V = window(&samples, start);
        let raw: V = window(&samples, start * 3);
        let lt = lanes(table);
        let indices: Vec<u8> = lanes(raw)
            .into_iter()
            .enumerate()
            .map(|(lane, x)| {
                if lane % 3 == 0 {
                    x
                } else {
                    x % (V::LANES as u8).max(1)
                }
            })
            .collect();
        let expected: Vec<u8> = indices
            .iter()
            .map(|&i| lt.get(usize::from(i)).copied().unwrap_or(0))
            .collect();
        assert_lanes(
            &format!("{isa} {name} swizzle_dyn window {start}"),
            &lanes(table.swizzle_dyn(V::load(&indices))),
            &expected,
        );
    }
}

fn index32(bits: u32, len: usize) -> (u32, usize) {
    let index = bits as usize % len;
    (index as u32, index)
}

fn index64(bits: u64, len: usize) -> (u64, usize) {
    let index = bits as usize % len;
    (index as u64, index)
}

macro_rules! width {
    ($isa:expr, $i8:ty, $u8:ty, $i16:ty, $u16:ty, $i32:ty, $u32:ty, $i64:ty, $u64:ty, $label:literal) => {{
        let isa = $isa;
        check::<$i8>(isa, concat!("I8 ", $label));
        check::<$u8>(isa, concat!("U8 ", $label));
        check::<$i16>(isa, concat!("I16 ", $label));
        check::<$u16>(isa, concat!("U16 ", $label));
        check::<$i32>(isa, concat!("I32 ", $label));
        check::<$u32>(isa, concat!("U32 ", $label));
        check::<$i64>(isa, concat!("I64 ", $label));
        check::<$u64>(isa, concat!("U64 ", $label));
        check_signed::<$i8>(isa, concat!("I8 ", $label));
        check_signed::<$i16>(isa, concat!("I16 ", $label));
        check_signed::<$i32>(isa, concat!("I32 ", $label));
        check_signed::<$i64>(isa, concat!("I64 ", $label));
        check_gather::<$i32>(isa, concat!("I32 ", $label), index32);
        check_gather::<$u32>(isa, concat!("U32 ", $label), index32);
        check_gather::<$i64>(isa, concat!("I64 ", $label), index64);
        check_gather::<$u64>(isa, concat!("U64 ", $label), index64);
    }};
}

macro_rules! conversions {
    ($isa:expr, $i8:ty, $u8:ty, $i16:ty, $u16:ty, $i32:ty, $u32:ty, $i64:ty, $u64:ty, $label:literal) => {{
        let isa = $isa;
        check_widen::<$i8>(isa, concat!("I8 ", $label), i16::from);
        check_widen::<$u8>(isa, concat!("U8 ", $label), u16::from);
        check_widen::<$i16>(isa, concat!("I16 ", $label), i32::from);
        check_widen::<$u16>(isa, concat!("U16 ", $label), u32::from);
        check_widen::<$i32>(isa, concat!("I32 ", $label), i64::from);
        check_widen::<$u32>(isa, concat!("U32 ", $label), u64::from);
        check_narrow::<$i16, $i8>(isa, concat!("I16 narrow ", $label), Narrow::narrow, |x| {
            x.clamp(-128, 127) as i8
        });
        check_narrow::<$u16, $u8>(isa, concat!("U16 narrow ", $label), Narrow::narrow, |x| {
            x.min(255) as u8
        });
        check_narrow::<$i32, $i16>(isa, concat!("I32 narrow ", $label), Narrow::narrow, |x| {
            x.clamp(-32768, 32767) as i16
        });
        check_narrow::<$u32, $u16>(isa, concat!("U32 narrow ", $label), Narrow::narrow, |x| {
            x.min(65535) as u16
        });
        check_narrow::<$i64, $i32>(isa, concat!("I64 narrow ", $label), Narrow::narrow, |x| {
            x.clamp(i32::MIN.into(), i32::MAX.into()) as i32
        });
        check_narrow::<$u64, $u32>(isa, concat!("U64 narrow ", $label), Narrow::narrow, |x| {
            x.min(u32::MAX.into()) as u32
        });
        check_narrow::<$i16, $u8>(
            isa,
            concat!("I16 narrow_unsigned ", $label),
            NarrowUnsigned::narrow_unsigned,
            |x| x.clamp(0, 255) as u8,
        );
        check_narrow::<$i32, $u16>(
            isa,
            concat!("I32 narrow_unsigned ", $label),
            NarrowUnsigned::narrow_unsigned,
            |x| x.clamp(0, 65535) as u16,
        );
        check_narrow::<$i64, $u32>(
            isa,
            concat!("I64 narrow_unsigned ", $label),
            NarrowUnsigned::narrow_unsigned,
            |x| x.clamp(0, u32::MAX.into()) as u32,
        );
        check_swizzle::<$u8>(isa, concat!("U8 ", $label));
    }};
}

#[inline(always)]
fn ints<S: Simd>(_: S) {
    width!(
        S::ISA,
        S::I8x1,
        S::U8x1,
        S::I16x1,
        S::U16x1,
        S::I32x1,
        S::U32x1,
        S::I64x1,
        S::U64x1,
        "x1"
    );
    width!(
        S::ISA,
        S::I8x16,
        S::U8x16,
        S::I16x8,
        S::U16x8,
        S::I32x4,
        S::U32x4,
        S::I64x2,
        S::U64x2,
        "128"
    );
    width!(
        S::ISA,
        S::I8x32,
        S::U8x32,
        S::I16x16,
        S::U16x16,
        S::I32x8,
        S::U32x8,
        S::I64x4,
        S::U64x4,
        "256"
    );
    width!(
        S::ISA,
        S::I8x64,
        S::U8x64,
        S::I16x32,
        S::U16x32,
        S::I32x16,
        S::U32x16,
        S::I64x8,
        S::U64x8,
        "512"
    );
    width!(
        S::ISA,
        S::I8s,
        S::U8s,
        S::I16s,
        S::U16s,
        S::I32s,
        S::U32s,
        S::I64s,
        S::U64s,
        "native"
    );
    conversions!(
        S::ISA,
        S::I8x16,
        S::U8x16,
        S::I16x8,
        S::U16x8,
        S::I32x4,
        S::U32x4,
        S::I64x2,
        S::U64x2,
        "128"
    );
    conversions!(
        S::ISA,
        S::I8x32,
        S::U8x32,
        S::I16x16,
        S::U16x16,
        S::I32x8,
        S::U32x8,
        S::I64x4,
        S::U64x4,
        "256"
    );
    conversions!(
        S::ISA,
        S::I8x64,
        S::U8x64,
        S::I16x32,
        S::U16x32,
        S::I32x16,
        S::U32x16,
        S::I64x8,
        S::U64x8,
        "512"
    );
}

#[test]
fn int_lanes_match_scalar() {
    each_level!(ints);
}
