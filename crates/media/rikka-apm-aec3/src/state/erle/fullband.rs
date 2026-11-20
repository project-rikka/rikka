use crate::common::{Spectrum, fast_approx_log2};
use crate::config::Erle;

const EPSILON: f32 = 1e-3;
const X2_BAND_ENERGY_THRESHOLD: f32 = 44_015_068.0;
const BLOCKS_TO_HOLD_ERLE: i32 = 100;
const POINTS_TO_ACCUMULATE: i32 = 6;

#[derive(Clone, Debug)]
pub(crate) struct FullbandErleEstimator {
    min_erle_log2: f32,
    hold_counters: Vec<i32>,
    erle_time_domain_log2: Vec<f32>,
    instantaneous: Vec<InstantaneousErle>,
    linear_filters_qualities: Vec<Option<f32>>,
}

impl FullbandErleEstimator {
    pub(crate) fn new(config: &Erle, capture_channels: usize) -> FullbandErleEstimator {
        let min_erle_log2 = fast_approx_log2(config.min + EPSILON);
        let mut estimator = FullbandErleEstimator {
            min_erle_log2,
            hold_counters: vec![0; capture_channels],
            erle_time_domain_log2: vec![min_erle_log2; capture_channels],
            instantaneous: vec![InstantaneousErle::new(config); capture_channels],
            linear_filters_qualities: vec![None; capture_channels],
        };
        estimator.reset();
        estimator
    }

    pub(crate) fn reset(&mut self) {
        for instantaneous in &mut self.instantaneous {
            instantaneous.reset();
        }
        self.update_quality_estimates();
        self.erle_time_domain_log2.fill(self.min_erle_log2);
        self.hold_counters.fill(0);
    }

    pub(crate) fn fullband_erle_log2(&self) -> f32 {
        let mut min = self.erle_time_domain_log2[0];
        for value in &self.erle_time_domain_log2[1..] {
            min = min.min(*value);
        }
        min
    }

    pub(crate) fn inst_linear_quality_estimates(&self) -> &[Option<f32>] {
        &self.linear_filters_qualities
    }

    pub(crate) fn update(
        &mut self,
        x2: &Spectrum,
        y2: &[Spectrum],
        e2: &[Spectrum],
        converged_filters: &[bool],
    ) {
        for ch in 0..y2.len() {
            if converged_filters[ch] {
                let x2_sum = crate::common::sum(x2);
                if x2_sum > X2_BAND_ENERGY_THRESHOLD * x2.len() as f32 {
                    let y2_sum = crate::common::sum(&y2[ch]);
                    let e2_sum = crate::common::sum(&e2[ch]);
                    if let Some(erle_log2) = self.instantaneous[ch].update(y2_sum, e2_sum) {
                        self.hold_counters[ch] = BLOCKS_TO_HOLD_ERLE;
                        let erle = &mut self.erle_time_domain_log2[ch];
                        *erle += 0.05 * (erle_log2 - *erle);
                        *erle = erle.max(self.min_erle_log2);
                    }
                }
            }
            self.hold_counters[ch] = self.hold_counters[ch].saturating_sub(1);
            if self.hold_counters[ch] == 0 {
                self.instantaneous[ch].reset_accumulators();
            }
        }
        self.update_quality_estimates();
    }

    fn update_quality_estimates(&mut self) {
        for (quality, instantaneous) in self
            .linear_filters_qualities
            .iter_mut()
            .zip(&self.instantaneous)
        {
            *quality = instantaneous.quality_estimate();
        }
    }
}

#[derive(Clone, Debug)]
struct InstantaneousErle {
    clamp_quality_to_zero: bool,
    clamp_quality_to_one: bool,
    erle_log2: Option<f32>,
    inst_quality_estimate: f32,
    max_erle_log2: f32,
    min_erle_log2: f32,
    y2_accumulated: f32,
    e2_accumulated: f32,
    points: i32,
}

impl InstantaneousErle {
    fn new(config: &Erle) -> InstantaneousErle {
        let mut erle = InstantaneousErle {
            clamp_quality_to_zero: config.clamp_quality_estimate_to_zero,
            clamp_quality_to_one: config.clamp_quality_estimate_to_one,
            erle_log2: None,
            inst_quality_estimate: 0.0,
            max_erle_log2: 0.0,
            min_erle_log2: 0.0,
            y2_accumulated: 0.0,
            e2_accumulated: 0.0,
            points: 0,
        };
        erle.reset();
        erle
    }

    fn reset(&mut self) {
        self.reset_accumulators();
        self.max_erle_log2 = -10.0;
        self.min_erle_log2 = 33.0;
        self.inst_quality_estimate = 0.0;
    }

    fn reset_accumulators(&mut self) {
        self.erle_log2 = None;
        self.inst_quality_estimate = 0.0;
        self.points = 0;
        self.e2_accumulated = 0.0;
        self.y2_accumulated = 0.0;
    }

    fn update(&mut self, y2_sum: f32, e2_sum: f32) -> Option<f32> {
        let mut updated = false;
        self.e2_accumulated += e2_sum;
        self.y2_accumulated += y2_sum;
        self.points += 1;
        if self.points == POINTS_TO_ACCUMULATE {
            if self.e2_accumulated > 0.0 {
                updated = true;
                self.erle_log2 = Some(fast_approx_log2(
                    self.y2_accumulated / self.e2_accumulated + EPSILON,
                ));
            }
            self.points = 0;
            self.e2_accumulated = 0.0;
            self.y2_accumulated = 0.0;
        }
        if !updated {
            return None;
        }
        let erle_log2 = self.erle_log2?;
        self.max_erle_log2 -= 0.0004;
        self.max_erle_log2 = self.max_erle_log2.max(erle_log2);
        self.min_erle_log2 += 0.0004;
        self.min_erle_log2 = self.min_erle_log2.min(erle_log2);

        const ALPHA: f32 = 0.07;
        let mut quality_estimate = 0.0;
        if self.max_erle_log2 > self.min_erle_log2 {
            quality_estimate =
                (erle_log2 - self.min_erle_log2) / (self.max_erle_log2 - self.min_erle_log2);
        }
        if quality_estimate > self.inst_quality_estimate {
            self.inst_quality_estimate = quality_estimate;
        } else {
            self.inst_quality_estimate += ALPHA * (quality_estimate - self.inst_quality_estimate);
        }
        Some(erle_log2)
    }

    fn quality_estimate(&self) -> Option<f32> {
        self.erle_log2?;
        let mut value = self.inst_quality_estimate;
        if self.clamp_quality_to_zero {
            value = value.max(0.0);
        }
        if self.clamp_quality_to_one {
            value = value.min(1.0);
        }
        Some(value)
    }
}
