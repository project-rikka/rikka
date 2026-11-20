use std::borrow::Cow;

use crate::entities::decode;

const RAW_TEXT: [&str; 7] = [
    "script", "style", "iframe", "noembed", "noframes", "noscript", "xmp",
];
const ESCAPABLE_RAW_TEXT: [&str; 2] = ["textarea", "title"];

type Attributes<'a> = Vec<(Cow<'a, str>, Cow<'a, str>)>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token<'a> {
    Text(Cow<'a, str>),
    Open(Tag<'a>),
    Close(Cow<'a, str>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag<'a> {
    name: Cow<'a, str>,
    attributes: Attributes<'a>,
    self_closing: bool,
}

impl<'a> Tag<'a> {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, value)| value.as_ref())
    }

    pub fn attributes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attributes
            .iter()
            .map(|(name, value)| (name.as_ref(), value.as_ref()))
    }

    #[must_use]
    pub fn is_self_closing(&self) -> bool {
        self.self_closing
    }

    pub(crate) fn into_parts(self) -> (Cow<'a, str>, Attributes<'a>) {
        (self.name, self.attributes)
    }
}

#[derive(Clone, Debug)]
pub struct Tokens<'a> {
    html: &'a str,
    at: usize,
    raw: Option<Raw>,
}

#[derive(Clone, Copy, Debug)]
struct Raw {
    name: &'static str,
    escapable: bool,
}

#[must_use]
pub fn tokens(html: &str) -> Tokens<'_> {
    Tokens {
        html,
        at: 0,
        raw: None,
    }
}

enum Markup<'a> {
    Skipped,
    Token(Token<'a>),
}

impl<'a> Iterator for Tokens<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        if let Some(raw) = self.raw.take()
            && let Some(text) = self.raw_text(raw)
        {
            return Some(text);
        }
        loop {
            if self.at >= self.html.len() {
                return None;
            }
            let (text_end, markup) = self.next_markup();
            if text_end > self.at {
                let text = &self.html[self.at..text_end];
                self.at = text_end;
                return Some(Token::Text(decode(text)));
            }
            let (markup, end) = markup?;
            self.at = end;
            if let Markup::Token(token) = markup {
                if let Token::Open(tag) = &token {
                    self.raw = raw_of(tag.name());
                }
                return Some(token);
            }
        }
    }
}

impl<'a> Tokens<'a> {
    fn next_markup(&self) -> (usize, Option<(Markup<'a>, usize)>) {
        let mut from = self.at;
        while let Some(offset) = self.html[from..].find('<') {
            let start = from + offset;
            if let Some(markup) = markup_at(self.html, start) {
                return (start, Some(markup));
            }
            from = start + 1;
        }
        (self.html.len(), None)
    }

    fn raw_text(&mut self, raw: Raw) -> Option<Token<'a>> {
        let rest = &self.html[self.at..];
        let end = closing(rest, raw.name).unwrap_or(rest.len());
        if end == 0 {
            return None;
        }
        let text = &rest[..end];
        self.at += end;
        Some(Token::Text(match raw.escapable {
            true => decode(text),
            false => Cow::Borrowed(text),
        }))
    }
}

fn raw_of(name: &str) -> Option<Raw> {
    let listed = |names: &[&'static str]| names.iter().copied().find(|known| *known == name);
    listed(&RAW_TEXT)
        .map(|name| Raw {
            name,
            escapable: false,
        })
        .or_else(|| {
            listed(&ESCAPABLE_RAW_TEXT).map(|name| Raw {
                name,
                escapable: true,
            })
        })
}

fn closing(text: &str, name: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(offset) = text[from..].find("</") {
        let start = from + offset;
        let after = start + 2 + name.len();
        let named = bytes
            .get(start + 2..after)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name.as_bytes()));
        let ended = bytes
            .get(after)
            .is_none_or(|byte| byte.is_ascii_whitespace() || matches!(byte, b'/' | b'>'));
        if named && ended {
            return Some(start);
        }
        from = start + 2;
    }
    None
}

fn markup_at(html: &str, start: usize) -> Option<(Markup<'_>, usize)> {
    let rest = &html[start..];
    if let Some(comment) = rest.strip_prefix("<!--") {
        let end = comment
            .find("-->")
            .map_or(html.len(), |index| start + 4 + index + 3);
        return Some((Markup::Skipped, end));
    }
    let bytes = rest.as_bytes();
    match bytes.get(1)? {
        b'!' | b'?' => {
            let end = rest.find('>').map_or(html.len(), |index| start + index + 1);
            Some((Markup::Skipped, end))
        }
        b'/' if bytes.get(2).is_some_and(u8::is_ascii_alphabetic) => {
            let (name, after) = name_at(rest, 2);
            let close = rest[after..].find('>')?;
            Some((Markup::Token(Token::Close(name)), start + after + close + 1))
        }
        byte if byte.is_ascii_alphabetic() => {
            let (name, after) = name_at(rest, 1);
            let (attributes, self_closing, end) = attributes_at(rest, after)?;
            Some((
                Markup::Token(Token::Open(Tag {
                    name,
                    attributes,
                    self_closing,
                })),
                start + end,
            ))
        }
        _ => None,
    }
}

fn name_at(text: &str, from: usize) -> (Cow<'_, str>, usize) {
    let length = text[from..]
        .bytes()
        .take_while(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'/' | b'>'))
        .count();
    (lowercase(&text[from..from + length]), from + length)
}

fn lowercase(name: &str) -> Cow<'_, str> {
    match name.bytes().any(|byte| byte.is_ascii_uppercase()) {
        true => Cow::Owned(name.to_ascii_lowercase()),
        false => Cow::Borrowed(name),
    }
}

fn attributes_at(text: &str, mut at: usize) -> Option<(Attributes<'_>, bool, usize)> {
    let bytes = text.as_bytes();
    let mut attributes = Vec::new();
    loop {
        at = skip_spaces(bytes, at);
        match bytes.get(at)? {
            b'>' => return Some((attributes, false, at + 1)),
            b'/' if bytes.get(at + 1) == Some(&b'>') => return Some((attributes, true, at + 2)),
            b'/' => {
                at += 1;
                continue;
            }
            _ => {}
        }
        let name_start = at;
        at += 1;
        while bytes
            .get(at)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'=' | b'>' | b'/'))
        {
            at += 1;
        }
        let name = lowercase(&text[name_start..at]);
        at = skip_spaces(bytes, at);
        let mut value = Cow::Borrowed("");
        if bytes.get(at) == Some(&b'=') {
            at = skip_spaces(bytes, at + 1);
            match bytes.get(at)? {
                quote @ (b'"' | b'\'') => {
                    let close = text[at + 1..].find(char::from(*quote))?;
                    value = decode(&text[at + 1..at + 1 + close]);
                    at += close + 2;
                }
                _ => {
                    let value_start = at;
                    while bytes.get(at).is_some_and(|byte| {
                        !byte.is_ascii_whitespace()
                            && *byte != b'>'
                            && !(*byte == b'/' && bytes.get(at + 1) == Some(&b'>'))
                    }) {
                        at += 1;
                    }
                    value = decode(&text[value_start..at]);
                }
            }
        }
        attributes.push((name, value));
    }
}

fn skip_spaces(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}
