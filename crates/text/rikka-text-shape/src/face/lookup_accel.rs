use std::sync::OnceLock;

use rikka_text_font::{
    ChainedSequenceContext, Coverage, GlyphPairPos, LigatureSubst, Lookup, LookupSubtable, PairPos,
    PosLookup, PosSubtable, SequenceContext, SubstLookup, SubstSubtable, SubtableLocation,
};

use crate::face::candidates::Candidates;
use crate::face::coverage_sequence::CoverageSequence;
use crate::face::digest::Digest;
use crate::face::glyph_map::GlyphMap;
use crate::face::glyph_sets::GlyphSets;
use crate::face::glyph_sets_builder::GlyphSetsBuilder;
use crate::face::lookup_filter::LookupFilter;
use crate::face::rule_index::{RuleIndex, chain_rule_indexes};

const REVERSE_CHAIN_SINGLE: u16 = 8;

#[derive(Debug, Default)]
pub(crate) struct SubtableAccel {
    pub(crate) location: Option<SubtableLocation>,
    pub(crate) seconds: Digest,
    pub(crate) class_maps: Box<[GlyphMap]>,
    pub(crate) rules: Box<[RuleIndex]>,
    pub(crate) sequence: Option<CoverageSequence>,
    pub(crate) targets: Option<GlyphMap>,
    pub(crate) ordered_pairs: Box<[u64]>,
}

impl SubtableAccel {
    #[inline]
    pub(crate) fn has_ordered_pairs(&self, coverage_index: u16) -> bool {
        let index = usize::from(coverage_index);
        self.ordered_pairs
            .get(index / 64)
            .is_some_and(|word| word >> (index % 64) & 1 != 0)
    }

    #[inline]
    pub(crate) fn rules(&self, set: u16) -> Option<&RuleIndex> {
        self.rules.get(usize::from(set))
    }

    #[inline]
    pub(crate) fn class(&self, slot: usize, glyph: u32) -> u16 {
        self.class_maps.get(slot).map_or(0, |map| map.class(glyph))
    }

    #[inline]
    pub(crate) fn target_index(&self, glyph: u32) -> Option<u16> {
        self.targets.as_ref()?.coverage_index(glyph)
    }
}

#[derive(Debug, Default)]
struct SubtableDetails {
    seconds: Digest,
    class_maps: Vec<GlyphMap>,
    rules: Box<[RuleIndex]>,
    sequence: Option<CoverageSequence>,
    targets: Option<GlyphMap>,
    ordered_pairs: Box<[u64]>,
}

#[derive(Debug)]
pub(crate) struct LookupAccel {
    pub(crate) filter: LookupFilter,
    pub(crate) candidates: Candidates,
    pub(crate) sets: GlyphSets,
    pub(crate) subtables: Box<[SubtableAccel]>,
}

impl LookupAccel {
    pub(crate) fn from_subst(lookup: &SubstLookup<'_>) -> Self {
        Self::build(
            lookup,
            lookup.kind() == REVERSE_CHAIN_SINGLE,
            |subtable, sets| {
                let details = match subtable {
                    SubstSubtable::Ligature(ligature) => SubtableDetails {
                        seconds: ligature_seconds(ligature),
                        ..SubtableDetails::default()
                    },
                    SubstSubtable::Context(context) => context_details(context, sets),
                    SubstSubtable::ChainContext(context) => chain_details(context, sets),
                    _ => SubtableDetails::default(),
                };
                (subtable.coverage(), details)
            },
        )
    }

    pub(crate) fn from_pos(lookup: &PosLookup<'_>) -> Self {
        Self::build(lookup, false, |subtable: &PosSubtable<'_>, sets| {
            let details = match subtable {
                PosSubtable::Pair(PairPos::Glyphs(pairs)) => SubtableDetails {
                    ordered_pairs: ordered_pairs(pairs),
                    ..SubtableDetails::default()
                },
                PosSubtable::Pair(PairPos::Classes(classes)) => SubtableDetails {
                    class_maps: vec![
                        GlyphMap::from_class_def(&classes.first_class_def()),
                        GlyphMap::from_class_def(&classes.second_class_def()),
                    ],
                    ..SubtableDetails::default()
                },
                PosSubtable::Cursive(cursive) => targets(&cursive.coverage()),
                PosSubtable::MarkBase(mark_base) => targets(&mark_base.base_coverage()),
                PosSubtable::MarkLigature(mark_ligature) => {
                    targets(&mark_ligature.ligature_coverage())
                }
                PosSubtable::MarkMark(mark_mark) => targets(&mark_mark.base_mark_coverage()),
                PosSubtable::Context(context) => context_details(context, sets),
                PosSubtable::ChainContext(context) => chain_details(context, sets),
                _ => SubtableDetails::default(),
            };
            (subtable.coverage(), details)
        })
    }

    fn build<'a, S: LookupSubtable<'a>>(
        lookup: &Lookup<'a, S>,
        reverse: bool,
        analyze: impl Fn(&S, &mut GlyphSetsBuilder) -> (Option<Coverage<'a>>, SubtableDetails),
    ) -> Self {
        let flag = lookup.flag();
        let mut props = u32::from(flag.0);
        if flag.use_mark_filtering_set() {
            props |= u32::from(lookup.mark_filtering_set().unwrap_or(0)) << 16;
        }
        let mut digest = Digest::default();
        let mut sets = GlyphSetsBuilder::default();
        let mut coverages = Vec::with_capacity(lookup.subtable_count());
        let subtables = (0..lookup.subtable_count())
            .map(|index| {
                let location = lookup.subtable_location(index);
                let Some((coverage, details)) = location
                    .and_then(|_| lookup.subtable(index))
                    .map(|subtable| analyze(&subtable, &mut sets))
                else {
                    coverages.push(None);
                    return SubtableAccel::default();
                };
                if let Some(coverage) = &coverage {
                    digest.union(&Digest::from_coverage(coverage));
                }
                coverages.push(coverage);
                SubtableAccel {
                    location,
                    seconds: details.seconds,
                    class_maps: details.class_maps.into_boxed_slice(),
                    rules: details.rules,
                    sequence: details.sequence,
                    targets: details.targets,
                    ordered_pairs: details.ordered_pairs,
                }
            })
            .collect();
        Self {
            filter: LookupFilter {
                props,
                reverse,
                digest,
            },
            candidates: Candidates::build(&coverages),
            sets: sets.build(),
            subtables,
        }
    }
}

