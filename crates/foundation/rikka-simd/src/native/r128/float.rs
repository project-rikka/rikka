use std::arch::x86_64::*;

use super::{blend, from_pd, from_ps, pd, ps};
use crate::Simd;
use crate::native::ops::{ElemOps, FloatOps, NarrowOps, NegOps, WidenOps};

const ROUND_DOWN: i32 = _MM_FROUND_TO_NEG_INF | _MM_FROUND_NO_EXC;
const ROUND_UP: i32 = _MM_FROUND_TO_POS_INF | _MM_FROUND_NO_EXC;
const ROUND_NEAREST: i32 = _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC;
const ROUND_ZERO: i32 = _MM_FROUND_TO_ZERO | _MM_FROUND_NO_EXC;

#[inline(always)]
fn round_magnitude_ps(x: __m128, toward_zero: bool) -> __m128 {
    unsafe {
        let sign = _mm_set1_ps(-0.0);
        let magnitude = _mm_andnot_ps(sign, x);
        let magic = _mm_set1_ps(8_388_608.0);
        let rounded = _mm_sub_ps(_mm_add_ps(magnitude, magic), magic);
        let rounded = if toward_zero {
            let over = _mm_cmpgt_ps(rounded, magnitude);
            _mm_sub_ps(rounded, _mm_and_ps(over, _mm_set1_ps(1.0)))
        } else {
            rounded
        };
        let small = _mm_cmplt_ps(magnitude, magic);
        let unsigned = _mm_or_ps(_mm_and_ps(small, rounded), _mm_andnot_ps(small, magnitude));
        _mm_or_ps(unsigned, _mm_and_ps(x, sign))
    }
}

#[inline(always)]
fn round_magnitude_pd(x: __m128d, toward_zero: bool) -> __m128d {
    unsafe {
        let sign = _mm_set1_pd(-0.0);
        let magnitude = _mm_andnot_pd(sign, x);
        let magic = _mm_set1_pd(4_503_599_627_370_496.0);
        let rounded = _mm_sub_pd(_mm_add_pd(magnitude, magic), magic);
        let rounded = if toward_zero {
            let over = _mm_cmpgt_pd(rounded, magnitude);
            _mm_sub_pd(rounded, _mm_and_pd(over, _mm_set1_pd(1.0)))
        } else {
            rounded
        };
        let small = _mm_cmplt_pd(magnitude, magic);
        let unsigned = _mm_or_pd(_mm_and_pd(small, rounded), _mm_andnot_pd(small, magnitude));
        _mm_or_pd(unsigned, _mm_and_pd(x, sign))
    }
}

impl ElemOps<__m128i> for f32 {
    #[inline(always)]
    fn equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_cmpeq_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_add_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_sub_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_mul_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_cmplt_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_cmple_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_min_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_max_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m128i) -> f32 {
        unsafe {
            let x = ps(a);
            let pairs = _mm_add_ps(x, _mm_movehl_ps(x, x));
            _mm_cvtss_f32(_mm_add_ss(pairs, _mm_shuffle_ps::<0x55>(pairs, pairs)))
        }
    }

    #[inline(always)]
    fn reduce_min<S: Simd>(a: __m128i) -> f32 {
        unsafe {
            let x = ps(a);
            let pairs = _mm_min_ps(x, _mm_movehl_ps(x, x));
            _mm_cvtss_f32(_mm_min_ss(pairs, _mm_shuffle_ps::<0x55>(pairs, pairs)))
        }
    }

    #[inline(always)]
    fn reduce_max<S: Simd>(a: __m128i) -> f32 {
        unsafe {
            let x = ps(a);
            let pairs = _mm_max_ps(x, _mm_movehl_ps(x, x));
            _mm_cvtss_f32(_mm_max_ss(pairs, _mm_shuffle_ps::<0x55>(pairs, pairs)))
        }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            let a = _mm_add_ps(ps(a), ps(_mm_slli_si128::<4>(a)));
            from_ps(_mm_add_ps(a, ps(_mm_slli_si128::<8>(from_ps(a)))))
        }
    }
}

impl NegOps<__m128i> for f32 {
    #[inline(always)]
    fn neg<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_ps(_mm_xor_ps(ps(a), _mm_set1_ps(-0.0))) }
    }
}

