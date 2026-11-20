use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;

use serde::Deserialize;
use serde::de::value::UnitDeserializer;
use serde::de::{
    DeserializeOwned, DeserializeSeed, Deserializer, Error as _, IgnoredAny, MapAccess, Visitor,
};

pub(crate) fn decode<T: DeserializeOwned>(path: &[&str], body: &[u8]) -> serde_json::Result<T> {
    match find(path, body)? {
        Some(value) => Ok(value),
        None => T::deserialize(UnitDeserializer::<serde_json::Error>::new())
            .map_err(|_| serde_json::Error::custom(format!("missing field `{}`", path.join(".")))),
    }
}

pub(crate) fn decode_or_default<T: DeserializeOwned + Default>(
    path: &[&str],
    body: &[u8],
) -> serde_json::Result<T> {
    Ok(find(path, body)?.unwrap_or_default())
}

fn find<T: DeserializeOwned>(path: &[&str], body: &[u8]) -> serde_json::Result<Option<T>> {
    let mut deserializer = serde_json::Deserializer::from_slice(body);
    let found = Field {
        path,
        kind: PhantomData,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;
    Ok(found)
}

struct Field<'p, T> {
    path: &'p [&'p str],
    kind: PhantomData<T>,
}

impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for Field<'_, T> {
    type Value = Option<T>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Option<T>, D::Error> {
        deserializer.deserialize_option(self)
    }
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for Field<'_, T> {
    type Value = Option<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "a response object holding {}",
            self.path.join(".")
        )
    }

    fn visit_none<E: serde::de::Error>(self) -> Result<Option<T>, E> {
        Ok(None)
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Option<T>, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Option<T>, D::Error> {
        deserializer.deserialize_map(self)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Option<T>, A::Error> {
        let Some((name, rest)) = self.path.split_first() else {
            return Err(A::Error::custom("empty field path"));
        };
        let mut found = None;
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            if key != *name {
                map.next_value::<IgnoredAny>()?;
            } else if rest.is_empty() {
                found = Some(map.next_value()?);
            } else {
                found = map.next_value_seed(Field {
                    path: rest,
                    kind: PhantomData,
                })?;
            }
        }
        Ok(found)
    }
}
