use ash::vk;

use crate::build::BuiltFrame;
use crate::composite::CompositePlan;
use crate::draw::Frame;
use crate::editor::GridDraws;
use crate::fluid::LiquidStyle;
use crate::gpu::NONE_INDEX;
use crate::graph::{FrameGraph, ImageId, ImageSpec, Import, Size, ViewGraph};
use crate::look::Look;
use crate::look::gpu::{GpuLook, TRACED_LIGHTS};
use crate::look::tier::Tiers;
use crate::overlay::OverlayFrame;
use crate::recipes::{EMISSION_FORMAT, ViewFormats};
use crate::shadow::ShadowView;
use crate::space::EmitterFrame;
use crate::temporal::TemporalFrame;
use crate::viewport::Viewport;
use glam::Vec3;

pub(crate) const PYRAMID_LEVELS: usize = 16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct HzbFacts {
    pub extent: vk::Extent2D,
    pub source: vk::Extent2D,
    pub levels: u32,
    pub level_slots: [u32; PYRAMID_LEVELS],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ViewFacts {
    pub extent: vk::Extent2D,
    pub formats: ViewFormats,
    pub multisampled: bool,
    pub cluster_counts: [u32; 3],
    pub identity_slot: u32,
    pub outline_slot: u32,
    pub identity_samples: bool,
    pub hzb: Option<HzbFacts>,
    pub depth_resolve: vk::ResolveModeFlags,
    pub display: vk::Extent2D,
}

impl ViewFacts {
    pub(crate) fn of(viewport: &Viewport, depth_resolve: vk::ResolveModeFlags) -> Self {
        let targets = viewport.targets();
        let images = &targets.images;
        let hzb = images.hzb.as_ref().map(|hzb| {
            let mut level_slots = [0; PYRAMID_LEVELS];
            for (slot, &level) in level_slots.iter_mut().zip(&hzb.level_slots) {
                *slot = level;
            }
            HzbFacts {
                extent: hzb.extent,
                source: targets.extent,
                levels: hzb.levels,
                level_slots,
            }
        });
        Self {
            extent: viewport.extent(),
            formats: viewport.formats(),
            multisampled: viewport.multisampled(),
            cluster_counts: targets.cluster_counts,
            identity_slot: targets.identity_slot,
            outline_slot: targets.outline_slot,
            identity_samples: images.identity.is_some(),
            hzb,
            depth_resolve,
            display: viewport.display_extent(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Stage<'a> {
    pub frame: &'a Frame,
    pub built: &'a BuiltFrame,
    pub look: Option<&'a Look>,
    pub shadows: &'a [ShadowView],
    pub leading: Option<Vec3>,
    pub style: Option<LiquidStyle>,
    pub view: ViewFacts,
    pub clustered: bool,
    pub composite: &'a CompositePlan,
    pub temporal: Option<TemporalFrame>,
    pub tiers: Tiers,
    pub traces: bool,
    pub traced_lights: [u32; TRACED_LIGHTS],
    pub emitters: &'a EmitterFrame,
    pub overlays: &'a OverlayFrame,
    pub grid: Option<GridDraws>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Guides {
    pub full: ImageId,
    pub half: ImageId,
    pub full_history: ImageId,
    pub half_history: ImageId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SkyImages {
    pub transmittance: ImageId,
    pub multiscatter: ImageId,
    pub view: ImageId,
    pub environment_base: ImageId,
    pub environment: ImageId,
    pub aerial: Option<ImageId>,
    pub levels: u32,
    pub panorama: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FluidImages {
    pub depth: ImageId,
    pub front: ImageId,
    pub smoothed: [ImageId; 2],
    pub splatted: [ImageId; 2],
    pub thickness: ImageId,
    pub scattering: ImageId,
    pub spread: [ImageId; 3],
    pub coverage: ImageId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SceneImages {
    pub color: ImageId,
    pub resolved: ImageId,
    pub depth: ImageId,
    pub depth_resolved: ImageId,
    pub emission_target: Option<ImageId>,
    pub emission: Option<ImageId>,
    pub identity_target: Option<ImageId>,
    pub identity: Option<ImageId>,
    pub outline: Option<ImageId>,
    pub hzb: Option<ImageId>,
    pub output: ImageId,
    pub atlas: Option<ImageId>,
    pub figure_atlas: Option<ImageId>,
    pub tint: Option<TintImages>,
    pub scene_depth: Option<ImageId>,
    pub surface: Option<ImageId>,
    pub layers: Option<ImageId>,
    pub hair: Option<ImageId>,
    pub face: Option<ImageId>,
    pub sky: Option<SkyImages>,
    pub fog: Option<(ImageId, [u32; 3])>,
    pub occlusion: Option<ImageId>,
    pub screen_shadow: Option<ImageId>,
    pub fluid: Option<FluidImages>,
    pub brdf: Option<ImageId>,
    pub ltc: Option<LtcImages>,
    pub scatter: Option<ImageId>,
    pub refraction: Option<ImageId>,
    pub opaque: Option<ImageId>,
    pub blended: Option<ImageId>,
    pub previous_depth: Option<ImageId>,
    pub post: Option<PostSurface>,
    pub guides: Option<Guides>,
    pub visibility: Option<ImageId>,
    pub indirect: Option<ImageId>,
    pub radiance: Option<ImageId>,
    pub emitter_motion: Option<ImageId>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PostSurface {
    pub depth: ImageId,
    pub motion: ImageId,
    pub ratio: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TintImages {
    pub color: ImageId,
    pub depth: ImageId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LtcImages {
    pub inverse: ImageId,
    pub amplitude: ImageId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AtlasImports {
    pub main: bool,
    pub figures: bool,
    pub tinted: bool,
}

impl SceneImages {
    pub(crate) fn import(
        graph: &mut FrameGraph<'_>,
        view: ViewFacts,
        atlases: AtlasImports,
    ) -> Self {
        let identity = view
            .formats
            .identity
            .then(|| graph.import(Import::Identity));
        if let Some(identity) = identity {
            graph.publish(identity);
        }
        let identity_target = match view.identity_samples {
            true => Some(graph.import(Import::IdentityTarget)),
            false => identity,
        };
        let atlas = atlases.main.then(|| graph.import(Import::ShadowAtlas));
        if let Some(atlas) = atlas {
            graph.publish(atlas);
        }
        let figure_atlas = atlases.figures.then(|| graph.import(Import::FigureAtlas));
        if let Some(figure_atlas) = figure_atlas {
            graph.publish(figure_atlas);
        }
        let tint = atlases.tinted.then(|| TintImages {
            color: graph.import(Import::ShadowTint),
            depth: graph.import(Import::ShadowTintDepth),
        });
        if let Some(tint) = tint {
            graph.publish(tint.color);
            graph.publish(tint.depth);
        }
        let output = graph.import(Import::Output);
        graph.publish(output);
        let resolved = graph.import(Import::Resolved);
        let depth_resolved = graph.import(Import::DepthResolved);
        let emission = view.formats.emission.then(|| {
            graph.create(ImageSpec::new(
                "rikka.three.emission",
                EMISSION_FORMAT,
                Size::Viewport,
            ))
        });
        let emission_target = emission.map(|resolved| match view.multisampled {
            true => graph.create(
                ImageSpec::new("rikka.three.emission_ms", EMISSION_FORMAT, Size::Viewport)
                    .multisampled(view.formats.samples),
            ),
            false => resolved,
        });
        Self {
            color: match view.multisampled {
                true => graph.import(Import::ColorTarget),
                false => resolved,
            },
            resolved,
            depth: match view.multisampled {
                true => graph.import(Import::DepthTarget),
                false => depth_resolved,
            },
            depth_resolved,
            emission_target,
            emission,
            identity_target,
            identity,
            outline: (view.outline_slot != NONE_INDEX).then(|| graph.import(Import::Outline)),
            hzb: view.hzb.map(|_| graph.import(Import::Hzb)),
            output,
            atlas,
            figure_atlas,
            tint,
            scene_depth: None,
            surface: None,
            layers: None,
            hair: None,
            face: None,
            sky: None,
            fog: None,
            occlusion: None,
            screen_shadow: None,
            fluid: None,
            brdf: None,
            ltc: None,
            scatter: None,
            refraction: None,
            opaque: None,
            blended: None,
            previous_depth: None,
            post: None,
            guides: None,
            visibility: None,
            indirect: None,
            radiance: None,
            emitter_motion: None,
        }
    }

    pub(crate) fn scene_reads(&self) -> impl Iterator<Item = ImageId> + use<> {
        let post = self.post.map(|post| [post.depth, post.motion]);
        [self.scene_depth, self.surface, self.hair, self.face]
            .into_iter()
            .flatten()
            .chain(post.into_iter().flatten())
    }

    pub(crate) fn display_depth(&self) -> Option<ImageId> {
        self.post.map(|post| post.depth).or(self.scene_depth)
    }

    pub(crate) fn sky_reads(&self) -> impl Iterator<Item = ImageId> + use<> {
        let sky = self.sky.map(|sky| {
            [
                Some(sky.transmittance),
                Some(sky.multiscatter),
                Some(sky.view),
                Some(sky.environment),
                sky.aerial,
            ]
        });
        sky.into_iter()
            .flatten()
            .flatten()
            .chain(self.fog.map(|(fog, _)| fog))
            .chain(self.occlusion)
            .chain(self.screen_shadow)
            .chain(self.atlas)
            .chain(self.figure_atlas)
            .chain(
                self.tint
                    .into_iter()
                    .flat_map(|tint| [tint.color, tint.depth]),
            )
            .chain(self.brdf)
            .chain(
                self.ltc
                    .into_iter()
                    .flat_map(|ltc| [ltc.inverse, ltc.amplitude]),
            )
            .chain(self.visibility)
            .chain(self.indirect)
    }

    pub(crate) fn shadow_reads(&self) -> impl Iterator<Item = ImageId> + use<> {
        self.atlas.into_iter().chain(self.figure_atlas).chain(
            self.tint
                .into_iter()
                .flat_map(|tint| [tint.color, tint.depth]),
        )
    }

    pub(crate) fn environment(&self) -> Option<ImageId> {
        self.sky.map(|sky| sky.environment)
    }

    pub(crate) fn publish(&self, graph: &ViewGraph, look: &mut GpuLook) {
        let slot =
            |image: Option<ImageId>| image.map_or(NONE_INDEX, |image| graph.bound(image).sampled);
        if let Some(depth) = self.scene_depth {
            look.scene.depth = slot(Some(depth));
            look.scene.surface = slot(self.surface);
            look.scene.hair = slot(self.hair);
            look.scene.face = slot(self.face);
        }
        if let Some(occlusion) = self.occlusion {
            look.space.occlusion_image = slot(Some(occlusion));
        }
        if let Some(shadow) = self.screen_shadow {
            look.space.screen_shadow_image = slot(Some(shadow));
        }
        if let Some(sky) = self.sky {
            look.sky.transmittance_lut = slot(Some(sky.transmittance));
            look.sky.multiscatter_lut = slot(Some(sky.multiscatter));
            look.sky.sky_view_lut = slot(Some(sky.view));
            look.sky.environment_image = slot(Some(sky.environment));
            look.sky.environment_levels = sky.levels;
            look.sky.aerial_lut = slot(sky.aerial);
            look.sky.panorama = sky.panorama;
        }
        if let Some((fog, grid)) = self.fog {
            look.sky.fog_volume = slot(Some(fog));
            look.sky.fog_grid = grid;
        }
        look.scene.brdf = slot(self.brdf);
        if let Some(ltc) = self.ltc {
            look.scene.ltc_inverse = slot(Some(ltc.inverse));
            look.scene.ltc_amplitude = slot(Some(ltc.amplitude));
        }
        look.scene.scatter = slot(self.scatter);
        look.scene.refraction = slot(self.refraction);
        look.trace.visibility_image = slot(self.visibility);
        look.trace.indirect_image = slot(self.indirect);
    }
}
