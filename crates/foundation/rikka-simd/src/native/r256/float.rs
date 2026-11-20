use std::arch::x86_64::*;

use super::{from_pd, from_ps, high, into_high_block, join, low, pd, ps};
use crate::Simd;
use crate::native::ops::{ElemOps, FloatOps, NarrowOps, NegOps, WidenOps};

const ROUND_DOWN: i32 = _MM_FROUND_TO_NEG_INF | _MM_FROUND_NO_EXC;
const ROUND_UP: i32 = _MM_FROUND_TO_POS_INF | _MM_FROUND_NO_EXC;
const ROUND_NEAREST: i32 = _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC;
const ROUND_ZERO: i32 = _MM_FROUND_TO_ZERO | _MM_FROUND_NO_EXC;

impl ElemOps<__m256i> for f32 {
    #[inline(always)]
    fn equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_cmp_ps::<_CMP_EQ_OQ>(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_add_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_sub_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_mul_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn less<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_cmp_ps::<_CMP_LT_OQ>(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_cmp_ps::<_CMP_LE_OQ>(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_min_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_max_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m256i) -> f32 {
        let halves = <f32 as ElemOps<__m128i>>::add::<S>(low(a), high(a));
        <f32 as ElemOps<__m128i>>::reduce_add::<S>(halves)
    }

    #[inline(always)]
    fn reduce_min<S: Simd>(a: __m256i) -> f32 {
        let halves = <f32 as ElemOps<__m128i>>::minimum::<S>(low(a), high(a));
        <f32 as ElemOps<__m128i>>::reduce_min::<S>(halves)
    }

    #[inline(always)]
    fn reduce_max<S: Simd>(a: __m256i) -> f32 {
        let halves = <f32 as ElemOps<__m128i>>::maximum::<S>(low(a), high(a));
        <f32 as ElemOps<__m128i>>::reduce_max::<S>(halves)
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let a = _mm256_add_ps(ps(a), ps(_mm256_slli_si256::<4>(a)));
            let a = _mm256_add_ps(a, ps(_mm256_slli_si256::<8>(from_ps(a))));
            let last = from_ps(_mm256_shuffle_ps::<0xFF>(a, a));
            from_ps(_mm256_add_ps(a, ps(into_high_block(last))))
        }
    }
}

impl NegOps<__m256i> for f32 {
    #[inline(always)]
    fn neg<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_xor_ps(ps(a), _mm256_set1_ps(-0.0))) }
    }
}

impl FloatOps<__m256i> for f32 {
    #[inline(always)]
    fn div<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_div_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn sqrt<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_sqrt_ps(ps(a))) }
    }

    #[inline(always)]
    fn recip_estimate<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                from_ps(_mm256_rcp14_ps(ps(a)))
            } else {
                from_ps(_mm256_rcp_ps(ps(a)))
            }
        }
    }

    #[inline(always)]
    fn rsqrt_estimate<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                from_ps(_mm256_rsqrt14_ps(ps(a)))
            } else {
                from_ps(_mm256_rsqrt_ps(ps(a)))
            }
        }
    }

    #[inline(always)]
    fn mul_add<S: Simd>(a: __m256i, b: __m256i, c: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_fmadd_ps(ps(a), ps(b), ps(c))) }
    }

    #[inline(always)]
    fn mul_sub<S: Simd>(a: __m256i, b: __m256i, c: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_fmsub_ps(ps(a), ps(b), ps(c))) }
    }

    #[inline(always)]
    fn neg_mul_add<S: Simd>(a: __m256i, b: __m256i, c: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_fnmadd_ps(ps(a), ps(b), ps(c))) }
    }

    #[inline(always)]
    fn abs<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_andnot_ps(_mm256_set1_ps(-0.0), ps(a))) }
    }

    #[inline(always)]
    fn copysign<S: Simd>(a: __m256i, sign: __m256i) -> __m256i {
        unsafe {
            let bit = _mm256_set1_ps(-0.0);
            from_ps(_mm256_or_ps(
                _mm256_andnot_ps(bit, ps(a)),
                _mm256_and_ps(bit, ps(sign)),
            ))
        }
    }

    #[inline(always)]
    fn floor<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_round_ps::<ROUND_DOWN>(ps(a))) }
    }

    #[inline(always)]
    fn ceil<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_round_ps::<ROUND_UP>(ps(a))) }
    }

    #[inline(always)]
    fn round<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_round_ps::<ROUND_NEAREST>(ps(a))) }
    }

    #[inline(always)]
    fn trunc<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_round_ps::<ROUND_ZERO>(ps(a))) }
    }

    #[inline(always)]
    fn is_nan<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_cmp_ps::<_CMP_UNORD_Q>(ps(a), ps(a))) }
    }

    #[inline(always)]
    fn to_int_trunc<S: Simd>(a: __m256i) -> __m256i {
        unsafe { _mm256_cvttps_epi32(ps(a)) }
    }

    #[inline(always)]
    fn to_int_round<S: Simd>(a: __m256i) -> __m256i {
        unsafe { _mm256_cvtps_epi32(ps(a)) }
    }

    #[inline(always)]
    fn from_int<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_ps(_mm256_cvtepi32_ps(a)) }
    }

    #[inline(always)]
    fn load_transposed<S: Simd>(rows: &[&[f32; 4]]) -> [__m256i; 4] {
        unsafe {
            let row = |index: usize| _mm_loadu_ps(rows[index].as_ptr());
            let pair = |first: usize| _mm256_set_m128(row(first + 4), row(first));
            let (t0, t1, t2, t3) = (pair(0), pair(1), pair(2), pair(3));
            let u0 = _mm256_unpacklo_ps(t0, t1);
            let u1 = _mm256_unpackhi_ps(t0, t1);
            let u2 = _mm256_unpacklo_ps(t2, t3);
            let u3 = _mm256_unpackhi_ps(t2, t3);
            [
                _mm256_shuffle_ps::<0x44>(u0, u2),
                _mm256_shuffle_ps::<0xEE>(u0, u2),
                _mm256_shuffle_ps::<0x44>(u1, u3),
                _mm256_shuffle_ps::<0xEE>(u1, u3),
            ]
            .map(from_ps)
        }
    }

    #[inline(always)]
    fn store_transposed<S: Simd>(columns: [__m256i; 4], rows: &mut [&mut [f32; 4]]) {
        unsafe {
            let [c0, c1, c2, c3] = columns.map(ps);
            let u0 = _mm256_unpacklo_ps(c0, c1);
            let u1 = _mm256_unpackhi_ps(c0, c1);
            let u2 = _mm256_unpacklo_ps(c2, c3);
            let u3 = _mm256_unpackhi_ps(c2, c3);
            let values = [
                _mm256_shuffle_ps::<0x44>(u0, u2),
                _mm256_shuffle_ps::<0xEE>(u0, u2),
                _mm256_shuffle_ps::<0x44>(u1, u3),
                _mm256_shuffle_ps::<0xEE>(u1, u3),
            ];
            for (index, value) in values.into_iter().enumerate() {
                _mm_storeu_ps(rows[index].as_mut_ptr(), _mm256_castps256_ps128(value));
                _mm_storeu_ps(
                    rows[index + 4].as_mut_ptr(),
                    _mm256_extractf128_ps::<1>(value),
                );
            }
        }
    }
}

