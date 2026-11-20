use std::iter::FusedIterator;
use std::ops::Range;

use super::dictionary::{DictionaryCursor, cj_breaks, is_cj};
use super::scanner::Scanner;
use crate::properties::{Properties, general_category};
use crate::tables::enums::{LineBreakClass, WordBreak};

const FIRST_COMPLEX_CONTEXT: char = '\u{0E01}';

const PICTOGRAPHIC: u8 = 1;
const JOINER_END: u8 = 1 << 1;
const LONE_SPACE: u8 = 1 << 2;
const ALPHANUMERIC: u8 = 1 << 3;
const COMPLEX: u8 = 1 << 4;
const CJ: u8 = 1 << 5;

#[inline(always)]
fn flag(set: bool, flag: u8) -> u8 {
    u8::from(set) * flag
}

#[derive(Clone, Copy, Debug)]
struct Unit {
    start: usize,
    class: WordBreak,
    flags: u8,
}

impl Unit {
    #[inline(always)]
    fn has(self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum WordRule {
    #[default]
    Prose,
    Code,
}

#[derive(Clone, Debug)]
struct Units<'a> {
    scanner: Scanner<'a>,
    rule: Option<WordRule>,
}

impl Units<'_> {
    #[inline(always)]
    fn is_complex(&self, character: char, properties: Properties) -> bool {
        character >= FIRST_COMPLEX_CONTEXT
            && self.rule.is_some()
            && properties.line_break_class() == LineBreakClass::ComplexContext
    }
}

impl Iterator for Units<'_> {
    type Item = Unit;

    #[inline(always)]
    fn next(&mut self) -> Option<Unit> {
        let (start, character, properties) = self.scanner.next()?;
        let mut class = properties.word_break();
        let category = properties.general_category();
        let complex = self.is_complex(character, properties);
        if complex
            && !matches!(
                class,
                WordBreak::Extend | WordBreak::Format | WordBreak::Zwj
            )
        {
            class = WordBreak::ALetter;
        }
        let cj = self.rule.is_some()
            && is_cj(character)
            && !matches!(
                class,
                WordBreak::Extend | WordBreak::Format | WordBreak::Zwj
            );
        if cj {
            class = WordBreak::Katakana;
        }
        if self.rule == Some(WordRule::Code) && is_infix(class) {
            class = WordBreak::Other;
        }
        let mut joiner_end = class == WordBreak::Zwj;
        let mut absorbed = false;
        if !matches!(class, WordBreak::Cr | WordBreak::Lf | WordBreak::Newline) {
            while let Some((_, ignored)) = self.scanner.next_if(|properties| {
                matches!(
                    properties.word_break(),
                    WordBreak::Extend | WordBreak::Format | WordBreak::Zwj
                )
            }) {
                joiner_end = ignored.word_break() == WordBreak::Zwj;
                absorbed = true;
            }
        }
        Some(Unit {
            start,
            class,
            flags: flag(properties.is_extended_pictographic(), PICTOGRAPHIC)
                | flag(joiner_end, JOINER_END)
                | flag(class == WordBreak::WSegSpace && !absorbed, LONE_SPACE)
                | flag(category.is_letter() || category.is_number(), ALPHANUMERIC)
                | flag(complex, COMPLEX)
                | flag(cj, CJ),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordSegment<'a> {
    pub start: usize,
    pub text: &'a str,
    pub is_word: bool,
}

#[derive(Clone, Debug)]
pub struct WordSegments<'a> {
    text: &'a str,
    units: Units<'a>,
    previous: Option<Unit>,
    current: Option<Unit>,
    next: Option<Unit>,
    after: Option<Unit>,
    odd_regional_indicators: bool,
    dictionary: Option<Box<DictionaryCursor>>,
    divided: Option<Range<usize>>,
    pieces: Vec<usize>,
    piece_start: usize,
}

pub fn word_segments(text: &str) -> WordSegments<'_> {
    WordSegments::new(text, WordRule::Prose)
}

impl<'a> WordSegments<'a> {
    pub fn new(text: &'a str, rule: WordRule) -> Self {
        Self::with_rule(text, Some(rule))
    }

    pub fn untailored(text: &'a str) -> Self {
        Self::with_rule(text, None)
    }

    fn with_rule(text: &'a str, rule: Option<WordRule>) -> Self {
        let mut units = Units {
            scanner: Scanner::new(text),
            rule,
        };
        let current = units.next();
        let next = units.next();
        let after = units.next();
        Self {
            text,
            units,
            previous: None,
            current,
            next,
            after,
            odd_regional_indicators: current
                .is_some_and(|unit| unit.class == WordBreak::RegionalIndicator),
            dictionary: None,
            divided: None,
            pieces: Vec::new(),
            piece_start: 0,
        }
    }

    fn advance(&mut self) {
        self.previous = self.current;
        self.current = self.next;
        self.next = self.after;
        self.after = self.units.next();
        self.odd_regional_indicators = self.current.is_some_and(|unit| {
            unit.class == WordBreak::RegionalIndicator && !self.odd_regional_indicators
        });
    }

