use bitflags::bitflags;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct Tags: u8 {
        const DIALOG = 1;
        const MODAL = 1 << 1;
        const URGENT = 1 << 2;
        const PASSIVE = 1 << 3;
    }
}
