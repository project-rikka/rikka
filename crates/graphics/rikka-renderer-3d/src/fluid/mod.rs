mod composite;
mod gpu;
mod layer;
mod material;
mod plan;
mod shadow;
mod stamp;
mod style;
mod surface;
mod targets;
mod wet;

use ash::vk;

use rikka_spirv::Shader;

use crate::pipeline::{PassFormats, Raster, Recipe};
use crate::recipes;
use crate::record::SURFACE_FORMAT;
use crate::shader::MaterialShader;
use crate::state::Builtin;
use crate::viewport::{COLOR_FORMAT, DEPTH_FORMAT};
use rikka_gpu::core::pipeline::Blend;

pub use layer::FluidLayer;
pub use material::FluidMaterial;
pub use style::LiquidStyle;
pub use wet::Wetness;

pub use gpu::{
    PARTICLE_BYTES as FLUID_PARTICLE_BYTES, SPRAY_BYTES as FLUID_SPRAY_BYTES,
    WET_BYTES as FLUID_WET_BYTES,
};

pub(crate) use gpu::{GpuFluid, GpuFluidLayer, GpuFluidMaterial, GpuFluidView};
pub(crate) use plan::FluidFrame;
pub(crate) use wet::wet_address;

use targets::{FRONT_FORMAT, THICKNESS_FORMAT};

pub(crate) const KERNELS: [Shader; 2] =
    [shaders_3d::FLUID_SMOOTH_COMP, shaders_3d::FLUID_BLUR_COMP];

pub(crate) struct FluidState {
    splat: MaterialShader,
    depth: MaterialShader,
    seed: MaterialShader,
    thickness: MaterialShader,
    shadow: MaterialShader,
    composite: MaterialShader,
    styled_composite: MaterialShader,
    stamp: MaterialShader,
    spray_vert: MaterialShader,
    spray_frag: MaterialShader,
}

impl FluidState {
    pub(crate) fn new() -> Self {
        Self {
            splat: MaterialShader::builtin(shaders_3d::FLUID_SPLAT_VERT),
            depth: MaterialShader::builtin(shaders_3d::FLUID_DEPTH_FRAG),
            seed: MaterialShader::builtin(shaders_3d::FLUID_SEED_FRAG),
            thickness: MaterialShader::builtin(shaders_3d::FLUID_THICKNESS_FRAG),
            shadow: MaterialShader::builtin(shaders_3d::FLUID_SHADOW_FRAG),
            composite: MaterialShader::builtin(shaders_3d::FLUID_COMPOSITE_FRAG),
            styled_composite: MaterialShader::builtin(shaders_3d::FLUID_COMPOSITE_STYLED_FRAG),
            stamp: MaterialShader::builtin(shaders_3d::FLUID_STAMP_FRAG),
            spray_vert: MaterialShader::builtin(shaders_3d::FLUID_SPRAY_VERT),
            spray_frag: MaterialShader::builtin(shaders_3d::FLUID_SPRAY_FRAG),
        }
    }

    fn front_formats() -> PassFormats {
        PassFormats::color(FRONT_FORMAT, DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1)
    }

    pub(crate) fn depth_recipe(&self) -> Recipe {
        recipes::recipe(
            &self.splat,
            Some(&self.depth),
            Raster::FILL,
            Self::front_formats(),
            false,
        )
    }

    pub(crate) fn seed_recipe(&self, builtin: &Builtin) -> Recipe {
        recipes::fullscreen(builtin, &self.seed, Raster::FILL, Self::front_formats())
    }

    pub(crate) fn thickness_recipe(&self) -> Recipe {
        recipes::recipe(
            &self.splat,
            Some(&self.thickness),
            Raster::blended(Blend::Accumulate),
            PassFormats::pair(
                THICKNESS_FORMAT,
                THICKNESS_FORMAT,
                vk::SampleCountFlags::TYPE_1,
            ),
            false,
        )
    }

    pub(crate) fn shadow_recipe(&self) -> Recipe {
        recipes::recipe(
            &self.splat,
            Some(&self.shadow),
            recipes::shadow_raster(),
            PassFormats::shadow(DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1),
            false,
        )
    }

    pub(crate) fn composite_recipe(&self, builtin: &Builtin, styled: bool) -> Recipe {
        let shader = match styled {
            true => &self.styled_composite,
            false => &self.composite,
        };
        recipes::filter(builtin, shader, Blend::Opaque, COLOR_FORMAT)
    }

    pub(crate) fn spray_recipe(&self) -> Recipe {
        recipes::recipe(
            &self.spray_vert,
            Some(&self.spray_frag),
            Raster::blended(Blend::Premultiplied),
            PassFormats::color(
                COLOR_FORMAT,
                vk::Format::UNDEFINED,
                vk::SampleCountFlags::TYPE_1,
            ),
            false,
        )
    }

    pub(crate) fn stamp_recipe(&self, builtin: &Builtin) -> Recipe {
        recipes::fullscreen(
            builtin,
            &self.stamp,
            Raster::FILL,
            PassFormats::color(SURFACE_FORMAT, DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1),
        )
    }

    pub(crate) fn recipes(&self, builtin: &Builtin) -> [Recipe; 8] {
        [
            self.depth_recipe(),
            self.seed_recipe(builtin),
            self.thickness_recipe(),
            self.shadow_recipe(),
            self.composite_recipe(builtin, false),
            self.composite_recipe(builtin, true),
            self.spray_recipe(),
            self.stamp_recipe(builtin),
        ]
    }
}
