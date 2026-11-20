use ash::vk;
use rustc_hash::FxHashMap;

use crate::compute::ComputedCopy;
use crate::core::convert::YcbcrConverter;
use crate::core::frame_ctx::GpuFrameCtx;
use crate::core::image::{GpuImage, ImageAccess, Texels, record_barriers, record_mip_chains};
use crate::error::Result;
use crate::texture::TextureId;
use crate::texture::copies::{
    computed_region, copy_region, copy_wave, record_copies, rewrite_barrier, wait_on, whole_region,
};
use crate::texture::pending_upload::Fill;
use crate::texture::ring::Retire;
#[cfg(unix)]
use crate::texture::source::TextureSource;
use crate::texture::writes::{CopySource, Writes};

use super::TexturePool;

impl TexturePool {
    #[cfg_attr(
        not(unix),
        expect(unused_variables, reason = "only planar dmabufs convert on upload")
    )]
    pub fn flush(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        converter: &mut YcbcrConverter,
    ) -> Result<Vec<vk::SemaphoreSubmitInfo<'static>>> {
        for former in self.commit() {
            ctx.retire.defer(ctx.frame_value, Box::new(former));
        }
        let waits = self.upload(ctx)?;
        self.sent = std::mem::take(&mut self.pending);

        let committed = std::mem::take(&mut self.committed);
        #[cfg(unix)]
        if let Err(error) = self.convert(ctx, converter, &committed) {
            self.committed = committed;
            return Err(error);
        }
        for id in &committed {
            if let Some(entry) = self.textures.get_mut(id) {
                entry.committed = false;
            }
        }
        self.converted = committed;
        Ok(waits)
    }

    pub fn sent_computed(&self) -> Vec<ComputedCopy> {
        self.sent
            .iter()
            .filter_map(|upload| match &upload.fill {
                Fill::Computed(copy) => Some(ComputedCopy::clone(copy)),
                Fill::Pixels(_) | Fill::Staged(_) | Fill::Blank => None,
            })
            .collect()
    }

    fn upload(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
    ) -> Result<Vec<vk::SemaphoreSubmitInfo<'static>>> {
        let mut waits = Vec::new();
        if self.pending.is_empty() && self.growing.is_empty() {
            return Ok(waits);
        }
        let mut staged: FxHashMap<TextureId, Writes> = FxHashMap::default();
        let reads = self.stage_mip_upgrades(&mut staged);
        self.stage_pending(ctx, &mut staged, &mut waits)?;
        self.sent_grown.append(&mut self.growing);
        let arrival = staged
            .keys()
            .filter_map(|id| self.textures.get(id))
            .map(|entry| entry.arrival)
            .max()
            .unwrap_or(0);
        if let Some(wait) = self.arrived(arrival, vk::PipelineStageFlags2::ALL_COMMANDS) {
            waits.push(wait);
        }

        ctx.begin_pass("rikka.uploads");
        let mut touched: Vec<(&mut GpuImage, &Writes)> = self
            .textures
            .iter_mut()
            .filter_map(|(id, entry)| staged.get(id).map(|batch| (&mut entry.image, batch)))
            .collect();
        open_destinations(ctx, &mut touched, reads);
        clear_blank_destinations(ctx, &touched);
        record_copy_waves(ctx, &touched);
        let mut chains: Vec<(&mut GpuImage, Texels)> = touched
            .into_iter()
            .map(|(image, batch)| {
                let whole = Texels::new([0, 0], [image.extent().width, image.extent().height]);
                (image, batch.texels.unwrap_or(whole))
            })
            .collect();
        record_mip_chains(ctx.device, ctx.cmd, &mut chains, ImageAccess::TABLE_SAMPLED);
        ctx.end_pass();
        Ok(waits)
    }

    fn stage_mip_upgrades(
        &self,
        staged: &mut FxHashMap<TextureId, Writes>,
    ) -> Vec<vk::ImageMemoryBarrier2<'static>> {
        let mut reads = Vec::new();
        for grown in &self.growing {
            let Some(entry) = self.textures.get(&grown.id) else {
                continue;
            };
            let whole = entry.image.extent();
            let texels = Texels::new([0, 0], [whole.width, whole.height]);
            let refilled = self
                .pending
                .iter()
                .any(|upload| upload.id == grown.id && upload.fills(whole));
            let batch = staged.entry(grown.id).or_insert_with(|| Writes {
                fresh: true,
                ..Writes::default()
            });
            batch.texels = Some(texels);
            if refilled || !grown.former.holds_content() {
                continue;
            }
            reads.push(grown.former.read_out());
            batch.placed.push((texels, 0));
            batch.copies.push((
                0,
                CopySource::Image(
                    grown.former.handle(),
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    whole_region(whole),
                ),
            ));
        }
        reads
    }

    fn stage_pending(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        staged: &mut FxHashMap<TextureId, Writes>,
        waits: &mut Vec<vk::SemaphoreSubmitInfo<'static>>,
    ) -> Result<()> {
        for upload in &mut self.pending {
            let Some(entry) = self.textures.get(&upload.id) else {
                continue;
            };
            let whole = entry.image.extent();
            let batch = staged.entry(upload.id).or_insert_with(|| Writes {
                fresh: upload.fills(whole),
                ..Writes::default()
            });
            let texels = upload.texels();
            let (offset, extent) = (upload.offset, upload.extent);
            batch.texels = Some(batch.texels.map_or(texels, |covered| covered.union(texels)));
            let source = match &mut upload.fill {
                Fill::Blank => {
                    batch.clears = true;
                    batch.texels = Some(Texels::new([0, 0], [whole.width, whole.height]));
                    continue;
                }
                Fill::Pixels(pixels) => {
                    let slice = ctx.staging.stage(pixels)?;
                    CopySource::Staged(slice.buffer, copy_region(slice.offset, offset, extent))
                }
                Fill::Staged(span) => {
                    span.retire_after(Retire::Frame(ctx.frame_value));
                    CopySource::Staged(span.buffer(), copy_region(span.offset(), offset, extent))
                }
                Fill::Computed(copy) => {
                    wait_on(waits, copy.wait());
                    CopySource::Image(
                        copy.source(),
                        vk::ImageLayout::GENERAL,
                        computed_region(copy, offset, extent),
                    )
                }
            };
            let wave = copy_wave(&batch.placed, texels).max(usize::from(batch.clears));
            batch.placed.push((texels, wave));
            batch.copies.push((wave, source));
        }
        Ok(())
    }

    #[cfg(unix)]
    fn convert(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        converter: &mut YcbcrConverter,
        committed: &[TextureId],
    ) -> Result<()> {
        let mut released = Vec::new();
        let mut converted = Ok(());
        for id in committed {
            let Some(entry) = self.textures.get_mut(id) else {
                continue;
            };
            if let TextureSource::Planar(planes) = &entry.source {
                let push = planes.convert_push();
                let views = planes.plane_views();
                converted =
                    converter.record(ctx, &[planes.acquire()], &mut entry.image, views, push);
                if converted.is_err() {
                    break;
                }
                released.push(planes.release());
            }
        }
        record_barriers(ctx.device, ctx.cmd, &released);
        converted
    }
}

