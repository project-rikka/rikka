use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use rikka_text_font::{Coverage, GlyphId};

use crate::face::glyph_sets::{BLOCK_WORDS, Block, GlyphSets, PAGE_BITS, PAGE_GLYPHS};

#[derive(Debug, Default)]
pub(crate) struct GlyphSetsBuilder {
    members: Vec<Vec<u16>>,
    ordered: HashMap<Vec<(u16, u16)>, u16, BuildHasherDefault<RangeHasher>>,
}

#[derive(Debug, Default)]
struct RangeHasher(u64);

impl Hasher for RangeHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x0100_0000_01B3);
        }
    }

    #[inline]
    fn write_u16(&mut self, value: u16) {
        self.0 = (self.0.rotate_left(5) ^ u64::from(value)).wrapping_mul(0x517C_C1B7_2722_0A95);
    }
}

impl GlyphSetsBuilder {
    pub(crate) fn add(&mut self, coverage: Option<Coverage<'_>>) -> Option<u16> {
        let ranges: Vec<(u16, u16)> = coverage
            .iter()
            .flat_map(|coverage| coverage.ranges())
            .map(|(first, last)| (first.0, last.0))
            .collect();
        if let Some(coverage) = coverage.filter(|coverage| !coverage.is_ordered()) {
            let mut glyphs: Vec<u16> = ranges
                .iter()
                .flat_map(|&(first, last)| first..=last)
                .filter(|&glyph| coverage.contains(GlyphId(glyph)))
                .collect();
            glyphs.sort_unstable();
            glyphs.dedup();
            return self.push(glyphs);
        }
        if let Some(&id) = self.ordered.get(&ranges) {
            return Some(id);
        }
        let glyphs = ranges
            .iter()
            .flat_map(|&(first, last)| first..=last)
            .collect();
        let id = self.push(glyphs)?;
        self.ordered.insert(ranges, id);
        Some(id)
    }

    fn push(&mut self, glyphs: Vec<u16>) -> Option<u16> {
        let id = u16::try_from(self.members.len()).ok()?;
        self.members.push(glyphs);
        Some(id)
    }

    pub(crate) fn build(self) -> GlyphSets {
        let pages = self
            .members
            .iter()
            .filter_map(|glyphs| glyphs.last())
            .map(|&last| usize::from(last >> PAGE_BITS) + 1)
            .max()
            .unwrap_or(0);
        let mut blocks: Vec<Block> = vec![[0; BLOCK_WORDS]];
        let mut block_ids: HashMap<Block, u32> = HashMap::from([([0; BLOCK_WORDS], 0)]);
        let mut directory = vec![0u32; pages * self.members.len()];
        for (set, glyphs) in self.members.iter().enumerate() {
            for page_glyphs in glyphs.chunk_by(|a, b| a >> PAGE_BITS == b >> PAGE_BITS) {
                let mut block = [0u64; BLOCK_WORDS];
                for &glyph in page_glyphs {
                    let bit = usize::from(glyph) % PAGE_GLYPHS;
                    block[bit / 64] |= 1 << (bit % 64);
                }
                let id = *block_ids.entry(block).or_insert_with(|| {
                    blocks.push(block);
                    (blocks.len() - 1) as u32
                });
                let page = usize::from(page_glyphs[0] >> PAGE_BITS);
                directory[set * pages + page] = id;
            }
        }
        GlyphSets::new(
            pages,
            directory.into_boxed_slice(),
            blocks.into_boxed_slice(),
        )
    }
}
