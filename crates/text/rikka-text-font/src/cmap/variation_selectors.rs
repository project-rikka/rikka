use std::cmp::Ordering;

use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

const SELECTOR_RECORD_SIZE: usize = 11;
const DEFAULT_RANGE_SIZE: usize = 4;
const MAPPING_SIZE: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariantGlyph {
    UseDefault,
    Glyph(GlyphId),
}

#[derive(Clone, Copy, Debug)]
pub struct VariationSelectors<'a> {
    data: &'a [u8],
    records: &'a [u8],
}

impl<'a> VariationSelectors<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 14 {
            return None;
        }
        let count = data.u32_at(6)? as usize;
        let records = data.range(10, count.checked_mul(SELECTOR_RECORD_SIZE)?)?;
        Some(Self { data, records })
    }

    pub fn selectors(&self) -> impl Iterator<Item = u32> + use<'a> {
        let records = self.records;
        (0..records.len() / SELECTOR_RECORD_SIZE)
            .filter_map(move |index| records.u24_at(index * SELECTOR_RECORD_SIZE))
    }

    pub fn map(&self, codepoint: u32, selector: u32) -> Option<VariantGlyph> {
        let record = self.find_selector(selector)?;
        if let Some(defaults) = self.data.offset32(0, 10 + record + 3) {
            let count = defaults.u32_at(0)? as usize;
            let ranges = defaults.range(4, count.checked_mul(DEFAULT_RANGE_SIZE)?)?;
            let found = search(ranges, DEFAULT_RANGE_SIZE, |at| {
                let start = ranges.u24_at(at).unwrap_or(u32::MAX);
                let additional = u32::from(ranges.u8_at(at + 3).unwrap_or(0));
                if codepoint < start {
                    Ordering::Greater
                } else if codepoint > start + additional {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });
            if found.is_some() {
                return Some(VariantGlyph::UseDefault);
            }
        }
        let mappings = self.data.offset32(0, 10 + record + 7)?;
        let count = mappings.u32_at(0)? as usize;
        let entries = mappings.range(4, count.checked_mul(MAPPING_SIZE)?)?;
        let at = search(entries, MAPPING_SIZE, |at| {
            entries.u24_at(at).unwrap_or(u32::MAX).cmp(&codepoint)
        })?;
        Some(VariantGlyph::Glyph(GlyphId(entries.u16_at(at + 3)?)))
    }

    fn find_selector(&self, selector: u32) -> Option<usize> {
        search(self.records, SELECTOR_RECORD_SIZE, |at| {
            self.records.u24_at(at).unwrap_or(u32::MAX).cmp(&selector)
        })
    }
}

fn search(data: &[u8], stride: usize, mut compare: impl FnMut(usize) -> Ordering) -> Option<usize> {
    let mut low = 0;
    let mut high = data.len() / stride;
    while low < high {
        let middle = low + (high - low) / 2;
        match compare(middle * stride) {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Some(middle * stride),
        }
    }
    None
}
