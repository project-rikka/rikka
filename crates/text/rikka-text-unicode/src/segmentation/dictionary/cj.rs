use std::ops::Range;

use crate::properties::properties;
use crate::tables::dictionaries::{CJ_DICTIONARY, CJ_STATES, CJ_SYMBOLS, CJ_TARGET_BITS};
use crate::tables::enums::Script;
use crate::{graphemes, nfkc_into};

const LONGEST_WORD: usize = 20;
const UNKNOWN_COST: u32 = 255;
const KATAKANA_COSTS: [u32; 9] = [8192, 984, 408, 240, 204, 252, 300, 372, 480];
const LONG_KATAKANA_COST: u32 = 8192;
const KATAKANA_GROUP_LIMIT: usize = 20;
const FIRST_AT: usize = CJ_SYMBOLS * 4;
const EDGES_AT: usize = FIRST_AT + (CJ_STATES + 1) * 4;
const TARGET_MASK: u32 = (1 << CJ_TARGET_BITS) - 1;

const FIRST_CJ: char = '\u{2E80}';

#[inline]
pub(crate) fn is_cj(character: char) -> bool {
    character >= FIRST_CJ
        && (matches!(
            properties(character).script(),
            Script::Han | Script::Hiragana | Script::Katakana
        ) || matches!(character, '\u{30FC}' | '\u{FF70}' | '\u{FF9E}' | '\u{FF9F}'))
}

fn is_katakana(character: char) -> bool {
    matches!(character, '\u{30A1}'..='\u{30FE}' | '\u{FF66}'..='\u{FF9F}')
        && character != '\u{30FB}'
}

fn katakana_cost(length: usize) -> u32 {
    KATAKANA_COSTS
        .get(length)
        .copied()
        .unwrap_or(LONG_KATAKANA_COST)
}

#[inline]
fn word(at: usize) -> u32 {
    let bytes = &CJ_DICTIONARY[at..at + 4];
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn symbol(character: char) -> Option<u32> {
    let code = character as u32;
    let (mut low, mut high) = (0, CJ_SYMBOLS);
    while low < high {
        let middle = low + (high - low) / 2;
        match word(middle * 4).cmp(&code) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Some(middle as u32),
        }
    }
    None
}

fn step(state: u32, symbol: u32) -> Option<u32> {
    let state = state as usize;
    let (mut low, mut high) = (
        word(FIRST_AT + state * 4) as usize,
        word(FIRST_AT + (state + 1) * 4) as usize,
    );
    while low < high {
        let middle = low + (high - low) / 2;
        let edge = word(EDGES_AT + middle * 4);
        match (edge >> CJ_TARGET_BITS).cmp(&symbol) {
            std::cmp::Ordering::Less => low = middle + 1,
            std::cmp::Ordering::Greater => high = middle,
            std::cmp::Ordering::Equal => return Some(edge & TARGET_MASK),
        }
    }
    None
}

fn cost(state: u32) -> Option<u32> {
    let cost = CJ_DICTIONARY[CJ_DICTIONARY.len() - CJ_STATES + state as usize];
    (cost != 0).then_some(u32::from(cost))
}

#[derive(Clone, Copy, Debug)]
struct Match {
    length: usize,
    cost: u32,
}

fn matches(characters: &[char], found: &mut Vec<Match>) {
    found.clear();
    let mut state = 0;
    for (index, &character) in characters.iter().take(LONGEST_WORD).enumerate() {
        let Some(next) = symbol(character).and_then(|symbol| step(state, symbol)) else {
            return;
        };
        state = next;
        if let Some(cost) = cost(state) {
            found.push(Match {
                length: index + 1,
                cost,
            });
        }
    }
}

#[derive(Debug, Default)]
struct Lattice {
    characters: Vec<char>,
    origins: Vec<usize>,
    best: Vec<u32>,
    previous: Vec<usize>,
    found: Vec<Match>,
    path: Vec<usize>,
    normalized: String,
}

impl Lattice {
    fn normalize(&mut self, text: &str, run: Range<usize>) {
        self.characters.clear();
        self.origins.clear();
        for (offset, cluster) in graphemes(&text[run.clone()]) {
            self.normalized.clear();
            nfkc_into(cluster, &mut self.normalized);
            for character in self.normalized.chars() {
                self.characters.push(character);
                self.origins.push(run.start + offset);
            }
        }
    }

    fn relax(&mut self, from: usize, length: usize, cost: u32) {
        let reached = self.best[from].saturating_add(cost);
        if reached < self.best[from + length] {
            self.best[from + length] = reached;
            self.previous[from + length] = from;
        }
    }

    fn solve(&mut self) {
        let count = self.characters.len();
        self.best.clear();
        self.best.resize(count + 1, u32::MAX);
        self.best[0] = 0;
        self.previous.clear();
        self.previous.resize(count + 1, 0);
        let mut found = std::mem::take(&mut self.found);
        let mut prior_katakana = false;
        for at in 0..count {
            matches(&self.characters[at..], &mut found);
            if found.first().is_none_or(|first| first.length != 1) {
                found.push(Match {
                    length: 1,
                    cost: UNKNOWN_COST,
                });
            }
            for candidate in &found {
                self.relax(at, candidate.length, candidate.cost);
            }
            let katakana = is_katakana(self.characters[at]);
            if katakana && !prior_katakana {
                let run = 1 + self.characters[at + 1..]
                    .iter()
                    .take(KATAKANA_GROUP_LIMIT - 1)
                    .take_while(|&&character| is_katakana(character))
                    .count();
                if run < KATAKANA_GROUP_LIMIT {
                    self.relax(at, run, katakana_cost(run));
                }
            }
            prior_katakana = katakana;
        }
        self.found = found;
    }

    fn breaks(&mut self, run: Range<usize>, breaks: &mut Vec<usize>) {
        self.path.clear();
        let mut at = self.previous[self.characters.len()];
        while at > 0 {
            self.path.push(at);
            at = self.previous[at];
        }
        for &position in self.path.iter().rev() {
            let origin = self.origins[position];
            if origin > run.start && breaks.last().is_none_or(|&last| origin > last) {
                breaks.push(origin);
            }
        }
    }
}

pub(crate) fn cj_breaks(text: &str, segment: Range<usize>, breaks: &mut Vec<usize>) {
    let mut lattice = Lattice::default();
    let mut at = segment.start;
    while let Some((offset, _)) = text[at..segment.end]
        .char_indices()
        .find(|&(_, character)| is_cj(character))
    {
        let start = at + offset;
        let end = text[start..segment.end]
            .char_indices()
            .find(|&(_, character)| !is_cj(character))
            .map_or(segment.end, |(offset, _)| start + offset);
        lattice.normalize(text, start..end);
        lattice.solve();
        lattice.breaks(start..end, breaks);
        at = end;
    }
}
