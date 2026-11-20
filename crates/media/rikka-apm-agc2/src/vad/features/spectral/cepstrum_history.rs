use crate::vad::common::{CEPSTRAL_HISTORY, NUM_BANDS};

#[derive(Clone, Debug)]
pub(crate) struct CepstrumHistory {
    tail: usize,
    values: [[f32; NUM_BANDS]; CEPSTRAL_HISTORY],
}

impl CepstrumHistory {
    pub(crate) fn new() -> CepstrumHistory {
        CepstrumHistory {
            tail: 0,
            values: [[0.0; NUM_BANDS]; CEPSTRAL_HISTORY],
        }
    }

    pub(crate) fn push(&mut self, cepstrum: &[f32; NUM_BANDS]) {
        self.values[self.tail] = *cepstrum;
        self.tail = (self.tail + 1) % CEPSTRAL_HISTORY;
    }

    pub(crate) fn get(&self, delay: usize) -> &[f32; NUM_BANDS] {
        &self.values[(self.tail + 2 * CEPSTRAL_HISTORY - 1 - delay) % CEPSTRAL_HISTORY]
    }
}
