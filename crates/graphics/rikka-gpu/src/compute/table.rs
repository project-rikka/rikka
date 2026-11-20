use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::error::Result;

const BINDING_SAMPLED: u32 = 0;
const BINDING_STORAGE: u32 = 1;
const SAMPLED: (u32, vk::DescriptorType) = (BINDING_SAMPLED, vk::DescriptorType::SAMPLED_IMAGE);
const STORAGE: (u32, vk::DescriptorType) = (BINDING_STORAGE, vk::DescriptorType::STORAGE_IMAGE);
const BINDING_SAMPLER: u32 = 2;

const WANTED_SLOTS: u32 = 8192;

const SPARE: u32 = 16;

pub(crate) const PUSH_BYTES: u32 = 128;

pub(crate) struct Table {
    device: Arc<RenderDevice>,
    sampler: vk::Sampler,
    layout: vk::DescriptorSetLayout,
    pool: vk::DescriptorPool,
    set: vk::DescriptorSet,
    pipeline_layout: vk::PipelineLayout,
    capacity: u32,
}

impl Table {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Result<Table> {
        let capacity = capacity_of(&device);
        let sampler_info = vk::SamplerCreateInfo::default()
            .mag_filter(vk::Filter::NEAREST)
            .min_filter(vk::Filter::NEAREST)
            .mipmap_mode(vk::SamplerMipmapMode::NEAREST)
            .address_mode_u(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_v(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .address_mode_w(vk::SamplerAddressMode::CLAMP_TO_EDGE)
            .unnormalized_coordinates(false);
        let sampler = unsafe { device.handle().create_sampler(&sampler_info, None)? };
        let mut table = Table {
            device,
            sampler,
            layout: vk::DescriptorSetLayout::null(),
            pool: vk::DescriptorPool::null(),
            set: vk::DescriptorSet::null(),
            pipeline_layout: vk::PipelineLayout::null(),
            capacity,
        };
        table.build()?;
        Ok(table)
    }

    fn build(&mut self) -> Result<()> {
        let device = self.device.handle();
        let indexed = vk::DescriptorBindingFlags::PARTIALLY_BOUND
            | vk::DescriptorBindingFlags::UPDATE_AFTER_BIND
            | vk::DescriptorBindingFlags::UPDATE_UNUSED_WHILE_PENDING;
        let flags = [indexed, indexed, vk::DescriptorBindingFlags::empty()];
        let samplers = [self.sampler];
        let bindings = [
            vk::DescriptorSetLayoutBinding::default()
                .binding(BINDING_SAMPLED)
                .descriptor_type(vk::DescriptorType::SAMPLED_IMAGE)
                .descriptor_count(self.capacity)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(BINDING_STORAGE)
                .descriptor_type(vk::DescriptorType::STORAGE_IMAGE)
                .descriptor_count(self.capacity)
                .stage_flags(vk::ShaderStageFlags::COMPUTE),
            vk::DescriptorSetLayoutBinding::default()
                .binding(BINDING_SAMPLER)
                .descriptor_type(vk::DescriptorType::SAMPLER)
                .descriptor_count(1)
                .stage_flags(vk::ShaderStageFlags::COMPUTE)
                .immutable_samplers(&samplers),
        ];
        let mut flags_info =
            vk::DescriptorSetLayoutBindingFlagsCreateInfo::default().binding_flags(&flags);
        let layout_info = vk::DescriptorSetLayoutCreateInfo::default()
            .flags(vk::DescriptorSetLayoutCreateFlags::UPDATE_AFTER_BIND_POOL)
            .bindings(&bindings)
            .push_next(&mut flags_info);
        self.layout = unsafe { device.create_descriptor_set_layout(&layout_info, None)? };

        let sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLED_IMAGE,
                descriptor_count: self.capacity,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::STORAGE_IMAGE,
                descriptor_count: self.capacity,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::SAMPLER,
                descriptor_count: 1,
            },
        ];
        let pool_info = vk::DescriptorPoolCreateInfo::default()
            .flags(vk::DescriptorPoolCreateFlags::UPDATE_AFTER_BIND)
            .max_sets(1)
            .pool_sizes(&sizes);
        self.pool = unsafe { device.create_descriptor_pool(&pool_info, None)? };

        let layouts = [self.layout];
        let allocate = vk::DescriptorSetAllocateInfo::default()
            .descriptor_pool(self.pool)
            .set_layouts(&layouts);
        self.set = unsafe { device.allocate_descriptor_sets(&allocate)? }[0];
        self.device.set_object_name(self.set, "rikka.compute.table");

        let ranges = [vk::PushConstantRange::default()
            .stage_flags(vk::ShaderStageFlags::COMPUTE)
            .size(PUSH_BYTES)];
        let pipeline_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&layouts)
            .push_constant_ranges(&ranges);
        self.pipeline_layout = unsafe { device.create_pipeline_layout(&pipeline_info, None)? };
        Ok(())
    }

    pub(crate) fn capacity(&self) -> u32 {
        self.capacity
    }

    pub(crate) fn set(&self) -> vk::DescriptorSet {
        self.set
    }

    pub(crate) fn pipeline_layout(&self) -> vk::PipelineLayout {
        self.pipeline_layout
    }

    pub(crate) fn write(&self, slot: u32, view: vk::ImageView) {
        self.write_bindings(slot, view, &[SAMPLED, STORAGE]);
    }

    #[cfg(unix)]
    pub(crate) fn write_sampled(&self, slot: u32, view: vk::ImageView) {
        self.write_bindings(slot, view, &[SAMPLED]);
    }

    pub(crate) fn write_storage(&self, slot: u32, view: vk::ImageView) {
        self.write_bindings(slot, view, &[STORAGE]);
    }

    fn write_bindings(
        &self,
        slot: u32,
        view: vk::ImageView,
        bindings: &[(u32, vk::DescriptorType)],
    ) {
        let image = [vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: view,
            image_layout: vk::ImageLayout::GENERAL,
        }];
        let writes: Vec<vk::WriteDescriptorSet<'_>> = bindings
            .iter()
            .map(|&(binding, kind)| {
                vk::WriteDescriptorSet::default()
                    .dst_set(self.set)
                    .dst_binding(binding)
                    .dst_array_element(slot)
                    .descriptor_type(kind)
                    .image_info(&image)
            })
            .collect();
        unsafe { self.device.handle().update_descriptor_sets(&writes, &[]) };
    }
}

fn capacity_of(device: &RenderDevice) -> u32 {
    let mut vulkan12 = vk::PhysicalDeviceVulkan12Properties::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut vulkan12);
    unsafe {
        device
            .instance()
            .handle()
            .get_physical_device_properties2(device.physical(), &mut properties)
    };
    let room = vulkan12
        .max_per_stage_descriptor_update_after_bind_sampled_images
        .min(vulkan12.max_descriptor_set_update_after_bind_sampled_images)
        .min(vulkan12.max_per_stage_descriptor_update_after_bind_storage_images)
        .min(vulkan12.max_descriptor_set_update_after_bind_storage_images)
        .min(vulkan12.max_per_stage_update_after_bind_resources / 2);
    room.saturating_sub(SPARE).clamp(1, WANTED_SLOTS)
}

impl Drop for Table {
    fn drop(&mut self) {
        let device = self.device.handle();
        unsafe {
            device.destroy_pipeline_layout(self.pipeline_layout, None);
            device.destroy_descriptor_pool(self.pool, None);
            device.destroy_descriptor_set_layout(self.layout, None);
            device.destroy_sampler(self.sampler, None);
        }
    }
}
