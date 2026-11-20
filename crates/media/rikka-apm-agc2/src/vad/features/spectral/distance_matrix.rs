use crate::vad::common::CEPSTRAL_HISTORY;

const S: usize = CEPSTRAL_HISTORY;

#[derive(Clone, Debug)]
pub(crate) struct DistanceMatrix {
    values: [f32; (S - 1) * (S - 1)],
}

impl DistanceMatrix {
    pub(crate) fn new() -> DistanceMatrix {
        DistanceMatrix {
            values: [0.0; (S - 1) * (S - 1)],
        }
    }

    pub(crate) fn push(&mut self, distances: &[f32; S - 1]) {
        self.values.copy_within(S.., 0);
        for (i, &distance) in distances.iter().enumerate() {
            self.values[(S - 1 - i) * (S - 1) - 1] = distance;
        }
    }

    pub(crate) fn get(&self, delay1: usize, delay2: usize) -> f32 {
        let mut row = S - 1 - delay1;
        let mut col = S - 1 - delay2;
        if row > col {
            std::mem::swap(&mut row, &mut col);
        }
        self.values[row * (S - 1) + (col - 1)]
    }
}
