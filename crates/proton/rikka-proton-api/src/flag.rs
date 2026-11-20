use serde::{Deserialize, Deserializer, Serializer};

#[derive(Deserialize)]
#[serde(untagged)]
enum WireFlag {
    Number(i64),
    Bool(bool),
}

pub fn serialize<S: Serializer>(value: &bool, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u8(u8::from(*value))
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    Ok(match WireFlag::deserialize(deserializer)? {
        WireFlag::Number(number) => number != 0,
        WireFlag::Bool(truth) => truth,
    })
}

#[must_use]
pub fn default_true() -> bool {
    true
}

#[must_use]
pub fn to_query_value(value: bool) -> String {
    u8::from(value).to_string()
}

pub mod optional {
    use serde::{Deserialize, Deserializer, Serializer};

    use super::WireFlag;

    pub fn serialize<S: Serializer>(
        value: &Option<bool>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_u8(u8::from(*value)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<bool>, D::Error> {
        match Option::<WireFlag>::deserialize(deserializer)? {
            Some(WireFlag::Number(number)) => Ok(Some(number != 0)),
            Some(WireFlag::Bool(truth)) => Ok(Some(truth)),
            None => Ok(None),
        }
    }
}
