use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WakeHint {
    #[default]
    Idle,
    In(Duration),
}

impl WakeHint {
    #[must_use]
    pub fn soonest(self, other: Self) -> Self {
        match (self, other) {
            (Self::In(left), Self::In(right)) => Self::In(left.min(right)),
            (Self::In(delay), Self::Idle) | (Self::Idle, Self::In(delay)) => Self::In(delay),
            (Self::Idle, Self::Idle) => Self::Idle,
        }
    }

    #[must_use]
    pub fn delay(self) -> Option<Duration> {
        match self {
            Self::Idle => None,
            Self::In(delay) => Some(delay),
        }
    }

    #[must_use]
    pub fn is_idle(self) -> bool {
        self == Self::Idle
    }
}
