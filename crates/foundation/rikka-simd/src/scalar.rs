use std::arch::x86_64::{
    _mm_cvtsd_si64, _mm_cvtss_si32, _mm_cvttsd_si64, _mm_cvttss_si32, _mm_set_sd, _mm_set_ss,
};

use crate::sealed::Sealed;

pub trait ScalarFloat: Copy + Sealed {
    type Int;

    fn to_int_trunc(self) -> Self::Int;

    fn to_int_round(self) -> Self::Int;
}

impl ScalarFloat for f32 {
    type Int = i32;

    #[inline(always)]
    fn to_int_trunc(self) -> i32 {
        unsafe { _mm_cvttss_si32(_mm_set_ss(self)) }
    }

    #[inline(always)]
    fn to_int_round(self) -> i32 {
        unsafe { _mm_cvtss_si32(_mm_set_ss(self)) }
    }
}

impl ScalarFloat for f64 {
    type Int = i64;

    #[inline(always)]
    fn to_int_trunc(self) -> i64 {
        unsafe { _mm_cvttsd_si64(_mm_set_sd(self)) }
    }

    #[inline(always)]
    fn to_int_round(self) -> i64 {
        unsafe { _mm_cvtsd_si64(_mm_set_sd(self)) }
    }
}
