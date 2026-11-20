use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::GlyphInfo;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::syllabic::dotted_circles::DottedCircle;
use crate::shapers::syllabic::syllables::{for_each_syllable, syllable_kind};
use crate::shapers::universal::categories::category as cat;
use crate::shapers::universal::machine::syllable;

const DOTTED_CIRCLE: DottedCircle = DottedCircle {
    broken: syllable::BROKEN,
    category: cat::B,
    position: 0,
    after_repha: Some(cat::R),
};

const fn flag(category: u8) -> u64 {
    1 << category
}

const POST_BASE: u64 = flag(cat::FABV)
    | flag(cat::FBLW)
    | flag(cat::FPST)
    | flag(cat::FMABV)
    | flag(cat::FMBLW)
    | flag(cat::FMPST)
    | flag(cat::MABV)
    | flag(cat::MBLW)
    | flag(cat::MPST)
    | flag(cat::MPRE)
    | flag(cat::VABV)
    | flag(cat::VBLW)
    | flag(cat::VPST)
    | flag(cat::VPRE)
    | flag(cat::VMABV)
    | flag(cat::VMBLW)
    | flag(cat::VMPST)
    | flag(cat::VMPRE);

const PRE_BASE_VOWELS: u64 = flag(cat::VPRE) | flag(cat::VMPRE);

#[inline]
fn has_category(info: &GlyphInfo, flags: u64) -> bool {
    flag(info.shaper_category & 63) & flags != 0
}

fn is_halant(info: &GlyphInfo) -> bool {
    matches!(info.shaper_category, cat::H | cat::HVM | cat::IS) && !info.ligated()
}

pub(crate) fn reorder(_: &PlanCore, font: &FontContext, buffer: &mut Buffer) -> bool {
    let inserted = DOTTED_CIRCLE.insert(font, buffer);
    for_each_syllable(buffer, |buffer, start, end| {
        if matches!(
            syllable_kind(&buffer.info[start]),
            syllable::VIRAMA_TERMINATED
                | syllable::SAKOT_TERMINATED
                | syllable::STANDARD
                | syllable::BROKEN
        ) {
            move_repha_forward(buffer, start, end);
            move_pre_base_vowels_back(buffer, start, end);
        }
    });
    inserted
}

fn move_repha_forward(buffer: &mut Buffer, start: usize, end: usize) {
    if buffer.info[start].shaper_category != cat::R || end - start <= 1 {
        return;
    }
    for index in start + 1..end {
        let info = &buffer.info[index];
        let post_base = has_category(info, POST_BASE) || is_halant(info);
        if post_base || index == end - 1 {
            let target = if post_base { index - 1 } else { index };
            buffer.merge_clusters(start, target + 1);
            buffer.info[start..=target].rotate_left(1);
            return;
        }
    }
}

fn move_pre_base_vowels_back(buffer: &mut Buffer, start: usize, end: usize) {
    let mut target = start;
    for index in start..end {
        let info = &buffer.info[index];
        if is_halant(info) {
            target = index + 1;
        } else if has_category(info, PRE_BASE_VOWELS) && info.lig_comp() == 0 && target < index {
            buffer.merge_clusters(target, index + 1);
            buffer.info[target..=index].rotate_right(1);
        }
    }
}
