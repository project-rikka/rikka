use ash::vk;

use crate::core::sampler::Filtering;

const TABLE_CAPACITY: u32 = 1024;

const MESH_TABLE_CAPACITY: u32 = 16384;

const STORAGE_IMAGE_CAPACITY: u32 = 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct FeatureRequest {
    pub ycbcr: bool,
    pub three_d: bool,
    pub compute: bool,
    pub anisotropy: bool,
    pub host_query_reset: bool,
    pub float16: bool,
    pub int16: bool,
}

impl FeatureRequest {
    pub(crate) fn plain(three_d: bool) -> Self {
        Self {
            three_d,
            ..Self::default()
        }
    }

    pub(crate) fn computing() -> Self {
        Self {
            compute: true,
            ..Self::default()
        }
    }

    pub(crate) fn missing_on(
        self,
        instance: &ash::Instance,
        physical: vk::PhysicalDevice,
        available: &FeatureChain,
    ) -> Vec<&'static str> {
        let mut missing = FeatureChain::required(self).missing_from(available);
        if self.three_d && !supports_compute_ballot(instance, physical) {
            missing.push("subgroup basic and ballot operations in compute shaders");
        }
        missing
    }
}

fn supports_compute_ballot(instance: &ash::Instance, physical: vk::PhysicalDevice) -> bool {
    let mut subgroup = vk::PhysicalDeviceSubgroupProperties::default();
    let mut properties = vk::PhysicalDeviceProperties2::default().push_next(&mut subgroup);
    unsafe { instance.get_physical_device_properties2(physical, &mut properties) };
    let operations = vk::SubgroupFeatureFlags::BASIC | vk::SubgroupFeatureFlags::BALLOT;
    subgroup
        .supported_stages
        .contains(vk::ShaderStageFlags::COMPUTE)
        && subgroup.supported_operations.contains(operations)
}

#[derive(Clone, Debug, Default)]
pub(crate) struct FeatureChain {
    pub features10: vk::PhysicalDeviceFeatures,
    pub features11: vk::PhysicalDeviceVulkan11Features<'static>,
    pub features12: vk::PhysicalDeviceVulkan12Features<'static>,
    pub features13: vk::PhysicalDeviceVulkan13Features<'static>,
}

impl FeatureChain {
    pub(crate) fn query(instance: &ash::Instance, physical: vk::PhysicalDevice) -> Self {
        let mut features11 = vk::PhysicalDeviceVulkan11Features::default();
        let mut features12 = vk::PhysicalDeviceVulkan12Features::default();
        let mut features13 = vk::PhysicalDeviceVulkan13Features::default();
        let mut features2 = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut features11)
            .push_next(&mut features12)
            .push_next(&mut features13);
        unsafe { instance.get_physical_device_features2(physical, &mut features2) };

