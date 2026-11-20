use std::mem::ManuallyDrop;
use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
#[cfg(unix)]
use crate::core::image::access::color_subresource_range;
use crate::core::image::access::{
    ImageAccess, QueueTransfer, WRITE_ACCESS, color_subresource_layers, image_barrier,
    record_barriers,
};
use crate::core::image::desc::{ImageDesc, validate_extent};
use crate::core::image::mips::Texels;
use crate::core::image::recording::{LayoutState, Tracking};
use crate::core::memory::Allocation;
use crate::error::Result;

pub(crate) enum ImageMemory {
    Allocated(ManuallyDrop<Allocation>),
    #[cfg(unix)]
    Imported(vk::DeviceMemory),
    Placed,
}

pub(crate) struct PartialImage<'a> {
    device: &'a RenderDevice,
    image: vk::Image,
    memory: Option<ImageMemory>,
}

impl<'a> PartialImage<'a> {
    pub(crate) fn new(device: &'a RenderDevice, image: vk::Image) -> Self {
        Self {
            device,
            image,
            memory: None,
        }
    }

    pub(crate) fn bound(&mut self, memory: ImageMemory) {
        self.memory = Some(memory);
    }

    fn into_memory(mut self) -> ImageMemory {
        let memory = self.memory.take().expect("image memory is bound");
        std::mem::forget(self);
        memory
    }
}

impl Drop for PartialImage<'_> {
    fn drop(&mut self) {
        unsafe { self.device.handle().destroy_image(self.image, None) };
        match self.memory.take() {
            Some(ImageMemory::Allocated(allocation)) => {
                self.device.free(ManuallyDrop::into_inner(allocation))
            }
            #[cfg(unix)]
            Some(ImageMemory::Imported(memory)) => unsafe {
                self.device.handle().free_memory(memory, None)
            },
            Some(ImageMemory::Placed) | None => {}
        }
    }
}

pub struct GpuImage {
    device: Arc<RenderDevice>,
    image: vk::Image,
    view: vk::ImageView,
    format: vk::Format,
    extent: vk::Extent2D,
    range: vk::ImageSubresourceRange,
    mip_views: Vec<vk::ImageView>,
    memory: ManuallyDrop<ImageMemory>,
    tracking: Tracking,
}

impl GpuImage {
    pub fn new(
        device: Arc<RenderDevice>,
        name: &str,
        extent: vk::Extent2D,
        format: vk::Format,
        usage: vk::ImageUsageFlags,
    ) -> Result<Self> {
        Self::create(device, name, &ImageDesc::color(extent, format, usage))
    }

    pub fn create(device: Arc<RenderDevice>, name: &str, desc: &ImageDesc) -> Result<Self> {
        validate_extent(&device, desc.extent)?;
        let families = desc.shared_by.unwrap_or_default();
        let image = unsafe {
            device
                .handle()
                .create_image(&desc.create_info(&families), None)?
        };
        let mut partial = PartialImage::new(&device, image);

        let allocation = device.allocate_image(name, image, desc.usage)?;
        let (memory, offset) = (allocation.memory(), allocation.offset());
        partial.bound(ImageMemory::Allocated(ManuallyDrop::new(allocation)));
        unsafe { device.handle().bind_image_memory(image, memory, offset)? };
        Self::finish_created(device.clone(), name, desc, partial)
    }

    pub fn requirements(device: &RenderDevice, desc: &ImageDesc) -> vk::MemoryRequirements {
        let families = desc.shared_by.unwrap_or_default();
        let info = desc.create_info(&families);
        let query = vk::DeviceImageMemoryRequirements::default().create_info(&info);
        let mut requirements = vk::MemoryRequirements2::default();
        unsafe {
            device
                .handle()
                .get_device_image_memory_requirements(&query, &mut requirements)
        };
        requirements.memory_requirements
    }

