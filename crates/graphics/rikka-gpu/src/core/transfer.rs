use std::any::Any;
use std::sync::Arc;

use ash::vk;
use rikka_sync::Mutex;

use crate::core::device::{QueueRole, RenderDevice};
use crate::core::frame::FrameTimeline;
use crate::core::ticks::{PASS_TARGET, Ticks};
use crate::error::Result;

const SLOTS: usize = 4;

const TRANSFER_PASS: &str = "rikka.transfer";

struct Slot {
    pool: vk::CommandPool,
    cmd: vk::CommandBuffer,
    value: u64,
    stamped: bool,
}

pub struct TransferLane {
    device: Arc<RenderDevice>,
    family: u32,
    timeline: Arc<FrameTimeline>,
    deferred: DeferredDrop,
    slots: Vec<Slot>,
    queries: vk::QueryPool,
    ticks: Option<Ticks>,
    next: u64,
    open: Option<usize>,
}

type DeferredItems = Vec<(u64, Box<dyn Any + Send>)>;

#[derive(Clone)]
pub struct DeferredDrop {
    timeline: Arc<FrameTimeline>,
    items: Arc<Mutex<DeferredItems>>,
}

impl DeferredDrop {
    fn keep_until(&self, value: u64, item: Box<dyn Any + Send>) {
        if self.timeline.reached(value) {
            return;
        }
        self.items.lock().push((value, item));
    }

    pub(crate) fn drop_after(&self, value: u64, item: Box<dyn Any + Send>) -> Box<dyn Any + Send> {
        Box::new(DropAfter {
            deferred: self.clone(),
            value,
            item: Some(item),
        })
    }

    pub(crate) fn timeline(&self) -> &Arc<FrameTimeline> {
        &self.timeline
    }

    fn collect(&self) {
        let finished = self.timeline.completed();
        self.items.lock().retain(|(value, _)| *value > finished);
    }
}

struct DropAfter {
    deferred: DeferredDrop,
    value: u64,
    item: Option<Box<dyn Any + Send>>,
}

impl Drop for DropAfter {
    fn drop(&mut self) {
        if let Some(item) = self.item.take() {
            self.deferred.keep_until(self.value, item);
        }
    }
}

impl TransferLane {
    pub fn new(device: Arc<RenderDevice>) -> Result<Option<Self>> {
        let Some(family) = device.transfer_family() else {
            return Ok(None);
        };
        let timeline = FrameTimeline::named(device.clone(), "rikka.transfer_timeline")?;
        let ticks = Ticks::new(&device, family).filter(|_| device.resets_queries_on_host());
        let queries = match ticks {
            Some(_) => {
                let info = vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    .query_count(SLOTS as u32 * 2);
                let pool = unsafe { device.handle().create_query_pool(&info, None)? };
                device.set_object_name(pool, "rikka.transfer.passes");
                pool
            }
            None => vk::QueryPool::null(),
        };
        Ok(Some(Self {
            deferred: DeferredDrop {
                timeline: timeline.clone(),
                items: Arc::default(),
            },
            device,
            family,
            timeline,
            slots: Vec::with_capacity(SLOTS),
            queries,
            ticks,
            next: 1,
            open: None,
        }))
    }

    pub(crate) fn device(&self) -> &Arc<RenderDevice> {
        &self.device
    }

    pub fn timeline(&self) -> &Arc<FrameTimeline> {
        &self.timeline
    }

    pub fn deferred(&self) -> DeferredDrop {
        self.deferred.clone()
    }

    pub fn collect(&mut self) {
        self.deferred.collect();
        let finished = self.timeline.completed();
        for index in 0..self.slots.len() {
            if self.slots[index].value <= finished {
                self.report(index);
            }
        }
    }

