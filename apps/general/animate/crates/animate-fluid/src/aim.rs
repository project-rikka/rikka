#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Aim {
    #[default]
    Ballistic,
    Direction,
}

impl Aim {
    pub const ALL: [Aim; 2] = [Self::Ballistic, Self::Direction];

    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::Ballistic => "ballistic",
            Self::Direction => "direction",
        }
    }

    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|aim| aim.id() == id)
    }
}
