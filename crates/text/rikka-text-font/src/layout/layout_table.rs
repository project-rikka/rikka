use std::ops::Deref;

use crate::layout::feature_list::{Feature, FeatureList};
use crate::layout::feature_variations::FeatureVariations;
use crate::layout::lookup::{PosLookup, SubstLookup};
use crate::layout::offset_list::OffsetList;
use crate::layout::script_list::ScriptList;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::table_id::TableId;

static EMPTY_LIST: [u8; 2] = [0, 0];

#[derive(Clone, Copy, Debug)]
pub struct LayoutTable<'a> {
    data: &'a [u8],
    lookup_list: usize,
    scripts: ScriptList<'a>,
    features: FeatureList<'a>,
    lookups: OffsetList<'a>,
    feature_variations: Option<FeatureVariations<'a>>,
}

#[derive(Clone, Copy, Debug)]
pub struct Gsub<'a> {
    table: LayoutTable<'a>,
}

#[derive(Clone, Copy, Debug)]
pub struct Gpos<'a> {
    table: LayoutTable<'a>,
}

impl<'a> LayoutTable<'a> {
    fn new(data: &'a [u8]) -> Option<Self> {
        let major = data.u16_at(0)?;
        let minor = data.u16_at(2)?;
        if major != 1 {
            return None;
        }
        let list = |at: usize| data.offset16(0, at).unwrap_or(&EMPTY_LIST);
        let feature_variations = if minor >= 1 {
            data.offset32(0, 10).and_then(FeatureVariations::new)
        } else {
            None
        };
        Some(Self {
            data,
            lookup_list: usize::from(data.u16_at(8).unwrap_or(0)),
            scripts: ScriptList::new(list(4)).or_else(|| ScriptList::new(&EMPTY_LIST))?,
            features: FeatureList::new(list(6)).or_else(|| FeatureList::new(&EMPTY_LIST))?,
            lookups: OffsetList::new(list(8), 0).or_else(|| OffsetList::new(&EMPTY_LIST, 0))?,
            feature_variations,
        })
    }

    #[inline]
    pub fn scripts(&self) -> ScriptList<'a> {
        self.scripts
    }

    #[inline]
    pub fn features(&self) -> FeatureList<'a> {
        self.features
    }

    #[inline]
    pub fn feature_variations(&self) -> Option<FeatureVariations<'a>> {
        self.feature_variations
    }

    pub fn feature(&self, index: u16, variations_index: Option<u32>) -> Option<Feature<'a>> {
        if let Some(record) = variations_index
            && let Some(variations) = self.feature_variations
            && let Some(substitution) = variations.substitution(record)
            && let Some(feature) = substitution.find(index, &self.features)
        {
            return Some(feature);
        }
        self.features.get(index)
    }

    #[inline]
    pub fn lookup_count(&self) -> usize {
        self.lookups.len()
    }

    #[inline]
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    #[inline]
    fn lookup_offset(&self, index: u16) -> Option<(&'a [u8], usize)> {
        let offset = self.lookups.offset(usize::from(index))?;
        Some((self.data, self.lookup_list.checked_add(offset)?))
    }
}

impl<'a> Gsub<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            table: LayoutTable::new(data)?,
        })
    }

    #[inline]
    pub fn lookup(&self, index: u16) -> Option<SubstLookup<'a>> {
        let (table, offset) = self.table.lookup_offset(index)?;
        SubstLookup::new(table, offset)
    }
}

impl<'a> Gpos<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        Some(Self {
            table: LayoutTable::new(data)?,
        })
    }

    #[inline]
    pub fn lookup(&self, index: u16) -> Option<PosLookup<'a>> {
        let (table, offset) = self.table.lookup_offset(index)?;
        PosLookup::new(table, offset)
    }
}

impl<'a> Deref for Gsub<'a> {
    type Target = LayoutTable<'a>;

    fn deref(&self) -> &LayoutTable<'a> {
        &self.table
    }
}

impl<'a> Deref for Gpos<'a> {
    type Target = LayoutTable<'a>;

    fn deref(&self) -> &LayoutTable<'a> {
        &self.table
    }
}

impl<'a> Font<'a> {
    pub fn gsub(&self) -> Option<Gsub<'a>> {
        Gsub::new(self.table_data(TableId::Gsub)?)
    }

    pub fn gpos(&self) -> Option<Gpos<'a>> {
        Gpos::new(self.table_data(TableId::Gpos)?)
    }
}
