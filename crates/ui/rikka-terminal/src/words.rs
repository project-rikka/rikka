use std::ops::{Range, RangeInclusive};

use alacritty_terminal::Term;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line, Point};
use alacritty_terminal::term::cell::Flags;
use rikka_text::{Affinity, WordRule, word_range_at};

const SCHEME_SEPARATOR: &str = "://";
const OPENERS: &[char] = &['(', '[', '{', '<', '"', '\'', '`'];
const TRAILERS: &[char] = &['.', ',', ';', ':', '!', '?', '"', '\'', '`'];
const PAIRS: &[(char, char)] = &[('(', ')'), ('[', ']'), ('{', '}'), ('<', '>')];

pub(crate) fn word_cells<T>(term: &Term<T>, point: Point) -> RangeInclusive<Point> {
    let line = LogicalLine::around(term, point);
    let at = line.byte_of(point);
    let word = word_at(&line.text, at);
    line.cell_at(word.start)..=line.cell_at(word.end.max(word.start + 1) - 1)
}

pub(crate) fn word_at(text: &str, at: usize) -> Range<usize> {
    link_at(text, at).unwrap_or_else(|| word_range_at(text, at, Affinity::After, WordRule::Code))
}

fn link_at(text: &str, at: usize) -> Option<Range<usize>> {
    let token = token_at(text, at)?;
    let found = &text[token.clone()];
    let mut link = match scheme_start(found) {
        Some(start) => token.start + start..token.end,
        None if found.contains('/') => trim_openers(text, token),
        None => return None,
    };
    link.end = trim_trailers(text, link.clone());
    (link.contains(&at) && text[link.clone()].contains('/')).then_some(link)
}

fn token_at(text: &str, at: usize) -> Option<Range<usize>> {
    let clicked = text[at..].chars().next()?;
    if clicked.is_whitespace() {
        return None;
    }
    let start = text[..at]
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map_or(0, |(index, character)| index + character.len_utf8());
    let end = text[at..]
        .char_indices()
        .find(|(_, character)| character.is_whitespace())
        .map_or(text.len(), |(index, _)| at + index);
    Some(start..end)
}

fn scheme_start(token: &str) -> Option<usize> {
    let separator = token.find(SCHEME_SEPARATOR)?;
    let widest = token[..separator]
        .char_indices()
        .rev()
        .take_while(|&(_, character)| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        })
        .last()
        .map(|(index, _)| index)?;
    let start = widest
        + token[widest..separator].find(|character: char| character.is_ascii_alphabetic())?;
    let addressed = separator + SCHEME_SEPARATOR.len() < token.len();
    addressed.then_some(start)
}

fn trim_openers(text: &str, mut token: Range<usize>) -> Range<usize> {
    while let Some(opener) = text[token.clone()]
        .chars()
        .next()
        .filter(|character| OPENERS.contains(character))
    {
        token.start += opener.len_utf8();
    }
    token
}

fn trim_trailers(text: &str, token: Range<usize>) -> usize {
    let mut end = token.end;
    while let Some(last) = text[token.start..end].chars().next_back() {
        let unbalanced = PAIRS.iter().any(|&(open, close)| {
            let inside = &text[token.start..end];
            last == close && inside.matches(close).count() > inside.matches(open).count()
        });
        if !TRAILERS.contains(&last) && !unbalanced {
            break;
        }
        end -= last.len_utf8();
    }
    end
}

struct LogicalLine {
    text: String,
    cells: Vec<(usize, Point)>,
}

impl LogicalLine {
    fn around<T>(term: &Term<T>, point: Point) -> LogicalLine {
        let grid = term.grid();
        let last = term.last_column();
        let wraps = |line: Line| grid[line][last].flags.contains(Flags::WRAPLINE);
        let mut first = point.line;
        while first > term.topmost_line() && wraps(first - 1) {
            first -= 1;
        }
        let mut final_line = point.line;
        while final_line < term.bottommost_line() && wraps(final_line) {
            final_line += 1;
        }

        let mut text = String::new();
        let mut cells = Vec::new();
        let mut line = first;
        while line <= final_line {
            for column in 0..grid.columns() {
                let cell = &grid[line][Column(column)];
                if cell
                    .flags
                    .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
                {
                    continue;
                }
                cells.push((text.len(), Point::new(line, Column(column))));
                text.push(cell.c);
                text.extend(cell.zerowidth().unwrap_or_default());
            }
            line += 1;
        }
        LogicalLine { text, cells }
    }

    fn byte_of(&self, point: Point) -> usize {
        let index = self.cells.partition_point(|&(_, cell)| cell <= point);
        self.cells[index.saturating_sub(1)].0
    }

    fn cell_at(&self, byte: usize) -> Point {
        let index = self.cells.partition_point(|&(start, _)| start <= byte);
        self.cells[index.saturating_sub(1)].1
    }
}
