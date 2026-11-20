use std::ptr;

use ash::vk;
use ash::vk::native;

use crate::Codec;
use crate::capabilities::Capabilities;
use crate::device::VideoDevice;
use crate::error::{Error, Result};

const PREFERRED_TUNING: vk::VideoEncodeTuningModeKHR =
    vk::VideoEncodeTuningModeKHR::ULTRA_LOW_LATENCY;
const FALLBACK_TUNING: vk::VideoEncodeTuningModeKHR = vk::VideoEncodeTuningModeKHR::LOW_LATENCY;

pub(crate) struct Profile {
    parts: Box<Parts>,
}

struct Parts {
    usage: vk::VideoEncodeUsageInfoKHR<'static>,
    h264: vk::VideoEncodeH264ProfileInfoKHR<'static>,
    h265: vk::VideoEncodeH265ProfileInfoKHR<'static>,
    info: vk::VideoProfileInfoKHR<'static>,
    list: vk::VideoProfileListInfoKHR<'static>,
}

unsafe impl Send for Profile {}

impl Profile {
    pub(crate) fn negotiate(device: &VideoDevice, codec: Codec) -> Result<(Profile, Capabilities)> {
        match Profile::query(device, codec, PREFERRED_TUNING) {
            Err(Error::Unsupported(_)) => Profile::query(device, codec, FALLBACK_TUNING),
            result => result,
        }
    }

    fn query(
        device: &VideoDevice,
        codec: Codec,
        tuning: vk::VideoEncodeTuningModeKHR,
    ) -> Result<(Profile, Capabilities)> {
        let profile = Profile::new(codec, tuning);
        let capabilities = Capabilities::query(device, &profile, codec)?;
        Ok((profile, capabilities))
    }

    fn new(codec: Codec, tuning: vk::VideoEncodeTuningModeKHR) -> Profile {
        let mut parts = Box::new(Parts {
            usage: vk::VideoEncodeUsageInfoKHR::default()
                .video_usage_hints(vk::VideoEncodeUsageFlagsKHR::STREAMING)
                .video_content_hints(vk::VideoEncodeContentFlagsKHR::DESKTOP)
                .tuning_mode(tuning),
            h264: vk::VideoEncodeH264ProfileInfoKHR::default()
                .std_profile_idc(native::StdVideoH264ProfileIdc_STD_VIDEO_H264_PROFILE_IDC_HIGH),
            h265: vk::VideoEncodeH265ProfileInfoKHR::default()
                .std_profile_idc(native::StdVideoH265ProfileIdc_STD_VIDEO_H265_PROFILE_IDC_MAIN),
            info: vk::VideoProfileInfoKHR::default()
                .video_codec_operation(codec.operation())
                .chroma_subsampling(vk::VideoChromaSubsamplingFlagsKHR::TYPE_420)
                .luma_bit_depth(vk::VideoComponentBitDepthFlagsKHR::TYPE_8)
                .chroma_bit_depth(vk::VideoComponentBitDepthFlagsKHR::TYPE_8),
            list: vk::VideoProfileListInfoKHR::default(),
        });
        let codec_profile: *const std::ffi::c_void = match codec {
            Codec::H264 => ptr::from_ref(&parts.h264).cast(),
            Codec::H265 => ptr::from_ref(&parts.h265).cast(),
        };
        parts.usage.p_next = codec_profile;
        parts.info.p_next = ptr::from_ref(&parts.usage).cast();
        parts.list.profile_count = 1;
        parts.list.p_profiles = ptr::from_ref(&parts.info);
        Profile { parts }
    }

    pub(crate) fn info(&self) -> &vk::VideoProfileInfoKHR<'static> {
        &self.parts.info
    }

    pub(crate) fn list(&self) -> &vk::VideoProfileListInfoKHR<'static> {
        &self.parts.list
    }
}

impl Codec {
    pub(crate) fn operation(self) -> vk::VideoCodecOperationFlagsKHR {
        match self {
            Codec::H264 => vk::VideoCodecOperationFlagsKHR::ENCODE_H264,
            Codec::H265 => vk::VideoCodecOperationFlagsKHR::ENCODE_H265,
        }
    }
}
