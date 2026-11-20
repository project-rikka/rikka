use rikka_text_font::{
    Array, ChainedSequenceContext, ChainedSequenceRule, ChainedSequenceRuleSet,
    SequenceLookupRecord,
};

use crate::face::coverage_sequence::CoverageSequence;
use crate::face::glyph_sets::GlyphSets;
use crate::face::lookup_accel::SubtableAccel;
use crate::face::rule_index::{IndexedRule, RuleIndex};
use crate::face::rule_summary::RuleSummary;
use crate::layout::apply_context::{ApplyContext, MAX_CONTEXT_LENGTH};
use crate::layout::contextual::{Lookahead, SLOW_PATH_RULE_LIMIT, covered, peek_two};
use crate::layout::matching::{GlyphPredicate, Matches};
use crate::layout::sequence::{
    MatchPositions, apply_nested_lookups, match_backtrack, match_input, match_lookahead,
};

const BACKTRACK: usize = 0;
const INPUT: usize = 1;
const LOOKAHEAD: usize = 2;

trait ChainKeys {
    fn key(&self, sequence: usize, glyph: u32) -> u16;
}

struct GlyphKeys;

impl ChainKeys for GlyphKeys {
    #[inline]
    fn key(&self, _: usize, glyph: u32) -> u16 {
        glyph as u16
    }
}

struct ClassKeys<'a> {
    accel: &'a SubtableAccel,
}

impl ChainKeys for ClassKeys<'_> {
    #[inline]
    fn key(&self, sequence: usize, glyph: u32) -> u16 {
        self.accel.class(sequence, glyph)
    }
}

pub(crate) fn apply_chain_context(
    ctx: &mut ApplyContext<'_>,
    context: &ChainedSequenceContext<'_>,
    accel: &SubtableAccel,
    coverage_index: u16,
) -> bool {
    match context {
        ChainedSequenceContext::Glyphs(glyphs) => {
            let Some(set) = glyphs.rule_set(coverage_index) else {
                return false;
            };
            apply_rule_set(ctx, &set, accel.rules(coverage_index), &GlyphKeys)
        }
        ChainedSequenceContext::Classes(classes) => {
            let keys = ClassKeys { accel };
            let class = keys.key(INPUT, ctx.buffer.cur(0).codepoint);
            let Some(set) = classes.rule_set(class) else {
                return false;
            };
            apply_rule_set(ctx, &set, accel.rules(class), &keys)
        }
        ChainedSequenceContext::Coverages(coverages) => {
            let input = coverages.input();
            let backtrack = coverages.backtrack();
            let lookahead = coverages.lookahead();
            let Some(rest) = input.len().checked_sub(1) else {
                return false;
            };
            apply_chain(
                ctx,
                (
                    rest,
                    &Matches(|glyph: u32, index: usize| covered(&input, index + 1, glyph)),
                ),
                (
                    backtrack.len(),
                    &Matches(|glyph: u32, index: usize| covered(&backtrack, index, glyph)),
                ),
                (
                    lookahead.len(),
                    &Matches(|glyph: u32, index: usize| covered(&lookahead, index, glyph)),
                ),
                || coverages.lookups(),
            )
        }
    }
}

pub(crate) fn apply_chain_sequence<'r>(
    ctx: &mut ApplyContext<'_>,
    sets: &GlyphSets,
    sequence: &CoverageSequence,
    records: impl FnOnce() -> Array<'r, SequenceLookupRecord>,
) -> bool {
    let input = sequence.input();
    let backtrack = sequence.backtrack();
    let lookahead = sequence.lookahead();
    let Some(rest) = input.len().checked_sub(1) else {
        return false;
    };
    let member = |sequence: &[u16], index: usize, glyph: u32| {
        sequence
            .get(index)
            .is_some_and(|&set| sets.contains(set, glyph))
    };
    apply_chain(
        ctx,
        (
            rest,
            &Matches(|glyph: u32, index: usize| member(input, index + 1, glyph)),
        ),
        (
            backtrack.len(),
            &Matches(|glyph: u32, index: usize| member(backtrack, index, glyph)),
        ),
        (
            lookahead.len(),
            &Matches(|glyph: u32, index: usize| member(lookahead, index, glyph)),
        ),
        records,
    )
}

fn apply_chain<'r>(
    ctx: &mut ApplyContext<'_>,
    (input_count, input): (usize, &impl GlyphPredicate),
    (backtrack_count, backtrack): (usize, &impl GlyphPredicate),
    (lookahead_count, lookahead): (usize, &impl GlyphPredicate),
    lookups: impl FnOnce() -> Array<'r, SequenceLookupRecord>,
) -> bool {
    let idx = ctx.buffer.idx;
    let mut positions: MatchPositions = [0; MAX_CONTEXT_LENGTH];
    let Ok(matched) = match_input(ctx, input_count, input, &mut positions) else {
        return false;
    };
    let end = match match_lookahead(ctx, lookahead_count, lookahead, matched.end) {
        Ok(end) => end,
        Err(end) => {
            ctx.buffer.unsafe_to_concat(idx, end);
            return false;
        }
    };
    let start = match match_backtrack(ctx, backtrack_count, backtrack) {
        Ok(start) => start,
        Err(start) => {
            ctx.buffer.unsafe_to_concat_from_outbuffer(start, end);
            return false;
        }
    };
    ctx.buffer.unsafe_to_break_from_outbuffer(start, end);
    apply_nested_lookups(ctx, &mut positions, input_count, matched.end, lookups());
    true
}

