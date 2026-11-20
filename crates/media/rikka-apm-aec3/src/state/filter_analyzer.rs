use crate::block::Block;
use crate::common::{BLOCK_LEN, BLOCK_LEN_LOG2, BLOCKS_PER_SECOND, FFT_LEN_BY_2, time_domain_len};
use crate::config::Config;
use crate::render::RenderBuffer;

#[expect(
    clippy::excessive_precision,
    reason = "values copied verbatim from WebRTC"
)]
const HIGHPASS: [f32; 3] = [0.792_974_2, -0.360_721_28, -0.470_477_66];

#[derive(Clone, Copy, Debug, Default)]
struct Region {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FilterAnalysis {
    pub(crate) any_filter_consistent: bool,
    pub(crate) max_echo_path_gain: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct FilterAnalyzer {
    bounded_erl: bool,
    default_gain: f32,
    h_highpass: Vec<Vec<f32>>,
    blocks_since_reset: usize,
    region: Region,
    states: Vec<AnalysisState>,
    filter_delays_blocks: Vec<i32>,
}

impl FilterAnalyzer {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> FilterAnalyzer {
        let mut analyzer = FilterAnalyzer {
            bounded_erl: config.ep_strength.bounded_erl,
            default_gain: config.ep_strength.default_gain,
            h_highpass: vec![
                vec![0.0; time_domain_len(config.filter.refined.length_blocks)];
                capture_channels
            ],
            blocks_since_reset: 0,
            region: Region::default(),
            states: vec![AnalysisState::new(config); capture_channels],
            filter_delays_blocks: vec![0; capture_channels],
        };
        analyzer.reset();
        analyzer
    }

    pub(crate) fn reset(&mut self) {
        self.blocks_since_reset = 0;
        self.region = Region::default();
        for state in &mut self.states {
            state.reset(self.default_gain);
        }
        self.filter_delays_blocks.fill(0);
    }

    pub(crate) fn filter_delays_blocks(&self) -> &[i32] {
        &self.filter_delays_blocks
    }

    pub(crate) fn filter_length_blocks(&self) -> i32 {
        self.states[0].filter_length_blocks
    }

    pub(crate) fn adjusted_filters(&self) -> &[Vec<f32>] {
        &self.h_highpass
    }

    pub(crate) fn update(
        &mut self,
        filters_time_domain: &[Vec<f32>],
        render: &RenderBuffer,
    ) -> FilterAnalysis {
        self.blocks_since_reset += 1;
        self.set_region_to_analyze(filters_time_domain[0].len());
        self.analyze_region(filters_time_domain, render);
        let mut analysis = FilterAnalysis {
            any_filter_consistent: self.states[0].consistent_estimate,
            max_echo_path_gain: self.states[0].gain,
        };
        for state in &self.states[1..filters_time_domain.len()] {
            analysis.any_filter_consistent |= state.consistent_estimate;
            analysis.max_echo_path_gain = analysis.max_echo_path_gain.max(state.gain);
        }
        analysis
    }

    fn analyze_region(&mut self, filters_time_domain: &[Vec<f32>], render: &RenderBuffer) {
        self.pre_process_filters(filters_time_domain);
        const ONE_BY_BLOCK_SIZE: f32 = 1.0 / BLOCK_LEN as f32;
        let sufficient_time_to_converge = self.blocks_since_reset > 5 * BLOCKS_PER_SECOND;
        for ch in 0..filters_time_domain.len() {
            let h = &self.h_highpass[ch];
            let state = &mut self.states[ch];
            state.peak_index = state.peak_index.min(h.len() - 1);
            state.peak_index =
                find_peak_index(h, state.peak_index, self.region.start, self.region.end);
            self.filter_delays_blocks[ch] = (state.peak_index >> BLOCK_LEN_LOG2) as i32;
            state.update_gain(h, sufficient_time_to_converge, self.bounded_erl);
            state.filter_length_blocks =
                (filters_time_domain[ch].len() as f32 * ONE_BY_BLOCK_SIZE) as i32;
            let delay_blocks = self.filter_delays_blocks[ch];
            state.consistent_estimate = state.consistent_filter_detector.detect(
                h,
                self.region,
                render.block(-(delay_blocks as isize)),
                state.peak_index,
                delay_blocks,
            );
        }
    }

    fn pre_process_filters(&mut self, filters_time_domain: &[Vec<f32>]) {
        let region = self.region;
        for (h_highpass, filter) in self.h_highpass.iter_mut().zip(filters_time_domain) {
            h_highpass.resize(filter.len(), 0.0);
            h_highpass[region.start..=region.end].fill(0.0);
            for k in (HIGHPASS.len() - 1).max(region.start)..=region.end {
                let mut value = h_highpass[k];
                for (j, coefficient) in HIGHPASS.iter().enumerate() {
                    value += filter[k - j] * coefficient;
                }
                h_highpass[k] = value;
            }
        }
    }

