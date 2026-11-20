use crate::layout::coverage::Coverage;
use crate::layout::gpos::mark_array::{AnchorMatrix, MarkArray};
use crate::layout::gpos::mark_attachment::MarkAttachment;

#[derive(Clone, Copy, Debug)]
pub struct MarkMarkPos<'a> {
    attachment: MarkAttachment<'a>,
    base_marks: AnchorMatrix<'a>,
}

impl<'a> MarkMarkPos<'a> {
    #[inline]
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let attachment = MarkAttachment::new(data)?;
        Some(Self {
            base_marks: AnchorMatrix::new(attachment.targets, attachment.class_count)?,
            attachment,
        })
    }

    #[inline]
    pub fn mark_coverage(&self) -> Coverage<'a> {
        self.attachment.mark_coverage()
    }

    #[inline]
    pub fn base_mark_coverage(&self) -> Coverage<'a> {
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
    pub fn base_marks(&self) -> AnchorMatrix<'a> {
        self.base_marks
    }
}
