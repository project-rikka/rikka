use rikka_types::Color;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value as Json};

use crate::ops::Parsed;
use crate::order::{Change, LayerRef, Order, Request};
use crate::vocabulary::{Fields, color, named};

const MAX_NAME: usize = 80;
const MAX_STEPS: usize = 64;

pub(crate) struct Item {
    kind: &'static str,
    fields: Map<String, Json>,
}

impl Item {
    pub(crate) fn read(
        mut fields: Map<String, Json>,
        key: &str,
        table: &[(&'static str, Fields)],
        what: &str,
    ) -> Parsed<Item> {
        let requested = match fields.remove(key) {
            Some(Json::String(text)) => text,
            _ => {
                return Err(format!(
                    "each {what} names its `{key}`: {}",
                    table
                        .iter()
                        .map(|(name, _)| *name)
                        .collect::<Vec<&str>>()
                        .join(", ")
                ));
            }
        };
        let (kind, takes) = table
            .iter()
            .find(|(name, _)| *name == requested)
            .copied()
            .ok_or_else(|| {
                format!(
                    "there is no {what} \"{requested}\"; use {}",
                    table
                        .iter()
                        .map(|(name, _)| *name)
                        .collect::<Vec<&str>>()
                        .join(", ")
                )
            })?;
        if let Some(stray) = fields.keys().find(|field| !takes.contains(&field.as_str())) {
            return Err(match takes.is_empty() {
                true => format!("`{stray}` is not for {kind}; it takes nothing more"),
                false => format!("`{stray}` is not for {kind}; it takes {}", takes.join(", ")),
            });
        }
        Ok(Item { kind, fields })
    }

    pub(crate) fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) fn take<T: DeserializeOwned>(&mut self, field: &str) -> Parsed<Option<T>> {
        match self.fields.remove(field) {
            None | Some(Json::Null) => Ok(None),
            Some(value) => serde_json::from_value(value.clone())
                .map(Some)
                .map_err(|_| format!("`{field}` cannot be {value}")),
        }
    }

    pub(crate) fn need<T: DeserializeOwned>(&mut self, field: &str, hint: &str) -> Parsed<T> {
        self.take(field)?
            .ok_or_else(|| format!("{} needs `{field}`, {hint}", self.kind))
    }

    pub(crate) fn color(&mut self, field: &str) -> Parsed<Option<Color>> {
        self.fields
            .remove(field)
            .filter(|value| !value.is_null())
            .map(|value| color(&value).map_err(|error| format!("`{field}`: {error}")))
            .transpose()
    }

    pub(crate) fn named<T: Copy>(
        &mut self,
        field: &str,
        table: &[(&'static str, T)],
    ) -> Parsed<Option<T>> {
        self.take::<String>(field)?
            .map(|name| named(table, &name, field))
            .transpose()
    }

    pub(crate) fn percent(&mut self, field: &str) -> Parsed<Option<f32>> {
        self.number_in_range(field, 0.0, 100.0)
            .map(|value| value.map(|percent| percent / 100.0))
    }

    pub(crate) fn number_in_range(
        &mut self,
        field: &str,
        least: f32,
        most: f32,
    ) -> Parsed<Option<f32>> {
        match self.take::<f32>(field)? {
            Some(value) if !(least..=most).contains(&value) => {
                Err(format!("`{field}` is {value}; use {least}–{most}"))
            }
            other => Ok(other),
        }
    }

    pub(crate) fn layer(&mut self, field: &str) -> Parsed<Option<LayerRef>> {
        self.fields
            .remove(field)
            .filter(|value| !value.is_null())
            .map(|value| layer_ref(&value))
            .transpose()
    }
}

pub(crate) fn layer_ref(value: &Json) -> Parsed<LayerRef> {
    if let Some(index) = value.as_u64().and_then(|index| u32::try_from(index).ok()) {
        return Ok(LayerRef::Index(index));
    }
    match value.as_str().map(str::trim) {
        Some(name) if !name.is_empty() => Ok(LayerRef::Name(name.to_owned())),
        _ => Err(format!(
            "layer is {value}; give a layer's index or exact name as Document lists it"
        )),
    }
}

pub(crate) fn changed(document: Option<String>, name: String, change: Change) -> Parsed<Order> {
    let name = name.trim().to_owned();
    if name.is_empty() {
        return Err(
            "`name` is empty; name the change for the history, in the person's language".to_owned(),
        );
    }
    if name.chars().count() > MAX_NAME {
        return Err(format!(
            "`name` is longer than {MAX_NAME} characters; keep it a short history entry"
        ));
    }
    Ok(Order {
        document,
        request: Request::Change { name, change },
    })
}

pub(crate) fn counted<T>(items: &[T], what: &str) -> Parsed<()> {
    match items.len() {
        0 => Err(format!("there are no {what}")),
        count if count > MAX_STEPS => Err(format!(
            "{count} {what} were given; split them, at most {MAX_STEPS} per call"
        )),
        _ => Ok(()),
    }
}

pub(crate) fn sized(area: [f32; 4], what: &str) -> Parsed<[f32; 4]> {
    match area.iter().all(|value| value.is_finite()) && area[2] > 0.0 && area[3] > 0.0 {
        true => Ok(area),
        false => Err(format!(
            "{what} is {area:?}; give [x, y, width, height] with a positive size"
        )),
    }
}

pub(crate) fn numbered<T>(
    items: Vec<Map<String, Json>>,
    what: &str,
    each: impl Fn(Map<String, Json>) -> Parsed<T>,
) -> Parsed<Vec<T>> {
    counted(&items, &format!("{what}s"))?;
    items
        .into_iter()
        .enumerate()
        .map(|(at, item)| {
            let kind = item
                .get("op")
                .or_else(|| item.get("tool"))
                .and_then(Json::as_str)
                .map(|kind| format!(" ({kind})"))
                .unwrap_or_default();
            each(item).map_err(|error| format!("{what} {}{kind}: {error}", at + 1))
        })
        .collect()
}
