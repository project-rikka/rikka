use rikka_text_font::{ClassDef, Coverage, GlyphId};

const PAGE_BITS: u32 = 8;
const PAGE_SIZE: usize = 1 << PAGE_BITS;
const NO_PAGE: u16 = u16::MAX;
const ABSENT: u16 = u16::MAX;

#[derive(Debug)]
pub(crate) struct GlyphMap {
    pages: Box<[u16]>,
    blocks: Box<[[u16; PAGE_SIZE]]>,
}

impl GlyphMap {
    fn build(entries: impl Iterator<Item = (u16, u16)>, empty: u16) -> Self {
        let mut pages: Vec<u16> = Vec::new();
        let mut blocks: Vec<[u16; PAGE_SIZE]> = Vec::new();
        for (glyph, value) in entries {
            let page = usize::from(glyph >> PAGE_BITS);
            if page >= pages.len() {
                pages.resize(page + 1, NO_PAGE);
            }
            if pages[page] == NO_PAGE {
                pages[page] = blocks.len() as u16;
                blocks.push([empty; PAGE_SIZE]);
            }
            blocks[usize::from(pages[page])][usize::from(glyph) % PAGE_SIZE] = value;
        }
        Self {
            pages: pages.into_boxed_slice(),
            blocks: blocks.into_boxed_slice(),
        }
    }

    pub(crate) fn from_coverage(coverage: &Coverage<'_>) -> Self {
        let clamp = |(glyph, index): (GlyphId, u16)| (glyph.0, index.min(ABSENT - 1));
        if coverage.is_ordered() {
            return Self::build(coverage.entries().map(clamp), ABSENT);
        }
        let entries = coverage
            .glyphs()
            .filter_map(|glyph| Some((glyph, coverage.get(glyph)?)));
        Self::build(entries.map(clamp), ABSENT)
    }

    pub(crate) fn from_class_def(class_def: &ClassDef<'_>) -> Self {
        let entries = class_def
            .ranges()
            .flat_map(|(first, last, _)| first.0..=last.0)
            .map(|glyph| (glyph, class_def.get(GlyphId(glyph))));
        Self::build(entries, 0)
    }

    #[inline]
    fn value(&self, glyph: u32) -> Option<u16> {
        let page = *self.pages.get((glyph >> PAGE_BITS) as usize)?;
        if page == NO_PAGE {
            return None;
        }
        Some(self.blocks[usize::from(page)][glyph as usize % PAGE_SIZE])
    }

    #[inline]
    pub(crate) fn coverage_index(&self, glyph: u32) -> Option<u16> {
        self.value(glyph).filter(|&index| index != ABSENT)
    }

    #[inline]
    pub(crate) fn class(&self, glyph: u32) -> u16 {
        self.value(glyph).unwrap_or(0)
    }
}
