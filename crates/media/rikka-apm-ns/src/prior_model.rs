use crate::common::{BIN_SIZE_DIFF, BIN_SIZE_FLATNESS, BIN_SIZE_LRT, FEATURE_UPDATE_WINDOW};
use crate::histograms::{Histogram, Histograms};

pub(crate) struct PriorModelEstimator {
    pub(crate) lrt: f32,
    pub(crate) flatness_threshold: f32,
    pub(crate) template_diff_threshold: f32,
    pub(crate) lrt_weighting: f32,
    pub(crate) flatness_weighting: f32,
    pub(crate) difference_weighting: f32,
}

#[derive(Clone, Copy)]
struct Peak {
    position: f32,
    weight: i32,
}

impl PriorModelEstimator {
    pub(crate) fn new(lrt: f32) -> PriorModelEstimator {
        PriorModelEstimator {
            lrt,
            flatness_threshold: 0.5,
            template_diff_threshold: 0.5,
            lrt_weighting: 1.0,
            flatness_weighting: 0.0,
            difference_weighting: 0.0,
        }
    }

    pub(crate) fn update(&mut self, histograms: &Histograms) {
        let low_lrt_fluctuations = self.update_lrt(&histograms.lrt);

        let flatness = first_of_two_largest_peaks(BIN_SIZE_FLATNESS, &histograms.spectral_flatness);
        let diff = first_of_two_largest_peaks(BIN_SIZE_DIFF, &histograms.spectral_diff);

        const MINIMUM_PEAK_WEIGHT: f32 = 0.3 * 500.0;
        let use_spec_flat =
            !((flatness.weight as f32) < MINIMUM_PEAK_WEIGHT || flatness.position < 0.6);
        let use_spec_diff = !((diff.weight as f32) < MINIMUM_PEAK_WEIGHT || low_lrt_fluctuations);

        self.template_diff_threshold = (1.2 * diff.position).clamp(0.16, 1.0);

        let one_by_feature_sum =
            1.0 / (1.0 + f32::from(u8::from(use_spec_flat)) + f32::from(u8::from(use_spec_diff)));
        self.lrt_weighting = one_by_feature_sum;

        if use_spec_flat {
            self.flatness_threshold = (0.9 * flatness.position).clamp(0.1, 0.95);
            self.flatness_weighting = one_by_feature_sum;
        } else {
            self.flatness_weighting = 0.0;
        }

        self.difference_weighting = if use_spec_diff {
            one_by_feature_sum
        } else {
            0.0
        };
    }

    fn update_lrt(&mut self, histogram: &Histogram) -> bool {
        const MAX_LRT: f32 = 1.0;
        const MIN_LRT: f32 = 0.2;
        const ONE_BY_WINDOW: f32 = 1.0 / FEATURE_UPDATE_WINDOW as f32;

        let mut average = 0.0f32;
        let mut count = 0i32;
        for (i, &bin) in histogram.iter().enumerate().take(10) {
            let bin_mid = (i as f32 + 0.5) * BIN_SIZE_LRT;
            average += bin as f32 * bin_mid;
            count += bin;
        }
        if count > 0 {
            average /= count as f32;
        }

        let mut average_squared = 0.0f32;
        let mut average_compl = 0.0f32;
        for (i, &bin) in histogram.iter().enumerate() {
            let bin_mid = (i as f32 + 0.5) * BIN_SIZE_LRT;
            average_squared += bin as f32 * bin_mid * bin_mid;
            average_compl += bin as f32 * bin_mid;
        }
        average_squared *= ONE_BY_WINDOW;
        average_compl *= ONE_BY_WINDOW;

        let low_lrt_fluctuations = average_squared - average * average_compl < 0.05;
        self.lrt = if low_lrt_fluctuations {
            MAX_LRT
        } else {
            (1.2 * average).clamp(MIN_LRT, MAX_LRT)
        };
        low_lrt_fluctuations
    }
}

fn first_of_two_largest_peaks(bin_size: f32, histogram: &Histogram) -> Peak {
    let mut peak = Peak {
        position: 0.0,
        weight: 0,
    };
    let mut secondary = peak;

    for (i, &value) in histogram.iter().enumerate() {
        let candidate = Peak {
            position: (i as f32 + 0.5) * bin_size,
            weight: value,
        };
        if value > peak.weight {
            secondary = peak;
            peak = candidate;
        } else if value > secondary.weight {
            secondary = candidate;
        }
    }

    if (secondary.position - peak.position).abs() < 2.0 * bin_size
        && secondary.weight as f32 > 0.5 * peak.weight as f32
    {
        peak.weight += secondary.weight;
        peak.position = 0.5 * (peak.position + secondary.position);
    }
    peak
}
