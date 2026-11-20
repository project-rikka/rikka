use std::time::Duration;

use crate::temporal::{DynamicScale, RenderScale, clamp_scale};

const SMOOTHING: f64 = 0.15;
const DAMPING: f32 = 0.5;
const STEP: f32 = 1.0 / 32.0;
const SETTLE_FRAMES: u32 = 12;

#[derive(Clone, Copy, Debug)]
pub(super) struct ScaleController {
    scale: f32,
    rule: RenderScale,
    smoothed: Option<f64>,
    since_change: u32,
}

impl ScaleController {
    pub(super) fn new(rule: RenderScale) -> Self {
        let scale = match rule {
            RenderScale::Fixed(scale) => clamp_scale(scale),
            RenderScale::Dynamic(dynamic) => bounds(dynamic).1,
        };
        Self {
            scale,
            rule,
            smoothed: None,
            since_change: 0,
        }
    }

    pub(super) fn scale(&self) -> f32 {
        self.scale
    }

    pub(super) fn highest(&self) -> f32 {
        match self.rule {
            RenderScale::Fixed(_) => self.scale,
            RenderScale::Dynamic(dynamic) => bounds(dynamic).1,
        }
    }

    pub(super) fn observe(&mut self, measured: Duration, at_scale: f32) {
        let RenderScale::Dynamic(dynamic) = self.rule else {
            return;
        };
        if at_scale != self.scale {
            return;
        }
        let nanos = measured.as_nanos() as f64;
        let smoothed = match self.smoothed {
            Some(previous) => previous + SMOOTHING * (nanos - previous),
            None => nanos,
        };
        self.smoothed = Some(smoothed);
        self.since_change = self.since_change.saturating_add(1);
        if self.since_change < SETTLE_FRAMES || smoothed <= 0.0 {
            return;
        }
        let target = dynamic.target.as_nanos() as f64;
        let ideal = self.scale * (target / smoothed).sqrt() as f32;
        let wanted = self.scale + DAMPING * (ideal - self.scale);
        let (low, high) = bounds(dynamic);
        let stepped = ((wanted / STEP).round() * STEP).clamp(low, high);
        if (stepped - self.scale).abs() >= STEP * 0.5 {
            self.scale = stepped;
            self.smoothed = None;
            self.since_change = 0;
        }
    }
}

fn bounds(dynamic: DynamicScale) -> (f32, f32) {
    let low = clamp_scale(dynamic.min.min(dynamic.max));
    let high = clamp_scale(dynamic.max.max(dynamic.min));
    (low, high)
}
