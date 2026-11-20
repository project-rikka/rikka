use crate::value::EmitterValue;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueKind {
    Amount { min: f32, max: f32 },
    Angle { min: f32, max: f32 },
    Position,
    Color,
}

impl ValueKind {
    #[must_use]
    pub fn fits(self, value: EmitterValue) -> bool {
        match (self, value) {
            (
                Self::Amount { min, max } | Self::Angle { min, max },
                EmitterValue::Number(number),
            ) => (min..=max).contains(&number),
            (Self::Position, EmitterValue::Three(three)) => {
                three.iter().all(|axis| axis.is_finite())
            }
            (Self::Color, EmitterValue::Three(three)) => {
                three.iter().all(|channel| (0.0..=1.0).contains(channel))
            }
            _ => false,
        }
    }
}

pub(crate) const fn amount(min: f32, max: f32) -> ValueKind {
    ValueKind::Amount { min, max }
}

pub(crate) const fn angle(min: f32, max: f32) -> ValueKind {
    ValueKind::Angle { min, max }
}
