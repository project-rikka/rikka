use ash::vk;
use rustc_hash::FxHashMap;

use crate::core::instance::VulkanInstance;
use crate::texture::packed::{COLOR_FORMATS, fourcc};

pub const DRM_FORMAT_MOD_LINEAR: u64 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrmFormat {
    pub fourcc: u32,
    pub modifier: u64,
}

const FORMATS: [(u32, vk::Format); 4] = [
    (fourcc(b"XR24"), vk::Format::B8G8R8A8_UNORM),
    (fourcc(b"AR24"), vk::Format::B8G8R8A8_UNORM),
    (fourcc(b"XB24"), vk::Format::R8G8B8A8_UNORM),
    (fourcc(b"AB24"), vk::Format::R8G8B8A8_UNORM),
];

const DEEP_FORMATS: [(u32, vk::Format); 4] = [
    (fourcc(b"XR30"), vk::Format::A2R10G10B10_UNORM_PACK32),
    (fourcc(b"AR30"), vk::Format::A2R10G10B10_UNORM_PACK32),
    (fourcc(b"XB30"), vk::Format::A2B10G10R10_UNORM_PACK32),
    (fourcc(b"AB30"), vk::Format::A2B10G10R10_UNORM_PACK32),
];

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImportUse {
    pub usage: vk::ImageUsageFlags,
    pub features: vk::FormatFeatureFlags,
}

pub(crate) const RENDER_USE: ImportUse = ImportUse {
    usage: vk::ImageUsageFlags::COLOR_ATTACHMENT,
    features: vk::FormatFeatureFlags::COLOR_ATTACHMENT,
};

pub(crate) const READABLE_RENDER_USE: ImportUse = ImportUse {
    usage: vk::ImageUsageFlags::from_raw(
        vk::ImageUsageFlags::COLOR_ATTACHMENT.as_raw() | vk::ImageUsageFlags::SAMPLED.as_raw(),
    ),
    features: vk::FormatFeatureFlags::from_raw(
        vk::FormatFeatureFlags::COLOR_ATTACHMENT.as_raw()
            | vk::FormatFeatureFlags::SAMPLED_IMAGE.as_raw()
            | vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR.as_raw(),
    ),
};

