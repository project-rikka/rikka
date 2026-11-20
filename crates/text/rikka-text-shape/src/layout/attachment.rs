use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_position::{GlyphPosition, attach_kind};
use crate::buffer::scratch_flags;
use crate::layout::apply_context::MAX_NESTING_LEVEL;

pub(crate) fn clear_attachments(buffer: &mut Buffer) {
    for position in &mut buffer.pos[..buffer.len] {
        position.attach_chain = 0;
        position.attach_kind = 0;
    }
}

pub(crate) fn propagate_attachment_offsets(buffer: &mut Buffer) {
    if buffer.scratch_flags & scratch_flags::HAS_GPOS_ATTACHMENT == 0 {
        return;
    }
    let len = buffer.len;
    let forward = buffer.direction.is_forward();
    let positions = &mut buffer.pos[..len];
    if forward {
        for index in 0..len {
            if positions[index].attach_chain != 0 {
                propagate(positions, index, forward, MAX_NESTING_LEVEL);
            }
        }
    } else {
        for index in (0..len).rev() {
            if positions[index].attach_chain != 0 {
                propagate(positions, index, forward, MAX_NESTING_LEVEL);
            }
        }
    }
}

fn propagate(positions: &mut [GlyphPosition], index: usize, forward: bool, nesting: usize) {
    let chain = positions[index].attach_chain;
    let kind = positions[index].attach_kind;
    positions[index].attach_chain = 0;
    let target = (index as isize + isize::from(chain)) as usize;
    if target >= positions.len() || nesting == 0 {
        return;
    }
    if positions[target].attach_chain != 0 {
        propagate(positions, target, forward, nesting - 1);
    }
    match kind {
        attach_kind::MARK => {
            positions[index].x_offset += positions[target].x_offset;
            positions[index].y_offset += positions[target].y_offset;
            if target < index {
                if forward {
                    for between in target..index {
                        positions[index].x_offset -= positions[between].x_advance;
                        positions[index].y_offset -= positions[between].y_advance;
                    }
                } else {
                    for between in target + 1..=index {
                        positions[index].x_offset += positions[between].x_advance;
                        positions[index].y_offset += positions[between].y_advance;
                    }
                }
            } else if forward {
                for between in index..target {
                    positions[index].x_offset += positions[between].x_advance;
                    positions[index].y_offset += positions[between].y_advance;
                }
            } else {
                for between in index + 1..=target {
                    positions[index].x_offset -= positions[between].x_advance;
                    positions[index].y_offset -= positions[between].y_advance;
                }
            }
        }
        attach_kind::CURSIVE => {
            positions[index].y_offset += positions[target].y_offset;
        }
        _ => {}
    }
}
