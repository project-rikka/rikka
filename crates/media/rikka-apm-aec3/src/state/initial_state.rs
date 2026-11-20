use crate::common::BLOCKS_PER_SECOND;
use crate::config::Config;

#[derive(Clone, Debug)]
pub(crate) struct InitialState {
    conservative_initial_phase: bool,
    initial_state_seconds: f32,
    transition_triggered: bool,
    initial_state: bool,
    strong_not_saturated_render_blocks: usize,
}

impl InitialState {
    pub(crate) fn new(config: &Config) -> InitialState {
        InitialState {
            conservative_initial_phase: config.filter.conservative_initial_phase,
            initial_state_seconds: config.filter.initial_state_seconds,
            transition_triggered: false,
            initial_state: true,
            strong_not_saturated_render_blocks: 0,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.initial_state = true;
        self.strong_not_saturated_render_blocks = 0;
    }

    pub(crate) fn transition_triggered(&self) -> bool {
        self.transition_triggered
    }

    pub(crate) fn update(&mut self, active_render: bool, saturated_capture: bool) {
        self.strong_not_saturated_render_blocks += usize::from(active_render && !saturated_capture);
        let previous = self.initial_state;
        self.initial_state = if self.conservative_initial_phase {
            self.strong_not_saturated_render_blocks < 5 * BLOCKS_PER_SECOND
        } else {
            (self.strong_not_saturated_render_blocks as f32)
                < self.initial_state_seconds * BLOCKS_PER_SECOND as f32
        };
        self.transition_triggered = !self.initial_state && previous;
    }
}
