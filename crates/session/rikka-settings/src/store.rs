use std::path::Path;

use rikka_types::Choice;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::atomic;
use crate::domain::Domain;
use crate::error::{Error, Result};
use crate::key::Key;
use crate::layers::{Layers, Source};
use crate::report::LoadReport;
use crate::schema::Schema;
use crate::scribe;
use crate::snapshot::Snapshot;
use crate::value::{Value, Values};

#[derive(Debug)]
pub struct Store {
    schema: Schema,
    domain: Domain,
    layers: Layers,
    system: Values,
    user: Values,
    unknown: Values,
}

impl Store {
    pub fn open(schema: Schema) -> Result<Self> {
        let (store, report) = Self::load(schema, Layers::resolve()?)?;
        report.log_warnings(&store.domain);
        Ok(store)
    }

    pub fn load(schema: Schema, layers: Layers) -> Result<(Self, LoadReport)> {
        let domain = schema.validate()?;
        let mut store = Self {
            schema,
            domain,
            layers,
            system: Values::new(),
            user: Values::new(),
            unknown: Values::new(),
        };
        let report = store.read_layers()?;
        Ok((store, report))
    }

    pub fn reload(&mut self) -> Result<LoadReport> {
        self.read_layers()
    }

    #[must_use]
    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    #[must_use]
    pub fn domain(&self) -> &Domain {
        &self.domain
    }

    #[must_use]
    pub fn unknown(&self) -> &Values {
        &self.unknown
    }

    pub fn value(&self, key: &str) -> Result<&Value> {
        Ok(self.resolved(self.schema.require(key)?))
    }

    fn resolved<'a>(&'a self, declared: &'a Key) -> &'a Value {
        self.user
            .get(declared.name())
            .or_else(|| self.system.get(declared.name()))
            .unwrap_or_else(|| declared.default_value())
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        self.value(key)?.get()
    }

