use std::f64::consts::PI;

use crate::vad::common::NUM_BANDS;

pub(crate) const OPUS_BANDS_24K: usize = 20;

const BAND_SIZES: [usize; OPUS_BANDS_24K - 1] = [
    4, 4, 4, 4, 4, 4, 4, 4, 8, 8, 8, 8, 16, 16, 16, 24, 24, 32, 48,
];

const WEIGHTS: [f32; 240] = [
    0.0,
    0.25,
    0.5,
    0.75,
    0.0,
    0.25,
    0.5,
    0.75,
    0.0,
    0.25,
    0.5,
    0.75,
    0.0,
    0.25,
    0.5,
    0.75,
    0.0,
    0.25,
    0.5,
    0.75,
    0.0,
    0.25,
    0.5,
    0.75,
    0.0,
    0.25,
    0.5,
    0.75,
    0.0,
    0.25,
    0.5,
    0.75,
    0.0,
    0.125,
    0.25,
    0.375,
    0.5,
    0.625,
    0.75,
    0.875,
    0.0,
    0.125,
    0.25,
    0.375,
    0.5,
    0.625,
    0.75,
    0.875,
    0.0,
    0.125,
    0.25,
    0.375,
    0.5,
    0.625,
    0.75,
    0.875,
    0.0,
    0.125,
    0.25,
    0.375,
    0.5,
    0.625,
    0.75,
    0.875,
    0.0,
    0.0625,
    0.125,
    0.1875,
    0.25,
    0.3125,
    0.375,
    0.4375,
    0.5,
    0.5625,
    0.625,
    0.6875,
    0.75,
    0.8125,
    0.875,
    0.9375,
    0.0,
    0.0625,
    0.125,
    0.1875,
    0.25,
    0.3125,
    0.375,
    0.4375,
    0.5,
    0.5625,
    0.625,
    0.6875,
    0.75,
    0.8125,
    0.875,
    0.9375,
    0.0,
    0.0625,
    0.125,
    0.1875,
    0.25,
    0.3125,
    0.375,
    0.4375,
    0.5,
    0.5625,
    0.625,
    0.6875,
    0.75,
    0.8125,
    0.875,
    0.9375,
    0.0,
    0.041_666_7,
    0.083_333_3,
    0.125,
    0.166_667,
    0.208_333,
    0.25,
    0.291_667,
    0.333_333,
    0.375,
    0.416_667,
    0.458_333,
    0.5,
    0.541_667,
    0.583_333,
    0.625,
    0.666_667,
    0.708_333,
    0.75,
    0.791_667,
    0.833_333,
    0.875,
    0.916_667,
    0.958_333,
    0.0,
    0.041_666_7,
    0.083_333_3,
    0.125,
    0.166_667,
    0.208_333,
    0.25,
    0.291_667,
    0.333_333,
    0.375,
    0.416_667,
    0.458_333,
    0.5,
    0.541_667,
    0.583_333,
    0.625,
    0.666_667,
    0.708_333,
    0.75,
    0.791_667,
    0.833_333,
    0.875,
    0.916_667,
    0.958_333,
    0.0,
    0.03125,
    0.0625,
    0.09375,
    0.125,
    0.15625,
    0.1875,
    0.21875,
    0.25,
    0.28125,
    0.3125,
    0.34375,
    0.375,
    0.40625,
    0.4375,
    0.46875,
    0.5,
    0.53125,
    0.5625,
    0.59375,
    0.625,
    0.65625,
    0.6875,
    0.71875,
    0.75,
    0.78125,
    0.8125,
    0.84375,
    0.875,
    0.90625,
    0.9375,
    0.96875,
    0.0,
    0.020_833_3,
    0.041_666_7,
    0.0625,
    0.083_333_3,
    0.104_167,
    0.125,
    0.145_833,
    0.166_667,
    0.1875,
    0.208_333,
    0.229_167,
    0.25,
    0.270_833,
    0.291_667,
    0.3125,
    0.333_333,
    0.354_167,
    0.375,
    0.395_833,
    0.416_667,
    0.4375,
    0.458_333,
    0.479_167,
    0.5,
    0.520_833,
    0.541_667,
    0.5625,
    0.583_333,
    0.604_167,
    0.625,
    0.645_833,
    0.666_667,
    0.6875,
    0.708_333,
    0.729_167,
    0.75,
    0.770_833,
    0.791_667,
    0.8125,
    0.833_333,
    0.854_167,
    0.875,
    0.895_833,
    0.916_667,
    0.9375,
    0.958_333,
    0.979_167,
];

const DCT_SCALING_FACTOR: f32 = 0.301_511_35;

pub(crate) fn cross_correlation(x: &[f32], y: &[f32], out: &mut [f32; OPUS_BANDS_24K]) {
    let mut k = 0;
    out[0] = 0.0;
    for (band, &size) in BAND_SIZES.iter().enumerate() {
        out[band + 1] = 0.0;
        for _ in 0..size {
            let v = x[2 * k] * y[2 * k] + x[2 * k + 1] * y[2 * k + 1];
            let tmp = WEIGHTS[k] * v;
            out[band] += v - tmp;
            out[band + 1] += tmp;
            k += 1;
        }
    }
    out[0] *= 2.0;
}

pub(crate) fn smoothed_log_magnitude(
    bands_energy: &[f32; OPUS_BANDS_24K],
    log_bands_energy: &mut [f32; NUM_BANDS],
) {
    const ONE_BY_HUNDRED: f32 = 1e-2;
    const LOG_ONE_BY_HUNDRED: f32 = -2.0;
    let mut log_max = LOG_ONE_BY_HUNDRED;
    let mut follow = LOG_ONE_BY_HUNDRED;
    let mut smooth = |x: f32| {
        let x = (log_max - 7.0).max((follow - 1.5).max(x));
        log_max = log_max.max(x);
        follow = (follow - 1.5).max(x);
        x
    };
    for (target, &energy) in log_bands_energy.iter_mut().zip(bands_energy) {
        *target = smooth((ONE_BY_HUNDRED + energy).log10());
    }
    for target in &mut log_bands_energy[OPUS_BANDS_24K..] {
        *target = smooth(LOG_ONE_BY_HUNDRED);
    }
}

pub(crate) fn dct_table() -> [f32; NUM_BANDS * NUM_BANDS] {
    let mut table = [0.0f32; NUM_BANDS * NUM_BANDS];
    let k = 0.5f64.sqrt();
    for i in 0..NUM_BANDS {
        for j in 0..NUM_BANDS {
            table[i * NUM_BANDS + j] =
                ((i as f64 + 0.5) * j as f64 * PI / NUM_BANDS as f64).cos() as f32;
        }
        table[i * NUM_BANDS] = (f64::from(table[i * NUM_BANDS]) * k) as f32;
    }
    table
}

pub(crate) fn dct(input: &[f32], table: &[f32; NUM_BANDS * NUM_BANDS], output: &mut [f32]) {
    for (i, value) in output.iter_mut().enumerate() {
        *value = 0.0;
        for (j, &x) in input.iter().enumerate() {
            *value += x * table[j * NUM_BANDS + i];
        }
        *value *= DCT_SCALING_FACTOR;
    }
}
