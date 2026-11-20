use crate::common::BLOCKS_PER_SECOND;
use crate::config::Config;
use crate::delay::DelayEstimate;

#[derive(Clone, Debug)]
pub(crate) struct FilterDelay {
    delay_headroom_blocks: i32,
    external_delay_reported: bool,
    filter_delays_blocks: Vec<i32>,
    min_filter_delay: i32,
    external_delay: Option<DelayEstimate>,
}

impl FilterDelay {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> FilterDelay {
        let delay_headroom_blocks = config.delay_headroom_blocks() as i32;
        FilterDelay {
            delay_headroom_blocks,
            external_delay_reported: false,
            filter_delays_blocks: vec![delay_headroom_blocks; capture_channels],
            min_filter_delay: delay_headroom_blocks,
            external_delay: None,
        }
    }

    pub(crate) fn external_delay_reported(&self) -> bool {
        self.external_delay_reported
    }

    pub(crate) fn direct_path_filter_delays(&self) -> &[i32] {
        &self.filter_delays_blocks
    }

    pub(crate) fn min_direct_path_filter_delay(&self) -> i32 {
        self.min_filter_delay
    }

    pub(crate) fn update(
        &mut self,
        analyzer_filter_delay_estimates_blocks: &[i32],
        external_delay: Option<DelayEstimate>,
        blocks_with_proper_filter_adaptation: usize,
    ) {
        if let Some(external) = external_delay
            && self
                .external_delay
                .is_none_or(|known| known.delay != external.delay)
        {
            self.external_delay = Some(external);
            self.external_delay_reported = true;
        }
        let delay_estimator_may_not_have_converged =
            blocks_with_proper_filter_adaptation < 2 * BLOCKS_PER_SECOND;
        if delay_estimator_may_not_have_converged && self.external_delay.is_some() {
            self.filter_delays_blocks.fill(self.delay_headroom_blocks);
        } else {
            self.filter_delays_blocks
                .copy_from_slice(analyzer_filter_delay_estimates_blocks);
        }
        self.min_filter_delay = self.filter_delays_blocks.iter().copied().min().unwrap_or(0);
    }
}
