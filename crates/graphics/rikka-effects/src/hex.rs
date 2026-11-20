use rikka_types::Color;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serializer};

pub(crate) fn serialize<S: Serializer>(color: &Color, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&color.to_hex())
}

pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Color, D::Error> {
    let text = String::deserialize(deserializer)?;
    Color::parse_hex(&text)
        .ok_or_else(|| D::Error::custom(format!("{text} is not a #rrggbb colour")))
}
