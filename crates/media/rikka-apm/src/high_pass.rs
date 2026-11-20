use rikka_apm_audio::FRAME;

const COEFFICIENTS_B: [f32; 3] = [0.99079, -1.98157, 0.99079];
const COEFFICIENTS_A: [f32; 2] = [-1.98149, 0.98166];

#[derive(Clone, Debug, Default)]
pub(crate) struct HighPass {
    x: [f32; 2],
    y: [f32; 2],
}

impl HighPass {
    pub(crate) fn process(&mut self, samples: &mut [f32; FRAME]) {
        let [b0, b1, b2] = COEFFICIENTS_B;
        let [a0, a1] = COEFFICIENTS_A;
        let [mut x0, mut x1] = self.x;
        let [mut y0, mut y1] = self.y;
        for sample in samples.iter_mut() {
            let input = *sample;
            let output = b0 * input + b1 * x0 + b2 * x1 - a0 * y0 - a1 * y1;
            x1 = x0;
            x0 = input;
            y1 = y0;
            y0 = output;
            *sample = output;
        }
        self.x = [x0, x1];
        self.y = [y0, y1];
    }
}
