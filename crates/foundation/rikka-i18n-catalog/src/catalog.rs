use std::fs;
use std::path::Path;

use rustc_hash::FxHashMap;

use crate::error::{Error, Result};
use crate::locale::Locale;
use crate::message::Message;

const METADATA: &str = "i18n";

#[derive(Clone, Debug, Default)]
pub struct Catalog {
    locale: Locale,
    name: Option<Box<str>>,
    fallback: Option<Locale>,
    messages: FxHashMap<Box<str>, Message>,
}

impl Catalog {
    pub fn parse(source_name: &str, source: &str) -> Result<Self> {
        let mut table: toml::Table = toml::from_str(source).map_err(|source| Error::Parse {
            source_name: source_name.to_owned(),
            source: Box::new(source),
        })?;

        let metadata = table.remove(METADATA);
        let mut catalog = Self::default();
        let declared = match metadata {
            Some(metadata) => catalog.read_metadata(source_name, &metadata)?,
            None => None,
        };
        catalog.locale =
            declared
                .or_else(|| locale_of(source_name))
                .ok_or_else(|| Error::UnknownLocale {
                    source_name: source_name.to_owned(),
                })?;

        let mut path = String::new();
        catalog.absorb(source_name, &mut path, table)?;
        Ok(catalog)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let source = fs::read_to_string(path).map_err(|source| Error::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&path.display().to_string(), &source)
    }

    pub fn locale(&self) -> &Locale {
        &self.locale
    }

    pub fn display_name(&self) -> &str {
        self.name.as_deref().unwrap_or(self.locale.language())
    }

    pub fn fallback(&self) -> Option<&Locale> {
        self.fallback.as_ref()
    }

    pub fn get(&self, key: &str) -> Option<&Message> {
        self.messages.get(key)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.messages.keys().map(|key| &**key)
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn merge(&mut self, other: Self) {
        self.messages.extend(other.messages);
        if other.name.is_some() {
            self.name = other.name;
        }
        if other.fallback.is_some() {
            self.fallback = other.fallback;
        }
    }

    fn read_metadata(
        &mut self,
        source_name: &str,
        metadata: &toml::Value,
    ) -> Result<Option<Locale>> {
        let unexpected = |key: &str, found: &'static str| Error::UnexpectedValue {
            source_name: source_name.to_owned(),
            key: format!("{METADATA}.{key}"),
            found,
        };
        let table = metadata
            .as_table()
            .ok_or_else(|| unexpected("", metadata.type_str()))?;

        let mut locale = None;
        for (key, value) in table {
            let text = value
                .as_str()
                .ok_or_else(|| unexpected(key, value.type_str()))?;
            match key.as_str() {
                "locale" => locale = Some(text.parse()?),
                "fallback" => self.fallback = Some(text.parse()?),
                "name" => self.name = Some(text.into()),
                _ => {
                    return Err(Error::UnknownMetadataKey {
                        source_name: source_name.to_owned(),
                        key: format!("{METADATA}.{key}"),
                    });
                }
            }
        }
        Ok(locale)
    }

    fn absorb(&mut self, source_name: &str, path: &mut String, table: toml::Table) -> Result<()> {
        for (key, value) in table {
            let restore = path.len();
            if !path.is_empty() {
                path.push('.');
            }
            path.push_str(&key);

            match value {
                toml::Value::String(source) => {
                    let message = Message::parse(&source).map_err(|source| Error::Message {
                        source_name: source_name.to_owned(),
                        key: path.clone(),
                        source,
                    })?;
                    self.messages.insert(path.as_str().into(), message);
                }
                toml::Value::Table(nested) => self.absorb(source_name, path, nested)?,
                other => {
                    return Err(Error::UnexpectedValue {
                        source_name: source_name.to_owned(),
                        key: path.clone(),
                        found: other.type_str(),
                    });
                }
            }
            path.truncate(restore);
        }
        Ok(())
    }
}

pub(crate) fn locale_of(source_name: &str) -> Option<Locale> {
    Path::new(source_name).file_stem()?.to_str()?.parse().ok()
}
