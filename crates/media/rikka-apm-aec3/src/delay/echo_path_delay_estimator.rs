use crate::block::Block;
use crate::common::{
    BLOCK_LEN, BLOCKS_PER_SECOND, MATCHED_FILTER_ALIGNMENT_SHIFT_SUB_BLOCKS,
    MATCHED_FILTER_WINDOW_SUB_BLOCKS,
};
use crate::config::Config;
use crate::delay::clockdrift::{ClockdriftDetector, Level};
use crate::delay::estimate::{DelayEstimate, Quality};
use crate::delay::lag_aggregator::LagAggregator;
use crate::delay::matched_filter::{MatchedFilter, MatchedFilterSettings};
use crate::render::{AlignmentMixer, Decimator};
use crate::ring::Ring;

#[derive(Clone, Debug)]
pub(crate) struct EchoPathDelayEstimator {
    down_sampling_factor: usize,
    sub_block_size: usize,
    capture_mixer: AlignmentMixer,
    capture_decimator: Decimator,
    matched_filter: MatchedFilter,
    lag_aggregator: LagAggregator,
    old_aggregated_lag: Option<DelayEstimate>,
    consistent_estimate_counter: usize,
    clockdrift_detector: ClockdriftDetector,
}

impl EchoPathDelayEstimator {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> EchoPathDelayEstimator {
        let down_sampling_factor = config.delay.down_sampling_factor;
        let sub_block_size = BLOCK_LEN
            .checked_div(down_sampling_factor)
            .unwrap_or(BLOCK_LEN);
        let matched_filter = MatchedFilter::new(&MatchedFilterSettings {
            sub_block_size,
            window_size_sub_blocks: MATCHED_FILTER_WINDOW_SUB_BLOCKS,
            filters: config.delay.num_filters,
            alignment_shift_sub_blocks: MATCHED_FILTER_ALIGNMENT_SHIFT_SUB_BLOCKS,
            excitation_limit: if down_sampling_factor == 8 {
                config.render_levels.poor_excitation_render_limit_ds8
            } else {
                config.render_levels.poor_excitation_render_limit
            },
            smoothing_fast: config.delay.delay_estimate_smoothing,
            smoothing_slow: config.delay.delay_estimate_smoothing_delay_found,
            matching_filter_threshold: config.delay.delay_candidate_detection_threshold,
            detect_pre_echo: config.delay.detect_pre_echo,
        });
        let lag_aggregator = LagAggregator::new(matched_filter.max_filter_lag(), &config.delay);
        EchoPathDelayEstimator {
            down_sampling_factor,
            sub_block_size,
            capture_mixer: AlignmentMixer::new(
                capture_channels,
                &config.delay.capture_alignment_mixing,
            ),
            capture_decimator: Decimator::new(down_sampling_factor),
            matched_filter,
            lag_aggregator,
            old_aggregated_lag: None,
            consistent_estimate_counter: 0,
            clockdrift_detector: ClockdriftDetector::new(),
        }
    }

    pub(crate) fn reset(&mut self, reset_delay_confidence: bool) {
        self.reset_parts(true, reset_delay_confidence);
    }

    pub(crate) fn has_clockdrift(&self) -> bool {
        self.clockdrift_detector.level() != Level::None
    }

    pub(crate) fn estimate_delay(
        &mut self,
        render: &Ring<f32>,
        capture: &Block,
    ) -> Option<DelayEstimate> {
        let mut downsampled_capture = [0.0; BLOCK_LEN];
        let mut downmixed_capture = [0.0; BLOCK_LEN];
        self.capture_mixer
            .produce_output(capture, &mut downmixed_capture);
        self.capture_decimator.decimate(
            &downmixed_capture,
            &mut downsampled_capture[..self.sub_block_size],
        );
        self.matched_filter.update(
            render,
            &downsampled_capture[..self.sub_block_size],
            self.lag_aggregator.reliable_delay_found(),
        );
        let mut aggregated = self
            .lag_aggregator
            .aggregate(self.matched_filter.best_lag_estimate());
        if aggregated.is_some_and(|estimate| estimate.quality == Quality::Refined) {
            self.clockdrift_detector
                .update(self.lag_aggregator.delay_at_highest_peak());
        }
        if let Some(estimate) = &mut aggregated {
            estimate.delay *= self.down_sampling_factor;
        }
        match (self.old_aggregated_lag, aggregated) {
            (Some(old), Some(new)) if old.delay == new.delay => {
                self.consistent_estimate_counter += 1
            }
            _ => self.consistent_estimate_counter = 0,
        }
        self.old_aggregated_lag = aggregated;
        if self.consistent_estimate_counter > BLOCKS_PER_SECOND / 2 {
            self.reset_parts(false, false);
        }
        aggregated
    }

    fn reset_parts(&mut self, reset_lag_aggregator: bool, reset_delay_confidence: bool) {
        if reset_lag_aggregator {
            self.lag_aggregator.reset(reset_delay_confidence);
        }
        self.matched_filter.reset(reset_lag_aggregator);
        self.old_aggregated_lag = None;
        self.consistent_estimate_counter = 0;
    }
}