fn open_destinations(
    ctx: &GpuFrameCtx<'_>,
    touched: &mut [(&mut GpuImage, &Writes)],
    reads: Vec<vk::ImageMemoryBarrier2<'static>>,
) {
    let mut barriers = reads;
    barriers.reserve(touched.len());
    for (image, batch) in touched.iter_mut() {
        if batch.fresh {
            image.discard();
        }
        barriers.extend(image.transition(ImageAccess::TRANSFER_DST));
    }
    record_barriers(ctx.device, ctx.cmd, &barriers);
}

fn clear_blank_destinations(ctx: &GpuFrameCtx<'_>, touched: &[(&mut GpuImage, &Writes)]) {
    let device = ctx.device.handle();
    for (image, batch) in touched {
        if batch.clears {
            unsafe {
                device.cmd_clear_color_image(
                    ctx.cmd,
                    image.handle(),
                    vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                    &vk::ClearColorValue::default(),
                    &[image.subresource_range()],
                )
            };
        }
    }
}

fn record_copy_waves(ctx: &GpuFrameCtx<'_>, touched: &[(&mut GpuImage, &Writes)]) {
    let device = ctx.device.handle();
    let waves = touched
        .iter()
        .flat_map(|(_, batch)| batch.copies.iter().map(|(wave, _)| wave + 1))
        .max()
        .unwrap_or(0);
    for wave in 0..waves {
        let between: Vec<_> = touched
            .iter()
            .filter(|(_, batch)| wave > 0 && batch.copies.iter().any(|copy| copy.0 == wave))
            .map(|(image, _)| rewrite_barrier(image))
            .collect();
        record_barriers(ctx.device, ctx.cmd, &between);
        for (image, batch) in touched {
            let copies: Vec<&CopySource> = batch
                .copies
                .iter()
                .filter(|copy| copy.0 == wave)
                .map(|copy| &copy.1)
                .collect();
            for group in copies.chunk_by(|a, b| a.same_buffer(b)) {
                record_copies(device, ctx.cmd, image.handle(), group);
            }
        }
    }
}
