use crate::cmap::subtable::CmapSubtable;
use crate::cmap::variation_selectors::{VariantGlyph, VariationSelectors};
use crate::name::mac_roman;
use crate::parse::bytes::Bytes;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;

const RECORD_SIZE: usize = 8;
const SYMBOL_PRIVATE_USE_BASE: u32 = 0xF000;

#[derive(Clone, Copy, Debug)]
pub struct Cmap<'a> {
    data: &'a [u8],
    records: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Repertoire {
    Unicode,
    Symbol,
    MacRoman,
}

#[derive(Clone, Copy, Debug)]
pub struct Charmap<'a> {
    subtable: CmapSubtable<'a>,
    repertoire: Repertoire,
    variations: Option<VariationSelectors<'a>>,
}

impl<'a> Cmap<'a> {
    pub fn new(data: &'a [u8]) -> Option<Self> {
        let count = usize::from(data.u16_at(2)?);
        let records = data.range(4, count * RECORD_SIZE)?;
        Some(Self { data, records })
    }

    pub fn subtables(&self) -> impl Iterator<Item = CmapSubtable<'a>> + use<'a> {
        let cmap = *self;
        (0..self.records.len() / RECORD_SIZE).filter_map(move |index| cmap.subtable(index))
    }

    fn subtable(&self, index: usize) -> Option<CmapSubtable<'a>> {
        let record = index * RECORD_SIZE;
        let platform_id = self.records.u16_at(record)?;
        let encoding_id = self.records.u16_at(record + 2)?;
        let data = self.data.tail(self.records.u32_at(record + 4)? as usize)?;
        CmapSubtable::new(platform_id, encoding_id, data)
    }

    pub fn variation_selectors(&self) -> Option<VariationSelectors<'a>> {
        (0..self.records.len() / RECORD_SIZE).find_map(|index| {
            let record = index * RECORD_SIZE;
            if self.records.u16_at(record)? != 0 || self.records.u16_at(record + 2)? != 5 {
                return None;
            }
            VariationSelectors::new(self.data.tail(self.records.u32_at(record + 4)? as usize)?)
        })
    }

    pub fn charmap(&self) -> Option<Charmap<'a>> {
        let mut best: Option<(u8, CmapSubtable<'a>)> = None;
        for subtable in self.subtables() {
            if !subtable.is_supported() {
                continue;
            }
            let rank = match (subtable.platform_id, subtable.encoding_id, subtable.format) {
                (3, 10, 12 | 13) | (0, 4 | 6, 12 | 13) => 9,
                (3, 10, _) | (0, 4 | 6, _) => 8,
                (3, 1, _) => 7,
                (0, 3, _) => 6,
                (0, 0..=2, _) => 5,
                (3, 0, _) => 4,
                (1, 0, _) => 3,
                _ => continue,
            };
            if best.is_none_or(|(best_rank, _)| rank > best_rank) {
                best = Some((rank, subtable));
            }
        }
        let (rank, subtable) = best?;
        Some(Charmap {
            subtable,
            repertoire: match rank {
                4 => Repertoire::Symbol,
                3 => Repertoire::MacRoman,
                _ => Repertoire::Unicode,
            },
            variations: self.variation_selectors(),
        })
    }
}

impl<'a> Charmap<'a> {
    #[inline]
    pub fn subtable(&self) -> CmapSubtable<'a> {
        self.subtable
    }

    #[inline]
    pub fn map(&self, codepoint: u32) -> Option<GlyphId> {
        match self.repertoire {
            Repertoire::Unicode => self.subtable.map(codepoint),
            Repertoire::Symbol => self.subtable.map(codepoint).or_else(|| {
                (codepoint <= 0xFF)
                    .then(|| self.subtable.map(SYMBOL_PRIVATE_USE_BASE + codepoint))
                    .flatten()
            }),
            Repertoire::MacRoman => self.subtable.map(u32::from(mac_roman::encode(codepoint)?)),
        }
    }

    pub fn map_variant(&self, codepoint: u32, selector: u32) -> Option<VariantGlyph> {
        self.variations?.map(codepoint, selector)
    }

    pub fn variation_selectors(&self) -> Option<VariationSelectors<'a>> {
        self.variations
    }

    pub fn for_each_mapping(&self, mut f: impl FnMut(u32, GlyphId)) {
        match self.repertoire {
            Repertoire::Unicode | Repertoire::Symbol => self.subtable.for_each_mapping(f),
            Repertoire::MacRoman => self.subtable.for_each_mapping(|code, glyph| {
                if let Ok(byte) = u8::try_from(code) {
                    f(u32::from(mac_roman::decode(byte)), glyph);
                }
            }),
        }
    }
}

impl<'a> Font<'a> {
    pub fn cmap(&self) -> Option<Cmap<'a>> {
        Cmap::new(self.table_data(TableId::Cmap)?)
    }

    pub fn charmap(&self) -> Option<Charmap<'a>> {
        self.cmap()?.charmap()
    }
}
