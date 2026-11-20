use std::sync::Arc;

use ash::vk;

use crate::error::{Error, Result};
use rikka_gpu::core::barrier::MemoryAccess;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;
use rikka_gpu::error::Store;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Span {
    pub offset: u32,
    pub count: u32,
}

impl Span {
    pub(crate) const EMPTY: Self = Self {
        offset: 0,
        count: 0,
    };

    fn end(self) -> u32 {
        self.offset + self.count
    }
}

#[derive(Debug, Default)]
struct FreeList {
    ranges: Vec<Span>,
}

impl FreeList {
    fn take(&mut self, count: u32) -> Option<u32> {
        let index = self.ranges.iter().position(|span| span.count >= count)?;
        let span = &mut self.ranges[index];
        let offset = span.offset;
        span.offset += count;
        span.count -= count;
        if span.count == 0 {
            self.ranges.remove(index);
        }
        Some(offset)
    }

    fn free(&mut self, span: Span) {
        if span.count == 0 {
            return;
        }
        let at = self
            .ranges
            .partition_point(|free| free.offset < span.offset);
        self.ranges.insert(at, span);

        if at + 1 < self.ranges.len() && self.ranges[at].end() == self.ranges[at + 1].offset {
            self.ranges[at].count += self.ranges[at + 1].count;
            self.ranges.remove(at + 1);
        }
        if at > 0 && self.ranges[at - 1].end() == self.ranges[at].offset {
            self.ranges[at - 1].count += self.ranges[at].count;
            self.ranges.remove(at);
        }
    }

    fn cut_tail(&mut self, watermark: u32) -> Option<u32> {
        let last = self.ranges.last()?;
        if last.end() != watermark {
            return None;
        }
        let offset = last.offset;
        self.ranges.pop();
        Some(offset)
    }
}

#[derive(Debug)]
struct Retiring {
    span: Span,
    after: Option<u64>,
}

#[derive(Debug, Default)]
struct Spans {
    watermark: u32,
    free: FreeList,
    retiring: Vec<Retiring>,
}

impl Spans {
    fn take(&mut self, count: u32, name: &'static str) -> Result<Span> {
        let offset = match self.free.take(count) {
            Some(offset) => offset,
            None => {
                let offset = self.watermark;
                self.watermark = offset
                    .checked_add(count)
                    .ok_or_else(|| Error::full(Store::Arena(name)))?;
                offset
            }
        };
        Ok(Span { offset, count })
    }

    fn free(&mut self, span: Span) {
        if span.count == 0 {
            return;
        }
        if span.end() != self.watermark {
            self.free.free(span);
            return;
        }
        self.watermark = span.offset;
        if let Some(offset) = self.free.cut_tail(self.watermark) {
            self.watermark = offset;
        }
    }

    fn retire(&mut self, span: Span) {
        if span.count > 0 {
            self.retiring.push(Retiring { span, after: None });
        }
    }

    fn reclaim(&mut self, frame_value: u64, completed: u64) {
        let mut released = Vec::new();
        self.retiring.retain_mut(|retiring| {
            let after = *retiring.after.get_or_insert(frame_value);
            if after > completed {
                return true;
            }
            released.push(retiring.span);
            false
        });
        for span in released {
            self.free(span);
        }
    }
}

struct Upload {
    offset: u32,
    data: Vec<u8>,
}

pub(crate) struct Lane {
    device: Arc<RenderDevice>,
    name: &'static str,
    stride: u32,
    usage: vk::BufferUsageFlags,
    buffer: GpuBuffer,
    capacity: u32,
    pending: Vec<Upload>,
    copy_forward: Option<GpuBuffer>,
    overwrites: bool,
}

const INITIAL_CAPACITY: u32 = 4096;

impl Lane {
    fn new(
        device: Arc<RenderDevice>,
        name: &'static str,
        stride: u32,
        usage: vk::BufferUsageFlags,
    ) -> Result<Self> {
        let buffer = Self::allocate(&device, name, stride, usage, INITIAL_CAPACITY)?;
        Ok(Self {
            device,
            name,
            stride,
            usage,
            buffer,
            capacity: INITIAL_CAPACITY,
            pending: Vec::new(),
            copy_forward: None,
            overwrites: false,
        })
    }

