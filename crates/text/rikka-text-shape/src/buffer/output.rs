use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::GlyphInfo;
use crate::buffer::glyph_position::GlyphPosition;

impl Buffer {
    pub(crate) fn clear_output(&mut self) {
        self.have_output = true;
        self.have_positions = false;
        self.idx = 0;
        self.out_len = 0;
        self.have_separate_output = false;
    }

    pub(crate) fn clear_positions(&mut self) {
        self.have_output = false;
        self.have_positions = true;
        self.out_len = 0;
        self.have_separate_output = false;
        for position in &mut self.pos[..self.len] {
            *position = GlyphPosition::default();
        }
    }

    pub(crate) fn sync(&mut self) -> bool {
        if !self.successful {
            self.have_output = false;
            self.out_len = 0;
            self.idx = 0;
            return false;
        }
        self.next_glyphs(self.len - self.idx);
        if self.have_separate_output {
            std::mem::swap(&mut self.info, &mut self.out);
            self.have_separate_output = false;
        }
        self.len = self.out_len;
        self.have_output = false;
        self.out_len = 0;
        self.idx = 0;
        true
    }

    #[must_use]
    pub(crate) fn make_room_for(&mut self, count_in: usize, count_out: usize) -> bool {
        if !self.ensure(self.out_len + count_out) {
            return false;
        }
        if !self.have_separate_output && self.out_len + count_out > self.idx + count_in {
            self.have_separate_output = true;
            let out_len = self.out_len;
            self.out[..out_len].copy_from_slice(&self.info[..out_len]);
        }
        true
    }

    fn set_out_info(&mut self, index: usize, info: GlyphInfo) {
        self.out_info_mut()[index] = info;
    }

    #[inline]
    pub(crate) fn next_glyph(&mut self) {
        if self.have_output {
            if self.have_separate_output || self.out_len != self.idx {
                if !self.ensure(self.out_len + 1) {
                    return;
                }
                let info = self.info[self.idx];
                let out_len = self.out_len;
                self.set_out_info(out_len, info);
            }
            self.out_len += 1;
        }
        self.idx += 1;
    }

    pub(crate) fn next_glyphs(&mut self, count: usize) {
        if self.have_output {
            if self.have_separate_output || self.out_len != self.idx {
                if !self.ensure(self.out_len + count) {
                    return;
                }
                let (idx, out_len) = (self.idx, self.out_len);
                if self.have_separate_output {
                    self.out[out_len..out_len + count]
                        .copy_from_slice(&self.info[idx..idx + count]);
                } else {
                    self.info.copy_within(idx..idx + count, out_len);
                }
            }
            self.out_len += count;
        }
        self.idx += count;
    }

    #[inline]
    pub(crate) fn skip_glyph(&mut self) {
        self.idx += 1;
    }

    pub(crate) fn replace_glyph(&mut self, codepoint: u32) {
        if self.have_separate_output || self.out_len != self.idx {
            if !self.make_room_for(1, 1) {
                return;
            }
            let info = self.info[self.idx];
            let out_len = self.out_len;
            self.set_out_info(out_len, info);
        }
        let out_len = self.out_len;
        self.out_info_mut()[out_len].codepoint = codepoint;
        self.idx += 1;
        self.out_len += 1;
    }

    pub(crate) fn replace_glyphs(&mut self, count_in: usize, codepoints: &[u32]) {
        if !self.make_room_for(count_in, codepoints.len()) {
            return;
        }
        self.merge_clusters(self.idx, self.idx + count_in);
        let original = self.info[self.idx];
        for (offset, &codepoint) in codepoints.iter().enumerate() {
            let index = self.out_len + offset;
            let mut info = original;
            info.codepoint = codepoint;
            self.set_out_info(index, info);
        }
        self.idx += count_in;
        self.out_len += codepoints.len();
    }

    pub(crate) fn output_glyph(&mut self, codepoint: u32) {
        if !self.make_room_for(0, 1) {
            return;
        }
        if self.idx == self.len && self.out_len == 0 {
            return;
        }
        let out_len = self.out_len;
        let mut info = if self.idx < self.len {
            self.info[self.idx]
        } else {
            self.out_info()[out_len - 1]
        };
        info.codepoint = codepoint;
        self.set_out_info(out_len, info);
        self.out_len += 1;
    }

    pub(crate) fn output_info(&mut self, info: GlyphInfo) {
        if !self.make_room_for(0, 1) {
            return;
        }
        let out_len = self.out_len;
        self.set_out_info(out_len, info);
        self.out_len += 1;
    }

    pub(crate) fn move_to(&mut self, target: usize) -> bool {
        if !self.have_output {
            self.idx = target;
            return true;
        }
        if !self.successful {
            return false;
        }
        if self.out_len < target {
            let count = target - self.out_len;
            if !self.make_room_for(count, count) {
                return false;
            }
            let (idx, out_len) = (self.idx, self.out_len);
            if self.have_separate_output {
                self.out[out_len..out_len + count].copy_from_slice(&self.info[idx..idx + count]);
            } else {
                self.info.copy_within(idx..idx + count, out_len);
            }
            self.idx += count;
            self.out_len += count;
        } else if self.out_len > target {
            let count = self.out_len - target;
            if self.idx < count && !self.shift_forward(count - self.idx) {
                return false;
            }
            self.idx -= count;
            self.out_len -= count;
            let (idx, out_len) = (self.idx, self.out_len);
            if self.have_separate_output {
                self.info[idx..idx + count].copy_from_slice(&self.out[out_len..out_len + count]);
            } else {
                self.info.copy_within(out_len..out_len + count, idx);
            }
        }
        true
    }

    fn shift_forward(&mut self, count: usize) -> bool {
        if !self.ensure(self.len + count) {
            return false;
        }
        self.max_ops -= (self.len - self.idx) as i32;
        if self.max_ops < 0 {
            self.successful = false;
            return false;
        }
        let (idx, len) = (self.idx, self.len);
        self.info.copy_within(idx..len, idx + count);
        if idx + count > len {
            for info in &mut self.info[len..idx + count] {
                *info = GlyphInfo::default();
            }
        }
        self.len += count;
        self.idx += count;
        true
    }
}
