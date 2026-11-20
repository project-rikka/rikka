use std::arch::x86_64::*;

use super::{carry_blocks, from_pd, from_ps, high, join, low, pd, ps, qwords};
use crate::Simd;
use crate::native::ops::{ElemOps, FloatOps, NarrowOps, NegOps, WidenOps};

const ROUND_DOWN: i32 = 0x09;
const ROUND_UP: i32 = 0x0A;
const ROUND_NEAREST: i32 = 0x08;
const ROUND_ZERO: i32 = 0x0B;

impl ElemOps<__m512i> for f32 {
    #[inline(always)]
    fn equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmp_ps_mask::<_CMP_EQ_OQ>(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_add_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_sub_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_mul_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn less<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmp_ps_mask::<_CMP_LT_OQ>(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmp_ps_mask::<_CMP_LE_OQ>(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_min_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_max_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m512i) -> f32 {
        let halves = <f32 as ElemOps<__m256i>>::add::<S>(low(a), high(a));
        <f32 as ElemOps<__m256i>>::reduce_add::<S>(halves)
    }

    #[inline(always)]
    fn reduce_min<S: Simd>(a: __m512i) -> f32 {
        let halves = <f32 as ElemOps<__m256i>>::minimum::<S>(low(a), high(a));
        <f32 as ElemOps<__m256i>>::reduce_min::<S>(halves)
    }

    #[inline(always)]
    fn reduce_max<S: Simd>(a: __m512i) -> f32 {
        let halves = <f32 as ElemOps<__m256i>>::maximum::<S>(low(a), high(a));
        <f32 as ElemOps<__m256i>>::reduce_max::<S>(halves)
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m512i) -> __m512i {
        unsafe {
            let add = |x: __m512i, y: __m512i| from_ps(_mm512_add_ps(ps(x), ps(y)));
            let a = add(a, _mm512_bslli_epi128::<4>(a));
            let a = add(a, _mm512_bslli_epi128::<8>(a));
            carry_blocks(a, |x| _mm512_shuffle_epi32::<0xFF>(x), add)
        }
    }
}

impl NegOps<__m512i> for f32 {
    #[inline(always)]
    fn neg<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_xor_si512(a, _mm512_set1_epi32(i32::MIN)) }
    }
}

impl FloatOps<__m512i> for f32 {
    #[inline(always)]
    fn div<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_div_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn sqrt<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_sqrt_ps(ps(a))) }
    }

    #[inline(always)]
    fn recip_estimate<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_rcp14_ps(ps(a))) }
    }

    #[inline(always)]
    fn rsqrt_estimate<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_rsqrt14_ps(ps(a))) }
    }

    #[inline(always)]
    fn mul_add<S: Simd>(a: __m512i, b: __m512i, c: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_fmadd_ps(ps(a), ps(b), ps(c))) }
    }

    #[inline(always)]
    fn mul_sub<S: Simd>(a: __m512i, b: __m512i, c: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_fmsub_ps(ps(a), ps(b), ps(c))) }
    }

    #[inline(always)]
    fn neg_mul_add<S: Simd>(a: __m512i, b: __m512i, c: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_fnmadd_ps(ps(a), ps(b), ps(c))) }
    }

    #[inline(always)]
    fn abs<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_andnot_si512(_mm512_set1_epi32(i32::MIN), a) }
    }

    #[inline(always)]
    fn copysign<S: Simd>(a: __m512i, sign: __m512i) -> __m512i {
        unsafe {
            let bit = _mm512_set1_epi32(i32::MIN);
            _mm512_or_si512(_mm512_andnot_si512(bit, a), _mm512_and_si512(bit, sign))
        }
    }

    #[inline(always)]
    fn floor<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_roundscale_ps::<ROUND_DOWN>(ps(a))) }
    }

    #[inline(always)]
    fn ceil<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_roundscale_ps::<ROUND_UP>(ps(a))) }
    }

    #[inline(always)]
    fn round<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_roundscale_ps::<ROUND_NEAREST>(ps(a))) }
    }

    #[inline(always)]
    fn trunc<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_roundscale_ps::<ROUND_ZERO>(ps(a))) }
    }

    #[inline(always)]
    fn is_nan<S: Simd>(a: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmp_ps_mask::<_CMP_UNORD_Q>(ps(a), ps(a))) }
    }

    #[inline(always)]
    fn to_int_trunc<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_cvttps_epi32(ps(a)) }
    }

    #[inline(always)]
    fn to_int_round<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_cvtps_epi32(ps(a)) }
    }

    #[inline(always)]
    fn from_int<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_ps(_mm512_cvtepi32_ps(a)) }
    }

    #[inline(always)]
    fn load_transposed<S: Simd>(rows: &[&[f32; 4]]) -> [__m512i; 4] {
        unsafe {
            let row = |index: usize| _mm_loadu_ps(rows[index].as_ptr());
            let blocks = |first: usize| {
                let combined = _mm512_castps128_ps512(row(first));
                let combined = _mm512_insertf32x4::<1>(combined, row(first + 4));
                let combined = _mm512_insertf32x4::<2>(combined, row(first + 8));
                _mm512_insertf32x4::<3>(combined, row(first + 12))
            };
            let (t0, t1, t2, t3) = (blocks(0), blocks(1), blocks(2), blocks(3));
            let u0 = _mm512_unpacklo_ps(t0, t1);
            let u1 = _mm512_unpackhi_ps(t0, t1);
            let u2 = _mm512_unpacklo_ps(t2, t3);
            let u3 = _mm512_unpackhi_ps(t2, t3);
            [
                _mm512_shuffle_ps::<0x44>(u0, u2),
                _mm512_shuffle_ps::<0xEE>(u0, u2),
                _mm512_shuffle_ps::<0x44>(u1, u3),
                _mm512_shuffle_ps::<0xEE>(u1, u3),
            ]
            .map(from_ps)
        }
    }

    #[inline(always)]
    fn store_transposed<S: Simd>(columns: [__m512i; 4], rows: &mut [&mut [f32; 4]]) {
        unsafe {
            let [c0, c1, c2, c3] = columns.map(ps);
            let u0 = _mm512_unpacklo_ps(c0, c1);
            let u1 = _mm512_unpackhi_ps(c0, c1);
            let u2 = _mm512_unpacklo_ps(c2, c3);
            let u3 = _mm512_unpackhi_ps(c2, c3);
            let values = [
                _mm512_shuffle_ps::<0x44>(u0, u2),
                _mm512_shuffle_ps::<0xEE>(u0, u2),
                _mm512_shuffle_ps::<0x44>(u1, u3),
                _mm512_shuffle_ps::<0xEE>(u1, u3),
            ];
            for (index, value) in values.into_iter().enumerate() {
                _mm_storeu_ps(rows[index].as_mut_ptr(), _mm512_castps512_ps128(value));
                _mm_storeu_ps(
                    rows[index + 4].as_mut_ptr(),
                    _mm512_extractf32x4_ps::<1>(value),
                );
                _mm_storeu_ps(
                    rows[index + 8].as_mut_ptr(),
                    _mm512_extractf32x4_ps::<2>(value),
                );
                _mm_storeu_ps(
                    rows[index + 12].as_mut_ptr(),
                    _mm512_extractf32x4_ps::<3>(value),
                );
            }
        }
    }
}