    fn allocate(
        device: &Arc<RenderDevice>,
        name: &'static str,
        stride: u32,
        usage: vk::BufferUsageFlags,
        capacity: u32,
    ) -> rikka_gpu::Result<GpuBuffer> {
        GpuBuffer::addressable(
            device.clone(),
            name,
            u64::from(capacity) * u64::from(stride),
            usage | vk::BufferUsageFlags::TRANSFER_DST | vk::BufferUsageFlags::TRANSFER_SRC,
            Placement::Device,
        )
    }

    pub(crate) fn address(&self) -> vk::DeviceAddress {
        self.buffer.device_address()
    }

    pub(crate) fn handle(&self) -> vk::Buffer {
        self.buffer.handle()
    }

    fn count_of(&self, data: &[u8]) -> Result<u32> {
        let stride = self.stride as usize;
        if !data.len().is_multiple_of(stride) {
            return Err(Error::Invalid(format!(
                "{}: {} bytes is not a multiple of the {stride}-byte element size",
                self.name,
                data.len()
            )));
        }
        u32::try_from(data.len() / stride).map_err(|_| Error::full(Store::Arena(self.name)))
    }

    fn reserve(&mut self, end: u32) -> Result<()> {
        if end <= self.capacity {
            return Ok(());
        }
        let capacity = end
            .checked_next_power_of_two()
            .unwrap_or(u32::MAX)
            .max(INITIAL_CAPACITY);
        let replacement =
            Self::allocate(&self.device, self.name, self.stride, self.usage, capacity)?;
        let previous = std::mem::replace(&mut self.buffer, replacement);
        if self.copy_forward.is_none() {
            self.copy_forward = Some(previous);
        }
        self.capacity = capacity;
        Ok(())
    }

    fn stage(&mut self, offset: u32, data: &[u8]) {
        self.pending.push(Upload {
            offset,
            data: data.to_vec(),
        });
    }

    fn cancel(&mut self, offset: u32) {
        self.pending.retain(|upload| upload.offset != offset);
    }

    fn flush(&mut self, ctx: &mut GpuFrameCtx<'_>, watermark: u32) -> Result<()> {
        if std::mem::take(&mut self.overwrites) {
            ctx.memory_barrier(drawn_before(), copied());
        }
        if let Some(previous) = self.copy_forward.take() {
            let bytes = u64::from(watermark) * u64::from(self.stride);
            if bytes > 0 {
                let region = vk::BufferCopy {
                    src_offset: 0,
                    dst_offset: 0,
                    size: bytes.min(previous.size()),
                };
                unsafe {
                    ctx.device.handle().cmd_copy_buffer(
                        ctx.cmd,
                        previous.handle(),
                        self.buffer.handle(),
                        &[region],
                    );
                }
            }
            ctx.retire.defer(ctx.frame_value, Box::new(previous));

            ctx.memory_barrier(copied(), copied());
        }

        for upload in std::mem::take(&mut self.pending) {
            let slice = ctx.staging.stage(&upload.data)?;
            let region = vk::BufferCopy {
                src_offset: slice.offset,
                dst_offset: u64::from(upload.offset) * u64::from(self.stride),
                size: upload.data.len() as vk::DeviceSize,
            };
            unsafe {
                ctx.device.handle().cmd_copy_buffer(
                    ctx.cmd,
                    slice.buffer,
                    self.buffer.handle(),
                    &[region],
                );
            }
        }
        Ok(())
    }

    fn has_pending(&self) -> bool {
        !self.pending.is_empty() || self.copy_forward.is_some()
    }
}

pub(crate) struct GpuArena<const LANES: usize> {
    name: &'static str,
    spans: Spans,
    lanes: [Lane; LANES],
}

pub(crate) struct LaneSpec {
    pub name: &'static str,
    pub stride: u32,
    pub usage: vk::BufferUsageFlags,
}

