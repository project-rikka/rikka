use toml::Table;

use crate::error::{Error, Result};
use crate::front_matter::FrontMatter;

const FENCE: &str = "+++";

pub(crate) struct Document {
    stem: String,
    front: Table,
    body: String,
}

impl Document {
    pub(crate) fn parse(stem: &str, text: &str) -> Result<Document> {
        let (front, body) = split(stem, text.trim_start_matches('\u{feff}'))?;
        let front = front.parse::<Table>().map_err(|error| Error::Malformed {
            document: stem.to_owned(),
            reason: error.to_string(),
        })?;

        Ok(Document {
            stem: stem.to_owned(),
            front,
            body: body.trim().to_owned(),
        })
    }

    pub(crate) fn stem(&self) -> &str {
        &self.stem
    }

    pub(crate) fn into_body(self) -> String {
        self.body
    }

    pub(crate) fn string(&self, key: &str) -> Result<Option<String>> {
        match self.front.get(key) {
            None => Ok(None),
            Some(toml::Value::String(value)) => Ok(Some(value.clone())),
            Some(_) => Err(self.malformed(format!("\"{key}\" is not a string"))),
        }
    }

    pub(crate) fn flag(&self, key: &str) -> Result<bool> {
        match self.front.get(key) {
            None => Ok(false),
            Some(toml::Value::Boolean(flag)) => Ok(*flag),
            Some(_) => Err(self.malformed(format!("\"{key}\" is not true or false"))),
        }
    }

    pub(crate) fn strings(&self, key: &str) -> Result<Vec<(String, String)>> {
        self.table(key)?
            .into_iter()
            .map(|(name, value)| match value {
                toml::Value::String(value) => Ok((name, value)),
                _ => Err(self.malformed(format!("[{key}] {name} is not a string"))),
            })
            .collect()
    }

    pub(crate) fn integers(&self, key: &str) -> Result<Vec<(String, u64)>> {
        self.table(key)?
            .into_iter()
            .map(
                |(name, value)| match value.as_integer().and_then(|it| u64::try_from(it).ok()) {
                    Some(value) => Ok((name, value)),
                    None => {
                        Err(self
                            .malformed(format!("[{key}] {name} is not a positive whole number")))
                    }
                },
            )
            .collect()
    }

    fn table(&self, key: &str) -> Result<Table> {
        match self.front.get(key) {
            None => Ok(Table::new()),
            Some(toml::Value::Table(table)) => Ok(table.clone()),
            Some(_) => Err(self.malformed(format!("\"{key}\" is not a table"))),
        }
    }

    fn malformed(&self, reason: String) -> Error {
        Error::Malformed {
            document: self.stem.clone(),
            reason,
        }
    }
}

fn split<'a>(stem: &str, text: &'a str) -> Result<(&'a str, &'a str)> {
    match FrontMatter::split(text, FENCE) {
        FrontMatter::Absent => Ok(("", text)),
        FrontMatter::Fenced { front, body } => Ok((front, body)),
        FrontMatter::Unclosed => Err(Error::Malformed {
            document: stem.to_owned(),
            reason: format!("the front matter is never closed with {FENCE}"),
        }),
    }
}

impl std::fmt::Debug for Document {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Document")
            .field("stem", &self.stem)
            .finish_non_exhaustive()
    }
}
