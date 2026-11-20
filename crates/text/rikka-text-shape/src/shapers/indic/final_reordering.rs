use rikka_text_unicode::{GeneralCategory, Script};

use crate::buffer::glyph_buffer::Buffer;
use crate::face::font_context::FontContext;
use crate::plan::shape_plan::PlanCore;
use crate::shapers::indic::classes::{
    MATRAS, flag, in_flags, is_consonant, is_halant, is_joiner, is_one_of,
};
use crate::shapers::indic::config::RephPosition;
use crate::shapers::indic::plan::{IndicPlan, feature};
use crate::shapers::indic::properties::{category as cat, position as pos};
use crate::shapers::syllabic::syllables::for_each_syllable;

const MATRAS_OR_HALANT: u64 = MATRAS | flag(cat::H);
const AFTER_SUB_STOPS: u64 = flag(pos::POST_C) | flag(pos::AFTER_POST) | flag(pos::SMVD);

pub(crate) fn final_reordering(plan: &PlanCore, font: &FontContext, buffer: &mut Buffer) -> bool {
    if buffer.len == 0 {
        return false;
    }
    let Some(indic) = plan.shaper_data::<IndicPlan>() else {
        return false;
    };
    let virama = (indic.config.virama != 0)
        .then(|| font.nominal_glyph(indic.config.virama))
        .flatten()
        .map(u32::from);
    for_each_syllable(buffer, |buffer, start, end| {
        reorder_syllable(indic, virama, buffer, start, end);
    });
    false
}

fn restore_halants(virama: Option<u32>, buffer: &mut Buffer, start: usize, end: usize) {
    let Some(virama) = virama else {
        return;
    };
    for info in &mut buffer.info[start..end] {
        if info.codepoint == virama && info.ligated() && info.multiplied() {
            info.shaper_category = cat::H;
            info.clear_ligated_and_multiplied();
        }
    }
}

fn has_script_without_half_forms(buffer: &Buffer) -> bool {
    matches!(buffer.script, Script::Malayalam | Script::Tamil)
}

fn find_base(indic: &IndicPlan, buffer: &mut Buffer, start: usize, end: usize) -> (usize, bool) {
    let pref = indic.masks[feature::PREF];
    let mut try_pref = pref != 0;
    let mut base = start;
    while base < end {
        if buffer.info[base].shaper_auxiliary < pos::BASE_C {
            base += 1;
            continue;
        }
        if try_pref
            && base + 1 < end
            && let Some(candidate) =
                (base + 1..end).find(|&index| buffer.info[index].mask & pref != 0)
        {
            let info = &buffer.info[candidate];
            if !(info.substituted() && info.ligated_and_didnt_multiply()) {
                base = candidate;
                while base < end && is_halant(&buffer.info[base]) {
                    base += 1;
                }
                if base < end {
                    buffer.info[base].shaper_auxiliary = pos::BASE_C;
                }
                try_pref = false;
            }
        }
        if buffer.script == Script::Malayalam {
            let mut index = base + 1;
            while index < end {
                while index < end && is_joiner(&buffer.info[index]) {
                    index += 1;
                }
                if index == end || !is_halant(&buffer.info[index]) {
                    break;
                }
                index += 1;
                while index < end && is_joiner(&buffer.info[index]) {
                    index += 1;
                }
                if index < end
                    && is_consonant(&buffer.info[index])
                    && buffer.info[index].shaper_auxiliary == pos::BELOW_C
                {
                    base = index;
                    buffer.info[base].shaper_auxiliary = pos::BASE_C;
                }
                index += 1;
            }
        }
        if start < base && base < end && buffer.info[base].shaper_auxiliary > pos::BASE_C {
            base -= 1;
        }
        break;
    }
    if base == end && start < base && is_one_of(&buffer.info[base - 1], flag(cat::ZWJ)) {
        base -= 1;
    }
    if base < end {
        while start < base && is_one_of(&buffer.info[base], flag(cat::N) | flag(cat::H)) {
            base -= 1;
        }
    }
    (base, try_pref)
}

fn reorder_pre_base_matras(
    buffer: &mut Buffer,
    start: usize,
    mut base: usize,
    end: usize,
) -> usize {
    if start + 1 >= end || start >= base {
        return base;
    }
    let mut new_position = if base == end { base - 2 } else { base - 1 };
    if !has_script_without_half_forms(buffer) {
        loop {
            while new_position > start && !is_one_of(&buffer.info[new_position], MATRAS_OR_HALANT) {
                new_position -= 1;
            }
            let info = &buffer.info[new_position];
            if is_halant(info) && info.shaper_auxiliary != pos::PRE_M {
                if new_position + 1 < end
                    && buffer.info[new_position + 1].shaper_category == cat::ZWJ
                    && new_position > start
                {
                    new_position -= 1;
                    continue;
                }
            } else {
                new_position = start;
            }
            break;
        }
    }
    if start < new_position && buffer.info[new_position].shaper_auxiliary != pos::PRE_M {
        for index in (start + 1..=new_position).rev() {
            if buffer.info[index - 1].shaper_auxiliary != pos::PRE_M {
                continue;
            }
            let old_position = index - 1;
            if old_position < base && base <= new_position {
                base -= 1;
            }
            buffer.info[old_position..=new_position].rotate_left(1);
            buffer.merge_clusters(new_position, end.min(base + 1));
            new_position -= 1;
        }
    } else if let Some(matra) =
        (start..base).find(|&index| buffer.info[index].shaper_auxiliary == pos::PRE_M)
    {
        buffer.merge_clusters(matra, end.min(base + 1));
    }
    base
}

