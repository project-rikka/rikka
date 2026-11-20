#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LookupFlag(pub u16);

impl LookupFlag {
    pub const RIGHT_TO_LEFT: u16 = 0x0001;
    pub const IGNORE_BASE_GLYPHS: u16 = 0x0002;
    pub const IGNORE_LIGATURES: u16 = 0x0004;
    pub const IGNORE_MARKS: u16 = 0x0008;
    pub const USE_MARK_FILTERING_SET: u16 = 0x0010;
    pub const MARK_ATTACHMENT_TYPE: u16 = 0xFF00;

    #[inline]
    pub fn use_mark_filtering_set(self) -> bool {
        self.0 & Self::USE_MARK_FILTERING_SET != 0
    }
}
