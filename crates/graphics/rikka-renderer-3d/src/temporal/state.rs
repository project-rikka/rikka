use std::sync::Arc;
use std::time::Duration;

use ash::vk;
use glam::Vec2;

use crate::error::Result;
use crate::temporal::jitter::{moving_phases, offset};
use crate::temporal::scale::ScaleController;
use crate::temporal::timer::ViewTimer;
use crate::temporal::{Antialiasing, RenderScale, Upscale, clamp_scale};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const MOVING_SAMPLES: f32 = 16.0;
const STILL_SAMPLES: f32 = 64.0;
const STILL_FRAMES_MAX: f32 = 256.0;
const MIP_BIAS: f32 = -1.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TemporalFrame {
    pub upscaled: bool,
    pub render: vk::Extent2D,
    pub previous: vk::Extent2D,
    pub display: vk::Extent2D,
    pub jitter: Vec2,
    pub jitter_ndc: Vec2,
    pub previous_jitter_ndc: Vec2,
    pub mip_bias: f32,
    pub limit: f32,
    pub reset: bool,
    pub still: bool,
    pub settled: bool,
    pub sharpness: f32,
}

impl TemporalFrame {
    pub(crate) fn ratio(&self) -> Vec2 {
        Vec2::new(
            self.render.width as f32 / self.display.width as f32,
            self.render.height as f32 / self.display.height as f32,
        )
    }
}

pub(crate) struct TemporalState {
    mode: Antialiasing,
    display: vk::Extent2D,
    controller: ScaleController,
    timer: Option<ViewTimer>,
    measured: Option<Duration>,
    sequence: u32,
    still: u32,
    signature: Option<u64>,
    settled: bool,
    jitter_ndc: Option<Vec2>,
    rendered: Option<vk::Extent2D>,
}

impl TemporalState {
    pub(crate) fn new(
        device: &Arc<RenderDevice>,
        mode: Antialiasing,
        display: vk::Extent2D,
    ) -> Result<Self> {
        let rule = match mode {
            Antialiasing::Upscale(Upscale { scale, .. }) => scale,
            Antialiasing::Multisample | Antialiasing::Temporal => RenderScale::Fixed(1.0),
        };
        let timer = match rule {
            RenderScale::Dynamic(_) => ViewTimer::new(device)?,
            RenderScale::Fixed(_) => None,
        };
        Ok(Self {
            mode,
            display,
            controller: ScaleController::new(rule),
            timer,
            measured: None,
            sequence: 0,
            still: 0,
            signature: None,
            settled: false,
            jitter_ndc: None,
            rendered: None,
        })
    }

    pub(crate) fn enabled(&self) -> bool {
        self.mode.is_temporal()
    }

    pub(crate) fn scale(&self) -> f32 {
        self.controller.scale()
    }

    pub(crate) fn is_settled(&self) -> bool {
        self.settled
    }

    pub(crate) fn measured(&self) -> Option<Duration> {
        self.measured
    }

    pub(crate) fn render_extent(&self) -> vk::Extent2D {
        self.extent_at(self.scale())
    }

    pub(crate) fn capacity_extent(&self) -> vk::Extent2D {
        self.extent_at(self.controller.highest())
    }

    fn extent_at(&self, scale: f32) -> vk::Extent2D {
        let side = |length: u32| ((length as f32 * scale).round() as u32).clamp(1, length);
        vk::Extent2D {
            width: side(self.display.width),
            height: side(self.display.height),
        }
    }

    pub(crate) fn observe(&mut self, completed: u64) {
        let Some(timer) = self.timer.as_mut() else {
            return;
        };
        let controller = &mut self.controller;
        let measured = &mut self.measured;
        timer.collect(completed, |sample| {
            *measured = Some(sample.time);
            controller.observe(sample.time, sample.scale);
        });
    }

    pub(crate) fn begin_timing(&mut self, ctx: &GpuFrameCtx<'_>) {
        if let Some(timer) = self.timer.as_mut() {
            timer.begin(ctx);
        }
    }

    pub(crate) fn end_timing(&mut self, ctx: &GpuFrameCtx<'_>, scale: f32) {
        if let Some(timer) = self.timer.as_mut() {
            timer.end(ctx, scale);
        }
    }

    pub(crate) fn advance(
        &mut self,
        render: vk::Extent2D,
        signature: u64,
        cut: bool,
    ) -> Option<TemporalFrame> {
        if !self.enabled() {
            return None;
        }
        let scale = clamp_scale(render.width as f32 / self.display.width as f32);
        let repeated = self.signature == Some(signature);
        let cut = cut && !repeated;
        match repeated {
            true => self.still = self.still.saturating_add(1),
            false => self.still = 0,
        }
        self.signature = Some(signature);
        let still_frames = (STILL_SAMPLES / (scale * scale)).min(STILL_FRAMES_MAX) as u32;
        let settled = self.still >= still_frames;
        self.settled = settled;
        let (index, limit) = match self.still {
            0 => {
                self.sequence = self.sequence.wrapping_add(1);
                (1 + self.sequence % moving_phases(scale), MOVING_SAMPLES)
            }
            still => (still + 1, STILL_SAMPLES),
        };
        let jitter = match settled {
            true => Vec2::ZERO,
            false => offset(index),
        };
        let size = Vec2::new(render.width as f32, render.height as f32);
        let jitter_ndc = 2.0 * jitter / size;
        let previous_jitter_ndc = match (cut, self.jitter_ndc) {
            (false, Some(previous)) => previous,
            _ => jitter_ndc,
        };
        self.jitter_ndc = Some(jitter_ndc);
        let previous = match (cut, self.rendered) {
            (false, Some(previous)) => previous,
            _ => render,
        };
        self.rendered = Some(render);
        Some(TemporalFrame {
            upscaled: matches!(self.mode, Antialiasing::Upscale(_)),
            render,
            previous,
            display: self.display,
            jitter,
            jitter_ndc,
            previous_jitter_ndc,
            mip_bias: scale.log2() + MIP_BIAS,
            limit,
            reset: cut,
            still: self.still > 0,
            settled,
            sharpness: match self.mode {
                Antialiasing::Upscale(upscale) => upscale.sharpness.clamp(0.0, 1.0),
                Antialiasing::Multisample | Antialiasing::Temporal => 0.0,
            },
        })
    }
}
