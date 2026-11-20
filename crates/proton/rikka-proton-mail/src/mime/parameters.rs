use std::collections::BTreeMap;
use std::fmt;

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

use crate::attachment::Disposition;
use crate::mime::{decode_charset, encoded_words};

const NOT_ATTRIBUTE_CHAR: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'!')
    .remove(b'#')
    .remove(b'$')
    .remove(b'&')
    .remove(b'+')
    .remove(b'-')
    .remove(b'.')
    .remove(b'^')
    .remove(b'_')
    .remove(b'`')
    .remove(b'|')
    .remove(b'~');

const SPECIALS: &[char] = &[
    '(', ')', '<', '>', '@', ',', ';', ':', '\\', '"', '/', '[', ']', '?', '=',
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parameters {
    entries: Vec<(String, String)>,
}

impl Parameters {
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(parameter_name, _)| parameter_name.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn set(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        match self
            .entries
            .iter_mut()
            .find(|(parameter_name, _)| parameter_name.eq_ignore_ascii_case(name))
        {
            Some((_, parameter_name)) => *parameter_name = value,
            None => self.entries.push((name.to_ascii_lowercase(), value)),
        }
    }

    fn parse(segments: &[String]) -> Parameters {
        let mut plain: Vec<(String, String)> = Vec::new();
        let mut sectioned: BTreeMap<String, Vec<Section>> = BTreeMap::new();

        for segment in segments {
            let Some((name, value)) = segment.split_once('=') else {
                continue;
            };
            let name = name.trim().to_ascii_lowercase();
            if name.is_empty() {
                continue;
            }
            let (value, quoted) = unquoted(value.trim());

            match name.split_once('*') {
                Some((base, marker)) => {
                    let (index, extended) = match marker.strip_suffix('*') {
                        Some(number) => (number, true),
                        None if marker.is_empty() => ("", true),
                        None => (marker, false),
                    };
                    let index = match index {
                        "" => 0,
                        number => match number.parse::<u32>() {
                            Ok(index) => index,
                            Err(_) => continue,
                        },
                    };
                    sectioned.entry(base.to_owned()).or_default().push(Section {
                        index,
                        extended: extended && !quoted,
                        value,
                    });
                }
                None => {
                    let value = match value.contains("=?") {
                        true => encoded_words::decode(&value),
                        false => value,
                    };
                    plain.push((name, value));
                }
            }
        }

        for (base, mut sections) in sectioned {
            sections.sort_by_key(|section| section.index);
            let joined = joined(&sections);
            plain.retain(|(name, _)| *name != base);
            plain.push((base, joined));
        }

        Parameters { entries: plain }
    }
}

impl fmt::Display for Parameters {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (name, value) in &self.entries {
            HeaderParameter::new(name, value).fmt(formatter)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HeaderParameter<'a> {
    name: &'a str,
    value: &'a str,
}

impl<'a> HeaderParameter<'a> {
    pub(crate) fn new(name: &'a str, value: &'a str) -> HeaderParameter<'a> {
        HeaderParameter { name, value }
    }
}

impl fmt::Display for HeaderParameter<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.name;
        match self.value.is_ascii() {
            true => {
                write!(formatter, "; {name}=")?;
                write_quoted(formatter, self.value)
            }
            false => {
                let encoded = utf8_percent_encode(self.value, NOT_ATTRIBUTE_CHAR);
                write!(formatter, "; {name}*=utf-8''{encoded}")
            }
        }
    }
}

struct Section {
    index: u32,
    extended: bool,
    value: String,
}

