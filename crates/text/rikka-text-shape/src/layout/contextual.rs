use rikka_text_font::{
    Array, CoverageArray, GlyphId, SequenceContext, SequenceLookupRecord, SequenceRule,
    SequenceRuleSet,
};

use crate::face::coverage_sequence::CoverageSequence;
use crate::face::glyph_sets::GlyphSets;
use crate::face::lookup_accel::SubtableAccel;
use crate::layout::apply_context::{ApplyContext, MAX_CONTEXT_LENGTH};
use crate::layout::matching::{EveryGlyph, GlyphPredicate, Matches, Skip};
use crate::layout::sequence::{MatchPositions, apply_nested_lookups, match_input};

pub(crate) const SLOW_PATH_RULE_LIMIT: usize = 4;

#[inline]
pub(crate) fn covered(coverages: &CoverageArray<'_>, index: usize, glyph: u32) -> bool {
    coverages
        .get(index)
        .is_some_and(|coverage| coverage.contains(GlyphId(glyph as u16)))
}

pub(crate) fn apply_context(
    ctx: &mut ApplyContext<'_>,
    context: &SequenceContext<'_>,
    accel: &SubtableAccel,
    coverage_index: u16,
) -> bool {
    match context {
        SequenceContext::Glyphs(glyphs) => {
            let Some(set) = glyphs.rule_set(coverage_index) else {
                return false;
            };
            apply_rule_set(ctx, &set, |glyph, value| glyph == u32::from(value))
        }
        SequenceContext::Classes(classes) => {
            let class_of = |glyph: u32| accel.class(0, glyph);
            let Some(set) = classes.rule_set(class_of(ctx.buffer.cur(0).codepoint)) else {
                return false;
            };
            apply_rule_set(ctx, &set, |glyph, value| class_of(glyph) == value)
        }
        SequenceContext::Coverages(coverages) => {
            let input = coverages.input();
            let Some(rest) = input.len().checked_sub(1) else {
                return false;
            };
            let predicate = Matches(|glyph: u32, index: usize| covered(&input, index + 1, glyph));
            apply_input_sequence(ctx, rest, &predicate, || coverages.lookups())
        }
    }
}

pub(crate) fn apply_context_sequence<'r>(
    ctx: &mut ApplyContext<'_>,
    sets: &GlyphSets,
    sequence: &CoverageSequence,
    records: impl FnOnce() -> Array<'r, SequenceLookupRecord>,
) -> bool {
    let input = sequence.input();
    let Some(rest) = input.len().checked_sub(1) else {
        return false;
    };
    let predicate = Matches(|glyph: u32, index: usize| {
        input
            .get(index + 1)
            .is_some_and(|&set| sets.contains(set, glyph))
    });
    apply_input_sequence(ctx, rest, &predicate, records)
}

fn apply_input_sequence<'r>(
    ctx: &mut ApplyContext<'_>,
    input_count: usize,
    predicate: &impl GlyphPredicate,
    records: impl FnOnce() -> Array<'r, SequenceLookupRecord>,
) -> bool {
    let mut positions: MatchPositions = [0; MAX_CONTEXT_LENGTH];
    match match_input(ctx, input_count, predicate, &mut positions) {
        Ok(matched) => {
            let idx = ctx.buffer.idx;
            ctx.buffer.unsafe_to_break(idx, matched.end);
            apply_nested_lookups(ctx, &mut positions, input_count, matched.end, records());
            true
        }
        Err(end) => {
            let idx = ctx.buffer.idx;
            ctx.buffer.unsafe_to_concat(idx, end);
            false
        }
    }
}

