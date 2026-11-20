use crate::common::{BIN_SIZE_DIFF, BIN_SIZE_FLATNESS, BIN_SIZE_LRT};
use crate::signal_model::SignalModel;

pub(crate) const HISTOGRAM_SIZE: usize = 1000;

pub(crate) type Histogram = [i32; HISTOGRAM_SIZE];

pub(crate) struct Histograms {
    pub(crate) lrt: Histogram,
    pub(crate) spectral_flatness: Histogram,
    pub(crate) spectral_diff: Histogram,
}

impl Histograms {
    pub(crate) fn new() -> Histograms {
        Histograms {
            lrt: [0; HISTOGRAM_SIZE],
            spectral_flatness: [0; HISTOGRAM_SIZE],
            spectral_diff: [0; HISTOGRAM_SIZE],
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Histograms::new();
    }

    pub(crate) fn update(&mut self, features: &SignalModel) {
        count(&mut self.lrt, features.lrt, BIN_SIZE_LRT);
        count(
            &mut self.spectral_flatness,
            features.spectral_flatness,
            BIN_SIZE_FLATNESS,
        );
        count(
            &mut self.spectral_diff,
            features.spectral_diff,
            BIN_SIZE_DIFF,
        );
    }
}

fn count(histogram: &mut Histogram, value: f32, bin_size: f32) {
    if value < HISTOGRAM_SIZE as f32 * bin_size
        && value >= 0.0
        && let Some(bin) = histogram.get_mut(((1.0 / bin_size) * value) as usize)
    {
        *bin += 1;
    }
}
