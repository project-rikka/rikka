use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::build::BuiltFrame;
use crate::draw::Background;
use crate::error::Result;
use crate::frame_data::FrameAddresses;
use crate::gpu::GpuPassPush;
use crate::graph::ImageId;
use crate::graph::{Attachment, Buffer, BufferUse, FrameGraph, Load, SHADER_STAGES};
use crate::material::{Cull, MaterialPass};
use crate::pbr;
use crate::pipeline::DrawState;
use crate::ray;
use crate::recipes::{self, ViewFormats};
use crate::record::cull::draws_read;
use crate::record::direct::DirectPass;
use crate::record::stage::{SceneImages, Stage};
use crate::record::{Chain, PASS_MASKED, Record};
use crate::shader::Tracing;
use crate::space::{particles_drawn, particles_read};
use crate::viewport::COLOR_FORMAT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::pipeline::Blend;

const SHADE: &str = "rikka.three.shade";
const SHADE_TRANSPARENT: &str = "rikka.three.shade_transparent";

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GridPush {
    frame: u64,
    color: [f32; 3],
    opacity: f32,
    spacing: f32,
    reach: f32,
    vertical_axis: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ClipBoundsPush {
    frame: u64,
    pad: [u32; 2],
    planes: [[f32; 4]; 6],
    color: [f32; 4],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Contents {
    sky: bool,
    opaque: bool,
    rest: bool,
}

#[derive(Clone, Copy)]
struct Segment {
    name: &'static str,
    contents: Contents,
    restore: bool,
    last: bool,
}

fn segments(split: bool) -> [Option<Segment>; 2] {
    let segment = |name, opaque, rest, restore, last| {
        Some(Segment {
            name,
            contents: Contents {
                sky: opaque,
                opaque,
                rest,
            },
            restore,
            last,
        })
    };
    match split {
        false => [segment(SHADE, true, true, false, true), None],
        true => [
            segment(SHADE, true, false, false, false),
            segment(SHADE_TRANSPARENT, false, true, true, true),
        ],
    }
}

impl Record<'_> {
    pub(super) fn declare_shade<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &mut SceneImages,
    ) -> Chain {
        let mut chain = Chain::start(scene.resolved);
        let scattering = pbr::scatters(stage, scene);
        let refracting = pbr::refracts(stage);
        let capture = stage.temporal.is_some() && blends_over_opaque(stage);
        let segments = segments(capture || refracting || scattering);
        for (at, segment) in segments.into_iter().flatten().enumerate() {
            if at == 1 && scattering {
                self.declare_pbr_scatter(graph, stage, scene, &mut chain);
            }
            if segment.name == SHADE_TRANSPARENT && (capture || refracting) {
                let opaque = capture_opaque(graph, &mut chain);
                if capture {
                    scene.opaque = Some(opaque);
                }
                if refracting {
                    scene.refraction = Some(opaque);
                }
            }
            self.declare_segment(graph, stage, scene, segment, &mut chain);
        }
        chain
    }

    fn declare_segment<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &SceneImages,
        segment: Segment,
        chain: &mut Chain,
    ) {
        let view = stage.view;
        let frame = stage.frame;
        let clear = match frame.background {
            Background::Transparent | Background::Sky => [0.0, 0.0, 0.0, 0.0],
            Background::Color(color) => [color.x, color.y, color.z, 1.0],
        };
        let load = match segment.restore {
            true => Load::Discard,
            false => Load::Clear(vk::ClearValue {
                color: vk::ClearColorValue { float32: clear },
            }),
        };
        let restored = segment.restore.then(|| chain.current());
        let mut color = Attachment::new(scene.color, load);
        if view.multisampled {
            color = color
                .transient()
                .resolve(scene.resolved, vk::ResolveModeFlags::AVERAGE);
        }
        let mut depth = Attachment::new(scene.depth, Load::Keep);
        if segment.last {
            depth = match (stage.temporal.is_some(), view.multisampled) {
                (false, _) => depth.transient(),
                (true, false) => depth,
                (true, true) => depth
                    .transient()
                    .resolve(scene.depth_resolved, view.depth_resolve),
            };
        }
        let mut pass = graph
            .render(segment.name)
            .color(color)
            .depth(depth)
            .read_all(scene.identity)
            .read_all(scene.sky_reads())
            .read_all(scene.scene_reads())
            .read_all(restored)
            .buffer(Buffer::Environment, BufferUse::storage_read(SHADER_STAGES));
        if let Some(atlas) = scene.atlas {
            pass = pass.read(atlas);
        }
        let particles = segment.contents.rest && stage.look.is_some_and(particles_drawn);
        if let Some(environment) = scene.environment().filter(|_| particles) {
            pass = pass.read_vertex(environment);
        }
        if segment.contents.rest && !stage.emitters.is_empty() {
            pass = pass.buffer(Buffer::Particles, particles_read());
        }
        if let (Some(target), Some(resolved)) = (scene.emission_target, scene.emission) {
            let load = match segment.restore {
                true => Load::Keep,
                false => Load::Clear(vk::ClearValue {
                    color: vk::ClearColorValue { float32: [0.0; 4] },
                }),
            };
            let mut emission = Attachment::new(target, load);
            if target != resolved {
                emission = emission.resolve(resolved, vk::ResolveModeFlags::AVERAGE);
                if segment.last {
                    emission = emission.transient();
                }
            }
            pass = pass.color(emission);
        }
        for (buffer, usage) in draws_read() {
            pass = pass.buffer(buffer, usage);
        }
        if view.formats.tracing == Tracing::RayQueries {
            pass = pass.buffer(Buffer::Scene, ray::traced_fragments());
        }
        let formats = view.formats;
        pass.record(move |record, ctx, scope| {
            if let Some(restored) = restored {
                record.restore(ctx, formats, scope.sampled(restored))?;
            }
            record.shade(ctx, stage, scope.addresses, segment.contents)
        });
        chain.restart(scene.resolved);
    }

    fn shade_backdrop(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        stage: Stage<'_>,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let view = stage.view.formats;
        let panorama = stage.look.and_then(|look| look.environment.image());
        let recipe = match (panorama, stage.frame.background) {
            (Some(image), Background::Sky | Background::Color(_)) if image.background > 0.0 => self
                .mesh
                .look_state
                .sky
                .panorama_recipe(&self.mesh.builtin, view),
            (None, Background::Sky) => recipes::sky(&self.mesh.builtin, view),
            _ => return Ok(()),
        };
        let behind = DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER_OR_EQUAL, false);
        self.fullscreen(
            ctx,
            &recipe,
            behind,
            bytemuck::bytes_of(&GpuPassPush {
                frame: addresses.refs,
                slot: 0,
                flags: 0,
            }),
        )
    }

    fn shade_clip_bounds(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        stage: Stage<'_>,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        let Some(bounds) = stage.frame.editor.bounds else {
            return Ok(());
        };
        let recipe = recipes::clip_bounds(&self.mesh.builtin, stage.view.formats);
        let behind = DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER_OR_EQUAL, false);
        let push = ClipBoundsPush {
            frame: addresses.refs,
            pad: [0; 2],
            planes: bounds.inward_planes().map(|plane| plane.to_array()),
            color: bounds.color.extend(1.0).to_array(),
        };
        self.fullscreen(ctx, &recipe, behind, bytemuck::bytes_of(&push))
    }

    fn shade(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        stage: Stage<'_>,
        addresses: &FrameAddresses,
        contents: Contents,
    ) -> Result<()> {
        let Stage {
            frame, built, look, ..
        } = stage;
        let view = stage.view.formats;
        let behind = DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER_OR_EQUAL, false);
        if contents.sky {
            self.shade_backdrop(ctx, stage, addresses)?;
            self.shade_clip_bounds(ctx, stage, addresses)?;
        }
        if contents.opaque {
            for prepassed in [true, false] {
                for (index, bucket) in built.buckets.iter().enumerate() {
                    if !bucket.pass.in_prepass() || bucket.hull == prepassed {
                        continue;
                    }
                    self.shade_bucket(ctx, built, addresses, view, index)?;
                }
            }
        }
        if !contents.rest {
            return Ok(());
        }
        self.draw_direct(
            ctx,
            built,
            addresses,
            &built.transparent,
            DirectPass::Shade(view),
        )?;
        if look.is_some() && !built.hair.is_empty() {
            self.draw_direct(
                ctx,
                built,
                addresses,
                &built.see_through,
                DirectPass::SeeThrough(view),
            )?;
        }
        if let Some(look) = look {
            self.particles(ctx, look, addresses, view)?;
        }
        self.space_emitters(ctx, stage, addresses)?;
        if frame.grid.enabled {
            let push = GridPush {
                frame: addresses.refs,
                color: frame.grid.color.to_array(),
                opacity: frame.grid.opacity,
                spacing: frame.grid.spacing,
                reach: frame.grid.reach,
                vertical_axis: u32::from(frame.grid.vertical_axis),
                _pad: 0,
            };
            self.fullscreen(
                ctx,
                &recipes::grid(&self.mesh.builtin, view),
                behind,
                bytemuck::bytes_of(&push),
            )?;
        }
        self.draw_direct(
            ctx,
            built,
            addresses,
            &built.overlay,
            DirectPass::Shade(view),
        )
    }

    fn restore(&mut self, ctx: &mut GpuFrameCtx<'_>, view: ViewFormats, source: u32) -> Result<()> {
        let recipe = recipes::restore(&self.mesh.builtin, view);
        self.fullscreen(
            ctx,
            &recipe,
            DrawState::NO_DEPTH,
            bytemuck::bytes_of(&[source, 0u32]),
        )
    }

    fn shade_bucket(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
        view: ViewFormats,
        index: usize,
    ) -> Result<()> {
        let bucket = &built.buckets[index];
        let index = index as u32;
        let recipe = recipes::shade(
            &self.mesh.builtin,
            &bucket.program,
            bucket.hull,
            bucket.state,
            bucket.features,
            view,
        );
        let pipeline = self.pipeline(&recipe)?;
        let (draw, flags) = match bucket.hull {
            false => (
                DrawState {
                    depth_write: false,
                    ..DrawState::from_material(bucket.state, vk::CompareOp::EQUAL)
                },
                0,
            ),
            true => (
                DrawState::from_material(bucket.state, vk::CompareOp::GREATER_OR_EQUAL),
                match bucket.pass {
                    MaterialPass::Masked => PASS_MASKED,
                    _ => 0,
                },
            ),
        };
        self.bind(ctx, pipeline, draw);
        self.push(
            ctx,
            &GpuPassPush {
                frame: addresses.refs,
                slot: 0,
                flags,
            },
        );
        for range in [Some(index), built.layout.late(index)]
            .into_iter()
            .flatten()
        {
            self.draw_range(
                ctx,
                addresses,
                range,
                built.bucket_table[range as usize],
                bucket.indices,
            );
        }
        Ok(())
    }
}

fn blends_over_opaque(stage: Stage<'_>) -> bool {
    let built = stage.built;
    !built.transparent.is_empty()
        || !built.overlay.is_empty()
        || stage.frame.grid.enabled
        || stage.look.is_some_and(particles_drawn)
        || !stage.emitters.is_empty()
}

fn capture_opaque(graph: &mut FrameGraph<'_>, chain: &mut Chain) -> ImageId {
    chain.filter(
        graph,
        "rikka.three.temporal.opaque",
        [],
        move |record, ctx, _, source| {
            let recipe = recipes::filter(
                &record.mesh.builtin,
                &record.mesh.builtin.copy_frag,
                Blend::Opaque,
                COLOR_FORMAT,
            );
            record.fullscreen(
                ctx,
                &recipe,
                DrawState::NO_DEPTH,
                bytemuck::bytes_of(&[source, 0u32]),
            )
        },
    );
    chain.current()
}
