use std::collections::BTreeMap;

use crate::document::Document;
use crate::error::{Error, Result};
use crate::facts::Facts;

#[derive(Clone, Debug)]
pub struct Prompt {
    stem: String,
    name: String,
    body: String,
    deferred: bool,
    params: BTreeMap<String, String>,
    limits: BTreeMap<String, u64>,
}

impl Prompt {
    pub fn read(stem: &str, text: &str) -> Result<Prompt> {
        let document = Document::parse(stem, text)?;
        let name = document.string("name")?.unwrap_or_else(|| stem.to_owned());
        let deferred = document.flag("deferred")?;
        let params = document.strings("params")?.into_iter().collect();
        let limits = document.integers("limits")?.into_iter().collect();

        Ok(Prompt {
            stem: document.stem().to_owned(),
            name,
            body: document.into_body(),
            deferred,
            params,
            limits,
        })
    }

    #[must_use]
    pub fn stem(&self) -> &str {
        &self.stem
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn is_deferred(&self) -> bool {
        self.deferred
    }

    pub fn body(&self, facts: &Facts) -> Result<String> {
        self.fill(&self.body, facts)
    }

    pub fn param(&self, name: &str, facts: &Facts) -> Result<String> {
        let described = self.params.get(name).ok_or_else(|| Error::Unstated {
            document: self.stem.clone(),
            kind: "parameter",
            name: name.to_owned(),
        })?;
        self.fill(described, facts)
    }

    pub fn params(&self) -> impl Iterator<Item = &str> {
        self.params.keys().map(String::as_str)
    }

    pub fn limit(&self, name: &str) -> Result<u64> {
        self.limits
            .get(name)
            .copied()
            .ok_or_else(|| Error::Unstated {
                document: self.stem.clone(),
                kind: "limit",
                name: name.to_owned(),
            })
    }

    fn fill(&self, text: &str, facts: &Facts) -> Result<String> {
        let mut filled = String::with_capacity(text.len());
        let mut rest = text;

        while let Some(at) = rest.find("{{") {
            filled.push_str(&rest[..at]);
            let after = &rest[at + 2..];

            let named = after
                .find("}}")
                .map(|end| after.split_at(end))
                .filter(|(name, _)| is_placeholder(name));

            let Some((name, tail)) = named else {
                filled.push_str("{{");
                rest = after;
                continue;
            };

            filled.push_str(&self.value(name, facts)?);
            rest = &tail[2..];
        }

        filled.push_str(rest);
        Ok(filled)
    }

    fn value(&self, name: &str, facts: &Facts) -> Result<String> {
        if let Some(limit) = self.limits.get(name) {
            return Ok(limit.to_string());
        }
        facts
            .get(name)
            .map(str::to_owned)
            .ok_or_else(|| Error::Unresolved {
                document: self.stem.clone(),
                name: name.to_owned(),
            })
    }
}

fn is_placeholder(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}
