use rikka_text_unicode::GeneralCategory;

use crate::buffer::glyph_buffer::Buffer;
use crate::buffer::glyph_info::glyph_props;
use crate::face::font_context::FontContext;
use crate::layout::dispatch::apply_layout_table;
use crate::pipeline::fallback_marks::recategorize_marks;
use crate::pipeline::masks::{mirror_characters, setup_masks};
use crate::pipeline::normalize::normalize;
use crate::plan::feature_map::TableKind;
use crate::plan::shape_plan::PlanCore;

pub(crate) fn substitute_default(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    mirror_characters(plan, font, buffer);
    normalize(plan, font, buffer);
    setup_masks(plan, font, buffer);
    if plan.fallback_mark_positioning {
        recategorize_marks(buffer);
    }
    for info in &mut buffer.info[..buffer.len] {
        info.codepoint = u32::from(info.nominal_glyph);
    }
}

pub(crate) fn substitute_by_plan(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) {
    for info in &mut buffer.info[..buffer.len] {
        info.glyph_props = font.glyph_props(info.codepoint);
        info.lig_props = 0;
    }
    if plan.fallback_glyph_classes {
        synthesize_glyph_classes(buffer);
    }
    buffer.update_digest();
    apply_layout_table(plan, font, buffer, TableKind::Gsub);
}

fn synthesize_glyph_classes(buffer: &mut Buffer) {
    for info in &mut buffer.info[..buffer.len] {
        info.glyph_props = if info.general_category() != GeneralCategory::NonspacingMark
            || info.is_default_ignorable()
        {
            glyph_props::BASE_GLYPH
        } else {
            glyph_props::MARK
        };
    }
}
