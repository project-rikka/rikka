use std::ops::Range;
use std::sync::Arc;

use ash::vk;
use rikka_text::{CurveBlob, CurveKey, GlyphRaster};

use crate::text::pending_uploads::PendingUploads;
use crate::text::residency::Residency;
use crate::text::word_ranges::WordRanges;
use rikka_gpu::core::buffer::{BufferBinding, GpuBuffer};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;
use rikka_gpu::core::staging::StagedCopies;
use rikka_gpu::error::{Error, Result, Store};

const FIRST_WORDS: u32 = 1 << 15;
const MAX_WORDS: u32 = 1 << 24;
const WORD: vk::DeviceSize = size_of::<u32>() as vk::DeviceSize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CurveSlot {
    pub offset: u32,
    pub bounds: [f32; 4],
}

#[derive(Debug)]
pub(crate) struct EncodedCurves {
    words: Vec<u32>,
    bounds: [f32; 4],
}

impl EncodedCurves {
    pub(crate) fn encode(
        raster: &mut GlyphRaster,
        blob: &mut CurveBlob,
        key: CurveKey,
    ) -> Option<Self> {
        let header = raster.curves(key, blob)?;
        let bounds = header.bounds;
        Some(Self {
            words: blob.words().to_vec(),
            bounds: [bounds.x_min, bounds.y_min, bounds.x_max, bounds.y_max],
        })
    }
}

struct CurveEntry {
    range: Range<u32>,
    slot: CurveSlot,
}

pub(crate) struct CurveStore {
    device: Arc<RenderDevice>,
    curves: Residency<CurveKey, CurveEntry>,
    words: Vec<u32>,
    ranges: WordRanges,
    buffer: Option<GpuBuffer>,
    whole: bool,
    settled: bool,
    spans: PendingUploads<Range<u32>>,
    copies: Vec<vk::BufferCopy>,
    parts: Vec<StagedCopies>,
}

impl CurveStore {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Self {
        let mut ranges = WordRanges::default();
        ranges.grow(FIRST_WORDS);
        Self {
            device,
            curves: Residency::default(),
            words: vec![0; FIRST_WORDS as usize],
            ranges,
            buffer: None,
            whole: false,
            settled: false,
            spans: PendingUploads::default(),
            copies: Vec::new(),
            parts: Vec::new(),
        }
    }

    pub(crate) fn contains(&self, key: &CurveKey) -> bool {
        self.curves.contains(key)
    }

    pub(crate) fn acquire_n(
        &mut self,
        key: CurveKey,
        frame: u64,
        count: u32,
    ) -> Option<Option<CurveSlot>> {
        self.curves
            .acquire_n(key, frame, count)
            .map(|placed| placed.map(|placed| placed.slot))
    }

    pub(crate) fn release_n(&mut self, key: &CurveKey, count: u32) -> bool {
        self.curves.release_n(key, count)
    }

    pub(crate) fn release(&mut self, key: &CurveKey) -> bool {
        self.curves.release(key)
    }

    pub(crate) fn insert(
        &mut self,
        key: CurveKey,
        encoded: Option<EncodedCurves>,
        completed: u64,
        frame: u64,
    ) -> Result<()> {
        if self.curves.contains(&key) {
            return Ok(());
        }
        let Some(encoded) = encoded else {
            self.curves.remember_empty(key, frame, completed);
            return Ok(());
        };
        let range = self.allocate(encoded.words.len() as u32, completed)?;
        self.words[range.start as usize..range.end as usize].copy_from_slice(&encoded.words);
        self.spans.push(range.clone());
        let slot = CurveSlot {
            offset: range.start,
            bounds: encoded.bounds,
        };
        self.curves.place(key, CurveEntry { range, slot }, frame);
        Ok(())
    }

