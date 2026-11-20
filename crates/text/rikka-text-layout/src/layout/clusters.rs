use std::ops::{ControlFlow, Range};

use crate::layout::params::LayoutParams;
use crate::layout::tabs::{tab_advance, tab_stop};
use crate::shaping::{Shaped, flag};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cluster {
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) x: f32,
    pub(crate) width: f32,
    pub(crate) flags: u8,
    pub(crate) run: usize,
}

impl Cluster {
    #[inline]
    pub(crate) fn is_whitespace(&self) -> bool {
        self.flags & flag::WHITESPACE != 0
    }

    #[inline]
    pub(crate) fn is_word_separator(&self) -> bool {
        self.flags & flag::WORD_SEPARATOR != 0
    }
}

pub(crate) fn for_each_cluster(
    shaped: &Shaped,
    range: Range<u32>,
    params: &LayoutParams,
    x: f32,
    mut visit: impl FnMut(&Cluster) -> ControlFlow<()>,
) {
    let glyphs = &shaped.glyphs;
    let count = glyphs.len();
    let mut index = glyphs.first_at_or_after(0..count, range.start);
    if index >= count {
        return;
    }
    let mut run = shaped.run_of_glyph(index);
    let mut x = x;
    while index < count && glyphs.clusters[index] < range.end {
        while shaped.runs[run].glyphs.end as usize <= index {
            run += 1;
        }
        let run_info = &shaped.runs[run];
        let size = shaped.styles[usize::from(run_info.style)].size(params.font_size);
        let start = glyphs.clusters[index];
        let flags = glyphs.flags[index];
        let end = glyphs.cluster_ends[index];
        let mut width = 0.0;
        while index < count && glyphs.clusters[index] == start {
            width += glyphs.advances[index] * size;
            index += 1;
        }
        if flags & flag::TAB != 0 {
            let face = &shaped.faces[usize::from(run_info.face)];
            width = tab_advance(x, tab_stop(face, size, shaped.tab_width));
        }
        if glyphs.ends_spaced_cluster(index - 1) {
            width += params.letter_spacing;
        }
        let cluster = Cluster {
            start,
            end,
            x,
            width,
            flags,
            run,
        };
        x += width;
        if visit(&cluster).is_break() {
            break;
        }
    }
}
