#![allow(
    missing_debug_implementations,
    non_camel_case_types,
    non_snake_case,
    dead_code,
    clippy::too_many_arguments,
    clippy::upper_case_acronyms
)]

use core::ffi::c_int;

use crate::avutil::*;
use crate::symbols;

#[repr(C)]
pub struct SwsContext {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct SwsFilter {
    _opaque: [u8; 0],
}

pub const SWS_FAST_BILINEAR: c_int = 1 << 0;
pub const SWS_BILINEAR: c_int = 1 << 1;
pub const SWS_BICUBIC: c_int = 1 << 2;
pub const SWS_LANCZOS: c_int = 1 << 9;
pub const SWS_FULL_CHR_H_INT: c_int = 1 << 13;
pub const SWS_FULL_CHR_H_INP: c_int = 1 << 14;
pub const SWS_ACCURATE_RND: c_int = 1 << 18;

pub const SWS_UNITY: c_int = 1 << 16;

symbols::symbols! {
    symbols::SWSCALE;

    fn sws_getContext(
        srcW: c_int,
        srcH: c_int,
        srcFormat: AVPixelFormat,
        dstW: c_int,
        dstH: c_int,
        dstFormat: AVPixelFormat,
        flags: c_int,
        srcFilter: *mut SwsFilter,
        dstFilter: *mut SwsFilter,
        param: *const f64,
    ) -> *mut SwsContext;
    fn sws_getCachedContext(
        context: *mut SwsContext,
        srcW: c_int,
        srcH: c_int,
        srcFormat: AVPixelFormat,
        dstW: c_int,
        dstH: c_int,
        dstFormat: AVPixelFormat,
        flags: c_int,
        srcFilter: *mut SwsFilter,
        dstFilter: *mut SwsFilter,
        param: *const f64,
    ) -> *mut SwsContext;
    fn sws_freeContext(swsContext: *mut SwsContext);
    fn sws_getCoefficients(colorspace: c_int) -> *const c_int;
    fn sws_setColorspaceDetails(
        c: *mut SwsContext,
        inv_table: *const c_int,
        srcRange: c_int,
        table: *const c_int,
        dstRange: c_int,
        brightness: c_int,
        contrast: c_int,
        saturation: c_int,
    ) -> c_int;
    fn sws_scale(
        c: *mut SwsContext,
        srcSlice: *const *const u8,
        srcStride: *const c_int,
        srcSliceY: c_int,
        srcSliceH: c_int,
        dst: *const *mut u8,
        dstStride: *const c_int,
    ) -> c_int;
}
