use std::cmp::Reverse;
use std::collections::HashMap;

use jiff::ToSpan;
use jiff::civil::Date;
use rikka_llm::ModelInfo;
use rikka_llm::wire::ListedModel;

use crate::document::{Catalog, Entry};

const STALE_AFTER_DAYS: i64 = 183;
const SHORTEST_PREFIX: usize = 2;

#[derive(Debug, Clone, Copy)]
pub enum Lookup {
    Exact(&'static str),
    Prefix(&'static [&'static str]),
}

struct Described {
    info: ModelInfo,
    vendor: &'static str,
    line: String,
    released: Option<Date>,
    deprecated: bool,
}

pub fn lineup(listed: Vec<ListedModel>, catalog: &Catalog, lookup: Lookup) -> Vec<ModelInfo> {
    let mut described: Vec<Described> = listed
        .into_iter()
        .filter_map(|model| describe(model, catalog, lookup))
        .collect();

    let newest_of_vendor = newest_by(&described, |model| model.vendor.to_owned());
    let newest_of_line = newest_by(&described, line_key);
    let superseded: Vec<bool> = described
        .iter()
        .map(|model| {
            let line_newest = newest_of_line.get(&line_key(model)).copied().flatten();
            let stale = line_newest
                .zip(newest_of_vendor.get(model.vendor).copied().flatten())
                .is_some_and(|(line, vendor)| {
                    vendor
                        .checked_sub(STALE_AFTER_DAYS.days())
                        .is_ok_and(|cutoff| line < cutoff)
                });
            let outrun = model
                .released
                .zip(line_newest)
                .is_some_and(|(released, newest)| released < newest);
            model.deprecated || stale || outrun
        })
        .collect();
    for (model, superseded) in described.iter_mut().zip(superseded) {
        model.info = model.info.clone().with_superseded(superseded);
    }

    described.sort_by_key(|model| (model.info.is_superseded(), Reverse(model.released)));
    described.into_iter().map(|model| model.info).collect()
}

fn describe(model: ListedModel, catalog: &Catalog, lookup: Lookup) -> Option<Described> {
    let found = find(catalog, lookup, model.id());
    let entry = found.map(|(_, entry)| entry);
    if entry.is_some_and(|entry| !entry.is_conversational()) {
        return None;
    }
    let context = model.context().or_else(|| entry?.context())?;
    let output = model.output().or_else(|| entry?.output()).unwrap_or(0);
    let name = model
        .name()
        .or_else(|| entry?.name())
        .unwrap_or(model.id())
        .to_owned();
    let line = line_of(entry.and_then(Entry::name).unwrap_or(&name))
        .unwrap_or_else(|| model.id().to_owned());
    let vision = model.vision().or(entry.map(Entry::vision)).unwrap_or(false);
    let efforts = model
        .efforts()
        .map(<[_]>::to_vec)
        .or_else(|| entry?.efforts())
        .map(|mut efforts| {
            efforts.sort_unstable();
            efforts.dedup();
            efforts
        })
        .unwrap_or_default();

    let info = ModelInfo::new(model.id(), name, context, output)
        .with_vision(vision)
        .with_efforts(efforts)
        .with_thinking_off(model.thinking_off().unwrap_or_default())
        .with_server_tools(model.server_tools().map(<[_]>::to_vec).unwrap_or_default())
        .with_price(entry.and_then(Entry::price));
    Some(Described {
        info,
        vendor: found.map_or("", |(vendor, _)| vendor),
        line,
        released: model.released().or_else(|| entry?.released()),
        deprecated: model.is_deprecated() || entry.is_some_and(Entry::is_deprecated),
    })
}

fn find<'a>(catalog: &'a Catalog, lookup: Lookup, id: &str) -> Option<(&'static str, &'a Entry)> {
    match lookup {
        Lookup::Exact(provider) => catalog.entry(provider, id).map(|entry| (provider, entry)),
        Lookup::Prefix(providers) => {
            let segments: Vec<&str> = id.split('-').collect();
            (SHORTEST_PREFIX..=segments.len()).rev().find_map(|kept| {
                let prefix = segments[..kept].join("-");
                providers.iter().find_map(|provider| {
                    catalog
                        .entry(provider, &prefix)
                        .map(|entry| (*provider, entry))
                })
            })
        }
    }
}

fn line_of(name: &str) -> Option<String> {
    let mut plain = String::with_capacity(name.len());
    let mut depth = 0usize;
    for character in name.chars() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => plain.push(character),
            _ => {}
        }
    }
    let words: Vec<String> = plain
        .split(|character: char| character.is_whitespace() || character == '-')
        .filter(|word| {
            !word.is_empty() && !word.chars().any(|character| character.is_ascii_digit())
        })
        .map(str::to_lowercase)
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

fn line_key(model: &Described) -> String {
    format!("{}\n{}", model.vendor, model.line)
}

fn newest_by(
    described: &[Described],
    key: impl Fn(&Described) -> String,
) -> HashMap<String, Option<Date>> {
    let mut newest: HashMap<String, Option<Date>> = HashMap::new();
    for model in described {
        let held = newest.entry(key(model)).or_default();
        *held = (*held).max(model.released);
    }
    newest
}
