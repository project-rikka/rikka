use std::fmt;

use crate::error::{Error, Result};
use crate::parse::bytes::Bytes;
use crate::sfnt::table_id::TableId;
use crate::sfnt::tag::Tag;
use crate::tables::head::Head;
use crate::tables::maxp::Maxp;

const TABLE_RECORD_SIZE: usize = 16;
const ABSENT: u16 = u16::MAX;

#[derive(Clone, Copy)]
pub struct Font<'a> {
    data: &'a [u8],
    records: &'a [u8],
    slots: [u16; TableId::COUNT],
    face_index: u32,
    units_per_em: u16,
    glyph_count: u16,
}

impl<'a> Font<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self> {
        crate::sfnt::file::FontFile::new(data)?.face(0)
    }

    pub(crate) fn from_directory(data: &'a [u8], offset: usize, face_index: u32) -> Result<Self> {
        let table_count = data.u16_at(offset + 4).ok_or(Error::Malformed)?;
        let records = data
            .range(offset + 12, usize::from(table_count) * TABLE_RECORD_SIZE)
            .ok_or(Error::Malformed)?;
        let mut slots = [ABSENT; TableId::COUNT];
        for index in 0..table_count {
            let Some(tag) = records.tag_at(usize::from(index) * TABLE_RECORD_SIZE) else {
                break;
            };
            if let Some(id) = TableId::from_tag(tag) {
                let slot = &mut slots[id as usize];
                if *slot == ABSENT {
                    *slot = index;
                }
            }
        }
        let mut font = Self {
            data,
            records,
            slots,
            face_index,
            units_per_em: 0,
            glyph_count: 0,
        };
        let head = font
            .table_data(TableId::Head)
            .ok_or(Error::MissingTable(TableId::Head.tag()))?;
        let maxp = font
            .table_data(TableId::Maxp)
            .ok_or(Error::MissingTable(TableId::Maxp.tag()))?;
        font.units_per_em = Head::new(head)
            .map(|head| head.units_per_em())
            .filter(|&units| units != 0)
            .ok_or(Error::Malformed)?;
        font.glyph_count = Maxp::new(maxp).ok_or(Error::Malformed)?.glyph_count();
        Ok(font)
    }

    #[inline]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    pub fn face_index(&self) -> u32 {
        self.face_index
    }

    #[inline]
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }

    #[inline]
    pub fn glyph_count(&self) -> u16 {
        self.glyph_count
    }

    #[inline]
    pub(crate) fn table_data(&self, id: TableId) -> Option<&'a [u8]> {
        let slot = self.slots[id as usize];
        if slot == ABSENT {
            return None;
        }
        self.record_data(usize::from(slot))
    }

    #[inline]
    pub(crate) fn has(&self, id: TableId) -> bool {
        self.slots[id as usize] != ABSENT
    }

    fn record_data(&self, index: usize) -> Option<&'a [u8]> {
        let record = index * TABLE_RECORD_SIZE;
        let offset = self.records.u32_at(record + 8)?;
        let length = self.records.u32_at(record + 12)?;
        self.data.range(offset as usize, length as usize)
    }

    pub fn table(&self, tag: Tag) -> Option<&'a [u8]> {
        if let Some(id) = TableId::from_tag(tag) {
            return self.table_data(id);
        }
        (0..self.records.len() / TABLE_RECORD_SIZE)
            .find(|&index| self.records.tag_at(index * TABLE_RECORD_SIZE) == Some(tag))
            .and_then(|index| self.record_data(index))
    }

    pub fn has_table(&self, tag: Tag) -> bool {
        self.table(tag).is_some()
    }

    pub fn table_tags(&self) -> impl Iterator<Item = Tag> + use<'a> {
        let records = self.records;
        (0..records.len() / TABLE_RECORD_SIZE)
            .filter_map(move |index| records.tag_at(index * TABLE_RECORD_SIZE))
    }
}

impl fmt::Debug for Font<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Font")
            .field("face_index", &self.face_index)
            .field("units_per_em", &self.units_per_em)
            .field("glyph_count", &self.glyph_count)
            .finish_non_exhaustive()
    }
}
