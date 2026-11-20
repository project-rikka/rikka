use rikka_text_unicode::Script;

use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::indic::classes::{
    JOINERS, MATRAS, flag, in_flags, is_consonant, is_joiner, is_one_of,
};
use crate::shapers::indic::config::{BelowFormsMode, RephMode};
use crate::shapers::indic::machine::syllable;
use crate::shapers::indic::plan::{IndicPlan, feature};
use crate::shapers::indic::properties::{category as cat, position as pos};
use crate::shapers::syllabic::dotted_circles::DottedCircle;
use crate::shapers::syllabic::syllables::{for_each_syllable, syllable_kind};

const LONG_SYLLABLE: usize = 127;
const VISITED: u8 = 255;
const PREF_LENGTH: usize = 2;

const DOTTED_CIRCLE: DottedCircle = DottedCircle {
    broken: syllable::BROKEN,
    category: cat::DOTTED_CIRCLE,
    position: pos::END,
    after_repha: Some(cat::REPHA),
};

const ATTACHED_TO_PREVIOUS: u64 =
    flag(cat::ZWJ) | flag(cat::ZWNJ) | flag(cat::N) | flag(cat::RS) | flag(cat::CM) | flag(cat::H);

pub(crate) fn initial_reordering(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) -> bool {
    let Some(indic) = plan.shaper_data::<IndicPlan>() else {
        return false;
    };
    update_consonant_positions(plan, indic, font, buffer);
    let inserted = DOTTED_CIRCLE.insert(font, buffer);
    for_each_syllable(buffer, |buffer, start, end| {
        if matches!(
            syllable_kind(&buffer.info[start]),
            syllable::CONSONANT | syllable::VOWEL | syllable::STANDALONE | syllable::BROKEN
        ) {
            reorder_consonant_syllable(plan, indic, font, buffer, start, end);
        }
    });
    inserted
}

fn update_consonant_positions(
    plan: &PlanCore,
    indic: &IndicPlan,
    font: &FontContext,
    buffer: &mut Buffer,
) {
    if indic.config.virama == 0 {
        return;
    }
    let Some(virama) = font.nominal_glyph(indic.config.virama) else {
        return;
    };
    let virama = u32::from(virama);
    for info in &mut buffer.info[..buffer.len] {
        if info.shaper_auxiliary == pos::BASE_C {
            let glyph = info.codepoint;
            info.shaper_auxiliary = indic.consonant_position(glyph, || {
                consonant_position(plan, indic, font, glyph, virama)
            });
        }
    }
}

fn consonant_position(
    plan: &PlanCore,
    indic: &IndicPlan,
    font: &FontContext,
    consonant: u32,
    virama: u32,
) -> u8 {
    let pair = |probe| indic.would_substitute_pair(plan, font, probe, virama, consonant);
    if pair(indic.blwf) || pair(indic.vatu) {
        pos::BELOW_C
    } else if pair(indic.pstf) || pair(indic.pref) {
        pos::POST_C
    } else {
        pos::BASE_C
    }
}

fn stable_sort_by_position(buffer: &mut Buffer, start: usize, end: usize) {
    for index in start + 1..end {
        let moved = buffer.info[index];
        let mut target = index;
        while target > start && buffer.info[target - 1].shaper_auxiliary > moved.shaper_auxiliary {
            buffer.info[target] = buffer.info[target - 1];
            target -= 1;
        }
        buffer.info[target] = moved;
    }
}

fn find_base(
    plan: &PlanCore,
    indic: &IndicPlan,
    font: &FontContext,
    buffer: &Buffer,
    start: usize,
    end: usize,
) -> (usize, bool) {
    let info = &buffer.info;
    let mode = indic.config.reph_mode;
    let mut base = end;
    let mut has_reph = false;
    let mut limit = start;
    if indic.masks[feature::RPHF] != 0
        && start + 3 <= end
        && ((mode == RephMode::Implicit && !is_joiner(&info[start + 2]))
            || (mode == RephMode::Explicit && info[start + 2].shaper_category == cat::ZWJ))
    {
        let glyphs = [
            info[start].codepoint,
            info[start + 1].codepoint,
            if mode == RephMode::Explicit {
                info[start + 2].codepoint
            } else {
                0
            },
        ];
        if indic.would_substitute(plan, font, indic.rphf, &glyphs[..2])
            || (mode == RephMode::Explicit
                && indic.would_substitute(plan, font, indic.rphf, &glyphs))
        {
            limit += 2;
            while limit < end && is_joiner(&info[limit]) {
                limit += 1;
            }
            base = start;
            has_reph = true;
        }
    } else if mode == RephMode::Logical && info[start].shaper_category == cat::REPHA {
        limit += 1;
        while limit < end && is_joiner(&info[limit]) {
            limit += 1;
        }
        base = start;
        has_reph = true;
    }

    let mut index = end;
    let mut seen_below = false;
    loop {
        index -= 1;
        let current = &info[index];
        if is_consonant(current) {
            let position = current.shaper_auxiliary;
            if position != pos::BELOW_C && (position != pos::POST_C || seen_below) {
                base = index;
                break;
            }
            if position == pos::BELOW_C {
                seen_below = true;
            }
            base = index;
        } else if start < index
            && current.shaper_category == cat::ZWJ
            && info[index - 1].shaper_category == cat::H
        {
            break;
        }
        if index <= limit {
            break;
        }
    }
    if has_reph && base == start && limit - base <= 2 {
        has_reph = false;
    }
    (base, has_reph)
}

