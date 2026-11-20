use std::os::fd::{AsRawFd, BorrowedFd, IntoRawFd};
use std::os::unix::fs::MetadataExt;
use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::image::{GpuImage, ImageMemory, PartialImage};
use crate::drm::format::{
    ImportUse, ModifierProperties, READABLE_RENDER_USE, RENDER_USE, SAMPLE_USE, TextureFormat,
    TextureLayout, modifier_properties, mutable_flags, vk_format,
};
use crate::error::{Error, Result};

pub(crate) const MAX_PLANES: usize = 4;

#[derive(Clone, Copy, Debug)]
pub struct DmabufPlane<'a> {
    pub offset: u32,
    pub stride: u32,
    pub fd: BorrowedFd<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct DmabufSpec<'a> {
    pub width: u32,
    pub height: u32,
    pub fourcc: u32,
    pub modifier: u64,
    pub planes: &'a [DmabufPlane<'a>],
}

impl DmabufSpec<'_> {
    fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Err(Error::Invalid(format!(
                "dmabuf size {}x{} is empty",
                self.width, self.height
            )));
        }
        if self.planes.is_empty() || self.planes.len() > MAX_PLANES {
            return Err(Error::Invalid(format!(
                "dmabuf has {} planes, expected 1 to {MAX_PLANES}",
                self.planes.len()
            )));
        }
        Ok(())
    }

    pub(crate) fn extent(&self) -> vk::Extent2D {
        vk::Extent2D {
            width: self.width,
            height: self.height,
        }
    }

    pub(crate) fn check_import(
        &self,
        device: &RenderDevice,
        format: vk::Format,
        purpose: ImportUse,
        views: &[vk::Format],
    ) -> Result<ModifierProperties> {
        self.validate()?;
        if device.dmabuf().is_none() {
            return Err(Error::Unsupported(
                "device does not support dmabuf import".into(),
            ));
        }
        let properties = modifier_properties(
            device.instance(),
            device.physical(),
            format,
            self.modifier,
            purpose,
            views,
        )
        .ok_or_else(|| {
            Error::Unsupported(format!(
                "device cannot import DRM format {:#010x} with modifier {:#018x} for {:?}",
                self.fourcc, self.modifier, purpose.usage
            ))
        })?;
        if properties.planes as usize != self.planes.len() {
            return Err(Error::Invalid(format!(
                "modifier {:#018x} gives DRM format {:#010x} {} planes, got {}",
                self.modifier,
                self.fourcc,
                properties.planes,
                self.planes.len()
            )));
        }
        let largest = properties.max_extent;
        if self.width > largest.width || self.height > largest.height {
            return Err(Error::Invalid(format!(
                "dmabuf size {}x{} exceeds the device import limit of {}x{}",
                self.width, self.height, largest.width, largest.height
            )));
        }
        if !self.is_disjoint(&properties) {
            self.check_one_buffer()?;
        }
        Ok(properties)
    }

    pub(crate) fn is_disjoint(&self, properties: &ModifierProperties) -> bool {
        self.planes.len() > 1 && properties.disjoint
    }

    fn check_one_buffer(&self) -> Result<()> {
        let Some((first, rest)) = self.planes.split_first() else {
            return Ok(());
        };
        if rest.is_empty() {
            return Ok(());
        }
        let buffer = buffer_identity(first.fd)?;
        for plane in rest {
            if buffer_identity(plane.fd)? != buffer {
                return Err(Error::Invalid(
                    "dmabuf planes use separate buffers but the layout is not disjoint".into(),
                ));
            }
        }
        Ok(())
    }
}

fn buffer_identity(fd: BorrowedFd<'_>) -> Result<(u64, u64)> {
    let file = std::fs::File::from(
        fd.try_clone_to_owned()
            .map_err(|err| Error::Invalid(format!("failed to duplicate dmabuf fd: {err}")))?,
    );
    let metadata = file
        .metadata()
        .map_err(|err| Error::Invalid(format!("failed to stat dmabuf fd: {err}")))?;
    Ok((metadata.dev(), metadata.ino()))
}

pub fn import_render(device: Arc<RenderDevice>, spec: &DmabufSpec<'_>) -> Result<(GpuImage, bool)> {
    let format = vk_format(spec.fourcc).ok_or_else(|| {
        Error::Unsupported(format!("unsupported DRM format {:#010x}", spec.fourcc))
    })?;
    let readable = spec
        .check_import(&device, format, READABLE_RENDER_USE, &[])
        .is_ok();
    let purpose = match readable {
        true => READABLE_RENDER_USE,
        false => RENDER_USE,
    };
    let image = import_single(
        device,
        spec,
        format,
        purpose,
        vk::ComponentMapping::default(),
        "rikka.dmabuf.render",
    )?;
    Ok((image, readable))
}

pub(crate) fn import_color(
    device: Arc<RenderDevice>,
    spec: &DmabufSpec<'_>,
    format: TextureFormat,
) -> Result<GpuImage> {
    let TextureLayout::Color { opaque } = format.layout else {
        return Err(Error::Invalid(
            "planar dmabuf cannot be sampled as a single image".into(),
        ));
    };
    let components = if opaque {
        vk::ComponentMapping::default().a(vk::ComponentSwizzle::ONE)
    } else {
        vk::ComponentMapping::default()
    };
    import_single(
        device,
        spec,
        format.image,
        SAMPLE_USE,
        components,
        "rikka.dmabuf.texture",
    )
}

