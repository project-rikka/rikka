use crate::buffer::glyph_buffer::Buffer;
use crate::unicode::combining_class::{HEBREW_METEG, HEBREW_POINT_VARIKA};

const MODIFIER_COMBINING_MARKS: [u32; 14] = [
    0x0654, 0x0655, 0x0658, 0x06DC, 0x06E3, 0x06E7, 0x06E8, 0x08CA, 0x08CB, 0x08CD, 0x08CE, 0x08CF,
    0x08D3, 0x08F3,
];

pub(crate) fn reorder_modifier_marks(buffer: &mut Buffer, start: usize, end: usize) {
    let mut start = start;
    let mut index = start;
    for class in [220u8, 230] {
        while index < end && buffer.info[index].modified_combining_class() < class {
            index += 1;
        }
        if index == end {
            break;
        }
        if buffer.info[index].modified_combining_class() > class {
            continue;
        }
        let mut run_end = index;
        while run_end < end
            && buffer.info[run_end].modified_combining_class() == class
            && MODIFIER_COMBINING_MARKS.contains(&buffer.info[run_end].codepoint)
        {
            run_end += 1;
        }
        if index == run_end {
            continue;
        }
        let count = run_end - index;
        buffer.merge_clusters(start, run_end);
        buffer.info[start..run_end].rotate_right(count);
        let new_start = start + count;
        let new_class = if class == 220 {
            HEBREW_METEG
        } else {
            HEBREW_POINT_VARIKA
        };
        while start < new_start {
            buffer.info[start].set_modified_combining_class(new_class);
            start += 1;
        }
        index = run_end;
    }
}
