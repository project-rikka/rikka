use crate::layout::tagged_records::TaggedRecords;
use crate::parse::array::Array;
use crate::parse::bytes::Bytes;
use crate::sfnt::tag::Tag;

#[derive(Clone, Copy, Debug)]
pub struct ScriptList<'a> {
    records: TaggedRecords<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct Script<'a> {
    data: &'a [u8],
    records: TaggedRecords<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct LangSys<'a> {
    required_feature_index: u16,
    feature_indices: Array<'a, u16>,
}

impl<'a> ScriptList<'a> {
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
    pub fn tag(&self, index: usize) -> Option<Tag> {
        self.records.tag(index)
    }

    pub fn get(&self, index: usize) -> Option<Script<'a>> {
        Script::new(self.records.data(index)?)
    }

    pub fn index_of(&self, tag: Tag) -> Option<usize> {
        self.records.find(tag)
    }

    pub fn find(&self, tag: Tag) -> Option<Script<'a>> {
        self.get(self.index_of(tag)?)
    }

    pub fn tags(&self) -> impl Iterator<Item = Tag> + use<'a> {
        let records = self.records;
        (0..records.len()).filter_map(move |index| records.tag(index))
    }
}

impl<'a> Script<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            data,
            records: TaggedRecords::new(data, 2)?,
        })
    }

    pub fn default_lang_sys(&self) -> Option<LangSys<'a>> {
        LangSys::new(self.data.offset16(0, 0)?)
    }

    #[inline]
    pub fn lang_sys_count(&self) -> usize {
        self.records.len()
    }

    #[inline]
    pub fn lang_sys_tag(&self, index: usize) -> Option<Tag> {
        self.records.tag(index)
    }

    pub fn lang_sys_at(&self, index: usize) -> Option<LangSys<'a>> {
        LangSys::new(self.records.data(index)?)
    }

    pub fn lang_sys_index(&self, tag: Tag) -> Option<usize> {
        self.records.find(tag)
    }

    pub fn lang_sys(&self, tag: Tag) -> Option<LangSys<'a>> {
        self.lang_sys_at(self.lang_sys_index(tag)?)
    }
}

impl<'a> LangSys<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let required_feature_index = data.u16_at(2)?;
        let count = usize::from(data.u16_at(4)?);
        Some(Self {
            required_feature_index,
            feature_indices: data.array(6, count)?,
        })
    }

    #[inline]
    pub fn required_feature_index(&self) -> Option<u16> {
        (self.required_feature_index != 0xFFFF).then_some(self.required_feature_index)
    }

    #[inline]
    pub fn feature_indices(&self) -> Array<'a, u16> {
        self.feature_indices
    }
}
