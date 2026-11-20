use std::time::Duration;

const DECAY: (u32, u32) = (31, 32);

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DecayingPeak(Duration);

impl DecayingPeak {
    pub(crate) fn add(&mut self, sample: Duration) {
        let (numerator, denominator) = DECAY;
        self.0 = (self.0 * numerator / denominator).max(sample);
    }

    pub(crate) fn get(self) -> Duration {
        self.0
    }
}
