use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::gpu::NONE_INDEX;
use crate::graph::{FrameGraph, ImageId, ImageSpec, PassScope, Size, groups};
use crate::look::Denoise;
use crate::record::SceneImages;

pub(super) const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
pub(super) const TILE: u32 = 8;
const FULL_PASSES: u32 = 2;

#[derive(Clone, Copy, Debug)]
pub(super) struct Signal {
    pub accumulated: &'static str,
    pub moments: &'static str,
    pub history: &'static str,
    pub moments_history: &'static str,
    pub filtered: &'static str,
    pub temporal: &'static str,
    pub spatial: &'static str,
    pub key: u64,
    pub channels: u32,
    pub halved: bool,
    pub clamped: bool,
    pub confident: bool,
}

impl Signal {
    pub(super) fn size(self) -> Size {
        match self.halved {
            true => Size::Divided(2),
            false => Size::Viewport,
        }
    }

    fn scale(self) -> u32 {
        match self.halved {
            true => 2,
            false => 1,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TemporalPush {
    frame: u64,
    noisy: u32,
    history: u32,
    moments_history: u32,
    guide_history: u32,
    guide: u32,
    target: u32,
    moments_target: u32,
    size: [u32; 2],
    scale: u32,
    channels: u32,
    longest: f32,
    fresh: u32,
    clamped: u32,
    tiles: u32,
    confidence: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SpatialPush {
    frame: u64,
    source: u32,
    guide: u32,
    moments: u32,
    target: u32,
    moments_copy: u32,
    size: [u32; 2],
    scale: u32,
    channels: u32,
    step: i32,
    variance_scale: f32,
    tiles: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct UpsamplePush {
    frame: u64,
    source: u32,
    guide: u32,
    guide_half: u32,
    target: u32,
    size: [u32; 2],
    half_size: [u32; 2],
    empty: [f32; 4],
}

pub(super) fn denoise(
    graph: &mut FrameGraph<'_>,
    scene: &SceneImages,
    noisy: ImageId,
    signal: Signal,
    denoise: Denoise,
    tiles: Option<ImageId>,
) -> ImageId {
    let guides = scene.guides.expect("denoised effects declare guides");
    let (guide, previous) = match signal.halved {
        true => (guides.half, guides.half_history),
        false => (guides.full, guides.full_history),
    };
    let size = signal.size();
    let extent = size.resolve(graph.extent());
    let spec = |name| ImageSpec::new(name, FORMAT, size).nearest();
    let accumulated = graph.create(spec(signal.accumulated));
    let moments = graph.create(spec(signal.moments));
    let history = graph.create(spec(signal.history).history(signal.key));
    let moments_history = graph.create(spec(signal.moments_history).history(signal.key));
    let longest = denoise.history.max(1.0);
    graph
        .compute(signal.temporal)
        .read_all(scene.scene_reads())
        .read(noisy)
        .read(history)
        .read(moments_history)
        .read(guide)
        .read(previous)
        .read_all(tiles)
        .write(accumulated)
        .write(moments)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::DENOISE_TEMPORAL_COMP,
                &TemporalPush {
                    frame: scope.frame(),
                    noisy: scope.sampled(noisy),
                    history: scope.sampled(history),
                    moments_history: scope.sampled(moments_history),
                    guide_history: scope.sampled(previous),
                    guide: scope.sampled(guide),
                    target: scope.storage(accumulated),
                    moments_target: scope.storage(moments),
                    size: [extent.width, extent.height],
                    scale: signal.scale(),
                    channels: signal.channels,
                    longest,
                    fresh: u32::from(scope.fresh(history) || scope.fresh(previous)),
                    clamped: u32::from(signal.clamped),
                    tiles: tile_slot(scope, tiles),
                    confidence: u32::from(signal.confident),
                },
                groups(extent, TILE),
            )
        });
    let filtering = denoise.passes > 0;
    let passes = match signal.halved {
        true => denoise.passes,
        false => denoise.passes.min(FULL_PASSES),
    }
    .max(1);
    let mut source = accumulated;
    for pass in 0..passes {
        let first = pass == 0;
        let target = match first {
            true => history,
            false => graph.create(spec(signal.filtered)),
        };
        let mut declared = graph
            .compute(signal.spatial)
            .read(guide)
            .read(source)
            .read(moments)
            .read_all(tiles)
            .write(target);
        if first {
            declared = declared.write(moments_history);
        }
        let from = source;
        declared.record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::DENOISE_SPATIAL_COMP,
                &SpatialPush {
                    frame: scope.frame(),
                    source: scope.sampled(from),
                    guide: scope.sampled(guide),
                    moments: scope.sampled(moments),
                    target: scope.storage(target),
                    moments_copy: match first {
                        true => scope.storage(moments_history),
                        false => NONE_INDEX,
                    },
                    size: [extent.width, extent.height],
                    scale: signal.scale(),
                    channels: signal.channels,
                    step: 1 << pass,
                    variance_scale: match filtering {
                        true => 1.0 / (1u32 << (2 * pass)) as f32,
                        false => 0.0,
                    },
                    tiles: tile_slot(scope, tiles),
                },
                groups(extent, TILE),
            )
        });
        source = target;
    }
    source
}

fn tile_slot(scope: &PassScope<'_>, tiles: Option<ImageId>) -> u32 {
    tiles.map_or(NONE_INDEX, |tiles| scope.sampled(tiles))
}

pub(super) fn upsample(
    graph: &mut FrameGraph<'_>,
    scene: &SceneImages,
    source: ImageId,
    name: &'static str,
    empty: [f32; 4],
) -> ImageId {
    let guides = scene.guides.expect("upsampled effects declare guides");
    let extent = graph.extent();
    let half = Size::Divided(2).resolve(extent);
    let target = graph.create(ImageSpec::new(name, FORMAT, Size::Viewport));
    graph.publish(target);
    graph
        .compute("rikka.ray.upsample")
        .read(guides.full)
        .read(guides.half)
        .read(source)
        .write(target)
        .record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::DENOISE_UPSAMPLE_COMP,
                &UpsamplePush {
                    frame: scope.frame(),
                    source: scope.sampled(source),
                    guide: scope.sampled(guides.full),
                    guide_half: scope.sampled(guides.half),
                    target: scope.storage(target),
                    size: [extent.width, extent.height],
                    half_size: [half.width, half.height],
                    empty,
                },
                groups(extent, TILE),
            )
        });
    target
}