fn halant_target(buffer: &Buffer, start: usize, base: usize) -> Option<usize> {
    let mut position = start + 1;
    while position < base && !is_halant(&buffer.info[position]) {
        position += 1;
    }
    if position >= base {
        return None;
    }
    if position + 1 < base && is_joiner(&buffer.info[position + 1]) {
        position += 1;
    }
    Some(position)
}

fn reph_target(indic: &IndicPlan, buffer: &Buffer, start: usize, base: usize, end: usize) -> usize {
    let reph_position = indic.config.reph_position;
    if reph_position != RephPosition::AfterPost {
        if let Some(target) = halant_target(buffer, start, base) {
            return target;
        }
        if reph_position == RephPosition::AfterMain {
            let mut target = base;
            while target + 1 < end && buffer.info[target + 1].shaper_auxiliary <= pos::AFTER_MAIN {
                target += 1;
            }
            if target < end {
                return target;
            }
        }
        if reph_position == RephPosition::AfterSub {
            let mut target = base;
            while target + 1 < end
                && !in_flags(buffer.info[target + 1].shaper_auxiliary, AFTER_SUB_STOPS)
            {
                target += 1;
            }
            if target < end {
                return target;
            }
        }
    }
    if let Some(target) = halant_target(buffer, start, base) {
        return target;
    }
    let mut target = end - 1;
    while target > start && buffer.info[target].shaper_auxiliary == pos::SMVD {
        target -= 1;
    }
    if is_halant(&buffer.info[target]) {
        let matras = (base + 1..target)
            .filter(|&index| in_flags(buffer.info[index].shaper_category, MATRAS))
            .count();
        target -= matras;
    }
    target
}

fn reorder_reph(
    indic: &IndicPlan,
    buffer: &mut Buffer,
    start: usize,
    mut base: usize,
    end: usize,
) -> usize {
    let first = &buffer.info[start];
    if start + 1 >= end
        || first.shaper_auxiliary != pos::RA_TO_BECOME_REPH
        || (first.shaper_category == cat::REPHA) == first.ligated_and_didnt_multiply()
    {
        return base;
    }
    let target = reph_target(indic, buffer, start, base, end);
    buffer.merge_clusters(start, target + 1);
    buffer.info[start..=target].rotate_left(1);
    if start < base && base <= target {
        base -= 1;
    }
    base
}

fn reorder_pre_base_consonant(
    indic: &IndicPlan,
    buffer: &mut Buffer,
    start: usize,
    base: usize,
    end: usize,
) {
    if base + 1 >= end {
        return;
    }
    let pref = indic.masks[feature::PREF];
    let Some(candidate) = (base + 1..end).find(|&index| buffer.info[index].mask & pref != 0) else {
        return;
    };
    if !buffer.info[candidate].ligated_and_didnt_multiply() {
        return;
    }
    let mut new_position = base;
    if !has_script_without_half_forms(buffer) {
        while new_position > start && !is_one_of(&buffer.info[new_position - 1], MATRAS_OR_HALANT) {
            new_position -= 1;
        }
    }
    if new_position > start
        && is_halant(&buffer.info[new_position - 1])
        && new_position < end
        && is_joiner(&buffer.info[new_position])
    {
        new_position += 1;
    }
    buffer.merge_clusters(new_position, candidate + 1);
    buffer.info[new_position..=candidate].rotate_right(1);
}

fn is_word_continuation(category: GeneralCategory) -> bool {
    matches!(
        category,
        GeneralCategory::Format
            | GeneralCategory::Unassigned
            | GeneralCategory::PrivateUse
            | GeneralCategory::Surrogate
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::UppercaseLetter
            | GeneralCategory::SpacingMark
            | GeneralCategory::EnclosingMark
            | GeneralCategory::NonspacingMark
    )
}

fn apply_init(indic: &IndicPlan, buffer: &mut Buffer, start: usize) {
    if buffer.info[start].shaper_auxiliary != pos::PRE_M {
        return;
    }
    if start == 0 || !is_word_continuation(buffer.info[start - 1].general_category()) {
        buffer.info[start].mask |= indic.masks[feature::INIT];
    } else {
        buffer.unsafe_to_break(start - 1, start + 1);
    }
}

fn reorder_syllable(
    indic: &IndicPlan,
    virama: Option<u32>,
    buffer: &mut Buffer,
    start: usize,
    end: usize,
) {
    restore_halants(virama, buffer, start, end);
    let (base, try_pref) = find_base(indic, buffer, start, end);
    let base = reorder_pre_base_matras(buffer, start, base, end);
    let base = reorder_reph(indic, buffer, start, base, end);
    if try_pref {
        reorder_pre_base_consonant(indic, buffer, start, base, end);
    }
    apply_init(indic, buffer, start);
}