impl FloatOps<__m128i> for f32 {
    #[inline(always)]
    fn div<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_ps(_mm_div_ps(ps(a), ps(b))) }
    }

    #[inline(always)]
    fn sqrt<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_ps(_mm_sqrt_ps(ps(a))) }
    }

    #[inline(always)]
    fn recip_estimate<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                from_ps(_mm_rcp14_ps(ps(a)))
            } else {
                from_ps(_mm_rcp_ps(ps(a)))
            }
        }
    }

    #[inline(always)]
    fn rsqrt_estimate<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                from_ps(_mm_rsqrt14_ps(ps(a)))
            } else {
                from_ps(_mm_rsqrt_ps(ps(a)))
            }
        }
    }

    #[inline(always)]
    fn mul_add<S: Simd>(a: __m128i, b: __m128i, c: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                from_ps(_mm_fmadd_ps(ps(a), ps(b), ps(c)))
            } else {
                from_ps(_mm_add_ps(_mm_mul_ps(ps(a), ps(b)), ps(c)))
            }
        }
    }

    #[inline(always)]
    fn mul_sub<S: Simd>(a: __m128i, b: __m128i, c: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                from_ps(_mm_fmsub_ps(ps(a), ps(b), ps(c)))
            } else {
                from_ps(_mm_sub_ps(_mm_mul_ps(ps(a), ps(b)), ps(c)))
            }
        }
    }

    #[inline(always)]
    fn neg_mul_add<S: Simd>(a: __m128i, b: __m128i, c: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                from_ps(_mm_fnmadd_ps(ps(a), ps(b), ps(c)))
            } else {
                from_ps(_mm_sub_ps(ps(c), _mm_mul_ps(ps(a), ps(b))))
            }
        }
    }

    #[inline(always)]
    fn abs<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_ps(_mm_andnot_ps(_mm_set1_ps(-0.0), ps(a))) }
    }

    #[inline(always)]
    fn copysign<S: Simd>(a: __m128i, sign: __m128i) -> __m128i {
        unsafe {
            let bit = _mm_set1_ps(-0.0);
            from_ps(_mm_or_ps(
                _mm_andnot_ps(bit, ps(a)),
                _mm_and_ps(bit, ps(sign)),
            ))
        }
    }

    #[inline(always)]
    fn floor<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                from_ps(_mm_round_ps::<ROUND_DOWN>(ps(a)))
            } else {
                let truncated = round_magnitude_ps(ps(a), true);
                let above = from_ps(_mm_cmpgt_ps(truncated, ps(a)));
                let lowered = from_ps(_mm_sub_ps(truncated, _mm_set1_ps(1.0)));
                blend::<S>(above, lowered, from_ps(truncated))
            }
        }
    }

    #[inline(always)]
    fn ceil<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                from_ps(_mm_round_ps::<ROUND_UP>(ps(a)))
            } else {
                let truncated = round_magnitude_ps(ps(a), true);
                let below = from_ps(_mm_cmplt_ps(truncated, ps(a)));
                let raised = from_ps(_mm_add_ps(truncated, _mm_set1_ps(1.0)));
                blend::<S>(below, raised, from_ps(truncated))
            }
        }
    }

    #[inline(always)]
    fn round<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                from_ps(_mm_round_ps::<ROUND_NEAREST>(ps(a)))
            } else {
                from_ps(round_magnitude_ps(ps(a), false))
            }
        }
    }

    #[inline(always)]
    fn trunc<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                from_ps(_mm_round_ps::<ROUND_ZERO>(ps(a)))
            } else {
                from_ps(round_magnitude_ps(ps(a), true))
            }
        }
    }

    #[inline(always)]
    fn is_nan<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_ps(_mm_cmpunord_ps(ps(a), ps(a))) }
    }

    #[inline(always)]
    fn to_int_trunc<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_cvttps_epi32(ps(a)) }
    }

    #[inline(always)]
    fn to_int_round<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_cvtps_epi32(ps(a)) }
    }

    #[inline(always)]
    fn from_int<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_ps(_mm_cvtepi32_ps(a)) }
    }

    #[inline(always)]
    fn load_transposed<S: Simd>(rows: &[&[f32; 4]]) -> [__m128i; 4] {
        unsafe {
            let row = |index: usize| _mm_loadu_ps(rows[index].as_ptr());
            let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));
            let u0 = _mm_unpacklo_ps(r0, r1);
            let u1 = _mm_unpackhi_ps(r0, r1);
            let u2 = _mm_unpacklo_ps(r2, r3);
            let u3 = _mm_unpackhi_ps(r2, r3);
            [
                _mm_movelh_ps(u0, u2),
                _mm_movehl_ps(u2, u0),
                _mm_movelh_ps(u1, u3),
                _mm_movehl_ps(u3, u1),
            ]
            .map(from_ps)
        }
    }

    #[inline(always)]
    fn store_transposed<S: Simd>(columns: [__m128i; 4], rows: &mut [&mut [f32; 4]]) {
        unsafe {
            let [c0, c1, c2, c3] = columns.map(ps);
            let u0 = _mm_unpacklo_ps(c0, c1);
            let u1 = _mm_unpackhi_ps(c0, c1);
            let u2 = _mm_unpacklo_ps(c2, c3);
            let u3 = _mm_unpackhi_ps(c2, c3);
            let values = [
                _mm_movelh_ps(u0, u2),
                _mm_movehl_ps(u2, u0),
                _mm_movelh_ps(u1, u3),
                _mm_movehl_ps(u3, u1),
            ];
            for (row, value) in rows.iter_mut().zip(values) {
                _mm_storeu_ps(row.as_mut_ptr(), value);
            }
        }
    }
}

