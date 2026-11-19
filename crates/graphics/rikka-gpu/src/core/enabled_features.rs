use ash::vk;

use crate::core::caps::FeatureChain;

impl FeatureChain {
    pub(crate) unsafe fn enabled_in(features: &vk::PhysicalDeviceFeatures2<'_>) -> Self {
        let mut chain = Self {
            features10: features.features,
            ..Self::default()
        };
        let mut next = features
            .p_next
            .cast_const()
            .cast::<vk::BaseInStructure<'_>>();
        while let Some(base) = unsafe { next.as_ref() } {
            unsafe { chain.merge(base.s_type, next.cast()) };
            next = base.p_next;
        }
        chain.detach();
        chain
    }

    unsafe fn merge(&mut self, kind: vk::StructureType, structure: *const ()) {
        let enable = |field: &mut vk::Bool32, value: vk::Bool32| *field |= value;
        let f11 = &mut self.features11;
        let f12 = &mut self.features12;
        let f13 = &mut self.features13;
        match kind {
            vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_FEATURES => {
                *f11 = unsafe { *structure.cast::<vk::PhysicalDeviceVulkan11Features<'_>>() };
            }
            vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_2_FEATURES => {
                *f12 = unsafe { *structure.cast::<vk::PhysicalDeviceVulkan12Features<'_>>() };
            }
            vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_3_FEATURES => {
                *f13 = unsafe { *structure.cast::<vk::PhysicalDeviceVulkan13Features<'_>>() };
            }
            vk::StructureType::PHYSICAL_DEVICE_SAMPLER_YCBCR_CONVERSION_FEATURES => {
                let alias = unsafe {
                    &*structure.cast::<vk::PhysicalDeviceSamplerYcbcrConversionFeatures<'_>>()
                };
                enable(
                    &mut f11.sampler_ycbcr_conversion,
                    alias.sampler_ycbcr_conversion,
                );
            }
            vk::StructureType::PHYSICAL_DEVICE_TIMELINE_SEMAPHORE_FEATURES => {
                let alias = unsafe {
                    &*structure.cast::<vk::PhysicalDeviceTimelineSemaphoreFeatures<'_>>()
                };
                enable(&mut f12.timeline_semaphore, alias.timeline_semaphore);
            }
            vk::StructureType::PHYSICAL_DEVICE_HOST_QUERY_RESET_FEATURES => {
                let alias =
                    unsafe { &*structure.cast::<vk::PhysicalDeviceHostQueryResetFeatures<'_>>() };
                enable(&mut f12.host_query_reset, alias.host_query_reset);
            }
            vk::StructureType::PHYSICAL_DEVICE_DESCRIPTOR_INDEXING_FEATURES => {
                let alias = unsafe {
                    &*structure.cast::<vk::PhysicalDeviceDescriptorIndexingFeatures<'_>>()
                };
                enable(
                    &mut f12.runtime_descriptor_array,
                    alias.runtime_descriptor_array,
                );
                enable(
                    &mut f12.shader_sampled_image_array_non_uniform_indexing,
                    alias.shader_sampled_image_array_non_uniform_indexing,
                );
                enable(
                    &mut f12.shader_storage_buffer_array_non_uniform_indexing,
                    alias.shader_storage_buffer_array_non_uniform_indexing,
                );
                enable(
                    &mut f12.shader_storage_image_array_non_uniform_indexing,
                    alias.shader_storage_image_array_non_uniform_indexing,
                );
                enable(
                    &mut f12.descriptor_binding_sampled_image_update_after_bind,
                    alias.descriptor_binding_sampled_image_update_after_bind,
                );
                enable(
                    &mut f12.descriptor_binding_storage_image_update_after_bind,
                    alias.descriptor_binding_storage_image_update_after_bind,
                );
                enable(
                    &mut f12.descriptor_binding_update_unused_while_pending,
                    alias.descriptor_binding_update_unused_while_pending,
                );
                enable(
                    &mut f12.descriptor_binding_partially_bound,
                    alias.descriptor_binding_partially_bound,
                );
            }
            vk::StructureType::PHYSICAL_DEVICE_BUFFER_DEVICE_ADDRESS_FEATURES => {
                let alias = unsafe {
                    &*structure.cast::<vk::PhysicalDeviceBufferDeviceAddressFeatures<'_>>()
                };
                enable(&mut f12.buffer_device_address, alias.buffer_device_address);
            }
            vk::StructureType::PHYSICAL_DEVICE_VULKAN_MEMORY_MODEL_FEATURES => {
                let alias = unsafe {
                    &*structure.cast::<vk::PhysicalDeviceVulkanMemoryModelFeatures<'_>>()
                };
                enable(&mut f12.vulkan_memory_model, alias.vulkan_memory_model);
                enable(
                    &mut f12.vulkan_memory_model_device_scope,
                    alias.vulkan_memory_model_device_scope,
                );
            }
            vk::StructureType::PHYSICAL_DEVICE_SCALAR_BLOCK_LAYOUT_FEATURES => {
                let alias = unsafe {
                    &*structure.cast::<vk::PhysicalDeviceScalarBlockLayoutFeatures<'_>>()
                };
                enable(&mut f12.scalar_block_layout, alias.scalar_block_layout);
            }
            vk::StructureType::PHYSICAL_DEVICE_DYNAMIC_RENDERING_FEATURES => {
                let alias =
                    unsafe { &*structure.cast::<vk::PhysicalDeviceDynamicRenderingFeatures<'_>>() };
                enable(&mut f13.dynamic_rendering, alias.dynamic_rendering);
            }
            vk::StructureType::PHYSICAL_DEVICE_SYNCHRONIZATION_2_FEATURES => {
                let alias =
                    unsafe { &*structure.cast::<vk::PhysicalDeviceSynchronization2Features<'_>>() };
                enable(&mut f13.synchronization2, alias.synchronization2);
            }
            vk::StructureType::PHYSICAL_DEVICE_MAINTENANCE_4_FEATURES => {
                let alias =
                    unsafe { &*structure.cast::<vk::PhysicalDeviceMaintenance4Features<'_>>() };
                enable(&mut f13.maintenance4, alias.maintenance4);
            }
            vk::StructureType::PHYSICAL_DEVICE_SHADER_DEMOTE_TO_HELPER_INVOCATION_FEATURES => {
                let alias = unsafe {
                    &*structure
                        .cast::<vk::PhysicalDeviceShaderDemoteToHelperInvocationFeatures<'_>>()
                };
                enable(
                    &mut f13.shader_demote_to_helper_invocation,
                    alias.shader_demote_to_helper_invocation,
                );
            }
            _ => {}
        }
    }
}
