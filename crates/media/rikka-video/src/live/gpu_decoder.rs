use std::ptr;

use rikka_gpu::ExternalVideoFrame;
use rikka_gpu::VideoDeviceHandles;

use crate::error::{Error, Result, check};
use crate::live::decoder::LiveCodec;
use crate::live::exchange;
use rikka_ffmpeg::CodecContext;
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avcodec;
use rikka_ffmpeg::avutil;
use rikka_video_core::decode::{get_format_vulkan, supports_vulkan};
use rikka_video_core::hw::HwDevice;
use rikka_video_core::init;
use rikka_video_core::interop;

const HELD_FRAMES: i32 = 4;

#[derive(Debug)]
pub struct GpuFrame {
    pub width: u32,
    pub height: u32,
    pub frame: ExternalVideoFrame,
}

pub struct GpuLiveDecoder {
    codec: CodecContext,
    name: String,
    packet: Packet,
    frame: Frame,
    device: HwDevice,
}

impl GpuLiveDecoder {
    pub fn open(codec: LiveCodec, handles: &VideoDeviceHandles) -> Result<GpuLiveDecoder> {
        init::ensure()?;
        let found = unsafe { avcodec::avcodec_find_decoder(codec.id()) };
        if found.is_null() || !unsafe { supports_vulkan(found) } {
            return Err(Error::Unsupported(format!(
                "Vulkan decoding of {codec:?} in this FFmpeg"
            )));
        }
        let device = HwDevice::new(handles)?;
        let context = unsafe { CodecContext::alloc(found) }?;
        let raw = context.as_ptr();
        let name = unsafe {
            std::ffi::CStr::from_ptr((*found).name)
                .to_string_lossy()
                .into_owned()
        };
        unsafe {
            exchange::refuse_damage(raw);
            (*raw).thread_count = 1;
            (*raw).hw_device_ctx = device.new_ref()?.into_raw();
            (*raw).get_format = Some(get_format_vulkan);
            (*raw).extra_hw_frames = HELD_FRAMES;
        }
        let status = unsafe { avcodec::avcodec_open2(raw, found, ptr::null_mut()) };
        check("avcodec_open2", status)?;
        Ok(GpuLiveDecoder {
            codec: context,
            name,
            packet: Packet::new()?,
            frame: Frame::new()?,
            device,
        })
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn decode(&mut self, data: &[u8]) -> Result<Vec<GpuFrame>> {
        let mut frames = Vec::new();
        let device = self.device.handles();
        exchange::decode_packet(
            &self.codec,
            &mut self.packet,
            data,
            &mut self.frame,
            |frame| {
                if frame.format() != avutil::AV_PIX_FMT_VULKAN {
                    return Err(Error::Unsupported(
                        "the decoder fell back to software frames".into(),
                    ));
                }
                let (width, height) = (frame.width().max(0) as u32, frame.height().max(0) as u32);
                let taken = std::mem::replace(frame, Frame::new()?);
                frames.push(GpuFrame {
                    width,
                    height,
                    frame: interop::to_external(taken, device)?,
                });
                Ok(())
            },
        )?;
        Ok(frames)
    }
}

impl std::fmt::Debug for GpuLiveDecoder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GpuLiveDecoder")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}
