bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct Anchor: u8 {
        const TOP = 1;
        const BOTTOM = 1 << 1;
        const LEFT = 1 << 2;
        const RIGHT = 1 << 3;
    }
}

impl Default for Anchor {
    fn default() -> Self {
        Self::empty()
    }
}
