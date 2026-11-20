#[derive(Clone, Copy, Debug)]
pub(crate) struct BiQuadParam {
    pub(crate) zero: (f32, f32),
    pub(crate) pole: (f32, f32),
    pub(crate) gain: f32,
    pub(crate) mirror_zero_along_i_axis: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct BiQuadCoefficients {
    pub(crate) b: [f32; 3],
    pub(crate) a: [f32; 2],
}

#[derive(Clone, Debug)]
struct BiQuad {
    coefficients: BiQuadCoefficients,
    x: [f32; 2],
    y: [f32; 2],
}

impl BiQuad {
    fn new(coefficients: BiQuadCoefficients) -> BiQuad {
        BiQuad {
            coefficients,
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn from_param(param: &BiQuadParam) -> BiQuad {
        let (z_r, z_i) = param.zero;
        let (p_r, p_i) = param.pole;
        let gain = param.gain;
        let b = if param.mirror_zero_along_i_axis {
            [gain, 0.0, gain * -(z_r * z_r)]
        } else {
            [gain, gain * -2.0 * z_r, gain * (z_r * z_r + z_i * z_i)]
        };
        BiQuad::new(BiQuadCoefficients {
            b,
            a: [-2.0 * p_r, p_r * p_r + p_i * p_i],
        })
    }

    fn apply(&mut self, data: &mut [f32]) {
        let [b0, b1, b2] = self.coefficients.b;
        let [a0, a1] = self.coefficients.a;
        let [mut x0, mut x1] = self.x;
        let [mut y0, mut y1] = self.y;
        for sample in data {
            let input = *sample;
            *sample = b0 * input + b1 * x0 + b2 * x1 - a0 * y0 - a1 * y1;
            x1 = x0;
            x0 = input;
            y1 = y0;
            y0 = *sample;
        }
        self.x = [x0, x1];
        self.y = [y0, y1];
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CascadedBiQuad {
    biquads: Vec<BiQuad>,
}

impl CascadedBiQuad {
    pub(crate) fn from_params(params: &[BiQuadParam]) -> CascadedBiQuad {
        CascadedBiQuad {
            biquads: params.iter().map(BiQuad::from_param).collect(),
        }
    }

    pub(crate) fn from_coefficients(
        coefficients: BiQuadCoefficients,
        count: usize,
    ) -> CascadedBiQuad {
        CascadedBiQuad {
            biquads: vec![BiQuad::new(coefficients); count],
        }
    }

    pub(crate) fn process(&mut self, input: &[f32], output: &mut [f32]) {
        output.copy_from_slice(input);
        self.process_in_place(output);
    }

    pub(crate) fn process_in_place(&mut self, data: &mut [f32]) {
        for biquad in &mut self.biquads {
            biquad.apply(data);
        }
    }
}
