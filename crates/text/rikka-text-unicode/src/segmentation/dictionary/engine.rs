use super::candidates::Candidates;
use super::language::{Language, is_elision, is_repetition, is_suffix};
use super::lexicon::Lexicon;
use super::{char_at, char_before};
use crate::general_category;

const LOOKAHEAD: usize = 3;
const ROOT_COMBINE_THRESHOLD: usize = 3;
const PREFIX_COMBINE_THRESHOLD: usize = 3;

#[derive(Clone, Debug)]
pub(crate) struct Engine {
    language: Language,
    lexicon: Lexicon,
    end: usize,
    cursor: usize,
    found: usize,
    words: [Candidates; LOOKAHEAD],
}

impl Engine {
    pub(crate) fn new(text: &str, language: Language, start: usize, end: usize) -> Self {
        let divisible = text[start..end]
            .chars()
            .nth(language.shortest_divisible_span() - 1)
            .is_some();
        Self {
            language,
            lexicon: Lexicon::for_language(language),
            end,
            cursor: if divisible { start } else { end },
            found: 0,
            words: [Candidates::default(); LOOKAHEAD],
        }
    }

    pub(crate) fn next_break(&mut self, text: &str) -> Option<usize> {
        if self.cursor >= self.end {
            return None;
        }
        let current = self.cursor;
        let (mut length, characters) = self.take_word(text);
        if self.cursor < self.end && characters < ROOT_COMBINE_THRESHOLD {
            length = self.combine_non_word(text, current, length);
        }
        while self.cursor < self.end
            && let Some(mark) = char_at(text, self.cursor).filter(|&character| is_mark(character))
        {
            self.cursor += mark.len_utf8();
            length += mark.len_utf8();
        }
        if self.language.has_suffixes() && self.cursor < self.end && length > 0 {
            length = self.attach_suffixes(text, current, length);
        }
        let boundary = current + length;
        (boundary < self.end).then_some(boundary)
    }

    fn slot(&self, ahead: usize) -> usize {
        (self.found + ahead) % LOOKAHEAD
    }

    fn candidates(&mut self, text: &str, ahead: usize) -> usize {
        let slot = self.slot(ahead);
        self.words[slot].find(&self.lexicon, text, &mut self.cursor, self.end)
    }

    fn take_word(&mut self, text: &str) -> (usize, usize) {
        let first = self.slot(0);
        let count = self.candidates(text, 0);
        if count == 0 {
            return (0, 0);
        }
        if count > 1 && self.cursor < self.end {
            self.choose_among_candidates(text);
        }
        let length = self.words[first].accept_marked(&mut self.cursor);
        let characters = self.words[first].marked_characters();
        self.found += 1;
        (length, characters)
    }

    fn choose_among_candidates(&mut self, text: &str) {
        let (first, second) = (self.slot(0), self.slot(1));
        loop {
            if self.candidates(text, 1) > 0 {
                self.words[first].mark_current();
                if self.cursor >= self.end {
                    return;
                }
                loop {
                    if self.candidates(text, 2) > 0 {
                        self.words[first].mark_current();
                        return;
                    }
                    if !self.words[second].back_up(&mut self.cursor) {
                        break;
                    }
                }
            }
            if !self.words[first].back_up(&mut self.cursor) {
                return;
            }
        }
    }

    fn combine_non_word(&mut self, text: &str, current: usize, length: usize) -> usize {
        let next = self.slot(0);
        let resume = current + length;
        if self.candidates(text, 0) > 0
            || (length > 0 && self.words[next].longest_prefix() >= PREFIX_COMBINE_THRESHOLD)
        {
            self.cursor = resume;
            return length;
        }
        let mut skipped = 0;
        while let Some(previous) = char_at(text, self.cursor) {
            self.cursor += previous.len_utf8();
            skipped += previous.len_utf8();
            if self.cursor >= self.end {
                break;
            }
            let Some(following) = char_at(text, self.cursor) else {
                break;
            };
            if self.language.can_end_word(previous) && self.language.can_begin_word(following) {
                let count = self.candidates(text, 1);
                self.cursor = resume + skipped;
                if count > 0 {
                    break;
                }
            }
        }
        if length == 0 {
            self.found += 1;
        }
        length + skipped
    }

    fn attach_suffixes(&mut self, text: &str, current: usize, mut length: usize) -> usize {
        let mut following = char_at(text, self.cursor);
        if self.candidates(text, 0) > 0 || !following.is_some_and(is_suffix) {
            self.cursor = current + length;
            return length;
        }
        if let Some(elision) = following.filter(|&character| is_elision(character))
            && !char_before(text, self.cursor).is_some_and(is_suffix)
        {
            self.cursor += elision.len_utf8();
            length += elision.len_utf8();
            following = char_at(text, self.cursor).filter(|_| self.cursor < self.end);
        }
        if let Some(repetition) = following.filter(|&character| is_repetition(character))
            && !char_before(text, self.cursor).is_some_and(is_repetition)
        {
            self.cursor += repetition.len_utf8();
            length += repetition.len_utf8();
        }
        length
    }
}

#[inline]
fn is_mark(character: char) -> bool {
    general_category(character).is_mark()
}
