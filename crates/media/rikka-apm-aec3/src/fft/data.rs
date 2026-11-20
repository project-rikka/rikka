use crate::common::{FFT_LEN, FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FftData {
    pub(crate) re: Spectrum,
    pub(crate) im: Spectrum,
}

impl Default for FftData {
    fn default() -> FftData {
        FftData {
            re: [0.0; SPECTRUM_LEN],
            im: [0.0; SPECTRUM_LEN],
        }
    }
}

impl FftData {
    pub(crate) fn assign(&mut self, source: &FftData) {
        self.re = source.re;
        self.im = source.im;
        self.im[0] = 0.0;
        self.im[FFT_LEN_BY_2] = 0.0;
    }

    pub(crate) fn clear(&mut self) {
        self.re = [0.0; SPECTRUM_LEN];
        self.im = [0.0; SPECTRUM_LEN];
    }

    pub(crate) fn spectrum(&self, power: &mut Spectrum) {
        for ((power, re), im) in power.iter_mut().zip(&self.re).zip(&self.im) {
            *power = re * re + im * im;
        }
    }

    pub(crate) fn copy_from_packed(&mut self, packed: &[f32; FFT_LEN]) {
        self.re[0] = packed[0];
        self.re[FFT_LEN_BY_2] = packed[1];
        self.im[0] = 0.0;
        self.im[FFT_LEN_BY_2] = 0.0;
        for k in 1..FFT_LEN_BY_2 {
            self.re[k] = packed[2 * k];
            self.im[k] = packed[2 * k + 1];
        }
    }

    pub(crate) fn copy_to_packed(&self, packed: &mut [f32; FFT_LEN]) {
        packed[0] = self.re[0];
        packed[1] = self.re[FFT_LEN_BY_2];
        for k in 1..FFT_LEN_BY_2 {
            packed[2 * k] = self.re[k];
            packed[2 * k + 1] = self.im[k];
        }
    }
}
