#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sources(u32);

impl Sources {
    pub const MONITOR: Sources = Sources(1);
    pub const WINDOW: Sources = Sources(2);
    pub const ANY: Sources = Sources(1 | 2);

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, other: Sources) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Sources {
    type Output = Sources;

    fn bitor(self, other: Sources) -> Sources {
        Sources(self.0 | other.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SourceKind {
    Monitor,
    Window,
    Virtual,
}

impl SourceKind {
    pub(crate) fn from_bits(bits: u32) -> SourceKind {
        match bits {
            2 => SourceKind::Window,
            4 => SourceKind::Virtual,
            _ => SourceKind::Monitor,
        }
    }
}
