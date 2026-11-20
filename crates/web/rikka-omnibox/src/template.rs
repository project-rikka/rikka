use std::fmt::{self, Write};

use url::Url;

use crate::error::{Error, Result};

const TERMS: [&str; 3] = ["{searchTerms}", "%s", "{searchTerms?}"];
const ENCODINGS: [&str; 4] = [
    "{inputEncoding}",
    "{outputEncoding}",
    "{inputEncoding?}",
    "{outputEncoding?}",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    Path,
    Query,
}

pub(crate) fn validate(template: &str) -> Result<()> {
    let template = template.trim();
    if !TERMS
        .iter()
        .any(|placeholder| template.contains(placeholder))
    {
        return Err(Error::MissingPlaceholder(template.to_owned()));
    }
    let url =
        expand(template, "rikka").ok_or_else(|| Error::InvalidTemplate(template.to_owned()))?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(Error::InvalidTemplate(template.to_owned()));
    }
    Ok(())
}

pub(crate) fn expand(template: &str, terms: &str) -> Option<Url> {
    let mut url = String::with_capacity(template.len() + terms.len() * 3);
    let mut placement = Placement::Path;
    let mut rest = template;
    while !rest.is_empty() {
        if let Some(placeholder) = TERMS
            .iter()
            .find(|placeholder| rest.starts_with(**placeholder))
        {
            encode(&mut url, terms, placement).ok()?;
            rest = &rest[placeholder.len()..];
            continue;
        }
        if let Some(placeholder) = ENCODINGS
            .iter()
            .find(|placeholder| rest.starts_with(**placeholder))
        {
            url.push_str("UTF-8");
            rest = &rest[placeholder.len()..];
            continue;
        }
        if let Some(optional) = optional_parameter(rest) {
            rest = &rest[optional..];
            continue;
        }
        let character = rest.chars().next()?;
        if matches!(character, '?' | '#') {
            placement = Placement::Query;
        }
        url.push(character);
        rest = &rest[character.len_utf8()..];
    }
    Url::parse(&url).ok()
}

fn optional_parameter(text: &str) -> Option<usize> {
    let inner = text.strip_prefix('{')?;
    let end = inner.find('}')?;
    let name = inner[..end].strip_suffix('?')?;
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b':' || byte == b'_');
    valid.then_some(end + 2)
}

fn encode(target: &mut impl Write, terms: &str, placement: Placement) -> fmt::Result {
    for byte in terms.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' => {
                target.write_char(char::from(byte))?;
            }
            b'*' if placement == Placement::Query => target.write_char('*')?,
            b'~' if placement == Placement::Path => target.write_char('~')?,
            b' ' if placement == Placement::Query => target.write_char('+')?,
            _ => write!(target, "%{byte:02X}")?,
        }
    }
    Ok(())
}
