pub(crate) mod exposure;
pub(crate) mod finish;
pub(crate) mod focus;
pub(crate) mod glow;
mod kernel;
pub(crate) mod motion;
mod optics;

use ash::vk;

use crate::gpu::NONE_INDEX;
use crate::graph::{ImageId, PassScope};
use crate::pipeline::Recipe;
use crate::recipes;
use crate::shader::MaterialShader;
use crate::state::Builtin;
use crate::viewport::COLOR_FORMAT;
use rikka_gpu::core::pipeline::Blend;
use rikka_gpu::texture::OUTPUT_FORMAT;
use rikka_spirv::Shader;

pub(crate) use finish::GpuGrade;
pub(crate) use glow::{GlowInputs, GpuGlow};
pub(crate) use optics::{sun_bearing, white_balance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pass {
    Glow,
    Grade,
    Tonemap,
    Lens,
    Layer,
    Present,
}

impl Pass {
    pub(crate) const ALL: [Self; 6] = [
        Self::Glow,
        Self::Grade,
        Self::Tonemap,
        Self::Lens,
        Self::Layer,
        Self::Present,
    ];

    fn index(self) -> usize {
        self as usize
    }

    fn format(self) -> vk::Format {
        match self {
            Self::Present => OUTPUT_FORMAT,
            _ => COLOR_FORMAT,
        }
    }
}

pub(crate) struct PostState {
    shaders: [MaterialShader; 6],
}

impl PostState {
    pub(crate) fn new() -> Self {
        Self {
            shaders: [
                shaders_3d::POST_GLOW_FRAG,
                shaders_3d::POST_GRADE_FRAG,
                shaders_3d::POST_TONEMAP_FRAG,
                shaders_3d::POST_LENS_FRAG,
                shaders_3d::COMPOSITE_LAYER_FRAG,
                shaders_3d::COMPOSITE_PRESENT_FRAG,
            ]
            .map(MaterialShader::builtin),
        }
    }

    pub(crate) fn recipe(&self, builtin: &Builtin, pass: Pass) -> Recipe {
        recipes::filter(
            builtin,
            &self.shaders[pass.index()],
            Blend::Opaque,
            pass.format(),
        )
    }

    pub(crate) fn recipes(&self, builtin: &Builtin) -> impl Iterator<Item = Recipe> {
        Pass::ALL.map(|pass| self.recipe(builtin, pass)).into_iter()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Exposed {
    Metered(ImageId),
    Fixed(f32),
}

impl Exposed {
    pub(crate) fn image(exposed: Option<Self>) -> Option<ImageId> {
        match exposed {
            Some(Self::Metered(state)) => Some(state),
            _ => None,
        }
    }

    pub(crate) fn push(exposed: Option<Self>, scope: &PassScope<'_>) -> (u32, f32) {
        match exposed {
            Some(Self::Metered(state)) => (scope.sampled(state), 1.0),
            Some(Self::Fixed(value)) => (NONE_INDEX, value),
            None => (NONE_INDEX, 1.0),
        }
    }
}

pub(crate) const KERNELS: [Shader; 15] = [
    shaders_3d::POST_EXPOSURE_COMP,
    shaders_3d::POST_EXPOSE_COMP,
    shaders_3d::POST_MOTION_TILES_COMP,
    shaders_3d::POST_MOTION_NEIGHBORS_COMP,
    shaders_3d::POST_MOTION_BLUR_COMP,
    shaders_3d::POST_FOCUS_SETUP_COMP,
    shaders_3d::POST_FOCUS_TILES_COMP,
    shaders_3d::POST_FOCUS_DILATE_COMP,
    shaders_3d::POST_FOCUS_GATHER_COMP,
    shaders_3d::POST_FOCUS_COMBINE_COMP,
    shaders_3d::POST_BLOOM_DOWN_COMP,
    shaders_3d::POST_BLOOM_UP_COMP,
    shaders_3d::POST_STREAK_DOWN_COMP,
    shaders_3d::POST_STREAK_UP_COMP,
    shaders_3d::POST_FLARE_COMP,
];
