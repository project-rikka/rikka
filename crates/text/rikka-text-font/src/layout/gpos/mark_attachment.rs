use crate::layout::coverage::Coverage;
use crate::layout::gpos::mark_array::MarkArray;
use crate::parse::bytes::Bytes;

#[derive(Clone, Copy, Debug)]
pub(crate) struct MarkAttachment<'a> {
    data: &'a [u8],
    pub(crate) class_count: u16,
    pub(crate) marks: MarkArray<'a>,
    pub(crate) targets: &'a [u8],
}

impl<'a> MarkAttachment<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        Some(Self {
            data,
            class_count: data.u16_at(6)?,
            marks: MarkArray::new(data.offset16(0, 8)?)?,
            targets: data.offset16(0, 10)?,
        })
    }

    #[inline]
    pub(crate) fn mark_coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 2)
    }

    #[inline]
    pub(crate) fn target_coverage(&self) -> Coverage<'a> {
        Coverage::from_offset(self.data, 4)
    }
}
