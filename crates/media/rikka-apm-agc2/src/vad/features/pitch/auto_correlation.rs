use crate::vad::common::{BUF_12K, MAX_PITCH_12K, NUM_LAGS_12K};
use rikka_fft::RealFft;

const FFT_SIZE: usize = 512;
const CONVOLUTION_LENGTH: usize = BUF_12K - MAX_PITCH_12K;

#[derive(Clone, Debug)]
pub(crate) struct AutoCorrelation {
    fft: RealFft,
    tmp: Vec<f32>,
    x: Vec<f32>,
    h: Vec<f32>,
}

impl AutoCorrelation {
    pub(crate) fn new() -> AutoCorrelation {
        AutoCorrelation {
            fft: RealFft::new(FFT_SIZE),
            tmp: vec![0.0; FFT_SIZE],
            x: vec![0.0; FFT_SIZE],
            h: vec![0.0; FFT_SIZE],
        }
    }

    pub(crate) fn compute(
        &mut self,
        pitch_buffer: &[f32; BUF_12K],
        auto_corr: &mut [f32; NUM_LAGS_12K],
    ) {
        for (target, source) in self
            .tmp
            .iter_mut()
            .zip(pitch_buffer[BUF_12K - CONVOLUTION_LENGTH..].iter().rev())
        {
            *target = *source;
        }
        self.tmp[CONVOLUTION_LENGTH..].fill(0.0);
        self.fft.forward(&self.tmp, &mut self.h);

        let length = CONVOLUTION_LENGTH + NUM_LAGS_12K;
        self.tmp[..length].copy_from_slice(&pitch_buffer[..length]);
        self.tmp[length..].fill(0.0);
        self.fft.forward(&self.tmp, &mut self.x);

        self.tmp.fill(0.0);
        self.fft
            .convolve_accumulate(&self.x, &self.h, &mut self.tmp, 1.0 / FFT_SIZE as f32);
        self.fft.backward(&mut self.tmp);

        auto_corr.copy_from_slice(
            &self.tmp[CONVOLUTION_LENGTH - 1..CONVOLUTION_LENGTH - 1 + NUM_LAGS_12K],
        );
    }
}
