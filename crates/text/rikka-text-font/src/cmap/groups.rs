use crate::parse::bytes::Bytes;
use crate::sfnt::glyph_id::GlyphId;

const GROUP_SIZE: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupMapping {
    Sequential,
    Constant,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Groups<'a> {
    groups: &'a [u8],
    mapping: GroupMapping,
}

impl<'a> Groups<'a> {
    pub(crate) fn new(data: &'a [u8], mapping: GroupMapping) -> Option<Self> {
        let count = data.u32_at(12)? as usize;
        let groups = data.range(16, count.checked_mul(GROUP_SIZE)?)?;
        Some(Self { groups, mapping })
    }

    #[inline]
    fn group(&self, index: usize) -> Option<(u32, u32, u32)> {
        let at = index * GROUP_SIZE;
        Some((
            self.groups.u32_at(at)?,
            self.groups.u32_at(at + 4)?,
            self.groups.u32_at(at + 8)?,
        ))
    }

    fn count(&self) -> usize {
        self.groups.len() / GROUP_SIZE
    }

    #[inline]
    pub(crate) fn map(&self, codepoint: u32) -> Option<u16> {
        let mut low = 0;
        let mut high = self.count();
        while low < high {
            let middle = low + (high - low) / 2;
            let (start, end, glyph) = self.group(middle)?;
            if codepoint < start {
                high = middle;
            } else if codepoint > end {
                low = middle + 1;
            } else {
                return u16::try_from(self.glyph_for(start, glyph, codepoint)).ok();
            }
        }
        None
    }

    #[inline]
    fn glyph_for(&self, start: u32, glyph: u32, codepoint: u32) -> u32 {
        match self.mapping {
            GroupMapping::Sequential => glyph.wrapping_add(codepoint - start),
            GroupMapping::Constant => glyph,
        }
    }

    pub(crate) fn for_each_mapping(&self, mut f: impl FnMut(u32, GlyphId)) {
        let mut previous_end = None;
        for index in 0..self.count() {
            let Some((start, end, glyph)) = self.group(index) else {
                return;
            };
            if start > end
                || end > 0x10FFFF
                || previous_end.is_some_and(|previous| start <= previous)
            {
                continue;
            }
            previous_end = Some(end);
            for codepoint in start..=end {
                match u16::try_from(self.glyph_for(start, glyph, codepoint)) {
                    Ok(0) => {}
                    Ok(glyph) => f(codepoint, GlyphId(glyph)),
                    Err(_) => break,
                }
            }
        }
    }
}