fn apply_rule(
    ctx: &mut ApplyContext<'_>,
    rule: &SequenceRule<'_>,
    matches: &impl Fn(u32, u16) -> bool,
) -> bool {
    let input = rule.input();
    let predicate = Matches(|glyph: u32, index: usize| {
        input.get(index).is_some_and(|value| matches(glyph, value))
    });
    let mut positions: MatchPositions = [0; MAX_CONTEXT_LENGTH];
    match match_input(ctx, input.len(), &predicate, &mut positions) {
        Ok(matched) => {
            let idx = ctx.buffer.idx;
            ctx.buffer.unsafe_to_break(idx, matched.end);
            apply_nested_lookups(
                ctx,
                &mut positions,
                input.len(),
                matched.end,
                rule.lookups(),
            );
            true
        }
        Err(_) => false,
    }
}

pub(crate) struct Lookahead {
    pub(crate) first: Option<usize>,
    pub(crate) second: Option<usize>,
    pub(crate) unsafe_to_first: usize,
    pub(crate) unsafe_to_second: usize,
}

pub(crate) fn peek_two(ctx: &ApplyContext<'_>) -> Option<Lookahead> {
    let mut iterator = ctx.context_iterator();
    iterator.reset(ctx.buffer, ctx.buffer.idx);
    let mut ignored = 0;
    if !iterator.next(ctx.buffer, ctx.font, &EveryGlyph, &mut ignored) {
        return Some(Lookahead {
            first: None,
            second: None,
            unsafe_to_first: 0,
            unsafe_to_second: 0,
        });
    }
    let first = iterator.index;
    if iterator.may_skip(ctx.font, &ctx.buffer.info[first]) != Skip::No {
        return None;
    }
    let unsafe_to_first = first + 1;
    if !iterator.next(ctx.buffer, ctx.font, &EveryGlyph, &mut ignored) {
        return Some(Lookahead {
            first: Some(first),
            second: None,
            unsafe_to_first,
            unsafe_to_second: 0,
        });
    }
    let second = iterator.index;
    if iterator.may_skip(ctx.font, &ctx.buffer.info[second]) != Skip::No {
        return None;
    }
    Some(Lookahead {
        first: Some(first),
        second: Some(second),
        unsafe_to_first,
        unsafe_to_second: second + 1,
    })
}

fn apply_rule_set(
    ctx: &mut ApplyContext<'_>,
    set: &SequenceRuleSet<'_>,
    matches: impl Fn(u32, u16) -> bool,
) -> bool {
    let Some(lookahead) = peek_two(ctx) else {
        return set.iter().any(|rule| apply_rule(ctx, &rule, &matches));
    };
    let Some(first) = lookahead.first else {
        return set
            .iter()
            .filter(|rule| rule.input().len() <= 1)
            .any(|rule| apply_rule(ctx, &rule, &matches));
    };
    let mut unsafe_to = None;
    let mut rules = set.iter().peekable();
    while let Some(rule) = rules.next() {
        let input = rule.input();
        let first_glyph = ctx.buffer.info[first].codepoint;
        if input.len() <= 1
            || input
                .first()
                .is_some_and(|value| matches(first_glyph, value))
        {
            let second_matches = match lookahead.second {
                None => true,
                Some(second) => {
                    input.len() <= 2
                        || input
                            .get(1)
                            .is_some_and(|value| matches(ctx.buffer.info[second].codepoint, value))
                }
            };
            if second_matches {
                if apply_rule(ctx, &rule, &matches) {
                    if let Some(end) = unsafe_to {
                        let idx = ctx.buffer.idx;
                        ctx.buffer.unsafe_to_concat(idx, end);
                    }
                    return true;
                }
            } else {
                unsafe_to = Some(lookahead.unsafe_to_second);
            }
        } else {
            unsafe_to.get_or_insert(lookahead.unsafe_to_first);
            let value = input.first();
            while rules
                .peek()
                .is_some_and(|next| !next.input().is_empty() && next.input().first() == value)
            {
                rules.next();
            }
        }
    }
    if let Some(end) = unsafe_to {
        let idx = ctx.buffer.idx;
        ctx.buffer.unsafe_to_concat(idx, end);
    }
    false
}
