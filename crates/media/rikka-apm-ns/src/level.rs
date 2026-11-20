#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Level {
    Low,
    #[default]
    Moderate,
    High,
    VeryHigh,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SuppressionParams {
    pub(crate) over_subtraction_factor: f32,
    pub(crate) minimum_attenuating_gain: f32,
    pub(crate) use_attenuation_adjustment: bool,
}

impl SuppressionParams {
    pub(crate) fn for_level(level: Level) -> SuppressionParams {
        let (over_subtraction_factor, minimum_attenuating_gain, use_attenuation_adjustment) =
            match level {
                Level::Low => (1.0, 0.5, false),
                Level::Moderate => (1.0, 0.25, true),
                Level::High => (1.1, 0.125, true),
                Level::VeryHigh => (1.25, 0.09, true),
            };
        SuppressionParams {
            over_subtraction_factor,
            minimum_attenuating_gain,
            use_attenuation_adjustment,
        }
    }
}