    fn set_region_to_analyze(&mut self, filter_size: usize) {
        const BLOCKS_TO_UPDATE: usize = 1;
        let region = &mut self.region;
        region.start = if region.end >= filter_size - 1 {
            0
        } else {
            region.end + 1
        };
        region.end = (region.start + BLOCKS_TO_UPDATE * BLOCK_LEN - 1).min(filter_size - 1);
    }
}

fn find_peak_index(filter: &[f32], peak_index_in: usize, start: usize, end: usize) -> usize {
    let mut peak_index = peak_index_in;
    let mut max_h2 = filter[peak_index] * filter[peak_index];
    for (k, value) in filter.iter().enumerate().take(end + 1).skip(start) {
        let h2 = value * value;
        if h2 > max_h2 {
            peak_index = k;
            max_h2 = h2;
        }
    }
    peak_index
}

#[derive(Clone, Debug)]
struct AnalysisState {
    gain: f32,
    peak_index: usize,
    filter_length_blocks: i32,
    consistent_estimate: bool,
    consistent_filter_detector: ConsistentFilterDetector,
}

impl AnalysisState {
    fn new(config: &Config) -> AnalysisState {
        let mut state = AnalysisState {
            gain: 0.0,
            peak_index: 0,
            filter_length_blocks: config.filter.refined_initial.length_blocks as i32,
            consistent_estimate: false,
            consistent_filter_detector: ConsistentFilterDetector::new(config),
        };
        state.reset(config.ep_strength.default_gain);
        state
    }

    fn reset(&mut self, default_gain: f32) {
        self.peak_index = 0;
        self.gain = default_gain;
        self.consistent_filter_detector.reset();
    }

    fn update_gain(
        &mut self,
        filter: &[f32],
        sufficient_time_to_converge: bool,
        bounded_erl: bool,
    ) {
        let peak = filter[self.peak_index].abs();
        if sufficient_time_to_converge && self.consistent_estimate {
            self.gain = peak;
        } else if self.gain != 0.0 {
            self.gain = self.gain.max(peak);
        }
        if bounded_erl && self.gain != 0.0 {
            self.gain = self.gain.max(0.01);
        }
    }
}

#[derive(Clone, Debug)]
struct ConsistentFilterDetector {
    significant_peak: bool,
    filter_floor_accum: f32,
    filter_secondary_peak: f32,
    filter_floor_low_limit: usize,
    filter_floor_high_limit: usize,
    active_render_threshold: f32,
    consistent_estimate_counter: usize,
    consistent_delay_reference: i32,
}

impl ConsistentFilterDetector {
    fn new(config: &Config) -> ConsistentFilterDetector {
        let limit = config.render_levels.active_render_limit;
        ConsistentFilterDetector {
            significant_peak: false,
            filter_floor_accum: 0.0,
            filter_secondary_peak: 0.0,
            filter_floor_low_limit: 0,
            filter_floor_high_limit: 0,
            active_render_threshold: limit * limit * FFT_LEN_BY_2 as f32,
            consistent_estimate_counter: 0,
            consistent_delay_reference: -10,
        }
    }

    fn reset(&mut self) {
        self.significant_peak = false;
        self.filter_floor_accum = 0.0;
        self.filter_secondary_peak = 0.0;
        self.filter_floor_low_limit = 0;
        self.filter_floor_high_limit = 0;
        self.consistent_estimate_counter = 0;
        self.consistent_delay_reference = -10;
    }

    fn detect(
        &mut self,
        filter: &[f32],
        region: Region,
        x_block: &Block,
        peak_index: usize,
        delay_blocks: i32,
    ) -> bool {
        if region.start == 0 {
            self.filter_floor_accum = 0.0;
            self.filter_secondary_peak = 0.0;
            self.filter_floor_low_limit = peak_index.saturating_sub(64);
            self.filter_floor_high_limit = if peak_index > filter.len() - 129 {
                0
            } else {
                peak_index + 128
            };
        }
        let mut floor_accum = self.filter_floor_accum;
        let mut secondary_peak = self.filter_secondary_peak;
        for value in &filter[region.start
            ..(region.end + 1)
                .min(self.filter_floor_low_limit)
                .max(region.start)]
        {
            let abs = value.abs();
            floor_accum += abs;
            secondary_peak = secondary_peak.max(abs);
        }
        for value in filter
            .iter()
            .take(region.end + 1)
            .skip(self.filter_floor_high_limit.max(region.start))
        {
            let abs = value.abs();
            floor_accum += abs;
            secondary_peak = secondary_peak.max(abs);
        }
        self.filter_floor_accum = floor_accum;
        self.filter_secondary_peak = secondary_peak;

        if region.end == filter.len() - 1 {
            let floor = self.filter_floor_accum
                / (self.filter_floor_low_limit + filter.len() - self.filter_floor_high_limit)
                    as f32;
            let abs_peak = filter[peak_index].abs();
            self.significant_peak =
                abs_peak > 10.0 * floor && abs_peak > 2.0 * self.filter_secondary_peak;
        }

        if self.significant_peak {
            let active_render_block = (0..x_block.channels()).any(|ch| {
                crate::common::sum_of_squares(x_block.view(0, ch)) > self.active_render_threshold
            });
            if self.consistent_delay_reference == delay_blocks {
                if active_render_block {
                    self.consistent_estimate_counter += 1;
                }
            } else {
                self.consistent_estimate_counter = 0;
                self.consistent_delay_reference = delay_blocks;
            }
        }
        self.consistent_estimate_counter as f32 > 1.5 * BLOCKS_PER_SECOND as f32
    }
}
