use crate::biquad::{BiQuadParam, CascadedBiQuad};
use crate::common::BLOCK_LEN;

const fn param(
    zero: (f32, f32),
    pole: (f32, f32),
    gain: f32,
    mirror_zero_along_i_axis: bool,
) -> BiQuadParam {
    BiQuadParam {
        zero,
        pole,
        gain,
        mirror_zero_along_i_axis,
    }
}

#[expect(
    clippy::excessive_precision,
    reason = "values copied verbatim from WebRTC"
)]
const LOW_PASS_DS2: BiQuadParam = param(
    (-1.0, 0.0),
    (0.138_332_31, 0.407_431_76),
    0.227_117_963_934_864_66_f64 as f32,
    false,
);
#[expect(
    clippy::excessive_precision,
    reason = "values copied verbatim from WebRTC"
)]
const LOW_PASS_DS4: [BiQuadParam; 3] = [
    param(
        (-0.088_738_42, 0.996_054_96),
        (0.759_162_27, 0.238_410_65),
        0.262_506_968_27,
        false,
    ),
    param(
        (0.622_738_32, 0.782_430_18),
        (0.748_921_12, 0.541_015_2),
        0.262_506_968_27,
        false,
    ),
    param(
        (0.711_076_93, 0.703_114_21),
        (0.748_955_34, 0.639_246_16),
        0.262_506_968_27,
        false,
    ),
];
const BAND_PASS_DS8: BiQuadParam = param(
    (1.0, 0.0),
    (0.760_181_5, 0.464_235_42),
    0.103_304_782_665_059_48_f64 as f32,
    true,
);
#[expect(
    clippy::excessive_precision,
    reason = "values copied verbatim from WebRTC"
)]
const HIGH_PASS: BiQuadParam = param(
    (1.0, 0.0),
    (0.727_121_79, 0.212_969_04),
    0.757_076_375_333_884_9_f64 as f32,
    false,
);

#[derive(Clone, Debug)]
pub(crate) struct Decimator {
    down_sampling_factor: usize,
    anti_aliasing: CascadedBiQuad,
    noise_reduction: CascadedBiQuad,
}

impl Decimator {
    pub(crate) fn new(down_sampling_factor: usize) -> Decimator {
        let anti_aliasing = match down_sampling_factor {
            4 => CascadedBiQuad::from_params(&LOW_PASS_DS4),
            8 => CascadedBiQuad::from_params(&[BAND_PASS_DS8; 5]),
            _ => CascadedBiQuad::from_params(&[LOW_PASS_DS2; 3]),
        };
        let noise_reduction = CascadedBiQuad::from_params(if down_sampling_factor == 8 {
            &[]
        } else {
            std::slice::from_ref(&HIGH_PASS)
        });
        Decimator {
            down_sampling_factor,
            anti_aliasing,
            noise_reduction,
        }
    }

    pub(crate) fn decimate(&mut self, input: &[f32; BLOCK_LEN], output: &mut [f32]) {
        let mut filtered = [0.0; BLOCK_LEN];
        self.anti_aliasing.process(input, &mut filtered);
        self.noise_reduction.process_in_place(&mut filtered);
        for (out, sample) in output
            .iter_mut()
            .zip(filtered.iter().step_by(self.down_sampling_factor))
        {
            *out = *sample;
        }
    }
}
