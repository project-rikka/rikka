use ash::vk;
use bytemuck::{Pod, Zeroable};
use smallvec::SmallVec;

use crate::composite::RenderLayer;
use crate::gpu::NONE_INDEX;
use crate::graph::{Attachment, FrameGraph, ImageId, ImageSpec, Load, Size, groups};
use crate::pipeline::DrawState;
use crate::post::{Exposed, Pass};
use crate::recipes;
use crate::record::{LAYERS_FORMAT, SceneImages};
use crate::viewport::COLOR_FORMAT;
use rikka_gpu::core::pipeline::Blend;

const SURFACE_TILE: u32 = 8;
const PREVIOUS: &str = "rikka.three.composite.previous";
const PREVIOUS_KEY: u64 = 0;

#[derive(Clone, Copy)]
enum Kind {
    Emission = 0,
    Occlusion = 1,
    Mask = 2,
    Copy = 3,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SurfacePush {
    surface: u32,
    normals: u32,
    velocity: u32,
    motion: u32,
    size: [u32; 2],
    ratio: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LayersPush {
    layers: u32,
    pad: u32,
    ratio: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DepthPush {
    depth: u32,
    target: u32,
    size: [u32; 2],
    ratio: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ConvertPush {
    source: u32,
    kind: u32,
    exposure: u32,
    exposure_value: f32,
    size: [f32; 2],
    source_size: [f32; 2],
    source_capacity: [f32; 2],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Surface {
    pub normals: ImageId,
    pub velocity: ImageId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Reading {
    pub node: usize,
    pub exposed: Exposed,
}

#[derive(Default)]
pub(crate) struct LayerImages {
    surface: Option<Surface>,
    identity: Option<ImageId>,
    depth: Option<ImageId>,
    mask: Option<ImageId>,
    occlusion: Option<ImageId>,
    previous: Option<ImageId>,
    cut: bool,
    emission: SmallVec<[(Option<usize>, ImageId); 4]>,
}

fn ratio_of(scene: &SceneImages) -> [f32; 2] {
    scene.post.map_or([1.0; 2], |post| post.ratio)
}

impl LayerImages {
    pub(crate) fn new(cut: bool) -> Self {
        Self {
            cut,
            ..Self::default()
        }
    }

    pub(crate) fn image(
        &mut self,
        graph: &mut FrameGraph<'_>,
        scene: &SceneImages,
        resolved: ImageId,
        layer: RenderLayer,
        reading: Option<Reading>,
    ) -> ImageId {
        match layer {
            RenderLayer::Image => resolved,
            RenderLayer::Depth => self
                .depth(graph, scene)
                .expect("looked frames have a scene depth"),
            RenderLayer::Normals => {
                self.surface(graph, scene)
                    .expect("looked frames have surfaces")
                    .normals
            }
            RenderLayer::Velocity => {
                self.surface(graph, scene)
                    .expect("looked frames have surfaces")
                    .velocity
            }
            RenderLayer::Layers => self.mask(graph, scene),
            RenderLayer::Emission => self
                .emission(graph, scene, reading)
                .expect("frames reading emission draw it"),
            RenderLayer::Occlusion => self.occlusion(graph, scene),
            RenderLayer::Previous => self.previous(graph, resolved),
        }
    }

    pub(crate) fn surface(
        &mut self,
        graph: &mut FrameGraph<'_>,
        scene: &SceneImages,
    ) -> Option<Surface> {
        if self.surface.is_some() {
            return self.surface;
        }
        let surface = scene.surface?;
        let normals = graph.create(
            ImageSpec::new("rikka.three.effect.normals", COLOR_FORMAT, Size::Viewport).nearest(),
        );
        let velocity = graph.create(
            ImageSpec::new("rikka.three.effect.velocity", COLOR_FORMAT, Size::Viewport).nearest(),
        );
        let extent = graph.extent();
        let motion = scene.post.map(|post| post.motion);
        let ratio = ratio_of(scene);
        graph
            .compute("rikka.three.effect.surface")
            .read(surface)
            .read_all(motion)
            .write(normals)
            .write(velocity)
            .record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::EFFECT_SURFACE_COMP,
                    &SurfacePush {
                        surface: scope.sampled(surface),
                        normals: scope.storage(normals),
                        velocity: scope.storage(velocity),
                        motion: motion.map_or(NONE_INDEX, |motion| scope.sampled(motion)),
                        size: [extent.width, extent.height],
                        ratio,
                    },
                    groups(extent, SURFACE_TILE),
                )
            });
        self.surface = Some(Surface { normals, velocity });
        self.surface
    }

    pub(crate) fn identity(
        &mut self,
        graph: &mut FrameGraph<'_>,
        scene: &SceneImages,
    ) -> Option<ImageId> {
        let layers = scene.layers?;
        if scene.post.is_none() {
            return Some(layers);
        }
        if self.identity.is_some() {
            return self.identity;
        }
        let upscaled = graph.create(
            ImageSpec::new("rikka.three.effect.layers", LAYERS_FORMAT, Size::Viewport).nearest(),
        );
        let ratio = ratio_of(scene);
        graph
            .render("rikka.three.effect.layers")
            .color(Attachment::new(upscaled, Load::Discard))
            .read(layers)
            .record(move |record, ctx, scope| {
                let shader = &record.mesh.look_state.effect_layers;
                let recipe =
                    recipes::filter(&record.mesh.builtin, shader, Blend::Opaque, LAYERS_FORMAT);
                record.fullscreen(
                    ctx,
                    &recipe,
                    DrawState::NO_DEPTH,
                    bytemuck::bytes_of(&LayersPush {
                        layers: scope.sampled(layers),
                        pad: 0,
                        ratio,
                    }),
                )
            });
        self.identity = Some(upscaled);
        self.identity
    }

    pub(crate) fn depth(
        &mut self,
        graph: &mut FrameGraph<'_>,
        scene: &SceneImages,
    ) -> Option<ImageId> {
        let depth = scene.scene_depth?;
        if scene.post.is_none() {
            return Some(depth);
        }
        if self.depth.is_some() {
            return self.depth;
        }
        let upscaled = graph.create(
            ImageSpec::new(
                "rikka.three.effect.depth",
                vk::Format::R32_SFLOAT,
                Size::Viewport,
            )
            .nearest(),
        );
        let extent = graph.extent();
        let ratio = ratio_of(scene);
        graph
            .compute("rikka.three.effect.depth")
            .read(depth)
            .write(upscaled)
            .record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::EFFECT_DEPTH_COMP,
                    &DepthPush {
                        depth: scope.sampled(depth),
                        target: scope.storage(upscaled),
                        size: [extent.width, extent.height],
                        ratio,
                    },
                    groups(extent, SURFACE_TILE),
                )
            });
        self.depth = Some(upscaled);
        self.depth
    }