impl ElemOps<__m128i> for f64 {
    #[inline(always)]
    fn equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_cmpeq_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_add_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_sub_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_mul_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_cmplt_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_cmple_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_min_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_max_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m128i) -> f64 {
        unsafe {
            let x = pd(a);
            _mm_cvtsd_f64(_mm_add_sd(x, _mm_unpackhi_pd(x, x)))
        }
    }

    #[inline(always)]
    fn reduce_min<S: Simd>(a: __m128i) -> f64 {
        unsafe {
            let x = pd(a);
            _mm_cvtsd_f64(_mm_min_sd(x, _mm_unpackhi_pd(x, x)))
        }
    }

    #[inline(always)]
    fn reduce_max<S: Simd>(a: __m128i) -> f64 {
        unsafe {
            let x = pd(a);
            _mm_cvtsd_f64(_mm_max_sd(x, _mm_unpackhi_pd(x, x)))
        }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_pd(_mm_add_pd(pd(a), pd(_mm_slli_si128::<8>(a)))) }
    }
}

impl NegOps<__m128i> for f64 {
    #[inline(always)]
    fn neg<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_pd(_mm_xor_pd(pd(a), _mm_set1_pd(-0.0))) }
    }
}

impl FloatOps<__m128i> for f64 {
    #[inline(always)]
    fn div<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { from_pd(_mm_div_pd(pd(a), pd(b))) }
    }

    #[inline(always)]
    fn sqrt<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_pd(_mm_sqrt_pd(pd(a))) }
    }

    #[inline(always)]
    fn recip_estimate<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                from_pd(_mm_rcp14_pd(pd(a)))
            } else {
                from_pd(_mm_div_pd(_mm_set1_pd(1.0), pd(a)))
            }
        }
    }

    #[inline(always)]
    fn rsqrt_estimate<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                from_pd(_mm_rsqrt14_pd(pd(a)))
            } else {
                from_pd(_mm_div_pd(_mm_set1_pd(1.0), _mm_sqrt_pd(pd(a))))
            }
        }
    }

    #[inline(always)]
    fn mul_add<S: Simd>(a: __m128i, b: __m128i, c: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                from_pd(_mm_fmadd_pd(pd(a), pd(b), pd(c)))
            } else {
                from_pd(_mm_add_pd(_mm_mul_pd(pd(a), pd(b)), pd(c)))
            }
        }
    }

    #[inline(always)]
    fn mul_sub<S: Simd>(a: __m128i, b: __m128i, c: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                from_pd(_mm_fmsub_pd(pd(a), pd(b), pd(c)))
            } else {
                from_pd(_mm_sub_pd(_mm_mul_pd(pd(a), pd(b)), pd(c)))
            }
        }
    }

    #[inline(always)]
    fn neg_mul_add<S: Simd>(a: __m128i, b: __m128i, c: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                from_pd(_mm_fnmadd_pd(pd(a), pd(b), pd(c)))
            } else {
                from_pd(_mm_sub_pd(pd(c), _mm_mul_pd(pd(a), pd(b))))
            }
        }
    }

    #[inline(always)]
    fn abs<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_pd(_mm_andnot_pd(_mm_set1_pd(-0.0), pd(a))) }
    }

    #[inline(always)]
    fn copysign<S: Simd>(a: __m128i, sign: __m128i) -> __m128i {
        unsafe {
            let bit = _mm_set1_pd(-0.0);
            from_pd(_mm_or_pd(
                _mm_andnot_pd(bit, pd(a)),
                _mm_and_pd(bit, pd(sign)),
            ))
        }
    }

    #[inline(always)]
    fn floor<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                from_pd(_mm_round_pd::<ROUND_DOWN>(pd(a)))
            } else {
                let truncated = round_magnitude_pd(pd(a), true);
                let above = from_pd(_mm_cmpgt_pd(truncated, pd(a)));
                let lowered = from_pd(_mm_sub_pd(truncated, _mm_set1_pd(1.0)));
                blend::<S>(above, lowered, from_pd(truncated))
            }
        }
    }

    #[inline(always)]
    fn ceil<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                from_pd(_mm_round_pd::<ROUND_UP>(pd(a)))
            } else {
                let truncated = round_magnitude_pd(pd(a), true);
                let below = from_pd(_mm_cmplt_pd(truncated, pd(a)));
                let raised = from_pd(_mm_add_pd(truncated, _mm_set1_pd(1.0)));
                blend::<S>(below, raised, from_pd(truncated))
            }
        }
    }

    #[inline(always)]
    fn round<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                from_pd(_mm_round_pd::<ROUND_NEAREST>(pd(a)))
            } else {
                from_pd(round_magnitude_pd(pd(a), false))
            }
        }
    }

    #[inline(always)]
    fn trunc<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                from_pd(_mm_round_pd::<ROUND_ZERO>(pd(a)))
            } else {
                from_pd(round_magnitude_pd(pd(a), true))
            }
        }
    }

    #[inline(always)]
    fn is_nan<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_pd(_mm_cmpunord_pd(pd(a), pd(a))) }
    }

    #[inline(always)]
    fn to_int_trunc<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_cvttpd_epi64(pd(a))
            } else {
                let x = pd(a);
                _mm_set_epi64x(_mm_cvttsd_si64(_mm_unpackhi_pd(x, x)), _mm_cvttsd_si64(x))
            }
        }
    }

    #[inline(always)]
    fn to_int_round<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_cvtpd_epi64(pd(a))
            } else {
                let x = pd(a);
                _mm_set_epi64x(_mm_cvtsd_si64(_mm_unpackhi_pd(x, x)), _mm_cvtsd_si64(x))
            }
        }
    }

    #[inline(always)]
    fn from_int<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                from_pd(_mm_cvtepi64_pd(a))
            } else {
                let zero = _mm_setzero_pd();
                let low = _mm_cvtsi64_sd(zero, _mm_cvtsi128_si64(a));
                let high = _mm_cvtsi64_sd(zero, _mm_cvtsi128_si64(_mm_unpackhi_epi64(a, a)));
                from_pd(_mm_unpacklo_pd(low, high))
            }
        }
    }

    #[inline(always)]
    fn load_transposed<S: Simd>(rows: &[&[f64; 4]]) -> [__m128i; 4] {
        unsafe {
            let half = |row: usize, at: usize| _mm_loadu_pd(rows[row][at..].as_ptr());
            let (low0, high0, low1, high1) = (half(0, 0), half(0, 2), half(1, 0), half(1, 2));
            [
                _mm_unpacklo_pd(low0, low1),
                _mm_unpackhi_pd(low0, low1),
                _mm_unpacklo_pd(high0, high1),
                _mm_unpackhi_pd(high0, high1),
            ]
            .map(from_pd)
        }
    }

    #[inline(always)]
    fn store_transposed<S: Simd>(columns: [__m128i; 4], rows: &mut [&mut [f64; 4]]) {
        unsafe {
            let [c0, c1, c2, c3] = columns.map(pd);
            let halves = [
                (_mm_unpacklo_pd(c0, c1), _mm_unpacklo_pd(c2, c3)),
                (_mm_unpackhi_pd(c0, c1), _mm_unpackhi_pd(c2, c3)),
            ];
            for (row, (low, high)) in rows.iter_mut().zip(halves) {
                _mm_storeu_pd(row.as_mut_ptr(), low);
                _mm_storeu_pd(row[2..].as_mut_ptr(), high);
            }
        }
    }
}

impl WidenOps<__m128i> for f32 {
    type Wide = f64;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_pd(_mm_cvtps_pd(ps(a))) }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m128i) -> __m128i {
        unsafe { from_pd(_mm_cvtps_pd(_mm_movehl_ps(ps(a), ps(a)))) }
    }
}

impl NarrowOps<__m128i> for f64 {
    type Narrow = f32;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe { from_ps(_mm_movelh_ps(_mm_cvtpd_ps(pd(low)), _mm_cvtpd_ps(pd(high)))) }
    }
}