impl ElemOps<__m256i> for f64 {
    #[inline(always)]
    fn equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_cmp_pd::<_CMP_EQ_OQ>(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_add_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_sub_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_mul_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn less<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_cmp_pd::<_CMP_LT_OQ>(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_cmp_pd::<_CMP_LE_OQ>(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_min_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_max_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m256i) -> f64 {
        let halves = <f64 as ElemOps<__m128i>>::add::<S>(low(a), high(a));
        <f64 as ElemOps<__m128i>>::reduce_add::<S>(halves)
    }

    #[inline(always)]
    fn reduce_min<S: Simd>(a: __m256i) -> f64 {
        let halves = <f64 as ElemOps<__m128i>>::minimum::<S>(low(a), high(a));
        <f64 as ElemOps<__m128i>>::reduce_min::<S>(halves)
    }

    #[inline(always)]
    fn reduce_max<S: Simd>(a: __m256i) -> f64 {
        let halves = <f64 as ElemOps<__m128i>>::maximum::<S>(low(a), high(a));
        <f64 as ElemOps<__m128i>>::reduce_max::<S>(halves)
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let a = _mm256_add_pd(pd(a), pd(_mm256_slli_si256::<8>(a)));
            let last = from_pd(_mm256_unpackhi_pd(a, a));
            from_pd(_mm256_add_pd(a, pd(into_high_block(last))))
        }
    }
}

impl NegOps<__m256i> for f64 {
    #[inline(always)]
    fn neg<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_xor_pd(pd(a), _mm256_set1_pd(-0.0))) }
    }
}

