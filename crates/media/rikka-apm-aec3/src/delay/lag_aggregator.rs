use crate::common::{BLOCK_LEN_LOG2, BLOCKS_PER_SECOND, MATCHED_FILTER_WINDOW_SUB_BLOCKS};
use crate::config::{Delay, DelaySelectionThresholds};
use crate::delay::estimate::{DelayEstimate, Quality};
use crate::delay::matched_filter::LagEstimate;

const HISTOGRAM_DATA_LEN: usize = 250;

#[derive(Clone, Debug)]
pub(crate) struct LagAggregator {
    significant_candidate_found: bool,
    thresholds: DelaySelectionThresholds,
    headroom: i32,
    highest_peak: HighestPeakAggregator,
    pre_echo: Option<PreEchoLagAggregator>,
}

impl LagAggregator {
    pub(crate) fn new(max_filter_lag: usize, config: &Delay) -> LagAggregator {
        LagAggregator {
            significant_candidate_found: false,
            thresholds: config.delay_selection_thresholds,
            headroom: (config.delay_headroom_samples / config.down_sampling_factor) as i32,
            highest_peak: HighestPeakAggregator::new(max_filter_lag),
            pre_echo: config
                .detect_pre_echo
                .then(|| PreEchoLagAggregator::new(max_filter_lag, config.down_sampling_factor)),
        }
    }

    pub(crate) fn reset(&mut self, hard_reset: bool) {
        self.highest_peak.reset();
        if let Some(pre_echo) = &mut self.pre_echo {
            pre_echo.reset();
        }
        if hard_reset {
            self.significant_candidate_found = false;
        }
    }

    pub(crate) fn reliable_delay_found(&self) -> bool {
        self.significant_candidate_found
    }

    pub(crate) fn delay_at_highest_peak(&self) -> i32 {
        self.highest_peak.candidate
    }

    pub(crate) fn aggregate(&mut self, lag_estimate: Option<LagEstimate>) -> Option<DelayEstimate> {
        let estimate = lag_estimate?;
        if let Some(pre_echo) = &mut self.pre_echo {
            pre_echo.aggregate((estimate.pre_echo_lag as i32 - self.headroom).max(0));
        }
        self.highest_peak
            .aggregate((estimate.lag as i32 - self.headroom).max(0));
        let candidate = self.highest_peak.candidate;
        let count = self.highest_peak.histogram[candidate as usize];
        self.significant_candidate_found =
            self.significant_candidate_found || count > self.thresholds.converged;
        if count > self.thresholds.converged
            || (count > self.thresholds.initial && !self.significant_candidate_found)
        {
            let quality = if self.significant_candidate_found {
                Quality::Refined
            } else {
                Quality::Coarse
            };
            let reported = self
                .pre_echo
                .as_ref()
                .map_or(candidate, |pre_echo| pre_echo.candidate);
            return Some(DelayEstimate::new(quality, reported as usize));
        }
        None
    }
}

#[derive(Clone, Debug)]
struct HighestPeakAggregator {
    histogram: Vec<i32>,
    histogram_data: [usize; HISTOGRAM_DATA_LEN],
    histogram_data_index: usize,
    candidate: i32,
}

impl HighestPeakAggregator {
    fn new(max_filter_lag: usize) -> HighestPeakAggregator {
        HighestPeakAggregator {
            histogram: vec![0; max_filter_lag + 1],
            histogram_data: [0; HISTOGRAM_DATA_LEN],
            histogram_data_index: 0,
            candidate: -1,
        }
    }

    fn reset(&mut self) {
        self.histogram.fill(0);
        self.histogram_data = [0; HISTOGRAM_DATA_LEN];
        self.histogram_data_index = 0;
    }

    fn aggregate(&mut self, lag: i32) {
        let slot = &mut self.histogram_data[self.histogram_data_index];
        self.histogram[*slot] -= 1;
        *slot = lag as usize;
        self.histogram[*slot] += 1;
        self.histogram_data_index = (self.histogram_data_index + 1) % HISTOGRAM_DATA_LEN;
        self.candidate = first_max_index(&self.histogram) as i32;
    }
}

#[derive(Clone, Debug)]
struct PreEchoLagAggregator {
    block_size_log2: usize,
    histogram_data: [Option<usize>; HISTOGRAM_DATA_LEN],
    histogram: Vec<i32>,
    histogram_data_index: usize,
    candidate: i32,
    number_updates: usize,
}

impl PreEchoLagAggregator {
    fn new(max_filter_lag: usize, down_sampling_factor: usize) -> PreEchoLagAggregator {
        let mut factor = down_sampling_factor >> 1;
        let mut factor_log2 = 0;
        while factor > 0 {
            factor_log2 += 1;
            factor >>= 1;
        }
        PreEchoLagAggregator {
            block_size_log2: BLOCK_LEN_LOG2.saturating_sub(factor_log2),
            histogram_data: [None; HISTOGRAM_DATA_LEN],
            histogram: vec![0; ((max_filter_lag + 1) * down_sampling_factor) >> BLOCK_LEN_LOG2],
            histogram_data_index: 0,
            candidate: 0,
            number_updates: 0,
        }
    }

    fn reset(&mut self) {
        self.histogram.fill(0);
        self.histogram_data = [None; HISTOGRAM_DATA_LEN];
        self.histogram_data_index = 0;
        self.candidate = 0;
    }

    fn aggregate(&mut self, pre_echo_lag: i32) {
        let pre_echo_block_size =
            ((pre_echo_lag >> self.block_size_log2) as usize).min(self.histogram.len() - 1);
        let slot = &mut self.histogram_data[self.histogram_data_index];
        if let Some(previous) = *slot {
            self.histogram[previous] -= 1;
        }
        *slot = Some(pre_echo_block_size);
        self.histogram[pre_echo_block_size] += 1;
        self.histogram_data_index = (self.histogram_data_index + 1) % HISTOGRAM_DATA_LEN;

        let candidate_block_size = if self.number_updates < BLOCKS_PER_SECOND * 2 {
            self.number_updates += 1;
            let mut penalization_per_delay = 1.0_f32;
            let mut max_histogram_value = -1.0_f32;
            let mut candidate = 0;
            let mut start = 0;
            while self.histogram.len() - start >= MATCHED_FILTER_WINDOW_SUB_BLOCKS {
                let window = &self.histogram[start..start + MATCHED_FILTER_WINDOW_SUB_BLOCKS];
                let max_index = start + first_max_index(window);
                let weighted_max_value = self.histogram[max_index] as f32 * penalization_per_delay;
                if weighted_max_value > max_histogram_value {
                    max_histogram_value = weighted_max_value;
                    candidate = max_index;
                }
                penalization_per_delay *= 0.7;
                start += MATCHED_FILTER_WINDOW_SUB_BLOCKS;
            }
            candidate
        } else {
            first_max_index(&self.histogram)
        };
        self.candidate = (candidate_block_size << self.block_size_log2) as i32;
    }
}

fn first_max_index(values: &[i32]) -> usize {
    let mut best = 0;
    for (index, value) in values.iter().enumerate() {
        if *value > values[best] {
            best = index;
        }
    }
    best
}
