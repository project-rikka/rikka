use ash::vk;
use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::composite::Glow;
use crate::draw::Background;
use crate::gpu::NONE_INDEX;
use crate::graph::{Attachment, FrameGraph, ImageId, ImageSpec, Load, Size, scaled};
use crate::look::resolve::sun_color;
use crate::pipeline::DrawState;
use crate::post::kernel::{covering, extent_of, size_of_extent, texel_of};
use crate::post::optics::sun_on_screen;
use crate::post::{Exposed, Pass};
use crate::record::{SceneImages, Stage};
use crate::viewport::COLOR_FORMAT;
use rikka_spirv::Shader;

const DOWN: &str = "rikka.three.post.bloom_down";
const UP: &str = "rikka.three.post.bloom_up";
const STREAK_DOWN: &str = "rikka.three.post.streak_down";
const STREAK_UP: &str = "rikka.three.post.streak_up";
const FLARE: &str = "rikka.three.post.flare";
const COMBINE: &str = "rikka.three.post.glow";
const MAX_LEVELS: u32 = 8;
const SMALLEST_LEVEL: u32 = 4;
const MIST_LEVEL: usize = 3;
const MAX_STREAK_LEVELS: u32 = 7;
const NARROWEST_STREAK: u32 = 8;
const STREAK_THRESHOLD: f32 = 1.0;
const STREAK_STRETCH: f32 = 0.9;
const BLOOM_KNEE: f32 = 0.5;
const SUN_MARGIN: f32 = 0.3;
const SUN_RADIUS: f32 = 0.012;

#[derive(Clone, Copy, Default)]
struct Glowing {
    bloom: Option<ImageId>,
    mist: Option<ImageId>,
    streak: Option<ImageId>,
    flare: Option<ImageId>,
}

