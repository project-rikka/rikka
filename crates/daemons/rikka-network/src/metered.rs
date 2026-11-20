#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Metered {
    #[default]
    Automatic,
    Yes,
    No,
}

impl Metered {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            1 | 3 => Self::Yes,
            2 | 4 => Self::No,
            _ => Self::Automatic,
        }
    }

    pub(crate) fn to_code(self) -> u32 {
        match self {
            Self::Automatic => 0,
            Self::Yes => 1,
            Self::No => 2,
        }
    }
}
