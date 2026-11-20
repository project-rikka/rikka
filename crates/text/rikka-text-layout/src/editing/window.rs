use std::borrow::Cow;
use std::ops::Range;

use rikka_text_unicode::{GraphemeClusterBreak, IndicConjunctBreak, needs_dictionary, properties};

use crate::editing::text_source::TextSource;

const INITIAL_RADIUS: usize = 256;
const DICTIONARY_RADIUS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Restarts {
    BetweenRuns,
    InsideDictionaryRuns,
}

pub(crate) struct Window<'a> {
    buffer: Cow<'a, str>,
    range: Range<usize>,
    offset: usize,
    is_text_start: bool,
    is_text_end: bool,
}

impl<'a> Window<'a> {
    pub(crate) fn new<S: TextSource + ?Sized>(
        source: &'a S,
        index: usize,
        radius: usize,
    ) -> Option<Self> {
        let len = source.byte_len();
        let requested = index.saturating_sub(radius)..index.saturating_add(radius).min(len);
        let (start, buffer) = source.covering(requested);
        let at = buffer.floor_char_boundary(index - start);
        let is_text_start = start == 0;
        let is_text_end = start + buffer.len() == len;
        let restarts = match radius >= DICTIONARY_RADIUS {
            true => Restarts::InsideDictionaryRuns,
            false => Restarts::BetweenRuns,
        };
        let from = if is_text_start {
            0
        } else {
            first_restart(&buffer, at, restarts)?
        };
        let to = if is_text_end {
            buffer.len()
        } else {
            last_restart(&buffer, at, restarts)?
        };
        Some(Self {
            offset: start + from,
            range: from..to,
            buffer,
            is_text_start,
            is_text_end,
        })
    }

    pub(crate) fn text(&self) -> &str {
        &self.buffer[self.range.clone()]
    }

    pub(crate) fn offset(&self) -> usize {
        self.offset
    }

    pub(crate) fn is_text_start(&self) -> bool {
        self.is_text_start
    }

    pub(crate) fn is_text_end(&self) -> bool {
        self.is_text_end
    }
}

pub(crate) fn find_in_window<S: TextSource + ?Sized, T>(
    source: &S,
    index: usize,
    find: impl Fn(&Window<'_>, usize) -> Option<T>,
) -> T {
    let index = index.min(source.byte_len());
    let mut radius = INITIAL_RADIUS;
    loop {
        if let Some(window) = Window::new(source, index, radius) {
            let at = window.text().floor_char_boundary(index - window.offset());
            if let Some(found) = find(&window, at) {
                return found;
            }
        }
        radius = radius.saturating_mul(2);
    }
}

fn first_restart(buffer: &str, upto: usize, restarts: Restarts) -> Option<usize> {
    let mut characters = buffer.char_indices();
    let (_, mut previous) = characters.next()?;
    for (at, character) in characters.take_while(|&(at, _)| at <= upto) {
        if is_restart(previous, character, restarts) {
            return Some(at);
        }
        previous = character;
    }
    None
}

fn last_restart(buffer: &str, from: usize, restarts: Restarts) -> Option<usize> {
    buffer[from..]
        .char_indices()
        .rev()
        .map(|(offset, character)| (from + offset, character))
        .find(|&(at, character)| {
            buffer[..at]
                .chars()
                .next_back()
                .is_some_and(|previous| is_restart(previous, character, restarts))
        })
        .map(|(at, _)| at)
}

fn is_restart(previous: char, character: char, restarts: Restarts) -> bool {
    let next = properties(character);
    next.grapheme_cluster_break() == GraphemeClusterBreak::Other
        && !next.is_extended_pictographic()
        && next.indic_conjunct_break() == IndicConjunctBreak::None
        && (restarts == Restarts::InsideDictionaryRuns || !needs_dictionary(character))
        && properties(previous).grapheme_cluster_break() != GraphemeClusterBreak::Prepend
}
