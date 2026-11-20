pub(crate) mod flag {
    pub(crate) const UNSAFE_TO_BREAK: u8 = 1;
    pub(crate) const WHITESPACE: u8 = 2;
    pub(crate) const TAB: u8 = 4;
    pub(crate) const SEPARATOR: u8 = 8;
    pub(crate) const WORD_SEPARATOR: u8 = 16;
    pub(crate) const LINE_BREAK: u8 = 32;
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GlyphColumns {
    pub(crate) ids: Vec<u16>,
    pub(crate) advances: Vec<f32>,
    pub(crate) x_offsets: Vec<f32>,
    pub(crate) y_offsets: Vec<f32>,
    pub(crate) clusters: Vec<u32>,
    pub(crate) cluster_ends: Vec<u32>,
    pub(crate) flags: Vec<u8>,
}

impl GlyphColumns {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.ids.len()
    }

    pub(crate) fn clear(&mut self) {
        self.ids.clear();
        self.advances.clear();
        self.x_offsets.clear();
        self.y_offsets.clear();
        self.clusters.clear();
        self.cluster_ends.clear();
        self.flags.clear();
    }

    pub(crate) fn truncate(&mut self, len: usize) {
        self.ids.truncate(len);
        self.advances.truncate(len);
        self.x_offsets.truncate(len);
        self.y_offsets.truncate(len);
        self.clusters.truncate(len);
        self.cluster_ends.truncate(len);
        self.flags.truncate(len);
    }

    pub(crate) fn push_ellipsis_glyph(&mut self, glyph: &crate::fonts::EllipsisGlyph, at: u32) {
        self.ids.push(glyph.id);
        self.advances.push(glyph.advance);
        self.x_offsets.push(glyph.x_offset);
        self.y_offsets.push(glyph.y_offset);
        self.clusters.push(at);
        self.cluster_ends.push(at);
        self.flags.push(0);
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        self.ids.capacity() * 2
            + (self.advances.capacity()
                + self.x_offsets.capacity()
                + self.y_offsets.capacity()
                + self.clusters.capacity()
                + self.cluster_ends.capacity())
                * 4
            + self.flags.capacity()
    }

    pub(crate) fn first_at_or_after(&self, glyphs: std::ops::Range<usize>, offset: u32) -> usize {
        glyphs.start + self.clusters[glyphs].partition_point(|&cluster| cluster < offset)
    }

    pub(crate) fn safe_before(&self, index: usize) -> bool {
        self.flags[index] & flag::UNSAFE_TO_BREAK == 0
    }

    #[inline]
    pub(crate) fn ends_spaced_cluster(&self, index: usize) -> bool {
        let cluster = self.clusters[index];
        self.cluster_ends[index] > cluster
            && self.flags[index] & flag::LINE_BREAK == 0
            && self
                .clusters
                .get(index + 1)
                .is_none_or(|&next| next != cluster)
    }
}
