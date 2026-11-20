use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::{GlyphInfo, glyph_flag};

#[inline]
pub(crate) fn set_cluster(info: &mut GlyphInfo, cluster: u32, mask: u32) {
    if info.cluster != cluster {
        info.mask = (info.mask & !glyph_flag::DEFINED) | (mask & glyph_flag::DEFINED);
    }
    info.cluster = cluster;
}

impl Buffer {
    #[inline]
    pub(crate) fn merge_clusters(&mut self, start: usize, end: usize) {
        if end - start < 2 {
            return;
        }
        self.merge_clusters_impl(start, end);
    }

    fn merge_clusters_impl(&mut self, mut start: usize, mut end: usize) {
        self.max_ops -= (end - start) as i32;
        if self.max_ops < 0 {
            self.successful = false;
        }
        let cluster = self.info[start..end]
            .iter()
            .map(|info| info.cluster)
            .min()
            .unwrap_or(0);
        if cluster != self.info[end - 1].cluster {
            while end < self.len && self.info[end - 1].cluster == self.info[end].cluster {
                end += 1;
            }
        }
        if cluster != self.info[start].cluster {
            while self.idx < start && self.info[start - 1].cluster == self.info[start].cluster {
                start -= 1;
            }
        }
        if self.idx == start && self.info[start].cluster != cluster {
            let old = self.info[start].cluster;
            let mut index = self.out_len;
            while index != 0 && self.out_info()[index - 1].cluster == old {
                set_cluster(&mut self.out_info_mut()[index - 1], cluster, 0);
                index -= 1;
            }
        }
        for info in &mut self.info[start..end] {
            set_cluster(info, cluster, 0);
        }
    }

    pub(crate) fn merge_out_clusters(&mut self, start: usize, end: usize) {
        if end - start < 2 {
            return;
        }
        self.merge_out_clusters_impl(start, end);
    }

    fn merge_out_clusters_impl(&mut self, mut start: usize, mut end: usize) {
        self.max_ops -= (end - start) as i32;
        if self.max_ops < 0 {
            self.successful = false;
        }
        let cluster = self.out_info()[start..end]
            .iter()
            .map(|info| info.cluster)
            .min()
            .unwrap_or(0);
        while start != 0 && self.out_info()[start - 1].cluster == self.out_info()[start].cluster {
            start -= 1;
        }
        while end < self.out_len && self.out_info()[end - 1].cluster == self.out_info()[end].cluster
        {
            end += 1;
        }
        if end == self.out_len {
            let last = self.out_info()[end - 1].cluster;
            let mut index = self.idx;
            while index < self.len && self.info[index].cluster == last {
                set_cluster(&mut self.info[index], cluster, 0);
                index += 1;
            }
        }
        for info in &mut self.out_info_mut()[start..end] {
            set_cluster(info, cluster, 0);
        }
    }

    pub(crate) fn delete_glyph(&mut self) {
        let cluster = self.info[self.idx].cluster;
        if (self.idx + 1 < self.len && cluster == self.info[self.idx + 1].cluster)
            || (self.out_len != 0 && cluster == self.out_info()[self.out_len - 1].cluster)
        {
            self.skip_glyph();
            return;
        }
        if self.out_len != 0 {
            let previous = self.out_info()[self.out_len - 1].cluster;
            if cluster < previous {
                let mask = self.info[self.idx].mask;
                let mut index = self.out_len;
                while index != 0 && self.out_info()[index - 1].cluster == previous {
                    set_cluster(&mut self.out_info_mut()[index - 1], cluster, mask);
                    index -= 1;
                }
            }
            self.skip_glyph();
            return;
        }
        if self.idx + 1 < self.len {
            self.merge_clusters(self.idx, self.idx + 2);
        }
        self.skip_glyph();
    }

    pub(crate) fn delete_glyphs_inplace(&mut self, filter: impl Fn(&GlyphInfo) -> bool) {
        let mut write_index = 0;
        for index in 0..self.len {
            if filter(&self.info[index]) {
                let cluster = self.info[index].cluster;
                if index + 1 < self.len && cluster == self.info[index + 1].cluster {
                    continue;
                }
                if write_index != 0 {
                    let previous = self.info[write_index - 1].cluster;
                    if cluster < previous {
                        let mask = self.info[index].mask;
                        let mut back = write_index;
                        while back > 0 && self.info[back - 1].cluster == previous {
                            set_cluster(&mut self.info[back - 1], cluster, mask);
                            back -= 1;
                        }
                    }
                    continue;
                }
                if index + 1 < self.len {
                    self.merge_clusters(index, index + 2);
                }
                continue;
            }
            if write_index != index {
                self.info[write_index] = self.info[index];
                self.pos[write_index] = self.pos[index];
            }
            write_index += 1;
        }
        self.len = write_index;
    }

    pub(crate) fn reverse_range(&mut self, start: usize, end: usize) {
        if end - start < 2 {
            return;
        }
        self.info[start..end].reverse();
        if self.have_positions {
            self.pos[start..end].reverse();
        }
    }

    pub(crate) fn reverse(&mut self) {
        if self.len != 0 {
            self.reverse_range(0, self.len);
        }
    }

    pub(crate) fn group_end(
        &self,
        start: usize,
        same_group: impl Fn(&GlyphInfo, &GlyphInfo) -> bool,
    ) -> usize {
        let mut end = start + 1;
        while end < self.len && same_group(&self.info[end - 1], &self.info[end]) {
            end += 1;
        }
        end
    }

    pub(crate) fn reverse_groups(&mut self, same_group: impl Fn(&GlyphInfo, &GlyphInfo) -> bool) {
        if self.len == 0 {
            return;
        }
        let mut start = 0;
        let mut index = 1;
        while index < self.len {
            if !same_group(&self.info[index - 1], &self.info[index]) {
                self.reverse_range(start, index);
                start = index;
            }
            index += 1;
        }
        self.reverse_range(start, index);
        self.reverse();
    }

    pub(crate) fn sort(
        &mut self,
        start: usize,
        end: usize,
        greater: impl Fn(&GlyphInfo, &GlyphInfo) -> bool,
    ) {
        for index in start + 1..end {
            let mut target = index;
            while target > start && greater(&self.info[target - 1], &self.info[index]) {
                target -= 1;
            }
            if index == target {
                continue;
            }
            self.merge_clusters(target, index + 1);
            let moved = self.info[index];
            self.info.copy_within(target..index, target + 1);
            self.info[target] = moved;
        }
    }
}

pub(crate) fn same_cluster(a: &GlyphInfo, b: &GlyphInfo) -> bool {
    a.cluster == b.cluster
}

pub(crate) fn same_grapheme(_: &GlyphInfo, b: &GlyphInfo) -> bool {
    b.is_continuation()
}
