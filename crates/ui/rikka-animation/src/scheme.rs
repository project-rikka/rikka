use crate::spring::Spring;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionScheme {
    Expressive,
    Standard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Property {
    Spatial,
    Effects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Speed {
    Fast,
    Default,
    Slow,
}

impl MotionScheme {
    pub const fn spring(self, property: Property, speed: Speed) -> Spring {
        match (self, property, speed) {
            (Self::Expressive, Property::Spatial, Speed::Fast) => Spring::FAST_SPATIAL,
            (Self::Expressive, Property::Spatial, Speed::Default) => Spring::DEFAULT_SPATIAL,
            (Self::Expressive, Property::Spatial, Speed::Slow) => Spring::SLOW_SPATIAL,
            (Self::Standard, Property::Spatial, Speed::Fast) => Spring::new(0.9, 1400.0),
            (Self::Standard, Property::Spatial, Speed::Default) => Spring::new(0.9, 700.0),
            (Self::Standard, Property::Spatial, Speed::Slow) => Spring::new(0.9, 300.0),
            (_, Property::Effects, Speed::Fast) => Spring::FAST_EFFECTS,
            (_, Property::Effects, Speed::Default) => Spring::DEFAULT_EFFECTS,
            (_, Property::Effects, Speed::Slow) => Spring::SLOW_EFFECTS,
        }
    }
}
