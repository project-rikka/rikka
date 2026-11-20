use crate::domain::Domain;
use crate::error::{Error, Result};
use crate::key::Key;
use crate::value::{Value, ValueType};

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Schema {
    domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    summary: Option<Box<str>>,
    #[serde(default)]
    keys: Vec<Key>,
}

impl Schema {
    pub fn new(domain: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            summary: None,
            keys: Vec::new(),
        }
    }

    #[must_use]
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into().into_boxed_str());
        self
    }

    #[must_use]
    pub fn key(mut self, key: Key) -> Self {
        self.keys.push(key);
        self
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    #[must_use]
    pub fn summary_text(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    #[must_use]
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    #[must_use]
    pub fn key_named(&self, name: &str) -> Option<&Key> {
        self.keys.iter().find(|key| key.name() == name)
    }

    pub fn require(&self, name: &str) -> Result<&Key> {
        self.key_named(name).ok_or_else(|| Error::UnknownKey {
            domain: self.domain.clone(),
            key: name.to_owned(),
        })
    }

    pub fn parse(&self, name: &str, text: &str) -> Result<Value> {
        let declared = self.require(name)?;
        match Value::from_literal(text) {
            Some(value) => Ok(value),
            None if declared.value_type() == ValueType::String => {
                Ok(Value::String(text.to_owned()))
            }
            None => Err(Error::refused(
                name,
                format!(
                    "`{text}` is not a value: expected {}",
                    declared.value_type()
                ),
            )),
        }
    }

    pub fn validate(&self) -> Result<Domain> {
        let domain: Domain = self.domain.parse()?;
        let mut seen: Vec<&str> = Vec::with_capacity(self.keys.len());
        for key in &self.keys {
            key.validate().map_err(|reason| Error::BadDeclaration {
                domain: self.domain.clone(),
                reason,
            })?;
            if seen.contains(&key.name()) {
                return Err(Error::DuplicateKey {
                    domain: self.domain.clone(),
                    key: key.name().to_owned(),
                });
            }
            seen.push(key.name());
        }
        Ok(domain)
    }
}
