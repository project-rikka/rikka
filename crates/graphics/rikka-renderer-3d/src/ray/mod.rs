mod commands;
mod denoise;
mod hooks;
mod reservoir;
pub(crate) mod scene;
mod structure;

use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::build::BuiltFrame;
use crate::draw::Frame;
use crate::error::Result;
use crate::gpu::{GpuPassPush, NONE_INDEX};
use crate::graph::{Buffer, BufferUse, FrameGraph, ImageId, ImageSpec, Size, groups};
use crate::look::gpu::TRACED_LIGHTS;
use crate::look::tier::Mode;
use crate::look::{Denoise, Look, Resampling};
use crate::record::command::{bind_compute, dispatch, push_constants};
use crate::record::{Chain, Record, SceneImages};
use rikka_gpu::core::barrier::MemoryAccess;
use rikka_gpu::core::compute::{groups as compute_groups, rows, widest_row};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_spirv::Shader;

use denoise::{FORMAT, Signal, TILE, denoise, upsample};

pub(crate) use scene::{RayFrame, RayScene};

const HOOK_GROUP: u32 = 64;

pub(crate) const KERNELS: [Shader; 6] = [
    shaders_3d::RAY_SHADOW_COMP,
    shaders_3d::RAY_OCCLUSION_COMP,
    shaders_3d::RAY_BOUNCE_COMP,
    shaders_3d::RAY_RESERVOIR_COMP,
    shaders_3d::RAY_RESAMPLE_COMP,
    shaders_3d::RAY_REFLECT_COMP,
];

pub(crate) const SCREEN_KERNELS: [Shader; 6] = [
    shaders_3d::RAY_GUIDE_COMP,
    shaders_3d::SCREEN_BOUNCE_COMP,
    shaders_3d::DENOISE_TEMPORAL_COMP,
    shaders_3d::DENOISE_SPATIAL_COMP,
    shaders_3d::DENOISE_UPSAMPLE_COMP,
    shaders_3d::RAY_KEEP_COMP,
];

const GUIDE_KEY: u64 = 0x7261_7900;
const RADIANCE_KEY: u64 = 0x7261_7901;
const REFLECTION_REACH: f32 = 100.0;
const MAX_SAMPLES: u32 = 16;
const RESAMPLED_HISTORY: f32 = 0.5;
const RESAMPLED_PASSES: u32 = 2;

macro_rules! signal {
    ($name:literal, $($field:ident: $value:expr),* $(,)?) => {
        Signal {
            accumulated: concat!("rikka.ray.", $name, ".accumulated"),
            moments: concat!("rikka.ray.", $name, ".moments"),
            history: concat!("rikka.ray.", $name, ".history"),
            moments_history: concat!("rikka.ray.", $name, ".moments_history"),
            filtered: concat!("rikka.ray.", $name, ".filtered"),
            temporal: concat!("rikka.ray.", $name, ".temporal"),
            spatial: concat!("rikka.ray.", $name, ".spatial"),
            confident: false,
            $($field: $value),*
        }
    };
}

const SHADOWS: Signal = signal!(
    "shadows",
    key: 0x7261_7910,
    channels: 4,
    halved: false,
    clamped: true,
);

const OCCLUSION: Signal = signal!(
    "occlusion",
    key: 0x7261_7911,
    channels: 1,
    halved: true,
    clamped: false,
);

const ILLUMINATION: Signal = signal!(
    "illumination",
    key: 0x7261_7912,
    channels: 3,
    halved: true,
    clamped: false,
);

