use rikka_text_font::{
    AlternateSubst, Array, ChainedSequenceContext, GlyphId, LigatureSet, LigatureSubst,
    LookupSubtable, MultipleSubst, ReverseChainSingleSubst, SequenceContext, SingleSubst,
    SubstSubtable,
};

use crate::buffer::glyph_info::glyph_props;
use crate::face::digest::Digest;
use crate::face::lookup_accel::{LookupAccel, SubtableAccel};
use crate::layout::apply_context::{ApplyContext, MAX_CONTEXT_LENGTH, MAX_NESTING_LEVEL};
use crate::layout::chain_context::{apply_chain_context, apply_chain_sequence};
use crate::layout::contextual::{apply_context, apply_context_sequence};
use crate::layout::matching::{EveryGlyph, Matches, Skip};
use crate::layout::sequence::{
    MatchPositions, ligate_input, match_backtrack, match_input, match_lookahead,
};
use crate::plan::map_builder::MAX_VALUE;

pub(crate) fn apply_subst_subtable(
    ctx: &mut ApplyContext<'_>,
    table: &[u8],
    lookup: &LookupAccel,
    accel: &SubtableAccel,
    coverage_index: u16,
) -> bool {
    let Some(location) = accel.location else {
        return false;
    };
    if let Some(sequence) = &accel.sequence {
        let records = || match SubstSubtable::from_location(table, location) {
            Some(SubstSubtable::Context(SequenceContext::Coverages(context))) => context.lookups(),
            Some(SubstSubtable::ChainContext(ChainedSequenceContext::Coverages(context))) => {
                context.lookups()
            }
            _ => Array::default(),
        };
        return if sequence.chained() {
            apply_chain_sequence(ctx, &lookup.sets, sequence, records)
        } else {
            apply_context_sequence(ctx, &lookup.sets, sequence, records)
        };
    }
    let Some(subtable) = SubstSubtable::from_location(table, location) else {
        return false;
    };
    match &subtable {
        SubstSubtable::Single(single) => apply_single(ctx, single, coverage_index),
        SubstSubtable::Multiple(multiple) => apply_multiple(ctx, multiple, coverage_index),
        SubstSubtable::Alternate(alternate) => apply_alternate(ctx, alternate, coverage_index),
        SubstSubtable::Ligature(ligature) => {
            apply_ligature_subst(ctx, ligature, &accel.seconds, coverage_index)
        }
        SubstSubtable::Context(context) => apply_context(ctx, context, accel, coverage_index),
        SubstSubtable::ChainContext(context) => {
            apply_chain_context(ctx, context, accel, coverage_index)
        }
        SubstSubtable::ReverseChainSingle(reverse) => {
            apply_reverse_chain(ctx, reverse, coverage_index)
        }
    }
}

#[inline]
fn current_glyph(ctx: &ApplyContext<'_>) -> GlyphId {
    GlyphId(ctx.buffer.cur(0).codepoint as u16)
}

fn apply_single(ctx: &mut ApplyContext<'_>, single: &SingleSubst<'_>, coverage_index: u16) -> bool {
    let Some(substitute) = single.substitute_at(coverage_index, current_glyph(ctx)) else {
        return false;
    };
    ctx.replace_glyph(u32::from(substitute.0));
    true
}

fn apply_multiple(
    ctx: &mut ApplyContext<'_>,
    multiple: &MultipleSubst<'_>,
    coverage_index: u16,
) -> bool {
    let Some(sequence) = multiple.sequence(coverage_index) else {
        return false;
    };
    match sequence.len() {
        0 => ctx.buffer.delete_glyph(),
        1 => {
            let Some(glyph) = sequence.first() else {
                return false;
            };
            ctx.replace_glyph(u32::from(glyph.0));
        }
        _ => {
            let class = if ctx.buffer.cur(0).is_ligature() {
                glyph_props::BASE_GLYPH
            } else {
                0
            };
            let lig_id = ctx.buffer.cur(0).lig_id();
            for (component, glyph) in sequence.iter().enumerate() {
                if lig_id == 0 {
                    ctx.buffer
                        .cur_mut(0)
                        .set_lig_props_for_component(component as u8);
                }
                ctx.output_glyph_for_component(u32::from(glyph.0), class);
            }
            ctx.buffer.skip_glyph();
        }
    }
    true
}

fn apply_alternate(
    ctx: &mut ApplyContext<'_>,
    alternate: &AlternateSubst<'_>,
    coverage_index: u16,
) -> bool {
    let Some(alternates) = alternate.alternate_set(coverage_index) else {
        return false;
    };
    let count = alternates.len() as u32;
    if count == 0 {
        return false;
    }
    let glyph_mask = ctx.buffer.cur(0).mask;
    let shift = ctx.lookup_mask.trailing_zeros();
    let mut index = (ctx.lookup_mask & glyph_mask) >> shift;
    if index == MAX_VALUE && ctx.random {
        let len = ctx.buffer.len;
        ctx.buffer.unsafe_to_break(0, len);
        index = ctx.random_number() % count + 1;
    }
    let Some(glyph) = index
        .checked_sub(1)
        .and_then(|index| alternates.get(index as usize))
    else {
        return false;
    };
    ctx.replace_glyph(u32::from(glyph.0));
    true
}

