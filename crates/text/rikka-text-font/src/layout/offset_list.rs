use crate::parse::array::Array;
use crate::parse::bytes::Bytes;

#[derive(Clone, Copy, Debug)]
pub(crate) struct OffsetList<'a> {
    base: &'a [u8],
    offsets: Array<'a, u16>,
}

impl<'a> OffsetList<'a> {
    #[inline]
    pub(crate) fn new(base: &'a [u8], count_at: usize) -> Option<Self> {
        let count = usize::from(base.u16_at(count_at)?);
        Some(Self {
            base,
            offsets: base.array(count_at + 2, count)?,
        })
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.offsets.len()
    }

    #[inline]
    pub(crate) fn offset(&self, index: usize) -> Option<usize> {
        let offset = usize::from(self.offsets.get(index)?);
        (offset != 0 && offset <= self.base.len()).then_some(offset)
    }

    #[inline]
    pub(crate) fn get(&self, index: usize) -> Option<&'a [u8]> {
        let offset = self.offsets.get(index)?;
        if offset == 0 {
            return None;
        }
        self.base.tail(usize::from(offset))
    }
}
