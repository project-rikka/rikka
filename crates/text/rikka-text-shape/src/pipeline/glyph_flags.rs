use crate::buffer::clusters::same_cluster;
use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::glyph_flag;
use crate::common::buffer_flags::BufferFlags;

pub(crate) fn propagate_flags(buffer: &mut Buffer) {
    let mut keep = glyph_flag::DEFINED;
    if !buffer.flags.contains(BufferFlags::PRODUCE_UNSAFE_TO_CONCAT) {
        keep &= !glyph_flag::UNSAFE_TO_CONCAT;
    }
    let tatweel = buffer
        .flags
        .contains(BufferFlags::PRODUCE_SAFE_TO_INSERT_TATWEEL);
    let mut start = 0;
    while start < buffer.len {
        let end = buffer.group_end(start, same_cluster);
        if tatweel {
            if end - start != 1 {
                let mut mask = buffer.info[start..end]
                    .iter()
                    .fold(0, |mask, info| mask | info.mask)
                    & glyph_flag::DEFINED;
                if mask & glyph_flag::UNSAFE_TO_BREAK != 0 {
                    mask &= !glyph_flag::SAFE_TO_INSERT_TATWEEL;
                }
                if mask & glyph_flag::SAFE_TO_INSERT_TATWEEL != 0 {
                    mask |= glyph_flag::UNSAFE_TO_BREAK | glyph_flag::UNSAFE_TO_CONCAT;
                }
                mask &= keep;
                for info in &mut buffer.info[start..end] {
                    info.mask = mask;
                }
            }
        } else if end - start == 1 {
            buffer.info[start].mask &= keep;
        } else {
            let mask = buffer.info[start..end]
                .iter()
                .fold(0, |mask, info| mask | info.mask)
                & keep;
            for info in &mut buffer.info[start..end] {
                info.mask = mask;
            }
        }
        start = end;
    }
}
