use crate::block::Block;
use crate::common::{FFT_LEN, FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum, safe_clamp};
use crate::fft::{self, FftData, SQRT_HANNING_128};

#[derive(Clone, Debug)]
pub(crate) struct SuppressionFilter {
    e_output_old: Vec<Vec<[f32; FFT_LEN_BY_2]>>,
}

impl SuppressionFilter {
    pub(crate) fn new(bands: usize, capture_channels: usize) -> SuppressionFilter {
        SuppressionFilter {
            e_output_old: vec![vec![[0.0; FFT_LEN_BY_2]; capture_channels]; bands],
        }
    }

    pub(crate) fn apply_gain(
        &mut self,
        comfort_noise: &[FftData],
        comfort_noise_high_band: &[FftData],
        suppression_gain: &Spectrum,
        high_bands_gain: f32,
        e_lowest_band: &[FftData],
        e: &mut Block,
    ) {
        let mut noise_gain = [0.0_f32; SPECTRUM_LEN];
        for (noise_gain, gain) in noise_gain.iter_mut().zip(suppression_gain) {
            *noise_gain = (1.0 - gain * gain).sqrt();
        }
        let high_bands_noise_scaling = 0.4 * (1.0 - high_bands_gain * high_bands_gain).sqrt();
        const IFFT_NORMALIZATION: f32 = 2.0 / FFT_LEN as f32;

        for ch in 0..e.channels() {
            let mut spectrum = FftData::default();
            spectrum.assign(&e_lowest_band[ch]);
            for i in 0..SPECTRUM_LEN {
                let re = spectrum.re[i] * suppression_gain[i];
                let im = spectrum.im[i] * suppression_gain[i];
                spectrum.re[i] = re + noise_gain[i] * comfort_noise[ch].re[i];
                spectrum.im[i] = im + noise_gain[i] * comfort_noise[ch].im[i];
            }
            let mut extended = [0.0; FFT_LEN];
            fft::ifft(&spectrum, &mut extended);
            let old = &mut self.e_output_old[0][ch];
            let e0 = e.view_mut(0, ch);
            for i in 0..FFT_LEN_BY_2 {
                let mut value = old[i] * SQRT_HANNING_128[FFT_LEN_BY_2 + i];
                value += extended[i] * SQRT_HANNING_128[i];
                e0[i] = value * IFFT_NORMALIZATION;
            }
            old.copy_from_slice(&extended[FFT_LEN_BY_2..]);

            for band in 1..e.bands() {
                for sample in e.view_mut(band, ch) {
                    *sample *= high_bands_gain;
                }
            }
            if e.bands() > 1 {
                spectrum.assign(&comfort_noise_high_band[ch]);
                let mut high_band_noise = [0.0; FFT_LEN];
                fft::ifft(&spectrum, &mut high_band_noise);
                let gain = high_bands_noise_scaling * IFFT_NORMALIZATION;
                for (sample, noise) in e.view_mut(1, ch).iter_mut().zip(&high_band_noise) {
                    *sample += noise * gain;
                }
            }
            for (band, old) in self
                .e_output_old
                .iter_mut()
                .enumerate()
                .skip(1)
                .take(e.bands() - 1)
            {
                std::mem::swap(e.view_mut(band, ch), &mut old[ch]);
            }
            for band in 0..e.bands() {
                for sample in e.view_mut(band, ch) {
                    *sample = safe_clamp(*sample, -32_768.0, 32_767.0);
                }
            }
        }
    }
}
