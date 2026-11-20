use crate::body::{Body, Emphasis, Link, Style, drawable};

pub(crate) fn read(source: &str) -> Body {
    if !source.contains(['<', '&']) {
        return crate::body::capped(Body {
            text: source.to_owned(),
            ..Body::default()
        });
    }

    let mut parsed = Parsed::default();
    let mut at = 0;
    while at < source.len() {
        let rest = &source[at..];
        if rest.starts_with('<')
            && let Some((tag, used)) = tag(source, at)
        {
            parsed.apply(tag);
            at += used;
            continue;
        }
        if rest.starts_with('&')
            && let Some((character, used)) = entity(rest)
        {
            parsed.push(character);
            at += used;
            continue;
        }
        let character = rest.chars().next().expect("`at` should be a char boundary");
        parsed.push(character);
        at += character.len_utf8();
    }
    parsed.finish()
}

#[derive(Debug, Default)]
struct Parsed {
    text: String,
    open: Vec<(Mark, usize)>,
    emphasis: Vec<Emphasis>,
    links: Vec<Link>,
}

#[derive(Debug)]
enum Mark {
    Style(Style),
    Link(String),
}

impl Parsed {
    fn apply(&mut self, tag: Tag) {
        match tag {
            Tag::Open(mark) => self.open.push((mark, self.text.len())),
            Tag::Close(closing) => self.close(closing),
            Tag::Break => self.text.push('\n'),
            Tag::Text(text) => text.chars().for_each(|character| self.push(character)),
            Tag::Ignored => {}
        }
    }

    fn push(&mut self, character: char) {
        if drawable(character) {
            self.text.push(character);
        }
    }

    fn close(&mut self, closing: Closing) {
        let found = self
            .open
            .iter()
            .rposition(|(mark, _)| mark.ends_at(closing));
        if let Some(index) = found {
            let (mark, start) = self.open.remove(index);
            self.record(mark, start);
        }
    }

    fn record(&mut self, mark: Mark, start: usize) {
        let range = start..self.text.len();
        if range.is_empty() {
            return;
        }
        match mark {
            Mark::Style(style) => self.emphasis.push(Emphasis { range, style }),
            Mark::Link(href) => self.links.push(Link { range, href }),
        }
    }

    fn finish(mut self) -> Body {
        while let Some((mark, start)) = self.open.pop() {
            self.record(mark, start);
        }
        crate::body::capped(Body {
            text: self.text,
            emphasis: self.emphasis,
            links: self.links,
        })
    }
}

impl Mark {
    fn ends_at(&self, closing: Closing) -> bool {
        match (self, closing) {
            (Mark::Style(open), Closing::Style(close)) => *open == close,
            (Mark::Link(_), Closing::Link) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
enum Tag {
    Open(Mark),
    Close(Closing),
    Break,
    Text(String),
    Ignored,
}

#[derive(Clone, Copy, Debug)]
enum Closing {
    Style(Style),
    Link,
}

fn tag(source: &str, from: usize) -> Option<(Tag, usize)> {
    let mut cursor = from + 1;
    let closing = source[cursor..].starts_with('/');
    if closing {
        cursor += 1;
    }

    let rest = source.get(cursor..)?;
    let name_len = rest
        .find(|character: char| !character.is_ascii_alphanumeric())
        .unwrap_or(rest.len());
    if name_len == 0 {
        return None;
    }
    let name = &rest[..name_len];
    cursor += name_len;

    let (attributes, used) = attributes_of(&source[cursor..])?;
    cursor += used;

    let lowered = name.to_ascii_lowercase();
    let tag = match (closing, lowered.as_str()) {
        (true, "b") => Tag::Close(Closing::Style(Style::Bold)),
        (true, "i") => Tag::Close(Closing::Style(Style::Italic)),
        (true, "u") => Tag::Close(Closing::Style(Style::Underline)),
        (true, "a") => Tag::Close(Closing::Link),
        (true, _) => Tag::Ignored,
        (false, "b") => Tag::Open(Mark::Style(Style::Bold)),
        (false, "i") => Tag::Open(Mark::Style(Style::Italic)),
        (false, "u") => Tag::Open(Mark::Style(Style::Underline)),
        (false, "a") => Tag::Open(Mark::Link(
            attribute(attributes, "href").unwrap_or_default(),
        )),
        (false, "br") => Tag::Break,
        (false, "img") => Tag::Text(attribute(attributes, "alt").unwrap_or_default()),
        (false, _) => Tag::Ignored,
    };
    Some((tag, cursor - from))
}

fn attributes_of(text: &str) -> Option<(&str, usize)> {
    let mut quote = None;
    for (at, character) in text.char_indices() {
        match (quote, character) {
            (Some(open), character) if character == open => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(character),
            (None, '>') => return Some((&text[..at], at + 1)),
            (None, '<') => return None,
            (None, _) => {}
        }
    }
    None
}

fn attribute(attributes: &str, attribute_name: &str) -> Option<String> {
    let mut rest = attributes.trim_start().trim_end_matches('/');
    while !rest.is_empty() {
        let name_len = rest
            .find(|character: char| character == '=' || character.is_whitespace())
            .unwrap_or(rest.len());
        if name_len == 0 {
            return None;
        }
        let name = &rest[..name_len];
        rest = rest[name_len..].trim_start();

        let mut value = None;
        if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            let quote = after.chars().next().filter(|c| matches!(c, '"' | '\''));
            match quote {
                Some(quote) => {
                    let inner = &after[quote.len_utf8()..];
                    let end = inner.find(quote)?;
                    value = Some(&inner[..end]);
                    rest = &inner[end + quote.len_utf8()..];
                }
                None => {
                    let end = after.find(char::is_whitespace).unwrap_or(after.len());
                    value = Some(&after[..end]);
                    rest = &after[end..];
                }
            }
        }
        if name.eq_ignore_ascii_case(attribute_name) {
            return value.map(unescape);
        }
        rest = rest.trim_start();
    }
    None
}

fn entity(text: &str) -> Option<(char, usize)> {
    const MOST: usize = 9;

    let rest = text.get(1..)?;
    let end = rest.bytes().take(MOST).position(|byte| byte == b';')?;
    let name = &rest[..end];
    let character = match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        _ => {
            let digits = name.strip_prefix('#')?;
            let code = match digits.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => digits.parse().ok()?,
            };
            char::from_u32(code)?
        }
    };
    Some((character, 1 + end + 1))
}

fn unescape(value: &str) -> String {
    if !value.contains('&') {
        return value.to_owned();
    }
    let mut out = String::with_capacity(value.len());
    let mut at = 0;
    while at < value.len() {
        let rest = &value[at..];
        if rest.starts_with('&')
            && let Some((character, used)) = entity(rest)
        {
            out.push(character);
            at += used;
            continue;
        }
        let character = rest.chars().next().expect("`at` should be a char boundary");
        out.push(character);
        at += character.len_utf8();
    }
    out
}
