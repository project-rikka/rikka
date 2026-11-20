use std::str::Chars;

use crate::properties::{Properties, properties};

#[derive(Clone, Debug)]
pub(crate) struct Scanner<'a> {
    characters: Chars<'a>,
    length: usize,
    peeked: Option<(char, Properties)>,
}

impl<'a> Scanner<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Self {
            characters: text.chars(),
            length: text.len(),
            peeked: None,
        }
    }

    #[inline(always)]
    pub(crate) fn next(&mut self) -> Option<(usize, char, Properties)> {
        let (character, properties) = match self.peeked.take() {
            Some(peeked) => peeked,
            None => {
                let character = self.characters.next()?;
                (character, properties(character))
            }
        };
        let start = self.length - self.characters.as_str().len() - character.len_utf8();
        Some((start, character, properties))
    }

    #[inline(always)]
    pub(crate) fn next_if(
        &mut self,
        accept: impl FnOnce(Properties) -> bool,
    ) -> Option<(char, Properties)> {
        let (character, properties) = match self.peeked.take() {
            Some(peeked) => peeked,
            None => {
                let character = self.characters.next()?;
                (character, properties(character))
            }
        };
        if accept(properties) {
            Some((character, properties))
        } else {
            self.peeked = Some((character, properties));
            None
        }
    }
}
