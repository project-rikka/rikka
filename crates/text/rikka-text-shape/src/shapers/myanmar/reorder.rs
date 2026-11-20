use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::indic::classes::is_consonant;
use crate::shapers::indic::properties::{category as cat, position as pos};
use crate::shapers::myanmar::machine::syllable;
use crate::shapers::syllabic::dotted_circles::DottedCircle;
use crate::shapers::syllabic::syllables::{for_each_syllable, syllable_kind};

const KINZI_LENGTH: usize = 3;

const DOTTED_CIRCLE: DottedCircle = DottedCircle {
    broken: syllable::BROKEN,
    category: cat::DOTTED_CIRCLE,
    position: pos::START,
    after_repha: None,
};

pub(crate) fn reorder(_: &PlanCore, font: &FontContext, buffer: &mut Buffer) -> bool {
    let inserted = DOTTED_CIRCLE.insert(font, buffer);
    for_each_syllable(buffer, |buffer, start, end| {
        if matches!(
            syllable_kind(&buffer.info[start]),
            syllable::CONSONANT | syllable::BROKEN
        ) {
            reorder_syllable(buffer, start, end);
        }
    });
    inserted
}

fn starts_with_kinzi(buffer: &Buffer, start: usize, end: usize) -> bool {
    start + KINZI_LENGTH <= end
        && buffer.info[start].shaper_category == cat::RA
        && buffer.info[start + 1].shaper_category == cat::AS
        && buffer.info[start + 2].shaper_category == cat::H
}

fn assign_positions(buffer: &mut Buffer, start: usize, end: usize) {
    let has_kinzi = starts_with_kinzi(buffer, start, end);
    let limit = if has_kinzi {
        start + KINZI_LENGTH
    } else {
        start
    };
    let base = (limit..end)
        .find(|&index| is_consonant(&buffer.info[index]))
        .unwrap_or(start);
    let info = &mut buffer.info;
    let mut index = start;
    while index < limit {
        info[index].shaper_auxiliary = pos::AFTER_MAIN;
        index += 1;
    }
    while index < base {
        info[index].shaper_auxiliary = pos::PRE_C;
        index += 1;
    }
    if index < end {
        info[index].shaper_auxiliary = pos::BASE_C;
        index += 1;
    }
    let mut position = pos::AFTER_MAIN;
    for index in index..end {
        let category = info[index].shaper_category;
        info[index].shaper_auxiliary = match category {
            cat::MR => pos::PRE_C,
            cat::VPRE => pos::PRE_M,
            cat::VS => info[index - 1].shaper_auxiliary,
            cat::VBLW if position == pos::AFTER_MAIN => {
                position = pos::BELOW_C;
                position
            }
            cat::A if position == pos::BELOW_C => pos::BEFORE_SUB,
            cat::VBLW if position == pos::BELOW_C => position,
            _ if position == pos::BELOW_C => {
                position = pos::AFTER_SUB;
                position
            }
            _ => position,
        };
    }
}

fn flip_left_matras(buffer: &mut Buffer, start: usize, end: usize) {
    let mut first = end;
    let mut last = end;
    for (index, info) in (start..end).zip(&buffer.info[start..end]) {
        if info.shaper_auxiliary == pos::PRE_M {
            if first == end {
                first = index;
            }
            last = index;
        }
    }
    if first >= last {
        return;
    }
    buffer.reverse_range(first, last + 1);
    let mut run_start = first;
    for index in first..=last {
        if buffer.info[index].shaper_category == cat::VPRE {
            buffer.reverse_range(run_start, index + 1);
            run_start = index + 1;
        }
    }
}

fn reorder_syllable(buffer: &mut Buffer, start: usize, end: usize) {
    assign_positions(buffer, start, end);
    buffer.sort(start, end, |a, b| a.shaper_auxiliary > b.shaper_auxiliary);
    flip_left_matras(buffer, start, end);
}