impl ElemOps<__m512i> for f64 {
    #[inline(always)]
    fn equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmp_pd_mask::<_CMP_EQ_OQ>(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_add_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_sub_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_mul_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn less<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmp_pd_mask::<_CMP_LT_OQ>(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmp_pd_mask::<_CMP_LE_OQ>(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_min_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_max_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m512i) -> f64 {
        let halves = <f64 as ElemOps<__m256i>>::add::<S>(low(a), high(a));
        <f64 as ElemOps<__m256i>>::reduce_add::<S>(halves)
    }

    #[inline(always)]
    fn reduce_min<S: Simd>(a: __m512i) -> f64 {
        let halves = <f64 as ElemOps<__m256i>>::minimum::<S>(low(a), high(a));
        <f64 as ElemOps<__m256i>>::reduce_min::<S>(halves)
    }

    #[inline(always)]
    fn reduce_max<S: Simd>(a: __m512i) -> f64 {
        let halves = <f64 as ElemOps<__m256i>>::maximum::<S>(low(a), high(a));
        <f64 as ElemOps<__m256i>>::reduce_max::<S>(halves)
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m512i) -> __m512i {
        unsafe {
            let add = |x: __m512i, y: __m512i| from_pd(_mm512_add_pd(pd(x), pd(y)));
            let a = add(a, _mm512_bslli_epi128::<8>(a));
            carry_blocks(a, |x| _mm512_shuffle_epi32::<0xEE>(x), add)
        }
    }
}

impl NegOps<__m512i> for f64 {
    #[inline(always)]
    fn neg<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_xor_si512(a, _mm512_set1_epi64(i64::MIN)) }
    }
}

