use crate::biquad::{BiQuadCoefficients, CascadedBiQuad};

const COEFFICIENTS_16KHZ: BiQuadCoefficients = BiQuadCoefficients {
    b: [0.972_61, -1.945_23, 0.972_61],
    a: [-1.944_48, 0.945_98],
};

#[derive(Clone, Debug)]
pub(crate) struct HighPassFilter {
    filters: Vec<CascadedBiQuad>,
}

impl HighPassFilter {
    pub(crate) fn new(channels: usize) -> HighPassFilter {
        HighPassFilter {
            filters: vec![CascadedBiQuad::from_coefficients(COEFFICIENTS_16KHZ, 1); channels],
        }
    }

    pub(crate) fn process(&mut self, channel: usize, samples: &mut [f32]) {
        self.filters[channel].process_in_place(samples);
    }
}
