use crate::parse::array::Array;
use crate::parse::bytes::Bytes;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Format4<'a> {
    end_codes: Array<'a, u16>,
    start_codes: Array<'a, u16>,
    deltas: Array<'a, u16>,
    range_offsets: Array<'a, u16>,
    range_offsets_data: &'a [u8],
}

impl<'a> Format4<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let segments = usize::from(data.u16_at(6)? / 2);
        let ends_at = 14;
        let starts_at = ends_at + segments * 2 + 2;
        let deltas_at = starts_at + segments * 2;
        let range_offsets_at = deltas_at + segments * 2;
        Some(Self {
            end_codes: data.array(ends_at, segments)?,
            start_codes: data.array(starts_at, segments)?,
            deltas: data.array(deltas_at, segments)?,
            range_offsets: data.array(range_offsets_at, segments)?,
            range_offsets_data: data.tail(range_offsets_at)?,
        })
    }

    #[inline]
    pub(crate) fn map(&self, codepoint: u32) -> Option<u16> {
        let codepoint = u16::try_from(codepoint).ok()?;
        let segment = match self.end_codes.binary_search_by(|end| end.cmp(&codepoint)) {
            Ok(index) | Err(index) => index,
        };
        self.map_in_segment(segment, codepoint)
    }

    #[inline]
    fn map_in_segment(&self, segment: usize, codepoint: u16) -> Option<u16> {
        let start = self.start_codes.get(segment)?;
        if codepoint < start {
            return None;
        }
        let delta = self.deltas.get(segment)?;
        let range_offset = self.range_offsets.get(segment)?;
        if range_offset == 0 {
            return Some(codepoint.wrapping_add(delta));
        }
        let at = segment * 2 + usize::from(range_offset) + usize::from(codepoint - start) * 2;
        let glyph = self.range_offsets_data.u16_at(at)?;
        (glyph != 0).then(|| glyph.wrapping_add(delta))
    }

    pub(crate) fn for_each_mapping(&self, mut f: impl FnMut(u32, crate::sfnt::glyph_id::GlyphId)) {
        let mut next_unmapped = 0u32;
        for segment in 0..self.end_codes.len() {
            let (Some(start), Some(end)) =
                (self.start_codes.get(segment), self.end_codes.get(segment))
            else {
                return;
            };
            if start > end || u32::from(start) < next_unmapped {
                continue;
            }
            next_unmapped = u32::from(end) + 1;
            for codepoint in start..=end {
                if let Some(glyph) = self
                    .map_in_segment(segment, codepoint)
                    .filter(|&glyph| glyph != 0)
                {
                    f(u32::from(codepoint), crate::sfnt::glyph_id::GlyphId(glyph));
                }
            }
        }
    }
}