fn ordered_pairs(pairs: &GlyphPairPos<'_>) -> Box<[u64]> {
    let count = pairs.pair_set_count();
    let mut words = vec![0u64; count.div_ceil(64)];
    for index in 0..count {
        let ordered = u16::try_from(index)
            .ok()
            .and_then(|index| pairs.pair_set(index))
            .is_some_and(|set| set.is_ordered());
        if ordered {
            words[index / 64] |= 1 << (index % 64);
        }
    }
    words.into_boxed_slice()
}

fn targets(coverage: &Coverage<'_>) -> SubtableDetails {
    SubtableDetails {
        targets: Some(GlyphMap::from_coverage(coverage)),
        ..SubtableDetails::default()
    }
}

fn context_details(context: &SequenceContext<'_>, sets: &mut GlyphSetsBuilder) -> SubtableDetails {
    match context {
        SequenceContext::Classes(classes) => SubtableDetails {
            class_maps: vec![GlyphMap::from_class_def(&classes.class_def())],
            ..SubtableDetails::default()
        },
        SequenceContext::Coverages(coverages) => SubtableDetails {
            sequence: CoverageSequence::build(false, [None, Some(coverages.input()), None], sets),
            ..SubtableDetails::default()
        },
        SequenceContext::Glyphs(_) => SubtableDetails::default(),
    }
}

fn chain_details(
    context: &ChainedSequenceContext<'_>,
    sets: &mut GlyphSetsBuilder,
) -> SubtableDetails {
    match context {
        ChainedSequenceContext::Classes(classes) => SubtableDetails {
            class_maps: vec![
                GlyphMap::from_class_def(&classes.backtrack_class_def()),
                GlyphMap::from_class_def(&classes.input_class_def()),
                GlyphMap::from_class_def(&classes.lookahead_class_def()),
            ],
            rules: chain_rule_indexes(context),
            ..SubtableDetails::default()
        },
        ChainedSequenceContext::Glyphs(_) => SubtableDetails {
            rules: chain_rule_indexes(context),
            ..SubtableDetails::default()
        },
        ChainedSequenceContext::Coverages(coverages) => SubtableDetails {
            sequence: CoverageSequence::build(
                true,
                [
                    Some(coverages.backtrack()),
                    Some(coverages.input()),
                    Some(coverages.lookahead()),
                ],
                sets,
            ),
            ..SubtableDetails::default()
        },
    }
}

fn ligature_seconds(subst: &LigatureSubst<'_>) -> Digest {
    let mut seconds = Digest::default();
    let count: usize = subst
        .coverage()
        .ranges()
        .map(|(first, last)| usize::from(last.0 - first.0) + 1)
        .sum();
    for set in (0..count).filter_map(|index| subst.ligature_set_at(index as u16)) {
        for ligature in set.iter() {
            match ligature.components().first() {
                Some(second) => seconds.add(u32::from(second.0)),
                None => seconds.add_range(0, u32::from(u16::MAX)),
            }
        }
    }
    seconds
}

#[derive(Debug, Default)]
pub(crate) struct LookupAccels {
    lookups: Box<[OnceLock<Option<LookupAccel>>]>,
}

impl LookupAccels {
    pub(crate) fn new(count: usize) -> Self {
        Self {
            lookups: (0..count).map(|_| OnceLock::new()).collect(),
        }
    }

    #[inline]
    pub(crate) fn get(
        &self,
        index: u16,
        build: impl FnOnce() -> Option<LookupAccel>,
    ) -> Option<&LookupAccel> {
        self.lookups
            .get(usize::from(index))?
            .get_or_init(build)
            .as_ref()
    }
}
