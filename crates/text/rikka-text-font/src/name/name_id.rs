#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NameId(pub u16);

impl NameId {
    pub const COPYRIGHT: Self = Self(0);
    pub const FAMILY: Self = Self(1);
    pub const SUBFAMILY: Self = Self(2);
    pub const UNIQUE_ID: Self = Self(3);
    pub const FULL_NAME: Self = Self(4);
    pub const VERSION: Self = Self(5);
    pub const POSTSCRIPT_NAME: Self = Self(6);
    pub const TRADEMARK: Self = Self(7);
    pub const MANUFACTURER: Self = Self(8);
    pub const DESIGNER: Self = Self(9);
    pub const DESCRIPTION: Self = Self(10);
    pub const LICENSE: Self = Self(13);
    pub const TYPOGRAPHIC_FAMILY: Self = Self(16);
    pub const TYPOGRAPHIC_SUBFAMILY: Self = Self(17);
    pub const SAMPLE_TEXT: Self = Self(19);
    pub const WWS_FAMILY: Self = Self(21);
    pub const WWS_SUBFAMILY: Self = Self(22);
    pub const VARIATIONS_POSTSCRIPT_PREFIX: Self = Self(25);
}