    pub(crate) fn emission(
        &mut self,
        graph: &mut FrameGraph<'_>,
        scene: &SceneImages,
        reading: Option<Reading>,
    ) -> Option<ImageId> {
        let emission = scene.emission?;
        let node = reading.map(|reading| reading.node);
        if let Some(&(_, image)) = self.emission.iter().find(|(known, _)| *known == node) {
            return Some(image);
        }
        let exposed = reading.map(|reading| reading.exposed);
        let image = convert(graph, Kind::Emission, Some(emission), exposed);
        self.emission.push((node, image));
        Some(image)
    }

    fn mask(&mut self, graph: &mut FrameGraph<'_>, scene: &SceneImages) -> ImageId {
        if let Some(mask) = self.mask {
            return mask;
        }
        let identity = self
            .identity(graph, scene)
            .expect("frames reading layers draw them");
        let mask = convert(graph, Kind::Mask, Some(identity), None);
        self.mask = Some(mask);
        mask
    }

    fn previous(&mut self, graph: &mut FrameGraph<'_>, resolved: ImageId) -> ImageId {
        if let Some(previous) = self.previous {
            return previous;
        }
        let kept = graph.create(previous_spec().previous(PREVIOUS_KEY));
        let target = graph.create(ImageSpec::new(PREVIOUS, COLOR_FORMAT, Size::Viewport));
        copy(graph, kept, Some(resolved), target, self.cut);
        self.previous = Some(target);
        target
    }

    fn occlusion(&mut self, graph: &mut FrameGraph<'_>, scene: &SceneImages) -> ImageId {
        if let Some(occlusion) = self.occlusion {
            return occlusion;
        }
        let occlusion = convert(graph, Kind::Occlusion, scene.occlusion, None);
        self.occlusion = Some(occlusion);
        occlusion
    }
}

fn previous_spec() -> ImageSpec {
    ImageSpec::new(PREVIOUS, COLOR_FORMAT, Size::Viewport)
}

pub(crate) fn keep_output(graph: &mut FrameGraph<'_>, output: ImageId) {
    let kept = graph.create(previous_spec().current(PREVIOUS_KEY));
    copy(graph, output, None, kept, false);
}

fn copy(
    graph: &mut FrameGraph<'_>,
    source: ImageId,
    fallback: Option<ImageId>,
    target: ImageId,
    cut: bool,
) {
    graph
        .render(PREVIOUS)
        .color(Attachment::new(target, Load::Discard))
        .read(source)
        .read_all(fallback)
        .record(move |record, ctx, scope| {
            let chosen = match fallback {
                Some(fallback) if cut || scope.fresh(source) => fallback,
                _ => source,
            };
            let push = ConvertPush {
                source: scope.sampled(chosen),
                kind: Kind::Copy as u32,
                exposure: NONE_INDEX,
                exposure_value: 1.0,
                size: [1.0; 2],
                source_size: [1.0; 2],
                source_capacity: [1.0; 2],
            };
            let recipe = record
                .mesh
                .look_state
                .post
                .recipe(&record.mesh.builtin, Pass::Layer);
            record.fullscreen(ctx, &recipe, DrawState::NO_DEPTH, bytemuck::bytes_of(&push))
        });
}

fn convert(
    graph: &mut FrameGraph<'_>,
    kind: Kind,
    source: Option<ImageId>,
    exposed: Option<Exposed>,
) -> ImageId {
    let extent = graph.extent();
    let target = graph.create(ImageSpec::new(
        "rikka.three.composite.layer",
        COLOR_FORMAT,
        Size::Viewport,
    ));
    graph
        .render("rikka.three.composite.layer")
        .color(Attachment::new(target, Load::Discard))
        .read_all(source)
        .read_all(Exposed::image(exposed))
        .record(move |record, ctx, scope| {
            let (exposure, exposure_value) = Exposed::push(exposed, scope);
            let size = |extent: vk::Extent2D| [extent.width as f32, extent.height as f32];
            let push = ConvertPush {
                source: source.map_or(NONE_INDEX, |source| scope.sampled(source)),
                kind: kind as u32,
                exposure,
                exposure_value,
                size: size(extent),
                source_size: source.map_or([1.0; 2], |source| size(scope.extent(source))),
                source_capacity: source
                    .map_or([1.0; 2], |source| size(scope.physical_extent(source))),
            };
            let recipe = record
                .mesh
                .look_state
                .post
                .recipe(&record.mesh.builtin, Pass::Layer);
            record.fullscreen(ctx, &recipe, DrawState::NO_DEPTH, bytemuck::bytes_of(&push))
        });
    target
}
