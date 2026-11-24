use ash::vk;
use rikka_gpu::{ExternalDevice, ExternalHandles, HostSubmit};
use rikka_renderer::{ExternalImage, Transform};

pub(super) const SIDE: u32 = 64;

const FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;

struct HostQueue {
    device: ash::Device,
    queue: vk::Queue,
}

impl HostSubmit for HostQueue {
    fn submit(&self, batches: &[vk::SubmitInfo2<'_>]) -> Result<(), vk::Result> {
        unsafe {
            self.device
                .queue_submit2(self.queue, batches, vk::Fence::null())
        }
    }
}

pub(super) struct Host {
    entry: ash::Entry,
    instance: ash::Instance,
    physical: vk::PhysicalDevice,
    device: ash::Device,
    queue: vk::Queue,
    family: u32,
    pool: vk::CommandPool,
    image: vk::Image,
    image_memory: vk::DeviceMemory,
    view: vk::ImageView,
    buffer: vk::Buffer,
    buffer_memory: vk::DeviceMemory,
}

impl Host {
    pub(super) fn new() -> Host {
        let entry = unsafe { ash::Entry::load() }.expect("a Vulkan loader is required");
        let application = vk::ApplicationInfo::default().api_version(vk::API_VERSION_1_3);
        let instance = unsafe {
            entry.create_instance(
                &vk::InstanceCreateInfo::default().application_info(&application),
                None,
            )
        }
        .expect("a Vulkan 1.3 instance is required");
        let (physical, family) = unsafe { instance.enumerate_physical_devices() }
            .unwrap()
            .into_iter()
            .filter(|physical| {
                let properties = unsafe { instance.get_physical_device_properties(*physical) };
                properties.api_version >= vk::API_VERSION_1_3
            })
            .find_map(|physical| {
                let families =
                    unsafe { instance.get_physical_device_queue_family_properties(physical) };
                families
                    .iter()
                    .position(|family| family.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                    .map(|family| (physical, family as u32))
            })
            .expect("a Vulkan 1.3 GPU with a graphics queue is required");
        let mut features12 = enabled12();
        let mut features13 = enabled13();
        let priorities = [1.0];
        let queues = [vk::DeviceQueueCreateInfo::default()
            .queue_family_index(family)
            .queue_priorities(&priorities)];
        let device = unsafe {
            instance.create_device(
                physical,
                &vk::DeviceCreateInfo::default()
                    .queue_create_infos(&queues)
                    .push_next(&mut features12)
                    .push_next(&mut features13),
                None,
            )
        }
        .unwrap();
        let queue = unsafe { device.get_device_queue(family, 0) };
        let pool = unsafe {
            device.create_command_pool(
                &vk::CommandPoolCreateInfo::default().queue_family_index(family),
                None,
            )
        }
        .unwrap();
        let mut host = Host {
            entry,
            instance,
            physical,
            device,
            queue,
            family,
            pool,
            image: vk::Image::null(),
            image_memory: vk::DeviceMemory::null(),
            view: vk::ImageView::null(),
            buffer: vk::Buffer::null(),
            buffer_memory: vk::DeviceMemory::null(),
        };
        host.make_image();
        host.make_buffer();
        host
    }

    pub(super) fn device(&self) -> ExternalDevice {
        let mut features12 = enabled12();
        let mut features13 = enabled13();
        let features = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut features12)
            .push_next(&mut features13);
        unsafe { self.bare_device().enabled_features(&features) }
    }

    pub(super) fn bare_device(&self) -> ExternalDevice {
        let handles = ExternalHandles {
            get_instance_proc_addr: self.entry.static_fn().get_instance_proc_addr,
            instance: self.instance.handle(),
            api_version: vk::API_VERSION_1_3,
            physical_device: self.physical,
            device: self.device.handle(),
            queue: self.queue,
            queue_family: self.family,
        };
        let queue = HostQueue {
            device: self.device.clone(),
            queue: self.queue,
        };
        unsafe { ExternalDevice::new(handles, queue) }
    }

    pub(super) fn image(&self) -> ExternalImage {
        ExternalImage {
            image: self.image,
            view: self.view,
            format: FORMAT,
            width: SIDE,
            height: SIDE,
            layout: vk::ImageLayout::GENERAL,
            transform: Transform::Normal,
        }
    }

