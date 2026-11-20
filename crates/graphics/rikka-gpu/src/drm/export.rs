use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::image::{GpuImage, ImageMemory, PartialImage};
use crate::drm::format::{
    DrmFormat, READABLE_RENDER_USE, RENDER_USE, exportable_modifiers, modifier_features, vk_format,
};
use crate::drm::target::DmabufTarget;
use crate::error::{Error, Result};

#[derive(Debug)]
pub struct ExportedPlane {
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Debug)]
pub struct ExportedDmabuf {
    pub target: DmabufTarget,
    pub format: DrmFormat,
    pub planes: Vec<ExportedPlane>,
}

pub struct ExportedImage {
    pub image: GpuImage,
    pub readable: bool,
    pub format: DrmFormat,
    pub planes: Vec<ExportedPlane>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Device,
    System,
}

pub fn export_render(
    device: Arc<RenderDevice>,
    extent: vk::Extent2D,
    fourcc: u32,
    modifiers: &[u64],
    placement: Placement,
) -> Result<ExportedImage> {
    let dmabuf = device
        .dmabuf()
        .ok_or_else(|| Error::Unsupported("device does not support dmabuf sharing".into()))?;
    let format = vk_format(fourcc)
        .ok_or_else(|| Error::Unsupported(format!("unsupported DRM format {fourcc:#010x}")))?;
    let (usage, layouts, readable) = layouts_for(&device, format, modifiers).ok_or_else(|| {
        Error::Unsupported(format!(
            "device exports none of the {} offered modifiers for DRM format {fourcc:#010x}",
            modifiers.len()
        ))
    })?;

    let mut listed =
        vk::ImageDrmFormatModifierListCreateInfoEXT::default().drm_format_modifiers(&layouts);
    let mut external = vk::ExternalMemoryImageCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .usage(usage)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .push_next(&mut external)
        .push_next(&mut listed);
    let image = unsafe { device.handle().create_image(&info, None)? };
    device.set_object_name(image, "rikka.dmabuf.exported");
    let mut partial = PartialImage::new(&device, image);
    let memory = allocate_exportable(&device, image, placement)?;
    partial.bound(ImageMemory::Imported(memory));
    unsafe { device.handle().bind_image_memory(image, memory, 0)? };

    let mut chosen = vk::ImageDrmFormatModifierPropertiesEXT::default();
    unsafe {
        dmabuf
            .modifiers
            .get_image_drm_format_modifier_properties(image, &mut chosen)?;
    }
    let modifier = chosen.drm_format_modifier;
    let planes = plane_layouts(&device, format, image, modifier);
    let fd_info = vk::MemoryGetFdInfoKHR::default()
        .memory(memory)
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let fd = unsafe { dmabuf.memory_fd.get_memory_fd(&fd_info)? };
    let fd = unsafe { OwnedFd::from_raw_fd(fd) };
    let planes = planes
        .into_iter()
        .map(|layout| {
            Ok(ExportedPlane {
                fd: fd.try_clone().map_err(|error| {
                    Error::Invalid(format!("failed to duplicate exported dmabuf: {error}"))
                })?,
                offset: plane_word(layout.offset)?,
                stride: plane_word(layout.row_pitch)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let image = GpuImage::finish_with(
        &device,
        partial,
        format,
        extent,
        vk::ComponentMapping::default(),
    )?;
    Ok(ExportedImage {
        image,
        readable,
        format: DrmFormat { fourcc, modifier },
        planes,
    })
}

fn layouts_for(
    device: &RenderDevice,
    format: vk::Format,
    offered: &[u64],
) -> Option<(vk::ImageUsageFlags, Vec<u64>, bool)> {
    let (instance, physical) = (device.instance(), device.physical());
    let among = |exportable: Vec<u64>| -> Vec<u64> {
        offered
            .iter()
            .copied()
            .filter(|modifier| exportable.contains(modifier))
            .collect()
    };
    let readable = among(exportable_modifiers(
        instance,
        physical,
        format,
        READABLE_RENDER_USE,
    ));
    if !readable.is_empty() {
        return Some((READABLE_RENDER_USE.usage, readable, true));
    }
    let drawable = among(exportable_modifiers(instance, physical, format, RENDER_USE));
    (!drawable.is_empty()).then_some((RENDER_USE.usage, drawable, false))
}

fn allocate_exportable(
    device: &RenderDevice,
    image: vk::Image,
    placement: Placement,
) -> Result<vk::DeviceMemory> {
    let info = vk::ImageMemoryRequirementsInfo2::default().image(image);
    let mut requirements = vk::MemoryRequirements2::default();
    unsafe {
        device
            .handle()
            .get_image_memory_requirements2(&info, &mut requirements);
    }
    let requirements = requirements.memory_requirements;
    let properties = unsafe {
        device
            .instance()
            .handle()
            .get_physical_device_memory_properties(device.physical())
    };
    let memory_type = memory_type(&properties, requirements.memory_type_bits, placement)
        .ok_or_else(|| Error::Unsupported("no memory type supports dmabuf export".into()))?;

    let mut export = vk::ExportMemoryAllocateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let mut dedicated = vk::MemoryDedicatedAllocateInfo::default().image(image);
    let allocate = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type)
        .push_next(&mut export)
        .push_next(&mut dedicated);
    Ok(unsafe { device.handle().allocate_memory(&allocate, None)? })
}

pub(crate) fn memory_type(
    properties: &vk::PhysicalDeviceMemoryProperties,
    allowed: u32,
    placement: Placement,
) -> Option<u32> {
    let types = &properties.memory_types[..properties.memory_type_count as usize];
    let allowed_types = || {
        types
            .iter()
            .enumerate()
            .filter(move |(index, _)| allowed & (1 << index) != 0)
    };
    let in_system_memory = |kind: &vk::MemoryType| {
        !properties.memory_heaps[kind.heap_index as usize]
            .flags
            .contains(vk::MemoryHeapFlags::DEVICE_LOCAL)
    };
    let chosen = |preferred: &dyn Fn(&vk::MemoryType) -> bool| {
        allowed_types()
            .find(|(_, kind)| preferred(kind))
            .map(|(index, _)| index as u32)
    };
    match placement {
        Placement::Device => chosen(&|kind| {
            kind.property_flags
                .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
        })
        .or_else(|| allowed_types().next().map(|(index, _)| index as u32)),
        Placement::System if types.iter().any(in_system_memory) => chosen(&in_system_memory),
        Placement::System => allowed_types().next().map(|(index, _)| index as u32),
    }
}

fn plane_layouts(
    device: &RenderDevice,
    format: vk::Format,
    image: vk::Image,
    modifier: u64,
) -> Vec<vk::SubresourceLayout> {
    let count = plane_count(device, format, modifier);
    const ASPECTS: [vk::ImageAspectFlags; 4] = [
        vk::ImageAspectFlags::MEMORY_PLANE_0_EXT,
        vk::ImageAspectFlags::MEMORY_PLANE_1_EXT,
        vk::ImageAspectFlags::MEMORY_PLANE_2_EXT,
        vk::ImageAspectFlags::MEMORY_PLANE_3_EXT,
    ];
    ASPECTS[..count.clamp(1, ASPECTS.len())]
        .iter()
        .map(|&aspect| unsafe {
            device.handle().get_image_subresource_layout(
                image,
                vk::ImageSubresource {
                    aspect_mask: aspect,
                    mip_level: 0,
                    array_layer: 0,
                },
            )
        })
        .collect()
}

fn plane_count(device: &RenderDevice, format: vk::Format, modifier: u64) -> usize {
    modifier_features(device.instance(), device.physical(), format)
        .iter()
        .find(|entry| entry.drm_format_modifier == modifier)
        .map_or(1, |entry| entry.drm_format_modifier_plane_count as usize)
}

fn plane_word(value: u64) -> Result<u32> {
    u32::try_from(value).map_err(|_| {
        Error::Invalid(format!(
            "exported dmabuf plane offset {value} is out of range"
        ))
    })
}

impl std::fmt::Debug for ExportedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportedImage").finish_non_exhaustive()
    }
}
