mod emitters;
mod lines;
mod occlusion;
mod particles;
mod reflections;
mod screen_shadows;

pub(crate) use emitters::{EmitterFrame, PARTICLE_BYTES, particles_read};
pub(crate) use particles::particles_drawn;

use crate::error::Result;
use crate::graph::FrameGraph;
use crate::look::Look;
use crate::look::gpu::GpuSpace;
use crate::look::tier::Mode;
use crate::pipeline::{DrawState, Raster, Recipe};
use crate::recipes::{self, ViewFormats};
use crate::record::{Chain, Record, SceneImages, Stage};
use crate::shader::MaterialShader;
use crate::state::Builtin;
use crate::viewport::COLOR_FORMAT;
use emitters::EmitterShaders;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::pipeline::Blend;
use rikka_spirv::Shader;

pub(crate) struct SpaceState {
    reflect: MaterialShader,
    lines: MaterialShader,
    particles_vert: MaterialShader,
    particles_frag: MaterialShader,
    pub(crate) emitters: EmitterShaders,
}

impl SpaceState {
    pub(crate) fn new() -> Self {
        Self {
            reflect: MaterialShader::builtin(shaders_3d::SPACE_REFLECT_FRAG),
            lines: MaterialShader::builtin(shaders_3d::SPACE_LINES_FRAG),
            particles_vert: MaterialShader::builtin(shaders_3d::PARTICLES_VERT),
            particles_frag: MaterialShader::builtin(shaders_3d::PARTICLES_FRAG),
            emitters: EmitterShaders::new(),
        }
    }

    fn filter_recipe(&self, builtin: &Builtin, shader: &MaterialShader) -> Recipe {
        recipes::filter(builtin, shader, Blend::Opaque, COLOR_FORMAT)
    }

    pub(crate) fn particles_recipe(&self, view: ViewFormats) -> Recipe {
        recipes::recipe(
            &self.particles_vert,
            Some(&self.particles_frag),
            Raster::blended(Blend::Premultiplied),
            view.shade(),
            false,
        )
    }

    pub(crate) fn filter_recipes(&self, builtin: &Builtin) -> [Recipe; 2] {
        [
            self.filter_recipe(builtin, &self.reflect),
            self.filter_recipe(builtin, &self.lines),
        ]
    }
}

impl Record<'_> {
    pub(crate) fn declare_occlusion<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        look: &'f Look,
        scene: &mut SceneImages,
    ) {
        let mut warm: smallvec::SmallVec<[Recipe; 3]> = smallvec::SmallVec::new();
        if stage.tiers.reflections != Mode::Off {
            warm.push(self.space_filter(|space| &space.reflect));
        }
        if look.lines.strength > 0.0 {
            warm.push(self.space_filter(|space| &space.lines));
        }
        if GpuSpace::particle_count(look.particles.amount) > 0 {
            warm.push(self.space_particles_recipe(stage.view.formats));
        }
        for recipe in &warm {
            self.mesh.pipelines.prepare(recipe);
        }
        let space = GpuSpace::from_look(look);
        if stage.tiers.occlusion == Mode::ScreenSpace {
            occlusion::declare(graph, scene);
        }
        if space.screen_shadows()
            && let Some(leading) = stage.leading
        {
            screen_shadows::declare(graph, scene, &stage.frame.camera, leading);
        }
    }

    pub(crate) fn declare_reflections<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        look: &'f Look,
        scene: &SceneImages,
        chain: &mut Chain,
    ) {
        let trace = match stage.tiers.reflections {
            Mode::Off => return,
            Mode::ScreenSpace => reflections::trace(graph, scene, chain),
            Mode::RayTraced => self.declare_ray_reflections(graph, look, scene, chain),
        };
        reflections::resolve(graph, scene, chain, trace);
    }

    pub(crate) fn declare_lines<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        look: &'f Look,
        scene: &SceneImages,
        chain: &mut Chain,
    ) {
        let lines = &look.lines;
        if lines.strength > 0.0 && lines.width > 0.0 {
            lines::declare(graph, scene, chain);
        }
    }

    fn space_filter(&self, shader: impl FnOnce(&SpaceState) -> &MaterialShader) -> Recipe {
        let space = &self.mesh.look_state.space;
        space.filter_recipe(&self.mesh.builtin, shader(space))
    }

    fn space_fullscreen(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        shader: impl FnOnce(&SpaceState) -> &MaterialShader,
        push: &[u8],
    ) -> Result<()> {
        let recipe = self.space_filter(shader);
        self.fullscreen(ctx, &recipe, DrawState::NO_DEPTH, push)
    }
}

pub(crate) const KERNELS: [Shader; 5] = [
    shaders_3d::SPACE_OCCLUSION_COMP,
    shaders_3d::SPACE_DENOISE_COMP,
    shaders_3d::SPACE_TRACE_COMP,
    shaders_3d::SPACE_SCREEN_SHADOW_COMP,
    shaders_3d::EMITTER_EVALUATE_COMP,
];
