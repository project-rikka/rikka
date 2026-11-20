use crate::layout::coverage::Coverage;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;

#[derive(Clone, Copy, Debug)]
pub struct CoverageArray<'a> {
    base: &'a [u8],
    offsets: Array<'a, u16>,
}

impl<'a> CoverageArray<'a> {
    #[inline]
    pub(crate) fn new(base: &'a [u8], offsets: Array<'a, u16>) -> Self {
        Self { base, offsets }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    #[inline]
    pub fn get(&self, index: usize) -> Option<Coverage<'a>> {
        Coverage::new(self.base.tail(usize::from(self.offsets.get(index)?))?)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = Option<Coverage<'a>>> + use<'a> {
        let array = *self;
        (0..self.len()).map(move |index| array.get(index))
    }
}
