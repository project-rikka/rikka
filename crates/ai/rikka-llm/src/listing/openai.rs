use serde_json::Value;

use crate::error::{Error, Result};
use crate::listing::{ListedModel, day_of_unix, text};
use crate::transport::Transport;

pub async fn models(transport: &Transport) -> Result<Vec<ListedModel>> {
    let answer = transport.get().await?;
    let models = answer
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::Malformed("the model listing holds no data".to_owned()))?;
    Ok(models.iter().filter_map(model).collect())
}

fn model(model: &Value) -> Option<ListedModel> {
    let shutting_down = model
        .get("shutdown_date")
        .is_some_and(|date| !date.is_null());

    Some(
        ListedModel::new(text(model, "id")?)
            .with_released(
                model
                    .get("created")
                    .and_then(Value::as_i64)
                    .and_then(day_of_unix),
            )
            .with_deprecated(shutting_down),
    )
}
