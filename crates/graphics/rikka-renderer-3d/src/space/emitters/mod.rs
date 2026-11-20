mod frame;
mod gpu;
mod record;

pub(crate) use frame::EmitterFrame;
pub(crate) use gpu::PARTICLE_BYTES;
pub(crate) use record::particles_read;

use ash::vk;

use crate::pipeline::{PassFormats, Raster, Recipe};
use crate::recipes::{self, EMISSION_FORMAT, ViewFormats};
use crate::record::LAYERS_FORMAT;
use crate::shader::MaterialShader;
use crate::viewport::COLOR_FORMAT;
use rikka_gpu::core::pipeline::Blend;

pub(crate) const MOTION_FORMAT: vk::Format = COLOR_FORMAT;

pub(crate) struct EmitterShaders {
    vertex: MaterialShader,
    shade: MaterialShader,
    emissive: MaterialShader,
    motion: MaterialShader,
    layers: MaterialShader,
}

impl EmitterShaders {
    pub(crate) fn new() -> Self {
        Self {
            vertex: MaterialShader::builtin(shaders_3d::EMITTERS_VERT),
            shade: MaterialShader::builtin(shaders_3d::EMITTERS_FRAG),
            emissive: MaterialShader::builtin(shaders_3d::EMITTERS_EMISSIVE_FRAG),
            motion: MaterialShader::builtin(shaders_3d::EMITTER_MOTION_FRAG),
            layers: MaterialShader::builtin(shaders_3d::EMITTER_LAYERS_FRAG),
        }
    }

    pub(crate) fn shade_recipe(&self, view: ViewFormats) -> Recipe {
        let (fragment, formats) = match view.emission {
            true => (
                &self.emissive,
                view.shade().with_second(EMISSION_FORMAT, true),
            ),
            false => (&self.shade, view.shade()),
        };
        recipes::recipe(
            &self.vertex,
            Some(fragment),
            Raster::blended(Blend::Premultiplied),
            formats,
            false,
        )
    }

    pub(crate) fn motion_recipe(&self) -> Recipe {
        self.single(
            &self.motion,
            Raster::blended(Blend::Accumulate),
            MOTION_FORMAT,
        )
    }

    pub(crate) fn layers_recipe(&self) -> Recipe {
        self.single(&self.layers, Raster::FILL, LAYERS_FORMAT)
    }

    fn single(&self, fragment: &MaterialShader, raster: Raster, format: vk::Format) -> Recipe {
        recipes::recipe(
            &self.vertex,
            Some(fragment),
            raster,
            PassFormats::color(format, vk::Format::UNDEFINED, vk::SampleCountFlags::TYPE_1),
            false,
        )
    }
}
