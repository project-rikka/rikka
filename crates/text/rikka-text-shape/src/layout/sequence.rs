use rikka_text_font::{Array, SequenceLookupRecord};
use rikka_text_unicode::GeneralCategory;

use crate::buffer::glyph_info::glyph_props;
use crate::layout::apply_context::{ApplyContext, MAX_CONTEXT_LENGTH};
use crate::layout::matching::{GlyphPredicate, Skip};

pub(crate) type MatchPositions = [u32; MAX_CONTEXT_LENGTH];

pub(crate) struct InputMatch {
    pub(crate) end: usize,
    pub(crate) total_component_count: u8,
}

pub(crate) fn match_input(
    ctx: &ApplyContext<'_>,
    input_count: usize,
    predicate: &impl GlyphPredicate,
    positions: &mut MatchPositions,
) -> Result<InputMatch, usize> {
    let count = input_count + 1;
    if count == 1 {
        positions[0] = ctx.buffer.idx as u32;
        return Ok(InputMatch {
            end: ctx.buffer.idx + 1,
            total_component_count: ctx.buffer.cur(0).lig_num_comps(),
        });
    }
    if count > MAX_CONTEXT_LENGTH {
        return Err(ctx.buffer.idx);
    }
    let mut iterator = ctx.input_iterator();
    iterator.reset(ctx.buffer, ctx.buffer.idx);
    let first = *ctx.buffer.cur(0);
    let first_lig_id = first.lig_id();
    let first_lig_comp = first.lig_comp();
    let mut total_component_count: u8 = 0;
    let mut base_may_skip: Option<bool> = None;
    for position in &mut positions[1..count] {
        let mut unsafe_to = 0;
        if !iterator.next(ctx.buffer, ctx.font, predicate, &mut unsafe_to) {
            return Err(unsafe_to);
        }
        *position = iterator.index as u32;
        let this = ctx.buffer.info[iterator.index];
        let this_lig_id = this.lig_id();
        let this_lig_comp = this.lig_comp();
        if first_lig_id != 0 && first_lig_comp != 0 {
            if first_lig_id != this_lig_id || first_lig_comp != this_lig_comp {
                let may_skip = *base_may_skip.get_or_insert_with(|| {
                    let out = ctx.buffer.out_info();
                    let mut index = ctx.buffer.out_len;
                    let mut found = false;
                    while index > 0 && out[index - 1].lig_id() == first_lig_id {
                        index -= 1;
                        if out[index].lig_comp() == 0 {
                            found = true;
                            break;
                        }
                    }
                    found && iterator.may_skip(ctx.font, &out[index]) == Skip::Yes
                });
                if !may_skip {
                    return Err(0);
                }
            }
        } else if this_lig_id != 0 && this_lig_comp != 0 && this_lig_id != first_lig_id {
            return Err(0);
        }
        total_component_count = total_component_count.wrapping_add(this.lig_num_comps());
    }
    positions[0] = ctx.buffer.idx as u32;
    Ok(InputMatch {
        end: iterator.index + 1,
        total_component_count: total_component_count.wrapping_add(first.lig_num_comps()),
    })
}

pub(crate) fn match_backtrack(
    ctx: &ApplyContext<'_>,
    count: usize,
    predicate: &impl GlyphPredicate,
) -> Result<usize, usize> {
    if count == 0 {
        return Ok(ctx.buffer.backtrack_len());
    }
    let mut iterator = ctx.context_iterator();
    iterator.reset_back(ctx.buffer, ctx.buffer.backtrack_len());
    for _ in 0..count {
        let mut unsafe_from = 0;
        if !iterator.prev(ctx.buffer, ctx.font, predicate, &mut unsafe_from) {
            return Err(unsafe_from);
        }
    }
    Ok(iterator.index)
}

pub(crate) fn match_lookahead(
    ctx: &ApplyContext<'_>,
    count: usize,
    predicate: &impl GlyphPredicate,
    start: usize,
) -> Result<usize, usize> {
    if count == 0 {
        return Ok(start);
    }
    let mut iterator = ctx.context_iterator();
    iterator.reset(ctx.buffer, start - 1);
    for _ in 0..count {
        let mut unsafe_to = 0;
        if !iterator.next(ctx.buffer, ctx.font, predicate, &mut unsafe_to) {
            return Err(unsafe_to);
        }
    }
    Ok(iterator.index + 1)
}

