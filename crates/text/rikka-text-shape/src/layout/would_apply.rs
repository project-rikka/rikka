use rikka_text_font::{
    Array, ChainedSequenceContext, ChainedSequenceRule, ClassDef, GlyphId, LookupSubtable,
    SequenceContext, SubstSubtable,
};

use crate::face::font_context::FontContext;
use crate::plan::feature_map::{LookupEntry, TableKind};

#[derive(Clone, Copy, Debug)]
pub(crate) struct WouldApply<'g> {
    glyphs: &'g [u32],
    zero_context: bool,
}

impl<'g> WouldApply<'g> {
    pub(crate) fn new(glyphs: &'g [u32], zero_context: bool) -> Self {
        Self {
            glyphs,
            zero_context,
        }
    }

    pub(crate) fn any_lookup(&self, font: &FontContext, lookups: &[LookupEntry]) -> bool {
        lookups.iter().any(|entry| self.lookup(font, entry.index))
    }

    fn lookup(&self, font: &FontContext, index: u16) -> bool {
        let Some(&first) = self.glyphs.first() else {
            return false;
        };
        let (Some(accel), Some(table)) = (
            font.lookup(TableKind::Gsub, index),
            font.table(TableKind::Gsub),
        ) else {
            return false;
        };
        accel.candidates.get(first).iter().any(|candidate| {
            accel.subtables[usize::from(candidate.subtable)]
                .location
                .and_then(|location| SubstSubtable::from_location(table, location))
                .is_some_and(|subtable| self.subtable(&subtable, candidate.coverage_index))
        })
    }

    fn subtable(&self, subtable: &SubstSubtable<'_>, coverage_index: u16) -> bool {
        let single = self.glyphs.len() == 1;
        match subtable {
            SubstSubtable::Single(_)
            | SubstSubtable::Multiple(_)
            | SubstSubtable::Alternate(_)
            | SubstSubtable::ReverseChainSingle(_) => single,
            SubstSubtable::Ligature(ligature) => {
                ligature.ligature_set_at(coverage_index).is_some_and(|set| {
                    set.iter().any(|ligature| {
                        let components = ligature.components();
                        self.rest_matches(components.len(), |index, glyph| {
                            components
                                .get(index)
                                .is_some_and(|component| u32::from(component.0) == glyph)
                        })
                    })
                })
            }
            SubstSubtable::Context(context) => self.context(context, coverage_index),
            SubstSubtable::ChainContext(context) => self.chain_context(context, coverage_index),
        }
    }

    fn rest_matches(&self, count: usize, matches: impl Fn(usize, u32) -> bool) -> bool {
        self.glyphs.len() == count + 1
            && self.glyphs[1..]
                .iter()
                .enumerate()
                .all(|(index, &glyph)| matches(index, glyph))
    }

    fn input_matches(&self, input: Array<'_, u16>, matches: impl Fn(u32, u16) -> bool) -> bool {
        self.rest_matches(input.len(), |index, glyph| {
            input.get(index).is_some_and(|value| matches(glyph, value))
        })
    }

    fn context(&self, context: &SequenceContext<'_>, coverage_index: u16) -> bool {
        match context {
            SequenceContext::Glyphs(glyphs) => glyphs.rule_set(coverage_index).is_some_and(|set| {
                set.iter()
                    .any(|rule| self.input_matches(rule.input(), same_glyph))
            }),
            SequenceContext::Classes(classes) => {
                let class_def = classes.class_def();
                classes
                    .rule_set(class_of(&class_def, self.glyphs[0]))
                    .is_some_and(|set| {
                        set.iter().any(|rule| {
                            self.input_matches(rule.input(), |glyph, class| {
                                class_of(&class_def, glyph) == class
                            })
                        })
                    })
            }
            SequenceContext::Coverages(coverages) => {
                let input = coverages.input();
                self.glyphs.len() == input.len()
                    && self
                        .glyphs
                        .iter()
                        .enumerate()
                        .skip(1)
                        .all(|(index, &glyph)| {
                            input
                                .get(index)
                                .is_some_and(|coverage| coverage.contains(GlyphId(glyph as u16)))
                        })
            }
        }
    }

    fn contextless(&self, rule: &ChainedSequenceRule<'_>) -> bool {
        !self.zero_context || (rule.backtrack().is_empty() && rule.lookahead().is_empty())
    }

    fn chain_context(&self, context: &ChainedSequenceContext<'_>, coverage_index: u16) -> bool {
        match context {
            ChainedSequenceContext::Glyphs(glyphs) => {
                glyphs.rule_set(coverage_index).is_some_and(|set| {
                    set.iter().any(|rule| {
                        self.contextless(&rule) && self.input_matches(rule.input(), same_glyph)
                    })
                })
            }
            ChainedSequenceContext::Classes(classes) => {
                let class_def = classes.input_class_def();
                classes
                    .rule_set(class_of(&class_def, self.glyphs[0]))
                    .is_some_and(|set| {
                        set.iter().any(|rule| {
                            self.contextless(&rule)
                                && self.input_matches(rule.input(), |glyph, class| {
                                    class_of(&class_def, glyph) == class
                                })
                        })
                    })
            }
            ChainedSequenceContext::Coverages(coverages) => {
                let input = coverages.input();
                (!self.zero_context
                    || (coverages.backtrack().is_empty() && coverages.lookahead().is_empty()))
                    && self.glyphs.len() == input.len()
                    && self
                        .glyphs
                        .iter()
                        .enumerate()
                        .skip(1)
                        .all(|(index, &glyph)| {
                            input
                                .get(index)
                                .is_some_and(|coverage| coverage.contains(GlyphId(glyph as u16)))
                        })
            }
        }
    }
}

fn same_glyph(glyph: u32, value: u16) -> bool {
    glyph == u32::from(value)
}

fn class_of(class_def: &ClassDef<'_>, glyph: u32) -> u16 {
    class_def.get(GlyphId(glyph as u16))
}
