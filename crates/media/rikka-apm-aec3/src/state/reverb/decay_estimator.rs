use crate::common::{FFT_LEN_BY_2, FFT_LEN_BY_2_LOG2, fast_approx_log2, time_domain_len};
use crate::config::Config;

const EARLY_REVERB_MIN_SIZE_BLOCKS: i32 = 3;
const BLOCKS_PER_SECTION: i32 = 6;
const EARLY_REVERB_FIRST_POINT_AT_LINEAR_REGRESSORS: f32 =
    -0.5 * BLOCKS_PER_SECTION as f32 * FFT_LEN_BY_2 as f32 + 0.5;

#[derive(Clone, Debug)]
pub(crate) struct ReverbDecayEstimator {
    filter_length_blocks: i32,
    filter_length_coefficients: usize,
    use_adaptive_echo_decay: bool,
    late_reverb_decay_estimator: LateReverbLinearRegressor,
    early_reverb_estimator: EarlyReverbLengthEstimator,
    late_reverb_start: i32,
    late_reverb_end: i32,
    block_to_analyze: i32,
    estimation_region_candidate_size: i32,
    estimation_region_identified: bool,
    previous_gains: Vec<f32>,
    decay: f32,
    mild_decay: f32,
    tail_gain: f32,
    smoothing_constant: f32,
}

impl ReverbDecayEstimator {
    pub(crate) fn new(config: &Config) -> ReverbDecayEstimator {
        let filter_length_blocks = config.filter.refined.length_blocks as i32;
        ReverbDecayEstimator {
            filter_length_blocks,
            filter_length_coefficients: time_domain_len(config.filter.refined.length_blocks),
            use_adaptive_echo_decay: config.ep_strength.default_len < 0.0,
            late_reverb_decay_estimator: LateReverbLinearRegressor::default(),
            early_reverb_estimator: EarlyReverbLengthEstimator::new(
                filter_length_blocks - EARLY_REVERB_MIN_SIZE_BLOCKS,
            ),
            late_reverb_start: EARLY_REVERB_MIN_SIZE_BLOCKS,
            late_reverb_end: EARLY_REVERB_MIN_SIZE_BLOCKS,
            block_to_analyze: 0,
            estimation_region_candidate_size: 0,
            estimation_region_identified: false,
            previous_gains: vec![0.0; config.filter.refined.length_blocks],
            decay: config.ep_strength.default_len.abs(),
            mild_decay: config.ep_strength.nearend_len.abs(),
            tail_gain: 0.0,
            smoothing_constant: 0.0,
        }
    }

    pub(crate) fn decay(&self, mild: bool) -> f32 {
        if self.use_adaptive_echo_decay || !mild {
            self.decay
        } else {
            self.mild_decay
        }
    }

    pub(crate) fn update(
        &mut self,
        filter: &[f32],
        filter_quality: Option<f32>,
        filter_delay_blocks: i32,
        usable_linear_filter: bool,
        stationary_signal: bool,
    ) {
        if stationary_signal {
            return;
        }
        let estimation_feasible = filter_delay_blocks
            < self.filter_length_blocks - EARLY_REVERB_MIN_SIZE_BLOCKS
            && filter.len() == self.filter_length_coefficients
            && filter_delay_blocks > 0
            && usable_linear_filter;
        if !estimation_feasible {
            self.reset_decay_estimation();
            return;
        }
        if !self.use_adaptive_echo_decay {
            return;
        }
        let new_smoothing = filter_quality.map_or(0.0, |quality| quality * 0.2);
        self.smoothing_constant = new_smoothing.max(self.smoothing_constant);
        if self.smoothing_constant == 0.0 {
            return;
        }
        if self.block_to_analyze < self.filter_length_blocks {
            self.analyze_filter(filter);
            self.block_to_analyze += 1;
        } else {
            self.estimate_decay(filter, filter_delay_blocks);
        }
    }

    fn reset_decay_estimation(&mut self) {
        self.early_reverb_estimator.reset();
        self.late_reverb_decay_estimator.reset(0);
        self.block_to_analyze = 0;
        self.estimation_region_candidate_size = 0;
        self.estimation_region_identified = false;
        self.smoothing_constant = 0.0;
        self.late_reverb_start = 0;
        self.late_reverb_end = 0;
    }

