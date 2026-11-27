#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropKind {
    Stage,
    Accessory,
}

impl PropKind {
    #[must_use]
    pub fn attaches(self) -> bool {
        self == Self::Accessory
    }
}
