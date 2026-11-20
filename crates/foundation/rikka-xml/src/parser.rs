use std::borrow::Cow;

use crate::element::{Attribute, Element, Node};
use crate::entities::decode;
use crate::error::{Error, Markup, Position, Result};
use crate::namespaces::{Declaration, Resolved, Role, Scopes};

pub const MAX_DEPTH: usize = 512;

const BYTE_ORDER_MARK: char = '\u{FEFF}';

pub fn parse(source: &str) -> Result<Element<'_>> {
    Parser::new(source, Recovery::Strict).run()
}

#[must_use]
pub fn parse_lenient(source: &str) -> Option<Element<'_>> {
    Parser::new(source, Recovery::Lenient).run().ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Recovery {
    Strict,
    Lenient,
}

struct Open<'a> {
    element: Element<'a>,
    scope: usize,
}

struct Tag<'a> {
    qualified: &'a str,
    attributes: Vec<(&'a str, Cow<'a, str>)>,
    start: usize,
}

enum TagEnd {
    Open,
    Empty,
    Dropped,
}

struct Parser<'a> {
    source: &'a str,
    position: usize,
    recovery: Recovery,
    open: Vec<Open<'a>>,
    scopes: Scopes<'a>,
    root: Option<Element<'a>>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str, recovery: Recovery) -> Self {
        Self {
            source,
            position: source
                .strip_prefix(BYTE_ORDER_MARK)
                .map_or(0, |rest| source.len() - rest.len()),
            recovery,
            open: Vec::new(),
            scopes: Scopes::default(),
            root: None,
        }
    }

    fn run(mut self) -> Result<Element<'a>> {
        while self.position < self.source.len() {
            let rest = self.rest();
            let Some(open) = rest.find('<') else {
                self.text(rest)?;
                self.position = self.source.len();
                break;
            };
            self.text(&rest[..open])?;
            self.position += open;
            let markup = self.rest();
            if markup.starts_with("<!--") {
                self.skip_past("-->", Markup::Comment)?;
            } else if let Some(data) = markup.strip_prefix("<![CDATA[") {
                self.character_data(data)?;
            } else if markup.starts_with("<?") {
                self.skip_past("?>", Markup::ProcessingInstruction)?;
            } else if markup.starts_with("<!") {
                self.declaration()?;
            } else if markup.starts_with("</") {
                self.end_tag()?;
            } else {
                self.start_tag()?;
            }
        }
        if let Some(unclosed) = self.open.last() {
            let (name, start) = (unclosed.element.qualified, unclosed.element.range.start);
            self.tolerate(start, |at| Error::UnclosedElement {
                name: name.to_owned(),
                at,
            })?;
        }
        while !self.open.is_empty() {
            self.close(self.source.len());
        }
        self.root.ok_or(Error::NoRoot)
    }

    fn rest(&self) -> &'a str {
        &self.source[self.position..]
    }

    fn locate(&self, offset: usize) -> Position {
        let before = &self.source[..offset];
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        let count = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
        Position {
            line: count(before.matches('\n').count()).saturating_add(1),
            column: count(before[line_start..].chars().count()).saturating_add(1),
        }
    }

    fn tolerate(&self, offset: usize, error: impl FnOnce(Position) -> Error) -> Result<()> {
        match self.recovery {
            Recovery::Strict => Err(error(self.locate(offset))),
            Recovery::Lenient => Ok(()),
        }
    }

    fn skip_past(&mut self, terminator: &str, markup: Markup) -> Result<()> {
        self.position = match self.rest().find(terminator) {
            Some(offset) => self.position + offset + terminator.len(),
            None => {
                self.tolerate(self.position, |at| Error::Unterminated { markup, at })?;
                self.source.len()
            }
        };
        Ok(())
    }

    fn declaration(&mut self) -> Result<()> {
        let rest = self.rest();
        let close = rest.find('>');
        match rest.find('[') {
            Some(subset) if close.is_none_or(|close| subset < close) => {
                self.skip_past("]>", Markup::Declaration)
            }
            _ => self.skip_past(">", Markup::Declaration),
        }
    }

    fn character_data(&mut self, data: &'a str) -> Result<()> {
        let start = self.position;
        let length = match data.find("]]>") {
            Some(length) => length,
            None => {
                self.tolerate(start, |at| Error::Unterminated {
                    markup: Markup::CharacterData,
                    at,
                })?;
                data.len()
            }
        };
        if length > 0 {
            self.push_text(Cow::Borrowed(&data[..length]), start)?;
        }
        self.position = (start + "<![CDATA[".len() + length + "]]>".len()).min(self.source.len());
        Ok(())
    }

    fn text(&mut self, text: &'a str) -> Result<()> {
        if text.is_empty() || (self.open.is_empty() && text.trim().is_empty()) {
            return Ok(());
        }
        let start = self.position;
        let decoded = decode(text);
        if let Some(unresolved) = decoded.unresolved {
            self.tolerate(start + unresolved, |at| Error::InvalidReference { at })?;
        }
        self.push_text(decoded.text, start)
    }

    fn push_text(&mut self, text: Cow<'a, str>, start: usize) -> Result<()> {
        match self.open.last_mut() {
            Some(open) => {
                open.element.children.push(Node::Text(text));
                Ok(())
            }
            None => self.tolerate(start, |at| Error::OutsideRoot { at }),
        }
    }

    fn end_tag(&mut self) -> Result<()> {
        let start = self.position;
        let rest = &self.rest()[2..];
        let Some(close) = rest.find('>') else {
            self.tolerate(start, |at| Error::Unterminated {
                markup: Markup::Tag,
                at,
            })?;
            self.position = self.source.len();
            return Ok(());
        };
        let name = rest[..close].trim();
        self.position += 2 + close + 1;
        let Some(depth) = self
            .open
            .iter()
            .rposition(|open| open.element.qualified == name)
        else {
            return self.tolerate(start, |at| Error::UnexpectedEndTag {
                name: name.to_owned(),
                at,
            });
        };
        if let Some(innermost) = self.open.last().filter(|_| depth + 1 < self.open.len()) {
            let open = innermost.element.qualified;
            self.tolerate(start, |at| Error::MismatchedEndTag {
                open: open.to_owned(),
                found: name.to_owned(),
                at,
            })?;
        }
        while self.open.len() > depth + 1 {
            self.close(start);
        }
        self.close(self.position);
        Ok(())
    }

    fn start_tag(&mut self) -> Result<()> {
        let start = self.position;
        let rest = &self.rest()[1..];
        let name_length = rest
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(rest.len());
        if name_length == 0 {
            self.tolerate(start, |at| Error::MalformedTag { at })?;
            self.push_text(Cow::Borrowed("<"), start)?;
            self.position += 1;
            return Ok(());
        }
        if rest[..name_length].contains(['<', '=', '&', '"', '\'']) {
            self.tolerate(start, |at| Error::MalformedTag { at })?;
        }
        let mut tag = Tag {
            qualified: &rest[..name_length],
            attributes: Vec::new(),
            start,
        };
        self.position += 1 + name_length;
        match self.attributes(&mut tag)? {
            TagEnd::Dropped => Ok(()),
            TagEnd::Open => self.enter(tag),
            TagEnd::Empty => {
                self.enter(tag)?;
                self.close(self.position);
                Ok(())
            }
        }
    }

    fn attributes(&mut self, tag: &mut Tag<'a>) -> Result<TagEnd> {
        loop {
            let tail = self.rest();
            let trimmed = tail.trim_start();
            self.position += tail.len() - trimmed.len();
            if trimmed.is_empty() {
                self.tolerate(tag.start, |at| Error::Unterminated {
                    markup: Markup::Tag,
                    at,
                })?;
                return Ok(TagEnd::Dropped);
            }
            if trimmed.starts_with("/>") {
                self.position += 2;
                return Ok(TagEnd::Empty);
            }
            if trimmed.starts_with('>') {
                self.position += 1;
                return Ok(TagEnd::Open);
            }
            let Some((key, value, consumed)) = self.attribute(trimmed)? else {
                self.skip_past(">", Markup::Tag)?;
                return Ok(TagEnd::Dropped);
            };
            if !key.is_empty() {
                tag.attributes.push((key, value));
            }
            self.position += consumed.max(1);
        }
    }

    fn attribute(&self, text: &'a str) -> Result<Option<(&'a str, Cow<'a, str>, usize)>> {
        let malformed = || self.tolerate(self.position, |at| Error::MalformedTag { at });
        let key_length = text
            .find(|c: char| c.is_whitespace() || c == '=' || c == '/' || c == '>')
            .unwrap_or(text.len());
        let key = &text[..key_length];
        if key.is_empty() {
            malformed()?;
        }
        let Some(after_equals) = text[key_length..].trim_start().strip_prefix('=') else {
            malformed()?;
            return Ok(Some((key, Cow::Borrowed(""), key_length)));
        };
        let value_text = after_equals.trim_start();
        let value_start = text.len() - value_text.len();
        let (body, body_start) = match value_text.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let body = &value_text[1..];
                let Some(end) = body.find(quote) else {
                    malformed()?;
                    return Ok(None);
                };
                (&body[..end], value_start + 1)
            }
            Some(_) => {
                malformed()?;
                let end = value_text
                    .find(|c: char| c.is_whitespace() || c == '>')
                    .unwrap_or(value_text.len());
                (&value_text[..end], value_start)
            }
            None => {
                malformed()?;
                return Ok(None);
            }
        };
        if body.contains('<') {
            malformed()?;
        }
        let decoded = decode(body);
        if let Some(unresolved) = decoded.unresolved {
            let offset = self.position + body_start + unresolved;
            self.tolerate(offset, |at| Error::InvalidReference { at })?;
        }
        let quotes = body_start - value_start;
        Ok(Some((key, decoded.text, body_start + body.len() + quotes)))
    }

    fn enter(&mut self, tag: Tag<'a>) -> Result<()> {
        if self.open.len() == MAX_DEPTH {
            return Err(Error::TooDeep {
                limit: MAX_DEPTH,
                at: self.locate(tag.start),
            });
        }
        if self.open.is_empty() && self.root.is_some() {
            self.tolerate(tag.start, |at| Error::OutsideRoot { at })?;
        }
        let scope = self.scopes.depth();
        let mut declarations = Vec::new();
        let mut plain: Vec<(&'a str, Cow<'a, str>)> = Vec::with_capacity(tag.attributes.len());
        for (name, value) in tag.attributes {
            match Role::of(name) {
                Role::Declares(prefix) => declarations.push(Declaration { prefix, uri: value }),
                Role::Attribute => {
                    if plain.iter().any(|&(known, _)| known == name) {
                        self.tolerate(tag.start, |at| Error::DuplicateAttribute {
                            name: name.to_owned(),
                            at,
                        })?;
                    }
                    plain.push((name, value));
                }
            }
        }
        self.scopes.declare(&declarations);
        let (prefix, name) = split(tag.qualified);
        let namespace = self.namespace(prefix, tag.start)?;
        let mut attributes = Vec::with_capacity(plain.len());
        for (qualified, value) in plain {
            let (prefix, name) = split(qualified);
            let namespace = match prefix {
                Some(_) => self.namespace(prefix, tag.start)?,
                None => None,
            };
            attributes.push(Attribute {
                name,
                namespace,
                value,
            });
        }
        self.open.push(Open {
            element: Element {
                qualified: tag.qualified,
                name,
                namespace,
                attributes,
                declarations,
                children: Vec::new(),
                range: tag.start..tag.start,
            },
            scope,
        });
        Ok(())
    }

    fn namespace(&self, prefix: Option<&str>, at: usize) -> Result<Option<Cow<'a, str>>> {
        match self.scopes.resolve(prefix) {
            Resolved::Namespace(uri) => Ok(Some(uri)),
            Resolved::None => Ok(None),
            Resolved::Undeclared => {
                self.tolerate(at, |at| Error::UndeclaredPrefix {
                    prefix: prefix.unwrap_or_default().to_owned(),
                    at,
                })?;
                Ok(None)
            }
        }
    }

    fn close(&mut self, end: usize) {
        let Some(Open { mut element, scope }) = self.open.pop() else {
            return;
        };
        self.scopes.leave(scope);
        element.range.end = end;
        match self.open.last_mut() {
            Some(parent) => parent.element.children.push(Node::Element(element)),
            None => {
                self.root.get_or_insert(element);
            }
        }
    }
}

fn split(qualified: &str) -> (Option<&str>, &str) {
    match qualified.split_once(':') {
        Some((prefix, name)) => (Some(prefix), name),
        None => (None, qualified),
    }
}
