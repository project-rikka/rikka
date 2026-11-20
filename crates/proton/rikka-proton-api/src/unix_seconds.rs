use jiff::Timestamp;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S: Serializer>(value: &Timestamp, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_i64(value.as_second())
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Timestamp, D::Error> {
    let seconds = i64::deserialize(deserializer)?;
    Timestamp::from_second(seconds)
        .map_err(|_| D::Error::custom(format!("timestamp {seconds} out of range")))
}

pub mod optional {
    use jiff::Timestamp;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &Option<Timestamp>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_i64(value.as_second()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Timestamp>, D::Error> {
        Ok(Option::<i64>::deserialize(deserializer)?
            .and_then(|seconds| Timestamp::from_second(seconds).ok()))
    }
}
