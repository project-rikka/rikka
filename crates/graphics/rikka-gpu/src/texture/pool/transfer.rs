use std::sync::Arc;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::frame::FrameTimeline;
use crate::core::frame_ctx::GpuFrameCtx;
use crate::core::image::{ImageAccess, Texels, record_barriers};
use crate::core::transfer::TransferLane;
use crate::error::Result;
use crate::texture::copies::{copy_region, copy_wave, rewrite_barrier};
use crate::texture::pending_upload::Fill;
use crate::texture::ring::Retire;

use super::TexturePool;

impl TexturePool {
    pub fn take_over(&mut self, ctx: &mut GpuFrameCtx<'_>, sampled: impl IntoIterator<Item = u32>) {
        self.taken.clear();
        if self.foreign.is_empty() && self.streams.is_empty() {
            return;
        }
        let mut arrival = 0;
        for slot in sampled {
            if let Some(id) = self.foreign.get(&slot) {
                self.taken.push(*id);
            }
            if let Some(entry) = self.streams.get(&slot).and_then(|id| self.textures.get(id)) {
                arrival = arrival.max(entry.arrival);
            }
        }
        if let Some(wait) = self.arrived(arrival, ImageAccess::TABLE_SAMPLED.stage) {
            ctx.waits_while_sampling([wait]);
        }
        if self.taken.is_empty() {
            return;
        }
        self.taken.sort_unstable();
        self.taken.dedup();
        let Self {
            textures,
            taken,
            barriers,
            ..
        } = self;
        barriers.clear();
        barriers.extend(
            taken
                .iter()
                .filter_map(|id| textures.get_mut(id).map(|entry| entry.image.take_over())),
        );
        record_barriers(ctx.device, ctx.cmd, barriers);
    }

    pub(super) fn arrived(
        &self,
        arrival: u64,
        stage: vk::PipelineStageFlags2,
    ) -> Option<vk::SemaphoreSubmitInfo<'static>> {
        let timeline = self.deferred.as_ref()?.timeline();
        (arrival > 0 && !timeline.reached(arrival)).then(|| {
            vk::SemaphoreSubmitInfo::default()
                .semaphore(timeline.handle())
                .value(arrival)
                .stage_mask(stage)
        })
    }

    pub fn extend_release_barriers(&mut self, into: &mut Vec<vk::ImageMemoryBarrier2<'static>>) {
        let textures = &mut self.textures;
        into.extend(self.taken.drain(..).filter_map(|id| {
            textures
                .get_mut(&id)
                .map(|entry| entry.image.release_barrier())
        }));
    }

    pub fn release_barriers(&mut self) -> Vec<vk::ImageMemoryBarrier2<'static>> {
        let mut out = Vec::new();
        self.extend_release_barriers(&mut out);
        out
    }

    pub fn stream(
        &mut self,
        lane: &mut TransferLane,
        frames: &FrameTimeline,
        last_frame: u64,
    ) -> Result<()> {
        lane.collect();
        self.select_streamable();
        if self.streaming.is_empty() {
            return Ok(());
        }
        let waits = match last_frame > 0 && !frames.reached(last_frame) {
            true => vec![
                vk::SemaphoreSubmitInfo::default()
                    .semaphore(frames.handle())
                    .value(last_frame)
                    .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS),
            ],
            false => Vec::new(),
        };
        let cmd = lane.begin()?;
        let device = Arc::clone(lane.device());
        let (entering, leaving) = self.streaming_barriers();
        record_barriers(&device, cmd, &entering);
        self.record_streamed_copies(&device, cmd);
        record_barriers(&device, cmd, &leaving);
        let value = lane.submit(&waits)?;
        self.finish_streaming(value);
        Ok(())
    }

    fn select_streamable(&mut self) {
        self.streaming.clear();
        for upload in &self.pending {
            if matches!(upload.fill, Fill::Staged(_)) && !self.streaming.contains(&upload.id) {
                self.streaming.push(upload.id);
            }
        }
        let (textures, pending, growing) = (&self.textures, &self.pending, &self.growing);
        self.streaming.retain(|id| {
            textures
                .get(id)
                .is_some_and(|entry| entry.shared_by.is_some() && entry.image.mip_levels() == 1)
                && !growing.iter().any(|grown| grown.id == *id)
                && pending
                    .iter()
                    .all(|upload| upload.id != *id || matches!(upload.fill, Fill::Staged(_)))
        });
    }

    fn streaming_barriers(
        &self,
    ) -> (
        Vec<vk::ImageMemoryBarrier2<'static>>,
        Vec<vk::ImageMemoryBarrier2<'static>>,
    ) {
        let mut entering = Vec::with_capacity(self.streaming.len());
        let mut leaving = Vec::with_capacity(self.streaming.len());
        for id in &self.streaming {
            let entry = &self.textures[id];
            let whole = entry.image.extent();
            let fresh = !entry.image.holds_content()
                || self
                    .pending
                    .iter()
                    .any(|upload| upload.id == *id && upload.fills(whole));
            let layout = match fresh {
                true => vk::ImageLayout::UNDEFINED,
                false => entry.image.access().layout,
            };
            let barrier = vk::ImageMemoryBarrier2::default()
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .image(entry.image.handle())
                .subresource_range(entry.image.subresource_range());
            entering.push(
                barrier
                    .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                    .src_access_mask(vk::AccessFlags2::NONE)
                    .dst_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                    .dst_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                    .old_layout(layout)
                    .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL),
            );
            leaving.push(
                barrier
                    .src_stage_mask(vk::PipelineStageFlags2::TRANSFER)
                    .src_access_mask(vk::AccessFlags2::TRANSFER_WRITE)
                    .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                    .dst_access_mask(vk::AccessFlags2::NONE)
                    .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                    .new_layout(ImageAccess::TABLE_SAMPLED.layout),
            );
        }
        (entering, leaving)
    }

    fn record_streamed_copies(&self, device: &RenderDevice, cmd: vk::CommandBuffer) {
        let handle = device.handle();
        for id in &self.streaming {
            let image = self.textures[id].image.handle();
            let mut placed: Vec<(Texels, usize)> = Vec::new();
            for upload in self.pending.iter().filter(|upload| upload.id == *id) {
                let Fill::Staged(span) = &upload.fill else {
                    continue;
                };
                let texels = upload.texels();
                let wave = copy_wave(&placed, texels);
                if wave > placed.iter().map(|(_, wave)| *wave).max().unwrap_or(0) {
                    record_barriers(device, cmd, &[rewrite_barrier(&self.textures[id].image)]);
                }
                placed.push((texels, wave));
                let region = copy_region(span.offset(), upload.offset, upload.extent);
                unsafe {
                    handle.cmd_copy_buffer_to_image(
                        cmd,
                        span.buffer(),
                        image,
                        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                        &[region],
                    )
                };
            }
        }
    }

    fn finish_streaming(&mut self, value: u64) {
        for id in &self.streaming {
            if let Some(entry) = self.textures.get_mut(id) {
                entry.arrival = value;
                entry.image.set_access(ImageAccess::TABLE_SAMPLED);
            }
        }
        let streaming = &self.streaming;
        self.pending.retain_mut(|upload| {
            if !streaming.contains(&upload.id) {
                return true;
            }
            if let Fill::Staged(span) = &mut upload.fill {
                span.retire_after(Retire::Transfer(value));
            }
            false
        });
    }
}
