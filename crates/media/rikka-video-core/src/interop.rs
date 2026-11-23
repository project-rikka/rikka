use ash::vk;
use rikka_gpu::{
    ExternalVideoFrame, FrameClaim, LentFrame, VideoDeviceHandles, VideoFrameImage, VideoPlane,
};

use crate::color;
use crate::error::{Error, Result};
use crate::ffi::vulkan::{AVVkFrame, AVVulkanFramesContext};
use rikka_ffmpeg::Frame;
use rikka_ffmpeg::avutil::{self, AVHWFramesContext};

pub(crate) const SURFACE_FLAGS: vk::ImageCreateFlags = vk::ImageCreateFlags::from_raw(
    vk::ImageCreateFlags::MUTABLE_FORMAT.as_raw() | vk::ImageCreateFlags::EXTENDED_USAGE.as_raw(),
);

struct PlaneSpec {
    formats: &'static [vk::Format],
    bit_depth: u32,
    storage_scale: f64,
}

fn plane_spec(sw_format: i32) -> Option<PlaneSpec> {
    const MSB10_IN_16: f64 = 65535.0 / ((1023 << 6) as f64);
    match sw_format {
        avutil::AV_PIX_FMT_NV12 => Some(PlaneSpec {
            formats: &NV12_FORMATS,
            bit_depth: 8,
            storage_scale: 1.0,
        }),
        avutil::AV_PIX_FMT_P010LE => Some(PlaneSpec {
            formats: &[vk::Format::R16_UNORM, vk::Format::R16G16_UNORM],
            bit_depth: 10,
            storage_scale: MSB10_IN_16,
        }),
        avutil::AV_PIX_FMT_P016LE => Some(PlaneSpec {
            formats: &[vk::Format::R16_UNORM, vk::Format::R16G16_UNORM],
            bit_depth: 16,
            storage_scale: 1.0,
        }),
        avutil::AV_PIX_FMT_YUV420P | avutil::AV_PIX_FMT_YUV422P | avutil::AV_PIX_FMT_YUV444P => {
            Some(PlaneSpec {
                formats: &[
                    vk::Format::R8_UNORM,
                    vk::Format::R8_UNORM,
                    vk::Format::R8_UNORM,
                ],
                bit_depth: 8,
                storage_scale: 1.0,
            })
        }
        avutil::AV_PIX_FMT_YUV420P10LE => Some(PlaneSpec {
            formats: &[
                vk::Format::R16_UNORM,
                vk::Format::R16_UNORM,
                vk::Format::R16_UNORM,
            ],
            bit_depth: 10,
            storage_scale: 65535.0 / 1023.0,
        }),
        _ => None,
    }
}

const NV12_FORMATS: [vk::Format; 2] = [vk::Format::R8_UNORM, vk::Format::R8G8_UNORM];

const PLANE_ASPECTS: [vk::ImageAspectFlags; 3] = [
    vk::ImageAspectFlags::PLANE_0,
    vk::ImageAspectFlags::PLANE_1,
    vk::ImageAspectFlags::PLANE_2,
];

#[derive(Clone, Copy, Debug)]
struct ImageState {
    layout: vk::ImageLayout,
    access: vk::AccessFlags2,
    queue_family: u32,
}

fn frames_context(frame: &Frame) -> Result<&AVHWFramesContext> {
    let frames_ref = frame.hw_frames_ctx();
    if frames_ref.is_null() {
        return Err(Error::Unsupported(
            "hardware frame without frames context".into(),
        ));
    }
    Ok(unsafe { &*(*frames_ref).data.cast::<AVHWFramesContext>() })
}

fn locked<T>(frame: &Frame, act: impl FnOnce(&mut AVVkFrame) -> T) -> Result<T> {
    let frames = frames_context(frame)?;
    let vk_frames = frames.hwctx.cast::<AVVulkanFramesContext>();
    let vk_frame = frame.data()[0].cast::<AVVkFrame>();
    if vk_frame.is_null() {
        return Err(Error::Unsupported("Vulkan frame without AVVkFrame".into()));
    }
    let (lock, unlock) = unsafe { ((*vk_frames).lock_frame, (*vk_frames).unlock_frame) };
    let (Some(lock), Some(unlock)) = (lock, unlock) else {
        return Err(Error::Unsupported(
            "Vulkan frames context has no lock_frame".into(),
        ));
    };
    let frames_context = (frames as *const AVHWFramesContext).cast_mut();
    unsafe { lock(frames_context, vk_frame) };
    let done = act(unsafe { &mut *vk_frame });
    unsafe { unlock(frames_context, vk_frame) };
    Ok(done)
}

