use rikka_app::prelude::*;
use rikka_settings::seam::{Announcement, Settings};
use rikka_settings::{Error, Schema, Value};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::session::Session;

const FOLLOWING: &str = "settings.domain.following";

pub(crate) trait DomainPage: 'static {
    fn domain(&self) -> &Domain;

    fn domain_mut(&mut self) -> &mut Domain;

    fn announced(&mut self, announcement: Announcement) {
        if !self.domain_mut().follow(announcement) {
            skip_view();
        }
    }
}

macro_rules! domain_page {
    ($page:ty) => {
        impl $crate::domain::DomainPage for $page {
            fn domain(&self) -> &$crate::domain::Domain {
                &self.domain
            }

            fn domain_mut(&mut self) -> &mut $crate::domain::Domain {
                &mut self.domain
            }
        }
    };
}

pub(crate) use domain_page;

#[derive(Debug)]
pub(crate) struct Domain {
    name: &'static str,
    settings: Option<Result<Settings, Error>>,
    error: Option<Error>,
}

impl Domain {
    pub(crate) fn new(name: &'static str) -> Self {
        Self {
            name,
            settings: None,
            error: None,
        }
    }

    pub(crate) fn open<P: DomainPage>(&mut self, session: &Session) {
        if matches!(self.settings, Some(Ok(_))) {
            return;
        }
        let adopted = session
            .seam(self.name)
            .and_then(|seam| Settings::adopt_with_seam(self.name, seam));
        if let Some(changes) = adopted.as_ref().ok().and_then(Settings::changes) {
            stream_keyed(
                (FOLLOWING, self.name),
                changes,
                |page: &mut P, announcement| {
                    page.announced(announcement);
                },
            );
        }
        self.settings = Some(adopted);
    }

    pub(crate) fn follow(&mut self, announcement: Announcement) -> bool {
        let Some(Ok(settings)) = &mut self.settings else {
            return false;
        };
        match announcement {
            Announcement::Changed(changed) => {
                changed.user_values != *settings.user_values() && settings.apply_change(&changed)
            }
            Announcement::Lagged => settings.refresh().is_ok(),
        }
    }

    pub(crate) fn unavailable(&self) -> Option<&Error> {
        self.settings.as_ref()?.as_ref().err()
    }

    pub(crate) fn error(&self) -> Option<&Error> {
        self.error.as_ref()
    }

    pub(crate) fn schema(&self) -> Option<&Schema> {
        self.adopted().map(Settings::schema)
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.adopted()?.get(key).ok()
    }

    pub(crate) fn non_empty_string(&self, key: &str) -> Option<String> {
        self.get::<String>(key).filter(|value| !value.is_empty())
    }

    pub(crate) fn one_of(
        &self,
        key: &str,
        options: &'static [(&'static str, &'static str)],
    ) -> &'static str {
        let stored = self.non_empty_string(key).unwrap_or_default();
        options
            .iter()
            .find(|(value, _)| *value == stored)
            .map_or(options[0].0, |(value, _)| *value)
    }

    pub(crate) fn value(&self, key: &str) -> Option<Value> {
        self.adopted()?.value(key).ok().cloned()
    }

    pub(crate) fn set(&mut self, key: &str, value: impl Serialize) {
        self.write(|settings| settings.set(key, value));
    }

    pub(crate) fn set_value(&mut self, key: &str, value: Value) {
        self.write(|settings| settings.set_value(key, value));
    }

    pub(crate) fn reset(&mut self, key: &str) {
        self.write(|settings| settings.reset(key));
    }

    fn adopted(&self) -> Option<&Settings> {
        self.settings.as_ref()?.as_ref().ok()
    }

    fn write(&mut self, write: impl FnOnce(&mut Settings) -> rikka_settings::Result<()>) {
        let Some(Ok(settings)) = &mut self.settings else {
            return;
        };
        self.error = write(settings).err();
    }
}
