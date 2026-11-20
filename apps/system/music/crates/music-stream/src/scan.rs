use std::ops::RangeInclusive;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scan<'a> {
    rest: &'a str,
}

#[derive(Clone, Debug)]
pub struct Matches<'a, 'p, F> {
    scan: Scan<'a>,
    prefix: &'p str,
    parse: F,
}

impl<'a> Scan<'a> {
    #[must_use]
    pub fn new(text: &'a str) -> Scan<'a> {
        Scan { rest: text }
    }

    #[must_use]
    pub fn rest(&self) -> &'a str {
        self.rest
    }

    pub fn find(&mut self, literal: &str) -> bool {
        match self.rest.find(literal) {
            Some(at) => {
                self.rest = &self.rest[at + literal.len()..];
                true
            }
            None => {
                self.rest = "";
                false
            }
        }
    }

    pub fn expect(&mut self, literal: &str) -> Option<()> {
        self.rest = self.rest.strip_prefix(literal)?;
        Some(())
    }

    pub fn skip(&mut self, literal: &str) {
        if let Some(rest) = self.rest.strip_prefix(literal) {
            self.rest = rest;
        }
    }

    pub fn skip_while(&mut self, class: impl Fn(char) -> bool) {
        self.rest = self.rest.trim_start_matches(class);
    }

    pub fn take(
        &mut self,
        count: RangeInclusive<usize>,
        class: impl Fn(char) -> bool,
    ) -> Option<&'a str> {
        let (taken, end) = self
            .rest
            .char_indices()
            .take_while(|&(_, character)| class(character))
            .take(*count.end())
            .fold((0, 0), |(taken, _), (at, character)| {
                (taken + 1, at + character.len_utf8())
            });
        if taken < *count.start() {
            return None;
        }
        let (found, rest) = self.rest.split_at(end);
        self.rest = rest;
        Some(found)
    }

    #[must_use]
    pub fn matches<'p, T, F>(self, prefix: &'p str, parse: F) -> Matches<'a, 'p, F>
    where
        F: FnMut(Scan<'a>) -> Option<T>,
    {
        Matches {
            scan: self,
            prefix,
            parse,
        }
    }
}

impl<'a, T, F> Iterator for Matches<'a, '_, F>
where
    F: FnMut(Scan<'a>) -> Option<T>,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        while self.scan.find(self.prefix) {
            if let Some(found) = (self.parse)(self.scan) {
                return Some(found);
            }
        }
        None
    }
}

#[must_use]
pub fn is_word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}
