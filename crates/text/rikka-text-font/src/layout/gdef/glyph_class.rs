#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GlyphClass {
    #[default]
    Unclassified,
    Base,
    Ligature,
    Mark,
    Component,
}

impl GlyphClass {
    #[inline]
    pub(crate) fn from_raw(class: u16) -> Self {
        match class {
            1 => Self::Base,
            2 => Self::Ligature,
            3 => Self::Mark,
            4 => Self::Component,
            _ => Self::Unclassified,
        }
    }
}
