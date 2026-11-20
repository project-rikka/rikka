use std::sync::Arc;
use std::time::Instant;

use ash::vk;

use crate::draw::batcher::Ledgered;
use rikka_gpu::core::buffer::{BufferBinding, GpuBuffer};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame::MAX_FRAMES_IN_FLIGHT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;
use rikka_gpu::core::provision::Provision;
use rikka_gpu::core::runtime::UNUSED_FOR;
use rikka_gpu::error::Result;

use super::PrimitivePipeline;
use super::upload::{InstanceUpload, send};

const SHRINK_BELOW: vk::DeviceSize = 4;

pub(super) struct StoreBuffer {
    buffer: GpuBuffer,
    pub(super) epoch: u64,
    version: u64,
    ahead: Option<Provision<GpuBuffer>>,
}

pub(super) struct RingSlot {
    buffer: GpuBuffer,
    epoch: u64,
    version: u64,
}

#[derive(Default)]
pub(super) struct RingTable {
    slots: [Option<RingSlot>; MAX_FRAMES_IN_FLIGHT],
}

impl RingTable {
    pub(super) fn refresh<T: bytemuck::Pod>(
        &mut self,
        device: &Arc<RenderDevice>,
        ctx: &mut GpuFrameCtx<'_>,
        name: &'static str,
        retained_scene: bool,
        ledgered: &Ledgered<'_, T>,
    ) -> Result<(BufferBinding, usize)> {
        let stride = size_of::<T>();
        let required =
            (size_of_val(ledgered.data).max(stride) as vk::DeviceSize).next_power_of_two();
        let ledger = ledgered.ledger;
        let slot = &mut self.slots[ctx.frame_slot];
        let fits = |entry: &RingSlot| {
            let size = entry.buffer.size();
            size >= required && size / SHRINK_BELOW <= required
        };
        if slot.as_ref().is_none_or(|entry| !fits(entry)) {
            let buffer = GpuBuffer::new(
                device.clone(),
                name,
                required,
                vk::BufferUsageFlags::STORAGE_BUFFER,
                Placement::Upload,
            )?;
            if let Some(old) = slot.replace(RingSlot {
                buffer,
                epoch: 0,
                version: 0,
            }) {
                ctx.retire.defer(ctx.frame_value, Box::new(old.buffer));
            }
        }
        let entry = slot.as_mut().expect("ring slot exists");
        let mapped = entry
            .buffer
            .mapped_slice_mut()
            .expect("ring buffer is host-visible");
        let whole =
            !retained_scene || entry.epoch != ledger.epoch || entry.version < ledger.since();
        let bytes: &[u8] = bytemuck::cast_slice(ledgered.data);
        let mut sent = 0;
        if whole {
            mapped[..bytes.len()].copy_from_slice(bytes);
            sent = bytes.len();
        } else {
            for change in ledger.changes_after(entry.version) {
                let from = change.first as usize * stride;
                let to = ((change.first + change.count) as usize * stride).min(bytes.len());
                if from < to {
                    mapped[from..to].copy_from_slice(&bytes[from..to]);
                    sent += to - from;
                }
            }
        }
        entry.epoch = ledger.epoch;
        entry.version = ledger.version;
        Ok((entry.buffer.binding(), sent))
    }

    pub(super) fn reset_epochs(&mut self) {
        for slot in self.slots.iter_mut().flatten() {
            slot.epoch = 0;
        }
    }
}

pub(crate) struct Store {
    pub(super) instances: Option<StoreBuffer>,
    pub(super) groups: RingTable,
    pub(super) clips: RingTable,
    pub(super) shown: RingTable,
    pub(super) captured: RingTable,
    pub(super) moves: RingTable,
    pub(super) extras: RingTable,
    pub(super) last_used: Instant,
}

impl Store {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            instances: None,
            groups: RingTable::default(),
            clips: RingTable::default(),
            shown: RingTable::default(),
            captured: RingTable::default(),
            moves: RingTable::default(),
            extras: RingTable::default(),
            last_used: now,
        }
    }
}