fn joined(sections: &[Section]) -> String {
    let mut label: Option<String> = None;
    let mut bytes = Vec::new();

    for (position, section) in sections.iter().enumerate() {
        if !section.extended {
            bytes.extend_from_slice(section.value.as_bytes());
            continue;
        }

        let mut encoded = section.value.as_str();
        if position == 0 {
            let mut pieces = encoded.splitn(3, '\'');
            if let (Some(named), Some(_), Some(rest)) =
                (pieces.next(), pieces.next(), pieces.next())
            {
                label = (!named.is_empty()).then(|| named.to_owned());
                encoded = rest;
            }
        }
        bytes.extend(percent_decode_str(encoded));
    }

    match label {
        Some(label) => decode_charset(&bytes, Some(&label)),
        None => String::from_utf8_lossy(&bytes).into_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentType {
    kind: String,
    subtype: String,
    parameters: Parameters,
}

impl ContentType {
    pub(crate) fn parse(value: &str) -> Option<ContentType> {
        let segments = segments(value);
        let (kind, subtype) = segments.first()?.split_once('/')?;
        let kind = kind.trim().to_ascii_lowercase();
        let subtype = subtype.trim().to_ascii_lowercase();

        let is_token = |word: &str| {
            !word.is_empty()
                && !word
                    .chars()
                    .any(|character| character.is_whitespace() || SPECIALS.contains(&character))
        };
        if !is_token(&kind) || !is_token(&subtype) {
            return None;
        }

        Some(ContentType {
            kind,
            subtype,
            parameters: Parameters::parse(&segments[1..]),
        })
    }

    pub(crate) fn new(kind: &str, subtype: &str) -> ContentType {
        ContentType {
            kind: kind.to_owned(),
            subtype: subtype.to_owned(),
            parameters: Parameters::default(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub fn subtype(&self) -> &str {
        &self.subtype
    }

    #[must_use]
    pub fn essence(&self) -> String {
        format!("{}/{}", self.kind, self.subtype)
    }

    #[must_use]
    pub fn is(&self, essence: &str) -> bool {
        essence.split_once('/').is_some_and(|(kind, subtype)| {
            kind.eq_ignore_ascii_case(&self.kind) && subtype.eq_ignore_ascii_case(&self.subtype)
        })
    }

    #[must_use]
    pub fn is_multipart(&self) -> bool {
        self.kind == "multipart"
    }

    #[must_use]
    pub fn is_text(&self) -> bool {
        self.kind == "text"
    }

    #[must_use]
    pub fn is_message(&self) -> bool {
        self.kind == "message" && matches!(self.subtype.as_str(), "rfc822" | "global")
    }

    #[must_use]
    pub fn parameters(&self) -> &Parameters {
        &self.parameters
    }

    #[must_use]
    pub fn parameter(&self, name: &str) -> Option<&str> {
        self.parameters.get(name)
    }

    #[must_use]
    pub fn charset(&self) -> Option<&str> {
        self.parameters.get("charset")
    }

    #[must_use]
    pub fn boundary(&self) -> Option<&str> {
        self.parameters
            .get("boundary")
            .filter(|found| !found.is_empty())
    }

    pub(crate) fn set_parameter(&mut self, name: &str, value: impl Into<String>) {
        self.parameters.set(name, value);
    }

    pub(crate) fn header_value(&self) -> String {
        format!("{}{}", self.essence(), self.parameters)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDisposition {
    kind: Disposition,
    parameters: Parameters,
}

impl ContentDisposition {
    pub(crate) fn parse(value: &str) -> Option<ContentDisposition> {
        let segments = segments(value);
        let kind = segments.first()?.trim().to_ascii_lowercase();
        if kind.is_empty() {
            return None;
        }

        Some(ContentDisposition {
            kind: Disposition::from(kind.as_str()),
            parameters: Parameters::parse(&segments[1..]),
        })
    }

    #[must_use]
    pub fn kind(&self) -> &Disposition {
        &self.kind
    }

    #[must_use]
    pub fn is_attachment(&self) -> bool {
        matches!(self.kind, Disposition::Attachment)
    }

    #[must_use]
    pub fn is_inline(&self) -> bool {
        self.kind.is_inline()
    }

    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        self.parameters
            .get("filename")
            .filter(|name| !name.is_empty())
    }

    #[must_use]
    pub fn parameters(&self) -> &Parameters {
        &self.parameters
    }
}

fn segments(value: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = value.chars();
    let mut depth = 0usize;

    while let Some(character) = chars.next() {
        match character {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth > 0 => {}
            '"' => {
                current.push('"');
                while let Some(inside) = chars.next() {
                    current.push(inside);
                    match inside {
                        '\\' => {
                            if let Some(escaped) = chars.next() {
                                current.push(escaped);
                            }
                        }
                        '"' => break,
                        _ => {}
                    }
                }
            }
            ';' => segments.push(std::mem::take(&mut current)),
            other => current.push(other),
        }
    }

    segments.push(current);
    segments
        .into_iter()
        .map(|segment| segment.trim().to_owned())
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn unquoted(value: &str) -> (String, bool) {
    let Some(inner) = value.strip_prefix('"') else {
        return (value.to_owned(), false);
    };

    let mut unescaped = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(character) = chars.next() {
        match character {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    unescaped.push(escaped);
                }
            }
            '"' => break,
            other => unescaped.push(other),
        }
    }

    (unescaped, true)
}

fn write_quoted(out: &mut impl fmt::Write, value: &str) -> fmt::Result {
    let is_token = !value.is_empty()
        && !value.chars().any(|character| {
            character.is_whitespace() || character.is_control() || SPECIALS.contains(&character)
        });
    if is_token {
        return out.write_str(value);
    }

    out.write_char('"')?;
    for character in value.chars() {
        if character == '"' || character == '\\' {
            out.write_char('\\')?;
        }
        out.write_char(character)?;
    }
    out.write_char('"')
}