    pub(crate) fn record(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<BufferBinding> {
        let ranges = &mut self.ranges;
        self.curves
            .sweep(ctx.frame_value, ctx.completed_value, |placed| {
                ranges.release(placed.range);
            });
        let needed = self.words.len() as vk::DeviceSize * WORD;
        let mut created = false;
        if self
            .buffer
            .as_ref()
            .is_none_or(|buffer| buffer.size() < needed)
        {
            let buffer = GpuBuffer::new(
                self.device.clone(),
                "rikka.glyph_curves",
                needed,
                vk::BufferUsageFlags::STORAGE_BUFFER
                    | vk::BufferUsageFlags::TRANSFER_DST
                    | vk::BufferUsageFlags::TRANSFER_SRC,
                Placement::Device,
            )?;
            created = true;
            match self.buffer.replace(buffer) {
                Some(old) => {
                    if !self.whole {
                        let new = self.buffer.as_ref().expect("buffer exists");
                        copy_contents(ctx, &old, new);
                    }
                    ctx.retire.defer(ctx.frame_value, Box::new(old));
                }
                None => self.whole = true,
            }
            self.settled = false;
        }
        self.upload(ctx, !created)?;
        Ok(self.buffer.as_ref().expect("store buffer exists").binding())
    }

    pub(crate) fn commit(&mut self) {
        self.spans.commit();
        self.settled = true;
    }

    pub(crate) fn revert(&mut self) {
        self.spans.revert();
        if !self.settled {
            self.whole = true;
        }
    }

    fn allocate(&mut self, len: u32, completed: u64) -> Result<Range<u32>> {
        if let Some(range) = self.ranges.allocate(len) {
            return Ok(range);
        }
        if let Some(range) = self.evict_for(len, completed) {
            return Ok(range);
        }
        let capacity = self.ranges.capacity();
        let most = self.most_words();
        let needed = capacity.saturating_add(len);
        if needed > most {
            return Err(Error::Full(Store::GlyphCurves { words: most }));
        }
        let grown = capacity
            .saturating_mul(2)
            .max(needed)
            .checked_next_power_of_two()
            .map_or(most, |grown| grown.min(most));
        self.words.resize(grown as usize, 0);
        self.ranges.grow(grown);
        tracing::debug!("glyph curve store grew to {grown} words");
        Ok(self
            .ranges
            .allocate(len)
            .expect("grown store fits the blob"))
    }

    fn evict_for(&mut self, len: u32, completed: u64) -> Option<Range<u32>> {
        let mut evictable = self.curves.evictable(completed);
        while let Some(placed) = evictable.next_where(|_| true) {
            if self.ranges.release(placed.range) >= len
                && let Some(range) = self.ranges.allocate(len)
            {
                return Some(range);
            }
        }
        None
    }

    fn most_words(&self) -> u32 {
        (self.device.limits().max_storage_buffer_range / WORD as u32).min(MAX_WORDS)
    }

    fn upload(&mut self, ctx: &mut GpuFrameCtx<'_>, in_use: bool) -> Result<()> {
        if !self.whole && self.spans.pending().is_empty() {
            return Ok(());
        }
        self.copies.clear();
        let push_copy = |copies: &mut Vec<vk::BufferCopy>, span: Range<u32>| {
            if span.is_empty() {
                return;
            }
            let offset = vk::DeviceSize::from(span.start) * WORD;
            copies.push(vk::BufferCopy {
                src_offset: offset,
                dst_offset: offset,
                size: vk::DeviceSize::from(span.end - span.start) * WORD,
            });
        };
        if self.whole {
            push_copy(&mut self.copies, 0..self.ranges.used_end());
        } else {
            let pending = self.spans.pending_mut();
            pending.sort_unstable_by_key(|span| span.start);
            let mut open: Option<Range<u32>> = None;
            for span in pending.iter().filter(|span| !span.is_empty()) {
                match &mut open {
                    Some(current) if span.start <= current.end => {
                        current.end = current.end.max(span.end);
                    }
                    Some(current) => {
                        push_copy(&mut self.copies, current.clone());
                        *current = span.clone();
                    }
                    None => open = Some(span.clone()),
                }
            }
            if let Some(current) = open {
                push_copy(&mut self.copies, current);
            }
        }
        self.whole = false;
        self.spans.mark_sent();
        if self.copies.is_empty() {
            return Ok(());
        }
        ctx.staging.stage_gathered(
            bytemuck::cast_slice(&self.words),
            &mut self.copies,
            &mut self.parts,
        )?;
        let buffer = self
            .buffer
            .as_ref()
            .expect("store buffer exists before upload")
            .handle();
        let before = [barrier(buffer, (READ.0, vk::AccessFlags2::NONE), WRITTEN)];
        let after = [barrier(buffer, WRITTEN, READ)];
        ctx.begin_pass("rikka.glyph_curves");
        let device = ctx.device.handle();
        unsafe {
            if in_use {
                device.cmd_pipeline_barrier2(
                    ctx.cmd,
                    &vk::DependencyInfo::default().buffer_memory_barriers(&before),
                );
            }
            for part in &self.parts {
                device.cmd_copy_buffer(
                    ctx.cmd,
                    part.buffer,
                    buffer,
                    &self.copies[part.copies.clone()],
                );
            }
            device.cmd_pipeline_barrier2(
                ctx.cmd,
                &vk::DependencyInfo::default().buffer_memory_barriers(&after),
            );
        }
        ctx.end_pass();
        Ok(())
    }
}

const READ: (vk::PipelineStageFlags2, vk::AccessFlags2) = (
    vk::PipelineStageFlags2::FRAGMENT_SHADER,
    vk::AccessFlags2::SHADER_STORAGE_READ,
);

const WRITTEN: (vk::PipelineStageFlags2, vk::AccessFlags2) = (
    vk::PipelineStageFlags2::COPY,
    vk::AccessFlags2::TRANSFER_WRITE,
);

fn copy_contents(ctx: &mut GpuFrameCtx<'_>, old: &GpuBuffer, new: &GpuBuffer) {
    let source = [barrier(
        old.handle(),
        WRITTEN,
        (
            vk::PipelineStageFlags2::COPY,
            vk::AccessFlags2::TRANSFER_READ,
        ),
    )];
    let carried = [barrier(
        new.handle(),
        WRITTEN,
        (WRITTEN.0 | READ.0, WRITTEN.1 | READ.1),
    )];
    let copy = vk::BufferCopy {
        src_offset: 0,
        dst_offset: 0,
        size: old.size(),
    };
    let device = ctx.device.handle();
    unsafe {
        device.cmd_pipeline_barrier2(
            ctx.cmd,
            &vk::DependencyInfo::default().buffer_memory_barriers(&source),
        );
        device.cmd_copy_buffer(ctx.cmd, old.handle(), new.handle(), &[copy]);
        device.cmd_pipeline_barrier2(
            ctx.cmd,
            &vk::DependencyInfo::default().buffer_memory_barriers(&carried),
        );
    }
}

fn barrier(
    buffer: vk::Buffer,
    (from_stage, from_access): (vk::PipelineStageFlags2, vk::AccessFlags2),
    (to_stage, to_access): (vk::PipelineStageFlags2, vk::AccessFlags2),
) -> vk::BufferMemoryBarrier2<'static> {
    vk::BufferMemoryBarrier2::default()
        .src_stage_mask(from_stage)
        .src_access_mask(from_access)
        .dst_stage_mask(to_stage)
        .dst_access_mask(to_access)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .buffer(buffer)
        .offset(0)
        .size(vk::WHOLE_SIZE)
}
