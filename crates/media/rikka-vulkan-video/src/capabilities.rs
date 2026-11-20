use std::ptr;

use ash::vk;

use crate::Codec;
use crate::device::VideoDevice;
use crate::error::{Error, Result};
use crate::profile::Profile;

const PICTURE_FORMATS: [vk::Format; 1] = [vk::Format::G8_B8R8_2PLANE_420_UNORM];

#[derive(Clone, Copy, Debug)]
pub(crate) struct H264Limits {
    pub(crate) max_level: ash::vk::native::StdVideoH264LevelIdc,
    pub(crate) min_qp: i32,
    pub(crate) max_qp: i32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct H265Limits {
    pub(crate) max_level: ash::vk::native::StdVideoH265LevelIdc,
    pub(crate) min_qp: i32,
    pub(crate) max_qp: i32,
    pub(crate) ctb_sizes: vk::VideoEncodeH265CtbSizeFlagsKHR,
    pub(crate) transform_sizes: vk::VideoEncodeH265TransformBlockSizeFlagsKHR,
    pub(crate) syntax: vk::VideoEncodeH265StdFlagsKHR,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum CodecLimits {
    H264(H264Limits),
    H265(H265Limits),
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Capabilities {
    pub(crate) size_alignment: u64,
    pub(crate) granularity: vk::Extent2D,
    pub(crate) min_extent: vk::Extent2D,
    pub(crate) max_extent: vk::Extent2D,
    pub(crate) dpb_slots: u32,
    pub(crate) header_version: vk::ExtensionProperties,
    pub(crate) separate_references: bool,
    pub(crate) rate_modes: vk::VideoEncodeRateControlModeFlagsKHR,
    pub(crate) quality_levels: u32,
    pub(crate) codec: CodecLimits,
    pub(crate) picture_format: vk::Format,
    pub(crate) reference_format: vk::Format,
}

impl Capabilities {
    pub(crate) fn query(
        device: &VideoDevice,
        profile: &Profile,
        codec: Codec,
    ) -> Result<Capabilities> {
        if !device.encode_ops.contains(codec.operation()) {
            return Err(Error::Unsupported(codec.to_string()));
        }
        let mut h264 = vk::VideoEncodeH264CapabilitiesKHR::default();
        let mut h265 = vk::VideoEncodeH265CapabilitiesKHR::default();
        let mut encode = vk::VideoEncodeCapabilitiesKHR::default();
        let mut video = vk::VideoCapabilitiesKHR::default();
        encode.p_next = match codec {
            Codec::H264 => ptr::from_mut(&mut h264).cast(),
            Codec::H265 => ptr::from_mut(&mut h265).cast(),
        };
        video.p_next = ptr::from_mut(&mut encode).cast();
        unsafe {
            (device
                .queue_instance
                .get_physical_device_video_capabilities_khr)(
                device.physical,
                profile.info(),
                &mut video,
            )
        }
        .result()
        .map_err(|error| match error {
            vk::Result::ERROR_VIDEO_PROFILE_CODEC_NOT_SUPPORTED_KHR
            | vk::Result::ERROR_VIDEO_PROFILE_OPERATION_NOT_SUPPORTED_KHR
            | vk::Result::ERROR_VIDEO_PROFILE_FORMAT_NOT_SUPPORTED_KHR
            | vk::Result::ERROR_VIDEO_PICTURE_LAYOUT_NOT_SUPPORTED_KHR => {
                Error::Unsupported(codec.to_string())
            }
            other => Error::Vulkan(other),
        })?;
        let (codec_limits, predicted) = match codec {
            Codec::H264 => (
                CodecLimits::H264(H264Limits {
                    max_level: h264.max_level_idc,
                    min_qp: h264.min_qp,
                    max_qp: h264.max_qp,
                }),
                h264.max_p_picture_l0_reference_count,
            ),
            Codec::H265 => (
                CodecLimits::H265(H265Limits {
                    max_level: h265.max_level_idc,
                    min_qp: h265.min_qp,
                    max_qp: h265.max_qp,
                    ctb_sizes: h265.ctb_sizes,
                    transform_sizes: h265.transform_block_sizes,
                    syntax: h265.std_syntax_flags,
                }),
                h265.max_p_picture_l0_reference_count,
            ),
        };
        if predicted == 0 || video.max_active_reference_pictures == 0 || video.max_dpb_slots < 2 {
            return Err(Error::Unsupported(format!("{codec} predicted pictures")));
        }
        let picture_format = format(device, profile, vk::ImageUsageFlags::VIDEO_ENCODE_SRC_KHR)?;
        let reference_format = format(device, profile, vk::ImageUsageFlags::VIDEO_ENCODE_DPB_KHR)?;
        Ok(Capabilities {
            size_alignment: video.min_bitstream_buffer_size_alignment.max(1),
            granularity: video.picture_access_granularity,
            min_extent: video.min_coded_extent,
            max_extent: video.max_coded_extent,
            dpb_slots: video.max_dpb_slots,
            header_version: video.std_header_version,
            separate_references: video
                .flags
                .contains(vk::VideoCapabilityFlagsKHR::SEPARATE_REFERENCE_IMAGES),
            rate_modes: encode.rate_control_modes,
            quality_levels: encode.max_quality_levels.max(1),
            codec: codec_limits,
            picture_format,
            reference_format,
        })
    }

    pub(crate) fn qp_range(&self) -> (i32, i32) {
        match self.codec {
            CodecLimits::H264(limits) => (limits.min_qp, limits.max_qp),
            CodecLimits::H265(limits) => (limits.min_qp, limits.max_qp),
        }
    }
}

fn format(
    device: &VideoDevice,
    profile: &Profile,
    usage: vk::ImageUsageFlags,
) -> Result<vk::Format> {
    let mut info = vk::PhysicalDeviceVideoFormatInfoKHR::default().image_usage(usage);
    info.p_next = ptr::from_ref(profile.list()).cast();
    let mut count = 0;
    let query = device
        .queue_instance
        .get_physical_device_video_format_properties_khr;
    unsafe { (query)(device.physical, &info, &mut count, ptr::null_mut()) }.result()?;
    let mut formats = vec![vk::VideoFormatPropertiesKHR::default(); count as usize];
    unsafe { (query)(device.physical, &info, &mut count, formats.as_mut_ptr()) }.result()?;
    formats.truncate(count as usize);
    PICTURE_FORMATS
        .into_iter()
        .find(|wanted| formats.iter().any(|offered| offered.format == *wanted))
        .ok_or_else(|| Error::Unsupported(format!("NV12 pictures for {usage:?}")))
}