    #[inline(never)]
    fn divide(&mut self, start: usize, end: usize) -> WordSegment<'a> {
        let piece_end = match self
            .dictionary
            .get_or_insert_default()
            .first_break_between(self.text, start, end)
        {
            Some(boundary) => {
                self.divided = Some(boundary..end);
                boundary
            }
            None => end,
        };
        let is_word = has_alphanumeric(&self.text[start..piece_end]);
        self.segment(start, piece_end, is_word)
    }

    #[inline(never)]
    fn divide_cj(&mut self, start: usize, end: usize) -> Option<WordSegment<'a>> {
        self.pieces.clear();
        cj_breaks(self.text, start..end, &mut self.pieces);
        self.pieces.push(end);
        self.pieces.reverse();
        self.piece_start = start;
        self.next_piece()
    }

    fn next_piece(&mut self) -> Option<WordSegment<'a>> {
        let end = self.pieces.pop()?;
        let start = std::mem::replace(&mut self.piece_start, end);
        let is_word = has_alphanumeric(&self.text[start..end]);
        Some(self.segment(start, end, is_word))
    }

    fn segment(&self, start: usize, end: usize, is_word: bool) -> WordSegment<'a> {
        WordSegment {
            start,
            text: &self.text[start..end],
            is_word,
        }
    }
}

impl<'a> Iterator for WordSegments<'a> {
    type Item = WordSegment<'a>;

    fn next(&mut self) -> Option<WordSegment<'a>> {
        if let Some(remainder) = self.divided.take() {
            return Some(self.divide(remainder.start, remainder.end));
        }
        if let Some(piece) = self.next_piece() {
            return Some(piece);
        }
        let first = self.current?;
        let mut combined_flags = first.flags;
        let end = loop {
            let current = self.current?;
            let Some(next) = self.next else {
                self.advance();
                break self.text.len();
            };
            let joined = joins(
                self.previous.map(|unit| unit.class),
                current,
                next,
                self.after.map(|unit| unit.class),
                self.odd_regional_indicators,
            );
            self.advance();
            if !joined {
                break next.start;
            }
            combined_flags |= next.flags;
        };
        if combined_flags & CJ != 0 {
            return self.divide_cj(first.start, end);
        }
        if combined_flags & COMPLEX != 0 {
            return Some(self.divide(first.start, end));
        }
        Some(self.segment(first.start, end, combined_flags & ALPHANUMERIC != 0))
    }
}

impl FusedIterator for WordSegments<'_> {}

fn has_alphanumeric(text: &str) -> bool {
    text.chars().any(|character| {
        let category = general_category(character);
        category.is_letter() || category.is_number()
    })
}

#[inline]
fn is_letter(class: WordBreak) -> bool {
    matches!(class, WordBreak::ALetter | WordBreak::HebrewLetter)
}

#[inline]
fn is_mid_letter(class: WordBreak) -> bool {
    matches!(
        class,
        WordBreak::MidLetter | WordBreak::MidNumLet | WordBreak::SingleQuote
    )
}

#[inline]
fn is_mid_number(class: WordBreak) -> bool {
    matches!(
        class,
        WordBreak::MidNum | WordBreak::MidNumLet | WordBreak::SingleQuote
    )
}

#[inline]
fn is_infix(class: WordBreak) -> bool {
    is_mid_letter(class) || is_mid_number(class) || class == WordBreak::DoubleQuote
}

fn joins(
    previous: Option<WordBreak>,
    current: Unit,
    next: Unit,
    after: Option<WordBreak>,
    odd_regional_indicators: bool,
) -> bool {
    use WordBreak as W;
    let (left, right) = (current.class, next.class);
    match (left, right) {
        (W::Cr, W::Lf) => return true,
        (W::Cr | W::Lf | W::Newline, _) | (_, W::Cr | W::Lf | W::Newline) => return false,
        _ => {}
    }
    if current.has(JOINER_END) && next.has(PICTOGRAPHIC) {
        return true;
    }
    if current.has(LONE_SPACE) && right == W::WSegSpace {
        return true;
    }
    match (left, right) {
        (W::HebrewLetter, W::SingleQuote) => true,
        (W::HebrewLetter, W::DoubleQuote) => after == Some(W::HebrewLetter),
        (W::DoubleQuote, W::HebrewLetter) => previous == Some(W::HebrewLetter),
        (W::Numeric, W::Numeric) | (W::Katakana, W::Katakana) => true,
        (W::RegionalIndicator, W::RegionalIndicator) => odd_regional_indicators,
        (W::ExtendNumLet, W::ExtendNumLet) => true,
        (W::ExtendNumLet, right) => is_letter(right) || matches!(right, W::Numeric | W::Katakana),
        (left, W::ExtendNumLet) => is_letter(left) || matches!(left, W::Numeric | W::Katakana),
        (left, right) if is_letter(left) && is_letter(right) => true,
        (left, right)
            if is_letter(left) && is_mid_letter(right) && after.is_some_and(is_letter) =>
        {
            true
        }
        (left, right)
            if is_mid_letter(left) && is_letter(right) && previous.is_some_and(is_letter) =>
        {
            true
        }
        (left, W::Numeric) if is_letter(left) => true,
        (W::Numeric, right) if is_letter(right) => true,
        (left, W::Numeric) if is_mid_number(left) => previous == Some(W::Numeric),
        (W::Numeric, right) if is_mid_number(right) => after == Some(W::Numeric),
        _ => false,
    }
}