pub(crate) const SAMPLE_USE: ImportUse = ImportUse {
    usage: vk::ImageUsageFlags::SAMPLED,
    features: vk::FormatFeatureFlags::from_raw(
        vk::FormatFeatureFlags::SAMPLED_IMAGE.as_raw()
            | vk::FormatFeatureFlags::SAMPLED_IMAGE_FILTER_LINEAR.as_raw(),
    ),
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextureLayout {
    Color { opaque: bool },
    Planar(PlanarLayout),
}

impl TextureLayout {
    pub(crate) fn views(&self) -> &'static [vk::Format] {
        match self {
            Self::Color { .. } => &[],
            Self::Planar(planar) => planar.views,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PlanarLayout {
    pub views: &'static [vk::Format],
    pub chroma_packed: bool,
    pub chroma_swapped: bool,
    pub bit_depth: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TextureFormat {
    pub image: vk::Format,
    pub layout: TextureLayout,
}

const fn planar(
    views: &'static [vk::Format],
    chroma_packed: bool,
    chroma_swapped: bool,
    bit_depth: u32,
) -> TextureLayout {
    TextureLayout::Planar(PlanarLayout {
        views,
        chroma_packed,
        chroma_swapped,
        bit_depth,
    })
}

const NV_VIEWS_8: [vk::Format; 2] = [vk::Format::R8_UNORM, vk::Format::R8G8_UNORM];
const NV_VIEWS_10: [vk::Format; 2] = [
    vk::Format::R10X6_UNORM_PACK16,
    vk::Format::R10X6G10X6_UNORM_2PACK16,
];
const NV_VIEWS_16: [vk::Format; 2] = [vk::Format::R16_UNORM, vk::Format::R16G16_UNORM];
const YUV_VIEWS_8: [vk::Format; 3] = [
    vk::Format::R8_UNORM,
    vk::Format::R8_UNORM,
    vk::Format::R8_UNORM,
];

const PLANAR_FORMATS: [(u32, vk::Format, TextureLayout); 11] = [
    (
        fourcc(b"NV12"),
        vk::Format::G8_B8R8_2PLANE_420_UNORM,
        planar(&NV_VIEWS_8, true, false, 8),
    ),
    (
        fourcc(b"NV21"),
        vk::Format::G8_B8R8_2PLANE_420_UNORM,
        planar(&NV_VIEWS_8, true, true, 8),
    ),
    (
        fourcc(b"NV16"),
        vk::Format::G8_B8R8_2PLANE_422_UNORM,
        planar(&NV_VIEWS_8, true, false, 8),
    ),
    (
        fourcc(b"NV24"),
        vk::Format::G8_B8R8_2PLANE_444_UNORM,
        planar(&NV_VIEWS_8, true, false, 8),
    ),
    (
        fourcc(b"YU12"),
        vk::Format::G8_B8_R8_3PLANE_420_UNORM,
        planar(&YUV_VIEWS_8, false, false, 8),
    ),
    (
        fourcc(b"YV12"),
        vk::Format::G8_B8_R8_3PLANE_420_UNORM,
        planar(&YUV_VIEWS_8, false, true, 8),
    ),
    (
        fourcc(b"YU16"),
        vk::Format::G8_B8_R8_3PLANE_422_UNORM,
        planar(&YUV_VIEWS_8, false, false, 8),
    ),
    (
        fourcc(b"YU24"),
        vk::Format::G8_B8_R8_3PLANE_444_UNORM,
        planar(&YUV_VIEWS_8, false, false, 8),
    ),
    (
        fourcc(b"P010"),
        vk::Format::G10X6_B10X6R10X6_2PLANE_420_UNORM_3PACK16,
        planar(&NV_VIEWS_10, true, false, 10),
    ),
    (
        fourcc(b"P012"),
        vk::Format::G12X4_B12X4R12X4_2PLANE_420_UNORM_3PACK16,
        planar(&NV_VIEWS_16, true, false, 12),
    ),
    (
        fourcc(b"P016"),
        vk::Format::G16_B16R16_2PLANE_420_UNORM,
        planar(&NV_VIEWS_16, true, false, 16),
    ),
];

pub(crate) fn vk_format(code: u32) -> Option<vk::Format> {
    FORMATS
        .iter()
        .chain(&DEEP_FORMATS)
        .find(|(fourcc, _)| *fourcc == code)
        .map(|(_, format)| *format)
}

fn texture_formats() -> impl Iterator<Item = (u32, vk::Format, TextureLayout)> {
    COLOR_FORMATS
        .iter()
        .map(|format| {
            (
                format.fourcc,
                format.image,
                TextureLayout::Color {
                    opaque: format.opaque,
                },
            )
        })
        .chain(PLANAR_FORMATS)
}

pub(crate) fn texture_format(code: u32) -> Option<TextureFormat> {
    texture_formats()
        .find(|(fourcc, _, _)| *fourcc == code)
        .map(|(_, image, layout)| TextureFormat { image, layout })
}

pub fn query_render_formats(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
) -> Vec<DrmFormat> {
    query_formats(
        instance,
        physical,
        RENDER_USE,
        FORMATS
            .iter()
            .chain(&DEEP_FORMATS)
            .map(|(fourcc, format)| (*fourcc, *format, &[][..])),
    )
}

pub fn query_export_formats(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
) -> Vec<DrmFormat> {
    let mut formats = Vec::new();
    for (fourcc, format) in FORMATS.into_iter().chain(DEEP_FORMATS) {
        formats.extend(
            exportable_modifiers(instance, physical, format, RENDER_USE)
                .into_iter()
                .map(|modifier| DrmFormat { fourcc, modifier }),
        );
    }
    formats
}

pub fn query_texture_formats(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
) -> Vec<DrmFormat> {
    query_formats(
        instance,
        physical,
        SAMPLE_USE,
        texture_formats().map(|(fourcc, format, layout)| (fourcc, format, layout.views())),
    )
}

fn query_formats<'a>(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    purpose: ImportUse,
    entries: impl IntoIterator<Item = (u32, vk::Format, &'a [vk::Format])>,
) -> Vec<DrmFormat> {
    let mut probed: FxHashMap<vk::Format, Vec<u64>> = FxHashMap::default();
    let mut formats = Vec::new();
    for (fourcc, format, views) in entries {
        let modifiers = probed
            .entry(format)
            .or_insert_with(|| usable_modifiers(instance, physical, format, purpose, views));
        formats.extend(
            modifiers
                .iter()
                .map(|&modifier| DrmFormat { fourcc, modifier }),
        );
    }
    formats
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ModifierProperties {
    pub planes: u32,
    pub disjoint: bool,
    pub max_extent: vk::Extent2D,
}

pub(crate) fn modifier_properties(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    format: vk::Format,
    modifier: u64,
    purpose: ImportUse,
    views: &[vk::Format],
) -> Option<ModifierProperties> {
    let entry = modifier_features(instance, physical, format)
        .into_iter()
        .find(|entry| entry.drm_format_modifier == modifier)?;
    if !entry
        .drm_format_modifier_tiling_features
        .contains(purpose.features)
    {
        return None;
    }
    let max_extent = external_extent(
        instance,
        physical,
        format,
        modifier,
        purpose.usage,
        views,
        vk::ExternalMemoryFeatureFlags::IMPORTABLE,
    )?;
    Some(ModifierProperties {
        planes: entry.drm_format_modifier_plane_count,
        disjoint: entry
            .drm_format_modifier_tiling_features
            .contains(vk::FormatFeatureFlags::DISJOINT),
        max_extent,
    })
}

fn usable_modifiers(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    format: vk::Format,
    purpose: ImportUse,
    views: &[vk::Format],
) -> Vec<u64> {
    modifiers_with(
        instance,
        physical,
        format,
        purpose,
        views,
        vk::ExternalMemoryFeatureFlags::IMPORTABLE,
    )
}

pub(crate) fn exportable_modifiers(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    format: vk::Format,
    purpose: ImportUse,
) -> Vec<u64> {
    modifiers_with(
        instance,
        physical,
        format,
        purpose,
        &[],
        vk::ExternalMemoryFeatureFlags::EXPORTABLE,
    )
}

fn modifiers_with(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    format: vk::Format,
    purpose: ImportUse,
    views: &[vk::Format],
    external: vk::ExternalMemoryFeatureFlags,
) -> Vec<u64> {
    modifier_features(instance, physical, format)
        .into_iter()
        .filter(|entry| {
            entry
                .drm_format_modifier_tiling_features
                .contains(purpose.features)
                && external_extent(
                    instance,
                    physical,
                    format,
                    entry.drm_format_modifier,
                    purpose.usage,
                    views,
                    external,
                )
                .is_some()
        })
        .map(|entry| entry.drm_format_modifier)
        .collect()
}

pub(crate) fn modifier_features(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    format: vk::Format,
) -> Vec<vk::DrmFormatModifierPropertiesEXT> {
    let mut list = vk::DrmFormatModifierPropertiesListEXT::default();
    let mut properties = vk::FormatProperties2::default().push_next(&mut list);
    unsafe {
        instance
            .handle()
            .get_physical_device_format_properties2(physical, format, &mut properties);
    }
    let count = list.drm_format_modifier_count as usize;
    if count == 0 {
        return Vec::new();
    }

    let mut entries = vec![vk::DrmFormatModifierPropertiesEXT::default(); count];
    let mut list = vk::DrmFormatModifierPropertiesListEXT::default()
        .drm_format_modifier_properties(&mut entries);
    let mut properties = vk::FormatProperties2::default().push_next(&mut list);
    unsafe {
        instance
            .handle()
            .get_physical_device_format_properties2(physical, format, &mut properties);
    }
    entries
}

fn external_extent(
    instance: &VulkanInstance,
    physical: vk::PhysicalDevice,
    format: vk::Format,
    modifier: u64,
    usage: vk::ImageUsageFlags,
    views: &[vk::Format],
    external_features: vk::ExternalMemoryFeatureFlags,
) -> Option<vk::Extent2D> {
    let mut modifier_info = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
        .drm_format_modifier(modifier)
        .sharing_mode(vk::SharingMode::EXCLUSIVE);
    let mut external_info = vk::PhysicalDeviceExternalImageFormatInfo::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let mut view_formats = vk::ImageFormatListCreateInfo::default().view_formats(views);
    let mut info = vk::PhysicalDeviceImageFormatInfo2::default()
        .format(format)
        .ty(vk::ImageType::TYPE_2D)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .usage(usage)
        .flags(mutable_flags(views))
        .push_next(&mut modifier_info)
        .push_next(&mut external_info);
    if !views.is_empty() {
        info = info.push_next(&mut view_formats);
    }

    let mut external = vk::ExternalImageFormatProperties::default();
    let limits = {
        let mut properties = vk::ImageFormatProperties2::default().push_next(&mut external);
        unsafe {
            instance
                .handle()
                .get_physical_device_image_format_properties2(physical, &info, &mut properties)
        }
        .ok()?;
        properties.image_format_properties
    };

    external
        .external_memory_properties
        .external_memory_features
        .contains(external_features)
        .then_some(vk::Extent2D {
            width: limits.max_extent.width,
            height: limits.max_extent.height,
        })
}

pub(crate) fn mutable_flags(views: &[vk::Format]) -> vk::ImageCreateFlags {
    if views.is_empty() {
        vk::ImageCreateFlags::empty()
    } else {
        vk::ImageCreateFlags::MUTABLE_FORMAT
    }
}
