use std::ops::Range;

use super::brackets::Sequence;
use super::classes::{
    ClassSet, ISOLATE_CONTROLS, ISOLATE_INITIATORS, NEEDS_RESOLUTION, NEUTRAL_OR_ISOLATE, REMOVED,
    strong_direction,
};
use super::level::{BaseDirection, Level};
use crate::properties::properties;
use crate::tables::enums::BidiClass;

use BidiClass as B;

const NONE: u32 = u32::MAX;
const STACK_CAPACITY: usize = Level::MAX_EXPLICIT as usize + 2;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BidiParagraph {
    pub range: Range<usize>,
    pub level: Level,
}

#[derive(Clone, Copy, Debug)]
struct Status {
    level: u8,
    override_class: Option<BidiClass>,
    isolate: bool,
}

#[derive(Debug, Default)]
pub struct Bidi {
    paragraphs: Vec<BidiParagraph>,
    levels: Vec<Level>,
    pub(super) characters: Vec<char>,
    pub(super) offsets: Vec<usize>,
    pub(super) original: Vec<BidiClass>,
    pub(super) resolved: Vec<u8>,
    classes: Vec<BidiClass>,
    embedding_levels: Vec<u8>,
    matching: Vec<u32>,
    pending: Vec<u32>,
    runs: Vec<(u32, u32)>,
    links: Vec<u32>,
    continuation: Vec<bool>,
    sequence: Vec<u32>,
    pairs: Vec<(u32, u32)>,
    text_len: usize,
    mixed: bool,
    uniform: bool,
}

impl Bidi {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn analyze(&mut self, text: &str, direction: BaseDirection) {
        self.paragraphs.clear();
        self.levels.clear();
        self.characters.clear();
        self.offsets.clear();
        self.original.clear();
        self.text_len = text.len();
        self.uniform = direction != BaseDirection::RightToLeft && self.analyze_uniform(text);
        if self.uniform {
            return;
        }
        let mut present = 0u32;
        for (offset, character) in text.char_indices() {
            let class = properties(character).bidi_class();
            present |= ClassSet::bit(class);
            self.characters.push(character);
            self.offsets.push(offset);
            self.original.push(class);
        }
        let count = self.characters.len();
        self.classes.clear();
        self.classes.extend_from_slice(&self.original);
        self.resolved.clear();
        self.resolved.resize(count, 0);
        self.matching.clear();
        self.matching.resize(count, NONE);
        let simple = !NEEDS_RESOLUTION.intersects(present);
        let mut start = 0;
        while start < count {
            let end = self.original[start..]
                .iter()
                .position(|&class| class == B::ParagraphSeparator)
                .map_or(count, |position| start + position + 1);
            let level = self.resolve_paragraph(start, end, direction, simple);
            let byte_end = self.offsets.get(end).copied().unwrap_or(text.len());
            self.paragraphs.push(BidiParagraph {
                range: self.offsets[start]..byte_end,
                level: Level::from_resolved(level),
            });
            start = end;
        }
        self.mixed = self.resolved.iter().any(|&level| level != 0);
        self.levels.reserve(text.len());
        for (index, &level) in self.resolved.iter().enumerate() {
            let end = self.offsets.get(index + 1).copied().unwrap_or(text.len());
            let width = end - self.offsets[index];
            self.levels
                .extend(std::iter::repeat_n(Level::from_resolved(level), width));
        }
    }

    fn analyze_uniform(&mut self, text: &str) -> bool {
        let mut start = 0;
        for (offset, character) in text.char_indices() {
            let class = properties(character).bidi_class();
            if NEEDS_RESOLUTION.has(class) {
                self.paragraphs.clear();
                return false;
            }
            if class == B::ParagraphSeparator {
                let end = offset + character.len_utf8();
                self.paragraphs.push(BidiParagraph {
                    range: start..end,
                    level: Level::LEFT_TO_RIGHT,
                });
                start = end;
            }
        }
        if start < text.len() {
            self.paragraphs.push(BidiParagraph {
                range: start..text.len(),
                level: Level::LEFT_TO_RIGHT,
            });
        }
        self.levels.resize(text.len(), Level::LEFT_TO_RIGHT);
        self.mixed = false;
        true
    }

    pub fn paragraphs(&self) -> &[BidiParagraph] {
        &self.paragraphs
    }

    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    pub fn is_left_to_right(&self) -> bool {
        !self.mixed
    }

