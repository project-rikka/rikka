use rikka_text_unicode::GeneralCategory;

use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::scratch_flags;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;
use crate::unicode::to_char;

const FRACTION_SLASH: u32 = 0x2044;

pub(crate) fn mirror_characters(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    if !plan.properties.direction.is_backward() {
        return;
    }
    let rtlm_mask = plan.rtlm_mask;
    for info in &mut buffer.info[..buffer.len] {
        if let Some(mirrored) = rikka_text_unicode::mirroring_glyph(to_char(info.codepoint))
            && font.has_glyph(mirrored as u32)
        {
            info.codepoint = mirrored as u32;
            continue;
        }
        info.mask |= rtlm_mask;
    }
}

pub(crate) fn setup_masks(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    setup_fraction_masks(plan, buffer);
    plan.shaper.setup_masks(plan, font, buffer);
    for feature in plan.features.iter().filter(|feature| !feature.is_global()) {
        let (mask, shift) = plan.map.mask(feature.tag);
        buffer.set_masks(feature.value << shift, mask, feature.start, feature.end);
    }
}

fn setup_fraction_masks(plan: &PlanCore, buffer: &mut Buffer) {
    if buffer.scratch_flags & scratch_flags::HAS_FRACTION_SLASH == 0 || !plan.has_frac {
        return;
    }
    let (pre_mask, post_mask) = if buffer.direction.is_forward() {
        (
            plan.numr_mask | plan.frac_mask,
            plan.frac_mask | plan.dnom_mask,
        )
    } else {
        (
            plan.frac_mask | plan.dnom_mask,
            plan.numr_mask | plan.frac_mask,
        )
    };
    let len = buffer.len;
    let is_digit = |buffer: &Buffer, index: usize| {
        buffer.info[index].general_category() == GeneralCategory::DecimalNumber
    };
    let mut index = 0;
    while index < len {
        if buffer.info[index].codepoint != FRACTION_SLASH {
            index += 1;
            continue;
        }
        let mut start = index;
        while start > 0 && is_digit(buffer, start - 1) {
            start -= 1;
        }
        let mut end = index + 1;
        while end < len && is_digit(buffer, end) {
            end += 1;
        }
        if start == index || end == index + 1 {
            if start == index {
                buffer.unsafe_to_concat(start, start + 1);
            }
            if end == index + 1 {
                buffer.unsafe_to_concat(end - 1, end);
            }
            index += 1;
            continue;
        }
        buffer.unsafe_to_break(start, end);
        for info in &mut buffer.info[start..index] {
            info.mask |= pre_mask;
        }
        buffer.info[index].mask |= plan.frac_mask;
        for info in &mut buffer.info[index + 1..end] {
            info.mask |= post_mask;
        }
        index = end;
    }
}