    pub(super) fn paint_halves(&self, left: [f32; 4], right: [f32; 4]) {
        self.record(|device, commands| {
            let into_general = [vk::ImageMemoryBarrier2::default()
                .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                .src_access_mask(vk::AccessFlags2::MEMORY_WRITE)
                .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                .dst_access_mask(vk::AccessFlags2::MEMORY_READ | vk::AccessFlags2::MEMORY_WRITE)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::GENERAL)
                .image(self.image)
                .subresource_range(color_range())];
            unsafe {
                device.cmd_pipeline_barrier2(
                    commands,
                    &vk::DependencyInfo::default().image_memory_barriers(&into_general),
                );
                device.cmd_clear_color_image(
                    commands,
                    self.image,
                    vk::ImageLayout::GENERAL,
                    &vk::ClearColorValue { float32: left },
                    &[color_range()],
                );
            }
            self.fill_right(device, commands, vk::ClearColorValue { float32: right });
            flush(device, commands);
        });
    }

    fn fill_right(
        &self,
        device: &ash::Device,
        commands: vk::CommandBuffer,
        clear: vk::ClearColorValue,
    ) {
        let half = SIDE / 2;
        flush(device, commands);
        let attachment = [vk::RenderingAttachmentInfo::default()
            .image_view(self.view)
            .image_layout(vk::ImageLayout::GENERAL)
            .load_op(vk::AttachmentLoadOp::LOAD)
            .store_op(vk::AttachmentStoreOp::STORE)];
        let area = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: vk::Extent2D {
                width: SIDE,
                height: SIDE,
            },
        };
        let rendering = vk::RenderingInfo::default()
            .render_area(area)
            .layer_count(1)
            .color_attachments(&attachment);
        let cleared = [vk::ClearAttachment::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .color_attachment(0)
            .clear_value(vk::ClearValue { color: clear })];
        let rects = [vk::ClearRect {
            rect: vk::Rect2D {
                offset: vk::Offset2D {
                    x: half as i32,
                    y: 0,
                },
                extent: vk::Extent2D {
                    width: SIDE - half,
                    height: SIDE,
                },
            },
            base_array_layer: 0,
            layer_count: 1,
        }];
        unsafe {
            device.cmd_begin_rendering(commands, &rendering);
            device.cmd_clear_attachments(commands, &cleared, &rects);
            device.cmd_end_rendering(commands);
        }
    }

    pub(super) fn read(&self) -> Vec<u8> {
        self.record(|device, commands| {
            flush(device, commands);
            let region = [vk::BufferImageCopy::default()
                .image_subresource(vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                })
                .image_extent(vk::Extent3D {
                    width: SIDE,
                    height: SIDE,
                    depth: 1,
                })];
            unsafe {
                device.cmd_copy_image_to_buffer(
                    commands,
                    self.image,
                    vk::ImageLayout::GENERAL,
                    self.buffer,
                    &region,
                );
            }
            flush(device, commands);
        });
        let size = (SIDE * SIDE * 4) as usize;
        let mapped = unsafe {
            self.device.map_memory(
                self.buffer_memory,
                0,
                size as u64,
                vk::MemoryMapFlags::empty(),
            )
        }
        .unwrap();
        let pixels = unsafe { std::slice::from_raw_parts(mapped.cast::<u8>(), size) }.to_vec();
        unsafe { self.device.unmap_memory(self.buffer_memory) };
        pixels
    }

    fn record(&self, record: impl FnOnce(&ash::Device, vk::CommandBuffer)) {
        let commands = unsafe {
            self.device.allocate_command_buffers(
                &vk::CommandBufferAllocateInfo::default()
                    .command_pool(self.pool)
                    .command_buffer_count(1),
            )
        }
        .unwrap()[0];
        unsafe {
            self.device
                .begin_command_buffer(commands, &vk::CommandBufferBeginInfo::default())
                .unwrap();
        }
        record(&self.device, commands);
        let buffers = [vk::CommandBufferSubmitInfo::default().command_buffer(commands)];
        let submit = [vk::SubmitInfo2::default().command_buffer_infos(&buffers)];
        unsafe {
            self.device.end_command_buffer(commands).unwrap();
            self.device
                .queue_submit2(self.queue, &submit, vk::Fence::null())
                .unwrap();
            self.device.queue_wait_idle(self.queue).unwrap();
            self.device.free_command_buffers(self.pool, &[commands]);
        }
    }

    fn make_image(&mut self) {
        let image = unsafe {
            self.device.create_image(
                &vk::ImageCreateInfo::default()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(FORMAT)
                    .extent(vk::Extent3D {
                        width: SIDE,
                        height: SIDE,
                        depth: 1,
                    })
                    .mip_levels(1)
                    .array_layers(1)
                    .samples(vk::SampleCountFlags::TYPE_1)
                    .usage(
                        vk::ImageUsageFlags::COLOR_ATTACHMENT
                            | vk::ImageUsageFlags::SAMPLED
                            | vk::ImageUsageFlags::TRANSFER_SRC
                            | vk::ImageUsageFlags::TRANSFER_DST,
                    ),
                None,
            )
        }
        .unwrap();
        let requirements = unsafe { self.device.get_image_memory_requirements(image) };
        let memory = self.allocate(requirements, vk::MemoryPropertyFlags::DEVICE_LOCAL);
        unsafe { self.device.bind_image_memory(image, memory, 0) }.unwrap();
        let view = unsafe {
            self.device.create_image_view(
                &vk::ImageViewCreateInfo::default()
                    .image(image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(FORMAT)
                    .subresource_range(color_range()),
                None,
            )
        }
        .unwrap();
        self.image = image;
        self.image_memory = memory;
        self.view = view;
    }

    fn make_buffer(&mut self) {
        let buffer = unsafe {
            self.device.create_buffer(
                &vk::BufferCreateInfo::default()
                    .size(u64::from(SIDE * SIDE * 4))
                    .usage(vk::BufferUsageFlags::TRANSFER_DST),
                None,
            )
        }
        .unwrap();
        let requirements = unsafe { self.device.get_buffer_memory_requirements(buffer) };
        let memory = self.allocate(
            requirements,
            vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
        );
        unsafe { self.device.bind_buffer_memory(buffer, memory, 0) }.unwrap();
        self.buffer = buffer;
        self.buffer_memory = memory;
    }

    fn allocate(
        &self,
        requirements: vk::MemoryRequirements,
        wanted: vk::MemoryPropertyFlags,
    ) -> vk::DeviceMemory {
        let properties = unsafe {
            self.instance
                .get_physical_device_memory_properties(self.physical)
        };
        let kind = (0..properties.memory_type_count)
            .find(|&index| {
                requirements.memory_type_bits & (1 << index) != 0
                    && properties.memory_types[index as usize]
                        .property_flags
                        .contains(wanted)
            })
            .expect("a matching memory type");
        unsafe {
            self.device.allocate_memory(
                &vk::MemoryAllocateInfo::default()
                    .allocation_size(requirements.size)
                    .memory_type_index(kind),
                None,
            )
        }
        .unwrap()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device.destroy_buffer(self.buffer, None);
            self.device.free_memory(self.buffer_memory, None);
            self.device.destroy_image_view(self.view, None);
            self.device.destroy_image(self.image, None);
            self.device.free_memory(self.image_memory, None);
            self.device.destroy_command_pool(self.pool, None);
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

fn enabled12() -> vk::PhysicalDeviceVulkan12Features<'static> {
    vk::PhysicalDeviceVulkan12Features::default()
        .timeline_semaphore(true)
        .runtime_descriptor_array(true)
        .shader_sampled_image_array_non_uniform_indexing(true)
        .descriptor_binding_sampled_image_update_after_bind(true)
        .descriptor_binding_partially_bound(true)
        .descriptor_binding_update_unused_while_pending(true)
}

fn enabled13() -> vk::PhysicalDeviceVulkan13Features<'static> {
    vk::PhysicalDeviceVulkan13Features::default()
        .dynamic_rendering(true)
        .synchronization2(true)
}

fn color_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    }
}

fn flush(device: &ash::Device, commands: vk::CommandBuffer) {
    let barrier = [vk::MemoryBarrier2::default()
        .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
        .src_access_mask(vk::AccessFlags2::MEMORY_WRITE)
        .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
        .dst_access_mask(vk::AccessFlags2::MEMORY_READ | vk::AccessFlags2::MEMORY_WRITE)];
    unsafe {
        device.cmd_pipeline_barrier2(
            commands,
            &vk::DependencyInfo::default().memory_barriers(&barrier),
        );
    }
}
