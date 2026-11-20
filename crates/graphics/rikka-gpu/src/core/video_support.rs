use std::collections::HashSet;
use std::ffi::{CStr, CString};

use ash::vk;

use crate::core::instance::VulkanInstance;

const VIDEO_QUEUE: &CStr = c"VK_KHR_video_queue";
const VIDEO_DECODE_QUEUE: &CStr = c"VK_KHR_video_decode_queue";
const VIDEO_ENCODE_QUEUE: &CStr = c"VK_KHR_video_encode_queue";
const VIDEO_MAINTENANCE1: &CStr = c"VK_KHR_video_maintenance1";
const DECODE_CODECS: [&CStr; 3] = [
    c"VK_KHR_video_decode_h264",
    c"VK_KHR_video_decode_h265",
    c"VK_KHR_video_decode_av1",
];
const ENCODE_CODECS: [&CStr; 3] = [
    c"VK_KHR_video_encode_h264",
    c"VK_KHR_video_encode_h265",
    c"VK_KHR_video_encode_av1",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VideoNeed {
    pub decode: bool,
    pub encode: bool,
}

impl VideoNeed {
    pub(crate) fn any(self) -> bool {
        self.decode || self.encode
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VideoQueueFamilyInfo {
    pub index: u32,
    pub flags: vk::QueueFlags,
    pub video_ops: vk::VideoCodecOperationFlagsKHR,
}

#[derive(Clone, Debug)]
pub(crate) struct VideoSupport {
    pub extensions: Vec<&'static CStr>,
    pub families: Vec<VideoQueueFamilyInfo>,
    pub video_maintenance1: bool,
}

pub(crate) fn detect(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    graphics_family: u32,
    available: &HashSet<CString>,
    need: VideoNeed,
) -> Option<VideoSupport> {
    let has = |name: &CStr| available.contains(name);

    let decode = need.decode && has(VIDEO_DECODE_QUEUE) && DECODE_CODECS.iter().any(|c| has(c));
    let encode = need.encode && has(VIDEO_ENCODE_QUEUE) && ENCODE_CODECS.iter().any(|c| has(c));
    if !has(VIDEO_QUEUE) || (!decode && !encode) {
        return None;
    }

    let mut features11 = vk::PhysicalDeviceVulkan11Features::default();
    let mut features2 = vk::PhysicalDeviceFeatures2::default().push_next(&mut features11);
    unsafe {
        instance
            .handle()
            .get_physical_device_features2(physical, &mut features2);
    }
    if features11.sampler_ycbcr_conversion == vk::FALSE {
        return None;
    }

    let family_count = unsafe {
        instance
            .handle()
            .get_physical_device_queue_family_properties2_len(physical)
    };
    let mut video_properties = vec![vk::QueueFamilyVideoPropertiesKHR::default(); family_count];
    let mut family_properties: Vec<vk::QueueFamilyProperties2<'_>> = video_properties
        .iter_mut()
        .map(|video| vk::QueueFamilyProperties2::default().push_next(video))
        .collect();
    unsafe {
        instance
            .handle()
            .get_physical_device_queue_family_properties2(physical, &mut family_properties);
    }

    let family_flags: Vec<vk::QueueFlags> = family_properties
        .iter()
        .map(|properties| properties.queue_family_properties.queue_flags)
        .collect();
    drop(family_properties);

    let family_doing = |needed: bool, op: vk::QueueFlags| {
        needed
            .then(|| {
                family_flags
                    .iter()
                    .zip(0u32..)
                    .find(|&(flags, index)| index != graphics_family && flags.contains(op))
            })
            .flatten()
            .map(|(&flags, index)| VideoQueueFamilyInfo {
                index,
                flags,
                video_ops: video_properties[index as usize].video_codec_operations,
            })
    };
    let decoding = family_doing(decode, vk::QueueFlags::VIDEO_DECODE_KHR);
    let encoding = family_doing(encode, vk::QueueFlags::VIDEO_ENCODE_KHR);
    let mut families: Vec<VideoQueueFamilyInfo> = decoding.into_iter().collect();
    families.extend(
        encoding.filter(|encoder| decoding.is_none_or(|decoder| decoder.index != encoder.index)),
    );
    if families.is_empty() {
        return None;
    }

    let mut extensions = vec![VIDEO_QUEUE];
    if decoding.is_some() {
        extensions.push(VIDEO_DECODE_QUEUE);
        extensions.extend(DECODE_CODECS.iter().copied().filter(|c| has(c)));
    }
    if encoding.is_some() {
        extensions.push(VIDEO_ENCODE_QUEUE);
        extensions.extend(ENCODE_CODECS.iter().copied().filter(|c| has(c)));
    }
    let video_maintenance1 = has(VIDEO_MAINTENANCE1);
    if video_maintenance1 {
        extensions.push(VIDEO_MAINTENANCE1);
    }

    Some(VideoSupport {
        extensions,
        families,
        video_maintenance1,
    })
}
