pub(crate) mod attach_kind {
    pub(crate) const MARK: u8 = 1;
    pub(crate) const CURSIVE: u8 = 2;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlyphPosition {
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub(crate) attach_chain: i16,
    pub(crate) attach_kind: u8,
}
