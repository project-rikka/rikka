#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Falloff {
    #[default]
    Smooth,
    Sphere,
    Root,
    Sharp,
    Linear,
    InverseSquare,
}

impl Falloff {
    #[must_use]
    pub fn apply(self, value: f32) -> f32 {
        match self {
            Falloff::Smooth => 3.0 * value * value - 2.0 * value * value * value,
            Falloff::Sphere => (2.0 * value - value * value).sqrt(),
            Falloff::Root => value.sqrt(),
            Falloff::Sharp => value * value,
            Falloff::Linear => value,
            Falloff::InverseSquare => value * (2.0 - value),
        }
    }
}
