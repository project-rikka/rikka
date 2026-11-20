mod candidates;
mod cj;
mod engine;
mod language;
mod lexicon;
mod run;

use std::iter::FusedIterator;

pub(crate) use cj::{cj_breaks, is_cj};
use run::{ComplexRun, is_complex};

#[inline]
fn char_at(text: &str, at: usize) -> Option<char> {
    text[at..].chars().next()
}

#[inline]
fn char_before(text: &str, at: usize) -> Option<char> {
    text[..at].chars().next_back()
}

#[derive(Clone, Debug, Default)]
pub(crate) struct DictionaryCursor {
    run: ComplexRun,
    pending: Option<usize>,
}

impl DictionaryCursor {
    pub(crate) fn breaks_at(&mut self, text: &str, position: usize) -> bool {
        if position >= self.run.end() {
            self.run = ComplexRun::from_position(text, position);
            self.pending = self.run.next_break(text);
        }
        while let Some(pending) = self.pending {
            if pending >= position {
                return pending == position;
            }
            self.pending = self.run.next_break(text);
        }
        false
    }

    pub(crate) fn first_break_between(
        &mut self,
        text: &str,
        from: usize,
        to: usize,
    ) -> Option<usize> {
        loop {
            while let Some(pending) = self.pending {
                if pending > from {
                    return (pending < to).then_some(pending);
                }
                self.pending = self.run.next_break(text);
            }
            let search = from.max(self.run.end());
            let (offset, _) = text
                .get(search..to)?
                .char_indices()
                .find(|&(_, character)| is_complex(character))?;
            self.run = ComplexRun::from_position(text, search + offset);
            self.pending = self.run.next_break(text);
        }
    }
}

pub fn needs_dictionary(character: char) -> bool {
    is_complex(character) || is_cj(character)
}

#[derive(Clone, Debug)]
pub struct DictionaryBreaks<'a> {
    text: &'a str,
    run: ComplexRun,
}

pub fn dictionary_breaks(text: &str) -> DictionaryBreaks<'_> {
    DictionaryBreaks {
        text,
        run: ComplexRun::default(),
    }
}

impl Iterator for DictionaryBreaks<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        loop {
            if let Some(boundary) = self.run.next_break(self.text) {
                return Some(boundary);
            }
            let searched = self.run.end();
            let Some((offset, _)) = self.text[searched..]
                .char_indices()
                .find(|&(_, character)| is_complex(character))
            else {
                self.run = ComplexRun::from_start(self.text, self.text.len());
                return None;
            };
            self.run = ComplexRun::from_start(self.text, searched + offset);
        }
    }
}

impl FusedIterator for DictionaryBreaks<'_> {}
