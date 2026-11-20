use crate::face::atomic_cache::AtomicCache;

const SLOTS: usize = 4096;
const MAPPED: u32 = 1 << 16;

#[derive(Debug, Default)]
pub(crate) struct CmapCache {
    glyphs: AtomicCache<SLOTS>,
}

impl CmapCache {
    #[inline]
    pub(crate) fn get(&self, codepoint: u32) -> Option<Option<u16>> {
        let value = self.glyphs.get(codepoint)?;
        Some((value & MAPPED != 0).then_some(value as u16))
    }

    #[inline]
    pub(crate) fn set(&self, codepoint: u32, glyph: Option<u16>) {
        self.glyphs.set(
            codepoint,
            glyph.map_or(0, |glyph| MAPPED | u32::from(glyph)),
        );
    }
}
