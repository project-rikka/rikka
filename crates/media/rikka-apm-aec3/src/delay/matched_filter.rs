use crate::ring::Ring;

const ACCUMULATED_ERROR_SUB_SAMPLE_RATE: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LagEstimate {
    pub(crate) lag: usize,
    pub(crate) pre_echo_lag: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct MatchedFilterSettings {
    pub(crate) sub_block_size: usize,
    pub(crate) window_size_sub_blocks: usize,
    pub(crate) filters: usize,
    pub(crate) alignment_shift_sub_blocks: usize,
    pub(crate) excitation_limit: f32,
    pub(crate) smoothing_fast: f32,
    pub(crate) smoothing_slow: f32,
    pub(crate) matching_filter_threshold: f32,
    pub(crate) detect_pre_echo: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct MatchedFilter {
    filter_intra_lag_shift: usize,
    filters: Vec<Vec<f32>>,
    accumulated_error: Vec<Vec<f32>>,
    instantaneous_accumulated_error: Vec<f32>,
    reported_lag_estimate: Option<LagEstimate>,
    winner_lag: Option<usize>,
    last_detected_best_lag_filter: Option<usize>,
    number_pre_echo_updates: usize,
    excitation_limit: f32,
    smoothing_fast: f32,
    smoothing_slow: f32,
    matching_filter_threshold: f32,
    detect_pre_echo: bool,
    sub_block_size: usize,
}

impl MatchedFilter {
    pub(crate) fn new(settings: &MatchedFilterSettings) -> MatchedFilter {
        let filter_len = settings.window_size_sub_blocks * settings.sub_block_size;
        let error_len = filter_len / ACCUMULATED_ERROR_SUB_SAMPLE_RATE;
        let (accumulated_error, instantaneous_accumulated_error) = if settings.detect_pre_echo {
            (
                vec![vec![1.0; error_len]; settings.filters],
                vec![0.0; error_len],
            )
        } else {
            (Vec::new(), Vec::new())
        };
        MatchedFilter {
            filter_intra_lag_shift: settings.alignment_shift_sub_blocks * settings.sub_block_size,
            filters: vec![vec![0.0; filter_len]; settings.filters],
            accumulated_error,
            instantaneous_accumulated_error,
            reported_lag_estimate: None,
            winner_lag: None,
            last_detected_best_lag_filter: None,
            number_pre_echo_updates: 0,
            excitation_limit: settings.excitation_limit,
            smoothing_fast: settings.smoothing_fast,
            smoothing_slow: settings.smoothing_slow,
            matching_filter_threshold: settings.matching_filter_threshold,
            detect_pre_echo: settings.detect_pre_echo,
            sub_block_size: settings.sub_block_size,
        }
    }

    pub(crate) fn max_filter_lag(&self) -> usize {
        self.filters.len() * self.filter_intra_lag_shift + self.filters[0].len()
    }

    pub(crate) fn best_lag_estimate(&self) -> Option<LagEstimate> {
        self.reported_lag_estimate
    }

    pub(crate) fn reset(&mut self, full_reset: bool) {
        for filter in &mut self.filters {
            filter.fill(0.0);
        }
        self.winner_lag = None;
        self.reported_lag_estimate = None;
        if full_reset {
            for error in &mut self.accumulated_error {
                error.fill(1.0);
            }
            self.number_pre_echo_updates = 0;
        }
    }

    pub(crate) fn update(&mut self, render: &Ring<f32>, capture: &[f32], use_slow_smoothing: bool) {
        let y = capture;
        let smoothing = if use_slow_smoothing {
            self.smoothing_slow
        } else {
            self.smoothing_fast
        };
        let x2_sum_threshold =
            self.filters[0].len() as f32 * self.excitation_limit * self.excitation_limit;
        let error_sum_anchor = crate::common::sum_of_squares(y);
        let mut winner_error_sum = error_sum_anchor;
        self.winner_lag = None;
        self.reported_lag_estimate = None;
        let mut alignment_shift = 0;
        let mut previous_lag_estimate = None;
        let mut winner_index = None;
        for n in 0..self.filters.len() {
            let compute_pre_echo =
                self.detect_pre_echo && Some(n) == self.last_detected_best_lag_filter;
            let x_start_index =
                (render.read + alignment_shift + self.sub_block_size - 1) % render.len();
            let core = filter_core(
                x_start_index,
                x2_sum_threshold,
                smoothing,
                &render.buffer,
                y,
                &mut self.filters[n],
                compute_pre_echo.then_some(&mut self.instantaneous_accumulated_error[..]),
            );
            let filter = &self.filters[n];
            let lag_estimate = max_square_peak_index(filter);
            let reliable = lag_estimate > 2
                && lag_estimate < filter.len() - 10
                && core.error_sum < self.matching_filter_threshold * error_sum_anchor;
            let lag = lag_estimate + alignment_shift;
            if core.filters_updated && reliable && core.error_sum < winner_error_sum {
                winner_error_sum = core.error_sum;
                winner_index = Some(n);
                if previous_lag_estimate == Some(lag) {
                    self.winner_lag = previous_lag_estimate;
                    winner_index = Some(n - 1);
                } else {
                    self.winner_lag = Some(lag);
                }
            }
            previous_lag_estimate = Some(lag);
            alignment_shift += self.filter_intra_lag_shift;
        }

        let (Some(winner_index), Some(winner_lag)) = (winner_index, self.winner_lag) else {
            return;
        };
        let mut estimate = LagEstimate {
            lag: winner_lag,
            pre_echo_lag: winner_lag,
        };
        if self.detect_pre_echo && self.last_detected_best_lag_filter == Some(winner_index) {
            const ENERGY_THRESHOLD: f32 = 1.0;
            if error_sum_anchor > ENERGY_THRESHOLD {
                update_accumulated_error(
                    &self.instantaneous_accumulated_error,
                    &mut self.accumulated_error[winner_index],
                    1.0 / error_sum_anchor,
                );
                self.number_pre_echo_updates += 1;
            }
            if self.number_pre_echo_updates >= 50 {
                estimate.pre_echo_lag = compute_pre_echo_lag(
                    &self.accumulated_error[winner_index],
                    winner_lag,
                    winner_index * self.filter_intra_lag_shift,
                );
            }
        }
        self.reported_lag_estimate = Some(estimate);
        self.last_detected_best_lag_filter = Some(winner_index);
    }
}

struct CoreResult {
    filters_updated: bool,
    error_sum: f32,
}

fn filter_core(
    mut x_start_index: usize,
    x2_sum_threshold: f32,
    smoothing: f32,
    x: &[f32],
    y: &[f32],
    h: &mut [f32],
    mut accumulated_error: Option<&mut [f32]>,
) -> CoreResult {
    let mut result = CoreResult {
        filters_updated: false,
        error_sum: 0.0,
    };
    if let Some(error) = accumulated_error.as_deref_mut() {
        error.fill(0.0);
    }
    let last = x.len() - 1;
    for &y_i in y {
        let mut x2_sum = 0.0_f32;
        let mut s = 0.0_f32;
        let mut x_index = x_start_index;
        if let Some(error) = accumulated_error.as_deref_mut() {
            for (k, h_k) in h.iter().enumerate() {
                x2_sum += x[x_index] * x[x_index];
                s += h_k * x[x_index];
                x_index = if x_index < last { x_index + 1 } else { 0 };
                if (k + 1) & 0b11 == 0 {
                    error[k >> 2] += (y_i - s) * (y_i - s);
                }
            }
        } else {
            for h_k in h.iter() {
                x2_sum += x[x_index] * x[x_index];
                s += h_k * x[x_index];
                x_index = if x_index < last { x_index + 1 } else { 0 };
            }
        }
        let e = y_i - s;
        let saturation = y_i >= 32000.0 || y_i <= -32000.0;
        result.error_sum += e * e;
        if x2_sum > x2_sum_threshold && !saturation {
            let alpha = smoothing * e / x2_sum;
            let mut x_index = x_start_index;
            for h_k in h.iter_mut() {
                *h_k += alpha * x[x_index];
                x_index = if x_index < last { x_index + 1 } else { 0 };
            }
            result.filters_updated = true;
        }
        x_start_index = if x_start_index > 0 {
            x_start_index - 1
        } else {
            last
        };
    }
    result
}

fn max_square_peak_index(h: &[f32]) -> usize {
    if h.len() < 2 {
        return 0;
    }
    let mut max_element1 = h[0] * h[0];
    let mut max_element2 = h[1] * h[1];
    let mut lag_estimate1 = 0;
    let mut lag_estimate2 = 1;
    let last_index = h.len() - 1;
    let mut k = 2;
    while k < last_index {
        let element1 = h[k] * h[k];
        let element2 = h[k + 1] * h[k + 1];
        if element1 > max_element1 {
            max_element1 = element1;
            lag_estimate1 = k;
        }
        if element2 > max_element2 {
            max_element2 = element2;
            lag_estimate2 = k + 1;
        }
        k += 2;
    }
    if max_element2 > max_element1 {
        max_element1 = max_element2;
        lag_estimate1 = lag_estimate2;
    }
    let last_element = h[last_index] * h[last_index];
    if last_element > max_element1 {
        return last_index;
    }
    lag_estimate1
}

fn update_accumulated_error(
    instantaneous: &[f32],
    accumulated: &mut [f32],
    one_over_error_sum_anchor: f32,
) {
    const SMOOTH_CONSTANT_INCREASES: f32 = 0.015;
    for (accumulated, instantaneous) in accumulated.iter_mut().zip(instantaneous) {
        let error_norm = instantaneous * one_over_error_sum_anchor;
        if error_norm < *accumulated {
            *accumulated = error_norm;
        } else {
            *accumulated += SMOOTH_CONSTANT_INCREASES * (error_norm - *accumulated);
        }
    }
}

fn compute_pre_echo_lag(
    accumulated_error: &[f32],
    lag: usize,
    alignment_shift_winner: usize,
) -> usize {
    const PRE_ECHO_THRESHOLD: f32 = 0.5;
    let mut pre_echo_lag_estimate = lag - alignment_shift_winner;
    let maximum_pre_echo_lag =
        (pre_echo_lag_estimate / ACCUMULATED_ERROR_SUB_SAMPLE_RATE).min(accumulated_error.len());
    for k in (0..maximum_pre_echo_lag).rev() {
        if accumulated_error[k] > PRE_ECHO_THRESHOLD {
            break;
        }
        pre_echo_lag_estimate = (k + 1) * ACCUMULATED_ERROR_SUB_SAMPLE_RATE - 1;
    }
    pre_echo_lag_estimate + alignment_shift_winner
}
