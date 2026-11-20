use serde_json::Value;

use crate::error::{Error, Result};
use crate::listing::{ListedModel, efforts, text, tokens};
use crate::transport::Transport;

const LISTED: &str = "list";

pub async fn models(transport: &Transport) -> Result<Vec<ListedModel>> {
    let answer = transport.get().await?;
    let models = answer
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Malformed("the model listing holds no models".to_owned()))?;

    let mut listed: Vec<(i64, ListedModel)> = models
        .iter()
        .filter(|model| model.get("visibility").and_then(Value::as_str) == Some(LISTED))
        .filter_map(|model| {
            let priority = model
                .get("priority")
                .and_then(Value::as_i64)
                .unwrap_or(i64::MAX);
            Some((priority, self::model(model)?))
        })
        .collect();
    listed.sort_by_key(|(priority, _)| *priority);
    Ok(listed.into_iter().map(|(_, model)| model).collect())
}

fn model(model: &Value) -> Option<ListedModel> {
    let levels = model
        .get("supported_reasoning_levels")
        .and_then(Value::as_array)
        .map(|levels| {
            efforts(
                levels
                    .iter()
                    .filter_map(|level| level.get("effort").and_then(Value::as_str)),
            )
        });
    let vision = model
        .get("input_modalities")
        .and_then(Value::as_array)
        .map(|inputs| inputs.iter().any(|input| input.as_str() == Some("image")));
    let upgraded = model
        .get("upgrade")
        .is_some_and(|upgrade| !upgrade.is_null());

    Some(
        ListedModel::new(text(model, "slug")?)
            .with_name(text(model, "display_name"))
            .with_context(tokens(model, "context_window"))
            .with_vision(vision)
            .with_efforts(levels)
            .with_deprecated(upgraded),
    )
}
