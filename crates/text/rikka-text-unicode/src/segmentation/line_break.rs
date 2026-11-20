use std::iter::FusedIterator;

use super::dictionary::DictionaryCursor;
use crate::properties::line_break_column;
use crate::tables::line_break::{
    LINE_BREAK_ABSORBING, LINE_BREAK_AFTER, LINE_BREAK_AHEAD, LINE_BREAK_AHEAD_KEYS,
    LINE_BREAK_AHEAD_TABLES, LINE_BREAK_ASCII, LINE_BREAK_ATTACHING, LINE_BREAK_BEYOND,
    LINE_BREAK_BEYOND_BASE, LINE_BREAK_BMP_BLOCKS, LINE_BREAK_BMP_LEAVES, LINE_BREAK_COLUMN_COUNT,
    LINE_BREAK_FIRST_COMPLEX_COLUMN, LINE_BREAK_START, LINE_BREAK_TRANSITIONS,
};

const ALLOWED: usize = 1;
const MANDATORY: usize = 2;
const DECISION_BITS: u32 = 2;
const DECISION_MASK: usize = 3;
const LOOKAHEAD_ROW: usize = LINE_BREAK_COLUMN_COUNT + 1;
const BATCH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BreakOpportunity {
    Mandatory,
    Allowed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LineBreak {
    pub offset: usize,
    pub opportunity: BreakOpportunity,
}

#[derive(Clone, Debug)]
pub struct LineBreaks<'a> {
    text: &'a str,
    position: usize,
    row: usize,
    finished: bool,
    complex_column: usize,
    dictionary: Option<Box<DictionaryCursor>>,
    batch: [usize; BATCH],
    batched: usize,
    taken: usize,
}

pub fn line_breaks(text: &str) -> LineBreaks<'_> {
    LineBreaks::with_complex_column(text, LINE_BREAK_FIRST_COMPLEX_COLUMN)
}

impl<'a> LineBreaks<'a> {
    pub fn untailored(text: &'a str) -> Self {
        Self::with_complex_column(text, usize::MAX)
    }

    fn with_complex_column(text: &'a str, complex_column: usize) -> Self {
        Self {
            text,
            position: 0,
            row: usize::from(LINE_BREAK_START),
            finished: text.is_empty(),
            complex_column,
            dictionary: None,
            batch: [0; BATCH],
            batched: 0,
            taken: 0,
        }
    }
}

#[inline(always)]
fn column_at(text: &[u8], position: usize) -> (usize, usize) {
    let lead = text[position];
    if lead < 0x80 {
        return (usize::from(LINE_BREAK_ASCII[usize::from(lead)]), 1);
    }
    let continuation = |offset: usize| usize::from(text[position + offset] & 0x3F);
    if lead < 0xE0 {
        return (bmp_column(usize::from(lead & 0x1F), continuation(1)), 2);
    }
    if lead < 0xF0 {
        let block = usize::from(lead & 0x0F) << 6 | continuation(1);
        return (bmp_column(block, continuation(2)), 3);
    }
    let codepoint = usize::from(lead & 0x07) << 18
        | continuation(1) << 12
        | continuation(2) << 6
        | continuation(3);
    (usize::from(line_break_column(codepoint as u32)), 4)
}

#[inline(always)]
fn bmp_column(block: usize, offset: usize) -> usize {
    usize::from(LINE_BREAK_BMP_LEAVES[usize::from(LINE_BREAK_BMP_BLOCKS[block]) + offset])
}

fn following_unit(text: &[u8], mut position: usize, unit: usize) -> Option<(usize, usize)> {
    if LINE_BREAK_ABSORBING[unit] {
        while position < text.len() {
            let (column, length) = column_at(text, position);
            if !LINE_BREAK_ATTACHING[column] {
                break;
            }
            position += length;
        }
    }
    (position < text.len()).then(|| {
        let (column, length) = column_at(text, position);
        (column, position + length)
    })
}

#[cold]
fn decide_ahead(text: &[u8], transition: usize, after_next: usize, next: usize) -> usize {
    let slot = LINE_BREAK_AHEAD_KEYS.partition_point(|&key| (key as usize) < transition);
    let table = usize::from(LINE_BREAK_AHEAD_TABLES[slot]);
    let after = following_unit(text, after_next, next);
    let column = after.map_or(LINE_BREAK_COLUMN_COUNT, |(column, _)| column);
    let decision = LINE_BREAK_AFTER[table * LOOKAHEAD_ROW + column];
    let Some(beyond_table) = decision.checked_sub(LINE_BREAK_BEYOND_BASE) else {
        return usize::from(decision);
    };
    let beyond = after.and_then(|(column, end)| following_unit(text, end, column));
    let column = beyond.map_or(LINE_BREAK_COLUMN_COUNT, |(column, _)| column);
    usize::from(LINE_BREAK_BEYOND[usize::from(beyond_table) * LOOKAHEAD_ROW + column])
}

impl LineBreaks<'_> {
    #[inline(never)]
    fn complex_decision(&mut self, position: usize, decision: usize) -> usize {
        let breaks = self
            .dictionary
            .get_or_insert_default()
            .breaks_at(self.text, position);
        if breaks && decision == 0 {
            ALLOWED
        } else {
            decision
        }
    }

    fn refill(&mut self) {
        let text = self.text.as_bytes();
        let complex_column = self.complex_column;
        let mut position = self.position;
        let mut row = self.row;
        let mut batched = 0;
        while batched < BATCH && position < text.len() {
            let (column, length) = column_at(text, position);
            let transition = row + column;
            let entry = usize::from(LINE_BREAK_TRANSITIONS[transition]);
            row = entry >> DECISION_BITS;
            let mut decision = entry & DECISION_MASK;
            if decision == usize::from(LINE_BREAK_AHEAD) {
                decision = decide_ahead(text, transition, position + length, column);
            }
            if column >= complex_column {
                decision = self.complex_decision(position, decision);
            }
            self.batch[batched] = position << DECISION_BITS | decision;
            batched += usize::from(decision != 0);
            position += length;
        }
        if batched < BATCH && position == text.len() && !self.finished {
            self.finished = true;
            self.batch[batched] = position << DECISION_BITS | MANDATORY;
            batched += 1;
        }
        self.position = position;
        self.row = row;
        self.batched = batched;
        self.taken = 0;
    }
}

impl Iterator for LineBreaks<'_> {
    type Item = LineBreak;

    #[inline]
    fn next(&mut self) -> Option<LineBreak> {
        if self.taken == self.batched {
            if self.finished && self.position == self.text.len() {
                return None;
            }
            self.refill();
            if self.batched == 0 {
                return None;
            }
        }
        let entry = self.batch[self.taken];
        self.taken += 1;
        Some(LineBreak {
            offset: entry >> DECISION_BITS,
            opportunity: match entry & DECISION_MASK {
                MANDATORY => BreakOpportunity::Mandatory,
                _ => BreakOpportunity::Allowed,
            },
        })
    }
}

impl FusedIterator for LineBreaks<'_> {}