    fn estimate_decay(&mut self, h: &[f32], peak_block: i32) {
        self.block_to_analyze =
            (peak_block + EARLY_REVERB_MIN_SIZE_BLOCKS).min(self.filter_length_blocks);
        let first_reverb_gain = block_energy_average(h, self.block_to_analyze as usize);
        let h_size_blocks = h.len() >> FFT_LEN_BY_2_LOG2;
        self.tail_gain = block_energy_average(h, h_size_blocks - 1);
        let peak_energy = block_energy_peak(h, peak_block as usize);
        let sufficient_reverb_decay = first_reverb_gain > 4.0 * self.tail_gain;
        let valid_filter = first_reverb_gain > 2.0 * self.tail_gain && peak_energy < 100.0;

        let size_early_reverb = self.early_reverb_estimator.estimate();
        let size_late_reverb = (self.estimation_region_candidate_size - size_early_reverb).max(0);
        if size_late_reverb >= 5 {
            if valid_filter && self.late_reverb_decay_estimator.estimate_available() {
                let mut decay =
                    2.0_f32.powf(self.late_reverb_decay_estimator.estimate() * FFT_LEN_BY_2 as f32);
                const MAX_DECAY: f32 = 0.95;
                const MIN_DECAY: f32 = 0.02;
                decay = (0.97 * self.decay).max(decay);
                decay = decay.min(MAX_DECAY);
                decay = decay.max(MIN_DECAY);
                self.decay += self.smoothing_constant * (decay - self.decay);
            }
            self.late_reverb_decay_estimator
                .reset(size_late_reverb * FFT_LEN_BY_2 as i32);
            self.late_reverb_start = peak_block + EARLY_REVERB_MIN_SIZE_BLOCKS + size_early_reverb;
            self.late_reverb_end =
                self.block_to_analyze + self.estimation_region_candidate_size - 1;
        } else {
            self.late_reverb_decay_estimator.reset(0);
            self.late_reverb_start = 0;
            self.late_reverb_end = 0;
        }
        self.estimation_region_identified = !(valid_filter && sufficient_reverb_decay);
        self.estimation_region_candidate_size = 0;
        self.smoothing_constant = 0.0;
        self.early_reverb_estimator.reset();
    }

    fn analyze_filter(&mut self, filter: &[f32]) {
        let start = self.block_to_analyze as usize * FFT_LEN_BY_2;
        let mut h2 = [0.0_f32; FFT_LEN_BY_2];
        for (h2, h) in h2.iter_mut().zip(&filter[start..start + FFT_LEN_BY_2]) {
            *h2 = h * h;
        }
        let gain = (crate::common::sum(&h2) * (1.0 / FFT_LEN_BY_2 as f32)).max(1e-32);
        let previous_gain = &mut self.previous_gains[self.block_to_analyze as usize];
        let adapting = *previous_gain > 1.1 * gain || *previous_gain < 0.9 * gain;
        let above_noise_floor = gain > self.tail_gain;
        *previous_gain = gain;

        self.estimation_region_identified =
            self.estimation_region_identified || adapting || !above_noise_floor;
        if !self.estimation_region_identified {
            self.estimation_region_candidate_size += 1;
        }
        if self.block_to_analyze <= self.late_reverb_end {
            let late = self.block_to_analyze >= self.late_reverb_start;
            for h2_k in h2 {
                let h2_log2 = fast_approx_log2((f64::from(h2_k) + 1e-10) as f32);
                if late {
                    self.late_reverb_decay_estimator.accumulate(h2_log2);
                }
                self.early_reverb_estimator
                    .accumulate(h2_log2, self.smoothing_constant);
            }
        }
    }
}

fn block_energy_peak(h: &[f32], peak_block: usize) -> f32 {
    let block = &h[peak_block * FFT_LEN_BY_2..(peak_block + 1) * FFT_LEN_BY_2];
    let mut peak = block[0];
    for value in &block[1..] {
        if peak * peak < value * value {
            peak = *value;
        }
    }
    peak * peak
}

fn block_energy_average(h: &[f32], block_index: usize) -> f32 {
    const ONE_BY_FFT_LEN_BY_2: f32 = 1.0 / FFT_LEN_BY_2 as f32;
    crate::common::sum_of_squares(&h[block_index * FFT_LEN_BY_2..(block_index + 1) * FFT_LEN_BY_2])
        * ONE_BY_FFT_LEN_BY_2
}

const fn symmetric_arithmetic_sum(n: i32) -> f32 {
    n as f32 * ((n * n) as f32 - 1.0) * (1.0 / 12.0)
}

#[derive(Clone, Debug, Default)]
struct LateReverbLinearRegressor {
    nz: f32,
    nn: f32,
    count: f32,
    n_total: i32,
    n: i32,
}

