use std::sync::Arc;

use ash::vk;
use rikka_sync::Mutex;

use crate::core::device::{QueueRole, RenderDevice};
use crate::core::frame::FrameTimeline;
use crate::core::image::record_barriers;
use crate::core::recorder::Recorder;
use crate::error::Result;
use crate::video::imports::VideoSemaphores;
use crate::video::lease::LentFrame;

pub(crate) struct VideoReleaser {
    device: Arc<RenderDevice>,
    releasing: Mutex<Option<Releasing>>,
}

struct Releasing {
    timeline: Arc<FrameTimeline>,
    recorder: Recorder,
    next: u64,
    handovers: Vec<vk::ImageMemoryBarrier2<'static>>,
    semaphores: VideoSemaphores,
}

impl VideoReleaser {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Self {
        Self {
            device,
            releasing: Mutex::new(None),
        }
    }

    pub(crate) fn release(&self, lent: LentFrame) {
        if let Err(error) = self.submit(&lent) {
            tracing::error!("cannot hand an unshown video frame back on the GPU: {error}");
            self.release_from_host(&lent);
        }
    }

    fn submit(&self, lent: &LentFrame) -> Result<()> {
        let mut guard = self.releasing.lock();
        if guard.is_none() {
            *guard = Some(Releasing::start(&self.device)?);
        }
        let releasing = guard.as_mut().expect("the releaser started above");
        let finished = releasing.timeline.finished()?;
        let cmd = releasing.recorder.command_buffer(&self.device, finished)?;
        let family = self.device.queue_family();
        releasing.handovers.clear();
        releasing.handovers.extend(
            lent.images
                .iter()
                .filter_map(|image| image.handover(family)),
        );
        let device = self.device.handle();
        let recorded = unsafe {
            device
                .begin_command_buffer(
                    cmd,
                    &vk::CommandBufferBeginInfo::default()
                        .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
                )
                .and_then(|()| {
                    record_barriers(&self.device, cmd, &releasing.handovers);
                    device.end_command_buffer(cmd)
                })
        };
        if let Err(error) = recorded {
            releasing.recorder.recycle(cmd);
            return Err(error.into());
        }
        let value = releasing.next;
        releasing.semaphores.clear();
        releasing.semaphores.settle(lent);
        releasing.semaphores.signals.push(
            vk::SemaphoreSubmitInfo::default()
                .semaphore(releasing.timeline.handle())
                .value(value)
                .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS),
        );
        let buffers = [vk::CommandBufferSubmitInfo::default().command_buffer(cmd)];
        let batch = vk::SubmitInfo2::default()
            .wait_semaphore_infos(&releasing.semaphores.waits)
            .command_buffer_infos(&buffers)
            .signal_semaphore_infos(&releasing.semaphores.signals);
        if let Err(error) = self
            .device
            .submit(QueueRole::Graphics, &[batch], vk::Fence::null())
        {
            releasing.recorder.recycle(cmd);
            return Err(error);
        }
        releasing.recorder.submitted(value, cmd);
        releasing.next += 1;
        Ok(())
    }

    fn release_from_host(&self, lent: &LentFrame) {
        let device = self.device.handle();
        for &(semaphore, value) in &lent.waits {
            let semaphores = [semaphore];
            let values = [value];
            let wait_info = vk::SemaphoreWaitInfo::default()
                .semaphores(&semaphores)
                .values(&values);
            if let Err(err) = unsafe { device.wait_semaphores(&wait_info, u64::MAX) } {
                tracing::error!("failed to wait for an unshown video frame: {err}");
            }
        }
        for &(semaphore, value) in &lent.signals {
            let signal_info = vk::SemaphoreSignalInfo::default()
                .semaphore(semaphore)
                .value(value);
            if let Err(err) = unsafe { device.signal_semaphore(&signal_info) } {
                tracing::error!("failed to release an unshown video frame: {err}");
            }
        }
    }
}

impl Releasing {
    fn start(device: &Arc<RenderDevice>) -> Result<Self> {
        Ok(Self {
            timeline: FrameTimeline::named(device.clone(), "rikka.video.releases")?,
            recorder: Recorder::new(device, device.queue_family())?,
            next: 1,
            handovers: Vec::new(),
            semaphores: VideoSemaphores::default(),
        })
    }
}

impl Drop for VideoReleaser {
    fn drop(&mut self) {
        let Some(releasing) = self.releasing.get_mut().take() else {
            return;
        };
        if let Err(error) = releasing.timeline.wait(releasing.next - 1) {
            tracing::warn!("failed to wait for video frame releases: {error}");
            self.device.wait_idle();
        }
        releasing.recorder.destroy(&self.device);
    }
}

impl std::fmt::Debug for VideoReleaser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoReleaser").finish_non_exhaustive()
    }
}