const REFLECTIONS: Signal = signal!(
    "reflections",
    key: 0x7261_7913,
    channels: 3,
    halved: true,
    clamped: true,
);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShadowPush {
    frame: u64,
    target: u32,
    samples: u32,
    size: [u32; 2],
    lights: [u32; TRACED_LIGHTS],
    tiles: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct OcclusionPush {
    frame: u64,
    target: u32,
    samples: u32,
    size: [u32; 2],
    radius: f32,
    strength: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BouncePush {
    frame: u64,
    target: u32,
    samples: u32,
    size: [u32; 2],
    reach: f32,
    radiance: u32,
    guide_history: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ScreenBouncePush {
    frame: u64,
    target: u32,
    samples: u32,
    size: [u32; 2],
    reach: f32,
    radiance: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ReflectPush {
    frame: u64,
    source: u32,
    target: u32,
    samples: u32,
    size: [u32; 2],
    reach: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct KeepPush {
    frame: u64,
    lit: u32,
    full: u32,
    half: u32,
    full_history: u32,
    half_history: u32,
    radiance: u32,
    size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GuidePush {
    frame: u64,
    full: u32,
    half: u32,
    size: [u32; 2],
}

pub(crate) fn samples(requested: u32) -> u32 {
    requested.clamp(1, MAX_SAMPLES)
}

fn noisy(graph: &mut FrameGraph<'_>, name: &'static str, size: Size) -> ImageId {
    graph.create(ImageSpec::new(name, FORMAT, size).nearest())
}

fn denoised_illumination(
    graph: &mut FrameGraph<'_>,
    scene: &SceneImages,
    raw: ImageId,
    look: &Look,
    resampled: bool,
) -> ImageId {
    let signal = Signal {
        confident: resampled,
        clamped: resampled,
        ..ILLUMINATION
    };
    let settings = match resampled {
        true => Denoise {
            history: (look.denoise.history * RESAMPLED_HISTORY).max(1.0),
            passes: look.denoise.passes.min(RESAMPLED_PASSES),
        },
        false => look.denoise,
    };
    let filtered = denoise(graph, scene, raw, signal, settings, None);
    upsample(graph, scene, filtered, "rikka.ray.illumination", [0.0; 4])
}

fn traced() -> BufferUse {
    BufferUse::new(
        vk::PipelineStageFlags2::COMPUTE_SHADER,
        vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR,
    )
}

pub(crate) fn traced_fragments() -> BufferUse {
    BufferUse::new(
        vk::PipelineStageFlags2::FRAGMENT_SHADER,
        vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR | vk::AccessFlags2::SHADER_STORAGE_READ,
    )
}

impl Record<'_> {
    pub(crate) fn ray_prepare(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame: &Frame,
        built: &BuiltFrame,
    ) -> Result<RayFrame> {
        let mesh = &mut *self.mesh;
        let rays = mesh
            .rays
            .as_mut()
            .expect("traced tiers resolve only with a ray scene");
        rays.prepare(ctx, frame, built, &mesh.meshes, &mesh.materials)
    }

    pub(crate) fn declare_ray_build(&mut self, graph: &mut FrameGraph<'_>) {
        graph
            .compute("rikka.ray.build")
            .buffer(
                Buffer::Deform,
                BufferUse::new(
                    vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR
                        | vk::PipelineStageFlags2::COMPUTE_SHADER,
                    vk::AccessFlags2::SHADER_READ | vk::AccessFlags2::SHADER_STORAGE_READ,
                ),
            )
            .buffer(
                Buffer::Scene,
                BufferUse::new(
                    vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR,
                    vk::AccessFlags2::ACCELERATION_STRUCTURE_READ_KHR
                        | vk::AccessFlags2::ACCELERATION_STRUCTURE_WRITE_KHR,
                ),
            )
            .record(|record, ctx, scope| {
                record.ray_hooks(ctx, scope.addresses.refs)?;
                if let Some(rays) = &mut record.mesh.rays {
                    rays.record(ctx);
                }
                Ok(())
            });
    }

    fn ray_hooks(&mut self, ctx: &mut GpuFrameCtx<'_>, frame: u64) -> Result<()> {
        let mesh = &mut *self.mesh;
        let Some(rays) = &mesh.rays else {
            return Ok(());
        };
        let jobs = rays.hook_jobs();
        if jobs.is_empty() {
            return Ok(());
        }
        ctx.memory_barrier(
            MemoryAccess::new(
                vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR
                    | vk::PipelineStageFlags2::COMPUTE_SHADER
                    | vk::PipelineStageFlags2::FRAGMENT_SHADER,
                vk::AccessFlags2::NONE,
            ),
            MemoryAccess::new(
                vk::PipelineStageFlags2::COMPUTE_SHADER,
                vk::AccessFlags2::SHADER_STORAGE_WRITE,
            ),
        );
        let layout = mesh.pipelines.layout();
        let widest = widest_row(ctx.device);
        for (slot, job) in jobs.iter().enumerate() {
            let pipeline = mesh.pipelines.material_kernel(&job.shader)?;
            let (x, y) = rows(compute_groups(job.vertex_count, HOOK_GROUP), widest);
            bind_compute(ctx, pipeline);
            let push = GpuPassPush {
                frame,
                slot: slot as u32,
                flags: x * HOOK_GROUP,
            };
            push_constants(ctx, layout, bytemuck::bytes_of(&push));
            dispatch(ctx, x, y, 1);
        }
        ctx.memory_barrier(
            MemoryAccess::new(
                vk::PipelineStageFlags2::COMPUTE_SHADER,
                vk::AccessFlags2::SHADER_STORAGE_WRITE,
            ),
            MemoryAccess::new(
                vk::PipelineStageFlags2::ACCELERATION_STRUCTURE_BUILD_KHR
                    | vk::PipelineStageFlags2::COMPUTE_SHADER
                    | vk::PipelineStageFlags2::FRAGMENT_SHADER,
                vk::AccessFlags2::SHADER_READ | vk::AccessFlags2::SHADER_STORAGE_READ,
            ),
        );
        self.forget_bindings();
        Ok(())
    }

    pub(crate) fn declare_ray_guides(
        &mut self,
        graph: &mut FrameGraph<'_>,
        scene: &mut SceneImages,
    ) {
        let spec = |name, size| ImageSpec::new(name, FORMAT, size).nearest();
        let full = graph.create(spec("rikka.ray.guide", Size::Viewport));
        let half = graph.create(spec("rikka.ray.guide.half", Size::Divided(2)));
        let full_history =
            graph.create(spec("rikka.ray.guide.previous", Size::Viewport).history(GUIDE_KEY));
        let half_history = graph
            .create(spec("rikka.ray.guide.half.previous", Size::Divided(2)).history(GUIDE_KEY));
        let extent = graph.extent();
        graph
            .compute("rikka.ray.guide")
            .read_all(scene.scene_reads())
            .write(full)
            .write(half)
            .record(move |record, ctx, scope| {
                record.dispatch_kernel(
                    ctx,
                    shaders_3d::RAY_GUIDE_COMP,
                    &GuidePush {
                        frame: scope.frame(),
                        full: scope.storage(full),
                        half: scope.storage(half),
                        size: [extent.width, extent.height],
                    },
                    groups(extent, TILE),
                )
            });
        scene.guides = Some(crate::record::Guides {
            full,
            half,
            full_history,
            half_history,
        });
    }

    pub(crate) fn declare_ray_occlusion<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        look: &'f Look,
        scene: &mut SceneImages,
    ) {
        let extent = Size::Divided(2).resolve(graph.extent());
        let occlusion = look.occlusion;
        let result = graph.grouped("rikka.ray.occlusion", |graph| {
            let raw = noisy(graph, "rikka.ray.occlusion.noisy", Size::Divided(2));
            graph
                .compute("rikka.ray.occlusion.trace")
                .read_all(scene.scene_reads())
                .buffer(Buffer::Scene, traced())
                .write(raw)
                .record(move |record, ctx, scope| {
                    record.dispatch_kernel(
                        ctx,
                        shaders_3d::RAY_OCCLUSION_COMP,
                        &OcclusionPush {
                            frame: scope.frame(),
                            target: scope.storage(raw),
                            samples: samples(occlusion.samples),
                            size: [extent.width, extent.height],
                            radius: occlusion.radius.max(0.01),
                            strength: occlusion.strength.max(0.0),
                        },
                        groups(extent, TILE),
                    )
                });
            let filtered = denoise(graph, scene, raw, OCCLUSION, look.denoise, None);
            upsample(
                graph,
                scene,
                filtered,
                "rikka.ray.occlusion",
                [1.0, 0.0, 0.0, 0.0],
            )
        });
        scene.occlusion = Some(result);
    }

    pub(crate) fn declare_ray_shadows<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        look: &'f Look,
        scene: &mut SceneImages,
        lights: [u32; TRACED_LIGHTS],
    ) {
        if lights.iter().all(|&light| light == NONE_INDEX) {
            return;
        }
        let extent = graph.extent();
        let count = look.shadows.samples;
        let result = graph.grouped("rikka.ray.shadows", |graph| {
            let raw = noisy(graph, "rikka.ray.shadows.noisy", Size::Viewport);
            let tiles = noisy(graph, "rikka.ray.shadows.tiles", Size::Divided(TILE));
            graph
                .compute("rikka.ray.shadows.trace")
                .read_all(scene.scene_reads())
                .buffer(Buffer::Scene, traced())
                .write(raw)
                .write(tiles)
                .record(move |record, ctx, scope| {
                    record.dispatch_kernel(
                        ctx,
                        shaders_3d::RAY_SHADOW_COMP,
                        &ShadowPush {
                            frame: scope.frame(),
                            target: scope.storage(raw),
                            samples: samples(count),
                            size: [extent.width, extent.height],
                            lights,
                            tiles: scope.storage(tiles),
                            _pad: 0,
                        },
                        groups(extent, TILE),
                    )
                });
            denoise(graph, scene, raw, SHADOWS, look.denoise, Some(tiles))
        });
        graph.publish(result);
        scene.visibility = Some(result);
    }

    pub(crate) fn declare_ray_illumination<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        look: &'f Look,
        mode: Mode,
        scene: &mut SceneImages,
    ) {
        let extent = Size::Divided(2).resolve(graph.extent());
        let radiance = graph.create(
            ImageSpec::new("rikka.ray.radiance", FORMAT, Size::Divided(2)).history(RADIANCE_KEY),
        );
        scene.radiance = Some(radiance);
        let guide = scene
            .guides
            .expect("illumination declares guides")
            .half_history;
        let illumination = look.illumination;
        let reads: smallvec::SmallVec<[ImageId; 24]> = scene
            .scene_reads()
            .chain(scene.sky_reads())
            .chain([radiance, guide])
            .collect();
        let resampled = mode == Mode::RayTraced && illumination.resampling != Resampling::Off;
        let result = graph.grouped("rikka.ray.illumination", |graph| {
            let raw = noisy(graph, "rikka.ray.illumination.noisy", Size::Divided(2));
            if resampled {
                reservoir::resample(graph, scene, illumination, radiance, raw);
                return denoised_illumination(graph, scene, raw, look, true);
            }
            let pass = graph
                .compute("rikka.ray.illumination.trace")
                .read_all(reads)
                .write(raw);
            match mode {
                Mode::RayTraced => {
                    pass.buffer(Buffer::Scene, traced())
                        .record(move |record, ctx, scope| {
                            let stale = scope.fresh(radiance) || scope.fresh(guide);
                            record.dispatch_kernel(
                                ctx,
                                shaders_3d::RAY_BOUNCE_COMP,
                                &BouncePush {
                                    frame: scope.frame(),
                                    target: scope.storage(raw),
                                    samples: samples(illumination.samples),
                                    size: [extent.width, extent.height],
                                    reach: illumination.reach.max(0.01),
                                    radiance: match stale {
                                        true => NONE_INDEX,
                                        false => scope.sampled(radiance),
                                    },
                                    guide_history: scope.sampled(guide),
                                    _pad: 0,
                                },
                                groups(extent, TILE),
                            )
                        })
                }
                Mode::ScreenSpace | Mode::Off => pass.record(move |record, ctx, scope| {
                    record.dispatch_kernel(
                        ctx,
                        shaders_3d::SCREEN_BOUNCE_COMP,
                        &ScreenBouncePush {
                            frame: scope.frame(),
                            target: scope.storage(raw),
                            samples: samples(illumination.samples),
                            size: [extent.width, extent.height],
                            reach: illumination.reach.max(0.01),
                            radiance: match scope.fresh(radiance) {
                                true => NONE_INDEX,
                                false => scope.sampled(radiance),
                            },
                        },
                        groups(extent, TILE),
                    )
                }),
            }
            denoised_illumination(graph, scene, raw, look, false)
        });
        scene.indirect = Some(result);
    }

    pub(crate) fn declare_ray_reflections<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        look: &'f Look,
        scene: &SceneImages,
        chain: &Chain,
    ) -> ImageId {
        let extent = Size::Divided(2).resolve(graph.extent());
        let source = chain.current();
        let count = look.reflections.samples;
        let reads: smallvec::SmallVec<[ImageId; 24]> = scene
            .scene_reads()
            .chain(scene.sky_reads())
            .chain([source])
            .collect();
        graph.grouped("rikka.ray.reflections", |graph| {
            let raw = noisy(graph, "rikka.ray.reflections.noisy", Size::Divided(2));
            graph
                .compute("rikka.ray.reflections.trace")
                .read_all(reads)
                .buffer(Buffer::Scene, traced())
                .write(raw)
                .record(move |record, ctx, scope| {
                    record.dispatch_kernel(
                        ctx,
                        shaders_3d::RAY_REFLECT_COMP,
                        &ReflectPush {
                            frame: scope.frame(),
                            source: scope.sampled(source),
                            target: scope.storage(raw),
                            samples: samples(count),
                            size: [extent.width, extent.height],
                            reach: REFLECTION_REACH,
                        },
                        groups(extent, TILE),
                    )
                });
            denoise(graph, scene, raw, REFLECTIONS, look.denoise, None)
        })
    }

    pub(crate) fn declare_ray_keep(
        &mut self,
        graph: &mut FrameGraph<'_>,
        scene: &SceneImages,
        chain: &Chain,
    ) {
        let Some(guides) = scene.guides else {
            return;
        };
        let extent = graph.extent();
        let lit = chain.current();
        let radiance = scene.radiance;
        let mut pass = graph
            .compute("rikka.ray.keep")
            .read(lit)
            .read(guides.full)
            .read(guides.half)
            .write(guides.full_history)
            .write(guides.half_history);
        if let Some(radiance) = radiance {
            pass = pass.write(radiance);
        }
        pass.record(move |record, ctx, scope| {
            record.dispatch_kernel(
                ctx,
                shaders_3d::RAY_KEEP_COMP,
                &KeepPush {
                    frame: scope.frame(),
                    lit: scope.sampled(lit),
                    full: scope.sampled(guides.full),
                    half: scope.sampled(guides.half),
                    full_history: scope.storage(guides.full_history),
                    half_history: scope.storage(guides.half_history),
                    radiance: radiance.map_or(NONE_INDEX, |radiance| scope.storage(radiance)),
                    size: [extent.width, extent.height],
                },
                groups(extent, TILE),
            )
        });
    }
}