    pub(crate) fn begin(&mut self) -> Result<vk::CommandBuffer> {
        let index = self.lease()?;
        self.report(index);
        let device = self.device.handle();
        let slot = &mut self.slots[index];
        unsafe {
            device.reset_command_pool(slot.pool, vk::CommandPoolResetFlags::empty())?;
            device.begin_command_buffer(
                slot.cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
        }
        let timed = self.queries != vk::QueryPool::null()
            && tracing::enabled!(target: PASS_TARGET, tracing::Level::DEBUG);
        if timed {
            let first = index as u32 * 2;
            unsafe {
                device.reset_query_pool(self.queries, first, 2);
                device.cmd_write_timestamp2(
                    slot.cmd,
                    vk::PipelineStageFlags2::ALL_COMMANDS,
                    self.queries,
                    first,
                );
            }
        }
        slot.stamped = timed;
        self.open = Some(index);
        Ok(slot.cmd)
    }

    pub(crate) fn submit(&mut self, waits: &[vk::SemaphoreSubmitInfo<'_>]) -> Result<u64> {
        let index = self.open.take().expect("transfer is recording");
        let device = self.device.handle();
        let slot = &mut self.slots[index];
        unsafe {
            if slot.stamped {
                device.cmd_write_timestamp2(
                    slot.cmd,
                    vk::PipelineStageFlags2::ALL_COMMANDS,
                    self.queries,
                    index as u32 * 2 + 1,
                );
            }
            device.end_command_buffer(slot.cmd)?;
        }
        let value = self.next;
        let signals = [vk::SemaphoreSubmitInfo::default()
            .semaphore(self.timeline.handle())
            .value(value)
            .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)];
        let commands = [vk::CommandBufferSubmitInfo::default().command_buffer(slot.cmd)];
        let batch = vk::SubmitInfo2::default()
            .wait_semaphore_infos(waits)
            .command_buffer_infos(&commands)
            .signal_semaphore_infos(&signals);
        let submitted = self
            .device
            .submit(QueueRole::Transfer, &[batch], vk::Fence::null());
        if let Err(error) = submitted {
            slot.stamped = false;
            return Err(error);
        }
        slot.value = value;
        self.next += 1;
        Ok(value)
    }

    pub(crate) fn abandon(&mut self) {
        if let Some(index) = self.open.take() {
            let slot = &mut self.slots[index];
            slot.stamped = false;
            unsafe {
                let _ = self.device.handle().end_command_buffer(slot.cmd);
            }
        }
    }

    fn lease(&mut self) -> Result<usize> {
        let finished = self.timeline.finished()?;
        if let Some(free) = self.slots.iter().position(|slot| slot.value <= finished) {
            return Ok(free);
        }
        if self.slots.len() < SLOTS {
            let pool = unsafe {
                self.device.handle().create_command_pool(
                    &vk::CommandPoolCreateInfo::default()
                        .flags(vk::CommandPoolCreateFlags::TRANSIENT)
                        .queue_family_index(self.family),
                    None,
                )?
            };
            let allocated = unsafe {
                self.device.handle().allocate_command_buffers(
                    &vk::CommandBufferAllocateInfo::default()
                        .command_pool(pool)
                        .level(vk::CommandBufferLevel::PRIMARY)
                        .command_buffer_count(1),
                )
            };
            let cmd = match allocated {
                Ok(buffers) => buffers[0],
                Err(error) => {
                    unsafe { self.device.handle().destroy_command_pool(pool, None) };
                    return Err(error.into());
                }
            };
            let index = self.slots.len();
            self.device
                .set_object_name(pool, &format!("rikka.transfer{index}"));
            self.slots.push(Slot {
                pool,
                cmd,
                value: 0,
                stamped: false,
            });
            return Ok(index);
        }
        let (oldest, value) = self
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| (index, slot.value))
            .min_by_key(|&(_, value)| value)
            .expect("transfer lane has slots");
        self.timeline.wait(value)?;
        Ok(oldest)
    }

    fn report(&mut self, index: usize) {
        let slot = &mut self.slots[index];
        let (true, Some(ticks)) = (std::mem::take(&mut slot.stamped), self.ticks) else {
            return;
        };
        let mut stamps = [0u64; 2];
        let read = unsafe {
            self.device.handle().get_query_pool_results(
                self.queries,
                index as u32 * 2,
                &mut stamps,
                vk::QueryResultFlags::TYPE_64,
            )
        };
        if read.is_ok() {
            ticks.report(TRANSFER_PASS, slot.value, stamps);
        }
    }
}

impl Drop for TransferLane {
    fn drop(&mut self) {
        self.abandon();
        if self.next > 1 && self.timeline.wait(self.next - 1).is_err() {
            self.device.wait_idle();
        }
        self.deferred.items.lock().clear();
        unsafe {
            for slot in &self.slots {
                self.device.handle().destroy_command_pool(slot.pool, None);
            }
            if self.queries != vk::QueryPool::null() {
                self.device.handle().destroy_query_pool(self.queries, None);
            }
        }
    }
}

impl std::fmt::Debug for TransferLane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferLane").finish_non_exhaustive()
    }
}

impl std::fmt::Debug for DeferredDrop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeferredDrop").finish_non_exhaustive()
    }
}
