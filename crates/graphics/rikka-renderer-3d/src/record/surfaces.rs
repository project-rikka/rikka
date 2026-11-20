use std::hash::{Hash, Hasher};

use ash::vk;
use glam::Mat4;
use rustc_hash::FxHasher;

use crate::build::BuiltFrame;
use crate::draw::Frame;
use crate::error::Result;
use crate::frame_data::FrameAddresses;
use crate::graph::{Attachment, Buffer, BufferUse, FrameGraph, ImageSpec, Size};
use crate::look::Look;
use crate::look::gpu::GpuLook;
use crate::material::MaterialPass;
use crate::pipeline::PassFormats;
use crate::record::Record;
use crate::record::cull::draws_read;
use crate::record::depth::clear_depth;
use crate::record::direct::DirectPass;
use crate::record::stage::{SceneImages, Stage};
use crate::viewport::{DEPTH_FORMAT, IDENTITY_FORMAT, Viewport};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

pub(crate) const SURFACE_FORMAT: vk::Format = vk::Format::R32G32B32A32_UINT;
pub(crate) const LAYERS_FORMAT: vk::Format = IDENTITY_FORMAT;

pub(crate) fn deform_signature(built: &BuiltFrame) -> u64 {
    let mut hasher = FxHasher::default();
    built.deform_vertices.hash(&mut hasher);
    for job in &built.skin_jobs {
        (job.out_offset, job.vertex_count, job.positions_in).hash(&mut hasher);
    }
    hasher.finish()
}

pub(crate) fn surface_formats(layers: bool) -> PassFormats {
    let formats = PassFormats::color(SURFACE_FORMAT, DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1);
    match layers {
        true => formats.with_second(LAYERS_FORMAT, true),
        false => formats,
    }
}

impl Record<'_> {
    pub(super) fn prepare_history(
        &mut self,
        viewport: &Viewport,
        view_projection: Mat4,
        signature: u64,
        frame: &Frame,
        look: Option<&Look>,
        gpu_look: &mut GpuLook,
    ) -> vk::DeviceAddress {
        let temporal = viewport.temporal.is_some();
        if look.is_none() && frame.fluids.is_empty() && !temporal {
            return 0;
        }
        let history = &viewport.history;
        let previous = history.previous_positions(signature).filter(|_| !frame.cut);
        gpu_look.scene.history = u32::from(previous.is_some());
        let (previous_view_projection, previous_time) = match frame.cut {
            true => (view_projection, frame.time),
            false => (
                history.view_projection.unwrap_or(view_projection),
                history.time.unwrap_or(frame.time),
            ),
        };
        gpu_look.scene.previous_view_projection = previous_view_projection.to_cols_array();
        let rotation = look
            .and_then(|look| look.environment.image())
            .map(|image| image.rotation);
        let turn = match (frame.cut, rotation, history.environment_rotation) {
            (false, Some(now), Some(before)) => before - now,
            _ => 0.0,
        };
        gpu_look.scene.previous_sky_view_projection = match turn == 0.0 {
            true => previous_view_projection,
            false => previous_view_projection * Mat4::from_rotation_y(turn),
        }
        .to_cols_array();
        gpu_look.scene.previous_time = previous_time;
        previous.unwrap_or(0)
    }

    pub(super) fn declare_surfaces<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &mut SceneImages,
    ) {
        let depth_spec =
            ImageSpec::new("rikka.three.scene_depth", DEPTH_FORMAT, Size::Viewport).nearest();
        let depth = match stage.temporal {
            Some(_) => {
                scene.previous_depth = Some(graph.create(depth_spec.previous(0)));
                graph.create(depth_spec.current(0))
            }
            None => graph.create(depth_spec),
        };
        let surface = graph.create(
            ImageSpec::new("rikka.three.surface", SURFACE_FORMAT, Size::Viewport).nearest(),
        );
        graph.publish(depth);
        graph.publish(surface);
        let layers = stage.composite.reads().layers.then(|| {
            graph.create(
                ImageSpec::new("rikka.three.layers", LAYERS_FORMAT, Size::Viewport).nearest(),
            )
        });
        let mut pass = graph
            .render("rikka.three.surfaces")
            .depth(Attachment::new(depth, clear_depth()))
            .color(Attachment::clear_color(
                surface,
                vk::ClearColorValue { uint32: [0; 4] },
            ));
        if let Some(layers) = layers {
            pass = pass.color(Attachment::clear_color(
                layers,
                vk::ClearColorValue { uint32: [0; 4] },
            ));
        }
        for (buffer, usage) in draws_read() {
            pass = pass.buffer(buffer, usage);
        }
        pass = pass.buffer(
            Buffer::History,
            BufferUse::storage_read(vk::PipelineStageFlags2::VERTEX_SHADER),
        );
        let built = stage.built;
        let with_layers = layers.is_some();
        pass.record(move |record, ctx, scope| {
            record.surfaces(ctx, built, scope.addresses, with_layers)
        });
        scene.scene_depth = Some(depth);
        scene.surface = Some(surface);
        scene.layers = layers;
    }

    fn surfaces(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
        layers: bool,
    ) -> Result<()> {
        for (index, bucket) in built.buckets.iter().enumerate() {
            if !bucket.pass.in_prepass() || bucket.hull {
                continue;
            }
            let masked = bucket.pass == MaterialPass::Masked;
            self.bind_direct(
                ctx,
                addresses,
                bucket,
                DirectPass::Surface { layers, masked },
            )?;
            for range in [Some(index as u32), built.layout.late(index as u32)]
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
        }
        let written = built.transparent.iter().filter(|draw| {
            let bucket = &built.buckets[draw.bucket as usize];
            bucket.state.depth_write && !bucket.hull
        });
        self.draw_direct(
            ctx,
            built,
            addresses,
            written,
            DirectPass::Surface {
                layers,
                masked: true,
            },
        )
    }
}
