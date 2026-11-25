mod channels;
mod community;
mod expressions;
mod hooks;
mod members;
mod mfa;
mod notify;
mod preloaded;
mod roles;
mod space;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use discord_core::ImageUpload;
use serde_json::Value;

use crate::error::{Error, Result};

fn data_uri(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn image(image: Option<&ImageUpload>) -> Value {
    image.map_or(Value::Null, |image| {
        Value::String(data_uri(&image.mime, &image.bytes))
    })
}

fn iso(millis: Option<i64>) -> Result<Value> {
    let Some(millis) = millis else {
        return Ok(Value::Null);
    };
    let instant = jiff::Timestamp::from_millisecond(millis).map_err(Error::unreadable)?;
    Ok(Value::String(instant.to_string()))
}

fn snowflakes<T: ToString>(ids: &[T]) -> Value {
    Value::Array(ids.iter().map(|id| Value::String(id.to_string())).collect())
}

fn edits_body<E>(edits: &[E], entry: fn(&E) -> (&'static str, Value)) -> Value {
    Value::Object(
        edits
            .iter()
            .map(|edit| {
                let (key, value) = entry(edit);
                (key.to_owned(), value)
            })
            .collect(),
    )
}