        let mut chain = Self {
            features10: features2.features,
            features11,
            features12,
            features13,
        };
        chain.detach();
        chain
    }

    fn detach(&mut self) {
        self.features11.p_next = std::ptr::null_mut();
        self.features12.p_next = std::ptr::null_mut();
        self.features13.p_next = std::ptr::null_mut();
    }

    pub(crate) fn required(wants: FeatureRequest) -> Self {
        let three_d = wants.three_d;
        let storage = three_d || wants.compute;
        let features10 = vk::PhysicalDeviceFeatures::default()
            .multi_draw_indirect(three_d)
            .draw_indirect_first_instance(three_d)
            .fill_mode_non_solid(three_d)
            .depth_clamp(three_d)
            .independent_blend(three_d)
            .sampler_anisotropy(three_d || wants.anisotropy)
            .shader_int64(storage)
            .shader_int16(wants.int16)
            .fragment_stores_and_atomics(three_d)
            .sample_rate_shading(three_d)
            .shader_storage_image_write_without_format(storage);
        let features11 =
            vk::PhysicalDeviceVulkan11Features::default().sampler_ycbcr_conversion(wants.ycbcr);
        let features12 = vk::PhysicalDeviceVulkan12Features::default()
            .timeline_semaphore(true)
            .shader_float16(wants.float16)
            .host_query_reset(wants.host_query_reset)
            .runtime_descriptor_array(true)
            .shader_sampled_image_array_non_uniform_indexing(true)
            .descriptor_binding_sampled_image_update_after_bind(true)
            .descriptor_binding_partially_bound(true)
            .descriptor_binding_update_unused_while_pending(true)
            .buffer_device_address(storage)
            .vulkan_memory_model(wants.compute)
            .vulkan_memory_model_device_scope(wants.compute)
            .draw_indirect_count(three_d)
            .scalar_block_layout(three_d)
            .shader_storage_buffer_array_non_uniform_indexing(three_d)
            .shader_storage_image_array_non_uniform_indexing(storage)
            .descriptor_binding_storage_image_update_after_bind(storage)
            .sampler_filter_minmax(three_d);
        let features13 = vk::PhysicalDeviceVulkan13Features::default()
            .dynamic_rendering(true)
            .synchronization2(true)
            .maintenance4(three_d)
            .shader_demote_to_helper_invocation(three_d);

        Self {
            features10,
            features11,
            features12,
            features13,
        }
    }

    pub(crate) fn missing_from(&self, available: &Self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        macro_rules! check {
            ($group:ident, $($field:ident => $name:literal),* $(,)?) => {
                $(
                    if self.$group.$field == vk::TRUE && available.$group.$field != vk::TRUE {
                        missing.push($name);
                    }
                )*
            };
        }
        check!(
            features10,
            multi_draw_indirect => "multiDrawIndirect",
            draw_indirect_first_instance => "drawIndirectFirstInstance",
            fill_mode_non_solid => "fillModeNonSolid",
            depth_clamp => "depthClamp",
            independent_blend => "independentBlend",
            sampler_anisotropy => "samplerAnisotropy",
            shader_int64 => "shaderInt64",
            fragment_stores_and_atomics => "fragmentStoresAndAtomics",
            sample_rate_shading => "sampleRateShading",
            shader_storage_image_write_without_format => "shaderStorageImageWriteWithoutFormat",
        );
        check!(
            features11,
            sampler_ycbcr_conversion => "samplerYcbcrConversion",
        );
        check!(
            features12,
            timeline_semaphore => "timelineSemaphore",
            host_query_reset => "hostQueryReset",
            runtime_descriptor_array => "runtimeDescriptorArray",
            shader_sampled_image_array_non_uniform_indexing => "shaderSampledImageArrayNonUniformIndexing",
            descriptor_binding_sampled_image_update_after_bind => "descriptorBindingSampledImageUpdateAfterBind",
            descriptor_binding_partially_bound => "descriptorBindingPartiallyBound",
            descriptor_binding_update_unused_while_pending => "descriptorBindingUpdateUnusedWhilePending",
            buffer_device_address => "bufferDeviceAddress",
            vulkan_memory_model => "vulkanMemoryModel",
            vulkan_memory_model_device_scope => "vulkanMemoryModelDeviceScope",
            draw_indirect_count => "drawIndirectCount",
            scalar_block_layout => "scalarBlockLayout",
            shader_storage_buffer_array_non_uniform_indexing => "shaderStorageBufferArrayNonUniformIndexing",
            shader_storage_image_array_non_uniform_indexing => "shaderStorageImageArrayNonUniformIndexing",
            descriptor_binding_storage_image_update_after_bind => "descriptorBindingStorageImageUpdateAfterBind",
            sampler_filter_minmax => "samplerFilterMinmax",
        );
        check!(
            features13,
            dynamic_rendering => "dynamicRendering",
            synchronization2 => "synchronization2",
            maintenance4 => "maintenance4",
            shader_demote_to_helper_invocation => "shaderDemoteToHelperInvocation",
        );
        missing
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DeviceProperties {
    pub limits: vk::PhysicalDeviceLimits,
    pub vendor_id: u32,
    pub table_capacity: u32,
    pub storage_image_capacity: u32,
    pub sample_counts: vk::SampleCountFlags,
    pub depth_resolve_modes: vk::ResolveModeFlags,
}

impl DeviceProperties {
    pub(crate) fn query(
        instance: &ash::Instance,
        physical: vk::PhysicalDevice,
        wants: FeatureRequest,
    ) -> Self {
        let mut vulkan12 = vk::PhysicalDeviceVulkan12Properties::default();
        let mut properties2 = vk::PhysicalDeviceProperties2::default().push_next(&mut vulkan12);
        unsafe { instance.get_physical_device_properties2(physical, &mut properties2) };
        let properties = properties2.properties;
        let limits = properties.limits;

        let required = match wants.three_d {
            true => MESH_TABLE_CAPACITY,
            false => TABLE_CAPACITY,
        };
        let (table_capacity, storage_image_capacity) = TableBudget::from_limits(&vulkan12).fit(
            required,
            match wants.three_d {
                true => STORAGE_IMAGE_CAPACITY,
                false => 0,
            },
        );

        Self {
            limits,
            vendor_id: properties.vendor_id,
            table_capacity,
            storage_image_capacity,
            sample_counts: limits.framebuffer_color_sample_counts
                & limits.framebuffer_depth_sample_counts
                & limits.sampled_image_color_sample_counts,
            depth_resolve_modes: vulkan12.supported_depth_resolve_modes,
        }
    }
}

const TABLE_SAMPLERS: u32 = Filtering::COUNT as u32 + 2;

const OTHER_SET_DESCRIPTORS: u32 = 16;

const COLOR_ATTACHMENTS: u32 = 8;

#[derive(Clone, Copy, Debug)]
struct TableBudget {
    samplers: u32,
    sampled_images: u32,
    storage_images: u32,
    resources: u32,
}

impl TableBudget {
    fn from_limits(limits: &vk::PhysicalDeviceVulkan12Properties<'_>) -> Self {
        Self {
            samplers: limits
                .max_per_stage_descriptor_update_after_bind_samplers
                .min(limits.max_descriptor_set_update_after_bind_samplers),
            sampled_images: limits
                .max_per_stage_descriptor_update_after_bind_sampled_images
                .min(limits.max_descriptor_set_update_after_bind_sampled_images),
            storage_images: limits
                .max_per_stage_descriptor_update_after_bind_storage_images
                .min(limits.max_descriptor_set_update_after_bind_storage_images),
            resources: limits.max_per_stage_update_after_bind_resources,
        }
    }

    fn fit(self, wanted_textures: u32, wanted_storage: u32) -> (u32, u32) {
        let spare = |limit: u32, taken: u32| limit.saturating_sub(taken + OTHER_SET_DESCRIPTORS);
        let storage = match wanted_storage {
            0 => 0,
            required => required
                .min(spare(self.storage_images, 0))
                .min(spare(self.resources, COLOR_ATTACHMENTS) / 3)
                .max(1),
        };
        let textures = wanted_textures
            .min(spare(self.samplers, TABLE_SAMPLERS))
            .min(spare(self.sampled_images, 0) / 2)
            .min(spare(self.resources, COLOR_ATTACHMENTS + storage) / 2)
            .max(1);
        (textures, storage)
    }
}
