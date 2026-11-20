use std::ops::{Deref, Range};
use std::str::FromStr;

use rikka_text_font::Tag;

use crate::error::Error;

pub(crate) const GLOBAL_START: u32 = 0;
pub(crate) const GLOBAL_END: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Feature {
    pub tag: Tag,
    pub value: u32,
    pub start: u32,
    pub end: u32,
}

impl Feature {
    pub fn new(tag: Tag, value: u32) -> Self {
        Self {
            tag,
            value,
            start: GLOBAL_START,
            end: GLOBAL_END,
        }
    }

    #[must_use]
    pub fn with_clusters(mut self, clusters: Range<u32>) -> Self {
        self.start = clusters.start;
        self.end = clusters.end;
        self
    }

    #[inline]
    pub fn is_global(&self) -> bool {
        self.start == GLOBAL_START && self.end == GLOBAL_END
    }
}

impl FromStr for Feature {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Error> {
        FeatureParser::new(text)
            .parse()
            .ok_or_else(|| Error::InvalidFeature(text.to_owned()))
    }
}

struct FeatureParser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> FeatureParser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            at: 0,
        }
    }

    fn parse(mut self) -> Option<Feature> {
        self.skip_spaces();
        let mut value = 1;
        if self.eat(b'-') {
            value = 0;
        } else {
            self.eat(b'+');
        }
        self.skip_spaces();
        let tag = self.tag()?;
        self.skip_spaces();
        let (start, end) = self.range()?;
        self.skip_spaces();
        let had_equal = self.eat(b'=');
        self.skip_spaces();
        match self.value() {
            Some(parsed) => value = parsed,
            None if had_equal => return None,
            None => {}
        }
        self.skip_spaces();
        (self.at == self.bytes.len()).then_some(Feature {
            tag,
            value,
            start,
            end,
        })
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    fn tag(&mut self) -> Option<Tag> {
        let quote = match self.peek() {
            Some(quote @ (b'\'' | b'"')) => {
                self.at += 1;
                Some(quote)
            }
            _ => None,
        };
        let start = self.at;
        while self.at - start < 4 && self.peek().is_some_and(|byte| byte.is_ascii_alphanumeric()) {
            self.at += 1;
        }
        if self.at == start {
            return None;
        }
        let mut tag = [b' '; 4];
        tag[..self.at - start].copy_from_slice(&self.bytes[start..self.at]);
        if let Some(quote) = quote
            && !self.eat(quote)
        {
            return None;
        }
        Some(Tag::new(&tag))
    }

    fn range(&mut self) -> Option<(u32, u32)> {
        if !self.eat(b'[') {
            return Some((GLOBAL_START, GLOBAL_END));
        }
        self.skip_spaces();
        let start = self.integer();
        self.skip_spaces();
        let end = if self.eat(b':') || self.eat(b';') {
            self.skip_spaces();
            self.integer().unwrap_or(GLOBAL_END)
        } else {
            match start {
                Some(start) if start != GLOBAL_END => start + 1,
                _ => GLOBAL_END,
            }
        };
        self.skip_spaces();
        self.eat(b']')
            .then_some((start.unwrap_or(GLOBAL_START), end))
    }

    fn integer(&mut self) -> Option<u32> {
        let negative = self.eat(b'-');
        let start = self.at;
        let mut value: u32 = 0;
        while let Some(digit) = self.peek().filter(u8::is_ascii_digit) {
            value = value.wrapping_mul(10).wrapping_add(u32::from(digit - b'0'));
            self.at += 1;
        }
        if self.at == start {
            if negative {
                self.at -= 1;
            }
            return None;
        }
        Some(if negative {
            value.wrapping_neg()
        } else {
            value
        })
    }

    fn value(&mut self) -> Option<u32> {
        if let Some(value) = self.integer() {
            return Some(value);
        }
        for (word, value) in [("on", 1), ("off", 0)] {
            let rest = &self.bytes[self.at..];
            if rest.len() >= word.len() && rest[..word.len()].eq_ignore_ascii_case(word.as_bytes())
            {
                self.at += word.len();
                return Some(value);
            }
        }
        None
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Features {
    list: Vec<Feature>,
}

impl Features {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, feature: Feature) {
        self.list.push(feature);
    }

    #[must_use]
    pub fn with(mut self, feature: Feature) -> Self {
        self.list.push(feature);
        self
    }
}

impl Deref for Features {
    type Target = [Feature];

    fn deref(&self) -> &[Feature] {
        &self.list
    }
}

impl FromIterator<Feature> for Features {
    fn from_iter<I: IntoIterator<Item = Feature>>(iter: I) -> Self {
        Self {
            list: iter.into_iter().collect(),
        }
    }
}

impl FromStr for Features {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self, Error> {
        text.split(',')
            .filter(|item| !item.trim().is_empty())
            .map(Feature::from_str)
            .collect()
    }
}
