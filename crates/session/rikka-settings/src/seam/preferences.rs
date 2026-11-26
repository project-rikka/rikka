use futures_util::Stream;
use rikka_types::Choice;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::schema::Schema;
use crate::seam::client::{Announcement, Settings};
use crate::seam::protocol::Changed;
use crate::value::{Value, Values};

#[derive(Debug)]
pub struct Preferences {
    schema: Schema,
    settings: Option<Settings>,
    unsaved: Values,
}

impl Preferences {
    #[must_use]
    pub fn open(schema: Schema) -> Self {
        let settings = Settings::open(schema.clone());
        Self::from_result(schema, settings)
    }

    #[must_use]
    pub fn local(schema: Schema) -> Self {
        let settings = Settings::local(schema.clone());
        Self::from_result(schema, settings)
    }

    #[must_use]
    pub fn in_memory(schema: Schema) -> Self {
        Self {
            schema,
            settings: None,
            unsaved: Values::new(),
        }
    }

    fn from_result(schema: Schema, result: Result<Settings>) -> Self {
        let settings = result
            .inspect_err(|error| {
                tracing::warn!(
                    "failed to open `{}`, using defaults: {error}",
                    schema.domain()
                );
            })
            .ok();
        Self {
            schema,
            settings,
            unsaved: Values::new(),
        }
    }

    #[must_use]
    pub fn get<T: DeserializeOwned + Default>(&self, key: &str) -> T {
        if let Some(value) = self.unsaved.get(key)
            && let Ok(read) = value.get()
        {
            return read;
        }
        if let Some(read) = self
            .settings
            .as_ref()
            .and_then(|settings| settings.read(key))
        {
            return read;
        }
        self.declared(key)
    }

    pub fn set(&mut self, key: &str, value: impl Serialize) -> bool {
        let value = match Value::from(&value) {
            Ok(value) => value,
            Err(error) => {
                tracing::warn!("rejected value for `{}/{key}`: {error}", self.domain());
                return false;
            }
        };
        let Some(settings) = self.settings.as_mut() else {
            self.unsaved.insert(key.to_owned(), value);
            return false;
        };
        match settings.set_value(key, value.clone()) {
            Ok(()) => {
                self.unsaved.remove(key);
                true
            }
            Err(error) => {
                tracing::warn!("failed to write `{}/{key}`: {error}", self.domain());
                if matches!(error, Error::WriteFile { .. } | Error::Session { .. }) {
                    self.unsaved.insert(key.to_owned(), value);
                }
                false
            }
        }
    }

    #[must_use]
    pub fn is_locked(&self, key: &str) -> bool {
        self.settings
            .as_ref()
            .is_some_and(|settings| settings.is_locked(key))
    }

    #[must_use]
    pub fn choice<C: Choice>(&self, key: &str) -> C {
        C::of_slug(&self.get::<String>(key))
    }

    pub fn set_choice<C: Choice>(&mut self, key: &str, choice: C) -> bool {
        if self.choice::<C>(key) == choice {
            return false;
        }
        self.set(key, choice.slug());
        true
    }

    pub fn refresh(&mut self) {
        let Some(settings) = self.settings.as_mut() else {
            return;
        };
        if let Err(error) = settings.refresh() {
            tracing::warn!(
                "failed to refresh `{}` from session: {error}",
                settings.domain()
            );
        }
    }

    pub fn apply_change(&mut self, changed: &Changed) -> bool {
        self.settings
            .as_mut()
            .is_some_and(|settings| settings.apply_change(changed))
    }

    #[must_use]
    pub fn changes(&self) -> Option<impl Stream<Item = Announcement> + Send + 'static> {
        self.settings.as_ref()?.changes()
    }

    fn domain(&self) -> &str {
        self.schema.domain()
    }

    fn declared<T: DeserializeOwned + Default>(&self, key: &str) -> T {
        let declared = self
            .schema
            .require(key)
            .map_err(|error| error.to_string())
            .and_then(|declared| {
                declared
                    .default_value()
                    .get()
                    .map_err(|error| error.to_string())
            });
        declared.unwrap_or_else(|error| {
            tracing::warn!("no default for `{}/{key}`: {error}", self.domain());
            T::default()
        })
    }
}
