use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use serde::de::{DeserializeOwned, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};

use rikka_prompt::FrontMatter;
use serde_json::Value;

use crate::fields;
use crate::front_matter_error::FrontMatterError;

const FENCE: &str = "---";

pub fn read<F: DeserializeOwned + Default>(text: &str) -> Result<(F, &str), FrontMatterError> {
    let (front, body) = match FrontMatter::split(text.trim_start_matches('\u{feff}'), FENCE) {
        FrontMatter::Absent => return Err(FrontMatterError::Missing),
        FrontMatter::Unclosed => return Err(FrontMatterError::Unclosed),
        FrontMatter::Fenced { front, body } => (front, body.trim()),
    };

    let fields = fields::parse(front);
    if fields.is_empty() {
        return Ok((F::default(), body));
    }
    F::deserialize(Value::Object(fields))
        .map(|front| (front, body))
        .map_err(FrontMatterError::Malformed)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Text(String);

impl Text {
    #[must_use]
    pub fn some(&self) -> Option<&str> {
        let value = self.0.trim();
        (!value.is_empty()).then_some(value)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fields(BTreeMap<String, String>);

impl Fields {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str).and_then(|value| {
            let value = value.trim();
            (!value.is_empty()).then_some(value)
        })
    }

    #[must_use]
    pub fn into_map(self) -> BTreeMap<String, String> {
        self.0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct List(Vec<String>);

impl List {
    #[must_use]
    pub fn into_vec(self) -> Vec<String> {
        self.0
    }
}

impl<'de> Deserialize<'de> for Text {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Text, D::Error> {
        deserializer.deserialize_any(TextVisitor)
    }
}

impl<'de> Deserialize<'de> for Fields {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Fields, D::Error> {
        deserializer.deserialize_any(FieldsVisitor)
    }
}

impl<'de> Deserialize<'de> for List {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<List, D::Error> {
        deserializer.deserialize_any(ListVisitor)
    }
}

struct TextVisitor;

impl<'de> Visitor<'de> for TextVisitor {
    type Value = Text;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a line of text")
    }

    fn visit_str<E>(self, value: &str) -> Result<Text, E> {
        Ok(Text(value.to_owned()))
    }

    fn visit_bool<E>(self, value: bool) -> Result<Text, E> {
        Ok(Text(value.to_string()))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Text, E> {
        Ok(Text(value.to_string()))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Text, E> {
        Ok(Text(value.to_string()))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Text, E> {
        Ok(Text(value.to_string()))
    }

    fn visit_unit<E>(self) -> Result<Text, E> {
        Ok(Text::default())
    }

    fn visit_none<E>(self) -> Result<Text, E> {
        Ok(Text::default())
    }

    fn visit_some<D: Deserializer<'de>>(self, inner: D) -> Result<Text, D::Error> {
        Text::deserialize(inner)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut list: A) -> Result<Text, A::Error> {
        while list.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Text::default())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut mapping: A) -> Result<Text, A::Error> {
        while mapping.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Text::default())
    }
}

struct FieldsVisitor;

impl<'de> Visitor<'de> for FieldsVisitor {
    type Value = Fields;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a mapping of text to text")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut mapping: A) -> Result<Fields, A::Error> {
        let mut fields = BTreeMap::new();
        while let Some((key, value)) = mapping.next_entry::<Text, Text>()? {
            if let Some(key) = key.some() {
                fields.insert(key.to_owned(), value.0);
            }
        }
        Ok(Fields(fields))
    }

    fn visit_unit<E>(self) -> Result<Fields, E> {
        Ok(Fields::default())
    }

    fn visit_none<E>(self) -> Result<Fields, E> {
        Ok(Fields::default())
    }

    fn visit_some<D: Deserializer<'de>>(self, inner: D) -> Result<Fields, D::Error> {
        Fields::deserialize(inner)
    }

    fn visit_str<E>(self, _said: &str) -> Result<Fields, E> {
        Ok(Fields::default())
    }

    fn visit_bool<E>(self, _said: bool) -> Result<Fields, E> {
        Ok(Fields::default())
    }

    fn visit_i64<E>(self, _said: i64) -> Result<Fields, E> {
        Ok(Fields::default())
    }

    fn visit_u64<E>(self, _said: u64) -> Result<Fields, E> {
        Ok(Fields::default())
    }

    fn visit_f64<E>(self, _said: f64) -> Result<Fields, E> {
        Ok(Fields::default())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut list: A) -> Result<Fields, A::Error> {
        while list.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Fields::default())
    }
}

struct ListVisitor;

impl<'de> Visitor<'de> for ListVisitor {
    type Value = List;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a list of text")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut list: A) -> Result<List, A::Error> {
        let mut words = Vec::new();
        while let Some(word) = list.next_element::<Text>()? {
            if let Some(word) = word.some() {
                words.push(word.to_owned());
            }
        }
        Ok(List(words))
    }

    fn visit_str<E>(self, value: &str) -> Result<List, E> {
        Ok(List(
            value
                .split(',')
                .map(str::trim)
                .filter(|word| !word.is_empty())
                .map(str::to_owned)
                .collect(),
        ))
    }

    fn visit_unit<E>(self) -> Result<List, E> {
        Ok(List::default())
    }

    fn visit_none<E>(self) -> Result<List, E> {
        Ok(List::default())
    }

    fn visit_some<D: Deserializer<'de>>(self, inner: D) -> Result<List, D::Error> {
        List::deserialize(inner)
    }

    fn visit_bool<E>(self, _said: bool) -> Result<List, E> {
        Ok(List::default())
    }

    fn visit_i64<E>(self, _said: i64) -> Result<List, E> {
        Ok(List::default())
    }

    fn visit_u64<E>(self, _said: u64) -> Result<List, E> {
        Ok(List::default())
    }

    fn visit_f64<E>(self, _said: f64) -> Result<List, E> {
        Ok(List::default())
    }

    fn visit_map<A: MapAccess<'de>>(self, mut mapping: A) -> Result<List, A::Error> {
        while mapping.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(List::default())
    }
}
