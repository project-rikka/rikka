#[cfg(unix)]
use std::os::fd::OwnedFd;
use std::sync::Arc;
use std::time::Duration;

use ash::vk;

use crate::core::device::RenderDevice;
use crate::core::frame::{FrameRing, FrameTimeline};
use crate::core::frame_ctx::GpuFrameCtx;
use crate::core::resource_table::{ResourceTable, TextureSlots};
use crate::core::retire::RetireBin;
use crate::core::sampler::Samplers;
#[cfg(unix)]
use crate::drm::{AcquireFences, Readiness, wait_signalled};
use crate::error::Result;

pub const UNUSED_FOR: Duration = Duration::from_secs(3);

pub struct BindPoint<'a> {
    pub device: &'a Arc<RenderDevice>,
    pub resources: &'a mut ResourceTable,
    pub samplers: &'a Samplers,
}

pub struct Recycler<'a> {
    pub resources: &'a mut ResourceTable,
    pub retire: &'a mut RetireBin,
    pub value: u64,
}

pub struct GpuRuntime {
    #[cfg(unix)]
    readiness: Option<Readiness>,
    frames: FrameRing,
    retire: RetireBin,
    resources: ResourceTable,
    samplers: Samplers,
    #[cfg(unix)]
    acquire: Option<AcquireFences>,
    device: Arc<RenderDevice>,
}

impl GpuRuntime {
    pub fn new(device: Arc<RenderDevice>) -> Result<Self> {
        let samplers = Samplers::new(device.clone())?;
        Ok(Self {
            #[cfg(unix)]
            readiness: None,
            frames: FrameRing::new(device.clone())?,
            retire: RetireBin::default(),
            resources: ResourceTable::new(device.clone(), &samplers)?,
            samplers,
            #[cfg(unix)]
            acquire: AcquireFences::new(device.clone()),
            device,
        })
    }

    pub fn device(&self) -> &Arc<RenderDevice> {
        &self.device
    }

    #[cfg(unix)]
    pub fn readable_after(&mut self, value: u64) -> Result<OwnedFd> {
        let readiness = match &mut self.readiness {
            Some(readiness) => readiness,
            empty => empty.insert(Readiness::new(self.frames.timeline().clone())?),
        };
        readiness.ready_fd(value)
    }

    pub fn table_layout(&self) -> vk::DescriptorSetLayout {
        self.resources.layout()
    }

    pub fn bind_point(&mut self) -> BindPoint<'_> {
        BindPoint {
            device: &self.device,
            resources: &mut self.resources,
            samplers: &self.samplers,
        }
    }

    pub fn recycler(&mut self) -> Recycler<'_> {
        Recycler {
            value: self.frames.next_value(),
            resources: &mut self.resources,
            retire: &mut self.retire,
        }
    }

    pub fn idle_recycler(&mut self) -> Recycler<'_> {
        Recycler {
            value: self.frames.next_value() - 1,
            resources: &mut self.resources,
            retire: &mut self.retire,
        }
    }

    pub fn trim(&mut self) {
        self.frames.trim();
        let completed = self.frames.completed_value();
        self.retire.collect(completed);
        self.resources.collect(completed, &self.samplers);
    }

    pub fn defer(&mut self, resource: Box<dyn std::any::Any + Send>) {
        let value = self.frames.next_value();
        self.retire.defer(value, resource);
    }

    pub fn defer_until(&mut self, value: u64, resource: Box<dyn std::any::Any + Send>) {
        self.retire.defer(value, resource);
    }

    pub fn record_once(&mut self, record: impl FnOnce(vk::CommandBuffer)) -> Result<u64> {
        let cmd = self.frames.begin()?;
        record(cmd);
        self.frames.submit(&[], &[])
    }

    pub fn begin_frame(&mut self) -> Result<GpuFrameCtx<'_>> {
        let cmd = self.frames.begin()?;
        let frame_slot = self.frames.current_slot();
        let frame_value = self.frames.next_value();
        let completed_value = self.frames.completed_value();
        let (frame, staging) = self.frames.recording_parts();
        for unneeded in staging.take_unneeded() {
            self.retire.defer(completed_value, Box::new(unneeded));
        }
        self.retire.collect_within_frame(completed_value);
        self.resources.collect(completed_value, &self.samplers);
        self.resources.prepare(cmd, frame_slot);
        #[cfg(unix)]
        if let Some(acquire) = &mut self.acquire {
            acquire.collect(completed_value);
        }

        let canvas_cmd = frame.canvas_commands();
        Ok(GpuFrameCtx {
            device: &self.device,
            cmd,
            staging,
            resources: &mut self.resources,
            samplers: &self.samplers,
            retire: &mut self.retire,
            frame_slot,
            frame_value,
            completed_value,
            timer: &mut frame.timer,
            setup_waits: &mut frame.setup_waits,
            setup_signals: &mut frame.setup_signals,
            sample_waits: &mut frame.sample_waits,
            entered_canvas: &mut frame.entered_canvas,
            canvas_cmd,
        })
    }

    pub fn abandon_frame(&mut self) {
        self.frames.abandon();
    }

    pub fn submit(
        &mut self,
        canvas_waits: &[vk::SemaphoreSubmitInfo<'_>],
        signals: &[vk::SemaphoreSubmitInfo<'_>],
    ) -> Result<u64> {
        let value = self.frames.submit(canvas_waits, signals)?;
        self.retire.release_due();
        #[cfg(unix)]
        if let Some(acquire) = &mut self.acquire {
            acquire.submitted(value);
        }
        Ok(value)
    }

    #[cfg(unix)]
    pub fn wait_on_sync_fd(&mut self, fence: OwnedFd) -> Result<Option<vk::Semaphore>> {
        match &mut self.acquire {
            Some(acquire) => acquire.import_fence(fence).map(Some),
            None => {
                wait_signalled(&fence)?;
                Ok(None)
            }
        }
    }

    pub fn take_stall(&mut self) -> std::time::Duration {
        self.frames.take_stall()
    }

    pub fn completed_value(&self) -> u64 {
        self.frames.completed_value()
    }

    pub fn next_free_frame(&self) -> Option<u64> {
        self.frames.slot_due()
    }

    pub fn has_free_frame_slot(&self) -> bool {
        self.frames.has_free_slot()
    }

    pub fn next_value(&self) -> u64 {
        self.frames.next_value()
    }

    pub fn timeline(&self) -> &Arc<FrameTimeline> {
        self.frames.timeline()
    }

    pub fn texture_slots(&self) -> TextureSlots {
        self.resources.texture_slots()
    }

    pub fn wait_value(&self, value: u64) -> Result<()> {
        self.frames.wait_value(value)
    }

    pub fn wait_all(&self) -> Result<()> {
        self.frames.wait_all()
    }
}

impl std::fmt::Debug for GpuRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuRuntime").finish_non_exhaustive()
    }
}

impl<'a> std::fmt::Debug for BindPoint<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BindPoint").finish_non_exhaustive()
    }
}

impl<'a> std::fmt::Debug for Recycler<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recycler").finish_non_exhaustive()
    }
}
