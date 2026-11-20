use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::scratch_flags;
use crate::common::buffer_flags::BufferFlags;
use crate::face::font_context::FontContext;

fn hides_ignorables(buffer: &Buffer) -> bool {
    buffer.scratch_flags & scratch_flags::HAS_DEFAULT_IGNORABLES != 0
        && !buffer
            .flags
            .contains(BufferFlags::PRESERVE_DEFAULT_IGNORABLES)
}

pub(crate) fn zero_width_default_ignorables(buffer: &mut Buffer) {
    if !hides_ignorables(buffer)
        || buffer
            .flags
            .contains(BufferFlags::REMOVE_DEFAULT_IGNORABLES)
    {
        return;
    }
    let len = buffer.len;
    for (info, position) in buffer.info[..len].iter().zip(&mut buffer.pos[..len]) {
        if info.is_default_ignorable() {
            position.x_advance = 0;
            position.y_advance = 0;
            position.x_offset = 0;
        }
    }
}

pub(crate) fn hide_default_ignorables(font: &FontContext, buffer: &mut Buffer) {
    if !hides_ignorables(buffer) {
        return;
    }
    if !buffer
        .flags
        .contains(BufferFlags::REMOVE_DEFAULT_IGNORABLES)
        && let Some(space) = font.nominal_glyph(u32::from(b' '))
    {
        for info in &mut buffer.info[..buffer.len] {
            if info.is_default_ignorable() {
                info.codepoint = u32::from(space);
            }
        }
        return;
    }
    buffer.delete_glyphs_inplace(|info| info.is_default_ignorable());
}
