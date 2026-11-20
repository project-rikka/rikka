use std::time::Duration;

use rikka_net::header;
use serde_json::Value;

use rikka_llm::wire::{self, count_of, text_of};
use rikka_llm::{Effort, Error, ModelInfo, Price, Result};

const MODELS: &str = "https://openrouter.ai/api/v1/models";

const TIMEOUT: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

const PER_MILLION: f64 = 1_000_000.0;

pub(crate) async fn fetch() -> Result<Vec<ModelInfo>> {
    let body: Value = wire::client(TIMEOUT, CONNECT_TIMEOUT)?
        .get(MODELS)
        .header(header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(wire::translate)?
        .error_for_status()
        .await
        .map_err(wire::translate)?
        .json()
        .await
        .map_err(wire::translate)?;

    let published = body
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Malformed("the model list is not a list".to_owned()))?;

    Ok(published.iter().filter_map(read).collect())
}

fn read(published: &Value) -> Option<ModelInfo> {
    let id = text_of(published, "id");
    if id.is_empty() {
        return None;
    }

    let architecture = published.get("architecture");
    let top = published.get("top_provider");

    let context = match count_of(published, "context_length") {
        0 => top.map(|top| count_of(top, "context_length")).unwrap_or(0),
        length => length,
    };
    if context == 0 {
        return None;
    }

    let output = top
        .map(|top| count_of(top, "max_completion_tokens"))
        .filter(|tokens| *tokens > 0)
        .unwrap_or(context);

    let name = match text_of(published, "name") {
        name if name.is_empty() => id.clone(),
        name => name,
    };

    Some(
        ModelInfo::new(id, name, context, u32::try_from(output).unwrap_or(u32::MAX))
            .with_vision(modality(architecture, "image"))
            .with_tools(parameter(published, "tools"))
            .with_efforts(efforts(published.get("reasoning")))
            .with_price(price(published.get("pricing"))),
    )
}

fn modality(architecture: Option<&Value>, modality_name: &str) -> bool {
    architecture
        .and_then(|architecture| architecture.get("input_modalities"))
        .and_then(Value::as_array)
        .is_some_and(|modalities| {
            modalities
                .iter()
                .any(|modality| modality.as_str() == Some(modality_name))
        })
}

fn parameter(published: &Value, name: &str) -> bool {
    published
        .get("supported_parameters")
        .and_then(Value::as_array)
        .is_some_and(|parameters| {
            parameters
                .iter()
                .any(|parameter| parameter.as_str() == Some(name))
        })
}

fn efforts(reasoning: Option<&Value>) -> Vec<Effort> {
    let mut efforts: Vec<Effort> = reasoning
        .and_then(|reasoning| reasoning.get("supported_efforts"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|effort| Effort::from_id(effort.as_str()?))
        .collect();

    efforts.sort_unstable();
    efforts.dedup();
    efforts
}

fn price(pricing: Option<&Value>) -> Option<Price> {
    let pricing = pricing?;
    let input = rate(pricing, "prompt")?;
    let output = rate(pricing, "completion").unwrap_or_default();

    let mut price = Price::new(input, output);
    if let Some(read) = rate(pricing, "input_cache_read") {
        price = price.cache_read(read);
    }
    if let Some(write) = rate(pricing, "input_cache_write") {
        price = price.cache_write(write);
    }
    Some(price)
}

fn rate(pricing: &Value, field: &str) -> Option<f64> {
    let per_token: f64 = pricing.get(field)?.as_str()?.trim().parse().ok()?;
    (per_token.is_finite() && per_token >= 0.0).then_some(per_token * PER_MILLION)
}
