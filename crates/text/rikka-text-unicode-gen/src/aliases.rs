use std::collections::HashMap;

use crate::error::{Error, Result};
use crate::ucd::File;

#[derive(Debug)]
pub(crate) struct Aliases {
    properties: HashMap<String, Vec<Vec<String>>>,
}

impl Aliases {
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let mut properties: HashMap<String, Vec<Vec<String>>> = HashMap::new();
        for line in text.lines() {
            let data = line.split('#').next().unwrap_or("").trim();
            if data.is_empty() {
                continue;
            }
            let mut fields = data.split(';').map(|field| field.trim().to_owned());
            let Some(property) = fields.next() else {
                continue;
            };
            properties
                .entry(property)
                .or_default()
                .push(fields.collect());
        }
        Ok(Self { properties })
    }

    pub(crate) fn values(&self, property: &str) -> Result<ValueSet> {
        let names =
            self.properties.get(property).cloned().ok_or_else(|| {
                Error::Data(format!("no value aliases for property `{property}`"))
            })?;
        let mut index = HashMap::new();
        for (position, aliases) in names.iter().enumerate() {
            for alias in aliases {
                index.entry(loose(alias)).or_insert(position);
            }
        }
        Ok(ValueSet { names, index })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ValueSet {
    names: Vec<Vec<String>>,
    index: HashMap<String, usize>,
}

impl ValueSet {
    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }

    pub(crate) fn short(&self, value: usize) -> &str {
        &self.names[value][0]
    }

    pub(crate) fn long(&self, value: usize) -> &str {
        &self.names[value][1]
    }

    pub(crate) fn lookup(&self, file: &File<'_>, line: usize, alias: &str) -> Result<usize> {
        self.index
            .get(&loose(alias))
            .copied()
            .ok_or_else(|| file.error(line, format!("unknown property value `{alias}`")))
    }
}

fn loose(name: &str) -> String {
    name.chars()
        .filter(|character| !matches!(character, '_' | '-' | ' '))
        .flat_map(char::to_lowercase)
        .collect()
}
