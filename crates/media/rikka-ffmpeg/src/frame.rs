use core::ffi::c_int;

use crate::avutil::{self, AVColorPrimaries, AVColorTransferCharacteristic, AVFrame, AVRational};
use crate::error::{Error, Result, check};

#[derive(Debug)]
pub struct Frame {
    ptr: *mut AVFrame,
}

unsafe impl Send for Frame {}

impl Frame {
    pub fn new() -> Result<Self> {
        let ptr = unsafe { avutil::av_frame_alloc() };
        if ptr.is_null() {
            return Err(Error::Unsupported("av_frame_alloc failed".into()));
        }
        Ok(Self { ptr })
    }

    pub fn as_ptr(&self) -> *const AVFrame {
        self.ptr
    }

    pub fn as_mut_ptr(&mut self) -> *mut AVFrame {
        self.ptr
    }

    fn get(&self) -> &AVFrame {
        unsafe { &*self.ptr }
    }

    pub fn width(&self) -> i32 {
        self.get().width
    }

    pub fn height(&self) -> i32 {
        self.get().height
    }

    pub fn format(&self) -> c_int {
        self.get().format
    }

    pub fn colorspace(&self) -> c_int {
        self.get().colorspace
    }

    pub fn color_range(&self) -> c_int {
        self.get().color_range
    }

    pub fn color_primaries(&self) -> AVColorPrimaries {
        self.get().color_primaries
    }

    pub fn color_trc(&self) -> AVColorTransferCharacteristic {
        self.get().color_trc
    }

    pub fn icc_profile(&self) -> Option<&[u8]> {
        let side =
            unsafe { avutil::av_frame_get_side_data(self.ptr, avutil::AV_FRAME_DATA_ICC_PROFILE) };
        if side.is_null() {
            return None;
        }
        let side = unsafe { &*side };
        match side.data.is_null() || side.size == 0 {
            true => None,
            false => Some(unsafe { std::slice::from_raw_parts(side.data, side.size) }),
        }
    }

    pub fn set_icc_profile(&mut self, profile: &[u8]) -> Result<()> {
        let side = unsafe {
            avutil::av_frame_new_side_data(
                self.ptr,
                avutil::AV_FRAME_DATA_ICC_PROFILE,
                profile.len(),
            )
        };
        if side.is_null() {
            return Err(Error::Unsupported("av_frame_new_side_data failed".into()));
        }
        unsafe {
            (*side)
                .data
                .copy_from_nonoverlapping(profile.as_ptr(), profile.len())
        };
        Ok(())
    }

    pub fn sample_aspect_ratio(&self) -> AVRational {
        self.get().sample_aspect_ratio
    }

    pub fn is_corrupt(&self) -> bool {
        self.get().flags & avutil::AV_FRAME_FLAG_CORRUPT != 0
    }

    pub fn is_damaged(&self) -> bool {
        self.is_corrupt() || self.get().decode_error_flags != 0
    }

    pub fn samples(&self) -> usize {
        usize::try_from(self.get().nb_samples).unwrap_or(0)
    }

    pub fn sample_rate(&self) -> i32 {
        self.get().sample_rate
    }

    pub fn channels(&self) -> usize {
        usize::try_from(self.get().ch_layout.nb_channels).unwrap_or(0)
    }

    pub fn extended_data(&self) -> *const *mut u8 {
        self.get().extended_data
    }

    pub fn pts_us(&self, time_base: AVRational) -> Option<i64> {
        self.timestamp()
            .map(|pts| unsafe { avutil::av_rescale_q(pts, time_base, avutil::AV_TIME_BASE_Q) })
    }

    pub fn timestamp(&self) -> Option<i64> {
        match self.get().best_effort_timestamp {
            avutil::AV_NOPTS_VALUE => match self.get().pts {
                avutil::AV_NOPTS_VALUE => None,
                pts => Some(pts),
            },
            pts => Some(pts),
        }
    }

    pub fn span(&self) -> i64 {
        self.get().duration
    }

    pub fn hw_frames_ctx(&self) -> *mut avutil::AVBufferRef {
        self.get().hw_frames_ctx
    }

    pub fn data(&self) -> &[*mut u8; avutil::AV_NUM_DATA_POINTERS] {
        &self.get().data
    }

    pub fn linesize(&self) -> &[c_int; avutil::AV_NUM_DATA_POINTERS] {
        &self.get().linesize
    }

    pub fn hold(&mut self, other: &Frame) -> Result<()> {
        self.unref();
        let status = unsafe { avutil::av_frame_ref(self.ptr, other.ptr) };
        check("av_frame_ref", status).map(|_| ())
    }

    pub fn unref(&mut self) {
        unsafe { avutil::av_frame_unref(self.ptr) };
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe { avutil::av_frame_free(&mut self.ptr) };
    }
}
