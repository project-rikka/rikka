use crate::layout::coverage::Coverage;
use crate::layout::gpos::mark_array::{AnchorMatrix, MarkArray};
use crate::layout::gpos::mark_attachment::MarkAttachment;
use crate::layout::offset_list::OffsetList;

#[derive(Clone, Copy, Debug)]
pub struct MarkLigPos<'a> {
    attachment: MarkAttachment<'a>,
    ligatures: OffsetList<'a>,
}

impl<'a> MarkLigPos<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let attachment = MarkAttachment::new(data)?;
        Some(Self {
            ligatures: OffsetList::new(attachment.targets, 0)?,
            attachment,
        })
    }

    #[inline]
    pub fn mark_coverage(&self) -> Coverage<'a> {
        self.attachment.mark_coverage()
    }

    #[inline]
    pub fn ligature_coverage(&self) -> Coverage<'a> {
        self.attachment.target_coverage()
    }

    #[inline]
    pub fn mark_class_count(&self) -> u16 {
        self.attachment.class_count
    }

    #[inline]
    pub fn marks(&self) -> MarkArray<'a> {
        self.attachment.marks
    }

    #[inline]
    pub fn ligature_count(&self) -> usize {
        self.ligatures.len()
    }

    #[inline]
    pub fn ligature(&self, coverage_index: u16) -> Option<AnchorMatrix<'a>> {
        AnchorMatrix::new(
            self.ligatures.get(usize::from(coverage_index))?,
            self.attachment.class_count,
        )
    }
}