fn move_old_spec_halant(buffer: &mut Buffer, base: usize, end: usize) {
    let disallow_double_halants = buffer.script == Script::Kannada;
    let Some(halant) = (base + 1..end).find(|&index| buffer.info[index].shaper_category == cat::H)
    else {
        return;
    };
    let mut last = end - 1;
    while last > halant {
        let info = &buffer.info[last];
        if is_consonant(info) || (disallow_double_halants && info.shaper_category == cat::H) {
            break;
        }
        last -= 1;
    }
    if buffer.info[last].shaper_category != cat::H && last > halant {
        buffer.info[halant..=last].rotate_left(1);
    }
}

fn attach_marks_to_previous(buffer: &mut Buffer, start: usize, end: usize) {
    let info = &mut buffer.info;
    let mut last_position = pos::START;
    for index in start..end {
        if in_flags(info[index].shaper_category, ATTACHED_TO_PREVIOUS) {
            info[index].shaper_auxiliary = last_position;
            if info[index].shaper_category == cat::H
                && info[index].shaper_auxiliary == pos::PRE_M
                && let Some(previous) = (start..index)
                    .rev()
                    .find(|&previous| info[previous].shaper_auxiliary != pos::PRE_M)
            {
                info[index].shaper_auxiliary = info[previous].shaper_auxiliary;
            }
        } else if info[index].shaper_auxiliary != pos::SMVD {
            if info[index].shaper_category == cat::MPST
                && index > start
                && info[index - 1].shaper_category == cat::SM
            {
                info[index - 1].shaper_auxiliary = info[index].shaper_auxiliary;
            }
            last_position = info[index].shaper_auxiliary;
        }
    }
}

fn attach_marks_to_post_base_consonants(buffer: &mut Buffer, base: usize, end: usize) {
    let info = &mut buffer.info;
    let mut last = base;
    for index in base + 1..end {
        if is_consonant(&info[index]) {
            let position = info[index].shaper_auxiliary;
            for mark in &mut info[last + 1..index] {
                if mark.shaper_auxiliary < pos::SMVD {
                    mark.shaper_auxiliary = position;
                }
            }
            last = index;
        } else if in_flags(info[index].shaper_category, MATRAS) {
            last = index;
        }
    }
}

fn sort_syllable(indic: &IndicPlan, buffer: &mut Buffer, start: usize, end: usize) -> usize {
    let syllable = buffer.info[start].syllable;
    for (offset, info) in buffer.info[start..end].iter_mut().enumerate() {
        info.syllable = offset as u8;
    }
    stable_sort_by_position(buffer, start, end);

    let mut base = end;
    let mut first_left_matra = end;
    let mut last_left_matra = end;
    for (index, info) in (start..end).zip(&buffer.info[start..end]) {
        let position = info.shaper_auxiliary;
        if position == pos::BASE_C {
            base = index;
            break;
        }
        if position == pos::PRE_M {
            if first_left_matra == end {
                first_left_matra = index;
            }
            last_left_matra = index;
        }
    }
    if first_left_matra < last_left_matra {
        buffer.reverse_range(first_left_matra, last_left_matra + 1);
        let mut run_start = first_left_matra;
        for index in first_left_matra..=last_left_matra {
            if in_flags(buffer.info[index].shaper_category, MATRAS) {
                buffer.reverse_range(run_start, index + 1);
                run_start = index + 1;
            }
        }
    }

    if indic.is_old_spec || end - start > LONG_SYLLABLE {
        buffer.merge_clusters(base, end);
    } else {
        for index in base..end {
            if buffer.info[index].syllable == VISITED {
                continue;
            }
            let (mut low, mut high) = (index, index);
            let mut cursor = start + usize::from(buffer.info[index].syllable);
            while cursor != index {
                low = low.min(cursor);
                high = high.max(cursor);
                let next = start + usize::from(buffer.info[cursor].syllable);
                buffer.info[cursor].syllable = VISITED;
                cursor = next;
            }
            buffer.merge_clusters(base.max(low), high + 1);
        }
    }
    for info in &mut buffer.info[start..end] {
        info.syllable = syllable;
    }
    base
}

