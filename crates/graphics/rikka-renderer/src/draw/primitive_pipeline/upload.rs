use std::time::Instant;

use ash::vk;
use rikka_colorimetry::BlendSpace;

use crate::draw::batcher::Upload;
use crate::draw::instance::GpuInstance;
use crate::draw::shuffle::Shuffle;
use crate::renderer::Colors;
use crate::scene::Viewer;
use rikka_gpu::core::buffer::{BufferBinding, GpuBuffer};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::staging::StagedCopies;
use rikka_gpu::error::Result;

use super::PrimitivePipeline;
use super::store::{Store, refresh};

const BINDINGS: usize = 9;

pub(super) type SetBindings = [BufferBinding; BINDINGS];

pub(crate) struct UploadStats {
    pub set: vk::DescriptorSet,
    pub bytes: usize,
    pub instances: usize,
}

#[derive(Default)]
pub(super) struct InstanceUpload {
    copies: Vec<vk::BufferCopy>,
    pub(super) spans: Vec<(u32, u32)>,
    parts: Vec<StagedCopies>,
    pub(super) shuffle: Shuffle,
}

pub(super) fn send<T: bytemuck::Pod>(
    ctx: &mut GpuFrameCtx<'_>,
    sending: &mut InstanceUpload,
    data: &[T],
    buffer: &GpuBuffer,
    in_use: bool,
) -> Result<usize> {
    let InstanceUpload {
        copies,
        spans,
        parts,
        ..
    } = sending;
    spans.sort_unstable();
    copies.clear();
    let stride = size_of::<T>() as vk::DeviceSize;
    let mut spans = spans.iter().copied().filter(|(from, to)| from < to);
    let Some(mut open) = spans.next() else {
        return Ok(0);
    };
    let mut sent = 0;
    let mut take = |(from, to): (u32, u32)| {
        let size = vk::DeviceSize::from(to - from) * stride;
        copies.push(vk::BufferCopy {
            src_offset: vk::DeviceSize::from(from) * stride,
            dst_offset: vk::DeviceSize::from(from) * stride,
            size,
        });
        sent += size as usize;
    };
    for (from, to) in spans {
        match from <= open.1 {
            true => open.1 = open.1.max(to),
            false => take(std::mem::replace(&mut open, (from, to))),
        }
    }
    take(open);

    ctx.staging
        .stage_gathered(bytemuck::cast_slice(data), copies, parts)?;
    let whole = |from: vk::PipelineStageFlags2,
                 from_access: vk::AccessFlags2,
                 to: vk::PipelineStageFlags2,
                 to_access: vk::AccessFlags2| {
        [vk::BufferMemoryBarrier2::default()
            .src_stage_mask(from)
            .src_access_mask(from_access)
            .dst_stage_mask(to)
            .dst_access_mask(to_access)
            .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
            .buffer(buffer.handle())
            .offset(0)
            .size(vk::WHOLE_SIZE)]
    };
    let readers = vk::PipelineStageFlags2::VERTEX_SHADER;
    ctx.begin_pass("rikka.instances");
    let device = ctx.device.handle();
    unsafe {
        if in_use {
            let before = whole(
                readers,
                vk::AccessFlags2::NONE,
                vk::PipelineStageFlags2::COPY,
                vk::AccessFlags2::TRANSFER_WRITE,
            );
            device.cmd_pipeline_barrier2(
                ctx.cmd,
                &vk::DependencyInfo::default().buffer_memory_barriers(&before),
            );
        }
        for part in parts.iter() {
            device.cmd_copy_buffer(
                ctx.cmd,
                part.buffer,
                buffer.handle(),
                &copies[part.copies.clone()],
            );
        }
        let after = whole(
            vk::PipelineStageFlags2::COPY,
            vk::AccessFlags2::TRANSFER_WRITE,
            readers,
            vk::AccessFlags2::SHADER_STORAGE_READ,
        );
        device.cmd_pipeline_barrier2(
            ctx.cmd,
            &vk::DependencyInfo::default().buffer_memory_barriers(&after),
        );
    }
    ctx.end_pass();
    Ok(sent)
}

