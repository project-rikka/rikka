use std::sync::Arc;

use ash::vk;

use crate::core::convert::{ConvertPlane, ConvertPush, DEEP_FORMAT, FX_FORMAT};
use crate::core::device::RenderDevice;
use crate::core::image::{ImageAccess, QueueTransfer, color_subresource_range, image_barrier};
use crate::drm::format::{PlanarLayout, SAMPLE_USE, TextureFormat, TextureLayout};
use crate::drm::image::{DmabufSpec, bind_whole_memory, create_image, import_memory};
use crate::drm::ycbcr::Ycbcr;
use crate::error::{Error, Result};

const PLANE_ASPECTS: [vk::ImageAspectFlags; 3] = [
    vk::ImageAspectFlags::PLANE_0,
    vk::ImageAspectFlags::PLANE_1,
    vk::ImageAspectFlags::PLANE_2,
];

pub(crate) struct PlanarDmabuf {
    device: Arc<RenderDevice>,
    image: vk::Image,
    memory: Vec<vk::DeviceMemory>,
    views: Vec<vk::ImageView>,
    layout: PlanarLayout,
    extent: vk::Extent2D,
    ycbcr: Ycbcr,
}

impl PlanarDmabuf {
    pub(crate) fn plane_views(&self) -> [ConvertPlane; 3] {
        let last = *self.views.last().expect("import has planes");
        std::array::from_fn(|index| ConvertPlane {
            view: self.views.get(index).copied().unwrap_or(last),
            linear: true,
        })
    }

    pub(crate) fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    pub(crate) fn convert_push(&self) -> ConvertPush {
        ConvertPush {
            rows: self.ycbcr.rows(self.layout.bit_depth),
            uv_scale: [1.0, 1.0],
            chroma_packed: self.layout.chroma_packed,
            chroma_swapped: self.layout.chroma_swapped,
            chroma_offset: self.ycbcr.chroma_offset(),
        }
    }

    pub(crate) fn converted_format(&self) -> vk::Format {
        match self.layout.bit_depth > 8 {
            true => DEEP_FORMAT,
            false => FX_FORMAT,
        }
    }

    pub(crate) fn acquire(&self) -> vk::ImageMemoryBarrier2<'static> {
        image_barrier(
            self.image,
            ImageAccess::FOREIGN_READY,
            ImageAccess::FRAGMENT_SAMPLED,
            QueueTransfer::acquire(self.device.queue_family()),
            color_subresource_range(),
        )
    }

    pub(crate) fn release(&self) -> vk::ImageMemoryBarrier2<'static> {
        image_barrier(
            self.image,
            ImageAccess::FRAGMENT_SAMPLED,
            ImageAccess::EXTERNAL,
            QueueTransfer::release(self.device.queue_family()),
            color_subresource_range(),
        )
    }
}

impl std::fmt::Debug for PlanarDmabuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlanarDmabuf")
            .field("width", &self.extent.width)
            .field("height", &self.extent.height)
            .field("planes", &self.views.len())
            .finish_non_exhaustive()
    }
}

impl Drop for PlanarDmabuf {
    fn drop(&mut self) {
        unsafe {
            for view in self.views.drain(..) {
                self.device.handle().destroy_image_view(view, None);
            }
            self.device.handle().destroy_image(self.image, None);
            for memory in self.memory.drain(..) {
                self.device.handle().free_memory(memory, None);
            }
        }
    }
}

pub(crate) fn import_planar(
    device: Arc<RenderDevice>,
    spec: &DmabufSpec<'_>,
    format: TextureFormat,
    ycbcr: Ycbcr,
) -> Result<PlanarDmabuf> {
    let TextureLayout::Planar(layout) = format.layout else {
        return Err(Error::Invalid("DRM format is not planar".to_owned()));
    };
    let modifier = spec.check_import(&device, format.image, SAMPLE_USE, layout.views)?;
    if spec.planes.len() != layout.views.len() {
        return Err(Error::Invalid(format!(
            "DRM format {:#010x} has {} planes, not {}",
            spec.fourcc,
            layout.views.len(),
            spec.planes.len()
        )));
    }
    let disjoint = spec.is_disjoint(&modifier);

    let image = create_image(
        &device,
        spec,
        format.image,
        SAMPLE_USE.usage,
        layout.views,
        disjoint,
    )?;
    device.set_object_name(image, "rikka.dmabuf.planar");
    let mut import = PlanarDmabuf {
        device: device.clone(),
        image,
        memory: Vec::new(),
        views: Vec::new(),
        layout,
        extent: spec.extent(),
        ycbcr,
    };

    if disjoint {
        for (index, plane) in spec.planes.iter().enumerate() {
            let mut plane_info =
                vk::ImagePlaneMemoryRequirementsInfo::default().plane_aspect(PLANE_ASPECTS[index]);
            let info = vk::ImageMemoryRequirementsInfo2::default()
                .image(image)
                .push_next(&mut plane_info);
            let mut requirements = vk::MemoryRequirements2::default();
            unsafe {
                device
                    .handle()
                    .get_image_memory_requirements2(&info, &mut requirements);
            }
            let memory = import_memory(&device, plane.fd, requirements.memory_requirements, None)?;
            import.memory.push(memory);
        }

        let mut plane_infos: Vec<vk::BindImagePlaneMemoryInfo> = (0..spec.planes.len())
            .map(|index| vk::BindImagePlaneMemoryInfo::default().plane_aspect(PLANE_ASPECTS[index]))
            .collect();
        let binds: Vec<vk::BindImageMemoryInfo<'_>> = plane_infos
            .iter_mut()
            .zip(&import.memory)
            .map(|(plane, memory)| {
                vk::BindImageMemoryInfo::default()
                    .image(image)
                    .memory(*memory)
                    .push_next(plane)
            })
            .collect();
        unsafe { device.handle().bind_image_memory2(&binds)? };
    } else {
        import
            .memory
            .push(bind_whole_memory(&device, image, spec.planes[0].fd)?);
    }

    for (index, view_format) in layout.views.iter().enumerate() {
        let info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(*view_format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: PLANE_ASPECTS[index],
                ..color_subresource_range()
            });
        let view = unsafe { device.handle().create_image_view(&info, None)? };
        device.set_object_name(view, "rikka.dmabuf.planar.plane");
        import.views.push(view);
    }

    Ok(import)
}
