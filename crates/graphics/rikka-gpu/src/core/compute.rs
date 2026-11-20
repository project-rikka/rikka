use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::shader::create_shader_module;
use crate::error::Result;

pub fn create_compute_pipeline(
    device: &RenderDevice,
    spirv: &[u8],
    layout: vk::PipelineLayout,
) -> Result<vk::Pipeline> {
    let module = create_shader_module(device, spirv)?;

    let stage = vk::PipelineShaderStageCreateInfo::default()
        .stage(vk::ShaderStageFlags::COMPUTE)
        .module(module)
        .name(c"main");
    let info = vk::ComputePipelineCreateInfo::default()
        .stage(stage)
        .layout(layout);

    let pipelines = unsafe {
        device
            .handle()
            .create_compute_pipelines(device.pipeline_cache(), &[info], None)
    };
    unsafe { device.handle().destroy_shader_module(module, None) };

    match pipelines {
        Ok(pipelines) => Ok(pipelines[0]),
        Err((_, err)) => Err(err.into()),
    }
}

pub fn groups(total: u32, local: u32) -> u32 {
    total.div_ceil(local).max(1)
}

pub fn widest_row(device: &RenderDevice) -> u32 {
    device.limits().max_compute_work_group_count[0].max(1)
}

pub fn rows(groups: u32, widest: u32) -> (u32, u32) {
    (groups.min(widest), groups.div_ceil(widest))
}
