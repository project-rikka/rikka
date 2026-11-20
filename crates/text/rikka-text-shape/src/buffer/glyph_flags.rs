use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::{GlyphInfo, glyph_flag};
use crate::common::buffer_flags::BufferFlags;

const MAX_FLAGGED_RANGE: usize = 255;

impl Buffer {
    #[inline]
    pub(crate) fn unsafe_to_break(&mut self, start: usize, end: usize) {
        self.set_glyph_flags(
            glyph_flag::UNSAFE_TO_BREAK | glyph_flag::UNSAFE_TO_CONCAT,
            Some((start, end)),
            true,
            false,
        );
    }

    #[inline]
    pub(crate) fn unsafe_to_break_from_outbuffer(&mut self, start: usize, end: usize) {
        self.set_glyph_flags(
            glyph_flag::UNSAFE_TO_BREAK | glyph_flag::UNSAFE_TO_CONCAT,
            Some((start, end)),
            true,
            true,
        );
    }

    #[inline]
    pub(crate) fn unsafe_to_concat(&mut self, start: usize, end: usize) {
        if self.flags.contains(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT) {
            self.set_glyph_flags(
                glyph_flag::UNSAFE_TO_CONCAT,
                Some((start, end)),
                false,
                false,
            );
        }
    }

    #[inline]
    pub(crate) fn unsafe_to_concat_all(&mut self) {
        if self.flags.contains(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT) {
            self.set_glyph_flags(glyph_flag::UNSAFE_TO_CONCAT, None, false, false);
        }
    }

    #[inline]
    pub(crate) fn unsafe_to_concat_from_outbuffer(&mut self, start: usize, end: usize) {
        if self.flags.contains(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT) {
            self.set_glyph_flags(
                glyph_flag::UNSAFE_TO_CONCAT,
                Some((start, end)),
                false,
                true,
            );
        }
    }

    pub(crate) fn safe_to_insert_tatweel(&mut self, start: usize, end: usize) {
        if !self
            .flags
            .contains(BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL)
        {
            self.unsafe_to_break(start, end);
            return;
        }
        self.set_glyph_flags(
            glyph_flag::SAFE_TO_INSERT_TATWEEL,
            Some((start, end)),
            true,
            false,
        );
    }

    fn set_glyph_flags(
        &mut self,
        mask: u32,
        range: Option<(usize, usize)>,
        interior: bool,
        from_out_buffer: bool,
    ) {
        if let Some((start, end)) = range
            && end.wrapping_sub(start) > MAX_FLAGGED_RANGE
        {
            return;
        }
        let (start, end) = range.unwrap_or((0, self.len));
        let end = end.min(self.len);
        if interior && !from_out_buffer && end.saturating_sub(start) < 2 {
            return;
        }
        if !from_out_buffer || !self.have_output {
            if !interior {
                for info in &mut self.info[start.min(end)..end] {
                    info.mask |= mask;
                }
            } else {
                let cluster = min_cluster(&self.info, start, end, u32::MAX);
                set_interior_flags(&mut self.info, start, end, cluster, mask);
            }
        } else if !interior {
            let out_len = self.out_len;
            for info in &mut self.out_info_mut()[start.min(out_len)..out_len] {
                info.mask |= mask;
            }
            let idx = self.idx;
            for info in &mut self.info[idx.min(end)..end] {
                info.mask |= mask;
            }
        } else {
            let cluster = min_cluster(&self.info, self.idx, end, u32::MAX);
            let cluster = min_cluster(self.out_info(), start, self.out_len, cluster);
            let out_len = self.out_len;
            set_interior_flags(self.out_info_mut(), start, out_len, cluster, mask);
            let idx = self.idx;
            set_interior_flags(&mut self.info, idx, end, cluster, mask);
        }
    }
}

fn min_cluster(infos: &[GlyphInfo], start: usize, end: usize, cluster: u32) -> u32 {
    if start >= end {
        return cluster;
    }
    cluster.min(infos[start].cluster.min(infos[end - 1].cluster))
}

fn set_interior_flags(infos: &mut [GlyphInfo], start: usize, end: usize, cluster: u32, mask: u32) {
    if start >= end {
        return;
    }
    let first = infos[start].cluster;
    let last = infos[end - 1].cluster;
    if cluster != first && cluster != last {
        for info in &mut infos[start..end] {
            if info.cluster != cluster {
                info.mask |= mask;
            }
        }
        return;
    }
    if cluster == first {
        let mut index = end;
        while start < index && infos[index - 1].cluster != first {
            if cluster != infos[index - 1].cluster {
                infos[index - 1].mask |= mask;
            }
            index -= 1;
        }
    } else {
        let mut index = start;
        while index < end && infos[index].cluster != last {
            if cluster != infos[index].cluster {
                infos[index].mask |= mask;
            }
            index += 1;
        }
    }
}
