use std::collections::HashMap;

use jiff::civil::Date;
use rikka_llm::{Effort, Error, Price, Result};
use serde::Deserialize;
use serde_json::Value;

const TEXT: &str = "text";
const PICTURE: &str = "image";
const EFFORT: &str = "effort";
const DEPRECATED: &str = "deprecated";

#[derive(Debug, Default)]
pub struct Catalog {
    providers: HashMap<String, HashMap<String, Entry>>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Entry {
    #[serde(default)]
    name: String,
    #[serde(default)]
    tool_call: bool,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    modalities: Modalities,
    #[serde(default)]
    limit: Limit,
    #[serde(default)]
    reasoning_options: Vec<ReasoningOption>,
    #[serde(default)]
    cost: Option<Cost>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Modalities {
    #[serde(default)]
    input: Vec<String>,
    #[serde(default)]
    output: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct Limit {
    #[serde(default)]
    context: u64,
    #[serde(default)]
    output: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct ReasoningOption {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    values: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct Cost {
    input: f64,
    output: f64,
    #[serde(default)]
    cache_read: Option<f64>,
    #[serde(default)]
    cache_write: Option<f64>,
}

#[derive(Deserialize)]
struct RawProvider {
    #[serde(default)]
    models: HashMap<String, Value>,
}

impl Catalog {
    pub fn parse(document: &[u8]) -> Result<Catalog> {
        let raw: HashMap<String, RawProvider> =
            serde_json::from_slice(document).map_err(|error| {
                Error::Malformed(format!("the model catalog is unreadable: {error}"))
            })?;
        let providers = raw
            .into_iter()
            .map(|(provider, raw)| {
                let models = raw
                    .models
                    .into_iter()
                    .filter_map(|(id, entry)| Some((id, serde_json::from_value(entry).ok()?)))
                    .collect();
                (provider, models)
            })
            .collect();
        Ok(Catalog { providers })
    }

    pub(crate) fn entry(&self, provider: &str, id: &str) -> Option<&Entry> {
        self.providers.get(provider)?.get(id)
    }
}

impl Entry {
    pub(crate) fn name(&self) -> Option<&str> {
        Some(self.name.trim()).filter(|name| !name.is_empty())
    }

    pub(crate) fn is_conversational(&self) -> bool {
        self.tool_call && self.modalities.output.iter().all(|output| output == TEXT)
    }

    pub(crate) fn context(&self) -> Option<u64> {
        Some(self.limit.context).filter(|tokens| *tokens > 0)
    }

    pub(crate) fn output(&self) -> Option<u32> {
        u32::try_from(self.limit.output)
            .ok()
            .filter(|tokens| *tokens > 0)
    }

    pub(crate) fn vision(&self) -> bool {
        self.modalities.input.iter().any(|input| input == PICTURE)
    }

    pub(crate) fn efforts(&self) -> Option<Vec<Effort>> {
        self.reasoning_options
            .iter()
            .find(|option| option.kind == EFFORT)
            .map(|option| {
                option
                    .values
                    .iter()
                    .filter_map(|value| Effort::from_id(value))
                    .collect()
            })
    }

    pub(crate) fn price(&self) -> Option<Price> {
        self.cost.map(|cost| {
            let price = Price::new(cost.input, cost.output);
            let price = match cost.cache_read {
                Some(read) => price.cache_read(read),
                None => price,
            };
            match cost.cache_write {
                Some(write) => price.cache_write(write),
                None => price,
            }
        })
    }

    pub(crate) fn released(&self) -> Option<Date> {
        self.release_date.as_deref()?.parse().ok()
    }

    pub(crate) fn is_deprecated(&self) -> bool {
        self.status.as_deref() == Some(DEPRECATED)
    }
}
