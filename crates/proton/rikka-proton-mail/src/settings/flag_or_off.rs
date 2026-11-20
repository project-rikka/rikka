use serde::{Deserializer, Serializer};

pub(crate) fn serialize<S: Serializer>(value: &bool, serializer: S) -> Result<S::Ok, S::Error> {
    rikka_proton_api::flag::serialize(value, serializer)
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(rikka_proton_api::flag::optional::deserialize(deserializer)?.unwrap_or(false))
}