    pub fn placed(
        device: Arc<RenderDevice>,
        name: &str,
        desc: &ImageDesc,
        memory: vk::DeviceMemory,
        offset: u64,
    ) -> Result<Self> {
        validate_extent(&device, desc.extent)?;
        let families = desc.shared_by.unwrap_or_default();
        let image = unsafe {
            device
                .handle()
                .create_image(&desc.create_info(&families), None)?
        };
        let mut partial = PartialImage::new(&device, image);
        partial.bound(ImageMemory::Placed);
        unsafe { device.handle().bind_image_memory(image, memory, offset)? };
        Self::finish_created(device.clone(), name, desc, partial)
    }

    fn finish_created(
        device: Arc<RenderDevice>,
        name: &str,
        desc: &ImageDesc,
        partial: PartialImage<'_>,
    ) -> Result<Self> {
        let image = partial.image;
        device.set_object_name(image, name);
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(desc.view_type)
            .format(desc.format)
            .components(desc.components)
            .subresource_range(desc.range());
        let view = unsafe { device.handle().create_image_view(&view_info, None)? };
        device.set_object_name(view, name);

        Ok(Self {
            device: device.clone(),
            image,
            view,
            format: desc.format,
            extent: desc.extent,
            range: desc.range(),
            mip_views: Vec::new(),
            memory: ManuallyDrop::new(partial.into_memory()),
            tracking: Tracking::fresh(),
        })
    }

    #[cfg(unix)]
    pub(crate) fn finish_with(
        device: &Arc<RenderDevice>,
        partial: PartialImage<'_>,
        format: vk::Format,
        extent: vk::Extent2D,
        components: vk::ComponentMapping,
    ) -> Result<Self> {
        let image = partial.image;
        let view_info = vk::ImageViewCreateInfo::default()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(format)
            .components(components)
            .subresource_range(color_subresource_range());
        let view = unsafe { device.handle().create_image_view(&view_info, None)? };

        Ok(Self {
            device: device.clone(),
            image,
            view,
            format,
            extent,
            range: color_subresource_range(),
            mip_views: Vec::new(),
            memory: ManuallyDrop::new(partial.into_memory()),
            tracking: Tracking::fresh(),
        })
    }

    pub fn device(&self) -> &Arc<RenderDevice> {
        &self.device
    }

    pub fn handle(&self) -> vk::Image {
        self.image
    }

    pub fn view(&self) -> vk::ImageView {
        self.view
    }

    pub fn format(&self) -> vk::Format {
        self.format
    }

    pub fn extent(&self) -> vk::Extent2D {
        self.extent
    }

    pub(crate) fn mip_levels(&self) -> u32 {
        self.range.level_count
    }

    pub(crate) fn subresource_range(&self) -> vk::ImageSubresourceRange {
        self.range
    }

    fn mip_extent(&self, level: u32) -> vk::Extent2D {
        vk::Extent2D {
            width: (self.extent.width >> level).max(1),
            height: (self.extent.height >> level).max(1),
        }
    }

    pub fn mip_view(&mut self, level: u32) -> Result<vk::ImageView> {
        if self.mip_views.is_empty() {
            self.mip_views = vec![vk::ImageView::null(); self.range.level_count as usize];
        }
        let slot = &mut self.mip_views[level as usize];
        if *slot != vk::ImageView::null() {
            return Ok(*slot);
        }
        let info = vk::ImageViewCreateInfo::default()
            .image(self.image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(self.format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask: self.range.aspect_mask,
                base_mip_level: level,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            });
        let view = unsafe { self.device.handle().create_image_view(&info, None)? };
        *slot = view;
        Ok(view)
    }

    pub fn barrier_to(&mut self, cmd: vk::CommandBuffer, next: ImageAccess) {
        if let Some(barrier) = self.transition(next) {
            record_barriers(&self.device, cmd, &[barrier]);
        }
    }

