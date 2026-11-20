use super::classes::strong_direction;
use crate::mirroring::paired_bracket;
use crate::normalization::decompose;
use crate::properties::BracketType;
use crate::tables::enums::BidiClass;

const MAX_OPEN_BRACKETS: usize = 63;

pub(crate) struct Sequence<'a> {
    pub(crate) indices: &'a [u32],
    pub(crate) characters: &'a [char],
    pub(crate) original: &'a [BidiClass],
    pub(crate) classes: &'a mut [BidiClass],
    pub(crate) embedding: BidiClass,
    pub(crate) start_of_sequence: BidiClass,
}

impl Sequence<'_> {
    #[inline]
    fn class(&self, position: usize) -> BidiClass {
        self.classes[self.indices[position] as usize]
    }

    pub(crate) fn resolve_brackets(&mut self, pairs: &mut Vec<(u32, u32)>) {
        self.identify_pairs(pairs);
        for &(open, close) in pairs.iter() {
            self.resolve_pair(open as usize, close as usize);
        }
    }

    fn identify_pairs(&self, pairs: &mut Vec<(u32, u32)>) {
        pairs.clear();
        let mut stack = [('\0', 0u32); MAX_OPEN_BRACKETS];
        let mut depth = 0;
        for (position, &index) in self.indices.iter().enumerate() {
            if self.classes[index as usize] != BidiClass::OtherNeutral {
                continue;
            }
            let character = self.characters[index as usize];
            let Some(bracket) = paired_bracket(character) else {
                continue;
            };
            match bracket.kind {
                BracketType::Open => {
                    if depth == MAX_OPEN_BRACKETS {
                        break;
                    }
                    stack[depth] = (canonical(bracket.pair), position as u32);
                    depth += 1;
                }
                BracketType::Close => {
                    let closing = canonical(character);
                    if let Some(found) = stack[..depth]
                        .iter()
                        .rposition(|&(pair, _)| pair == closing)
                    {
                        pairs.push((stack[found].1, position as u32));
                        depth = found;
                    }
                }
            }
        }
        pairs.sort_unstable();
    }

    fn resolve_pair(&mut self, open: usize, close: usize) {
        let embedding = self.embedding;
        let mut opposite_inside = false;
        for position in open + 1..close {
            match strong_direction(self.class(position)) {
                Some(direction) if direction == embedding => {
                    self.set_pair(open, close, embedding);
                    return;
                }
                Some(_) => opposite_inside = true,
                None => {}
            }
        }
        if !opposite_inside {
            return;
        }
        let context = (0..open)
            .rev()
            .find_map(|position| strong_direction(self.class(position)))
            .unwrap_or(self.start_of_sequence);
        self.set_pair(open, close, context);
    }

    fn set_pair(&mut self, open: usize, close: usize, direction: BidiClass) {
        for bracket in [open, close] {
            self.classes[self.indices[bracket] as usize] = direction;
            for &index in &self.indices[bracket + 1..] {
                if self.original[index as usize] != BidiClass::NonspacingMark {
                    break;
                }
                self.classes[index as usize] = direction;
            }
        }
    }
}

fn canonical(character: char) -> char {
    match decompose(character) {
        Some((single, None)) => single,
        _ => character,
    }
}
