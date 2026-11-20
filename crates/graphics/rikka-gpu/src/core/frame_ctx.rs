use std::ffi::CStr;
use std::sync::Arc;

use ash::vk;

use crate::core::barrier::{MemoryAccess, record_memory_barrier};
use crate::core::device::RenderDevice;
use crate::core::resource_table::ResourceTable;
use crate::core::retire::RetireBin;
use crate::core::runtime::Recycler;
use crate::core::sampler::Samplers;
use crate::core::staging::StagingBelt;
use crate::core::ticks::{PASS_TARGET, Ticks};
use crate::error::Result;

const TIMED_PASSES: u32 = 128;

const NESTING: usize = 16;

const LABEL_BYTES: usize = 96;

const FRAME_PASS: &str = "rikka.frame";

pub struct GpuFrameCtx<'a> {
    pub device: &'a Arc<RenderDevice>,
    pub cmd: vk::CommandBuffer,
    pub staging: &'a mut StagingBelt,
    pub resources: &'a mut ResourceTable,
    pub samplers: &'a Samplers,
    pub retire: &'a mut RetireBin,
    pub frame_slot: usize,
    pub frame_value: u64,
    pub completed_value: u64,
    pub(super) timer: &'a mut PassTimer,
    pub(super) setup_waits: &'a mut Vec<vk::SemaphoreSubmitInfo<'static>>,
    pub(super) setup_signals: &'a mut Vec<vk::SemaphoreSubmitInfo<'static>>,
    pub(super) sample_waits: &'a mut Vec<vk::SemaphoreSubmitInfo<'static>>,
    pub(super) entered_canvas: &'a mut bool,
    pub(super) canvas_cmd: vk::CommandBuffer,
}

impl GpuFrameCtx<'_> {
    pub fn memory_barrier(&self, from: MemoryAccess, to: MemoryAccess) {
        record_memory_barrier(self.device, self.cmd, from, to);
    }

    pub fn recycler(&mut self) -> Recycler<'_> {
        Recycler {
            resources: self.resources,
            retire: self.retire,
            value: self.frame_value,
        }
    }
    pub fn begin_pass(&mut self, name: &'static str) {
        self.timer.begin(self.cmd, name);
    }

    pub fn end_pass(&mut self) {
        self.timer.end(self.cmd);
    }

    pub fn waits_on(&mut self, waits: impl IntoIterator<Item = vk::SemaphoreSubmitInfo<'static>>) {
        self.setup_waits.extend(waits);
    }

    pub fn signals_after_setup(
        &mut self,
        signals: impl IntoIterator<Item = vk::SemaphoreSubmitInfo<'static>>,
    ) {
        self.setup_signals.extend(signals);
    }

    pub(crate) fn waits_while_sampling(
        &mut self,
        waits: impl IntoIterator<Item = vk::SemaphoreSubmitInfo<'static>>,
    ) {
        self.sample_waits.extend(waits);
    }

    pub fn begin_canvas(&mut self) {
        *self.entered_canvas = true;
        self.cmd = self.canvas_cmd;
    }
}

pub(crate) struct PassTimer {
    device: Arc<RenderDevice>,
    pool: vk::QueryPool,
    ticks: Option<Ticks>,
    timing: bool,
    passes: Vec<&'static str>,
    open: Vec<Option<u32>>,
    awaiting: bool,
    frame: u64,
    stamps: Vec<u64>,
}

impl PassTimer {
    pub(crate) fn new(device: Arc<RenderDevice>, slot: usize) -> Result<Self> {
        let ticks = Ticks::new(&device, device.queue_family());
        let pool = match ticks {
            Some(_) => {
                let info = vk::QueryPoolCreateInfo::default()
                    .query_type(vk::QueryType::TIMESTAMP)
                    .query_count(TIMED_PASSES * 2);
                let pool = unsafe { device.handle().create_query_pool(&info, None)? };
                device.set_object_name(pool, &format!("rikka.frame{slot}.passes"));
                pool
            }
            None => vk::QueryPool::null(),
        };
        Ok(Self {
            device,
            pool,
            ticks,
            timing: false,
            passes: Vec::with_capacity(TIMED_PASSES as usize),
            open: Vec::with_capacity(NESTING),
            awaiting: false,
            frame: 0,
            stamps: Vec::with_capacity(TIMED_PASSES as usize * 2),
        })
    }

