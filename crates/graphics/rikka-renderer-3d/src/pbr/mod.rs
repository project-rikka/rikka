mod lut;
mod scatter;

pub(crate) use scatter::{refracts, scatters};

use rikka_spirv::Shader;

pub(crate) const KERNELS: [Shader; 2] = [shaders_3d::PBR_BRDF_COMP, shaders_3d::SCATTER_BLUR_COMP];