    pub(super) fn is_uniform(&self) -> bool {
        self.uniform
    }

    pub(super) fn text_len(&self) -> usize {
        self.text_len
    }

    pub(super) fn paragraph_level(&self, offset: usize) -> u8 {
        let index = self
            .paragraphs
            .partition_point(|paragraph| paragraph.range.end <= offset)
            .min(self.paragraphs.len().saturating_sub(1));
        self.paragraphs
            .get(index)
            .map_or(0, |paragraph| paragraph.level.number())
    }

    fn resolve_paragraph(
        &mut self,
        start: usize,
        end: usize,
        direction: BaseDirection,
        simple: bool,
    ) -> u8 {
        self.match_isolates(start, end);
        let level = match direction {
            BaseDirection::LeftToRight => 0,
            BaseDirection::RightToLeft => 1,
            BaseDirection::Auto => u8::from(self.first_strong(start, end) == Some(B::RightToLeft)),
        };
        if simple && level == 0 {
            return 0;
        }
        self.resolve_explicit(start, end, level);
        self.embedding_levels.clear();
        self.embedding_levels
            .extend_from_slice(&self.resolved[start..end]);
        self.resolve_runs(start, end, level);
        for index in start..end {
            if REMOVED.has(self.classes[index]) {
                self.resolved[index] = if index > start {
                    self.resolved[index - 1]
                } else {
                    level
                };
            }
        }
        level
    }

    fn match_isolates(&mut self, start: usize, end: usize) {
        self.pending.clear();
        for index in start..end {
            match self.original[index] {
                class if ISOLATE_INITIATORS.has(class) => self.pending.push(index as u32),
                B::PopDirectionalIsolate => {
                    if let Some(initiator) = self.pending.pop() {
                        self.matching[initiator as usize] = index as u32;
                        self.matching[index] = initiator;
                    }
                }
                _ => {}
            }
        }
    }

    fn first_strong(&self, start: usize, end: usize) -> Option<BidiClass> {
        let mut index = start;
        while index < end {
            match self.original[index] {
                B::LeftToRight => return Some(B::LeftToRight),
                B::RightToLeft | B::ArabicLetter => return Some(B::RightToLeft),
                class if ISOLATE_INITIATORS.has(class) => {
                    let matching = self.matching[index];
                    if matching == NONE {
                        return None;
                    }
                    index = matching as usize;
                }
                _ => {}
            }
            index += 1;
        }
        None
    }

    fn resolve_explicit(&mut self, start: usize, end: usize, paragraph_level: u8) {
        let mut stack = [Status {
            level: paragraph_level,
            override_class: None,
            isolate: false,
        }; STACK_CAPACITY];
        let mut depth = 1;
        let mut overflow_isolates = 0u32;
        let mut overflow_embeddings = 0u32;
        let mut valid_isolates = 0u32;
        for index in start..end {
            let class = self.original[index];
            let top = stack[depth - 1];
            match class {
                B::RightToLeftEmbedding
                | B::LeftToRightEmbedding
                | B::RightToLeftOverride
                | B::LeftToRightOverride => {
                    let rtl = matches!(class, B::RightToLeftEmbedding | B::RightToLeftOverride);
                    let level = next_level(top.level, rtl);
                    if level <= Level::MAX_EXPLICIT
                        && overflow_isolates == 0
                        && overflow_embeddings == 0
                    {
                        stack[depth] = Status {
                            level,
                            override_class: match class {
                                B::RightToLeftOverride => Some(B::RightToLeft),
                                B::LeftToRightOverride => Some(B::LeftToRight),
                                _ => None,
                            },
                            isolate: false,
                        };
                        depth += 1;
                    } else if overflow_isolates == 0 {
                        overflow_embeddings += 1;
                    }
                    self.resolved[index] = top.level;
                    self.classes[index] = B::BoundaryNeutral;
                }
                B::RightToLeftIsolate | B::LeftToRightIsolate | B::FirstStrongIsolate => {
                    self.resolved[index] = top.level;
                    self.classes[index] = top.override_class.unwrap_or(class);
                    let rtl = match class {
                        B::RightToLeftIsolate => true,
                        B::LeftToRightIsolate => false,
                        _ => {
                            let matching = self.matching[index];
                            let limit = if matching == NONE {
                                end
                            } else {
                                matching as usize
                            };
                            self.first_strong(index + 1, limit) == Some(B::RightToLeft)
                        }
                    };
                    let level = next_level(top.level, rtl);
                    if level <= Level::MAX_EXPLICIT
                        && overflow_isolates == 0
                        && overflow_embeddings == 0
                    {
                        valid_isolates += 1;
                        stack[depth] = Status {
                            level,
                            override_class: None,
                            isolate: true,
                        };
                        depth += 1;
                    } else {
                        overflow_isolates += 1;
                    }
                }
                B::PopDirectionalIsolate => {
                    if overflow_isolates > 0 {
                        overflow_isolates -= 1;
                    } else if valid_isolates > 0 {
                        overflow_embeddings = 0;
                        while !stack[depth - 1].isolate {
                            depth -= 1;
                        }
                        depth -= 1;
                        valid_isolates -= 1;
                    }
                    let top = stack[depth - 1];
                    self.resolved[index] = top.level;
                    self.classes[index] = top.override_class.unwrap_or(class);
                }
                B::PopDirectionalFormat => {
                    if overflow_isolates > 0 {
                    } else if overflow_embeddings > 0 {
                        overflow_embeddings -= 1;
                    } else if !top.isolate && depth >= 2 {
                        depth -= 1;
                    }
                    self.resolved[index] = top.level;
                    self.classes[index] = B::BoundaryNeutral;
                }
                B::ParagraphSeparator => {
                    self.resolved[index] = paragraph_level;
                }
                B::BoundaryNeutral => {
                    self.resolved[index] = top.level;
                }
                _ => {
                    self.resolved[index] = top.level;
                    self.classes[index] = top.override_class.unwrap_or(class);
                }
            }
        }
    }

