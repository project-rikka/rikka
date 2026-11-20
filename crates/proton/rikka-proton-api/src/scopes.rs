use std::fmt;

use serde::de::{self, Deserializer, SeqAccess, Visitor};

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    struct Scopes;

    impl<'de> Visitor<'de> for Scopes {
        type Value = Vec<String>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a list of scopes or a space separated line of them")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Vec<String>, E> {
            Ok(value.split_whitespace().map(str::to_owned).collect())
        }

        fn visit_unit<E: de::Error>(self) -> Result<Vec<String>, E> {
            Ok(Vec::new())
        }

        fn visit_none<E: de::Error>(self) -> Result<Vec<String>, E> {
            Ok(Vec::new())
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Vec<String>, A::Error> {
            let mut scopes = Vec::new();
            while let Some(scope) = sequence.next_element::<String>()? {
                scopes.push(scope);
            }
            Ok(scopes)
        }
    }

    deserializer.deserialize_any(Scopes)
}
