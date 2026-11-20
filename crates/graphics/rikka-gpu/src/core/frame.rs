use std::sync::Arc;
use std::time::{Duration, Instant};

use ash::vk;

use crate::core::device::{QueueRole, RenderDevice};
use crate::core::frame_ctx::PassTimer;
use crate::core::image::{Recording, Tickets};
use crate::core::staging::StagingBelt;
use crate::error::Result;

pub const MAX_FRAMES_IN_FLIGHT: usize = 3;

pub(crate) struct Frame {
    pub timer: PassTimer,
    pub setup_waits: Vec<vk::SemaphoreSubmitInfo<'static>>,
    pub setup_signals: Vec<vk::SemaphoreSubmitInfo<'static>>,
    pub sample_waits: Vec<vk::SemaphoreSubmitInfo<'static>>,
    pub entered_canvas: bool,
    setup: vk::CommandBuffer,
    canvas: vk::CommandBuffer,
    command_pool: vk::CommandPool,
    signal_value: u64,
}

impl Frame {
    pub(crate) fn canvas_commands(&self) -> vk::CommandBuffer {
        self.canvas
    }
}

pub struct FrameTimeline {
    device: Arc<RenderDevice>,
    handle: vk::Semaphore,
}

impl FrameTimeline {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Result<Arc<Self>> {
        Self::named(device, "rikka.frame_timeline")
    }

    pub(crate) fn named(device: Arc<RenderDevice>, name: &str) -> Result<Arc<Self>> {
        let mut type_info = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(0);
        let info = vk::SemaphoreCreateInfo::default().push_next(&mut type_info);
        let handle = unsafe { device.handle().create_semaphore(&info, None)? };
        device.set_object_name(handle, name);
        Ok(Arc::new(Self { device, handle }))
    }

    pub(crate) fn handle(&self) -> vk::Semaphore {
        self.handle
    }

    pub fn finished(&self) -> Result<u64> {
        Ok(unsafe {
            self.device
                .handle()
                .get_semaphore_counter_value(self.handle)
        }
        .inspect_err(|result| self.device.observe_loss(*result))?)
    }

    pub fn reached(&self, value: u64) -> bool {
        self.finished().is_ok_and(|finished| finished >= value)
    }

    pub fn completed(&self) -> u64 {
        self.finished().unwrap_or(0)
    }

    pub fn wait(&self, value: u64) -> Result<()> {
        let semaphores = [self.handle];
        let values = [value];
        let wait_info = vk::SemaphoreWaitInfo::default()
            .semaphores(&semaphores)
            .values(&values);
        unsafe { self.device.handle().wait_semaphores(&wait_info, u64::MAX) }
            .inspect_err(|result| self.device.observe_loss(*result))?;
        Ok(())
    }

    pub(crate) fn signal(&self, value: u64) -> Result<()> {
        let info = vk::SemaphoreSignalInfo::default()
            .semaphore(self.handle)
            .value(value);
        unsafe { self.device.handle().signal_semaphore(&info)? };
        Ok(())
    }
}

impl std::fmt::Debug for FrameTimeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameTimeline")
            .field("finished", &self.finished())
            .finish()
    }
}

impl Drop for FrameTimeline {
    fn drop(&mut self) {
        unsafe { self.device.handle().destroy_semaphore(self.handle, None) };
    }
}

pub(crate) struct FrameRing {
    device: Arc<RenderDevice>,
    timeline: Arc<FrameTimeline>,
    next_value: u64,
    current: usize,
    frames: Vec<Frame>,
    staging: StagingBelt,
    recording: Option<Recording>,
    tickets: Tickets,
    stalled: Duration,
    signals: Vec<vk::SemaphoreSubmitInfo<'static>>,
    canvas_waits: Vec<vk::SemaphoreSubmitInfo<'static>>,
}