impl Glowing {
    fn images(self) -> impl Iterator<Item = ImageId> {
        [self.bloom, self.mist, self.streak, self.flare]
            .into_iter()
            .flatten()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuGlow {
    pub bloom_tint: [f32; 3],
    pub bloom_amount: f32,
    pub streak_tint: [f32; 3],
    pub streak_amount: f32,
    pub leak_tint: [f32; 3],
    pub leak_amount: f32,
    pub mist_amount: f32,
    pub flare_amount: f32,
    pub additive: u32,
    pub sun_facing: f32,
    pub sun_direction: [f32; 2],
    pub sun_strength: f32,
    pub pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CombinePush {
    params: u64,
    source: u32,
    bloom: u32,
    mist: u32,
    streak: u32,
    flare: u32,
    time: f32,
    size: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DownPush {
    source: u32,
    destination: u32,
    first: u32,
    size: [u32; 2],
    texel: [f32; 2],
    threshold: f32,
    knee: f32,
    source_size: [f32; 2],
    source_capacity: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UpPush {
    lower: u32,
    current: u32,
    destination: u32,
    pad: u32,
    size: [u32; 2],
    texel: [f32; 2],
    spread: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct StreakDownPush {
    source: u32,
    destination: u32,
    first: u32,
    size: [u32; 2],
    texel: [f32; 2],
    threshold: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FlarePush {
    source: u32,
    destination: u32,
    exposure: u32,
    exposure_value: f32,
    depth: u32,
    pad: u32,
    size: [u32; 2],
    depth_size: [u32; 2],
    sun_uv: [f32; 2],
    aspect: f32,
    sun: [f32; 3],
    blades: f32,
    rotation: f32,
    sun_radius: f32,
}

#[derive(Clone, Copy)]
struct Level {
    image: ImageId,
    extent: vk::Extent2D,
}

type Levels = smallvec::SmallVec<[Level; MAX_LEVELS as usize]>;

#[derive(Clone, Copy)]
pub(crate) struct GlowInputs {
    pub image: ImageId,
    pub source: Option<ImageId>,
    pub block: u64,
    pub exposed: Option<Exposed>,
}

fn level(graph: &mut FrameGraph<'_>, name: &'static str, extent: vk::Extent2D) -> Level {
    Level {
        image: graph.create(ImageSpec::new(
            name,
            COLOR_FORMAT,
            Size::Fixed(extent.width, extent.height),
        )),
        extent,
    }
}

pub(crate) fn declare<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    glow: &'f Glow,
    inputs: GlowInputs,
) -> ImageId {
    if !glow.is_active() {
        return inputs.image;
    }
    let mut glowing = Glowing::default();
    if glow.glows() {
        let extent = graph.extent();
        let source = Level {
            image: inputs.source.unwrap_or(inputs.image),
            extent,
        };
        let downs = bloom_down(graph, glow, source);
        if glow.bloom > 0.0 || glow.diffusion > 0.0 {
            let ups = bloom_up(graph, &downs, glow.radius);
            if glow.bloom > 0.0 {
                glowing.bloom = Some(ups[0].image);
            }
            if glow.diffusion > 0.0 {
                glowing.mist = Some(ups[MIST_LEVEL.min(ups.len() - 1)].image);
            }
        }
        if glow.streaks > 0.0 {
            glowing.streak = Some(streaks(graph, downs[0]));
        }
        if glow.flare > 0.0 {
            let ghosts = downs[2.min(downs.len() - 1)];
            glowing.flare = Some(flare(graph, stage, glow, scene, ghosts, inputs.exposed));
        }
    }
    combine(graph, stage, inputs, glowing)
}

fn combine<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    inputs: GlowInputs,
    glowing: Glowing,
) -> ImageId {
    let extent = graph.extent();
    let target = graph.create(ImageSpec::new(COMBINE, COLOR_FORMAT, Size::Viewport));
    let time = stage.frame.time;
    graph
        .render(COMBINE)
        .color(Attachment::new(target, Load::Discard))
        .read(inputs.image)
        .read_all(glowing.images())
        .record(move |record, ctx, scope| {
            let slot =
                |image: Option<ImageId>| image.map_or(NONE_INDEX, |image| scope.sampled(image));
            let push = CombinePush {
                params: scope.addresses.effects + inputs.block,
                source: scope.sampled(inputs.image),
                bloom: slot(glowing.bloom),
                mist: slot(glowing.mist),
                streak: slot(glowing.streak),
                flare: slot(glowing.flare),
                time,
                size: extent_of(extent),
            };
            let recipe = record
                .mesh
                .look_state
                .post
                .recipe(&record.mesh.builtin, Pass::Glow);
            record.fullscreen(ctx, &recipe, DrawState::NO_DEPTH, bytemuck::bytes_of(&push))
        });
    target
}

fn bloom_down(graph: &mut FrameGraph<'_>, glow: &Glow, source: Level) -> Levels {
    let extent = graph.extent();
    let threshold = glow.threshold.max(0.0);
    let mut input = source;
    let mut levels = Levels::new();
    for index in 0..level_count(extent) {
        let target = level(graph, DOWN, scaled(extent, 2 << index));
        graph
            .compute(DOWN)
            .read(input.image)
            .write(target.image)
            .record(move |record, ctx, scope| {
                let push = DownPush {
                    source: scope.sampled(input.image),
                    destination: scope.storage(target.image),
                    first: u32::from(index == 0),
                    size: size_of_extent(target.extent),
                    texel: texel_of(input.extent),
                    threshold,
                    knee: BLOOM_KNEE,
                    source_size: extent_of(scope.extent(input.image)),
                    source_capacity: extent_of(scope.physical_extent(input.image)),
                };
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::POST_BLOOM_DOWN_COMP,
                    &push,
                    covering(target.extent),
                )
            });
        input = target;
        levels.push(input);
    }
    levels
}

fn bloom_up(graph: &mut FrameGraph<'_>, downs: &[Level], radius: f32) -> Levels {
    let spread = 0.45 + 0.45 * radius.clamp(0.0, 1.0);
    let mut ups = Levels::from_slice(downs);
    for index in (0..downs.len() - 1).rev() {
        let lower = ups[index + 1];
        let current = downs[index];
        let target = level(graph, UP, current.extent);
        declare_up(
            graph,
            UP,
            shaders_3d::POST_BLOOM_UP_COMP,
            lower,
            current,
            target,
            spread,
        );
        ups[index] = target;
    }
    ups
}

fn declare_up(
    graph: &mut FrameGraph<'_>,
    name: &'static str,
    kernel: Shader,
    lower: Level,
    current: Level,
    target: Level,
    spread: f32,
) {
    graph
        .compute(name)
        .read(lower.image)
        .read(current.image)
        .write(target.image)
        .record(move |record, ctx, scope| {
            let push = UpPush {
                lower: scope.sampled(lower.image),
                current: scope.sampled(current.image),
                destination: scope.storage(target.image),
                pad: 0,
                size: size_of_extent(current.extent),
                texel: texel_of(lower.extent),
                spread,
            };
            record.dispatch_kernel(ctx, kernel, &push, covering(current.extent))
        });
}

fn streaks(graph: &mut FrameGraph<'_>, half: Level) -> ImageId {
    let first = scaled(graph.extent(), 4);
    let mut input = half;
    let mut downs = Levels::new();
    for index in 0..MAX_STREAK_LEVELS {
        let size = vk::Extent2D {
            width: first.width.div_ceil(1 << index).max(1),
            height: first.height,
        };
        if index > 0 && size.width < NARROWEST_STREAK {
            break;
        }
        let target = level(graph, STREAK_DOWN, size);
        graph
            .compute(STREAK_DOWN)
            .read(input.image)
            .write(target.image)
            .record(move |record, ctx, scope| {
                let push = StreakDownPush {
                    source: scope.sampled(input.image),
                    destination: scope.storage(target.image),
                    first: u32::from(index == 0),
                    size: size_of_extent(size),
                    texel: texel_of(input.extent),
                    threshold: STREAK_THRESHOLD,
                };
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::POST_STREAK_DOWN_COMP,
                    &push,
                    covering(size),
                )
            });
        input = target;
        downs.push(input);
    }
    let mut lower = downs[downs.len() - 1];
    for index in (0..downs.len() - 1).rev() {
        let current = downs[index];
        let target = level(graph, STREAK_UP, current.extent);
        declare_up(
            graph,
            STREAK_UP,
            shaders_3d::POST_STREAK_UP_COMP,
            lower,
            current,
            target,
            STREAK_STRETCH,
        );
        lower = target;
    }
    lower.image
}

fn flare<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    glow: &'f Glow,
    scene: &SceneImages,
    ghosts: Level,
    exposed: Option<Exposed>,
) -> ImageId {
    let extent = graph.extent();
    let size = scaled(extent, 4);
    let frame = stage.frame;
    let sky = stage.look.and_then(|look| look.environment.sky());
    let sun = match (frame.background, sky) {
        (Background::Sky, Some(sky)) => sun_on_screen(frame.camera.view_projection(), sky.to_sun())
            .filter(|uv| uv.min_element() > -SUN_MARGIN && uv.max_element() < 1.0 + SUN_MARGIN)
            .map(|uv| (uv, sun_color(sky))),
        _ => None,
    };
    let (sun_uv, sun) = match sun {
        Some((uv, color)) => (uv.to_array(), color),
        None => ([0.0; 2], Vec3::ZERO),
    };
    let depth = scene
        .display_depth()
        .expect("looked frames have a scene depth");
    let target = level(graph, FLARE, size);
    graph
        .compute(FLARE)
        .read(ghosts.image)
        .read_all(Exposed::image(exposed))
        .read(depth)
        .write(target.image)
        .record(move |record, ctx, scope| {
            let (exposure, exposure_value) = Exposed::push(exposed, scope);
            let push = FlarePush {
                source: scope.sampled(ghosts.image),
                destination: scope.storage(target.image),
                exposure,
                exposure_value,
                depth: scope.sampled(depth),
                pad: 0,
                size: size_of_extent(size),
                depth_size: size_of_extent(extent),
                sun_uv,
                aspect: extent.width as f32 / extent.height as f32,
                sun: sun.to_array(),
                blades: glow.blades as f32,
                rotation: glow.blade_rotation,
                sun_radius: SUN_RADIUS,
            };
            record.dispatch_kernel(ctx, shaders_3d::POST_FLARE_COMP, &push, covering(size))
        });
    target.image
}

fn level_count(extent: vk::Extent2D) -> u32 {
    let smallest = (extent.width.min(extent.height) / 2).max(1);
    let fits = (smallest / SMALLEST_LEVEL).max(1).ilog2() + 1;
    fits.clamp(1, MAX_LEVELS)
}
