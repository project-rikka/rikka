use std::ptr;
use std::sync::Arc;

use ash::vk;

use crate::capabilities::Capabilities;
use crate::device::VideoDevice;
use crate::error::Result;
use crate::profile::Profile;

pub(crate) struct Image {
    device: Arc<VideoDevice>,
    pub(crate) image: vk::Image,
    memory: vk::DeviceMemory,
    pub(crate) views: Vec<vk::ImageView>,
}

pub(crate) struct ImageRequest<'a> {
    pub(crate) profile: &'a Profile,
    pub(crate) format: vk::Format,
    pub(crate) extent: vk::Extent2D,
    pub(crate) layers: u32,
    pub(crate) usage: vk::ImageUsageFlags,
    pub(crate) view_usage: vk::ImageUsageFlags,
    pub(crate) flags: vk::ImageCreateFlags,
    pub(crate) shared: bool,
}

impl Image {
    pub(crate) fn new(device: &Arc<VideoDevice>, request: &ImageRequest<'_>) -> Result<Image> {
        let families = device.families();
        let mut info = vk::ImageCreateInfo::default()
            .flags(request.flags)
            .image_type(vk::ImageType::TYPE_2D)
            .format(request.format)
            .extent(vk::Extent3D {
                width: request.extent.width,
                height: request.extent.height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(request.layers)
            .samples(vk::SampleCountFlags::TYPE_1)
            .tiling(vk::ImageTiling::OPTIMAL)
            .usage(request.usage)
            .initial_layout(vk::ImageLayout::UNDEFINED);
        info = match request.shared && families.len() > 1 {
            true => info
                .sharing_mode(vk::SharingMode::CONCURRENT)
                .queue_family_indices(&families),
            false => info.sharing_mode(vk::SharingMode::EXCLUSIVE),
        };
        info.p_next = ptr::from_ref(request.profile.list()).cast();
        let image = unsafe { device.device.create_image(&info, None) }?;
        let requirements = unsafe { device.device.get_image_memory_requirements(image) };
        let memory = match device.allocate(
            requirements,
            vk::MemoryPropertyFlags::DEVICE_LOCAL,
            vk::MemoryPropertyFlags::empty(),
        ) {
            Ok(memory) => memory,
            Err(error) => {
                unsafe { device.device.destroy_image(image, None) };
                return Err(error);
            }
        };
        let mut made = Image {
            device: Arc::clone(device),
            image,
            memory,
            views: Vec::new(),
        };
        unsafe { device.device.bind_image_memory(image, memory, 0) }?;
        for layer in 0..request.layers {
            let mut usage = vk::ImageViewUsageCreateInfo::default().usage(request.view_usage);
            let view = vk::ImageViewCreateInfo::default()
                .image(image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(request.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: layer,
                    layer_count: 1,
                })
                .push_next(&mut usage);
            made.views
                .push(unsafe { device.device.create_image_view(&view, None) }?);
        }
        Ok(made)
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            for view in self.views.drain(..) {
                self.device.device.destroy_image_view(view, None);
            }
            self.device.device.destroy_image(self.image, None);
            self.device.device.free_memory(self.memory, None);
        }
    }
}

pub(crate) struct Slot {
    pub(crate) view: vk::ImageView,
}

pub(crate) struct Dpb {
    images: Vec<Image>,
    pub(crate) slots: Vec<Slot>,
}

impl Dpb {
    pub(crate) fn new(
        device: &Arc<VideoDevice>,
        capabilities: &Capabilities,
        profile: &Profile,
        extent: vk::Extent2D,
        count: u32,
    ) -> Result<Dpb> {
        let request = |layers| ImageRequest {
            profile,
            format: capabilities.reference_format,
            extent,
            layers,
            usage: vk::ImageUsageFlags::VIDEO_ENCODE_DPB_KHR,
            view_usage: vk::ImageUsageFlags::VIDEO_ENCODE_DPB_KHR,
            flags: vk::ImageCreateFlags::empty(),
            shared: false,
        };
        let mut images = Vec::new();
        let mut slots = Vec::new();
        match capabilities.separate_references {
            true => {
                for _ in 0..count {
                    let image = Image::new(device, &request(1))?;
                    slots.push(Slot {
                        view: image.views[0],
                    });
                    images.push(image);
                }
            }
            false => {
                let image = Image::new(device, &request(count))?;
                for layer in 0..count {
                    slots.push(Slot {
                        view: image.views[layer as usize],
                    });
                }
                images.push(image);
            }
        }
        Ok(Dpb { images, slots })
    }

    pub(crate) fn images(&self) -> impl Iterator<Item = vk::Image> + '_ {
        self.images.iter().map(|image| image.image)
    }
}
