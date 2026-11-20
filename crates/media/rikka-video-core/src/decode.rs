use std::ptr;

use crate::error::{Error, Result, check};
use crate::ffi::vulkan::AVVulkanFramesContext;
use crate::hw::HwDevice;
use crate::interop::SURFACE_FLAGS;
use rikka_ffmpeg::BufferRef;
use rikka_ffmpeg::CodecContext;
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avcodec::{self, AVCodec, AVCodecContext, AVCodecParameters, AVPacket};
use rikka_ffmpeg::avutil::{self, AVHWFramesContext, AVPixelFormat, AVRational};

const MISSING_WORK_SURFACES: i32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decoded {
    Frame,
    NeedsInput,
    Eof,
}

pub struct Decoder {
    ctx: CodecContext,
    hw: bool,
}

pub unsafe fn supports_vulkan(codec: *const AVCodec) -> bool {
    for index in 0.. {
        let config = unsafe { avcodec::avcodec_get_hw_config(codec, index) };
        if config.is_null() {
            return false;
        }
        let config = unsafe { &*config };
        if config.device_type == avutil::AV_HWDEVICE_TYPE_VULKAN
            && config.methods & avcodec::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX != 0
        {
            return true;
        }
    }
    false
}

pub unsafe extern "C" fn get_format_vulkan(
    ctx: *mut AVCodecContext,
    formats: *const AVPixelFormat,
) -> AVPixelFormat {
    let mut fallback = avutil::AV_PIX_FMT_NONE;
    let mut cursor = formats;
    unsafe {
        while *cursor != avutil::AV_PIX_FMT_NONE {
            if *cursor == avutil::AV_PIX_FMT_VULKAN {
                if let Err(error) = decode_surfaces(ctx) {
                    tracing::warn!("cannot make decode-only Vulkan surfaces: {error}");
                }
                return avutil::AV_PIX_FMT_VULKAN;
            }
            if fallback == avutil::AV_PIX_FMT_NONE {
                fallback = *cursor;
            }
            cursor = cursor.add(1);
        }
    }
    tracing::warn!("decoder offers no Vulkan format, using software frames");
    fallback
}

unsafe fn decode_surfaces(ctx: *mut AVCodecContext) -> Result<()> {
    unsafe {
        avutil::av_buffer_unref(&mut (*ctx).hw_frames_ctx);
        let mut made = ptr::null_mut();
        let found = avcodec::avcodec_get_hw_frames_parameters(
            ctx,
            (*ctx).hw_device_ctx,
            avutil::AV_PIX_FMT_VULKAN,
            &mut made,
        );
        check("avcodec_get_hw_frames_parameters", found)?;
        let frames = BufferRef::from_raw(made, "avcodec_get_hw_frames_parameters")?;
        let context = (*frames.as_ptr()).data.cast::<AVHWFramesContext>();
        if (*context).initial_pool_size > 0 {
            (*context).initial_pool_size += MISSING_WORK_SURFACES;
        }
        let vulkan = &mut *(*context).hwctx.cast::<AVVulkanFramesContext>();
        vulkan.img_flags = SURFACE_FLAGS;
        check(
            "av_hwframe_ctx_init",
            avutil::av_hwframe_ctx_init(frames.as_ptr()),
        )?;
        (*ctx).hw_frames_ctx = frames.into_raw();
    }
    Ok(())
}

impl Decoder {
    pub fn open(
        codec: *const AVCodec,
        parameters: *const AVCodecParameters,
        packet_time_base: AVRational,
        hw: Option<&HwDevice>,
    ) -> Result<Self> {
        Self::opened(
            codec,
            parameters,
            packet_time_base,
            hw,
            avcodec::FF_THREAD_FRAME | avcodec::FF_THREAD_SLICE,
        )
    }

    pub fn for_stills(
        codec: *const AVCodec,
        parameters: *const AVCodecParameters,
        packet_time_base: AVRational,
    ) -> Result<Self> {
        Self::opened(
            codec,
            parameters,
            packet_time_base,
            None,
            avcodec::FF_THREAD_SLICE,
        )
    }

    fn opened(
        codec: *const AVCodec,
        parameters: *const AVCodecParameters,
        packet_time_base: AVRational,
        hw: Option<&HwDevice>,
        threading: i32,
    ) -> Result<Self> {
        let ctx = unsafe { CodecContext::alloc(codec) }?;
        let raw = ctx.as_ptr();
        let applied = unsafe { avcodec::avcodec_parameters_to_context(raw, parameters) };
        check("avcodec_parameters_to_context", applied)?;

        let hw = hw.filter(|_| unsafe { supports_vulkan(codec) });
        unsafe {
            (*raw).pkt_timebase = packet_time_base;
            match hw {
                Some(device) => {
                    (*raw).hw_device_ctx = device.new_ref()?.into_raw();
                    (*raw).get_format = Some(get_format_vulkan);
                    (*raw).extra_hw_frames = 8;
                }
                None => {
                    (*raw).thread_count = 0;
                    (*raw).thread_type = threading;
                }
            }
        }

        let status = unsafe { avcodec::avcodec_open2(raw, codec, ptr::null_mut()) };
        check("avcodec_open2", status)?;
        Ok(Self {
            ctx,
            hw: hw.is_some(),
        })
    }

    pub fn is_hardware(&self) -> bool {
        self.hw
    }

    pub fn send(&mut self, packet: Option<&mut Packet>) -> Result<()> {
        self.send_raw(packet.map_or(ptr::null(), |packet| packet.as_mut_ptr().cast_const()))
    }

    pub fn send_borrowed(&mut self, packet: &AVPacket) -> Result<()> {
        self.send_raw(packet)
    }

    fn send_raw(&mut self, packet: *const AVPacket) -> Result<()> {
        let result = unsafe { avcodec::avcodec_send_packet(self.ctx.as_ptr(), packet) };
        if result == avutil::AVERROR_EAGAIN {
            return Err(Error::Unsupported(
                "decoder rejected input while frames are pending".into(),
            ));
        }
        check("avcodec_send_packet", result)?;
        Ok(())
    }

    pub fn receive(&mut self, frame: &mut Frame) -> Result<Decoded> {
        let result =
            unsafe { avcodec::avcodec_receive_frame(self.ctx.as_ptr(), frame.as_mut_ptr()) };
        if result == avutil::AVERROR_EAGAIN {
            return Ok(Decoded::NeedsInput);
        }
        if result == avutil::AVERROR_EOF {
            return Ok(Decoded::Eof);
        }
        check("avcodec_receive_frame", result)?;
        Ok(Decoded::Frame)
    }

    pub fn flush(&mut self) {
        unsafe { avcodec::avcodec_flush_buffers(self.ctx.as_ptr()) };
    }
}

impl std::fmt::Debug for Decoder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Decoder").finish_non_exhaustive()
    }
}
