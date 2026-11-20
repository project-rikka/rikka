#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Easy,
    Detail,
}

impl Mode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Easy => Self::Detail,
            Self::Detail => Self::Easy,
        }
    }
}
