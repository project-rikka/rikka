use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;

pub(crate) fn number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => number.as_u64().unwrap_or(0),
        Value::String(text) => text.parse().unwrap_or(0),
        _ => 0,
    })
}

pub(crate) fn id<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => Some(number.to_string()).filter(|id| id != "0"),
        Value::String(text) => Some(text).filter(|id| !id.is_empty() && id != "0"),
        _ => None,
    })
}

pub(crate) fn text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => Some(number.to_string()),
        Value::String(text) => Some(text),
        _ => None,
    })
}

pub(crate) fn kind<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Number(number) => number
            .as_u64()
            .and_then(|kind| u8::try_from(kind).ok())
            .unwrap_or(0),
        Value::String(text) if text == "member" => 1,
        _ => 0,
    })
}

pub(crate) fn snowflake<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(text(deserializer)?.unwrap_or_default())
}

pub(crate) fn snowflakes<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<String>, D::Error> {
    Ok(Option::<Vec<Value>>::deserialize(deserializer)?
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| match value {
            Value::Number(number) => Some(number.to_string()),
            Value::String(text) => Some(text),
            _ => None,
        })
        .collect())
}

pub(crate) fn list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    Ok(readable(
        Option::<Vec<Value>>::deserialize(deserializer)?.unwrap_or_default(),
    ))
}

pub(crate) fn flag<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(Option::<bool>::deserialize(deserializer)?.unwrap_or(false))
}

pub(crate) fn listed<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Listed {
        Entries { entries: Vec<Value> },
        Plain(Vec<Value>),
    }

    Ok(match Option::<Listed>::deserialize(deserializer)? {
        Some(Listed::Entries { entries }) | Some(Listed::Plain(entries)) => readable(entries),
        None => Vec::new(),
    })
}

fn readable<T: DeserializeOwned>(values: Vec<Value>) -> Vec<T> {
    values
        .into_iter()
        .filter_map(|value| match T::deserialize(value) {
            Ok(read) => Some(read),
            Err(error) => {
                tracing::warn!(
                    %error,
                    kind = std::any::type_name::<T>(),
                    "skipping unreadable Discord entry"
                );
                None
            }
        })
        .collect()
}
