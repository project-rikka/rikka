use std::cmp::Ordering;

use crate::parse::bytes::Bytes;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;
use crate::sfnt::tag::Tag;
use crate::variations::item_variation_store::{DeltaSetIndex, ItemVariationStore};

#[derive(Clone, Copy, Debug)]
pub struct Mvar<'a> {
    store: Option<ItemVariationStore<'a>>,
    records: &'a [u8],
    record_size: usize,
}

impl Mvar<'_> {
    pub const HORIZONTAL_ASCENDER: Tag = Tag::new(b"hasc");
    pub const HORIZONTAL_DESCENDER: Tag = Tag::new(b"hdsc");
    pub const HORIZONTAL_LINE_GAP: Tag = Tag::new(b"hlgp");
    pub const HORIZONTAL_CLIPPING_ASCENT: Tag = Tag::new(b"hcla");
    pub const HORIZONTAL_CLIPPING_DESCENT: Tag = Tag::new(b"hcld");
    pub const VERTICAL_ASCENDER: Tag = Tag::new(b"vasc");
    pub const VERTICAL_DESCENDER: Tag = Tag::new(b"vdsc");
    pub const VERTICAL_LINE_GAP: Tag = Tag::new(b"vlgp");
    pub const HORIZONTAL_CARET_RISE: Tag = Tag::new(b"hcrs");
    pub const HORIZONTAL_CARET_RUN: Tag = Tag::new(b"hcrn");
    pub const HORIZONTAL_CARET_OFFSET: Tag = Tag::new(b"hcof");
    pub const X_HEIGHT: Tag = Tag::new(b"xhgt");
    pub const CAP_HEIGHT: Tag = Tag::new(b"cpht");
    pub const SUBSCRIPT_X_SIZE: Tag = Tag::new(b"sbxs");
    pub const SUBSCRIPT_Y_SIZE: Tag = Tag::new(b"sbys");
    pub const SUBSCRIPT_X_OFFSET: Tag = Tag::new(b"sbxo");
    pub const SUBSCRIPT_Y_OFFSET: Tag = Tag::new(b"sbyo");
    pub const SUPERSCRIPT_X_SIZE: Tag = Tag::new(b"spxs");
    pub const SUPERSCRIPT_Y_SIZE: Tag = Tag::new(b"spys");
    pub const SUPERSCRIPT_X_OFFSET: Tag = Tag::new(b"spxo");
    pub const SUPERSCRIPT_Y_OFFSET: Tag = Tag::new(b"spyo");
    pub const STRIKEOUT_SIZE: Tag = Tag::new(b"strs");
    pub const STRIKEOUT_OFFSET: Tag = Tag::new(b"stro");
    pub const UNDERLINE_SIZE: Tag = Tag::new(b"unds");
    pub const UNDERLINE_OFFSET: Tag = Tag::new(b"undo");
}

impl<'a> Mvar<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let record_size = usize::from(data.u16_at(6)?);
        let count = usize::from(data.u16_at(8)?);
        if record_size < 8 {
            return None;
        }
        Some(Self {
            store: data.offset16(0, 10).and_then(ItemVariationStore::new),
            records: data.range(12, count * record_size)?,
            record_size,
        })
    }

    pub fn delta(&self, tag: Tag, coords: &[F2Dot14]) -> f32 {
        let Some(store) = self.store else {
            return 0.0;
        };
        let mut low = 0;
        let mut high = self.records.len() / self.record_size;
        while low < high {
            let middle = low + (high - low) / 2;
            let record = middle * self.record_size;
            let Some(record_tag) = self.records.tag_at(record) else {
                return 0.0;
            };
            match record_tag.cmp(&tag) {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => {
                    let (Some(outer), Some(inner)) = (
                        self.records.u16_at(record + 4),
                        self.records.u16_at(record + 6),
                    ) else {
                        return 0.0;
                    };
                    return store.delta(DeltaSetIndex { outer, inner }, coords);
                }
            }
        }
        0.0
    }
}

impl<'a> Font<'a> {
    pub fn mvar(&self) -> Option<Mvar<'a>> {
        Mvar::new(self.table_data(TableId::Mvar)?)
    }
}
