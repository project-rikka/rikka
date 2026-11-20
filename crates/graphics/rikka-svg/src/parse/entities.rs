use std::borrow::Cow;
use std::collections::HashMap;

use crate::error::{Error, Result};

const MAX_EXPANDED: usize = 64 * 1024 * 1024;
const MAX_NESTING: usize = 8;
const DOCTYPE: &str = "<!DOCTYPE";
const ENTITY: &str = "<!ENTITY";
const VERBATIM: [(&str, &str); 3] = [("<!--", "-->"), ("<![CDATA[", "]]>"), ("<?", "?>")];

struct Entities<'a> {
    values: HashMap<&'a str, &'a str>,
}

pub(crate) fn expanded(source: &str) -> Result<Cow<'_, str>> {
    let Some((entities, body)) = declared(source) else {
        return Ok(Cow::Borrowed(source));
    };
    if entities.values.is_empty() || !source[body..].contains('&') {
        return Ok(Cow::Borrowed(source));
    }
    let mut text = String::with_capacity(source.len());
    text.push_str(&source[..body]);
    let mut rest = &source[body..];
    while let Some(at) = rest.find(['&', '<']) {
        text.push_str(&rest[..at]);
        rest = &rest[at..];
        let verbatim = VERBATIM
            .iter()
            .find(|(open, _)| rest.starts_with(open))
            .map(|(_, close)| rest.find(close).map_or(rest.len(), |end| end + close.len()));
        let taken = match verbatim {
            Some(length) => {
                text.push_str(&rest[..length]);
                length
            }
            None => entities.reference(rest, 0, &mut text)?,
        };
        rest = &rest[taken..];
    }
    text.push_str(rest);
    Ok(Cow::Owned(text))
}

impl Entities<'_> {
    fn reference(&self, text: &str, depth: usize, out: &mut String) -> Result<usize> {
        let Some((name, value)) = self.named(text) else {
            out.push_str(&text[..1]);
            return Ok(1);
        };
        if depth >= MAX_NESTING {
            return Err(Error::TooComplex);
        }
        let mut rest = value;
        while let Some(at) = rest.find('&') {
            out.push_str(&rest[..at]);
            rest = &rest[at..];
            let taken = self.reference(rest, depth + 1, out)?;
            rest = &rest[taken..];
        }
        out.push_str(rest);
        if out.len() > MAX_EXPANDED {
            return Err(Error::TooComplex);
        }
        Ok(name.len() + 2)
    }

    fn named<'t>(&self, text: &'t str) -> Option<(&'t str, &str)> {
        let rest = text.strip_prefix('&')?;
        let end = rest.find(|character: char| {
            !(character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | ':'))
        })?;
        let name = &rest[..end];
        let value = *self.values.get(name)?;
        rest[end..].starts_with(';').then_some((name, value))
    }
}

fn declared(source: &str) -> Option<(Entities<'_>, usize)> {
    let start = source.find(DOCTYPE)?;
    let subset = start + source[start..].find(['[', '>'])?;
    if !source[subset..].starts_with('[') {
        return None;
    }
    let mut values = HashMap::new();
    let mut at = subset + 1;
    loop {
        let rest = &source[at..];
        let rest = rest.trim_start();
        at = source.len() - rest.len();
        if let Some(after) = rest.strip_prefix(']') {
            let close = after.find('>')?;
            return Some((Entities { values }, at + 1 + close + 1));
        }
        if rest.starts_with("<!--") {
            at += rest.find("-->")? + 3;
            continue;
        }
        if let Some(declaration) = rest.strip_prefix(ENTITY) {
            let declaration = declaration.trim_start();
            let (name, value) = declaration.split_once(char::is_whitespace)?;
            let value = value.trim_start();
            if let Some(quote) = value
                .chars()
                .next()
                .filter(|quote| matches!(quote, '"' | '\''))
                && name != "%"
            {
                let literal = &value[1..];
                let literal = &literal[..literal.find(quote)?];
                values.entry(name).or_insert(literal);
            }
        }
        at += markup_length(rest)?;
    }
}

fn markup_length(markup: &str) -> Option<usize> {
    if markup.starts_with('%') {
        return markup.find(';').map(|end| end + 1);
    }
    if !markup.starts_with('<') {
        return None;
    }
    let mut quote = None;
    for (index, character) in markup.char_indices() {
        match (quote, character) {
            (None, '"' | '\'') => quote = Some(character),
            (Some(open), _) if open == character => quote = None,
            (None, '>') => return Some(index + 1),
            _ => {}
        }
    }
    None
}