    pub fn read<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get(key)
            .inspect_err(|error| tracing::warn!("failed to read `{}/{key}`: {error}", self.domain))
            .ok()
    }

    pub fn read_or<T: DeserializeOwned>(&self, key: &str, declared: T) -> T {
        self.read(key).unwrap_or(declared)
    }

    pub fn choice<C: Choice>(&self, key: &str) -> C {
        self.read::<String>(key)
            .map(|slug| C::of_slug(&slug))
            .unwrap_or_default()
    }
    pub fn source(&self, key: &str) -> Result<Source> {
        self.schema.require(key)?;
        Ok(self.source_of(key))
    }

    fn source_of(&self, name: &str) -> Source {
        if self.user.contains_key(name) {
            Source::User
        } else if self.system.contains_key(name) {
            Source::System
        } else {
            Source::Default
        }
    }

    pub fn set(&mut self, key: &str, value: impl Serialize) -> Result<()> {
        self.set_value(key, Value::from(&value)?)
    }

    pub fn set_choice<C: Choice>(&mut self, key: &str, choice: C) -> Result<()> {
        self.set_value(key, Value::String(choice.slug().to_owned()))
    }

    pub fn set_value(&mut self, key: &str, value: Value) -> Result<()> {
        let accepted = self
            .schema
            .require(key)?
            .accepts(value)
            .map_err(|reason| Error::refused(key, reason))?;
        if self.user.get(key) == Some(&accepted) && self.in_step() {
            return Ok(());
        }
        self.user.insert(key.to_owned(), accepted);
        self.persist()
    }

    pub fn reset(&mut self, key: &str) -> Result<()> {
        self.schema.require(key)?;
        if self.user.remove(key).is_none() && self.in_step() {
            return Ok(());
        }
        self.persist()
    }

    fn in_step(&self) -> bool {
        !scribe::failed(&self.layers.user_file(&self.domain))
    }

    #[must_use]
    pub fn user_values(&self) -> &Values {
        &self.user
    }

    pub fn set_user_values(&mut self, values: Values) -> LoadReport {
        let mut report = LoadReport::default();
        let mut valid = Values::new();
        for (name, value) in values {
            let Some(declared) = self.schema.key_named(&name) else {
                continue;
            };
            match declared.accepts(value) {
                Ok(accepted) => {
                    valid.insert(name, accepted);
                }
                Err(reason) => {
                    report.reject(&self.layers.user_file(&self.domain), Some(&name), reason)
                }
            }
        }
        self.user = valid;
        report
    }

    #[must_use]
    pub fn sources(&self) -> std::collections::BTreeMap<String, Source> {
        self.schema
            .keys()
            .iter()
            .map(|key| (key.name().to_owned(), self.source_of(key.name())))
            .collect()
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let values = self
            .schema
            .keys()
            .iter()
            .map(|key| (key.name().to_owned(), self.resolved(key).clone()))
            .collect();
        Snapshot::new(self.domain.clone(), values)
    }

    fn read_layers(&mut self) -> Result<LoadReport> {
        let mut report = LoadReport::default();
        let system_file = self.layers.system_file(&self.domain);
        let system_document = atomic::read(&system_file).map_err(|source| Error::ReadFile {
            path: system_file.clone(),
            source,
        })?;
        let system = self.read_layer(&system_file, system_document, None, &mut report);
        let user_file = self.layers.user_file(&self.domain);
        let user_document =
            self.layers
                .read_user(&self.domain)
                .map_err(|source| Error::ReadFile {
                    path: user_file.clone(),
                    source,
                })?;
        let mut unknown = Values::new();
        let user = self.read_layer(&user_file, user_document, Some(&mut unknown), &mut report);
        self.system = system;
        self.user = user;
        self.unknown = unknown;
        Ok(report)
    }

    fn read_layer(
        &self,
        path: &Path,
        document: Option<String>,
        carry: Option<&mut Values>,
        report: &mut LoadReport,
    ) -> Values {
        let Some(contents) = document else {
            return Values::new();
        };

        let parsed: toml::Table = match toml::from_str(&contents) {
            Ok(parsed) => parsed,
            Err(error) => {
                report.reject(path, None, error.to_string());
                return Values::new();
            }
        };

        let mut valid = Values::new();
        let mut carry = carry;
        for (name, raw) in parsed {
            let value = match Value::from_toml(raw) {
                Ok(value) => value,
                Err(reason) => {
                    report.reject(path, Some(&name), reason);
                    continue;
                }
            };
            match self.schema.key_named(&name) {
                Some(declared) => match declared.accepts(value) {
                    Ok(accepted) => {
                        valid.insert(name, accepted);
                    }
                    Err(reason) => report.reject(path, Some(&name), reason),
                },
                None => match &mut carry {
                    Some(carry) => {
                        tracing::debug!(
                            domain = %self.domain,
                            setting = %name,
                            "preserving undeclared setting",
                        );
                        carry.insert(name, value);
                    }
                    None => {
                        report.reject(path, Some(&name), "is not a setting of this domain".into());
                    }
                },
            }
        }
        valid
    }

    fn persist(&self) -> Result<()> {
        let document = match self.user.is_empty() && self.unknown.is_empty() {
            true => None,
            false => Some(self.encoded()?),
        };
        match self.layers.write_user(&self.domain, document) {
            Some(source) => Err(Error::WriteFile {
                path: self.layers.user_file(&self.domain),
                source,
            }),
            None => Ok(()),
        }
    }

    fn encoded(&self) -> Result<String> {
        let mut document = toml::Table::new();
        for (name, value) in self.unknown.iter().chain(&self.user) {
            document.insert(name.clone(), value.to_toml());
        }
        toml::ser::to_string_pretty(&document).map_err(|source| Error::Encode {
            domain: self.domain.as_str().to_owned(),
            source: Box::new(source),
        })
    }
}

impl Drop for Store {
    fn drop(&mut self) {
        scribe::wait_for_writes(&self.layers.user_file(&self.domain));
    }
}
