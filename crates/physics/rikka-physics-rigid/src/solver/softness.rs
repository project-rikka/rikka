use std::f32::consts::TAU;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Softness {
    pub bias_rate: f32,
    pub mass_scale: f32,
    pub impulse_scale: f32,
}

impl Softness {
    pub(crate) fn new(hertz: f32, damping_ratio: f32, h: f32) -> Self {
        if hertz <= 0.0 {
            return Self {
                bias_rate: 0.0,
                mass_scale: 0.0,
                impulse_scale: 0.0,
            };
        }
        let omega = TAU * hertz;
        let a1 = 2.0 * damping_ratio + h * omega;
        let a2 = h * omega * a1;
        let a3 = 1.0 / (1.0 + a2);
        Self {
            bias_rate: omega / a1,
            mass_scale: a2 * a3,
            impulse_scale: a3,
        }
    }
}
