#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Loops {
    #[default]
    Infinite,
    Finite(u32),
}

impl Loops {
    #[must_use]
    pub const fn is_infinite(self) -> bool {
        matches!(self, Loops::Infinite)
    }

    #[must_use]
    pub const fn count(self) -> Option<u32> {
        match self {
            Loops::Infinite => None,
            Loops::Finite(count) => Some(count),
        }
    }
}