impl LateReverbLinearRegressor {
    fn reset(&mut self, num_data_points: i32) {
        self.nz = 0.0;
        self.nn = symmetric_arithmetic_sum(num_data_points);
        self.count = if num_data_points > 0 {
            -num_data_points as f32 * 0.5 + 0.5
        } else {
            0.0
        };
        self.n_total = num_data_points;
        self.n = 0;
    }

    fn accumulate(&mut self, z: f32) {
        self.nz += self.count * z;
        self.count += 1.0;
        self.n += 1;
    }

    fn estimate(&self) -> f32 {
        if self.nn == 0.0 {
            0.0
        } else {
            self.nz / self.nn
        }
    }

    fn estimate_available(&self) -> bool {
        self.n == self.n_total && self.n_total != 0
    }
}

#[derive(Clone, Debug)]
struct EarlyReverbLengthEstimator {
    numerators_smooth: Vec<f32>,
    numerators: Vec<f32>,
    coefficients_counter: i32,
    block_counter: i32,
    n_sections: i32,
}

impl EarlyReverbLengthEstimator {
    fn new(max_blocks: i32) -> EarlyReverbLengthEstimator {
        let len = (max_blocks - BLOCKS_PER_SECTION) as usize;
        EarlyReverbLengthEstimator {
            numerators_smooth: vec![0.0; len],
            numerators: vec![0.0; len],
            coefficients_counter: 0,
            block_counter: 0,
            n_sections: 0,
        }
    }

    fn reset(&mut self) {
        self.coefficients_counter = 0;
        self.numerators.fill(0.0);
        self.block_counter = 0;
    }

    fn accumulate(&mut self, value: f32, smoothing: f32) {
        let first_section_index = (self.block_counter - BLOCKS_PER_SECTION + 1).max(0);
        let last_section_index = self.block_counter.min(self.numerators.len() as i32 - 1);
        let x_value =
            self.coefficients_counter as f32 + EARLY_REVERB_FIRST_POINT_AT_LINEAR_REGRESSORS;
        let value_to_inc = FFT_LEN_BY_2 as f32 * value;
        let mut value_to_add =
            x_value * value + (self.block_counter - last_section_index) as f32 * value_to_inc;
        let mut section = last_section_index;
        while section >= first_section_index {
            self.numerators[section as usize] += value_to_add;
            section -= 1;
            value_to_add += value_to_inc;
        }
        self.coefficients_counter += 1;
        if self.coefficients_counter == FFT_LEN_BY_2 as i32 {
            if self.block_counter >= BLOCKS_PER_SECTION - 1 {
                let section = (self.block_counter - (BLOCKS_PER_SECTION - 1)) as usize;
                if let (Some(smooth), Some(numerator)) = (
                    self.numerators_smooth.get_mut(section),
                    self.numerators.get(section),
                ) {
                    *smooth += smoothing * (numerator - *smooth);
                    self.n_sections = section as i32 + 1;
                }
            }
            self.block_counter += 1;
            self.coefficients_counter = 0;
        }
    }

    fn estimate(&self) -> i32 {
        const N: i32 = BLOCKS_PER_SECTION * FFT_LEN_BY_2 as i32;
        const NN: f32 = symmetric_arithmetic_sum(N);
        #[expect(
            clippy::excessive_precision,
            reason = "values copied verbatim from WebRTC"
        )]
        const NUMERATOR_11: f32 = 0.137_503_523_749_935_02 * NN / FFT_LEN_BY_2 as f32;
        #[expect(
            clippy::excessive_precision,
            reason = "values copied verbatim from WebRTC"
        )]
        const NUMERATOR_08: f32 = -0.321_928_094_887_362_29 * NN / FFT_LEN_BY_2 as f32;
        const SECTIONS_TO_ANALYZE: i32 = 9;
        if self.n_sections < SECTIONS_TO_ANALYZE {
            return 0;
        }
        let tail = &self.numerators_smooth[SECTIONS_TO_ANALYZE as usize..self.n_sections as usize];
        let min_numerator_tail = tail.iter().copied().fold(f32::INFINITY, f32::min);
        let mut early_reverb_size_minus_1 = 0;
        for (k, numerator) in self
            .numerators_smooth
            .iter()
            .take(SECTIONS_TO_ANALYZE as usize)
            .enumerate()
        {
            if *numerator > NUMERATOR_11
                || (*numerator < NUMERATOR_08 && *numerator < 0.9 * min_numerator_tail)
            {
                early_reverb_size_minus_1 = k as i32;
            }
        }
        if early_reverb_size_minus_1 == 0 {
            0
        } else {
            early_reverb_size_minus_1 + 1
        }
    }
}
