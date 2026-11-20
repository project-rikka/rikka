use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};

const BRANDS_STYLE: &str = "brands";

#[derive(Debug)]
pub(crate) struct Icon {
    pub(crate) name: String,
    pub(crate) codepoint: char,
}

#[derive(Deserialize)]
struct FontAwesomeIcon {
    unicode: String,
    styles: Vec<String>,
    #[serde(default)]
    aliases: Option<Aliases>,
}

#[derive(Deserialize)]
struct Aliases {
    #[serde(default)]
    names: Vec<String>,
}

pub(crate) fn material(path: &Path, text: &str) -> Result<Vec<Icon>> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            let invalid = |message: &str| Error::Listing {
                path: path.to_owned(),
                line: index + 1,
                message: message.to_owned(),
            };
            let mut fields = line.split_whitespace();
            let (Some(name), Some(value), None) = (fields.next(), fields.next(), fields.next())
            else {
                return Err(invalid("expected `<name> <hex codepoint>`"));
            };
            icon(name, value)
        })
        .collect()
}

pub(crate) fn brands(path: &Path, text: &str) -> Result<Vec<Icon>> {
    let listed: BTreeMap<String, FontAwesomeIcon> =
        serde_json::from_str(text).map_err(|source| Error::Json {
            path: path.to_owned(),
            source,
        })?;
    let mut icons = Vec::new();
    for (name, listed) in &listed {
        if !listed.styles.iter().any(|style| style == BRANDS_STYLE) {
            continue;
        }
        let aliases = listed.aliases.iter().flat_map(|aliases| &aliases.names);
        for name in std::iter::once(name).chain(aliases) {
            icons.push(icon(name, &listed.unicode)?);
        }
    }
    Ok(icons)
}

fn icon(name: &str, value: &str) -> Result<Icon> {
    let codepoint = u32::from_str_radix(value, 16)
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| Error::Codepoint {
            name: name.to_owned(),
            value: value.to_owned(),
        })?;
    Ok(Icon {
        name: name.to_owned(),
        codepoint,
    })
}
