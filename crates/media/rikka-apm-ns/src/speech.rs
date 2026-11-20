use crate::common::{BINS, LONG_STARTUP_BLOCKS, Spectrum};
use crate::fast_math::exp;
use crate::signal_model::{Observation, SignalModelEstimator};

pub(crate) struct SpeechProbabilityEstimator {
    signal_model: SignalModelEstimator,
    prior_speech_probability: f32,
    speech_probability: Spectrum,
}

impl SpeechProbabilityEstimator {
    pub(crate) fn new() -> SpeechProbabilityEstimator {
        SpeechProbabilityEstimator {
            signal_model: SignalModelEstimator::new(),
            prior_speech_probability: 0.5,
            speech_probability: [0.0; BINS],
        }
    }

    pub(crate) fn prior_probability(&self) -> f32 {
        self.prior_speech_probability
    }

    pub(crate) fn probability(&self) -> &Spectrum {
        &self.speech_probability
    }

    pub(crate) fn update(&mut self, num_analyzed_frames: i32, observation: &Observation<'_>) {
        const WIDTH_PRIOR_0: f32 = 4.0;
        const WIDTH_PRIOR_1: f32 = 2.0 * WIDTH_PRIOR_0;

        if num_analyzed_frames < LONG_STARTUP_BLOCKS {
            self.signal_model
                .adjust_normalization(num_analyzed_frames, observation.signal_energy);
        }
        self.signal_model.update(observation);

        let model = self.signal_model.model();
        let prior = self.signal_model.prior_model();

        let width_prior = if model.lrt < prior.lrt {
            WIDTH_PRIOR_1
        } else {
            WIDTH_PRIOR_0
        };
        let indicator0 = 0.5 * ((width_prior * (model.lrt - prior.lrt)).tanh() + 1.0);

        let width_prior = if model.spectral_flatness > prior.flatness_threshold {
            WIDTH_PRIOR_1
        } else {
            WIDTH_PRIOR_0
        };
        let indicator1 = 0.5
            * ((1.0 * width_prior * (prior.flatness_threshold - model.spectral_flatness)).tanh()
                + 1.0);

        let width_prior = if model.spectral_diff < prior.template_diff_threshold {
            WIDTH_PRIOR_1
        } else {
            WIDTH_PRIOR_0
        };
        let indicator2 = 0.5
            * ((width_prior * (model.spectral_diff - prior.template_diff_threshold)).tanh() + 1.0);

        let ind_prior = prior.lrt_weighting * indicator0
            + prior.flatness_weighting * indicator1
            + prior.difference_weighting * indicator2;

        self.prior_speech_probability += 0.1 * (ind_prior - self.prior_speech_probability);
        self.prior_speech_probability = self.prior_speech_probability.clamp(0.01, 1.0);

        let gain_prior =
            (1.0 - self.prior_speech_probability) / (self.prior_speech_probability + 0.0001);

        for (probability, &avg_log_lrt) in
            self.speech_probability.iter_mut().zip(&model.avg_log_lrt)
        {
            *probability = 1.0 / (1.0 + gain_prior * exp(-avg_log_lrt));
        }
    }
}
