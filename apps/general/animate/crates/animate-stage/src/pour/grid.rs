use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_physics::StepRate;

const STEPS_PER_SECOND: u32 = 60;
const STEPS_PER_FRAME: u32 = (STEPS_PER_SECOND as f32 / FRAMES_PER_SECOND) as u32;

pub(crate) const GRID: StepRate = StepRate::new(STEPS_PER_SECOND);

pub(crate) fn step_of(time: f32) -> u32 {
    GRID.step_of(time)
}

pub(crate) fn step_of_frame(frame: u32) -> u32 {
    frame.saturating_mul(STEPS_PER_FRAME)
}

pub fn frame_at_step(step: u32) -> f32 {
    step as f32 / STEPS_PER_FRAME as f32
}