pub(crate) fn ligate_input(
    ctx: &mut ApplyContext<'_>,
    positions: &[u32],
    match_end: usize,
    total_component_count: u8,
    ligature: u32,
) {
    ctx.buffer.merge_clusters(ctx.buffer.idx, match_end);
    let mut is_base_ligature = ctx.buffer.info[positions[0] as usize].is_base_glyph();
    let mut is_mark_ligature = ctx.buffer.info[positions[0] as usize].is_mark();
    for &position in &positions[1..] {
        if !ctx.buffer.info[position as usize].is_mark() {
            is_base_ligature = false;
            is_mark_ligature = false;
        }
    }
    let is_ligature = !is_base_ligature && !is_mark_ligature;
    let class = if is_ligature {
        glyph_props::LIGATURE
    } else {
        0
    };
    let lig_id = if is_ligature {
        ctx.buffer.allocate_lig_id()
    } else {
        0
    };
    let first = ctx.buffer.cur_mut(0);
    let mut last_lig_id = first.lig_id();
    let mut last_num_comps = first.lig_num_comps();
    let mut comps_so_far = last_num_comps;
    if is_ligature {
        first.set_lig_props_for_ligature(lig_id, total_component_count);
        if first.general_category() == GeneralCategory::NonspacingMark {
            first.set_general_category(GeneralCategory::OtherLetter);
        }
    }
    ctx.replace_glyph_with_ligature(ligature, class);
    for &position in &positions[1..] {
        while ctx.buffer.idx < position as usize && ctx.buffer.successful {
            if is_ligature {
                let current = ctx.buffer.cur_mut(0);
                let mut this_comp = current.lig_comp();
                if this_comp == 0 {
                    this_comp = last_num_comps;
                }
                let new_comp = comps_so_far
                    .wrapping_sub(last_num_comps)
                    .wrapping_add(this_comp.min(last_num_comps));
                current.set_lig_props_for_mark(lig_id, new_comp);
            }
            ctx.buffer.next_glyph();
        }
        let current = ctx.buffer.cur(0);
        last_lig_id = current.lig_id();
        last_num_comps = current.lig_num_comps();
        comps_so_far = comps_so_far.wrapping_add(last_num_comps);
        ctx.buffer.idx += 1;
    }
    if !is_mark_ligature && last_lig_id != 0 {
        for index in ctx.buffer.idx..ctx.buffer.len {
            let info = &mut ctx.buffer.info[index];
            if last_lig_id != info.lig_id() {
                break;
            }
            let this_comp = info.lig_comp();
            if this_comp == 0 {
                break;
            }
            let new_comp = comps_so_far
                .wrapping_sub(last_num_comps)
                .wrapping_add(this_comp.min(last_num_comps));
            info.set_lig_props_for_mark(lig_id, new_comp);
        }
    }
}

pub(crate) fn apply_nested_lookups(
    ctx: &mut ApplyContext<'_>,
    positions: &mut MatchPositions,
    input_count: usize,
    match_end: usize,
    records: Array<'_, SequenceLookupRecord>,
) {
    let mut count = input_count + 1;
    let backtrack_len = ctx.buffer.backtrack_len();
    let delta = backtrack_len as isize - ctx.buffer.idx as isize;
    for position in &mut positions[..count] {
        *position = (*position as isize + delta) as u32;
    }
    let mut end = backtrack_len as isize + match_end as isize - ctx.buffer.idx as isize;
    for record in records {
        if !ctx.buffer.successful {
            break;
        }
        let index = usize::from(record.sequence_index);
        if index >= count {
            continue;
        }
        let original_len = ctx.buffer.backtrack_len() + ctx.buffer.lookahead_len();
        if positions[index] as usize >= original_len {
            continue;
        }
        if !ctx.buffer.move_to(positions[index] as usize) {
            break;
        }
        if ctx.buffer.max_ops <= 0 {
            break;
        }
        if !ctx.recurse(record.lookup_index) {
            continue;
        }
        let new_len = ctx.buffer.backtrack_len() + ctx.buffer.lookahead_len();
        let mut delta = new_len as isize - original_len as isize;
        if delta == 0 {
            continue;
        }
        end += delta;
        let current = positions[index] as isize;
        if end < current {
            delta += current - end;
            end = current;
        }
        let mut next = index + 1;
        if delta > 0 {
            if delta as usize + count > MAX_CONTEXT_LENGTH {
                break;
            }
        } else {
            delta = delta.max(next as isize - count as isize);
            next = (next as isize - delta) as usize;
        }
        let target = (next as isize + delta) as usize;
        positions.copy_within(next..count, target);
        next = target;
        count = (count as isize + delta) as usize;
        for position in index + 1..next {
            positions[position] = positions[position - 1] + 1;
        }
        while next < count {
            positions[next] = (positions[next] as isize + delta) as u32;
            next += 1;
        }
    }
    ctx.buffer.move_to(end.max(0) as usize);
}
