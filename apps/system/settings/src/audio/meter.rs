use rikka_app::prelude::*;
use rikka_sound::Peaks;

const STEPS: f32 = 512.0;

pub(super) fn quantize(level: f32) -> u16 {
    (level.clamp(0.0, 1.0) * STEPS) as u16
}

pub(super) fn meter(
    meters: &Watchers,
    peaks: Option<&Peaks>,
    node: u32,
    color: Color,
) -> LiveProgress {
    let peaks = peaks.cloned();
    live_progress(meters, move || {
        peaks
            .as_ref()
            .map_or(0.0, |peaks| f32::from(quantize(peaks.level(node))) / STEPS)
    })
    .color(color)
}
