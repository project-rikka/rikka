#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubtableLocation {
    offset: u32,
    kind: u16,
}

impl SubtableLocation {
    #[inline]
    pub(crate) fn new(offset: usize, kind: u16) -> Option<Self> {
        Some(Self {
            offset: u32::try_from(offset).ok()?,
            kind,
        })
    }

    #[inline]
    pub fn offset(&self) -> usize {
        self.offset as usize
    }

    #[inline]
    pub fn kind(&self) -> u16 {
        self.kind
    }
}