fn lasting(info: &vk::SemaphoreSubmitInfo<'_>) -> vk::SemaphoreSubmitInfo<'static> {
    vk::SemaphoreSubmitInfo::default()
        .semaphore(info.semaphore)
        .value(info.value)
        .stage_mask(info.stage_mask)
        .device_index(info.device_index)
}

impl FrameRing {
    pub(crate) fn new(device: Arc<RenderDevice>) -> Result<Self> {
        let timeline = FrameTimeline::new(device.clone())?;
        let mut ring = Self {
            staging: StagingBelt::new(device.clone()),
            device,
            timeline,
            next_value: 1,
            current: 0,
            frames: Vec::with_capacity(MAX_FRAMES_IN_FLIGHT),
            recording: None,
            tickets: Tickets::default(),
            stalled: Duration::ZERO,
            signals: Vec::new(),
            canvas_waits: Vec::new(),
        };
        for slot in 0..MAX_FRAMES_IN_FLIGHT {
            let frame = Self::create_frame(&ring.device, slot)?;
            ring.frames.push(frame);
        }
        Ok(ring)
    }

    fn create_frame(device: &Arc<RenderDevice>, slot: usize) -> Result<Frame> {
        let pool_info = vk::CommandPoolCreateInfo::default()
            .flags(vk::CommandPoolCreateFlags::TRANSIENT)
            .queue_family_index(device.queue_family());
        let command_pool = unsafe { device.handle().create_command_pool(&pool_info, None)? };
        device.set_object_name(command_pool, &format!("rikka.frame{slot}"));

        let alloc_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(2);
        let parts = unsafe { device.handle().allocate_command_buffers(&alloc_info) }
            .map_err(Into::into)
            .and_then(|buffers| Ok((buffers, PassTimer::new(device.clone(), slot)?)));
        let (buffers, timer) = match parts {
            Ok(parts) => parts,
            Err(err) => {
                unsafe { device.handle().destroy_command_pool(command_pool, None) };
                return Err(err);
            }
        };
        device.set_object_name(buffers[0], &format!("rikka.frame{slot}.setup"));
        device.set_object_name(buffers[1], &format!("rikka.frame{slot}.canvas"));

        Ok(Frame {
            timer,
            setup_waits: Vec::new(),
            setup_signals: Vec::new(),
            sample_waits: Vec::new(),
            entered_canvas: false,
            setup: buffers[0],
            canvas: buffers[1],
            command_pool,
            signal_value: 0,
        })
    }
    pub(crate) fn begin(&mut self) -> Result<vk::CommandBuffer> {
        self.abandon();
        self.current = self.lease()?;
        self.staging.open(self.next_value, self.completed_value());

        let frame = &mut self.frames[self.current];
        unsafe {
            self.device
                .handle()
                .reset_command_pool(frame.command_pool, vk::CommandPoolResetFlags::empty())?;
            let begin_info = vk::CommandBufferBeginInfo::default()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
            for cmd in [frame.setup, frame.canvas] {
                self.device
                    .handle()
                    .begin_command_buffer(cmd, &begin_info)?;
            }
        }
        frame.setup_waits.clear();
        frame.setup_signals.clear();
        frame.sample_waits.clear();
        frame.entered_canvas = false;
        frame.timer.start(frame.setup);
        self.recording = Some(Recording::open(&mut self.tickets));
        Ok(frame.setup)
    }

    pub(crate) fn trim(&mut self) {
        if self.recording.is_some() {
            return;
        }
        let completed = self.completed_value();
        self.staging.empty(completed);
        for frame in &mut self.frames {
            if frame.signal_value > completed {
                continue;
            }
            let released = unsafe {
                self.device.handle().reset_command_pool(
                    frame.command_pool,
                    vk::CommandPoolResetFlags::RELEASE_RESOURCES,
                )
            };
            if let Err(error) = released {
                tracing::warn!("failed to reset idle command pool: {error}");
            }
        }
    }

    pub(crate) fn slot_due(&self) -> Option<u64> {
        if self.has_free_slot() {
            return None;
        }
        Some(self.oldest().1)
    }

    pub(crate) fn has_free_slot(&self) -> bool {
        self.oldest().1 <= self.completed_value()
    }

    fn oldest(&self) -> (usize, u64) {
        self.frames
            .iter()
            .enumerate()
            .map(|(index, frame)| (index, frame.signal_value))
            .min_by_key(|&(_, value)| value)
            .expect("the ring holds every frame slot")
    }

    fn lease(&mut self) -> Result<usize> {
        let (oldest, signal_value) = self.oldest();
        if signal_value <= self.completed_value() {
            return Ok(oldest);
        }
        let started = Instant::now();
        self.wait_value(signal_value)?;
        self.stalled += started.elapsed();
        Ok(oldest)
    }

    pub(crate) fn take_stall(&mut self) -> Duration {
        std::mem::take(&mut self.stalled)
    }

    pub(crate) fn recording_parts(&mut self) -> (&mut Frame, &mut StagingBelt) {
        (&mut self.frames[self.current], &mut self.staging)
    }

    pub(crate) fn current_slot(&self) -> usize {
        self.current
    }

    pub(crate) fn abandon(&mut self) {
        if self.recording.take().is_some() {
            self.frames[self.current].timer.abandon();
        }
    }

    pub(crate) fn submit(
        &mut self,
        canvas_waits: &[vk::SemaphoreSubmitInfo<'_>],
        extra_signals: &[vk::SemaphoreSubmitInfo<'_>],
    ) -> Result<u64> {
        let submitted = self.submit_recorded(canvas_waits, extra_signals);
        match submitted {
            Ok(value) => {
                if let Some(recording) = self.recording.take() {
                    recording.commit();
                }
                Ok(value)
            }
            Err(err) => {
                self.abandon();
                Err(err)
            }
        }
    }

    fn submit_recorded(
        &mut self,
        canvas_waits: &[vk::SemaphoreSubmitInfo<'_>],
        extra_signals: &[vk::SemaphoreSubmitInfo<'_>],
    ) -> Result<u64> {
        let value = self.next_value;
        let frame = &mut self.frames[self.current];

        frame.timer.finish(match frame.entered_canvas {
            true => frame.canvas,
            false => frame.setup,
        });
        unsafe {
            self.device.handle().end_command_buffer(frame.setup)?;
            self.device.handle().end_command_buffer(frame.canvas)?;
        };

        let signals = &mut self.signals;
        signals.clear();
        signals.push(
            vk::SemaphoreSubmitInfo::default()
                .semaphore(self.timeline.handle())
                .value(value)
                .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS),
        );
        signals.extend(extra_signals.iter().map(lasting));

        let setup = [vk::CommandBufferSubmitInfo::default().command_buffer(frame.setup)];
        let canvas = [vk::CommandBufferSubmitInfo::default().command_buffer(frame.canvas)];
        frame.setup_waits.extend_from_slice(&frame.sample_waits);
        let canvas_waits_with_samples = &mut self.canvas_waits;
        canvas_waits_with_samples.clear();
        canvas_waits_with_samples.extend(canvas_waits.iter().map(lasting));
        if !frame.entered_canvas {
            frame
                .setup_waits
                .extend_from_slice(canvas_waits_with_samples);
        }
        canvas_waits_with_samples.extend_from_slice(&frame.sample_waits);
        let first = vk::SubmitInfo2::default()
            .wait_semaphore_infos(&frame.setup_waits)
            .command_buffer_infos(&setup);
        let (one, both);
        let batches: &[vk::SubmitInfo2<'_>] = match frame.entered_canvas {
            true => {
                both = [
                    first.signal_semaphore_infos(&frame.setup_signals),
                    vk::SubmitInfo2::default()
                        .wait_semaphore_infos(canvas_waits_with_samples)
                        .command_buffer_infos(&canvas)
                        .signal_semaphore_infos(signals),
                ];
                &both
            }
            false => {
                signals.extend_from_slice(&frame.setup_signals);
                one = [first.signal_semaphore_infos(signals)];
                &one
            }
        };

        self.device
            .submit(QueueRole::Graphics, batches, vk::Fence::null())?;

        frame.signal_value = value;
        frame.timer.submitted(value);
        self.next_value += 1;
        Ok(value)
    }

    pub(crate) fn completed_value(&self) -> u64 {
        self.timeline.completed()
    }

    pub(crate) fn next_value(&self) -> u64 {
        self.next_value
    }

    pub(crate) fn timeline(&self) -> &Arc<FrameTimeline> {
        &self.timeline
    }

    pub(crate) fn wait_value(&self, value: u64) -> Result<()> {
        self.timeline.wait(value)
    }

    pub(crate) fn wait_all(&self) -> Result<()> {
        if self.next_value > 1 {
            self.wait_value(self.next_value - 1)?;
        }
        Ok(())
    }
}

impl Drop for FrameRing {
    fn drop(&mut self) {
        self.abandon();
        if let Err(err) = self.wait_all() {
            tracing::error!("failed to wait for in-flight frames: {err}");
            self.device.wait_idle();
        }
        unsafe {
            for frame in &self.frames {
                self.device
                    .handle()
                    .destroy_command_pool(frame.command_pool, None);
            }
        }
    }
}