impl<const LANES: usize> GpuArena<LANES> {
    pub(crate) fn new(device: &Arc<RenderDevice>, lanes: [LaneSpec; LANES]) -> Result<Self> {
        let name = lanes.first().map_or("rikka.three.arena", |lane| lane.name);
        let mut built = Vec::with_capacity(LANES);
        for lane in lanes {
            built.push(Lane::new(
                device.clone(),
                lane.name,
                lane.stride,
                lane.usage,
            )?);
        }
        let Ok(lanes) = built.try_into() else {
            unreachable!("one lane per spec")
        };
        Ok(Self {
            name,
            spans: Spans::default(),
            lanes,
        })
    }

    pub(crate) fn lane(&self, lane: usize) -> &Lane {
        &self.lanes[lane]
    }

    pub(crate) fn insert(&mut self, data: [&[u8]; LANES]) -> Result<Span> {
        let mut count = None;
        for (lane, bytes) in self.lanes.iter().zip(data) {
            let elements = lane.count_of(bytes)?;
            if *count.get_or_insert(elements) != elements {
                return Err(Error::Invalid(format!(
                    "{}: all lanes need the same element count",
                    self.name
                )));
            }
        }
        let count = count.unwrap_or(0);
        if count == 0 {
            return Ok(Span::EMPTY);
        }

        let span = self.spans.take(count, self.name)?;
        for lane in &mut self.lanes {
            if let Err(error) = lane.reserve(span.end()) {
                self.spans.free(span);
                return Err(error);
            }
        }
        for (lane, bytes) in self.lanes.iter_mut().zip(data) {
            lane.stage(span.offset, bytes);
        }
        Ok(span)
    }

    pub(crate) fn cancel(&mut self, span: Span) {
        if span.count == 0 {
            return;
        }
        for lane in &mut self.lanes {
            lane.cancel(span.offset);
        }
        self.spans.free(span);
    }

    pub(crate) fn overwrite(
        &mut self,
        lane: usize,
        span: Span,
        first: u32,
        bytes: &[u8],
    ) -> Result<()> {
        let lane = &mut self.lanes[lane];
        let count = lane.count_of(bytes)?;
        if u64::from(first) + u64::from(count) > u64::from(span.count) {
            return Err(Error::Invalid(format!(
                "{}: write ends at element {} of {}",
                lane.name,
                u64::from(first) + u64::from(count),
                span.count
            )));
        }
        if count > 0 {
            lane.stage(span.offset + first, bytes);
            lane.overwrites = true;
        }
        Ok(())
    }

    pub(crate) fn retire(&mut self, span: Span) {
        self.spans.retire(span);
    }

    pub(crate) fn flush(&mut self, ctx: &mut GpuFrameCtx<'_>) -> Result<()> {
        self.spans.reclaim(ctx.frame_value, ctx.completed_value);
        let watermark = self.spans.watermark;
        for lane in &mut self.lanes {
            lane.flush(ctx, watermark)?;
        }
        Ok(())
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.lanes.iter().any(Lane::has_pending)
    }
}

impl GpuArena<1> {
    pub(crate) fn single(
        device: &Arc<RenderDevice>,
        name: &'static str,
        stride: u32,
        usage: vk::BufferUsageFlags,
    ) -> Result<Self> {
        Self::new(
            device,
            [LaneSpec {
                name,
                stride,
                usage,
            }],
        )
    }

    pub(crate) fn address(&self) -> vk::DeviceAddress {
        self.lanes[0].address()
    }

    pub(crate) fn handle(&self) -> vk::Buffer {
        self.lanes[0].handle()
    }
}

pub(crate) fn drawn_before() -> MemoryAccess {
    MemoryAccess::new(
        vk::PipelineStageFlags2::ALL_COMMANDS,
        vk::AccessFlags2::NONE,
    )
}

pub(crate) fn copied() -> MemoryAccess {
    MemoryAccess::new(
        vk::PipelineStageFlags2::COPY,
        vk::AccessFlags2::TRANSFER_WRITE,
    )
}
