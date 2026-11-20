use crate::parse::bytes::Bytes;
use crate::sfnt::tag::Tag;

const RECORD_SIZE: usize = 6;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TaggedRecords<'a> {
    base: &'a [u8],
    records: &'a [u8],
}

impl<'a> TaggedRecords<'a> {
    pub(crate) fn new(base: &'a [u8], count_at: usize) -> Option<Self> {
        let count = usize::from(base.u16_at(count_at)?);
        let records = base.range(count_at + 2, count * RECORD_SIZE)?;
        Some(Self { base, records })
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.records.len() / RECORD_SIZE
    }

    #[inline]
    pub(crate) fn tag(&self, index: usize) -> Option<Tag> {
        self.records.tag_at(index.checked_mul(RECORD_SIZE)?)
    }

    #[inline]
    pub(crate) fn data(&self, index: usize) -> Option<&'a [u8]> {
        let offset = self.records.u16_at(index.checked_mul(RECORD_SIZE)? + 4)?;
        self.base.tail(usize::from(offset))
    }

    pub(crate) fn find(&self, tag: Tag) -> Option<usize> {
        let mut low = 0;
        let mut high = self.len();
        while low < high {
            let middle = low + (high - low) / 2;
            match self.tag(middle)?.cmp(&tag) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Some(middle),
            }
        }
        (0..self.len()).find(|&index| self.tag(index) == Some(tag))
    }
}