fn image_count(frame: &AVVkFrame) -> usize {
    frame
        .img
        .iter()
        .take_while(|image| **image != vk::Image::null())
        .count()
}

fn planes(frame: &AVVkFrame, formats: &[vk::Format]) -> Result<Vec<VideoPlane>> {
    let image_count = image_count(frame);
    let plane_count = formats.len();
    if image_count == 1 && plane_count > 1 {
        return Ok(formats
            .iter()
            .zip(PLANE_ASPECTS)
            .map(|(&format, aspect)| VideoPlane {
                image: frame.img[0],
                aspect,
                view_format: format,
            })
            .collect());
    }
    if image_count == plane_count {
        return Ok(formats
            .iter()
            .zip(frame.img)
            .map(|(&format, image)| VideoPlane {
                image,
                aspect: vk::ImageAspectFlags::COLOR,
                view_format: format,
            })
            .collect());
    }
    Err(Error::Unsupported(format!(
        "unexpected Vulkan frame shape: {image_count} images for {plane_count} planes"
    )))
}

fn claim(frame: &mut AVVkFrame, to: ImageState) -> FrameClaim {
    let images = 0..image_count(frame);
    let claimed = FrameClaim {
        images: images
            .clone()
            .map(|index| VideoFrameImage {
                image: frame.img[index],
                layout: frame.layout[index],
                source_queue_family: (frame.queue_family[index] != vk::QUEUE_FAMILY_IGNORED)
                    .then_some(frame.queue_family[index]),
            })
            .collect(),
        waits: images
            .clone()
            .map(|index| (frame.sem[index], frame.sem_value[index]))
            .collect(),
        signals: images
            .clone()
            .map(|index| (frame.sem[index], frame.sem_value[index] + 1))
            .collect(),
    };
    for index in images {
        frame.sem_value[index] += 1;
        frame.layout[index] = to.layout;
        frame.access[index] = to.access;
        if frame.queue_family[index] != vk::QUEUE_FAMILY_IGNORED {
            frame.queue_family[index] = to.queue_family;
        }
    }
    claimed
}

struct DecodedFrame {
    frame: Frame,
    sampled: ImageState,
}

impl LentFrame for DecodedFrame {
    fn claim(&mut self) -> FrameClaim {
        let sampled = self.sampled;
        locked(&self.frame, |vk_frame| claim(vk_frame, sampled))
            .expect("the frame was checked when it was lent")
    }
}

pub fn to_external(frame: Frame, handles: &VideoDeviceHandles) -> Result<ExternalVideoFrame> {
    let frames_ctx = frames_context(&frame)?;
    let sw_format = frames_ctx.sw_format;
    let Some(spec) = plane_spec(sw_format) else {
        return Err(Error::Unsupported(format!(
            "Vulkan decode surface format {} is not supported yet",
            rikka_ffmpeg::pixel_format_name(sw_format)
        )));
    };
    let planes = locked(&frame, |vk_frame| planes(vk_frame, spec.formats))??;
    let coded_width = frames_ctx.width.max(1) as f32;
    let coded_height = frames_ctx.height.max(1) as f32;
    let color_matrix = color::ycbcr_matrix(
        frame.colorspace(),
        frame.color_range(),
        spec.bit_depth,
        spec.storage_scale,
        frame.height(),
    );
    let uv_scale = [
        frame.width() as f32 / coded_width,
        frame.height() as f32 / coded_height,
    ];
    Ok(ExternalVideoFrame {
        chroma_packed: spec.formats.len() == 2,
        uv_scale,
        color_matrix,
        planes,
        lease: handles.lease(DecodedFrame {
            frame,
            sampled: ImageState {
                layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                access: vk::AccessFlags2::SHADER_READ,
                queue_family: handles.graphics_queue_family,
            },
        }),
    })
}
