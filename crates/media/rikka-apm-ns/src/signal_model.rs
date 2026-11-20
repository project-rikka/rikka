use crate::common::{BINS, FEATURE_UPDATE_WINDOW, LRT_FEATURE_THRESHOLD, ONE_BY_BINS, Spectrum};
use crate::fast_math::{exp, log};
use crate::histograms::Histograms;
use crate::prior_model::PriorModelEstimator;

pub(crate) struct SignalModel {
    pub(crate) lrt: f32,
    pub(crate) spectral_diff: f32,
    pub(crate) spectral_flatness: f32,
    pub(crate) avg_log_lrt: Spectrum,
}

impl SignalModel {
    fn new() -> SignalModel {
        const FLATNESS_FEATURE_THRESHOLD: f32 = 0.5;
        SignalModel {
            lrt: LRT_FEATURE_THRESHOLD,
            spectral_diff: FLATNESS_FEATURE_THRESHOLD,
            spectral_flatness: FLATNESS_FEATURE_THRESHOLD,
            avg_log_lrt: [LRT_FEATURE_THRESHOLD; BINS],
        }
    }
}

pub(crate) struct Observation<'a> {
    pub(crate) prior_snr: &'a Spectrum,
    pub(crate) post_snr: &'a Spectrum,
    pub(crate) conservative_noise: &'a Spectrum,
    pub(crate) signal_spectrum: &'a Spectrum,
    pub(crate) signal_spectral_sum: f32,
    pub(crate) signal_energy: f32,
}

pub(crate) struct SignalModelEstimator {
    diff_normalization: f32,
    signal_energy_sum: f32,
    histograms: Histograms,
    histogram_analysis_counter: i32,
    prior_model: PriorModelEstimator,
    features: SignalModel,
}

impl SignalModelEstimator {
    pub(crate) fn new() -> SignalModelEstimator {
        SignalModelEstimator {
            diff_normalization: 0.0,
            signal_energy_sum: 0.0,
            histograms: Histograms::new(),
            histogram_analysis_counter: FEATURE_UPDATE_WINDOW,
            prior_model: PriorModelEstimator::new(LRT_FEATURE_THRESHOLD),
            features: SignalModel::new(),
        }
    }

    pub(crate) fn model(&self) -> &SignalModel {
        &self.features
    }

    pub(crate) fn prior_model(&self) -> &PriorModelEstimator {
        &self.prior_model
    }

    pub(crate) fn adjust_normalization(&mut self, num_analyzed_frames: i32, signal_energy: f32) {
        self.diff_normalization *= num_analyzed_frames as f32;
        self.diff_normalization += signal_energy;
        self.diff_normalization /= (num_analyzed_frames + 1) as f32;
    }

    pub(crate) fn update(&mut self, observation: &Observation<'_>) {
        update_spectral_flatness(
            observation.signal_spectrum,
            observation.signal_spectral_sum,
            &mut self.features.spectral_flatness,
        );

        let spectral_diff = spectral_diff(
            observation.conservative_noise,
            observation.signal_spectrum,
            observation.signal_spectral_sum,
            self.diff_normalization,
        );
        self.features.spectral_diff += 0.3 * (spectral_diff - self.features.spectral_diff);

        self.signal_energy_sum += observation.signal_energy;

        self.histogram_analysis_counter -= 1;
        if self.histogram_analysis_counter > 0 {
            self.histograms.update(&self.features);
        } else {
            self.prior_model.update(&self.histograms);
            self.histograms.clear();
            self.histogram_analysis_counter = FEATURE_UPDATE_WINDOW;
            self.signal_energy_sum /= FEATURE_UPDATE_WINDOW as f32;
            self.diff_normalization = 0.5 * (self.signal_energy_sum + self.diff_normalization);
            self.signal_energy_sum = 0.0;
        }

        update_spectral_lrt(
            observation.prior_snr,
            observation.post_snr,
            &mut self.features.avg_log_lrt,
            &mut self.features.lrt,
        );
    }
}

fn spectral_diff(
    conservative_noise: &Spectrum,
    signal_spectrum: &Spectrum,
    signal_spectral_sum: f32,
    diff_normalization: f32,
) -> f32 {
    let mut noise_average = 0.0f32;
    for &noise in conservative_noise {
        noise_average += noise;
    }
    noise_average *= ONE_BY_BINS;
    let signal_average = signal_spectral_sum * ONE_BY_BINS;

    let mut covariance = 0.0f32;
    let mut noise_variance = 0.0f32;
    let mut signal_variance = 0.0f32;
    for (&signal, &noise) in signal_spectrum.iter().zip(conservative_noise) {
        let signal_diff = signal - signal_average;
        let noise_diff = noise - noise_average;
        covariance += signal_diff * noise_diff;
        noise_variance += noise_diff * noise_diff;
        signal_variance += signal_diff * signal_diff;
    }
    covariance *= ONE_BY_BINS;
    noise_variance *= ONE_BY_BINS;
    signal_variance *= ONE_BY_BINS;

    let spectral_diff = signal_variance - (covariance * covariance) / (noise_variance + 0.0001);
    spectral_diff / (diff_normalization + 0.0001)
}

fn update_spectral_flatness(
    signal_spectrum: &Spectrum,
    signal_spectral_sum: f32,
    spectral_flatness: &mut f32,
) {
    const AVERAGING: f32 = 0.3;
    if signal_spectrum[1..].contains(&0.0) {
        *spectral_flatness -= AVERAGING * *spectral_flatness;
        return;
    }

    let mut num = 0.0f32;
    for &value in &signal_spectrum[1..] {
        num += log(value);
    }
    let mut denom = signal_spectral_sum - signal_spectrum[0];
    denom *= ONE_BY_BINS;
    num *= ONE_BY_BINS;

    let spectral_tmp = exp(num) / denom;
    *spectral_flatness += AVERAGING * (spectral_tmp - *spectral_flatness);
}

fn update_spectral_lrt(
    prior_snr: &Spectrum,
    post_snr: &Spectrum,
    avg_log_lrt: &mut Spectrum,
    lrt: &mut f32,
) {
    for i in 0..BINS {
        let tmp1 = 1.0 + 2.0 * prior_snr[i];
        let tmp2 = 2.0 * prior_snr[i] / (tmp1 + 0.0001);
        let bessel_tmp = (post_snr[i] + 1.0) * tmp2;
        avg_log_lrt[i] += 0.5 * (bessel_tmp - log(tmp1) - avg_log_lrt[i]);
    }

    let mut sum = 0.0f32;
    for &value in avg_log_lrt.iter() {
        sum += value;
    }
    *lrt = sum * ONE_BY_BINS;
}