fn setup_syllable_masks(
    indic: &IndicPlan,
    buffer: &mut Buffer,
    start: usize,
    base: usize,
    end: usize,
) {
    let masks = &indic.masks;
    for info in &mut buffer.info[start..end] {
        if info.shaper_auxiliary != pos::RA_TO_BECOME_REPH {
            break;
        }
        info.mask |= masks[feature::RPHF];
    }
    let mut pre_base = masks[feature::HALF];
    if !indic.is_old_spec && indic.config.below_forms == BelowFormsMode::PreAndPost {
        pre_base |= masks[feature::BLWF];
    }
    for info in &mut buffer.info[start..base.min(end)] {
        info.mask |= pre_base;
    }
    let post_base = masks[feature::BLWF] | masks[feature::ABVF] | masks[feature::PSTF];
    if base < end {
        for info in &mut buffer.info[base + 1..end] {
            info.mask |= post_base;
        }
    }

    if indic.is_old_spec && buffer.script == Script::Devanagari {
        for index in start..base.saturating_sub(1) {
            let info = &buffer.info;
            if info[index].shaper_category == cat::RA
                && info[index + 1].shaper_category == cat::H
                && (index + 2 == base || info[index + 2].shaper_category != cat::ZWJ)
            {
                buffer.info[index].mask |= masks[feature::BLWF];
                buffer.info[index + 1].mask |= masks[feature::BLWF];
            }
        }
    }
}

fn mark_pre_base_reordering(
    plan: &PlanCore,
    indic: &IndicPlan,
    font: &FontContext,
    buffer: &mut Buffer,
    base: usize,
    end: usize,
) {
    let mask = indic.masks[feature::PREF];
    if mask == 0 || base + PREF_LENGTH >= end {
        return;
    }
    for index in base + 1..end - PREF_LENGTH + 1 {
        let glyphs = [
            buffer.info[index].codepoint,
            buffer.info[index + 1].codepoint,
        ];
        if indic.would_substitute(plan, font, indic.pref, &glyphs) {
            buffer.info[index].mask |= mask;
            buffer.info[index + 1].mask |= mask;
            return;
        }
    }
}

fn apply_joiners(indic: &IndicPlan, buffer: &mut Buffer, start: usize, end: usize) {
    let half = indic.masks[feature::HALF];
    for index in start + 1..end {
        if !is_one_of(&buffer.info[index], JOINERS) {
            continue;
        }
        let non_joiner = buffer.info[index].shaper_category == cat::ZWNJ;
        let mut previous = index;
        loop {
            previous -= 1;
            if non_joiner {
                buffer.info[previous].mask &= !half;
            }
            if previous <= start || is_consonant(&buffer.info[previous]) {
                break;
            }
        }
    }
}

fn reorder_consonant_syllable(
    plan: &PlanCore,
    indic: &IndicPlan,
    font: &FontContext,
    buffer: &mut Buffer,
    start: usize,
    end: usize,
) {
    if buffer.script == Script::Kannada
        && start + 3 <= end
        && is_one_of(&buffer.info[start], flag(cat::RA))
        && is_one_of(&buffer.info[start + 1], flag(cat::H))
        && is_one_of(&buffer.info[start + 2], flag(cat::ZWJ))
    {
        buffer.merge_clusters(start + 1, start + 3);
        buffer.info.swap(start + 1, start + 2);
    }

    let (base, has_reph) = find_base(plan, indic, font, buffer, start, end);
    for info in &mut buffer.info[start..base] {
        info.shaper_auxiliary = info.shaper_auxiliary.min(pos::PRE_C);
    }
    if base < end {
        buffer.info[base].shaper_auxiliary = pos::BASE_C;
    }
    if has_reph {
        buffer.info[start].shaper_auxiliary = pos::RA_TO_BECOME_REPH;
    }
    if indic.is_old_spec {
        move_old_spec_halant(buffer, base, end);
    }
    attach_marks_to_previous(buffer, start, end);
    attach_marks_to_post_base_consonants(buffer, base, end);
    let base = sort_syllable(indic, buffer, start, end);
    setup_syllable_masks(indic, buffer, start, base, end);
    mark_pre_base_reordering(plan, indic, font, buffer, base, end);
    apply_joiners(indic, buffer, start, end);
}
