use ash::vk;

use crate::core::device::RenderDevice;
use crate::error::{Error, Result};

pub(crate) fn create_shader_module(
    device: &RenderDevice,
    spirv: &[u8],
) -> Result<vk::ShaderModule> {
    let words = spirv_words(spirv)?;
    let info = vk::ShaderModuleCreateInfo::default().code(&words);
    Ok(unsafe { device.handle().create_shader_module(&info, None)? })
}

pub fn spirv_words(spirv: &[u8]) -> Result<Vec<u32>> {
    rikka_spirv::words(spirv).map_err(malformed)
}

pub fn malformed(error: rikka_spirv::Error) -> Error {
    Error::InvalidShader(format!("not SPIR-V: {error}"))
}
