use std::sync::Arc;
use std::time::Duration;

use ash::vk;

use crate::error::Result;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::ticks::Ticks;

const SLOTS: usize = 4;

#[derive(Clone, Copy, Debug)]
struct Pending {
    frame: u64,
    scale: f32,
}

pub(super) struct ViewTimer {
    device: Arc<RenderDevice>,
    pool: vk::QueryPool,
    ticks: Ticks,
    pending: [Option<Pending>; SLOTS],
    next: usize,
    open: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Measured {
    pub time: Duration,
    pub scale: f32,
}

impl ViewTimer {
    pub(super) fn new(device: &Arc<RenderDevice>) -> Result<Option<Self>> {
        let Some(ticks) = Ticks::new(device, device.queue_family()) else {
            return Ok(None);
        };
        let info = vk::QueryPoolCreateInfo::default()
            .query_type(vk::QueryType::TIMESTAMP)
            .query_count(SLOTS as u32 * 2);
        let pool = unsafe { device.handle().create_query_pool(&info, None)? };
        device.set_object_name(pool, "rikka.three.view_timer");
        Ok(Some(Self {
            device: device.clone(),
            pool,
            ticks,
            pending: [None; SLOTS],
            next: 0,
            open: None,
        }))
    }

    pub(super) fn collect(&mut self, completed: u64, mut observe: impl FnMut(Measured)) {
        for (slot, pending) in self.pending.iter_mut().enumerate() {
            let Some(entry) = *pending else {
                continue;
            };
            if entry.frame > completed {
                continue;
            }
            *pending = None;
            let mut stamps = [0u64; 2];
            let read = unsafe {
                self.device.handle().get_query_pool_results(
                    self.pool,
                    slot as u32 * 2,
                    &mut stamps,
                    vk::QueryResultFlags::TYPE_64,
                )
            };
            if read.is_ok() {
                let nanos = self.ticks.nanos(stamps);
                observe(Measured {
                    time: Duration::from_nanos(nanos as u64),
                    scale: entry.scale,
                });
            }
        }
    }

    pub(super) fn begin(&mut self, ctx: &GpuFrameCtx<'_>) {
        let slot = self.next;
        if self.pending[slot].is_some() {
            self.open = None;
            return;
        }
        self.next = (slot + 1) % SLOTS;
        let first = slot as u32 * 2;
        unsafe {
            let device = self.device.handle();
            device.cmd_reset_query_pool(ctx.cmd, self.pool, first, 2);
            device.cmd_write_timestamp2(
                ctx.cmd,
                vk::PipelineStageFlags2::TOP_OF_PIPE,
                self.pool,
                first,
            );
        }
        self.open = Some(slot);
    }

    pub(super) fn end(&mut self, ctx: &GpuFrameCtx<'_>, scale: f32) {
        let Some(slot) = self.open.take() else {
            return;
        };
        unsafe {
            self.device.handle().cmd_write_timestamp2(
                ctx.cmd,
                vk::PipelineStageFlags2::BOTTOM_OF_PIPE,
                self.pool,
                slot as u32 * 2 + 1,
            );
        }
        self.pending[slot] = Some(Pending {
            frame: ctx.frame_value,
            scale,
        });
    }
}

impl Drop for ViewTimer {
    fn drop(&mut self) {
        unsafe { self.device.handle().destroy_query_pool(self.pool, None) };
    }
}
