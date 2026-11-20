use crate::{BAND, BANDS, FRAME};

const SUB_SAMPLING: usize = BANDS;
const DCT_SIZE: usize = BANDS;
const SPARSITY: usize = 4;
const STRIDE_LOG2: usize = 2;
const STRIDE: usize = 1 << STRIDE_LOG2;
const FILTER_SIZE: usize = 4;
const MEMORY: usize = FILTER_SIZE * STRIDE - 1;
const ZERO_FILTERS: usize = 2;
const FILTERS: usize = SPARSITY * BANDS - ZERO_FILTERS;
const ZERO_FILTER_1: usize = 3;
const ZERO_FILTER_2: usize = 9;

const FILTER_COEFFS: [[f32; FILTER_SIZE]; FILTERS] = [
    [-0.000_477_49, -0.004_968_88, 0.165_471_18, 0.004_254_96],
    [-0.001_732_87, -0.015_857_78, 0.149_890_04, 0.009_941_13],
    [-0.003_048_15, -0.025_360_82, 0.121_545_42, 0.011_579_93],
    [-0.003_469_46, -0.025_878_86, 0.047_604_41, 0.006_075_94],
    [-0.001_547_17, -0.011_360_76, 0.013_874_58, 0.001_863_53],
    [0.001_863_53, 0.013_874_58, -0.011_360_76, -0.001_547_17],
    [0.006_075_94, 0.047_604_41, -0.025_878_86, -0.003_469_46],
    [0.009_832_12, 0.085_431_75, -0.029_827_67, -0.003_835_09],
    [0.009_941_13, 0.149_890_04, -0.015_857_78, -0.001_732_87],
    [0.004_254_96, 0.165_471_18, -0.004_968_88, -0.000_477_49],
];

const DCT_MODULATION: [[f32; DCT_SIZE]; FILTERS] = [
    [2.0, 2.0, 2.0],
    [1.732_050_8, 0.0, -1.732_050_8],
    [1.0, -2.0, 1.0],
    [-1.0, 2.0, -1.0],
    [-1.732_050_8, 0.0, 1.732_050_8],
    [-2.0, -2.0, -2.0],
    [-1.732_050_8, 0.0, 1.732_050_8],
    [-1.0, 2.0, -1.0],
    [1.0, -2.0, 1.0],
    [1.732_050_8, 0.0, -1.732_050_8],
];

#[derive(Clone, Debug)]
pub(crate) struct ThreeBandFilterBank {
    analysis: [[f32; MEMORY]; FILTERS],
    synthesis: [[f32; MEMORY]; FILTERS],
}

impl ThreeBandFilterBank {
    pub(crate) fn new() -> ThreeBandFilterBank {
        ThreeBandFilterBank {
            analysis: [[0.0; MEMORY]; FILTERS],
            synthesis: [[0.0; MEMORY]; FILTERS],
        }
    }

    pub(crate) fn analysis(&mut self, input: &[f32; FRAME], out: &mut [[f32; BAND]; BANDS]) {
        for band in out.iter_mut() {
            band.fill(0.0);
        }
        for downsampling in 0..SUB_SAMPLING {
            let mut subsampled = [0.0; BAND];
            for (k, sample) in subsampled.iter_mut().enumerate() {
                *sample = input[(SUB_SAMPLING - 1) - downsampling + SUB_SAMPLING * k];
            }
            for in_shift in 0..STRIDE {
                let Some(filter) = filter_index(downsampling + in_shift * SUB_SAMPLING) else {
                    continue;
                };
                let mut filtered = [0.0; BAND];
                filter_core(
                    &FILTER_COEFFS[filter],
                    &subsampled,
                    in_shift,
                    &mut filtered,
                    &mut self.analysis[filter],
                );
                for (band, modulation) in out.iter_mut().zip(DCT_MODULATION[filter]) {
                    for (sample, value) in band.iter_mut().zip(filtered) {
                        *sample += modulation * value;
                    }
                }
            }
        }
    }

    pub(crate) fn synthesis(&mut self, input: &[[f32; BAND]; BANDS], out: &mut [f32; FRAME]) {
        out.fill(0.0);
        for upsampling in 0..SUB_SAMPLING {
            for in_shift in 0..STRIDE {
                let Some(filter) = filter_index(upsampling + in_shift * SUB_SAMPLING) else {
                    continue;
                };
                let mut modulated = [0.0; BAND];
                for (band, modulation) in input.iter().zip(DCT_MODULATION[filter]) {
                    for (sample, value) in modulated.iter_mut().zip(band) {
                        *sample += modulation * value;
                    }
                }
                let mut filtered = [0.0; BAND];
                filter_core(
                    &FILTER_COEFFS[filter],
                    &modulated,
                    in_shift,
                    &mut filtered,
                    &mut self.synthesis[filter],
                );
                for (k, value) in filtered.iter().enumerate() {
                    out[upsampling + SUB_SAMPLING * k] += SUB_SAMPLING as f32 * value;
                }
            }
        }
    }
}

fn filter_index(index: usize) -> Option<usize> {
    match index {
        ZERO_FILTER_1 | ZERO_FILTER_2 => None,
        index if index < ZERO_FILTER_1 => Some(index),
        index if index < ZERO_FILTER_2 => Some(index - 1),
        index => Some(index - 2),
    }
}

fn filter_core(
    filter: &[f32; FILTER_SIZE],
    input: &[f32; BAND],
    in_shift: usize,
    out: &mut [f32; BAND],
    state: &mut [f32; MEMORY],
) {
    out.fill(0.0);
    for (k, sample) in out.iter_mut().enumerate().take(in_shift) {
        let mut j = MEMORY + k - in_shift;
        for tap in filter {
            *sample += state[j] * tap;
            j = j.wrapping_sub(STRIDE);
        }
    }
    for (shift, k) in (in_shift..FILTER_SIZE * STRIDE).enumerate() {
        let limit = FILTER_SIZE.min(1 + (shift >> STRIDE_LOG2));
        let mut sum = 0.0;
        for (i, tap) in filter.iter().enumerate().take(limit) {
            sum += input[shift - i * STRIDE] * tap;
        }
        for (i, tap) in filter.iter().enumerate().skip(limit) {
            sum += state[MEMORY + shift - i * STRIDE] * tap;
        }
        out[k] += sum;
    }
    for (offset, k) in (FILTER_SIZE * STRIDE..BAND).enumerate() {
        let shift = FILTER_SIZE * STRIDE - in_shift + offset;
        for (i, tap) in filter.iter().enumerate() {
            out[k] += input[shift - i * STRIDE] * tap;
        }
    }
    state.copy_from_slice(&input[BAND - MEMORY..]);
}