impl FloatOps<__m256i> for f64 {
    #[inline(always)]
    fn div<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_div_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn sqrt<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_sqrt_pd(pd(a))) }
    }

    #[inline(always)]
    fn recip_estimate<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                from_pd(_mm256_rcp14_pd(pd(a)))
            } else {
                from_pd(_mm256_div_pd(_mm256_set1_pd(1.0), pd(a)))
            }
        }
    }

    #[inline(always)]
    fn rsqrt_estimate<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                from_pd(_mm256_rsqrt14_pd(pd(a)))
            } else {
                from_pd(_mm256_div_pd(_mm256_set1_pd(1.0), _mm256_sqrt_pd(pd(a))))
            }
        }
    }

    #[inline(always)]
    fn mul_add<S: Simd>(a: __m256i, b: __m256i, c: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_fmadd_pd(pd(a), pd(b), pd(c))) }
    }

    #[inline(always)]
    fn mul_sub<S: Simd>(a: __m256i, b: __m256i, c: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_fmsub_pd(pd(a), pd(b), pd(c))) }
    }

    #[inline(always)]
    fn neg_mul_add<S: Simd>(a: __m256i, b: __m256i, c: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_fnmadd_pd(pd(a), pd(b), pd(c))) }
    }

    #[inline(always)]
    fn abs<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_andnot_pd(_mm256_set1_pd(-0.0), pd(a))) }
    }

    #[inline(always)]
    fn copysign<S: Simd>(a: __m256i, sign: __m256i) -> __m256i {
        unsafe {
            let bit = _mm256_set1_pd(-0.0);
            from_pd(_mm256_or_pd(
                _mm256_andnot_pd(bit, pd(a)),
                _mm256_and_pd(bit, pd(sign)),
            ))
        }
    }

    #[inline(always)]
    fn floor<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_round_pd::<ROUND_DOWN>(pd(a))) }
    }

    #[inline(always)]
    fn ceil<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_round_pd::<ROUND_UP>(pd(a))) }
    }

    #[inline(always)]
    fn round<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_round_pd::<ROUND_NEAREST>(pd(a))) }
    }

    #[inline(always)]
    fn trunc<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_round_pd::<ROUND_ZERO>(pd(a))) }
    }

    #[inline(always)]
    fn is_nan<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_cmp_pd::<_CMP_UNORD_Q>(pd(a), pd(a))) }
    }

    #[inline(always)]
    fn to_int_trunc<S: Simd>(a: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_cvttpd_epi64(pd(a)) }
        } else {
            join(
                <f64 as FloatOps<__m128i>>::to_int_trunc::<S>(low(a)),
                <f64 as FloatOps<__m128i>>::to_int_trunc::<S>(high(a)),
            )
        }
    }

    #[inline(always)]
    fn to_int_round<S: Simd>(a: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_cvtpd_epi64(pd(a)) }
        } else {
            join(
                <f64 as FloatOps<__m128i>>::to_int_round::<S>(low(a)),
                <f64 as FloatOps<__m128i>>::to_int_round::<S>(high(a)),
            )
        }
    }

    #[inline(always)]
    fn from_int<S: Simd>(a: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { from_pd(_mm256_cvtepi64_pd(a)) }
        } else {
            join(
                <f64 as FloatOps<__m128i>>::from_int::<S>(low(a)),
                <f64 as FloatOps<__m128i>>::from_int::<S>(high(a)),
            )
        }
    }

    #[inline(always)]
    fn load_transposed<S: Simd>(rows: &[&[f64; 4]]) -> [__m256i; 4] {
        unsafe {
            let row = |index: usize| _mm256_loadu_pd(rows[index].as_ptr());
            let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
            let t0 = _mm256_unpacklo_pd(r0, r1);
            let t1 = _mm256_unpackhi_pd(r0, r1);
            let t2 = _mm256_unpacklo_pd(r2, r3);
            let t3 = _mm256_unpackhi_pd(r2, r3);
            [
                _mm256_permute2f128_pd::<0x20>(t0, t2),
                _mm256_permute2f128_pd::<0x20>(t1, t3),
                _mm256_permute2f128_pd::<0x31>(t0, t2),
                _mm256_permute2f128_pd::<0x31>(t1, t3),
            ]
            .map(from_pd)
        }
    }

    #[inline(always)]
    fn store_transposed<S: Simd>(columns: [__m256i; 4], rows: &mut [&mut [f64; 4]]) {
        unsafe {
            let [c0, c1, c2, c3] = columns.map(pd);
            let t0 = _mm256_permute2f128_pd::<0x20>(c0, c2);
            let t1 = _mm256_permute2f128_pd::<0x20>(c1, c3);
            let t2 = _mm256_permute2f128_pd::<0x31>(c0, c2);
            let t3 = _mm256_permute2f128_pd::<0x31>(c1, c3);
            let values = [
                _mm256_unpacklo_pd(t0, t1),
                _mm256_unpackhi_pd(t0, t1),
                _mm256_unpacklo_pd(t2, t3),
                _mm256_unpackhi_pd(t2, t3),
            ];
            for (row, value) in rows.iter_mut().zip(values) {
                _mm256_storeu_pd(row.as_mut_ptr(), value);
            }
        }
    }
}

impl WidenOps<__m256i> for f32 {
    type Wide = f64;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_cvtps_pd(_mm256_castps256_ps128(ps(a)))) }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m256i) -> __m256i {
        unsafe { from_pd(_mm256_cvtps_pd(_mm256_extractf128_ps::<1>(ps(a)))) }
    }
}

impl NarrowOps<__m256i> for f64 {
    type Narrow = f32;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        unsafe {
            from_ps(_mm256_set_m128(
                _mm256_cvtpd_ps(pd(high)),
                _mm256_cvtpd_ps(pd(low)),
            ))
        }
    }
}
