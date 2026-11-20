mod decay_estimator;
mod frequency_response;
mod model;

pub(crate) use model::ReverbModel;

use crate::common::Spectrum;
use crate::config::Config;
use decay_estimator::ReverbDecayEstimator;
use frequency_response::ReverbFrequencyResponse;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReverbInput<'a> {
    pub(crate) impulse_responses: &'a [Vec<f32>],
    pub(crate) frequency_responses: &'a [Vec<Spectrum>],
    pub(crate) linear_filter_qualities: &'a [Option<f32>],
    pub(crate) filter_delays_blocks: &'a [i32],
    pub(crate) usable_linear_estimates: &'a [bool],
    pub(crate) stationary_block: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct ReverbModelEstimator {
    decay_estimators: Vec<ReverbDecayEstimator>,
    frequency_responses: Vec<ReverbFrequencyResponse>,
}

impl ReverbModelEstimator {
    pub(crate) fn new(config: &Config, capture_channels: usize) -> ReverbModelEstimator {
        let conservative = config.ep_strength.use_conservative_tail_frequency_response;
        ReverbModelEstimator {
            decay_estimators: vec![ReverbDecayEstimator::new(config); capture_channels],
            frequency_responses: vec![ReverbFrequencyResponse::new(conservative); capture_channels],
        }
    }

    pub(crate) fn reverb_decay(&self, mild: bool) -> f32 {
        self.decay_estimators[0].decay(mild)
    }

    pub(crate) fn reverb_frequency_response(&self) -> &Spectrum {
        self.frequency_responses[0].frequency_response()
    }

    pub(crate) fn update(&mut self, input: &ReverbInput<'_>) {
        for (ch, (decay, response)) in self
            .decay_estimators
            .iter_mut()
            .zip(&mut self.frequency_responses)
            .enumerate()
        {
            let quality = input.linear_filter_qualities[ch];
            let delay = input.filter_delays_blocks[ch];
            response.update(
                &input.frequency_responses[ch],
                delay,
                quality,
                input.stationary_block,
            );
            decay.update(
                &input.impulse_responses[ch],
                quality,
                delay,
                input.usable_linear_estimates[ch],
                input.stationary_block,
            );
        }
    }
}
