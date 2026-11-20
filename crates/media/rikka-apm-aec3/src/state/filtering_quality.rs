use crate::common::BLOCKS_PER_SECOND;

#[derive(Clone, Copy, Debug)]
pub(crate) struct QualityInput {
    pub(crate) active_render: bool,
    pub(crate) transparent_mode: bool,
    pub(crate) saturated_capture: bool,
    pub(crate) external_delay: bool,
    pub(crate) any_filter_converged: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct FilteringQualityAnalyzer {
    use_linear_filter: bool,
    overall_usable_linear_estimates: bool,
    filter_update_blocks_since_reset: usize,
    filter_update_blocks_since_start: usize,
    convergence_seen: bool,
    usable_linear_filter_estimates: Vec<bool>,
}

impl FilteringQualityAnalyzer {
    pub(crate) fn new(
        use_linear_filter: bool,
        capture_channels: usize,
    ) -> FilteringQualityAnalyzer {
        FilteringQualityAnalyzer {
            use_linear_filter,
            overall_usable_linear_estimates: false,
            filter_update_blocks_since_reset: 0,
            filter_update_blocks_since_start: 0,
            convergence_seen: false,
            usable_linear_filter_estimates: vec![false; capture_channels],
        }
    }

    pub(crate) fn linear_filter_usable(&self) -> bool {
        self.overall_usable_linear_estimates
    }

    pub(crate) fn usable_linear_filter_outputs(&self) -> &[bool] {
        &self.usable_linear_filter_estimates
    }

    pub(crate) fn reset(&mut self) {
        self.usable_linear_filter_estimates.fill(false);
        self.overall_usable_linear_estimates = false;
        self.filter_update_blocks_since_reset = 0;
    }

    pub(crate) fn update(&mut self, input: &QualityInput) {
        let filter_update = usize::from(input.active_render && !input.saturated_capture);
        self.filter_update_blocks_since_reset += filter_update;
        self.filter_update_blocks_since_start += filter_update;
        self.convergence_seen = self.convergence_seen || input.any_filter_converged;
        let sufficient_data_to_converge_at_startup =
            self.filter_update_blocks_since_start as f32 > BLOCKS_PER_SECOND as f32 * 0.4;
        let sufficient_data_to_converge_at_reset = sufficient_data_to_converge_at_startup
            && self.filter_update_blocks_since_reset as f32 > BLOCKS_PER_SECOND as f32 * 0.2;
        self.overall_usable_linear_estimates = sufficient_data_to_converge_at_startup
            && sufficient_data_to_converge_at_reset
            && (input.external_delay || self.convergence_seen)
            && !input.transparent_mode;
        if self.use_linear_filter {
            self.usable_linear_filter_estimates
                .fill(self.overall_usable_linear_estimates);
        }
    }
}