fn import_single(
    device: Arc<RenderDevice>,
    spec: &DmabufSpec<'_>,
    format: vk::Format,
    purpose: ImportUse,
    components: vk::ComponentMapping,
    name: &str,
) -> Result<GpuImage> {
    spec.check_import(&device, format, purpose, &[])?;
    let image = create_image(&device, spec, format, purpose.usage, &[], false)?;
    device.set_object_name(image, name);
    let mut partial = PartialImage::new(&device, image);
    let memory = bind_whole_memory(&device, image, spec.planes[0].fd)?;
    partial.bound(ImageMemory::Imported(memory));
    GpuImage::finish_with(&device, partial, format, spec.extent(), components)
}

pub(crate) fn bind_whole_memory(
    device: &RenderDevice,
    image: vk::Image,
    fd: BorrowedFd<'_>,
) -> Result<vk::DeviceMemory> {
    let info = vk::ImageMemoryRequirementsInfo2::default().image(image);
    let mut requirements = vk::MemoryRequirements2::default();
    unsafe {
        device
            .handle()
            .get_image_memory_requirements2(&info, &mut requirements);
    }
    let memory = import_memory(device, fd, requirements.memory_requirements, Some(image))?;
    if let Err(err) = unsafe { device.handle().bind_image_memory(image, memory, 0) } {
        unsafe { device.handle().free_memory(memory, None) };
        return Err(err.into());
    }
    Ok(memory)
}

pub(crate) fn create_image(
    device: &RenderDevice,
    spec: &DmabufSpec<'_>,
    format: vk::Format,
    usage: vk::ImageUsageFlags,
    view_formats: &[vk::Format],
    disjoint: bool,
) -> Result<vk::Image> {
    let plane_layouts: Vec<vk::SubresourceLayout> = spec
        .planes
        .iter()
        .map(|plane| vk::SubresourceLayout {
            offset: plane.offset as u64,
            size: 0,
            row_pitch: plane.stride as u64,
            array_pitch: 0,
            depth_pitch: 0,
        })
        .collect();

    let mut flags = mutable_flags(view_formats);
    if disjoint {
        flags |= vk::ImageCreateFlags::DISJOINT;
    }

    let mut modifier_info = vk::ImageDrmFormatModifierExplicitCreateInfoEXT::default()
        .drm_format_modifier(spec.modifier)
        .plane_layouts(&plane_layouts);
    let mut external = vk::ExternalMemoryImageCreateInfo::default()
        .handle_types(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT);
    let mut list = vk::ImageFormatListCreateInfo::default().view_formats(view_formats);
    let mut info = vk::ImageCreateInfo::default()
        .image_type(vk::ImageType::TYPE_2D)
        .format(format)
        .extent(vk::Extent3D {
            width: spec.width,
            height: spec.height,
            depth: 1,
        })
        .mip_levels(1)
        .array_layers(1)
        .samples(vk::SampleCountFlags::TYPE_1)
        .tiling(vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT)
        .usage(usage)
        .flags(flags)
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED)
        .push_next(&mut external)
        .push_next(&mut modifier_info);
    if !view_formats.is_empty() {
        info = info.push_next(&mut list);
    }

    Ok(unsafe { device.handle().create_image(&info, None)? })
}

pub(crate) fn import_memory(
    device: &RenderDevice,
    fd: BorrowedFd<'_>,
    requirements: vk::MemoryRequirements,
    dedicated: Option<vk::Image>,
) -> Result<vk::DeviceMemory> {
    let dmabuf = device
        .dmabuf()
        .ok_or_else(|| Error::Unsupported("device does not support dmabuf import".into()))?;

    let mut fd_properties = vk::MemoryFdPropertiesKHR::default();
    unsafe {
        dmabuf.memory_fd.get_memory_fd_properties(
            vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
            fd.as_raw_fd(),
            &mut fd_properties,
        )?;
    }
    let usable = requirements.memory_type_bits & fd_properties.memory_type_bits;
    let memory_type = (usable != 0)
        .then(|| usable.trailing_zeros())
        .ok_or_else(|| Error::Unsupported("no memory type supports this dmabuf".into()))?;

    let duplicate = fd
        .try_clone_to_owned()
        .map_err(|err| Error::Invalid(format!("failed to duplicate dmabuf fd: {err}")))?;
    let mut import = vk::ImportMemoryFdInfoKHR::default()
        .handle_type(vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT)
        .fd(duplicate.as_raw_fd());
    let mut dedicated_info = vk::MemoryDedicatedAllocateInfo::default();
    let mut allocate = vk::MemoryAllocateInfo::default()
        .allocation_size(requirements.size)
        .memory_type_index(memory_type)
        .push_next(&mut import);
    if let Some(image) = dedicated {
        dedicated_info = dedicated_info.image(image);
        allocate = allocate.push_next(&mut dedicated_info);
    }

    let memory = unsafe { device.handle().allocate_memory(&allocate, None)? };
    let _ = duplicate.into_raw_fd();
    Ok(memory)
}
