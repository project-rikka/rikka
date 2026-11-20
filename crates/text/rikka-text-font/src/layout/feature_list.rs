use crate::layout::tagged_records::TaggedRecords;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::tag::Tag;

#[derive(Clone, Copy, Debug)]
pub struct FeatureList<'a> {
    records: TaggedRecords<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct Feature<'a> {
    tag: Tag,
    data: &'a [u8],
    lookup_indices: Array<'a, u16>,
}

impl<'a> FeatureList<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            records: TaggedRecords::new(data, 0)?,
        })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.len() == 0
    }

    #[inline]
    pub fn tag(&self, index: u16) -> Option<Tag> {
        self.records.tag(usize::from(index))
    }

    pub fn get(&self, index: u16) -> Option<Feature<'a>> {
        let index = usize::from(index);
        Feature::new(self.records.tag(index)?, self.records.data(index)?)
    }

    pub fn iter(&self) -> impl Iterator<Item = Feature<'a>> + use<'a> {
        let list = *self;
        (0..u16::try_from(self.len()).unwrap_or(u16::MAX)).filter_map(move |index| list.get(index))
    }
}

impl<'a> Feature<'a> {
    pub(crate) fn new(tag: Tag, data: &'a [u8]) -> Option<Self> {
        let count = usize::from(data.u16_at(2)?);
        Some(Self {
            tag,
            data,
            lookup_indices: data.array(4, count)?,
        })
    }

    #[inline]
    pub fn tag(&self) -> Tag {
        self.tag
    }

    #[inline]
    pub fn lookup_indices(&self) -> Array<'a, u16> {
        self.lookup_indices
    }

    pub fn params(&self) -> Option<&'a [u8]> {
        self.data.offset16(0, 0)
    }
}