    fn resolve_runs(&mut self, start: usize, end: usize, paragraph_level: u8) {
        self.runs.clear();
        for index in start..end {
            if REMOVED.has(self.classes[index]) {
                continue;
            }
            match self.runs.last_mut() {
                Some(run)
                    if self.embedding_levels[run.1 as usize - start]
                        == self.embedding_levels[index - start] =>
                {
                    run.1 = index as u32;
                }
                _ => self.runs.push((index as u32, index as u32)),
            }
        }
        self.links.clear();
        self.links.resize(self.runs.len(), NONE);
        self.continuation.clear();
        self.continuation.resize(self.runs.len(), false);
        for run in 0..self.runs.len() {
            let last = self.runs[run].1 as usize;
            let matching = self.matching[last];
            if !ISOLATE_INITIATORS.has(self.original[last]) || matching == NONE {
                continue;
            }
            if let Ok(next) = self.runs.binary_search_by_key(&matching, |run| run.0) {
                self.links[run] = next as u32;
                self.continuation[next] = true;
            }
        }
        for run in 0..self.runs.len() {
            if self.continuation[run] {
                continue;
            }
            self.sequence.clear();
            let mut current = run;
            loop {
                let (first, last) = self.runs[current];
                for index in first..=last {
                    if !REMOVED.has(self.classes[index as usize]) {
                        self.sequence.push(index);
                    }
                }
                match self.links[current] {
                    NONE => break,
                    next => current = next as usize,
                }
            }
            self.resolve_sequence(start, end, paragraph_level);
        }
    }

    fn resolve_sequence(&mut self, start: usize, end: usize, paragraph_level: u8) {
        let first = self.sequence[0] as usize;
        let last = self.sequence[self.sequence.len() - 1] as usize;
        let level = self.embedding_levels[first - start];
        let before = (start..first)
            .rev()
            .find(|&index| !REMOVED.has(self.classes[index]))
            .map_or(paragraph_level, |index| {
                self.embedding_levels[index - start]
            });
        let after = if ISOLATE_INITIATORS.has(self.original[last]) {
            paragraph_level
        } else {
            (last + 1..end)
                .find(|&index| !REMOVED.has(self.classes[index]))
                .map_or(paragraph_level, |index| {
                    self.embedding_levels[index - start]
                })
        };
        let start_of_sequence = direction_of(level.max(before));
        let end_of_sequence = direction_of(level.max(after));
        let embedding = direction_of(level);

        self.resolve_weak(start_of_sequence);
        let mut sequence = Sequence {
            indices: &self.sequence,
            characters: &self.characters,
            original: &self.original,
            classes: &mut self.classes,
            embedding,
            start_of_sequence,
        };
        sequence.resolve_brackets(&mut self.pairs);
        self.resolve_neutral(start_of_sequence, end_of_sequence, embedding);
        for &index in &self.sequence {
            let index = index as usize;
            let level = self.resolved[index];
            self.resolved[index] += match (level & 1, self.classes[index]) {
                (0, B::RightToLeft) => 1,
                (0, B::ArabicNumber | B::EuropeanNumber) => 2,
                (1, B::LeftToRight | B::ArabicNumber | B::EuropeanNumber) => 1,
                _ => 0,
            };
        }
    }

