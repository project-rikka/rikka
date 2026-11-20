mod activations;
mod dense;
mod gru;
mod weights;

use dense::{Activation, Dense};
use gru::Gru;

use crate::vad::common::FEATURE_VECTOR;

#[derive(Clone, Debug)]
pub(crate) struct Rnn {
    input: Dense,
    hidden: Gru,
    output: Dense,
}

impl Rnn {
    pub(crate) fn new() -> Rnn {
        Rnn {
            input: Dense::new(
                FEATURE_VECTOR,
                &weights::INPUT_DENSE_BIAS,
                &weights::INPUT_DENSE_WEIGHTS,
                Activation::Tansig,
            ),
            hidden: Gru::new(
                weights::INPUT_DENSE_BIAS.len(),
                &weights::HIDDEN_GRU_BIAS,
                &weights::HIDDEN_GRU_WEIGHTS,
                &weights::HIDDEN_GRU_RECURRENT_WEIGHTS,
            ),
            output: Dense::new(
                weights::HIDDEN_GRU_BIAS.len() / 3,
                &weights::OUTPUT_DENSE_BIAS,
                &weights::OUTPUT_DENSE_WEIGHTS,
                Activation::Sigmoid,
            ),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.hidden.reset();
    }

    pub(crate) fn probability(
        &mut self,
        features: &[f32; FEATURE_VECTOR],
        is_silence: bool,
    ) -> f32 {
        if is_silence {
            self.reset();
            return 0.0;
        }
        self.input.compute(features);
        self.hidden.compute(self.input.output());
        self.output.compute(self.hidden.state());
        self.output.output()[0]
    }
}
