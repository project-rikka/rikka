use crate::common::{BLOCK_LEN, SPECTRUM_LEN, Spectrum};
use crate::fft::FftData;

#[derive(Clone, Debug)]
pub(crate) struct SubtractorOutput {
    pub(crate) s_refined: [f32; BLOCK_LEN],
    pub(crate) s_coarse: [f32; BLOCK_LEN],
    pub(crate) e_refined: [f32; BLOCK_LEN],
    pub(crate) e_coarse: [f32; BLOCK_LEN],
    pub(crate) e_refined_fft: FftData,
    pub(crate) e2_refined_spectrum: Spectrum,
    pub(crate) e2_coarse_spectrum: Spectrum,
    pub(crate) s2_refined: f32,
    pub(crate) s2_coarse: f32,
    pub(crate) e2_refined: f32,
    pub(crate) e2_coarse: f32,
    pub(crate) y2: f32,
    pub(crate) s_refined_max_abs: f32,
    pub(crate) s_coarse_max_abs: f32,
}

impl Default for SubtractorOutput {
    fn default() -> SubtractorOutput {
        SubtractorOutput {
            s_refined: [0.0; BLOCK_LEN],
            s_coarse: [0.0; BLOCK_LEN],
            e_refined: [0.0; BLOCK_LEN],
            e_coarse: [0.0; BLOCK_LEN],
            e_refined_fft: FftData::default(),
            e2_refined_spectrum: [0.0; SPECTRUM_LEN],
            e2_coarse_spectrum: [0.0; SPECTRUM_LEN],
            s2_refined: 0.0,
            s2_coarse: 0.0,
            e2_refined: 0.0,
            e2_coarse: 0.0,
            y2: 0.0,
            s_refined_max_abs: 0.0,
            s_coarse_max_abs: 0.0,
        }
    }
}

impl SubtractorOutput {
    pub(crate) fn compute_metrics(&mut self, y: &[f32]) {
        use crate::common::sum_of_squares;
        self.y2 = sum_of_squares(y);
        self.e2_refined = sum_of_squares(&self.e_refined);
        self.e2_coarse = sum_of_squares(&self.e_coarse);
        self.s2_refined = sum_of_squares(&self.s_refined);
        self.s2_coarse = sum_of_squares(&self.s_coarse);
        self.s_refined_max_abs = max_abs(&self.s_refined);
        self.s_coarse_max_abs = max_abs(&self.s_coarse);
    }
}

fn max_abs(values: &[f32]) -> f32 {
    let mut max = values[0];
    let mut min = values[0];
    for value in &values[1..] {
        if *value > max {
            max = *value;
        }
        if *value < min {
            min = *value;
        }
    }
    max.max(-min)
}
