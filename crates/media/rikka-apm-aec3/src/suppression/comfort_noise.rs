use crate::common::{FFT_LEN_BY_2, SPECTRUM_LEN, Spectrum};
use crate::fft::FftData;

#[expect(clippy::approx_constant, reason = "values copied verbatim from WebRTC")]
const SQRT2_SIN: [f32; 32] = [
    0.0,
    0.275_899_4,
    0.541_196_1,
    0.785_695,
    1.0,
    1.175_875_6,
    1.306_563,
    1.387_039_8,
    1.414_213_6,
    1.387_039_8,
    1.306_563,
    1.175_875_6,
    1.0,
    0.785_695,
    0.541_196_1,
    0.275_899_4,
    0.0,
    -0.275_899_4,
    -0.541_196_1,
    -0.785_695,
    -1.0,
    -1.175_875_6,
    -1.306_563,
    -1.387_039_8,
    -1.414_213_6,
    -1.387_039_8,
    -1.306_563,
    -1.175_875_6,
    -1.0,
    -0.785_695,
    -0.541_196_1,
    -0.275_899_4,
];

#[derive(Clone, Debug)]
pub(crate) struct ComfortNoiseGenerator {
    seed: u32,
    noise_floor: f32,
    n2_initial: Option<Vec<Spectrum>>,
    y2_smoothed: Vec<Spectrum>,
    n2: Vec<Spectrum>,
    n2_counter: i32,
}

impl ComfortNoiseGenerator {
    pub(crate) fn new(noise_floor_dbfs: f32, capture_channels: usize) -> ComfortNoiseGenerator {
        #[expect(
            clippy::excessive_precision,
            reason = "values copied verbatim from WebRTC"
        )]
        const DBFS_NORMALIZATION: f32 = 90.308_998_699_194_36;
        ComfortNoiseGenerator {
            seed: 42,
            noise_floor: 64.0 * 10.0_f32.powf((DBFS_NORMALIZATION + noise_floor_dbfs) * 0.1),
            n2_initial: Some(vec![[0.0; SPECTRUM_LEN]; capture_channels]),
            y2_smoothed: vec![[0.0; SPECTRUM_LEN]; capture_channels],
            n2: vec![[1.0e6; SPECTRUM_LEN]; capture_channels],
            n2_counter: 0,
        }
    }

    pub(crate) fn noise_spectrum(&self) -> &[Spectrum] {
        &self.n2
    }

    pub(crate) fn compute(
        &mut self,
        saturated_capture: bool,
        capture_spectrum: &[Spectrum],
        lower_band_noise: &mut [FftData],
        upper_band_noise: &mut [FftData],
    ) {
        if !saturated_capture {
            for (smoothed, y2) in self.y2_smoothed.iter_mut().zip(capture_spectrum) {
                for (a, b) in smoothed.iter_mut().zip(y2) {
                    *a += 0.1 * (b - *a);
                }
            }
            if self.n2_counter > 50 {
                for (n2, smoothed) in self.n2.iter_mut().zip(&self.y2_smoothed) {
                    for (a, b) in n2.iter_mut().zip(smoothed) {
                        *a = if *b < *a {
                            (0.9 * b + 0.1 * *a) * 1.0002
                        } else {
                            *a * 1.0002
                        };
                    }
                }
            }
            if self.n2_initial.is_some() {
                self.n2_counter += 1;
                if self.n2_counter == 1000 {
                    self.n2_initial = None;
                } else if let Some(initial) = &mut self.n2_initial {
                    for (initial, n2) in initial.iter_mut().zip(&self.n2) {
                        for (b, a) in initial.iter_mut().zip(n2) {
                            *b = if *a > *b { *b + 0.001 * (a - *b) } else { *a };
                        }
                    }
                }
            }
            for n2 in &mut self.n2 {
                for n in n2 {
                    *n = n.max(self.noise_floor);
                }
            }
            if let Some(initial) = &mut self.n2_initial {
                for n2 in initial {
                    for n in n2 {
                        *n = n.max(self.noise_floor);
                    }
                }
            }
        }
        let n2 = self.n2_initial.as_ref().unwrap_or(&self.n2);
        for ((n2, low), high) in n2.iter().zip(lower_band_noise).zip(upper_band_noise) {
            generate_comfort_noise(n2, &mut self.seed, low, high);
        }
    }
}

fn generate_comfort_noise(n2: &Spectrum, seed: &mut u32, low: &mut FftData, high: &mut FftData) {
    let mut n = *n2;
    for value in &mut n {
        *value = value.sqrt();
    }
    const ONE_BY_NUM_BANDS: f32 = 1.0 / (SPECTRUM_LEN / 2 + 1) as f32;
    const SPECTRUM_LEN_BY_2: usize = SPECTRUM_LEN / 2;
    let high_band_noise_level = crate::common::sum(&n[SPECTRUM_LEN_BY_2..]) * ONE_BY_NUM_BANDS;
    low.re[0] = 0.0;
    low.re[FFT_LEN_BY_2] = 0.0;
    high.re[0] = 0.0;
    high.re[FFT_LEN_BY_2] = 0.0;
    for k in 1..FFT_LEN_BY_2 {
        const INDEX_MASK: usize = 32 - 1;
        *seed = seed.wrapping_mul(69_069).wrapping_add(1) & (0x8000_0000 - 1);
        let i = (*seed >> 26) as usize;
        let x = SQRT2_SIN[i];
        let y = SQRT2_SIN[(i + 8) & INDEX_MASK];
        low.re[k] = n[k] * x;
        low.im[k] = n[k] * y;
        high.re[k] = high_band_noise_level * x;
        high.im[k] = high_band_noise_level * y;
    }
}
