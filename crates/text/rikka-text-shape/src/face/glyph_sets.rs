pub(crate) const PAGE_BITS: u32 = 8;
pub(crate) const PAGE_GLYPHS: usize = 1 << PAGE_BITS;
pub(crate) const BLOCK_WORDS: usize = PAGE_GLYPHS / 64;

pub(crate) type Block = [u64; BLOCK_WORDS];

#[derive(Debug, Default)]
pub(crate) struct GlyphSets {
    pages: usize,
    directory: Box<[u32]>,
    blocks: Box<[Block]>,
}

impl GlyphSets {
    pub(crate) fn new(pages: usize, directory: Box<[u32]>, blocks: Box<[Block]>) -> Self {
        Self {
            pages,
            directory,
            blocks,
        }
    }

    #[inline]
    pub(crate) fn contains(&self, set: u16, glyph: u32) -> bool {
        let page = (glyph >> PAGE_BITS) as usize;
        if page >= self.pages {
            return false;
        }
        let Some(&block) = self.directory.get(usize::from(set) * self.pages + page) else {
            return false;
        };
        let bit = glyph as usize % PAGE_GLYPHS;
        self.blocks[block as usize][bit / 64] >> (bit % 64) & 1 != 0
    }
}