pub(super) fn refresh<T: bytemuck::Pod>(
    device: &Arc<RenderDevice>,
    ctx: &mut GpuFrameCtx<'_>,
    sending: &mut InstanceUpload,
    buffer_slot: &mut Option<StoreBuffer>,
    (name, retained_scene): (&'static str, bool),
    ledgered: &Ledgered<'_, T>,
) -> Result<(BufferBinding, usize)> {
    let stride = size_of::<T>();
    let required = (size_of_val(ledgered.data).max(stride) as vk::DeviceSize).next_power_of_two();
    let ledger = ledgered.ledger;
    let fits = |buffer_slot: &StoreBuffer| {
        let size = buffer_slot.buffer.size();
        size >= required && size / SHRINK_BELOW <= required
    };
    let relocation = ledger.relocation.as_ref().filter(|relocation| {
        retained_scene
            && buffer_slot.as_ref().is_some_and(|buffer_slot| {
                buffer_slot.epoch == relocation.from
                    && buffer_slot.version + 1 == relocation.version
            })
    });
    let created = buffer_slot
        .as_ref()
        .is_none_or(|buffer_slot| !fits(buffer_slot));
    let mut shuffled = false;
    if !created && let Some(relocation) = relocation {
        let buffer_slot = buffer_slot.as_mut().expect("relocated store has a buffer");
        shuffled = sending.shuffle.apply(
            device,
            ctx,
            &buffer_slot.buffer,
            &relocation.moves,
            stride as vk::DeviceSize,
        )?;
        buffer_slot.epoch = ledger.epoch;
    }
    if created {
        let ahead = buffer_slot
            .as_mut()
            .and_then(|buffer_slot| buffer_slot.ahead.take());
        let buffer = match provided(ctx, ahead, required) {
            Some(buffer) => buffer,
            None => store_buffer(device.clone(), name, required)?,
        };
        let fresh = StoreBuffer {
            buffer,
            epoch: 0,
            version: 0,
            ahead: None,
        };
        if let Some(old) = buffer_slot.replace(fresh) {
            let current =
                retained_scene && old.epoch == ledger.epoch && old.version >= ledger.since();
            let copies: Vec<vk::BufferCopy> = match relocation {
                Some(relocation) => relocation
                    .moves
                    .iter()
                    .map(|moved| {
                        let unit = |at: u32| vk::DeviceSize::from(at) * stride as vk::DeviceSize;
                        vk::BufferCopy {
                            src_offset: unit(moved.from),
                            dst_offset: unit(moved.to),
                            size: unit(moved.count),
                        }
                    })
                    .collect(),
                None if current => vec![vk::BufferCopy {
                    src_offset: 0,
                    dst_offset: 0,
                    size: old.buffer.size(),
                }],
                None => Vec::new(),
            };
            let fresh = buffer_slot.as_mut().expect("buffer exists");
            if copy_contents(ctx, &old.buffer, &fresh.buffer, copies) {
                fresh.epoch = ledger.epoch;
                fresh.version = old.version;
            }
            ctx.retire.defer(ctx.frame_value, Box::new(old.buffer));
        }
    }
    let buffer_slot = buffer_slot.as_mut().expect("buffer exists");
    let whole = !retained_scene
        || buffer_slot.epoch != ledger.epoch
        || buffer_slot.version < ledger.since();
    sending.spans.clear();
    match whole {
        true => sending.spans.push((0, ledgered.data.len() as u32)),
        false => sending.spans.extend(
            ledger
                .changes_after(buffer_slot.version)
                .map(|change| (change.first, change.first + change.count)),
        ),
    }
    let needs_pre_barrier = !created && !shuffled;
    let sent = send(
        ctx,
        sending,
        ledgered.data,
        &buffer_slot.buffer,
        needs_pre_barrier,
    )?;
    buffer_slot.epoch = ledger.epoch;
    buffer_slot.version = ledger.version;
    let filled = size_of_val(ledgered.data) as vk::DeviceSize;
    if buffer_slot.ahead.is_none() && nearly_full(filled, buffer_slot.buffer.size()) {
        let size = buffer_slot.buffer.size() * 2;
        buffer_slot.ahead = Some(Provision::start(device, move |device| {
            store_buffer(device, name, size)
        }));
    }
    Ok((buffer_slot.buffer.binding(), sent))
}

fn nearly_full(filled: vk::DeviceSize, size: vk::DeviceSize) -> bool {
    filled * 4 > size * 3
}

fn store_buffer(device: Arc<RenderDevice>, name: &str, size: vk::DeviceSize) -> Result<GpuBuffer> {
    GpuBuffer::new(
        device,
        name,
        size,
        vk::BufferUsageFlags::STORAGE_BUFFER
            | vk::BufferUsageFlags::TRANSFER_SRC
            | vk::BufferUsageFlags::TRANSFER_DST,
        Placement::Device,
    )
}

fn provided(
    ctx: &mut GpuFrameCtx<'_>,
    ahead: Option<Provision<GpuBuffer>>,
    required: vk::DeviceSize,
) -> Option<GpuBuffer> {
    match ahead?.take()? {
        Ok(buffer) if buffer.size() >= required && buffer.size() / SHRINK_BELOW <= required => {
            Some(buffer)
        }
        Ok(buffer) => {
            ctx.retire.defer(ctx.frame_value, Box::new(buffer));
            None
        }
        Err(error) => {
            tracing::debug!("failed to preallocate store buffer: {error}");
            None
        }
    }
}

fn copy_contents(
    ctx: &mut GpuFrameCtx<'_>,
    from: &GpuBuffer,
    to: &GpuBuffer,
    mut copies: Vec<vk::BufferCopy>,
) -> bool {
    for copy in &mut copies {
        copy.size = copy
            .size
            .min(from.size().saturating_sub(copy.src_offset))
            .min(to.size().saturating_sub(copy.dst_offset));
    }
    copies.retain(|copy| copy.size > 0);
    if copies.is_empty() {
        return false;
    }
    let barrier =
        |buffer: &GpuBuffer,
         (from_stage, from_access): (vk::PipelineStageFlags2, vk::AccessFlags2),
         (to_stage, to_access): (vk::PipelineStageFlags2, vk::AccessFlags2)| {
            vk::BufferMemoryBarrier2::default()
                .src_stage_mask(from_stage)
                .src_access_mask(from_access)
                .dst_stage_mask(to_stage)
                .dst_access_mask(to_access)
                .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
                .buffer(buffer.handle())
                .offset(0)
                .size(vk::WHOLE_SIZE)
        };
    let written = (
        vk::PipelineStageFlags2::COPY,
        vk::AccessFlags2::TRANSFER_WRITE,
    );
    let before = [barrier(
        from,
        written,
        (
            vk::PipelineStageFlags2::COPY,
            vk::AccessFlags2::TRANSFER_READ,
        ),
    )];
    let after = [barrier(
        to,
        written,
        (
            vk::PipelineStageFlags2::COPY | vk::PipelineStageFlags2::VERTEX_SHADER,
            vk::AccessFlags2::TRANSFER_WRITE | vk::AccessFlags2::SHADER_STORAGE_READ,
        ),
    )];
    ctx.begin_pass("rikka.instances.copy_contents");
    let device = ctx.device.handle();
    unsafe {
        device.cmd_pipeline_barrier2(
            ctx.cmd,
            &vk::DependencyInfo::default().buffer_memory_barriers(&before),
        );
        device.cmd_copy_buffer(ctx.cmd, from.handle(), to.handle(), &copies);
        device.cmd_pipeline_barrier2(
            ctx.cmd,
            &vk::DependencyInfo::default().buffer_memory_barriers(&after),
        );
    }
    ctx.end_pass();
    true
}

impl PrimitivePipeline {
    pub(super) fn release_idle_stores(&mut self, ctx: &mut GpuFrameCtx<'_>, now: Instant) {
        let frame_value = ctx.frame_value;
        let expired = self
            .stores
            .extract_if(|_, store| now.duration_since(store.last_used) >= UNUSED_FOR);
        for (_, store) in expired {
            ctx.retire.defer(frame_value, Box::new(store));
        }
    }
}
