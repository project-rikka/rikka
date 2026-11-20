#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EmitterValue {
    Number(f32),
    Three([f32; 3]),
}

impl EmitterValue {
    #[must_use]
    pub fn mix(self, other: Self, t: f32) -> Self {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => Self::Number(a + (b - a) * t),
            (Self::Three(a), Self::Three(b)) => Self::Three(std::array::from_fn(|index| {
                a[index] + (b[index] - a[index]) * t
            })),
            (value, _) => value,
        }
    }

    #[must_use]
    pub fn number(self) -> Option<f32> {
        match self {
            Self::Number(number) => Some(number),
            _ => None,
        }
    }

    #[must_use]
    pub fn three(self) -> Option<[f32; 3]> {
        match self {
            Self::Three(three) => Some(three),
            _ => None,
        }
    }
}
