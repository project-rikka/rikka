use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::indic::properties::{category as cat, position as pos};
use crate::shapers::khmer::machine::syllable;
use crate::shapers::khmer::shaper::KhmerPlan;
use crate::shapers::syllabic::dotted_circles::DottedCircle;
use crate::shapers::syllabic::syllables::{for_each_syllable, syllable_kind};

const MAX_COENGS: usize = 2;

const DOTTED_CIRCLE: DottedCircle = DottedCircle {
    broken: syllable::BROKEN,
    category: cat::DOTTED_CIRCLE,
    position: pos::START,
    after_repha: Some(cat::REPHA),
};

pub(crate) fn reorder(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) -> bool {
    let inserted = DOTTED_CIRCLE.insert(font, buffer);
    let Some(khmer) = plan.shaper_data::<KhmerPlan>() else {
        return inserted;
    };
    for_each_syllable(buffer, |buffer, start, end| {
        if matches!(
            syllable_kind(&buffer.info[start]),
            syllable::CONSONANT | syllable::BROKEN
        ) {
            reorder_syllable(khmer, buffer, start, end);
        }
    });
    inserted
}

fn reorder_syllable(khmer: &KhmerPlan, buffer: &mut Buffer, start: usize, end: usize) {
    let post_base = khmer.below | khmer.above | khmer.post;
    for info in &mut buffer.info[start + 1..end] {
        info.mask |= post_base;
    }
    let mut coengs = 0;
    for index in start + 1..end {
        let category = buffer.info[index].shaper_category;
        if category == cat::H && coengs <= MAX_COENGS && index + 1 < end {
            coengs += 1;
            if buffer.info[index + 1].shaper_category == cat::RA {
                buffer.info[index].mask |= khmer.pre_base;
                buffer.info[index + 1].mask |= khmer.pre_base;
                buffer.merge_clusters(start, index + 2);
                buffer.info[start..index + 2].rotate_right(2);
                if khmer.conjunct_after_ro != 0 {
                    for info in &mut buffer.info[index + 2..end] {
                        info.mask |= khmer.conjunct_after_ro;
                    }
                }
                coengs = MAX_COENGS;
            }
        } else if category == cat::VPRE {
            buffer.merge_clusters(start, index + 1);
            buffer.info[start..=index].rotate_right(1);
        }
    }
}
