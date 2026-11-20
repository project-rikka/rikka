use core::ffi::{c_int, c_void};
use std::ptr;

use crate::avcodec::{self, AVCodec, AVCodecContext, AVSampleFormat};
use crate::avutil::{AVChannelLayout, AVPixelFormat};
use crate::error::{Error, Result};

#[derive(Debug)]
pub struct CodecContext(*mut AVCodecContext);

unsafe impl Send for CodecContext {}

impl CodecContext {
    pub unsafe fn alloc(codec: *const AVCodec) -> Result<Self> {
        let ctx = unsafe { avcodec::avcodec_alloc_context3(codec) };
        match ctx.is_null() {
            true => Err(Error::Unsupported("avcodec_alloc_context3 failed".into())),
            false => Ok(Self(ctx)),
        }
    }

    pub fn as_ptr(&self) -> *mut AVCodecContext {
        self.0
    }

    pub fn pixel_formats(&self) -> Option<Vec<AVPixelFormat>> {
        unsafe { self.supported(avcodec::AV_CODEC_CONFIG_PIX_FORMAT) }
    }

    pub fn sample_formats(&self) -> Option<Vec<AVSampleFormat>> {
        unsafe { self.supported(avcodec::AV_CODEC_CONFIG_SAMPLE_FORMAT) }
    }

    pub fn sample_rates(&self) -> Option<Vec<c_int>> {
        unsafe { self.supported(avcodec::AV_CODEC_CONFIG_SAMPLE_RATE) }
    }

    pub fn channel_counts(&self) -> Option<Vec<c_int>> {
        let layouts: Vec<*const AVChannelLayout> = unsafe {
            self.supported_at(
                avcodec::AV_CODEC_CONFIG_CHANNEL_LAYOUT,
                size_of::<AVChannelLayout>(),
            )
        }?;
        Some(
            layouts
                .into_iter()
                .map(|layout| unsafe { (*layout).nb_channels })
                .collect(),
        )
    }

    unsafe fn supported<T: Copy>(&self, config: avcodec::AVCodecConfig) -> Option<Vec<T>> {
        let places: Vec<*const T> = unsafe { self.supported_at(config, size_of::<T>()) }?;
        Some(places.into_iter().map(|place| unsafe { *place }).collect())
    }

    unsafe fn supported_at<T>(
        &self,
        config: avcodec::AVCodecConfig,
        stride: usize,
    ) -> Option<Vec<*const T>> {
        let mut configs: *const c_void = ptr::null();
        let mut count: c_int = 0;
        let status = unsafe {
            avcodec::avcodec_get_supported_config(
                self.0,
                (*self.0).codec,
                config,
                0,
                &mut configs,
                &mut count,
            )
        };
        if status < 0 || configs.is_null() {
            return None;
        }
        let start = configs.cast::<u8>();
        Some(
            (0..count.max(0) as usize)
                .map(|index| unsafe { start.add(index * stride).cast::<T>() })
                .collect(),
        )
    }
}

impl Drop for CodecContext {
    fn drop(&mut self) {
        unsafe { avcodec::avcodec_free_context(&mut self.0) };
    }
}
