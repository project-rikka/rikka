use std::iter::FusedIterator;

use crate::properties::{Properties, properties};
use crate::tables::enums::{GraphemeClusterBreak, IndicConjunctBreak};

const CLASS_COUNT: usize = 16;

static KEEP_TOGETHER: [u16; CLASS_COUNT] = keep_together();

const fn bit(class: GraphemeClusterBreak) -> u16 {
    1 << class as u16
}

const fn keep_together() -> [u16; CLASS_COUNT] {
    use GraphemeClusterBreak as G;
    let extending = bit(G::Extend) | bit(G::Zwj) | bit(G::SpacingMark);
    let mut table = [extending; CLASS_COUNT];
    table[G::Cr as usize] = bit(G::Lf);
    table[G::Lf as usize] = 0;
    table[G::Control as usize] = 0;
    table[G::Prepend as usize] = !(bit(G::Cr) | bit(G::Lf) | bit(G::Control));
    table[G::L as usize] = extending | bit(G::L) | bit(G::V) | bit(G::Lv) | bit(G::Lvt);
    table[G::Lv as usize] = extending | bit(G::V) | bit(G::T);
    table[G::V as usize] = extending | bit(G::V) | bit(G::T);
    table[G::Lvt as usize] = extending | bit(G::T);
    table[G::T as usize] = extending | bit(G::T);
    table
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Pictographic {
    #[default]
    None,
    Extended,
    Joined,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Conjunct {
    #[default]
    None,
    Consonant,
    Linked,
}

#[derive(Clone, Copy, Debug, Default)]
struct ClusterState {
    odd_regional_indicators: bool,
    pictographic: Pictographic,
    conjunct: Conjunct,
}

impl ClusterState {
    #[inline]
    fn advance(&mut self, properties: Properties, class: GraphemeClusterBreak) {
        self.odd_regional_indicators =
            class == GraphemeClusterBreak::RegionalIndicator && !self.odd_regional_indicators;
        self.pictographic = if properties.is_extended_pictographic() {
            Pictographic::Extended
        } else if self.pictographic == Pictographic::Extended {
            match class {
                GraphemeClusterBreak::Extend => Pictographic::Extended,
                GraphemeClusterBreak::Zwj => Pictographic::Joined,
                _ => Pictographic::None,
            }
        } else {
            Pictographic::None
        };
        self.conjunct = match (properties.indic_conjunct_break(), self.conjunct) {
            (IndicConjunctBreak::Consonant, _) => Conjunct::Consonant,
            (_, Conjunct::None) => Conjunct::None,
            (IndicConjunctBreak::Linker, _) => Conjunct::Linked,
            (IndicConjunctBreak::Extend, state) => state,
            (IndicConjunctBreak::None, _) => Conjunct::None,
        };
    }

    #[inline]
    fn joins(
        &self,
        previous: GraphemeClusterBreak,
        next: GraphemeClusterBreak,
        properties: Properties,
    ) -> bool {
        if KEEP_TOGETHER[previous as usize] & bit(next) != 0 {
            return true;
        }
        match next {
            GraphemeClusterBreak::RegionalIndicator => self.odd_regional_indicators,
            _ if properties.is_extended_pictographic() => self.pictographic == Pictographic::Joined,
            _ => {
                self.conjunct == Conjunct::Linked
                    && properties.indic_conjunct_break() == IndicConjunctBreak::Consonant
            }
        }
    }
}

pub(crate) fn cluster_end(text: &str, start: usize) -> usize {
    let mut characters = text[start..].char_indices();
    let Some((_, first)) = characters.next() else {
        return start;
    };
    let first = properties(first);
    let mut previous = first.grapheme_cluster_break();
    let mut state = ClusterState::default();
    state.advance(first, previous);
    for (offset, character) in characters {
        let properties = properties(character);
        let class = properties.grapheme_cluster_break();
        if !state.joins(previous, class, properties) {
            return start + offset;
        }
        state.advance(properties, class);
        previous = class;
    }
    text.len()
}

#[derive(Clone, Debug)]
pub struct Graphemes<'a> {
    text: &'a str,
    position: usize,
}

pub fn graphemes(text: &str) -> Graphemes<'_> {
    Graphemes { text, position: 0 }
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = (usize, &'a str);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let start = self.position;
        if start >= self.text.len() {
            return None;
        }
        let end = cluster_end(self.text, start);
        self.position = end;
        Some((start, &self.text[start..end]))
    }
}

impl FusedIterator for Graphemes<'_> {}