    pub fn transition(&mut self, next: ImageAccess) -> Option<vk::ImageMemoryBarrier2<'static>> {
        let current = self.tracking.current();
        if current.keeps && current.state == next && !next.access.intersects(WRITE_ACCESS) {
            return None;
        }
        let mut from = current.state;
        if !current.keeps {
            from.layout = vk::ImageLayout::UNDEFINED;
        }
        let barrier = image_barrier(self.image, from, next, QueueTransfer::NONE, self.range);
        self.settle(next);
        Some(barrier)
    }

    pub fn discard(&mut self) {
        let current = self.tracking.current();
        self.tracking.set(LayoutState {
            keeps: false,
            ..current
        });
    }

    pub fn access(&self) -> ImageAccess {
        self.tracking.current().state
    }

    pub fn holds_content(&self) -> bool {
        self.tracking.current().keeps
    }

    pub(crate) fn read_out(&self) -> vk::ImageMemoryBarrier2<'static> {
        image_barrier(
            self.image,
            self.access(),
            ImageAccess::TRANSFER_SRC,
            QueueTransfer::NONE,
            self.range,
        )
    }

    pub fn set_access(&mut self, access: ImageAccess) {
        self.settle(access);
    }

    pub(crate) fn take_over(&mut self) -> vk::ImageMemoryBarrier2<'static> {
        let barrier = image_barrier(
            self.image,
            ImageAccess::FOREIGN_READY,
            ImageAccess::TABLE_SAMPLED,
            QueueTransfer::acquire(self.device.queue_family()),
            self.range,
        );
        self.settle(ImageAccess::TABLE_SAMPLED);
        barrier
    }

    pub(crate) fn release_barrier(&mut self) -> vk::ImageMemoryBarrier2<'static> {
        let barrier = image_barrier(
            self.image,
            self.access(),
            ImageAccess::EXTERNAL,
            QueueTransfer::release(self.device.queue_family()),
            self.range,
        );
        self.settle(ImageAccess::EXTERNAL);
        barrier
    }

    pub(super) fn settle(&mut self, state: ImageAccess) {
        self.tracking.set(LayoutState { state, keeps: true });
    }

    pub(super) fn level_barrier(
        &self,
        levels: std::ops::Range<u32>,
        current: ImageAccess,
        next: ImageAccess,
    ) -> vk::ImageMemoryBarrier2<'static> {
        image_barrier(
            self.image,
            current,
            next,
            QueueTransfer::NONE,
            vk::ImageSubresourceRange {
                base_mip_level: levels.start,
                level_count: levels.end - levels.start,
                ..self.range
            },
        )
    }

    pub(super) fn blit_down(&self, cmd: vk::CommandBuffer, level: u32, from: Texels) -> Texels {
        let (source, down) = from.below(self.mip_extent(level - 1));
        let layers = |mip_level| vk::ImageSubresourceLayers {
            mip_level,
            ..color_subresource_layers()
        };
        let blit = vk::ImageBlit2::default()
            .src_subresource(layers(level - 1))
            .src_offsets(source.corners())
            .dst_subresource(layers(level))
            .dst_offsets(down.corners());
        let blits = [blit];
        let info = vk::BlitImageInfo2::default()
            .src_image(self.image)
            .src_image_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
            .dst_image(self.image)
            .dst_image_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
            .regions(&blits)
            .filter(vk::Filter::LINEAR);
        unsafe { self.device.handle().cmd_blit_image2(cmd, &info) };
        down
    }
}

impl Drop for GpuImage {
    fn drop(&mut self) {
        unsafe {
            for &view in &self.mip_views {
                if view != vk::ImageView::null() {
                    self.device.handle().destroy_image_view(view, None);
                }
            }
            self.device.handle().destroy_image_view(self.view, None);
            self.device.handle().destroy_image(self.image, None);
            match ManuallyDrop::take(&mut self.memory) {
                ImageMemory::Allocated(allocation) => {
                    self.device.free(ManuallyDrop::into_inner(allocation))
                }
                #[cfg(unix)]
                ImageMemory::Imported(memory) => self.device.handle().free_memory(memory, None),
                ImageMemory::Placed => {}
            }
        }
    }
}

impl std::fmt::Debug for GpuImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuImage").finish_non_exhaustive()
    }
}
