use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::GlyphInfo;
use crate::buffer::scratch_flags;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::syllabic::machine::SyllableMachine;

const SERIAL_LIMIT: u8 = 16;
const KIND_MASK: u8 = 0x0F;

#[inline]
pub(crate) fn syllable_kind(info: &GlyphInfo) -> u8 {
    info.syllable & KIND_MASK
}

pub(crate) fn find_syllables(buffer: &mut Buffer, machine: &SyllableMachine, broken: u8) {
    find_syllables_skipping(buffer, machine, broken, |_, _| true);
}

pub(crate) fn find_syllables_skipping(
    buffer: &mut Buffer,
    machine: &SyllableMachine,
    broken: u8,
    included: impl Fn(&[GlyphInfo], usize) -> bool,
) {
    let len = buffer.len;
    let next = |infos: &[GlyphInfo], from: usize| {
        (from..len)
            .find(|&index| included(infos, index))
            .unwrap_or(len)
    };
    let mut serial = 1u8;
    let mut start = next(&buffer.info[..len], 0);
    while start < len {
        let infos = &buffer.info[..len];
        let (end, kind) = machine.longest_match(
            start,
            len,
            |from| next(infos, from),
            |index| infos[index].shaper_category,
        );
        for info in &mut buffer.info[start..end] {
            info.syllable = (serial << 4) | kind;
        }
        if kind == broken {
            buffer.scratch_flags |= scratch_flags::HAS_BROKEN_SYLLABLE;
        }
        serial += 1;
        if serial == SERIAL_LIMIT {
            serial = 1;
        }
        start = end;
    }
}

#[inline]
pub(crate) fn next_syllable(buffer: &Buffer, start: usize) -> usize {
    if start >= buffer.len {
        return start;
    }
    let syllable = buffer.info[start].syllable;
    let mut end = start + 1;
    while end < buffer.len && buffer.info[end].syllable == syllable {
        end += 1;
    }
    end
}

pub(crate) fn for_each_syllable(
    buffer: &mut Buffer,
    mut visit: impl FnMut(&mut Buffer, usize, usize),
) {
    let mut start = 0;
    while start < buffer.len {
        let end = next_syllable(buffer, start);
        visit(buffer, start, end);
        start = end;
    }
}

pub(crate) fn mark_syllables_unsafe_to_break(buffer: &mut Buffer) {
    for_each_syllable(buffer, |buffer, start, end| {
        buffer.unsafe_to_break(start, end)
    });
}

pub(crate) fn clear_syllables(_: &PlanCore, _: &FontContext, buffer: &mut Buffer) -> bool {
    for info in &mut buffer.info[..buffer.len] {
        info.syllable = 0;
    }
    false
}