fn sequence_matches<'a>(
    keys: &'a impl ChainKeys,
    sequence: usize,
    values: Array<'a, u16>,
) -> (usize, Matches<impl Fn(u32, usize) -> bool + 'a>) {
    (
        values.len(),
        Matches(move |glyph: u32, index: usize| {
            values
                .get(index)
                .is_some_and(|value| keys.key(sequence, glyph) == value)
        }),
    )
}

fn apply_rule(
    ctx: &mut ApplyContext<'_>,
    rule: &ChainedSequenceRule<'_>,
    keys: &impl ChainKeys,
) -> bool {
    let (input_count, input) = sequence_matches(keys, INPUT, rule.input());
    let (backtrack_count, backtrack) = sequence_matches(keys, BACKTRACK, rule.backtrack());
    let (lookahead_count, lookahead) = sequence_matches(keys, LOOKAHEAD, rule.lookahead());
    apply_chain(
        ctx,
        (input_count, &input),
        (backtrack_count, &backtrack),
        (lookahead_count, &lookahead),
        || rule.lookups(),
    )
}

#[derive(Clone, Copy)]
struct ChainGlyphKeys {
    input: u16,
    lookahead: u16,
}

impl ChainGlyphKeys {
    fn read(ctx: &ApplyContext<'_>, keys: &impl ChainKeys, index: usize) -> Self {
        let glyph = ctx.buffer.info[index].codepoint;
        Self {
            input: keys.key(INPUT, glyph),
            lookahead: keys.key(LOOKAHEAD, glyph),
        }
    }
}

fn matches_second(summary: &RuleSummary, second: Option<ChainGlyphKeys>) -> bool {
    let Some(second) = second else {
        return true;
    };
    match summary.inputs {
        0 => summary.lookaheads <= 1 || summary.lookahead[1] == second.lookahead,
        1 => summary.lookaheads == 0 || summary.lookahead[0] == second.lookahead,
        _ => summary.input[1] == second.input,
    }
}

struct FirstMatches<'r> {
    buckets: [&'r [IndexedRule]; 3],
}

impl<'r> Iterator for FirstMatches<'r> {
    type Item = &'r IndexedRule;

    #[inline]
    fn next(&mut self) -> Option<&'r IndexedRule> {
        let bucket = (0..self.buckets.len())
            .filter_map(|bucket| Some((bucket, self.buckets[bucket].first()?.rule)))
            .min_by_key(|&(_, rule)| rule)?
            .0;
        let (first, rest) = self.buckets[bucket].split_first()?;
        self.buckets[bucket] = rest;
        Some(first)
    }
}

fn apply_rule_set(
    ctx: &mut ApplyContext<'_>,
    set: &ChainedSequenceRuleSet<'_>,
    rules: Option<&RuleIndex>,
    keys: &impl ChainKeys,
) -> bool {
    let slow = |ctx: &mut ApplyContext<'_>| set.iter().any(|rule| apply_rule(ctx, &rule, keys));
    let Some(rules) = rules.filter(|rules| rules.rule_count() == set.len()) else {
        return slow(ctx);
    };
    if set.len() <= SLOW_PATH_RULE_LIMIT {
        return slow(ctx);
    }
    let Some(peeked) = peek_two(ctx) else {
        return slow(ctx);
    };
    let apply_indexed = |ctx: &mut ApplyContext<'_>, indexed: &IndexedRule| {
        set.get(usize::from(indexed.rule))
            .is_some_and(|rule| apply_rule(ctx, &rule, keys))
    };
    let Some(first) = peeked.first else {
        return rules
            .wildcards()
            .iter()
            .any(|indexed| apply_indexed(ctx, indexed));
    };
    let first = ChainGlyphKeys::read(ctx, keys, first);
    let second = peeked
        .second
        .map(|second| ChainGlyphKeys::read(ctx, keys, second));
    let candidates = FirstMatches {
        buckets: [
            rules.with_input(first.input),
            rules.with_lookahead(first.lookahead),
            rules.wildcards(),
        ],
    };
    let mut scanned = 0;
    let mut second_mismatch = false;
    for indexed in candidates {
        scanned += 1;
        if !matches_second(&indexed.summary, second) {
            second_mismatch = true;
        } else if apply_indexed(ctx, indexed) {
            let first_mismatch = usize::from(indexed.present_before) >= scanned;
            mark_unsafe(ctx, &peeked, second_mismatch, first_mismatch);
            return true;
        }
    }
    mark_unsafe(ctx, &peeked, second_mismatch, rules.present() > scanned);
    false
}

fn mark_unsafe(
    ctx: &mut ApplyContext<'_>,
    peeked: &Lookahead,
    second_mismatch: bool,
    first_mismatch: bool,
) {
    let end = if second_mismatch {
        peeked.unsafe_to_second
    } else if first_mismatch {
        peeked.unsafe_to_first
    } else {
        return;
    };
    let idx = ctx.buffer.idx;
    ctx.buffer.unsafe_to_concat(idx, end);
}
