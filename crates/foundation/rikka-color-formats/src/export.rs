use rikka_settings::session::color_picker::{Format, HEX};
use serde::ser::{SerializeMap, Serializer};

use crate::rgb::Rgb;
use crate::template::Template;

const SEPARATOR: char = ';';

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grouping {
    ByColor,
    ByFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Document {
    Text,
    Json,
}

impl Document {
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Document::Text => "txt",
            Document::Json => "json",
        }
    }

    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Document> {
        match extension.to_ascii_lowercase().as_str() {
            "txt" => Some(Document::Text),
            "json" => Some(Document::Json),
            _ => None,
        }
    }
}

struct Group {
    key: String,
    entries: Vec<(String, String)>,
}

struct Groups<'a>(&'a [Group]);

struct Entries<'a>(&'a [(String, String)]);

impl serde::Serialize for Groups<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for group in self.0 {
            map.serialize_entry(&group.key, &Entries(&group.entries))?;
        }
        map.end()
    }
}

impl serde::Serialize for Entries<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

#[must_use]
pub fn export(
    colors: &[Rgb],
    formats: &[Format],
    grouping: Grouping,
    document: Document,
) -> String {
    let groups = grouped(colors, formats, grouping);
    match document {
        Document::Text => text(&groups),
        Document::Json => serde_json::to_string_pretty(&Groups(&groups))
            .expect("string maps always serialize to JSON"),
    }
}

fn grouped(colors: &[Rgb], formats: &[Format], grouping: Grouping) -> Vec<Group> {
    let color_key = |at: usize| format!("color{}", at + 1);
    let columns: Vec<(String, Template)> = formats
        .iter()
        .map(|format| (format.name.to_uppercase(), Template::parse(&format.format)))
        .collect();
    match grouping {
        Grouping::ByColor => colors
            .iter()
            .enumerate()
            .map(|(at, rgb)| Group {
                key: color_key(at),
                entries: columns
                    .iter()
                    .map(|(heading, template)| (heading.clone(), entry(heading, template, *rgb)))
                    .collect(),
            })
            .collect(),
        Grouping::ByFormat => columns
            .iter()
            .map(|(heading, template)| Group {
                key: heading.clone(),
                entries: colors
                    .iter()
                    .enumerate()
                    .map(|(at, rgb)| (color_key(at), entry(heading, template, *rgb)))
                    .collect(),
            })
            .collect(),
    }
}

fn entry(heading: &str, template: &Template, rgb: Rgb) -> String {
    let rendered = template.render(rgb);
    let rendered = match heading == HEX {
        true => format!("#{rendered}"),
        false => rendered,
    };
    strip_heading(&rendered, heading)
}

fn strip_heading(rendered: &str, heading: &str) -> String {
    let needle: Vec<char> = heading.chars().flat_map(char::to_lowercase).collect();
    if needle.is_empty() {
        return rendered.to_owned();
    }
    let mut remaining = String::with_capacity(rendered.len());
    let mut rest = rendered;
    while let Some(first) = rest.chars().next() {
        match heading_match_len(rest, &needle) {
            Some(length) => rest = &rest[length..],
            None => {
                remaining.push(first);
                rest = &rest[first.len_utf8()..];
            }
        }
    }
    remaining
}

fn heading_match_len(rest: &str, needle: &[char]) -> Option<usize> {
    let mut lowered = Vec::with_capacity(needle.len());
    for (at, character) in rest.char_indices() {
        lowered.extend(character.to_lowercase());
        if !needle.starts_with(&lowered) {
            return None;
        }
        if lowered.len() == needle.len() {
            return Some(at + character.len_utf8());
        }
    }
    None
}

fn text(groups: &[Group]) -> String {
    let mut text = String::new();
    for group in groups {
        text.push_str(&group.key);
        for (key, value) in &group.entries {
            text.push(SEPARATOR);
            text.push_str(key);
            text.push_str(value);
        }
        text.push('\n');
    }
    text
}
