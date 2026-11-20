use glam::Vec2;

const MOVING_PHASES_PER_AREA: f32 = 8.0;
const MOVING_PHASES_MAX: u32 = 64;

pub(super) fn halton(index: u32, base: u32) -> f32 {
    let mut fraction = 1.0;
    let mut result = 0.0;
    let mut rest = index;
    while rest > 0 {
        fraction /= base as f32;
        result += fraction * (rest % base) as f32;
        rest /= base;
    }
    result
}

pub(super) fn offset(index: u32) -> Vec2 {
    Vec2::new(halton(index, 2), halton(index, 3)) - 0.5
}

pub(super) fn moving_phases(scale: f32) -> u32 {
    let area = 1.0 / (scale * scale);
    ((MOVING_PHASES_PER_AREA * area).ceil() as u32).clamp(8, MOVING_PHASES_MAX)
}
