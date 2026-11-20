use rikka_text_font::{ChainedSequenceContext, ChainedSequenceRuleSet};

use crate::face::rule_summary::{LOOKAHEAD_KEY, RuleSummary, WILDCARD_KEY};

#[derive(Clone, Copy, Debug)]
pub(crate) struct IndexedRule {
    pub(crate) rule: u16,
    pub(crate) present_before: u16,
    pub(crate) summary: RuleSummary,
}

#[derive(Debug, Default)]
pub(crate) struct RuleIndex {
    rule_count: usize,
    present: usize,
    keys: Box<[u32]>,
    rules: Box<[IndexedRule]>,
}

impl RuleIndex {
    fn new(set: Option<ChainedSequenceRuleSet<'_>>) -> Self {
        let Some(set) = set else {
            return Self::default();
        };
        let mut keyed: Vec<(u32, IndexedRule)> = Vec::new();
        for index in 0..set.len() {
            let Some(rule) = set.get(index) else {
                continue;
            };
            let summary = RuleSummary::new(&rule);
            keyed.push((
                summary.first_key(),
                IndexedRule {
                    rule: index as u16,
                    present_before: keyed.len() as u16,
                    summary,
                },
            ));
        }
        let present = keyed.len();
        keyed.sort_by_key(|&(key, rule)| (key, rule.rule));
        Self {
            rule_count: set.len(),
            present,
            keys: keyed.iter().map(|&(key, _)| key).collect(),
            rules: keyed.into_iter().map(|(_, rule)| rule).collect(),
        }
    }

    #[inline]
    pub(crate) fn rule_count(&self) -> usize {
        self.rule_count
    }

    #[inline]
    pub(crate) fn present(&self) -> usize {
        self.present
    }

    #[inline]
    pub(crate) fn with_key(&self, key: u32) -> &[IndexedRule] {
        let start = self.keys.partition_point(|&known| known < key);
        let end = start + self.keys[start..].partition_point(|&known| known == key);
        &self.rules[start..end]
    }

    #[inline]
    pub(crate) fn with_input(&self, value: u16) -> &[IndexedRule] {
        self.with_key(u32::from(value))
    }

    #[inline]
    pub(crate) fn with_lookahead(&self, value: u16) -> &[IndexedRule] {
        self.with_key(LOOKAHEAD_KEY | u32::from(value))
    }

    #[inline]
    pub(crate) fn wildcards(&self) -> &[IndexedRule] {
        self.with_key(WILDCARD_KEY)
    }
}

pub(crate) fn chain_rule_indexes(context: &ChainedSequenceContext<'_>) -> Box<[RuleIndex]> {
    match context {
        ChainedSequenceContext::Glyphs(glyphs) => {
            let count: usize = glyphs
                .coverage()
                .ranges()
                .map(|(first, last)| usize::from(last.0.saturating_sub(first.0)) + 1)
                .sum();
            (0..count.min(usize::from(u16::MAX)))
                .map(|index| RuleIndex::new(glyphs.rule_set(index as u16)))
                .collect()
        }
        ChainedSequenceContext::Classes(classes) => {
            let count = classes
                .input_class_def()
                .ranges()
                .map(|(_, _, class)| usize::from(class) + 1)
                .max()
                .unwrap_or(1);
            (0..count)
                .map(|class| RuleIndex::new(classes.rule_set(class as u16)))
                .collect()
        }
        ChainedSequenceContext::Coverages(_) => Box::default(),
    }
}