impl PrimitivePipeline {
    pub(crate) fn upload(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        upload: &Upload<'_>,
        curves: BufferBinding,
        viewer: Viewer,
        (colors, space): (&Colors, &BlendSpace),
        now: Instant,
    ) -> Result<UploadStats> {
        self.unconfirmed = Some(upload.scene);
        self.release_idle_stores(ctx, now);

        let store = self
            .stores
            .entry(upload.scene)
            .or_insert_with(|| Store::new(now));
        store.last_used = now;
        let retained_scene = upload.scene.is_some();
        let (instance_buffer, instance_bytes) = refresh(
            &self.device,
            ctx,
            &mut self.sending,
            &mut store.instances,
            ("rikka.instances", retained_scene),
            &upload.instances,
        )?;
        let (group_buffer, group_bytes) = store.groups.refresh(
            &self.device,
            ctx,
            "rikka.groups",
            retained_scene,
            &upload.groups,
        )?;
        let (clip_buffer, clip_bytes) = store.clips.refresh(
            &self.device,
            ctx,
            "rikka.group_clips",
            retained_scene,
            &upload.clips,
        )?;
        let (shown_buffer, shown_bytes) = match viewer {
            Viewer::Screen => store.shown.refresh(
                &self.device,
                ctx,
                "rikka.shown",
                retained_scene,
                &upload.shown,
            )?,
            Viewer::Capture => store.captured.refresh(
                &self.device,
                ctx,
                "rikka.captured",
                retained_scene,
                &upload.captured,
            )?,
        };
        let (move_buffer, move_bytes) = store.moves.refresh(
            &self.device,
            ctx,
            "rikka.moves",
            retained_scene,
            &upload.moves,
        )?;
        let (extra_buffer, extra_bytes) = store.extras.refresh(
            &self.device,
            ctx,
            "rikka.node_clips",
            retained_scene,
            &upload.extras,
        )?;
        let (paint_buffer, paint_bytes) = store.paints.refresh(
            &self.device,
            ctx,
            "rikka.node_paints",
            retained_scene,
            &upload.paints,
        )?;
        let conversions = self.colors.bind(&self.device, ctx, colors, space)?;

        let set = self.instance_sets[ctx.frame_slot];
        let bindings = [
            instance_buffer,
            group_buffer,
            shown_buffer,
            clip_buffer,
            curves,
            move_buffer,
            conversions,
            extra_buffer,
            paint_buffer,
        ];
        let written = &mut self.written[ctx.frame_slot];
        if *written != Some(bindings) {
            write_set(&self.device, set, &bindings);
            *written = Some(bindings);
        }
        Ok(UploadStats {
            set,
            bytes: instance_bytes
                + group_bytes
                + clip_bytes
                + shown_bytes
                + move_bytes
                + extra_bytes
                + paint_bytes,
            instances: instance_bytes / size_of::<GpuInstance>(),
        })
    }

    pub(crate) fn revert_uploads(&mut self) {
        if let Some(scene) = self.unconfirmed.take()
            && let Some(store) = self.stores.get_mut(&scene)
        {
            if let Some(buffer_slot) = &mut store.instances {
                buffer_slot.epoch = 0;
            }
            store.groups.reset_epochs();
            store.clips.reset_epochs();
            store.shown.reset_epochs();
            store.captured.reset_epochs();
            store.moves.reset_epochs();
            store.extras.reset_epochs();
            store.paints.reset_epochs();
        }
    }
}

fn write_set(device: &RenderDevice, set: vk::DescriptorSet, bindings: &SetBindings) {
    let infos = bindings.map(|binding| {
        [vk::DescriptorBufferInfo {
            buffer: binding.handle,
            offset: 0,
            range: vk::WHOLE_SIZE,
        }]
    });
    let writes: [vk::WriteDescriptorSet<'_>; BINDINGS] = std::array::from_fn(|binding| {
        vk::WriteDescriptorSet::default()
            .dst_set(set)
            .dst_binding(binding as u32)
            .descriptor_type(vk::DescriptorType::STORAGE_BUFFER)
            .buffer_info(&infos[binding])
    });
    unsafe { device.handle().update_descriptor_sets(&writes, &[]) };
}
