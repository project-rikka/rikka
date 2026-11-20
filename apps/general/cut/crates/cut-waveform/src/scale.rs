const FLOOR_DB: f32 = -48.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Scale {
    Linear,
    #[default]
    Logarithmic,
}

impl Scale {
    pub const ALL: [Scale; 2] = [Scale::Logarithmic, Scale::Linear];

    #[must_use]
    pub fn level(self, amplitude: f32, gain: f32) -> f32 {
        let linear = amplitude.abs() * gain.max(0.0);
        let level = match self {
            Scale::Linear => linear,
            Scale::Logarithmic if linear <= 0.0 => 0.0,
            Scale::Logarithmic => 1.0 - 20.0 * linear.log10() / FLOOR_DB,
        };
        level.clamp(0.0, 1.0)
    }

    #[must_use]
    pub fn clips(amplitude: f32, gain: f32) -> bool {
        amplitude.abs() * gain > 1.0
    }
}