    fn resolve_weak(&mut self, start_of_sequence: BidiClass) {
        let classes = &mut self.classes;
        let sequence = &self.sequence;
        let mut previous = start_of_sequence;
        let mut last_strong = start_of_sequence;
        for &index in sequence {
            let class = &mut classes[index as usize];
            if *class == B::NonspacingMark {
                *class = if ISOLATE_CONTROLS.has(previous) {
                    B::OtherNeutral
                } else {
                    previous
                };
            }
            match *class {
                B::LeftToRight | B::RightToLeft | B::ArabicLetter => last_strong = *class,
                B::EuropeanNumber if last_strong == B::ArabicLetter => *class = B::ArabicNumber,
                _ => {}
            }
            previous = *class;
        }
        for &index in sequence {
            if classes[index as usize] == B::ArabicLetter {
                classes[index as usize] = B::RightToLeft;
            }
        }
        for position in 1..sequence.len().saturating_sub(1) {
            let class = classes[sequence[position] as usize];
            if !matches!(class, B::EuropeanSeparator | B::CommonSeparator) {
                continue;
            }
            let before = classes[sequence[position - 1] as usize];
            let after = classes[sequence[position + 1] as usize];
            if before == after
                && (before == B::EuropeanNumber
                    || (class == B::CommonSeparator && before == B::ArabicNumber))
            {
                classes[sequence[position] as usize] = before;
            }
        }
        let mut position = 0;
        while position < sequence.len() {
            if classes[sequence[position] as usize] != B::EuropeanTerminator {
                position += 1;
                continue;
            }
            let run_start = position;
            while position < sequence.len()
                && classes[sequence[position] as usize] == B::EuropeanTerminator
            {
                position += 1;
            }
            let touches_number = (run_start > 0
                && classes[sequence[run_start - 1] as usize] == B::EuropeanNumber)
                || (position < sequence.len()
                    && classes[sequence[position] as usize] == B::EuropeanNumber);
            if touches_number {
                for &index in &sequence[run_start..position] {
                    classes[index as usize] = B::EuropeanNumber;
                }
            }
        }
        let mut last_strong = start_of_sequence;
        for &index in sequence {
            let class = &mut classes[index as usize];
            match *class {
                B::EuropeanSeparator | B::EuropeanTerminator | B::CommonSeparator => {
                    *class = B::OtherNeutral;
                }
                B::LeftToRight | B::RightToLeft => last_strong = *class,
                B::EuropeanNumber if last_strong == B::LeftToRight => *class = B::LeftToRight,
                _ => {}
            }
        }
    }

    fn resolve_neutral(
        &mut self,
        start_of_sequence: BidiClass,
        end_of_sequence: BidiClass,
        embedding: BidiClass,
    ) {
        let classes = &mut self.classes;
        let sequence = &self.sequence;
        let mut position = 0;
        while position < sequence.len() {
            if !NEUTRAL_OR_ISOLATE.has(classes[sequence[position] as usize]) {
                position += 1;
                continue;
            }
            let run_start = position;
            while position < sequence.len()
                && NEUTRAL_OR_ISOLATE.has(classes[sequence[position] as usize])
            {
                position += 1;
            }
            let before = match run_start {
                0 => Some(start_of_sequence),
                _ => strong_direction(classes[sequence[run_start - 1] as usize]),
            };
            let after = match sequence.get(position) {
                None => Some(end_of_sequence),
                Some(&index) => strong_direction(classes[index as usize]),
            };
            let direction = if before == after {
                before.unwrap_or(embedding)
            } else {
                embedding
            };
            for &index in &sequence[run_start..position] {
                classes[index as usize] = direction;
            }
        }
    }
}

#[inline]
fn next_level(level: u8, rtl: bool) -> u8 {
    if rtl {
        (level + 1) | 1
    } else {
        (level + 2) & !1
    }
}

#[inline]
fn direction_of(level: u8) -> BidiClass {
    if level & 1 == 1 {
        B::RightToLeft
    } else {
        B::LeftToRight
    }
}
