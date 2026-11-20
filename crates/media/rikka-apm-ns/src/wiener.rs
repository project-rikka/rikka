use crate::common::{BINS, LONG_STARTUP_BLOCKS, SHORT_STARTUP_BLOCKS, Spectrum};
use crate::level::SuppressionParams;

pub(crate) struct WienerFilter {
    params: SuppressionParams,
    spectrum_prev_process: Spectrum,
    initial_spectral_estimate: Spectrum,
    filter: Spectrum,
}

pub(crate) struct NoiseSpectra<'a> {
    pub(crate) noise: &'a Spectrum,
    pub(crate) prev_noise: &'a Spectrum,
    pub(crate) parametric_noise: &'a Spectrum,
}

impl WienerFilter {
    pub(crate) fn new(params: SuppressionParams) -> WienerFilter {
        WienerFilter {
            params,
            spectrum_prev_process: [0.0; BINS],
            initial_spectral_estimate: [0.0; BINS],
            filter: [1.0; BINS],
        }
    }

    pub(crate) fn filter(&self) -> &Spectrum {
        &self.filter
    }

    pub(crate) fn update(
        &mut self,
        num_analyzed_frames: i32,
        spectra: &NoiseSpectra<'_>,
        signal_spectrum: &Spectrum,
    ) {
        let over_subtraction = self.params.over_subtraction_factor;
        let floor = self.params.minimum_attenuating_gain;
        for i in 0..BINS {
            let prev_tsa =
                self.spectrum_prev_process[i] / (spectra.prev_noise[i] + 0.0001) * self.filter[i];
            let current_tsa = if signal_spectrum[i] > spectra.noise[i] {
                signal_spectrum[i] / (spectra.noise[i] + 0.0001) - 1.0
            } else {
                0.0
            };
            let snr_prior = 0.98 * prev_tsa + (1.0 - 0.98) * current_tsa;
            self.filter[i] = (snr_prior / (over_subtraction + snr_prior)).clamp(floor, 1.0);
        }

        if num_analyzed_frames < SHORT_STARTUP_BLOCKS {
            const ONE_BY_SHORT_STARTUP: f32 = 1.0 / SHORT_STARTUP_BLOCKS as f32;
            let remaining = (SHORT_STARTUP_BLOCKS - num_analyzed_frames) as f32;
            let frames = num_analyzed_frames as f32;
            for i in 0..BINS {
                self.initial_spectral_estimate[i] += signal_spectrum[i];
                let mut filter_initial = self.initial_spectral_estimate[i]
                    - over_subtraction * spectra.parametric_noise[i];
                filter_initial /= self.initial_spectral_estimate[i] + 0.0001;
                filter_initial = filter_initial.clamp(floor, 1.0);
                filter_initial *= remaining;
                self.filter[i] *= frames;
                self.filter[i] += filter_initial;
                self.filter[i] *= ONE_BY_SHORT_STARTUP;
            }
        }

        self.spectrum_prev_process = *signal_spectrum;
    }

    pub(crate) fn overall_scaling_factor(
        &self,
        num_analyzed_frames: i32,
        prior_speech_probability: f32,
        energy_before_filtering: f32,
        energy_after_filtering: f32,
    ) -> f32 {
        const B_LIM: f32 = 0.5;
        if !self.params.use_attenuation_adjustment || num_analyzed_frames <= LONG_STARTUP_BLOCKS {
            return 1.0;
        }

        let mut gain = (energy_after_filtering / (energy_before_filtering + 1.0)).sqrt();

        let mut scale_factor1 = 1.0;
        if gain > B_LIM {
            scale_factor1 = 1.0 + 1.3 * (gain - B_LIM);
            if gain * scale_factor1 > 1.0 {
                scale_factor1 = 1.0 / gain;
            }
        }

        let mut scale_factor2 = 1.0;
        if gain < B_LIM {
            gain = gain.max(self.params.minimum_attenuating_gain);
            scale_factor2 = 1.0 - 0.3 * (B_LIM - gain);
        }

        prior_speech_probability * scale_factor1 + (1.0 - prior_speech_probability) * scale_factor2
    }
}
