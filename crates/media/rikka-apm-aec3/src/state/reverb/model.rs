use crate::common::{SPECTRUM_LEN, Spectrum};

#[derive(Clone, Debug)]
pub(crate) struct ReverbModel {
    reverb: Spectrum,
}

impl ReverbModel {
    pub(crate) fn new() -> ReverbModel {
        ReverbModel {
            reverb: [0.0; SPECTRUM_LEN],
        }
    }

    pub(crate) fn reverb(&self) -> &Spectrum {
        &self.reverb
    }

    pub(crate) fn update_without_shaping(
        &mut self,
        power_spectrum: &[f32],
        scaling: f32,
        decay: f32,
    ) {
        if decay > 0.0 {
            for (reverb, power) in self.reverb.iter_mut().zip(power_spectrum) {
                *reverb = (*reverb + power * scaling) * decay;
            }
        }
    }

    pub(crate) fn update(&mut self, power_spectrum: &[f32], scaling: &[f32], decay: f32) {
        if decay > 0.0 {
            for ((reverb, power), scaling) in
                self.reverb.iter_mut().zip(power_spectrum).zip(scaling)
            {
                *reverb = (*reverb + power * scaling) * decay;
            }
        }
    }
}