impl FloatOps<__m512i> for f64 {
    #[inline(always)]
    fn div<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_div_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn sqrt<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_sqrt_pd(pd(a))) }
    }

    #[inline(always)]
    fn recip_estimate<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_rcp14_pd(pd(a))) }
    }

    #[inline(always)]
    fn rsqrt_estimate<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_rsqrt14_pd(pd(a))) }
    }

    #[inline(always)]
    fn mul_add<S: Simd>(a: __m512i, b: __m512i, c: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_fmadd_pd(pd(a), pd(b), pd(c))) }
    }

    #[inline(always)]
    fn mul_sub<S: Simd>(a: __m512i, b: __m512i, c: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_fmsub_pd(pd(a), pd(b), pd(c))) }
    }

    #[inline(always)]
    fn neg_mul_add<S: Simd>(a: __m512i, b: __m512i, c: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_fnmadd_pd(pd(a), pd(b), pd(c))) }
    }

    #[inline(always)]
    fn abs<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_andnot_si512(_mm512_set1_epi64(i64::MIN), a) }
    }

    #[inline(always)]
    fn copysign<S: Simd>(a: __m512i, sign: __m512i) -> __m512i {
        unsafe {
            let bit = _mm512_set1_epi64(i64::MIN);
            _mm512_or_si512(_mm512_andnot_si512(bit, a), _mm512_and_si512(bit, sign))
        }
    }

    #[inline(always)]
    fn floor<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_roundscale_pd::<ROUND_DOWN>(pd(a))) }
    }

    #[inline(always)]
    fn ceil<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_roundscale_pd::<ROUND_UP>(pd(a))) }
    }

    #[inline(always)]
    fn round<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_roundscale_pd::<ROUND_NEAREST>(pd(a))) }
    }

    #[inline(always)]
    fn trunc<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_roundscale_pd::<ROUND_ZERO>(pd(a))) }
    }

    #[inline(always)]
    fn is_nan<S: Simd>(a: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmp_pd_mask::<_CMP_UNORD_Q>(pd(a), pd(a))) }
    }

    #[inline(always)]
    fn to_int_trunc<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_cvttpd_epi64(pd(a)) }
    }

    #[inline(always)]
    fn to_int_round<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_cvtpd_epi64(pd(a)) }
    }

    #[inline(always)]
    fn from_int<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_cvtepi64_pd(a)) }
    }

    #[inline(always)]
    fn load_transposed<S: Simd>(rows: &[&[f64; 4]]) -> [__m512i; 4] {
        unsafe {
            let row = |index: usize| {
                let low_row = _mm512_castpd256_pd512(_mm256_loadu_pd(rows[index].as_ptr()));
                _mm512_insertf64x4::<1>(low_row, _mm256_loadu_pd(rows[index + 4].as_ptr()))
            };
            let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
            let t0 = _mm512_unpacklo_pd(r0, r1);
            let t1 = _mm512_unpackhi_pd(r0, r1);
            let t2 = _mm512_unpacklo_pd(r2, r3);
            let t3 = _mm512_unpackhi_pd(r2, r3);
            let first = qwords([0, 1, 8, 9, 4, 5, 12, 13]);
            let second = qwords([2, 3, 10, 11, 6, 7, 14, 15]);
            [
                _mm512_permutex2var_pd(t0, first, t2),
                _mm512_permutex2var_pd(t1, first, t3),
                _mm512_permutex2var_pd(t0, second, t2),
                _mm512_permutex2var_pd(t1, second, t3),
            ]
            .map(from_pd)
        }
    }

    #[inline(always)]
    fn store_transposed<S: Simd>(columns: [__m512i; 4], rows: &mut [&mut [f64; 4]]) {
        unsafe {
            let [c0, c1, c2, c3] = columns.map(pd);
            let first = qwords([0, 1, 8, 9, 4, 5, 12, 13]);
            let second = qwords([2, 3, 10, 11, 6, 7, 14, 15]);
            let t0 = _mm512_permutex2var_pd(c0, first, c2);
            let t1 = _mm512_permutex2var_pd(c1, first, c3);
            let t2 = _mm512_permutex2var_pd(c0, second, c2);
            let t3 = _mm512_permutex2var_pd(c1, second, c3);
            let values = [
                _mm512_unpacklo_pd(t0, t1),
                _mm512_unpackhi_pd(t0, t1),
                _mm512_unpacklo_pd(t2, t3),
                _mm512_unpackhi_pd(t2, t3),
            ];
            for (index, value) in values.into_iter().enumerate() {
                _mm256_storeu_pd(rows[index].as_mut_ptr(), _mm512_castpd512_pd256(value));
                _mm256_storeu_pd(
                    rows[index + 4].as_mut_ptr(),
                    _mm512_extractf64x4_pd::<1>(value),
                );
            }
        }
    }
}

impl WidenOps<__m512i> for f32 {
    type Wide = f64;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_cvtps_pd(_mm256_castsi256_ps(low(a)))) }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m512i) -> __m512i {
        unsafe { from_pd(_mm512_cvtps_pd(_mm256_castsi256_ps(high(a)))) }
    }
}

impl NarrowOps<__m512i> for f64 {
    type Narrow = f32;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m512i, high: __m512i) -> __m512i {
        unsafe {
            join(
                _mm256_castps_si256(_mm512_cvtpd_ps(pd(low))),
                _mm256_castps_si256(_mm512_cvtpd_ps(pd(high))),
            )
        }
    }
}
