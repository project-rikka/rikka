use crate::common::BLOCKS_PER_SECOND;
use crate::config::Config;

const BLOCKS_SINCE_CONVERGED_FILTER_INIT: usize = 10_000;
const BLOCKS_SINCE_CONSISTENT_ESTIMATE_INIT: usize = 10_000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TransparentModeInput {
    pub(crate) filter_delay_blocks: i32,
    pub(crate) any_filter_consistent: bool,
    pub(crate) any_filter_converged: bool,
    pub(crate) all_filters_diverged: bool,
    pub(crate) active_render: bool,
    pub(crate) saturated_capture: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct TransparentMode {
    linear_and_stable_echo_path: bool,
    capture_block_counter: usize,
    transparency_activated: bool,
    active_blocks_since_sane_filter: usize,
    sane_filter_observed: bool,
    finite_erl_recently_detected: bool,
    non_converged_sequence_size: usize,
    diverged_sequence_size: usize,
    active_non_converged_sequence_size: usize,
    num_converged_blocks: usize,
    recent_convergence_during_activity: bool,
    strong_not_saturated_render_blocks: usize,
}

impl TransparentMode {
    pub(crate) fn create(config: &Config) -> Option<TransparentMode> {
        (!config.ep_strength.bounded_erl).then_some(TransparentMode {
            linear_and_stable_echo_path: config.echo_removal_control.linear_and_stable_echo_path,
            capture_block_counter: 0,
            transparency_activated: false,
            active_blocks_since_sane_filter: BLOCKS_SINCE_CONSISTENT_ESTIMATE_INIT,
            sane_filter_observed: false,
            finite_erl_recently_detected: false,
            non_converged_sequence_size: BLOCKS_SINCE_CONVERGED_FILTER_INIT,
            diverged_sequence_size: 0,
            active_non_converged_sequence_size: 0,
            num_converged_blocks: 0,
            recent_convergence_during_activity: false,
            strong_not_saturated_render_blocks: 0,
        })
    }

    pub(crate) fn active(&self) -> bool {
        self.transparency_activated
    }

    pub(crate) fn reset(&mut self) {
        self.non_converged_sequence_size = BLOCKS_SINCE_CONVERGED_FILTER_INIT;
        self.diverged_sequence_size = 0;
        self.strong_not_saturated_render_blocks = 0;
        if self.linear_and_stable_echo_path {
            self.recent_convergence_during_activity = false;
        }
    }

    pub(crate) fn update(&mut self, input: &TransparentModeInput) {
        self.capture_block_counter += 1;
        self.strong_not_saturated_render_blocks +=
            usize::from(input.active_render && !input.saturated_capture);
        if input.any_filter_consistent && input.filter_delay_blocks < 5 {
            self.sane_filter_observed = true;
            self.active_blocks_since_sane_filter = 0;
        } else if input.active_render {
            self.active_blocks_since_sane_filter += 1;
        }
        let sane_filter_recently_seen = if self.sane_filter_observed {
            self.active_blocks_since_sane_filter <= 30 * BLOCKS_PER_SECOND
        } else {
            self.capture_block_counter <= 5 * BLOCKS_PER_SECOND
        };

        if input.any_filter_converged {
            self.recent_convergence_during_activity = true;
            self.active_non_converged_sequence_size = 0;
            self.non_converged_sequence_size = 0;
            self.num_converged_blocks += 1;
        } else {
            self.non_converged_sequence_size += 1;
            if self.non_converged_sequence_size > 20 * BLOCKS_PER_SECOND {
                self.num_converged_blocks = 0;
            }
            if input.active_render {
                self.active_non_converged_sequence_size += 1;
                if self.active_non_converged_sequence_size > 60 * BLOCKS_PER_SECOND {
                    self.recent_convergence_during_activity = false;
                }
            }
        }

        if !input.all_filters_diverged {
            self.diverged_sequence_size = 0;
        } else {
            self.diverged_sequence_size += 1;
            if self.diverged_sequence_size >= 60 {
                self.non_converged_sequence_size = BLOCKS_SINCE_CONVERGED_FILTER_INIT;
            }
        }

        if self.active_non_converged_sequence_size > 60 * BLOCKS_PER_SECOND {
            self.finite_erl_recently_detected = false;
        }
        if self.num_converged_blocks > 50 {
            self.finite_erl_recently_detected = true;
        }

        self.transparency_activated = if self.finite_erl_recently_detected
            || (sane_filter_recently_seen && self.recent_convergence_during_activity)
        {
            false
        } else {
            self.strong_not_saturated_render_blocks > 6 * BLOCKS_PER_SECOND
        };
    }
}