fn apply_ligature_subst(
    ctx: &mut ApplyContext<'_>,
    subst: &LigatureSubst<'_>,
    seconds: &Digest,
    coverage_index: u16,
) -> bool {
    let Some(set) = subst.ligature_set_at(coverage_index) else {
        return false;
    };
    apply_ligature_set(ctx, &set, seconds)
}

fn apply_ligature_set(ctx: &mut ApplyContext<'_>, set: &LigatureSet<'_>, seconds: &Digest) -> bool {
    let count = set.len();
    let mut second = None;
    let mut unsafe_to = 0;
    if count > 1 {
        let mut iterator = ctx.context_iterator();
        iterator.reset(ctx.buffer, ctx.buffer.idx);
        if iterator.next(ctx.buffer, ctx.font, &EveryGlyph, &mut unsafe_to) {
            let info = ctx.buffer.info[iterator.index];
            unsafe_to = iterator.index + 1;
            if iterator.may_skip(ctx.font, &info) == Skip::No {
                second = Some(info.codepoint as u16);
            }
        }
    }
    let Some(second) = second else {
        if ctx.buffer.idx + 1 >= ctx.buffer.len {
            return apply_final_ligature(ctx, set);
        }
        return set.iter().any(|ligature| {
            apply_ligature(
                ctx,
                ligature.components().len(),
                |index| ligature.components().get(index).map_or(0, |glyph| glyph.0),
                ligature.glyph().0,
            )
        });
    };
    if !seconds.may_have(u32::from(second)) {
        return false;
    }
    let mut unsafe_to_concat = false;
    for ligature in set.iter() {
        let components = ligature.components();
        match components.first() {
            Some(first) if first.0 != second => unsafe_to_concat = true,
            _ => {
                if apply_ligature(
                    ctx,
                    components.len(),
                    |index| components.get(index).map_or(0, |glyph| glyph.0),
                    ligature.glyph().0,
                ) {
                    if unsafe_to_concat {
                        let idx = ctx.buffer.idx;
                        ctx.buffer.unsafe_to_concat(idx, unsafe_to);
                    }
                    return true;
                }
            }
        }
    }
    if unsafe_to_concat {
        let idx = ctx.buffer.idx;
        ctx.buffer.unsafe_to_concat(idx, unsafe_to);
    }
    false
}

fn apply_final_ligature(ctx: &mut ApplyContext<'_>, set: &LigatureSet<'_>) -> bool {
    let mut marked = false;
    for ligature in set.iter() {
        let count = ligature.components().len();
        if count == 0 {
            ctx.replace_glyph(u32::from(ligature.glyph().0));
            return true;
        }
        if !marked && count < MAX_CONTEXT_LENGTH {
            let idx = ctx.buffer.idx;
            ctx.buffer.unsafe_to_concat(idx, idx + 1);
            marked = true;
        }
    }
    false
}

pub(crate) fn apply_ligature(
    ctx: &mut ApplyContext<'_>,
    component_count: usize,
    component: impl Fn(usize) -> u16,
    ligature: u16,
) -> bool {
    if component_count == 0 {
        ctx.replace_glyph(u32::from(ligature));
        return true;
    }
    let predicate = Matches(|glyph: u32, index: usize| glyph == u32::from(component(index)));
    let mut positions: MatchPositions = [0; MAX_CONTEXT_LENGTH];
    match match_input(ctx, component_count, &predicate, &mut positions) {
        Ok(matched) => {
            ligate_input(
                ctx,
                &positions[..component_count + 1],
                matched.end,
                matched.total_component_count,
                u32::from(ligature),
            );
            true
        }
        Err(end) => {
            let idx = ctx.buffer.idx;
            ctx.buffer.unsafe_to_concat(idx, end);
            false
        }
    }
}

fn apply_reverse_chain(
    ctx: &mut ApplyContext<'_>,
    reverse: &ReverseChainSingleSubst<'_>,
    coverage_index: u16,
) -> bool {
    if ctx.nesting_level_left != MAX_NESTING_LEVEL {
        return false;
    }
    let Some(substitute) = reverse.substitute_at(coverage_index) else {
        return false;
    };
    let backtrack = reverse.backtrack();
    let lookahead = reverse.lookahead();
    let backtrack_predicate = Matches(|glyph: u32, index: usize| {
        backtrack
            .get(index)
            .is_some_and(|coverage| coverage.contains(GlyphId(glyph as u16)))
    });
    let lookahead_predicate = Matches(|glyph: u32, index: usize| {
        lookahead
            .get(index)
            .is_some_and(|coverage| coverage.contains(GlyphId(glyph as u16)))
    });
    let (start, end) = match match_backtrack(ctx, backtrack.len(), &backtrack_predicate) {
        Ok(start) => match match_lookahead(
            ctx,
            lookahead.len(),
            &lookahead_predicate,
            ctx.buffer.idx + 1,
        ) {
            Ok(end) => {
                ctx.buffer.unsafe_to_break_from_outbuffer(start, end);
                ctx.replace_glyph_inplace(u32::from(substitute.0));
                return true;
            }
            Err(end) => (start, end),
        },
        Err(start) => (start, 0),
    };
    ctx.buffer.unsafe_to_concat_from_outbuffer(start, end);
    false
}
