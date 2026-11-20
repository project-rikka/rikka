mod atmosphere;
mod environment;
mod fog;
mod images;
mod kernel;
mod panorama;

use glam::Vec3;

use crate::error::{Error, Result};
use crate::graph::FrameGraph;
use crate::look::{Environment, EnvironmentImage, Fog, Look, Sky};
use crate::pipeline::{Raster, Recipe};
use crate::recipes::{self, ViewFormats};
use crate::record::stage::SkyImages;
use crate::record::{Chain, Record, SceneImages, Stage};
use crate::shader::MaterialShader;
use crate::state::Builtin;
use crate::viewport::COLOR_FORMAT;
use rikka_gpu::core::pipeline::Blend;
use rikka_gpu::texture::TextureId;
use rikka_spirv::Shader;

#[derive(Clone, Copy, PartialEq)]
struct SkyInputs {
    sky: Sky,
    fog: Fog,
    time: f32,
}

#[derive(Clone, Copy, PartialEq)]
struct Capture {
    texture: TextureId,
    scale: Vec3,
    clamp: Option<f32>,
}

#[derive(Clone, Copy, PartialEq)]
enum Lit {
    Sky(SkyInputs),
    Image { capture: Capture, rotation: f32 },
}

#[derive(Clone, Copy, PartialEq)]
struct Medium {
    haze: f32,
    ground: Vec3,
}

pub(crate) struct SkyState {
    fog_apply: MaterialShader,
    panorama: MaterialShader,
    lit: Option<Lit>,
    tabulated: Option<Medium>,
    captured: Option<Capture>,
}

impl SkyState {
    pub(crate) fn new() -> Self {
        Self {
            fog_apply: MaterialShader::builtin(shaders_3d::FOG_APPLY_FRAG),
            panorama: MaterialShader::builtin(shaders_3d::SKY_PANORAMA_FRAG),
            lit: None,
            tabulated: None,
            captured: None,
        }
    }

    pub(crate) fn fog_recipe(&self, builtin: &Builtin) -> Recipe {
        recipes::filter(builtin, &self.fog_apply, Blend::Opaque, COLOR_FORMAT)
    }

    pub(crate) fn panorama_recipe(&self, builtin: &Builtin, view: ViewFormats) -> Recipe {
        recipes::fullscreen(
            builtin,
            &self.panorama,
            Raster::blended(Blend::Premultiplied),
            view.shade(),
        )
    }
}

impl Record<'_> {
    pub(crate) fn declare_atmosphere<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        look: &'f Look,
        scene: &mut SceneImages,
    ) -> Result<()> {
        let mut sky_images = images::shared(graph);
        match &look.environment {
            Environment::None => self.mesh.look_state.sky.lit = None,
            Environment::Sky(sky) => {
                self.declare_physical_sky(graph, stage, look, *sky, &mut sky_images)
            }
            Environment::Image(image) => self.declare_panorama(graph, image, &mut sky_images)?,
        }
        scene.sky = Some(sky_images);
        Ok(())
    }

    fn declare_physical_sky(
        &mut self,
        graph: &mut FrameGraph<'_>,
        stage: Stage<'_>,
        look: &Look,
        sky: Sky,
        sky_images: &mut SkyImages,
    ) {
        let stored = &self.mesh.look_state.images;
        let kept =
            stored.holds_shared(images::TRANSMITTANCE) && stored.holds_shared(images::MULTISCATTER);
        let state = &mut self.mesh.look_state.sky;
        let lit = Some(Lit::Sky(SkyInputs {
            sky,
            fog: look.fog,
            time: stage.frame.time,
        }));
        let rebuild = state.lit != lit;
        state.lit = lit;
        let medium = Some(Medium {
            haze: sky.haze,
            ground: sky.ground,
        });
        let retabulate = state.tabulated != medium || !kept;
        state.tabulated = medium;
        let aerial = images::aerial(graph);
        sky_images.aerial = Some(aerial);
        let tables = *sky_images;
        graph.grouped("rikka.three.atmosphere", |graph| {
            if retabulate {
                atmosphere::declare_medium(graph, tables);
            }
            if rebuild {
                atmosphere::declare_view(graph, tables);
                environment::declare_environment(graph, tables);
            }
            atmosphere::declare_aerial(graph, tables, aerial);
        });
    }

    fn declare_panorama(
        &mut self,
        graph: &mut FrameGraph<'_>,
        image: &EnvironmentImage,
        sky_images: &mut SkyImages,
    ) -> Result<()> {
        let source =
            self.textures
                .slot(image.texture)
                .ok_or(Error::Gpu(rikka_gpu::Error::Unknown(
                    rikka_gpu::Resource::Texture(image.texture),
                )))?;
        sky_images.panorama = source;
        let stored = &self.mesh.look_state.images;
        let kept = stored.holds_shared(images::PANORAMA);
        let lighting = stored.holds_shared(images::ENVIRONMENT);
        let state = &mut self.mesh.look_state.sky;
        let capture = Capture {
            texture: image.texture,
            scale: image.radiance_scale(),
            clamp: image.sun_clamp,
        };
        let recapture = state.captured != Some(capture) || !kept;
        state.captured = Some(capture);
        let lit = Some(Lit::Image {
            capture,
            rotation: image.rotation,
        });
        let relight = recapture || state.lit != lit || !lighting;
        state.lit = lit;
        if !relight {
            return Ok(());
        }
        let tables = *sky_images;
        let atlas = images::panorama(graph);
        graph.grouped("rikka.three.atmosphere", |graph| {
            if recapture {
                panorama::declare_capture(graph, atlas, source);
            }
            panorama::declare_lighting(graph, tables, atlas);
        });
        Ok(())
    }

    pub(crate) fn declare_fog_volume<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        _stage: Stage<'f>,
        look: &'f Look,
        scene: &mut SceneImages,
    ) {
        if look.fog.density <= 0.0 {
            return;
        }
        fog::declare_froxels(graph, scene);
    }

    pub(crate) fn declare_fog<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        look: &'f Look,
        scene: &SceneImages,
        chain: &mut Chain,
    ) {
        fog::declare_apply(graph, look, scene, chain);
    }
}

pub(crate) const KERNELS: [Shader; 13] = [
    shaders_3d::SKY_TRANSMITTANCE_COMP,
    shaders_3d::SKY_MULTISCATTER_COMP,
    shaders_3d::SKY_VIEW_COMP,
    shaders_3d::SKY_ENVIRONMENT_COMP,
    shaders_3d::SKY_HARMONICS_COMP,
    shaders_3d::SKY_PREFILTER_COMP,
    shaders_3d::SKY_AERIAL_COMP,
    shaders_3d::SKY_SHAFTS_COMP,
    shaders_3d::FOG_VOLUME_COMP,
    shaders_3d::SKY_PANORAMA_CAPTURE_COMP,
    shaders_3d::SKY_PANORAMA_REDUCE_COMP,
    shaders_3d::SKY_PANORAMA_HARMONICS_COMP,
    shaders_3d::SKY_PANORAMA_PREFILTER_COMP,
];
