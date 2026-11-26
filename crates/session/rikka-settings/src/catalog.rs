use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::atomic;
use crate::domain::Domain;
use crate::error::{Error, Result};
use crate::key::Key;
use crate::layers::Layers;
use crate::report::LoadReport;
use crate::schema::Schema;
use crate::store::Store;

const INSTANCE_SEPARATOR: char = '-';

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Catalog {
    domains: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    #[serde(flatten)]
    schema: Schema,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    instances: Option<Box<str>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting<'a> {
    pub instance: Option<&'a str>,
    pub key: &'a Key,
}

impl Catalog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn schema(mut self, schema: Schema) -> Self {
        self.domains.push(Entry {
            schema,
            instances: None,
        });
        self
    }

    #[must_use]
    pub fn instanced(mut self, noun: &str, schema: Schema) -> Self {
        self.domains.push(Entry {
            schema,
            instances: Some(noun.into()),
        });
        self
    }

    pub fn merge(parts: impl IntoIterator<Item = Catalog>) -> Result<Self> {
        let mut domains: Vec<Entry> = parts.into_iter().flat_map(|part| part.domains).collect();
        domains.sort_by(|left, right| left.domain().cmp(right.domain()));
        let merged = Self { domains };
        merged.validate()?;
        Ok(merged)
    }

    pub fn from_json(text: &str) -> Result<Self> {
        let catalog: Self =
            serde_json::from_str(text).map_err(|error| Error::Catalog(error.to_string()))?;
        catalog.validate()?;
        Ok(catalog)
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self)
            .unwrap_or_else(|error| unreachable!("a catalog is plain data: {error}"));
        text.push('\n');
        text
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.domains
    }

    #[must_use]
    pub fn entry(&self, domain: &str) -> Option<&Entry> {
        self.domains.iter().find(|entry| entry.domain() == domain)
    }

    fn validate(&self) -> Result<()> {
        let mut seen = BTreeSet::new();
        for entry in &self.domains {
            entry.schema.validate()?;
            if !seen.insert(entry.domain()) {
                return Err(Error::DuplicateDomain(entry.domain().to_owned()));
            }
        }
        Ok(())
    }
}

impl From<Schema> for Catalog {
    fn from(schema: Schema) -> Self {
        Self::new().schema(schema)
    }
}

impl Entry {
    #[must_use]
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        self.schema.domain()
    }

    #[must_use]
    pub fn instances(&self) -> Option<&str> {
        self.instances.as_deref()
    }

    #[must_use]
    pub fn setting<'a>(&'a self, name: &'a str) -> Option<Setting<'a>> {
        if self.instances.is_none() {
            return self.schema.key_named(name).map(|key| Setting {
                instance: None,
                key,
            });
        }
        self.schema
            .keys()
            .iter()
            .filter_map(|key| {
                let instance = name
                    .strip_suffix(key.name())?
                    .strip_suffix(INSTANCE_SEPARATOR)?;
                (!instance.is_empty()).then_some(Setting {
                    instance: Some(instance),
                    key,
                })
            })
            .max_by_key(|setting| setting.key.name().len())
    }

    #[must_use]
    pub fn instance_schema<I: AsRef<str>>(&self, instances: impl IntoIterator<Item = I>) -> Schema {
        let mut schema = Schema::new(self.domain());
        if let Some(summary) = self.schema.summary_text() {
            schema = schema.summary(summary);
        }
        for instance in instances {
            for key in self.schema.keys() {
                schema = schema.key(key.renamed(instance_key(instance.as_ref(), key.name())));
            }
        }
        schema
    }

    pub fn open(&self, layers: Layers) -> Result<(Store, LoadReport)> {
        let schema = match self.instances {
            None => self.schema.clone(),
            Some(_) => self.instance_schema(self.named_instances(&layers)?),
        };
        Store::load(schema, layers)
    }

    fn named_instances(&self, layers: &Layers) -> Result<BTreeSet<String>> {
        let domain: Domain = self.domain().parse()?;
        let user = layers.user_file(&domain);
        let mut documents = vec![
            layers
                .read_user(&domain)
                .map_err(|source| Error::ReadFile { path: user, source })?,
        ];
        let mut files: Vec<PathBuf> = vec![layers.system_file(&domain)];
        files.extend(
            layers
                .declared_files(&domain)
                .flat_map(|declared| [declared.values, declared.locked]),
        );
        for file in files {
            documents.push(atomic::read(&file).map_err(|source| Error::ReadFile {
                path: file.clone(),
                source,
            })?);
        }
        let mut instances = BTreeSet::new();
        for document in documents.into_iter().flatten() {
            let Ok(table) = document.parse::<toml::Table>() else {
                continue;
            };
            instances.extend(table.keys().filter_map(|name| {
                self.setting(name)
                    .and_then(|setting| setting.instance)
                    .map(str::to_owned)
            }));
        }
        Ok(instances)
    }
}

#[must_use]
pub fn instance_key(instance: &str, suffix: &str) -> String {
    format!("{instance}{INSTANCE_SEPARATOR}{suffix}")
}

pub fn print(catalog: impl Into<Catalog>) -> ExitCode {
    let catalog: Catalog = catalog.into();
    if let Err(error) = catalog.validate() {
        eprintln!("invalid settings catalog: {error}");
        return ExitCode::FAILURE;
    }
    match std::io::stdout()
        .lock()
        .write_all(catalog.to_json().as_bytes())
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("failed to write the settings catalog: {error}");
            ExitCode::FAILURE
        }
    }
}
