use std::collections::BTreeMap;
use std::path::Path;

use rikka_types::Choice;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::atomic;
use crate::domain::Domain;
use crate::error::{Error, Result};
use crate::key::Key;
use crate::layers::Layers;
use crate::report::LoadReport;
use crate::schema::Schema;
use crate::scribe;
use crate::snapshot::Snapshot;
use crate::source::Source;
use crate::value::{Value, Values};

#[derive(Debug)]
pub struct Store {
    schema: Schema,
    domain: Domain,
    layers: Layers,
    beneath: Vec<Layer>,
    user: Values,
    locked: Vec<Layer>,
    unknown: Values,
}

#[derive(Debug)]
struct Layer {
    source: Source,
    values: Values,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resolved<'a> {
    pub source: Source,
    pub value: &'a Value,
}

enum Undeclared<'a> {
    Reject,
    Skip,
    Carry(&'a mut Values),
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
            beneath: Vec::new(),
            user: Values::new(),
            locked: Vec::new(),
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
    pub fn layers(&self) -> &Layers {
        &self.layers
    }

    #[must_use]
    pub fn unknown(&self) -> &Values {
        &self.unknown
    }

    pub fn value(&self, key: &str) -> Result<&Value> {
        Ok(self.resolved(self.schema.require(key)?).value)
    }

    pub fn resolve(&self, key: &str) -> Result<Resolved<'_>> {
        Ok(self.resolved(self.schema.require(key)?))
    }

    pub fn beneath_user(&self, key: &str) -> Result<Resolved<'_>> {
        Ok(self.beneath_user_of(self.schema.require(key)?))
    }

    fn resolved<'a>(&'a self, declared: &'a Key) -> Resolved<'a> {
        let name = declared.name();
        if let Some(locked) = found(&self.locked, name) {
            return locked;
        }
        match self.user.get(name) {
            Some(value) => Resolved {
                source: Source::User,
                value,
            },
            None => self.beneath_user_of(declared),
        }
    }

    fn beneath_user_of<'a>(&'a self, declared: &'a Key) -> Resolved<'a> {
        found(&self.beneath, declared.name()).unwrap_or(Resolved {
            source: Source::Default,
            value: declared.default_value(),
        })
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
        Ok(self.resolve(key)?.source)
    }

    #[must_use]
    pub fn is_locked(&self, key: &str) -> bool {
        found(&self.locked, key).is_some()
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
        self.refuse_locked(key)?;
        if self.user.get(key) == Some(&accepted) && self.in_step() {
            return Ok(());
        }
        self.user.insert(key.to_owned(), accepted);
        self.persist()
    }

    pub fn reset(&mut self, key: &str) -> Result<()> {
        self.schema.require(key)?;
        self.refuse_locked(key)?;
        if self.user.remove(key).is_none() && self.in_step() {
            return Ok(());
        }
        self.persist()
    }

    fn refuse_locked(&self, key: &str) -> Result<()> {
        match self.is_locked(key) {
            true => Err(Error::Locked {
                domain: self.domain.as_str().to_owned(),
                key: key.to_owned(),
            }),
            false => Ok(()),
        }
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
    pub fn sources(&self) -> BTreeMap<String, Source> {
        self.schema
            .keys()
            .iter()
            .map(|key| (key.name().to_owned(), self.resolved(key).source))
            .collect()
    }

    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let values = self
            .schema
            .keys()
            .iter()
            .map(|key| (key.name().to_owned(), self.resolved(key).value.clone()))
            .collect();
        Snapshot::new(self.domain.clone(), values)
    }

    fn read_layers(&mut self) -> Result<LoadReport> {
        let mut report = LoadReport::default();
        let system_file = self.layers.system_file(&self.domain);
        let mut beneath = vec![Layer {
            source: Source::System,
            values: self.read_file(&system_file, Undeclared::Reject, &mut report)?,
        }];
        let mut locked = Vec::new();
        for files in self.layers.declared_files(&self.domain) {
            beneath.push(Layer {
                source: Source::Declared(files.scope),
                values: self.read_file(&files.values, Undeclared::Skip, &mut report)?,
            });
            locked.push(Layer {
                source: Source::Locked(files.scope),
                values: self.read_file(&files.locked, Undeclared::Skip, &mut report)?,
            });
        }
        let user_file = self.layers.user_file(&self.domain);
        let user_document =
            self.layers
                .read_user(&self.domain)
                .map_err(|source| Error::ReadFile {
                    path: user_file.clone(),
                    source,
                })?;
        let mut unknown = Values::new();
        let user = self.read_layer(
            &user_file,
            user_document,
            Undeclared::Carry(&mut unknown),
            &mut report,
        );
        self.beneath = beneath;
        self.locked = locked;
        self.user = user;
        self.unknown = unknown;
        Ok(report)
    }

    fn read_file(
        &self,
        path: &Path,
        undeclared: Undeclared<'_>,
        report: &mut LoadReport,
    ) -> Result<Values> {
        let document = atomic::read(path).map_err(|source| Error::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(self.read_layer(path, document, undeclared, report))
    }

    fn read_layer(
        &self,
        path: &Path,
        document: Option<String>,
        mut undeclared: Undeclared<'_>,
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
                None => match &mut undeclared {
                    Undeclared::Reject => {
                        report.reject(path, Some(&name), "is not a setting of this domain".into());
                    }
                    Undeclared::Skip => {
                        tracing::debug!(
                            domain = %self.domain,
                            setting = %name,
                            path = %path.display(),
                            "skipping a declared setting this domain does not have",
                        );
                    }
                    Undeclared::Carry(carry) => {
                        tracing::debug!(
                            domain = %self.domain,
                            setting = %name,
                            "preserving undeclared setting",
                        );
                        carry.insert(name, value);
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

fn found<'a>(layers: &'a [Layer], name: &str) -> Option<Resolved<'a>> {
    layers.iter().rev().find_map(|layer| {
        layer.values.get(name).map(|value| Resolved {
            source: layer.source,
            value,
        })
    })
}

impl Drop for Store {
    fn drop(&mut self) {
        scribe::wait_for_writes(&self.layers.user_file(&self.domain));
    }
}