    pub(crate) fn start(&mut self, cmd: vk::CommandBuffer) {
        if self.awaiting {
            self.report();
        }
        self.passes.clear();
        self.open.clear();
        self.awaiting = false;
        self.timing = self.pool != vk::QueryPool::null()
            && tracing::enabled!(target: PASS_TARGET, tracing::Level::DEBUG);
        if self.timing {
            unsafe {
                self.device
                    .handle()
                    .cmd_reset_query_pool(cmd, self.pool, 0, TIMED_PASSES * 2)
            };
            self.passes.push(FRAME_PASS);
            self.stamp(cmd, 0);
        }
    }

    fn begin(&mut self, cmd: vk::CommandBuffer, name: &'static str) {
        if let Some(debug) = self.device.debug_utils() {
            let mut bytes = [0; LABEL_BYTES];
            let info = vk::DebugUtilsLabelEXT::default().label_name(label_of(&mut bytes, name));
            unsafe { debug.cmd_begin_debug_utils_label(cmd, &info) };
        }
        let timed = (self.timing && (self.passes.len() as u32) < TIMED_PASSES).then(|| {
            let index = self.passes.len() as u32;
            self.passes.push(name);
            self.stamp(cmd, index * 2);
            index
        });
        self.open.push(timed);
    }

    fn end(&mut self, cmd: vk::CommandBuffer) {
        let Some(timed) = self.open.pop() else {
            return;
        };
        if let Some(index) = timed {
            self.stamp(cmd, index * 2 + 1);
        }
        if let Some(debug) = self.device.debug_utils() {
            unsafe { debug.cmd_end_debug_utils_label(cmd) };
        }
    }

    fn stamp(&self, cmd: vk::CommandBuffer, query: u32) {
        unsafe {
            self.device.handle().cmd_write_timestamp2(
                cmd,
                vk::PipelineStageFlags2::ALL_COMMANDS,
                self.pool,
                query,
            )
        };
    }

    pub(crate) fn finish(&mut self, cmd: vk::CommandBuffer) {
        if self.timing && !self.passes.is_empty() {
            self.stamp(cmd, 1);
        }
    }

    pub(crate) fn submitted(&mut self, frame: u64) {
        self.frame = frame;
        self.awaiting = !self.passes.is_empty();
    }

    pub(crate) fn abandon(&mut self) {
        self.passes.clear();
        self.open.clear();
        self.awaiting = false;
    }

    fn report(&mut self) {
        let Some(ticks) = self.ticks else {
            return;
        };
        self.stamps.clear();
        self.stamps.resize(self.passes.len() * 2, 0);
        let read = unsafe {
            self.device.handle().get_query_pool_results(
                self.pool,
                0,
                &mut self.stamps,
                vk::QueryResultFlags::TYPE_64,
            )
        };
        if read.is_err() {
            return;
        }
        for (name, pair) in self.passes.iter().zip(self.stamps.as_chunks::<2>().0) {
            ticks.report(name, self.frame, *pair);
        }
    }
}

impl Drop for PassTimer {
    fn drop(&mut self) {
        if self.pool != vk::QueryPool::null() {
            unsafe { self.device.handle().destroy_query_pool(self.pool, None) };
        }
    }
}

fn label_of<'a>(bytes: &'a mut [u8; LABEL_BYTES], name: &str) -> &'a CStr {
    let length = name.len().min(LABEL_BYTES - 1);
    bytes[..length].copy_from_slice(&name.as_bytes()[..length]);
    CStr::from_bytes_until_nul(bytes).expect("the last label byte stays nul")
}

impl<'a> std::fmt::Debug for GpuFrameCtx<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuFrameCtx").finish_non_exhaustive()
    }
}
